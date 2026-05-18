# Current Session Handoff

Generated: 2026-05-18

Continuity snapshot for RustyCore C++ -> Rust migration in `/home/server/rustycore`.

## Repository State

- Branch: `develop`
- Base before #374: clean `develop...origin/develop [ahead 27]` after `#NEXT.R8.ENTITIES.373`.
- Most recent completed slice: `#NEXT.R8.ENTITIES.374 — startup LoadRespawnTimes -> map-owned respawn store`.
- Expected tree after committing #374: clean, ahead 28. No push/install/restart performed.

## Critical Rules

- Source of truth is TrinityCore C++ 3.4.3 build 12340/TDB442 in `/home/server/woltk-trinity-legacy`.
- Do not use Retail, Cataclysm, 3.3.5, C#, or CypherCore as authority.
- Do not trust existing Rust, tests, comments, prior AI work, or roadmap checkboxes without C++ anchors.
- No live server start, install, restart, push, or merge unless Joe explicitly requests it.
- Commit locally only after independent review plus validation.

## Progress Estimate

Overall core migration estimate after #374 `startup LoadRespawnTimes -> map-owned respawn store`: `~86.2%`.

This remains intentionally below the R8 TSV row-completion ratio because heavy runtime ownership gaps remain: full live `CheckRespawn`/`ProcessRespawns` side effects beyond represented safe zero-delete branches, real `PoolMgr`, `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete during tick, corpse load, optimized map-local by-spawn indexes, real dynamic escort config/runtime feeding the closure, grid/session fanout, ObjectAccessor ownership, and broader Unit/Player inventory/auras/threat/motion/update-field work.

Manual test point: no new client-facing manual milestone from #374; this is a startup map-owned respawn timer load dependency validated with focused unit checks.

## Most Recent Completed Slices

Current completed local slice:

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

Expected remaining gaps: DB respawn save/delete during tick, full `DoRespawn`/`LoadFromDB`, real `PoolMgr`, linked-respawn persistence/reschedule, corpse load, entity creation/fanout, optimized by-spawn indexes and broader canonical object ownership.

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

`#NEXT.R8.ENTITIES.367` plus #368/#369 do **not** complete full live respawn runtime. Remaining heavy dependencies toward >95% core include:

1. Expand live `ProcessRespawns` beyond the delete-only inactive-spawn-group branch without faking PoolMgr/DB/entity side effects.
2. Complete C++-shaped `CheckRespawn` beyond the represented spawn-group guard: real map-local creature/gameobject by-spawn stores, escort exceptions, and linked respawn data.
3. Implement real `PoolMgr`/pool selection state or explicitly block pool branches until its source-of-truth owner exists.
4. Implement `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete, and grid/session fanout.
5. Add real map-local creature/gameobject by-spawn stores and ObjectAccessor-like ownership instead of session-local fallback state.
6. Continue reducing Player/Unit/Creature/GameObject lifecycle, UpdateFields, inventory/equipment, auras, threat, motion, spawn/despawn/respawn gaps.

Recommended next slice: complete another bounded `ProcessRespawns`/`CheckRespawn` dependency only when its source-of-truth owner exists (for example by-spawn live stores or linked-respawn data), or implement explicit PoolMgr/DoRespawn ownership. Do not turn the blocked Allowed/pool/reschedule branches into timer deletion without real side effects.
