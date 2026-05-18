# Current Session Handoff

Generated: 2026-05-18

Continuity snapshot for RustyCore C++ -> Rust migration in `/home/server/rustycore`.

## Repository State

- Branch: `develop`
- Base before this slice: `64d42b1 #NEXT.R8.ENTITIES.367 map owned respawn store integration`
- Local branch relation before committing #368: `develop...origin/develop [ahead 21]`
- Most recent completed slice after this commit: `#NEXT.R8.ENTITIES.368 — Represented Map::CheckRespawn spawn-group guard`
- Expected tree after committing #368: clean, ahead 22. No push/install/restart performed.

## Critical Rules

- Source of truth is TrinityCore C++ 3.4.3 build 12340/TDB442 in `/home/server/woltk-trinity-legacy`.
- Do not use Retail, Cataclysm, 3.3.5, C#, or CypherCore as authority.
- Do not trust existing Rust, tests, comments, prior AI work, or roadmap checkboxes without C++ anchors.
- No live server start, install, restart, push, or merge unless Joe explicitly requests it.
- Commit locally only after independent review plus validation.

## Progress Estimate

Overall core migration estimate after #367 plus current uncommitted #368 dependency slice: `~84.6%`.

This remains intentionally below the R8 TSV row-completion ratio because heavy runtime ownership gaps remain: live `ProcessRespawns` execution, real `PoolMgr`, `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete, linked respawn checks, real map-local by-spawn creature/gameobject stores, grid/session fanout, ObjectAccessor ownership, and broader Unit/Player inventory/auras/threat/motion/update-field work.

Manual test point: no new client-facing manual milestone from #368; this is a map-owned respawn/check-respawn dependency validated with focused unit/integration checks.

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

- `#NEXT.R8.ENTITIES.368`
  - Represents only the first `Map::CheckRespawn` spawn-group guard from `Map.cpp:1956-1964`: caller-supplied `SpawnStore` metadata -> map-owned `SpawnGroupRuntimeState` -> mutate `RespawnInfoLikeCpp::respawn_time = 0` for inactive groups.
  - Missing `SpawnData` is explicit `MissingSpawnData` fallback with no mutation; C++ would assert, but RustyCore still uses a caller-supplied metadata bridge.
  - Full `Map::CheckRespawn`, live `ProcessRespawns`, by-spawn live existence checks, escort exceptions, linked respawn, PoolMgr, DoRespawn and DB persistence/delete remain partial/pending.

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

`#NEXT.R8.ENTITIES.367` plus the current #368 guard dependency do **not** complete full live respawn runtime. Remaining heavy dependencies toward >95% core include:

1. Wire live `ProcessRespawns` scheduler execution from `MapManager`/world-server using the map-owned #367 store, without faking PoolMgr/DB/entity side effects.
2. Complete C++-shaped `CheckRespawn` beyond the represented spawn-group guard: real map-local creature/gameobject by-spawn stores, escort exceptions, and linked respawn data.
3. Implement real `PoolMgr`/pool selection state or explicitly block pool branches until its source-of-truth owner exists.
4. Implement `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete, and grid/session fanout.
5. Add real map-local creature/gameobject by-spawn stores and ObjectAccessor-like ownership instead of session-local fallback state.
6. Continue reducing Player/Unit/Creature/GameObject lifecycle, UpdateFields, inventory/equipment, auras, threat, motion, spawn/despawn/respawn gaps.

Recommended next slice: live `ProcessRespawns` execution seam over map-owned timers that can safely execute only the currently complete branches (or remains planned-only where PoolMgr/DoRespawn/DB/linked-respawn are missing), with explicit blockers for unsafe side effects rather than fake runtime.
