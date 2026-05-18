# Current Session Handoff

Generated: 2026-05-18

Continuity snapshot for RustyCore C++ -> Rust migration in `/home/server/rustycore`.

## Repository State

- Branch: `develop`
- Base before this slice: `7f404a3 #NEXT.R8.ENTITIES.365 gate spawn condition scheduler on effective map updates`
- Local branch relation before committing #366: `develop...origin/develop [ahead 19]`
- Slice in progress at this handoff update: `#NEXT.R8.ENTITIES.366 — Map-owned respawn timer store + ProcessRespawns planner dependency`
- Expected tree after local commit: clean, ahead 20. No push/install/restart performed.

## Critical Rules

- Source of truth is TrinityCore C++ 3.4.3 build 12340/TDB442 in `/home/server/woltk-trinity-legacy`.
- Do not use Retail, Cataclysm, 3.3.5, C#, or CypherCore as authority.
- Do not trust existing Rust, tests, comments, prior AI work, or roadmap checkboxes without C++ anchors.
- No live server start, install, restart, push, or merge unless Joe explicitly requests it.
- Commit locally only after independent review plus validation.

## Progress Estimate

Overall core migration estimate after #366: `~84%`.

This is intentionally below the R8 TSV row-completion ratio because heavy runtime ownership gaps remain: live `ProcessRespawns`, real `PoolMgr`, `DoRespawn` entity creation/`LoadFromDB`, DB respawn persistence/delete, linked respawn checks, real map-local by-spawn creature/gameobject stores, grid/session fanout, object accessor ownership, and broader Unit/Player inventory/auras/threat/motion/update-field work.

Manual test point: no new client-facing manual milestone from #366; this is a map-owned store/planner dependency validated with focused unit/integration checks.

## Most Recent Completed Slices

Recent committed baseline before this slice:

- `#NEXT.R8.ENTITIES.365`
  - `MapManager` spawn-condition scheduler gates C++ `_respawnCheckTimer`-like updates on effective map updates and calls SetInactive-safe spawn group condition logic.
  - Commit before #366: `7f404a3`.

Current slice prepared for commit:

- `#NEXT.R8.ENTITIES.366`
  - Adds `wow-map::RespawnStoreLikeCpp` with separate creature/gameobject respawn maps and an ordered queue of `RespawnInfoLikeCpp`.
  - Adds `process_due_respawns_like_cpp(now, is_part_of_pool, check_respawn)` planner returning explicit actions for pool update, do-respawn, delete, reschedule/save, and invalid non-future reschedule.
  - Reexports the respawn store/planner types from `wow-map`.
  - Updates R8 inventory/roadmap honestly as complete only for the store/planner dependency.

## C++ Anchors for #366

- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:161-180`
  - `RespawnInfo { type, spawnId, entry, respawnTime, gridId }` and `CompareRespawnInfo` ordering.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:472-480`
  - `GetRespawnTime` returns `0` for missing entries or types without respawn map.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h:748-777`
  - Map-owned `_respawnTimes`, `_creatureRespawnTimesBySpawnId`, `_gameObjectRespawnTimesBySpawnId`; AreaTrigger has no respawn map.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2057-2090`
  - `AddRespawnInfo`: reject zero spawn id/no map; replace when new time `<=` existing; reject later duplicate; insert heap and map.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2107-2150`
  - `GetRespawnInfo`, `UnloadAllRespawnInfos`, `DeleteRespawnInfo`; heap/map coherency, DB delete external.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2191-2240`
  - `ProcessRespawns`: process only due top; pool branch before `CheckRespawn`; then `DoRespawn`, delete, or reschedule/save.
- `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:1950-2023`
  - `CheckRespawn` contract: allowed, zero/delete, or strictly future reschedule.

## Validation for #366

Independent review:

- `wow-reviewer`: `APROBADO`.

Checks run with `RUSTUP_HOME=/home/cdmonio/.rustup CARGO_HOME=/home/cdmonio/.cargo`:

```bash
cargo fmt --check
cargo test -p wow-map respawn_info
cargo test -p wow-map process_respawns
cargo test -p wow-map do_for_all_maps_mut
cargo test -p world-server spawn_group_condition_update
cargo check -p world-server
git diff --check
git status --short --branch
```

Results:

- `respawn_info`: 3 passed.
- `process_respawns`: 7 passed.
- `do_for_all_maps_mut`: 1 passed.
- `spawn_group_condition_update`: 5 passed.
- `cargo check -p world-server`: OK.
- `git diff --check`: OK.

Warnings observed are pre-existing workspace warnings (for example `unsafe` in `wow-core/src/guid.rs`, unused imports/variables in existing crates); they are not introduced by #366.

## Remaining Gaps / Next Dependency

`#NEXT.R8.ENTITIES.366` does **not** complete full live respawn runtime. Remaining heavy dependencies toward >95% core include:

1. Wire map-owned respawn store into `Map`/`MapManager` as canonical runtime state.
2. Implement C++-shaped `CheckRespawn` against spawn group activity, real map-local creature/gameobject by-spawn stores, escort exceptions, and linked respawn data.
3. Implement live `ProcessRespawns` scheduler execution using the #366 planner actions without faking `PoolMgr`, `DoRespawn`, DB persistence/delete, or `LoadFromDB`.
4. Add real map-local creature/gameobject by-spawn stores and ObjectAccessor-like ownership instead of session-local fallback state.
5. Continue reducing Player/Unit/Creature/GameObject lifecycle, UpdateFields, inventory/equipment, auras, threat, motion, spawn/despawn/respawn gaps.

Recommended next slice: map-owned `RespawnStoreLikeCpp` integration into `Map`/`MapManager` wrappers and timer-key exposure for canonical spawn/grid load consumers, still without faking live DB/pool/entity creation.
