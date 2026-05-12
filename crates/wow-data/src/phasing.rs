// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ObjectMgr` phasing metadata.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use tracing::info;
use wow_database::{WorldDatabase, WorldStatements};

use crate::{AreaTableStore, PhaseStore};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseConditionContainer {
    represented_rows: usize,
}

impl PhaseConditionContainer {
    pub const fn represented_rows(&self) -> usize {
        self.represented_rows
    }

    pub const fn is_empty(&self) -> bool {
        self.represented_rows == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseInfoStruct {
    pub id: u32,
    pub areas: HashSet<u32>,
}

impl PhaseInfoStruct {
    /// C++ `PhaseInfoStruct::IsAllowedInArea`.
    pub fn is_allowed_in_area_like_cpp(
        &self,
        area_id: u32,
        mut is_in_area: impl FnMut(u32, u32) -> bool,
    ) -> bool {
        self.areas
            .iter()
            .any(|area_to_check| is_in_area(area_id, *area_to_check))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAreaInfo {
    pub phase_id: u32,
    pub sub_area_exclusions: HashSet<u32>,
    pub conditions: PhaseConditionContainer,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseInfoStore {
    phase_info_by_id: HashMap<u32, PhaseInfoStruct>,
    phase_info_by_area: HashMap<u32, Vec<PhaseAreaInfo>>,
}

impl PhaseInfoStore {
    /// C++ `ObjectMgr::LoadPhases` seeds `_phaseInfoById` from `sPhaseStore`.
    pub fn from_phase_store_like_cpp(phase_store: &PhaseStore) -> Self {
        let phase_info_by_id = phase_store
            .entries()
            .map(|phase| {
                (
                    phase.id,
                    PhaseInfoStruct {
                        id: phase.id,
                        areas: HashSet::new(),
                    },
                )
            })
            .collect();

        Self {
            phase_info_by_id,
            phase_info_by_area: HashMap::new(),
        }
    }

    pub fn phase_info(&self, phase_id: u32) -> Option<&PhaseInfoStruct> {
        self.phase_info_by_id.get(&phase_id)
    }

    pub fn phases_for_area(&self, area_id: u32) -> Option<&[PhaseAreaInfo]> {
        self.phase_info_by_area.get(&area_id).map(Vec::as_slice)
    }

    pub fn phase_info_count(&self) -> usize {
        self.phase_info_by_id.len()
    }

    pub fn phase_area_count(&self) -> usize {
        self.phase_info_by_area.values().map(Vec::len).sum()
    }

    pub fn load_area_phases_from_rows_like_cpp(
        &mut self,
        area_store: &AreaTableStore,
        phase_store: &PhaseStore,
        rows: impl IntoIterator<Item = (u32, u32)>,
    ) -> usize {
        let mut count = 0usize;
        for (area_id, phase_id) in rows {
            if !area_store.contains(area_id) || !phase_store.contains(phase_id) {
                continue;
            }

            let phase_info =
                self.phase_info_by_id
                    .entry(phase_id)
                    .or_insert_with(|| PhaseInfoStruct {
                        id: phase_id,
                        areas: HashSet::new(),
                    });
            phase_info.areas.insert(area_id);
            self.phase_info_by_area
                .entry(area_id)
                .or_default()
                .push(PhaseAreaInfo {
                    phase_id,
                    sub_area_exclusions: HashSet::new(),
                    conditions: PhaseConditionContainer::default(),
                });
            count += 1;
        }

        self.populate_sub_area_exclusions_like_cpp(area_store);
        count
    }

    pub async fn load_area_phases_like_cpp(
        &mut self,
        db: &WorldDatabase,
        area_store: &AreaTableStore,
        phase_store: &PhaseStore,
    ) -> Result<usize> {
        let stmt = db.prepare(WorldStatements::SEL_PHASE_AREAS);
        let mut result = db.query(&stmt).await?;
        if result.is_empty() {
            return Ok(0);
        }

        let mut rows = Vec::new();
        loop {
            rows.push((result.read(0), result.read(1)));
            if !result.next_row() {
                break;
            }
        }

        let count = self.load_area_phases_from_rows_like_cpp(area_store, phase_store, rows);
        info!("Loaded {count} phase area definitions");
        Ok(count)
    }

    fn populate_sub_area_exclusions_like_cpp(&mut self, area_store: &AreaTableStore) {
        let area_phase_pairs: Vec<_> = self
            .phase_info_by_area
            .iter()
            .flat_map(|(area_id, phases)| {
                phases
                    .iter()
                    .map(|phase| (*area_id, phase.phase_id))
                    .collect::<Vec<_>>()
            })
            .collect();

        for (child_area_id, phase_id) in area_phase_pairs {
            let mut parent_area_id = child_area_id;
            loop {
                let Some(area) = area_store.get(parent_area_id) else {
                    break;
                };

                parent_area_id = u32::from(area.parent_area_id);
                if parent_area_id == 0 {
                    break;
                }

                let Some(parent_area_phases) = self.phase_info_by_area.get_mut(&parent_area_id)
                else {
                    continue;
                };

                for parent_area_phase in parent_area_phases {
                    if parent_area_phase.phase_id == phase_id {
                        parent_area_phase.sub_area_exclusions.insert(child_area_id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AreaTableEntry, PhaseEntry};

    #[test]
    fn phase_info_store_seeds_from_phase_store_like_cpp_load_phases() {
        let phase_store = PhaseStore::from_entries([
            PhaseEntry { id: 10, flags: 0 },
            PhaseEntry { id: 20, flags: 0 },
        ]);

        let store = PhaseInfoStore::from_phase_store_like_cpp(&phase_store);

        assert_eq!(store.phase_info_count(), 2);
        assert_eq!(store.phase_info(10).map(|phase| phase.id), Some(10));
        assert_eq!(store.phase_info(20).map(|phase| phase.id), Some(20));
        assert!(store.phase_info(30).is_none());
        assert_eq!(store.phase_area_count(), 0);
    }

    #[test]
    fn phase_info_is_allowed_in_area_delegates_area_tree_like_cpp() {
        let phase_info = PhaseInfoStruct {
            id: 10,
            areas: HashSet::from([100, 200]),
        };

        assert!(
            phase_info.is_allowed_in_area_like_cpp(101, |area_id, area_to_check| {
                area_id == 101 && area_to_check == 100
            })
        );
        assert!(!phase_info.is_allowed_in_area_like_cpp(101, |_, _| false));
    }

    #[test]
    fn phase_area_rows_skip_missing_area_or_phase_like_cpp() {
        let area_store = AreaTableStore::from_entries([AreaTableEntry {
            id: 100,
            parent_area_id: 0,
        }]);
        let phase_store = PhaseStore::from_entries([PhaseEntry { id: 10, flags: 0 }]);
        let mut store = PhaseInfoStore::from_phase_store_like_cpp(&phase_store);

        let count = store.load_area_phases_from_rows_like_cpp(
            &area_store,
            &phase_store,
            [(100, 10), (101, 10), (100, 11)],
        );

        assert_eq!(count, 1);
        assert_eq!(store.phase_area_count(), 1);
        assert_eq!(store.phase_info(10).map(|phase| phase.areas.len()), Some(1));
        assert!(store.phases_for_area(101).is_none());
    }

    #[test]
    fn phase_area_rows_populate_parent_sub_area_exclusions_like_cpp() {
        let area_store = AreaTableStore::from_entries([
            AreaTableEntry {
                id: 100,
                parent_area_id: 0,
            },
            AreaTableEntry {
                id: 101,
                parent_area_id: 100,
            },
        ]);
        let phase_store = PhaseStore::from_entries([PhaseEntry { id: 10, flags: 0 }]);
        let mut store = PhaseInfoStore::from_phase_store_like_cpp(&phase_store);

        let count = store.load_area_phases_from_rows_like_cpp(
            &area_store,
            &phase_store,
            [(100, 10), (101, 10)],
        );

        assert_eq!(count, 2);
        let parent_phase = store
            .phases_for_area(100)
            .and_then(|phases| phases.iter().find(|phase| phase.phase_id == 10))
            .expect("parent area phase missing");
        assert!(parent_phase.sub_area_exclusions.contains(&101));
    }
}
