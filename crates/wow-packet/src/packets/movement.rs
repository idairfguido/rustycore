// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Movement packet definitions.
//!
//! Handles all CMSG_MOVE_* client packets (player movement) and server-side
//! movement packets (MoveUpdate, OnMonsterMove).

use wow_constants::movement::{MovementFlag, MovementFlag2, MovementFlags3};
use wow_constants::{ClientOpcodes, ServerOpcodes};
use wow_core::{ObjectGuid, Position};

use crate::world_packet::{PacketError, WorldPacket};
use crate::{ClientPacket, ServerPacket};

// ── MovementInfo ─────────────────────────────────────────────────

/// Full movement info parsed from any CMSG_MOVE_* packet.
///
/// Binary layout (from C# PacketHandlerExtensions.Read):
/// ```text
/// PackedGuid  guid
/// u32         movement_flags
/// u32         movement_flags2
/// u32         movement_flags3
/// u32         time (ms)
/// f32         x, y, z, orientation
/// f32         pitch
/// f32         step_up_start_elevation
/// u32         remove_movement_forces_count
/// u32         move_index
/// PackedGuid  × remove_movement_forces_count (GUIDs to remove)
/// bit         has_standing_on_gameobject_guid
/// bit         has_transport
/// bit         has_fall
/// bit         has_spline
/// bit         height_change_failed
/// bit         remote_time_valid
/// bit         has_inertia
/// bit         has_adv_flying
/// [flush]
/// [transport info if has_transport]
/// [standing_on_guid if has_standing_on_gameobject_guid]
/// [inertia if has_inertia]
/// [adv_flying if has_adv_flying]
/// [fall info if has_fall]
/// ```
#[derive(Debug, Clone)]
pub struct MovementInfo {
    pub guid: ObjectGuid,
    pub flags: MovementFlag,
    pub flags2: MovementFlag2,
    pub flags3: MovementFlags3,
    pub time: u32,
    pub position: Position,
    pub pitch: f32,
    pub step_up_start_elevation: f32,
    pub jump: JumpInfo,
    pub transport: Option<TransportInfo>,
    pub inertia: Option<InertiaInfo>,
    pub adv_flying: Option<AdvFlyingInfo>,
    pub standing_on_gameobject_guid: Option<ObjectGuid>,
}

impl Default for MovementInfo {
    fn default() -> Self {
        Self {
            guid: ObjectGuid::EMPTY,
            flags: MovementFlag::NONE,
            flags2: MovementFlag2::NONE,
            flags3: MovementFlags3::NONE,
            time: 0,
            position: Position::ZERO,
            pitch: 0.0,
            step_up_start_elevation: 0.0,
            jump: JumpInfo::default(),
            transport: None,
            inertia: None,
            adv_flying: None,
            standing_on_gameobject_guid: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct JumpInfo {
    pub fall_time: u32,
    pub z_speed: f32,
    pub has_direction: bool,
    pub sin_angle: f32,
    pub cos_angle: f32,
    pub xy_speed: f32,
}

#[derive(Debug, Clone)]
pub struct TransportInfo {
    pub guid: ObjectGuid,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    pub seat: i8,
    pub time: u32,
    pub prev_time: Option<u32>,
    pub vehicle_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct InertiaInfo {
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub lifetime: u32,
}

#[derive(Debug, Clone, Default)]
pub struct AdvFlyingInfo {
    pub forward_velocity: f32,
    pub up_velocity: f32,
}

impl MovementInfo {
    /// Parse from packet buffer (after opcode has been consumed).
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        let guid = pkt.read_packed_guid()?;
        let flags = MovementFlag::from_bits_truncate(pkt.read_uint32()?);
        let flags2 = MovementFlag2::from_bits_truncate(pkt.read_uint32()?);
        let flags3 = MovementFlags3::from_bits_truncate(pkt.read_uint32()?);
        let time = pkt.read_uint32()?;
        let x = pkt.read_float()?;
        let y = pkt.read_float()?;
        let z = pkt.read_float()?;
        let o = pkt.read_float()?;
        let pitch = pkt.read_float()?;
        let step_up_start_elevation = pkt.read_float()?;

        let remove_forces_count = pkt.read_uint32()?;
        let _move_index = pkt.read_uint32()?;

        // skip force GUIDs
        for _ in 0..remove_forces_count {
            pkt.read_packed_guid()?;
        }

        let has_standing_on_go = pkt.has_bit()?;
        let has_transport = pkt.has_bit()?;
        let has_fall = pkt.has_bit()?;
        let _has_spline = pkt.has_bit()?;
        let _height_change_failed = pkt.has_bit()?;
        let _remote_time_valid = pkt.has_bit()?;
        let has_inertia = pkt.has_bit()?;
        let has_adv_flying = pkt.has_bit()?;

        let transport = if has_transport {
            let tguid = pkt.read_packed_guid()?;
            let tx = pkt.read_float()?;
            let ty = pkt.read_float()?;
            let tz = pkt.read_float()?;
            let to_ = pkt.read_float()?;
            let seat = pkt.read_int8()?;
            let ttime = pkt.read_uint32()?;

            let has_prev = pkt.has_bit()?;
            let has_vehicle_id = pkt.has_bit()?;
            // bit reader auto-resets on next byte read

            let prev_time = if has_prev {
                Some(pkt.read_uint32()?)
            } else {
                None
            };
            let vehicle_id = if has_vehicle_id {
                Some(pkt.read_int32()?)
            } else {
                None
            };

            Some(TransportInfo {
                guid: tguid,
                x: tx,
                y: ty,
                z: tz,
                o: to_,
                seat,
                time: ttime,
                prev_time,
                vehicle_id,
            })
        } else {
            None
        };

        let standing_on_gameobject_guid = if has_standing_on_go {
            Some(pkt.read_packed_guid()?)
        } else {
            None
        };

        let inertia = if has_inertia {
            Some(InertiaInfo {
                id: pkt.read_int32()?,
                x: pkt.read_float()?,
                y: pkt.read_float()?,
                z: pkt.read_float()?,
                lifetime: pkt.read_uint32()?,
            })
        } else {
            None
        };

        let adv_flying = if has_adv_flying {
            let fwd = pkt.read_float()?;
            let up = pkt.read_float()?;
            Some(AdvFlyingInfo {
                forward_velocity: fwd,
                up_velocity: up,
            })
        } else {
            None
        };

        let jump = if has_fall {
            let fall_time = pkt.read_uint32()?;
            let z_speed = pkt.read_float()?;
            let has_direction = pkt.has_bit()?;
            // bit reader auto-resets on next byte read
            let (sin_angle, cos_angle, xy_speed) = if has_direction {
                (pkt.read_float()?, pkt.read_float()?, pkt.read_float()?)
            } else {
                (0.0, 0.0, 0.0)
            };
            JumpInfo {
                fall_time,
                z_speed,
                has_direction,
                sin_angle,
                cos_angle,
                xy_speed,
            }
        } else {
            JumpInfo::default()
        };

        Ok(MovementInfo {
            guid,
            flags,
            flags2,
            flags3,
            time,
            position: Position::new(x, y, z, o),
            pitch,
            step_up_start_elevation,
            jump,
            transport,
            inertia,
            adv_flying,
            standing_on_gameobject_guid,
        })
    }

    /// Write movement info to a packet (for MoveUpdate broadcasts).
    pub fn write(&self, pkt: &mut WorldPacket) {
        let has_transport = self.transport.is_some();
        let has_fall_direction = self
            .flags
            .intersects(MovementFlag::FALLING | MovementFlag::FALLING_FAR);
        let has_fall = has_fall_direction || self.jump.fall_time != 0;
        let has_inertia = self.inertia.is_some();
        let has_adv_flying = self.adv_flying.is_some();
        let has_standing_on_gameobject_guid = self.standing_on_gameobject_guid.is_some();

        pkt.write_packed_guid(&self.guid);
        pkt.write_uint32(self.flags.bits());
        pkt.write_uint32(self.flags2.bits());
        pkt.write_uint32(self.flags3.bits());
        pkt.write_uint32(self.time);
        pkt.write_float(self.position.x);
        pkt.write_float(self.position.y);
        pkt.write_float(self.position.z);
        pkt.write_float(self.position.orientation);
        pkt.write_float(self.pitch);
        pkt.write_float(self.step_up_start_elevation);

        pkt.write_uint32(0u32); // remove_forces_count
        pkt.write_uint32(0u32); // move_index

        pkt.write_bit(has_standing_on_gameobject_guid);
        pkt.write_bit(has_transport);
        pkt.write_bit(has_fall);
        pkt.write_bit(false); // has_spline
        pkt.write_bit(false); // height_change_failed
        pkt.write_bit(false); // remote_time_valid
        pkt.write_bit(has_inertia);
        pkt.write_bit(has_adv_flying);
        pkt.flush_bits();

        if let Some(t) = &self.transport {
            pkt.write_packed_guid(&t.guid);
            pkt.write_float(t.x);
            pkt.write_float(t.y);
            pkt.write_float(t.z);
            pkt.write_float(t.o);
            pkt.write_int8(t.seat);
            pkt.write_uint32(t.time);
            pkt.write_bit(t.prev_time.is_some());
            pkt.write_bit(t.vehicle_id.is_some());
            pkt.flush_bits();
            if let Some(pt) = t.prev_time {
                pkt.write_uint32(pt);
            }
            if let Some(vid) = t.vehicle_id {
                pkt.write_int32(vid);
            }
        }

        if let Some(guid) = &self.standing_on_gameobject_guid {
            pkt.write_packed_guid(guid);
        }

        if let Some(inertia) = &self.inertia {
            pkt.write_int32(inertia.id);
            pkt.write_float(inertia.x);
            pkt.write_float(inertia.y);
            pkt.write_float(inertia.z);
            pkt.write_uint32(inertia.lifetime);
        }

        if let Some(af) = &self.adv_flying {
            pkt.write_float(af.forward_velocity);
            pkt.write_float(af.up_velocity);
        }

        if has_fall {
            pkt.write_uint32(self.jump.fall_time);
            pkt.write_float(self.jump.z_speed);
            pkt.write_bit(has_fall_direction);
            pkt.flush_bits();
            if has_fall_direction {
                pkt.write_float(self.jump.sin_angle);
                pkt.write_float(self.jump.cos_angle);
                pkt.write_float(self.jump.xy_speed);
            }
        }
    }
}

// ── ClientPlayerMovement (CMSG_MOVE_*) ───────────────────────────

/// Generic movement packet sent by the client for all movement opcodes.
#[derive(Debug, Clone)]
pub struct ClientPlayerMovement {
    pub info: MovementInfo,
}

/// Macro to implement ClientPacket for all movement opcodes.
macro_rules! impl_movement_client_packet {
    ($opcode:ident) => {
        // We can't use a macro for const OPCODE easily with multiple,
        // so we implement a shared read function instead.
    };
}

impl ClientPlayerMovement {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        let info = MovementInfo::read(pkt)?;
        Ok(Self { info })
    }
}

// ── Movement ACK client packets ──────────────────────────────────

/// C++ `WorldPackets::Movement::MovementAck`.
#[derive(Debug, Clone)]
pub struct MovementAck {
    pub status: MovementInfo,
    pub ack_index: i32,
}

impl MovementAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            status: MovementInfo::read(pkt)?,
            ack_index: pkt.read_int32()?,
        })
    }
}

/// Generic ACK packet used by root, hover, water-walk and similar movement toggles.
#[derive(Debug, Clone)]
pub struct MovementAckMessage {
    pub ack: MovementAck,
}

impl MovementAckMessage {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            ack: MovementAck::read(pkt)?,
        })
    }
}

/// ACK packet carrying a movement speed or movement-force magnitude.
#[derive(Debug, Clone)]
pub struct MovementSpeedAck {
    pub ack: MovementAck,
    pub speed: f32,
}

impl MovementSpeedAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            ack: MovementAck::read(pkt)?,
            speed: pkt.read_float()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveKnockBackSpeeds {
    pub horz_speed: f32,
    pub vert_speed: f32,
}

#[derive(Debug, Clone)]
pub struct MoveKnockBackAck {
    pub ack: MovementAck,
    pub speeds: Option<MoveKnockBackSpeeds>,
}

impl MoveKnockBackAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        let ack = MovementAck::read(pkt)?;
        let speeds = if pkt.read_bit()? {
            Some(MoveKnockBackSpeeds {
                horz_speed: pkt.read_float()?,
                vert_speed: pkt.read_float()?,
            })
        } else {
            None
        };
        Ok(Self { ack, speeds })
    }
}

#[derive(Debug, Clone)]
pub struct MoveSetCollisionHeightAck {
    pub data: MovementAck,
    pub height: f32,
    pub mount_display_id: u32,
    pub reason: u8,
}

impl MoveSetCollisionHeightAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            data: MovementAck::read(pkt)?,
            height: pkt.read_float()?,
            mount_display_id: pkt.read_uint32()?,
            reason: pkt.read_uint8()?,
        })
    }
}

/// C++ `MovementForceType`, stored as two bits on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementForceType {
    SingleDirectional,
    Gravity,
    Unknown(u8),
}

impl MovementForceType {
    fn from_wire(value: u8) -> Self {
        match value {
            0 => Self::SingleDirectional,
            1 => Self::Gravity,
            value => Self::Unknown(value),
        }
    }

    pub fn to_wire(self) -> u8 {
        match self {
            Self::SingleDirectional => 0,
            Self::Gravity => 1,
            Self::Unknown(value) => value & 0x03,
        }
    }
}

/// C++ `MovementForce` wire shape.
#[derive(Debug, Clone, PartialEq)]
pub struct MovementForce {
    pub id: ObjectGuid,
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub transport_id: u32,
    pub magnitude: f32,
    pub unused_910: i32,
    pub force_type: MovementForceType,
}

impl MovementForce {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            id: pkt.read_packed_guid()?,
            origin: [pkt.read_float()?, pkt.read_float()?, pkt.read_float()?],
            direction: [pkt.read_float()?, pkt.read_float()?, pkt.read_float()?],
            transport_id: pkt.read_uint32()?,
            magnitude: pkt.read_float()?,
            unused_910: pkt.read_int32()?,
            force_type: MovementForceType::from_wire(pkt.read_bits(2)? as u8),
        })
    }

    pub fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.id);
        for value in self.origin {
            pkt.write_float(value);
        }
        for value in self.direction {
            pkt.write_float(value);
        }
        pkt.write_uint32(self.transport_id);
        pkt.write_float(self.magnitude);
        pkt.write_int32(self.unused_910);
        pkt.write_bits(u32::from(self.force_type.to_wire()), 2);
        pkt.flush_bits();
    }
}

#[derive(Debug, Clone)]
pub struct MoveApplyMovementForceAck {
    pub ack: MovementAck,
    pub force: MovementForce,
}

impl MoveApplyMovementForceAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            ack: MovementAck::read(pkt)?,
            force: MovementForce::read(pkt)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MoveRemoveMovementForceAck {
    pub ack: MovementAck,
    pub id: ObjectGuid,
}

impl MoveRemoveMovementForceAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            ack: MovementAck::read(pkt)?,
            id: pkt.read_packed_guid()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MoveUpdateApplyMovementForce {
    pub status: MovementInfo,
    pub force: MovementForce,
}

impl ServerPacket for MoveUpdateApplyMovementForce {
    const OPCODE: ServerOpcodes = ServerOpcodes::MoveUpdateApplyMovementForce;

    fn write(&self, pkt: &mut WorldPacket) {
        self.status.write(pkt);
        self.force.write(pkt);
    }
}

#[derive(Debug, Clone)]
pub struct MoveUpdateRemoveMovementForce {
    pub status: MovementInfo,
    pub trigger_guid: ObjectGuid,
}

impl ServerPacket for MoveUpdateRemoveMovementForce {
    const OPCODE: ServerOpcodes = ServerOpcodes::MoveUpdateRemoveMovementForce;

    fn write(&self, pkt: &mut WorldPacket) {
        self.status.write(pkt);
        pkt.write_packed_guid(&self.trigger_guid);
    }
}

#[derive(Debug, Clone)]
pub struct MoveTimeSkipped {
    pub mover_guid: ObjectGuid,
    pub time_skipped: u32,
}

impl MoveTimeSkipped {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            mover_guid: pkt.read_packed_guid()?,
            time_skipped: pkt.read_uint32()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MoveSplineDone {
    pub status: MovementInfo,
    pub spline_id: i32,
}

impl MoveSplineDone {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            status: MovementInfo::read(pkt)?,
            spline_id: pkt.read_int32()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct MoveTeleportAck {
    pub mover_guid: ObjectGuid,
    pub ack_index: i32,
    pub move_time: i32,
}

impl MoveTeleportAck {
    pub fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            mover_guid: pkt.read_packed_guid()?,
            ack_index: pkt.read_int32()?,
            move_time: pkt.read_int32()?,
        })
    }
}

// ── MoveUpdate (SMSG_MOVE_UPDATE) ────────────────────────────────

/// Broadcast a player's movement to nearby players.
#[derive(Debug, Clone)]
pub struct MoveUpdate {
    pub info: MovementInfo,
}

impl ServerPacket for MoveUpdate {
    const OPCODE: ServerOpcodes = ServerOpcodes::MoveUpdate;

    fn write(&self, pkt: &mut WorldPacket) {
        self.info.write(pkt);
    }
}

// ── MonsterMove (SMSG_ON_MONSTER_MOVE) ───────────────────────────

/// Server moves a creature/NPC along a spline path.
///
/// Simplified version: single straight-line move with one destination point.
///
/// Wire format (simplified, no cyclic/uncompressed):
/// ```text
/// PackedGuid  mover_guid
/// Vector3     current_pos (f32 x3)
/// u32         spline_id
/// u32         move_time_ms
/// u32         spline_flags
/// u8          face_type  (0=none)
/// f32         face_direction (if face_type == 4)
/// i32         points_count  (count of dest points)
/// Vector3     destination point (last point, then packed deltas)
/// i32         packed_deltas_count (0 for single point)
/// ```
#[derive(Debug, Clone)]
pub struct MonsterMove {
    pub mover_guid: ObjectGuid,
    pub current_pos: Position,
    pub spline_id: u32,
    pub move_time_ms: u32,
    /// SplineFlag: 0x400 = UncompressedPath, 0x800 = Cyclic, default 0
    pub spline_flags: u32,
    pub destination: Position,
}

impl ServerPacket for MonsterMove {
    const OPCODE: ServerOpcodes = ServerOpcodes::OnMonsterMove;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.mover_guid);
        // current position
        pkt.write_float(self.current_pos.x);
        pkt.write_float(self.current_pos.y);
        pkt.write_float(self.current_pos.z);
        // spline id
        pkt.write_uint32(self.spline_id);
        // MovementMonsterSpline::Write
        // move time
        pkt.write_uint32(self.move_time_ms);
        // spline flags (0 = default linear)
        pkt.write_uint32(self.spline_flags);
        // face type (0 = none/direction)
        pkt.write_uint8(0u8);
        // face direction (written when face_type == 4, FACE_ANGLE)
        // For face_type 0 we skip face data.

        // No AnimTierTransition (spline flags don't have Animation)
        // No JumpExtraData
        // No FadeObjectTime

        // Points (uncompressed path = false, so last point + packed deltas)
        // For a single-destination move: 1 destination point, 0 packed deltas
        pkt.write_int32(1i32); // points count
        pkt.write_float(self.destination.x);
        pkt.write_float(self.destination.y);
        pkt.write_float(self.destination.z);
        pkt.write_int32(0i32); // packed deltas count
    }
}

// ── MonsterMoveStop ───────────────────────────────────────────────

/// Stops a creature's current spline movement.
#[derive(Debug, Clone)]
pub struct MonsterMoveStop {
    pub mover_guid: ObjectGuid,
    pub current_pos: Position,
    pub spline_id: u32,
}

impl ServerPacket for MonsterMoveStop {
    const OPCODE: ServerOpcodes = ServerOpcodes::OnMonsterMove;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.mover_guid);
        pkt.write_float(self.current_pos.x);
        pkt.write_float(self.current_pos.y);
        pkt.write_float(self.current_pos.z);
        pkt.write_uint32(self.spline_id);
        // move_time = 0 → stop
        pkt.write_uint32(0u32);
        // SplineFlag::Done = 0x100
        pkt.write_uint32(0x100u32);
        pkt.write_uint8(0u8); // face_type = none
        pkt.write_int32(0i32); // no points
        pkt.write_int32(0i32); // no packed deltas
    }
}

// ── SetActiveMover (CMSG 0x3A3C) ──────────────────────────────────

/// Client sets which unit is currently being moved (should be player's own GUID).
/// Sent after login and when switching controlled units (e.g., vehicles).
///
/// C#: `SetActiveMover` in MovementPackets.cs
#[derive(Debug, Clone)]
pub struct SetActiveMover {
    pub active_mover: ObjectGuid,
}

impl ClientPacket for SetActiveMover {
    const OPCODE: ClientOpcodes = ClientOpcodes::SetActiveMover;

    fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        let active_mover = pkt.read_packed_guid()?;
        Ok(Self { active_mover })
    }
}

// ── MoveInitActiveMoverComplete (CMSG 0x3A46) ─────────────────────

/// Client acknowledges that the active mover has been fully initialized.
/// Sent after login; server may update transport timing flags.
///
/// C#: `MoveInitActiveMoverComplete` in MovementPackets.cs
#[derive(Debug, Clone)]
pub struct MoveInitActiveMoverComplete {
    /// Ticks relative to server time (used for transport sync).
    pub ticks: u32,
}

impl ClientPacket for MoveInitActiveMoverComplete {
    const OPCODE: ClientOpcodes = ClientOpcodes::MoveInitActiveMoverComplete;

    fn read(pkt: &mut WorldPacket) -> Result<Self, PacketError> {
        let ticks = pkt.read_uint32()?;
        Ok(Self { ticks })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_packet::WorldPacket;
    use wow_core::guid::HighGuid;

    #[test]
    fn movement_info_write_includes_fall_data_when_falling_flag_is_set_like_cpp() {
        let mut info = MovementInfo {
            guid: ObjectGuid::create_player(1, 42),
            flags: MovementFlag::FALLING,
            time: 1234,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            jump: JumpInfo {
                fall_time: 0,
                z_speed: 5.0,
                has_direction: false,
                sin_angle: 0.25,
                cos_angle: 0.75,
                xy_speed: 6.0,
            },
            ..MovementInfo::default()
        };

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);

        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let decoded = MovementInfo::read(&mut pkt).unwrap();
        assert_eq!(decoded.flags, MovementFlag::FALLING);
        assert_eq!(decoded.jump.fall_time, 0);
        assert_eq!(decoded.jump.z_speed, 5.0);
        assert!(decoded.jump.has_direction);
        assert_eq!(decoded.jump.sin_angle, 0.25);
        assert_eq!(decoded.jump.cos_angle, 0.75);
        assert_eq!(decoded.jump.xy_speed, 6.0);

        info.flags = MovementFlag::NONE;
        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let decoded = MovementInfo::read(&mut pkt).unwrap();
        assert_eq!(decoded.jump.fall_time, 0);
        assert!(!decoded.jump.has_direction);
    }

    #[test]
    fn movement_info_preserves_standing_guid_and_inertia_like_cpp() {
        let info = MovementInfo {
            guid: ObjectGuid::create_player(1, 42),
            time: 77,
            position: Position::new(10.0, 20.0, 30.0, 1.5),
            standing_on_gameobject_guid: Some(ObjectGuid::create_world_object(
                HighGuid::GameObject,
                0,
                1,
                0,
                0,
                7,
                9001,
            )),
            inertia: Some(InertiaInfo {
                id: 12,
                x: 1.0,
                y: 2.0,
                z: 3.0,
                lifetime: 400,
            }),
            ..MovementInfo::default()
        };

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);

        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let decoded = MovementInfo::read(&mut pkt).unwrap();
        assert_eq!(
            decoded.standing_on_gameobject_guid,
            Some(ObjectGuid::create_world_object(
                HighGuid::GameObject,
                0,
                1,
                0,
                0,
                7,
                9001,
            ))
        );
        let inertia = decoded.inertia.unwrap();
        assert_eq!(inertia.id, 12);
        assert_eq!(inertia.x, 1.0);
        assert_eq!(inertia.y, 2.0);
        assert_eq!(inertia.z, 3.0);
        assert_eq!(inertia.lifetime, 400);
    }

    #[test]
    fn movement_ack_packets_read_cpp_field_order() {
        let info = MovementInfo {
            guid: ObjectGuid::create_player(1, 42),
            time: 1234,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            ..MovementInfo::default()
        };

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(77);
        pkt.write_float(7.5);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let speed_ack = MovementSpeedAck::read(&mut pkt).unwrap();
        assert_eq!(speed_ack.ack.status.guid, info.guid);
        assert_eq!(speed_ack.ack.ack_index, 77);
        assert_eq!(speed_ack.speed, 7.5);

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(78);
        pkt.write_bit(true);
        pkt.write_float(8.0);
        pkt.write_float(9.0);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let knockback = MoveKnockBackAck::read(&mut pkt).unwrap();
        assert_eq!(knockback.ack.ack_index, 78);
        assert_eq!(
            knockback.speeds,
            Some(MoveKnockBackSpeeds {
                horz_speed: 8.0,
                vert_speed: 9.0
            })
        );

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(79);
        pkt.write_float(2.25);
        pkt.write_uint32(123);
        pkt.write_uint8(4);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let collision = MoveSetCollisionHeightAck::read(&mut pkt).unwrap();
        assert_eq!(collision.data.ack_index, 79);
        assert_eq!(collision.height, 2.25);
        assert_eq!(collision.mount_display_id, 123);
        assert_eq!(collision.reason, 4);
    }

    #[test]
    fn movement_time_spline_and_teleport_ack_packets_read_cpp_field_order() {
        let guid = ObjectGuid::create_player(1, 42);

        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&guid);
        pkt.write_uint32(250);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let skipped = MoveTimeSkipped::read(&mut pkt).unwrap();
        assert_eq!(skipped.mover_guid, guid);
        assert_eq!(skipped.time_skipped, 250);

        let info = MovementInfo {
            guid,
            time: 1234,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            ..MovementInfo::default()
        };
        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(9001);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let spline = MoveSplineDone::read(&mut pkt).unwrap();
        assert_eq!(spline.status.guid, guid);
        assert_eq!(spline.spline_id, 9001);

        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&guid);
        pkt.write_int32(11);
        pkt.write_int32(12);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let teleport = MoveTeleportAck::read(&mut pkt).unwrap();
        assert_eq!(teleport.mover_guid, guid);
        assert_eq!(teleport.ack_index, 11);
        assert_eq!(teleport.move_time, 12);
    }

    #[test]
    fn movement_force_ack_packets_read_cpp_field_order() {
        let player_guid = ObjectGuid::create_player(1, 42);
        let force_guid = ObjectGuid::create_world_object(HighGuid::GameObject, 0, 1, 0, 0, 9, 88);
        let info = MovementInfo {
            guid: player_guid,
            time: 1234,
            position: Position::new(1.0, 2.0, 3.0, 4.0),
            ..MovementInfo::default()
        };
        let force = MovementForce {
            id: force_guid,
            origin: [1.0, 2.0, 3.0],
            direction: [4.0, 5.0, 6.0],
            transport_id: 7,
            magnitude: 8.5,
            unused_910: 9,
            force_type: MovementForceType::Gravity,
        };

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(33);
        force.write(&mut pkt);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let apply = MoveApplyMovementForceAck::read(&mut pkt).unwrap();
        assert_eq!(apply.ack.status.guid, player_guid);
        assert_eq!(apply.ack.ack_index, 33);
        assert_eq!(apply.force, force);

        let mut pkt = WorldPacket::new_empty();
        info.write(&mut pkt);
        pkt.write_int32(34);
        pkt.write_packed_guid(&force_guid);
        let mut pkt = WorldPacket::from_bytes(pkt.data());
        let remove = MoveRemoveMovementForceAck::read(&mut pkt).unwrap();
        assert_eq!(remove.ack.status.guid, player_guid);
        assert_eq!(remove.ack.ack_index, 34);
        assert_eq!(remove.id, force_guid);
    }
}
