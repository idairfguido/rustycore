// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Instance packet definitions.

use crate::{ServerPacket, WorldPacket};
use wow_constants::ServerOpcodes;

/// C++ `WorldPackets::Instance::InstanceLock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceLockInfo {
    pub instance_id: u64,
    pub map_id: u32,
    pub difficulty_id: u32,
    pub time_remaining: i32,
    pub completed_mask: u32,
    pub locked: bool,
    pub extended: bool,
}

impl InstanceLockInfo {
    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_uint32(self.map_id);
        pkt.write_uint32(self.difficulty_id);
        pkt.write_uint64(self.instance_id);
        pkt.write_int32(self.time_remaining);
        pkt.write_uint32(self.completed_mask);
        pkt.write_bit(self.locked);
        pkt.write_bit(self.extended);
        pkt.flush_bits();
    }
}

/// C++ `WorldPackets::Instance::InstanceInfo` / `SMSG_INSTANCE_INFO`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstanceInfo {
    pub locks: Vec<InstanceLockInfo>,
}

impl ServerPacket for InstanceInfo {
    const OPCODE: ServerOpcodes = ServerOpcodes::InstanceInfo;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_int32(self.locks.len() as i32);
        for lock in &self.locks {
            lock.write(pkt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_instance_info_matches_cpp_lock_count_only() {
        let mut pkt = WorldPacket::new_server(ServerOpcodes::InstanceInfo);

        InstanceInfo::default().write(&mut pkt);

        assert_eq!(&pkt.data()[2..], &[0, 0, 0, 0]);
    }

    #[test]
    fn instance_info_lock_serialization_matches_cpp_order_and_bits() {
        let mut pkt = WorldPacket::new_server(ServerOpcodes::InstanceInfo);

        InstanceInfo {
            locks: vec![InstanceLockInfo {
                instance_id: 0x0102_0304_0506_0708,
                map_id: 631,
                difficulty_id: 3,
                time_remaining: 3600,
                completed_mask: 0xA5,
                locked: true,
                extended: false,
            }],
        }
        .write(&mut pkt);

        assert_eq!(
            &pkt.data()[2..],
            &[
                1, 0, 0, 0, // LockList.size()
                0x77, 0x02, 0x00, 0x00, // MapID
                0x03, 0x00, 0x00, 0x00, // DifficultyID
                0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // InstanceID
                0x10, 0x0E, 0x00, 0x00, // TimeRemaining
                0xA5, 0x00, 0x00, 0x00, // CompletedMask
                0x80, // Locked=true, Extended=false
            ]
        );
    }
}
