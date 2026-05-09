use wow_constants::{PowerType, TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatureLifecycleStats {
    pub max_health: u64,
    pub health: u64,
    pub power_type: PowerType,
    pub max_mana: i32,
    pub mana: i32,
    pub min_damage: f32,
    pub max_damage: f32,
}

impl CreatureLifecycleStats {
    pub const fn new(max_health: u64, health: u64, max_mana: i32, mana: i32) -> Self {
        Self {
            max_health,
            health,
            power_type: PowerType::Mana,
            max_mana,
            mana,
            min_damage: 0.0,
            max_damage: 0.0,
        }
    }
}

/// Represented subset of TrinityCore `CreatureTemplate`/difficulty data used by
/// `Creature::InitEntry` and `CreateFromProto`.
///
/// ObjectMgr, DB2 model stores, addon/equipment table loading and script binding are deliberately
/// external to this record. Callers pass the already-resolved values that `wow-entities` can own.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureTemplateLifecycleRecord {
    pub entry: u32,
    pub original_entry: u32,
    pub difficulty_id: u8,
    pub name: String,
    pub unit_class: u8,
    pub faction: u32,
    pub display_id: u32,
    pub model_dimensions: Option<CreatureModelDimensions>,
    pub scale: f32,
    pub speed_walk: f32,
    pub speed_run: f32,
    pub spells: [u32; MAX_CREATURE_SPELLS],
    pub classification: u32,
    pub flags_extra: u32,
    pub type_flags: u32,
    pub movement_type: MovementGeneratorType,
    pub min_level: u8,
    pub max_level: u8,
    pub equipment_id: u8,
    pub original_equipment_id: i8,
}

/// Represented subset of TrinityCore `CreatureData` consumed by `Creature::LoadFromDB`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureSpawnLifecycleRecord {
    pub spawn_id: u64,
    pub map_id: u32,
    pub instance_id: u32,
    pub position: Position,
    pub home_position: Position,
    pub phase_id: Option<u32>,
    pub phase_group: Option<u32>,
    pub terrain_swap_map: Option<u32>,
    pub spawn_group_id: Option<u32>,
    pub spawn_group_name: Option<String>,
    pub pool_id: Option<u32>,
    pub equipment_id: Option<u8>,
    pub original_equipment_id: Option<i8>,
    pub wander_distance: f32,
    pub respawn_delay: u32,
    pub respawn_time: i64,
    pub movement_type: MovementGeneratorType,
    pub string_id: Option<String>,
    pub is_active: bool,
    pub inactive_by_spawn_group: bool,
    pub duplicate_spawn_found: bool,
    pub add_to_map: bool,
    pub respawn_compatibility_mode: bool,
}

/// Resolved, testable input for TrinityCore `Creature::Create`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureCreateLifecycleRecord {
    pub guid: ObjectGuid,
    pub entry: u32,
    pub map_id: u32,
    pub instance_id: u32,
    pub position: Position,
    pub dynamic: bool,
    pub vehicle_id: Option<u32>,
    pub template: CreatureTemplateLifecycleRecord,
    pub spawn: Option<CreatureSpawnLifecycleRecord>,
    pub selected_level: u8,
    pub stats: CreatureLifecycleStats,
    pub selected_display_id: u32,
    pub selected_model_dimensions: Option<CreatureModelDimensions>,
    pub selected_equipment_id: u8,
    pub selected_original_equipment_id: i8,
    pub corpse_delay: u32,
    pub ignore_corpse_decay_ratio: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatureLoadFromDbLifecycleRecord {
    pub create: CreatureCreateLifecycleRecord,
    pub spawn: CreatureSpawnLifecycleRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatureLifecycleMetadata {
    pub template_entry: u32,
    pub original_entry: u32,
    pub difficulty_id: u8,
    pub unit_class: u8,
    pub classification: u32,
    pub flags_extra: u32,
    pub type_flags: u32,
    pub selected_level: u8,
    pub selected_display_id: u32,
    pub selected_model_dimensions: Option<CreatureModelDimensions>,
    pub template_scale: f32,
    pub speed_walk: f32,
    pub speed_run: f32,
    pub spawn_id: u64,
    pub spawn_map_id: u32,
    pub spawn_instance_id: u32,
    pub spawn_position: Position,
    pub home_position: Position,
    pub phase_id: Option<u32>,
    pub phase_group: Option<u32>,
    pub terrain_swap_map: Option<u32>,
    pub spawn_group_id: Option<u32>,
    pub spawn_group_name: Option<String>,
    pub pool_id: Option<u32>,
    pub string_id: Option<String>,
    pub is_spawn_active: bool,
    pub inactive_by_spawn_group: bool,
    pub duplicate_spawn_found: bool,
    pub add_to_map_requested: bool,
    pub map_insertion_requested: bool,
    pub dynamic_spawn: bool,
    pub vehicle_id: Option<u32>,
    pub equipment_id: u8,
    pub original_equipment_id: i8,
}

impl Default for CreatureLifecycleMetadata {
    fn default() -> Self {
        Self {
            template_entry: 0,
            original_entry: 0,
            difficulty_id: 0,
            unit_class: 0,
            classification: 0,
            flags_extra: 0,
            type_flags: 0,
            selected_level: 0,
            selected_display_id: 0,
            selected_model_dimensions: None,
            template_scale: 1.0,
            speed_walk: 1.0,
            speed_run: 1.0,
            spawn_id: 0,
            spawn_map_id: 0,
            spawn_instance_id: 0,
            spawn_position: Position::ZERO,
            home_position: Position::ZERO,
            phase_id: None,
            phase_group: None,
            terrain_swap_map: None,
            spawn_group_id: None,
            spawn_group_name: None,
            pool_id: None,
            string_id: None,
            is_spawn_active: true,
            inactive_by_spawn_group: false,
            duplicate_spawn_found: false,
            add_to_map_requested: false,
            map_insertion_requested: false,
            dynamic_spawn: false,
            vehicle_id: None,
            equipment_id: 0,
            original_equipment_id: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatureLifecycleStep {
    SetMapAndPhase,
    ApplyRespawnCompatibility,
    LookupTemplateAndDifficulty,
    RelocateAndValidatePosition,
    InitEntryAndCreateFromProto,
    SelectLevel,
    UpdateLevelDependantStats,
    ApplyAddonEquipmentSparringHoverScriptFlags,
    InitializeThreatManager,
    LoadFromDbSpawnHomeRespawnInactiveChecks,
    SetSpawnHealthDefaultMovementAndStringId,
    AddToMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureLifecyclePlan {
    steps: Vec<CreatureLifecycleStep>,
}

impl CreatureLifecyclePlan {
    pub fn trinity_create_load_from_db() -> Self {
        Self {
            steps: vec![
                CreatureLifecycleStep::SetMapAndPhase,
                CreatureLifecycleStep::ApplyRespawnCompatibility,
                CreatureLifecycleStep::LookupTemplateAndDifficulty,
                CreatureLifecycleStep::RelocateAndValidatePosition,
                CreatureLifecycleStep::InitEntryAndCreateFromProto,
                CreatureLifecycleStep::SelectLevel,
                CreatureLifecycleStep::UpdateLevelDependantStats,
                CreatureLifecycleStep::ApplyAddonEquipmentSparringHoverScriptFlags,
                CreatureLifecycleStep::InitializeThreatManager,
                CreatureLifecycleStep::LoadFromDbSpawnHomeRespawnInactiveChecks,
                CreatureLifecycleStep::SetSpawnHealthDefaultMovementAndStringId,
                CreatureLifecycleStep::AddToMap,
            ],
        }
    }

    pub fn steps(&self) -> &[CreatureLifecycleStep] {
        &self.steps
    }

    pub fn position_of(&self, step: CreatureLifecycleStep) -> Option<usize> {
        self.steps.iter().position(|candidate| *candidate == step)
    }

    pub fn occurs_before(
        &self,
        before: CreatureLifecycleStep,
        after: CreatureLifecycleStep,
    ) -> bool {
        match (self.position_of(before), self.position_of(after)) {
            (Some(before_index), Some(after_index)) => before_index < after_index,
            _ => false,
        }
    }
}

impl Default for CreatureLifecyclePlan {
    fn default() -> Self {
        Self::trinity_create_load_from_db()
    }
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
    grid_unload_cleanup_before_delete_count: u32,
    grid_unload_delete_requested: bool,
    grid_unload_respawn_relocation_requested: bool,
    lifecycle_metadata: CreatureLifecycleMetadata,
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
            grid_unload_cleanup_before_delete_count: 0,
            grid_unload_delete_requested: false,
            grid_unload_respawn_relocation_requested: false,
            lifecycle_metadata: CreatureLifecycleMetadata::default(),
        }
    }

    pub fn create_from_lifecycle(record: CreatureCreateLifecycleRecord) -> Self {
        let mut creature = Self::new(false);
        creature.apply_create_lifecycle(record);
        creature
    }

    pub fn load_from_db_lifecycle(record: CreatureLoadFromDbLifecycleRecord) -> Self {
        let mut creature = Self::create_from_lifecycle(record.create);
        creature.apply_load_from_db_lifecycle(&record.spawn);
        creature
    }

    pub fn apply_create_lifecycle(&mut self, record: CreatureCreateLifecycleRecord) {
        let template = &record.template;
        let spawn = record.spawn.as_ref();
        let map_id = spawn.map(|spawn| spawn.map_id).unwrap_or(record.map_id);
        let instance_id = spawn
            .map(|spawn| spawn.instance_id)
            .unwrap_or(record.instance_id);
        let position = spawn.map(|spawn| spawn.position).unwrap_or(record.position);
        let home_position = spawn
            .map(|spawn| spawn.home_position)
            .unwrap_or(record.position);
        let equipment_id = spawn
            .and_then(|spawn| spawn.equipment_id)
            .unwrap_or(record.selected_equipment_id);
        let original_equipment_id = spawn
            .and_then(|spawn| spawn.original_equipment_id)
            .unwrap_or(record.selected_original_equipment_id);

        self.unit.world_mut().object_mut().create(record.guid);
        let _ = self.unit.world_mut().set_map(map_id, instance_id);
        self.unit.world_mut().relocate(position);
        self.unit.world_mut().set_name(template.name.clone());

        self.unit.world_mut().object_mut().set_entry(record.entry);
        self.original_entry = template.original_entry;
        self.unit.world_mut().object_mut().set_scale(template.scale);
        self.unit.set_race(0);
        self.unit.set_class(template.unit_class);
        self.set_faction(template.faction);
        self.set_display_id(
            record.selected_display_id,
            true,
            record
                .selected_model_dimensions
                .or(template.model_dimensions),
        );
        self.spells = template.spells;
        self.equipment_id = equipment_id;
        self.original_equipment_id = original_equipment_id;
        self.default_movement_type = spawn
            .map(|spawn| spawn.movement_type)
            .unwrap_or(template.movement_type);
        self.set_corpse_delay(record.corpse_delay, record.ignore_corpse_decay_ratio);
        if let Some(spawn) = spawn {
            self.apply_spawn_lifecycle(spawn);
        }

        self.unit.set_level(record.selected_level);
        self.set_power_type(record.stats.power_type);
        self.unit.set_max_health(record.stats.max_health);
        self.unit.set_health(record.stats.health);
        self.unit
            .set_max_power(PowerType::Mana, record.stats.max_mana);
        self.unit.set_power(PowerType::Mana, record.stats.mana);

        self.lifecycle_metadata = CreatureLifecycleMetadata {
            template_entry: template.entry,
            original_entry: template.original_entry,
            difficulty_id: template.difficulty_id,
            unit_class: template.unit_class,
            classification: template.classification,
            flags_extra: template.flags_extra,
            type_flags: template.type_flags,
            selected_level: record.selected_level,
            selected_display_id: record.selected_display_id,
            selected_model_dimensions: record
                .selected_model_dimensions
                .or(template.model_dimensions),
            template_scale: template.scale,
            speed_walk: template.speed_walk,
            speed_run: template.speed_run,
            spawn_id: spawn.map(|spawn| spawn.spawn_id).unwrap_or(0),
            spawn_map_id: map_id,
            spawn_instance_id: instance_id,
            spawn_position: position,
            home_position,
            phase_id: spawn.and_then(|spawn| spawn.phase_id),
            phase_group: spawn.and_then(|spawn| spawn.phase_group),
            terrain_swap_map: spawn.and_then(|spawn| spawn.terrain_swap_map),
            spawn_group_id: spawn.and_then(|spawn| spawn.spawn_group_id),
            spawn_group_name: spawn.and_then(|spawn| spawn.spawn_group_name.clone()),
            pool_id: spawn.and_then(|spawn| spawn.pool_id),
            string_id: spawn.and_then(|spawn| spawn.string_id.clone()),
            is_spawn_active: spawn.map(|spawn| spawn.is_active).unwrap_or(true),
            inactive_by_spawn_group: spawn
                .map(|spawn| spawn.inactive_by_spawn_group)
                .unwrap_or(false),
            duplicate_spawn_found: spawn
                .map(|spawn| spawn.duplicate_spawn_found)
                .unwrap_or(false),
            add_to_map_requested: spawn.map(|spawn| spawn.add_to_map).unwrap_or(false),
            map_insertion_requested: spawn.map(|spawn| spawn.add_to_map).unwrap_or(false),
            dynamic_spawn: record.dynamic,
            vehicle_id: record.vehicle_id,
            equipment_id,
            original_equipment_id,
        };

        self.clear_data_changes();
    }

    pub fn apply_load_from_db_lifecycle(&mut self, spawn: &CreatureSpawnLifecycleRecord) {
        self.apply_spawn_lifecycle(spawn);
        self.lifecycle_metadata.spawn_id = spawn.spawn_id;
        self.lifecycle_metadata.spawn_map_id = spawn.map_id;
        self.lifecycle_metadata.spawn_instance_id = spawn.instance_id;
        self.lifecycle_metadata.spawn_position = spawn.position;
        self.lifecycle_metadata.home_position = spawn.home_position;
        self.lifecycle_metadata.phase_id = spawn.phase_id;
        self.lifecycle_metadata.phase_group = spawn.phase_group;
        self.lifecycle_metadata.terrain_swap_map = spawn.terrain_swap_map;
        self.lifecycle_metadata.spawn_group_id = spawn.spawn_group_id;
        self.lifecycle_metadata.spawn_group_name = spawn.spawn_group_name.clone();
        self.lifecycle_metadata.pool_id = spawn.pool_id;
        self.lifecycle_metadata.string_id = spawn.string_id.clone();
        self.lifecycle_metadata.is_spawn_active = spawn.is_active;
        self.lifecycle_metadata.inactive_by_spawn_group = spawn.inactive_by_spawn_group;
        self.lifecycle_metadata.duplicate_spawn_found = spawn.duplicate_spawn_found;
        self.lifecycle_metadata.add_to_map_requested = spawn.add_to_map;
        self.lifecycle_metadata.map_insertion_requested = spawn.add_to_map;
        if let Some(equipment_id) = spawn.equipment_id {
            self.lifecycle_metadata.equipment_id = equipment_id;
        }
        if let Some(original_equipment_id) = spawn.original_equipment_id {
            self.lifecycle_metadata.original_equipment_id = original_equipment_id;
        }
        self.clear_data_changes();
    }

    fn apply_spawn_lifecycle(&mut self, spawn: &CreatureSpawnLifecycleRecord) {
        self.set_spawn_id(spawn.spawn_id);
        self.set_respawn_compatibility_mode(spawn.respawn_compatibility_mode);
        self.wander_distance = spawn.wander_distance;
        self.set_respawn_delay(spawn.respawn_delay);
        self.set_respawn_time(spawn.respawn_time);
        self.default_movement_type = spawn.movement_type;
        if let Some(equipment_id) = spawn.equipment_id {
            self.equipment_id = equipment_id;
        }
        if let Some(original_equipment_id) = spawn.original_equipment_id {
            self.original_equipment_id = original_equipment_id;
        }
        let _ = self
            .unit
            .world_mut()
            .set_map(spawn.map_id, spawn.instance_id);
        self.unit.world_mut().relocate(spawn.position);
    }

    pub const fn lifecycle_metadata(&self) -> &CreatureLifecycleMetadata {
        &self.lifecycle_metadata
    }

    pub fn clear_data_changes(&mut self) {
        self.unit.clear_unit_data_changes();
        self.unit.world_mut().object_mut().clear_update_mask(false);
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

    pub fn set_spell(&mut self, slot: usize, spell_id: u32) {
        if slot < MAX_CREATURE_SPELLS {
            self.spells[slot] = spell_id;
        }
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

    pub const fn cleanup_before_delete_count(&self) -> u32 {
        self.grid_unload_cleanup_before_delete_count
    }

    pub const fn grid_unload_delete_requested(&self) -> bool {
        self.grid_unload_delete_requested
    }

    pub const fn grid_unload_respawn_relocation_requested(&self) -> bool {
        self.grid_unload_respawn_relocation_requested
    }

    pub fn set_destroyed_object(&mut self, destroyed: bool) {
        self.unit
            .world_mut()
            .object_mut()
            .set_destroyed_object(destroyed);
    }

    /// Rust placeholder for TrinityCore `Creature::RemoveAllDynObjects`.
    ///
    /// Dynamic-object ownership is not represented on canonical `Creature` yet,
    /// so the grid unload bridge can call this safely as an explicit no-op.
    pub fn remove_all_dyn_objects(&mut self) {}

    /// Rust placeholder for TrinityCore `Creature::RemoveAllAreaTriggers`.
    ///
    /// Area-trigger ownership is not represented on canonical `Creature` yet,
    /// so the grid unload bridge can call this safely as an explicit no-op.
    pub fn remove_all_area_triggers(&mut self) {}

    pub fn combat_stop(&mut self) {
        self.unit.set_attacking(None);
    }

    pub fn request_respawn_relocation_from_grid_unload(&mut self) {
        self.grid_unload_respawn_relocation_requested = true;
    }

    pub fn cleanup_before_delete(&mut self) {
        self.grid_unload_cleanup_before_delete_count = self
            .grid_unload_cleanup_before_delete_count
            .saturating_add(1);
    }

    pub fn request_delete_from_grid_unload(&mut self) {
        self.grid_unload_delete_requested = true;
        self.unit.world_mut().clear_current_cell();
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
        assert_eq!(creature.cleanup_before_delete_count(), 0);
        assert!(!creature.grid_unload_delete_requested());
        assert!(!creature.grid_unload_respawn_relocation_requested());
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

    #[test]
    fn creature_grid_unload_helpers_apply_represented_state() {
        let victim = wow_core::ObjectGuid::new(1, 2);
        let mut creature = Creature::new(false);
        creature.unit_mut().set_attacking(Some(victim));
        creature.unit_mut().world_mut().set_current_cell(7, 8);

        creature.set_destroyed_object(true);
        creature.remove_all_dyn_objects();
        creature.remove_all_area_triggers();
        creature.combat_stop();
        creature.request_respawn_relocation_from_grid_unload();
        creature.cleanup_before_delete();
        creature.request_delete_from_grid_unload();

        assert!(creature.unit().world().object().is_destroyed_object());
        assert_eq!(creature.unit().attacking(), None);
        assert!(creature.grid_unload_respawn_relocation_requested());
        assert_eq!(creature.cleanup_before_delete_count(), 1);
        assert!(creature.grid_unload_delete_requested());
        assert_eq!(creature.unit().world().current_cell(), None);
        assert!(!creature.unit().world().object().is_in_grid());
    }

    fn creature_lifecycle_template() -> CreatureTemplateLifecycleRecord {
        let mut spells = [0; MAX_CREATURE_SPELLS];
        spells[0] = 133;
        spells[3] = 116;
        CreatureTemplateLifecycleRecord {
            entry: 1001,
            original_entry: 9001,
            difficulty_id: 2,
            name: "lifecycle wolf".to_string(),
            unit_class: 1,
            faction: 14,
            display_id: 2001,
            model_dimensions: Some(CreatureModelDimensions {
                bounding_radius: 0.4,
                combat_reach: 1.2,
            }),
            scale: 1.5,
            speed_walk: 1.0,
            speed_run: 1.14286,
            spells,
            classification: 3,
            flags_extra: 0x10,
            type_flags: 0x20,
            movement_type: MovementGeneratorType::Idle,
            min_level: 70,
            max_level: 72,
            equipment_id: 4,
            original_equipment_id: -4,
        }
    }

    fn creature_lifecycle_spawn() -> CreatureSpawnLifecycleRecord {
        CreatureSpawnLifecycleRecord {
            spawn_id: 44_000,
            map_id: 571,
            instance_id: 3,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            home_position: Position::new(5.0, 6.0, 7.0, 1.0),
            phase_id: Some(169),
            phase_group: Some(12),
            terrain_swap_map: Some(609),
            spawn_group_id: Some(77),
            spawn_group_name: Some("lifecycle group".to_string()),
            pool_id: Some(88),
            equipment_id: Some(9),
            original_equipment_id: Some(-9),
            wander_distance: 12.5,
            respawn_delay: 45,
            respawn_time: 123_456,
            movement_type: MovementGeneratorType::Idle,
            string_id: Some("creature-string".to_string()),
            is_active: false,
            inactive_by_spawn_group: true,
            duplicate_spawn_found: true,
            add_to_map: true,
            respawn_compatibility_mode: true,
        }
    }

    fn creature_lifecycle_create_record() -> CreatureCreateLifecycleRecord {
        CreatureCreateLifecycleRecord {
            guid: ObjectGuid::new(8, 1001),
            entry: 1001,
            map_id: 571,
            instance_id: 3,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            dynamic: false,
            vehicle_id: Some(101),
            template: creature_lifecycle_template(),
            spawn: None,
            selected_level: 71,
            stats: CreatureLifecycleStats::new(5_000, 4_500, 1_000, 750),
            selected_display_id: 3001,
            selected_model_dimensions: Some(CreatureModelDimensions {
                bounding_radius: 0.5,
                combat_reach: 2.0,
            }),
            selected_equipment_id: 6,
            selected_original_equipment_id: -6,
            corpse_delay: 90,
            ignore_corpse_decay_ratio: true,
        }
    }

    #[test]
    fn creature_lifecycle_create_applies_template_stats_and_clean_baseline() {
        let creature = Creature::create_from_lifecycle(creature_lifecycle_create_record());

        assert_eq!(
            creature.unit().world().object().guid(),
            ObjectGuid::new(8, 1001)
        );
        assert_eq!(creature.unit().world().object().entry(), 1001);
        assert_eq!(creature.unit().world().map_id(), 571);
        assert_eq!(creature.unit().world().instance_id(), 3);
        assert_eq!(
            creature.unit().world().position(),
            Position::new(1.0, 2.0, 3.0, 4.0)
        );
        assert_eq!(creature.unit().data().race, 0);
        assert_eq!(creature.unit().data().class_id, 1);
        assert_eq!(creature.unit().data().faction_template, 14);
        assert_eq!(creature.unit().data().display_id, 3001);
        assert_eq!(creature.unit().data().native_display_id, 3001);
        assert_eq!(creature.unit().world().object().scale(), 1.5);
        assert_eq!(
            creature.unit().data().bounding_radius,
            0.5 * 1.5 * crate::DEFAULT_PLAYER_DISPLAY_SCALE
        );
        assert_eq!(
            creature.unit().data().combat_reach,
            2.0 * 1.5 * crate::DEFAULT_PLAYER_DISPLAY_SCALE
        );
        assert_eq!(creature.spells()[0], 133);
        assert_eq!(creature.spells()[3], 116);
        assert_eq!(creature.equipment_id(), 6);
        assert_eq!(creature.original_equipment_id(), -6);
        assert_eq!(creature.unit().data().level, 71);
        assert_eq!(creature.unit().data().max_health, 5_000);
        assert_eq!(creature.unit().data().health, 4_500);
        assert_eq!(creature.unit().get_max_power(PowerType::Mana), 1_000);
        assert_eq!(creature.unit().get_power(PowerType::Mana), 750);
        assert_eq!(creature.corpse_delay(), 90);
        assert!(creature.ignore_corpse_decay_ratio());
        assert_eq!(creature.lifecycle_metadata().template_entry, 1001);
        assert_eq!(creature.lifecycle_metadata().original_entry, 9001);
        assert_eq!(creature.lifecycle_metadata().difficulty_id, 2);
        assert_eq!(creature.lifecycle_metadata().classification, 3);
        assert_eq!(creature.unit().changed_object_type_mask(), 0);
    }

    #[test]
    fn creature_lifecycle_load_from_db_applies_spawn_bridge_state() {
        let create = creature_lifecycle_create_record();
        let spawn = creature_lifecycle_spawn();
        let creature =
            Creature::load_from_db_lifecycle(CreatureLoadFromDbLifecycleRecord { create, spawn });

        assert_eq!(creature.spawn_id(), 44_000);
        assert_eq!(creature.wander_distance(), 12.5);
        assert_eq!(creature.respawn_delay(), 45);
        assert_eq!(creature.respawn_time(), 123_456);
        assert_eq!(
            creature.default_movement_type(),
            MovementGeneratorType::Idle
        );
        assert_eq!(creature.unit().world().map_id(), 571);
        assert_eq!(creature.unit().world().instance_id(), 3);
        assert_eq!(
            creature.unit().world().position(),
            Position::new(1.0, 2.0, 3.0, 4.0)
        );
        assert!(creature.respawn_compatibility_mode());
        assert_eq!(creature.equipment_id(), 9);
        assert_eq!(creature.original_equipment_id(), -9);
        let metadata = creature.lifecycle_metadata();
        assert_eq!(metadata.home_position, Position::new(5.0, 6.0, 7.0, 1.0));
        assert_eq!(metadata.phase_id, Some(169));
        assert_eq!(metadata.terrain_swap_map, Some(609));
        assert_eq!(metadata.spawn_group_id, Some(77));
        assert_eq!(
            metadata.spawn_group_name.as_deref(),
            Some("lifecycle group")
        );
        assert_eq!(metadata.string_id.as_deref(), Some("creature-string"));
        assert!(metadata.add_to_map_requested);
        assert!(metadata.map_insertion_requested);
        assert!(metadata.duplicate_spawn_found);
        assert!(!metadata.is_spawn_active);
        assert!(metadata.inactive_by_spawn_group);
        assert_eq!(creature.unit().changed_object_type_mask(), 0);
    }

    #[test]
    fn creature_lifecycle_health_is_clamped_to_max_health() {
        let mut record = creature_lifecycle_create_record();
        record.stats.max_health = 100;
        record.stats.health = 150;

        let creature = Creature::create_from_lifecycle(record);

        assert_eq!(creature.unit().data().max_health, 100);
        assert_eq!(creature.unit().data().health, 100);
    }

    #[test]
    fn creature_lifecycle_plan_preserves_trinity_critical_order() {
        let plan = CreatureLifecyclePlan::trinity_create_load_from_db();

        assert!(plan.occurs_before(
            CreatureLifecycleStep::LookupTemplateAndDifficulty,
            CreatureLifecycleStep::InitEntryAndCreateFromProto
        ));
        assert!(plan.occurs_before(
            CreatureLifecycleStep::RelocateAndValidatePosition,
            CreatureLifecycleStep::InitEntryAndCreateFromProto
        ));
        assert!(plan.occurs_before(
            CreatureLifecycleStep::SelectLevel,
            CreatureLifecycleStep::UpdateLevelDependantStats
        ));
        assert!(plan.occurs_before(
            CreatureLifecycleStep::UpdateLevelDependantStats,
            CreatureLifecycleStep::AddToMap
        ));
        assert_eq!(plan.steps().last(), Some(&CreatureLifecycleStep::AddToMap));
    }

    #[test]
    fn creature_lifecycle_create_with_spawn_cleans_object_and_unit_masks() {
        let mut record = creature_lifecycle_create_record();
        record.spawn = Some(creature_lifecycle_spawn());

        let creature = Creature::create_from_lifecycle(record);

        assert_eq!(creature.unit().values_update().changed_object_type_mask, 0);
        assert_eq!(
            creature
                .unit()
                .world()
                .object()
                .values_update()
                .changed_object_type_mask,
            0
        );
        assert_eq!(creature.spawn_id(), 44_000);
    }
}
