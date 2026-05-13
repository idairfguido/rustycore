//! C++ `PersonalPhaseTracker` state machines with runtime map operations injected by callers.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use wow_core::ObjectGuid;
use wow_entities::PhaseShift;

pub const DELETE_TIME_DEFAULT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersonalPhaseSpawns {
    objects: HashSet<ObjectGuid>,
    grids: HashSet<u16>,
    duration_remaining: Option<Duration>,
}

impl PersonalPhaseSpawns {
    pub fn is_empty_like_cpp(&self) -> bool {
        self.objects.is_empty() && self.grids.is_empty()
    }

    pub fn objects_like_cpp(&self) -> impl Iterator<Item = ObjectGuid> + '_ {
        self.objects.iter().copied()
    }

    pub fn grids_like_cpp(&self) -> impl Iterator<Item = u16> + '_ {
        self.grids.iter().copied()
    }

    pub const fn duration_remaining_like_cpp(&self) -> Option<Duration> {
        self.duration_remaining
    }
}

/// C++ `PlayerPersonalPhasesTracker`, tracking personal phases for one owner.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerPersonalPhasesTracker {
    spawns: HashMap<u32, PersonalPhaseSpawns>,
}

impl PlayerPersonalPhasesTracker {
    pub fn register_tracked_object_like_cpp(&mut self, phase_id: u32, object: ObjectGuid) {
        self.spawns
            .entry(phase_id)
            .or_default()
            .objects
            .insert(object);
    }

    pub fn unregister_tracked_object_like_cpp(&mut self, object: ObjectGuid) {
        for spawns in self.spawns.values_mut() {
            spawns.objects.remove(&object);
        }
    }

    pub fn on_owner_phases_changed_like_cpp(&mut self, owner_phase_shift: &PhaseShift) {
        for (phase_id, spawns) in &mut self.spawns {
            if spawns.duration_remaining.is_none()
                && !owner_phase_shift.has_phase_like_cpp(*phase_id)
            {
                spawns.duration_remaining = Some(DELETE_TIME_DEFAULT);
            }
        }

        for phase_ref in owner_phase_shift.phases_like_cpp() {
            if let Some(spawns) = self.spawns.get_mut(&phase_ref.id()) {
                spawns.duration_remaining = None;
            }
        }
    }

    pub fn mark_all_phases_for_deletion_like_cpp(&mut self) {
        for spawns in self.spawns.values_mut() {
            spawns.duration_remaining = Some(DELETE_TIME_DEFAULT);
        }
    }

    pub fn update_like_cpp(&mut self, diff: Duration, mut despawn_object: impl FnMut(ObjectGuid)) {
        let mut expired_phases = Vec::new();
        for (phase_id, spawns) in &mut self.spawns {
            let Some(remaining) = spawns.duration_remaining else {
                continue;
            };

            let remaining = remaining.saturating_sub(diff);
            spawns.duration_remaining = Some(remaining);
            if remaining.is_zero() {
                for object in spawns.objects_like_cpp() {
                    despawn_object(object);
                }
                spawns.objects.clear();
                spawns.grids.clear();
                expired_phases.push(*phase_id);
            }
        }

        for phase_id in expired_phases {
            self.spawns.remove(&phase_id);
        }
    }

    pub fn is_grid_loaded_for_phase_like_cpp(&self, grid_id: u16, phase_id: u32) -> bool {
        self.spawns
            .get(&phase_id)
            .is_some_and(|spawns| spawns.grids.contains(&grid_id))
    }

    pub fn set_grid_loaded_for_phase_like_cpp(&mut self, grid_id: u16, phase_id: u32) {
        self.spawns
            .entry(phase_id)
            .or_default()
            .grids
            .insert(grid_id);
    }

    pub fn set_grid_unloaded_like_cpp(&mut self, grid_id: u16) {
        self.spawns.retain(|_, spawns| {
            spawns.grids.remove(&grid_id);
            !spawns.is_empty_like_cpp()
        });
    }

    pub fn is_empty_like_cpp(&self) -> bool {
        self.spawns.is_empty()
    }

    pub fn phase_spawns_like_cpp(&self, phase_id: u32) -> Option<&PersonalPhaseSpawns> {
        self.spawns.get(&phase_id)
    }

    pub fn phase_count_like_cpp(&self) -> usize {
        self.spawns.len()
    }
}

/// C++ `MultiPersonalPhaseTracker`, tracking all personal-phase owners on one map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultiPersonalPhaseTracker {
    player_data: HashMap<ObjectGuid, PlayerPersonalPhasesTracker>,
}

impl MultiPersonalPhaseTracker {
    pub fn load_grid_like_cpp(
        &mut self,
        phase_shift: &PhaseShift,
        grid_id: u16,
        mut has_personal_spawns: impl FnMut(u32) -> bool,
        mut load_phase: impl FnMut(ObjectGuid, u32),
    ) -> bool {
        if !phase_shift.has_personal_phase_like_cpp() {
            return false;
        }

        let phase_owner = phase_shift.personal_guid_like_cpp();
        let player_tracker = self.player_data.entry(phase_owner).or_default();
        let mut loaded_any = false;

        for phase_ref in phase_shift.phases_like_cpp() {
            if !phase_ref.is_personal() {
                continue;
            }

            if !has_personal_spawns(phase_ref.id()) {
                continue;
            }

            if player_tracker.is_grid_loaded_for_phase_like_cpp(grid_id, phase_ref.id()) {
                continue;
            }

            load_phase(phase_owner, phase_ref.id());
            player_tracker.set_grid_loaded_for_phase_like_cpp(grid_id, phase_ref.id());
            loaded_any = true;
        }

        loaded_any
    }

    pub fn unload_grid_like_cpp(&mut self, grid_id: u16) {
        self.player_data.retain(|_, player_tracker| {
            player_tracker.set_grid_unloaded_like_cpp(grid_id);
            !player_tracker.is_empty_like_cpp()
        });
    }

    pub fn register_tracked_object_like_cpp(
        &mut self,
        phase_id: u32,
        phase_owner: ObjectGuid,
        object: ObjectGuid,
    ) {
        assert!(phase_id != 0);
        assert!(!phase_owner.is_empty());
        assert!(!object.is_empty());

        self.player_data
            .entry(phase_owner)
            .or_default()
            .register_tracked_object_like_cpp(phase_id, object);
    }

    pub fn unregister_tracked_object_like_cpp(
        &mut self,
        phase_owner: ObjectGuid,
        object: ObjectGuid,
    ) {
        if let Some(player_tracker) = self.player_data.get_mut(&phase_owner) {
            player_tracker.unregister_tracked_object_like_cpp(object);
        }
    }

    pub fn on_owner_phase_changed_like_cpp(
        &mut self,
        phase_owner: ObjectGuid,
        owner_phase_shift: &PhaseShift,
        grid_id: Option<u16>,
        has_personal_spawns: impl FnMut(u32) -> bool,
        load_phase: impl FnMut(ObjectGuid, u32),
    ) -> bool {
        if let Some(player_tracker) = self.player_data.get_mut(&phase_owner) {
            player_tracker.on_owner_phases_changed_like_cpp(owner_phase_shift);
        }

        if let Some(grid_id) = grid_id {
            return self.load_grid_like_cpp(
                owner_phase_shift,
                grid_id,
                has_personal_spawns,
                load_phase,
            );
        }

        false
    }

    pub fn mark_all_phases_for_deletion_like_cpp(&mut self, phase_owner: ObjectGuid) {
        if let Some(player_tracker) = self.player_data.get_mut(&phase_owner) {
            player_tracker.mark_all_phases_for_deletion_like_cpp();
        }
    }

    pub fn update_like_cpp(&mut self, diff: Duration, mut despawn_object: impl FnMut(ObjectGuid)) {
        self.player_data.retain(|_, player_tracker| {
            player_tracker.update_like_cpp(diff, &mut despawn_object);
            !player_tracker.is_empty_like_cpp()
        });
    }

    pub fn owner_tracker_like_cpp(
        &self,
        phase_owner: ObjectGuid,
    ) -> Option<&PlayerPersonalPhasesTracker> {
        self.player_data.get(&phase_owner)
    }

    pub fn owner_count_like_cpp(&self) -> usize {
        self.player_data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::PhaseFlags;
    use wow_core::guid::HighGuid;

    fn player_guid(low: i64) -> ObjectGuid {
        ObjectGuid::create_player(1, low)
    }

    fn object_guid(low: i64) -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::Creature, 0, 0, 571, 0, 1, low)
    }

    fn personal_phase_shift(
        owner: ObjectGuid,
        phases: impl IntoIterator<Item = u32>,
    ) -> PhaseShift {
        let mut phase_shift = PhaseShift::default();
        for phase_id in phases {
            phase_shift.add_phase_like_cpp(phase_id, PhaseFlags::PERSONAL, 1);
        }
        phase_shift.set_personal_guid_like_cpp(owner);
        phase_shift
    }

    #[test]
    fn player_tracker_marks_missing_owner_phases_for_deletion_and_resets_existing_like_cpp() {
        let object = object_guid(100);
        let mut tracker = PlayerPersonalPhasesTracker::default();
        tracker.register_tracked_object_like_cpp(10, object);
        tracker.register_tracked_object_like_cpp(20, object);

        tracker.on_owner_phases_changed_like_cpp(&personal_phase_shift(player_guid(1), [10]));

        assert_eq!(
            tracker
                .phase_spawns_like_cpp(10)
                .and_then(PersonalPhaseSpawns::duration_remaining_like_cpp),
            None
        );
        assert_eq!(
            tracker
                .phase_spawns_like_cpp(20)
                .and_then(PersonalPhaseSpawns::duration_remaining_like_cpp),
            Some(DELETE_TIME_DEFAULT)
        );

        tracker.on_owner_phases_changed_like_cpp(&personal_phase_shift(player_guid(1), [10, 20]));
        assert_eq!(
            tracker
                .phase_spawns_like_cpp(20)
                .and_then(PersonalPhaseSpawns::duration_remaining_like_cpp),
            None
        );
    }

    #[test]
    fn player_tracker_update_despawns_expired_phase_like_cpp() {
        let object = object_guid(100);
        let mut tracker = PlayerPersonalPhasesTracker::default();
        tracker.register_tracked_object_like_cpp(10, object);
        tracker.set_grid_loaded_for_phase_like_cpp(37, 10);
        tracker.mark_all_phases_for_deletion_like_cpp();

        let mut despawned = Vec::new();
        tracker.update_like_cpp(Duration::from_secs(59), |guid| despawned.push(guid));
        assert!(despawned.is_empty());
        assert_eq!(tracker.phase_count_like_cpp(), 1);

        tracker.update_like_cpp(Duration::from_secs(1), |guid| despawned.push(guid));
        assert_eq!(despawned, vec![object]);
        assert!(tracker.is_empty_like_cpp());
    }

    #[test]
    fn player_tracker_grid_unload_erases_empty_phases_like_cpp() {
        let mut tracker = PlayerPersonalPhasesTracker::default();
        tracker.set_grid_loaded_for_phase_like_cpp(37, 10);
        tracker.set_grid_loaded_for_phase_like_cpp(38, 20);
        tracker.register_tracked_object_like_cpp(20, object_guid(200));

        tracker.set_grid_unloaded_like_cpp(37);
        assert!(!tracker.is_grid_loaded_for_phase_like_cpp(37, 10));
        assert!(tracker.phase_spawns_like_cpp(10).is_none());
        assert!(tracker.phase_spawns_like_cpp(20).is_some());
    }

    #[test]
    fn multi_tracker_load_grid_only_loads_owner_personal_phases_once_like_cpp() {
        let owner = player_guid(1);
        let mut tracker = MultiPersonalPhaseTracker::default();
        let mut phase_shift = personal_phase_shift(owner, [10, 20]);
        phase_shift.add_phase_like_cpp(30, PhaseFlags::NONE, 1);

        let mut loaded = Vec::new();
        assert!(tracker.load_grid_like_cpp(
            &phase_shift,
            37,
            |phase_id| phase_id == 10,
            |owner, phase_id| loaded.push((owner, phase_id)),
        ));
        assert_eq!(loaded, vec![(owner, 10)]);

        assert!(!tracker.load_grid_like_cpp(
            &phase_shift,
            37,
            |phase_id| phase_id == 10,
            |owner, phase_id| loaded.push((owner, phase_id)),
        ));
        assert_eq!(loaded, vec![(owner, 10)]);
    }

    #[test]
    fn multi_tracker_owner_phase_change_marks_old_and_loads_new_grid_like_cpp() {
        let owner = player_guid(1);
        let old_object = object_guid(100);
        let mut tracker = MultiPersonalPhaseTracker::default();
        tracker.register_tracked_object_like_cpp(10, owner, old_object);

        let mut loaded = Vec::new();
        let loaded_any = tracker.on_owner_phase_changed_like_cpp(
            owner,
            &personal_phase_shift(owner, [20]),
            Some(37),
            |phase_id| phase_id == 20,
            |owner, phase_id| loaded.push((owner, phase_id)),
        );

        assert!(loaded_any);
        assert_eq!(loaded, vec![(owner, 20)]);
        let owner_tracker = tracker.owner_tracker_like_cpp(owner).unwrap();
        assert_eq!(
            owner_tracker
                .phase_spawns_like_cpp(10)
                .and_then(PersonalPhaseSpawns::duration_remaining_like_cpp),
            Some(DELETE_TIME_DEFAULT)
        );
        assert!(owner_tracker.is_grid_loaded_for_phase_like_cpp(37, 20));
    }
}
