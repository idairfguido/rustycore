// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ConditionMgr` data rows.

use anyhow::Result;
use num_traits::FromPrimitive;
use tracing::info;
use wow_constants::{ConditionSourceType, ConditionType};
use wow_database::{WorldDatabase, WorldStatements};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionDbRowLikeCpp {
    pub source_type_or_reference_id: i32,
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
    pub else_group: u32,
    pub condition_type_or_reference: i32,
    pub condition_target: u8,
    pub condition_value1: u32,
    pub condition_value2: u32,
    pub condition_value3: u32,
    pub condition_string_value1: String,
    pub negative_condition: bool,
    pub error_type: u32,
    pub error_text_id: u32,
    pub script_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionRowSkipReason {
    SelfReference(i32),
    InvalidConditionType(i32),
    InvalidSourceType(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedConditionRow {
    pub row: ConditionDbRowLikeCpp,
    pub reason: ConditionRowSkipReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConditionLoadReport {
    pub conditions: Vec<Condition>,
    pub skipped: Vec<SkippedConditionRow>,
}

impl ConditionLoadReport {
    pub fn parsed_count(&self) -> usize {
        self.conditions.len()
    }
}

/// C++ `ConditionMgr::LoadConditions` row-to-`Condition` conversion before source/type validation.
pub fn parse_condition_row_like_cpp(
    row: ConditionDbRowLikeCpp,
    mut script_id_for_name: impl FnMut(&str) -> u32,
) -> Result<Condition, SkippedConditionRow> {
    let mut condition = Condition {
        source_group: row.source_group,
        source_entry: row.source_entry,
        source_id: row.source_id,
        else_group: row.else_group,
        condition_target: row.condition_target,
        condition_value1: row.condition_value1,
        condition_value2: row.condition_value2,
        condition_value3: row.condition_value3,
        condition_string_value1: row.condition_string_value1.clone(),
        negative_condition: row.negative_condition,
        error_type: row.error_type,
        error_text_id: row.error_text_id,
        script_id: script_id_for_name(&row.script_name),
        ..Condition::default()
    };

    if row.condition_type_or_reference >= 0 {
        condition.condition_type = ConditionType::from_i32(row.condition_type_or_reference)
            .ok_or_else(|| SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::InvalidConditionType(
                    row.condition_type_or_reference,
                ),
            })?;
    }

    if row.source_type_or_reference_id >= 0 {
        condition.source_type = ConditionSourceType::from_i32(row.source_type_or_reference_id)
            .ok_or_else(|| SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::InvalidSourceType(row.source_type_or_reference_id),
            })?;
    }

    if row.condition_type_or_reference < 0 {
        if row.condition_type_or_reference == row.source_type_or_reference_id {
            return Err(SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::SelfReference(row.source_type_or_reference_id),
            });
        }

        condition.reference_id = u32::try_from(-row.condition_type_or_reference).unwrap_or(0);
    }

    if row.source_type_or_reference_id < 0 {
        condition.source_type = ConditionSourceType::ReferenceCondition;
        condition.source_group = u32::try_from(-row.source_type_or_reference_id).unwrap_or(0);
    }

    Ok(condition)
}

pub fn parse_condition_rows_like_cpp(
    rows: impl IntoIterator<Item = ConditionDbRowLikeCpp>,
    mut script_id_for_name: impl FnMut(&str) -> u32,
) -> ConditionLoadReport {
    let mut report = ConditionLoadReport::default();
    for row in rows {
        match parse_condition_row_like_cpp(row, &mut script_id_for_name) {
            Ok(condition) => report.conditions.push(condition),
            Err(skipped) => report.skipped.push(skipped),
        }
    }
    report
}

pub async fn load_condition_rows_like_cpp(
    db: &WorldDatabase,
    script_id_for_name: impl FnMut(&str) -> u32,
) -> Result<ConditionLoadReport> {
    let stmt = db.prepare(WorldStatements::SEL_CONDITIONS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(ConditionLoadReport::default());
    }

    let mut rows = Vec::new();
    loop {
        rows.push(ConditionDbRowLikeCpp {
            source_type_or_reference_id: result.read(0),
            source_group: result.read(1),
            source_entry: result.read(2),
            source_id: result.read(3),
            else_group: result.read(4),
            condition_type_or_reference: result.read(5),
            condition_target: result.read(6),
            condition_value1: result.read(7),
            condition_value2: result.read(8),
            condition_value3: result.read(9),
            condition_string_value1: result.read_string(10),
            negative_condition: result.read(11),
            error_type: result.read(12),
            error_text_id: result.read(13),
            script_name: result.read_string(14),
        });

        if !result.next_row() {
            break;
        }
    }

    let report = parse_condition_rows_like_cpp(rows, script_id_for_name);
    info!(
        "Parsed {} conditions rows ({} skipped before validation)",
        report.parsed_count(),
        report.skipped.len()
    );
    Ok(report)
}

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

    #[test]
    fn parse_condition_row_maps_positive_source_and_type_like_cpp() {
        let condition = parse_condition_row_like_cpp(
            ConditionDbRowLikeCpp {
                source_type_or_reference_id: ConditionSourceType::Phase as i32,
                source_group: 12,
                source_entry: 34,
                source_id: 0,
                else_group: 2,
                condition_type_or_reference: ConditionType::Aura as i32,
                condition_target: 1,
                condition_value1: 100,
                condition_value2: 2,
                condition_value3: 3,
                condition_string_value1: String::from("string"),
                negative_condition: true,
                error_type: 4,
                error_text_id: 5,
                script_name: String::from("script"),
            },
            |name| u32::from(name == "script") * 77,
        )
        .unwrap();

        assert_eq!(condition.source_type, ConditionSourceType::Phase);
        assert_eq!(condition.condition_type, ConditionType::Aura);
        assert_eq!(condition.source_group, 12);
        assert_eq!(condition.source_entry, 34);
        assert_eq!(condition.else_group, 2);
        assert_eq!(condition.condition_target, 1);
        assert_eq!(condition.condition_value1, 100);
        assert_eq!(condition.condition_value2, 2);
        assert_eq!(condition.condition_value3, 3);
        assert_eq!(condition.condition_string_value1, "string");
        assert!(condition.negative_condition);
        assert_eq!(condition.error_type, 4);
        assert_eq!(condition.error_text_id, 5);
        assert_eq!(condition.script_id, 77);
    }

    #[test]
    fn parse_condition_row_maps_reference_template_like_cpp() {
        let condition = parse_condition_row_like_cpp(
            ConditionDbRowLikeCpp {
                source_type_or_reference_id: -500,
                source_group: 9,
                source_entry: 8,
                source_id: 7,
                else_group: 0,
                condition_type_or_reference: -(ConditionType::Aura as i32),
                condition_target: 0,
                condition_value1: 0,
                condition_value2: 0,
                condition_value3: 0,
                condition_string_value1: String::new(),
                negative_condition: false,
                error_type: 0,
                error_text_id: 0,
                script_name: String::new(),
            },
            |_| 0,
        )
        .unwrap();

        assert_eq!(
            condition.source_type,
            ConditionSourceType::ReferenceCondition
        );
        assert_eq!(condition.source_group, 500);
        assert_eq!(condition.source_entry, 8);
        assert_eq!(condition.source_id, 7);
        assert_eq!(condition.reference_id, ConditionType::Aura as u32);
        assert_eq!(condition.condition_type, ConditionType::None);
    }

    #[test]
    fn parse_condition_row_skips_self_reference_like_cpp() {
        let row = ConditionDbRowLikeCpp {
            source_type_or_reference_id: -42,
            source_group: 0,
            source_entry: 0,
            source_id: 0,
            else_group: 0,
            condition_type_or_reference: -42,
            condition_target: 0,
            condition_value1: 0,
            condition_value2: 0,
            condition_value3: 0,
            condition_string_value1: String::new(),
            negative_condition: false,
            error_type: 0,
            error_text_id: 0,
            script_name: String::new(),
        };

        let skipped = parse_condition_row_like_cpp(row, |_| 0).unwrap_err();

        assert_eq!(skipped.reason, ConditionRowSkipReason::SelfReference(-42));
    }
}
