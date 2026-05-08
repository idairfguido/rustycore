// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! World Server — entry point.
//!
//! Accepts WoW client connections after BNet authentication, performs the
//! world-server handshake (challenge → auth → encryption), creates a
//! WorldSession for each client, and dispatches packets to handlers.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::info;
use wow_config::{DatabaseInfo, LoadReport, WorldConfigSet};
use wow_core::{ObjectGuidGenerator, guid::HighGuid};
use wow_database::{
    CharStatements, CharacterDatabase, HotfixDatabase, LoginDatabase, LoginStatements,
    WorldDatabase, build_connection_string,
};
use wow_network::session_mgr::SessionManager;
use wow_network::world_socket::{AccountInfo, AccountLookup};
use wow_network::{GroupRegistry, PendingInvites, PlayerRegistry, SessionResources};
use wow_world::{MapManager as LegacyMapManager, SharedMapManager, WorldSession};

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

    let item_store = Arc::new(
        wow_data::ItemStore::load(&data_dir, &locale)
            .context("Failed to load Item.db2 — check DataDir and DBC.Locale config")?,
    );
    info!("Loaded {} items from Item.db2", item_store.len());

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

    // Load ItemRandomSuffix.db2 for C++ ApplyEnchantment random suffix amount resolution.
    let item_random_suffix_store = Arc::new(
        wow_data::ItemRandomSuffixStore::load(&data_dir, &locale)
            .context("Failed to load ItemRandomSuffix.db2 — check DataDir and DBC.Locale config")?,
    );
    info!(
        "Loaded {} item random suffixes from ItemRandomSuffix.db2",
        item_random_suffix_store.len()
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

    // Diagnostic: check if known problem items exist in cache
    for item_id in [58256i32, 58274, 58257] {
        let has = hotfix_blob_cache.get(0x919BE54E, item_id); // ItemSparse table hash
        info!(
            "HotfixBlobCache check: ItemSparse record {} → {}",
            item_id,
            if let Some(b) = has {
                format!("FOUND ({} bytes)", b.len())
            } else {
                "NOT FOUND".into()
            }
        );
    }

    // Load SkillLineAbility.db2 + SkillRaceClassInfo.db2 for auto-learned spells
    let skill_store = Arc::new(
        wow_data::SkillStore::load(&data_dir, &locale)
            .context("Failed to load SkillLineAbility/SkillRaceClassInfo DB2 files")?,
    );

    // Load spell metadata (cast time, cooldown, effects, etc.) — Phase 2
    let spell_store = Arc::new(
        wow_data::SpellStore::load(&hotfix_db)
            .await
            .context("Failed to load SpellStore")?,
    );
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

    // Load player_xp_for_level table
    let player_xp_table = {
        use wow_database::WorldStatements;
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

    // Shared group registry and pending invites
    let group_registry = Arc::new(GroupRegistry::new());
    let pending_invites = Arc::new(PendingInvites::new());

    // Shared world state (creatures/grids visible to every session on the same map).
    // Each session gets a clone of this Arc on creation.
    let shared_map: SharedMapManager = Arc::new(std::sync::RwLock::new(LegacyMapManager::new()));

    let canonical_map_manager = Arc::new(Mutex::new(create_canonical_map_manager(&world_configs)));

    // Build session resources
    let session_resources = Arc::new(SessionResources {
        char_db: Some(Arc::clone(&char_db)),
        login_db: Some(Arc::clone(&login_db)),
        world_db: Some(Arc::clone(&world_db)),
        guid_generator: Some(Arc::clone(&guid_generator)),
        currency_types_store: Some(Arc::clone(&currency_types_store)),
        item_store: Some(Arc::clone(&item_store)),
        item_appearance_store: Some(Arc::clone(&item_appearance_store)),
        item_modified_appearance_store: Some(Arc::clone(&item_modified_appearance_store)),
        player_stats: Some(Arc::clone(&player_stats)),
        item_stats_store: Some(Arc::clone(&item_stats_store)),
        item_random_suffix_store: Some(Arc::clone(&item_random_suffix_store)),
        spell_item_enchantment_store: Some(Arc::clone(&spell_item_enchantment_store)),
        hotfix_blob_cache: Some(Arc::clone(&hotfix_blob_cache)),
        skill_store: Some(Arc::clone(&skill_store)),
        spell_store: Some(Arc::clone(&spell_store)),
        area_trigger_store: Some(Arc::clone(&area_trigger_store)),
        quest_store: Some(Arc::clone(&quest_store)),
        quest_xp_store: Some(Arc::clone(&quest_xp_store)),
        player_xp_table: Some(Arc::clone(&player_xp_table)),
        player_registry: Some(Arc::clone(&player_registry)),
        group_registry: Some(Arc::clone(&group_registry)),
        pending_invites: Some(Arc::clone(&pending_invites)),
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
        async move {
            if let Err(e) = wow_network::start_world_listener(
                realm_addr,
                lookup,
                resources,
                move |account, pkt_rx, send_tx, res| {
                    let mgr = Arc::clone(&mgr);
                    let smap = Arc::clone(&smap);
                    let accessor = Arc::clone(&accessor);
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

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
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

    info!("World server stopped.");
    Ok(())
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
    if let Some(ref generator) = resources.guid_generator {
        session.set_guid_generator(Arc::clone(generator));
    }
    if let Some(ref db) = resources.world_db {
        session.set_world_db(Arc::clone(db));
    }
    if let Some(ref store) = resources.currency_types_store {
        session.set_currency_types_store(Arc::clone(store));
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
    if let Some(ref store) = resources.player_stats {
        session.set_player_stats(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_stats_store {
        session.set_item_stats_store(Arc::clone(store));
    }
    if let Some(ref store) = resources.item_random_suffix_store {
        session.set_item_random_suffix_store(Arc::clone(store));
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
    if let Some(ref store) = resources.area_trigger_store {
        session.set_area_trigger_store(Arc::clone(store));
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
    session.cleanup_shared_runtime_state();
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
    use super::{load_world_config_from, world_config_u8, world_config_u16};
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
