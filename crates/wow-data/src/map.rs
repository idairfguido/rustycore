// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Map.db2 and MapDifficulty.db2 readers.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

pub const MAP_FLAG_FLEXIBLE_RAID_LOCKING: u32 = 0x0000_8000;
pub const MAP_DIFFICULTY_FLAG_USE_LOOT_BASED_LOCK: u8 = 0x02;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapEntry {
    pub id: u32,
    pub instance_type: i8,
    pub flags1: u32,
}

impl MapEntry {
    pub const fn is_flex_locking(self) -> bool {
        self.flags1 & MAP_FLAG_FLEXIBLE_RAID_LOCKING != 0
    }
}

pub struct MapStore {
    entries: HashMap<u32, MapEntry>,
}

impl MapStore {
    pub fn from_entries(entries: impl IntoIterator<Item = MapEntry>) -> Self {
        Self {
            entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    /// Load Map.db2 from `{data_dir}/dbc/{locale}/Map.db2`.
    ///
    /// C++ refs:
    /// - `DB2Structure.h::MapEntry`
    /// - `DB2LoadInfo.h::MapLoadInfo`
    /// - `DB2Stores.cpp::sMapStore`
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir).join("dbc").join(locale).join("Map.db2");
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = HashMap::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            let entry = MapEntry {
                id,
                // WDC4 record ids supply C++ field 0 (`ID`) and this reader
                // exposes `Flags[3]` as one array field, so C++ field 8 -> 7
                // and C++ fields 22..24 -> field 21.
                instance_type: reader.get_field_i8(idx, 7),
                flags1: reader.get_field_u32(idx, 21),
            };
            entries.insert(id, entry);
        }

        info!("Loaded {} maps from {}", entries.len(), path.display());
        Ok(Self { entries })
    }

    pub fn get(&self, id: u32) -> Option<&MapEntry> {
        self.entries.get(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapDifficultyEntry {
    pub id: u32,
    pub map_id: u32,
    pub difficulty_id: u8,
    pub lock_id: u8,
    pub reset_interval: u8,
    pub flags: u8,
}

impl MapDifficultyEntry {
    pub const fn is_using_encounter_locks(self) -> bool {
        self.flags & MAP_DIFFICULTY_FLAG_USE_LOOT_BASED_LOCK != 0
    }
}

pub struct MapDifficultyStore {
    by_id: HashMap<u32, MapDifficultyEntry>,
    by_map_difficulty: HashMap<(u32, u8), u32>,
}

impl MapDifficultyStore {
    pub fn from_entries(entries: impl IntoIterator<Item = MapDifficultyEntry>) -> Self {
        let mut by_id = HashMap::new();
        let mut by_map_difficulty = HashMap::new();
        for entry in entries {
            by_map_difficulty.insert((entry.map_id, entry.difficulty_id), entry.id);
            by_id.insert(entry.id, entry);
        }

        Self {
            by_id,
            by_map_difficulty,
        }
    }

    /// Load MapDifficulty.db2 from `{data_dir}/dbc/{locale}/MapDifficulty.db2`.
    ///
    /// C++ refs:
    /// - `DB2Structure.h::MapDifficultyEntry`
    /// - `DB2LoadInfo.h::MapDifficultyLoadInfo`
    /// - `DB2Manager::GetMapDifficultyData`
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("MapDifficulty.db2");
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = Vec::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            entries.push(MapDifficultyEntry {
                id,
                // WDC4 record ids supply C++ field 0 (`ID`) and this reader
                // exposes the numeric payload in physical order:
                // ContentTuning, ItemContextPicker, ItemContext,
                // DifficultyID, LockID, ResetInterval, MaxPlayers, Flags, MapID.
                difficulty_id: reader.get_field_u8(idx, 3),
                lock_id: reader.get_field_u8(idx, 4),
                reset_interval: reader.get_field_u8(idx, 5),
                flags: reader.get_field_u8(idx, 7),
                map_id: reader.get_field_u32(idx, 8),
            });
        }

        let store = Self::from_entries(entries);
        info!(
            "Loaded {} map difficulties from {}",
            store.len(),
            path.display()
        );
        Ok(store)
    }

    pub fn get(&self, map_id: u32, difficulty_id: u8) -> Option<&MapDifficultyEntry> {
        self.by_map_difficulty
            .get(&(map_id, difficulty_id))
            .and_then(|id| self.by_id.get(id))
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
    fn map_store_flex_locking_flag_matches_cpp() {
        let store = MapStore::from_entries([MapEntry {
            id: 631,
            instance_type: 2,
            flags1: MAP_FLAG_FLEXIBLE_RAID_LOCKING,
        }]);

        assert!(store.get(631).unwrap().is_flex_locking());
        assert!(store.get(1).is_none());
    }

    #[test]
    fn map_difficulty_store_indexes_by_map_and_difficulty_like_cpp() {
        let store = MapDifficultyStore::from_entries([MapDifficultyEntry {
            id: 900,
            map_id: 631,
            difficulty_id: 4,
            lock_id: 7,
            reset_interval: 2,
            flags: MAP_DIFFICULTY_FLAG_USE_LOOT_BASED_LOCK,
        }]);

        let entry = store.get(631, 4).unwrap();
        assert_eq!(entry.lock_id, 7);
        assert!(entry.is_using_encounter_locks());
        assert!(store.get(631, 3).is_none());
    }

    #[test]
    fn load_map_and_map_difficulty_db2_when_fixtures_exist() {
        let data_dir = "/home/server/woltk-server-core/Data";
        let locale = "esES";
        let dbc_dir = Path::new(data_dir).join("dbc").join(locale);
        if !dbc_dir.join("Map.db2").exists() || !dbc_dir.join("MapDifficulty.db2").exists() {
            eprintln!("Skipping test: Map.db2/MapDifficulty.db2 not found");
            return;
        }

        let maps = MapStore::load(data_dir, locale).expect("failed to load maps");
        let difficulties =
            MapDifficultyStore::load(data_dir, locale).expect("failed to load map difficulties");

        assert!(!maps.is_empty());
        assert!(!difficulties.is_empty());
        assert!(maps.get(0).is_some());

        let icc = maps.get(631).expect("Icecrown Citadel map missing");
        assert_eq!(icc.instance_type, 2);

        let known_difficulty = difficulties
            .get(32, 4)
            .expect("known MapDifficulty row for map 32 difficulty 4 missing");
        assert_eq!(known_difficulty.map_id, 32);
        assert_eq!(known_difficulty.difficulty_id, 4);
        assert_eq!(known_difficulty.reset_interval, 2);
    }
}
