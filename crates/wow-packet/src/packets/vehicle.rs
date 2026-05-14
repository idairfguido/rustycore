// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Vehicle packet definitions.

use wow_constants::ServerOpcodes;
use wow_core::ObjectGuid;

use crate::packets::movement::{MovementAck, MovementInfo};
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

/// Mirrors C++ `WorldPackets::Vehicle::MoveDismissVehicle`.
#[derive(Debug, Clone)]
pub struct MoveDismissVehicle {
    pub status: MovementInfo,
}

impl ClientPacket for MoveDismissVehicle {
    const OPCODE: wow_constants::ClientOpcodes = wow_constants::ClientOpcodes::MoveDismissVehicle;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            status: MovementInfo::read(packet)?,
        })
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::RequestVehiclePrevSeat`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestVehiclePrevSeat;

impl ClientPacket for RequestVehiclePrevSeat {
    const OPCODE: wow_constants::ClientOpcodes =
        wow_constants::ClientOpcodes::RequestVehiclePrevSeat;

    fn read(_packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::RequestVehicleNextSeat`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestVehicleNextSeat;

impl ClientPacket for RequestVehicleNextSeat {
    const OPCODE: wow_constants::ClientOpcodes =
        wow_constants::ClientOpcodes::RequestVehicleNextSeat;

    fn read(_packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::MoveChangeVehicleSeats`.
#[derive(Debug, Clone)]
pub struct MoveChangeVehicleSeats {
    pub status: MovementInfo,
    pub dst_vehicle: ObjectGuid,
    pub dst_seat_index: u8,
}

impl ClientPacket for MoveChangeVehicleSeats {
    const OPCODE: wow_constants::ClientOpcodes =
        wow_constants::ClientOpcodes::MoveChangeVehicleSeats;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            status: MovementInfo::read(packet)?,
            dst_vehicle: packet.read_packed_guid()?,
            dst_seat_index: packet.read_uint8()?,
        })
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::RequestVehicleSwitchSeat`.
#[derive(Debug, Clone, Copy)]
pub struct RequestVehicleSwitchSeat {
    pub vehicle: ObjectGuid,
    pub seat_index: u8,
}

impl ClientPacket for RequestVehicleSwitchSeat {
    const OPCODE: wow_constants::ClientOpcodes =
        wow_constants::ClientOpcodes::RequestVehicleSwitchSeat;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            vehicle: packet.read_packed_guid()?,
            seat_index: packet.read_uint8()?,
        })
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::RideVehicleInteract`.
#[derive(Debug, Clone, Copy)]
pub struct RideVehicleInteract {
    pub vehicle: ObjectGuid,
}

impl ClientPacket for RideVehicleInteract {
    const OPCODE: wow_constants::ClientOpcodes = wow_constants::ClientOpcodes::RideVehicleInteract;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            vehicle: packet.read_packed_guid()?,
        })
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::EjectPassenger`.
#[derive(Debug, Clone, Copy)]
pub struct EjectPassenger {
    pub passenger: ObjectGuid,
}

impl ClientPacket for EjectPassenger {
    const OPCODE: wow_constants::ClientOpcodes = wow_constants::ClientOpcodes::EjectPassenger;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self {
            passenger: packet.read_packed_guid()?,
        })
    }
}

/// Mirrors C++ `WorldPackets::Vehicle::RequestVehicleExit`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestVehicleExit;

impl ClientPacket for RequestVehicleExit {
    const OPCODE: wow_constants::ClientOpcodes = wow_constants::ClientOpcodes::RequestVehicleExit;

    fn read(_packet: &mut WorldPacket) -> Result<Self, PacketError> {
        Ok(Self)
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

    #[test]
    fn request_vehicle_switch_seat_reads_cpp_layout() {
        let guid = ObjectGuid::create_player(1, 42);
        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&guid);
        pkt.write_uint8(3);

        let parsed = RequestVehicleSwitchSeat::read(&mut pkt).unwrap();

        assert_eq!(parsed.vehicle, guid);
        assert_eq!(parsed.seat_index, 3);
    }

    #[test]
    fn ride_vehicle_interact_and_eject_passenger_read_packed_guid() {
        let guid = ObjectGuid::create_player(1, 42);
        let mut ride = WorldPacket::new_empty();
        ride.write_packed_guid(&guid);
        let mut eject = WorldPacket::new_empty();
        eject.write_packed_guid(&guid);

        assert_eq!(RideVehicleInteract::read(&mut ride).unwrap().vehicle, guid);
        assert_eq!(EjectPassenger::read(&mut eject).unwrap().passenger, guid);
    }
}
