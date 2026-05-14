// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Difficulty.db2 reader.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

/// Minimal C++ `DifficultyEntry` store for `sDifficultyStore.LookupEntry`.
pub struct DifficultyStore {
    ids: HashSet<u32>,
}

impl DifficultyStore {
    pub fn from_ids(ids: impl IntoIterator<Item = u32>) -> Self {
        Self {
            ids: ids.into_iter().collect(),
        }
    }

    /// Load Difficulty.db2 from `{data_dir}/dbc/{locale}/Difficulty.db2`.
    ///
    /// C++ refs:
    /// - `DB2Stores.cpp::sDifficultyStore`
    /// - `ConditionMgr::isConditionTypeValid(CONDITION_DIFFICULTY_ID)`
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("Difficulty.db2");

        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        let mut ids = HashSet::with_capacity(reader.total_count());
        for (id, _) in reader.iter_records() {
            ids.insert(id);
        }

        info!("Loaded {} difficulties from {}", ids.len(), path.display());
        Ok(Self { ids })
    }

    pub fn contains(&self, id: u32) -> bool {
        self.ids.contains(&id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_store_indexes_record_ids_like_cpp_store() {
        let store = DifficultyStore::from_ids([0, 1, 23]);

        assert!(store.contains(1));
        assert!(!store.contains(2));
        assert_eq!(store.len(), 3);
    }
}
