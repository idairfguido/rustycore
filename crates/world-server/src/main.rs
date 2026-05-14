// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! World Server — entry point.
//!
//! Accepts WoW client connections after BNet authentication, performs the
//! world-server handshake (challenge → auth → encryption), creates a
//! WorldSession for each client, and dispatches packets to handlers.

use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{info, warn};
use wow_config::{DatabaseInfo, LoadReport, WorldConfigSet};
use wow_core::{ObjectGuidGenerator, guid::HighGuid};
use wow_database::{
    CharStatements, CharacterDatabase, HotfixDatabase, LoginDatabase, LoginStatements,
    WorldDatabase, WorldStatements, build_connection_string,
};
use wow_instances::{InstanceLockMgr, MapDb2Entries, MapDifficultyResetInterval};
use wow_loot::{
    LootConditionId, LootConditionLinkReport, LootConditionReferenceUseLikeCpp,
    LootReferenceCheckReport, LootStore, LootStoreKind, LootStores, LootTemplateRow,
    check_loot_condition_links_like_cpp, check_loot_condition_references_like_cpp,
    check_loot_references_like_cpp, loot_store_kind_for_condition_source_type_like_cpp,
};
use wow_network::session_mgr::SessionManager;
use wow_network::world_socket::{AccountInfo, AccountLookup};
use wow_network::{
    GroupRegistry, LootDropRatesLikeCpp, PendingInvites, PlayerRegistry, SessionResources,
};
use wow_world::{
    MMapRuntimeConfigLikeCpp, MapManager as LegacyMapManager, SharedMapManager,
    WorldMMapPathfinderWorkerLikeCpp, WorldSession,
};

type SharedCanonicalMapManager = Arc<Mutex<wow_map::MapManager>>;

const WORLD_CONFIG_CANDIDATES: &[&str] = &[
    "worldserver.conf",
    "worldserver.conf.dist",
    "WorldServer.conf",
    "WorldServer.conf.dist",
];
const WORLD_CONFIG_DIR: &str = "worldserver.conf.d";

// ── Account lookup implementation ────────────────────────────────

/// Looks up account information from the login database using the realm join ticket.
///
/// The realm join ticket sent by the client in AuthSession is actually the game
/// account username (e.g. "2#1"), NOT the BNet LoginTicket (TC-xxx). The C#
/// RustyCore WorldSocket.HandleAuthSession uses SEL_ACCOUNT_INFO_BY_NAME with
/// `WHERE a.username = ?` to look it up directly.
struct DbAccountLookup {
    login_db: Arc<LoginDatabase>,
    realm_id: u16,
    win64_auth_seed: [u8; 16],
}

impl AccountLookup for DbAccountLookup {
    fn lookup_account(
        &self,
        realm_join_ticket: &str,
    ) -> Pin<Box<dyn Future<Output = Option<AccountInfo>> + Send + '_>> {
        let ticket = realm_join_ticket.to_owned();
        let realm_id = self.realm_id;
        Box::pin(async move {
            // The realm_join_ticket is the game account username (e.g. "2#1").
            // Query SEL_ACCOUNT_INFO_BY_NAME: params are (RealmID, username).
            //
            // Columns returned:
            //  0: a.id                  (account_id)
            //  1: a.session_key_bnet    (hex session key)
            //  2: ba.last_ip
            //  3: ba.locked
            //  4: ba.lock_country
            //  5: a.expansion
            //  6: a.mutetime
            //  7: ba.locale
            //  8: a.recruiter
            //  9: a.os
            // 10: a.timezone_offset
            // 11: ba.id                 (battlenet_account_id)
            // 12: aa.SecurityLevel
            // 13: bab ban expr          (is_banned_bnet)
            // 14: ab ban expr           (is_banned_account)
            // 15: r.id                  (recruiter)
            let mut stmt = self
                .login_db
                .prepare(LoginStatements::SEL_ACCOUNT_INFO_BY_NAME);
            stmt.set_i32(0, i32::from(realm_id));
            stmt.set_string(1, &ticket);

            let result = match self.login_db.query(&stmt).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("DB error looking up account by name '{ticket}': {e}");
                    return None;
                }
            };

            if result.is_empty() {
                tracing::warn!("No account found for realm_join_ticket '{ticket}'");
                return None;
            }

            let account_id: u32 = result.read(0);
            // session_key_bnet is varbinary(64) — read as raw bytes, then hex-encode
            let session_key_raw: Vec<u8> = result.try_read(1).unwrap_or_default();
            let session_key_hex: String =
                session_key_raw.iter().map(|b| format!("{b:02X}")).collect();
            let last_ip: String = result.try_read(2).unwrap_or_default();
            let is_locked: u8 = result.try_read(3).unwrap_or(0);
            let lock_country: String = result.try_read(4).unwrap_or_default();
            let expansion: u8 = result.try_read(5).unwrap_or(2);
            let _mutetime: i64 = result.try_read(6).unwrap_or(0);
            let locale_raw: String = result
                .try_read::<u8>(7)
                .map(|v| v.to_string())
                .unwrap_or_else(|| result.try_read::<String>(7).unwrap_or_default());
            let recruiter: u32 = result.try_read(8).unwrap_or(0);
            let os: String = result.try_read(9).unwrap_or_default();
            let timezone_offset: i16 = result.try_read(10).unwrap_or(0);
            let bnet_id: u32 = result.try_read(11).unwrap_or(0);
            let security: u8 = result.try_read(12).unwrap_or(0);
            let is_banned_bnet: u32 = result.try_read(13).unwrap_or(0);
            let is_banned_account: u32 = result.try_read(14).unwrap_or(0);

            if account_id == 0 {
                tracing::warn!("Account id is 0 for ticket '{ticket}'");
                return None;
            }

            if session_key_hex.is_empty() {
                tracing::warn!("No session key for account {account_id} (ticket '{ticket}')");
                return None;
            }

            let locale_name = locale_id_to_name(&locale_raw);
            tracing::info!(
                "Account lookup OK: id={account_id}, bnet_id={bnet_id}, os={os}, locale_raw='{locale_raw}', locale='{locale_name}'"
            );

            Some(AccountInfo {
                id: account_id,
                session_key_hex,
                last_ip,
                is_locked_to_ip: is_locked != 0,
                lock_country,
                expansion,
                mute_time: 0,
                locale: locale_name,
                recruiter,
                os,
                timezone_offset: i32::from(timezone_offset),
                battlenet_account_id: bnet_id,
                security,
                is_banned_bnet: is_banned_bnet != 0,
                is_banned_account: is_banned_account != 0,
                win64_auth_seed: self.win64_auth_seed,
                client_address: None,            // Set by accept loop after auth
                derived_session_key: Vec::new(), // Set by accept loop after auth
            })
        })
    }
}

// ── Main ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("RustyCore World Server starting...");

    load_world_config()?;
    let world_configs = wow_config::load_world_config_values();

    // Connect to login database (needed for session key validation)
    let login_info = wow_config::get_database_info_default(
        "Login",
        DatabaseInfo::new("127.0.0.1", 3306, "trinity", "trinity", "auth"),
    );

    let conn_str = build_connection_string(
        &login_info.host,
        &login_info.port_or_socket,
        &login_info.username,
        &login_info.password,
        &login_info.database,
    );
    let login_db = LoginDatabase::open(&conn_str)
        .await
        .context("Failed to connect to login database")?;

    info!("Connected to login database");

    // Connect to character database
    let char_info = wow_config::get_database_info_default(
        "Character",
        DatabaseInfo::new("127.0.0.1", 3306, "trinity", "trinity", "characters"),
    );

    let char_conn_str = build_connection_string(
        &char_info.host,
        &char_info.port_or_socket,
        &char_info.username,
        &char_info.password,
        &char_info.database,
    );
    let char_db = CharacterDatabase::open(&char_conn_str)
        .await
        .context("Failed to connect to character database")?;

    info!("Connected to character database");

    // Connect to world database
    let world_info = wow_config::get_database_info_default(
        "World",
        DatabaseInfo::new("127.0.0.1", 3306, "trinity", "trinity", "world"),
    );

    let world_conn_str = build_connection_string(
        &world_info.host,
        &world_info.port_or_socket,
        &world_info.username,
        &world_info.password,
        &world_info.database,
    );
    let world_db = WorldDatabase::open(&world_conn_str)
        .await
        .context("Failed to connect to world database")?;

    info!("Connected to world database");
    let world_db = Arc::new(world_db);

    // Connect to hotfix database
    let hotfix_info = wow_config::get_database_info_default(
        "Hotfix",
        DatabaseInfo::new("127.0.0.1", 3306, "trinity", "trinity", "hotfixes"),
    );

    let hotfix_conn_str = build_connection_string(
        &hotfix_info.host,
        &hotfix_info.port_or_socket,
        &hotfix_info.username,
        &hotfix_info.password,
        &hotfix_info.database,
    );
    let hotfix_db = HotfixDatabase::open(&hotfix_conn_str)
        .await
        .context("Failed to connect to hotfix database")?;

    info!("Connected to hotfix database");

    // ── Database auto-update ──────────────────────────────────────────────
    let auto_setup = wow_config::get_string_default("Updates.AutoSetup", "1");
    if auto_setup != "0" && auto_setup.to_lowercase() != "false" {
        use wow_database::updater::DbUpdater;
        let src = wow_config::get_string_default("Updates.SourcePath", ".");

        let auth_up = DbUpdater::new(
            login_db.pool().clone(),
            &login_info.host,
            &login_info.port_or_socket,
            &login_info.username,
            &login_info.password,
            &login_info.database,
        );
        if let Err(e) = auth_up
            .populate(&format!("{src}/sql/base/auth_database.sql"))
            .await
        {
            tracing::warn!("Auth populate skipped: {e}");
        }
        if let Err(e) = auth_up.update(&src).await {
            tracing::warn!("Auth update error: {e}");
        }

        let char_up = DbUpdater::new(
            char_db.pool().clone(),
            &char_info.host,
            &char_info.port_or_socket,
            &char_info.username,
            &char_info.password,
            &char_info.database,
        );
        if let Err(e) = char_up
            .populate(&format!("{src}/sql/base/characters_database.sql"))
            .await
        {
            tracing::warn!("Characters populate skipped: {e}");
        }
        if let Err(e) = char_up.update(&src).await {
            tracing::warn!("Characters update error: {e}");
        }

        // world + hotfixes: only update (base SQL is the full TDB, downloaded separately)
        let world_up = DbUpdater::new(
            world_db.pool().clone(),
            &world_info.host,
            &world_info.port_or_socket,
            &world_info.username,
            &world_info.password,
            &world_info.database,
        );
        if let Err(e) = world_up.update(&src).await {
            tracing::warn!("World update error: {e}");
        }

        let hotfix_up = DbUpdater::new(
            hotfix_db.pool().clone(),
            &hotfix_info.host,
            &hotfix_info.port_or_socket,
            &hotfix_info.username,
            &hotfix_info.password,
            &hotfix_info.database,
        );
        if let Err(e) = hotfix_up.update(&src).await {
            tracing::warn!("Hotfix update error: {e}");
        }
    }
    // ─────────────────────────────────────────────────────────────────────

    let hotfix_db = Arc::new(hotfix_db);

    // Initialize GUID generator from MAX(guid) in characters table
    let max_guid = {
        let stmt = char_db.prepare(CharStatements::SEL_MAX_GUID);
        match char_db.query(&stmt).await {
            Ok(result) => {
                if result.is_empty() || result.is_null(0) {
                    1i64
                } else {
                    let max_val: u32 = result.try_read(0).unwrap_or(0);
                    (max_val as i64) + 1
                }
            }
            Err(_) => 1i64,
        }
    };

    let guid_generator = Arc::new(ObjectGuidGenerator::new(HighGuid::Player, max_guid));
    info!("GUID generator initialized, next counter: {max_guid}");

    let char_db = Arc::new(char_db);

    // Load Item.db2 for inventory_type lookups (replaces item_type_cache table)
    let data_dir = wow_config::get_string_default("DataDir", "./Data");
    let locale_raw = wow_config::get_string_default("DBC.Locale", "esES");
    let locale = locale_id_to_name(&locale_raw);
    let currency_types_store = Arc::new(
        wow_data::CurrencyTypesStore::load(&data_dir, &locale)
            .context("Failed to load CurrencyTypes.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} currencies from CurrencyTypes.db2",
        currency_types_store.len()
    );

    let import_price_stores = Arc::new(
        wow_data::ImportPriceStores::load(&data_dir, &locale)
            .context("Failed to load ImportPrice*.db2 — check DataDir and DBC.Locale config")?,
    );
    info!("Loaded ImportPrice*.db2 stores");

    let item_class_store = Arc::new(
        wow_data::ItemClassStore::load(&data_dir, &locale)
            .context("Failed to load ItemClass.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item classes from ItemClass.db2",
        item_class_store.len()
    );

    let item_currency_cost_store = Arc::new(
        wow_data::ItemCurrencyCostStore::load(&data_dir, &locale)
            .context("Failed to load ItemCurrencyCost.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item currency costs from ItemCurrencyCost.db2",
        item_currency_cost_store.len()
    );

    let item_extended_cost_store = Arc::new(
        wow_data::ItemExtendedCostStore::load(&data_dir, &locale)
            .context("Failed to load ItemExtendedCost.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item extended costs from ItemExtendedCost.db2",
        item_extended_cost_store.len()
    );

    let item_store = Arc::new(
        wow_data::ItemStore::load(&data_dir, &locale)
            .context("Failed to load Item.db2 — check DataDir and DBC.Locale config")?,
    );
    info!("Loaded {} items from Item.db2", item_store.len());

    let item_price_base_store = Arc::new(
        wow_data::ItemPriceBaseStore::load(&data_dir, &locale)
            .context("Failed to load ItemPriceBase.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item price base rows from ItemPriceBase.db2",
        item_price_base_store.len()
    );

    // Load ChrSpecialization.db2 for C++ loot-specialization validation.
    let chr_specialization_store = Arc::new(
        wow_data::ChrSpecializationStore::load(&data_dir, &locale).context(
            "Failed to load ChrSpecialization.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} chr specializations from ChrSpecialization.db2",
        chr_specialization_store.len()
    );

    // Load DungeonEncounter.db2 for C++ instance encounter lock/loot metadata.
    let dungeon_encounter_store = Arc::new(
        wow_data::DungeonEncounterStore::load(&data_dir, &locale)
            .context("Failed to load DungeonEncounter.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} dungeon encounters from DungeonEncounter.db2",
        dungeon_encounter_store.len()
    );

    // Load Map.db2 + MapDifficulty.db2 for C++ InstanceLockMgr MapDb2Entries resolution.
    let map_store = Arc::new(
        wow_data::MapStore::load(&data_dir, &locale)
            .context("Failed to load Map.db2 — check DataDir and DBC.Locale config")?,
    );
    info!("Loaded {} maps from Map.db2", map_store.len());
    let (world_safe_loc_store, world_safe_loc_report) =
        wow_data::WorldSafeLocStore::load_like_cpp(world_db.as_ref(), &map_store)
            .await
            .context("Failed to load C++ world_safe_locs")?;
    info!(
        "Loaded {} world safe locs ({} missing maps, {} invalid positions)",
        world_safe_loc_store.len(),
        world_safe_loc_report.missing_maps.len(),
        world_safe_loc_report.invalid_positions.len()
    );
    let ui_map_x_map_art_store = Arc::new(
        wow_data::UiMapXMapArtStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load UiMapXMapArt.db2 / hotfix rows")?,
    );
    let area_table_store = Arc::new(
        wow_data::AreaTableStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load AreaTable.db2 / hotfix rows")?,
    );
    let phase_store = Arc::new(
        wow_data::PhaseStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load Phase.db2 / hotfix rows")?,
    );
    let phase_group_store = Arc::new(
        wow_data::PhaseGroupStore::load_with_hotfixes(&data_dir, &locale, &phase_store, &hotfix_db)
            .await
            .context("Failed to load PhaseXPhaseGroup.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} phases and {} phase-group rows",
        phase_store.len(),
        phase_group_store.len()
    );
    let mut phase_info_store = wow_data::PhaseInfoStore::from_phase_store_like_cpp(&phase_store);
    phase_info_store
        .load_area_phases_like_cpp(world_db.as_ref(), &area_table_store, &phase_store)
        .await
        .context("Failed to load phase_area rows")?;
    info!(
        "Seeded {} phase info records and {} phase area rows",
        phase_info_store.phase_info_count(),
        phase_info_store.phase_area_count()
    );
    let terrain_swap_store = Arc::new(
        wow_data::load_terrain_swaps(world_db.as_ref(), &map_store, |phase_id| {
            ui_map_x_map_art_store.is_ui_map_phase(phase_id)
        })
        .await
        .context("Failed to load C++ terrain swap stores")?,
    );
    let mut graveyard_store = wow_data::GraveyardStore::default();
    let graveyard_report = graveyard_store
        .load_graveyard_zones_like_cpp(
            world_db.as_ref(),
            |safe_loc_id| world_safe_loc_store.contains(safe_loc_id),
            |area_id| area_table_store.get(area_id).is_some(),
        )
        .await
        .context("Failed to load C++ graveyard_zone links")?;
    info!(
        "Loaded {} graveyard-zone links ({} missing safe locs, {} missing zones, {} duplicates)",
        graveyard_report.loaded,
        graveyard_report.missing_safe_locs.len(),
        graveyard_report.missing_zones.len(),
        graveyard_report.duplicates.len()
    );
    let (mut gossip_store, gossip_load_report) =
        wow_data::GossipStore::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load C++ gossip_menu/gossip_menu_option condition keys")?;
    info!(
        "Loaded {} gossip menu rows and {} gossip menu option keys",
        gossip_load_report.menu_rows, gossip_load_report.menu_item_rows
    );
    let (spawn_group_store, spawn_group_report) =
        wow_data::SpawnGroupTemplateStore::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load C++ spawn_group_template rows")?;
    info!(
        "Loaded {} spawn group templates ({} invalid flags, {} system/manual flag fixes, {} inserted defaults)",
        spawn_group_store.len(),
        spawn_group_report.invalid_flags.len(),
        spawn_group_report.system_manual_spawn_flags.len(),
        spawn_group_report.inserted_default_groups.len()
    );
    let creature_template_store = Arc::new(
        wow_data::WorldIdStore::load_like_cpp(
            world_db.as_ref(),
            "creature_template",
            WorldStatements::SEL_CREATURE_TEMPLATE_IDS,
        )
        .await
        .context("Failed to load creature_template ids for C++ ConditionMgr validation")?,
    );
    let gameobject_template_store = Arc::new(
        wow_data::WorldIdStore::load_like_cpp(
            world_db.as_ref(),
            "gameobject_template",
            WorldStatements::SEL_GAMEOBJECT_TEMPLATE_IDS,
        )
        .await
        .context("Failed to load gameobject_template ids for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation world id stores: {} creature templates, {} gameobject templates",
        creature_template_store.len(),
        gameobject_template_store.len()
    );
    let creature_template_mount_store = Arc::new(
        wow_data::CreatureTemplateMountStoreLikeCpp::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load creature_template mount fallback rows")?,
    );
    info!(
        "Loaded {} creature template mount fallback rows",
        creature_template_mount_store.len()
    );
    let creature_display_info_store = Arc::new(
        wow_data::CreatureDisplayInfoStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load CreatureDisplayInfo.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} creature display info rows",
        creature_display_info_store.len()
    );
    let creature_model_data_store = Arc::new(
        wow_data::CreatureModelDataStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load CreatureModelData.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} creature model data rows",
        creature_model_data_store.len()
    );
    let vehicle_store = Arc::new(
        wow_data::VehicleStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load Vehicle.db2 / hotfix rows")?,
    );
    info!("Loaded {} vehicle rows", vehicle_store.len());
    let vehicle_seat_store = Arc::new(
        wow_data::VehicleSeatStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load VehicleSeat.db2 / hotfix rows")?,
    );
    info!("Loaded {} vehicle seat rows", vehicle_seat_store.len());
    let vehicle_template_store = Arc::new(
        wow_data::VehicleTemplateStoreLikeCpp::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load C++ vehicle_template rows")?,
    );
    let vehicle_accessory_store = Arc::new(
        wow_data::VehicleAccessoryStoreLikeCpp::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load C++ vehicle accessory rows")?,
    );
    let creature_spawn_store = Arc::new(
        wow_data::WorldSpawnIdStore::load_like_cpp(
            world_db.as_ref(),
            "creature",
            WorldStatements::SEL_CREATURE_SPAWN_IDS,
        )
        .await
        .context("Failed to load creature spawn ids for C++ ConditionMgr validation")?,
    );
    let gameobject_spawn_store = Arc::new(
        wow_data::WorldSpawnIdStore::load_like_cpp(
            world_db.as_ref(),
            "gameobject",
            WorldStatements::SEL_GAMEOBJECT_SPAWN_IDS,
        )
        .await
        .context("Failed to load gameobject spawn ids for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation spawn id stores: {} creature spawns, {} gameobject spawns",
        creature_spawn_store.len(),
        gameobject_spawn_store.len()
    );
    let active_event_store = Arc::new(
        wow_data::WorldIdStore::load_like_cpp(
            world_db.as_ref(),
            "game_event",
            WorldStatements::SEL_VALID_GAME_EVENT_IDS,
        )
        .await
        .context("Failed to load game_event ids for C++ ConditionMgr validation")?,
    );
    let world_state_store = Arc::new(
        wow_data::WorldIdStore::load_like_cpp(
            world_db.as_ref(),
            "world_state",
            WorldStatements::SEL_WORLD_STATE_IDS,
        )
        .await
        .context("Failed to load world_state ids for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation world id stores: {} valid game events, {} world states",
        active_event_store.len(),
        world_state_store.len()
    );
    let trainer_store = Arc::new(
        wow_data::WorldIdStore::load_like_cpp(
            world_db.as_ref(),
            "trainer",
            WorldStatements::SEL_TRAINER_IDS,
        )
        .await
        .context("Failed to load trainer ids for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation trainer id store: {} trainers",
        trainer_store.len()
    );
    let area_trigger_template_store = Arc::new(
        wow_data::AreaTriggerTemplateStore::load_like_cpp(world_db.as_ref())
            .await
            .context("Failed to load areatrigger_template keys for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation area-trigger template store: {} templates",
        area_trigger_template_store.len()
    );

    let map_difficulty_store = Arc::new(
        wow_data::MapDifficultyStore::load(&data_dir, &locale)
            .context("Failed to load MapDifficulty.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} map difficulties from MapDifficulty.db2",
        map_difficulty_store.len()
    );
    let map_difficulty_x_condition_store = Arc::new(
        wow_data::MapDifficultyXConditionStore::load(&data_dir, &locale).context(
            "Failed to load MapDifficultyXCondition.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} map difficulty conditions from MapDifficultyXCondition.db2",
        map_difficulty_x_condition_store.len()
    );
    let mount_store = Arc::new(
        wow_data::MountStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load Mount.db2 / hotfix rows")?,
    );
    info!("Loaded {} mounts from Mount.db2", mount_store.len());
    let mount_capability_store = Arc::new(
        wow_data::MountCapabilityStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load MountCapability.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} mount capabilities from MountCapability.db2",
        mount_capability_store.len()
    );
    let mount_type_x_capability_store = Arc::new(
        wow_data::MountTypeXCapabilityStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load MountTypeXCapability.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} mount type capability rows from MountTypeXCapability.db2",
        mount_type_x_capability_store.len()
    );
    let mount_x_display_store = Arc::new(
        wow_data::MountXDisplayStore::load_with_hotfixes(&data_dir, &locale, &hotfix_db)
            .await
            .context("Failed to load MountXDisplay.db2 / hotfix rows")?,
    );
    info!(
        "Loaded {} mount display rows from MountXDisplay.db2",
        mount_x_display_store.len()
    );
    let difficulty_store = Arc::new(
        wow_data::DifficultyStore::load(&data_dir, &locale)
            .context("Failed to load Difficulty.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} difficulties from Difficulty.db2",
        difficulty_store.len()
    );
    let faction_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "Faction.db2")
            .context("Failed to load Faction.db2 — check DataDir and DBC.Locale config")?,
    );
    let achievement_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "Achievement.db2")
            .context("Failed to load Achievement.db2 — check DataDir and DBC.Locale config")?,
    );
    let criteria_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "Criteria.db2")
            .context("Failed to load Criteria.db2 — check DataDir and DBC.Locale config")?,
    );
    let battlemaster_list_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "BattlemasterList.db2")
            .context("Failed to load BattlemasterList.db2 — check DataDir and DBC.Locale config")?,
    );
    let char_titles_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "CharTitles.db2")
            .context("Failed to load CharTitles.db2 — check DataDir and DBC.Locale config")?,
    );
    let battle_pet_species_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "BattlePetSpecies.db2")
            .context("Failed to load BattlePetSpecies.db2 — check DataDir and DBC.Locale config")?,
    );
    let scenario_step_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "ScenarioStep.db2")
            .context("Failed to load ScenarioStep.db2 — check DataDir and DBC.Locale config")?,
    );
    let scene_script_package_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "SceneScriptPackage.db2").context(
            "Failed to load SceneScriptPackage.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    let player_condition_store = Arc::new(
        wow_data::PlayerConditionStore::load(&data_dir, &locale)
            .context("Failed to load PlayerCondition.db2 — check DataDir and DBC.Locale config")?,
    );
    let world_state_expression_store = Arc::new(
        wow_data::WorldStateExpressionStore::load(&data_dir, &locale).context(
            "Failed to load WorldStateExpression.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    let conversation_line_store = Arc::new(
        wow_data::Db2IdStore::load(&data_dir, &locale, "ConversationLine.db2")
            .context("Failed to load ConversationLine.db2 — check DataDir and DBC.Locale config")?,
    );
    let conversation_line_template_store = Arc::new(
        wow_data::WorldIdStore::load_filtering_like_cpp(
            world_db.as_ref(),
            "conversation_line_template",
            WorldStatements::SEL_CONVERSATION_LINE_TEMPLATE_IDS,
            |line_id| conversation_line_store.contains(line_id),
        )
        .await
        .context("Failed to load conversation_line_template ids for C++ ConditionMgr validation")?,
    );
    info!(
        "Loaded condition validation DB2 id stores: {} factions, {} achievements, {} criteria, {} battlemaster lists, {} titles, {} battle pet species, {} scenario steps, {} scene script packages, {} player conditions, {} world state expressions, {} conversation lines",
        faction_store.len(),
        achievement_store.len(),
        criteria_store.len(),
        battlemaster_list_store.len(),
        char_titles_store.len(),
        battle_pet_species_store.len(),
        scenario_step_store.len(),
        scene_script_package_store.len(),
        player_condition_store.len(),
        world_state_expression_store.len(),
        conversation_line_store.len()
    );
    info!(
        "Loaded condition validation conversation line template store: {} templates",
        conversation_line_template_store.len()
    );

    // Load ItemAppearance.db2 for item display-info resolution.
    let item_appearance_store = Arc::new(
        wow_data::ItemAppearanceStore::load(&data_dir, &locale)
            .context("Failed to load ItemAppearance.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item appearances from ItemAppearance.db2",
        item_appearance_store.len()
    );

    // Load ItemModifiedAppearance.db2 for transmog/visible-item appearance resolution.
    let item_modified_appearance_store = Arc::new(
        wow_data::ItemModifiedAppearanceStore::load(&data_dir, &locale).context(
            "Failed to load ItemModifiedAppearance.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} item modified appearances from ItemModifiedAppearance.db2",
        item_modified_appearance_store.len()
    );

    // Load player level stats from world DB
    let player_stats = Arc::new(
        wow_data::PlayerStatsStore::load(&world_db)
            .await
            .context("Failed to load player_levelstats")?,
    );
    info!("Loaded {} player level stat entries", player_stats.len());

    // Load item stat modifiers from ItemSparse.db2 (gear bonuses: STR, AGI, STA, etc.)
    let item_stats_store = Arc::new(
        wow_data::ItemStatsStore::load(&data_dir, &locale)
            .context("Failed to load ItemSparse.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} items with stat modifiers from ItemSparse.db2",
        item_stats_store.len()
    );

    // Load Lock.db2 for C++ sLockStore existence checks during CMSG_OPEN_ITEM.
    let lock_store = Arc::new(
        wow_data::LockStore::load(&data_dir, &locale)
            .context("Failed to load Lock.db2 — check DataDir and DBC.Locale config")?,
    );
    info!("Loaded {} locks from Lock.db2", lock_store.len());

    // Load ItemRandomSuffix.db2 for C++ ApplyEnchantment random suffix amount resolution.
    let item_random_suffix_store = Arc::new(
        wow_data::ItemRandomSuffixStore::load(&data_dir, &locale)
            .context("Failed to load ItemRandomSuffix.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item random suffixes from ItemRandomSuffix.db2",
        item_random_suffix_store.len()
    );

    // Load ItemRandomProperties.db2 and RandPropPoints.db2 plus the world-table
    // random enchantment groups for C++ ItemEnchantmentMgr::GenerateRandomProperties.
    let item_random_properties_store = Arc::new(
        wow_data::ItemRandomPropertiesStore::load(&data_dir, &locale).context(
            "Failed to load ItemRandomProperties.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} item random properties from ItemRandomProperties.db2",
        item_random_properties_store.len()
    );

    let rand_prop_points_store = Arc::new(
        wow_data::RandPropPointsStore::load(&data_dir, &locale)
            .context("Failed to load RandPropPoints.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} random property point rows from RandPropPoints.db2",
        rand_prop_points_store.len()
    );

    // Load ItemDisenchantLoot.db2 for C++ sItemDisenchantLootStore lookup.
    let item_disenchant_loot_store = Arc::new(
        wow_data::ItemDisenchantLootStore::load(&data_dir, &locale).context(
            "Failed to load ItemDisenchantLoot.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} item disenchant loot rows from ItemDisenchantLoot.db2",
        item_disenchant_loot_store.len()
    );

    let item_random_enchantment_template_store = Arc::new(
        wow_data::ItemRandomEnchantmentTemplateStore::load_validated(
            &world_db,
            &item_random_properties_store,
            &item_random_suffix_store,
        )
        .await
        .context("Failed to load item_random_enchantment_template")?,
    );

    // Load SpellItemEnchantment.db2 for ApplyEnchantment and arena enchantment checks.
    let spell_item_enchantment_store = Arc::new(
        wow_data::SpellItemEnchantmentStore::load(&data_dir, &locale).context(
            "Failed to load SpellItemEnchantment.db2 — check DataDir and DBC.Locale config",
        )?,
    );
    info!(
        "Loaded {} spell item enchantments from SpellItemEnchantment.db2",
        spell_item_enchantment_store.len()
    );

    // Build hotfix blob cache — pre-loads raw DB2 record bytes and hotfix DB overlays for DBReply.
    let mut hotfix_blob_cache = wow_data::build_hotfix_blob_cache(&data_dir, &locale);
    match hotfix_blob_cache
        .load_hotfix_blobs_from_db(&hotfix_db, &locale)
        .await
    {
        Ok(n) => info!("HotfixBlobCache: loaded {n} hotfix_blob rows"),
        Err(e) => tracing::warn!("HotfixBlobCache: failed to load hotfix_blob rows: {e}"),
    }
    match hotfix_blob_cache
        .load_hotfix_data_from_db(&hotfix_db, &locale)
        .await
    {
        Ok(n) => info!("HotfixBlobCache: loaded {n} hotfix_data rows"),
        Err(e) => tracing::warn!("HotfixBlobCache: failed to load hotfix_data rows: {e}"),
    }
    match hotfix_blob_cache
        .load_hotfix_optional_data_from_db(&hotfix_db, &locale)
        .await
    {
        Ok(n) => info!("HotfixBlobCache: loaded {n} hotfix_optional_data rows"),
        Err(e) => tracing::warn!("HotfixBlobCache: failed to load hotfix_optional_data rows: {e}"),
    }
    let hotfix_blob_cache = Arc::new(hotfix_blob_cache);

    // Load SkillLineAbility.db2 + SkillRaceClassInfo.db2 for auto-learned spells
    let skill_store = Arc::new(
        wow_data::SkillStore::load(&data_dir, &locale)
            .context("Failed to load SkillLineAbility/SkillRaceClassInfo DB2 files")?,
    );

    // Load spell metadata (cast time, cooldown, effects, etc.) — Phase 2
    let mut spell_store = wow_data::SpellStore::load(&hotfix_db)
        .await
        .context("Failed to load SpellStore")?;
    info!("Loaded {} spells from SpellStore", spell_store.len());

    // Load area trigger store (collision detection + teleportation)
    let area_trigger_store = Arc::new(
        wow_data::load_area_triggers(&world_db)
            .await
            .context("Failed to load area triggers")?,
    );

    // Load quest store (templates + objectives + NPC relations)
    let quest_store = Arc::new(
        wow_data::quest::load_quests(&world_db)
            .await
            .context("Failed to load quest store")?,
    );
    let disable_mgr = Arc::new(
        load_disable_mgr_like_cpp(
            world_db.as_ref(),
            &map_store,
            &map_difficulty_store,
            &spell_store,
            quest_store.as_ref(),
            criteria_store.as_ref(),
            battlemaster_list_store.as_ref(),
        )
        .await?,
    );
    let mmap_disabled_map_ids = disable_mgr.disabled_mmap_map_ids_like_cpp();
    info!(
        "Loaded {} C++ mmap disable rows",
        mmap_disabled_map_ids.len()
    );

    let loaded_loot_stores = load_loot_stores_like_cpp(&world_db, &item_store)
        .await
        .context("Failed to load C++ LootTemplates_* foundation stores")?;
    let loot_reference_report = check_loot_references_like_cpp(&loaded_loot_stores);
    log_loot_reference_report_like_cpp(&loot_reference_report);
    let loot_condition_ids = load_loot_condition_ids_like_cpp(&world_db)
        .await
        .context("Failed to load C++ loot-template condition IDs")?;
    let mut loot_condition_report =
        check_loot_condition_links_like_cpp(&loaded_loot_stores, loot_condition_ids, |item_id| {
            item_store.get(item_id).is_some()
        });
    let loot_condition_reference_uses = load_loot_condition_reference_uses_like_cpp(&world_db)
        .await
        .context("Failed to load C++ loot-template condition reference uses")?;
    let condition_reference_template_ids =
        load_condition_reference_template_ids_like_cpp(&world_db)
            .await
            .context("Failed to load C++ condition reference template IDs")?;
    check_loot_condition_references_like_cpp(
        &mut loot_condition_report,
        loot_condition_reference_uses,
        condition_reference_template_ids,
    );
    log_loot_condition_link_report_like_cpp(&loot_condition_report);
    let loot_stores = Arc::new(loaded_loot_stores);
    let loaded_loot_templates: usize = loot_stores
        .values()
        .map(|store| store.templates().len())
        .sum();
    info!(
        "Loaded {} C++ loot-template stores with {} template IDs",
        loot_stores.len(),
        loaded_loot_templates
    );

    // Load player_xp_for_level table
    let player_xp_table = {
        let mut stmt = world_db.prepare(WorldStatements::SEL_PLAYER_XP_FOR_LEVEL);
        let mut table = vec![0u32; 82]; // index = level, 0=unused, 81=max
        if let Ok(result) = world_db.query(&stmt).await {
            let mut r = result;
            loop {
                let lvl: u8 = r.try_read::<u8>(0).unwrap_or(0);
                let xp: u32 = r.try_read::<u32>(1).unwrap_or(0);
                if (lvl as usize) < table.len() {
                    table[lvl as usize] = xp;
                }
                if !r.next_row() {
                    break;
                }
            }
        }
        Arc::new(table)
    };

    // Load QuestXP.db2 for accurate XP rewards
    let dbc_path = format!("{}/dbc/{}", data_dir, locale);
    let quest_xp_store = Arc::new(
        wow_data::quest_xp::QuestXpStore::load(&dbc_path).unwrap_or_else(|e| {
            tracing::warn!("QuestXP.db2 not loaded ({e}), using fallback XP table");
            wow_data::quest_xp::QuestXpStore::default()
        }),
    );

    // Get realm ID and load build-specific auth seed
    let realm_id: u16 = wow_config::get_value("RealmID").unwrap_or(1);

    let (realm_build, win64_auth_seed) = load_realm_auth_seed(&login_db, realm_id).await?;
    info!("Realm {realm_id} build {realm_build}, Win64AuthSeed loaded");

    // Load realm addresses from realmlist table (for ConnectTo)
    let (realm_external_address, realm_local_address) =
        load_realm_addresses(&login_db, realm_id).await?;
    info!(
        "Realm addresses: external={}, local={}",
        format_ipv4(realm_external_address),
        format_ipv4(realm_local_address),
    );

    // Wrap login_db in Arc for sharing between account lookup and sessions
    let login_db = Arc::new(login_db);

    // Build handler dispatch table
    let table = wow_handler::build_dispatch_table();
    info!("Loaded {} packet handlers", table.len());

    // Build account lookup
    let account_lookup: Arc<dyn AccountLookup> = Arc::new(DbAccountLookup {
        login_db: Arc::clone(&login_db),
        realm_id,
        win64_auth_seed,
    });

    // Shared player registry for broadcast (chat, emotes, movement)
    let player_registry = Arc::new(PlayerRegistry::new());
    let object_accessor = wow_world::new_shared_object_accessor();

    let mut condition_load_report =
        wow_data::conditions::load_condition_rows_like_cpp(world_db.as_ref(), |_| 0)
            .await
            .context("Failed to load C++ conditions table")?;
    let loot_template_exists = |source_type: wow_constants::ConditionSourceType,
                                source_group: u32| {
        loot_store_kind_for_condition_source_type_like_cpp(source_type as i32)
            .and_then(|kind| loot_stores.get(&kind))
            .is_some_and(|store| store.have_loot_for(source_group))
    };
    let loot_source_entry_exists = |source_type: wow_constants::ConditionSourceType,
                                    source_group: u32,
                                    source_entry: i32| {
        let Some(source_entry) = u32::try_from(source_entry).ok() else {
            return false;
        };
        let Some(store) = loot_store_kind_for_condition_source_type_like_cpp(source_type as i32)
            .and_then(|kind| loot_stores.get(&kind))
        else {
            return false;
        };
        let Some(template) = store.get_loot_for(source_group) else {
            return false;
        };

        item_store.get(source_entry).is_some() || template.is_reference_like_cpp(source_entry)
    };
    let externally_skipped_conditions =
        wow_data::conditions::apply_external_condition_validation_like_cpp(
            &mut condition_load_report,
            wow_data::conditions::ConditionExternalValidationStoresLikeCpp {
                item_store: Some(item_store.as_ref()),
                spell_store: Some(&spell_store),
                area_table_store: Some(area_table_store.as_ref()),
                skill_store: Some(skill_store.as_ref()),
                map_store: Some(map_store.as_ref()),
                phase_store: Some(phase_store.as_ref()),
                quest_store: Some(quest_store.as_ref()),
                area_trigger_store: Some(area_trigger_store.as_ref()),
                graveyard_store: Some(&graveyard_store),
                spawn_group_store: Some(&spawn_group_store),
                creature_template_store: Some(creature_template_store.as_ref()),
                gameobject_template_store: Some(gameobject_template_store.as_ref()),
                trainer_store: Some(trainer_store.as_ref()),
                conversation_line_template_store: Some(conversation_line_template_store.as_ref()),
                area_trigger_template_store: Some(area_trigger_template_store.as_ref()),
                creature_spawn_store: Some(creature_spawn_store.as_ref()),
                gameobject_spawn_store: Some(gameobject_spawn_store.as_ref()),
                active_event_store: Some(active_event_store.as_ref()),
                world_state_store: Some(world_state_store.as_ref()),
                difficulty_store: Some(difficulty_store.as_ref()),
                faction_store: Some(faction_store.as_ref()),
                achievement_store: Some(achievement_store.as_ref()),
                char_titles_store: Some(char_titles_store.as_ref()),
                battle_pet_species_store: Some(battle_pet_species_store.as_ref()),
                scenario_step_store: Some(scenario_step_store.as_ref()),
                scene_script_package_store: Some(scene_script_package_store.as_ref()),
                player_condition_store: Some(player_condition_store.as_ref()),
                max_skill_value: Some(max_skill_value_like_cpp(&world_configs)),
                loot_template_exists: Some(&loot_template_exists),
                loot_source_entry_exists: Some(&loot_source_entry_exists),
            },
        );
    for skipped in &condition_load_report.skipped {
        warn!(
            "Condition row skipped during C++ load-shape parsing: {:?}: {:?}",
            skipped.row, skipped.reason
        );
    }
    for skipped in &externally_skipped_conditions {
        warn!(
            "Condition row skipped during C++ external validation: {:?}: {:?}",
            skipped.condition, skipped.reason
        );
    }
    for warning in &condition_load_report.warnings {
        warn!("Condition load warning: {warning:?}");
    }
    let condition_store = Arc::new(condition_load_report.into_store_like_cpp());
    let condition_attachment_report = wow_data::attach_loaded_conditions_like_cpp(
        condition_store.as_ref(),
        Some(&mut gossip_store),
        Some(&mut spell_store),
        Some(&mut phase_info_store),
        Some(&mut graveyard_store),
    );
    for missing in &condition_attachment_report.gossip_menus.missing_menus {
        warn!(
            "ConditionMgr gossip attachment warning: GossipMenu {} not found for condition id {:?}",
            missing.source_group, missing
        );
    }
    for missing in &condition_attachment_report
        .gossip_menu_items
        .missing_menu_items
    {
        warn!(
            "ConditionMgr gossip attachment warning: GossipMenuId {} Item {} not found for condition id {:?}",
            missing.source_group, missing.source_entry, missing
        );
    }
    info!(
        "Loaded C++ ConditionMgr store: {} buckets, {} externally skipped conditions, {} spell-click aura spell ids, {} spell implicit target condition rows attached ({} deferred), {} gossip menu condition rows attached ({} missing menus), {} gossip menu option condition rows attached ({} missing items), {} phase condition rows attached, {} graveyard condition rows attached",
        condition_store.bucket_count(),
        externally_skipped_conditions.len(),
        condition_attachment_report.spell_click_aura_spell_ids.len(),
        condition_attachment_report.spell_implicit_target_condition_count,
        condition_attachment_report.deferred_spell_implicit_target_condition_count,
        condition_attachment_report
            .gossip_menus
            .attached_condition_count,
        condition_attachment_report.gossip_menus.missing_menus.len(),
        condition_attachment_report
            .gossip_menu_items
            .attached_condition_count,
        condition_attachment_report
            .gossip_menu_items
            .missing_menu_items
            .len(),
        condition_attachment_report.phases.attached_condition_count,
        condition_attachment_report
            .graveyards
            .attached_condition_count
    );
    wow_world::conditions::set_condition_mgr_store_like_cpp(Arc::clone(&condition_store));
    let spell_store = Arc::new(spell_store);

    // Shared group registry and pending invites
    let group_registry = Arc::new(GroupRegistry::new());
    let pending_invites = Arc::new(PendingInvites::new());

    // Shared world state (creatures/grids visible to every session on the same map).
    // Each session gets a clone of this Arc on creation.
    let shared_map: SharedMapManager = Arc::new(std::sync::RwLock::new(LegacyMapManager::new()));
    let mut loaded_instance_lock_mgr = InstanceLockMgr::default();
    let instance_lock_load_issues = loaded_instance_lock_mgr
        .load_from_database_like_cpp(&char_db, |map_id, difficulty_id| {
            map_db2_entries_from_stores(&map_store, &map_difficulty_store, map_id, difficulty_id)
        })
        .await
        .context("Failed to load instance locks from character database")?;
    for issue in &instance_lock_load_issues {
        warn!("Instance lock load issue: {issue:?}");
    }
    let instance_lock_stats = loaded_instance_lock_mgr.statistics();
    info!(
        "Loaded instance locks: {} shared instances, {} players, {} issues",
        instance_lock_stats.instance_count,
        instance_lock_stats.player_count,
        instance_lock_load_issues.len()
    );
    let registered_instance_ids = loaded_instance_lock_mgr.registered_instance_ids_like_cpp_order();
    let instance_lock_mgr = Arc::new(std::sync::RwLock::new(loaded_instance_lock_mgr));

    let canonical_map_manager = Arc::new(Mutex::new(create_canonical_map_manager(&world_configs)));
    register_loaded_instance_ids(
        &shared_map,
        canonical_map_manager.as_ref(),
        &registered_instance_ids,
    );

    // Build session resources
    let session_resources = Arc::new(SessionResources {
        char_db: Some(Arc::clone(&char_db)),
        login_db: Some(Arc::clone(&login_db)),
        world_db: Some(Arc::clone(&world_db)),
        guid_generator: Some(Arc::clone(&guid_generator)),
        instance_lock_mgr: Some(Arc::clone(&instance_lock_mgr)),
        currency_types_store: Some(Arc::clone(&currency_types_store)),
        import_price_stores: Some(Arc::clone(&import_price_stores)),
        item_class_store: Some(Arc::clone(&item_class_store)),
        item_currency_cost_store: Some(Arc::clone(&item_currency_cost_store)),
        item_extended_cost_store: Some(Arc::clone(&item_extended_cost_store)),
        item_store: Some(Arc::clone(&item_store)),
        item_appearance_store: Some(Arc::clone(&item_appearance_store)),
        item_modified_appearance_store: Some(Arc::clone(&item_modified_appearance_store)),
        item_price_base_store: Some(Arc::clone(&item_price_base_store)),
        player_stats: Some(Arc::clone(&player_stats)),
        item_stats_store: Some(Arc::clone(&item_stats_store)),
        item_random_suffix_store: Some(Arc::clone(&item_random_suffix_store)),
        item_random_properties_store: Some(Arc::clone(&item_random_properties_store)),
        rand_prop_points_store: Some(Arc::clone(&rand_prop_points_store)),
        item_random_enchantment_template_store: Some(Arc::clone(
            &item_random_enchantment_template_store,
        )),
        item_disenchant_loot_store: Some(Arc::clone(&item_disenchant_loot_store)),
        loot_stores: Some(Arc::clone(&loot_stores)),
        condition_store: Some(Arc::clone(&condition_store)),
        player_condition_store: Some(Arc::clone(&player_condition_store)),
        disable_mgr: Some(Arc::clone(&disable_mgr)),
        lock_store: Some(Arc::clone(&lock_store)),
        spell_item_enchantment_store: Some(Arc::clone(&spell_item_enchantment_store)),
        hotfix_blob_cache: Some(Arc::clone(&hotfix_blob_cache)),
        skill_store: Some(Arc::clone(&skill_store)),
        spell_store: Some(Arc::clone(&spell_store)),
        area_table_store: Some(Arc::clone(&area_table_store)),
        area_trigger_store: Some(Arc::clone(&area_trigger_store)),
        chr_specialization_store: Some(Arc::clone(&chr_specialization_store)),
        dungeon_encounter_store: Some(Arc::clone(&dungeon_encounter_store)),
        map_store: Some(Arc::clone(&map_store)),
        map_difficulty_store: Some(Arc::clone(&map_difficulty_store)),
        map_difficulty_x_condition_store: Some(Arc::clone(&map_difficulty_x_condition_store)),
        creature_template_mount_store: Some(Arc::clone(&creature_template_mount_store)),
        creature_display_info_store: Some(Arc::clone(&creature_display_info_store)),
        creature_model_data_store: Some(Arc::clone(&creature_model_data_store)),
        mount_store: Some(Arc::clone(&mount_store)),
        mount_capability_store: Some(Arc::clone(&mount_capability_store)),
        mount_type_x_capability_store: Some(Arc::clone(&mount_type_x_capability_store)),
        mount_x_display_store: Some(Arc::clone(&mount_x_display_store)),
        vehicle_store: Some(Arc::clone(&vehicle_store)),
        vehicle_seat_store: Some(Arc::clone(&vehicle_seat_store)),
        vehicle_template_store: Some(Arc::clone(&vehicle_template_store)),
        vehicle_accessory_store: Some(Arc::clone(&vehicle_accessory_store)),
        terrain_swap_store: Some(Arc::clone(&terrain_swap_store)),
        phase_store: Some(Arc::clone(&phase_store)),
        phase_group_store: Some(Arc::clone(&phase_group_store)),
        quest_store: Some(Arc::clone(&quest_store)),
        quest_xp_store: Some(Arc::clone(&quest_xp_store)),
        player_xp_table: Some(Arc::clone(&player_xp_table)),
        player_registry: Some(Arc::clone(&player_registry)),
        group_registry: Some(Arc::clone(&group_registry)),
        pending_invites: Some(Arc::clone(&pending_invites)),
        loot_drop_rates: loot_drop_rates_like_cpp(&world_configs),
        enable_ae_loot: world_config_bool(&world_configs, "CONFIG_ENABLE_AE_LOOT", false),
        realm_id,
        realm_external_address,
        realm_local_address,
    });

    // Create SessionManager for ConnectTo flow
    let session_mgr = Arc::new(SessionManager::new());

    // Network configuration
    let bind_ip = wow_config::get_string_default("BindIP", "0.0.0.0");
    let world_port = world_config_u16(&world_configs, "CONFIG_PORT_WORLD", 8085);
    let instance_port = world_config_u16(&world_configs, "CONFIG_PORT_INSTANCE", 8086);
    let max_expansion = world_config_u8(&world_configs, "CONFIG_EXPANSION", 2);
    let mmap_runtime_config = mmap_runtime_config_like_cpp(&world_configs, mmap_disabled_map_ids);
    info!(
        "WORLD: MMap pathfinding: {}, data directory: {}/mmaps",
        if mmap_runtime_config.enabled {
            "enabled"
        } else {
            "disabled"
        },
        mmap_runtime_config.data_dir
    );
    let mmap_pathfinder = mmap_runtime_config.enabled.then(|| {
        Arc::new(
            WorldMMapPathfinderWorkerLikeCpp::spawn_with_parent_map_data_like_cpp(
                &mmap_runtime_config.data_dir,
                map_store.parent_child_map_data_like_cpp(),
            ),
        )
    });

    let realm_addr: SocketAddr = format!("{bind_ip}:{world_port}")
        .parse()
        .context("Invalid bind address")?;
    let instance_addr: SocketAddr = format!("{bind_ip}:{instance_port}")
        .parse()
        .context("Invalid instance bind address")?;

    info!("Starting realm listener on {realm_addr}");
    info!("Starting instance listener on {instance_addr}");

    // Spawn realm listener (existing world listener)
    let realm_handle = tokio::spawn({
        let lookup = Arc::clone(&account_lookup);
        let resources = Arc::clone(&session_resources);
        let mgr = Arc::clone(&session_mgr);
        let smap = Arc::clone(&shared_map);
        let accessor = Arc::clone(&object_accessor);
        let port = instance_port;
        let mmap_config = mmap_runtime_config.clone();
        let mmap_pathfinder = mmap_pathfinder.clone();
        async move {
            if let Err(e) = wow_network::start_world_listener(
                realm_addr,
                lookup,
                resources,
                move |account, pkt_rx, send_tx, res| {
                    let mgr = Arc::clone(&mgr);
                    let smap = Arc::clone(&smap);
                    let accessor = Arc::clone(&accessor);
                    let mmap_pathfinder = mmap_pathfinder.clone();
                    create_session(
                        account,
                        pkt_rx,
                        send_tx,
                        res,
                        mgr,
                        smap,
                        accessor,
                        port,
                        max_expansion,
                        mmap_config.clone(),
                        mmap_pathfinder,
                    )
                },
            )
            .await
            {
                tracing::error!("Realm listener error: {e}");
            }
        }
    });

    // Spawn instance listener
    let instance_handle = tokio::spawn({
        let mgr = Arc::clone(&session_mgr);
        async move {
            if let Err(e) = wow_network::start_instance_listener(instance_addr, mgr).await {
                tracing::error!("Instance listener error: {e}");
            }
        }
    });

    let map_update_interval_ms = world_config_u32(&world_configs, "CONFIG_INTERVAL_MAPUPDATE", 10)
        .max(wow_map::MIN_MAP_UPDATE_DELAY_MS);
    let map_update_handle =
        spawn_canonical_map_update_loop(Arc::clone(&canonical_map_manager), map_update_interval_ms);

    set_realm_online(&login_db, realm_id).await?;

    // Wait for shutdown signal
    tokio::select! {
        _ = shutdown_signal() => {
            info!("Shutdown signal received, stopping...");
        }
        result = realm_handle => {
            if let Err(e) = result {
                tracing::error!("Realm listener task failed: {e}");
            }
        }
        result = instance_handle => {
            if let Err(e) = result {
                tracing::error!("Instance listener task failed: {e}");
            }
        }
        result = map_update_handle => {
            if let Err(e) = result {
                tracing::error!("Map update task failed: {e}");
            }
        }
    }

    if let Err(e) = set_realm_offline(&login_db, realm_id).await {
        tracing::error!("Failed to mark realm {realm_id} offline: {e}");
    }

    info!("World server stopped.");
    Ok(())
}

async fn set_realm_online(login_db: &LoginDatabase, realm_id: u16) -> Result<()> {
    const REALM_FLAG_OFFLINE: u8 = 0x02;

    login_db
        .direct_execute(&format!(
            "UPDATE realmlist SET flag = flag & ~{REALM_FLAG_OFFLINE}, population = 0 WHERE id = {realm_id}"
        ))
        .await
        .context("Failed to mark realm online")?;

    info!("Realm {realm_id} marked online");
    Ok(())
}

async fn set_realm_offline(login_db: &LoginDatabase, realm_id: u16) -> Result<()> {
    const REALM_FLAG_OFFLINE: u8 = 0x02;

    login_db
        .direct_execute(&format!(
            "UPDATE realmlist SET flag = flag | {REALM_FLAG_OFFLINE} WHERE id = {realm_id}"
        ))
        .await
        .context("Failed to mark realm offline")?;

    info!("Realm {realm_id} marked offline");
    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn load_world_config() -> Result<LoadReport> {
    load_world_config_from(WORLD_CONFIG_CANDIDATES, WORLD_CONFIG_DIR)
}

fn load_world_config_from(config_candidates: &[&str], config_dir: &str) -> Result<LoadReport> {
    let loaded_config = wow_config::load_config_with_fallbacks(config_candidates, config_dir)
        .context("Failed to load worldserver.conf")?;

    if loaded_config.candidate_index > 1 {
        tracing::warn!(
            config = %loaded_config.initial_file,
            "Using legacy Rust config filename; prefer worldserver.conf"
        );
    }

    Ok(loaded_config)
}

fn world_config_u16(configs: &WorldConfigSet, enum_name: &str, default: u16) -> u16 {
    configs
        .get_int(enum_name)
        .map(|value| value as u16)
        .unwrap_or(default)
}

fn world_config_u8(configs: &WorldConfigSet, enum_name: &str, default: u8) -> u8 {
    configs
        .get_int(enum_name)
        .map(|value| value as u8)
        .unwrap_or(default)
}

fn world_config_u32(configs: &WorldConfigSet, enum_name: &str, default: u32) -> u32 {
    configs
        .get_int(enum_name)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(default)
}

fn world_config_f32(configs: &WorldConfigSet, enum_name: &str, default: f32) -> f32 {
    configs.get_float(enum_name).unwrap_or(default)
}

fn world_config_bool(configs: &WorldConfigSet, enum_name: &str, default: bool) -> bool {
    configs.get_bool(enum_name).unwrap_or(default)
}

fn max_skill_value_like_cpp(configs: &WorldConfigSet) -> u32 {
    let max_player_level = u32::from(world_config_u8(configs, "CONFIG_MAX_PLAYER_LEVEL", 80));
    if max_player_level > 60 {
        300 + ((max_player_level - 60) * 75) / 10
    } else {
        max_player_level * 5
    }
}

fn mmap_runtime_config_like_cpp(
    configs: &WorldConfigSet,
    disabled_map_ids: HashSet<u32>,
) -> MMapRuntimeConfigLikeCpp {
    MMapRuntimeConfigLikeCpp {
        data_dir: wow_config::get_string_default("DataDir", "./Data"),
        enabled: world_config_bool(configs, "CONFIG_ENABLE_MMAPS", true),
        disabled_map_ids,
    }
}

async fn load_disable_mgr_like_cpp(
    world_db: &WorldDatabase,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    spell_store: &wow_data::SpellStore,
    quest_store: &wow_data::quest::QuestStore,
    criteria_store: &wow_data::Db2IdStore,
    battlemaster_list_store: &wow_data::Db2IdStore,
) -> Result<wow_data::DisableMgrLikeCpp> {
    let (disable_mgr, _) = wow_data::DisableMgrLikeCpp::load_like_cpp(
        world_db,
        wow_data::DisableMgrRefsLikeCpp {
            map_store: Some(map_store),
            map_difficulty_store: Some(map_difficulty_store),
            spell_store: Some(spell_store),
            quest_store: Some(quest_store),
            criteria_store: Some(criteria_store),
            battlemaster_list_store: Some(battlemaster_list_store),
            ..Default::default()
        },
    )
    .await
    .context("Failed to query C++ disables")?;

    Ok(disable_mgr)
}

fn loot_drop_rates_like_cpp(configs: &WorldConfigSet) -> LootDropRatesLikeCpp {
    LootDropRatesLikeCpp {
        item_poor: world_config_f32(configs, "RATE_DROP_ITEM_POOR", 1.0),
        item_normal: world_config_f32(configs, "RATE_DROP_ITEM_NORMAL", 1.0),
        item_uncommon: world_config_f32(configs, "RATE_DROP_ITEM_UNCOMMON", 1.0),
        item_rare: world_config_f32(configs, "RATE_DROP_ITEM_RARE", 1.0),
        item_epic: world_config_f32(configs, "RATE_DROP_ITEM_EPIC", 1.0),
        item_legendary: world_config_f32(configs, "RATE_DROP_ITEM_LEGENDARY", 1.0),
        item_artifact: world_config_f32(configs, "RATE_DROP_ITEM_ARTIFACT", 1.0),
        item_referenced: world_config_f32(configs, "RATE_DROP_ITEM_REFERENCED", 1.0),
        item_referenced_amount: world_config_f32(configs, "RATE_DROP_ITEM_REFERENCED_AMOUNT", 1.0),
        money: world_config_f32(configs, "RATE_DROP_MONEY", 1.0),
    }
}

async fn load_loot_stores_like_cpp(
    world_db: &WorldDatabase,
    item_store: &wow_data::ItemStore,
) -> Result<LootStores> {
    let mut stores = LootStores::new();

    for kind in LootStoreKind::ALL_LIKE_CPP {
        let rows = load_loot_template_rows_like_cpp(world_db, kind).await?;
        let mut store = LootStore::for_kind_like_cpp(kind);
        let accepted = store
            .load_rows_like_cpp(rows, |item_id| item_store.get(item_id).is_some())
            .map_err(|err| anyhow::anyhow!("invalid loot row in {:?}: {:?}", kind, err))?;
        info!(
            table = store.definition().table_name,
            entry_name = store.definition().entry_name,
            rates_allowed = store.definition().rates_allowed,
            accepted_rows = accepted,
            template_ids = store.templates().len(),
            "Loaded C++ loot template store foundation"
        );
        stores.insert(kind, store);
    }

    Ok(stores)
}

fn log_loot_reference_report_like_cpp(report: &LootReferenceCheckReport) {
    if report.is_clean() {
        info!("C++ loot reference verification completed with no gaps");
        return;
    }

    for reference_use in &report.missing_references {
        let store_definition = reference_use.store_kind.definition_like_cpp();
        tracing::warn!(
            table = store_definition.table_name,
            entry = reference_use.entry,
            item_id = reference_use.item_id,
            reference = reference_use.reference,
            "C++ loot reference verification found missing reference_loot_template entry"
        );
    }

    for reference_id in &report.unused_reference_ids {
        tracing::warn!(
            table = LootStoreKind::Reference.definition_like_cpp().table_name,
            entry = *reference_id,
            "C++ loot reference verification found unused reference_loot_template entry"
        );
    }
}

fn log_loot_condition_link_report_like_cpp(report: &LootConditionLinkReport) {
    if report.is_clean() {
        info!(
            linked_conditions = report.linked,
            "C++ loot condition structural linking completed with no gaps"
        );
        return;
    }

    for condition_id in &report.unsupported_source_types {
        tracing::warn!(
            source_type = condition_id.source_type,
            source_group = condition_id.source_group,
            source_entry = condition_id.source_entry,
            "C++ loot condition structural linking found unsupported loot condition source type"
        );
    }

    for missing in &report.missing_templates {
        let store_definition = missing.store_kind.definition_like_cpp();
        tracing::warn!(
            table = store_definition.table_name,
            source_type = missing.condition_id.source_type,
            source_group = missing.condition_id.source_group,
            source_entry = missing.condition_id.source_entry,
            "C++ loot condition structural linking found missing loot template"
        );
    }

    for missing in &report.missing_item_templates {
        let store_definition = missing.store_kind.definition_like_cpp();
        tracing::warn!(
            table = store_definition.table_name,
            source_type = missing.condition_id.source_type,
            source_group = missing.condition_id.source_group,
            source_entry = missing.condition_id.source_entry,
            "C++ loot condition structural linking found missing item template for SourceEntry"
        );
    }

    for missing in &report.missing_template_items {
        let store_definition = missing.store_kind.definition_like_cpp();
        tracing::warn!(
            table = store_definition.table_name,
            source_type = missing.condition_id.source_type,
            source_group = missing.condition_id.source_group,
            source_entry = missing.condition_id.source_entry,
            "C++ loot condition structural linking found SourceEntry absent from loot template"
        );
    }

    for missing in &report.missing_reference_templates {
        tracing::warn!(
            source_type = missing.condition_id.source_type,
            source_group = missing.condition_id.source_group,
            source_entry = missing.condition_id.source_entry,
            reference_id = missing.reference_id,
            "C++ loot condition structural linking found missing condition reference template"
        );
    }
}

async fn load_loot_condition_ids_like_cpp(
    world_db: &WorldDatabase,
) -> Result<Vec<LootConditionId>> {
    let stmt = world_db.prepare(WorldStatements::SEL_LOOT_TEMPLATE_CONDITION_IDS);
    let mut result = world_db.query(&stmt).await?;
    let mut condition_ids = Vec::new();

    if result.is_empty() {
        return Ok(condition_ids);
    }

    loop {
        condition_ids.push(LootConditionId {
            source_type: result.try_read::<i32>(0).unwrap_or(0),
            source_group: result.try_read::<u32>(1).unwrap_or(0),
            source_entry: result.try_read::<u32>(2).unwrap_or(0),
        });

        if !result.next_row() {
            break;
        }
    }

    Ok(condition_ids)
}

async fn load_loot_condition_reference_uses_like_cpp(
    world_db: &WorldDatabase,
) -> Result<Vec<LootConditionReferenceUseLikeCpp>> {
    let stmt = world_db.prepare(WorldStatements::SEL_LOOT_TEMPLATE_CONDITION_REFERENCE_USES);
    let mut result = world_db.query(&stmt).await?;
    let mut reference_uses = Vec::new();

    if result.is_empty() {
        return Ok(reference_uses);
    }

    loop {
        reference_uses.push(LootConditionReferenceUseLikeCpp {
            condition_id: LootConditionId {
                source_type: result.try_read::<i32>(0).unwrap_or(0),
                source_group: result.try_read::<u32>(1).unwrap_or(0),
                source_entry: result.try_read::<u32>(2).unwrap_or(0),
            },
            reference_id: result.try_read::<u32>(3).unwrap_or(0),
        });

        if !result.next_row() {
            break;
        }
    }

    Ok(reference_uses)
}

async fn load_condition_reference_template_ids_like_cpp(
    world_db: &WorldDatabase,
) -> Result<Vec<u32>> {
    let stmt = world_db.prepare(WorldStatements::SEL_CONDITION_REFERENCE_TEMPLATE_IDS);
    let mut result = world_db.query(&stmt).await?;
    let mut template_ids = Vec::new();

    if result.is_empty() {
        return Ok(template_ids);
    }

    loop {
        template_ids.push(result.try_read::<u32>(0).unwrap_or(0));

        if !result.next_row() {
            break;
        }
    }

    Ok(template_ids)
}

async fn load_loot_template_rows_like_cpp(
    world_db: &WorldDatabase,
    kind: LootStoreKind,
) -> Result<Vec<LootTemplateRow>> {
    let statement = loot_store_all_rows_statement_like_cpp(kind);
    let stmt = world_db.prepare(statement);
    let mut result = world_db.query(&stmt).await?;
    let mut rows = Vec::new();

    if result.is_empty() {
        return Ok(rows);
    }

    loop {
        rows.push(LootTemplateRow {
            entry: result.try_read::<u32>(0).unwrap_or(0),
            item: wow_loot::LootStoreItem {
                item_id: result.try_read::<u32>(1).unwrap_or(0),
                reference: result.try_read::<u32>(2).unwrap_or(0),
                chance: result.try_read::<f32>(3).unwrap_or(0.0),
                needs_quest: result.try_read::<u8>(4).unwrap_or(0) != 0,
                loot_mode: result.try_read::<u16>(5).unwrap_or(0),
                group_id: result.try_read::<u8>(6).unwrap_or(0),
                min_count: result.try_read::<u8>(7).unwrap_or(0),
                max_count: result.try_read::<u8>(8).unwrap_or(0),
            },
        });

        if !result.next_row() {
            break;
        }
    }

    Ok(rows)
}

fn loot_store_all_rows_statement_like_cpp(kind: LootStoreKind) -> WorldStatements {
    match kind {
        LootStoreKind::Creature => WorldStatements::SEL_CREATURE_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Disenchant => WorldStatements::SEL_DISENCHANT_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Fishing => WorldStatements::SEL_FISHING_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Gameobject => WorldStatements::SEL_GAMEOBJECT_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Item => WorldStatements::SEL_ITEM_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Mail => WorldStatements::SEL_MAIL_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Milling => WorldStatements::SEL_MILLING_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Pickpocketing => WorldStatements::SEL_PICKPOCKETING_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Prospecting => WorldStatements::SEL_PROSPECTING_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Reference => WorldStatements::SEL_REFERENCE_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Skinning => WorldStatements::SEL_SKINNING_LOOT_TEMPLATE_ALL_ROWS,
        LootStoreKind::Spell => WorldStatements::SEL_SPELL_LOOT_TEMPLATE_ALL_ROWS,
    }
}

fn create_canonical_map_manager(configs: &WorldConfigSet) -> wow_map::MapManager {
    let grid_cleanup_delay_ms =
        world_config_u32(configs, "CONFIG_INTERVAL_GRIDCLEAN", 5 * 60 * 1000)
            .max(wow_map::MIN_GRID_DELAY_MS);
    let map_update_interval_ms = world_config_u32(configs, "CONFIG_INTERVAL_MAPUPDATE", 10)
        .max(wow_map::MIN_MAP_UPDATE_DELAY_MS);
    let map_update_threads = world_config_u32(configs, "CONFIG_NUMTHREADS", 1);

    let mut manager = wow_map::MapManager::new(grid_cleanup_delay_ms, map_update_interval_ms);
    if map_update_threads > 0 {
        manager
            .map_updater_mut()
            .activate(map_update_threads as usize);
    }

    info!(
        "Canonical MapManager initialized: grid_cleanup_delay_ms={}, map_update_interval_ms={}, map_update_threads={}",
        grid_cleanup_delay_ms, map_update_interval_ms, map_update_threads,
    );

    manager
}

fn map_db2_entries_from_stores(
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    map_id: u32,
    difficulty_id: u8,
) -> Option<MapDb2Entries> {
    let map = map_store.get(map_id)?;
    let map_difficulty = map_difficulty_store.get(map_id, difficulty_id)?;

    Some(MapDb2Entries {
        map_id,
        difficulty_id,
        lock_id: u32::from(map_difficulty.lock_id),
        reset_interval: match map_difficulty.reset_interval {
            1 => MapDifficultyResetInterval::Daily,
            2 => MapDifficultyResetInterval::Weekly,
            _ => MapDifficultyResetInterval::Anytime,
        },
        is_flex_locking: map.is_flex_locking(),
        is_using_encounter_locks: map_difficulty.is_using_encounter_locks(),
    })
}

fn register_loaded_instance_ids(
    legacy_map_manager: &SharedMapManager,
    canonical_map_manager: &Mutex<wow_map::MapManager>,
    instance_ids: &[u32],
) {
    let Some(max_instance_id) = instance_ids.iter().copied().max() else {
        return;
    };

    match legacy_map_manager.write() {
        Ok(mut manager) => {
            manager.init_instance_ids_from_max(max_instance_id);
            for &instance_id in instance_ids {
                manager.register_instance_id(instance_id);
            }
        }
        Err(_) => warn!("Legacy MapManager lock poisoned; persisted instance ids not registered"),
    }

    match canonical_map_manager.lock() {
        Ok(mut manager) => {
            manager.init_instance_ids(u64::from(max_instance_id));
            for &instance_id in instance_ids {
                manager.register_instance_id(instance_id);
            }
        }
        Err(_) => {
            warn!("Canonical MapManager lock poisoned; persisted instance ids not registered")
        }
    }

    info!(
        "Registered {} persisted instance ids with MapManager, max_instance_id={}",
        instance_ids.len(),
        max_instance_id
    );
}

fn spawn_canonical_map_update_loop(
    map_manager: SharedCanonicalMapManager,
    tick_interval_ms: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_millis(u64::from(tick_interval_ms)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut last_tick = Instant::now();
        loop {
            interval.tick().await;

            let now = Instant::now();
            let diff_ms = now
                .duration_since(last_tick)
                .as_millis()
                .min(u128::from(u32::MAX)) as u32;
            last_tick = now;

            if diff_ms == 0 {
                continue;
            }

            let Ok(mut manager) = map_manager.lock() else {
                tracing::error!("Canonical MapManager mutex poisoned; stopping map update loop");
                break;
            };
            manager.update(diff_ms);
        }
    })
}

/// Load the realm's gamebuild from `realmlist` and the corresponding
/// Win64AuthSeed from `build_info`. Both are in the login database.
async fn load_realm_auth_seed(login_db: &LoginDatabase, realm_id: u16) -> Result<(u32, [u8; 16])> {
    // Query realmlist for the gamebuild
    let result = login_db
        .direct_query(&format!(
            "SELECT gamebuild FROM realmlist WHERE id = {realm_id}"
        ))
        .await
        .context("Failed to query realmlist")?;

    let build: u32 = if result.is_empty() {
        tracing::warn!("Realm {realm_id} not found in realmlist, using default build");
        51943
    } else {
        result.read(0)
    };

    // Query build_info for the Win64AuthSeed
    let seed_result = login_db
        .direct_query(&format!(
            "SELECT win64AuthSeed FROM build_info WHERE build = {build}"
        ))
        .await
        .context("Failed to query build_info")?;

    let seed_hex: String = if seed_result.is_empty() {
        anyhow::bail!("No build_info entry for build {build}");
    } else {
        seed_result.try_read(0).unwrap_or_default()
    };

    if seed_hex.len() != 32 {
        anyhow::bail!(
            "Invalid Win64AuthSeed for build {build}: expected 32 hex chars, got {}",
            seed_hex.len()
        );
    }

    // Parse hex string into 16 bytes
    let mut seed = [0u8; 16];
    for (i, byte) in seed.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&seed_hex[i * 2..i * 2 + 2], 16)
            .with_context(|| format!("Invalid hex in auth seed at position {i}"))?;
    }

    Ok((build, seed))
}

/// Parse an IPv4 address string into 4 bytes.
fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let addr: std::net::Ipv4Addr = s.parse().ok()?;
    Some(addr.octets())
}

/// Create and run a WorldSession for an authenticated connection.
///
/// This is called by the accept loop after auth completes.
/// Runs the session update loop until the packet channel is closed.
async fn create_session(
    account: AccountInfo,
    pkt_rx: flume::Receiver<wow_packet::WorldPacket>,
    send_tx: flume::Sender<Vec<u8>>,
    resources: Arc<SessionResources>,
    session_mgr: Arc<SessionManager>,
    shared_map: SharedMapManager,
    object_accessor: wow_world::SharedObjectAccessor,
    instance_port: u16,
    max_expansion: u8,
    mmap_runtime_config: MMapRuntimeConfigLikeCpp,
    mmap_pathfinder: Option<Arc<WorldMMapPathfinderWorkerLikeCpp>>,
) {
    info!(
        "Creating session for account {} (bnet_id={})",
        account.id, account.battlenet_account_id
    );

    // Use the DERIVED 40-byte session key from realm auth handshake.
    // C# writes this to the DB (UPD_ACCOUNT_INFO_CONTINUED_SESSION) and the
    // instance socket reads it back. We skip the DB roundtrip by passing it directly.
    // NOTE: This is NOT the raw BNet key (64 bytes) from the DB. It's the
    // HMAC-SHA256 derived key used for AuthContinuedSession validation.
    let session_key_raw = account.derived_session_key.clone();

    // C# caps only ActiveExpansionLevel to the server's max expansion,
    // but sends AccountExpansionLevel as the raw DB value (e.g. 9=Dragonflight).
    // The client uses AccountExpansionLevel to unlock classes in the char list.
    let active_expansion = account.expansion.min(max_expansion);
    let account_expansion = account.expansion; // raw from DB, NOT capped

    let mut session = WorldSession::new(
        account.id,
        String::new(), // account_name
        account.security,
        active_expansion,
        account_expansion, // AccountExpansionLevel: raw from DB, like C#
        54261,             // build
        session_key_raw,
        account.locale.clone(),
        pkt_rx,
        send_tx,
    );

    // Configure session with resources
    if let Some(ref db) = resources.char_db {
        session.set_char_db(Arc::clone(db));
    }
    if let Some(ref db) = resources.login_db {
        session.set_login_db(Arc::clone(db));
    }
    session.set_battlenet_account_id(account.battlenet_account_id);
    if let Some(ref generator) = resources.guid_generator {
        session.set_guid_generator(Arc::clone(generator));
    }
    if let Some(ref mgr) = resources.instance_lock_mgr {
        session.set_instance_lock_mgr(Arc::clone(mgr));
    }
    if let Some(ref db) = resources.world_db {
        session.set_world_db(Arc::clone(db));
    }
    if let Some(ref store) = resources.currency_types_store {
        session.set_currency_types_store(Arc::clone(store));
    }
    if let Some(ref stores) = resources.import_price_stores {
        session.set_import_price_stores(Arc::clone(stores));
    }
    if let Some(ref store) = resources.item_class_store {
        session.set_item_class_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_currency_cost_store {
        session.set_item_currency_cost_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_extended_cost_store {
        session.set_item_extended_cost_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_store {
        session.set_item_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_appearance_store {
        session.set_item_appearance_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_modified_appearance_store {
        session.set_item_modified_appearance_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_price_base_store {
        session.set_item_price_base_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.player_stats {
        session.set_player_stats(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_stats_store {
        session.set_item_stats_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_random_suffix_store {
        session.set_item_random_suffix_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_random_properties_store {
        session.set_item_random_properties_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.rand_prop_points_store {
        session.set_rand_prop_points_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_random_enchantment_template_store {
        session.set_item_random_enchantment_template_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_disenchant_loot_store {
        session.set_item_disenchant_loot_store(Arc::clone(store));
    }
    if let Some(ref stores) = resources.loot_stores {
        session.set_loot_stores(Arc::clone(stores));
    }
    if let Some(ref store) = resources.condition_store {
        session.set_condition_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.player_condition_store {
        session.set_player_condition_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.disable_mgr {
        session.set_disable_mgr(Arc::clone(store));
    }
    if let Some(ref store) = resources.lock_store {
        session.set_lock_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.spell_item_enchantment_store {
        session.set_spell_item_enchantment_store(Arc::clone(store));
    }
    if let Some(ref cache) = resources.hotfix_blob_cache {
        session.set_hotfix_blob_cache(Arc::clone(cache));
    }
    if let Some(ref store) = resources.skill_store {
        session.set_skill_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.spell_store {
        session.set_spell_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.area_table_store {
        session.set_area_table_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.area_trigger_store {
        session.set_area_trigger_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.chr_specialization_store {
        session.set_chr_specialization_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.dungeon_encounter_store {
        session.set_dungeon_encounter_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.map_store {
        session.set_map_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.map_difficulty_store {
        session.set_map_difficulty_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.map_difficulty_x_condition_store {
        session.set_map_difficulty_x_condition_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.creature_template_mount_store {
        session.set_creature_template_mount_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.creature_display_info_store {
        session.set_creature_display_info_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.creature_model_data_store {
        session.set_creature_model_data_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.mount_store {
        session.set_mount_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.mount_capability_store {
        session.set_mount_capability_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.mount_type_x_capability_store {
        session.set_mount_type_x_capability_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.mount_x_display_store {
        session.set_mount_x_display_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.vehicle_store {
        session.set_vehicle_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.vehicle_seat_store {
        session.set_vehicle_seat_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.vehicle_template_store {
        session.set_vehicle_template_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.vehicle_accessory_store {
        session.set_vehicle_accessory_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.terrain_swap_store {
        session.set_terrain_swap_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.phase_store {
        session.set_phase_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.phase_group_store {
        session.set_phase_group_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.quest_store {
        session.set_quest_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.quest_xp_store {
        session.set_quest_xp_store(Arc::clone(store));
    }
    if let Some(ref table) = resources.player_xp_table {
        session.set_player_xp_table(Arc::clone(table));
    }
    if let Some(ref registry) = resources.player_registry {
        session.set_player_registry(Arc::clone(registry));
    }
    session.set_loot_drop_rates_like_cpp(resources.loot_drop_rates);
    session.set_enable_ae_loot_like_cpp(resources.enable_ae_loot);
    session.set_mmap_runtime_config_like_cpp(mmap_runtime_config);
    if let Some(pathfinder) = mmap_pathfinder {
        session.set_mmap_pathfinder_like_cpp(pathfinder);
    }
    session.set_object_accessor(object_accessor);
    if let (Some(greg), Some(pinv)) = (&resources.group_registry, &resources.pending_invites) {
        session.set_group_registry(Arc::clone(greg), Arc::clone(pinv));
    }
    session.set_realm_id(resources.realm_id);
    session.set_map_manager(shared_map);

    // Select the correct realm IP for ConnectTo based on client address.
    // C# logic: loopback → localAddress, otherwise → externalAddress.
    // For LAN clients, use localAddress if they're in the same subnet.
    let connect_ip = get_address_for_client(
        account.client_address,
        resources.realm_external_address,
        resources.realm_local_address,
    );

    // Configure ConnectTo flow — client needs an instance connection
    // for movement/interaction packets (UpdateObject, MoveSetActiveMover,
    // all movement opcodes use ConnectionType.Instance in C#).
    session.set_session_mgr(session_mgr);
    session.set_instance_endpoint(connect_ip, instance_port);

    // Send session init packets (AuthResponse + glue screen data).
    // These are the first encrypted packets the client receives.
    session.send_session_init_packets();

    info!("Session ready for account {}", account.id);

    // Session update loop
    loop {
        // Process incoming packets
        let count = session.update(50);

        // Dispatch pending packets (async handlers)
        session.process_pending().await;

        if session.is_disconnecting() {
            info!("Session for account {} disconnecting", account.id);
            break;
        }

        // Sleep to avoid busy-waiting (50ms tick)
        if count == 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }
    session
        .cleanup_shared_runtime_state_on_disconnect_like_cpp()
        .await;
}

/// Load realm external and local addresses from the `realmlist` table.
///
/// C# stores these as `address` (external/public) and `localAddress` (LAN).
/// Returns `([external_ip], [local_ip])`.
async fn load_realm_addresses(
    login_db: &LoginDatabase,
    realm_id: u16,
) -> Result<([u8; 4], [u8; 4])> {
    let result = login_db
        .direct_query(&format!(
            "SELECT address, localAddress FROM realmlist WHERE id = {realm_id}"
        ))
        .await
        .context("Failed to query realmlist for addresses")?;

    if result.is_empty() {
        anyhow::bail!("Realm {realm_id} not found in realmlist table");
    }

    let external_str: String = result.read_string(0);
    let local_str: String = result.read_string(1);

    let external = parse_ipv4(&external_str).unwrap_or([127, 0, 0, 1]);
    let local = parse_ipv4(&local_str).unwrap_or([127, 0, 0, 1]);

    Ok((external, local))
}

/// Select the correct realm IP for a client, matching C#'s `GetAddressForClient`.
///
/// - Loopback client (127.0.0.1) → local address
/// - LAN client (same /24 subnet as local address) → local address
/// - Everything else → external (public) address
fn get_address_for_client(
    client_ip: Option<std::net::IpAddr>,
    external: [u8; 4],
    local: [u8; 4],
) -> [u8; 4] {
    let client = match client_ip {
        Some(std::net::IpAddr::V4(v4)) => v4.octets(),
        _ => return external, // unknown or IPv6 → external
    };

    // Loopback → local
    if client[0] == 127 {
        return local;
    }

    // Same /24 subnet as local address → local
    if client[0] == local[0] && client[1] == local[1] && client[2] == local[2] {
        return local;
    }

    external
}

/// Format an IPv4 address for display.
fn format_ipv4(ip: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

/// Decode a hex string into raw bytes.
fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let hex = hex.trim();
    if hex.is_empty() {
        return Vec::new();
    }
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Map DBC.Locale config value to the folder name.
///
/// The config can be a numeric ID (C# style) or already a locale name.
/// WoW locale IDs: 0=enUS, 1=koKR, 2=frFR, 3=deDE, 4=zhCN, 5=zhTW,
/// 6=esES, 7=esMX, 8=ruRU, 9=jaJP, 10=ptBR, 11=itIT.
fn locale_id_to_name(raw: &str) -> String {
    match raw.trim() {
        "0" => "enUS".into(),
        "1" => "koKR".into(),
        "2" => "frFR".into(),
        "3" => "deDE".into(),
        "4" => "zhCN".into(),
        "5" => "zhTW".into(),
        "6" => "esES".into(),
        "7" => "esMX".into(),
        "8" => "ruRU".into(),
        "9" => "jaJP".into(),
        "10" => "ptBR".into(),
        "11" => "itIT".into(),
        other => other.into(), // already a name like "esES"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        load_world_config_from, loot_drop_rates_like_cpp, mmap_runtime_config_like_cpp,
        world_config_bool, world_config_u8, world_config_u16,
    };
    use std::collections::HashSet;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn world_config_resolution_prefers_lowercase_cpp_name() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        let root = unique_temp_dir("world_config_resolution");
        let lower = root.join("worldserver.conf");
        let legacy = root.join("WorldServer.conf");

        fs::write(&lower, "WorldServerPort = 8085\n").expect("write lower failed");
        fs::write(&legacy, "WorldServerPort = 9000\n").expect("write legacy failed");

        let report = load_world_config_from(
            &[
                lower.to_str().expect("utf8 path"),
                legacy.to_str().expect("utf8 path"),
            ],
            root.join("worldserver.conf.d").to_str().expect("utf8 path"),
        )
        .expect("config should load");

        assert_eq!(report.candidate_index, 0);
        assert_eq!(wow_config::get_value::<u16>("WorldServerPort"), Some(8085));

        fs::remove_dir_all(root).expect("cleanup failed");
    }

    #[test]
    fn world_network_config_uses_resolved_world_configs() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        wow_config::load_config_from_str(
            r#"
WorldServerPort = 70000
InstanceServerPort = 70001
Expansion = 9
"#,
        )
        .expect("config should load");

        let configs = wow_config::load_world_config_values();
        assert_eq!(world_config_u16(&configs, "CONFIG_PORT_WORLD", 8085), 4464);
        assert_eq!(
            world_config_u16(&configs, "CONFIG_PORT_INSTANCE", 8086),
            4465
        );
        assert_eq!(world_config_u8(&configs, "CONFIG_EXPANSION", 2), 9);
    }

    #[test]
    fn loot_drop_rates_use_cpp_world_config_keys() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        wow_config::load_config_from_str(
            r#"
Rate.Drop.Item.Poor = 0.5
Rate.Drop.Item.Rare = 3
Rate.Drop.Item.Referenced = 4
Rate.Drop.Item.ReferencedAmount = 2
Rate.Drop.Money = 6
"#,
        )
        .expect("config should load");

        let configs = wow_config::load_world_config_values();
        let rates = loot_drop_rates_like_cpp(&configs);
        assert_eq!(rates.item_poor, 0.5);
        assert_eq!(rates.item_normal, 1.0);
        assert_eq!(rates.item_rare, 3.0);
        assert_eq!(rates.item_referenced, 4.0);
        assert_eq!(rates.item_referenced_amount, 2.0);
        assert_eq!(rates.money, 6.0);
    }

    #[test]
    fn enable_ae_loot_uses_cpp_world_config_key() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        wow_config::load_config_from_str("EnableAELoot = 1\n").expect("config should load");

        let configs = wow_config::load_world_config_values();
        assert!(world_config_bool(&configs, "CONFIG_ENABLE_AE_LOOT", false));
    }

    #[test]
    fn mmap_runtime_config_uses_cpp_world_config_key_and_data_dir() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        wow_config::load_config_from_str(
            r#"
DataDir = "/srv/wow-data"
mmap.enablePathFinding = 0
"#,
        )
        .expect("config should load");

        let configs = wow_config::load_world_config_values();
        let mmap_config = mmap_runtime_config_like_cpp(&configs, HashSet::from([1]));
        assert_eq!(mmap_config.data_dir, "/srv/wow-data");
        assert!(!mmap_config.enabled);
        assert!(!mmap_config.pathfinding_enabled_for_map_like_cpp(0));
        assert!(!mmap_config.pathfinding_enabled_for_map_like_cpp(1));
    }

    #[test]
    fn mmap_runtime_config_applies_cpp_disable_mgr_map_gate() {
        let _guard = TEST_LOCK.lock().expect("test lock poisoned");
        wow_config::load_config_from_str("mmap.enablePathFinding = 1\n")
            .expect("config should load");

        let configs = wow_config::load_world_config_values();
        let mmap_config = mmap_runtime_config_like_cpp(&configs, HashSet::from([571]));
        assert!(mmap_config.pathfinding_enabled_for_map_like_cpp(0));
        assert!(!mmap_config.pathfinding_enabled_for_map_like_cpp(571));
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "rustycore_world_server_{name}_{}",
            std::process::id()
        ));

        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir failed");
        path
    }
}
