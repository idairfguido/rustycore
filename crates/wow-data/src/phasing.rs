// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ObjectMgr` phasing metadata.

use std::collections::{HashMap, HashSet};

use crate::PhaseStore;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PhaseEntry;

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
}
