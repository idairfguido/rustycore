// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! `WorldSession` — per-player session that receives packets from the
//! [`WorldSocket`](wow_network::WorldSocket) and dispatches them to handlers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tracing::{debug, info, trace, warn};

use wow_constants::{
    BagFamilyMask, BuyResult, ClientOpcodes, InventoryResult, InventoryType, ItemBondingType,
    ItemClass, ItemContext, ItemEnchantmentType, ItemFlags, ItemFlags2, SellResult, TypeId,
    TypeMask,
};
use wow_constants::item::{CurrencyTypes, CurrencyTypesFlags};
use wow_constants::unit::Team;
use wow_core::{ObjectGuid, ObjectGuidGenerator};
use wow_data::{
    AreaTriggerStore, CurrencyTypesEntry, CurrencyTypesStore, HotfixBlobCache, ItemAppearanceStore,
    ItemExtendedCostStore, ItemModifiedAppearanceStore, ItemRandomSuffixStore, ItemStatsStore,
    ItemStore, PlayerStatsStore, SkillStore, SpellItemEnchantmentStore, SpellStore,
};
use wow_database::{
    CharStatements, CharacterDatabase, LoginDatabase, PreparedStatement, SqlTransaction,
    StatementDef, WorldDatabase,
};
use wow_entities::{
    ApplyEnchantmentEffectRef, ApplyEnchantmentRandomSuffixRef, ApplyEnchantmentTemplateRef,
    BANK_SLOT_BAG_END, BANK_SLOT_BAG_START, CanStoreItemArgs, CanUnequipItemArgs,
    INVENTORY_DEFAULT_SIZE, INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_END,
    INVENTORY_SLOT_BAG_START, Item, ItemCreateInfo, ItemPosCount, ItemSlotRef, ItemStorageRef,
    ItemStorageTemplate, REAGENT_BAG_SLOT_END, REAGENT_BAG_SLOT_START,
    BUYBACK_SLOT_COUNT, BUYBACK_SLOT_END, BUYBACK_SLOT_START, NULL_BAG, NULL_SLOT,
    ObjectAccessor, PLAYER_SLOT_END, Player, PlayerEnchantTimeUpdate, PlayerInventoryStorage,
    PlayerItemTimeUpdate, SendNewItemDelivery, SendNewItemDisplayText, SendNewItemPlan,
    WorldObject, is_bag_pos, is_equipment_packed_pos, make_item_pos, MAX_ITEM_SPELLS,
};
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus, build_dispatch_table};
use wow_network::session_mgr::{InstanceLink, SessionManager};
use wow_network::{GroupRegistry, PendingInvites, PlayerBroadcastInfo, PlayerRegistry};
use wow_packet::packets::item::{
    InventoryChangeFailure, ItemEnchantTimeUpdate, ItemInstance, ItemMod, ItemModList,
    ItemPushResult, ItemPushResultDisplayType, ItemTimeUpdate,
};
use wow_packet::packets::misc::{BuyFailed, SellResponse};
use wow_packet::{ClientPacket, WorldPacket};

/// Maximum number of packets processed per `update()` call.
const MAX_PACKETS_PER_UPDATE: usize = 100;

pub type SharedObjectAccessor = Arc<RwLock<ObjectAccessor>>;

pub fn new_shared_object_accessor() -> SharedObjectAccessor {
    Arc::new(RwLock::new(ObjectAccessor::default()))
}

/// Current state of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Authenticated but no character selected.
    Authed,
    /// Character is logged into the world.
    LoggedIn,
    /// Character is transferring between maps.
    Transfer,
    /// Session is being disconnected.
    Disconnecting,
}

/// Spell casting state — tracks an in-progress spell cast with a timer.
///
/// Used for spells with cast time > 0. When the player initiates a cast,
/// this stores the spell ID, target, and cast start time. The main loop
/// calls `tick_active_spell_cast()` each frame to check if casting completes.
#[derive(Debug, Clone)]
pub struct SpellCastState {
    /// Spell ID being cast.
    pub spell_id: i32,
    /// Target GUID for the spell.
    pub target_guid: ObjectGuid,
    /// Client's cast ID (for SMSG_SPELL_GO echo).
    pub cast_id: ObjectGuid,
    /// When the cast started (Instant::now()).
    pub cast_start_time: Instant,
    /// Total cast time in milliseconds.
    pub cast_time_ms: u32,
    /// Spell visual IDs.
    pub spell_visual: wow_packet::packets::spell::SpellCastVisual,
}

/// Per-player session on the world server.
///
/// Receives deserialized packets from the socket layer via a channel,
/// dispatches them to registered handlers, and sends responses back.
pub struct WorldSession {
    // Account info
    pub account_id: u32,
    pub account_name: String,
    pub security: u8,
    pub expansion: u8,
    pub account_expansion: u8,
    pub build: u32,
    pub session_key: Vec<u8>,
    pub locale: String,

    // Inbound packet queue (from WorldSocket)
    packet_rx: flume::Receiver<WorldPacket>,

    // Outbound channel (serialized bytes back to WorldSocket)
    send_tx: flume::Sender<Vec<u8>>,

    // State
    state: SessionState,
    last_packet_time: Instant,

    // Dispatch table (built once, shared ref)
    dispatch_table: HashMap<ClientOpcodes, &'static PacketHandlerEntry>,

    // Character database
    char_db: Option<Arc<CharacterDatabase>>,

    // Login database (for realmcharacters updates)
    login_db: Option<Arc<LoginDatabase>>,

    // World database (for creature templates, spawns, etc.)
    world_db: Option<Arc<WorldDatabase>>,

    // Currency types store (CurrencyTypes.db2 data)
    currency_types_store: Option<Arc<CurrencyTypesStore>>,

    // Item extended cost store (ItemExtendedCost.db2 data)
    item_extended_cost_store: Option<Arc<ItemExtendedCostStore>>,

    // Item store (Item.db2 BasicData — class/subclass)
    item_store: Option<Arc<ItemStore>>,

    // Item appearance store (ItemAppearance.db2 data)
    item_appearance_store: Option<Arc<ItemAppearanceStore>>,

    // Item modified appearance store (ItemModifiedAppearance.db2 data)
    item_modified_appearance_store: Option<Arc<ItemModifiedAppearanceStore>>,

    // Player level stats store (race/class/level → base stats)
    player_stats: Option<Arc<PlayerStatsStore>>,

    // Item stat modifiers store (item_id → stat bonuses from ItemSparse.db2)
    item_stats_store: Option<Arc<ItemStatsStore>>,

    // Item random suffix store (ItemRandomSuffix.db2 data)
    item_random_suffix_store: Option<Arc<ItemRandomSuffixStore>>,

    // Spell item enchantment store (SpellItemEnchantment.db2 data)
    spell_item_enchantment_store: Option<Arc<SpellItemEnchantmentStore>>,

    // Hotfix blob cache: raw DB2 record bytes for DBReply responses
    hotfix_blob_cache: Option<Arc<HotfixBlobCache>>,

    // Skill store (auto-learned spells from SkillLineAbility.db2 + SkillRaceClassInfo.db2)
    skill_store: Option<Arc<SkillStore>>,

    // Area trigger store (collision detection + teleportation)
    area_trigger_store: Option<Arc<AreaTriggerStore>>,

    // Shared player registry for broadcasting to nearby sessions
    player_registry: Option<Arc<PlayerRegistry>>,

    // Shared C++-style ObjectAccessor for canonical in-world/player-owned object lookups.
    object_accessor: Option<SharedObjectAccessor>,

    // Shared group registry for party management
    group_registry: Option<Arc<GroupRegistry>>,

    // Pending party invites: invited_guid → inviter_guid
    pending_invites: Option<Arc<PendingInvites>>,

    // Group state for this session
    pub(crate) group_guid: Option<u64>,

    // Realm ID for GUID creation
    realm_id: u16,

    // GUID generator for new characters
    guid_generator: Option<Arc<ObjectGuidGenerator>>,

    // Characters confirmed for this account
    legit_characters: Vec<ObjectGuid>,

    // Pending async packets to process
    pending_packets: Vec<WorldPacket>,

    // ── ConnectTo flow ──────────────────────────────────────────
    /// GUID of the character being logged in (set during PlayerLogin).
    player_loading: Option<ObjectGuid>,

    /// The ConnectToKey.Raw value for the pending instance connection.
    connect_to_key: Option<i64>,

    /// The last ConnectToSerial used (for retry logic).
    connect_to_serial: Option<wow_packet::packets::auth::ConnectToSerial>,

    /// Session manager for ConnectTo flow (shared with instance listener).
    session_mgr: Option<Arc<SessionManager>>,

    /// Instance server address (IP for ConnectTo packet).
    instance_address: [u8; 4],

    /// Instance server port.
    instance_port: u16,

    /// Oneshot receiver for instance link delivery.
    instance_link_rx: Option<tokio::sync::oneshot::Receiver<InstanceLink>>,

    // ── Time sync ─────────────────────────────────────────────────
    /// Next sequence index for TimeSyncRequest.
    pub(crate) time_sync_next_counter: u32,

    /// Time remaining until next TimeSyncRequest (in ms).
    pub(crate) time_sync_timer_ms: u32,

    // ── Logout ──────────────────────────────────────────────────────
    /// When set, the session is counting down to logout (20s timer).
    /// `None` means no logout is pending.
    pub(crate) logout_time: Option<Instant>,
    /// Timestamp set when the player enters the world (PlayerLogin).
    pub(crate) login_time: Option<Instant>,
    /// Total played time loaded from DB (seconds).
    pub(crate) total_played_time: u32,
    /// Time played at current level loaded from DB (seconds).
    pub(crate) level_played_time: u32,
    /// Player's current money in copper (1 gold = 10,000 copper).
    /// Loaded from `characters.money` on login; saved on logout + buy/sell.
    pub(crate) player_gold: u64,
    pub(crate) player_xp: u32,
    /// XP required to reach next level, cached from player_xp_for_level.
    pub(crate) player_next_level_xp: u32,
    /// Currently selected target GUID (SetSelection).
    pub(crate) selection_guid: Option<wow_core::ObjectGuid>,

    /// GUID of the character currently logged in (set after login completes).
    pub(crate) player_guid: Option<ObjectGuid>,

    /// Pending creature spawn request (set during login, processed async).
    pub(crate) pending_creature_spawn: Option<PendingCreatureSpawn>,
    /// Creatures waiting to respawn after corpse despawn.
    pub(crate) respawn_queue: Vec<PendingRespawn>,

    /// In-memory inventory: slot → (item ObjectGuid, entry_id, db_guid).
    pub(crate) inventory_items: HashMap<u8, InventoryItem>,

    /// In-memory buyback slots, kept separate from normal inventory like C++ `GetItemByGuid`.
    pub(crate) buyback_items: HashMap<u8, InventoryItem>,
    pub(crate) buyback_price: [u32; BUYBACK_SLOT_COUNT],
    pub(crate) buyback_timestamp: [i64; BUYBACK_SLOT_COUNT],
    pub(crate) current_buyback_slot: u8,

    /// C++ `_currencyStorage`, keyed by CurrencyTypes.db2 ID.
    pub(crate) player_currencies: HashMap<u32, PlayerCurrency>,

    /// In-memory item objects keyed by item GUID, mirroring C++ `Player::m_items` ownership.
    pub(crate) inventory_item_objects: HashMap<ObjectGuid, Item>,

    /// Current map ID for VALUES update packets.
    pub(crate) current_map_id: u16,

    /// Race of the currently logged-in character (set at login).
    pub(crate) player_race: u8,
    /// Class of the currently logged-in character (set at login).
    pub(crate) player_class: u8,
    /// Level of the currently logged-in character (set at login).
    pub(crate) player_level: u8,
    /// Gender of the currently logged-in character (set at login).
    pub(crate) player_gender: u8,
    /// All known spell IDs for the logged-in character (DB + DBC merged).
    pub(crate) known_spells: Vec<i32>,

    // ── Dual-connection (realm + instance) ───────────────────────
    // After ConnectTo completes, the session uses the instance socket for
    // game packets but MUST keep the realm socket alive — the WoW client
    // disconnects if either connection drops.
    /// Realm packet receiver — kept alive after ConnectTo to prevent realm
    /// socket closure.  Also drained in `update()` for realm-type packets.
    realm_packet_rx: Option<flume::Receiver<WorldPacket>>,
    /// Realm send channel — kept alive so the realm writer task persists.
    realm_send_tx: Option<flume::Sender<Vec<u8>>>,

    // ── Movement & World position ─────────────────────────────────
    /// Server-side position of the player (updated from CMSG_MOVE_*).
    pub(crate) player_position: Option<wow_core::Position>,

    /// Cached character name for chat messages.
    pub(crate) player_name: Option<String>,

    // ── Creature AI tracking ──────────────────────────────────────
    /// All creatures visible/tracked by this session, keyed by GUID.
    /// Legacy per-session storage. New code should prefer `MapManager` access
    /// (see `map_manager` field) which is shared across sessions on the same map.
    pub(crate) creatures: std::collections::HashMap<wow_core::ObjectGuid, wow_ai::CreatureAI>,
    /// Per-session finite vendor stock state, mirroring Creature::m_vendorItemCounts
    /// until vendor ownership moves into the shared creature model.
    pub(crate) vendor_item_counts: HashMap<(wow_core::ObjectGuid, u32), VendorItemCount>,

    /// Tick counter for creature movement (throttle to every N ticks).
    pub(crate) creature_tick: u32,

    /// Shared, server-wide map state. When `Some`, creature reads/writes can
    /// route through here so all sessions on the same map see the same world.
    /// `None` until the world server injects the manager (see `set_map_manager`).
    pub(crate) map_manager: Option<crate::map_manager::SharedMapManager>,

    // ── Combat state ─────────────────────────────────────────────
    /// Current auto-attack target (None if not in combat).
    pub(crate) combat_target: Option<wow_core::ObjectGuid>,

    /// True when the player is engaged in combat.
    pub(crate) in_combat: bool,

    // ── Aura system ───────────────────────────────────────────────
    /// All visible auras on the player: slot (0-254) → AuraApplication
    pub(crate) visible_auras: HashMap<u8, AuraApplication>,

    // ── Spell casting ──────────────────────────────────────────────
    /// Spell store (metadata for all known spells: cast time, cooldown, effects, etc.)
    pub spell_store: Option<Arc<SpellStore>>,
    /// Currently active spell cast (if any). Set when a cast starts, cleared when it completes.
    pub(crate) active_spell_cast: Option<SpellCastState>,
    /// Last time a spell was executed (used to enforce global cooldown timers).
    pub(crate) last_spell_cast_time: Option<Instant>,
    /// Per-spell cooldown tracking: spell_id → last cast time.
    /// Used to enforce spell-specific cooldown timers.
    pub(crate) last_spell_cast_time_per_spell: HashMap<i32, Instant>,

    // ── Quest system ───────────────────────────────────────────────
    /// Quest template store (loaded from world DB at startup).
    pub(crate) quest_store: Option<Arc<wow_data::quest::QuestStore>>,
    pub(crate) quest_xp_store: Option<Arc<wow_data::quest_xp::QuestXpStore>>,
    pub(crate) player_xp_table: Option<Arc<Vec<u32>>>,
    /// Active quests for this player: quest_id → status.
    pub(crate) player_quests: HashMap<u32, crate::handlers::quest::PlayerQuestStatus>,
    /// Quests the player has already been rewarded for (non-repeatable quests cannot be re-taken).
    /// C# ref: m_RewardedQuests
    pub(crate) rewarded_quests: std::collections::HashSet<u32>,

    // ── Loot ──────────────────────────────────────────────────────
    /// Active loot windows keyed by creature GUID.
    pub(crate) loot_table: std::collections::HashMap<wow_core::ObjectGuid, wow_packet::packets::loot::CreatureLoot>,
    /// Mirrors C++ PlayerData::LootTargetGUID for guards that compare active loot by GUID.
    pub(crate) active_loot_guid: wow_core::ObjectGuid,

    // ── Dynamic visibility tracking ───────────────────────────────
    /// GUIDs of all creatures currently visible to this client.
    /// Updated on login and each visibility refresh (player movement).
    pub(crate) visible_creatures: std::collections::HashSet<wow_core::ObjectGuid>,
    /// GUIDs of all game objects currently visible to this client.
    pub(crate) visible_gameobjects: std::collections::HashSet<wow_core::ObjectGuid>,
    /// Position at which visibility was last fully recalculated.
    pub(crate) last_visibility_pos: Option<wow_core::Position>,

    // ── Gossip state ──────────────────────────────────────────────
    /// Active gossip options for the NPC the player is talking to.
    /// Stored when SMSG_GOSSIP_MESSAGE is sent, used when CMSG_GOSSIP_SELECT_OPTION arrives.
    pub(crate) gossip_options: Vec<GossipOptionInfo>,
    /// GUID of the NPC the current gossip menu belongs to.
    pub(crate) gossip_source_guid: Option<wow_core::ObjectGuid>,

    // ── Area trigger tracking ──────────────────────────────────────
    /// Currently active area trigger ID (to prevent retriggering on same position).
    /// Set to Some(trigger_id) when entered, None when exited.
    pub(crate) active_area_trigger: Option<u32>,
    /// Pending far teleport destination (map_id, position).
    /// Set by `teleport_to`, consumed by `handle_world_port_response`.
    pub(crate) pending_teleport: Option<(u32, wow_core::Position)>,

    // ── QueryCreature cache ────────────────────────────────────────
    /// Creature entry IDs for which we've already sent a QueryCreatureResponse.
    /// The client caches the response locally, so we skip duplicates.
    pub(crate) creature_query_cache: std::collections::HashSet<u32>,
}

/// A gossip option stored server-side for routing when the player selects it.
#[derive(Debug, Clone)]
pub struct GossipOptionInfo {
    pub gossip_option_id: i32,
    pub option_npc: u8,
    pub action_menu_id: u32,
}

/// An item tracked in the session's in-memory inventory.
#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub guid: ObjectGuid,
    pub entry_id: u32,
    pub db_guid: u64,
    /// InventoryType from Item.db2 (e.g. 1=Head, 5=Chest, 13=Weapon).
    /// Loaded from the item store at login, with slot-based fallback.
    pub inventory_type: Option<u8>,
}

/// Current finite stock for a vendor item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VendorItemCount {
    pub count: u32,
    pub last_increment_time: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PlayerCurrencyState {
    Unchanged = 0,
    Changed = 1,
    New = 2,
    Removed = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlayerCurrency {
    pub state: PlayerCurrencyState,
    pub quantity: u32,
    pub weekly_quantity: u32,
    pub tracked_quantity: u32,
    pub increased_cap_quantity: u32,
    pub earned_quantity: u32,
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlayerCurrencyDelta {
    pub currency_id: u32,
    pub quantity: u32,
    pub amount: u32,
    pub weekly_quantity: Option<u32>,
    pub max_quantity: Option<u32>,
    pub total_earned: Option<u32>,
    pub suppress_chat_log: bool,
}

fn player_team_for_race_cpp(race: u8) -> Team {
    match race {
        2 | 5 | 6 | 8 | 9 | 10 | 26 | 27 | 28 | 31 | 35 | 36 | 70 => Team::Horde,
        _ => Team::Alliance,
    }
}

fn currency_max_quantity_cpp(entry: &CurrencyTypesEntry, currency: &PlayerCurrency) -> u32 {
    if !entry.has_max_quantity(false, false) {
        return 0;
    }

    let mut max_quantity = entry.max_qty;
    if entry.flags.contains(CurrencyTypesFlags::DYNAMIC_MAXIMUM) {
        max_quantity = max_quantity.saturating_add(currency.increased_cap_quantity);
    }
    max_quantity
}

/// An aura applied to the player.
#[derive(Debug, Clone)]
pub struct AuraApplication {
    /// Spell ID of the aura
    pub spell_id: i32,
    /// GUID of the unit that cast the aura
    pub caster_guid: ObjectGuid,
    /// Aura slot (0-254)
    pub slot: u8,
    /// Total duration in milliseconds (0 = permanent)
    pub duration_total: u32,
    /// Remaining duration in milliseconds
    pub duration_remaining: u32,
    /// Stack count
    pub stack_count: u8,
    /// Aura flags (bitmask)
    pub aura_flags: u32,
    /// Monotonic timestamp when this aura was applied — used for expiry checks.
    pub applied_at: Instant,
}

/// Parameters for spawning nearby creatures after login.
pub struct PendingCreatureSpawn {
    pub map_id: u16,
    pub position: wow_core::Position,
    pub zone_id: u32,
}

/// A creature waiting to respawn after its corpse despawned.
///
/// Stored in `WorldSession::respawn_queue`; processed by `tick_creatures_sync`.
/// C# ref: `Creature::AllLootRemovedFromCorpse` → `m_respawnTime` → `Map::AddToMap`.
pub struct PendingRespawn {
    /// When to respawn.
    pub respawn_at: std::time::Instant,
    /// Home position (spawn point).
    pub home_pos: wow_core::Position,
    /// Full create data — reused verbatim for the respawn CREATE packet.
    pub create_data: wow_packet::packets::update::CreatureCreateData,
    /// AI fields needed to rebuild `CreatureAI`.
    pub max_hp: u32,
    pub level: u8,
    pub min_dmg: u32,
    pub max_dmg: u32,
    pub aggro_radius: f32,
    pub npc_flags: u32,
    pub unit_flags: u32,
    pub map_id: u16,
}

fn is_represented_bag_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
        || (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&slot)
        || (REAGENT_BAG_SLOT_START..REAGENT_BAG_SLOT_END).contains(&slot)
}

impl WorldSession {
    /// Create a new session with the given account info and channels.
    pub fn new(
        account_id: u32,
        account_name: String,
        security: u8,
        expansion: u8,
        account_expansion: u8,
        build: u32,
        session_key: Vec<u8>,
        locale: String,
        packet_rx: flume::Receiver<WorldPacket>,
        send_tx: flume::Sender<Vec<u8>>,
    ) -> Self {
        Self {
            account_id,
            account_name,
            security,
            expansion,
            account_expansion,
            build,
            session_key,
            locale,
            packet_rx,
            send_tx,
            state: SessionState::Authed,
            last_packet_time: Instant::now(),
            dispatch_table: build_dispatch_table(),
            char_db: None,
            login_db: None,
            world_db: None,
            currency_types_store: None,
            item_extended_cost_store: None,
            item_store: None,
            item_appearance_store: None,
            item_modified_appearance_store: None,
            player_stats: None,
            item_stats_store: None,
            item_random_suffix_store: None,
            spell_item_enchantment_store: None,
            hotfix_blob_cache: None,
            skill_store: None,
            area_trigger_store: None,
            player_registry: None,
            object_accessor: None,
            group_registry: None,
            pending_invites: None,
            group_guid: None,
            realm_id: 1,
            guid_generator: None,
            legit_characters: Vec::new(),
            pending_packets: Vec::new(),
            player_loading: None,
            connect_to_key: None,
            connect_to_serial: None,
            session_mgr: None,
            instance_address: [127, 0, 0, 1],
            instance_port: 8086,
            instance_link_rx: None,
            time_sync_next_counter: 0,
            time_sync_timer_ms: 0,
            logout_time: None,
            login_time: None,
            total_played_time: 0,
            level_played_time: 0,
            player_gold: 0,
            player_xp: 0,
            player_next_level_xp: 400,
            player_xp_table: None,
            selection_guid: None,
            player_guid: None,
            pending_creature_spawn: None,
            respawn_queue: Vec::new(),
            inventory_items: HashMap::new(),
            buyback_items: HashMap::new(),
            buyback_price: [0; BUYBACK_SLOT_COUNT],
            buyback_timestamp: [0; BUYBACK_SLOT_COUNT],
            current_buyback_slot: BUYBACK_SLOT_START,
            player_currencies: HashMap::new(),
            inventory_item_objects: HashMap::new(),
            current_map_id: 0,
            player_race: 0,
            player_class: 0,
            player_level: 0,
            player_gender: 0,
            known_spells: Vec::new(),
            realm_packet_rx: None,
            realm_send_tx: None,
            player_position: None,
            player_name: None,
            creatures: std::collections::HashMap::new(),
            vendor_item_counts: HashMap::new(),
            creature_tick: 0,
            map_manager: None,
            combat_target: None,
            in_combat: false,
            visible_auras: HashMap::new(),
            spell_store: None,
            quest_store: None,
            quest_xp_store: None,
            player_quests: HashMap::new(),
            rewarded_quests: std::collections::HashSet::new(),
            active_spell_cast: None,
            last_spell_cast_time: None,
            last_spell_cast_time_per_spell: HashMap::new(),
            loot_table: std::collections::HashMap::new(),
            active_loot_guid: ObjectGuid::EMPTY,
            visible_creatures: std::collections::HashSet::new(),
            visible_gameobjects: std::collections::HashSet::new(),
            last_visibility_pos: None,
            gossip_options: Vec::new(),
            gossip_source_guid: None,
            active_area_trigger: None,
            pending_teleport: None,
            creature_query_cache: std::collections::HashSet::new(),
        }
    }

    /// Set the character database for this session.
    pub fn set_char_db(&mut self, db: Arc<CharacterDatabase>) {
        self.char_db = Some(db);
    }

    /// Inject the shared map manager. Call once at session creation, before login.
    pub fn set_map_manager(&mut self, mgr: crate::map_manager::SharedMapManager) {
        self.map_manager = Some(mgr);
    }

    /// Set the realm ID for GUID creation.
    pub fn set_realm_id(&mut self, realm_id: u16) {
        self.realm_id = realm_id;
    }

    /// Compute the Virtual Realm Address: `(Region << 24) | (Battlegroup << 16) | RealmId`.
    ///
    /// Region and Battlegroup come from the `realmlist` table in the auth database.
    /// For a typical single-realm setup: Region=1, Battlegroup=1.
    pub(crate) fn virtual_realm_address(&self) -> u32 {
        // TODO: read Region/Battlegroup from realmlist table instead of hardcoding
        let region: u32 = 1;
        let battlegroup: u32 = 1;
        (region << 24) | (battlegroup << 16) | u32::from(self.realm_id)
    }

    /// Set the GUID generator for new characters.
    pub fn set_guid_generator(&mut self, generator: Arc<ObjectGuidGenerator>) {
        self.guid_generator = Some(generator);
    }

    /// Set the login database for this session.
    pub fn set_login_db(&mut self, db: Arc<LoginDatabase>) {
        self.login_db = Some(db);
    }

    /// Get the character database reference.
    pub fn char_db(&self) -> Option<&Arc<CharacterDatabase>> {
        self.char_db.as_ref()
    }

    /// Get the login database reference.
    pub fn login_db(&self) -> Option<&Arc<LoginDatabase>> {
        self.login_db.as_ref()
    }

    /// Set the world database for this session.
    pub fn set_world_db(&mut self, db: Arc<WorldDatabase>) {
        self.world_db = Some(db);
    }

    /// Get the world database reference.
    pub fn world_db(&self) -> Option<&Arc<WorldDatabase>> {
        self.world_db.as_ref()
    }

    /// Set the currency types store for this session.
    pub fn set_currency_types_store(&mut self, store: Arc<CurrencyTypesStore>) {
        self.currency_types_store = Some(store);
    }

    /// Get the currency types store reference.
    pub fn currency_types_store(&self) -> Option<&Arc<CurrencyTypesStore>> {
        self.currency_types_store.as_ref()
    }

    /// Set the item extended cost store for this session.
    pub fn set_item_extended_cost_store(&mut self, store: Arc<ItemExtendedCostStore>) {
        self.item_extended_cost_store = Some(store);
    }

    /// Get the item extended cost store reference.
    pub fn item_extended_cost_store(&self) -> Option<&Arc<ItemExtendedCostStore>> {
        self.item_extended_cost_store.as_ref()
    }

    /// Set the item store for this session.
    pub fn set_item_store(&mut self, store: Arc<ItemStore>) {
        self.item_store = Some(store);
    }

    /// Get the item store reference.
    pub fn item_store(&self) -> Option<&Arc<ItemStore>> {
        self.item_store.as_ref()
    }

    /// C++ `Player::GetCurrencyQuantity`.
    pub(crate) fn player_currency_quantity(&self, currency_id: u32) -> u32 {
        self.player_currencies
            .get(&currency_id)
            .map(|currency| currency.quantity)
            .unwrap_or(0)
    }

    /// C++ `Player::HasCurrency`.
    pub(crate) fn has_currency(&self, currency_id: u32, amount: u32) -> bool {
        self.player_currency_quantity(currency_id) >= amount
    }

    /// C++ `Player::AddCurrency(..., CurrencyGainSource::Vendor)` without aura gain bonuses.
    pub(crate) fn add_currency_vendor(
        &mut self,
        currency_id: u32,
        amount: u32,
    ) -> Result<Option<PlayerCurrencyDelta>, ()> {
        if amount == 0 {
            return Ok(None);
        }

        let Some(entry) = self
            .currency_types_store
            .as_ref()
            .and_then(|store| store.get(currency_id))
            .copied()
        else {
            return Err(());
        };

        let player_team = player_team_for_race_cpp(self.player_race);
        if (entry.is_alliance() && player_team != Team::Alliance)
            || (entry.is_horde() && player_team != Team::Horde)
        {
            return Err(());
        }

        if entry.award_condition_id != 0
            || entry.faction_id != 0
            || currency_id == CurrencyTypes::Azerite as u32
        {
            return Err(());
        }

        let currency = self
            .player_currencies
            .entry(currency_id)
            .or_insert(PlayerCurrency {
                state: PlayerCurrencyState::New,
                quantity: 0,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 0,
                flags: 0,
            });

        let weekly_cap = entry.max_earnable_per_week;
        let mut applied = amount;
        if weekly_cap != 0 && currency.weekly_quantity.saturating_add(applied) > weekly_cap {
            applied = weekly_cap.saturating_sub(currency.weekly_quantity);
        }

        let max_quantity = currency_max_quantity_cpp(&entry, currency);
        if max_quantity != 0 && currency.quantity.saturating_add(applied) > max_quantity {
            applied = max_quantity.saturating_sub(currency.quantity);
        }

        if applied == 0 {
            return Ok(None);
        }

        if currency.state != PlayerCurrencyState::New {
            currency.state = PlayerCurrencyState::Changed;
        }
        currency.quantity = currency.quantity.saturating_add(applied);
        if weekly_cap != 0 {
            currency.weekly_quantity = currency.weekly_quantity.saturating_add(applied);
        }
        if entry.is_tracking_quantity() {
            currency.tracked_quantity = currency.tracked_quantity.saturating_add(applied);
        }
        if entry.has_total_earned() {
            currency.earned_quantity = currency.earned_quantity.saturating_add(applied);
        }

        let scaler = entry.scaler().max(1) as u32;
        Ok(Some(PlayerCurrencyDelta {
            currency_id,
            quantity: currency.quantity,
            amount: applied,
            weekly_quantity: ((currency.weekly_quantity / scaler) > 0)
                .then_some(currency.weekly_quantity),
            max_quantity: (max_quantity != 0).then_some(max_quantity),
            total_earned: entry.has_total_earned().then_some(currency.earned_quantity),
            suppress_chat_log: entry.is_suppressing_chat_log(false),
        }))
    }

    /// C++ `Player::AddCurrency(..., CurrencyGainSource::ItemRefund)`.
    pub(crate) fn add_currency_item_refund(
        &mut self,
        currency_id: u32,
        amount: u32,
    ) -> Result<Option<PlayerCurrencyDelta>, ()> {
        if amount == 0 {
            return Ok(None);
        }

        let Some(entry) = self
            .currency_types_store
            .as_ref()
            .and_then(|store| store.get(currency_id))
            .copied()
        else {
            return Err(());
        };

        let player_team = player_team_for_race_cpp(self.player_race);
        if (entry.is_alliance() && player_team != Team::Alliance)
            || (entry.is_horde() && player_team != Team::Horde)
        {
            return Ok(None);
        }

        if entry.award_condition_id != 0 {
            return Err(());
        }
        if entry.faction_id != 0 || currency_id == CurrencyTypes::Azerite as u32 {
            return Ok(None);
        }

        let currency = self
            .player_currencies
            .entry(currency_id)
            .or_insert(PlayerCurrency {
                state: PlayerCurrencyState::New,
                quantity: 0,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 0,
                flags: 0,
            });

        if currency.state != PlayerCurrencyState::New {
            currency.state = PlayerCurrencyState::Changed;
        }
        currency.quantity = currency.quantity.saturating_add(amount);

        let scaler = entry.scaler().max(1) as u32;
        let max_quantity = currency_max_quantity_cpp(&entry, currency);
        Ok(Some(PlayerCurrencyDelta {
            currency_id,
            quantity: currency.quantity,
            amount,
            weekly_quantity: ((currency.weekly_quantity / scaler) > 0)
                .then_some(currency.weekly_quantity),
            max_quantity: (max_quantity != 0).then_some(max_quantity),
            total_earned: entry.has_total_earned().then_some(currency.earned_quantity),
            suppress_chat_log: entry.is_suppressing_chat_log(false),
        }))
    }

    /// C++ `Player::RemoveCurrency` underflow guard for vendor costs.
    pub(crate) fn remove_currency(&mut self, currency_id: u32, amount: u32) -> bool {
        if amount == 0 {
            return true;
        }

        let Some(currency) = self.player_currencies.get_mut(&currency_id) else {
            return false;
        };
        if currency.quantity == 0 {
            return false;
        }

        let removed = amount.min(currency.quantity);
        currency.quantity -= removed;
        if currency.state != PlayerCurrencyState::New {
            currency.state = PlayerCurrencyState::Changed;
        }
        true
    }

    /// C++ `Player::_SaveCurrency` for changed/new currency rows.
    pub(crate) fn append_player_currency_save_statements(
        &mut self,
        tx: &mut SqlTransaction,
        character_guid: u64,
    ) {
        let Some(store) = self.currency_types_store.as_ref() else {
            return;
        };
        for (&currency_id, currency) in &mut self.player_currencies {
            if !store.has_record(currency_id) {
                continue;
            }
            let Ok(currency_db_id) = u16::try_from(currency_id) else {
                continue;
            };

            match currency.state {
                PlayerCurrencyState::New => {
                    let mut stmt =
                        PreparedStatement::new(CharStatements::REP_PLAYER_CURRENCY.sql());
                    stmt.set_u64(0, character_guid);
                    stmt.set_u16(1, currency_db_id);
                    stmt.set_u32(2, currency.quantity);
                    stmt.set_u32(3, currency.weekly_quantity);
                    stmt.set_u32(4, currency.tracked_quantity);
                    stmt.set_u32(5, currency.increased_cap_quantity);
                    stmt.set_u32(6, currency.earned_quantity);
                    stmt.set_u8(7, currency.flags);
                    tx.append(stmt);
                    currency.state = PlayerCurrencyState::Unchanged;
                }
                PlayerCurrencyState::Changed => {
                    let mut stmt =
                        PreparedStatement::new(CharStatements::UPD_PLAYER_CURRENCY.sql());
                    stmt.set_u32(0, currency.quantity);
                    stmt.set_u32(1, currency.weekly_quantity);
                    stmt.set_u32(2, currency.tracked_quantity);
                    stmt.set_u32(3, currency.increased_cap_quantity);
                    stmt.set_u32(4, currency.earned_quantity);
                    stmt.set_u8(5, currency.flags);
                    stmt.set_u64(6, character_guid);
                    stmt.set_u16(7, currency_db_id);
                    tx.append(stmt);
                    currency.state = PlayerCurrencyState::Unchanged;
                }
                PlayerCurrencyState::Unchanged | PlayerCurrencyState::Removed => {}
            }
        }
    }

    /// Set the item appearance store for this session.
    pub fn set_item_appearance_store(&mut self, store: Arc<ItemAppearanceStore>) {
        self.item_appearance_store = Some(store);
    }

    /// Get the item appearance store reference.
    pub fn item_appearance_store(&self) -> Option<&Arc<ItemAppearanceStore>> {
        self.item_appearance_store.as_ref()
    }

    /// Set the item modified appearance store for this session.
    pub fn set_item_modified_appearance_store(
        &mut self,
        store: Arc<ItemModifiedAppearanceStore>,
    ) {
        self.item_modified_appearance_store = Some(store);
    }

    /// Get the item modified appearance store reference.
    pub fn item_modified_appearance_store(&self) -> Option<&Arc<ItemModifiedAppearanceStore>> {
        self.item_modified_appearance_store.as_ref()
    }

    /// Build the closure result expected by `Item::visible_entry` and
    /// `Item::visible_appearance_mod_id` from `ItemModifiedAppearance.db2`.
    pub fn item_modified_appearance_ref(&self, id: u32) -> Option<(u32, u16)> {
        self.item_modified_appearance_store
            .as_ref()
            .and_then(|store| store.get(id))
            .and_then(|entry| {
                Some((
                    u32::try_from(entry.item_id).ok()?,
                    u16::try_from(entry.item_appearance_modifier_id).ok()?,
                ))
            })
    }

    /// C++ `DB2Manager::GetItemModifiedAppearance`.
    pub fn item_modified_appearance_for_item(
        &self,
        item_id: u32,
        appearance_mod_id: u32,
    ) -> Option<u32> {
        self.item_modified_appearance_store
            .as_ref()
            .and_then(|store| store.get_for_item(item_id, appearance_mod_id))
            .map(|entry| entry.id)
    }

    /// C++ `DB2Manager::GetItemDisplayId`.
    pub fn item_display_id(&self, item_id: u32, appearance_mod_id: u32) -> Option<u32> {
        let modified = self
            .item_modified_appearance_store
            .as_ref()
            .and_then(|store| store.get_for_item(item_id, appearance_mod_id))?;
        let appearance_id = u32::try_from(modified.item_appearance_id).ok()?;
        self.item_appearance_store
            .as_ref()
            .and_then(|store| store.item_display_info_id(appearance_id))
    }

    /// Set the player stats store for this session.
    pub fn set_player_stats(&mut self, store: Arc<PlayerStatsStore>) {
        self.player_stats = Some(store);
    }

    /// Get the player stats store reference.
    pub fn player_stats(&self) -> Option<&Arc<PlayerStatsStore>> {
        self.player_stats.as_ref()
    }

    /// Set the item stats store for this session.
    pub fn set_item_stats_store(&mut self, store: Arc<ItemStatsStore>) {
        self.item_stats_store = Some(store);
    }

    /// Get the item stats store reference.
    pub fn item_stats_store(&self) -> Option<&Arc<ItemStatsStore>> {
        self.item_stats_store.as_ref()
    }

    /// Resolve C++ `ItemTemplate::ExtendedData->Flags[0]`.
    pub fn item_template_flags(&self, item_id: u32) -> Option<ItemFlags> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.item_flags(item_id))
    }

    /// C++ `Item::IsBoundAccountWide` template-flag predicate.
    pub fn is_item_bound_account_wide(&self, item_id: u32) -> bool {
        self.item_template_flags(item_id)
            .is_some_and(|flags| flags.contains(ItemFlags::IS_BOUND_TO_ACCOUNT))
    }

    /// Resolve C++ `ItemTemplate::GetLockID()` (`ItemSparseEntry::LockID`).
    pub fn item_template_lock_id(&self, item_id: u32) -> Option<u16> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.sparse_template(item_id))
            .map(|template| template.lock_id)
    }

    /// Resolve the C++ `ItemTemplate` subset used by storage validation.
    pub fn item_storage_template(&self, item_id: u32) -> Option<ItemStorageTemplate> {
        let basic = self.item_store.as_ref()?.get(item_id)?;
        let sparse = self.item_stats_store.as_ref()?.sparse_template(item_id)?;
        let class_id = <ItemClass as num_traits::FromPrimitive>::from_u8(basic.class_id)?;
        let inventory_type =
            <InventoryType as num_traits::FromPrimitive>::from_i8(sparse.inventory_type)?;
        let bonding = <ItemBondingType as num_traits::FromPrimitive>::from_u8(sparse.bonding)?;

        Some(ItemStorageTemplate {
            entry: item_id,
            class_id,
            subclass_id: u32::from(basic.subclass_id),
            inventory_type,
            bonding,
            bag_family: BagFamilyMask::from_bits_retain(sparse.bag_family),
            max_stack_size: sparse.max_stack_size(),
            max_count: sparse.max_count,
            item_limit_category: u32::from(sparse.limit_category),
            container_slots: sparse.container_slots,
            sell_price: sparse.sell_price,
            is_crafting_reagent: (sparse.flags[1] & ItemFlags2::UsedInATradeskill as u32) != 0,
            flags: sparse.item_flags(),
        })
    }

    /// Resolve C++ `ItemTemplate::GetInventoryType()` for equipment-slot mapping.
    pub fn item_template_inventory_type(&self, item_id: u32) -> Option<u8> {
        self.item_storage_template(item_id)
            .map(|template| template.inventory_type as u8)
            .filter(|&inventory_type| inventory_type != InventoryType::NonEquip as u8)
    }

    pub fn item_template_max_durability(&self, item_id: u32) -> u32 {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.sparse_template(item_id))
            .map(|template| template.max_durability)
            .unwrap_or(0)
    }

    pub(crate) fn make_inventory_item_object(
        &self,
        item_guid: ObjectGuid,
        entry_id: u32,
        owner_guid: ObjectGuid,
        count: u32,
        durability: u32,
        context: ItemContext,
        slot: u8,
    ) -> Item {
        let max_durability = self.item_template_max_durability(entry_id).max(durability);
        let mut item = Item::new(i64::from(self.total_played_time));
        item.initialize_created_state(ItemCreateInfo {
            guid: item_guid,
            item_id: entry_id,
            context,
            owner: Some(owner_guid),
            max_durability,
            expiration: 0,
            spell_charges: [0; MAX_ITEM_SPELLS],
        });
        item.set_count(count.max(1));
        item.set_durability(durability);
        item.set_slot(slot);
        item.set_container_guid(ObjectGuid::EMPTY);
        item
    }

    pub(crate) fn insert_inventory_item_object(&mut self, item: Item) -> Option<Item> {
        self.inventory_item_objects.insert(item.object().guid(), item)
    }

    pub(crate) fn remove_inventory_item_object(&mut self, item_guid: ObjectGuid) -> Option<Item> {
        self.inventory_item_objects.remove(&item_guid)
    }

    pub(crate) fn set_inventory_item_object_slot(&mut self, item_guid: ObjectGuid, slot: u8) {
        if let Some(item) = self.inventory_item_objects.get_mut(&item_guid) {
            item.set_slot(slot);
        }
    }

    pub(crate) fn is_buyback_slot(slot: u8) -> bool {
        (BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&slot)
    }

    /// Remove a fully-looted runtime item after its DB rows were deleted.
    pub(crate) fn remove_fully_looted_runtime_item(
        &mut self,
        bag: u8,
        slot: u8,
        item_guid: ObjectGuid,
    ) {
        if bag == INVENTORY_SLOT_BAG_0
            && self
                .inventory_items
                .get(&slot)
                .is_some_and(|item| item.guid == item_guid)
        {
            self.inventory_items.remove(&slot);
        }
        self.remove_inventory_item_object(item_guid);
        self.sync_object_accessor_player();
    }

    /// Resolve an inventory item by (bag, slot) following C++ Player::GetItemByPos.
    ///
    /// - `bag == INVENTORY_SLOT_BAG_0`       → top-level direct inventory (buyback excluded).
    /// - `bag` in carried/bank/reagent range → search nested runtime items inside the bag.
    pub(crate) fn get_inventory_item_by_pos(&self, bag: u8, slot: u8) -> Option<InventoryItem> {
        if bag == INVENTORY_SLOT_BAG_0 {
            if (slot as usize) >= PLAYER_SLOT_END || Self::is_buyback_slot(slot) {
                return None;
            }
            self.inventory_items.get(&slot).cloned()
        } else if is_represented_bag_slot(bag) {
            let bag_item = self.inventory_items.get(&bag)?;
            let bag_guid = bag_item.guid;
            let nested = self
                .inventory_item_objects
                .values()
                .find(|item| item.container_guid() == bag_guid && item.slot() == slot)?;
            let guid = nested.object().guid();
            let entry_id = nested.object().entry();
            Some(InventoryItem {
                guid,
                entry_id,
                db_guid: guid.counter() as u64,
                inventory_type: self.item_template_inventory_type(entry_id),
            })
        } else {
            None
        }
    }

    pub(crate) fn select_buyback_slot_cpp(&self) -> u8 {
        let mut slot = self.current_buyback_slot;
        if self.buyback_items.contains_key(&slot) {
            let mut oldest_slot = BUYBACK_SLOT_START;
            let mut oldest_time = self.buyback_timestamp[0];

            for candidate in BUYBACK_SLOT_START + 1..BUYBACK_SLOT_END {
                let candidate_index = (candidate - BUYBACK_SLOT_START) as usize;
                if !self.buyback_items.contains_key(&candidate) {
                    oldest_slot = candidate;
                    break;
                }
                let candidate_time = self.buyback_timestamp[candidate_index];
                if oldest_time > candidate_time {
                    oldest_time = candidate_time;
                    oldest_slot = candidate;
                }
            }

            slot = oldest_slot;
        }

        slot
    }

    pub(crate) fn advance_buyback_slot_cpp(&mut self) {
        if self.current_buyback_slot < BUYBACK_SLOT_END - 1 {
            self.current_buyback_slot += 1;
        }
    }

    fn direct_inventory_player_snapshot(&self) -> Option<Player> {
        let player_guid = self.player_guid?;
        let mut player = Player::new(None, false);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);

        for (&slot, item) in &self.inventory_items {
            if (slot as usize) < PLAYER_SLOT_END && !Self::is_buyback_slot(slot) {
                let _ = player.store_top_level_item(slot, item.guid);
            }
        }

        Some(player)
    }

    pub(crate) fn can_destroy_direct_item_like_cpp(
        &self,
        slot: u8,
        source_item: Option<&Item>,
        proto: Option<&ItemStorageTemplate>,
        source_is_not_empty_bag: bool,
    ) -> InventoryResult {
        let pos = make_item_pos(INVENTORY_SLOT_BAG_0, slot);
        if !is_equipment_packed_pos(pos) && !is_bag_pos(pos) {
            return InventoryResult::Ok;
        }

        let Some(player) = self.direct_inventory_player_snapshot() else {
            return InventoryResult::Ok;
        };

        player.can_unequip_item(CanUnequipItemArgs {
            pos,
            source_item,
            proto,
            swap: false,
            source_is_not_empty_bag,
            is_charmed: false,
            is_in_combat: self.in_combat,
            is_in_progress_arena: false,
        })
    }

    pub(crate) fn direct_item_contains_items(&self, item_guid: ObjectGuid) -> bool {
        self.inventory_item_objects
            .values()
            .any(|item| item.container_guid() == item_guid)
    }

    pub(crate) fn set_active_loot_guid(&mut self, guid: ObjectGuid) {
        self.active_loot_guid = guid;
    }

    pub(crate) fn clear_active_loot_guid_if(&mut self, guid: ObjectGuid) {
        if self.active_loot_guid == guid {
            self.active_loot_guid = ObjectGuid::EMPTY;
        }
    }

    pub(crate) fn is_active_loot_guid(&self, guid: ObjectGuid) -> bool {
        !guid.is_empty() && self.active_loot_guid == guid
    }

    pub fn plan_store_new_direct_inventory_item(
        &self,
        entry_id: u32,
        count: u32,
    ) -> Option<(InventoryResult, Vec<ItemPosCount>, Option<u32>)> {
        self.plan_store_new_direct_inventory_item_at(entry_id, count, NULL_BAG, NULL_SLOT)
    }

    pub fn plan_store_new_direct_inventory_item_at(
        &self,
        entry_id: u32,
        count: u32,
        bag: u8,
        slot: u8,
    ) -> Option<(InventoryResult, Vec<ItemPosCount>, Option<u32>)> {
        let player = self.direct_inventory_player_snapshot()?;
        let proto = self.item_storage_template(entry_id);
        let mut template_cache = HashMap::new();
        for (&slot, item) in &self.inventory_items {
            if Self::is_buyback_slot(slot) {
                continue;
            }
            if let Some(template) = self.item_storage_template(item.entry_id) {
                template_cache.insert(item.entry_id, template);
            }
        }

        let mut slot_items = Vec::new();
        let mut stored_items = Vec::new();
        for (&slot, inventory_item) in &self.inventory_items {
            if Self::is_buyback_slot(slot) {
                continue;
            }
            let Some(item) = self.inventory_item_objects.get(&inventory_item.guid) else {
                continue;
            };
            slot_items.push(ItemSlotRef::new(INVENTORY_SLOT_BAG_0, slot, item));
            stored_items.push(ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                slot,
                item,
                template_cache.get(&inventory_item.entry_id),
            ));
        }

        let mut dest = Vec::new();
        let outcome = player.can_store_item(&mut dest, CanStoreItemArgs {
            bag,
            slot,
            entry: entry_id,
            count,
            proto: proto.as_ref(),
            source_item: None,
            source_is_not_empty_bag: false,
            source_bop_trade_allowed_for_player: false,
            swap: false,
            limit_category: None,
            slot_items: &slot_items,
            stored_items: &stored_items,
            bag_templates: &[],
        });

        Some((outcome.result, dest, outcome.no_space_count))
    }

    /// Set the item random suffix store for this session.
    pub fn set_item_random_suffix_store(&mut self, store: Arc<ItemRandomSuffixStore>) {
        self.item_random_suffix_store = Some(store);
    }

    /// Get the item random suffix store reference.
    pub fn item_random_suffix_store(&self) -> Option<&Arc<ItemRandomSuffixStore>> {
        self.item_random_suffix_store.as_ref()
    }

    /// Resolve C++ `sItemRandomSuffixStore.LookupEntry(abs(RandomPropertiesID))`.
    pub fn apply_enchantment_random_suffix_ref(
        &self,
        random_properties_id: i32,
    ) -> Option<ApplyEnchantmentRandomSuffixRef> {
        let id = random_properties_id.unsigned_abs();
        if id == 0 {
            return None;
        }

        self.item_random_suffix_store
            .as_ref()
            .and_then(|store| store.get(id))
            .map(|entry| {
                ApplyEnchantmentRandomSuffixRef::new(
                    entry.id,
                    entry.enchantments,
                    entry.allocation_pct,
                )
            })
    }

    /// Set the spell item enchantment store for this session.
    pub fn set_spell_item_enchantment_store(&mut self, store: Arc<SpellItemEnchantmentStore>) {
        self.spell_item_enchantment_store = Some(store);
    }

    /// Get the spell item enchantment store reference.
    pub fn spell_item_enchantment_store(&self) -> Option<&Arc<SpellItemEnchantmentStore>> {
        self.spell_item_enchantment_store.as_ref()
    }

    /// C++ `SpellMgr::IsArenaAllowedEnchancment`.
    pub fn is_arena_allowed_enchantment(&self, enchantment_id: u32) -> bool {
        self.spell_item_enchantment_store
            .as_ref()
            .is_some_and(|store| store.is_arena_allowed_enchantment(enchantment_id))
    }

    /// Build the entity-level `ApplyEnchantment` template from `SpellItemEnchantment.db2`.
    pub fn apply_enchantment_template_ref(
        &self,
        enchantment_id: i32,
        required_skill_value: u16,
        condition_fits: bool,
    ) -> Option<ApplyEnchantmentTemplateRef> {
        let id = u32::try_from(enchantment_id).ok()?;
        self.spell_item_enchantment_store
            .as_ref()
            .and_then(|store| store.get(id))
            .map(|entry| {
                let mut template = ApplyEnchantmentTemplateRef::new(enchantment_id);
                template.condition_id = u32::from(entry.condition_id);
                template.condition_fits = condition_fits;
                template.min_level = entry.min_level;
                template.required_skill_id = u32::from(entry.required_skill_id);
                template.required_skill_rank = entry.required_skill_rank;
                template.required_skill_value = required_skill_value;
                template
            })
    }

    /// Build the C++ three `SpellItemEnchantmentEntry` effect refs.
    pub fn apply_enchantment_effect_refs(
        &self,
        enchantment_id: u32,
    ) -> Option<[ApplyEnchantmentEffectRef; 3]> {
        self.spell_item_enchantment_store
            .as_ref()
            .and_then(|store| store.get(enchantment_id))
            .map(|entry| {
                std::array::from_fn(|index| {
                    let amount = entry.effect_points_min[index] as u32;
                    let arg = entry.effect_arg[index];
                    match <ItemEnchantmentType as num_traits::FromPrimitive>::from_u8(entry.effect[index]) {
                        Some(effect_type) => ApplyEnchantmentEffectRef::known(effect_type, amount, arg),
                        None => ApplyEnchantmentEffectRef::unknown(u32::from(entry.effect[index]), amount, arg),
                    }
                })
            })
    }

    /// Set the hotfix blob cache for this session.
    pub fn set_hotfix_blob_cache(&mut self, cache: Arc<HotfixBlobCache>) {
        self.hotfix_blob_cache = Some(cache);
    }

    /// Get the hotfix blob cache reference.
    pub fn hotfix_blob_cache(&self) -> Option<&Arc<HotfixBlobCache>> {
        self.hotfix_blob_cache.as_ref()
    }

    /// Set the area trigger store for this session.
    pub fn set_area_trigger_store(&mut self, store: Arc<AreaTriggerStore>) {
        self.area_trigger_store = Some(store);
    }

    /// Get the area trigger store reference.
    pub fn area_trigger_store(&self) -> Option<&Arc<AreaTriggerStore>> {
        self.area_trigger_store.as_ref()
    }

    /// Set the skill store for this session.
    pub fn set_skill_store(&mut self, store: Arc<SkillStore>) {
        self.skill_store = Some(store);
    }

    /// Get the skill store reference.
    pub fn skill_store(&self) -> Option<&Arc<SkillStore>> {
        self.skill_store.as_ref()
    }

    /// Set the spell store for this session.
    pub fn set_spell_store(&mut self, store: Arc<SpellStore>) {
        self.spell_store = Some(store);
    }

    /// Get the spell store reference.
    pub fn spell_store(&self) -> Option<&Arc<SpellStore>> {
        self.spell_store.as_ref()
    }

    /// Set the quest store shared reference.
    pub fn set_quest_store(&mut self, store: Arc<wow_data::quest::QuestStore>) {
        self.quest_store = Some(store);
    }

    /// Save current player gold to the characters DB.
    pub(crate) async fn save_player_gold(&self) {
        use wow_database::CharStatements;
        let guid = match self.player_guid { Some(g) => g.counter() as u32, None => return };
        let char_db = match self.char_db() { Some(db) => Arc::clone(db), None => return };
        let mut stmt = char_db.prepare(CharStatements::UPD_CHAR_MONEY);
        stmt.set_u64(0, self.player_gold);
        stmt.set_u32(1, guid);
        let _ = char_db.execute(&stmt).await;
    }

    /// Give XP to the player, leveling up if threshold reached.
    /// C# ref: Player.GiveXP(xp, victim)
    pub(crate) async fn give_xp(&mut self, xp: u32, victim: wow_core::ObjectGuid, is_kill: bool) {
        use wow_packet::packets::misc::{LogXpGain, LevelUpInfo};
        use wow_packet::ServerPacket;

        if xp == 0 { return; }
        if self.player_level >= 80 { return; } // max level

        // Send floating XP text — C# LogXPGain
        self.send_packet(&LogXpGain {
            victim,
            original: xp as i32,
            reason: if is_kill { 0 } else { 1 },
            amount: xp as i32,
            group_bonus: 1.0,
        });

        self.player_xp = self.player_xp.saturating_add(xp);

        // Level up loop — C# while (newXP >= nextLvlXP && !IsMaxLevel())
        while self.player_xp >= self.player_next_level_xp && self.player_level < 80 {
            self.player_xp -= self.player_next_level_xp;
            let new_level = self.player_level + 1;

            info!(
                account = self.account_id,
                new_level,
                "Player leveled up"
            );

            // Send SMSG_LEVELUP_INFO — "Ding!" popup
            // Stats deltas are loaded from player_levelstats in real impl;
            // for now send 0 deltas (client will update from UpdateObject).
            self.send_packet(&LevelUpInfo {
                level: new_level as i32,
                health_delta: 0,
                power_delta: [0i32; 10],
                stat_delta: [0i32; 5],
                num_new_talents: 0,
            });

            self.player_level = new_level;
            self.refresh_next_level_xp();

            // Persist new level to DB
            if let Some(guid) = self.player_guid {
                let char_db = self.char_db().map(Arc::clone);
                if let Some(db) = char_db {
                    use wow_database::CharStatements;
                    let mut stmt = db.prepare(CharStatements::UPD_CHAR_LEVEL);
                    stmt.set_u8(0, self.player_level);
                    stmt.set_u32(1, self.player_xp);
                    stmt.set_u32(2, guid.counter() as u32);
                    let _ = db.execute(&stmt).await;
                }
            }
        }

        // Persist current XP
        if let Some(guid) = self.player_guid {
            let char_db = self.char_db().map(Arc::clone);
            if let Some(db) = char_db {
                use wow_database::CharStatements;
                let mut stmt = db.prepare(CharStatements::UPD_CHAR_XP);
                stmt.set_u32(0, self.player_xp);
                stmt.set_u32(1, guid.counter() as u32);
                let _ = db.execute(&stmt).await;
            }
        }
    }

    /// XP reward for killing a creature.
    /// C# ref: Formulas.XPGain / Formulas.BaseGain
    pub(crate) fn creature_kill_xp(&self, mob_level: u8) -> u32 {
        let pl = self.player_level as i32;
        let ml = mob_level as i32;

        // nBaseExp by content level (WotLK = 71-80 content)
        let n_base_exp: i32 = if pl >= 71 { 580 }
                              else if pl >= 61 { 235 }
                              else { 45 };

        // Gray level check
        let gray = self.gray_level(pl as u8) as i32;
        if ml <= gray { return 0; }

        let base_gain = if ml >= pl {
            let diff = (ml - pl).min(4);
            ((pl * 5 + n_base_exp) * (20 + diff) / 10 + 1) / 2
        } else {
            let zd = self.zero_difference(pl as u8) as i32;
            (pl * 5 + n_base_exp) * (zd + ml - pl) / zd
        };

        base_gain.max(0) as u32
    }

    /// Level at which mobs give 0 XP ("gray") — C# Formulas.GetGrayLevel
    fn gray_level(&self, pl: u8) -> u8 {
        let p = pl as i32;
        let g = if p <= 5 { 0 }
                else if p <= 39 { p - 5 - p / 10 }
                else if p <= 59 { p - 1 - p / 5 }
                else { p - 9 };
        g.max(0) as u8
    }

    /// Zero-difference table — C# Formulas.GetZeroDifference
    fn zero_difference(&self, pl: u8) -> u8 {
        match pl {
            0..=3   => 5,
            4..=9   => 6,
            10..=11 => 7,
            12..=15 => 8,
            16..=19 => 9,
            20..=29 => 11,
            30..=39 => 12,
            40..=44 => 13,
            45..=49 => 14,
            50..=54 => 15,
            55..=59 => 16,
            _       => 17,
        }
    }

    /// Called when the player kills a creature. Checks all active kill-objective quests
    /// and updates progress. Sends SMSG_QUEST_UPDATE_ADD_CREDIT if progress was made.
    pub(crate) async fn on_creature_killed(
        &mut self,
        creature_entry: u32,
        creature_guid: wow_core::ObjectGuid,
    ) {
        use wow_packet::packets::quest::{QuestUpdateAddCredit, QuestUpdateComplete};
        use wow_packet::ServerPacket;

        let Some(store) = self.quest_store.clone() else { return };

        // Objective type 0 = Monster/NPC kill
        const OBJ_TYPE_MONSTER: u8 = 0;

        // Collect quest IDs that have a matching kill objective to avoid borrow issues
        let matching: Vec<(u32, usize, i32)> = self.player_quests.values()
            .filter(|qs| qs.status == 1) // only incomplete quests
            .filter_map(|qs| {
                let quest = store.get(qs.quest_id)?;
                for (i, obj) in quest.objectives.iter().enumerate() {
                    if obj.obj_type == OBJ_TYPE_MONSTER && obj.object_id == creature_entry as i32 {
                        let idx = obj.storage_index.max(0) as usize;
                        return Some((qs.quest_id, idx, obj.amount));
                    }
                }
                None
            })
            .collect();

        for (quest_id, obj_idx, required) in matching {
            let Some(qs) = self.player_quests.get_mut(&quest_id) else { continue };
            if qs.objective_counts.len() <= obj_idx {
                qs.objective_counts.resize(obj_idx + 1, 0);
            }
            if qs.objective_counts[obj_idx] >= required {
                continue; // Already done
            }
            qs.objective_counts[obj_idx] += 1;
            let current = qs.objective_counts[obj_idx];

            debug!(
                account = self.account_id,
                quest_id, obj_idx, current, required,
                "Quest kill objective progress"
            );

            // SMSG_QUEST_UPDATE_ADD_CREDIT
            self.send_packet(&QuestUpdateAddCredit {
                victim_guid: creature_guid,
                quest_id,
                object_id: creature_entry as i32,
                count: current as u16,
                required: required as u16,
                objective_type: OBJ_TYPE_MONSTER,
            });

            // Check if quest is now complete (all objectives satisfied)
            let all_done = {
                let quest = store.get(quest_id);
                quest.map_or(false, |q| {
                    let qs = self.player_quests.get(&quest_id).unwrap();
                    q.objectives.iter().enumerate().all(|(i, obj)| {
                        let idx = obj.storage_index.max(0) as usize;
                        let progress = qs.objective_counts.get(idx).copied().unwrap_or(0);
                        progress >= obj.amount
                    })
                })
            };

            if all_done {
                if let Some(qs) = self.player_quests.get_mut(&quest_id) {
                    qs.status = 2; // Complete
                }
                self.send_packet(&QuestUpdateComplete { quest_id });
                info!(account = self.account_id, quest_id, "Quest objectives complete");
            }
        }
    }

    /// Set the QuestXP store (loaded from QuestXP.db2).
    pub fn set_quest_xp_store(&mut self, store: Arc<wow_data::quest_xp::QuestXpStore>) {
        self.quest_xp_store = Some(store);
    }

    /// Set the player XP table (xp required per level).
    pub fn set_player_xp_table(&mut self, table: Arc<Vec<u32>>) {
        self.player_xp_table = Some(table);
        self.refresh_next_level_xp();
    }

    /// Update player_next_level_xp from the table based on current level.
    pub(crate) fn refresh_next_level_xp(&mut self) {
        if let Some(table) = &self.player_xp_table {
            let lvl = self.player_level as usize;
            self.player_next_level_xp = table.get(lvl).copied().unwrap_or(u32::MAX);
        }
    }

    /// Calculate XP reward for a quest.
    /// C# ref: Quest::XPValue(player, questLevel, xpDifficulty, xpMultiplier)
    pub(crate) fn calculate_quest_xp(&self, difficulty: u32, quest_level: i32) -> u32 {
        if let Some(store) = &self.quest_xp_store {
            store.calculate_xp(quest_level, self.player_level, difficulty)
        } else {
            // Fallback if DB2 not loaded
            const XP_TABLE: [u32; 10] = [0, 50, 100, 200, 400, 650, 1000, 1500, 2500, 4000];
            XP_TABLE[difficulty.min(9) as usize]
        }
    }

    /// Check if a spell is on cooldown (global or per-spell).
    ///
    /// Returns true if either the global cooldown (1500ms) or the spell-specific
    /// cooldown from SpellStore is still active.
    pub fn is_spell_on_cooldown(&self, spell_id: i32) -> bool {
        let Some(last_cast) = self.last_spell_cast_time else {
            return false; // Never casted
        };

        let elapsed_ms = last_cast.elapsed().as_millis() as u32;

        // Global cooldown: 1500ms
        if elapsed_ms < 1500 {
            return true;
        }

        // Per-spell cooldown (if exists in SpellStore)
        if let Some(store) = &self.spell_store {
            if let Some(spell_info) = store.get(spell_id) {
                if elapsed_ms < spell_info.cooldown_ms {
                    return true;
                }
            }
        }

        false
    }

    /// Set the shared player registry (used for broadcast).
    pub fn set_player_registry(&mut self, registry: Arc<PlayerRegistry>) {
        self.player_registry = Some(registry);
    }

    /// Set the shared ObjectAccessor used for C++-style object lookup.
    pub fn set_object_accessor(&mut self, accessor: SharedObjectAccessor) {
        self.object_accessor = Some(accessor);
    }

    /// Get a reference to the shared ObjectAccessor.
    pub fn object_accessor(&self) -> Option<&SharedObjectAccessor> {
        self.object_accessor.as_ref()
    }

    /// Get a reference to the shared player registry.
    pub fn player_registry(&self) -> Option<&Arc<PlayerRegistry>> {
        self.player_registry.as_ref()
    }

    /// Set the shared group registry and pending invites.
    pub fn set_group_registry(&mut self, reg: Arc<GroupRegistry>, invites: Arc<PendingInvites>) {
        self.group_registry = Some(reg);
        self.pending_invites = Some(invites);
    }

    /// Get a reference to the shared group registry.
    pub fn group_registry(&self) -> Option<&Arc<GroupRegistry>> {
        self.group_registry.as_ref()
    }

    /// Get a reference to the shared pending invites map.
    pub fn pending_invites(&self) -> Option<&Arc<PendingInvites>> {
        self.pending_invites.as_ref()
    }

    /// Register this session in the player registry.
    /// Called after player login is complete (player_guid + position both set).
    pub(crate) fn register_in_player_registry(&self) {
        use crate::handlers::character::default_display_id;
        let (Some(guid), Some(pos), Some(name), Some(reg)) = (
            self.player_guid,
            self.player_position,
            &self.player_name,
            &self.player_registry,
        ) else {
            return;
        };
        let mut visible_items = [(0i32, 0u16, 0u16); 19];
        for (slot, item) in &self.inventory_items {
            if (*slot as usize) < 19 {
                visible_items[*slot as usize] = (item.entry_id as i32, 0u16, 0u16);
            }
        }
        reg.insert(guid, PlayerBroadcastInfo {
            map_id: self.current_map_id,
            position: pos,
            send_tx: self.send_tx.clone(),
            player_name: name.clone(),
            account_id: self.account_id,
            race: self.player_race,
            class: self.player_class,
            sex: self.player_gender,
            level: self.player_level,
            display_id: default_display_id(self.player_race, self.player_gender),
            visible_items,
        });
        debug!(
            "Registered player {:?} ({}) in broadcast registry (map {})",
            guid, name, self.current_map_id
        );
    }

    fn object_accessor_player_object(&self) -> Option<WorldObject> {
        let (Some(guid), Some(pos), Some(name)) =
            (self.player_guid, self.player_position, &self.player_name)
        else {
            return None;
        };

        let mut object = WorldObject::new(true, TypeId::Player, TypeMask::PLAYER);
        object.object_mut().create(guid);
        object.set_name(name);
        if object.set_map(u32::from(self.current_map_id), 0).is_err() {
            return None;
        }
        object.relocate(pos);
        object.object_mut().add_to_world();
        Some(object)
    }

    fn object_accessor_inventory_snapshot(&self) -> PlayerInventoryStorage {
        let mut inventory = PlayerInventoryStorage::default();
        for (&slot, item) in &self.inventory_items {
            if (slot as usize) < PLAYER_SLOT_END {
                inventory.items[slot as usize] = Some(item.guid);
            }
        }
        inventory
    }

    pub(crate) fn sync_object_accessor_player(&self) {
        let Some(accessor) = &self.object_accessor else {
            return;
        };
        let Some(object) = self.object_accessor_player_object() else {
            return;
        };
        let Some(name) = &self.player_name else {
            return;
        };

        let inventory = self.object_accessor_inventory_snapshot();
        let items = self.inventory_item_objects.values().cloned();
        if let Err(err) =
            accessor
                .write()
                .add_player_with_inventory_and_items(name, object, inventory, items)
        {
            warn!("Failed to sync player into ObjectAccessor: {err:?}");
        }
    }

    pub(crate) fn unregister_from_object_accessor(&self) {
        let (Some(guid), Some(accessor)) = (self.player_guid, &self.object_accessor) else {
            return;
        };
        accessor.write().remove_player(guid);
    }

    pub fn cleanup_shared_runtime_state(&mut self) {
        self.unregister_from_player_registry();
        self.unregister_from_object_accessor();
        self.inventory_items.clear();
        self.inventory_item_objects.clear();
    }

    /// Remove this session from the player registry.
    /// Called on logout or disconnect.
    pub(crate) fn unregister_from_player_registry(&self) {
        let (Some(guid), Some(reg)) = (self.player_guid, &self.player_registry) else {
            return;
        };
        reg.remove(&guid);
        debug!("Unregistered player {:?} from broadcast registry", guid);
    }

    /// Update this session's position (and map) in the player registry.
    /// Called whenever `player_position` changes.
    pub(crate) fn update_registry_position(&self) {
        let (Some(guid), Some(pos), Some(reg)) = (
            self.player_guid,
            self.player_position,
            &self.player_registry,
        ) else {
            return;
        };
        if let Some(mut entry) = reg.get_mut(&guid) {
            entry.position = pos;
            entry.map_id = self.current_map_id;
        }
        if let Some(accessor) = &self.object_accessor {
            if let Some(object) = accessor.write().player_object_mut(guid) {
                object.world_relocate(u32::from(self.current_map_id), pos);
            }
        }
    }

    /// Get the realm ID.
    pub fn realm_id(&self) -> u16 {
        self.realm_id
    }

    /// Get the GUID generator.
    pub fn guid_generator(&self) -> Option<&Arc<ObjectGuidGenerator>> {
        self.guid_generator.as_ref()
    }

    /// Set the session manager for ConnectTo flow.
    pub fn set_session_mgr(&mut self, mgr: Arc<SessionManager>) {
        self.session_mgr = Some(mgr);
    }

    /// Set the instance server address and port.
    pub fn set_instance_endpoint(&mut self, addr: [u8; 4], port: u16) {
        self.instance_address = addr;
        self.instance_port = port;
    }

    /// Get the session manager reference.
    pub fn session_mgr(&self) -> Option<&Arc<SessionManager>> {
        self.session_mgr.as_ref()
    }

    /// Get the instance server address.
    pub fn instance_address(&self) -> [u8; 4] {
        self.instance_address
    }

    /// Get the instance server port.
    pub fn instance_port(&self) -> u16 {
        self.instance_port
    }

    /// Set the player loading GUID (ConnectTo flow).
    pub fn set_player_loading(&mut self, guid: Option<ObjectGuid>) {
        self.player_loading = guid;
    }

    /// Get the player loading GUID.
    pub fn player_loading(&self) -> Option<ObjectGuid> {
        self.player_loading
    }

    /// Set the ConnectTo key.
    pub fn set_connect_to_key(&mut self, key: Option<i64>) {
        self.connect_to_key = key;
    }

    /// Set the ConnectTo serial.
    pub fn set_connect_to_serial(&mut self, serial: Option<wow_packet::packets::auth::ConnectToSerial>) {
        self.connect_to_serial = serial;
    }

    /// Set the instance link receiver.
    pub fn set_instance_link_rx(&mut self, rx: Option<tokio::sync::oneshot::Receiver<InstanceLink>>) {
        self.instance_link_rx = rx;
    }

    /// Get a clone of the send channel.
    pub fn send_tx(&self) -> &flume::Sender<Vec<u8>> {
        &self.send_tx
    }

    /// Set the list of legitimate characters for this account.
    pub fn set_legit_characters(&mut self, guids: Vec<ObjectGuid>) {
        self.legit_characters = guids;
    }

    /// Check if a GUID is in the legit characters list.
    pub fn is_legit_character(&self, guid: &ObjectGuid) -> bool {
        self.legit_characters.contains(guid)
    }

    /// Remove a GUID from the legit characters list.
    pub fn remove_legit_character(&mut self, guid: &ObjectGuid) {
        self.legit_characters.retain(|g| g != guid);
    }

    /// Process queued packets (up to [`MAX_PACKETS_PER_UPDATE`] per call).
    ///
    /// Returns the number of packets processed.
    pub fn update(&mut self, diff_ms: u32) -> usize {
        let mut processed = 0;

        // Drain the primary (instance) packet channel
        while processed < MAX_PACKETS_PER_UPDATE {

            let pkt = match self.packet_rx.try_recv() {
                Ok(p) => p,
                Err(flume::TryRecvError::Empty) => break,
                Err(flume::TryRecvError::Disconnected) => {
                    debug!("Packet channel disconnected for account {}", self.account_id);
                    self.state = SessionState::Disconnecting;
                    break;
                }
            };

            self.last_packet_time = Instant::now();
            self.pending_packets.push(pkt);
            processed += 1;
        }

        // Also drain the realm socket channel (after ConnectTo, realm-type
        // packets like BattlenetRequest, Ping, etc. arrive here)
        if let Some(ref realm_rx) = self.realm_packet_rx {
            while processed < MAX_PACKETS_PER_UPDATE {
                match realm_rx.try_recv() {
                    Ok(pkt) => {
                        self.last_packet_time = Instant::now();
                        self.pending_packets.push(pkt);
                        processed += 1;
                    }
                    Err(flume::TryRecvError::Empty) => break,
                    Err(flume::TryRecvError::Disconnected) => {
                        info!(
                            "Realm socket disconnected for account {} (instance still active)",
                            self.account_id
                        );
                        // Realm dropped — don't disconnect immediately, the
                        // instance socket may still be fine.
                        self.realm_packet_rx = None;
                        break;
                    }
                }
            }
        }

        // ── Creature AI tick ─────────────────────────────────────────
        // Throttle to every 4 ticks (~200ms at 50ms tick).
        if self.state == SessionState::LoggedIn {
            self.creature_tick = self.creature_tick.wrapping_add(1);
            if self.creature_tick % 4 == 0 {
                self.tick_creatures_sync();
            }
            // Combat tick every 2 ticks (~100ms)
            if self.creature_tick % 2 == 0 {
                self.tick_combat_sync();
            }
            // Aura expiry tick every 4 ticks (~200ms)
            if self.creature_tick % 4 == 0 {
                self.tick_auras();
            }
        }

        // ── Periodic TimeSyncRequest ──────────────────────────────
        // C# sends first resync 5s after login, then every 10s.
        // The client MUST receive periodic TimeSyncRequests or its
        // internal clock sync state becomes inconsistent → crash.
        if self.state == SessionState::LoggedIn && self.time_sync_timer_ms > 0 {
            if diff_ms >= self.time_sync_timer_ms {
                self.send_time_sync();
            } else {
                self.time_sync_timer_ms -= diff_ms;
            }
        }

        // ── Logout timer ────────────────────────────────────────────
        if let Some(logout_time) = self.logout_time {
            if Instant::now() >= logout_time {
                self.logout_time = None;
                self.complete_logout();
            }
        }

        processed
    }

    // ── Aura system ───────────────────────────────────────────────

    /// Apply an aura to the player and send SMSG_AURA_UPDATE.
    pub fn apply_aura(
        &mut self,
        spell_id: i32,
        caster_guid: ObjectGuid,
        duration_ms: u32,
        aura_flags: u32,
    ) -> Result<(), &'static str> {

        // Find a free slot (0-254)
        let mut slot = 0u8;
        while self.visible_auras.contains_key(&slot) && slot < 255 {
            slot += 1;
        }

        if slot >= 255 {
            return Err("No free aura slots");
        }

        // Create aura
        let aura = AuraApplication {
            spell_id,
            caster_guid,
            slot,
            duration_total: duration_ms,
            duration_remaining: duration_ms,
            stack_count: 1,
            aura_flags,
            applied_at: Instant::now(),
        };

        self.visible_auras.insert(slot, aura);

        // Send SMSG_AURA_UPDATE
        self.send_aura_update_applied(spell_id, slot, caster_guid, duration_ms, aura_flags);

        Ok(())
    }

    /// Remove an aura by slot and send SMSG_AURA_UPDATE.
    pub fn remove_aura(&mut self, slot: u8) -> Result<(), &'static str> {

        if self.visible_auras.remove(&slot).is_none() {
            return Err("Aura slot not found");
        }

        // Send SMSG_AURA_UPDATE (removal)
        self.send_aura_update_removed(slot);

        Ok(())
    }

    /// Check all active auras for expiry and remove those whose duration has elapsed.
    /// Called from the synchronous tick loop (~every 200ms via creature_tick).
    pub(crate) fn tick_auras(&mut self) {
        if self.visible_auras.is_empty() {
            return;
        }

        // Collect expired slots (avoid borrow conflict)
        let expired: Vec<u8> = self
            .visible_auras
            .values()
            .filter(|a| {
                // Permanent auras (duration_total == 0) never expire
                a.duration_total > 0
                    && a.applied_at.elapsed().as_millis() as u32 >= a.duration_total
            })
            .map(|a| a.slot)
            .collect();

        for slot in expired {
            let spell_id = self.visible_auras.get(&slot).map(|a| a.spell_id).unwrap_or(0);
            let _ = self.remove_aura(slot);
            debug!(
                account = self.account_id,
                slot = slot,
                spell_id = spell_id,
                "Aura expired"
            );
        }
    }

    fn send_aura_update_applied(
        &self,
        spell_id: i32,
        slot: u8,
        caster: ObjectGuid,
        duration: u32,
        flags: u32,
    ) {
        use wow_packet::packets::aura::{AuraUpdate, AuraData};
        use wow_packet::ServerPacket;

        let update = AuraUpdate {
            target_guid: self.player_guid.unwrap_or(ObjectGuid::EMPTY),
            updated_auras: vec![AuraData {
                slot,
                spell_id,
                aura_flags: flags,
                duration_total: duration,
                duration_remaining: duration,
                stack_count: 1,
                caster_guid: caster,
                effect_data: None,
            }],
            removed_aura_slots: vec![],
        };
        self.send_packet(&update);
    }

    fn send_aura_update_removed(&self, slot: u8) {
        use wow_packet::packets::aura::AuraUpdate;

        let update = AuraUpdate {
            target_guid: self.player_guid.unwrap_or(ObjectGuid::EMPTY),
            updated_auras: vec![],
            removed_aura_slots: vec![slot],
        };
        self.send_packet(&update);
    }

    /// Send a TimeSyncRequest and schedule the next one.
    pub(crate) fn send_time_sync(&mut self) {
        use wow_packet::packets::misc::TimeSyncRequest;
        self.send_packet(&TimeSyncRequest {
            sequence_index: self.time_sync_next_counter,
        });
        trace!(
            "Sent TimeSyncRequest(seq={}) for account {}",
            self.time_sync_next_counter,
            self.account_id
        );
        // First 2 syncs are 5s apart, then every 10s (matches C#)
        self.time_sync_timer_ms = if self.time_sync_next_counter <= 1 {
            5000
        } else {
            10000
        };
        self.time_sync_next_counter += 1;
    }

    /// Process pending packets asynchronously. Call after `update()`.
    pub async fn process_pending(&mut self) {
        // ── Spell casting tick ─────────────────────────────────────────
        // Check if an active spell cast has completed and execute it.
        if self.state == SessionState::LoggedIn {
            self.tick_active_spell_cast().await;
        }

        // Check for instance link delivery (ConnectTo flow)
        self.poll_instance_link().await;

        // Process pending creature/gameobject spawn (async DB query)
        if let Some(spawn) = self.pending_creature_spawn.take() {
            self.send_nearby_creatures(spawn.map_id, &spawn.position, spawn.zone_id)
                .await;
            self.send_nearby_gameobjects(spawn.map_id, &spawn.position, spawn.zone_id)
                .await;
        }

        let packets: Vec<WorldPacket> = self.pending_packets.drain(..).collect();
        for pkt in packets {
            self.dispatch_packet(pkt).await;
        }
    }


    /// Poll the instance link oneshot. When received, swap channels and
    /// continue the player login on the instance socket.
    async fn poll_instance_link(&mut self) {
        let rx = match self.instance_link_rx.as_mut() {
            Some(rx) => rx,
            None => return,
        };

        // Non-blocking check
        match rx.try_recv() {
            Ok(link) => {
                info!(
                    "Instance link received for account {}, swapping channels",
                    self.account_id
                );

                // Keep the old realm channels alive — if either TCP connection
                // drops the WoW client disconnects the whole session.
                // The realm reader/writer tasks hold the other ends of these
                // channels, so keeping these receivers/senders prevents the
                // realm socket from closing.
                let old_send_tx = std::mem::replace(&mut self.send_tx, link.send_tx);
                self.realm_send_tx = Some(old_send_tx);

                if let Some(pkt_rx) = link.pkt_rx {
                    let old_packet_rx = std::mem::replace(&mut self.packet_rx, pkt_rx);
                    self.realm_packet_rx = Some(old_packet_rx);
                }

                self.instance_link_rx = None;

                // Continue the player login sequence on the instance socket
                self.handle_continue_player_login().await;
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Not ready yet, keep waiting
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                warn!(
                    "Instance link channel closed for account {} — instance connection failed",
                    self.account_id
                );
                self.instance_link_rx = None;
                self.player_loading = None;
                self.connect_to_key = None;
            }
        }
    }

    /// Dispatch a single packet to its registered handler.
    async fn dispatch_packet(&mut self, mut pkt: WorldPacket) {
        let opcode_raw = pkt.opcode_raw();
        let opcode: ClientOpcodes = match num_traits::FromPrimitive::from_u32(u32::from(opcode_raw)) {
            Some(op) => op,
            None => {
                info!(
                    "Unknown client opcode 0x{opcode_raw:04X} from account {}",
                    self.account_id
                );
                return;
            }
        };

        let entry = match self.dispatch_table.get(&opcode) {
            Some(e) => *e,
            None => {
                info!(
                    "No handler for {:?} (0x{opcode_raw:04X}) from account {}",
                    opcode,
                    self.account_id
                );
                return;
            }
        };

        // Check session status
        if !self.is_status_allowed(entry.status) {
            warn!(
                "Handler {} rejected: session state {:?} doesn't match required {:?}",
                entry.handler_name, self.state, entry.status
            );
            return;
        }

        debug!(
            "Dispatching {:?} via {} for account {}",
            opcode, entry.handler_name, self.account_id
        );

        // Skip opcode before reading payload
        pkt.skip_opcode();

        match opcode {
            ClientOpcodes::EnumCharacters => {
                self.handle_enum_characters().await;
            }
            ClientOpcodes::CreateCharacter => {
                match wow_packet::packets::character::CreateCharacter::read(&mut pkt) {
                    Ok(create) => self.handle_create_character(create).await,
                    Err(e) => warn!("Failed to read CreateCharacter: {e}"),
                }
            }
            ClientOpcodes::CharDelete => {
                match wow_packet::packets::character::CharDelete::read(&mut pkt) {
                    Ok(del) => self.handle_char_delete(del).await,
                    Err(e) => warn!("Failed to read CharDelete: {e}"),
                }
            }
            ClientOpcodes::PlayerLogin => {
                match wow_packet::packets::character::PlayerLogin::read(&mut pkt) {
                    Ok(login) => self.handle_player_login(login).await,
                    Err(e) => warn!("Failed to read PlayerLogin: {e}"),
                }
            }
            ClientOpcodes::ConnectToFailed => {
                match wow_packet::packets::auth::ConnectToFailed::read(&mut pkt) {
                    Ok(failed) => self.handle_connect_to_failed(failed).await,
                    Err(e) => warn!("Failed to read ConnectToFailed: {e}"),
                }
            }
            ClientOpcodes::GetUndeleteCharacterCooldownStatus => {
                self.handle_get_undelete_cooldown_status().await;
            }
            ClientOpcodes::BattlenetRequest => {
                match wow_packet::packets::battlenet::BattlenetRequest::read(&mut pkt) {
                    Ok(req) => self.handle_battlenet_request(req).await,
                    Err(e) => warn!("Failed to read BattlenetRequest: {e}"),
                }
            }
            ClientOpcodes::ServerTimeOffsetRequest => {
                self.handle_server_time_offset_request().await;
            }
            ClientOpcodes::RequestPlayedTime => {
                // TriggerScriptEvent: 1 byte bool — mirrors it back in the response.
                let trigger = pkt.read_uint8().unwrap_or(0) != 0;
                self.handle_request_played_time(trigger).await;
            }
            ClientOpcodes::SetSelection => {
                self.handle_set_selection(pkt).await;
            }
            ClientOpcodes::AreaTrigger => {
                self.handle_area_trigger(pkt).await;
            }
            ClientOpcodes::RequestCemeteryList => {
                self.handle_request_cemetery_list(pkt).await;
            }
            ClientOpcodes::TaxiNodeStatusQuery => {
                self.handle_taxi_node_status_query(pkt).await;
            }
            ClientOpcodes::ChatJoinChannel => {
                self.handle_chat_join_channel(pkt).await;
            }
            ClientOpcodes::MoveTimeSkipped => {
                self.handle_move_time_skipped(pkt).await;
            }
            ClientOpcodes::DbQueryBulk => {
                match wow_packet::packets::misc::DbQueryBulk::read(&mut pkt) {
                    Ok(query) => self.handle_db_query_bulk(query).await,
                    Err(e) => warn!("Failed to read DbQueryBulk: {e}"),
                }
            }
            ClientOpcodes::HotfixRequest => {
                match wow_packet::packets::misc::HotfixRequest::read(&mut pkt) {
                    Ok(req) => self.handle_hotfix_request(req).await,
                    Err(e) => warn!("Failed to read HotfixRequest: {e}"),
                }
            }
            ClientOpcodes::TimeSyncResponse
            | ClientOpcodes::TimeSyncResponseDropped
            | ClientOpcodes::TimeSyncResponseFailed => {
                match wow_packet::packets::misc::TimeSyncResponse::read(&mut pkt) {
                    Ok(resp) => self.handle_time_sync_response(resp).await,
                    Err(e) => warn!("Failed to read TimeSyncResponse: {e}"),
                }
            }
            ClientOpcodes::LogoutRequest => {
                match wow_packet::packets::misc::LogoutRequest::read(&mut pkt) {
                    Ok(req) => self.handle_logout_request(req).await,
                    Err(e) => warn!("Failed to read LogoutRequest: {e}"),
                }
            }
            ClientOpcodes::LogoutCancel => {
                self.handle_logout_cancel().await;
            }
            ClientOpcodes::QueryCreature => {
                match wow_packet::packets::query::QueryCreature::read(&mut pkt) {
                    Ok(query) => self.handle_query_creature(query).await,
                    Err(e) => warn!("Failed to read QueryCreature: {e}"),
                }
            }

            ClientOpcodes::QueryGameObject => {
                match wow_packet::packets::query::QueryGameObject::read(&mut pkt) {
                    Ok(query) => self.handle_query_game_object(query).await,
                    Err(e) => warn!("Failed to read QueryGameObject: {e}"),
                }
            }
            ClientOpcodes::QueryPlayerNames => {
                match wow_packet::packets::query::QueryPlayerNames::read(&mut pkt) {
                    Ok(query) => self.handle_query_player_names(query).await,
                    Err(e) => warn!("Failed to read QueryPlayerNames: {e}"),
                }
            }
            ClientOpcodes::QueryRealmName => {
                match wow_packet::packets::query::QueryRealmName::read(&mut pkt) {
                    Ok(query) => self.handle_query_realm_name(query),
                    Err(e) => warn!("Failed to read QueryRealmName: {e}"),
                }
            }
            ClientOpcodes::Ping => {
                match wow_packet::packets::auth::Ping::read(&mut pkt) {
                    Ok(ping) => self.handle_ping(ping).await,
                    Err(e) => warn!("Failed to read Ping: {e}"),
                }
            }
            ClientOpcodes::TalkToGossip => {
                match wow_packet::packets::gossip::Hello::read(&mut pkt) {
                    Ok(hello) => self.handle_gossip_hello(hello).await,
                    Err(e) => warn!("Failed to read TalkToGossip: {e}"),
                }
            }
            ClientOpcodes::AuctionHelloRequest => {
                self.handle_auction_hello_request(pkt).await;
            }
            ClientOpcodes::BankerActivate => {
                match wow_packet::packets::gossip::Hello::read(&mut pkt) {
                    Ok(hello) => self.handle_banker_activate(hello).await,
                    Err(e) => warn!("Failed to read BankerActivate: {e}"),
                }
            }
            ClientOpcodes::BinderActivate => {
                match wow_packet::packets::gossip::Hello::read(&mut pkt) {
                    Ok(hello) => self.handle_binder_activate(hello).await,
                    Err(e) => warn!("Failed to read BinderActivate: {e}"),
                }
            }
            ClientOpcodes::TabardVendorActivate => {
                self.handle_tabard_vendor_activate(pkt).await;
            }
            ClientOpcodes::SpiritHealerActivate => {
                self.handle_spirit_healer_activate(pkt).await;
            }
            ClientOpcodes::RepairItem => {
                self.handle_repair_item(pkt).await;
            }
            ClientOpcodes::RequestStabledPets => {
                self.handle_request_stabled_pets(pkt).await;
            }
            ClientOpcodes::GossipSelectOption => {
                match wow_packet::packets::gossip::GossipSelectOption::read(&mut pkt) {
                    Ok(select) => self.handle_gossip_select_option(select).await,
                    Err(e) => warn!("Failed to read GossipSelectOption: {e}"),
                }
            }
            ClientOpcodes::QueryNpcText => {
                match wow_packet::packets::gossip::QueryNpcText::read(&mut pkt) {
                    Ok(query) => self.handle_query_npc_text(query).await,
                    Err(e) => warn!("Failed to read QueryNpcText: {e}"),
                }
            }
            ClientOpcodes::ListInventory => {
                match wow_packet::packets::gossip::Hello::read(&mut pkt) {
                    Ok(hello) => self.handle_list_inventory(hello).await,
                    Err(e) => warn!("Failed to read ListInventory: {e}"),
                }
            }
            ClientOpcodes::BuyItem => {
                match wow_packet::packets::misc::BuyItem::read(&mut pkt) {
                    Ok(buy) => self.handle_buy_item(buy).await,
                    Err(e) => warn!("Failed to read BuyItem: {e}"),
                }
            }
            ClientOpcodes::BuyBackItem => {
                match wow_packet::packets::misc::BuyBackItem::read(&mut pkt) {
                    Ok(buyback) => self.handle_buy_back_item(buyback).await,
                    Err(e) => warn!("Failed to read BuyBackItem: {e}"),
                }
            }
            ClientOpcodes::SellItem => {
                match wow_packet::packets::misc::SellItem::read(&mut pkt) {
                    Ok(sell) => self.handle_sell_item(sell).await,
                    Err(e) => warn!("Failed to read SellItem: {e}"),
                }
            }
            ClientOpcodes::ItemPurchaseRefund => {
                match wow_packet::packets::item::ItemPurchaseRefund::read(&mut pkt) {
                    Ok(refund) => self.handle_item_purchase_refund(refund).await,
                    Err(e) => warn!("Failed to read ItemPurchaseRefund: {e}"),
                }
            }
            ClientOpcodes::TrainerList => {
                match wow_packet::packets::gossip::Hello::read(&mut pkt) {
                    Ok(hello) => self.handle_trainer_list(hello).await,
                    Err(e) => warn!("Failed to read TrainerList: {e}"),
                }
            }
            ClientOpcodes::QuestGiverHello => {
                self.handle_quest_giver_hello(pkt).await;
            }
            ClientOpcodes::QuestGiverStatusQuery => {
                self.handle_quest_giver_status_query(pkt).await;
            }
            ClientOpcodes::QuestGiverStatusMultipleQuery => {
                self.handle_quest_giver_status_multiple_query().await;
            }
            ClientOpcodes::SwapInvItem => {
                match wow_packet::packets::item::SwapInvItem::read(&mut pkt) {
                    Ok(swap) => self.handle_swap_inv_item(swap).await,
                    Err(e) => warn!("Failed to read SwapInvItem: {e}"),
                }
            }
            ClientOpcodes::AutoEquipItem => {
                match wow_packet::packets::item::AutoEquipItem::read(&mut pkt) {
                    Ok(equip) => self.handle_auto_equip_item(equip).await,
                    Err(e) => warn!("Failed to read AutoEquipItem: {e}"),
                }
            }
            ClientOpcodes::SwapItem => {
                match wow_packet::packets::item::SwapItem::read(&mut pkt) {
                    Ok(swap) => self.handle_swap_item(swap).await,
                    Err(e) => warn!("Failed to read SwapItem: {e}"),
                }
            }
            ClientOpcodes::AutoStoreBagItem => {
                match wow_packet::packets::item::AutoStoreBagItem::read(&mut pkt) {
                    Ok(store) => self.handle_auto_store_bag_item(store).await,
                    Err(e) => warn!("Failed to read AutoStoreBagItem: {e}"),
                }
            }
            ClientOpcodes::DestroyItem => {
                match wow_packet::packets::item::DestroyItemPkt::read(&mut pkt) {
                    Ok(destroy) => self.handle_destroy_item(destroy).await,
                    Err(e) => warn!("Failed to read DestroyItem: {e}"),
                }
            }
            ClientOpcodes::ShowTradeSkill => {
                match wow_packet::packets::misc::ShowTradeSkill::read(&mut pkt) {
                    Ok(show) => self.handle_show_trade_skill(show).await,
                    Err(e) => warn!("Failed to read ShowTradeSkill: {e}"),
                }
            }
            // ── Movement opcodes (all share the same handler) ───────
            ClientOpcodes::MoveStartForward
            | ClientOpcodes::MoveStartBackward
            | ClientOpcodes::MoveStop
            | ClientOpcodes::MoveStartStrafeLeft
            | ClientOpcodes::MoveStartStrafeRight
            | ClientOpcodes::MoveStopStrafe
            | ClientOpcodes::MoveStartTurnLeft
            | ClientOpcodes::MoveStartTurnRight
            | ClientOpcodes::MoveStopTurn
            | ClientOpcodes::MoveStartPitchUp
            | ClientOpcodes::MoveStartPitchDown
            | ClientOpcodes::MoveStopPitch
            | ClientOpcodes::MoveSetRunMode
            | ClientOpcodes::MoveSetWalkMode
            | ClientOpcodes::MoveHeartbeat
            | ClientOpcodes::MoveFallLand
            | ClientOpcodes::MoveFallReset
            | ClientOpcodes::MoveJump
            | ClientOpcodes::MoveSetFacing
            | ClientOpcodes::MoveSetFacingHeartbeat
            | ClientOpcodes::MoveSetPitch
            | ClientOpcodes::MoveSetFly
            | ClientOpcodes::MoveStartAscend
            | ClientOpcodes::MoveStopAscend
            | ClientOpcodes::MoveStartDescend
            | ClientOpcodes::MoveStartSwim
            | ClientOpcodes::MoveStopSwim
            | ClientOpcodes::MoveUpdateFallSpeed => {
                self.handle_movement(pkt).await;
            }

            // ── Movement control opcodes ────────────────────────────
            ClientOpcodes::SetActiveMover => {
                match wow_packet::packets::movement::SetActiveMover::read(&mut pkt) {
                    Ok(mover) => self.handle_set_active_mover(mover).await,
                    Err(e) => warn!("Failed to read SetActiveMover: {e}"),
                }
            }
            ClientOpcodes::MoveInitActiveMoverComplete => {
                match wow_packet::packets::movement::MoveInitActiveMoverComplete::read(&mut pkt) {
                    Ok(init) => self.handle_move_init_active_mover_complete(init).await,
                    Err(e) => warn!("Failed to read MoveInitActiveMoverComplete: {e}"),
                }
            }

            // ── Combat opcodes ──────────────────────────────────────
            ClientOpcodes::AttackSwing => {
                self.handle_attack_swing(pkt).await;
            }
            ClientOpcodes::AttackStop => {
                self.handle_attack_stop(pkt).await;
            }
            ClientOpcodes::SetSheathed => {
                self.handle_set_sheathed(pkt);
            }

            // ── Loot opcodes ────────────────────────────────────────
            ClientOpcodes::LootUnit => {
                self.handle_loot_unit(pkt).await;
            }
            ClientOpcodes::LootItem => {
                self.handle_loot_item(pkt).await;
            }
            ClientOpcodes::LootMoney => {
                self.handle_loot_money(pkt).await;
            }
            ClientOpcodes::LootRelease => {
                self.handle_loot_release(pkt).await;
            }

            // ── Chat opcodes ────────────────────────────────────────
            ClientOpcodes::ChatMessageSay => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Say).await;
            }
            ClientOpcodes::ChatMessageYell => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Yell).await;
            }
            ClientOpcodes::ChatMessageParty => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Party).await;
            }
            ClientOpcodes::ChatMessageGuild => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Guild).await;
            }
            ClientOpcodes::ChatMessageRaid => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Raid).await;
            }
            ClientOpcodes::ChatMessageRaidWarning => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::RaidWarning).await;
            }
            ClientOpcodes::ChatMessageInstanceChat => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::InstanceChat).await;
            }
            ClientOpcodes::ChatMessageWhisper => {
                self.handle_chat_whisper(pkt).await;
            }
            ClientOpcodes::ChatMessageEmote => {
                self.handle_chat_emote(pkt).await;
            }

            // ── Spell cast ────────────────────────────────────────────────────
            ClientOpcodes::CastSpell => {
                self.handle_cast_spell(pkt).await;
            }
            ClientOpcodes::CancelCast => {
                self.handle_cancel_cast(pkt).await;
            }
            ClientOpcodes::CancelChannelling => {
                self.handle_cancel_channelling(pkt).await;
            }
            ClientOpcodes::OpenItem => {
                self.handle_open_item(pkt).await;
            }

            // ── QueryTime / QueryNextMailTime ─────────────────────────────────
            ClientOpcodes::QueryTime => {
                self.handle_query_time().await;
            }
            ClientOpcodes::QueryNextMailTime => {
                self.handle_query_next_mail_time().await;
            }

            // ── Silent-ignore stubs (login-time client packets, no response) ──
            ClientOpcodes::LoadingScreenNotify => {
                self.handle_loading_screen_notify(pkt).await;
            }
            ClientOpcodes::ViolenceLevel => {
                self.handle_violence_level(pkt).await;
            }
            ClientOpcodes::OverrideScreenFlash => {
                self.handle_override_screen_flash(pkt).await;
            }
            ClientOpcodes::QueuedMessagesEnd => {
                self.handle_queued_messages_end(pkt).await;
            }
            ClientOpcodes::ChatUnregisterAllAddonPrefixes => {
                self.handle_chat_unregister_all_addon_prefixes(pkt).await;
            }
            ClientOpcodes::SetActionBarToggles => {
                self.handle_set_action_bar_toggles(pkt).await;
            }
            ClientOpcodes::SaveCufProfiles => {
                self.handle_save_cuf_profiles(pkt).await;
            }
            ClientOpcodes::GuildSetAchievementTracking => {
                self.handle_guild_set_achievement_tracking(pkt).await;
            }
            ClientOpcodes::GetItemPurchaseData => {
                self.handle_get_item_purchase_data(pkt).await;
            }
            ClientOpcodes::RequestForcedReactions => {
                self.handle_request_forced_reactions(pkt).await;
            }
            ClientOpcodes::RequestBattlefieldStatus => {
                self.handle_request_battlefield_status(pkt).await;
            }
            ClientOpcodes::RequestRatedPvpInfo => {
                self.handle_request_rated_pvp_info(pkt).await;
            }
            ClientOpcodes::RequestPvpRewards => {
                self.handle_request_pvp_rewards(pkt).await;
            }
            ClientOpcodes::DfGetSystemInfo => {
                self.handle_df_get_system_info(pkt).await;
            }
            ClientOpcodes::DfGetJoinStatus => {
                self.handle_df_get_join_status(pkt).await;
            }
            ClientOpcodes::CalendarGetNumPending => {
                self.handle_calendar_get_num_pending(pkt).await;
            }
            ClientOpcodes::GmTicketGetCaseStatus => {
                self.handle_gm_ticket_get_case_status(pkt).await;
            }
            ClientOpcodes::GuildBankRemainingWithdrawMoneyQuery => {
                self.handle_guild_bank_remaining_withdraw_money_query(pkt).await;
            }
            ClientOpcodes::BattlePetRequestJournal => {
                self.handle_battle_pet_request_journal(pkt).await;
            }
            ClientOpcodes::ArenaTeamRoster => {
                self.handle_arena_team_roster(pkt).await;
            }
            ClientOpcodes::RequestRaidInfo => {
                self.handle_request_raid_info(pkt).await;
            }
            ClientOpcodes::RequestConquestFormulaConstants => {
                self.handle_request_conquest_formula_constants(pkt).await;
            }
            ClientOpcodes::RequestLfgListBlacklist => {
                self.handle_request_lfg_list_blacklist(pkt).await;
            }
            ClientOpcodes::LfgListGetStatus => {
                self.handle_lfg_list_get_status(pkt).await;
            }
            ClientOpcodes::GetAccountCharacterList => {
                self.handle_get_account_character_list(pkt).await;
            }
            ClientOpcodes::QueryCountdownTimer => {
                self.handle_request_countdown_timer(pkt).await;
            }
            ClientOpcodes::CalendarGet => {
                self.handle_calendar_get(pkt).await;
            }
            ClientOpcodes::CloseInteraction => {
                self.handle_close_interaction(pkt).await;
            }
            ClientOpcodes::AuctionListBidderItems => {
                self.handle_auction_list_bidder_items(pkt).await;
            }
            ClientOpcodes::AuctionListOwnerItems => {
                self.handle_auction_list_owner_items(pkt).await;
            }
            ClientOpcodes::AuctionListPendingSales => {
                self.handle_auction_list_pending_sales(pkt).await;
            }
            ClientOpcodes::CommerceTokenGetLog => {
                self.handle_commerce_token_get_log(pkt).await;
            }
            ClientOpcodes::GameObjUse => {
                self.handle_game_obj_use(pkt).await;
            }
            ClientOpcodes::GameObjReportUse => {
                self.handle_game_obj_report_use(pkt).await;
            }
            ClientOpcodes::AddFriend => {
                self.handle_add_friend(pkt).await;
            }
            ClientOpcodes::DelFriend => {
                self.handle_del_friend(pkt).await;
            }
            ClientOpcodes::SendContactList => {
                self.handle_send_contact_list(pkt).await;
            }

            // ── Group / Party opcodes ─────────────────────────────────────────
            ClientOpcodes::PartyInvite => {
                self.handle_party_invite(pkt).await;
            }
            ClientOpcodes::PartyInviteResponse => {
                self.handle_party_invite_response(pkt).await;
            }
            ClientOpcodes::LeaveGroup => {
                self.handle_leave_group(pkt).await;
            }

            ClientOpcodes::Inspect => {
                self.handle_inspect(pkt).await;
            }

            // Empty stubs matching C# — these client opcodes are sent during
            // character select but require no response (Blizzard services).
            ClientOpcodes::BattlePayGetProductList
            | ClientOpcodes::BattlePayGetPurchaseList
            | ClientOpcodes::UpdateVasPurchaseStates
            | ClientOpcodes::SocialContractRequest => {
                trace!(
                    "Stub handler for {:?} (0x{:04X}) — no response needed",
                    opcode,
                    opcode_raw
                );
            }
            _ => {
                match entry.processing {
                    PacketProcessing::Inplace => {
                        trace!(
                            "Processing {:?} inplace via {}",
                            opcode,
                            entry.handler_name
                        );
                    }
                    PacketProcessing::ThreadUnsafe => {
                        trace!(
                            "Queuing {:?} for thread-unsafe processing via {}",
                            opcode,
                            entry.handler_name
                        );
                    }
                    PacketProcessing::ThreadSafe => {
                        trace!(
                            "Processing {:?} via thread-safe handler {}",
                            opcode, entry.handler_name
                        );
                    }
                }
            }
        }
    }

    /// Check if the handler's required status matches the current session state.
    ///
    /// Matches C# WorldSession.Update() switch logic:
    /// - `Authed` → allowed in ANY state (authenticated, in-world, or transferring)
    /// - `LoggedIn` → only when player is in-world
    /// - `Transfer` → only during map transfers
    /// - `LoggedInOrRecentlyLogout` → in-world or recently disconnected
    fn is_status_allowed(&self, required: SessionStatus) -> bool {
        match required {
            SessionStatus::Authed => true, // C#: always allowed once authenticated
            SessionStatus::LoggedIn => self.state == SessionState::LoggedIn,
            SessionStatus::Transfer => self.state == SessionState::Transfer,
            SessionStatus::LoggedInOrRecentlyLogout => {
                self.state == SessionState::LoggedIn
                    || self.state == SessionState::Disconnecting
            }
        }
    }

    /// Check for area triggers at the player's current position.
    ///
    /// This is called after movement updates to handle:
    /// - Teleportation triggers (e.g., dungeon exits)
    /// - Spell effects (e.g., silencing fields)
    /// - Custom trigger actions
    ///
    /// Manages trigger state to prevent retriggering:
    /// - Entry: when player enters a trigger (was not in one)
    /// - Exit: when player leaves a trigger (was in one, no longer is)
    pub async fn check_area_triggers(&mut self) {
        let (Some(pos), Some(store)) = (self.player_position, self.area_trigger_store.as_ref()) else {
            return;
        };

        // Get all triggers at the current position on the player's current map
        let triggers = store.get_triggers_at_position(self.current_map_id, &pos);

        // Check if we've exited the previous trigger
        if let Some(prev_trigger_id) = self.active_area_trigger {
            if !triggers.iter().any(|t| t.trigger_id == prev_trigger_id) {
                info!(
                    account = self.account_id,
                    trigger_id = prev_trigger_id,
                    "Exited area trigger"
                );
                self.active_area_trigger = None;
            }
        }

        // Check if we've entered a new trigger
        if let Some(trigger) = triggers.first() {
            let trigger_id = trigger.trigger_id;

            // Only trigger if this is a NEW trigger (wasn't active before)
            if self.active_area_trigger != Some(trigger_id) {
                info!(
                    account = self.account_id,
                    trigger_id = trigger.trigger_id,
                    "Entered area trigger"
                );
                self.active_area_trigger = Some(trigger_id);

                // Handle teleportation if present
                if let Some(ref teleport) = trigger.teleport {
                    info!(
                        account = self.account_id,
                        trigger_id = trigger.trigger_id,
                        target_map = teleport.target_map,
                        target_x = teleport.target_position.x,
                        target_y = teleport.target_position.y,
                        target_z = teleport.target_position.z,
                        "Teleporting player via area trigger"
                    );
                    self.teleport_to(teleport.target_map, teleport.target_position)
                        .await;
                }
            }
        }
    }

    /// Teleport the player to a new map and position.
    ///
    /// Sends SMSG_TRANSFER_PENDING (0x25cd) to initiate the transfer.
    /// The client will respond with CMSG_WORLD_PORT_ACK when ready.
    ///
    /// C# ref: Player.TeleportTo → SendTransferPending
    pub async fn teleport_to(&mut self, new_map: u32, new_pos: wow_core::Position) {
        // Validate inputs
        if new_map as u16 > 0xFFF {
            warn!(
                "Invalid map ID {} for teleport from account {}",
                new_map, self.account_id
            );
            return;
        }

        let Some(current_pos) = self.player_position else {
            warn!(
                "Cannot teleport account {}: no current position",
                self.account_id
            );
            return;
        };

        info!(
            account = self.account_id,
            old_map = self.current_map_id,
            new_map = new_map,
            old_pos = format!("({:.2}, {:.2}, {:.2})", current_pos.x, current_pos.y, current_pos.z),
            new_pos = format!("({:.2}, {:.2}, {:.2})", new_pos.x, new_pos.y, new_pos.z),
            "Player teleporting to new map"
        );

        use wow_packet::packets::misc::{SuspendToken, TransferPending};

        // 1. SMSG_TRANSFER_PENDING — tell client to start loading screen
        let transfer_pending = TransferPending {
            map_id: new_map,
            old_map_position: current_pos,
            ship: None,
            transfer_spell_id: None,
        };
        self.send_packet(&transfer_pending);

        // 2. Store pending destination — completed in handle_world_port_response
        self.pending_teleport = Some((new_map, new_pos));
        self.active_area_trigger = None;

        // 3. SMSG_SUSPEND_TOKEN — pause movement processing on client
        self.send_packet(&SuspendToken { sequence_index: 1, reason: 1 });

        // 4. Transition to Transfer state — only WorldPortResponse accepted now
        self.state = SessionState::Transfer;

        info!(
            account = self.account_id,
            "Teleport initiated: map {} → {} dest ({:.2}, {:.2}, {:.2}); awaiting WorldPortResponse",
            self.current_map_id, new_map, new_pos.x, new_pos.y, new_pos.z
        );
    }

    /// Send a server packet back to the client via the instance (default) channel.
    pub fn send_packet(&self, pkt: &impl wow_packet::ServerPacket) {
        let data = pkt.to_bytes();
        if self.send_tx.send(data).is_err() {
            warn!("Send channel closed for account {}", self.account_id);
        }
    }

    /// Send a server packet on the **realm** connection.
    ///
    /// Some packets (e.g. `QueryPlayerNamesResponse`) must travel on the
    /// realm socket, not the instance socket.  Falls back to `send_tx` if
    /// no realm channel exists (pre-ConnectTo or single-connection mode).
    pub fn send_packet_realm(&self, pkt: &impl wow_packet::ServerPacket) {
        let data = pkt.to_bytes();
        let tx = self.realm_send_tx.as_ref().unwrap_or(&self.send_tx);
        if tx.send(data).is_err() {
            warn!("Realm send channel closed for account {}", self.account_id);
        }
    }

    /// Send pre-serialized packet bytes to the client.
    ///
    /// Used for packets with dynamic opcodes (e.g. `SetSpellModifier`
    /// which uses the same struct for Flat and Pct variants).
    pub fn send_raw_packet(&self, data: &[u8]) {
        if self.send_tx.send(data.to_vec()).is_err() {
            warn!("Send channel closed for account {}", self.account_id);
        }
    }

    pub fn send_equip_error(
        &self,
        result: InventoryResult,
        item1: Option<ObjectGuid>,
        item2: Option<ObjectGuid>,
        required_level: u32,
        limit_category: u32,
    ) {
        let mut packet = InventoryChangeFailure::new(
            result,
            item1.unwrap_or(ObjectGuid::EMPTY),
            item2.unwrap_or(ObjectGuid::EMPTY),
        );

        if result != InventoryResult::Ok {
            packet.container_b_slot = 0;
            match result {
                InventoryResult::CantEquipLevelI | InventoryResult::PurchaseLevelTooLow => {
                    packet.level = required_level;
                }
                InventoryResult::ItemMaxLimitCategoryCountExceededIs
                | InventoryResult::ItemMaxLimitCategorySocketedExceededIs
                | InventoryResult::ItemMaxLimitCategoryEquippedExceededIs => {
                    packet.limit_category = limit_category;
                }
                _ => {}
            }
        }

        self.send_packet(&packet);
    }

    pub fn send_buy_error(
        &self,
        result: BuyResult,
        creature_guid: Option<ObjectGuid>,
        item: u32,
    ) {
        self.send_packet(&BuyFailed {
            vendor_guid: creature_guid.unwrap_or(ObjectGuid::EMPTY),
            muid: item as i32,
            reason: result,
        });
    }

    pub fn send_sell_error(
        &self,
        result: SellResult,
        creature_guid: Option<ObjectGuid>,
        item_guid: ObjectGuid,
    ) {
        self.send_packet(&SellResponse::error(
            creature_guid.unwrap_or(ObjectGuid::EMPTY),
            item_guid,
            result,
        ));
    }

    fn item_push_result_from_send_new_item_plan(plan: &SendNewItemPlan) -> ItemPushResult {
        ItemPushResult {
            player_guid: plan.player_guid,
            slot: plan.slot,
            slot_in_bag: i32::from(plan.slot_in_bag),
            item: ItemInstance {
                item_id: plan.item_instance.item_id as i32,
                random_properties_seed: plan.item_instance.random_properties_seed,
                random_properties_id: plan.item_instance.random_properties_id,
                item_bonus: None,
                modifications: ItemModList {
                    values: plan
                        .item_instance
                        .modifications
                        .iter()
                        .map(|modifier| ItemMod::new(modifier.value, modifier.modifier_type))
                        .collect(),
                },
            },
            quest_log_item_id: plan.quest_log_item_id as i32,
            quantity: plan.quantity as i32,
            quantity_in_inventory: plan.quantity_in_inventory as i32,
            dungeon_encounter_id: plan.dungeon_encounter_id as i32,
            battle_pet_species_id: plan.battle_pet_species_id as i32,
            battle_pet_breed_id: plan.battle_pet_breed_id as i32,
            battle_pet_breed_quality: u32::from(plan.battle_pet_breed_quality),
            battle_pet_level: plan.battle_pet_level as i32,
            item_guid: plan.item_guid,
            pushed: plan.pushed,
            display_text: match plan.display_text {
                SendNewItemDisplayText::Normal => ItemPushResultDisplayType::Normal,
                SendNewItemDisplayText::EncounterLoot => ItemPushResultDisplayType::EncounterLoot,
            },
            created: plan.created,
            is_bonus_roll: false,
            is_encounter_loot: plan.is_encounter_loot,
        }
    }

    pub fn send_new_item_plan(&self, plan: &SendNewItemPlan) {
        let packet = Self::item_push_result_from_send_new_item_plan(plan);
        if plan.delivery == SendNewItemDelivery::GroupBroadcast {
            use wow_packet::ServerPacket;

            if self.broadcast_item_push_result_to_group(packet.to_bytes()) {
                return;
            }
        }

        self.send_packet(&packet);
    }

    fn broadcast_item_push_result_to_group(&self, bytes: Vec<u8>) -> bool {
        let (Some(group_guid), Some(group_registry), Some(player_registry)) =
            (self.group_guid, &self.group_registry, &self.player_registry)
        else {
            return false;
        };

        let Some(group) = group_registry.get(&group_guid) else {
            return false;
        };

        let mut delivered = false;
        for member_guid in &group.members {
            if let Some(member) = player_registry.get(member_guid) {
                delivered |= member.send_tx.send(bytes.clone()).is_ok();
            }
        }

        delivered
    }

    pub fn send_item_time_update_plan(&self, update: &PlayerItemTimeUpdate) {
        self.send_packet(&ItemTimeUpdate {
            item_guid: update.item_guid,
            duration_left: update.expiration,
        });
    }

    pub fn send_item_time_update_plans(&self, updates: &[PlayerItemTimeUpdate]) {
        for update in updates {
            self.send_item_time_update_plan(update);
        }
    }

    pub fn send_item_enchant_time_update_plan(
        &self,
        owner_guid: ObjectGuid,
        update: &PlayerEnchantTimeUpdate,
    ) {
        self.send_packet(&ItemEnchantTimeUpdate {
            owner_guid,
            item_guid: update.item_guid,
            duration_left: update.duration_secs,
            slot: update.slot as u32,
        });
    }

    pub fn send_item_enchant_time_update_plans(
        &self,
        owner_guid: ObjectGuid,
        updates: &[PlayerEnchantTimeUpdate],
    ) {
        for update in updates {
            self.send_item_enchant_time_update_plan(owner_guid, update);
        }
    }

    /// Send session initialization packets (first encrypted packets after
    /// EnterEncryptedModeAck). Matches C# `InitializeSessionCallback`.
    ///
    /// These packets are sent immediately when the session starts, before any
    /// client packets are processed. They tell the client that auth succeeded
    /// and provide the initial glue screen data (character select).
    ///
    /// Exact C# order:
    /// 1. AuthResponse
    /// 2. SetTimeZoneInformation
    /// 3. FeatureSystemStatusGlueScreen (NOT the in-game FeatureSystemStatus!)
    /// 4. ClientCacheVersion
    /// 5. AvailableHotfixes (empty)
    /// 6. AccountDataTimes (global)
    /// 7. TutorialFlags
    /// 8. ConnectionStatus (State=1)
    pub fn send_session_init_packets(&self) {
        use wow_packet::packets::auth::*;
        use wow_packet::packets::misc::*;

        let vra = self.virtual_realm_address();

        // 1. AuthResponse (OK) — tells the client authentication succeeded
        let auth_response = AuthResponse {
            result: 0, // OK
            success_info: Some(AuthSuccessInfo {
                virtual_realm_address: vra,
                virtual_realms: vec![VirtualRealmInfo {
                    realm_address: vra,
                    is_local: true,
                    is_internal_realm: false,
                    realm_name_actual: String::from("RustyCore"),
                    realm_name_normalized: String::from("rustycore"),
                }],
                time_rested: 0,
                active_expansion_level: self.expansion,
                account_expansion_level: self.account_expansion,
                time_seconds_until_pc_kick: 0,
                available_classes: default_available_classes(),
                templates: vec![],
                currency_id: 0,
                time: unix_now(),
                game_time_info: GameTimeInfo {
                    billing_plan: 0,
                    time_remain: 0,
                    unknown735: 0,
                    in_game_room: false,
                },
                is_expansion_trial: false,
                force_character_template: false,
                num_players_horde: None,
                num_players_alliance: None,
                expansion_trial_expiration: None,
            }),
            wait_info: None,
        };
        self.send_packet(&auth_response);

        // 2. SetTimeZoneInformation
        self.send_packet(&SetTimeZoneInformation::utc());

        // 3. FeatureSystemStatusGlueScreen (character select version, NOT in-game)
        self.send_packet(&FeatureSystemStatusGlueScreen::default_wotlk());

        // 4. ClientCacheVersion (from world DB version.cache_id = 24081)
        self.send_packet(&ClientCacheVersion { cache_version: 24081 });

        // 5. AvailableHotfixes (empty — no hotfixes)
        self.send_packet(&AvailableHotfixes {
            virtual_realm_address: vra,
        });

        // 6. AccountDataTimes (global)
        self.send_packet(&AccountDataTimes::global());

        // 7. TutorialFlags
        self.send_packet(&TutorialFlags::all_shown());

        // 8. ConnectionStatus (State=1, SuppressNotification=true)
        // C# BattlenetPackets.cs: ConnectionStatus has no ConnectionType override,
        // so it's sent on the realm socket. State uses 2 bits, SuppressNotification
        // defaults to true.
        self.send_packet(&ConnectionStatus {
            state: 1,
            suppress_notification: true,
        });

        info!(
            "Session init packets sent for account {} (8 packets: AuthResponse → ConnectionStatus)",
            self.account_id
        );
    }

    /// Set the logged-in player GUID.
    pub fn set_player_guid(&mut self, guid: Option<ObjectGuid>) {
        self.player_guid = guid;
    }

    /// Get the logged-in player GUID.
    pub fn player_guid(&self) -> Option<ObjectGuid> {
        self.player_guid
    }

    /// Complete the logout: send LogoutComplete and mark session for disconnect.
    fn complete_logout(&mut self) {
        use wow_packet::packets::misc::LogoutComplete;

        info!("Logout complete for account {}", self.account_id);
        self.send_packet(&LogoutComplete);
        self.player_guid = None;
        self.state = SessionState::Authed;
    }

    /// Kick the session (mark as disconnecting).
    pub fn kick(&mut self, reason: &str) {
        warn!(
            "Kicking account {} ({}): {reason}",
            self.account_id, self.account_name
        );
        self.state = SessionState::Disconnecting;
    }

    /// Get the current session state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Set the session state (e.g., after character login).
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Time since the last packet was received.
    pub fn idle_time(&self) -> std::time::Duration {
        self.last_packet_time.elapsed()
    }

    /// Whether the session is disconnecting.
    pub fn is_disconnecting(&self) -> bool {
        self.state == SessionState::Disconnecting
    }

    /// Restore the realm socket as the primary send/receive channel.
    ///
    /// After a ConnectTo flow, `send_tx` and `packet_rx` point to the
    /// instance socket while the realm channels are stored in
    /// `realm_send_tx` / `realm_packet_rx`.  On logout the client
    /// returns to character select on the REALM connection, so we must
    /// swap back.  The old instance channels are simply dropped — the
    /// instance reader/writer tasks will notice and exit.
    pub(crate) fn restore_realm_channels(&mut self) {
        if let Some(realm_tx) = self.realm_send_tx.take() {
            info!(
                "Restoring realm send channel as primary for account {}",
                self.account_id
            );
            self.send_tx = realm_tx;
        }
        if let Some(realm_rx) = self.realm_packet_rx.take() {
            info!(
                "Restoring realm packet channel as primary for account {}",
                self.account_id
            );
            self.packet_rx = realm_rx;
        }
        // Clear any pending ConnectTo state
        self.instance_link_rx = None;
        self.connect_to_key = None;
        self.connect_to_serial = None;
        self.player_loading = None;
    }
}

// ── Creature AI / Combat tick methods ────────────────────────────

impl WorldSession {
    /// Called every ~200ms from the update loop.
    /// Advances creature movement state and sends MonsterMove packets.
    pub(crate) fn tick_creatures_sync(&mut self) {
        use wow_packet::packets::movement::MonsterMove;
        use wow_packet::ServerPacket;

        // Collect packets to send (avoids borrow conflict with send_packet)
        let mut to_send: Vec<Vec<u8>> = Vec::new();

        let guids: Vec<wow_core::ObjectGuid> = self.creatures.keys().cloned().collect();

        // ── Corpse despawn ─────────────────────────────────────────────────
        // C# ref: `Creature.RemoveCorpse` / `AllLootRemovedFromCorpse`.
        // After `corpse_despawn_at` passes, remove the dead creature from the
        // world and notify the client (destroy block in SMSG_UPDATE_OBJECT).
        let now = std::time::Instant::now();
        let despawn_guids: Vec<wow_core::ObjectGuid> = guids
            .iter()
            .filter(|g| {
                self.creatures
                    .get(g)
                    .map(|c| {
                        !c.is_alive
                            && c.corpse_despawn_at
                                .map(|t| now >= t)
                                .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if !despawn_guids.is_empty() {
            use wow_packet::packets::update::{CreatureCreateData, UpdateObject};
            use wow_packet::ServerPacket;

            let map_id = self.current_map_id;
            for g in &despawn_guids {
                // Before removing, save data needed for respawn.
                if let Some(c) = self.creatures.remove(g) {
                    // C# ref: AllLootRemovedFromCorpse → m_respawnTime = corpseRemoveTime + respawnDelay
                    let respawn_at = now + std::time::Duration::from_secs(c.respawn_time_secs);
                    // Build CreatureCreateData from saved AI fields (with sensible defaults
                    // for fields not stored in CreatureAI: scale, unit_class, timers, speeds).
                    let create_data = CreatureCreateData {
                        guid: c.guid,
                        entry: c.entry,
                        display_id: c.display_id,
                        native_display_id: c.display_id,
                        health: c.max_hp as i64,
                        max_health: c.max_hp as i64,
                        level: c.level,
                        faction_template: c.faction as i32,
                        npc_flags: c.npc_flags as u64,
                        unit_flags: c.unit_flags,
                        unit_flags2: 0,
                        unit_flags3: 0,
                        scale: 1.0,
                        unit_class: 1,
                        base_attack_time: 2000,
                        ranged_attack_time: 0,
                        zone_id: 0,
                        speed_walk_rate: 1.0,
                        speed_run_rate: 1.14286,
                    };
                    self.respawn_queue.push(PendingRespawn {
                        respawn_at,
                        home_pos: c.home_pos,
                        create_data,
                        max_hp: c.max_hp,
                        level: c.level,
                        min_dmg: c.min_dmg,
                        max_dmg: c.max_dmg,
                        aggro_radius: c.aggro_radius,
                        npc_flags: c.npc_flags,
                        unit_flags: c.unit_flags,
                        map_id,
                    });
                    tracing::info!(
                        "Corpse despawned: {:?} (entry {}) — respawn in {}s",
                        g, c.entry, c.respawn_time_secs
                    );
                }
                self.visible_creatures.remove(g);
            }
            let pkt = UpdateObject::destroy_objects(despawn_guids, map_id);
            if let Err(e) = self.send_tx.send(pkt.to_bytes()) {
                tracing::warn!("Failed to send despawn UpdateObject: {e}");
            }
        }
        // ── Respawn queue ──────────────────────────────────────────────────
        // C# ref: Creature::Update → RemoveCorpse → respawn via Map::AddToMap.
        let ready: Vec<PendingRespawn> = {
            let mut remaining = Vec::new();
            let mut spawn_now = Vec::new();
            for r in self.respawn_queue.drain(..) {
                if now >= r.respawn_at {
                    spawn_now.push(r);
                } else {
                    remaining.push(r);
                }
            }
            self.respawn_queue = remaining;
            spawn_now
        };

        for r in ready {
            use wow_packet::packets::update::UpdateObject;
            use wow_packet::ServerPacket;

            let guid = r.create_data.guid;
            let entry = r.create_data.entry;
            let display_id = r.create_data.display_id;
            let faction = r.create_data.faction_template as u32;

            tracing::info!(
                "Creature respawned: {:?} (entry {}) at {:?}",
                guid, entry, r.home_pos
            );

            // Send CREATE block to client.
            let block = UpdateObject::create_creature_block(r.create_data, &r.home_pos);
            let pkt = UpdateObject::create_creatures(vec![block], r.map_id);
            if let Err(e) = self.send_tx.send(pkt.to_bytes()) {
                tracing::warn!("Failed to send respawn packet: {e}");
            }

            // Recreate the creature AI at home position, fully alive.
            let ai = wow_ai::CreatureAI::new(
                guid,
                entry,
                r.home_pos,
                r.max_hp,
                r.level,
                r.min_dmg,
                r.max_dmg,
                r.aggro_radius,
                display_id,
                faction,
                r.npc_flags,
                r.unit_flags,
            );

            self.creatures.insert(guid, ai);
            self.visible_creatures.insert(guid);
        }
        // ──────────────────────────────────────────────────────────────────

        for guid in guids {
            let creature = match self.creatures.get_mut(&guid) {
                Some(c) => c,
                None => continue,
            };

            if !creature.is_alive {
                if creature.should_respawn() {
                    creature.respawn();
                }
                continue;
            }

            match creature.state {
                wow_ai::CreatureState::Idle => {
                    if creature.movement_finished() {
                        if creature.move_target.is_some() {
                            creature.finish_move();
                        }
                        if creature.should_wander() {
                            let dst = creature.pick_wander_destination();
                            let from = creature.current_pos;
                            let sid = creature.spline_id;
                            let dist = from.distance(&dst);
                            let dur = ((dist / 2.5) * 1000.0) as u32;
                            creature.begin_move(dst);
                            creature.state = wow_ai::CreatureState::WalkingRandom;
                            creature.reset_wander_timer();
                            // TODO: verify MonsterMove wire format before enabling
                        // let pkt = MonsterMove { ... };
                        // to_send.push(pkt.to_bytes());
                        let _ = (guid, from, sid, dur, dst);
                        }
                    }
                }
                wow_ai::CreatureState::WalkingRandom => {
                    if creature.movement_finished() {
                        creature.finish_move();
                        creature.state = wow_ai::CreatureState::Idle;
                        creature.reset_wander_timer();
                    }
                }
                wow_ai::CreatureState::Returning => {
                    if creature.movement_finished() {
                        creature.finish_move();
                        creature.state = wow_ai::CreatureState::Idle;
                    }
                }
                wow_ai::CreatureState::InCombat
                | wow_ai::CreatureState::Dead
                | wow_ai::CreatureState::WalkingWaypoint => {}
            }
        }

        // Send all movement packets
        for data in to_send {
            if self.send_tx.send(data).is_err() {
                break;
            }
        }
    }

    /// Called every ~100ms. Checks if an in-progress spell cast has completed.
    ///
    /// If `active_spell_cast` is set and its cast time has elapsed, this method
    /// executes the spell (applies effects, cooldowns, etc.) and clears the cast state.
    pub(crate) async fn tick_active_spell_cast(&mut self) {
        let Some(ref cast_state) = self.active_spell_cast.clone() else {
            return;
        };

        let elapsed_ms = cast_state.cast_start_time.elapsed().as_millis() as u32;

        if elapsed_ms >= cast_state.cast_time_ms {
            let spell_id = cast_state.spell_id;
            let target = cast_state.target_guid;
            let cast_id = cast_state.cast_id;
            let spell_visual = cast_state.spell_visual.clone();

            self.active_spell_cast = None;
            self.last_spell_cast_time = Some(Instant::now());

            // ← AQUÍ: Ejecutar spell
            if let Err(e) = self.execute_spell_with_visual(spell_id, target, cast_id, spell_visual).await {
                warn!(account = self.account_id, "Spell execution failed: {}", e);
                // Send CastFailed so client cancels cast animation
                use wow_packet::packets::spell::CastFailed;
                use wow_packet::ServerPacket;
                self.send_packet(&CastFailed {
                    cast_id,
                    spell_id,
                    reason: 2, // SpellCastResult::NotKnown
                    fail_arg1: 0,
                    fail_arg2: 0,
                });
            }
        }
    }

    /// Called every ~100ms. Handles auto-attack swing timer (player → creature).
    pub(crate) fn tick_combat_sync(&mut self) {
        use wow_packet::packets::combat::{AttackerStateUpdate, VICTIM_STATE_HIT, SAttackStop};
        use wow_packet::ServerPacket;

        let (player_guid, combat_target) = match (self.player_guid, self.combat_target) {
            (Some(pg), Some(ct)) => (pg, ct),
            _ => return,
        };

        // Check if target still exists
        let creature_exists = self.creatures.contains_key(&combat_target);
        if !creature_exists {
            self.combat_target = None;
            self.in_combat = false;
            return;
        }

        // Gather combat data, mutate creature state
        let (dmg, target_level, now_dead, was_alive) = {
            let creature = self.creatures.get_mut(&combat_target).unwrap();
            if !creature.is_alive {
                return;
            }
            if creature.state != wow_ai::CreatureState::InCombat {
                creature.enter_combat(player_guid);
            }
            if !creature.can_swing() {
                return;
            }
            let dmg = creature.roll_damage().max(1);
            let level = creature.level;
            let died = creature.take_damage(dmg);
            creature.record_swing();
            (dmg, level, died, true)
        };

        if !was_alive { return; }

        let over_damage = if now_dead { 0i32 } else { -1i32 };

        // Send damage event
        let state_update = AttackerStateUpdate {
            attacker: player_guid,
            victim: combat_target,
            damage: dmg as i32,
            over_damage,
            victim_state: VICTIM_STATE_HIT,
            school_mask: 1,
            target_level,
            expansion: 2,
        };
        if self.send_tx.send(state_update.to_bytes()).is_err() { return; }

        // TODO: creature health VALUES update — format needs verification vs client
        // (temporarily disabled to prevent client crash from malformed packet)

        if now_dead {
            let stop = SAttackStop {
                attacker: player_guid,
                victim: combat_target,
                now_dead: true,
            };
            let _ = self.send_tx.send(stop.to_bytes());
            self.combat_target = None;
            self.in_combat = false;
        }
    }

    /// Broadcast the newly logged-in player's CREATE block to all other players on the same map.
    ///
    /// Called after login is complete. Iterates through all players in the registry
    /// who are on the same map, creates an UpdateObject with the new player's CREATE block,
    /// and sends it to each via their send_tx channel.
    ///
    /// C# ref: `Player::SendInitialPacketsAfterAddToMap` → WorldSession broadcast logic.
    pub(crate) fn broadcast_create_player_to_others(&self) {
        use wow_packet::packets::update::{UpdateObject, PlayerCombatStats};
        use wow_packet::ServerPacket;

        let Some(guid) = self.player_guid else { return };
        let Some(registry) = &self.player_registry else { return };
        let Some(pos) = self.player_position else { return };

        // Build visible_items from this player's equipped inventory.
        let mut visible_items = [(0i32, 0u16, 0u16); 19];
        for (slot, item) in &self.inventory_items {
            if (*slot as usize) < 19 {
                visible_items[*slot as usize] = (item.entry_id as i32, 0u16, 0u16);
            }
        }
        let empty_inv_slots = [ObjectGuid::EMPTY; 141];
        let empty_skills = Vec::new();

        // Create the UpdateObject for this player (with is_self=false for other players)
        use crate::handlers::character::default_display_id;
        let update = UpdateObject::create_player(
            guid,
            self.player_race,
            self.player_class,
            self.player_gender,
            self.player_level,
            default_display_id(self.player_race, self.player_gender),
            &pos,
            self.current_map_id,
            0, // zone_id (would need to track)
            false, // is_self: other players see this as a regular player, not ActivePlayer
            visible_items,
            empty_inv_slots,
            PlayerCombatStats::default(), // other players don't need detailed combat stats
            empty_skills,
            self.player_gold,
            vec![], // quest_log — not sent to other players
        );

        // Serialize once, reuse for all broadcasts
        let bytes = update.to_bytes();

        // Count players to broadcast to
        let mut broadcast_count = 0;

        // Iterate through all players in the registry on the same map
        for entry in registry.iter() {
            let (other_guid, broadcast_info) = entry.pair();
            // Don't send to ourselves
            if *other_guid == guid {
                continue;
            }
            // Only send to players on the same map
            if broadcast_info.map_id != self.current_map_id {
                continue;
            }

            broadcast_count += 1;

            if let Err(_) = broadcast_info.send_tx.send(bytes.clone()) {
                debug!("Failed to broadcast CreatePlayer to {:?}", other_guid);
            } else {
                trace!("Broadcast CreatePlayer {:?} to {:?}", guid, other_guid);
            }
        }

        if broadcast_count > 0 {
            info!(
                "Broadcasted CreatePlayer for {:?} to {} players on map {}",
                guid, broadcast_count, self.current_map_id
            );
        }
    }

    /// Broadcast DestroyObject to all players on the same map when this player disconnects.
    pub(crate) fn broadcast_destroy_player_to_others(&self) {
        use wow_packet::packets::update::UpdateObject;
        use wow_packet::ServerPacket;

        let Some(guid) = self.player_guid else { return };
        let Some(registry) = &self.player_registry else { return };

        let destroy = UpdateObject::destroy_objects(vec![guid], self.current_map_id);
        let bytes = destroy.to_bytes();

        let mut count = 0usize;
        for entry in registry.iter() {
            let (other_guid, info) = entry.pair();
            if *other_guid == guid { continue; }
            if info.map_id != self.current_map_id { continue; }
            if info.send_tx.send(bytes.clone()).is_ok() {
                count += 1;
            }
        }
        if count > 0 {
            info!("Broadcast DestroyPlayer {:?} to {} players", guid, count);
        }
    }

    /// Receive CREATE blocks from all other players currently on the same map.
    ///
    /// Called after login is complete. Queries the player registry for all players
    /// on the current map (excluding self), builds their CREATE blocks, and sends
    /// them as UpdateObjects to this session.
    ///
    /// C# ref: `Player::SendInitialPacketsAfterAddToMap` → populate visibility with other players.
    pub(crate) fn receive_other_players_on_map(&self) {
        use wow_packet::packets::update::{UpdateObject, PlayerCombatStats};

        let Some(guid) = self.player_guid else { return };
        let Some(registry) = &self.player_registry else { return };

        let empty_inv_slots = [ObjectGuid::EMPTY; 141];
        let empty_skills = Vec::new();
        let default_combat_stats = PlayerCombatStats::default();

        let mut player_count = 0;

        // Iterate through all players in the registry
        for entry in registry.iter() {
            let (other_guid, broadcast_info) = entry.pair();

            // Skip self and players on different maps
            if *other_guid == guid || broadcast_info.map_id != self.current_map_id {
                continue;
            }

            player_count += 1;

            // Create UpdateObject for this other player using cached data from broadcast_info
            let update = UpdateObject::create_player(
                *other_guid,
                broadcast_info.race,
                broadcast_info.class,
                broadcast_info.sex,
                broadcast_info.level,
                broadcast_info.display_id,
                &broadcast_info.position,
                broadcast_info.map_id,
                0, // zone_id (unknown — would need separate tracking)
                false, // is_self: this is another player, not us
                broadcast_info.visible_items,
                empty_inv_slots,
                default_combat_stats,
                empty_skills.clone(),
                0, // coinage (don't send other players' gold)
                vec![], // quest_log — not sent to other players
            );

            self.send_packet(&update);
            trace!(
                "Sent CREATE block for other player {:?} to account {}",
                other_guid, self.account_id
            );
        }

        if player_count > 0 {
            info!(
                "Received CREATE blocks from {} other players on map {} for {:?}",
                player_count, self.current_map_id, guid
            );
        }
    }

    /// Check if any hostile creature should aggro the player based on proximity.
    /// Called from movement handlers (CMSG_MOVE_*).
    pub(crate) async fn check_creature_aggro(&mut self) {
        use wow_packet::packets::combat::AttackStart;
        use wow_packet::ServerPacket;

        if self.in_combat { return; }

        let player_pos = match self.player_position { Some(p) => p, None => return };
        let player_guid = match self.player_guid { Some(g) => g, None => return };

        let guids: Vec<wow_core::ObjectGuid> = self.creatures.keys().cloned().collect();
        let mut aggro_guid: Option<wow_core::ObjectGuid> = None;

        for guid in guids {
            let creature = match self.creatures.get_mut(&guid) {
                Some(c) => c,
                None => continue,
            };
            if !creature.is_alive || creature.aggro_radius <= 0.0 { continue; }
            if creature.try_aggro(player_guid, &player_pos) {
                aggro_guid = Some(guid);
                break;
            }
        }

        if let Some(guid) = aggro_guid {
            let start = AttackStart { attacker: guid, victim: player_guid };
            let _ = self.send_tx.send(start.to_bytes());
            self.combat_target = Some(guid);
            self.in_combat = true;
        }
    }

    /// Execute a spell — apply effects, set cooldown, send SMSG_SPELL_GO.
    ///
    /// Called for instant-cast spells. Delegates to execute_spell_with_visual
    /// with default cast_id and visual.
    pub async fn execute_spell(&mut self, spell_id: i32, target_guid: ObjectGuid) -> Result<(), &'static str> {
        use wow_packet::packets::spell::SpellCastVisual;
        
        self.execute_spell_with_visual(
            spell_id,
            target_guid,
            ObjectGuid::EMPTY,
            SpellCastVisual {
                spell_visual_id: 0,
                script_visual_id: 0,
            },
        ).await
    }

    /// Execute a spell with full visual/cast info — apply effects, set cooldown, send SMSG_SPELL_GO.
    ///
    /// Called after cast time completes or for instant-cast spells.
    /// Supports: heal (type 6), damage (type 2), aura application (type 35).
    pub async fn execute_spell_with_visual(
        &mut self,
        spell_id: i32,
        target_guid: ObjectGuid,
        cast_id: ObjectGuid,
        spell_visual: wow_packet::packets::spell::SpellCastVisual,
    ) -> Result<(), &'static str> {
        let player_guid = self.player_guid.ok_or("No player GUID")?;

        // Obtener SpellInfo
        let spell_info = self.spell_store()
            .and_then(|store| store.get(spell_id))
            .ok_or("Spell not found")?;

        info!(
            account = self.account_id,
            spell_id = spell_id,
            target = ?target_guid,
            effect_type = spell_info.effect_type,
            "Executing spell effect"
        );

        // Send SMSG_SPELL_GO
        use wow_packet::packets::spell::{SpellGoPkt, SpellTargetData};
        use wow_packet::ServerPacket;

        let go_pkt = SpellGoPkt {
            caster: player_guid,
            cast_id,
            spell_id,
            visual: spell_visual,
            target: SpellTargetData {
                flags: 0x2, // SpellCastTargetFlags::Unit
                unit: target_guid,
                item: ObjectGuid::EMPTY,
            },
            hit_targets: vec![target_guid],
        };
        self.send_packet(&go_pkt);

        // Aplicar efecto según type
        match spell_info.effect_type {
            6 => {
                // SPELL_EFFECT_HEAL
                let heal_amount = spell_info.effect_base_points as u32;
                self.apply_heal(target_guid, heal_amount).await?;
            }
            2 => {
                // SPELL_EFFECT_SCHOOL_DAMAGE
                let damage_amount = spell_info.effect_base_points as u32;
                self.apply_damage(target_guid, damage_amount).await?;
            }
            35 => {
                // SPELL_EFFECT_APPLY_AURA
                self.apply_aura(spell_id, player_guid, 30000, 0x00000001)?;
            }
            _ => {
                debug!("Spell effect type {} not yet implemented", spell_info.effect_type);
            }
        }

        // Set global cooldown
        self.last_spell_cast_time = Some(Instant::now());

        // Set per-spell cooldown
        self.last_spell_cast_time_per_spell.insert(spell_id, Instant::now());

        // Notify client so action bar shows the cooldown animation
        use wow_packet::packets::spell::CooldownEvent;
        self.send_packet(&CooldownEvent { spell_id, is_pet: false });

        Ok(())
    }

    /// Helper: apply heal to target (self or creature).
    async fn apply_heal(&mut self, target_guid: ObjectGuid, heal_amount: u32) -> Result<(), &'static str> {
        let player_guid = self.player_guid.ok_or("No player GUID")?;

        // Si target es el mismo jugador
        if target_guid == player_guid {
            info!(
                account = self.account_id,
                heal = heal_amount,
                "Healed self"
            );
            // TODO: Actualizar HP del jugador en la DB
            // self.player_health = min(self.player_health + heal_amount, self.player_max_health);
            // Enviar UpdateObject con VALUES update
            return Ok(());
        }

        // Si target es otra criatura/jugador
        if let Some(_creature) = self.creatures.get(&target_guid) {
            info!(
                account = self.account_id,
                creature = ?target_guid,
                heal = heal_amount,
                "Healed creature"
            );
            // TODO: Actualizar HP de la criatura
            return Ok(());
        }

        Err("Target not found")
    }

    /// Helper: apply damage to target creature.
    async fn apply_damage(&mut self, target_guid: ObjectGuid, damage_amount: u32) -> Result<(), &'static str> {
        let _player_guid = self.player_guid.ok_or("No player GUID")?;

        // Si target es otra criatura — end mutable borrow before calling async methods
        let kill_info: Option<(u32, wow_core::ObjectGuid)> = {
            if let Some(creature) = self.creatures.get_mut(&target_guid) {
                info!(
                    account = self.account_id,
                    creature = ?target_guid,
                    damage = damage_amount,
                    "Dealt damage to creature"
                );

                creature.hp = creature.hp.saturating_sub(damage_amount);

                if creature.hp == 0 {
                    info!("Creature {} (entry={}) killed", target_guid, creature.entry);
                    creature.state = wow_ai::CreatureState::Dead;
                    creature.is_alive = false;
                    Some((creature.entry, target_guid))
                } else {
                    None
                }
            } else {
                return Err("Target creature not found");
            }
        };

        // Process creature death outside the mutable borrow
        if let Some((entry, guid)) = kill_info {
            // Give XP for the kill
            let mob_level = self.creatures.get(&guid)
                .map(|c| c.level as u8)
                .unwrap_or(1);
            let xp = self.creature_kill_xp(mob_level);
            if xp > 0 {
                self.give_xp(xp, guid, true).await;
            }
            self.on_creature_killed(entry, guid).await;
        }

        Ok(())
    }
}

/// Current Unix timestamp (seconds since epoch).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Available race/class combinations from `class_expansion_requirement` table.
///
/// Data matches exactly what C# ObjectManager loads from the world DB.
/// ActiveExpansionLevel/AccountExpansionLevel: 0 for all except Death Knight (class 6)
/// which requires WotLK (active=2). MinActiveExpansionLevel is the minimum active
/// expansion across all races for that class.
fn default_available_classes() -> Vec<wow_packet::packets::auth::RaceClassAvailability> {
    use wow_packet::packets::auth::{ClassAvailability, RaceClassAvailability};

    // (race_id, &[(class_id, active_expansion_level, account_expansion_level)])
    let data: &[(u8, &[(u8, u8, u8)])] = &[
        (1,  &[(1,0,0),(2,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(8,0,0),(9,0,0)]),              // Human
        (2,  &[(1,0,0),(3,0,0),(4,0,0),(6,2,0),(7,0,0),(8,0,0),(9,0,0)]),                       // Orc
        (3,  &[(1,0,0),(2,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(7,0,0),(8,0,0),(9,0,0)]),       // Dwarf
        (4,  &[(1,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(8,0,0),(11,0,0)]),                       // Night Elf
        (5,  &[(1,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(8,0,0),(9,0,0)]),                       // Undead
        (6,  &[(1,0,0),(2,0,0),(3,0,0),(5,0,0),(6,2,0),(7,0,0),(11,0,0)]),                      // Tauren
        (7,  &[(1,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(8,0,0),(9,0,0)]),                       // Gnome
        (8,  &[(1,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(7,0,0),(8,0,0),(9,0,0),(11,0,0)]),      // Troll
        (10, &[(1,0,0),(2,0,0),(3,0,0),(4,0,0),(5,0,0),(6,2,0),(8,0,0),(9,0,0)]),                // Blood Elf
        (11, &[(1,0,0),(2,0,0),(3,0,0),(5,0,0),(6,2,0),(7,0,0),(8,0,0)]),                       // Draenei
    ];

    // MinActiveExpansionLevel per class = min across all races for that class
    // All classes have active=0 across all races except class 6 (DK) which is always 2
    let min_active = |class_id: u8| -> u8 {
        if class_id == 6 { 2 } else { 0 }
    };

    data.iter()
        .map(|&(race_id, classes)| RaceClassAvailability {
            race_id,
            classes: classes
                .iter()
                .map(|&(class_id, active_exp, account_exp)| ClassAvailability {
                    class_id,
                    active_expansion_level: active_exp,
                    account_expansion_level: account_exp,
                    min_active_expansion_level: min_active(class_id),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{
        BagFamilyMask, EnchantmentSlot, InventoryResult, InventoryType, ItemBondingType,
        ItemClass, ItemContext, ItemFieldFlags, ItemFlags, ItemFlags2, ItemUpdateState, ServerOpcodes,
        SpellItemEnchantmentFlags,
    };
    use wow_core::Position;
    use wow_data::{
        ItemAppearanceEntry, ItemAppearanceStore, ItemModifiedAppearanceEntry,
        ItemModifiedAppearanceStore, ItemRandomSuffixEntry, ItemRandomSuffixStore, ItemRecord,
        ItemSparseTemplateEntry, ItemStatsStore, ItemStore, SpellItemEnchantmentEntry,
        SpellItemEnchantmentStore,
    };
    use wow_entities::{
        AccessorObjectRef, BANK_SLOT_BAG_START, EQUIPMENT_SLOT_CHEST, INVENTORY_SLOT_BAG_START,
        REAGENT_BAG_SLOT_START, SendNewItemInstancePlan, SendNewItemModifier,
    };
    use wow_network::{GroupInfo, PlayerBroadcastInfo};
    use wow_packet::ServerPacket;

    fn make_session() -> (WorldSession, flume::Sender<WorldPacket>, flume::Receiver<Vec<u8>>) {
        let (pkt_tx, pkt_rx) = flume::bounded(100);
        let (send_tx, send_rx) = flume::bounded(100);

        let session = WorldSession::new(
            1,
            "TestAccount".into(),
            0,
            2,
            9, // account_expansion (raw from DB)
            54261,
            vec![0u8; 40],
            "esES".into(),
            pkt_rx,
            send_tx,
        );

        (session, pkt_tx, send_rx)
    }

    fn install_stackable_test_item_template(
        session: &mut WorldSession,
        entry: u32,
        max_stack_size: i32,
    ) {
        session.set_item_store(Arc::new(ItemStore::from_records([ItemRecord {
            id: entry,
            class_id: ItemClass::Consumable as u8,
            subclass_id: 0,
            material: 0,
            inventory_type: InventoryType::NonEquip as i8,
            sheathe_type: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            entry,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                stackable: max_stack_size,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                max_durability: 0,
                limit_category: 0,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));
    }

    fn send_new_item_plan(delivery: SendNewItemDelivery) -> SendNewItemPlan {
        SendNewItemPlan {
            player_guid: ObjectGuid::create_player(1, 42),
            item_guid: ObjectGuid::create_item(1, 500),
            item_entry: 9001,
            item_instance: SendNewItemInstancePlan {
                item_id: 9001,
                random_properties_seed: 456,
                random_properties_id: -77,
                modifications: vec![
                    SendNewItemModifier {
                        value: 123,
                        modifier_type: 3,
                    },
                    SendNewItemModifier {
                        value: 25,
                        modifier_type: 5,
                    },
                ],
            },
            slot: 4,
            slot_in_bag: 7,
            quest_log_item_id: 777,
            quantity: 3,
            quantity_in_inventory: 9,
            battle_pet_species_id: 123,
            battle_pet_breed_id: 0xBC,
            battle_pet_breed_quality: 0x1A,
            battle_pet_level: 25,
            pushed: true,
            created: false,
            display_text: SendNewItemDisplayText::EncounterLoot,
            dungeon_encounter_id: 615,
            is_encounter_loot: true,
            delivery,
        }
    }

    fn broadcast_info(guid: ObjectGuid, send_tx: flume::Sender<Vec<u8>>) -> PlayerBroadcastInfo {
        PlayerBroadcastInfo {
            map_id: 0,
            position: Position::new(0.0, 0.0, 0.0, 0.0),
            send_tx,
            player_name: format!("Player{}", guid.counter()),
            account_id: guid.counter() as u32,
            race: 1,
            class: 1,
            sex: 0,
            level: 1,
            display_id: 49,
            visible_items: [(0, 0, 0); 19],
        }
    }

    #[test]
    fn session_starts_authed() {
        let (session, _, _) = make_session();
        assert_eq!(session.state(), SessionState::Authed);
    }

    #[test]
    fn player_currency_helpers_match_cpp_storage_lookup() {
        let (mut session, _, _) = make_session();
        assert_eq!(session.player_currency_quantity(395), 0);
        assert!(!session.has_currency(395, 1));

        session.player_currencies.insert(
            395,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 42,
                weekly_quantity: 5,
                tracked_quantity: 6,
                increased_cap_quantity: 7,
                earned_quantity: 8,
                flags: 9,
            },
        );

        assert_eq!(session.player_currency_quantity(395), 42);
        assert!(session.has_currency(395, 42));
        assert!(!session.has_currency(395, 43));
    }

    fn currency_entry(id: u32) -> wow_data::CurrencyTypesEntry {
        wow_data::CurrencyTypesEntry {
            id,
            category_id: 0,
            inventory_icon_file_id: 0,
            spell_weight: 0,
            spell_category: 0,
            max_qty: 0,
            max_earnable_per_week: 0,
            quality: 0,
            faction_id: 0,
            award_condition_id: 0,
            flags: wow_constants::CurrencyTypesFlags::empty(),
            flags_b: wow_constants::CurrencyTypesFlagsB::empty(),
        }
    }

    #[test]
    fn player_currency_vendor_add_caps_and_marks_state_like_cpp() {
        let (mut session, _, _) = make_session();
        session.player_race = 1;
        session.set_currency_types_store(Arc::new(wow_data::CurrencyTypesStore::from_entries([
            wow_data::CurrencyTypesEntry {
                max_qty: 150,
                max_earnable_per_week: 120,
                flags: wow_constants::CurrencyTypesFlags::TRACK_QUANTITY,
                flags_b: wow_constants::CurrencyTypesFlagsB::USE_TOTAL_EARNED_FOR_EARNED,
                ..currency_entry(395)
            },
            currency_entry(396),
        ])));
        session.player_currencies.insert(
            395,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 90,
                weekly_quantity: 95,
                tracked_quantity: 4,
                increased_cap_quantity: 0,
                earned_quantity: 7,
                flags: 0,
            },
        );

        let delta = session.add_currency_vendor(395, 70).unwrap().unwrap();
        assert_eq!(delta.currency_id, 395);
        assert_eq!(delta.amount, 25);
        assert_eq!(delta.quantity, 115);
        assert_eq!(delta.weekly_quantity, Some(120));
        assert_eq!(delta.max_quantity, Some(150));
        assert_eq!(delta.total_earned, Some(32));
        assert_eq!(session.player_currency_quantity(395), 115);
        assert_eq!(
            session.player_currencies.get(&395).map(|currency| currency.state),
            Some(PlayerCurrencyState::Changed)
        );

        let delta = session.add_currency_vendor(396, 3).unwrap().unwrap();
        assert_eq!(delta.quantity, 3);
        assert_eq!(
            session.player_currencies.get(&396).map(|currency| currency.state),
            Some(PlayerCurrencyState::New)
        );
    }

    #[test]
    fn player_currency_item_refund_ignores_caps_and_total_counters_like_cpp() {
        let (mut session, _, _) = make_session();
        session.player_race = 1;
        session.set_currency_types_store(Arc::new(wow_data::CurrencyTypesStore::from_entries([
            wow_data::CurrencyTypesEntry {
                max_qty: 100,
                max_earnable_per_week: 50,
                flags: wow_constants::CurrencyTypesFlags::TRACK_QUANTITY,
                flags_b: wow_constants::CurrencyTypesFlagsB::USE_TOTAL_EARNED_FOR_EARNED,
                ..currency_entry(395)
            },
        ])));
        session.player_currencies.insert(
            395,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 95,
                weekly_quantity: 49,
                tracked_quantity: 11,
                increased_cap_quantity: 0,
                earned_quantity: 12,
                flags: 0,
            },
        );

        let delta = session.add_currency_item_refund(395, 20).unwrap().unwrap();
        assert_eq!(delta.currency_id, 395);
        assert_eq!(delta.amount, 20);
        assert_eq!(delta.quantity, 115);
        assert_eq!(delta.weekly_quantity, Some(49));
        assert_eq!(delta.max_quantity, Some(100));
        assert_eq!(delta.total_earned, Some(12));

        let currency = session.player_currencies.get(&395).unwrap();
        assert_eq!(currency.quantity, 115);
        assert_eq!(currency.weekly_quantity, 49);
        assert_eq!(currency.tracked_quantity, 11);
        assert_eq!(currency.earned_quantity, 12);
        assert_eq!(currency.state, PlayerCurrencyState::Changed);
    }

    #[test]
    fn player_currency_remove_and_save_state_match_cpp() {
        let (mut session, _, _) = make_session();
        session.set_currency_types_store(Arc::new(wow_data::CurrencyTypesStore::from_entries([
            currency_entry(395),
            currency_entry(396),
            currency_entry(397),
        ])));
        session.player_currencies.insert(
            395,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 42,
                weekly_quantity: 5,
                tracked_quantity: 6,
                increased_cap_quantity: 7,
                earned_quantity: 8,
                flags: 9,
            },
        );
        session.player_currencies.insert(
            396,
            PlayerCurrency {
                state: PlayerCurrencyState::New,
                quantity: 3,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 0,
                flags: 0,
            },
        );
        session.player_currencies.insert(
            397,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 10,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 0,
                flags: 0,
            },
        );

        assert!(session.remove_currency(395, 100));
        assert_eq!(session.player_currency_quantity(395), 0);
        assert_eq!(
            session.player_currencies.get(&395).map(|currency| currency.state),
            Some(PlayerCurrencyState::Changed)
        );
        assert!(!session.remove_currency(999, 1));

        let mut tx = SqlTransaction::new();
        session.append_player_currency_save_statements(&mut tx, 1);
        assert_eq!(tx.len(), 2);
        assert_eq!(
            session.player_currencies.get(&395).map(|currency| currency.state),
            Some(PlayerCurrencyState::Unchanged)
        );
        assert_eq!(
            session.player_currencies.get(&396).map(|currency| currency.state),
            Some(PlayerCurrencyState::Unchanged)
        );
        assert_eq!(
            session.player_currencies.get(&397).map(|currency| currency.state),
            Some(PlayerCurrencyState::Unchanged)
        );
    }

    #[test]
    fn update_empty_queue() {
        let (mut session, _, _) = make_session();
        let processed = session.update(100);
        assert_eq!(processed, 0);
    }

    #[test]
    fn update_processes_packets() {
        let (mut session, pkt_tx, _) = make_session();

        // Send some packets (they'll be logged as "no handler" but won't crash)
        for _ in 0..5 {
            let pkt = WorldPacket::from_bytes(&[0x00, 0x00]); // opcode 0
            pkt_tx.send(pkt).unwrap();
        }

        let processed = session.update(100);
        assert_eq!(processed, 5);
        assert_eq!(session.pending_packets.len(), 5);
    }

    #[test]
    fn kick_sets_disconnecting() {
        let (mut session, _, _) = make_session();
        session.kick("test");
        assert!(session.is_disconnecting());
    }

    #[test]
    fn disconnected_channel_sets_disconnecting() {
        let (mut session, pkt_tx, _) = make_session();
        drop(pkt_tx); // Close the channel

        session.update(100);
        assert!(session.is_disconnecting());
    }

    #[test]
    fn send_packet_works() {
        let (session, _, send_rx) = make_session();

        let pong = wow_packet::packets::auth::Pong { serial: 42 };
        session.send_packet(&pong);

        let data = send_rx.try_recv().unwrap();
        assert_eq!(data.len(), 6); // opcode(2) + serial(4)
    }

    #[test]
    fn send_equip_error_preserves_cpp_item_level_and_limit_fields() {
        let (session, _, send_rx) = make_session();
        let item1 = ObjectGuid::new(0, 0x0102);
        let item2 = ObjectGuid::new(0, 0x0506);
        let expected = InventoryChangeFailure::new(
            InventoryResult::CantEquipLevelI,
            item1,
            item2,
        )
        .with_level(42)
        .to_bytes();

        session.send_equip_error(
            InventoryResult::CantEquipLevelI,
            Some(item1),
            Some(item2),
            42,
            0,
        );
        assert_eq!(send_rx.try_recv().unwrap(), expected);

        let expected = InventoryChangeFailure::error(
            InventoryResult::ItemMaxLimitCategoryEquippedExceededIs,
        )
        .with_limit_category(777)
        .to_bytes();
        session.send_equip_error(
            InventoryResult::ItemMaxLimitCategoryEquippedExceededIs,
            None,
            None,
            0,
            777,
        );
        assert_eq!(send_rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn send_buy_and_sell_error_mirror_cpp_empty_vendor_fallback() {
        let (session, _, send_rx) = make_session();
        let vendor_guid = ObjectGuid::new(0, 0x0102);
        let item_guid = ObjectGuid::new(0, 0x0506);

        let expected = BuyFailed {
            vendor_guid,
            muid: 6948,
            reason: BuyResult::CantFindItem,
        }
        .to_bytes();
        session.send_buy_error(BuyResult::CantFindItem, Some(vendor_guid), 6948);
        assert_eq!(send_rx.try_recv().unwrap(), expected);

        let expected = SellResponse::error(
            ObjectGuid::EMPTY,
            item_guid,
            SellResult::YouDontOwnThatItem,
        )
        .to_bytes();
        session.send_sell_error(SellResult::YouDontOwnThatItem, None, item_guid);
        assert_eq!(send_rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn apply_enchantment_random_suffix_ref_uses_cpp_abs_lookup() {
        let (mut session, _, _) = make_session();
        session.set_item_random_suffix_store(Arc::new(ItemRandomSuffixStore::from_entries([
            ItemRandomSuffixEntry {
                id: 77,
                enchantments: [901, 900, 902, 0, 0],
                allocation_pct: [1_000, 2_000, 3_000, 0, 0],
            },
        ])));

        let suffix = session
            .apply_enchantment_random_suffix_ref(-77)
            .expect("random suffix should resolve by abs(RandomPropertiesID)");

        assert_eq!(suffix.id, 77);
        assert_eq!(suffix.enchantments, [901, 900, 902, 0, 0]);
        assert_eq!(suffix.allocation_pct, [1_000, 2_000, 3_000, 0, 0]);
        assert!(session.apply_enchantment_random_suffix_ref(0).is_none());
        assert!(session.apply_enchantment_random_suffix_ref(-78).is_none());
    }

    #[test]
    fn item_modified_appearance_helpers_use_cpp_lookup_shapes() {
        let (mut session, _, _) = make_session();
        session.set_item_appearance_store(Arc::new(ItemAppearanceStore::from_entries([
            ItemAppearanceEntry {
                id: 1000,
                display_type: 0,
                item_display_info_id: 555,
                default_icon_file_data_id: 0,
                ui_order: 0,
            },
            ItemAppearanceEntry {
                id: 1001,
                display_type: 0,
                item_display_info_id: 777,
                default_icon_file_data_id: 0,
                ui_order: 0,
            },
        ])));
        session.set_item_modified_appearance_store(Arc::new(
            ItemModifiedAppearanceStore::from_entries([
                ItemModifiedAppearanceEntry {
                    id: 10,
                    item_id: 100,
                    item_appearance_modifier_id: 0,
                    item_appearance_id: 1000,
                    order_index: 0,
                    transmog_source_type_enum: 0,
                },
                ItemModifiedAppearanceEntry {
                    id: 11,
                    item_id: 100,
                    item_appearance_modifier_id: 2,
                    item_appearance_id: 1001,
                    order_index: 0,
                    transmog_source_type_enum: 0,
                },
            ]),
        ));

        assert_eq!(session.item_modified_appearance_ref(11), Some((100, 2)));
        assert_eq!(session.item_modified_appearance_for_item(100, 2), Some(11));
        assert_eq!(session.item_modified_appearance_for_item(100, 9), Some(10));
        assert_eq!(session.item_modified_appearance_for_item(101, 0), None);
        assert_eq!(session.item_display_id(100, 2), Some(777));
        assert_eq!(session.item_display_id(100, 9), Some(555));
        assert_eq!(session.item_display_id(101, 0), None);
    }

    #[test]
    fn item_template_flags_use_item_sparse_flags_like_cpp() {
        let (mut session, _, _) = make_session();
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_parts(
            [],
            [
                (100, [ItemFlags::IS_BOUND_TO_ACCOUNT.bits() as u32, 0, 0, 0]),
                (101, [0, 0, 0, 0]),
            ],
        )));

        assert!(
            session
                .item_template_flags(100)
                .is_some_and(|flags| flags.contains(ItemFlags::IS_BOUND_TO_ACCOUNT))
        );
        assert!(session.is_item_bound_account_wide(100));
        assert!(!session.is_item_bound_account_wide(101));
        assert_eq!(session.item_template_flags(102), None);
    }

    #[test]
    fn item_storage_template_combines_basic_and_sparse_data_like_cpp() {
        let (mut session, _, _) = make_session();
        session.set_item_store(Arc::new(ItemStore::from_records([ItemRecord {
            id: 100,
            class_id: ItemClass::Container as u8,
            subclass_id: 0,
            material: 0,
            inventory_type: InventoryType::Weapon as i8,
            sheathe_type: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            100,
            ItemSparseTemplateEntry {
                flags: [
                    ItemFlags::IS_BOUND_TO_ACCOUNT.bits() as u32,
                    ItemFlags2::UsedInATradeskill as u32,
                    0,
                    0,
                ],
                bag_family: BagFamilyMask::HERBS.bits(),
                stackable: i32::MAX,
                max_count: 3,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 99,
                max_durability: 88,
                limit_category: 44,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: ItemBondingType::OnEquip as u8,
                container_slots: 16,
                inventory_type: InventoryType::Bag as i8,
            },
        )])));

        let template = session.item_storage_template(100).unwrap();

        assert_eq!(template.entry, 100);
        assert_eq!(template.class_id, ItemClass::Container);
        assert_eq!(template.subclass_id, 0);
        assert_eq!(template.inventory_type, InventoryType::Bag);
        assert_eq!(template.bonding, ItemBondingType::OnEquip);
        assert_eq!(template.bag_family, BagFamilyMask::HERBS);
        assert_eq!(template.max_stack_size, 0x7FFF_FFFE);
        assert_eq!(template.max_count, 3);
        assert_eq!(template.item_limit_category, 44);
        assert_eq!(template.container_slots, 16);
        assert_eq!(template.sell_price, 99);
        assert!(template.is_crafting_reagent);
        assert!(template.is_bound_account_wide());
        assert_eq!(session.item_template_inventory_type(100), Some(InventoryType::Bag as u8));
        assert_eq!(session.item_storage_template(101), None);
    }

    fn insert_open_item_bag_with_child(
        session: &mut WorldSession,
        player_guid: ObjectGuid,
        bag_slot: u8,
        inner_slot: u8,
    ) -> (ObjectGuid, ObjectGuid) {
        let bag_guid = ObjectGuid::create_item(1, 1001);
        session.inventory_items.insert(bag_slot, InventoryItem {
            guid: bag_guid,
            entry_id: 101,
            db_guid: 1001,
            inventory_type: Some(InventoryType::Bag as u8),
        });
        let bag_item = session.make_inventory_item_object(
            bag_guid, 101, player_guid, 1, 0, ItemContext::None, bag_slot,
        );
        session.insert_inventory_item_object(bag_item);

        let child_guid = ObjectGuid::create_item(1, 1002);
        let mut child = session.make_inventory_item_object(
            child_guid, 700, player_guid, 1, 0, ItemContext::None, inner_slot,
        );
        child.set_container_guid_and_slot(bag_guid, bag_slot);
        session.insert_inventory_item_object(child);

        (bag_guid, child_guid)
    }

    fn install_open_item_has_loot_template(session: &mut WorldSession, entry: u32) {
        install_open_item_has_loot_template_with_lock(session, entry, 0);
    }

    fn install_open_item_has_loot_template_with_lock(
        session: &mut WorldSession,
        entry: u32,
        lock_id: u16,
    ) {
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            entry,
            ItemSparseTemplateEntry {
                flags: [ItemFlags::HAS_LOOT.bits() as u32, 0, 0, 0],
                bag_family: 0,
                stackable: 1,
                max_count: 0,
                lock_id,
                required_reputation_rank: 0,
                sell_price: 0,
                max_durability: 0,
                limit_category: 0,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));
    }

    fn insert_open_item_top_level(
        session: &mut WorldSession,
        player_guid: ObjectGuid,
        slot: u8,
        item_guid: ObjectGuid,
        entry: u32,
        unlocked: bool,
    ) {
        session.inventory_items.insert(slot, InventoryItem {
            guid: item_guid,
            entry_id: entry,
            db_guid: item_guid.counter() as u64,
            inventory_type: None,
        });
        let mut item = session.make_inventory_item_object(
            item_guid, entry, player_guid, 1, 0, ItemContext::None, slot,
        );
        if unlocked {
            item.set_item_flag(ItemFieldFlags::UNLOCKED);
        }
        session.insert_inventory_item_object(item);
    }

    #[test]
    fn item_template_lock_id_uses_item_sparse_lock_id_like_cpp() {
        let (mut session, _, _) = make_session();
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            700,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                stackable: 1,
                max_count: 0,
                lock_id: 99,
                required_reputation_rank: 0,
                sell_price: 0,
                max_durability: 0,
                limit_category: 0,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));

        assert_eq!(session.item_template_lock_id(700), Some(99));
        assert_eq!(session.item_template_lock_id(701), None);
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_resolves_top_level_like_cpp() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));

        let top_guid = ObjectGuid::create_item(1, 900);
        session.inventory_items.insert(23, InventoryItem {
            guid: top_guid,
            entry_id: 700,
            db_guid: 900,
            inventory_type: None,
        });
        let top_item = session.make_inventory_item_object(
            top_guid, 700, player_guid, 1, 0, ItemContext::None, 23,
        );
        session.insert_inventory_item_object(top_item);

        assert_eq!(
            session.get_inventory_item_by_pos(INVENTORY_SLOT_BAG_0, 23).map(|i| i.guid),
            Some(top_guid)
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_excludes_buyback_top_level_like_cpp() {
        let (mut session, _, _) = make_session();
        session.buyback_items.insert(BUYBACK_SLOT_START, InventoryItem {
            guid: ObjectGuid::create_item(1, 901),
            entry_id: 701,
            db_guid: 901,
            inventory_type: None,
        });

        assert!(
            session
                .get_inventory_item_by_pos(INVENTORY_SLOT_BAG_0, BUYBACK_SLOT_START)
                .is_none()
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_resolves_nested_carried_bag_like_cpp() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        let (_, child_guid) = insert_open_item_bag_with_child(
            &mut session,
            player_guid,
            INVENTORY_SLOT_BAG_START,
            5,
        );

        assert_eq!(
            session.get_inventory_item_by_pos(INVENTORY_SLOT_BAG_START, 5).map(|i| i.guid),
            Some(child_guid)
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_resolves_nested_bank_bag_like_cpp() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        let (_, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, BANK_SLOT_BAG_START, 5);

        assert_eq!(
            session.get_inventory_item_by_pos(BANK_SLOT_BAG_START, 5).map(|i| i.guid),
            Some(child_guid)
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_resolves_nested_reagent_bag_like_cpp() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        let (_, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, REAGENT_BAG_SLOT_START, 5);

        assert_eq!(
            session
                .get_inventory_item_by_pos(REAGENT_BAG_SLOT_START, 5)
                .map(|i| i.guid),
            Some(child_guid)
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_missing_bag_or_empty_slot_is_missing() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        insert_open_item_bag_with_child(&mut session, player_guid, INVENTORY_SLOT_BAG_START, 5);

        assert!(
            session
                .get_inventory_item_by_pos(INVENTORY_SLOT_BAG_START + 1, 0)
                .is_none()
        );
        assert!(
            session
                .get_inventory_item_by_pos(INVENTORY_SLOT_BAG_START, 3)
                .is_none()
        );
    }

    #[test]
    fn open_item_nested_item_preserves_top_level_bag_slot_and_inner_slot() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        let (bag_guid, child_guid) = insert_open_item_bag_with_child(
            &mut session,
            player_guid,
            INVENTORY_SLOT_BAG_START,
            5,
        );

        let child = session.inventory_item_objects.get(&child_guid).unwrap();
        assert_eq!(child.container_guid(), bag_guid);
        assert_eq!(child.bag_slot(), INVENTORY_SLOT_BAG_START);
        assert_eq!(child.slot(), 5);
        assert_eq!(child.position(), u16::from(INVENTORY_SLOT_BAG_START) << 8 | 5);
    }

    async fn assert_open_item_nested_has_loot_opens_without_internal_bag_error(bag_slot: u8) {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template(&mut session, 700);
        let (_, child_guid) = insert_open_item_bag_with_child(&mut session, player_guid, bag_slot, 5);

        session
            .handle_open_item(WorldPacket::from_bytes(&[bag_slot, 5]))
            .await;

        let sent = send_rx.try_recv().unwrap();
        let opcode = u16::from_le_bytes([sent[0], sent[1]]);
        assert_eq!(opcode, ServerOpcodes::LootResponse as u16);
        assert_ne!(opcode, ServerOpcodes::InventoryChangeFailure as u16);
        assert!(session.loot_table.contains_key(&child_guid));
        assert!(
            session
                .inventory_item_objects
                .get(&child_guid)
                .is_some_and(|item| item.loot_generated())
        );
    }

    #[tokio::test]
    async fn open_item_nested_has_loot_opens_without_internal_bag_error() {
        assert_open_item_nested_has_loot_opens_without_internal_bag_error(INVENTORY_SLOT_BAG_START)
            .await;
    }

    #[tokio::test]
    async fn open_item_nested_bank_bag_has_loot_opens_without_internal_bag_error() {
        assert_open_item_nested_has_loot_opens_without_internal_bag_error(BANK_SLOT_BAG_START)
            .await;
    }

    #[tokio::test]
    async fn open_item_nested_reagent_bag_has_loot_opens_without_internal_bag_error() {
        assert_open_item_nested_has_loot_opens_without_internal_bag_error(REAGENT_BAG_SLOT_START)
            .await;
    }

    #[tokio::test]
    async fn open_item_locked_container_returns_item_locked_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 900);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 700, false);

        session
            .handle_open_item(WorldPacket::from_bytes(&[INVENTORY_SLOT_BAG_0, 23]))
            .await;

        let sent = send_rx.try_recv().unwrap();
        assert_eq!(
            sent,
            InventoryChangeFailure::new(InventoryResult::ItemLocked, item_guid, ObjectGuid::EMPTY)
                .to_bytes()
        );
        assert!(!session.loot_table.contains_key(&item_guid));
        assert!(
            session
                .inventory_item_objects
                .get(&item_guid)
                .is_some_and(|item| !item.loot_generated())
        );
    }

    #[tokio::test]
    async fn open_item_unlocked_locked_template_continues_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 901);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 700, true);

        session
            .handle_open_item(WorldPacket::from_bytes(&[INVENTORY_SLOT_BAG_0, 23]))
            .await;

        let sent = send_rx.try_recv().unwrap();
        let opcode = u16::from_le_bytes([sent[0], sent[1]]);
        assert_eq!(opcode, ServerOpcodes::LootResponse as u16);
        assert!(session.loot_table.contains_key(&item_guid));
        assert!(
            session
                .inventory_item_objects
                .get(&item_guid)
                .is_some_and(|item| item.loot_generated())
        );
    }

    #[tokio::test]
    async fn open_item_missing_runtime_object_fails_closed_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 902);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        session.inventory_items.insert(23, InventoryItem {
            guid: item_guid,
            entry_id: 700,
            db_guid: item_guid.counter() as u64,
            inventory_type: None,
        });
        assert!(!session.inventory_item_objects.contains_key(&item_guid));

        session
            .handle_open_item(WorldPacket::from_bytes(&[INVENTORY_SLOT_BAG_0, 23]))
            .await;

        let sent = send_rx.try_recv().unwrap();
        assert_eq!(
            sent,
            InventoryChangeFailure::new(InventoryResult::ItemLocked, item_guid, ObjectGuid::EMPTY)
                .to_bytes()
        );
        assert!(!session.loot_table.contains_key(&item_guid));
    }

    fn assert_open_item_release_destroy_nested_item_leaves_container_in_place(bag_slot: u8) {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        let (bag_guid, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, bag_slot, 5);
        let child = session.inventory_item_objects.get(&child_guid).unwrap();
        let child_bag = child.bag_slot();
        let child_slot = child.slot();

        let inv = session.get_inventory_item_by_pos(child_bag, child_slot);
        assert!(inv.is_some());
        assert_eq!(inv.unwrap().guid, child_guid);

        session.remove_fully_looted_runtime_item(child_bag, child_slot, child_guid);
        assert!(session.get_inventory_item_by_pos(child_bag, child_slot).is_none());
        assert!(!session.inventory_item_objects.contains_key(&child_guid));
        assert!(session.inventory_items.contains_key(&bag_slot));
        assert_eq!(session.inventory_items[&bag_slot].guid, bag_guid);
    }

    #[test]
    fn open_item_release_destroy_nested_item_leaves_container_in_place() {
        assert_open_item_release_destroy_nested_item_leaves_container_in_place(INVENTORY_SLOT_BAG_START);
    }

    #[test]
    fn open_item_release_destroy_nested_bank_bag_item_leaves_container_in_place() {
        assert_open_item_release_destroy_nested_item_leaves_container_in_place(BANK_SLOT_BAG_START);
    }

    #[test]
    fn direct_destroy_uses_cpp_can_unequip_gate_for_equipment_and_bags() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        session.set_item_store(Arc::new(ItemStore::from_records([
            ItemRecord {
                id: 100,
                class_id: ItemClass::Armor as u8,
                subclass_id: 0,
                material: 0,
                inventory_type: InventoryType::Chest as i8,
                sheathe_type: 0,
            },
            ItemRecord {
                id: 101,
                class_id: ItemClass::Container as u8,
                subclass_id: 0,
                material: 0,
                inventory_type: InventoryType::Bag as i8,
                sheathe_type: 0,
            },
        ])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([
            (
                100,
                ItemSparseTemplateEntry {
                    flags: [0, 0, 0, 0],
                    bag_family: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 0,
                    max_durability: 0,
                    limit_category: 0,
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    bonding: 0,
                    container_slots: 0,
                    inventory_type: InventoryType::Chest as i8,
                },
            ),
            (
                101,
                ItemSparseTemplateEntry {
                    flags: [0, 0, 0, 0],
                    bag_family: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 0,
                    max_durability: 0,
                    limit_category: 0,
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    bonding: 0,
                    container_slots: 16,
                    inventory_type: InventoryType::Bag as i8,
                },
            ),
        ])));

        let chest_guid = ObjectGuid::create_item(1, 1000);
        session.inventory_items.insert(EQUIPMENT_SLOT_CHEST, InventoryItem {
            guid: chest_guid,
            entry_id: 100,
            db_guid: 1000,
            inventory_type: Some(InventoryType::Chest as u8),
        });
        let chest_item = session.make_inventory_item_object(
            chest_guid,
            100,
            player_guid,
            1,
            0,
            ItemContext::None,
            EQUIPMENT_SLOT_CHEST,
        );
        session.insert_inventory_item_object(chest_item);
        let chest_proto = session.item_storage_template(100);
        session.in_combat = true;
        assert_eq!(
            session.can_destroy_direct_item_like_cpp(
                EQUIPMENT_SLOT_CHEST,
                session.inventory_item_objects.get(&chest_guid),
                chest_proto.as_ref(),
                false,
            ),
            InventoryResult::NotInCombat
        );
        session.in_combat = false;

        let bag_guid = ObjectGuid::create_item(1, 1001);
        session.inventory_items.insert(INVENTORY_SLOT_BAG_START, InventoryItem {
            guid: bag_guid,
            entry_id: 101,
            db_guid: 1001,
            inventory_type: Some(InventoryType::Bag as u8),
        });
        let bag_item = session.make_inventory_item_object(
            bag_guid,
            101,
            player_guid,
            1,
            0,
            ItemContext::None,
            INVENTORY_SLOT_BAG_START,
        );
        session.insert_inventory_item_object(bag_item);
        let child_guid = ObjectGuid::create_item(1, 1002);
        let mut child = session.make_inventory_item_object(
            child_guid,
            100,
            player_guid,
            1,
            0,
            ItemContext::None,
            0,
        );
        child.set_container_guid_and_slot(bag_guid, 0);
        session.insert_inventory_item_object(child);

        let bag_proto = session.item_storage_template(101);
        assert!(session.direct_item_contains_items(bag_guid));
        assert_eq!(
            session.can_destroy_direct_item_like_cpp(
                INVENTORY_SLOT_BAG_START,
                session.inventory_item_objects.get(&bag_guid),
                bag_proto.as_ref(),
                session.direct_item_contains_items(bag_guid),
            ),
            InventoryResult::DestroyNonemptyBag
        );
    }

    #[test]
    fn active_loot_guid_tracks_cpp_loot_target_guid_comparisons() {
        let (mut session, _, _) = make_session();
        let loot_guid = ObjectGuid::create_item(1, 700);
        let other_guid = ObjectGuid::create_item(1, 701);

        assert!(!session.is_active_loot_guid(loot_guid));
        session.set_active_loot_guid(loot_guid);
        assert!(session.is_active_loot_guid(loot_guid));
        assert!(!session.is_active_loot_guid(other_guid));

        session.clear_active_loot_guid_if(other_guid);
        assert!(session.is_active_loot_guid(loot_guid));
        session.clear_active_loot_guid_if(loot_guid);
        assert!(!session.is_active_loot_guid(loot_guid));
    }

    #[test]
    fn inventory_item_object_uses_template_durability_and_runtime_fields() {
        let (mut session, _, _) = make_session();
        let owner_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 900);
        session.total_played_time = 123;
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            700,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                stackable: 1,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                max_durability: 55,
                limit_category: 0,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: 0,
                container_slots: 0,
                inventory_type: 0,
            },
        )])));

        let mut item = session.make_inventory_item_object(
            item_guid,
            700,
            owner_guid,
            3,
            44,
            ItemContext::Vendor,
            35,
        );
        item.set_state(ItemUpdateState::Unchanged);
        session.insert_inventory_item_object(item);

        let stored = session.inventory_item_objects.get(&item_guid).unwrap();
        assert_eq!(stored.object().entry(), 700);
        assert_eq!(stored.data().owner, owner_guid);
        assert_eq!(stored.data().contained_in, owner_guid);
        assert_eq!(stored.data().stack_count, 3);
        assert_eq!(stored.data().max_durability, 55);
        assert_eq!(stored.data().durability, 44);
        assert_eq!(stored.data().context, ItemContext::Vendor as i32);
        assert_eq!(stored.slot(), 35);
        assert_eq!(stored.update_state(), ItemUpdateState::Unchanged);

        session.set_inventory_item_object_slot(item_guid, 36);
        assert_eq!(session.inventory_item_objects.get(&item_guid).unwrap().slot(), 36);
        assert!(session.remove_inventory_item_object(item_guid).is_some());
        assert!(!session.inventory_item_objects.contains_key(&item_guid));
    }

    #[test]
    fn object_accessor_sync_exposes_session_inventory_items_like_cpp() {
        let (mut session, _, _) = make_session();
        let accessor = new_shared_object_accessor();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 900);

        session.set_object_accessor(Arc::clone(&accessor));
        session.set_player_guid(Some(player_guid));
        session.player_name = Some("Jaina".into());
        session.player_position = Some(Position::new(1.0, 2.0, 3.0, 0.0));
        session.current_map_id = 571;
        session.inventory_items.insert(23, InventoryItem {
            guid: item_guid,
            entry_id: 700,
            db_guid: 900,
            inventory_type: None,
        });
        let item = session.make_inventory_item_object(
            item_guid,
            700,
            player_guid,
            2,
            0,
            ItemContext::None,
            23,
        );
        session.insert_inventory_item_object(item);
        session.sync_object_accessor_player();

        {
            let accessor = accessor.read();
            let player = accessor.find_connected_player(player_guid).unwrap();
            match accessor.get_object_ref_by_type_mask(player, item_guid, TypeMask::ITEM) {
                Some(AccessorObjectRef::Item(item)) => {
                    assert_eq!(item.object().guid(), item_guid);
                    assert_eq!(item.slot(), 23);
                    assert_eq!(item.count(), 2);
                }
                other => panic!("expected item ref, got {other:?}"),
            }
        }

        let moved = session.inventory_items.remove(&23).unwrap();
        session.inventory_items.insert(24, moved);
        session.set_inventory_item_object_slot(item_guid, 24);
        session.sync_object_accessor_player();
        {
            let accessor = accessor.read();
            let item = accessor.player_item(player_guid, item_guid).unwrap();
            assert_eq!(item.slot(), 24);
        }

        session.inventory_items.remove(&24);
        session.remove_inventory_item_object(item_guid);
        session.sync_object_accessor_player();
        assert!(accessor.read().player_item(player_guid, item_guid).is_none());

        session.cleanup_shared_runtime_state();
        let accessor = accessor.read();
        assert!(accessor.find_connected_player(player_guid).is_none());
        assert!(session.inventory_items.is_empty());
        assert!(session.inventory_item_objects.is_empty());
    }

    #[test]
    fn direct_inventory_store_plan_uses_cpp_can_store_merge_then_empty_order() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 900);
        session.set_player_guid(Some(player_guid));
        session.set_item_store(Arc::new(ItemStore::from_records([ItemRecord {
            id: 700,
            class_id: ItemClass::Consumable as u8,
            subclass_id: 0,
            material: 0,
            inventory_type: InventoryType::NonEquip as i8,
            sheathe_type: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            700,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                stackable: 20,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                max_durability: 0,
                limit_category: 0,
                required_reputation_faction: 0,
                allowable_class: -1,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));

        session.inventory_items.insert(35, InventoryItem {
            guid: item_guid,
            entry_id: 700,
            db_guid: 900,
            inventory_type: None,
        });
        let item = session.make_inventory_item_object(
            item_guid,
            700,
            player_guid,
            18,
            0,
            ItemContext::None,
            35,
        );
        session.insert_inventory_item_object(item);

        let (result, dest, no_space) = session
            .plan_store_new_direct_inventory_item(700, 5)
            .expect("player snapshot should exist");

        assert_eq!(result, InventoryResult::Ok);
        assert_eq!(no_space, None);
        assert_eq!(dest.len(), 2);
        assert_eq!(
            dest[0],
            ItemPosCount::new((u16::from(INVENTORY_SLOT_BAG_0) << 8) | 35, 2)
        );
        assert_eq!(
            dest[1],
            ItemPosCount::new((u16::from(INVENTORY_SLOT_BAG_0) << 8) | 36, 3)
        );
    }

    #[test]
    fn direct_inventory_store_plan_respects_cpp_explicit_empty_slot() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        install_stackable_test_item_template(&mut session, 700, 20);

        let (result, dest, no_space) = session
            .plan_store_new_direct_inventory_item_at(700, 5, INVENTORY_SLOT_BAG_0, 36)
            .expect("player snapshot should exist");

        assert_eq!(result, InventoryResult::Ok);
        assert_eq!(no_space, None);
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                (u16::from(INVENTORY_SLOT_BAG_0) << 8) | 36,
                5,
            )]
        );
    }

    #[test]
    fn direct_inventory_store_plan_respects_cpp_explicit_stack_before_other_merge() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        install_stackable_test_item_template(&mut session, 700, 20);

        for (slot, db_guid) in [(35, 900_u64), (36, 901_u64)] {
            let item_guid = ObjectGuid::create_item(1, db_guid as i64);
            session.inventory_items.insert(slot, InventoryItem {
                guid: item_guid,
                entry_id: 700,
                db_guid,
                inventory_type: None,
            });
            let item = session.make_inventory_item_object(
                item_guid,
                700,
                player_guid,
                18,
                0,
                ItemContext::None,
                slot,
            );
            session.insert_inventory_item_object(item);
        }

        let (result, dest, no_space) = session
            .plan_store_new_direct_inventory_item_at(700, 3, INVENTORY_SLOT_BAG_0, 36)
            .expect("player snapshot should exist");

        assert_eq!(result, InventoryResult::Ok);
        assert_eq!(no_space, None);
        assert_eq!(
            dest,
            vec![
                ItemPosCount::new((u16::from(INVENTORY_SLOT_BAG_0) << 8) | 36, 2),
                ItemPosCount::new((u16::from(INVENTORY_SLOT_BAG_0) << 8) | 35, 1),
            ]
        );
    }

    #[test]
    fn spell_item_enchantment_helpers_use_cpp_store_fields() {
        let (mut session, _, _) = make_session();
        session.set_spell_item_enchantment_store(Arc::new(
            SpellItemEnchantmentStore::from_entries([SpellItemEnchantmentEntry {
                id: 900,
                effect_arg: [7, 8, 9],
                effect_points_min: [10, -2, 30],
                item_visual: 44,
                flags: SpellItemEnchantmentFlags::ALLOW_ENTERING_ARENA,
                required_skill_id: 333,
                required_skill_rank: 75,
                item_level: 11,
                charges: 0,
                effect: [
                    ItemEnchantmentType::Resistance as u8,
                    ItemEnchantmentType::Stat as u8,
                    250,
                ],
                condition_id: 12,
                min_level: 20,
                max_level: 0,
            }])),
        );

        assert!(session.is_arena_allowed_enchantment(900));
        assert!(!session.is_arena_allowed_enchantment(901));

        let template = session
            .apply_enchantment_template_ref(900, 80, false)
            .expect("template should resolve from SpellItemEnchantment.db2");
        assert_eq!(template.enchantment_id, 900);
        assert_eq!(template.condition_id, 12);
        assert!(!template.condition_fits);
        assert_eq!(template.min_level, 20);
        assert_eq!(template.required_skill_id, 333);
        assert_eq!(template.required_skill_rank, 75);
        assert_eq!(template.required_skill_value, 80);

        assert_eq!(
            session.apply_enchantment_effect_refs(900).unwrap(),
            [
                ApplyEnchantmentEffectRef::known(ItemEnchantmentType::Resistance, 10, 7),
                ApplyEnchantmentEffectRef::known(ItemEnchantmentType::Stat, (-2i16) as u32, 8),
                ApplyEnchantmentEffectRef::unknown(250, 30, 9),
            ]
        );
    }

    #[test]
    fn send_new_item_plan_maps_entity_fields_to_item_push_result_like_cpp() {
        let plan = send_new_item_plan(SendNewItemDelivery::Direct);
        let packet = WorldSession::item_push_result_from_send_new_item_plan(&plan);

        assert_eq!(packet.player_guid, plan.player_guid);
        assert_eq!(packet.item_guid, plan.item_guid);
        assert_eq!(packet.slot, 4);
        assert_eq!(packet.slot_in_bag, 7);
        assert_eq!(packet.quest_log_item_id, 777);
        assert_eq!(packet.quantity, 3);
        assert_eq!(packet.quantity_in_inventory, 9);
        assert_eq!(packet.dungeon_encounter_id, 615);
        assert_eq!(packet.display_text, ItemPushResultDisplayType::EncounterLoot);
        assert!(packet.pushed);
        assert!(!packet.created);
        assert!(!packet.is_bonus_roll);
        assert!(packet.is_encounter_loot);
        assert_eq!(packet.item.item_id, 9001);
        assert_eq!(packet.item.random_properties_seed, 456);
        assert_eq!(packet.item.random_properties_id, -77);
        assert!(packet.item.item_bonus.is_none());
        assert_eq!(
            packet.item.modifications.values,
            vec![ItemMod::new(123, 3), ItemMod::new(25, 5)]
        );
    }

    #[test]
    fn send_new_item_plan_direct_sends_item_push_result_to_session() {
        let (session, _, send_rx) = make_session();
        let plan = send_new_item_plan(SendNewItemDelivery::Direct);
        let expected = WorldSession::item_push_result_from_send_new_item_plan(&plan).to_bytes();

        session.send_new_item_plan(&plan);

        assert_eq!(send_rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn send_new_item_plan_group_broadcasts_to_group_members_including_self() {
        let (mut session, _, send_rx) = make_session();
        let self_guid = ObjectGuid::create_player(1, 42);
        let other_guid = ObjectGuid::create_player(1, 43);
        let (self_tx, self_rx) = flume::bounded(10);
        let (other_tx, other_rx) = flume::bounded(10);
        let player_registry = Arc::new(PlayerRegistry::default());
        player_registry.insert(self_guid, broadcast_info(self_guid, self_tx));
        player_registry.insert(other_guid, broadcast_info(other_guid, other_tx));
        let group_registry = Arc::new(GroupRegistry::default());
        let mut group = GroupInfo::new(self_guid);
        group.add_member(other_guid);
        let group_guid = group.group_guid;
        group_registry.insert(group_guid, group);
        session.player_guid = Some(self_guid);
        session.group_guid = Some(group_guid);
        session.set_player_registry(player_registry);
        session.set_group_registry(group_registry, Arc::new(PendingInvites::default()));
        let plan = send_new_item_plan(SendNewItemDelivery::GroupBroadcast);
        let expected = WorldSession::item_push_result_from_send_new_item_plan(&plan).to_bytes();

        session.send_new_item_plan(&plan);

        assert_eq!(self_rx.try_recv().unwrap(), expected);
        assert_eq!(other_rx.try_recv().unwrap(), expected);
        assert!(send_rx.try_recv().is_err());
    }

    #[test]
    fn send_item_time_update_plan_sends_cpp_packet() {
        let (session, _, send_rx) = make_session();
        let update = PlayerItemTimeUpdate {
            item_guid: ObjectGuid::new(0, 0x0102),
            expiration: 300,
        };
        let expected = ItemTimeUpdate {
            item_guid: update.item_guid,
            duration_left: update.expiration,
        }
        .to_bytes();

        session.send_item_time_update_plan(&update);

        assert_eq!(send_rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn send_item_enchant_time_update_plan_sends_cpp_packet() {
        let (session, _, send_rx) = make_session();
        let owner_guid = ObjectGuid::new(0, 0x0102);
        let update = PlayerEnchantTimeUpdate {
            item_guid: ObjectGuid::new(0, 0x0506),
            slot: EnchantmentSlot::EnhancementSocket,
            duration_secs: 45,
        };
        let expected = ItemEnchantTimeUpdate {
            owner_guid,
            item_guid: update.item_guid,
            duration_left: update.duration_secs,
            slot: update.slot as u32,
        }
        .to_bytes();

        session.send_item_enchant_time_update_plan(owner_guid, &update);

        assert_eq!(send_rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn state_transitions() {
        let (mut session, _, _) = make_session();
        assert_eq!(session.state(), SessionState::Authed);

        session.set_state(SessionState::LoggedIn);
        assert_eq!(session.state(), SessionState::LoggedIn);

        session.set_state(SessionState::Transfer);
        assert_eq!(session.state(), SessionState::Transfer);
    }

    #[test]
    fn legit_characters_management() {
        let (mut session, _, _) = make_session();

        let guid1 = ObjectGuid::create_player(1, 1);
        let guid2 = ObjectGuid::create_player(1, 2);
        let guid3 = ObjectGuid::create_player(1, 3);

        session.set_legit_characters(vec![guid1, guid2, guid3]);
        assert!(session.is_legit_character(&guid1));
        assert!(session.is_legit_character(&guid2));
        assert!(!session.is_legit_character(&ObjectGuid::create_player(1, 99)));

        session.remove_legit_character(&guid2);
        assert!(!session.is_legit_character(&guid2));
        assert!(session.is_legit_character(&guid1));
    }

    #[test]
    fn char_db_and_realm_id() {
        let (mut session, _, _) = make_session();

        assert!(session.char_db().is_none());
        assert_eq!(session.realm_id(), 1);

        session.set_realm_id(5);
        assert_eq!(session.realm_id(), 5);
    }

    #[test]
    fn dispatch_metadata_matches_cpp_for_touched_opcodes() {
        let (session, _, _) = make_session();
        let table = &session.dispatch_table;

        let cases = [
            (
                ClientOpcodes::AreaTrigger,
                SessionStatus::LoggedIn,
                PacketProcessing::Inplace,
            ),
            (
                ClientOpcodes::SetSelection,
                SessionStatus::LoggedIn,
                PacketProcessing::ThreadUnsafe,
            ),
            (
                ClientOpcodes::TaxiNodeStatusQuery,
                SessionStatus::LoggedIn,
                PacketProcessing::ThreadSafe,
            ),
            (
                ClientOpcodes::TimeSyncResponse,
                SessionStatus::LoggedIn,
                PacketProcessing::ThreadSafe,
            ),
            (
                ClientOpcodes::TimeSyncResponseDropped,
                SessionStatus::LoggedIn,
                PacketProcessing::ThreadSafe,
            ),
            (
                ClientOpcodes::TimeSyncResponseFailed,
                SessionStatus::LoggedIn,
                PacketProcessing::ThreadSafe,
            ),
            (
                ClientOpcodes::TrainerList,
                SessionStatus::LoggedIn,
                PacketProcessing::Inplace,
            ),
            (
                ClientOpcodes::TrainerBuySpell,
                SessionStatus::LoggedIn,
                PacketProcessing::Inplace,
            ),
        ];

        for (opcode, status, processing) in cases {
            let entry = table
                .get(&opcode)
                .unwrap_or_else(|| panic!("missing dispatch entry for {opcode:?}"));
            assert_eq!(entry.status, status, "{opcode:?} status");
            assert_eq!(entry.processing, processing, "{opcode:?} processing");
        }
    }

    #[test]
    fn dispatch_table_has_no_duplicate_registered_opcodes() {
        let mut counts = std::collections::HashMap::new();
        for entry in inventory::iter::<PacketHandlerEntry> {
            *counts.entry(entry.opcode).or_insert(0usize) += 1;
        }

        let duplicates: Vec<_> = counts
            .into_iter()
            .filter_map(|(opcode, count)| (count > 1).then_some((opcode, count)))
            .collect();

        assert!(duplicates.is_empty(), "duplicate handlers: {duplicates:?}");
    }
}
