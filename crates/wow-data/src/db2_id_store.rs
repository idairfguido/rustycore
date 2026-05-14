// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Minimal DB2 ID stores for C++ `LookupEntry(id)` validation.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone)]
pub struct Db2IdStore {
    name: &'static str,
    ids: HashSet<u32>,
}

impl Db2IdStore {
    pub fn from_ids(name: &'static str, ids: impl IntoIterator<Item = u32>) -> Self {
        Self {
            name,
            ids: ids.into_iter().collect(),
        }
    }

    pub fn load(data_dir: &str, locale: &str, filename: &'static str) -> Result<Self> {
        let path = Path::new(data_dir).join("dbc").join(locale).join(filename);
        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        let mut ids = HashSet::with_capacity(reader.total_count());
        for (id, _) in reader.iter_records() {
            ids.insert(id);
        }

        info!("Loaded {} rows from {}", ids.len(), path.display());
        Ok(Self {
            name: filename,
            ids,
        })
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

    pub const fn name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db2_id_store_indexes_record_ids_like_cpp_store() {
        let store = Db2IdStore::from_ids("Example.db2", [1, 7, 42]);

        assert_eq!(store.name(), "Example.db2");
        assert!(store.contains(7));
        assert!(!store.contains(8));
        assert_eq!(store.len(), 3);
    }
}
