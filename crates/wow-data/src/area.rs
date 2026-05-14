// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Minimal AreaTable.db2 reader for C++ phasing area-parent checks.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;
use wow_database::{HotfixDatabase, HotfixStatements};

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaTableEntry {
    pub id: u32,
    pub parent_area_id: u16,
    pub mount_flags: i32,
    pub flags: u32,
}

#[derive(Debug, Clone, Default)]
pub struct AreaTableStore {
    entries: HashMap<u32, AreaTableEntry>,
}

pub const AREA_FLAG_IS_SUBZONE_LIKE_CPP: u32 = 0x4000_0000;

impl AreaTableEntry {
    pub fn is_subzone_like_cpp(&self) -> bool {
        self.flags & AREA_FLAG_IS_SUBZONE_LIKE_CPP != 0
    }
}

impl AreaTableStore {
    pub fn from_entries(entries: impl IntoIterator<Item = AreaTableEntry>) -> Self {
        Self {
            entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    /// Load AreaTable.db2 from `{data_dir}/dbc/{locale}/AreaTable.db2`.
    ///
    /// C++ refs:
    /// - `DB2Structure.h::AreaTableEntry`
    /// - `DB2LoadInfo.h::AreaTableLoadInfo`
    /// - `ObjectMgr::LoadAreaPhases`
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("AreaTable.db2");

        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = HashMap::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            entries.insert(
                id,
                AreaTableEntry {
                    id,
                    // WDC4 record ids supply C++ field 0 (`ID`), so
                    // `ParentAreaID` is DB2Meta field index 3.
                    parent_area_id: reader.get_field_u16(idx, 3),
                    // `MountFlags` is C++ field index 17, DB2Meta field index 16.
                    mount_flags: reader.get_field_i32(idx, 16),
                    // `Flags1` is C++ field index 22, DB2Meta field index 21
                    // when the record id supplies `ID`.
                    flags: reader.get_field_u32(idx, 21),
                },
            );
        }

        info!("Loaded {} areas from {}", entries.len(), path.display());
        Ok(Self { entries })
    }

    pub async fn load_with_hotfixes(
        data_dir: &str,
        locale: &str,
        hotfix_db: &HotfixDatabase,
    ) -> Result<Self> {
        let mut store = Self::load(data_dir, locale)?;
        let hotfix_rows = store.load_hotfix_rows(hotfix_db).await?;
        if hotfix_rows != 0 {
            info!("Loaded {hotfix_rows} AreaTable hotfix rows");
        }
        Ok(store)
    }

    async fn load_hotfix_rows(&mut self, db: &HotfixDatabase) -> Result<usize> {
        let stmt = db.prepare(HotfixStatements::SEL_AREA_TABLE);
        let mut result = db.query(&stmt).await?;
        if result.is_empty() {
            return Ok(0);
        }

        let mut count = 0usize;
        loop {
            let id: u32 = result.read(0);
            self.entries.insert(
                id,
                AreaTableEntry {
                    id,
                    parent_area_id: result.read(4),
                    mount_flags: result.read(17),
                    flags: result.read(22),
                },
            );
            count += 1;

            if !result.next_row() {
                break;
            }
        }
        Ok(count)
    }

    pub fn get(&self, id: u32) -> Option<&AreaTableEntry> {
        self.entries.get(&id)
    }

    pub fn contains(&self, id: u32) -> bool {
        self.entries.contains_key(&id)
    }

    /// C++ `DB2Manager::IsInArea(objectAreaId, areaId)`.
    pub fn is_in_area_like_cpp(&self, mut object_area_id: u32, area_id: u32) -> bool {
        loop {
            if object_area_id == area_id {
                return true;
            }

            let Some(object_area) = self.get(object_area_id) else {
                return false;
            };

            object_area_id = u32::from(object_area.parent_area_id);
            if object_area_id == 0 {
                return false;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_table_store_indexes_parent_area_like_cpp() {
        let store = AreaTableStore::from_entries([
            AreaTableEntry {
                id: 100,
                parent_area_id: 0,
                mount_flags: 0,
                flags: 0,
            },
            AreaTableEntry {
                id: 101,
                parent_area_id: 100,
                mount_flags: 0,
                flags: AREA_FLAG_IS_SUBZONE_LIKE_CPP,
            },
        ]);

        assert!(store.contains(100));
        assert_eq!(store.get(101).map(|area| area.parent_area_id), Some(100));
        assert!(store.is_in_area_like_cpp(101, 100));
        assert!(store.is_in_area_like_cpp(101, 101));
        assert!(!store.is_in_area_like_cpp(101, 999));
        assert_eq!(
            store.get(101).map(|area| area.is_subzone_like_cpp()),
            Some(true)
        );
    }

    #[test]
    fn load_area_table_db2_when_fixture_exists() {
        let data_dir = "/home/server/woltk-server-core/Data";
        let locale = "esES";
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("AreaTable.db2");
        if !path.exists() {
            eprintln!(
                "Skipping test: AreaTable.db2 not found at {}",
                path.display()
            );
            return;
        }

        let store = AreaTableStore::load(data_dir, locale).expect("failed to load AreaTable.db2");
        assert!(!store.is_empty());
    }
}
