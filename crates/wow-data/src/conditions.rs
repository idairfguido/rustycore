// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ConditionMgr` data rows.

use wow_constants::{ConditionSourceType, ConditionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ConditionId {
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
}

impl ConditionId {
    pub const fn new(source_group: u32, source_entry: i32, source_id: u32) -> Self {
        Self {
            source_group,
            source_entry,
            source_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub source_type: ConditionSourceType,
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
    pub else_group: u32,
    pub condition_type: ConditionType,
    pub condition_value1: u32,
    pub condition_value2: u32,
    pub condition_value3: u32,
    pub condition_string_value1: String,
    pub error_type: u32,
    pub error_text_id: u32,
    pub reference_id: u32,
    pub script_id: u32,
    pub condition_target: u8,
    pub negative_condition: bool,
}

impl Default for Condition {
    fn default() -> Self {
        Self {
            source_type: ConditionSourceType::None,
            source_group: 0,
            source_entry: 0,
            source_id: 0,
            else_group: 0,
            condition_type: ConditionType::None,
            condition_value1: 0,
            condition_value2: 0,
            condition_value3: 0,
            condition_string_value1: String::new(),
            error_type: 0,
            error_text_id: 0,
            reference_id: 0,
            script_id: 0,
            condition_target: 0,
            negative_condition: false,
        }
    }
}

impl Condition {
    /// C++ `Condition::isLoaded`.
    pub const fn is_loaded_like_cpp(&self) -> bool {
        self.condition_type as u32 > ConditionType::None as u32
            || self.reference_id != 0
            || self.script_id != 0
    }

    pub const fn id_like_cpp(&self) -> ConditionId {
        ConditionId::new(self.source_group, self.source_entry, self.source_id)
    }
}

pub type ConditionContainer = Vec<Condition>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_default_matches_cpp_constructor() {
        let condition = Condition::default();

        assert_eq!(condition.source_type, ConditionSourceType::None);
        assert_eq!(condition.source_group, 0);
        assert_eq!(condition.source_entry, 0);
        assert_eq!(condition.source_id, 0);
        assert_eq!(condition.else_group, 0);
        assert_eq!(condition.condition_type, ConditionType::None);
        assert_eq!(condition.condition_value1, 0);
        assert_eq!(condition.condition_value2, 0);
        assert_eq!(condition.condition_value3, 0);
        assert!(condition.condition_string_value1.is_empty());
        assert_eq!(condition.error_type, 0);
        assert_eq!(condition.error_text_id, 0);
        assert_eq!(condition.reference_id, 0);
        assert_eq!(condition.script_id, 0);
        assert_eq!(condition.condition_target, 0);
        assert!(!condition.negative_condition);
        assert!(!condition.is_loaded_like_cpp());
    }

    #[test]
    fn condition_is_loaded_matches_cpp() {
        let mut condition = Condition {
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        assert!(condition.is_loaded_like_cpp());

        condition.condition_type = ConditionType::None;
        condition.reference_id = 42;
        assert!(condition.is_loaded_like_cpp());

        condition.reference_id = 0;
        condition.script_id = 7;
        assert!(condition.is_loaded_like_cpp());
    }

    #[test]
    fn condition_id_matches_cpp_key_fields() {
        let condition = Condition {
            source_group: 12,
            source_entry: -45,
            source_id: 67,
            ..Condition::default()
        };

        assert_eq!(condition.id_like_cpp(), ConditionId::new(12, -45, 67));
    }
}
