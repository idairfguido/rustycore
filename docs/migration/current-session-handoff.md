# Current Session Handoff

Generated: 2026-05-18

Continuity snapshot for RustyCore C++ -> Rust migration in `/home/server/rustycore`.

## Repository State

- Branch: `develop`
- Base before this slice: `64d42b1 #NEXT.R8.ENTITIES.367 map owned respawn store integration`
- Local branch relation before committing #368: `develop...origin/develop [ahead 21]`
- Most recent completed slice after this commit: `#NEXT.R8.ENTITIES.369 — ProcessRespawns delete-only safe execution seam`
- Expected tree after committing #369: clean, ahead 23. No push/install/restart performed.

## Critical Rules

- Source of truth is TrinityCore C++ 3.4.3 build 12340/TDB442 in `/home/server/woltk-trinity-legacy`.
- Do not use Retail, Cataclysm, 3.3.5, C#, or CypherCore as authority.
- Do not trust existing Rust, tests, comments, prior AI work, or roadmap checkboxes without C++ anchors.
- No live server start, install, restart, push, or merge unless Joe explicitly requests it.
- Commit locally only after independent review plus validation.

## Progress Estimate

Overall core migration estimate after #369 delete-only ProcessRespawns seam: `~84.8%`.

This remains intentionally below the R8 TSV row-completion ratio because heavy runtime ownership gaps remain: full live `ProcessRespawns` execution beyond the safe inactive-spawn-group delete-only seam, real `PoolMgr`, `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete, linked respawn checks, real map-local by-spawn creature/gameobject stores, grid/session fanout, ObjectAccessor ownership, and broader Unit/Player inventory/auras/threat/motion/update-field work.

Manual test point: no new client-facing manual milestone from #369; this is a map-owned respawn/check-respawn dependency validated with focused unit/integration checks.

## Most Recent Completed Slices

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

Current slice:

- `#NEXT.R8.ENTITIES.369`
  - Executes only the safe C++ `Map::ProcessRespawns` branch from `Map.cpp:2226-2231` over map-owned timers when the already-represented `CheckRespawn` spawn-group guard from `Map.cpp:1959-1964` clears `respawn_time` to zero for inactive groups.
  - `world-server` now preserves `Map.cpp:682-688` ordering by calling the delete-only `ProcessRespawns` seam before `UpdateSpawnGroupConditions` SetInactive when the scheduler fires.
  - Missing metadata, pool runtime (`pool_id != 0`), active/Allowed `DoRespawn`, DB persistence/delete, linked respawn, by-spawn live stores, entity creation and fanout remain blocked and leave the oldest due timer intact.

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
