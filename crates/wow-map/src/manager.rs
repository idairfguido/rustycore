//! MapManager skeleton.
//!
//! C++ references:
//! - `game/Maps/MapManager.h`
//! - `game/Maps/MapManager.cpp`

use std::collections::BTreeMap;

use crate::MapKey;
use crate::map::{Map, NoopGridLifecycle, NoopTerrainGridLoader};
use crate::spawn::Difficulty;

pub const MIN_GRID_DELAY_MS: u32 = 60_000;
pub const MIN_MAP_UPDATE_DELAY_MS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedMapKind {
    World,
    Dungeon { has_reset_schedule: bool },
    Battleground,
}

impl ManagedMapKind {
    pub const fn is_dungeon(self) -> bool {
        matches!(self, Self::Dungeon { .. })
    }

    pub const fn is_battleground_or_arena(self) -> bool {
        matches!(self, Self::Battleground)
    }

    pub const fn frees_instance_id_on_destroy(self) -> bool {
        match self {
            Self::Battleground => true,
            Self::Dungeon { has_reset_schedule } => !has_reset_schedule,
            Self::World => false,
        }
    }
}

#[derive(Debug)]
pub struct ManagedMap {
    map: Map<NoopTerrainGridLoader, NoopGridLifecycle>,
    kind: ManagedMapKind,
    can_unload: bool,
    player_count: u32,
    update_calls: Vec<u32>,
    delayed_update_calls: Vec<u32>,
    unload_all_calls: u32,
}

impl ManagedMap {
    pub fn new(
        map_id: u32,
        instance_id: u32,
        difficulty: Difficulty,
        grid_expiry_ms: i64,
        kind: ManagedMapKind,
    ) -> Self {
        Self {
            map: Map::new(map_id, instance_id, difficulty, grid_expiry_ms),
            kind,
            can_unload: false,
            player_count: 0,
            update_calls: Vec::new(),
            delayed_update_calls: Vec::new(),
            unload_all_calls: 0,
        }
    }

    pub const fn map_id(&self) -> u32 {
        self.map.map_id()
    }

    pub const fn instance_id(&self) -> u32 {
        self.map.instance_id()
    }

    pub const fn kind(&self) -> ManagedMapKind {
        self.kind
    }

    pub fn map(&self) -> &Map<NoopTerrainGridLoader, NoopGridLifecycle> {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut Map<NoopTerrainGridLoader, NoopGridLifecycle> {
        &mut self.map
    }

    pub fn set_can_unload(&mut self, can_unload: bool) {
        self.can_unload = can_unload;
    }

    pub fn set_player_count(&mut self, player_count: u32) {
        self.player_count = player_count;
    }

    pub const fn player_count(&self) -> u32 {
        self.player_count
    }

    pub fn update_calls(&self) -> &[u32] {
        &self.update_calls
    }

    pub fn delayed_update_calls(&self) -> &[u32] {
        &self.delayed_update_calls
    }

    pub const fn unload_all_calls(&self) -> u32 {
        self.unload_all_calls
    }

    fn can_unload(&self, _diff_ms: u32) -> bool {
        self.can_unload
    }

    fn remove_all_players(&mut self) {
        self.player_count = 0;
    }

    fn have_players(&self) -> bool {
        self.player_count > 0
    }

    fn update(&mut self, diff_ms: u32) {
        self.update_calls.push(diff_ms);
    }

    fn delayed_update(&mut self, diff_ms: u32) {
        self.delayed_update_calls.push(diff_ms);
    }

    fn unload_all(&mut self) {
        self.unload_all_calls += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceIdAllocator {
    free_instance_ids: Vec<bool>,
    next_instance_id: u32,
}

impl Default for InstanceIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceIdAllocator {
    pub fn new() -> Self {
        Self {
            free_instance_ids: vec![false, true],
            next_instance_id: 1,
        }
    }

    pub fn init_instance_ids(&mut self, max_existing_instance_id: u64) {
        self.next_instance_id = 1;
        self.free_instance_ids = vec![true; max_existing_instance_id as usize + 2];
        self.free_instance_ids[0] = false;
    }

    pub fn register_instance_id(&mut self, instance_id: u32) {
        self.ensure_len(instance_id as usize + 1);
        self.free_instance_ids[instance_id as usize] = false;
        if self.next_instance_id == instance_id {
            self.next_instance_id += 1;
        }
    }

    pub fn generate_instance_id(&mut self) -> Option<u32> {
        if self.next_instance_id == u32::MAX {
            return None;
        }

        let new_instance_id = self.next_instance_id;
        self.ensure_len(new_instance_id as usize + 1);
        self.free_instance_ids[new_instance_id as usize] = false;

        let search_start = self.next_instance_id.saturating_add(1) as usize;
        if let Some(next) = self
            .free_instance_ids
            .iter()
            .enumerate()
            .skip(search_start)
            .find_map(|(index, free)| (*free).then_some(index as u32))
        {
            self.next_instance_id = next;
        } else {
            self.next_instance_id = self.free_instance_ids.len() as u32;
            self.free_instance_ids.push(true);
        }

        Some(new_instance_id)
    }

    pub fn free_instance_id(&mut self, instance_id: u32) {
        self.ensure_len(instance_id as usize + 1);
        self.next_instance_id = self.next_instance_id.min(instance_id);
        self.free_instance_ids[instance_id as usize] = true;
    }

    pub const fn next_instance_id(&self) -> u32 {
        self.next_instance_id
    }

    fn ensure_len(&mut self, len: usize) {
        if self.free_instance_ids.len() < len {
            self.free_instance_ids.resize(len, true);
            self.free_instance_ids[0] = false;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntervalTimer {
    interval_ms: u32,
    current_ms: u32,
}

impl IntervalTimer {
    const fn new(interval_ms: u32) -> Self {
        Self {
            interval_ms,
            current_ms: 0,
        }
    }

    fn set_interval(&mut self, interval_ms: u32) {
        self.interval_ms = interval_ms.max(MIN_MAP_UPDATE_DELAY_MS);
    }

    fn reset(&mut self) {
        self.current_ms = 0;
    }

    fn update(&mut self, diff_ms: u32) {
        self.current_ms = self.current_ms.saturating_add(diff_ms);
    }

    const fn passed(self) -> bool {
        self.current_ms >= self.interval_ms
    }

    const fn current(self) -> u32 {
        self.current_ms
    }

    fn set_current(&mut self, current_ms: u32) {
        self.current_ms = current_ms;
    }
}

#[derive(Debug)]
pub struct MapManager {
    grid_cleanup_delay_ms: u32,
    maps: BTreeMap<MapKey, ManagedMap>,
    timer: IntervalTimer,
    instance_ids: InstanceIdAllocator,
    updater: MapUpdater,
    scheduled_scripts: usize,
}

impl Default for MapManager {
    fn default() -> Self {
        Self::new(MIN_GRID_DELAY_MS, MIN_MAP_UPDATE_DELAY_MS)
    }
}

impl MapManager {
    pub fn new(grid_cleanup_delay_ms: u32, map_update_interval_ms: u32) -> Self {
        let mut manager = Self {
            grid_cleanup_delay_ms: MIN_GRID_DELAY_MS,
            maps: BTreeMap::new(),
            timer: IntervalTimer::new(MIN_MAP_UPDATE_DELAY_MS),
            instance_ids: InstanceIdAllocator::new(),
            updater: MapUpdater::default(),
            scheduled_scripts: 0,
        };
        manager.set_grid_cleanup_delay(grid_cleanup_delay_ms);
        manager.set_map_update_interval(map_update_interval_ms);
        manager
    }

    pub const fn grid_cleanup_delay_ms(&self) -> u32 {
        self.grid_cleanup_delay_ms
    }

    pub fn set_grid_cleanup_delay(&mut self, delay_ms: u32) {
        self.grid_cleanup_delay_ms = delay_ms.max(MIN_GRID_DELAY_MS);
    }

    pub fn set_map_update_interval(&mut self, interval_ms: u32) {
        self.timer.set_interval(interval_ms);
        self.timer.reset();
    }

    pub fn create_world_map(&mut self, map_id: u32, instance_id: u32) -> &mut ManagedMap {
        self.create_map_entry(map_id, instance_id, 0, ManagedMapKind::World)
    }

    pub fn create_map_entry(
        &mut self,
        map_id: u32,
        instance_id: u32,
        difficulty: Difficulty,
        kind: ManagedMapKind,
    ) -> &mut ManagedMap {
        let key = MapKey::new(map_id, instance_id);
        self.maps.entry(key).or_insert_with(|| {
            ManagedMap::new(
                map_id,
                instance_id,
                difficulty,
                i64::from(self.grid_cleanup_delay_ms),
                kind,
            )
        })
    }

    pub fn find_map(&self, map_id: u32, instance_id: u32) -> Option<&ManagedMap> {
        self.maps.get(&MapKey::new(map_id, instance_id))
    }

    pub fn find_map_mut(&mut self, map_id: u32, instance_id: u32) -> Option<&mut ManagedMap> {
        self.maps.get_mut(&MapKey::new(map_id, instance_id))
    }

    pub fn do_for_all_maps<F>(&self, mut worker: F)
    where
        F: FnMut(&ManagedMap),
    {
        for map in self.maps.values() {
            worker(map);
        }
    }

    pub fn do_for_all_maps_with_map_id<F>(&self, map_id: u32, mut worker: F)
    where
        F: FnMut(&ManagedMap),
    {
        let start = MapKey::new(map_id, 0);
        let end = MapKey::new(map_id, u32::MAX);
        for (_, map) in self.maps.range(start..=end) {
            worker(map);
        }
    }

    pub fn update(&mut self, diff_ms: u32) {
        self.timer.update(diff_ms);
        if !self.timer.passed() {
            return;
        }

        let current = self.timer.current();
        let keys: Vec<MapKey> = self.maps.keys().copied().collect();
        let mut destroyed = Vec::new();

        for key in keys {
            let Some(map) = self.maps.get_mut(&key) else {
                continue;
            };

            if map.can_unload(diff_ms) {
                if Self::destroy_map_inner(map, &mut self.instance_ids) {
                    destroyed.push(key);
                }
                continue;
            }

            if self.updater.activated() {
                self.updater.schedule_update(map, current);
            } else {
                map.update(current);
            }
        }

        if self.updater.activated() {
            self.updater.wait();
        }

        for key in destroyed {
            self.maps.remove(&key);
        }

        for map in self.maps.values_mut() {
            map.delayed_update(current);
        }

        self.timer.set_current(0);
    }

    pub fn destroy_map(&mut self, map_id: u32, instance_id: u32) -> bool {
        let key = MapKey::new(map_id, instance_id);
        let Some(map) = self.maps.get_mut(&key) else {
            return false;
        };

        if Self::destroy_map_inner(map, &mut self.instance_ids) {
            self.maps.remove(&key);
            true
        } else {
            false
        }
    }

    fn destroy_map_inner(map: &mut ManagedMap, instance_ids: &mut InstanceIdAllocator) -> bool {
        map.remove_all_players();
        if map.have_players() {
            return false;
        }

        map.unload_all();

        if map.kind().frees_instance_id_on_destroy() {
            instance_ids.free_instance_id(map.instance_id());
        }

        true
    }

    pub fn unload_all(&mut self) {
        for map in self.maps.values_mut() {
            map.unload_all();
        }
        self.maps.clear();
    }

    pub fn num_instances(&self) -> u32 {
        self.maps
            .values()
            .filter(|map| map.kind().is_dungeon())
            .count() as u32
    }

    pub fn num_players_in_instances(&self) -> u32 {
        self.maps
            .values()
            .filter(|map| map.kind().is_dungeon())
            .map(ManagedMap::player_count)
            .sum()
    }

    pub fn init_instance_ids(&mut self, max_existing_instance_id: u64) {
        self.instance_ids
            .init_instance_ids(max_existing_instance_id);
    }

    pub fn register_instance_id(&mut self, instance_id: u32) {
        self.instance_ids.register_instance_id(instance_id);
    }

    pub fn generate_instance_id(&mut self) -> Option<u32> {
        self.instance_ids.generate_instance_id()
    }

    pub fn free_instance_id(&mut self, instance_id: u32) {
        self.instance_ids.free_instance_id(instance_id);
    }

    pub fn next_instance_id(&self) -> u32 {
        self.instance_ids.next_instance_id()
    }

    pub fn map_updater(&self) -> &MapUpdater {
        &self.updater
    }

    pub fn map_updater_mut(&mut self) -> &mut MapUpdater {
        &mut self.updater
    }

    pub fn increase_scheduled_scripts_count(&mut self) {
        self.scheduled_scripts += 1;
    }

    pub fn decrease_scheduled_script_count(&mut self) {
        self.scheduled_scripts = self.scheduled_scripts.saturating_sub(1);
    }

    pub fn decrease_scheduled_script_count_by(&mut self, count: usize) {
        self.scheduled_scripts = self.scheduled_scripts.saturating_sub(count);
    }

    pub const fn is_script_scheduled(&self) -> bool {
        self.scheduled_scripts > 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapUpdater {
    worker_threads: usize,
    pending_requests: usize,
    scheduled_updates: usize,
    wait_calls: usize,
}

impl MapUpdater {
    pub fn activate(&mut self, num_threads: usize) {
        self.worker_threads = self.worker_threads.saturating_add(num_threads);
    }

    pub fn deactivate(&mut self) {
        self.wait();
        self.worker_threads = 0;
    }

    pub const fn activated(&self) -> bool {
        self.worker_threads > 0
    }

    pub fn schedule_update(&mut self, map: &mut ManagedMap, diff_ms: u32) {
        self.pending_requests += 1;
        self.scheduled_updates += 1;
        map.update(diff_ms);
        self.update_finished();
    }

    pub fn wait(&mut self) {
        self.wait_calls += 1;
        debug_assert_eq!(self.pending_requests, 0);
    }

    pub const fn scheduled_updates(&self) -> usize {
        self.scheduled_updates
    }

    pub const fn wait_calls(&self) -> usize {
        self.wait_calls
    }

    fn update_finished(&mut self) {
        self.pending_requests = self.pending_requests.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_are_clamped_like_map_manager_h() {
        let manager = MapManager::new(1, 0);

        assert_eq!(manager.grid_cleanup_delay_ms(), MIN_GRID_DELAY_MS);
    }

    #[test]
    fn create_and_find_map_uses_cpp_map_key_shape() {
        let mut manager = MapManager::default();

        manager.create_world_map(1, 0);

        let map = manager.find_map(1, 0).unwrap();
        assert_eq!(map.map_id(), 1);
        assert_eq!(map.instance_id(), 0);
        assert!(manager.find_map(1, 1).is_none());
    }

    #[test]
    fn do_for_all_maps_with_map_id_uses_ordered_pair_range() {
        let mut manager = MapManager::default();
        manager.create_world_map(1, 0);
        manager.create_world_map(2, 0);
        manager.create_map_entry(1, 3, 0, ManagedMapKind::World);

        let mut keys = Vec::new();
        manager.do_for_all_maps_with_map_id(1, |map| {
            keys.push((map.map_id(), map.instance_id()));
        });

        assert_eq!(keys, vec![(1, 0), (1, 3)]);
    }

    #[test]
    fn update_waits_for_interval_then_updates_and_delayed_updates_maps() {
        let mut manager = MapManager::new(MIN_GRID_DELAY_MS, 10);
        manager.create_world_map(1, 0);

        manager.update(9);
        assert!(manager.find_map(1, 0).unwrap().update_calls().is_empty());

        manager.update(1);

        let map = manager.find_map(1, 0).unwrap();
        assert_eq!(map.update_calls(), &[10]);
        assert_eq!(map.delayed_update_calls(), &[10]);
    }

    #[test]
    fn update_destroys_unloadable_maps_before_delayed_update() {
        let mut manager = MapManager::new(MIN_GRID_DELAY_MS, 1);
        manager.create_map_entry(
            33,
            7,
            1,
            ManagedMapKind::Dungeon {
                has_reset_schedule: false,
            },
        );
        manager.find_map_mut(33, 7).unwrap().set_can_unload(true);
        manager.init_instance_ids(10);
        for instance_id in 1..=7 {
            manager.register_instance_id(instance_id);
        }

        manager.update(1);

        assert!(manager.find_map(33, 7).is_none());
        assert_eq!(manager.next_instance_id(), 7);
    }

    #[test]
    fn destroy_map_removes_players_then_unloads_and_removes_entry() {
        let mut manager = MapManager::default();
        manager.create_world_map(1, 0).set_player_count(2);

        assert!(manager.destroy_map(1, 0));
        assert!(manager.find_map(1, 0).is_none());
    }

    #[test]
    fn num_instances_and_players_match_dungeon_filter() {
        let mut manager = MapManager::default();
        manager.create_world_map(1, 0).set_player_count(10);
        manager
            .create_map_entry(
                33,
                7,
                1,
                ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
            )
            .set_player_count(3);

        assert_eq!(manager.num_instances(), 1);
        assert_eq!(manager.num_players_in_instances(), 3);
    }

    #[test]
    fn instance_id_allocator_reuses_lowest_freed_id() {
        let mut allocator = InstanceIdAllocator::new();
        allocator.init_instance_ids(3);
        allocator.register_instance_id(1);
        allocator.register_instance_id(2);

        assert_eq!(allocator.generate_instance_id(), Some(3));
        allocator.free_instance_id(2);
        assert_eq!(allocator.generate_instance_id(), Some(2));
    }

    #[test]
    fn scheduled_script_counter_saturates_on_decrease() {
        let mut manager = MapManager::default();

        manager.increase_scheduled_scripts_count();
        assert!(manager.is_script_scheduled());
        manager.decrease_scheduled_script_count_by(2);
        assert!(!manager.is_script_scheduled());
    }

    #[test]
    fn activated_map_updater_uses_schedule_and_wait_path() {
        let mut manager = MapManager::new(MIN_GRID_DELAY_MS, 1);
        manager.create_world_map(1, 0);
        manager.map_updater_mut().activate(2);

        manager.update(1);

        let map = manager.find_map(1, 0).unwrap();
        assert_eq!(map.update_calls(), &[1]);
        assert_eq!(manager.map_updater().scheduled_updates(), 1);
        assert_eq!(manager.map_updater().wait_calls(), 1);
        assert!(manager.map_updater().activated());

        manager.map_updater_mut().deactivate();
        assert!(!manager.map_updater().activated());
    }
}
