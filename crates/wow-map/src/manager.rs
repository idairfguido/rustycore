//! MapManager skeleton.
//!
//! C++ references:
//! - `game/Maps/MapManager.h`
//! - `game/Maps/MapManager.cpp`

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateMapEntryKind {
    World,
    Dungeon,
    BattlegroundOrArena,
    Garrison,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateMapEntryContext {
    pub map_id: u32,
    pub kind: CreateMapEntryKind,
    pub split_by_faction: bool,
    pub flex_locking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateMapDifficultyContext {
    pub difficulty_id: Difficulty,
    pub has_reset_schedule: bool,
    pub is_instance_id_bound: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateMapInstanceLockContext {
    pub instance_id: u32,
    pub difficulty_id: Difficulty,
    pub token: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateMapPlayerContext {
    pub guid_counter: u64,
    pub team_id: u32,
    pub battleground_id: u32,
    pub has_battleground: bool,
    pub player_difficulty_id: Difficulty,
    pub player_recent_instance_id: u32,
    pub group: Option<CreateMapGroupContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateMapGroupContext {
    pub difficulty_id: Difficulty,
    pub recent_instance_owner_guid_counter: u64,
    pub recent_instance_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateMapSideEffect {
    TeleportToBattlegroundEntryPoint,
    CreateInstanceLockForNewInstance {
        owner_guid_counter: u64,
        instance_id: u32,
    },
    SetInstanceLockInstanceId {
        instance_id: u32,
    },
    SetGroupRecentInstance {
        owner_guid_counter: u64,
        instance_id: u32,
    },
    SetPlayerRecentInstance {
        instance_id: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateMapDecision {
    Existing {
        key: MapKey,
        difficulty_id: Difficulty,
        side_effects: Vec<CreateMapSideEffect>,
    },
    Create {
        key: MapKey,
        difficulty_id: Difficulty,
        kind: ManagedMapKind,
        side_effects: Vec<CreateMapSideEffect>,
    },
    Reject {
        side_effects: Vec<CreateMapSideEffect>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExistingInstanceMapContext {
    pub instance_lock_token: Option<u64>,
}

#[derive(Debug)]
pub struct ManagedMap {
    map: Map<NoopTerrainGridLoader, NoopGridLifecycle>,
    kind: ManagedMapKind,
    can_unload: bool,
    player_count: u32,
    instance_lock_token: Option<u64>,
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
            instance_lock_token: None,
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

    pub const fn instance_lock_token(&self) -> Option<u64> {
        self.instance_lock_token
    }

    pub fn set_instance_lock_token(&mut self, token: Option<u64>) {
        self.instance_lock_token = token;
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

pub type SpawnGroupInitializerLikeCpp = Arc<dyn Fn(&mut ManagedMap) + Send + Sync>;

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

pub struct MapManager {
    grid_cleanup_delay_ms: u32,
    maps: BTreeMap<MapKey, ManagedMap>,
    timer: IntervalTimer,
    instance_ids: InstanceIdAllocator,
    updater: MapUpdater,
    scheduled_scripts: usize,
    spawn_group_initializer_like_cpp: Option<SpawnGroupInitializerLikeCpp>,
}

impl fmt::Debug for MapManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapManager")
            .field("grid_cleanup_delay_ms", &self.grid_cleanup_delay_ms)
            .field("maps", &self.maps)
            .field("timer", &self.timer)
            .field("instance_ids", &self.instance_ids)
            .field("updater", &self.updater)
            .field("scheduled_scripts", &self.scheduled_scripts)
            .field(
                "spawn_group_initializer_like_cpp",
                &self
                    .spawn_group_initializer_like_cpp
                    .as_ref()
                    .map(|_| "<hook>"),
            )
            .finish()
    }
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
            spawn_group_initializer_like_cpp: None,
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

    pub fn set_spawn_group_initializer_like_cpp(
        &mut self,
        initializer: impl Fn(&mut ManagedMap) + Send + Sync + 'static,
    ) {
        self.spawn_group_initializer_like_cpp = Some(Arc::new(initializer));
    }

    pub fn clear_spawn_group_initializer_like_cpp(&mut self) {
        self.spawn_group_initializer_like_cpp = None;
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
        let grid_cleanup_delay_ms = self.grid_cleanup_delay_ms;
        let spawn_group_initializer_like_cpp = self.spawn_group_initializer_like_cpp.clone();
        match self.maps.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let mut map = ManagedMap::new(
                    map_id,
                    instance_id,
                    difficulty,
                    i64::from(grid_cleanup_delay_ms),
                    kind,
                );
                if let Some(initializer) = spawn_group_initializer_like_cpp {
                    initializer(&mut map);
                }
                entry.insert(map)
            }
        }
    }

    pub fn create_map_decision_like_cpp(
        &mut self,
        entry: Option<CreateMapEntryContext>,
        player: Option<CreateMapPlayerContext>,
        map_difficulty: impl FnOnce(u32, Difficulty) -> Option<CreateMapDifficultyContext>,
        active_instance_lock: Option<CreateMapInstanceLockContext>,
        existing_instance_map: impl FnOnce(u32, u32) -> Option<ExistingInstanceMapContext>,
    ) -> CreateMapDecision {
        let Some(player) = player else {
            return CreateMapDecision::Reject {
                side_effects: Vec::new(),
            };
        };
        let Some(entry) = entry else {
            return CreateMapDecision::Reject {
                side_effects: Vec::new(),
            };
        };

        match entry.kind {
            CreateMapEntryKind::BattlegroundOrArena => {
                let instance_id = player.battleground_id;
                if instance_id == 0 {
                    return CreateMapDecision::Reject {
                        side_effects: Vec::new(),
                    };
                }

                let key = MapKey::new(entry.map_id, instance_id);
                if self.find_map(entry.map_id, instance_id).is_some() {
                    return CreateMapDecision::Existing {
                        key,
                        difficulty_id: 0,
                        side_effects: Vec::new(),
                    };
                }

                if !player.has_battleground {
                    return CreateMapDecision::Reject {
                        side_effects: vec![CreateMapSideEffect::TeleportToBattlegroundEntryPoint],
                    };
                }

                CreateMapDecision::Create {
                    key,
                    difficulty_id: 0,
                    kind: ManagedMapKind::Battleground,
                    side_effects: Vec::new(),
                }
            }
            CreateMapEntryKind::Dungeon => {
                let group = player.group;
                let mut difficulty = group
                    .map(|group| group.difficulty_id)
                    .unwrap_or(player.player_difficulty_id);
                let Some(difficulty_context) = map_difficulty(entry.map_id, difficulty) else {
                    return CreateMapDecision::Reject {
                        side_effects: Vec::new(),
                    };
                };

                let owner_guid_counter = group
                    .map(|group| group.recent_instance_owner_guid_counter)
                    .unwrap_or(player.guid_counter);
                let mut side_effects = Vec::new();
                let instance_lock = active_instance_lock;
                let mut instance_id = 0;

                if let Some(lock) = instance_lock {
                    instance_id = lock.instance_id;
                    if !entry.flex_locking {
                        difficulty = lock.difficulty_id;
                    }
                } else {
                    if !difficulty_context.has_reset_schedule {
                        instance_id = group
                            .map(|group| group.recent_instance_id)
                            .unwrap_or(player.player_recent_instance_id);
                    }

                    if instance_id == 0 {
                        let Some(generated) = self.generate_instance_id() else {
                            return CreateMapDecision::Reject {
                                side_effects: Vec::new(),
                            };
                        };
                        instance_id = generated;
                    }

                    if difficulty_context.has_reset_schedule {
                        side_effects.push(CreateMapSideEffect::CreateInstanceLockForNewInstance {
                            owner_guid_counter,
                            instance_id,
                        });
                    }
                }

                let existing = self.find_map(entry.map_id, instance_id).map(|map| {
                    ExistingInstanceMapContext {
                        instance_lock_token: map.instance_lock_token(),
                    }
                });
                let existing =
                    existing.or_else(|| existing_instance_map(entry.map_id, instance_id));

                if !difficulty_context.is_instance_id_bound
                    && let (Some(lock), Some(existing)) = (instance_lock, existing)
                    && existing.instance_lock_token != Some(lock.token)
                {
                    let Some(generated) = self.generate_instance_id() else {
                        return CreateMapDecision::Reject { side_effects };
                    };
                    instance_id = generated;
                    side_effects
                        .push(CreateMapSideEffect::SetInstanceLockInstanceId { instance_id });
                }

                let key = MapKey::new(entry.map_id, instance_id);
                if self.find_map(entry.map_id, instance_id).is_some() {
                    return CreateMapDecision::Existing {
                        key,
                        difficulty_id: difficulty,
                        side_effects,
                    };
                }

                if let Some(group) = group {
                    side_effects.push(CreateMapSideEffect::SetGroupRecentInstance {
                        owner_guid_counter: group.recent_instance_owner_guid_counter,
                        instance_id,
                    });
                } else {
                    side_effects.push(CreateMapSideEffect::SetPlayerRecentInstance { instance_id });
                }

                CreateMapDecision::Create {
                    key,
                    difficulty_id: difficulty,
                    kind: ManagedMapKind::Dungeon {
                        has_reset_schedule: difficulty_context.has_reset_schedule,
                    },
                    side_effects,
                }
            }
            CreateMapEntryKind::Garrison => CreateMapDecision::Create {
                key: MapKey::new(entry.map_id, player.guid_counter as u32),
                difficulty_id: 0,
                kind: ManagedMapKind::World,
                side_effects: Vec::new(),
            },
            CreateMapEntryKind::World => {
                let instance_id = if entry.split_by_faction {
                    player.team_id
                } else {
                    0
                };
                let key = MapKey::new(entry.map_id, instance_id);
                if self.find_map(entry.map_id, instance_id).is_some() {
                    CreateMapDecision::Existing {
                        key,
                        difficulty_id: 0,
                        side_effects: Vec::new(),
                    }
                } else {
                    CreateMapDecision::Create {
                        key,
                        difficulty_id: 0,
                        kind: ManagedMapKind::World,
                        side_effects: Vec::new(),
                    }
                }
            }
        }
    }

    pub fn find_instance_id_for_player_like_cpp(
        &self,
        entry: Option<CreateMapEntryContext>,
        player: Option<CreateMapPlayerContext>,
        map_difficulty: impl FnOnce(u32, Difficulty) -> Option<CreateMapDifficultyContext>,
        active_instance_lock: Option<CreateMapInstanceLockContext>,
        existing_instance_map: impl FnOnce(u32, u32) -> Option<ExistingInstanceMapContext>,
    ) -> u32 {
        let Some(player) = player else {
            return 0;
        };
        let Some(entry) = entry else {
            return 0;
        };

        match entry.kind {
            CreateMapEntryKind::BattlegroundOrArena => player.battleground_id,
            CreateMapEntryKind::Dungeon => {
                let group = player.group;
                let difficulty = group
                    .map(|group| group.difficulty_id)
                    .unwrap_or(player.player_difficulty_id);
                let Some(difficulty_context) = map_difficulty(entry.map_id, difficulty) else {
                    return 0;
                };

                let mut instance_id = 0;
                if let Some(lock) = active_instance_lock {
                    instance_id = lock.instance_id;
                } else if !difficulty_context.has_reset_schedule {
                    instance_id = group
                        .map(|group| group.recent_instance_id)
                        .unwrap_or(player.player_recent_instance_id);
                }

                if instance_id == 0 {
                    return 0;
                }

                let existing = self.find_map(entry.map_id, instance_id).map(|map| {
                    ExistingInstanceMapContext {
                        instance_lock_token: map.instance_lock_token(),
                    }
                });
                let existing =
                    existing.or_else(|| existing_instance_map(entry.map_id, instance_id));
                if !difficulty_context.is_instance_id_bound
                    && let (Some(lock), Some(existing)) = (active_instance_lock, existing)
                    && existing.instance_lock_token != Some(lock.token)
                {
                    return 0;
                }

                instance_id
            }
            CreateMapEntryKind::Garrison => player.guid_counter as u32,
            CreateMapEntryKind::World => {
                if entry.split_by_faction {
                    player.team_id
                } else {
                    0
                }
            }
        }
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::spawn::{SpawnGroupFlags, SpawnGroupTemplateData};

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
    fn map_manager_init_spawn_group_state_hook_runs_once_for_new_maps_only() {
        let mut manager = MapManager::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        manager.set_spawn_group_initializer_like_cpp(move |map| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            map.set_player_count(7);
        });

        manager.create_world_map(571, 0);
        manager.create_world_map(571, 0);
        manager.create_map_entry(571, 0, 0, ManagedMapKind::World);

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(manager.find_map(571, 0).unwrap().player_count(), 7);

        manager.create_map_entry(
            571,
            9,
            1,
            ManagedMapKind::Dungeon {
                has_reset_schedule: false,
            },
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        manager.clear_spawn_group_initializer_like_cpp();
        manager.create_world_map(1, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn map_manager_init_spawn_group_state_hook_can_mutate_managed_map_spawn_groups() {
        let manual = SpawnGroupTemplateData {
            group_id: 10,
            name: "manual".to_string(),
            map_id: 571,
            flags: SpawnGroupFlags::MANUAL_SPAWN,
        };
        let automatic = SpawnGroupTemplateData {
            group_id: 11,
            name: "automatic".to_string(),
            map_id: 571,
            flags: SpawnGroupFlags::NONE,
        };
        let system = SpawnGroupTemplateData {
            group_id: 12,
            name: "system".to_string(),
            map_id: 571,
            flags: SpawnGroupFlags::SYSTEM,
        };
        let groups = Arc::new(vec![manual.clone(), automatic.clone(), system.clone()]);

        let mut manager = MapManager::default();
        manager.set_spawn_group_initializer_like_cpp({
            let groups = Arc::clone(&groups);
            move |managed_map| {
                managed_map
                    .map_mut()
                    .init_spawn_group_state_like_cpp(groups.iter(), |group| {
                        group.group_id == manual.group_id
                    });
            }
        });

        manager.create_world_map(571, 0);
        let map = manager.find_map(571, 0).unwrap().map();

        assert!(map.is_spawn_group_active_like_cpp(Some(&groups[0])));
        assert!(!map.is_spawn_group_active_like_cpp(Some(&groups[1])));
        assert!(map.is_spawn_group_active_like_cpp(Some(&groups[2])));
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

    fn world_entry(map_id: u32) -> CreateMapEntryContext {
        CreateMapEntryContext {
            map_id,
            kind: CreateMapEntryKind::World,
            split_by_faction: false,
            flex_locking: false,
        }
    }

    fn dungeon_entry(map_id: u32, flex_locking: bool) -> CreateMapEntryContext {
        CreateMapEntryContext {
            map_id,
            kind: CreateMapEntryKind::Dungeon,
            split_by_faction: false,
            flex_locking,
        }
    }

    fn player() -> CreateMapPlayerContext {
        CreateMapPlayerContext {
            guid_counter: 77,
            team_id: 469,
            battleground_id: 0,
            has_battleground: false,
            player_difficulty_id: 1,
            player_recent_instance_id: 0,
            group: None,
        }
    }

    fn difficulty(
        difficulty_id: Difficulty,
        has_reset_schedule: bool,
        is_instance_id_bound: bool,
    ) -> CreateMapDifficultyContext {
        CreateMapDifficultyContext {
            difficulty_id,
            has_reset_schedule,
            is_instance_id_bound,
        }
    }

    #[test]
    fn create_map_decision_rejects_missing_player_or_map_entry_like_cpp() {
        let mut manager = MapManager::default();

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(world_entry(1)),
                None,
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Reject {
                side_effects: Vec::new(),
            }
        );
        assert_eq!(
            manager.create_map_decision_like_cpp(
                None,
                Some(player()),
                |_, _| None,
                None,
                |_, _| None
            ),
            CreateMapDecision::Reject {
                side_effects: Vec::new(),
            }
        );
    }

    #[test]
    fn create_map_decision_world_uses_zero_or_team_instance_like_cpp() {
        let mut manager = MapManager::default();
        let mut split_entry = world_entry(530);
        split_entry.split_by_faction = true;

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(world_entry(0)),
                Some(player()),
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(0, 0),
                difficulty_id: 0,
                kind: ManagedMapKind::World,
                side_effects: Vec::new(),
            }
        );
        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(split_entry),
                Some(player()),
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(530, 469),
                difficulty_id: 0,
                kind: ManagedMapKind::World,
                side_effects: Vec::new(),
            }
        );
    }

    #[test]
    fn create_map_decision_battleground_requires_instance_and_bg_pointer_like_cpp() {
        let mut manager = MapManager::default();
        let entry = CreateMapEntryContext {
            map_id: 489,
            kind: CreateMapEntryKind::BattlegroundOrArena,
            split_by_faction: false,
            flex_locking: false,
        };

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(entry),
                Some(player()),
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Reject {
                side_effects: Vec::new(),
            }
        );

        let mut bg_player = player();
        bg_player.battleground_id = 12;
        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(entry),
                Some(bg_player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Reject {
                side_effects: vec![CreateMapSideEffect::TeleportToBattlegroundEntryPoint],
            }
        );

        bg_player.has_battleground = true;
        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(entry),
                Some(bg_player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(489, 12),
                difficulty_id: 0,
                kind: ManagedMapKind::Battleground,
                side_effects: Vec::new(),
            }
        );
    }

    #[test]
    fn create_map_decision_dungeon_uses_active_lock_and_resets_difficulty_like_cpp() {
        let mut manager = MapManager::default();
        manager
            .create_map_entry(
                33,
                42,
                2,
                ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
            )
            .set_instance_lock_token(Some(9));

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(dungeon_entry(33, false)),
                Some(player()),
                |_, requested| Some(difficulty(requested, true, true)),
                Some(CreateMapInstanceLockContext {
                    instance_id: 42,
                    difficulty_id: 2,
                    token: 9,
                }),
                |_, _| None,
            ),
            CreateMapDecision::Existing {
                key: MapKey::new(33, 42),
                difficulty_id: 2,
                side_effects: Vec::new(),
            }
        );
    }

    #[test]
    fn create_map_decision_normal_dungeon_reuses_recent_instance_like_cpp() {
        let mut manager = MapManager::default();
        let mut player = player();
        player.player_recent_instance_id = 7;

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(dungeon_entry(33, false)),
                Some(player),
                |_, requested| Some(difficulty(requested, false, true)),
                None,
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(33, 7),
                difficulty_id: 1,
                kind: ManagedMapKind::Dungeon {
                    has_reset_schedule: false,
                },
                side_effects: vec![CreateMapSideEffect::SetPlayerRecentInstance { instance_id: 7 }],
            }
        );
    }

    #[test]
    fn create_map_decision_dungeon_generates_instance_and_lock_side_effect_like_cpp() {
        let mut manager = MapManager::default();
        manager.init_instance_ids(3);
        manager.register_instance_id(1);

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(dungeon_entry(631, false)),
                Some(player()),
                |_, requested| Some(difficulty(requested, true, true)),
                None,
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(631, 2),
                difficulty_id: 1,
                kind: ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
                side_effects: vec![
                    CreateMapSideEffect::CreateInstanceLockForNewInstance {
                        owner_guid_counter: 77,
                        instance_id: 2,
                    },
                    CreateMapSideEffect::SetPlayerRecentInstance { instance_id: 2 },
                ],
            }
        );
    }

    #[test]
    fn create_map_decision_flex_lock_conflict_regenerates_instance_like_cpp() {
        let mut manager = MapManager::default();
        manager.init_instance_ids(50);
        for instance_id in 1..=42 {
            manager.register_instance_id(instance_id);
        }
        manager
            .create_map_entry(
                631,
                42,
                3,
                ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
            )
            .set_instance_lock_token(Some(100));

        assert_eq!(
            manager.create_map_decision_like_cpp(
                Some(dungeon_entry(631, true)),
                Some(player()),
                |_, requested| Some(difficulty(requested, true, false)),
                Some(CreateMapInstanceLockContext {
                    instance_id: 42,
                    difficulty_id: 3,
                    token: 200,
                }),
                |_, _| None,
            ),
            CreateMapDecision::Create {
                key: MapKey::new(631, 43),
                difficulty_id: 1,
                kind: ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
                side_effects: vec![
                    CreateMapSideEffect::SetInstanceLockInstanceId { instance_id: 43 },
                    CreateMapSideEffect::SetPlayerRecentInstance { instance_id: 43 },
                ],
            }
        );
    }

    #[test]
    fn find_instance_id_for_player_matches_cpp_world_bg_and_garrison_branches() {
        let manager = MapManager::default();
        let mut split = world_entry(609);
        split.split_by_faction = true;
        let bg = CreateMapEntryContext {
            map_id: 489,
            kind: CreateMapEntryKind::BattlegroundOrArena,
            split_by_faction: false,
            flex_locking: false,
        };
        let garrison = CreateMapEntryContext {
            map_id: 1152,
            kind: CreateMapEntryKind::Garrison,
            split_by_faction: false,
            flex_locking: false,
        };
        let mut player = player();
        player.team_id = 1;
        player.battleground_id = 12;

        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(world_entry(0)),
                Some(player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            0
        );
        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(split),
                Some(player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            1
        );
        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(bg),
                Some(player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            12
        );
        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(garrison),
                Some(player),
                |_, _| None,
                None,
                |_, _| None,
            ),
            77
        );
    }

    #[test]
    fn find_instance_id_for_player_matches_cpp_dungeon_lock_and_recent_rules() {
        let mut manager = MapManager::default();
        manager
            .create_map_entry(
                631,
                42,
                3,
                ManagedMapKind::Dungeon {
                    has_reset_schedule: true,
                },
            )
            .set_instance_lock_token(Some(100));
        let mut player = player();
        player.player_recent_instance_id = 7;

        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(dungeon_entry(33, false)),
                Some(player),
                |_, requested| Some(difficulty(requested, false, true)),
                None,
                |_, _| None,
            ),
            7
        );
        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(dungeon_entry(631, true)),
                Some(player),
                |_, requested| Some(difficulty(requested, true, false)),
                Some(CreateMapInstanceLockContext {
                    instance_id: 42,
                    difficulty_id: 3,
                    token: 200,
                }),
                |_, _| None,
            ),
            0
        );
        assert_eq!(
            manager.find_instance_id_for_player_like_cpp(
                Some(dungeon_entry(631, true)),
                Some(player),
                |_, requested| Some(difficulty(requested, true, false)),
                Some(CreateMapInstanceLockContext {
                    instance_id: 42,
                    difficulty_id: 3,
                    token: 100,
                }),
                |_, _| None,
            ),
            42
        );
    }
}
