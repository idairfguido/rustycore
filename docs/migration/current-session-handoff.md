# Current Session Handoff

Generated: 2026-05-18

Continuity snapshot for RustyCore C++ -> Rust migration in `/home/server/rustycore`.

## Repository State

- Branch: `develop`
- Current branch state before #380 finalization: `develop...origin/develop [ahead 34]` with a clean tree.
- Current branch state after #382 local commit: `develop...origin/develop [ahead 37]`.
- Current branch state after #383 local commit: `develop...origin/develop [ahead 38]`.
- Current branch state after #384 local commit: `develop...origin/develop [ahead 39]`.
- Current branch state after #385 local commit: `develop...origin/develop [ahead 40]`.
- Current branch state after #386 local commit: `develop...origin/develop [ahead 41]` with a clean tree.
- Latest completed slice in this handoff: `#NEXT.R8.ENTITIES.386 — PoolMgr SpawnPool/UpdatePool deterministic orchestration plan` (implemented/tested locally; independent review found eager RNG consumption and the final fix uses lazy per-specialization explicit rolls only at the C++ rand_chance point; final review `APROBADO`; validation passed; committed locally).
- No push/install/restart performed for #383, #384, #385, or #386.

## Critical Rules

- Source of truth is TrinityCore C++ 3.4.3 build 12340/TDB442 in `/home/server/woltk-trinity-legacy`.
- Do not use Retail, Cataclysm, 3.3.5, C#, or CypherCore as authority.
- Do not trust existing Rust, tests, comments, prior AI work, or roadmap checkboxes without C++ anchors.
- No live server start, install, restart, push, or merge unless Joe explicitly requests it.
- Commit locally only after independent review plus validation.

## Progress Estimate

Overall core migration estimate after #386 `PoolMgr SpawnPool/UpdatePool deterministic orchestration plan`: `~88.9%`.

This remains intentionally below the R8 TSV row-completion ratio because heavy runtime ownership gaps remain: real `PoolMgr::SpawnPool`/`DespawnPool` orchestration, live chance/RNG integration, recursive subpool live integration, full live `ProcessRespawns` pool and `DoRespawn` branches, entity creation/`LoadFromDB`, corpse load, AreaTrigger Create/Load/Update runtime, templates/spawns, AI, caster unregister, unit enter/exit, movement/visibility/transport, real terrain/vmap/dynamic-tree collision, transports, visibility overrides/cinematic/sight runtime, full entity-specific `AddToWorld`/`RemoveFromWorld` side effects beyond the object/spawn-id store, real dynamic escort config/runtime feeding the closure, grid/session fanout, ObjectAccessor ownership, DB save/delete coverage beyond current seams, and broader Unit/Player inventory/auras/threat/motion/update-field work.

Manual test point: no new client-facing manual milestone from #386; this is a deterministic `PoolMgr::SpawnPool`/`UpdatePool` orchestration plan seam for later live PoolMgr ownership, validated with focused `wow-map` unit checks.

## Most Recent Completed Slices

- `#NEXT.R8.ENTITIES.386` (implemented/tested locally; final review `APROBADO`; validation passed; committed locally)
  - Adds pure C++-shaped `PoolMgrLikeCpp` planning state in `wow-map`: templates, separate Creature/GameObject/Pool groups, spawn->pool and child-pool->mother-pool indices, builder validation, typed errors, and public plan/result wrappers for represented `SpawnPool<T>`, top-level `SpawnPool`, `UpdatePool`, `IsPartOfAPool`, `IsEmpty`, and `CheckPool`.
  - Preserves C++ order and early returns from `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:776-811`, `833-920`, and uses existing #385 `PoolGroupLikeCpp::spawn_object_plan_like_cpp` for the represented `PoolGroup<T>::SpawnObject` side-effect seam from `PoolMgr.cpp:288-407`. The deterministic API now passes a lazy per-specialization explicit-roll provider through Pool -> GameObject -> Creature and calls it only inside `PoolGroup<T>::SpawnObject` when `count > 0` and `ExplicitlyChanced` is non-empty, matching the C++ `rand_chance()` point and avoiding rolls for missing groups, empty groups, equal-only groups, and count<=0. Runtime source of truth remains caller-provided map-owned `SpawnedPoolDataLikeCpp`; no parallel spawned-state cache is introduced.
  - Remaining gaps: LoadFromDB real, auto-spawn map init, DespawnPool live, Spawn1Object/ReSpawn1Object/Despawn1Object real, entity creation/AddToMap/RemoveFromMap, DB save/delete, ProcessRespawns pool branch, recursive live PoolMgr execution, and grid/session fanout.

- `#NEXT.R8.ENTITIES.385` (completed; review `APROBADO`; focused validation passed; committed locally at current HEAD)
  - Adds deterministic `PoolGroupLikeCpp::spawn_object_plan_like_cpp` over map-owned `SpawnedPoolDataLikeCpp`: signed count calculation, trigger increment semantics, required explicit-chanced single-roll selection representing C++ `rand_chance()` when that bucket is non-empty, equal-chanced deterministic selection hook, normal spawn mutations, trigger respawn no-counter-change, and fallback trigger despawn/removal.
  - Returns explicit action records for future side-effect owners (`SpawnOne`, `RespawnOne`, `DespawnOne`) without live entity creation, DB writes, recursive live `PoolMgr`, grid-loaded checks, AddToMap/RemoveFromMap, install/restart, fanout, or server startup.
  - Remaining gaps: `PoolMgr::SpawnPool` orchestration order, `DespawnPool`, entity `CreateFromDB`/`AddToMap`/`RemoveFromMap`, grid-loaded checks, DB save/delete, `ProcessRespawns` pool branch, recursive live PoolMgr integration, and grid/session fanout.

- `#NEXT.R8.ENTITIES.384` (completed; review/validation complete for this slice)
  - Adds `wow_map::pool` pure C++-shaped `PoolTemplateDataLikeCpp`, `PoolObjectLikeCpp`, `PoolMemberKindLikeCpp`, `PoolGroupLikeCpp`, and `PoolRelationRemovalLikeCpp`, reexported from `wow-map`.
  - Represents the C++ `PoolGroup<T>` data helpers only: `AddEntry` explicit/equal bucket selection, `CheckPool` explicit chance validation only when equal bucket is empty, normal `isEmptyDeepCheck`, Pool-of-Pools deep-check via caller closure without truncating oversized child ids, and Pool-only `RemoveOneRelation` first explicit plus first equal removal.
  - This closes only pure in-memory pool foundation. Remaining gaps: no live `PoolMgr`, no `SpawnPool`/`DespawnPool`, no RNG/chance rolling, no DB loader, no `ProcessRespawns` pool branch, no entity creation/AddToMap/RemoveFromMap, no `Map::pool_data` synchronization from `PoolGroup`, and no grid/session fanout.

- `#NEXT.R8.ENTITIES.383` (completed; review `APROBADO`; focused validation passed; committed locally at current HEAD)
  - Adds map-owned `SpawnedPoolDataLikeCpp` to `wow_map::Map`, initialized in `with_hooks`, with C++-shaped creature/gameobject/subpool membership, duplicate-add counter semantics, saturating/remove-create-zero semantics, AreaTrigger unsupported typed error, and spawned object extraction for grid-load checks.
  - `Map::spawn_grid_load_state_like_cpp` now feeds `SpawnGridLoadStateLikeCpp` from map-owned respawn timers, map-owned spawn-group state, and map-owned spawned pool creature/gameobject entries.
  - This closes only map-owned SpawnedPoolData state plus the grid-load seam. Remaining gaps: no real `PoolMgr::SpawnPool`/`DespawnPool`, no RNG/chance, no `ProcessRespawns` pool branch, no entity creation/AddToMap/RemoveFromMap, no DB persistence/delete, and no grid/session fanout.

- `#NEXT.R8.ENTITIES.382` (completed; review `APROBADO`; focused validation passed; committed locally at current HEAD)
  - `WorldSession::canonical_player_entity_snapshot_like_cpp` now copies represented player shape into the canonical `Player` before map insertion/replacement: default display/native display from race/gender, mount display id, object scale, and current collision height.
  - `update_player_collision_height_like_cpp` recomputes height when DB2 stores are available, but always propagates current represented mount display id plus collision height to an existing canonical player, preserving the allowed session -> canonical sync direction and fallback height behavior.
  - This closes only the canonical player shape dependency for map-local/ObjectAccessor consumers. It does not close full Maps/ObjectAccessor/LOS ownership, real terrain/vmap/dynamic-tree collision, or full Player Create/Load/login lifecycle.

- `#NEXT.R8.ENTITIES.381` (completed; review `APROBADO`; focused validation passed; committed locally at current HEAD)
  - Adds resolved runtime `WorldObject::collision_height_like_cpp` state (base default `0.0`) plus C++-shaped LOS endpoint construction for `IsWithinLOS`, `IsWithinLOSInMap`, and `GetHitSpherePointFor`.
  - `Unit::set_collision_height_like_cpp` hydrates the embedded `WorldObject` scalar, and `Unit::set_combat_reach` keeps `UnitData` and embedded `WorldObject` reach coherent for hit-sphere/distance helpers.
  - This is bounded endpoint parity only; it does not close #NEXT.R8.ENTITIES.005. Terrain/vmap/dynamic-tree collision, transports, visibility overrides/cinematic/sight runtime, DB2/model collision-height hydration, and broader Unit/Player runtime remain gaps.

- `#NEXT.R8.ENTITIES.380`
  - Adds `MapWorldObjectEnvironment` as the explicit terrain/dynamic-tree hook for `Map` when it implements `wow_entities::WorldObjectEnvironment`.
  - `Map` is now passable to `WorldObject` helpers: `map_id`/`instance_id` come from map fields, visibility range reads `visible_distance`, and LOS/map-height/floor queries delegate to `self.terrain` rather than `WorldObject` knowing `wow-map` or scanning object/session state.
  - `NoopTerrainGridLoader` is an explicit noop fallback returning LOS `true` and `INVALID_HEIGHT` heights. This is not real terrain and does not close #NEXT.R8.ENTITIES.005; terrain/vmap/dynamic tree, collision-height/hit-sphere exactness, transports, visibility overrides/cinematic/sight runtime remain pending.

- `#NEXT.R8.ENTITIES.379` (completed; review `APROBADO`; focused validation passed; committed locally by Foreman as `6de2839`)
  - Adds C++-shaped `Map::get_creature_by_spawn_id_like_cpp`, `Map::get_gameobject_by_spawn_id_like_cpp`, and `Map::get_world_object_by_spawn_id_like_cpp(type, spawn_id)` over the existing map-local Creature/GameObject/AreaTrigger by-spawn GUID-set indexes.
  - Source-of-truth runtime store remains `Map::map_objects`; the by-spawn maps remain derived indexes maintained only by `insert_map_object_record`/`remove_map_object`. Getters read the index, deterministically choose an indexed GUID for Rust tests, prefer alive Creature / spawned GameObject like C++, and resolve back through `map_objects`, returning `None` for absent/stale/type-mismatched records or spawn id zero.
  - Matches the C++ switch shape for Creature, GameObject, and AreaTrigger without adding DB load, object creation, respawn timers, `AreaTrigger` respawn support, `PoolMgr`, `DoRespawn`, Create/Load/Update runtime, AddToWorld/RemoveFromWorld side effects, ObjectAccessor/grid/session fanout, or broader Unit/Player systems.

- `#NEXT.R8.ENTITIES.378` (completed; review `APROBADO`; focused validation passed; committed locally by Foreman after validation)
  - Adds typed `MapObjectRecord::AreaTrigger` and `MapObjectRecord::new_area_trigger` while preserving generic `MapObjectRecord::new(AccessorObjectKind::AreaTrigger, WorldObject)` bridge behavior.
  - Adds map-owned derived `area_triggers_by_spawn_id` GUID-set index beside Creature/GameObject indexes, updated only through `insert_map_object_record`/`remove_map_object` replace/remove paths, skipping spawn id zero per C++ `if (_spawnId)` / `IsStaticSpawn()`.
  - Adds C++-shaped AreaTrigger count/GUID/get-by-spawn helpers that resolve through canonical `map_objects` and do not scan the full store.
  - Does not implement AreaTrigger Create/Load/Update runtime, templates/spawns, AI, caster unregister, unit enter/exit, movement/visibility/transport, full entity-specific AddToWorld/RemoveFromWorld side effects outside the store, ObjectAccessor/grid/session fanout, PoolMgr/DoRespawn, or broader Unit/Player systems.

- `#NEXT.R8.ENTITIES.377`
  - Adds map-owned typed multimap-like by-spawn-id indexes for Creature and GameObject live records beside the primary `map_objects` GUID store, matching Trinity's `_creatureBySpawnIdStore` and `_gameobjectBySpawnIdStore` ownership shape.
  - `insert_map_object_record` validates before mutation, unindexes replaced records for the same GUID, indexes the new stored Creature/GameObject when `spawn_id != 0`, and `remove_map_object` prunes empty spawn-id entries. Relocate/remove/reinsert paths inherit this coherence through those helpers.
  - `Map::check_respawn_live_object_guard_like_cpp` now consults the typed spawn-id indexes and then resolves GUIDs through `map_objects`, preserving dead-creature skip, dynamic escort closure exception, GO blocker semantics, timer-zero mutations, missing metadata behavior and unsupported AreaTrigger outcome.
  - Does not implement AreaTrigger by-spawn store, real entity-specific `AddToWorld`/`RemoveFromWorld` side effects beyond `MapObjectRecord`, PoolMgr, DoRespawn/LoadFromDB, DB save/delete, entity creation/fanout, broader ObjectAccessor ownership or real escort runtime.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.376`
  - Executes the C++ `CheckRespawn` linked-respawn future-delay branch inside the map-owned ProcessRespawns timer loop: the original due timer is removed, the same `RespawnInfoLikeCpp` is reinserted at the future linked time, later due timers can still be processed, and world-server queues/executes `CHAR_REP_RESPAWN(type, spawnId, respawnTime, mapId, instanceId)` outside the `MapManager` lock for world maps only.
  - Source-of-truth runtime timers remain map-owned `wow_map::Map` / `RespawnStoreLikeCpp`; `world-server` only bridges the character DB prepared statement and async execution, with non-world maps skipped like C++ `Instanceable()` and invalid map ids skipped without truncation.
  - Does not implement `PoolMgr`, `DoRespawn`/`LoadFromDB`, corpse load, entity creation/fanout, ObjectAccessor/grid ownership, optimized by-spawn indexes, real dynamic escort runtime or broader Unit/Player systems.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.375`
  - Adds the C++ `RemoveRespawnTime(..., true)` / `DeleteRespawnInfoFromDB` side effect for the already-executed safe `ProcessRespawns` zero-delete branches: inactive spawn-group and live object blocker.
  - Source-of-truth runtime timers remain map-owned `wow_map::Map` / `RespawnStoreLikeCpp`; `world-server` only observes before/after timer keys for the same map tick and composes `CharStatements::DEL_RESPAWN` because it owns `CharacterDatabase` and async execution.
  - Synchronization direction is strictly map timer removal -> queued DB delete -> async execute outside the `MapManager` lock. DB failures are logged and do not reinsert timers; non-world maps skip like C++ `Instanceable()`, and invalid `map_id > u16::MAX` skips without truncation.
  - Does not implement future `SaveRespawnInfoDB` reschedules, `DoRespawn`/`LoadFromDB`, PoolMgr, linked-respawn reschedule/persistence, corpse load, entity creation/fanout, ObjectAccessor/grid ownership or optimized by-spawn indexes.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.374`
  - Loads `characters.respawn` once at startup into a read-only snapshot indexed by `MapKey`, validates rows through canonical spawn metadata (`SpawnStore`), computes C++ grid ids from spawn metadata, and applies only Creature/GameObject timers to `ManagedMapKind::World` maps before `InitSpawnGroupState`.
  - Source-of-truth runtime timers remain `wow_map::Map` / `RespawnStoreLikeCpp`; the DB snapshot is startup input only and never writes/deletes DB.
  - Invalid type, AreaTrigger and missing metadata rows are ignored with counters; dungeon/battleground maps skip the snapshot.
  - Does not implement DB save/delete during tick, `DoRespawn`, PoolMgr, linked-respawn persistence, entity live creation/fanout or corpse load.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.373`
  - Adds `Map::process_due_respawns_composite_delete_only_like_cpp`, which uses the composite `Map::check_respawn_like_cpp` over map-owned `RespawnStoreLikeCpp` timers and executes only fully safe in-memory zero-delete effects for inactive spawn-group and live creature/gameobject blockers.
  - `world-server` scheduler now preserves C++ `Map.cpp:682-688` order while passing `SpawnStore`, `LinkedRespawnStoreLikeCpp`, `now`, fixed jitter bridge and explicit false/false dynamic escort bridge into `wow-map`; it does not duplicate `CheckRespawn` logic.
  - Linked-respawn outcomes are detected but counted as blocked pending DB persistence/heap decrease ownership, preserving the original timer; PoolMgr, `DoRespawn`, DB save/delete, unsupported types, missing metadata, entity creation and fanout preserve the oldest due timer/order.
  - Source-of-truth runtime timers remain map-owned `wow_map::Map` / `RespawnStoreLikeCpp`; spawn/linked metadata remain explicit bridges from `world-server`; live blockers read only `Map::map_objects`.
  - Independent review returned `APROBADO`; focused checks passed; committed locally for this slice. No push/install/restart performed.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.372`
  - Adds `Map::check_respawn_like_cpp` and `CheckRespawnCompositeOutcomeLikeCpp` to compose the already represented C++ guards in strict order: spawn-group guard, live-object blocker, linked-respawn guard, then `Allowed`.
  - Source-of-truth runtime timers remain map-owned `wow_map::Map` / `RespawnStoreLikeCpp`; metadata remains the explicit `SpawnStore` bridge; live blockers read `Map::map_objects`; linked respawn metadata remains read-only `LinkedRespawnStoreLikeCpp`.
  - Preserves early-return non-effects: unsupported `AreaTrigger` returns `UnsupportedSpawnType` before mutable guards and keeps `respawn_time` unchanged; earlier blockers prevent linked reschedule.
  - Does not implement full live `CheckRespawn`/`ProcessRespawns`, PoolMgr, `DoRespawn`/`LoadFromDB`, DB save/delete, entity creation, real escort runtime ownership, optimized by-spawn indexes or grid/session fanout.

Previous completed local slice:

- `#NEXT.R8.ENTITIES.371`
  - Adds linked respawn metadata/load/store and the pure linked-time guard dependency for `Map::CheckRespawn`.
  - Source-of-truth runtime timers remain map-owned `wow_map::Map` / `RespawnStoreLikeCpp`; linked respawn metadata is loaded DB -> validated canonical metadata -> read-only linked store.
  - Does not implement full `CheckRespawn`/`ProcessRespawns`, PoolMgr, `DoRespawn`, DB save/delete, live entity creation or fanout.

## C++ Anchors for #386

- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:776-811` and `806-810` — typed `SpawnPool<Pool>`, `SpawnPool<GameObject>`, `SpawnPool<Creature>` and top-level strict order Pool -> GameObject -> Creature.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:891-920` — `UpdatePool<T>` mother-pool branch before typed branch and `UpdatePool(pool_id, type, spawnId)` Creature/GameObject dispatch with unsupported/default abort represented as typed error.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:850-888` — `IsEmpty`/`CheckPool` order GameObject -> Creature -> Pool and first-failure return.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:833-848` — `IsPartOfAPool(type, spawnId)` Creature/GameObject lookup, AreaTrigger returns 0, default abort represented as typed unsupported where Rust can express it.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:288-407` and `305-308` — `PoolGroup<T>::SpawnObject` rolls `rand_chance()` independently and lazily only after `count > 0` for each non-empty explicitly-chanced typed group; `Spawn1Object`/`ReSpawn1Object` live side effects remain represented by existing `PoolSpawnObjectPlanLikeCpp` actions; no `CreateFromDB`, `AddToMap`, `DespawnPool`, DB or fanout in #386.

## Validation for #386

Observed locally after lazy-roll review fix and final review approval:

- `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map pool_mgr`: OK, 13 passed after lazy-roll review fix.
- `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check`: OK.
- `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo PROTOC=/home/cdmonio/.local/protoc/bin/protoc cargo check -p world-server`: OK (warnings only; pre-existing workspace warnings).
- `git diff --check`: OK.

## C++ Anchors for #385

- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:288-350` — `PoolGroup<T>::SpawnObject`: signed `count = limit - spawns.GetSpawnedObjects(poolId)`, trigger increment, explicit-chanced single `rand_chance()` roll before equal fallback, `AddSpawn<T>` before `Spawn1Object`, trigger `ReSpawn1Object`, and final `DespawnObject(triggerFrom)` if trigger remains.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:184-218` — `DespawnObject` removes/despawns only entries currently spawned; represented final-trigger removal mutates map-owned `SpawnedPoolDataLikeCpp` only for spawned trigger entries.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:353-422` — `Spawn1Object`, `ReSpawn1Object`, and `RemoveRespawnTimeFromDB` live side effects remain future owners and are represented only as action records/gaps.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:776-811` — full `PoolMgr::SpawnPool` orchestration order Pool -> GameObject -> Creature remains a gap.
- `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.h:51-83` and `/home/server/woltk-trinity-legacy/src/server/game/Pools/PoolMgr.cpp:39-125` — C++ `SpawnedPoolData` map-owned state shape.

## Validation for #385

Independent review:

- Initial `wow-reviewer`: `CAMBIOS NECESARIOS` because `explicit_roll: Option<f32>` allowed bypassing mandatory C++ explicit `rand_chance()`.
- Correction applied: `spawn_object_plan_like_cpp` now requires `explicit_roll: f32`; no `Option`/`None` path remains, and tests use deterministic rolls.
- Final `wow-reviewer`: `APROBADO`.

Final observed results before local commit, with `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo`:

- `cargo fmt --check`: OK.
- `cargo test -p wow-map pool_group`: OK, 12 passed.
- `PROTOC=/home/cdmonio/.local/protoc/bin/protoc cargo check -p world-server`: OK (warnings only; pre-existing workspace warnings).
- `git diff --check`: OK.
- `git status --short --branch`: dirty only with the five expected #385 files before commit; expected clean/ahead 40 after commit.

## C++ Anchors for #381

- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h:146` — `DEFAULT_COLLISION_HEIGHT = 2.03128f`; not used as a blanket `wow-entities` fallback.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h:496` — base `WorldObject::GetCombatReach() const { return 0.0f; }`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h:557-560` — declarations for `IsWithinLOS`, `IsWithinLOSInMap`, and `GetHitSpherePointFor`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h:748-752` — base `GetCollisionHeight() const { return 0.0f; }` and `GetMapHeight`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1158-1165` — `WorldObject::GetHitSpherePointFor` uses position plus collision height, direction-or-zero toward dest, movement limited by `min(dest.GetExactDist(GetPosition()), GetCombatReach())`, and absolute angle to contact.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1167-1185` — `IsWithinLOS(point)` adds source collision height to target z; player source uses raw position plus collision height; non-player source uses hit sphere; not-in-world returns true.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1187-1210` — `IsWithinLOSInMap(obj)` branch order and endpoint construction, including target-player z using source `GetCollisionHeight()`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.h:1743` and `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.cpp:13571-13596` — `Unit::GetCollisionHeight()` override is DB2/model/mount-derived with default fallback; Rust hydration is external to `wow-entities`.

## Expected Validation for #381

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-entities los
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-entities collision_height
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: #381 completes only canonical LOS endpoint construction from resolved runtime scalar state. It does not close #NEXT.R8.ENTITIES.005; real terrain/vmap/dynamic-tree collision, transports, DB2/model collision-height hydration, visibility overrides/cinematic/sight runtime and broader object ownership remain pending.

## C++ Anchors for #380

- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1167-1185` — `WorldObject::IsWithinLOS` delegates to `GetMap()->isInLineOfSight(...)` when in world, otherwise returns true.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1187-1210` — `WorldObject::IsWithinLOSInMap` requires `IsInMap(obj)` and delegates LOS to map.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1366-1371` — `UpdateGroundPositionZ` uses `GetMapHeight` and applies hover only when height is valid.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:1449-1485` — visibility/sight range use `GetMap()->GetVisibilityRange()` except explicit overrides/far-visible/cinematic/creature sight branches.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp:3749-3769` — `GetFloorZ` maxes static floor with `Map::GetGameObjectFloor`; `GetMapHeight` adds `Z_OFFSET_FIND_HEIGHT` unless `MAX_HEIGHT` and delegates to `Map::GetHeight`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:217` — `Map::GetVisibilityRange` returns `m_VisibleDistance`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:287-290` — `Map::GetHeight` maxes static terrain height with gameobject floor.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:456-464` — `Map::isInLineOfSight` and `GetGameObjectFloor` are map/dynamic-tree ownership.

## Expected Validation for #380

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map world_object_
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: #380 completes only the `Map`↔`WorldObjectEnvironment` hook integration. It does not close #NEXT.R8.ENTITIES.005; real terrain/vmap/dynamic-tree collision, exact collision-height/hit-sphere endpoint adjustment, transport relocation, visibility overrides/cinematic/sight runtime and broader object ownership remain pending.

## C++ Anchors for #379

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:400-430` — typed `GetCreatureBySpawnId`, `GetGameObjectBySpawnId`, `GetAreaTriggerBySpawnId`, and inline `GetWorldObjectBySpawnId` switch over Creature/GameObject/AreaTrigger stores.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3473-3499` — Creature lookup prefers `IsAlive()` then falls back to the first store entry; GameObject lookup prefers `isSpawned()` then falls back to the first store entry.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/GameObject/GameObject.h:251-256` — `isSpawned()` formula: zero respawn delay, positive respawn time for non-default spawned objects, or zero respawn time for default-spawned objects.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3501-3508` — `Map::GetAreaTriggerBySpawnId` uses `equal_range`, returns null if empty, otherwise first store pointer; Rust resolves the indexed GUID through canonical `map_objects` and returns `None` for missing/stale/type-mismatched entries.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:748-777` and `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1954-2023`, `2165-2240` — respawn timers remain Creature/GameObject-only for represented respawn support; this helper does not add AreaTrigger respawn support.

## Expected Validation for #379

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map world_object_by_spawn_id
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map spawn_id_store
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: AreaTrigger Create/Load/Update runtime, templates/spawns, AI, caster unregister, unit enter/exit, movement/visibility/transport, full entity-specific AddToWorld/RemoveFromWorld side effects outside the store, ObjectAccessor/grid/session fanout, PoolMgr/DoRespawn, broader Unit/Player systems.

## C++ Anchors for #378

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:418-430` — `Map` exposes `_objectsStore` and typed by-spawn unordered multimaps for Creature, GameObject and AreaTrigger.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:793-796` — private `_objectsStore`, `_creatureBySpawnIdStore`, `_gameobjectBySpawnIdStore`, `_areaTriggerBySpawnIdStore` fields.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/AreaTrigger/AreaTrigger.cpp:62-76` — `AreaTrigger::AddToWorld` inserts into object store and, when `_spawnId != 0`, into `GetAreaTriggerBySpawnIdStore()` before `WorldObject::AddToWorld()`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/AreaTrigger/AreaTrigger.cpp:78-102` — `AreaTrigger::RemoveFromWorld` runs script/caster/AI/enter-exit side effects, removes from world, erases static spawn-id pair, then removes from object store.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3501-3508` — `Map::GetAreaTriggerBySpawnId` uses `equal_range`, returns null if empty, otherwise returns `bounds.first->second` without spawned filtering.

## Expected Validation for #378

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map area_trigger_spawn_id
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map spawn_id_store
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: AreaTrigger Create/Load/Update runtime, templates/spawns, AI, caster unregister, unit enter/exit, movement/visibility/transport, full entity-specific AddToWorld/RemoveFromWorld side effects outside the store, ObjectAccessor/grid/session fanout, PoolMgr/DoRespawn, broader Unit/Player systems.

## C++ Anchors for #377

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:414-493` — `Map` exposes `_objectsStore` and typed by-spawn unordered multimaps for Creature, GameObject and AreaTrigger.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:793-796` — private `_objectsStore`, `_creatureBySpawnIdStore`, `_gameobjectBySpawnIdStore`, `_areaTriggerBySpawnIdStore` fields; foreman spec also cited nearby respawn-store fields at `Map.h:748-777`.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Creature/Creature.cpp:330-419` — `Creature::AddToWorld` inserts into `GetObjectsStore()` and, when `m_spawnId != 0`, into `GetCreatureBySpawnIdStore()`; `RemoveFromWorld` erases the `(spawnId, this)` pair and removes from object store.
- `/home/server/woltk-trinity-legacy/src/server/game/Entities/GameObject/GameObject.cpp:899-968` — `GameObject::AddToWorld`/`RemoveFromWorld` mirror the same object-store and spawn-id-store lifecycle for gameobjects.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1966-2002` — `Map::CheckRespawn` creature guard iterates `_creatureBySpawnIdStore.equal_range`, skips dead creatures and escorted dynamic escort NPCs, while GO guard checks `_gameobjectBySpawnIdStore.find`; blockers clear `respawnTime=0`.

## Expected Validation for #377

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map spawn_id_store
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map check_respawn_live_object_guard
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: AreaTrigger by-spawn store, real entity-specific `AddToWorld`/`RemoveFromWorld` side effects beyond `MapObjectRecord`, real `PoolMgr`, `DoRespawn`/`LoadFromDB`, corpse load, entity creation/fanout, ObjectAccessor/grid ownership, DB save/delete beyond current seams, real dynamic escort runtime and broader canonical object ownership.

## C++ Anchors for #376

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:682-688` — `Map::Update` calls `ProcessRespawns(); UpdateSpawnGroupConditions();` when `_respawnCheckTimer` fires.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2004-2020` — `Map::CheckRespawn` linked-respawn guard mutates `respawnTime` to the linked creature/gameobject respawn time plus random 5-15 seconds, or `std::numeric_limits<time_t>::max()` for infinite linked delay.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240` — `ProcessRespawns` removes due handle, calls `CheckRespawn`, decreases/reinserts the heap handle for future reschedules, calls `SaveRespawnInfoDB`, and continues so later due timers remain eligible.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3549-3560` — `SaveRespawnInfoDB` no-ops for `Instanceable()` maps and otherwise executes `CHAR_REP_RESPAWN(type, spawnId, respawnTime, mapId, instanceId)`.

## Expected Validation for #376

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map process_respawns
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p world-server respawn_db_save
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p world-server spawn_group_condition_update
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: real `PoolMgr`, `DoRespawn`/`LoadFromDB`, corpse load, entity creation/fanout, ObjectAccessor/grid ownership, optimized by-spawn indexes, real dynamic escort runtime and broader canonical object ownership.

## C++ Anchors for #375

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:682-688` — `Map::Update` calls `ProcessRespawns(); UpdateSpawnGroupConditions();` when `_respawnCheckTimer` fires.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240` — `ProcessRespawns` processes due timers in heap order; zero-delete branches remove timer then call `RemoveRespawnTime(..., true)`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2140-2146` — `RemoveRespawnTime` erases timer maps/heap and calls `DeleteRespawnInfoFromDB`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2152-2163` — `DeleteRespawnInfoFromDB` no-ops for `Instanceable()`, otherwise executes `CHAR_DEL_RESPAWN(type, spawnId, GetId(), GetInstanceId())`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3549-3560` — future `SaveRespawnInfoDB` reschedule persistence remains a gap.

## Expected Validation for #375

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p world-server respawn_db_delete
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p world-server spawn_group_condition_update
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map process_respawns
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
git status --short --branch
```

Expected remaining gaps: `SaveRespawnInfoDB` future reschedule, `DoRespawn`/`LoadFromDB`, real `PoolMgr`, linked-respawn reschedule/persistence, corpse load, entity creation/fanout, ObjectAccessor/grid ownership, optimized by-spawn indexes and broader canonical object ownership.

## C++ Anchors for #374

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/MapManager.cpp:71-76` — `CreateWorldMap` calls `LoadRespawnTimes(); LoadCorpseData(); InitSpawnGroupState();` in that order.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/MapManager.cpp:100-110` — `CreateInstance` calls `LoadRespawnTimes`, but instanceable maps are no-op in `Map::LoadRespawnTimes`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3516-3546` — `SaveRespawnTime` validates metadata, builds `RespawnInfo`, calls `AddRespawnInfo`, and with `startup=true` does not write DB.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3563-3594` — `LoadRespawnTimes` reads respawn rows, validates type/metadata, computes grid id from spawn point, and ignores/logs bad rows.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2057-2090` and `Map.h:748-777` — only creature/gameobject respawn maps accept inserts; duplicates keep the earlier timer.

## Expected Validation for #374

```bash
cargo fmt --check
cargo test -p wow-database respawn_startup_load_statement_reads_all_rows_without_placeholders
cargo test -p world-server persisted_respawn
cargo check -p world-server
git diff --check
```

Expected remaining gaps after #374: DB respawn save/delete during tick, full `DoRespawn`/`LoadFromDB`, real `PoolMgr`, linked-respawn persistence/reschedule, corpse load, entity creation/fanout, optimized by-spawn indexes and broader canonical object ownership.

## C++ Anchors for #373

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:666-688` — scheduler order: `ProcessRespawns(); UpdateSpawnGroupConditions();` when respawn timer fires.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1950-2023` — `Map::CheckRespawn` contract and guard order.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1959-1964` — inactive spawn-group clears `respawnTime=0` and returns false.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1966-2002` — live creature/gameobject blocker clears `respawnTime=0` and returns false.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2004-2020` — linked respawn mutates future/infinite time and returns false; C++ persistence happens later in `ProcessRespawns`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240` — `ProcessRespawns` due-order loop, PoolMgr, DoRespawn, zero-delete and DB-persisting reschedule branches.

## Expected Validation for #373

```bash
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo fmt --check
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map process_respawns
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p wow-map check_respawn_like_cpp
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo test -p world-server spawn_group_condition_update
RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo cargo check -p world-server
git diff --check
```

Expected remaining gaps: real PoolMgr, `DoRespawn`/`LoadFromDB`, DB save/delete/persistence for respawn info, linked-respawn heap decrease plus `SaveRespawnInfoDB`, real dynamic escort config/runtime, entity creation/fanout, optimized by-spawn indexes and broader canonical object ownership.

## C++ Anchors for #372

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1950-2023` — `Map::CheckRespawn` contract and branch order.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1956-1964` — spawn-group inactive branch clears `respawnTime` and returns false.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1966-2002` — live creature/gameobject blocker and dynamic escort exception.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1994-1996` — unsupported/default spawn type abort branch represented defensively as explicit unsupported outcome without timer mutation.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2004-2020` — linked respawn branch after earlier guards allow.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3607-3620` — `Map::GetLinkedRespawnTime` reads map-owned timers via linked GUID.

## Validation for #372

Independent review:

- Initial `wow-reviewer`: `CAMBIOS NECESARIOS` for AreaTrigger + inactive spawn group mutating `respawn_time` before unsupported outcome.
- Correction applied: `AreaTrigger` is rejected at the start of the composite helper before mutable guards; regression test added.
- Final `wow-reviewer`: `APROBADO`.

Final observed results before local commit:

- `cargo test -p wow-map check_respawn_like_cpp`: OK, 8 passed.
- `cargo test -p wow-map linked_respawn`: OK, 6 passed.
- `cargo fmt --check`: OK.
- `cargo check -p world-server`: OK (warnings only; no errors).
- `git diff --check`: OK.

## C++ Anchors for #371

- `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:1811-1997` — `ObjectMgr::LoadLinkedRespawn` SQL, validation and `_linkedRespawnStore` insert.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/SpawnData.h:120-126` — `LinkedRespawnType` values 0..3.
- `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.h:1503-1508` — missing linked GUID returns empty.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:3607-3620` — `Map::GetLinkedRespawnTime`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2004-2020` — linked respawn branch in `Map::CheckRespawn`.
- `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Structure.h:2611-2614` — instanceable map types.

## Validation for #371

Developer/Foreman validation during slice:

```bash
cargo test -p wow-map linked_respawn
cargo test -p world-server linked_respawn
```

Final observed results before local commit:

- `cargo test -p wow-map linked_respawn`: OK, 6 passed.
- `cargo test -p world-server linked_respawn`: OK, 3 passed.
- `cargo fmt --check`: OK.
- `cargo check -p world-server`: OK (warnings only; no errors).
- `git diff --check`: OK.
- Independent `wow-reviewer`: APROBADO.

Recent committed baseline before this slice:

- `#NEXT.R8.ENTITIES.366`
  - Added `wow-map::RespawnStoreLikeCpp` with separate creature/gameobject respawn maps and an ordered queue of `RespawnInfoLikeCpp`.
  - Added `process_due_respawns_like_cpp(now, is_part_of_pool, check_respawn)` planner returning explicit actions for pool update, do-respawn, delete, reschedule/save, and invalid non-future reschedule.
  - Commit before #367: `8595022`.

Current completed slice:

- `#NEXT.R8.ENTITIES.367`
  - `wow-map::Map` now owns a private `RespawnStoreLikeCpp` next to map-owned `SpawnGroupRuntimeState`.
  - Added C++-shaped map wrappers for add/get time/get info/remove/unload/timer-key iteration and planned due-respawn processing.
  - `Map::spawn_grid_load_state_like_cpp(&SpawnStore)` now feeds `SpawnGridLoadStateLikeCpp` from map-owned respawn timer keys plus map-owned spawn-group state while preserving caller-supplied `SpawnStore` metadata as the bridge.
  - Updated R8 inventory/roadmap honestly as complete only for the Map-owned respawn-store integration dependency.

Recent completed slice:

- `#NEXT.R8.ENTITIES.369`
  - Executes only the safe C++ `Map::ProcessRespawns` branch from `Map.cpp:2226-2231` over map-owned timers when the already-represented `CheckRespawn` spawn-group guard from `Map.cpp:1959-1964` clears `respawn_time` to zero for inactive groups.
  - `world-server` now preserves `Map.cpp:682-688` ordering by calling the delete-only `ProcessRespawns` seam before `UpdateSpawnGroupConditions` SetInactive when the scheduler fires.
  - Missing metadata, pool runtime (`pool_id != 0`), active/Allowed `DoRespawn`, DB persistence/delete, linked respawn, by-spawn live stores, entity creation and fanout remain blocked and leave the oldest due timer intact.

Current completed slice:

- `#NEXT.R8.ENTITIES.370`
  - Adds `Map::check_respawn_live_object_guard_like_cpp` for the C++ `Map::CheckRespawn` live-object blocker in `Map.cpp:1966-2002`.
  - Uses canonical map-owned `map_objects` as the local runtime source of truth: alive same-spawn creatures block, dynamic escort groups can be allowed only through explicit config plus caller escort predicate, and any same-spawn gameobject blocks.
  - Missing `SpawnData` and AreaTrigger return explicit outcomes without mutating `respawn_time`; unsupported/default C++ abort is represented without panic.
  - This helper is not yet integrated into full `CheckRespawn`/`ProcessRespawns` and does not implement linked respawn, PoolMgr, DoRespawn/LoadFromDB, DB save/delete, optimized by-spawn indexes, real escort runtime feeding the closure, or fanout.

## C++ Anchors for #370

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1950-2023`
  - `Map::CheckRespawn` contract: return true to spawn, clear `respawnTime=0` to delete, or reschedule future.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1966-2002`
  - Live by-spawn guard: creature equal-range ignores non-alive, optionally ignores escorted dynamic escort NPCs, blocks otherwise; gameobject blocks by any same spawn id; `alreadyExists` clears `respawnTime=0` and returns false.

## C++ Anchors for #369

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:666-688`
  - `Map::Update` calls `ProcessRespawns(); UpdateSpawnGroupConditions();` when `_respawnCheckTimer` expires and resets the timer.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1950-2023`
  - `Map::CheckRespawn` must allow, clear `respawnTime=0`, or reschedule future; the represented first guard clears inactive spawn-group timers.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240`
  - `Map::ProcessRespawns` processes only due timers in order; #369 executes only the zero-respawn-time delete branch and blocks PoolMgr, DoRespawn and future-reschedule branches.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2165-2188`
  - `DoRespawn` creates Creature/GameObject and calls `LoadFromDB`; still out of scope.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2152-2163`
  - `DeleteRespawnInfoFromDB` deletes DB state; still out of scope.

## C++ Anchors for #367

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:472-480`
  - `Map::GetRespawnTime` reads the map-owned respawn map and returns `0` when missing or when the type has no respawn map.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:748-777`
  - `Map` owns `_respawnTimes`, `_creatureRespawnTimesBySpawnId`, `_gameObjectRespawnTimesBySpawnId`; `SPAWN_TYPE_AREATRIGGER` returns `nullptr`.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2057-2090`
  - `AddRespawnInfo` rejects spawn id `0`/no map, replaces when new `respawnTime <= existing`, rejects later duplicates, and inserts heap plus map.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2107-2150`
  - `GetRespawnInfo`, `UnloadAllRespawnInfos`, and `DeleteRespawnInfo` keep queue/map coherent; DB delete remains external.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240`
  - `ProcessRespawns` consumes the map-owned store; PoolMgr, DoRespawn, DB persistence/delete and real entity creation remain side effects outside this slice.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2286-2305`
  - `ShouldBeSpawnedOnGridLoad` checks `GetRespawnTime(type, spawnId) != 0` before spawn-group and pool checks.

## Validation for #370

Independent review:

- `wow-reviewer`: `APROBADO`.

Checks run with `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo`:

```bash
cargo fmt --check
cargo test -p wow-map check_respawn_live_object_guard
cargo check -p world-server
git diff --check
git status --short --branch
```

Results:

- `cargo fmt --check`: OK.
- `cargo test -p wow-map check_respawn_live_object_guard`: 6 passed.
- `cargo check -p world-server`: OK.
- `git diff --check`: OK.
- `git status --short --branch`: dirty only with the four expected #370 files before commit; expected clean/ahead 24 after commit.

Warnings observed are pre-existing workspace warnings (for example `unsafe` in `wow-core/src/guid.rs`, unused imports/variables in existing crates, and existing `world-server` warnings); they are not introduced by #370.

## Validation for #369

Independent review:

- `wow-reviewer`: `APROBADO`.

Checks run with `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo`:

```bash
cargo fmt --check
cargo test -p wow-map process_respawns
cargo test -p world-server spawn_group_condition_update
cargo check -p world-server
git diff --check
git status --short --branch
```

Focused tests cover positive inactive-spawn-group timer deletion, active/Allowed timer preservation, missing metadata preservation, pool timer preservation, and C++ order preservation when the first due timer blocks.

Results:

- `cargo fmt --check`: OK.
- `cargo test -p wow-map process_respawns`: 12 passed.
- `cargo test -p world-server spawn_group_condition_update`: 7 passed.
- `cargo check -p world-server`: OK.
- `git diff --check`: OK.

Warnings observed are pre-existing workspace warnings (for example `unsafe` in `wow-core/src/guid.rs`, unused imports/variables in existing crates, and the existing `spawn_group_templates` dead-code warning in `world-server`).

## Validation for #367

Independent review:

- `wow-reviewer`: `APROBADO`.

Checks run with `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo`:

```bash
cargo fmt --check
cargo test -p wow-map map_owned_respawn
cargo test -p wow-map process_respawns
cargo test -p wow-map do_for_all_maps_mut
cargo check -p world-server
git diff --check
git status --short --branch
```

Results:

- `map_owned_respawn`: 4 passed.
- `process_respawns`: 7 passed.
- `do_for_all_maps_mut`: 1 passed.
- `cargo check -p world-server`: OK.
- `git diff --check`: OK.

Warnings observed are pre-existing workspace warnings (for example `unsafe` in `wow-core/src/guid.rs`, unused imports/variables in existing crates); they are not introduced by #367.

## Remaining Gaps / Next Dependency

`#NEXT.R8.ENTITIES.386` adds only the deterministic `PoolMgr::SpawnPool`/`UpdatePool` orchestration plan seam; it does **not** complete full live respawn/runtime ownership. Remaining heavy dependencies toward >95% core include:

1. Wire real live `PoolMgr` ownership/execution from the deterministic plan: `LoadFromDB`, auto-spawn map init, live `SpawnPool`/`DespawnPool`, recursive subpool execution, and map-owned chance/RNG integration.
2. Expand live `ProcessRespawns` beyond represented/delete-only branches without faking PoolMgr/DB/entity side effects.
3. Complete C++-shaped `CheckRespawn` beyond the represented spawn-group guard: real map-local creature/gameobject by-spawn stores, escort exceptions, and linked respawn data.
4. Implement `DoRespawn` entity creation/`LoadFromDB`, `AddToMap`/`RemoveFromMap`, DB respawn persistence/delete, and grid/session fanout.
5. Add real map-local creature/gameobject by-spawn stores and ObjectAccessor-like ownership instead of session-local fallback state.
6. Continue reducing Player/Unit/Creature/GameObject lifecycle, UpdateFields, inventory/equipment, auras, threat, motion, spawn/despawn/respawn gaps.

Recommended next slice: use #386 as the seam and implement the next bounded live `PoolMgr`/`DoRespawn` ownership dependency only where the source-of-truth owner exists (for example map-owned by-spawn stores or explicit live `Spawn1Object`/`ReSpawn1Object`/`Despawn1Object` action execution). Do not turn blocked Allowed/pool/reschedule branches into timer deletion without real side effects.
