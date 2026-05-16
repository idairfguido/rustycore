// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! `WorldSession` — per-player session that receives packets from the
//! [`WorldSocket`](wow_network::WorldSocket) and dispatches them to handlers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use parking_lot::RwLock;
use rand::seq::SliceRandom;
use tracing::{debug, info, trace, warn};

use crate::entity_update_bridge::player_values_update_to_update_object;
use crate::map_manager::{WorldMMapPathRequestLikeCpp, WorldMMapPathfinderWorkerLikeCpp};
use crate::phasing::{
    init_db_phase_shift_like_cpp, init_db_visible_map_id_like_cpp,
    party_member_phase_states_like_cpp,
};
use wow_constants::item::{CurrencyTypes, CurrencyTypesFlags};
use wow_constants::movement::MovementFlag;
use wow_constants::unit::{Gender, Team, UnitFlags, UnitStandStateType, WeaponAttackType};
use wow_constants::{
    BagFamilyMask, BuyResult, ClientOpcodes, InventoryResult, InventoryType, ItemBondingType,
    ItemClass, ItemContext, ItemEnchantmentType, ItemFlags, ItemFlags2, ItemQuality, SellResult,
    TypeId, TypeMask, UnitState,
};
use wow_core::{ObjectGuid, ObjectGuidGenerator, Position};
use wow_data::{
    AreaTableStore, AreaTriggerStore, ChrSpecializationStore, ConditionEntriesByTypeStore,
    CreatureDisplayInfoStore, CreatureModelDataStore, CreatureTemplateMountStoreLikeCpp,
    CurrencyTypesEntry, CurrencyTypesStore, DISABLE_TYPE_MAP, DisableMgrLikeCpp,
    DisableWorldObjectRefLikeCpp, DungeonEncounterStore, GameObjectDisplayInfoStore,
    HotfixBlobCache, ImportPriceStores, ItemAppearanceStore, ItemClassStore, ItemCurrencyCostStore,
    ItemDisenchantLootStore, ItemExtendedCostStore, ItemModifiedAppearanceStore,
    ItemPriceBaseStore, ItemRandomEnchantmentTemplateStore, ItemRandomPropertiesStore,
    ItemRandomPropertyTemplateEntry, ItemRandomSuffixStore, ItemStatsStore, ItemStore, LockStore,
    MapDifficultyStore, MapDifficultyXConditionStore, MapStore, MountCapabilityStore, MountStore,
    MountTypeXCapabilityStore, MountXDisplayStore, PhaseGroupStore, PhaseStore,
    PlayerConditionAuraLikeCpp, PlayerConditionContextLikeCpp, PlayerConditionCountLikeCpp,
    PlayerConditionPartyStatusLikeCpp, PlayerConditionQuestKillLikeCpp,
    PlayerConditionReputationLikeCpp, PlayerConditionSkillLikeCpp, PlayerConditionStore,
    PlayerStatsStore, RandPropPointsStore, SkillStore, SpellItemEnchantmentStore, SpellMiscStore,
    SpellRangeStore, SpellStore, VEHICLE_SEAT_FLAG_CAN_ATTACK, VehicleAccessoryStoreLikeCpp,
    VehicleSeatStore, VehicleStore, VehicleTemplateStoreLikeCpp,
    is_player_meeting_condition_like_cpp,
};
use wow_database::{
    CharStatements, CharacterDatabase, LoginDatabase, PreparedStatement, SqlTransaction,
    StatementDef, WorldDatabase,
};
use wow_entities::{
    AccessorObjectKind, ApplyEnchantmentEffectRef, ApplyEnchantmentRandomSuffixRef,
    ApplyEnchantmentTemplateRef, BANK_SLOT_BAG_END, BANK_SLOT_BAG_START, BUYBACK_SLOT_COUNT,
    BUYBACK_SLOT_END, BUYBACK_SLOT_START, CanStoreItemArgs, CanUnequipItemArgs,
    EQUIPMENT_SLOT_MAINHAND, GameObject, INVENTORY_DEFAULT_SIZE, INVENTORY_SLOT_BAG_0,
    INVENTORY_SLOT_BAG_END, INVENTORY_SLOT_BAG_START, Item, ItemCreateInfo, ItemPosCount,
    ItemSlotRef, ItemStorageRef, ItemStorageTemplate, MAX_ITEM_SPELLS, NULL_BAG, NULL_SLOT,
    ObjectAccessor, PLAYER_SLOT_END, PhaseShift, Player, PlayerEnchantTimeUpdate,
    PlayerInventoryStorage, PlayerItemTimeUpdate, REAGENT_BAG_SLOT_END, REAGENT_BAG_SLOT_START,
    SendNewItemDelivery, SendNewItemDisplayText, SendNewItemPlan, Vehicle, VehicleAccessory,
    VisibleItemValues, WorldObject, is_bag_pos, is_equipment_packed_pos, make_item_pos,
};
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus, build_dispatch_table};
use wow_loot::LootStores;
use wow_network::session_mgr::{InstanceLink, SessionManager};
use wow_network::{
    GroupRegistry, LootDropRatesLikeCpp, PendingInvites, PlayerBroadcastInfo, PlayerRegistry,
    SessionCommand,
};
use wow_packet::packets::item::{
    InventoryChangeFailure, ItemEnchantTimeUpdate, ItemInstance, ItemMod, ItemModList,
    ItemPushResult, ItemPushResultDisplayType, ItemTimeUpdate,
};
use wow_packet::packets::misc::{AccountMount, AccountMountUpdate, BuyFailed, SellResponse};
use wow_recastdetour::PathQueryFilterContext;

const PLAYER_FLAGS_UBER_LIKE_CPP: u32 = 0x0008_0000;
const ATTACK_DISPLAY_DELAY_LIKE_CPP_MS: u32 = 200;

fn rounded_median_u32(sorted_values: &[u32]) -> u32 {
    debug_assert!(!sorted_values.is_empty());
    let mid = sorted_values.len() / 2;
    if sorted_values.len() % 2 == 1 {
        sorted_values[mid]
    } else {
        ((f64::from(sorted_values[mid - 1]) + f64::from(sorted_values[mid])) / 2.0).round() as u32
    }
}

fn set_active_player_update_bit_like_cpp(mask: &mut [u32; 48], bit: usize) {
    mask[bit / 32] |= 1 << (bit % 32);
}
use wow_packet::{ClientPacket, WorldPacket};

/// Maximum number of packets processed per `update()` call.
const MAX_PACKETS_PER_UPDATE: usize = 100;
const TRANSFER_ABORT_MAP_NOT_ALLOWED_LIKE_CPP: u32 = 16;
const MAP_BATTLEGROUND_LIKE_CPP: i8 = 3;
const MAP_ARENA_LIKE_CPP: i8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MMapRuntimeConfigLikeCpp {
    pub data_dir: String,
    pub enabled: bool,
    pub disabled_map_ids: HashSet<u32>,
}

impl Default for MMapRuntimeConfigLikeCpp {
    fn default() -> Self {
        Self {
            data_dir: "./Data".to_string(),
            enabled: true,
            disabled_map_ids: HashSet::new(),
        }
    }
}

impl MMapRuntimeConfigLikeCpp {
    pub fn pathfinding_enabled_for_map_like_cpp(&self, map_id: u32) -> bool {
        self.enabled && !self.disabled_map_ids.contains(&map_id)
    }

    pub fn should_try_pathfinding_like_cpp(
        &self,
        map_id: u32,
        owner_ignores_pathfinding: bool,
    ) -> bool {
        self.pathfinding_enabled_for_map_like_cpp(map_id) && !owner_ignores_pathfinding
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepresentedLootRollVote {
    pub vote: u8,
    pub roll_number: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct RepresentedLootRollState {
    pub loot_obj: ObjectGuid,
    pub loot_list_id: u8,
    pub end_time: Instant,
    pub voters: HashMap<ObjectGuid, RepresentedLootRollVote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepresentedGameObjectUseEffect {
    TriggerGameEvent {
        gameobject_guid: ObjectGuid,
        player_guid: ObjectGuid,
        event_id: u32,
    },
    TriggerLinkedTrap {
        gameobject_guid: ObjectGuid,
        player_guid: ObjectGuid,
        trap_entry: u32,
    },
    CastSpell {
        gameobject_guid: ObjectGuid,
        player_guid: ObjectGuid,
        spell_id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepresentedPendingBind {
    pub instance_id: u32,
    pub time_until_lock_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RepresentedGameObjectUseState {
    pub loot_state: Option<wow_entities::LootState>,
    pub loot_state_unit_guid: wow_core::ObjectGuid,
    pub owner_guid: Option<wow_core::ObjectGuid>,
    pub go_state: Option<wow_entities::GoState>,
    pub dynamic_flags: u32,
    pub despawn_delay_secs: Option<u32>,
    pub per_player_despawn_secs: Option<u32>,
    pub per_player_despawn_until: Option<Instant>,
    pub personal_loot_uses: u32,
    pub chest_restock_time_secs: Option<u32>,
    pub chest_consumable: Option<bool>,
    pub chest_personal_loot_id: Option<u32>,
    pub map_id: Option<u16>,
    pub position: Option<wow_core::Position>,
    pub display_id: Option<u32>,
    pub scale: f32,
    pub rotation: [f32; 4],
    pub go_type: Option<u8>,
    pub interact_radius_override: Option<u32>,
    pub lock_id: Option<u32>,
    pub fishing_hole_max_opens: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MovementAckEventLikeCpp {
    pub opcode: ClientOpcodes,
    pub mover_guid: ObjectGuid,
    pub ack_index: Option<i32>,
    pub movement_force_id: Option<ObjectGuid>,
    pub movement_force_type: Option<u8>,
    pub adjusted_time: Option<u32>,
    pub speed: Option<f32>,
    pub time_skipped: Option<u32>,
    pub spline_id: Option<i32>,
    pub accepted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveSplineDoneTaxiActionLikeCpp {
    InvalidMovement,
    InProgressNoFlightGenerator,
    InProgressNoTeleport,
    TeleportRequested,
    FinalCleanup,
    IgnoredUnexpectedFinalPath,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RepresentedTaxiFlightNodeLikeCpp {
    pub map_id: u16,
    pub position: wow_core::Position,
    pub teleport_flag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MoveSplineDoneTaxiEventLikeCpp {
    pub spline_id: i32,
    pub action: MoveSplineDoneTaxiActionLikeCpp,
    pub destination_node_id: Option<u32>,
    pub teleport_map_id: Option<u16>,
    pub teleport_position: Option<wow_core::Position>,
    pub honorless_target_cast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveTeleportAckActionLikeCpp {
    NotBeingTeleportedNear,
    WrongMover,
    MissingDestination,
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MoveTeleportAckEventLikeCpp {
    pub mover_guid: ObjectGuid,
    pub ack_index: i32,
    pub move_time: i32,
    pub action: MoveTeleportAckActionLikeCpp,
    pub destination_map_id: Option<u16>,
    pub destination_position: Option<wow_core::Position>,
    pub old_zone_id: Option<u32>,
    pub new_zone_id: Option<u32>,
    pub new_area_id: Option<u32>,
    pub honorless_target_cast: bool,
    pub pvp_disabled: bool,
    pub pet_resummon_requested: bool,
    pub delayed_operations_processed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RepresentedTaxiFlightStateLikeCpp {
    current_node: RepresentedTaxiFlightNodeLikeCpp,
    node_after_teleport: Option<RepresentedTaxiFlightNodeLikeCpp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MovementSpeedAckActionLikeCpp {
    Accepted,
    SkippedPending,
    Corrected,
    Kicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitMoveTypeLikeCpp {
    Walk = 0,
    Run = 1,
    RunBack = 2,
    Swim = 3,
    SwimBack = 4,
    TurnRate = 5,
    Flight = 6,
    FlightBack = 7,
    PitchRate = 8,
}

impl UnitMoveTypeLikeCpp {
    pub(crate) const COUNT: usize = 9;

    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MovementSpeedAckEventLikeCpp {
    pub opcode: ClientOpcodes,
    pub move_type: Option<UnitMoveTypeLikeCpp>,
    pub ack_speed: f32,
    pub expected_speed: Option<f32>,
    pub remaining_forced_changes: Option<u8>,
    pub action: MovementSpeedAckActionLikeCpp,
}

const PLAYER_BASE_MOVE_SPEED_LIKE_CPP: [f32; UnitMoveTypeLikeCpp::COUNT] = [
    2.5,      // MOVE_WALK
    7.0,      // MOVE_RUN
    4.5,      // MOVE_RUN_BACK
    4.722222, // MOVE_SWIM
    2.5,      // MOVE_SWIM_BACK
    3.141594, // MOVE_TURN_RATE
    7.0,      // MOVE_FLIGHT
    4.5,      // MOVE_FLIGHT_BACK
    3.14,     // MOVE_PITCH_RATE
];

impl Default for RepresentedGameObjectUseState {
    fn default() -> Self {
        Self {
            loot_state: None,
            loot_state_unit_guid: wow_core::ObjectGuid::EMPTY,
            owner_guid: None,
            go_state: None,
            dynamic_flags: 0,
            despawn_delay_secs: None,
            per_player_despawn_secs: None,
            per_player_despawn_until: None,
            personal_loot_uses: 0,
            chest_restock_time_secs: None,
            chest_consumable: None,
            chest_personal_loot_id: None,
            map_id: None,
            position: None,
            display_id: None,
            scale: 1.0,
            rotation: [0.0, 0.0, 0.0, 1.0],
            go_type: None,
            interact_radius_override: None,
            lock_id: None,
            fishing_hole_max_opens: None,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepresentedLootRollCriteriaEvent {
    RollAnyNeed {
        player_guid: ObjectGuid,
        quantity: u32,
    },
    RollAnyGreed {
        player_guid: ObjectGuid,
        quantity: u32,
    },
    RollNeed {
        player_guid: ObjectGuid,
        item_id: u32,
        roll_number: u8,
    },
    RollGreed {
        player_guid: ObjectGuid,
        item_id: u32,
        roll_number: u8,
    },
    Disenchant {
        player_guid: ObjectGuid,
        spell_id: u32,
    },
}

pub type SharedObjectAccessor = Arc<RwLock<ObjectAccessor>>;
pub(crate) const SKILL_RIDING_LIKE_CPP: u16 = 762;
pub(crate) const LIQUID_MAP_IN_WATER_LIKE_CPP: u32 = 0x0000_0004;
pub(crate) const LIQUID_MAP_UNDER_WATER_LIKE_CPP: u32 = 0x0000_0008;
pub type SharedCanonicalMapManager = Arc<Mutex<wow_map::MapManager>>;

pub fn new_shared_object_accessor() -> SharedObjectAccessor {
    Arc::new(RwLock::new(ObjectAccessor::default()))
}

fn gender_from_u8(value: u8) -> Gender {
    match value {
        1 => Gender::Female,
        2 => Gender::None,
        _ => Gender::Male,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SessionPlayerController {
    guid: ObjectGuid,
    name: String,
    position: wow_core::Position,
    map_id: u16,
    race: u8,
    class: u8,
    level: u8,
    gender: u8,
    gold: u64,
    xp: u32,
    next_level_xp: u32,
    selection_guid: Option<ObjectGuid>,
    known_spells: Vec<i32>,
    skill_values: HashMap<u16, u16>,
    currencies: HashMap<u32, PlayerCurrency>,
    inventory: SessionPlayerInventoryRuntime,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionPlayerInventoryRuntime {
    inventory_items: HashMap<u8, InventoryItem>,
    buyback_items: HashMap<u8, InventoryItem>,
    buyback_price: [u32; BUYBACK_SLOT_COUNT],
    buyback_timestamp: [i64; BUYBACK_SLOT_COUNT],
    current_buyback_slot: u8,
    item_objects: HashMap<ObjectGuid, Item>,
}

impl Default for SessionPlayerInventoryRuntime {
    fn default() -> Self {
        Self {
            inventory_items: HashMap::new(),
            buyback_items: HashMap::new(),
            buyback_price: [0; BUYBACK_SLOT_COUNT],
            buyback_timestamp: [0; BUYBACK_SLOT_COUNT],
            current_buyback_slot: BUYBACK_SLOT_START,
            item_objects: HashMap::new(),
        }
    }
}

impl SessionPlayerController {
    pub(crate) fn new(
        guid: ObjectGuid,
        name: String,
        position: wow_core::Position,
        map_id: u16,
        race: u8,
        class: u8,
        level: u8,
        gender: u8,
    ) -> Self {
        Self {
            guid,
            name,
            position,
            map_id,
            race,
            class,
            level,
            gender,
            gold: 0,
            xp: 0,
            next_level_xp: 400,
            selection_guid: None,
            known_spells: Vec::new(),
            skill_values: HashMap::new(),
            currencies: HashMap::new(),
            inventory: SessionPlayerInventoryRuntime::default(),
        }
    }

    pub(crate) fn guid(&self) -> ObjectGuid {
        self.guid
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn position(&self) -> wow_core::Position {
        self.position
    }

    pub(crate) fn map_id(&self) -> u16 {
        self.map_id
    }

    pub(crate) fn race(&self) -> u8 {
        self.race
    }

    pub(crate) fn class(&self) -> u8 {
        self.class
    }

    pub(crate) fn level(&self) -> u8 {
        self.level
    }

    pub(crate) fn gender(&self) -> u8 {
        self.gender
    }

    pub(crate) fn gold(&self) -> u64 {
        self.gold
    }

    pub(crate) fn xp(&self) -> u32 {
        self.xp
    }

    pub(crate) fn next_level_xp(&self) -> u32 {
        self.next_level_xp
    }

    #[allow(dead_code)]
    pub(crate) fn selection_guid(&self) -> Option<ObjectGuid> {
        self.selection_guid
    }

    pub(crate) fn known_spells(&self) -> &[i32] {
        &self.known_spells
    }

    pub(crate) fn skill_values(&self) -> &HashMap<u16, u16> {
        &self.skill_values
    }

    pub(crate) fn currencies(&self) -> &HashMap<u32, PlayerCurrency> {
        &self.currencies
    }

    pub(crate) fn inventory(&self) -> &SessionPlayerInventoryRuntime {
        &self.inventory
    }

    fn inventory_mut(&mut self) -> &mut SessionPlayerInventoryRuntime {
        &mut self.inventory
    }

    fn set_map_position(&mut self, map_id: u16, position: wow_core::Position) {
        self.map_id = map_id;
        self.position = position;
    }

    fn set_level(&mut self, level: u8) {
        self.level = level;
    }

    fn set_gold(&mut self, gold: u64) {
        self.gold = gold;
    }

    fn set_xp(&mut self, xp: u32) {
        self.xp = xp;
    }

    fn set_next_level_xp(&mut self, xp: u32) {
        self.next_level_xp = xp;
    }

    fn set_selection_guid(&mut self, guid: Option<ObjectGuid>) {
        self.selection_guid = guid;
    }

    fn set_known_spells(&mut self, spells: Vec<i32>) {
        self.known_spells = spells;
    }

    fn set_skill_values(&mut self, skill_values: HashMap<u16, u16>) {
        self.skill_values = skill_values;
    }

    fn learn_spell(&mut self, spell_id: i32) {
        if !self.known_spells.contains(&spell_id) {
            self.known_spells.push(spell_id);
        }
    }

    fn set_currencies(&mut self, currencies: HashMap<u32, PlayerCurrency>) {
        self.currencies = currencies;
    }

    fn set_inventory(&mut self, inventory: SessionPlayerInventoryRuntime) {
        self.inventory = inventory;
    }
}

impl SessionPlayerInventoryRuntime {
    pub(crate) fn inventory_items(&self) -> &HashMap<u8, InventoryItem> {
        &self.inventory_items
    }

    pub(crate) fn buyback_items(&self) -> &HashMap<u8, InventoryItem> {
        &self.buyback_items
    }

    pub(crate) fn buyback_price(&self) -> &[u32; BUYBACK_SLOT_COUNT] {
        &self.buyback_price
    }

    pub(crate) fn buyback_timestamp(&self) -> &[i64; BUYBACK_SLOT_COUNT] {
        &self.buyback_timestamp
    }

    pub(crate) fn current_buyback_slot(&self) -> u8 {
        self.current_buyback_slot
    }

    pub(crate) fn item_objects(&self) -> &HashMap<ObjectGuid, Item> {
        &self.item_objects
    }
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
    battlenet_account_id: u32,
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

    // Cross-session commands executed by this session's own update loop.
    session_command_tx: flume::Sender<SessionCommand>,
    session_command_rx: flume::Receiver<SessionCommand>,

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

    // Import price stores (ImportPrice*.db2 data)
    import_price_stores: Option<Arc<ImportPriceStores>>,

    // Item class store (ItemClass.db2 data)
    item_class_store: Option<Arc<ItemClassStore>>,

    // Item currency cost store (ItemCurrencyCost.db2 data)
    item_currency_cost_store: Option<Arc<ItemCurrencyCostStore>>,

    // Item extended cost store (ItemExtendedCost.db2 data)
    item_extended_cost_store: Option<Arc<ItemExtendedCostStore>>,

    // Item store (Item.db2 BasicData — class/subclass)
    item_store: Option<Arc<ItemStore>>,

    // Item appearance store (ItemAppearance.db2 data)
    item_appearance_store: Option<Arc<ItemAppearanceStore>>,

    // Item modified appearance store (ItemModifiedAppearance.db2 data)
    item_modified_appearance_store: Option<Arc<ItemModifiedAppearanceStore>>,

    // Item price base store (ItemPriceBase.db2 data)
    item_price_base_store: Option<Arc<ItemPriceBaseStore>>,

    // Player level stats store (race/class/level → base stats)
    player_stats: Option<Arc<PlayerStatsStore>>,

    // Item stat modifiers store (item_id → stat bonuses from ItemSparse.db2)
    item_stats_store: Option<Arc<ItemStatsStore>>,

    // Item random suffix store (ItemRandomSuffix.db2 data)
    item_random_suffix_store: Option<Arc<ItemRandomSuffixStore>>,

    // Item random properties store (ItemRandomProperties.db2 data)
    item_random_properties_store: Option<Arc<ItemRandomPropertiesStore>>,

    // RandPropPoints store (RandPropPoints.db2 data)
    rand_prop_points_store: Option<Arc<RandPropPointsStore>>,

    // item_random_enchantment_template rows grouped like C++ ItemEnchantmentMgr.
    item_random_enchantment_template_store: Option<Arc<ItemRandomEnchantmentTemplateStore>>,

    // ItemDisenchantLoot store (ItemDisenchantLoot.db2 data)
    item_disenchant_loot_store: Option<Arc<ItemDisenchantLootStore>>,

    // C++ LootTemplates_* store foundation.
    loot_stores: Option<Arc<LootStores>>,

    // C++ ConditionMgr condition store loaded from world.conditions.
    condition_store: Option<Arc<ConditionEntriesByTypeStore>>,

    // C++ PlayerCondition.db2 store used by ConditionMgr player-condition checks.
    player_condition_store: Option<Arc<PlayerConditionStore>>,

    // C++ DisableMgr store loaded from world.disables.
    disable_mgr: Option<Arc<DisableMgrLikeCpp>>,

    // Lock store (Lock.db2 data)
    lock_store: Option<Arc<LockStore>>,

    // Spell item enchantment store (SpellItemEnchantment.db2 data)
    spell_item_enchantment_store: Option<Arc<SpellItemEnchantmentStore>>,

    // Hotfix blob cache: raw DB2 record bytes for DBReply responses
    hotfix_blob_cache: Option<Arc<HotfixBlobCache>>,

    // Skill store (auto-learned spells from SkillLineAbility.db2 + SkillRaceClassInfo.db2)
    skill_store: Option<Arc<SkillStore>>,

    // Area table store (area hierarchy + mount flags)
    area_table_store: Option<Arc<AreaTableStore>>,

    // Area trigger store (collision detection + teleportation)
    area_trigger_store: Option<Arc<AreaTriggerStore>>,

    // ChrSpecialization store (loot specialization validation)
    chr_specialization_store: Option<Arc<ChrSpecializationStore>>,

    // DungeonEncounter store (instance encounter lock/loot metadata)
    dungeon_encounter_store: Option<Arc<DungeonEncounterStore>>,

    // Map stores (Map.db2 + MapDifficulty.db2)
    map_store: Option<Arc<MapStore>>,
    map_difficulty_store: Option<Arc<MapDifficultyStore>>,
    map_difficulty_x_condition_store: Option<Arc<MapDifficultyXConditionStore>>,
    creature_template_mount_store: Option<Arc<CreatureTemplateMountStoreLikeCpp>>,
    creature_display_info_store: Option<Arc<CreatureDisplayInfoStore>>,
    gameobject_display_info_store: Option<Arc<GameObjectDisplayInfoStore>>,
    creature_model_data_store: Option<Arc<CreatureModelDataStore>>,
    mount_store: Option<Arc<MountStore>>,
    mount_capability_store: Option<Arc<MountCapabilityStore>>,
    mount_type_x_capability_store: Option<Arc<MountTypeXCapabilityStore>>,
    mount_x_display_store: Option<Arc<MountXDisplayStore>>,
    vehicle_store: Option<Arc<VehicleStore>>,
    vehicle_seat_store: Option<Arc<VehicleSeatStore>>,
    vehicle_template_store: Option<Arc<VehicleTemplateStoreLikeCpp>>,
    vehicle_accessory_store: Option<Arc<VehicleAccessoryStoreLikeCpp>>,
    terrain_swap_store: Option<Arc<wow_data::TerrainSwapStore>>,
    phase_store: Option<Arc<PhaseStore>>,
    phase_group_store: Option<Arc<PhaseGroupStore>>,

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
    pub(crate) pass_on_group_loot: bool,
    pub(crate) represented_enchanting_skill: u16,
    player_skill_values_like_cpp: HashMap<u16, u16>,

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
    /// Time sync requests sent to the client: sequence index -> server send time.
    pub(crate) time_sync_pending_requests: HashMap<u32, u32>,
    /// Last Trinity-style clock delta samples, `(clock_delta, round_trip_duration)`.
    pub(crate) time_sync_clock_delta_queue: VecDeque<(i64, u32)>,
    /// Server-client clock delta used to translate movement times.
    pub(crate) time_sync_clock_delta: i64,

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
    player_gold: u64,
    player_xp: u32,
    /// XP required to reach next level, cached from player_xp_for_level.
    player_next_level_xp: u32,
    /// Currently selected target GUID (SetSelection).
    selection_guid: Option<wow_core::ObjectGuid>,

    /// GUID of the character currently logged in (set after login completes).
    player_guid: Option<ObjectGuid>,
    /// Attached player controller, mirroring C++ `WorldSession::_player` ownership.
    player_controller: Option<SessionPlayerController>,

    /// Pending creature spawn request (set during login, processed async).
    pub(crate) pending_creature_spawn: Option<PendingCreatureSpawn>,
    /// Creatures waiting to respawn after corpse despawn.
    pub(crate) respawn_queue: Vec<PendingRespawn>,

    /// In-memory inventory: slot → (item ObjectGuid, entry_id, db_guid).
    inventory_items: HashMap<u8, InventoryItem>,

    /// In-memory buyback slots, kept separate from normal inventory like C++ `GetItemByGuid`.
    buyback_items: HashMap<u8, InventoryItem>,
    buyback_price: [u32; BUYBACK_SLOT_COUNT],
    buyback_timestamp: [i64; BUYBACK_SLOT_COUNT],
    current_buyback_slot: u8,

    /// C++ `_currencyStorage`, keyed by CurrencyTypes.db2 ID.
    player_currencies: HashMap<u32, PlayerCurrency>,

    /// In-memory item objects keyed by item GUID, mirroring C++ `Player::m_items` ownership.
    inventory_item_objects: HashMap<ObjectGuid, Item>,

    /// Current map ID for VALUES update packets.
    current_map_id: u16,

    /// Race of the currently logged-in character (set at login).
    player_race: u8,
    /// Class of the currently logged-in character (set at login).
    player_class: u8,
    /// Level of the currently logged-in character (set at login).
    player_level: u8,
    /// Gender of the currently logged-in character (set at login).
    player_gender: u8,
    /// C++ ActivePlayerData::LootSpecID represented session state.
    loot_specialization_id: u32,
    /// All known spell IDs for the logged-in character (DB + DBC merged).
    known_spells: Vec<i32>,
    /// C++ `CollectionMgr::_mounts` represented account mount collection.
    account_mounts_like_cpp: HashMap<i32, u8>,

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
    player_position: Option<wow_core::Position>,
    /// Last accepted player movement flags, mirroring C++ `Unit::m_movementInfo`.
    player_movement_flags_like_cpp: MovementFlag,
    /// Last terrain liquid status, mirroring C++ `WorldObject::m_liquidStatus`.
    player_liquid_status_like_cpp: u32,

    /// Cached character name for chat messages.
    player_name: Option<String>,

    // Addon chat filtering state. Mirrors C++ WorldSession::_registeredAddonPrefixes
    // and _filterAddonMessages.
    pub(crate) registered_addon_prefixes: Vec<String>,
    pub(crate) filter_addon_messages: bool,

    // ── Creature AI tracking ──────────────────────────────────────
    /// Tick counter for creature movement (throttle to every N ticks).
    pub(crate) creature_tick: u32,
    #[cfg(test)]
    pub(crate) creatures: std::collections::HashMap<wow_core::ObjectGuid, wow_ai::CreatureAI>,
    /// Per-session finite vendor stock state, mirroring Creature::m_vendorItemCounts
    /// until vendor ownership moves into the shared creature model.
    pub(crate) vendor_item_counts: HashMap<(wow_core::ObjectGuid, u32), VendorItemCount>,

    /// Shared, server-wide map state. When `Some`, creature reads/writes can
    /// route through here so all sessions on the same map see the same world.
    /// `None` until the world server injects the manager (see `set_map_manager`).
    pub(crate) map_manager: Option<crate::map_manager::SharedMapManager>,
    /// Canonical C++-style `wow-map` manager. This is injected separately from
    /// the legacy `wow-world` manager while handlers migrate to `wow-map`.
    pub(crate) canonical_map_manager: Option<SharedCanonicalMapManager>,
    /// Dedicated Detour owner handle. The underlying `MMapManager` remains on
    /// its worker thread because Detour state is not `Send + Sync`.
    mmap_pathfinder_like_cpp: Option<Arc<WorldMMapPathfinderWorkerLikeCpp>>,
    /// Shared C++ `InstanceLockMgr` analogue used by raid-info and instance entry paths.
    pub(crate) instance_lock_mgr: Option<Arc<std::sync::RwLock<wow_instances::InstanceLockMgr>>>,

    // ── Combat state ─────────────────────────────────────────────
    /// Current auto-attack target (None if not in combat).
    pub(crate) combat_target: Option<wow_core::ObjectGuid>,
    /// Last represented player melee tick used to decrement C++ `m_attackTimer`.
    combat_tick_last_at_like_cpp: Instant,
    /// Represented result of C++ `IsWithinLOSInMap(victim)` for melee swings until LOS runtime is canonical.
    player_melee_los_to_target_like_cpp: Option<bool>,

    /// True when the player is engaged in combat.
    pub(crate) in_combat: bool,
    /// Represented `Player::IsAlive()` state for handler guards that need C++ ordering.
    player_alive_like_cpp: bool,
    /// Represented `Player::IsGameMaster()` movement/fall guard.
    player_game_master_like_cpp: bool,
    /// Represented `CHEAT_GOD` movement/fall guard.
    player_cheat_god_like_cpp: bool,
    /// Represented `IsImmunedToDamage(SPELL_SCHOOL_MASK_NORMAL)` fall guard.
    player_normal_damage_immune_like_cpp: bool,
    /// Represented `IsImmuneToEnvironmentalDamage()` guard inside EnvironmentalDamage.
    player_environmental_damage_immune_like_cpp: bool,
    /// Represented player health used by movement/environmental side effects.
    player_health_like_cpp: u32,
    /// Represented player max health used by movement/environmental side effects.
    player_max_health_like_cpp: u32,
    /// Represented `Unit::m_movementInfo.time` for client movement ACK side effects.
    player_movement_time_like_cpp: u32,
    /// C++ `Player::m_lastFallTime`.
    last_fall_time_like_cpp: u32,
    /// C++ `Player::m_lastFallZ`.
    last_fall_z_like_cpp: f32,
    /// Recorded fall damage events until combat log/update packet runtime is complete.
    fall_damage_events_like_cpp: Vec<MovementFallDamageEvent>,
    /// C++ `PLAYER_FLAGS_IS_OUT_OF_BOUNDS` represented state.
    player_out_of_bounds_like_cpp: bool,
    /// Recorded `DAMAGE_FALL_TO_VOID` events until environmental damage packets are complete.
    under_map_damage_events_like_cpp: Vec<MovementUnderMapDamageEvent>,
    /// Represented stand state used by movement side effects until UnitData owns it.
    player_stand_state_like_cpp: UnitStandStateType,
    /// Count of C++ temporary pet unsummon side effects requested by movement.
    temporary_pet_unsummon_requests_like_cpp: u32,
    /// Count of C++ jump proc side effects requested by movement.
    movement_jump_proc_requests_like_cpp: u32,
    /// Represented `ActivePlayerData::LocalFlags`.
    active_player_local_flags_like_cpp: u32,
    /// Represented `ActivePlayerData::TransportServerTime`.
    active_player_transport_server_time_like_cpp: i32,
    /// Count of visibility refreshes requested by movement initialization.
    movement_visibility_refresh_requests_like_cpp: u32,
    /// ACKs accepted by represented movement handling until full Unit movement runtime/broadcasts exist.
    movement_ack_events_like_cpp: Vec<MovementAckEventLikeCpp>,
    /// Represented `PlayerTaxi::m_TaxiDestinations` until PlayerTaxi/MotionMaster runtime is canonical.
    taxi_destinations_like_cpp: Vec<u32>,
    /// Minimal TaxiNodes.db2 map lookup used by represented `MoveSplineDone` taxi transitions.
    taxi_node_map_ids_like_cpp: HashMap<u32, u16>,
    /// Represented active `FlightPathMovementGenerator`, if any.
    taxi_flight_state_like_cpp: Option<RepresentedTaxiFlightStateLikeCpp>,
    /// Represented unit flags touched by `CleanupAfterTaxiFlight`.
    taxi_unit_flags_like_cpp: UnitFlags,
    /// Represented mount state touched by `CleanupAfterTaxiFlight`.
    taxi_mounted_like_cpp: bool,
    /// Represented `Unit::SetMountDisplayId` until UnitData owns live player fields.
    player_mount_display_id_like_cpp: i32,
    /// Represented vehicle id selected from mount creature template until VehicleKit exists.
    player_mount_vehicle_id_like_cpp: u32,
    /// Represented C++ `Unit::m_vehicleKit` for player mounts until Unit owns live vehicle state.
    player_mount_vehicle_kit_like_cpp: Option<Vehicle>,
    /// Vehicle accessory rows selected by C++ `Vehicle::InstallAllAccessories(false)`.
    player_mount_vehicle_accessories_like_cpp: Vec<VehicleAccessory>,
    /// Represented number of VehicleSeat rows installed by C++ `Vehicle` constructor.
    player_mount_vehicle_seat_count_like_cpp: u8,
    /// Represented C++ `Vehicle::UsableSeatNum`.
    player_mount_vehicle_usable_seat_count_like_cpp: u8,
    /// Represented current `VehicleSeatEntry::Flags` for C++ `HandleAttackSwingOpcode`.
    player_vehicle_seat_flags_like_cpp: Option<i32>,
    /// Represented current pet GUID until player-owned pet runtime is canonical.
    represented_pet_guid_like_cpp: Option<ObjectGuid>,
    /// Represented current pet react state for C++ mount/dismount PetMode side effects.
    represented_pet_react_state_like_cpp: u8,
    /// Represented current pet command state for C++ mount/dismount PetMode side effects.
    represented_pet_command_state_like_cpp: u8,
    /// C++ `Player::m_temporaryPetReactState` saved by `DisablePetControlsOnMount`.
    temporary_mount_pet_react_state_like_cpp: Option<u8>,
    /// Count of C++ `CreateVehicleKit` mount side effects represented until Vehicle runtime sends packets.
    mount_vehicle_create_requests_like_cpp: u32,
    /// Count of C++ `RemoveVehicleKit` mount side effects represented until Vehicle runtime sends packets.
    mount_vehicle_remove_requests_like_cpp: u32,
    /// Count of C++ `SendOnCancelExpectedVehicleRideAura` packets emitted after vehicle-kit creation.
    mount_cancel_expected_vehicle_aura_packets_like_cpp: u32,
    /// Count of C++ `DisablePetControlsOnMount` side effects represented until pet runtime is canonical.
    mount_pet_control_disable_requests_like_cpp: u32,
    /// Count of C++ `EnablePetControlsOnDismount` side effects represented until pet runtime is canonical.
    mount_pet_control_enable_requests_like_cpp: u32,
    /// Count of C++ mount/dismount pet resummon calls represented until pet runtime is canonical.
    mount_pet_resummon_requests_like_cpp: u32,
    /// Count of C++ mount collision-height updates represented until movement packets are canonical.
    mount_collision_height_update_requests_like_cpp: u32,
    /// C++ `Unit::m_movementCounter` equivalent for vehicle-rec movement control packets.
    mount_vehicle_movement_sequence_like_cpp: u32,
    /// Represented `Unit::GetCollisionHeight()` until model-display collision data owns it.
    player_collision_height_like_cpp: f32,
    /// Represented `Object::GetObjectScale()` for movement collision-height packets.
    player_object_scale_like_cpp: f32,
    /// Represented `UnitData::ScaleDuration` for movement collision-height packets.
    player_scale_duration_like_cpp: i32,
    /// Represented `UnitData::Flags` for player deltas not yet backed by canonical Unit.
    player_unit_flags_like_cpp: UnitFlags,
    /// Represented `UNIT_FLAG_MOUNT` state until UnitData owns live player flags.
    player_mounted_like_cpp: bool,
    /// Represented `pvpInfo.IsHostile` branch for Honorless Target after taxi landing.
    player_pvp_hostile_like_cpp: bool,
    /// Represented `Player::IsPvP()` branch for friendly-area near teleport handling.
    player_pvp_enabled_like_cpp: bool,
    /// Represented `PLAYER_FLAGS_IN_PVP` branch for friendly-area near teleport handling.
    player_in_pvp_flag_like_cpp: bool,
    /// Current represented zone/area ids until Map/Terrain runtime can calculate them.
    player_zone_id_like_cpp: u32,
    player_area_id_like_cpp: u32,
    /// `MoveSplineDone` taxi decisions recorded until full Taxi/MotionMaster runtime exists.
    move_spline_done_taxi_events_like_cpp: Vec<MoveSplineDoneTaxiEventLikeCpp>,
    /// C++ `Player::mSemaphoreTeleport_Near` represented state.
    near_teleport_pending_like_cpp: bool,
    /// C++ `Player::m_teleport_dest` represented state for near teleports.
    near_teleport_destination_like_cpp: Option<(u16, wow_core::Position)>,
    /// Represented zone/area for the pending near-teleport destination.
    near_teleport_destination_zone_area_like_cpp: Option<(u32, u32)>,
    /// Near teleport ACK side-effect audit events.
    move_teleport_ack_events_like_cpp: Vec<MoveTeleportAckEventLikeCpp>,
    /// Count of C++ `ResummonPetTemporaryUnSummonedIfAny` calls after near teleport ACK.
    temporary_pet_resummon_requests_like_cpp: u32,
    /// Count of C++ `ProcessDelayedOperations` calls after successful near teleport ACK.
    delayed_operations_processed_like_cpp: u32,
    /// C++ `Player::m_forced_speed_changes[MAX_MOVE_TYPE]` represented state.
    forced_speed_changes_like_cpp: [u8; UnitMoveTypeLikeCpp::COUNT],
    /// C++ `Unit::m_speed_rate[MAX_MOVE_TYPE]` represented state for player-controlled movers.
    movement_speed_rates_like_cpp: [f32; UnitMoveTypeLikeCpp::COUNT],
    /// Represented transport guard for speed ACK anticheat; C++ skips speed mismatch while on transport.
    player_on_transport_like_cpp: bool,
    /// C++ `Player::m_movementForceModMagnitudeChanges` represented state.
    movement_force_mod_magnitude_changes_like_cpp: u8,
    /// C++ `MovementForces::GetModMagnitude()` represented value; default is 1.0 when no force container exists.
    movement_force_mod_magnitude_like_cpp: f32,
    /// Speed ACK outcomes recorded until full Unit speed runtime owns this state.
    movement_speed_ack_events_like_cpp: Vec<MovementSpeedAckEventLikeCpp>,

    // ── Aura system ───────────────────────────────────────────────
    /// All visible auras on the player: slot (0-254) → AuraApplication
    pub(crate) visible_auras: HashMap<u8, AuraApplication>,

    // ── Spell casting ──────────────────────────────────────────────
    /// Spell store (metadata for all known spells: cast time, cooldown, effects, etc.)
    pub spell_store: Option<Arc<SpellStore>>,
    spell_misc_store: Option<Arc<SpellMiscStore>>,
    spell_range_store: Option<Arc<SpellRangeStore>>,
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
    pub(crate) loot_table:
        std::collections::HashMap<wow_core::ObjectGuid, wow_packet::packets::loot::CreatureLoot>,
    /// Mirrors C++ PlayerData::LootTargetGUID for guards that compare active loot by GUID.
    pub(crate) active_loot_guid: wow_core::ObjectGuid,
    /// Represented owner GUIDs currently visible through C++ `Player::m_AELootView`.
    pub(crate) active_loot_view_owners: std::collections::HashSet<wow_core::ObjectGuid>,
    /// Represented pending group/NBG loot rolls keyed by `(LootObj, LootListID)`.
    pub(crate) represented_loot_rolls:
        std::collections::HashMap<(wow_core::ObjectGuid, u8), RepresentedLootRollState>,
    #[cfg(test)]
    pub(crate) represented_loot_roll_criteria_events: Vec<RepresentedLootRollCriteriaEvent>,
    /// C++ `sWorld->getRate(...)` subset used by represented loot generation.
    loot_drop_rates: LootDropRatesLikeCpp,
    /// C++ `CONFIG_ENABLE_AE_LOOT` represented switch.
    enable_ae_loot_like_cpp: bool,
    /// C++ `CONFIG_ENABLE_MMAPS` + `DataDir` represented until map lifecycle owns real mmaps.
    mmap_runtime_config_like_cpp: MMapRuntimeConfigLikeCpp,
    /// Session-local representation of `GameObject::m_unique_users` for no-GetLootId chest uses.
    pub(crate) represented_unique_gameobject_uses: std::collections::HashSet<wow_core::ObjectGuid>,
    /// Represented C++ `GameEvents::Trigger` and `TriggeringLinkedGameObject` hook points.
    pub(crate) represented_gameobject_use_effects: Vec<RepresentedGameObjectUseEffect>,
    /// Session-local represented `GameObject` use state until canonical GO runtime ownership lands.
    pub(crate) represented_gameobject_use_states:
        std::collections::HashMap<wow_core::ObjectGuid, RepresentedGameObjectUseState>,
    /// C++ `Player::SetPendingBind` represented until `InstanceMap` owns real bind confirmation.
    pub(crate) pending_bind: Option<RepresentedPendingBind>,
    /// Confirmed pending bind ids, used by represented `CMSG_INSTANCE_LOCK_RESPONSE`.
    pub(crate) represented_confirmed_pending_binds: Vec<u32>,
    /// Count of represented `Player::RepopAtGraveyard` calls from rejected pending binds.
    pub(crate) represented_repop_at_graveyard_count: u32,
    /// Session-local representation of `GameObject::m_tapList` for personal encounter loot.
    pub(crate) represented_gameobject_tap_lists:
        std::collections::HashMap<wow_core::ObjectGuid, Vec<wow_core::ObjectGuid>>,
    /// Session-local representation of `Player::IsLockedToDungeonEncounter` for encounter loot.
    pub(crate) represented_locked_dungeon_encounters:
        std::collections::HashSet<(wow_core::ObjectGuid, u32)>,
    /// Session-local per-player money for represented personal encounter loot.
    pub(crate) represented_personal_loot_money:
        std::collections::HashMap<(wow_core::ObjectGuid, wow_core::ObjectGuid), u32>,
    /// Owners whose money must be read from `represented_personal_loot_money`.
    pub(crate) represented_personal_loot_owners: std::collections::HashSet<wow_core::ObjectGuid>,

    // ── Dynamic visibility tracking ───────────────────────────────
    /// GUIDs of all creatures currently visible to this client.
    /// Updated on login and each visibility refresh (player movement).
    pub(crate) visible_creatures: std::collections::HashSet<wow_core::ObjectGuid>,
    /// GUIDs of all game objects currently visible to this client.
    pub(crate) visible_gameobjects: std::collections::HashSet<wow_core::ObjectGuid>,
    /// Represented C++ `GameObject::GetPhaseShift()` for visible DB-spawned
    /// gameobjects until canonical gameobject map ownership lands.
    pub(crate) represented_gameobject_phase_shifts:
        std::collections::HashMap<wow_core::ObjectGuid, PhaseShift>,
    /// Represented C++ `Player::GetPhaseShift()` for session DB visibility
    /// filtering until canonical player `WorldObject` phase ownership lands.
    pub(crate) represented_player_phase_shift: PhaseShift,
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

#[derive(Debug, Clone, Default)]
pub(crate) struct RepresentedPlayerConditionContextLikeCpp {
    spells: Vec<u32>,
    items: Vec<PlayerConditionCountLikeCpp>,
    currencies: Vec<PlayerConditionCountLikeCpp>,
    completed_quests: Vec<u32>,
    current_quests: Vec<u32>,
    complete_quests: Vec<u32>,
    auras: Vec<PlayerConditionAuraLikeCpp>,
    skills: Vec<PlayerConditionSkillLikeCpp>,
    reputations: Vec<PlayerConditionReputationLikeCpp>,
    explored_area_ids: Vec<u16>,
    parent_area_ids: Vec<u32>,
    achievements: Vec<u16>,
    lfg_values: Vec<PlayerConditionCountLikeCpp>,
    modifier_tree_ids: Vec<u32>,
    quest_kills: Vec<PlayerConditionQuestKillLikeCpp>,
    avg_item_level: f32,
    avg_equipped_item_level: f32,
    mainhand_weapon_subclass: Option<u8>,
}

impl RepresentedPlayerConditionContextLikeCpp {
    pub(crate) fn as_context<'a>(
        &'a self,
        session: &'a WorldSession,
    ) -> PlayerConditionContextLikeCpp<'a> {
        let class = session.player_class_like_cpp();
        let (_, area_id) = session.player_zone_area_like_cpp();
        PlayerConditionContextLikeCpp {
            race: session.player_race_like_cpp(),
            class_mask: if class == 0 {
                0
            } else {
                1u32 << u32::from(class.saturating_sub(1))
            },
            gender: session.player_gender_like_cpp(),
            native_gender: session.player_gender_like_cpp(),
            power_type: -1,
            power: 0,
            max_power: 0,
            primary_specialization_id: (session.loot_specialization_id != 0)
                .then_some(session.loot_specialization_id),
            skills: &self.skills,
            language_skill: 0,
            reputations: &self.reputations,
            current_pvp_faction: 0,
            pvp_medals_mask: 0,
            lifetime_max_pvp_rank: 0,
            movement_flags: [0, 0],
            mainhand_weapon_subclass: self.mainhand_weapon_subclass,
            party_status: if session.group_guid.is_some() {
                PlayerConditionPartyStatusLikeCpp::InParty
            } else {
                PlayerConditionPartyStatusLikeCpp::Solo
            },
            completed_quests: &self.completed_quests,
            current_quests: &self.current_quests,
            complete_quests: &self.complete_quests,
            spells: &self.spells,
            items: &self.items,
            currencies: &self.currencies,
            explored_area_ids: &self.explored_area_ids,
            auras: &self.auras,
            weather_id: 0,
            achievements: &self.achievements,
            lfg_values: &self.lfg_values,
            area_id,
            parent_area_ids: &self.parent_area_ids,
            expansion: session.expansion as i8,
            server_expansion: session.account_expansion as i8,
            is_game_master: session.security > 0,
            phase_satisfied: true,
            quest_kill_id: 0,
            quest_kills: &self.quest_kills,
            avg_item_level: self.avg_item_level,
            avg_equipped_item_level: self.avg_equipped_item_level,
            modifier_tree_ids: &self.modifier_tree_ids,
            chr_specializations: session.chr_specialization_store.as_deref(),
            world_state_expressions: None,
            world_state_expression_context: None,
        }
    }
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

fn player_team_id_for_race_cpp(race: u8) -> u32 {
    match player_team_for_race_cpp(race) {
        Team::Horde => 1,
        _ => 0,
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
    /// Trinity SpellAuraInterruptFlags bitmask used by represented removal paths.
    pub aura_interrupt_flags: u32,
    /// Trinity SpellAuraInterruptFlags2 bitmask used by represented removal paths.
    pub aura_interrupt_flags2: u32,
    /// Represented Trinity aura effect queried directly by movement/fall handlers.
    pub represented_effect: Option<RepresentedAuraEffectLikeCpp>,
    /// C++ `GetTotalAuraModifier` amount for represented integer aura effects.
    pub represented_amount: i32,
    /// C++ `GetTotalAuraMultiplier` factor for represented multiplier aura effects.
    pub represented_multiplier: f32,
    /// Monotonic timestamp when this aura was applied — used for expiry checks.
    pub applied_at: Instant,
}

pub(crate) const SPELL_AURA_INTERRUPT_FLAG_LOOTING_LIKE_CPP: u32 = 0x0000_0800;
pub(crate) const SPELL_AURA_INTERRUPT_FLAG_LANDING_OR_FLIGHT_LIKE_CPP: u32 = 0x0200_0000;
pub(crate) const SPELL_AURA_INTERRUPT_FLAG2_JUMP_LIKE_CPP: u32 = 0x0000_0020;
pub(crate) const PLAYER_LOCAL_FLAG_OVERRIDE_TRANSPORT_SERVER_TIME_LIKE_CPP: u32 = 0x0000_8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentedAuraEffectLikeCpp {
    FeatherFall,
    Hover,
    SafeFall,
    Fly,
    Ghost,
    Mounted,
    MountedFlightSpeed,
    ModifyFallDamagePct,
    WaterWalk,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MovementFallDamageEvent {
    pub z_diff: f32,
    pub damage: u32,
    pub final_damage: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MovementUnderMapDamageEvent {
    pub z: f32,
    pub min_height: f32,
    pub damage: u32,
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
    pub loot_id: u32,
    pub gold_min: u32,
    pub gold_max: u32,
    pub boss_id: Option<u32>,
    pub dungeon_encounter_id: u32,
    pub phase_use_flags: u8,
    pub phase_id: u16,
    pub phase_group_id: u32,
    pub terrain_swap_map: i32,
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
        let (session_command_tx, session_command_rx) = flume::bounded(256);

        Self {
            account_id,
            battlenet_account_id: account_id,
            account_name,
            security,
            expansion,
            account_expansion,
            build,
            session_key,
            locale,
            packet_rx,
            send_tx,
            session_command_tx,
            session_command_rx,
            state: SessionState::Authed,
            last_packet_time: Instant::now(),
            dispatch_table: build_dispatch_table(),
            char_db: None,
            login_db: None,
            world_db: None,
            currency_types_store: None,
            import_price_stores: None,
            item_class_store: None,
            item_currency_cost_store: None,
            item_extended_cost_store: None,
            item_store: None,
            item_appearance_store: None,
            item_modified_appearance_store: None,
            item_price_base_store: None,
            player_stats: None,
            item_stats_store: None,
            item_random_suffix_store: None,
            item_random_properties_store: None,
            rand_prop_points_store: None,
            item_random_enchantment_template_store: None,
            item_disenchant_loot_store: None,
            loot_stores: None,
            condition_store: None,
            player_condition_store: None,
            disable_mgr: None,
            lock_store: None,
            spell_item_enchantment_store: None,
            hotfix_blob_cache: None,
            skill_store: None,
            area_table_store: None,
            area_trigger_store: None,
            chr_specialization_store: None,
            dungeon_encounter_store: None,
            map_store: None,
            map_difficulty_store: None,
            map_difficulty_x_condition_store: None,
            creature_template_mount_store: None,
            creature_display_info_store: None,
            gameobject_display_info_store: None,
            creature_model_data_store: None,
            mount_store: None,
            mount_capability_store: None,
            mount_type_x_capability_store: None,
            mount_x_display_store: None,
            vehicle_store: None,
            vehicle_seat_store: None,
            vehicle_template_store: None,
            vehicle_accessory_store: None,
            terrain_swap_store: None,
            phase_store: None,
            phase_group_store: None,
            player_registry: None,
            object_accessor: None,
            group_registry: None,
            pending_invites: None,
            group_guid: None,
            pass_on_group_loot: false,
            represented_enchanting_skill: 0,
            player_skill_values_like_cpp: HashMap::new(),
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
            time_sync_pending_requests: HashMap::new(),
            time_sync_clock_delta_queue: VecDeque::with_capacity(6),
            time_sync_clock_delta: 0,
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
            player_controller: None,
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
            loot_specialization_id: 0,
            known_spells: Vec::new(),
            account_mounts_like_cpp: HashMap::new(),
            realm_packet_rx: None,
            realm_send_tx: None,
            player_position: None,
            player_movement_flags_like_cpp: MovementFlag::NONE,
            player_liquid_status_like_cpp: 0,
            player_name: None,
            registered_addon_prefixes: Vec::new(),
            filter_addon_messages: false,
            creature_tick: 0,
            #[cfg(test)]
            creatures: std::collections::HashMap::new(),
            vendor_item_counts: HashMap::new(),
            map_manager: None,
            canonical_map_manager: None,
            mmap_pathfinder_like_cpp: None,
            combat_target: None,
            combat_tick_last_at_like_cpp: Instant::now(),
            player_melee_los_to_target_like_cpp: None,
            in_combat: false,
            player_alive_like_cpp: true,
            player_game_master_like_cpp: false,
            player_cheat_god_like_cpp: false,
            player_normal_damage_immune_like_cpp: false,
            player_environmental_damage_immune_like_cpp: false,
            player_health_like_cpp: 100,
            player_max_health_like_cpp: 100,
            player_movement_time_like_cpp: 0,
            last_fall_time_like_cpp: 0,
            last_fall_z_like_cpp: 0.0,
            fall_damage_events_like_cpp: Vec::new(),
            player_out_of_bounds_like_cpp: false,
            under_map_damage_events_like_cpp: Vec::new(),
            player_stand_state_like_cpp: UnitStandStateType::Stand,
            temporary_pet_unsummon_requests_like_cpp: 0,
            movement_jump_proc_requests_like_cpp: 0,
            active_player_local_flags_like_cpp: 0,
            active_player_transport_server_time_like_cpp: 0,
            movement_visibility_refresh_requests_like_cpp: 0,
            movement_ack_events_like_cpp: Vec::new(),
            taxi_destinations_like_cpp: Vec::new(),
            taxi_node_map_ids_like_cpp: HashMap::new(),
            taxi_flight_state_like_cpp: None,
            taxi_unit_flags_like_cpp: UnitFlags::empty(),
            taxi_mounted_like_cpp: false,
            player_mount_display_id_like_cpp: 0,
            player_mount_vehicle_id_like_cpp: 0,
            player_mount_vehicle_kit_like_cpp: None,
            player_mount_vehicle_accessories_like_cpp: Vec::new(),
            player_mount_vehicle_seat_count_like_cpp: 0,
            player_mount_vehicle_usable_seat_count_like_cpp: 0,
            player_vehicle_seat_flags_like_cpp: None,
            represented_pet_guid_like_cpp: None,
            represented_pet_react_state_like_cpp:
                wow_packet::packets::pet::REACT_DEFENSIVE_LIKE_CPP,
            represented_pet_command_state_like_cpp:
                wow_packet::packets::pet::COMMAND_FOLLOW_LIKE_CPP,
            temporary_mount_pet_react_state_like_cpp: None,
            mount_vehicle_create_requests_like_cpp: 0,
            mount_vehicle_remove_requests_like_cpp: 0,
            mount_cancel_expected_vehicle_aura_packets_like_cpp: 0,
            mount_pet_control_disable_requests_like_cpp: 0,
            mount_pet_control_enable_requests_like_cpp: 0,
            mount_pet_resummon_requests_like_cpp: 0,
            mount_collision_height_update_requests_like_cpp: 0,
            mount_vehicle_movement_sequence_like_cpp: 0,
            player_collision_height_like_cpp: 1.0,
            player_object_scale_like_cpp: 1.0,
            player_scale_duration_like_cpp: 0,
            player_unit_flags_like_cpp: UnitFlags::PLAYER_CONTROLLED,
            player_mounted_like_cpp: false,
            player_pvp_hostile_like_cpp: false,
            player_pvp_enabled_like_cpp: false,
            player_in_pvp_flag_like_cpp: false,
            player_zone_id_like_cpp: 0,
            player_area_id_like_cpp: 0,
            move_spline_done_taxi_events_like_cpp: Vec::new(),
            near_teleport_pending_like_cpp: false,
            near_teleport_destination_like_cpp: None,
            near_teleport_destination_zone_area_like_cpp: None,
            move_teleport_ack_events_like_cpp: Vec::new(),
            temporary_pet_resummon_requests_like_cpp: 0,
            delayed_operations_processed_like_cpp: 0,
            forced_speed_changes_like_cpp: [0; UnitMoveTypeLikeCpp::COUNT],
            movement_speed_rates_like_cpp: [1.0; UnitMoveTypeLikeCpp::COUNT],
            player_on_transport_like_cpp: false,
            movement_force_mod_magnitude_changes_like_cpp: 0,
            movement_force_mod_magnitude_like_cpp: 1.0,
            movement_speed_ack_events_like_cpp: Vec::new(),
            visible_auras: HashMap::new(),
            spell_store: None,
            spell_misc_store: None,
            spell_range_store: None,
            quest_store: None,
            quest_xp_store: None,
            player_quests: HashMap::new(),
            rewarded_quests: std::collections::HashSet::new(),
            active_spell_cast: None,
            last_spell_cast_time: None,
            last_spell_cast_time_per_spell: HashMap::new(),
            loot_table: std::collections::HashMap::new(),
            active_loot_guid: ObjectGuid::EMPTY,
            active_loot_view_owners: std::collections::HashSet::new(),
            represented_loot_rolls: std::collections::HashMap::new(),
            #[cfg(test)]
            represented_loot_roll_criteria_events: Vec::new(),
            loot_drop_rates: LootDropRatesLikeCpp::default(),
            enable_ae_loot_like_cpp: false,
            mmap_runtime_config_like_cpp: MMapRuntimeConfigLikeCpp::default(),
            represented_unique_gameobject_uses: std::collections::HashSet::new(),
            represented_gameobject_use_effects: Vec::new(),
            represented_gameobject_use_states: std::collections::HashMap::new(),
            pending_bind: None,
            represented_confirmed_pending_binds: Vec::new(),
            represented_repop_at_graveyard_count: 0,
            represented_gameobject_tap_lists: std::collections::HashMap::new(),
            represented_locked_dungeon_encounters: std::collections::HashSet::new(),
            represented_personal_loot_money: std::collections::HashMap::new(),
            represented_personal_loot_owners: std::collections::HashSet::new(),
            visible_creatures: std::collections::HashSet::new(),
            visible_gameobjects: std::collections::HashSet::new(),
            represented_gameobject_phase_shifts: std::collections::HashMap::new(),
            represented_player_phase_shift: PhaseShift::default(),
            last_visibility_pos: None,
            gossip_options: Vec::new(),
            gossip_source_guid: None,
            active_area_trigger: None,
            pending_teleport: None,
            creature_query_cache: std::collections::HashSet::new(),
            instance_lock_mgr: None,
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

    pub fn set_canonical_map_manager(&mut self, mgr: SharedCanonicalMapManager) {
        self.canonical_map_manager = Some(mgr);
    }

    fn canonical_player_entity_snapshot_like_cpp(&self) -> Option<Player> {
        let guid = self.player_guid()?;
        let position = self.player_position_like_cpp()?;
        let name = self.player_name_like_cpp()?;
        let mut player = Player::new(Some(u64::from(self.account_id)), false);
        player.unit_mut().world_mut().object_mut().create(guid);
        player.unit_mut().world_mut().set_name(name);
        player
            .unit_mut()
            .world_mut()
            .set_map(u32::from(self.player_map_id_like_cpp()), 0)
            .ok()?;
        player.unit_mut().world_mut().relocate(position);
        player.unit_mut().world_mut().object_mut().add_to_world();
        player.set_race_class_gender(
            self.player_race_like_cpp(),
            self.player_class_like_cpp(),
            gender_from_u8(self.player_gender_like_cpp()),
        );
        player.unit_mut().set_level(self.player_level_like_cpp());
        player
            .unit_mut()
            .set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
        player
            .unit_mut()
            .set_unit_flags_like_cpp(self.player_unit_flags_like_cpp);
        if let Some(selection) = self.selection_guid_like_cpp() {
            player.set_selection(selection);
        }
        Some(player)
    }

    fn sync_canonical_player_entity_like_cpp(
        managed: &mut wow_map::ManagedMap,
        mut player: Player,
    ) {
        let guid = player.guid();
        let map = managed.map_mut();
        if let Some(existing) = map.get_typed_player_mut(guid) {
            let attacking = existing.unit().attacking();
            let target = existing.unit().data().target;
            let player_flags = existing.data().player_flags;
            let player_flags_ex = existing.data().player_flags_ex;
            player.replace_all_player_flags(player_flags);
            player.replace_all_player_flags_ex(player_flags_ex);
            *existing = player;
            existing.unit_mut().set_attacking(attacking);
            existing.unit_mut().set_target(target);
            return;
        }

        let Ok(record) = wow_entities::MapObjectRecord::new_player(player) else {
            return;
        };
        let _ = map.insert_map_object_record(record);
    }

    pub(crate) fn mutate_canonical_player_like_cpp<R>(
        &mut self,
        f: impl FnOnce(&mut Player) -> R,
    ) -> Option<R> {
        let guid = self.player_guid()?;
        self.mutate_canonical_player_by_guid_like_cpp(guid, f)
    }

    pub(crate) fn mutate_canonical_player_by_guid_like_cpp<R>(
        &mut self,
        guid: ObjectGuid,
        f: impl FnOnce(&mut Player) -> R,
    ) -> Option<R> {
        let map_id = u32::from(self.player_map_id_like_cpp());
        let manager = Arc::clone(self.canonical_map_manager.as_ref()?);
        let mut manager = manager.lock().ok()?;
        let mut instance_id = None;
        manager.do_for_all_maps_with_map_id(map_id, |managed| {
            if instance_id.is_none() && managed.map().get_typed_player(guid).is_some() {
                instance_id = Some(managed.instance_id());
            }
        });
        let managed = manager.find_map_mut(map_id, instance_id.unwrap_or(0))?;
        let player = managed.map_mut().get_typed_player_mut(guid)?;
        Some(f(player))
    }

    fn canonical_player_has_player_flag_like_cpp(
        &self,
        guid: ObjectGuid,
        flag: u32,
    ) -> Option<bool> {
        let map_id = u32::from(self.player_map_id_like_cpp());
        let manager = Arc::clone(self.canonical_map_manager.as_ref()?);
        let manager = manager.lock().ok()?;
        let mut result = None;
        manager.do_for_all_maps_with_map_id(map_id, |managed| {
            if result.is_none() {
                result = managed
                    .map()
                    .get_typed_player(guid)
                    .map(|player| player.has_player_flag(flag));
            }
        });
        result
    }

    fn canonical_player_attack_target_like_cpp(&self) -> Option<ObjectGuid> {
        let guid = self.player_guid?;
        let map_id = u32::from(self.player_map_id_like_cpp());
        let manager = Arc::clone(self.canonical_map_manager.as_ref()?);
        let manager = manager.lock().ok()?;
        let mut result = None;
        manager.do_for_all_maps_with_map_id(map_id, |managed| {
            if result.is_none() {
                result = managed
                    .map()
                    .get_typed_player(guid)
                    .and_then(|player| player.unit().attacking());
            }
        });
        result
    }

    fn is_player_facing_target_for_melee_like_cpp(
        player_position: Position,
        target_position: Position,
    ) -> bool {
        let dx = target_position.x - player_position.x;
        let dy = target_position.y - player_position.y;
        if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
            return true;
        }

        let target_angle = dy.atan2(dx);
        let mut diff =
            (target_angle - player_position.orientation).rem_euclid(std::f32::consts::TAU);
        if diff > std::f32::consts::PI {
            diff = std::f32::consts::TAU - diff;
        }
        diff <= std::f32::consts::PI / 3.0
    }

    fn take_canonical_player_attack_swings_like_cpp(
        &mut self,
        diff_ms: u32,
        in_melee_range: bool,
        facing_target: bool,
        within_los: bool,
    ) -> Option<Vec<u32>> {
        self.mutate_canonical_player_like_cpp(|player| {
            let unit = player.unit_mut();
            unit.update_attack_timers_like_cpp(diff_ms);
            let mut swings = Vec::new();
            let mut processed_ready_attack = false;
            let has_auto_attack_error = !in_melee_range || !facing_target;
            let melee_state_update_allowed =
                within_los && unit.can_attacker_state_update_melee_like_cpp(false);

            if unit.is_attack_ready_like_cpp(WeaponAttackType::BaseAttack) {
                processed_ready_attack = true;
                if has_auto_attack_error {
                    unit.set_attack_timer(WeaponAttackType::BaseAttack, 100);
                } else {
                    if unit.can_dual_wield_like_cpp()
                        && unit.attack_timer(WeaponAttackType::OffAttack)
                            < ATTACK_DISPLAY_DELAY_LIKE_CPP_MS
                    {
                        unit.set_attack_timer(
                            WeaponAttackType::OffAttack,
                            ATTACK_DISPLAY_DELAY_LIKE_CPP_MS,
                        );
                    }
                    if melee_state_update_allowed {
                        unit.remove_attacking_interrupt_auras_like_cpp();
                        if unit
                            .current_spell(wow_entities::CurrentSpellSlot::Melee)
                            .is_some()
                        {
                            let _ = unit.finish_spell(wow_entities::CurrentSpellSlot::Melee);
                        } else {
                            let [min_damage, max_damage] =
                                unit.weapon_damage(WeaponAttackType::BaseAttack);
                            swings
                                .push(min_damage.max(1.0).min(max_damage.max(1.0)).round() as u32);
                        }
                    }
                    unit.reset_attack_timer_like_cpp(WeaponAttackType::BaseAttack);
                }
            }

            if unit.can_dual_wield_like_cpp()
                && unit.is_attack_ready_like_cpp(WeaponAttackType::OffAttack)
            {
                processed_ready_attack = true;
                if has_auto_attack_error {
                    unit.set_attack_timer(WeaponAttackType::OffAttack, 100);
                } else {
                    if unit.attack_timer(WeaponAttackType::BaseAttack)
                        < ATTACK_DISPLAY_DELAY_LIKE_CPP_MS
                    {
                        unit.set_attack_timer(
                            WeaponAttackType::BaseAttack,
                            ATTACK_DISPLAY_DELAY_LIKE_CPP_MS,
                        );
                    }
                    if melee_state_update_allowed {
                        unit.remove_attacking_interrupt_auras_like_cpp();
                        let [min_damage, max_damage] =
                            unit.weapon_damage(WeaponAttackType::OffAttack);
                        swings.push(min_damage.max(1.0).min(max_damage.max(1.0)).round() as u32);
                    }
                    unit.reset_attack_timer_like_cpp(WeaponAttackType::OffAttack);
                }
            }

            processed_ready_attack.then_some(swings)
        })
        .flatten()
    }

    pub(crate) fn mutate_canonical_creature_by_guid_like_cpp<R>(
        &mut self,
        guid: ObjectGuid,
        f: impl FnOnce(&mut wow_entities::Creature) -> R,
    ) -> Option<R> {
        let map_id = u32::from(self.player_map_id_like_cpp());
        let manager = Arc::clone(self.canonical_map_manager.as_ref()?);
        let mut manager = manager.lock().ok()?;
        let managed = manager.find_map_mut(map_id, 0)?;
        let creature = managed.map_mut().get_typed_creature_mut(guid)?;
        Some(f(creature))
    }

    fn canonical_unit_attack_target_state_like_cpp(
        &self,
        guid: ObjectGuid,
    ) -> (bool, bool, wow_entities::UnitAttackContextLikeCpp) {
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return (
                true,
                true,
                wow_entities::UnitAttackContextLikeCpp::default(),
            );
        };
        let Ok(manager) = manager.lock() else {
            return (
                true,
                true,
                wow_entities::UnitAttackContextLikeCpp::default(),
            );
        };
        let Some(map) = manager.find_map(u32::from(self.player_map_id_like_cpp()), 0) else {
            return (
                true,
                true,
                wow_entities::UnitAttackContextLikeCpp::default(),
            );
        };
        if let Some(player) = map.map().get_typed_player(guid) {
            return (
                player.unit().is_alive(),
                player.unit().world().object().is_in_world(),
                wow_entities::UnitAttackContextLikeCpp {
                    victim_is_game_master_player: player.is_game_master_like_cpp(),
                    victim_unit_state: player.unit().unit_state(),
                    victim_unit_flags: player.unit().unit_flags_like_cpp().bits(),
                    victim_has_affecting_player: true,
                    ..Default::default()
                },
            );
        }
        if let Some(creature) = map.map().get_typed_creature(guid) {
            return (
                creature.is_alive(),
                creature.unit().world().object().is_in_world(),
                wow_entities::UnitAttackContextLikeCpp {
                    victim_is_evading_creature: creature.is_evading_attacks_like_cpp(),
                    victim_unit_state: creature.unit().unit_state(),
                    victim_unit_flags: creature.unit().unit_flags_like_cpp().bits(),
                    ..Default::default()
                },
            );
        }
        (
            true,
            true,
            wow_entities::UnitAttackContextLikeCpp::default(),
        )
    }

    fn player_vehicle_seat_allows_attack_like_cpp(&self) -> bool {
        match self.player_vehicle_seat_flags_like_cpp {
            Some(flags) => flags & VEHICLE_SEAT_FLAG_CAN_ATTACK != 0,
            None => true,
        }
    }

    fn add_canonical_attacker_like_cpp(&mut self, victim: ObjectGuid, attacker: ObjectGuid) {
        if self
            .mutate_canonical_player_by_guid_like_cpp(victim, |victim| {
                victim.unit_mut().add_attacker_like_cpp(attacker)
            })
            .is_some()
        {
            return;
        }
        let _ = self.mutate_canonical_creature_by_guid_like_cpp(victim, |victim| {
            victim.unit_mut().add_attacker_like_cpp(attacker)
        });
    }

    fn remove_canonical_attacker_like_cpp(&mut self, victim: ObjectGuid, attacker: ObjectGuid) {
        if self
            .mutate_canonical_player_by_guid_like_cpp(victim, |victim| {
                victim.unit_mut().remove_attacker_like_cpp(attacker)
            })
            .is_some()
        {
            return;
        }
        let _ = self.mutate_canonical_creature_by_guid_like_cpp(victim, |victim| {
            victim.unit_mut().remove_attacker_like_cpp(attacker)
        });
    }

    pub(crate) fn start_player_attack_like_cpp(&mut self, victim: ObjectGuid) {
        let _ = self.ensure_canonical_world_map_for_current_player_like_cpp();
        let player_guid = self.player_guid();
        let attacker_is_mounted_player = self.player_mounted_like_cpp;
        let (victim_alive, victim_in_world, mut attack_context) =
            self.canonical_unit_attack_target_state_like_cpp(victim);
        if !self.player_vehicle_seat_allows_attack_like_cpp() {
            self.combat_target = None;
            self.in_combat = false;
            if self.selection_guid_like_cpp() == Some(victim) {
                self.set_selection_guid_like_cpp(None);
            }
            return;
        }
        attack_context.attacker_is_mounted_player = attacker_is_mounted_player;
        attack_context.attacker_unit_flags = self.player_unit_flags_like_cpp.bits();
        attack_context.attacker_has_affecting_player = true;
        attack_context.attacker_is_player_uber = player_guid
            .and_then(|guid| {
                self.canonical_player_has_player_flag_like_cpp(guid, PLAYER_FLAGS_UBER_LIKE_CPP)
            })
            .unwrap_or(false);
        self.combat_target = Some(victim);
        self.in_combat = true;
        self.set_selection_guid_like_cpp(Some(victim));
        let outcome = self.mutate_canonical_player_like_cpp(|player| {
            player.unit_mut().attack_with_context_like_cpp(
                victim,
                victim_alive,
                victim_in_world,
                true,
                attack_context,
            )
        });
        let previous = match outcome {
            Some(wow_entities::UnitAttackStartOutcome::NewTarget { previous }) => previous,
            Some(
                wow_entities::UnitAttackStartOutcome::MeleeStartedSameTarget
                | wow_entities::UnitAttackStartOutcome::MeleeStoppedSameTarget
                | wow_entities::UnitAttackStartOutcome::NoChangeSameTarget,
            ) => None,
            Some(
                wow_entities::UnitAttackStartOutcome::InvalidSelfTarget
                | wow_entities::UnitAttackStartOutcome::InvalidDeadAttacker
                | wow_entities::UnitAttackStartOutcome::InvalidDeadVictim
                | wow_entities::UnitAttackStartOutcome::InvalidVictimNotInWorld
                | wow_entities::UnitAttackStartOutcome::InvalidMountedAttacker
                | wow_entities::UnitAttackStartOutcome::InvalidAttackerEvading
                | wow_entities::UnitAttackStartOutcome::InvalidVictimGameMaster
                | wow_entities::UnitAttackStartOutcome::InvalidVictimEvading
                | wow_entities::UnitAttackStartOutcome::InvalidAttackTarget,
            )
            | None => {
                self.combat_target = None;
                self.in_combat = false;
                if self.selection_guid_like_cpp() == Some(victim) {
                    self.set_selection_guid_like_cpp(None);
                }
                return;
            }
        };
        if let Some(player_guid) = player_guid {
            if let Some(previous) = previous {
                self.remove_canonical_attacker_like_cpp(previous, player_guid);
            }
            self.add_canonical_attacker_like_cpp(victim, player_guid);
        }
    }

    pub(crate) fn stop_player_attack_like_cpp(&mut self) -> Option<ObjectGuid> {
        let player_guid = self.player_guid()?;
        let target = self
            .mutate_canonical_player_like_cpp(|player| {
                match player.unit_mut().attack_stop_like_cpp() {
                    wow_entities::UnitAttackStopOutcome::Stopped { victim } => Some(victim),
                    wow_entities::UnitAttackStopOutcome::NoVictim => None,
                }
            })
            .flatten()
            .or_else(|| self.combat_target.take())?;
        self.combat_target = None;
        self.in_combat = false;
        if self.selection_guid_like_cpp() == Some(target) {
            self.set_selection_guid_like_cpp(None);
        }
        self.remove_canonical_attacker_like_cpp(target, player_guid);
        Some(target)
    }

    pub(crate) fn ensure_canonical_world_map_for_current_player_like_cpp(
        &mut self,
    ) -> Option<wow_map::CreateMapDecision> {
        let map_id = u32::from(self.player_map_id_like_cpp());
        let map_entry = self.map_store.as_ref()?.get(map_id).copied()?;
        if map_entry.is_dungeon() || map_entry.is_battleground_or_arena() || map_entry.is_garrison()
        {
            return None;
        }

        let player_guid = self.player_guid?;
        let player = wow_map::CreateMapPlayerContext {
            guid_counter: player_guid.counter() as u64,
            team_id: player_team_id_for_race_cpp(self.player_race_like_cpp()),
            battleground_id: 0,
            has_battleground: false,
            player_difficulty_id: 0,
            player_recent_instance_id: 0,
            group: None,
        };
        let entry = wow_map::CreateMapEntryContext {
            map_id,
            kind: wow_map::CreateMapEntryKind::World,
            split_by_faction: map_entry.is_split_by_faction(),
            flex_locking: map_entry.is_flex_locking(),
        };

        let player_entity = self.canonical_player_entity_snapshot_like_cpp();
        let manager = Arc::clone(self.canonical_map_manager.as_ref()?);
        let mut manager = manager.lock().ok()?;
        let decision = manager.create_map_decision_like_cpp(
            Some(entry),
            Some(player),
            |_, _| None,
            None,
            |_, _| None,
        );

        if let wow_map::CreateMapDecision::Create {
            key,
            difficulty_id,
            kind,
            ..
        } = &decision
        {
            manager.create_map_entry(key.map_id, key.instance_id, *difficulty_id, *kind);
        }

        if let Some(player) = player_entity {
            let key = match &decision {
                wow_map::CreateMapDecision::Existing { key, .. }
                | wow_map::CreateMapDecision::Create { key, .. } => Some(*key),
                wow_map::CreateMapDecision::Reject { .. } => None,
            };
            if let Some(key) = key {
                if let Some(managed) = manager.find_map_mut(key.map_id, key.instance_id) {
                    Self::sync_canonical_player_entity_like_cpp(managed, player);
                }
            }
        }

        Some(decision)
    }

    /// Inject the dedicated Detour worker handle. The session only sends
    /// path requests; it never owns raw mmap/navmesh state.
    pub fn set_mmap_pathfinder_like_cpp(
        &mut self,
        pathfinder: Arc<WorldMMapPathfinderWorkerLikeCpp>,
    ) {
        self.mmap_pathfinder_like_cpp = Some(pathfinder);
    }

    /// Inject the shared C++ `InstanceLockMgr` analogue.
    pub fn set_instance_lock_mgr(
        &mut self,
        mgr: Arc<std::sync::RwLock<wow_instances::InstanceLockMgr>>,
    ) {
        self.instance_lock_mgr = Some(mgr);
    }

    /// Set the realm ID for GUID creation.
    /// Register a creature through canonical map state when available, keeping
    /// the legacy per-session AI facade as a compatibility cache.
    pub(crate) fn register_world_creature(
        &mut self,
        map_id: u16,
        position: wow_core::Position,
        create_data: wow_packet::packets::update::CreatureCreateData,
        min_dmg: u32,
        max_dmg: u32,
        aggro_radius: f32,
        loot_id: u32,
        gold_min: u32,
        gold_max: u32,
        boss_id: Option<u32>,
        dungeon_encounter_id: u32,
        phase_use_flags: u8,
        phase_id: u16,
        phase_group_id: u32,
        terrain_swap_map: i32,
    ) {
        let guid = create_data.guid;
        let entry = create_data.entry;
        let hp = create_data.health.max(1) as u32;
        let level = create_data.level;
        let display_id = create_data.display_id;
        let faction = create_data.faction_template.max(0) as u32;
        let npc_flags = create_data.npc_flags as u32;
        let unit_flags = create_data.unit_flags;
        let (db_phase_shift, validated_terrain_swap_map) = self.db_spawn_phase_shift_like_cpp(
            map_id,
            phase_use_flags,
            phase_id,
            phase_group_id,
            terrain_swap_map,
        );
        let canonical_creature = {
            let mut creature = wow_entities::Creature::new(false);
            creature.unit_mut().world_mut().object_mut().create(guid);
            creature
                .unit_mut()
                .world_mut()
                .object_mut()
                .set_entry(entry);
            let _ = creature
                .unit_mut()
                .world_mut()
                .set_map(u32::from(map_id), 0);
            creature.unit_mut().world_mut().relocate(position);
            *creature.unit_mut().world_mut().phase_shift_mut() = db_phase_shift.clone();
            creature.unit_mut().set_level(level);
            creature.unit_mut().set_max_health(u64::from(hp));
            creature.unit_mut().set_health(u64::from(hp));
            creature.set_ai_identity_runtime(display_id, faction, npc_flags, unit_flags);
            creature.configure_ai_runtime(position, aggro_radius, 5.0, 30);
            creature.ai_ownership_mut().min_damage = min_dmg;
            creature.ai_ownership_mut().max_damage = max_dmg;
            creature.ai_ownership_mut().loot_id = loot_id;
            creature.ai_ownership_mut().gold_min = gold_min;
            creature.ai_ownership_mut().gold_max = gold_max;
            creature.ai_ownership_mut().boss_id = boss_id;
            creature.ai_ownership_mut().dungeon_encounter_id = dungeon_encounter_id;
            creature.ai_ownership_mut().phase_use_flags = phase_use_flags;
            creature.ai_ownership_mut().phase_id = phase_id;
            creature.ai_ownership_mut().phase_group_id = phase_group_id;
            creature.ai_ownership_mut().terrain_swap_map = validated_terrain_swap_map;
            creature
        };
        self.insert_canonical_creature_map_object_like_cpp(map_id, canonical_creature.clone());

        if let Some(manager) = &self.map_manager {
            let (grid_x, grid_y) = crate::map_manager::world_to_grid_coords(position.x, position.y);
            let mut manager = manager
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if manager.find_creature(map_id, 0, guid).is_none() {
                let world_creature = crate::map_manager::WorldCreature::from_canonical(
                    canonical_creature,
                    create_data.clone(),
                );
                manager.add_creature(map_id, 0, grid_x, grid_y, world_creature);
            }
        }
    }

    fn insert_canonical_creature_map_object_like_cpp(
        &mut self,
        map_id: u16,
        mut creature: wow_entities::Creature,
    ) {
        let guid = creature.unit().world().object().guid();
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return;
        };
        let Ok(mut manager) = manager.lock() else {
            return;
        };
        let Some(map) = manager.find_map_mut(u32::from(map_id), 0) else {
            return;
        };
        if map.map().get_creature(guid).is_some() {
            return;
        }

        let object = creature.unit().world().clone();
        let _ = map
            .map_mut()
            .add_to_map_like_cpp(AccessorObjectKind::Creature, object);
        creature.unit_mut().world_mut().object_mut().add_to_world();
        let Ok(record) = wow_entities::MapObjectRecord::new_creature(creature) else {
            return;
        };
        let _ = map.map_mut().insert_map_object_record(record);
    }

    pub(crate) fn remove_world_creature(
        &mut self,
        guid: ObjectGuid,
    ) -> Option<crate::map_manager::WorldCreature> {
        let manager = self.map_manager.as_ref().cloned()?;
        let removed = {
            let mut manager = manager
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            manager.remove_creature_any(self.player_map_id_like_cpp(), 0, guid)
        };
        if removed.is_some() {
            self.remove_canonical_creature_map_object_like_cpp(guid);
        }
        removed
    }

    fn remove_canonical_creature_map_object_like_cpp(&mut self, guid: ObjectGuid) {
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return;
        };
        let Ok(mut manager) = manager.lock() else {
            return;
        };
        let Some(map) = manager.find_map_mut(u32::from(self.player_map_id_like_cpp()), 0) else {
            return;
        };
        let _ = map.map_mut().remove_from_map_like_cpp(guid, true);
    }

    fn relocate_canonical_creature_map_object_like_cpp(
        &mut self,
        guid: ObjectGuid,
        position: wow_core::Position,
    ) {
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return;
        };
        let Ok(mut manager) = manager.lock() else {
            return;
        };
        let Some(map) = manager.find_map_mut(u32::from(self.player_map_id_like_cpp()), 0) else {
            return;
        };
        let _ = map.map_mut().relocate_map_object_like_cpp(guid, position);
    }

    fn sync_canonical_creature_entity_like_cpp(&mut self, mut creature: wow_entities::Creature) {
        let guid = creature.unit().world().object().guid();
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return;
        };
        let Ok(mut manager) = manager.lock() else {
            return;
        };
        let Some(map) = manager.find_map_mut(u32::from(self.player_map_id_like_cpp()), 0) else {
            return;
        };
        if map.map().get_creature(guid).is_none() {
            return;
        }
        creature.unit_mut().world_mut().object_mut().add_to_world();
        let Ok(record) = wow_entities::MapObjectRecord::new_creature(creature) else {
            return;
        };
        let _ = map.map_mut().insert_map_object_record(record);
    }

    pub(crate) fn record_represented_gameobject_runtime_state_like_cpp(
        &mut self,
        map_id: u16,
        guid: ObjectGuid,
        entry: u32,
        position: wow_core::Position,
        go_type: u8,
    ) {
        let state = self
            .represented_gameobject_use_states
            .entry(guid)
            .or_default();
        state.map_id = Some(map_id);
        state.position = Some(position);
        state.go_type = Some(go_type);
        self.upsert_canonical_gameobject_map_object_like_cpp(map_id, guid, entry, position);
    }

    pub(crate) fn record_represented_gameobject_interact_radius_override_like_cpp(
        &mut self,
        guid: ObjectGuid,
        interact_radius_override: u32,
    ) {
        self.represented_gameobject_use_states
            .entry(guid)
            .or_default()
            .interact_radius_override =
            (interact_radius_override != 0).then_some(interact_radius_override);
    }

    pub(crate) fn record_represented_gameobject_lock_id_like_cpp(
        &mut self,
        guid: ObjectGuid,
        lock_id: u32,
    ) {
        self.represented_gameobject_use_states
            .entry(guid)
            .or_default()
            .lock_id = (lock_id != 0).then_some(lock_id);
    }

    pub(crate) fn record_represented_gameobject_display_model_like_cpp(
        &mut self,
        guid: ObjectGuid,
        display_id: u32,
        scale: f32,
        rotation: [f32; 4],
    ) {
        let state = self
            .represented_gameobject_use_states
            .entry(guid)
            .or_default();
        state.display_id = (display_id != 0).then_some(display_id);
        state.scale = scale;
        state.rotation = rotation;
    }

    pub(crate) fn represented_gameobject_is_per_player_despawned_like_cpp(
        &mut self,
        guid: ObjectGuid,
    ) -> bool {
        let Some(state) = self.represented_gameobject_use_states.get_mut(&guid) else {
            return false;
        };
        match state.per_player_despawn_until {
            Some(until) if until > Instant::now() => true,
            Some(_) => {
                state.per_player_despawn_until = None;
                state.per_player_despawn_secs = None;
                false
            }
            None => false,
        }
    }

    #[cfg(test)]
    pub(crate) fn record_represented_gameobject_owner_guid_like_cpp(
        &mut self,
        guid: ObjectGuid,
        owner_guid: ObjectGuid,
    ) {
        self.represented_gameobject_use_states
            .entry(guid)
            .or_default()
            .owner_guid = (!owner_guid.is_empty()).then_some(owner_guid);
    }

    pub(crate) fn record_represented_fishing_hole_max_opens_like_cpp(
        &mut self,
        guid: ObjectGuid,
        max_opens: u32,
    ) {
        self.represented_gameobject_use_states
            .entry(guid)
            .or_default()
            .fishing_hole_max_opens = Some(max_opens);
    }

    fn upsert_canonical_gameobject_map_object_like_cpp(
        &mut self,
        map_id: u16,
        guid: ObjectGuid,
        entry: u32,
        position: wow_core::Position,
    ) {
        let Some(manager) = self.canonical_map_manager.as_ref() else {
            return;
        };
        let Ok(mut manager) = manager.lock() else {
            return;
        };
        let Some(map) = manager.find_map_mut(u32::from(map_id), 0) else {
            return;
        };
        if map.map().get_game_object(guid).is_some() {
            let _ = map.map_mut().relocate_map_object_like_cpp(guid, position);
            return;
        }

        let mut game_object = GameObject::new();
        game_object.world_mut().object_mut().create(guid);
        game_object.world_mut().object_mut().set_entry(entry);
        if game_object
            .world_mut()
            .set_map(u32::from(map_id), 0)
            .is_err()
        {
            return;
        }
        game_object.world_mut().relocate(position);
        let _ = map
            .map_mut()
            .add_to_map_like_cpp(AccessorObjectKind::GameObject, game_object.world().clone());
        game_object.world_mut().object_mut().add_to_world();
        let Ok(record) = wow_entities::MapObjectRecord::new_game_object(game_object) else {
            return;
        };
        let _ = map.map_mut().insert_map_object_record(record);
    }

    pub(crate) fn mutate_world_creature<F, R>(&mut self, guid: ObjectGuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut crate::map_manager::WorldCreature) -> R,
    {
        let mut f = Some(f);
        if let Some(manager) = self.map_manager.as_ref().cloned() {
            let result = {
                let mut manager = manager
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(creature) =
                    manager.find_creature_mut(self.player_map_id_like_cpp(), 0, guid)
                {
                    let result = f.take().expect("creature mutator is called once")(creature);
                    Some((result, creature.position(), creature.creature.clone()))
                } else {
                    None
                }
            };
            if let Some((result, position, creature)) = result {
                self.relocate_canonical_creature_map_object_like_cpp(guid, position);
                self.sync_canonical_creature_entity_like_cpp(creature);
                return Some(result);
            }
        }

        #[cfg(test)]
        {
            let mut legacy = self.creatures.remove(&guid)?;
            let mut creature =
                Self::test_world_creature_from_legacy(self.player_map_id_like_cpp(), &legacy);
            let result = f.take().expect("creature mutator is called once")(&mut creature);
            Self::sync_test_world_creature_to_legacy(&creature, &mut legacy);
            let position = creature.position();
            let canonical_creature = creature.creature.clone();
            self.creatures.insert(guid, legacy);
            self.relocate_canonical_creature_map_object_like_cpp(guid, position);
            self.sync_canonical_creature_entity_like_cpp(canonical_creature);
            return Some(result);
        }

        #[cfg(not(test))]
        {
            None
        }
    }

    pub(crate) fn world_creature_guids(&self) -> Vec<ObjectGuid> {
        if let Some(manager) = &self.map_manager {
            return manager
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .creature_guids(self.player_map_id_like_cpp(), 0);
        }

        #[cfg(test)]
        {
            return self.creatures.keys().copied().collect();
        }

        #[cfg(not(test))]
        {
            Vec::new()
        }
    }

    #[cfg(test)]
    fn test_world_creature_from_legacy(
        map_id: u16,
        legacy: &wow_ai::CreatureAI,
    ) -> crate::map_manager::WorldCreature {
        let mut creature = wow_entities::Creature::new(false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(legacy.guid);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .set_entry(legacy.entry);
        let _ = creature.unit_mut().world_mut().set_map(map_id as u32, 0);
        creature.unit_mut().world_mut().relocate(legacy.current_pos);
        creature.set_ai_home_position(legacy.home_pos);
        creature.unit_mut().set_level(legacy.level);
        creature.unit_mut().set_max_health(u64::from(legacy.max_hp));
        creature.unit_mut().set_health(u64::from(legacy.hp));
        creature.set_ai_identity_runtime(
            legacy.display_id,
            legacy.faction,
            legacy.npc_flags,
            legacy.unit_flags,
        );
        creature.configure_ai_runtime(
            legacy.home_pos,
            legacy.aggro_radius,
            legacy.wander_radius,
            legacy.respawn_time_secs,
        );
        {
            let ai = creature.ai_ownership_mut();
            ai.state = match legacy.state {
                wow_ai::CreatureState::Idle => wow_entities::CreatureAiState::Idle,
                wow_ai::CreatureState::WalkingRandom => {
                    wow_entities::CreatureAiState::WalkingRandom
                }
                wow_ai::CreatureState::WalkingWaypoint => {
                    wow_entities::CreatureAiState::WalkingWaypoint
                }
                wow_ai::CreatureState::InCombat => wow_entities::CreatureAiState::InCombat,
                wow_ai::CreatureState::Dead => wow_entities::CreatureAiState::Dead,
                wow_ai::CreatureState::Returning => wow_entities::CreatureAiState::Returning,
            };
            ai.move_target = legacy.move_target;
            ai.move_duration_ms = legacy.move_duration_ms;
            ai.combat_target = legacy.combat_target;
            ai.min_damage = legacy.min_dmg;
            ai.max_damage = legacy.max_dmg;
            ai.swing_timer_ms = legacy.swing_timer_ms;
            ai.spline_id = legacy.spline_id;
            ai.loot_id = legacy.loot_id;
            ai.gold_min = legacy.gold_min;
            ai.gold_max = legacy.gold_max;
            ai.boss_id = legacy.boss_id;
            ai.dungeon_encounter_id = legacy.dungeon_encounter_id;
        }
        if !legacy.is_alive {
            creature.mark_ai_dead(0);
        }

        crate::map_manager::WorldCreature::from_canonical(
            creature,
            wow_packet::packets::update::CreatureCreateData {
                guid: legacy.guid,
                entry: legacy.entry,
                display_id: legacy.display_id,
                native_display_id: legacy.display_id,
                health: legacy.max_hp as i64,
                max_health: legacy.max_hp as i64,
                level: legacy.level,
                faction_template: legacy.faction as i32,
                npc_flags: legacy.npc_flags as u64,
                unit_flags: legacy.unit_flags,
                unit_flags2: 0,
                unit_flags3: 0,
                scale: 1.0,
                unit_class: 1,
                base_attack_time: 2000,
                ranged_attack_time: 0,
                zone_id: 0,
                speed_walk_rate: 1.0,
                speed_run_rate: 1.14286,
            },
        )
    }

    #[cfg(test)]
    fn sync_test_world_creature_to_legacy(
        creature: &crate::map_manager::WorldCreature,
        legacy: &mut wow_ai::CreatureAI,
    ) {
        legacy.current_pos = creature.position();
        legacy.home_pos = creature.home_position();
        legacy.hp = creature.current_hp();
        legacy.max_hp = creature.max_hp();
        legacy.level = creature.level();
        legacy.state = match creature.state() {
            wow_entities::CreatureAiState::Idle => wow_ai::CreatureState::Idle,
            wow_entities::CreatureAiState::WalkingRandom => wow_ai::CreatureState::WalkingRandom,
            wow_entities::CreatureAiState::WalkingWaypoint => {
                wow_ai::CreatureState::WalkingWaypoint
            }
            wow_entities::CreatureAiState::InCombat => wow_ai::CreatureState::InCombat,
            wow_entities::CreatureAiState::Dead => wow_ai::CreatureState::Dead,
            wow_entities::CreatureAiState::Returning => wow_ai::CreatureState::Returning,
        };
        let ai = creature.creature.ai_ownership();
        legacy.move_target = ai.move_target;
        legacy.move_duration_ms = ai.move_duration_ms;
        legacy.combat_target = ai.combat_target;
        legacy.min_dmg = ai.min_damage;
        legacy.max_dmg = ai.max_damage;
        legacy.aggro_radius = ai.aggro_radius;
        legacy.wander_radius = ai.wander_radius;
        legacy.respawn_time_secs = ai.respawn_time_secs;
        legacy.npc_flags = ai.npc_flags;
        legacy.unit_flags = ai.unit_flags;
        legacy.display_id = ai.display_id;
        legacy.faction = ai.faction;
        legacy.spline_id = ai.spline_id;
        legacy.swing_timer_ms = ai.swing_timer_ms;
        legacy.loot_id = ai.loot_id;
        legacy.gold_min = ai.gold_min;
        legacy.gold_max = ai.gold_max;
        legacy.boss_id = ai.boss_id;
        legacy.dungeon_encounter_id = ai.dungeon_encounter_id;
        legacy.is_alive = creature.is_alive();
        legacy.corpse_despawn_at = creature.corpse_despawn_at();
    }

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

    pub fn set_battlenet_account_id(&mut self, battlenet_account_id: u32) {
        self.battlenet_account_id = battlenet_account_id;
    }

    pub fn battlenet_account_id(&self) -> u32 {
        self.battlenet_account_id
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

    /// Set the C++ ImportPrice*.db2 stores for this session.
    pub fn set_import_price_stores(&mut self, stores: Arc<ImportPriceStores>) {
        self.import_price_stores = Some(stores);
    }

    /// C++ `sImportPriceQualityStore.LookupEntry(quality + 1)`.
    pub fn import_price_quality_factor_like_cpp(&self, quality: u32) -> Option<f32> {
        self.import_price_stores
            .as_ref()
            .and_then(|stores| stores.quality.get(quality + 1))
            .map(|entry| entry.data)
    }

    /// Set the C++ ItemPriceBase.db2 store for this session.
    pub fn set_item_price_base_store(&mut self, store: Arc<ItemPriceBaseStore>) {
        self.item_price_base_store = Some(store);
    }

    /// C++ `sItemPriceBaseStore.LookupEntry(itemLevel)`.
    pub fn item_price_base_like_cpp(&self, item_level: u32) -> Option<(f32, f32)> {
        self.item_price_base_store
            .as_ref()
            .and_then(|store| store.get(item_level))
            .map(|entry| (entry.armor, entry.weapon))
    }

    /// C++ `Item::GetBuyPrice(proto, quality, itemLevel, standardPrice)`.
    ///
    /// This preserves the contrasted branch behavior where `standardPrice`
    /// remains false even after the calculated-price path.
    pub fn item_buy_price_like_cpp(
        &self,
        item_id: u32,
        quality: u32,
        item_level: u32,
    ) -> Option<(u32, bool)> {
        let basic = self.item_store.as_ref()?.get(item_id)?;
        let sparse = self.item_stats_store.as_ref()?.sparse_template(item_id)?;
        let flags2 = sparse.flags[1];
        let standard_price = false;

        if (flags2 & ItemFlags2::OverrideGoldCost as u32) != 0 {
            return Some((sparse.buy_price, standard_price));
        }

        let stores = self.import_price_stores.as_ref()?;
        let quality_price = match stores.quality.get(quality + 1) {
            Some(entry) => entry.data,
            None => return Some((0, standard_price)),
        };
        let (base_armor, base_weapon) = match self.item_price_base_like_cpp(item_level) {
            Some(base) => base,
            None => return Some((0, standard_price)),
        };

        let mut inventory_type =
            <InventoryType as num_traits::FromPrimitive>::from_i8(sparse.inventory_type)
                .unwrap_or(InventoryType::NonEquip);
        let mut base_factor = if matches!(
            inventory_type,
            InventoryType::Weapon
                | InventoryType::Weapon2Hand
                | InventoryType::WeaponMainhand
                | InventoryType::WeaponOffhand
                | InventoryType::Ranged
                | InventoryType::Thrown
                | InventoryType::RangedRight
        ) {
            base_weapon
        } else {
            base_armor
        };

        if inventory_type == InventoryType::Robe {
            inventory_type = InventoryType::Chest;
        }

        if basic.class_id == ItemClass::Gem as u8 && basic.subclass_id == 11 {
            inventory_type = InventoryType::Weapon;
            base_factor = base_weapon / 3.0;
        }

        let type_factor = match inventory_type {
            InventoryType::Head
            | InventoryType::Neck
            | InventoryType::Shoulders
            | InventoryType::Chest
            | InventoryType::Waist
            | InventoryType::Legs
            | InventoryType::Feet
            | InventoryType::Wrists
            | InventoryType::Hands
            | InventoryType::Finger
            | InventoryType::Trinket
            | InventoryType::Cloak
            | InventoryType::Holdable => {
                let armor_price = match stores.armor.get(inventory_type as u32) {
                    Some(entry) => entry,
                    None => return Some((0, standard_price)),
                };
                match basic.subclass_id {
                    0 | 1 => armor_price.cloth_modifier,
                    2 => armor_price.leather_modifier,
                    3 => armor_price.chain_modifier,
                    4 => armor_price.plate_modifier,
                    _ => 1.0,
                }
            }
            InventoryType::Shield => match stores.shield.get(2) {
                Some(entry) => entry.data,
                None => return Some((0, standard_price)),
            },
            InventoryType::WeaponMainhand => match stores.weapon.get(1) {
                Some(entry) => entry.data,
                None => return Some((0, standard_price)),
            },
            InventoryType::WeaponOffhand => match stores.weapon.get(2) {
                Some(entry) => entry.data,
                None => return Some((0, standard_price)),
            },
            InventoryType::Weapon => match stores.weapon.get(3) {
                Some(entry) => entry.data,
                None => return Some((0, standard_price)),
            },
            InventoryType::Weapon2Hand => match stores.weapon.get(4) {
                Some(entry) => entry.data,
                None => return Some((0, standard_price)),
            },
            InventoryType::Ranged | InventoryType::RangedRight | InventoryType::Relic => {
                match stores.weapon.get(5) {
                    Some(entry) => entry.data,
                    None => return Some((0, standard_price)),
                }
            }
            _ => return Some((sparse.buy_price, standard_price)),
        };

        let cost = sparse.price_variance
            * type_factor
            * base_factor
            * quality_price
            * sparse.price_random_value;
        Some((cost as u32, standard_price))
    }

    /// C++ `Item::GetSellPrice(proto, quality, itemLevel)`.
    pub fn item_sell_price_like_cpp(
        &self,
        item_id: u32,
        quality: u32,
        item_level: u32,
    ) -> Option<u32> {
        let basic = self.item_store.as_ref()?.get(item_id)?;
        let sparse = self.item_stats_store.as_ref()?.sparse_template(item_id)?;

        if (sparse.flags[1] & ItemFlags2::OverrideGoldCost as u32) != 0 {
            return Some(sparse.sell_price);
        }

        let (cost, standard_price) = self.item_buy_price_like_cpp(item_id, quality, item_level)?;
        if standard_price {
            let price_modifier =
                self.item_class_price_modifier_like_cpp(u32::from(basic.class_id))?;
            let buy_count = sparse.vendor_stack_count.max(1);
            Some((cost as f32 * price_modifier / buy_count as f32) as u32)
        } else {
            Some(sparse.sell_price)
        }
    }

    /// C++ `Item::GetDisenchantLoot`.
    ///
    /// `can_disenchant_bonus` represents `BonusData::CanDisenchant`, which is
    /// not yet a canonical Rust item-bonus subsystem.
    pub fn item_disenchant_loot_like_cpp(
        &self,
        item_id: u32,
        quality: u32,
        item_level: u32,
        can_disenchant_bonus: bool,
    ) -> Option<(u32, u16)> {
        if !can_disenchant_bonus {
            return None;
        }

        let basic = self.item_store.as_ref()?.get(item_id)?;
        let sparse = self.item_stats_store.as_ref()?.sparse_template(item_id)?;
        let item_flags = sparse.item_flags();

        if item_flags.contains(ItemFlags::CONJURED)
            || item_flags.contains(ItemFlags::NO_DISENCHANT)
            || sparse.bonding == ItemBondingType::Quest as u8
        {
            return None;
        }

        if sparse.zone_bound[0] != 0
            || sparse.zone_bound[1] != 0
            || sparse.instance_bound != 0
            || sparse.max_stack_size() > 1
        {
            return None;
        }

        if self.item_sell_price_like_cpp(item_id, quality, item_level) == Some(0)
            && !self.has_item_currency_cost_like_cpp(item_id)
        {
            return None;
        }

        let store = self.item_disenchant_loot_store.as_ref()?;
        store
            .find_for_item_like_cpp(
                u32::from(basic.class_id),
                basic.subclass_id as i8,
                quality as u8,
                item_level,
                sparse.required_expansion,
            )
            .map(|entry| (entry.id, entry.skill_required))
    }

    /// Set the item class store for this session.
    pub fn set_item_class_store(&mut self, store: Arc<ItemClassStore>) {
        self.item_class_store = Some(store);
    }

    /// C++ `sDB2Manager.GetItemClassByOldEnum(itemClass)`.
    pub fn item_class_price_modifier_like_cpp(&self, item_class: u32) -> Option<f32> {
        self.item_class_store
            .as_ref()
            .and_then(|store| store.get_by_old_enum(item_class))
            .map(|entry| entry.price_modifier)
    }

    /// Set the item currency cost store for this session.
    pub fn set_item_currency_cost_store(&mut self, store: Arc<ItemCurrencyCostStore>) {
        self.item_currency_cost_store = Some(store);
    }

    /// C++ `sDB2Manager.HasItemCurrencyCost(itemId)`.
    pub fn has_item_currency_cost_like_cpp(&self, item_id: u32) -> bool {
        self.item_currency_cost_store
            .as_ref()
            .is_some_and(|store| store.has_item_currency_cost(item_id))
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

    /// Resolve C++ `ItemTemplate::GetRandomSelect()`.
    pub(crate) fn item_template_random_select(&self, item_id: u32) -> u16 {
        self.item_store
            .as_ref()
            .map(|store| store.random_select(item_id))
            .unwrap_or(0)
    }

    /// Resolve C++ `ItemTemplate::GetRandomSuffixGroupID()`.
    pub(crate) fn item_template_random_suffix_group_id(&self, item_id: u32) -> u16 {
        self.item_store
            .as_ref()
            .map(|store| store.random_suffix_group_id(item_id))
            .unwrap_or(0)
    }

    /// Get the item store reference.
    pub fn item_store(&self) -> Option<&Arc<ItemStore>> {
        self.item_store.as_ref()
    }

    /// C++ `Player::GetCurrencyQuantity`.
    pub(crate) fn player_currency_quantity(&self, currency_id: u32) -> u32 {
        self.player_currencies_like_cpp()
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

        let player_team = player_team_for_race_cpp(self.player_race_like_cpp());
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

        let mut currencies = self.player_currencies_like_cpp().clone();
        let currency = currencies.entry(currency_id).or_insert(PlayerCurrency {
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
        let delta = PlayerCurrencyDelta {
            currency_id,
            quantity: currency.quantity,
            amount: applied,
            weekly_quantity: ((currency.weekly_quantity / scaler) > 0)
                .then_some(currency.weekly_quantity),
            max_quantity: (max_quantity != 0).then_some(max_quantity),
            total_earned: entry.has_total_earned().then_some(currency.earned_quantity),
            suppress_chat_log: entry.is_suppressing_chat_log(false),
        };
        self.set_player_currencies_like_cpp(currencies);
        Ok(Some(delta))
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

        let player_team = player_team_for_race_cpp(self.player_race_like_cpp());
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

        let mut currencies = self.player_currencies_like_cpp().clone();
        let currency = currencies.entry(currency_id).or_insert(PlayerCurrency {
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
        let delta = PlayerCurrencyDelta {
            currency_id,
            quantity: currency.quantity,
            amount,
            weekly_quantity: ((currency.weekly_quantity / scaler) > 0)
                .then_some(currency.weekly_quantity),
            max_quantity: (max_quantity != 0).then_some(max_quantity),
            total_earned: entry.has_total_earned().then_some(currency.earned_quantity),
            suppress_chat_log: entry.is_suppressing_chat_log(false),
        };
        self.set_player_currencies_like_cpp(currencies);
        Ok(Some(delta))
    }

    /// C++ `Player::RemoveCurrency` underflow guard for vendor costs.
    pub(crate) fn remove_currency(&mut self, currency_id: u32, amount: u32) -> bool {
        if amount == 0 {
            return true;
        }

        let mut currencies = self.player_currencies_like_cpp().clone();
        let Some(currency) = currencies.get_mut(&currency_id) else {
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
        self.set_player_currencies_like_cpp(currencies);
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
        let mut currencies = self.player_currencies_like_cpp().clone();
        for (&currency_id, currency) in &mut currencies {
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
        self.set_player_currencies_like_cpp(currencies);
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
    pub fn set_item_modified_appearance_store(&mut self, store: Arc<ItemModifiedAppearanceStore>) {
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

    pub fn set_loot_drop_rates_like_cpp(&mut self, rates: LootDropRatesLikeCpp) {
        self.loot_drop_rates = rates;
    }

    pub fn set_enable_ae_loot_like_cpp(&mut self, enabled: bool) {
        self.enable_ae_loot_like_cpp = enabled;
    }

    pub fn set_mmap_runtime_config_like_cpp(&mut self, config: MMapRuntimeConfigLikeCpp) {
        self.mmap_runtime_config_like_cpp = config;
    }

    pub(crate) fn enable_ae_loot_like_cpp(&self) -> bool {
        self.enable_ae_loot_like_cpp
    }

    pub fn mmap_runtime_config_like_cpp(&self) -> &MMapRuntimeConfigLikeCpp {
        &self.mmap_runtime_config_like_cpp
    }

    pub fn loot_drop_rates_like_cpp(&self) -> LootDropRatesLikeCpp {
        self.loot_drop_rates
    }

    pub(crate) fn item_drop_rate_like_cpp(&self, item_id: u32) -> f32 {
        let quality = self
            .item_template_quality(item_id)
            .and_then(<ItemQuality as num_traits::FromPrimitive>::from_i8);
        match quality {
            Some(ItemQuality::Poor) => self.loot_drop_rates.item_poor,
            Some(ItemQuality::Normal) => self.loot_drop_rates.item_normal,
            Some(ItemQuality::Uncommon) => self.loot_drop_rates.item_uncommon,
            Some(ItemQuality::Rare) => self.loot_drop_rates.item_rare,
            Some(ItemQuality::Epic) => self.loot_drop_rates.item_epic,
            Some(ItemQuality::Legendary) => self.loot_drop_rates.item_legendary,
            Some(ItemQuality::Artifact) => self.loot_drop_rates.item_artifact,
            _ => 1.0,
        }
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

    /// Resolve C++ `ItemTemplate::ExtendedData->Flags[1]`.
    pub fn item_template_flags2(&self, item_id: u32) -> Option<u32> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.sparse_template(item_id))
            .map(|template| template.flags[1])
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

    pub fn item_template_start_quest_id(&self, item_id: u32) -> Option<i32> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.sparse_template(item_id))
            .map(|template| template.start_quest_id)
    }

    pub fn item_template_quality(&self, item_id: u32) -> Option<i8> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.random_property_template(item_id))
            .map(|template| template.quality)
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

    /// Resolve C++ `ItemSparseEntry` data used by random-property generation.
    pub(crate) fn item_random_property_template(
        &self,
        item_id: u32,
    ) -> Option<ItemRandomPropertyTemplateEntry> {
        self.item_stats_store
            .as_ref()
            .and_then(|store| store.random_property_template(item_id))
            .copied()
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
        let item_guid = item.object().guid();
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.item_objects.insert(item_guid, item)
        })
    }

    pub(crate) fn update_inventory_item_object_like_cpp(
        &mut self,
        item_guid: ObjectGuid,
        update: impl FnOnce(&mut Item),
    ) -> bool {
        let mut update = Some(update);
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            let Some(item) = inventory.item_objects.get_mut(&item_guid) else {
                return false;
            };
            if let Some(update) = update.take() {
                update(item);
            }
            true
        })
    }

    pub(crate) fn remove_inventory_item_object(&mut self, item_guid: ObjectGuid) -> Option<Item> {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.item_objects.remove(&item_guid)
        })
    }

    pub(crate) fn set_inventory_item_object_slot(&mut self, item_guid: ObjectGuid, slot: u8) {
        self.update_inventory_item_object_like_cpp(item_guid, |item| {
            item.set_slot(slot);
        });
    }

    pub(crate) fn clear_inventory_items_and_objects_like_cpp(&mut self) {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.inventory_items.clear();
            inventory.item_objects.clear();
        });
    }

    pub(crate) fn clear_all_inventory_runtime_like_cpp(&mut self) {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            *inventory = SessionPlayerInventoryRuntime::default();
        });
    }

    pub(crate) fn clear_buyback_runtime_like_cpp(&mut self) {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.buyback_items.clear();
            inventory.buyback_price = [0; BUYBACK_SLOT_COUNT];
            inventory.buyback_timestamp = [0; BUYBACK_SLOT_COUNT];
            inventory.current_buyback_slot = BUYBACK_SLOT_START;
        });
    }

    pub(crate) fn insert_inventory_item_like_cpp(
        &mut self,
        slot: u8,
        item: InventoryItem,
    ) -> Option<InventoryItem> {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.inventory_items.insert(slot, item)
        })
    }

    pub(crate) fn remove_inventory_item_like_cpp(&mut self, slot: u8) -> Option<InventoryItem> {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.inventory_items.remove(&slot)
        })
    }

    pub(crate) fn insert_buyback_item_like_cpp(
        &mut self,
        slot: u8,
        item: InventoryItem,
    ) -> Option<InventoryItem> {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.buyback_items.insert(slot, item)
        })
    }

    pub(crate) fn remove_buyback_item_like_cpp(&mut self, slot: u8) -> Option<InventoryItem> {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.buyback_items.remove(&slot)
        })
    }

    pub(crate) fn set_current_buyback_slot_like_cpp(&mut self, slot: u8) {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.current_buyback_slot = slot;
        });
    }

    pub(crate) fn set_buyback_slot_metadata_like_cpp(
        &mut self,
        slot: u8,
        price: u32,
        timestamp: i64,
    ) {
        if !(BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&slot) {
            return;
        }
        let index = (slot - BUYBACK_SLOT_START) as usize;
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            inventory.buyback_price[index] = price;
            inventory.buyback_timestamp[index] = timestamp;
        });
    }

    pub(crate) fn clear_buyback_slot_metadata_like_cpp(&mut self, slot: u8) {
        self.set_buyback_slot_metadata_like_cpp(slot, 0, 0);
    }

    pub(crate) fn update_inventory_item_metadata_like_cpp(
        &mut self,
        slot: u8,
        item_guid: ObjectGuid,
        entry_id: u32,
        inventory_type: Option<u8>,
    ) -> bool {
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            let Some(inventory_item) = inventory
                .inventory_items
                .get_mut(&slot)
                .filter(|inventory_item| inventory_item.guid == item_guid)
            else {
                return false;
            };
            inventory_item.entry_id = entry_id;
            inventory_item.inventory_type = inventory_type;
            true
        })
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
                .inventory_items_like_cpp()
                .get(&slot)
                .is_some_and(|item| item.guid == item_guid)
        {
            self.remove_inventory_item_like_cpp(slot);
        }
        self.remove_inventory_item_object(item_guid);
        self.sync_object_accessor_player();
        self.sync_player_registry_state_like_cpp();
    }

    pub(crate) fn represented_inventory_item_counts_like_cpp(&self) -> HashMap<u32, u32> {
        let inventory_items = self.inventory_items_like_cpp();
        let item_objects = self.inventory_item_objects_like_cpp();
        inventory_items
            .values()
            .filter_map(|inventory_item| item_objects.get(&inventory_item.guid))
            .chain(item_objects.values().filter(|item| {
                !item.container_guid().is_empty()
                    && item_objects.contains_key(&item.container_guid())
            }))
            .filter(|item| !item.is_in_trade())
            .fold(HashMap::new(), |mut counts, item| {
                let entry_id = item.object().entry();
                counts
                    .entry(entry_id)
                    .and_modify(|count| *count = count.saturating_add(item.count()))
                    .or_insert(item.count());
                counts
            })
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
            self.inventory_items_like_cpp().get(&slot).cloned()
        } else if is_represented_bag_slot(bag) {
            let bag_item = self.inventory_items_like_cpp().get(&bag)?;
            let bag_guid = bag_item.guid;
            let nested = self
                .inventory_item_objects_like_cpp()
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
        let buyback_items = self.buyback_items_like_cpp();
        let buyback_timestamp = self.buyback_timestamp_like_cpp();
        let mut slot = self.current_buyback_slot_like_cpp();
        if buyback_items.contains_key(&slot) {
            let mut oldest_slot = BUYBACK_SLOT_START;
            let mut oldest_time = buyback_timestamp[0];

            for candidate in BUYBACK_SLOT_START + 1..BUYBACK_SLOT_END {
                let candidate_index = (candidate - BUYBACK_SLOT_START) as usize;
                if !buyback_items.contains_key(&candidate) {
                    oldest_slot = candidate;
                    break;
                }
                let candidate_time = buyback_timestamp[candidate_index];
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
        self.mutate_player_inventory_runtime_like_cpp(|inventory| {
            if inventory.current_buyback_slot < BUYBACK_SLOT_END - 1 {
                inventory.current_buyback_slot += 1;
            }
        });
    }

    fn direct_inventory_player_snapshot(&self) -> Option<Player> {
        let player_guid = self.player_guid()?;
        let mut player = Player::new(None, false);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);

        for (&slot, item) in self.inventory_items_like_cpp() {
            if (slot as usize) < PLAYER_SLOT_END && !Self::is_buyback_slot(slot) {
                let _ = player.store_top_level_item(slot, item.guid);
            }
        }

        Some(player)
    }

    fn player_values_update_snapshot(&self) -> Option<Player> {
        let mut player = self.direct_inventory_player_snapshot()?;
        player.set_money(self.player_gold_like_cpp());
        let inventory_items = self.inventory_items_like_cpp();
        let buyback_items = self.buyback_items_like_cpp();
        let buyback_price = self.buyback_price_like_cpp();
        let buyback_timestamp = self.buyback_timestamp_like_cpp();

        for slot in 0..19u8 {
            let visible = inventory_items.get(&slot).map(|item| VisibleItemValues {
                item_id: item.entry_id as i32,
                item_appearance_mod_id: 0,
                item_visual: 0,
            });
            player.set_visible_item_slot(slot, visible);
        }

        for slot in 15..=17u8 {
            let visible = inventory_items.get(&slot).map(|item| VisibleItemValues {
                item_id: item.entry_id as i32,
                item_appearance_mod_id: 0,
                item_visual: 0,
            });
            player
                .unit_mut()
                .set_virtual_item((slot - 15) as usize, visible);
        }

        for (&slot, item) in buyback_items {
            if (slot as usize) < PLAYER_SLOT_END {
                player.set_inv_slot(slot as usize, item.guid);
            }
        }
        for index in 0..BUYBACK_SLOT_COUNT {
            player.set_buyback_price(index, buyback_price[index]);
            player.set_buyback_timestamp(index, buyback_timestamp[index]);
        }

        player.clear_data_changes();
        Some(player)
    }

    pub(crate) fn send_player_values_update_from_entity_bridge(
        &self,
        inv_slot_changes: &[(u8, ObjectGuid)],
        visible_item_changes: &[(u8, i32, u16, u16)],
        virtual_item_changes: &[(u8, i32, u16, u16)],
        buyback_changes: &[(u8, u32, i64)],
        coinage: Option<u64>,
    ) {
        let Some(guid) = self.player_guid() else {
            return;
        };
        let Some(mut player) = self.player_values_update_snapshot() else {
            return;
        };

        if let Some(coinage) = coinage {
            player.set_money(coinage);
            player.mark_money_changed();
        }

        for &(slot, item_guid) in inv_slot_changes {
            player.set_inv_slot(slot as usize, item_guid);
            player.mark_inv_slot_changed(slot as usize);
        }

        for &(slot, item_id, appearance_mod_id, item_visual) in visible_item_changes {
            let visible = (item_id != 0 || appearance_mod_id != 0 || item_visual != 0).then_some(
                VisibleItemValues {
                    item_id,
                    item_appearance_mod_id: appearance_mod_id,
                    item_visual,
                },
            );
            player.set_visible_item_slot(slot, visible);
            player.mark_visible_item_slot_changed(slot);
        }

        for &(index, item_id, appearance_mod_id, item_visual) in virtual_item_changes {
            let visible = (item_id != 0 || appearance_mod_id != 0 || item_visual != 0).then_some(
                VisibleItemValues {
                    item_id,
                    item_appearance_mod_id: appearance_mod_id,
                    item_visual,
                },
            );
            player.unit_mut().set_virtual_item(index as usize, visible);
            player.unit_mut().mark_virtual_item_changed(index as usize);
        }

        for &(slot, price, timestamp) in buyback_changes {
            if !(BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&slot) {
                continue;
            }
            let index = (slot - BUYBACK_SLOT_START) as usize;
            player.set_buyback_price(index, price);
            player.mark_buyback_price_changed(index);
            player.set_buyback_timestamp(index, timestamp);
            player.mark_buyback_timestamp_changed(index);
        }

        let update = player.values_update(true);
        if let Some(packet) =
            player_values_update_to_update_object(guid, self.player_map_id_like_cpp(), &update)
        {
            self.send_packet(&packet);
        }
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
        self.inventory_item_objects_like_cpp()
            .values()
            .any(|item| item.container_guid() == item_guid)
    }

    pub(crate) fn represented_bag_contains_active_item_loot_like_cpp(
        &self,
        bag_guid: ObjectGuid,
    ) -> bool {
        if self.active_loot_view_owners.is_empty() {
            return false;
        }

        self.inventory_item_objects_like_cpp().values().any(|item| {
            item.container_guid() == bag_guid
                && self.active_loot_view_owners.contains(&item.object().guid())
                && self.loot_table.contains_key(&item.object().guid())
        })
    }

    pub(crate) fn set_active_loot_guid(&mut self, guid: ObjectGuid) {
        self.active_loot_guid = ObjectGuid::EMPTY;
        self.active_loot_view_owners.clear();
        self.add_active_loot_view_owner_like_cpp(guid);
    }

    pub(crate) fn add_active_loot_view_owner_like_cpp(&mut self, guid: ObjectGuid) {
        if guid.is_empty() {
            return;
        }

        if self.active_loot_guid.is_empty() {
            self.active_loot_guid = guid;
        }

        self.active_loot_view_owners.insert(guid);
    }

    pub(crate) fn clear_active_loot_guid_if(&mut self, guid: ObjectGuid) {
        self.active_loot_view_owners.remove(&guid);
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
        let inventory_items = self.inventory_items_like_cpp();
        let item_objects = self.inventory_item_objects_like_cpp();
        let mut template_cache = HashMap::new();
        for (&slot, item) in inventory_items {
            if Self::is_buyback_slot(slot) {
                continue;
            }
            if let Some(template) = self.item_storage_template(item.entry_id) {
                template_cache.insert(item.entry_id, template);
            }
        }

        let mut slot_items = Vec::new();
        let mut stored_items = Vec::new();
        for (&slot, inventory_item) in inventory_items {
            if Self::is_buyback_slot(slot) {
                continue;
            }
            let Some(item) = item_objects.get(&inventory_item.guid) else {
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
        let outcome = player.can_store_item(
            &mut dest,
            CanStoreItemArgs {
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
            },
        );

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

    /// Set the item random properties store for this session.
    pub fn set_item_random_properties_store(&mut self, store: Arc<ItemRandomPropertiesStore>) {
        self.item_random_properties_store = Some(store);
    }

    /// Get the item random properties store reference.
    pub fn item_random_properties_store(&self) -> Option<&Arc<ItemRandomPropertiesStore>> {
        self.item_random_properties_store.as_ref()
    }

    /// Set the random property points store for this session.
    pub fn set_rand_prop_points_store(&mut self, store: Arc<RandPropPointsStore>) {
        self.rand_prop_points_store = Some(store);
    }

    /// Get the random property points store reference.
    pub fn rand_prop_points_store(&self) -> Option<&Arc<RandPropPointsStore>> {
        self.rand_prop_points_store.as_ref()
    }

    /// Set the item random enchantment template store for this session.
    pub fn set_item_random_enchantment_template_store(
        &mut self,
        store: Arc<ItemRandomEnchantmentTemplateStore>,
    ) {
        self.item_random_enchantment_template_store = Some(store);
    }

    /// Get the item random enchantment template store reference.
    pub fn item_random_enchantment_template_store(
        &self,
    ) -> Option<&Arc<ItemRandomEnchantmentTemplateStore>> {
        self.item_random_enchantment_template_store.as_ref()
    }

    /// Set the item disenchant loot store for this session.
    pub fn set_item_disenchant_loot_store(&mut self, store: Arc<ItemDisenchantLootStore>) {
        self.item_disenchant_loot_store = Some(store);
    }

    /// Get the item disenchant loot store reference.
    pub fn item_disenchant_loot_store(&self) -> Option<&Arc<ItemDisenchantLootStore>> {
        self.item_disenchant_loot_store.as_ref()
    }

    /// Set the C++ LootTemplates_* foundation stores for this session.
    pub fn set_loot_stores(&mut self, stores: Arc<LootStores>) {
        self.loot_stores = Some(stores);
    }

    /// Get the C++ LootTemplates_* foundation stores.
    pub fn loot_stores(&self) -> Option<&Arc<LootStores>> {
        self.loot_stores.as_ref()
    }

    /// Set the C++ ConditionMgr store loaded from the `conditions` table.
    pub fn set_condition_store(&mut self, store: Arc<ConditionEntriesByTypeStore>) {
        self.condition_store = Some(store);
    }

    /// Get the loaded ConditionMgr store reference.
    pub fn condition_store(&self) -> Option<&Arc<ConditionEntriesByTypeStore>> {
        self.condition_store.as_ref()
    }

    /// Set the C++ PlayerCondition.db2 store for this session.
    pub fn set_player_condition_store(&mut self, store: Arc<PlayerConditionStore>) {
        self.player_condition_store = Some(store);
    }

    /// Get the loaded PlayerCondition.db2 store reference.
    pub fn player_condition_store(&self) -> Option<&Arc<PlayerConditionStore>> {
        self.player_condition_store.as_ref()
    }

    /// Set the C++ DisableMgr store loaded from the `disables` table.
    pub fn set_disable_mgr(&mut self, store: Arc<DisableMgrLikeCpp>) {
        self.disable_mgr = Some(store);
    }

    /// Get the loaded DisableMgr store reference.
    pub fn disable_mgr(&self) -> Option<&Arc<DisableMgrLikeCpp>> {
        self.disable_mgr.as_ref()
    }

    /// Set the lock store for this session.
    pub fn set_lock_store(&mut self, store: Arc<LockStore>) {
        self.lock_store = Some(store);
    }

    pub(crate) fn lock_store(&self) -> Option<&Arc<LockStore>> {
        self.lock_store.as_ref()
    }

    /// C++ `sLockStore.LookupEntry(lockId)`.
    pub fn lock_entry_exists_like_cpp(&self, lock_id: u32) -> bool {
        self.lock_store
            .as_ref()
            .is_some_and(|store| store.contains(lock_id))
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
                    match <ItemEnchantmentType as num_traits::FromPrimitive>::from_u8(
                        entry.effect[index],
                    ) {
                        Some(effect_type) => {
                            ApplyEnchantmentEffectRef::known(effect_type, amount, arg)
                        }
                        None => ApplyEnchantmentEffectRef::unknown(
                            u32::from(entry.effect[index]),
                            amount,
                            arg,
                        ),
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

    /// Set the ChrSpecialization store for this session.
    pub fn set_chr_specialization_store(&mut self, store: Arc<ChrSpecializationStore>) {
        self.chr_specialization_store = Some(store);
    }

    /// Get the ChrSpecialization store reference.
    pub fn chr_specialization_store(&self) -> Option<&Arc<ChrSpecializationStore>> {
        self.chr_specialization_store.as_ref()
    }

    /// Set the DungeonEncounter store for this session.
    pub fn set_dungeon_encounter_store(&mut self, store: Arc<DungeonEncounterStore>) {
        self.dungeon_encounter_store = Some(store);
    }

    /// Get the DungeonEncounter store reference.
    pub fn dungeon_encounter_store(&self) -> Option<&Arc<DungeonEncounterStore>> {
        self.dungeon_encounter_store.as_ref()
    }

    pub fn set_map_store(&mut self, store: Arc<MapStore>) {
        self.map_store = Some(store);
    }

    pub(crate) fn map_store(&self) -> Option<&Arc<MapStore>> {
        self.map_store.as_ref()
    }

    pub fn set_map_difficulty_store(&mut self, store: Arc<MapDifficultyStore>) {
        self.map_difficulty_store = Some(store);
    }

    pub fn set_map_difficulty_x_condition_store(
        &mut self,
        store: Arc<MapDifficultyXConditionStore>,
    ) {
        self.map_difficulty_x_condition_store = Some(store);
    }

    pub fn set_creature_template_mount_store(
        &mut self,
        store: Arc<CreatureTemplateMountStoreLikeCpp>,
    ) {
        self.creature_template_mount_store = Some(store);
    }

    pub fn set_creature_display_info_store(&mut self, store: Arc<CreatureDisplayInfoStore>) {
        self.creature_display_info_store = Some(store);
    }

    pub fn set_gameobject_display_info_store(&mut self, store: Arc<GameObjectDisplayInfoStore>) {
        self.gameobject_display_info_store = Some(store);
    }

    pub(crate) fn gameobject_display_info_store(&self) -> Option<&Arc<GameObjectDisplayInfoStore>> {
        self.gameobject_display_info_store.as_ref()
    }

    pub fn set_creature_model_data_store(&mut self, store: Arc<CreatureModelDataStore>) {
        self.creature_model_data_store = Some(store);
    }

    pub fn set_mount_store(&mut self, store: Arc<MountStore>) {
        self.mount_store = Some(store);
    }

    pub(crate) fn mount_store(&self) -> Option<&Arc<MountStore>> {
        self.mount_store.as_ref()
    }

    pub fn set_mount_capability_store(&mut self, store: Arc<MountCapabilityStore>) {
        self.mount_capability_store = Some(store);
    }

    pub fn set_mount_type_x_capability_store(&mut self, store: Arc<MountTypeXCapabilityStore>) {
        self.mount_type_x_capability_store = Some(store);
    }

    pub fn set_mount_x_display_store(&mut self, store: Arc<MountXDisplayStore>) {
        self.mount_x_display_store = Some(store);
    }

    pub fn set_vehicle_store(&mut self, store: Arc<VehicleStore>) {
        self.vehicle_store = Some(store);
    }

    pub fn set_vehicle_seat_store(&mut self, store: Arc<VehicleSeatStore>) {
        self.vehicle_seat_store = Some(store);
    }

    pub fn set_vehicle_template_store(&mut self, store: Arc<VehicleTemplateStoreLikeCpp>) {
        self.vehicle_template_store = Some(store);
    }

    pub fn set_vehicle_accessory_store(&mut self, store: Arc<VehicleAccessoryStoreLikeCpp>) {
        self.vehicle_accessory_store = Some(store);
    }

    #[allow(dead_code)]
    pub(crate) fn mount_capability_store(&self) -> Option<&Arc<MountCapabilityStore>> {
        self.mount_capability_store.as_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn mount_type_x_capability_store(&self) -> Option<&Arc<MountTypeXCapabilityStore>> {
        self.mount_type_x_capability_store.as_ref()
    }

    #[allow(dead_code)]
    pub(crate) fn mount_x_display_store(&self) -> Option<&Arc<MountXDisplayStore>> {
        self.mount_x_display_store.as_ref()
    }

    pub fn set_terrain_swap_store(&mut self, store: Arc<wow_data::TerrainSwapStore>) {
        self.terrain_swap_store = Some(store);
    }

    pub fn set_phase_store(&mut self, store: Arc<PhaseStore>) {
        self.phase_store = Some(store);
    }

    pub fn set_phase_group_store(&mut self, store: Arc<PhaseGroupStore>) {
        self.phase_group_store = Some(store);
    }

    pub(crate) fn record_represented_gameobject_db_phase_shift_like_cpp(
        &mut self,
        guid: ObjectGuid,
        map_id: u16,
        phase_use_flags: u8,
        phase_id: u16,
        phase_group_id: u32,
        terrain_swap_map: i32,
    ) {
        let (phase_shift, _) = self.db_spawn_phase_shift_like_cpp(
            map_id,
            phase_use_flags,
            phase_id,
            phase_group_id,
            terrain_swap_map,
        );
        self.record_represented_gameobject_phase_shift_like_cpp(guid, phase_shift);
    }

    pub(crate) fn record_represented_gameobject_phase_shift_like_cpp(
        &mut self,
        guid: ObjectGuid,
        phase_shift: PhaseShift,
    ) {
        self.represented_gameobject_phase_shifts
            .insert(guid, phase_shift);
    }

    pub(crate) fn db_spawn_phase_shift_like_cpp(
        &self,
        map_id: u16,
        phase_use_flags: u8,
        phase_id: u16,
        phase_group_id: u32,
        terrain_swap_map: i32,
    ) -> (PhaseShift, i32) {
        let mut phase_shift = PhaseShift::default();
        if let (Some(phase_store), Some(phase_group_store)) =
            (&self.phase_store, &self.phase_group_store)
        {
            init_db_phase_shift_like_cpp(
                &mut phase_shift,
                phase_store,
                phase_group_store,
                phase_use_flags,
                phase_id,
                phase_group_id,
            );
        }

        let mut validated_terrain_swap_map = -1;
        if let (Some(map_store), Some(terrain_swap_store)) =
            (&self.map_store, &self.terrain_swap_store)
            && let Some(terrain_swap_map) = terrain_swap_store.validate_spawn_terrain_swap_like_cpp(
                map_store,
                u32::from(map_id),
                terrain_swap_map,
            )
        {
            init_db_visible_map_id_like_cpp(
                &mut phase_shift,
                terrain_swap_store,
                i32::try_from(terrain_swap_map).unwrap_or(-1),
            );
            validated_terrain_swap_map = i32::try_from(terrain_swap_map).unwrap_or(-1);
        }

        (phase_shift, validated_terrain_swap_map)
    }

    pub(crate) fn represented_player_phase_shift_like_cpp(&self) -> &PhaseShift {
        &self.represented_player_phase_shift
    }

    #[cfg(test)]
    pub(crate) fn set_represented_player_phase_shift_like_cpp(&mut self, phase_shift: PhaseShift) {
        self.represented_player_phase_shift = phase_shift;
    }

    pub(crate) fn can_see_phase_shift_like_cpp(&self, other: &PhaseShift) -> bool {
        self.represented_player_phase_shift_like_cpp()
            .can_see(other)
    }

    pub(crate) fn map_difficulty_store(&self) -> Option<&Arc<MapDifficultyStore>> {
        self.map_difficulty_store.as_ref()
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

    pub fn set_spell_misc_store(&mut self, store: Arc<SpellMiscStore>) {
        self.spell_misc_store = Some(store);
    }

    pub(crate) fn spell_misc_store(&self) -> Option<&Arc<SpellMiscStore>> {
        self.spell_misc_store.as_ref()
    }

    pub fn set_spell_range_store(&mut self, store: Arc<SpellRangeStore>) {
        self.spell_range_store = Some(store);
    }

    pub(crate) fn spell_range_store(&self) -> Option<&Arc<SpellRangeStore>> {
        self.spell_range_store.as_ref()
    }

    pub fn set_area_table_store(&mut self, store: Arc<AreaTableStore>) {
        self.area_table_store = Some(store);
    }

    /// Set the quest store shared reference.
    pub fn set_quest_store(&mut self, store: Arc<wow_data::quest::QuestStore>) {
        self.quest_store = Some(store);
    }

    /// Save current player gold to the characters DB.
    pub(crate) async fn save_player_gold(&self) {
        use wow_database::CharStatements;
        let guid = match self.player_guid() {
            Some(g) => g.counter() as u32,
            None => return,
        };
        let char_db = match self.char_db() {
            Some(db) => Arc::clone(db),
            None => return,
        };
        let mut stmt = char_db.prepare(CharStatements::UPD_CHAR_MONEY);
        stmt.set_u64(0, self.player_gold_like_cpp());
        stmt.set_u32(1, guid);
        let _ = char_db.execute(&stmt).await;
    }

    /// Give XP to the player, leveling up if threshold reached.
    /// C# ref: Player.GiveXP(xp, victim)
    pub(crate) async fn give_xp(&mut self, xp: u32, victim: wow_core::ObjectGuid, is_kill: bool) {
        use wow_packet::ServerPacket;
        use wow_packet::packets::misc::{LevelUpInfo, LogXpGain};

        if xp == 0 {
            return;
        }
        if self.player_level_like_cpp() >= 80 {
            return;
        } // max level

        // Send floating XP text — C# LogXPGain
        self.send_packet(&LogXpGain {
            victim,
            original: xp as i32,
            reason: if is_kill { 0 } else { 1 },
            amount: xp as i32,
            group_bonus: 1.0,
        });

        self.set_player_xp_like_cpp(self.player_xp_like_cpp().saturating_add(xp));

        // Level up loop — C# while (newXP >= nextLvlXP && !IsMaxLevel())
        while self.player_xp_like_cpp() >= self.player_next_level_xp_like_cpp()
            && self.player_level_like_cpp() < 80
        {
            self.set_player_xp_like_cpp(
                self.player_xp_like_cpp() - self.player_next_level_xp_like_cpp(),
            );
            let new_level = self.player_level_like_cpp() + 1;

            info!(account = self.account_id, new_level, "Player leveled up");

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

            self.set_player_level_like_cpp(new_level);
            self.refresh_next_level_xp();

            // Persist new level to DB
            if let Some(guid) = self.player_guid() {
                let char_db = self.char_db().map(Arc::clone);
                if let Some(db) = char_db {
                    use wow_database::CharStatements;
                    let mut stmt = db.prepare(CharStatements::UPD_CHAR_LEVEL);
                    stmt.set_u8(0, self.player_level_like_cpp());
                    stmt.set_u32(1, self.player_xp_like_cpp());
                    stmt.set_u32(2, guid.counter() as u32);
                    let _ = db.execute(&stmt).await;
                }
            }
        }

        // Persist current XP
        if let Some(guid) = self.player_guid() {
            let char_db = self.char_db().map(Arc::clone);
            if let Some(db) = char_db {
                use wow_database::CharStatements;
                let mut stmt = db.prepare(CharStatements::UPD_CHAR_XP);
                stmt.set_u32(0, self.player_xp_like_cpp());
                stmt.set_u32(1, guid.counter() as u32);
                let _ = db.execute(&stmt).await;
            }
        }
    }

    /// XP reward for killing a creature.
    /// C# ref: Formulas.XPGain / Formulas.BaseGain
    pub(crate) fn creature_kill_xp(&self, mob_level: u8) -> u32 {
        let pl = self.player_level_like_cpp() as i32;
        let ml = mob_level as i32;

        // nBaseExp by content level (WotLK = 71-80 content)
        let n_base_exp: i32 = if pl >= 71 {
            580
        } else if pl >= 61 {
            235
        } else {
            45
        };

        // Gray level check
        let gray = self.gray_level(pl as u8) as i32;
        if ml <= gray {
            return 0;
        }

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
        let g = if p <= 5 {
            0
        } else if p <= 39 {
            p - 5 - p / 10
        } else if p <= 59 {
            p - 1 - p / 5
        } else {
            p - 9
        };
        g.max(0) as u8
    }

    /// Zero-difference table — C# Formulas.GetZeroDifference
    fn zero_difference(&self, pl: u8) -> u8 {
        match pl {
            0..=3 => 5,
            4..=9 => 6,
            10..=11 => 7,
            12..=15 => 8,
            16..=19 => 9,
            20..=29 => 11,
            30..=39 => 12,
            40..=44 => 13,
            45..=49 => 14,
            50..=54 => 15,
            55..=59 => 16,
            _ => 17,
        }
    }

    /// Called when the player kills a creature. Checks all active kill-objective quests
    /// and updates progress. Sends SMSG_QUEST_UPDATE_ADD_CREDIT if progress was made.
    pub(crate) async fn on_creature_killed(
        &mut self,
        creature_entry: u32,
        creature_guid: wow_core::ObjectGuid,
    ) {
        use wow_packet::ServerPacket;
        use wow_packet::packets::quest::{QuestUpdateAddCredit, QuestUpdateComplete};

        let Some(store) = self.quest_store.clone() else {
            return;
        };

        // Objective type 0 = Monster/NPC kill
        const OBJ_TYPE_MONSTER: u8 = 0;

        // Collect quest IDs that have a matching kill objective to avoid borrow issues
        let matching: Vec<(u32, usize, i32)> = self
            .player_quests
            .values()
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
            let Some(qs) = self.player_quests.get_mut(&quest_id) else {
                continue;
            };
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
                quest_id, obj_idx, current, required, "Quest kill objective progress"
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
                info!(
                    account = self.account_id,
                    quest_id, "Quest objectives complete"
                );
            }
        }
        self.sync_player_registry_state_like_cpp();
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
            let lvl = self.player_level_like_cpp() as usize;
            self.set_player_next_level_xp_like_cpp(table.get(lvl).copied().unwrap_or(u32::MAX));
        }
    }

    /// Calculate XP reward for a quest.
    /// C# ref: Quest::XPValue(player, questLevel, xpDifficulty, xpMultiplier)
    pub(crate) fn calculate_quest_xp(&self, difficulty: u32, quest_level: i32) -> u32 {
        if let Some(store) = &self.quest_xp_store {
            store.calculate_xp(quest_level, self.player_level_like_cpp(), difficulty)
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

    pub(crate) fn broadcast_to_movement_set_like_cpp(&self, bytes: Vec<u8>, include_self: bool) {
        let (Some(guid), Some(registry)) = (self.player_guid(), self.player_registry()) else {
            return;
        };
        let current_map_id = self.player_map_id_like_cpp();

        for entry in registry.iter() {
            let (other_guid, other_info): (&ObjectGuid, &PlayerBroadcastInfo) = entry.pair();
            if !include_self && *other_guid == guid {
                continue;
            }
            if other_info.map_id != current_map_id {
                continue;
            }
            let _ = other_info.send_tx.send(bytes.clone());
        }
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
            self.player_guid(),
            self.player_position_like_cpp(),
            self.player_name_like_cpp(),
            &self.player_registry,
        ) else {
            return;
        };
        let map_id = self.player_map_id_like_cpp();
        let race = self.player_race_like_cpp();
        let class = self.player_class_like_cpp();
        let gender = self.player_gender_like_cpp();
        let level = self.player_level_like_cpp();
        let mut visible_items = [(0i32, 0u16, 0u16); 19];
        for (slot, item) in self.inventory_items_like_cpp() {
            if (*slot as usize) < 19 {
                visible_items[*slot as usize] = (item.entry_id as i32, 0u16, 0u16);
            }
        }
        reg.insert(
            guid,
            PlayerBroadcastInfo {
                map_id,
                position: pos,
                send_tx: self.send_tx.clone(),
                command_tx: self.session_command_tx.clone(),
                active_loot_rolls: self
                    .represented_loot_rolls
                    .keys()
                    .map(|key| (key.0, key.1))
                    .collect(),
                pass_on_group_loot: self.pass_on_group_loot,
                enchanting_skill: self.represented_enchanting_skill,
                known_spells: self.known_spells_like_cpp().to_vec(),
                active_quest_statuses: self
                    .player_quests
                    .iter()
                    .map(|(quest_id, status)| (*quest_id, status.status))
                    .collect(),
                active_quest_objective_counts: self
                    .player_quests
                    .iter()
                    .map(|(quest_id, status)| (*quest_id, status.objective_counts.clone()))
                    .collect(),
                rewarded_quests: self.rewarded_quests.clone(),
                inventory_item_counts: self.represented_inventory_item_counts_like_cpp(),
                party_member_phase_states: party_member_phase_states_like_cpp(
                    self.represented_player_phase_shift_like_cpp(),
                )
                .unwrap_or_default(),
                player_name: name.to_string(),
                account_id: self.account_id,
                race,
                class,
                sex: gender,
                level,
                display_id: default_display_id(race, gender),
                visible_items,
            },
        );
        debug!(
            "Registered player {:?} ({}) in broadcast registry (map {})",
            guid, name, map_id
        );
    }

    pub(crate) fn sync_player_registry_state_like_cpp(&self) {
        let (Some(guid), Some(registry)) = (self.player_guid(), &self.player_registry) else {
            return;
        };
        if let Some(mut info) = registry.get_mut(&guid) {
            info.active_loot_rolls = self
                .represented_loot_rolls
                .keys()
                .map(|key| (key.0, key.1))
                .collect();
            info.pass_on_group_loot = self.pass_on_group_loot;
            info.enchanting_skill = self.represented_enchanting_skill;
            info.known_spells = self.known_spells_like_cpp().to_vec();
            info.active_quest_statuses = self
                .player_quests
                .iter()
                .map(|(quest_id, status)| (*quest_id, status.status))
                .collect();
            info.active_quest_objective_counts = self
                .player_quests
                .iter()
                .map(|(quest_id, status)| (*quest_id, status.objective_counts.clone()))
                .collect();
            info.rewarded_quests = self.rewarded_quests.clone();
            info.inventory_item_counts = self.represented_inventory_item_counts_like_cpp();
            info.party_member_phase_states =
                party_member_phase_states_like_cpp(self.represented_player_phase_shift_like_cpp())
                    .unwrap_or_default();
        }
    }

    fn object_accessor_player_object(&self) -> Option<WorldObject> {
        let (Some(guid), Some(pos), Some(name)) = (
            self.player_guid(),
            self.player_position_like_cpp(),
            self.player_name_like_cpp(),
        ) else {
            return None;
        };

        let mut object = WorldObject::new(true, TypeId::Player, TypeMask::PLAYER);
        object.object_mut().create(guid);
        object.set_name(name);
        if object
            .set_map(u32::from(self.player_map_id_like_cpp()), 0)
            .is_err()
        {
            return None;
        }
        object.relocate(pos);
        *object.phase_shift_mut() = self.represented_player_phase_shift.clone();
        object.object_mut().add_to_world();
        Some(object)
    }

    fn object_accessor_inventory_snapshot(&self) -> PlayerInventoryStorage {
        let mut inventory = PlayerInventoryStorage::default();
        for (&slot, item) in self.inventory_items_like_cpp() {
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
        let Some(name) = self.player_name_like_cpp() else {
            return;
        };

        let inventory = self.object_accessor_inventory_snapshot();
        let items = self.inventory_item_objects_like_cpp().values().cloned();
        if let Err(err) = accessor
            .write()
            .add_player_with_inventory_and_items(name, object, inventory, items)
        {
            warn!("Failed to sync player into ObjectAccessor: {err:?}");
        }
    }

    pub(crate) fn unregister_from_object_accessor(&self) {
        let (Some(guid), Some(accessor)) = (self.player_guid(), &self.object_accessor) else {
            return;
        };
        accessor.write().remove_player(guid);
    }

    pub fn cleanup_shared_runtime_state(&mut self) {
        self.unregister_from_player_registry();
        self.unregister_from_object_accessor();
        self.clear_inventory_items_and_objects_like_cpp();
    }

    pub async fn cleanup_shared_runtime_state_on_disconnect_like_cpp(&mut self) {
        if let Some(player_guid) = self.player_guid()
            && !self.active_loot_guid.is_empty()
        {
            self.close_active_loot_windows_like_cpp(player_guid);
        }
        self.cleanup_shared_runtime_state();
    }

    /// Remove this session from the player registry.
    /// Called on logout or disconnect.
    pub(crate) fn unregister_from_player_registry(&self) {
        let (Some(guid), Some(reg)) = (self.player_guid(), &self.player_registry) else {
            return;
        };
        reg.remove(&guid);
        debug!("Unregistered player {:?} from broadcast registry", guid);
    }

    /// Update this session's position (and map) in the player registry.
    /// Called whenever `player_position` changes.
    pub(crate) fn update_registry_position(&self) {
        let (Some(guid), Some(pos), Some(reg)) = (
            self.player_guid(),
            self.player_position_like_cpp(),
            &self.player_registry,
        ) else {
            return;
        };
        let map_id = self.player_map_id_like_cpp();
        if let Some(mut entry) = reg.get_mut(&guid) {
            entry.position = pos;
            entry.map_id = map_id;
        }
        if let Some(accessor) = &self.object_accessor {
            if let Some(object) = accessor.write().player_object_mut(guid) {
                object.world_relocate(u32::from(map_id), pos);
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
    pub fn set_connect_to_serial(
        &mut self,
        serial: Option<wow_packet::packets::auth::ConnectToSerial>,
    ) {
        self.connect_to_serial = serial;
    }

    /// Set the instance link receiver.
    pub fn set_instance_link_rx(
        &mut self,
        rx: Option<tokio::sync::oneshot::Receiver<InstanceLink>>,
    ) {
        self.instance_link_rx = rx;
    }

    /// Get a clone of the send channel.
    pub fn send_tx(&self) -> &flume::Sender<Vec<u8>> {
        &self.send_tx
    }

    #[cfg(test)]
    pub(crate) fn session_command_tx(&self) -> flume::Sender<SessionCommand> {
        self.session_command_tx.clone()
    }

    pub(crate) fn drain_session_commands(&self) -> Vec<SessionCommand> {
        let mut commands = Vec::new();
        while let Ok(command) = self.session_command_rx.try_recv() {
            commands.push(command);
        }
        commands
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
                    debug!(
                        "Packet channel disconnected for account {}",
                        self.account_id
                    );
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
            aura_interrupt_flags: 0,
            aura_interrupt_flags2: 0,
            represented_effect: None,
            represented_amount: 0,
            represented_multiplier: 1.0,
            applied_at: Instant::now(),
        };

        self.visible_auras.insert(slot, aura);

        // Send SMSG_AURA_UPDATE
        self.send_aura_update_applied(spell_id, slot, caster_guid, duration_ms, aura_flags);

        Ok(())
    }

    fn apply_represented_mounted_aura_like_cpp(
        &mut self,
        spell_id: i32,
        caster_guid: ObjectGuid,
        effect: &wow_data::SpellEffectInfo,
    ) -> Result<(), &'static str> {
        let selected_display_id = u32::try_from(spell_id)
            .ok()
            .and_then(|spell_id| self.select_represented_mount_aura_display_like_cpp(spell_id))
            .unwrap_or(0);
        let creature_entry = u32::try_from(effect.effect_misc_value_1).unwrap_or(0);
        let creature_template_mount =
            self.represented_mount_creature_template_fallback_like_cpp(creature_entry);
        let display_id = if selected_display_id != 0 {
            selected_display_id
        } else {
            creature_template_mount
                .map(|(display_id, _)| display_id)
                .unwrap_or(0)
        };
        let vehicle_id = creature_template_mount
            .map(|(_, vehicle_id)| vehicle_id)
            .unwrap_or(0);

        let mut slot = 0u8;
        while self.visible_auras.contains_key(&slot) && slot < 255 {
            slot += 1;
        }

        if slot >= 255 {
            return Err("No free aura slots");
        }

        let aura = AuraApplication {
            spell_id,
            caster_guid,
            slot,
            duration_total: 0,
            duration_remaining: 0,
            stack_count: 1,
            aura_flags: 0x0000_0001,
            aura_interrupt_flags: 0,
            aura_interrupt_flags2: 0,
            represented_effect: Some(RepresentedAuraEffectLikeCpp::Mounted),
            represented_amount: effect.effect_base_points,
            represented_multiplier: 1.0,
            applied_at: Instant::now(),
        };

        self.visible_auras.insert(slot, aura);
        self.player_mount_display_id_like_cpp = display_id;
        self.player_mounted_like_cpp = true;
        self.player_unit_flags_like_cpp.insert(UnitFlags::MOUNT);
        if self.create_player_mount_vehicle_kit_like_cpp(vehicle_id, creature_entry) {
            self.mount_vehicle_create_requests_like_cpp = self
                .mount_vehicle_create_requests_like_cpp
                .saturating_add(1);
            self.send_set_vehicle_rec_id_like_cpp(vehicle_id);
            self.send_on_cancel_expected_vehicle_ride_aura_like_cpp();
        }
        self.mount_pet_control_disable_requests_like_cpp = self
            .mount_pet_control_disable_requests_like_cpp
            .saturating_add(1);
        self.disable_pet_controls_on_mount_like_cpp(
            wow_packet::packets::pet::REACT_PASSIVE_LIKE_CPP,
            wow_packet::packets::pet::COMMAND_FOLLOW_LIKE_CPP,
        );
        self.mount_collision_height_update_requests_like_cpp = self
            .mount_collision_height_update_requests_like_cpp
            .saturating_add(1);
        self.update_player_collision_height_like_cpp();
        self.send_movement_set_collision_height_like_cpp(
            wow_packet::packets::movement::UPDATE_COLLISION_HEIGHT_REASON_MOUNT_LIKE_CPP,
        );

        self.send_aura_update_applied(spell_id, slot, caster_guid, 0, 0x0000_0001);
        self.send_represented_mount_unit_update_like_cpp(display_id);

        Ok(())
    }

    fn create_player_mount_vehicle_kit_like_cpp(
        &mut self,
        vehicle_id: u32,
        creature_entry: u32,
    ) -> bool {
        if vehicle_id == 0 {
            return false;
        }
        let Some(player_guid) = self.player_guid() else {
            return false;
        };

        let Some(vehicle) = self
            .vehicle_store
            .as_ref()
            .and_then(|store| store.get(vehicle_id))
        else {
            if self.vehicle_store.is_some() {
                return false;
            }
            self.player_mount_vehicle_id_like_cpp = vehicle_id;
            self.player_mount_vehicle_kit_like_cpp = None;
            self.player_mount_vehicle_accessories_like_cpp = self
                .vehicle_accessory_store
                .as_ref()
                .and_then(|store| store.accessories_for_vehicle_like_cpp(None, creature_entry))
                .map(<[VehicleAccessory]>::to_vec)
                .unwrap_or_default();
            self.player_mount_vehicle_seat_count_like_cpp = 0;
            self.player_mount_vehicle_usable_seat_count_like_cpp = 0;
            return true;
        };

        let seat_defs = self
            .vehicle_seat_store
            .as_ref()
            .map(|store| store.seat_defs_for_vehicle_like_cpp(vehicle))
            .unwrap_or_default();
        let mut vehicle_kit = Vehicle::new(
            player_guid,
            TypeId::Player,
            self.player_position_like_cpp()
                .unwrap_or(wow_core::Position::ZERO),
            vehicle_id,
            creature_entry,
            seat_defs,
        );
        vehicle_kit.install();
        let accessories = self
            .vehicle_accessory_store
            .as_ref()
            .and_then(|store| store.accessories_for_vehicle_like_cpp(None, creature_entry))
            .map(<[VehicleAccessory]>::to_vec)
            .unwrap_or_default();
        let accessory_plan = vehicle_kit.install_all_accessories_plan_like_cpp(false, &accessories);
        self.player_mount_vehicle_id_like_cpp = vehicle_id;
        self.player_mount_vehicle_seat_count_like_cpp =
            vehicle_kit.seats().len().min(u8::MAX as usize) as u8;
        self.player_mount_vehicle_usable_seat_count_like_cpp =
            vehicle_kit.usable_seat_num().min(u32::from(u8::MAX)) as u8;
        self.player_mount_vehicle_accessories_like_cpp = accessory_plan.accessories;
        self.player_mount_vehicle_kit_like_cpp = Some(vehicle_kit);
        true
    }

    #[allow(dead_code)]
    fn player_mount_vehicle_despawn_delay_ms_like_cpp(&self) -> i32 {
        let Some(vehicle_kit) = self.player_mount_vehicle_kit_like_cpp.as_ref() else {
            return 1;
        };

        self.vehicle_template_store
            .as_ref()
            .map(|store| store.despawn_delay_ms_like_cpp(vehicle_kit.creature_entry()))
            .unwrap_or(1)
    }

    fn send_set_vehicle_rec_id_like_cpp(&mut self, vehicle_id: u32) {
        let Some(player_guid) = self.player_guid() else {
            return;
        };
        let vehicle_rec_id = i32::try_from(vehicle_id).unwrap_or(i32::MAX);
        let sequence_index = self.mount_vehicle_movement_sequence_like_cpp;
        self.mount_vehicle_movement_sequence_like_cpp = self
            .mount_vehicle_movement_sequence_like_cpp
            .wrapping_add(1);

        self.send_packet(&wow_packet::packets::vehicle::MoveSetVehicleRecId {
            mover_guid: player_guid,
            sequence_index,
            vehicle_rec_id,
        });
        self.send_packet(&wow_packet::packets::vehicle::SetVehicleRecId {
            vehicle_guid: player_guid,
            vehicle_rec_id,
        });
    }

    fn send_on_cancel_expected_vehicle_ride_aura_like_cpp(&mut self) {
        self.mount_cancel_expected_vehicle_aura_packets_like_cpp = self
            .mount_cancel_expected_vehicle_aura_packets_like_cpp
            .saturating_add(1);
        self.send_packet(&wow_packet::packets::vehicle::OnCancelExpectedRideVehicleAura);
    }

    fn disable_pet_controls_on_mount_like_cpp(&mut self, react_state: u8, command_state: u8) {
        let Some(pet_guid) = self.represented_pet_guid_like_cpp else {
            return;
        };

        self.temporary_mount_pet_react_state_like_cpp =
            Some(self.represented_pet_react_state_like_cpp);
        self.represented_pet_react_state_like_cpp = react_state;
        self.represented_pet_command_state_like_cpp = command_state;
        self.send_packet(&wow_packet::packets::pet::PetMode {
            pet_guid,
            react_state,
            command_state,
            flag: 0,
        });
    }

    fn enable_pet_controls_on_dismount_like_cpp(&mut self) {
        if let Some(pet_guid) = self.represented_pet_guid_like_cpp {
            if let Some(react_state) = self.temporary_mount_pet_react_state_like_cpp {
                self.represented_pet_react_state_like_cpp = react_state;
            }
            self.send_packet(&wow_packet::packets::pet::PetMode {
                pet_guid,
                react_state: self.represented_pet_react_state_like_cpp,
                command_state: self.represented_pet_command_state_like_cpp,
                flag: 0,
            });
        }

        self.temporary_mount_pet_react_state_like_cpp = None;
    }

    fn update_player_collision_height_like_cpp(&mut self) {
        let (Some(display_store), Some(model_store)) = (
            self.creature_display_info_store.as_ref(),
            self.creature_model_data_store.as_ref(),
        ) else {
            return;
        };

        let native_display_id = crate::handlers::character::default_display_id(
            self.player_race_like_cpp(),
            self.player_gender_like_cpp(),
        );
        let mount_display_id = u32::try_from(self.player_mount_display_id_like_cpp)
            .ok()
            .filter(|id| *id != 0);
        if let Some(height) = wow_data::unit_collision_height_like_cpp(
            self.player_object_scale_like_cpp,
            native_display_id,
            mount_display_id,
            display_store,
            model_store,
        ) {
            self.player_collision_height_like_cpp = height;
        }
    }

    fn send_movement_set_collision_height_like_cpp(&mut self, reason: u8) {
        let Some(player_guid) = self.player_guid() else {
            return;
        };
        let sequence_index = self.mount_vehicle_movement_sequence_like_cpp;
        self.mount_vehicle_movement_sequence_like_cpp = self
            .mount_vehicle_movement_sequence_like_cpp
            .wrapping_add(1);

        self.send_packet(&wow_packet::packets::movement::MoveSetCollisionHeight {
            mover_guid: player_guid,
            sequence_index,
            height: self.player_collision_height_like_cpp,
            scale: self.player_object_scale_like_cpp,
            reason,
            mount_display_id: u32::try_from(self.player_mount_display_id_like_cpp).unwrap_or(0),
            scale_duration: self.player_scale_duration_like_cpp,
        });

        use wow_packet::ServerPacket;
        let mut status = self.current_player_movement_info_like_cpp(player_guid);
        status.time = self.player_movement_time_like_cpp();
        self.broadcast_to_movement_set_like_cpp(
            wow_packet::packets::movement::MoveUpdateCollisionHeight {
                status,
                height: self.player_collision_height_like_cpp,
                scale: self.player_object_scale_like_cpp,
            }
            .to_bytes(),
            false,
        );
    }

    fn current_player_movement_info_like_cpp(
        &self,
        player_guid: ObjectGuid,
    ) -> wow_packet::packets::movement::MovementInfo {
        wow_packet::packets::movement::MovementInfo {
            guid: player_guid,
            position: self
                .player_position_like_cpp()
                .unwrap_or(wow_core::Position::ZERO),
            time: self.player_movement_time_like_cpp(),
            ..wow_packet::packets::movement::MovementInfo::default()
        }
    }

    fn send_represented_mount_unit_update_like_cpp(&mut self, display_id: i32) {
        let Some(player_guid) = self.player_guid() else {
            return;
        };

        use wow_packet::packets::update::{UnitDataValuesDeltaUpdate, UpdateObject};
        let mut data = UnitDataValuesDeltaUpdate::default();
        data.unit_data_mask[1] |= 1 << (41 - 32);
        data.unit_data_mask[1] |= 1 << (51 - 32);
        data.flags = self.player_unit_flags_like_cpp.bits();
        data.mount_display_id = display_id;

        self.send_packet(&UpdateObject::unit_values_update(
            player_guid,
            self.player_map_id_like_cpp(),
            data,
        ));
    }

    /// Remove an aura by slot and send SMSG_AURA_UPDATE.
    pub fn remove_aura(&mut self, slot: u8) -> Result<(), &'static str> {
        let Some(aura) = self.visible_auras.remove(&slot) else {
            return Err("Aura slot not found");
        };

        if aura.represented_effect == Some(RepresentedAuraEffectLikeCpp::Mounted) {
            let was_mounted = self.player_mounted_like_cpp;
            let vehicle_id = self.player_mount_vehicle_id_like_cpp;
            self.player_mount_display_id_like_cpp = 0;
            self.player_mount_vehicle_id_like_cpp = 0;
            if let Some(vehicle_kit) = self.player_mount_vehicle_kit_like_cpp.as_mut() {
                vehicle_kit.uninstall();
            }
            self.player_mount_vehicle_kit_like_cpp = None;
            self.player_mount_vehicle_accessories_like_cpp.clear();
            self.player_mount_vehicle_seat_count_like_cpp = 0;
            self.player_mount_vehicle_usable_seat_count_like_cpp = 0;
            self.player_mounted_like_cpp = false;
            self.player_unit_flags_like_cpp.remove(UnitFlags::MOUNT);
            if was_mounted {
                if vehicle_id != 0 {
                    self.mount_vehicle_remove_requests_like_cpp = self
                        .mount_vehicle_remove_requests_like_cpp
                        .saturating_add(1);
                    self.send_set_vehicle_rec_id_like_cpp(0);
                }
                self.mount_pet_control_enable_requests_like_cpp = self
                    .mount_pet_control_enable_requests_like_cpp
                    .saturating_add(1);
                self.enable_pet_controls_on_dismount_like_cpp();
                self.mount_pet_resummon_requests_like_cpp =
                    self.mount_pet_resummon_requests_like_cpp.saturating_add(1);
                self.mount_collision_height_update_requests_like_cpp = self
                    .mount_collision_height_update_requests_like_cpp
                    .saturating_add(1);
                self.update_player_collision_height_like_cpp();
                self.send_movement_set_collision_height_like_cpp(
                    wow_packet::packets::movement::UPDATE_COLLISION_HEIGHT_REASON_MOUNT_LIKE_CPP,
                );
            }
            self.send_represented_mount_unit_update_like_cpp(0);
        }

        // Send SMSG_AURA_UPDATE (removal)
        self.send_aura_update_removed(slot);

        Ok(())
    }

    pub(crate) fn remove_auras_with_looting_interrupt_flags_like_cpp(&mut self) -> usize {
        self.remove_auras_with_interrupt_flags_like_cpp(
            SPELL_AURA_INTERRUPT_FLAG_LOOTING_LIKE_CPP,
            0,
        )
    }

    pub(crate) fn remove_auras_with_interrupt_flags_like_cpp(
        &mut self,
        flags: u32,
        flags2: u32,
    ) -> usize {
        let slots: Vec<u8> = self
            .visible_auras
            .values()
            .filter(|aura| {
                (flags != 0 && aura.aura_interrupt_flags & flags != 0)
                    || (flags2 != 0 && aura.aura_interrupt_flags2 & flags2 != 0)
            })
            .map(|aura| aura.slot)
            .collect();

        let removed = slots.len();
        for slot in slots {
            let _ = self.remove_aura(slot);
        }
        removed
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
            let spell_id = self
                .visible_auras
                .get(&slot)
                .map(|a| a.spell_id)
                .unwrap_or(0);
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
        use wow_packet::ServerPacket;
        use wow_packet::packets::aura::{AuraData, AuraUpdate};

        let update = AuraUpdate {
            target_guid: self.player_guid().unwrap_or(ObjectGuid::EMPTY),
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
            target_guid: self.player_guid().unwrap_or(ObjectGuid::EMPTY),
            updated_auras: vec![],
            removed_aura_slots: vec![slot],
        };
        self.send_packet(&update);
    }

    /// Send a TimeSyncRequest and schedule the next one.
    pub(crate) fn send_time_sync(&mut self) {
        use wow_packet::packets::misc::TimeSyncRequest;
        let sequence_index = self.time_sync_next_counter;
        self.send_packet(&TimeSyncRequest { sequence_index });
        trace!(
            "Sent TimeSyncRequest(seq={}) for account {}",
            sequence_index, self.account_id
        );
        self.time_sync_pending_requests
            .insert(sequence_index, Self::game_time_ms_like_cpp());
        // C++ uses 5s for the first request, then 10s.
        self.time_sync_timer_ms = if self.time_sync_next_counter == 0 {
            5000
        } else {
            10000
        };
        self.time_sync_next_counter += 1;
    }

    /// Monotonic millisecond counter matching TrinityCore's `getMSTime()` scale.
    pub(crate) fn game_time_ms_like_cpp() -> u32 {
        static SERVER_START: OnceLock<Instant> = OnceLock::new();
        let start = SERVER_START.get_or_init(Instant::now);
        start.elapsed().as_millis() as u32
    }

    pub(crate) fn reset_time_sync_like_cpp(&mut self) {
        self.time_sync_next_counter = 0;
        self.time_sync_pending_requests.clear();
    }

    pub(crate) fn record_time_sync_response_like_cpp(
        &mut self,
        sequence_index: u32,
        client_time: u32,
    ) {
        let Some(server_time_at_sent) = self.time_sync_pending_requests.remove(&sequence_index)
        else {
            return;
        };

        let received_time = Self::game_time_ms_like_cpp();
        let round_trip_duration = received_time.wrapping_sub(server_time_at_sent);
        let lag_delay = round_trip_duration / 2;
        let clock_delta =
            i64::from(server_time_at_sent) + i64::from(lag_delay) - i64::from(client_time);

        if self.time_sync_clock_delta_queue.len() == 6 {
            self.time_sync_clock_delta_queue.pop_front();
        }
        self.time_sync_clock_delta_queue
            .push_back((clock_delta, round_trip_duration));
        self.compute_new_clock_delta_like_cpp();
    }

    fn compute_new_clock_delta_like_cpp(&mut self) {
        if self.time_sync_clock_delta_queue.is_empty() {
            return;
        }

        let mut latencies: Vec<u32> = self
            .time_sync_clock_delta_queue
            .iter()
            .map(|(_, round_trip_duration)| *round_trip_duration)
            .collect();
        latencies.sort_unstable();
        let latency_median = rounded_median_u32(&latencies);
        let latency_mean =
            latencies.iter().map(|v| f64::from(*v)).sum::<f64>() / latencies.len() as f64;
        let latency_variance = latencies
            .iter()
            .map(|v| {
                let diff = f64::from(*v) - latency_mean;
                diff * diff
            })
            .sum::<f64>()
            / latencies.len() as f64;
        let latency_standard_deviation = latency_variance.sqrt().round() as u32;

        let latency_threshold = latency_standard_deviation.saturating_add(latency_median);
        let mut clock_delta_sum = 0i64;
        let mut sample_size_after_filtering = 0u32;
        for (clock_delta, round_trip_duration) in &self.time_sync_clock_delta_queue {
            if *round_trip_duration < latency_threshold {
                clock_delta_sum += *clock_delta;
                sample_size_after_filtering += 1;
            }
        }

        if sample_size_after_filtering != 0 {
            let mean_clock_delta =
                (clock_delta_sum as f64 / f64::from(sample_size_after_filtering)).round() as i64;
            if (mean_clock_delta - self.time_sync_clock_delta).abs() > 25 {
                self.time_sync_clock_delta = mean_clock_delta;
            }
        } else if self.time_sync_clock_delta == 0 {
            self.time_sync_clock_delta = self
                .time_sync_clock_delta_queue
                .back()
                .map(|(clock_delta, _)| *clock_delta)
                .unwrap_or_default();
        }
    }

    pub(crate) fn adjust_client_movement_time_like_cpp(&self, time: u32) -> u32 {
        let movement_time = i64::from(time) + self.time_sync_clock_delta;
        if self.time_sync_clock_delta == 0 || !(0..=i64::from(u32::MAX)).contains(&movement_time) {
            warn!(
                "The computed movement time using clockDelta is erroneous. Using fallback instead"
            );
            Self::game_time_ms_like_cpp()
        } else {
            movement_time as u32
        }
    }

    /// Process pending packets asynchronously. Call after `update()`.
    pub async fn process_pending(&mut self) {
        self.process_represented_session_commands_like_cpp().await;

        // ── Spell casting tick ─────────────────────────────────────────
        // Check if an active spell cast has completed and execute it.
        if self.state == SessionState::LoggedIn {
            self.tick_represented_loot_rolls_like_cpp().await;
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
        let opcode: ClientOpcodes = match num_traits::FromPrimitive::from_u32(u32::from(opcode_raw))
        {
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
                    opcode, self.account_id
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
            ClientOpcodes::Ping => match wow_packet::packets::auth::Ping::read(&mut pkt) {
                Ok(ping) => self.handle_ping(ping).await,
                Err(e) => warn!("Failed to read Ping: {e}"),
            },
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
            ClientOpcodes::BuyItem => match wow_packet::packets::misc::BuyItem::read(&mut pkt) {
                Ok(buy) => self.handle_buy_item(buy).await,
                Err(e) => warn!("Failed to read BuyItem: {e}"),
            },
            ClientOpcodes::BuyBackItem => {
                match wow_packet::packets::misc::BuyBackItem::read(&mut pkt) {
                    Ok(buyback) => self.handle_buy_back_item(buyback).await,
                    Err(e) => warn!("Failed to read BuyBackItem: {e}"),
                }
            }
            ClientOpcodes::SellItem => match wow_packet::packets::misc::SellItem::read(&mut pkt) {
                Ok(sell) => self.handle_sell_item(sell).await,
                Err(e) => warn!("Failed to read SellItem: {e}"),
            },
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
            ClientOpcodes::SwapItem => match wow_packet::packets::item::SwapItem::read(&mut pkt) {
                Ok(swap) => self.handle_swap_item(swap).await,
                Err(e) => warn!("Failed to read SwapItem: {e}"),
            },
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
            ClientOpcodes::MoveSetVehicleRecIdAck => {
                let opcode = pkt.client_opcode().unwrap_or(opcode);
                match wow_packet::packets::vehicle::MoveSetVehicleRecIdAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_set_vehicle_rec_id_ack(opcode, ack).await,
                    Err(e) => warn!("Failed to read MoveSetVehicleRecIdAck: {e}"),
                }
            }
            ClientOpcodes::MoveDismissVehicle => {
                match wow_packet::packets::vehicle::MoveDismissVehicle::read(&mut pkt) {
                    Ok(packet) => self.handle_move_dismiss_vehicle(packet).await,
                    Err(e) => warn!("Failed to read MoveDismissVehicle: {e}"),
                }
            }
            ClientOpcodes::RequestVehiclePrevSeat => {
                match wow_packet::packets::vehicle::RequestVehiclePrevSeat::read(&mut pkt) {
                    Ok(packet) => self.handle_request_vehicle_prev_seat(packet).await,
                    Err(e) => warn!("Failed to read RequestVehiclePrevSeat: {e}"),
                }
            }
            ClientOpcodes::RequestVehicleNextSeat => {
                match wow_packet::packets::vehicle::RequestVehicleNextSeat::read(&mut pkt) {
                    Ok(packet) => self.handle_request_vehicle_next_seat(packet).await,
                    Err(e) => warn!("Failed to read RequestVehicleNextSeat: {e}"),
                }
            }
            ClientOpcodes::MoveChangeVehicleSeats => {
                match wow_packet::packets::vehicle::MoveChangeVehicleSeats::read(&mut pkt) {
                    Ok(packet) => self.handle_move_change_vehicle_seats(packet).await,
                    Err(e) => warn!("Failed to read MoveChangeVehicleSeats: {e}"),
                }
            }
            ClientOpcodes::RequestVehicleSwitchSeat => {
                match wow_packet::packets::vehicle::RequestVehicleSwitchSeat::read(&mut pkt) {
                    Ok(packet) => self.handle_request_vehicle_switch_seat(packet).await,
                    Err(e) => warn!("Failed to read RequestVehicleSwitchSeat: {e}"),
                }
            }
            ClientOpcodes::RideVehicleInteract => {
                match wow_packet::packets::vehicle::RideVehicleInteract::read(&mut pkt) {
                    Ok(packet) => self.handle_ride_vehicle_interact(packet).await,
                    Err(e) => warn!("Failed to read RideVehicleInteract: {e}"),
                }
            }
            ClientOpcodes::EjectPassenger => {
                match wow_packet::packets::vehicle::EjectPassenger::read(&mut pkt) {
                    Ok(packet) => self.handle_eject_passenger(packet).await,
                    Err(e) => warn!("Failed to read EjectPassenger: {e}"),
                }
            }
            ClientOpcodes::RequestVehicleExit => {
                match wow_packet::packets::vehicle::RequestVehicleExit::read(&mut pkt) {
                    Ok(packet) => self.handle_request_vehicle_exit(packet).await,
                    Err(e) => warn!("Failed to read RequestVehicleExit: {e}"),
                }
            }
            ClientOpcodes::MoveCollisionDisableAck
            | ClientOpcodes::MoveCollisionEnableAck
            | ClientOpcodes::MoveEnableDoubleJumpAck
            | ClientOpcodes::MoveEnableSwimToFlyTransAck
            | ClientOpcodes::MoveFeatherFallAck
            | ClientOpcodes::MoveForceRootAck
            | ClientOpcodes::MoveForceUnrootAck
            | ClientOpcodes::MoveGravityDisableAck
            | ClientOpcodes::MoveGravityEnableAck
            | ClientOpcodes::MoveHoverAck
            | ClientOpcodes::MoveInertiaDisableAck
            | ClientOpcodes::MoveInertiaEnableAck
            | ClientOpcodes::MoveSetCanFlyAck
            | ClientOpcodes::MoveSetCanTurnWhileFallingAck
            | ClientOpcodes::MoveSetIgnoreMovementForcesAck
            | ClientOpcodes::MoveWaterWalkAck => {
                let opcode = pkt.client_opcode().unwrap_or(opcode);
                match wow_packet::packets::movement::MovementAckMessage::read(&mut pkt) {
                    Ok(ack) => self.handle_movement_ack_message(opcode, ack).await,
                    Err(e) => warn!("Failed to read MovementAckMessage: {e}"),
                }
            }
            ClientOpcodes::MoveForceWalkSpeedChangeAck
            | ClientOpcodes::MoveForceRunSpeedChangeAck
            | ClientOpcodes::MoveForceRunBackSpeedChangeAck
            | ClientOpcodes::MoveForceSwimSpeedChangeAck
            | ClientOpcodes::MoveForceSwimBackSpeedChangeAck
            | ClientOpcodes::MoveForceTurnRateChangeAck
            | ClientOpcodes::MoveForceFlightSpeedChangeAck
            | ClientOpcodes::MoveForceFlightBackSpeedChangeAck
            | ClientOpcodes::MoveForcePitchRateChangeAck
            | ClientOpcodes::MoveSetModMovementForceMagnitudeAck => {
                let opcode = pkt.client_opcode().unwrap_or(opcode);
                match wow_packet::packets::movement::MovementSpeedAck::read(&mut pkt) {
                    Ok(ack) => self.handle_movement_speed_ack(opcode, ack).await,
                    Err(e) => warn!("Failed to read MovementSpeedAck: {e}"),
                }
            }
            ClientOpcodes::MoveKnockBackAck => {
                match wow_packet::packets::movement::MoveKnockBackAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_knock_back_ack(ack).await,
                    Err(e) => warn!("Failed to read MoveKnockBackAck: {e}"),
                }
            }
            ClientOpcodes::MoveSetCollisionHeightAck => {
                match wow_packet::packets::movement::MoveSetCollisionHeightAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_set_collision_height_ack(ack).await,
                    Err(e) => warn!("Failed to read MoveSetCollisionHeightAck: {e}"),
                }
            }
            ClientOpcodes::MoveApplyMovementForceAck => {
                match wow_packet::packets::movement::MoveApplyMovementForceAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_apply_movement_force_ack(ack).await,
                    Err(e) => warn!("Failed to read MoveApplyMovementForceAck: {e}"),
                }
            }
            ClientOpcodes::MoveRemoveMovementForceAck => {
                match wow_packet::packets::movement::MoveRemoveMovementForceAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_remove_movement_force_ack(ack).await,
                    Err(e) => warn!("Failed to read MoveRemoveMovementForceAck: {e}"),
                }
            }
            ClientOpcodes::MoveTimeSkipped => {
                match wow_packet::packets::movement::MoveTimeSkipped::read(&mut pkt) {
                    Ok(skipped) => self.handle_move_time_skipped(skipped).await,
                    Err(e) => warn!("Failed to read MoveTimeSkipped: {e}"),
                }
            }
            ClientOpcodes::MoveSplineDone => {
                match wow_packet::packets::movement::MoveSplineDone::read(&mut pkt) {
                    Ok(done) => self.handle_move_spline_done(done).await,
                    Err(e) => warn!("Failed to read MoveSplineDone: {e}"),
                }
            }
            ClientOpcodes::MoveTeleportAck => {
                match wow_packet::packets::movement::MoveTeleportAck::read(&mut pkt) {
                    Ok(ack) => self.handle_move_teleport_ack(ack).await,
                    Err(e) => warn!("Failed to read MoveTeleportAck: {e}"),
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
            ClientOpcodes::LootRoll => match wow_packet::packets::loot::LootRoll::read(&mut pkt) {
                Ok(roll) => self.handle_loot_roll(roll).await,
                Err(e) => warn!("Failed to read LootRoll: {e}"),
            },
            ClientOpcodes::MasterLootItem => {
                match wow_packet::packets::loot::MasterLootItem::read(&mut pkt) {
                    Ok(master_loot_item) => self.handle_master_loot_item(master_loot_item).await,
                    Err(e) => warn!("Failed to read MasterLootItem: {e}"),
                }
            }
            ClientOpcodes::SetLootSpecialization => {
                match wow_packet::packets::loot::SetLootSpecialization::read(&mut pkt) {
                    Ok(set_loot_specialization) => {
                        self.handle_set_loot_specialization(set_loot_specialization)
                            .await;
                    }
                    Err(e) => warn!("Failed to read SetLootSpecialization: {e}"),
                }
            }

            // ── Chat opcodes ────────────────────────────────────────
            ClientOpcodes::ChatMessageSay => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Say)
                    .await;
            }
            ClientOpcodes::ChatMessageYell => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Yell)
                    .await;
            }
            ClientOpcodes::ChatMessageParty => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Party)
                    .await;
            }
            ClientOpcodes::ChatMessageGuild => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Guild)
                    .await;
            }
            ClientOpcodes::ChatMessageRaid => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::Raid)
                    .await;
            }
            ClientOpcodes::ChatMessageRaidWarning => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::RaidWarning)
                    .await;
            }
            ClientOpcodes::ChatMessageInstanceChat => {
                self.handle_chat_message(pkt, wow_packet::packets::chat::ChatMsg::InstanceChat)
                    .await;
            }
            ClientOpcodes::ChatMessageWhisper => {
                self.handle_chat_whisper(pkt).await;
            }
            ClientOpcodes::ChatMessageEmote => {
                self.handle_chat_emote(pkt).await;
            }
            ClientOpcodes::ChatRegisterAddonPrefixes => {
                self.handle_chat_register_addon_prefixes(pkt).await;
            }
            ClientOpcodes::ChatAddonMessage => {
                self.handle_chat_addon_message(pkt).await;
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
                self.handle_guild_bank_remaining_withdraw_money_query(pkt)
                    .await;
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
            ClientOpcodes::ResetInstances => {
                self.handle_reset_instances(pkt).await;
            }
            ClientOpcodes::InstanceLockResponse => {
                self.handle_instance_lock_response(pkt).await;
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
            ClientOpcodes::CancelTrade => {
                self.handle_cancel_trade(pkt).await;
            }
            ClientOpcodes::ReportClientVariables => {
                self.handle_report_client_variables(pkt).await;
            }
            ClientOpcodes::ReportEnabledAddons => {
                self.handle_report_enabled_addons(pkt).await;
            }
            ClientOpcodes::ReportKeybindingExecutionCounts => {
                self.handle_report_keybinding_execution_counts(pkt).await;
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
            ClientOpcodes::SetLootMethod => {
                self.handle_set_loot_method(pkt).await;
            }
            ClientOpcodes::OptOutOfLoot => {
                self.handle_opt_out_of_loot(pkt).await;
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
                    opcode, opcode_raw
                );
            }
            _ => match entry.processing {
                PacketProcessing::Inplace => {
                    trace!("Processing {:?} inplace via {}", opcode, entry.handler_name);
                }
                PacketProcessing::ThreadUnsafe => {
                    trace!(
                        "Queuing {:?} for thread-unsafe processing via {}",
                        opcode, entry.handler_name
                    );
                }
                PacketProcessing::ThreadSafe => {
                    trace!(
                        "Processing {:?} via thread-safe handler {}",
                        opcode, entry.handler_name
                    );
                }
            },
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
                self.state == SessionState::LoggedIn || self.state == SessionState::Disconnecting
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
        let (Some(pos), Some(store)) = (
            self.player_position_like_cpp(),
            self.area_trigger_store.as_ref(),
        ) else {
            return;
        };

        // Get all triggers at the current position on the player's current map
        let triggers = store.get_triggers_at_position(self.player_map_id_like_cpp(), &pos);

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

        if self.is_map_disabled_for_player_like_cpp(new_map) {
            warn!(
                account = self.account_id,
                map_id = new_map,
                "Teleport blocked by C++ DisableMgr map gate"
            );
            self.send_packet(&wow_packet::packets::misc::TransferAborted {
                map_id: new_map,
                arg: 0,
                map_difficulty_x_condition_id: 0,
                transfer_abort: TRANSFER_ABORT_MAP_NOT_ALLOWED_LIKE_CPP,
            });
            return;
        }

        let Some(current_pos) = self.player_position_like_cpp() else {
            warn!(
                "Cannot teleport account {}: no current position",
                self.account_id
            );
            return;
        };

        info!(
            account = self.account_id,
            old_map = self.player_map_id_like_cpp(),
            new_map = new_map,
            old_pos = format!(
                "({:.2}, {:.2}, {:.2})",
                current_pos.x, current_pos.y, current_pos.z
            ),
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
        self.send_packet(&SuspendToken {
            sequence_index: 1,
            reason: 1,
        });

        // 4. Transition to Transfer state — only WorldPortResponse accepted now
        self.state = SessionState::Transfer;

        info!(
            account = self.account_id,
            "Teleport initiated: map {} → {} dest ({:.2}, {:.2}, {:.2}); awaiting WorldPortResponse",
            self.player_map_id_like_cpp(),
            new_map,
            new_pos.x,
            new_pos.y,
            new_pos.z
        );
    }

    fn is_map_disabled_for_player_like_cpp(&self, map_id: u32) -> bool {
        let Some(disable_mgr) = self.disable_mgr() else {
            return false;
        };
        let Some(map_store) = self.map_store() else {
            return false;
        };

        let current_map_id = u32::from(self.player_map_id_like_cpp());
        let (_, area_id) = self.player_zone_area_like_cpp();
        let current_map_instance_type = map_store
            .get(current_map_id)
            .map(|entry| entry.instance_type);

        disable_mgr.is_disabled_for_like_cpp(
            DISABLE_TYPE_MAP,
            map_id,
            Some(DisableWorldObjectRefLikeCpp {
                type_id: TypeId::Player,
                map_id: current_map_id,
                area_id,
                is_pet: false,
                is_battle_arena: current_map_instance_type == Some(MAP_ARENA_LIKE_CPP),
                is_battleground: current_map_instance_type == Some(MAP_BATTLEGROUND_LIKE_CPP),
                player_map_difficulty: None,
            }),
            0,
            Some(map_store.as_ref()),
        )
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

    pub fn send_buy_error(&self, result: BuyResult, creature_guid: Option<ObjectGuid>, item: u32) {
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
    /// 5. AvailableHotfixes
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
        self.send_packet(&ClientCacheVersion {
            cache_version: 24081,
        });

        let hotfixes = self
            .hotfix_blob_cache
            .as_ref()
            .map(|cache| {
                cache
                    .available_hotfix_ids(&self.locale)
                    .into_iter()
                    .map(|id| HotfixId {
                        push_id: id.push_id,
                        unique_id: id.unique_id,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 5. AvailableHotfixes
        self.send_packet(&AvailableHotfixes {
            virtual_realm_address: vra,
            hotfixes,
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
        if guid.is_none() {
            self.player_controller = None;
        }
    }

    pub(crate) fn set_loaded_player_name_like_cpp(&mut self, name: String) {
        self.player_name = Some(name);
    }

    pub(crate) fn set_loaded_player_identity_like_cpp(
        &mut self,
        map_id: u16,
        race: u8,
        class: u8,
        level: u8,
        gender: u8,
    ) {
        self.current_map_id = map_id;
        self.player_race = race;
        self.player_class = class;
        self.player_level = level;
        self.player_gender = gender;
        if let Some(controller) = &mut self.player_controller {
            controller.map_id = map_id;
            controller.race = race;
            controller.class = class;
            controller.set_level(level);
            controller.gender = gender;
        }
    }

    pub(crate) fn attach_player_controller_like_cpp(
        &mut self,
        mut controller: SessionPlayerController,
    ) {
        controller.set_gold(self.player_gold);
        controller.set_xp(self.player_xp);
        controller.set_next_level_xp(self.player_next_level_xp);
        controller.set_selection_guid(self.selection_guid);
        controller.set_known_spells(self.known_spells.clone());
        controller.set_currencies(self.player_currencies.clone());
        controller.set_inventory(self.session_player_inventory_runtime_like_cpp());
        self.player_guid = Some(controller.guid());
        self.player_name = Some(controller.name().to_string());
        self.player_position = Some(controller.position());
        self.current_map_id = controller.map_id();
        self.player_race = controller.race();
        self.player_class = controller.class();
        self.player_level = controller.level();
        self.player_gender = controller.gender();
        self.player_controller = Some(controller);
    }

    pub(crate) fn set_player_map_position_like_cpp(
        &mut self,
        map_id: u16,
        position: wow_core::Position,
    ) {
        self.current_map_id = map_id;
        self.player_position = Some(position);
        if let Some(controller) = &mut self.player_controller {
            controller.set_map_position(map_id, position);
        }
    }

    pub(crate) fn set_player_position_like_cpp(&mut self, position: wow_core::Position) {
        self.set_player_map_position_like_cpp(self.current_map_id, position);
    }

    pub(crate) fn set_player_movement_time_like_cpp(&mut self, time: u32) {
        self.player_movement_time_like_cpp = time;
    }

    pub(crate) fn set_player_movement_flags_like_cpp(&mut self, flags: MovementFlag) {
        self.player_movement_flags_like_cpp = flags;
    }

    #[allow(dead_code)]
    pub(crate) fn set_player_liquid_status_like_cpp(&mut self, status: u32) {
        self.player_liquid_status_like_cpp = status;
    }

    pub(crate) fn set_player_level_like_cpp(&mut self, level: u8) {
        self.player_level = level;
        if let Some(controller) = &mut self.player_controller {
            controller.set_level(level);
        }
    }

    #[cfg(test)]
    pub(crate) fn set_player_class_like_cpp(&mut self, class: u8) {
        self.player_class = class;
        if let Some(controller) = &mut self.player_controller {
            controller.class = class;
        }
    }

    pub(crate) fn set_player_gold_like_cpp(&mut self, gold: u64) {
        self.player_gold = gold;
        if let Some(controller) = &mut self.player_controller {
            controller.set_gold(gold);
        }
    }

    pub(crate) fn set_player_xp_like_cpp(&mut self, xp: u32) {
        self.player_xp = xp;
        if let Some(controller) = &mut self.player_controller {
            controller.set_xp(xp);
        }
    }

    pub(crate) fn set_player_next_level_xp_like_cpp(&mut self, xp: u32) {
        self.player_next_level_xp = xp;
        if let Some(controller) = &mut self.player_controller {
            controller.set_next_level_xp(xp);
        }
    }

    pub(crate) fn set_selection_guid_like_cpp(&mut self, guid: Option<ObjectGuid>) {
        self.selection_guid = guid;
        if let Some(controller) = &mut self.player_controller {
            controller.set_selection_guid(guid);
        }
    }

    pub(crate) fn set_known_spells_like_cpp(&mut self, spells: Vec<i32>) {
        self.known_spells = spells.clone();
        if let Some(controller) = &mut self.player_controller {
            controller.set_known_spells(spells);
        }
    }

    pub(crate) fn set_account_mounts_like_cpp(&mut self, mounts: Vec<AccountMount>) {
        self.account_mounts_like_cpp = mounts
            .into_iter()
            .map(|mount| (mount.spell_id, mount.flags))
            .collect();
    }

    #[allow(dead_code)]
    pub(crate) fn set_represented_pet_mode_state_like_cpp(
        &mut self,
        pet_guid: Option<ObjectGuid>,
        react_state: u8,
        command_state: u8,
    ) {
        self.represented_pet_guid_like_cpp = pet_guid;
        self.represented_pet_react_state_like_cpp = react_state;
        self.represented_pet_command_state_like_cpp = command_state;
        self.temporary_mount_pet_react_state_like_cpp = None;
    }

    #[cfg(test)]
    pub(crate) fn account_mounts_like_cpp(&self) -> &HashMap<i32, u8> {
        &self.account_mounts_like_cpp
    }

    pub(crate) fn account_mount_rows_like_cpp(&self) -> Vec<AccountMount> {
        self.account_mounts_like_cpp
            .iter()
            .map(|(&spell_id, &flags)| AccountMount { spell_id, flags })
            .collect()
    }

    pub(crate) fn mount_set_favorite_like_cpp(
        &mut self,
        mount_spell_id: u32,
        is_favorite: bool,
    ) -> bool {
        let Ok(spell_id) = i32::try_from(mount_spell_id) else {
            return false;
        };
        let Some(flags) = self.account_mounts_like_cpp.get_mut(&spell_id) else {
            return false;
        };

        if is_favorite {
            *flags |= 0x01;
        } else {
            *flags &= !0x01;
        }
        let updated_flags = *flags;

        self.send_packet(&AccountMountUpdate::partial(vec![AccountMount {
            spell_id,
            flags: updated_flags,
        }]));
        true
    }

    pub(crate) fn set_player_skill_values_like_cpp(&mut self, skill_values: HashMap<u16, u16>) {
        self.player_skill_values_like_cpp = skill_values.clone();
        if let Some(controller) = &mut self.player_controller {
            controller.set_skill_values(skill_values);
        }
    }

    pub(crate) fn learn_known_spell_like_cpp(&mut self, spell_id: i32) {
        if !self.known_spells.contains(&spell_id) {
            self.known_spells.push(spell_id);
        }
        if let Some(controller) = &mut self.player_controller {
            controller.learn_spell(spell_id);
        }
    }

    pub(crate) fn sync_player_currencies_like_cpp(&mut self) {
        if let Some(controller) = &mut self.player_controller {
            controller.set_currencies(self.player_currencies.clone());
        }
    }

    pub(crate) fn set_player_currencies_like_cpp(
        &mut self,
        currencies: HashMap<u32, PlayerCurrency>,
    ) {
        self.player_currencies = currencies.clone();
        if let Some(controller) = &mut self.player_controller {
            controller.set_currencies(currencies);
        }
    }

    pub(crate) fn clear_player_currencies_like_cpp(&mut self) {
        self.set_player_currencies_like_cpp(HashMap::new());
    }

    pub(crate) fn session_player_inventory_runtime_like_cpp(
        &self,
    ) -> SessionPlayerInventoryRuntime {
        SessionPlayerInventoryRuntime {
            inventory_items: self.inventory_items.clone(),
            buyback_items: self.buyback_items.clone(),
            buyback_price: self.buyback_price,
            buyback_timestamp: self.buyback_timestamp,
            current_buyback_slot: self.current_buyback_slot,
            item_objects: self.inventory_item_objects.clone(),
        }
    }

    pub(crate) fn sync_player_inventory_like_cpp(&mut self) {
        let inventory = self.session_player_inventory_runtime_like_cpp();
        if let Some(controller) = &mut self.player_controller {
            controller.set_inventory(inventory);
        }
    }

    fn mirror_player_inventory_runtime_to_legacy_like_cpp(
        &mut self,
        inventory: &SessionPlayerInventoryRuntime,
    ) {
        self.inventory_items = inventory.inventory_items.clone();
        self.buyback_items = inventory.buyback_items.clone();
        self.buyback_price = inventory.buyback_price;
        self.buyback_timestamp = inventory.buyback_timestamp;
        self.current_buyback_slot = inventory.current_buyback_slot;
        self.inventory_item_objects = inventory.item_objects.clone();
    }

    pub(crate) fn mutate_player_inventory_runtime_like_cpp<R>(
        &mut self,
        update: impl FnOnce(&mut SessionPlayerInventoryRuntime) -> R,
    ) -> R {
        if self.player_controller.is_some() {
            let (result, inventory) = {
                let controller = self.player_controller.as_mut().expect("checked above");
                let result = update(controller.inventory_mut());
                (result, controller.inventory().clone())
            };
            self.mirror_player_inventory_runtime_to_legacy_like_cpp(&inventory);
            result
        } else {
            let mut inventory = self.session_player_inventory_runtime_like_cpp();
            let result = update(&mut inventory);
            self.mirror_player_inventory_runtime_to_legacy_like_cpp(&inventory);
            result
        }
    }

    pub(crate) fn player_name_like_cpp(&self) -> Option<&str> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::name)
            .or(self.player_name.as_deref())
    }

    pub(crate) fn player_position_like_cpp(&self) -> Option<wow_core::Position> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::position)
            .or(self.player_position)
    }

    pub(crate) fn player_map_id_like_cpp(&self) -> u16 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::map_id)
            .unwrap_or(self.current_map_id)
    }

    pub(crate) fn player_movement_flags_like_cpp(&self) -> MovementFlag {
        self.player_movement_flags_like_cpp
    }

    pub(crate) fn player_liquid_status_like_cpp(&self) -> u32 {
        self.player_liquid_status_like_cpp
    }

    pub(crate) fn player_race_like_cpp(&self) -> u8 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::race)
            .unwrap_or(self.player_race)
    }

    pub(crate) fn player_class_like_cpp(&self) -> u8 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::class)
            .unwrap_or(self.player_class)
    }

    pub(crate) fn player_level_like_cpp(&self) -> u8 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::level)
            .unwrap_or(self.player_level)
    }

    pub(crate) fn player_gender_like_cpp(&self) -> u8 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::gender)
            .unwrap_or(self.player_gender)
    }

    #[cfg(test)]
    pub(crate) fn loot_specialization_id_like_cpp(&self) -> u32 {
        self.loot_specialization_id
    }

    pub(crate) fn set_loot_specialization_id_like_cpp(&mut self, spec_id: u32) {
        self.loot_specialization_id = spec_id;
    }

    pub(crate) fn player_gold_like_cpp(&self) -> u64 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::gold)
            .unwrap_or(self.player_gold)
    }

    pub(crate) fn player_xp_like_cpp(&self) -> u32 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::xp)
            .unwrap_or(self.player_xp)
    }

    pub(crate) fn player_next_level_xp_like_cpp(&self) -> u32 {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::next_level_xp)
            .unwrap_or(self.player_next_level_xp)
    }

    #[allow(dead_code)]
    pub(crate) fn selection_guid_like_cpp(&self) -> Option<ObjectGuid> {
        self.player_controller
            .as_ref()
            .and_then(SessionPlayerController::selection_guid)
            .or(self.selection_guid)
    }

    pub(crate) fn known_spells_like_cpp(&self) -> &[i32] {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::known_spells)
            .unwrap_or(&self.known_spells)
    }

    pub(crate) fn player_skill_values_like_cpp(&self) -> &HashMap<u16, u16> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::skill_values)
            .unwrap_or(&self.player_skill_values_like_cpp)
    }

    pub(crate) fn player_skill_value_like_cpp(&self, skill_id: u16) -> u16 {
        self.player_skill_values_like_cpp()
            .get(&skill_id)
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn player_currencies_like_cpp(&self) -> &HashMap<u32, PlayerCurrency> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::currencies)
            .unwrap_or(&self.player_currencies)
    }

    pub(crate) fn represented_player_condition_context_like_cpp(
        &self,
    ) -> RepresentedPlayerConditionContextLikeCpp {
        let spells = self
            .known_spells_like_cpp()
            .iter()
            .filter_map(|spell_id| u32::try_from(*spell_id).ok())
            .collect();
        let items = self
            .represented_inventory_item_counts_like_cpp()
            .into_iter()
            .map(|(id, count)| PlayerConditionCountLikeCpp { id, count })
            .collect();
        let currencies = self
            .player_currencies_like_cpp()
            .iter()
            .map(|(&id, currency)| PlayerConditionCountLikeCpp {
                id,
                count: currency.quantity,
            })
            .collect();
        let completed_quests = self.rewarded_quests.iter().copied().collect();
        let current_quests = self
            .player_quests
            .iter()
            .filter_map(|(&quest_id, status)| {
                (status.status == 1 || status.status == 2).then_some(quest_id)
            })
            .collect();
        let complete_quests = self
            .player_quests
            .iter()
            .filter_map(|(&quest_id, status)| (status.status == 2).then_some(quest_id))
            .collect();
        let auras = self
            .visible_auras
            .values()
            .filter_map(|aura| {
                Some(PlayerConditionAuraLikeCpp {
                    spell_id: u32::try_from(aura.spell_id).ok()?,
                    stacks: aura.stack_count,
                })
            })
            .collect();
        let skills = self
            .player_skill_values_like_cpp()
            .iter()
            .map(|(&id, &value)| PlayerConditionSkillLikeCpp { id, value })
            .collect();

        let mut item_level_sum = 0u32;
        let mut item_level_count = 0u32;
        let mut equipped_level_sum = 0u32;
        let mut equipped_level_count = 0u32;
        let mut mainhand_weapon_subclass = None;
        for (&slot, inventory_item) in self.inventory_items_like_cpp() {
            let Some(template) = self
                .item_stats_store
                .as_ref()
                .and_then(|store| store.random_property_template(inventory_item.entry_id))
            else {
                continue;
            };
            item_level_sum = item_level_sum.saturating_add(u32::from(template.item_level));
            item_level_count = item_level_count.saturating_add(1);
            if is_equipment_packed_pos(make_item_pos(INVENTORY_SLOT_BAG_0, slot)) {
                equipped_level_sum =
                    equipped_level_sum.saturating_add(u32::from(template.item_level));
                equipped_level_count = equipped_level_count.saturating_add(1);
            }
            if slot == EQUIPMENT_SLOT_MAINHAND {
                mainhand_weapon_subclass = self
                    .item_store
                    .as_ref()
                    .and_then(|store| store.get(inventory_item.entry_id))
                    .map(|record| record.subclass_id);
            }
        }

        RepresentedPlayerConditionContextLikeCpp {
            spells,
            items,
            currencies,
            completed_quests,
            current_quests,
            complete_quests,
            auras,
            skills,
            avg_item_level: if item_level_count == 0 {
                0.0
            } else {
                item_level_sum as f32 / item_level_count as f32
            },
            avg_equipped_item_level: if equipped_level_count == 0 {
                0.0
            } else {
                equipped_level_sum as f32 / equipped_level_count as f32
            },
            mainhand_weapon_subclass,
            ..Default::default()
        }
    }

    pub(crate) fn represented_meets_player_condition_id_like_cpp(
        &self,
        player_condition_id: u32,
    ) -> bool {
        if player_condition_id == 0 {
            return true;
        }

        let Some(store) = self.player_condition_store.as_ref() else {
            return false;
        };
        let Some(condition) = store.get(player_condition_id) else {
            return true;
        };

        let context = self.represented_player_condition_context_like_cpp();
        is_player_meeting_condition_like_cpp(condition, &context.as_context(self))
    }

    #[allow(dead_code)]
    pub(crate) fn represented_taxi_edge_distance_like_cpp(
        &self,
        destination_has_required_team_flag: bool,
        destination_condition_id: u32,
        distance: u32,
    ) -> u32 {
        if !destination_has_required_team_flag {
            return u16::MAX as u32;
        }

        if !self.represented_meets_player_condition_id_like_cpp(destination_condition_id) {
            return u16::MAX as u32;
        }

        distance
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_x_display_usable_like_cpp(
        &self,
        player_condition_id: u32,
    ) -> bool {
        self.represented_meets_player_condition_id_like_cpp(player_condition_id)
    }

    #[allow(dead_code)]
    pub(crate) fn represented_taxi_usable_mount_displays_like_cpp(
        &self,
        flying_mount_id: u32,
    ) -> Vec<i32> {
        let Some(mount) = self
            .mount_store
            .as_ref()
            .and_then(|store| store.get_by_id(flying_mount_id))
        else {
            return Vec::new();
        };

        if !self
            .known_spells_like_cpp()
            .contains(&mount.source_spell_id)
        {
            return Vec::new();
        }

        let Some(displays) = self
            .mount_x_display_store
            .as_ref()
            .and_then(|store| store.displays_for_mount_like_cpp(mount.id))
        else {
            return Vec::new();
        };

        displays
            .iter()
            .filter(|display| {
                display.player_condition_id == 0
                    || self.represented_mount_x_display_usable_like_cpp(display.player_condition_id)
            })
            .map(|display| display.creature_display_info_id)
            .collect()
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_aura_display_candidates_like_cpp(
        &self,
        spell_id: u32,
    ) -> Vec<i32> {
        let Some(mount) = self
            .mount_store
            .as_ref()
            .and_then(|store| store.get_by_source_spell_id_like_cpp(spell_id))
        else {
            return Vec::new();
        };

        if mount.flags & wow_data::MOUNT_FLAG_SELF_MOUNT != 0 {
            return vec![wow_data::DISPLAYID_HIDDEN_MOUNT];
        }

        let Some(displays) = self
            .mount_x_display_store
            .as_ref()
            .and_then(|store| store.displays_for_mount_like_cpp(mount.id))
        else {
            return Vec::new();
        };

        displays
            .iter()
            .filter(|display| {
                display.player_condition_id == 0
                    || self.represented_mount_x_display_usable_like_cpp(display.player_condition_id)
            })
            .map(|display| display.creature_display_info_id)
            .collect()
    }

    #[allow(dead_code)]
    pub(crate) fn select_represented_mount_aura_display_like_cpp(
        &self,
        spell_id: u32,
    ) -> Option<i32> {
        let candidates = self.represented_mount_aura_display_candidates_like_cpp(spell_id);
        candidates.choose(&mut rand::thread_rng()).copied()
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_creature_template_fallback_like_cpp(
        &self,
        creature_entry: u32,
    ) -> Option<(i32, u32)> {
        let template = self
            .creature_template_mount_store
            .as_ref()?
            .get(creature_entry)?;
        let display_id = template.choose_display_id_like_cpp(&mut rand::thread_rng())?;
        Some((i32::try_from(display_id).unwrap_or(0), template.vehicle_id))
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_source_spell_usable_like_cpp(&self, spell_id: u32) -> bool {
        let Some(mount) = self
            .mount_store
            .as_ref()
            .and_then(|store| store.get_by_source_spell_id_like_cpp(spell_id))
        else {
            return false;
        };

        self.represented_meets_player_condition_id_like_cpp(mount.player_condition_id)
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_capability_for_type_like_cpp(
        &self,
        mount_type_id: u16,
        riding_skill: u32,
        mount_restriction_flags: Option<u8>,
        is_submerged: bool,
        is_in_water: bool,
    ) -> Option<wow_data::MountCapabilityEntry> {
        let capability_store = self.mount_capability_store.as_ref()?;
        let type_store = self.mount_type_x_capability_store.as_ref()?;
        let area_store = self.area_table_store.as_ref()?;

        let map_id = u32::from(self.player_map_id_like_cpp());
        let map = self.map_store.as_ref().and_then(|store| store.get(map_id));
        let (_, area_id) = self.player_zone_area_like_cpp();
        let mount_flags = mount_restriction_flags.unwrap_or_else(|| {
            area_store
                .get(area_id)
                .map(|area| area.mount_flags as u8)
                .unwrap_or(0)
        });
        let context = wow_data::MountCapabilityContextLikeCpp {
            riding_skill,
            mount_flags,
            is_submerged,
            is_in_water,
            map_id: map_id as i32,
            cosmetic_parent_map_id: map
                .map(|entry| i32::from(entry.cosmetic_parent_map_id))
                .unwrap_or(-1),
            parent_map_id: map
                .map(|entry| i32::from(entry.parent_map_id))
                .unwrap_or(-1),
        };

        capability_store
            .select_for_mount_type_like_cpp(
                type_store,
                mount_type_id,
                &context,
                |required_area_id| {
                    area_store.is_in_area_like_cpp(area_id, u32::from(required_area_id))
                },
                |aura_id| {
                    self.visible_auras
                        .values()
                        .any(|aura| u32::try_from(aura.spell_id).ok() == Some(aura_id))
                },
                |spell_id| self.known_spells_like_cpp().contains(&spell_id),
            )
            .copied()
    }

    #[allow(dead_code)]
    pub(crate) fn represented_mount_capability_for_type_from_session_like_cpp(
        &self,
        mount_type_id: u16,
        mount_restriction_flags: Option<u8>,
    ) -> Option<wow_data::MountCapabilityEntry> {
        let (is_submerged, is_in_water) = self.represented_player_mount_liquid_state_like_cpp();
        self.represented_mount_capability_for_type_like_cpp(
            mount_type_id,
            u32::from(self.player_skill_value_like_cpp(SKILL_RIDING_LIKE_CPP)),
            mount_restriction_flags,
            is_submerged,
            is_in_water,
        )
    }

    pub(crate) fn represented_player_mount_liquid_state_like_cpp(&self) -> (bool, bool) {
        let liquid_status = self.player_liquid_status_like_cpp();
        let is_submerged = liquid_status & LIQUID_MAP_UNDER_WATER_LIKE_CPP != 0
            || self
                .player_movement_flags_like_cpp()
                .contains(MovementFlag::SWIMMING);
        let is_in_water =
            liquid_status & (LIQUID_MAP_IN_WATER_LIKE_CPP | LIQUID_MAP_UNDER_WATER_LIKE_CPP) != 0;
        (is_submerged, is_in_water)
    }

    #[allow(dead_code)]
    pub(crate) fn represented_failed_map_difficulty_x_condition_like_cpp(
        &self,
        map_difficulty_id: u32,
    ) -> Option<u32> {
        let store = self.map_difficulty_x_condition_store.as_ref()?;
        let player_conditions = self.player_condition_store.as_ref()?;
        let context = self.represented_player_condition_context_like_cpp();
        store.failed_condition_like_cpp(map_difficulty_id, player_conditions, |condition| {
            is_player_meeting_condition_like_cpp(condition, &context.as_context(self))
        })
    }

    pub(crate) fn player_inventory_like_cpp(&self) -> Option<&SessionPlayerInventoryRuntime> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::inventory)
    }

    pub(crate) fn inventory_items_like_cpp(&self) -> &HashMap<u8, InventoryItem> {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::inventory_items)
            .unwrap_or(&self.inventory_items)
    }

    pub(crate) fn buyback_items_like_cpp(&self) -> &HashMap<u8, InventoryItem> {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::buyback_items)
            .unwrap_or(&self.buyback_items)
    }

    pub(crate) fn buyback_price_like_cpp(&self) -> &[u32; BUYBACK_SLOT_COUNT] {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::buyback_price)
            .unwrap_or(&self.buyback_price)
    }

    pub(crate) fn buyback_timestamp_like_cpp(&self) -> &[i64; BUYBACK_SLOT_COUNT] {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::buyback_timestamp)
            .unwrap_or(&self.buyback_timestamp)
    }

    pub(crate) fn current_buyback_slot_like_cpp(&self) -> u8 {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::current_buyback_slot)
            .unwrap_or(self.current_buyback_slot)
    }

    pub(crate) fn inventory_item_objects_like_cpp(&self) -> &HashMap<ObjectGuid, Item> {
        self.player_inventory_like_cpp()
            .map(SessionPlayerInventoryRuntime::item_objects)
            .unwrap_or(&self.inventory_item_objects)
    }

    pub fn set_player_alive_like_cpp(&mut self, alive: bool) {
        self.player_alive_like_cpp = alive;
        if !alive {
            self.player_health_like_cpp = 0;
        } else if self.player_health_like_cpp == 0 {
            self.player_health_like_cpp = self.player_max_health_like_cpp.max(1);
        }
    }

    pub(crate) fn player_is_alive_like_cpp(&self) -> bool {
        self.player_alive_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn set_player_game_master_like_cpp(&mut self, is_game_master: bool) {
        self.player_game_master_like_cpp = is_game_master;
    }

    #[cfg(test)]
    pub(crate) fn set_player_mounted_like_cpp(&mut self, mounted: bool) {
        self.player_mounted_like_cpp = mounted;
        self.player_mount_display_id_like_cpp = if mounted { 1 } else { 0 };
    }

    #[cfg(test)]
    pub(crate) fn set_player_cheat_god_like_cpp(&mut self, enabled: bool) {
        self.player_cheat_god_like_cpp = enabled;
    }

    #[cfg(test)]
    pub(crate) fn set_player_normal_damage_immune_like_cpp(&mut self, immune: bool) {
        self.player_normal_damage_immune_like_cpp = immune;
    }

    #[cfg(test)]
    pub(crate) fn set_player_environmental_damage_immune_like_cpp(&mut self, immune: bool) {
        self.player_environmental_damage_immune_like_cpp = immune;
    }

    pub(crate) fn set_player_health_like_cpp(&mut self, health: u32, max_health: u32) {
        self.player_max_health_like_cpp = max_health.max(1);
        self.player_health_like_cpp = health.min(self.player_max_health_like_cpp);
        self.player_alive_like_cpp = self.player_health_like_cpp > 0;
    }

    #[cfg(test)]
    pub(crate) fn player_health_like_cpp(&self) -> u32 {
        self.player_health_like_cpp
    }

    pub(crate) fn set_fall_information_like_cpp(&mut self, time: u32, z: f32) {
        self.last_fall_time_like_cpp = time;
        self.last_fall_z_like_cpp = z;
    }

    pub(crate) fn update_fall_information_if_needed_like_cpp(
        &mut self,
        movement_info: &wow_packet::packets::movement::MovementInfo,
        is_fall_land: bool,
    ) {
        if self.last_fall_time_like_cpp >= movement_info.jump.fall_time
            || self.last_fall_z_like_cpp <= movement_info.position.z
            || is_fall_land
        {
            self.set_fall_information_like_cpp(
                movement_info.jump.fall_time,
                movement_info.position.z,
            );
        }
    }

    pub(crate) fn handle_fall_like_cpp(
        &mut self,
        movement_info: &wow_packet::packets::movement::MovementInfo,
    ) -> Option<MovementFallDamageEvent> {
        let z_diff = self.last_fall_z_like_cpp - movement_info.position.z;
        if z_diff < 14.57
            || !self.player_alive_like_cpp
            || self.player_game_master_like_cpp
            || self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::Hover)
            || self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::FeatherFall)
            || self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::Fly)
            || self.player_normal_damage_immune_like_cpp
        {
            return None;
        }

        let safe_fall =
            self.total_represented_aura_modifier_like_cpp(RepresentedAuraEffectLikeCpp::SafeFall);
        let damage_percent = 0.018 * (z_diff - safe_fall as f32) - 0.2426;
        if damage_percent <= 0.0 {
            return None;
        }

        let mut damage = (damage_percent * self.player_max_health_like_cpp as f32) as u32;
        if self.player_cheat_god_like_cpp {
            damage = 0;
        }
        damage = (damage as f32
            * self.total_represented_aura_multiplier_like_cpp(
                RepresentedAuraEffectLikeCpp::ModifyFallDamagePct,
            )) as u32;
        if self
            .visible_auras
            .values()
            .any(|aura| aura.spell_id == 43_621)
        {
            damage = self.player_max_health_like_cpp / 2;
        }
        damage = damage.min(self.player_max_health_like_cpp);
        if damage == 0 {
            return None;
        }

        let original_health = self.player_health_like_cpp;
        let final_damage = if self.player_environmental_damage_immune_like_cpp {
            0
        } else {
            damage.min(original_health)
        };
        self.player_health_like_cpp = self.player_health_like_cpp.saturating_sub(final_damage);
        if self.player_health_like_cpp == 0 {
            self.player_alive_like_cpp = false;
        }

        let event = MovementFallDamageEvent {
            z_diff,
            damage,
            final_damage,
        };
        self.fall_damage_events_like_cpp.push(event);
        Some(event)
    }

    fn has_represented_aura_effect_like_cpp(&self, effect: RepresentedAuraEffectLikeCpp) -> bool {
        self.visible_auras
            .values()
            .any(|aura| aura.represented_effect == Some(effect))
    }

    fn total_represented_aura_modifier_like_cpp(
        &self,
        effect: RepresentedAuraEffectLikeCpp,
    ) -> i32 {
        self.visible_auras
            .values()
            .filter(|aura| aura.represented_effect == Some(effect))
            .map(|aura| aura.represented_amount)
            .sum()
    }

    fn total_represented_aura_multiplier_like_cpp(
        &self,
        effect: RepresentedAuraEffectLikeCpp,
    ) -> f32 {
        self.visible_auras
            .values()
            .filter(|aura| aura.represented_effect == Some(effect))
            .fold(1.0, |acc, aura| acc * aura.represented_multiplier)
    }

    #[cfg(test)]
    pub(crate) fn fall_damage_events_like_cpp(&self) -> &[MovementFallDamageEvent] {
        &self.fall_damage_events_like_cpp
    }

    pub(crate) fn player_min_height_like_cpp(&self, position: wow_core::Position) -> f32 {
        let map_id = self.player_map_id_like_cpp();
        self.map_manager
            .as_ref()
            .and_then(|manager| {
                manager
                    .read()
                    .ok()
                    .map(|manager| manager.min_height_like_cpp(map_id, 0, position.x, position.y))
            })
            .unwrap_or(crate::map_manager::DEFAULT_MIN_HEIGHT_LIKE_CPP)
    }

    pub(crate) fn handle_under_map_like_cpp(
        &mut self,
        movement_info: &wow_packet::packets::movement::MovementInfo,
    ) -> Option<MovementUnderMapDamageEvent> {
        let min_height = self.player_min_height_like_cpp(movement_info.position);
        if movement_info.position.z >= min_height {
            self.player_out_of_bounds_like_cpp = false;
            return None;
        }

        if !self.player_alive_like_cpp {
            return None;
        }

        self.player_out_of_bounds_like_cpp = true;
        let damage = self.player_max_health_like_cpp;
        self.player_health_like_cpp = self.player_health_like_cpp.saturating_sub(damage);
        if self.player_health_like_cpp == 0 {
            self.player_alive_like_cpp = false;
        }

        // C++ calls KillPlayer if EnvironmentalDamage did not kill due to GM/immunity.
        if self.player_alive_like_cpp {
            self.set_player_alive_like_cpp(false);
        }

        let event = MovementUnderMapDamageEvent {
            z: movement_info.position.z,
            min_height,
            damage,
        };
        self.under_map_damage_events_like_cpp.push(event);
        Some(event)
    }

    #[cfg(test)]
    pub(crate) fn under_map_damage_events_like_cpp(&self) -> &[MovementUnderMapDamageEvent] {
        &self.under_map_damage_events_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn player_out_of_bounds_like_cpp(&self) -> bool {
        self.player_out_of_bounds_like_cpp
    }

    pub(crate) fn set_player_stand_state_like_cpp(&mut self, state: UnitStandStateType) {
        self.player_stand_state_like_cpp = state;
    }

    #[cfg(test)]
    pub(crate) fn player_stand_state_like_cpp(&self) -> UnitStandStateType {
        self.player_stand_state_like_cpp
    }

    pub(crate) fn player_is_sit_state_like_cpp(&self) -> bool {
        matches!(
            self.player_stand_state_like_cpp,
            UnitStandStateType::Sit
                | UnitStandStateType::SitChair
                | UnitStandStateType::SitLowChair
                | UnitStandStateType::SitMediumChair
                | UnitStandStateType::SitHighChair
        )
    }

    pub(crate) fn request_temporary_pet_unsummon_like_cpp(&mut self) {
        self.temporary_pet_unsummon_requests_like_cpp = self
            .temporary_pet_unsummon_requests_like_cpp
            .saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn temporary_pet_unsummon_requests_like_cpp(&self) -> u32 {
        self.temporary_pet_unsummon_requests_like_cpp
    }

    pub(crate) fn request_jump_proc_like_cpp(&mut self) {
        self.movement_jump_proc_requests_like_cpp =
            self.movement_jump_proc_requests_like_cpp.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn movement_jump_proc_requests_like_cpp(&self) -> u32 {
        self.movement_jump_proc_requests_like_cpp
    }

    pub(crate) fn apply_move_init_active_mover_complete_like_cpp(&mut self, ticks: u32) {
        self.active_player_local_flags_like_cpp |=
            PLAYER_LOCAL_FLAG_OVERRIDE_TRANSPORT_SERVER_TIME_LIKE_CPP;
        self.active_player_transport_server_time_like_cpp =
            Self::game_time_ms_like_cpp().saturating_sub(ticks) as i32;
        self.movement_visibility_refresh_requests_like_cpp = self
            .movement_visibility_refresh_requests_like_cpp
            .saturating_add(1);
        self.send_active_player_transport_server_time_update_like_cpp();
    }

    fn send_active_player_transport_server_time_update_like_cpp(&self) {
        let Some(guid) = self.player_guid() else {
            return;
        };

        use wow_packet::packets::update::{ActivePlayerDataValuesUpdate, UpdateObject};

        let mut data = ActivePlayerDataValuesUpdate::default();
        set_active_player_update_bit_like_cpp(&mut data.active_player_data_mask, 38);
        set_active_player_update_bit_like_cpp(&mut data.active_player_data_mask, 69);
        set_active_player_update_bit_like_cpp(&mut data.active_player_data_mask, 70);
        set_active_player_update_bit_like_cpp(&mut data.active_player_data_mask, 118);
        data.local_flags = self.active_player_local_flags_like_cpp;
        data.transport_server_time = self.active_player_transport_server_time_like_cpp;
        self.send_packet(&UpdateObject::full_active_player_values_update(
            guid,
            self.player_map_id_like_cpp(),
            data,
        ));
    }

    #[cfg(test)]
    pub(crate) fn active_player_local_flags_like_cpp(&self) -> u32 {
        self.active_player_local_flags_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn active_player_transport_server_time_like_cpp(&self) -> i32 {
        self.active_player_transport_server_time_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn movement_visibility_refresh_requests_like_cpp(&self) -> u32 {
        self.movement_visibility_refresh_requests_like_cpp
    }

    pub(crate) fn validate_movement_ack_status_like_cpp(
        &self,
        status: &wow_packet::packets::movement::MovementInfo,
    ) -> bool {
        let Some(player_guid) = self.player_guid() else {
            return false;
        };

        status.guid == player_guid && status.position.is_valid_map_coord_like_cpp()
    }

    pub(crate) fn validate_and_sanitize_movement_ack_status_represented_like_cpp(
        &self,
        status: &mut wow_packet::packets::movement::MovementInfo,
    ) -> bool {
        if !self.validate_movement_ack_status_like_cpp(status) {
            return false;
        }

        self.sanitize_movement_info_flags_represented_like_cpp(status);
        true
    }

    pub(crate) fn sanitize_movement_info_flags_represented_like_cpp(
        &self,
        movement_info: &mut wow_packet::packets::movement::MovementInfo,
    ) -> MovementFlag {
        let mut removed = MovementFlag::empty();
        let remove_flags = |movement_info: &mut wow_packet::packets::movement::MovementInfo,
                            removed: &mut MovementFlag,
                            flags: MovementFlag| {
            movement_info.flags.remove(flags);
            *removed |= flags;
        };

        let root_allowed_by_fixed_vehicle_like_cpp = false;
        if movement_info.flags.contains(MovementFlag::ROOT) {
            if root_allowed_by_fixed_vehicle_like_cpp {
                if movement_info.flags.intersects(MovementFlag::MASK_MOVING) {
                    remove_flags(movement_info, &mut removed, MovementFlag::MASK_MOVING);
                }
            } else {
                remove_flags(movement_info, &mut removed, MovementFlag::ROOT);
            }
        }

        if movement_info.flags.contains(MovementFlag::HOVER)
            && !self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::Hover)
        {
            remove_flags(movement_info, &mut removed, MovementFlag::HOVER);
        }

        if movement_info
            .flags
            .contains(MovementFlag::ASCENDING | MovementFlag::DESCENDING)
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::ASCENDING | MovementFlag::DESCENDING,
            );
        }

        if movement_info
            .flags
            .contains(MovementFlag::LEFT | MovementFlag::RIGHT)
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::LEFT | MovementFlag::RIGHT,
            );
        }

        if movement_info
            .flags
            .contains(MovementFlag::STRAFE_LEFT | MovementFlag::STRAFE_RIGHT)
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::STRAFE_LEFT | MovementFlag::STRAFE_RIGHT,
            );
        }

        if movement_info
            .flags
            .contains(MovementFlag::PITCH_UP | MovementFlag::PITCH_DOWN)
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::PITCH_UP | MovementFlag::PITCH_DOWN,
            );
        }

        if movement_info
            .flags
            .contains(MovementFlag::FORWARD | MovementFlag::BACKWARD)
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::FORWARD | MovementFlag::BACKWARD,
            );
        }

        if movement_info.flags.contains(MovementFlag::WATER_WALK)
            && !self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::WaterWalk)
            && !self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::Ghost)
        {
            remove_flags(movement_info, &mut removed, MovementFlag::WATER_WALK);
        }

        if movement_info.flags.contains(MovementFlag::FALLING_SLOW)
            && !self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::FeatherFall)
        {
            remove_flags(movement_info, &mut removed, MovementFlag::FALLING_SLOW);
        }

        if movement_info
            .flags
            .intersects(MovementFlag::FLYING | MovementFlag::CAN_FLY)
            && !self.player_game_master_like_cpp
            && !self.has_represented_aura_effect_like_cpp(RepresentedAuraEffectLikeCpp::Fly)
            && !self.has_represented_aura_effect_like_cpp(
                RepresentedAuraEffectLikeCpp::MountedFlightSpeed,
            )
        {
            remove_flags(
                movement_info,
                &mut removed,
                MovementFlag::FLYING | MovementFlag::CAN_FLY,
            );
        }

        if movement_info
            .flags
            .intersects(MovementFlag::DISABLE_GRAVITY | MovementFlag::CAN_FLY)
            && movement_info.flags.contains(MovementFlag::FALLING)
        {
            remove_flags(movement_info, &mut removed, MovementFlag::FALLING);
        }

        let has_step_up_elevation_like_cpp =
            movement_info.step_up_start_elevation.abs() > f32::EPSILON;
        if movement_info.flags.contains(MovementFlag::SPLINE_ELEVATION)
            && !has_step_up_elevation_like_cpp
        {
            remove_flags(movement_info, &mut removed, MovementFlag::SPLINE_ELEVATION);
        }

        if has_step_up_elevation_like_cpp {
            movement_info.flags.insert(MovementFlag::SPLINE_ELEVATION);
        }

        removed
    }

    pub(crate) fn record_movement_ack_event_like_cpp(&mut self, event: MovementAckEventLikeCpp) {
        self.movement_ack_events_like_cpp.push(event);
    }

    pub(crate) fn apply_knock_back_ack_like_cpp(
        &mut self,
        opcode: ClientOpcodes,
        ack: &mut wow_packet::packets::movement::MovementAck,
    ) -> bool {
        if !self.validate_and_sanitize_movement_ack_status_represented_like_cpp(&mut ack.status) {
            self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
                opcode,
                mover_guid: ack.status.guid,
                ack_index: Some(ack.ack_index),
                movement_force_id: None,
                movement_force_type: None,
                adjusted_time: None,
                speed: None,
                time_skipped: None,
                spline_id: None,
                accepted: false,
            });
            return false;
        }

        let mut status = ack.status.clone();
        status.time = self.adjust_client_movement_time_like_cpp(status.time);
        self.player_movement_time_like_cpp = status.time;
        self.set_player_movement_flags_like_cpp(status.flags);
        self.set_player_position_like_cpp(status.position);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode,
            mover_guid: status.guid,
            ack_index: Some(ack.ack_index),
            movement_force_id: None,
            movement_force_type: None,
            adjusted_time: Some(status.time),
            speed: None,
            time_skipped: None,
            spline_id: None,
            accepted: true,
        });
        true
    }

    pub(crate) fn apply_move_time_skipped_like_cpp(
        &mut self,
        mover_guid: ObjectGuid,
        time_skipped: u32,
    ) -> bool {
        let accepted = self.player_guid() == Some(mover_guid);
        if accepted {
            self.player_movement_time_like_cpp = self
                .player_movement_time_like_cpp
                .saturating_add(time_skipped);
        }

        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode: ClientOpcodes::MoveTimeSkipped,
            mover_guid,
            ack_index: None,
            movement_force_id: None,
            movement_force_type: None,
            adjusted_time: accepted.then_some(self.player_movement_time_like_cpp),
            speed: None,
            time_skipped: Some(time_skipped),
            spline_id: None,
            accepted,
        });
        accepted
    }

    pub(crate) fn record_validated_movement_ack_like_cpp(
        &mut self,
        opcode: ClientOpcodes,
        ack: &mut wow_packet::packets::movement::MovementAck,
        speed: Option<f32>,
    ) -> bool {
        let accepted =
            self.validate_and_sanitize_movement_ack_status_represented_like_cpp(&mut ack.status);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode,
            mover_guid: ack.status.guid,
            ack_index: Some(ack.ack_index),
            movement_force_id: None,
            movement_force_type: None,
            adjusted_time: None,
            speed,
            time_skipped: None,
            spline_id: None,
            accepted,
        });
        accepted
    }

    pub(crate) fn record_apply_movement_force_ack_like_cpp(
        &mut self,
        ack: &mut wow_packet::packets::movement::MovementAck,
        force: &wow_packet::packets::movement::MovementForce,
    ) -> bool {
        if !self.validate_and_sanitize_movement_ack_status_represented_like_cpp(&mut ack.status) {
            self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
                opcode: ClientOpcodes::MoveApplyMovementForceAck,
                mover_guid: ack.status.guid,
                ack_index: Some(ack.ack_index),
                movement_force_id: Some(force.id),
                movement_force_type: Some(force.force_type.to_wire()),
                adjusted_time: None,
                speed: None,
                time_skipped: None,
                spline_id: None,
                accepted: false,
            });
            return false;
        }

        let adjusted_time = self.adjust_client_movement_time_like_cpp(ack.status.time);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode: ClientOpcodes::MoveApplyMovementForceAck,
            mover_guid: ack.status.guid,
            ack_index: Some(ack.ack_index),
            movement_force_id: Some(force.id),
            movement_force_type: Some(force.force_type.to_wire()),
            adjusted_time: Some(adjusted_time),
            speed: None,
            time_skipped: None,
            spline_id: None,
            accepted: true,
        });
        true
    }

    pub(crate) fn record_remove_movement_force_ack_like_cpp(
        &mut self,
        ack: &mut wow_packet::packets::movement::MovementAck,
        force_id: ObjectGuid,
    ) -> bool {
        if !self.validate_and_sanitize_movement_ack_status_represented_like_cpp(&mut ack.status) {
            self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
                opcode: ClientOpcodes::MoveRemoveMovementForceAck,
                mover_guid: ack.status.guid,
                ack_index: Some(ack.ack_index),
                movement_force_id: Some(force_id),
                movement_force_type: None,
                adjusted_time: None,
                speed: None,
                time_skipped: None,
                spline_id: None,
                accepted: false,
            });
            return false;
        }

        let adjusted_time = self.adjust_client_movement_time_like_cpp(ack.status.time);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode: ClientOpcodes::MoveRemoveMovementForceAck,
            mover_guid: ack.status.guid,
            ack_index: Some(ack.ack_index),
            movement_force_id: Some(force_id),
            movement_force_type: None,
            adjusted_time: Some(adjusted_time),
            speed: None,
            time_skipped: None,
            spline_id: None,
            accepted: true,
        });
        true
    }

    pub(crate) fn record_move_spline_done_like_cpp(
        &mut self,
        status: &mut wow_packet::packets::movement::MovementInfo,
        spline_id: i32,
    ) -> bool {
        let accepted = self.validate_and_sanitize_movement_ack_status_represented_like_cpp(status);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode: ClientOpcodes::MoveSplineDone,
            mover_guid: status.guid,
            ack_index: None,
            movement_force_id: None,
            movement_force_type: None,
            adjusted_time: None,
            speed: None,
            time_skipped: None,
            spline_id: Some(spline_id),
            accepted,
        });
        accepted
    }

    pub(crate) fn handle_move_spline_done_taxi_like_cpp(
        &mut self,
        status: &mut wow_packet::packets::movement::MovementInfo,
        spline_id: i32,
    ) -> MoveSplineDoneTaxiActionLikeCpp {
        if !self.record_move_spline_done_like_cpp(status, spline_id) {
            return self.record_move_spline_done_taxi_event_like_cpp(
                spline_id,
                MoveSplineDoneTaxiActionLikeCpp::InvalidMovement,
                None,
                None,
                None,
                false,
            );
        }

        let current_destination = self.taxi_destinations_like_cpp.get(1).copied();
        if let Some(destination_node_id) = current_destination {
            let Some(flight) = self.taxi_flight_state_like_cpp else {
                return self.record_move_spline_done_taxi_event_like_cpp(
                    spline_id,
                    MoveSplineDoneTaxiActionLikeCpp::InProgressNoFlightGenerator,
                    Some(destination_node_id),
                    None,
                    None,
                    false,
                );
            };

            let destination_map_id = self
                .taxi_node_map_ids_like_cpp
                .get(&destination_node_id)
                .copied();
            let should_teleport = destination_map_id
                .map(|map_id| map_id != self.player_map_id_like_cpp())
                .unwrap_or(false)
                || flight.current_node.teleport_flag;

            if should_teleport {
                if let (Some(map_id), Some(node)) = (destination_map_id, flight.node_after_teleport)
                {
                    self.taxi_flight_state_like_cpp = Some(RepresentedTaxiFlightStateLikeCpp {
                        current_node: node,
                        node_after_teleport: None,
                    });
                    self.set_player_map_position_like_cpp(map_id, node.position);
                    return self.record_move_spline_done_taxi_event_like_cpp(
                        spline_id,
                        MoveSplineDoneTaxiActionLikeCpp::TeleportRequested,
                        Some(destination_node_id),
                        Some(map_id),
                        Some(node.position),
                        false,
                    );
                }
            }

            return self.record_move_spline_done_taxi_event_like_cpp(
                spline_id,
                MoveSplineDoneTaxiActionLikeCpp::InProgressNoTeleport,
                Some(destination_node_id),
                None,
                None,
                false,
            );
        }

        if self.taxi_destinations_like_cpp.len() != 1 {
            return self.record_move_spline_done_taxi_event_like_cpp(
                spline_id,
                MoveSplineDoneTaxiActionLikeCpp::IgnoredUnexpectedFinalPath,
                None,
                None,
                None,
                false,
            );
        }

        self.taxi_destinations_like_cpp.clear();
        self.taxi_flight_state_like_cpp = None;
        self.taxi_mounted_like_cpp = false;
        self.taxi_unit_flags_like_cpp
            .remove(UnitFlags::REMOVE_CLIENT_CONTROL | UnitFlags::ON_TAXI);
        let current_z = self
            .player_position_like_cpp()
            .map(|position| position.z)
            .unwrap_or(status.position.z);
        self.set_fall_information_like_cpp(0, current_z);
        let honorless_target_cast = self.player_pvp_hostile_like_cpp;

        self.record_move_spline_done_taxi_event_like_cpp(
            spline_id,
            MoveSplineDoneTaxiActionLikeCpp::FinalCleanup,
            None,
            None,
            None,
            honorless_target_cast,
        )
    }

    fn record_move_spline_done_taxi_event_like_cpp(
        &mut self,
        spline_id: i32,
        action: MoveSplineDoneTaxiActionLikeCpp,
        destination_node_id: Option<u32>,
        teleport_map_id: Option<u16>,
        teleport_position: Option<wow_core::Position>,
        honorless_target_cast: bool,
    ) -> MoveSplineDoneTaxiActionLikeCpp {
        self.move_spline_done_taxi_events_like_cpp
            .push(MoveSplineDoneTaxiEventLikeCpp {
                spline_id,
                action,
                destination_node_id,
                teleport_map_id,
                teleport_position,
                honorless_target_cast,
            });
        action
    }

    pub(crate) fn record_move_teleport_ack_like_cpp(
        &mut self,
        mover_guid: ObjectGuid,
        ack_index: i32,
        move_time: i32,
    ) -> bool {
        let accepted = self.player_guid() == Some(mover_guid);
        self.record_movement_ack_event_like_cpp(MovementAckEventLikeCpp {
            opcode: ClientOpcodes::MoveTeleportAck,
            mover_guid,
            ack_index: Some(ack_index),
            movement_force_id: None,
            movement_force_type: None,
            adjusted_time: (move_time >= 0).then_some(move_time as u32),
            speed: None,
            time_skipped: None,
            spline_id: None,
            accepted,
        });
        accepted
    }

    pub(crate) fn handle_move_teleport_ack_like_cpp(
        &mut self,
        mover_guid: ObjectGuid,
        ack_index: i32,
        move_time: i32,
    ) -> MoveTeleportAckActionLikeCpp {
        let accepted = self.record_move_teleport_ack_like_cpp(mover_guid, ack_index, move_time);
        if !self.near_teleport_pending_like_cpp {
            return self.record_move_teleport_ack_event_like_cpp(
                mover_guid,
                ack_index,
                move_time,
                MoveTeleportAckActionLikeCpp::NotBeingTeleportedNear,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                false,
                false,
            );
        }

        if !accepted {
            return self.record_move_teleport_ack_event_like_cpp(
                mover_guid,
                ack_index,
                move_time,
                MoveTeleportAckActionLikeCpp::WrongMover,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                false,
                false,
            );
        }

        let Some((map_id, destination)) = self.near_teleport_destination_like_cpp else {
            return self.record_move_teleport_ack_event_like_cpp(
                mover_guid,
                ack_index,
                move_time,
                MoveTeleportAckActionLikeCpp::MissingDestination,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                false,
                false,
            );
        };

        self.near_teleport_pending_like_cpp = false;
        let old_zone = self.player_zone_id_like_cpp;
        self.set_player_map_position_like_cpp(map_id, destination);
        self.update_registry_position();
        self.set_fall_information_like_cpp(0, destination.z);

        let (new_zone, new_area) = self
            .near_teleport_destination_zone_area_like_cpp
            .unwrap_or((self.player_zone_id_like_cpp, self.player_area_id_like_cpp));
        self.player_zone_id_like_cpp = new_zone;
        self.player_area_id_like_cpp = new_area;

        let zone_changed = old_zone != new_zone;
        let honorless_target_cast = zone_changed && self.player_pvp_hostile_like_cpp;
        let pvp_disabled = zone_changed
            && !self.player_pvp_hostile_like_cpp
            && self.player_pvp_enabled_like_cpp
            && !self.player_in_pvp_flag_like_cpp;
        if pvp_disabled {
            self.player_pvp_enabled_like_cpp = false;
        }

        self.temporary_pet_resummon_requests_like_cpp = self
            .temporary_pet_resummon_requests_like_cpp
            .saturating_add(1);
        self.delayed_operations_processed_like_cpp =
            self.delayed_operations_processed_like_cpp.saturating_add(1);

        self.record_move_teleport_ack_event_like_cpp(
            mover_guid,
            ack_index,
            move_time,
            MoveTeleportAckActionLikeCpp::Accepted,
            Some(map_id),
            Some(destination),
            Some(old_zone),
            Some(new_zone),
            Some(new_area),
            honorless_target_cast,
            pvp_disabled,
            true,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_move_teleport_ack_event_like_cpp(
        &mut self,
        mover_guid: ObjectGuid,
        ack_index: i32,
        move_time: i32,
        action: MoveTeleportAckActionLikeCpp,
        destination_map_id: Option<u16>,
        destination_position: Option<wow_core::Position>,
        old_zone_id: Option<u32>,
        new_zone_id: Option<u32>,
        new_area_id: Option<u32>,
        honorless_target_cast: bool,
        pvp_disabled: bool,
        pet_resummon_requested: bool,
        delayed_operations_processed: bool,
    ) -> MoveTeleportAckActionLikeCpp {
        self.move_teleport_ack_events_like_cpp
            .push(MoveTeleportAckEventLikeCpp {
                mover_guid,
                ack_index,
                move_time,
                action,
                destination_map_id,
                destination_position,
                old_zone_id,
                new_zone_id,
                new_area_id,
                honorless_target_cast,
                pvp_disabled,
                pet_resummon_requested,
                delayed_operations_processed,
            });
        action
    }

    #[cfg(test)]
    pub(crate) fn movement_ack_events_like_cpp(&self) -> &[MovementAckEventLikeCpp] {
        &self.movement_ack_events_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn set_taxi_destinations_like_cpp(&mut self, destinations: Vec<u32>) {
        self.taxi_destinations_like_cpp = destinations;
    }

    #[cfg(test)]
    pub(crate) fn taxi_destinations_like_cpp(&self) -> &[u32] {
        &self.taxi_destinations_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn set_taxi_node_map_id_like_cpp(&mut self, node_id: u32, map_id: u16) {
        self.taxi_node_map_ids_like_cpp.insert(node_id, map_id);
    }

    #[cfg(test)]
    pub(crate) fn set_taxi_flight_state_like_cpp(
        &mut self,
        current_node: RepresentedTaxiFlightNodeLikeCpp,
        node_after_teleport: Option<RepresentedTaxiFlightNodeLikeCpp>,
    ) {
        self.taxi_flight_state_like_cpp = Some(RepresentedTaxiFlightStateLikeCpp {
            current_node,
            node_after_teleport,
        });
    }

    #[cfg(test)]
    pub(crate) fn set_taxi_cleanup_state_like_cpp(&mut self, unit_flags: UnitFlags, mounted: bool) {
        self.taxi_unit_flags_like_cpp = unit_flags;
        self.taxi_mounted_like_cpp = mounted;
    }

    #[cfg(test)]
    pub(crate) fn taxi_unit_flags_like_cpp(&self) -> UnitFlags {
        self.taxi_unit_flags_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn taxi_mounted_like_cpp(&self) -> bool {
        self.taxi_mounted_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn set_player_pvp_hostile_like_cpp(&mut self, hostile: bool) {
        self.player_pvp_hostile_like_cpp = hostile;
    }

    #[cfg(test)]
    pub(crate) fn fall_information_like_cpp(&self) -> (u32, f32) {
        (self.last_fall_time_like_cpp, self.last_fall_z_like_cpp)
    }

    #[cfg(test)]
    pub(crate) fn move_spline_done_taxi_events_like_cpp(
        &self,
    ) -> &[MoveSplineDoneTaxiEventLikeCpp] {
        &self.move_spline_done_taxi_events_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn set_player_zone_area_like_cpp(&mut self, zone_id: u32, area_id: u32) {
        self.player_zone_id_like_cpp = zone_id;
        self.player_area_id_like_cpp = area_id;
    }

    pub(crate) fn player_zone_area_like_cpp(&self) -> (u32, u32) {
        (self.player_zone_id_like_cpp, self.player_area_id_like_cpp)
    }

    #[cfg(test)]
    pub(crate) fn set_player_pvp_state_like_cpp(
        &mut self,
        hostile: bool,
        pvp_enabled: bool,
        in_pvp_flag: bool,
    ) {
        self.player_pvp_hostile_like_cpp = hostile;
        self.player_pvp_enabled_like_cpp = pvp_enabled;
        self.player_in_pvp_flag_like_cpp = in_pvp_flag;
    }

    #[cfg(test)]
    pub(crate) fn set_near_teleport_pending_like_cpp(
        &mut self,
        pending: bool,
        destination: Option<(u16, wow_core::Position)>,
        zone_area: Option<(u32, u32)>,
    ) {
        self.near_teleport_pending_like_cpp = pending;
        self.near_teleport_destination_like_cpp = destination;
        self.near_teleport_destination_zone_area_like_cpp = zone_area;
    }

    #[cfg(test)]
    pub(crate) fn near_teleport_pending_like_cpp(&self) -> bool {
        self.near_teleport_pending_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn move_teleport_ack_events_like_cpp(&self) -> &[MoveTeleportAckEventLikeCpp] {
        &self.move_teleport_ack_events_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn temporary_pet_resummon_requests_like_cpp(&self) -> u32 {
        self.temporary_pet_resummon_requests_like_cpp
    }

    #[cfg(test)]
    pub(crate) fn delayed_operations_processed_like_cpp(&self) -> u32 {
        self.delayed_operations_processed_like_cpp
    }

    pub(crate) fn player_movement_time_like_cpp(&self) -> u32 {
        self.player_movement_time_like_cpp
    }

    pub(crate) fn latest_movement_ack_adjusted_time_like_cpp(&self) -> Option<u32> {
        self.movement_ack_events_like_cpp
            .last()
            .and_then(|event| event.adjusted_time)
    }

    pub(crate) fn movement_speed_ack_move_type_like_cpp(
        opcode: ClientOpcodes,
    ) -> Option<UnitMoveTypeLikeCpp> {
        match opcode {
            ClientOpcodes::MoveForceWalkSpeedChangeAck => Some(UnitMoveTypeLikeCpp::Walk),
            ClientOpcodes::MoveForceRunSpeedChangeAck => Some(UnitMoveTypeLikeCpp::Run),
            ClientOpcodes::MoveForceRunBackSpeedChangeAck => Some(UnitMoveTypeLikeCpp::RunBack),
            ClientOpcodes::MoveForceSwimSpeedChangeAck => Some(UnitMoveTypeLikeCpp::Swim),
            ClientOpcodes::MoveForceSwimBackSpeedChangeAck => Some(UnitMoveTypeLikeCpp::SwimBack),
            ClientOpcodes::MoveForceTurnRateChangeAck => Some(UnitMoveTypeLikeCpp::TurnRate),
            ClientOpcodes::MoveForceFlightSpeedChangeAck => Some(UnitMoveTypeLikeCpp::Flight),
            ClientOpcodes::MoveForceFlightBackSpeedChangeAck => {
                Some(UnitMoveTypeLikeCpp::FlightBack)
            }
            ClientOpcodes::MoveForcePitchRateChangeAck => Some(UnitMoveTypeLikeCpp::PitchRate),
            _ => None,
        }
    }

    pub(crate) fn player_movement_speed_like_cpp(&self, move_type: UnitMoveTypeLikeCpp) -> f32 {
        PLAYER_BASE_MOVE_SPEED_LIKE_CPP[move_type.index()]
            * self.movement_speed_rates_like_cpp[move_type.index()]
    }

    pub(crate) fn handle_force_speed_change_ack_like_cpp(
        &mut self,
        opcode: ClientOpcodes,
        ack: &mut wow_packet::packets::movement::MovementAck,
        speed: f32,
    ) -> bool {
        let Some(move_type) = Self::movement_speed_ack_move_type_like_cpp(opcode) else {
            self.movement_speed_ack_events_like_cpp
                .push(MovementSpeedAckEventLikeCpp {
                    opcode,
                    move_type: None,
                    ack_speed: speed,
                    expected_speed: None,
                    remaining_forced_changes: None,
                    action: MovementSpeedAckActionLikeCpp::Kicked,
                });
            return false;
        };

        if !self.record_validated_movement_ack_like_cpp(opcode, ack, Some(speed)) {
            self.movement_speed_ack_events_like_cpp
                .push(MovementSpeedAckEventLikeCpp {
                    opcode,
                    move_type: Some(move_type),
                    ack_speed: speed,
                    expected_speed: None,
                    remaining_forced_changes: None,
                    action: MovementSpeedAckActionLikeCpp::Kicked,
                });
            return false;
        }

        let index = move_type.index();
        if self.forced_speed_changes_like_cpp[index] > 0 {
            self.forced_speed_changes_like_cpp[index] =
                self.forced_speed_changes_like_cpp[index].saturating_sub(1);
            if self.forced_speed_changes_like_cpp[index] > 0 {
                self.movement_speed_ack_events_like_cpp
                    .push(MovementSpeedAckEventLikeCpp {
                        opcode,
                        move_type: Some(move_type),
                        ack_speed: speed,
                        expected_speed: Some(self.player_movement_speed_like_cpp(move_type)),
                        remaining_forced_changes: Some(self.forced_speed_changes_like_cpp[index]),
                        action: MovementSpeedAckActionLikeCpp::SkippedPending,
                    });
                return true;
            }
        }

        let expected_speed = self.player_movement_speed_like_cpp(move_type);
        let action = if !self.player_on_transport_like_cpp && (expected_speed - speed).abs() > 0.01
        {
            if expected_speed > speed {
                // C++ calls SetSpeedRate(GetSpeedRate()) to force the client back to the server value.
                MovementSpeedAckActionLikeCpp::Corrected
            } else {
                self.kick("WorldSession::HandleForceSpeedChangeAck Incorrect speed");
                MovementSpeedAckActionLikeCpp::Kicked
            }
        } else {
            MovementSpeedAckActionLikeCpp::Accepted
        };

        self.movement_speed_ack_events_like_cpp
            .push(MovementSpeedAckEventLikeCpp {
                opcode,
                move_type: Some(move_type),
                ack_speed: speed,
                expected_speed: Some(expected_speed),
                remaining_forced_changes: Some(self.forced_speed_changes_like_cpp[index]),
                action,
            });
        !matches!(action, MovementSpeedAckActionLikeCpp::Kicked)
    }

    pub(crate) fn handle_movement_force_mod_magnitude_ack_like_cpp(
        &mut self,
        opcode: ClientOpcodes,
        ack: &mut wow_packet::packets::movement::MovementAck,
        speed: f32,
    ) -> bool {
        if !self.record_validated_movement_ack_like_cpp(opcode, ack, Some(speed)) {
            self.movement_speed_ack_events_like_cpp
                .push(MovementSpeedAckEventLikeCpp {
                    opcode,
                    move_type: None,
                    ack_speed: speed,
                    expected_speed: None,
                    remaining_forced_changes: None,
                    action: MovementSpeedAckActionLikeCpp::Kicked,
                });
            return false;
        }

        let mut action = MovementSpeedAckActionLikeCpp::Accepted;
        if self.movement_force_mod_magnitude_changes_like_cpp > 0 {
            self.movement_force_mod_magnitude_changes_like_cpp = self
                .movement_force_mod_magnitude_changes_like_cpp
                .saturating_sub(1);
            if self.movement_force_mod_magnitude_changes_like_cpp == 0
                && (self.movement_force_mod_magnitude_like_cpp - speed).abs() > 0.01
            {
                self.kick(
                    "WorldSession::HandleMoveSetModMovementForceMagnitudeAck Incorrect magnitude",
                );
                action = MovementSpeedAckActionLikeCpp::Kicked;
            }
        }

        self.movement_speed_ack_events_like_cpp
            .push(MovementSpeedAckEventLikeCpp {
                opcode,
                move_type: None,
                ack_speed: speed,
                expected_speed: Some(self.movement_force_mod_magnitude_like_cpp),
                remaining_forced_changes: Some(self.movement_force_mod_magnitude_changes_like_cpp),
                action,
            });
        !matches!(action, MovementSpeedAckActionLikeCpp::Kicked)
    }

    #[cfg(test)]
    pub(crate) fn set_forced_speed_changes_like_cpp(
        &mut self,
        move_type: UnitMoveTypeLikeCpp,
        count: u8,
    ) {
        self.forced_speed_changes_like_cpp[move_type.index()] = count;
    }

    #[cfg(test)]
    pub(crate) fn set_player_movement_speed_rate_like_cpp(
        &mut self,
        move_type: UnitMoveTypeLikeCpp,
        rate: f32,
    ) {
        self.movement_speed_rates_like_cpp[move_type.index()] = rate.max(0.01);
    }

    #[cfg(test)]
    pub(crate) fn set_player_on_transport_like_cpp(&mut self, on_transport: bool) {
        self.player_on_transport_like_cpp = on_transport;
    }

    #[cfg(test)]
    pub(crate) fn set_movement_force_mod_magnitude_changes_like_cpp(&mut self, count: u8) {
        self.movement_force_mod_magnitude_changes_like_cpp = count;
    }

    #[cfg(test)]
    pub(crate) fn set_movement_force_mod_magnitude_like_cpp(&mut self, magnitude: f32) {
        self.movement_force_mod_magnitude_like_cpp = magnitude;
    }

    #[cfg(test)]
    pub(crate) fn movement_speed_ack_events_like_cpp(&self) -> &[MovementSpeedAckEventLikeCpp] {
        &self.movement_speed_ack_events_like_cpp
    }

    pub(crate) fn interrupt_non_melee_spell_cast_for_loot_like_cpp(&mut self) -> bool {
        self.active_spell_cast.take().is_some()
    }

    /// Get the logged-in player GUID.
    pub fn player_guid(&self) -> Option<ObjectGuid> {
        self.player_controller
            .as_ref()
            .map(SessionPlayerController::guid)
            .or(self.player_guid)
    }

    /// Complete the logout: send LogoutComplete and mark session for disconnect.
    fn complete_logout(&mut self) {
        use wow_packet::packets::misc::LogoutComplete;

        info!("Logout complete for account {}", self.account_id);
        self.send_packet(&LogoutComplete);
        self.set_player_guid(None);
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
        use wow_packet::ServerPacket;
        use wow_packet::packets::movement::{MonsterMove, MovementMonsterSpline};

        // Collect packets to send (avoids borrow conflict with send_packet)
        let mut to_send: Vec<Vec<u8>> = Vec::new();

        let guids = self.world_creature_guids();

        // ── Corpse despawn ─────────────────────────────────────────────────
        // C# ref: `Creature.RemoveCorpse` / `AllLootRemovedFromCorpse`.
        // After `corpse_despawn_at` passes, remove the dead creature from the
        // world and notify the client (destroy block in SMSG_UPDATE_OBJECT).
        let now = std::time::Instant::now();
        let despawn_guids: Vec<wow_core::ObjectGuid> = guids
            .iter()
            .filter(|g| {
                self.mutate_world_creature(**g, |c| {
                    !c.is_alive() && c.corpse_despawn_at().map(|t| now >= t).unwrap_or(false)
                })
                .unwrap_or(false)
            })
            .copied()
            .collect();

        if !despawn_guids.is_empty() {
            use wow_packet::ServerPacket;
            use wow_packet::packets::update::{CreatureCreateData, UpdateObject};

            let map_id = self.player_map_id_like_cpp();
            for g in &despawn_guids {
                // Before removing, save data needed for respawn.
                if let Some(c) = self.remove_world_creature(*g) {
                    // C# ref: AllLootRemovedFromCorpse → m_respawnTime = corpseRemoveTime + respawnDelay
                    let respawn_at = now
                        + std::time::Duration::from_secs(
                            c.creature.ai_ownership().respawn_time_secs,
                        );
                    // Build CreatureCreateData from saved AI fields (with sensible defaults
                    // for fields not stored in CreatureAI: scale, unit_class, timers, speeds).
                    let create_data = CreatureCreateData {
                        guid: c.guid(),
                        entry: c.entry(),
                        display_id: c.display_id(),
                        native_display_id: c.display_id(),
                        health: c.max_hp() as i64,
                        max_health: c.max_hp() as i64,
                        level: c.level(),
                        faction_template: c.faction() as i32,
                        npc_flags: c.npc_flags() as u64,
                        unit_flags: c.unit_flags(),
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
                        home_pos: c.home_position(),
                        create_data,
                        max_hp: c.max_hp(),
                        level: c.level(),
                        min_dmg: c.min_dmg(),
                        max_dmg: c.max_dmg(),
                        aggro_radius: c.creature.ai_ownership().aggro_radius,
                        npc_flags: c.npc_flags(),
                        unit_flags: c.unit_flags(),
                        map_id,
                        loot_id: c.loot_id(),
                        gold_min: c.gold_min(),
                        gold_max: c.gold_max(),
                        boss_id: c.boss_id(),
                        dungeon_encounter_id: c.dungeon_encounter_id(),
                        phase_use_flags: c.creature.ai_ownership().phase_use_flags,
                        phase_id: c.creature.ai_ownership().phase_id,
                        phase_group_id: c.creature.ai_ownership().phase_group_id,
                        terrain_swap_map: c.creature.ai_ownership().terrain_swap_map,
                    });
                    tracing::info!(
                        "Corpse despawned: {:?} (entry {}) — respawn in {}s",
                        g,
                        c.entry(),
                        c.creature.ai_ownership().respawn_time_secs
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
            use wow_packet::ServerPacket;
            use wow_packet::packets::update::UpdateObject;

            let guid = r.create_data.guid;
            let entry = r.create_data.entry;
            tracing::info!(
                "Creature respawned: {:?} (entry {}) at {:?}",
                guid,
                entry,
                r.home_pos
            );

            // Send CREATE block to client.
            let block = UpdateObject::create_creature_block(r.create_data.clone(), &r.home_pos);
            let pkt = UpdateObject::create_creatures(vec![block], r.map_id);
            if let Err(e) = self.send_tx.send(pkt.to_bytes()) {
                tracing::warn!("Failed to send respawn packet: {e}");
            }

            // Recreate canonical map state.
            self.register_world_creature(
                r.map_id,
                r.home_pos,
                r.create_data.clone(),
                r.min_dmg,
                r.max_dmg,
                r.aggro_radius,
                r.loot_id,
                r.gold_min,
                r.gold_max,
                r.boss_id,
                r.dungeon_encounter_id,
                r.phase_use_flags,
                r.phase_id,
                r.phase_group_id,
                r.terrain_swap_map,
            );
            self.visible_creatures.insert(guid);
        }
        // ──────────────────────────────────────────────────────────────────

        let mmap_runtime_config = self.mmap_runtime_config_like_cpp.clone();
        let mmap_pathfinder = self.mmap_pathfinder_like_cpp.clone();
        for guid in guids {
            let _ = self.mutate_world_creature(guid, |creature| {
                if !creature.is_alive() {
                    if creature.should_respawn() {
                        creature.respawn();
                    }
                    return;
                }

                match creature.state() {
                    wow_entities::CreatureAiState::Idle => {
                        if creature.movement_finished() {
                            if creature.move_target().is_some() {
                                creature.finish_move();
                            }
                            if creature.should_wander() {
                                let dst = creature.pick_wander_destination();
                                let owner_ignores_pathfinding = creature
                                    .creature
                                    .unit()
                                    .has_unit_state(UnitState::IGNORE_PATHFINDING.bits());
                                let source_map_id = creature.map_id();
                                let source_instance_id = creature.instance_id();
                                let movement = if mmap_runtime_config
                                    .should_try_pathfinding_like_cpp(
                                        source_map_id,
                                        owner_ignores_pathfinding,
                                    ) {
                                    let detour_path = mmap_pathfinder.as_ref().and_then(|worker| {
                                        match worker.calculate_path_like_cpp(
                                            WorldMMapPathRequestLikeCpp {
                                                start: creature.position(),
                                                destination: dst,
                                                mesh_map_id: source_map_id,
                                                instance_map_id: source_map_id,
                                                instance_id: source_instance_id,
                                                filter_context: PathQueryFilterContext::creature(
                                                    true, false, false, false,
                                                ),
                                                force_destination: false,
                                                phase_shift: creature.phase_shift().clone(),
                                            },
                                        ) {
                                            Ok(path) => path,
                                            Err(error) => {
                                                tracing::warn!(
                                                    "mmap pathfinding failed for creature {:?}: {:?}",
                                                    guid,
                                                    error
                                                );
                                                None
                                            }
                                        }
                                    });
                                    creature
                                        .begin_move_spline_with_detour_path_like_cpp(
                                            dst,
                                            detour_path.as_ref(),
                                            false,
                                        )
                                        .map(|(from, spline, _path)| (from, spline))
                                } else {
                                    creature.begin_move_spline_like_cpp(dst)
                                };
                                if let Some((from, move_spline)) = movement {
                                    creature
                                        .creature
                                        .set_ai_state(wow_entities::CreatureAiState::WalkingRandom);
                                    creature.reset_wander_timer();
                                    let pkt = MonsterMove {
                                        mover_guid: guid,
                                        current_pos: from,
                                        spline: MovementMonsterSpline::from_move_spline(
                                            &move_spline,
                                        ),
                                    };
                                    to_send.push(pkt.to_bytes());
                                } else {
                                    creature.reset_wander_timer();
                                }
                            }
                        }
                    }
                    wow_entities::CreatureAiState::WalkingRandom => {
                        if creature.update_move_spline_like_cpp() || creature.movement_finished() {
                            creature.finish_move();
                            creature
                                .creature
                                .set_ai_state(wow_entities::CreatureAiState::Idle);
                            creature.reset_wander_timer();
                        }
                    }
                    wow_entities::CreatureAiState::Returning => {
                        if creature.movement_finished() {
                            creature.finish_move();
                            creature
                                .creature
                                .set_ai_state(wow_entities::CreatureAiState::Idle);
                        }
                    }
                    wow_entities::CreatureAiState::InCombat
                    | wow_entities::CreatureAiState::Dead
                    | wow_entities::CreatureAiState::WalkingWaypoint => {}
                }
            });
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
            if let Err(e) = self
                .execute_spell_with_visual(spell_id, target, cast_id, spell_visual)
                .await
            {
                warn!(account = self.account_id, "Spell execution failed: {}", e);
                // Send CastFailed so client cancels cast animation
                use wow_packet::ServerPacket;
                use wow_packet::packets::spell::CastFailed;
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
        use wow_packet::ServerPacket;
        use wow_packet::packets::combat::{AttackerStateUpdate, SAttackStop, VICTIM_STATE_HIT};
        use wow_packet::packets::movement::MonsterMoveStop;

        let Some(player_guid) = self.player_guid() else {
            return;
        };
        let Some(combat_target) = self
            .canonical_player_attack_target_like_cpp()
            .or(self.combat_target)
        else {
            self.combat_target = None;
            self.in_combat = false;
            return;
        };
        self.combat_target = Some(combat_target);
        let now = Instant::now();
        let diff_ms = now
            .saturating_duration_since(self.combat_tick_last_at_like_cpp)
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        self.combat_tick_last_at_like_cpp = now;
        // Check if target still exists. C++ `Unit::UpdateMeleeAttackingState`
        // is driven from `GetVictim()`; if the represented creature vanished,
        // clear the canonical player's attack state as well as session mirrors.
        let Some(target_position) =
            self.mutate_world_creature(combat_target, |creature| creature.position())
        else {
            let _ = self.mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().attack_stop_like_cpp()
            });
            self.combat_target = None;
            self.in_combat = false;
            return;
        };
        let player_position = self.player_position_like_cpp();
        let in_melee_range = player_position
            .map(|position| position.distance(&target_position) <= 5.0)
            .unwrap_or(true);
        let facing_target = player_position
            .map(|position| {
                Self::is_player_facing_target_for_melee_like_cpp(position, target_position)
            })
            .unwrap_or(true);
        let within_los = self.player_melee_los_to_target_like_cpp.unwrap_or(true);
        let canonical_swing_damages = self.take_canonical_player_attack_swings_like_cpp(
            diff_ms,
            in_melee_range,
            facing_target,
            within_los,
        );
        if self.canonical_map_manager.is_some() && canonical_swing_damages.is_none() {
            return;
        }

        // Gather combat data from the canonical map-owned creature before
        // emitting combat packets.
        let Some((swings, target_level, now_dead, move_stop)) = self
            .mutate_world_creature(combat_target, |creature| {
                if !creature.is_alive() {
                    return None;
                }
                if creature.state() != wow_entities::CreatureAiState::InCombat {
                    creature.enter_combat(player_guid);
                }
                let damages = match canonical_swing_damages.as_ref() {
                    Some(damages) => damages.clone(),
                    None => {
                        if !creature.can_swing() {
                            return None;
                        }
                        vec![creature.roll_damage().max(1)]
                    }
                };
                let level = creature.level();
                let mut sent_swings = Vec::new();
                let mut died = false;
                let mut move_stop = None;
                for dmg in damages {
                    if !creature.is_alive() {
                        break;
                    }
                    let damage = dmg.max(1);
                    let health_before = creature.current_hp();
                    died = creature.take_damage(damage);
                    let over_damage = if died {
                        damage.saturating_sub(health_before) as i32
                    } else {
                        -1
                    };
                    creature
                        .creature
                        .unit_mut()
                        .subsystems_mut()
                        .combat
                        .add_threat(player_guid, damage as f32);
                    sent_swings.push((damage, died, over_damage));
                    if died {
                        let combat = &mut creature.creature.unit_mut().subsystems_mut().combat;
                        combat.clear_threat();
                        combat.clear_attackers();
                        move_stop = creature.stop_move_spline_like_cpp().map(|stop| {
                            MonsterMoveStop {
                                mover_guid: combat_target,
                                current_pos: stop.position,
                                spline_id: stop.spline_id,
                            }
                            .to_bytes()
                        });
                        break;
                    }
                }
                if canonical_swing_damages.is_none() {
                    creature.record_swing();
                }
                Some((sent_swings, level, died, move_stop))
            })
            .flatten()
        else {
            return;
        };

        for (dmg, _swing_killed, over_damage) in &swings {
            let state_update = AttackerStateUpdate {
                attacker: player_guid,
                victim: combat_target,
                damage: *dmg as i32,
                over_damage: *over_damage,
                victim_state: VICTIM_STATE_HIT,
                school_mask: 1,
                target_level,
                expansion: 2,
            };
            let _ = self.send_tx.send(state_update.to_bytes());
        }

        // TODO: creature health VALUES update — format needs verification vs client
        // (temporarily disabled to prevent client crash from malformed packet)

        if now_dead {
            if let Some(bytes) = move_stop {
                let _ = self.send_tx.send(bytes);
            }
            let stop = SAttackStop {
                attacker: player_guid,
                victim: combat_target,
                now_dead: true,
            };
            let _ = self.send_tx.send(stop.to_bytes());
            let _ = self.mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().attack_stop_like_cpp()
            });
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
        use wow_packet::ServerPacket;
        use wow_packet::packets::update::{PlayerCombatStats, UpdateObject};

        let Some(guid) = self.player_guid() else {
            return;
        };
        let Some(registry) = &self.player_registry else {
            return;
        };
        let Some(pos) = self.player_position_like_cpp() else {
            return;
        };
        let map_id = self.player_map_id_like_cpp();
        let race = self.player_race_like_cpp();
        let class = self.player_class_like_cpp();
        let gender = self.player_gender_like_cpp();
        let level = self.player_level_like_cpp();

        // Build visible_items from this player's equipped inventory.
        let mut visible_items = [(0i32, 0u16, 0u16); 19];
        for (slot, item) in self.inventory_items_like_cpp() {
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
            race,
            class,
            gender,
            level,
            default_display_id(race, gender),
            &pos,
            map_id,
            0,     // zone_id (would need to track)
            false, // is_self: other players see this as a regular player, not ActivePlayer
            visible_items,
            empty_inv_slots,
            PlayerCombatStats::default(), // other players don't need detailed combat stats
            empty_skills,
            self.player_gold_like_cpp(),
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
            if broadcast_info.map_id != map_id {
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
                guid, broadcast_count, map_id
            );
        }
    }

    /// Broadcast DestroyObject to all players on the same map when this player disconnects.
    pub(crate) fn broadcast_destroy_player_to_others(&self) {
        use wow_packet::ServerPacket;
        use wow_packet::packets::update::UpdateObject;

        let Some(guid) = self.player_guid() else {
            return;
        };
        let Some(registry) = &self.player_registry else {
            return;
        };
        let map_id = self.player_map_id_like_cpp();

        let destroy = UpdateObject::destroy_objects(vec![guid], map_id);
        let bytes = destroy.to_bytes();

        let mut count = 0usize;
        for entry in registry.iter() {
            let (other_guid, info) = entry.pair();
            if *other_guid == guid {
                continue;
            }
            if info.map_id != map_id {
                continue;
            }
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
        use wow_packet::packets::update::{PlayerCombatStats, UpdateObject};

        let Some(guid) = self.player_guid() else {
            return;
        };
        let Some(registry) = &self.player_registry else {
            return;
        };
        let map_id = self.player_map_id_like_cpp();

        let empty_inv_slots = [ObjectGuid::EMPTY; 141];
        let empty_skills = Vec::new();
        let default_combat_stats = PlayerCombatStats::default();

        let mut player_count = 0;

        // Iterate through all players in the registry
        for entry in registry.iter() {
            let (other_guid, broadcast_info) = entry.pair();

            // Skip self and players on different maps
            if *other_guid == guid || broadcast_info.map_id != map_id {
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
                0,     // zone_id (unknown — would need separate tracking)
                false, // is_self: this is another player, not us
                broadcast_info.visible_items,
                empty_inv_slots,
                default_combat_stats,
                empty_skills.clone(),
                0,      // coinage (don't send other players' gold)
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
                player_count, map_id, guid
            );
        }
    }

    /// Check if any hostile creature should aggro the player based on proximity.
    /// Called from movement handlers (CMSG_MOVE_*).
    pub(crate) async fn check_creature_aggro(&mut self) {
        use wow_packet::ServerPacket;
        use wow_packet::packets::combat::AttackStart;

        if self.in_combat {
            return;
        }

        let player_pos = match self.player_position_like_cpp() {
            Some(p) => p,
            None => return,
        };
        let player_guid = match self.player_guid() {
            Some(g) => g,
            None => return,
        };

        let guids = self.world_creature_guids();
        let mut aggro_guid: Option<wow_core::ObjectGuid> = None;

        for guid in guids {
            let aggroed = self
                .mutate_world_creature(guid, |creature| {
                    if !creature.is_alive() || creature.creature.ai_ownership().aggro_radius <= 0.0
                    {
                        return false;
                    }
                    creature.try_aggro(player_guid, &player_pos)
                })
                .unwrap_or(false);
            if aggroed {
                aggro_guid = Some(guid);
                break;
            }
        }

        if let Some(guid) = aggro_guid {
            let start = AttackStart {
                attacker: guid,
                victim: player_guid,
            };
            let _ = self.send_tx.send(start.to_bytes());
            self.combat_target = Some(guid);
            self.in_combat = true;
        }
    }

    /// Execute a spell — apply effects, set cooldown, send SMSG_SPELL_GO.
    ///
    /// Called for instant-cast spells. Delegates to execute_spell_with_visual
    /// with default cast_id and visual.
    pub async fn execute_spell(
        &mut self,
        spell_id: i32,
        target_guid: ObjectGuid,
    ) -> Result<(), &'static str> {
        use wow_packet::packets::spell::SpellCastVisual;

        self.execute_spell_with_visual(
            spell_id,
            target_guid,
            ObjectGuid::EMPTY,
            SpellCastVisual {
                spell_visual_id: 0,
                script_visual_id: 0,
            },
        )
        .await
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
        let player_guid = self.player_guid().ok_or("No player GUID")?;

        // Obtener SpellInfo
        let spell_info = self
            .spell_store()
            .and_then(|store| store.get(spell_id))
            .ok_or("Spell not found")?;
        let mounted_aura_effect = spell_info
            .effects()
            .iter()
            .find(|effect| effect.is_mounted_aura_like_cpp())
            .cloned();
        let effect_type = spell_info.effect_type;
        let effect_base_points = spell_info.effect_base_points;

        info!(
            account = self.account_id,
            spell_id = spell_id,
            target = ?target_guid,
            effect_type = effect_type,
            "Executing spell effect"
        );

        // Send SMSG_SPELL_GO
        use wow_packet::ServerPacket;
        use wow_packet::packets::spell::{SpellGoPkt, SpellTargetData};

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
        match effect_type {
            6 => {
                // SPELL_EFFECT_HEAL
                let heal_amount = effect_base_points as u32;
                self.apply_heal(target_guid, heal_amount).await?;
            }
            2 => {
                // SPELL_EFFECT_SCHOOL_DAMAGE
                let damage_amount = effect_base_points as u32;
                self.apply_damage(target_guid, damage_amount).await?;
            }
            35 => {
                // SPELL_EFFECT_APPLY_AURA
                if let Some(effect) = mounted_aura_effect.as_ref() {
                    self.apply_represented_mounted_aura_like_cpp(spell_id, player_guid, effect)?;
                } else {
                    self.apply_aura(spell_id, player_guid, 30000, 0x00000001)?;
                }
            }
            _ => {
                debug!("Spell effect type {} not yet implemented", effect_type);
            }
        }

        // Set global cooldown
        self.last_spell_cast_time = Some(Instant::now());

        // Set per-spell cooldown
        self.last_spell_cast_time_per_spell
            .insert(spell_id, Instant::now());

        // Notify client so action bar shows the cooldown animation
        use wow_packet::packets::spell::CooldownEvent;
        self.send_packet(&CooldownEvent {
            spell_id,
            is_pet: false,
        });

        Ok(())
    }

    /// Helper: apply heal to target (self or creature).
    async fn apply_heal(
        &mut self,
        target_guid: ObjectGuid,
        heal_amount: u32,
    ) -> Result<(), &'static str> {
        let player_guid = self.player_guid().ok_or("No player GUID")?;

        // Si target es el mismo jugador
        if target_guid == player_guid {
            info!(account = self.account_id, heal = heal_amount, "Healed self");
            // TODO: Actualizar HP del jugador en la DB
            // self.player_health = min(self.player_health + heal_amount, self.player_max_health);
            // Enviar UpdateObject con VALUES update
            return Ok(());
        }

        // Si target es otra criatura/jugador
        if self.mutate_world_creature(target_guid, |_| ()).is_some() {
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
    async fn apply_damage(
        &mut self,
        target_guid: ObjectGuid,
        damage_amount: u32,
    ) -> Result<(), &'static str> {
        use wow_packet::ServerPacket;
        use wow_packet::packets::movement::MonsterMoveStop;

        let _player_guid = self.player_guid().ok_or("No player GUID")?;
        let account_id = self.account_id;

        // Si target es otra criatura — mutate canonical shared map state.
        let kill_info = self
            .mutate_world_creature(target_guid, |creature| {
                info!(
                    account = account_id,
                    creature = ?target_guid,
                    damage = damage_amount,
                    "Dealt damage to creature"
                );

                let died = creature.take_damage(damage_amount);
                if died {
                    info!(
                        "Creature {} (entry={}) killed",
                        target_guid,
                        creature.entry()
                    );
                    let move_stop = creature.stop_move_spline_like_cpp().map(|stop| {
                        MonsterMoveStop {
                            mover_guid: target_guid,
                            current_pos: stop.position,
                            spline_id: stop.spline_id,
                        }
                        .to_bytes()
                    });
                    Some((creature.entry(), target_guid, move_stop))
                } else {
                    None
                }
            })
            .ok_or("Target creature not found")?;

        // Process creature death outside the mutable borrow
        if let Some((entry, guid, move_stop)) = kill_info {
            if let Some(bytes) = move_stop {
                let _ = self.send_tx.send(bytes);
            }
            // Give XP for the kill
            let mob_level = self
                .mutate_world_creature(guid, |creature| creature.level())
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
        (
            1,
            &[
                (1, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Human
        (
            2,
            &[
                (1, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (6, 2, 0),
                (7, 0, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Orc
        (
            3,
            &[
                (1, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (7, 0, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Dwarf
        (
            4,
            &[
                (1, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (8, 0, 0),
                (11, 0, 0),
            ],
        ), // Night Elf
        (
            5,
            &[
                (1, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Undead
        (
            6,
            &[
                (1, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (7, 0, 0),
                (11, 0, 0),
            ],
        ), // Tauren
        (
            7,
            &[
                (1, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Gnome
        (
            8,
            &[
                (1, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (7, 0, 0),
                (8, 0, 0),
                (9, 0, 0),
                (11, 0, 0),
            ],
        ), // Troll
        (
            10,
            &[
                (1, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (4, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (8, 0, 0),
                (9, 0, 0),
            ],
        ), // Blood Elf
        (
            11,
            &[
                (1, 0, 0),
                (2, 0, 0),
                (3, 0, 0),
                (5, 0, 0),
                (6, 2, 0),
                (7, 0, 0),
                (8, 0, 0),
            ],
        ), // Draenei
    ];

    // MinActiveExpansionLevel per class = min across all races for that class
    // All classes have active=0 across all races except class 6 (DK) which is always 2
    let min_active = |class_id: u8| -> u8 { if class_id == 6 { 2 } else { 0 } };

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
        BagFamilyMask, EnchantmentSlot, InventoryResult, InventoryType, ItemBondingType, ItemClass,
        ItemContext, ItemFieldFlags, ItemFlags, ItemFlags2, ItemUpdateState, PhaseShiftFlags,
        ServerOpcodes, SpellItemEnchantmentFlags,
    };
    use wow_core::{Position, guid::HighGuid};
    use wow_data::{
        ImportPriceArmorEntry, ImportPriceArmorStore, ImportPriceQualityEntry,
        ImportPriceQualityStore, ImportPriceShieldEntry, ImportPriceShieldStore, ImportPriceStores,
        ImportPriceWeaponEntry, ImportPriceWeaponStore, ItemAppearanceEntry, ItemAppearanceStore,
        ItemClassEntry, ItemClassStore, ItemCurrencyCostEntry, ItemCurrencyCostStore,
        ItemDisenchantLootEntry, ItemDisenchantLootStore, ItemModifiedAppearanceEntry,
        ItemModifiedAppearanceStore, ItemPriceBaseEntry, ItemPriceBaseStore, ItemRandomSuffixEntry,
        ItemRandomSuffixStore, ItemRecord, ItemSparseTemplateEntry, ItemStatsStore, ItemStore,
        LockEntry, LockStore, SpellItemEnchantmentEntry, SpellItemEnchantmentStore,
    };
    use wow_entities::{
        AccessorObjectRef, BANK_SLOT_BAG_START, EQUIPMENT_SLOT_CHEST, INVENTORY_SLOT_BAG_START,
        REAGENT_BAG_SLOT_START, SendNewItemInstancePlan, SendNewItemModifier,
    };
    use wow_movement::MoveSplineFlag;
    use wow_network::{GroupInfo, PlayerBroadcastInfo};
    use wow_packet::ServerPacket;
    use wow_packet::packets::loot::{
        CreatureLoot, LOOT_TYPE_CORPSE_LIKE_CPP, LOOT_TYPE_ITEM_LIKE_CPP, LootEntry, LootEntryFlags,
    };

    fn make_session() -> (
        WorldSession,
        flume::Sender<WorldPacket>,
        flume::Receiver<Vec<u8>>,
    ) {
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

    fn drain_server_opcodes(rx: &flume::Receiver<Vec<u8>>) -> Vec<u16> {
        rx.try_iter()
            .filter_map(|bytes| {
                (bytes.len() >= 2).then(|| u16::from_le_bytes([bytes[0], bytes[1]]))
            })
            .collect()
    }

    #[test]
    fn represented_player_condition_context_uses_live_session_state_like_cpp() {
        let (mut session, _, _) = make_session();
        session.player_race = 1;
        session.player_class = 2;
        session.player_gender = 1;
        session.set_known_spells_like_cpp(vec![635, -1, 19740]);
        session.set_player_skill_values_like_cpp(HashMap::from([
            (SKILL_RIDING_LIKE_CPP, 75),
            (333, 125),
        ]));
        session.player_currencies.insert(
            81,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 25,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 0,
                flags: 0,
            },
        );
        session.player_quests.insert(
            100,
            crate::handlers::quest::PlayerQuestStatus {
                quest_id: 100,
                status: 1,
                explored: false,
                objective_counts: vec![],
            },
        );
        session.player_quests.insert(
            101,
            crate::handlers::quest::PlayerQuestStatus {
                quest_id: 101,
                status: 2,
                explored: false,
                objective_counts: vec![],
            },
        );
        session.rewarded_quests.insert(200);
        session.set_player_zone_area_like_cpp(12, 34);

        let owned = session.represented_player_condition_context_like_cpp();
        let context = owned.as_context(&session);

        assert_eq!(context.race, 1);
        assert_eq!(context.class_mask, 0b10);
        assert_eq!(context.gender, 1);
        assert_eq!(context.area_id, 34);
        assert_eq!(context.expansion, 2);
        assert_eq!(context.server_expansion, 9);
        assert_eq!(context.spells, &[635, 19740]);
        assert!(context.skills.contains(&PlayerConditionSkillLikeCpp {
            id: SKILL_RIDING_LIKE_CPP,
            value: 75,
        }));
        assert_eq!(
            session.player_skill_value_like_cpp(SKILL_RIDING_LIKE_CPP),
            75
        );
        assert_eq!(
            context.currencies,
            &[PlayerConditionCountLikeCpp { id: 81, count: 25 }]
        );
        assert!(context.current_quests.contains(&100));
        assert!(context.current_quests.contains(&101));
        assert_eq!(context.complete_quests, &[101]);
        assert_eq!(context.completed_quests, &[200]);
    }

    #[test]
    fn represented_player_condition_id_matches_cpp_lookup_semantics() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        assert!(!session.represented_meets_player_condition_id_like_cpp(42));

        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert!(session.represented_meets_player_condition_id_like_cpp(0));
        assert!(session.represented_meets_player_condition_id_like_cpp(42));
        assert!(!session.represented_meets_player_condition_id_like_cpp(43));
        assert!(session.represented_meets_player_condition_id_like_cpp(999));
    }

    #[test]
    fn represented_taxi_edge_distance_matches_cpp_condition_filter() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert_eq!(
            session.represented_taxi_edge_distance_like_cpp(true, 42, 1234),
            1234
        );
        assert_eq!(
            session.represented_taxi_edge_distance_like_cpp(true, 43, 1234),
            u16::MAX as u32
        );
        assert_eq!(
            session.represented_taxi_edge_distance_like_cpp(false, 42, 1234),
            u16::MAX as u32
        );
        assert_eq!(
            session.represented_taxi_edge_distance_like_cpp(true, 999, 1234),
            1234
        );
    }

    #[test]
    fn represented_mount_x_display_usable_matches_cpp_condition_filter() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert!(session.represented_mount_x_display_usable_like_cpp(0));
        assert!(session.represented_mount_x_display_usable_like_cpp(42));
        assert!(!session.represented_mount_x_display_usable_like_cpp(43));
        assert!(session.represented_mount_x_display_usable_like_cpp(999));
    }

    #[test]
    fn represented_taxi_usable_mount_displays_match_cpp_filter() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_known_spells_like_cpp(vec![100]);
        session.set_mount_store(Arc::new(wow_data::MountStore::from_entries([
            wow_data::MountEntry {
                id: 7,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 100,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
            wow_data::MountEntry {
                id: 8,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 101,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
        ])));
        session.set_mount_x_display_store(Arc::new(wow_data::MountXDisplayStore::from_entries([
            wow_data::MountXDisplayEntry {
                id: 1,
                creature_display_info_id: 1000,
                player_condition_id: 42,
                mount_id: 7,
            },
            wow_data::MountXDisplayEntry {
                id: 2,
                creature_display_info_id: 1001,
                player_condition_id: 43,
                mount_id: 7,
            },
            wow_data::MountXDisplayEntry {
                id: 3,
                creature_display_info_id: 1002,
                player_condition_id: 0,
                mount_id: 7,
            },
        ])));
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert_eq!(
            session.represented_taxi_usable_mount_displays_like_cpp(7),
            vec![1000, 1002]
        );
        assert!(
            session
                .represented_taxi_usable_mount_displays_like_cpp(8)
                .is_empty()
        );
        assert!(
            session
                .represented_taxi_usable_mount_displays_like_cpp(99)
                .is_empty()
        );
    }

    #[test]
    fn represented_mount_aura_display_candidates_match_cpp_filter() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_mount_store(Arc::new(wow_data::MountStore::from_entries([
            wow_data::MountEntry {
                id: 7,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 100,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
            wow_data::MountEntry {
                id: 8,
                mount_type_id: 0,
                flags: wow_data::MOUNT_FLAG_SELF_MOUNT,
                source_type_enum: 0,
                source_spell_id: 101,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
        ])));
        session.set_mount_x_display_store(Arc::new(wow_data::MountXDisplayStore::from_entries([
            wow_data::MountXDisplayEntry {
                id: 1,
                creature_display_info_id: 1000,
                player_condition_id: 42,
                mount_id: 7,
            },
            wow_data::MountXDisplayEntry {
                id: 2,
                creature_display_info_id: 1001,
                player_condition_id: 43,
                mount_id: 7,
            },
            wow_data::MountXDisplayEntry {
                id: 3,
                creature_display_info_id: 1002,
                player_condition_id: 0,
                mount_id: 7,
            },
        ])));
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert_eq!(
            session.represented_mount_aura_display_candidates_like_cpp(100),
            vec![1000, 1002]
        );
        assert_eq!(
            session.represented_mount_aura_display_candidates_like_cpp(101),
            vec![wow_data::DISPLAYID_HIDDEN_MOUNT]
        );
        assert!(
            session
                .represented_mount_aura_display_candidates_like_cpp(999)
                .is_empty()
        );
    }

    #[test]
    fn represented_mounted_aura_toggles_mount_flag_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let other_guid = ObjectGuid::create_player(1, 43);
        let pet_guid =
            ObjectGuid::create_world_object(wow_core::guid::HighGuid::Pet, 0, 1, 0, 0, 500, 44);
        let registry = Arc::new(PlayerRegistry::default());
        let (other_tx, other_rx) = flume::bounded(8);
        session.set_player_guid(Some(player_guid));
        session.set_player_registry(Arc::clone(&registry));
        session.set_player_position_like_cpp(Position::new(1.0, 2.0, 3.0, 0.5));
        session.set_represented_pet_mode_state_like_cpp(
            Some(pet_guid),
            wow_packet::packets::pet::REACT_DEFENSIVE_LIKE_CPP,
            wow_packet::packets::pet::COMMAND_STAY_LIKE_CPP,
        );
        session.player_race = 1;
        session.player_gender = 0;
        registry.insert(
            player_guid,
            broadcast_info(player_guid, flume::bounded(1).0),
        );
        registry.insert(other_guid, broadcast_info(other_guid, other_tx));
        session.set_creature_template_mount_store(Arc::new(
            wow_data::CreatureTemplateMountStoreLikeCpp::from_entries([
                wow_data::CreatureTemplateMountEntryLikeCpp {
                    entry: 1234,
                    vehicle_id: 55,
                    models: vec![wow_data::CreatureTemplateMountModelLikeCpp {
                        display_id: 4321,
                        display_scale: 1.0,
                        probability: 0.0,
                    }],
                },
            ]),
        ));
        let native_display_id = crate::handlers::character::default_display_id(
            session.player_race_like_cpp(),
            session.player_gender_like_cpp(),
        );
        session.set_creature_display_info_store(Arc::new(
            wow_data::CreatureDisplayInfoStore::from_entries([
                wow_data::CreatureDisplayInfoEntry {
                    id: native_display_id,
                    model_id: 100,
                    creature_model_scale: 1.2,
                },
                wow_data::CreatureDisplayInfoEntry {
                    id: 4321,
                    model_id: 200,
                    creature_model_scale: 1.5,
                },
            ]),
        ));
        session.set_creature_model_data_store(Arc::new(
            wow_data::CreatureModelDataStore::from_entries([
                wow_data::CreatureModelDataEntry {
                    id: 100,
                    collision_height: 2.0,
                    model_scale: 1.1,
                    mount_height: 0.0,
                },
                wow_data::CreatureModelDataEntry {
                    id: 200,
                    collision_height: 0.0,
                    model_scale: 1.0,
                    mount_height: 4.0,
                },
            ]),
        ));
        session.set_vehicle_store(Arc::new(wow_data::VehicleStore::from_entries([
            wow_data::VehicleEntry {
                id: 55,
                flags: 0,
                flags_b: 0,
                seat_ids: [1000, 1001, 0, 0, 0, 0, 0, 0],
            },
        ])));
        session.set_vehicle_seat_store(Arc::new(wow_data::VehicleSeatStore::from_entries([
            wow_data::VehicleSeatEntry {
                id: 1000,
                attachment_offset_x: 0.0,
                attachment_offset_y: 0.0,
                attachment_offset_z: 0.0,
                flags: wow_data::VEHICLE_SEAT_FLAG_CAN_ENTER_OR_EXIT,
                flags_b: 0,
                flags_c: 0,
            },
            wow_data::VehicleSeatEntry {
                id: 1001,
                attachment_offset_x: 0.0,
                attachment_offset_y: 0.0,
                attachment_offset_z: 0.0,
                flags: 0,
                flags_b: wow_data::VEHICLE_SEAT_FLAG_B_USABLE_FORCED,
                flags_c: 0,
            },
        ])));
        session.set_vehicle_template_store(Arc::new(
            wow_data::VehicleTemplateStoreLikeCpp::from_entries([(
                1234,
                wow_entities::VehicleTemplate {
                    despawn_delay_ms: 2500,
                },
            )]),
        ));
        session.set_vehicle_accessory_store(Arc::new(
            wow_data::VehicleAccessoryStoreLikeCpp::from_parts(
                std::iter::empty::<(u64, Vec<wow_entities::VehicleAccessory>)>(),
                [(
                    1234,
                    vec![
                        wow_entities::VehicleAccessory {
                            accessory_entry: 7001,
                            seat_id: 0,
                            is_minion: true,
                            summoned_type: 8,
                            summon_time_ms: 0,
                        },
                        wow_entities::VehicleAccessory {
                            accessory_entry: 7002,
                            seat_id: 1,
                            is_minion: false,
                            summoned_type: 6,
                            summon_time_ms: 5000,
                        },
                    ],
                )],
            ),
        ));
        let effect = wow_data::SpellEffectInfo {
            effect: wow_data::spell::spell_effect_types::SPELL_EFFECT_APPLY_AURA,
            effect_aura: wow_data::spell::aura_types::SPELL_AURA_MOUNTED,
            effect_base_points: 77,
            effect_misc_value_1: 1234,
            ..Default::default()
        };

        session
            .apply_represented_mounted_aura_like_cpp(100, ObjectGuid::EMPTY, &effect)
            .unwrap();

        assert_eq!(session.player_mount_display_id_like_cpp, 4321);
        assert_eq!(session.player_mount_vehicle_id_like_cpp, 55);
        assert_eq!(session.player_mount_vehicle_seat_count_like_cpp, 2);
        assert_eq!(session.player_mount_vehicle_usable_seat_count_like_cpp, 1);
        let vehicle_kit = session.player_mount_vehicle_kit_like_cpp.as_ref().unwrap();
        assert_eq!(vehicle_kit.vehicle_id(), 55);
        assert_eq!(vehicle_kit.creature_entry(), 1234);
        assert_eq!(vehicle_kit.status(), wow_entities::VehicleStatus::Installed);
        assert_eq!(vehicle_kit.seats().len(), 2);
        assert_eq!(session.player_mount_vehicle_accessories_like_cpp.len(), 2);
        assert_eq!(
            session.player_mount_vehicle_accessories_like_cpp[0].accessory_entry,
            7001
        );
        assert_eq!(
            session.player_mount_vehicle_despawn_delay_ms_like_cpp(),
            2500
        );
        assert!(session.player_mounted_like_cpp);
        assert_eq!(session.mount_vehicle_create_requests_like_cpp, 1);
        assert_eq!(session.mount_vehicle_remove_requests_like_cpp, 0);
        assert_eq!(
            session.mount_cancel_expected_vehicle_aura_packets_like_cpp,
            1
        );
        assert_eq!(session.mount_pet_control_disable_requests_like_cpp, 1);
        assert_eq!(session.mount_pet_control_enable_requests_like_cpp, 0);
        assert_eq!(session.mount_pet_resummon_requests_like_cpp, 0);
        assert_eq!(session.mount_collision_height_update_requests_like_cpp, 1);
        assert_eq!(
            session.represented_pet_react_state_like_cpp,
            wow_packet::packets::pet::REACT_PASSIVE_LIKE_CPP
        );
        assert_eq!(
            session.represented_pet_command_state_like_cpp,
            wow_packet::packets::pet::COMMAND_FOLLOW_LIKE_CPP
        );
        assert!((session.player_collision_height_like_cpp - 7.32).abs() < 0.0001);
        let opcodes = drain_server_opcodes(&send_rx);
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::MoveSetVehicleRecId as u16)));
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::SetVehicleRecId as u16)));
        assert!(
            opcodes
                .contains(&(wow_constants::ServerOpcodes::OnCancelExpectedRideVehicleAura as u16))
        );
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::PetMode as u16)));
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::MoveSetCollisionHeight as u16)));
        let broadcast = wow_packet::WorldPacket::from_bytes(&other_rx.try_recv().unwrap());
        assert_eq!(
            broadcast.server_opcode(),
            Some(wow_constants::ServerOpcodes::MoveUpdateCollisionHeight)
        );
        assert!(
            session
                .player_unit_flags_like_cpp
                .contains(UnitFlags::PLAYER_CONTROLLED | UnitFlags::MOUNT)
        );

        let slot = session
            .visible_auras
            .iter()
            .find_map(|(&slot, aura)| (aura.spell_id == 100).then_some(slot))
            .unwrap();
        session.remove_aura(slot).unwrap();

        assert_eq!(session.player_mount_display_id_like_cpp, 0);
        assert_eq!(session.player_mount_vehicle_id_like_cpp, 0);
        assert!(session.player_mount_vehicle_kit_like_cpp.is_none());
        assert!(session.player_mount_vehicle_accessories_like_cpp.is_empty());
        assert_eq!(session.player_mount_vehicle_despawn_delay_ms_like_cpp(), 1);
        assert_eq!(session.player_mount_vehicle_seat_count_like_cpp, 0);
        assert_eq!(session.player_mount_vehicle_usable_seat_count_like_cpp, 0);
        assert!(!session.player_mounted_like_cpp);
        assert_eq!(session.mount_vehicle_create_requests_like_cpp, 1);
        assert_eq!(session.mount_vehicle_remove_requests_like_cpp, 1);
        assert_eq!(session.mount_pet_control_disable_requests_like_cpp, 1);
        assert_eq!(session.mount_pet_control_enable_requests_like_cpp, 1);
        assert_eq!(session.mount_pet_resummon_requests_like_cpp, 1);
        assert_eq!(session.mount_collision_height_update_requests_like_cpp, 2);
        assert_eq!(
            session.represented_pet_react_state_like_cpp,
            wow_packet::packets::pet::REACT_DEFENSIVE_LIKE_CPP
        );
        assert_eq!(
            session.represented_pet_command_state_like_cpp,
            wow_packet::packets::pet::COMMAND_FOLLOW_LIKE_CPP
        );
        assert_eq!(session.temporary_mount_pet_react_state_like_cpp, None);
        assert!((session.player_collision_height_like_cpp - 2.64).abs() < 0.0001);
        let opcodes = drain_server_opcodes(&send_rx);
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::MoveSetVehicleRecId as u16)));
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::SetVehicleRecId as u16)));
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::PetMode as u16)));
        assert!(opcodes.contains(&(wow_constants::ServerOpcodes::MoveSetCollisionHeight as u16)));
        let broadcast = wow_packet::WorldPacket::from_bytes(&other_rx.try_recv().unwrap());
        assert_eq!(
            broadcast.server_opcode(),
            Some(wow_constants::ServerOpcodes::MoveUpdateCollisionHeight)
        );
        assert!(
            session
                .player_unit_flags_like_cpp
                .contains(UnitFlags::PLAYER_CONTROLLED)
        );
        assert!(
            !session
                .player_unit_flags_like_cpp
                .contains(UnitFlags::MOUNT)
        );
    }

    #[test]
    fn represented_mount_aura_keeps_creature_vehicle_with_mount_display_like_cpp() {
        let (mut session, _, _) = make_session();
        session.set_player_guid(Some(ObjectGuid::create_player(1, 12345)));
        session.set_mount_store(Arc::new(wow_data::MountStore::from_entries([
            wow_data::MountEntry {
                id: 7,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 100,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
        ])));
        session.set_mount_x_display_store(Arc::new(wow_data::MountXDisplayStore::from_entries([
            wow_data::MountXDisplayEntry {
                id: 1,
                creature_display_info_id: 1000,
                player_condition_id: 0,
                mount_id: 7,
            },
        ])));
        session.set_creature_template_mount_store(Arc::new(
            wow_data::CreatureTemplateMountStoreLikeCpp::from_entries([
                wow_data::CreatureTemplateMountEntryLikeCpp {
                    entry: 1234,
                    vehicle_id: 55,
                    models: vec![wow_data::CreatureTemplateMountModelLikeCpp {
                        display_id: 4321,
                        display_scale: 1.0,
                        probability: 0.0,
                    }],
                },
            ]),
        ));
        let effect = wow_data::SpellEffectInfo {
            effect: wow_data::spell::spell_effect_types::SPELL_EFFECT_APPLY_AURA,
            effect_aura: wow_data::spell::aura_types::SPELL_AURA_MOUNTED,
            effect_base_points: 77,
            effect_misc_value_1: 1234,
            ..Default::default()
        };

        session
            .apply_represented_mounted_aura_like_cpp(100, ObjectGuid::EMPTY, &effect)
            .unwrap();

        assert_eq!(session.player_mount_display_id_like_cpp, 1000);
        assert_eq!(session.player_mount_vehicle_id_like_cpp, 55);
        assert!(session.player_mounted_like_cpp);
        assert!(
            session
                .player_unit_flags_like_cpp
                .contains(UnitFlags::PLAYER_CONTROLLED | UnitFlags::MOUNT)
        );
        assert_eq!(session.mount_vehicle_create_requests_like_cpp, 1);
    }

    #[test]
    fn represented_mount_source_spell_usable_matches_cpp_mount_condition_filter() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_mount_store(Arc::new(wow_data::MountStore::from_entries([
            wow_data::MountEntry {
                id: 1,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 100,
                player_condition_id: 42,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
            wow_data::MountEntry {
                id: 2,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 101,
                player_condition_id: 43,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
        ])));
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));

        assert!(session.represented_mount_source_spell_usable_like_cpp(100));
        assert!(!session.represented_mount_source_spell_usable_like_cpp(101));
        assert!(!session.represented_mount_source_spell_usable_like_cpp(999));
    }

    #[test]
    fn represented_mount_capability_for_type_uses_session_state_like_cpp() {
        let (mut session, _, _) = make_session();
        session.current_map_id = 1;
        session.set_player_zone_area_like_cpp(10, 77);
        session.set_known_spells_like_cpp(vec![456]);
        session.set_player_skill_values_like_cpp(HashMap::from([(SKILL_RIDING_LIKE_CPP, 75)]));
        session
            .apply_aura(123, ObjectGuid::EMPTY, 30_000, 0)
            .unwrap();
        session.set_area_table_store(Arc::new(wow_data::AreaTableStore::from_entries([
            wow_data::AreaTableEntry {
                id: 10,
                parent_area_id: 0,
                mount_flags: i32::from(wow_data::AREA_MOUNT_FLAG_ALLOW_GROUND_MOUNTS),
                flags: 0,
            },
            wow_data::AreaTableEntry {
                id: 77,
                parent_area_id: 10,
                mount_flags: i32::from(wow_data::AREA_MOUNT_FLAG_ALLOW_GROUND_MOUNTS),
                flags: 0,
            },
        ])));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: 0,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
            wow_data::MapEntry {
                id: 1,
                instance_type: 0,
                parent_map_id: 0,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.set_mount_capability_store(Arc::new(wow_data::MountCapabilityStore::from_entries(
            [
                wow_data::MountCapabilityEntry {
                    id: 10,
                    flags: wow_data::MOUNT_CAPABILITY_FLAG_FLYING,
                    req_riding_skill: 0,
                    req_area_id: 0,
                    req_spell_aura_id: 0,
                    req_spell_known_id: 0,
                    mod_spell_aura_id: 1000,
                    req_map_id: -1,
                },
                wow_data::MountCapabilityEntry {
                    id: 11,
                    flags: wow_data::MOUNT_CAPABILITY_FLAG_GROUND,
                    req_riding_skill: 75,
                    req_area_id: 10,
                    req_spell_aura_id: 123,
                    req_spell_known_id: 456,
                    mod_spell_aura_id: 1001,
                    req_map_id: 0,
                },
            ],
        )));
        session.set_mount_type_x_capability_store(Arc::new(
            wow_data::MountTypeXCapabilityStore::from_entries([
                wow_data::MountTypeXCapabilityEntry {
                    id: 1,
                    mount_type_id: 7,
                    mount_capability_id: 10,
                    order_index: 0,
                },
                wow_data::MountTypeXCapabilityEntry {
                    id: 2,
                    mount_type_id: 7,
                    mount_capability_id: 11,
                    order_index: 1,
                },
            ]),
        ));

        assert_eq!(
            session
                .represented_mount_capability_for_type_from_session_like_cpp(7, None)
                .map(|capability| capability.id),
            Some(11)
        );
        session.set_player_skill_values_like_cpp(HashMap::from([(SKILL_RIDING_LIKE_CPP, 74)]));
        assert!(
            session
                .represented_mount_capability_for_type_from_session_like_cpp(7, None)
                .is_none()
        );
    }

    #[test]
    fn represented_mount_liquid_state_uses_cpp_liquid_bits_and_swimming_flag() {
        let (mut session, _, _) = make_session();

        assert_eq!(
            session.represented_player_mount_liquid_state_like_cpp(),
            (false, false)
        );

        session.set_player_liquid_status_like_cpp(LIQUID_MAP_IN_WATER_LIKE_CPP);
        assert_eq!(
            session.represented_player_mount_liquid_state_like_cpp(),
            (false, true)
        );

        session.set_player_liquid_status_like_cpp(LIQUID_MAP_UNDER_WATER_LIKE_CPP);
        assert_eq!(
            session.represented_player_mount_liquid_state_like_cpp(),
            (true, true)
        );

        session.set_player_liquid_status_like_cpp(0);
        session.set_player_movement_flags_like_cpp(MovementFlag::SWIMMING);
        assert_eq!(
            session.represented_player_mount_liquid_state_like_cpp(),
            (true, false)
        );
    }

    #[test]
    fn represented_failed_map_difficulty_x_condition_matches_cpp_first_failed_order() {
        let (mut session, _, _) = make_session();
        session.player_class = 1;
        session.set_player_condition_store(Arc::new(wow_data::PlayerConditionStore::from_entries(
            [
                wow_data::PlayerConditionEntry {
                    id: 42,
                    class_mask: 1,
                    ..Default::default()
                },
                wow_data::PlayerConditionEntry {
                    id: 43,
                    class_mask: 1 << 1,
                    ..Default::default()
                },
            ],
        )));
        session.set_map_difficulty_x_condition_store(Arc::new(
            wow_data::MapDifficultyXConditionStore::from_entries([
                wow_data::MapDifficultyXConditionEntry {
                    id: 100,
                    failure_description: String::new(),
                    player_condition_id: 43,
                    order_index: 20,
                    map_difficulty_id: 7,
                },
                wow_data::MapDifficultyXConditionEntry {
                    id: 101,
                    failure_description: String::new(),
                    player_condition_id: 42,
                    order_index: 10,
                    map_difficulty_id: 7,
                },
            ]),
        ));

        assert_eq!(
            session.represented_failed_map_difficulty_x_condition_like_cpp(7),
            Some(100)
        );
        assert_eq!(
            session.represented_failed_map_difficulty_x_condition_like_cpp(8),
            None
        );
    }

    #[test]
    fn mmap_runtime_config_matches_cpp_pathfinding_gate() {
        let mut config = MMapRuntimeConfigLikeCpp::default();
        config.disabled_map_ids.insert(571);

        assert!(config.should_try_pathfinding_like_cpp(0, false));
        assert!(!config.should_try_pathfinding_like_cpp(571, false));
        assert!(!config.should_try_pathfinding_like_cpp(0, true));

        config.enabled = false;
        assert!(!config.should_try_pathfinding_like_cpp(0, false));
    }

    #[test]
    fn time_sync_response_sets_initial_clock_delta_like_cpp() {
        let (mut session, _pkt_tx, _send_rx) = make_session();
        let sent_time = WorldSession::game_time_ms_like_cpp().wrapping_sub(20);
        session.time_sync_pending_requests.insert(7, sent_time);

        session.record_time_sync_response_like_cpp(7, sent_time.wrapping_sub(1_000));

        assert!(session.time_sync_pending_requests.is_empty());
        assert_eq!(session.time_sync_clock_delta_queue.len(), 1);
        assert!(
            session.time_sync_clock_delta >= 1_000,
            "expected initial fallback delta from first sample"
        );
    }

    #[test]
    fn adjust_client_movement_time_uses_clock_delta_or_cpp_fallback() {
        let (mut session, _pkt_tx, _send_rx) = make_session();
        session.time_sync_clock_delta = 250;
        assert_eq!(session.adjust_client_movement_time_like_cpp(1_000), 1_250);

        session.time_sync_clock_delta = 0;
        let adjusted = session.adjust_client_movement_time_like_cpp(1_000);
        assert_ne!(adjusted, 1_000);
    }

    #[test]
    fn send_time_sync_uses_cpp_timer_sequence() {
        let (mut session, _pkt_tx, _send_rx) = make_session();

        session.send_time_sync();
        assert_eq!(session.time_sync_next_counter, 1);
        assert_eq!(session.time_sync_timer_ms, 5_000);
        assert!(session.time_sync_pending_requests.contains_key(&0));

        session.send_time_sync();
        assert_eq!(session.time_sync_next_counter, 2);
        assert_eq!(session.time_sync_timer_ms, 10_000);
        assert!(session.time_sync_pending_requests.contains_key(&1));
    }

    fn shared_map_manager() -> crate::map_manager::SharedMapManager {
        Arc::new(std::sync::RwLock::new(crate::map_manager::MapManager::new()))
    }

    fn shared_canonical_map_manager() -> SharedCanonicalMapManager {
        Arc::new(Mutex::new(wow_map::MapManager::default()))
    }

    fn test_creature_guid(counter: i64) -> ObjectGuid {
        ObjectGuid::create_world_object(wow_core::guid::HighGuid::Creature, 0, 1, 0, 0, 1, counter)
    }

    #[test]
    fn canonical_world_map_login_binding_uses_cpp_split_faction_instance() {
        let (mut session, _pkt_tx, _send_rx) = make_session();
        let canonical = shared_canonical_map_manager();
        let guid = ObjectGuid::create_player(1, 42);

        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 609,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: 571,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            guid,
            "Orc".to_string(),
            Position::new(1.0, 2.0, 3.0, 0.0),
            609,
            2,
            1,
            10,
            0,
        ));

        let decision = session
            .ensure_canonical_world_map_for_current_player_like_cpp()
            .expect("world map decision");

        assert_eq!(
            decision,
            wow_map::CreateMapDecision::Create {
                key: wow_map::MapKey::new(609, 1),
                difficulty_id: 0,
                kind: wow_map::ManagedMapKind::World,
                side_effects: Vec::new(),
            }
        );
        assert!(canonical.lock().unwrap().find_map(609, 1).is_some());
    }

    #[test]
    fn canonical_world_map_login_binding_skips_dungeons_until_runtime_fields_exist() {
        let (mut session, _pkt_tx, _send_rx) = make_session();
        let canonical = shared_canonical_map_manager();
        let guid = ObjectGuid::create_player(1, 42);

        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 33,
                instance_type: wow_data::map::MAP_INSTANCE,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            guid,
            "Player".to_string(),
            Position::new(1.0, 2.0, 3.0, 0.0),
            33,
            1,
            1,
            10,
            0,
        ));

        assert!(
            session
                .ensure_canonical_world_map_for_current_player_like_cpp()
                .is_none()
        );
        assert!(canonical.lock().unwrap().find_map(33, 0).is_none());
    }

    #[test]
    fn session_player_controller_tracks_cpp_attached_player_identity() {
        let (mut session, _pkt_tx, _send_rx) = make_session();
        let guid = ObjectGuid::create_player(1, 42);
        let start = Position::new(1.0, 2.0, 3.0, 4.0);
        session.set_player_gold_like_cpp(1234);
        session.set_player_xp_like_cpp(55);
        session.set_player_next_level_xp_like_cpp(4000);
        session.set_selection_guid_like_cpp(Some(test_creature_guid(77)));
        session.set_known_spells_like_cpp(vec![118, 133]);
        session.player_currencies.insert(
            395,
            PlayerCurrency {
                state: PlayerCurrencyState::Unchanged,
                quantity: 9,
                weekly_quantity: 0,
                tracked_quantity: 0,
                increased_cap_quantity: 0,
                earned_quantity: 9,
                flags: 0,
            },
        );
        let item_guid = ObjectGuid::create_item(1, 500);
        session.inventory_items.insert(
            23,
            InventoryItem {
                guid: item_guid,
                entry_id: 700,
                db_guid: 500,
                inventory_type: None,
            },
        );
        let item_object =
            session.make_inventory_item_object(item_guid, 700, guid, 2, 0, ItemContext::None, 23);
        session
            .inventory_item_objects
            .insert(item_guid, item_object);

        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            guid,
            "Jaina".to_string(),
            start,
            571,
            1,
            8,
            70,
            0,
        ));

        assert_eq!(session.player_guid(), Some(guid));
        assert_eq!(session.player_name_like_cpp(), Some("Jaina"));
        assert_eq!(session.player_position_like_cpp(), Some(start));
        assert_eq!(session.player_map_id_like_cpp(), 571);
        assert_eq!(session.player_race_like_cpp(), 1);
        assert_eq!(session.player_class_like_cpp(), 8);
        assert_eq!(session.player_level_like_cpp(), 70);
        assert_eq!(session.player_gender_like_cpp(), 0);
        assert_eq!(session.player_gold_like_cpp(), 1234);
        assert_eq!(session.player_xp_like_cpp(), 55);
        assert_eq!(session.player_next_level_xp_like_cpp(), 4000);
        assert_eq!(
            session.selection_guid_like_cpp(),
            Some(test_creature_guid(77))
        );
        assert_eq!(session.known_spells_like_cpp(), &[118, 133]);
        assert_eq!(session.player_currency_quantity(395), 9);
        assert_eq!(session.inventory_items_like_cpp()[&23].guid, item_guid);
        assert_eq!(
            session.inventory_item_objects_like_cpp()[&item_guid].count(),
            2
        );
        assert_eq!(session.player_guid, Some(guid));
        assert_eq!(session.player_name.as_deref(), Some("Jaina"));
        assert_eq!(session.player_position, Some(start));
        assert_eq!(session.current_map_id, 571);

        let moved = Position::new(5.0, 6.0, 7.0, 8.0);
        session.set_player_map_position_like_cpp(1, moved);
        session.set_player_level_like_cpp(71);
        session.set_player_gold_like_cpp(2000);
        session.set_player_xp_like_cpp(66);
        session.learn_known_spell_like_cpp(116);
        session.inventory_items.remove(&23);
        assert!(session.inventory_items_like_cpp().contains_key(&23));
        session.remove_inventory_item_like_cpp(23);

        assert_eq!(session.player_position_like_cpp(), Some(moved));
        assert_eq!(session.player_map_id_like_cpp(), 1);
        assert_eq!(session.player_level_like_cpp(), 71);
        assert_eq!(session.player_gold_like_cpp(), 2000);
        assert_eq!(session.player_xp_like_cpp(), 66);
        assert!(session.known_spells_like_cpp().contains(&116));
        assert!(!session.inventory_items_like_cpp().contains_key(&23));
        assert_eq!(session.player_position, Some(moved));
        assert_eq!(session.current_map_id, 1);
        assert_eq!(session.player_level, 71);

        session.set_player_guid(None);
        assert_eq!(session.player_guid(), None);
        assert!(session.player_controller.is_none());
    }

    fn test_creature_create_data(
        guid: ObjectGuid,
        entry: u32,
        hp: u32,
    ) -> wow_packet::packets::update::CreatureCreateData {
        wow_packet::packets::update::CreatureCreateData {
            guid,
            entry,
            display_id: 100,
            native_display_id: 100,
            health: hp as i64,
            max_health: hp as i64,
            level: 2,
            faction_template: 14,
            npc_flags: 0,
            unit_flags: 0,
            unit_flags2: 0,
            unit_flags3: 0,
            scale: 1.0,
            unit_class: 1,
            base_attack_time: 2000,
            ranged_attack_time: 0,
            zone_id: 0,
            speed_walk_rate: 1.0,
            speed_run_rate: 1.14286,
        }
    }

    fn register_test_creature(
        session: &mut WorldSession,
        manager: crate::map_manager::SharedMapManager,
        guid: ObjectGuid,
        hp: u32,
    ) {
        session.set_map_manager(manager);
        session.current_map_id = 0;
        session.register_world_creature(
            0,
            Position::new(10.0, 10.0, 0.0, 0.0),
            test_creature_create_data(guid, 9001, hp),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );
    }

    #[test]
    fn register_world_creature_mirrors_existing_canonical_map_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(611);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.current_map_id = 571;
        session.register_world_creature(
            571,
            Position::new(10.0, 20.0, 30.0, 1.0),
            test_creature_create_data(guid, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );

        let guard = canonical.lock().unwrap();
        let creature = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_creature(guid)
            .expect("creature inserted into canonical map");
        assert_eq!(creature.guid(), guid);
        assert_eq!(creature.object().entry(), 9001);
        assert_eq!(creature.position(), Position::new(10.0, 20.0, 30.0, 1.0));
        assert!(creature.object().is_in_world());
        let typed = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_creature(guid)
            .expect("creature stored as typed Creature entity");
        assert_eq!(typed.unit().world().object().entry(), 9001);
        assert_eq!(typed.current_health(), 25);
    }

    #[test]
    fn mutate_world_creature_relocates_canonical_map_object_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(612);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.current_map_id = 571;
        session.register_world_creature(
            571,
            Position::new(10.0, 20.0, 30.0, 1.0),
            test_creature_create_data(guid, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );

        let moved = Position::new(15.0, 25.0, 35.0, 2.0);
        session.mutate_world_creature(guid, |creature| {
            creature.creature.set_ai_position(moved);
        });

        let guard = canonical.lock().unwrap();
        let creature = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_creature(guid)
            .expect("creature remains in canonical map");
        assert_eq!(creature.position(), moved);
        let typed = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_creature(guid)
            .expect("creature remains a typed Creature entity");
        assert_eq!(typed.position(), moved);
    }

    #[test]
    fn represented_gameobject_runtime_state_uses_typed_canonical_map_object_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let guid = ObjectGuid::create_world_object(HighGuid::GameObject, 0, 1, 571, 0, 9001, 44);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.record_represented_gameobject_runtime_state_like_cpp(
            571,
            guid,
            9001,
            Position::new(10.0, 20.0, 30.0, 1.0),
            wow_entities::GAMEOBJECT_TYPE_CHEST as u8,
        );

        let guard = canonical.lock().unwrap();
        let map = guard.find_map(571, 0).unwrap().map();
        assert_eq!(map.get_game_object(guid).unwrap().guid(), guid);
        let typed = map
            .get_typed_game_object(guid)
            .expect("gameobject stored as typed GameObject entity");
        assert_eq!(typed.world().object().entry(), 9001);
        assert_eq!(
            typed.world().position(),
            Position::new(10.0, 20.0, 30.0, 1.0)
        );
    }

    #[test]
    fn remove_world_creature_removes_canonical_map_object_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(613);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.current_map_id = 571;
        session.register_world_creature(
            571,
            Position::new(10.0, 20.0, 30.0, 1.0),
            test_creature_create_data(guid, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );

        assert!(session.remove_world_creature(guid).is_some());
        let guard = canonical.lock().unwrap();
        assert!(
            guard
                .find_map(571, 0)
                .unwrap()
                .map()
                .get_creature(guid)
                .is_none()
        );
    }

    #[test]
    fn register_world_creature_applies_valid_terrain_swap_visible_map_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(609);
        let map_store = Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: 0,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
            wow_data::MapEntry {
                id: 609,
                instance_type: 0,
                parent_map_id: 571,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ]));
        let terrain_swap_store = Arc::new(wow_data::TerrainSwapStore::from_rows_like_cpp(
            &map_store,
            [],
            [],
            |_| true,
        ));

        session.set_map_manager(Arc::clone(&manager));
        session.set_map_store(map_store);
        session.set_terrain_swap_store(terrain_swap_store);
        session.register_world_creature(
            571,
            Position::new(10.0, 10.0, 0.0, 0.0),
            test_creature_create_data(guid, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            609,
        );

        let guard = manager
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let creature = guard.find_creature(571, 0, guid).unwrap();
        assert!(creature.phase_shift().has_visible_map_id_like_cpp(609));
        assert_eq!(creature.creature.ai_ownership().terrain_swap_map, 609);
    }

    #[test]
    fn register_world_creature_applies_db_phase_shift_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(710);
        let phase_store = Arc::new(wow_data::PhaseStore::from_entries([
            wow_data::PhaseEntry { id: 10, flags: 0 },
            wow_data::PhaseEntry { id: 20, flags: 0 },
        ]));
        let phase_group_store = Arc::new(wow_data::PhaseGroupStore::from_entries(
            &phase_store,
            [wow_data::PhaseXPhaseGroupEntry {
                id: 1,
                phase_id: 20,
                phase_group_id: 7,
            }],
        ));

        session.set_map_manager(Arc::clone(&manager));
        session.set_phase_store(phase_store);
        session.set_phase_group_store(phase_group_store);
        session.register_world_creature(
            571,
            Position::new(10.0, 10.0, 0.0, 0.0),
            test_creature_create_data(guid, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            crate::phasing::PHASE_USE_FLAGS_INVERSE,
            0,
            7,
            -1,
        );

        let guard = manager
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let creature = guard.find_creature(571, 0, guid).unwrap();
        assert!(creature.phase_shift().is_db_phase_shift_like_cpp());
        assert!(creature.phase_shift().has_phase_like_cpp(20));
        assert_eq!(
            creature.phase_shift().flags_like_cpp(),
            PhaseShiftFlags::INVERSE
        );
        assert_eq!(
            creature.creature.ai_ownership().phase_use_flags,
            crate::phasing::PHASE_USE_FLAGS_INVERSE
        );
        assert_eq!(creature.creature.ai_ownership().phase_group_id, 7);
    }

    #[test]
    fn represented_gameobject_phase_shift_applies_db_phase_and_visible_map_like_cpp() {
        let (mut session, _, _) = make_session();
        let guid = ObjectGuid::create_world_object(HighGuid::GameObject, 0, 1, 571, 0, 1, 900);
        let phase_store = Arc::new(wow_data::PhaseStore::from_entries([wow_data::PhaseEntry {
            id: 20,
            flags: 0,
        }]));
        let phase_group_store = Arc::new(wow_data::PhaseGroupStore::from_entries(
            &phase_store,
            [wow_data::PhaseXPhaseGroupEntry {
                id: 1,
                phase_id: 20,
                phase_group_id: 7,
            }],
        ));
        let map_store = Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: 0,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
            wow_data::MapEntry {
                id: 609,
                instance_type: 0,
                parent_map_id: 571,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ]));
        let terrain_swap_store = Arc::new(wow_data::TerrainSwapStore::from_rows_like_cpp(
            &map_store,
            [],
            [],
            |_| true,
        ));

        session.set_phase_store(phase_store);
        session.set_phase_group_store(phase_group_store);
        session.set_map_store(map_store);
        session.set_terrain_swap_store(terrain_swap_store);
        session.record_represented_gameobject_db_phase_shift_like_cpp(
            guid,
            571,
            crate::phasing::PHASE_USE_FLAGS_INVERSE,
            0,
            7,
            609,
        );

        let phase_shift = session
            .represented_gameobject_phase_shifts
            .get(&guid)
            .unwrap();
        assert!(phase_shift.is_db_phase_shift_like_cpp());
        assert!(phase_shift.has_phase_like_cpp(20));
        assert!(phase_shift.has_visible_map_id_like_cpp(609));
        assert_eq!(phase_shift.flags_like_cpp(), PhaseShiftFlags::INVERSE);
    }

    #[test]
    fn session_db_spawn_phase_visibility_uses_player_phase_can_see_like_cpp() {
        let (mut session, _, _) = make_session();
        let phase_store = Arc::new(wow_data::PhaseStore::from_entries([wow_data::PhaseEntry {
            id: 20,
            flags: 0,
        }]));
        let phase_group_store = Arc::new(wow_data::PhaseGroupStore::from_entries(
            &phase_store,
            [wow_data::PhaseXPhaseGroupEntry {
                id: 1,
                phase_id: 20,
                phase_group_id: 7,
            }],
        ));

        session.set_phase_store(Arc::clone(&phase_store));
        session.set_phase_group_store(Arc::clone(&phase_group_store));

        let (target_phase_shift, _) = session.db_spawn_phase_shift_like_cpp(571, 0, 20, 0, -1);
        assert!(!session.can_see_phase_shift_like_cpp(&target_phase_shift));

        let mut player_phase_shift = PhaseShift::default();
        init_db_phase_shift_like_cpp(
            &mut player_phase_shift,
            &phase_store,
            &phase_group_store,
            0,
            20,
            0,
        );
        session.set_represented_player_phase_shift_like_cpp(player_phase_shift);
        assert!(session.can_see_phase_shift_like_cpp(&target_phase_shift));

        let (always_visible_shift, _) = session.db_spawn_phase_shift_like_cpp(
            571,
            crate::phasing::PHASE_USE_FLAGS_ALWAYS_VISIBLE,
            20,
            0,
            -1,
        );
        session.set_represented_player_phase_shift_like_cpp(PhaseShift::default());
        assert!(session.can_see_phase_shift_like_cpp(&always_visible_shift));
    }

    #[test]
    fn tick_creatures_sync_launches_real_move_spline_for_represented_wander() {
        let (mut session, _, send_rx) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(77);
        register_test_creature(&mut session, manager, guid, 25);
        session
            .mutate_world_creature(guid, |creature| {
                let ai = creature.creature.ai_ownership_mut();
                ai.wander_delay_ms = 0;
                ai.move_start_ms = 0;
                ai.wander_radius = 3.0;
            })
            .unwrap();

        session.tick_creatures_sync();

        let sent = send_rx.try_recv().unwrap();
        let opcode = u16::from_le_bytes([sent[0], sent[1]]);
        assert_eq!(opcode, ServerOpcodes::OnMonsterMove as u16);
        let mut pkt = WorldPacket::from_bytes(&sent[2..]);
        assert_eq!(pkt.read_packed_guid().unwrap(), guid);
        assert_eq!(pkt.read_float().unwrap(), 10.0);
        assert_eq!(pkt.read_float().unwrap(), 10.0);
        assert_eq!(pkt.read_float().unwrap(), 0.0);
        assert_eq!(pkt.read_uint32().unwrap(), 2);
        let destination = Position::new(
            pkt.read_float().unwrap(),
            pkt.read_float().unwrap(),
            pkt.read_float().unwrap(),
            0.0,
        );
        assert!(!pkt.has_bit().unwrap());
        assert_eq!(pkt.read_bits(3).unwrap(), 0);
        assert_eq!(
            pkt.read_uint32().unwrap(),
            MoveSplineFlag::SMOOTH_GROUND_PATH.bits()
        );
        assert_eq!(pkt.read_int32().unwrap(), 0);
        let move_time = pkt.read_uint32().unwrap();
        assert!(move_time > 0);
        assert_eq!(pkt.read_uint32().unwrap(), 0);
        assert_eq!(pkt.read_uint8().unwrap(), 0);
        assert_eq!(pkt.read_packed_guid().unwrap(), ObjectGuid::EMPTY);
        assert_eq!(pkt.read_int8().unwrap(), -1);
        assert_eq!(pkt.read_bits(2).unwrap(), 0);
        assert_eq!(pkt.read_bits(16).unwrap(), 1);
        assert!(!pkt.has_bit().unwrap());
        assert!(!pkt.has_bit().unwrap());
        assert_eq!(pkt.read_bits(16).unwrap(), 0);
        assert!(!pkt.has_bit().unwrap());
        assert!(!pkt.has_bit().unwrap());
        assert!(!pkt.has_bit().unwrap());
        assert!(!pkt.has_bit().unwrap());
        assert_eq!(pkt.read_float().unwrap(), destination.x);
        assert_eq!(pkt.read_float().unwrap(), destination.y);
        assert_eq!(pkt.read_float().unwrap(), destination.z);
        assert!(pkt.is_empty());
        assert!(send_rx.try_recv().is_err());

        session
            .mutate_world_creature(guid, |creature| {
                let motion_spline = &creature.creature.unit().subsystems().motion.spline;
                assert!(motion_spline.enabled);
                assert!(!motion_spline.finalized);
                assert_eq!(motion_spline.spline_id, 2);
                assert_eq!(motion_spline.duration_ms, move_time);
                assert_eq!(
                    motion_spline.final_destination,
                    Some((
                        destination.x as i32,
                        destination.y as i32,
                        destination.z as i32
                    ))
                );
                assert_eq!(
                    creature.state(),
                    wow_entities::CreatureAiState::WalkingRandom
                );
            })
            .unwrap();
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
            random_select: 0,
            random_suffix_group_id: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            entry,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                start_quest_id: 0,
                stackable: max_stack_size,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 0,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
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
        let (command_tx, _command_rx) = flume::bounded(1);
        PlayerBroadcastInfo {
            map_id: 0,
            position: Position::new(0.0, 0.0, 0.0, 0.0),
            send_tx,
            command_tx,
            active_loot_rolls: Vec::new(),
            pass_on_group_loot: false,
            enchanting_skill: 0,
            known_spells: Vec::new(),
            active_quest_statuses: Default::default(),
            active_quest_objective_counts: Default::default(),
            rewarded_quests: Default::default(),
            inventory_item_counts: Default::default(),
            party_member_phase_states: Default::default(),
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
    fn player_registry_publishes_loot_condition_state_like_cpp() {
        let (mut session, _, _) = make_session();
        let guid = ObjectGuid::create_player(1, 42);
        let registry = Arc::new(PlayerRegistry::default());
        session.set_player_guid(Some(guid));
        session.player_position = Some(Position::ZERO);
        session.player_name = Some("Tester".to_string());
        session.known_spells = vec![12_345];
        session.player_quests.insert(
            100,
            crate::handlers::quest::PlayerQuestStatus {
                quest_id: 100,
                status: 1,
                explored: false,
                objective_counts: vec![2, 3],
            },
        );
        session.rewarded_quests.insert(200);
        let item_guid = ObjectGuid::create_item(1, 500);
        session.inventory_items.insert(
            0,
            InventoryItem {
                guid: item_guid,
                entry_id: 9001,
                db_guid: 500,
                inventory_type: Some(0),
            },
        );
        session.insert_inventory_item_object(session.make_inventory_item_object(
            item_guid,
            9001,
            guid,
            4,
            0,
            ItemContext::None,
            0,
        ));
        let bag_guid = ObjectGuid::create_item(1, 501);
        session.inventory_items.insert(
            1,
            InventoryItem {
                guid: bag_guid,
                entry_id: 8000,
                db_guid: 501,
                inventory_type: Some(18),
            },
        );
        session.insert_inventory_item_object(session.make_inventory_item_object(
            bag_guid,
            8000,
            guid,
            1,
            0,
            ItemContext::None,
            1,
        ));
        let child_guid = ObjectGuid::create_item(1, 502);
        let mut child_item =
            session.make_inventory_item_object(child_guid, 9001, guid, 2, 0, ItemContext::None, 0);
        child_item.set_container_guid_and_slot(bag_guid, 0);
        session.insert_inventory_item_object(child_item);
        session.set_player_registry(Arc::clone(&registry));

        session.register_in_player_registry();
        {
            let info = registry.get(&guid).expect("registered player");
            assert_eq!(info.known_spells, vec![12_345]);
            assert_eq!(info.active_quest_statuses.get(&100), Some(&1));
            assert_eq!(
                info.active_quest_objective_counts.get(&100),
                Some(&vec![2, 3])
            );
            assert!(info.rewarded_quests.contains(&200));
            assert_eq!(info.inventory_item_counts.get(&9001), Some(&6));
        }

        session.known_spells.push(54_321);
        session.player_quests.insert(
            300,
            crate::handlers::quest::PlayerQuestStatus {
                quest_id: 300,
                status: 2,
                explored: false,
                objective_counts: vec![7],
            },
        );
        session.rewarded_quests.insert(400);
        if let Some(item) = session.inventory_item_objects.get_mut(&item_guid) {
            item.set_count(6);
        }
        session.sync_player_registry_state_like_cpp();

        let info = registry.get(&guid).expect("synced player");
        assert!(info.known_spells.contains(&54_321));
        assert_eq!(info.active_quest_statuses.get(&300), Some(&2));
        assert_eq!(info.active_quest_objective_counts.get(&300), Some(&vec![7]));
        assert!(info.rewarded_quests.contains(&400));
        assert_eq!(info.inventory_item_counts.get(&9001), Some(&8));
    }

    #[test]
    fn session_starts_authed() {
        let (session, _, _) = make_session();
        assert_eq!(session.state(), SessionState::Authed);
    }

    #[test]
    fn death_sync_preserves_existing_canonical_death_time_ms() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_001);
        register_test_creature(&mut session, manager.clone(), guid, 40);

        {
            let mut manager = manager.write().unwrap();
            let world_creature = manager.find_creature_mut(0, 0, guid).unwrap();
            world_creature.creature.mark_ai_dead(1_234);
        }
        session
            .mutate_world_creature(guid, |creature| {
                creature.take_damage(40);
            })
            .unwrap();

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert_eq!(
            world_creature.creature.ai_ownership().death_time_ms,
            Some(1_234)
        );
        assert_eq!(world_creature.current_hp(), 0);
    }

    #[tokio::test]
    async fn spell_damage_syncs_canonical_creature_health() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_002);
        session.player_guid = Some(ObjectGuid::create_player(1, 42));
        register_test_creature(&mut session, manager.clone(), guid, 40);

        session.apply_damage(guid, 7).await.unwrap();

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert_eq!(world_creature.current_hp(), 33);
    }

    #[tokio::test]
    async fn killing_moving_creature_sends_cpp_like_monster_move_stop() {
        let (mut session, _, send_rx) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_202);
        session.player_guid = Some(ObjectGuid::create_player(1, 202));
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature
                    .begin_move_spline_like_cpp(Position::new(20.0, 10.0, 0.0, 0.0))
                    .expect("valid represented spline");
            })
            .unwrap();

        session.apply_damage(guid, 40).await.unwrap();

        let sent = send_rx.try_recv().unwrap();
        let opcode = u16::from_le_bytes([sent[0], sent[1]]);
        assert_eq!(opcode, ServerOpcodes::OnMonsterMove as u16);
        let mut pkt = WorldPacket::from_bytes(&sent[2..]);
        assert_eq!(pkt.read_packed_guid().unwrap(), guid);
        assert_eq!(pkt.read_float().unwrap(), 10.0);
        assert_eq!(pkt.read_float().unwrap(), 10.0);
        assert_eq!(pkt.read_float().unwrap(), 0.0);
        assert_eq!(pkt.read_uint32().unwrap(), 3);
        assert_eq!(pkt.read_float().unwrap(), 0.0);
        assert_eq!(pkt.read_float().unwrap(), 0.0);
        assert_eq!(pkt.read_float().unwrap(), 0.0);
        assert!(!pkt.has_bit().unwrap());
        assert_eq!(pkt.read_bits(3).unwrap(), 2);
        assert_eq!(pkt.read_uint32().unwrap(), 0);
        assert_eq!(pkt.read_int32().unwrap(), 0);
        assert_eq!(pkt.read_uint32().unwrap(), 0);
        assert_eq!(pkt.read_uint32().unwrap(), 0);
        assert_eq!(pkt.read_uint8().unwrap(), 0);
        assert_eq!(pkt.read_packed_guid().unwrap(), ObjectGuid::EMPTY);
        assert_eq!(pkt.read_int8().unwrap(), -1);
    }

    #[test]
    fn combat_tick_damage_syncs_canonical_creature_health() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_003);
        let player = ObjectGuid::create_player(1, 43);
        session.player_guid = Some(player);
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert!(world_creature.current_hp() < 40);
    }

    #[test]
    fn combat_tick_uses_canonical_player_victim_when_session_target_is_empty_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_014);
        let player = ObjectGuid::create_player(1, 63);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Attacker".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().set_attacking(Some(guid));
                player.unit_mut().set_target(guid);
            })
            .unwrap();
        session.combat_target = None;
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert!(world_creature.current_hp() < 40);
        assert_eq!(session.combat_target, Some(guid));
    }

    #[test]
    fn combat_tick_uses_canonical_player_base_attack_timer_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_016);
        let player = ObjectGuid::create_player(1, 65);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Timer".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().set_attacking(Some(guid));
                player.unit_mut().set_target(guid);
                player
                    .unit_mut()
                    .set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                player
                    .unit_mut()
                    .set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
            })
            .unwrap();
        session.combat_target = None;
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();
        let after_first = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(after_first, 33);

        session.tick_combat_sync();
        let after_second = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(after_second, after_first);
    }

    #[test]
    fn combat_tick_damage_adds_creature_threat_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_024);
        let player = ObjectGuid::create_player(1, 73);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Threat".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let guard = manager.read().unwrap();
        let world_creature = guard.find_creature(0, 0, guid).unwrap();
        assert_eq!(
            world_creature
                .creature
                .unit()
                .subsystems()
                .combat
                .threat_value(player),
            Some(7.0)
        );
    }

    #[test]
    fn combat_tick_base_attack_casts_current_melee_spell_instead_of_damage_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_023);
        let player = ObjectGuid::create_player(1, 72);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "MeleeSpell".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        let melee_spell = wow_entities::CurrentSpellRef::new(12_345, Some(player), None);
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
                unit.set_current_cast_spell(wow_entities::CurrentSpellSlot::Melee, melee_spell);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 40);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .current_spell(wow_entities::CurrentSpellSlot::Melee),
            None
        );
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::BaseAttack),
            2_000
        );
    }

    #[test]
    fn combat_tick_los_failure_resets_timer_without_damage_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_022);
        let player = ObjectGuid::create_player(1, 71);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Los".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        session.player_melee_los_to_target_like_cpp = Some(false);
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 40);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::BaseAttack),
            2_000
        );
    }

    #[test]
    fn combat_tick_removes_attacking_interrupt_auras_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_021);
        let player = ObjectGuid::create_player(1, 70);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Aura".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        let aura = wow_entities::AppliedAuraRef::new(405, player, 0, 0x1);
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.subsystems_mut().auras.register_applied_aura(
                    aura,
                    None,
                    wow_entities::SPELL_AURA_INTERRUPT_FLAG_ATTACKING_LIKE_CPP,
                    0,
                );
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert!(!player_entity.unit().subsystems().auras.has_applied(aura));
    }

    #[test]
    fn combat_tick_pacified_player_resets_timer_without_damage_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_020);
        let player = ObjectGuid::create_player(1, 69);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Pacified".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_unit_flags_like_cpp(UnitFlags::PLAYER_CONTROLLED | UnitFlags::PACIFIED);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 40);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::BaseAttack),
            2_000
        );
    }

    #[test]
    fn combat_tick_uses_canonical_player_offhand_timer_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_019);
        let player = ObjectGuid::create_player(1, 68);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Dual".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_can_dual_wield_like_cpp(true);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::OffAttack, 2_000);
                unit.set_attack_timer(WeaponAttackType::BaseAttack, 500);
                unit.set_attack_timer(WeaponAttackType::OffAttack, 0);
                unit.set_weapon_damage(WeaponAttackType::OffAttack, 4.0, 4.0);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 36);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::OffAttack),
            2_000
        );
    }

    #[test]
    fn combat_tick_out_of_range_sets_short_retry_timer_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_017);
        let player = ObjectGuid::create_player(1, 66);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Far".to_string(),
            Position::new(100.0, 100.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().set_attacking(Some(guid));
                player.unit_mut().set_target(guid);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 40);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::BaseAttack),
            100
        );
    }

    #[test]
    fn combat_tick_bad_facing_sets_short_retry_timer_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_018);
        let player = ObjectGuid::create_player(1, 67);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Facing".to_string(),
            Position::new(10.0, 10.0, 0.0, std::f32::consts::PI),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().set_attacking(Some(guid));
                player.unit_mut().set_target(guid);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| {
                creature
                    .creature
                    .set_ai_position(Position::new(12.0, 10.0, 0.0, 0.0));
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let hp = manager
            .read()
            .unwrap()
            .find_creature(0, 0, guid)
            .unwrap()
            .current_hp();
        assert_eq!(hp, 40);
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(
            player_entity
                .unit()
                .attack_timer(WeaponAttackType::BaseAttack),
            100
        );
    }

    #[test]
    fn combat_tick_clears_canonical_player_attack_when_target_dies_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_015);
        let player = ObjectGuid::create_player(1, 64);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Killer".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.unit_mut().set_attacking(Some(guid));
                player.unit_mut().set_target(guid);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 1);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.unit_mut().add_attacker_like_cpp(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        {
            let manager = manager.read().unwrap();
            let world_creature = manager.find_creature(0, 0, guid).unwrap();
            assert_eq!(world_creature.current_hp(), 0);
            assert!(
                world_creature
                    .creature
                    .unit()
                    .subsystems()
                    .combat
                    .threat
                    .is_empty()
            );
            assert!(
                world_creature
                    .creature
                    .unit()
                    .subsystems()
                    .combat
                    .attackers
                    .is_empty()
            );
        }

        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(0, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(player_entity.unit().attacking(), None);
        assert_eq!(player_entity.unit().data().target, ObjectGuid::EMPTY);
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn combat_tick_reports_cpp_like_over_damage_on_killing_swing() {
        let (mut session, _, send_rx) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let guid = test_creature_guid(18_025);
        let player = ObjectGuid::create_player(1, 74);

        canonical.lock().unwrap().create_world_map(0, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 0,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Overkill".to_string(),
            Position::new(10.0, 10.0, 0.0, 0.0),
            0,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                unit.set_attacking(Some(guid));
                unit.set_target(guid);
                unit.set_base_attack_time_like_cpp(WeaponAttackType::BaseAttack, 2_000);
                unit.set_weapon_damage(WeaponAttackType::BaseAttack, 7.0, 7.0);
            })
            .unwrap();
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 3);
        session
            .mutate_world_creature(guid, |creature| {
                creature.enter_combat(player);
                creature.creature.ai_ownership_mut().last_swing_ms = 0;
                creature.creature.ai_ownership_mut().swing_timer_ms = 0;
            })
            .unwrap();

        session.tick_combat_sync();

        let sent = send_rx.try_recv().unwrap();
        let opcode = u16::from_le_bytes([sent[0], sent[1]]);
        assert_eq!(opcode, ServerOpcodes::AttackerStateUpdate as u16);
        let mut outer = WorldPacket::from_bytes(&sent[2..]);
        let size = outer.read_uint32().unwrap() as usize;
        let data = outer.read_bytes(size).unwrap();
        let mut info = WorldPacket::from_bytes(&data);
        assert_eq!(info.read_uint32().unwrap(), 0x0000_0002);
        assert_eq!(info.read_packed_guid().unwrap(), player);
        assert_eq!(info.read_packed_guid().unwrap(), guid);
        assert_eq!(info.read_int32().unwrap(), 7);
        assert_eq!(info.read_int32().unwrap(), 7);
        assert_eq!(info.read_int32().unwrap(), 4);
    }

    #[tokio::test]
    async fn attack_stop_resets_canonical_combat_state() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_004);
        let player = ObjectGuid::create_player(1, 44);
        session.player_guid = Some(player);
        session.combat_target = Some(guid);
        session.in_combat = true;
        register_test_creature(&mut session, manager.clone(), guid, 40);
        session
            .mutate_world_creature(guid, |creature| creature.enter_combat(player))
            .unwrap();

        session
            .handle_attack_stop(WorldPacket::from_bytes(&[]))
            .await;

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert_eq!(
            world_creature.creature.ai_state(),
            wow_entities::CreatureAiState::Returning
        );
        assert_eq!(world_creature.creature.ai_ownership().combat_target, None);
    }

    #[test]
    fn player_attack_start_stop_updates_canonical_unit_combat_state_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 45);
        let victim = test_creature_guid(18_006);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));

        session.start_player_attack_like_cpp(victim);
        {
            let guard = canonical.lock().unwrap();
            let player_entity = guard
                .find_map(571, 0)
                .unwrap()
                .map()
                .get_typed_player(player)
                .expect("player stored as canonical typed Player");
            assert_eq!(player_entity.unit().attacking(), Some(victim));
            assert_eq!(player_entity.unit().data().target, victim);
        }

        assert_eq!(session.stop_player_attack_like_cpp(), Some(victim));
        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .expect("player remains canonical typed Player");
        assert_eq!(player_entity.unit().attacking(), None);
        assert_eq!(player_entity.unit().data().target, ObjectGuid::EMPTY);
    }

    #[test]
    fn player_attack_tracks_typed_player_victim_attacker_set_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let attacker = ObjectGuid::create_player(1, 47);
        let victim = ObjectGuid::create_player(1, 48);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            attacker,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();

        let mut victim_player = Player::new(Some(8), false);
        victim_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(victim);
        victim_player
            .unit_mut()
            .world_mut()
            .set_map(571, 0)
            .unwrap();
        victim_player
            .unit_mut()
            .world_mut()
            .relocate(Position::new(11.0, 20.0, 30.0, 0.0));
        victim_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();
        canonical
            .lock()
            .unwrap()
            .find_map_mut(571, 0)
            .unwrap()
            .map_mut()
            .insert_map_object_record(
                wow_entities::MapObjectRecord::new_player(victim_player).unwrap(),
            )
            .unwrap();

        session.start_player_attack_like_cpp(victim);
        {
            let guard = canonical.lock().unwrap();
            let map = guard.find_map(571, 0).unwrap().map();
            assert!(
                map.get_typed_player(victim)
                    .unwrap()
                    .unit()
                    .has_attacker_like_cpp(attacker)
            );
        }

        assert_eq!(session.stop_player_attack_like_cpp(), Some(victim));
        let guard = canonical.lock().unwrap();
        assert!(
            !guard
                .find_map(571, 0)
                .unwrap()
                .map()
                .get_typed_player(victim)
                .unwrap()
                .unit()
                .has_attacker_like_cpp(attacker)
        );
    }

    #[test]
    fn player_attack_game_master_typed_player_is_rejected_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let attacker = ObjectGuid::create_player(1, 52);
        let victim = ObjectGuid::create_player(1, 53);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            attacker,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();

        let mut victim_player = Player::new(Some(9), false);
        victim_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(victim);
        victim_player
            .unit_mut()
            .world_mut()
            .set_map(571, 0)
            .unwrap();
        victim_player
            .unit_mut()
            .world_mut()
            .relocate(Position::new(11.0, 20.0, 30.0, 0.0));
        victim_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();
        victim_player.set_game_master_like_cpp(true);
        canonical
            .lock()
            .unwrap()
            .find_map_mut(571, 0)
            .unwrap()
            .map_mut()
            .insert_map_object_record(
                wow_entities::MapObjectRecord::new_player(victim_player).unwrap(),
            )
            .unwrap();

        session.start_player_attack_like_cpp(victim);

        let guard = canonical.lock().unwrap();
        let map = guard.find_map(571, 0).unwrap().map();
        assert_eq!(
            map.get_typed_player(attacker).unwrap().unit().attacking(),
            None
        );
        assert!(
            !map.get_typed_player(victim)
                .unwrap()
                .unit()
                .has_attacker_like_cpp(attacker)
        );
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn mounted_player_attack_is_rejected_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 49);
        let victim = test_creature_guid(18_007);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Mounted".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        session.set_player_mounted_like_cpp(true);

        session.start_player_attack_like_cpp(victim);

        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(player_entity.unit().attacking(), None);
        assert_eq!(player_entity.unit().data().target, ObjectGuid::EMPTY);
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn player_attack_rejects_canonical_uber_player_flag_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 62);
        let victim = test_creature_guid(18_013);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Uber".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        let _ = session.ensure_canonical_world_map_for_current_player_like_cpp();
        session
            .mutate_canonical_player_like_cpp(|player| {
                player.set_player_flag(PLAYER_FLAGS_UBER_LIKE_CPP);
            })
            .unwrap();

        session.start_player_attack_like_cpp(victim);

        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(player_entity.unit().attacking(), None);
        assert_eq!(player_entity.unit().data().target, ObjectGuid::EMPTY);
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn player_attack_from_vehicle_seat_requires_can_attack_flag_like_cpp() {
        let (mut session, _, _) = make_session();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 61);
        let blocked_victim = test_creature_guid(18_011);
        let allowed_victim = test_creature_guid(18_012);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Passenger".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));

        session.player_vehicle_seat_flags_like_cpp = Some(0);
        session.start_player_attack_like_cpp(blocked_victim);

        {
            let guard = canonical.lock().unwrap();
            let player_entity = guard
                .find_map(571, 0)
                .unwrap()
                .map()
                .get_typed_player(player)
                .unwrap();
            assert_eq!(player_entity.unit().attacking(), None);
            assert_eq!(player_entity.unit().data().target, ObjectGuid::EMPTY);
        }
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);

        session.player_vehicle_seat_flags_like_cpp = Some(wow_data::VEHICLE_SEAT_FLAG_CAN_ATTACK);
        session.start_player_attack_like_cpp(allowed_victim);

        let guard = canonical.lock().unwrap();
        let player_entity = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_player(player)
            .unwrap();
        assert_eq!(player_entity.unit().attacking(), Some(allowed_victim));
        assert_eq!(player_entity.unit().data().target, allowed_victim);
        assert_eq!(session.combat_target, Some(allowed_victim));
        assert!(session.in_combat);
    }

    #[test]
    fn player_attack_tracks_typed_creature_victim_attacker_set_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 50);
        let victim = test_creature_guid(18_008);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        session.register_world_creature(
            571,
            Position::new(11.0, 20.0, 30.0, 0.0),
            test_creature_create_data(victim, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );

        session.start_player_attack_like_cpp(victim);
        {
            let guard = canonical.lock().unwrap();
            let creature = guard
                .find_map(571, 0)
                .unwrap()
                .map()
                .get_typed_creature(victim)
                .unwrap();
            assert!(creature.unit().has_attacker_like_cpp(player));
        }

        assert_eq!(session.stop_player_attack_like_cpp(), Some(victim));
        let guard = canonical.lock().unwrap();
        let creature = guard
            .find_map(571, 0)
            .unwrap()
            .map()
            .get_typed_creature(victim)
            .unwrap();
        assert!(!creature.unit().has_attacker_like_cpp(player));
    }

    #[test]
    fn player_attack_dead_typed_creature_is_rejected_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 51);
        let victim = test_creature_guid(18_009);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        session.register_world_creature(
            571,
            Position::new(11.0, 20.0, 30.0, 0.0),
            test_creature_create_data(victim, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );
        session
            .mutate_world_creature(victim, |creature| {
                creature.take_damage(25);
            })
            .unwrap();

        session.start_player_attack_like_cpp(victim);

        let guard = canonical.lock().unwrap();
        let map = guard.find_map(571, 0).unwrap().map();
        assert_eq!(
            map.get_typed_player(player).unwrap().unit().attacking(),
            None
        );
        assert!(
            !map.get_typed_creature(victim)
                .unwrap()
                .unit()
                .has_attacker_like_cpp(player)
        );
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn player_attack_evading_typed_creature_is_rejected_like_cpp() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let canonical = shared_canonical_map_manager();
        let player = ObjectGuid::create_player(1, 54);
        let victim = test_creature_guid(18_010);

        canonical.lock().unwrap().create_world_map(571, 0);
        session.set_map_manager(manager);
        session.set_canonical_map_manager(Arc::clone(&canonical));
        session.set_map_store(Arc::new(wow_data::MapStore::from_entries([
            wow_data::MapEntry {
                id: 571,
                instance_type: wow_data::map::MAP_COMMON,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])));
        session.attach_player_controller_like_cpp(SessionPlayerController::new(
            player,
            "Warrior".to_string(),
            Position::new(10.0, 20.0, 30.0, 0.0),
            571,
            1,
            1,
            80,
            0,
        ));
        session.register_world_creature(
            571,
            Position::new(11.0, 20.0, 30.0, 0.0),
            test_creature_create_data(victim, 9001, 25),
            3,
            5,
            20.0,
            0,
            0,
            0,
            None,
            0,
            0,
            0,
            0,
            -1,
        );
        session
            .mutate_world_creature(victim, |creature| {
                creature.creature.set_in_evade_mode_like_cpp(true);
            })
            .unwrap();

        session.start_player_attack_like_cpp(victim);

        let guard = canonical.lock().unwrap();
        let map = guard.find_map(571, 0).unwrap().map();
        assert_eq!(
            map.get_typed_player(player).unwrap().unit().attacking(),
            None
        );
        assert!(
            !map.get_typed_creature(victim)
                .unwrap()
                .unit()
                .has_attacker_like_cpp(player)
        );
        assert_eq!(session.combat_target, None);
        assert!(!session.in_combat);
    }

    #[test]
    fn corpse_despawn_syncs_canonical_corpse_timer() {
        let (mut session, _, _) = make_session();
        let manager = shared_map_manager();
        let guid = test_creature_guid(18_005);
        register_test_creature(&mut session, manager.clone(), guid, 40);
        let despawn_at = Instant::now() + std::time::Duration::from_secs(30);

        session
            .mutate_world_creature(guid, |creature| {
                creature.take_damage(40);
                creature.set_corpse_despawn_at(Some(despawn_at));
            })
            .unwrap();

        let manager = manager.read().unwrap();
        let world_creature = manager.find_creature(0, 0, guid).unwrap();
        assert!(world_creature.corpse_despawn_at().is_some());
        assert_eq!(world_creature.current_hp(), 0);
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
            session
                .player_currencies
                .get(&395)
                .map(|currency| currency.state),
            Some(PlayerCurrencyState::Changed)
        );

        let delta = session.add_currency_vendor(396, 3).unwrap().unwrap();
        assert_eq!(delta.quantity, 3);
        assert_eq!(
            session
                .player_currencies
                .get(&396)
                .map(|currency| currency.state),
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
            session
                .player_currencies
                .get(&395)
                .map(|currency| currency.state),
            Some(PlayerCurrencyState::Changed)
        );
        assert!(!session.remove_currency(999, 1));

        let mut tx = SqlTransaction::new();
        session.append_player_currency_save_statements(&mut tx, 1);
        assert_eq!(tx.len(), 2);
        assert_eq!(
            session
                .player_currencies
                .get(&395)
                .map(|currency| currency.state),
            Some(PlayerCurrencyState::Unchanged)
        );
        assert_eq!(
            session
                .player_currencies
                .get(&396)
                .map(|currency| currency.state),
            Some(PlayerCurrencyState::Unchanged)
        );
        assert_eq!(
            session
                .player_currencies
                .get(&397)
                .map(|currency| currency.state),
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
        let expected = InventoryChangeFailure::new(InventoryResult::CantEquipLevelI, item1, item2)
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

        let expected =
            InventoryChangeFailure::error(InventoryResult::ItemMaxLimitCategoryEquippedExceededIs)
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

        let expected =
            SellResponse::error(ObjectGuid::EMPTY, item_guid, SellResult::YouDontOwnThatItem)
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
            random_select: 0,
            random_suffix_group_id: 0,
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
                start_quest_id: 0,
                stackable: i32::MAX,
                max_count: 3,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 99,
                buy_price: 123,
                vendor_stack_count: 2,
                price_variance: 1.25,
                price_random_value: 0.75,
                max_durability: 88,
                limit_category: 44,
                instance_bound: 7,
                zone_bound: [8, 9],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 3,
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
        assert_eq!(
            session.item_template_inventory_type(100),
            Some(InventoryType::Bag as u8)
        );
        assert_eq!(session.item_storage_template(101), None);
    }

    #[test]
    fn item_buy_and_sell_price_follow_contrasted_cpp_standard_price_shape() {
        let (mut session, _, _) = make_session();
        session.set_item_store(Arc::new(ItemStore::from_records([
            ItemRecord {
                id: 200,
                class_id: ItemClass::Armor as u8,
                subclass_id: 3,
                material: 0,
                inventory_type: InventoryType::Chest as i8,
                sheathe_type: 0,
                random_select: 0,
                random_suffix_group_id: 0,
            },
            ItemRecord {
                id: 201,
                class_id: ItemClass::Gem as u8,
                subclass_id: 11,
                material: 0,
                inventory_type: InventoryType::Relic as i8,
                sheathe_type: 0,
                random_select: 0,
                random_suffix_group_id: 0,
            },
        ])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([
            (
                200,
                ItemSparseTemplateEntry {
                    flags: [0, 0, 0, 0],
                    bag_family: 0,
                    start_quest_id: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 77,
                    buy_price: 123,
                    vendor_stack_count: 2,
                    price_variance: 1.5,
                    price_random_value: 2.0,
                    max_durability: 0,
                    limit_category: 0,
                    instance_bound: 0,
                    zone_bound: [0, 0],
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    required_expansion: 0,
                    bonding: 0,
                    container_slots: 0,
                    inventory_type: InventoryType::Chest as i8,
                },
            ),
            (
                201,
                ItemSparseTemplateEntry {
                    flags: [0, 0, 0, 0],
                    bag_family: 0,
                    start_quest_id: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 88,
                    buy_price: 0,
                    vendor_stack_count: 1,
                    price_variance: 1.0,
                    price_random_value: 1.0,
                    max_durability: 0,
                    limit_category: 0,
                    instance_bound: 0,
                    zone_bound: [0, 0],
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    required_expansion: 0,
                    bonding: 0,
                    container_slots: 0,
                    inventory_type: InventoryType::Relic as i8,
                },
            ),
        ])));
        session.set_import_price_stores(Arc::new(ImportPriceStores {
            armor: ImportPriceArmorStore::from_entries([ImportPriceArmorEntry {
                id: InventoryType::Chest as u32,
                cloth_modifier: 1.0,
                leather_modifier: 2.0,
                chain_modifier: 3.0,
                plate_modifier: 4.0,
            }]),
            quality: ImportPriceQualityStore::from_entries([ImportPriceQualityEntry {
                id: ItemQuality::Rare as u32 + 1,
                data: 5.0,
            }]),
            shield: ImportPriceShieldStore::from_entries([ImportPriceShieldEntry {
                id: 2,
                data: 9.0,
            }]),
            weapon: ImportPriceWeaponStore::from_entries([
                ImportPriceWeaponEntry { id: 3, data: 7.0 },
                ImportPriceWeaponEntry { id: 5, data: 11.0 },
            ]),
        }));
        session.set_item_price_base_store(Arc::new(ItemPriceBaseStore::from_entries([
            ItemPriceBaseEntry {
                id: 10,
                item_level: 10,
                armor: 100.0,
                weapon: 300.0,
            },
        ])));
        session.set_item_class_store(Arc::new(ItemClassStore::from_entries([ItemClassEntry {
            id: 4,
            class_id: ItemClass::Armor as i8,
            price_modifier: 0.25,
            flags: 0,
        }])));
        session.set_item_disenchant_loot_store(Arc::new(ItemDisenchantLootStore::from_entries([
            ItemDisenchantLootEntry {
                id: 900,
                subclass: -1,
                quality: ItemQuality::Rare as u8,
                min_level: 1,
                max_level: 20,
                skill_required: 175,
                expansion_id: 0,
                class_id: ItemClass::Armor as u32,
            },
        ])));
        session.set_item_currency_cost_store(Arc::new(ItemCurrencyCostStore::from_entries([
            ItemCurrencyCostEntry {
                id: 1,
                item_id: 201,
            },
        ])));

        assert_eq!(
            session.item_buy_price_like_cpp(200, ItemQuality::Rare as u32, 10),
            Some((4500, false))
        );
        assert_eq!(
            session.item_sell_price_like_cpp(200, ItemQuality::Rare as u32, 10),
            Some(77)
        );
        assert_eq!(
            session.item_buy_price_like_cpp(201, ItemQuality::Rare as u32, 10),
            Some((3500, false))
        );
        assert_eq!(
            session.item_disenchant_loot_like_cpp(200, ItemQuality::Rare as u32, 10, true),
            Some((900, 175))
        );
        assert_eq!(
            session.item_disenchant_loot_like_cpp(200, ItemQuality::Rare as u32, 10, false),
            None
        );
    }

    fn insert_open_item_bag_with_child(
        session: &mut WorldSession,
        player_guid: ObjectGuid,
        bag_slot: u8,
        inner_slot: u8,
    ) -> (ObjectGuid, ObjectGuid) {
        let bag_guid = ObjectGuid::create_item(1, 1001);
        session.inventory_items.insert(
            bag_slot,
            InventoryItem {
                guid: bag_guid,
                entry_id: 101,
                db_guid: 1001,
                inventory_type: Some(InventoryType::Bag as u8),
            },
        );
        let bag_item = session.make_inventory_item_object(
            bag_guid,
            101,
            player_guid,
            1,
            0,
            ItemContext::None,
            bag_slot,
        );
        session.insert_inventory_item_object(bag_item);

        let child_guid = ObjectGuid::create_item(1, 1002);
        let mut child = session.make_inventory_item_object(
            child_guid,
            700,
            player_guid,
            1,
            0,
            ItemContext::None,
            inner_slot,
        );
        child.set_container_guid_and_slot(bag_guid, bag_slot);
        session.insert_inventory_item_object(child);

        (bag_guid, child_guid)
    }

    fn install_open_item_has_loot_template(session: &mut WorldSession, entry: u32) {
        install_open_item_has_loot_template_with_lock(session, entry, 0);
    }

    fn install_open_item_template_with_flags(
        session: &mut WorldSession,
        entry: u32,
        flags: ItemFlags,
        lock_id: u16,
    ) {
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            entry,
            ItemSparseTemplateEntry {
                flags: [flags.bits() as u32, 0, 0, 0],
                bag_family: 0,
                start_quest_id: 0,
                stackable: 1,
                max_count: 0,
                lock_id,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 0,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));
    }

    fn install_open_item_has_loot_template_with_lock(
        session: &mut WorldSession,
        entry: u32,
        lock_id: u16,
    ) {
        install_open_item_template_with_flags(session, entry, ItemFlags::HAS_LOOT, lock_id);
    }

    fn install_lock_store(session: &mut WorldSession, lock_id: u32) {
        session.set_lock_store(Arc::new(LockStore::from_entries([LockEntry {
            id: lock_id,
            index: [0; 8],
            skill: [0; 8],
            lock_type: [0; 8],
            action: [0; 8],
        }])));
    }

    fn insert_open_item_top_level(
        session: &mut WorldSession,
        player_guid: ObjectGuid,
        slot: u8,
        item_guid: ObjectGuid,
        entry: u32,
        unlocked: bool,
    ) {
        session.inventory_items.insert(
            slot,
            InventoryItem {
                guid: item_guid,
                entry_id: entry,
                db_guid: item_guid.counter() as u64,
                inventory_type: None,
            },
        );
        let mut item = session.make_inventory_item_object(
            item_guid,
            entry,
            player_guid,
            1,
            0,
            ItemContext::None,
            slot,
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
                start_quest_id: 0,
                stackable: 1,
                max_count: 0,
                lock_id: 99,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 0,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
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
        session.inventory_items.insert(
            23,
            InventoryItem {
                guid: top_guid,
                entry_id: 700,
                db_guid: 900,
                inventory_type: None,
            },
        );
        let top_item = session.make_inventory_item_object(
            top_guid,
            700,
            player_guid,
            1,
            0,
            ItemContext::None,
            23,
        );
        session.insert_inventory_item_object(top_item);

        assert_eq!(
            session
                .get_inventory_item_by_pos(INVENTORY_SLOT_BAG_0, 23)
                .map(|i| i.guid),
            Some(top_guid)
        );
    }

    #[test]
    fn open_item_get_inventory_item_by_pos_excludes_buyback_top_level_like_cpp() {
        let (mut session, _, _) = make_session();
        session.buyback_items.insert(
            BUYBACK_SLOT_START,
            InventoryItem {
                guid: ObjectGuid::create_item(1, 901),
                entry_id: 701,
                db_guid: 901,
                inventory_type: None,
            },
        );

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
        let (_, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, INVENTORY_SLOT_BAG_START, 5);

        assert_eq!(
            session
                .get_inventory_item_by_pos(INVENTORY_SLOT_BAG_START, 5)
                .map(|i| i.guid),
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
            session
                .get_inventory_item_by_pos(BANK_SLOT_BAG_START, 5)
                .map(|i| i.guid),
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
        let (bag_guid, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, INVENTORY_SLOT_BAG_START, 5);

        let child = session.inventory_item_objects.get(&child_guid).unwrap();
        assert_eq!(child.container_guid(), bag_guid);
        assert_eq!(child.bag_slot(), INVENTORY_SLOT_BAG_START);
        assert_eq!(child.slot(), 5);
        assert_eq!(
            child.position(),
            u16::from(INVENTORY_SLOT_BAG_START) << 8 | 5
        );
    }

    async fn assert_open_item_nested_has_loot_opens_without_internal_bag_error(bag_slot: u8) {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template(&mut session, 700);
        let (_, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, bag_slot, 5);

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
    async fn open_item_wrapped_without_has_loot_does_not_generate_loot_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 903);
        session.set_player_guid(Some(player_guid));
        install_open_item_template_with_flags(&mut session, 700, ItemFlags::empty(), 0);
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 700, true);
        session
            .inventory_item_objects
            .get_mut(&item_guid)
            .unwrap()
            .set_item_flag(ItemFieldFlags::WRAPPED);

        session
            .handle_open_item(WorldPacket::from_bytes(&[INVENTORY_SLOT_BAG_0, 23]))
            .await;

        assert!(!session.loot_table.contains_key(&item_guid));
        assert!(send_rx.try_recv().is_err());
        assert!(
            session
                .inventory_item_objects
                .get(&item_guid)
                .is_some_and(|item| !item.loot_generated() && item.is_wrapped())
        );
    }

    #[test]
    fn open_item_wrapped_gift_row_helper_updates_runtime_and_top_level_metadata_like_cpp() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let gift_creator = ObjectGuid::create_player(1, 77);
        let item_guid = ObjectGuid::create_item(1, 904);
        session.set_player_guid(Some(player_guid));
        session.set_item_store(Arc::new(ItemStore::from_records([ItemRecord {
            id: 200,
            class_id: ItemClass::Weapon as u8,
            subclass_id: 0,
            material: 0,
            inventory_type: InventoryType::Weapon as i8,
            sheathe_type: 0,
            random_select: 0,
            random_suffix_group_id: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            200,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                start_quest_id: 0,
                stackable: 1,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 40,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::Weapon as i8,
            },
        )])));
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 100, true);
        {
            let item = session.inventory_item_objects.get_mut(&item_guid).unwrap();
            item.set_gift_creator(gift_creator);
            item.set_item_flag(ItemFieldFlags::WRAPPED);
            item.set_durability(55);
            item.force_state(ItemUpdateState::Unchanged);
        }

        let durability = session
            .apply_wrapped_gift_row_to_runtime_item_like_cpp(
                INVENTORY_SLOT_BAG_0,
                item_guid,
                23,
                200,
                ItemFieldFlags::SOULBOUND.bits(),
            )
            .unwrap();

        let item = session.inventory_item_objects.get(&item_guid).unwrap();
        assert_eq!(durability, 55);
        assert_eq!(item.object().entry(), 200);
        assert_eq!(item.data().gift_creator, ObjectGuid::EMPTY);
        assert_eq!(item.item_flags_bits(), ItemFieldFlags::SOULBOUND.bits());
        assert_eq!(item.data().max_durability, 40);
        assert_eq!(item.data().durability, 55);
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
        assert!(!item.is_wrapped());
        let inventory_item = session.inventory_items.get(&23).unwrap();
        assert_eq!(inventory_item.entry_id, 200);
        assert_eq!(
            inventory_item.inventory_type,
            Some(InventoryType::Weapon as u8)
        );
    }

    #[tokio::test]
    async fn open_item_wrapped_locked_template_returns_item_locked_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 905);
        session.set_player_guid(Some(player_guid));
        install_open_item_template_with_flags(&mut session, 700, ItemFlags::empty(), 123);
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 700, false);
        {
            let item = session.inventory_item_objects.get_mut(&item_guid).unwrap();
            item.set_item_flag(ItemFieldFlags::WRAPPED);
            item.set_durability(17);
            item.force_state(ItemUpdateState::Unchanged);
        }

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
        let item = session.inventory_item_objects.get(&item_guid).unwrap();
        assert_eq!(item.object().entry(), 700);
        assert_eq!(item.data().durability, 17);
        assert_eq!(item.update_state(), ItemUpdateState::Unchanged);
        assert!(item.is_wrapped());
    }

    #[tokio::test]
    async fn open_item_locked_container_returns_item_locked_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 900);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        install_lock_store(&mut session, 123);
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
        install_lock_store(&mut session, 123);
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
    async fn open_item_unknown_lock_id_returns_item_locked_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 903);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        session.set_lock_store(Arc::new(LockStore::from_entries([])));
        insert_open_item_top_level(&mut session, player_guid, 23, item_guid, 700, true);

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

    #[tokio::test]
    async fn open_item_missing_runtime_object_fails_closed_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 902);
        session.set_player_guid(Some(player_guid));
        install_open_item_has_loot_template_with_lock(&mut session, 700, 123);
        session.inventory_items.insert(
            23,
            InventoryItem {
                guid: item_guid,
                entry_id: 700,
                db_guid: item_guid.counter() as u64,
                inventory_type: None,
            },
        );
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
        assert!(
            session
                .get_inventory_item_by_pos(child_bag, child_slot)
                .is_none()
        );
        assert!(!session.inventory_item_objects.contains_key(&child_guid));
        assert!(session.inventory_items.contains_key(&bag_slot));
        assert_eq!(session.inventory_items[&bag_slot].guid, bag_guid);
    }

    #[test]
    fn open_item_release_destroy_nested_item_leaves_container_in_place() {
        assert_open_item_release_destroy_nested_item_leaves_container_in_place(
            INVENTORY_SLOT_BAG_START,
        );
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
                random_select: 0,
                random_suffix_group_id: 0,
            },
            ItemRecord {
                id: 101,
                class_id: ItemClass::Container as u8,
                subclass_id: 0,
                material: 0,
                inventory_type: InventoryType::Bag as i8,
                sheathe_type: 0,
                random_select: 0,
                random_suffix_group_id: 0,
            },
        ])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([
            (
                100,
                ItemSparseTemplateEntry {
                    flags: [0, 0, 0, 0],
                    bag_family: 0,
                    start_quest_id: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 0,
                    buy_price: 0,
                    vendor_stack_count: 1,
                    price_variance: 1.0,
                    price_random_value: 1.0,
                    max_durability: 0,
                    limit_category: 0,
                    instance_bound: 0,
                    zone_bound: [0, 0],
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    required_expansion: 0,
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
                    start_quest_id: 0,
                    stackable: 1,
                    max_count: 0,
                    lock_id: 0,
                    required_reputation_rank: 0,
                    sell_price: 0,
                    buy_price: 0,
                    vendor_stack_count: 1,
                    price_variance: 1.0,
                    price_random_value: 1.0,
                    max_durability: 0,
                    limit_category: 0,
                    instance_bound: 0,
                    zone_bound: [0, 0],
                    required_reputation_faction: 0,
                    allowable_class: -1,
                    required_expansion: 0,
                    bonding: 0,
                    container_slots: 16,
                    inventory_type: InventoryType::Bag as i8,
                },
            ),
        ])));

        let chest_guid = ObjectGuid::create_item(1, 1000);
        session.inventory_items.insert(
            EQUIPMENT_SLOT_CHEST,
            InventoryItem {
                guid: chest_guid,
                entry_id: 100,
                db_guid: 1000,
                inventory_type: Some(InventoryType::Chest as u8),
            },
        );
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
        session.inventory_items.insert(
            INVENTORY_SLOT_BAG_START,
            InventoryItem {
                guid: bag_guid,
                entry_id: 101,
                db_guid: 1001,
                inventory_type: Some(InventoryType::Bag as u8),
            },
        );
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
    fn moved_bag_detects_active_child_item_loot_like_cpp_swap_item() {
        let (mut session, _, _) = make_session();
        let player_guid = ObjectGuid::create_player(1, 1);
        let (bag_guid, child_guid) =
            insert_open_item_bag_with_child(&mut session, player_guid, INVENTORY_SLOT_BAG_START, 0);
        let other_child = ObjectGuid::create_item(1, 1003);

        assert!(!session.represented_bag_contains_active_item_loot_like_cpp(bag_guid));

        session.set_active_loot_guid(other_child);
        assert!(!session.represented_bag_contains_active_item_loot_like_cpp(bag_guid));

        session.set_active_loot_guid(child_guid);
        assert!(!session.represented_bag_contains_active_item_loot_like_cpp(bag_guid));

        session.loot_table.insert(
            child_guid,
            CreatureLoot {
                loot_guid: child_guid,
                coins: 0,
                unlooted_count: 0,
                loot_type: LOOT_TYPE_ITEM_LIKE_CPP,
                dungeon_encounter_id: 0,
                loot_method: 0,
                loot_master: ObjectGuid::EMPTY,
                round_robin_player: ObjectGuid::EMPTY,
                player_ffa_items: Vec::new(),
                players_looting: Vec::new(),
                allowed_looters: Vec::new(),
                items: vec![LootEntry {
                    loot_list_id: 1,
                    item_id: 700,
                    quantity: 1,
                    random_properties_id: 0,
                    random_properties_seed: 0,
                    item_context: ItemContext::None as u8,
                    flags: LootEntryFlags::default(),
                    allowed_looters: vec![player_guid],
                    roll_winner: ObjectGuid::EMPTY,
                    ffa_looted_by: Vec::new(),
                    taken: false,
                }],
                looted_by_player: false,
            },
        );

        assert!(session.represented_bag_contains_active_item_loot_like_cpp(bag_guid));
    }

    #[tokio::test]
    async fn loot_money_consumes_only_current_active_loot_like_cpp() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let active_guid = test_creature_guid(19_001);
        let inactive_guid = test_creature_guid(19_002);
        session.set_player_guid(Some(player_guid));
        session.player_gold = 100;
        session.loot_table.insert(
            active_guid,
            CreatureLoot {
                loot_guid: active_guid,
                coins: 37,
                unlooted_count: 0,
                loot_type: LOOT_TYPE_CORPSE_LIKE_CPP,
                dungeon_encounter_id: 0,
                loot_method: 0,
                loot_master: ObjectGuid::EMPTY,
                round_robin_player: ObjectGuid::EMPTY,
                player_ffa_items: Vec::new(),
                players_looting: Vec::new(),
                allowed_looters: Vec::new(),
                items: Vec::new(),
                looted_by_player: false,
            },
        );
        session.loot_table.insert(
            inactive_guid,
            CreatureLoot {
                loot_guid: inactive_guid,
                coins: 91,
                unlooted_count: 0,
                loot_type: LOOT_TYPE_CORPSE_LIKE_CPP,
                dungeon_encounter_id: 0,
                loot_method: 0,
                loot_master: ObjectGuid::EMPTY,
                round_robin_player: ObjectGuid::EMPTY,
                player_ffa_items: Vec::new(),
                players_looting: Vec::new(),
                allowed_looters: Vec::new(),
                items: vec![LootEntry {
                    loot_list_id: 0,
                    item_id: 25,
                    quantity: 1,
                    random_properties_id: 0,
                    random_properties_seed: 0,
                    item_context: 0,
                    flags: LootEntryFlags::default(),
                    allowed_looters: Vec::new(),
                    roll_winner: ObjectGuid::EMPTY,
                    ffa_looted_by: Vec::new(),
                    taken: false,
                }],
                looted_by_player: false,
            },
        );
        session.set_active_loot_guid(active_guid);

        let mut pkt = WorldPacket::new_empty();
        pkt.write_bit(false);
        pkt.flush_bits();
        pkt.reset_read();
        session.handle_loot_money(pkt).await;

        assert_eq!(session.player_gold, 137);
        assert_eq!(session.loot_table.get(&active_guid).unwrap().coins, 0);
        assert_eq!(session.loot_table.get(&inactive_guid).unwrap().coins, 91);

        let coin_removed = send_rx.try_recv().unwrap();
        let mut coin_removed = WorldPacket::from_bytes(&coin_removed);
        assert_eq!(
            coin_removed.read_uint16().unwrap(),
            ServerOpcodes::CoinRemoved as u16
        );
        assert_eq!(coin_removed.read_packed_guid().unwrap(), active_guid);

        let money_notify = send_rx.try_recv().unwrap();
        let mut money_notify = WorldPacket::from_bytes(&money_notify);
        assert_eq!(
            money_notify.read_uint16().unwrap(),
            ServerOpcodes::LootMoneyNotify as u16
        );
        assert_eq!(money_notify.read_uint64().unwrap(), 37);
        assert!(send_rx.try_recv().is_err());
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
                start_quest_id: 0,
                stackable: 1,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 55,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
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
        assert_eq!(
            session
                .inventory_item_objects
                .get(&item_guid)
                .unwrap()
                .slot(),
            36
        );
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
        session.inventory_items.insert(
            23,
            InventoryItem {
                guid: item_guid,
                entry_id: 700,
                db_guid: 900,
                inventory_type: None,
            },
        );
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
        assert!(
            accessor
                .read()
                .player_item(player_guid, item_guid)
                .is_none()
        );

        session.cleanup_shared_runtime_state();
        let accessor = accessor.read();
        assert!(accessor.find_connected_player(player_guid).is_none());
        assert!(session.inventory_items.is_empty());
        assert!(session.inventory_item_objects.is_empty());
    }

    #[test]
    fn object_accessor_sync_preserves_represented_player_phase_shift_like_cpp() {
        let (mut session, _, _) = make_session();
        let accessor = new_shared_object_accessor();
        let player_guid = ObjectGuid::create_player(1, 43);

        let mut player_phase_shift = PhaseShift::default();
        player_phase_shift.add_phase_like_cpp(20, wow_constants::PhaseFlags::empty(), 1);
        session.set_represented_player_phase_shift_like_cpp(player_phase_shift.clone());

        session.set_object_accessor(Arc::clone(&accessor));
        session.set_player_guid(Some(player_guid));
        session.player_name = Some("Thrall".into());
        session.player_position = Some(Position::new(1.0, 2.0, 3.0, 0.0));
        session.current_map_id = 1;
        session.sync_object_accessor_player();

        let accessor = accessor.read();
        let player = accessor.find_connected_player(player_guid).unwrap();
        assert!(player.phase_shift().can_see(&player_phase_shift));
        assert!(player.phase_shift().has_phase_like_cpp(20));
    }

    #[tokio::test]
    async fn disconnect_cleanup_releases_active_loot_views_like_cpp_logout_player() {
        let (mut session, _, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let loot_guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 19_040);
        session.set_player_guid(Some(player_guid));
        session.set_active_loot_guid(loot_guid);
        session.loot_table.insert(
            loot_guid,
            CreatureLoot {
                loot_guid,
                coins: 0,
                unlooted_count: 0,
                loot_type: LOOT_TYPE_CORPSE_LIKE_CPP,
                dungeon_encounter_id: 0,
                loot_method: 0,
                loot_master: ObjectGuid::EMPTY,
                round_robin_player: ObjectGuid::EMPTY,
                player_ffa_items: Vec::new(),
                players_looting: Vec::new(),
                allowed_looters: Vec::new(),
                items: vec![LootEntry {
                    loot_list_id: 0,
                    item_id: 25,
                    quantity: 1,
                    random_properties_id: 0,
                    random_properties_seed: 0,
                    item_context: 0,
                    flags: LootEntryFlags::default(),
                    allowed_looters: vec![player_guid],
                    roll_winner: ObjectGuid::EMPTY,
                    ffa_looted_by: Vec::new(),
                    taken: false,
                }],
                looted_by_player: false,
            },
        );

        session
            .cleanup_shared_runtime_state_on_disconnect_like_cpp()
            .await;

        let sent = send_rx.try_recv().unwrap();
        let mut sent = WorldPacket::from_bytes(&sent);
        assert_eq!(
            sent.read_uint16().unwrap(),
            wow_constants::ServerOpcodes::LootRelease as u16
        );
        assert_eq!(sent.read_packed_guid().unwrap(), loot_guid);
        assert_eq!(sent.read_packed_guid().unwrap(), player_guid);
        assert!(!session.is_active_loot_guid(loot_guid));
        assert!(session.loot_table.contains_key(&loot_guid));
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
            random_select: 0,
            random_suffix_group_id: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            700,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                start_quest_id: 0,
                stackable: 20,
                max_count: 0,
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 0,
                limit_category: 0,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));

        session.inventory_items.insert(
            35,
            InventoryItem {
                guid: item_guid,
                entry_id: 700,
                db_guid: 900,
                inventory_type: None,
            },
        );
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
            session.inventory_items.insert(
                slot,
                InventoryItem {
                    guid: item_guid,
                    entry_id: 700,
                    db_guid,
                    inventory_type: None,
                },
            );
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
            }]),
        ));

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
        assert_eq!(
            packet.display_text,
            ItemPushResultDisplayType::EncounterLoot
        );
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
    fn dispatch_metadata_matches_cpp_for_registered_active_opcodes() {
        let (session, _, _) = make_session();
        let table = &session.dispatch_table;

        fn status_from_cpp(value: &str) -> Option<SessionStatus> {
            match value {
                "STATUS_AUTHED" => Some(SessionStatus::Authed),
                "STATUS_LOGGEDIN" => Some(SessionStatus::LoggedIn),
                "STATUS_TRANSFER" => Some(SessionStatus::Transfer),
                "STATUS_LOGGEDIN_OR_RECENTLY_LOGGOUT" => {
                    Some(SessionStatus::LoggedInOrRecentlyLogout)
                }
                "STATUS_NEVER" | "STATUS_UNHANDLED" => None,
                other => panic!("unknown C++ session status {other}"),
            }
        }

        fn processing_from_cpp(value: &str) -> PacketProcessing {
            match value {
                "PROCESS_INPLACE" => PacketProcessing::Inplace,
                "PROCESS_THREADUNSAFE" => PacketProcessing::ThreadUnsafe,
                "PROCESS_THREADSAFE" => PacketProcessing::ThreadSafe,
                other => panic!("unknown C++ packet processing {other}"),
            }
        }

        let cpp_metadata =
            include_str!("../../../docs/migration/inventory/cpp-client-handlers.tsv");
        let mut expected = std::collections::HashMap::new();
        let mut cpp_never_or_unhandled = std::collections::HashSet::new();

        for line in cpp_metadata.lines().skip(1) {
            let columns: Vec<_> = line.split('\t').collect();
            let rust_const = columns[9];
            if rust_const == "-" {
                continue;
            }

            let cpp_status = columns[3];
            let cpp_processing = columns[4];
            if let Some(status) = status_from_cpp(cpp_status) {
                expected.insert(rust_const, (status, processing_from_cpp(cpp_processing)));
            } else {
                cpp_never_or_unhandled.insert(rust_const);
            }
        }

        let compatibility_exceptions = [
            "BattlePayGetPurchaseList",
            "ConnectToFailed",
            "GetAccountCharacterList",
            "OverrideScreenFlash",
            "Ping",
            "QueryCountdownTimer",
            "ReportClientVariables",
            "ReportEnabledAddons",
            "ReportKeybindingExecutionCounts",
            "RequestConquestFormulaConstants",
            "UpdateVasPurchaseStates",
        ];

        for entry in table.values() {
            let opcode_name = format!("{:?}", entry.opcode);
            if compatibility_exceptions.contains(&opcode_name.as_str()) {
                assert!(
                    cpp_never_or_unhandled.contains(opcode_name.as_str()),
                    "{opcode_name} is listed as a compatibility exception but C++ metadata is active"
                );
                continue;
            }

            let (status, processing) = expected
                .get(opcode_name.as_str())
                .unwrap_or_else(|| panic!("missing C++ metadata row for {opcode_name}"));
            assert_eq!(entry.status, *status, "{opcode_name} status");
            assert_eq!(entry.processing, *processing, "{opcode_name} processing");
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
