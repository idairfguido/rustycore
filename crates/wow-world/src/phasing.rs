// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `PhasingHandler` façade slices that are independent of runtime unit graphs.

use std::{collections::HashSet, error::Error, fmt};

use wow_constants::{PhaseFlags, TypeId};
use wow_core::ObjectGuid;
use wow_data::{PhaseGroupStore, PhaseStore};
use wow_entities::{PhaseShift, Unit, WorldObject};
use wow_packet::packets::misc::{PhaseShiftChange, PhaseShiftDataPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseVisibilityUpdate {
    pub update_visibility: bool,
    pub changed: bool,
}

impl PhaseVisibilityUpdate {
    pub const fn new(update_visibility: bool, changed: bool) -> Self {
        Self {
            update_visibility,
            changed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseShiftPacketBuildError {
    PhaseIdOutOfRange(u32),
    VisibleMapIdOutOfRange(u32),
    UiMapPhaseIdOutOfRange(u32),
}

impl fmt::Display for PhaseShiftPacketBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhaseIdOutOfRange(id) => {
                write!(f, "phase id {id} does not fit SMSG_PHASE_SHIFT_CHANGE")
            }
            Self::VisibleMapIdOutOfRange(id) => {
                write!(
                    f,
                    "visible map id {id} does not fit SMSG_PHASE_SHIFT_CHANGE"
                )
            }
            Self::UiMapPhaseIdOutOfRange(id) => {
                write!(
                    f,
                    "UI map phase id {id} does not fit SMSG_PHASE_SHIFT_CHANGE"
                )
            }
        }
    }
}

impl Error for PhaseShiftPacketBuildError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlledUnitInfo {
    pub guid: ObjectGuid,
    pub type_id: TypeId,
    pub has_vehicle: bool,
}

impl ControlledUnitInfo {
    pub const fn new(guid: ObjectGuid, type_id: TypeId, has_vehicle: bool) -> Self {
        Self {
            guid,
            type_id,
            has_vehicle,
        }
    }
}

/// C++ `PhasingHandler::ControlledUnitVisitor` visited-set and selection rules.
pub struct ControlledUnitVisitor {
    visited: HashSet<ObjectGuid>,
}

impl ControlledUnitVisitor {
    pub fn new(owner_guid: ObjectGuid) -> Self {
        let mut visited = HashSet::new();
        visited.insert(owner_guid);
        Self { visited }
    }

    pub fn was_visited(&self, guid: ObjectGuid) -> bool {
        self.visited.contains(&guid)
    }

    pub fn visit_controlled_of_like_cpp<ResolveControlled, SummonExists, VehiclePassengers, Visit>(
        &mut self,
        unit: &Unit,
        mut resolve_controlled: ResolveControlled,
        mut summon_exists: SummonExists,
        vehicle_passengers: VehiclePassengers,
        mut visit: Visit,
    ) where
        ResolveControlled: FnMut(ObjectGuid) -> Option<ControlledUnitInfo>,
        SummonExists: FnMut(ObjectGuid) -> bool,
        VehiclePassengers: IntoIterator<Item = ObjectGuid>,
        Visit: FnMut(ObjectGuid),
    {
        for controlled_guid in &unit.subsystems().control.controlled_guids {
            let Some(controlled) = resolve_controlled(*controlled_guid) else {
                continue;
            };
            if controlled.type_id != TypeId::Player
                && !controlled.has_vehicle
                && self.visited.insert(controlled.guid)
            {
                visit(controlled.guid);
            }
        }

        for summon_guid in unit.subsystems().control.summon_slots {
            if !summon_guid.is_empty()
                && summon_exists(summon_guid)
                && self.visited.insert(summon_guid)
            {
                visit(summon_guid);
            }
        }

        for passenger_guid in vehicle_passengers {
            if !passenger_guid.is_empty()
                && passenger_guid != unit.world().guid()
                && self.visited.insert(passenger_guid)
            {
                visit(passenger_guid);
            }
        }
    }
}

/// C++ local `PhasingHandler.cpp::GetPhaseFlags`.
pub fn phase_flags_for_id_like_cpp(phase_store: &PhaseStore, phase_id: u32) -> PhaseFlags {
    if phase_store.is_cosmetic_phase(phase_id) {
        return PhaseFlags::COSMETIC;
    }

    if phase_store.is_personal_phase(phase_id) {
        return PhaseFlags::PERSONAL;
    }

    PhaseFlags::NONE
}

/// C++ `PhasingHandler::ResetPhaseShift`.
pub fn reset_phase_shift_like_cpp(object: &mut WorldObject) {
    object.phase_shift_mut().clear();
    object.suppressed_phase_shift_mut().clear();
}

/// C++ `PhasingHandler::InheritPhaseShift`.
pub fn inherit_phase_shift_like_cpp(target: &mut WorldObject, source: &WorldObject) {
    *target.phase_shift_mut() = source.phase_shift().clone();
    *target.suppressed_phase_shift_mut() = source.suppressed_phase_shift().clone();
}

/// C++ `PhasingHandler::SetAlwaysVisible`.
pub fn set_always_visible_like_cpp(
    object: &mut WorldObject,
    apply: bool,
    update_visibility: bool,
) -> PhaseVisibilityUpdate {
    object.phase_shift_mut().set_always_visible_like_cpp(apply);
    PhaseVisibilityUpdate::new(update_visibility, true)
}

/// C++ `PhasingHandler::SetInversed`.
pub fn set_inversed_like_cpp(
    object: &mut WorldObject,
    apply: bool,
    update_visibility: bool,
) -> PhaseVisibilityUpdate {
    object.phase_shift_mut().set_inversed_like_cpp(apply);
    PhaseVisibilityUpdate::new(update_visibility, true)
}

/// C++ `PhasingHandler::AddPhase` core mutation, excluding runtime controlled-unit traversal.
pub fn add_phase_like_cpp(
    object: &mut WorldObject,
    phase_store: &PhaseStore,
    phase_id: u32,
    personal_guid: ObjectGuid,
    update_visibility: bool,
) -> PhaseVisibilityUpdate {
    let flags = phase_flags_for_id_like_cpp(phase_store, phase_id);
    let changed = object
        .phase_shift_mut()
        .add_phase_like_cpp(phase_id, flags, 1);

    if object.phase_shift().has_personal_phase_like_cpp() {
        object
            .phase_shift_mut()
            .set_personal_guid_like_cpp(personal_guid);
    }

    PhaseVisibilityUpdate::new(update_visibility, changed)
}

/// Public C++ `PhasingHandler::AddPhase` entry shape for a single object.
pub fn add_object_phase_like_cpp(
    object: &mut WorldObject,
    phase_store: &PhaseStore,
    phase_id: u32,
    update_visibility: bool,
) -> PhaseVisibilityUpdate {
    add_phase_like_cpp(
        object,
        phase_store,
        phase_id,
        object.guid(),
        update_visibility,
    )
}

/// C++ `PhasingHandler::RemovePhase` core mutation, excluding runtime controlled-unit traversal.
pub fn remove_phase_like_cpp(
    object: &mut WorldObject,
    phase_id: u32,
    update_visibility: bool,
) -> PhaseVisibilityUpdate {
    let changed = object.phase_shift_mut().remove_phase_like_cpp(phase_id);
    PhaseVisibilityUpdate::new(update_visibility, changed)
}

/// C++ `PhasingHandler::AddPhaseGroup` core mutation, excluding runtime controlled-unit traversal.
pub fn add_phase_group_like_cpp(
    object: &mut WorldObject,
    phase_store: &PhaseStore,
    phase_group_store: &PhaseGroupStore,
    phase_group_id: u32,
    personal_guid: ObjectGuid,
    update_visibility: bool,
) -> Option<PhaseVisibilityUpdate> {
    let phases = phase_group_store.phases_for_group(phase_group_id)?;
    let mut changed = false;
    for phase_id in phases {
        let flags = phase_flags_for_id_like_cpp(phase_store, *phase_id);
        changed = object
            .phase_shift_mut()
            .add_phase_like_cpp(*phase_id, flags, 1)
            || changed;
    }

    if object.phase_shift().has_personal_phase_like_cpp() {
        object
            .phase_shift_mut()
            .set_personal_guid_like_cpp(personal_guid);
    }

    Some(PhaseVisibilityUpdate::new(update_visibility, changed))
}

/// Public C++ `PhasingHandler::AddPhaseGroup` entry shape for a single object.
pub fn add_object_phase_group_like_cpp(
    object: &mut WorldObject,
    phase_store: &PhaseStore,
    phase_group_store: &PhaseGroupStore,
    phase_group_id: u32,
    update_visibility: bool,
) -> Option<PhaseVisibilityUpdate> {
    add_phase_group_like_cpp(
        object,
        phase_store,
        phase_group_store,
        phase_group_id,
        object.guid(),
        update_visibility,
    )
}

/// C++ `PhasingHandler::RemovePhaseGroup` core mutation, excluding runtime controlled-unit traversal.
pub fn remove_phase_group_like_cpp(
    object: &mut WorldObject,
    phase_group_store: &PhaseGroupStore,
    phase_group_id: u32,
    update_visibility: bool,
) -> Option<PhaseVisibilityUpdate> {
    let phases = phase_group_store.phases_for_group(phase_group_id)?;
    let mut changed = false;
    for phase_id in phases {
        changed = object.phase_shift_mut().remove_phase_like_cpp(*phase_id) || changed;
    }

    Some(PhaseVisibilityUpdate::new(update_visibility, changed))
}

/// C++ `PhasingHandler::SendToPlayer(Player const*, PhaseShift const&)` packet build step.
pub fn phase_shift_change_for_player_like_cpp(
    player_guid: ObjectGuid,
    phase_shift: &PhaseShift,
) -> Result<PhaseShiftChange, PhaseShiftPacketBuildError> {
    let phases = phase_shift
        .phases_like_cpp()
        .map(|phase| {
            Ok(PhaseShiftDataPhase {
                phase_flags: phase.flags().bits(),
                id: u16::try_from(phase.id())
                    .map_err(|_| PhaseShiftPacketBuildError::PhaseIdOutOfRange(phase.id()))?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let visible_map_ids = phase_shift
        .visible_map_ids_like_cpp()
        .map(|visible_map_id| {
            u16::try_from(visible_map_id)
                .map_err(|_| PhaseShiftPacketBuildError::VisibleMapIdOutOfRange(visible_map_id))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let ui_map_phase_ids = phase_shift
        .ui_map_phase_ids_like_cpp()
        .map(|ui_map_phase_id| {
            u16::try_from(ui_map_phase_id)
                .map_err(|_| PhaseShiftPacketBuildError::UiMapPhaseIdOutOfRange(ui_map_phase_id))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PhaseShiftChange {
        player_guid,
        phase_shift_flags: phase_shift.flags_like_cpp().bits(),
        phases,
        personal_guid: phase_shift.personal_guid_like_cpp(),
        visible_map_ids,
        preload_map_ids: Vec::new(),
        ui_map_phase_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{PhaseFlags, PhaseShiftFlags, TypeId, TypeMask};
    use wow_core::{ObjectGuid, guid::HighGuid};
    use wow_data::{
        PhaseEntry, PhaseXPhaseGroupEntry,
        phase::{PHASE_ENTRY_FLAG_COSMETIC, PHASE_ENTRY_FLAG_PERSONAL},
    };

    fn world_object() -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        object.object_mut().create(ObjectGuid::create_world_object(
            HighGuid::Creature,
            0,
            0,
            571,
            0,
            1,
            1,
        ));
        object
    }

    fn unit(guid: ObjectGuid) -> Unit {
        let mut unit = Unit::new(true);
        unit.world_mut().object_mut().create(guid);
        unit
    }

    fn phase_store() -> PhaseStore {
        PhaseStore::from_entries([
            PhaseEntry { id: 10, flags: 0 },
            PhaseEntry {
                id: 20,
                flags: PHASE_ENTRY_FLAG_PERSONAL,
            },
            PhaseEntry {
                id: 30,
                flags: PHASE_ENTRY_FLAG_COSMETIC,
            },
        ])
    }

    fn phase_group_store(phase_store: &PhaseStore) -> PhaseGroupStore {
        PhaseGroupStore::from_entries(
            phase_store,
            [
                PhaseXPhaseGroupEntry {
                    id: 1,
                    phase_id: 10,
                    phase_group_id: 7,
                },
                PhaseXPhaseGroupEntry {
                    id: 2,
                    phase_id: 20,
                    phase_group_id: 7,
                },
                PhaseXPhaseGroupEntry {
                    id: 3,
                    phase_id: 99,
                    phase_group_id: 7,
                },
            ],
        )
    }

    #[test]
    fn reset_phase_shift_clears_active_and_suppressed_like_cpp() {
        let mut object = world_object();
        object
            .phase_shift_mut()
            .add_phase_like_cpp(10, PhaseFlags::NONE, 1);
        object
            .suppressed_phase_shift_mut()
            .add_phase_like_cpp(20, PhaseFlags::NONE, 1);

        reset_phase_shift_like_cpp(&mut object);

        assert!(
            object
                .phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::UNPHASED)
        );
        assert!(
            object
                .suppressed_phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::UNPHASED)
        );
        assert!(!object.phase_shift().has_phase_like_cpp(10));
        assert!(!object.suppressed_phase_shift().has_phase_like_cpp(20));
    }

    #[test]
    fn controlled_unit_visitor_matches_cpp_selection_rules() {
        let owner_guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 1);
        let controlled_guid =
            ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 2);
        let controlled_player = ObjectGuid::create_player(1, 50);
        let nested_vehicle_passenger =
            ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 3);
        let summon_guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 4);
        let missing_summon =
            ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 5);
        let vehicle_passenger =
            ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, 6);

        let mut unit = unit(owner_guid);
        unit.subsystems_mut()
            .control
            .add_controlled(controlled_guid);
        unit.subsystems_mut()
            .control
            .add_controlled(controlled_player);
        unit.subsystems_mut()
            .control
            .add_controlled(nested_vehicle_passenger);
        unit.subsystems_mut()
            .control
            .set_summon_slot(0, summon_guid);
        unit.subsystems_mut()
            .control
            .set_summon_slot(1, missing_summon);

        let mut visitor = ControlledUnitVisitor::new(owner_guid);
        let mut visited = Vec::new();
        visitor.visit_controlled_of_like_cpp(
            &unit,
            |guid| {
                if guid == controlled_guid {
                    Some(ControlledUnitInfo::new(guid, TypeId::Unit, false))
                } else if guid == controlled_player {
                    Some(ControlledUnitInfo::new(guid, TypeId::Player, false))
                } else if guid == nested_vehicle_passenger {
                    Some(ControlledUnitInfo::new(guid, TypeId::Unit, true))
                } else {
                    None
                }
            },
            |guid| guid == summon_guid,
            [vehicle_passenger, owner_guid, summon_guid],
            |guid| visited.push(guid),
        );

        visited.sort();
        assert_eq!(
            visited,
            vec![controlled_guid, summon_guid, vehicle_passenger]
        );
        assert!(visitor.was_visited(owner_guid));
    }

    #[test]
    fn add_and_remove_phase_core_match_cpp_mutation() {
        let phase_store = phase_store();
        let mut object = world_object();
        let personal_guid = object.guid();

        let update = add_object_phase_like_cpp(&mut object, &phase_store, 20, true);

        assert_eq!(update, PhaseVisibilityUpdate::new(true, true));
        assert!(object.phase_shift().has_phase_like_cpp(20));
        assert_eq!(
            object
                .phase_shift()
                .phase_ref_like_cpp(20)
                .map(|phase| phase.flags()),
            Some(PhaseFlags::PERSONAL)
        );
        assert_eq!(object.phase_shift().personal_guid_like_cpp(), personal_guid);

        let update = add_object_phase_like_cpp(&mut object, &phase_store, 20, true);
        assert_eq!(update, PhaseVisibilityUpdate::new(true, false));

        let update = remove_phase_like_cpp(&mut object, 20, false);
        assert_eq!(update, PhaseVisibilityUpdate::new(false, false));
        assert!(object.phase_shift().has_phase_like_cpp(20));

        let update = remove_phase_like_cpp(&mut object, 20, false);
        assert_eq!(update, PhaseVisibilityUpdate::new(false, true));
        assert!(!object.phase_shift().has_phase_like_cpp(20));
    }

    #[test]
    fn phase_group_core_uses_cpp_group_lookup_and_phase_flags() {
        let phase_store = phase_store();
        let phase_group_store = phase_group_store(&phase_store);
        let mut object = world_object();

        let update =
            add_object_phase_group_like_cpp(&mut object, &phase_store, &phase_group_store, 7, true);

        assert_eq!(update, Some(PhaseVisibilityUpdate::new(true, true)));
        assert!(object.phase_shift().has_phase_like_cpp(10));
        assert!(object.phase_shift().has_phase_like_cpp(20));
        assert!(!object.phase_shift().has_phase_like_cpp(99));

        let missing_group = add_object_phase_group_like_cpp(
            &mut object,
            &phase_store,
            &phase_group_store,
            99,
            true,
        );
        assert_eq!(missing_group, None);

        let update = remove_phase_group_like_cpp(&mut object, &phase_group_store, 7, false);
        assert_eq!(update, Some(PhaseVisibilityUpdate::new(false, true)));
        assert!(!object.phase_shift().has_phase_like_cpp(10));
        assert!(!object.phase_shift().has_phase_like_cpp(20));
    }

    #[test]
    fn inherit_phase_shift_copies_active_and_suppressed_like_cpp() {
        let mut source = world_object();
        let mut target = world_object();
        source
            .phase_shift_mut()
            .add_phase_like_cpp(10, PhaseFlags::NONE, 1);
        source
            .suppressed_phase_shift_mut()
            .add_phase_like_cpp(20, PhaseFlags::NONE, 1);

        inherit_phase_shift_like_cpp(&mut target, &source);

        assert!(target.phase_shift().has_phase_like_cpp(10));
        assert!(target.suppressed_phase_shift().has_phase_like_cpp(20));
    }

    #[test]
    fn set_visibility_flags_match_cpp_and_report_update_request() {
        let mut object = world_object();

        let update = set_always_visible_like_cpp(&mut object, true, true);
        assert_eq!(update, PhaseVisibilityUpdate::new(true, true));
        assert!(
            object
                .phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::ALWAYS_VISIBLE)
        );

        let update = set_inversed_like_cpp(&mut object, true, false);
        assert_eq!(update, PhaseVisibilityUpdate::new(false, true));
        assert!(
            object
                .phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::INVERSE)
        );
        assert!(
            object
                .phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::INVERSE_UNPHASED)
        );
        assert!(
            !object
                .phase_shift()
                .flags_like_cpp()
                .contains(PhaseShiftFlags::UNPHASED)
        );
    }

    #[test]
    fn phase_shift_change_packet_copies_phase_shift_like_cpp_send_to_player() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let personal_guid = ObjectGuid::create_player(1, 99);
        let mut phase_shift = PhaseShift::default();
        phase_shift.add_phase_like_cpp(10, PhaseFlags::COSMETIC, 1);
        phase_shift.add_phase_like_cpp(20, PhaseFlags::PERSONAL, 1);
        phase_shift.set_personal_guid_like_cpp(personal_guid);
        phase_shift.add_visible_map_id_like_cpp(609, 1);
        phase_shift.add_ui_map_phase_id_like_cpp(42, 1);

        let packet = phase_shift_change_for_player_like_cpp(player_guid, &phase_shift).unwrap();

        assert_eq!(packet.player_guid, player_guid);
        assert_eq!(
            packet.phase_shift_flags,
            phase_shift.flags_like_cpp().bits()
        );
        assert_eq!(packet.personal_guid, personal_guid);
        assert_eq!(
            packet.phases,
            vec![
                PhaseShiftDataPhase {
                    phase_flags: PhaseFlags::COSMETIC.bits(),
                    id: 10,
                },
                PhaseShiftDataPhase {
                    phase_flags: PhaseFlags::PERSONAL.bits(),
                    id: 20,
                },
            ]
        );
        assert_eq!(packet.visible_map_ids, vec![609]);
        assert!(packet.preload_map_ids.is_empty());
        assert_eq!(packet.ui_map_phase_ids, vec![42]);
    }

    #[test]
    fn phase_shift_change_packet_rejects_values_that_do_not_fit_cpp_wire_types() {
        let mut phase_shift = PhaseShift::default();
        phase_shift.add_phase_like_cpp(u16::MAX as u32 + 1, PhaseFlags::NONE, 1);

        let error = match phase_shift_change_for_player_like_cpp(ObjectGuid::EMPTY, &phase_shift) {
            Ok(_) => panic!("overflowing phase id must be rejected"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            PhaseShiftPacketBuildError::PhaseIdOutOfRange(u16::MAX as u32 + 1)
        );
    }
}
