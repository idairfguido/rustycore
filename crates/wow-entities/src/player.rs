use wow_constants::{Gender, PowerType, TypeId, TypeMask};
use wow_core::ObjectGuid;

use crate::{
    ObjectDataUpdate, Unit, UnitDataUpdate, UpdateMask,
    update_fields::{
        ACTIVE_PLAYER_DATA_BITS, PLAYER_DATA_BITS, TYPEID_ACTIVE_PLAYER, TYPEID_PLAYER,
    },
};

pub const MAX_MONEY_AMOUNT: u64 = 99_999_999_999;
pub const TEAM_OTHER: u8 = 0;

pub const PLAYER_DATA_PARENT_BIT: usize = 0;
pub const PLAYER_DATA_LOOT_TARGET_GUID_BIT: usize = 6;
pub const PLAYER_DATA_FLAGS_BIT: usize = 7;
pub const PLAYER_DATA_FLAGS_EX_BIT: usize = 8;
pub const PLAYER_DATA_NUM_BANK_SLOTS_BIT: usize = 12;
pub const PLAYER_DATA_NATIVE_SEX_BIT: usize = 13;
pub const PLAYER_DATA_CURRENT_SPEC_ID_BIT: usize = 24;

pub const ACTIVE_PLAYER_DATA_PARENT_BIT: usize = 0;
pub const ACTIVE_PLAYER_DATA_COINAGE_BIT: usize = 28;
pub const ACTIVE_PLAYER_DATA_XP_BIT: usize = 29;
pub const ACTIVE_PLAYER_DATA_NEXT_LEVEL_XP_BIT: usize = 30;
pub const ACTIVE_PLAYER_DATA_CHARACTER_POINTS_BIT: usize = 33;
pub const ACTIVE_PLAYER_DATA_NUM_BACKPACK_SLOTS_BIT: usize = 104;
pub const ACTIVE_PLAYER_DATA_INV_SLOTS_PARENT_BIT: usize = 124;
pub const ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT: usize = 125;
pub const PLAYER_SLOT_END: usize = 141;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerDataValues {
    pub loot_target_guid: ObjectGuid,
    pub player_flags: u32,
    pub player_flags_ex: u32,
    pub num_bank_slots: u8,
    pub native_sex: u8,
    pub current_spec_id: u32,
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!player.player_data_changes_mask().is_any_set());
        assert!(!player.active_player_data_changes_mask().is_any_set());
    }

    #[test]
    fn can_filter_whispers_permission_keeps_constructor_accept_flag_false() {
        let player = Player::new(None, true);
        assert!(!player.accept_whispers());
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
}
