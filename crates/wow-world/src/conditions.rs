// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Runtime side of C++ `ConditionMgr` evaluation context.

use num_traits::FromPrimitive;
use wow_constants::MAX_CONDITION_TARGETS;
use wow_constants::{
    ComparisonType, ConditionSourceType, ConditionType, RelationType, TypeId, TypeMask,
    UnitStandStateType,
};
use wow_data::{Condition, ConditionEntriesByTypeStore, ConditionId};
use wow_entities::WorldObject;
use wow_loot::{LootStoreItemContext, condition_source_type_for_loot_store_kind_like_cpp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConditionMapRef {
    pub map_id: u32,
    pub instance_id: u32,
}

impl ConditionMapRef {
    pub const fn new(map_id: u32, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
        }
    }
}

#[derive(Debug)]
pub struct ConditionSourceInfo<'a> {
    pub condition_targets: [Option<&'a WorldObject>; MAX_CONDITION_TARGETS],
    pub unit_targets: [Option<ConditionUnitSnapshot>; MAX_CONDITION_TARGETS],
    pub player_targets: [Option<ConditionPlayerSnapshot>; MAX_CONDITION_TARGETS],
    pub spawn_id_targets: [Option<u64>; MAX_CONDITION_TARGETS],
    pub private_object_targets: [bool; MAX_CONDITION_TARGETS],
    pub string_id_targets: [Option<&'a [&'a str]>; MAX_CONDITION_TARGETS],
    pub condition_map: Option<ConditionMapRef>,
    pub last_failed_condition: Option<&'a Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionUnitSnapshot {
    pub level: u32,
    pub health: u64,
    pub max_health: u64,
    pub class_mask: u32,
    pub race: u8,
    pub creature_type: Option<u32>,
    pub is_alive: bool,
    pub is_charmed: bool,
    pub in_water: bool,
    pub unit_state: u32,
    pub stand_state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionPlayerSnapshot {
    pub team: u32,
    pub native_gender: u32,
    pub drunken_state: u32,
    pub can_be_game_master: bool,
    pub is_game_master: bool,
    pub pet_type: Option<u32>,
    pub is_in_flight: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionMeetResult {
    Evaluated(bool),
    Unsupported,
}

impl ConditionMeetResult {
    pub const fn value(self) -> Option<bool> {
        match self {
            Self::Evaluated(value) => Some(value),
            Self::Unsupported => None,
        }
    }
}

impl<'a> ConditionSourceInfo<'a> {
    /// C++ `ConditionSourceInfo(WorldObject const*, WorldObject const*, WorldObject const*)`.
    pub fn from_targets(
        target0: Option<&'a WorldObject>,
        target1: Option<&'a WorldObject>,
        target2: Option<&'a WorldObject>,
    ) -> Self {
        let condition_targets = [target0, target1, target2];
        let condition_map = condition_targets
            .iter()
            .flatten()
            .next()
            .map(|target| ConditionMapRef::new(target.map_id(), target.instance_id()));

        Self {
            condition_targets,
            unit_targets: [None; MAX_CONDITION_TARGETS],
            player_targets: [None; MAX_CONDITION_TARGETS],
            spawn_id_targets: [None; MAX_CONDITION_TARGETS],
            private_object_targets: [false; MAX_CONDITION_TARGETS],
            string_id_targets: [None; MAX_CONDITION_TARGETS],
            condition_map,
            last_failed_condition: None,
        }
    }

    /// C++ `ConditionSourceInfo(Map const*)`.
    pub const fn from_map(condition_map: ConditionMapRef) -> Self {
        Self {
            condition_targets: [None; MAX_CONDITION_TARGETS],
            unit_targets: [None; MAX_CONDITION_TARGETS],
            player_targets: [None; MAX_CONDITION_TARGETS],
            spawn_id_targets: [None; MAX_CONDITION_TARGETS],
            private_object_targets: [false; MAX_CONDITION_TARGETS],
            string_id_targets: [None; MAX_CONDITION_TARGETS],
            condition_map: Some(condition_map),
            last_failed_condition: None,
        }
    }

    pub fn set_unit_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionUnitSnapshot,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.unit_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_player_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionPlayerSnapshot,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.player_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_spawn_id_target_snapshot(&mut self, target_index: usize, spawn_id: u64) {
        if target_index < MAX_CONDITION_TARGETS {
            self.spawn_id_targets[target_index] = Some(spawn_id);
        }
    }

    pub fn set_private_object_target_snapshot(&mut self, target_index: usize, is_private: bool) {
        if target_index < MAX_CONDITION_TARGETS {
            self.private_object_targets[target_index] = is_private;
        }
    }

    pub fn set_string_id_target_snapshot(
        &mut self,
        target_index: usize,
        string_ids: &'a [&'a str],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.string_id_targets[target_index] = Some(string_ids);
        }
    }

    pub fn mark_failed_like_cpp(&mut self, condition: &'a Condition) {
        self.last_failed_condition = Some(condition);
    }
}

fn compare_values_u64_like_cpp(comparison_type: u32, left: u64, right: u64) -> bool {
    match ComparisonType::from_u32(comparison_type) {
        Some(ComparisonType::Eq) => left == right,
        Some(ComparisonType::High) => left > right,
        Some(ComparisonType::Low) => left < right,
        Some(ComparisonType::HighEq) => left >= right,
        Some(ComparisonType::LowEq) => left <= right,
        _ => false,
    }
}

fn compare_values_f32_like_cpp(comparison_type: u32, left: f32, right: f32) -> bool {
    match ComparisonType::from_u32(comparison_type) {
        Some(ComparisonType::Eq) => left == right,
        Some(ComparisonType::High) => left > right,
        Some(ComparisonType::Low) => left < right,
        Some(ComparisonType::HighEq) => left >= right,
        Some(ComparisonType::LowEq) => left <= right,
        _ => false,
    }
}

fn is_player_object_like_cpp(object: &WorldObject) -> bool {
    matches!(
        object.object().type_id(),
        TypeId::Player | TypeId::ActivePlayer
    )
}

fn is_unit_object_like_cpp(object: &WorldObject) -> bool {
    matches!(
        object.object().type_id(),
        TypeId::Unit | TypeId::Player | TypeId::ActivePlayer
    ) || object
        .object()
        .type_mask()
        .intersects(TypeMask::UNIT | TypeMask::PLAYER | TypeMask::ACTIVE_PLAYER)
}

fn unit_stand_state_is_sit_like_cpp(stand_state: u32) -> bool {
    stand_state == UnitStandStateType::Sit as u32
        || stand_state == UnitStandStateType::SitChair as u32
        || stand_state == UnitStandStateType::SitLowChair as u32
        || stand_state == UnitStandStateType::SitMediumChair as u32
        || stand_state == UnitStandStateType::SitHighChair as u32
}

fn unit_stand_state_is_stand_like_cpp(stand_state: u32) -> bool {
    !unit_stand_state_is_sit_like_cpp(stand_state)
        && stand_state != UnitStandStateType::Sleep as u32
        && stand_state != UnitStandStateType::Kneel as u32
}

/// Ported subset of C++ `Condition::Meets`.
///
/// Returns `Unsupported` for condition types whose exact C++ state is not yet represented by
/// the Rust entity/DB2/runtime layers.
pub fn condition_meets_basic_like_cpp<'a>(
    condition: &'a Condition,
    source_info: &mut ConditionSourceInfo<'a>,
    mut is_in_area_like_cpp: impl FnMut(u32, u32) -> bool,
) -> ConditionMeetResult {
    if usize::from(condition.condition_target) >= MAX_CONDITION_TARGETS {
        return ConditionMeetResult::Evaluated(false);
    }

    let mut cond_meets = false;
    let mut needs_object = false;

    match condition.condition_type {
        ConditionType::None => cond_meets = true,
        ConditionType::MapId => {
            cond_meets = source_info
                .condition_map
                .is_some_and(|map| map.map_id == condition.condition_value1);
        }
        ConditionType::ActiveEvent
        | ConditionType::InstanceInfo
        | ConditionType::WorldState
        | ConditionType::RealmAchievement
        | ConditionType::DifficultyId
        | ConditionType::ScenarioStep => return ConditionMeetResult::Unsupported,
        _ => needs_object = true,
    }

    let target_index = usize::from(condition.condition_target);
    let object = source_info.condition_targets[target_index];
    if needs_object && object.is_none() {
        return ConditionMeetResult::Evaluated(false);
    }

    if let Some(object) = object {
        let unit = source_info.unit_targets[target_index];
        let player =
            source_info.player_targets[target_index].filter(|_| is_player_object_like_cpp(object));
        match condition.condition_type {
            ConditionType::ZoneId => cond_meets = object.zone_id() == condition.condition_value1,
            ConditionType::AreaId => {
                cond_meets = is_in_area_like_cpp(object.area_id(), condition.condition_value1);
            }
            ConditionType::Class => {
                if let Some(unit) = unit {
                    cond_meets = (unit.class_mask & condition.condition_value1) != 0;
                }
            }
            ConditionType::Team => {
                if let Some(player) = player {
                    cond_meets = player.team == condition.condition_value1;
                }
            }
            ConditionType::Race => {
                if let Some(unit) = unit {
                    cond_meets = unit.race != 0
                        && condition.condition_value1 & (1_u32 << u32::from(unit.race - 1)) != 0;
                }
            }
            ConditionType::Gender => {
                if let Some(player) = player {
                    cond_meets = player.native_gender == condition.condition_value1;
                }
            }
            ConditionType::Level => {
                if let Some(unit) = unit {
                    cond_meets = compare_values_u64_like_cpp(
                        condition.condition_value2,
                        u64::from(unit.level),
                        u64::from(condition.condition_value1),
                    );
                }
            }
            ConditionType::ObjectEntryGuid | ConditionType::ObjectEntryGuidLegacy => {
                let type_id = object.object().type_id();
                if type_id as u32 == condition.condition_value1 {
                    cond_meets = condition.condition_value2 == 0
                        || object.object().entry() == condition.condition_value2;

                    if condition.condition_value3 != 0
                        && matches!(type_id, TypeId::Unit | TypeId::GameObject)
                    {
                        cond_meets &=
                            source_info.spawn_id_targets[target_index].is_some_and(|spawn_id| {
                                spawn_id == u64::from(condition.condition_value3)
                            });
                    }
                }
            }
            ConditionType::TypeMask | ConditionType::TypeMaskLegacy => {
                cond_meets = object
                    .object()
                    .is_type(TypeMask::from_bits_truncate(condition.condition_value1));
            }
            ConditionType::DistanceTo => {
                if let Some(to_object) = usize::try_from(condition.condition_value1)
                    .ok()
                    .filter(|target| *target < MAX_CONDITION_TARGETS)
                    .and_then(|target| source_info.condition_targets[target])
                {
                    cond_meets = compare_values_f32_like_cpp(
                        condition.condition_value3,
                        object.distance(to_object),
                        condition.condition_value2 as f32,
                    );
                }
            }
            ConditionType::RelationTo => {
                if RelationType::from_u32(condition.condition_value2)
                    != Some(RelationType::SelfRelation)
                {
                    return ConditionMeetResult::Unsupported;
                }

                if let Some(to_object) = usize::try_from(condition.condition_value1)
                    .ok()
                    .filter(|target| *target < MAX_CONDITION_TARGETS)
                    .and_then(|target| source_info.condition_targets[target])
                    && is_unit_object_like_cpp(object)
                    && is_unit_object_like_cpp(to_object)
                {
                    cond_meets = std::ptr::eq(object, to_object);
                }
            }
            ConditionType::PhaseId => {
                cond_meets = object
                    .phase_shift()
                    .has_phase_like_cpp(condition.condition_value1);
            }
            ConditionType::TerrainSwap => {
                cond_meets = object
                    .phase_shift()
                    .has_visible_map_id_like_cpp(condition.condition_value1);
            }
            ConditionType::Alive => {
                if let Some(unit) = unit {
                    cond_meets = unit.is_alive;
                }
            }
            ConditionType::HpVal => {
                if let Some(unit) = unit {
                    cond_meets = compare_values_u64_like_cpp(
                        condition.condition_value2,
                        unit.health,
                        u64::from(condition.condition_value1),
                    );
                }
            }
            ConditionType::HpPct => {
                if let Some(unit) = unit {
                    let health_pct = if unit.max_health == 0 {
                        0.0
                    } else {
                        (unit.health as f32 / unit.max_health as f32) * 100.0
                    };
                    cond_meets = compare_values_f32_like_cpp(
                        condition.condition_value2,
                        health_pct,
                        condition.condition_value1 as f32,
                    );
                }
            }
            ConditionType::UnitState => {
                if let Some(unit) = unit {
                    cond_meets = (unit.unit_state & condition.condition_value1) != 0;
                }
            }
            ConditionType::InWater => {
                if let Some(unit) = unit {
                    cond_meets = unit.in_water;
                }
            }
            ConditionType::CreatureType => {
                if let Some(unit) = unit
                    && object.object().type_id() == TypeId::Unit
                    && let Some(creature_type) = unit.creature_type
                {
                    cond_meets = creature_type == condition.condition_value1;
                }
            }
            ConditionType::StandState => {
                if let Some(unit) = unit {
                    cond_meets = if condition.condition_value1 == 0 {
                        unit.stand_state == condition.condition_value2
                    } else if condition.condition_value2 == 0 {
                        unit_stand_state_is_stand_like_cpp(unit.stand_state)
                    } else if condition.condition_value2 == 1 {
                        unit_stand_state_is_sit_like_cpp(unit.stand_state)
                    } else {
                        false
                    };
                }
            }
            ConditionType::DrunkenState => {
                if let Some(player) = player {
                    cond_meets = player.drunken_state >= condition.condition_value1;
                }
            }
            ConditionType::GameMaster => {
                if let Some(player) = player {
                    cond_meets = if condition.condition_value1 == 1 {
                        player.can_be_game_master
                    } else {
                        player.is_game_master
                    };
                }
            }
            ConditionType::PetType => {
                if let Some(player) = player
                    && let Some(pet_type) = player.pet_type
                {
                    cond_meets = ((1_u32 << pet_type) & condition.condition_value1) != 0;
                }
            }
            ConditionType::Taxi => {
                if let Some(player) = player {
                    cond_meets = player.is_in_flight;
                }
            }
            ConditionType::Charmed => {
                if let Some(unit) = unit {
                    cond_meets = unit.is_charmed;
                }
            }
            ConditionType::PrivateObject => {
                cond_meets = source_info.private_object_targets[target_index];
            }
            ConditionType::StringId => {
                if matches!(object.object().type_id(), TypeId::Unit | TypeId::GameObject)
                    && let Some(string_ids) = source_info.string_id_targets[target_index]
                {
                    cond_meets = string_ids
                        .iter()
                        .any(|string_id| *string_id == condition.condition_string_value1.as_str());
                }
            }
            ConditionType::None | ConditionType::MapId => {}
            _ => return ConditionMeetResult::Unsupported,
        }
    }

    if condition.negative_condition {
        cond_meets = !cond_meets;
    }

    if !cond_meets {
        source_info.mark_failed_like_cpp(condition);
    }

    ConditionMeetResult::Evaluated(cond_meets)
}

/// C++ `ConditionMgr::IsObjectMeetToConditions`.
pub fn is_object_meet_to_conditions_like_cpp<'a>(
    source_info: &mut ConditionSourceInfo<'a>,
    conditions: &'a [Condition],
    condition_store: &'a ConditionEntriesByTypeStore,
    mut meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if conditions.is_empty() {
        return true;
    }

    is_object_meet_to_condition_list_like_cpp(source_info, conditions, condition_store, &mut meets)
}

fn is_object_meet_to_condition_list_like_cpp<'a, F>(
    source_info: &mut ConditionSourceInfo<'a>,
    conditions: &'a [Condition],
    condition_store: &'a ConditionEntriesByTypeStore,
    meets: &mut F,
) -> bool
where
    F: FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
{
    let mut else_group_store = std::collections::BTreeMap::<u32, bool>::new();

    for condition in conditions {
        if !condition.is_loaded_like_cpp() {
            continue;
        }

        let group_passed = else_group_store.entry(condition.else_group).or_insert(true);
        if !*group_passed {
            continue;
        }

        if condition.reference_id != 0 {
            if let Some(reference_conditions) = condition_store.conditions_for_like_cpp(
                ConditionSourceType::ReferenceCondition,
                ConditionId::new(condition.reference_id, 0, 0),
            ) && !is_object_meet_to_condition_list_like_cpp(
                source_info,
                reference_conditions.as_slice(),
                condition_store,
                meets,
            ) {
                *group_passed = false;
            }
        } else if !meets(condition, source_info) {
            *group_passed = false;
        }
    }

    else_group_store.values().any(|passed| *passed)
}

/// C++ `ConditionMgr::IsObjectMeetingNotGroupedConditions`.
pub fn is_object_meeting_not_grouped_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    source_type: ConditionSourceType,
    entry: u32,
    source_info: &mut ConditionSourceInfo<'a>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if (source_type as u32) > ConditionSourceType::None as u32
        && (source_type as u32) < ConditionSourceType::Max as u32
    {
        if let Some(conditions) = condition_store
            .conditions_for_like_cpp(source_type, ConditionId::new(0, entry as i32, 0))
        {
            return is_object_meet_to_conditions_like_cpp(
                source_info,
                conditions.as_slice(),
                condition_store,
                meets,
            );
        }
    }

    true
}

/// C++ `ConditionMgr::HasConditionsForNotGroupedEntry`.
pub fn has_conditions_for_not_grouped_entry_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    source_type: ConditionSourceType,
    entry: u32,
) -> bool {
    (source_type as u32) > ConditionSourceType::None as u32
        && (source_type as u32) < ConditionSourceType::Max as u32
        && condition_store
            .conditions_for_like_cpp(source_type, ConditionId::new(0, entry as i32, 0))
            .is_some()
}

/// C++ `ConditionMgr::IsObjectMeetingSpellClickConditions`.
pub fn is_object_meeting_spell_click_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
    clicker: Option<&'a WorldObject>,
    target: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::SpellClickEvent,
        ConditionId::new(creature_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(clicker, target, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::HasConditionsForSpellClickEvent`.
pub fn has_conditions_for_spell_click_event_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
) -> bool {
    condition_store
        .conditions_for_like_cpp(
            ConditionSourceType::SpellClickEvent,
            ConditionId::new(creature_id, spell_id as i32, 0),
        )
        .is_some()
}

/// C++ `ConditionMgr::IsObjectMeetingVehicleSpellConditions`.
pub fn is_object_meeting_vehicle_spell_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
    player: Option<&'a WorldObject>,
    vehicle: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::VehicleSpell,
        ConditionId::new(creature_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, vehicle, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingSmartEventConditions`.
pub fn is_object_meeting_smart_event_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    entry_or_guid: i64,
    event_id: u32,
    source_type: u32,
    unit: Option<&'a WorldObject>,
    base_object: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::SmartEvent,
        ConditionId::new(event_id + 1, entry_or_guid as i32, source_type),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(unit, base_object, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingVendorItemConditions`.
pub fn is_object_meeting_vendor_item_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    item_id: u32,
    player: Option<&'a WorldObject>,
    vendor: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::NpcVendor,
        ConditionId::new(creature_id, item_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, vendor, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::GetConditionsForAreaTrigger`.
pub fn conditions_for_area_trigger_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    area_trigger_id: u32,
    is_server_side: bool,
) -> Option<&[Condition]> {
    condition_store
        .conditions_for_like_cpp(
            ConditionSourceType::AreaTrigger,
            ConditionId::new(area_trigger_id, i32::from(is_server_side), 0),
        )
        .map(|conditions| conditions.as_slice())
}

/// C++ `ConditionMgr::IsObjectMeetingTrainerSpellConditions`.
pub fn is_object_meeting_trainer_spell_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    trainer_id: u32,
    spell_id: u32,
    player: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::TrainerSpell,
        ConditionId::new(trainer_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingVisibilityByObjectIdConditions`.
pub fn is_object_meeting_visibility_by_object_id_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    object_type: u32,
    entry: u32,
    seer: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::ObjectIdVisibility,
        ConditionId::new(object_type, entry as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(seer, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `LootTemplate::LinkConditions` + `LootItem::AllowedForPlayer` condition check.
///
/// Rust keeps loot items immutable and passes `LootStoreItemContext` during fill; this resolves the
/// same condition bucket C++ links into `LootStoreItem::conditions` and evaluates it with the looter
/// as target0.
pub fn is_loot_store_item_meeting_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    context: LootStoreItemContext,
    looter: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    let Some(source_type) = ConditionSourceType::from_u32(
        condition_source_type_for_loot_store_kind_like_cpp(context.store_kind) as u32,
    ) else {
        return true;
    };

    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        source_type,
        ConditionId::new(context.entry, context.item.item_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(looter, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{ConditionType, PhaseFlags, TypeId, TypeMask};
    use wow_core::Position;
    use wow_loot::{LootStoreItem, LootStoreItemContext, LootStoreKind};

    fn world_object(map_id: u32, instance_id: u32) -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        object.set_map(map_id, instance_id).unwrap();
        object
    }

    fn player_object(map_id: u32, instance_id: u32) -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Player, TypeMask::PLAYER | TypeMask::UNIT);
        object.set_map(map_id, instance_id).unwrap();
        object
    }

    #[test]
    fn condition_source_info_uses_first_non_null_target_map_like_cpp() {
        let target1 = world_object(571, 2);
        let target2 = world_object(1, 9);

        let info = ConditionSourceInfo::from_targets(None, Some(&target1), Some(&target2));

        assert_eq!(info.condition_targets[0].map(WorldObject::map_id), None);
        assert_eq!(
            info.condition_targets[1].map(WorldObject::map_id),
            Some(571)
        );
        assert_eq!(info.condition_map, Some(ConditionMapRef::new(571, 2)));
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn condition_source_info_map_constructor_matches_cpp() {
        let info = ConditionSourceInfo::from_map(ConditionMapRef::new(530, 7));

        assert!(info.condition_targets.iter().all(Option::is_none));
        assert_eq!(info.condition_map, Some(ConditionMapRef::new(530, 7)));
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn condition_source_info_tracks_last_failed_condition_like_cpp() {
        let condition = Condition::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        info.mark_failed_like_cpp(&condition);

        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &condition
        ));
    }

    #[test]
    fn basic_condition_meets_map_zone_area_and_negative_like_cpp() {
        let mut target = world_object(571, 2);
        target.set_zone_and_area(67, 123);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);

        let map_condition = Condition {
            condition_type: ConditionType::MapId,
            condition_value1: 571,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&map_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let zone_condition = Condition {
            condition_type: ConditionType::ZoneId,
            condition_value1: 67,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&zone_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let area_condition = Condition {
            condition_type: ConditionType::AreaId,
            condition_value1: 999,
            negative_condition: true,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&area_condition, &mut info, |current, required| {
                current == 123 && required == 999
            }),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &area_condition
        ));
    }

    #[test]
    fn basic_condition_meets_object_type_entry_mask_and_phasing_like_cpp() {
        let mut target = world_object(571, 2);
        target.object_mut().set_entry(1001);
        target
            .phase_shift_mut()
            .add_phase_like_cpp(55, PhaseFlags::NONE, 1);
        target.phase_shift_mut().add_visible_map_id_like_cpp(609, 1);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_spawn_id_target_snapshot(0, 42);
        info.set_private_object_target_snapshot(0, true);
        info.set_string_id_target_snapshot(0, &["template-id", "spawn-id", "script-id"]);

        let entry_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&entry_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let spawn_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            condition_value3: 42,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&spawn_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let private_condition = Condition {
            condition_type: ConditionType::PrivateObject,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&private_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let string_id_condition = Condition {
            condition_type: ConditionType::StringId,
            condition_string_value1: "spawn-id".to_string(),
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&string_id_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let mask_condition = Condition {
            condition_type: ConditionType::TypeMask,
            condition_value1: TypeMask::UNIT.bits(),
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&mask_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let phase_condition = Condition {
            condition_type: ConditionType::PhaseId,
            condition_value1: 55,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&phase_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let terrain_condition = Condition {
            condition_type: ConditionType::TerrainSwap,
            condition_value1: 609,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&terrain_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );
    }

    #[test]
    fn basic_condition_meets_distance_to_uses_cpp_target_and_combat_reach_distance() {
        let mut target0 = world_object(571, 2);
        let mut target1 = world_object(571, 2);
        target0.relocate(Position::xyz(0.0, 0.0, 0.0));
        target1.relocate(Position::xyz(3.0, 4.0, 0.0));
        target0.set_combat_reach(1.0);
        target1.set_combat_reach(1.0);

        let mut info = ConditionSourceInfo::from_targets(Some(&target0), Some(&target1), None);
        let condition = Condition {
            condition_type: ConditionType::DistanceTo,
            condition_value1: 1,
            condition_value2: 3,
            condition_value3: ComparisonType::LowEq as u32,
            ..Condition::default()
        };

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );
    }

    #[test]
    fn basic_condition_meets_relation_self_like_cpp() {
        let target = world_object(571, 2);
        let other = world_object(571, 2);
        let self_condition = Condition {
            condition_type: ConditionType::RelationTo,
            condition_value1: 1,
            condition_value2: RelationType::SelfRelation as u32,
            ..Condition::default()
        };

        let mut same_info = ConditionSourceInfo::from_targets(Some(&target), Some(&target), None);
        assert_eq!(
            condition_meets_basic_like_cpp(&self_condition, &mut same_info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let mut other_info = ConditionSourceInfo::from_targets(Some(&target), Some(&other), None);
        assert_eq!(
            condition_meets_basic_like_cpp(&self_condition, &mut other_info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );

        let party_condition = Condition {
            condition_type: ConditionType::RelationTo,
            condition_value1: 1,
            condition_value2: RelationType::InParty as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&party_condition, &mut other_info, |_, _| false),
            ConditionMeetResult::Unsupported
        );
    }

    #[test]
    fn basic_condition_meets_unit_snapshot_class_race_level_and_life_like_cpp() {
        let target = world_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 70,
                health: 750,
                max_health: 1000,
                class_mask: 1 << (2 - 1),
                race: 4,
                creature_type: Some(7),
                is_alive: true,
                is_charmed: true,
                in_water: true,
                unit_state: 0x20,
                stand_state: 8,
            },
        );

        let conditions = vec![
            Condition {
                condition_type: ConditionType::Class,
                condition_value1: 1 << (2 - 1),
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Race,
                condition_value1: 1 << (4 - 1),
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Level,
                condition_value1: 60,
                condition_value2: ComparisonType::HighEq as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Alive,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::HpVal,
                condition_value1: 700,
                condition_value2: ComparisonType::High as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::HpPct,
                condition_value1: 75,
                condition_value2: ComparisonType::Eq as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::UnitState,
                condition_value1: 0x20,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::InWater,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::CreatureType,
                condition_value1: 7,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Charmed,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::StandState,
                condition_value1: 0,
                condition_value2: UnitStandStateType::Kneel as u32,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }
    }

    #[test]
    fn basic_condition_meets_player_snapshot_branches_like_cpp() {
        let target = player_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_player_target_snapshot(
            0,
            ConditionPlayerSnapshot {
                team: 469,
                native_gender: 1,
                drunken_state: 2,
                can_be_game_master: true,
                is_game_master: false,
                pet_type: Some(2),
                is_in_flight: true,
            },
        );

        let conditions = vec![
            Condition {
                condition_type: ConditionType::Team,
                condition_value1: 469,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Gender,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::DrunkenState,
                condition_value1: 2,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::GameMaster,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::PetType,
                condition_value1: 1 << 2,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Taxi,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }
    }

    #[test]
    fn basic_condition_meets_stand_state_modes_like_cpp() {
        let target = world_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 1,
                health: 1,
                max_health: 1,
                class_mask: 1,
                race: 1,
                creature_type: None,
                is_alive: true,
                is_charmed: false,
                in_water: false,
                unit_state: 0,
                stand_state: UnitStandStateType::SitChair as u32,
            },
        );

        let sit_mode = Condition {
            condition_type: ConditionType::StandState,
            condition_value1: 1,
            condition_value2: 1,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&sit_mode, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let stand_mode = Condition {
            condition_type: ConditionType::StandState,
            condition_value1: 1,
            condition_value2: 0,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&stand_mode, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_creature_type_requires_creature_like_cpp() {
        let target = player_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 1,
                health: 1,
                max_health: 1,
                class_mask: 1,
                race: 1,
                creature_type: Some(7),
                is_alive: true,
                is_charmed: false,
                in_water: false,
                unit_state: 0,
                stand_state: UnitStandStateType::Stand as u32,
            },
        );

        let condition = Condition {
            condition_type: ConditionType::CreatureType,
            condition_value1: 7,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_unit_branches_fail_without_unit_snapshot_like_cpp_tounit_null() {
        let target = world_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        let condition = Condition {
            condition_type: ConditionType::Alive,
            ..Condition::default()
        };

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &condition
        ));
    }

    #[test]
    fn basic_condition_meets_missing_object_returns_false_before_negative_like_cpp() {
        let condition = Condition {
            condition_type: ConditionType::ZoneId,
            condition_value1: 67,
            negative_condition: true,
            ..Condition::default()
        };
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| true),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn basic_condition_meets_reports_unsupported_for_unrepresented_cpp_state() {
        let mut target = world_object(571, 2);
        target.object_mut().set_entry(1001);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);

        let spawn_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            condition_value3: 77,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&spawn_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );

        let unrepresented_condition = Condition {
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&unrepresented_condition, &mut info, |_, _| false),
            ConditionMeetResult::Unsupported
        );
    }

    #[test]
    fn object_meet_conditions_uses_cpp_else_group_or_of_and() {
        let conditions = vec![
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 2,
                ..Condition::default()
            },
            Condition {
                else_group: 1,
                condition_type: ConditionType::Aura,
                condition_value1: 3,
                ..Condition::default()
            },
        ];
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| condition.condition_value1 != 2,
        );

        assert!(passed);
    }

    #[test]
    fn object_meet_conditions_short_circuits_failed_group_like_cpp() {
        let conditions = vec![
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 2,
                ..Condition::default()
            },
        ];
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);
        let mut checked = Vec::new();

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| {
                checked.push(condition.condition_value1);
                false
            },
        );

        assert!(!passed);
        assert_eq!(checked, vec![1]);
    }

    #[test]
    fn object_meet_conditions_expands_reference_conditions_like_cpp() {
        let reference_condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: 55,
            condition_type: ConditionType::Aura,
            condition_value1: 7,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([reference_condition]);
        let conditions = vec![Condition {
            condition_type: ConditionType::None,
            reference_id: 55,
            ..Condition::default()
        }];
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| condition.condition_value1 == 7,
        );

        assert!(passed);
    }

    #[test]
    fn not_grouped_conditions_missing_bucket_passes_like_cpp() {
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert!(is_object_meeting_not_grouped_conditions_like_cpp(
            &store,
            ConditionSourceType::Phase,
            42,
            &mut info,
            |_, _| false,
        ));
    }

    #[test]
    fn not_grouped_conditions_uses_zero_source_group_and_id_like_cpp() {
        let condition = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 0,
            source_entry: 42,
            source_id: 0,
            condition_type: ConditionType::Aura,
            condition_value1: 10,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert!(is_object_meeting_not_grouped_conditions_like_cpp(
            &store,
            ConditionSourceType::Phase,
            42,
            &mut info,
            |condition, _| condition.condition_value1 == 10,
        ));
    }

    #[test]
    fn has_not_grouped_conditions_uses_cpp_zero_group_entry_key() {
        let condition = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 0,
            source_entry: -1,
            source_id: 0,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);

        assert!(has_conditions_for_not_grouped_entry_like_cpp(
            &store,
            ConditionSourceType::Phase,
            u32::MAX,
        ));
        assert!(!has_conditions_for_not_grouped_entry_like_cpp(
            &store,
            ConditionSourceType::None,
            u32::MAX,
        ));
    }

    #[test]
    fn spell_click_conditions_use_cpp_key_and_target_order() {
        let condition = Condition {
            source_type: ConditionSourceType::SpellClickEvent,
            source_group: 123,
            source_entry: -1,
            source_id: 0,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let clicker = world_object(571, 1);
        let target = world_object(1, 2);

        assert!(has_conditions_for_spell_click_event_like_cpp(
            &store,
            123,
            u32::MAX,
        ));
        assert!(is_object_meeting_spell_click_conditions_like_cpp(
            &store,
            123,
            u32::MAX,
            Some(&clicker),
            Some(&target),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &clicker)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &target)
            },
        ));
    }

    #[test]
    fn vehicle_vendor_and_trainer_conditions_match_cpp_keys_and_targets() {
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::VehicleSpell,
                source_group: 10,
                source_entry: 20,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::NpcVendor,
                source_group: 30,
                source_entry: 40,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::TrainerSpell,
                source_group: 50,
                source_entry: 60,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
        ]);
        let player = world_object(571, 1);
        let other = world_object(571, 1);

        assert!(is_object_meeting_vehicle_spell_conditions_like_cpp(
            &store,
            10,
            20,
            Some(&player),
            Some(&other),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &player)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &other)
            },
        ));
        assert!(is_object_meeting_vendor_item_conditions_like_cpp(
            &store,
            30,
            40,
            Some(&player),
            Some(&other),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &player)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &other)
            },
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            50,
            60,
            Some(&player),
            |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &player),
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            30,
            40,
            Some(&player),
            |_, _| false,
        ));
    }

    #[test]
    fn smart_event_and_visibility_conditions_match_cpp_composite_keys() {
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::SmartEvent,
                source_group: 8,
                source_entry: -7,
                source_id: 9,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::ObjectIdVisibility,
                source_group: 11,
                source_entry: -1,
                source_id: 0,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
        ]);
        let unit = world_object(571, 1);
        let base = world_object(571, 1);

        assert!(is_object_meeting_smart_event_conditions_like_cpp(
            &store,
            -7,
            7,
            9,
            Some(&unit),
            Some(&base),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &unit)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &base)
            },
        ));
        assert!(
            is_object_meeting_visibility_by_object_id_conditions_like_cpp(
                &store,
                11,
                u32::MAX,
                Some(&unit),
                |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &unit),
            )
        );
    }

    #[test]
    fn area_trigger_lookup_uses_server_side_as_cpp_source_entry() {
        let client_condition = Condition {
            source_type: ConditionSourceType::AreaTrigger,
            source_group: 77,
            source_entry: 0,
            condition_type: ConditionType::Aura,
            condition_value1: 10,
            ..Condition::default()
        };
        let server_condition = Condition {
            source_type: ConditionSourceType::AreaTrigger,
            source_group: 77,
            source_entry: 1,
            condition_type: ConditionType::Aura,
            condition_value1: 20,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            client_condition,
            server_condition,
        ]);

        assert_eq!(
            conditions_for_area_trigger_like_cpp(&store, 77, false).unwrap()[0].condition_value1,
            10
        );
        assert_eq!(
            conditions_for_area_trigger_like_cpp(&store, 77, true).unwrap()[0].condition_value1,
            20
        );
    }

    #[test]
    fn loot_store_item_conditions_use_cpp_store_entry_item_key_and_looter_target() {
        let condition = Condition {
            source_type: ConditionSourceType::CreatureLootTemplate,
            source_group: 500,
            source_entry: 6948,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let looter = world_object(571, 1);
        let context = LootStoreItemContext {
            store_kind: LootStoreKind::Creature,
            entry: 500,
            item: LootStoreItem {
                item_id: 6948,
                reference: 0,
                chance: 100.0,
                needs_quest: false,
                loot_mode: 1,
                group_id: 0,
                min_count: 1,
                max_count: 1,
            },
        };

        assert!(is_loot_store_item_meeting_conditions_like_cpp(
            &store,
            context,
            Some(&looter),
            |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &looter),
        ));

        let missing_item_context = LootStoreItemContext {
            item: LootStoreItem {
                item_id: 6949,
                ..context.item
            },
            ..context
        };
        assert!(is_loot_store_item_meeting_conditions_like_cpp(
            &store,
            missing_item_context,
            Some(&looter),
            |_, _| false,
        ));
    }

    #[test]
    fn specialized_condition_lookups_default_to_true_when_missing_like_cpp() {
        let store = ConditionEntriesByTypeStore::default();

        assert!(is_object_meeting_spell_click_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_vehicle_spell_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_smart_event_conditions_like_cpp(
            &store,
            1,
            2,
            3,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_vendor_item_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            |_, _| false,
        ));
        assert!(
            is_object_meeting_visibility_by_object_id_conditions_like_cpp(
                &store,
                1,
                2,
                None,
                |_, _| false,
            )
        );
        assert!(conditions_for_area_trigger_like_cpp(&store, 1, false).is_none());
    }
}
