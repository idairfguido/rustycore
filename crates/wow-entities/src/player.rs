use bitflags::bitflags;
use wow_constants::{
    BagFamilyMask, Gender, InventoryResult, ItemFieldFlags, ItemUpdateState, PowerType, TypeId,
    TypeMask,
};
use wow_core::ObjectGuid;

use crate::{
    Bag, EQUIPMENT_SLOT_END, INVENTORY_SLOT_BAG_0, Item, ItemStorageTemplate, MAX_BAG_SIZE,
    NULL_SLOT, ObjectDataUpdate, PROFESSION_SLOT_END, PROFESSION_SLOT_START, Unit, UnitDataUpdate,
    UpdateMask, item_can_go_into_bag,
    update_fields::{
        ACTIVE_PLAYER_DATA_BITS, PLAYER_DATA_BITS, TYPEID_ACTIVE_PLAYER, TYPEID_PLAYER,
    },
};

pub const MAX_MONEY_AMOUNT: u64 = 99_999_999_999;
pub const TEAM_OTHER: u8 = 0;
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

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{
        BagFamilyMask, InventoryResult, ItemBondingType, ItemClass, ItemContext, ItemFieldFlags,
        ItemSubClassContainer,
    };

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
}
