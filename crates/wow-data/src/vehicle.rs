//! Vehicle.db2 / VehicleSeat.db2 readers for C++ `VehicleKit` parity.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;
use wow_database::{HotfixDatabase, HotfixStatements};
use wow_entities::{VehicleSeatAddon, VehicleSeatInfo};

use crate::wdc4::Wdc4Reader;

pub const MAX_VEHICLE_SEATS_LIKE_CPP: usize = 8;
pub const VEHICLE_SEAT_FLAG_SHOULD_USE_VEH_SEAT_EXIT_ANIM_ON_VOLUNTARY_EXIT: i32 = 0x0000_0008;
pub const VEHICLE_SEAT_FLAG_CAN_CONTROL: i32 = 0x0000_0800;
pub const VEHICLE_SEAT_FLAG_UNCONTROLLED: i32 = 0x0000_2000;
pub const VEHICLE_SEAT_FLAG_UNK18: i32 = 0x0002_0000;
pub const VEHICLE_SEAT_FLAG_CAN_ENTER_OR_EXIT: i32 = 0x0200_0000;
pub const VEHICLE_SEAT_FLAG_B_USABLE_FORCED: i32 = 0x0000_0002;
pub const VEHICLE_SEAT_FLAG_B_USABLE_FORCED_2: i32 = 0x0000_0040;
pub const VEHICLE_SEAT_FLAG_B_USABLE_FORCED_3: i32 = 0x0000_0100;
pub const VEHICLE_SEAT_FLAG_B_USABLE_FORCED_4: i32 = 0x0200_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleEntry {
    pub id: u32,
    pub flags: i32,
    pub flags_b: i32,
    pub seat_ids: [u16; MAX_VEHICLE_SEATS_LIKE_CPP],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleSeatEntry {
    pub id: u32,
    pub flags: i32,
    pub flags_b: i32,
    pub flags_c: i32,
}

impl VehicleSeatEntry {
    pub fn has_flag(&self, flag: i32) -> bool {
        self.flags & flag != 0
    }

    pub fn has_flag_b(&self, flag: i32) -> bool {
        self.flags_b & flag != 0
    }

    pub fn can_enter_or_exit_like_cpp(&self) -> bool {
        self.has_flag(
            VEHICLE_SEAT_FLAG_CAN_ENTER_OR_EXIT
                | VEHICLE_SEAT_FLAG_CAN_CONTROL
                | VEHICLE_SEAT_FLAG_SHOULD_USE_VEH_SEAT_EXIT_ANIM_ON_VOLUNTARY_EXIT,
        )
    }

    pub fn usable_by_override_like_cpp(&self) -> bool {
        self.has_flag(VEHICLE_SEAT_FLAG_UNCONTROLLED | VEHICLE_SEAT_FLAG_UNK18)
            || self.has_flag_b(
                VEHICLE_SEAT_FLAG_B_USABLE_FORCED
                    | VEHICLE_SEAT_FLAG_B_USABLE_FORCED_2
                    | VEHICLE_SEAT_FLAG_B_USABLE_FORCED_3
                    | VEHICLE_SEAT_FLAG_B_USABLE_FORCED_4,
            )
    }

    pub fn to_vehicle_seat_info_like_cpp(&self) -> VehicleSeatInfo {
        VehicleSeatInfo {
            id: self.id,
            can_enter_or_exit: self.can_enter_or_exit_like_cpp(),
            usable_by_override: self.usable_by_override_like_cpp(),
        }
    }
}

pub struct VehicleStore {
    by_id: HashMap<u32, VehicleEntry>,
}

pub struct VehicleSeatStore {
    by_id: HashMap<u32, VehicleSeatEntry>,
}

impl VehicleStore {
    pub fn from_entries(entries: impl IntoIterator<Item = VehicleEntry>) -> Self {
        Self {
            by_id: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("Vehicle.db2");
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = Vec::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            let mut seat_ids = [0u16; MAX_VEHICLE_SEATS_LIKE_CPP];
            for (offset, seat_id) in seat_ids.iter_mut().enumerate() {
                *seat_id = reader.get_field_u16(idx, 17 + offset);
            }
            entries.push(VehicleEntry {
                id,
                flags: reader.get_field_i32(idx, 1),
                flags_b: reader.get_field_i32(idx, 2),
                seat_ids,
            });
        }

        let store = Self::from_entries(entries);
        info!("Loaded {} vehicles from {}", store.len(), path.display());
        Ok(store)
    }

    pub async fn load_with_hotfixes(
        data_dir: &str,
        locale: &str,
        hotfix_db: &HotfixDatabase,
    ) -> Result<Self> {
        let mut store = Self::load(data_dir, locale)?;
        let hotfix_rows = store.load_hotfix_rows(hotfix_db).await?;
        if hotfix_rows != 0 {
            info!("Loaded {hotfix_rows} Vehicle hotfix rows");
        }
        Ok(store)
    }

    async fn load_hotfix_rows(&mut self, db: &HotfixDatabase) -> Result<usize> {
        let stmt = db.prepare(HotfixStatements::SEL_VEHICLE);
        let mut result = db.query(&stmt).await?;
        if result.is_empty() {
            return Ok(0);
        }

        let mut count = 0usize;
        loop {
            let mut seat_ids = [0u16; MAX_VEHICLE_SEATS_LIKE_CPP];
            for (offset, seat_id) in seat_ids.iter_mut().enumerate() {
                *seat_id = result.read(18 + offset);
            }
            let entry = VehicleEntry {
                id: result.read(0),
                flags: result.read(1),
                flags_b: result.read(2),
                seat_ids,
            };
            self.by_id.insert(entry.id, entry);
            count += 1;

            if !result.next_row() {
                break;
            }
        }
        Ok(count)
    }

    pub fn get(&self, id: u32) -> Option<&VehicleEntry> {
        self.by_id.get(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

impl VehicleSeatStore {
    pub fn from_entries(entries: impl IntoIterator<Item = VehicleSeatEntry>) -> Self {
        Self {
            by_id: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("VehicleSeat.db2");
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = Vec::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            entries.push(VehicleSeatEntry {
                id,
                flags: reader.get_field_i32(idx, 7),
                flags_b: reader.get_field_i32(idx, 8),
                flags_c: reader.get_field_i32(idx, 9),
            });
        }

        let store = Self::from_entries(entries);
        info!(
            "Loaded {} vehicle seats from {}",
            store.len(),
            path.display()
        );
        Ok(store)
    }

    pub async fn load_with_hotfixes(
        data_dir: &str,
        locale: &str,
        hotfix_db: &HotfixDatabase,
    ) -> Result<Self> {
        let mut store = Self::load(data_dir, locale)?;
        let hotfix_rows = store.load_hotfix_rows(hotfix_db).await?;
        if hotfix_rows != 0 {
            info!("Loaded {hotfix_rows} VehicleSeat hotfix rows");
        }
        Ok(store)
    }

    async fn load_hotfix_rows(&mut self, db: &HotfixDatabase) -> Result<usize> {
        let stmt = db.prepare(HotfixStatements::SEL_VEHICLE_SEAT);
        let mut result = db.query(&stmt).await?;
        if result.is_empty() {
            return Ok(0);
        }

        let mut count = 0usize;
        loop {
            let entry = VehicleSeatEntry {
                id: result.read(0),
                flags: result.read(7),
                flags_b: result.read(8),
                flags_c: result.read(9),
            };
            self.by_id.insert(entry.id, entry);
            count += 1;

            if !result.next_row() {
                break;
            }
        }
        Ok(count)
    }

    pub fn get(&self, id: u32) -> Option<&VehicleSeatEntry> {
        self.by_id.get(&id)
    }

    pub fn seat_defs_for_vehicle_like_cpp(
        &self,
        vehicle: &VehicleEntry,
    ) -> Vec<(i8, VehicleSeatInfo, VehicleSeatAddon)> {
        vehicle
            .seat_ids
            .iter()
            .enumerate()
            .filter_map(|(seat_index, seat_id)| {
                let seat = self.get(u32::from(*seat_id))?;
                Some((
                    i8::try_from(seat_index).ok()?,
                    seat.to_vehicle_seat_info_like_cpp(),
                    VehicleSeatAddon::default(),
                ))
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicle_seat_flags_match_cpp_helpers() {
        let control = VehicleSeatEntry {
            id: 1,
            flags: VEHICLE_SEAT_FLAG_CAN_CONTROL,
            flags_b: 0,
            flags_c: 0,
        };
        assert!(control.can_enter_or_exit_like_cpp());
        assert!(!control.usable_by_override_like_cpp());

        let forced = VehicleSeatEntry {
            id: 2,
            flags: 0,
            flags_b: VEHICLE_SEAT_FLAG_B_USABLE_FORCED_3,
            flags_c: 0,
        };
        assert!(!forced.can_enter_or_exit_like_cpp());
        assert!(forced.usable_by_override_like_cpp());
    }

    #[test]
    fn vehicle_seat_defs_follow_cpp_seat_order() {
        let vehicles = VehicleStore::from_entries([VehicleEntry {
            id: 10,
            flags: 0,
            flags_b: 0,
            seat_ids: [100, 0, 101, 0, 0, 0, 0, 0],
        }]);
        let seats = VehicleSeatStore::from_entries([
            VehicleSeatEntry {
                id: 100,
                flags: VEHICLE_SEAT_FLAG_CAN_ENTER_OR_EXIT,
                flags_b: 0,
                flags_c: 0,
            },
            VehicleSeatEntry {
                id: 101,
                flags: 0,
                flags_b: VEHICLE_SEAT_FLAG_B_USABLE_FORCED,
                flags_c: 0,
            },
        ]);

        let defs = seats.seat_defs_for_vehicle_like_cpp(vehicles.get(10).unwrap());
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].0, 0);
        assert_eq!(defs[0].1.id, 100);
        assert!(defs[0].1.can_enter_or_exit);
        assert_eq!(defs[1].0, 2);
        assert_eq!(defs[1].1.id, 101);
        assert!(defs[1].1.usable_by_override);
    }
}
