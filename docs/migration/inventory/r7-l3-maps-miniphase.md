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

## Follow-Up Work Items

- [ ] **#NEXT.L3.MAPS.005** Port grid unload helpers (`ObjectGridStoper`, `ObjectGridEvacuator`, `ObjectGridCleaner`, `ObjectGridUnloader`) against real entity lifecycle once entities exist.
- [ ] **#NEXT.L3.MAPS.006** Replace legacy `wow-world/src/map_manager.rs` only after the new `wow-map` skeleton owns grid lifecycle.
