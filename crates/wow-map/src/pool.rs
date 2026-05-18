//! Pure C++-shaped pool data helpers.
//!
//! Source of truth: TrinityCore `PoolMgr.h` / `PoolMgr.cpp` pool data and
//! `PoolGroup<T>` helpers. This module intentionally does not implement live
//! `PoolMgr` runtime, RNG, DB loading, entity creation, `SpawnPool`, or
//! `DespawnPool`. `Map::pool_data` remains the map-owned source of truth for
//! spawned pool state; `PoolGroupLikeCpp` is only foundation data for later
//! layers.

use crate::map::SpawnedPoolDataLikeCpp;
use crate::spawn::{SpawnId, SpawnObjectType};

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
}

/// Deterministic result of represented C++ `PoolGroup<T>::SpawnObject`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PoolSpawnObjectPlanLikeCpp {
    pub actions: Vec<PoolSpawnObjectActionLikeCpp>,
    pub selected: Vec<PoolObjectLikeCpp>,
    pub despawned_trigger: Option<u64>,
    pub respawned_trigger: bool,
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
    /// API requires an explicit roll value so callers cannot skip C++
    /// `rand_chance()` whenever `ExplicitlyChanced` is non-empty; the value is
    /// ignored for groups without explicit-chanced members.
    pub fn spawn_object_plan_like_cpp(
        &self,
        spawns: &mut SpawnedPoolDataLikeCpp,
        limit: u32,
        trigger_from: u64,
        explicit_roll: f32,
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
                let mut roll = explicit_roll;
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

        let plan = group.spawn_object_plan_like_cpp(&mut spawns, 1, 0, 30.0, choose_first_indices);

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

        let plan = group.spawn_object_plan_like_cpp(&mut spawns, 2, 0, 50.0, choose_first_indices);

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

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 0, 80.0, |candidates, count| {
                assert_eq!(candidates, &[PoolObjectLikeCpp::new(212, 0.0)]);
                assert_eq!(count, 1);
                vec![0]
            });

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
        let plan = group.spawn_object_plan_like_cpp(&mut spawns, 3, 0, 0.0, |candidates, count| {
            observed_candidates = candidates.to_vec();
            assert_eq!(count, 2);
            vec![1, 99, 1, 0]
        });

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
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 401, 50.0, choose_first_indices);

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

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 501, 0.0, |_candidates, count| {
                assert_eq!(count, 1);
                vec![1]
            });

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

        let plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 601, 0.0, |_candidates, _count| {
                vec![0]
            });

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

        let spawn_plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 0, 0.0, |_candidates, _count| vec![0]);
        assert_eq!(
            spawn_plan.actions,
            vec![PoolSpawnObjectActionLikeCpp::SpawnOne {
                kind: PoolMemberKindLikeCpp::Pool,
                guid: 701,
            }]
        );
        assert!(spawns.is_spawned_pool_like_cpp(701));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);

        let respawn_plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 701, 0.0, |_candidates, _count| {
                vec![0]
            });
        assert!(respawn_plan.respawned_trigger);
        assert!(respawn_plan.actions.is_empty());
        assert!(spawns.is_spawned_pool_like_cpp(701));
        assert_eq!(spawns.get_spawned_objects_like_cpp(7), 1);

        let despawn_plan =
            group.spawn_object_plan_like_cpp(&mut spawns, 1, 701, 0.0, |_candidates, _count| {
                vec![1]
            });
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
}
