# Migration: Maps

> **C++ canonical path:** `src/server/game/Maps/`
> **Rust target crate(s):** `crates/wow-world/`, `crates/wow-map/`
> **Layer:** L3 (World layer)
> **Status:** 🔧 broken (rewrite needed)
> **Audited vs C++:** ⚠️ partial (Maps.md audit log via MIGRATION_ROADMAP.md indicates core architecture divergence)
> **Last updated:** 2026-05-01

---

## 1. Purpose

The Maps module in TrinityCore manages the world's spatiotemporal state: loading/unloading grids, tracking entities (players, creatures, GameObjects, corpses) within cells, terrain visibility queries, and coordinate transformations. It bridges the gap between a player's in-game position and the Grid/Cell hierarchy that lazily loads object spawns. It is the foundational memory manager for all spatial queries and respawn timers.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `src/server/game/Maps/Map.h` | 917 | Map class definition, grid management interface, player/object tracking |
| `src/server/game/Maps/Map.cpp` | 4014 | Map lifecycle (ctor/dtor), entity add/remove, grid load/unload state machine, Update() loop |
| `src/server/game/Maps/MapManager.h` | 183 | MapManager singleton definition; instance creation, lookup, threading |
| `src/server/game/Maps/MapManager.cpp` | 461 | MapManager implementation: CreateMap, CreateInstance, CreateBattleground, Update(), threading setup |
| `src/server/game/Maps/GridMap.h` | 103 | GridMap wrapper for terrain tile querying (height, liquid, flags) |
| `src/server/game/Maps/GridMap.cpp` | 694 | GridMap impl: height queries, water/liquid detection, area trigger checks |
| `src/server/game/Maps/MapUpdater.h` | 65 | Multithreaded map update executor (thread pool pattern) |
| `src/server/game/Maps/MapUpdater.cpp` | 123 | MapUpdater activation, thread spawning, work distribution |
| `src/server/game/Maps/TerrainMgr.h` | 167 | Terrain manager for VMap/ADT queries, visibility, pathfinding |
| `src/server/game/Maps/TerrainMgr.cpp` | 877 | VMap integration, ADT loading, line-of-sight, height sampling |
| `src/server/game/Maps/TransportMgr.h` | 185 | Static/dynamic transports (boats, elevators) lifecycle |
| `src/server/game/Maps/TransportMgr.cpp` | 714 | Transport spawning, path following, passenger handling |
| `src/server/game/Maps/AreaBoundary.h` | 167 | Area boundary definitions for safe/PvP zone checks |
| `src/server/game/Maps/AreaBoundary.cpp` | 107 | Boundary queries: IsInAreaTrigger, CheckBoundary, area type masks |
| `src/server/game/Maps/MapScripts.cpp` | 899 | Script event hooks: OnCreate, OnDestroy, OnObjectEnter, OnObjectLeave |
| `src/server/game/Maps/MapObject.h` | 60 | Marker base class for objects entering a map (deprecated but present) |
| `src/server/game/Maps/MapReference.h` | 51 | Intrusive reference node for player presence tracking in Map |
| `src/server/game/Maps/MapRefManager.h` | 52 | Manager for MapReference doubly-linked list (player tracking) |
| `src/server/game/Maps/ZoneScript.h` | 104 | Base class for zone/instance scripting hooks |
| `src/server/game/Maps/SpawnData.h` | 128 | Spawn data POD struct (type, entry, position, guids) |
| `src/server/game/Maps/enuminfo_SpawnData.cpp` | 67 | Enum reflection for SpawnData serialization |

**Total C++ lines in Maps/:** ~10,194 (excluding blank/comments)

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Map` | class | Main world/instance map container, inherits GridRefManager<NGridType> for player tracking |
| `MapManager` | class | Singleton managing all Map instances (world + instances); handles creation, lookup, threading |
| `MapEntry` | struct | DB2 record (Map.db2) — map ID, instance type, name, flags |
| `MapDifficultyEntry` | struct | DB2 record — difficulty mode per map (normal/heroic/mythic) |
| `TerrainInfo` | class | Terrain data (VMap, ADT, liquid) for a single map instance |
| `GridMap` | class | Single ADT tile terrain queries (height/water sampler) |
| `TransportInfo` | struct | Static/dynamic transport metadata (path, speed, seats) |
| `AreaBoundary` | class | 2D area boundary polygon for zone/PvP checks |
| `RespawnInfo` | struct | Respawn queue entry (type, entry, respawn time, gridId) |
| `ZoneDynamicInfo` | struct | Per-zone runtime state (music, weather, light overrides) |
| `GridInfo` | class | NGrid metadata (load timer, unload lock counter, visibility update timer) |
| `grid_state_t` | enum | Grid lifecycle state: INVALID, ACTIVE, IDLE, REMOVAL |
| `TransferAbortReason` | enum | Failure codes for instance entry attempts (max players, not found, locked, etc.) |
| `GridMapTypeMask` | enum | Bitmask for object type filtering (creature, gameobject, player, etc.) |
| `SpawnData` | struct | Generic spawn metadata (type, entry, position) — used for respawn tracking |
| `InstanceMap` | class | Map subclass for dungeons/raids (derives from Map; adds difficulty, instance lock) |
| `BattlegroundMap` | class | Map subclass for BGs/arenas (derives from Map; minimal scripting) |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `Map::AddPlayerToMap(Player*, bool)` | Adds player to map, assigns grid, broadcasts SMSG_NEW_WORLD | `Player::InitGroundPositionUpdateTime`, `Map::LoadGrid`, `WorldSession` |
| `Map::RemovePlayerFromMap(Player*, bool)` | Removes player from map, broadcasts SMSG_LOGOUT_COMPLETE | `GridRefManager::unlink` |
| `Map::AddToMap<T>(T*)` | Generic add object to grid + cell | `Object::SetMap`, `Grid::AddWorldObject` or `Grid::AddGridObject` |
| `Map::RemoveFromMap<T>(T*, bool)` | Generic remove object from grid + cell | `Object::RemoveFromWorld`, `Grid` visitor |
| `Map::LoadGrid(float, float)` | Explicitly load a grid cell at coords, fetch spawns from DB | `ObjectGridLoader::LoadN`, `Map::LoadGridForActiveObject` |
| `Map::LoadGridForActiveObject(float, float, WorldObject*)` | Load grid if object has active spawn point (prevents clone loading) | `Map::LoadGrid` |
| `Map::LoadAllCells()` | Preload all grids on map (used for instances) | Called by MapManager on instance create if CONFIG_INSTANCEMAP_LOAD_GRIDS |
| `Map::UnloadGrid(NGridType&, bool)` | Unload a single grid, despawn entities, clear from container | `GridStates::RemovalState::Update`, `Map::UnloadAll` |
| `Map::UnloadAll()` | Unload all grids and remove all objects | Called on map destruction or shutdown |
| `Map::IsGridLoaded(GridCoord)` | Query if a grid is currently loaded | Visibility checks, entity relocation |
| `Map::IsRemovalGrid(float, float)` | Check if grid is in REMOVAL state (about to unload) | Entity movement validation |
| `Map::Update(uint32)` | Per-frame map tick; updates all grids, objects, respawns | MapUpdater, main world loop |
| `Map::PlayerRelocation(Player*, x, y, z, o)` | Player moved; update grid cell, broadcast to nearby | `WorldSession::HandleMovement` |
| `Map::CreatureRelocation(Creature*, x, y, z, a, bool)` | Creature moved; update grid, respawn lock logic | `Creature::Update`, `MotionMaster::OnPathFinished` |
| `Map::GameObjectRelocation(GameObject*, x, y, z, o, bool)` | GameObject moved; update grid | `Transport::UpdatePassengers` |
| `Map::VisitNearbyCellsOf(WorldObject*, gridVisitor, worldVisitor)` | Iterate nearby entities in adjacent cells (9x9 grid) | Spell AoE, threat updates, visibility broadcasts |
| `Map::Visit<T, CONTAINER>(Cell, visitor)` | Visit single cell container (grid or world objects) | GridNotifiers, spell effects |
| `Map::GetVisibilityRange()` | Returns m_VisibleDistance (default ~100 yards) | Client rendering, entity interest |
| `Map::InitVisibilityDistance()` | Compute visibility range per map type | MapManager::InitializeVisibilityDistanceInfo |
| `Map::GetZoneId(PhaseShift, x, y, z)` | Lookup zone ID at position (terrain-based) | Phasing, quest validation |
| `Map::GetAreaId(PhaseShift, x, y, z)` | Lookup area ID at position (finer than zone) | Talent restriction checks |
| `Map::GetHeight(PhaseShift, x, y, z, vmap, maxDist)` | Ground/gameobject floor height at position | Fall damage, mount height |
| `Map::GetWaterLevel(PhaseShift, x, y)` | Water surface height at 2D position | Swimming validation |
| `MapManager::CreateMap(uint32, Player*)` | Create or fetch map instance for player (singleton per map+instance) | World::TransferPlayer |
| `MapManager::FindMap(uint32, uint32)` | Look up existing Map by ID + instance ID | Session::HandleTransferRequest |
| `MapManager::Update(uint32)` | Main world tick; dispatches to all Map::Update() | Main server loop |
| `MapManager::Initialize()` | Init state machine, spawn worker threads | Server startup |

---

## 5. Module dependencies

**Depends on:**
- `Grids` — stores NGridType, grid state machine (GridStates), ObjectGridLoader for cell loading
- `Entities` (Player, Creature, GameObject, DynamicObject, AreaTrigger, Corpse, Pet, Transport) — entities stored in grids + cells; Map is container
- `Database` (WorldDatabase, CharacterDatabase) — loads spawns, respawn times, corpse data from DB
- `Data` (DB2Stores: MapStore, MapDifficultyStore, AreaTriggerStore, LiquidStore) — map metadata, area info
- `TerrainMgr` — terrain queries (height, liquid, vmap, visibility)
- `SharedDefines` — enum definitions (Difficulty, SpawnObjectType, TransferAbortReason)
- `ObjectAccessor` — object lookups by GUID
- `ScriptMgr` — script callbacks (OnObjectEnter, OnObjectLeave, OnDestroy)
- `PhasingHandler` — zone/area visibility based on phase shifts

**Depended on by:**
- `World` — creates maps, orchestrates map updates in main loop
- `WorldSession` — player->map transfer, movement packets parsed into Map::PlayerRelocation
- `Creature`, `Player`, `GameObject` — stored in Map grids/cells; all positional queries route through Map
- `TransportMgr` — uses Map::AddToMap/RemoveFromMap for transport spawning
- `OutdoorPvPMgr` — zone-based PvP checks routed through Map area lookups
- `BattlegroundMgr` — creates BattlegroundMaps
- `Instances` / `DungeonFinding` — creates InstanceMaps
- `Spells` / `Combat` — positional queries (nearby enemies, LoS checks)
- `Movement` / `MotionMaster` — relocation calls back to Map when creature moves

---

## 6. SQL / DB queries (if any)

Map does **not** directly emit SQL; all DB I/O is delegated to specialized loaders or managers.

| Source / Context | Purpose | Database |
|---|---|---|
| `ObjectGridLoader::LoadN()` → creature_template, creature, gameobject, gameobject_template | Spawn loading when grid transitions to ACTIVE | world |
| `Map::LoadRespawnTimes()` → creature_respawn, gameobject_respawn | Load respawn timers on map init | character |
| `Map::LoadCorpseData()` → corpse | Load player corpses on map init | character |
| `Map::InitSpawnGroupState()` → spawn_group, spawn_group_template | Load conditional spawn groups | world |
| `TerrainMgr::LoadTerrain()` → (implicit via VMap DLL, ADT binary files) | Terrain/vmap loading | filesystem (not SQL) |

**DB2 Stores accessed:**
| Store | What it loads | Read by |
|---|---|---|
| `MapStore` | Map.db2 (map IDs, names, type, PvP flag, corpse decay time) | MapManager::CreateMap, Map::GetEntry |
| `MapDifficultyStore` | MapDifficulty.db2 (per-map difficulty modes) | Map::GetMapDifficulty, MapManager::CreateInstance |
| `AreaTriggerStore` | AreaTrigger.db2 (trigger points, radius, shape) | Map::GetAreaId, AreaBoundary checks |
| `LiquidStore` (implicit) | Liquid data per ADT tile | GridMap::GetLiquidStatus, Map::IsInWater |

---

## 7. Wire-protocol packets (if any)

Maps module **originates** very few packets; most are sent by Player, Creature, or Transport after Map relocation.

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `SMSG_NEW_WORLD` | server → client | `Map::AddPlayerToMap` after grid load, broadcasts map ID + position |
| `SMSG_LOGOUT_COMPLETE` | server → client | `Map::RemovePlayerFromMap` after cleanup |
| `SMSG_TRANSFER_PENDING` | server → client | `Map::AddPlayerToMap` (instance enter, phasing transition) |
| `SMSG_TRANSFER_ABORTED` | server → client | `MapManager::CreateMap` (denied due to difficulty/lock/max players) |
| `SMSG_DESTROY_OBJECT` | server → client | `Map::RemoveFromMap` → Object broadcast to nearby |
| `SMSG_CREATE_OBJECT` (legacy) / `SMSG_UPDATE_OBJECT` | server → client | `Map::AddToMap` → broadcast to nearby cells (via VisitNearbyCells) |

The module also **processes** CMSG_MOVE_* packets indirectly via WorldSession → Map::PlayerRelocation.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-world/src/map_manager.rs` — ~350 lines — **scaffold only**; basic MapManager skeleton, no grid state machine
- `crates/wow-world/src/map.rs` (if exists) — not found; Map impl missing
- `crates/wow-map/src/lib.rs` — **empty scaffold** (0 impl)
- `crates/wow-world/src/session.rs` (legacy) — 100+ lines — contains `WorldCreature` struct (creature AI state) **not a Map concern** but used in place of proper entity hierarchy

**What's implemented:**
- `GridCoord` struct with `surrounding()` method (basic neighbor grid calculation)
- Constants: `GRID_SIZE = 64.0`, `VISIBILITY_RADIUS = 100.0`, `DEFAULT_GRID_UNLOAD_TIME = 300s`
- `WorldCreature` POD in session (creature entry, HP, position, movement state, combat target) — **should be in Entity layer, not map_manager**
- MapManager singleton stub (no CreateMap, no FindMap logic)
- Bare `GridCoord` distance calculation

**What's missing vs C++:**
1. **No Map class** — no grid container, no entity tracking, no state machine
2. **No grid lifecycle** — no ACTIVE/IDLE/REMOVAL states; no lazy loading/unloading
3. **No ObjectGridLoader** — no spawning of creatures/GameObjects from DB on grid load
4. **No terrain queries** — no height/water/area lookups
5. **No player relocation** — no cell movement tracking or nearby entity broadcasts
6. **No respawn manager** — no respawn timer queue or timers
7. **No grid state machine** — the C++ GridStates (InvalidState, ActiveState, IdleState, RemovalState) classes **not ported**
8. **No visibility tracking** — no VisitNearbyCellsOf or cell-based entity interest
9. **No InstanceMap/BattlegroundMap subclasses** — no difficulty/lock handling
10. **No MapUpdater threading** — maps not updated in parallel threads
11. **No coordinate transformations** — no conversion from world coords to grid/cell indices
12. **Separated creature AI** — WorldCreature is jammed into session.rs instead of properly living in the map grid system

**Suspicious / likely divergent (pre-audit hypothesis from MIGRATION_ROADMAP.md):**
- The C++ model is: Map contains NGrids → NGrid contains Cells → Cell contains entities. Rust version has no hierarchical container model yet.
- WorldCreature in session.rs is a conflation of entity data + AI state; C++ separates Creature (Entity) from CreatureAI (behavior). RustyCore doesn't have proper polymorphic entity hierarchy.
- No lazy-load pattern; C++ grids load spawns on demand (GRID_STATE_INVALID → GRID_STATE_ACTIVE → load via ObjectGridLoader). Rust version has no equivalent.

**Tests existing:**
- 0 tests in `crates/wow-world/` or `crates/wow-map/`

---

## 9. Migration sub-tasks

Numerated for reference in MIGRATION_ROADMAP.md section 5.

Complexity: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, split).

- [ ] **#MAPS.1** Extract Map::GetId(), GetEntry(), GetDifficultyID(), Instanceable(), IsDungeon() etc. into MapProperties struct; build static DB2 query layer (L)
- [ ] **#MAPS.2** Implement GridCoord → NGridType coord mapping (Trinity::ComputeGridCoord reverse, MAX_NUMBER_OF_GRIDS=64, SIZE_OF_GRIDS=533.33) with unit tests (L)
- [ ] **#MAPS.3** Port GridDefines (MAX_NUMBER_OF_CELLS=8, SIZE_OF_GRID_CELL=66.67, TOTAL_NUMBER_OF_CELLS_PER_MAP=512) as constants + cell coord math (M)
- [ ] **#MAPS.4** Implement GridInfo state tracker (timer, unload locks, visibility update periodic) matching C++ GridInfo (M)
- [ ] **#MAPS.5** Implement grid state machine classes: GridState trait, InvalidState, ActiveState, IdleState, RemovalState (matches GridStates.h) (M)
- [ ] **#MAPS.6** Build NGrid container (8x8 cell grid per terrain node) with GetGridType(x,y), SetGridState, GetGridState, isGridObjectDataLoaded (H)
- [ ] **#MAPS.7** Port ObjectGridLoader pattern: load creatures + GameObjects from DB when grid enters ACTIVE state, filtered by cell (H)
- [ ] **#MAPS.8** Implement Map::LoadGrid(x, y) trigger → ObjectGridLoader::LoadN() → spawn loading (M)
- [ ] **#MAPS.9** Implement Map::UnloadGrid(NGrid, force) → despawn entities → unload from DB (M)
- [ ] **#MAPS.10** Implement Map::Update(diff_ms) main loop → iterate all NGrids → state update for each grid (H)
- [ ] **#MAPS.11** Implement MapManager::CreateMap(map_id, player?) → instance lookup or create → return Map or InstanceMap (M)
- [ ] **#MAPS.12** Implement MapManager::FindMap(map_id, instance_id) lookup from DashMap (L)
- [ ] **#MAPS.13** Implement MapManager::Update(diff_ms) → iterate all maps → Map::Update (L)
- [ ] **#MAPS.14** Port Grid<T> generic container (Grid.h template) as Rust generic; TypeMapContainer variant (H)
- [ ] **#MAPS.15** Implement Cell-level entity tracking: AddWorldObject, AddGridObject, RemoveWorldObject, RemoveGridObject (M)
- [ ] **#MAPS.16** Implement Map::AddPlayerToMap(player) → load surrounding grids → broadcast SMSG_NEW_WORLD (M)
- [ ] **#MAPS.17** Implement Map::RemovePlayerFromMap(player) → check if last player → schedule grid unload (M)
- [ ] **#MAPS.18** Implement Map::PlayerRelocation(player, x, y, z, o) → cell change detection → update grid cell → broadcast to nearby (M)
- [ ] **#MAPS.19** Implement Map::CreatureRelocation(creature, x, y, z, a) → cell update + respawn lock check (M)
- [ ] **#MAPS.20** Implement Map::VisitNearbyCellsOf(object, visitor) → 9x9 grid iteration around object (H)
- [ ] **#MAPS.21** Implement RespawnInfo queue (min-heap, respawn times, type/entry/spawnId) matching C++ respawn system (H)
- [ ] **#MAPS.22** Implement InstanceMap subclass (Map + difficulty + instance lock + script context) (M)
- [ ] **#MAPS.23** Implement BattlegroundMap subclass (Map + BG reference) (L)
- [ ] **#MAPS.24** Port TerrainInfo queries (height, water, area, LoS) wrapper around wow-recastdetour or stub (XL — deferred to L5)
- [ ] **#MAPS.25** Refactor WorldCreature from session.rs into crates/wow-entity/creature.rs; remove from session, use entity lookup (H)

---

## 10. Regression tests to write

Tests that demonstrate Rust behavior ≡ C++ behavior for key invariants.

- [ ] Test: GridCoord::surrounding() returns exactly 9 cells in 3x3 around self (including self)
- [ ] Test: GridCoord::distance_squared() matches Manhattan-like grid distance metric
- [ ] Test: coord_to_grid(x, y) → grid_coord matches Trinity::ComputeGridCoord (stochastic test on boundaries: -100, 0, 100, 1000 etc.)
- [ ] Test: GridInfo timer expiry matches C++ TimeTracker logic (reset on state change, update on tick)
- [ ] Test: Grid state transitions: INVALID → no update → ACTIVE (on player enter) → IDLE (on no players) → REMOVAL → unload
- [ ] Test: ObjectGridLoader loads creatures + GameObjects only for the target cell (verify spawn_id ranges)
- [ ] Test: Map::AddPlayerToMap broadcasts SMSG_NEW_WORLD with correct map_id + position
- [ ] Test: Map::PlayerRelocation on cell boundary updates grid cell reference
- [ ] Test: Map::VisitNearbyCellsOf(cell=center) visits exactly 9 cells (8 neighbors + self)
- [ ] Test: RespawnInfo heap ordering (earlier respawn times sorted first) matches C++ CompareRespawnInfo
- [ ] Test: UnloadGrid removes all entities from container (creature, GameObject, etc. counts → 0)
- [ ] Test: MapManager::Update() ticks all maps; verify all grids receive Update(diff) call
- [ ] Test: Concurrent Map::Update() + entity add/remove (no race conditions, ARC/RwLock safety)
- [ ] Test: Instance map cannot unload while player is inside (unload lock)
- [ ] Test: BattlegroundMap unloads when last player leaves (time-based vs explicit)

---

## 11. Notes / gotchas

**C++ historical bugs / design quirks:**
- **Grid state machine timing:** The C++ GridInfo::UpdateTimeTracker(diff) is called only by ActiveState and RemovalState, not InvalidState. RustyCore must preserve this asymmetry (GridStates.cpp:28-43).
- **GRID_STATE_REMOVAL race condition:** If a player enters a grid during RemovalState, the unload can be deferred. C++ uses explicit unload locks (`getUnloadLock()`) to prevent despawning active entities (GridInfo.cpp:36-39).
- **Respawn heap performance:** C++ uses `boost::heap::fibonacci_heap` for O(1) peek/pop of next respawn; Rust BinaryHeap is O(log n) — acceptable but slower (Map.cpp:78).
- **VisitNearbyCellsOf asymmetry:** Grid cells are 8x8 per NGrid, but VisitNearbyCellsOf visits in a 9-cell radius around player, not grid boundary. Off-by-one risks exist if boundaries are misaligned (Map.cpp:214).
- **Player interest management:** C++ has subtle visibility "interest" protocol — clients only see creatures/objects in cells nearby. If VisitNearbyCellsOf is off, clients get stale data or pop-in bugs.
- **Respawn timer cleanup:** Respawns are per-cell; orphaned RespawnInfo entries (creature deleted, respawn timer still in heap) can cause memory leaks if cleanup is missed (Map::UnloadGrid → UnloadRespawnInfos).
- **World map vs Instance map:** Both inherit from Map but behave differently:
  - **World maps** unload grids on timeout (default 5 min idle).
  - **Instance maps** keep grids loaded while locked (DungeonEncounter not defeated).
  - **Battleground maps** unload immediately after all players leave.
  This logic is in Map::CanUnload() and MapManager::CreateWorldMap/CreateInstance. Must preserve per-type behavior (MapManager.cpp:71-131).
- **TransferAbortReason enum:** 33 distinct failure codes (TRANSFER_ABORT_*). Not all are actively used in 3.4.3; some are legacy. Document which ones actually send to client (Map.h:83-107).

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class Map` | `struct Map` in `crates/wow-world/src/map.rs` | Contains Arc<DashMap<GridCoord, NGrid>>, player_refs: Vec<PlayerRef>, owned objects |
| `MapManager` (singleton) | `static MAPMANAGER: OnceLock<MapManager>` in `crates/world-server/src/main.rs` or dedicated module | Singleton pattern using OnceLock or lazy_static |
| `Map::AddPlayerToMap(Player*)` | `fn add_player_to_map(&mut self, player_id: u64) -> Result<Position>` | player_id instead of pointer; return position or error |
| `Map::LoadGrid(float, float)` | `fn load_grid(&mut self, x: f32, y: f32) -> Result<()>` or `async` | Fetches DB, spawns creatures/GameObjects |
| `Map::Update(uint32)` | `fn update(&mut self, diff_ms: u32)` | Iterate NGrids, call state.update(&mut self, &mut grid, diff_ms) |
| `NGridType` (NGrid<8, ...>) | `struct NGrid { cells: [[Cell; 8]; 8], state: GridState, ... }` | 2D array of cells, state enum |
| `Grid<T>` (template) | `struct Cell<T>` / `CellContainer` generic over AllGridObjectTypes | Rust generics instead of C++ templates |
| `GridState` | `enum GridState { Invalid, Active, Idle, Removal }` | Enum instead of virtual class hierarchy |
| `GridInfo` | `struct GridInfo { timer: TimeTracker, unload_locks: u16, vis_timer: PeriodicTimer }` | Same fields, simpler structure |
| `ObjectGridLoader::LoadN()` | `fn load_cell(cell: &mut Cell, grid_id: u32, cx: u8, cy: u8, db: &WorldDb)` | Async/blocking DB query; populate cell |
| `RespawnInfo` | `struct RespawnEntry { spawn_type: SpawnType, entry: u32, respawn_time: SystemTime, grid_id: u32 }` | SystemTime instead of time_t (Unix epoch) |
| `Map::Player* player` | `Arc<RwLock<Player>>` or `player_id: u64` + session lookup | Avoid raw pointers; use Arc or IDs + lookup |
| `Creature*, GameObject*` | `Arc<RwLock<Creature>>` / `Box<GameObject>` | Owned or shared, not raw ptrs |
| `std::unordered_map<ObjectGuid, Creature*>` | `DashMap<ObjectGuid, Arc<RwLock<Creature>>>` | Concurrent hashmap for thread-safe entity lookups |
| `Trinity::ComputeGridCoord(x, y)` | `fn world_to_grid_coord(x: f32, y: f32) -> GridCoord` | Pure function in util module |
| `grid.GetGridState()` | `grid.state` (direct field access) or `grid.get_state()` (method) | Pattern match on enum |
| `void Foo::Update(uint32)` | `fn update(&mut self, diff_ms: u32)` | — |

---

*Template version: 1.0 (2026-05-01).* Revision: initial complete audit port.

