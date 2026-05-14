// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `DisableMgr` data model and runtime checks.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use tracing::{info, warn};
use wow_constants::TypeId;
use wow_database::WorldDatabase;

use crate::MapStore;

pub const DISABLE_TYPE_SPELL: u32 = 0;
pub const DISABLE_TYPE_QUEST: u32 = 1;
pub const DISABLE_TYPE_MAP: u32 = 2;
pub const DISABLE_TYPE_BATTLEGROUND: u32 = 3;
pub const DISABLE_TYPE_CRITERIA: u32 = 4;
pub const DISABLE_TYPE_OUTDOORPVP: u32 = 5;
pub const DISABLE_TYPE_VMAP: u32 = 6;
pub const DISABLE_TYPE_MMAP: u32 = 7;
pub const DISABLE_TYPE_LFG_MAP: u32 = 8;
pub const MAX_DISABLE_TYPES: u32 = 9;

pub const SPELL_DISABLE_PLAYER: u16 = 0x01;
pub const SPELL_DISABLE_CREATURE: u16 = 0x02;
pub const SPELL_DISABLE_PET: u16 = 0x04;
pub const SPELL_DISABLE_DEPRECATED_SPELL: u16 = 0x08;
pub const SPELL_DISABLE_MAP: u16 = 0x10;
pub const SPELL_DISABLE_AREA: u16 = 0x20;
pub const SPELL_DISABLE_LOS: u16 = 0x40;
pub const SPELL_DISABLE_GAMEOBJECT: u16 = 0x80;
pub const SPELL_DISABLE_ARENAS: u16 = 0x100;
pub const SPELL_DISABLE_BATTLEGROUNDS: u16 = 0x200;
pub const MAX_SPELL_DISABLE_TYPE: u16 = SPELL_DISABLE_PLAYER
    | SPELL_DISABLE_CREATURE
    | SPELL_DISABLE_PET
    | SPELL_DISABLE_DEPRECATED_SPELL
    | SPELL_DISABLE_MAP
    | SPELL_DISABLE_AREA
    | SPELL_DISABLE_LOS
    | SPELL_DISABLE_GAMEOBJECT
    | SPELL_DISABLE_ARENAS
    | SPELL_DISABLE_BATTLEGROUNDS;

pub const MMAP_DISABLE_PATHFINDING: u8 = 0x0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableDbRowLikeCpp {
    pub source_type: u32,
    pub entry: u32,
    pub flags: u16,
    pub params_0: String,
    pub params_1: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisableDataLikeCpp {
    pub flags: u16,
    pub params_0: HashSet<u32>,
    pub params_1: HashSet<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisableLoadReportLikeCpp {
    pub loaded_count: usize,
    pub skipped_rows: Vec<DisableSkippedRowLikeCpp>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableSkippedRowLikeCpp {
    pub row: DisableDbRowLikeCpp,
    pub reason: String,
}

#[derive(Clone, Copy, Default)]
pub struct DisableMgrRefsLikeCpp<'a> {
    pub map_store: Option<&'a MapStore>,
    pub spell_exists: Option<fn(u32) -> bool>,
    pub quest_exists: Option<fn(u32) -> bool>,
    pub criteria_exists: Option<fn(u32) -> bool>,
    pub battlemaster_exists: Option<fn(u32) -> bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisableWorldObjectRefLikeCpp {
    pub type_id: TypeId,
    pub map_id: u32,
    pub area_id: u32,
    pub is_pet: bool,
    pub is_battle_arena: bool,
    pub is_battleground: bool,
    pub player_map_difficulty: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisableMgrLikeCpp {
    by_type: [HashMap<u32, DisableDataLikeCpp>; MAX_DISABLE_TYPES as usize],
}

impl DisableMgrLikeCpp {
    pub fn from_rows_like_cpp(
        rows: impl IntoIterator<Item = DisableDbRowLikeCpp>,
        refs: DisableMgrRefsLikeCpp<'_>,
    ) -> (Self, DisableLoadReportLikeCpp) {
        let mut mgr = Self::default();
        let mut report = DisableLoadReportLikeCpp::default();

        for row in rows {
            match parse_row_like_cpp(&row, refs) {
                Ok((data, warnings)) => {
                    report.warnings.extend(warnings);
                    mgr.by_type[row.source_type as usize].insert(row.entry, data);
                    report.loaded_count += 1;
                }
                Err(reason) => report
                    .skipped_rows
                    .push(DisableSkippedRowLikeCpp { row, reason }),
            }
        }

        (mgr, report)
    }

    pub async fn load_like_cpp(
        db: &WorldDatabase,
        refs: DisableMgrRefsLikeCpp<'_>,
    ) -> Result<(Self, DisableLoadReportLikeCpp)> {
        let mut result = db
            .direct_query("SELECT sourceType, entry, flags, params_0, params_1 FROM disables")
            .await?;
        if result.is_empty() {
            info!("Loaded 0 disables. DB table `disables` is empty");
            return Ok((Self::default(), DisableLoadReportLikeCpp::default()));
        }

        let mut rows = Vec::new();
        loop {
            rows.push(DisableDbRowLikeCpp {
                source_type: result.read(0),
                entry: result.read(1),
                flags: result.read(2),
                params_0: result.read_string(3),
                params_1: result.read_string(4),
            });
            if !result.next_row() {
                break;
            }
        }

        let (mgr, report) = Self::from_rows_like_cpp(rows, refs);
        info!("Loaded {} disables", report.loaded_count);
        for skipped in &report.skipped_rows {
            warn!(
                "Skipped disable type {} entry {}: {}",
                skipped.row.source_type, skipped.row.entry, skipped.reason
            );
        }
        for warning in &report.warnings {
            warn!("{warning}");
        }
        Ok((mgr, report))
    }

    pub fn is_disabled_for_like_cpp(
        &self,
        source_type: u32,
        entry: u32,
        object_ref: Option<DisableWorldObjectRefLikeCpp>,
        flags: u8,
        map_store: Option<&MapStore>,
    ) -> bool {
        if source_type >= MAX_DISABLE_TYPES {
            return false;
        }

        let Some(data) = self.by_type[source_type as usize].get(&entry) else {
            return false;
        };

        match source_type {
            DISABLE_TYPE_SPELL => is_spell_disabled_like_cpp(data, object_ref, flags),
            DISABLE_TYPE_MAP | DISABLE_TYPE_LFG_MAP => {
                is_map_disabled_like_cpp(entry, data, object_ref, map_store)
            }
            DISABLE_TYPE_QUEST
            | DISABLE_TYPE_BATTLEGROUND
            | DISABLE_TYPE_OUTDOORPVP
            | DISABLE_TYPE_CRITERIA
            | DISABLE_TYPE_MMAP => true,
            DISABLE_TYPE_VMAP => flags & (data.flags as u8) != 0,
            _ => false,
        }
    }

    pub fn is_vmap_disabled_for_like_cpp(&self, entry: u32, flags: u8) -> bool {
        self.is_disabled_for_like_cpp(DISABLE_TYPE_VMAP, entry, None, flags, None)
    }

    pub fn is_pathfinding_enabled_like_cpp(&self, map_id: u32, config_enable_mmaps: bool) -> bool {
        config_enable_mmaps
            && !self.is_disabled_for_like_cpp(
                DISABLE_TYPE_MMAP,
                map_id,
                None,
                MMAP_DISABLE_PATHFINDING,
                None,
            )
    }

    pub fn disabled_mmap_map_ids_like_cpp(&self) -> HashSet<u32> {
        self.by_type[DISABLE_TYPE_MMAP as usize]
            .keys()
            .copied()
            .collect()
    }
}

fn parse_row_like_cpp(
    row: &DisableDbRowLikeCpp,
    refs: DisableMgrRefsLikeCpp<'_>,
) -> std::result::Result<(DisableDataLikeCpp, Vec<String>), String> {
    if row.source_type >= MAX_DISABLE_TYPES {
        return Err(format!(
            "Invalid type {} specified in `disables` table",
            row.source_type
        ));
    }

    let mut data = DisableDataLikeCpp {
        flags: row.flags,
        params_0: HashSet::new(),
        params_1: HashSet::new(),
    };
    let mut warnings = Vec::new();

    match row.source_type {
        DISABLE_TYPE_SPELL => {
            if refs.spell_exists.is_some_and(|exists| !exists(row.entry))
                && row.flags & SPELL_DISABLE_DEPRECATED_SPELL == 0
            {
                return Err(format!("Spell entry {} doesn't exist in dbc", row.entry));
            }
            if row.flags == 0 || row.flags > MAX_SPELL_DISABLE_TYPE {
                return Err(format!("Disable flags for spell {} are invalid", row.entry));
            }
            if row.flags & SPELL_DISABLE_MAP != 0 {
                data.params_0 =
                    parse_u32_set_like_cpp(&row.params_0, "map", row.entry, &mut warnings);
            }
            if row.flags & SPELL_DISABLE_AREA != 0 {
                data.params_1 =
                    parse_u32_set_like_cpp(&row.params_1, "area", row.entry, &mut warnings);
            }
        }
        DISABLE_TYPE_QUEST => {}
        DISABLE_TYPE_MAP | DISABLE_TYPE_LFG_MAP | DISABLE_TYPE_VMAP | DISABLE_TYPE_MMAP => {
            if refs
                .map_store
                .is_some_and(|store| store.get(row.entry).is_none())
            {
                return Err(format!("Map entry {} doesn't exist in dbc", row.entry));
            }
        }
        DISABLE_TYPE_BATTLEGROUND => {
            if refs
                .battlemaster_exists
                .is_some_and(|exists| !exists(row.entry))
            {
                return Err(format!(
                    "Battleground entry {} doesn't exist in dbc",
                    row.entry
                ));
            }
            if row.flags != 0 {
                warnings.push(format!(
                    "Disable flags specified for battleground {}, useless data",
                    row.entry
                ));
            }
        }
        DISABLE_TYPE_OUTDOORPVP => {
            if row.flags != 0 {
                warnings.push(format!(
                    "Disable flags specified for outdoor PvP {}, useless data",
                    row.entry
                ));
            }
        }
        DISABLE_TYPE_CRITERIA => {
            if refs
                .criteria_exists
                .is_some_and(|exists| !exists(row.entry))
            {
                return Err(format!("Criteria entry {} doesn't exist in dbc", row.entry));
            }
            if row.flags != 0 {
                warnings.push(format!(
                    "Disable flags specified for Criteria {}, useless data",
                    row.entry
                ));
            }
        }
        _ => {}
    }

    Ok((data, warnings))
}

fn parse_u32_set_like_cpp(
    text: &str,
    name: &str,
    spell_id: u32,
    warnings: &mut Vec<String>,
) -> HashSet<u32> {
    let mut values = HashSet::new();
    for token in text.split(',').filter(|token| !token.is_empty()) {
        match token.parse::<u32>() {
            Ok(value) => {
                values.insert(value);
            }
            Err(_) => warnings.push(format!(
                "Disable {name} '{token}' for spell {spell_id} is invalid, skipped"
            )),
        }
    }
    values
}

fn is_spell_disabled_like_cpp(
    data: &DisableDataLikeCpp,
    object_ref: Option<DisableWorldObjectRefLikeCpp>,
    flags: u8,
) -> bool {
    let spell_flags = data.flags;
    if let Some(object_ref) = object_ref {
        let type_matches = (object_ref.type_id == TypeId::Player
            && spell_flags & SPELL_DISABLE_PLAYER != 0)
            || (object_ref.type_id == TypeId::Unit
                && (spell_flags & SPELL_DISABLE_CREATURE != 0
                    || (object_ref.is_pet && spell_flags & SPELL_DISABLE_PET != 0)))
            || (object_ref.type_id == TypeId::GameObject
                && spell_flags & SPELL_DISABLE_GAMEOBJECT != 0);

        if !type_matches {
            return false;
        }

        if spell_flags & (SPELL_DISABLE_ARENAS | SPELL_DISABLE_BATTLEGROUNDS) != 0 {
            if spell_flags & SPELL_DISABLE_ARENAS != 0 && object_ref.is_battle_arena {
                return true;
            }
            if spell_flags & SPELL_DISABLE_BATTLEGROUNDS != 0 && object_ref.is_battleground {
                return true;
            }
        }

        if spell_flags & SPELL_DISABLE_MAP != 0 {
            if data.params_0.contains(&object_ref.map_id) {
                return true;
            }
            if spell_flags & SPELL_DISABLE_AREA == 0 {
                return false;
            }
        }

        if spell_flags & SPELL_DISABLE_AREA != 0 {
            return data.params_1.contains(&object_ref.area_id);
        }

        return true;
    }

    if spell_flags & SPELL_DISABLE_DEPRECATED_SPELL != 0 {
        return true;
    }
    if flags & (SPELL_DISABLE_LOS as u8) != 0 {
        return spell_flags & SPELL_DISABLE_LOS != 0;
    }
    false
}

fn is_map_disabled_like_cpp(
    entry: u32,
    data: &DisableDataLikeCpp,
    object_ref: Option<DisableWorldObjectRefLikeCpp>,
    map_store: Option<&MapStore>,
) -> bool {
    let Some(object_ref) = object_ref else {
        return true;
    };
    if object_ref.type_id != TypeId::Player {
        return false;
    }

    let Some(map_entry) = map_store.and_then(|store| store.get(entry)) else {
        return false;
    };
    match map_entry.instance_type {
        0 => true,
        1 | 2 => object_ref.player_map_difficulty.is_some_and(|difficulty| {
            map_disable_flags_match_difficulty_like_cpp(data.flags, difficulty)
        }),
        _ => false,
    }
}

fn map_disable_flags_match_difficulty_like_cpp(flags: u16, difficulty: u8) -> bool {
    const DUNGEON_STATUSFLAG_NORMAL: u16 = 0x01;
    const DUNGEON_STATUSFLAG_HEROIC: u16 = 0x02;
    const RAID_STATUSFLAG_10MAN_HEROIC: u16 = 0x04;
    const RAID_STATUSFLAG_25MAN_HEROIC: u16 = 0x08;

    match difficulty {
        1 => flags & DUNGEON_STATUSFLAG_NORMAL != 0,
        2 => flags & DUNGEON_STATUSFLAG_HEROIC != 0,
        5 => flags & RAID_STATUSFLAG_10MAN_HEROIC != 0,
        6 => flags & RAID_STATUSFLAG_25MAN_HEROIC != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MapEntry;

    fn row(source_type: u32, entry: u32, flags: u16) -> DisableDbRowLikeCpp {
        DisableDbRowLikeCpp {
            source_type,
            entry,
            flags,
            params_0: String::new(),
            params_1: String::new(),
        }
    }

    fn map_store() -> MapStore {
        MapStore::from_entries([
            MapEntry {
                id: 0,
                instance_type: 0,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
            MapEntry {
                id: 571,
                instance_type: 1,
                parent_map_id: -1,
                cosmetic_parent_map_id: -1,
                flags1: 0,
            },
        ])
    }

    #[test]
    fn disable_type_constants_match_cpp_header() {
        assert_eq!(DISABLE_TYPE_SPELL, 0);
        assert_eq!(DISABLE_TYPE_VMAP, 6);
        assert_eq!(DISABLE_TYPE_MMAP, 7);
        assert_eq!(DISABLE_TYPE_LFG_MAP, 8);
        assert_eq!(MAX_DISABLE_TYPES, 9);
        assert_eq!(MMAP_DISABLE_PATHFINDING, 0);
    }

    #[test]
    fn spell_disable_flags_and_map_area_params_match_cpp() {
        let spell_row = DisableDbRowLikeCpp {
            source_type: DISABLE_TYPE_SPELL,
            entry: 123,
            flags: SPELL_DISABLE_PLAYER | SPELL_DISABLE_MAP | SPELL_DISABLE_AREA,
            params_0: "571,not-a-map".to_string(),
            params_1: "42".to_string(),
        };
        let (mgr, report) =
            DisableMgrLikeCpp::from_rows_like_cpp([spell_row], DisableMgrRefsLikeCpp::default());
        assert_eq!(report.loaded_count, 1);
        assert_eq!(report.warnings.len(), 1);

        let player = DisableWorldObjectRefLikeCpp {
            type_id: TypeId::Player,
            map_id: 571,
            area_id: 1,
            is_pet: false,
            is_battle_arena: false,
            is_battleground: false,
            player_map_difficulty: None,
        };
        assert!(mgr.is_disabled_for_like_cpp(DISABLE_TYPE_SPELL, 123, Some(player), 0, None));

        let area_player = DisableWorldObjectRefLikeCpp {
            map_id: 0,
            area_id: 42,
            ..player
        };
        assert!(mgr.is_disabled_for_like_cpp(DISABLE_TYPE_SPELL, 123, Some(area_player), 0, None));

        let other = DisableWorldObjectRefLikeCpp {
            map_id: 0,
            area_id: 7,
            ..player
        };
        assert!(!mgr.is_disabled_for_like_cpp(DISABLE_TYPE_SPELL, 123, Some(other), 0, None));
    }

    #[test]
    fn mmap_disable_makes_pathfinding_false_like_cpp() {
        let maps = map_store();
        let (mgr, report) = DisableMgrLikeCpp::from_rows_like_cpp(
            [row(DISABLE_TYPE_MMAP, 571, 0)],
            DisableMgrRefsLikeCpp {
                map_store: Some(&maps),
                ..Default::default()
            },
        );
        assert_eq!(report.loaded_count, 1);
        assert!(mgr.is_pathfinding_enabled_like_cpp(0, true));
        assert!(!mgr.is_pathfinding_enabled_like_cpp(571, true));
        assert!(!mgr.is_pathfinding_enabled_like_cpp(0, false));
        assert_eq!(
            mgr.disabled_mmap_map_ids_like_cpp(),
            HashSet::from([571_u32])
        );
    }

    #[test]
    fn vmap_checks_requested_flags_like_cpp() {
        let maps = map_store();
        let (mgr, report) = DisableMgrLikeCpp::from_rows_like_cpp(
            [row(DISABLE_TYPE_VMAP, 571, 0x03)],
            DisableMgrRefsLikeCpp {
                map_store: Some(&maps),
                ..Default::default()
            },
        );
        assert_eq!(report.loaded_count, 1);
        assert!(mgr.is_vmap_disabled_for_like_cpp(571, 0x01));
        assert!(!mgr.is_vmap_disabled_for_like_cpp(571, 0x04));
    }

    #[test]
    fn map_disable_without_ref_returns_true_like_cpp() {
        let maps = map_store();
        let (mgr, _) = DisableMgrLikeCpp::from_rows_like_cpp(
            [row(DISABLE_TYPE_MAP, 571, 0x02)],
            DisableMgrRefsLikeCpp {
                map_store: Some(&maps),
                ..Default::default()
            },
        );
        assert!(mgr.is_disabled_for_like_cpp(DISABLE_TYPE_MAP, 571, None, 0, Some(&maps)));

        let player = DisableWorldObjectRefLikeCpp {
            type_id: TypeId::Player,
            map_id: 0,
            area_id: 0,
            is_pet: false,
            is_battle_arena: false,
            is_battleground: false,
            player_map_difficulty: Some(2),
        };
        assert!(mgr.is_disabled_for_like_cpp(DISABLE_TYPE_MAP, 571, Some(player), 0, Some(&maps)));
    }

    #[test]
    fn invalid_disable_type_is_skipped_like_cpp() {
        let (mgr, report) = DisableMgrLikeCpp::from_rows_like_cpp(
            [row(MAX_DISABLE_TYPES, 1, 0)],
            DisableMgrRefsLikeCpp::default(),
        );
        assert_eq!(report.loaded_count, 0);
        assert_eq!(report.skipped_rows.len(), 1);
        assert!(!mgr.is_disabled_for_like_cpp(MAX_DISABLE_TYPES, 1, None, 0, None));
    }
}
