//! Pure C++-shaped pool data helpers.
//!
//! Source of truth: TrinityCore `PoolMgr.h` / `PoolMgr.cpp` pool data and
//! `PoolGroup<T>` helpers. This module intentionally does not implement live
//! `PoolMgr` runtime, RNG, DB loading, entity creation, or live side effects.
//! It does implement deterministic C++-shaped `SpawnPool`/`DespawnPool` plans
//! over the caller-provided map-owned `SpawnedPoolDataLikeCpp`; plans record
//! future live side effects without performing DB writes, AddToMap/RemoveFromMap,
//! packet fanout, or entity creation/destruction.

use crate::map::SpawnedPoolDataLikeCpp;
use crate::spawn::{SpawnId, SpawnObjectType};
use std::collections::{HashMap, HashSet};

/// C++ `PoolTemplateData { uint32 MaxLimit; int32 MapId; }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolTemplateDataLikeCpp {
    pub max_limit: u32,
    pub map_id: i32,
}

impl PoolTemplateDataLikeCpp {
    #[must_use]
    pub const fn new(max_limit: u32, map_id: i32) -> Self {
        Self { max_limit, map_id }
    }
}

/// C++ `PoolObject { uint64 guid; float chance; }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolObjectLikeCpp {
    pub guid: u64,
    pub chance: f32,
}

impl PoolObjectLikeCpp {
    #[must_use]
    pub const fn new(guid: u64, chance: f32) -> Self {
        Self { guid, chance }
    }
}

/// Tag for the C++ template parameter of `PoolGroup<T>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolMemberKindLikeCpp {
    Creature,
    GameObject,
    Pool,
}

/// Evidence returned by `PoolGroup<Pool>::RemoveOneRelation` representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PoolRelationRemovalLikeCpp {
    pub removed_explicit: bool,
    pub removed_equal: bool,
}

/// Planned side-effect placeholder for C++ `Spawn1Object`/`ReSpawn1Object`/
/// `DespawnObject` calls.
///
/// These actions intentionally do not create entities, write DB rows, call
/// `AddToMap`, recurse through live `PoolMgr::SpawnPool`, or fan out packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSpawnObjectActionLikeCpp {
    SpawnOne {
        kind: PoolMemberKindLikeCpp,
        guid: u64,
    },
    RespawnOne {
        kind: PoolMemberKindLikeCpp,
        guid: u64,
    },
    DespawnOne {
        kind: PoolMemberKindLikeCpp,
        guid: u64,
    },
    RemoveRespawnTime {
        kind: PoolMemberKindLikeCpp,
        guid: u64,
    },
}

/// Deterministic result of represented C++ `PoolGroup<T>::DespawnObject`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoolDespawnObjectPlanLikeCpp {
    pub actions: Vec<PoolSpawnObjectActionLikeCpp>,
    pub requested_guid: u64,
    pub always_delete_respawn_time: bool,
    pub despawned: Vec<u64>,
    pub removed_respawn_times: Vec<(PoolMemberKindLikeCpp, u64)>,
    pub child_pool_plans: Vec<PoolDespawnPoolPlanLikeCpp>,
}

/// One represented specialization call of C++ `PoolMgr::DespawnPool<T>`.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolTypedDespawnPlanLikeCpp {
    pub kind: PoolMemberKindLikeCpp,
    pub pool_id: u32,
    pub requested_guid: u64,
    pub always_delete_respawn_time: bool,
    pub object_plan: Option<PoolDespawnObjectPlanLikeCpp>,
    pub skip_reason: Option<PoolMgrPlanSkipReasonLikeCpp>,
}

/// Deterministic result of represented C++ `PoolMgr::DespawnPool(...)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoolDespawnPoolPlanLikeCpp {
    pub pool_id: u32,
    pub always_delete_respawn_time: bool,
    pub subplans: Vec<PoolTypedDespawnPlanLikeCpp>,
}

/// Deterministic result of represented C++ `PoolGroup<T>::SpawnObject`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoolSpawnObjectPlanLikeCpp {
    pub actions: Vec<PoolSpawnObjectActionLikeCpp>,
    pub selected: Vec<PoolObjectLikeCpp>,
    pub despawned_trigger: Option<u64>,
    pub respawned_trigger: bool,
}

/// Typed no-op/blocked reason recorded by represented `PoolMgr::SpawnPool` planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMgrPlanSkipReasonLikeCpp {
    MissingGroup,
    EmptyGroup,
}

/// Typed error for deterministic `PoolMgr` planning helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMgrPlanErrorLikeCpp {
    MissingTemplate {
        pool_id: u32,
    },
    WrongGroupKind {
        expected: PoolMemberKindLikeCpp,
        actual: PoolMemberKindLikeCpp,
    },
    UnsupportedSpawnType {
        spawn_type: SpawnObjectType,
    },
    ChildPoolIdOverflow {
        child_pool_id: u64,
    },
    /// Rust guard for invalid pool dependency data that Trinity normally avoids
    /// during pool loading by removing circular relations.
    ChildPoolCycle {
        pool_id: u32,
    },
}

/// One represented specialization call of C++ `PoolMgr::SpawnPool<T>`.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolTypedSpawnPlanLikeCpp {
    pub kind: PoolMemberKindLikeCpp,
    pub pool_id: u32,
    pub trigger_from: u64,
    pub max_limit: Option<u32>,
    pub object_plan: Option<PoolSpawnObjectPlanLikeCpp>,
    pub skip_reason: Option<PoolMgrPlanSkipReasonLikeCpp>,
}

/// Deterministic result of represented C++ `PoolMgr::SpawnPool(...)` orchestration.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoolSpawnPoolPlanLikeCpp {
    pub pool_id: u32,
    pub subplans: Vec<PoolTypedSpawnPlanLikeCpp>,
}

/// C++-shaped pure `PoolMgr` planner.
///
/// This struct owns only template/group/index data needed to plan C++
/// `PoolMgr::SpawnPool`/`UpdatePool` branch order. Runtime spawned state stays
/// exclusively in the caller-provided map-owned `SpawnedPoolDataLikeCpp`; live
/// entity creation, DB writes, recursive live PoolMgr execution, scripts, map
/// fanout, and server networking are intentionally represented only as action
/// records returned by the existing `PoolGroupLikeCpp::spawn_object_plan_like_cpp`.
#[derive(Debug, Clone, Default)]
pub struct PoolMgrLikeCpp {
    pub templates: HashMap<u32, PoolTemplateDataLikeCpp>,
    pub creature_groups: HashMap<u32, PoolGroupLikeCpp>,
    pub gameobject_groups: HashMap<u32, PoolGroupLikeCpp>,
    pub pool_groups: HashMap<u32, PoolGroupLikeCpp>,
    pub creature_spawn_to_pool: HashMap<SpawnId, u32>,
    pub gameobject_spawn_to_pool: HashMap<SpawnId, u32>,
    pub child_pool_to_parent: HashMap<u32, u32>,
    /// C++ `mAutoSpawnPoolsPerMap`; key intentionally stays signed to preserve
    /// `PoolTemplateData::MapId == -1` during honest load/report validation.
    pub auto_spawn_pools_per_map: HashMap<i32, Vec<u32>>,
}

impl PoolMgrLikeCpp {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_template_like_cpp(&mut self, pool_id: u32, template: PoolTemplateDataLikeCpp) {
        self.templates.insert(pool_id, template);
    }

    pub fn insert_or_replace_group_like_cpp(
        &mut self,
        kind: PoolMemberKindLikeCpp,
        pool_id: u32,
        mut group: PoolGroupLikeCpp,
    ) -> Result<Option<PoolGroupLikeCpp>, PoolMgrPlanErrorLikeCpp> {
        if group.member_kind() != kind {
            return Err(PoolMgrPlanErrorLikeCpp::WrongGroupKind {
                expected: kind,
                actual: group.member_kind(),
            });
        }
        group.set_pool_id_like_cpp(pool_id);
        let replaced = match kind {
            PoolMemberKindLikeCpp::Creature => self.creature_groups.insert(pool_id, group),
            PoolMemberKindLikeCpp::GameObject => self.gameobject_groups.insert(pool_id, group),
            PoolMemberKindLikeCpp::Pool => self.pool_groups.insert(pool_id, group),
        };
        Ok(replaced)
    }

    pub fn register_spawn_pool_relation_like_cpp(
        &mut self,
        kind: PoolMemberKindLikeCpp,
        spawn_id: SpawnId,
        pool_id: u32,
    ) -> Result<Option<u32>, PoolMgrPlanErrorLikeCpp> {
        match kind {
            PoolMemberKindLikeCpp::Creature => {
                Ok(self.creature_spawn_to_pool.insert(spawn_id, pool_id))
            }
            PoolMemberKindLikeCpp::GameObject => {
                Ok(self.gameobject_spawn_to_pool.insert(spawn_id, pool_id))
            }
            PoolMemberKindLikeCpp::Pool => {
                self.register_child_pool_relation_like_cpp(spawn_id, pool_id)
            }
        }
    }

    pub fn register_child_pool_relation_like_cpp(
        &mut self,
        child_pool_id: u64,
        parent_pool_id: u32,
    ) -> Result<Option<u32>, PoolMgrPlanErrorLikeCpp> {
        let child_pool_id = u32::try_from(child_pool_id)
            .map_err(|_| PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow { child_pool_id })?;
        Ok(self
            .child_pool_to_parent
            .insert(child_pool_id, parent_pool_id))
    }

    pub fn add_auto_spawn_pool_like_cpp(&mut self, map_id: i32, pool_id: u32) {
        self.auto_spawn_pools_per_map
            .entry(map_id)
            .or_default()
            .push(pool_id);
    }

    #[must_use]
    pub fn auto_spawn_pools_for_map_like_cpp(&self, map_id: i32) -> &[u32] {
        self.auto_spawn_pools_per_map
            .get(&map_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn auto_spawn_pools_per_map_like_cpp(&self) -> &HashMap<i32, Vec<u32>> {
        &self.auto_spawn_pools_per_map
    }

    pub fn remove_child_pool_relation_like_cpp(
        &mut self,
        child_pool_id: u32,
        parent_pool_id: u32,
    ) -> PoolRelationRemovalLikeCpp {
        let removal = self
            .pool_groups
            .get_mut(&parent_pool_id)
            .map(|group| group.remove_one_relation_like_cpp(child_pool_id))
            .unwrap_or_default();
        self.child_pool_to_parent.remove(&child_pool_id);
        removal
    }

    #[must_use]
    pub fn top_level_auto_spawn_candidate_like_cpp(&self, pool_id: u32) -> Option<i32> {
        if self.is_empty_like_cpp(pool_id) || !self.check_pool_like_cpp(pool_id) {
            return None;
        }
        if self.child_pool_to_parent.contains_key(&pool_id) {
            return None;
        }
        self.templates.get(&pool_id).map(|template| template.map_id)
    }

    pub fn is_part_of_a_pool_like_cpp(
        &self,
        spawn_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Result<u32, PoolMgrPlanErrorLikeCpp> {
        match spawn_type {
            SpawnObjectType::Creature => {
                Ok(*self.creature_spawn_to_pool.get(&spawn_id).unwrap_or(&0))
            }
            SpawnObjectType::GameObject => {
                Ok(*self.gameobject_spawn_to_pool.get(&spawn_id).unwrap_or(&0))
            }
            SpawnObjectType::AreaTrigger => Ok(0),
        }
    }

    pub fn spawn_pool_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        mut explicit_roll_for: impl FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        mut choose_equal: impl FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    ) -> Result<PoolSpawnPoolPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let mut plan = PoolSpawnPoolPlanLikeCpp {
            pool_id,
            subplans: Vec::new(),
        };
        for kind in [
            PoolMemberKindLikeCpp::Pool,
            PoolMemberKindLikeCpp::GameObject,
            PoolMemberKindLikeCpp::Creature,
        ] {
            plan.subplans.push(self.spawn_typed_pool_plan_like_cpp(
                kind,
                spawns,
                pool_id,
                0,
                &mut explicit_roll_for,
                &mut choose_equal,
            )?);
        }
        Ok(plan)
    }

    pub fn spawn_typed_pool_plan_like_cpp(
        &self,
        kind: PoolMemberKindLikeCpp,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        trigger_from: u64,
        mut explicit_roll_for: impl FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        choose_equal: impl FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    ) -> Result<PoolTypedSpawnPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let Some(group) = self.group_like_cpp(kind, pool_id) else {
            return Ok(PoolTypedSpawnPlanLikeCpp {
                kind,
                pool_id,
                trigger_from,
                max_limit: None,
                object_plan: None,
                skip_reason: Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup),
            });
        };
        if group.is_empty_like_cpp() {
            return Ok(PoolTypedSpawnPlanLikeCpp {
                kind,
                pool_id,
                trigger_from,
                max_limit: None,
                object_plan: None,
                skip_reason: Some(PoolMgrPlanSkipReasonLikeCpp::EmptyGroup),
            });
        }
        self.ensure_no_child_pool_overflow_like_cpp(group)?;
        let template = self
            .templates
            .get(&pool_id)
            .ok_or(PoolMgrPlanErrorLikeCpp::MissingTemplate { pool_id })?;
        let object_plan = group.spawn_object_plan_like_cpp(
            spawns,
            template.max_limit,
            trigger_from,
            || explicit_roll_for(kind, pool_id),
            choose_equal,
        );
        Ok(PoolTypedSpawnPlanLikeCpp {
            kind,
            pool_id,
            trigger_from,
            max_limit: Some(template.max_limit),
            object_plan: Some(object_plan),
            skip_reason: None,
        })
    }

    pub fn update_pool_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        spawn_type: SpawnObjectType,
        spawn_id: SpawnId,
        mut explicit_roll_for: impl FnMut(PoolMemberKindLikeCpp, u32) -> f32,
        choose_equal: impl FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    ) -> Result<PoolTypedSpawnPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        if spawn_type == SpawnObjectType::AreaTrigger {
            return Err(PoolMgrPlanErrorLikeCpp::UnsupportedSpawnType { spawn_type });
        }
        if let Some(&mother_pool_id) = self.child_pool_to_parent.get(&pool_id) {
            return self.spawn_typed_pool_plan_like_cpp(
                PoolMemberKindLikeCpp::Pool,
                spawns,
                mother_pool_id,
                u64::from(pool_id),
                &mut explicit_roll_for,
                choose_equal,
            );
        }
        let kind = match spawn_type {
            SpawnObjectType::Creature => PoolMemberKindLikeCpp::Creature,
            SpawnObjectType::GameObject => PoolMemberKindLikeCpp::GameObject,
            SpawnObjectType::AreaTrigger => unreachable!("AreaTrigger returned above"),
        };
        self.spawn_typed_pool_plan_like_cpp(
            kind,
            spawns,
            pool_id,
            spawn_id,
            explicit_roll_for,
            choose_equal,
        )
    }

    pub fn despawn_pool_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        always_delete_respawn_time: bool,
    ) -> Result<PoolDespawnPoolPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let mut visiting = HashSet::new();
        self.despawn_pool_plan_with_visited_like_cpp(
            spawns,
            pool_id,
            always_delete_respawn_time,
            &mut visiting,
        )
    }

    fn despawn_pool_plan_with_visited_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        always_delete_respawn_time: bool,
        visiting: &mut HashSet<u32>,
    ) -> Result<PoolDespawnPoolPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        if !visiting.insert(pool_id) {
            return Err(PoolMgrPlanErrorLikeCpp::ChildPoolCycle { pool_id });
        }

        let mut plan = PoolDespawnPoolPlanLikeCpp {
            pool_id,
            always_delete_respawn_time,
            subplans: Vec::new(),
        };
        for kind in [
            PoolMemberKindLikeCpp::Creature,
            PoolMemberKindLikeCpp::GameObject,
            PoolMemberKindLikeCpp::Pool,
        ] {
            plan.subplans
                .push(self.despawn_typed_pool_plan_with_visited_like_cpp(
                    kind,
                    spawns,
                    pool_id,
                    0,
                    always_delete_respawn_time,
                    visiting,
                )?);
        }
        visiting.remove(&pool_id);
        Ok(plan)
    }

    pub fn despawn_typed_pool_plan_like_cpp(
        &self,
        kind: PoolMemberKindLikeCpp,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        requested_guid: u64,
        always_delete_respawn_time: bool,
    ) -> Result<PoolTypedDespawnPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let mut visiting = HashSet::new();
        visiting.insert(pool_id);
        self.despawn_typed_pool_plan_with_visited_like_cpp(
            kind,
            spawns,
            pool_id,
            requested_guid,
            always_delete_respawn_time,
            &mut visiting,
        )
    }

    fn despawn_typed_pool_plan_with_visited_like_cpp(
        &self,
        kind: PoolMemberKindLikeCpp,
        spawns: &mut SpawnedPoolDataLikeCpp,
        pool_id: u32,
        requested_guid: u64,
        always_delete_respawn_time: bool,
        visiting: &mut HashSet<u32>,
    ) -> Result<PoolTypedDespawnPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let Some(group) = self.group_like_cpp(kind, pool_id) else {
            return Ok(PoolTypedDespawnPlanLikeCpp {
                kind,
                pool_id,
                requested_guid,
                always_delete_respawn_time,
                object_plan: None,
                skip_reason: Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup),
            });
        };
        if group.is_empty_like_cpp() {
            return Ok(PoolTypedDespawnPlanLikeCpp {
                kind,
                pool_id,
                requested_guid,
                always_delete_respawn_time,
                object_plan: None,
                skip_reason: Some(PoolMgrPlanSkipReasonLikeCpp::EmptyGroup),
            });
        }
        let object_plan = group.despawn_object_plan_like_cpp(
            spawns,
            requested_guid,
            always_delete_respawn_time,
            |spawns, child_pool_id, always_delete_respawn_time| {
                self.despawn_pool_plan_with_visited_like_cpp(
                    spawns,
                    child_pool_id,
                    always_delete_respawn_time,
                    visiting,
                )
            },
        )?;
        Ok(PoolTypedDespawnPlanLikeCpp {
            kind,
            pool_id,
            requested_guid,
            always_delete_respawn_time,
            object_plan: Some(object_plan),
            skip_reason: None,
        })
    }

    #[must_use]
    pub fn is_empty_like_cpp(&self, pool_id: u32) -> bool {
        let mut visiting = HashSet::new();
        self.is_empty_with_visited_like_cpp(pool_id, &mut visiting)
    }

    #[must_use]
    pub fn check_pool_like_cpp(&self, pool_id: u32) -> bool {
        for kind in [
            PoolMemberKindLikeCpp::GameObject,
            PoolMemberKindLikeCpp::Creature,
            PoolMemberKindLikeCpp::Pool,
        ] {
            if let Some(group) = self.group_like_cpp(kind, pool_id) {
                if !group.check_pool_like_cpp() {
                    return false;
                }
            }
        }
        true
    }

    fn group_like_cpp(
        &self,
        kind: PoolMemberKindLikeCpp,
        pool_id: u32,
    ) -> Option<&PoolGroupLikeCpp> {
        match kind {
            PoolMemberKindLikeCpp::Creature => self.creature_groups.get(&pool_id),
            PoolMemberKindLikeCpp::GameObject => self.gameobject_groups.get(&pool_id),
            PoolMemberKindLikeCpp::Pool => self.pool_groups.get(&pool_id),
        }
    }

    fn ensure_no_child_pool_overflow_like_cpp(
        &self,
        group: &PoolGroupLikeCpp,
    ) -> Result<(), PoolMgrPlanErrorLikeCpp> {
        if group.member_kind() != PoolMemberKindLikeCpp::Pool {
            return Ok(());
        }
        for child in group
            .explicitly_chanced_like_cpp()
            .iter()
            .chain(group.equal_chanced_like_cpp().iter())
        {
            if u32::try_from(child.guid).is_err() {
                return Err(PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow {
                    child_pool_id: child.guid,
                });
            }
        }
        Ok(())
    }

    fn is_empty_with_visited_like_cpp(&self, pool_id: u32, visiting: &mut HashSet<u32>) -> bool {
        if !visiting.insert(pool_id) {
            return false;
        }
        for kind in [
            PoolMemberKindLikeCpp::GameObject,
            PoolMemberKindLikeCpp::Creature,
            PoolMemberKindLikeCpp::Pool,
        ] {
            if let Some(group) = self.group_like_cpp(kind, pool_id) {
                let empty = if kind == PoolMemberKindLikeCpp::Pool {
                    group.is_empty_deep_check_like_cpp(|child_pool_id| {
                        self.is_empty_with_visited_like_cpp(child_pool_id, visiting)
                    })
                } else {
                    group.is_empty_deep_check_like_cpp(|_| true)
                };
                if !empty {
                    visiting.remove(&pool_id);
                    return false;
                }
            }
        }
        visiting.remove(&pool_id);
        true
    }
}

/// C++-shaped `PoolGroup<T>` buckets and pure helpers.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolGroupLikeCpp {
    pool_id: u32,
    member_kind: PoolMemberKindLikeCpp,
    explicitly_chanced: Vec<PoolObjectLikeCpp>,
    equal_chanced: Vec<PoolObjectLikeCpp>,
}

impl PoolGroupLikeCpp {
    /// C++ constructor initializes `poolId` to zero.
    #[must_use]
    pub const fn new(member_kind: PoolMemberKindLikeCpp) -> Self {
        Self {
            pool_id: 0,
            member_kind,
            explicitly_chanced: Vec::new(),
            equal_chanced: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_pool_id(member_kind: PoolMemberKindLikeCpp, pool_id: u32) -> Self {
        Self {
            pool_id,
            member_kind,
            explicitly_chanced: Vec::new(),
            equal_chanced: Vec::new(),
        }
    }

    pub const fn set_pool_id_like_cpp(&mut self, pool_id: u32) {
        self.pool_id = pool_id;
    }

    #[must_use]
    pub const fn pool_id_like_cpp(&self) -> u32 {
        self.pool_id
    }

    #[must_use]
    pub const fn member_kind(&self) -> PoolMemberKindLikeCpp {
        self.member_kind
    }

    #[must_use]
    pub fn explicitly_chanced_like_cpp(&self) -> &[PoolObjectLikeCpp] {
        &self.explicitly_chanced
    }

    #[must_use]
    pub fn equal_chanced_like_cpp(&self) -> &[PoolObjectLikeCpp] {
        &self.equal_chanced
    }

    /// C++ `isEmpty()`: both chance buckets are empty.
    #[must_use]
    pub fn is_empty_like_cpp(&self) -> bool {
        self.explicitly_chanced.is_empty() && self.equal_chanced.is_empty()
    }

    /// C++ `isEmptyDeepCheck()`.
    ///
    /// For Creature/GameObject groups this is the normal `isEmpty()` helper and
    /// the child-pool closure is not called. For Pool-of-Pools groups this
    /// represents `sPoolMgr->IsEmpty(child_guid)`. Child GUIDs above `u32::MAX`
    /// are treated as non-empty rather than truncated silently.
    pub fn is_empty_deep_check_like_cpp(
        &self,
        mut is_child_pool_empty: impl FnMut(u32) -> bool,
    ) -> bool {
        if self.member_kind != PoolMemberKindLikeCpp::Pool {
            return self.is_empty_like_cpp();
        }

        for child in self
            .explicitly_chanced
            .iter()
            .chain(self.equal_chanced.iter())
        {
            let Ok(child_pool_id) = u32::try_from(child.guid) else {
                return false;
            };
            if !is_child_pool_empty(child_pool_id) {
                return false;
            }
        }

        true
    }

    /// C++ `AddEntry`: non-zero chance with maxentries one is explicit;
    /// everything else is equal-chanced.
    pub fn add_entry_like_cpp(&mut self, pool_object: PoolObjectLikeCpp, maxentries: u32) {
        if pool_object.chance != 0.0 && maxentries == 1 {
            self.explicitly_chanced.push(pool_object);
        } else {
            self.equal_chanced.push(pool_object);
        }
    }

    /// C++ `CheckPool`: validate explicit total only when equal-chanced is empty.
    #[must_use]
    pub fn check_pool_like_cpp(&self) -> bool {
        if self.equal_chanced.is_empty() {
            let chance = self
                .explicitly_chanced
                .iter()
                .map(|entry| entry.chance)
                .sum::<f32>();
            if chance != 100.0 && chance != 0.0 {
                return false;
            }
        }

        true
    }

    /// Deterministic representation of C++ `PoolGroup<T>::SpawnObject`.
    ///
    /// Source-of-truth spawned-pool state is the caller-provided map-owned
    /// `SpawnedPoolDataLikeCpp`. This helper mutates only that state for the
    /// represented `spawns.AddSpawn<T>` and final `DespawnObject(...triggerFrom)`
    /// branches, then returns explicit action records for live side effects that
    /// still belong to future owners (`Spawn1Object`, `ReSpawn1Object`,
    /// `DespawnObject`, recursive live `PoolMgr::SpawnPool`). The deterministic
    /// API requires a lazy explicit-roll provider so callers cannot skip C++
    /// `rand_chance()` when `ExplicitlyChanced` is non-empty, while still
    /// avoiding RNG consumption for C++ paths that do not call it.
    pub fn spawn_object_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        limit: u32,
        trigger_from: u64,
        mut explicit_roll: impl FnMut() -> f32,
        mut choose_equal: impl FnMut(&[PoolObjectLikeCpp], usize) -> Vec<usize>,
    ) -> PoolSpawnObjectPlanLikeCpp {
        let mut plan = PoolSpawnObjectPlanLikeCpp::default();
        let mut trigger_from = trigger_from;
        let spawned = i64::from(spawns.get_spawned_objects_like_cpp(self.pool_id));
        let mut count = i64::from(limit) - spawned;
        if trigger_from != 0 {
            count += 1;
        }

        if count > 0 {
            let mut rolled_objects = Vec::new();

            if !self.explicitly_chanced.is_empty() {
                let mut roll = explicit_roll();
                for obj in &self.explicitly_chanced {
                    roll -= obj.chance;
                    if roll < 0.0
                        && (obj.guid == trigger_from
                            || !self.is_spawned_in_map_like_cpp(spawns, obj.guid))
                    {
                        rolled_objects.push(*obj);
                        break;
                    }
                }
            }

            if !self.equal_chanced.is_empty() && rolled_objects.is_empty() {
                let candidates = self
                    .equal_chanced
                    .iter()
                    .copied()
                    .filter(|obj| {
                        obj.guid == trigger_from
                            || !self.is_spawned_in_map_like_cpp(spawns, obj.guid)
                    })
                    .collect::<Vec<_>>();
                let requested = match usize::try_from(count) {
                    Ok(value) => value.min(candidates.len()),
                    Err(_) => candidates.len(),
                };
                let chosen_indices = choose_equal(&candidates, requested);
                let mut used = vec![false; candidates.len()];
                for index in chosen_indices {
                    if rolled_objects.len() >= requested {
                        break;
                    }
                    if let Some(candidate) = candidates.get(index).copied() {
                        if !used[index] {
                            used[index] = true;
                            rolled_objects.push(candidate);
                        }
                    }
                }
            }

            for obj in rolled_objects {
                plan.selected.push(obj);
                if obj.guid == trigger_from {
                    plan.respawned_trigger = true;
                    if self.member_kind != PoolMemberKindLikeCpp::Pool {
                        plan.actions.push(PoolSpawnObjectActionLikeCpp::RespawnOne {
                            kind: self.member_kind,
                            guid: obj.guid,
                        });
                    }
                    trigger_from = 0;
                } else if self.add_spawn_to_map_like_cpp(spawns, obj.guid) {
                    plan.actions.push(PoolSpawnObjectActionLikeCpp::SpawnOne {
                        kind: self.member_kind,
                        guid: obj.guid,
                    });
                }
            }
        }

        if trigger_from != 0
            && self.contains_guid_like_cpp(trigger_from)
            && self.is_spawned_in_map_like_cpp(spawns, trigger_from)
            && self.remove_spawn_from_map_like_cpp(spawns, trigger_from)
        {
            plan.despawned_trigger = Some(trigger_from);
            plan.actions.push(PoolSpawnObjectActionLikeCpp::DespawnOne {
                kind: self.member_kind,
                guid: trigger_from,
            });
        }

        plan
    }

    /// Deterministic representation of C++ `PoolGroup<T>::DespawnObject`.
    ///
    /// Bucket order is exactly C++: `EqualChanced` first, then
    /// `ExplicitlyChanced`. The helper mutates only caller-owned
    /// `SpawnedPoolDataLikeCpp` when C++ would call `RemoveSpawn<T>`, records
    /// `Despawn1Object`/respawn-time delete side effects as plan actions, and
    /// delegates child-pool recursion before removing the child from the parent
    /// spawned relation.
    pub fn despawn_object_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        requested_guid: u64,
        always_delete_respawn_time: bool,
        mut despawn_child_pool: impl FnMut(
            &mut SpawnedPoolDataLikeCpp,
            u32,
            bool,
        ) -> Result<
            PoolDespawnPoolPlanLikeCpp,
            PoolMgrPlanErrorLikeCpp,
        >,
    ) -> Result<PoolDespawnObjectPlanLikeCpp, PoolMgrPlanErrorLikeCpp> {
        let mut plan = PoolDespawnObjectPlanLikeCpp {
            requested_guid,
            always_delete_respawn_time,
            ..PoolDespawnObjectPlanLikeCpp::default()
        };

        for object in self
            .equal_chanced
            .iter()
            .chain(self.explicitly_chanced.iter())
        {
            if self.member_kind == PoolMemberKindLikeCpp::Pool
                && u32::try_from(object.guid).is_err()
            {
                return Err(PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow {
                    child_pool_id: object.guid,
                });
            }
            let spawned = self.is_spawned_in_map_like_cpp(spawns, object.guid);
            if spawned {
                if requested_guid == 0 || object.guid == requested_guid {
                    plan.actions.push(PoolSpawnObjectActionLikeCpp::DespawnOne {
                        kind: self.member_kind,
                        guid: object.guid,
                    });
                    if self.member_kind == PoolMemberKindLikeCpp::Pool {
                        let child_pool_id = u32::try_from(object.guid).map_err(|_| {
                            PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow {
                                child_pool_id: object.guid,
                            }
                        })?;
                        let child_plan =
                            despawn_child_pool(spawns, child_pool_id, always_delete_respawn_time)?;
                        plan.child_pool_plans.push(child_plan);
                    }
                    if self.remove_spawn_from_map_like_cpp(spawns, object.guid) {
                        plan.despawned.push(object.guid);
                    }
                }
            } else if always_delete_respawn_time && self.member_kind != PoolMemberKindLikeCpp::Pool
            {
                plan.actions
                    .push(PoolSpawnObjectActionLikeCpp::RemoveRespawnTime {
                        kind: self.member_kind,
                        guid: object.guid,
                    });
                plan.removed_respawn_times
                    .push((self.member_kind, object.guid));
            }
        }

        Ok(plan)
    }

    /// C++ specialization `PoolGroup<Pool>::RemoveOneRelation`.
    ///
    /// Creature/GameObject groups have no specialization in C++; this pure Rust
    /// helper treats them as an explicit no-op. For Pool groups, it removes the
    /// first matching child from `ExplicitlyChanced` and then the first matching
    /// child from `EqualChanced`, so one match can be removed from each bucket.
    pub fn remove_one_relation_like_cpp(
        &mut self,
        child_pool_id: u32,
    ) -> PoolRelationRemovalLikeCpp {
        if self.member_kind != PoolMemberKindLikeCpp::Pool {
            return PoolRelationRemovalLikeCpp::default();
        }

        let mut removal = PoolRelationRemovalLikeCpp::default();
        let child_pool_id = u64::from(child_pool_id);

        if let Some(index) = self
            .explicitly_chanced
            .iter()
            .position(|entry| entry.guid == child_pool_id)
        {
            self.explicitly_chanced.remove(index);
            removal.removed_explicit = true;
        }

        if let Some(index) = self
            .equal_chanced
            .iter()
            .position(|entry| entry.guid == child_pool_id)
        {
            self.equal_chanced.remove(index);
            removal.removed_equal = true;
        }

        removal
    }

    fn contains_guid_like_cpp(&self, guid: u64) -> bool {
        self.explicitly_chanced
            .iter()
            .chain(self.equal_chanced.iter())
            .any(|entry| entry.guid == guid)
    }

    fn is_spawned_in_map_like_cpp(&self, spawns: &SpawnedPoolDataLikeCpp, guid: u64) -> bool {
        match self.member_kind {
            PoolMemberKindLikeCpp::Creature => spawns.is_spawned_creature_like_cpp(guid),
            PoolMemberKindLikeCpp::GameObject => spawns.is_spawned_gameobject_like_cpp(guid),
            PoolMemberKindLikeCpp::Pool => u32::try_from(guid)
                .ok()
                .is_some_and(|sub_pool_id| spawns.is_spawned_pool_like_cpp(sub_pool_id)),
        }
    }

    fn add_spawn_to_map_like_cpp(&self, spawns: &mut SpawnedPoolDataLikeCpp, guid: u64) -> bool {
        match self.member_kind {
            PoolMemberKindLikeCpp::Creature => spawns
                .add_spawn_like_cpp(SpawnObjectType::Creature, guid as SpawnId, self.pool_id)
                .is_ok(),
            PoolMemberKindLikeCpp::GameObject => spawns
                .add_spawn_like_cpp(SpawnObjectType::GameObject, guid as SpawnId, self.pool_id)
                .is_ok(),
            PoolMemberKindLikeCpp::Pool => {
                let Ok(sub_pool_id) = u32::try_from(guid) else {
                    return false;
                };
                spawns.add_pool_spawn_like_cpp(sub_pool_id, self.pool_id);
                true
            }
        }
    }

    fn remove_spawn_from_map_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        guid: u64,
    ) -> bool {
        match self.member_kind {
            PoolMemberKindLikeCpp::Creature => spawns
                .remove_spawn_like_cpp(SpawnObjectType::Creature, guid as SpawnId, self.pool_id)
                .is_ok(),
            PoolMemberKindLikeCpp::GameObject => spawns
                .remove_spawn_like_cpp(SpawnObjectType::GameObject, guid as SpawnId, self.pool_id)
                .is_ok(),
            PoolMemberKindLikeCpp::Pool => {
                let Ok(sub_pool_id) = u32::try_from(guid) else {
                    return false;
                };
                spawns.remove_pool_spawn_like_cpp(sub_pool_id, self.pool_id);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choose_first_indices(_candidates: &[PoolObjectLikeCpp], count: usize) -> Vec<usize> {
        (0..count).collect()
    }

    #[test]
    fn pool_group_spawn_object_explicit_roll_spawns_first_eligible_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 40.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 60.0), 1);

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 0, || 30.0, choose_first_indices);

        assert_eq!(plan.selected, vec![PoolObjectLikeCpp::new(101, 40.0)]);
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::Creature,
                guid: 101,
            }]
        );
        assert!(spawns.is_spawned_creature_like_cpp(101));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);
    }

    #[test]
    fn pool_group_spawn_object_explicit_spawned_miss_falls_back_to_equal_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::GameObject, 201, 7),
            Ok(())
        );
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(201, 100.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(202, 0.0), 1);

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 2, 0, || 50.0, choose_first_indices);

        assert_eq!(plan.selected, vec![PoolObjectLikeCpp::new(202, 0.0)]);
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::GameObject,
                guid: 202,
            }]
        );
        assert!(spawns.is_spawned_gameobject_like_cpp(201));
        assert!(spawns.is_spawned_gameobject_like_cpp(202));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 2);
    }

    #[test]
    fn pool_group_spawn_object_explicit_roll_miss_falls_back_to_equal_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(211, 40.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(212, 0.0), 1);

        let plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            0,
            || 80.0,
            |candidates, count| {
                assert_eq!(candidates, &[PoolObjectLikeCpp::new(212, 0.0)]);
                assert_eq!(count, 1);
                vec![0]
            },
        );

        assert_eq!(plan.selected, vec![PoolObjectLikeCpp::new(212, 0.0)]);
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::GameObject,
                guid: 212,
            }]
        );
        assert!(!spawns.is_spawned_gameobject_like_cpp(211));
        assert!(spawns.is_spawned_gameobject_like_cpp(212));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);
    }

    #[test]
    fn pool_group_spawn_object_equal_candidates_and_deterministic_selection_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 302, 7),
            Ok(())
        );
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(301, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(302, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(303, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(304, 0.0), 1);

        let mut observed_candidates = Vec::new();
        let plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            3,
            0,
            || 0.0,
            |candidates, count| {
                observed_candidates = candidates.to_vec();
                assert_eq!(count, 2);
                vec![1, 99, 1, 0]
            },
        );

        assert_eq!(
            observed_candidates,
            vec![
                PoolObjectLikeCpp::new(301, 0.0),
                PoolObjectLikeCpp::new(303, 0.0),
                PoolObjectLikeCpp::new(304, 0.0),
            ]
        );
        assert_eq!(
            plan.selected,
            vec![
                PoolObjectLikeCpp::new(303, 0.0),
                PoolObjectLikeCpp::new(301, 0.0),
            ]
        );
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 3);
        assert!(spawns.is_spawned_creature_like_cpp(301));
        assert!(spawns.is_spawned_creature_like_cpp(303));
        assert!(!spawns.is_spawned_creature_like_cpp(304));
    }

    #[test]
    fn pool_group_spawn_object_trigger_selected_respawns_without_counter_change_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 401, 7),
            Ok(())
        );
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(401, 100.0), 1);

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 401, || 50.0, choose_first_indices);

        assert_eq!(plan.selected, vec![PoolObjectLikeCpp::new(401, 100.0)]);
        assert!(plan.respawned_trigger);
        assert_eq!(plan.despawned_trigger, None);
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::RespawnOne {
                kind: PoolMemberKindLikeCpp::Creature,
                guid: 401,
            }]
        );
        assert!(spawns.is_spawned_creature_like_cpp(401));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);
    }

    #[test]
    fn pool_group_spawn_object_unselected_trigger_despawns_and_new_spawn_keeps_net_counter_like_cpp()
     {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::GameObject, 501, 7),
            Ok(())
        );
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(501, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(502, 0.0), 1);

        let plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            501,
            || 0.0,
            |_candidates, count| {
                assert_eq!(count, 1);
                vec![1]
            },
        );

        assert_eq!(plan.selected, vec![PoolObjectLikeCpp::new(502, 0.0)]);
        assert_eq!(plan.despawned_trigger, Some(501));
        assert_eq!(
            plan.actions,
            vec![
                PoolSpawnObjectActionLikeCpp::SpawnOne {
                    kind: PoolMemberKindLikeCpp::GameObject,
                    guid: 502,
                },
                PoolSpawnObjectActionLikeCpp::DespawnOne {
                    kind: PoolMemberKindLikeCpp::GameObject,
                    guid: 501,
                },
            ]
        );
        assert!(!spawns.is_spawned_gameobject_like_cpp(501));
        assert!(spawns.is_spawned_gameobject_like_cpp(502));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);
    }

    #[test]
    fn pool_group_spawn_object_count_non_positive_still_despawns_trigger_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 601, 7),
            Ok(())
        );
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 602, 7),
            Ok(())
        );
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 603, 7),
            Ok(())
        );
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(601, 0.0), 1);

        let plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            601,
            || 0.0,
            |_candidates, _count| vec![0],
        );

        assert!(plan.selected.is_empty());
        assert_eq!(plan.despawned_trigger, Some(601));
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::DespawnOne {
                kind: PoolMemberKindLikeCpp::Creature,
                guid: 601,
            }]
        );
        assert!(!spawns.is_spawned_creature_like_cpp(601));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 2);
    }

    #[test]
    fn pool_group_spawn_object_pool_kind_subpool_state_and_respawn_noop_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(701, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(702, 0.0), 1);

        let spawn_plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            0,
            || 0.0,
            |_candidates, _count| vec![0],
        );
        assert_eq!(
            spawn_plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::Pool,
                guid: 701,
            }]
        );
        assert!(spawns.is_spawned_pool_like_cpp(701));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);

        let respawn_plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            701,
            || 0.0,
            |_candidates, _count| vec![0],
        );
        assert!(respawn_plan.respawned_trigger);
        assert!(respawn_plan.actions.is_empty());
        assert!(spawns.is_spawned_pool_like_cpp(701));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);

        let despawn_plan = group.spawn_object_plan_like_cpp(
            &mut spawns,
            1,
            701,
            || 0.0,
            |_candidates, _count| vec![1],
        );
        assert_eq!(despawn_plan.despawned_trigger, Some(701));
        assert_eq!(
            despawn_plan.actions,
            vec![
                PoolSpawnObjectActionLikeCpp::SpawnOne {
                    kind: PoolMemberKindLikeCpp::Pool,
                    guid: 702,
                },
                PoolSpawnObjectActionLikeCpp::DespawnOne {
                    kind: PoolMemberKindLikeCpp::Pool,
                    guid: 701,
                },
            ]
        );
        assert!(!spawns.is_spawned_pool_like_cpp(701));
        assert!(spawns.is_spawned_pool_like_cpp(702));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);
    }

    #[test]
    fn pool_group_add_entry_buckets_match_cpp() {
        let mut group = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Creature);

        group.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 25.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(2, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(3, 25.0), 2);

        assert_eq!(
            group.explicitly_chanced_like_cpp(),
            &[PoolObjectLikeCpp::new(1, 25.0)]
        );
        assert_eq!(
            group.equal_chanced_like_cpp(),
            &[
                PoolObjectLikeCpp::new(2, 0.0),
                PoolObjectLikeCpp::new(3, 25.0),
            ]
        );
    }

    #[test]
    fn pool_group_check_pool_matches_cpp() {
        let mut valid_explicit = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::GameObject);
        valid_explicit.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 60.0), 1);
        valid_explicit.add_entry_like_cpp(PoolObjectLikeCpp::new(2, 40.0), 1);
        assert!(valid_explicit.check_pool_like_cpp());

        let mut invalid_explicit = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::GameObject);
        invalid_explicit.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 60.0), 1);
        assert!(!invalid_explicit.check_pool_like_cpp());

        let mut zero_total = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::GameObject);
        zero_total.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 0.0), 1);
        assert!(zero_total.check_pool_like_cpp());

        let mut equal_present = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::GameObject);
        equal_present.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 60.0), 1);
        equal_present.add_entry_like_cpp(PoolObjectLikeCpp::new(2, 0.0), 1);
        assert!(equal_present.check_pool_like_cpp());
    }

    #[test]
    fn pool_group_empty_deep_check_matches_cpp() {
        let empty_creature = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Creature);
        let mut creature_closure_calls = 0;
        assert!(empty_creature.is_empty_deep_check_like_cpp(|_| {
            creature_closure_calls += 1;
            false
        }));
        assert_eq!(creature_closure_calls, 0);

        let mut gameobject = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::GameObject);
        gameobject.add_entry_like_cpp(PoolObjectLikeCpp::new(1, 0.0), 1);
        let mut gameobject_closure_calls = 0;
        assert!(!gameobject.is_empty_deep_check_like_cpp(|_| {
            gameobject_closure_calls += 1;
            true
        }));
        assert_eq!(gameobject_closure_calls, 0);

        let mut pool = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Pool);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 50.0), 1);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(20, 0.0), 1);
        let mut visited = Vec::new();
        assert!(!pool.is_empty_deep_check_like_cpp(|child_pool_id| {
            visited.push(child_pool_id);
            child_pool_id != 20
        }));
        assert_eq!(visited, vec![10, 20]);

        let mut overflowing_pool = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Pool);
        overflowing_pool
            .add_entry_like_cpp(PoolObjectLikeCpp::new(u64::from(u32::MAX) + 1, 1.0), 1);
        assert!(!overflowing_pool.is_empty_deep_check_like_cpp(|_| true));
    }

    #[test]
    fn pool_group_remove_one_relation_matches_cpp() {
        let mut pool = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Pool);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 50.0), 1);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 40.0), 1);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 0.0), 1);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 0.0), 2);
        pool.add_entry_like_cpp(PoolObjectLikeCpp::new(11, 0.0), 1);

        let removal = pool.remove_one_relation_like_cpp(10);
        assert_eq!(
            removal,
            PoolRelationRemovalLikeCpp {
                removed_explicit: true,
                removed_equal: true,
            }
        );
        assert_eq!(
            pool.explicitly_chanced_like_cpp(),
            &[PoolObjectLikeCpp::new(10, 40.0)]
        );
        assert_eq!(
            pool.equal_chanced_like_cpp(),
            &[
                PoolObjectLikeCpp::new(10, 0.0),
                PoolObjectLikeCpp::new(11, 0.0),
            ]
        );

        let mut creature = PoolGroupLikeCpp::new(PoolMemberKindLikeCpp::Creature);
        creature.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 50.0), 1);
        assert_eq!(
            creature.remove_one_relation_like_cpp(10),
            PoolRelationRemovalLikeCpp::default()
        );
        assert_eq!(
            creature.explicitly_chanced_like_cpp(),
            &[PoolObjectLikeCpp::new(10, 50.0)]
        );
    }

    fn group_with_one(kind: PoolMemberKindLikeCpp, pool_id: u32, guid: u64) -> PoolGroupLikeCpp {
        let mut group = PoolGroupLikeCpp::with_pool_id(kind, pool_id);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(guid, 0.0), 1);
        group
    }

    #[test]
    fn pool_mgr_spawn_pool_orders_pool_gameobject_creature_and_mutates_map_spawns_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(3, 571));
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            10,
            group_with_one(PoolMemberKindLikeCpp::Creature, 10, 101),
        )
        .unwrap();
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            10,
            group_with_one(PoolMemberKindLikeCpp::GameObject, 10, 201),
        )
        .unwrap();
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Pool,
            10,
            group_with_one(PoolMemberKindLikeCpp::Pool, 10, 301),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();

        let mut roll_calls = Vec::new();
        let plan = mgr
            .spawn_pool_plan_like_cpp(
                &mut spawns,
                10,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    0.0
                },
                choose_first_indices,
            )
            .unwrap();

        let kinds = plan
            .subplans
            .iter()
            .map(|subplan| subplan.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                PoolMemberKindLikeCpp::Pool,
                PoolMemberKindLikeCpp::GameObject,
                PoolMemberKindLikeCpp::Creature,
            ]
        );
        assert!(roll_calls.is_empty());
        assert!(spawns.is_spawned_pool_like_cpp(301));
        assert!(spawns.is_spawned_gameobject_like_cpp(201));
        assert!(spawns.is_spawned_creature_like_cpp(101));
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 3);
        assert_eq!(
            plan.subplans[0].object_plan.as_ref().unwrap().actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::Pool,
                guid: 301,
            }]
        );
    }

    #[test]
    fn pool_mgr_spawn_pool_uses_independent_explicit_rolls_per_top_level_kind_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(2, 571));

        let mut gameobject_group =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10);
        gameobject_group.add_entry_like_cpp(PoolObjectLikeCpp::new(201, 25.0), 1);
        gameobject_group.add_entry_like_cpp(PoolObjectLikeCpp::new(202, 25.0), 1);
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            10,
            gameobject_group,
        )
        .unwrap();

        let mut creature_group =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        creature_group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 25.0), 1);
        creature_group.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 25.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, creature_group)
            .unwrap();

        let mut spawns = SpawnedPoolDataLikeCpp::new();
        let mut roll_calls = Vec::new();
        let plan = mgr
            .spawn_pool_plan_like_cpp(
                &mut spawns,
                10,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    match kind {
                        PoolMemberKindLikeCpp::Pool => 0.0,
                        PoolMemberKindLikeCpp::GameObject => 10.0,
                        PoolMemberKindLikeCpp::Creature => 30.0,
                    }
                },
                choose_first_indices,
            )
            .unwrap();

        assert_eq!(
            roll_calls,
            vec![
                (PoolMemberKindLikeCpp::GameObject, 10),
                (PoolMemberKindLikeCpp::Creature, 10),
            ]
        );
        assert_eq!(
            plan.subplans[0].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup)
        );
        assert!(spawns.is_spawned_gameobject_like_cpp(201));
        assert!(!spawns.is_spawned_gameobject_like_cpp(202));
        assert!(!spawns.is_spawned_creature_like_cpp(101));
        assert!(spawns.is_spawned_creature_like_cpp(102));

        let gameobject_plan = plan.subplans[1].object_plan.as_ref().unwrap();
        let creature_plan = plan.subplans[2].object_plan.as_ref().unwrap();
        assert_eq!(
            gameobject_plan.selected,
            vec![PoolObjectLikeCpp::new(201, 25.0)]
        );
        assert_eq!(
            creature_plan.selected,
            vec![PoolObjectLikeCpp::new(102, 25.0)]
        );
    }

    #[test]
    fn pool_mgr_missing_template_errors_without_limit_zero_or_spawn_mutation_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            10,
            group_with_one(PoolMemberKindLikeCpp::Creature, 10, 101),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();

        let result = mgr.spawn_typed_pool_plan_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            &mut spawns,
            10,
            0,
            |_, _| 0.0,
            choose_first_indices,
        );

        assert_eq!(
            result,
            Err(PoolMgrPlanErrorLikeCpp::MissingTemplate { pool_id: 10 })
        );
        assert!(!spawns.is_spawned_creature_like_cpp(101));
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 0);
    }

    #[test]
    fn pool_mgr_missing_and_empty_groups_are_noop_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            10,
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();

        let mut roll_calls = Vec::new();
        let plan = mgr
            .spawn_pool_plan_like_cpp(
                &mut spawns,
                10,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    0.0
                },
                choose_first_indices,
            )
            .unwrap();

        assert_eq!(
            plan.subplans[0].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup)
        );
        assert_eq!(
            plan.subplans[1].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::EmptyGroup)
        );
        assert_eq!(
            plan.subplans[2].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup)
        );
        assert!(roll_calls.is_empty());
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 0);
    }

    #[test]
    fn pool_mgr_equal_only_group_does_not_request_explicit_roll_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            10,
            group_with_one(PoolMemberKindLikeCpp::Creature, 10, 101),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        let mut roll_calls = Vec::new();

        let plan = mgr
            .spawn_pool_plan_like_cpp(
                &mut spawns,
                10,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    0.0
                },
                choose_first_indices,
            )
            .unwrap();

        assert!(roll_calls.is_empty());
        assert!(plan.subplans[2].object_plan.is_some());
        assert!(spawns.is_spawned_creature_like_cpp(101));
    }

    #[test]
    fn pool_mgr_count_non_positive_does_not_request_explicit_roll_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 100.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, group)
            .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 10),
            Ok(())
        );
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 102, 10),
            Ok(())
        );
        let mut roll_calls = Vec::new();

        let plan = mgr
            .spawn_typed_pool_plan_like_cpp(
                PoolMemberKindLikeCpp::Creature,
                &mut spawns,
                10,
                0,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    0.0
                },
                choose_first_indices,
            )
            .unwrap();

        assert!(roll_calls.is_empty());
        assert!(plan.object_plan.unwrap().selected.is_empty());
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 2);
    }

    #[test]
    fn pool_mgr_update_pool_child_pool_uses_mother_pool_branch_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(50, PoolTemplateDataLikeCpp::new(1, 571));
        let mut mother_group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 50);
        mother_group.add_entry_like_cpp(PoolObjectLikeCpp::new(70, 0.0), 1);
        mother_group.add_entry_like_cpp(PoolObjectLikeCpp::new(71, 0.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 50, mother_group)
            .unwrap();
        mgr.register_child_pool_relation_like_cpp(70, 50).unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        spawns.add_pool_spawn_like_cpp(70, 50);

        let plan = mgr
            .update_pool_plan_like_cpp(
                &mut spawns,
                70,
                SpawnObjectType::Creature,
                999,
                |_, _| 0.0,
                |_candidates, count| {
                    assert_eq!(count, 1);
                    vec![1]
                },
            )
            .unwrap();

        assert_eq!(plan.kind, PoolMemberKindLikeCpp::Pool);
        assert_eq!(plan.pool_id, 50);
        assert_eq!(plan.trigger_from, 70);
        let object_plan = plan.object_plan.unwrap();
        assert_eq!(object_plan.selected, vec![PoolObjectLikeCpp::new(71, 0.0)]);
        assert_eq!(object_plan.despawned_trigger, Some(70));
        assert!(!spawns.is_spawned_pool_like_cpp(70));
        assert!(spawns.is_spawned_pool_like_cpp(71));
    }

    #[test]
    fn pool_mgr_update_pool_creature_and_gameobject_no_child_dispatch_typed_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        mgr.insert_template_like_cpp(20, PoolTemplateDataLikeCpp::new(1, 571));
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            10,
            group_with_one(PoolMemberKindLikeCpp::Creature, 10, 101),
        )
        .unwrap();
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            20,
            group_with_one(PoolMemberKindLikeCpp::GameObject, 20, 201),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 10),
            Ok(())
        );
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::GameObject, 201, 20),
            Ok(())
        );

        let creature_plan = mgr
            .update_pool_plan_like_cpp(
                &mut spawns,
                10,
                SpawnObjectType::Creature,
                101,
                |_, _| 0.0,
                choose_first_indices,
            )
            .unwrap();
        let gameobject_plan = mgr
            .update_pool_plan_like_cpp(
                &mut spawns,
                20,
                SpawnObjectType::GameObject,
                201,
                |_, _| 0.0,
                choose_first_indices,
            )
            .unwrap();

        assert_eq!(creature_plan.kind, PoolMemberKindLikeCpp::Creature);
        assert_eq!(creature_plan.trigger_from, 101);
        assert!(creature_plan.object_plan.unwrap().respawned_trigger);
        assert_eq!(gameobject_plan.kind, PoolMemberKindLikeCpp::GameObject);
        assert_eq!(gameobject_plan.trigger_from, 201);
        assert!(gameobject_plan.object_plan.unwrap().respawned_trigger);
    }

    #[test]
    fn pool_mgr_update_pool_areatrigger_is_unsupported_and_preserves_spawns_like_cpp() {
        let mgr = PoolMgrLikeCpp::new();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 10),
            Ok(())
        );

        let mut roll_calls = Vec::new();
        let result = mgr.update_pool_plan_like_cpp(
            &mut spawns,
            10,
            SpawnObjectType::AreaTrigger,
            301,
            |kind, pool_id| {
                roll_calls.push((kind, pool_id));
                0.0
            },
            choose_first_indices,
        );

        assert_eq!(
            result,
            Err(PoolMgrPlanErrorLikeCpp::UnsupportedSpawnType {
                spawn_type: SpawnObjectType::AreaTrigger,
            })
        );
        assert!(spawns.is_spawned_creature_like_cpp(101));
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 1);
        assert_eq!(
            mgr.is_part_of_a_pool_like_cpp(SpawnObjectType::AreaTrigger, 301),
            Ok(0)
        );
        assert!(roll_calls.is_empty());
    }

    #[test]
    fn pool_mgr_update_pool_mother_branch_requests_lazy_pool_roll_only_when_explicit_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(50, PoolTemplateDataLikeCpp::new(1, 571));
        let mut mother_group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 50);
        mother_group.add_entry_like_cpp(PoolObjectLikeCpp::new(70, 50.0), 1);
        mother_group.add_entry_like_cpp(PoolObjectLikeCpp::new(71, 50.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 50, mother_group)
            .unwrap();
        mgr.register_child_pool_relation_like_cpp(70, 50).unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        spawns.add_pool_spawn_like_cpp(70, 50);
        let mut roll_calls = Vec::new();

        let plan = mgr
            .update_pool_plan_like_cpp(
                &mut spawns,
                70,
                SpawnObjectType::Creature,
                999,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    60.0
                },
                choose_first_indices,
            )
            .unwrap();

        assert_eq!(roll_calls, vec![(PoolMemberKindLikeCpp::Pool, 50)]);
        assert_eq!(plan.kind, PoolMemberKindLikeCpp::Pool);
        assert_eq!(
            plan.object_plan.unwrap().selected,
            vec![PoolObjectLikeCpp::new(71, 50.0)]
        );
        assert!(!spawns.is_spawned_pool_like_cpp(70));
        assert!(spawns.is_spawned_pool_like_cpp(71));
    }

    #[test]
    fn pool_mgr_update_pool_typed_branch_requests_lazy_roll_only_when_explicit_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 571));
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 50.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 50.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, group)
            .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            spawns.add_spawn_like_cpp(SpawnObjectType::Creature, 101, 10),
            Ok(())
        );
        let mut roll_calls = Vec::new();

        let plan = mgr
            .update_pool_plan_like_cpp(
                &mut spawns,
                10,
                SpawnObjectType::Creature,
                101,
                |kind, pool_id| {
                    roll_calls.push((kind, pool_id));
                    60.0
                },
                choose_first_indices,
            )
            .unwrap();

        assert_eq!(roll_calls, vec![(PoolMemberKindLikeCpp::Creature, 10)]);
        assert_eq!(plan.kind, PoolMemberKindLikeCpp::Creature);
        assert_eq!(
            plan.object_plan.unwrap().selected,
            vec![PoolObjectLikeCpp::new(102, 50.0)]
        );
        assert!(!spawns.is_spawned_creature_like_cpp(101));
        assert!(spawns.is_spawned_creature_like_cpp(102));
    }

    #[test]
    fn pool_mgr_is_empty_and_check_pool_preserve_cpp_order_and_result() {
        let mut mgr = PoolMgrLikeCpp::new();
        let mut invalid_gameobject =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10);
        invalid_gameobject.add_entry_like_cpp(PoolObjectLikeCpp::new(201, 60.0), 1);
        let mut invalid_creature =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        invalid_creature.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 50.0), 1);
        invalid_creature.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 50.0), 1);
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            10,
            invalid_gameobject,
        )
        .unwrap();
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, invalid_creature)
            .unwrap();

        assert!(!mgr.is_empty_like_cpp(10));
        assert!(!mgr.check_pool_like_cpp(10));

        let mut parent = PoolMgrLikeCpp::new();
        let mut parent_group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 40);
        parent_group.add_entry_like_cpp(PoolObjectLikeCpp::new(41, 0.0), 1);
        parent
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 40, parent_group)
            .unwrap();
        assert!(parent.is_empty_like_cpp(40));
        parent
            .insert_or_replace_group_like_cpp(
                PoolMemberKindLikeCpp::Creature,
                41,
                group_with_one(PoolMemberKindLikeCpp::Creature, 41, 411),
            )
            .unwrap();
        assert!(!parent.is_empty_like_cpp(40));

        let mut cyclic = PoolMgrLikeCpp::new();
        cyclic
            .insert_or_replace_group_like_cpp(
                PoolMemberKindLikeCpp::Pool,
                1,
                group_with_one(PoolMemberKindLikeCpp::Pool, 1, 1),
            )
            .unwrap();
        assert!(!cyclic.is_empty_like_cpp(1));
    }

    #[test]
    fn pool_mgr_builders_validate_group_kind_and_child_pool_overflow_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        assert_eq!(
            mgr.insert_or_replace_group_like_cpp(
                PoolMemberKindLikeCpp::Creature,
                10,
                PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10),
            ),
            Err(PoolMgrPlanErrorLikeCpp::WrongGroupKind {
                expected: PoolMemberKindLikeCpp::Creature,
                actual: PoolMemberKindLikeCpp::GameObject,
            })
        );
        assert_eq!(
            mgr.register_child_pool_relation_like_cpp(u64::from(u32::MAX) + 1, 10),
            Err(PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow {
                child_pool_id: u64::from(u32::MAX) + 1,
            })
        );
    }

    #[test]
    fn pool_mgr_despawn_pool_order_and_bucket_order_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        let mut creatures = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 10);
        creatures.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 0.0), 2);
        creatures.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 100.0), 1);
        let mut gameobjects = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10);
        gameobjects.add_entry_like_cpp(PoolObjectLikeCpp::new(201, 0.0), 2);
        gameobjects.add_entry_like_cpp(PoolObjectLikeCpp::new(202, 100.0), 1);
        let mut pools = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 10);
        pools.add_entry_like_cpp(PoolObjectLikeCpp::new(20, 0.0), 2);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 10, creatures)
            .unwrap();
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::GameObject, 10, gameobjects)
            .unwrap();
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 10, pools)
            .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        spawns
            .add_spawn_like_cpp(SpawnObjectType::Creature, 101, 10)
            .unwrap();
        spawns
            .add_spawn_like_cpp(SpawnObjectType::Creature, 102, 10)
            .unwrap();
        spawns
            .add_spawn_like_cpp(SpawnObjectType::GameObject, 201, 10)
            .unwrap();
        spawns
            .add_spawn_like_cpp(SpawnObjectType::GameObject, 202, 10)
            .unwrap();
        spawns.add_pool_spawn_like_cpp(20, 10);

        let plan = mgr
            .despawn_pool_plan_like_cpp(&mut spawns, 10, false)
            .unwrap();

        assert_eq!(
            plan.subplans
                .iter()
                .map(|subplan| subplan.kind)
                .collect::<Vec<_>>(),
            vec![
                PoolMemberKindLikeCpp::Creature,
                PoolMemberKindLikeCpp::GameObject,
                PoolMemberKindLikeCpp::Pool,
            ]
        );
        assert_eq!(
            plan.subplans[0].object_plan.as_ref().unwrap().despawned,
            vec![101, 102]
        );
        assert_eq!(
            plan.subplans[1].object_plan.as_ref().unwrap().despawned,
            vec![201, 202]
        );
        assert_eq!(
            plan.subplans[2].object_plan.as_ref().unwrap().despawned,
            vec![20]
        );
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 0);
    }

    #[test]
    fn pool_mgr_despawn_mutates_parent_and_nested_child_pool_state_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        let mut parent_pools = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 10);
        parent_pools.add_entry_like_cpp(PoolObjectLikeCpp::new(20, 0.0), 1);
        let mut child_creatures =
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 20);
        child_creatures.add_entry_like_cpp(PoolObjectLikeCpp::new(2001, 0.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 10, parent_pools)
            .unwrap();
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 20, child_creatures)
            .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        spawns.add_pool_spawn_like_cpp(20, 10);
        spawns
            .add_spawn_like_cpp(SpawnObjectType::Creature, 2001, 20)
            .unwrap();

        let plan = mgr
            .despawn_pool_plan_like_cpp(&mut spawns, 10, false)
            .unwrap();

        let pool_object_plan = plan.subplans[2].object_plan.as_ref().unwrap();
        assert_eq!(pool_object_plan.despawned, vec![20]);
        assert_eq!(pool_object_plan.child_pool_plans.len(), 1);
        assert_eq!(
            pool_object_plan.child_pool_plans[0].subplans[0]
                .object_plan
                .as_ref()
                .unwrap()
                .despawned,
            vec![2001]
        );
        assert!(!spawns.is_spawned_pool_like_cpp(20));
        assert!(!spawns.is_spawned_creature_like_cpp(2001));
        assert_eq!(spawns.get_spawned_objects_like_cpp(10), 0);
        assert_eq!(spawns.get_spawned_objects_like_cpp(20), 0);
    }

    #[test]
    fn pool_mgr_despawn_missing_and_empty_groups_skip_without_template_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::GameObject,
            10,
            PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::GameObject, 10),
        )
        .unwrap();
        let mut spawns = SpawnedPoolDataLikeCpp::new();

        let plan = mgr
            .despawn_pool_plan_like_cpp(&mut spawns, 10, true)
            .unwrap();

        assert_eq!(
            plan.subplans[0].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup)
        );
        assert_eq!(
            plan.subplans[1].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::EmptyGroup)
        );
        assert_eq!(
            plan.subplans[2].skip_reason,
            Some(PoolMgrPlanSkipReasonLikeCpp::MissingGroup)
        );
    }

    #[test]
    fn pool_group_despawn_delete_respawn_time_only_for_unspawned_creature_and_go_like_cpp() {
        let mut spawns = SpawnedPoolDataLikeCpp::new();
        spawns
            .add_spawn_like_cpp(SpawnObjectType::Creature, 101, 7)
            .unwrap();
        let mut group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 7);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 0.0), 1);
        group.add_entry_like_cpp(PoolObjectLikeCpp::new(102, 0.0), 1);

        let plan = group
            .despawn_object_plan_like_cpp(&mut spawns, 999, true, |_, _, _| unreachable!())
            .unwrap();

        assert!(spawns.is_spawned_creature_like_cpp(101));
        assert_eq!(plan.despawned, Vec::<u64>::new());
        assert_eq!(
            plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::RemoveRespawnTime {
                kind: PoolMemberKindLikeCpp::Creature,
                guid: 102,
            }]
        );
        assert_eq!(
            plan.removed_respawn_times,
            vec![(PoolMemberKindLikeCpp::Creature, 102)]
        );

        let mut pool_spawns = SpawnedPoolDataLikeCpp::new();
        let pool_group = group_with_one(PoolMemberKindLikeCpp::Pool, 7, 20);
        let pool_plan = pool_group
            .despawn_object_plan_like_cpp(&mut pool_spawns, 0, true, |_, _, _| unreachable!())
            .unwrap();
        assert!(pool_plan.actions.is_empty());
        assert!(pool_plan.removed_respawn_times.is_empty());
    }

    #[test]
    fn pool_mgr_despawn_child_pool_overflow_and_cycle_are_typed_errors_like_cpp() {
        let mut overflow_mgr = PoolMgrLikeCpp::new();
        let mut overflow_group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 10);
        let overflowing_child = u64::from(u32::MAX) + 1;
        overflow_group.add_entry_like_cpp(PoolObjectLikeCpp::new(overflowing_child, 0.0), 1);
        overflow_mgr
            .insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 10, overflow_group)
            .unwrap();
        let mut overflow_spawns = SpawnedPoolDataLikeCpp::new();
        assert_eq!(
            overflow_mgr.despawn_pool_plan_like_cpp(&mut overflow_spawns, 10, false),
            Err(PoolMgrPlanErrorLikeCpp::ChildPoolIdOverflow {
                child_pool_id: overflowing_child,
            })
        );

        let mut cyclic = PoolMgrLikeCpp::new();
        cyclic
            .insert_or_replace_group_like_cpp(
                PoolMemberKindLikeCpp::Pool,
                1,
                group_with_one(PoolMemberKindLikeCpp::Pool, 1, 1),
            )
            .unwrap();
        let mut cyclic_spawns = SpawnedPoolDataLikeCpp::new();
        cyclic_spawns.add_pool_spawn_like_cpp(1, 1);
        assert_eq!(
            cyclic.despawn_pool_plan_like_cpp(&mut cyclic_spawns, 1, false),
            Err(PoolMgrPlanErrorLikeCpp::ChildPoolCycle { pool_id: 1 })
        );
    }

    #[test]
    fn pool_mgr_autospawn_tracks_top_level_non_child_only_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        mgr.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 530));
        mgr.insert_template_like_cpp(20, PoolTemplateDataLikeCpp::new(1, 530));
        mgr.insert_or_replace_group_like_cpp(
            PoolMemberKindLikeCpp::Creature,
            10,
            group_with_one(PoolMemberKindLikeCpp::Creature, 10, 1001),
        )
        .unwrap();
        let mut parent_group = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, 20);
        parent_group.add_entry_like_cpp(PoolObjectLikeCpp::new(10, 0.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Pool, 20, parent_group)
            .unwrap();
        mgr.register_child_pool_relation_like_cpp(10, 20).unwrap();

        assert_eq!(mgr.top_level_auto_spawn_candidate_like_cpp(10), None);
        assert_eq!(mgr.top_level_auto_spawn_candidate_like_cpp(20), Some(530));
        mgr.add_auto_spawn_pool_like_cpp(530, 20);
        assert_eq!(mgr.auto_spawn_pools_for_map_like_cpp(530), &[20]);
        assert!(mgr.auto_spawn_pools_for_map_like_cpp(-1).is_empty());
    }
}
