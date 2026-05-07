# R7 L3 Maps Mini-Phase

> Generated: 2026-05-07
> Rule: every Maps claim is contrasted against `/home/server/woltk-trinity-legacy/src/server/game/Grids/` and `/home/server/woltk-trinity-legacy/src/server/game/Maps/`.

## Closed Tasks

- [x] **#NEXT.L3.MAPS.001** Port `GridInfo`, `NGrid` and grid-state transitions.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Grids/NGrid.h`, `NGrid.cpp`, `GridStates.cpp`, `GridStates.h`.
  Rust targets: `crates/wow-map/src/grid.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: `GridStateKind` numeric values match C++; `GridInfo` `TimeTracker`/relocation timer/unload-lock semantics are covered by tests; `NGrid` owns 8x8 cells, grid id, x/y, loaded flag and state; Active/Idle/Removal transitions are represented behind a small `MapGridHost` trait so full `Map` can consume them without reimplementing state logic.

- [x] **#NEXT.L3.MAPS.002** Port `Map` grid lifecycle skeleton.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.h`, `Map.cpp`.
  Rust targets: `crates/wow-map/src/map.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: `Map` owns a 64x64 pointer-equivalent grid table; constructor starts with empty grid slots; `EnsureGridCreated` creates an idle `NGrid` and calls terrain load with reversed coords; `EnsureGridLoaded` marks data loaded before invoking the loader hook; active-object loading sets `GRID_STATE_ACTIVE` and 0.1 expiry; `UnloadGrid` refuses world creatures/near active objects unless forced and invokes object/terrain unload hooks.

- [x] **#NEXT.L3.MAPS.003** Port `ObjectGridLoader::LoadN` GUID/container pass.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Grids/ObjectGridLoader.h`, `ObjectGridLoader.cpp`.
  Rust targets: `crates/wow-map/src/object_grid_loader.rs`, `crates/wow-map/src/spawn.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: loader iterates all 8x8 cells, reads creature/gameobject/areatrigger IDs from `SpawnStore`, applies a `ShouldBeSpawnedOnGridLoad`-equivalent filter, materializes stable `ObjectGuid`s, loads world/grid corpses from a map corpse store, and can be wired through `Map::EnsureGridLoaded` via `SpawnGridLifecycle`.

- [x] **#NEXT.L3.MAPS.004** Port personal phase grid loading/tracking.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Phasing/PersonalPhaseTracker.h`, `PersonalPhaseTracker.cpp`, `ObjectGridLoader.cpp`.
  Rust targets: `crates/wow-map/src/personal_phase.rs`, `crates/wow-map/src/object_grid_loader.rs`, `crates/wow-map/src/map.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: player phase shifts load personal creature/gameobject spawns only for personal phases with matching spawn sets; grids are not reloaded for the same owner/phase/grid; unload removes grid tracking; phase changes and explicit deletion use the C++ one-minute delete timer.

- [x] **#NEXT.L3.MAPS.005** Port grid unload helper traversal/order as GUID actions.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Grids/ObjectGridLoader.h`, `ObjectGridLoader.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp::UnloadGrid`.
  Rust targets: `crates/wow-map/src/grid_unload.rs`, `crates/wow-map/src/grid.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: `ObjectGridStoper` emits creature-only dynobject/areatrigger/combat-stop actions; `ObjectGridEvacuator` emits creature/GO respawn-relocation actions; `ObjectGridCleaner` emits destroyed+cleanup actions for grid object containers; `ObjectGridUnloader` deletes non-corpse grid objects and clears grid object sets in C++ order.

- [x] **#NEXT.L3.MAPS.007** Port `MapManager` structural skeleton.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Maps/MapManager.h`, `MapManager.cpp`.
  Rust targets: `crates/wow-map/src/manager.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: ordered `(map_id, instance_id)` map store; find/range iteration; delay clamps; serial update/delayed-update order; destroy/unload and reusable instance id allocator semantics; instance/player statistics and scheduled script counter.

- [x] **#NEXT.L3.MAPS.010** Port `MapUpdater` API and deterministic fallback path.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Maps/MapUpdater.h`, `MapUpdater.cpp`, `MapManager.cpp`.
  Rust targets: `crates/wow-map/src/manager.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: `activate`, `deactivate`, `activated`, `schedule_update` and `wait` mirror the C++ control-flow contract; activated `MapManager::update` schedules maps through `MapUpdater` and waits before delayed cleanup; current Rust execution remains inline/deterministic until a safe worker-pool ownership model exists.

## Follow-Up Work Items

- [ ] **#NEXT.L3.MAPS.006** Bind grid unload actions to real entity lifecycle once canonical entities exist.
- [ ] **#NEXT.L3.MAPS.008** Bind `MapManager::CreateMap(uint32, Player*)` to real Player/Group/InstanceLock/Battleground/DB2 models.
- [ ] **#NEXT.L3.MAPS.009** Replace legacy `wow-world/src/map_manager.rs` only after the new `wow-map` skeleton owns grid lifecycle.
- [ ] **#NEXT.L3.MAPS.011** Replace `MapUpdater` inline fallback with a real worker pool once map ownership can safely match C++ parallel requests.
