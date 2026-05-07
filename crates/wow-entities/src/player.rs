use std::collections::HashSet;

use bitflags::bitflags;
use wow_constants::{
    BagFamilyMask, Gender, InventoryResult, InventoryType, ItemBondingType, ItemClass,
    ItemFieldFlags, ItemFieldFlags2, ItemSubClassContainer, ItemSubClassQuiver, ItemSubClassWeapon,
    ItemSubclassProfession, ItemUpdateState, PowerType, TypeId, TypeMask,
};
use wow_core::ObjectGuid;

use crate::{
    Bag, EQUIPMENT_SLOT_BACK, EQUIPMENT_SLOT_BODY, EQUIPMENT_SLOT_CHEST, EQUIPMENT_SLOT_END,
    EQUIPMENT_SLOT_FEET, EQUIPMENT_SLOT_FINGER1, EQUIPMENT_SLOT_FINGER2, EQUIPMENT_SLOT_HANDS,
    EQUIPMENT_SLOT_HEAD, EQUIPMENT_SLOT_LEGS, EQUIPMENT_SLOT_MAINHAND, EQUIPMENT_SLOT_NECK,
    EQUIPMENT_SLOT_OFFHAND, EQUIPMENT_SLOT_SHOULDERS, EQUIPMENT_SLOT_TABARD,
    EQUIPMENT_SLOT_TRINKET1, EQUIPMENT_SLOT_TRINKET2, EQUIPMENT_SLOT_WAIST, EQUIPMENT_SLOT_WRISTS,
    INVENTORY_SLOT_BAG_0, Item, ItemStorageTemplate, MAX_BAG_SIZE, NULL_SLOT, ObjectDataUpdate,
    PROFESSION_SLOT_COOKING_GEAR1, PROFESSION_SLOT_COOKING_TOOL, PROFESSION_SLOT_END,
    PROFESSION_SLOT_FISHING_TOOL, PROFESSION_SLOT_MAX_COUNT, PROFESSION_SLOT_PROFESSION1_GEAR1,
    PROFESSION_SLOT_PROFESSION1_GEAR2, PROFESSION_SLOT_PROFESSION1_TOOL,
    PROFESSION_SLOT_PROFESSION2_GEAR1, PROFESSION_SLOT_PROFESSION2_GEAR2, PROFESSION_SLOT_START,
    Unit, UnitDataUpdate, UpdateMask, item_can_go_into_bag,
    update_fields::{
        ACTIVE_PLAYER_DATA_BITS, PLAYER_DATA_BITS, TYPEID_ACTIVE_PLAYER, TYPEID_PLAYER,
    },
};

pub const MAX_MONEY_AMOUNT: u64 = 99_999_999_999;
pub const TEAM_OTHER: u8 = 0;
pub const TEAM_HORDE_ID: u32 = 67;
pub const TEAM_ALLIANCE_ID: u32 = 469;
pub const CLASS_WARRIOR: u8 = 1;
pub const CLASS_PALADIN: u8 = 2;
pub const CLASS_HUNTER: u8 = 3;
pub const CLASS_SHAMAN: u8 = 7;
pub const SKILL_PLATE_MAIL: u32 = 293;
pub const SKILL_MAIL: u32 = 413;
pub const NULL_BAG: u8 = 0;

pub const PLAYER_DATA_PARENT_BIT: usize = 0;
pub const PLAYER_DATA_LOOT_TARGET_GUID_BIT: usize = 6;
pub const PLAYER_DATA_FLAGS_BIT: usize = 7;
pub const PLAYER_DATA_FLAGS_EX_BIT: usize = 8;
pub const PLAYER_DATA_NUM_BANK_SLOTS_BIT: usize = 12;
pub const PLAYER_DATA_NATIVE_SEX_BIT: usize = 13;
pub const PLAYER_DATA_CURRENT_SPEC_ID_BIT: usize = 24;
pub const PLAYER_DATA_VISIBLE_ITEMS_PARENT_BIT: usize = 61;
pub const PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT: usize = 62;

pub const ACTIVE_PLAYER_DATA_PARENT_BIT: usize = 0;
pub const ACTIVE_PLAYER_DATA_COINAGE_BIT: usize = 28;
pub const ACTIVE_PLAYER_DATA_XP_BIT: usize = 29;
pub const ACTIVE_PLAYER_DATA_NEXT_LEVEL_XP_BIT: usize = 30;
pub const ACTIVE_PLAYER_DATA_CHARACTER_POINTS_BIT: usize = 33;
pub const ACTIVE_PLAYER_DATA_NUM_BACKPACK_SLOTS_BIT: usize = 104;
pub const ACTIVE_PLAYER_DATA_INV_SLOTS_PARENT_BIT: usize = 124;
pub const ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT: usize = 125;
pub const ACTIVE_PLAYER_DATA_BUYBACK_PARENT_BIT: usize = 549;
pub const ACTIVE_PLAYER_DATA_BUYBACK_PRICE_FIRST_BIT: usize = 550;
pub const ACTIVE_PLAYER_DATA_BUYBACK_TIMESTAMP_FIRST_BIT: usize = 562;
pub const PLAYER_SLOT_END: usize = 141;
pub const INVENTORY_DEFAULT_SIZE: u8 = 16;
pub const INVENTORY_SLOT_BAG_START: u8 = 30;
pub const INVENTORY_SLOT_BAG_END: u8 = 34;
pub const REAGENT_BAG_SLOT_START: u8 = 34;
pub const REAGENT_BAG_SLOT_END: u8 = 35;
pub const INVENTORY_SLOT_ITEM_START: u8 = 35;
pub const INVENTORY_SLOT_ITEM_END: u8 = 59;
pub const BANK_SLOT_ITEM_START: u8 = 59;
pub const BANK_SLOT_ITEM_END: u8 = 87;
pub const BANK_SLOT_BAG_START: u8 = 87;
pub const BANK_SLOT_BAG_END: u8 = 94;
pub const BUYBACK_SLOT_START: u8 = 94;
pub const BUYBACK_SLOT_END: u8 = 106;
pub const BUYBACK_SLOT_COUNT: usize = (BUYBACK_SLOT_END - BUYBACK_SLOT_START) as usize;
pub const KEYRING_SLOT_START: u8 = 106;
pub const KEYRING_SLOT_END: u8 = 138;
pub const CHILD_EQUIPMENT_SLOT_START: u8 = 138;
pub const CHILD_EQUIPMENT_SLOT_END: u8 = 141;
pub const ITEM_LIMIT_CATEGORY_MODE_HAVE: u8 = 0;
pub const ITEM_LIMIT_CATEGORY_MODE_EQUIP: u8 = 1;

pub const fn make_item_pos(bag: u8, slot: u8) -> u16 {
    u16::from_be_bytes([bag, slot])
}

pub fn is_inventory_pos(bag: u8, slot: u8) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 && slot == NULL_SLOT {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0
        && (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).contains(&slot)
    {
        return true;
    }
    if (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&bag) {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0 && (KEYRING_SLOT_START..KEYRING_SLOT_END).contains(&slot) {
        return true;
    }
    if is_child_equipment_pos(bag, slot) {
        return true;
    }
    false
}

pub fn is_inventory_packed_pos(pos: u16) -> bool {
    let [bag, slot] = pos.to_be_bytes();
    is_inventory_pos(bag, slot)
}

pub fn is_equipment_pos(bag: u8, slot: u8) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 && slot < EQUIPMENT_SLOT_END {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0 && (PROFESSION_SLOT_START..PROFESSION_SLOT_END).contains(&slot) {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0
        && (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
    {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0 && (REAGENT_BAG_SLOT_START..REAGENT_BAG_SLOT_END).contains(&slot)
    {
        return true;
    }
    false
}

pub fn is_equipment_packed_pos(pos: u16) -> bool {
    let [bag, slot] = pos.to_be_bytes();
    is_equipment_pos(bag, slot)
}

pub fn is_bank_pos(bag: u8, slot: u8) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 && (BANK_SLOT_ITEM_START..BANK_SLOT_ITEM_END).contains(&slot) {
        return true;
    }
    if bag == INVENTORY_SLOT_BAG_0 && (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&slot) {
        return true;
    }
    if (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&bag) {
        return true;
    }
    false
}

pub fn is_bank_packed_pos(pos: u16) -> bool {
    let [bag, slot] = pos.to_be_bytes();
    is_bank_pos(bag, slot)
}

pub fn is_bag_pos(pos: u16) -> bool {
    let [bag, slot] = pos.to_be_bytes();
    bag == INVENTORY_SLOT_BAG_0 && is_bag_storage_slot(slot)
}

pub fn is_child_equipment_pos(bag: u8, slot: u8) -> bool {
    bag == INVENTORY_SLOT_BAG_0
        && (CHILD_EQUIPMENT_SLOT_START..CHILD_EQUIPMENT_SLOT_END).contains(&slot)
}

pub fn is_child_equipment_packed_pos(pos: u16) -> bool {
    let [bag, slot] = pos.to_be_bytes();
    is_child_equipment_pos(bag, slot)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemPosCount {
    pub pos: u16,
    pub count: u32,
}

impl ItemPosCount {
    pub const fn new(pos: u16, count: u32) -> Self {
        Self { pos, count }
    }

    pub fn is_contained_in(&self, positions: &[ItemPosCount]) -> bool {
        positions.iter().any(|position| position.pos == self.pos)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ItemSlotRef<'a> {
    pub bag: u8,
    pub slot: u8,
    pub item: &'a Item,
}

impl<'a> ItemSlotRef<'a> {
    pub const fn new(bag: u8, slot: u8, item: &'a Item) -> Self {
        Self { bag, slot, item }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ItemStorageRef<'a> {
    pub bag: u8,
    pub slot: u8,
    pub item: &'a Item,
    pub template: Option<&'a ItemStorageTemplate>,
}

impl<'a> ItemStorageRef<'a> {
    pub const fn new(
        bag: u8,
        slot: u8,
        item: &'a Item,
        template: Option<&'a ItemStorageTemplate>,
    ) -> Self {
        Self {
            bag,
            slot,
            item,
            template,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BagTemplateRef<'a> {
    pub bag: u8,
    pub template: &'a ItemStorageTemplate,
}

impl<'a> BagTemplateRef<'a> {
    pub const fn new(bag: u8, template: &'a ItemStorageTemplate) -> Self {
        Self { bag, template }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CanStoreItemArgs<'a> {
    pub bag: u8,
    pub slot: u8,
    pub entry: u32,
    pub count: u32,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub source_item: Option<&'a Item>,
    pub source_is_not_empty_bag: bool,
    pub source_bop_trade_allowed_for_player: bool,
    pub swap: bool,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub slot_items: &'a [ItemSlotRef<'a>],
    pub stored_items: &'a [ItemStorageRef<'a>],
    pub bag_templates: &'a [BagTemplateRef<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct CanBankItemArgs<'a> {
    pub bag: u8,
    pub slot: u8,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub source_item: Option<&'a Item>,
    pub source_is_not_empty_bag: bool,
    pub source_is_bag: bool,
    pub source_is_currency_token: bool,
    pub source_bop_trade_allowed_for_player: bool,
    pub swap: bool,
    pub can_use_result: InventoryResult,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub slot_items: &'a [ItemSlotRef<'a>],
    pub stored_items: &'a [ItemStorageRef<'a>],
    pub bag_templates: &'a [BagTemplateRef<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct FindEquipSlotArgs<'a> {
    pub proto: &'a ItemStorageTemplate,
    pub slot: u8,
    pub swap: bool,
    pub can_dual_wield: bool,
    pub can_titan_grip: bool,
    pub is_two_hand_used: bool,
    pub has_required_profession_skill: bool,
    pub profession_slot: Option<u8>,
    pub equipped_items: &'a [ItemSlotRef<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct CanEquipItemArgs<'a> {
    pub slot: u8,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub source_item: Option<&'a Item>,
    pub source_bop_trade_allowed_for_player: bool,
    pub swap: bool,
    pub not_loading: bool,
    pub is_stunned: bool,
    pub is_charmed: bool,
    pub is_in_combat: bool,
    pub is_in_progress_arena: bool,
    pub weapon_change_timer_active: bool,
    pub current_generic_spell_allows_equip: Option<bool>,
    pub current_channeled_spell_allows_equip: Option<bool>,
    pub heirloom_required_level_failed: bool,
    pub can_use_result: InventoryResult,
    pub can_equip_unique_result: InventoryResult,
    pub can_dual_wield: bool,
    pub can_titan_grip: bool,
    pub is_two_hand_used: bool,
    pub proto_always_allow_dual_wield: bool,
    pub has_required_profession_skill: bool,
    pub profession_slot: Option<u8>,
    pub offhand_can_unequip_result: InventoryResult,
    pub offhand_can_store_result: InventoryResult,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub equipped_items: &'a [ItemSlotRef<'a>],
    pub stored_items: &'a [ItemStorageRef<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanEquipItemOutcome {
    pub result: InventoryResult,
    pub dest: u16,
    pub unique_ignore_slot: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipItemObjectOutcome {
    Equipped,
    Merged,
}

#[derive(Debug, Clone, Copy)]
pub struct CanUnequipItemArgs<'a> {
    pub pos: u16,
    pub source_item: Option<&'a Item>,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub swap: bool,
    pub source_is_not_empty_bag: bool,
    pub is_charmed: bool,
    pub is_in_combat: bool,
    pub is_in_progress_arena: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CanUseItemTemplateArgs<'a> {
    pub proto: Option<&'a ItemStorageTemplate>,
    pub skip_required_level_check: bool,
    pub player_level: u8,
    pub team: u32,
    pub allowable_class_matches: bool,
    pub allowable_race_matches: bool,
    pub internal_item: bool,
    pub faction_horde: bool,
    pub faction_alliance: bool,
    pub required_skill: u32,
    pub required_skill_rank: u32,
    pub required_skill_value: u32,
    pub required_spell: u32,
    pub has_required_spell: bool,
    pub base_required_level: u8,
    pub holiday_id: u32,
    pub holiday_active: bool,
    pub required_reputation_faction: u32,
    pub required_reputation_rank: u32,
    pub player_reputation_rank: u32,
    pub effect0_spell_id: Option<u32>,
    pub effect1_spell_id: Option<u32>,
    pub has_effect1_spell: bool,
    pub artifact_specialization: Option<u32>,
    pub primary_specialization: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct CanUseItemArgs<'a> {
    pub source_item: Option<&'a Item>,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub not_loading: bool,
    pub is_alive: bool,
    pub player_level: u8,
    pub item_required_level: u8,
    pub source_bop_trade_allowed_for_player: bool,
    pub template_args: CanUseItemTemplateArgs<'a>,
    pub item_skill: u32,
    pub item_skill_value: u32,
    pub has_item_skill: bool,
    pub player_class: u8,
    pub proto_is_heirloom: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EquippedGemRef {
    pub slot: u8,
    pub entry: u32,
    pub limit_category: u32,
}

impl EquippedGemRef {
    pub const fn new(slot: u8, entry: u32, limit_category: u32) -> Self {
        Self {
            slot,
            entry,
            limit_category,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CanEquipUniqueItemTemplateArgs<'a> {
    pub proto: Option<&'a ItemStorageTemplate>,
    pub except_slot: u8,
    pub limit_count: u32,
    pub unique_equippable: bool,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub equipped_items: &'a [ItemStorageRef<'a>],
    pub equipped_gems: &'a [EquippedGemRef],
}

#[derive(Debug, Clone, Copy)]
pub struct SocketedGemUniqueRef<'a> {
    pub proto: Option<&'a ItemStorageTemplate>,
    pub unique_equippable: bool,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub source_limit_category_count: u32,
}

impl<'a> SocketedGemUniqueRef<'a> {
    pub const fn new(
        proto: Option<&'a ItemStorageTemplate>,
        unique_equippable: bool,
        limit_category: Option<&'a ItemLimitCategoryTemplate>,
        source_limit_category_count: u32,
    ) -> Self {
        Self {
            proto,
            unique_equippable,
            limit_category,
            source_limit_category_count,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CanEquipUniqueItemArgs<'a> {
    pub source_item: Option<&'a Item>,
    pub proto: Option<&'a ItemStorageTemplate>,
    pub except_slot: u8,
    pub limit_count: u32,
    pub unique_equippable: bool,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub equipped_items: &'a [ItemStorageRef<'a>],
    pub equipped_gems: &'a [EquippedGemRef],
    pub socketed_gems: &'a [SocketedGemUniqueRef<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanStoreItemOutcome {
    pub result: InventoryResult,
    pub no_space_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemLimitCategoryTemplate {
    pub id: u32,
    pub quantity: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct CanTakeMoreSimilarItemsArgs<'a> {
    pub proto: Option<&'a ItemStorageTemplate>,
    pub count: u32,
    pub source_item: Option<&'a Item>,
    pub current_item_count: u32,
    pub limit_category: Option<&'a ItemLimitCategoryTemplate>,
    pub current_limit_category_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanTakeMoreSimilarItemsOutcome {
    pub result: InventoryResult,
    pub no_space_count: Option<u32>,
    pub offending_item_id: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct DestroyItemCountItemRef<'a> {
    pub bag: u8,
    pub slot: u8,
    pub item: &'a Item,
    pub can_unequip_result: InventoryResult,
}

impl<'a> DestroyItemCountItemRef<'a> {
    pub const fn new(bag: u8, slot: u8, item: &'a Item) -> Self {
        Self {
            bag,
            slot,
            item,
            can_unequip_result: InventoryResult::Ok,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroyItemCountAction {
    pub bag: u8,
    pub slot: u8,
    pub removed_count: u32,
    pub remaining_count: u32,
    pub destroy_stack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestroyItemCountPlan {
    pub removed_count: u32,
    pub actions: Vec<DestroyItemCountAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroyFilteredItemRef {
    pub bag: u8,
    pub slot: u8,
    pub should_destroy: bool,
}

impl DestroyFilteredItemRef {
    pub const fn new(bag: u8, slot: u8, should_destroy: bool) -> Self {
        Self {
            bag,
            slot,
            should_destroy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroyFilteredItemAction {
    pub bag: u8,
    pub slot: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemPreflightItem {
    pub is_bag: bool,
    pub is_empty_bag: bool,
    pub is_child: bool,
    pub parent_pos: Option<u16>,
    pub can_unequip_result: InventoryResult,
}

impl SwapItemPreflightItem {
    pub const fn regular() -> Self {
        Self {
            is_bag: false,
            is_empty_bag: false,
            is_child: false,
            parent_pos: None,
            can_unequip_result: InventoryResult::Ok,
        }
    }

    pub const fn bag(is_empty_bag: bool) -> Self {
        Self {
            is_bag: true,
            is_empty_bag,
            is_child: false,
            parent_pos: None,
            can_unequip_result: InventoryResult::Ok,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemPreflightResult {
    NoSource,
    ChildRedirect {
        first_src: u16,
        first_dst: u16,
        second_src: u16,
        second_dst: u16,
    },
    Error(InventoryResult),
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemPreflightPlan {
    pub result: SwapItemPreflightResult,
    pub src_unequip_swap: Option<bool>,
    pub dst_unequip_swap: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemEmptyDestinationResult {
    OccupiedDestination,
    InvalidDestinationNoop,
    Error(InventoryResult),
    MoveToInventory {
        quest_added_from_bank: bool,
    },
    MoveToBank {
        quest_removed: bool,
    },
    Equip {
        dest: u16,
        auto_unequip_offhand: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemEmptyDestinationPlan {
    pub result: SwapItemEmptyDestinationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemMergeFillResult {
    ContinueToRealSwap,
    InvalidDestinationNoop,
    MoveMergedStackToInventory,
    MoveMergedStackToBank,
    EquipMergedStack {
        dest: u16,
        auto_unequip_offhand: bool,
    },
    PartialFill {
        source_remaining_count: u32,
        destination_count: u32,
        send_updates: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemMergeFillPlan {
    pub result: SwapItemMergeFillResult,
    pub send_refund_info: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemRealSwapValidationSubject {
    Source,
    Destination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemRealSwapTarget {
    Inventory,
    Bank,
    Equip { dest: u16 },
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemRealSwapValidationResult {
    Error {
        result: InventoryResult,
        subject: SwapItemRealSwapValidationSubject,
    },
    Continue {
        source_target: SwapItemRealSwapTarget,
        destination_target: SwapItemRealSwapTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemRealSwapValidationPlan {
    pub result: SwapItemRealSwapValidationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapBagItemRef {
    pub slot: u8,
    pub can_go_into_empty_bag: bool,
}

impl SwapBagItemRef {
    pub const fn new(slot: u8, can_go_into_empty_bag: bool) -> Self {
        Self {
            slot,
            can_go_into_empty_bag,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapBagRef<'a> {
    pub is_empty: bool,
    pub bag_size: u8,
    pub items: &'a [SwapBagItemRef],
}

impl<'a> SwapBagRef<'a> {
    pub const fn new(is_empty: bool, bag_size: u8, items: &'a [SwapBagItemRef]) -> Self {
        Self {
            is_empty,
            bag_size,
            items,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapBagItemMove {
    pub from_slot: u8,
    pub to_slot: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapItemBagExchangeResult {
    Continue,
    Error(InventoryResult),
    Exchange {
        empty_bag_is_source: bool,
        moves: Vec<SwapBagItemMove>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapItemBagExchangePlan {
    pub result: SwapItemBagExchangeResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapItemRealSwapExecutionPlan {
    pub remove_destination_update: bool,
    pub remove_source_update: bool,
    pub source_target: SwapItemRealSwapTarget,
    pub destination_target: SwapItemRealSwapTarget,
    pub apply_item_dependent_auras: bool,
    pub release_loot: bool,
    pub auto_unequip_offhand: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemErrorItemOrder {
    SourceDestination,
    SourceOnly,
    DestinationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapItemMissingPhase {
    EmptyDestination,
    MergeFill,
    RealSwapValidation,
    BagExchange,
    RealSwapExecution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapItemOrchestrationResult {
    NoSource,
    ChildRedirect {
        first_src: u16,
        first_dst: u16,
        second_src: u16,
        second_dst: u16,
    },
    Error {
        result: InventoryResult,
        item_order: SwapItemErrorItemOrder,
    },
    EmptyDestination(SwapItemEmptyDestinationPlan),
    MergeFill(SwapItemMergeFillPlan),
    RealSwap {
        bag_exchange: SwapItemBagExchangePlan,
        execution: SwapItemRealSwapExecutionPlan,
    },
    InconsistentRealSwapTargets {
        validation_source_target: SwapItemRealSwapTarget,
        validation_destination_target: SwapItemRealSwapTarget,
        execution_source_target: SwapItemRealSwapTarget,
        execution_destination_target: SwapItemRealSwapTarget,
    },
    MissingPhase(SwapItemMissingPhase),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapItemOrchestrationPlan {
    pub result: SwapItemOrchestrationResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulboundTradeableItemRef {
    pub guid: ObjectGuid,
    pub owner_guid: ObjectGuid,
    pub trade_expired: bool,
}

impl SoulboundTradeableItemRef {
    pub const fn new(guid: ObjectGuid, owner_guid: ObjectGuid, trade_expired: bool) -> Self {
        Self {
            guid,
            owner_guid,
            trade_expired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitanGripPenaltyAction {
    None,
    Cast(u32),
    Remove(u32),
}

fn item_ref_by_pos<'a>(items: &'a [ItemSlotRef<'a>], bag: u8, slot: u8) -> Option<&'a Item> {
    items
        .iter()
        .find(|slot_item| slot_item.bag == bag && slot_item.slot == slot)
        .map(|slot_item| slot_item.item)
}

fn bag_template_by_pos<'a>(
    templates: &'a [BagTemplateRef<'a>],
    bag: u8,
) -> Option<&'a ItemStorageTemplate> {
    templates
        .iter()
        .find(|bag_template| bag_template.bag == bag)
        .map(|bag_template| bag_template.template)
}

fn item_storage_ref_by_guid<'a>(
    items: &[ItemStorageRef<'a>],
    guid: ObjectGuid,
) -> Option<ItemStorageRef<'a>> {
    items
        .iter()
        .find(|stored| stored.item.object().guid() == guid)
        .copied()
}

fn cpp_keyring_family_gate_applies(slot: u8) -> bool {
    let keyring_limit =
        i16::from(KEYRING_SLOT_START) + i16::from(KEYRING_SLOT_START) - i16::from(KEYRING_SLOT_END);
    i16::from(slot) >= i16::from(KEYRING_SLOT_START) && i16::from(slot) < keyring_limit
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemSearchLocation: u8 {
        const EQUIPMENT = 0x01;
        const INVENTORY = 0x02;
        const BANK = 0x04;
        const REAGENT_BANK = 0x08;

        const DEFAULT = Self::EQUIPMENT.bits() | Self::INVENTORY.bits();
        const EVERYWHERE = Self::EQUIPMENT.bits() | Self::INVENTORY.bits()
            | Self::BANK.bits() | Self::REAGENT_BANK.bits();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemSearchCallbackResult {
    Stop,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStorageError {
    InvalidPlayerSlot(u8),
    InvalidBagSlot(u8),
    InvalidBagItemSlot(u8),
    UnknownBag(u8),
    EmptyPlayerSlot(u8),
    EmptyBagItemSlot {
        bag: u8,
        slot: u8,
    },
    OccupiedPlayerSlot(u8),
    OccupiedBagItemSlot {
        bag: u8,
        slot: u8,
    },
    MismatchedBagGuid {
        bag: u8,
        expected: ObjectGuid,
        actual: ObjectGuid,
    },
    MismatchedItemGuid {
        slot: u8,
        expected: ObjectGuid,
        actual: ObjectGuid,
    },
    MismatchedBagItemGuid {
        bag: u8,
        slot: u8,
        expected: ObjectGuid,
        actual: ObjectGuid,
    },
    SplitItemLootGenerated,
    InvalidSplitCount {
        available: u32,
        requested: u32,
    },
    TooFewItemsToSplit {
        available: u32,
        requested: u32,
    },
    SplitItemInTrade,
    TopLevelBuybackHiddenFromGetItemByPos(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBagStorage {
    pub bag_guid: ObjectGuid,
    pub bag_size: u8,
    pub slots: [Option<ObjectGuid>; MAX_BAG_SIZE],
}

impl PlayerBagStorage {
    pub fn new(bag_guid: ObjectGuid, bag_size: u8) -> Self {
        assert!(bag_size as usize <= MAX_BAG_SIZE);
        Self {
            bag_guid,
            bag_size,
            slots: [None; MAX_BAG_SIZE],
        }
    }

    pub fn item_by_pos(&self, slot: u8) -> Option<ObjectGuid> {
        if slot < self.bag_size {
            self.slots[slot as usize]
        } else {
            None
        }
    }

    pub fn set_item(&mut self, slot: u8, guid: Option<ObjectGuid>) {
        assert!((slot as usize) < MAX_BAG_SIZE);
        self.slots[slot as usize] = guid;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerInventoryStorage {
    pub items: [Option<ObjectGuid>; PLAYER_SLOT_END],
    pub bags: [Option<PlayerBagStorage>; PLAYER_SLOT_END],
    pub current_buyback_slot: u8,
}

impl PlayerInventoryStorage {
    pub fn get_item_by_guid_everywhere(&self, guid: ObjectGuid) -> Option<ObjectGuid> {
        self.items
            .iter()
            .enumerate()
            .filter(|(slot, _)| !is_buyback_slot(*slot as u8))
            .find_map(|(_, item_guid)| (*item_guid == Some(guid)).then_some(guid))
            .or_else(|| {
                self.bags
                    .iter()
                    .filter_map(|bag| *bag)
                    .flat_map(|bag| bag.slots.into_iter().take(bag.bag_size as usize))
                    .find_map(|item_guid| (item_guid == Some(guid)).then_some(guid))
            })
    }
}

impl Default for PlayerInventoryStorage {
    fn default() -> Self {
        Self {
            items: [None; PLAYER_SLOT_END],
            bags: [None; PLAYER_SLOT_END],
            current_buyback_slot: BUYBACK_SLOT_START,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VisibleItemValues {
    pub item_id: i32,
    pub item_appearance_mod_id: u16,
    pub item_visual: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerDataValues {
    pub loot_target_guid: ObjectGuid,
    pub player_flags: u32,
    pub player_flags_ex: u32,
    pub num_bank_slots: u8,
    pub native_sex: u8,
    pub current_spec_id: u32,
    pub visible_items: [VisibleItemValues; EQUIPMENT_SLOT_END as usize],
}

impl Default for PlayerDataValues {
    fn default() -> Self {
        Self {
            loot_target_guid: ObjectGuid::EMPTY,
            player_flags: 0,
            player_flags_ex: 0,
            num_bank_slots: 0,
            native_sex: Gender::Male as u8,
            current_spec_id: 0,
            visible_items: [VisibleItemValues::default(); EQUIPMENT_SLOT_END as usize],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivePlayerDataValues {
    pub coinage: u64,
    pub xp: i32,
    pub next_level_xp: i32,
    pub character_points: i32,
    pub num_backpack_slots: u8,
    pub inv_slots: [ObjectGuid; PLAYER_SLOT_END],
    pub buyback_price: [u32; BUYBACK_SLOT_COUNT],
    pub buyback_timestamp: [i64; BUYBACK_SLOT_COUNT],
}

impl Default for ActivePlayerDataValues {
    fn default() -> Self {
        Self {
            coinage: 0,
            xp: 0,
            next_level_xp: 0,
            character_points: 0,
            num_backpack_slots: 0,
            inv_slots: [ObjectGuid::EMPTY; PLAYER_SLOT_END],
            buyback_price: [0; BUYBACK_SLOT_COUNT],
            buyback_timestamp: [0; BUYBACK_SLOT_COUNT],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerDataUpdate {
    pub mask: UpdateMask,
    pub values: PlayerDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivePlayerDataUpdate {
    pub mask: UpdateMask,
    pub values: ActivePlayerDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub unit_data: Option<UnitDataUpdate>,
    pub player_data: Option<PlayerDataUpdate>,
    pub active_player_data: Option<ActivePlayerDataUpdate>,
}

impl PlayerValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    unit: Unit,
    session_id: Option<u64>,
    data: PlayerDataValues,
    active_data: ActivePlayerDataValues,
    inventory: PlayerInventoryStorage,
    player_data_changes: UpdateMask,
    active_player_data_changes: UpdateMask,
    mod_melee_hit_chance: f32,
    mod_ranged_hit_chance: f32,
    mod_spell_hit_chance: f32,
    ingame_time: u32,
    shared_quest_id: u32,
    extra_flags: u32,
    team: u8,
    is_active: bool,
    controlled_by_player: bool,
    accept_whispers: bool,
    can_titan_grip: bool,
    titan_grip_penalty_spell_id: u32,
    soulbound_tradeable_items: HashSet<ObjectGuid>,
}

impl Player {
    pub fn new(session_id: Option<u64>, can_filter_whispers: bool) -> Self {
        let mut unit = Unit::new(true);
        unit.set_type(
            TypeId::Player,
            TypeMask::OBJECT | TypeMask::UNIT | TypeMask::PLAYER,
        );

        Self {
            unit,
            session_id,
            data: PlayerDataValues::default(),
            active_data: ActivePlayerDataValues::default(),
            inventory: PlayerInventoryStorage::default(),
            player_data_changes: UpdateMask::new(PLAYER_DATA_BITS),
            active_player_data_changes: UpdateMask::new(ACTIVE_PLAYER_DATA_BITS),
            mod_melee_hit_chance: 7.5,
            mod_ranged_hit_chance: 7.5,
            mod_spell_hit_chance: 15.0,
            ingame_time: 0,
            shared_quest_id: 0,
            extra_flags: 0,
            team: TEAM_OTHER,
            is_active: true,
            controlled_by_player: true,
            accept_whispers: !can_filter_whispers,
            can_titan_grip: false,
            titan_grip_penalty_spell_id: 0,
            soulbound_tradeable_items: HashSet::new(),
        }
    }

    pub const fn unit(&self) -> &Unit {
        &self.unit
    }

    pub fn unit_mut(&mut self) -> &mut Unit {
        &mut self.unit
    }

    pub const fn guid(&self) -> ObjectGuid {
        self.unit.world().object().guid()
    }

    pub const fn session_id(&self) -> Option<u64> {
        self.session_id
    }

    pub fn bind_session(&mut self, session_id: Option<u64>) {
        self.session_id = session_id;
    }

    pub const fn data(&self) -> &PlayerDataValues {
        &self.data
    }

    pub const fn active_data(&self) -> &ActivePlayerDataValues {
        &self.active_data
    }

    pub const fn inventory(&self) -> &PlayerInventoryStorage {
        &self.inventory
    }

    pub fn soulbound_tradeable_items(&self) -> &HashSet<ObjectGuid> {
        &self.soulbound_tradeable_items
    }

    pub const fn hit_chances(&self) -> (f32, f32, f32) {
        (
            self.mod_melee_hit_chance,
            self.mod_ranged_hit_chance,
            self.mod_spell_hit_chance,
        )
    }

    pub const fn team(&self) -> u8 {
        self.team
    }

    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    pub const fn controlled_by_player(&self) -> bool {
        self.controlled_by_player
    }

    pub const fn accept_whispers(&self) -> bool {
        self.accept_whispers
    }

    pub const fn ingame_time(&self) -> u32 {
        self.ingame_time
    }

    pub const fn shared_quest_id(&self) -> u32 {
        self.shared_quest_id
    }

    pub const fn extra_flags(&self) -> u32 {
        self.extra_flags
    }

    pub fn player_data_changes_mask(&self) -> &UpdateMask {
        &self.player_data_changes
    }

    pub fn active_player_data_changes_mask(&self) -> &UpdateMask {
        &self.active_player_data_changes
    }

    pub fn clear_player_data_changes(&mut self) {
        self.player_data_changes.reset_all();
    }

    pub fn clear_active_player_data_changes(&mut self) {
        self.active_player_data_changes.reset_all();
    }

    pub fn clear_data_changes(&mut self) {
        self.clear_player_data_changes();
        self.clear_active_player_data_changes();
        self.unit.clear_unit_data_changes();
        self.unit.world_mut().object_mut().clear_update_mask(false);
    }

    pub fn set_selection(&mut self, guid: ObjectGuid) {
        self.unit.set_target(guid);
    }

    pub fn set_race_class_gender(&mut self, race: u8, class_id: u8, gender: Gender) {
        self.unit.set_race(race);
        self.unit.set_class(class_id);
        self.unit.set_player_class(class_id);
        self.unit.set_gender(gender);
        self.set_native_gender(gender);
    }

    pub fn set_native_gender(&mut self, gender: Gender) {
        self.set_player_u8(PLAYER_DATA_NATIVE_SEX_BIT, gender as u8, |data| {
            &mut data.native_sex
        });
    }

    pub fn replace_all_player_flags(&mut self, flags: u32) {
        self.set_player_u32(PLAYER_DATA_FLAGS_BIT, flags, |data| &mut data.player_flags);
    }

    pub fn set_player_flag(&mut self, flag: u32) {
        self.replace_all_player_flags(self.data.player_flags | flag);
    }

    pub fn remove_player_flag(&mut self, flag: u32) {
        self.replace_all_player_flags(self.data.player_flags & !flag);
    }

    pub fn has_player_flag(&self, flag: u32) -> bool {
        (self.data.player_flags & flag) != 0
    }

    pub fn replace_all_player_flags_ex(&mut self, flags: u32) {
        self.set_player_u32(PLAYER_DATA_FLAGS_EX_BIT, flags, |data| {
            &mut data.player_flags_ex
        });
    }

    pub fn set_player_flag_ex(&mut self, flag: u32) {
        self.replace_all_player_flags_ex(self.data.player_flags_ex | flag);
    }

    pub fn remove_player_flag_ex(&mut self, flag: u32) {
        self.replace_all_player_flags_ex(self.data.player_flags_ex & !flag);
    }

    pub fn has_player_flag_ex(&self, flag: u32) -> bool {
        (self.data.player_flags_ex & flag) != 0
    }

    pub fn set_loot_guid(&mut self, guid: ObjectGuid) {
        self.set_player_guid(PLAYER_DATA_LOOT_TARGET_GUID_BIT, guid, |data| {
            &mut data.loot_target_guid
        });
    }

    pub fn set_bank_bag_slot_count(&mut self, count: u8) {
        self.set_player_u8(PLAYER_DATA_NUM_BANK_SLOTS_BIT, count, |data| {
            &mut data.num_bank_slots
        });
    }

    pub fn set_primary_specialization(&mut self, spec: u32) {
        self.set_player_u32(PLAYER_DATA_CURRENT_SPEC_ID_BIT, spec, |data| {
            &mut data.current_spec_id
        });
    }

    pub fn set_visible_item_slot(&mut self, slot: u8, item: Option<VisibleItemValues>) {
        if slot >= EQUIPMENT_SLOT_END {
            return;
        }

        let value = item.unwrap_or_default();
        let target = &mut self.data.visible_items[slot as usize];
        if *target != value {
            *target = value;
            self.mark_player_data_array(
                PLAYER_DATA_VISIBLE_ITEMS_PARENT_BIT,
                PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT,
                slot as usize,
            );
        }
    }

    pub fn set_money(&mut self, value: u64) {
        self.set_active_u64(ACTIVE_PLAYER_DATA_COINAGE_BIT, value, |data| {
            &mut data.coinage
        });
    }

    pub fn modify_money(&mut self, amount: i64) -> bool {
        if amount == 0 {
            return true;
        }

        if amount < 0 {
            self.set_money(
                self.active_data
                    .coinage
                    .saturating_sub(amount.unsigned_abs()),
            );
            return true;
        }

        let amount = amount as u64;
        if amount <= MAX_MONEY_AMOUNT && self.active_data.coinage <= MAX_MONEY_AMOUNT - amount {
            self.set_money(self.active_data.coinage + amount);
            true
        } else {
            false
        }
    }

    pub fn set_xp(&mut self, xp: i32) {
        self.set_active_i32(ACTIVE_PLAYER_DATA_XP_BIT, xp, |data| &mut data.xp);
    }

    pub fn set_next_level_xp(&mut self, xp: i32) {
        self.set_active_i32(ACTIVE_PLAYER_DATA_NEXT_LEVEL_XP_BIT, xp, |data| {
            &mut data.next_level_xp
        });
    }

    pub fn set_free_primary_professions(&mut self, points: u16) {
        self.set_active_i32(
            ACTIVE_PLAYER_DATA_CHARACTER_POINTS_BIT,
            i32::from(points),
            |data| &mut data.character_points,
        );
    }

    pub fn set_inventory_slot_count(&mut self, count: u8) {
        self.set_active_u8(ACTIVE_PLAYER_DATA_NUM_BACKPACK_SLOTS_BIT, count, |data| {
            &mut data.num_backpack_slots
        });
    }

    pub fn set_inv_slot(&mut self, slot: usize, guid: ObjectGuid) {
        if slot >= PLAYER_SLOT_END || self.active_data.inv_slots[slot] == guid {
            return;
        }

        self.active_data.inv_slots[slot] = guid;
        self.mark_active_player_data_array(
            ACTIVE_PLAYER_DATA_INV_SLOTS_PARENT_BIT,
            ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT,
            slot,
        );
    }

    pub fn is_valid_pos(&self, bag: u8, slot: u8, explicit_pos: bool) -> bool {
        if bag == NULL_BAG && !explicit_pos {
            return true;
        }

        if bag == INVENTORY_SLOT_BAG_0 {
            if slot == NULL_SLOT && !explicit_pos {
                return true;
            }
            if slot < EQUIPMENT_SLOT_END {
                return true;
            }
            if (PROFESSION_SLOT_START..PROFESSION_SLOT_END).contains(&slot) {
                return true;
            }
            if (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot) {
                return true;
            }
            if (REAGENT_BAG_SLOT_START..REAGENT_BAG_SLOT_END).contains(&slot) {
                return true;
            }
            let backpack_end = INVENTORY_SLOT_ITEM_START
                .saturating_add(self.active_data.num_backpack_slots)
                .min(INVENTORY_SLOT_ITEM_END);
            if (INVENTORY_SLOT_ITEM_START..backpack_end).contains(&slot) {
                return true;
            }
            if (BANK_SLOT_ITEM_START..BANK_SLOT_ITEM_END).contains(&slot) {
                return true;
            }
            if (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&slot) {
                return true;
            }
            if (KEYRING_SLOT_START..KEYRING_SLOT_END).contains(&slot) {
                return true;
            }
            return false;
        }

        let Some(bag_storage) = self
            .inventory
            .bags
            .get(bag as usize)
            .and_then(Option::as_ref)
        else {
            return false;
        };

        if slot == NULL_SLOT && !explicit_pos {
            return true;
        }

        slot < bag_storage.bag_size
    }

    pub fn is_valid_packed_pos(&self, pos: u16, explicit_pos: bool) -> bool {
        let [bag, slot] = pos.to_be_bytes();
        self.is_valid_pos(bag, slot, explicit_pos)
    }

    pub fn can_store_item_in_specific_slot(
        &self,
        bag: u8,
        slot: u8,
        dest: &mut Vec<ItemPosCount>,
        proto: &ItemStorageTemplate,
        count: &mut u32,
        swap: bool,
        existing_item: Option<&Item>,
        source_item: Option<&Item>,
        source_is_not_empty_bag: bool,
        bag_proto: Option<&ItemStorageTemplate>,
    ) -> InventoryResult {
        let existing_item = existing_item.filter(|existing| {
            source_item.is_none_or(|source| existing.object().guid() != source.object().guid())
        });

        if let Some(source) = source_item {
            if source_is_not_empty_bag && !is_bag_pos(make_item_pos(bag, slot)) {
                return InventoryResult::DestroyNonemptyBag;
            }

            let source_is_child = source.has_item_flag(ItemFieldFlags::CHILD);
            if source_is_child && !is_equipment_pos(bag, slot) && !is_child_equipment_pos(bag, slot)
            {
                return InventoryResult::WrongBagType3;
            }
            if !source_is_child && is_child_equipment_pos(bag, slot) {
                return InventoryResult::WrongBagType3;
            }
        }

        let need_space = if existing_item.is_none() || swap {
            if slot == REAGENT_BAG_SLOT_START {
                return InventoryResult::WrongBagType;
            }

            if bag == INVENTORY_SLOT_BAG_0 {
                if cpp_keyring_family_gate_applies(slot)
                    && !proto.bag_family.contains(BagFamilyMask::KEYS)
                {
                    return InventoryResult::WrongBagType;
                }

                if (BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&slot)
                    || slot as usize >= PLAYER_SLOT_END
                {
                    return InventoryResult::WrongBagType;
                }
            } else {
                if self.get_bag_by_pos(bag).is_none() {
                    return InventoryResult::WrongBagType;
                }

                let Some(bag_proto) = bag_proto else {
                    return InventoryResult::WrongBagType;
                };

                if slot >= bag_proto.container_slots {
                    return InventoryResult::WrongBagType;
                }

                if !item_can_go_into_bag(proto, bag_proto) {
                    return InventoryResult::WrongBagType;
                }
            }

            proto.max_stack_size
        } else {
            let existing_item = existing_item.expect("checked Some above");
            let result = existing_item.can_be_merged_partly_with(proto.entry, proto.max_stack_size);
            if result != InventoryResult::Ok {
                return result;
            }

            proto.max_stack_size - existing_item.count()
        };

        let need_space = need_space.min(*count);
        let new_position = ItemPosCount::new(make_item_pos(bag, slot), need_space);
        if !new_position.is_contained_in(dest) {
            dest.push(new_position);
            *count -= need_space;
        }

        InventoryResult::Ok
    }

    pub fn can_store_item_in_inventory_slots(
        &self,
        slot_begin: u8,
        slot_end: u8,
        dest: &mut Vec<ItemPosCount>,
        proto: &ItemStorageTemplate,
        count: &mut u32,
        merge: bool,
        source_item: Option<&Item>,
        source_is_not_empty_bag: bool,
        skip_bag: u8,
        skip_slot: u8,
        slot_items: &[ItemSlotRef<'_>],
    ) -> InventoryResult {
        if source_item.is_some() && source_is_not_empty_bag {
            return InventoryResult::DestroyNonemptyBag;
        }

        for slot in slot_begin..slot_end {
            if skip_bag == INVENTORY_SLOT_BAG_0 && slot == skip_slot {
                continue;
            }

            if slot == REAGENT_BAG_SLOT_START {
                continue;
            }

            let existing_item =
                item_ref_by_pos(slot_items, INVENTORY_SLOT_BAG_0, slot).filter(|existing| {
                    source_item
                        .is_none_or(|source| existing.object().guid() != source.object().guid())
                });

            if existing_item.is_some() != merge {
                continue;
            }

            let mut need_space = proto.max_stack_size;
            if let Some(existing_item) = existing_item {
                if existing_item.can_be_merged_partly_with(proto.entry, proto.max_stack_size)
                    != InventoryResult::Ok
                {
                    continue;
                }

                need_space -= existing_item.count();
            }

            need_space = need_space.min(*count);
            let new_position =
                ItemPosCount::new(make_item_pos(INVENTORY_SLOT_BAG_0, slot), need_space);
            if !new_position.is_contained_in(dest) {
                dest.push(new_position);
                *count -= need_space;

                if *count == 0 {
                    return InventoryResult::Ok;
                }
            }
        }

        InventoryResult::Ok
    }

    pub fn can_store_item_in_bag(
        &self,
        bag: u8,
        dest: &mut Vec<ItemPosCount>,
        proto: &ItemStorageTemplate,
        count: &mut u32,
        merge: bool,
        non_specialized: bool,
        source_item: Option<&Item>,
        source_is_not_empty_bag: bool,
        skip_bag: u8,
        skip_slot: u8,
        bag_proto: Option<&ItemStorageTemplate>,
        slot_items: &[ItemSlotRef<'_>],
    ) -> InventoryResult {
        if bag == skip_bag {
            return InventoryResult::WrongBagType;
        }

        let Some(bag_storage) = self
            .inventory
            .bags
            .get(bag as usize)
            .and_then(Option::as_ref)
        else {
            return InventoryResult::WrongBagType;
        };

        if source_item.is_some_and(|source| source.object().guid() == bag_storage.bag_guid) {
            return InventoryResult::WrongBagType;
        }

        if let Some(source) = source_item {
            if source_is_not_empty_bag {
                return InventoryResult::DestroyNonemptyBag;
            }

            if source.has_item_flag(ItemFieldFlags::CHILD) {
                return InventoryResult::WrongBagType3;
            }
        }

        let Some(bag_proto) = bag_proto else {
            return InventoryResult::WrongBagType;
        };

        let bag_is_regular_container = bag_proto.class_id == ItemClass::Container
            && bag_proto.subclass_id == ItemSubClassContainer::Container as u32;
        if non_specialized != bag_is_regular_container {
            return InventoryResult::WrongBagType;
        }

        if !item_can_go_into_bag(proto, bag_proto) {
            return InventoryResult::WrongBagType;
        }

        for slot in 0..bag_storage.bag_size {
            if slot == skip_slot {
                continue;
            }

            let existing_item = item_ref_by_pos(slot_items, bag, slot).filter(|existing| {
                source_item.is_none_or(|source| existing.object().guid() != source.object().guid())
            });

            if existing_item.is_some() != merge {
                continue;
            }

            let mut need_space = proto.max_stack_size;
            if let Some(existing_item) = existing_item {
                if existing_item.can_be_merged_partly_with(proto.entry, proto.max_stack_size)
                    != InventoryResult::Ok
                {
                    continue;
                }

                need_space -= existing_item.count();
            }

            need_space = need_space.min(*count);
            let new_position = ItemPosCount::new(make_item_pos(bag, slot), need_space);
            if !new_position.is_contained_in(dest) {
                dest.push(new_position);
                *count -= need_space;

                if *count == 0 {
                    return InventoryResult::Ok;
                }
            }
        }

        InventoryResult::Ok
    }

    pub fn can_take_more_similar_items(
        &self,
        args: CanTakeMoreSimilarItemsArgs<'_>,
    ) -> CanTakeMoreSimilarItemsOutcome {
        let Some(proto) = args.proto else {
            return CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::ItemMaxCount,
                no_space_count: Some(args.count),
                offending_item_id: None,
            };
        };

        if args.source_item.is_some_and(Item::loot_generated) {
            return CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::LootGone,
                no_space_count: None,
                offending_item_id: None,
            };
        }

        if (proto.max_count <= 0 && proto.item_limit_category == 0) || proto.max_count == i32::MAX {
            return can_take_more_similar_ok();
        }

        if proto.max_count > 0 {
            let max_count = proto.max_count as u32;
            if args.current_item_count.saturating_add(args.count) > max_count {
                return CanTakeMoreSimilarItemsOutcome {
                    result: InventoryResult::ItemMaxCount,
                    no_space_count: Some(
                        args.current_item_count
                            .saturating_add(args.count)
                            .saturating_sub(max_count),
                    ),
                    offending_item_id: None,
                };
            }
        }

        if proto.item_limit_category != 0 {
            let Some(limit_category) = args.limit_category else {
                return CanTakeMoreSimilarItemsOutcome {
                    result: InventoryResult::NotEquippable,
                    no_space_count: Some(args.count),
                    offending_item_id: None,
                };
            };

            if limit_category.flags == ITEM_LIMIT_CATEGORY_MODE_HAVE {
                let limit_quantity = u32::from(limit_category.quantity);
                if args.current_limit_category_count.saturating_add(args.count) > limit_quantity {
                    return CanTakeMoreSimilarItemsOutcome {
                        result: InventoryResult::ItemMaxLimitCategoryCountExceededIs,
                        no_space_count: Some(
                            args.current_limit_category_count
                                .saturating_add(args.count)
                                .saturating_sub(limit_quantity),
                        ),
                        offending_item_id: Some(proto.entry),
                    };
                }
            }
        }

        can_take_more_similar_ok()
    }

    pub fn item_count_by_entry(
        &self,
        entry: u32,
        in_bank_also: bool,
        skip_item: Option<&Item>,
        stored_items: &[ItemStorageRef<'_>],
    ) -> u32 {
        stored_items
            .iter()
            .filter(|stored| {
                is_equipment_pos(stored.bag, stored.slot)
                    || is_inventory_pos(stored.bag, stored.slot)
                    || (in_bank_also && is_bank_pos(stored.bag, stored.slot))
            })
            .filter(|stored| {
                skip_item.is_none_or(|skip| stored.item.object().guid() != skip.object().guid())
            })
            .filter(|stored| stored.item.object().entry() == entry)
            .map(|stored| stored.item.count())
            .sum()
    }

    pub fn item_count_with_limit_category(
        &self,
        limit_category: u32,
        skip_item: Option<&Item>,
        stored_items: &[ItemStorageRef<'_>],
    ) -> u32 {
        stored_items
            .iter()
            .filter(|stored| {
                skip_item.is_none_or(|skip| stored.item.object().guid() != skip.object().guid())
            })
            .filter(|stored| {
                stored
                    .template
                    .is_some_and(|template| template.item_limit_category == limit_category)
            })
            .map(|stored| stored.item.count())
            .sum()
    }

    pub fn item_by_entry<'a>(
        &self,
        entry: u32,
        location: ItemSearchLocation,
        stored_items: &'a [ItemStorageRef<'a>],
    ) -> Option<ItemStorageRef<'a>> {
        let mut result = None;
        self.for_each_item_storage_ref(location, stored_items, |stored| {
            if stored.item.object().entry() == entry {
                result = Some(stored);
                ItemSearchCallbackResult::Stop
            } else {
                ItemSearchCallbackResult::Continue
            }
        });
        result
    }

    pub fn item_list_by_entry<'a>(
        &self,
        entry: u32,
        in_bank_also: bool,
        stored_items: &'a [ItemStorageRef<'a>],
    ) -> Vec<ItemStorageRef<'a>> {
        let mut location = ItemSearchLocation::EQUIPMENT
            | ItemSearchLocation::INVENTORY
            | ItemSearchLocation::REAGENT_BANK;
        if in_bank_also {
            location |= ItemSearchLocation::BANK;
        }

        let mut item_list = Vec::new();
        self.for_each_item_storage_ref(location, stored_items, |stored| {
            if stored.item.object().entry() == entry {
                item_list.push(stored);
            }
            ItemSearchCallbackResult::Continue
        });
        item_list
    }

    pub fn can_store_item(
        &self,
        dest: &mut Vec<ItemPosCount>,
        args: CanStoreItemArgs<'_>,
    ) -> CanStoreItemOutcome {
        let Some(proto) = args.proto else {
            return can_store_item_error(
                if args.swap {
                    InventoryResult::CantSwap
                } else {
                    InventoryResult::ItemNotFound
                },
                args.count,
                0,
            );
        };

        if let Some(source) = args.source_item {
            if source.loot_generated() {
                return can_store_item_error(InventoryResult::LootGone, args.count, 0);
            }

            if source.is_binded_not_with(
                self.guid(),
                proto,
                args.source_bop_trade_allowed_for_player,
            ) {
                return can_store_item_error(InventoryResult::NotOwner, args.count, 0);
            }
        }

        let mut count = args.count;
        let similar_result = self.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
            proto: args.proto,
            count,
            source_item: args.source_item,
            current_item_count: self.item_count_by_entry(
                proto.entry,
                true,
                args.source_item,
                args.stored_items,
            ),
            limit_category: args.limit_category,
            current_limit_category_count: self.item_count_with_limit_category(
                proto.item_limit_category,
                args.source_item,
                args.stored_items,
            ),
        });
        let no_similar_count = if similar_result.result == InventoryResult::Ok {
            0
        } else {
            let no_similar_count = similar_result.no_space_count.unwrap_or(0);
            if count == no_similar_count {
                return can_store_item_error(similar_result.result, no_similar_count, 0);
            }
            count -= no_similar_count;
            no_similar_count
        };

        if args.bag != NULL_BAG && args.slot != NULL_SLOT {
            let result = self.can_store_item_in_specific_slot(
                args.bag,
                args.slot,
                dest,
                proto,
                &mut count,
                args.swap,
                item_ref_by_pos(args.slot_items, args.bag, args.slot),
                args.source_item,
                args.source_is_not_empty_bag,
                bag_template_by_pos(args.bag_templates, args.bag),
            );
            if result != InventoryResult::Ok {
                return can_store_item_error(result, count, no_similar_count);
            }

            if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                return outcome;
            }
        }

        let inventory_end = INVENTORY_SLOT_ITEM_START
            .saturating_add(self.active_data.num_backpack_slots)
            .min(INVENTORY_SLOT_ITEM_END);

        if args.bag != NULL_BAG {
            if proto.max_stack_size != 1 {
                if args.bag == INVENTORY_SLOT_BAG_0 {
                    let result = self.can_store_item_in_inventory_slots(
                        CHILD_EQUIPMENT_SLOT_START,
                        CHILD_EQUIPMENT_SLOT_END,
                        dest,
                        proto,
                        &mut count,
                        true,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        return can_store_item_error(result, count, no_similar_count);
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }

                    let result = self.can_store_item_in_inventory_slots(
                        INVENTORY_SLOT_ITEM_START,
                        inventory_end,
                        dest,
                        proto,
                        &mut count,
                        true,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        return can_store_item_error(result, count, no_similar_count);
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }
                } else {
                    let mut result = self.can_store_item_in_bag(
                        args.bag,
                        dest,
                        proto,
                        &mut count,
                        true,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        NULL_BAG,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, args.bag),
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        result = self.can_store_item_in_bag(
                            args.bag,
                            dest,
                            proto,
                            &mut count,
                            true,
                            true,
                            args.source_item,
                            args.source_is_not_empty_bag,
                            NULL_BAG,
                            args.slot,
                            bag_template_by_pos(args.bag_templates, args.bag),
                            args.slot_items,
                        );
                    }
                    if result != InventoryResult::Ok {
                        return can_store_item_error(result, count, no_similar_count);
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }
                }
            }

            if args.bag == INVENTORY_SLOT_BAG_0 {
                if proto.bag_family.contains(BagFamilyMask::KEYS) {
                    let result = self.can_store_item_in_inventory_slots(
                        KEYRING_SLOT_START,
                        KEYRING_SLOT_END,
                        dest,
                        proto,
                        &mut count,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        return can_store_item_error(result, count, no_similar_count);
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }
                }

                if args
                    .source_item
                    .is_some_and(|source| source.has_item_flag(ItemFieldFlags::CHILD))
                {
                    let result = self.can_store_item_in_inventory_slots(
                        CHILD_EQUIPMENT_SLOT_START,
                        CHILD_EQUIPMENT_SLOT_END,
                        dest,
                        proto,
                        &mut count,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        return can_store_item_error(result, count, no_similar_count);
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }
                }

                let result = self.can_store_item_in_inventory_slots(
                    INVENTORY_SLOT_ITEM_START,
                    inventory_end,
                    dest,
                    proto,
                    &mut count,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    return can_store_item_error(result, count, no_similar_count);
                }
                if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                    return outcome;
                }
            } else {
                let mut result = self.can_store_item_in_bag(
                    args.bag,
                    dest,
                    proto,
                    &mut count,
                    false,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    NULL_BAG,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, args.bag),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    result = self.can_store_item_in_bag(
                        args.bag,
                        dest,
                        proto,
                        &mut count,
                        false,
                        true,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        NULL_BAG,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, args.bag),
                        args.slot_items,
                    );
                }
                if result != InventoryResult::Ok {
                    return can_store_item_error(result, count, no_similar_count);
                }
                if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                    return outcome;
                }
            }
        }

        if proto.max_stack_size != 1 {
            let result = self.can_store_item_in_inventory_slots(
                CHILD_EQUIPMENT_SLOT_START,
                CHILD_EQUIPMENT_SLOT_END,
                dest,
                proto,
                &mut count,
                true,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                return can_store_item_error(result, count, no_similar_count);
            }
            if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                return outcome;
            }

            let result = self.can_store_item_in_inventory_slots(
                INVENTORY_SLOT_ITEM_START,
                inventory_end,
                dest,
                proto,
                &mut count,
                true,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                return can_store_item_error(result, count, no_similar_count);
            }
            if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                return outcome;
            }

            if !proto.bag_family.is_empty() {
                for bag_slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
                    let result = self.can_store_item_in_bag(
                        bag_slot,
                        dest,
                        proto,
                        &mut count,
                        true,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, bag_slot),
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        continue;
                    }
                    if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                        return outcome;
                    }
                }
            }

            for bag_slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
                let result = self.can_store_item_in_bag(
                    bag_slot,
                    dest,
                    proto,
                    &mut count,
                    true,
                    true,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, bag_slot),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    continue;
                }
                if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                    return outcome;
                }
            }
        }

        if !proto.bag_family.is_empty() {
            if proto.bag_family.contains(BagFamilyMask::KEYS) {
                let result = self.can_store_item_in_inventory_slots(
                    KEYRING_SLOT_START,
                    KEYRING_SLOT_END,
                    dest,
                    proto,
                    &mut count,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    return can_store_item_error(result, count, no_similar_count);
                }
                if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                    return outcome;
                }
            }

            for bag_slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
                let result = self.can_store_item_in_bag(
                    bag_slot,
                    dest,
                    proto,
                    &mut count,
                    false,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, bag_slot),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    continue;
                }
                if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                    return outcome;
                }
            }
        }

        if args.source_is_not_empty_bag {
            return CanStoreItemOutcome {
                result: InventoryResult::BagInBag,
                no_space_count: None,
            };
        }

        if args
            .source_item
            .is_some_and(|source| source.has_item_flag(ItemFieldFlags::CHILD))
        {
            let result = self.can_store_item_in_inventory_slots(
                CHILD_EQUIPMENT_SLOT_START,
                CHILD_EQUIPMENT_SLOT_END,
                dest,
                proto,
                &mut count,
                false,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                return can_store_item_error(result, count, no_similar_count);
            }
            if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                return outcome;
            }
        }

        let mut search_slot_start = INVENTORY_SLOT_ITEM_START;
        if args.source_item.is_none()
            && proto.class_id == ItemClass::Container
            && proto.subclass_id == ItemSubClassContainer::Container as u32
            && matches!(
                proto.bonding,
                ItemBondingType::None | ItemBondingType::OnAcquire
            )
        {
            search_slot_start = INVENTORY_SLOT_BAG_START;
        }

        let result = self.can_store_item_in_inventory_slots(
            search_slot_start,
            inventory_end,
            dest,
            proto,
            &mut count,
            false,
            args.source_item,
            args.source_is_not_empty_bag,
            args.bag,
            args.slot,
            args.slot_items,
        );
        if result != InventoryResult::Ok {
            return can_store_item_error(result, count, no_similar_count);
        }
        if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
            return outcome;
        }

        for bag_slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
            let result = self.can_store_item_in_bag(
                bag_slot,
                dest,
                proto,
                &mut count,
                false,
                true,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                bag_template_by_pos(args.bag_templates, bag_slot),
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                continue;
            }
            if let Some(outcome) = can_store_item_count_zero(count, no_similar_count) {
                return outcome;
            }
        }

        can_store_item_error(InventoryResult::InvFull, count, no_similar_count)
    }

    pub fn find_equip_slot(&self, args: FindEquipSlotArgs<'_>) -> u8 {
        let slots = equip_slot_candidates(args);
        if slots[0] == NULL_SLOT {
            return NULL_SLOT;
        }

        if args.slot != NULL_SLOT {
            if args.swap
                || item_ref_by_pos(args.equipped_items, INVENTORY_SLOT_BAG_0, args.slot).is_none()
            {
                for candidate in slots {
                    if candidate == args.slot {
                        return args.slot;
                    }
                }
            }
        } else {
            for candidate in slots {
                if candidate != NULL_SLOT
                    && item_ref_by_pos(args.equipped_items, INVENTORY_SLOT_BAG_0, candidate)
                        .is_none()
                    && (candidate != EQUIPMENT_SLOT_OFFHAND || !args.is_two_hand_used)
                {
                    return candidate;
                }
            }

            if args.swap {
                let mut min_item_level = u32::MAX;
                let mut min_item_level_index = 0usize;
                for (index, candidate) in slots.into_iter().enumerate() {
                    if candidate == NULL_SLOT {
                        continue;
                    }

                    if let Some(equipped) =
                        item_ref_by_pos(args.equipped_items, INVENTORY_SLOT_BAG_0, candidate)
                    {
                        let item_level = u32::from(equipped.data().debug_item_level);
                        if item_level < min_item_level {
                            min_item_level = item_level;
                            min_item_level_index = index;
                        }
                    }
                }

                return slots[min_item_level_index];
            }
        }

        NULL_SLOT
    }

    pub fn can_equip_item(&self, args: CanEquipItemArgs<'_>) -> CanEquipItemOutcome {
        let Some(source) = args.source_item else {
            return can_equip_item_outcome(if args.swap {
                InventoryResult::CantSwap
            } else {
                InventoryResult::ItemNotFound
            });
        };

        let Some(proto) = args.proto else {
            return can_equip_item_outcome(if args.swap {
                InventoryResult::CantSwap
            } else {
                InventoryResult::ItemNotFound
            });
        };

        if source.loot_generated() {
            return can_equip_item_outcome(InventoryResult::LootGone);
        }

        if source.is_binded_not_with(self.guid(), proto, args.source_bop_trade_allowed_for_player) {
            return can_equip_item_outcome(InventoryResult::NotOwner);
        }

        let similar_result = self.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
            proto: args.proto,
            count: source.count(),
            source_item: args.source_item,
            current_item_count: self.item_count_by_entry(
                proto.entry,
                false,
                args.source_item,
                args.stored_items,
            ),
            limit_category: args.limit_category,
            current_limit_category_count: self.item_count_with_limit_category(
                proto.item_limit_category,
                args.source_item,
                args.stored_items,
            ),
        });
        if similar_result.result != InventoryResult::Ok {
            return can_equip_item_outcome(similar_result.result);
        }

        if args.not_loading {
            if args.is_stunned {
                return can_equip_item_outcome(InventoryResult::GenericStunned);
            }

            if args.is_charmed {
                return can_equip_item_outcome(InventoryResult::ClientLockedOut);
            }

            if !proto.can_change_equip_state_in_combat() {
                if args.is_in_combat {
                    return can_equip_item_outcome(InventoryResult::NotInCombat);
                }

                if args.is_in_progress_arena {
                    return can_equip_item_outcome(InventoryResult::NotDuringArenaMatch);
                }
            }

            if args.is_in_combat
                && (proto.class_id == ItemClass::Weapon
                    || proto.inventory_type == InventoryType::Relic)
                && args.weapon_change_timer_active
            {
                return can_equip_item_outcome(InventoryResult::ItemCooldown);
            }

            if matches!(args.current_generic_spell_allows_equip, Some(false))
                || matches!(args.current_channeled_spell_allows_equip, Some(false))
            {
                return can_equip_item_outcome(InventoryResult::ClientLockedOut);
            }
        }

        if args.heirloom_required_level_failed {
            return can_equip_item_outcome(InventoryResult::NotEquippable);
        }

        let eslot = self.find_equip_slot(FindEquipSlotArgs {
            proto,
            slot: args.slot,
            swap: args.swap,
            can_dual_wield: args.can_dual_wield,
            can_titan_grip: args.can_titan_grip,
            is_two_hand_used: args.is_two_hand_used,
            has_required_profession_skill: args.has_required_profession_skill,
            profession_slot: args.profession_slot,
            equipped_items: args.equipped_items,
        });
        if eslot == NULL_SLOT {
            return can_equip_item_outcome(InventoryResult::NotEquippable);
        }

        if args.can_use_result != InventoryResult::Ok {
            return can_equip_item_outcome(args.can_use_result);
        }

        if !args.swap && item_ref_by_pos(args.equipped_items, INVENTORY_SLOT_BAG_0, eslot).is_some()
        {
            return can_equip_item_outcome(InventoryResult::NoSlotAvailable);
        }

        let mut ignore = paired_unique_ignore_slot(eslot).unwrap_or(NULL_SLOT);
        if ignore == NULL_SLOT
            || !item_ref_by_pos(args.equipped_items, INVENTORY_SLOT_BAG_0, ignore)
                .is_some_and(|equipped| std::ptr::eq(equipped, source))
        {
            ignore = eslot;
        }
        let unique_ignore_slot = if args.swap { ignore } else { NULL_SLOT };
        if args.can_equip_unique_result != InventoryResult::Ok {
            return CanEquipItemOutcome {
                result: args.can_equip_unique_result,
                dest: 0,
                unique_ignore_slot: Some(unique_ignore_slot),
            };
        }

        if proto.class_id == ItemClass::Quiver {
            for stored in args.stored_items {
                if stored.bag != INVENTORY_SLOT_BAG_0
                    || !(INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&stored.slot)
                    || std::ptr::eq(stored.item, source)
                {
                    continue;
                }

                if let Some(bag_proto) = stored.template {
                    if bag_proto.class_id == proto.class_id && (!args.swap || stored.slot != eslot)
                    {
                        return CanEquipItemOutcome {
                            result: if bag_proto.subclass_id == ItemSubClassQuiver::AmmoPouch as u32
                            {
                                InventoryResult::OnlyOneAmmo
                            } else {
                                InventoryResult::OnlyOneQuiver
                            },
                            dest: 0,
                            unique_ignore_slot: Some(unique_ignore_slot),
                        };
                    }
                }
            }
        }

        if eslot == EQUIPMENT_SLOT_OFFHAND {
            match proto.inventory_type {
                InventoryType::Weapon
                    if proto.subclass_id == ItemSubClassWeapon::Polearm as u32 =>
                {
                    return can_equip_item_outcome(InventoryResult::TwoHandSkillNotFound);
                }
                InventoryType::Weapon if !args.can_dual_wield => {
                    return can_equip_item_outcome(InventoryResult::TwoHandSkillNotFound);
                }
                InventoryType::WeaponOffhand
                    if !args.can_dual_wield && !args.proto_always_allow_dual_wield =>
                {
                    return can_equip_item_outcome(InventoryResult::TwoHandSkillNotFound);
                }
                InventoryType::Weapon2Hand if !args.can_dual_wield || !args.can_titan_grip => {
                    return can_equip_item_outcome(InventoryResult::TwoHandSkillNotFound);
                }
                _ => {}
            }

            if args.is_two_hand_used {
                return can_equip_item_outcome(InventoryResult::Equipped2handed);
            }
        }

        if proto.inventory_type == InventoryType::Weapon2Hand {
            if eslot == EQUIPMENT_SLOT_OFFHAND {
                if !args.can_titan_grip {
                    return can_equip_item_outcome(InventoryResult::NotEquippable);
                }
            } else if eslot != EQUIPMENT_SLOT_MAINHAND {
                return can_equip_item_outcome(InventoryResult::NotEquippable);
            }

            if !args.can_titan_grip
                && item_ref_by_pos(
                    args.equipped_items,
                    INVENTORY_SLOT_BAG_0,
                    EQUIPMENT_SLOT_OFFHAND,
                )
                .is_some()
                && (!args.not_loading
                    || args.offhand_can_unequip_result != InventoryResult::Ok
                    || args.offhand_can_store_result != InventoryResult::Ok)
            {
                return can_equip_item_outcome(if args.swap {
                    InventoryResult::CantSwap
                } else {
                    InventoryResult::InvFull
                });
            }
        }

        CanEquipItemOutcome {
            result: InventoryResult::Ok,
            dest: make_item_pos(INVENTORY_SLOT_BAG_0, eslot),
            unique_ignore_slot: Some(unique_ignore_slot),
        }
    }

    pub fn can_unequip_item(&self, args: CanUnequipItemArgs<'_>) -> InventoryResult {
        if !is_equipment_packed_pos(args.pos) && !is_bag_pos(args.pos) {
            return InventoryResult::Ok;
        }

        let Some(source) = args.source_item else {
            return InventoryResult::Ok;
        };

        let Some(proto) = args.proto else {
            return InventoryResult::ItemNotFound;
        };

        if source.loot_generated() {
            return InventoryResult::LootGone;
        }

        if args.is_charmed {
            return InventoryResult::ClientLockedOut;
        }

        if !proto.can_change_equip_state_in_combat() {
            if args.is_in_combat {
                return InventoryResult::NotInCombat;
            }

            if args.is_in_progress_arena {
                return InventoryResult::NotDuringArenaMatch;
            }
        }

        if !args.swap && args.source_is_not_empty_bag {
            return InventoryResult::DestroyNonemptyBag;
        }

        InventoryResult::Ok
    }

    pub fn can_use_item_template(&self, args: CanUseItemTemplateArgs<'_>) -> InventoryResult {
        if args.proto.is_none() {
            return InventoryResult::ItemNotFound;
        }

        if args.internal_item {
            return InventoryResult::CantEquipEver;
        }

        if args.faction_horde && args.team != TEAM_HORDE_ID {
            return InventoryResult::CantEquipEver;
        }

        if args.faction_alliance && args.team != TEAM_ALLIANCE_ID {
            return InventoryResult::CantEquipEver;
        }

        if !args.allowable_class_matches || !args.allowable_race_matches {
            return InventoryResult::CantEquipEver;
        }

        if args.required_skill != 0 {
            if args.required_skill_value == 0 {
                return InventoryResult::ProficiencyNeeded;
            }

            if args.required_skill_value < args.required_skill_rank {
                return InventoryResult::CantEquipSkill;
            }
        }

        if args.required_spell != 0 && !args.has_required_spell {
            return InventoryResult::ProficiencyNeeded;
        }

        if !args.skip_required_level_check && args.player_level < args.base_required_level {
            return InventoryResult::CantEquipLevelI;
        }

        if args.holiday_id != 0 && !args.holiday_active {
            return InventoryResult::ClientLockedOut;
        }

        if args.required_reputation_faction != 0
            && args.player_reputation_rank < args.required_reputation_rank
        {
            return InventoryResult::CantEquipReputation;
        }

        if matches!(args.effect0_spell_id, Some(483 | 55_884))
            && args.effect1_spell_id.is_some()
            && args.has_effect1_spell
        {
            return InventoryResult::InternalBagError;
        }

        if args
            .artifact_specialization
            .is_some_and(|spec| spec != args.primary_specialization)
        {
            return InventoryResult::CantUseItem;
        }

        InventoryResult::Ok
    }

    pub fn can_use_item(&self, mut args: CanUseItemArgs<'_>) -> InventoryResult {
        let Some(source) = args.source_item else {
            return InventoryResult::ItemNotFound;
        };

        if !args.is_alive && args.not_loading {
            return InventoryResult::PlayerDead;
        }

        let Some(proto) = args.proto else {
            return InventoryResult::ItemNotFound;
        };

        if source.is_binded_not_with(self.guid(), proto, args.source_bop_trade_allowed_for_player) {
            return InventoryResult::NotOwner;
        }

        if args.player_level < args.item_required_level {
            return InventoryResult::CantEquipLevelI;
        }

        args.template_args.proto = args.proto;
        args.template_args.skip_required_level_check = true;
        let template_result = self.can_use_item_template(args.template_args);
        if template_result != InventoryResult::Ok {
            return template_result;
        }

        if args.item_skill != 0 {
            let allow_equip = args.proto_is_heirloom
                && proto.class_id == ItemClass::Armor
                && !args.has_item_skill
                && match args.player_class {
                    CLASS_HUNTER | CLASS_SHAMAN => args.item_skill == SKILL_MAIL,
                    CLASS_PALADIN | CLASS_WARRIOR => args.item_skill == SKILL_PLATE_MAIL,
                    _ => false,
                };

            if !allow_equip && args.item_skill_value == 0 {
                return InventoryResult::ProficiencyNeeded;
            }
        }

        InventoryResult::Ok
    }

    pub fn can_equip_unique_item_template(
        &self,
        args: CanEquipUniqueItemTemplateArgs<'_>,
    ) -> InventoryResult {
        let Some(proto) = args.proto else {
            return InventoryResult::ItemNotFound;
        };

        if args.unique_equippable
            && (has_equipped_item_entry(args.equipped_items, proto.entry, args.except_slot)
                || has_equipped_gem_entry(args.equipped_gems, proto.entry, args.except_slot))
        {
            return InventoryResult::ItemUniqueEquippable;
        }

        if proto.item_limit_category != 0 {
            let Some(limit_category) = args.limit_category else {
                return InventoryResult::NotEquippable;
            };
            let limit_quantity = u32::from(limit_category.quantity);

            if args.limit_count > limit_quantity {
                return InventoryResult::ItemMaxLimitCategoryEquippedExceededIs;
            }

            let required_count = limit_quantity.saturating_sub(args.limit_count) + 1;
            if equipped_item_limit_category_count(
                args.equipped_items,
                proto.item_limit_category,
                args.except_slot,
            ) >= required_count
            {
                return InventoryResult::ItemMaxLimitCategoryEquippedExceededIs;
            }

            if equipped_gem_limit_category_count(
                args.equipped_gems,
                proto.item_limit_category,
                args.except_slot,
            ) >= required_count
            {
                return InventoryResult::ItemMaxCountEquippedSocketed;
            }
        }

        InventoryResult::Ok
    }

    pub fn can_equip_unique_item(&self, args: CanEquipUniqueItemArgs<'_>) -> InventoryResult {
        let Some(source) = args.source_item else {
            return InventoryResult::ItemNotFound;
        };

        let template_result = self.can_equip_unique_item_template(CanEquipUniqueItemTemplateArgs {
            proto: args.proto,
            except_slot: args.except_slot,
            limit_count: args.limit_count,
            unique_equippable: args.unique_equippable,
            limit_category: args.limit_category,
            equipped_items: args.equipped_items,
            equipped_gems: args.equipped_gems,
        });
        if template_result != InventoryResult::Ok {
            return template_result;
        }

        for gem in args.socketed_gems {
            let Some(gem_proto) = gem.proto else {
                continue;
            };

            let gem_limit_count = if !source.is_equipped() && gem_proto.item_limit_category != 0 {
                gem.source_limit_category_count
            } else {
                1
            };

            let gem_result = self.can_equip_unique_item_template(CanEquipUniqueItemTemplateArgs {
                proto: Some(gem_proto),
                except_slot: args.except_slot,
                limit_count: gem_limit_count,
                unique_equippable: gem.unique_equippable,
                limit_category: gem.limit_category,
                equipped_items: args.equipped_items,
                equipped_gems: args.equipped_gems,
            });
            if gem_result != InventoryResult::Ok {
                return gem_result;
            }
        }

        InventoryResult::Ok
    }

    pub fn can_bank_item(
        &self,
        dest: &mut Vec<ItemPosCount>,
        args: CanBankItemArgs<'_>,
    ) -> InventoryResult {
        let Some(source) = args.source_item else {
            return if args.swap {
                InventoryResult::CantSwap
            } else {
                InventoryResult::ItemNotFound
            };
        };

        let Some(proto) = args.proto else {
            return if args.swap {
                InventoryResult::CantSwap
            } else {
                InventoryResult::ItemNotFound
            };
        };

        if source.loot_generated() {
            return InventoryResult::LootGone;
        }

        if source.is_binded_not_with(self.guid(), proto, args.source_bop_trade_allowed_for_player) {
            return InventoryResult::NotOwner;
        }

        if args.source_is_currency_token {
            return InventoryResult::CantSwap;
        }

        let similar_result = self.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
            proto: args.proto,
            count: source.count(),
            source_item: args.source_item,
            current_item_count: self.item_count_by_entry(
                proto.entry,
                true,
                args.source_item,
                args.stored_items,
            ),
            limit_category: args.limit_category,
            current_limit_category_count: self.item_count_with_limit_category(
                proto.item_limit_category,
                args.source_item,
                args.stored_items,
            ),
        });
        if similar_result.result != InventoryResult::Ok {
            return similar_result.result;
        }

        let mut count = source.count();

        if args.bag != NULL_BAG && args.slot != NULL_SLOT {
            if (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&args.slot) {
                if !args.source_is_bag {
                    return InventoryResult::WrongSlot;
                }

                if args.slot - BANK_SLOT_BAG_START >= self.data.num_bank_slots {
                    return InventoryResult::NoBankSlot;
                }

                if args.can_use_result != InventoryResult::Ok {
                    return args.can_use_result;
                }
            }

            let result = self.can_store_item_in_specific_slot(
                args.bag,
                args.slot,
                dest,
                proto,
                &mut count,
                args.swap,
                item_ref_by_pos(args.slot_items, args.bag, args.slot),
                args.source_item,
                args.source_is_not_empty_bag,
                bag_template_by_pos(args.bag_templates, args.bag),
            );
            if result != InventoryResult::Ok {
                return result;
            }

            if count == 0 {
                return InventoryResult::Ok;
            }
        }

        if args.bag != NULL_BAG {
            if args.source_is_not_empty_bag {
                return InventoryResult::BagInBag;
            }

            if proto.max_stack_size != 1 {
                if args.bag == INVENTORY_SLOT_BAG_0 {
                    let result = self.can_store_item_in_inventory_slots(
                        BANK_SLOT_ITEM_START,
                        BANK_SLOT_ITEM_END,
                        dest,
                        proto,
                        &mut count,
                        true,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        return result;
                    }
                    if count == 0 {
                        return InventoryResult::Ok;
                    }
                } else {
                    let mut result = self.can_store_item_in_bag(
                        args.bag,
                        dest,
                        proto,
                        &mut count,
                        true,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        NULL_BAG,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, args.bag),
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        result = self.can_store_item_in_bag(
                            args.bag,
                            dest,
                            proto,
                            &mut count,
                            true,
                            true,
                            args.source_item,
                            args.source_is_not_empty_bag,
                            NULL_BAG,
                            args.slot,
                            bag_template_by_pos(args.bag_templates, args.bag),
                            args.slot_items,
                        );
                    }
                    if result != InventoryResult::Ok {
                        return result;
                    }
                    if count == 0 {
                        return InventoryResult::Ok;
                    }
                }
            }

            if args.bag == INVENTORY_SLOT_BAG_0 {
                let result = self.can_store_item_in_inventory_slots(
                    BANK_SLOT_ITEM_START,
                    BANK_SLOT_ITEM_END,
                    dest,
                    proto,
                    &mut count,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    return result;
                }
                if count == 0 {
                    return InventoryResult::Ok;
                }
            } else {
                let mut result = self.can_store_item_in_bag(
                    args.bag,
                    dest,
                    proto,
                    &mut count,
                    false,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    NULL_BAG,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, args.bag),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    result = self.can_store_item_in_bag(
                        args.bag,
                        dest,
                        proto,
                        &mut count,
                        false,
                        true,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        NULL_BAG,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, args.bag),
                        args.slot_items,
                    );
                }
                if result != InventoryResult::Ok {
                    return result;
                }
                if count == 0 {
                    return InventoryResult::Ok;
                }
            }
        }

        if proto.max_stack_size != 1 {
            let result = self.can_store_item_in_inventory_slots(
                BANK_SLOT_ITEM_START,
                BANK_SLOT_ITEM_END,
                dest,
                proto,
                &mut count,
                true,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                return result;
            }
            if count == 0 {
                return InventoryResult::Ok;
            }

            if !proto.bag_family.is_empty() {
                for bag_slot in BANK_SLOT_BAG_START..BANK_SLOT_BAG_END {
                    let result = self.can_store_item_in_bag(
                        bag_slot,
                        dest,
                        proto,
                        &mut count,
                        true,
                        false,
                        args.source_item,
                        args.source_is_not_empty_bag,
                        args.bag,
                        args.slot,
                        bag_template_by_pos(args.bag_templates, bag_slot),
                        args.slot_items,
                    );
                    if result != InventoryResult::Ok {
                        continue;
                    }
                    if count == 0 {
                        return InventoryResult::Ok;
                    }
                }
            }

            for bag_slot in BANK_SLOT_BAG_START..BANK_SLOT_BAG_END {
                let result = self.can_store_item_in_bag(
                    bag_slot,
                    dest,
                    proto,
                    &mut count,
                    true,
                    true,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, bag_slot),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    continue;
                }
                if count == 0 {
                    return InventoryResult::Ok;
                }
            }
        }

        if !proto.bag_family.is_empty() {
            for bag_slot in BANK_SLOT_BAG_START..BANK_SLOT_BAG_END {
                let result = self.can_store_item_in_bag(
                    bag_slot,
                    dest,
                    proto,
                    &mut count,
                    false,
                    false,
                    args.source_item,
                    args.source_is_not_empty_bag,
                    args.bag,
                    args.slot,
                    bag_template_by_pos(args.bag_templates, bag_slot),
                    args.slot_items,
                );
                if result != InventoryResult::Ok {
                    continue;
                }
                if count == 0 {
                    return InventoryResult::Ok;
                }
            }
        }

        let result = self.can_store_item_in_inventory_slots(
            BANK_SLOT_ITEM_START,
            BANK_SLOT_ITEM_END,
            dest,
            proto,
            &mut count,
            false,
            args.source_item,
            args.source_is_not_empty_bag,
            args.bag,
            args.slot,
            args.slot_items,
        );
        if result != InventoryResult::Ok {
            return result;
        }
        if count == 0 {
            return InventoryResult::Ok;
        }

        for bag_slot in BANK_SLOT_BAG_START..BANK_SLOT_BAG_END {
            let result = self.can_store_item_in_bag(
                bag_slot,
                dest,
                proto,
                &mut count,
                false,
                true,
                args.source_item,
                args.source_is_not_empty_bag,
                args.bag,
                args.slot,
                bag_template_by_pos(args.bag_templates, bag_slot),
                args.slot_items,
            );
            if result != InventoryResult::Ok {
                continue;
            }
            if count == 0 {
                return InventoryResult::Ok;
            }
        }

        InventoryResult::BankFull
    }

    pub fn top_level_item_guid(&self, slot: u8) -> Option<ObjectGuid> {
        self.inventory.items.get(slot as usize).copied().flatten()
    }

    pub fn register_bag_storage(
        &mut self,
        bag_slot: u8,
        bag_guid: ObjectGuid,
        bag_size: u8,
    ) -> Result<(), PlayerStorageError> {
        if !is_bag_storage_slot(bag_slot) {
            return Err(PlayerStorageError::InvalidBagSlot(bag_slot));
        }
        if bag_size as usize > MAX_BAG_SIZE {
            return Err(PlayerStorageError::InvalidBagItemSlot(bag_size));
        }

        self.inventory.bags[bag_slot as usize] = Some(PlayerBagStorage::new(bag_guid, bag_size));
        Ok(())
    }

    pub fn store_top_level_item(
        &mut self,
        slot: u8,
        guid: ObjectGuid,
    ) -> Result<(), PlayerStorageError> {
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        self.inventory.items[slot as usize] = Some(guid);
        self.set_inv_slot(slot as usize, guid);
        Ok(())
    }

    pub fn visualize_item(
        &mut self,
        slot: u8,
        guid: ObjectGuid,
        visible: VisibleItemValues,
    ) -> Result<(), PlayerStorageError> {
        self.store_top_level_item(slot, guid)?;
        if slot < EQUIPMENT_SLOT_END {
            self.set_visible_item_slot(slot, Some(visible));
        }
        Ok(())
    }

    pub fn visualize_item_object(
        &mut self,
        slot: u8,
        item: &mut Item,
        visible: VisibleItemValues,
    ) -> Result<(), PlayerStorageError> {
        let item_guid = item.object().guid();
        self.store_top_level_item(slot, item_guid)?;

        let owner_guid = self.guid();
        item.bind_if_visualized();
        item.set_contained_in(owner_guid);
        item.set_owner_guid(owner_guid);
        item.set_slot(slot);
        item.set_container_guid(ObjectGuid::EMPTY);

        if slot < EQUIPMENT_SLOT_END {
            self.set_visible_item_slot(slot, Some(visible));
        }

        item.set_state(ItemUpdateState::Changed);
        Ok(())
    }

    pub fn equip_item_object(
        &mut self,
        pos: u16,
        item: &mut Item,
        existing: Option<&mut Item>,
        visible: VisibleItemValues,
    ) -> Result<EquipItemObjectOutcome, PlayerStorageError> {
        let bag = (pos >> 8) as u8;
        let slot = pos as u8;
        if bag != INVENTORY_SLOT_BAG_0 {
            return Err(PlayerStorageError::UnknownBag(bag));
        }
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        match existing {
            None => {
                if self.top_level_item_guid(slot).is_some() {
                    return Err(PlayerStorageError::OccupiedPlayerSlot(slot));
                }

                self.visualize_item_object(slot, item, visible)?;
                item.set_item_flag2(ItemFieldFlags2::EQUIPPED);
                Ok(EquipItemObjectOutcome::Equipped)
            }
            Some(existing) => {
                let Some(expected_guid) = self.top_level_item_guid(slot) else {
                    return Err(PlayerStorageError::EmptyPlayerSlot(slot));
                };

                let actual_guid = existing.object().guid();
                if expected_guid != actual_guid {
                    return Err(PlayerStorageError::MismatchedItemGuid {
                        slot,
                        expected: expected_guid,
                        actual: actual_guid,
                    });
                }

                existing.set_count(existing.count() + item.count());
                existing.set_state(ItemUpdateState::Changed);

                item.set_owner_guid(self.guid());
                item.set_not_refundable();
                item.clear_soulbound_tradeable();
                item.set_state(ItemUpdateState::Removed);
                Ok(EquipItemObjectOutcome::Merged)
            }
        }
    }

    pub fn quick_equip_item_object(
        &mut self,
        pos: u16,
        item: &mut Item,
        visible: VisibleItemValues,
    ) -> Result<(), PlayerStorageError> {
        let bag = (pos >> 8) as u8;
        let slot = pos as u8;
        if bag != INVENTORY_SLOT_BAG_0 {
            return Err(PlayerStorageError::UnknownBag(bag));
        }
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        self.visualize_item_object(slot, item, visible)?;
        item.set_item_flag2(ItemFieldFlags2::EQUIPPED);
        Ok(())
    }

    pub fn store_item_object(
        &mut self,
        slot: u8,
        item: &mut Item,
        count: u32,
    ) -> Result<(), PlayerStorageError> {
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        if self.inventory.items[slot as usize].is_some() {
            return Err(PlayerStorageError::OccupiedPlayerSlot(slot));
        }

        let item_guid = item.object().guid();
        self.store_top_level_item(slot, item_guid)?;

        let owner_guid = self.guid();
        item.set_count(count);
        item.bind_if_stored(is_bag_storage_slot(slot));
        item.set_contained_in(owner_guid);
        item.set_owner_guid(owner_guid);
        item.set_slot(slot);
        item.set_container_guid(ObjectGuid::EMPTY);
        item.set_state(ItemUpdateState::Changed);
        Ok(())
    }

    pub fn store_cloned_item_object(
        &mut self,
        slot: u8,
        source: &Item,
        new_guid: ObjectGuid,
        count: u32,
    ) -> Result<Item, PlayerStorageError> {
        let mut cloned = source.clone_item_for_store(new_guid, Some(self.guid()), count);
        self.store_item_object(slot, &mut cloned, count)?;
        Ok(cloned)
    }

    pub fn split_item_to_empty_top_level_object(
        &mut self,
        slot: u8,
        source: &mut Item,
        new_guid: ObjectGuid,
        count: u32,
    ) -> Result<Item, PlayerStorageError> {
        validate_split_source(source, count)?;

        let cloned = self.store_cloned_item_object(slot, source, new_guid, count)?;
        source.set_count(source.count() - count);
        source.set_state(ItemUpdateState::Changed);
        Ok(cloned)
    }

    pub fn merge_top_level_item_stack_object(
        &mut self,
        slot: u8,
        existing: &mut Item,
        incoming: &mut Item,
        count: u32,
    ) -> Result<(), PlayerStorageError> {
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        let Some(expected_guid) = self.top_level_item_guid(slot) else {
            return Err(PlayerStorageError::EmptyPlayerSlot(slot));
        };

        let actual_guid = existing.object().guid();
        if expected_guid != actual_guid {
            return Err(PlayerStorageError::MismatchedItemGuid {
                slot,
                expected: expected_guid,
                actual: actual_guid,
            });
        }

        existing.bind_if_stored(is_bag_storage_slot(slot));
        existing.set_count(existing.count() + count);
        existing.set_state(ItemUpdateState::Changed);

        let owner_guid = self.guid();
        incoming.set_owner_guid(owner_guid);
        incoming.set_not_refundable();
        incoming.clear_soulbound_tradeable();
        incoming.set_state(ItemUpdateState::Removed);
        Ok(())
    }

    pub fn remove_top_level_item(
        &mut self,
        slot: u8,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        if slot as usize >= PLAYER_SLOT_END {
            return Err(PlayerStorageError::InvalidPlayerSlot(slot));
        }

        let removed = self.inventory.items[slot as usize].take();
        self.set_inv_slot(slot as usize, ObjectGuid::EMPTY);
        if slot < EQUIPMENT_SLOT_END {
            self.set_visible_item_slot(slot, None);
        }
        if is_bag_storage_slot(slot) {
            self.inventory.bags[slot as usize] = None;
        }
        Ok(removed)
    }

    pub fn remove_item_object(
        &mut self,
        bag: u8,
        slot: u8,
        item: Option<&mut Item>,
        bag_object: Option<&mut Bag>,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        let Some(item) = item else {
            return Ok(None);
        };

        let item_guid = item.object().guid();
        let removed = if bag == INVENTORY_SLOT_BAG_0 {
            let Some(expected_guid) = self.top_level_item_guid(slot) else {
                return Err(PlayerStorageError::EmptyPlayerSlot(slot));
            };
            if expected_guid != item_guid {
                return Err(PlayerStorageError::MismatchedItemGuid {
                    slot,
                    expected: expected_guid,
                    actual: item_guid,
                });
            }

            if slot < INVENTORY_SLOT_BAG_END {
                item.remove_item_flag2(ItemFieldFlags2::EQUIPPED);
            }

            self.remove_top_level_item(slot)?
        } else {
            let Some(bag_object) = bag_object else {
                return Err(PlayerStorageError::UnknownBag(bag));
            };
            let expected_bag_guid = self
                .get_bag_by_pos(bag)
                .ok_or(PlayerStorageError::UnknownBag(bag))?;
            let actual_bag_guid = bag_object.item().object().guid();
            if expected_bag_guid != actual_bag_guid {
                return Err(PlayerStorageError::MismatchedBagGuid {
                    bag,
                    expected: expected_bag_guid,
                    actual: actual_bag_guid,
                });
            }

            let expected_guid = self
                .inventory
                .bags
                .get(bag as usize)
                .and_then(Option::as_ref)
                .and_then(|bag_storage| bag_storage.item_by_pos(slot))
                .ok_or(PlayerStorageError::EmptyBagItemSlot { bag, slot })?;
            if expected_guid != item_guid {
                return Err(PlayerStorageError::MismatchedBagItemGuid {
                    bag,
                    slot,
                    expected: expected_guid,
                    actual: item_guid,
                });
            }

            bag_object.remove_item(slot);
            self.remove_bag_item(bag, slot)?
        };

        item.set_contained_in(ObjectGuid::EMPTY);
        item.set_slot(NULL_SLOT);
        item.set_container_guid(ObjectGuid::EMPTY);
        Ok(removed)
    }

    pub fn move_item_from_inventory_object(
        &mut self,
        bag: u8,
        slot: u8,
        item: Option<&mut Item>,
        bag_object: Option<&mut Bag>,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        let Some(item) = item else {
            return Ok(None);
        };

        let removed = self.remove_item_object(bag, slot, Some(&mut *item), bag_object)?;
        if removed.is_some() {
            item.set_not_refundable();
        }
        Ok(removed)
    }

    pub fn finalize_move_item_to_inventory_object(
        &self,
        original_item_guid: ObjectGuid,
        last_item: &mut Item,
        in_character_inventory_db: bool,
    ) -> bool {
        if original_item_guid != last_item.object().guid() {
            return false;
        }

        if last_item.owner_guid() != self.guid() {
            last_item.set_owner_guid(self.guid());
        }

        last_item.set_state(if in_character_inventory_db {
            ItemUpdateState::Changed
        } else {
            ItemUpdateState::New
        });
        true
    }

    pub fn destroy_item_object(
        &mut self,
        bag: u8,
        slot: u8,
        item: Option<&mut Item>,
        bag_object: Option<&mut Bag>,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        let Some(item) = item else {
            return Ok(None);
        };

        let item_guid = item.object().guid();
        let removed = if bag == INVENTORY_SLOT_BAG_0 {
            let Some(expected_guid) = self.top_level_item_guid(slot) else {
                return Err(PlayerStorageError::EmptyPlayerSlot(slot));
            };
            if expected_guid != item_guid {
                return Err(PlayerStorageError::MismatchedItemGuid {
                    slot,
                    expected: expected_guid,
                    actual: item_guid,
                });
            }

            self.remove_top_level_item(slot)?
        } else {
            let Some(bag_object) = bag_object else {
                return Err(PlayerStorageError::UnknownBag(bag));
            };
            let expected_bag_guid = self
                .get_bag_by_pos(bag)
                .ok_or(PlayerStorageError::UnknownBag(bag))?;
            let actual_bag_guid = bag_object.item().object().guid();
            if expected_bag_guid != actual_bag_guid {
                return Err(PlayerStorageError::MismatchedBagGuid {
                    bag,
                    expected: expected_bag_guid,
                    actual: actual_bag_guid,
                });
            }

            let expected_guid = self
                .inventory
                .bags
                .get(bag as usize)
                .and_then(Option::as_ref)
                .and_then(|bag_storage| bag_storage.item_by_pos(slot))
                .ok_or(PlayerStorageError::EmptyBagItemSlot { bag, slot })?;
            if expected_guid != item_guid {
                return Err(PlayerStorageError::MismatchedBagItemGuid {
                    bag,
                    slot,
                    expected: expected_guid,
                    actual: item_guid,
                });
            }

            bag_object.remove_item(slot);
            self.remove_bag_item(bag, slot)?
        };

        item.set_not_refundable();
        item.clear_soulbound_tradeable();
        item.set_contained_in(ObjectGuid::EMPTY);
        item.set_slot(NULL_SLOT);
        item.set_container_guid(ObjectGuid::EMPTY);
        item.set_state(ItemUpdateState::Removed);
        Ok(removed)
    }

    pub fn destroy_item_count_for_item_object(
        &mut self,
        item: Option<&mut Item>,
        count: &mut u32,
        bag_object: Option<&mut Bag>,
    ) -> Result<(), PlayerStorageError> {
        let Some(item) = item else {
            return Ok(());
        };

        if item.count() <= *count {
            *count -= item.count();
            let bag = item.bag_slot();
            let slot = item.slot();
            self.destroy_item_object(bag, slot, Some(item), bag_object)?;
        } else {
            item.set_count(item.count() - *count);
            *count = 0;
            item.set_state(ItemUpdateState::Changed);
        }

        Ok(())
    }

    pub fn destroy_item_count_by_entry_plan(
        &self,
        item_entry: u32,
        count: u32,
        unequip_check: bool,
        inventory_slot_count: u8,
        items: &[DestroyItemCountItemRef<'_>],
    ) -> DestroyItemCountPlan {
        let mut plan = DestroyItemCountPlan {
            removed_count: 0,
            actions: Vec::new(),
        };
        if count == 0 {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            INVENTORY_SLOT_ITEM_START,
            INVENTORY_SLOT_ITEM_START.saturating_add(inventory_slot_count),
            false,
            unequip_check,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            KEYRING_SLOT_START,
            KEYRING_SLOT_END,
            false,
            unequip_check,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_bag_ranges(
            &mut plan,
            items,
            item_entry,
            count,
            INVENTORY_SLOT_BAG_START,
            INVENTORY_SLOT_BAG_END,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            EQUIPMENT_SLOT_HEAD,
            INVENTORY_SLOT_BAG_END,
            true,
            unequip_check,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            BANK_SLOT_ITEM_START,
            BANK_SLOT_ITEM_END,
            false,
            unequip_check,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_bag_ranges(
            &mut plan,
            items,
            item_entry,
            count,
            BANK_SLOT_BAG_START,
            BANK_SLOT_BAG_END,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            BANK_SLOT_BAG_START,
            BANK_SLOT_BAG_END,
            true,
            unequip_check,
        );
        if plan.removed_count >= count {
            return plan;
        }

        destroy_item_count_scan_top_level_range(
            &mut plan,
            items,
            item_entry,
            count,
            CHILD_EQUIPMENT_SLOT_START,
            CHILD_EQUIPMENT_SLOT_END,
            false,
            unequip_check,
        );

        plan
    }

    pub fn destroy_zone_limited_item_plan(
        &self,
        inventory_slot_count: u8,
        items: &[DestroyFilteredItemRef],
    ) -> Vec<DestroyFilteredItemAction> {
        let mut actions = Vec::new();
        destroy_filtered_scan_top_level_range(
            &mut actions,
            items,
            INVENTORY_SLOT_ITEM_START,
            INVENTORY_SLOT_ITEM_START.saturating_add(inventory_slot_count),
        );
        destroy_filtered_scan_top_level_range(
            &mut actions,
            items,
            KEYRING_SLOT_START,
            KEYRING_SLOT_END,
        );
        destroy_filtered_scan_bag_ranges(
            &mut actions,
            items,
            INVENTORY_SLOT_BAG_START,
            INVENTORY_SLOT_BAG_END,
        );
        destroy_filtered_scan_top_level_range(
            &mut actions,
            items,
            EQUIPMENT_SLOT_HEAD,
            INVENTORY_SLOT_BAG_END,
        );
        actions
    }

    pub fn destroy_conjured_items_plan(
        &self,
        inventory_slot_count: u8,
        items: &[DestroyFilteredItemRef],
    ) -> Vec<DestroyFilteredItemAction> {
        let mut actions = Vec::new();
        destroy_filtered_scan_top_level_range(
            &mut actions,
            items,
            INVENTORY_SLOT_ITEM_START,
            INVENTORY_SLOT_ITEM_START.saturating_add(inventory_slot_count),
        );
        destroy_filtered_scan_bag_ranges(
            &mut actions,
            items,
            INVENTORY_SLOT_BAG_START,
            INVENTORY_SLOT_BAG_END,
        );
        destroy_filtered_scan_top_level_range(
            &mut actions,
            items,
            EQUIPMENT_SLOT_HEAD,
            INVENTORY_SLOT_BAG_END,
        );
        actions
    }

    pub fn swap_item_preflight_plan(
        &self,
        src: u16,
        dst: u16,
        is_alive: bool,
        src_item: Option<SwapItemPreflightItem>,
        dst_item: Option<SwapItemPreflightItem>,
    ) -> SwapItemPreflightPlan {
        let Some(src_item) = src_item else {
            return SwapItemPreflightPlan {
                result: SwapItemPreflightResult::NoSource,
                src_unequip_swap: None,
                dst_unequip_swap: None,
            };
        };

        if src_item.is_child {
            if let Some(parent_pos) = src_item.parent_pos {
                if is_equipment_packed_pos(src) {
                    return SwapItemPreflightPlan {
                        result: SwapItemPreflightResult::ChildRedirect {
                            first_src: dst,
                            first_dst: src,
                            second_src: parent_pos,
                            second_dst: dst,
                        },
                        src_unequip_swap: None,
                        dst_unequip_swap: None,
                    };
                }
            }
        } else if let Some(dst_item) = dst_item {
            if dst_item.is_child {
                if let Some(parent_pos) = dst_item.parent_pos {
                    if is_equipment_packed_pos(dst) {
                        return SwapItemPreflightPlan {
                            result: SwapItemPreflightResult::ChildRedirect {
                                first_src: src,
                                first_dst: dst,
                                second_src: parent_pos,
                                second_dst: src,
                            },
                            src_unequip_swap: None,
                            dst_unequip_swap: None,
                        };
                    }
                }
            }
        }

        if !is_alive {
            return SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::PlayerDead),
                src_unequip_swap: None,
                dst_unequip_swap: None,
            };
        }

        let mut src_unequip_swap = None;
        if is_equipment_packed_pos(src) || is_bag_pos(src) {
            let swap = !is_bag_pos(src)
                || is_bag_pos(dst)
                || dst_item.is_some_and(|item| item.is_bag && item.is_empty_bag);
            src_unequip_swap = Some(swap);
            if src_item.can_unequip_result != InventoryResult::Ok {
                return SwapItemPreflightPlan {
                    result: SwapItemPreflightResult::Error(src_item.can_unequip_result),
                    src_unequip_swap,
                    dst_unequip_swap: None,
                };
            }
        }

        let [_src_bag, src_slot] = src.to_be_bytes();
        let [dst_bag, _dst_slot] = dst.to_be_bytes();
        if is_bag_pos(src) && src_slot == dst_bag {
            return SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::BagInBag),
                src_unequip_swap,
                dst_unequip_swap: None,
            };
        }

        let [src_bag, _src_slot] = src.to_be_bytes();
        let [_dst_bag, dst_slot] = dst.to_be_bytes();
        if is_bag_pos(dst) && src_bag == dst_slot {
            return SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::CantSwap),
                src_unequip_swap,
                dst_unequip_swap: None,
            };
        }

        let mut dst_unequip_swap = None;
        if let Some(dst_item) = dst_item {
            if is_equipment_packed_pos(dst) || is_bag_pos(dst) {
                let swap = !is_bag_pos(dst)
                    || is_bag_pos(src)
                    || (src_item.is_bag && src_item.is_empty_bag);
                dst_unequip_swap = Some(swap);
                if dst_item.can_unequip_result != InventoryResult::Ok {
                    return SwapItemPreflightPlan {
                        result: SwapItemPreflightResult::Error(dst_item.can_unequip_result),
                        src_unequip_swap,
                        dst_unequip_swap,
                    };
                }
            }
        }

        SwapItemPreflightPlan {
            result: SwapItemPreflightResult::Continue,
            src_unequip_swap,
            dst_unequip_swap,
        }
    }

    pub fn swap_item_empty_destination_plan(
        &self,
        src: u16,
        dst: u16,
        dst_item_present: bool,
        can_store_result: InventoryResult,
        can_bank_result: InventoryResult,
        can_equip_result: InventoryResult,
        equip_dest: u16,
    ) -> SwapItemEmptyDestinationPlan {
        if dst_item_present {
            return SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::OccupiedDestination,
            };
        }

        if is_inventory_packed_pos(dst) {
            if can_store_result != InventoryResult::Ok {
                return SwapItemEmptyDestinationPlan {
                    result: SwapItemEmptyDestinationResult::Error(can_store_result),
                };
            }

            return SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::MoveToInventory {
                    quest_added_from_bank: is_bank_packed_pos(src),
                },
            };
        }

        if is_bank_packed_pos(dst) {
            if can_bank_result != InventoryResult::Ok {
                return SwapItemEmptyDestinationPlan {
                    result: SwapItemEmptyDestinationResult::Error(can_bank_result),
                };
            }

            return SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::MoveToBank {
                    quest_removed: true,
                },
            };
        }

        if is_equipment_packed_pos(dst) {
            if can_equip_result != InventoryResult::Ok {
                return SwapItemEmptyDestinationPlan {
                    result: SwapItemEmptyDestinationResult::Error(can_equip_result),
                };
            }

            return SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::Equip {
                    dest: equip_dest,
                    auto_unequip_offhand: true,
                },
            };
        }

        SwapItemEmptyDestinationPlan {
            result: SwapItemEmptyDestinationResult::InvalidDestinationNoop,
        }
    }

    pub fn swap_item_merge_fill_plan(
        &self,
        dst: u16,
        source_is_bag: bool,
        destination_is_bag: bool,
        source_count: u32,
        destination_count: u32,
        source_max_stack_size: u32,
        can_store_result: InventoryResult,
        can_bank_result: InventoryResult,
        can_equip_result: InventoryResult,
        equip_dest: u16,
        is_in_world: bool,
    ) -> SwapItemMergeFillPlan {
        if source_is_bag || destination_is_bag {
            return SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::ContinueToRealSwap,
                send_refund_info: false,
            };
        }

        let destination_kind = if is_inventory_packed_pos(dst) {
            Some((
                can_store_result,
                SwapItemMergeFillResult::MoveMergedStackToInventory,
            ))
        } else if is_bank_packed_pos(dst) {
            Some((
                can_bank_result,
                SwapItemMergeFillResult::MoveMergedStackToBank,
            ))
        } else if is_equipment_packed_pos(dst) {
            Some((
                can_equip_result,
                SwapItemMergeFillResult::EquipMergedStack {
                    dest: equip_dest,
                    auto_unequip_offhand: true,
                },
            ))
        } else {
            None
        };

        let Some((validation_result, move_result)) = destination_kind else {
            return SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::InvalidDestinationNoop,
                send_refund_info: false,
            };
        };

        if validation_result != InventoryResult::Ok {
            return SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::ContinueToRealSwap,
                send_refund_info: false,
            };
        }

        if source_count.saturating_add(destination_count) <= source_max_stack_size {
            return SwapItemMergeFillPlan {
                result: move_result,
                send_refund_info: true,
            };
        }

        SwapItemMergeFillPlan {
            result: SwapItemMergeFillResult::PartialFill {
                source_remaining_count: source_count
                    .saturating_add(destination_count)
                    .saturating_sub(source_max_stack_size),
                destination_count: source_max_stack_size,
                send_updates: is_in_world,
            },
            send_refund_info: true,
        }
    }

    pub fn swap_item_real_swap_validation_plan(
        &self,
        src: u16,
        dst: u16,
        source_can_store_result: InventoryResult,
        source_can_bank_result: InventoryResult,
        source_can_equip_result: InventoryResult,
        source_equip_dest: u16,
        source_equip_dest_can_unequip_result: InventoryResult,
        destination_can_store_result: InventoryResult,
        destination_can_bank_result: InventoryResult,
        destination_can_equip_result: InventoryResult,
        destination_equip_dest: u16,
        destination_equip_dest_can_unequip_result: InventoryResult,
    ) -> SwapItemRealSwapValidationPlan {
        let (source_result, source_target) = swap_item_real_swap_target_for_destination(
            dst,
            source_can_store_result,
            source_can_bank_result,
            source_can_equip_result,
            source_equip_dest,
            source_equip_dest_can_unequip_result,
        );
        if source_result != InventoryResult::Ok {
            return SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Error {
                    result: source_result,
                    subject: SwapItemRealSwapValidationSubject::Source,
                },
            };
        }

        let (destination_result, destination_target) = swap_item_real_swap_target_for_destination(
            src,
            destination_can_store_result,
            destination_can_bank_result,
            destination_can_equip_result,
            destination_equip_dest,
            destination_equip_dest_can_unequip_result,
        );
        if destination_result != InventoryResult::Ok {
            return SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Error {
                    result: destination_result,
                    subject: SwapItemRealSwapValidationSubject::Destination,
                },
            };
        }

        SwapItemRealSwapValidationPlan {
            result: SwapItemRealSwapValidationResult::Continue {
                source_target,
                destination_target,
            },
        }
    }

    pub fn swap_item_bag_exchange_plan(
        &self,
        src: u16,
        dst: u16,
        source_bag: Option<SwapBagRef<'_>>,
        destination_bag: Option<SwapBagRef<'_>>,
    ) -> SwapItemBagExchangePlan {
        let (Some(source_bag), Some(destination_bag)) = (source_bag, destination_bag) else {
            return SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Continue,
            };
        };

        let Some((empty_bag_is_source, empty_bag, full_bag)) =
            (if source_bag.is_empty && !is_bag_pos(src) {
                Some((true, source_bag, destination_bag))
            } else if destination_bag.is_empty && !is_bag_pos(dst) {
                Some((false, destination_bag, source_bag))
            } else {
                None
            })
        else {
            return SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Continue,
            };
        };

        let mut count = 0u8;
        for slot in 0..full_bag.bag_size {
            if let Some(item_ref) = full_bag.items.iter().find(|item| item.slot == slot) {
                if !item_ref.can_go_into_empty_bag {
                    return SwapItemBagExchangePlan {
                        result: SwapItemBagExchangeResult::Error(InventoryResult::BagInBag),
                    };
                }
                count = count.saturating_add(1);
            }
        }

        if count > empty_bag.bag_size {
            return SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Error(InventoryResult::CantSwap),
            };
        }

        let mut moves = Vec::new();
        let mut to_slot = 0u8;
        for slot in 0..full_bag.bag_size {
            if full_bag.items.iter().any(|item| item.slot == slot) {
                moves.push(SwapBagItemMove {
                    from_slot: slot,
                    to_slot,
                });
                to_slot = to_slot.saturating_add(1);
            }
        }

        SwapItemBagExchangePlan {
            result: SwapItemBagExchangeResult::Exchange {
                empty_bag_is_source,
                moves,
            },
        }
    }

    pub fn swap_item_real_swap_execution_plan(
        &self,
        src: u16,
        dst: u16,
        source_target: SwapItemRealSwapTarget,
        destination_target: SwapItemRealSwapTarget,
        ae_loot_view_not_empty: bool,
        source_bag_has_looted_item: bool,
        destination_bag_has_looted_item: bool,
    ) -> SwapItemRealSwapExecutionPlan {
        let [src_bag, src_slot] = src.to_be_bytes();
        let [dst_bag, dst_slot] = dst.to_be_bytes();
        let apply_item_dependent_auras = (src_bag == INVENTORY_SLOT_BAG_0
            && src_slot < INVENTORY_SLOT_BAG_END)
            || (dst_bag == INVENTORY_SLOT_BAG_0 && dst_slot < INVENTORY_SLOT_BAG_END);
        let release_loot = ae_loot_view_not_empty
            && ((is_bag_pos(src) && source_bag_has_looted_item)
                || (is_bag_pos(dst) && destination_bag_has_looted_item));

        SwapItemRealSwapExecutionPlan {
            remove_destination_update: false,
            remove_source_update: false,
            source_target,
            destination_target,
            apply_item_dependent_auras,
            release_loot,
            auto_unequip_offhand: true,
        }
    }

    pub fn swap_item_orchestration_plan(
        &self,
        preflight: SwapItemPreflightPlan,
        empty_destination: Option<SwapItemEmptyDestinationPlan>,
        merge_fill: Option<SwapItemMergeFillPlan>,
        real_swap_validation: Option<SwapItemRealSwapValidationPlan>,
        bag_exchange: Option<SwapItemBagExchangePlan>,
        real_swap_execution: Option<SwapItemRealSwapExecutionPlan>,
    ) -> SwapItemOrchestrationPlan {
        match preflight.result {
            SwapItemPreflightResult::NoSource => {
                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::NoSource,
                };
            }
            SwapItemPreflightResult::ChildRedirect {
                first_src,
                first_dst,
                second_src,
                second_dst,
            } => {
                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::ChildRedirect {
                        first_src,
                        first_dst,
                        second_src,
                        second_dst,
                    },
                };
            }
            SwapItemPreflightResult::Error(result) => {
                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::Error {
                        result,
                        item_order: SwapItemErrorItemOrder::SourceDestination,
                    },
                };
            }
            SwapItemPreflightResult::Continue => {}
        }

        let Some(empty_destination) = empty_destination else {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(
                    SwapItemMissingPhase::EmptyDestination,
                ),
            };
        };
        match empty_destination.result {
            SwapItemEmptyDestinationResult::OccupiedDestination => {}
            SwapItemEmptyDestinationResult::Error(result) => {
                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::Error {
                        result,
                        item_order: SwapItemErrorItemOrder::SourceOnly,
                    },
                };
            }
            _ => {
                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::EmptyDestination(empty_destination),
                };
            }
        }

        let Some(merge_fill) = merge_fill else {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(SwapItemMissingPhase::MergeFill),
            };
        };
        if merge_fill.result != SwapItemMergeFillResult::ContinueToRealSwap {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MergeFill(merge_fill),
            };
        }

        let Some(real_swap_validation) = real_swap_validation else {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(
                    SwapItemMissingPhase::RealSwapValidation,
                ),
            };
        };
        let (source_target, destination_target) = match real_swap_validation.result {
            SwapItemRealSwapValidationResult::Error { result, subject } => {
                let item_order = match subject {
                    SwapItemRealSwapValidationSubject::Source => {
                        SwapItemErrorItemOrder::SourceDestination
                    }
                    SwapItemRealSwapValidationSubject::Destination => {
                        SwapItemErrorItemOrder::DestinationSource
                    }
                };

                return SwapItemOrchestrationPlan {
                    result: SwapItemOrchestrationResult::Error { result, item_order },
                };
            }
            SwapItemRealSwapValidationResult::Continue {
                source_target,
                destination_target,
            } => (source_target, destination_target),
        };

        let Some(bag_exchange) = bag_exchange else {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(
                    SwapItemMissingPhase::BagExchange,
                ),
            };
        };
        if let SwapItemBagExchangeResult::Error(result) = &bag_exchange.result {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::Error {
                    result: *result,
                    item_order: SwapItemErrorItemOrder::SourceDestination,
                },
            };
        }

        let Some(real_swap_execution) = real_swap_execution else {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(
                    SwapItemMissingPhase::RealSwapExecution,
                ),
            };
        };
        if real_swap_execution.source_target != source_target
            || real_swap_execution.destination_target != destination_target
        {
            return SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::InconsistentRealSwapTargets {
                    validation_source_target: source_target,
                    validation_destination_target: destination_target,
                    execution_source_target: real_swap_execution.source_target,
                    execution_destination_target: real_swap_execution.destination_target,
                },
            };
        }

        SwapItemOrchestrationPlan {
            result: SwapItemOrchestrationResult::RealSwap {
                bag_exchange,
                execution: real_swap_execution,
            },
        }
    }

    pub fn store_bag_item(
        &mut self,
        bag: u8,
        slot: u8,
        guid: ObjectGuid,
    ) -> Result<(), PlayerStorageError> {
        let bag_storage = self
            .inventory
            .bags
            .get_mut(bag as usize)
            .and_then(Option::as_mut)
            .ok_or(PlayerStorageError::UnknownBag(bag))?;
        if slot as usize >= MAX_BAG_SIZE || slot >= bag_storage.bag_size {
            return Err(PlayerStorageError::InvalidBagItemSlot(slot));
        }

        bag_storage.set_item(slot, Some(guid));
        Ok(())
    }

    pub fn store_bag_item_object(
        &mut self,
        bag_slot: u8,
        bag: &mut Bag,
        item_slot: u8,
        item: &mut Item,
        count: u32,
    ) -> Result<(), PlayerStorageError> {
        let bag_guid = bag.item().object().guid();
        let bag_storage = self
            .inventory
            .bags
            .get(bag_slot as usize)
            .and_then(Option::as_ref)
            .ok_or(PlayerStorageError::UnknownBag(bag_slot))?;

        if bag_storage.bag_guid != bag_guid {
            return Err(PlayerStorageError::MismatchedBagGuid {
                bag: bag_slot,
                expected: bag_storage.bag_guid,
                actual: bag_guid,
            });
        }

        if item_slot as usize >= MAX_BAG_SIZE || item_slot >= bag_storage.bag_size {
            return Err(PlayerStorageError::InvalidBagItemSlot(item_slot));
        }

        if bag_storage.item_by_pos(item_slot).is_some() {
            return Err(PlayerStorageError::OccupiedBagItemSlot {
                bag: bag_slot,
                slot: item_slot,
            });
        }

        item.set_count(count);
        item.bind_if_stored(false);
        bag.store_item(item_slot, item);
        self.store_bag_item(bag_slot, item_slot, item.object().guid())?;
        item.set_state(ItemUpdateState::Changed);
        bag.item_mut().set_state(ItemUpdateState::Changed);
        Ok(())
    }

    pub fn store_cloned_bag_item_object(
        &mut self,
        bag_slot: u8,
        bag: &mut Bag,
        item_slot: u8,
        source: &Item,
        new_guid: ObjectGuid,
        count: u32,
    ) -> Result<Item, PlayerStorageError> {
        let mut cloned = source.clone_item_for_store(new_guid, Some(self.guid()), count);
        self.store_bag_item_object(bag_slot, bag, item_slot, &mut cloned, count)?;
        Ok(cloned)
    }

    pub fn split_item_to_empty_bag_item_object(
        &mut self,
        bag_slot: u8,
        bag: &mut Bag,
        item_slot: u8,
        source: &mut Item,
        new_guid: ObjectGuid,
        count: u32,
    ) -> Result<Item, PlayerStorageError> {
        validate_split_source(source, count)?;

        let cloned =
            self.store_cloned_bag_item_object(bag_slot, bag, item_slot, source, new_guid, count)?;
        source.set_count(source.count() - count);
        source.set_state(ItemUpdateState::Changed);
        Ok(cloned)
    }

    pub fn merge_bag_item_stack_object(
        &mut self,
        bag_slot: u8,
        bag: &Bag,
        item_slot: u8,
        existing: &mut Item,
        incoming: &mut Item,
        count: u32,
    ) -> Result<(), PlayerStorageError> {
        let bag_guid = bag.item().object().guid();
        let bag_storage = self
            .inventory
            .bags
            .get(bag_slot as usize)
            .and_then(Option::as_ref)
            .ok_or(PlayerStorageError::UnknownBag(bag_slot))?;

        if bag_storage.bag_guid != bag_guid {
            return Err(PlayerStorageError::MismatchedBagGuid {
                bag: bag_slot,
                expected: bag_storage.bag_guid,
                actual: bag_guid,
            });
        }

        if item_slot as usize >= MAX_BAG_SIZE || item_slot >= bag_storage.bag_size {
            return Err(PlayerStorageError::InvalidBagItemSlot(item_slot));
        }

        let Some(expected_guid) = bag_storage.item_by_pos(item_slot) else {
            return Err(PlayerStorageError::EmptyBagItemSlot {
                bag: bag_slot,
                slot: item_slot,
            });
        };

        let bag_slot_guid = bag.item_by_pos(item_slot).unwrap_or(ObjectGuid::EMPTY);
        if bag_slot_guid != expected_guid {
            return Err(PlayerStorageError::MismatchedBagItemGuid {
                bag: bag_slot,
                slot: item_slot,
                expected: expected_guid,
                actual: bag_slot_guid,
            });
        }

        let actual_guid = existing.object().guid();
        if expected_guid != actual_guid {
            return Err(PlayerStorageError::MismatchedBagItemGuid {
                bag: bag_slot,
                slot: item_slot,
                expected: expected_guid,
                actual: actual_guid,
            });
        }

        existing.bind_if_stored(false);
        existing.set_count(existing.count() + count);
        existing.set_state(ItemUpdateState::Changed);

        let owner_guid = self.guid();
        incoming.set_owner_guid(owner_guid);
        incoming.set_not_refundable();
        incoming.clear_soulbound_tradeable();
        incoming.set_state(ItemUpdateState::Removed);
        Ok(())
    }

    pub fn remove_bag_item(
        &mut self,
        bag: u8,
        slot: u8,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        let bag_storage = self
            .inventory
            .bags
            .get_mut(bag as usize)
            .and_then(Option::as_mut)
            .ok_or(PlayerStorageError::UnknownBag(bag))?;
        if slot as usize >= MAX_BAG_SIZE || slot >= bag_storage.bag_size {
            return Err(PlayerStorageError::InvalidBagItemSlot(slot));
        }

        let removed = bag_storage.item_by_pos(slot);
        bag_storage.set_item(slot, None);
        Ok(removed)
    }

    pub fn get_bag_by_pos(&self, bag: u8) -> Option<ObjectGuid> {
        if is_bag_storage_slot(bag) {
            self.inventory.bags[bag as usize].map(|bag| bag.bag_guid)
        } else {
            None
        }
    }

    pub fn get_item_by_pos(&self, bag: u8, slot: u8) -> Option<ObjectGuid> {
        if bag == INVENTORY_SLOT_BAG_0
            && (slot as usize) < PLAYER_SLOT_END
            && !is_buyback_slot(slot)
        {
            return self.inventory.items[slot as usize];
        }

        self.inventory
            .bags
            .get(bag as usize)
            .and_then(|bag| bag.as_ref())
            .and_then(|bag| bag.item_by_pos(slot))
    }

    pub fn get_item_by_packed_pos(&self, pos: u16) -> Option<ObjectGuid> {
        self.get_item_by_pos((pos >> 8) as u8, (pos & 0xFF) as u8)
    }

    pub fn get_item_by_guid(&self, guid: ObjectGuid) -> Option<ObjectGuid> {
        let mut found = false;
        self.for_each_item_guid(ItemSearchLocation::EVERYWHERE, |item_guid| {
            if item_guid == guid {
                found = true;
                ItemSearchCallbackResult::Stop
            } else {
                ItemSearchCallbackResult::Continue
            }
        });

        found.then_some(guid)
    }

    pub fn for_each_item_guid(
        &self,
        location: ItemSearchLocation,
        mut callback: impl FnMut(ObjectGuid) -> ItemSearchCallbackResult,
    ) -> bool {
        if location.contains(ItemSearchLocation::EQUIPMENT) {
            for slot in 0..EQUIPMENT_SLOT_END {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
            for slot in PROFESSION_SLOT_START..PROFESSION_SLOT_END {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
        }

        if location.contains(ItemSearchLocation::INVENTORY) {
            let inventory_end = INVENTORY_SLOT_ITEM_START
                .saturating_add(self.active_data.num_backpack_slots)
                .min(INVENTORY_SLOT_ITEM_END);
            for slot in INVENTORY_SLOT_BAG_START..inventory_end {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
            for slot in KEYRING_SLOT_START..KEYRING_SLOT_END {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
            for slot in CHILD_EQUIPMENT_SLOT_START..CHILD_EQUIPMENT_SLOT_END {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
            for bag_slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
                if self.visit_bag_items(bag_slot, &mut callback) {
                    return false;
                }
            }
        }

        if location.contains(ItemSearchLocation::BANK) {
            for slot in BANK_SLOT_ITEM_START..BANK_SLOT_BAG_END {
                if self.visit_top_slot(slot, &mut callback) {
                    return false;
                }
            }
            for bag_slot in BANK_SLOT_BAG_START..BANK_SLOT_BAG_END {
                if self.visit_bag_items(bag_slot, &mut callback) {
                    return false;
                }
            }
        }

        if location.contains(ItemSearchLocation::REAGENT_BANK) {
            for bag_slot in REAGENT_BAG_SLOT_START..REAGENT_BAG_SLOT_END {
                if self.visit_bag_items(bag_slot, &mut callback) {
                    return false;
                }
            }
        }

        true
    }

    pub fn for_each_item_storage_ref<'a>(
        &self,
        location: ItemSearchLocation,
        stored_items: &'a [ItemStorageRef<'a>],
        mut callback: impl FnMut(ItemStorageRef<'a>) -> ItemSearchCallbackResult,
    ) -> bool {
        self.for_each_item_guid(location, |guid| {
            if let Some(stored) = item_storage_ref_by_guid(stored_items, guid) {
                callback(stored)
            } else {
                ItemSearchCallbackResult::Continue
            }
        })
    }

    pub fn set_buyback_price(&mut self, slot: usize, price: u32) {
        if slot >= BUYBACK_SLOT_COUNT || self.active_data.buyback_price[slot] == price {
            return;
        }

        self.active_data.buyback_price[slot] = price;
        self.mark_active_player_data_array(
            ACTIVE_PLAYER_DATA_BUYBACK_PARENT_BIT,
            ACTIVE_PLAYER_DATA_BUYBACK_PRICE_FIRST_BIT,
            slot,
        );
    }

    pub fn set_buyback_timestamp(&mut self, slot: usize, timestamp: i64) {
        if slot >= BUYBACK_SLOT_COUNT || self.active_data.buyback_timestamp[slot] == timestamp {
            return;
        }

        self.active_data.buyback_timestamp[slot] = timestamp;
        self.mark_active_player_data_array(
            ACTIVE_PLAYER_DATA_BUYBACK_PARENT_BIT,
            ACTIVE_PLAYER_DATA_BUYBACK_TIMESTAMP_FIRST_BIT,
            slot,
        );
    }

    pub fn get_item_from_buyback_slot(&self, slot: u8) -> Option<ObjectGuid> {
        if is_buyback_slot(slot) {
            self.inventory.items[slot as usize]
        } else {
            None
        }
    }

    pub fn remove_item_from_buyback_slot(&mut self, slot: u8) -> Option<ObjectGuid> {
        if !is_buyback_slot(slot) {
            return None;
        }

        let removed = self.inventory.items[slot as usize].take();
        let buyback_index = (slot - BUYBACK_SLOT_START) as usize;
        self.set_inv_slot(slot as usize, ObjectGuid::EMPTY);
        self.set_buyback_price(buyback_index, 0);
        self.set_buyback_timestamp(buyback_index, 0);
        if self.inventory.items[self.inventory.current_buyback_slot as usize].is_some() {
            self.inventory.current_buyback_slot = slot;
        }
        removed
    }

    pub fn remove_item_from_buyback_slot_object(
        &mut self,
        slot: u8,
        item: Option<&mut Item>,
        delete_item: bool,
    ) -> Result<Option<ObjectGuid>, PlayerStorageError> {
        if !is_buyback_slot(slot) {
            return Ok(None);
        }

        let stored_guid = self.inventory.items[slot as usize];
        let mut item = item;
        if let (Some(expected), Some(actual_item)) = (stored_guid, item.as_deref()) {
            let actual = actual_item.object().guid();
            if expected != actual {
                return Err(PlayerStorageError::MismatchedItemGuid {
                    slot,
                    expected,
                    actual,
                });
            }
        }

        if stored_guid.is_some() {
            if let Some(item) = item.as_deref_mut() {
                item.object_mut().remove_from_world();
                if delete_item {
                    item.set_state(ItemUpdateState::Removed);
                }
            }
        }

        Ok(self.remove_item_from_buyback_slot(slot))
    }

    pub fn add_item_to_buyback_slot(&mut self, guid: ObjectGuid, price: u32, timestamp: i64) -> u8 {
        let mut slot = self.inventory.current_buyback_slot;
        if self.inventory.items[slot as usize].is_some() {
            let mut oldest_slot = BUYBACK_SLOT_START;
            let mut oldest_time = self.active_data.buyback_timestamp[0];

            for candidate in BUYBACK_SLOT_START + 1..BUYBACK_SLOT_END {
                let candidate_index = (candidate - BUYBACK_SLOT_START) as usize;
                if self.inventory.items[candidate as usize].is_none() {
                    oldest_slot = candidate;
                    break;
                }
                let candidate_time = self.active_data.buyback_timestamp[candidate_index];
                if oldest_time > candidate_time {
                    oldest_time = candidate_time;
                    oldest_slot = candidate;
                }
            }
            slot = oldest_slot;
        }

        self.remove_item_from_buyback_slot(slot);
        self.inventory.items[slot as usize] = Some(guid);
        let buyback_index = (slot - BUYBACK_SLOT_START) as usize;
        self.set_inv_slot(slot as usize, guid);
        self.set_buyback_price(buyback_index, price);
        self.set_buyback_timestamp(buyback_index, timestamp);

        if self.inventory.current_buyback_slot < BUYBACK_SLOT_END - 1 {
            self.inventory.current_buyback_slot += 1;
        }

        slot
    }

    pub fn add_item_to_buyback_slot_object(
        &mut self,
        item: &Item,
        item_template: Option<&ItemStorageTemplate>,
        game_time: i64,
        login_time: i64,
        overwritten_item: Option<&mut Item>,
    ) -> Result<u8, PlayerStorageError> {
        let mut slot = self.inventory.current_buyback_slot;
        if self.inventory.items[slot as usize].is_some() {
            let mut oldest_slot = BUYBACK_SLOT_START;
            let mut oldest_time = self.active_data.buyback_timestamp[0];

            for candidate in BUYBACK_SLOT_START + 1..BUYBACK_SLOT_END {
                let candidate_index = (candidate - BUYBACK_SLOT_START) as usize;
                if self.inventory.items[candidate as usize].is_none() {
                    oldest_slot = candidate;
                    break;
                }
                let candidate_time = self.active_data.buyback_timestamp[candidate_index];
                if oldest_time > candidate_time {
                    oldest_time = candidate_time;
                    oldest_slot = candidate;
                }
            }
            slot = oldest_slot;
        }

        self.remove_item_from_buyback_slot_object(slot, overwritten_item, true)?;

        let buyback_index = (slot - BUYBACK_SLOT_START) as usize;
        let price = item_template
            .map(|proto| proto.sell_price.wrapping_mul(item.count()))
            .unwrap_or(0);
        let timestamp = (game_time - login_time + (30 * 3600)) as u32 as i64;

        self.inventory.items[slot as usize] = Some(item.object().guid());
        self.set_inv_slot(slot as usize, item.object().guid());
        self.set_buyback_price(buyback_index, price);
        self.set_buyback_timestamp(buyback_index, timestamp);

        if self.inventory.current_buyback_slot < BUYBACK_SLOT_END - 1 {
            self.inventory.current_buyback_slot += 1;
        }

        Ok(slot)
    }

    pub fn add_tradeable_item(&mut self, item: &Item) {
        self.soulbound_tradeable_items.insert(item.object().guid());
    }

    pub fn remove_tradeable_item(&mut self, item: &Item) {
        self.soulbound_tradeable_items.remove(&item.object().guid());
    }

    pub fn update_soulbound_trade_items(
        &mut self,
        items: &[SoulboundTradeableItemRef],
    ) -> Vec<ObjectGuid> {
        let player_guid = self.guid();
        let mut removed = Vec::new();
        self.soulbound_tradeable_items.retain(|guid| {
            let keep = items.iter().any(|item| {
                item.guid == *guid && item.owner_guid == player_guid && !item.trade_expired
            });
            if !keep {
                removed.push(*guid);
            }
            keep
        });
        removed
    }

    pub const fn can_titan_grip(&self) -> bool {
        self.can_titan_grip
    }

    pub const fn titan_grip_penalty_spell_id(&self) -> u32 {
        self.titan_grip_penalty_spell_id
    }

    pub fn set_can_titan_grip(&mut self, value: bool, penalty_spell_id: u32) {
        if value == self.can_titan_grip {
            return;
        }

        self.can_titan_grip = value;
        self.titan_grip_penalty_spell_id = penalty_spell_id;
    }

    pub const fn is_use_equipped_weapon(
        mainhand: bool,
        is_in_feral_form: bool,
        is_disarmed: bool,
    ) -> bool {
        !is_in_feral_form && (!mainhand || !is_disarmed)
    }

    pub fn is_two_hand_used_template(&self, main_template: Option<&ItemStorageTemplate>) -> bool {
        let Some(template) = main_template else {
            return false;
        };

        (template.inventory_type == InventoryType::Weapon2Hand && !self.can_titan_grip)
            || template.inventory_type == InventoryType::Ranged
            || (template.inventory_type == InventoryType::RangedRight
                && template.class_id == ItemClass::Weapon
                && template.subclass_id != ItemSubClassWeapon::Wand as u32)
    }

    pub fn is_using_two_handed_weapon_in_one_hand_template(
        main_template: Option<&ItemStorageTemplate>,
        off_template: Option<&ItemStorageTemplate>,
    ) -> bool {
        if off_template
            .is_some_and(|template| template.inventory_type == InventoryType::Weapon2Hand)
        {
            return true;
        }

        main_template.is_some_and(|template| template.inventory_type == InventoryType::Weapon2Hand)
            && off_template.is_some()
    }

    pub fn check_titan_grip_penalty_action(
        &self,
        using_two_handed_weapon_in_one_hand: bool,
        has_penalty_aura: bool,
    ) -> TitanGripPenaltyAction {
        if !self.can_titan_grip {
            return TitanGripPenaltyAction::None;
        }

        if using_two_handed_weapon_in_one_hand {
            if has_penalty_aura {
                TitanGripPenaltyAction::None
            } else {
                TitanGripPenaltyAction::Cast(self.titan_grip_penalty_spell_id)
            }
        } else {
            TitanGripPenaltyAction::Remove(self.titan_grip_penalty_spell_id)
        }
    }

    pub fn set_power_index(&mut self, power: PowerType, index: Option<usize>) {
        self.unit.set_power_index(power, index);
    }

    pub fn changed_object_type_mask(&self, include_active_player: bool) -> u32 {
        self.unit.changed_object_type_mask()
            | if self.player_data_changes.is_any_set() {
                1 << TYPEID_PLAYER
            } else {
                0
            }
            | if include_active_player && self.active_player_data_changes.is_any_set() {
                1 << TYPEID_ACTIVE_PLAYER
            } else {
                0
            }
    }

    pub fn values_update(&self, include_active_player: bool) -> PlayerValuesUpdate {
        let unit_update = self.unit.values_update();
        PlayerValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(include_active_player),
            object_data: unit_update.object_data,
            unit_data: unit_update.unit_data,
            player_data: self
                .player_data_changes
                .is_any_set()
                .then(|| PlayerDataUpdate {
                    mask: self.player_data_changes.clone(),
                    values: self.data,
                }),
            active_player_data: (include_active_player
                && self.active_player_data_changes.is_any_set())
            .then(|| ActivePlayerDataUpdate {
                mask: self.active_player_data_changes.clone(),
                values: self.active_data,
            }),
        }
    }

    fn set_player_u32(
        &mut self,
        bit: usize,
        value: u32,
        field: impl FnOnce(&mut PlayerDataValues) -> &mut u32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_player_data(bit);
        }
    }

    fn set_player_u8(
        &mut self,
        bit: usize,
        value: u8,
        field: impl FnOnce(&mut PlayerDataValues) -> &mut u8,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_player_data(bit);
        }
    }

    fn set_player_guid(
        &mut self,
        bit: usize,
        value: ObjectGuid,
        field: impl FnOnce(&mut PlayerDataValues) -> &mut ObjectGuid,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_player_data(bit);
        }
    }

    fn set_active_u64(
        &mut self,
        bit: usize,
        value: u64,
        field: impl FnOnce(&mut ActivePlayerDataValues) -> &mut u64,
    ) {
        let target = field(&mut self.active_data);
        if *target != value {
            *target = value;
            self.mark_active_player_data(bit);
        }
    }

    fn set_active_i32(
        &mut self,
        bit: usize,
        value: i32,
        field: impl FnOnce(&mut ActivePlayerDataValues) -> &mut i32,
    ) {
        let target = field(&mut self.active_data);
        if *target != value {
            *target = value;
            self.mark_active_player_data(bit);
        }
    }

    fn set_active_u8(
        &mut self,
        bit: usize,
        value: u8,
        field: impl FnOnce(&mut ActivePlayerDataValues) -> &mut u8,
    ) {
        let target = field(&mut self.active_data);
        if *target != value {
            *target = value;
            self.mark_active_player_data(bit);
        }
    }

    fn mark_player_data(&mut self, bit: usize) {
        self.player_data_changes.set(PLAYER_DATA_PARENT_BIT);
        self.player_data_changes.set(bit);
    }

    fn mark_player_data_array(
        &mut self,
        parent_bit: usize,
        first_element_bit: usize,
        index: usize,
    ) {
        self.player_data_changes.set(parent_bit);
        self.player_data_changes.set(first_element_bit + index);
    }

    fn mark_active_player_data(&mut self, bit: usize) {
        self.active_player_data_changes
            .set(ACTIVE_PLAYER_DATA_PARENT_BIT);
        self.active_player_data_changes.set(bit);
    }

    fn mark_active_player_data_array(
        &mut self,
        parent_bit: usize,
        first_element_bit: usize,
        index: usize,
    ) {
        self.active_player_data_changes.set(parent_bit);
        self.active_player_data_changes
            .set(first_element_bit + index);
    }

    fn visit_top_slot(
        &self,
        slot: u8,
        callback: &mut impl FnMut(ObjectGuid) -> ItemSearchCallbackResult,
    ) -> bool {
        self.inventory.items[slot as usize]
            .map(|guid| matches!(callback(guid), ItemSearchCallbackResult::Stop))
            .unwrap_or(false)
    }

    fn visit_bag_items(
        &self,
        bag_slot: u8,
        callback: &mut impl FnMut(ObjectGuid) -> ItemSearchCallbackResult,
    ) -> bool {
        let Some(bag) = self.inventory.bags[bag_slot as usize] else {
            return false;
        };

        bag.slots
            .iter()
            .take(bag.bag_size as usize)
            .filter_map(|guid| *guid)
            .any(|guid| matches!(callback(guid), ItemSearchCallbackResult::Stop))
    }
}

fn equip_slot_candidates(args: FindEquipSlotArgs<'_>) -> [u8; 4] {
    let mut slots = [NULL_SLOT; 4];
    match args.proto.inventory_type {
        InventoryType::Head => slots[0] = EQUIPMENT_SLOT_HEAD,
        InventoryType::Neck => slots[0] = EQUIPMENT_SLOT_NECK,
        InventoryType::Shoulders => slots[0] = EQUIPMENT_SLOT_SHOULDERS,
        InventoryType::Body => slots[0] = EQUIPMENT_SLOT_BODY,
        InventoryType::Chest | InventoryType::Robe => slots[0] = EQUIPMENT_SLOT_CHEST,
        InventoryType::Waist => slots[0] = EQUIPMENT_SLOT_WAIST,
        InventoryType::Legs => slots[0] = EQUIPMENT_SLOT_LEGS,
        InventoryType::Feet => slots[0] = EQUIPMENT_SLOT_FEET,
        InventoryType::Wrists => slots[0] = EQUIPMENT_SLOT_WRISTS,
        InventoryType::Hands => slots[0] = EQUIPMENT_SLOT_HANDS,
        InventoryType::Finger => {
            slots[0] = EQUIPMENT_SLOT_FINGER1;
            slots[1] = EQUIPMENT_SLOT_FINGER2;
        }
        InventoryType::Trinket => {
            slots[0] = EQUIPMENT_SLOT_TRINKET1;
            slots[1] = EQUIPMENT_SLOT_TRINKET2;
        }
        InventoryType::Cloak => slots[0] = EQUIPMENT_SLOT_BACK,
        InventoryType::Weapon => {
            slots[0] = EQUIPMENT_SLOT_MAINHAND;
            if args.can_dual_wield {
                slots[1] = EQUIPMENT_SLOT_OFFHAND;
            }
        }
        InventoryType::Shield | InventoryType::WeaponOffhand | InventoryType::Holdable => {
            slots[0] = EQUIPMENT_SLOT_OFFHAND;
        }
        InventoryType::Ranged | InventoryType::WeaponMainhand | InventoryType::RangedRight => {
            slots[0] = EQUIPMENT_SLOT_MAINHAND;
        }
        InventoryType::Weapon2Hand => {
            slots[0] = EQUIPMENT_SLOT_MAINHAND;
            if args.can_dual_wield && args.can_titan_grip {
                slots[1] = EQUIPMENT_SLOT_OFFHAND;
            }
        }
        InventoryType::Tabard => slots[0] = EQUIPMENT_SLOT_TABARD,
        InventoryType::Bag => {
            slots[0] = INVENTORY_SLOT_BAG_START;
            slots[1] = INVENTORY_SLOT_BAG_START + 1;
            slots[2] = INVENTORY_SLOT_BAG_START + 2;
            slots[3] = INVENTORY_SLOT_BAG_START + 3;
        }
        InventoryType::ProfessionTool | InventoryType::ProfessionGear => {
            if args.proto.class_id != ItemClass::Profession || !args.has_required_profession_skill {
                return slots;
            }

            let is_tool = args.proto.inventory_type == InventoryType::ProfessionTool;
            match args.proto.subclass_id {
                value if value == ItemSubclassProfession::Cooking as u32 => {
                    slots[0] = if is_tool {
                        PROFESSION_SLOT_COOKING_TOOL
                    } else {
                        PROFESSION_SLOT_COOKING_GEAR1
                    };
                }
                value if value == ItemSubclassProfession::Fishing as u32 => {
                    if !is_tool {
                        return [NULL_SLOT; 4];
                    }
                    slots[0] = PROFESSION_SLOT_FISHING_TOOL;
                }
                value
                    if value == ItemSubclassProfession::Blacksmithing as u32
                        || value == ItemSubclassProfession::Leatherworking as u32
                        || value == ItemSubclassProfession::Alchemy as u32
                        || value == ItemSubclassProfession::Herbalism as u32
                        || value == ItemSubclassProfession::Mining as u32
                        || value == ItemSubclassProfession::Tailoring as u32
                        || value == ItemSubclassProfession::Engineering as u32
                        || value == ItemSubclassProfession::Enchanting as u32
                        || value == ItemSubclassProfession::Skinning as u32
                        || value == ItemSubclassProfession::Jewelcrafting as u32
                        || value == ItemSubclassProfession::Inscription as u32 =>
                {
                    let Some(profession_slot) = args.profession_slot else {
                        return [NULL_SLOT; 4];
                    };

                    if is_tool {
                        slots[0] = PROFESSION_SLOT_PROFESSION1_TOOL
                            + profession_slot * PROFESSION_SLOT_MAX_COUNT;
                    } else {
                        // C++ writes slots[0] twice here, so primary profession gear1 is unreachable.
                        slots[0] = PROFESSION_SLOT_PROFESSION1_GEAR1
                            + profession_slot * PROFESSION_SLOT_MAX_COUNT;
                        slots[0] = PROFESSION_SLOT_PROFESSION1_GEAR2
                            + profession_slot * PROFESSION_SLOT_MAX_COUNT;
                    }
                }
                _ => return [NULL_SLOT; 4],
            }
        }
        _ => return slots,
    }
    slots
}

fn paired_unique_ignore_slot(slot: u8) -> Option<u8> {
    match slot {
        EQUIPMENT_SLOT_MAINHAND => Some(EQUIPMENT_SLOT_OFFHAND),
        EQUIPMENT_SLOT_OFFHAND => Some(EQUIPMENT_SLOT_MAINHAND),
        EQUIPMENT_SLOT_FINGER1 => Some(EQUIPMENT_SLOT_FINGER2),
        EQUIPMENT_SLOT_FINGER2 => Some(EQUIPMENT_SLOT_FINGER1),
        EQUIPMENT_SLOT_TRINKET1 => Some(EQUIPMENT_SLOT_TRINKET2),
        EQUIPMENT_SLOT_TRINKET2 => Some(EQUIPMENT_SLOT_TRINKET1),
        PROFESSION_SLOT_PROFESSION1_GEAR1 => Some(PROFESSION_SLOT_PROFESSION1_GEAR2),
        PROFESSION_SLOT_PROFESSION1_GEAR2 => Some(PROFESSION_SLOT_PROFESSION1_GEAR1),
        PROFESSION_SLOT_PROFESSION2_GEAR1 => Some(PROFESSION_SLOT_PROFESSION2_GEAR2),
        PROFESSION_SLOT_PROFESSION2_GEAR2 => Some(PROFESSION_SLOT_PROFESSION2_GEAR1),
        _ => None,
    }
}

fn has_equipped_item_entry(
    equipped_items: &[ItemStorageRef<'_>],
    entry: u32,
    except_slot: u8,
) -> bool {
    equipped_items.iter().any(|stored| {
        stored.bag == INVENTORY_SLOT_BAG_0
            && stored.slot != except_slot
            && stored.item.object().entry() == entry
    })
}

fn has_equipped_gem_entry(equipped_gems: &[EquippedGemRef], entry: u32, except_slot: u8) -> bool {
    equipped_gems
        .iter()
        .any(|gem| gem.slot != except_slot && gem.entry == entry)
}

fn equipped_item_limit_category_count(
    equipped_items: &[ItemStorageRef<'_>],
    limit_category: u32,
    except_slot: u8,
) -> u32 {
    equipped_items
        .iter()
        .filter(|stored| {
            stored.bag == INVENTORY_SLOT_BAG_0
                && stored.slot != except_slot
                && stored
                    .template
                    .is_some_and(|template| template.item_limit_category == limit_category)
        })
        .map(|stored| stored.item.count())
        .sum()
}

fn equipped_gem_limit_category_count(
    equipped_gems: &[EquippedGemRef],
    limit_category: u32,
    except_slot: u8,
) -> u32 {
    equipped_gems
        .iter()
        .filter(|gem| gem.slot != except_slot && gem.limit_category == limit_category)
        .count() as u32
}

fn destroy_item_count_item_by_pos<'a>(
    items: &[DestroyItemCountItemRef<'a>],
    bag: u8,
    slot: u8,
) -> Option<DestroyItemCountItemRef<'a>> {
    items
        .iter()
        .find(|item_ref| item_ref.bag == bag && item_ref.slot == slot)
        .copied()
}

fn destroy_item_count_consider_item(
    plan: &mut DestroyItemCountPlan,
    item_ref: DestroyItemCountItemRef<'_>,
    item_entry: u32,
    requested_count: u32,
    require_unequip_for_full_stack: bool,
    unequip_check: bool,
) {
    if plan.removed_count >= requested_count
        || item_ref.item.object().entry() != item_entry
        || item_ref.item.is_in_trade()
    {
        return;
    }

    let needed = requested_count - plan.removed_count;
    let item_count = item_ref.item.count();
    if item_count <= needed {
        if require_unequip_for_full_stack
            && unequip_check
            && item_ref.can_unequip_result != InventoryResult::Ok
        {
            return;
        }

        plan.actions.push(DestroyItemCountAction {
            bag: item_ref.bag,
            slot: item_ref.slot,
            removed_count: item_count,
            remaining_count: 0,
            destroy_stack: true,
        });
        plan.removed_count += item_count;
    } else {
        plan.actions.push(DestroyItemCountAction {
            bag: item_ref.bag,
            slot: item_ref.slot,
            removed_count: needed,
            remaining_count: item_count - needed,
            destroy_stack: false,
        });
        plan.removed_count = requested_count;
    }
}

fn destroy_item_count_scan_top_level_range(
    plan: &mut DestroyItemCountPlan,
    items: &[DestroyItemCountItemRef<'_>],
    item_entry: u32,
    requested_count: u32,
    start: u8,
    end: u8,
    require_unequip_for_full_stack: bool,
    unequip_check: bool,
) {
    for slot in start..end {
        if let Some(item_ref) = destroy_item_count_item_by_pos(items, INVENTORY_SLOT_BAG_0, slot) {
            destroy_item_count_consider_item(
                plan,
                item_ref,
                item_entry,
                requested_count,
                require_unequip_for_full_stack,
                unequip_check,
            );
            if plan.removed_count >= requested_count {
                return;
            }
        }
    }
}

fn destroy_item_count_scan_bag_ranges(
    plan: &mut DestroyItemCountPlan,
    items: &[DestroyItemCountItemRef<'_>],
    item_entry: u32,
    requested_count: u32,
    start_bag: u8,
    end_bag: u8,
) {
    for bag in start_bag..end_bag {
        for slot in 0..MAX_BAG_SIZE as u8 {
            if let Some(item_ref) = destroy_item_count_item_by_pos(items, bag, slot) {
                destroy_item_count_consider_item(
                    plan,
                    item_ref,
                    item_entry,
                    requested_count,
                    false,
                    false,
                );
                if plan.removed_count >= requested_count {
                    return;
                }
            }
        }
    }
}

fn destroy_filtered_item_by_pos(
    items: &[DestroyFilteredItemRef],
    bag: u8,
    slot: u8,
) -> Option<DestroyFilteredItemRef> {
    items
        .iter()
        .find(|item_ref| item_ref.bag == bag && item_ref.slot == slot)
        .copied()
}

fn destroy_filtered_consider_item(
    actions: &mut Vec<DestroyFilteredItemAction>,
    item_ref: DestroyFilteredItemRef,
) {
    if item_ref.should_destroy {
        actions.push(DestroyFilteredItemAction {
            bag: item_ref.bag,
            slot: item_ref.slot,
        });
    }
}

fn destroy_filtered_scan_top_level_range(
    actions: &mut Vec<DestroyFilteredItemAction>,
    items: &[DestroyFilteredItemRef],
    start: u8,
    end: u8,
) {
    for slot in start..end {
        if let Some(item_ref) = destroy_filtered_item_by_pos(items, INVENTORY_SLOT_BAG_0, slot) {
            destroy_filtered_consider_item(actions, item_ref);
        }
    }
}

fn destroy_filtered_scan_bag_ranges(
    actions: &mut Vec<DestroyFilteredItemAction>,
    items: &[DestroyFilteredItemRef],
    start_bag: u8,
    end_bag: u8,
) {
    for bag in start_bag..end_bag {
        for slot in 0..MAX_BAG_SIZE as u8 {
            if let Some(item_ref) = destroy_filtered_item_by_pos(items, bag, slot) {
                destroy_filtered_consider_item(actions, item_ref);
            }
        }
    }
}

fn swap_item_real_swap_target_for_destination(
    destination: u16,
    can_store_result: InventoryResult,
    can_bank_result: InventoryResult,
    can_equip_result: InventoryResult,
    equip_dest: u16,
    equip_dest_can_unequip_result: InventoryResult,
) -> (InventoryResult, SwapItemRealSwapTarget) {
    if is_inventory_packed_pos(destination) {
        return (can_store_result, SwapItemRealSwapTarget::Inventory);
    }

    if is_bank_packed_pos(destination) {
        return (can_bank_result, SwapItemRealSwapTarget::Bank);
    }

    if is_equipment_packed_pos(destination) {
        if can_equip_result == InventoryResult::Ok {
            return (
                equip_dest_can_unequip_result,
                SwapItemRealSwapTarget::Equip { dest: equip_dest },
            );
        }

        return (
            can_equip_result,
            SwapItemRealSwapTarget::Equip { dest: equip_dest },
        );
    }

    (InventoryResult::Ok, SwapItemRealSwapTarget::None)
}

fn is_bag_storage_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
        || (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&slot)
        || (REAGENT_BAG_SLOT_START..REAGENT_BAG_SLOT_END).contains(&slot)
}

fn is_buyback_slot(slot: u8) -> bool {
    (BUYBACK_SLOT_START..BUYBACK_SLOT_END).contains(&slot)
}

fn validate_split_source(source: &Item, count: u32) -> Result<(), PlayerStorageError> {
    if source.loot_generated() {
        return Err(PlayerStorageError::SplitItemLootGenerated);
    }

    let available = source.count();
    if count == 0 || available == count {
        return Err(PlayerStorageError::InvalidSplitCount {
            available,
            requested: count,
        });
    }

    if available < count {
        return Err(PlayerStorageError::TooFewItemsToSplit {
            available,
            requested: count,
        });
    }

    if source.is_in_trade() {
        return Err(PlayerStorageError::SplitItemInTrade);
    }

    Ok(())
}

fn can_store_item_error(
    result: InventoryResult,
    count: u32,
    no_similar_count: u32,
) -> CanStoreItemOutcome {
    CanStoreItemOutcome {
        result,
        no_space_count: Some(count + no_similar_count),
    }
}

fn can_store_item_count_zero(count: u32, no_similar_count: u32) -> Option<CanStoreItemOutcome> {
    (count == 0).then(|| {
        if no_similar_count == 0 {
            CanStoreItemOutcome {
                result: InventoryResult::Ok,
                no_space_count: None,
            }
        } else {
            can_store_item_error(InventoryResult::ItemMaxCount, count, no_similar_count)
        }
    })
}

fn can_equip_item_outcome(result: InventoryResult) -> CanEquipItemOutcome {
    CanEquipItemOutcome {
        result,
        dest: 0,
        unique_ignore_slot: None,
    }
}

fn can_take_more_similar_ok() -> CanTakeMoreSimilarItemsOutcome {
    CanTakeMoreSimilarItemsOutcome {
        result: InventoryResult::Ok,
        no_space_count: None,
        offending_item_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{
        BagFamilyMask, InventoryResult, InventoryType, ItemBondingType, ItemClass, ItemContext,
        ItemFieldFlags, ItemSubClassContainer, ItemSubclassProfession,
    };

    fn can_store_args<'a>(
        bag: u8,
        slot: u8,
        proto: Option<&'a ItemStorageTemplate>,
        count: u32,
    ) -> CanStoreItemArgs<'a> {
        CanStoreItemArgs {
            bag,
            slot,
            entry: proto.map_or(0, |proto| proto.entry),
            count,
            proto,
            source_item: None,
            source_is_not_empty_bag: false,
            source_bop_trade_allowed_for_player: false,
            swap: false,
            limit_category: None,
            slot_items: &[],
            stored_items: &[],
            bag_templates: &[],
        }
    }

    fn item_with_guid_entry(low: i64, entry: u32) -> Item {
        let mut item = Item::default();
        item.object_mut().create(ObjectGuid::create_item(1, low));
        item.object_mut().set_entry(entry);
        item
    }

    fn can_bank_args<'a>(
        bag: u8,
        slot: u8,
        proto: Option<&'a ItemStorageTemplate>,
        source_item: Option<&'a Item>,
    ) -> CanBankItemArgs<'a> {
        CanBankItemArgs {
            bag,
            slot,
            proto,
            source_item,
            source_is_not_empty_bag: false,
            source_is_bag: false,
            source_is_currency_token: false,
            source_bop_trade_allowed_for_player: false,
            swap: false,
            can_use_result: InventoryResult::Ok,
            limit_category: None,
            slot_items: &[],
            stored_items: &[],
            bag_templates: &[],
        }
    }

    fn find_equip_args<'a>(
        proto: &'a ItemStorageTemplate,
        slot: u8,
        swap: bool,
        equipped_items: &'a [ItemSlotRef<'a>],
    ) -> FindEquipSlotArgs<'a> {
        FindEquipSlotArgs {
            proto,
            slot,
            swap,
            can_dual_wield: false,
            can_titan_grip: false,
            is_two_hand_used: false,
            has_required_profession_skill: false,
            profession_slot: None,
            equipped_items,
        }
    }

    fn can_equip_args<'a>(
        slot: u8,
        proto: Option<&'a ItemStorageTemplate>,
        source_item: Option<&'a Item>,
    ) -> CanEquipItemArgs<'a> {
        CanEquipItemArgs {
            slot,
            proto,
            source_item,
            source_bop_trade_allowed_for_player: false,
            swap: false,
            not_loading: true,
            is_stunned: false,
            is_charmed: false,
            is_in_combat: false,
            is_in_progress_arena: false,
            weapon_change_timer_active: false,
            current_generic_spell_allows_equip: None,
            current_channeled_spell_allows_equip: None,
            heirloom_required_level_failed: false,
            can_use_result: InventoryResult::Ok,
            can_equip_unique_result: InventoryResult::Ok,
            can_dual_wield: false,
            can_titan_grip: false,
            is_two_hand_used: false,
            proto_always_allow_dual_wield: false,
            has_required_profession_skill: false,
            profession_slot: None,
            offhand_can_unequip_result: InventoryResult::Ok,
            offhand_can_store_result: InventoryResult::Ok,
            limit_category: None,
            equipped_items: &[],
            stored_items: &[],
        }
    }

    fn can_unequip_args<'a>(
        pos: u16,
        proto: Option<&'a ItemStorageTemplate>,
        source_item: Option<&'a Item>,
    ) -> CanUnequipItemArgs<'a> {
        CanUnequipItemArgs {
            pos,
            source_item,
            proto,
            swap: false,
            source_is_not_empty_bag: false,
            is_charmed: false,
            is_in_combat: false,
            is_in_progress_arena: false,
        }
    }

    fn can_use_template_args<'a>(
        proto: Option<&'a ItemStorageTemplate>,
    ) -> CanUseItemTemplateArgs<'a> {
        CanUseItemTemplateArgs {
            proto,
            skip_required_level_check: false,
            player_level: 70,
            team: TEAM_HORDE_ID,
            allowable_class_matches: true,
            allowable_race_matches: true,
            internal_item: false,
            faction_horde: false,
            faction_alliance: false,
            required_skill: 0,
            required_skill_rank: 0,
            required_skill_value: 0,
            required_spell: 0,
            has_required_spell: false,
            base_required_level: 0,
            holiday_id: 0,
            holiday_active: false,
            required_reputation_faction: 0,
            required_reputation_rank: 0,
            player_reputation_rank: 0,
            effect0_spell_id: None,
            effect1_spell_id: None,
            has_effect1_spell: false,
            artifact_specialization: None,
            primary_specialization: 0,
        }
    }

    fn can_use_args<'a>(
        proto: Option<&'a ItemStorageTemplate>,
        source_item: Option<&'a Item>,
    ) -> CanUseItemArgs<'a> {
        CanUseItemArgs {
            source_item,
            proto,
            not_loading: true,
            is_alive: true,
            player_level: 70,
            item_required_level: 0,
            source_bop_trade_allowed_for_player: false,
            template_args: can_use_template_args(proto),
            item_skill: 0,
            item_skill_value: 0,
            has_item_skill: false,
            player_class: CLASS_WARRIOR,
            proto_is_heirloom: false,
        }
    }

    fn can_equip_unique_template_args<'a>(
        proto: Option<&'a ItemStorageTemplate>,
    ) -> CanEquipUniqueItemTemplateArgs<'a> {
        CanEquipUniqueItemTemplateArgs {
            proto,
            except_slot: NULL_SLOT,
            limit_count: 1,
            unique_equippable: false,
            limit_category: None,
            equipped_items: &[],
            equipped_gems: &[],
        }
    }

    fn can_equip_unique_args<'a>(
        source_item: Option<&'a Item>,
        proto: Option<&'a ItemStorageTemplate>,
    ) -> CanEquipUniqueItemArgs<'a> {
        CanEquipUniqueItemArgs {
            source_item,
            proto,
            except_slot: NULL_SLOT,
            limit_count: 1,
            unique_equippable: false,
            limit_category: None,
            equipped_items: &[],
            equipped_gems: &[],
            socketed_gems: &[],
        }
    }

    #[test]
    fn player_constructor_matches_cpp_base_state() {
        let player = Player::new(Some(42), false);

        assert_eq!(player.unit().world().object().type_id(), TypeId::Player);
        assert_eq!(
            player.unit().world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::UNIT | TypeMask::PLAYER
        );
        assert_eq!(player.session_id(), Some(42));
        assert_eq!(player.hit_chances(), (7.5, 7.5, 15.0));
        assert_eq!(player.ingame_time(), 0);
        assert_eq!(player.shared_quest_id(), 0);
        assert_eq!(player.extra_flags(), 0);
        assert_eq!(player.team(), TEAM_OTHER);
        assert!(player.is_active());
        assert!(player.controlled_by_player());
        assert!(player.accept_whispers());
        assert_eq!(
            player.data().visible_items,
            [VisibleItemValues::default(); EQUIPMENT_SLOT_END as usize]
        );
        assert!(!player.player_data_changes_mask().is_any_set());
        assert!(!player.active_player_data_changes_mask().is_any_set());
    }

    #[test]
    fn can_filter_whispers_permission_keeps_constructor_accept_flag_false() {
        let player = Player::new(None, true);
        assert!(!player.accept_whispers());
    }

    #[test]
    fn player_position_classifiers_match_cpp_static_helpers() {
        assert!(is_inventory_pos(INVENTORY_SLOT_BAG_0, NULL_SLOT));
        assert!(!is_inventory_pos(NULL_BAG, NULL_SLOT));
        assert!(is_inventory_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START
        ));
        assert!(is_inventory_pos(INVENTORY_SLOT_BAG_START, 0));
        assert!(is_inventory_pos(INVENTORY_SLOT_BAG_0, KEYRING_SLOT_START));
        assert!(is_inventory_pos(
            INVENTORY_SLOT_BAG_0,
            CHILD_EQUIPMENT_SLOT_START
        ));
        assert!(!is_inventory_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_BAG_START
        ));
        assert!(is_inventory_packed_pos(make_item_pos(
            INVENTORY_SLOT_BAG_START,
            5
        )));

        assert!(is_equipment_pos(INVENTORY_SLOT_BAG_0, 0));
        assert!(is_equipment_pos(
            INVENTORY_SLOT_BAG_0,
            PROFESSION_SLOT_START
        ));
        assert!(is_equipment_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_BAG_START
        ));
        assert!(is_equipment_pos(
            INVENTORY_SLOT_BAG_0,
            REAGENT_BAG_SLOT_START
        ));
        assert!(!is_equipment_pos(INVENTORY_SLOT_BAG_START, 0));
        assert!(is_equipment_packed_pos(make_item_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_BAG_START
        )));

        assert!(is_bank_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START));
        assert!(is_bank_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_BAG_START));
        assert!(is_bank_pos(BANK_SLOT_BAG_START, 0));
        assert!(!is_bank_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START
        ));
        assert!(is_bank_packed_pos(make_item_pos(BANK_SLOT_BAG_START, 2)));

        assert!(is_bag_pos(make_item_pos(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_BAG_START
        )));
        assert!(is_bag_pos(make_item_pos(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_BAG_START
        )));
        assert!(is_bag_pos(make_item_pos(
            INVENTORY_SLOT_BAG_0,
            REAGENT_BAG_SLOT_START
        )));
        assert!(!is_bag_pos(make_item_pos(INVENTORY_SLOT_BAG_START, 0)));

        assert!(is_child_equipment_pos(
            INVENTORY_SLOT_BAG_0,
            CHILD_EQUIPMENT_SLOT_START
        ));
        assert!(is_child_equipment_packed_pos(make_item_pos(
            INVENTORY_SLOT_BAG_0,
            CHILD_EQUIPMENT_SLOT_START
        )));
        assert!(!is_child_equipment_pos(
            INVENTORY_SLOT_BAG_START,
            CHILD_EQUIPMENT_SLOT_START
        ));
    }

    #[test]
    fn player_is_valid_pos_matches_cpp_top_level_and_bag_rules() {
        let bag_guid = ObjectGuid::create_item(1, 300);
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(16);

        assert!(player.is_valid_pos(NULL_BAG, NULL_SLOT, false));
        assert!(!player.is_valid_pos(NULL_BAG, NULL_SLOT, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, NULL_SLOT, false));
        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_0, NULL_SLOT, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, 0, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, PROFESSION_SLOT_START, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, REAGENT_BAG_SLOT_START, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 15, true));
        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 16, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_BAG_START, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_0, KEYRING_SLOT_START, true));
        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_0, CHILD_EQUIPMENT_SLOT_START, true));

        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_START, 0, true));
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_START, NULL_SLOT, false));
        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_START, NULL_SLOT, true));
        assert!(player.is_valid_pos(INVENTORY_SLOT_BAG_START, 3, true));
        assert!(!player.is_valid_pos(INVENTORY_SLOT_BAG_START, 4, true));
        assert!(player.is_valid_packed_pos(make_item_pos(INVENTORY_SLOT_BAG_START, 3), true));
    }

    #[test]
    fn find_equip_slot_maps_inventory_types_like_cpp() {
        let player = Player::new(None, false);
        let head = ItemStorageTemplate {
            inventory_type: InventoryType::Head,
            ..ItemStorageTemplate::regular_item(1, 1)
        };
        let robe = ItemStorageTemplate {
            inventory_type: InventoryType::Robe,
            ..ItemStorageTemplate::regular_item(2, 1)
        };
        let bag = ItemStorageTemplate {
            inventory_type: InventoryType::Bag,
            ..ItemStorageTemplate::regular_item(3, 1)
        };
        let weapon = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(4, 1)
        };
        let two_hand = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon2Hand,
            ..ItemStorageTemplate::regular_item(5, 1)
        };

        assert_eq!(
            player.find_equip_slot(find_equip_args(&head, NULL_SLOT, false, &[])),
            EQUIPMENT_SLOT_HEAD
        );
        assert_eq!(
            player.find_equip_slot(find_equip_args(&robe, NULL_SLOT, false, &[])),
            EQUIPMENT_SLOT_CHEST
        );
        assert_eq!(
            player.find_equip_slot(find_equip_args(&bag, NULL_SLOT, false, &[])),
            INVENTORY_SLOT_BAG_START
        );
        assert_eq!(
            player.find_equip_slot(find_equip_args(&weapon, EQUIPMENT_SLOT_OFFHAND, false, &[])),
            NULL_SLOT
        );

        let mut dual_args = find_equip_args(&weapon, EQUIPMENT_SLOT_OFFHAND, false, &[]);
        dual_args.can_dual_wield = true;
        assert_eq!(player.find_equip_slot(dual_args), EQUIPMENT_SLOT_OFFHAND);

        let mut titan_args = find_equip_args(&two_hand, EQUIPMENT_SLOT_OFFHAND, false, &[]);
        titan_args.can_dual_wield = true;
        assert_eq!(player.find_equip_slot(titan_args), NULL_SLOT);
        titan_args.can_titan_grip = true;
        assert_eq!(player.find_equip_slot(titan_args), EQUIPMENT_SLOT_OFFHAND);
    }

    #[test]
    fn find_equip_slot_requested_free_and_swap_paths_match_cpp() {
        let player = Player::new(None, false);
        let finger = ItemStorageTemplate {
            inventory_type: InventoryType::Finger,
            ..ItemStorageTemplate::regular_item(10, 1)
        };
        let mut ring1 = Item::default();
        ring1.set_debug_item_level(120);
        let mut ring2 = Item::default();
        ring2.set_debug_item_level(45);
        let equipped = [
            ItemSlotRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_FINGER1, &ring1),
            ItemSlotRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_FINGER2, &ring2),
        ];

        assert_eq!(
            player.find_equip_slot(find_equip_args(
                &finger,
                EQUIPMENT_SLOT_FINGER1,
                false,
                &equipped
            )),
            NULL_SLOT
        );
        assert_eq!(
            player.find_equip_slot(find_equip_args(
                &finger,
                EQUIPMENT_SLOT_FINGER1,
                true,
                &equipped
            )),
            EQUIPMENT_SLOT_FINGER1
        );
        assert_eq!(
            player.find_equip_slot(find_equip_args(&finger, NULL_SLOT, true, &equipped)),
            EQUIPMENT_SLOT_FINGER2
        );

        let equipped = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_FINGER1,
            &ring1,
        )];
        assert_eq!(
            player.find_equip_slot(find_equip_args(&finger, NULL_SLOT, false, &equipped)),
            EQUIPMENT_SLOT_FINGER2
        );
    }

    #[test]
    fn find_equip_slot_twohand_offhand_and_professions_match_cpp_edges() {
        let player = Player::new(None, false);
        let weapon = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(20, 1)
        };
        let mut mainhand = Item::default();
        mainhand.set_debug_item_level(100);
        let equipped = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_MAINHAND,
            &mainhand,
        )];
        let mut args = find_equip_args(&weapon, NULL_SLOT, false, &equipped);
        args.can_dual_wield = true;
        args.is_two_hand_used = true;
        assert_eq!(player.find_equip_slot(args), NULL_SLOT);

        let cooking_gear = ItemStorageTemplate {
            class_id: ItemClass::Profession,
            subclass_id: ItemSubclassProfession::Cooking as u32,
            inventory_type: InventoryType::ProfessionGear,
            ..ItemStorageTemplate::regular_item(21, 1)
        };
        let fishing_gear = ItemStorageTemplate {
            class_id: ItemClass::Profession,
            subclass_id: ItemSubclassProfession::Fishing as u32,
            inventory_type: InventoryType::ProfessionGear,
            ..ItemStorageTemplate::regular_item(22, 1)
        };
        let blacksmithing_gear = ItemStorageTemplate {
            class_id: ItemClass::Profession,
            subclass_id: ItemSubclassProfession::Blacksmithing as u32,
            inventory_type: InventoryType::ProfessionGear,
            ..ItemStorageTemplate::regular_item(23, 1)
        };

        let mut profession_args = find_equip_args(&cooking_gear, NULL_SLOT, false, &[]);
        profession_args.has_required_profession_skill = true;
        assert_eq!(
            player.find_equip_slot(profession_args),
            PROFESSION_SLOT_COOKING_GEAR1
        );

        profession_args.proto = &fishing_gear;
        assert_eq!(player.find_equip_slot(profession_args), NULL_SLOT);

        profession_args.proto = &blacksmithing_gear;
        profession_args.profession_slot = Some(0);
        assert_eq!(
            player.find_equip_slot(profession_args),
            PROFESSION_SLOT_PROFESSION1_GEAR2
        );
    }

    #[test]
    fn can_equip_item_preflight_and_runtime_guards_match_cpp_order() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate {
            inventory_type: InventoryType::Head,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let mut source = Item::default();
        source.set_count(1);

        assert_eq!(
            player
                .can_equip_item(can_equip_args(NULL_SLOT, Some(&proto), None))
                .result,
            InventoryResult::ItemNotFound
        );

        let mut swap_missing = can_equip_args(NULL_SLOT, None, Some(&source));
        swap_missing.swap = true;
        assert_eq!(
            player.can_equip_item(swap_missing).result,
            InventoryResult::CantSwap
        );

        source.set_loot_generated(true);
        assert_eq!(
            player
                .can_equip_item(can_equip_args(NULL_SLOT, Some(&proto), Some(&source)))
                .result,
            InventoryResult::LootGone
        );
        source.set_loot_generated(false);

        source.set_item_flag(ItemFieldFlags::SOULBOUND);
        source.set_owner_guid(ObjectGuid::create_player(1, 99));
        assert_eq!(
            player
                .can_equip_item(can_equip_args(NULL_SLOT, Some(&proto), Some(&source)))
                .result,
            InventoryResult::NotOwner
        );
        source.remove_item_flag(ItemFieldFlags::SOULBOUND);

        let limited = ItemStorageTemplate {
            max_count: 1,
            ..proto
        };
        source.object_mut().create(ObjectGuid::create_item(1, 900));
        source.object_mut().set_entry(limited.entry);
        let mut stored = Item::default();
        stored.object_mut().create(ObjectGuid::create_item(1, 901));
        stored.object_mut().set_entry(limited.entry);
        stored.set_count(1);
        let stored_items = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            &stored,
            Some(&limited),
        )];
        let mut limit_args = can_equip_args(NULL_SLOT, Some(&limited), Some(&source));
        limit_args.stored_items = &stored_items;
        assert_eq!(
            player.can_equip_item(limit_args).result,
            InventoryResult::ItemMaxCount
        );

        let mut stunned = can_equip_args(NULL_SLOT, Some(&proto), Some(&source));
        stunned.is_stunned = true;
        stunned.is_charmed = true;
        assert_eq!(
            player.can_equip_item(stunned).result,
            InventoryResult::GenericStunned
        );

        let mut combat = can_equip_args(NULL_SLOT, Some(&proto), Some(&source));
        combat.is_in_combat = true;
        assert_eq!(
            player.can_equip_item(combat).result,
            InventoryResult::NotInCombat
        );

        let weapon = ItemStorageTemplate {
            class_id: ItemClass::Weapon,
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(101, 1)
        };
        let mut cooldown = can_equip_args(NULL_SLOT, Some(&weapon), Some(&source));
        cooldown.is_in_combat = true;
        cooldown.weapon_change_timer_active = true;
        assert_eq!(
            player.can_equip_item(cooldown).result,
            InventoryResult::ItemCooldown
        );

        let mut casting = can_equip_args(NULL_SLOT, Some(&weapon), Some(&source));
        casting.current_generic_spell_allows_equip = Some(false);
        assert_eq!(
            player.can_equip_item(casting).result,
            InventoryResult::ClientLockedOut
        );
    }

    #[test]
    fn can_equip_item_destination_use_and_unique_paths_match_cpp() {
        let player = Player::new(None, false);
        let head = ItemStorageTemplate {
            inventory_type: InventoryType::Head,
            ..ItemStorageTemplate::regular_item(200, 1)
        };
        let finger = ItemStorageTemplate {
            inventory_type: InventoryType::Finger,
            ..ItemStorageTemplate::regular_item(201, 1)
        };
        let mut source = Item::default();
        source.set_count(1);
        let mut equipped_head = Item::default();
        equipped_head.set_count(1);
        let equipped = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_HEAD,
            &equipped_head,
        )];

        let outcome = player.can_equip_item(can_equip_args(NULL_SLOT, Some(&head), Some(&source)));
        assert_eq!(outcome.result, InventoryResult::Ok);
        assert_eq!(
            outcome.dest,
            make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_HEAD)
        );
        assert_eq!(outcome.unique_ignore_slot, Some(NULL_SLOT));

        let mut occupied = can_equip_args(NULL_SLOT, Some(&head), Some(&source));
        occupied.equipped_items = &equipped;
        assert_eq!(
            player.can_equip_item(occupied).result,
            InventoryResult::NotEquippable
        );

        let mut can_use = can_equip_args(NULL_SLOT, Some(&head), Some(&source));
        can_use.can_use_result = InventoryResult::CantEquipSkill;
        assert_eq!(
            player.can_equip_item(can_use).result,
            InventoryResult::CantEquipSkill
        );

        let mut source_ring = Item::default();
        source_ring.set_count(1);
        let other_ring = Item::default();
        let rings = [
            ItemSlotRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_FINGER1, &other_ring),
            ItemSlotRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_FINGER2, &source_ring),
        ];
        let mut unique = can_equip_args(EQUIPMENT_SLOT_FINGER1, Some(&finger), Some(&source_ring));
        unique.swap = true;
        unique.equipped_items = &rings;
        unique.can_equip_unique_result = InventoryResult::ItemUniqueEquippable;
        let outcome = player.can_equip_item(unique);
        assert_eq!(outcome.result, InventoryResult::ItemUniqueEquippable);
        assert_eq!(outcome.unique_ignore_slot, Some(EQUIPMENT_SLOT_FINGER2));
    }

    #[test]
    fn can_equip_item_quiver_offhand_and_twohand_edges_match_cpp() {
        let player = Player::new(None, false);
        let mut source = Item::default();
        source.set_count(1);
        let bag_quiver = ItemStorageTemplate {
            class_id: ItemClass::Quiver,
            subclass_id: ItemSubClassQuiver::AmmoPouch as u32,
            inventory_type: InventoryType::Bag,
            ..ItemStorageTemplate::regular_item(300, 1)
        };
        let existing_quiver = Item::default();
        let stored_items = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_BAG_START,
            &existing_quiver,
            Some(&bag_quiver),
        )];
        let mut quiver_args = can_equip_args(NULL_SLOT, Some(&bag_quiver), Some(&source));
        quiver_args.stored_items = &stored_items;
        assert_eq!(
            player.can_equip_item(quiver_args).result,
            InventoryResult::OnlyOneAmmo
        );

        let polearm = ItemStorageTemplate {
            class_id: ItemClass::Weapon,
            subclass_id: ItemSubClassWeapon::Polearm as u32,
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(301, 1)
        };
        let mut polearm_args =
            can_equip_args(EQUIPMENT_SLOT_OFFHAND, Some(&polearm), Some(&source));
        polearm_args.can_dual_wield = true;
        assert_eq!(
            player.can_equip_item(polearm_args).result,
            InventoryResult::TwoHandSkillNotFound
        );

        let offhand_weapon = ItemStorageTemplate {
            inventory_type: InventoryType::WeaponOffhand,
            ..ItemStorageTemplate::regular_item(302, 1)
        };
        assert_eq!(
            player
                .can_equip_item(can_equip_args(
                    EQUIPMENT_SLOT_OFFHAND,
                    Some(&offhand_weapon),
                    Some(&source)
                ))
                .result,
            InventoryResult::TwoHandSkillNotFound
        );

        let mut twohand_used =
            can_equip_args(EQUIPMENT_SLOT_OFFHAND, Some(&offhand_weapon), Some(&source));
        twohand_used.proto_always_allow_dual_wield = true;
        twohand_used.is_two_hand_used = true;
        assert_eq!(
            player.can_equip_item(twohand_used).result,
            InventoryResult::Equipped2handed
        );

        let twohand = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon2Hand,
            ..ItemStorageTemplate::regular_item(303, 1)
        };
        let offhand_item = Item::default();
        let equipped_offhand = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_OFFHAND,
            &offhand_item,
        )];
        let mut twohand_args = can_equip_args(NULL_SLOT, Some(&twohand), Some(&source));
        twohand_args.equipped_items = &equipped_offhand;
        twohand_args.offhand_can_store_result = InventoryResult::InvFull;
        assert_eq!(
            player.can_equip_item(twohand_args).result,
            InventoryResult::InvFull
        );

        twohand_args.swap = true;
        assert_eq!(
            player.can_equip_item(twohand_args).result,
            InventoryResult::CantSwap
        );
    }

    #[test]
    fn can_unequip_item_matches_cpp_position_template_and_runtime_guards() {
        let player = Player::new(None, false);
        let armor = ItemStorageTemplate {
            inventory_type: InventoryType::Chest,
            ..ItemStorageTemplate::regular_item(400, 1)
        };
        let weapon = ItemStorageTemplate {
            class_id: ItemClass::Weapon,
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(401, 1)
        };
        let bag = ItemStorageTemplate {
            inventory_type: InventoryType::Bag,
            ..ItemStorageTemplate::regular_item(402, 1)
        };
        let mut source = Item::default();
        source.set_count(1);

        assert_eq!(
            player.can_unequip_item(can_unequip_args(
                make_item_pos(INVENTORY_SLOT_BAG_START, 0),
                Some(&armor),
                Some(&source),
            )),
            InventoryResult::Ok
        );
        assert_eq!(
            player.can_unequip_item(can_unequip_args(
                make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
                Some(&armor),
                None,
            )),
            InventoryResult::Ok
        );
        assert_eq!(
            player.can_unequip_item(can_unequip_args(
                make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
                None,
                Some(&source),
            )),
            InventoryResult::ItemNotFound
        );

        source.set_loot_generated(true);
        assert_eq!(
            player.can_unequip_item(can_unequip_args(
                make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
                Some(&armor),
                Some(&source),
            )),
            InventoryResult::LootGone
        );
        source.set_loot_generated(false);

        let mut charmed = can_unequip_args(
            make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
            Some(&armor),
            Some(&source),
        );
        charmed.is_charmed = true;
        assert_eq!(
            player.can_unequip_item(charmed),
            InventoryResult::ClientLockedOut
        );

        let mut combat = can_unequip_args(
            make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
            Some(&armor),
            Some(&source),
        );
        combat.is_in_combat = true;
        assert_eq!(
            player.can_unequip_item(combat),
            InventoryResult::NotInCombat
        );

        let mut arena = can_unequip_args(
            make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST),
            Some(&armor),
            Some(&source),
        );
        arena.is_in_progress_arena = true;
        assert_eq!(
            player.can_unequip_item(arena),
            InventoryResult::NotDuringArenaMatch
        );

        let mut weapon_combat = can_unequip_args(
            make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND),
            Some(&weapon),
            Some(&source),
        );
        weapon_combat.is_in_combat = true;
        assert_eq!(player.can_unequip_item(weapon_combat), InventoryResult::Ok);

        let mut non_empty_bag = can_unequip_args(
            make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START),
            Some(&bag),
            Some(&source),
        );
        non_empty_bag.source_is_not_empty_bag = true;
        assert_eq!(
            player.can_unequip_item(non_empty_bag),
            InventoryResult::DestroyNonemptyBag
        );

        non_empty_bag.swap = true;
        assert_eq!(player.can_unequip_item(non_empty_bag), InventoryResult::Ok);
    }

    #[test]
    fn can_use_item_template_matches_cpp_access_requirement_order() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(500, 1);

        assert_eq!(
            player.can_use_item_template(can_use_template_args(None)),
            InventoryResult::ItemNotFound
        );

        let mut args = can_use_template_args(Some(&proto));
        args.internal_item = true;
        args.faction_horde = true;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipEver
        );

        args.internal_item = false;
        args.team = TEAM_ALLIANCE_ID;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipEver
        );

        args.faction_horde = false;
        args.faction_alliance = true;
        args.team = TEAM_HORDE_ID;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipEver
        );

        args.faction_alliance = false;
        args.allowable_class_matches = false;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipEver
        );

        args.allowable_class_matches = true;
        args.allowable_race_matches = false;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipEver
        );

        args.allowable_race_matches = true;
        args.required_skill = 164;
        args.required_skill_rank = 75;
        args.required_skill_value = 0;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::ProficiencyNeeded
        );

        args.required_skill_value = 50;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipSkill
        );

        args.required_skill_value = 75;
        args.required_spell = 1000;
        args.has_required_spell = false;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::ProficiencyNeeded
        );
    }

    #[test]
    fn can_use_item_template_matches_cpp_late_requirement_order() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(501, 1);
        let mut args = can_use_template_args(Some(&proto));

        args.player_level = 20;
        args.base_required_level = 30;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipLevelI
        );

        args.skip_required_level_check = true;
        assert_eq!(player.can_use_item_template(args), InventoryResult::Ok);

        args.skip_required_level_check = false;
        args.player_level = 70;
        args.holiday_id = 1;
        args.holiday_active = false;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::ClientLockedOut
        );

        args.holiday_active = true;
        args.required_reputation_faction = 72;
        args.required_reputation_rank = 5;
        args.player_reputation_rank = 4;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantEquipReputation
        );

        args.player_reputation_rank = 5;
        args.effect0_spell_id = Some(483);
        args.effect1_spell_id = Some(9000);
        args.has_effect1_spell = true;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::InternalBagError
        );

        args.has_effect1_spell = false;
        args.artifact_specialization = Some(2);
        args.primary_specialization = 1;
        assert_eq!(
            player.can_use_item_template(args),
            InventoryResult::CantUseItem
        );

        args.primary_specialization = 2;
        assert_eq!(player.can_use_item_template(args), InventoryResult::Ok);
    }

    #[test]
    fn can_use_item_object_matches_cpp_item_level_and_template_order() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(600, 1);
        let mut source = Item::default();
        source.set_count(1);

        assert_eq!(
            player.can_use_item(can_use_args(Some(&proto), None)),
            InventoryResult::ItemNotFound
        );

        let mut dead = can_use_args(Some(&proto), Some(&source));
        dead.is_alive = false;
        assert_eq!(player.can_use_item(dead), InventoryResult::PlayerDead);

        dead.not_loading = false;
        assert_eq!(player.can_use_item(dead), InventoryResult::Ok);

        assert_eq!(
            player.can_use_item(can_use_args(None, Some(&source))),
            InventoryResult::ItemNotFound
        );

        source.set_item_flag(ItemFieldFlags::SOULBOUND);
        source.set_owner_guid(ObjectGuid::create_player(1, 99));
        assert_eq!(
            player.can_use_item(can_use_args(Some(&proto), Some(&source))),
            InventoryResult::NotOwner
        );
        source.remove_item_flag(ItemFieldFlags::SOULBOUND);

        let mut level = can_use_args(Some(&proto), Some(&source));
        level.player_level = 20;
        level.item_required_level = 30;
        level.template_args.internal_item = true;
        assert_eq!(player.can_use_item(level), InventoryResult::CantEquipLevelI);

        let mut template = can_use_args(Some(&proto), Some(&source));
        template.template_args.internal_item = true;
        assert_eq!(
            player.can_use_item(template),
            InventoryResult::CantEquipEver
        );
    }

    #[test]
    fn can_use_item_object_matches_cpp_skill_and_heirloom_morph() {
        let player = Player::new(None, false);
        let armor = ItemStorageTemplate {
            class_id: ItemClass::Armor,
            inventory_type: InventoryType::Chest,
            ..ItemStorageTemplate::regular_item(601, 1)
        };
        let weapon = ItemStorageTemplate {
            class_id: ItemClass::Weapon,
            inventory_type: InventoryType::Weapon,
            ..ItemStorageTemplate::regular_item(602, 1)
        };
        let source = Item::default();

        let mut no_skill = can_use_args(Some(&weapon), Some(&source));
        no_skill.item_skill = SKILL_MAIL;
        no_skill.item_skill_value = 0;
        assert_eq!(
            player.can_use_item(no_skill),
            InventoryResult::ProficiencyNeeded
        );

        no_skill.item_skill_value = 1;
        assert_eq!(player.can_use_item(no_skill), InventoryResult::Ok);

        let mut hunter_mail = can_use_args(Some(&armor), Some(&source));
        hunter_mail.item_skill = SKILL_MAIL;
        hunter_mail.item_skill_value = 0;
        hunter_mail.has_item_skill = false;
        hunter_mail.proto_is_heirloom = true;
        hunter_mail.player_class = CLASS_HUNTER;
        assert_eq!(player.can_use_item(hunter_mail), InventoryResult::Ok);

        let mut warrior_mail = hunter_mail;
        warrior_mail.player_class = CLASS_WARRIOR;
        assert_eq!(
            player.can_use_item(warrior_mail),
            InventoryResult::ProficiencyNeeded
        );

        let mut paladin_plate = can_use_args(Some(&armor), Some(&source));
        paladin_plate.item_skill = SKILL_PLATE_MAIL;
        paladin_plate.item_skill_value = 0;
        paladin_plate.has_item_skill = false;
        paladin_plate.proto_is_heirloom = true;
        paladin_plate.player_class = CLASS_PALADIN;
        assert_eq!(player.can_use_item(paladin_plate), InventoryResult::Ok);
    }

    #[test]
    fn can_equip_unique_item_template_matches_cpp_unique_entry_guards() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(700, 1);
        assert_eq!(
            player.can_equip_unique_item_template(can_equip_unique_template_args(None)),
            InventoryResult::ItemNotFound
        );

        let mut equipped = Item::default();
        equipped.object_mut().set_entry(700);
        equipped.set_count(1);
        let equipped_items = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_FINGER1,
            &equipped,
            Some(&proto),
        )];

        let mut args = can_equip_unique_template_args(Some(&proto));
        args.unique_equippable = true;
        args.equipped_items = &equipped_items;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::ItemUniqueEquippable
        );

        args.except_slot = EQUIPMENT_SLOT_FINGER1;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::Ok
        );

        let equipped_gems = [EquippedGemRef::new(EQUIPMENT_SLOT_CHEST, 700, 0)];
        args.equipped_items = &[];
        args.equipped_gems = &equipped_gems;
        args.except_slot = NULL_SLOT;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::ItemUniqueEquippable
        );
    }

    #[test]
    fn can_equip_unique_item_template_matches_cpp_limit_category_guards() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate {
            item_limit_category: 10,
            ..ItemStorageTemplate::regular_item(701, 1)
        };
        let limit = ItemLimitCategoryTemplate {
            id: 10,
            quantity: 2,
            flags: ITEM_LIMIT_CATEGORY_MODE_EQUIP,
        };
        let mut equipped = Item::default();
        equipped.object_mut().set_entry(702);
        equipped.set_count(1);
        let equipped_items = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            EQUIPMENT_SLOT_TRINKET1,
            &equipped,
            Some(&proto),
        )];
        let equipped_gems = [EquippedGemRef::new(EQUIPMENT_SLOT_CHEST, 703, 10)];

        let mut args = can_equip_unique_template_args(Some(&proto));
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::NotEquippable
        );

        args.limit_category = Some(&limit);
        args.limit_count = 3;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::ItemMaxLimitCategoryEquippedExceededIs
        );

        args.limit_count = 2;
        args.equipped_items = &equipped_items;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::ItemMaxLimitCategoryEquippedExceededIs
        );

        args.equipped_items = &[];
        args.equipped_gems = &equipped_gems;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::ItemMaxCountEquippedSocketed
        );

        args.except_slot = EQUIPMENT_SLOT_CHEST;
        assert_eq!(
            player.can_equip_unique_item_template(args),
            InventoryResult::Ok
        );
    }

    #[test]
    fn can_equip_unique_item_object_matches_cpp_template_then_gem_order() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(704, 1);
        let source = Item::default();
        let gem_proto = ItemStorageTemplate::regular_item(705, 1);
        let socketed_gems = [
            SocketedGemUniqueRef::new(None, true, None, 1),
            SocketedGemUniqueRef::new(Some(&gem_proto), true, None, 1),
        ];
        let equipped_gems = [EquippedGemRef::new(EQUIPMENT_SLOT_CHEST, 705, 0)];
        let base_equipped_gems = [EquippedGemRef::new(EQUIPMENT_SLOT_CHEST, 704, 0)];

        assert_eq!(
            player.can_equip_unique_item(can_equip_unique_args(None, Some(&proto))),
            InventoryResult::ItemNotFound
        );

        let mut template_first = can_equip_unique_args(Some(&source), Some(&proto));
        template_first.unique_equippable = true;
        template_first.equipped_gems = &base_equipped_gems;
        template_first.socketed_gems = &socketed_gems;
        assert_eq!(
            player.can_equip_unique_item(template_first),
            InventoryResult::ItemUniqueEquippable
        );

        let mut gem_args = can_equip_unique_args(Some(&source), Some(&proto));
        gem_args.socketed_gems = &socketed_gems;
        gem_args.equipped_gems = &equipped_gems;
        assert_eq!(
            player.can_equip_unique_item(gem_args),
            InventoryResult::ItemUniqueEquippable
        );
    }

    #[test]
    fn can_equip_unique_item_object_matches_cpp_socketed_gem_limit_count() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(706, 1);
        let gem_proto = ItemStorageTemplate {
            item_limit_category: 20,
            ..ItemStorageTemplate::regular_item(707, 1)
        };
        let limit = ItemLimitCategoryTemplate {
            id: 20,
            quantity: 2,
            flags: ITEM_LIMIT_CATEGORY_MODE_EQUIP,
        };
        let socketed_gems = [SocketedGemUniqueRef::new(
            Some(&gem_proto),
            false,
            Some(&limit),
            2,
        )];
        let equipped_gems = [EquippedGemRef::new(EQUIPMENT_SLOT_CHEST, 708, 20)];

        let mut source = Item::default();
        source.set_slot(INVENTORY_SLOT_ITEM_START);
        let mut unequipped = can_equip_unique_args(Some(&source), Some(&proto));
        unequipped.socketed_gems = &socketed_gems;
        unequipped.equipped_gems = &equipped_gems;
        assert_eq!(
            player.can_equip_unique_item(unequipped),
            InventoryResult::ItemMaxCountEquippedSocketed
        );

        let mut equipped_source = Item::default();
        equipped_source.set_slot(EQUIPMENT_SLOT_FINGER1);
        let mut equipped = can_equip_unique_args(Some(&equipped_source), Some(&proto));
        equipped.socketed_gems = &socketed_gems;
        equipped.equipped_gems = &equipped_gems;
        assert_eq!(player.can_equip_unique_item(equipped), InventoryResult::Ok);
    }

    #[test]
    fn item_pos_count_containment_matches_cpp_pos_only_check() {
        let target = ItemPosCount::new(make_item_pos(INVENTORY_SLOT_BAG_0, 10), 1);
        let positions = [ItemPosCount::new(
            make_item_pos(INVENTORY_SLOT_BAG_0, 10),
            99,
        )];

        assert!(target.is_contained_in(&positions));
        assert!(
            !ItemPosCount::new(make_item_pos(INVENTORY_SLOT_BAG_0, 11), 1)
                .is_contained_in(&positions)
        );
    }

    #[test]
    fn can_store_item_in_specific_slot_allocates_empty_top_level_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut dest = Vec::new();
        let mut count = 7;

        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut dest,
                &proto,
                &mut count,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                7,
            )]
        );
        assert_eq!(count, 0);

        let mut duplicate_count = 3;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut dest,
                &proto,
                &mut duplicate_count,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::Ok
        );
        assert_eq!(dest.len(), 1);
        assert_eq!(duplicate_count, 3);
    }

    #[test]
    fn can_store_item_in_specific_slot_merges_existing_stack_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut existing = Item::default();
        existing
            .object_mut()
            .create(ObjectGuid::create_item(1, 100));
        existing.object_mut().set_entry(6948);
        existing.set_count(12);
        let mut dest = Vec::new();
        let mut count = 10;

        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut dest,
                &proto,
                &mut count,
                false,
                Some(&existing),
                None,
                false,
                None,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                8,
            )]
        );
        assert_eq!(count, 2);

        existing.object_mut().set_entry(6949);
        let mut swap_count = 1;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &mut Vec::new(),
                &proto,
                &mut swap_count,
                true,
                Some(&existing),
                None,
                false,
                None,
            ),
            InventoryResult::Ok
        );

        let mut blocked_count = 2;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &mut Vec::new(),
                &proto,
                &mut blocked_count,
                false,
                Some(&existing),
                None,
                false,
                None,
            ),
            InventoryResult::CantStack
        );
        assert_eq!(blocked_count, 2);
    }

    #[test]
    fn can_store_item_in_specific_slot_applies_source_move_guards_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 101));
        source.object_mut().set_entry(6948);
        source.set_count(1);

        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                Some(&source),
                true,
                None,
            ),
            InventoryResult::DestroyNonemptyBag
        );

        let mut bag_slot_count = 1;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut bag_slot_count,
                false,
                None,
                Some(&source),
                true,
                None,
            ),
            InventoryResult::Ok
        );

        let mut same_source_count = 1;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut Vec::new(),
                &proto,
                &mut same_source_count,
                false,
                Some(&source),
                Some(&source),
                false,
                None,
            ),
            InventoryResult::Ok
        );

        source.set_item_flag(ItemFieldFlags::CHILD);
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                Some(&source),
                false,
                None,
            ),
            InventoryResult::WrongBagType3
        );
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                CHILD_EQUIPMENT_SLOT_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                Some(&source),
                false,
                None,
            ),
            InventoryResult::Ok
        );
    }

    #[test]
    fn can_store_item_in_specific_slot_applies_empty_slot_fit_guards_like_cpp() {
        let mut player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let regular_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::Container as u32,
            container_slots: 2,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let herb_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::HerbContainer as u32,
            container_slots: 2,
            ..ItemStorageTemplate::regular_item(101, 1)
        };
        let herb = ItemStorageTemplate {
            bag_family: BagFamilyMask::HERBS,
            ..ItemStorageTemplate::regular_item(2447, 20)
        };

        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                REAGENT_BAG_SLOT_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::WrongBagType
        );
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                BUYBACK_SLOT_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::WrongBagType
        );
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_START,
                0,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::WrongBagType
        );

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, ObjectGuid::create_item(1, 300), 2)
            .unwrap();
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_START,
                2,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                None,
                false,
                Some(&regular_bag_proto),
            ),
            InventoryResult::WrongBagType
        );
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_START,
                0,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                None,
                None,
                false,
                Some(&herb_bag_proto),
            ),
            InventoryResult::WrongBagType
        );

        let mut dest = Vec::new();
        let mut count = 3;
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_START,
                0,
                &mut dest,
                &herb,
                &mut count,
                false,
                None,
                None,
                false,
                Some(&herb_bag_proto),
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_START, 0),
                3
            )]
        );
    }

    #[test]
    fn can_store_item_in_specific_slot_preserves_cpp_keyring_gate_condition() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut count = 1;

        assert!(!cpp_keyring_family_gate_applies(KEYRING_SLOT_START));
        assert_eq!(
            player.can_store_item_in_specific_slot(
                INVENTORY_SLOT_BAG_0,
                KEYRING_SLOT_START,
                &mut Vec::new(),
                &proto,
                &mut count,
                false,
                None,
                None,
                false,
                None,
            ),
            InventoryResult::Ok
        );
    }

    #[test]
    fn can_store_item_in_inventory_slots_merges_matching_stacks_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut matching = Item::default();
        matching
            .object_mut()
            .create(ObjectGuid::create_item(1, 200));
        matching.object_mut().set_entry(6948);
        matching.set_count(16);
        let mut wrong_entry = Item::default();
        wrong_entry
            .object_mut()
            .create(ObjectGuid::create_item(1, 201));
        wrong_entry.object_mut().set_entry(6949);
        wrong_entry.set_count(1);
        let slot_items = [
            ItemSlotRef::new(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START, &matching),
            ItemSlotRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &wrong_entry,
            ),
        ];
        let mut dest = Vec::new();
        let mut count = 6;

        assert_eq!(
            player.can_store_item_in_inventory_slots(
                INVENTORY_SLOT_ITEM_START,
                INVENTORY_SLOT_ITEM_START + 3,
                &mut dest,
                &proto,
                &mut count,
                true,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                &slot_items,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                4,
            )]
        );
        assert_eq!(count, 2);
    }

    #[test]
    fn can_store_item_in_inventory_slots_allocates_empty_slots_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut occupied = Item::default();
        occupied
            .object_mut()
            .create(ObjectGuid::create_item(1, 202));
        occupied.object_mut().set_entry(6948);
        occupied.set_count(1);
        let slot_items = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            &occupied,
        )];
        let mut dest = vec![ItemPosCount::new(
            make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1),
            1,
        )];
        let mut count = 7;

        assert_eq!(
            player.can_store_item_in_inventory_slots(
                INVENTORY_SLOT_ITEM_START,
                INVENTORY_SLOT_ITEM_START + 3,
                &mut dest,
                &proto,
                &mut count,
                false,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                &slot_items,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![
                ItemPosCount::new(
                    make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1),
                    1,
                ),
                ItemPosCount::new(
                    make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 2),
                    7,
                ),
            ]
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn can_store_item_in_inventory_slots_applies_cpp_source_and_skip_rules() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 203));
        source.object_mut().set_entry(6948);
        source.set_count(1);

        assert_eq!(
            player.can_store_item_in_inventory_slots(
                INVENTORY_SLOT_ITEM_START,
                INVENTORY_SLOT_ITEM_START + 1,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                Some(&source),
                true,
                NULL_BAG,
                NULL_SLOT,
                &[],
            ),
            InventoryResult::DestroyNonemptyBag
        );

        let slot_items = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            &source,
        )];
        let mut dest = Vec::new();
        let mut count = 1;
        assert_eq!(
            player.can_store_item_in_inventory_slots(
                INVENTORY_SLOT_ITEM_START,
                INVENTORY_SLOT_ITEM_START + 2,
                &mut dest,
                &proto,
                &mut count,
                false,
                Some(&source),
                false,
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &slot_items,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                1,
            )]
        );
        assert_eq!(count, 0);
    }

    #[test]
    fn can_store_item_in_bag_applies_cpp_bag_and_source_guards() {
        let mut player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let regular_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::Container as u32,
            container_slots: 4,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let bag_guid = ObjectGuid::create_item(1, 300);

        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                true,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType
        );

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                true,
                None,
                false,
                INVENTORY_SLOT_BAG_START,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType
        );

        let mut source_bag = Item::default();
        source_bag.object_mut().create(bag_guid);
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                true,
                Some(&source_bag),
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType
        );

        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 301));
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                true,
                Some(&source),
                true,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::DestroyNonemptyBag
        );

        source.set_item_flag(ItemFieldFlags::CHILD);
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &proto,
                &mut 1,
                false,
                true,
                Some(&source),
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType3
        );
    }

    #[test]
    fn can_store_item_in_bag_applies_cpp_specialized_mode_and_family_rules() {
        let mut player = Player::new(None, false);
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, ObjectGuid::create_item(1, 310), 2)
            .unwrap();
        let misc = ItemStorageTemplate::regular_item(6948, 20);
        let herb = ItemStorageTemplate {
            bag_family: BagFamilyMask::HERBS,
            ..ItemStorageTemplate::regular_item(2447, 20)
        };
        let regular_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::Container as u32,
            container_slots: 2,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let herb_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::HerbContainer as u32,
            container_slots: 2,
            ..ItemStorageTemplate::regular_item(101, 1)
        };

        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &misc,
                &mut 1,
                false,
                false,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType
        );
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut Vec::new(),
                &misc,
                &mut 1,
                false,
                false,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&herb_bag_proto),
                &[],
            ),
            InventoryResult::WrongBagType
        );

        let mut dest = Vec::new();
        let mut count = 1;
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut dest,
                &herb,
                &mut count,
                false,
                false,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&herb_bag_proto),
                &[],
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_START, 0),
                1,
            )]
        );
    }

    #[test]
    fn can_store_item_in_bag_scans_slots_like_cpp_merge_and_empty_modes() {
        let mut player = Player::new(None, false);
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, ObjectGuid::create_item(1, 320), 3)
            .unwrap();
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let regular_bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::Container as u32,
            container_slots: 3,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let mut matching = Item::default();
        matching
            .object_mut()
            .create(ObjectGuid::create_item(1, 321));
        matching.object_mut().set_entry(6948);
        matching.set_count(16);
        let mut wrong_entry = Item::default();
        wrong_entry
            .object_mut()
            .create(ObjectGuid::create_item(1, 322));
        wrong_entry.object_mut().set_entry(6949);
        wrong_entry.set_count(1);
        let slot_items = [
            ItemSlotRef::new(INVENTORY_SLOT_BAG_START, 0, &matching),
            ItemSlotRef::new(INVENTORY_SLOT_BAG_START, 1, &wrong_entry),
        ];
        let mut merge_dest = Vec::new();
        let mut merge_count = 6;

        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut merge_dest,
                &proto,
                &mut merge_count,
                true,
                true,
                None,
                false,
                NULL_BAG,
                NULL_SLOT,
                Some(&regular_bag_proto),
                &slot_items,
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            merge_dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_START, 0),
                4,
            )]
        );
        assert_eq!(merge_count, 2);

        let mut empty_dest = Vec::new();
        let mut empty_count = 7;
        assert_eq!(
            player.can_store_item_in_bag(
                INVENTORY_SLOT_BAG_START,
                &mut empty_dest,
                &proto,
                &mut empty_count,
                false,
                true,
                None,
                false,
                NULL_BAG,
                2,
                Some(&regular_bag_proto),
                &slot_items,
            ),
            InventoryResult::Ok
        );
        assert!(empty_dest.is_empty());
        assert_eq!(empty_count, 7);
    }

    #[test]
    fn can_take_more_similar_items_matches_cpp_max_count_guards() {
        let player = Player::new(None, false);
        let unlimited = ItemStorageTemplate::regular_item(6948, 20);

        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: None,
                count: 3,
                source_item: None,
                current_item_count: 0,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::ItemMaxCount,
                no_space_count: Some(3),
                offending_item_id: None,
            }
        );
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&unlimited),
                count: 3,
                source_item: None,
                current_item_count: 999,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            can_take_more_similar_ok()
        );

        let mut source = Item::default();
        source.set_loot_generated(true);
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&unlimited),
                count: 3,
                source_item: Some(&source),
                current_item_count: 0,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::LootGone,
                no_space_count: None,
                offending_item_id: None,
            }
        );

        let limited = ItemStorageTemplate {
            max_count: 10,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&limited),
                count: 4,
                source_item: None,
                current_item_count: 8,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::ItemMaxCount,
                no_space_count: Some(2),
                offending_item_id: None,
            }
        );

        let max_int = ItemStorageTemplate {
            max_count: i32::MAX,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&max_int),
                count: 4,
                source_item: None,
                current_item_count: u32::MAX - 4,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            can_take_more_similar_ok()
        );
    }

    #[test]
    fn can_take_more_similar_items_matches_cpp_limit_category_guards() {
        let player = Player::new(None, false);
        let limited_category = ItemStorageTemplate {
            item_limit_category: 77,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };

        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&limited_category),
                count: 3,
                source_item: None,
                current_item_count: 0,
                limit_category: None,
                current_limit_category_count: 0,
            }),
            CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::NotEquippable,
                no_space_count: Some(3),
                offending_item_id: None,
            }
        );

        let have_limit = ItemLimitCategoryTemplate {
            id: 77,
            quantity: 5,
            flags: ITEM_LIMIT_CATEGORY_MODE_HAVE,
        };
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&limited_category),
                count: 3,
                source_item: None,
                current_item_count: 0,
                limit_category: Some(&have_limit),
                current_limit_category_count: 4,
            }),
            CanTakeMoreSimilarItemsOutcome {
                result: InventoryResult::ItemMaxLimitCategoryCountExceededIs,
                no_space_count: Some(2),
                offending_item_id: Some(6948),
            }
        );

        let equip_limit = ItemLimitCategoryTemplate {
            id: 77,
            quantity: 1,
            flags: ITEM_LIMIT_CATEGORY_MODE_EQUIP,
        };
        assert_eq!(
            player.can_take_more_similar_items(CanTakeMoreSimilarItemsArgs {
                proto: Some(&limited_category),
                count: 99,
                source_item: None,
                current_item_count: 0,
                limit_category: Some(&equip_limit),
                current_limit_category_count: 99,
            }),
            can_take_more_similar_ok()
        );
    }

    #[test]
    fn item_count_by_entry_matches_cpp_locations_and_skip_item() {
        let player = Player::new(None, false);
        let mut inventory_item = Item::default();
        inventory_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 610));
        inventory_item.object_mut().set_entry(6948);
        inventory_item.set_count(2);
        let mut bank_item = Item::default();
        bank_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 611));
        bank_item.object_mut().set_entry(6948);
        bank_item.set_count(3);
        let mut other_item = Item::default();
        other_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 612));
        other_item.object_mut().set_entry(6949);
        other_item.set_count(7);
        let stored = [
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &inventory_item,
                None,
            ),
            ItemStorageRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, &bank_item, None),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &other_item,
                None,
            ),
        ];

        assert_eq!(player.item_count_by_entry(6948, false, None, &stored), 2);
        assert_eq!(player.item_count_by_entry(6948, true, None, &stored), 5);
        assert_eq!(
            player.item_count_by_entry(6948, true, Some(&inventory_item), &stored),
            3
        );
    }

    #[test]
    fn item_count_with_limit_category_matches_cpp_everywhere_and_skip_item() {
        let player = Player::new(None, false);
        let limited_template = ItemStorageTemplate {
            item_limit_category: 77,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };
        let other_template = ItemStorageTemplate {
            item_limit_category: 78,
            ..ItemStorageTemplate::regular_item(6949, 20)
        };
        let mut limited_item = Item::default();
        limited_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 620));
        limited_item.object_mut().set_entry(6948);
        limited_item.set_count(2);
        let mut bank_limited_item = Item::default();
        bank_limited_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 621));
        bank_limited_item.object_mut().set_entry(6948);
        bank_limited_item.set_count(3);
        let mut other_item = Item::default();
        other_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 622));
        other_item.object_mut().set_entry(6949);
        other_item.set_count(7);
        let stored = [
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &limited_item,
                Some(&limited_template),
            ),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                BANK_SLOT_ITEM_START,
                &bank_limited_item,
                Some(&limited_template),
            ),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &other_item,
                Some(&other_template),
            ),
        ];

        assert_eq!(player.item_count_with_limit_category(77, None, &stored), 5);
        assert_eq!(
            player.item_count_with_limit_category(77, Some(&limited_item), &stored),
            3
        );
    }

    #[test]
    fn item_by_entry_matches_cpp_for_each_item_order_and_stop() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);

        let equipped = item_with_guid_entry(640, 900);
        let inventory_bag = item_with_guid_entry(641, 900);
        let inventory_item = item_with_guid_entry(642, 900);
        let bag_item = item_with_guid_entry(643, 900);
        let bank_item = item_with_guid_entry(644, 900);

        player
            .store_top_level_item(EQUIPMENT_SLOT_CHEST, equipped.object().guid())
            .unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_BAG_START, inventory_bag.object().guid())
            .unwrap();
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, inventory_bag.object().guid(), 4)
            .unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, inventory_item.object().guid())
            .unwrap();
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 0, bag_item.object().guid())
            .unwrap();
        player
            .store_top_level_item(BANK_SLOT_ITEM_START, bank_item.object().guid())
            .unwrap();

        let stored = [
            ItemStorageRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, &bank_item, None),
            ItemStorageRef::new(INVENTORY_SLOT_BAG_START, 0, &bag_item, None),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &inventory_item,
                None,
            ),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_BAG_START,
                &inventory_bag,
                None,
            ),
            ItemStorageRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST, &equipped, None),
        ];

        let default_found = player
            .item_by_entry(900, ItemSearchLocation::DEFAULT, &stored)
            .unwrap();
        assert_eq!(default_found.item.object().guid(), equipped.object().guid());

        let inventory_found = player
            .item_by_entry(900, ItemSearchLocation::INVENTORY, &stored)
            .unwrap();
        assert_eq!(
            inventory_found.item.object().guid(),
            inventory_bag.object().guid()
        );

        assert!(
            player
                .item_by_entry(901, ItemSearchLocation::EVERYWHERE, &stored)
                .is_none()
        );
    }

    #[test]
    fn item_list_by_entry_matches_cpp_locations_bank_and_reagent_order() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);

        let equipped = item_with_guid_entry(650, 901);
        let inventory_item = item_with_guid_entry(651, 901);
        let bank_item = item_with_guid_entry(652, 901);
        let reagent_bag = item_with_guid_entry(653, 1);
        let reagent_item = item_with_guid_entry(654, 901);
        let other_item = item_with_guid_entry(655, 902);

        player
            .store_top_level_item(EQUIPMENT_SLOT_HEAD, equipped.object().guid())
            .unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, inventory_item.object().guid())
            .unwrap();
        player
            .store_top_level_item(BANK_SLOT_ITEM_START, bank_item.object().guid())
            .unwrap();
        player
            .store_top_level_item(REAGENT_BAG_SLOT_START, reagent_bag.object().guid())
            .unwrap();
        player
            .register_bag_storage(REAGENT_BAG_SLOT_START, reagent_bag.object().guid(), 3)
            .unwrap();
        player
            .store_bag_item(REAGENT_BAG_SLOT_START, 1, reagent_item.object().guid())
            .unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START + 1, other_item.object().guid())
            .unwrap();

        let stored = [
            ItemStorageRef::new(REAGENT_BAG_SLOT_START, 1, &reagent_item, None),
            ItemStorageRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, &bank_item, None),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &inventory_item,
                None,
            ),
            ItemStorageRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_HEAD, &equipped, None),
            ItemStorageRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START + 1,
                &other_item,
                None,
            ),
        ];

        let without_bank = player.item_list_by_entry(901, false, &stored);
        assert_eq!(
            without_bank
                .iter()
                .map(|stored| stored.item.object().guid())
                .collect::<Vec<_>>(),
            vec![
                equipped.object().guid(),
                inventory_item.object().guid(),
                reagent_item.object().guid(),
            ]
        );

        let with_bank = player.item_list_by_entry(901, true, &stored);
        assert_eq!(
            with_bank
                .iter()
                .map(|stored| stored.item.object().guid())
                .collect::<Vec<_>>(),
            vec![
                equipped.object().guid(),
                inventory_item.object().guid(),
                bank_item.object().guid(),
                reagent_item.object().guid(),
            ]
        );
    }

    #[test]
    fn can_store_item_preflight_matches_cpp_template_source_and_similar_guards() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);

        assert_eq!(
            player.can_store_item(
                &mut Vec::new(),
                can_store_args(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START, None, 3),
            ),
            CanStoreItemOutcome {
                result: InventoryResult::ItemNotFound,
                no_space_count: Some(3),
            }
        );

        let mut swap_missing =
            can_store_args(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START, None, 3);
        swap_missing.swap = true;
        assert_eq!(
            player.can_store_item(&mut Vec::new(), swap_missing),
            CanStoreItemOutcome {
                result: InventoryResult::CantSwap,
                no_space_count: Some(3),
            }
        );

        let mut source = Item::default();
        source.set_loot_generated(true);
        let mut loot_args = can_store_args(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            Some(&proto),
            3,
        );
        loot_args.source_item = Some(&source);
        assert_eq!(
            player.can_store_item(&mut Vec::new(), loot_args),
            CanStoreItemOutcome {
                result: InventoryResult::LootGone,
                no_space_count: Some(3),
            }
        );

        source.set_loot_generated(false);
        source.set_owner_guid(ObjectGuid::create_player(1, 42));
        source.set_item_flag(ItemFieldFlags::SOULBOUND);
        let mut bound_args = can_store_args(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            Some(&proto),
            3,
        );
        bound_args.source_item = Some(&source);
        assert_eq!(
            player.can_store_item(&mut Vec::new(), bound_args),
            CanStoreItemOutcome {
                result: InventoryResult::NotOwner,
                no_space_count: Some(3),
            }
        );

        let limited_proto = ItemStorageTemplate {
            max_count: 3,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };
        let mut similar_args = can_store_args(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            Some(&limited_proto),
            3,
        );
        let mut existing_limited = Item::default();
        existing_limited
            .object_mut()
            .create(ObjectGuid::create_item(1, 501));
        existing_limited.object_mut().set_entry(6948);
        existing_limited.set_count(3);
        let stored_limited = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START + 1,
            &existing_limited,
            Some(&limited_proto),
        )];
        similar_args.stored_items = &stored_limited;
        assert_eq!(
            player.can_store_item(&mut Vec::new(), similar_args),
            CanStoreItemOutcome {
                result: InventoryResult::ItemMaxCount,
                no_space_count: Some(3),
            }
        );
    }

    #[test]
    fn can_store_item_reports_item_max_count_after_partial_similar_limit_like_cpp() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(16);
        let proto = ItemStorageTemplate {
            max_count: 10,
            ..ItemStorageTemplate::regular_item(6948, 20)
        };
        let mut args = can_store_args(NULL_BAG, NULL_SLOT, Some(&proto), 5);
        let mut existing_limited = Item::default();
        existing_limited
            .object_mut()
            .create(ObjectGuid::create_item(1, 502));
        existing_limited.object_mut().set_entry(6948);
        existing_limited.set_count(7);
        let stored_limited = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START + 1,
            &existing_limited,
            Some(&proto),
        )];
        args.stored_items = &stored_limited;
        let mut dest = Vec::new();

        assert_eq!(
            player.can_store_item(&mut dest, args),
            CanStoreItemOutcome {
                result: InventoryResult::ItemMaxCount,
                no_space_count: Some(2),
            }
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                3,
            )]
        );
    }

    #[test]
    fn can_store_item_fills_specific_slot_then_continues_search_like_cpp() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(16);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut existing = Item::default();
        existing
            .object_mut()
            .create(ObjectGuid::create_item(1, 401));
        existing.object_mut().set_entry(6948);
        existing.set_count(15);
        let slot_items = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            &existing,
        )];
        let mut args = can_store_args(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            Some(&proto),
            10,
        );
        args.slot_items = &slot_items;
        let mut dest = Vec::new();

        assert_eq!(
            player.can_store_item(&mut dest, args),
            CanStoreItemOutcome {
                result: InventoryResult::Ok,
                no_space_count: None,
            }
        );
        assert_eq!(
            dest,
            vec![
                ItemPosCount::new(
                    make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
                    5,
                ),
                ItemPosCount::new(
                    make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1),
                    5,
                ),
            ]
        );
    }

    #[test]
    fn can_store_item_general_search_handles_new_bag_direct_equip_and_bag_in_bag() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(16);
        let bag_proto = ItemStorageTemplate {
            class_id: ItemClass::Container,
            subclass_id: ItemSubClassContainer::Container as u32,
            bonding: ItemBondingType::None,
            max_stack_size: 1,
            container_slots: 16,
            ..ItemStorageTemplate::regular_item(100, 1)
        };
        let mut dest = Vec::new();

        assert_eq!(
            player.can_store_item(
                &mut dest,
                can_store_args(NULL_BAG, NULL_SLOT, Some(&bag_proto), 1)
            ),
            CanStoreItemOutcome {
                result: InventoryResult::Ok,
                no_space_count: None,
            }
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START),
                1,
            )]
        );

        let source = Item::default();
        let mut bag_in_bag_args = can_store_args(NULL_BAG, NULL_SLOT, Some(&bag_proto), 1);
        bag_in_bag_args.source_item = Some(&source);
        bag_in_bag_args.source_is_not_empty_bag = true;
        assert_eq!(
            player.can_store_item(&mut Vec::new(), bag_in_bag_args),
            CanStoreItemOutcome {
                result: InventoryResult::BagInBag,
                no_space_count: None,
            }
        );
    }

    #[test]
    fn can_bank_item_preflight_matches_cpp_item_template_and_source_guards() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 700));
        source.object_mut().set_entry(6948);
        source.set_count(3);

        assert_eq!(
            player.can_bank_item(
                &mut Vec::new(),
                can_bank_args(
                    INVENTORY_SLOT_BAG_0,
                    BANK_SLOT_ITEM_START,
                    Some(&proto),
                    None
                ),
            ),
            InventoryResult::ItemNotFound
        );

        let mut missing_swap = can_bank_args(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_ITEM_START,
            Some(&proto),
            None,
        );
        missing_swap.swap = true;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), missing_swap),
            InventoryResult::CantSwap
        );

        assert_eq!(
            player.can_bank_item(
                &mut Vec::new(),
                can_bank_args(
                    INVENTORY_SLOT_BAG_0,
                    BANK_SLOT_ITEM_START,
                    None,
                    Some(&source)
                ),
            ),
            InventoryResult::ItemNotFound
        );

        source.set_loot_generated(true);
        assert_eq!(
            player.can_bank_item(
                &mut Vec::new(),
                can_bank_args(
                    INVENTORY_SLOT_BAG_0,
                    BANK_SLOT_ITEM_START,
                    Some(&proto),
                    Some(&source),
                ),
            ),
            InventoryResult::LootGone
        );

        source.set_loot_generated(false);
        source.set_owner_guid(ObjectGuid::create_player(1, 42));
        source.set_item_flag(ItemFieldFlags::SOULBOUND);
        assert_eq!(
            player.can_bank_item(
                &mut Vec::new(),
                can_bank_args(
                    INVENTORY_SLOT_BAG_0,
                    BANK_SLOT_ITEM_START,
                    Some(&proto),
                    Some(&source),
                ),
            ),
            InventoryResult::NotOwner
        );

        source.remove_item_flag(ItemFieldFlags::SOULBOUND);
        let mut currency_args = can_bank_args(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_ITEM_START,
            Some(&proto),
            Some(&source),
        );
        currency_args.source_is_currency_token = true;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), currency_args),
            InventoryResult::CantSwap
        );

        let limited_proto = ItemStorageTemplate {
            max_count: 3,
            ..proto
        };
        let mut existing = Item::default();
        existing
            .object_mut()
            .create(ObjectGuid::create_item(1, 701));
        existing.object_mut().set_entry(6948);
        existing.set_count(3);
        let stored = [ItemStorageRef::new(
            INVENTORY_SLOT_BAG_0,
            INVENTORY_SLOT_ITEM_START,
            &existing,
            Some(&limited_proto),
        )];
        let mut limit_args = can_bank_args(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_ITEM_START,
            Some(&limited_proto),
            Some(&source),
        );
        limit_args.stored_items = &stored;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), limit_args),
            InventoryResult::ItemMaxCount
        );
    }

    #[test]
    fn can_bank_item_specific_bank_bag_slot_matches_cpp_guards() {
        let mut player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 1);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 710));
        source.object_mut().set_entry(6948);
        source.set_count(1);

        assert_eq!(
            player.can_bank_item(
                &mut Vec::new(),
                can_bank_args(
                    INVENTORY_SLOT_BAG_0,
                    BANK_SLOT_BAG_START,
                    Some(&proto),
                    Some(&source),
                ),
            ),
            InventoryResult::WrongSlot
        );

        let mut bag_args = can_bank_args(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_BAG_START,
            Some(&proto),
            Some(&source),
        );
        bag_args.source_is_bag = true;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), bag_args),
            InventoryResult::NoBankSlot
        );

        player.set_bank_bag_slot_count(1);
        bag_args.can_use_result = InventoryResult::CantUseItem;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), bag_args),
            InventoryResult::CantUseItem
        );
    }

    #[test]
    fn can_bank_item_fills_specific_slot_then_continues_bank_search_like_cpp() {
        let player = Player::new(None, false);
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 720));
        source.object_mut().set_entry(6948);
        source.set_count(10);
        let mut existing = Item::default();
        existing
            .object_mut()
            .create(ObjectGuid::create_item(1, 721));
        existing.object_mut().set_entry(6948);
        existing.set_count(15);
        let slot_items = [ItemSlotRef::new(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_ITEM_START,
            &existing,
        )];
        let mut args = can_bank_args(
            INVENTORY_SLOT_BAG_0,
            BANK_SLOT_ITEM_START,
            Some(&proto),
            Some(&source),
        );
        args.slot_items = &slot_items;
        let mut dest = Vec::new();

        assert_eq!(player.can_bank_item(&mut dest, args), InventoryResult::Ok);
        assert_eq!(
            dest,
            vec![
                ItemPosCount::new(make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START), 5),
                ItemPosCount::new(
                    make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START + 1),
                    5
                ),
            ]
        );
    }

    #[test]
    fn can_bank_item_general_search_and_full_bank_match_cpp() {
        let proto = ItemStorageTemplate::regular_item(6948, 20);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 730));
        source.object_mut().set_entry(6948);
        source.set_count(3);
        let player = Player::new(None, false);
        let mut dest = Vec::new();

        assert_eq!(
            player.can_bank_item(
                &mut dest,
                can_bank_args(NULL_BAG, NULL_SLOT, Some(&proto), Some(&source)),
            ),
            InventoryResult::Ok
        );
        assert_eq!(
            dest,
            vec![ItemPosCount::new(
                make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START),
                3,
            )]
        );

        let mut occupied_items = Vec::new();
        for idx in 0..(BANK_SLOT_ITEM_END - BANK_SLOT_ITEM_START) {
            let mut occupied = Item::default();
            occupied
                .object_mut()
                .create(ObjectGuid::create_item(1, 800 + idx as i64));
            occupied.object_mut().set_entry(9999);
            occupied.set_count(1);
            occupied_items.push(occupied);
        }
        let slot_items = occupied_items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                ItemSlotRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START + idx as u8, item)
            })
            .collect::<Vec<_>>();
        let mut full_args = can_bank_args(NULL_BAG, NULL_SLOT, Some(&proto), Some(&source));
        full_args.slot_items = &slot_items;
        assert_eq!(
            player.can_bank_item(&mut Vec::new(), full_args),
            InventoryResult::BankFull
        );
    }

    #[test]
    fn player_identity_setters_mark_cpp_unit_and_playerdata_bits() {
        let mut player = Player::new(None, false);
        player.clear_data_changes();

        player.set_race_class_gender(1, 2, Gender::Female);
        player.set_selection(ObjectGuid::new(7, 11));

        assert_eq!(player.unit().data().race, 1);
        assert_eq!(player.unit().data().class_id, 2);
        assert_eq!(player.unit().data().player_class_id, 2);
        assert_eq!(player.unit().data().sex, Gender::Female as u8);
        assert_eq!(player.data().native_sex, Gender::Female as u8);
        assert_eq!(player.unit().data().target, ObjectGuid::new(7, 11));
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_NATIVE_SEX_BIT)
        );
    }

    #[test]
    fn player_flags_and_loot_guid_mark_playerdata_bits() {
        let mut player = Player::new(None, false);

        player.set_player_flag(0x20);
        player.set_player_flag_ex(0x04);
        player.set_loot_guid(ObjectGuid::new(9, 3));
        player.set_bank_bag_slot_count(6);
        player.set_primary_specialization(62);

        assert!(player.has_player_flag(0x20));
        assert!(player.has_player_flag_ex(0x04));
        assert_eq!(player.data().loot_target_guid, ObjectGuid::new(9, 3));
        assert_eq!(player.data().num_bank_slots, 6);
        assert_eq!(player.data().current_spec_id, 62);
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_PARENT_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_FLAGS_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_FLAGS_EX_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_LOOT_TARGET_GUID_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_NUM_BANK_SLOTS_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_CURRENT_SPEC_ID_BIT)
        );

        player.remove_player_flag(0x20);
        player.remove_player_flag_ex(0x04);
        assert!(!player.has_player_flag(0x20));
        assert!(!player.has_player_flag_ex(0x04));
    }

    #[test]
    fn money_matches_cpp_modify_clamps_and_active_playerdata_coinage_bit() {
        let mut player = Player::new(None, false);

        player.set_money(100);
        assert_eq!(player.active_data().coinage, 100);
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_COINAGE_BIT)
        );

        assert!(player.modify_money(-150));
        assert_eq!(player.active_data().coinage, 0);

        player.set_money(MAX_MONEY_AMOUNT - 1);
        assert!(!player.modify_money(2));
        assert_eq!(player.active_data().coinage, MAX_MONEY_AMOUNT - 1);
        assert!(!player.modify_money(i64::MAX));
        assert_eq!(player.active_data().coinage, MAX_MONEY_AMOUNT - 1);

        assert!(player.modify_money(1));
        assert_eq!(player.active_data().coinage, MAX_MONEY_AMOUNT);
    }

    #[test]
    fn active_player_fields_and_inventory_slots_mark_cpp_bits() {
        let mut player = Player::new(None, false);

        player.set_xp(123);
        player.set_next_level_xp(456);
        player.set_free_primary_professions(2);
        player.set_inventory_slot_count(16);
        player.set_inv_slot(3, ObjectGuid::new(4, 5));

        assert_eq!(player.active_data().xp, 123);
        assert_eq!(player.active_data().next_level_xp, 456);
        assert_eq!(player.active_data().character_points, 2);
        assert_eq!(player.active_data().num_backpack_slots, 16);
        assert_eq!(player.active_data().inv_slots[3], ObjectGuid::new(4, 5));
        assert_eq!(player.active_data().buyback_price, [0; BUYBACK_SLOT_COUNT]);
        assert_eq!(
            player.active_data().buyback_timestamp,
            [0; BUYBACK_SLOT_COUNT]
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_PARENT_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_XP_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_NEXT_LEVEL_XP_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_CHARACTER_POINTS_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_NUM_BACKPACK_SLOTS_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_INV_SLOTS_PARENT_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT + 3)
        );
    }

    #[test]
    fn values_update_splits_player_and_active_player_for_receiver() {
        let mut player = Player::new(None, false);

        player.set_player_flag(0x20);
        player.set_money(50);

        let other_view = player.values_update(false);
        assert!(other_view.has_data());
        assert_eq!(other_view.changed_object_type_mask, 1 << TYPEID_PLAYER);
        assert!(other_view.player_data.is_some());
        assert!(other_view.active_player_data.is_none());

        let self_view = player.values_update(true);
        assert_eq!(
            self_view.changed_object_type_mask,
            (1 << TYPEID_PLAYER) | (1 << TYPEID_ACTIVE_PLAYER)
        );
        assert!(self_view.active_player_data.is_some());
    }

    #[test]
    fn player_inventory_storage_matches_cpp_get_item_by_pos_rules() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);
        player.clear_active_player_data_changes();

        let equipped = ObjectGuid::create_item(1, 100);
        let bag_guid = ObjectGuid::create_item(1, 200);
        let bag_item = ObjectGuid::create_item(1, 201);
        let buyback = ObjectGuid::create_item(1, 300);

        player.store_top_level_item(0, equipped).unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_BAG_START, bag_guid)
            .unwrap();
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 2, bag_item)
            .unwrap();
        player
            .store_top_level_item(BUYBACK_SLOT_START, buyback)
            .unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, 0),
            Some(equipped)
        );
        assert_eq!(
            player.get_item_by_packed_pos((u16::from(INVENTORY_SLOT_BAG_0) << 8) | 0),
            Some(equipped)
        );
        assert_eq!(
            player.get_bag_by_pos(INVENTORY_SLOT_BAG_START),
            Some(bag_guid)
        );
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2),
            Some(bag_item)
        );
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, BUYBACK_SLOT_START),
            None
        );
        assert_eq!(
            player.get_item_from_buyback_slot(BUYBACK_SLOT_START),
            Some(buyback)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT)
        );
    }

    #[test]
    fn visible_item_slot_marks_cpp_playerdata_array_bits() {
        let mut player = Player::new(None, false);
        player.clear_data_changes();

        let visible = VisibleItemValues {
            item_id: 19019,
            item_appearance_mod_id: 7,
            item_visual: 3,
        };
        player.set_visible_item_slot(15, Some(visible));

        assert_eq!(player.data().visible_items[15], visible);
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_VISIBLE_ITEMS_PARENT_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT + 15)
        );

        player.clear_player_data_changes();
        player.set_visible_item_slot(15, None);
        assert_eq!(
            player.data().visible_items[15],
            VisibleItemValues::default()
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT + 15)
        );
    }

    #[test]
    fn visualize_item_updates_equipment_storage_and_visible_item_like_cpp() {
        let mut player = Player::new(None, false);
        player.clear_data_changes();
        player.clear_active_player_data_changes();

        let guid = ObjectGuid::create_item(1, 500);
        let visible = VisibleItemValues {
            item_id: 500,
            item_appearance_mod_id: 1,
            item_visual: 2,
        };

        player.visualize_item(0, guid, visible).unwrap();

        assert_eq!(player.get_item_by_pos(INVENTORY_SLOT_BAG_0, 0), Some(guid));
        assert_eq!(player.active_data().inv_slots[0], guid);
        assert_eq!(player.data().visible_items[0], visible);
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT)
        );
        assert!(
            player
                .player_data_changes_mask()
                .is_set(PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT)
        );

        player.remove_top_level_item(0).unwrap();
        assert_eq!(player.data().visible_items[0], VisibleItemValues::default());
        assert_eq!(player.active_data().inv_slots[0], ObjectGuid::EMPTY);
    }

    #[test]
    fn visualize_item_object_mutates_item_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 500);
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        let visible = VisibleItemValues {
            item_id: 500,
            item_appearance_mod_id: 1,
            item_visual: 2,
        };

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.clear_data_changes();
        player.clear_active_player_data_changes();
        item.object_mut().create(item_guid);
        item.set_container_guid_and_slot(ObjectGuid::create_item(1, 700), 4);
        item.set_bonding(ItemBondingType::OnEquip);
        item.force_state(ItemUpdateState::Unchanged);
        item.clear_item_data_changes();

        player.visualize_item_object(0, &mut item, visible).unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, 0),
            Some(item_guid)
        );
        assert_eq!(player.active_data().inv_slots[0], item_guid);
        assert_eq!(player.data().visible_items[0], visible);
        assert_eq!(item.data().contained_in, player_guid);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), 0);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert_eq!(item.bag_slot(), INVENTORY_SLOT_BAG_0);
        assert!(item.is_soul_bound());
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
    }

    #[test]
    fn equip_item_object_empty_slot_visualizes_and_flags_item_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 510);
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        let visible = VisibleItemValues {
            item_id: 510,
            item_appearance_mod_id: 4,
            item_visual: 9,
        };

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_bonding(ItemBondingType::OnEquip);
        item.force_state(ItemUpdateState::Unchanged);
        item.clear_item_data_changes();

        assert_eq!(
            player
                .equip_item_object(
                    make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND),
                    &mut item,
                    None,
                    visible,
                )
                .unwrap(),
            EquipItemObjectOutcome::Equipped
        );

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND),
            Some(item_guid)
        );
        assert_eq!(
            player.data().visible_items[EQUIPMENT_SLOT_MAINHAND as usize],
            visible
        );
        assert_eq!(item.data().contained_in, player_guid);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), EQUIPMENT_SLOT_MAINHAND);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert!(item.is_soul_bound());
        assert!(item.has_item_flag2(ItemFieldFlags2::EQUIPPED));
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
    }

    #[test]
    fn equip_item_object_merges_existing_stack_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let existing_guid = ObjectGuid::create_item(1, 511);
        let incoming_guid = ObjectGuid::create_item(1, 512);
        let mut player = Player::new(None, false);
        let mut existing = Item::default();
        let mut incoming = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        existing.object_mut().create(existing_guid);
        existing.set_count(2);
        existing.force_state(ItemUpdateState::Unchanged);
        incoming.object_mut().create(incoming_guid);
        incoming.set_count(3);
        incoming.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        incoming.force_state(ItemUpdateState::Unchanged);

        player
            .store_top_level_item(EQUIPMENT_SLOT_FINGER1, existing_guid)
            .unwrap();

        assert_eq!(
            player
                .equip_item_object(
                    make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_FINGER1),
                    &mut incoming,
                    Some(&mut existing),
                    VisibleItemValues::default(),
                )
                .unwrap(),
            EquipItemObjectOutcome::Merged
        );

        assert_eq!(existing.count(), 5);
        assert_eq!(existing.update_state(), ItemUpdateState::Changed);
        assert_eq!(incoming.owner_guid(), player_guid);
        assert!(!incoming.has_item_flag(ItemFieldFlags::REFUNDABLE));
        assert!(!incoming.has_item_flag(ItemFieldFlags::BOP_TRADEABLE));
        assert_eq!(incoming.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn quick_equip_item_object_visualizes_and_flags_item_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 513);
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        let visible = VisibleItemValues {
            item_id: 513,
            item_appearance_mod_id: 8,
            item_visual: 1,
        };

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.force_state(ItemUpdateState::Unchanged);

        player
            .quick_equip_item_object(
                make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_OFFHAND),
                &mut item,
                visible,
            )
            .unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_OFFHAND),
            Some(item_guid)
        );
        assert_eq!(
            player.data().visible_items[EQUIPMENT_SLOT_OFFHAND as usize],
            visible
        );
        assert_eq!(item.data().contained_in, player_guid);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), EQUIPMENT_SLOT_OFFHAND);
        assert!(item.has_item_flag2(ItemFieldFlags2::EQUIPPED));
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
    }

    #[test]
    fn remove_item_object_unlinks_equipment_without_clearing_owner_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 514);
        let mut player = Player::new(None, false);
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_owner_guid(player_guid);
        item.set_contained_in(player_guid);
        item.set_slot(EQUIPMENT_SLOT_MAINHAND);
        item.set_item_flag2(ItemFieldFlags2::EQUIPPED);
        player
            .visualize_item(
                EQUIPMENT_SLOT_MAINHAND,
                item_guid,
                VisibleItemValues {
                    item_id: 514,
                    item_appearance_mod_id: 3,
                    item_visual: 2,
                },
            )
            .unwrap();

        assert_eq!(
            player
                .remove_item_object(
                    INVENTORY_SLOT_BAG_0,
                    EQUIPMENT_SLOT_MAINHAND,
                    Some(&mut item),
                    None,
                )
                .unwrap(),
            Some(item_guid)
        );

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND),
            None
        );
        assert_eq!(
            player.data().visible_items[EQUIPMENT_SLOT_MAINHAND as usize],
            VisibleItemValues::default()
        );
        assert_eq!(item.data().contained_in, ObjectGuid::EMPTY);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), NULL_SLOT);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert!(!item.has_item_flag2(ItemFieldFlags2::EQUIPPED));
    }

    #[test]
    fn remove_item_object_unlinks_bag_item_like_cpp_bag_removeitem() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 800);
        let item_guid = ObjectGuid::create_item(1, 515);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        bag.item_mut().object_mut().create(bag_guid);
        bag.item_mut().set_owner_guid(player_guid);
        item.object_mut().create(item_guid);
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 10)
            .unwrap();
        bag.store_item(2, &mut item);
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 2, item_guid)
            .unwrap();

        assert_eq!(
            player
                .remove_item_object(INVENTORY_SLOT_BAG_START, 2, Some(&mut item), Some(&mut bag))
                .unwrap(),
            Some(item_guid)
        );

        assert_eq!(player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2), None);
        assert_eq!(bag.data().slots[2], ObjectGuid::EMPTY);
        assert_eq!(item.data().contained_in, ObjectGuid::EMPTY);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert_eq!(item.slot(), NULL_SLOT);
    }

    #[test]
    fn move_item_from_inventory_object_unlinks_and_clears_refund_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 516);
        let mut player = Player::new(None, false);
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_owner_guid(player_guid);
        item.set_contained_in(player_guid);
        item.set_slot(INVENTORY_SLOT_ITEM_START);
        item.set_item_flag(ItemFieldFlags::REFUNDABLE);
        item.set_refund_recipient(player_guid);
        item.set_paid_money(10);
        item.set_paid_extended_cost(20);
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, item_guid)
            .unwrap();

        assert_eq!(
            player
                .move_item_from_inventory_object(
                    INVENTORY_SLOT_BAG_0,
                    INVENTORY_SLOT_ITEM_START,
                    Some(&mut item),
                    None,
                )
                .unwrap(),
            Some(item_guid)
        );

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            None
        );
        assert_eq!(item.data().contained_in, ObjectGuid::EMPTY);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), NULL_SLOT);
        assert!(!item.has_item_flag(ItemFieldFlags::REFUNDABLE));
        assert_eq!(item.refund_recipient(), ObjectGuid::EMPTY);
        assert_eq!(item.paid_money(), 0);
        assert_eq!(item.paid_extended_cost(), 0);
    }

    #[test]
    fn finalize_move_item_to_inventory_object_marks_original_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 517);
        let other_owner = ObjectGuid::create_player(1, 77);
        let mut player = Player::new(None, false);
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_owner_guid(other_owner);
        item.force_state(ItemUpdateState::Unchanged);

        assert!(player.finalize_move_item_to_inventory_object(item_guid, &mut item, false));
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.update_state(), ItemUpdateState::New);

        item.force_state(ItemUpdateState::Unchanged);
        assert!(player.finalize_move_item_to_inventory_object(item_guid, &mut item, true));
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
    }

    #[test]
    fn finalize_move_item_to_inventory_object_skips_merged_stack_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let original_guid = ObjectGuid::create_item(1, 518);
        let merged_guid = ObjectGuid::create_item(1, 519);
        let mut player = Player::new(None, false);
        let mut merged = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        merged.object_mut().create(merged_guid);
        merged.force_state(ItemUpdateState::Unchanged);

        assert!(!player.finalize_move_item_to_inventory_object(original_guid, &mut merged, false));
        assert_eq!(merged.owner_guid(), ObjectGuid::EMPTY);
        assert_eq!(merged.update_state(), ItemUpdateState::Unchanged);
    }

    #[test]
    fn destroy_item_object_removes_top_level_item_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 520);
        let mut player = Player::new(None, false);
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_owner_guid(player_guid);
        item.set_contained_in(player_guid);
        item.set_slot(EQUIPMENT_SLOT_MAINHAND);
        item.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        item.set_item_flag2(ItemFieldFlags2::EQUIPPED);
        item.force_state(ItemUpdateState::Unchanged);
        player
            .visualize_item(
                EQUIPMENT_SLOT_MAINHAND,
                item_guid,
                VisibleItemValues {
                    item_id: 520,
                    item_appearance_mod_id: 6,
                    item_visual: 7,
                },
            )
            .unwrap();

        assert_eq!(
            player
                .destroy_item_object(
                    INVENTORY_SLOT_BAG_0,
                    EQUIPMENT_SLOT_MAINHAND,
                    Some(&mut item),
                    None,
                )
                .unwrap(),
            Some(item_guid)
        );

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND),
            None
        );
        assert_eq!(
            player.data().visible_items[EQUIPMENT_SLOT_MAINHAND as usize],
            VisibleItemValues::default()
        );
        assert_eq!(item.data().contained_in, ObjectGuid::EMPTY);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), NULL_SLOT);
        assert!(!item.has_item_flag(ItemFieldFlags::REFUNDABLE));
        assert!(!item.has_item_flag(ItemFieldFlags::BOP_TRADEABLE));
        assert!(item.has_item_flag2(ItemFieldFlags2::EQUIPPED));
        assert_eq!(item.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn destroy_item_object_removes_bag_item_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 801);
        let item_guid = ObjectGuid::create_item(1, 521);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        bag.item_mut().object_mut().create(bag_guid);
        bag.item_mut().set_owner_guid(player_guid);
        item.object_mut().create(item_guid);
        item.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        item.force_state(ItemUpdateState::Unchanged);
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 10)
            .unwrap();
        bag.store_item(3, &mut item);
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 3, item_guid)
            .unwrap();

        assert_eq!(
            player
                .destroy_item_object(INVENTORY_SLOT_BAG_START, 3, Some(&mut item), Some(&mut bag))
                .unwrap(),
            Some(item_guid)
        );

        assert_eq!(player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 3), None);
        assert_eq!(bag.data().slots[3], ObjectGuid::EMPTY);
        assert_eq!(item.data().contained_in, ObjectGuid::EMPTY);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert_eq!(item.slot(), NULL_SLOT);
        assert!(!item.has_item_flag(ItemFieldFlags::REFUNDABLE));
        assert!(!item.has_item_flag(ItemFieldFlags::BOP_TRADEABLE));
        assert_eq!(item.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn destroy_item_count_for_item_object_decrements_partial_stack_like_cpp() {
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        let mut count = 3;

        item.set_count(8);
        item.force_state(ItemUpdateState::Unchanged);

        player
            .destroy_item_count_for_item_object(Some(&mut item), &mut count, None)
            .unwrap();

        assert_eq!(item.count(), 5);
        assert_eq!(count, 0);
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
    }

    #[test]
    fn destroy_item_count_for_item_object_destroys_full_stack_like_cpp() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 522);
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        let mut count = 7;

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        item.object_mut().create(item_guid);
        item.set_owner_guid(player_guid);
        item.set_contained_in(player_guid);
        item.set_slot(INVENTORY_SLOT_ITEM_START);
        item.set_count(5);
        item.force_state(ItemUpdateState::Unchanged);
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, item_guid)
            .unwrap();

        player
            .destroy_item_count_for_item_object(Some(&mut item), &mut count, None)
            .unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            None
        );
        assert_eq!(item.slot(), NULL_SLOT);
        assert_eq!(item.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn destroy_item_count_by_entry_plan_matches_cpp_scan_order_and_partial_stop() {
        let player = Player::new(None, false);
        let mut inventory = Item::default();
        let mut bag_item = Item::default();
        let mut bank = Item::default();

        inventory.object_mut().set_entry(900);
        inventory.set_count(2);
        bag_item.object_mut().set_entry(900);
        bag_item.set_count(3);
        bank.object_mut().set_entry(900);
        bank.set_count(5);

        let items = [
            DestroyItemCountItemRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, &bank),
            DestroyItemCountItemRef::new(INVENTORY_SLOT_BAG_START, 4, &bag_item),
            DestroyItemCountItemRef::new(
                INVENTORY_SLOT_BAG_0,
                INVENTORY_SLOT_ITEM_START,
                &inventory,
            ),
        ];

        let plan = player.destroy_item_count_by_entry_plan(900, 4, false, 16, &items);

        assert_eq!(plan.removed_count, 4);
        assert_eq!(
            plan.actions,
            vec![
                DestroyItemCountAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: INVENTORY_SLOT_ITEM_START,
                    removed_count: 2,
                    remaining_count: 0,
                    destroy_stack: true,
                },
                DestroyItemCountAction {
                    bag: INVENTORY_SLOT_BAG_START,
                    slot: 4,
                    removed_count: 2,
                    remaining_count: 1,
                    destroy_stack: false,
                },
            ]
        );
    }

    #[test]
    fn destroy_item_count_by_entry_plan_matches_cpp_unequip_check_for_full_equipment_stack() {
        let player = Player::new(None, false);
        let mut equipped = Item::default();
        let mut bank = Item::default();

        equipped.object_mut().set_entry(901);
        equipped.set_count(1);
        bank.object_mut().set_entry(901);
        bank.set_count(1);

        let mut blocked_equipped =
            DestroyItemCountItemRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_MAINHAND, &equipped);
        blocked_equipped.can_unequip_result = InventoryResult::CantEquipEver;
        let items = [
            blocked_equipped,
            DestroyItemCountItemRef::new(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START, &bank),
        ];

        let plan = player.destroy_item_count_by_entry_plan(901, 1, true, 16, &items);

        assert_eq!(plan.removed_count, 1);
        assert_eq!(
            plan.actions,
            vec![DestroyItemCountAction {
                bag: INVENTORY_SLOT_BAG_0,
                slot: BANK_SLOT_ITEM_START,
                removed_count: 1,
                remaining_count: 0,
                destroy_stack: true,
            }]
        );
    }

    #[test]
    fn destroy_zone_limited_item_plan_matches_cpp_scan_order() {
        let player = Player::new(None, false);
        let items = [
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, KEYRING_SLOT_START, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_START, 2, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1, false),
        ];

        assert_eq!(
            player.destroy_zone_limited_item_plan(16, &items),
            vec![
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: INVENTORY_SLOT_ITEM_START,
                },
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: KEYRING_SLOT_START,
                },
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_START,
                    slot: 2,
                },
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: EQUIPMENT_SLOT_CHEST,
                },
            ]
        );
    }

    #[test]
    fn destroy_conjured_items_plan_matches_cpp_scan_order_without_keyring() {
        let player = Player::new(None, false);
        let items = [
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, KEYRING_SLOT_START, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_START, 1, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST, true),
            DestroyFilteredItemRef::new(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START, true),
        ];

        assert_eq!(
            player.destroy_conjured_items_plan(16, &items),
            vec![
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: INVENTORY_SLOT_ITEM_START,
                },
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_START,
                    slot: 1,
                },
                DestroyFilteredItemAction {
                    bag: INVENTORY_SLOT_BAG_0,
                    slot: EQUIPMENT_SLOT_CHEST,
                },
            ]
        );
    }

    #[test]
    fn store_item_object_mutates_empty_top_level_slot_like_cpp_storeitem() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let item_guid = ObjectGuid::create_item(1, 600);
        let mut player = Player::new(None, false);
        let mut item = Item::default();

        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.clear_active_player_data_changes();
        item.object_mut().create(item_guid);
        item.set_bonding(ItemBondingType::OnAcquire);
        item.force_state(ItemUpdateState::Unchanged);
        item.clear_item_data_changes();

        player
            .store_item_object(INVENTORY_SLOT_ITEM_START, &mut item, 4)
            .unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            Some(item_guid)
        );
        assert_eq!(
            player.active_data().inv_slots[INVENTORY_SLOT_ITEM_START as usize],
            item_guid
        );
        assert_eq!(item.count(), 4);
        assert_eq!(item.data().contained_in, player_guid);
        assert_eq!(item.owner_guid(), player_guid);
        assert_eq!(item.slot(), INVENTORY_SLOT_ITEM_START);
        assert_eq!(item.container_guid(), ObjectGuid::EMPTY);
        assert_eq!(item.bag_slot(), INVENTORY_SLOT_BAG_0);
        assert!(item.is_soul_bound());
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
        assert!(
            player.active_player_data_changes_mask().is_set(
                ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT + INVENTORY_SLOT_ITEM_START as usize
            )
        );
    }

    #[test]
    fn store_item_object_binds_on_equip_only_for_bag_positions_like_cpp_storeitem() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let mut player = Player::new(None, false);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);

        let mut inventory_item = Item::default();
        inventory_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 601));
        inventory_item.set_bonding(ItemBondingType::OnEquip);
        player
            .store_item_object(INVENTORY_SLOT_ITEM_START, &mut inventory_item, 1)
            .unwrap();
        assert!(!inventory_item.is_soul_bound());

        let mut bag_item = Item::default();
        bag_item
            .object_mut()
            .create(ObjectGuid::create_item(1, 602));
        bag_item.set_bonding(ItemBondingType::OnEquip);
        player
            .store_item_object(INVENTORY_SLOT_BAG_START, &mut bag_item, 1)
            .unwrap();
        assert!(bag_item.is_soul_bound());
    }

    #[test]
    fn store_item_object_rejects_occupied_slot_until_stack_merge_registry_exists() {
        let existing = ObjectGuid::create_item(1, 700);
        let incoming = ObjectGuid::create_item(1, 701);
        let mut player = Player::new(None, false);
        let mut item = Item::default();
        item.object_mut().create(incoming);
        item.force_state(ItemUpdateState::Unchanged);

        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, existing)
            .unwrap();
        let result = player.store_item_object(INVENTORY_SLOT_ITEM_START, &mut item, 3);

        assert_eq!(
            result,
            Err(PlayerStorageError::OccupiedPlayerSlot(
                INVENTORY_SLOT_ITEM_START
            ))
        );
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            Some(existing)
        );
        assert_eq!(item.count(), 0);
        assert_eq!(item.update_state(), ItemUpdateState::Unchanged);
    }

    #[test]
    fn store_cloned_item_object_keeps_source_and_stores_clone_like_cpp_storeitem_clone() {
        let owner = ObjectGuid::create_player(1, 42);
        let source_guid = ObjectGuid::create_item(1, 760);
        let clone_guid = ObjectGuid::create_item(1, 761);
        let mut player = Player::new(None, false);
        let mut source = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        source.object_mut().create(source_guid);
        source.object_mut().set_entry(6948);
        source.set_count(8);
        source.set_bonding(ItemBondingType::OnAcquire);
        source.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        source.force_state(ItemUpdateState::Unchanged);

        let cloned = player
            .store_cloned_item_object(INVENTORY_SLOT_ITEM_START, &source, clone_guid, 3)
            .unwrap();

        assert_eq!(source.object().guid(), source_guid);
        assert_eq!(source.count(), 8);
        assert!(source.is_refundable());
        assert!(source.is_bop_tradeable());
        assert_eq!(source.update_state(), ItemUpdateState::Unchanged);
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            Some(clone_guid)
        );
        assert_eq!(cloned.object().guid(), clone_guid);
        assert_eq!(cloned.object().entry(), 6948);
        assert_eq!(cloned.count(), 3);
        assert_eq!(cloned.owner_guid(), owner);
        assert!(cloned.is_soul_bound());
        assert!(!cloned.is_refundable());
        assert!(!cloned.is_bop_tradeable());
        assert_eq!(cloned.slot(), INVENTORY_SLOT_ITEM_START);
        assert_eq!(cloned.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn split_item_to_empty_top_level_object_matches_cpp_split_allocation() {
        let owner = ObjectGuid::create_player(1, 42);
        let source_guid = ObjectGuid::create_item(1, 762);
        let clone_guid = ObjectGuid::create_item(1, 763);
        let mut player = Player::new(None, false);
        let mut source = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        source.object_mut().create(source_guid);
        source.object_mut().set_entry(6948);
        source.set_count(8);
        source.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        source.force_state(ItemUpdateState::Unchanged);
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, source_guid)
            .unwrap();

        let cloned = player
            .split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START + 1,
                &mut source,
                clone_guid,
                3,
            )
            .unwrap();

        assert_eq!(source.count(), 5);
        assert_eq!(source.update_state(), ItemUpdateState::Changed);
        assert!(source.is_refundable());
        assert!(source.is_bop_tradeable());
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START),
            Some(source_guid)
        );
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1),
            Some(clone_guid)
        );
        assert_eq!(cloned.object().guid(), clone_guid);
        assert_eq!(cloned.count(), 3);
        assert!(!cloned.is_refundable());
        assert!(!cloned.is_bop_tradeable());
        assert_eq!(cloned.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn split_item_to_empty_top_level_object_rolls_back_source_like_cpp_on_failure() {
        let owner = ObjectGuid::create_player(1, 42);
        let source_guid = ObjectGuid::create_item(1, 764);
        let occupied_guid = ObjectGuid::create_item(1, 765);
        let clone_guid = ObjectGuid::create_item(1, 766);
        let mut player = Player::new(None, false);
        let mut source = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        source.object_mut().create(source_guid);
        source.object_mut().set_entry(6948);
        source.set_count(8);
        source.force_state(ItemUpdateState::Unchanged);
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, source_guid)
            .unwrap();
        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START + 1, occupied_guid)
            .unwrap();

        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START + 1,
                &mut source,
                clone_guid,
                3,
            ),
            Err(PlayerStorageError::OccupiedPlayerSlot(
                INVENTORY_SLOT_ITEM_START + 1
            ))
        );

        assert_eq!(source.count(), 8);
        assert_eq!(source.update_state(), ItemUpdateState::Unchanged);
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1),
            Some(occupied_guid)
        );
    }

    #[test]
    fn store_bag_item_object_mutates_bag_branch_like_cpp_storeitem() {
        let owner = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 800);
        let item_guid = ObjectGuid::create_item(1, 801);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut item = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: bag_guid,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        bag.item_mut().set_slot(INVENTORY_SLOT_BAG_START);
        bag.item_mut().force_state(ItemUpdateState::Unchanged);
        bag.clear_container_data_changes();
        item.object_mut().create(item_guid);
        item.set_bonding(ItemBondingType::Quest);
        item.force_state(ItemUpdateState::Unchanged);

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        player
            .store_bag_item_object(INVENTORY_SLOT_BAG_START, &mut bag, 2, &mut item, 3)
            .unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2),
            Some(item_guid)
        );
        assert_eq!(bag.item_by_pos(2), Some(item_guid));
        assert_eq!(item.count(), 3);
        assert_eq!(item.data().contained_in, bag_guid);
        assert_eq!(item.owner_guid(), owner);
        assert_eq!(item.container_guid(), bag_guid);
        assert_eq!(item.bag_slot(), INVENTORY_SLOT_BAG_START);
        assert_eq!(item.slot(), 2);
        assert!(item.is_soul_bound());
        assert_eq!(item.update_state(), ItemUpdateState::Changed);
        assert_eq!(bag.item().update_state(), ItemUpdateState::Changed);
        assert!(
            bag.container_data_changes_mask()
                .is_set(crate::CONTAINER_DATA_SLOTS_FIRST_BIT + 2)
        );
    }

    #[test]
    fn store_bag_item_object_rejects_mismatched_or_occupied_bag_slot() {
        let owner = ObjectGuid::create_player(1, 42);
        let registered_bag = ObjectGuid::create_item(1, 810);
        let actual_bag = ObjectGuid::create_item(1, 811);
        let existing = ObjectGuid::create_item(1, 812);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut item = Item::default();

        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: actual_bag,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        item.object_mut().create(ObjectGuid::create_item(1, 813));
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, registered_bag, 4)
            .unwrap();

        assert_eq!(
            player.store_bag_item_object(INVENTORY_SLOT_BAG_START, &mut bag, 2, &mut item, 1),
            Err(PlayerStorageError::MismatchedBagGuid {
                bag: INVENTORY_SLOT_BAG_START,
                expected: registered_bag,
                actual: actual_bag,
            })
        );

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START + 1, actual_bag, 4)
            .unwrap();
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START + 1, 2, existing)
            .unwrap();
        assert_eq!(
            player.store_bag_item_object(INVENTORY_SLOT_BAG_START + 1, &mut bag, 2, &mut item, 1),
            Err(PlayerStorageError::OccupiedBagItemSlot {
                bag: INVENTORY_SLOT_BAG_START + 1,
                slot: 2,
            })
        );
        assert_eq!(item.count(), 0);
        assert_eq!(bag.item_by_pos(2), None);
    }

    #[test]
    fn store_cloned_bag_item_object_keeps_source_and_stores_clone_like_cpp_storeitem_clone() {
        let owner = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 860);
        let source_guid = ObjectGuid::create_item(1, 861);
        let clone_guid = ObjectGuid::create_item(1, 862);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut source = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: bag_guid,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        bag.item_mut().set_slot(INVENTORY_SLOT_BAG_START);
        source.object_mut().create(source_guid);
        source.object_mut().set_entry(6948);
        source.set_count(8);
        source.set_bonding(ItemBondingType::OnEquip);
        source.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        source.force_state(ItemUpdateState::Unchanged);

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        let cloned = player
            .store_cloned_bag_item_object(
                INVENTORY_SLOT_BAG_START,
                &mut bag,
                2,
                &source,
                clone_guid,
                3,
            )
            .unwrap();

        assert_eq!(source.object().guid(), source_guid);
        assert_eq!(source.count(), 8);
        assert!(source.is_refundable());
        assert!(source.is_bop_tradeable());
        assert_eq!(source.update_state(), ItemUpdateState::Unchanged);
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2),
            Some(clone_guid)
        );
        assert_eq!(bag.item_by_pos(2), Some(clone_guid));
        assert_eq!(cloned.object().guid(), clone_guid);
        assert_eq!(cloned.object().entry(), 6948);
        assert_eq!(cloned.count(), 3);
        assert_eq!(cloned.owner_guid(), owner);
        assert!(!cloned.is_soul_bound());
        assert!(!cloned.is_refundable());
        assert!(!cloned.is_bop_tradeable());
        assert_eq!(cloned.container_guid(), bag_guid);
        assert_eq!(cloned.bag_slot(), INVENTORY_SLOT_BAG_START);
        assert_eq!(cloned.slot(), 2);
        assert_eq!(cloned.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn split_item_to_empty_bag_item_object_matches_cpp_split_allocation() {
        let owner = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 870);
        let source_guid = ObjectGuid::create_item(1, 871);
        let clone_guid = ObjectGuid::create_item(1, 872);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut source = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: bag_guid,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        bag.item_mut().set_slot(INVENTORY_SLOT_BAG_START);
        source.object_mut().create(source_guid);
        source.object_mut().set_entry(6948);
        source.set_count(8);
        source.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        bag.store_item(1, &mut source);
        source.force_state(ItemUpdateState::Unchanged);

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 1, source_guid)
            .unwrap();
        let cloned = player
            .split_item_to_empty_bag_item_object(
                INVENTORY_SLOT_BAG_START,
                &mut bag,
                2,
                &mut source,
                clone_guid,
                3,
            )
            .unwrap();

        assert_eq!(source.count(), 5);
        assert_eq!(source.update_state(), ItemUpdateState::Changed);
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 1),
            Some(source_guid)
        );
        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2),
            Some(clone_guid)
        );
        assert_eq!(bag.item_by_pos(1), Some(source_guid));
        assert_eq!(bag.item_by_pos(2), Some(clone_guid));
        assert_eq!(cloned.object().guid(), clone_guid);
        assert_eq!(cloned.count(), 3);
        assert!(!cloned.is_refundable());
        assert!(!cloned.is_bop_tradeable());
        assert_eq!(cloned.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn split_item_rejects_zero_all_or_too_many_like_cpp_guards() {
        let mut player = Player::new(None, false);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 880));
        source.set_count(8);

        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 881),
                0,
            ),
            Err(PlayerStorageError::InvalidSplitCount {
                available: 8,
                requested: 0,
            })
        );
        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 882),
                8,
            ),
            Err(PlayerStorageError::InvalidSplitCount {
                available: 8,
                requested: 8,
            })
        );
        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 883),
                9,
            ),
            Err(PlayerStorageError::TooFewItemsToSplit {
                available: 8,
                requested: 9,
            })
        );
        assert_eq!(source.count(), 8);
        assert_eq!(source.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn split_item_rejects_loot_and_trade_states_in_cpp_order() {
        let mut player = Player::new(None, false);
        let mut source = Item::default();
        source.object_mut().create(ObjectGuid::create_item(1, 884));
        source.set_count(8);
        source.set_loot_generated(true);
        source.set_in_trade(true);

        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 885),
                8,
            ),
            Err(PlayerStorageError::SplitItemLootGenerated)
        );

        source.set_loot_generated(false);
        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 886),
                8,
            ),
            Err(PlayerStorageError::InvalidSplitCount {
                available: 8,
                requested: 8,
            })
        );
        assert_eq!(
            player.split_item_to_empty_top_level_object(
                INVENTORY_SLOT_ITEM_START,
                &mut source,
                ObjectGuid::create_item(1, 887),
                3,
            ),
            Err(PlayerStorageError::SplitItemInTrade)
        );
        assert_eq!(source.count(), 8);
        assert_eq!(source.update_state(), ItemUpdateState::New);
    }

    #[test]
    fn merge_top_level_item_stack_object_matches_cpp_existing_stack_branch() {
        let owner = ObjectGuid::create_player(1, 42);
        let existing_guid = ObjectGuid::create_item(1, 820);
        let incoming_guid = ObjectGuid::create_item(1, 821);
        let mut player = Player::new(None, false);
        let mut existing = Item::default();
        let mut incoming = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        existing.object_mut().create(existing_guid);
        existing.set_bonding(ItemBondingType::OnEquip);
        existing.set_count(5);
        existing.force_state(ItemUpdateState::Unchanged);
        incoming.object_mut().create(incoming_guid);
        incoming.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        incoming.set_refund_recipient(ObjectGuid::create_player(1, 99));
        incoming.set_paid_money(10);
        incoming.set_paid_extended_cost(20);
        incoming.force_state(ItemUpdateState::Unchanged);

        player
            .store_top_level_item(INVENTORY_SLOT_BAG_START, existing_guid)
            .unwrap();
        player
            .merge_top_level_item_stack_object(
                INVENTORY_SLOT_BAG_START,
                &mut existing,
                &mut incoming,
                3,
            )
            .unwrap();

        assert_eq!(existing.count(), 8);
        assert!(existing.is_soul_bound());
        assert_eq!(existing.update_state(), ItemUpdateState::Changed);
        assert_eq!(incoming.owner_guid(), owner);
        assert!(!incoming.is_refundable());
        assert!(!incoming.is_bop_tradeable());
        assert_eq!(incoming.refund_recipient(), ObjectGuid::EMPTY);
        assert_eq!(incoming.paid_money(), 0);
        assert_eq!(incoming.paid_extended_cost(), 0);
        assert_eq!(incoming.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn merge_top_level_item_stack_object_rejects_empty_or_mismatched_slot() {
        let expected = ObjectGuid::create_item(1, 830);
        let actual = ObjectGuid::create_item(1, 831);
        let mut player = Player::new(None, false);
        let mut existing = Item::default();
        let mut incoming = Item::default();
        existing.object_mut().create(actual);

        assert_eq!(
            player.merge_top_level_item_stack_object(
                INVENTORY_SLOT_ITEM_START,
                &mut existing,
                &mut incoming,
                1,
            ),
            Err(PlayerStorageError::EmptyPlayerSlot(
                INVENTORY_SLOT_ITEM_START
            ))
        );

        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, expected)
            .unwrap();
        assert_eq!(
            player.merge_top_level_item_stack_object(
                INVENTORY_SLOT_ITEM_START,
                &mut existing,
                &mut incoming,
                1,
            ),
            Err(PlayerStorageError::MismatchedItemGuid {
                slot: INVENTORY_SLOT_ITEM_START,
                expected,
                actual,
            })
        );
    }

    #[test]
    fn merge_bag_item_stack_object_matches_cpp_existing_stack_branch() {
        let owner = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 840);
        let existing_guid = ObjectGuid::create_item(1, 841);
        let incoming_guid = ObjectGuid::create_item(1, 842);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut existing = Item::default();
        let mut incoming = Item::default();

        player.unit_mut().world_mut().object_mut().create(owner);
        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: bag_guid,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        bag.item_mut().set_slot(INVENTORY_SLOT_BAG_START);
        bag.item_mut().force_state(ItemUpdateState::Unchanged);
        existing.object_mut().create(existing_guid);
        existing.set_bonding(ItemBondingType::OnEquip);
        existing.set_count(5);
        existing.force_state(ItemUpdateState::Unchanged);
        incoming.object_mut().create(incoming_guid);
        incoming.set_item_flag(ItemFieldFlags::REFUNDABLE | ItemFieldFlags::BOP_TRADEABLE);
        incoming.set_refund_recipient(ObjectGuid::create_player(1, 99));
        incoming.set_paid_money(10);
        incoming.set_paid_extended_cost(20);
        incoming.force_state(ItemUpdateState::Unchanged);
        bag.store_item(2, &mut existing);

        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();
        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 2, existing_guid)
            .unwrap();
        player
            .merge_bag_item_stack_object(
                INVENTORY_SLOT_BAG_START,
                &bag,
                2,
                &mut existing,
                &mut incoming,
                3,
            )
            .unwrap();

        assert_eq!(
            player.get_item_by_pos(INVENTORY_SLOT_BAG_START, 2),
            Some(existing_guid)
        );
        assert_eq!(bag.item_by_pos(2), Some(existing_guid));
        assert_eq!(existing.count(), 8);
        assert!(!existing.is_soul_bound());
        assert_eq!(existing.update_state(), ItemUpdateState::Changed);
        assert_eq!(bag.item().update_state(), ItemUpdateState::Unchanged);
        assert_eq!(incoming.owner_guid(), owner);
        assert!(!incoming.is_refundable());
        assert!(!incoming.is_bop_tradeable());
        assert_eq!(incoming.refund_recipient(), ObjectGuid::EMPTY);
        assert_eq!(incoming.paid_money(), 0);
        assert_eq!(incoming.paid_extended_cost(), 0);
        assert_eq!(incoming.update_state(), ItemUpdateState::Removed);
    }

    #[test]
    fn merge_bag_item_stack_object_rejects_empty_or_mismatched_slot() {
        let owner = ObjectGuid::create_player(1, 42);
        let bag_guid = ObjectGuid::create_item(1, 850);
        let expected = ObjectGuid::create_item(1, 851);
        let actual = ObjectGuid::create_item(1, 852);
        let mut player = Player::new(None, false);
        let mut bag = Bag::default();
        let mut existing = Item::default();
        let mut incoming = Item::default();

        bag.try_initialize_created_state(crate::BagCreateInfo {
            guid: bag_guid,
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        existing.object_mut().create(actual);
        player
            .register_bag_storage(INVENTORY_SLOT_BAG_START, bag_guid, 4)
            .unwrap();

        assert_eq!(
            player.merge_bag_item_stack_object(
                INVENTORY_SLOT_BAG_START,
                &bag,
                2,
                &mut existing,
                &mut incoming,
                1,
            ),
            Err(PlayerStorageError::EmptyBagItemSlot {
                bag: INVENTORY_SLOT_BAG_START,
                slot: 2,
            })
        );

        player
            .store_bag_item(INVENTORY_SLOT_BAG_START, 2, expected)
            .unwrap();
        assert_eq!(
            player.merge_bag_item_stack_object(
                INVENTORY_SLOT_BAG_START,
                &bag,
                2,
                &mut existing,
                &mut incoming,
                1,
            ),
            Err(PlayerStorageError::MismatchedBagItemGuid {
                bag: INVENTORY_SLOT_BAG_START,
                slot: 2,
                expected,
                actual: ObjectGuid::EMPTY,
            })
        );

        bag.store_item(2, &mut existing);
        assert_eq!(
            player.merge_bag_item_stack_object(
                INVENTORY_SLOT_BAG_START,
                &bag,
                2,
                &mut existing,
                &mut incoming,
                1,
            ),
            Err(PlayerStorageError::MismatchedBagItemGuid {
                bag: INVENTORY_SLOT_BAG_START,
                slot: 2,
                expected,
                actual,
            })
        );
    }

    #[test]
    fn player_get_item_by_guid_scans_everywhere_except_buyback_like_cpp_for_each_item() {
        let mut player = Player::new(None, false);
        player.set_inventory_slot_count(INVENTORY_DEFAULT_SIZE);

        let inventory_item = ObjectGuid::create_item(1, 10);
        let bank_item = ObjectGuid::create_item(1, 11);
        let reagent_bag = ObjectGuid::create_item(1, 12);
        let reagent_item = ObjectGuid::create_item(1, 13);
        let buyback = ObjectGuid::create_item(1, 14);

        player
            .store_top_level_item(INVENTORY_SLOT_ITEM_START, inventory_item)
            .unwrap();
        player
            .store_top_level_item(BANK_SLOT_ITEM_START, bank_item)
            .unwrap();
        player
            .store_top_level_item(REAGENT_BAG_SLOT_START, reagent_bag)
            .unwrap();
        player
            .register_bag_storage(REAGENT_BAG_SLOT_START, reagent_bag, 3)
            .unwrap();
        player
            .store_bag_item(REAGENT_BAG_SLOT_START, 1, reagent_item)
            .unwrap();
        player
            .store_top_level_item(BUYBACK_SLOT_START, buyback)
            .unwrap();

        assert_eq!(
            player.get_item_by_guid(inventory_item),
            Some(inventory_item)
        );
        assert_eq!(player.get_item_by_guid(bank_item), Some(bank_item));
        assert_eq!(player.get_item_by_guid(reagent_item), Some(reagent_item));
        assert_eq!(player.get_item_by_guid(buyback), None);

        let mut visited = Vec::new();
        let completed = player.for_each_item_guid(ItemSearchLocation::INVENTORY, |guid| {
            visited.push(guid);
            ItemSearchCallbackResult::Continue
        });
        assert!(completed);
        assert!(visited.contains(&inventory_item));
        assert!(!visited.contains(&bank_item));
    }

    #[test]
    fn player_buyback_slots_follow_cpp_current_slot_and_masks() {
        let mut player = Player::new(None, false);
        player.clear_active_player_data_changes();

        let first = ObjectGuid::create_item(1, 1000);
        let second = ObjectGuid::create_item(1, 1001);

        let first_slot = player.add_item_to_buyback_slot(first, 123, 456);
        assert_eq!(first_slot, BUYBACK_SLOT_START);
        assert_eq!(
            player.inventory().current_buyback_slot,
            BUYBACK_SLOT_START + 1
        );
        assert_eq!(player.get_item_from_buyback_slot(first_slot), Some(first));
        assert_eq!(player.active_data().buyback_price[0], 123);
        assert_eq!(player.active_data().buyback_timestamp[0], 456);
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_BUYBACK_PRICE_FIRST_BIT)
        );
        assert!(
            player
                .active_player_data_changes_mask()
                .is_set(ACTIVE_PLAYER_DATA_BUYBACK_TIMESTAMP_FIRST_BIT)
        );

        let second_slot = player.add_item_to_buyback_slot(second, 200, 500);
        assert_eq!(second_slot, BUYBACK_SLOT_START + 1);
        assert_eq!(
            player.remove_item_from_buyback_slot(first_slot),
            Some(first)
        );
        assert_eq!(player.get_item_from_buyback_slot(first_slot), None);
        assert_eq!(
            player.active_data().inv_slots[first_slot as usize],
            ObjectGuid::EMPTY
        );
        assert_eq!(player.active_data().buyback_price[0], 0);
        assert_eq!(player.active_data().buyback_timestamp[0], 0);
    }

    #[test]
    fn add_item_to_buyback_slot_object_matches_cpp_price_time_and_replacement() {
        let mut player = Player::new(None, false);
        let mut overwritten = item_with_guid_entry(1100, 7000);
        overwritten.set_count(3);
        overwritten.force_state(ItemUpdateState::Unchanged);
        let old_proto = ItemStorageTemplate {
            sell_price: 11,
            ..ItemStorageTemplate::regular_item(7000, 20)
        };

        let old_slot = player
            .add_item_to_buyback_slot_object(&overwritten, Some(&old_proto), 2000, 1000, None)
            .unwrap();
        assert_eq!(old_slot, BUYBACK_SLOT_START);
        assert_eq!(
            player.get_item_from_buyback_slot(old_slot),
            Some(overwritten.object().guid())
        );
        assert_eq!(player.active_data().buyback_price[0], 33);
        assert_eq!(player.active_data().buyback_timestamp[0], 109000);

        player.set_buyback_timestamp(0, 50);
        for slot in BUYBACK_SLOT_START + 1..BUYBACK_SLOT_END {
            let guid = ObjectGuid::create_item(1, 2000 + slot as i64);
            player.add_item_to_buyback_slot(guid, 1, 100 + slot as i64);
        }

        overwritten.object_mut().add_to_world();
        let mut replacement = item_with_guid_entry(1101, 7001);
        replacement.set_count(4);
        let replacement_proto = ItemStorageTemplate {
            sell_price: 9,
            ..ItemStorageTemplate::regular_item(7001, 20)
        };

        let replaced_slot = player
            .add_item_to_buyback_slot_object(
                &replacement,
                Some(&replacement_proto),
                5000,
                3000,
                Some(&mut overwritten),
            )
            .unwrap();

        assert_eq!(replaced_slot, old_slot);
        assert!(!overwritten.object().is_in_world());
        assert_eq!(overwritten.update_state(), ItemUpdateState::Removed);
        assert_eq!(
            player.get_item_from_buyback_slot(replaced_slot),
            Some(replacement.object().guid())
        );
        assert_eq!(player.active_data().buyback_price[0], 36);
        assert_eq!(player.active_data().buyback_timestamp[0], 110000);
        assert_eq!(
            player.inventory().current_buyback_slot,
            BUYBACK_SLOT_START + 1
        );
    }

    #[test]
    fn remove_item_from_buyback_slot_object_matches_cpp_item_side_effects() {
        let mut player = Player::new(None, false);
        let mut item = item_with_guid_entry(1010, 6948);
        item.force_state(ItemUpdateState::Unchanged);
        item.object_mut().add_to_world();

        let slot = player.add_item_to_buyback_slot(item.object().guid(), 123, 456);
        assert_eq!(
            player
                .remove_item_from_buyback_slot_object(slot, Some(&mut item), true)
                .unwrap(),
            Some(item.object().guid())
        );

        assert!(!item.object().is_in_world());
        assert_eq!(item.update_state(), ItemUpdateState::Removed);
        assert_eq!(player.get_item_from_buyback_slot(slot), None);
        assert_eq!(
            player.active_data().inv_slots[slot as usize],
            ObjectGuid::EMPTY
        );
        assert_eq!(player.active_data().buyback_price[0], 0);
        assert_eq!(player.active_data().buyback_timestamp[0], 0);

        let mut keep_state_item = item_with_guid_entry(1011, 6949);
        keep_state_item.force_state(ItemUpdateState::Unchanged);
        keep_state_item.object_mut().add_to_world();
        let keep_slot = player.add_item_to_buyback_slot(keep_state_item.object().guid(), 200, 500);

        player
            .remove_item_from_buyback_slot_object(keep_slot, Some(&mut keep_state_item), false)
            .unwrap();
        assert!(!keep_state_item.object().is_in_world());
        assert_eq!(keep_state_item.update_state(), ItemUpdateState::Unchanged);
    }

    #[test]
    fn remove_item_from_buyback_slot_object_rejects_mismatched_item_ref() {
        let mut player = Player::new(None, false);
        let expected = ObjectGuid::create_item(1, 1020);
        let mut actual = item_with_guid_entry(1021, 6948);

        let slot = player.add_item_to_buyback_slot(expected, 123, 456);
        assert_eq!(
            player.remove_item_from_buyback_slot_object(slot, Some(&mut actual), true),
            Err(PlayerStorageError::MismatchedItemGuid {
                slot,
                expected,
                actual: actual.object().guid(),
            })
        );
        assert_eq!(player.get_item_from_buyback_slot(slot), Some(expected));
        assert_eq!(player.active_data().buyback_price[0], 123);
        assert_eq!(player.active_data().buyback_timestamp[0], 456);
    }

    #[test]
    fn soulbound_tradeable_item_set_matches_cpp_add_remove_and_update() {
        let mut player = Player::new(None, false);
        let mut keep = item_with_guid_entry(1200, 7000);
        keep.set_owner_guid(player.guid());
        let mut expired = item_with_guid_entry(1201, 7001);
        expired.set_owner_guid(player.guid());
        let mut wrong_owner = item_with_guid_entry(1202, 7002);
        wrong_owner.set_owner_guid(ObjectGuid::create_player(1, 99));
        let missing = item_with_guid_entry(1203, 7003);
        let removed_directly = item_with_guid_entry(1204, 7004);

        player.add_tradeable_item(&keep);
        player.add_tradeable_item(&expired);
        player.add_tradeable_item(&wrong_owner);
        player.add_tradeable_item(&missing);
        player.add_tradeable_item(&removed_directly);
        player.remove_tradeable_item(&removed_directly);

        assert!(
            player
                .soulbound_tradeable_items()
                .contains(&keep.object().guid())
        );
        assert!(
            !player
                .soulbound_tradeable_items()
                .contains(&removed_directly.object().guid())
        );

        let removed = player.update_soulbound_trade_items(&[
            SoulboundTradeableItemRef::new(keep.object().guid(), keep.owner_guid(), false),
            SoulboundTradeableItemRef::new(expired.object().guid(), expired.owner_guid(), true),
            SoulboundTradeableItemRef::new(
                wrong_owner.object().guid(),
                wrong_owner.owner_guid(),
                false,
            ),
        ]);

        assert!(
            player
                .soulbound_tradeable_items()
                .contains(&keep.object().guid())
        );
        assert_eq!(player.soulbound_tradeable_items().len(), 1);
        assert!(removed.contains(&expired.object().guid()));
        assert!(removed.contains(&wrong_owner.object().guid()));
        assert!(removed.contains(&missing.object().guid()));
        assert!(!removed.contains(&removed_directly.object().guid()));
    }

    #[test]
    fn titan_grip_and_equipped_weapon_helpers_match_cpp_representable_rules() {
        let mut player = Player::new(None, false);
        let two_hand = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon2Hand,
            class_id: ItemClass::Weapon,
            ..ItemStorageTemplate::regular_item(2000, 1)
        };
        let one_hand = ItemStorageTemplate {
            inventory_type: InventoryType::Weapon,
            class_id: ItemClass::Weapon,
            ..ItemStorageTemplate::regular_item(2001, 1)
        };
        let ranged = ItemStorageTemplate {
            inventory_type: InventoryType::Ranged,
            class_id: ItemClass::Weapon,
            ..ItemStorageTemplate::regular_item(2002, 1)
        };
        let ranged_right_non_wand = ItemStorageTemplate {
            inventory_type: InventoryType::RangedRight,
            class_id: ItemClass::Weapon,
            subclass_id: ItemSubClassWeapon::Bow as u32,
            ..ItemStorageTemplate::regular_item(2003, 1)
        };
        let wand = ItemStorageTemplate {
            inventory_type: InventoryType::RangedRight,
            class_id: ItemClass::Weapon,
            subclass_id: ItemSubClassWeapon::Wand as u32,
            ..ItemStorageTemplate::regular_item(2004, 1)
        };

        assert!(Player::is_use_equipped_weapon(false, false, true));
        assert!(!Player::is_use_equipped_weapon(true, false, true));
        assert!(!Player::is_use_equipped_weapon(false, true, false));

        assert!(!player.can_titan_grip());
        assert_eq!(player.titan_grip_penalty_spell_id(), 0);
        assert!(player.is_two_hand_used_template(Some(&two_hand)));
        assert!(player.is_two_hand_used_template(Some(&ranged)));
        assert!(player.is_two_hand_used_template(Some(&ranged_right_non_wand)));
        assert!(!player.is_two_hand_used_template(Some(&wand)));
        assert!(!player.is_two_hand_used_template(None));

        player.set_can_titan_grip(true, 49152);
        player.set_can_titan_grip(true, 99999);
        assert!(player.can_titan_grip());
        assert_eq!(player.titan_grip_penalty_spell_id(), 49152);
        assert!(!player.is_two_hand_used_template(Some(&two_hand)));

        assert!(Player::is_using_two_handed_weapon_in_one_hand_template(
            Some(&one_hand),
            Some(&two_hand),
        ));
        assert!(Player::is_using_two_handed_weapon_in_one_hand_template(
            Some(&two_hand),
            Some(&one_hand),
        ));
        assert!(!Player::is_using_two_handed_weapon_in_one_hand_template(
            Some(&two_hand),
            None,
        ));
        assert!(!Player::is_using_two_handed_weapon_in_one_hand_template(
            Some(&one_hand),
            Some(&one_hand),
        ));

        assert_eq!(
            player.check_titan_grip_penalty_action(true, false),
            TitanGripPenaltyAction::Cast(49152)
        );
        assert_eq!(
            player.check_titan_grip_penalty_action(true, true),
            TitanGripPenaltyAction::None
        );
        assert_eq!(
            player.check_titan_grip_penalty_action(false, true),
            TitanGripPenaltyAction::Remove(49152)
        );

        player.set_can_titan_grip(false, 0);
        assert_eq!(
            player.check_titan_grip_penalty_action(true, false),
            TitanGripPenaltyAction::None
        );
    }

    #[test]
    fn swap_item_preflight_matches_cpp_no_source_child_and_dead_order() {
        let player = Player::new(None, false);
        let src = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);
        let dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let parent = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_HEAD);

        assert_eq!(
            player.swap_item_preflight_plan(src, dst, true, None, None),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::NoSource,
                src_unequip_swap: None,
                dst_unequip_swap: None,
            }
        );

        let mut child_source = SwapItemPreflightItem::regular();
        child_source.is_child = true;
        child_source.parent_pos = Some(parent);
        assert_eq!(
            player.swap_item_preflight_plan(src, dst, false, Some(child_source), None),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::ChildRedirect {
                    first_src: dst,
                    first_dst: src,
                    second_src: parent,
                    second_dst: dst,
                },
                src_unequip_swap: None,
                dst_unequip_swap: None,
            }
        );

        let mut child_dst = SwapItemPreflightItem::regular();
        child_dst.is_child = true;
        child_dst.parent_pos = Some(parent);
        assert_eq!(
            player.swap_item_preflight_plan(
                dst,
                src,
                true,
                Some(SwapItemPreflightItem::regular()),
                Some(child_dst)
            ),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::ChildRedirect {
                    first_src: dst,
                    first_dst: src,
                    second_src: parent,
                    second_dst: dst,
                },
                src_unequip_swap: None,
                dst_unequip_swap: None,
            }
        );

        let mut blocked_source = SwapItemPreflightItem::regular();
        blocked_source.can_unequip_result = InventoryResult::CantEquipEver;
        assert_eq!(
            player.swap_item_preflight_plan(src, dst, false, Some(blocked_source), None),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::PlayerDead),
                src_unequip_swap: None,
                dst_unequip_swap: None,
            }
        );
    }

    #[test]
    fn swap_item_preflight_matches_cpp_unequip_and_bag_self_guards() {
        let player = Player::new(None, false);
        let equipped_src = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);
        let inventory_dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let source = SwapItemPreflightItem::regular();

        assert_eq!(
            player.swap_item_preflight_plan(equipped_src, inventory_dst, true, Some(source), None),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Continue,
                src_unequip_swap: Some(true),
                dst_unequip_swap: None,
            }
        );

        let mut blocked_source = SwapItemPreflightItem::regular();
        blocked_source.can_unequip_result = InventoryResult::ClientLockedOut;
        assert_eq!(
            player.swap_item_preflight_plan(
                equipped_src,
                inventory_dst,
                true,
                Some(blocked_source),
                None
            ),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::ClientLockedOut),
                src_unequip_swap: Some(true),
                dst_unequip_swap: None,
            }
        );

        let bag_slot = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START);
        let inside_same_bag = make_item_pos(INVENTORY_SLOT_BAG_START, 0);
        assert_eq!(
            player.swap_item_preflight_plan(
                bag_slot,
                inside_same_bag,
                true,
                Some(SwapItemPreflightItem::bag(false)),
                None,
            ),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::BagInBag),
                src_unequip_swap: Some(false),
                dst_unequip_swap: None,
            }
        );
        assert_eq!(
            player.swap_item_preflight_plan(
                inside_same_bag,
                bag_slot,
                true,
                Some(SwapItemPreflightItem::regular()),
                Some(SwapItemPreflightItem::bag(false)),
            ),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::CantSwap),
                src_unequip_swap: None,
                dst_unequip_swap: None,
            }
        );

        let mut blocked_dst = SwapItemPreflightItem::bag(true);
        blocked_dst.can_unequip_result = InventoryResult::CantEquipEver;
        let other_bag_slot = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START + 1);
        assert_eq!(
            player.swap_item_preflight_plan(
                inventory_dst,
                other_bag_slot,
                true,
                Some(SwapItemPreflightItem::bag(true)),
                Some(blocked_dst),
            ),
            SwapItemPreflightPlan {
                result: SwapItemPreflightResult::Error(InventoryResult::CantEquipEver),
                src_unequip_swap: None,
                dst_unequip_swap: Some(true),
            }
        );
    }

    #[test]
    fn swap_item_empty_destination_plan_matches_cpp_move_case() {
        let player = Player::new(None, false);
        let inventory_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let inventory_dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1);
        let bank_src = make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START);
        let bank_dst = make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START + 1);
        let equip_dst = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);
        let equip_dest = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);

        assert_eq!(
            player.swap_item_empty_destination_plan(
                inventory_src,
                inventory_dst,
                true,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::OccupiedDestination,
            }
        );

        assert_eq!(
            player.swap_item_empty_destination_plan(
                bank_src,
                inventory_dst,
                false,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::MoveToInventory {
                    quest_added_from_bank: true,
                },
            }
        );

        assert_eq!(
            player.swap_item_empty_destination_plan(
                inventory_src,
                inventory_dst,
                false,
                InventoryResult::InvFull,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::Error(InventoryResult::InvFull),
            }
        );

        assert_eq!(
            player.swap_item_empty_destination_plan(
                inventory_src,
                bank_dst,
                false,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::MoveToBank {
                    quest_removed: true,
                },
            }
        );

        assert_eq!(
            player.swap_item_empty_destination_plan(
                inventory_src,
                equip_dst,
                false,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::Equip {
                    dest: equip_dest,
                    auto_unequip_offhand: true,
                },
            }
        );

        assert_eq!(
            player.swap_item_empty_destination_plan(
                inventory_src,
                make_item_pos(BUYBACK_SLOT_START, 0),
                false,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest,
            ),
            SwapItemEmptyDestinationPlan {
                result: SwapItemEmptyDestinationResult::InvalidDestinationNoop,
            }
        );
    }

    #[test]
    fn swap_item_merge_fill_plan_matches_cpp_occupied_non_bag_case() {
        let player = Player::new(None, false);
        let inventory_dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let bank_dst = make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START);
        let equip_dst = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);
        let equip_dest = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);

        assert_eq!(
            player.swap_item_merge_fill_plan(
                inventory_dst,
                true,
                false,
                3,
                4,
                20,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::ContinueToRealSwap,
                send_refund_info: false,
            }
        );

        assert_eq!(
            player.swap_item_merge_fill_plan(
                inventory_dst,
                false,
                false,
                3,
                4,
                20,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::ContinueToRealSwap,
                send_refund_info: false,
            }
        );

        assert_eq!(
            player.swap_item_merge_fill_plan(
                inventory_dst,
                false,
                false,
                3,
                4,
                20,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::MoveMergedStackToInventory,
                send_refund_info: true,
            }
        );

        assert_eq!(
            player.swap_item_merge_fill_plan(
                bank_dst,
                false,
                false,
                3,
                4,
                20,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::MoveMergedStackToBank,
                send_refund_info: true,
            }
        );

        assert_eq!(
            player.swap_item_merge_fill_plan(
                equip_dst,
                false,
                false,
                3,
                4,
                20,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::EquipMergedStack {
                    dest: equip_dest,
                    auto_unequip_offhand: true,
                },
                send_refund_info: true,
            }
        );

        assert_eq!(
            player.swap_item_merge_fill_plan(
                inventory_dst,
                false,
                false,
                15,
                12,
                20,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest,
                true,
            ),
            SwapItemMergeFillPlan {
                result: SwapItemMergeFillResult::PartialFill {
                    source_remaining_count: 7,
                    destination_count: 20,
                    send_updates: true,
                },
                send_refund_info: true,
            }
        );
    }

    #[test]
    fn swap_item_real_swap_validation_plan_matches_cpp_bidirectional_checks() {
        let player = Player::new(None, false);
        let inventory_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let bank_dst = make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START);
        let equip_src = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);
        let equip_dst = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_LEGS);
        let equip_dest = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_LEGS);
        let equip_dest2 = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);

        assert_eq!(
            player.swap_item_real_swap_validation_plan(
                inventory_src,
                bank_dst,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                equip_dest,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest2,
                InventoryResult::Ok,
            ),
            SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Continue {
                    source_target: SwapItemRealSwapTarget::Bank,
                    destination_target: SwapItemRealSwapTarget::Inventory,
                },
            }
        );

        assert_eq!(
            player.swap_item_real_swap_validation_plan(
                inventory_src,
                bank_dst,
                InventoryResult::CantSwap,
                InventoryResult::InvFull,
                InventoryResult::CantSwap,
                equip_dest,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest2,
                InventoryResult::Ok,
            ),
            SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Error {
                    result: InventoryResult::InvFull,
                    subject: SwapItemRealSwapValidationSubject::Source,
                },
            }
        );

        assert_eq!(
            player.swap_item_real_swap_validation_plan(
                inventory_src,
                bank_dst,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                InventoryResult::CantSwap,
                equip_dest,
                InventoryResult::Ok,
                InventoryResult::ClientLockedOut,
                InventoryResult::Ok,
                InventoryResult::Ok,
                equip_dest2,
                InventoryResult::Ok,
            ),
            SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Error {
                    result: InventoryResult::ClientLockedOut,
                    subject: SwapItemRealSwapValidationSubject::Destination,
                },
            }
        );

        assert_eq!(
            player.swap_item_real_swap_validation_plan(
                equip_src,
                equip_dst,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                equip_dest,
                InventoryResult::DestroyNonemptyBag,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::Ok,
                equip_dest2,
                InventoryResult::Ok,
            ),
            SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Error {
                    result: InventoryResult::DestroyNonemptyBag,
                    subject: SwapItemRealSwapValidationSubject::Source,
                },
            }
        );

        assert_eq!(
            player.swap_item_real_swap_validation_plan(
                make_item_pos(BUYBACK_SLOT_START, 0),
                make_item_pos(BUYBACK_SLOT_START + 1, 0),
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                InventoryResult::CantSwap,
                equip_dest2,
                InventoryResult::CantSwap,
            ),
            SwapItemRealSwapValidationPlan {
                result: SwapItemRealSwapValidationResult::Continue {
                    source_target: SwapItemRealSwapTarget::None,
                    destination_target: SwapItemRealSwapTarget::None,
                },
            }
        );
    }

    #[test]
    fn swap_item_bag_exchange_plan_matches_cpp_empty_bag_exchange() {
        let player = Player::new(None, false);
        let inventory_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let inventory_dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START + 1);
        let bag_slot_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START);
        let full_items = [
            SwapBagItemRef::new(0, true),
            SwapBagItemRef::new(2, true),
            SwapBagItemRef::new(4, true),
        ];
        let full_bag = SwapBagRef::new(false, 5, &full_items);
        let empty_bag = SwapBagRef::new(true, 4, &[]);

        assert_eq!(
            player.swap_item_bag_exchange_plan(inventory_src, inventory_dst, None, Some(full_bag)),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Continue,
            }
        );

        assert_eq!(
            player.swap_item_bag_exchange_plan(
                inventory_src,
                inventory_dst,
                Some(empty_bag),
                Some(full_bag),
            ),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Exchange {
                    empty_bag_is_source: true,
                    moves: vec![
                        SwapBagItemMove {
                            from_slot: 0,
                            to_slot: 0,
                        },
                        SwapBagItemMove {
                            from_slot: 2,
                            to_slot: 1,
                        },
                        SwapBagItemMove {
                            from_slot: 4,
                            to_slot: 2,
                        },
                    ],
                },
            }
        );

        assert_eq!(
            player.swap_item_bag_exchange_plan(
                inventory_src,
                inventory_dst,
                Some(full_bag),
                Some(empty_bag),
            ),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Exchange {
                    empty_bag_is_source: false,
                    moves: vec![
                        SwapBagItemMove {
                            from_slot: 0,
                            to_slot: 0,
                        },
                        SwapBagItemMove {
                            from_slot: 2,
                            to_slot: 1,
                        },
                        SwapBagItemMove {
                            from_slot: 4,
                            to_slot: 2,
                        },
                    ],
                },
            }
        );

        assert_eq!(
            player.swap_item_bag_exchange_plan(
                bag_slot_src,
                inventory_dst,
                Some(empty_bag),
                Some(full_bag),
            ),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Continue,
            }
        );

        let blocked_items = [SwapBagItemRef::new(0, true), SwapBagItemRef::new(1, false)];
        let blocked_bag = SwapBagRef::new(false, 2, &blocked_items);
        assert_eq!(
            player.swap_item_bag_exchange_plan(
                inventory_src,
                inventory_dst,
                Some(empty_bag),
                Some(blocked_bag),
            ),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Error(InventoryResult::BagInBag),
            }
        );

        let small_empty_bag = SwapBagRef::new(true, 2, &[]);
        assert_eq!(
            player.swap_item_bag_exchange_plan(
                inventory_src,
                inventory_dst,
                Some(small_empty_bag),
                Some(full_bag),
            ),
            SwapItemBagExchangePlan {
                result: SwapItemBagExchangeResult::Error(InventoryResult::CantSwap),
            }
        );
    }

    #[test]
    fn swap_item_real_swap_execution_plan_matches_cpp_final_actions() {
        let player = Player::new(None, false);
        let inventory_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_ITEM_START);
        let equip_dst = make_item_pos(INVENTORY_SLOT_BAG_0, EQUIPMENT_SLOT_CHEST);

        assert_eq!(
            player.swap_item_real_swap_execution_plan(
                inventory_src,
                equip_dst,
                SwapItemRealSwapTarget::Equip { dest: equip_dst },
                SwapItemRealSwapTarget::Inventory,
                false,
                false,
                false,
            ),
            SwapItemRealSwapExecutionPlan {
                remove_destination_update: false,
                remove_source_update: false,
                source_target: SwapItemRealSwapTarget::Equip { dest: equip_dst },
                destination_target: SwapItemRealSwapTarget::Inventory,
                apply_item_dependent_auras: true,
                release_loot: false,
                auto_unequip_offhand: true,
            }
        );

        let bag_src = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START);
        let bank_dst = make_item_pos(INVENTORY_SLOT_BAG_0, BANK_SLOT_ITEM_START);
        assert!(
            player
                .swap_item_real_swap_execution_plan(
                    bag_src,
                    bank_dst,
                    SwapItemRealSwapTarget::Bank,
                    SwapItemRealSwapTarget::Inventory,
                    true,
                    true,
                    false,
                )
                .release_loot
        );

        let bag_dst = make_item_pos(INVENTORY_SLOT_BAG_0, INVENTORY_SLOT_BAG_START + 1);
        assert!(
            player
                .swap_item_real_swap_execution_plan(
                    bank_dst,
                    bag_dst,
                    SwapItemRealSwapTarget::Inventory,
                    SwapItemRealSwapTarget::Bank,
                    true,
                    false,
                    true,
                )
                .release_loot
        );
        assert!(
            !player
                .swap_item_real_swap_execution_plan(
                    bank_dst,
                    bag_dst,
                    SwapItemRealSwapTarget::Inventory,
                    SwapItemRealSwapTarget::Bank,
                    false,
                    false,
                    true,
                )
                .release_loot
        );
    }

    #[test]
    fn swap_item_orchestration_plan_matches_cpp_branch_order() {
        let player = Player::new(None, false);
        let continue_preflight = SwapItemPreflightPlan {
            result: SwapItemPreflightResult::Continue,
            src_unequip_swap: None,
            dst_unequip_swap: None,
        };
        let occupied_destination = SwapItemEmptyDestinationPlan {
            result: SwapItemEmptyDestinationResult::OccupiedDestination,
        };
        let continue_merge = SwapItemMergeFillPlan {
            result: SwapItemMergeFillResult::ContinueToRealSwap,
            send_refund_info: false,
        };
        let inventory_bank_validation = SwapItemRealSwapValidationPlan {
            result: SwapItemRealSwapValidationResult::Continue {
                source_target: SwapItemRealSwapTarget::Inventory,
                destination_target: SwapItemRealSwapTarget::Bank,
            },
        };
        let no_bag_exchange = SwapItemBagExchangePlan {
            result: SwapItemBagExchangeResult::Continue,
        };
        let execution = SwapItemRealSwapExecutionPlan {
            remove_destination_update: false,
            remove_source_update: false,
            source_target: SwapItemRealSwapTarget::Inventory,
            destination_target: SwapItemRealSwapTarget::Bank,
            apply_item_dependent_auras: false,
            release_loot: false,
            auto_unequip_offhand: true,
        };

        assert_eq!(
            player.swap_item_orchestration_plan(
                SwapItemPreflightPlan {
                    result: SwapItemPreflightResult::Error(InventoryResult::PlayerDead),
                    src_unequip_swap: None,
                    dst_unequip_swap: None,
                },
                None,
                None,
                None,
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::Error {
                    result: InventoryResult::PlayerDead,
                    item_order: SwapItemErrorItemOrder::SourceDestination,
                },
            }
        );

        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(SwapItemEmptyDestinationPlan {
                    result: SwapItemEmptyDestinationResult::Error(InventoryResult::InvFull),
                }),
                None,
                None,
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::Error {
                    result: InventoryResult::InvFull,
                    item_order: SwapItemErrorItemOrder::SourceOnly,
                },
            }
        );

        let move_to_bank = SwapItemEmptyDestinationPlan {
            result: SwapItemEmptyDestinationResult::MoveToBank {
                quest_removed: true,
            },
        };
        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(move_to_bank),
                None,
                None,
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::EmptyDestination(move_to_bank),
            }
        );

        let partial_fill = SwapItemMergeFillPlan {
            result: SwapItemMergeFillResult::PartialFill {
                source_remaining_count: 2,
                destination_count: 20,
                send_updates: true,
            },
            send_refund_info: true,
        };
        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                Some(partial_fill),
                None,
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MergeFill(partial_fill),
            }
        );

        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                Some(continue_merge),
                Some(SwapItemRealSwapValidationPlan {
                    result: SwapItemRealSwapValidationResult::Error {
                        result: InventoryResult::CantEquipEver,
                        subject: SwapItemRealSwapValidationSubject::Destination,
                    },
                }),
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::Error {
                    result: InventoryResult::CantEquipEver,
                    item_order: SwapItemErrorItemOrder::DestinationSource,
                },
            }
        );

        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                Some(continue_merge),
                Some(inventory_bank_validation),
                Some(SwapItemBagExchangePlan {
                    result: SwapItemBagExchangeResult::Error(InventoryResult::BagInBag),
                }),
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::Error {
                    result: InventoryResult::BagInBag,
                    item_order: SwapItemErrorItemOrder::SourceDestination,
                },
            }
        );

        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                Some(continue_merge),
                Some(inventory_bank_validation),
                Some(no_bag_exchange.clone()),
                Some(execution),
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::RealSwap {
                    bag_exchange: no_bag_exchange,
                    execution,
                },
            }
        );
    }

    #[test]
    fn swap_item_orchestration_plan_keeps_phase_gaps_visible() {
        let player = Player::new(None, false);
        let continue_preflight = SwapItemPreflightPlan {
            result: SwapItemPreflightResult::Continue,
            src_unequip_swap: None,
            dst_unequip_swap: None,
        };

        assert_eq!(
            player.swap_item_orchestration_plan(continue_preflight, None, None, None, None, None),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(
                    SwapItemMissingPhase::EmptyDestination,
                ),
            }
        );

        let occupied_destination = SwapItemEmptyDestinationPlan {
            result: SwapItemEmptyDestinationResult::OccupiedDestination,
        };
        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                None,
                None,
                None,
                None,
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::MissingPhase(SwapItemMissingPhase::MergeFill),
            }
        );

        let continue_merge = SwapItemMergeFillPlan {
            result: SwapItemMergeFillResult::ContinueToRealSwap,
            send_refund_info: false,
        };
        let validation = SwapItemRealSwapValidationPlan {
            result: SwapItemRealSwapValidationResult::Continue {
                source_target: SwapItemRealSwapTarget::Inventory,
                destination_target: SwapItemRealSwapTarget::Bank,
            },
        };
        let mismatched_execution = SwapItemRealSwapExecutionPlan {
            remove_destination_update: false,
            remove_source_update: false,
            source_target: SwapItemRealSwapTarget::Bank,
            destination_target: SwapItemRealSwapTarget::Inventory,
            apply_item_dependent_auras: false,
            release_loot: false,
            auto_unequip_offhand: true,
        };

        assert_eq!(
            player.swap_item_orchestration_plan(
                continue_preflight,
                Some(occupied_destination),
                Some(continue_merge),
                Some(validation),
                Some(SwapItemBagExchangePlan {
                    result: SwapItemBagExchangeResult::Continue,
                }),
                Some(mismatched_execution),
            ),
            SwapItemOrchestrationPlan {
                result: SwapItemOrchestrationResult::InconsistentRealSwapTargets {
                    validation_source_target: SwapItemRealSwapTarget::Inventory,
                    validation_destination_target: SwapItemRealSwapTarget::Bank,
                    execution_source_target: SwapItemRealSwapTarget::Bank,
                    execution_destination_target: SwapItemRealSwapTarget::Inventory,
                },
            }
        );
    }
}
