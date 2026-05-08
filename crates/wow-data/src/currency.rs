// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! CurrencyTypes.db2 reader.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;
use wow_constants::{CurrencyTypesFlags, CurrencyTypesFlagsB};

use crate::wdc4::Wdc4Reader;

/// C++ `CurrencyTypesEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyTypesEntry {
    pub id: u32,
    pub category_id: u8,
    pub inventory_icon_file_id: i32,
    pub spell_weight: u32,
    pub spell_category: u8,
    pub max_qty: u32,
    pub max_earnable_per_week: u32,
    pub quality: i8,
    pub faction_id: i32,
    pub award_condition_id: i32,
    pub flags: CurrencyTypesFlags,
    pub flags_b: CurrencyTypesFlagsB,
}

impl CurrencyTypesEntry {
    pub fn scaler(&self) -> i32 {
        if self.flags.contains(CurrencyTypesFlags::SCALER_100) {
            100
        } else {
            1
        }
    }

    pub fn has_max_earnable_per_week(&self) -> bool {
        self.max_earnable_per_week != 0
            || self
                .flags
                .contains(CurrencyTypesFlags::COMPUTED_WEEKLY_MAXIMUM)
    }

    pub fn has_max_quantity(&self, on_load: bool, on_update_version: bool) -> bool {
        if on_load
            && self
                .flags
                .contains(CurrencyTypesFlags::IGNORE_MAX_QTY_ON_LOAD)
        {
            return false;
        }

        if on_update_version
            && self
                .flags
                .contains(CurrencyTypesFlags::UPDATE_VERSION_IGNORE_MAX)
        {
            return false;
        }

        self.max_qty != 0 || self.flags.contains(CurrencyTypesFlags::DYNAMIC_MAXIMUM)
    }

    pub fn has_total_earned(&self) -> bool {
        self.flags_b
            .contains(CurrencyTypesFlagsB::USE_TOTAL_EARNED_FOR_EARNED)
    }

    pub fn is_alliance(&self) -> bool {
        self.flags.contains(CurrencyTypesFlags::IS_ALLIANCE_ONLY)
    }

    pub fn is_horde(&self) -> bool {
        self.flags.contains(CurrencyTypesFlags::IS_HORDE_ONLY)
    }

    pub fn is_suppressing_chat_log(&self, on_update_version: bool) -> bool {
        (on_update_version
            && self
                .flags
                .contains(CurrencyTypesFlags::SUPPRESS_CHAT_MESSAGE_ON_VERSION_CHANGE))
            || self
                .flags
                .contains(CurrencyTypesFlags::SUPPRESS_CHAT_MESSAGES)
    }

    pub fn is_tracking_quantity(&self) -> bool {
        self.flags.contains(CurrencyTypesFlags::TRACK_QUANTITY)
    }
}

/// In-memory store for `CurrencyTypes.db2`.
pub struct CurrencyTypesStore {
    entries: HashMap<u32, CurrencyTypesEntry>,
}

impl CurrencyTypesStore {
    pub fn from_entries(entries: impl IntoIterator<Item = CurrencyTypesEntry>) -> Self {
        Self {
            entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
        }
    }

    /// Load CurrencyTypes.db2 from `{data_dir}/dbc/{locale}/CurrencyTypes.db2`.
    ///
    /// C++ refs:
    /// - `DB2Structure.h::CurrencyTypesEntry`
    /// - `DB2LoadInfo.h::CurrencyTypesLoadInfo`
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("CurrencyTypes.db2");

        let reader = Wdc4Reader::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;

        let mut entries = HashMap::with_capacity(reader.total_count());
        for (id, idx) in reader.iter_records() {
            let record = CurrencyTypesEntry {
                id,
                category_id: reader.get_field_u8(idx, 2),
                inventory_icon_file_id: reader.get_field_i32(idx, 3),
                spell_weight: reader.get_field_u32(idx, 4),
                spell_category: reader.get_field_u8(idx, 5),
                max_qty: reader.get_field_u32(idx, 6),
                max_earnable_per_week: reader.get_field_u32(idx, 7),
                quality: reader.get_field_i8(idx, 8),
                faction_id: reader.get_field_i32(idx, 9),
                award_condition_id: reader.get_field_i32(idx, 10),
                flags: CurrencyTypesFlags::from_bits_retain(
                    reader.get_array_element(idx, 11, 0, 32),
                ),
                flags_b: CurrencyTypesFlagsB::from_bits_retain(
                    reader.get_array_element(idx, 11, 1, 32),
                ),
            };
            entries.insert(id, record);
        }

        info!(
            "Loaded {} currencies from {}",
            entries.len(),
            path.display()
        );
        Ok(Self { entries })
    }

    pub fn get(&self, id: u32) -> Option<&CurrencyTypesEntry> {
        self.entries.get(&id)
    }

    pub fn has_record(&self, id: u32) -> bool {
        self.entries.contains_key(&id)
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
    fn currency_type_helpers_match_cpp_flags() {
        let entry = CurrencyTypesEntry {
            id: 1,
            category_id: 0,
            inventory_icon_file_id: 0,
            spell_weight: 0,
            spell_category: 0,
            max_qty: 0,
            max_earnable_per_week: 0,
            quality: 0,
            faction_id: 0,
            award_condition_id: 0,
            flags: CurrencyTypesFlags::SCALER_100
                | CurrencyTypesFlags::DYNAMIC_MAXIMUM
                | CurrencyTypesFlags::IS_ALLIANCE_ONLY
                | CurrencyTypesFlags::TRACK_QUANTITY,
            flags_b: CurrencyTypesFlagsB::USE_TOTAL_EARNED_FOR_EARNED,
        };

        assert_eq!(entry.scaler(), 100);
        assert!(entry.has_max_quantity(false, false));
        assert!(entry.has_total_earned());
        assert!(entry.is_alliance());
        assert!(!entry.is_horde());
        assert!(entry.is_tracking_quantity());
    }

    #[test]
    fn load_currency_types_store() {
        let data_dir = "/home/server/woltk-server-core/Data";
        let locale = "esES";
        let path = Path::new(data_dir)
            .join("dbc")
            .join(locale)
            .join("CurrencyTypes.db2");
        if !path.exists() {
            eprintln!(
                "Skipping test: CurrencyTypes.db2 not found at {}",
                path.display()
            );
            return;
        }

        let store =
            CurrencyTypesStore::load(data_dir, locale).expect("failed to load CurrencyTypesStore");
        assert!(!store.is_empty());
        assert!(store.has_record(395));
    }
}
