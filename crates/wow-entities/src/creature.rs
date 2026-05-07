use wow_constants::{PowerType, TypeId, TypeMask};

use crate::Unit;

pub const CREATURE_REGEN_INTERVAL_MS: u32 = 2_000;
pub const MAX_CREATURE_SPELLS: usize = 8;
pub const DEFAULT_RESPAWN_DELAY_SECS: u32 = 300;
pub const DEFAULT_CORPSE_DELAY_SECS: u32 = 60;
pub const DEFAULT_BOUNDARY_CHECK_TIME_MS: u32 = 2_500;
pub const DEFAULT_MONSTER_SIGHT_DISTANCE: f32 = 50.0;
pub const LOOT_MODE_DEFAULT: u16 = 0x1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReactState {
    Passive = 0,
    Defensive = 1,
    Aggressive = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MovementGeneratorType {
    Idle = 0,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureModelDimensions {
    pub bounding_radius: f32,
    pub combat_reach: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Creature {
    unit: Unit,
    player_damage_req: u32,
    dont_clear_tap_list_on_evade: bool,
    pickpocket_loot_restore: i64,
    corpse_remove_time: i64,
    respawn_time: i64,
    respawn_delay: u32,
    corpse_delay: u32,
    ignore_corpse_decay_ratio: bool,
    wander_distance: f32,
    boundary_check_time: u32,
    combat_pulse_time: u32,
    combat_pulse_delay: u32,
    react_state: ReactState,
    default_movement_type: MovementGeneratorType,
    spawn_id: u64,
    equipment_id: u8,
    original_equipment_id: i8,
    already_call_assistance: bool,
    already_searched_assistance: bool,
    cannot_reach_target: bool,
    cannot_reach_timer: u32,
    melee_damage_school_mask: u32,
    original_entry: u32,
    trigger_just_appeared: bool,
    respawn_compatibility_mode: bool,
    last_damaged_time: i64,
    regenerate_health: bool,
    is_missing_can_swim_flag_out_of_combat: bool,
    gossip_menu_id: u32,
    sparring_health_pct: u8,
    regen_timer: u32,
    spells: [u32; MAX_CREATURE_SPELLS],
    disable_reputation_gain: bool,
    sight_distance: f32,
    combat_distance: f32,
    loot_mode: u16,
    is_temp_world_object: bool,
}

impl Creature {
    pub fn new(is_world_object: bool) -> Self {
        let mut unit = Unit::new(is_world_object);
        unit.set_type(TypeId::Unit, TypeMask::OBJECT | TypeMask::UNIT);
        unit.set_power_index(PowerType::Mana, Some(0));
        unit.set_power_index(PowerType::ComboPoints, Some(2));

        Self {
            unit,
            player_damage_req: 0,
            dont_clear_tap_list_on_evade: false,
            pickpocket_loot_restore: 0,
            corpse_remove_time: 0,
            respawn_time: 0,
            respawn_delay: DEFAULT_RESPAWN_DELAY_SECS,
            corpse_delay: DEFAULT_CORPSE_DELAY_SECS,
            ignore_corpse_decay_ratio: false,
            wander_distance: 0.0,
            boundary_check_time: DEFAULT_BOUNDARY_CHECK_TIME_MS,
            combat_pulse_time: 0,
            combat_pulse_delay: 0,
            react_state: ReactState::Aggressive,
            default_movement_type: MovementGeneratorType::Idle,
            spawn_id: 0,
            equipment_id: 0,
            original_equipment_id: 0,
            already_call_assistance: false,
            already_searched_assistance: false,
            cannot_reach_target: false,
            cannot_reach_timer: 0,
            melee_damage_school_mask: 0x1,
            original_entry: 0,
            trigger_just_appeared: true,
            respawn_compatibility_mode: false,
            last_damaged_time: 0,
            regenerate_health: true,
            is_missing_can_swim_flag_out_of_combat: false,
            gossip_menu_id: 0,
            sparring_health_pct: 0,
            regen_timer: CREATURE_REGEN_INTERVAL_MS,
            spells: [0; MAX_CREATURE_SPELLS],
            disable_reputation_gain: false,
            sight_distance: DEFAULT_MONSTER_SIGHT_DISTANCE,
            combat_distance: 0.0,
            loot_mode: LOOT_MODE_DEFAULT,
            is_temp_world_object: false,
        }
    }

    pub const fn unit(&self) -> &Unit {
        &self.unit
    }

    pub fn unit_mut(&mut self) -> &mut Unit {
        &mut self.unit
    }

    pub const fn player_damage_req(&self) -> u32 {
        self.player_damage_req
    }

    pub const fn corpse_remove_time(&self) -> i64 {
        self.corpse_remove_time
    }

    pub const fn respawn_time(&self) -> i64 {
        self.respawn_time
    }

    pub fn set_respawn_time(&mut self, respawn_time: i64) {
        self.respawn_time = respawn_time;
    }

    pub const fn respawn_delay(&self) -> u32 {
        self.respawn_delay
    }

    pub fn set_respawn_delay(&mut self, delay: u32) {
        self.respawn_delay = delay;
    }

    pub const fn corpse_delay(&self) -> u32 {
        self.corpse_delay
    }

    pub fn set_corpse_delay(&mut self, delay: u32, ignore_corpse_decay_ratio: bool) {
        self.corpse_delay = delay;
        if ignore_corpse_decay_ratio {
            self.ignore_corpse_decay_ratio = true;
        }
    }

    pub const fn ignore_corpse_decay_ratio(&self) -> bool {
        self.ignore_corpse_decay_ratio
    }

    pub const fn wander_distance(&self) -> f32 {
        self.wander_distance
    }

    pub const fn boundary_check_time(&self) -> u32 {
        self.boundary_check_time
    }

    pub const fn combat_pulse_time(&self) -> u32 {
        self.combat_pulse_time
    }

    pub const fn combat_pulse_delay(&self) -> u32 {
        self.combat_pulse_delay
    }

    pub const fn react_state(&self) -> ReactState {
        self.react_state
    }

    pub fn set_react_state(&mut self, state: ReactState) {
        self.react_state = state;
    }

    pub fn has_react_state(&self, state: ReactState) -> bool {
        self.react_state == state
    }

    pub const fn default_movement_type(&self) -> MovementGeneratorType {
        self.default_movement_type
    }

    pub const fn spawn_id(&self) -> u64 {
        self.spawn_id
    }

    pub fn set_spawn_id(&mut self, spawn_id: u64) {
        self.spawn_id = spawn_id;
    }

    pub const fn equipment_id(&self) -> u8 {
        self.equipment_id
    }

    pub const fn original_equipment_id(&self) -> i8 {
        self.original_equipment_id
    }

    pub const fn already_call_assistance(&self) -> bool {
        self.already_call_assistance
    }

    pub const fn already_searched_assistance(&self) -> bool {
        self.already_searched_assistance
    }

    pub const fn cannot_reach_target(&self) -> bool {
        self.cannot_reach_target
    }

    pub const fn cannot_reach_timer(&self) -> u32 {
        self.cannot_reach_timer
    }

    pub const fn melee_damage_school_mask(&self) -> u32 {
        self.melee_damage_school_mask
    }

    pub const fn original_entry(&self) -> u32 {
        self.original_entry
    }

    pub const fn trigger_just_appeared(&self) -> bool {
        self.trigger_just_appeared
    }

    pub const fn respawn_compatibility_mode(&self) -> bool {
        self.respawn_compatibility_mode
    }

    pub fn set_respawn_compatibility_mode(&mut self, enabled: bool) {
        self.respawn_compatibility_mode = enabled;
    }

    pub const fn last_damaged_time(&self) -> i64 {
        self.last_damaged_time
    }

    pub const fn regenerate_health(&self) -> bool {
        self.regenerate_health
    }

    pub const fn is_missing_can_swim_flag_out_of_combat(&self) -> bool {
        self.is_missing_can_swim_flag_out_of_combat
    }

    pub const fn gossip_menu_id(&self) -> u32 {
        self.gossip_menu_id
    }

    pub const fn sparring_health_pct(&self) -> u8 {
        self.sparring_health_pct
    }

    pub const fn regen_timer(&self) -> u32 {
        self.regen_timer
    }

    pub const fn spells(&self) -> [u32; MAX_CREATURE_SPELLS] {
        self.spells
    }

    pub const fn disable_reputation_gain(&self) -> bool {
        self.disable_reputation_gain
    }

    pub const fn sight_distance(&self) -> f32 {
        self.sight_distance
    }

    pub const fn combat_distance(&self) -> f32 {
        self.combat_distance
    }

    pub const fn loot_mode(&self) -> u16 {
        self.loot_mode
    }

    pub fn reset_loot_mode(&mut self) {
        self.loot_mode = LOOT_MODE_DEFAULT;
    }

    pub const fn is_temp_world_object(&self) -> bool {
        self.is_temp_world_object
    }

    pub fn get_power_index(&self, power: PowerType) -> Option<usize> {
        if power == self.power_type() {
            Some(0)
        } else if power == PowerType::ComboPoints {
            Some(2)
        } else {
            None
        }
    }

    pub fn power_type(&self) -> PowerType {
        power_type_from_u8(self.unit.data().display_power)
    }

    pub fn set_power_type(&mut self, power: PowerType) {
        let old_power = self.power_type();
        if old_power != PowerType::ComboPoints {
            self.unit.set_power_index(old_power, None);
        }
        self.unit.set_display_power(power);
        self.unit.set_power_index(power, Some(0));
        self.unit.set_power_index(PowerType::ComboPoints, Some(2));
    }

    pub fn set_display_id(
        &mut self,
        display_id: u32,
        set_native: bool,
        model: Option<CreatureModelDimensions>,
    ) {
        self.unit.set_display_id(display_id, set_native);

        if let Some(model) = model {
            let scale = self.unit.world().object().scale() * self.unit.data().display_scale;
            self.unit.set_bounding_radius(model.bounding_radius * scale);
            self.unit.set_combat_reach(model.combat_reach * scale);
        }
    }

    pub fn set_faction(&mut self, faction: u32) {
        self.unit.set_faction(faction);
    }
}

fn power_type_from_u8(power: u8) -> PowerType {
    match power {
        1 => PowerType::Rage,
        2 => PowerType::Focus,
        3 => PowerType::Energy,
        4 => PowerType::Happiness,
        5 => PowerType::Runes,
        6 => PowerType::RunicPower,
        7 => PowerType::SoulShards,
        8 => PowerType::LunarPower,
        9 => PowerType::HolyPower,
        10 => PowerType::AlternatePower,
        11 => PowerType::Maelstrom,
        12 => PowerType::Chi,
        13 => PowerType::Insanity,
        14 => PowerType::ComboPoints,
        15 => PowerType::DemonicFury,
        16 => PowerType::ArcaneCharges,
        17 => PowerType::Fury,
        18 => PowerType::Pain,
        19 => PowerType::Essence,
        20 => PowerType::RuneBlood,
        21 => PowerType::RuneFrost,
        22 => PowerType::RuneUnholy,
        23 => PowerType::AlternateQuest,
        24 => PowerType::AlternateEncounter,
        25 => PowerType::AlternateMount,
        _ => PowerType::Mana,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_constructor_matches_cpp_base_state() {
        let creature = Creature::new(false);

        assert_eq!(creature.unit().world().object().type_id(), TypeId::Unit);
        assert_eq!(
            creature.unit().world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::UNIT
        );
        assert!(!creature.unit().world().is_world_object());
        assert_eq!(creature.player_damage_req(), 0);
        assert_eq!(creature.corpse_remove_time(), 0);
        assert_eq!(creature.respawn_time(), 0);
        assert_eq!(creature.respawn_delay(), DEFAULT_RESPAWN_DELAY_SECS);
        assert_eq!(creature.corpse_delay(), DEFAULT_CORPSE_DELAY_SECS);
        assert!(!creature.ignore_corpse_decay_ratio());
        assert_eq!(creature.wander_distance(), 0.0);
        assert_eq!(
            creature.boundary_check_time(),
            DEFAULT_BOUNDARY_CHECK_TIME_MS
        );
        assert_eq!(creature.combat_pulse_time(), 0);
        assert_eq!(creature.combat_pulse_delay(), 0);
        assert_eq!(creature.react_state(), ReactState::Aggressive);
        assert_eq!(
            creature.default_movement_type(),
            MovementGeneratorType::Idle
        );
        assert_eq!(creature.spawn_id(), 0);
        assert_eq!(creature.equipment_id(), 0);
        assert_eq!(creature.original_equipment_id(), 0);
        assert!(!creature.already_call_assistance());
        assert!(!creature.already_searched_assistance());
        assert!(!creature.cannot_reach_target());
        assert_eq!(creature.cannot_reach_timer(), 0);
        assert_eq!(creature.melee_damage_school_mask(), 0x1);
        assert_eq!(creature.original_entry(), 0);
        assert!(creature.trigger_just_appeared());
        assert!(!creature.respawn_compatibility_mode());
        assert_eq!(creature.last_damaged_time(), 0);
        assert!(creature.regenerate_health());
        assert!(!creature.is_missing_can_swim_flag_out_of_combat());
        assert_eq!(creature.gossip_menu_id(), 0);
        assert_eq!(creature.sparring_health_pct(), 0);
        assert_eq!(creature.regen_timer(), CREATURE_REGEN_INTERVAL_MS);
        assert_eq!(creature.spells(), [0; MAX_CREATURE_SPELLS]);
        assert!(!creature.disable_reputation_gain());
        assert_eq!(creature.sight_distance(), DEFAULT_MONSTER_SIGHT_DISTANCE);
        assert_eq!(creature.combat_distance(), 0.0);
        assert_eq!(creature.loot_mode(), LOOT_MODE_DEFAULT);
        assert!(!creature.is_temp_world_object());
    }

    #[test]
    fn creature_power_index_matches_cpp_stat_system() {
        let mut creature = Creature::new(false);

        assert_eq!(creature.get_power_index(PowerType::Mana), Some(0));
        assert_eq!(creature.get_power_index(PowerType::ComboPoints), Some(2));
        assert_eq!(creature.get_power_index(PowerType::Energy), None);

        creature.set_power_type(PowerType::Energy);
        assert_eq!(creature.power_type(), PowerType::Energy);
        assert_eq!(creature.get_power_index(PowerType::Energy), Some(0));
        assert_eq!(creature.get_power_index(PowerType::Mana), None);
        assert_eq!(creature.get_power_index(PowerType::ComboPoints), Some(2));
    }

    #[test]
    fn creature_respawn_and_corpse_setters_match_cpp_fields() {
        let mut creature = Creature::new(false);

        creature.set_respawn_delay(45);
        creature.set_respawn_time(1234);
        creature.set_corpse_delay(10, true);
        creature.set_respawn_compatibility_mode(true);
        creature.set_spawn_id(99);

        assert_eq!(creature.respawn_delay(), 45);
        assert_eq!(creature.respawn_time(), 1234);
        assert_eq!(creature.corpse_delay(), 10);
        assert!(creature.ignore_corpse_decay_ratio());
        assert!(creature.respawn_compatibility_mode());
        assert_eq!(creature.spawn_id(), 99);
    }

    #[test]
    fn creature_display_with_model_updates_unit_dimensions_like_cpp() {
        let mut creature = Creature::new(false);

        creature.unit_mut().world_mut().object_mut().set_scale(2.0);
        creature.set_display_id(
            1234,
            true,
            Some(CreatureModelDimensions {
                bounding_radius: 0.3,
                combat_reach: 1.5,
            }),
        );

        let scale = 2.0 * crate::DEFAULT_PLAYER_DISPLAY_SCALE;
        assert_eq!(creature.unit().data().display_id, 1234);
        assert_eq!(creature.unit().data().native_display_id, 1234);
        assert_eq!(creature.unit().data().bounding_radius, 0.3 * scale);
        assert_eq!(creature.unit().data().combat_reach, 1.5 * scale);
    }

    #[test]
    fn creature_react_state_and_faction_use_unit_fields() {
        let mut creature = Creature::new(false);

        creature.set_react_state(ReactState::Passive);
        creature.set_faction(35);

        assert!(creature.has_react_state(ReactState::Passive));
        assert_eq!(creature.unit().data().faction_template, 35);
    }
}
