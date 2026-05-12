# Migration: Movement / PathGenerator (Detour navmesh consumer)

> **C++ canonical path:** `src/server/game/Movement/PathGenerator.{h,cpp}` + Detour bridge from `src/common/Collision/Management/MMapManager.{h,cpp}` + `src/common/Collision/Maps/MMapDefines.h` + 3rdparty `dep/recastnavigation/Detour/`
> **Rust target crate(s):** `crates/wow-recastdetour/` (MMapDefines slice + future FFI bindings to Recast/Detour) + `crates/wow-movement/src/path_generator.rs`
> **Layer:** L5 sub-module (depends on `wow-collision` / VMaps L3 for height fallback, mmap tile loading from disk)
> **Status:** ⚠️ first portable slices started — `wow-movement::PathGenerator` now has C++ constants/flags, no-navmesh shortcut fallback, geometry helpers, path length and `ShortenPathUntilDist`; `wow-recastdetour` now vendors and compiles legacy Detour C++, has first `dtNavMesh` Rust binding (`alloc/free/init/getMaxTiles/addTile/removeTile`) with real success/error smoke coverage, `dtNavMeshQuery` allocation/init/free, `findNearestPoly` and a `dtQueryFilter` wrapper, and has `MMapDefines.h` constants/flags, `MmapTileHeader`, `.mmtile` blob reader, Detour constants, `dtNavMeshParams` parsing and a pre-FFI `MMapManager` map cache, but no operational tile loader or full query method set yet
> **Audited vs C++:** ✅ complete 2026-05-01
> **Last updated:** 2026-05-12

> Sub-doc of [`movement.md`](movement.md). Cross-links: [`movement-generators.md`](movement-generators.md) (`Chase`/`Follow`/`Point`/`Waypoint`/`Confused`/`Fleeing` are the consumers), [`movement-spline.md`](movement-spline.md) (`MoveSplineInit::MoveTo` invokes `PathGenerator::CalculatePath`), [`common-collision.md`](common-collision.md) (height/LOS via VMaps; mmap tile coordinates align with VMap grid), [`ai-base.md`](ai-base.md) (AI scripts indirectly trigger pathfind via generators).

---

## 1. Purpose

Bridges server motion to **walkable terrain**: given a `(start, end)` pair, produce a polyline that follows ground/water/transitions, avoids obstacles (trees, walls, cliffs), and respects creature affinity (ground vs water vs lava). Wraps Recast/Detour's `dtNavMesh` + `dtNavMeshQuery` with a per-`WorldObject` `dtQueryFilter`. Loads precomputed `mmtile` files (533.333-yard tiles, one per map grid cell) on demand and queries them via Detour's funnel algorithm + smooth path post-processing. Used by chase, follow, point, waypoint, confused, fleeing — anything that needs to "move there" rather than "snap there".

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `game/Movement/PathGenerator.cpp` | 1045 | `prefix` |
| `game/Movement/PathGenerator.h` | 150 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Movement/PathGenerator.h` | 150 | Class definition + `PathType` enum + constants (`MAX_PATH_LENGTH=74`, `SMOOTH_PATH_STEP_SIZE=4.0f`, `SMOOTH_PATH_SLOP=0.3f`, `INVALID_POLYREF=0`) + private smooth-path helpers |
| `src/server/game/Movement/PathGenerator.cpp` | 1045 | All path computation: `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `BuildShortcut`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, `AddFarFromPolyFlags`, `ShortenPathUntilDist`, filter setup |
| `src/common/Collision/Management/MMapManager.h` | ~90 | Singleton tile cache: `loadMap(mapId,x,y)`, `unloadMap`, `loadMapInstance(meshMapId, instanceMapId, instanceId)`, `loadMapData(mapId)`, `GetNavMesh(mapId)`, `GetNavMeshQuery(meshMapId, instanceMapId, instanceId)` |
| `src/common/Collision/Management/MMapManager.cpp` | ~360 | Reads `mmaps/{mapid:04}.mmap` as `dtNavMeshParams` + per-tile `{mapid:04}{x:02}{y:02}.mmtile`; calls `dtNavMesh::init`/`addTile` |
| `src/common/Collision/Maps/MMapDefines.h` | 71 | `NavArea`, `NavTerrainFlag` (`GROUND=1`, `GROUND_STEEP=2`, `WATER=4`, `MAGMA_SLIME=8`, `EMPTY=0`) + `MMAP_MAGIC` + `MMAP_VERSION` + 20-byte `MmapTileHeader` |
| `dep/recastnavigation/Detour/Include/DetourNavMesh.h` | 3rdparty | `dtNavMesh`, `dtMeshTile`, `dtPoly`, `dtPolyRef` |
| `dep/recastnavigation/Detour/Include/DetourNavMeshQuery.h` | 3rdparty | `dtNavMeshQuery::findPath`, `findStraightPath`, `closestPointOnPoly`, `moveAlongSurface`, `getPolyHeight` |
| `dep/recastnavigation/Detour/Include/DetourCommon.h` | 3rdparty | Helper math: `dtVdist`, `dtVlerp`, `dtVcopy`, `dtTriArea2D` |

Optional (some Trinity forks have `PathFinderTerrainInfo.cpp` exposing per-tile terrain queries — *not present in this WotLK 3.4.3 source*; verified absent in `/home/server/woltk-trinity-legacy/src/server/game/Movement/`).

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `PathGenerator` | class | Per-`WorldObject` pathfinder; holds Detour query, filter, polyref array, output `_pathPoints` |
| `PathType` | enum (uint8/bitfield) | `BLANK=0x00` / `NORMAL=0x01` / `SHORTCUT=0x02` / `INCOMPLETE=0x04` / `NOPATH=0x08` / `NOT_USING_PATH=0x10` / `SHORT=0x20` / `FARFROMPOLY_START=0x40` / `FARFROMPOLY_END=0x80` / `FARFROMPOLY=START|END` |
| `dtPolyRef` | typedef (uint64) | Detour polygon reference (tile + poly index packed) |
| `dtNavMeshParams` | struct | 28-byte `.mmap` payload: `orig[3]`, `tileWidth`, `tileHeight`, `maxTiles`, `maxPolys` |
| `dtNavMesh` | 3rdparty class | Loaded navmesh per-map; tile-addressable |
| `dtNavMeshQuery` | 3rdparty class | Query object holding the path-find buffers; per-instance because not thread-safe |
| `dtQueryFilter` | 3rdparty class | Per-`WorldObject` filter: include/exclude `NavTerrain` flags; per-area cost |
| `dtPoly` | 3rdparty struct | Polygon header within a tile |
| `NavTerrainFlag` | enum | `EMPTY=0`, `GROUND=1`, `GROUND_STEEP=2`, `WATER=4`, `MAGMA_SLIME=8` (derived from `NAV_AREA_MAX_VALUE - NAV_AREA_*`) |
| `MMapManager` (singleton) | class | Tile cache; `loadMap(mapId)`, `loadMapInstance`, per-instance `dtNavMeshQuery` map |
| `MMapData` (per-map) | struct | `dtNavMesh*` + per-instance `unordered_map<uint32, dtNavMeshQuery*>` + loaded tile set |
| `MmapTileHeader` | struct | 20-byte on-disk header: `mmapMagic`, `dtVersion`, `mmapVersion`, `size`, `usesLiquids`, `padding[3]` |
| `Movement::PointsArray` | typedef | `std::vector<Vector3>` — output type |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `PathGenerator::PathGenerator(WorldObject const* owner)` | Resolve owner's mapId → `MMapManager::getNavMesh + getNavMeshQuery`; setup filter | `MMapManager::Get*`, `CreateFilter` |
| `PathGenerator::~PathGenerator()` | Release filter; query is owned by `MMapManager` (not freed here) | — |
| `PathGenerator::CalculatePath(destX, destY, destZ, forceDest=false)` | Top-level entry: build polypath then point path; or shortcut if mmaps unavailable | `BuildPolyPath`, `BuildPointPath`, `BuildShortcut` |
| `PathGenerator::IsInvalidDestinationZ(target) const` | Check if dest Z is outside reasonable polygon vertical range | `Map::GetHeight` |
| `PathGenerator::GetStartPosition()` / `GetEndPosition()` / `GetActualEndPosition()` | Read the three positions tracked through path build | — |
| `PathGenerator::GetPath() const` | Return computed `_pathPoints` (PointsArray) | — |
| `PathGenerator::GetPathLength() const` | Sum of segment distances | — |
| `PathGenerator::GetPathType() const` | Return computed `PathType` bitmask | — |
| `PathGenerator::SetUseStraightPath(bool)` | Switch between `findStraightPath` (game-style funnel) vs `FindSmoothPath` | sets `_useStraightPath` |
| `PathGenerator::SetPathLengthLimit(distance)` | Clamp `_pointPathLimit = min(distance/SMOOTH_PATH_STEP_SIZE, MAX_POINT_PATH_LENGTH=74)` | — |
| `PathGenerator::SetUseRaycast(bool)` | Use Detour raycast (for chase: cheap straight-line LOS check first) | sets `_useRaycast` |
| `PathGenerator::ShortenPathUntilDist(point, dist)` | Trim final segments until path-end is `dist` away from `point` (chase/follow) | — |
| `PathGenerator::BuildPolyPath(start, end)` | Snap start/end to nearest polys; `dtNavMeshQuery::findPath` to fill `_pathPolyRefs[]` | `GetPolyByLocation`, Detour findPath |
| `PathGenerator::BuildPointPath(start, end)` | Convert polypath → smooth/straight world-space points | `FindSmoothPath` or `findStraightPath` |
| `PathGenerator::BuildShortcut()` | Fallback: 2-point straight line `[start, actualEnd]` (no mmaps / `PATHFIND_NOT_USING_PATH`) | — |
| `PathGenerator::FindSmoothPath(startPos, endPos, polyPath, polyPathSize, smoothPath, smoothPathSize, maxSize)` | Detour smooth-path: iterate `GetSteerTarget` + `moveAlongSurface` + step `SMOOTH_PATH_STEP_SIZE` | `GetSteerTarget`, `dtNavMeshQuery::moveAlongSurface`, `FixupCorridor` |
| `PathGenerator::FixupCorridor(path, npath, maxPath, visited, nvisited)` | Merge `visited[]` into corridor (resync after `moveAlongSurface`) | array splice |
| `PathGenerator::GetSteerTarget(startPos, endPos, minTargetDist, polyPath, polyPathSize, &steerPos, &steerFlag, &steerRef)` | Pick next steering target by walking along the corridor | `dtNavMeshQuery::findStraightPath` |
| `PathGenerator::GetPathPolyByPosition(polyPath, size, point, &dist)` | Find which corridor poly is closest to `point` | `dtNavMeshQuery::closestPointOnPolyBoundary` |
| `PathGenerator::GetPolyByLocation(point, &dist)` | Find any nearest poly within filter | `dtNavMeshQuery::findNearestPoly` |
| `PathGenerator::HaveTile(point) const` | Check if mmap tile at `(tx, ty)` is loaded | `dtNavMesh::getTileAndPolyByRef` |
| `PathGenerator::Dist3DSqr(p1, p2)` / `InRange(p1, p2, r, h)` / `InRangeYZX(v1, v2, r, h)` | Geometry helpers | — |
| `PathGenerator::GetNavTerrain(x, y, z) const` | Determine ground/water/magma/slime at point (used to set filter) | `dtPoly::getArea` |
| `PathGenerator::CreateFilter()` | Initial filter setup based on owner type (creature vs player, swimming, flying) | `dtQueryFilter::setIncludeFlags` |
| `PathGenerator::UpdateFilter()` | Refresh filter when owner state changes (e.g. enters water) | as above |
| `PathGenerator::NormalizePath()` | Clamp Z of each point to actual ground via `Map::GetHeight` | `Map::GetHeight` |
| `PathGenerator::AddFarFromPolyFlags(startFar, endFar)` | Set `PATHFIND_FARFROMPOLY_START/END` bits when snap distance exceeds tolerance | sets `_type` |
| `MMapManager::loadMapData(mapId)` | Open `mmaps/{mapid:04}.mmap`, read `dtNavMeshParams`, initialize `dtNavMesh` | file I/O, Detour |
| `MMapManager::loadMap(mapId, x, y)` | Load `mmaps/{mapid:04}{x:02}{y:02}.mmtile`; `dtNavMesh::addTile` | file I/O, Detour |
| `MMapManager::loadMapInstance(mapId, instanceId)` | Allocate per-instance `dtNavMeshQuery` (Detour query is not thread-safe; one per instance) | Detour `init` |
| `MMapManager::unloadMap(mapId)` / `unloadMapInstance` | Tear down | Detour `removeTile`, `dtFreeNavMeshQuery` |
| `MMapManager::getNavMesh(mapId)` / `getNavMeshQuery(mapId, instanceId)` | Accessors used by `PathGenerator` ctor | — |

---

## 5. Module dependencies

**Depends on:**
- `dep/recastnavigation/Detour/` — entire Detour runtime: `dtNavMesh`, `dtNavMeshQuery`, `dtQueryFilter`, `dtPoly`, `dtVdist`/`dtVlerp` math.
- `MMapManager` (in `src/common/Collision/Management/`) — loads `.mmap`/`.mmtile` files from disk; per-map and per-instance Detour query allocation.
- `Maps` — `Map::GetHeight` (VMap fallback for `IsInvalidDestinationZ` and `NormalizePath` Z clamp).
- `Entities/WorldObject` — owner provides `mapId`, `instanceId`, current position, swim/fly state.
- [`common-collision.md`](common-collision.md) — `VMapManager` for height query when normalizing path Z; mmap tile coords align 1:1 with VMap grid (1600/3 = 533.333 yards per tile).
- [`movement-spline.md`](movement-spline.md) — produces `PointsArray` consumed by `MoveSplineInit::MovebyPath`.
- `Config` — `mmap.enabled`, `mmap.allowedMaps` flags.

**Depended on by:**
- Every [`movement-generators.md`](movement-generators.md) generator that calls `MoveSplineInit::MoveTo(..., generatePath=true)`: Chase, Follow, Point, Waypoint, Confused, Fleeing, Random (when `_setRandomLocation` validates pickability).
- `MotionMaster::MoveCharge(PathGenerator const& path, ...)` accepts a pre-computed `PathGenerator` for charge spells.
- `Spell System` — line-of-sight charge / blink path validation.
- `Creature::CanWalkOnWater`, `Creature::IsInWater` — feed back into filter setup.

---

## 6. SQL / DB queries (if any)

`PathGenerator` itself emits **no SQL**. Two indirect data sources:

| Statement / Source | Purpose | DB |
|---|---|---|
| Config `mmap.enabled` (worldserver.conf) | Master switch — disabled → all paths fall back to `BuildShortcut` | config |
| Config `mmap.allowedMaps` | Per-map allowlist | config |

DBC/DB2 stores indirectly relevant:

| Store | What it loads | Read by |
|---|---|---|
| `LiquidTypeStore` (`LiquidType.dbc`) | Used by `GetNavTerrain` to distinguish water/magma/slime when querying poly area | indirect via Map |

**On-disk data** (the real "DB"):

| Path | Format | Loaded by |
|---|---|---|
| `mmaps/{mapid:04}.mmap` | Binary `dtNavMeshParams` payload | `MMapManager::loadMapData` → `dtNavMesh::init` |
| `mmaps/{mapid:04}{x:02}{y:02}.mmtile` | 20-byte `MmapTileHeader` + Detour tile blob | `MMapManager::loadMap` → `dtNavMesh::addTile` |
| `vmaps/...` | VMap binary (used by `Map::GetHeight` fallback) | `VMapManager` |

Tile generation is offline (`mmaps_generator` tool from Trinity); the runtime only loads.

---

## 7. Wire-protocol packets (if any)

`PathGenerator` does not emit packets. Its output (`PointsArray`) is consumed by `MoveSplineInit::MovebyPath` which then emits `SMSG_ON_MONSTER_MOVE` (see [`movement-spline.md`](movement-spline.md) §7 and [`movement.md`](movement.md) §7).

| Opcode | Direction | Sent/Received in |
|---|---|---|
| (none directly) | — | — |
| (indirect) `SMSG_ON_MONSTER_MOVE` | S→C | via `MoveSplineInit::Launch` after `PathGenerator::CalculatePath` |

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-recastdetour` | `crate_dir` | 1 | partial | `portable_slice` | Vendored legacy Detour compiles via `build.rs`; first `dtNavMesh` binding exists including `addTile/removeTile`; `MMapDefines.h` constants/flags, `MmapTileHeader`, `.mmtile` blob reader, Detour constants, `dtNavMeshParams` parser, pre-FFI `MMapManager` map cache and helper names/pack ids |
| `crates/wow-movement/src/path_generator.rs` | `path` | 1 | partial | `portable_slice` | C++ constants/flags, no-navmesh shortcut fallback, path length, geometry helpers and `ShortenPathUntilDist`; no Detour-backed pathfinding |
| `crates/wow-recastdetour/Cargo.toml` | `file` | 1 | partial | `exists_manifest` | declares `bitflags`/`thiserror` and `cc` build dependency for vendored Detour |
| `crates/wow-recastdetour/src/lib.rs` | `file` | 1 | partial | `portable_slice` | portable `MMapDefines.h`/Detour constants, `.mmap` params, first `dtNavMesh` wrapper with tile add/remove and pre-FFI `MMapManager`; query/filter bindings pending |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-recastdetour/Cargo.toml` — declares `bitflags`/`thiserror`.
- `crates/wow-recastdetour/build.rs` — compiles the vendored legacy Detour C++ sources with `cc`.
- `crates/wow-recastdetour/cpp/detour_c_api.cpp` — narrow C ABI bridge for the initial `dtNavMesh` functions.
- `crates/wow-recastdetour/vendor/Detour/` — exact copy of legacy `dep/recastnavigation/Detour/{Include,Source}`; `CMakeLists.txt` intentionally not copied because Cargo owns the build.
- `crates/wow-recastdetour/src/lib.rs` — portable `MMapDefines.h`/Detour header slice: `MMAP_MAGIC`, `MMAP_VERSION`, 20-byte `MmapTileHeader`, `.mmtile` blob reader, `dtNavMeshParams`, Detour version/magic constants/status helpers, first `DetourNavMesh` wrapper, `NavArea`, `NavTerrainFlag`, tile-id packing, C++ file naming helpers and pre-FFI `MMapManager` map cache.
- `crates/wow-movement/src/path_generator.rs` — first portable slice: `PathType`, constants, degraded no-navmesh shortcut fallback, `SetPathLengthLimit`, distance/range helpers, far-from-poly flags and `ShortenPathUntilDist` shape.
- *No* `mmap` tile loader anywhere.
- First hand-rolled Detour C ABI exists for `dtNavMesh`, `dtNavMeshQuery` allocation/init/free, `dtNavMeshQuery::findNearestPoly` and `dtQueryFilter` flags/area costs; remaining query methods are still pending.

**What's implemented:**
- Portable, owner-independent pathgen slice in `wow-movement`: exact `PathType` bits and constants from `PathGenerator.h`, C++-style no-navmesh fallback (`BuildShortcut` followed by `PATHFIND_NORMAL | PATHFIND_NOT_USING_PATH`), path length, `SetUseStraightPath`, `SetUseRaycast`, `SetPathLengthLimit`, `Dist3DSqr`, `InRange`, `InRangeYZX`, `AddFarFromPolyFlags`, and `ShortenPathUntilDist` with line-of-sight abstracted as a callback.
- `wow-recastdetour` vendors and compiles the legacy Detour C++ sources, exposes the first safe Rust wrapper for `dtNavMesh` allocation/init/free and tile add/remove, covers C++ `MMapDefines.h` data layout, validates `.mmtile` headers and reads the tile data blob before Detour owns it, can parse the `.mmap` `dtNavMeshParams` payload that `MMapManager::loadMapData` reads before `dtNavMesh::init`, and has a pre-FFI `MMapManager` skeleton for `loadedMMaps`, `InitializeThreadUnsafe`, failed-open placeholders, `parentMapData`, `loadMapData` and `unloadMap(mapId)` map-level state.

**What's missing vs C++:**
- **Detour-backed `PathGenerator` behavior** — the real `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, filter setup and mmap-backed branches remain unported.
- **Owner-backed context** — the portable `PathGenerator` does not own a `WorldObject`, `Map`, `MMapManager`, collision height or phase shift yet; LOS/Z normalization are represented through explicit callbacks or left for owner integration.
- **`dtNavMeshQuery` methods / filter integration** — `dtNavMesh` allocation/init/free/addTile/removeTile, `dtNavMeshQuery` allocation/init/free, `findNearestPoly` and `dtQueryFilter` flags/area costs now have wrappers and smoke coverage, but the remaining query methods (`findPath`, `findStraightPath`, `moveAlongSurface`, etc.) and `PathGenerator::CreateFilter/UpdateFilter` integration are still missing.
- **`MMapManager` tile/query branch** — map-level cache and `.mmap` param loading exist, but there is no operational `loadMap(mapId,x,y)` success path, no `loadMapInstance` and no per-instance query allocation.
- **Operational `.mmtile` Detour loading** — the 20-byte tile header parser, blob reader and generated Detour tile success path exist, but `MMapManager::loadMap(mapId,x,y)` still does not read real tile files into the runtime navmesh.
- **Filter creation integration** — raw `dtQueryFilter` wrapper exists, but `PathGenerator::CreateFilter` / `UpdateFilter`, creature-type → include-flags mapping and forced map flags are not wired yet.
- **Path normalization** — no Z-clamp via `Map::GetHeight`; no `IsInvalidDestinationZ`.
- **Pathgen runtime integration** — portable `ShortenPathUntilDist` and `AddFarFromPolyFlags` exist, but are not yet wired into Detour-backed `CalculatePath`/chase runtime because owner `WorldObject`, map height and Detour query context are still missing.
- **Detour-backed query methods** — pure path constants, `NavTerrainFlag`, Detour magic/version constants, `dtNavMeshParams` layout, `dtNavMesh` init/tile ownership, query allocation, `findNearestPoly` and filter wrapper are declared, but `findPath`, `findStraightPath`, `moveAlongSurface`, `getPolyHeight`, `raycast` and tile data runtime still need FFI.
- **Per-instance threading model integration** — Detour `dtNavMeshQuery` is not thread-safe; the Rust wrapper is `!Send + !Sync` and lifetime-bound to its mesh, but `MMapManager` still needs one query per `(instanceMapId, instanceId)` mirroring C++.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- The current `wow_ai::wander` linear-tween bypasses path-finding entirely; creatures will path through walls, off cliffs, into lava — visible immediately on any map with terrain.
- Without `BuildShortcut` fallback, missing mmaps will hard-fail rather than degrade gracefully (C++ falls back to straight-line + `PATHFIND_NOT_USING_PATH`).
- No mmap tile coordinate alignment with the VMap grid is documented anywhere — when both load, they need to agree on `tile_x = floor(64 - y / 533.333)` (note WoW's coord flip).
- Detour `dtPolyRef` is `uint64` — Rust must use `u64` and not `u32` (an easy mistake from older docs).
- The C++ `dtQueryFilter` per-area cost table (`m_areaCost[64]`) needs to be initialised in `CreateFilter`; default `1.0` is fine for ground but lava/slime should be punitive (`100.0`+) — replicate or creatures will path through fire.
- `FindSmoothPath` uses `MAX_SMOOTH_PATH_NODES = 74` (matching `MAX_POINT_PATH_LENGTH`); the buffer is stack-allocated in C++. Rust port must size matching.
- Per-instance query (one `dtNavMeshQuery` per `(mapId, instanceId)`) means dungeon raids cannot share a single query object — the Rust `MapManager` design currently has no plumbing for this.

**Tests existing:**
- Unit tests cover the portable `PathGenerator` slice in `wow-movement`: constants/flags, no-navmesh shortcut fallback, invalid coordinate rejection, path length, geometry helpers, far-from-poly flags, path-length limit and `ShortenPathUntilDist`.
- Unit tests cover `wow-recastdetour` portable header/flag work: `MMAP_MAGIC`, `MMAP_VERSION`, `MmapTileHeader` layout/round-trip/error cases, `.mmtile` blob size/error cases, Detour constants, `dtNavMeshParams` layout/round-trip, nav terrain flag values, tile ID packing, C++ file naming helpers and pre-FFI `MMapManager` map-level load/unload/thread-unsafe behavior.
- Test: `.mmap` map header parser reads/writes the exact 28-byte `dtNavMeshParams` layout (`orig[3]`, `tileWidth`, `tileHeight`, `maxTiles`, `maxPolys`); real `dtNavMesh::init` remains pending behind FFI.
- Unit tests cover real Detour `addTile`/`removeTile` failure paths and a successful insert/remove using tile data generated by legacy Detour's `dtCreateNavMeshData`.
- Unit test covers `dtNavMeshQuery::init(navMesh, 1024)`, matching `MMapManager::loadMapInstance`; query methods are still pending.
- Unit test covers `dtQueryFilter` defaults (`include=0xffff`, `exclude=0`, all area costs `1.0`) and include/exclude/area-cost mutators against real Detour.
- Unit test covers `dtNavMeshQuery::findNearestPoly` using a generated valid tile, default filter and the low C++ `PathGenerator::GetPolyByLocation` extent shape `{3,5,3}`.
- Unit test smoke-initializes a real vendored `dtNavMesh` through the Rust wrapper and verifies `getMaxTiles`.

---

## 9. Migration sub-tasks

<!-- REFINE.022:BEGIN task-wbs -->

### R2 Task WBS (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md` + targets verificados en `docs/migration/inventory/r2-rust-targets.tsv`. C++ sigue siendo el oraculo; estas tareas son el suelo de cobertura por archivo, no una prueba de port correcto.

- [ ] **#MOVEMENT_PATHGEN.WBS.001** Partir y cerrar la migracion auditada de `game/Movement/PathGenerator.cpp`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`
  Rust target: `crates/wow-recastdetour`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `needs_split`; C++ file has 1045 lines; split by public API, state model, persistence, runtime behavior and tests before implementation. Assignment basis: prefix.
- [ ] **#MOVEMENT_PATHGEN.WBS.002** Cerrar la migracion auditada de `game/Movement/PathGenerator.h`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h`
  Rust target: `crates/wow-recastdetour`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.

<!-- REFINE.022:END task-wbs -->

Numbered for cross-reference from `MIGRATION_ROADMAP.md` §5. Complexity: **L** (<1h), **M** (1-4h), **H** (4-12h), **XL** (>12h, splitear).

- [x] **#MOVE-PATH.1** Decide FFI strategy: use vendored legacy Detour from `/home/server/woltk-trinity-legacy/dep/recastnavigation/Detour` copied into `crates/wow-recastdetour/vendor/Detour`, compiled by Cargo `build.rs`; Rust binding layer will be hand-rolled narrow C ABI first, not `recastnavigation-sys`. (M)
- [x] **#MOVE-PATH.2** Vendor or link Recast/Detour C++; produce `build.rs` that compiles `Detour/Source/*.cpp` into a static lib. Done with legacy Detour copied into `crates/wow-recastdetour/vendor/Detour` and compiled by `cc`. (H)
- [ ] **#MOVE-PATH.3** Generate FFI bindings (`bindgen` or hand-rolled) for `dtNavMesh` (`init`, `addTile`, `removeTile`, `getTileAndPolyByRef`, `getMaxTiles`), `dtNavMeshQuery` (`init`, `findNearestPoly`, `findPath`, `findStraightPath`, `closestPointOnPoly`, `moveAlongSurface`, `getPolyHeight`, `raycast`), `dtQueryFilter`. Partial: `dtNavMesh` `alloc/free/init/getMaxTiles/addTile/removeTile`, `dtNavMeshQuery` `alloc/free/init/findNearestPoly` and `dtQueryFilter` include/exclude/area-cost APIs are hand-rolled and tested for smoke/error/success paths with generated Detour tile data where applicable. (XL — split per class)
- [ ] **#MOVE-PATH.4** Safe Rust wrappers: `struct DetourNavMesh(*mut dtNavMesh)` + Drop; `struct DetourNavMeshQuery(*mut dtNavMeshQuery)` + Drop; `struct DetourQueryFilter`. Partial: `DetourNavMesh`, lifetime-bound `DetourNavMeshQuery<'mesh>` and `DetourQueryFilter` wrappers have `Drop` and `!Send + !Sync` markers; tile add/remove and filter mutator coverage exists; query method wrappers pending. (H)
- [x] **#MOVE-PATH.5** `MmapTileHeader` struct + `mmtile` file parser: 20-byte header struct/parser is done with magic/version/size/liquid flag validation, file reader and blob extraction. Handing ownership to `dtNavMesh::addTile` remains in FFI/tile runtime tasks. (M)
- [x] **#MOVE-PATH.6** `.mmap` map header parser: reads `dtNavMeshParams` (origin, tileWidth/Height, maxTiles, maxPolys) as the exact 28-byte C++ layout. (M)
- [x] **#MOVE-PATH.7** `MMapManager` skeleton: C++-like `loadedMMaps` cache, `InitializeThreadUnsafe` placeholders, failed-open placeholders, `parentMapData`, `.mmap` `load_map_data(map_id)` and map-level `unload_map(map_id)` are ported pre-FFI. Tile-level load/unload and queries remain in later tasks. (H)
- [ ] **#MOVE-PATH.8** Per-instance `dtNavMeshQuery` allocator: `MMapData { mesh: DetourNavMesh, queries: HashMap<InstanceId, DetourNavMeshQuery> }`. (M)
- [ ] **#MOVE-PATH.9** On-demand tile load: `load_map_data(map_id, tx, ty)` reads file lazily; track loaded set per map. (H)
- [x] **#MOVE-PATH.10** `NavTerrainFlag` enum from actual `MMapDefines.h`: `EMPTY=0`, `GROUND=1`, `GROUND_STEEP=2`, `WATER=4`, `MAGMA_SLIME=8`; old doc values were corrected after re-checking the legacy tree. Poly-area → flag mapping still belongs to Detour-backed filter work. (L)
- [x] **#MOVE-PATH.11** Port `PathType` bitflags + constants (`MAX_PATH_LENGTH=74`, `SMOOTH_PATH_STEP_SIZE=4.0`, `SMOOTH_PATH_SLOP=0.3`, `INVALID_POLYREF=0`). (L)
- [ ] **#MOVE-PATH.12** `PathGenerator` skeleton: portable state now exists in `wow-movement` (`polyrefs: [u64; 74]`, `path_points`, `_type`, options and start/end/actual-end positions). Remaining work: owner `WorldObject`, `MMapManager`, Detour query/filter ownership and map height/LOS context. (M)
- [ ] **#MOVE-PATH.13** `PathGenerator::create_filter()` with creature-type → include-flags mapping (ground vs water vs flying vs lava-immune). (M)
- [x] **#MOVE-PATH.14** Port `BuildShortcut`/no-mmap fallback: portable `calculate_without_navmesh_like_cpp` builds `[start,end]` and marks `PATHFIND_NORMAL | PATHFIND_NOT_USING_PATH` like C++ `CalculatePath` when mmaps are unavailable. (L)
- [ ] **#MOVE-PATH.15** Port `BuildPolyPath`: `find_nearest_poly(start)` + `find_nearest_poly(end)` + `find_path(start_poly, end_poly, &filter)`. (H)
- [ ] **#MOVE-PATH.16** Port `BuildPointPath`: dispatch between `findStraightPath` (`_useStraightPath=true`) and `FindSmoothPath`. (H)
- [ ] **#MOVE-PATH.17** Port `FindSmoothPath` + `GetSteerTarget` + `FixupCorridor` (the Detour smooth-path tutorial code, identical math). (XL)
- [ ] **#MOVE-PATH.18** Port `NormalizePath` (Z-clamp via `Map::GetHeight`). (M)
- [ ] **#MOVE-PATH.19** Port `IsInvalidDestinationZ` + `AddFarFromPolyFlags` + `ShortenPathUntilDist`. `AddFarFromPolyFlags` and `ShortenPathUntilDist` shape are represented; `IsInvalidDestinationZ`, collision-height hit sphere and real `Map::isInLineOfSight` remain owner-backed gaps. (M)
- [x] **#MOVE-PATH.20** Port `Dist3DSqr` / `InRange` / `InRangeYZX` helpers. (L)
- [ ] **#MOVE-PATH.21** Port `CalculatePath(destX, destY, destZ, force_dest)` top-level entry orchestrating all the above. (H)
- [x] **#MOVE-PATH.22** Port `SetUseStraightPath` / `SetPathLengthLimit` / `SetUseRaycast` setters. (L)
- [ ] **#MOVE-PATH.23** Wire `PathGenerator` into `MoveSplineInit::MoveTo(generatePath=true, ...)` — see [`movement-spline.md`](movement-spline.md) #MOVE-SPL.14. (M)
- [ ] **#MOVE-PATH.24** Snapshot tests: known map + known start/end → expected path waypoints (record from C++ trinity, replay in Rust). (H)
- [ ] **#MOVE-PATH.25** Stress test: 1000 chase paths/sec on Stormwind tile; verify no leaks of `dtNavMeshQuery`. (M)
- [ ] **#MOVE-PATH.26** Config plumbing: `mmap.enabled`, `mmap.allowedMaps` from worldserver.conf. (L)

---

## 10. Regression tests to write

<!-- REFINE.024:BEGIN tests-required -->

### R2 Tests required (generated)

> Fuente: cobertura C++ asignada y targets Rust verificados. Estos gates son obligatorios para cerrar tareas WBS; `n/a` solo vale con razon explicita y referencia C++/producto.

| Gate | Required coverage | Acceptance |
|---|---|---|
| `#MOVEMENT_PATHGEN.TEST.001 / unit` | Unit tests for pure data structures, parsers, state transitions and edge cases directly ported from C++ invariants. C++ scope: 2 files / 1195 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h`. Rust target: `crates/wow-recastdetour`. | `cargo test -p wow-recastdetour` passes for the touched target(s); every migrated behavior has focused tests or an explicit documented n/a. |
| `#MOVEMENT_PATHGEN.TEST.002 / golden` | Golden/fixture tests derived from C++ packet bytes, SQL rows, config defaults, DB2 records or deterministic algorithm outputs. C++ scope: 2 files / 1195 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h`. Rust target: `crates/wow-recastdetour`. | Golden fixtures are checked in or generated by a documented harness; Rust output matches C++ semantics byte-for-byte where wire/data format is involved. |
| `#MOVEMENT_PATHGEN.TEST.003 / integration` | Integration tests for startup/load paths, database access, registry wiring and cross-crate behavior. C++ scope: 2 files / 1195 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h`. Rust target: `crates/wow-recastdetour`. | The module can be loaded/exercised through its real Rust service boundary without panics, missing handlers or silent default-success paths. |
| `#MOVEMENT_PATHGEN.TEST.004 / e2e` | Client/bot or scripted runtime scenario when the module affects login, world session, packets, entities, maps, gameplay or content. C++ scope: 2 files / 1195 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h`. Rust target: `crates/wow-recastdetour`. | Bot/client scenario covers the user-visible path, or the doc marks E2E n/a with a concrete product-scope reason before implementation closes. |

<!-- REFINE.024:END tests-required -->

- [x] Test: `mmtile` header parser validates magic `MMAP` + mmap version and rejects bad magic/version/short headers; real captured tile blob test remains pending until Detour tile data loader exists.
- [ ] Test: `MMapManager::load_map(0)` (Eastern Kingdoms) succeeds and exposes a non-null `dtNavMesh`.
- [ ] Test: `MMapManager` loads tile `(map=0, tx=32, ty=32)` (centre of EK) on demand without preloading.
- [ ] Test: `MMapManager::load_map_instance(map_id=0, instance=0)` allocates a fresh `dtNavMeshQuery`; second call reuses it.
- [x] Test: `PathGenerator::CalculatePath` with mmap disabled / map not in allowlist → represented no-navmesh fallback `PATHFIND_NORMAL | PATHFIND_NOT_USING_PATH` + path = `[start, end]` as C++ `CalculatePath` sets after `BuildShortcut`.
- [ ] Test: `PathGenerator::CalculatePath` with mmaps loaded → `PATHFIND_NORMAL` + path length ≥ Euclidean distance.
- [ ] Test: `PathGenerator::CalculatePath` across two adjacent tiles → smooth path crosses tile boundary without duplicate points.
- [ ] Test: `PathGenerator::CalculatePath` with start far from any poly → `PATHFIND_FARFROMPOLY_START`.
- [ ] Test: `PathGenerator::CalculatePath` with end on inaccessible cliff → `PATHFIND_INCOMPLETE` (path leads to closest reachable point).
- [x] Test: `PathGenerator::ShortenPathUntilDist(target=last_point, dist=3.0)` removes segments until `|last - target| ≈ 3.0`, with LOS represented by callback.
- [ ] Test: `PathGenerator::IsInvalidDestinationZ` rejects Z values 100+ yards above the polygon's vertical bound.
- [ ] Test: `NormalizePath` clamps each Z within 5 yards of `Map::GetHeight(x, y)`.
- [ ] Test: `CreateFilter` for a swimming creature includes `WATER` and excludes `MAGMA|SLIME`.
- [ ] Test: `CreateFilter` for a flying creature includes `EMPTY` and skips ground checks.
- [ ] Test: `dtQueryFilter` area cost for `MAGMA` ≥ 50.0 (creatures avoid lava).
- [x] Test: Path length limit `SetPathLengthLimit(50.0)` produces 12 points after C++ integer truncation (`uint32(50 / 4.0)`).
- [ ] Test: Far-tile path (`tile A` → `tile B` neither loaded) triggers on-demand load of both tiles.
- [ ] Test: Concurrent calls from two map instances each use their own `dtNavMeshQuery` (`!Send + !Sync` boundaries hold).
- [ ] Test: Drop semantics: dropping `DetourNavMesh` calls `dtFreeNavMesh`; dropping `DetourNavMeshQuery` calls `dtFreeNavMeshQuery` (LeakSanitizer clean).
- [ ] Test: Snapshot — Stormwind path from (-8949, -132, 84) to (-8949, -50, 84) matches recorded C++ output within 0.1y per waypoint.

---

## 11. Notes / gotchas

<!-- REFINE.025:BEGIN product-scope -->

### R2 Product scope / exclusions (generated)

> Fuente: cabecera del doc + inventario C++ asignado. Ninguna marca de alcance elimina C++ del backlog: solo define si se implementa, se sustituye por idiom Rust o se desactiva explicitamente para producto.

| Scope | Decision | C++ retained | Evidence |
|---|---|---|---|
| `active_port_scope` | Full C++ surface remains in migration scope; no product exclusion recorded. | 2 files / 1195 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h` | `crates/wow-recastdetour/` + `crates/wow-movement/src/path_generator.rs` are active partial ports; Detour FFI, tile loader, query/filter bridge and full `PathGenerator` remain open |

<!-- REFINE.025:END product-scope -->

<!-- REFINE.023:BEGIN known-divergences -->

### R2 Known divergences / bugs (generated)

> Fuente: C++ asignado en `cpp-files-by-module.md` + target Rust verificado en `r2-rust-targets.tsv`. Esto enumera divergencias estructurales conocidas; no sustituye la auditoria funcional contra C++ antes de cerrar tareas.

| ID | Rust evidence | C++ evidence | Status | Notes |
|---|---|---|---|---|
| `#MOVEMENT_PATHGEN.DIV.001` | `crates/wow-recastdetour` (`partial_port`) | 2 C++ files / 1195 lines assigned; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h` | `partial` | Portable headers, params and map cache exist; Detour FFI and tile/query runtime absent. |
| `#MOVEMENT_PATHGEN.DIV.002` | `crates/wow-movement/src/path_generator.rs` (`partial_port`) | 2 C++ files / 1195 lines assigned; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h` | `partial` | Owner-independent fallback slice exists; Detour-backed path calculation remains absent. |
| `#MOVEMENT_PATHGEN.DIV.003` | `crates/wow-recastdetour/src/lib.rs` (`partial_port`) | 2 C++ files / 1195 lines assigned; refs: `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.h` | `partial` | File is no longer empty; the missing surface is `dtNavMeshQuery` methods, tile runtime loading and pathgen-safe method wrappers/filter integration. |

<!-- REFINE.023:END known-divergences -->

- **Detour is not thread-safe.** `dtNavMesh` (read-only after load) is shareable, but `dtNavMeshQuery` holds per-search state and **must** be one-per-thread or one-per-map-instance. C++ keys it `(mapId, instanceId)`; the Rust `MapManager` design must match — sharing across threads will crash or silently corrupt paths.
- **`dtPolyRef` is `uint64`**, NOT `uint32`. Old TC docs sometimes show `uint32`. Use `u64`.
- **WoW coordinate flip.** Detour internally uses `(x, y_up, z)` with `y` as the up axis. WoW uses `(x, y, z_up)`. C++ `PathGenerator` does the swap inline (`InRangeYZX` is a giveaway — Y/Z/X order). Replicate or all paths are visibly off-axis.
- **Tile coordinates.** A WoW map is 64×64 grid of 533.333-yard tiles. `tx = 32 - floor(y / 533.333)`, `ty = 32 - floor(x / 533.333)` — note the swap and the negation. Off-by-one here makes the wrong tile load and `findNearestPoly` returns INVALID_POLYREF.
- **`MAX_PATH_LENGTH = 74` is canonical.** Trinity comment: "74 * 4.0 = 296y, way more than evade range". If you raise it, smooth-path `SMOOTH_PATH_STEP_SIZE=4.0` cost rises linearly. Don't change without good reason.
- **`SMOOTH_PATH_SLOP = 0.3f`** is the steer-target tolerance. Lower → snappier; higher → more direct. Match C++.
- **`BuildShortcut` is the safety net.** When mmaps are disabled or the map isn't in the allowlist, the path becomes `[start, end]` with `PATHFIND_NOT_USING_PATH`. Generators must be tolerant of this — chase will still work visually but creatures path through walls. Document this as a known degraded mode, not a bug.
- **`AddFarFromPolyFlags`** is set when start or end snap distance exceeds tolerance (~3y horizontal, 6y vertical). It's a *flag*, not an error; callers can choose to ignore or to log/teleport-back.
- **`UpdateFilter`** must be called whenever the owner state changes (enters water, takes off, gains levitate aura). Otherwise path filter is stale and creature pathing through water freezes.
- **Per-instance `dtNavMeshQuery` allocation is non-trivially memory-expensive** (~2-4 MiB per query depending on `MAX_NODES`). For dungeons with many phases this adds up — share where the dungeon ID matches and contexts agree.
- **`_useRaycast` is a chase-specific optimization.** When the target is straight-line LOS-clear, skip the smooth path and just do a raycast — faster and visually identical. Don't apply this to evade or flee (need actual avoidance).
- **`closestPointOnPoly` vs `closestPointOnPolyBoundary`**. The former snaps to the polygon area (3D); the latter to its 2D edge. Use the right one or `actualEndPosition` is wrong.
- **`MmapTileHeader` magic and version.** `MMAP_MAGIC = 'MMAP'` (4 bytes), `MMAP_VERSION = 15` for WotLK 3.4.3. If the offline `mmaps_generator` produced a different version, fail loudly — otherwise creatures path on stale geometry.
- **Memory layout of `dtNavMeshTile`** is endian-dependent. The `mmaps_generator` runs on the build host; if its endian differs from the runtime host (rare today), tiles need byte-swapping. Document the assumption.
- **`dtNavMesh::addTile` takes ownership** of the tile blob memory. The Rust FFI must `Box::leak` or use `dtAllocSetCustom` to keep the buffer alive for the lifetime of the navmesh. Bad lifetime here = use-after-free crashes minutes after start.
- **Liquids tile flag.** The `MmapTileHeader::usesLiquids` bit determines whether the tile's polys carry `WATER`/`MAGMA`/`SLIME` area flags. If set, runtime can use these for filter; if unset, must consult VMap liquid info instead. Replicate the dual-source logic.
- **Pathfind during creature respawn.** Calling `PathGenerator::CalculatePath` before the creature is added to the map (and thus before `MMapManager::loadMapInstance` for its instance) crashes — caller must check `Unit::IsInWorld()` first.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class PathGenerator` | `struct PathGenerator<'a> { source: &'a WorldObject, mesh: &'a DetourNavMesh, query: &'a DetourNavMeshQuery, filter: DetourQueryFilter, path_polyrefs: [u64; 74], poly_length: u32, path_points: Vec<Vec3>, ty: PathType, use_straight_path: bool, force_destination: bool, point_path_limit: u32, use_raycast: bool, start_pos: Vec3, end_pos: Vec3, actual_end_pos: Vec3 }` | Borrow nav mesh + query from `MMapManager` |
| `enum PathType` | `bitflags! struct PathType: u8 { const BLANK = 0; const NORMAL = 0x01; ... }` | Bitwise combos preserved |
| `dtPolyRef` (uint64) | `u64` | Native |
| `dtNavMesh*` | `struct DetourNavMesh(*mut sys::dtNavMesh)` + `Drop` | `!Send + !Sync` (read OK on multiple threads only because Detour read paths are reentrant — document carefully) |
| `dtNavMeshQuery*` | `struct DetourNavMeshQuery(*mut sys::dtNavMeshQuery)` + `Drop` | `!Send + !Sync` strictly per-instance |
| `dtQueryFilter` | `struct DetourQueryFilter { include: u16, exclude: u16, area_cost: [f32; 64] }` (copy semantics) | Owned per `PathGenerator` |
| `dtPoly` | accessed only through Detour FFI, not exposed to Rust | — |
| `enum NavTerrain` | `bitflags! struct NavTerrain: u16 { const GROUND = 1; const MAGMA = 2; const SLIME = 4; const WATER = 8; const EMPTY = 16; }` | — |
| `MMapManager` (singleton) | `struct MMapManager { maps: DashMap<u32, MMapData>, base_path: PathBuf, allowed_maps: HashSet<u32>, enabled: bool }` | DashMap for concurrent map load; `OnceCell` for global access |
| `MMapData` | `struct MMapData { mesh: DetourNavMesh, queries: DashMap<u32, DetourNavMeshQuery>, loaded_tiles: HashSet<(u8, u8)> }` | Per-instance query map |
| `MmapTileHeader` | `#[repr(C, packed)] struct MmapTileHeader { magic: [u8; 4], dt_version: u32, mmap_version: u32, size: u32, uses_liquids: u8, _padding: [u8; 3] }` | Matches on-disk layout |
| `MAX_PATH_LENGTH` / `MAX_POINT_PATH_LENGTH` | `const MAX_PATH_LENGTH: usize = 74;` | — |
| `SMOOTH_PATH_STEP_SIZE` / `SMOOTH_PATH_SLOP` | `const SMOOTH_PATH_STEP_SIZE: f32 = 4.0;` / `const SMOOTH_PATH_SLOP: f32 = 0.3;` | — |
| `INVALID_POLYREF` | `const INVALID_POLYREF: u64 = 0;` | — |
| `Movement::PointsArray` | `Vec<Vec3>` | — |
| `G3D::Vector3` | `glam::Vec3` | Already in workspace |
| `bool PathGenerator::CalculatePath(x, y, z, force)` | `pub fn calculate_path(&mut self, dest: Vec3, force_dest: bool) -> bool` | — |
| `void NormalizePath()` | `fn normalize_path(&mut self, map: &Map)` | Pass `&Map` to query height |
| `dtStatus` (Detour return) | `Result<(), DetourError>` | Wrap status codes into typed error |
| `dtVdist` / `dtVcopy` / `dtVlerp` | `glam::Vec3::distance` / `Vec3` ops | Don't cross FFI for math |

---

*Template version: 1.0 (2026-05-01).* Last updated: 2026-05-01.

---

## 13. Audit (2026-05-01)

**Scope.** Cross-checked `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.{h,cpp}` (150 + 1045 lines) and the supporting `src/common/Collision/Management/MMapManager.{h,cpp}` + `src/common/Collision/Maps/MMapDefines.h` (~80) + `dep/recastnavigation/Detour/` against the Rust workspace at `/home/server/rustycore/crates/`. Note: this WotLK 3.4.3 source tree does **not** ship a separate `PathFinderTerrainInfo.cpp` — terrain queries are inlined into `PathGenerator::GetNavTerrain` and `Map::GetHeight`. Verified by directory listing.

**`wow-recastdetour` has the first real Detour FFI slice, but not the full query/runtime layer.** The workspace now has a portable `MMapDefines.h`/Detour header slice (`MMAP_MAGIC`, `MMAP_VERSION`, `MmapTileHeader`, `dtNavMeshParams`, Detour constants, file naming helpers and map-level `MMapManager` state), vendored legacy Detour C++ built by Cargo, a narrow `dtNavMesh` wrapper for allocation/init/free/addTile/removeTile, a `dtNavMeshQuery` allocation/init/free wrapper matching `MMapManager::loadMapInstance`, `findNearestPoly`, and a `dtQueryFilter` flags/area-cost wrapper. Remaining path query methods and the `MMapManager` tile/query runtime are still absent.

**`PathGenerator` real Detour branch: absent.** The C++ class is **1045 lines** of pathfinding logic — `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `BuildShortcut`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, `AddFarFromPolyFlags`, `ShortenPathUntilDist`, `CreateFilter`, `UpdateFilter`, `IsInvalidDestinationZ`, plus 8 geometry helpers. Rust now ships the portable owner-independent subset (`PathType`, constants, no-navmesh `BuildShortcut` fallback, geometry helpers, path length and `ShortenPathUntilDist` shape), but none of the Detour-backed methods are operational yet.

**`MMapManager` tile/query runtime: absent.** Rust now has the map-level `loadedMMaps` cache and `.mmap` param reader, `dtNavMesh::init/addTile/removeTile` has smoke-tested FFI, and standalone `dtNavMeshQuery::init(navMesh, 1024)` is covered. The on-demand `.mmtile` loader (`mmaps/{mapid:04}{x:02}{y:02}.mmtile`), per-instance query map and runtime tile ownership handoff are still absent. The runtime cannot load real Detour tile blobs from disk yet.

**Filter and area cost integration: absent.** The raw `dtQueryFilter` wrapper can set include/exclude flags and per-area costs, but the C++ `CreateFilter`/`UpdateFilter` setup is not connected yet: ground/water/magma/slime area costs, per-creature include flags, forced map flags and in-water/in-combat adjustments still need a Rust owner context. Without this setup, every creature would still use only whichever raw filter the caller manually configures.

**Threading model: partly represented, not integrated.** Detour `dtNavMeshQuery` is **not thread-safe**; C++ keys it `(mapId, instanceId)` with one query per dungeon instance. The Rust wrapper is `!Send + !Sync` and tied to the mesh lifetime, but the Rust `MapManager` still has no per-instance query map. Adding navmesh queries naively into the existing `Arc<RwLock<MapManager>>` will either deadlock under concurrent path-find or share unsafe state across instances.

**Worst divergence.** This is the single **largest external-dependency greenfield** in the engine. Beyond the pure code port, the Rust workspace must (a) decide on an FFI strategy (vendored Detour vs `recastnavigation-sys` crate), (b) wire C++ build into Cargo via `build.rs`, (c) generate or write FFI bindings for ~15 Detour classes/functions, (d) build safe Rust wrappers with `Drop` and `!Send + !Sync` correctly applied, (e) port the on-disk `.mmap`/`.mmtile` parser, (f) port the 1045-line `PathGenerator` itself, (g) plumb per-instance queries into `MapManager`, and only then can [`movement-generators.md`](movement-generators.md) (Chase, Follow, Point, Waypoint, Confused, Fleeing) call `PathGenerator::CalculatePath`. Until this lands, every server-driven creature motion either snaps in straight lines (`BuildShortcut` fallback equivalent — the only mode currently working in `wow_ai::wander`) or pathfinding is unavailable, with creatures pathing through walls and off cliffs visible from the first session. Estimated XL across §9 tasks #MOVE-PATH.1 → #MOVE-PATH.26, with #MOVE-PATH.3 (Detour FFI) and #MOVE-PATH.17 (`FindSmoothPath` port) being the biggest individual chunks.
