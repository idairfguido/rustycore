// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `game/Instances` foundation.

use wow_data::{DungeonEncounterEntry, DungeonEncounterStore};

/// C++ `MAX_DUNGEON_ENCOUNTERS_PER_BOSS`.
pub const MAX_DUNGEON_ENCOUNTERS_PER_BOSS: usize = 4;

/// C++ `EncounterState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncounterState {
    NotStarted = 0,
    InProgress = 1,
    Fail = 2,
    Done = 3,
    Special = 4,
    ToBeDecided = 5,
}

impl Default for EncounterState {
    fn default() -> Self {
        Self::ToBeDecided
    }
}

/// C++ `DungeonEncounterData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonEncounterData {
    pub boss_id: u32,
    pub dungeon_encounter_ids: [u32; MAX_DUNGEON_ENCOUNTERS_PER_BOSS],
}

/// Minimal C++ `BossInfo` data needed for `GetBossDungeonEncounter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BossInfo {
    pub state: EncounterState,
    dungeon_encounters: [Option<u32>; MAX_DUNGEON_ENCOUNTERS_PER_BOSS],
}

impl Default for BossInfo {
    fn default() -> Self {
        Self {
            state: EncounterState::ToBeDecided,
            dungeon_encounters: [None; MAX_DUNGEON_ENCOUNTERS_PER_BOSS],
        }
    }
}

impl BossInfo {
    /// C++ `BossInfo::GetDungeonEncounterForDifficulty`.
    pub fn dungeon_encounter_for_difficulty<'a>(
        &self,
        store: &'a DungeonEncounterStore,
        difficulty_id: u32,
    ) -> Option<&'a DungeonEncounterEntry> {
        self.dungeon_encounters
            .iter()
            .flatten()
            .filter_map(|encounter_id| store.get(*encounter_id))
            .find(|encounter| {
                encounter.difficulty_id == 0
                    || u32::try_from(encounter.difficulty_id).ok() == Some(difficulty_id)
            })
    }
}

/// Minimal C++ `InstanceScript` base data for encounter metadata lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceScriptBase {
    difficulty_id: u32,
    bosses: Vec<BossInfo>,
}

impl InstanceScriptBase {
    pub fn new(difficulty_id: u32, boss_count: usize) -> Self {
        Self {
            difficulty_id,
            bosses: vec![BossInfo::default(); boss_count],
        }
    }

    pub fn difficulty_id(&self) -> u32 {
        self.difficulty_id
    }

    pub fn boss_count(&self) -> usize {
        self.bosses.len()
    }

    pub fn boss(&self, boss_id: u32) -> Option<&BossInfo> {
        self.bosses.get(boss_id as usize)
    }

    /// C++ `InstanceScript::LoadDungeonEncounterData(uint32, array<uint32, 4>)`.
    pub fn load_dungeon_encounter_data(
        &mut self,
        store: &DungeonEncounterStore,
        boss_id: u32,
        dungeon_encounter_ids: [u32; MAX_DUNGEON_ENCOUNTERS_PER_BOSS],
    ) {
        let Some(boss) = self.bosses.get_mut(boss_id as usize) else {
            return;
        };

        for (slot, encounter_id) in dungeon_encounter_ids.into_iter().enumerate() {
            boss.dungeon_encounters[slot] = store.get(encounter_id).map(|entry| entry.id);
        }
    }

    /// C++ `InstanceScript::LoadDungeonEncounterData(T const&)`.
    pub fn load_dungeon_encounter_data_rows(
        &mut self,
        store: &DungeonEncounterStore,
        rows: impl IntoIterator<Item = DungeonEncounterData>,
    ) {
        for row in rows {
            self.load_dungeon_encounter_data(store, row.boss_id, row.dungeon_encounter_ids);
        }
    }

    /// C++ `InstanceScript::GetBossDungeonEncounter(uint32)`.
    pub fn boss_dungeon_encounter<'a>(
        &self,
        store: &'a DungeonEncounterStore,
        boss_id: u32,
    ) -> Option<&'a DungeonEncounterEntry> {
        self.boss(boss_id)?
            .dungeon_encounter_for_difficulty(store, self.difficulty_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encounter(id: u32, difficulty_id: i32) -> DungeonEncounterEntry {
        DungeonEncounterEntry {
            id,
            map_id: 631,
            difficulty_id,
            order_index: 0,
            bit: 0,
            flags: 0,
            faction: -1,
        }
    }

    #[test]
    fn boss_info_selects_first_any_or_matching_difficulty_like_cpp() {
        let store = DungeonEncounterStore::from_entries([encounter(1, 0), encounter(2, 4)]);
        let mut script = InstanceScriptBase::new(4, 1);

        script.load_dungeon_encounter_data(&store, 0, [1, 2, 0, 0]);

        assert_eq!(script.boss_dungeon_encounter(&store, 0).unwrap().id, 1);
    }

    #[test]
    fn boss_info_skips_non_matching_difficulty_like_cpp() {
        let store = DungeonEncounterStore::from_entries([encounter(1, 3), encounter(2, 4)]);
        let mut script = InstanceScriptBase::new(4, 1);

        script.load_dungeon_encounter_data(&store, 0, [1, 2, 0, 0]);

        assert_eq!(script.boss_dungeon_encounter(&store, 0).unwrap().id, 2);
    }

    #[test]
    fn load_dungeon_encounter_data_ignores_invalid_boss_or_missing_rows_like_cpp() {
        let store = DungeonEncounterStore::from_entries([encounter(2, 4)]);
        let mut script = InstanceScriptBase::new(4, 1);

        script.load_dungeon_encounter_data(&store, 99, [2, 0, 0, 0]);
        assert!(script.boss_dungeon_encounter(&store, 0).is_none());

        script.load_dungeon_encounter_data(&store, 0, [1, 0, 0, 0]);
        assert!(script.boss_dungeon_encounter(&store, 0).is_none());
    }
}
