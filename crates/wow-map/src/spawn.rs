//! Spawn metadata and cell indexes used by grid loading.
//!
//! C++ references:
//! - `game/Maps/SpawnData.h`
//! - `game/Globals/ObjectMgr.cpp` (`AddSpawnDataToGrid`)
//! - `game/Globals/AreaTriggerDataStore.cpp` (`LoadAreaTriggerSpawns`)

use std::collections::{BTreeMap, BTreeSet};

use crate::coords::compute_cell_coord;

pub type SpawnId = u64;
pub type Difficulty = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SpawnObjectType {
    Creature = 0,
    GameObject = 1,
    AreaTrigger = 2,
}

impl SpawnObjectType {
    pub const fn type_has_data(self) -> bool {
        matches!(self, Self::Creature | Self::GameObject | Self::AreaTrigger)
    }

    pub const fn mask(self) -> u32 {
        1 << self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpawnGroupFlags(pub u32);

impl SpawnGroupFlags {
    pub const NONE: Self = Self(0x00);
    pub const SYSTEM: Self = Self(0x01);
    pub const COMPATIBILITY_MODE: Self = Self(0x02);
    pub const MANUAL_SPAWN: Self = Self(0x04);
    pub const DYNAMIC_SPAWN_RATE: Self = Self(0x08);
    pub const ESCORTQUESTNPC: Self = Self(0x10);
    pub const DESPAWN_ON_CONDITION_FAILURE: Self = Self(0x20);
    pub const ALL: Self = Self(
        Self::SYSTEM.0
            | Self::COMPATIBILITY_MODE.0
            | Self::MANUAL_SPAWN.0
            | Self::DYNAMIC_SPAWN_RATE.0
            | Self::ESCORTQUESTNPC.0
            | Self::DESPAWN_ON_CONDITION_FAILURE.0,
    );

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    pub const fn truncate_to_all(self) -> Self {
        Self(self.0 & Self::ALL.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

impl SpawnPosition {
    pub const fn new(x: f32, y: f32, z: f32, orientation: f32) -> Self {
        Self {
            x,
            y,
            z,
            orientation,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnGroupTemplateData {
    pub group_id: u32,
    pub name: String,
    pub map_id: u32,
    pub flags: SpawnGroupFlags,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnData {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    pub map_id: u32,
    pub db_data: bool,
    pub id: u32,
    pub spawn_point: SpawnPosition,
    pub phase_use_flags: u8,
    pub phase_id: u32,
    pub phase_group: u32,
    pub terrain_swap_map: i32,
    pub pool_id: u32,
    pub spawn_time_secs: i32,
    pub spawn_difficulties: Vec<Difficulty>,
    pub script_id: u32,
    pub string_id: String,
}

impl SpawnData {
    pub fn cell_id(&self) -> u32 {
        compute_cell_coord(self.spawn_point.x, self.spawn_point.y).get_id()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellSpawnGuids {
    pub creatures: BTreeSet<SpawnId>,
    pub gameobjects: BTreeSet<SpawnId>,
    pub area_triggers: BTreeSet<SpawnId>,
}

impl CellSpawnGuids {
    pub fn is_empty(&self) -> bool {
        self.creatures.is_empty() && self.gameobjects.is_empty() && self.area_triggers.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpawnMapKey {
    pub map_id: u32,
    pub difficulty: Difficulty,
}

impl SpawnMapKey {
    pub const fn new(map_id: u32, difficulty: Difficulty) -> Self {
        Self { map_id, difficulty }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersonalSpawnMapKey {
    pub map_id: u32,
    pub difficulty: Difficulty,
    pub phase_id: u32,
}

impl PersonalSpawnMapKey {
    pub const fn new(map_id: u32, difficulty: Difficulty, phase_id: u32) -> Self {
        Self {
            map_id,
            difficulty,
            phase_id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpawnStore {
    object_guids: BTreeMap<SpawnMapKey, BTreeMap<u32, CellSpawnGuids>>,
    personal_object_guids: BTreeMap<PersonalSpawnMapKey, BTreeMap<u32, CellSpawnGuids>>,
}

impl SpawnStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `ObjectMgr::AddSpawnDataToGrid` for creature/gameobject spawns.
    pub fn add_object_spawn<F>(&mut self, data: &SpawnData, is_personal_phase: F)
    where
        F: Fn(u32) -> bool,
    {
        match data.object_type {
            SpawnObjectType::Creature | SpawnObjectType::GameObject => {}
            SpawnObjectType::AreaTrigger => {
                self.add_area_trigger_spawn(data);
                return;
            }
        }

        let cell_id = data.cell_id();
        if is_personal_phase(data.phase_id) {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = PersonalSpawnMapKey::new(data.map_id, difficulty, data.phase_id);
                let cell = self
                    .personal_object_guids
                    .entry(key)
                    .or_default()
                    .entry(cell_id)
                    .or_default();
                insert_spawn(cell, data.object_type, data.spawn_id);
            }
        } else {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = SpawnMapKey::new(data.map_id, difficulty);
                let cell = self
                    .object_guids
                    .entry(key)
                    .or_default()
                    .entry(cell_id)
                    .or_default();
                insert_spawn(cell, data.object_type, data.spawn_id);
            }
        }
    }

    /// C++ `AreaTriggerDataStore::LoadAreaTriggerSpawns` indexes static area
    /// triggers by map/difficulty/cell only; it does not use ObjectMgr's
    /// personal-phase store.
    pub fn add_area_trigger_spawn(&mut self, data: &SpawnData) {
        debug_assert_eq!(data.object_type, SpawnObjectType::AreaTrigger);
        let cell_id = data.cell_id();
        for difficulty in data.spawn_difficulties.iter().copied() {
            let key = SpawnMapKey::new(data.map_id, difficulty);
            self.object_guids
                .entry(key)
                .or_default()
                .entry(cell_id)
                .or_default()
                .area_triggers
                .insert(data.spawn_id);
        }
    }

    pub fn remove_object_spawn<F>(&mut self, data: &SpawnData, is_personal_phase: F)
    where
        F: Fn(u32) -> bool,
    {
        match data.object_type {
            SpawnObjectType::Creature | SpawnObjectType::GameObject => {}
            SpawnObjectType::AreaTrigger => {
                self.remove_area_trigger_spawn(data);
                return;
            }
        }

        let cell_id = data.cell_id();
        if is_personal_phase(data.phase_id) {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = PersonalSpawnMapKey::new(data.map_id, difficulty, data.phase_id);
                if let Some(cells) = self.personal_object_guids.get_mut(&key) {
                    remove_spawn_from_cells(cells, cell_id, data.object_type, data.spawn_id);
                }
            }
        } else {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = SpawnMapKey::new(data.map_id, difficulty);
                if let Some(cells) = self.object_guids.get_mut(&key) {
                    remove_spawn_from_cells(cells, cell_id, data.object_type, data.spawn_id);
                }
            }
        }
    }

    pub fn remove_area_trigger_spawn(&mut self, data: &SpawnData) {
        debug_assert_eq!(data.object_type, SpawnObjectType::AreaTrigger);
        let cell_id = data.cell_id();
        for difficulty in data.spawn_difficulties.iter().copied() {
            let key = SpawnMapKey::new(data.map_id, difficulty);
            if let Some(cells) = self.object_guids.get_mut(&key) {
                remove_spawn_from_cells(
                    cells,
                    cell_id,
                    SpawnObjectType::AreaTrigger,
                    data.spawn_id,
                );
            }
        }
    }

    pub fn cell_object_guids(
        &self,
        map_id: u32,
        difficulty: Difficulty,
        cell_id: u32,
    ) -> Option<&CellSpawnGuids> {
        self.object_guids
            .get(&SpawnMapKey::new(map_id, difficulty))?
            .get(&cell_id)
    }

    pub fn cell_personal_object_guids(
        &self,
        map_id: u32,
        difficulty: Difficulty,
        phase_id: u32,
        cell_id: u32,
    ) -> Option<&CellSpawnGuids> {
        self.personal_object_guids
            .get(&PersonalSpawnMapKey::new(map_id, difficulty, phase_id))?
            .get(&cell_id)
    }

    pub fn has_personal_spawns(&self, map_id: u32, difficulty: Difficulty, phase_id: u32) -> bool {
        self.personal_object_guids
            .contains_key(&PersonalSpawnMapKey::new(map_id, difficulty, phase_id))
    }
}

fn insert_spawn(cell: &mut CellSpawnGuids, object_type: SpawnObjectType, spawn_id: SpawnId) {
    match object_type {
        SpawnObjectType::Creature => {
            cell.creatures.insert(spawn_id);
        }
        SpawnObjectType::GameObject => {
            cell.gameobjects.insert(spawn_id);
        }
        SpawnObjectType::AreaTrigger => {
            cell.area_triggers.insert(spawn_id);
        }
    }
}

fn remove_spawn_from_cells(
    cells: &mut BTreeMap<u32, CellSpawnGuids>,
    cell_id: u32,
    object_type: SpawnObjectType,
    spawn_id: SpawnId,
) {
    if let Some(cell) = cells.get_mut(&cell_id) {
        match object_type {
            SpawnObjectType::Creature => {
                cell.creatures.remove(&spawn_id);
            }
            SpawnObjectType::GameObject => {
                cell.gameobjects.remove(&spawn_id);
            }
            SpawnObjectType::AreaTrigger => {
                cell.area_triggers.remove(&spawn_id);
            }
        }
        if cell.is_empty() {
            cells.remove(&cell_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(object_type: SpawnObjectType, spawn_id: SpawnId, x: f32, y: f32) -> SpawnData {
        SpawnData {
            object_type,
            spawn_id,
            map_id: 571,
            db_data: true,
            id: 42,
            spawn_point: SpawnPosition::new(x, y, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![0, 1],
            script_id: 0,
            string_id: String::new(),
        }
    }

    #[test]
    fn spawn_constants_match_spawn_data_h() {
        assert_eq!(SpawnObjectType::Creature as u8, 0);
        assert_eq!(SpawnObjectType::GameObject as u8, 1);
        assert_eq!(SpawnObjectType::AreaTrigger as u8, 2);
        assert_eq!(SpawnObjectType::Creature.mask(), 0x1);
        assert_eq!(SpawnObjectType::GameObject.mask(), 0x2);
        assert_eq!(SpawnObjectType::AreaTrigger.mask(), 0x4);
        assert_eq!(SpawnGroupFlags::ALL.0, 0x3f);
        assert!(
            SpawnGroupFlags(0xff)
                .truncate_to_all()
                .contains(SpawnGroupFlags::SYSTEM)
        );
    }

    #[test]
    fn object_spawn_store_indexes_creatures_by_map_difficulty_and_cell() {
        let mut store = SpawnStore::new();
        let data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |_| false);

        assert!(
            store
                .cell_object_guids(571, 0, cell_id)
                .unwrap()
                .creatures
                .contains(&100)
        );
        assert!(
            store
                .cell_object_guids(571, 1, cell_id)
                .unwrap()
                .creatures
                .contains(&100)
        );
        assert!(store.cell_object_guids(571, 2, cell_id).is_none());
    }

    #[test]
    fn object_spawn_store_uses_personal_phase_index_for_creatures_and_gameobjects() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::GameObject, 200, 0.0, 0.0);
        data.phase_id = 9001;
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |phase_id| phase_id == 9001);

        assert!(store.cell_object_guids(571, 0, cell_id).is_none());
        assert!(store.has_personal_spawns(571, 0, 9001));
        assert!(
            store
                .cell_personal_object_guids(571, 0, 9001, cell_id)
                .unwrap()
                .gameobjects
                .contains(&200)
        );
    }

    #[test]
    fn area_trigger_store_follows_cpp_non_personal_location_index() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::AreaTrigger, 300, 0.0, 0.0);
        data.phase_id = 9001;
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |phase_id| phase_id == 9001);

        assert!(
            store
                .cell_object_guids(571, 0, cell_id)
                .unwrap()
                .area_triggers
                .contains(&300)
        );
        assert!(
            store
                .cell_personal_object_guids(571, 0, 9001, cell_id)
                .is_none()
        );
    }

    #[test]
    fn removing_spawn_cleans_empty_cell_entry() {
        let mut store = SpawnStore::new();
        let data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |_| false);
        assert!(store.cell_object_guids(571, 0, cell_id).is_some());

        store.remove_object_spawn(&data, |_| false);
        assert!(store.cell_object_guids(571, 0, cell_id).is_none());
        assert!(store.cell_object_guids(571, 1, cell_id).is_none());
    }
}
