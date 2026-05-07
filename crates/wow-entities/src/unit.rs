use wow_constants::{DeathState, Gender, PowerType, TypeId, TypeMask, WeaponAttackType};
use wow_core::ObjectGuid;

use crate::{
    ObjectDataUpdate, UpdateMask, WorldObject,
    update_fields::{TYPEID_UNIT, UNIT_DATA_BITS},
};

pub const MAX_MOVE_TYPE: usize = 9;
pub const MAX_ATTACK: usize = 3;
pub const MAX_POWERS: usize = 26;
pub const MAX_POWERS_PER_CLASS: usize = 10;
pub const BASE_MINDAMAGE: f32 = 1.0;
pub const BASE_MAXDAMAGE: f32 = 2.0;
pub const DEFAULT_PLAYER_DISPLAY_SCALE: f32 = 1.0;

pub const UNIT_DATA_PARENT_BIT: usize = 0;
pub const UNIT_DATA_HEALTH_BIT: usize = 5;
pub const UNIT_DATA_MAX_HEALTH_BIT: usize = 6;
pub const UNIT_DATA_DISPLAY_ID_BIT: usize = 7;
pub const UNIT_DATA_DISPLAY_POWER_BIT: usize = 28;
pub const UNIT_DATA_LEVEL_BIT: usize = 30;
pub const UNIT_DATA_FACTION_TEMPLATE_BIT: usize = 40;
pub const UNIT_DATA_FLAGS_BIT: usize = 41;
pub const UNIT_DATA_FLAGS2_BIT: usize = 42;
pub const UNIT_DATA_FLAGS3_BIT: usize = 43;
pub const UNIT_DATA_BOUNDING_RADIUS_BIT: usize = 46;
pub const UNIT_DATA_COMBAT_REACH_BIT: usize = 47;
pub const UNIT_DATA_DISPLAY_SCALE_BIT: usize = 48;
pub const UNIT_DATA_NATIVE_DISPLAY_ID_BIT: usize = 49;
pub const UNIT_DATA_NATIVE_DISPLAY_SCALE_BIT: usize = 50;
pub const UNIT_DATA_TARGET_BIT: usize = 19;
pub const UNIT_DATA_RACE_BIT: usize = 24;
pub const UNIT_DATA_CLASS_ID_BIT: usize = 25;
pub const UNIT_DATA_PLAYER_CLASS_ID_BIT: usize = 26;
pub const UNIT_DATA_SEX_BIT: usize = 27;
pub const UNIT_DATA_POWER_PARENT_BIT: usize = 116;
pub const UNIT_DATA_POWER_FIRST_BIT: usize = 137;
pub const UNIT_DATA_MAX_POWER_FIRST_BIT: usize = 147;

pub const BASE_MOVE_SPEED: [f32; MAX_MOVE_TYPE] =
    [2.5, 7.0, 4.5, 4.722222, 2.5, 3.141594, 7.0, 4.5, 3.14];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitDataValues {
    pub health: u64,
    pub max_health: u64,
    pub display_id: i32,
    pub target: ObjectGuid,
    pub race: u8,
    pub class_id: u8,
    pub player_class_id: u8,
    pub sex: u8,
    pub display_power: u8,
    pub level: i32,
    pub faction_template: i32,
    pub flags: u32,
    pub flags2: u32,
    pub flags3: u32,
    pub bounding_radius: f32,
    pub combat_reach: f32,
    pub display_scale: f32,
    pub native_display_id: i32,
    pub native_display_scale: f32,
    pub power: [i32; MAX_POWERS_PER_CLASS],
    pub max_power: [i32; MAX_POWERS_PER_CLASS],
}

impl Default for UnitDataValues {
    fn default() -> Self {
        Self {
            health: 0,
            max_health: 0,
            display_id: 0,
            target: ObjectGuid::EMPTY,
            race: 0,
            class_id: 0,
            player_class_id: 0,
            sex: Gender::Male as u8,
            display_power: PowerType::Mana as u8,
            level: 0,
            faction_template: 0,
            flags: 0,
            flags2: 0,
            flags3: 0,
            bounding_radius: 0.0,
            combat_reach: 0.0,
            display_scale: 0.0,
            native_display_id: 0,
            native_display_scale: 0.0,
            power: [0; MAX_POWERS_PER_CLASS],
            max_power: [0; MAX_POWERS_PER_CLASS],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitDataUpdate {
    pub mask: UpdateMask,
    pub values: UnitDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub unit_data: Option<UnitDataUpdate>,
}

impl UnitValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    world: WorldObject,
    data: UnitDataValues,
    unit_data_changes: UpdateMask,
    death_state: DeathState,
    unit_state: u32,
    attacking: Option<ObjectGuid>,
    base_attack_speed: [u32; MAX_ATTACK],
    mod_attack_speed_pct: [f32; MAX_ATTACK],
    weapon_damage: [[f32; 2]; MAX_ATTACK],
    speed_rate: [f32; MAX_MOVE_TYPE],
    power_index: [Option<usize>; MAX_POWERS],
}

impl Unit {
    pub fn new(is_world_object: bool) -> Self {
        let mut world = WorldObject::new(
            is_world_object,
            TypeId::Unit,
            TypeMask::OBJECT | TypeMask::UNIT,
        );
        world
            .object_mut()
            .create_flags_mut()
            .insert(crate::CreateObjectFlags::MOVEMENT_UPDATE);

        let mut unit = Self {
            world,
            data: UnitDataValues::default(),
            unit_data_changes: UpdateMask::new(UNIT_DATA_BITS),
            death_state: DeathState::Alive,
            unit_state: 0,
            attacking: None,
            base_attack_speed: [0; MAX_ATTACK],
            mod_attack_speed_pct: [1.0; MAX_ATTACK],
            weapon_damage: [[BASE_MINDAMAGE, BASE_MAXDAMAGE]; MAX_ATTACK],
            speed_rate: [1.0; MAX_MOVE_TYPE],
            power_index: [None; MAX_POWERS],
        };
        unit.set_power_index(PowerType::Mana, Some(0));
        unit
    }

    pub const fn world(&self) -> &WorldObject {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut WorldObject {
        &mut self.world
    }

    pub(crate) fn set_type(&mut self, type_id: TypeId, type_mask: TypeMask) {
        self.world.object_mut().set_type(type_id, type_mask);
    }

    pub const fn data(&self) -> &UnitDataValues {
        &self.data
    }

    pub const fn death_state(&self) -> DeathState {
        self.death_state
    }

    pub fn set_death_state(&mut self, state: DeathState) {
        self.death_state = state;
    }

    pub const fn is_alive(&self) -> bool {
        matches!(self.death_state, DeathState::Alive)
    }

    pub const fn is_dead(&self) -> bool {
        matches!(self.death_state, DeathState::Dead | DeathState::Corpse)
    }

    pub const fn unit_state(&self) -> u32 {
        self.unit_state
    }

    pub fn add_unit_state(&mut self, flags: u32) {
        self.unit_state |= flags;
    }

    pub fn clear_unit_state(&mut self, flags: u32) {
        self.unit_state &= !flags;
    }

    pub fn has_unit_state(&self, flags: u32) -> bool {
        (self.unit_state & flags) != 0
    }

    pub const fn attacking(&self) -> Option<ObjectGuid> {
        self.attacking
    }

    pub fn set_attacking(&mut self, victim: Option<ObjectGuid>) {
        self.attacking = victim;
    }

    pub const fn base_attack_speed(&self) -> [u32; MAX_ATTACK] {
        self.base_attack_speed
    }

    pub const fn mod_attack_speed_pct(&self) -> [f32; MAX_ATTACK] {
        self.mod_attack_speed_pct
    }

    pub const fn weapon_damage(&self, attack: WeaponAttackType) -> [f32; 2] {
        self.weapon_damage[attack as usize]
    }

    pub const fn speed_rate(&self) -> [f32; MAX_MOVE_TYPE] {
        self.speed_rate
    }

    pub fn unit_data_changes_mask(&self) -> &UpdateMask {
        &self.unit_data_changes
    }

    pub fn clear_unit_data_changes(&mut self) {
        self.unit_data_changes.reset_all();
    }

    pub fn set_level(&mut self, level: u8) {
        self.set_i32_field(UNIT_DATA_LEVEL_BIT, i32::from(level), |data| {
            &mut data.level
        });
    }

    pub fn set_faction(&mut self, faction: u32) {
        self.set_i32_field(UNIT_DATA_FACTION_TEMPLATE_BIT, faction as i32, |data| {
            &mut data.faction_template
        });
    }

    pub fn set_bounding_radius(&mut self, radius: f32) {
        self.set_f32_field(UNIT_DATA_BOUNDING_RADIUS_BIT, radius, |data| {
            &mut data.bounding_radius
        });
    }

    pub fn set_combat_reach(&mut self, reach: f32) {
        self.set_f32_field(UNIT_DATA_COMBAT_REACH_BIT, reach, |data| {
            &mut data.combat_reach
        });
    }

    pub fn set_display_id(&mut self, display_id: u32, set_native: bool) {
        self.set_i32_field(UNIT_DATA_DISPLAY_ID_BIT, display_id as i32, |data| {
            &mut data.display_id
        });
        self.set_f32_field(
            UNIT_DATA_DISPLAY_SCALE_BIT,
            DEFAULT_PLAYER_DISPLAY_SCALE,
            |data| &mut data.display_scale,
        );

        if set_native {
            self.set_i32_field(UNIT_DATA_NATIVE_DISPLAY_ID_BIT, display_id as i32, |data| {
                &mut data.native_display_id
            });
            self.set_f32_field(
                UNIT_DATA_NATIVE_DISPLAY_SCALE_BIT,
                DEFAULT_PLAYER_DISPLAY_SCALE,
                |data| &mut data.native_display_scale,
            );
        }
    }

    pub fn set_display_power(&mut self, power: PowerType) {
        self.set_u8_field(UNIT_DATA_DISPLAY_POWER_BIT, power as u8, |data| {
            &mut data.display_power
        });
    }

    pub fn set_target(&mut self, target: ObjectGuid) {
        self.set_guid_field(UNIT_DATA_TARGET_BIT, target, |data| &mut data.target);
    }

    pub fn set_race(&mut self, race: u8) {
        self.set_u8_field(UNIT_DATA_RACE_BIT, race, |data| &mut data.race);
    }

    pub fn set_class(&mut self, class_id: u8) {
        self.set_u8_field(UNIT_DATA_CLASS_ID_BIT, class_id, |data| &mut data.class_id);
    }

    pub fn set_player_class(&mut self, class_id: u8) {
        self.set_u8_field(UNIT_DATA_PLAYER_CLASS_ID_BIT, class_id, |data| {
            &mut data.player_class_id
        });
    }

    pub fn set_gender(&mut self, gender: Gender) {
        self.set_u8_field(UNIT_DATA_SEX_BIT, gender as u8, |data| &mut data.sex);
    }

    pub fn set_health(&mut self, mut value: u64) {
        if matches!(self.death_state, DeathState::JustDied | DeathState::Corpse) {
            value = 0;
        } else if value > self.data.max_health {
            value = self.data.max_health;
        }
        self.set_u64_field(UNIT_DATA_HEALTH_BIT, value, |data| &mut data.health);
    }

    pub fn set_max_health(&mut self, mut value: u64) {
        if value == 0 {
            value = 1;
        }
        let current = self.data.health;
        self.set_u64_field(UNIT_DATA_MAX_HEALTH_BIT, value, |data| &mut data.max_health);
        if value < current {
            self.set_health(value);
        }
    }

    pub fn set_power_index(&mut self, power: PowerType, index: Option<usize>) {
        if let Some(slot) = power_slot(power) {
            self.power_index[slot] = index.filter(|value| *value < MAX_POWERS_PER_CLASS);
        }
    }

    pub fn get_power_index(&self, power: PowerType) -> Option<usize> {
        power_slot(power).and_then(|slot| self.power_index[slot])
    }

    pub fn get_power(&self, power: PowerType) -> i32 {
        self.get_power_index(power)
            .map(|index| self.data.power[index])
            .unwrap_or(0)
    }

    pub fn get_max_power(&self, power: PowerType) -> i32 {
        self.get_power_index(power)
            .map(|index| self.data.max_power[index])
            .unwrap_or(0)
    }

    pub fn set_power(&mut self, power: PowerType, mut value: i32) {
        let Some(index) = self.get_power_index(power) else {
            return;
        };
        let max = self.data.max_power[index];
        if value > max {
            value = max;
        }
        if self.data.power[index] != value {
            self.data.power[index] = value;
            self.mark_unit_data_array(UNIT_DATA_POWER_PARENT_BIT, UNIT_DATA_POWER_FIRST_BIT, index);
        }
    }

    pub fn set_max_power(&mut self, power: PowerType, value: i32) {
        let Some(index) = self.get_power_index(power) else {
            return;
        };
        let current = self.data.power[index];
        if self.data.max_power[index] != value {
            self.data.max_power[index] = value;
            self.mark_unit_data_array(
                UNIT_DATA_POWER_PARENT_BIT,
                UNIT_DATA_MAX_POWER_FIRST_BIT,
                index,
            );
        }
        if value < current {
            self.set_power(power, value);
        }
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.world.object().changed_object_type_mask()
            | if self.unit_data_changes.is_any_set() {
                1 << TYPEID_UNIT
            } else {
                0
            }
    }

    pub fn values_update(&self) -> UnitValuesUpdate {
        let object_update = self.world.object().values_update();
        UnitValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: object_update.object_data,
            unit_data: self.unit_data_changes.is_any_set().then(|| UnitDataUpdate {
                mask: self.unit_data_changes.clone(),
                values: self.data,
            }),
        }
    }

    fn set_u64_field(
        &mut self,
        bit: usize,
        value: u64,
        field: impl FnOnce(&mut UnitDataValues) -> &mut u64,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_unit_data(bit);
        }
    }

    fn set_i32_field(
        &mut self,
        bit: usize,
        value: i32,
        field: impl FnOnce(&mut UnitDataValues) -> &mut i32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_unit_data(bit);
        }
    }

    fn set_u8_field(
        &mut self,
        bit: usize,
        value: u8,
        field: impl FnOnce(&mut UnitDataValues) -> &mut u8,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_unit_data(bit);
        }
    }

    fn set_guid_field(
        &mut self,
        bit: usize,
        value: ObjectGuid,
        field: impl FnOnce(&mut UnitDataValues) -> &mut ObjectGuid,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_unit_data(bit);
        }
    }

    fn set_f32_field(
        &mut self,
        bit: usize,
        value: f32,
        field: impl FnOnce(&mut UnitDataValues) -> &mut f32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_unit_data(bit);
        }
    }

    fn mark_unit_data(&mut self, bit: usize) {
        self.unit_data_changes.set(UNIT_DATA_PARENT_BIT);
        self.unit_data_changes.set(bit);
    }

    fn mark_unit_data_array(&mut self, parent_bit: usize, first_element_bit: usize, index: usize) {
        self.unit_data_changes.set(parent_bit);
        self.unit_data_changes.set(first_element_bit + index);
    }
}

fn power_slot(power: PowerType) -> Option<usize> {
    let value = power as i8;
    (0..MAX_POWERS as i8)
        .contains(&value)
        .then_some(value as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_constructor_matches_cpp_base_state() {
        let unit = Unit::new(true);

        assert_eq!(unit.world().object().type_id(), TypeId::Unit);
        assert_eq!(
            unit.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::UNIT
        );
        assert!(
            unit.world()
                .object()
                .create_flags()
                .contains(crate::CreateObjectFlags::MOVEMENT_UPDATE)
        );
        assert_eq!(unit.death_state(), DeathState::Alive);
        assert_eq!(unit.unit_state(), 0);
        assert_eq!(unit.attacking(), None);
        assert_eq!(unit.base_attack_speed(), [0; MAX_ATTACK]);
        assert_eq!(unit.mod_attack_speed_pct(), [1.0; MAX_ATTACK]);
        assert_eq!(unit.weapon_damage(WeaponAttackType::BaseAttack), [1.0, 2.0]);
        assert_eq!(unit.speed_rate(), [1.0; MAX_MOVE_TYPE]);
        assert!(!unit.unit_data_changes_mask().is_any_set());
    }

    #[test]
    fn health_and_max_health_follow_cpp_clamps() {
        let mut unit = Unit::new(true);

        unit.set_max_health(0);
        assert_eq!(unit.data().max_health, 1);
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_MAX_HEALTH_BIT)
        );

        unit.clear_unit_data_changes();
        unit.set_max_health(100);
        unit.set_health(150);
        assert_eq!(unit.data().health, 100);
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_HEALTH_BIT));

        unit.clear_unit_data_changes();
        unit.set_max_health(40);
        assert_eq!(unit.data().max_health, 40);
        assert_eq!(unit.data().health, 40);
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_MAX_HEALTH_BIT)
        );
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_HEALTH_BIT));

        unit.clear_unit_data_changes();
        unit.set_death_state(DeathState::Corpse);
        unit.set_health(30);
        assert_eq!(unit.data().health, 0);
    }

    #[test]
    fn power_setters_use_derived_power_index_and_cpp_clamps() {
        let mut unit = Unit::new(true);

        assert_eq!(unit.get_power(PowerType::Energy), 0);
        unit.set_power(PowerType::Energy, 10);
        assert!(
            !unit
                .unit_data_changes_mask()
                .is_set(UNIT_DATA_POWER_PARENT_BIT)
        );

        unit.set_power_index(PowerType::Energy, Some(3));
        unit.set_max_power(PowerType::Energy, 100);
        unit.set_power(PowerType::Energy, 150);

        assert_eq!(unit.get_power_index(PowerType::Energy), Some(3));
        assert_eq!(unit.get_power(PowerType::Energy), 100);
        assert_eq!(unit.get_max_power(PowerType::Energy), 100);
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_POWER_PARENT_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_POWER_FIRST_BIT + 3)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_MAX_POWER_FIRST_BIT + 3)
        );
    }

    #[test]
    fn display_level_faction_and_reach_mark_unitdata_bits() {
        let mut unit = Unit::new(true);

        unit.set_level(70);
        unit.set_race(1);
        unit.set_class(2);
        unit.set_player_class(2);
        unit.set_gender(Gender::Female);
        unit.set_target(ObjectGuid::new(7, 11));
        unit.set_faction(35);
        unit.set_bounding_radius(0.5);
        unit.set_combat_reach(1.5);
        unit.set_display_id(1234, true);

        assert_eq!(unit.data().level, 70);
        assert_eq!(unit.data().race, 1);
        assert_eq!(unit.data().class_id, 2);
        assert_eq!(unit.data().player_class_id, 2);
        assert_eq!(unit.data().sex, Gender::Female as u8);
        assert_eq!(unit.data().target, ObjectGuid::new(7, 11));
        assert_eq!(unit.data().faction_template, 35);
        assert_eq!(unit.data().bounding_radius, 0.5);
        assert_eq!(unit.data().combat_reach, 1.5);
        assert_eq!(unit.data().display_id, 1234);
        assert_eq!(unit.data().display_scale, DEFAULT_PLAYER_DISPLAY_SCALE);
        assert_eq!(unit.data().native_display_id, 1234);
        assert_eq!(
            unit.data().native_display_scale,
            DEFAULT_PLAYER_DISPLAY_SCALE
        );
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_PARENT_BIT));
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_LEVEL_BIT));
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_RACE_BIT));
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_CLASS_ID_BIT));
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_PLAYER_CLASS_ID_BIT)
        );
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_SEX_BIT));
        assert!(unit.unit_data_changes_mask().is_set(UNIT_DATA_TARGET_BIT));
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_FACTION_TEMPLATE_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_BOUNDING_RADIUS_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_COMBAT_REACH_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_DISPLAY_ID_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_DISPLAY_SCALE_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_NATIVE_DISPLAY_ID_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_NATIVE_DISPLAY_SCALE_BIT)
        );
    }

    #[test]
    fn values_update_sets_unit_object_type_bit() {
        let mut unit = Unit::new(true);

        unit.set_level(12);
        let update = unit.values_update();

        assert!(update.has_data());
        assert_eq!(update.changed_object_type_mask, 1 << TYPEID_UNIT);
        let unit_data = update.unit_data.unwrap();
        assert_eq!(unit_data.values.level, 12);
        assert!(unit_data.mask.is_set(UNIT_DATA_LEVEL_BIT));
    }
}
