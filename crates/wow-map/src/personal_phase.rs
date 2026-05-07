//! Personal phase grid tracking.
//!
//! C++ references:
//! - `game/Phasing/PersonalPhaseTracker.h`
//! - `game/Phasing/PersonalPhaseTracker.cpp`

use std::collections::{BTreeMap, BTreeSet};

use wow_core::ObjectGuid;

use crate::grid::NGrid;
use crate::object_grid_loader::{GridSpawnLoadFilter, ObjectGridLoadCounts, ObjectGridLoader};

pub const PERSONAL_PHASE_DELETE_TIME_DEFAULT_MS: i64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseRef {
    pub id: u32,
    pub personal: bool,
}

impl PhaseRef {
    pub const fn new(id: u32, personal: bool) -> Self {
        Self { id, personal }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseShift {
    personal_guid: Option<ObjectGuid>,
    phases: Vec<PhaseRef>,
}

impl PhaseShift {
    pub fn new(personal_guid: Option<ObjectGuid>, phases: Vec<PhaseRef>) -> Self {
        Self {
            personal_guid,
            phases,
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub const fn personal_guid(&self) -> Option<ObjectGuid> {
        self.personal_guid
    }

    pub fn has_personal_phase(&self) -> bool {
        self.personal_guid.is_some() && self.phases.iter().any(|phase| phase.personal)
    }

    pub fn has_phase(&self, phase_id: u32) -> bool {
        self.phases.iter().any(|phase| phase.id == phase_id)
    }

    pub fn phases(&self) -> &[PhaseRef] {
        &self.phases
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonalPhaseSpawns {
    pub objects: BTreeSet<ObjectGuid>,
    pub grids: BTreeSet<u32>,
    pub duration_remaining_ms: Option<i64>,
}

impl PersonalPhaseSpawns {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.grids.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerPersonalPhasesTracker {
    spawns: BTreeMap<u32, PersonalPhaseSpawns>,
}

impl PlayerPersonalPhasesTracker {
    pub fn register_tracked_object(&mut self, phase_id: u32, object: ObjectGuid) {
        self.spawns
            .entry(phase_id)
            .or_default()
            .objects
            .insert(object);
    }

    pub fn unregister_tracked_object(&mut self, object: ObjectGuid) {
        for spawns in self.spawns.values_mut() {
            spawns.objects.remove(&object);
        }
        self.spawns.retain(|_, spawns| !spawns.is_empty());
    }

    pub fn on_owner_phases_changed(&mut self, phase_shift: &PhaseShift) {
        for (phase_id, spawns) in &mut self.spawns {
            if spawns.duration_remaining_ms.is_none() && !phase_shift.has_phase(*phase_id) {
                spawns.duration_remaining_ms = Some(PERSONAL_PHASE_DELETE_TIME_DEFAULT_MS);
            }
        }

        for phase in phase_shift.phases() {
            if let Some(spawns) = self.spawns.get_mut(&phase.id) {
                spawns.duration_remaining_ms = None;
            }
        }
    }

    pub fn mark_all_phases_for_deletion(&mut self) {
        for spawns in self.spawns.values_mut() {
            spawns.duration_remaining_ms = Some(PERSONAL_PHASE_DELETE_TIME_DEFAULT_MS);
        }
    }

    pub fn update(&mut self, diff_ms: u32) -> Vec<ObjectGuid> {
        let mut despawned = Vec::new();
        let mut empty_phases = Vec::new();

        for (phase_id, spawns) in &mut self.spawns {
            if let Some(remaining) = spawns.duration_remaining_ms.as_mut() {
                *remaining -= i64::from(diff_ms);
                if *remaining <= 0 {
                    despawned.extend(spawns.objects.iter().copied());
                    spawns.objects.clear();
                    spawns.grids.clear();
                    empty_phases.push(*phase_id);
                }
            }
        }

        for phase_id in empty_phases {
            self.spawns.remove(&phase_id);
        }

        despawned
    }

    pub fn is_grid_loaded_for_phase(&self, grid_id: u32, phase_id: u32) -> bool {
        self.spawns
            .get(&phase_id)
            .is_some_and(|spawns| spawns.grids.contains(&grid_id))
    }

    pub fn set_grid_loaded_for_phase(&mut self, grid_id: u32, phase_id: u32) {
        self.spawns
            .entry(phase_id)
            .or_default()
            .grids
            .insert(grid_id);
    }

    pub fn set_grid_unloaded(&mut self, grid_id: u32) {
        for spawns in self.spawns.values_mut() {
            spawns.grids.remove(&grid_id);
        }
        self.spawns.retain(|_, spawns| !spawns.is_empty());
    }

    pub fn is_empty(&self) -> bool {
        self.spawns.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiPersonalPhaseTracker {
    player_data: BTreeMap<ObjectGuid, PlayerPersonalPhasesTracker>,
}

impl MultiPersonalPhaseTracker {
    pub fn load_grid<Filter>(
        &mut self,
        phase_shift: &PhaseShift,
        grid: &mut NGrid,
        loader: &mut ObjectGridLoader<'_, Filter>,
    ) -> ObjectGridLoadCounts
    where
        Filter: GridSpawnLoadFilter,
    {
        if !phase_shift.has_personal_phase() {
            return ObjectGridLoadCounts::default();
        }

        let phase_owner = phase_shift
            .personal_guid()
            .expect("personal phase shifts must have a personal owner guid");
        let player_tracker = self.player_data.entry(phase_owner).or_default();
        let mut total = ObjectGridLoadCounts::default();

        for phase in phase_shift.phases() {
            if !phase.personal {
                continue;
            }

            if !loader.has_personal_spawns(phase.id) {
                continue;
            }

            if player_tracker.is_grid_loaded_for_phase(grid.grid_id(), phase.id) {
                continue;
            }

            let counts = loader.load_personal_phase(grid, phase.id);
            total.gameobjects += counts.gameobjects;
            total.creatures += counts.creatures;
            total.corpses += counts.corpses;
            total.area_triggers += counts.area_triggers;
            player_tracker.set_grid_loaded_for_phase(grid.grid_id(), phase.id);
        }

        total
    }

    pub fn unload_grid(&mut self, grid: &NGrid) {
        for tracker in self.player_data.values_mut() {
            tracker.set_grid_unloaded(grid.grid_id());
        }
        self.player_data.retain(|_, tracker| !tracker.is_empty());
    }

    pub fn register_tracked_object(
        &mut self,
        phase_id: u32,
        phase_owner: ObjectGuid,
        object: ObjectGuid,
    ) {
        assert!(phase_id != 0);
        assert!(phase_owner != ObjectGuid::EMPTY);
        assert!(object != ObjectGuid::EMPTY);
        self.player_data
            .entry(phase_owner)
            .or_default()
            .register_tracked_object(phase_id, object);
    }

    pub fn unregister_tracked_object(&mut self, phase_owner: ObjectGuid, object: ObjectGuid) {
        if let Some(tracker) = self.player_data.get_mut(&phase_owner) {
            tracker.unregister_tracked_object(object);
        }
        self.player_data.retain(|_, tracker| !tracker.is_empty());
    }

    pub fn on_owner_phase_changed<Filter>(
        &mut self,
        phase_owner: ObjectGuid,
        phase_shift: &PhaseShift,
        grid: Option<&mut NGrid>,
        loader: &mut ObjectGridLoader<'_, Filter>,
    ) -> ObjectGridLoadCounts
    where
        Filter: GridSpawnLoadFilter,
    {
        if let Some(tracker) = self.player_data.get_mut(&phase_owner) {
            tracker.on_owner_phases_changed(phase_shift);
        }

        if let Some(grid) = grid {
            self.load_grid(phase_shift, grid, loader)
        } else {
            ObjectGridLoadCounts::default()
        }
    }

    pub fn mark_all_phases_for_deletion(&mut self, phase_owner: ObjectGuid) {
        if let Some(tracker) = self.player_data.get_mut(&phase_owner) {
            tracker.mark_all_phases_for_deletion();
        }
    }

    pub fn update(&mut self, diff_ms: u32) -> Vec<ObjectGuid> {
        let mut despawned = Vec::new();
        for tracker in self.player_data.values_mut() {
            despawned.extend(tracker.update(diff_ms));
        }
        self.player_data.retain(|_, tracker| !tracker.is_empty());
        despawned
    }

    pub fn tracker_count(&self) -> usize {
        self.player_data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::guid::HighGuid;

    use crate::object_grid_loader::{CorpseCellStore, ObjectGridLoader};
    use crate::spawn::{SpawnData, SpawnObjectType, SpawnPosition, SpawnStore};

    fn owner_guid() -> ObjectGuid {
        ObjectGuid::create_player(1, 100)
    }

    fn personal_spawn(object_type: SpawnObjectType, spawn_id: u64, phase_id: u32) -> SpawnData {
        SpawnData {
            object_type,
            spawn_id,
            map_id: 571,
            db_data: true,
            id: 42,
            spawn_point: SpawnPosition::new(0.0, 0.0, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        }
    }

    #[test]
    fn load_grid_loads_each_personal_phase_once_per_owner_and_grid() {
        let mut store = SpawnStore::new();
        let spawn = personal_spawn(SpawnObjectType::Creature, 100, 9);
        store.add_object_spawn(&spawn, |phase_id| phase_id == 9);
        let corpses = CorpseCellStore::new();
        let mut loader = ObjectGridLoader::new(&store, &corpses, 571, 1, 1, 1);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let phase_shift = PhaseShift::new(Some(owner_guid()), vec![PhaseRef::new(9, true)]);
        let mut tracker = MultiPersonalPhaseTracker::default();

        let first = tracker.load_grid(&phase_shift, &mut grid, &mut loader);
        let second = tracker.load_grid(&phase_shift, &mut grid, &mut loader);

        assert_eq!(first.creatures, 1);
        assert_eq!(second.creatures, 0);
        assert_eq!(
            grid.get_grid_type(0, 0)
                .unwrap()
                .grid_objects
                .creatures
                .len(),
            1
        );
        assert_eq!(tracker.tracker_count(), 1);
    }

    #[test]
    fn load_grid_skips_non_personal_phases_and_missing_personal_spawn_sets() {
        let store = SpawnStore::new();
        let corpses = CorpseCellStore::new();
        let mut loader = ObjectGridLoader::new(&store, &corpses, 571, 1, 1, 1);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let phase_shift = PhaseShift::new(
            Some(owner_guid()),
            vec![PhaseRef::new(7, false), PhaseRef::new(9, true)],
        );
        let mut tracker = MultiPersonalPhaseTracker::default();

        let counts = tracker.load_grid(&phase_shift, &mut grid, &mut loader);

        assert_eq!(counts.creatures, 0);
        assert_eq!(tracker.tracker_count(), 1);
    }

    #[test]
    fn unload_grid_removes_grid_tracking_and_empty_owner_trackers() {
        let mut tracker = MultiPersonalPhaseTracker::default();
        let owner = owner_guid();
        let grid = NGrid::from_coords(32, 32, 1000, true);
        tracker
            .player_data
            .entry(owner)
            .or_default()
            .set_grid_loaded_for_phase(grid.grid_id(), 9);

        tracker.unload_grid(&grid);

        assert_eq!(tracker.tracker_count(), 0);
    }

    #[test]
    fn owner_phase_change_marks_missing_phases_for_deletion_and_update_despawns() {
        let owner = owner_guid();
        let object = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 571, 1, 42, 100);
        let mut player = PlayerPersonalPhasesTracker::default();
        player.register_tracked_object(9, object);

        player.on_owner_phases_changed(&PhaseShift::empty());
        let despawned = player.update(PERSONAL_PHASE_DELETE_TIME_DEFAULT_MS as u32);

        assert_eq!(despawned, vec![object]);
        assert!(player.is_empty());
        assert_ne!(owner, ObjectGuid::EMPTY);
    }

    #[test]
    fn mark_all_phases_for_deletion_uses_default_one_minute_timer() {
        let object = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 571, 1, 42, 100);
        let mut player = PlayerPersonalPhasesTracker::default();
        player.register_tracked_object(9, object);

        player.mark_all_phases_for_deletion();
        assert!(player.update(59_999).is_empty());
        assert_eq!(player.update(1), vec![object]);
    }
}
