# Migration: Grids

> **C++ canonical path:** `src/server/game/Grids/`
> **Rust target crate(s):** `crates/wow-world/` (shared with Maps)
> **Layer:** L3 (World layer, substrate for Maps)
> **Status:** 🔧 broken (missing core state machine)
> **Audited vs C++:** ⚠️ partial (architecture divergence: no NGrid/Cell hierarchy, no GridStates)
> **Last updated:** 2026-05-01

---

## 1. Purpose

Grids is the spatial partitioning system that subdivides a Map into a 64x64 grid of NGrids, each containing an 8x8 array of Cells. Grids implement lazy loading: spawns are loaded only when a grid transitions to ACTIVE state (player or active object nearby), and unloaded when entering REMOVAL state after idling. This architecture enables efficient entity tracking, visibility queries, and memory management across large open worlds.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/src/server/game/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `Grids/GridDefines.h` | 251 | Global grid constants, typedefs, bitmasks for object types |
| `Grids/Grid.h` | 142 | Generic Grid<T> template for cell-level entity containers |
| `Grids/NGrid.h` | 183 | NGrid template wrapper: 8x8 cell grid with state + timers |
| `Grids/GridInfo.h` (part of NGrid.h) | ~50 | GridInfo metadata: unload timers, lock counters, visibility tracker |
| `Grids/GridStates.h` | 56 | GridState abstract base class + InvalidState, ActiveState, IdleState, RemovalState |
| `Grids/GridStates.cpp` | 65 | State machine implementations: Update() logic for each state |
| `Grids/NGrid.cpp` | 36 | NGrid ctor/dtor, minimal impl (mostly inline in .h) |
| `Grids/GridReference.h` | 51 | Intrusive reference node for doubly-linked list of entities in cell |
| `Grids/GridRefManager.h` | 38 | Manager for GridReference nodes (intrusive container) |
| `Grids/GridLoader.h` | 76 | GridLoader<T> template interface for loading spawns into grid |
| `Grids/ObjectGridLoader.h` | 126 | ObjectGridLoader + PersonalPhaseGridLoader + ObjectGridStoper |
| `Grids/ObjectGridLoader.cpp` | 283 | ObjectGridLoader::Visit(CreatureMapType), Visit(GameObjectMapType), LoadN() impl |
| `Grids/Cells/CellImpl.h` | (directory) | Cell visitor implementations (GridNotifiersImpl) |
| `Grids/Dynamic/DynamicTreeImpl.h` | (directory) | Dynamic object spatial tree (visibility queries) |
| `Grids/Notifiers/GridNotifiers.h` | (directory) | Visitor patterns for cell iteration |

**Total C++ lines in Grids/:** ~1,307 (excluding subdirectories)

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Grid<ACTIVE_OBJECT, WORLD_OBJECT_TYPES, GRID_OBJECT_TYPES>` | template class | Single cell container; holds world + grid objects via TypeMapContainer |
| `NGrid<N, ACTIVE_OBJECT, WORLD_OBJECT_TYPES, GRID_OBJECT_TYPES>` | template class | NxN grid of Cells; state machine, timers, object data loaded flag |
| `GridInfo` | class | Metadata for NGrid: time tracker, unload lock counters, visibility update timer |
| `GridState` | class (abstract) | Base class for grid lifecycle state machine |
| `InvalidState` | class | Grid state: uninitialized, no load/unload actions |
| `ActiveState` | class | Grid state: active (player nearby), periodic idle check, entities updated |
| `IdleState` | class | Grid state: idle (no players), transition to REMOVAL on timeout |
| `RemovalState` | class | Grid state: scheduled for unload, despawn entities after delay |
| `GridReference<SPECIFIC_OBJECT>` | template struct | Intrusive doubly-linked list node for entity tracking |
| `GridRefManager<TYPE>` | template class | Doubly-linked list container for GridReference nodes |
| `GridLoader<ACTIVE_OBJECT, WORLD_OBJECT_TYPES, GRID_OBJECT_TYPES>` | template class | Abstract loader interface; subclassed by ObjectGridLoader |
| `ObjectGridLoaderBase` | class | Base for creature/GameObject loaders; tracks load counts |
| `ObjectGridLoader` | class | Loads creatures + GameObjects from DB on grid load |
| `PersonalPhaseGridLoader` | class | Loads phase-specific spawns (personal instances) |
| `ObjectGridStoper` | class | Visitor to despawn creatures before grid unload |
| `grid_state_t` | enum | Grid lifecycle: INVALID, ACTIVE, IDLE, REMOVAL (4 states) |
| `GridMapTypeMask` | enum | Bitmask for filtering entity types by mask |
| `AllWorldObjectTypes` | typedef | TYPELIST_4: Player, Creature, Corpse, DynamicObject |
| `AllGridObjectTypes` | typedef | TYPELIST_7: GameObject, Creature, DynamicObject, Corpse, AreaTrigger, SceneObject, Conversation |
| `AllMapStoredObjectTypes` | typedef | TYPELIST_8: All grid + world object types |
| `TypeMapContainer<T>` | template | Heterogeneous container for typed object lists |
| `NGridType` | typedef | NGrid<8, Player, AllWorldObjectTypes, AllGridObjectTypes> (concrete instance) |
| `GridType` | typedef | Grid<Player, AllWorldObjectTypes, AllGridObjectTypes> (concrete instance) |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `NGrid::NGrid(id, x, y, expiry, unload)` | Constructor; initializes 8x8 cell array, state INVALID, timers | GridInfo ctor |
| `NGrid::GetGridType(x, y)` | Retrieve Cell at position (x, y) within grid (0-7 range) | Cell methods |
| `NGrid::GetGridState()` | Query current lifecycle state (INVALID/ACTIVE/IDLE/REMOVAL) | State machine |
| `NGrid::SetGridState(state)` | Transition to new state | State machine transition |
| `NGrid::GetGridId()` | Return unique grid ID (map-local) | Entity relocation, respawn tracking |
| `NGrid::getX(), getY()` | Get 2D grid coordinates (0-63 in 64x64 map grid) | Entity relocation |
| `NGrid::isGridObjectDataLoaded()` | Query if spawns have been loaded from DB | ObjectGridLoader |
| `GridInfo::getTimeTracker()` | Query current timer for state expiry | State machine Update() |
| `GridInfo::getUnloadLock()` | Check if unload is blocked (has active locks or explicit lock) | Unload prevention |
| `GridInfo::incUnloadActiveLock(), decUnloadActiveLock()` | Increment/decrement active spawn point locks | Creature spawn, active object tracking |
| `GridInfo::setUnloadExplicitLock(bool)` | Manually lock/unlock grid from unloading | Admin commands, instance locks |
| `GridInfo::ResetTimeTracker(interval)` | Reset timer to interval (called on state transition) | State machine |
| `GridInfo::UpdateTimeTracker(diff)` | Tick timer by diff_ms | GridState::Update() |
| `Grid::AddWorldObject(SPECIFIC_OBJECT*)` | Add world object to cell (Player, Corpse, etc.) | Map::AddToMap |
| `Grid::AddGridObject(SPECIFIC_OBJECT*)` | Add grid object to cell (Creature, GameObject, etc.) | Map::AddToMap |
| `Grid::Visit<T>(visitor)` | Iterate grid or world objects with visitor pattern | Spell AoE, threat updates |
| `Grid::GetWorldObjectCountInGrid<T>()` | Count objects of type T in cell | Activity checks |
| `GridReference<T>::link(mgr, obj)` | Create intrusive link for object in container | GridRefManager |
| `GridRefManager<T>::isEmpty()` | Check if container is empty | Activity checks |
| `ObjectGridLoader::Visit(CreatureMapType&)` | Load creatures from DB for cell; spawn into Map | Grid load transition |
| `ObjectGridLoader::Visit(GameObjectMapType&)` | Load GameObjects from DB for cell; spawn into Map | Grid load transition |
| `ObjectGridLoader::Visit(AreaTriggerMapType&)` | Load area triggers for cell | Grid load transition |
| `ObjectGridLoader::LoadN()` | Main entry point; queries DB and populates cell | Map::LoadGrid |
| `ObjectGridStoper::Visit(CreatureMapType&)` | Stop + despawn all creatures in cell before unload | Grid unload transition |
| `GridState::Update(Map&, NGrid&, GridInfo&, diff)` | Per-frame state machine tick (pure virtual) | Map::Update loop |
| `InvalidState::Update()` | No-op; grid is uninitialized | State machine |
| `ActiveState::Update()` | Check player presence; if none, transition to IDLE; else reset expiry timer | GridStates.cpp:28-44 |
| `IdleState::Update()` | Transition to REMOVAL after delay | GridStates.cpp:47-51 |
| `RemovalState::Update()` | Unload grid if no unload locks remain | GridStates.cpp:54-64 |

---

## 5. Module dependencies

**Depends on:**
- `Maps` — NGrid is contained by Map via GridRefManager; lifecycle driven by Map::Update()
- `Entities` (Creature, GameObject, AreaTrigger, Corpse, Player) — entities stored in Cells within NGrid
- `Database` (WorldDatabase) — ObjectGridLoader queries creature, gameobject tables
- `Data` (creature_template, gameobject_template DB2/cache) — spawn metadata
- `Cell` (implicit via Grid.h) — Cell visitor pattern
- `GridNotifiers` — visitor implementations for spell AoE, threat, visibility

**Depended on by:**
- `Maps` — Map uses NGrid as primary spatial container
- `Entities` — entities belong to Cells, which belong to NGrids
- `Movement` — relocation triggers cell updates
- `Spells` / `Combat` — AoE checks iterate nearby cells via VisitNearbyCellsOf
- `Scripts` — grid state hooks (OnGridLoad, OnGridUnload)
- `Phasing` — personal phase grids use PersonalPhaseGridLoader

---

## 6. SQL / DB queries (if any)

Grids **does not** directly emit SQL; all queries routed through ObjectGridLoader.

| Source / Context | Query Pattern | Purpose | Database |
|---|---|---|---|
| `ObjectGridLoader::Visit(CreatureMapType&)` | `SELECT creature FROM world.creature WHERE map=? AND grid_id=? AND cell_x=? AND cell_y=?` | Load spawns when grid activates | world |
| `ObjectGridLoader::Visit(GameObjectMapType&)` | `SELECT gameobject FROM world.gameobject WHERE map=? AND grid_id=? AND cell_x=? AND cell_y=?` | Load objects when grid activates | world |
| `ObjectGridLoader::Visit(AreaTriggerMapType&)` | `SELECT areatrigger FROM world.areatrigger WHERE map=? AND grid_id=?` | Load triggers when grid activates | world |
| (implicit) Template loading | (DB2 queries via DataStores) | creature_template, gameobject_template | world |

**No DB2 stores directly accessed by Grids** (filtered/loaded by ObjectMgr before ObjectGridLoader).

---

## 7. Wire-protocol packets (if any)

Grids module **does not originate** any packets directly. All object creation/destruction packets are sent by individual entities or Map::VisitNearbyCellsOf broadcasts.

| Opcode | Direction | Context |
|---|---|---|
| `SMSG_CREATE_OBJECT` / `SMSG_UPDATE_OBJECT` | server → client | Sent when entity spawns in cell (via creature/GameObject handlers) |
| `SMSG_DESTROY_OBJECT` | server → client | Sent when entity despawns in cell (grid unload or entity removal) |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-world/src/map_manager.rs` — ~350 lines — contains bare `GridCoord` struct only
- No dedicated `grid.rs` or grid state machine file

**What's implemented:**
- `GridCoord` struct: `x: i16, y: i16`
- `GridCoord::new(x, y)` constructor
- `GridCoord::surrounding()` → returns 9 cells in 3x3 around self
- `GridCoord::distance_squared(other)` → grid distance metric
- Constants: `GRID_SIZE = 64.0` (64x64 yards per cell), `DEFAULT_GRID_UNLOAD_TIME = 300s`

**What's missing vs C++:**
1. **No NGrid<> template/struct** — no 8x8 cell array container
2. **No GridInfo** — no state timer, unload locks, visibility tracker
3. **No GridState state machine** — no InvalidState, ActiveState, IdleState, RemovalState
4. **No Grid<T> generic container** — no cell-level entity storage
5. **No GridReference / GridRefManager** — no intrusive list tracking for entities
6. **No ObjectGridLoader** — no DB spawn loading on grid activation
7. **No coordinate conversions** — no world→grid, grid→cell index functions
8. **No grid lifecycle** — no ACTIVE/IDLE/REMOVAL transitions, no lazy loading/unloading
9. **No visibility tracking** — no NGrid::VisitAllGrids or cell iteration patterns
10. **No PersonalPhaseGridLoader** — no phase-specific grid loading

**Suspicious / likely divergent:**
- WorldCreature in session.rs is entity-like data but belongs in grid cells; architecture doesn't support hierarchical entity storage yet.
- No separation between "world objects" (Player, Corpse, DynamicObject) and "grid objects" (Creature, GameObject, AreaTrigger, Conversation).
- No lazy-load pattern: static spawning vs. dynamic on-demand loading not implemented.

**Tests existing:**
- 0 tests for grid coordinate math, state transitions, or entity loading

---

## 9. Migration sub-tasks

Numerated for reference in MIGRATION_ROADMAP.md section 5.

Complexity: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, split).

- [ ] **#GRIDS.1** Port GridDefines constants: MAX_NUMBER_OF_GRIDS=64, MAX_NUMBER_OF_CELLS=8, SIZE_OF_GRIDS=533.33, SIZE_OF_GRID_CELL=66.67 (L)
- [ ] **#GRIDS.2** Implement world-to-grid coordinate conversion: world_x,y → grid_idx (Trinity::ComputeGridCoord equivalent) with unit tests (L)
- [ ] **#GRIDS.3** Implement world-to-cell coordinate conversion within grid: world_x,y → cell_x,cell_y (0-7 range) (L)
- [ ] **#GRIDS.4** Enhance GridCoord: add `cell_from_world(x,y)`, boundary checking, GridCoord↔u32 serialization (M)
- [ ] **#GRIDS.5** Implement GridInfo struct: timer (TimeTracker), unload_locks (u16), vis_timer (PeriodicTimer), methods inc/dec locks (M)
- [ ] **#GRIDS.6** Implement GridState enum + Update trait: { Invalid, Active, Idle, Removal } each with update_state(&mut self, grid, info, diff) (M)
- [ ] **#GRIDS.7** Implement InvalidState::update() → no-op (L)
- [ ] **#GRIDS.8** Implement ActiveState::update() → periodic idle check; if no players + no active objects, transition to IDLE (M)
- [ ] **#GRIDS.9** Implement IdleState::update() → transition to REMOVAL after delay (L)
- [ ] **#GRIDS.10** Implement RemovalState::update() → if no unload locks, call map.unload_grid(self) (M)
- [ ] **#GRIDS.11** Build TypeMapContainer generic: holds heterogeneous object types (Creature, GameObject, etc.) in separate Vec/DashMap (H)
- [ ] **#GRIDS.12** Implement Cell<T> generic struct: containers for grid_objects + world_objects, add/remove/visit methods (H)
- [ ] **#GRIDS.13** Implement NGrid<T> struct: cells: [[Cell<T>; 8]; 8], state, info, grid_id, x, y; getters/setters (H)
- [ ] **#GRIDS.14** Implement NGrid::GetGridType(x, y) bounds-checked getter for Cell at index (M)
- [ ] **#GRIDS.15** Implement NGrid::SetGridState(state) with state transition logging (L)
- [ ] **#GRIDS.16** Implement GridRefManager trait/impl for intrusive reference tracking (entity→cell link) (H)
- [ ] **#GRIDS.17** Implement ObjectGridLoaderBase: load counts (creatures, gameobjects, areaTriggers) tracking (M)
- [ ] **#GRIDS.18** Implement ObjectGridLoader::Visit(CreatureMapType) → query DB, create Creature, add to cell (M)
- [ ] **#GRIDS.19** Implement ObjectGridLoader::Visit(GameObjectMapType) → query DB, create GameObject, add to cell (M)
- [ ] **#GRIDS.20** Implement ObjectGridLoader::Visit(AreaTriggerMapType) → query DB, create AreaTrigger, add to cell (L)
- [ ] **#GRIDS.21** Implement ObjectGridLoader::LoadN() entry point → iterate cells, call Visit for each type (M)
- [ ] **#GRIDS.22** Implement ObjectGridStoper visitor → despawn all creatures in cell before unload (M)
- [ ] **#GRIDS.23** Implement Cell::Visit<Visitor>() pattern → iterate entities matching type, call visitor for each (H)
- [ ] **#GRIDS.24** Implement NGrid::VisitAllGrids(visitor) → iterate all 64 cells, call visitor.Visit(cell) (M)
- [ ] **#GRIDS.25** Integration: wire NGrid state machine into Map::Update() loop (M)

---

## 10. Regression tests to write

Tests demonstrating Rust behavior ≡ C++ behavior for grid lifecycle and loading.

- [ ] Test: world_x,y → GridCoord matches Trinity::ComputeGridCoord for 100+ random points
- [ ] Test: GridCoord → cell_x,cell_y conversion for all boundaries (0-7 range, wrapping)
- [ ] Test: GridCoord::surrounding() returns 9 cells (verified each is adjacent or center)
- [ ] Test: GridInfo::inc/dec_unload_lock() maintains u16 counter correctly
- [ ] Test: GridInfo::getUnloadLock() returns true iff locks > 0 or explicit_lock == true
- [ ] Test: GridState transition sequence: Invalid → Active (player enter) → Idle (player leave + timeout) → Removal → Unload
- [ ] Test: ActiveState::update() triggers transition to Idle only if no players AND no active objects
- [ ] Test: RemovalState::update() calls map.unload_grid() only if getUnloadLock() == false
- [ ] Test: ObjectGridLoader populates cell with correct creature count (verify spawn_id ranges)
- [ ] Test: ObjectGridLoader loads only creatures in target cell (cell_x, cell_y bounds)
- [ ] Test: ObjectGridStoper despawns all creatures from cell (verify count → 0)
- [ ] Test: Cell::Visit iterates exactly N entities of type T
- [ ] Test: NGrid::GetGridType(x, y) returns correct Cell for x,y in [0,7] range
- [ ] Test: NGrid::VisitAllGrids calls visitor.Visit(cell) exactly 64 times
- [ ] Test: Multiple concurrent NGrid state transitions (no race conditions, ARC/RwLock safety)

---

## 11. Notes / gotchas

**C++ historical bugs / design quirks:**

- **Grid state timing asymmetry (GridStates.cpp:28-44):** ActiveState calls `info.UpdateTimeTracker(diff)` and checks expiry; IdleState and RemovalState do not call Update on InvalidState. This timing model must be preserved in Rust.
- **Unload lock increment logic (NGrid.h:36-39):** `incUnloadActiveLock()` is called when a creature with an active spawn point enters grid. Decrement must match increment 1:1; off-by-one causes memory leaks or premature despawning.
- **VisibilityNotifyPeriod (NGrid.h:28):** Grid only updates visibility/player notifications once per `DEFAULT_VISIBILITY_NOTIFY_PERIOD` (1000ms) to reduce network spam. Rust must throttle similarly.
- **Respawn timer ownership:** RespawnInfo entries are tied to grid lifecycle; if grid unloads before respawn fires, entry must be persisted to character DB. C++ uses ObjectMgr respawn queues; Rust must replicate.
- **Cell boundary rounding:** C++ SIZE_OF_GRID_CELL = 66.67f (SIZE_OF_GRIDS / MAX_NUMBER_OF_CELLS). Float divisions can cause off-by-one errors near cell boundaries. Recommend fixed-point or int arithmetic.
- **PersonalPhaseGridLoader complexity:** Loads spawns based on phase visibility for personal instances. Requires second loader class; not used in open world (only instances).
- **GridReference intrusive linking:** C++ uses boost intrusive pointers. Rust must use Arc<RwLock<T>> or custom intrusive structures; order matters for cleanup (link before add, unlink after remove).
- **Creature respawn state:** When creature despawns (grid unload), respawn timer is saved to DB (table: creature_respawn). Rust must mimic; track respawn time per creature_id+map_id.
- **GridLoader template specialization:** C++ Grid.h has explicit template instantiation for concrete NGridType. Rust may use static dispatch or dynamic dispatch; must match performance.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `Grid<A, W, G>` (template) | `struct Cell<T>` | Generic cell container; TypeMapContainer for heterogeneous types |
| `NGrid<N, A, W, G>` | `struct NGrid { cells: [[Cell; 8]; 8], ... }` | Fixed 8x8 array, state machine, timers |
| `GridInfo` | `struct GridInfo { timer: TimeTracker, unload_locks: u16, vis_timer: PeriodicTimer }` | Metadata for NGrid |
| `GridState` | `enum GridState { Invalid, Active, Idle, Removal }` | State machine (not virtual class hierarchy) |
| `GridState::Update(&Map, &NGrid, &GridInfo, uint32)` | `fn update_state(state: &mut GridState, grid: &mut NGrid, map: &Map, diff: u32)` | Pattern-matched enum dispatch |
| `InvalidState`, `ActiveState`, `IdleState`, `RemovalState` | Enum variants with match arms | No subclasses needed in Rust |
| `GridReference<T>` | `Arc<RwLock<T>>` or intrusive `LinkedListNode<T>` | Ownership tracking for entities |
| `GridRefManager<T>` | `Vec<Arc<RwLock<T>>>` or custom intrusive list | Container for GridReference nodes |
| `ObjectGridLoaderBase` | `struct ObjectGridLoaderBase { cell, grid, map, counts }` | Base data structure (no inheritance) |
| `ObjectGridLoader` | `impl ObjectGridLoader { fn visit_creatures(...), visit_gameobjects(...), load_n(...) }` | Methods, not virtual inheritance |
| `ObjectGridStoper` | `struct ObjectGridStoper; impl Visitor for ObjectGridStoper { fn visit_creatures(...) }` | Visitor pattern via trait impl |
| `typedef AllWorldObjectTypes` | `type AllWorldObjectTypes = (Player, Creature, Corpse, DynamicObject);` | Tuple or enum variant aggregate |
| `typedef AllGridObjectTypes` | `type AllGridObjectTypes = (GameObject, Creature, ..., Conversation);` | — |
| `grid.GetGridState()` | `grid.state` or `grid.get_state()` | Direct field or method |
| `grid.SetGridState(state)` | `grid.state = state;` | Direct assignment or setter |
| `info.getUnloadLock()` | `info.unload_locked()` → `self.unload_locks > 0 \|\| self.explicit_lock` | Boolean method |
| `Trinity::ComputeGridCoord(x, y)` | `fn world_to_grid(x: f32, y: f32) -> GridCoord` | Pure function in util |
| `NGrid::GetGridType(x, y)` | `ngrid.cells[x as usize][y as usize]` | Direct indexing with bounds |
| `Cell::AddWorldObject(obj)` | `cell.world_objects.insert(obj)` | TypeMapContainer or Vec insert |
| `Cell::Visit(visitor)` | `for obj in cell.world_objects.iter() { visitor.visit(obj) }` | Trait pattern matching |

---

*Template version: 1.0 (2026-05-01).* Revision: initial complete audit port.

