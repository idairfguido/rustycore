// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Runtime side of C++ `ConditionMgr` evaluation context.

use wow_constants::ConditionSourceType;
use wow_constants::MAX_CONDITION_TARGETS;
use wow_data::{Condition, ConditionEntriesByTypeStore, ConditionId};
use wow_entities::WorldObject;

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
    pub condition_map: Option<ConditionMapRef>,
    pub last_failed_condition: Option<&'a Condition>,
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
            condition_map,
            last_failed_condition: None,
        }
    }

    /// C++ `ConditionSourceInfo(Map const*)`.
    pub const fn from_map(condition_map: ConditionMapRef) -> Self {
        Self {
            condition_targets: [None; MAX_CONDITION_TARGETS],
            condition_map: Some(condition_map),
            last_failed_condition: None,
        }
    }

    pub fn mark_failed_like_cpp(&mut self, condition: &'a Condition) {
        self.last_failed_condition = Some(condition);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{ConditionType, TypeId, TypeMask};

    fn world_object(map_id: u32, instance_id: u32) -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
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
}
