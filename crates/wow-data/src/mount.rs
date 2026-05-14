// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Mount.db2 reader and C++ `DB2Manager::GetMount` lookup helpers.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone, PartialEq)]
pub struct MountEntry {
    pub id: u32,
    pub mount_type_id: u16,
    pub flags: u16,
    pub source_type_enum: i8,
    pub source_spell_id: i32,
    pub player_condition_id: u32,
    pub mount_fly_ride_height: f32,
    pub ui_model_scene_id: i32,
}

pub struct MountStore {
    by_id: HashMap<u32, MountEntry>,
    by_source_spell_id: HashMap<u32, u32>,
}

impl MountStore {
    pub fn from_entries(entries: impl IntoIterator<Item = MountEntry>) -> Self {
        let mut by_id = HashMap::new();
        let mut by_source_spell_id = HashMap::new();
        for entry in entries {
            if let Ok(source_spell_id) = u32::try_from(entry.source_spell_id) {
                by_source_spell_id.insert(source_spell_id, entry.id);
            }
            by_id.insert(entry.id, entry);
        }

        Self {
            by_id,
            by_source_spell_id,
        }
    }

    /// Load Mount.db2 from `{data_dir}/dbc/{locale}/Mount.db2`.
    ///
    /// C++ refs:
    /// - `DB2Structure.h::MountEntry`
    /// - `DB2LoadInfo.h::MountLoadInfo`
    /// - `DB2Stores.cpp` `_mountsBySpellId[mount->SourceSpellID] = mount`.
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("Mount.db2");
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = Vec::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            entries.push(MountEntry {
                id,
                mount_type_id: reader.get_field_u16(idx, 3),
                flags: reader.get_field_u16(idx, 4),
                source_type_enum: reader.get_field_i8(idx, 5),
                source_spell_id: reader.get_field_i32(idx, 6),
                player_condition_id: reader.get_field_u32(idx, 7),
                mount_fly_ride_height: f32::from_bits(reader.get_field_u32(idx, 8)),
                ui_model_scene_id: reader.get_field_i32(idx, 9),
            });
        }

        let store = Self::from_entries(entries);
        info!("Loaded {} mounts from {}", store.len(), path.display());
        Ok(store)
    }

    pub fn get_by_id(&self, id: u32) -> Option<&MountEntry> {
        self.by_id.get(&id)
    }

    pub fn get_by_source_spell_id_like_cpp(&self, spell_id: u32) -> Option<&MountEntry> {
        self.by_source_spell_id
            .get(&spell_id)
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
    fn mount_store_indexes_by_source_spell_like_cpp() {
        let store = MountStore::from_entries([
            MountEntry {
                id: 1,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: 100,
                player_condition_id: 42,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
            MountEntry {
                id: 2,
                mount_type_id: 0,
                flags: 0,
                source_type_enum: 0,
                source_spell_id: -1,
                player_condition_id: 0,
                mount_fly_ride_height: 0.0,
                ui_model_scene_id: 0,
            },
        ]);

        let mount = store.get_by_source_spell_id_like_cpp(100).unwrap();
        assert_eq!(mount.id, 1);
        assert_eq!(mount.player_condition_id, 42);
        assert!(store.get_by_source_spell_id_like_cpp(999).is_none());
    }
}
