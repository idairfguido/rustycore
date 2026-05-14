// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Vehicle packet definitions.

use wow_constants::ServerOpcodes;
use wow_core::ObjectGuid;

use crate::packets::movement::MovementAck;
use crate::{ClientPacket, PacketError, ServerPacket, WorldPacket};

/// Mirrors C++ `WorldPackets::Vehicle::MoveSetVehicleRecID`.
pub struct MoveSetVehicleRecId {
    pub mover_guid: ObjectGuid,
    pub sequence_index: u32,
    pub vehicle_rec_id: i32,
}

impl ServerPacket for MoveSetVehicleRecId {
    const OPCODE: ServerOpcodes = ServerOpcodes::MoveSetVehicleRecId;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.mover_guid);
        pkt.write_uint32(self.sequence_index);
        pkt.write_int32(self.vehicle_rec_id);
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::SetVehicleRecID`.
pub struct SetVehicleRecId {
    pub vehicle_guid: ObjectGuid,
    pub vehicle_rec_id: i32,
}

impl ServerPacket for SetVehicleRecId {
    const OPCODE: ServerOpcodes = ServerOpcodes::SetVehicleRecId;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.vehicle_guid);
        pkt.write_int32(self.vehicle_rec_id);
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::OnCancelExpectedRideVehicleAura`.
pub struct OnCancelExpectedRideVehicleAura;

impl ServerPacket for OnCancelExpectedRideVehicleAura {
    const OPCODE: ServerOpcodes = ServerOpcodes::OnCancelExpectedRideVehicleAura;

    fn write(&self, _pkt: &mut WorldPacket) {}
}

/// Mirrors C++ `WorldPackets::Vehicle::MoveSetVehicleRecIdAck`.
#[derive(Debug, Clone)]
pub struct MoveSetVehicleRecIdAck {
    pub data: MovementAck,
    pub vehicle_rec_id: i32,
}

impl ClientPacket for MoveSetVehicleRecIdAck {
    const OPCODE: wow_constants::ClientOpcodes =
        wow_constants::ClientOpcodes::MoveSetVehicleRecIdAck;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            data: MovementAck::read(packet)?,
            vehicle_rec_id: packet.read_int32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_set_vehicle_rec_id_matches_cpp_opcode_and_tail() {
        let guid = ObjectGuid::create_player(1, 42);
        let pkt = MoveSetVehicleRecId {
            mover_guid: guid,
            sequence_index: 7,
            vehicle_rec_id: 55,
        };
        let bytes = pkt.to_bytes();

        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x2e14);
        assert!(bytes.len() > 10);
        assert_eq!(
            &bytes[bytes.len() - 8..bytes.len() - 4],
            &7u32.to_le_bytes()
        );
        assert_eq!(&bytes[bytes.len() - 4..], &55i32.to_le_bytes());
    }

    #[test]
    fn set_vehicle_rec_id_matches_cpp_opcode_and_tail() {
        let guid = ObjectGuid::create_player(1, 42);
        let pkt = SetVehicleRecId {
            vehicle_guid: guid,
            vehicle_rec_id: 55,
        };
        let bytes = pkt.to_bytes();

        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x26f7);
        assert!(bytes.len() > 6);
        assert_eq!(&bytes[bytes.len() - 4..], &55i32.to_le_bytes());
    }

    #[test]
    fn on_cancel_expected_ride_vehicle_aura_matches_cpp_empty_packet() {
        let bytes = OnCancelExpectedRideVehicleAura.to_bytes();

        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x26e6);
        assert_eq!(bytes.len(), 2);
    }
}
