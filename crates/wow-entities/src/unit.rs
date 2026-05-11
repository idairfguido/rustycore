use wow_constants::{
    DeathState, Gender, PowerType, SpellState, TypeId, TypeMask, UnitState, WeaponAttackType,
};
use wow_core::ObjectGuid;

use crate::{
    CurrentSpellRef, CurrentSpellSlot, ObjectDataUpdate, UnitSubsystems, UpdateMask,
    VisibleItemValues, WorldObject,
    update_fields::{TYPEID_UNIT, UNIT_DATA_BITS},
};

pub const MAX_MOVE_TYPE: usize = 9;
pub const MAX_ATTACK: usize = 3;
pub const MAX_POWERS: usize = 26;
pub const MAX_POWERS_PER_CLASS: usize = 10;
pub const BASE_MINDAMAGE: f32 = 1.0;
pub const BASE_MAXDAMAGE: f32 = 2.0;
pub const DEFAULT_PLAYER_DISPLAY_SCALE: f32 = 1.0;
pub const AUTO_SHOT_SPELL_ID: u32 = 75;

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
pub const UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT: usize = 167;
pub const UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT: usize = 168;

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
    pub virtual_items: [VisibleItemValues; MAX_ATTACK],
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
            virtual_items: [VisibleItemValues::default(); MAX_ATTACK],
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
    base_attack_speed: [u32; MAX_ATTACK],
    mod_attack_speed_pct: [f32; MAX_ATTACK],
    weapon_damage: [[f32; 2]; MAX_ATTACK],
    speed_rate: [f32; MAX_MOVE_TYPE],
    power_index: [Option<usize>; MAX_POWERS],
    subsystems: UnitSubsystems,
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
            base_attack_speed: [0; MAX_ATTACK],
            mod_attack_speed_pct: [1.0; MAX_ATTACK],
            weapon_damage: [[BASE_MINDAMAGE, BASE_MAXDAMAGE]; MAX_ATTACK],
            speed_rate: [1.0; MAX_MOVE_TYPE],
            power_index: [None; MAX_POWERS],
            subsystems: UnitSubsystems::default(),
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

    pub fn set_current_cast_spell(
        &mut self,
        slot: CurrentSpellSlot,
        spell: CurrentSpellRef,
    ) -> Option<CurrentSpellRef> {
        if self.subsystems.spells.current_spell(slot) == Some(spell) {
            return None;
        }

        match slot {
            CurrentSpellSlot::Generic => {
                self.interrupt_spell(CurrentSpellSlot::Generic, false, true);
                if self
                    .current_spell(CurrentSpellSlot::Channeled)
                    .is_some_and(|current| !current.allow_actions_during_channel)
                {
                    self.interrupt_spell(CurrentSpellSlot::Channeled, false, true);
                }
                if self
                    .current_spell(CurrentSpellSlot::Autorepeat)
                    .is_some_and(|current| current.spell_id != AUTO_SHOT_SPELL_ID)
                {
                    self.interrupt_spell(CurrentSpellSlot::Autorepeat, true, true);
                }
                if spell.cast_time_ms > 0 {
                    self.add_unit_state(UnitState::CASTING.bits());
                }
            }
            CurrentSpellSlot::Channeled => {
                self.interrupt_spell(CurrentSpellSlot::Generic, false, true);
                self.interrupt_spell(CurrentSpellSlot::Channeled, true, true);
                if self
                    .current_spell(CurrentSpellSlot::Autorepeat)
                    .is_some_and(|current| current.spell_id != AUTO_SHOT_SPELL_ID)
                {
                    self.interrupt_spell(CurrentSpellSlot::Autorepeat, true, true);
                }
                self.add_unit_state(UnitState::CASTING.bits());
            }
            CurrentSpellSlot::Autorepeat => {
                if spell.spell_id != AUTO_SHOT_SPELL_ID {
                    self.interrupt_spell(CurrentSpellSlot::Generic, false, true);
                    self.interrupt_spell(CurrentSpellSlot::Channeled, false, true);
                }
            }
            CurrentSpellSlot::Melee => {}
        }

        self.subsystems.spells.current_spells.insert(slot, spell)
    }

    pub fn current_spell(&self, slot: CurrentSpellSlot) -> Option<CurrentSpellRef> {
        self.subsystems.spells.current_spell(slot)
    }

    pub fn interrupt_spell(
        &mut self,
        slot: CurrentSpellSlot,
        with_delayed: bool,
        with_instant: bool,
    ) -> Option<CurrentSpellRef> {
        let spell = self.current_spell(slot)?;
        if !with_delayed && spell.state == SpellState::Delayed {
            return None;
        }
        if !with_instant && spell.cast_time_ms == 0 && spell.state != SpellState::Casting {
            return None;
        }
        if !spell.interruptible {
            return None;
        }

        let removed = self.subsystems.spells.clear_current_spell(slot);
        self.sync_casting_unit_state();
        removed
    }

    pub fn finish_spell(&mut self, slot: CurrentSpellSlot) -> Option<CurrentSpellRef> {
        let removed = self.subsystems.spells.clear_current_spell(slot);
        self.sync_casting_unit_state();
        removed
    }

    pub fn interrupt_non_melee_spells(
        &mut self,
        spell_id: Option<u32>,
        with_delayed: bool,
        with_instant: bool,
    ) -> Vec<(CurrentSpellSlot, CurrentSpellRef)> {
        let mut removed = Vec::new();
        for slot in [
            CurrentSpellSlot::Generic,
            CurrentSpellSlot::Autorepeat,
            CurrentSpellSlot::Channeled,
        ] {
            let Some(spell) = self.current_spell(slot) else {
                continue;
            };
            if spell_id.is_some_and(|wanted| wanted != spell.spell_id) {
                continue;
            }
            let slot_with_delayed = with_delayed || slot == CurrentSpellSlot::Channeled;
            let slot_with_instant = with_instant || slot == CurrentSpellSlot::Channeled;
            if let Some(interrupted) =
                self.interrupt_spell(slot, slot_with_delayed, slot_with_instant)
            {
                removed.push((slot, interrupted));
            }
        }
        removed
    }

    pub fn find_current_spell_by_spell_id(&self, spell_id: u32) -> Option<CurrentSpellRef> {
        self.subsystems
            .spells
            .find_current_spell_by_spell_id(spell_id)
    }

    fn sync_casting_unit_state(&mut self) {
        if self.current_spell(CurrentSpellSlot::Generic).is_none()
            && self.current_spell(CurrentSpellSlot::Channeled).is_none()
        {
            self.clear_unit_state(UnitState::CASTING.bits());
        }
    }

    pub const fn attacking(&self) -> Option<ObjectGuid> {
        self.subsystems.combat.attacking_guid
    }

    pub fn set_attacking(&mut self, victim: Option<ObjectGuid>) {
        self.subsystems.combat.set_attacking(victim);
    }

    pub const fn subsystems(&self) -> &UnitSubsystems {
        &self.subsystems
    }

    pub fn subsystems_mut(&mut self) -> &mut UnitSubsystems {
        &mut self.subsystems
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

    pub fn set_weapon_damage(
        &mut self,
        attack: WeaponAttackType,
        min_damage: f32,
        max_damage: f32,
    ) {
        let slot = attack as usize;
        if slot < MAX_ATTACK {
            self.weapon_damage[slot] = [min_damage, max_damage];
        }
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

    pub fn set_virtual_item(&mut self, index: usize, visible: Option<VisibleItemValues>) {
        if index >= MAX_ATTACK {
            return;
        }

        let value = visible.unwrap_or_default();
        if self.data.virtual_items[index] != value {
            self.data.virtual_items[index] = value;
            self.mark_unit_data_array(
                UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT,
                UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT,
                index,
            );
        }
    }

    pub fn mark_virtual_item_changed(&mut self, index: usize) {
        if index >= MAX_ATTACK {
            return;
        }

        self.mark_unit_data_array(
            UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT,
            UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT,
            index,
        );
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
    use crate::{
        AppliedAuraRef, AuraRef, CurrentSpellRef, CurrentSpellSlot, MAX_SUMMON_SLOT,
        MovementGeneratorKind, OwnedAuraRef,
    };

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
        assert!(unit.subsystems().auras.owned_auras.is_empty());
        assert!(unit.subsystems().auras.applied_auras.is_empty());
        assert!(unit.subsystems().auras.interruptible_auras.is_empty());
        assert!(unit.subsystems().auras.aura_state_auras.is_empty());
        assert_eq!(unit.subsystems().auras.aura_state_mask, 0);
        assert_eq!(unit.subsystems().auras.removed_auras_count, 0);
        assert!(unit.subsystems().auras.can_proc());
        assert!(unit.subsystems().spells.current_spells.is_empty());
        assert!(unit.subsystems().spells.history.cooldowns.is_empty());
        assert!(unit.subsystems().combat.threat.is_empty());
        assert!(unit.subsystems().combat.threat_refs.is_empty());
        assert!(unit.subsystems().combat.threatened_by_me.is_empty());
        assert!(unit.subsystems().combat.pve_refs.is_empty());
        assert!(unit.subsystems().combat.pvp_refs.is_empty());
        assert_eq!(unit.subsystems().combat.current_victim_guid, None);
        assert_eq!(unit.subsystems().combat.fixate_guid, None);
        assert!(unit.subsystems().combat.attackers.is_empty());
        assert_eq!(unit.subsystems().combat.attacking_guid, None);
        assert!(!unit.subsystems().combat.combat_disallowed);
        assert_eq!(
            unit.subsystems().motion.current_generator,
            MovementGeneratorKind::Idle
        );
        assert!(!unit.subsystems().motion.paused);
        assert!(!unit.subsystems().motion.spline.enabled);
        assert!(unit.subsystems().motion.spline.finalized);
        assert_eq!(unit.subsystems().control.charmer_guid, None);
        assert_eq!(unit.subsystems().control.owner_guid, None);
        assert_eq!(unit.subsystems().control.minion_guid, None);
        assert_eq!(
            unit.subsystems().control.summon_slots,
            [ObjectGuid::EMPTY; MAX_SUMMON_SLOT]
        );
        assert!(!unit.subsystems().control.has_charm_info());
        assert_eq!(unit.subsystems().vehicle.vehicle_guid, None);
        assert_eq!(unit.subsystems().ai.active_ai, None);
        assert!(!unit.subsystems().ai.locked);
        assert!(!unit.subsystems().ai.scheduled_change_pending);
        assert!(!unit.unit_data_changes_mask().is_any_set());
    }

    #[test]
    fn attacking_uses_combat_subsystem_as_single_source_of_truth() {
        let mut unit = Unit::new(true);
        let victim = ObjectGuid::new(1, 10);
        let other_victim = ObjectGuid::new(1, 11);

        unit.set_attacking(Some(victim));
        assert_eq!(unit.attacking(), Some(victim));
        assert_eq!(unit.subsystems().combat.attacking_guid, Some(victim));

        unit.subsystems_mut()
            .combat
            .set_attacking(Some(other_victim));
        assert_eq!(unit.attacking(), Some(other_victim));

        unit.subsystems_mut().combat.clear_attackers();
        assert_eq!(unit.attacking(), None);

        unit.set_attacking(Some(victim));
        unit.subsystems_mut().clear_runtime_state();
        assert_eq!(unit.attacking(), None);
    }

    #[test]
    fn unit_subsystem_helpers_do_not_mark_update_fields() {
        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let target = ObjectGuid::new(1, 2);

        unit.clear_unit_data_changes();
        let owned = OwnedAuraRef::new(17, caster, None);
        let applied = AppliedAuraRef::new(17, caster, 3, 0x7);
        unit.subsystems_mut().auras.add_owned(owned);
        unit.subsystems_mut().auras.add_applied(applied);
        unit.subsystems_mut()
            .auras
            .set_visible(3, AuraRef::new(17, caster));
        unit.subsystems_mut().spells.set_current_spell(
            CurrentSpellSlot::Channeled,
            CurrentSpellRef::new(42, Some(caster), None),
        );
        unit.subsystems_mut()
            .spells
            .history
            .set_cooldown(42, 100, 1_500);
        unit.subsystems_mut().combat.add_threat(target, 2.0);
        unit.subsystems_mut().motion.start_spline(9, 500);
        unit.subsystems_mut().control.set_charmer(caster, true);
        unit.subsystems_mut().vehicle.enter_vehicle(target, Some(0));
        unit.subsystems_mut().ai.push("TestAI");

        assert!(unit.subsystems().auras.has_owned(owned));
        assert!(unit.subsystems().auras.has_applied(applied));
        assert_eq!(
            unit.subsystems()
                .spells
                .current_spell(CurrentSpellSlot::Channeled)
                .map(|spell| spell.spell_id),
            Some(42)
        );
        assert_eq!(
            unit.subsystems()
                .spells
                .history
                .cooldown(42)
                .map(|cooldown| cooldown.cooldown_end_ms),
            Some(1_600)
        );
        assert!(unit.subsystems().combat.is_threatened_by(target));
        assert!(unit.subsystems().motion.spline.enabled);
        assert!(unit.subsystems().control.is_charmed());
        assert_eq!(unit.subsystems().vehicle.vehicle_guid, Some(target));
        assert_eq!(unit.subsystems().ai.active_ai.as_deref(), Some("TestAI"));
        assert!(!unit.unit_data_changes_mask().is_any_set());
    }

    #[test]
    fn current_spell_slots_follow_cpp_ids_and_breakage_rules() {
        assert_eq!(CurrentSpellSlot::Melee as u8, 0);
        assert_eq!(CurrentSpellSlot::Generic as u8, 1);
        assert_eq!(CurrentSpellSlot::Channeled as u8, 2);
        assert_eq!(CurrentSpellSlot::Autorepeat as u8, 3);

        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let generic = CurrentSpellRef::new(100, Some(caster), None).with_cast_time_ms(1_500);
        let auto_shot = CurrentSpellRef::new(AUTO_SHOT_SPELL_ID, Some(caster), None);
        let other_auto = CurrentSpellRef::new(200, Some(caster), None);

        unit.set_current_cast_spell(CurrentSpellSlot::Autorepeat, auto_shot);
        unit.set_current_cast_spell(CurrentSpellSlot::Generic, generic);
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), Some(generic));
        assert_eq!(
            unit.current_spell(CurrentSpellSlot::Autorepeat),
            Some(auto_shot)
        );
        assert!(unit.has_unit_state(UnitState::CASTING.bits()));

        unit.set_current_cast_spell(CurrentSpellSlot::Autorepeat, other_auto);
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), None);
        assert_eq!(
            unit.current_spell(CurrentSpellSlot::Autorepeat),
            Some(other_auto)
        );
        assert!(!unit.has_unit_state(UnitState::CASTING.bits()));
    }

    #[test]
    fn current_spell_generic_respects_channels_that_allow_actions() {
        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let channel_with_actions = CurrentSpellRef::new(300, Some(caster), None)
            .with_cast_time_ms(2_000)
            .with_allow_actions_during_channel(true);
        let generic = CurrentSpellRef::new(301, Some(caster), None).with_cast_time_ms(1_000);

        unit.set_current_cast_spell(CurrentSpellSlot::Channeled, channel_with_actions);
        unit.set_current_cast_spell(CurrentSpellSlot::Generic, generic);
        assert_eq!(
            unit.current_spell(CurrentSpellSlot::Channeled),
            Some(channel_with_actions)
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), Some(generic));

        let regular_channel =
            CurrentSpellRef::new(302, Some(caster), None).with_cast_time_ms(2_000);
        let next_generic = CurrentSpellRef::new(303, Some(caster), None).with_cast_time_ms(1_000);
        unit.set_current_cast_spell(CurrentSpellSlot::Channeled, regular_channel);
        unit.set_current_cast_spell(CurrentSpellSlot::Generic, next_generic);
        assert_eq!(unit.current_spell(CurrentSpellSlot::Channeled), None);
        assert_eq!(
            unit.current_spell(CurrentSpellSlot::Generic),
            Some(next_generic)
        );
    }

    #[test]
    fn interrupt_spell_honors_cpp_delayed_instant_and_interruptible_guards() {
        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let instant = CurrentSpellRef::new(400, Some(caster), None);
        let delayed = CurrentSpellRef::new(401, Some(caster), None)
            .with_cast_time_ms(1_000)
            .with_state(SpellState::Delayed);
        let casting_instant =
            CurrentSpellRef::new(402, Some(caster), None).with_state(SpellState::Casting);
        let protected = CurrentSpellRef::new(403, Some(caster), None)
            .with_cast_time_ms(1_000)
            .with_interruptible(false);

        unit.set_current_cast_spell(CurrentSpellSlot::Generic, instant);
        assert_eq!(
            unit.interrupt_spell(CurrentSpellSlot::Generic, true, false),
            None
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), Some(instant));

        unit.set_current_cast_spell(CurrentSpellSlot::Generic, delayed);
        assert_eq!(
            unit.interrupt_spell(CurrentSpellSlot::Generic, false, true),
            None
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), Some(delayed));

        unit.set_current_cast_spell(CurrentSpellSlot::Generic, casting_instant);
        assert_eq!(
            unit.interrupt_spell(CurrentSpellSlot::Generic, true, false),
            Some(casting_instant)
        );

        unit.set_current_cast_spell(CurrentSpellSlot::Generic, protected);
        assert_eq!(
            unit.interrupt_spell(CurrentSpellSlot::Generic, true, true),
            None
        );
        assert_eq!(
            unit.current_spell(CurrentSpellSlot::Generic),
            Some(protected)
        );
        assert_eq!(
            unit.finish_spell(CurrentSpellSlot::Generic),
            Some(protected)
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), None);
    }

    #[test]
    fn interrupt_non_melee_spells_filters_and_forces_channeled_interrupts() {
        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let melee = CurrentSpellRef::new(500, Some(caster), None);
        let generic = CurrentSpellRef::new(501, Some(caster), None).with_cast_time_ms(1_000);
        let auto = CurrentSpellRef::new(502, Some(caster), None);
        let delayed_channel = CurrentSpellRef::new(503, Some(caster), None)
            .with_state(SpellState::Delayed)
            .with_cast_time_ms(1_000);

        unit.subsystems_mut()
            .spells
            .set_current_spell(CurrentSpellSlot::Melee, melee);
        unit.subsystems_mut()
            .spells
            .set_current_spell(CurrentSpellSlot::Generic, generic);
        unit.subsystems_mut()
            .spells
            .set_current_spell(CurrentSpellSlot::Autorepeat, auto);
        unit.subsystems_mut()
            .spells
            .set_current_spell(CurrentSpellSlot::Channeled, delayed_channel);

        let removed = unit.interrupt_non_melee_spells(Some(503), false, false);
        assert_eq!(
            removed,
            vec![(CurrentSpellSlot::Channeled, delayed_channel)]
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Melee), Some(melee));
        assert_eq!(unit.current_spell(CurrentSpellSlot::Generic), Some(generic));
        assert_eq!(unit.current_spell(CurrentSpellSlot::Autorepeat), Some(auto));

        let removed = unit.interrupt_non_melee_spells(None, true, true);
        assert_eq!(
            removed,
            vec![
                (CurrentSpellSlot::Generic, generic),
                (CurrentSpellSlot::Autorepeat, auto),
            ]
        );
        assert_eq!(unit.current_spell(CurrentSpellSlot::Melee), Some(melee));
    }

    #[test]
    fn find_current_spell_by_spell_id_searches_all_cpp_slots() {
        let mut unit = Unit::new(true);
        let caster = ObjectGuid::new(1, 1);
        let melee = CurrentSpellRef::new(600, Some(caster), None);
        let channel = CurrentSpellRef::new(601, Some(caster), None).with_cast_time_ms(1_000);

        unit.set_current_cast_spell(CurrentSpellSlot::Melee, melee);
        unit.set_current_cast_spell(CurrentSpellSlot::Channeled, channel);

        assert_eq!(unit.find_current_spell_by_spell_id(600), Some(melee));
        assert_eq!(unit.find_current_spell_by_spell_id(601), Some(channel));
        assert_eq!(unit.find_current_spell_by_spell_id(602), None);
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
    fn virtual_item_updates_mark_cpp_parent_and_element_bits() {
        let mut unit = Unit::new(true);
        unit.clear_unit_data_changes();

        unit.set_virtual_item(
            1,
            Some(VisibleItemValues {
                item_id: 19019,
                item_appearance_mod_id: 2,
                item_visual: 3,
            }),
        );

        assert_eq!(unit.data().virtual_items[1].item_id, 19019);
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT + 1)
        );
        assert!(
            !unit
                .unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT)
        );

        unit.clear_unit_data_changes();
        unit.set_virtual_item(1, None);
        assert_eq!(unit.data().virtual_items[1], VisibleItemValues::default());
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT + 1)
        );
    }

    #[test]
    fn virtual_item_mark_changed_forces_default_value_delta() {
        let mut unit = Unit::new(true);
        unit.clear_unit_data_changes();

        unit.mark_virtual_item_changed(2);

        assert_eq!(unit.data().virtual_items[2], VisibleItemValues::default());
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT)
        );
        assert!(
            unit.unit_data_changes_mask()
                .is_set(UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT + 2)
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
