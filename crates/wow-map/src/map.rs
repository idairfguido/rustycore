//! Map grid lifecycle skeleton.
//!
//! C++ references:
//! - `game/Maps/Map.h`
//! - `game/Maps/Map.cpp`

use std::collections::{HashMap, HashSet};

use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::cell::{Cell, GridObjectGuids, WorldObjectGuids, calculate_cell_area_like_cpp};
use crate::coords::{
    CellCoord, GridCoord, MAX_NUMBER_OF_CELLS, MAX_NUMBER_OF_GRIDS, SIZE_OF_GRID_CELL,
    TOTAL_NUMBER_OF_CELLS_PER_MAP, compute_cell_coord, is_valid_map_coord_2d,
};
use crate::grid::{GridStateKind, MapGridHost, NGrid, update_grid_state};
use crate::grid_unload::{
    GridObjectKind, GridUnloadAction, GridUnloadApplyOutcome, GridUnloadEntityStore,
    apply_grid_unload_actions,
};
use crate::object_grid_loader::{GridSpawnLoadFilter, ObjectGridLoader};
use crate::personal_phase::{MultiPersonalPhaseTracker, PhaseShift};
use crate::pool::{
    PoolInitForMapPlanLikeCpp, PoolMemberKindLikeCpp, PoolMgrLikeCpp, PoolMgrPlanErrorLikeCpp,
    PoolObjectLikeCpp, PoolSpawnObjectActionLikeCpp, PoolSpawnObjectPlanLikeCpp,
    PoolTypedSpawnPlanLikeCpp,
};
use crate::spawn::{
    AddRespawnInfoOutcomeLikeCpp, CheckRespawnOutcomeLikeCpp,
    CheckRespawnSpawnGroupGuardOutcomeLikeCpp, Difficulty, LinkedRespawnStoreLikeCpp,
    ProcessRespawnActionLikeCpp, RespawnInfoLikeCpp, RespawnStoreLikeCpp,
    SpawnGridLoadStateLikeCpp, SpawnGroupActiveChange, SpawnGroupFlags, SpawnGroupRuntimeState,
    SpawnGroupTemplateData, SpawnId, SpawnObjectType, SpawnStore,
};
use wow_core::{ObjectGuid, ObjectGuidGenerator, Position, guid::HighGuid};
use wow_entities::{
    AccessorObjectKind, AreaTrigger, CombatBeginContextLikeCpp, CombatSubsystem, Conversation,
    Corpse, Creature, DynamicObject, DynamicObjectType, GameObject, INVALID_HEIGHT,
    LineOfSightQuery, MAX_VISIBILITY_DISTANCE, MapBindingError, MapObjectRecord,
    ObjectAccessorError, ObjectAccessorMapSource, ObjectNotifyFlags, Player, SceneObject, Unit,
    UnitSharedVisionSetWorldObjectRequestLikeCpp, WorldObject, WorldObjectEnvironment,
    WorldObjectHeightQuery,
};

const GRID_SLOT_COUNT: usize = (MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_GRIDS) as usize;

#[derive(Clone, Copy)]
struct CombatUnitSnapshotLikeCpp<'a> {
    guid: ObjectGuid,
    unit: &'a Unit,
    game_master_player: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveObjectKind {
    Player,
    NonPlayer,
}

impl From<AccessorObjectKind> for ActiveObjectKind {
    fn from(kind: AccessorObjectKind) -> Self {
        match kind {
            AccessorObjectKind::Player => Self::Player,
            _ => Self::NonPlayer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapGuidSequenceErrorLikeCpp {
    /// Mirrors the C++ `static_assert` in `Map::GenerateLowGuid<high>` /
    /// `Map::GetMaxLowGuid<high>` (`Map.h:514-526`) without panicking for
    /// runtime-selected Rust `HighGuid` values.
    UnsupportedSequenceSource { high: HighGuid },
}

struct MapGuidSequenceGeneratorLikeCpp {
    generator: ObjectGuidGenerator,
}

impl std::fmt::Debug for MapGuidSequenceGeneratorLikeCpp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapGuidSequenceGeneratorLikeCpp")
            .field("high", &self.generator.high_guid())
            .field("next_after_max_used", &self.generator.next_after_max_used())
            .finish()
    }
}

impl MapGuidSequenceGeneratorLikeCpp {
    fn new(high: HighGuid) -> Self {
        Self {
            generator: ObjectGuidGenerator::new(high, 1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicRespawnScalingConfig {
    pub creature_rate: f64,
    pub creature_minimum_secs: u32,
    pub gameobject_rate: f64,
    pub gameobject_minimum_secs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicRespawnScalingNoopReason {
    DynamicModeDisabled,
    UnsupportedMode,
    BattlegroundOrArena,
    UnsupportedSpawnType,
    MissingSpawnMetadata,
    MissingDynamicSpawnRateFlag,
    MissingZonePlayerCount,
    ZeroZonePlayers,
    AdjustFactorAtLeastOne,
    DelayAtOrBelowMinimum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnedPoolDataErrorLikeCpp {
    /// C++ `SpawnedPoolData::IsSpawnedObject(SpawnObjectType, ...)` aborts for
    /// non Creature/GameObject types (`PoolMgr.cpp:66-77`). Rust returns a typed
    /// error at the seam instead of treating AreaTrigger as pooled/spawned.
    UnsupportedSpawnObjectType(SpawnObjectType),
}

/// Map-owned parity seam for C++ `SpawnedPoolData` (`PoolMgr.h:51-83`).
///
/// This is only the map-local state shape and helpers used by C++
/// `Map::_poolData` / `Map::GetPoolData()`. It does not implement real
/// `PoolMgr::SpawnPool`, `DespawnPool`, RNG/chance, entity creation,
/// AddToMap/RemoveFromMap, DB persistence/delete, or grid/session fanout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnedPoolDataLikeCpp {
    spawned_creatures: HashSet<SpawnId>,
    spawned_gameobjects: HashSet<SpawnId>,
    spawned_pools: HashMap<u32, u32>,
}

impl SpawnedPoolDataLikeCpp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_spawned_objects_like_cpp(&self, pool_id: u32) -> u32 {
        self.spawned_pools.get(&pool_id).copied().unwrap_or(0)
    }

    pub fn is_spawned_creature_like_cpp(&self, spawn_id: SpawnId) -> bool {
        self.spawned_creatures.contains(&spawn_id)
    }

    pub fn is_spawned_gameobject_like_cpp(&self, spawn_id: SpawnId) -> bool {
        self.spawned_gameobjects.contains(&spawn_id)
    }

    pub fn is_spawned_pool_like_cpp(&self, sub_pool_id: u32) -> bool {
        self.spawned_pools.contains_key(&sub_pool_id)
    }

    pub fn is_spawned_object_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Result<bool, SpawnedPoolDataErrorLikeCpp> {
        match object_type {
            SpawnObjectType::Creature => Ok(self.is_spawned_creature_like_cpp(spawn_id)),
            SpawnObjectType::GameObject => Ok(self.is_spawned_gameobject_like_cpp(spawn_id)),
            SpawnObjectType::AreaTrigger => Err(
                SpawnedPoolDataErrorLikeCpp::UnsupportedSpawnObjectType(object_type),
            ),
        }
    }

    pub fn add_spawn_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        pool_id: u32,
    ) -> Result<(), SpawnedPoolDataErrorLikeCpp> {
        match object_type {
            SpawnObjectType::Creature => {
                self.spawned_creatures.insert(spawn_id);
                *self.spawned_pools.entry(pool_id).or_insert(0) += 1;
                Ok(())
            }
            SpawnObjectType::GameObject => {
                self.spawned_gameobjects.insert(spawn_id);
                *self.spawned_pools.entry(pool_id).or_insert(0) += 1;
                Ok(())
            }
            SpawnObjectType::AreaTrigger => Err(
                SpawnedPoolDataErrorLikeCpp::UnsupportedSpawnObjectType(object_type),
            ),
        }
    }

    pub fn remove_spawn_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        pool_id: u32,
    ) -> Result<(), SpawnedPoolDataErrorLikeCpp> {
        match object_type {
            SpawnObjectType::Creature => {
                self.spawned_creatures.remove(&spawn_id);
                Self::decrement_pool_counter_like_cpp(&mut self.spawned_pools, pool_id);
                Ok(())
            }
            SpawnObjectType::GameObject => {
                self.spawned_gameobjects.remove(&spawn_id);
                Self::decrement_pool_counter_like_cpp(&mut self.spawned_pools, pool_id);
                Ok(())
            }
            SpawnObjectType::AreaTrigger => Err(
                SpawnedPoolDataErrorLikeCpp::UnsupportedSpawnObjectType(object_type),
            ),
        }
    }

    pub fn add_pool_spawn_like_cpp(&mut self, sub_pool_id: u32, pool_id: u32) {
        self.spawned_pools.insert(sub_pool_id, 0);
        *self.spawned_pools.entry(pool_id).or_insert(0) += 1;
    }

    pub fn remove_pool_spawn_like_cpp(&mut self, sub_pool_id: u32, pool_id: u32) {
        self.spawned_pools.remove(&sub_pool_id);
        Self::decrement_pool_counter_like_cpp(&mut self.spawned_pools, pool_id);
    }

    pub fn spawned_objects_like_cpp(&self) -> Vec<(SpawnObjectType, SpawnId)> {
        let mut spawned = self
            .spawned_creatures
            .iter()
            .copied()
            .map(|spawn_id| (SpawnObjectType::Creature, spawn_id))
            .chain(
                self.spawned_gameobjects
                    .iter()
                    .copied()
                    .map(|spawn_id| (SpawnObjectType::GameObject, spawn_id)),
            )
            .collect::<Vec<_>>();
        spawned.sort_unstable();
        spawned
    }

    fn decrement_pool_counter_like_cpp(spawned_pools: &mut HashMap<u32, u32>, pool_id: u32) {
        let counter = spawned_pools.entry(pool_id).or_insert(0);
        if *counter > 0 {
            *counter -= 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicRespawnScalingOutcome {
    pub delay_secs: u32,
    pub noop_reason: Option<DynamicRespawnScalingNoopReason>,
}

impl DynamicRespawnScalingOutcome {
    pub const fn unchanged(delay_secs: u32, reason: DynamicRespawnScalingNoopReason) -> Self {
        Self {
            delay_secs,
            noop_reason: Some(reason),
        }
    }

    pub const fn scaled(delay_secs: u32) -> Self {
        Self {
            delay_secs,
            noop_reason: None,
        }
    }

    pub const fn was_scaled(self) -> bool {
        self.noop_reason.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicRespawnScalingContext {
    pub mode: u32,
    pub spawn_type: Option<SpawnObjectType>,
    pub spawn_metadata_present: bool,
    pub spawn_group_flags: Option<SpawnGroupFlags>,
    pub is_battleground_or_arena: bool,
    pub zone_player_count: Option<u32>,
    pub config: DynamicRespawnScalingConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnGroupConditionActionLikeCpp {
    Noop,
    Spawn { ignore_respawn: bool, force: bool },
    Despawn { delete_respawn_times: bool },
    SetInactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddObjectToRemoveListOutcomeLikeCpp {
    pub guid: ObjectGuid,
    pub queued: bool,
    pub duplicate: bool,
    pub missing_or_stale: bool,
    pub unsupported_kind: Option<AccessorObjectKind>,
    pub cleanup_before_delete_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddObjectToSwitchListStatusLikeCpp {
    Queued,
    CancelledOppositeToggle,
    DuplicateSameDirectionAbort,
    MissingOrStale,
    IgnoredNonUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddObjectToSwitchListOutcomeLikeCpp {
    pub guid: ObjectGuid,
    pub on: bool,
    pub status: AddObjectToSwitchListStatusLikeCpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetWorldObjectStatusLikeCpp {
    MissingOrStale,
    NotInWorld,
    Delegated(AddObjectToSwitchListStatusLikeCpp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetWorldObjectOutcomeLikeCpp {
    pub guid: ObjectGuid,
    pub on: bool,
    pub status: SetWorldObjectStatusLikeCpp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSetViewpointStatusLikeCpp {
    Applied,
    Removed,
    MissingPlayer,
    MissingTarget,
    TargetNotUnit,
    TargetNotDynamicObject,
    TargetIsVehicleBase,
    AlreadyHasViewpoint,
    ViewpointMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSetViewpointOutcomeLikeCpp {
    pub player_guid: ObjectGuid,
    pub target_guid: ObjectGuid,
    pub apply: bool,
    pub status: PlayerSetViewpointStatusLikeCpp,
    pub set_world_object: Option<SetWorldObjectOutcomeLikeCpp>,
    pub update_visibility_requested: bool,
    pub set_seer_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicObjectCasterViewpointStatusLikeCpp {
    CasterPlayerResolved,
    MissingDynamicObject,
    MissingCaster,
    CasterNotPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicObjectCasterViewpointOutcomeLikeCpp {
    pub player_guid: ObjectGuid,
    pub dynamic_object_guid: ObjectGuid,
    pub apply: bool,
    pub status: DynamicObjectCasterViewpointStatusLikeCpp,
    pub player_set_viewpoint: PlayerSetViewpointOutcomeLikeCpp,
    pub dynamic_object_viewpoint_toggled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicObjectUpdateStatusLikeCpp {
    Updated,
    ExpiredRemoveQueued,
    MissingDynamicObject,
    NotDynamicObject,
    NotInWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicObjectUpdateOutcomeLikeCpp {
    pub dynamic_object_guid: ObjectGuid,
    pub elapsed_ms: u32,
    pub status: DynamicObjectUpdateStatusLikeCpp,
    pub duration_before_ms: Option<i32>,
    pub duration_after_ms: Option<i32>,
    pub aura_update_owner_calls_before: Option<u32>,
    pub aura_update_owner_calls_after: Option<u32>,
    pub script_update_would_run: bool,
    pub remove_list: Option<AddObjectToRemoveListOutcomeLikeCpp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FarsightDynamicObjectCreateStatusLikeCpp {
    Created,
    MissingCasterPlayer,
    CasterNotInWorld,
    CasterWrongMap,
    InvalidDestination,
    MapIdNotRepresentableInGuid,
    SpellIdNotRepresentable,
    CastTimeNotRepresentable,
    GuidSequenceError(MapGuidSequenceErrorLikeCpp),
    DynamicObjectRecordError(ObjectAccessorError),
    AddToMapError,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FarsightDynamicObjectCreateOutcomeLikeCpp {
    pub status: FarsightDynamicObjectCreateStatusLikeCpp,
    pub caster_player_guid: ObjectGuid,
    pub dynamic_object_guid: Option<ObjectGuid>,
    pub low_guid: Option<i64>,
    pub add_to_map: Option<AddToMapOutcome>,
    pub caster_viewpoint: Option<DynamicObjectCasterViewpointOutcomeLikeCpp>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RemoveAllObjectsInRemoveListOutcomeLikeCpp {
    pub switch_processed: usize,
    pub switch_executed: usize,
    pub switch_missing_or_stale: usize,
    pub switch_unsupported_kinds: usize,
    pub switch_permanent_world_objects: usize,
    pub switch_invalid_or_unloaded_grid: usize,
    pub processed: usize,
    pub removed: usize,
    pub missing_or_stale: usize,
    pub remove_errors: usize,
    pub unsupported_kinds: usize,
    pub creature_second_cleanup_count: usize,
    pub dynamic_object_remove_aura_cleanup_count: usize,
    pub dynamic_object_unbound_caster_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SwitchGridContainersOutcomeLikeCpp {
    executed: bool,
    missing_or_stale: bool,
    unsupported_kind: bool,
    permanent_world_object: bool,
    invalid_or_unloaded_grid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DespawnAllBySpawnIdOutcomeLikeCpp {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    /// Number of live objects snapshotted from the by-spawn store and queued via
    /// `AddObjectToRemoveList`; physical deletion is deferred to
    /// `remove_all_objects_in_remove_list_like_cpp`.
    pub queued: usize,
    /// Legacy compatibility counter retained for callers from the pre-#419 seam.
    /// It is no longer incremented by `despawn_all_by_spawn_id_like_cpp`; use
    /// `queued` for C++ `Map::DespawnAll` parity and drain the map remove-list for
    /// physical removal.
    pub removed: usize,
    pub duplicates: usize,
    pub stale_index_entries: usize,
    pub remove_errors: usize,
    pub unsupported_live_despawn_type: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpawnGroupDespawnOutcomeLikeCpp {
    pub group_id: u32,
    pub blocked_missing_group: usize,
    pub blocked_system_group: usize,
    pub metadata_entries: usize,
    pub respawn_timers_removed: usize,
    pub respawn_timers_missing: usize,
    pub respawn_timer_unsupported_types: usize,
    pub objects_removed: usize,
    pub stale_index_entries: usize,
    pub remove_errors: usize,
    pub unsupported_live_despawn_types: usize,
    pub applied_inactive_change: Option<SpawnGroupActiveChange>,
}

impl SpawnGroupDespawnOutcomeLikeCpp {
    pub const fn blocked_missing_group(group_id: u32) -> Self {
        Self {
            group_id,
            blocked_missing_group: 1,
            blocked_system_group: 0,
            metadata_entries: 0,
            respawn_timers_removed: 0,
            respawn_timers_missing: 0,
            respawn_timer_unsupported_types: 0,
            objects_removed: 0,
            stale_index_entries: 0,
            remove_errors: 0,
            unsupported_live_despawn_types: 0,
            applied_inactive_change: None,
        }
    }

    pub const fn blocked_system_group(group_id: u32) -> Self {
        Self {
            group_id,
            blocked_missing_group: 0,
            blocked_system_group: 1,
            metadata_entries: 0,
            respawn_timers_removed: 0,
            respawn_timers_missing: 0,
            respawn_timer_unsupported_types: 0,
            objects_removed: 0,
            stale_index_entries: 0,
            remove_errors: 0,
            unsupported_live_despawn_types: 0,
            applied_inactive_change: None,
        }
    }

    pub const fn executed(group_id: u32) -> Self {
        Self {
            group_id,
            blocked_missing_group: 0,
            blocked_system_group: 0,
            metadata_entries: 0,
            respawn_timers_removed: 0,
            respawn_timers_missing: 0,
            respawn_timer_unsupported_types: 0,
            objects_removed: 0,
            stale_index_entries: 0,
            remove_errors: 0,
            unsupported_live_despawn_types: 0,
            applied_inactive_change: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnGroupSpawnLoadPlanLikeCpp {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSpawnActionLoadPlanLikeCpp {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    pub respawn: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SpawnGroupSpawnOutcomeLikeCpp {
    pub group_id: u32,
    pub blocked_missing_group: usize,
    pub blocked_system_group: usize,
    pub metadata_entries: usize,
    pub stale_index_entries: usize,
    pub respawn_timers_removed: usize,
    pub respawn_timers_missing: usize,
    pub skipped_respawn_timer_active: usize,
    pub skipped_live_object_active: usize,
    pub skipped_difficulty_mismatch: usize,
    pub skipped_unloaded_grid: usize,
    pub blocked_loaded_grid_creature_loads: usize,
    pub blocked_loaded_grid_gameobject_loads: usize,
    pub unsupported_spawn_types: usize,
    pub load_plans: Vec<SpawnGroupSpawnLoadPlanLikeCpp>,
    pub applied_active_change: Option<SpawnGroupActiveChange>,
}

impl SpawnGroupSpawnOutcomeLikeCpp {
    pub fn blocked_missing_group(group_id: u32) -> Self {
        Self {
            group_id,
            blocked_missing_group: 1,
            ..Self::default()
        }
    }

    pub fn blocked_system_group(group_id: u32) -> Self {
        Self {
            group_id,
            blocked_system_group: 1,
            ..Self::default()
        }
    }

    pub fn executed(group_id: u32) -> Self {
        Self {
            group_id,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnGroupConditionUpdateOutcomeLikeCpp {
    pub group_id: u32,
    pub action: SpawnGroupConditionActionLikeCpp,
    pub applied_change: Option<SpawnGroupActiveChange>,
    pub despawn_outcome: Option<SpawnGroupDespawnOutcomeLikeCpp>,
    pub spawn_outcome: Option<SpawnGroupSpawnOutcomeLikeCpp>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedGridRespawnRecordsLikeCpp {
    pub pre_add_records: Vec<MapObjectRecord>,
    pub primary_record: MapObjectRecord,
}

impl LoadedGridRespawnRecordsLikeCpp {
    pub fn primary_only(primary_record: MapObjectRecord) -> Self {
        Self {
            pre_add_records: Vec::new(),
            primary_record,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ProcessRespawnsSafeSideEffectsSummaryLikeCpp {
    pub deleted_inactive_spawn_group: usize,
    pub deleted_live_object_blocker: usize,
    pub rescheduled_linked_respawns: Vec<RespawnInfoLikeCpp>,
    pub processed_pool_timers: usize,
    /// C++ `DoRespawn` removes the timer before calling into `DoRespawn`; when
    /// the target grid is unloaded, `DoRespawn` returns immediately and grid
    /// load can create the object later because no respawn timer remains.
    pub processed_unloaded_grid_respawns: usize,
    pub pool_update_plans: Vec<PoolTypedSpawnPlanLikeCpp>,
    pub pool_objects_removed: usize,
    pub pool_respawn_timers_removed: usize,
    pub pool_respawn_timers_missing: usize,
    pub pool_stale_index_entries: usize,
    pub pool_remove_errors: usize,
    pub pool_spawn_actions_skipped_unloaded_grid: usize,
    pub pool_spawn_actions_blocked_loaded_grid: usize,
    pub pool_spawn_action_load_plans: Vec<PoolSpawnActionLoadPlanLikeCpp>,
    pub pool_spawn_actions_missing_spawn_data: usize,
    pub pool_unsupported_action_kind: usize,
    pub blocked_pool_plan_errors: Vec<PoolMgrPlanErrorLikeCpp>,
    pub blocked_missing_spawn_data: usize,
    /// Loaded-grid non-pooled `DoRespawn` timers whose caller-supplied typed
    /// `MapObjectRecord` was successfully loaded and inserted through
    /// `AddToMap`. This is only the map-owned execution seam; DB/template
    /// resolution stays with the caller-provided loader.
    pub executed_loaded_grid_respawns: usize,
    /// Loaded-grid non-pooled `DoRespawn` timers that stayed queued because the
    /// explicit caller loader did not return a typed DB-backed record.
    pub blocked_loaded_grid_respawn_loads: usize,
    /// Loaded-grid non-pooled `DoRespawn` timers whose loader returned a record,
    /// after which C++ has already popped/erased the timer before `AddToMap`; the
    /// timer therefore stays removed even when Rust `AddToMap` rejects it.
    pub blocked_loaded_grid_respawn_add_to_map: usize,
    /// Legacy compatibility counter for the pre-#390 seam where any pooled timer
    /// blocked `ProcessRespawns`. New pooled-timer planner errors are reported in
    /// `blocked_pool_plan_errors`; successful pooled timers increment
    /// `processed_pool_timers` and remove the map-owned respawn timer.
    pub blocked_pool_runtime: usize,
    pub blocked_do_respawn_runtime: usize,
    pub blocked_linked_respawn_non_future: usize,
    pub blocked_unsupported_spawn_type: usize,
}

pub type ProcessRespawnsDeleteOnlySummaryLikeCpp = ProcessRespawnsSafeSideEffectsSummaryLikeCpp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRespawnLiveObjectGuardOutcomeLikeCpp {
    Allowed,
    AliveCreatureBlocksRespawn,
    GameObjectBlocksRespawn,
    MissingSpawnData,
    UnsupportedSpawnType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRespawnLinkedRespawnGuardOutcomeLikeCpp {
    Allowed,
    LinkedInfinite,
    LinkedSelfNeverRespawn,
    LinkedDelayed,
    UnsupportedSpawnType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRespawnCompositeOutcomeLikeCpp {
    Allowed,
    InactiveSpawnGroupDeletedTimer,
    AliveCreatureBlocksRespawn,
    GameObjectBlocksRespawn,
    LinkedInfinite,
    LinkedSelfNeverRespawn,
    LinkedDelayed,
    MissingSpawnData,
    UnsupportedSpawnType,
}

const WEEK_SECS_LIKE_CPP: i64 = 7 * 24 * 60 * 60;

impl SpawnGroupConditionActionLikeCpp {
    pub const fn spawn_group_spawn_default() -> Self {
        Self::Spawn {
            ignore_respawn: false,
            force: false,
        }
    }

    pub const fn condition_failure_despawn() -> Self {
        Self::Despawn {
            delete_respawn_times: true,
        }
    }
}

/// Rust equivalent of C++ `Map::ApplyDynamicModeRespawnScaling`.
///
/// C++ anchors:
/// - `GameObject.cpp:1665-1672` calls this before persisting GO respawn time.
/// - `Map.cpp:2242-2284` contains the dynamic respawn guards and formula.
/// - `Map.h:657-660` declares the map helper.
///
/// This helper is pure because RustyCore does not yet own the canonical map
/// spawn-metadata and zone-player-count stores needed by a `Map` method. Future
/// GameObject runtime wiring must pass canonical metadata/counts into this
/// function; this function must not read or mutate session-local fallback state.
pub fn apply_dynamic_mode_respawn_scaling_like_cpp(
    respawn_delay_secs: u32,
    context: DynamicRespawnScalingContext,
) -> DynamicRespawnScalingOutcome {
    if context.mode == 0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::DynamicModeDisabled,
        );
    }

    if context.mode != 1 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedMode,
        );
    }

    if context.is_battleground_or_arena {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::BattlegroundOrArena,
        );
    }

    let Some(spawn_type) = context.spawn_type else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );
    };

    if !matches!(
        spawn_type,
        SpawnObjectType::Creature | SpawnObjectType::GameObject
    ) {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );
    }

    if !context.spawn_metadata_present {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    }

    let Some(spawn_group_flags) = context.spawn_group_flags else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    };

    if !spawn_group_flags.contains(SpawnGroupFlags::DYNAMIC_SPAWN_RATE) {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingDynamicSpawnRateFlag,
        );
    }

    let Some(player_count) = context.zone_player_count else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingZonePlayerCount,
        );
    };

    if player_count == 0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::ZeroZonePlayers,
        );
    }

    let (rate, time_minimum) = match spawn_type {
        SpawnObjectType::Creature => (
            context.config.creature_rate,
            context.config.creature_minimum_secs,
        ),
        SpawnObjectType::GameObject => (
            context.config.gameobject_rate,
            context.config.gameobject_minimum_secs,
        ),
        SpawnObjectType::AreaTrigger => {
            return DynamicRespawnScalingOutcome::unchanged(
                respawn_delay_secs,
                DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
            );
        }
    };

    let adjust_factor = rate / f64::from(player_count);
    if adjust_factor >= 1.0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::AdjustFactorAtLeastOne,
        );
    }

    if respawn_delay_secs <= time_minimum {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::DelayAtOrBelowMinimum,
        );
    }

    let scaled = (f64::from(respawn_delay_secs) * adjust_factor).ceil() as u32;
    DynamicRespawnScalingOutcome::scaled(scaled.max(time_minimum))
}

pub trait TerrainGridLoader {
    fn load_map_and_vmap(&mut self, grid_x: u32, grid_y: u32);
    fn unload_map(&mut self, grid_x: u32, grid_y: u32);
}

/// Terrain/dynamic-tree hook used by `Map` when it acts as a
/// `WorldObjectEnvironment` for `WorldObject` helpers.
///
/// This is the explicit ownership seam for C++ `Map::isInLineOfSight`,
/// `Map::GetHeight`, and `Map::GetGameObjectFloor`. Implementations may be a
/// noop while real terrain/vmap/dynamic-tree runtime is not ported, but callers
/// must still flow through `WorldObject -> WorldObjectEnvironment -> Map -> terrain`.
pub trait MapWorldObjectEnvironment {
    fn line_of_sight(&self, query: LineOfSightQuery<'_>) -> bool;

    fn map_height(
        &self,
        object: &WorldObject,
        x: f32,
        y: f32,
        z: f32,
        query: WorldObjectHeightQuery,
    ) -> f32;

    fn floor_z(&self, object: &WorldObject, position: Position, max_search_dist: f32) -> f32;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopTerrainGridLoader;

impl TerrainGridLoader for NoopTerrainGridLoader {
    fn load_map_and_vmap(&mut self, _grid_x: u32, _grid_y: u32) {}
    fn unload_map(&mut self, _grid_x: u32, _grid_y: u32) {}
}

impl MapWorldObjectEnvironment for NoopTerrainGridLoader {
    fn line_of_sight(&self, _query: LineOfSightQuery<'_>) -> bool {
        true
    }

    fn map_height(
        &self,
        _object: &WorldObject,
        _x: f32,
        _y: f32,
        _z: f32,
        _query: WorldObjectHeightQuery,
    ) -> f32 {
        INVALID_HEIGHT
    }

    fn floor_z(&self, _object: &WorldObject, _position: Position, _max_search_dist: f32) -> f32 {
        INVALID_HEIGHT
    }
}

pub trait GridLifecycle {
    fn load_grid_objects(&mut self, grid: &mut NGrid, cell: &Cell);
    fn stop_grid_objects(&mut self, grid: &NGrid);
    fn evacuate_grid(&mut self, grid: &mut NGrid);
    fn clean_grid(&mut self, grid: &mut NGrid);
    fn unload_grid_objects(&mut self, grid: &mut NGrid);
    fn take_unload_actions_like_cpp(&mut self) -> Vec<GridUnloadAction> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopGridLifecycle;

impl GridLifecycle for NoopGridLifecycle {
    fn load_grid_objects(&mut self, _grid: &mut NGrid, _cell: &Cell) {}
    fn stop_grid_objects(&mut self, _grid: &NGrid) {}
    fn evacuate_grid(&mut self, _grid: &mut NGrid) {}
    fn clean_grid(&mut self, _grid: &mut NGrid) {}
    fn unload_grid_objects(&mut self, _grid: &mut NGrid) {}
}

#[derive(Debug)]
pub struct Map<Terrain = NoopTerrainGridLoader, Lifecycle = NoopGridLifecycle> {
    map_id: u32,
    instance_id: u32,
    spawn_mode: Difficulty,
    grid_expiry_ms: i64,
    grid_unload: bool,
    visible_distance: f32,
    grids: Vec<Option<Box<NGrid>>>,
    terrain: Terrain,
    lifecycle: Lifecycle,
    active_cells: HashSet<CellCoord>,
    personal_phase_tracker: MultiPersonalPhaseTracker,
    spawn_group_state: SpawnGroupRuntimeState,
    respawn_store: RespawnStoreLikeCpp,
    pool_data: SpawnedPoolDataLikeCpp,
    grid_state_unloaded: bool,
    /// Map-local typed by-spawn-id live-object stores, matching C++
    /// `_creatureBySpawnIdStore`, `_gameobjectBySpawnIdStore`, and
    /// `_areaTriggerBySpawnIdStore` beside `_objectsStore` (`Map.h:418-430`,
    /// private fields at `Map.h:793-796`).
    ///
    /// Rust keeps `map_objects` as the source-of-truth object store. These
    /// indexes are derived only from `insert_map_object_record`/`remove_map_object`
    /// and store GUID sets to preserve Trinity's unordered-multimap-like
    /// cardinality without making pointers canonical state. Spawn id zero is
    /// omitted, matching C++ `if (_spawnId)` / `IsStaticSpawn()`.
    ///
    /// AreaTrigger runtime side effects outside the object/spawn-id store
    /// (`ZoneScript`, caster unregister, AI removal, unit enter/exit, visibility,
    /// movement/transport, full entity-specific AddToWorld/RemoveFromWorld) remain
    /// outside this slice.
    creatures_by_spawn_id: HashMap<SpawnId, HashSet<ObjectGuid>>,
    gameobjects_by_spawn_id: HashMap<SpawnId, HashSet<ObjectGuid>>,
    area_triggers_by_spawn_id: HashMap<SpawnId, HashSet<ObjectGuid>>,
    map_objects: HashMap<ObjectGuid, MapObjectRecord>,
    /// Map-owned deferred physical removal queue matching C++
    /// `Map::i_objectsToRemove` (`Map.cpp:2547-2555`, `2574-2646`).
    ///
    /// Source of truth remains `map_objects`: enqueue mutates the canonical
    /// record, and only `remove_all_objects_in_remove_list_like_cpp` drains this
    /// set into `remove_from_map_like_cpp(..., true)`. Session/ObjectAccessor/DB
    /// caches must not drain or reconstruct this queue.
    objects_to_remove: HashSet<ObjectGuid>,
    /// Map-owned temporary Unit world-object switch queue matching C++
    /// `Map::i_objectsToSwitch` (`Map.h:651-652`) and
    /// `Map::AddObjectToSwitchList` (`Map.cpp:2557-2572`).
    ///
    /// Source of truth remains `map_objects`; callers representing
    /// `WorldObject::SetWorldObject(on)` may enqueue `guid -> on`, and only
    /// `remove_all_objects_in_remove_list_like_cpp` drains this map-local queue
    /// before `objects_to_remove` (`Map.cpp:2574-2594`). Session/ObjectAccessor/DB
    /// caches must not reconstruct or drain it.
    objects_to_switch: HashMap<ObjectGuid, bool>,
    /// C++ `Map::_guidGenerators` (`Map.h:789-791`), lazy initialized by
    /// `Map::GetGuidSequenceGenerator` (`Map.cpp:2505-2511`). This stores only
    /// map-owned sequence counters; callers must compose full ObjectGuids with
    /// their own entry/map/server/realm context and must not feed DB spawn ids
    /// back into this map-local runtime identity source. Trinity's constructor
    /// seeds Transport from global ObjectMgr (`Map.cpp:145-166`); that external
    /// synchronization is intentionally out of scope for this seam, so all
    /// supported HighGuid generators start lazily at 1 unless explicitly set.
    guid_generators: HashMap<HighGuid, MapGuidSequenceGeneratorLikeCpp>,
    /// Map-owned seam for C++ `urand` consumers that are owned by `Map` runtime
    /// state. This slice wires Creature::SelectLevel only; DB/cache callers may
    /// request random level selection through `&mut Map` but must not own or
    /// replay this RNG themselves.
    creature_level_rng_like_cpp: StdRng,
}

impl Map<NoopTerrainGridLoader, NoopGridLifecycle> {
    pub fn new(map_id: u32, instance_id: u32, spawn_mode: Difficulty, grid_expiry_ms: i64) -> Self {
        Self::with_hooks(
            map_id,
            instance_id,
            spawn_mode,
            grid_expiry_ms,
            true,
            100.0,
            NoopTerrainGridLoader,
            NoopGridLifecycle,
        )
    }
}

impl<Terrain, Lifecycle> Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    #[allow(clippy::too_many_arguments)]
    pub fn with_hooks(
        map_id: u32,
        instance_id: u32,
        spawn_mode: Difficulty,
        grid_expiry_ms: i64,
        grid_unload: bool,
        visible_distance: f32,
        terrain: Terrain,
        lifecycle: Lifecycle,
    ) -> Self {
        Self {
            map_id,
            instance_id,
            spawn_mode,
            grid_expiry_ms,
            grid_unload,
            visible_distance,
            grids: std::iter::repeat_with(|| None)
                .take(GRID_SLOT_COUNT)
                .collect(),
            terrain,
            lifecycle,
            active_cells: HashSet::new(),
            personal_phase_tracker: MultiPersonalPhaseTracker::default(),
            spawn_group_state: SpawnGroupRuntimeState::new(),
            respawn_store: RespawnStoreLikeCpp::new(),
            pool_data: SpawnedPoolDataLikeCpp::new(),
            grid_state_unloaded: false,
            creatures_by_spawn_id: HashMap::new(),
            gameobjects_by_spawn_id: HashMap::new(),
            area_triggers_by_spawn_id: HashMap::new(),
            map_objects: HashMap::new(),
            objects_to_remove: HashSet::new(),
            objects_to_switch: HashMap::new(),
            guid_generators: HashMap::new(),
            creature_level_rng_like_cpp: StdRng::from_entropy(),
        }
    }

    pub const fn map_id(&self) -> u32 {
        self.map_id
    }

    pub const fn instance_id(&self) -> u32 {
        self.instance_id
    }

    pub const fn spawn_mode(&self) -> Difficulty {
        self.spawn_mode
    }

    /// Mirrors TrinityCore `urand(min, max)` (`Random.cpp:35-47`): assert
    /// `max >= min` and sample an inclusive integer range. Ownership remains on
    /// `Map` so loaded-grid runtime consumers advance one canonical RNG stream.
    pub fn urand_inclusive_like_cpp(&mut self, min: u32, max: u32) -> u32 {
        assert!(max >= min, "C++ urand requires max >= min");
        self.creature_level_rng_like_cpp.gen_range(min..=max)
    }

    /// Mirrors `Creature::SelectLevel` for DB/template min/max rows: fixed rows
    /// use `MinLevel` without consuming RNG; variable rows call inclusive `urand`.
    pub fn select_creature_level_like_cpp(&mut self, min_level: u8, max_level: u8) -> u8 {
        if min_level == max_level {
            return min_level;
        }
        let selected = self.urand_inclusive_like_cpp(u32::from(min_level), u32::from(max_level));
        selected as u8
    }

    #[cfg(test)]
    fn seed_creature_level_rng_for_tests_like_cpp(&mut self, seed: u64) {
        self.creature_level_rng_like_cpp = StdRng::seed_from_u64(seed);
    }

    pub fn generate_low_guid_like_cpp(
        &mut self,
        high: HighGuid,
    ) -> Result<i64, MapGuidSequenceErrorLikeCpp> {
        Self::ensure_map_guid_sequence_source_like_cpp(high)?;
        Ok(self
            .guid_sequence_generator_like_cpp(high)
            .generator
            .generate())
    }

    pub fn get_max_low_guid_like_cpp(
        &mut self,
        high: HighGuid,
    ) -> Result<i64, MapGuidSequenceErrorLikeCpp> {
        Self::ensure_map_guid_sequence_source_like_cpp(high)?;
        Ok(self
            .guid_sequence_generator_like_cpp(high)
            .generator
            .next_after_max_used())
    }

    pub fn set_guid_sequence_like_cpp(
        &mut self,
        high: HighGuid,
        next: i64,
    ) -> Result<(), MapGuidSequenceErrorLikeCpp> {
        Self::ensure_map_guid_sequence_source_like_cpp(high)?;
        self.guid_sequence_generator_like_cpp(high)
            .generator
            .set(next);
        Ok(())
    }

    fn guid_sequence_generator_like_cpp(
        &mut self,
        high: HighGuid,
    ) -> &mut MapGuidSequenceGeneratorLikeCpp {
        self.guid_generators
            .entry(high)
            .or_insert_with(|| MapGuidSequenceGeneratorLikeCpp::new(high))
    }

    fn ensure_map_guid_sequence_source_like_cpp(
        high: HighGuid,
    ) -> Result<(), MapGuidSequenceErrorLikeCpp> {
        match high {
            HighGuid::WorldTransaction
            | HighGuid::StaticDoor
            | HighGuid::Transport
            | HighGuid::Conversation
            | HighGuid::Creature
            | HighGuid::Vehicle
            | HighGuid::Pet
            | HighGuid::GameObject
            | HighGuid::DynamicObject
            | HighGuid::AreaTrigger
            | HighGuid::Corpse
            | HighGuid::LootObject
            | HighGuid::SceneObject
            | HighGuid::Scenario
            | HighGuid::AIGroup
            | HighGuid::DynamicDoor
            | HighGuid::Vignette
            | HighGuid::CallForHelp
            | HighGuid::AIResource
            | HighGuid::AILock
            | HighGuid::AILockTicket
            | HighGuid::Cast => Ok(()),
            _ => Err(MapGuidSequenceErrorLikeCpp::UnsupportedSequenceSource { high }),
        }
    }

    pub const fn grid_expiry_ms(&self) -> i64 {
        self.grid_expiry_ms
    }

    pub const fn grid_unload(&self) -> bool {
        self.grid_unload
    }

    pub const fn visibility_range(&self) -> f32 {
        self.visible_distance
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    pub fn personal_phase_tracker(&self) -> &MultiPersonalPhaseTracker {
        &self.personal_phase_tracker
    }

    /// Map-owned bridge for C++ `Map::_respawnTimes` and the per-type respawn maps.
    ///
    /// C++ anchors:
    /// - `Map.h:472-480` returns zero when a respawn time is missing or the type has no map.
    /// - `Map.h:748-777` stores respawn queues/maps on `Map`; AreaTrigger has no respawn map.
    /// - `Map.cpp:2057-2150` adds, replaces, gets, removes, and unloads respawn info coherently.
    pub const fn respawn_store_like_cpp(&self) -> &RespawnStoreLikeCpp {
        &self.respawn_store
    }

    /// Mutable access to the map-owned respawn store for bounded tests/bridges.
    ///
    /// Future runtime callers must treat `Map` as the owner/source of truth and
    /// must not keep external respawn stores that later overwrite this state.
    pub fn respawn_store_like_cpp_mut(&mut self) -> &mut RespawnStoreLikeCpp {
        &mut self.respawn_store
    }

    pub fn add_respawn_info_like_cpp(
        &mut self,
        info: RespawnInfoLikeCpp,
    ) -> AddRespawnInfoOutcomeLikeCpp {
        self.respawn_store.add_respawn_info_like_cpp(info)
    }

    pub fn get_respawn_time_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> i64 {
        self.respawn_store
            .get_respawn_time_like_cpp(object_type, spawn_id)
    }

    pub fn get_respawn_info_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<&RespawnInfoLikeCpp> {
        self.respawn_store
            .get_respawn_info_like_cpp(object_type, spawn_id)
    }

    /// C++ `Map::GetLinkedRespawnTime` dependency slice.
    ///
    /// C++ anchor: `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3607-3620`.
    /// The linked respawn store is read-only ObjectMgr-style metadata; the timer
    /// source of truth remains this `Map`'s map-owned `RespawnStoreLikeCpp`.
    pub fn get_linked_respawn_time_like_cpp(
        &self,
        guid: ObjectGuid,
        linked_store: &LinkedRespawnStoreLikeCpp,
    ) -> i64 {
        let linked_guid = linked_store.get_linked_respawn_guid_like_cpp(guid);
        match linked_guid.high_type() {
            HighGuid::Creature => self.get_respawn_time_like_cpp(
                SpawnObjectType::Creature,
                linked_guid.counter() as SpawnId,
            ),
            HighGuid::GameObject => self.get_respawn_time_like_cpp(
                SpawnObjectType::GameObject,
                linked_guid.counter() as SpawnId,
            ),
            _ => 0,
        }
    }

    /// Linked-respawn branch from C++ `Map::CheckRespawn`.
    ///
    /// C++ anchor: `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2004-2020`.
    /// This implements only the linked-time guard after earlier live-object
    /// blockers have already cleared. It never runs PoolMgr, DoRespawn, DB
    /// save/delete, entity creation, fanout, or RNG; the caller supplies the
    /// explicit jitter that represents C++ `urand(5, 15)`.
    pub fn check_respawn_linked_respawn_guard_like_cpp(
        &self,
        info: &mut RespawnInfoLikeCpp,
        linked_store: &LinkedRespawnStoreLikeCpp,
        now: i64,
        jitter_secs: u32,
    ) -> CheckRespawnLinkedRespawnGuardOutcomeLikeCpp {
        let Some(guid_high) = (match info.object_type {
            SpawnObjectType::Creature => Some(HighGuid::Creature),
            SpawnObjectType::GameObject => Some(HighGuid::GameObject),
            SpawnObjectType::AreaTrigger => None,
        }) else {
            return CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::UnsupportedSpawnType;
        };

        let this_guid = ObjectGuid::create_world_object(
            guid_high,
            0,
            0,
            self.map_id as u16,
            0,
            info.entry,
            info.spawn_id as i64,
        );
        let linked_time = self.get_linked_respawn_time_like_cpp(this_guid, linked_store);
        if linked_time == 0 {
            return CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::Allowed;
        }

        if linked_time == i64::MAX {
            info.respawn_time = linked_time;
            return CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedInfinite;
        }

        if linked_store.get_linked_respawn_guid_like_cpp(this_guid) == this_guid {
            info.respawn_time = now + WEEK_SECS_LIKE_CPP;
            return CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedSelfNeverRespawn;
        }

        info.respawn_time = now.max(linked_time) + i64::from(jitter_secs);
        CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedDelayed
    }

    pub fn remove_respawn_time_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<RespawnInfoLikeCpp> {
        self.respawn_store
            .remove_respawn_time_like_cpp(object_type, spawn_id)
    }

    pub fn unload_all_respawn_infos_like_cpp(&mut self) {
        self.respawn_store.unload_all_respawn_infos_like_cpp();
    }

    pub fn respawn_timer_keys_like_cpp(
        &self,
    ) -> impl Iterator<Item = (SpawnObjectType, SpawnId)> + '_ {
        self.respawn_store.respawn_timer_keys_like_cpp()
    }

    /// Delegates the C++ `Map::ProcessRespawns` action planner to the map-owned store.
    ///
    /// This only plans side effects. It does not execute PoolMgr, DoRespawn,
    /// DB persistence/delete, linked-respawn checks, entity creation, or fanout.
    pub fn process_due_respawns_like_cpp(
        &mut self,
        now: i64,
        is_part_of_pool: impl FnMut(SpawnObjectType, SpawnId) -> Option<u32>,
        check_respawn: impl FnMut(&mut RespawnInfoLikeCpp) -> CheckRespawnOutcomeLikeCpp,
    ) -> Vec<ProcessRespawnActionLikeCpp> {
        self.respawn_store
            .process_due_respawns_like_cpp(now, is_part_of_pool, check_respawn)
    }

    /// Executes the safe map-local half of actions returned by represented C++
    /// `PoolMgr::UpdatePool` planning.
    ///
    /// C++ anchors:
    /// - `PoolMgr.cpp:183-257` `DespawnObject` / `Despawn1Object` removes
    ///   current map objects and optionally removes respawn timers.
    /// - `PoolMgr.cpp:353-403` `Spawn1Object` / `ReSpawn1Object` create only
    ///   on loaded grids; RustyCore reports that missing runtime instead of
    ///   creating DB-backed entities in `wow-map`.
    fn apply_pool_typed_spawn_plan_safe_map_actions_like_cpp(
        &mut self,
        plan: &PoolTypedSpawnPlanLikeCpp,
        spawn_store: &SpawnStore,
        summary: &mut ProcessRespawnsSafeSideEffectsSummaryLikeCpp,
    ) {
        if let Some(object_plan) = plan.object_plan.as_ref() {
            self.apply_pool_spawn_object_plan_safe_map_actions_like_cpp(
                object_plan,
                spawn_store,
                summary,
            );
        }
    }

    fn apply_pool_spawn_object_plan_safe_map_actions_like_cpp(
        &mut self,
        plan: &PoolSpawnObjectPlanLikeCpp,
        spawn_store: &SpawnStore,
        summary: &mut ProcessRespawnsSafeSideEffectsSummaryLikeCpp,
    ) {
        for action in &plan.actions {
            self.apply_pool_spawn_object_action_safe_map_action_like_cpp(
                *action,
                spawn_store,
                summary,
            );
        }
    }

    fn apply_pool_spawn_object_action_safe_map_action_like_cpp(
        &mut self,
        action: PoolSpawnObjectActionLikeCpp,
        spawn_store: &SpawnStore,
        summary: &mut ProcessRespawnsSafeSideEffectsSummaryLikeCpp,
    ) {
        match action {
            PoolSpawnObjectActionLikeCpp::DespawnOne { kind, guid } => {
                self.apply_pool_despawn_one_safe_map_action_like_cpp(kind, guid, summary);
            }
            PoolSpawnObjectActionLikeCpp::RespawnOne { kind, guid } => {
                self.apply_pool_despawn_one_safe_map_action_like_cpp(kind, guid, summary);
                self.report_pool_spawn_one_action_like_cpp(kind, guid, true, spawn_store, summary);
            }
            PoolSpawnObjectActionLikeCpp::RemoveRespawnTime { kind, guid } => {
                let Some(object_type) = pool_member_kind_to_spawn_object_type_like_cpp(kind) else {
                    summary.pool_unsupported_action_kind += 1;
                    return;
                };
                if self
                    .remove_respawn_time_like_cpp(object_type, guid as SpawnId)
                    .is_some()
                {
                    summary.pool_respawn_timers_removed += 1;
                } else {
                    summary.pool_respawn_timers_missing += 1;
                }
            }
            PoolSpawnObjectActionLikeCpp::SpawnOne { kind, guid } => {
                self.report_pool_spawn_one_action_like_cpp(kind, guid, false, spawn_store, summary);
            }
        }
    }

    fn apply_pool_despawn_one_safe_map_action_like_cpp(
        &mut self,
        kind: PoolMemberKindLikeCpp,
        spawn_id: u64,
        summary: &mut ProcessRespawnsSafeSideEffectsSummaryLikeCpp,
    ) {
        let spawn_id = spawn_id as SpawnId;
        let guids = match kind {
            PoolMemberKindLikeCpp::Creature => {
                self.creature_spawn_id_store_guids_like_cpp(spawn_id)
            }
            PoolMemberKindLikeCpp::GameObject => {
                self.gameobject_spawn_id_store_guids_like_cpp(spawn_id)
            }
            PoolMemberKindLikeCpp::Pool => {
                summary.pool_unsupported_action_kind += 1;
                return;
            }
        };

        for guid in guids {
            if self.map_object_record(guid).is_none() {
                summary.pool_stale_index_entries += 1;
                continue;
            }
            let outcome = self.add_object_to_remove_list_like_cpp(guid);
            if outcome.missing_or_stale {
                summary.pool_stale_index_entries += 1;
            } else if outcome.unsupported_kind.is_some() {
                summary.pool_unsupported_action_kind += 1;
            } else if outcome.queued {
                summary.pool_objects_removed += 1;
            }
        }
    }

    fn report_pool_spawn_one_action_like_cpp(
        &self,
        kind: PoolMemberKindLikeCpp,
        spawn_id: u64,
        respawn: bool,
        spawn_store: &SpawnStore,
        summary: &mut ProcessRespawnsSafeSideEffectsSummaryLikeCpp,
    ) {
        let Some(object_type) = pool_member_kind_to_spawn_object_type_like_cpp(kind) else {
            summary.pool_unsupported_action_kind += 1;
            return;
        };
        let Some(spawn_data) = spawn_store.spawn_data(object_type, spawn_id as SpawnId) else {
            summary.pool_spawn_actions_missing_spawn_data += 1;
            return;
        };
        let cell = cell_from_world(spawn_data.spawn_point.x, spawn_data.spawn_point.y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        if self.is_grid_loaded(grid) {
            summary.pool_spawn_actions_blocked_loaded_grid += 1;
            summary
                .pool_spawn_action_load_plans
                .push(PoolSpawnActionLoadPlanLikeCpp {
                    object_type,
                    spawn_id: spawn_id as SpawnId,
                    respawn,
                });
        } else {
            summary.pool_spawn_actions_skipped_unloaded_grid += 1;
        }
    }

    /// Safe side-effect seam for represented C++ `Map::ProcessRespawns` branches.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2191-2198` processes only due respawn timers in queue order.
    /// - `Map.cpp:2200-2211` detects `PoolMgr::IsPartOfAPool` before
    ///   `CheckRespawn`, updates map-owned `SpawnedPoolData` through
    ///   `PoolMgr::UpdatePool`, then removes the respawn timer with DB-delete
    ///   ownership left to the caller bridge.
    /// - `Map.cpp:2213-2224` allowed respawn removes+calls `DoRespawn`; blocked here.
    /// - `Map.cpp:2226-2231` removes a timer when `CheckRespawn` set respawnTime=0.
    /// - `Map.cpp:2233-2238` updates the heap position and persists a future
    ///   `respawnTime` when `CheckRespawn` rescheduled the timer.
    ///
    /// This helper executes only safe map-owned in-memory effects represented so
    /// far: pooled timer -> deterministic `UpdatePool` plan + map-owned
    /// `SpawnedPoolDataLikeCpp` mutation + timer removal, `DoRespawn`'s unloaded-grid
    /// early return after timer removal, loaded-grid non-pooled `DoRespawn` via a
    /// caller-supplied typed `MapObjectRecord` loader, zero-delete for inactive
    /// spawn-groups/live-object blockers, and linked-respawn future reschedule by
    /// replacing the same map-owned respawn timer. DB effects, live record
    /// construction, grid/session fanout, and scripts stay outside this lock-owned
    /// helper.
    /// If the oldest due timer needs an unavailable/error branch, it is left intact
    /// and processing stops to preserve C++ queue order.
    pub fn process_due_respawns_composite_loaded_grid_respawns_like_cpp<F, R, C, L>(
        &mut self,
        now: i64,
        spawn_store: &SpawnStore,
        linked_store: &LinkedRespawnStoreLikeCpp,
        pool_mgr: &PoolMgrLikeCpp,
        jitter_secs: u32,
        respawn_dynamic_escortnpc: bool,
        mut is_creature_escorted: F,
        mut explicit_roll_for: R,
        mut choose_equal: C,
        mut load_record: L,
    ) -> ProcessRespawnsSafeSideEffectsSummaryLikeCpp
    where
        F: FnMut(ObjectGuid, &Creature) -> bool,
        R: FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        C: FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
        L: FnMut(&mut Self, SpawnObjectType, SpawnId) -> Option<LoadedGridRespawnRecordsLikeCpp>,
    {
        let mut summary = ProcessRespawnsSafeSideEffectsSummaryLikeCpp::default();

        loop {
            let next_key = { self.respawn_timer_keys_like_cpp().next() };
            let Some((object_type, spawn_id)) = next_key else {
                break;
            };
            let Some(info) = self
                .get_respawn_info_like_cpp(object_type, spawn_id)
                .cloned()
            else {
                summary.blocked_missing_spawn_data += 1;
                break;
            };
            if now < info.respawn_time {
                break;
            }

            match pool_mgr.is_part_of_a_pool_like_cpp(object_type, spawn_id) {
                Ok(0) => {}
                Ok(pool_id) => match pool_mgr.update_pool_plan_like_cpp(
                    &mut self.pool_data,
                    pool_id,
                    object_type,
                    spawn_id,
                    &mut explicit_roll_for,
                    &mut choose_equal,
                ) {
                    Ok(plan) => {
                        self.apply_pool_typed_spawn_plan_safe_map_actions_like_cpp(
                            &plan,
                            spawn_store,
                            &mut summary,
                        );
                        self.remove_respawn_time_like_cpp(object_type, spawn_id);
                        summary.processed_pool_timers += 1;
                        summary.pool_update_plans.push(plan);
                        continue;
                    }
                    Err(error) => {
                        summary.blocked_pool_plan_errors.push(error);
                        break;
                    }
                },
                Err(error) => {
                    summary.blocked_pool_plan_errors.push(error);
                    break;
                }
            }

            if spawn_store.spawn_data(object_type, spawn_id).is_none() {
                summary.blocked_missing_spawn_data += 1;
                break;
            }

            let mut checked_info = info;
            match self.check_respawn_like_cpp(
                &mut checked_info,
                spawn_store,
                linked_store,
                now,
                jitter_secs,
                respawn_dynamic_escortnpc,
                &mut is_creature_escorted,
            ) {
                CheckRespawnCompositeOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer
                    if checked_info.respawn_time == 0 =>
                {
                    self.remove_respawn_time_like_cpp(object_type, spawn_id);
                    summary.deleted_inactive_spawn_group += 1;
                }
                CheckRespawnCompositeOutcomeLikeCpp::AliveCreatureBlocksRespawn
                | CheckRespawnCompositeOutcomeLikeCpp::GameObjectBlocksRespawn
                    if checked_info.respawn_time == 0 =>
                {
                    self.remove_respawn_time_like_cpp(object_type, spawn_id);
                    summary.deleted_live_object_blocker += 1;
                }
                CheckRespawnCompositeOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer
                | CheckRespawnCompositeOutcomeLikeCpp::AliveCreatureBlocksRespawn
                | CheckRespawnCompositeOutcomeLikeCpp::GameObjectBlocksRespawn => {
                    summary.blocked_do_respawn_runtime += 1;
                    break;
                }
                CheckRespawnCompositeOutcomeLikeCpp::Allowed => {
                    if is_grid_id_loaded(self, checked_info.grid_id) {
                        let Some(records) = load_record(self, object_type, spawn_id) else {
                            summary.blocked_loaded_grid_respawn_loads += 1;
                            summary.blocked_do_respawn_runtime += 1;
                            break;
                        };

                        // C++ `ProcessRespawns` pops/erases the timer before
                        // calling `DoRespawn`. For DB-backed GameObjects,
                        // `GameObject::Create` may also create and AddToMap a
                        // linked trap first; that AddToMap failure only deletes
                        // the trap and does not block the owner. The primary
                        // `AddToMap` result remains determinant as in C++.
                        self.remove_respawn_time_like_cpp(object_type, spawn_id);
                        for pre_add_record in records.pre_add_records {
                            let _ = self.add_map_object_record_to_map_like_cpp(pre_add_record);
                        }
                        match self.add_map_object_record_to_map_like_cpp(records.primary_record) {
                            Ok(_outcome) => {
                                summary.executed_loaded_grid_respawns += 1;
                            }
                            Err(_error) => {
                                summary.blocked_loaded_grid_respawn_add_to_map += 1;
                            }
                        }
                        continue;
                    }

                    self.remove_respawn_time_like_cpp(object_type, spawn_id);
                    summary.processed_unloaded_grid_respawns += 1;
                    continue;
                }
                CheckRespawnCompositeOutcomeLikeCpp::LinkedInfinite
                | CheckRespawnCompositeOutcomeLikeCpp::LinkedSelfNeverRespawn
                | CheckRespawnCompositeOutcomeLikeCpp::LinkedDelayed => {
                    if checked_info.respawn_time == i64::MAX || checked_info.respawn_time > now {
                        let rescheduled_info = checked_info.clone();
                        self.remove_respawn_time_like_cpp(object_type, spawn_id);
                        self.add_respawn_info_like_cpp(checked_info);
                        summary.rescheduled_linked_respawns.push(rescheduled_info);
                    } else {
                        summary.blocked_linked_respawn_non_future += 1;
                        break;
                    }
                }
                CheckRespawnCompositeOutcomeLikeCpp::MissingSpawnData => {
                    summary.blocked_missing_spawn_data += 1;
                    break;
                }
                CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType => {
                    summary.blocked_unsupported_spawn_type += 1;
                    break;
                }
            }
        }

        summary
    }

    /// Compatibility wrapper that preserves the old safe-side-effects API by
    /// keeping loaded-grid non-pooled `DoRespawn` blocked through a loader that
    /// returns no typed record.
    pub fn process_due_respawns_composite_safe_side_effects_like_cpp<F, R, C>(
        &mut self,
        now: i64,
        spawn_store: &SpawnStore,
        linked_store: &LinkedRespawnStoreLikeCpp,
        pool_mgr: &PoolMgrLikeCpp,
        jitter_secs: u32,
        respawn_dynamic_escortnpc: bool,
        is_creature_escorted: F,
        explicit_roll_for: R,
        choose_equal: C,
    ) -> ProcessRespawnsSafeSideEffectsSummaryLikeCpp
    where
        F: FnMut(ObjectGuid, &Creature) -> bool,
        R: FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        C: FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    {
        self.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            now,
            spawn_store,
            linked_store,
            pool_mgr,
            jitter_secs,
            respawn_dynamic_escortnpc,
            is_creature_escorted,
            explicit_roll_for,
            choose_equal,
            |_map, _object_type, _spawn_id| None,
        )
    }

    /// Compatibility wrapper for callers that still use the old delete-only name.
    pub fn process_due_respawns_composite_delete_only_like_cpp<F>(
        &mut self,
        now: i64,
        spawn_store: &SpawnStore,
        linked_store: &LinkedRespawnStoreLikeCpp,
        jitter_secs: u32,
        respawn_dynamic_escortnpc: bool,
        is_creature_escorted: F,
    ) -> ProcessRespawnsDeleteOnlySummaryLikeCpp
    where
        F: FnMut(ObjectGuid, &Creature) -> bool,
    {
        let pool_mgr = PoolMgrLikeCpp::new();
        self.process_due_respawns_composite_safe_side_effects_like_cpp(
            now,
            spawn_store,
            linked_store,
            &pool_mgr,
            jitter_secs,
            respawn_dynamic_escortnpc,
            is_creature_escorted,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        )
    }

    /// Compatibility wrapper for the original inactive-spawn-group delete-only seam.
    pub fn process_due_respawns_spawn_group_delete_only_like_cpp(
        &mut self,
        now: i64,
        spawn_store: &SpawnStore,
    ) -> ProcessRespawnsDeleteOnlySummaryLikeCpp {
        let linked_store = LinkedRespawnStoreLikeCpp::new();
        self.process_due_respawns_composite_safe_side_effects_like_cpp(
            now,
            spawn_store,
            &linked_store,
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        )
    }

    /// First represented guard from C++ `Map::CheckRespawn`.
    ///
    /// C++ anchors:
    /// - `Map.cpp:1956-1957` resolves `SpawnData` and asserts when missing.
    /// - `Map.cpp:1959-1964` clears `respawnTime` and returns false when the
    ///   spawn group is inactive.
    ///
    /// This is only the spawn-group subdependency of `CheckRespawn`. It does not
    /// implement live by-spawn existence, escort dynamic rules, gameobject live
    /// checks, linked respawn, random 5-15 reschedule, PoolMgr, `DoRespawn`, DB
    /// save/delete, or world-server tick integration. Missing `SpawnData` is a
    /// temporary defensive fallback for incomplete ownership: C++ would assert;
    /// RustyCore returns `MissingSpawnData`, does not mutate `respawn_time`, and
    /// leaves timer deletion/reschedule decisions to the caller.
    pub fn check_respawn_spawn_group_guard_like_cpp(
        &self,
        info: &mut RespawnInfoLikeCpp,
        spawn_store: &SpawnStore,
    ) -> CheckRespawnSpawnGroupGuardOutcomeLikeCpp {
        let Some(spawn_data) = spawn_store.spawn_data(info.object_type, info.spawn_id) else {
            return CheckRespawnSpawnGroupGuardOutcomeLikeCpp::MissingSpawnData;
        };

        if !self.is_spawn_group_active_like_cpp(Some(&spawn_data.spawn_group)) {
            info.respawn_time = 0;
            return CheckRespawnSpawnGroupGuardOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer;
        }

        CheckRespawnSpawnGroupGuardOutcomeLikeCpp::Allowed
    }

    /// Live object existence guard from C++ `Map::CheckRespawn`.
    ///
    /// C++ anchors:
    /// - `Map.cpp:1966-2002` checks whether an already-live creature/gameobject
    ///   with the same spawn id blocks respawn, clears `respawnTime`, and returns
    ///   false when blocked.
    /// - `Map.cpp:1972-1983` allows dynamic escort NPC respawn only when the
    ///   matching live creature is already escorting.
    ///
    /// Source of truth for this slice is canonical map-owned `map_objects`, with
    /// typed map-local by-spawn-id indexes mirroring Trinity's multimap stores.
    /// Callers must provide the `CONFIG_RESPAWN_DYNAMIC_ESCORTNPC` value and the
    /// real escort runtime predicate; this helper does not invent
    /// `Creature::IsEscorted`, PoolMgr, linked respawn, `DoRespawn`, DB writes, or
    /// fanout side effects.
    pub fn check_respawn_live_object_guard_like_cpp<F>(
        &self,
        info: &mut RespawnInfoLikeCpp,
        spawn_store: &SpawnStore,
        respawn_dynamic_escortnpc: bool,
        mut is_creature_escorted: F,
    ) -> CheckRespawnLiveObjectGuardOutcomeLikeCpp
    where
        F: FnMut(ObjectGuid, &Creature) -> bool,
    {
        let Some(spawn_data) = spawn_store.spawn_data(info.object_type, info.spawn_id) else {
            return CheckRespawnLiveObjectGuardOutcomeLikeCpp::MissingSpawnData;
        };

        match info.object_type {
            SpawnObjectType::Creature => {
                let is_escort = respawn_dynamic_escortnpc
                    && spawn_data
                        .spawn_group
                        .flags
                        .contains(SpawnGroupFlags::ESCORTQUESTNPC);

                let Some(creature_guids) = self.creatures_by_spawn_id.get(&info.spawn_id) else {
                    return CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed;
                };

                for guid in creature_guids {
                    let Some(record) = self.map_objects.get(guid) else {
                        continue;
                    };
                    let Some(creature) = record.creature() else {
                        continue;
                    };
                    if creature.spawn_id() != info.spawn_id || !creature.is_alive() {
                        continue;
                    }
                    if is_escort && is_creature_escorted(creature.guid(), creature) {
                        continue;
                    }

                    info.respawn_time = 0;
                    return CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn;
                }

                CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed
            }
            SpawnObjectType::GameObject => {
                if self
                    .gameobjects_by_spawn_id
                    .get(&info.spawn_id)
                    .is_some_and(|gameobject_guids| {
                        gameobject_guids.iter().any(|guid| {
                            self.map_objects.get(guid).is_some_and(|record| {
                                record.game_object().is_some_and(|gameobject| {
                                    gameobject.spawn_id() == info.spawn_id
                                })
                            })
                        })
                    })
                {
                    info.respawn_time = 0;
                    return CheckRespawnLiveObjectGuardOutcomeLikeCpp::GameObjectBlocksRespawn;
                }

                CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed
            }
            SpawnObjectType::AreaTrigger => {
                CheckRespawnLiveObjectGuardOutcomeLikeCpp::UnsupportedSpawnType
            }
        }
    }

    /// Composite helper preserving represented C++ `Map::CheckRespawn` guard order.
    ///
    /// C++ anchors:
    /// - `Map.cpp:1950-2023` defines the full return/mutate contract.
    /// - `Map.cpp:1956-1964` checks spawn-group activity first.
    /// - `Map.cpp:1966-2002` checks live object blockers second.
    /// - `Map.cpp:2004-2020` checks linked respawn only after earlier guards allow.
    ///
    /// Runtime timer source of truth is this map-owned `RespawnStoreLikeCpp` via
    /// `RespawnInfoLikeCpp`; metadata stays caller-supplied `SpawnStore` until
    /// ObjectMgr ownership moves into `Map`; live blockers come from `map_objects`;
    /// linked metadata is read-only. This helper deliberately does not execute
    /// PoolMgr, `DoRespawn`, DB save/delete, entity creation, fanout, or RNG.
    pub fn check_respawn_like_cpp<F>(
        &self,
        info: &mut RespawnInfoLikeCpp,
        spawn_store: &SpawnStore,
        linked_store: &LinkedRespawnStoreLikeCpp,
        now: i64,
        jitter_secs: u32,
        respawn_dynamic_escortnpc: bool,
        mut is_creature_escorted: F,
    ) -> CheckRespawnCompositeOutcomeLikeCpp
    where
        F: FnMut(ObjectGuid, &Creature) -> bool,
    {
        if matches!(info.object_type, SpawnObjectType::AreaTrigger) {
            return CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType;
        }

        match self.check_respawn_spawn_group_guard_like_cpp(info, spawn_store) {
            CheckRespawnSpawnGroupGuardOutcomeLikeCpp::Allowed => {}
            CheckRespawnSpawnGroupGuardOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer => {
                return CheckRespawnCompositeOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer;
            }
            CheckRespawnSpawnGroupGuardOutcomeLikeCpp::MissingSpawnData => {
                return CheckRespawnCompositeOutcomeLikeCpp::MissingSpawnData;
            }
        }

        match self.check_respawn_live_object_guard_like_cpp(
            info,
            spawn_store,
            respawn_dynamic_escortnpc,
            &mut is_creature_escorted,
        ) {
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed => {}
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn => {
                return CheckRespawnCompositeOutcomeLikeCpp::AliveCreatureBlocksRespawn;
            }
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::GameObjectBlocksRespawn => {
                return CheckRespawnCompositeOutcomeLikeCpp::GameObjectBlocksRespawn;
            }
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::MissingSpawnData => {
                return CheckRespawnCompositeOutcomeLikeCpp::MissingSpawnData;
            }
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::UnsupportedSpawnType => {
                return CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType;
            }
        }

        match self.check_respawn_linked_respawn_guard_like_cpp(info, linked_store, now, jitter_secs)
        {
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::Allowed => {
                CheckRespawnCompositeOutcomeLikeCpp::Allowed
            }
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedInfinite => {
                CheckRespawnCompositeOutcomeLikeCpp::LinkedInfinite
            }
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedSelfNeverRespawn => {
                CheckRespawnCompositeOutcomeLikeCpp::LinkedSelfNeverRespawn
            }
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedDelayed => {
                CheckRespawnCompositeOutcomeLikeCpp::LinkedDelayed
            }
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::UnsupportedSpawnType => {
                CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType
            }
        }
    }

    /// Map-owned bridge for C++ `Map::_toggledSpawnGroupIds`.
    ///
    /// C++ anchors:
    /// - `Map.h:780-781` stores toggled spawn group ids on `Map`.
    /// - `Map.cpp:2427-2439` toggles only non-system existing groups.
    /// - `Map.cpp:2441-2453` queries missing/system/default/manual semantics.
    ///
    /// RustyCore does not yet wire ObjectMgr/SpawnStore ownership into `Map`, so
    /// callers must pass the already-resolved template as an honest bridge.
    pub const fn spawn_group_state(&self) -> &SpawnGroupRuntimeState {
        &self.spawn_group_state
    }

    pub fn set_spawn_group_active_like_cpp(
        &mut self,
        group: Option<&SpawnGroupTemplateData>,
        state: bool,
    ) -> SpawnGroupActiveChange {
        self.spawn_group_state
            .set_spawn_group_active_like_cpp(group, state)
    }

    pub fn set_spawn_group_inactive_like_cpp(
        &mut self,
        group: Option<&SpawnGroupTemplateData>,
    ) -> SpawnGroupActiveChange {
        self.set_spawn_group_active_like_cpp(group, false)
    }

    pub fn is_spawn_group_active_like_cpp(&self, group: Option<&SpawnGroupTemplateData>) -> bool {
        self.spawn_group_state.is_spawn_group_active_like_cpp(group)
    }

    pub const fn pool_data_like_cpp(&self) -> &SpawnedPoolDataLikeCpp {
        &self.pool_data
    }

    pub const fn pool_data_mut_like_cpp(&mut self) -> &mut SpawnedPoolDataLikeCpp {
        &mut self.pool_data
    }

    /// C++ `Map` constructor calls `sPoolMgr->InitPoolsForMap(this)` before
    /// startup respawn and spawn-group initialization. This represented seam
    /// applies deterministic autospawn `SpawnPool` plans into the map-owned
    /// `SpawnedPoolDataLikeCpp` and returns action records for future live
    /// `Spawn1Object`/`ReSpawn1Object`/`DespawnObject` owners; it does not create
    /// entities or fan out packets.
    pub fn init_pools_for_map_like_cpp(
        &mut self,
        pool_mgr: &PoolMgrLikeCpp,
        explicit_roll_for: impl FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        choose_equal: impl FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    ) -> PoolInitForMapPlanLikeCpp {
        pool_mgr.init_pools_for_map_plan_like_cpp(
            self.map_id,
            &mut self.pool_data,
            explicit_roll_for,
            choose_equal,
        )
    }

    /// Bridge for C++ `Map::ShouldBeSpawnedOnGridLoad` callers while `Map` does
    /// not yet own the ObjectMgr spawn metadata. The canonical toggle state,
    /// respawn timers, and `SpawnedPoolData` are map-owned; spawn metadata remains
    /// caller-supplied.
    pub fn spawn_grid_load_state_like_cpp<'a>(
        &'a self,
        spawn_store: &'a SpawnStore,
    ) -> SpawnGridLoadStateLikeCpp<'a> {
        SpawnGridLoadStateLikeCpp::new(spawn_store, &self.spawn_group_state)
            .with_respawn_timers(self.respawn_store.respawn_timer_keys_like_cpp())
            .with_pool_spawned_objects(self.pool_data.spawned_objects_like_cpp())
    }

    /// Pure bridge for C++ `Map::InitSpawnGroupState` over pre-resolved group
    /// templates. It intentionally applies only active-state toggles; live
    /// spawn/despawn, pool runtime, respawn persistence, and fanout are later gaps.
    pub fn init_spawn_group_state_like_cpp<'a, I, F>(
        &mut self,
        groups: I,
        mut meets_conditions: F,
    ) -> Vec<(u32, SpawnGroupActiveChange)>
    where
        I: IntoIterator<Item = &'a SpawnGroupTemplateData>,
        F: FnMut(&SpawnGroupTemplateData) -> bool,
    {
        let mut changes = Vec::new();
        for group in groups {
            if group.is_system() {
                continue;
            }
            let active = meets_conditions(group);
            changes.push((
                group.group_id,
                self.set_spawn_group_active_like_cpp(Some(group), active),
            ));
        }
        changes
    }

    /// Pure action planner for C++ `Map::UpdateSpawnGroupConditions` over
    /// pre-resolved spawn-group templates.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2471-2502` loops map groups, compares
    ///   `IsSpawnGroupActive` with `ConditionMgr`, and runs spawn/despawn or
    ///   inactive branches.
    /// - `Map.cpp:2427-2453` owns `_toggledSpawnGroupIds` semantics through
    ///   `SetSpawnGroupActive` / `IsSpawnGroupActive`.
    /// - `SpawnData.h:51-63` defines manual and condition-failure flags.
    ///
    /// This does not run live `SpawnGroupSpawn`/`SpawnGroupDespawn`, touch DB,
    /// mutate toggles, simulate pools, persist respawns, create entities, or
    /// fan out updates. The closure only replaces C++
    /// `ConditionMgr::IsMapMeetingNotGroupedConditions` for already-resolved
    /// condition outcomes.
    pub fn plan_update_spawn_group_conditions_like_cpp<'a, I, F>(
        &self,
        groups: I,
        mut meets_conditions: F,
    ) -> Vec<(u32, SpawnGroupConditionActionLikeCpp)>
    where
        I: IntoIterator<Item = &'a SpawnGroupTemplateData>,
        F: FnMut(&SpawnGroupTemplateData) -> bool,
    {
        let mut actions = Vec::new();
        for group in groups {
            let is_active = self.is_spawn_group_active_like_cpp(Some(group));
            let should_be_active = meets_conditions(group);

            if group.flags.contains(SpawnGroupFlags::MANUAL_SPAWN) {
                if is_active
                    && !should_be_active
                    && group
                        .flags
                        .contains(SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE)
                {
                    actions.push((
                        group.group_id,
                        SpawnGroupConditionActionLikeCpp::condition_failure_despawn(),
                    ));
                } else {
                    actions.push((group.group_id, SpawnGroupConditionActionLikeCpp::Noop));
                }
                continue;
            }

            if is_active == should_be_active {
                actions.push((group.group_id, SpawnGroupConditionActionLikeCpp::Noop));
                continue;
            }

            let action = if should_be_active {
                SpawnGroupConditionActionLikeCpp::spawn_group_spawn_default()
            } else if group
                .flags
                .contains(SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE)
            {
                SpawnGroupConditionActionLikeCpp::condition_failure_despawn()
            } else {
                SpawnGroupConditionActionLikeCpp::SetInactive
            };
            actions.push((group.group_id, action));
        }
        actions
    }

    /// C++ `Map::AddObjectToRemoveList` represented over canonical map records.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2547-2555` asserts same map/instance, marks destroyed, runs
    ///   `CleanupsBeforeDelete(false)`, and inserts into `i_objectsToRemove`.
    /// - `Object.cpp:1826-1835` delegates `WorldObject::AddObjectToRemoveList` to
    ///   the owning map when present.
    ///
    /// Divergence note: the C++ `std::set` insert is deduplicated, but the
    /// cleanup call happens before insertion; this Rust seam preserves that order
    /// and reports `duplicate=true` while still incrementing represented cleanup.
    pub fn add_object_to_remove_list_like_cpp(
        &mut self,
        guid: ObjectGuid,
    ) -> AddObjectToRemoveListOutcomeLikeCpp {
        let Some(record) = self.map_objects.get_mut(&guid) else {
            return AddObjectToRemoveListOutcomeLikeCpp {
                guid,
                queued: false,
                duplicate: false,
                missing_or_stale: true,
                unsupported_kind: None,
                cleanup_before_delete_count: 0,
            };
        };

        let kind = record.kind();
        debug_assert_eq!(record.object().map_id(), self.map_id);
        debug_assert_eq!(record.object().instance_id(), self.instance_id);

        let cleanup_before_delete_count =
            cleanup_map_object_record_before_delete_like_cpp(record, kind, false);
        let inserted = self.objects_to_remove.insert(guid);
        AddObjectToRemoveListOutcomeLikeCpp {
            guid,
            queued: inserted,
            duplicate: !inserted,
            missing_or_stale: false,
            unsupported_kind: remove_list_grid_kind_like_cpp(kind)
                .is_none()
                .then_some(kind),
            cleanup_before_delete_count,
        }
    }

    /// C++ `WorldObject::SetWorldObject(bool)` facade owned by `Map` over the
    /// canonical `MapObjectRecord` store.
    ///
    /// C++ anchors:
    /// - `Object.cpp:910-916` returns when `!IsInWorld()`, otherwise delegates
    ///   to the owning map's `AddObjectToSwitchList(this, on)`.
    /// - `Map.cpp:2557-2572` keeps Unit validation/queue duplicate semantics in
    ///   `add_object_to_switch_list_like_cpp`; this facade does not move grid
    ///   containers or mutate temporary world-object state.
    pub fn set_world_object_like_cpp(
        &mut self,
        guid: ObjectGuid,
        on: bool,
    ) -> SetWorldObjectOutcomeLikeCpp {
        let Some(record) = self.map_object_record(guid) else {
            return SetWorldObjectOutcomeLikeCpp {
                guid,
                on,
                status: SetWorldObjectStatusLikeCpp::MissingOrStale,
            };
        };

        if !record.object().object().is_in_world() {
            return SetWorldObjectOutcomeLikeCpp {
                guid,
                on,
                status: SetWorldObjectStatusLikeCpp::NotInWorld,
            };
        }

        let delegated = self.add_object_to_switch_list_like_cpp(guid, on);
        SetWorldObjectOutcomeLikeCpp {
            guid,
            on,
            status: SetWorldObjectStatusLikeCpp::Delegated(delegated.status),
        }
    }

    /// Applies the request emitted by C++-shaped Unit shared-vision transitions
    /// to this map-owned `WorldObject::SetWorldObject(bool)` facade.
    ///
    /// C++ anchors:
    /// - `Unit.cpp:6489-6509` emits `SetWorldObject(true/false)` only at the
    ///   empty/non-empty shared-vision boundary.
    /// - `Object.cpp:910-916` keeps the in-world guard before map delegation.
    /// - `Map.cpp:2557-2572` owns switch-list validation/queue semantics, while
    ///   `Map.cpp:2574-2594` drains later.
    ///
    /// Ownership stays one-way: Unit emits a DTO, the map owner applies it over
    /// canonical `map_objects`/`objects_to_switch`; this method does not run the
    /// drain, rebuild missing records, fan out visibility, or wire sessions.
    pub fn apply_unit_shared_vision_set_world_object_request_like_cpp(
        &mut self,
        request: UnitSharedVisionSetWorldObjectRequestLikeCpp,
    ) -> SetWorldObjectOutcomeLikeCpp {
        self.set_world_object_like_cpp(request.unit_guid, request.on)
    }

    fn player_set_viewpoint_outcome_like_cpp(
        player_guid: ObjectGuid,
        target_guid: ObjectGuid,
        apply: bool,
        status: PlayerSetViewpointStatusLikeCpp,
        set_world_object: Option<SetWorldObjectOutcomeLikeCpp>,
        update_visibility_requested: bool,
        set_seer_requested: bool,
    ) -> PlayerSetViewpointOutcomeLikeCpp {
        PlayerSetViewpointOutcomeLikeCpp {
            player_guid,
            target_guid,
            apply,
            status,
            set_world_object,
            update_visibility_requested,
            set_seer_requested,
        }
    }

    fn map_record_unit_mut_like_cpp(record: &mut MapObjectRecord) -> Option<&mut Unit> {
        match record.kind() {
            AccessorObjectKind::Creature => record.creature_mut().map(Creature::unit_mut),
            AccessorObjectKind::Pet => record.pet_mut().map(|pet| pet.creature_mut().unit_mut()),
            _ => None,
        }
    }

    /// Bounded map-owned seam for the Unit-target shared-vision branch of C++
    /// `Player::SetViewpoint(WorldObject* target, bool apply)`.
    ///
    /// C++ anchors:
    /// - `Player.cpp:25344-25387` owns FarsightObject guards/mutations,
    ///   requests `UpdateVisibilityOf`, calls `Unit::Add/RemovePlayerToVision`
    ///   only for Unit targets that are not `GetVehicleBase()`, and requests
    ///   `SetSeer`.
    /// - `Unit.cpp:6489-6509` toggles Unit active state and emits
    ///   `SetWorldObject(true/false)` only at shared-vision empty boundaries.
    /// - `Object.cpp:910-916` / `Map.cpp:2557-2594` keep the SetWorldObject
    ///   map-owned switch-list enqueue/drain split.
    ///
    /// Scope: this helper mutates only canonical `Map::map_objects` typed Player
    /// and typed Creature/Pet Unit targets already in this same map. It consumes
    /// the Unit-emitted SetWorldObject DTO immediately through the Map facade, but
    /// does not drain queues, fan out visibility, implement `SetSeer`, access
    /// ObjectAccessor/session mirrors, create records, send packets, or touch DB.
    pub fn apply_player_set_viewpoint_unit_like_cpp(
        &mut self,
        player_guid: ObjectGuid,
        target_guid: ObjectGuid,
        apply: bool,
        vehicle_base_guid: Option<ObjectGuid>,
    ) -> PlayerSetViewpointOutcomeLikeCpp {
        let Some(player) = self.get_typed_player(player_guid) else {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                PlayerSetViewpointStatusLikeCpp::MissingPlayer,
                None,
                false,
                false,
            );
        };

        let current_farsight = player.active_data().farsight_object;
        if apply {
            if !current_farsight.is_empty() {
                return Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    target_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::AlreadyHasViewpoint,
                    None,
                    false,
                    false,
                );
            }
        } else if current_farsight != target_guid {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                PlayerSetViewpointStatusLikeCpp::ViewpointMismatch,
                None,
                false,
                false,
            );
        }

        let Some(target_record) = self.map_object_record(target_guid) else {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                PlayerSetViewpointStatusLikeCpp::MissingTarget,
                None,
                false,
                false,
            );
        };
        if !matches!(
            target_record.kind(),
            AccessorObjectKind::Creature | AccessorObjectKind::Pet
        ) {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                PlayerSetViewpointStatusLikeCpp::TargetNotUnit,
                None,
                false,
                false,
            );
        }

        let vehicle_base_skip = vehicle_base_guid == Some(target_guid);
        if !vehicle_base_skip {
            let Some(target_record) = self.map_objects.get_mut(&target_guid) else {
                return Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    target_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::MissingTarget,
                    None,
                    false,
                    false,
                );
            };
            if Self::map_record_unit_mut_like_cpp(target_record).is_none() {
                return Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    target_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::TargetNotUnit,
                    None,
                    false,
                    false,
                );
            }
        }

        let Some(player) = self.get_typed_player_mut(player_guid) else {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                PlayerSetViewpointStatusLikeCpp::MissingPlayer,
                None,
                false,
                false,
            );
        };
        player.set_farsight_object_like_cpp(if apply {
            target_guid
        } else {
            ObjectGuid::EMPTY
        });

        if vehicle_base_skip {
            return Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                target_guid,
                apply,
                if apply {
                    PlayerSetViewpointStatusLikeCpp::Applied
                } else {
                    PlayerSetViewpointStatusLikeCpp::Removed
                },
                None,
                apply,
                true,
            );
        }

        let request = {
            let Some(target_record) = self.map_objects.get_mut(&target_guid) else {
                return Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    target_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::MissingTarget,
                    None,
                    false,
                    false,
                );
            };
            let Some(target_unit) = Self::map_record_unit_mut_like_cpp(target_record) else {
                return Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    target_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::TargetNotUnit,
                    None,
                    false,
                    false,
                );
            };
            if apply {
                target_unit.add_player_to_vision_like_cpp(player_guid)
            } else {
                target_unit.remove_player_from_vision_like_cpp(player_guid)
            }
            .set_world_object
        };
        let set_world_object = request.map(|request| {
            self.apply_unit_shared_vision_set_world_object_request_like_cpp(request)
        });

        Self::player_set_viewpoint_outcome_like_cpp(
            player_guid,
            target_guid,
            apply,
            if apply {
                PlayerSetViewpointStatusLikeCpp::Applied
            } else {
                PlayerSetViewpointStatusLikeCpp::Removed
            },
            set_world_object,
            apply,
            true,
        )
    }

    /// Map-owned seam for C++ `Spell::EffectAddFarsight` ->
    /// `DynamicObject::CreateDynamicObject` -> `SetDuration` ->
    /// `SetCasterViewpoint`.
    ///
    /// C++ anchors:
    /// - `SpellEffects.cpp:2237-2261` runs only after HIT handling has selected
    ///   a Player caster, returns if the Player is not in world, creates
    ///   `DynamicObject(true)`, calls `CreateDynamicObject`, then sets duration
    ///   and caster viewpoint.
    /// - `DynamicObject.cpp:84-133` binds the object to the caster map, validates
    ///   the destination, creates a world-object GUID from map/spell/low guid,
    ///   inherits phase, sets entry/scale/update fields, marks world objects
    ///   active before AddToMap, and inserts through `Map::AddToMap`.
    /// - `DynamicObject.cpp:209-239` resolves the already-bound caster pointer for
    ///   `SetCasterViewpoint`; Rust represents that by `DynamicObject::bound_caster()`
    ///   and delegates to `apply_dynamic_object_caster_viewpoint_like_cpp`.
    ///
    /// Ownership: source-of-truth is this `Map::map_objects` for both the caster
    /// Player and the newly-created DynamicObject. Per #NEXT.R8.ENTITIES.428
    /// invariants, represented fallback paths validate all rejectable inputs before
    /// low-guid consumption so a missing/wrong caster or invalid destination leaves
    /// the Map seam unmutated; this is an explicitly bounded creation-seam guard even
    /// though C++ receives `guidlow` before `CreateDynamicObject` validates `pos`.
    /// This does not parse live Spell targets, create dummy records, register through
    /// ObjectAccessor, implement transport passenger offsets, UpdatePositionData,
    /// ZoneScript, aura/update lifecycle, real SetSeer/fanout, packets/session mirrors,
    /// DB, or spell handler wiring.
    #[allow(clippy::too_many_arguments)]
    pub fn create_farsight_dynamic_object_like_cpp(
        &mut self,
        caster_player_guid: ObjectGuid,
        spell_id: u32,
        spell_x_spell_visual_id: i32,
        dest: Position,
        radius: f32,
        duration_ms: i32,
        cast_time_ms: u64,
        realm_id: u16,
        server_id: u32,
    ) -> FarsightDynamicObjectCreateOutcomeLikeCpp {
        let early = |status| FarsightDynamicObjectCreateOutcomeLikeCpp {
            status,
            caster_player_guid,
            dynamic_object_guid: None,
            low_guid: None,
            add_to_map: None,
            caster_viewpoint: None,
        };

        let Some(caster_player) = self.get_typed_player(caster_player_guid) else {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::MissingCasterPlayer);
        };
        let caster_world = caster_player.unit().world();
        if !caster_world.object().is_in_world() {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::CasterNotInWorld);
        }
        if caster_world.map_id() != self.map_id || caster_world.instance_id() != self.instance_id {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::CasterWrongMap);
        }
        if !dest.is_valid_map_coord_like_cpp() {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::InvalidDestination);
        }
        if self.map_id > 0x1FFF {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::MapIdNotRepresentableInGuid);
        }
        let Ok(spell_id_i32) = i32::try_from(spell_id) else {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::SpellIdNotRepresentable);
        };
        let Ok(cast_time_ms_u32) = u32::try_from(cast_time_ms) else {
            return early(FarsightDynamicObjectCreateStatusLikeCpp::CastTimeNotRepresentable);
        };
        let inherited_phase_shift = caster_world.phase_shift().clone();
        let inherited_suppressed_phase_shift = caster_world.suppressed_phase_shift().clone();

        let low_guid = match self.generate_low_guid_like_cpp(HighGuid::DynamicObject) {
            Ok(low_guid) => low_guid,
            Err(error) => {
                return early(FarsightDynamicObjectCreateStatusLikeCpp::GuidSequenceError(
                    error,
                ));
            }
        };
        let dynamic_object_guid = ObjectGuid::create_world_object(
            HighGuid::DynamicObject,
            0,
            realm_id,
            self.map_id as u16,
            server_id,
            spell_id,
            low_guid,
        );

        let mut dynamic_object = DynamicObject::new(true);
        dynamic_object
            .world_mut()
            .object_mut()
            .create(dynamic_object_guid);
        if dynamic_object
            .world_mut()
            .set_map(self.map_id, self.instance_id)
            .is_err()
        {
            return FarsightDynamicObjectCreateOutcomeLikeCpp {
                status: FarsightDynamicObjectCreateStatusLikeCpp::DynamicObjectRecordError(
                    ObjectAccessorError::ObjectHasNoMap {
                        guid: dynamic_object_guid,
                    },
                ),
                caster_player_guid,
                dynamic_object_guid: Some(dynamic_object_guid),
                low_guid: Some(low_guid),
                add_to_map: None,
                caster_viewpoint: None,
            };
        }
        dynamic_object.world_mut().relocate(dest);
        *dynamic_object.world_mut().phase_shift_mut() = inherited_phase_shift;
        *dynamic_object.world_mut().suppressed_phase_shift_mut() = inherited_suppressed_phase_shift;
        dynamic_object.world_mut().object_mut().set_entry(spell_id);
        dynamic_object.world_mut().object_mut().set_scale(1.0);
        dynamic_object.set_caster_guid(caster_player_guid);
        dynamic_object.set_dynamic_object_type(DynamicObjectType::FarsightFocus);
        dynamic_object.set_spell_visual_id(spell_x_spell_visual_id);
        dynamic_object.set_spell_id(spell_id_i32);
        dynamic_object.set_radius(radius);
        dynamic_object.set_cast_time_ms(cast_time_ms_u32);
        dynamic_object.bind_to_caster(caster_player_guid);
        dynamic_object.set_duration(duration_ms);
        if dynamic_object.world().is_world_object() {
            dynamic_object.world_mut().set_active(true);
        }

        let record = match MapObjectRecord::new_dynamic_object(dynamic_object) {
            Ok(record) => record,
            Err(error) => {
                return FarsightDynamicObjectCreateOutcomeLikeCpp {
                    status: FarsightDynamicObjectCreateStatusLikeCpp::DynamicObjectRecordError(
                        error,
                    ),
                    caster_player_guid,
                    dynamic_object_guid: Some(dynamic_object_guid),
                    low_guid: Some(low_guid),
                    add_to_map: None,
                    caster_viewpoint: None,
                };
            }
        };
        let add_to_map = match self.add_map_object_record_to_map_like_cpp(record) {
            Ok(outcome) => outcome,
            Err(_error) => {
                return FarsightDynamicObjectCreateOutcomeLikeCpp {
                    status: FarsightDynamicObjectCreateStatusLikeCpp::AddToMapError,
                    caster_player_guid,
                    dynamic_object_guid: Some(dynamic_object_guid),
                    low_guid: Some(low_guid),
                    add_to_map: None,
                    caster_viewpoint: None,
                };
            }
        };
        let caster_viewpoint =
            self.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, true);

        FarsightDynamicObjectCreateOutcomeLikeCpp {
            status: FarsightDynamicObjectCreateStatusLikeCpp::Created,
            caster_player_guid,
            dynamic_object_guid: Some(dynamic_object_guid),
            low_guid: Some(low_guid),
            add_to_map: Some(add_to_map),
            caster_viewpoint: Some(caster_viewpoint),
        }
    }

    /// Map-owned seam for the non-aura branch of C++ `DynamicObject::Update`.
    ///
    /// C++ anchors:
    /// - `DynamicObject.cpp:136-165` asserts same-map caster, updates aura-bound
    ///   DynamicObjects through the aura path (unsupported here), otherwise
    ///   decrements `_duration` by `p_time` or marks expired, then calls `Remove()`
    ///   on expiry and `sScriptMgr->OnDynamicObjectUpdate` otherwise.
    /// - `DynamicObject.cpp:167-171` makes `Remove()` enqueue through
    ///   `AddObjectToRemoveList()` only when the object is in world.
    /// - `Map.cpp:2547-2555` owns `AddObjectToRemoveList` cleanup and deferred
    ///   remove-list insertion, represented by `add_object_to_remove_list_like_cpp`.
    ///
    /// Ownership: source-of-truth is canonical `Map::map_objects`; this helper
    /// mutates only the typed `MapObjectRecord::DynamicObject` duration and, after
    /// dropping that mutable borrow, enqueues the same GUID through the existing
    /// remove-list facade. Aura-bound DynamicObjects only record represented
    /// `Aura::UpdateOwner` evidence and removed/expired checks. It does not drain removal, run scripts,
    /// write ObjectAccessor/session mirrors, fan out visibility, send packets, or
    /// create fallback records.
    pub fn update_dynamic_object_like_cpp(
        &mut self,
        dynamic_object_guid: ObjectGuid,
        elapsed_ms: u32,
    ) -> DynamicObjectUpdateOutcomeLikeCpp {
        let Some(record) = self.map_object_record(dynamic_object_guid) else {
            return DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::MissingDynamicObject,
                duration_before_ms: None,
                duration_after_ms: None,
                aura_update_owner_calls_before: None,
                aura_update_owner_calls_after: None,
                script_update_would_run: false,
                remove_list: None,
            };
        };

        if record.kind() != AccessorObjectKind::DynamicObject {
            return DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::NotDynamicObject,
                duration_before_ms: None,
                duration_after_ms: None,
                aura_update_owner_calls_before: None,
                aura_update_owner_calls_after: None,
                script_update_would_run: false,
                remove_list: None,
            };
        }

        let Some(dynamic_object) = record.dynamic_object() else {
            return DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::NotDynamicObject,
                duration_before_ms: None,
                duration_after_ms: None,
                aura_update_owner_calls_before: None,
                aura_update_owner_calls_after: None,
                script_update_would_run: false,
                remove_list: None,
            };
        };

        let duration_before_ms = dynamic_object.duration_ms();
        let aura_update_owner_calls_before = dynamic_object.represented_aura_update_owner_count();
        if !dynamic_object.world().object().is_in_world() {
            return DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::NotInWorld,
                duration_before_ms: Some(duration_before_ms),
                duration_after_ms: Some(duration_before_ms),
                aura_update_owner_calls_before: Some(aura_update_owner_calls_before),
                aura_update_owner_calls_after: Some(aura_update_owner_calls_before),
                script_update_would_run: false,
                remove_list: None,
            };
        }

        let aura_bound_before = dynamic_object.has_aura();

        let (expired, duration_after_ms, aura_update_owner_calls_after) = {
            let Some(record) = self.map_objects.get_mut(&dynamic_object_guid) else {
                return DynamicObjectUpdateOutcomeLikeCpp {
                    dynamic_object_guid,
                    elapsed_ms,
                    status: DynamicObjectUpdateStatusLikeCpp::MissingDynamicObject,
                    duration_before_ms: Some(duration_before_ms),
                    duration_after_ms: Some(duration_before_ms),
                    aura_update_owner_calls_before: Some(aura_update_owner_calls_before),
                    aura_update_owner_calls_after: Some(aura_update_owner_calls_before),
                    script_update_would_run: false,
                    remove_list: None,
                };
            };
            let Some(dynamic_object) = record.dynamic_object_mut() else {
                return DynamicObjectUpdateOutcomeLikeCpp {
                    dynamic_object_guid,
                    elapsed_ms,
                    status: DynamicObjectUpdateStatusLikeCpp::NotDynamicObject,
                    duration_before_ms: Some(duration_before_ms),
                    duration_after_ms: Some(duration_before_ms),
                    aura_update_owner_calls_before: Some(aura_update_owner_calls_before),
                    aura_update_owner_calls_after: Some(aura_update_owner_calls_before),
                    script_update_would_run: false,
                    remove_list: None,
                };
            };
            let expired = if aura_bound_before {
                dynamic_object.update_aura_bound_like_cpp(elapsed_ms)
            } else {
                dynamic_object.update_non_aura_duration(elapsed_ms)
            };
            (
                expired,
                dynamic_object.duration_ms(),
                dynamic_object.represented_aura_update_owner_count(),
            )
        };

        if expired {
            let remove_list = self.add_object_to_remove_list_like_cpp(dynamic_object_guid);
            DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::ExpiredRemoveQueued,
                duration_before_ms: Some(duration_before_ms),
                duration_after_ms: Some(duration_after_ms),
                aura_update_owner_calls_before: Some(aura_update_owner_calls_before),
                aura_update_owner_calls_after: Some(aura_update_owner_calls_after),
                script_update_would_run: false,
                remove_list: Some(remove_list),
            }
        } else {
            DynamicObjectUpdateOutcomeLikeCpp {
                dynamic_object_guid,
                elapsed_ms,
                status: DynamicObjectUpdateStatusLikeCpp::Updated,
                duration_before_ms: Some(duration_before_ms),
                duration_after_ms: Some(duration_after_ms),
                aura_update_owner_calls_before: Some(aura_update_owner_calls_before),
                aura_update_owner_calls_after: Some(aura_update_owner_calls_after),
                script_update_would_run: true,
                remove_list: None,
            }
        }
    }

    /// Bounded map-owned caller-consumption seam for C++
    /// `DynamicObject::SetCasterViewpoint` / `RemoveCasterViewpoint`.
    ///
    /// C++ anchors:
    /// - `DynamicObject.cpp:209-225` resolves the caster from the DynamicObject's
    ///   `_caster`, calls `Player::SetViewpoint(this, apply)` only when `_caster`
    ///   is a Player, and then toggles `_isViewpoint` without checking the Player
    ///   helper's early-return result.
    /// - `DynamicObject.cpp:233-239` represents `_caster` as a previously bound
    ///   same-map Unit pointer; this helper consumes `DynamicObject::bound_caster()`
    ///   as that represented pointer equivalent and never falls back to the raw
    ///   caster GUID field or to a caller-provided Player.
    /// - `Player.cpp:25344-25387` owns FarsightObject guards/mutations,
    ///   `UpdateVisibilityOf` on apply, and `SetSeer`; DynamicObject targets do
    ///   not run the Unit shared-vision / SetWorldObject branch.
    ///
    /// Ownership: source-of-truth is canonical `Map::map_objects`. The helper
    /// first validates the typed DynamicObject record, then resolves the Player
    /// from `DynamicObject::bound_caster()` before any Player mutation. It does
    /// not create records, silently fall back from `caster_guid`, drain
    /// switch/remove lists, fan out visibility, implement full SetSeer, write
    /// session/ObjectAccessor mirrors, send packets, or touch DB.
    pub fn apply_dynamic_object_caster_viewpoint_like_cpp(
        &mut self,
        dynamic_object_guid: ObjectGuid,
        apply: bool,
    ) -> DynamicObjectCasterViewpointOutcomeLikeCpp {
        let outcome =
            |player_guid, status, player_set_viewpoint, dynamic_object_viewpoint_toggled| {
                DynamicObjectCasterViewpointOutcomeLikeCpp {
                    player_guid,
                    dynamic_object_guid,
                    apply,
                    status,
                    player_set_viewpoint,
                    dynamic_object_viewpoint_toggled,
                }
            };
        let player_outcome = |player_guid, status| {
            Self::player_set_viewpoint_outcome_like_cpp(
                player_guid,
                dynamic_object_guid,
                apply,
                status,
                None,
                false,
                false,
            )
        };

        let Some(dynamic_object) = self
            .map_object_record(dynamic_object_guid)
            .and_then(MapObjectRecord::dynamic_object)
        else {
            return outcome(
                ObjectGuid::EMPTY,
                DynamicObjectCasterViewpointStatusLikeCpp::MissingDynamicObject,
                player_outcome(
                    ObjectGuid::EMPTY,
                    PlayerSetViewpointStatusLikeCpp::MissingTarget,
                ),
                false,
            );
        };

        let Some(player_guid) = dynamic_object.bound_caster() else {
            return outcome(
                ObjectGuid::EMPTY,
                DynamicObjectCasterViewpointStatusLikeCpp::MissingCaster,
                player_outcome(
                    ObjectGuid::EMPTY,
                    PlayerSetViewpointStatusLikeCpp::MissingPlayer,
                ),
                false,
            );
        };

        let Some(player) = self.get_typed_player(player_guid) else {
            return outcome(
                player_guid,
                DynamicObjectCasterViewpointStatusLikeCpp::CasterNotPlayer,
                player_outcome(player_guid, PlayerSetViewpointStatusLikeCpp::MissingPlayer),
                false,
            );
        };
        let current_farsight = player.active_data().farsight_object;

        let player_set_viewpoint = if apply {
            if current_farsight.is_empty() {
                if let Some(player) = self.get_typed_player_mut(player_guid) {
                    player.set_farsight_object_like_cpp(dynamic_object_guid);
                    Self::player_set_viewpoint_outcome_like_cpp(
                        player_guid,
                        dynamic_object_guid,
                        apply,
                        PlayerSetViewpointStatusLikeCpp::Applied,
                        None,
                        true,
                        true,
                    )
                } else {
                    player_outcome(player_guid, PlayerSetViewpointStatusLikeCpp::MissingPlayer)
                }
            } else {
                player_outcome(
                    player_guid,
                    PlayerSetViewpointStatusLikeCpp::AlreadyHasViewpoint,
                )
            }
        } else if current_farsight == dynamic_object_guid {
            if let Some(player) = self.get_typed_player_mut(player_guid) {
                player.set_farsight_object_like_cpp(ObjectGuid::EMPTY);
                Self::player_set_viewpoint_outcome_like_cpp(
                    player_guid,
                    dynamic_object_guid,
                    apply,
                    PlayerSetViewpointStatusLikeCpp::Removed,
                    None,
                    false,
                    true,
                )
            } else {
                player_outcome(player_guid, PlayerSetViewpointStatusLikeCpp::MissingPlayer)
            }
        } else {
            player_outcome(
                player_guid,
                PlayerSetViewpointStatusLikeCpp::ViewpointMismatch,
            )
        };

        let mut dynamic_object_viewpoint_toggled = false;
        if let Some(record) = self.map_objects.get_mut(&dynamic_object_guid) {
            if let Some(dynamic_object) = record.dynamic_object_mut() {
                if apply {
                    dynamic_object.set_caster_viewpoint();
                } else {
                    dynamic_object.remove_caster_viewpoint();
                }
                dynamic_object_viewpoint_toggled = true;
            }
        }

        outcome(
            player_guid,
            DynamicObjectCasterViewpointStatusLikeCpp::CasterPlayerResolved,
            player_set_viewpoint,
            dynamic_object_viewpoint_toggled,
        )
    }

    /// C++ `Map::AddObjectToSwitchList` represented over canonical map records.
    ///
    /// C++ anchors:
    /// - `Map.h:345-346` declares `AddObjectToRemoveList` beside
    ///   `AddObjectToSwitchList`; `Map.h:651-652` owns both queues.
    /// - `Map.cpp:2557-2572` accepts only `TYPEID_UNIT`, inserts first toggle,
    ///   cancels an opposite pending toggle, and aborts on duplicate direction.
    /// - `Object.cpp:910-915` shows `WorldObject::SetWorldObject(on)` enqueues
    ///   through the owning map only when the object is already in world.
    pub fn add_object_to_switch_list_like_cpp(
        &mut self,
        guid: ObjectGuid,
        on: bool,
    ) -> AddObjectToSwitchListOutcomeLikeCpp {
        let Some(record) = self.map_object_record(guid) else {
            return AddObjectToSwitchListOutcomeLikeCpp {
                guid,
                on,
                status: AddObjectToSwitchListStatusLikeCpp::MissingOrStale,
            };
        };

        debug_assert_eq!(record.object().map_id(), self.map_id);
        debug_assert_eq!(record.object().instance_id(), self.instance_id);

        if !switch_list_unit_kind_like_cpp(record.kind()) {
            return AddObjectToSwitchListOutcomeLikeCpp {
                guid,
                on,
                status: AddObjectToSwitchListStatusLikeCpp::IgnoredNonUnit,
            };
        }

        match self.objects_to_switch.get(&guid).copied() {
            None => {
                self.objects_to_switch.insert(guid, on);
                AddObjectToSwitchListOutcomeLikeCpp {
                    guid,
                    on,
                    status: AddObjectToSwitchListStatusLikeCpp::Queued,
                }
            }
            Some(existing) if existing != on => {
                self.objects_to_switch.remove(&guid);
                AddObjectToSwitchListOutcomeLikeCpp {
                    guid,
                    on,
                    status: AddObjectToSwitchListStatusLikeCpp::CancelledOppositeToggle,
                }
            }
            Some(_) => AddObjectToSwitchListOutcomeLikeCpp {
                guid,
                on,
                status: AddObjectToSwitchListStatusLikeCpp::DuplicateSameDirectionAbort,
            },
        }
    }

    /// C++ `Map::RemoveAllObjectsInRemoveList` physical map-local drain.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2574-2594` drains `i_objectsToSwitch` first and calls
    ///   `SwitchGridContainers<Creature>` for non-permanent Unit objects.
    /// - `Map.cpp:2596-2646` then drains `i_objectsToRemove`; supported grid
    ///   object types call `RemoveFromMap(..., true)`, Creature runs a second
    ///   `CleanupsBeforeDelete()` immediately before removal, and non-grid types
    ///   are logged/ignored.
    /// - `Map.cpp:933-951` shows `RemoveFromMap(T*, true)` does the physical map
    ///   removal/reset/delete path.
    pub fn remove_all_objects_in_remove_list_like_cpp(
        &mut self,
    ) -> RemoveAllObjectsInRemoveListOutcomeLikeCpp {
        let mut switches = self.objects_to_switch.drain().collect::<Vec<_>>();
        switches.sort_by_key(|(guid, _)| guid.to_raw_bytes());
        let guids = self.objects_to_remove.drain().collect::<Vec<_>>();
        let mut outcome = RemoveAllObjectsInRemoveListOutcomeLikeCpp {
            switch_processed: switches.len(),
            processed: guids.len(),
            ..Default::default()
        };

        for (guid, on) in switches {
            let switch = self.switch_grid_containers_like_cpp(guid, on);
            if switch.executed {
                outcome.switch_executed += 1;
            } else if switch.missing_or_stale {
                outcome.switch_missing_or_stale += 1;
            } else if switch.unsupported_kind {
                outcome.switch_unsupported_kinds += 1;
            } else if switch.permanent_world_object {
                outcome.switch_permanent_world_objects += 1;
            } else if switch.invalid_or_unloaded_grid {
                outcome.switch_invalid_or_unloaded_grid += 1;
            }
        }

        for guid in guids {
            let Some(kind) = self.map_object_record(guid).map(MapObjectRecord::kind) else {
                outcome.missing_or_stale += 1;
                continue;
            };

            if remove_list_grid_kind_like_cpp(kind).is_none() {
                outcome.unsupported_kinds += 1;
                continue;
            }

            if matches!(kind, AccessorObjectKind::Creature | AccessorObjectKind::Pet) {
                if let Some(record) = self.map_objects.get_mut(&guid) {
                    outcome.creature_second_cleanup_count +=
                        cleanup_map_object_record_before_delete_like_cpp(record, kind, true);
                }
            }

            match self.remove_from_map_like_cpp(guid, true) {
                Ok(removed) => {
                    outcome.removed += 1;
                    if let Some(cleanup) = removed.dynamic_object_remove_cleanup {
                        if cleanup.removed_aura_pending_delete {
                            outcome.dynamic_object_remove_aura_cleanup_count += 1;
                        }
                        if cleanup.unbound_caster.is_some() {
                            outcome.dynamic_object_unbound_caster_count += 1;
                        }
                    }
                }
                Err(RemoveFromMapError::ObjectNotFound { .. }) => outcome.missing_or_stale += 1,
                Err(_) => outcome.remove_errors += 1,
            }
        }

        outcome
    }

    /// C++ `Map::SwitchGridContainers<Creature>` represented for Creature/Pet.
    ///
    /// C++ anchors:
    /// - `Map.cpp:260-305` computes the current cell, returns on invalid coords or
    ///   unloaded grid, moves Unit GUID between `grid_objects.creatures` and
    ///   `world_objects.creatures`, then writes `Creature::m_isTempWorldObject`.
    /// - `Object.cpp:918-925` makes `WorldObject::IsWorldObject` true for a
    ///   Creature with `m_isTempWorldObject`, while `Object.h:723-724` keeps
    ///   permanent world-object state in base `m_isWorldObject`.
    fn switch_grid_containers_like_cpp(
        &mut self,
        guid: ObjectGuid,
        on: bool,
    ) -> SwitchGridContainersOutcomeLikeCpp {
        let Some(record) = self.map_object_record(guid) else {
            return SwitchGridContainersOutcomeLikeCpp::missing_or_stale();
        };
        let kind = record.kind();
        if !switch_list_unit_kind_like_cpp(kind) {
            return SwitchGridContainersOutcomeLikeCpp::unsupported_kind();
        }
        if record.object().is_world_object() {
            return SwitchGridContainersOutcomeLikeCpp::permanent_world_object();
        }

        let position = record.object().position();
        if !is_valid_map_coord_2d(position.x, position.y) {
            return SwitchGridContainersOutcomeLikeCpp::invalid_or_unloaded_grid();
        }

        let cell = Cell::from_world(position.x, position.y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        if !self.is_grid_loaded(grid) {
            return SwitchGridContainersOutcomeLikeCpp::invalid_or_unloaded_grid();
        }

        let Some(ngrid) = self.get_ngrid_mut(grid) else {
            return SwitchGridContainersOutcomeLikeCpp::invalid_or_unloaded_grid();
        };
        let Some(local_cell) = ngrid.get_grid_type_mut(cell.cell_x(), cell.cell_y()) else {
            return SwitchGridContainersOutcomeLikeCpp::invalid_or_unloaded_grid();
        };

        if on {
            local_cell.grid_objects.creatures.remove(&guid);
            local_cell.world_objects.creatures.insert(guid);
        } else {
            local_cell.world_objects.creatures.remove(&guid);
            local_cell.grid_objects.creatures.insert(guid);
        }

        if let Some(record) = self.map_objects.get_mut(&guid) {
            set_record_temp_world_object_like_cpp(record, on);
        }

        SwitchGridContainersOutcomeLikeCpp::executed()
    }

    /// C++ `Map::DespawnAll` represented over map-local by-spawn indexes.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2034-2055` snapshots Creature/GameObject by-spawn stores and
    ///   queues each object through `AddObjectToRemoveList`.
    /// - `Map.cpp:2547-2555` marks each queued object destroyed and runs cleanup
    ///   before insertion into the map-owned remove-list.
    /// - `Map.cpp:2574-2646` later physically drains the list.
    pub fn despawn_all_by_spawn_id_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> DespawnAllBySpawnIdOutcomeLikeCpp {
        let mut outcome = DespawnAllBySpawnIdOutcomeLikeCpp {
            object_type,
            spawn_id,
            queued: 0,
            removed: 0,
            duplicates: 0,
            stale_index_entries: 0,
            remove_errors: 0,
            unsupported_live_despawn_type: 0,
        };

        let guids = match object_type {
            SpawnObjectType::Creature => self.creature_spawn_id_store_guids_like_cpp(spawn_id),
            SpawnObjectType::GameObject => self.gameobject_spawn_id_store_guids_like_cpp(spawn_id),
            SpawnObjectType::AreaTrigger => {
                outcome.unsupported_live_despawn_type = 1;
                return outcome;
            }
        };

        for guid in guids {
            let still_matches = match object_type {
                SpawnObjectType::Creature => self
                    .map_object_record(guid)
                    .and_then(MapObjectRecord::creature)
                    .is_some_and(|creature| creature.spawn_id() == spawn_id),
                SpawnObjectType::GameObject => self
                    .map_object_record(guid)
                    .and_then(MapObjectRecord::game_object)
                    .is_some_and(|gameobject| gameobject.spawn_id() == spawn_id),
                SpawnObjectType::AreaTrigger => false,
            };
            if !still_matches {
                outcome.stale_index_entries += 1;
                continue;
            }

            let queued = self.add_object_to_remove_list_like_cpp(guid);
            if queued.missing_or_stale {
                outcome.stale_index_entries += 1;
            } else if queued.unsupported_kind.is_some() {
                outcome.unsupported_live_despawn_type += 1;
            } else if queued.duplicate {
                outcome.duplicates += 1;
            } else if queued.queued {
                outcome.queued += 1;
            }
        }

        outcome
    }

    /// C++ `Map::SpawnGroupDespawn(groupId, deleteRespawnTimes)` represented over
    /// map-owned runtime state and caller-supplied ObjectMgr-like `SpawnStore`.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2404-2425` validates existing/non-system group, iterates
    ///   `sObjectMgr->GetSpawnMetadataForGroup`, optionally calls
    ///   `RemoveRespawnTime`, calls `DespawnAll`, then marks the group inactive.
    /// - `Map.cpp:2140-2163` DB delete is owned by callers; this helper only
    ///   mutates map-owned respawn timers so world-server can derive before/after
    ///   `CHAR_DEL_RESPAWN` work outside the lock.
    pub fn spawn_group_despawn_like_cpp(
        &mut self,
        group: Option<&SpawnGroupTemplateData>,
        delete_respawn_times: bool,
        spawn_store: &SpawnStore,
    ) -> SpawnGroupDespawnOutcomeLikeCpp {
        let Some(group) = group else {
            return SpawnGroupDespawnOutcomeLikeCpp::blocked_missing_group(0);
        };
        if group.is_system() {
            return SpawnGroupDespawnOutcomeLikeCpp::blocked_system_group(group.group_id);
        }

        let mut outcome = SpawnGroupDespawnOutcomeLikeCpp::executed(group.group_id);
        if let Some(members) = spawn_store.spawn_group_members(group.group_id) {
            let members = members.iter().copied().collect::<Vec<_>>();
            for member in members {
                let Some(spawn_data) = spawn_store.spawn_data(member.object_type, member.spawn_id)
                else {
                    outcome.metadata_entries += 1;
                    outcome.stale_index_entries += 1;
                    continue;
                };
                if spawn_data.map_id != self.map_id {
                    continue;
                }

                outcome.metadata_entries += 1;
                if delete_respawn_times {
                    match member.object_type {
                        SpawnObjectType::Creature | SpawnObjectType::GameObject => {
                            if self
                                .remove_respawn_time_like_cpp(member.object_type, member.spawn_id)
                                .is_some()
                            {
                                outcome.respawn_timers_removed += 1;
                            } else {
                                outcome.respawn_timers_missing += 1;
                            }
                        }
                        SpawnObjectType::AreaTrigger => {
                            outcome.respawn_timer_unsupported_types += 1;
                        }
                    }
                }

                let despawn =
                    self.despawn_all_by_spawn_id_like_cpp(member.object_type, member.spawn_id);
                outcome.objects_removed += despawn.queued;
                outcome.stale_index_entries += despawn.stale_index_entries;
                outcome.remove_errors += despawn.remove_errors;
                outcome.unsupported_live_despawn_types += despawn.unsupported_live_despawn_type;
            }
        }
        outcome.applied_inactive_change =
            Some(self.set_spawn_group_active_like_cpp(Some(group), false));
        outcome
    }

    /// C++ `Map::SpawnGroupSpawn(groupId, ignoreRespawn, force)` represented as a
    /// safe map-local planning/execution seam over map-owned active state,
    /// respawn timers, and by-spawn live-object indexes.
    ///
    /// C++ anchors:
    /// - `Map.cpp:2315-2324` validates existing/non-system group and marks it
    ///   active before iterating metadata.
    /// - `Map.cpp:2326-2353` iterates ObjectMgr spawn metadata, removes respawn
    ///   timers when forced/ignoring, skips active timers and live objects.
    /// - `Map.cpp:2356-2395` checks difficulty/grid-loaded before calling
    ///   Creature/GameObject/AreaTrigger `LoadFromDB`.
    ///
    /// RustyCore deliberately does not create DB-backed entities here. Loaded-grid
    /// Creature/GameObject work is returned as explicit `load_plans` and counted
    /// as blocked; AreaTrigger live creation is reported unsupported.
    pub fn spawn_group_spawn_like_cpp(
        &mut self,
        group: Option<&SpawnGroupTemplateData>,
        ignore_respawn: bool,
        force: bool,
        spawn_store: &SpawnStore,
    ) -> SpawnGroupSpawnOutcomeLikeCpp {
        let Some(group) = group else {
            return SpawnGroupSpawnOutcomeLikeCpp::blocked_missing_group(0);
        };
        if group.is_system() {
            return SpawnGroupSpawnOutcomeLikeCpp::blocked_system_group(group.group_id);
        }

        let mut outcome = SpawnGroupSpawnOutcomeLikeCpp::executed(group.group_id);
        outcome.applied_active_change =
            Some(self.set_spawn_group_active_like_cpp(Some(group), true));

        if let Some(members) = spawn_store.spawn_group_members(group.group_id) {
            let members = members.iter().copied().collect::<Vec<_>>();
            for member in members {
                let Some(spawn_data) = spawn_store.spawn_data(member.object_type, member.spawn_id)
                else {
                    outcome.stale_index_entries += 1;
                    continue;
                };
                if spawn_data.map_id != self.map_id {
                    continue;
                }

                outcome.metadata_entries += 1;
                match member.object_type {
                    SpawnObjectType::Creature | SpawnObjectType::GameObject => {
                        if force || ignore_respawn {
                            if self
                                .remove_respawn_time_like_cpp(member.object_type, member.spawn_id)
                                .is_some()
                            {
                                outcome.respawn_timers_removed += 1;
                            } else {
                                outcome.respawn_timers_missing += 1;
                            }
                        }

                        if self.get_respawn_time_like_cpp(member.object_type, member.spawn_id) != 0
                        {
                            outcome.skipped_respawn_timer_active += 1;
                            continue;
                        }

                        if !force {
                            let live_blocks = match member.object_type {
                                SpawnObjectType::Creature => self
                                    .get_creature_by_spawn_id_like_cpp(member.spawn_id)
                                    .is_some_and(Creature::is_alive),
                                SpawnObjectType::GameObject => self
                                    .get_gameobject_by_spawn_id_like_cpp(member.spawn_id)
                                    .is_some(),
                                SpawnObjectType::AreaTrigger => false,
                            };
                            if live_blocks {
                                outcome.skipped_live_object_active += 1;
                                continue;
                            }
                        }
                    }
                    SpawnObjectType::AreaTrigger => {
                        outcome.unsupported_spawn_types += 1;
                        continue;
                    }
                }

                if !spawn_data.spawn_difficulties.contains(&self.spawn_mode()) {
                    outcome.skipped_difficulty_mismatch += 1;
                    continue;
                }

                let cell = cell_from_world(spawn_data.spawn_point.x, spawn_data.spawn_point.y);
                let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
                if !self.is_grid_loaded(grid) {
                    outcome.skipped_unloaded_grid += 1;
                    continue;
                }

                outcome.load_plans.push(SpawnGroupSpawnLoadPlanLikeCpp {
                    object_type: member.object_type,
                    spawn_id: member.spawn_id,
                    force,
                });
                match member.object_type {
                    SpawnObjectType::Creature => outcome.blocked_loaded_grid_creature_loads += 1,
                    SpawnObjectType::GameObject => {
                        outcome.blocked_loaded_grid_gameobject_loads += 1
                    }
                    SpawnObjectType::AreaTrigger => outcome.unsupported_spawn_types += 1,
                }
            }
        }

        outcome
    }

    /// C++-shaped `Map::UpdateSpawnGroupConditions` bridge over pre-resolved
    /// templates that executes the complete represented `SetSpawnGroupInactive`
    /// branch, the map-local `SpawnGroupDespawn(..., true)` condition-failure
    /// branch, and the safe map-local `SpawnGroupSpawn` planning branch. Entity
    /// creation remains blocked/planned by `SpawnGroupSpawnOutcomeLikeCpp`.
    pub fn apply_update_spawn_group_conditions_represented_like_cpp<'a, I, F>(
        &mut self,
        groups: I,
        spawn_store: &SpawnStore,
        meets_conditions: F,
    ) -> Vec<SpawnGroupConditionUpdateOutcomeLikeCpp>
    where
        I: IntoIterator<Item = &'a SpawnGroupTemplateData>,
        F: FnMut(&SpawnGroupTemplateData) -> bool,
    {
        let groups = groups.into_iter().collect::<Vec<_>>();
        let planned_actions = self
            .plan_update_spawn_group_conditions_like_cpp(groups.iter().copied(), meets_conditions);

        planned_actions
            .into_iter()
            .zip(groups)
            .map(|((group_id, action), group)| {
                let mut applied_change = None;
                let mut despawn_outcome = None;
                let mut spawn_outcome = None;
                match action {
                    SpawnGroupConditionActionLikeCpp::SetInactive => {
                        applied_change = Some(self.set_spawn_group_inactive_like_cpp(Some(group)));
                    }
                    SpawnGroupConditionActionLikeCpp::Despawn {
                        delete_respawn_times,
                    } => {
                        despawn_outcome = Some(self.spawn_group_despawn_like_cpp(
                            Some(group),
                            delete_respawn_times,
                            spawn_store,
                        ));
                    }
                    SpawnGroupConditionActionLikeCpp::Spawn {
                        ignore_respawn,
                        force,
                    } => {
                        spawn_outcome = Some(self.spawn_group_spawn_like_cpp(
                            Some(group),
                            ignore_respawn,
                            force,
                            spawn_store,
                        ));
                    }
                    SpawnGroupConditionActionLikeCpp::Noop => {}
                }

                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id,
                    action,
                    applied_change,
                    despawn_outcome,
                    spawn_outcome,
                }
            })
            .collect()
    }

    /// Legacy wrapper preserving the pre-#391 SetInactive-only seam for focused
    /// tests/callers that explicitly require planned-only despawn evidence.
    pub fn apply_update_spawn_group_conditions_set_inactive_like_cpp<'a, I, F>(
        &mut self,
        groups: I,
        meets_conditions: F,
    ) -> Vec<SpawnGroupConditionUpdateOutcomeLikeCpp>
    where
        I: IntoIterator<Item = &'a SpawnGroupTemplateData>,
        F: FnMut(&SpawnGroupTemplateData) -> bool,
    {
        let groups = groups.into_iter().collect::<Vec<_>>();
        let planned_actions = self
            .plan_update_spawn_group_conditions_like_cpp(groups.iter().copied(), meets_conditions);

        planned_actions
            .into_iter()
            .zip(groups)
            .map(|((group_id, action), group)| {
                let applied_change = if action == SpawnGroupConditionActionLikeCpp::SetInactive {
                    Some(self.set_spawn_group_inactive_like_cpp(Some(group)))
                } else {
                    None
                };

                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id,
                    action,
                    applied_change,
                    despawn_outcome: None,
                    spawn_outcome: None,
                }
            })
            .collect()
    }

    pub fn map_object_count(&self) -> usize {
        self.map_objects.len()
    }

    pub fn objects_to_remove_count_like_cpp(&self) -> usize {
        self.objects_to_remove.len()
    }

    pub fn objects_to_switch_count_like_cpp(&self) -> usize {
        self.objects_to_switch.len()
    }

    pub fn pending_switch_like_cpp(&self, guid: ObjectGuid) -> Option<bool> {
        self.objects_to_switch.get(&guid).copied()
    }

    #[cfg(test)]
    fn enqueue_object_to_remove_for_test(&mut self, guid: ObjectGuid) {
        self.objects_to_remove.insert(guid);
    }

    #[cfg(test)]
    fn enqueue_object_to_switch_for_test(&mut self, guid: ObjectGuid, on: bool) {
        self.objects_to_switch.insert(guid, on);
    }

    pub fn creature_spawn_id_store_count_like_cpp(&self, spawn_id: SpawnId) -> usize {
        self.creatures_by_spawn_id
            .get(&spawn_id)
            .map_or(0, HashSet::len)
    }

    pub fn gameobject_spawn_id_store_count_like_cpp(&self, spawn_id: SpawnId) -> usize {
        self.gameobjects_by_spawn_id
            .get(&spawn_id)
            .map_or(0, HashSet::len)
    }

    pub fn area_trigger_spawn_id_store_count_like_cpp(&self, spawn_id: SpawnId) -> usize {
        self.area_triggers_by_spawn_id
            .get(&spawn_id)
            .map_or(0, HashSet::len)
    }

    pub fn creature_spawn_id_store_guids_like_cpp(&self, spawn_id: SpawnId) -> Vec<ObjectGuid> {
        self.creatures_by_spawn_id
            .get(&spawn_id)
            .map(|guids| {
                let mut guids: Vec<_> = guids.iter().copied().collect();
                guids.sort();
                guids
            })
            .unwrap_or_default()
    }

    pub fn gameobject_spawn_id_store_guids_like_cpp(&self, spawn_id: SpawnId) -> Vec<ObjectGuid> {
        self.gameobjects_by_spawn_id
            .get(&spawn_id)
            .map(|guids| {
                let mut guids: Vec<_> = guids.iter().copied().collect();
                guids.sort();
                guids
            })
            .unwrap_or_default()
    }

    pub fn area_trigger_spawn_id_store_guids_like_cpp(&self, spawn_id: SpawnId) -> Vec<ObjectGuid> {
        self.area_triggers_by_spawn_id
            .get(&spawn_id)
            .map(|guids| {
                let mut guids: Vec<_> = guids.iter().copied().collect();
                // C++ returns the first unordered_multimap entry; Rust sorts for deterministic tests.
                guids.sort();
                guids
            })
            .unwrap_or_default()
    }

    pub fn get_creature_by_spawn_id_like_cpp(&self, spawn_id: SpawnId) -> Option<&Creature> {
        let mut fallback_guid = None;
        let mut alive_guid = None;
        for guid in self.creature_spawn_id_store_guids_like_cpp(spawn_id) {
            let Some(creature) = self
                .map_object_record(guid)
                .and_then(MapObjectRecord::creature)
            else {
                continue;
            };
            if creature.spawn_id() != spawn_id {
                continue;
            }
            fallback_guid.get_or_insert(guid);
            if creature.is_alive() {
                alive_guid = Some(guid);
                break;
            }
        }

        alive_guid
            .or(fallback_guid)
            .and_then(|guid| self.map_object_record(guid)?.creature())
    }

    pub fn get_gameobject_by_spawn_id_like_cpp(&self, spawn_id: SpawnId) -> Option<&GameObject> {
        let mut fallback_guid = None;
        let mut spawned_guid = None;
        for guid in self.gameobject_spawn_id_store_guids_like_cpp(spawn_id) {
            let Some(gameobject) = self
                .map_object_record(guid)
                .and_then(MapObjectRecord::game_object)
            else {
                continue;
            };
            if gameobject.spawn_id() != spawn_id {
                continue;
            }
            fallback_guid.get_or_insert(guid);
            if Self::gameobject_is_spawned_like_cpp(gameobject) {
                spawned_guid = Some(guid);
                break;
            }
        }

        spawned_guid
            .or(fallback_guid)
            .and_then(|guid| self.map_object_record(guid)?.game_object())
    }

    fn gameobject_is_spawned_like_cpp(gameobject: &GameObject) -> bool {
        gameobject.respawn_delay_time() == 0
            || (gameobject.respawn_time() > 0 && !gameobject.spawned_by_default())
            || (gameobject.respawn_time() == 0 && gameobject.spawned_by_default())
    }

    pub fn get_area_trigger_by_spawn_id_like_cpp(&self, spawn_id: SpawnId) -> Option<&AreaTrigger> {
        self.area_trigger_spawn_id_store_guids_like_cpp(spawn_id)
            .into_iter()
            .find_map(|guid| self.map_object_record(guid)?.area_trigger())
    }

    pub fn get_world_object_by_spawn_id_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<&WorldObject> {
        match object_type {
            SpawnObjectType::Creature => self
                .get_creature_by_spawn_id_like_cpp(spawn_id)
                .map(|creature| creature.unit().world()),
            SpawnObjectType::GameObject => self
                .get_gameobject_by_spawn_id_like_cpp(spawn_id)
                .map(GameObject::world),
            SpawnObjectType::AreaTrigger => self
                .get_area_trigger_by_spawn_id_like_cpp(spawn_id)
                .map(AreaTrigger::world),
        }
    }

    pub fn insert_map_object(
        &mut self,
        kind: AccessorObjectKind,
        object: WorldObject,
    ) -> Result<Option<MapObjectRecord>, MapObjectStoreError> {
        let record = MapObjectRecord::new(kind, object)?;
        self.insert_map_object_record(record)
    }

    pub fn insert_map_object_record(
        &mut self,
        record: MapObjectRecord,
    ) -> Result<Option<MapObjectRecord>, MapObjectStoreError> {
        self.validate_map_object(record.object())?;
        let guid = record.object().guid();
        let previous = self.map_objects.remove(&guid);
        if let Some(previous_record) = previous.as_ref() {
            self.unindex_map_object_record_by_spawn_id_like_cpp(previous_record);
        }
        self.index_map_object_record_by_spawn_id_like_cpp(&record);
        self.map_objects.insert(guid, record);
        Ok(previous)
    }

    fn index_map_object_record_by_spawn_id_like_cpp(&mut self, record: &MapObjectRecord) {
        if let Some(creature) = record.creature() {
            let spawn_id = creature.spawn_id();
            if spawn_id != 0 {
                self.creatures_by_spawn_id
                    .entry(spawn_id)
                    .or_default()
                    .insert(creature.guid());
            }
            return;
        }

        if let Some(gameobject) = record.game_object() {
            let spawn_id = gameobject.spawn_id();
            if spawn_id != 0 {
                self.gameobjects_by_spawn_id
                    .entry(spawn_id)
                    .or_default()
                    .insert(gameobject.world().guid());
            }
            return;
        }

        if let Some(area_trigger) = record.area_trigger() {
            let spawn_id = area_trigger.spawn_id();
            if spawn_id != 0 {
                self.area_triggers_by_spawn_id
                    .entry(spawn_id)
                    .or_default()
                    .insert(area_trigger.world().guid());
            }
        }
    }

    fn unindex_map_object_record_by_spawn_id_like_cpp(&mut self, record: &MapObjectRecord) {
        if let Some(creature) = record.creature() {
            Self::remove_spawn_id_index_entry_like_cpp(
                &mut self.creatures_by_spawn_id,
                creature.spawn_id(),
                creature.guid(),
            );
            return;
        }

        if let Some(gameobject) = record.game_object() {
            Self::remove_spawn_id_index_entry_like_cpp(
                &mut self.gameobjects_by_spawn_id,
                gameobject.spawn_id(),
                gameobject.world().guid(),
            );
            return;
        }

        if let Some(area_trigger) = record.area_trigger() {
            Self::remove_spawn_id_index_entry_like_cpp(
                &mut self.area_triggers_by_spawn_id,
                area_trigger.spawn_id(),
                area_trigger.world().guid(),
            );
        }
    }

    fn remove_spawn_id_index_entry_like_cpp(
        index: &mut HashMap<SpawnId, HashSet<ObjectGuid>>,
        spawn_id: SpawnId,
        guid: ObjectGuid,
    ) {
        if spawn_id == 0 {
            return;
        }

        if let Some(guids) = index.get_mut(&spawn_id) {
            guids.remove(&guid);
            if guids.is_empty() {
                index.remove(&spawn_id);
            }
        }
    }

    pub fn add_to_map_like_cpp(
        &mut self,
        kind: AccessorObjectKind,
        object: WorldObject,
    ) -> Result<AddToMapOutcome, AddToMapError> {
        let record = MapObjectRecord::new(kind, object).map_err(MapObjectStoreError::from)?;
        self.add_map_object_record_to_map_like_cpp(record)
    }

    pub fn add_map_object_record_to_map_like_cpp(
        &mut self,
        mut record: MapObjectRecord,
    ) -> Result<AddToMapOutcome, AddToMapError> {
        let kind = record.kind();
        let guid = record.object().guid();
        let position = record.object().position();
        let is_world_object = record.object().is_world_object();

        if record.object().object().is_in_world() {
            let cell = Cell::from_world(position.x, position.y);
            let previous = self.insert_map_object_record(record)?;
            return Ok(AddToMapOutcome {
                guid,
                cell: cell.cell_coord(),
                grid: GridCoord::new(cell.grid_x(), cell.grid_y()),
                inserted: previous.is_none(),
                already_in_world: true,
                grid_created: false,
                grid_loaded: false,
                inserted_into_cell: false,
            });
        }

        self.validate_map_object(record.object())?;

        if !is_valid_map_coord_2d(position.x, position.y) {
            return Err(AddToMapError::InvalidCoordinates {
                guid,
                x: position.x,
                y: position.y,
            });
        }

        let cell = Cell::from_world(position.x, position.y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        let active_object = is_active_object_like_cpp(kind, record.object());
        let grid_loaded = if active_object {
            self.ensure_grid_loaded_for_active_object(&cell, kind.into())
        } else {
            false
        };
        let grid_created = if active_object {
            false
        } else {
            self.ensure_grid_created(grid)
        };

        {
            let ngrid = self
                .get_ngrid_mut(grid)
                .expect("Map::AddToMap must have created or loaded the target grid");
            let local_cell = ngrid
                .get_grid_type_mut(cell.cell_x(), cell.cell_y())
                .expect("cell coordinates must be local to target grid");
            insert_object_guid_in_cell_like_cpp(local_cell, kind, is_world_object, guid);
        }

        {
            let object = record.object_mut();
            object.set_current_cell(cell.cell_x(), cell.cell_y());
            object.object_mut().add_to_world();
            object.object_mut().set_is_new_object(true);
            // Rust does not emit visibility here yet; keep the flag lifecycle identical to
            // C++ `Map::AddToMap` after `UpdateObjectVisibilityOnCreate()` returns.
            object.object_mut().set_is_new_object(false);
        }

        let previous = self.insert_map_object_record(record)?;
        Ok(AddToMapOutcome {
            guid,
            cell: cell.cell_coord(),
            grid,
            inserted: previous.is_none(),
            already_in_world: false,
            grid_created,
            grid_loaded,
            inserted_into_cell: true,
        })
    }

    pub fn remove_map_object(&mut self, guid: ObjectGuid) -> Option<MapObjectRecord> {
        let record = self.map_objects.remove(&guid)?;
        self.unindex_map_object_record_by_spawn_id_like_cpp(&record);
        Some(record)
    }

    pub fn remove_from_map_like_cpp(
        &mut self,
        guid: ObjectGuid,
        delete_from_world: bool,
    ) -> Result<RemoveFromMapOutcome, RemoveFromMapError> {
        let should_cleanup_dynamic_object_caster_viewpoint = self
            .map_object_record(guid)
            .and_then(MapObjectRecord::dynamic_object)
            .is_some_and(|dynamic_object| {
                dynamic_object.world().object().is_in_world()
                    && dynamic_object.is_caster_viewpoint()
            });
        let dynamic_object_caster_viewpoint = should_cleanup_dynamic_object_caster_viewpoint
            .then(|| self.apply_dynamic_object_caster_viewpoint_like_cpp(guid, false));
        let dynamic_object_remove_cleanup = self
            .map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::dynamic_object_mut)
            .and_then(|dynamic_object| {
                if !dynamic_object.world().object().is_in_world() {
                    return None;
                }

                let had_aura = dynamic_object.has_aura();
                if had_aura {
                    dynamic_object.remove_aura();
                }

                let unbound_caster = dynamic_object.bound_caster();
                if unbound_caster.is_some() {
                    dynamic_object.unbind_from_caster();
                }

                Some(DynamicObjectRemoveCleanupOutcomeLikeCpp {
                    had_aura,
                    removed_aura_pending_delete: dynamic_object.has_removed_aura_pending_delete(),
                    unbound_caster,
                })
            });
        let record = self
            .remove_map_object(guid)
            .ok_or(RemoveFromMapError::ObjectNotFound { guid })?;
        let linked_trap_guid = record
            .game_object()
            .map(GameObject::linked_trap_guid_like_cpp)
            .filter(|linked_guid| !linked_guid.is_empty() && *linked_guid != guid);
        let kind = record.kind();
        let was_world_object_like_cpp = map_record_is_world_object_like_cpp(&record);
        let mut object = record.into_object();
        let was_in_world = object.object().is_in_world();
        let was_active = is_active_object_like_cpp(kind, &object);
        let cell = Cell::from_world(object.position().x, object.position().y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());

        if let Some(linked_trap_guid) = linked_trap_guid {
            // C++ `GameObject::RemoveFromWorld` despawns `m_linkedTrap` before
            // `WorldObject::RemoveFromWorld` (`GameObject.cpp:939-943`), and
            // `Map::RemoveFromMap` calls `obj->RemoveFromWorld()` before grid
            // removal (`Map.cpp:933-951`). This bounded seam represents that
            // ordering map-locally with no scheduler/fanout, avoids
            // self-recursion, and tolerates traps already removed by another
            // path.
            if self.map_object_record(linked_trap_guid).is_some() {
                let _ = self.remove_from_map_like_cpp(linked_trap_guid, true);
            }
        }

        object.object_mut().remove_from_world();
        let removed_from_cell = remove_object_guid_from_cell_like_cpp(
            self,
            grid,
            &cell,
            kind,
            was_world_object_like_cpp,
            guid,
        );
        if was_active {
            self.unmark_active_cell(cell.cell_coord());
        }

        object.clear_current_cell();
        object.reset_map().map_err(RemoveFromMapError::ResetMap)?;

        Ok(RemoveFromMapOutcome {
            guid,
            cell: cell.cell_coord(),
            grid,
            was_in_world,
            was_active,
            removed_from_cell,
            delete_from_world,
            dynamic_object_caster_viewpoint,
            dynamic_object_remove_cleanup,
            object: if delete_from_world {
                None
            } else {
                Some(object)
            },
        })
    }

    pub fn relocate_map_object_like_cpp(
        &mut self,
        guid: ObjectGuid,
        new_position: Position,
    ) -> Result<MapObjectRelocationOutcome, MapObjectRelocationError> {
        if !is_valid_map_coord_2d(new_position.x, new_position.y) {
            return Err(MapObjectRelocationError::InvalidCoordinates {
                guid,
                x: new_position.x,
                y: new_position.y,
            });
        }

        let record = self
            .map_object_record(guid)
            .ok_or(MapObjectRelocationError::ObjectNotFound { guid })?;
        let kind = record.kind();
        let old_position = record.object().position();
        let old_cell = Cell::from_world(old_position.x, old_position.y);
        let new_cell = Cell::from_world(new_position.x, new_position.y);
        let old_grid = GridCoord::new(old_cell.grid_x(), old_cell.grid_y());
        let new_grid = GridCoord::new(new_cell.grid_x(), new_cell.grid_y());
        let diff_cell = old_cell.diff_cell(&new_cell);
        let diff_grid = old_cell.diff_grid(&new_cell);

        if !diff_cell && !diff_grid {
            let mut record = self
                .remove_map_object(guid)
                .expect("record was just observed");
            record.object_mut().relocate(new_position);
            self.insert_map_object_record(record)
                .map_err(MapObjectRelocationError::Store)?;
            return Ok(MapObjectRelocationOutcome {
                guid,
                old_cell: old_cell.cell_coord(),
                new_cell: new_cell.cell_coord(),
                old_grid,
                new_grid,
                moved_between_cells: false,
                loaded_grid: false,
                created_grid: false,
                relocated: true,
                blocked_by_unloaded_grid: false,
            });
        }

        let active_object = is_active_object_like_cpp(kind, record.object());
        let loaded_grid = if diff_grid && active_object {
            self.ensure_grid_loaded_for_active_object(&new_cell, kind.into())
        } else {
            false
        };
        let created_grid = if diff_grid && !active_object {
            if !self.is_grid_loaded(new_grid) {
                return Ok(MapObjectRelocationOutcome {
                    guid,
                    old_cell: old_cell.cell_coord(),
                    new_cell: new_cell.cell_coord(),
                    old_grid,
                    new_grid,
                    moved_between_cells: false,
                    loaded_grid: false,
                    created_grid: false,
                    relocated: false,
                    blocked_by_unloaded_grid: true,
                });
            }
            self.ensure_grid_created(new_grid)
        } else {
            false
        };

        let mut record = self
            .remove_map_object(guid)
            .expect("record was just observed");
        let object_is_world_object = record.object().is_world_object();
        let removed = remove_object_guid_from_cell_like_cpp(
            self,
            old_grid,
            &old_cell,
            kind,
            object_is_world_object,
            guid,
        );
        debug_assert!(removed, "relocated object should have been in its old cell");
        {
            let ngrid = self
                .get_ngrid_mut(new_grid)
                .expect("relocation target grid must be loaded or created");
            let local_cell = ngrid
                .get_grid_type_mut(new_cell.cell_x(), new_cell.cell_y())
                .expect("cell coordinates must be local to target grid");
            insert_object_guid_in_cell_like_cpp(local_cell, kind, object_is_world_object, guid);
        }
        record.object_mut().relocate(new_position);
        record
            .object_mut()
            .set_current_cell(new_cell.cell_x(), new_cell.cell_y());
        self.insert_map_object_record(record)
            .map_err(MapObjectRelocationError::Store)?;

        Ok(MapObjectRelocationOutcome {
            guid,
            old_cell: old_cell.cell_coord(),
            new_cell: new_cell.cell_coord(),
            old_grid,
            new_grid,
            moved_between_cells: true,
            loaded_grid,
            created_grid,
            relocated: true,
            blocked_by_unloaded_grid: false,
        })
    }

    pub fn nearby_cell_guids_like_cpp(&self, x: f32, y: f32, radius: f32) -> NearbyCellGuids {
        if !is_valid_map_coord_2d(x, y) {
            return NearbyCellGuids::default();
        }

        let area = calculate_cell_area_like_cpp(x, y, radius);
        let mut result = NearbyCellGuids::default();
        for cell_x in area.low_bound.x_coord..=area.high_bound.x_coord {
            for cell_y in area.low_bound.y_coord..=area.high_bound.y_coord {
                result.visited_cells += 1;
                let cell = Cell::from_cell_coord(CellCoord::new(cell_x, cell_y));
                let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
                else {
                    continue;
                };
                let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                    continue;
                };
                result.merge_world(&local_cell.world_objects);
                result.merge_grid(&local_cell.grid_objects);
            }
        }

        result
    }

    pub fn visit_nearby_cells_of_like_cpp(
        &self,
        centers: impl IntoIterator<Item = NearbyCellVisitCenter>,
    ) -> NearbyCellVisitPlan {
        let mut marked_cells = HashSet::new();
        let mut marked_cells_in_visit_order = Vec::new();
        let mut nearby = NearbyCellGuids::default();
        let mut skipped_missing_centers = Vec::new();
        let mut skipped_invalid_position_centers = Vec::new();

        for center in centers {
            let Some(object) = self.map_object(center.guid) else {
                skipped_missing_centers.push(center.guid);
                continue;
            };
            let position = object.position();
            if !is_valid_map_coord_2d(position.x, position.y) {
                skipped_invalid_position_centers.push(center.guid);
                continue;
            }

            let area =
                calculate_cell_area_like_cpp(position.x, position.y, center.activation_radius);
            for cell_x in area.low_bound.x_coord..=area.high_bound.x_coord {
                for cell_y in area.low_bound.y_coord..=area.high_bound.y_coord {
                    let cell_coord = CellCoord::new(cell_x, cell_y);
                    if !marked_cells.insert(cell_coord) {
                        continue;
                    }

                    marked_cells_in_visit_order.push(cell_coord);
                    nearby.visited_cells += 1;
                    let cell = Cell::from_cell_coord(cell_coord);
                    let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
                    else {
                        continue;
                    };
                    let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                        continue;
                    };
                    nearby.merge_world(&local_cell.world_objects);
                    nearby.merge_grid(&local_cell.grid_objects);
                }
            }
        }

        NearbyCellVisitPlan {
            marked_cells: marked_cells_in_visit_order,
            nearby,
            skipped_missing_centers,
            skipped_invalid_position_centers,
        }
    }

    pub fn object_update_plan_for_nearby_like_cpp(
        &self,
        nearby: &NearbyCellGuids,
        diff_ms: u32,
    ) -> ObjectUpdatePlan {
        let mut update_guids = Vec::new();
        for guid in nearby
            .world
            .creatures
            .iter()
            .chain(nearby.world.dynamic_objects.iter())
            .chain(nearby.grid.creatures.iter())
            .chain(nearby.grid.gameobjects.iter())
            .chain(nearby.grid.dynamic_objects.iter())
            .chain(nearby.grid.area_triggers.iter())
            .chain(nearby.grid.scene_objects.iter())
            .chain(nearby.grid.conversations.iter())
        {
            if self
                .map_object(*guid)
                .is_some_and(|object| object.object().is_in_world())
            {
                update_guids.push(*guid);
            }
        }

        update_guids.sort();
        update_guids.dedup();
        ObjectUpdatePlan {
            diff_ms,
            update_guids,
        }
    }

    pub fn map_update_visit_plan_like_cpp(
        &self,
        sources: impl IntoIterator<Item = MapUpdatePlayerSources>,
        active_non_player_guids: impl IntoIterator<Item = ObjectGuid>,
        transport_guids: impl IntoIterator<Item = ObjectGuid>,
        diff_ms: u32,
    ) -> MapUpdateVisitPlan {
        let mut session_update_players = Vec::new();
        let mut player_update_guids = Vec::new();
        let mut nearby_visit_centers = Vec::new();
        let mut saw_player_source = false;

        for source in sources {
            saw_player_source = true;
            if !self.object_is_in_world(source.player_guid) {
                continue;
            }

            session_update_players.push(source.player_guid);
            player_update_guids.push(source.player_guid);
            nearby_visit_centers.push(source.player_guid);

            if let Some(viewpoint) = source.viewpoint_guid
                && self.object_is_in_world(viewpoint)
            {
                nearby_visit_centers.push(viewpoint);
            }

            push_in_world_guids(
                self,
                &mut nearby_visit_centers,
                source.far_combat_unit_guids,
            );
            push_in_world_guids(
                self,
                &mut nearby_visit_centers,
                source.far_aura_caster_guids,
            );
            push_in_world_guids(self, &mut nearby_visit_centers, source.far_summon_guids);
        }

        let mut saw_active_non_player_source = false;
        for guid in active_non_player_guids {
            saw_active_non_player_source = true;
            if self.object_is_in_world(guid) {
                nearby_visit_centers.push(guid);
            }
        }

        let mut transport_update_guids = Vec::new();
        for guid in transport_guids {
            if self.map_object(guid).is_some() {
                transport_update_guids.push(guid);
            }
        }

        sort_dedup(&mut session_update_players);
        sort_dedup(&mut player_update_guids);
        sort_dedup(&mut nearby_visit_centers);
        sort_dedup(&mut transport_update_guids);
        let process_relocation_notifies = saw_player_source || saw_active_non_player_source;

        MapUpdateVisitPlan {
            diff_ms,
            session_update_players,
            player_update_guids,
            nearby_visit_centers,
            transport_update_guids,
            process_relocation_notifies,
        }
    }

    pub fn process_relocation_notifies_plan_like_cpp(
        &mut self,
        marked_cells: impl IntoIterator<Item = CellCoord>,
        diff_ms: u32,
        visibility_notify_period_ms: i64,
    ) -> RelocationNotifyProcessPlan {
        let marked_cells: HashSet<_> = marked_cells.into_iter().collect();
        let mut delayed_relocation_cells = Vec::new();
        let mut reset_notify_cells = Vec::new();
        let mut reset_timer_grids = Vec::new();
        let mut expired_active_grids = Vec::new();

        for grid_x in 0..MAX_NUMBER_OF_GRIDS {
            for grid_y in 0..MAX_NUMBER_OF_GRIDS {
                let coord = GridCoord::new(grid_x, grid_y);
                let Some(grid) = self.get_ngrid_mut(coord) else {
                    continue;
                };
                if grid.state() != GridStateKind::Active {
                    continue;
                }

                grid.info_mut()
                    .relocation_timer_mut()
                    .tracker_update(diff_ms);
                if !grid.info().relocation_timer().tracker_passed() {
                    continue;
                }

                expired_active_grids.push(coord);
                delayed_relocation_cells
                    .extend(marked_cells_in_grid_like_cpp(coord, &marked_cells));
            }
        }

        for coord in &expired_active_grids {
            let Some(grid) = self.get_ngrid_mut(*coord) else {
                continue;
            };
            if grid.state() != GridStateKind::Active {
                continue;
            }
            if !grid.info().relocation_timer().tracker_passed() {
                continue;
            }

            grid.info_mut()
                .relocation_timer_mut()
                .tracker_reset(diff_ms, visibility_notify_period_ms);
            reset_timer_grids.push(*coord);
            reset_notify_cells.extend(marked_cells_in_grid_like_cpp(*coord, &marked_cells));
        }

        RelocationNotifyProcessPlan {
            diff_ms,
            delayed_relocation_cells,
            reset_notify_cells,
            reset_timer_grids,
        }
    }

    pub fn process_relocation_notifies_like_cpp(
        &mut self,
        marked_cells: impl IntoIterator<Item = CellCoord>,
        diff_ms: u32,
        visibility_notify_period_ms: i64,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> ProcessRelocationNotifiesOutcome {
        let process_plan = self.process_relocation_notifies_plan_like_cpp(
            marked_cells,
            diff_ms,
            visibility_notify_period_ms,
        );
        let delayed_plan = self.delayed_unit_relocation_for_cells_like_cpp(
            process_plan.delayed_relocation_cells.iter().copied(),
            invalid_non_self_viewpoints,
        );
        let reset_outcome = self
            .reset_notify_flags_for_cells_like_cpp(process_plan.reset_notify_cells.iter().copied());

        ProcessRelocationNotifiesOutcome {
            process_plan,
            delayed_plan,
            reset_outcome,
        }
    }

    pub fn reset_notify_flags_for_cells_like_cpp(
        &mut self,
        cells: impl IntoIterator<Item = CellCoord>,
    ) -> ResetNotifyFlagsOutcome {
        let mut reset_player_guids = Vec::new();
        let mut reset_creature_guids = Vec::new();
        let mut missing_guids = Vec::new();

        for cell_coord in cells {
            let cell = Cell::from_cell_coord(cell_coord);
            let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y())) else {
                continue;
            };
            let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                continue;
            };

            reset_player_guids.extend(local_cell.world_objects.players.iter().copied());
            reset_creature_guids.extend(local_cell.grid_objects.creatures.iter().copied());
            reset_creature_guids.extend(local_cell.world_objects.creatures.iter().copied());
        }

        sort_dedup(&mut reset_player_guids);
        sort_dedup(&mut reset_creature_guids);

        for guid in reset_player_guids
            .iter()
            .chain(reset_creature_guids.iter())
            .copied()
        {
            let Some(record) = self.map_objects.get_mut(&guid) else {
                missing_guids.push(guid);
                continue;
            };
            record.object_mut().object_mut().reset_all_notifies();
        }

        ResetNotifyFlagsOutcome {
            reset_player_guids,
            reset_creature_guids,
            missing_guids,
        }
    }

    pub fn delayed_unit_relocation_for_cells_like_cpp(
        &self,
        cells: impl IntoIterator<Item = CellCoord>,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> DelayedUnitRelocationForCellsPlan {
        let invalid_non_self_viewpoints: HashSet<_> =
            invalid_non_self_viewpoints.into_iter().collect();
        let mut cell_plans = Vec::new();

        for cell_coord in cells {
            let nearby = self.exact_cell_guids_like_cpp(cell_coord);
            let creatures_needing_notify = nearby
                .world
                .creatures
                .iter()
                .chain(nearby.grid.creatures.iter())
                .copied()
                .filter(|guid| self.object_needs_notify_visibility(*guid));
            let player_viewpoints_needing_notify = nearby
                .world
                .players
                .iter()
                .copied()
                .filter(|guid| self.object_needs_notify_visibility(*guid));

            let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(
                &nearby,
                creatures_needing_notify,
                player_viewpoints_needing_notify,
                invalid_non_self_viewpoints.iter().copied(),
            );
            if !plan.creature_relocations.is_empty()
                || !plan.player_relocations.is_empty()
                || !plan.skipped_invalid_viewpoints.is_empty()
            {
                cell_plans.push(DelayedUnitRelocationCellPlan { cell_coord, plan });
            }
        }

        DelayedUnitRelocationForCellsPlan { cell_plans }
    }

    pub fn delayed_unit_relocation_visibility_plans_like_cpp(
        &self,
        delayed_plan: &DelayedUnitRelocationForCellsPlan,
        player_contexts: impl IntoIterator<Item = DelayedPlayerRelocationContext>,
        creature_contexts: impl IntoIterator<Item = DelayedCreatureRelocationContext>,
    ) -> DelayedUnitRelocationVisibilityPlans {
        let player_contexts: HashMap<_, _> = player_contexts
            .into_iter()
            .map(|context| (context.player_guid, context))
            .collect();
        let creature_contexts: HashMap<_, _> = creature_contexts
            .into_iter()
            .map(|context| (context.creature_guid, context))
            .collect();
        let mut creature_plans = Vec::new();
        let mut player_plans = Vec::new();
        let mut skipped_missing_sources = Vec::new();
        let mut skipped_invalid_source_positions = Vec::new();
        let mut missing_player_contexts = Vec::new();

        for cell_plan in &delayed_plan.cell_plans {
            for creature_guid in &cell_plan.plan.creature_relocations {
                let Some(creature) = self.map_object(*creature_guid) else {
                    skipped_missing_sources.push(*creature_guid);
                    continue;
                };
                let position = creature.position();
                if !is_valid_map_coord_2d(position.x, position.y) {
                    skipped_invalid_source_positions.push(*creature_guid);
                    continue;
                }

                let nearby = self.nearby_cell_guids_like_cpp(
                    position.x,
                    position.y,
                    MAX_VISIBILITY_DISTANCE + creature.combat_reach(),
                );
                let player_seers_needing_notify = nearby
                    .world
                    .players
                    .iter()
                    .copied()
                    .filter(|guid| self.object_needs_notify_visibility(*guid));
                let creatures_needing_notify = nearby
                    .world
                    .creatures
                    .iter()
                    .chain(nearby.grid.creatures.iter())
                    .copied()
                    .filter(|guid| self.object_needs_notify_visibility(*guid));
                let source_creature_alive = creature_contexts
                    .get(creature_guid)
                    .is_none_or(|context| context.source_creature_alive);
                let visibility_plan = CreatureRelocationVisibilityPlan::from_nearby_like_cpp(
                    *creature_guid,
                    source_creature_alive,
                    &nearby,
                    player_seers_needing_notify,
                    creatures_needing_notify,
                );
                creature_plans.push(CreatureDelayedRelocationVisibilityPlan {
                    creature_guid: *creature_guid,
                    cell_coord: cell_plan.cell_coord,
                    nearby,
                    visibility_plan,
                });
            }

            for player_guid in &cell_plan.plan.player_relocations {
                let Some(context) = player_contexts.get(player_guid) else {
                    missing_player_contexts.push(*player_guid);
                    continue;
                };
                let Some(viewpoint) = self.map_object(context.viewpoint_guid) else {
                    skipped_missing_sources.push(context.viewpoint_guid);
                    continue;
                };
                let position = viewpoint.position();
                if !is_valid_map_coord_2d(position.x, position.y) {
                    skipped_invalid_source_positions.push(context.viewpoint_guid);
                    continue;
                }

                let nearby = self.nearby_cell_guids_like_cpp(
                    position.x,
                    position.y,
                    MAX_VISIBILITY_DISTANCE + viewpoint.combat_reach(),
                );
                let visibility_plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
                    *player_guid,
                    context.previous_client_guids.iter().copied(),
                    &nearby,
                    context.relocated_for_ai,
                );
                player_plans.push(PlayerDelayedRelocationVisibilityPlan {
                    player_guid: *player_guid,
                    viewpoint_guid: context.viewpoint_guid,
                    cell_coord: cell_plan.cell_coord,
                    nearby,
                    visibility_plan,
                });
            }
        }

        sort_dedup(&mut skipped_missing_sources);
        sort_dedup(&mut skipped_invalid_source_positions);
        sort_dedup(&mut missing_player_contexts);

        DelayedUnitRelocationVisibilityPlans {
            creature_plans,
            player_plans,
            skipped_missing_sources,
            skipped_invalid_source_positions,
            missing_player_contexts,
        }
    }

    pub fn process_map_object_move_list_like_cpp(
        &mut self,
        entries: impl IntoIterator<Item = MapObjectMoveListEntry>,
    ) -> MapObjectMoveListPlan {
        let mut plan = MapObjectMoveListPlan::default();

        for entry in entries {
            let Some(record) = self.map_object_record(entry.guid) else {
                plan.skipped_other_map_or_missing.push(entry.guid);
                continue;
            };
            if record.kind() != entry.kind {
                plan.skipped_kind_mismatch.push(entry.guid);
                continue;
            }

            if entry.move_state != MapObjectCellMoveState::Active {
                plan.reset_inactive_or_none.push(entry.guid);
                continue;
            }

            if !record.object().object().is_in_world() {
                plan.skipped_not_in_world.push(entry.guid);
                continue;
            }

            match self.relocate_map_object_like_cpp(entry.guid, entry.new_position) {
                Ok(outcome) if outcome.relocated => {
                    plan.relocated.push(entry.guid);
                    continue;
                }
                Ok(outcome) if outcome.blocked_by_unloaded_grid => {}
                Ok(_) => {}
                Err(MapObjectRelocationError::InvalidCoordinates { .. }) => {
                    plan.failed_invalid_position.push(entry.guid);
                    continue;
                }
                Err(MapObjectRelocationError::ObjectNotFound { .. }) => {
                    plan.skipped_other_map_or_missing.push(entry.guid);
                    continue;
                }
                Err(MapObjectRelocationError::Record(_) | MapObjectRelocationError::Store(_)) => {
                    plan.failed_store.push(entry.guid);
                    continue;
                }
            }

            match entry.kind {
                AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
                    if let Some(respawn_position) = entry.respawn_position
                        && self
                            .relocate_map_object_like_cpp(entry.guid, respawn_position)
                            .is_ok_and(|outcome| outcome.relocated)
                    {
                        plan.respawn_relocated.push(entry.guid);
                        continue;
                    }

                    if entry.kind == AccessorObjectKind::Pet || entry.is_pet {
                        plan.pet_removed.push(entry.guid);
                    } else {
                        plan.remove_from_world.push(entry.guid);
                    }
                }
                AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
                    if let Some(respawn_position) = entry.respawn_position
                        && self
                            .relocate_map_object_like_cpp(entry.guid, respawn_position)
                            .is_ok_and(|outcome| outcome.relocated)
                    {
                        plan.respawn_relocated.push(entry.guid);
                        continue;
                    }

                    plan.remove_from_world.push(entry.guid);
                }
                AccessorObjectKind::DynamicObject | AccessorObjectKind::AreaTrigger => {
                    plan.blocked_unloaded_grid.push(entry.guid);
                }
                AccessorObjectKind::Player
                | AccessorObjectKind::Corpse
                | AccessorObjectKind::SceneObject
                | AccessorObjectKind::Conversation => {
                    plan.unsupported_kind.push(entry.guid);
                }
            }
        }

        plan
    }

    pub fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord> {
        self.map_objects.get(&guid)
    }

    pub fn map_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_record(guid).map(MapObjectRecord::object)
    }

    fn object_is_in_world(&self, guid: ObjectGuid) -> bool {
        self.map_object(guid)
            .is_some_and(|object| object.object().is_in_world())
    }

    fn object_needs_notify_visibility(&self, guid: ObjectGuid) -> bool {
        self.map_object(guid).is_some_and(|object| {
            object
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        })
    }

    fn exact_cell_guids_like_cpp(&self, cell_coord: CellCoord) -> NearbyCellGuids {
        let mut nearby = NearbyCellGuids::default();
        let cell = Cell::from_cell_coord(cell_coord);
        let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y())) else {
            return nearby;
        };
        let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
            return nearby;
        };

        nearby.visited_cells = 1;
        nearby.merge_world(&local_cell.world_objects);
        nearby.merge_grid(&local_cell.grid_objects);
        nearby
    }

    pub fn map_object_by_kind(
        &self,
        guid: ObjectGuid,
        allowed: &[AccessorObjectKind],
    ) -> Option<&WorldObject> {
        let record = self.map_object_record(guid)?;
        allowed.contains(&record.kind()).then_some(record.object())
    }

    pub fn get_creature(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Creature])
    }

    pub fn get_typed_creature(&self, guid: ObjectGuid) -> Option<&Creature> {
        let record = self.map_object_record(guid)?;
        if record.kind() != AccessorObjectKind::Creature {
            return None;
        }
        record.creature()
    }

    pub fn get_typed_creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut Creature> {
        let record = self.map_objects.get_mut(&guid)?;
        if record.kind() != AccessorObjectKind::Creature {
            return None;
        }
        record.creature_mut()
    }

    pub fn get_pet(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Pet])
    }

    pub fn get_game_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(
            guid,
            &[
                AccessorObjectKind::GameObject,
                AccessorObjectKind::Transport,
            ],
        )
    }

    pub fn get_typed_game_object(&self, guid: ObjectGuid) -> Option<&GameObject> {
        let record = self.map_object_record(guid)?;
        if !matches!(
            record.kind(),
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport
        ) {
            return None;
        }
        record.game_object()
    }

    pub fn get_typed_game_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut GameObject> {
        let record = self.map_objects.get_mut(&guid)?;
        if !matches!(
            record.kind(),
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport
        ) {
            return None;
        }
        record.game_object_mut()
    }

    pub fn get_typed_player(&self, guid: ObjectGuid) -> Option<&Player> {
        let record = self.map_object_record(guid)?;
        if record.kind() != AccessorObjectKind::Player {
            return None;
        }
        record.player()
    }

    pub fn get_typed_player_mut(&mut self, guid: ObjectGuid) -> Option<&mut Player> {
        let record = self.map_objects.get_mut(&guid)?;
        if record.kind() != AccessorObjectKind::Player {
            return None;
        }
        record.player_mut()
    }

    pub fn get_typed_dynamic_object(&self, guid: ObjectGuid) -> Option<&DynamicObject> {
        let record = self.map_object_record(guid)?;
        if record.kind() != AccessorObjectKind::DynamicObject {
            return None;
        }
        record.dynamic_object()
    }

    pub fn get_typed_dynamic_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut DynamicObject> {
        let record = self.map_objects.get_mut(&guid)?;
        if record.kind() != AccessorObjectKind::DynamicObject {
            return None;
        }
        record.dynamic_object_mut()
    }

    fn combat_unit_snapshot_like_cpp(
        &self,
        guid: ObjectGuid,
    ) -> Option<CombatUnitSnapshotLikeCpp<'_>> {
        if let Some(player) = self.get_typed_player(guid) {
            return Some(CombatUnitSnapshotLikeCpp {
                guid,
                unit: player.unit(),
                game_master_player: player.is_game_master_like_cpp(),
            });
        }
        self.get_typed_creature(guid)
            .map(|creature| CombatUnitSnapshotLikeCpp {
                guid,
                unit: creature.unit(),
                game_master_player: false,
            })
    }

    fn combat_begin_context_like_cpp(
        &self,
        owner: CombatUnitSnapshotLikeCpp<'_>,
        target: CombatUnitSnapshotLikeCpp<'_>,
    ) -> CombatBeginContextLikeCpp {
        let owner_world = owner.unit.world();
        let target_world = target.unit.world();
        CombatBeginContextLikeCpp {
            same_unit: owner.guid == target.guid,
            attacker_in_world: owner_world.object().is_in_world(),
            victim_in_world: target_world.object().is_in_world(),
            attacker_alive: owner.unit.is_alive(),
            victim_alive: target.unit.is_alive(),
            same_map: owner_world.is_in_map(target_world),
            same_phase: owner_world.in_same_phase(target_world),
            attacker_unit_state: owner.unit.unit_state(),
            victim_unit_state: target.unit.unit_state(),
            attacker_combat_disallowed: owner.unit.subsystems().combat.combat_disallowed,
            victim_combat_disallowed: target.unit.subsystems().combat.combat_disallowed,
            relation_represented: false,
            attacker_is_friendly_to_victim: false,
            victim_is_friendly_to_attacker: false,
            attacker_or_owner_player_is_game_master: owner.game_master_player,
            victim_or_owner_player_is_game_master: target.game_master_player,
        }
    }

    pub fn typed_combat_unit_guids_like_cpp(&self) -> Vec<ObjectGuid> {
        self.map_objects
            .iter()
            .filter_map(|(guid, record)| {
                matches!(
                    record.kind(),
                    AccessorObjectKind::Player | AccessorObjectKind::Creature
                )
                .then_some(*guid)
            })
            .collect()
    }

    pub fn revalidate_all_combat_refs_like_cpp(&mut self) -> Vec<(ObjectGuid, ObjectGuid)> {
        let owner_guids = self.typed_combat_unit_guids_like_cpp();
        let mut invalid = Vec::new();

        for owner_guid in owner_guids {
            let Some(owner) = self.combat_unit_snapshot_like_cpp(owner_guid) else {
                continue;
            };
            let refs: Vec<_> = owner
                .unit
                .subsystems()
                .combat
                .pve_refs
                .keys()
                .chain(owner.unit.subsystems().combat.pvp_refs.keys())
                .copied()
                .collect();

            for target_guid in refs {
                let Some(target) = self.combat_unit_snapshot_like_cpp(target_guid) else {
                    invalid.push((owner_guid, target_guid));
                    continue;
                };
                if !CombatSubsystem::can_begin_combat_like_cpp(
                    self.combat_begin_context_like_cpp(owner, target),
                ) {
                    invalid.push((owner_guid, target_guid));
                }
            }
        }

        for (owner_guid, target_guid) in &invalid {
            if let Some(owner) = self.get_typed_player_mut(*owner_guid) {
                owner
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*target_guid);
            } else if let Some(owner) = self.get_typed_creature_mut(*owner_guid) {
                owner
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*target_guid);
            }

            if let Some(target) = self.get_typed_player_mut(*target_guid) {
                target
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*owner_guid);
            } else if let Some(target) = self.get_typed_creature_mut(*target_guid) {
                target
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*owner_guid);
            }
        }

        invalid
    }

    pub fn get_transport(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Transport])
    }

    pub fn get_dynamic_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::DynamicObject])
    }

    pub fn get_area_trigger(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::AreaTrigger])
    }

    pub fn get_corpse(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Corpse])
    }

    pub fn get_scene_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::SceneObject])
    }

    pub fn get_conversation(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Conversation])
    }

    fn validate_map_object(&self, object: &WorldObject) -> Result<(), MapObjectStoreError> {
        if object.map_id() == self.map_id && object.instance_id() == self.instance_id {
            return Ok(());
        }

        Err(MapObjectStoreError::WrongMap {
            guid: object.guid(),
            expected_map_id: self.map_id,
            expected_instance_id: self.instance_id,
            actual_map_id: object.map_id(),
            actual_instance_id: object.instance_id(),
        })
    }

    pub fn mark_active_cell(&mut self, cell: CellCoord) {
        assert!(cell.is_coord_valid());
        self.active_cells.insert(cell);
    }

    pub fn unmark_active_cell(&mut self, cell: CellCoord) {
        self.active_cells.remove(&cell);
    }

    pub fn get_ngrid(&self, coord: GridCoord) -> Option<&NGrid> {
        let index = grid_index(coord)?;
        self.grids[index].as_deref()
    }

    pub fn get_ngrid_mut(&mut self, coord: GridCoord) -> Option<&mut NGrid> {
        let index = grid_index(coord)?;
        self.grids[index].as_deref_mut()
    }

    pub fn set_ngrid(&mut self, coord: GridCoord, grid: Option<NGrid>) {
        let index = checked_grid_index(coord);
        self.grids[index] = grid.map(Box::new);
    }

    pub fn is_grid_loaded(&self, coord: GridCoord) -> bool {
        self.get_ngrid(coord)
            .is_some_and(NGrid::grid_object_data_loaded)
    }

    pub fn ensure_grid_created(&mut self, coord: GridCoord) -> bool {
        let index = checked_grid_index(coord);
        if self.grids[index].is_some() {
            return false;
        }

        let mut grid = NGrid::from_coords(
            coord.x_coord as i32,
            coord.y_coord as i32,
            self.grid_expiry_ms,
            self.grid_unload,
        );
        grid.set_state(GridStateKind::Idle);
        self.grids[index] = Some(Box::new(grid));

        let (terrain_x, terrain_y) = terrain_grid_coords(coord);
        self.terrain.load_map_and_vmap(terrain_x, terrain_y);
        true
    }

    pub fn ensure_grid_loaded(&mut self, cell: &Cell) -> bool {
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.ensure_grid_created(coord);
        let index = checked_grid_index(coord);
        let grid = self.grids[index].as_mut().expect("grid was just created");
        if grid.grid_object_data_loaded() {
            return false;
        }

        grid.set_grid_object_data_loaded(true);
        self.lifecycle.load_grid_objects(grid, cell);
        true
    }

    pub fn ensure_grid_loaded_for_active_object(
        &mut self,
        cell: &Cell,
        kind: ActiveObjectKind,
    ) -> bool {
        let loaded_now = self.ensure_grid_loaded(cell);
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.mark_active_cell(cell.cell_coord());

        if matches!(kind, ActiveObjectKind::Player) {
            // Use `ensure_grid_loaded_for_player_phase` when phase-shift state
            // is available; this entry point only has the object kind.
        }

        let active_expiry_ms = (self.grid_expiry_ms as f32 * 0.1) as i64;
        let grid = self.get_ngrid_mut(coord).expect("grid was just loaded");
        if grid.state() != GridStateKind::Active {
            grid.info_mut().reset_time_tracker(active_expiry_ms);
            grid.set_state(GridStateKind::Active);
        }

        loaded_now
    }

    pub fn ensure_grid_loaded_for_player_phase<Filter>(
        &mut self,
        cell: &Cell,
        phase_shift: &PhaseShift,
        loader: &mut ObjectGridLoader<'_, Filter>,
    ) -> bool
    where
        Filter: GridSpawnLoadFilter,
    {
        let loaded_now = self.ensure_grid_loaded(cell);
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.mark_active_cell(cell.cell_coord());

        let active_expiry_ms = (self.grid_expiry_ms as f32 * 0.1) as i64;
        let index = checked_grid_index(coord);
        let grid = self.grids[index].as_mut().expect("grid was just loaded");
        self.personal_phase_tracker
            .load_grid(phase_shift, grid, loader);

        if grid.state() != GridStateKind::Active {
            grid.info_mut().reset_time_tracker(active_expiry_ms);
            grid.set_state(GridStateKind::Active);
        }

        loaded_now
    }

    pub fn load_grid(&mut self, x: f32, y: f32) -> bool {
        self.ensure_grid_loaded(&Cell::from_world(x, y))
    }

    pub fn load_grid_for_active_object(&mut self, x: f32, y: f32, kind: ActiveObjectKind) -> bool {
        self.ensure_grid_loaded_for_active_object(&Cell::from_world(x, y), kind)
    }

    pub fn reset_grid_expiry(&self, grid: &mut NGrid, factor: f32) {
        grid.info_mut()
            .reset_time_tracker((self.grid_expiry_ms as f32 * factor) as i64);
    }

    pub fn active_objects_near_grid(&self, grid: &NGrid) -> bool {
        active_cells_near_grid(&self.active_cells, self.visible_distance, grid)
    }

    pub fn unload_grid_at(&mut self, coord: GridCoord, unload_all: bool) -> bool {
        let index = checked_grid_index(coord);
        let Some(mut grid) = self.grids[index].take() else {
            return false;
        };

        if !self.can_unload_grid(&grid, unload_all) {
            self.grids[index] = Some(grid);
            return false;
        }

        self.run_unload_lifecycle(&mut grid, unload_all);
        true
    }

    pub fn update_grid_state_at(&mut self, coord: GridCoord, diff_ms: u32) -> bool {
        let index = checked_grid_index(coord);
        let Some(mut grid) = self.grids[index].take() else {
            return false;
        };

        self.grid_state_unloaded = false;
        update_grid_state(self, &mut grid, diff_ms);
        if self.grid_state_unloaded {
            self.grid_state_unloaded = false;
            true
        } else {
            self.grids[index] = Some(grid);
            false
        }
    }

    fn can_unload_grid(&self, grid: &NGrid, unload_all: bool) -> bool {
        unload_all
            || (grid.world_creature_count_in_ngrid() == 0 && !self.active_objects_near_grid(grid))
    }

    fn run_unload_lifecycle(&mut self, grid: &mut NGrid, unload_all: bool) {
        if !unload_all {
            self.lifecycle.evacuate_grid(grid);
            self.drain_grid_unload_actions_like_cpp();
        }

        self.lifecycle.clean_grid(grid);
        self.drain_grid_unload_actions_like_cpp();
        self.personal_phase_tracker.unload_grid(grid);
        self.lifecycle.unload_grid_objects(grid);
        self.drain_grid_unload_actions_like_cpp();

        let coord = GridCoord::new(grid.x() as u32, grid.y() as u32);
        let (terrain_x, terrain_y) = terrain_grid_coords(coord);
        self.terrain.unload_map(terrain_x, terrain_y);
    }

    fn drain_grid_unload_actions_like_cpp(&mut self) -> Vec<GridUnloadApplyOutcome> {
        let actions = self.lifecycle.take_unload_actions_like_cpp();
        if actions.is_empty() {
            return Vec::new();
        }

        apply_grid_unload_actions(self, actions)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapObjectStoreError {
    InvalidRecord(ObjectAccessorError),
    WrongMap {
        guid: ObjectGuid,
        expected_map_id: u32,
        expected_instance_id: u32,
        actual_map_id: u32,
        actual_instance_id: u32,
    },
}

impl From<ObjectAccessorError> for MapObjectStoreError {
    fn from(error: ObjectAccessorError) -> Self {
        Self::InvalidRecord(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddToMapOutcome {
    pub guid: ObjectGuid,
    pub cell: CellCoord,
    pub grid: GridCoord,
    pub inserted: bool,
    pub already_in_world: bool,
    pub grid_created: bool,
    pub grid_loaded: bool,
    pub inserted_into_cell: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddToMapError {
    InvalidCoordinates { guid: ObjectGuid, x: f32, y: f32 },
    Store(MapObjectStoreError),
}

impl From<MapObjectStoreError> for AddToMapError {
    fn from(error: MapObjectStoreError) -> Self {
        Self::Store(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoveFromMapOutcome {
    pub guid: ObjectGuid,
    pub cell: CellCoord,
    pub grid: GridCoord,
    pub was_in_world: bool,
    pub was_active: bool,
    pub removed_from_cell: bool,
    pub delete_from_world: bool,
    pub dynamic_object_caster_viewpoint: Option<DynamicObjectCasterViewpointOutcomeLikeCpp>,
    pub dynamic_object_remove_cleanup: Option<DynamicObjectRemoveCleanupOutcomeLikeCpp>,
    pub object: Option<WorldObject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicObjectRemoveCleanupOutcomeLikeCpp {
    pub had_aura: bool,
    pub removed_aura_pending_delete: bool,
    pub unbound_caster: Option<ObjectGuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveFromMapError {
    ObjectNotFound { guid: ObjectGuid },
    ResetMap(MapBindingError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapObjectRelocationOutcome {
    pub guid: ObjectGuid,
    pub old_cell: CellCoord,
    pub new_cell: CellCoord,
    pub old_grid: GridCoord,
    pub new_grid: GridCoord,
    pub moved_between_cells: bool,
    pub loaded_grid: bool,
    pub created_grid: bool,
    pub relocated: bool,
    pub blocked_by_unloaded_grid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapObjectRelocationError {
    ObjectNotFound { guid: ObjectGuid },
    InvalidCoordinates { guid: ObjectGuid, x: f32, y: f32 },
    Record(ObjectAccessorError),
    Store(MapObjectStoreError),
}

#[derive(Debug, Clone, Default)]
pub struct NearbyCellGuids {
    pub world: WorldObjectGuids,
    pub grid: GridObjectGuids,
    pub visited_cells: usize,
}

impl NearbyCellGuids {
    pub fn is_empty(&self) -> bool {
        self.world.is_empty() && self.grid.is_empty()
    }

    pub fn len(&self) -> usize {
        self.world.len() + self.grid.len()
    }

    pub fn all_guids(&self) -> HashSet<ObjectGuid> {
        let mut guids = HashSet::with_capacity(self.len());
        guids.extend(self.world.players.iter().copied());
        guids.extend(self.world.creatures.iter().copied());
        guids.extend(self.world.corpses.iter().copied());
        guids.extend(self.world.dynamic_objects.iter().copied());
        guids.extend(self.grid.gameobjects.iter().copied());
        guids.extend(self.grid.creatures.iter().copied());
        guids.extend(self.grid.dynamic_objects.iter().copied());
        guids.extend(self.grid.corpses.iter().copied());
        guids.extend(self.grid.area_triggers.iter().copied());
        guids.extend(self.grid.scene_objects.iter().copied());
        guids.extend(self.grid.conversations.iter().copied());
        guids
    }

    fn merge_world(&mut self, other: &WorldObjectGuids) {
        self.world.players.extend(other.players.iter().copied());
        self.world.creatures.extend(other.creatures.iter().copied());
        self.world.corpses.extend(other.corpses.iter().copied());
        self.world
            .dynamic_objects
            .extend(other.dynamic_objects.iter().copied());
    }

    fn merge_grid(&mut self, other: &GridObjectGuids) {
        self.grid
            .gameobjects
            .extend(other.gameobjects.iter().copied());
        self.grid.creatures.extend(other.creatures.iter().copied());
        self.grid
            .dynamic_objects
            .extend(other.dynamic_objects.iter().copied());
        self.grid.corpses.extend(other.corpses.iter().copied());
        self.grid
            .area_triggers
            .extend(other.area_triggers.iter().copied());
        self.grid
            .scene_objects
            .extend(other.scene_objects.iter().copied());
        self.grid
            .conversations
            .extend(other.conversations.iter().copied());
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearbyCellVisitCenter {
    pub guid: ObjectGuid,
    pub activation_radius: f32,
}

#[derive(Debug, Clone, Default)]
pub struct NearbyCellVisitPlan {
    pub marked_cells: Vec<CellCoord>,
    pub nearby: NearbyCellGuids,
    pub skipped_missing_centers: Vec<ObjectGuid>,
    pub skipped_invalid_position_centers: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerRelocationVisibilityPlan {
    pub visible_guids: HashSet<ObjectGuid>,
    pub out_of_range_guids: HashSet<ObjectGuid>,
    pub reciprocal_player_updates: HashSet<ObjectGuid>,
    pub ai_relocation_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl PlayerRelocationVisibilityPlan {
    pub fn from_nearby_like_cpp(
        player_guid: ObjectGuid,
        previous_client_guids: impl IntoIterator<Item = ObjectGuid>,
        nearby: &NearbyCellGuids,
        relocated_for_ai: bool,
    ) -> Self {
        let visible_guids = nearby.all_guids();
        let mut out_of_range_guids: HashSet<_> = previous_client_guids.into_iter().collect();
        out_of_range_guids.remove(&player_guid);

        let mut reciprocal_player_updates = HashSet::new();
        let mut ai_relocation_checks = Vec::new();
        for guid in &visible_guids {
            out_of_range_guids.remove(guid);

            if guid.is_player() && *guid != player_guid {
                reciprocal_player_updates.insert(*guid);
            } else if relocated_for_ai && guid.is_any_type_creature() {
                ai_relocation_checks.push((*guid, player_guid));
            }
        }

        for guid in &out_of_range_guids {
            if guid.is_player() {
                reciprocal_player_updates.insert(*guid);
            }
        }

        Self {
            visible_guids,
            out_of_range_guids,
            reciprocal_player_updates,
            ai_relocation_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatureRelocationVisibilityPlan {
    pub player_visibility_updates: HashSet<ObjectGuid>,
    pub ai_relocation_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl CreatureRelocationVisibilityPlan {
    pub fn from_nearby_like_cpp(
        creature_guid: ObjectGuid,
        source_creature_alive: bool,
        nearby: &NearbyCellGuids,
        player_seers_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        creatures_needing_notify: impl IntoIterator<Item = ObjectGuid>,
    ) -> Self {
        let player_seers_needing_notify: HashSet<_> =
            player_seers_needing_notify.into_iter().collect();
        let creatures_needing_notify: HashSet<_> = creatures_needing_notify.into_iter().collect();
        let mut player_visibility_updates = HashSet::new();
        let mut ai_relocation_checks = Vec::new();

        for player in &nearby.world.players {
            if !player_seers_needing_notify.contains(player) {
                player_visibility_updates.insert(*player);
            }
            ai_relocation_checks.push((creature_guid, *player));
        }

        if source_creature_alive {
            for creature in nearby_creature_guids_excluding(nearby, creature_guid) {
                ai_relocation_checks.push((creature_guid, creature));
                if !creatures_needing_notify.contains(&creature) {
                    ai_relocation_checks.push((creature, creature_guid));
                }
            }
        }

        Self {
            player_visibility_updates,
            ai_relocation_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelayedUnitRelocationPlan {
    pub creature_relocations: Vec<ObjectGuid>,
    pub player_relocations: Vec<ObjectGuid>,
    pub skipped_invalid_viewpoints: Vec<ObjectGuid>,
}

impl DelayedUnitRelocationPlan {
    pub fn from_nearby_like_cpp(
        nearby: &NearbyCellGuids,
        creatures_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        player_viewpoints_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> Self {
        let creatures_needing_notify: HashSet<_> = creatures_needing_notify.into_iter().collect();
        let player_viewpoints_needing_notify: HashSet<_> =
            player_viewpoints_needing_notify.into_iter().collect();
        let invalid_non_self_viewpoints: HashSet<_> =
            invalid_non_self_viewpoints.into_iter().collect();

        let mut creature_relocations: Vec<_> = nearby
            .world
            .creatures
            .iter()
            .chain(nearby.grid.creatures.iter())
            .copied()
            .filter(|guid| creatures_needing_notify.contains(guid))
            .collect();
        creature_relocations.sort();
        creature_relocations.dedup();

        let mut player_relocations = Vec::new();
        let mut skipped_invalid_viewpoints = Vec::new();
        let mut players: Vec<_> = nearby.world.players.iter().copied().collect();
        players.sort();
        for player in players {
            if !player_viewpoints_needing_notify.contains(&player) {
                continue;
            }

            if invalid_non_self_viewpoints.contains(&player) {
                skipped_invalid_viewpoints.push(player);
            } else {
                player_relocations.push(player);
            }
        }

        Self {
            creature_relocations,
            player_relocations,
            skipped_invalid_viewpoints,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelayedUnitRelocationForCellsPlan {
    pub cell_plans: Vec<DelayedUnitRelocationCellPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedUnitRelocationCellPlan {
    pub cell_coord: CellCoord,
    pub plan: DelayedUnitRelocationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedPlayerRelocationContext {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: ObjectGuid,
    pub previous_client_guids: Vec<ObjectGuid>,
    pub relocated_for_ai: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelayedCreatureRelocationContext {
    pub creature_guid: ObjectGuid,
    pub source_creature_alive: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DelayedUnitRelocationVisibilityPlans {
    pub creature_plans: Vec<CreatureDelayedRelocationVisibilityPlan>,
    pub player_plans: Vec<PlayerDelayedRelocationVisibilityPlan>,
    pub skipped_missing_sources: Vec<ObjectGuid>,
    pub skipped_invalid_source_positions: Vec<ObjectGuid>,
    pub missing_player_contexts: Vec<ObjectGuid>,
}

#[derive(Debug, Clone)]
pub struct CreatureDelayedRelocationVisibilityPlan {
    pub creature_guid: ObjectGuid,
    pub cell_coord: CellCoord,
    pub nearby: NearbyCellGuids,
    pub visibility_plan: CreatureRelocationVisibilityPlan,
}

#[derive(Debug, Clone)]
pub struct PlayerDelayedRelocationVisibilityPlan {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: ObjectGuid,
    pub cell_coord: CellCoord,
    pub nearby: NearbyCellGuids,
    pub visibility_plan: PlayerRelocationVisibilityPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AIRelocationPlan {
    pub creature_unit_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl AIRelocationPlan {
    pub fn from_nearby_like_cpp(
        unit_guid: ObjectGuid,
        unit_is_creature: bool,
        nearby: &NearbyCellGuids,
    ) -> Self {
        let nearby_creatures = nearby_creature_guids_excluding(nearby, unit_guid);
        let mut creature_unit_checks = Vec::with_capacity(if unit_is_creature {
            nearby_creatures.len() * 2
        } else {
            nearby_creatures.len()
        });

        for creature in nearby_creatures {
            creature_unit_checks.push((creature, unit_guid));
            if unit_is_creature {
                creature_unit_checks.push((unit_guid, creature));
            }
        }

        Self {
            creature_unit_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectUpdatePlan {
    pub diff_ms: u32,
    pub update_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapUpdatePlayerSources {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: Option<ObjectGuid>,
    pub far_combat_unit_guids: Vec<ObjectGuid>,
    pub far_aura_caster_guids: Vec<ObjectGuid>,
    pub far_summon_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapUpdateVisitPlan {
    pub diff_ms: u32,
    pub session_update_players: Vec<ObjectGuid>,
    pub player_update_guids: Vec<ObjectGuid>,
    pub nearby_visit_centers: Vec<ObjectGuid>,
    pub transport_update_guids: Vec<ObjectGuid>,
    pub process_relocation_notifies: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelocationNotifyProcessPlan {
    pub diff_ms: u32,
    pub delayed_relocation_cells: Vec<CellCoord>,
    pub reset_notify_cells: Vec<CellCoord>,
    pub reset_timer_grids: Vec<GridCoord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessRelocationNotifiesOutcome {
    pub process_plan: RelocationNotifyProcessPlan,
    pub delayed_plan: DelayedUnitRelocationForCellsPlan,
    pub reset_outcome: ResetNotifyFlagsOutcome,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResetNotifyFlagsOutcome {
    pub reset_player_guids: Vec<ObjectGuid>,
    pub reset_creature_guids: Vec<ObjectGuid>,
    pub missing_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapObjectCellMoveState {
    None,
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapObjectMoveListEntry {
    pub guid: ObjectGuid,
    pub kind: AccessorObjectKind,
    pub move_state: MapObjectCellMoveState,
    pub new_position: Position,
    pub respawn_position: Option<Position>,
    pub is_pet: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapObjectMoveListPlan {
    pub relocated: Vec<ObjectGuid>,
    pub respawn_relocated: Vec<ObjectGuid>,
    pub remove_from_world: Vec<ObjectGuid>,
    pub pet_removed: Vec<ObjectGuid>,
    pub blocked_unloaded_grid: Vec<ObjectGuid>,
    pub reset_inactive_or_none: Vec<ObjectGuid>,
    pub skipped_not_in_world: Vec<ObjectGuid>,
    pub skipped_other_map_or_missing: Vec<ObjectGuid>,
    pub skipped_kind_mismatch: Vec<ObjectGuid>,
    pub failed_invalid_position: Vec<ObjectGuid>,
    pub failed_store: Vec<ObjectGuid>,
    pub unsupported_kind: Vec<ObjectGuid>,
}

fn is_active_object_like_cpp(kind: AccessorObjectKind, object: &WorldObject) -> bool {
    kind == AccessorObjectKind::Player || object.is_active()
}

fn push_in_world_guids<Terrain, Lifecycle>(
    map: &Map<Terrain, Lifecycle>,
    target: &mut Vec<ObjectGuid>,
    guids: impl IntoIterator<Item = ObjectGuid>,
) where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    target.extend(
        guids
            .into_iter()
            .filter(|guid| map.object_is_in_world(*guid)),
    );
}

fn sort_dedup(guids: &mut Vec<ObjectGuid>) {
    guids.sort();
    guids.dedup();
}

fn marked_cells_in_grid_like_cpp(
    grid: GridCoord,
    marked_cells: &HashSet<CellCoord>,
) -> Vec<CellCoord> {
    let cell_min_x = grid.x_coord * MAX_NUMBER_OF_CELLS;
    let cell_min_y = grid.y_coord * MAX_NUMBER_OF_CELLS;
    let cell_max_x = cell_min_x + MAX_NUMBER_OF_CELLS;
    let cell_max_y = cell_min_y + MAX_NUMBER_OF_CELLS;
    let mut cells = Vec::new();

    for x in cell_min_x..cell_max_x {
        for y in cell_min_y..cell_max_y {
            let cell = CellCoord::new(x, y);
            if marked_cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }

    cells
}

fn nearby_creature_guids_excluding(
    nearby: &NearbyCellGuids,
    excluded: ObjectGuid,
) -> Vec<ObjectGuid> {
    let mut nearby_creatures: Vec<_> = nearby
        .world
        .creatures
        .iter()
        .chain(nearby.grid.creatures.iter())
        .copied()
        .filter(|guid| *guid != excluded)
        .collect();
    nearby_creatures.sort();
    nearby_creatures.dedup();
    nearby_creatures
}

fn remove_list_grid_kind_like_cpp(kind: AccessorObjectKind) -> Option<GridObjectKind> {
    match kind {
        AccessorObjectKind::Creature | AccessorObjectKind::Pet => Some(GridObjectKind::Creature),
        AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
            Some(GridObjectKind::GameObject)
        }
        AccessorObjectKind::DynamicObject => Some(GridObjectKind::DynamicObject),
        AccessorObjectKind::AreaTrigger => Some(GridObjectKind::AreaTrigger),
        AccessorObjectKind::Corpse => Some(GridObjectKind::Corpse),
        AccessorObjectKind::SceneObject => Some(GridObjectKind::SceneObject),
        AccessorObjectKind::Conversation => Some(GridObjectKind::Conversation),
        AccessorObjectKind::Player => None,
    }
}

fn switch_list_unit_kind_like_cpp(kind: AccessorObjectKind) -> bool {
    matches!(kind, AccessorObjectKind::Creature | AccessorObjectKind::Pet)
}

fn set_record_temp_world_object_like_cpp(record: &mut MapObjectRecord, on: bool) {
    match record.kind() {
        AccessorObjectKind::Creature => {
            if let Some(creature) = record.creature_mut() {
                creature.set_temp_world_object_like_cpp(on);
            }
        }
        AccessorObjectKind::Pet => {
            if let Some(pet) = record.pet_mut() {
                pet.creature_mut().set_temp_world_object_like_cpp(on);
            }
        }
        _ => {}
    }
}

fn map_record_is_world_object_like_cpp(record: &MapObjectRecord) -> bool {
    if record.object().is_world_object() {
        return true;
    }
    if let Some(creature) = record.creature() {
        return creature.is_temp_world_object();
    }
    if let Some(pet) = record.pet() {
        return pet.creature().is_temp_world_object();
    }
    false
}

impl SwitchGridContainersOutcomeLikeCpp {
    const fn executed() -> Self {
        Self {
            executed: true,
            missing_or_stale: false,
            unsupported_kind: false,
            permanent_world_object: false,
            invalid_or_unloaded_grid: false,
        }
    }

    const fn missing_or_stale() -> Self {
        Self {
            executed: false,
            missing_or_stale: true,
            unsupported_kind: false,
            permanent_world_object: false,
            invalid_or_unloaded_grid: false,
        }
    }

    const fn unsupported_kind() -> Self {
        Self {
            executed: false,
            missing_or_stale: false,
            unsupported_kind: true,
            permanent_world_object: false,
            invalid_or_unloaded_grid: false,
        }
    }

    const fn permanent_world_object() -> Self {
        Self {
            executed: false,
            missing_or_stale: false,
            unsupported_kind: false,
            permanent_world_object: true,
            invalid_or_unloaded_grid: false,
        }
    }

    const fn invalid_or_unloaded_grid() -> Self {
        Self {
            executed: false,
            missing_or_stale: false,
            unsupported_kind: false,
            permanent_world_object: false,
            invalid_or_unloaded_grid: true,
        }
    }
}

fn cleanup_map_object_record_before_delete_like_cpp(
    record: &mut MapObjectRecord,
    kind: AccessorObjectKind,
    creature_second_cleanup: bool,
) -> usize {
    match kind {
        AccessorObjectKind::Creature => record.creature_mut().map_or(0, |creature| {
            if !creature_second_cleanup {
                creature.set_destroyed_object(true);
            }
            creature.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::Pet => record.pet_mut().map_or(0, |pet| {
            if !creature_second_cleanup {
                pet.creature_mut().set_destroyed_object(true);
            }
            pet.creature_mut().cleanup_before_delete();
            1
        }),
        AccessorObjectKind::GameObject => record.game_object_mut().map_or(0, |game_object| {
            game_object.set_destroyed_object(true);
            game_object.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::Transport => record.transport_mut().map_or(0, |transport| {
            transport.game_object_mut().set_destroyed_object(true);
            let _removed_static_passengers = transport.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::DynamicObject => {
            record.dynamic_object_mut().map_or(0, |dynamic_object| {
                dynamic_object.set_destroyed_object(true);
                dynamic_object.cleanup_before_delete();
                1
            })
        }
        AccessorObjectKind::AreaTrigger => record.area_trigger_mut().map_or(0, |area_trigger| {
            area_trigger.set_destroyed_object(true);
            area_trigger.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::Corpse => record.corpse_mut().map_or(0, |corpse| {
            corpse.set_destroyed_object(true);
            corpse.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::SceneObject => record.scene_object_mut().map_or(0, |scene_object| {
            scene_object.set_destroyed_object(true);
            scene_object.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::Conversation => record.conversation_mut().map_or(0, |conversation| {
            conversation.set_destroyed_object(true);
            conversation.cleanup_before_delete();
            1
        }),
        AccessorObjectKind::Player => {
            // No typed represented `CleanupsBeforeDelete` exists for Player in this
            // bounded map remove-list seam. Preserve at least the base
            // `WorldObject::SetDestroyedObject(true)` mutation and report no
            // represented cleanup.
            record.object_mut().object_mut().set_destroyed_object(true);
            0
        }
    }
}

fn insert_object_guid_in_cell_like_cpp(
    cell: &mut Cell,
    kind: AccessorObjectKind,
    is_world_object: bool,
    guid: ObjectGuid,
) {
    match kind {
        AccessorObjectKind::Player => {
            cell.world_objects.players.insert(guid);
        }
        AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
            if is_world_object {
                cell.world_objects.creatures.insert(guid);
            } else {
                cell.grid_objects.creatures.insert(guid);
            }
        }
        AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
            cell.grid_objects.gameobjects.insert(guid);
        }
        AccessorObjectKind::DynamicObject => {
            if is_world_object {
                cell.world_objects.dynamic_objects.insert(guid);
            } else {
                cell.grid_objects.dynamic_objects.insert(guid);
            }
        }
        AccessorObjectKind::AreaTrigger => {
            cell.grid_objects.area_triggers.insert(guid);
        }
        AccessorObjectKind::Corpse => {
            if is_world_object {
                cell.world_objects.corpses.insert(guid);
            } else {
                cell.grid_objects.corpses.insert(guid);
            }
        }
        AccessorObjectKind::SceneObject => {
            cell.grid_objects.scene_objects.insert(guid);
        }
        AccessorObjectKind::Conversation => {
            cell.grid_objects.conversations.insert(guid);
        }
    }
}

fn remove_object_guid_from_cell_like_cpp<Terrain, Lifecycle>(
    map: &mut Map<Terrain, Lifecycle>,
    grid: GridCoord,
    cell: &Cell,
    kind: AccessorObjectKind,
    is_world_object: bool,
    guid: ObjectGuid,
) -> bool
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    let Some(ngrid) = map.get_ngrid_mut(grid) else {
        return false;
    };
    let Some(local_cell) = ngrid.get_grid_type_mut(cell.cell_x(), cell.cell_y()) else {
        return false;
    };

    match kind {
        AccessorObjectKind::Player => local_cell.world_objects.players.remove(&guid),
        AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
            if is_world_object {
                local_cell.world_objects.creatures.remove(&guid)
            } else {
                local_cell.grid_objects.creatures.remove(&guid)
            }
        }
        AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
            local_cell.grid_objects.gameobjects.remove(&guid)
        }
        AccessorObjectKind::DynamicObject => {
            if is_world_object {
                local_cell.world_objects.dynamic_objects.remove(&guid)
            } else {
                local_cell.grid_objects.dynamic_objects.remove(&guid)
            }
        }
        AccessorObjectKind::AreaTrigger => local_cell.grid_objects.area_triggers.remove(&guid),
        AccessorObjectKind::Corpse => {
            if is_world_object {
                local_cell.world_objects.corpses.remove(&guid)
            } else {
                local_cell.grid_objects.corpses.remove(&guid)
            }
        }
        AccessorObjectKind::SceneObject => local_cell.grid_objects.scene_objects.remove(&guid),
        AccessorObjectKind::Conversation => local_cell.grid_objects.conversations.remove(&guid),
    }
}

impl<Terrain, Lifecycle> GridUnloadEntityStore for Map<Terrain, Lifecycle> {
    fn creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut Creature> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::creature_mut)
    }

    fn game_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut GameObject> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::game_object_mut)
    }

    fn dynamic_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut DynamicObject> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::dynamic_object_mut)
    }

    fn corpse_mut(&mut self, guid: ObjectGuid) -> Option<&mut Corpse> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::corpse_mut)
    }

    fn area_trigger_mut(&mut self, guid: ObjectGuid) -> Option<&mut AreaTrigger> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::area_trigger_mut)
    }

    fn scene_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut SceneObject> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::scene_object_mut)
    }

    fn conversation_mut(&mut self, guid: ObjectGuid) -> Option<&mut Conversation> {
        self.map_objects
            .get_mut(&guid)
            .and_then(MapObjectRecord::conversation_mut)
    }
}

impl<Terrain, Lifecycle> ObjectAccessorMapSource for Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    fn map_id(&self) -> u32 {
        self.map_id
    }

    fn instance_id(&self) -> u32 {
        self.instance_id
    }

    fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord> {
        self.map_objects.get(&guid)
    }
}

impl<Terrain, Lifecycle> WorldObjectEnvironment for Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader + MapWorldObjectEnvironment,
    Lifecycle: GridLifecycle,
{
    fn map_id(&self) -> u32 {
        self.map_id
    }

    fn instance_id(&self) -> u32 {
        self.instance_id
    }

    fn visibility_range(&self) -> f32 {
        self.visible_distance
    }

    fn line_of_sight(&self, query: LineOfSightQuery<'_>) -> bool {
        self.terrain.line_of_sight(query)
    }

    fn map_height(
        &self,
        object: &WorldObject,
        x: f32,
        y: f32,
        z: f32,
        query: WorldObjectHeightQuery,
    ) -> f32 {
        self.terrain.map_height(object, x, y, z, query)
    }

    fn floor_z(&self, object: &WorldObject, position: Position, max_search_dist: f32) -> f32 {
        self.terrain.floor_z(object, position, max_search_dist)
    }
}

impl<Terrain, Lifecycle> MapGridHost for Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    fn active_objects_near_grid(&self, grid: &NGrid) -> bool {
        Map::active_objects_near_grid(self, grid)
    }

    fn stop_grid_objects(&mut self, grid: &NGrid) {
        self.lifecycle.stop_grid_objects(grid);
        self.drain_grid_unload_actions_like_cpp();
    }

    fn reset_grid_expiry(&mut self, grid: &mut NGrid, factor: f32) {
        Map::reset_grid_expiry(self, grid, factor);
    }

    fn unload_grid(&mut self, grid: &mut NGrid, unload_all: bool) -> bool {
        if !self.can_unload_grid(grid, unload_all) {
            return false;
        }

        self.run_unload_lifecycle(grid, unload_all);
        self.grid_state_unloaded = true;
        true
    }
}

fn grid_index(coord: GridCoord) -> Option<usize> {
    coord
        .is_coord_valid()
        .then_some((coord.x_coord * MAX_NUMBER_OF_GRIDS + coord.y_coord) as usize)
}

fn checked_grid_index(coord: GridCoord) -> usize {
    grid_index(coord).expect("grid coordinates must be within MAX_NUMBER_OF_GRIDS")
}

fn terrain_grid_coords(coord: GridCoord) -> (u32, u32) {
    (
        (MAX_NUMBER_OF_GRIDS - 1) - coord.x_coord,
        (MAX_NUMBER_OF_GRIDS - 1) - coord.y_coord,
    )
}

fn active_cells_near_grid(
    active_cells: &HashSet<CellCoord>,
    visible_distance: f32,
    grid: &NGrid,
) -> bool {
    let mut cell_min = CellCoord::new(
        grid.x() as u32 * MAX_NUMBER_OF_CELLS,
        grid.y() as u32 * MAX_NUMBER_OF_CELLS,
    );
    let mut cell_max = CellCoord::new(
        cell_min.x_coord + MAX_NUMBER_OF_CELLS,
        cell_min.y_coord + MAX_NUMBER_OF_CELLS,
    );
    let cell_range = (visible_distance / SIZE_OF_GRID_CELL).ceil() as u32 + 1;

    cell_min.dec_x(cell_range);
    cell_min.dec_y(cell_range);
    cell_max.inc_x(cell_range);
    cell_max.inc_y(cell_range);

    active_cells.iter().any(|cell| {
        cell_min.x_coord <= cell.x_coord
            && cell.x_coord <= cell_max.x_coord
            && cell_min.y_coord <= cell.y_coord
            && cell.y_coord <= cell_max.y_coord
    })
}

fn pool_member_kind_to_spawn_object_type_like_cpp(
    kind: PoolMemberKindLikeCpp,
) -> Option<SpawnObjectType> {
    match kind {
        PoolMemberKindLikeCpp::Creature => Some(SpawnObjectType::Creature),
        PoolMemberKindLikeCpp::GameObject => Some(SpawnObjectType::GameObject),
        PoolMemberKindLikeCpp::Pool => None,
    }
}

pub fn is_grid_id_loaded<Terrain, Lifecycle>(map: &Map<Terrain, Lifecycle>, grid_id: u32) -> bool
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    map.is_grid_loaded(GridCoord::new(
        grid_id % MAX_NUMBER_OF_GRIDS,
        grid_id / MAX_NUMBER_OF_GRIDS,
    ))
}

pub fn cell_from_grid_center(coord: GridCoord) -> Cell {
    let cell = CellCoord::new(
        coord.x_coord * MAX_NUMBER_OF_CELLS,
        coord.y_coord * MAX_NUMBER_OF_CELLS,
    );
    Cell::from_cell_coord(cell)
}

pub fn cell_from_world(x: f32, y: f32) -> Cell {
    Cell::from_cell_coord(compute_cell_coord(x, y))
}

pub const fn total_cell_count() -> u32 {
    TOTAL_NUMBER_OF_CELLS_PER_MAP * TOTAL_NUMBER_OF_CELLS_PER_MAP
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid_unload::{
        GridObjectKind, GridUnloadAction, GridUnloadApplyOutcome, GuidGridUnloadLifecycle,
        apply_grid_unload_action, apply_grid_unload_actions,
    };
    use crate::pool::{PoolGroupLikeCpp, PoolTemplateDataLikeCpp};
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use wow_constants::{DeathState, TypeId, TypeMask};
    use wow_core::{ObjectGuid, Position, guid::HighGuid};
    use wow_entities::{
        AccessorObjectRef, Creature, GameObject, ObjectAccessor, ObjectNotifyFlags, Player,
    };

    #[derive(Debug, Default)]
    struct RecordingTerrain {
        loads: Vec<(u32, u32)>,
        unloads: Vec<(u32, u32)>,
    }

    impl TerrainGridLoader for RecordingTerrain {
        fn load_map_and_vmap(&mut self, grid_x: u32, grid_y: u32) {
            self.loads.push((grid_x, grid_y));
        }

        fn unload_map(&mut self, grid_x: u32, grid_y: u32) {
            self.unloads.push((grid_x, grid_y));
        }
    }

    impl MapWorldObjectEnvironment for RecordingTerrain {
        fn line_of_sight(&self, _query: LineOfSightQuery<'_>) -> bool {
            true
        }

        fn map_height(
            &self,
            _object: &WorldObject,
            _x: f32,
            _y: f32,
            _z: f32,
            _query: WorldObjectHeightQuery,
        ) -> f32 {
            INVALID_HEIGHT
        }

        fn floor_z(
            &self,
            _object: &WorldObject,
            _position: Position,
            _max_search_dist: f32,
        ) -> f32 {
            INVALID_HEIGHT
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct LosCall {
        source_guid: ObjectGuid,
        target_guid: Option<ObjectGuid>,
        from: Position,
        to: Position,
        check_dynamic: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct HeightCall {
        object_guid: ObjectGuid,
        x: f32,
        y: f32,
        z: f32,
        query: WorldObjectHeightQuery,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct FloorCall {
        object_guid: ObjectGuid,
        position: Position,
        max_search_dist: f32,
    }

    #[derive(Debug)]
    struct RecordingWorldObjectTerrain {
        los_result: bool,
        height_result: f32,
        floor_result: f32,
        los_calls: RefCell<Vec<LosCall>>,
        height_calls: RefCell<Vec<HeightCall>>,
        floor_calls: RefCell<Vec<FloorCall>>,
    }

    impl RecordingWorldObjectTerrain {
        fn new(los_result: bool, height_result: f32, floor_result: f32) -> Self {
            Self {
                los_result,
                height_result,
                floor_result,
                los_calls: RefCell::new(Vec::new()),
                height_calls: RefCell::new(Vec::new()),
                floor_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl TerrainGridLoader for RecordingWorldObjectTerrain {
        fn load_map_and_vmap(&mut self, _grid_x: u32, _grid_y: u32) {}
        fn unload_map(&mut self, _grid_x: u32, _grid_y: u32) {}
    }

    impl MapWorldObjectEnvironment for RecordingWorldObjectTerrain {
        fn line_of_sight(&self, query: LineOfSightQuery<'_>) -> bool {
            self.los_calls.borrow_mut().push(LosCall {
                source_guid: query.source.guid(),
                target_guid: query.target.map(WorldObject::guid),
                from: query.from.position,
                to: query.to.position,
                check_dynamic: query.options.check_dynamic,
            });
            self.los_result
        }

        fn map_height(
            &self,
            object: &WorldObject,
            x: f32,
            y: f32,
            z: f32,
            query: WorldObjectHeightQuery,
        ) -> f32 {
            self.height_calls.borrow_mut().push(HeightCall {
                object_guid: object.guid(),
                x,
                y,
                z,
                query,
            });
            self.height_result
        }

        fn floor_z(&self, object: &WorldObject, position: Position, max_search_dist: f32) -> f32 {
            self.floor_calls.borrow_mut().push(FloorCall {
                object_guid: object.guid(),
                position,
                max_search_dist,
            });
            self.floor_result
        }
    }

    #[derive(Debug, Default)]
    struct RecordingLifecycle {
        loads: usize,
        stops: usize,
        evacuates: usize,
        cleans: usize,
        unloads: usize,
    }

    impl GridLifecycle for RecordingLifecycle {
        fn load_grid_objects(&mut self, _grid: &mut NGrid, _cell: &Cell) {
            self.loads += 1;
        }

        fn stop_grid_objects(&mut self, _grid: &NGrid) {
            self.stops += 1;
        }

        fn evacuate_grid(&mut self, _grid: &mut NGrid) {
            self.evacuates += 1;
        }

        fn clean_grid(&mut self, _grid: &mut NGrid) {
            self.cleans += 1;
        }

        fn unload_grid_objects(&mut self, _grid: &mut NGrid) {
            self.unloads += 1;
        }
    }

    fn test_map() -> Map<RecordingTerrain, RecordingLifecycle> {
        Map::with_hooks(
            571,
            7,
            1,
            1000,
            true,
            100.0,
            RecordingTerrain::default(),
            RecordingLifecycle::default(),
        )
    }

    fn guid_unload_test_map() -> Map<RecordingTerrain, GuidGridUnloadLifecycle> {
        Map::with_hooks(
            571,
            7,
            1,
            1000,
            true,
            100.0,
            RecordingTerrain::default(),
            GuidGridUnloadLifecycle::new(),
        )
    }

    fn world_object_environment_test_map(
        terrain: RecordingWorldObjectTerrain,
        visible_distance: f32,
    ) -> Map<RecordingWorldObjectTerrain, RecordingLifecycle> {
        Map::with_hooks(
            571,
            7,
            1,
            1000,
            true,
            visible_distance,
            terrain,
            RecordingLifecycle::default(),
        )
    }

    #[test]
    fn guid_sequence_creature_starts_at_one_like_cpp() {
        let mut map = test_map();

        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::Creature), Ok(1));
        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::Creature), Ok(2));
        assert_eq!(map.get_max_low_guid_like_cpp(HighGuid::Creature), Ok(3));
    }

    #[test]
    fn guid_sequence_creature_and_gameobject_are_independent_like_cpp() {
        let mut map = test_map();

        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::Creature), Ok(1));
        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::GameObject), Ok(1));
        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::Creature), Ok(2));
        assert_eq!(map.get_max_low_guid_like_cpp(HighGuid::GameObject), Ok(2));
    }

    #[test]
    fn guid_sequence_accepts_non_creature_gameobject_map_sources_like_cpp() {
        let mut map = test_map();

        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::AreaTrigger), Ok(1));
        assert_eq!(
            map.generate_low_guid_like_cpp(HighGuid::DynamicObject),
            Ok(1)
        );
        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::AreaTrigger), Ok(2));
        assert_eq!(
            map.get_max_low_guid_like_cpp(HighGuid::DynamicObject),
            Ok(2)
        );
    }

    #[test]
    fn guid_sequence_is_map_instance_local_like_cpp() {
        let mut first_map = test_map();
        let mut second_map = test_map();

        assert_eq!(
            first_map.generate_low_guid_like_cpp(HighGuid::Creature),
            Ok(1)
        );
        assert_eq!(
            first_map.generate_low_guid_like_cpp(HighGuid::Creature),
            Ok(2)
        );
        assert_eq!(
            second_map.generate_low_guid_like_cpp(HighGuid::Creature),
            Ok(1)
        );
        assert_eq!(
            second_map.get_max_low_guid_like_cpp(HighGuid::Creature),
            Ok(2)
        );
    }

    #[test]
    fn guid_sequence_transport_can_be_set_for_future_global_sync_like_cpp() {
        let mut map = test_map();

        assert_eq!(
            map.set_guid_sequence_like_cpp(HighGuid::Transport, 77),
            Ok(())
        );
        assert_eq!(map.generate_low_guid_like_cpp(HighGuid::Transport), Ok(77));
        assert_eq!(map.get_max_low_guid_like_cpp(HighGuid::Transport), Ok(78));
    }

    #[test]
    fn guid_sequence_rejects_non_map_local_high_guid_like_cpp() {
        let mut map = test_map();

        assert_eq!(
            map.generate_low_guid_like_cpp(HighGuid::Player),
            Err(MapGuidSequenceErrorLikeCpp::UnsupportedSequenceSource {
                high: HighGuid::Player,
            })
        );
    }

    #[test]
    fn urand_inclusive_like_cpp_stays_within_inclusive_bounds() {
        let mut map = test_map();
        map.seed_creature_level_rng_for_tests_like_cpp(0x407);

        let mut saw_min = false;
        let mut saw_max = false;
        for _ in 0..512 {
            let value = map.urand_inclusive_like_cpp(18, 20);
            assert!((18..=20).contains(&value));
            saw_min |= value == 18;
            saw_max |= value == 20;
        }

        assert!(saw_min, "inclusive C++ urand should be able to return min");
        assert!(saw_max, "inclusive C++ urand should be able to return max");
    }

    #[test]
    #[should_panic(expected = "C++ urand requires max >= min")]
    fn urand_inclusive_like_cpp_asserts_max_at_least_min_like_cpp() {
        let mut map = test_map();
        let _ = map.urand_inclusive_like_cpp(20, 18);
    }

    #[test]
    fn select_creature_level_fixed_path_does_not_consume_rng_like_cpp() {
        let mut fixed_then_variable = test_map();
        fixed_then_variable.seed_creature_level_rng_for_tests_like_cpp(0x407);
        assert_eq!(
            fixed_then_variable.select_creature_level_like_cpp(19, 19),
            19
        );
        let after_fixed = fixed_then_variable.select_creature_level_like_cpp(18, 20);

        let mut variable_only = test_map();
        variable_only.seed_creature_level_rng_for_tests_like_cpp(0x407);
        let without_fixed = variable_only.select_creature_level_like_cpp(18, 20);

        assert_eq!(after_fixed, without_fixed);
        assert!((18..=20).contains(&after_fixed));
    }

    #[test]
    fn map_init_pools_for_map_mutates_map_owned_pool_data_like_cpp() {
        let mut pool_mgr = PoolMgrLikeCpp::new();
        pool_mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 0.0), 1);
        pool_mgr
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, group)
            .expect("test pool group");
        pool_mgr.add_auto_spawn_pool_like_cpp(571, 10);
        let mut map = test_map();

        let plan = map.init_pools_for_map_like_cpp(
            &pool_mgr,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        );

        assert_eq!(plan.map_id, 571);
        assert_eq!(plan.planned(), 1);
        assert!(map.pool_data_like_cpp().is_spawned_creature_like_cpp(101));
        assert_eq!(map.pool_data_like_cpp().get_spawned_objects_like_cpp(10), 1);
    }

    #[test]
    fn world_object_visibility_range_reads_map_visible_distance_like_cpp() {
        let map = world_object_environment_test_map(
            RecordingWorldObjectTerrain::new(true, INVALID_HEIGHT, INVALID_HEIGHT),
            123.5,
        );
        let object = world_object(HighGuid::DynamicObject, 571, 7, true);

        assert_eq!(object.get_visibility_range(&map), 123.5);
    }

    #[test]
    fn world_object_los_delegates_to_map_environment_hook_like_cpp() {
        let map = world_object_environment_test_map(
            RecordingWorldObjectTerrain::new(false, INVALID_HEIGHT, INVALID_HEIGHT),
            100.0,
        );
        let mut source = world_object(HighGuid::DynamicObject, 571, 7, true);
        source.relocate(Position::new(1.0, 2.0, 3.0, 0.25));
        let mut target = world_object_with_counter(HighGuid::GameObject, 2, 571, 7, true);
        target.relocate(Position::new(4.0, 5.0, 6.0, 0.75));

        let result = source.is_within_los_in_map(
            &target,
            &map,
            wow_entities::LineOfSightOptions {
                check_dynamic: true,
            },
        );

        assert!(!result);
        assert_eq!(
            map.terrain().los_calls.borrow().as_slice(),
            &[LosCall {
                source_guid: source.guid(),
                target_guid: Some(target.guid()),
                from: source.position(),
                to: target.position(),
                check_dynamic: true,
            }]
        );
    }

    #[test]
    fn world_object_map_height_and_floor_delegate_to_map_environment_hook_like_cpp() {
        let map = world_object_environment_test_map(
            RecordingWorldObjectTerrain::new(true, 88.0, 25.0),
            100.0,
        );
        let mut object = world_object(HighGuid::DynamicObject, 571, 7, true);
        object.relocate(Position::new(1.0, 2.0, 3.0, 0.25));
        object.set_static_floor_z(20.0);
        let height_query = WorldObjectHeightQuery {
            vmap: false,
            distance_to_search: 9.0,
        };

        let height = object.get_map_height(&map, 4.0, 5.0, 6.0, height_query);
        let floor = object.get_floor_z(&map);

        assert_eq!(height, 88.0);
        assert_eq!(floor, 25.0);
        assert_eq!(
            map.terrain().height_calls.borrow().as_slice(),
            &[HeightCall {
                object_guid: object.guid(),
                x: 4.0,
                y: 5.0,
                z: 6.5,
                query: height_query,
            }]
        );
        assert_eq!(
            map.terrain().floor_calls.borrow().as_slice(),
            &[FloorCall {
                object_guid: object.guid(),
                position: Position::new(1.0, 2.0, 3.5, 0.25),
                max_search_dist: 50.0,
            }]
        );
    }

    fn spawn_group(group_id: u32, flags: SpawnGroupFlags) -> SpawnGroupTemplateData {
        SpawnGroupTemplateData {
            group_id,
            name: format!("group-{group_id}"),
            map_id: 571,
            flags,
        }
    }

    const fn spawn_group_flags(left: SpawnGroupFlags, right: SpawnGroupFlags) -> SpawnGroupFlags {
        SpawnGroupFlags(left.0 | right.0)
    }

    fn spawn_data(
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        spawn_group: SpawnGroupTemplateData,
    ) -> crate::spawn::SpawnData {
        crate::spawn::SpawnData {
            object_type,
            spawn_id,
            map_id: 571,
            db_data: true,
            spawn_group,
            id: 99,
            spawn_point: crate::spawn::SpawnPosition::new(0.0, 0.0, 0.0, 0.0),
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: 0,
            pool_id: 0,
            spawn_time_secs: 0,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        }
    }

    fn spawn_group_store(
        group: SpawnGroupTemplateData,
        mut spawns: Vec<crate::spawn::SpawnData>,
    ) -> (SpawnGroupTemplateData, SpawnStore) {
        let mut store = SpawnStore::new();
        let mut templates = BTreeMap::from([(group.group_id, group.clone())]);
        let rows = spawns
            .iter()
            .map(|spawn| crate::spawn::SpawnGroupMemberRow {
                group_id: group.group_id,
                spawn_type: spawn.object_type as u8,
                spawn_id: spawn.spawn_id,
            })
            .collect::<Vec<_>>();
        for spawn in &spawns {
            match spawn.object_type {
                SpawnObjectType::Creature | SpawnObjectType::GameObject => {
                    store.add_object_spawn(spawn, |_| false);
                }
                SpawnObjectType::AreaTrigger => store.add_area_trigger_spawn(spawn),
            }
        }
        store.apply_spawn_groups_like_cpp(&mut templates, rows);
        for spawn in &mut spawns {
            spawn.spawn_group = templates
                .get(&group.group_id)
                .expect("group resolved")
                .clone();
        }
        (
            templates
                .get(&group.group_id)
                .expect("group resolved")
                .clone(),
            store,
        )
    }

    fn respawn_info(
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        respawn_time: i64,
    ) -> RespawnInfoLikeCpp {
        RespawnInfoLikeCpp {
            object_type,
            spawn_id,
            entry: 42,
            respawn_time,
            grid_id: 7,
        }
    }

    fn test_creature_for_spawn(spawn_id: SpawnId, counter: i64, alive: bool) -> Creature {
        let mut creature = Creature::new(false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::Creature, counter));
        creature.unit_mut().world_mut().object_mut().set_entry(42);
        creature.unit_mut().world_mut().set_map(571, 7).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        creature.unit_mut().world_mut().object_mut().add_to_world();
        creature.unit_mut().set_death_state(DeathState::Alive);
        creature.unit_mut().set_max_health(100);
        creature.unit_mut().set_health(100);
        creature.set_spawn_id(spawn_id);
        if !alive {
            creature.mark_ai_dead(1);
        }
        creature
    }

    fn test_gameobject_for_spawn(spawn_id: SpawnId, counter: i64) -> GameObject {
        let mut gameobject = GameObject::new();
        gameobject
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::GameObject, counter));
        gameobject.world_mut().object_mut().set_entry(42);
        gameobject.world_mut().set_map(571, 7).unwrap();
        gameobject
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        gameobject.world_mut().object_mut().add_to_world();
        gameobject.set_spawn_id(spawn_id);
        gameobject
    }

    fn test_area_trigger_for_spawn(spawn_id: SpawnId, counter: i64) -> AreaTrigger {
        let mut area_trigger = AreaTrigger::new();
        area_trigger
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::AreaTrigger, counter));
        area_trigger.world_mut().object_mut().set_entry(42);
        area_trigger.world_mut().set_map(571, 7).unwrap();
        area_trigger
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        area_trigger.world_mut().object_mut().add_to_world();
        area_trigger.set_spawn_id(spawn_id);
        area_trigger
    }

    #[test]
    fn grid_unload_actions_apply_to_map_owned_creature_record() {
        let mut map = test_map();
        let creature_guid = guid(HighGuid::Creature, 3711);
        let mut creature = test_creature_for_spawn(371, 3711, true);
        creature.unit_mut().world_mut().set_current_cell(3, 4);
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let outcomes = apply_grid_unload_actions(
            &mut map,
            [
                GridUnloadAction::CreatureRespawnRelocation(creature_guid),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature_guid),
                GridUnloadAction::DeleteObject(GridObjectKind::Creature, creature_guid),
            ],
        );

        assert_eq!(outcomes, vec![GridUnloadApplyOutcome::Applied; 3]);
        assert_eq!(map.map_object_count(), 1);
        let creature = map
            .map_object_record(creature_guid)
            .unwrap()
            .creature()
            .unwrap();
        assert!(creature.grid_unload_respawn_relocation_requested());
        assert_eq!(creature.cleanup_before_delete_count(), 1);
        assert!(creature.grid_unload_delete_requested());
        assert_eq!(creature.unit().world().current_cell(), None);
    }

    #[test]
    fn grid_unload_actions_apply_to_map_owned_gameobject_record() {
        let mut map = test_map();
        let go_guid = guid(HighGuid::GameObject, 3712);
        let mut gameobject = test_gameobject_for_spawn(372, 3712);
        gameobject.world_mut().set_current_cell(5, 6);
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        let outcomes = apply_grid_unload_actions(
            &mut map,
            [
                GridUnloadAction::GameObjectRespawnRelocation(go_guid),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::GameObject, go_guid),
                GridUnloadAction::DeleteObject(GridObjectKind::GameObject, go_guid),
            ],
        );

        assert_eq!(outcomes, vec![GridUnloadApplyOutcome::Applied; 3]);
        assert_eq!(map.map_object_count(), 1);
        let gameobject = map
            .map_object_record(go_guid)
            .unwrap()
            .game_object()
            .unwrap();
        assert!(gameobject.grid_unload_respawn_relocation_requested());
        assert_eq!(gameobject.cleanup_before_delete_count(), 1);
        assert!(gameobject.grid_unload_delete_requested());
        assert_eq!(gameobject.world().current_cell(), None);
    }

    #[test]
    fn grid_unload_map_store_missing_and_kind_mismatch_are_best_effort() {
        let mut map = test_map();
        let go_guid = guid(HighGuid::GameObject, 3713);
        let gameobject = test_gameobject_for_spawn(373, 3713);
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        assert_eq!(
            apply_grid_unload_action(
                &mut map,
                GridUnloadAction::CreatureRespawnRelocation(go_guid),
            ),
            GridUnloadApplyOutcome::MissingEntity
        );
        assert_eq!(
            apply_grid_unload_action(
                &mut map,
                GridUnloadAction::CreatureRespawnRelocation(guid(HighGuid::Creature, 3714)),
            ),
            GridUnloadApplyOutcome::MissingEntity
        );

        let gameobject = map
            .map_object_record(go_guid)
            .unwrap()
            .game_object()
            .unwrap();
        assert!(!gameobject.grid_unload_respawn_relocation_requested());
        assert_eq!(gameobject.cleanup_before_delete_count(), 0);
        assert!(!gameobject.grid_unload_delete_requested());
    }

    #[test]
    fn map_owned_respawn_get_time_zero_area_trigger_and_inserted_timers_like_cpp() {
        let mut map = test_map();

        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::AreaTrigger, 10),
            0
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::AreaTrigger, 10, 100)),
            AddRespawnInfoOutcomeLikeCpp::RejectedUnsupportedType
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 100)),
            AddRespawnInfoOutcomeLikeCpp::Inserted
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 20, 200)),
            AddRespawnInfoOutcomeLikeCpp::Inserted
        );

        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            100
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            200
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::AreaTrigger, 10),
            0
        );
    }

    #[test]
    fn map_owned_respawn_add_replace_remove_unload_and_timer_keys_like_cpp() {
        let mut map = test_map();

        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 100)),
            AddRespawnInfoOutcomeLikeCpp::Inserted
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 150)),
            AddRespawnInfoOutcomeLikeCpp::RejectedExistingSoonerOrEqual
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            100
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 90)),
            AddRespawnInfoOutcomeLikeCpp::ReplacedExisting
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            90
        );
        assert_eq!(
            map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 20, 80)),
            AddRespawnInfoOutcomeLikeCpp::Inserted
        );

        let timer_keys = map.respawn_timer_keys_like_cpp().collect::<Vec<_>>();
        assert_eq!(
            timer_keys,
            vec![
                (SpawnObjectType::GameObject, 20),
                (SpawnObjectType::Creature, 10)
            ]
        );

        let removed = map.remove_respawn_time_like_cpp(SpawnObjectType::Creature, 10);
        assert_eq!(removed.map(|info| info.respawn_time), Some(90));
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            0
        );
        assert_eq!(
            map.respawn_timer_keys_like_cpp().collect::<Vec<_>>(),
            vec![(SpawnObjectType::GameObject, 20)]
        );

        map.unload_all_respawn_infos_like_cpp();
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            0
        );
        assert!(map.respawn_timer_keys_like_cpp().next().is_none());
    }

    #[test]
    fn map_owned_respawn_grid_load_state_uses_map_timer_and_group_sources_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(12, SpawnGroupFlags::MANUAL_SPAWN);
        let spawn = spawn_data(SpawnObjectType::Creature, 42, manual.clone());
        store.add_object_spawn(&spawn, |_| false);

        assert!(
            !map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );

        map.set_spawn_group_active_like_cpp(Some(&manual), true);
        assert!(
            map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );

        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 42, 100));
        assert!(
            !map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );

        map.remove_respawn_time_like_cpp(SpawnObjectType::Creature, 42);
        assert!(
            map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );
    }

    #[test]
    fn spawned_pool_data_creature_gameobject_and_dispatcher_like_cpp() {
        let mut pool_data = SpawnedPoolDataLikeCpp::new();

        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 0);
        assert_eq!(
            pool_data.is_spawned_object_like_cpp(SpawnObjectType::Creature, 101),
            Ok(false)
        );
        assert_eq!(
            pool_data.is_spawned_object_like_cpp(SpawnObjectType::GameObject, 202),
            Ok(false)
        );
        assert_eq!(
            pool_data.is_spawned_object_like_cpp(SpawnObjectType::AreaTrigger, 303),
            Err(SpawnedPoolDataErrorLikeCpp::UnsupportedSpawnObjectType(
                SpawnObjectType::AreaTrigger
            ))
        );

        assert_eq!(
            pool_data.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 7),
            Ok(())
        );
        assert_eq!(
            pool_data.add_spawn_like_cpp(SpawnObjectType::GameObject, 202, 7),
            Ok(())
        );
        assert!(pool_data.is_spawned_creature_like_cpp(101));
        assert!(pool_data.is_spawned_gameobject_like_cpp(202));
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 2);
        assert_eq!(
            pool_data.spawned_objects_like_cpp(),
            vec![
                (SpawnObjectType::Creature, 101),
                (SpawnObjectType::GameObject, 202),
            ]
        );
    }

    #[test]
    fn spawned_pool_data_duplicate_add_and_remove_counter_semantics_like_cpp() {
        let mut pool_data = SpawnedPoolDataLikeCpp::new();

        assert_eq!(
            pool_data.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 7),
            Ok(())
        );
        assert_eq!(
            pool_data.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 7),
            Ok(())
        );
        assert!(pool_data.is_spawned_creature_like_cpp(101));
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 2);

        assert_eq!(
            pool_data.remove_spawn_like_cpp(SpawnObjectType::Creature, 101, 7),
            Ok(())
        );
        assert!(!pool_data.is_spawned_creature_like_cpp(101));
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 1);

        assert_eq!(
            pool_data.remove_spawn_like_cpp(SpawnObjectType::Creature, 101, 7),
            Ok(())
        );
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 0);
        assert_eq!(
            pool_data.remove_spawn_like_cpp(SpawnObjectType::GameObject, 202, 99),
            Ok(())
        );
        assert_eq!(pool_data.get_spawned_objects_like_cpp(99), 0);
    }

    #[test]
    fn spawned_pool_data_pool_subpool_membership_and_counts_like_cpp() {
        let mut pool_data = SpawnedPoolDataLikeCpp::new();

        pool_data.add_pool_spawn_like_cpp(70, 7);
        assert!(pool_data.is_spawned_pool_like_cpp(70));
        assert_eq!(pool_data.get_spawned_objects_like_cpp(70), 0);
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 1);

        pool_data.remove_pool_spawn_like_cpp(70, 7);
        assert!(!pool_data.is_spawned_pool_like_cpp(70));
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 0);

        pool_data.remove_pool_spawn_like_cpp(70, 7);
        assert_eq!(pool_data.get_spawned_objects_like_cpp(7), 0);
    }

    #[test]
    fn grid_load_state_uses_map_pool_data_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(14, SpawnGroupFlags::NONE);
        let mut creature_spawn = spawn_data(SpawnObjectType::Creature, 501, active.clone());
        creature_spawn.pool_id = 7;
        let mut gameobject_spawn = spawn_data(SpawnObjectType::GameObject, 502, active);
        gameobject_spawn.pool_id = 7;
        store.add_object_spawn(&creature_spawn, |_| false);
        store.add_object_spawn(&gameobject_spawn, |_| false);

        let grid_state = map.spawn_grid_load_state_like_cpp(&store);
        assert!(!grid_state.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 501));
        assert!(!grid_state.should_be_spawned_on_grid_load(SpawnObjectType::GameObject, 502));

        assert_eq!(
            map.pool_data_mut_like_cpp()
                .add_spawn_like_cpp(SpawnObjectType::Creature, 501, 7),
            Ok(())
        );
        let grid_state = map.spawn_grid_load_state_like_cpp(&store);
        assert!(grid_state.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 501));
        assert!(!grid_state.should_be_spawned_on_grid_load(SpawnObjectType::GameObject, 502));

        assert_eq!(
            map.pool_data_mut_like_cpp()
                .add_spawn_like_cpp(SpawnObjectType::GameObject, 502, 7),
            Ok(())
        );
        let grid_state = map.spawn_grid_load_state_like_cpp(&store);
        assert!(grid_state.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 501));
        assert!(grid_state.should_be_spawned_on_grid_load(SpawnObjectType::GameObject, 502));

        assert_eq!(
            map.pool_data_mut_like_cpp()
                .remove_spawn_like_cpp(SpawnObjectType::Creature, 501, 7),
            Ok(())
        );
        let grid_state = map.spawn_grid_load_state_like_cpp(&store);
        assert!(!grid_state.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 501));
        assert!(grid_state.should_be_spawned_on_grid_load(SpawnObjectType::GameObject, 502));
    }

    #[test]
    fn map_owned_respawn_process_due_respawns_delegates_to_owned_store_like_cpp() {
        let mut map = test_map();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 100));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 20, 200));

        let actions = map.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |_| CheckRespawnOutcomeLikeCpp::Allowed,
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::DoRespawn {
                object_type: SpawnObjectType::Creature,
                spawn_id: 10,
                grid_id: 7,
            }]
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            200
        );

        let future_actions = map.process_due_respawns_like_cpp(
            150,
            |_, _| None,
            |_| CheckRespawnOutcomeLikeCpp::Allowed,
        );
        assert!(future_actions.is_empty());
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            200
        );
    }

    #[test]
    fn process_respawns_delete_only_inactive_spawn_group_removes_map_owned_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(12, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 42, manual), |_| {
            false
        });
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 42, 100));

        let summary = map.process_due_respawns_spawn_group_delete_only_like_cpp(100, &store);

        assert_eq!(summary.deleted_inactive_spawn_group, 1);
        assert_eq!(summary.blocked_missing_spawn_data, 0);
        assert_eq!(summary.blocked_pool_runtime, 0);
        assert_eq!(summary.blocked_do_respawn_runtime, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 42),
            0
        );
    }

    #[test]
    fn process_respawns_delete_only_active_due_timer_loaded_grid_blocks_do_respawn_and_preserves_timer_like_cpp()
     {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(13, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 43, active), |_| {
            false
        });
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 43, 100));

        let summary = map.process_due_respawns_spawn_group_delete_only_like_cpp(100, &store);

        assert_eq!(summary.deleted_inactive_spawn_group, 0);
        assert_eq!(summary.processed_unloaded_grid_respawns, 0);
        assert_eq!(summary.blocked_do_respawn_runtime, 1);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 43),
            100
        );
    }

    #[test]
    fn process_respawns_allowed_unloaded_grid_removes_timer_and_continues_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(16, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 47, active.clone()),
            |_| false,
        );
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 48, active), |_| {
            false
        });
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 47, 90));
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::GameObject,
            spawn_id: 48,
            entry: 42,
            respawn_time: 100,
            grid_id: 8,
        });

        let summary = map.process_due_respawns_spawn_group_delete_only_like_cpp(100, &store);

        assert_eq!(summary.processed_unloaded_grid_respawns, 2);
        assert_eq!(summary.blocked_do_respawn_runtime, 0);
        assert_eq!(summary.deleted_inactive_spawn_group, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 47),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 48),
            0
        );
        assert!(
            map.get_respawn_info_like_cpp(SpawnObjectType::Creature, 47)
                .is_none()
        );
        assert!(
            map.get_respawn_info_like_cpp(SpawnObjectType::GameObject, 48)
                .is_none()
        );
    }

    #[test]
    fn process_respawns_delete_only_missing_metadata_preserves_timer_like_cpp() {
        let mut map = test_map();
        let store = SpawnStore::new();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 44, 100));

        let summary = map.process_due_respawns_spawn_group_delete_only_like_cpp(100, &store);

        assert_eq!(summary.deleted_inactive_spawn_group, 0);
        assert_eq!(summary.blocked_missing_spawn_data, 1);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 44),
            100
        );
    }

    #[test]
    fn process_respawns_loaded_grid_creature_loader_adds_record_and_removes_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(397, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 39701, active),
            |_| false,
        );
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 39701, 100));
        let expected_guid = guid(HighGuid::Creature, 3970101);
        let mut loader_calls = 0;

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |map, object_type, spawn_id| {
                loader_calls += 1;
                assert_eq!(object_type, SpawnObjectType::Creature);
                assert_eq!(spawn_id, 39701);
                let low = map
                    .generate_low_guid_like_cpp(HighGuid::Creature)
                    .expect("map-owned Creature low-guid allocator");
                assert_eq!(low, 1);
                let mut creature = test_creature_for_spawn(39701, 3970101, true);
                creature
                    .unit_mut()
                    .world_mut()
                    .object_mut()
                    .remove_from_world();
                Some(LoadedGridRespawnRecordsLikeCpp::primary_only(
                    MapObjectRecord::new_creature(creature).unwrap(),
                ))
            },
        );

        assert_eq!(loader_calls, 1);
        assert_eq!(summary.executed_loaded_grid_respawns, 1);
        assert_eq!(summary.blocked_loaded_grid_respawn_loads, 0);
        assert_eq!(summary.blocked_loaded_grid_respawn_add_to_map, 0);
        assert_eq!(summary.blocked_do_respawn_runtime, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 39701),
            0
        );
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(39701), 1);
        let record = map.map_object_record(expected_guid).unwrap();
        assert!(record.object().object().is_in_world());
        assert!(record.creature().is_some());
        assert!(map.get_creature_by_spawn_id_like_cpp(39701).is_some());
        let cell = Cell::from_world(record.object().position().x, record.object().position().y);
        let grid = map
            .get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
            .unwrap();
        let local_cell = grid
            .get_grid_type(cell.cell_x(), cell.cell_y())
            .expect("record inserted into target cell");
        assert!(local_cell.grid_objects.creatures.contains(&expected_guid));
    }

    #[test]
    fn process_respawns_loaded_grid_gameobject_loader_adds_record_and_removes_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(398, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::GameObject, 39801, active),
            |_| false,
        );
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 39801, 100));
        let expected_guid = guid(HighGuid::GameObject, 3980101);

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |_map, object_type, spawn_id| {
                assert_eq!(object_type, SpawnObjectType::GameObject);
                assert_eq!(spawn_id, 39801);
                let mut gameobject = test_gameobject_for_spawn(39801, 3980101);
                gameobject.world_mut().object_mut().remove_from_world();
                Some(LoadedGridRespawnRecordsLikeCpp::primary_only(
                    MapObjectRecord::new_game_object(gameobject).unwrap(),
                ))
            },
        );

        assert_eq!(summary.executed_loaded_grid_respawns, 1);
        assert_eq!(summary.blocked_loaded_grid_respawn_loads, 0);
        assert_eq!(summary.blocked_loaded_grid_respawn_add_to_map, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 39801),
            0
        );
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(39801), 1);
        let record = map.map_object_record(expected_guid).unwrap();
        assert!(record.object().object().is_in_world());
        assert!(record.game_object().is_some());
        assert!(map.get_gameobject_by_spawn_id_like_cpp(39801).is_some());
        let cell = Cell::from_world(record.object().position().x, record.object().position().y);
        let grid = map
            .get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
            .unwrap();
        let local_cell = grid
            .get_grid_type(cell.cell_x(), cell.cell_y())
            .expect("record inserted into target cell");
        assert!(local_cell.grid_objects.gameobjects.contains(&expected_guid));
    }

    #[test]
    fn process_respawns_loaded_grid_pre_add_records_are_best_effort_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(409, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::GameObject, 40901, active),
            |_| false,
        );
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 40901, 100));
        let owner_guid = guid(HighGuid::GameObject, 4090101);
        let trap_guid = guid(HighGuid::GameObject, 4090102);
        let missing_trap_guid = guid(HighGuid::GameObject, 4090103);

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |_map, object_type, spawn_id| {
                assert_eq!(object_type, SpawnObjectType::GameObject);
                assert_eq!(spawn_id, 40901);
                let mut trap = test_gameobject_for_spawn(0, 4090102);
                trap.world_mut().object_mut().remove_from_world();
                let mut missing_trap = test_gameobject_for_spawn(0, 4090103);
                missing_trap.world_mut().object_mut().remove_from_world();
                missing_trap
                    .world_mut()
                    .relocate(Position::xyz(1_000_000.0, 1_000_000.0, 0.0));
                let mut owner = test_gameobject_for_spawn(40901, 4090101);
                owner.world_mut().object_mut().remove_from_world();
                owner.set_linked_trap_like_cpp(trap_guid);
                Some(LoadedGridRespawnRecordsLikeCpp {
                    pre_add_records: vec![
                        MapObjectRecord::new_game_object(trap).unwrap(),
                        MapObjectRecord::new_game_object(missing_trap).unwrap(),
                    ],
                    primary_record: MapObjectRecord::new_game_object(owner).unwrap(),
                })
            },
        );

        assert_eq!(summary.executed_loaded_grid_respawns, 1);
        assert_eq!(summary.blocked_loaded_grid_respawn_add_to_map, 0);
        assert!(map.map_object_record(owner_guid).is_some());
        assert!(map.map_object_record(trap_guid).is_some());
        assert!(map.map_object_record(missing_trap_guid).is_none());

        map.remove_from_map_like_cpp(owner_guid, true).unwrap();
        assert!(map.map_object_record(owner_guid).is_none());
        assert!(map.map_object_record(trap_guid).is_none());
    }

    #[test]
    fn process_respawns_loaded_grid_loader_none_preserves_timer_and_stops_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(399, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 39901, active.clone()),
            |_| false,
        );
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 39902, active),
            |_| false,
        );
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 39901, 90));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 39902, 100));

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |_map, _object_type, _spawn_id| None,
        );

        assert_eq!(summary.executed_loaded_grid_respawns, 0);
        assert_eq!(summary.blocked_loaded_grid_respawn_loads, 1);
        assert_eq!(map.map_object_count(), 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 39901),
            90
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 39902),
            100
        );
    }

    #[test]
    fn process_respawns_unloaded_grid_allowed_branch_does_not_call_loader_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(400, SpawnGroupFlags::NONE);
        let mut far_spawn = spawn_data(SpawnObjectType::Creature, 40001, active);
        far_spawn.spawn_point = crate::spawn::SpawnPosition::new(1_000.0, 1_000.0, 0.0, 0.0);
        store.add_object_spawn(&far_spawn, |_| false);
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 40001, 100));
        let mut loader_calls = 0;

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |_map, _object_type, _spawn_id| {
                loader_calls += 1;
                None
            },
        );

        assert_eq!(loader_calls, 0);
        assert_eq!(summary.processed_unloaded_grid_respawns, 1);
        assert_eq!(summary.executed_loaded_grid_respawns, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 40001),
            0
        );
    }

    #[test]
    fn process_respawns_loaded_grid_add_to_map_failure_counts_and_removes_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(401, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 40101, active),
            |_| false,
        );
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 40101, 100));
        let expected_guid = guid(HighGuid::Creature, 4010101);

        let summary = map.process_due_respawns_composite_loaded_grid_respawns_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &PoolMgrLikeCpp::new(),
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
            |_map, _object_type, _spawn_id| {
                let mut creature = test_creature_for_spawn(40101, 4010101, true);
                creature
                    .unit_mut()
                    .world_mut()
                    .object_mut()
                    .remove_from_world();
                creature.unit_mut().world_mut().relocate(Position::xyz(
                    1_000_000.0,
                    1_000_000.0,
                    0.0,
                ));
                Some(LoadedGridRespawnRecordsLikeCpp::primary_only(
                    MapObjectRecord::new_creature(creature).unwrap(),
                ))
            },
        );

        assert_eq!(summary.executed_loaded_grid_respawns, 0);
        assert_eq!(summary.blocked_loaded_grid_respawn_add_to_map, 1);
        assert_eq!(summary.blocked_loaded_grid_respawn_loads, 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 40101),
            0
        );
        assert!(map.map_object_record(expected_guid).is_none());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(40101), 0);
    }

    #[test]
    fn process_respawns_pool_timer_updates_pool_plan_removes_timer_and_continues_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(14, SpawnGroupFlags::NONE);
        let inactive = spawn_group(15, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 45, active), |_| {
            false
        });
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 46, inactive), |_| {
            false
        });
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 45, 90));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 46, 100));
        let mut pool_mgr = PoolMgrLikeCpp::new();
        pool_mgr.insert_template_like_cpp(55, PoolTemplateDataLikeCpp::new(1, 571));
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 55);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(45, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(145, 0.0), 1);
        pool_mgr
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::GameObject, 55, group)
            .expect("test pool group");
        pool_mgr
            .register_spawn_pool_relation_like_cpp(PoolMemberKindLikeCpp::GameObject, 45, 55)
            .expect("test spawn pool relation");

        let summary = map.process_due_respawns_composite_safe_side_effects_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &pool_mgr,
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        );

        assert_eq!(summary.processed_pool_timers, 1);
        assert_eq!(summary.processed_unloaded_grid_respawns, 0);
        assert_eq!(summary.pool_update_plans.len(), 1);
        assert_eq!(summary.blocked_pool_plan_errors, Vec::new());
        assert_eq!(summary.blocked_pool_runtime, 0);
        assert_eq!(summary.deleted_inactive_spawn_group, 1);
        assert!(map.pool_data_like_cpp().is_spawned_gameobject_like_cpp(145));
        assert_eq!(map.pool_data_like_cpp().get_spawned_objects_like_cpp(55), 1);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 45),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 46),
            0
        );
    }

    #[test]
    fn process_respawns_pool_plan_despawn_one_removes_live_creature_and_gameobject_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(31, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 71, active.clone()),
            |_| false,
        );
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 72, active), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(71, 7101, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(72, 7201)).unwrap(),
        )
        .unwrap();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 71, 100));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 72, 100));

        assert_eq!(
            map.pool_data_mut_like_cpp()
                .add_spawn_like_cpp(SpawnObjectType::Creature, 71, 171),
            Ok(())
        );
        assert_eq!(
            map.pool_data_mut_like_cpp()
                .add_spawn_like_cpp(SpawnObjectType::GameObject, 72, 172),
            Ok(())
        );
        let mut pool_mgr = PoolMgrLikeCpp::new();
        pool_mgr.insert_template_like_cpp(171, PoolTemplateDataLikeCpp::new(0, 571));
        pool_mgr.insert_template_like_cpp(172, PoolTemplateDataLikeCpp::new(0, 571));
        let mut creature_group =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 171);
        creature_group.add_entry_like_cpp(PoolObjectLikeCpp::new(71, 0.0), 1);
        pool_mgr
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 171, creature_group)
            .expect("test creature pool group");
        pool_mgr
            .register_spawn_pool_relation_like_cpp(PoolMemberKindLikeCpp::Creature, 71, 171)
            .expect("test creature pool relation");
        let mut gameobject_group =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 172);
        gameobject_group.add_entry_like_cpp(PoolObjectLikeCpp::new(72, 0.0), 1);
        pool_mgr
            .insert_or_replace_group_like_cpp(
                PoolMemberKindLikeCpp::GameObject,
                172,
                gameobject_group,
            )
            .expect("test gameobject pool group");
        pool_mgr
            .register_spawn_pool_relation_like_cpp(PoolMemberKindLikeCpp::GameObject, 72, 172)
            .expect("test gameobject pool relation");

        let summary = map.process_due_respawns_composite_safe_side_effects_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &pool_mgr,
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        );

        assert_eq!(summary.processed_pool_timers, 2);
        assert_eq!(summary.pool_objects_removed, 2);
        assert_eq!(summary.pool_stale_index_entries, 0);
        assert_eq!(summary.pool_remove_errors, 0);
        assert_eq!(map.map_object_count(), 0);
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(71), 0);
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(72), 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 71),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 72),
            0
        );
    }

    #[test]
    fn process_respawns_pool_respawn_one_despawns_without_removing_unrelated_respawn_timer_like_cpp()
     {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(32, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 73, active.clone()),
            |_| false,
        );
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 74, active), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(73, 7301, true)).unwrap(),
        )
        .unwrap();
        map.ensure_grid_loaded(&cell_from_world(0.0, 0.0));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 74, 150));
        let plan = PoolTypedSpawnPlanLikeCpp {
            kind: PoolMemberKindLikeCpp::Creature,
            pool_id: 173,
            trigger_from: 73,
            max_limit: Some(1),
            object_plan: Some(PoolSpawnObjectPlanLikeCpp {
                actions: vec![PoolSpawnObjectActionLikeCpp::RespawnOne {
                    kind: PoolMemberKindLikeCpp::Creature,
                    guid: 73,
                }],
                selected: vec![],
                despawned_trigger: None,
                respawned_trigger: true,
            }),
            skip_reason: None,
        };
        let mut summary = ProcessRespawnsSafeSideEffectsSummaryLikeCpp::default();

        map.apply_pool_typed_spawn_plan_safe_map_actions_like_cpp(&plan, &store, &mut summary);

        assert_eq!(summary.pool_objects_removed, 1);
        assert_eq!(summary.pool_spawn_actions_skipped_unloaded_grid, 0);
        assert_eq!(summary.pool_spawn_actions_blocked_loaded_grid, 1);
        assert_eq!(
            summary.pool_spawn_action_load_plans,
            vec![PoolSpawnActionLoadPlanLikeCpp {
                object_type: SpawnObjectType::Creature,
                spawn_id: 73,
                respawn: true,
            }]
        );
        assert_eq!(summary.pool_respawn_timers_removed, 0);
        assert_eq!(map.map_object_count(), 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 74),
            150
        );
    }

    #[test]
    fn process_respawns_pool_remove_respawn_time_action_removes_member_timer_like_cpp() {
        let mut map = test_map();
        let store = SpawnStore::new();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 75, 200));
        let plan = PoolTypedSpawnPlanLikeCpp {
            kind: PoolMemberKindLikeCpp::GameObject,
            pool_id: 175,
            trigger_from: 0,
            max_limit: Some(1),
            object_plan: Some(PoolSpawnObjectPlanLikeCpp {
                actions: vec![
                    PoolSpawnObjectActionLikeCpp::RemoveRespawnTime {
                        kind: PoolMemberKindLikeCpp::GameObject,
                        guid: 75,
                    },
                    PoolSpawnObjectActionLikeCpp::RemoveRespawnTime {
                        kind: PoolMemberKindLikeCpp::GameObject,
                        guid: 76,
                    },
                ],
                selected: vec![],
                despawned_trigger: None,
                respawned_trigger: false,
            }),
            skip_reason: None,
        };
        let mut summary = ProcessRespawnsSafeSideEffectsSummaryLikeCpp::default();

        map.apply_pool_typed_spawn_plan_safe_map_actions_like_cpp(&plan, &store, &mut summary);

        assert_eq!(summary.pool_respawn_timers_removed, 1);
        assert_eq!(summary.pool_respawn_timers_missing, 1);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 75),
            0
        );
    }

    #[test]
    fn process_respawns_pool_spawn_action_reports_unloaded_loaded_and_missing_spawn_data_like_cpp()
    {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(33, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 76, active.clone()),
            |_| false,
        );
        let mut unloaded_spawn = spawn_data(SpawnObjectType::GameObject, 77, active);
        unloaded_spawn.spawn_point = crate::spawn::SpawnPosition::new(1_000.0, 1_000.0, 0.0, 0.0);
        store.add_object_spawn(&unloaded_spawn, |_| false);
        let loaded_cell = cell_from_world(0.0, 0.0);
        map.ensure_grid_loaded(&loaded_cell);
        let plan = PoolTypedSpawnPlanLikeCpp {
            kind: PoolMemberKindLikeCpp::Creature,
            pool_id: 176,
            trigger_from: 0,
            max_limit: Some(1),
            object_plan: Some(PoolSpawnObjectPlanLikeCpp {
                actions: vec![
                    PoolSpawnObjectActionLikeCpp::SpawnOne {
                        kind: PoolMemberKindLikeCpp::Creature,
                        guid: 76,
                    },
                    PoolSpawnObjectActionLikeCpp::SpawnOne {
                        kind: PoolMemberKindLikeCpp::GameObject,
                        guid: 77,
                    },
                    PoolSpawnObjectActionLikeCpp::SpawnOne {
                        kind: PoolMemberKindLikeCpp::Creature,
                        guid: 78,
                    },
                    PoolSpawnObjectActionLikeCpp::SpawnOne {
                        kind: PoolMemberKindLikeCpp::Pool,
                        guid: 179,
                    },
                ],
                selected: vec![],
                despawned_trigger: None,
                respawned_trigger: false,
            }),
            skip_reason: None,
        };
        let mut summary = ProcessRespawnsSafeSideEffectsSummaryLikeCpp::default();

        map.apply_pool_typed_spawn_plan_safe_map_actions_like_cpp(&plan, &store, &mut summary);

        assert_eq!(summary.pool_spawn_actions_blocked_loaded_grid, 1);
        assert_eq!(summary.pool_spawn_actions_skipped_unloaded_grid, 1);
        assert_eq!(summary.pool_spawn_actions_missing_spawn_data, 1);
        assert_eq!(summary.pool_unsupported_action_kind, 1);
        assert_eq!(
            summary.pool_spawn_action_load_plans,
            vec![PoolSpawnActionLoadPlanLikeCpp {
                object_type: SpawnObjectType::Creature,
                spawn_id: 76,
                respawn: false,
            }]
        );
    }

    #[test]
    fn process_respawns_pool_plan_error_preserves_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(14, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 47, active), |_| {
            false
        });
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 47, 100));
        let mut pool_mgr = PoolMgrLikeCpp::new();
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 55);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(47, 0.0), 1);
        pool_mgr
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 55, group)
            .expect("test pool group");
        pool_mgr
            .register_spawn_pool_relation_like_cpp(PoolMemberKindLikeCpp::Creature, 47, 55)
            .expect("test spawn pool relation");

        let summary = map.process_due_respawns_composite_safe_side_effects_like_cpp(
            100,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            &pool_mgr,
            5,
            false,
            |_, _| false,
            |_, _| 0.0,
            |_candidates, count| (0..count).collect(),
        );

        assert_eq!(summary.processed_pool_timers, 0);
        assert_eq!(
            summary.blocked_pool_plan_errors,
            vec![PoolMgrPlanErrorLikeCpp::MissingTemplate { pool_id: 55 }]
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 47),
            100
        );
        assert!(!map.pool_data_like_cpp().is_spawned_creature_like_cpp(47));
    }

    #[test]
    fn process_respawns_delete_only_preserves_cpp_order_when_first_due_blocks_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(15, SpawnGroupFlags::NONE);
        let manual = spawn_group(16, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 50, active), |_| {
            false
        });
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 40, manual), |_| {
            false
        });
        map.ensure_grid_loaded(&cell_from_grid_center(GridCoord::new(7, 0)));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 50, 90));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 40, 100));

        let summary = map.process_due_respawns_spawn_group_delete_only_like_cpp(100, &store);

        assert_eq!(summary.deleted_inactive_spawn_group, 0);
        assert_eq!(summary.blocked_do_respawn_runtime, 1);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 50),
            90
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 40),
            100
        );
    }

    #[test]
    fn check_respawn_spawn_group_guard_inactive_manual_group_clears_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(12, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 42, manual), |_| {
            false
        });
        let mut info = respawn_info(SpawnObjectType::Creature, 42, 100);

        let outcome = map.check_respawn_spawn_group_guard_like_cpp(&mut info, &store);

        assert_eq!(
            outcome,
            CheckRespawnSpawnGroupGuardOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer
        );
        assert_eq!(info.respawn_time, 0);
    }

    #[test]
    fn check_respawn_spawn_group_guard_active_manual_group_preserves_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(12, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 42, manual.clone()),
            |_| false,
        );
        map.set_spawn_group_active_like_cpp(Some(&manual), true);
        let mut info = respawn_info(SpawnObjectType::Creature, 42, 100);

        let outcome = map.check_respawn_spawn_group_guard_like_cpp(&mut info, &store);

        assert_eq!(outcome, CheckRespawnSpawnGroupGuardOutcomeLikeCpp::Allowed);
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn check_respawn_spawn_group_guard_system_group_preserves_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let system = spawn_group(1, SpawnGroupFlags::SYSTEM);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 43, system), |_| {
            false
        });
        let mut info = respawn_info(SpawnObjectType::GameObject, 43, 100);

        let outcome = map.check_respawn_spawn_group_guard_like_cpp(&mut info, &store);

        assert_eq!(outcome, CheckRespawnSpawnGroupGuardOutcomeLikeCpp::Allowed);
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn check_respawn_spawn_group_guard_missing_metadata_preserves_timer_like_cpp() {
        let map = test_map();
        let store = SpawnStore::new();
        let mut info = respawn_info(SpawnObjectType::Creature, 44, 100);

        let outcome = map.check_respawn_spawn_group_guard_like_cpp(&mut info, &store);

        assert_eq!(
            outcome,
            CheckRespawnSpawnGroupGuardOutcomeLikeCpp::MissingSpawnData
        );
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn check_respawn_live_object_guard_alive_creature_same_spawn_clears_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(21, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 51, group), |_| false);
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(51, 51, true)).unwrap(),
        )
        .unwrap();
        let mut info = respawn_info(SpawnObjectType::Creature, 51, 100);

        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn
        );
        assert_eq!(info.respawn_time, 0);
    }

    #[test]
    fn check_respawn_live_object_guard_dead_creature_same_spawn_allows_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(22, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 52, group), |_| false);
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(52, 52, false)).unwrap(),
        )
        .unwrap();
        let mut info = respawn_info(SpawnObjectType::Creature, 52, 100);

        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(outcome, CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed);
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn check_respawn_live_object_guard_dynamic_escort_closure_allows_only_when_config_enabled_like_cpp()
     {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(23, SpawnGroupFlags::ESCORTQUESTNPC);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 53, group.clone()),
            |_| false,
        );
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(53, 53, true)).unwrap(),
        )
        .unwrap();

        let mut info_config_enabled = respawn_info(SpawnObjectType::Creature, 53, 100);
        let enabled_outcome = map.check_respawn_live_object_guard_like_cpp(
            &mut info_config_enabled,
            &store,
            true,
            |_, _| true,
        );
        assert_eq!(
            enabled_outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed
        );
        assert_eq!(info_config_enabled.respawn_time, 100);

        let mut info_config_disabled = respawn_info(SpawnObjectType::Creature, 53, 100);
        let disabled_outcome = map.check_respawn_live_object_guard_like_cpp(
            &mut info_config_disabled,
            &store,
            false,
            |_, _| true,
        );
        assert_eq!(
            disabled_outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn
        );
        assert_eq!(info_config_disabled.respawn_time, 0);
    }

    #[test]
    fn check_respawn_live_object_guard_gameobject_same_spawn_clears_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(24, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 54, group), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(54, 54)).unwrap(),
        )
        .unwrap();
        let mut info = respawn_info(SpawnObjectType::GameObject, 54, 100);

        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::GameObjectBlocksRespawn
        );
        assert_eq!(info.respawn_time, 0);
    }

    #[test]
    fn check_respawn_live_object_guard_missing_spawn_data_preserves_timer_like_cpp() {
        let map = test_map();
        let store = SpawnStore::new();
        let mut info = respawn_info(SpawnObjectType::Creature, 55, 100);

        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::MissingSpawnData
        );
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn check_respawn_live_object_guard_area_trigger_unsupported_preserves_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(25, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::AreaTrigger, 56, group), |_| {
            false
        });
        let mut info = respawn_info(SpawnObjectType::AreaTrigger, 56, 100);

        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::UnsupportedSpawnType
        );
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn spawn_id_store_two_live_creatures_same_spawn_blocks_respawn_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(26, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 57, group), |_| false);

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(57, 5701, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(57, 5702, true)).unwrap(),
        )
        .unwrap();

        assert_eq!(map.creature_spawn_id_store_count_like_cpp(57), 2);
        let mut info = respawn_info(SpawnObjectType::Creature, 57, 100);
        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn
        );
        assert_eq!(info.respawn_time, 0);
    }

    #[test]
    fn spawn_id_store_removing_creatures_prunes_index_and_guard_allows_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(27, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 58, group), |_| false);
        let first_guid = guid(HighGuid::Creature, 5801);
        let second_guid = guid(HighGuid::Creature, 5802);

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(58, 5801, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(58, 5802, true)).unwrap(),
        )
        .unwrap();

        assert_eq!(map.creature_spawn_id_store_count_like_cpp(58), 2);
        assert!(map.remove_map_object(first_guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(58), 1);

        let mut blocked_info = respawn_info(SpawnObjectType::Creature, 58, 100);
        let blocked = map.check_respawn_live_object_guard_like_cpp(
            &mut blocked_info,
            &store,
            false,
            |_, _| false,
        );
        assert_eq!(
            blocked,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::AliveCreatureBlocksRespawn
        );
        assert_eq!(blocked_info.respawn_time, 0);

        assert!(map.remove_map_object(second_guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(58), 0);

        let mut allowed_info = respawn_info(SpawnObjectType::Creature, 58, 100);
        let allowed = map.check_respawn_live_object_guard_like_cpp(
            &mut allowed_info,
            &store,
            false,
            |_, _| false,
        );
        assert_eq!(allowed, CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed);
        assert_eq!(allowed_info.respawn_time, 100);
    }

    #[test]
    fn spawn_id_store_replacing_same_guid_moves_creature_spawn_id_like_cpp() {
        let mut map = test_map();
        let guid = guid(HighGuid::Creature, 5901);

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(59, 5901, true)).unwrap(),
        )
        .unwrap();
        let previous = map
            .insert_map_object_record(
                MapObjectRecord::new_creature(test_creature_for_spawn(60, 5901, true)).unwrap(),
            )
            .unwrap();

        assert!(previous.is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(59), 0);
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(60), 1);
        assert_eq!(map.creature_spawn_id_store_guids_like_cpp(60), vec![guid]);
    }

    #[test]
    fn world_object_by_spawn_id_typed_getters_return_indexed_objects_like_cpp() {
        let mut map = test_map();
        let creature = test_creature_for_spawn(67, 6701, true);
        let creature_guid = creature.unit().world().guid();
        let gameobject = test_gameobject_for_spawn(68, 6801);
        let gameobject_guid = gameobject.world().guid();
        let area_trigger = test_area_trigger_for_spawn(69, 6901);
        let area_trigger_guid = area_trigger.world().guid();

        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_area_trigger(area_trigger).unwrap())
            .unwrap();

        let creature = map.get_creature_by_spawn_id_like_cpp(67).unwrap();
        assert_eq!(creature.unit().world().guid(), creature_guid);
        assert_eq!(
            creature.unit().world().position(),
            Position::xyz(1.0, 2.0, 3.0)
        );
        let gameobject = map.get_gameobject_by_spawn_id_like_cpp(68).unwrap();
        assert_eq!(gameobject.world().guid(), gameobject_guid);
        assert_eq!(gameobject.world().position(), Position::xyz(1.0, 2.0, 3.0));
        let area_trigger = map.get_area_trigger_by_spawn_id_like_cpp(69).unwrap();
        assert_eq!(area_trigger.world().guid(), area_trigger_guid);
        assert_eq!(
            area_trigger.world().position(),
            Position::xyz(1.0, 2.0, 3.0)
        );

        assert_eq!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::Creature, 67)
                .unwrap()
                .guid(),
            creature_guid
        );
        assert_eq!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::GameObject, 68)
                .unwrap()
                .guid(),
            gameobject_guid
        );
        assert_eq!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::AreaTrigger, 69)
                .unwrap()
                .guid(),
            area_trigger_guid
        );
    }

    #[test]
    fn world_object_by_spawn_id_absent_and_zero_spawn_return_none_like_cpp() {
        let mut map = test_map();

        assert!(map.get_creature_by_spawn_id_like_cpp(75).is_none());
        assert!(map.get_gameobject_by_spawn_id_like_cpp(75).is_none());
        assert!(map.get_area_trigger_by_spawn_id_like_cpp(75).is_none());
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::Creature, 75)
                .is_none()
        );
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::GameObject, 75)
                .is_none()
        );
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::AreaTrigger, 75)
                .is_none()
        );

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(0, 6001, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(0, 6002)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(0, 6003)).unwrap(),
        )
        .unwrap();

        assert_eq!(map.creature_spawn_id_store_count_like_cpp(0), 0);
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(0), 0);
        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(0), 0);
        assert_eq!(map.map_object_count(), 3);
        assert!(map.get_creature_by_spawn_id_like_cpp(0).is_none());
        assert!(map.get_gameobject_by_spawn_id_like_cpp(0).is_none());
        assert!(map.get_area_trigger_by_spawn_id_like_cpp(0).is_none());
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::Creature, 0)
                .is_none()
        );
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::GameObject, 0)
                .is_none()
        );
        assert!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::AreaTrigger, 0)
                .is_none()
        );
    }

    #[test]
    fn world_object_by_spawn_id_creature_prefers_alive_then_fallback_like_cpp() {
        let mut map = test_map();
        let dead_guid = guid(HighGuid::Creature, 7601);
        let alive_guid = guid(HighGuid::Creature, 7602);

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(76, 7601, false)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(76, 7602, true)).unwrap(),
        )
        .unwrap();

        assert_eq!(
            map.creature_spawn_id_store_guids_like_cpp(76),
            vec![dead_guid, alive_guid]
        );
        assert_eq!(
            map.get_creature_by_spawn_id_like_cpp(76)
                .unwrap()
                .unit()
                .world()
                .guid(),
            alive_guid
        );
        assert_eq!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::Creature, 76)
                .unwrap()
                .guid(),
            alive_guid
        );

        assert!(map.remove_map_object(alive_guid).is_some());
        assert_eq!(
            map.get_creature_by_spawn_id_like_cpp(76)
                .unwrap()
                .unit()
                .world()
                .guid(),
            dead_guid
        );
    }

    #[test]
    fn world_object_by_spawn_id_gameobject_prefers_spawned_then_fallback_like_cpp() {
        let mut map = test_map();
        let despawned_guid = guid(HighGuid::GameObject, 7801);
        let spawned_guid = guid(HighGuid::GameObject, 7802);
        let mut despawned = test_gameobject_for_spawn(78, 7801);
        despawned.set_respawn_delay_time(30);
        despawned.set_respawn_time(100);
        despawned.set_spawned_by_default(true);
        let mut spawned = test_gameobject_for_spawn(78, 7802);
        spawned.set_respawn_delay_time(30);
        spawned.set_respawn_time(100);
        spawned.set_spawned_by_default(false);

        map.insert_map_object_record(MapObjectRecord::new_game_object(despawned).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_game_object(spawned).unwrap())
            .unwrap();

        assert_eq!(
            map.gameobject_spawn_id_store_guids_like_cpp(78),
            vec![despawned_guid, spawned_guid]
        );
        assert_eq!(
            map.get_gameobject_by_spawn_id_like_cpp(78)
                .unwrap()
                .world()
                .guid(),
            spawned_guid
        );
        assert_eq!(
            map.get_world_object_by_spawn_id_like_cpp(SpawnObjectType::GameObject, 78)
                .unwrap()
                .guid(),
            spawned_guid
        );

        assert!(map.remove_map_object(spawned_guid).is_some());
        assert_eq!(
            map.get_gameobject_by_spawn_id_like_cpp(78)
                .unwrap()
                .world()
                .guid(),
            despawned_guid
        );
    }

    #[test]
    fn area_trigger_spawn_id_store_indexes_and_gets_typed_object_like_cpp() {
        let mut map = test_map();
        let area_trigger = test_area_trigger_for_spawn(70, 7001);
        let guid = area_trigger.world().guid();

        map.insert_map_object_record(MapObjectRecord::new_area_trigger(area_trigger).unwrap())
            .unwrap();

        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(70), 1);
        assert_eq!(
            map.area_trigger_spawn_id_store_guids_like_cpp(70),
            vec![guid]
        );
        let stored = map.get_area_trigger_by_spawn_id_like_cpp(70).unwrap();
        assert_eq!(stored.world().guid(), guid);
        assert_eq!(stored.spawn_id(), 70);
    }

    #[test]
    fn area_trigger_spawn_id_store_remove_desindexes_like_cpp() {
        let mut map = test_map();
        let guid = guid(HighGuid::AreaTrigger, 7101);

        map.insert_map_object_record(
            MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(71, 7101)).unwrap(),
        )
        .unwrap();
        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(71), 1);

        assert!(map.remove_map_object(guid).is_some());
        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(71), 0);
        assert!(map.get_area_trigger_by_spawn_id_like_cpp(71).is_none());
    }

    #[test]
    fn area_trigger_spawn_id_store_replacing_same_guid_moves_spawn_id_like_cpp() {
        let mut map = test_map();
        let guid = guid(HighGuid::AreaTrigger, 7201);

        map.insert_map_object_record(
            MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(72, 7201)).unwrap(),
        )
        .unwrap();
        let previous = map
            .insert_map_object_record(
                MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(73, 7201)).unwrap(),
            )
            .unwrap();

        assert!(previous.is_some());
        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(72), 0);
        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(73), 1);
        assert_eq!(
            map.area_trigger_spawn_id_store_guids_like_cpp(73),
            vec![guid]
        );
    }

    #[test]
    fn area_trigger_spawn_id_store_multiple_same_spawn_keeps_multimap_cardinality_like_cpp() {
        let mut map = test_map();
        let first_guid = guid(HighGuid::AreaTrigger, 7401);
        let second_guid = guid(HighGuid::AreaTrigger, 7402);

        map.insert_map_object_record(
            MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(74, 7402)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_area_trigger(test_area_trigger_for_spawn(74, 7401)).unwrap(),
        )
        .unwrap();

        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(74), 2);
        assert_eq!(
            map.area_trigger_spawn_id_store_guids_like_cpp(74),
            vec![first_guid, second_guid]
        );
        assert_eq!(
            map.get_area_trigger_by_spawn_id_like_cpp(74)
                .unwrap()
                .world()
                .guid(),
            first_guid
        );
    }

    #[test]
    fn area_trigger_spawn_id_store_absent_query_returns_none_like_cpp() {
        let map = test_map();

        assert_eq!(map.area_trigger_spawn_id_store_count_like_cpp(75), 0);
        assert!(
            map.area_trigger_spawn_id_store_guids_like_cpp(75)
                .is_empty()
        );
        assert!(map.get_area_trigger_by_spawn_id_like_cpp(75).is_none());
    }

    #[test]
    fn spawn_id_store_gameobject_same_spawn_blocks_until_removed_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(28, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 61, group), |_| {
            false
        });
        let gameobject_guid = guid(HighGuid::GameObject, 6101);

        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(61, 6101)).unwrap(),
        )
        .unwrap();
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(61), 1);

        let mut blocked_info = respawn_info(SpawnObjectType::GameObject, 61, 100);
        let blocked = map.check_respawn_live_object_guard_like_cpp(
            &mut blocked_info,
            &store,
            false,
            |_, _| false,
        );
        assert_eq!(
            blocked,
            CheckRespawnLiveObjectGuardOutcomeLikeCpp::GameObjectBlocksRespawn
        );
        assert_eq!(blocked_info.respawn_time, 0);

        assert!(map.remove_map_object(gameobject_guid).is_some());
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(61), 0);

        let mut allowed_info = respawn_info(SpawnObjectType::GameObject, 61, 100);
        let allowed = map.check_respawn_live_object_guard_like_cpp(
            &mut allowed_info,
            &store,
            false,
            |_, _| false,
        );
        assert_eq!(allowed, CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed);
        assert_eq!(allowed_info.respawn_time, 100);
    }

    #[test]
    fn spawn_id_store_dead_creature_indexed_but_does_not_block_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(29, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 62, group), |_| false);

        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(62, 6201, false)).unwrap(),
        )
        .unwrap();

        assert_eq!(map.creature_spawn_id_store_count_like_cpp(62), 1);
        let mut info = respawn_info(SpawnObjectType::Creature, 62, 100);
        let outcome =
            map.check_respawn_live_object_guard_like_cpp(&mut info, &store, false, |_, _| false);

        assert_eq!(outcome, CheckRespawnLiveObjectGuardOutcomeLikeCpp::Allowed);
        assert_eq!(info.respawn_time, 100);
    }

    #[test]
    fn map_spawn_group_initial_state_system_active_and_not_toggleable() {
        let mut map = test_map();
        let system = spawn_group(1, SpawnGroupFlags::SYSTEM);

        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
        assert!(map.is_spawn_group_active_like_cpp(Some(&system)));
        assert_eq!(
            map.set_spawn_group_active_like_cpp(Some(&system), false),
            SpawnGroupActiveChange::SystemGroup
        );
        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
        assert!(map.is_spawn_group_active_like_cpp(Some(&system)));
    }

    #[test]
    fn map_spawn_group_manual_default_inactive_activate_toggles_deactivate_clears() {
        let mut map = test_map();
        let manual = spawn_group(10, SpawnGroupFlags::MANUAL_SPAWN);

        assert!(!map.is_spawn_group_active_like_cpp(Some(&manual)));
        assert_eq!(
            map.set_spawn_group_active_like_cpp(Some(&manual), true),
            SpawnGroupActiveChange::Toggled
        );
        assert!(map.spawn_group_state().is_toggled(manual.group_id));
        assert!(map.is_spawn_group_active_like_cpp(Some(&manual)));

        assert_eq!(
            map.set_spawn_group_inactive_like_cpp(Some(&manual)),
            SpawnGroupActiveChange::ClearedToggle
        );
        assert!(!map.spawn_group_state().is_toggled(manual.group_id));
        assert!(!map.is_spawn_group_active_like_cpp(Some(&manual)));
    }

    #[test]
    fn map_spawn_group_non_manual_default_active_deactivate_toggles_activate_clears() {
        let mut map = test_map();
        let automatic = spawn_group(11, SpawnGroupFlags::NONE);

        assert!(map.is_spawn_group_active_like_cpp(Some(&automatic)));
        assert_eq!(
            map.set_spawn_group_inactive_like_cpp(Some(&automatic)),
            SpawnGroupActiveChange::Toggled
        );
        assert!(map.spawn_group_state().is_toggled(automatic.group_id));
        assert!(!map.is_spawn_group_active_like_cpp(Some(&automatic)));

        assert_eq!(
            map.set_spawn_group_active_like_cpp(Some(&automatic), true),
            SpawnGroupActiveChange::ClearedToggle
        );
        assert!(!map.spawn_group_state().is_toggled(automatic.group_id));
        assert!(map.is_spawn_group_active_like_cpp(Some(&automatic)));
    }

    #[test]
    fn map_spawn_group_missing_group_returns_false_and_does_not_mutate_toggles() {
        let mut map = test_map();

        assert!(!map.is_spawn_group_active_like_cpp(None));
        assert_eq!(
            map.set_spawn_group_active_like_cpp(None, true),
            SpawnGroupActiveChange::MissingGroup
        );
        assert_eq!(
            map.set_spawn_group_inactive_like_cpp(None),
            SpawnGroupActiveChange::MissingGroup
        );
        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
    }

    #[test]
    fn map_spawn_group_grid_load_bridge_uses_map_owned_toggle_state() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(12, SpawnGroupFlags::MANUAL_SPAWN);
        let spawn = crate::spawn::SpawnData {
            object_type: SpawnObjectType::Creature,
            spawn_id: 42,
            map_id: 571,
            db_data: true,
            spawn_group: manual.clone(),
            id: 99,
            spawn_point: crate::spawn::SpawnPosition::new(0.0, 0.0, 0.0, 0.0),
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: 0,
            pool_id: 0,
            spawn_time_secs: 0,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        };
        store.add_object_spawn(&spawn, |_| false);

        assert!(
            !map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );

        map.set_spawn_group_active_like_cpp(Some(&manual), true);
        assert!(
            map.spawn_grid_load_state_like_cpp(&store)
                .should_be_spawned_on_grid_load(SpawnObjectType::Creature, 42)
        );
    }

    #[test]
    fn map_spawn_group_init_bridge_skips_system_and_applies_condition_semantics() {
        let mut map = test_map();
        let system = spawn_group(1, SpawnGroupFlags::SYSTEM);
        let manual = spawn_group(20, SpawnGroupFlags::MANUAL_SPAWN);
        let automatic = spawn_group(21, SpawnGroupFlags::NONE);
        let groups = [&system, &manual, &automatic];

        let changes = map.init_spawn_group_state_like_cpp(groups, |group| group.group_id == 20);

        assert_eq!(
            changes,
            vec![
                (20, SpawnGroupActiveChange::Toggled),
                (21, SpawnGroupActiveChange::Toggled)
            ]
        );
        assert!(!map.spawn_group_state().is_toggled(system.group_id));
        assert!(map.spawn_group_state().is_toggled(manual.group_id));
        assert!(map.spawn_group_state().is_toggled(automatic.group_id));
        assert!(map.is_spawn_group_active_like_cpp(Some(&manual)));
        assert!(!map.is_spawn_group_active_like_cpp(Some(&automatic)));
    }

    #[test]
    fn update_spawn_group_conditions_manual_active_condition_false_with_despawn_flag_plans_despawn()
    {
        let mut map = test_map();
        let manual = spawn_group(
            30,
            spawn_group_flags(
                SpawnGroupFlags::MANUAL_SPAWN,
                SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE,
            ),
        );
        map.set_spawn_group_active_like_cpp(Some(&manual), true);

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&manual], |_| false);

        assert_eq!(
            actions,
            vec![(
                30,
                SpawnGroupConditionActionLikeCpp::Despawn {
                    delete_respawn_times: true
                }
            )]
        );
    }

    #[test]
    fn update_spawn_group_conditions_manual_active_condition_true_is_noop() {
        let mut map = test_map();
        let manual = spawn_group(
            31,
            spawn_group_flags(
                SpawnGroupFlags::MANUAL_SPAWN,
                SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE,
            ),
        );
        map.set_spawn_group_active_like_cpp(Some(&manual), true);

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&manual], |_| true);

        assert_eq!(actions, vec![(31, SpawnGroupConditionActionLikeCpp::Noop)]);
    }

    #[test]
    fn update_spawn_group_conditions_automatic_inactive_condition_true_plans_spawn() {
        let mut map = test_map();
        let automatic = spawn_group(32, SpawnGroupFlags::NONE);
        map.set_spawn_group_inactive_like_cpp(Some(&automatic));

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&automatic], |_| true);

        assert_eq!(
            actions,
            vec![(
                32,
                SpawnGroupConditionActionLikeCpp::Spawn {
                    ignore_respawn: false,
                    force: false
                }
            )]
        );
    }

    #[test]
    fn update_spawn_group_conditions_automatic_active_condition_false_with_despawn_flag_plans_despawn()
     {
        let map = test_map();
        let automatic = spawn_group(33, SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE);

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&automatic], |_| false);

        assert_eq!(
            actions,
            vec![(
                33,
                SpawnGroupConditionActionLikeCpp::Despawn {
                    delete_respawn_times: true
                }
            )]
        );
    }

    #[test]
    fn update_spawn_group_conditions_automatic_active_condition_false_without_despawn_flag_sets_inactive()
     {
        let map = test_map();
        let automatic = spawn_group(34, SpawnGroupFlags::NONE);

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&automatic], |_| false);

        assert_eq!(
            actions,
            vec![(34, SpawnGroupConditionActionLikeCpp::SetInactive)]
        );
    }

    #[test]
    fn update_spawn_group_conditions_automatic_active_condition_true_is_noop() {
        let map = test_map();
        let automatic = spawn_group(35, SpawnGroupFlags::NONE);

        let actions = map.plan_update_spawn_group_conditions_like_cpp([&automatic], |_| true);

        assert_eq!(actions, vec![(35, SpawnGroupConditionActionLikeCpp::Noop)]);
    }

    #[test]
    fn update_spawn_group_conditions_planner_is_pure_and_preserves_spawn_group_state() {
        let mut map = test_map();
        let manual = spawn_group(
            36,
            spawn_group_flags(
                SpawnGroupFlags::MANUAL_SPAWN,
                SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE,
            ),
        );
        let automatic = spawn_group(37, SpawnGroupFlags::NONE);
        map.set_spawn_group_active_like_cpp(Some(&manual), true);
        let before = map
            .spawn_group_state()
            .toggled_spawn_group_ids()
            .iter()
            .copied()
            .collect::<Vec<_>>();

        let actions =
            map.plan_update_spawn_group_conditions_like_cpp([&manual, &automatic], |_| false);
        let after = map
            .spawn_group_state()
            .toggled_spawn_group_ids()
            .iter()
            .copied()
            .collect::<Vec<_>>();

        assert_eq!(
            actions,
            vec![
                (
                    36,
                    SpawnGroupConditionActionLikeCpp::Despawn {
                        delete_respawn_times: true
                    }
                ),
                (37, SpawnGroupConditionActionLikeCpp::SetInactive),
            ]
        );
        assert_eq!(after, before);
        assert!(map.is_spawn_group_active_like_cpp(Some(&manual)));
        assert!(map.is_spawn_group_active_like_cpp(Some(&automatic)));
    }

    #[test]
    fn update_spawn_group_conditions_apply_automatic_condition_failure_without_despawn_sets_inactive()
     {
        let mut map = test_map();
        let automatic = spawn_group(38, SpawnGroupFlags::NONE);

        let outcomes =
            map.apply_update_spawn_group_conditions_set_inactive_like_cpp([&automatic], |_| false);

        assert_eq!(
            outcomes,
            vec![SpawnGroupConditionUpdateOutcomeLikeCpp {
                group_id: 38,
                action: SpawnGroupConditionActionLikeCpp::SetInactive,
                applied_change: Some(SpawnGroupActiveChange::Toggled),
                despawn_outcome: None,
                spawn_outcome: None,
            }]
        );
        assert!(!map.is_spawn_group_active_like_cpp(Some(&automatic)));
        assert!(map.spawn_group_state().is_toggled(automatic.group_id));
    }

    #[test]
    fn update_spawn_group_conditions_apply_automatic_condition_failure_with_despawn_only_plans_despawn()
     {
        let mut map = test_map();
        let automatic = spawn_group(39, SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE);

        let outcomes =
            map.apply_update_spawn_group_conditions_set_inactive_like_cpp([&automatic], |_| false);

        assert_eq!(
            outcomes,
            vec![SpawnGroupConditionUpdateOutcomeLikeCpp {
                group_id: 39,
                action: SpawnGroupConditionActionLikeCpp::Despawn {
                    delete_respawn_times: true
                },
                applied_change: None,
                despawn_outcome: None,
                spawn_outcome: None,
            }]
        );
        assert!(map.is_spawn_group_active_like_cpp(Some(&automatic)));
        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
    }

    #[test]
    fn update_spawn_group_conditions_apply_automatic_inactive_condition_true_only_plans_spawn() {
        let mut map = test_map();
        let automatic = spawn_group(40, SpawnGroupFlags::NONE);
        assert_eq!(
            map.set_spawn_group_inactive_like_cpp(Some(&automatic)),
            SpawnGroupActiveChange::Toggled
        );

        let outcomes =
            map.apply_update_spawn_group_conditions_set_inactive_like_cpp([&automatic], |_| true);

        assert_eq!(
            outcomes,
            vec![SpawnGroupConditionUpdateOutcomeLikeCpp {
                group_id: 40,
                action: SpawnGroupConditionActionLikeCpp::Spawn {
                    ignore_respawn: false,
                    force: false,
                },
                applied_change: None,
                despawn_outcome: None,
                spawn_outcome: None,
            }]
        );
        assert!(!map.is_spawn_group_active_like_cpp(Some(&automatic)));
        assert!(map.spawn_group_state().is_toggled(automatic.group_id));
    }

    #[test]
    fn update_spawn_group_conditions_condition_failure_despawns_live_objects_and_timers_like_cpp() {
        let group = spawn_group(391, SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE);
        let mut store = SpawnStore::new();
        let mut templates = BTreeMap::from([(group.group_id, group.clone())]);
        let creature_spawn = spawn_data(
            SpawnObjectType::Creature,
            10,
            SpawnGroupTemplateData::default_group(),
        );
        let gameobject_spawn = spawn_data(
            SpawnObjectType::GameObject,
            20,
            SpawnGroupTemplateData::default_group(),
        );
        store.add_object_spawn(&creature_spawn, |_| false);
        store.add_object_spawn(&gameobject_spawn, |_| false);
        store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                crate::spawn::SpawnGroupMemberRow {
                    group_id: group.group_id,
                    spawn_type: SpawnObjectType::Creature as u8,
                    spawn_id: 10,
                },
                crate::spawn::SpawnGroupMemberRow {
                    group_id: group.group_id,
                    spawn_type: SpawnObjectType::GameObject as u8,
                    spawn_id: 20,
                },
            ],
        );
        let group = templates.get(&391).expect("group resolved").clone();
        let mut map = test_map();
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(10, 10, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(20, 20)).unwrap(),
        )
        .unwrap();
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(10), 1);
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(20), 1);
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 100));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 20, 100));

        let outcomes =
            map.apply_update_spawn_group_conditions_represented_like_cpp([&group], &store, |_| {
                false
            });

        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].action,
            SpawnGroupConditionActionLikeCpp::condition_failure_despawn()
        );
        let despawn = outcomes[0].despawn_outcome.expect("despawn executed");
        assert_eq!(despawn.objects_removed, 2);
        assert_eq!(despawn.respawn_timers_removed, 2);
        assert_eq!(despawn.blocked_missing_group, 0);
        assert_eq!(despawn.blocked_system_group, 0);
        assert_eq!(despawn.unsupported_live_despawn_types, 0);
        assert_eq!(
            despawn.applied_inactive_change,
            Some(SpawnGroupActiveChange::Toggled)
        );
        assert_eq!(map.map_object_count(), 0);
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(10), 0);
        assert_eq!(map.gameobject_spawn_id_store_count_like_cpp(20), 0);
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            0
        );
        assert_eq!(
            map.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            0
        );
        assert!(!map.is_spawn_group_active_like_cpp(Some(&group)));
    }

    #[test]
    fn spawn_group_spawn_missing_or_system_group_blocks_without_activation_like_cpp() {
        let mut map = test_map();
        let store = SpawnStore::new();
        let system = spawn_group(3940, SpawnGroupFlags::SYSTEM);

        let missing = map.spawn_group_spawn_like_cpp(None, false, false, &store);
        let system_outcome = map.spawn_group_spawn_like_cpp(Some(&system), false, false, &store);

        assert_eq!(missing.blocked_missing_group, 1);
        assert_eq!(missing.applied_active_change, None);
        assert_eq!(system_outcome.blocked_system_group, 1);
        assert_eq!(system_outcome.applied_active_change, None);
        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
    }

    #[test]
    fn spawn_group_spawn_loaded_grid_creature_and_gameobject_are_planned_but_not_created_like_cpp()
    {
        let group = spawn_group(3941, SpawnGroupFlags::NONE);
        let (group, store) = spawn_group_store(
            group,
            vec![
                spawn_data(
                    SpawnObjectType::Creature,
                    101,
                    SpawnGroupTemplateData::default_group(),
                ),
                spawn_data(
                    SpawnObjectType::GameObject,
                    201,
                    SpawnGroupTemplateData::default_group(),
                ),
            ],
        );
        let mut map = test_map();
        map.set_spawn_group_inactive_like_cpp(Some(&group));
        map.load_grid(0.0, 0.0);

        let outcome = map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(outcome.metadata_entries, 2);
        assert_eq!(
            outcome.applied_active_change,
            Some(SpawnGroupActiveChange::ClearedToggle)
        );
        assert_eq!(outcome.blocked_loaded_grid_creature_loads, 1);
        assert_eq!(outcome.blocked_loaded_grid_gameobject_loads, 1);
        assert_eq!(
            outcome.load_plans,
            vec![
                SpawnGroupSpawnLoadPlanLikeCpp {
                    object_type: SpawnObjectType::Creature,
                    spawn_id: 101,
                    force: false,
                },
                SpawnGroupSpawnLoadPlanLikeCpp {
                    object_type: SpawnObjectType::GameObject,
                    spawn_id: 201,
                    force: false,
                },
            ]
        );
        assert_eq!(map.map_object_count(), 0);
        assert!(map.is_spawn_group_active_like_cpp(Some(&group)));
    }

    #[test]
    fn spawn_group_spawn_respawn_timer_skips_unless_ignore_or_force_removes_like_cpp() {
        let group = spawn_group(3942, SpawnGroupFlags::NONE);
        let (group, store) = spawn_group_store(
            group,
            vec![spawn_data(
                SpawnObjectType::Creature,
                102,
                SpawnGroupTemplateData::default_group(),
            )],
        );
        let mut blocked_map = test_map();
        blocked_map.load_grid(0.0, 0.0);
        blocked_map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 102, 100));

        let blocked = blocked_map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(blocked.skipped_respawn_timer_active, 1);
        assert_eq!(blocked.load_plans.len(), 0);
        assert_eq!(
            blocked_map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 102),
            100
        );

        let mut ignore_map = test_map();
        ignore_map.load_grid(0.0, 0.0);
        ignore_map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 102, 100));
        let ignored = ignore_map.spawn_group_spawn_like_cpp(Some(&group), true, false, &store);

        assert_eq!(ignored.respawn_timers_removed, 1);
        assert_eq!(ignored.skipped_respawn_timer_active, 0);
        assert_eq!(ignored.blocked_loaded_grid_creature_loads, 1);
        assert_eq!(
            ignore_map.get_respawn_time_like_cpp(SpawnObjectType::Creature, 102),
            0
        );

        let mut force_map = test_map();
        force_map.load_grid(0.0, 0.0);
        force_map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 102, 100));
        let forced = force_map.spawn_group_spawn_like_cpp(Some(&group), false, true, &store);
        assert_eq!(forced.respawn_timers_removed, 1);
        assert_eq!(forced.load_plans[0].force, true);
    }

    #[test]
    fn spawn_group_spawn_live_object_skip_is_bypassed_by_force_like_cpp() {
        let group = spawn_group(3943, SpawnGroupFlags::NONE);
        let (group, store) = spawn_group_store(
            group,
            vec![
                spawn_data(
                    SpawnObjectType::Creature,
                    103,
                    SpawnGroupTemplateData::default_group(),
                ),
                spawn_data(
                    SpawnObjectType::GameObject,
                    203,
                    SpawnGroupTemplateData::default_group(),
                ),
            ],
        );
        let mut map = test_map();
        map.load_grid(0.0, 0.0);
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(103, 103, true)).unwrap(),
        )
        .unwrap();
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(203, 203)).unwrap(),
        )
        .unwrap();

        let skipped = map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(skipped.skipped_live_object_active, 2);
        assert!(skipped.load_plans.is_empty());
        assert_eq!(map.map_object_count(), 2);

        let forced = map.spawn_group_spawn_like_cpp(Some(&group), false, true, &store);
        assert_eq!(forced.skipped_live_object_active, 0);
        assert_eq!(forced.load_plans.len(), 2);
        assert_eq!(forced.respawn_timers_missing, 2);
        assert_eq!(map.map_object_count(), 2);
    }

    #[test]
    fn spawn_group_spawn_difficulty_mismatch_precedes_unloaded_grid_like_cpp() {
        let group = spawn_group(3944, SpawnGroupFlags::NONE);
        let mut spawn = spawn_data(
            SpawnObjectType::Creature,
            104,
            SpawnGroupTemplateData::default_group(),
        );
        spawn.spawn_difficulties = vec![2];
        let (group, store) = spawn_group_store(group, vec![spawn]);
        let mut map = test_map();

        let outcome = map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(outcome.skipped_difficulty_mismatch, 1);
        assert_eq!(outcome.skipped_unloaded_grid, 0);
        assert!(outcome.load_plans.is_empty());
    }

    #[test]
    fn spawn_group_spawn_unloaded_grid_skips_before_plan_like_cpp() {
        let group = spawn_group(3945, SpawnGroupFlags::NONE);
        let (group, store) = spawn_group_store(
            group,
            vec![spawn_data(
                SpawnObjectType::GameObject,
                205,
                SpawnGroupTemplateData::default_group(),
            )],
        );
        let mut map = test_map();

        let outcome = map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(outcome.skipped_unloaded_grid, 1);
        assert_eq!(outcome.blocked_loaded_grid_gameobject_loads, 0);
        assert!(outcome.load_plans.is_empty());
    }

    #[test]
    fn spawn_group_spawn_area_trigger_is_unsupported_not_complete_like_cpp() {
        let group = spawn_group(3946, SpawnGroupFlags::NONE);
        let (group, store) = spawn_group_store(
            group,
            vec![spawn_data(
                SpawnObjectType::AreaTrigger,
                305,
                SpawnGroupTemplateData::default_group(),
            )],
        );
        let mut map = test_map();
        map.load_grid(0.0, 0.0);

        let outcome = map.spawn_group_spawn_like_cpp(Some(&group), false, false, &store);

        assert_eq!(outcome.metadata_entries, 1);
        assert_eq!(outcome.unsupported_spawn_types, 1);
        assert!(outcome.load_plans.is_empty());
    }

    #[test]
    fn update_spawn_group_conditions_spawn_branch_returns_spawn_outcome_and_activates_like_cpp() {
        let group = spawn_group(392, SpawnGroupFlags::NONE);
        let mut store = SpawnStore::new();
        let mut templates = BTreeMap::from([(group.group_id, group.clone())]);
        let creature_spawn = spawn_data(
            SpawnObjectType::Creature,
            30,
            SpawnGroupTemplateData::default_group(),
        );
        store.add_object_spawn(&creature_spawn, |_| false);
        store.apply_spawn_groups_like_cpp(
            &mut templates,
            [crate::spawn::SpawnGroupMemberRow {
                group_id: group.group_id,
                spawn_type: SpawnObjectType::Creature as u8,
                spawn_id: 30,
            }],
        );
        let group = templates.get(&392).expect("group resolved").clone();
        let mut map = test_map();
        map.set_spawn_group_inactive_like_cpp(Some(&group));
        map.load_grid(0.0, 0.0);

        let outcomes =
            map.apply_update_spawn_group_conditions_represented_like_cpp([&group], &store, |_| {
                true
            });

        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].action,
            SpawnGroupConditionActionLikeCpp::spawn_group_spawn_default()
        );
        assert_eq!(outcomes[0].applied_change, None);
        assert_eq!(outcomes[0].despawn_outcome, None);
        let spawn = outcomes[0].spawn_outcome.as_ref().expect("spawn executed");
        assert_eq!(
            spawn.applied_active_change,
            Some(SpawnGroupActiveChange::ClearedToggle)
        );
        assert_eq!(spawn.blocked_loaded_grid_creature_loads, 1);
        assert_eq!(
            spawn.load_plans,
            vec![SpawnGroupSpawnLoadPlanLikeCpp {
                object_type: SpawnObjectType::Creature,
                spawn_id: 30,
                force: false,
            }]
        );
        assert_eq!(map.map_object_count(), 0);
        assert!(map.is_spawn_group_active_like_cpp(Some(&group)));
    }

    #[test]
    fn update_spawn_group_conditions_apply_manual_condition_failure_never_sets_inactive() {
        let mut map = test_map();
        let manual_with_despawn = spawn_group(
            41,
            spawn_group_flags(
                SpawnGroupFlags::MANUAL_SPAWN,
                SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE,
            ),
        );
        let manual_without_despawn = spawn_group(42, SpawnGroupFlags::MANUAL_SPAWN);
        map.set_spawn_group_active_like_cpp(Some(&manual_with_despawn), true);
        map.set_spawn_group_active_like_cpp(Some(&manual_without_despawn), true);

        let outcomes = map.apply_update_spawn_group_conditions_set_inactive_like_cpp(
            [&manual_with_despawn, &manual_without_despawn],
            |_| false,
        );

        assert_eq!(
            outcomes,
            vec![
                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id: 41,
                    action: SpawnGroupConditionActionLikeCpp::Despawn {
                        delete_respawn_times: true
                    },
                    applied_change: None,
                    despawn_outcome: None,
                    spawn_outcome: None,
                },
                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id: 42,
                    action: SpawnGroupConditionActionLikeCpp::Noop,
                    applied_change: None,
                    despawn_outcome: None,
                    spawn_outcome: None,
                },
            ]
        );
        assert!(map.is_spawn_group_active_like_cpp(Some(&manual_with_despawn)));
        assert!(map.is_spawn_group_active_like_cpp(Some(&manual_without_despawn)));
        assert!(
            map.spawn_group_state()
                .is_toggled(manual_with_despawn.group_id)
        );
        assert!(
            map.spawn_group_state()
                .is_toggled(manual_without_despawn.group_id)
        );
    }

    #[test]
    fn update_spawn_group_conditions_apply_active_equals_should_is_noop_without_change() {
        let mut map = test_map();
        let automatic = spawn_group(43, SpawnGroupFlags::NONE);
        let manual = spawn_group(44, SpawnGroupFlags::MANUAL_SPAWN);

        let outcomes = map.apply_update_spawn_group_conditions_set_inactive_like_cpp(
            [&automatic, &manual],
            |group| group.group_id == automatic.group_id,
        );

        assert_eq!(
            outcomes,
            vec![
                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id: 43,
                    action: SpawnGroupConditionActionLikeCpp::Noop,
                    applied_change: None,
                    despawn_outcome: None,
                    spawn_outcome: None,
                },
                SpawnGroupConditionUpdateOutcomeLikeCpp {
                    group_id: 44,
                    action: SpawnGroupConditionActionLikeCpp::Noop,
                    applied_change: None,
                    despawn_outcome: None,
                    spawn_outcome: None,
                },
            ]
        );
        assert!(map.is_spawn_group_active_like_cpp(Some(&automatic)));
        assert!(!map.is_spawn_group_active_like_cpp(Some(&manual)));
        assert!(map.spawn_group_state().toggled_spawn_group_ids().is_empty());
    }

    fn dynamic_respawn_context(
        spawn_type: Option<SpawnObjectType>,
    ) -> DynamicRespawnScalingContext {
        DynamicRespawnScalingContext {
            mode: 1,
            spawn_type,
            spawn_metadata_present: true,
            spawn_group_flags: Some(SpawnGroupFlags::DYNAMIC_SPAWN_RATE),
            is_battleground_or_arena: false,
            zone_player_count: Some(4),
            config: DynamicRespawnScalingConfig {
                creature_rate: 1.0,
                creature_minimum_secs: 30,
                gameobject_rate: 1.5,
                gameobject_minimum_secs: 60,
            },
        }
    }

    fn assert_dynamic_respawn_noop(
        context: DynamicRespawnScalingContext,
        reason: DynamicRespawnScalingNoopReason,
    ) {
        let outcome = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);
        assert_eq!(outcome.delay_secs, 120);
        assert_eq!(outcome.noop_reason, Some(reason));
        assert!(!outcome.was_scaled());
    }

    #[test]
    fn dynamic_respawn_bg_or_arena_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.is_battleground_or_arena = true;

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::BattlegroundOrArena,
        );
    }

    fn linked_respawn_guid(high: HighGuid, entry: u32, spawn_id: SpawnId) -> ObjectGuid {
        ObjectGuid::create_world_object(high, 0, 0, 571, 0, entry, spawn_id as i64)
    }

    #[test]
    fn linked_respawn_time_missing_link_returns_zero_like_cpp() {
        let map = test_map();
        let store = LinkedRespawnStoreLikeCpp::new();

        assert_eq!(
            map.get_linked_respawn_time_like_cpp(
                linked_respawn_guid(HighGuid::Creature, 42, 100),
                &store,
            ),
            0
        );
    }

    #[test]
    fn linked_respawn_time_reads_creature_and_gameobject_timers_like_cpp() {
        let mut map = test_map();
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 1234,
            grid_id: 7,
        });
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::GameObject,
            spawn_id: 300,
            entry: 88,
            respawn_time: 5678,
            grid_id: 7,
        });
        let slave_creature = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let master_creature = linked_respawn_guid(HighGuid::Creature, 77, 200);
        let slave_go = linked_respawn_guid(HighGuid::GameObject, 43, 101);
        let master_go = linked_respawn_guid(HighGuid::GameObject, 88, 300);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(slave_creature, master_creature);
        linked.insert_like_cpp(slave_go, master_go);

        assert_eq!(
            map.get_linked_respawn_time_like_cpp(slave_creature, &linked),
            1234
        );
        assert_eq!(
            map.get_linked_respawn_time_like_cpp(slave_go, &linked),
            5678
        );
    }

    #[test]
    fn check_respawn_linked_respawn_guard_no_linked_time_leaves_info_unchanged_like_cpp() {
        let map = test_map();
        let linked = LinkedRespawnStoreLikeCpp::new();
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);
        let original = info.clone();

        let outcome = map.check_respawn_linked_respawn_guard_like_cpp(&mut info, &linked, 1000, 5);

        assert_eq!(
            outcome,
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::Allowed
        );
        assert_eq!(info, original);
    }

    #[test]
    fn check_respawn_linked_respawn_guard_self_link_sets_week_like_cpp() {
        let mut map = test_map();
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 100,
            entry: 42,
            respawn_time: 1200,
            grid_id: 7,
        });
        let this = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this, this);
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);

        let outcome = map.check_respawn_linked_respawn_guard_like_cpp(&mut info, &linked, 1000, 5);

        assert_eq!(
            outcome,
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedSelfNeverRespawn
        );
        assert_eq!(info.respawn_time, 1000 + WEEK_SECS_LIKE_CPP);
    }

    #[test]
    fn check_respawn_linked_respawn_guard_infinite_time_sets_i64_max_like_cpp() {
        let mut map = test_map();
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::GameObject,
            spawn_id: 200,
            entry: 77,
            respawn_time: i64::MAX,
            grid_id: 7,
        });
        let this = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let master = linked_respawn_guid(HighGuid::GameObject, 77, 200);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this, master);
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);

        let outcome = map.check_respawn_linked_respawn_guard_like_cpp(&mut info, &linked, 1000, 15);

        assert_eq!(
            outcome,
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedInfinite
        );
        assert_eq!(info.respawn_time, i64::MAX);
    }

    #[test]
    fn check_respawn_linked_respawn_guard_delays_by_max_now_or_linked_plus_jitter_like_cpp() {
        let mut map = test_map();
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 900,
            grid_id: 7,
        });
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::GameObject,
            spawn_id: 300,
            entry: 88,
            respawn_time: 1200,
            grid_id: 7,
        });
        let this_past = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let this_future = linked_respawn_guid(HighGuid::GameObject, 43, 101);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this_past, linked_respawn_guid(HighGuid::Creature, 77, 200));
        linked.insert_like_cpp(
            this_future,
            linked_respawn_guid(HighGuid::GameObject, 88, 300),
        );

        let mut past = respawn_info(SpawnObjectType::Creature, 100, 55);
        let past_outcome =
            map.check_respawn_linked_respawn_guard_like_cpp(&mut past, &linked, 1000, 5);
        assert_eq!(
            past_outcome,
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedDelayed
        );
        assert_eq!(past.respawn_time, 1005);

        let mut future = respawn_info(SpawnObjectType::GameObject, 101, 55);
        future.entry = 43;
        let future_outcome =
            map.check_respawn_linked_respawn_guard_like_cpp(&mut future, &linked, 1000, 15);
        assert_eq!(
            future_outcome,
            CheckRespawnLinkedRespawnGuardOutcomeLikeCpp::LinkedDelayed
        );
        assert_eq!(future.respawn_time, 1215);
    }

    #[test]
    fn check_respawn_like_cpp_inactive_spawn_group_stops_before_live_and_linked_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(61, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, manual), |_| {
            false
        });
        let this = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let master = linked_respawn_guid(HighGuid::Creature, 77, 200);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this, master);
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);
        let mut escort_checked = false;

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, true, |_, _| {
                escort_checked = true;
                false
            });

        assert_eq!(
            outcome,
            CheckRespawnCompositeOutcomeLikeCpp::InactiveSpawnGroupDeletedTimer
        );
        assert_eq!(info.respawn_time, 0);
        assert!(!escort_checked);
    }

    #[test]
    fn check_respawn_like_cpp_live_blocker_stops_before_linked_reschedule_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(62, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, group), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(100, 100, true)).unwrap(),
        )
        .unwrap();
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 1200,
            grid_id: 7,
        });
        let this = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let master = linked_respawn_guid(HighGuid::Creature, 77, 200);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this, master);
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnCompositeOutcomeLikeCpp::AliveCreatureBlocksRespawn
        );
        assert_eq!(info.respawn_time, 0);
    }

    #[test]
    fn check_respawn_like_cpp_linked_delayed_runs_after_allowed_guards_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(63, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, group), |_| {
            false
        });
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 1200,
            grid_id: 7,
        });
        let this = linked_respawn_guid(HighGuid::Creature, 42, 100);
        let master = linked_respawn_guid(HighGuid::Creature, 77, 200);
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(this, master);
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 11, false, |_, _| false);

        assert_eq!(outcome, CheckRespawnCompositeOutcomeLikeCpp::LinkedDelayed);
        assert_eq!(info.respawn_time, 1211);
    }

    #[test]
    fn check_respawn_like_cpp_allowed_path_preserves_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(64, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 101, group), |_| {
            false
        });
        let linked = LinkedRespawnStoreLikeCpp::new();
        let mut info = respawn_info(SpawnObjectType::GameObject, 101, 55);

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, false, |_, _| false);

        assert_eq!(outcome, CheckRespawnCompositeOutcomeLikeCpp::Allowed);
        assert_eq!(info.respawn_time, 55);
    }

    #[test]
    fn check_respawn_like_cpp_missing_metadata_preserves_timer_and_stops_like_cpp() {
        let map = test_map();
        let store = SpawnStore::new();
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(
            linked_respawn_guid(HighGuid::Creature, 42, 100),
            linked_respawn_guid(HighGuid::Creature, 77, 200),
        );
        let mut info = respawn_info(SpawnObjectType::Creature, 100, 55);
        let mut escort_checked = false;

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, true, |_, _| {
                escort_checked = true;
                false
            });

        assert_eq!(
            outcome,
            CheckRespawnCompositeOutcomeLikeCpp::MissingSpawnData
        );
        assert_eq!(info.respawn_time, 55);
        assert!(!escort_checked);
    }

    #[test]
    fn check_respawn_like_cpp_unsupported_areatrigger_preserves_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(65, SpawnGroupFlags::NONE);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::AreaTrigger, 102, group),
            |_| false,
        );
        let linked = LinkedRespawnStoreLikeCpp::new();
        let mut info = respawn_info(SpawnObjectType::AreaTrigger, 102, 55);

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, false, |_, _| false);

        assert_eq!(
            outcome,
            CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType
        );
        assert_eq!(info.respawn_time, 55);
    }

    #[test]
    fn check_respawn_like_cpp_areatrigger_manual_inactive_group_preserves_timer_like_cpp() {
        let map = test_map();
        let mut store = SpawnStore::new();
        let manual = spawn_group(66, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::AreaTrigger, 103, manual),
            |_| false,
        );
        let linked = LinkedRespawnStoreLikeCpp::new();
        let mut info = respawn_info(SpawnObjectType::AreaTrigger, 103, 55);
        let mut escort_checked = false;

        let outcome =
            map.check_respawn_like_cpp(&mut info, &store, &linked, 1000, 5, true, |_, _| {
                escort_checked = true;
                false
            });

        assert_eq!(
            outcome,
            CheckRespawnCompositeOutcomeLikeCpp::UnsupportedSpawnType
        );
        assert_eq!(info.respawn_time, 55);
        assert!(!escort_checked);
    }

    #[test]
    fn process_respawns_composite_live_creature_blocker_deletes_due_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(67, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, group), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_creature(test_creature_for_spawn(100, 100, true)).unwrap(),
        )
        .unwrap();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 100, 10));

        let summary = map.process_due_respawns_composite_delete_only_like_cpp(
            10,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            5,
            false,
            |_, _| false,
        );

        assert_eq!(summary.deleted_live_object_blocker, 1);
        assert_eq!(summary.blocked_do_respawn_runtime, 0);
        assert!(
            map.get_respawn_info_like_cpp(SpawnObjectType::Creature, 100)
                .is_none()
        );
    }

    #[test]
    fn process_respawns_composite_live_gameobject_blocker_deletes_due_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(68, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::GameObject, 101, group), |_| {
            false
        });
        map.insert_map_object_record(
            MapObjectRecord::new_game_object(test_gameobject_for_spawn(101, 101)).unwrap(),
        )
        .unwrap();
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 101, 10));

        let summary = map.process_due_respawns_composite_delete_only_like_cpp(
            10,
            &store,
            &LinkedRespawnStoreLikeCpp::new(),
            5,
            false,
            |_, _| false,
        );

        assert_eq!(summary.deleted_live_object_blocker, 1);
        assert_eq!(summary.blocked_do_respawn_runtime, 0);
        assert!(
            map.get_respawn_info_like_cpp(SpawnObjectType::GameObject, 101)
                .is_none()
        );
    }

    #[test]
    fn process_respawns_composite_linked_respawn_reschedules_future_timer_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let group = spawn_group(69, SpawnGroupFlags::NONE);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, group), |_| {
            false
        });
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 100, 10));
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 1200,
            grid_id: 7,
        });
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(
            linked_respawn_guid(HighGuid::Creature, 42, 100),
            linked_respawn_guid(HighGuid::Creature, 77, 200),
        );

        let summary = map.process_due_respawns_composite_delete_only_like_cpp(
            10,
            &store,
            &linked,
            5,
            false,
            |_, _| false,
        );

        assert_eq!(summary.rescheduled_linked_respawns.len(), 1);
        assert_eq!(summary.blocked_linked_respawn_non_future, 0);
        assert_eq!(summary.deleted_inactive_spawn_group, 0);
        assert_eq!(summary.deleted_live_object_blocker, 0);
        let rescheduled = &summary.rescheduled_linked_respawns[0];
        assert_eq!(rescheduled.spawn_id, 100);
        assert_eq!(rescheduled.respawn_time, 1205);
        assert_eq!(
            map.get_respawn_info_like_cpp(SpawnObjectType::Creature, 100)
                .unwrap()
                .respawn_time,
            1205
        );
    }

    #[test]
    fn process_respawns_composite_linked_reschedule_allows_later_due_delete_like_cpp() {
        let mut map = test_map();
        let mut store = SpawnStore::new();
        let active = spawn_group(70, SpawnGroupFlags::NONE);
        let inactive = spawn_group(71, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&spawn_data(SpawnObjectType::Creature, 100, active), |_| {
            false
        });
        store.add_object_spawn(
            &spawn_data(SpawnObjectType::Creature, 101, inactive),
            |_| false,
        );
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 100, 9));
        map.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 101, 10));
        map.add_respawn_info_like_cpp(RespawnInfoLikeCpp {
            object_type: SpawnObjectType::Creature,
            spawn_id: 200,
            entry: 77,
            respawn_time: 1200,
            grid_id: 7,
        });
        let mut linked = LinkedRespawnStoreLikeCpp::new();
        linked.insert_like_cpp(
            linked_respawn_guid(HighGuid::Creature, 42, 100),
            linked_respawn_guid(HighGuid::Creature, 77, 200),
        );

        let summary = map.process_due_respawns_composite_delete_only_like_cpp(
            10,
            &store,
            &linked,
            5,
            false,
            |_, _| false,
        );

        assert_eq!(summary.rescheduled_linked_respawns.len(), 1);
        assert_eq!(summary.deleted_inactive_spawn_group, 1);
        assert_eq!(
            map.get_respawn_info_like_cpp(SpawnObjectType::Creature, 100)
                .unwrap()
                .respawn_time,
            1205
        );
        assert!(
            map.get_respawn_info_like_cpp(SpawnObjectType::Creature, 101)
                .is_none()
        );
    }

    #[test]
    fn dynamic_respawn_unsupported_type_and_missing_metadata_do_not_scale() {
        assert_dynamic_respawn_noop(
            dynamic_respawn_context(Some(SpawnObjectType::AreaTrigger)),
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );

        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.spawn_metadata_present = false;
        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    }

    #[test]
    fn dynamic_respawn_without_dynamic_spawn_rate_flag_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.spawn_group_flags = Some(SpawnGroupFlags::NONE);

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::MissingDynamicSpawnRateFlag,
        );
    }

    #[test]
    fn dynamic_respawn_missing_or_zero_players_do_not_scale() {
        let mut missing = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        missing.zone_player_count = None;
        assert_dynamic_respawn_noop(
            missing,
            DynamicRespawnScalingNoopReason::MissingZonePlayerCount,
        );

        let mut zero = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        zero.zone_player_count = Some(0);
        assert_dynamic_respawn_noop(zero, DynamicRespawnScalingNoopReason::ZeroZonePlayers);
    }

    #[test]
    fn dynamic_respawn_adjust_factor_at_least_one_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.zone_player_count = Some(1);
        context.config.gameobject_rate = 1.0;

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::AdjustFactorAtLeastOne,
        );
    }

    #[test]
    fn dynamic_respawn_delay_at_or_below_minimum_does_not_scale() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        let outcome = apply_dynamic_mode_respawn_scaling_like_cpp(60, context);

        assert_eq!(outcome.delay_secs, 60);
        assert_eq!(
            outcome.noop_reason,
            Some(DynamicRespawnScalingNoopReason::DelayAtOrBelowMinimum)
        );
    }

    #[test]
    fn dynamic_respawn_gameobject_ceil_scales_and_clamps_to_minimum() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        let scaled = apply_dynamic_mode_respawn_scaling_like_cpp(241, context);

        assert_eq!(scaled.delay_secs, 91);
        assert!(scaled.was_scaled());

        let clamped = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);
        assert_eq!(clamped.delay_secs, 60);
        assert!(clamped.was_scaled());
    }

    #[test]
    fn dynamic_respawn_creature_uses_creature_rate_and_minimum() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::Creature));
        let scaled = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);

        assert_eq!(scaled.delay_secs, 30);
        assert!(scaled.was_scaled());
    }

    #[test]
    fn dynamic_respawn_unsupported_mode_is_safe_noop() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.mode = 2;

        assert_dynamic_respawn_noop(context, DynamicRespawnScalingNoopReason::UnsupportedMode);
    }

    fn guid(high: HighGuid, counter: i64) -> ObjectGuid {
        if high == HighGuid::Player {
            ObjectGuid::create_global(high, 0, counter)
        } else if high == HighGuid::Transport {
            ObjectGuid::create_transport(high, counter)
        } else {
            ObjectGuid::create_world_object(high, 0, 1, 571, 7, 100, counter)
        }
    }

    fn world_object(high: HighGuid, map_id: u32, instance_id: u32, in_world: bool) -> WorldObject {
        let type_id = guid(high, 1).type_id();
        let type_mask = match type_id {
            wow_core::guid::TypeId::Player => TypeMask::PLAYER,
            wow_core::guid::TypeId::Unit => TypeMask::UNIT,
            wow_core::guid::TypeId::GameObject => TypeMask::GAME_OBJECT,
            wow_core::guid::TypeId::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            wow_core::guid::TypeId::Corpse => TypeMask::CORPSE,
            wow_core::guid::TypeId::AreaTrigger => TypeMask::AREA_TRIGGER,
            wow_core::guid::TypeId::SceneObject => TypeMask::SCENE_OBJECT,
            wow_core::guid::TypeId::Conversation => TypeMask::CONVERSATION,
            _ => TypeMask::OBJECT,
        };
        let mut object = WorldObject::new(false, convert_type_id(type_id), type_mask);
        object.object_mut().create(guid(high, 1));
        object.set_map(map_id, instance_id).unwrap();
        object.relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            object.object_mut().add_to_world();
        }
        object
    }

    fn world_object_with_counter(
        high: HighGuid,
        counter: i64,
        map_id: u32,
        instance_id: u32,
        in_world: bool,
    ) -> WorldObject {
        let object_guid = guid(high, counter);
        let type_id = object_guid.type_id();
        let type_mask = match type_id {
            wow_core::guid::TypeId::Player => TypeMask::PLAYER,
            wow_core::guid::TypeId::Unit => TypeMask::UNIT,
            wow_core::guid::TypeId::GameObject => TypeMask::GAME_OBJECT,
            wow_core::guid::TypeId::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            wow_core::guid::TypeId::Corpse => TypeMask::CORPSE,
            wow_core::guid::TypeId::AreaTrigger => TypeMask::AREA_TRIGGER,
            wow_core::guid::TypeId::SceneObject => TypeMask::SCENE_OBJECT,
            wow_core::guid::TypeId::Conversation => TypeMask::CONVERSATION,
            _ => TypeMask::OBJECT,
        };
        let mut object = WorldObject::new(false, convert_type_id(type_id), type_mask);
        object.object_mut().create(object_guid);
        object.set_map(map_id, instance_id).unwrap();
        object.relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            object.object_mut().add_to_world();
        }
        object
    }

    fn game_object_with_counter(
        counter: i64,
        map_id: u32,
        instance_id: u32,
        in_world: bool,
    ) -> GameObject {
        let mut game_object = GameObject::new();
        game_object
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::GameObject, counter));
        game_object
            .world_mut()
            .set_map(map_id, instance_id)
            .unwrap();
        game_object
            .world_mut()
            .relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            game_object.world_mut().object_mut().add_to_world();
        }
        game_object
    }

    fn convert_type_id(type_id: wow_core::guid::TypeId) -> TypeId {
        match type_id {
            wow_core::guid::TypeId::Object => TypeId::Object,
            wow_core::guid::TypeId::Item => TypeId::Item,
            wow_core::guid::TypeId::Container => TypeId::Container,
            wow_core::guid::TypeId::AzeriteEmpoweredItem => TypeId::AzeriteEmpoweredItem,
            wow_core::guid::TypeId::AzeriteItem => TypeId::AzeriteItem,
            wow_core::guid::TypeId::Unit => TypeId::Unit,
            wow_core::guid::TypeId::Player => TypeId::Player,
            wow_core::guid::TypeId::ActivePlayer => TypeId::ActivePlayer,
            wow_core::guid::TypeId::GameObject => TypeId::GameObject,
            wow_core::guid::TypeId::DynamicObject => TypeId::DynamicObject,
            wow_core::guid::TypeId::Corpse => TypeId::Corpse,
            wow_core::guid::TypeId::AreaTrigger => TypeId::AreaTrigger,
            wow_core::guid::TypeId::SceneObject => TypeId::SceneObject,
            wow_core::guid::TypeId::Conversation => TypeId::Conversation,
        }
    }

    #[test]
    fn map_constructor_starts_with_empty_grid_slots_like_cpp_pointer_array() {
        let map = test_map();

        assert_eq!(map.map_id(), 571);
        assert_eq!(map.instance_id(), 7);
        assert_eq!(map.spawn_mode(), 1);
        assert_eq!(map.grid_expiry_ms(), 1000);
        assert!(map.grid_unload());
        assert_eq!(map.visibility_range(), 100.0);
        assert_eq!(map.grids.len(), GRID_SLOT_COUNT);
        assert!(map.grids.iter().all(Option::is_none));
    }

    #[test]
    fn map_object_store_inserts_finds_typed_objects_and_removes_by_guid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let gameobject = world_object(HighGuid::GameObject, 571, 7, true);
        let creature_guid = creature.guid();
        let gameobject_guid = gameobject.guid();

        assert!(
            map.insert_map_object(AccessorObjectKind::Creature, creature)
                .unwrap()
                .is_none()
        );
        assert!(
            map.insert_map_object(AccessorObjectKind::GameObject, gameobject)
                .unwrap()
                .is_none()
        );

        assert_eq!(map.map_object_count(), 2);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().guid(),
            creature_guid
        );
        assert_eq!(
            map.get_game_object(gameobject_guid).unwrap().guid(),
            gameobject_guid
        );
        assert!(map.get_game_object(creature_guid).is_none());

        assert_eq!(
            map.remove_map_object(creature_guid)
                .unwrap()
                .object()
                .guid(),
            creature_guid
        );
        assert!(map.get_creature(creature_guid).is_none());
        assert_eq!(map.map_object_count(), 1);
    }

    #[test]
    fn map_object_store_can_hold_typed_gameobject_entity_like_cpp() {
        let mut map = test_map();
        let mut gameobject = GameObject::new();
        let guid = guid(HighGuid::GameObject, 77);
        gameobject.world_mut().object_mut().create(guid);
        gameobject.world_mut().object_mut().set_entry(123);
        gameobject.world_mut().set_map(571, 7).unwrap();
        gameobject
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        gameobject.set_created_by(ObjectGuid::create_player(1, 42));

        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        assert_eq!(map.get_game_object(guid).unwrap().guid(), guid);
        assert_eq!(
            map.get_typed_game_object(guid).unwrap().owner_guid(),
            ObjectGuid::create_player(1, 42)
        );
    }

    #[test]
    fn map_object_store_can_hold_typed_creature_entity_like_cpp() {
        let mut map = test_map();
        let mut creature = Creature::new(false);
        let guid = guid(HighGuid::Creature, 78);
        creature.unit_mut().world_mut().object_mut().create(guid);
        creature.unit_mut().world_mut().object_mut().set_entry(321);
        creature.unit_mut().world_mut().set_map(571, 7).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        creature.unit_mut().world_mut().object_mut().add_to_world();
        creature.unit_mut().set_level(42);

        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        assert_eq!(map.get_creature(guid).unwrap().guid(), guid);
        assert_eq!(
            map.get_typed_creature(guid).unwrap().unit().data().level,
            42
        );
        map.get_typed_creature_mut(guid)
            .unwrap()
            .unit_mut()
            .set_level(43);
        assert_eq!(
            map.get_typed_creature(guid).unwrap().unit().data().level,
            43
        );
    }

    #[test]
    fn map_object_store_can_hold_typed_player_entity_like_cpp() {
        let mut map = test_map();
        let mut player = Player::new(Some(7), false);
        let player_guid = guid(HighGuid::Player, 42);
        let victim_guid = guid(HighGuid::Creature, 77);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.unit_mut().world_mut().set_map(571, 7).unwrap();
        player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        player.unit_mut().world_mut().object_mut().add_to_world();
        player.unit_mut().set_attacking(Some(victim_guid));

        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        assert_eq!(map.map_object(player_guid).unwrap().guid(), player_guid);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .unit()
                .attacking(),
            Some(victim_guid)
        );
        map.get_typed_player_mut(player_guid)
            .unwrap()
            .unit_mut()
            .set_attacking(None);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .unit()
                .attacking(),
            None
        );
    }

    #[test]
    fn map_revalidates_all_typed_combat_refs_like_cpp_multi_owner_sweep() {
        let mut map = test_map();
        let alive_player_guid = guid(HighGuid::Player, 501);
        let dead_player_guid = guid(HighGuid::Player, 502);
        let creature_guid = guid(HighGuid::Creature, 503);

        let mut alive_player = Player::new(Some(7), false);
        alive_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(alive_player_guid);
        alive_player.unit_mut().world_mut().set_map(571, 7).unwrap();
        alive_player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        alive_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();

        let mut dead_player = Player::new(Some(7), false);
        dead_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(dead_player_guid);
        dead_player.unit_mut().world_mut().set_map(571, 7).unwrap();
        dead_player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(11.0, 20.0, 30.0));
        dead_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();
        dead_player.unit_mut().set_death_state(DeathState::Dead);

        let mut creature = Creature::new(false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(creature_guid);
        creature.unit_mut().world_mut().set_map(571, 7).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(12.0, 20.0, 30.0));
        creature.unit_mut().world_mut().object_mut().add_to_world();

        map.insert_map_object_record(MapObjectRecord::new_player(alive_player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_player(dead_player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        map.get_typed_player_mut(alive_player_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(creature_guid, false, false);
        map.get_typed_creature_mut(creature_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(alive_player_guid, false, false);
        map.get_typed_player_mut(dead_player_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(creature_guid, false, false);
        map.get_typed_creature_mut(creature_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(dead_player_guid, false, false);

        let invalid = map.revalidate_all_combat_refs_like_cpp();

        assert!(invalid.contains(&(dead_player_guid, creature_guid)));
        assert!(invalid.contains(&(creature_guid, dead_player_guid)));
        assert!(
            map.get_typed_player(alive_player_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(creature_guid)
        );
        assert!(
            map.get_typed_creature(creature_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(alive_player_guid)
        );
        assert!(
            !map.get_typed_player(dead_player_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(creature_guid)
        );
        assert!(
            !map.get_typed_creature(creature_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(dead_player_guid)
        );
    }

    #[test]
    fn map_object_store_rejects_records_from_other_map_or_instance() {
        let mut map = test_map();
        let other_map_creature = world_object(HighGuid::Creature, 530, 7, true);
        let other_instance_creature = world_object(HighGuid::Creature, 571, 8, true);

        assert!(matches!(
            map.insert_map_object(AccessorObjectKind::Creature, other_map_creature),
            Err(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                expected_instance_id: 7,
                actual_map_id: 530,
                actual_instance_id: 7,
                ..
            })
        ));
        assert!(matches!(
            map.insert_map_object(AccessorObjectKind::Creature, other_instance_creature),
            Err(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                expected_instance_id: 7,
                actual_map_id: 571,
                actual_instance_id: 8,
                ..
            })
        ));
        assert_eq!(map.map_object_count(), 0);
    }

    #[test]
    fn object_accessor_can_consult_map_owned_object_store() {
        let accessor = ObjectAccessor::default();
        let mut map = test_map();
        let context = world_object(HighGuid::Player, 571, 7, true);
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();

        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();

        assert_eq!(
            accessor
                .get_world_object_from_map_source(&context, &map, creature_guid)
                .unwrap()
                .guid(),
            creature_guid
        );
        assert!(matches!(
            accessor.get_object_ref_by_type_mask_from_map_source(
                &context,
                &map,
                creature_guid,
                TypeMask::UNIT
            ),
            Some(AccessorObjectRef::WorldObject(object)) if object.guid() == creature_guid
        ));
    }

    #[test]
    fn add_to_map_like_cpp_creates_grid_marks_world_and_stores_grid_object() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.inserted);
        assert!(!outcome.already_in_world);
        assert!(outcome.grid_created);
        assert!(!outcome.grid_loaded);
        assert!(outcome.inserted_into_cell);

        let stored = map.get_creature(guid).unwrap();
        assert!(stored.object().is_in_world());
        assert!(stored.object().is_in_grid());
        assert!(!stored.object().is_new_object());
        assert_eq!(
            stored.current_cell(),
            Some((
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS
            ))
        );

        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.grid_objects.creatures.contains(&guid));
        assert!(!cell.world_objects.creatures.contains(&guid));
    }

    #[test]
    fn add_map_object_record_to_map_like_cpp_preserves_typed_creature_spawn_index() {
        let mut map = test_map();
        let mut creature = test_creature_for_spawn(396, 39601, false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        let guid = creature.guid();

        let outcome = map
            .add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.inserted);
        assert!(!outcome.already_in_world);
        assert!(outcome.inserted_into_cell);
        assert!(map.get_creature_by_spawn_id_like_cpp(396).is_some());
        assert!(
            map.map_object_record(guid)
                .and_then(MapObjectRecord::creature)
                .is_some()
        );

        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.grid_objects.creatures.contains(&guid));
        assert!(!cell.world_objects.creatures.contains(&guid));
    }

    #[test]
    fn add_map_object_record_to_map_like_cpp_preserves_typed_gameobject_spawn_index() {
        let mut map = test_map();
        let mut gameobject = test_gameobject_for_spawn(396, 39602);
        gameobject.world_mut().object_mut().remove_from_world();
        let guid = gameobject.world().guid();

        let outcome = map
            .add_map_object_record_to_map_like_cpp(
                MapObjectRecord::new_game_object(gameobject).unwrap(),
            )
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.inserted);
        assert!(!outcome.already_in_world);
        assert!(outcome.inserted_into_cell);
        assert!(map.get_gameobject_by_spawn_id_like_cpp(396).is_some());
        assert!(
            map.map_object_record(guid)
                .and_then(MapObjectRecord::game_object)
                .is_some()
        );

        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.grid_objects.gameobjects.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_active_world_object_loads_grid_and_world_container() {
        let mut map = test_map();
        let mut object = WorldObject::new(true, TypeId::DynamicObject, TypeMask::DYNAMIC_OBJECT);
        object.object_mut().create(guid(HighGuid::DynamicObject, 2));
        object.set_map(571, 7).unwrap();
        object.relocate(Position::xyz(20.0, 20.0, 3.0));
        object.set_active(true);
        let guid = object.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::DynamicObject, object)
            .unwrap();

        assert!(outcome.grid_loaded);
        assert!(!outcome.grid_created);
        assert!(map.is_grid_loaded(outcome.grid));
        assert_eq!(map.lifecycle().loads, 1);
        let grid = map.get_ngrid(outcome.grid).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.world_objects.dynamic_objects.contains(&guid));
        assert!(!cell.grid_objects.dynamic_objects.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_player_is_active_even_without_runtime_active_flag() {
        let mut map = test_map();
        let player = world_object(HighGuid::Player, 571, 7, false);
        let guid = player.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.grid_loaded);
        assert!(!outcome.grid_created);
        assert!(map.is_grid_loaded(outcome.grid));
        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.world_objects.players.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_rejects_invalid_coordinates_before_grid_mutation() {
        let mut map = test_map();
        let mut creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        creature.relocate(Position::xyz(f32::NAN, 0.0, 0.0));

        assert!(matches!(
            map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature),
            Err(AddToMapError::InvalidCoordinates { guid: actual, .. }) if actual == guid
        ));
        assert_eq!(map.map_object_count(), 0);
        assert!(map.terrain().loads.is_empty());
    }

    #[test]
    fn add_to_map_like_cpp_rejects_wrong_map_before_grid_mutation() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 530, 7, false);

        assert!(matches!(
            map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature),
            Err(AddToMapError::Store(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                actual_map_id: 530,
                ..
            }))
        ));
        assert_eq!(map.map_object_count(), 0);
        assert!(map.terrain().loads.is_empty());
    }

    #[test]
    fn remove_from_map_like_cpp_removes_store_cell_and_resets_object_binding() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        assert!(map.get_creature(guid).is_some());

        let removed = map.remove_from_map_like_cpp(guid, false).unwrap();

        assert_eq!(removed.guid, guid);
        assert_eq!(removed.cell, added.cell);
        assert!(removed.was_in_world);
        assert!(!removed.was_active);
        assert!(removed.removed_from_cell);
        assert!(!removed.delete_from_world);
        assert!(map.get_creature(guid).is_none());

        let grid = map.get_ngrid(removed.grid).unwrap();
        let cell = grid
            .get_grid_type(
                removed.cell.x_coord % MAX_NUMBER_OF_CELLS,
                removed.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(!cell.grid_objects.creatures.contains(&guid));

        let object = removed.object.unwrap();
        assert!(!object.object().is_in_world());
        assert!(!object.object().is_in_grid());
        assert!(!object.has_current_map());
        assert_eq!(object.current_cell(), None);
    }

    #[test]
    fn remove_list_enqueue_creature_marks_destroyed_cleans_and_keeps_record_like_cpp() {
        let mut map = test_map();
        let spawn_id = 41901;
        let mut creature = test_creature_for_spawn(spawn_id, 4190101, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        let added = map
            .add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let outcome = map.add_object_to_remove_list_like_cpp(guid);

        assert_eq!(outcome.guid, guid);
        assert!(outcome.queued);
        assert!(!outcome.duplicate);
        assert_eq!(outcome.cleanup_before_delete_count, 1);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        assert_eq!(map.map_object_count(), 1);
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 1);
        assert!(
            map.exact_cell_guids_like_cpp(added.cell)
                .grid
                .creatures
                .contains(&guid)
        );
        let creature = map.get_typed_creature(guid).unwrap();
        assert!(creature.unit().world().object().is_destroyed_object());
        assert_eq!(creature.cleanup_before_delete_count(), 1);
    }

    #[test]
    fn remove_list_drain_physically_removes_creature_and_second_cleanup_like_cpp() {
        let mut map = test_map();
        let spawn_id = 41902;
        let mut creature = test_creature_for_spawn(spawn_id, 4190201, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        let added = map
            .add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();
        assert!(map.add_object_to_remove_list_like_cpp(guid).queued);

        let outcome = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(outcome.processed, 1);
        assert_eq!(outcome.removed, 1);
        assert_eq!(outcome.creature_second_cleanup_count, 1);
        assert_eq!(outcome.missing_or_stale, 0);
        assert_eq!(outcome.remove_errors, 0);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert!(map.map_object_record(guid).is_none());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 0);
        assert!(
            !map.exact_cell_guids_like_cpp(added.cell)
                .grid
                .creatures
                .contains(&guid)
        );
    }

    #[test]
    fn remove_list_duplicate_enqueue_follows_cpp_cleanup_before_set_insert_like_cpp() {
        let mut map = test_map();
        let mut creature = test_creature_for_spawn(41903, 4190301, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let first = map.add_object_to_remove_list_like_cpp(guid);
        let second = map.add_object_to_remove_list_like_cpp(guid);

        assert!(first.queued);
        assert!(second.duplicate);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        assert_eq!(
            map.get_typed_creature(guid)
                .unwrap()
                .cleanup_before_delete_count(),
            2
        );
    }

    #[test]
    fn remove_list_drain_missing_stale_guid_does_not_create_object_like_cpp() {
        let mut map = test_map();
        let guid = guid(HighGuid::Creature, 4190401);
        map.enqueue_object_to_remove_for_test(guid);

        let outcome = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(outcome.processed, 1);
        assert_eq!(outcome.missing_or_stale, 1);
        assert_eq!(outcome.removed, 0);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 0);
    }

    fn add_loaded_grid_creature_for_switch(
        map: &mut Map<RecordingTerrain, RecordingLifecycle>,
        spawn_id: SpawnId,
        counter: i64,
    ) -> (ObjectGuid, CellCoord, GridCoord) {
        let cell = Cell::from_world(1.0, 2.0);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        map.ensure_grid_loaded(&cell);
        let mut creature = test_creature_for_spawn(spawn_id, counter, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        let outcome = map
            .add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();
        assert!(outcome.inserted_into_cell);
        (guid, outcome.cell, grid)
    }

    fn local_cell_for_switch<'a>(
        map: &'a Map<RecordingTerrain, RecordingLifecycle>,
        grid: GridCoord,
        cell: CellCoord,
    ) -> &'a Cell {
        map.get_ngrid(grid)
            .unwrap()
            .get_grid_type(
                cell.x_coord % MAX_NUMBER_OF_CELLS,
                cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap()
    }

    fn test_player_for_viewpoint(counter: i64) -> Player {
        let mut player = Player::new(Some(7), false);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::Player, counter));
        player.unit_mut().world_mut().set_map(571, 7).unwrap();
        player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        player.unit_mut().world_mut().object_mut().add_to_world();
        player
    }

    fn test_dynamic_object_for_viewpoint(counter: i64) -> DynamicObject {
        let mut dynamic_object = DynamicObject::new(true);
        dynamic_object
            .world_mut()
            .object_mut()
            .create(guid(HighGuid::DynamicObject, counter));
        dynamic_object.world_mut().set_map(571, 7).unwrap();
        dynamic_object
            .world_mut()
            .relocate(Position::xyz(11.0, 21.0, 31.0));
        dynamic_object.world_mut().object_mut().add_to_world();
        dynamic_object
    }

    fn create_farsight_focus_for_tests<Terrain, Lifecycle>(
        map: &mut Map<Terrain, Lifecycle>,
        caster_player_guid: ObjectGuid,
    ) -> FarsightDynamicObjectCreateOutcomeLikeCpp
    where
        Terrain: TerrainGridLoader,
        Lifecycle: GridLifecycle,
    {
        map.create_farsight_dynamic_object_like_cpp(
            caster_player_guid,
            12_345,
            678,
            Position::new(100.0, 200.0, 30.0, 1.5),
            42.5,
            30_000,
            987_654,
            1,
            7,
        )
    }

    #[test]
    fn farsight_dynamic_object_create_inserts_focus_and_sets_viewpoint_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4280101);
        let player_guid = player.guid();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome = create_farsight_focus_for_tests(&mut map, player_guid);

        assert_eq!(
            outcome.status,
            FarsightDynamicObjectCreateStatusLikeCpp::Created
        );
        assert_eq!(outcome.caster_player_guid, player_guid);
        assert_eq!(outcome.low_guid, Some(1));
        let dynamic_guid = outcome.dynamic_object_guid.unwrap();
        assert_eq!(dynamic_guid.high_type(), HighGuid::DynamicObject);
        assert_ne!(dynamic_guid.counter(), 12_345);
        assert_eq!(
            map.get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                .unwrap(),
            2
        );
        let add_to_map = outcome.add_to_map.unwrap();
        assert!(add_to_map.inserted);
        assert!(add_to_map.inserted_into_cell);
        assert!(!add_to_map.already_in_world);

        let dynamic_object = map.get_typed_dynamic_object(dynamic_guid).unwrap();
        assert_eq!(dynamic_object.world().guid(), dynamic_guid);
        assert_eq!(dynamic_object.world().map_id(), 571);
        assert_eq!(dynamic_object.world().instance_id(), 7);
        assert_eq!(
            dynamic_object.world().position(),
            Position::new(100.0, 200.0, 30.0, 1.5)
        );
        assert!(dynamic_object.world().object().is_in_world());
        assert!(dynamic_object.world().is_active());
        assert_eq!(dynamic_object.world().object().entry(), 12_345);
        assert_eq!(dynamic_object.world().object().scale(), 1.0);
        assert_eq!(dynamic_object.caster_guid(), player_guid);
        assert_eq!(dynamic_object.bound_caster(), Some(player_guid));
        assert_eq!(
            dynamic_object.data().dynamic_object_type,
            DynamicObjectType::FarsightFocus as u8
        );
        assert_eq!(dynamic_object.data().spell_visual_id, 678);
        assert_eq!(dynamic_object.spell_id(), 12_345);
        assert_eq!(dynamic_object.radius(), 42.5);
        assert_eq!(dynamic_object.data().cast_time_ms, 987_654);
        assert_eq!(dynamic_object.duration_ms(), 30_000);
        assert!(dynamic_object.is_caster_viewpoint());
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_guid
        );
        let viewpoint = outcome.caster_viewpoint.unwrap();
        assert_eq!(viewpoint.dynamic_object_guid, dynamic_guid);
        assert_eq!(
            viewpoint.status,
            DynamicObjectCasterViewpointStatusLikeCpp::CasterPlayerResolved
        );
        assert_eq!(
            viewpoint.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::Applied
        );
        assert!(viewpoint.player_set_viewpoint.update_visibility_requested);
        assert!(viewpoint.player_set_viewpoint.set_seer_requested);
    }

    #[test]
    fn farsight_dynamic_object_create_missing_caster_does_not_mutate_or_consume_low_guid_like_cpp()
    {
        let mut map = test_map();
        let missing_player_guid = guid(HighGuid::Player, 4280201);

        let outcome = create_farsight_focus_for_tests(&mut map, missing_player_guid);

        assert_eq!(
            outcome.status,
            FarsightDynamicObjectCreateStatusLikeCpp::MissingCasterPlayer
        );
        assert_eq!(outcome.dynamic_object_guid, None);
        assert_eq!(map.map_objects.len(), 0);
        assert_eq!(
            map.get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                .unwrap(),
            1
        );
    }

    #[test]
    fn farsight_dynamic_object_create_untyped_caster_record_does_not_mutate_like_cpp() {
        let mut map = test_map();
        let mut player_object = world_object_with_counter(HighGuid::Player, 4280301, 571, 7, true);
        let player_guid = player_object.guid();
        player_object.object_mut().add_to_world();
        map.insert_map_object(AccessorObjectKind::Player, player_object)
            .unwrap();

        let outcome = create_farsight_focus_for_tests(&mut map, player_guid);

        assert_eq!(
            outcome.status,
            FarsightDynamicObjectCreateStatusLikeCpp::MissingCasterPlayer
        );
        assert_eq!(map.map_objects.len(), 1);
        assert_eq!(
            map.get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                .unwrap(),
            1
        );
    }

    #[test]
    fn farsight_dynamic_object_create_caster_not_in_world_or_wrong_map_do_not_mutate_like_cpp() {
        let mut not_in_world_map = test_map();
        let mut not_in_world_player = test_player_for_viewpoint(4280401);
        let not_in_world_guid = not_in_world_player.guid();
        not_in_world_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        not_in_world_map
            .insert_map_object_record(MapObjectRecord::new_player(not_in_world_player).unwrap())
            .unwrap();

        let not_in_world =
            create_farsight_focus_for_tests(&mut not_in_world_map, not_in_world_guid);

        assert_eq!(
            not_in_world.status,
            FarsightDynamicObjectCreateStatusLikeCpp::CasterNotInWorld
        );
        assert_eq!(not_in_world_map.map_objects.len(), 1);
        assert_eq!(
            not_in_world_map
                .get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                .unwrap(),
            1
        );

        let mut wrong_map = test_map();
        let wrong_map_player = test_player_for_viewpoint(4280402);
        let wrong_map_guid = wrong_map_player.guid();
        wrong_map
            .insert_map_object_record(MapObjectRecord::new_player(wrong_map_player).unwrap())
            .unwrap();
        wrong_map.map_id = 530;

        let wrong_map_outcome = create_farsight_focus_for_tests(&mut wrong_map, wrong_map_guid);

        assert_eq!(
            wrong_map_outcome.status,
            FarsightDynamicObjectCreateStatusLikeCpp::CasterWrongMap
        );
        assert_eq!(wrong_map.map_objects.len(), 1);
        assert_eq!(
            wrong_map
                .get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                .unwrap(),
            1
        );
    }

    #[test]
    fn farsight_dynamic_object_create_invalid_destination_preserves_no_mutation_like_cpp() {
        let invalid_destinations = [
            Position::new(f32::NAN, 200.0, 30.0, 1.5),
            Position::new(100.0, 200.0, f32::NAN, 1.5),
            Position::new(100.0, 200.0, Position::MAP_HALFSIZE_LIKE_CPP, 1.5),
            Position::new(100.0, 200.0, 30.0, f32::NAN),
            Position::new(100.0, 200.0, 30.0, f32::INFINITY),
        ];

        for (index, dest) in invalid_destinations.into_iter().enumerate() {
            let mut map = test_map();
            let player = test_player_for_viewpoint(4280501 + index as i64);
            let player_guid = player.guid();
            map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
                .unwrap();

            let outcome = map.create_farsight_dynamic_object_like_cpp(
                player_guid,
                12_345,
                678,
                dest,
                42.5,
                30_000,
                987_654,
                1,
                7,
            );

            assert_eq!(
                outcome.status,
                FarsightDynamicObjectCreateStatusLikeCpp::InvalidDestination
            );
            assert_eq!(map.map_objects.len(), 1);
            assert_eq!(
                map.get_max_low_guid_like_cpp(HighGuid::DynamicObject)
                    .unwrap(),
                1
            );
            assert_eq!(
                map.get_typed_player(player_guid)
                    .unwrap()
                    .active_data()
                    .farsight_object,
                ObjectGuid::EMPTY
            );
        }
    }

    #[test]
    fn farsight_dynamic_object_create_reports_viewpoint_no_mutation_without_panicking_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4280601);
        let player_guid = player.guid();
        let existing_guid = guid(HighGuid::Creature, 4280609);
        player.set_farsight_object_like_cpp(existing_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome = create_farsight_focus_for_tests(&mut map, player_guid);

        assert_eq!(
            outcome.status,
            FarsightDynamicObjectCreateStatusLikeCpp::Created
        );
        let dynamic_guid = outcome.dynamic_object_guid.unwrap();
        let viewpoint = outcome.caster_viewpoint.unwrap();
        assert_eq!(
            viewpoint.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::AlreadyHasViewpoint
        );
        assert!(!viewpoint.player_set_viewpoint.update_visibility_requested);
        assert!(!viewpoint.player_set_viewpoint.set_seer_requested);
        assert!(viewpoint.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(
            map.get_typed_dynamic_object(dynamic_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
    }

    #[test]
    fn dynamic_object_caster_viewpoint_apply_sets_player_and_toggles_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4260101);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260102);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, true);

        assert_eq!(outcome.player_guid, player_guid);
        assert_eq!(outcome.dynamic_object_guid, dynamic_object_guid);
        assert!(outcome.apply);
        assert_eq!(
            outcome.status,
            DynamicObjectCasterViewpointStatusLikeCpp::CasterPlayerResolved
        );
        assert!(outcome.dynamic_object_viewpoint_toggled);
        assert_eq!(
            outcome.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::Applied
        );
        assert_eq!(outcome.player_set_viewpoint.set_world_object, None);
        assert!(outcome.player_set_viewpoint.update_visibility_requested);
        assert!(outcome.player_set_viewpoint.set_seer_requested);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_object_guid
        );
        assert!(
            map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
    }

    #[test]
    fn dynamic_object_caster_viewpoint_apply_existing_viewpoint_only_toggles_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4260201);
        let player_guid = player.guid();
        let existing_guid = guid(HighGuid::Creature, 4260209);
        player.set_farsight_object_like_cpp(existing_guid);
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260202);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, true);

        assert_eq!(
            outcome.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::AlreadyHasViewpoint
        );
        assert_eq!(outcome.player_set_viewpoint.set_world_object, None);
        assert!(!outcome.player_set_viewpoint.update_visibility_requested);
        assert!(!outcome.player_set_viewpoint.set_seer_requested);
        assert!(outcome.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(
            map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
    }

    #[test]
    fn dynamic_object_caster_viewpoint_remove_match_clears_player_and_toggles_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4260301);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260302);
        let dynamic_object_guid = dynamic_object.world().guid();
        player.set_farsight_object_like_cpp(dynamic_object_guid);
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome =
            map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, false);

        assert_eq!(
            outcome.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::Removed
        );
        assert_eq!(outcome.player_set_viewpoint.set_world_object, None);
        assert!(!outcome.player_set_viewpoint.update_visibility_requested);
        assert!(outcome.player_set_viewpoint.set_seer_requested);
        assert!(outcome.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert!(
            !map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
    }

    #[test]
    fn dynamic_object_caster_viewpoint_remove_mismatch_only_toggles_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4260401);
        let player_guid = player.guid();
        let existing_guid = guid(HighGuid::Creature, 4260409);
        player.set_farsight_object_like_cpp(existing_guid);
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260402);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome =
            map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, false);

        assert_eq!(
            outcome.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::ViewpointMismatch
        );
        assert_eq!(outcome.player_set_viewpoint.set_world_object, None);
        assert!(!outcome.player_set_viewpoint.update_visibility_requested);
        assert!(!outcome.player_set_viewpoint.set_seer_requested);
        assert!(outcome.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(
            !map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
    }

    #[test]
    fn dynamic_object_caster_viewpoint_missing_records_do_not_create_or_mutate_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4260501);
        let player_guid = player.guid();
        let missing_dynamic_object_guid = guid(HighGuid::DynamicObject, 4260502);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let missing_dynamic_object =
            map.apply_dynamic_object_caster_viewpoint_like_cpp(missing_dynamic_object_guid, true);

        assert_eq!(
            missing_dynamic_object.status,
            DynamicObjectCasterViewpointStatusLikeCpp::MissingDynamicObject
        );
        assert_eq!(
            missing_dynamic_object.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::MissingTarget
        );
        assert!(!missing_dynamic_object.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert_eq!(map.map_object_count(), 1);

        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260503);
        let dynamic_object_guid = dynamic_object.world().guid();
        let missing_player_guid = guid(HighGuid::Player, 4260504);
        dynamic_object.set_caster_guid(missing_player_guid);
        dynamic_object.bind_to_caster(missing_player_guid);
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let missing_player =
            map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, true);

        assert_eq!(
            missing_player.status,
            DynamicObjectCasterViewpointStatusLikeCpp::CasterNotPlayer
        );
        assert_eq!(
            missing_player.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::MissingPlayer
        );
        assert!(!missing_player.dynamic_object_viewpoint_toggled);
        assert!(
            !map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
        assert_eq!(map.map_object_count(), 2);
    }

    #[test]
    fn dynamic_object_caster_viewpoint_absent_bound_caster_no_mutation_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4260601);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4260602);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(player_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.apply_dynamic_object_caster_viewpoint_like_cpp(dynamic_object_guid, true);

        assert_eq!(
            outcome.status,
            DynamicObjectCasterViewpointStatusLikeCpp::MissingCaster
        );
        assert_eq!(
            outcome.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::MissingPlayer
        );
        assert!(!outcome.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert!(
            !map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_caster_viewpoint_match_cleans_player_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4270101);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4270102);
        let dynamic_object_guid = dynamic_object.world().guid();
        player.set_farsight_object_like_cpp(dynamic_object_guid);
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, false)
            .unwrap();

        let viewpoint = removed.dynamic_object_caster_viewpoint.unwrap();
        assert_eq!(viewpoint.player_guid, player_guid);
        assert_eq!(viewpoint.dynamic_object_guid, dynamic_object_guid);
        assert!(!viewpoint.apply);
        assert_eq!(
            viewpoint.status,
            DynamicObjectCasterViewpointStatusLikeCpp::CasterPlayerResolved
        );
        assert_eq!(
            viewpoint.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::Removed
        );
        assert!(!viewpoint.player_set_viewpoint.update_visibility_requested);
        assert!(viewpoint.player_set_viewpoint.set_seer_requested);
        assert!(viewpoint.dynamic_object_viewpoint_toggled);
        assert!(map.map_object_record(dynamic_object_guid).is_none());
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert!(!removed.object.unwrap().object().is_in_world());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_caster_viewpoint_mismatch_keeps_player_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4270201);
        let player_guid = player.guid();
        let existing_guid = guid(HighGuid::Creature, 4270209);
        player.set_farsight_object_like_cpp(existing_guid);
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4270202);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, true)
            .unwrap();

        let viewpoint = removed.dynamic_object_caster_viewpoint.unwrap();
        assert_eq!(
            viewpoint.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::ViewpointMismatch
        );
        assert!(!viewpoint.player_set_viewpoint.update_visibility_requested);
        assert!(!viewpoint.player_set_viewpoint.set_seer_requested);
        assert!(viewpoint.dynamic_object_viewpoint_toggled);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(map.map_object_record(dynamic_object_guid).is_none());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_not_viewpoint_skips_cleanup_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4270301);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4270302);
        let dynamic_object_guid = dynamic_object.world().guid();
        player.set_farsight_object_like_cpp(dynamic_object_guid);
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, true)
            .unwrap();

        assert_eq!(removed.dynamic_object_caster_viewpoint, None);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_object_guid
        );
        assert!(map.map_object_record(dynamic_object_guid).is_none());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_not_in_world_skips_viewpoint_cleanup_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4270401);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4270402);
        let dynamic_object_guid = dynamic_object.world().guid();
        player.set_farsight_object_like_cpp(dynamic_object_guid);
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        dynamic_object.world_mut().object_mut().remove_from_world();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, true)
            .unwrap();

        assert_eq!(removed.dynamic_object_caster_viewpoint, None);
        assert!(!removed.was_in_world);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_object_guid
        );
        assert!(map.map_object_record(dynamic_object_guid).is_none());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_aura_and_caster_cleanup_like_cpp() {
        let mut map = test_map();
        let caster_guid = guid(HighGuid::Player, 4300101);
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4300102);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(caster_guid);
        dynamic_object.set_aura_bound();
        dynamic_object.bind_to_caster(caster_guid);
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, false)
            .unwrap();

        assert_eq!(removed.dynamic_object_caster_viewpoint, None);
        assert_eq!(
            removed.dynamic_object_remove_cleanup,
            Some(DynamicObjectRemoveCleanupOutcomeLikeCpp {
                had_aura: true,
                removed_aura_pending_delete: true,
                unbound_caster: Some(caster_guid),
            })
        );
        assert!(!removed.object.unwrap().object().is_in_world());
        assert!(map.map_object_record(dynamic_object_guid).is_none());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_without_aura_or_caster_reports_no_cleanup_like_cpp()
    {
        let mut map = test_map();
        let dynamic_object = test_dynamic_object_for_viewpoint(4300201);
        let dynamic_object_guid = dynamic_object.world().guid();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, false)
            .unwrap();

        assert_eq!(removed.dynamic_object_caster_viewpoint, None);
        assert_eq!(
            removed.dynamic_object_remove_cleanup,
            Some(DynamicObjectRemoveCleanupOutcomeLikeCpp {
                had_aura: false,
                removed_aura_pending_delete: false,
                unbound_caster: None,
            })
        );
        assert!(!removed.object.unwrap().object().is_in_world());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_not_in_world_skips_aura_and_caster_cleanup_like_cpp()
    {
        let mut map = test_map();
        let caster_guid = guid(HighGuid::Player, 4300301);
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4300302);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_caster_guid(caster_guid);
        dynamic_object.set_aura_bound();
        dynamic_object.bind_to_caster(caster_guid);
        dynamic_object.world_mut().object_mut().remove_from_world();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, false)
            .unwrap();

        assert_eq!(removed.dynamic_object_caster_viewpoint, None);
        assert_eq!(removed.dynamic_object_remove_cleanup, None);
        assert!(!removed.was_in_world);
        assert!(!removed.object.unwrap().object().is_in_world());
    }

    #[test]
    fn remove_from_map_like_cpp_dynamic_object_viewpoint_aura_and_caster_order_evidence_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4300401);
        let player_guid = player.guid();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4300402);
        let dynamic_object_guid = dynamic_object.world().guid();
        player.set_farsight_object_like_cpp(dynamic_object_guid);
        dynamic_object.set_caster_guid(player_guid);
        dynamic_object.bind_to_caster(player_guid);
        dynamic_object.set_caster_viewpoint();
        dynamic_object.set_aura_bound();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let removed = map
            .remove_from_map_like_cpp(dynamic_object_guid, false)
            .unwrap();

        let viewpoint = removed.dynamic_object_caster_viewpoint.unwrap();
        assert_eq!(
            viewpoint.player_set_viewpoint.status,
            PlayerSetViewpointStatusLikeCpp::Removed
        );
        assert!(viewpoint.dynamic_object_viewpoint_toggled);
        assert_eq!(
            removed.dynamic_object_remove_cleanup,
            Some(DynamicObjectRemoveCleanupOutcomeLikeCpp {
                had_aura: true,
                removed_aura_pending_delete: true,
                unbound_caster: Some(player_guid),
            })
        );
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert!(!removed.object.unwrap().object().is_in_world());
    }

    #[test]
    fn dynamic_object_update_non_aura_decrements_duration_without_queue_like_cpp() {
        let mut map = test_map();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4290101);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_duration(1_000);
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.update_dynamic_object_like_cpp(dynamic_object_guid, 250);

        assert_eq!(outcome.dynamic_object_guid, dynamic_object_guid);
        assert_eq!(outcome.elapsed_ms, 250);
        assert_eq!(outcome.status, DynamicObjectUpdateStatusLikeCpp::Updated);
        assert_eq!(outcome.duration_before_ms, Some(1_000));
        assert_eq!(outcome.duration_after_ms, Some(750));
        assert!(outcome.script_update_would_run);
        assert_eq!(outcome.remove_list, None);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert_eq!(
            map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .duration_ms(),
            750
        );
        assert!(
            !map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .world()
                .object()
                .is_destroyed_object()
        );
    }

    #[test]
    fn dynamic_object_update_non_aura_expiry_queues_remove_list_and_preserves_record_like_cpp() {
        let mut map = test_map();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4290201);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_duration(250);
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.update_dynamic_object_like_cpp(dynamic_object_guid, 250);

        assert_eq!(
            outcome.status,
            DynamicObjectUpdateStatusLikeCpp::ExpiredRemoveQueued
        );
        assert_eq!(outcome.duration_before_ms, Some(250));
        assert_eq!(outcome.duration_after_ms, Some(250));
        assert!(!outcome.script_update_would_run);
        let remove_list = outcome.remove_list.unwrap();
        assert_eq!(remove_list.guid, dynamic_object_guid);
        assert!(remove_list.queued);
        assert!(!remove_list.duplicate);
        assert!(!remove_list.missing_or_stale);
        assert_eq!(remove_list.unsupported_kind, None);
        assert_eq!(remove_list.cleanup_before_delete_count, 1);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        assert!(map.map_object_record(dynamic_object_guid).is_some());
        let dynamic_object = map.get_typed_dynamic_object(dynamic_object_guid).unwrap();
        assert_eq!(dynamic_object.duration_ms(), 250);
        assert!(dynamic_object.world().object().is_destroyed_object());
        assert_eq!(dynamic_object.cleanup_before_delete_count(), 1);
    }

    #[test]
    fn dynamic_object_update_expired_then_remove_list_drain_clears_farsight_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4290301);
        let player_guid = player.guid();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        let create = create_farsight_focus_for_tests(&mut map, player_guid);
        assert_eq!(
            create.status,
            FarsightDynamicObjectCreateStatusLikeCpp::Created
        );
        let dynamic_object_guid = create.dynamic_object_guid.unwrap();
        map.get_typed_dynamic_object_mut(dynamic_object_guid)
            .unwrap()
            .set_duration(1);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_object_guid
        );
        assert!(
            map.get_typed_dynamic_object(dynamic_object_guid)
                .unwrap()
                .is_caster_viewpoint()
        );

        let update = map.update_dynamic_object_like_cpp(dynamic_object_guid, 1);

        assert_eq!(
            update.status,
            DynamicObjectUpdateStatusLikeCpp::ExpiredRemoveQueued
        );
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        assert!(map.map_object_record(dynamic_object_guid).is_some());
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            dynamic_object_guid
        );

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.processed, 1);
        assert_eq!(drain.removed, 1);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert!(map.map_object_record(dynamic_object_guid).is_none());
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
    }

    #[test]
    fn dynamic_object_update_not_in_world_returns_no_mutation_or_queue_like_cpp() {
        let mut map = test_map();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4290401);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_duration(1_000);
        dynamic_object.world_mut().object_mut().remove_from_world();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.update_dynamic_object_like_cpp(dynamic_object_guid, 250);

        assert_eq!(outcome.status, DynamicObjectUpdateStatusLikeCpp::NotInWorld);
        assert_eq!(outcome.duration_before_ms, Some(1_000));
        assert_eq!(outcome.duration_after_ms, Some(1_000));
        assert!(!outcome.script_update_would_run);
        assert_eq!(outcome.remove_list, None);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        let dynamic_object = map.get_typed_dynamic_object(dynamic_object_guid).unwrap();
        assert_eq!(dynamic_object.duration_ms(), 1_000);
        assert!(!dynamic_object.world().object().is_destroyed_object());
    }

    #[test]
    fn dynamic_object_update_aura_bound_not_expired_runs_represented_update_owner_like_cpp() {
        let mut map = test_map();
        let mut dynamic_object = test_dynamic_object_for_viewpoint(4290501);
        let dynamic_object_guid = dynamic_object.world().guid();
        dynamic_object.set_duration(1_000);
        dynamic_object.set_aura_bound();
        map.insert_map_object_record(MapObjectRecord::new_dynamic_object(dynamic_object).unwrap())
            .unwrap();

        let outcome = map.update_dynamic_object_like_cpp(dynamic_object_guid, 250);

        assert_eq!(outcome.status, DynamicObjectUpdateStatusLikeCpp::Updated);
        assert_eq!(outcome.duration_before_ms, Some(1_000));
        assert_eq!(outcome.duration_after_ms, Some(1_000));
        assert_eq!(outcome.aura_update_owner_calls_before, Some(0));
        assert_eq!(outcome.aura_update_owner_calls_after, Some(1));
        assert!(outcome.script_update_would_run);
        assert_eq!(outcome.remove_list, None);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        let dynamic_object = map.get_typed_dynamic_object(dynamic_object_guid).unwrap();
        assert_eq!(dynamic_object.duration_ms(), 1_000);
        assert!(dynamic_object.has_aura());
        assert_eq!(dynamic_object.represented_aura_update_owner_count(), 1);
        assert!(!dynamic_object.world().object().is_destroyed_object());
    }

    #[test]
    fn dynamic_object_update_aura_bound_expired_queues_remove_and_drain_cleans_aura_caster_like_cpp()
     {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4310201);
        let player_guid = player.guid();
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        let create = create_farsight_focus_for_tests(&mut map, player_guid);
        assert_eq!(
            create.status,
            FarsightDynamicObjectCreateStatusLikeCpp::Created
        );
        let dynamic_object_guid = create.dynamic_object_guid.unwrap();
        {
            let dynamic_object = map
                .get_typed_dynamic_object_mut(dynamic_object_guid)
                .unwrap();
            dynamic_object.set_duration(1_000);
            dynamic_object.set_aura_bound();
            dynamic_object.set_aura_removed_like_cpp(true);
        }

        let outcome = map.update_dynamic_object_like_cpp(dynamic_object_guid, 250);

        assert_eq!(
            outcome.status,
            DynamicObjectUpdateStatusLikeCpp::ExpiredRemoveQueued
        );
        assert_eq!(outcome.duration_before_ms, Some(1_000));
        assert_eq!(outcome.duration_after_ms, Some(1_000));
        assert_eq!(outcome.aura_update_owner_calls_before, Some(0));
        assert_eq!(outcome.aura_update_owner_calls_after, Some(0));
        assert!(!outcome.script_update_would_run);
        assert!(outcome.remove_list.unwrap().queued);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        let dynamic_object = map.get_typed_dynamic_object(dynamic_object_guid).unwrap();
        assert!(dynamic_object.has_aura());
        assert_eq!(dynamic_object.bound_caster(), Some(player_guid));
        assert!(dynamic_object.world().object().is_destroyed_object());

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.processed, 1);
        assert_eq!(drain.removed, 1);
        assert_eq!(drain.dynamic_object_remove_aura_cleanup_count, 1);
        assert_eq!(drain.dynamic_object_unbound_caster_count, 1);
        assert!(map.map_object_record(dynamic_object_guid).is_none());
    }

    #[test]
    fn dynamic_object_update_missing_or_non_dynamic_creates_no_dummy_or_queue_like_cpp() {
        let mut map = test_map();
        let missing_guid = guid(HighGuid::DynamicObject, 4290601);
        let creature = world_object_with_counter(HighGuid::Creature, 4290602, 571, 7, true);
        let creature_guid = creature.guid();
        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();
        let untyped_dynamic =
            world_object_with_counter(HighGuid::DynamicObject, 4290603, 571, 7, true);
        let untyped_dynamic_guid = untyped_dynamic.guid();
        map.insert_map_object(AccessorObjectKind::DynamicObject, untyped_dynamic)
            .unwrap();

        let missing = map.update_dynamic_object_like_cpp(missing_guid, 250);
        let non_dynamic = map.update_dynamic_object_like_cpp(creature_guid, 250);
        let untyped = map.update_dynamic_object_like_cpp(untyped_dynamic_guid, 250);

        assert_eq!(
            missing.status,
            DynamicObjectUpdateStatusLikeCpp::MissingDynamicObject
        );
        assert_eq!(
            non_dynamic.status,
            DynamicObjectUpdateStatusLikeCpp::NotDynamicObject
        );
        assert_eq!(
            untyped.status,
            DynamicObjectUpdateStatusLikeCpp::NotDynamicObject
        );
        assert_eq!(missing.remove_list, None);
        assert_eq!(non_dynamic.remove_list, None);
        assert_eq!(untyped.remove_list, None);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 2);
        assert!(map.map_object_record(missing_guid).is_none());
        assert!(map.map_object_record(creature_guid).is_some());
        assert!(map.map_object_record(untyped_dynamic_guid).is_some());
    }

    #[test]
    fn player_set_viewpoint_apply_unit_target_consumes_set_world_object_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4240101);
        let player_guid = player.guid();
        let (target_guid, _cell, _grid) =
            add_loaded_grid_creature_for_switch(&mut map, 424010, 4240102);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome =
            map.apply_player_set_viewpoint_unit_like_cpp(player_guid, target_guid, true, None);

        assert_eq!(outcome.status, PlayerSetViewpointStatusLikeCpp::Applied);
        assert!(outcome.update_visibility_requested);
        assert!(outcome.set_seer_requested);
        assert_eq!(
            outcome.set_world_object,
            Some(SetWorldObjectOutcomeLikeCpp {
                guid: target_guid,
                on: true,
                status: SetWorldObjectStatusLikeCpp::Delegated(
                    AddObjectToSwitchListStatusLikeCpp::Queued
                ),
            })
        );
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            target_guid
        );
        assert!(
            map.get_typed_creature(target_guid)
                .unwrap()
                .unit()
                .subsystems()
                .control
                .shared_vision_guids
                .contains(&player_guid)
        );
        assert_eq!(map.pending_switch_like_cpp(target_guid), Some(true));
    }

    #[test]
    fn player_set_viewpoint_apply_existing_viewpoint_is_no_mutation_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4240201);
        let player_guid = player.guid();
        let existing_guid = guid(HighGuid::Creature, 4240209);
        player.set_farsight_object_like_cpp(existing_guid);
        let (target_guid, _cell, _grid) =
            add_loaded_grid_creature_for_switch(&mut map, 424020, 4240202);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome =
            map.apply_player_set_viewpoint_unit_like_cpp(player_guid, target_guid, true, None);

        assert_eq!(
            outcome.status,
            PlayerSetViewpointStatusLikeCpp::AlreadyHasViewpoint
        );
        assert_eq!(outcome.set_world_object, None);
        assert!(!outcome.update_visibility_requested);
        assert!(!outcome.set_seer_requested);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(
            map.get_typed_creature(target_guid)
                .unwrap()
                .unit()
                .subsystems()
                .control
                .shared_vision_guids
                .is_empty()
        );
        assert_eq!(map.pending_switch_like_cpp(target_guid), None);
    }

    #[test]
    fn player_set_viewpoint_remove_last_viewer_consumes_set_world_object_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4240301);
        let player_guid = player.guid();
        let (target_guid, _cell, _grid) =
            add_loaded_grid_creature_for_switch(&mut map, 424030, 4240302);
        player.set_farsight_object_like_cpp(target_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();
        map.get_typed_creature_mut(target_guid)
            .unwrap()
            .unit_mut()
            .add_player_to_vision_like_cpp(player_guid);
        assert_eq!(map.pending_switch_like_cpp(target_guid), None);

        let outcome =
            map.apply_player_set_viewpoint_unit_like_cpp(player_guid, target_guid, false, None);

        assert_eq!(outcome.status, PlayerSetViewpointStatusLikeCpp::Removed);
        assert!(!outcome.update_visibility_requested);
        assert!(outcome.set_seer_requested);
        assert_eq!(
            outcome.set_world_object,
            Some(SetWorldObjectOutcomeLikeCpp {
                guid: target_guid,
                on: false,
                status: SetWorldObjectStatusLikeCpp::Delegated(
                    AddObjectToSwitchListStatusLikeCpp::Queued
                ),
            })
        );
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            ObjectGuid::EMPTY
        );
        assert!(
            map.get_typed_creature(target_guid)
                .unwrap()
                .unit()
                .subsystems()
                .control
                .shared_vision_guids
                .is_empty()
        );
        assert_eq!(map.pending_switch_like_cpp(target_guid), Some(false));
    }

    #[test]
    fn player_set_viewpoint_remove_mismatch_is_no_mutation_like_cpp() {
        let mut map = test_map();
        let mut player = test_player_for_viewpoint(4240401);
        let player_guid = player.guid();
        let (target_guid, _cell, _grid) =
            add_loaded_grid_creature_for_switch(&mut map, 424040, 4240402);
        let existing_guid = guid(HighGuid::Creature, 4240409);
        player.set_farsight_object_like_cpp(existing_guid);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome =
            map.apply_player_set_viewpoint_unit_like_cpp(player_guid, target_guid, false, None);

        assert_eq!(
            outcome.status,
            PlayerSetViewpointStatusLikeCpp::ViewpointMismatch
        );
        assert_eq!(outcome.set_world_object, None);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            existing_guid
        );
        assert!(
            map.get_typed_creature(target_guid)
                .unwrap()
                .unit()
                .subsystems()
                .control
                .shared_vision_guids
                .is_empty()
        );
        assert_eq!(map.pending_switch_like_cpp(target_guid), None);
    }

    #[test]
    fn player_set_viewpoint_vehicle_base_skips_unit_shared_vision_like_cpp() {
        let mut map = test_map();
        let player = test_player_for_viewpoint(4240501);
        let player_guid = player.guid();
        let (target_guid, _cell, _grid) =
            add_loaded_grid_creature_for_switch(&mut map, 424050, 4240502);
        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        let outcome = map.apply_player_set_viewpoint_unit_like_cpp(
            player_guid,
            target_guid,
            true,
            Some(target_guid),
        );

        assert_eq!(outcome.status, PlayerSetViewpointStatusLikeCpp::Applied);
        assert!(outcome.update_visibility_requested);
        assert!(outcome.set_seer_requested);
        assert_eq!(outcome.set_world_object, None);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .active_data()
                .farsight_object,
            target_guid
        );
        assert!(
            map.get_typed_creature(target_guid)
                .unwrap()
                .unit()
                .subsystems()
                .control
                .shared_vision_guids
                .is_empty()
        );
        assert_eq!(map.pending_switch_like_cpp(target_guid), None);
    }

    #[test]
    fn unit_shared_vision_set_world_object_request_enqueues_like_cpp() {
        let mut map = test_map();
        let spawn_id = 423010;
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, spawn_id, 4230101);

        let outcome = map.apply_unit_shared_vision_set_world_object_request_like_cpp(
            UnitSharedVisionSetWorldObjectRequestLikeCpp {
                unit_guid: guid,
                on: true,
            },
        );

        assert_eq!(outcome.guid, guid);
        assert_eq!(outcome.on, true);
        assert_eq!(
            outcome.status,
            SetWorldObjectStatusLikeCpp::Delegated(AddObjectToSwitchListStatusLikeCpp::Queued)
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 1);
        assert_eq!(map.pending_switch_like_cpp(guid), Some(true));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_executed, 1);
        assert!(map.map_object_record(guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 1);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(!local_cell.grid_objects.creatures.contains(&guid));
        assert!(local_cell.world_objects.creatures.contains(&guid));
        assert!(map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn unit_shared_vision_set_world_object_request_opposite_toggle_cancels_like_cpp() {
        let mut map = test_map();
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, 423020, 4230201);

        assert_eq!(
            map.apply_unit_shared_vision_set_world_object_request_like_cpp(
                UnitSharedVisionSetWorldObjectRequestLikeCpp {
                    unit_guid: guid,
                    on: true,
                },
            )
            .status,
            SetWorldObjectStatusLikeCpp::Delegated(AddObjectToSwitchListStatusLikeCpp::Queued)
        );
        assert_eq!(
            map.apply_unit_shared_vision_set_world_object_request_like_cpp(
                UnitSharedVisionSetWorldObjectRequestLikeCpp {
                    unit_guid: guid,
                    on: false,
                },
            )
            .status,
            SetWorldObjectStatusLikeCpp::Delegated(
                AddObjectToSwitchListStatusLikeCpp::CancelledOppositeToggle
            )
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 0);
        assert_eq!(drain.switch_executed, 0);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(local_cell.grid_objects.creatures.contains(&guid));
        assert!(!local_cell.world_objects.creatures.contains(&guid));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn unit_shared_vision_set_world_object_request_uses_existing_fallbacks_like_cpp() {
        let mut map = test_map();
        let missing_guid = guid(HighGuid::Creature, 4230301);

        let missing = map.apply_unit_shared_vision_set_world_object_request_like_cpp(
            UnitSharedVisionSetWorldObjectRequestLikeCpp {
                unit_guid: missing_guid,
                on: true,
            },
        );

        assert_eq!(missing.status, SetWorldObjectStatusLikeCpp::MissingOrStale);
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 0);

        let gameobject = test_gameobject_for_spawn(423030, 4230302);
        let gameobject_guid = gameobject.world().guid();
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        let non_unit = map.apply_unit_shared_vision_set_world_object_request_like_cpp(
            UnitSharedVisionSetWorldObjectRequestLikeCpp {
                unit_guid: gameobject_guid,
                on: true,
            },
        );

        assert_eq!(
            non_unit.status,
            SetWorldObjectStatusLikeCpp::Delegated(
                AddObjectToSwitchListStatusLikeCpp::IgnoredNonUnit
            )
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 1);
        let drain = map.remove_all_objects_in_remove_list_like_cpp();
        assert_eq!(drain.switch_processed, 0);
        assert_eq!(map.map_object_count(), 1);
    }

    #[test]
    fn set_world_object_like_cpp_creature_in_world_enqueues_and_drain_executes() {
        let mut map = test_map();
        let spawn_id = 421010;
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, spawn_id, 4210101);

        let outcome = map.set_world_object_like_cpp(guid, true);

        assert_eq!(outcome.guid, guid);
        assert_eq!(outcome.on, true);
        assert_eq!(
            outcome.status,
            SetWorldObjectStatusLikeCpp::Delegated(AddObjectToSwitchListStatusLikeCpp::Queued)
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 1);
        assert_eq!(map.pending_switch_like_cpp(guid), Some(true));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
        assert!(
            local_cell_for_switch(&map, grid, cell)
                .grid_objects
                .creatures
                .contains(&guid)
        );

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_executed, 1);
        assert!(map.map_object_record(guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 1);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(!local_cell.grid_objects.creatures.contains(&guid));
        assert!(local_cell.world_objects.creatures.contains(&guid));
        assert!(map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn set_world_object_like_cpp_creature_not_in_world_does_not_enqueue_or_mutate() {
        let mut map = test_map();
        let mut creature = test_creature_for_spawn(421020, 4210201, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let outcome = map.set_world_object_like_cpp(guid, true);

        assert_eq!(outcome.status, SetWorldObjectStatusLikeCpp::NotInWorld);
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 1);
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
        let drain = map.remove_all_objects_in_remove_list_like_cpp();
        assert_eq!(drain.switch_processed, 0);
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn set_world_object_like_cpp_non_unit_in_world_uses_ignored_outcome_without_queue() {
        let mut map = test_map();
        let gameobject = test_gameobject_for_spawn(421030, 4210301);
        let guid = gameobject.world().guid();
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        let outcome = map.set_world_object_like_cpp(guid, true);

        assert_eq!(
            outcome.status,
            SetWorldObjectStatusLikeCpp::Delegated(
                AddObjectToSwitchListStatusLikeCpp::IgnoredNonUnit
            )
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 1);
    }

    #[test]
    fn set_world_object_like_cpp_missing_stale_does_not_create_records() {
        let mut map = test_map();
        let guid = guid(HighGuid::Creature, 4210401);

        let outcome = map.set_world_object_like_cpp(guid, true);

        assert_eq!(outcome.status, SetWorldObjectStatusLikeCpp::MissingOrStale);
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 0);
        let drain = map.remove_all_objects_in_remove_list_like_cpp();
        assert_eq!(drain.switch_processed, 0);
        assert_eq!(map.map_object_count(), 0);
    }

    #[test]
    fn set_world_object_like_cpp_opposite_toggle_cancels_before_drain() {
        let mut map = test_map();
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, 421050, 4210501);

        assert_eq!(
            map.set_world_object_like_cpp(guid, true).status,
            SetWorldObjectStatusLikeCpp::Delegated(AddObjectToSwitchListStatusLikeCpp::Queued)
        );
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
        assert_eq!(
            map.set_world_object_like_cpp(guid, false).status,
            SetWorldObjectStatusLikeCpp::Delegated(
                AddObjectToSwitchListStatusLikeCpp::CancelledOppositeToggle
            )
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 0);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(local_cell.grid_objects.creatures.contains(&guid));
        assert!(!local_cell.world_objects.creatures.contains(&guid));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn switch_list_on_moves_creature_from_grid_to_world_container_like_cpp() {
        let mut map = test_map();
        let spawn_id = 420010;
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, spawn_id, 4200101);
        assert!(
            local_cell_for_switch(&map, grid, cell)
                .grid_objects
                .creatures
                .contains(&guid)
        );

        let queued = map.add_object_to_switch_list_like_cpp(guid, true);
        assert_eq!(queued.status, AddObjectToSwitchListStatusLikeCpp::Queued);
        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_executed, 1);
        assert!(map.map_object_record(guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 1);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(!local_cell.grid_objects.creatures.contains(&guid));
        assert!(local_cell.world_objects.creatures.contains(&guid));
        assert!(map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn switch_list_off_moves_temp_creature_from_world_to_grid_container_like_cpp() {
        let mut map = test_map();
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, 420020, 4200201);
        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        assert_eq!(
            map.remove_all_objects_in_remove_list_like_cpp()
                .switch_executed,
            1
        );
        assert!(map.get_typed_creature(guid).unwrap().is_temp_world_object());

        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, false).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_executed, 1);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(local_cell.grid_objects.creatures.contains(&guid));
        assert!(!local_cell.world_objects.creatures.contains(&guid));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn switch_list_opposite_toggle_before_drain_cancels_like_cpp() {
        let mut map = test_map();
        let (guid, cell, grid) = add_loaded_grid_creature_for_switch(&mut map, 420030, 4200301);

        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, false).status,
            AddObjectToSwitchListStatusLikeCpp::CancelledOppositeToggle
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 0);
        let local_cell = local_cell_for_switch(&map, grid, cell);
        assert!(local_cell.grid_objects.creatures.contains(&guid));
        assert!(!local_cell.world_objects.creatures.contains(&guid));
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn switch_list_duplicate_same_direction_reports_abort_outcome_like_cpp() {
        let mut map = test_map();
        let (guid, _, _) = add_loaded_grid_creature_for_switch(&mut map, 420040, 4200401);

        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::DuplicateSameDirectionAbort
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 1);
        assert_eq!(map.pending_switch_like_cpp(guid), Some(true));
    }

    #[test]
    fn switch_list_non_unit_gameobject_enqueue_is_ignored_like_cpp() {
        let mut map = test_map();
        let gameobject = test_gameobject_for_spawn(420050, 4200501);
        let guid = gameobject.world().guid();
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        let outcome = map.add_object_to_switch_list_like_cpp(guid, true);

        assert_eq!(
            outcome.status,
            AddObjectToSwitchListStatusLikeCpp::IgnoredNonUnit
        );
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
    }

    #[test]
    fn switch_list_stale_guid_drain_does_not_create_dummy_like_cpp() {
        let mut map = test_map();
        let guid = guid(HighGuid::Creature, 4200601);
        map.enqueue_object_to_switch_for_test(guid, true);

        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_missing_or_stale, 1);
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.map_object_count(), 0);
    }

    #[test]
    fn switch_list_unloaded_grid_does_not_create_grid_and_drains_like_cpp() {
        let mut map = test_map();
        let creature = test_creature_for_spawn(420070, 4200701, true);
        let guid = creature.guid();
        let cell = Cell::from_world(1.0, 2.0);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();
        assert!(map.get_ngrid(grid).is_none());

        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_invalid_or_unloaded_grid, 1);
        assert!(map.get_ngrid(grid).is_none());
        assert!(!map.get_typed_creature(guid).unwrap().is_temp_world_object());
    }

    #[test]
    fn remove_list_drain_runs_switch_list_before_physical_remove_like_cpp() {
        let mut map = test_map();
        let spawn_id = 420080;
        let (guid, _, _) = add_loaded_grid_creature_for_switch(&mut map, spawn_id, 4200801);

        assert_eq!(
            map.add_object_to_switch_list_like_cpp(guid, true).status,
            AddObjectToSwitchListStatusLikeCpp::Queued
        );
        assert!(map.add_object_to_remove_list_like_cpp(guid).queued);
        let drain = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(drain.switch_processed, 1);
        assert_eq!(drain.switch_executed, 1);
        assert_eq!(drain.processed, 1);
        assert_eq!(drain.removed, 1);
        assert_eq!(map.objects_to_switch_count_like_cpp(), 0);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert!(map.map_object_record(guid).is_none());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 0);
    }

    #[test]
    fn despawn_all_by_spawn_id_queues_and_defers_physical_removal_like_cpp() {
        let mut map = test_map();
        let spawn_id = 41905;
        let mut creature = test_creature_for_spawn(spawn_id, 4190501, true);
        let guid = creature.guid();
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .remove_from_world();
        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let outcome = map.despawn_all_by_spawn_id_like_cpp(SpawnObjectType::Creature, spawn_id);

        assert_eq!(outcome.queued, 1);
        assert_eq!(outcome.removed, 0);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 1);
        assert!(map.map_object_record(guid).is_some());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 1);

        let drain = map.remove_all_objects_in_remove_list_like_cpp();
        assert_eq!(drain.removed, 1);
        assert!(map.map_object_record(guid).is_none());
        assert_eq!(map.creature_spawn_id_store_count_like_cpp(spawn_id), 0);
    }

    #[test]
    fn remove_list_drain_gameobject_owner_removes_linked_trap_like_cpp() {
        let mut map = test_map();
        let mut owner = game_object_with_counter(4190601, 571, 7, false);
        let trap = game_object_with_counter(4190602, 571, 7, false);
        let owner_guid = owner.world().guid();
        let trap_guid = trap.world().guid();
        owner.set_linked_trap_like_cpp(trap_guid);

        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_game_object(trap).unwrap())
            .unwrap();
        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_game_object(owner).unwrap())
            .unwrap();
        assert!(map.add_object_to_remove_list_like_cpp(owner_guid).queued);

        let outcome = map.remove_all_objects_in_remove_list_like_cpp();

        assert_eq!(outcome.removed, 1);
        assert_eq!(map.objects_to_remove_count_like_cpp(), 0);
        assert!(map.map_object_record(owner_guid).is_none());
        assert!(map.map_object_record(trap_guid).is_none());
    }

    #[test]
    fn linked_trap_remove_owner_removes_trap_map_local_and_leaves_unrelated_objects() {
        let mut map = test_map();
        let mut owner = game_object_with_counter(10, 571, 7, false);
        let trap = game_object_with_counter(11, 571, 7, false);
        let unrelated = game_object_with_counter(12, 571, 7, false);
        let owner_guid = owner.world().guid();
        let trap_guid = trap.world().guid();
        let unrelated_guid = unrelated.world().guid();
        owner.set_linked_trap_like_cpp(trap_guid);

        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_game_object(trap).unwrap())
            .unwrap();
        map.add_map_object_record_to_map_like_cpp(
            MapObjectRecord::new_game_object(unrelated).unwrap(),
        )
        .unwrap();
        map.add_map_object_record_to_map_like_cpp(MapObjectRecord::new_game_object(owner).unwrap())
            .unwrap();

        let removed = map.remove_from_map_like_cpp(owner_guid, true).unwrap();

        assert_eq!(removed.guid, owner_guid);
        assert!(map.map_object_record(owner_guid).is_none());
        assert!(map.map_object_record(trap_guid).is_none());
        assert!(map.map_object_record(unrelated_guid).is_some());
    }

    #[test]
    fn remove_from_map_like_cpp_can_delete_object_and_reports_missing_guid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let removed = map.remove_from_map_like_cpp(guid, true).unwrap();
        assert!(removed.delete_from_world);
        assert!(removed.object.is_none());
        assert_eq!(map.map_object_count(), 0);

        assert_eq!(
            map.remove_from_map_like_cpp(guid, false),
            Err(RemoveFromMapError::ObjectNotFound { guid })
        );
    }

    #[test]
    fn relocate_map_object_like_cpp_same_cell_only_updates_position() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(2.0, 3.0, 4.0))
            .unwrap();

        assert!(outcome.relocated);
        assert!(!outcome.moved_between_cells);
        assert_eq!(outcome.old_cell, added.cell);
        assert_eq!(outcome.new_cell, added.cell);
        assert_eq!(
            map.get_creature(guid).unwrap().position(),
            Position::xyz(2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn relocate_map_object_like_cpp_moves_between_cells_in_same_grid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        let new_position = Position::xyz(90.0, 20.0, 5.0);

        let outcome = map
            .relocate_map_object_like_cpp(guid, new_position)
            .unwrap();

        assert!(outcome.relocated);
        assert!(outcome.moved_between_cells);
        assert_eq!(outcome.old_grid, outcome.new_grid);
        assert_eq!(map.get_creature(guid).unwrap().position(), new_position);
        assert_eq!(
            map.get_creature(guid).unwrap().current_cell(),
            Some((
                outcome.new_cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.new_cell.y_coord % MAX_NUMBER_OF_CELLS
            ))
        );

        let old_grid = map.get_ngrid(added.grid).unwrap();
        let old_cell = old_grid
            .get_grid_type(
                added.cell.x_coord % MAX_NUMBER_OF_CELLS,
                added.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(!old_cell.grid_objects.creatures.contains(&guid));

        let new_cell = old_grid
            .get_grid_type(
                outcome.new_cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.new_cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(new_cell.grid_objects.creatures.contains(&guid));
    }

    #[test]
    fn relocate_map_object_like_cpp_blocks_normal_object_to_unloaded_grid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(700.0, 20.0, 5.0))
            .unwrap();

        assert!(!outcome.relocated);
        assert!(outcome.blocked_by_unloaded_grid);
        assert_eq!(
            map.get_creature(guid).unwrap().position(),
            Position::xyz(1.0, 2.0, 3.0)
        );
        let old_grid = map.get_ngrid(added.grid).unwrap();
        let old_cell = old_grid
            .get_grid_type(
                added.cell.x_coord % MAX_NUMBER_OF_CELLS,
                added.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(old_cell.grid_objects.creatures.contains(&guid));
    }

    #[test]
    fn relocate_map_object_like_cpp_active_object_loads_new_grid_and_moves() {
        let mut map = test_map();
        let mut object = WorldObject::new(true, TypeId::DynamicObject, TypeMask::DYNAMIC_OBJECT);
        object.object_mut().create(guid(HighGuid::DynamicObject, 3));
        object.set_map(571, 7).unwrap();
        object.relocate(Position::xyz(20.0, 20.0, 3.0));
        object.set_active(true);
        let guid = object.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::DynamicObject, object)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(700.0, 20.0, 5.0))
            .unwrap();

        assert!(outcome.relocated);
        assert!(outcome.moved_between_cells);
        assert_ne!(outcome.old_grid, outcome.new_grid);
        assert!(outcome.loaded_grid);
        assert!(map.is_grid_loaded(outcome.new_grid));
        assert_eq!(
            map.get_dynamic_object(guid).unwrap().position(),
            Position::xyz(700.0, 20.0, 5.0)
        );
    }

    #[test]
    fn nearby_cell_guids_like_cpp_visits_existing_cells_without_loading_grids() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object(HighGuid::GameObject, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();

        let nearby = map.nearby_cell_guids_like_cpp(0.0, 0.0, 70.0);

        assert_eq!(nearby.visited_cells, 16);
        assert_eq!(nearby.len(), 2);
        assert!(nearby.grid.creatures.contains(&creature_guid));
        assert!(nearby.grid.gameobjects.contains(&gameobject_guid));
        assert_eq!(map.terrain().loads.len(), 1);

        let far = map.nearby_cell_guids_like_cpp(700.0, 700.0, 0.0);
        assert_eq!(far.visited_cells, 1);
        assert!(far.is_empty());
        assert_eq!(map.terrain().loads.len(), 1);
    }

    #[test]
    fn nearby_cell_guids_like_cpp_rejects_invalid_center_without_visits() {
        let map = test_map();
        let nearby = map.nearby_cell_guids_like_cpp(f32::NAN, 0.0, 100.0);

        assert_eq!(nearby.visited_cells, 0);
        assert!(nearby.is_empty());
    }

    #[test]
    fn visit_nearby_cells_of_like_cpp_marks_cells_once_and_collects_objects() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let viewpoint = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let viewpoint_guid = viewpoint.guid();
        let creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let creature_guid = creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, viewpoint)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let plan = map.visit_nearby_cells_of_like_cpp([
            NearbyCellVisitCenter {
                guid: player_guid,
                activation_radius: 0.0,
            },
            NearbyCellVisitCenter {
                guid: viewpoint_guid,
                activation_radius: 0.0,
            },
        ]);

        assert_eq!(plan.marked_cells.len(), 1);
        assert_eq!(plan.nearby.visited_cells, 1);
        assert!(plan.nearby.world.players.contains(&player_guid));
        assert!(plan.nearby.grid.creatures.contains(&viewpoint_guid));
        assert!(plan.nearby.grid.creatures.contains(&creature_guid));
    }

    #[test]
    fn visit_nearby_cells_of_like_cpp_skips_missing_and_invalid_centers() {
        let mut map = test_map();
        let mut invalid_center = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let invalid_guid = invalid_center.guid();
        invalid_center.relocate(Position::xyz(f32::NAN, 0.0, 0.0));
        map.insert_map_object(AccessorObjectKind::Player, invalid_center)
            .unwrap();
        let missing = guid(HighGuid::Player, 9);

        let plan = map.visit_nearby_cells_of_like_cpp([
            NearbyCellVisitCenter {
                guid: invalid_guid,
                activation_radius: 100.0,
            },
            NearbyCellVisitCenter {
                guid: missing,
                activation_radius: 100.0,
            },
        ]);

        assert!(plan.marked_cells.is_empty());
        assert!(plan.nearby.is_empty());
        assert_eq!(plan.skipped_invalid_position_centers, vec![invalid_guid]);
        assert_eq!(plan.skipped_missing_centers, vec![missing]);
    }

    #[test]
    fn player_relocation_visibility_plan_matches_cpp_visible_and_out_of_range_shape() {
        let player = guid(HighGuid::Player, 1);
        let other_player = guid(HighGuid::Player, 2);
        let old_player = guid(HighGuid::Player, 3);
        let creature = guid(HighGuid::Creature, 4);
        let old_creature = guid(HighGuid::Creature, 5);
        let gameobject = guid(HighGuid::GameObject, 6);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player);
        nearby.world.players.insert(other_player);
        nearby.grid.creatures.insert(creature);
        nearby.grid.gameobjects.insert(gameobject);

        let plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
            player,
            [other_player, old_player, old_creature],
            &nearby,
            true,
        );

        assert!(plan.visible_guids.contains(&player));
        assert!(plan.visible_guids.contains(&other_player));
        assert!(plan.visible_guids.contains(&creature));
        assert!(plan.visible_guids.contains(&gameobject));
        assert_eq!(
            plan.out_of_range_guids,
            HashSet::from([old_player, old_creature])
        );
        assert_eq!(
            plan.reciprocal_player_updates,
            HashSet::from([other_player, old_player])
        );
        assert_eq!(plan.ai_relocation_checks, vec![(creature, player)]);
    }

    #[test]
    fn player_relocation_visibility_plan_skips_ai_when_not_relocated_for_ai() {
        let player = guid(HighGuid::Player, 1);
        let creature = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature);

        let plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
            player,
            [creature],
            &nearby,
            false,
        );

        assert!(plan.out_of_range_guids.is_empty());
        assert!(plan.ai_relocation_checks.is_empty());
    }

    #[test]
    fn creature_relocation_visibility_plan_matches_cpp_player_and_creature_visits() {
        let source = guid(HighGuid::Creature, 1);
        let player_visible = guid(HighGuid::Player, 2);
        let player_needs_notify = guid(HighGuid::Player, 3);
        let creature_normal = guid(HighGuid::Creature, 4);
        let creature_needs_notify = guid(HighGuid::Creature, 5);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player_visible);
        nearby.world.players.insert(player_needs_notify);
        nearby.grid.creatures.insert(source);
        nearby.grid.creatures.insert(creature_normal);
        nearby.grid.creatures.insert(creature_needs_notify);

        let plan = CreatureRelocationVisibilityPlan::from_nearby_like_cpp(
            source,
            true,
            &nearby,
            [player_needs_notify],
            [creature_needs_notify],
        );

        assert_eq!(
            plan.player_visibility_updates,
            HashSet::from([player_visible])
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, player_visible))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, player_needs_notify))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, creature_normal))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(creature_normal, source))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, creature_needs_notify))
        );
        assert!(
            !plan
                .ai_relocation_checks
                .contains(&(creature_needs_notify, source))
        );
    }

    #[test]
    fn creature_relocation_visibility_plan_skips_creature_visits_when_source_dead() {
        let source = guid(HighGuid::Creature, 1);
        let player = guid(HighGuid::Player, 2);
        let creature = guid(HighGuid::Creature, 3);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player);
        nearby.grid.creatures.insert(creature);

        let plan =
            CreatureRelocationVisibilityPlan::from_nearby_like_cpp(source, false, &nearby, [], []);

        assert_eq!(plan.player_visibility_updates, HashSet::from([player]));
        assert_eq!(plan.ai_relocation_checks, vec![(source, player)]);
    }

    #[test]
    fn delayed_unit_relocation_plan_selects_only_units_needing_notify_like_cpp() {
        let creature_notify = guid(HighGuid::Creature, 1);
        let creature_normal = guid(HighGuid::Creature, 2);
        let world_creature_notify = guid(HighGuid::Creature, 3);
        let player_notify = guid(HighGuid::Player, 4);
        let player_normal = guid(HighGuid::Player, 5);
        let player_invalid_viewpoint = guid(HighGuid::Player, 6);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature_notify);
        nearby.grid.creatures.insert(creature_normal);
        nearby.world.creatures.insert(world_creature_notify);
        nearby.world.players.insert(player_notify);
        nearby.world.players.insert(player_normal);
        nearby.world.players.insert(player_invalid_viewpoint);

        let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(
            &nearby,
            [creature_notify, world_creature_notify],
            [player_notify, player_invalid_viewpoint],
            [player_invalid_viewpoint],
        );

        assert_eq!(
            plan.creature_relocations,
            vec![creature_notify, world_creature_notify]
        );
        assert_eq!(plan.player_relocations, vec![player_notify]);
        assert_eq!(
            plan.skipped_invalid_viewpoints,
            vec![player_invalid_viewpoint]
        );
    }

    #[test]
    fn delayed_unit_relocation_plan_deduplicates_creatures_from_world_and_grid_sets() {
        let creature = guid(HighGuid::Creature, 1);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature);
        nearby.world.creatures.insert(creature);

        let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(&nearby, [creature], [], []);

        assert_eq!(plan.creature_relocations, vec![creature]);
        assert!(plan.player_relocations.is_empty());
    }

    #[test]
    fn delayed_unit_relocation_for_cells_like_cpp_reads_notify_flags_from_map_store() {
        let mut map = test_map();
        let creature_notify = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_notify_guid = creature_notify.guid();
        let creature_normal = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let player_notify = world_object_with_counter(HighGuid::Player, 3, 571, 7, false);
        let player_notify_guid = player_notify.guid();
        let player_invalid = world_object_with_counter(HighGuid::Player, 4, 571, 7, false);
        let player_invalid_guid = player_invalid.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature_notify)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature_normal)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_notify)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_invalid)
            .unwrap();
        for guid in [
            creature_notify_guid,
            player_notify_guid,
            player_invalid_guid,
        ] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], [player_invalid_guid]);

        assert_eq!(plan.cell_plans.len(), 1);
        assert_eq!(plan.cell_plans[0].cell_coord, cell);
        assert_eq!(
            plan.cell_plans[0].plan.creature_relocations,
            vec![creature_notify_guid]
        );
        assert_eq!(
            plan.cell_plans[0].plan.player_relocations,
            vec![player_notify_guid]
        );
        assert_eq!(
            plan.cell_plans[0].plan.skipped_invalid_viewpoints,
            vec![player_invalid_guid]
        );
    }

    #[test]
    fn process_relocation_notifies_like_cpp_selects_delayed_before_resetting_flags() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let player = world_object_with_counter(HighGuid::Player, 2, 571, 7, false);
        let player_guid = player.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap()
            .cell;
        let active_cell = Cell::from_cell_coord(cell);
        let active_grid = GridCoord::new(active_cell.grid_x(), active_cell.grid_y());
        map.get_ngrid_mut(active_grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();
        for guid in [creature_guid, player_guid] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let outcome = map.process_relocation_notifies_like_cpp(
            [cell],
            1000,
            1000,
            std::iter::empty::<ObjectGuid>(),
        );

        assert_eq!(outcome.process_plan.delayed_relocation_cells, vec![cell]);
        assert_eq!(outcome.process_plan.reset_notify_cells, vec![cell]);
        assert_eq!(outcome.process_plan.reset_timer_grids, vec![active_grid]);
        assert_eq!(outcome.delayed_plan.cell_plans.len(), 1);
        assert_eq!(
            outcome.delayed_plan.cell_plans[0].plan.creature_relocations,
            vec![creature_guid]
        );
        assert_eq!(
            outcome.delayed_plan.cell_plans[0].plan.player_relocations,
            vec![player_guid]
        );
        assert_eq!(outcome.reset_outcome.reset_player_guids, vec![player_guid]);
        assert_eq!(
            outcome.reset_outcome.reset_creature_guids,
            vec![creature_guid]
        );
        assert!(
            !map.map_object(creature_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            !map.map_object(player_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
    }

    #[test]
    fn delayed_unit_relocation_visibility_plans_use_cpp_max_visibility_visits() {
        let mut map = test_map();
        let source_creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let source_creature_guid = source_creature.guid();
        let other_creature = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let other_creature_guid = other_creature.guid();
        let notified_creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let notified_creature_guid = notified_creature.guid();
        let player_notify = world_object_with_counter(HighGuid::Player, 4, 571, 7, false);
        let player_notify_guid = player_notify.guid();
        let player_normal = world_object_with_counter(HighGuid::Player, 5, 571, 7, false);
        let player_normal_guid = player_normal.guid();
        let old_player = guid(HighGuid::Player, 6);
        let old_creature = guid(HighGuid::Creature, 7);

        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, source_creature)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, other_creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, notified_creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_notify)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_normal)
            .unwrap();
        for guid in [
            source_creature_guid,
            notified_creature_guid,
            player_notify_guid,
        ] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let delayed_plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], []);
        let plans = map.delayed_unit_relocation_visibility_plans_like_cpp(
            &delayed_plan,
            [DelayedPlayerRelocationContext {
                player_guid: player_notify_guid,
                viewpoint_guid: player_notify_guid,
                previous_client_guids: vec![old_player, old_creature],
                relocated_for_ai: true,
            }],
            [DelayedCreatureRelocationContext {
                creature_guid: source_creature_guid,
                source_creature_alive: true,
            }],
        );

        assert_eq!(plans.creature_plans.len(), 2);
        let source_plan = plans
            .creature_plans
            .iter()
            .find(|plan| plan.creature_guid == source_creature_guid)
            .unwrap();
        assert_eq!(source_plan.cell_coord, cell);
        assert!(
            source_plan
                .visibility_plan
                .player_visibility_updates
                .contains(&player_normal_guid)
        );
        assert!(
            !source_plan
                .visibility_plan
                .player_visibility_updates
                .contains(&player_notify_guid)
        );
        assert!(
            source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(source_creature_guid, other_creature_guid))
        );
        assert!(
            source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(other_creature_guid, source_creature_guid))
        );
        assert!(
            !source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(notified_creature_guid, source_creature_guid))
        );

        assert_eq!(plans.player_plans.len(), 1);
        let player_plan = &plans.player_plans[0];
        assert_eq!(player_plan.player_guid, player_notify_guid);
        assert_eq!(player_plan.viewpoint_guid, player_notify_guid);
        assert!(
            player_plan
                .visibility_plan
                .out_of_range_guids
                .contains(&old_player)
        );
        assert!(
            player_plan
                .visibility_plan
                .out_of_range_guids
                .contains(&old_creature)
        );
        assert!(
            player_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(source_creature_guid, player_notify_guid))
        );
    }

    #[test]
    fn delayed_unit_relocation_visibility_plans_report_missing_player_contexts_like_cpp_gap() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap()
            .cell;
        map.map_objects
            .get_mut(&player_guid)
            .unwrap()
            .object_mut()
            .object_mut()
            .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);

        let delayed_plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], []);
        let plans = map.delayed_unit_relocation_visibility_plans_like_cpp(
            &delayed_plan,
            std::iter::empty::<DelayedPlayerRelocationContext>(),
            std::iter::empty::<DelayedCreatureRelocationContext>(),
        );

        assert!(plans.player_plans.is_empty());
        assert_eq!(plans.missing_player_contexts, vec![player_guid]);
    }

    #[test]
    fn ai_relocation_plan_for_player_checks_nearby_creatures_against_source_unit() {
        let player = guid(HighGuid::Player, 1);
        let world_creature = guid(HighGuid::Creature, 2);
        let grid_creature = guid(HighGuid::Creature, 3);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(world_creature);
        nearby.grid.creatures.insert(grid_creature);

        let plan = AIRelocationPlan::from_nearby_like_cpp(player, false, &nearby);

        assert_eq!(
            plan.creature_unit_checks,
            vec![(world_creature, player), (grid_creature, player)]
        );
    }

    #[test]
    fn ai_relocation_plan_for_creature_checks_both_cpp_directions() {
        let source = guid(HighGuid::Creature, 1);
        let other = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(source);
        nearby.grid.creatures.insert(other);

        let plan = AIRelocationPlan::from_nearby_like_cpp(source, true, &nearby);

        assert_eq!(
            plan.creature_unit_checks,
            vec![(other, source), (source, other)]
        );
    }

    #[test]
    fn ai_relocation_plan_deduplicates_world_grid_creatures_and_skips_self_worker_noop() {
        let source = guid(HighGuid::Creature, 1);
        let other = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(source);
        nearby.grid.creatures.insert(source);
        nearby.world.creatures.insert(other);
        nearby.grid.creatures.insert(other);

        let plan = AIRelocationPlan::from_nearby_like_cpp(source, false, &nearby);

        assert_eq!(plan.creature_unit_checks, vec![(other, source)]);
    }

    #[test]
    fn object_update_plan_for_nearby_like_cpp_selects_in_world_updateable_objects_only() {
        let mut map = test_map();
        let player = world_object(HighGuid::Player, 571, 7, true);
        let player_guid = player.guid();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();
        let gameobject = world_object(HighGuid::GameObject, 571, 7, true);
        let gameobject_guid = gameobject.guid();
        let dynamic_not_in_world = world_object(HighGuid::DynamicObject, 571, 7, false);
        let dynamic_guid = dynamic_not_in_world.guid();
        let missing_conversation = guid(HighGuid::Conversation, 9);
        map.insert_map_object(AccessorObjectKind::Player, player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::DynamicObject, dynamic_not_in_world)
            .unwrap();

        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player_guid);
        nearby.grid.creatures.insert(creature_guid);
        nearby.grid.gameobjects.insert(gameobject_guid);
        nearby.grid.dynamic_objects.insert(dynamic_guid);
        nearby.grid.conversations.insert(missing_conversation);

        let plan = map.object_update_plan_for_nearby_like_cpp(&nearby, 42);

        assert_eq!(plan.diff_ms, 42);
        assert_eq!(plan.update_guids, vec![creature_guid, gameobject_guid]);
    }

    #[test]
    fn object_update_plan_for_nearby_like_cpp_deduplicates_world_and_grid_objects() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();
        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(creature_guid);
        nearby.grid.creatures.insert(creature_guid);

        let plan = map.object_update_plan_for_nearby_like_cpp(&nearby, 1);

        assert_eq!(plan.update_guids, vec![creature_guid]);
    }

    #[test]
    fn map_update_visit_plan_like_cpp_filters_sources_by_cpp_in_world_guards() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, true);
        let player_guid = player.guid();
        let offline_player = world_object_with_counter(HighGuid::Player, 2, 571, 7, false);
        let offline_player_guid = offline_player.guid();
        let viewpoint = world_object_with_counter(HighGuid::Creature, 3, 571, 7, true);
        let viewpoint_guid = viewpoint.guid();
        let far_combat = world_object_with_counter(HighGuid::Creature, 4, 571, 7, true);
        let far_combat_guid = far_combat.guid();
        let offline_aura = world_object_with_counter(HighGuid::Creature, 5, 571, 7, false);
        let offline_aura_guid = offline_aura.guid();
        let active_non_player = world_object_with_counter(HighGuid::DynamicObject, 6, 571, 7, true);
        let active_non_player_guid = active_non_player.guid();
        let transport = world_object_with_counter(HighGuid::Transport, 7, 571, 7, false);
        let transport_guid = transport.guid();

        map.insert_map_object(AccessorObjectKind::Player, player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Player, offline_player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, viewpoint)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, far_combat)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, offline_aura)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::DynamicObject, active_non_player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Transport, transport)
            .unwrap();

        let plan = map.map_update_visit_plan_like_cpp(
            [
                MapUpdatePlayerSources {
                    player_guid,
                    viewpoint_guid: Some(viewpoint_guid),
                    far_combat_unit_guids: vec![far_combat_guid],
                    far_aura_caster_guids: vec![offline_aura_guid],
                    far_summon_guids: vec![],
                },
                MapUpdatePlayerSources {
                    player_guid: offline_player_guid,
                    viewpoint_guid: Some(far_combat_guid),
                    far_combat_unit_guids: vec![viewpoint_guid],
                    far_aura_caster_guids: vec![],
                    far_summon_guids: vec![],
                },
            ],
            [active_non_player_guid, offline_aura_guid],
            [transport_guid],
            50,
        );

        assert_eq!(plan.diff_ms, 50);
        assert_eq!(plan.session_update_players, vec![player_guid]);
        assert_eq!(plan.player_update_guids, vec![player_guid]);
        assert_eq!(plan.transport_update_guids, vec![transport_guid]);
        assert_eq!(
            plan.nearby_visit_centers
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([
                player_guid,
                viewpoint_guid,
                far_combat_guid,
                active_non_player_guid
            ])
        );
        assert!(plan.process_relocation_notifies);
    }

    #[test]
    fn map_update_visit_plan_like_cpp_processes_relocation_notifies_only_for_players_or_active_non_players()
     {
        let mut map = test_map();
        let transport = world_object_with_counter(HighGuid::Transport, 7, 571, 7, false);
        let transport_guid = transport.guid();
        map.insert_map_object(AccessorObjectKind::Transport, transport)
            .unwrap();

        let plan = map.map_update_visit_plan_like_cpp(
            std::iter::empty::<MapUpdatePlayerSources>(),
            std::iter::empty::<ObjectGuid>(),
            [transport_guid],
            1,
        );

        assert_eq!(plan.transport_update_guids, vec![transport_guid]);
        assert!(!plan.process_relocation_notifies);
    }

    #[test]
    fn process_relocation_notifies_plan_like_cpp_waits_for_active_grid_timer() {
        let mut map = test_map();
        let grid = GridCoord::new(2, 3);
        map.ensure_grid_created(grid);
        map.get_ngrid_mut(grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        let marked = CellCoord::new(2 * MAX_NUMBER_OF_CELLS, 3 * MAX_NUMBER_OF_CELLS);

        let plan = map.process_relocation_notifies_plan_like_cpp([marked], 999, 1000);

        assert!(plan.delayed_relocation_cells.is_empty());
        assert!(plan.reset_notify_cells.is_empty());
        assert!(plan.reset_timer_grids.is_empty());
    }

    #[test]
    fn process_relocation_notifies_plan_like_cpp_visits_marked_cells_and_resets_timer() {
        let mut map = test_map();
        let active_grid = GridCoord::new(2, 3);
        let idle_grid = GridCoord::new(4, 5);
        map.ensure_grid_created(active_grid);
        map.ensure_grid_created(idle_grid);
        map.get_ngrid_mut(active_grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        map.get_ngrid_mut(idle_grid)
            .unwrap()
            .set_state(GridStateKind::Idle);
        let marked_a = CellCoord::new(2 * MAX_NUMBER_OF_CELLS, 3 * MAX_NUMBER_OF_CELLS);
        let marked_b = CellCoord::new(2 * MAX_NUMBER_OF_CELLS + 1, 3 * MAX_NUMBER_OF_CELLS);
        let marked_idle = CellCoord::new(4 * MAX_NUMBER_OF_CELLS, 5 * MAX_NUMBER_OF_CELLS);

        let plan = map.process_relocation_notifies_plan_like_cpp(
            [marked_b, marked_idle, marked_a],
            1000,
            1000,
        );

        assert_eq!(plan.diff_ms, 1000);
        assert_eq!(plan.delayed_relocation_cells, vec![marked_a, marked_b]);
        assert_eq!(plan.reset_notify_cells, vec![marked_a, marked_b]);
        assert_eq!(plan.reset_timer_grids, vec![active_grid]);
        assert_eq!(
            map.get_ngrid(active_grid)
                .unwrap()
                .info()
                .relocation_timer()
                .expire_time_ms(),
            1000
        );
    }

    #[test]
    fn reset_notify_flags_for_cells_like_cpp_resets_only_players_and_creatures() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let creature = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 3, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        let player_cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        for guid in [player_guid, creature_guid, gameobject_guid] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let outcome = map.reset_notify_flags_for_cells_like_cpp([player_cell]);

        assert_eq!(outcome.reset_player_guids, vec![player_guid]);
        assert_eq!(outcome.reset_creature_guids, vec![creature_guid]);
        assert!(outcome.missing_guids.is_empty());
        assert!(
            !map.map_object(player_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            !map.map_object(creature_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            map.map_object(gameobject_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_relocates_active_entries_and_resets_inactive() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 2, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(5.0, 5.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: gameobject_guid,
                kind: AccessorObjectKind::GameObject,
                move_state: MapObjectCellMoveState::Inactive,
                new_position: Position::xyz(6.0, 6.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
        ]);

        assert_eq!(plan.relocated, vec![creature_guid]);
        assert_eq!(plan.reset_inactive_or_none, vec![gameobject_guid]);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().position(),
            Position::xyz(5.0, 5.0, 3.0)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_uses_respawn_or_removal_fallbacks() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 2, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        let pet = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let pet_guid = pet.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, pet)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: Some(Position::xyz(2.0, 2.0, 3.0)),
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: gameobject_guid,
                kind: AccessorObjectKind::GameObject,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: pet_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: true,
            },
        ]);

        assert_eq!(plan.respawn_relocated, vec![creature_guid]);
        assert_eq!(plan.remove_from_world, vec![gameobject_guid]);
        assert_eq!(plan.pet_removed, vec![pet_guid]);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().position(),
            Position::xyz(2.0, 2.0, 3.0)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_blocks_dynamic_and_skips_not_in_world() {
        let mut map = test_map();
        let dynamic = world_object_with_counter(HighGuid::DynamicObject, 1, 571, 7, false);
        let dynamic_guid = dynamic.guid();
        let area_trigger = world_object_with_counter(HighGuid::AreaTrigger, 2, 571, 7, false);
        let area_trigger_guid = area_trigger.guid();
        let offline_creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let offline_creature_guid = offline_creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::DynamicObject, dynamic)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::AreaTrigger, area_trigger)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, offline_creature)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: dynamic_guid,
                kind: AccessorObjectKind::DynamicObject,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: area_trigger_guid,
                kind: AccessorObjectKind::AreaTrigger,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: offline_creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(2.0, 2.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
        ]);

        assert_eq!(
            plan.blocked_unloaded_grid,
            vec![dynamic_guid, area_trigger_guid]
        );
        assert_eq!(plan.skipped_not_in_world, vec![offline_creature_guid]);
    }

    #[test]
    fn ensure_grid_created_sets_idle_grid_and_loads_reversed_terrain_coords() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);

        assert!(map.ensure_grid_created(coord));
        assert!(!map.ensure_grid_created(coord));

        let grid = map.get_ngrid(coord).unwrap();
        assert_eq!(grid.grid_id(), 2 * MAX_NUMBER_OF_GRIDS + 3);
        assert_eq!(grid.state(), GridStateKind::Idle);
        assert!(!grid.grid_object_data_loaded());
        assert_eq!(map.terrain().loads, vec![(61, 60)]);
    }

    #[test]
    fn ensure_grid_loaded_marks_loaded_before_object_loader_hook() {
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(2, 3));

        assert!(map.ensure_grid_loaded(&cell));
        assert!(!map.ensure_grid_loaded(&cell));

        assert!(map.is_grid_loaded(GridCoord::new(2, 3)));
        assert_eq!(map.lifecycle().loads, 1);
    }

    #[test]
    fn active_object_loading_sets_grid_active_and_short_expiry() {
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(2, 3));

        assert!(map.ensure_grid_loaded_for_active_object(&cell, ActiveObjectKind::NonPlayer));

        let grid = map.get_ngrid(GridCoord::new(2, 3)).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        assert_eq!(grid.info().time_tracker().remaining_ms(), 100);
        assert!(map.active_objects_near_grid(grid));
    }

    #[test]
    fn player_phase_loading_invokes_personal_phase_tracker_before_activation() {
        let mut store = crate::spawn::SpawnStore::new();
        let spawn = crate::spawn::SpawnData {
            object_type: crate::spawn::SpawnObjectType::Creature,
            spawn_id: 100,
            map_id: 571,
            db_data: true,
            spawn_group: crate::spawn::SpawnGroupTemplateData::default_group(),
            id: 42,
            spawn_point: crate::spawn::SpawnPosition::new(0.0, 0.0, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id: 9,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        };
        store.add_object_spawn(&spawn, |phase_id| phase_id == 9);
        let corpses = crate::object_grid_loader::CorpseCellStore::new();
        let mut loader =
            crate::object_grid_loader::ObjectGridLoader::new(&store, &corpses, 571, 1, 1, 1);
        let owner = ObjectGuid::create_player(1, 100);
        let phase_shift = crate::personal_phase::PhaseShift::new(
            Some(owner),
            vec![crate::personal_phase::PhaseRef::new(9, true)],
        );
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(32, 32));

        assert!(map.ensure_grid_loaded_for_player_phase(&cell, &phase_shift, &mut loader));

        let grid = map.get_ngrid(GridCoord::new(32, 32)).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        assert_eq!(
            grid.get_grid_type(0, 0)
                .unwrap()
                .grid_objects
                .creatures
                .len(),
            1
        );
        assert_eq!(map.personal_phase_tracker().tracker_count(), 1);
    }

    #[test]
    fn unload_grid_applies_guid_lifecycle_actions_to_canonical_map_objects_like_cpp() {
        let mut map = guid_unload_test_map();
        let coord = GridCoord::new(2, 3);
        let cell = cell_from_grid_center(coord);
        assert!(map.ensure_grid_loaded(&cell));

        let creature = test_creature_for_spawn(4181, 4181, true);
        let creature_guid = creature.unit().world().guid();
        let gameobject = test_gameobject_for_spawn(4182, 4182);
        let gameobject_guid = gameobject.world().guid();
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        let grid_cell = map
            .get_ngrid_mut(coord)
            .unwrap()
            .get_grid_type_mut(0, 0)
            .unwrap();
        grid_cell.grid_objects.creatures.insert(creature_guid);
        grid_cell.grid_objects.gameobjects.insert(gameobject_guid);

        assert!(map.unload_grid_at(coord, true));

        assert!(map.get_ngrid(coord).is_none());
        assert_eq!(map.terrain().unloads, vec![(61, 60)]);
        assert_eq!(map.map_object_count(), 2);

        let creature = map
            .map_object_record(creature_guid)
            .unwrap()
            .creature()
            .unwrap();
        assert!(creature.unit().world().object().is_destroyed_object());
        assert_eq!(creature.cleanup_before_delete_count(), 2);
        assert!(creature.grid_unload_delete_requested());
        assert!(!creature.grid_unload_respawn_relocation_requested());

        let gameobject = map
            .map_object_record(gameobject_guid)
            .unwrap()
            .game_object()
            .unwrap();
        assert!(gameobject.world().object().is_destroyed_object());
        assert_eq!(gameobject.cleanup_before_delete_count(), 2);
        assert!(gameobject.grid_unload_delete_requested());
        assert!(!gameobject.grid_unload_respawn_relocation_requested());
    }

    #[test]
    fn unload_grid_purges_personal_phase_tracker_before_unloader_like_cpp() {
        let mut store = crate::spawn::SpawnStore::new();
        let spawn = crate::spawn::SpawnData {
            object_type: crate::spawn::SpawnObjectType::Creature,
            spawn_id: 4183,
            map_id: 571,
            db_data: true,
            spawn_group: crate::spawn::SpawnGroupTemplateData::default_group(),
            id: 42,
            spawn_point: crate::spawn::SpawnPosition::new(0.0, 0.0, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id: 9,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        };
        store.add_object_spawn(&spawn, |phase_id| phase_id == 9);
        let corpses = crate::object_grid_loader::CorpseCellStore::new();
        let mut loader =
            crate::object_grid_loader::ObjectGridLoader::new(&store, &corpses, 571, 1, 1, 1);
        let owner = ObjectGuid::create_player(1, 4183);
        let phase_shift = crate::personal_phase::PhaseShift::new(
            Some(owner),
            vec![crate::personal_phase::PhaseRef::new(9, true)],
        );
        let mut map = test_map();
        let coord = GridCoord::new(32, 32);
        let cell = cell_from_grid_center(coord);

        assert!(map.ensure_grid_loaded_for_player_phase(&cell, &phase_shift, &mut loader));
        assert_eq!(map.personal_phase_tracker().tracker_count(), 1);

        assert!(map.unload_grid_at(coord, true));

        assert_eq!(map.personal_phase_tracker().tracker_count(), 0);
        assert!(map.get_ngrid(coord).is_none());
    }

    #[test]
    fn active_to_idle_stop_drains_guid_lifecycle_stoper_actions_into_creature_like_cpp() {
        let mut map = guid_unload_test_map();
        let coord = GridCoord::new(2, 3);
        assert!(map.ensure_grid_loaded(&cell_from_grid_center(coord)));

        let dynamic_object_guid = guid(HighGuid::DynamicObject, 4184);
        let area_trigger_guid = guid(HighGuid::AreaTrigger, 4185);
        let victim_guid = guid(HighGuid::Creature, 4186);
        let mut creature = test_creature_for_spawn(4184, 4184, true);
        let creature_guid = creature.unit().world().guid();
        creature.register_dynamic_object(dynamic_object_guid);
        creature.register_area_trigger(area_trigger_guid);
        creature.unit_mut().set_attacking(Some(victim_guid));
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        let grid = map.get_ngrid_mut(coord).unwrap();
        grid.get_grid_type_mut(0, 0)
            .unwrap()
            .grid_objects
            .creatures
            .insert(creature_guid);
        grid.set_state(GridStateKind::Active);

        assert!(!map.update_grid_state_at(coord, 1001));

        let grid = map.get_ngrid(coord).unwrap();
        assert_eq!(grid.state(), GridStateKind::Idle);
        let creature = map
            .map_object_record(creature_guid)
            .unwrap()
            .creature()
            .unwrap();
        assert!(!creature.is_in_combat());
        assert!(creature.dynamic_objects().is_empty());
        assert_eq!(
            creature.removed_dynamic_objects_from_grid_unload(),
            &[dynamic_object_guid]
        );
        assert!(creature.area_triggers().is_empty());
        assert_eq!(
            creature.removed_area_triggers_from_grid_unload(),
            &[area_trigger_guid]
        );
    }

    #[test]
    fn unload_grid_refuses_world_creatures_and_active_neighbors_unless_forced() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        let cell = cell_from_grid_center(coord);
        map.ensure_grid_loaded(&cell);
        map.get_ngrid_mut(coord)
            .unwrap()
            .get_grid_type_mut(0, 0)
            .unwrap()
            .world_objects
            .creatures
            .insert(ObjectGuid::new(1, 1));

        assert!(!map.unload_grid_at(coord, false));
        assert!(map.is_grid_loaded(coord));

        assert!(map.unload_grid_at(coord, true));
        assert!(map.get_ngrid(coord).is_none());
        assert_eq!(map.lifecycle().evacuates, 0);
        assert_eq!(map.lifecycle().cleans, 1);
        assert_eq!(map.lifecycle().unloads, 1);
        assert_eq!(map.terrain().unloads, vec![(61, 60)]);
    }

    #[test]
    fn update_grid_state_at_removes_grid_when_removal_unloads_successfully() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        map.ensure_grid_loaded(&cell_from_grid_center(coord));
        map.get_ngrid_mut(coord)
            .unwrap()
            .set_state(GridStateKind::Removal);

        assert!(map.update_grid_state_at(coord, 1001));

        assert!(map.get_ngrid(coord).is_none());
        assert_eq!(map.lifecycle().evacuates, 1);
        assert_eq!(map.lifecycle().cleans, 1);
        assert_eq!(map.lifecycle().unloads, 1);
    }

    #[test]
    fn active_objects_near_grid_matches_cpp_cell_range_expansion() {
        let mut map = test_map();
        let coord = GridCoord::new(10, 10);
        map.ensure_grid_created(coord);
        let grid = map.get_ngrid(coord).unwrap();
        assert!(!map.active_objects_near_grid(grid));

        map.mark_active_cell(CellCoord::new(79, 80));
        let grid = map.get_ngrid(coord).unwrap();
        assert!(map.active_objects_near_grid(grid));

        map.unmark_active_cell(CellCoord::new(79, 80));
        map.mark_active_cell(CellCoord::new(1, 1));
        let grid = map.get_ngrid(coord).unwrap();
        assert!(!map.active_objects_near_grid(grid));
    }

    #[test]
    fn grid_id_loaded_uses_cpp_public_grid_id_decomposition() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        map.ensure_grid_loaded(&cell_from_grid_center(coord));

        assert!(is_grid_id_loaded(&map, 3 * MAX_NUMBER_OF_GRIDS + 2));
    }
}
