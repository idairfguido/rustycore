// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `PhasingHandler` façade slices that are independent of runtime unit graphs.

use std::{error::Error, fmt};

use wow_core::ObjectGuid;
use wow_entities::{PhaseShift, WorldObject};
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
