# R7 L3 Maps Mini-Phase

> Generated: 2026-05-07
> Rule: every Maps claim is contrasted against `/home/server/woltk-trinity-legacy/src/server/game/Grids/` and `/home/server/woltk-trinity-legacy/src/server/game/Maps/`.

## Closed Tasks

- [x] **#NEXT.L3.MAPS.001** Port `GridInfo`, `NGrid` and grid-state transitions.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Grids/NGrid.h`, `NGrid.cpp`, `GridStates.cpp`, `GridStates.h`.
  Rust targets: `crates/wow-map/src/grid.rs`, `crates/wow-map/src/lib.rs`.
  Acceptance: `GridStateKind` numeric values match C++; `GridInfo` `TimeTracker`/relocation timer/unload-lock semantics are covered by tests; `NGrid` owns 8x8 cells, grid id, x/y, loaded flag and state; Active/Idle/Removal transitions are represented behind a small `MapGridHost` trait so full `Map` can consume them without reimplementing state logic.

## Follow-Up Work Items

- [ ] **#NEXT.L3.MAPS.002** Port `Map` skeleton: `i_grids[64][64]`, create/load/unload hooks, expiry reset and terrain hook boundary.
- [ ] **#NEXT.L3.MAPS.003** Port `ObjectGridLoader::LoadN` on top of `SpawnStore` + `NGrid`.
- [ ] **#NEXT.L3.MAPS.004** Replace legacy `wow-world/src/map_manager.rs` only after the new `wow-map` skeleton owns grid lifecycle.
