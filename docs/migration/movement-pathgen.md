# Migration: Movement / PathGenerator (Detour navmesh consumer)

> **C++ canonical path:** `src/server/game/Movement/PathGenerator.{h,cpp}` + Detour bridge from `src/server/collision/Management/MMapManager.{h,cpp}` + `src/server/collision/Management/MMapDefines.h` + 3rdparty `dep/recastnavigation/Detour/`
> **Rust target crate(s):** `crates/wow-recastdetour/` (currently 0-line placeholder; FFI bindings to Recast/Detour) + future `crates/wow-movement/src/path_generator.rs`
> **Layer:** L5 sub-module (depends on `wow-collision` / VMaps L3 for height fallback, mmap tile loading from disk)
> **Status:** ❌ not started — `wow-recastdetour` is a 0-line placeholder; no `PathGenerator`; no mmap tile loader
> **Audited vs C++:** ✅ complete 2026-05-01
> **Last updated:** 2026-05-01

> Sub-doc of [`movement.md`](movement.md). Cross-links: [`movement-generators.md`](movement-generators.md) (`Chase`/`Follow`/`Point`/`Waypoint`/`Confused`/`Fleeing` are the consumers), [`movement-spline.md`](movement-spline.md) (`MoveSplineInit::MoveTo` invokes `PathGenerator::CalculatePath`), [`common-collision.md`](common-collision.md) (height/LOS via VMaps; mmap tile coordinates align with VMap grid), [`ai-base.md`](ai-base.md) (AI scripts indirectly trigger pathfind via generators).

---

## 1. Purpose

Bridges server motion to **walkable terrain**: given a `(start, end)` pair, produce a polyline that follows ground/water/transitions, avoids obstacles (trees, walls, cliffs), and respects creature affinity (ground vs water vs lava). Wraps Recast/Detour's `dtNavMesh` + `dtNavMeshQuery` with a per-`WorldObject` `dtQueryFilter`. Loads precomputed `mmtile` files (533.333-yard tiles, one per map grid cell) on demand and queries them via Detour's funnel algorithm + smooth path post-processing. Used by chase, follow, point, waypoint, confused, fleeing — anything that needs to "move there" rather than "snap there".

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Movement/PathGenerator.h` | 150 | Class definition + `PathType` enum + constants (`MAX_PATH_LENGTH=74`, `SMOOTH_PATH_STEP_SIZE=4.0f`, `SMOOTH_PATH_SLOP=0.3f`, `INVALID_POLYREF=0`) + private smooth-path helpers |
| `src/server/game/Movement/PathGenerator.cpp` | 1045 | All path computation: `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `BuildShortcut`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, `AddFarFromPolyFlags`, `ShortenPathUntilDist`, filter setup |
| `src/server/collision/Management/MMapManager.h` | ~150 | Singleton tile cache: `loadMap(mapId)`, `unloadMap`, `loadMapInstance(mapId, instanceId)`, `loadMapData(mapId, x, y)`, `getNavMesh(mapId)`, `getNavMeshQuery(mapId, instanceId)` |
| `src/server/collision/Management/MMapManager.cpp` | ~600 | Reads `mmaps/{mapid:03}.mmap` header + per-tile `{mapid:03}{tx:02}{ty:02}.mmtile` files; calls `dtNavMesh::addTile` |
| `src/server/collision/Management/MMapDefines.h` | ~80 | `NavTerrain` enum (GROUND=1, MAGMA=2, SLIME=4, WATER=8, EMPTY=16) + `MMAP_MAGIC` + `MMAP_VERSION` |
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
| `dtNavMesh` | 3rdparty class | Loaded navmesh per-map; tile-addressable |
| `dtNavMeshQuery` | 3rdparty class | Query object holding the path-find buffers; per-instance because not thread-safe |
| `dtQueryFilter` | 3rdparty class | Per-`WorldObject` filter: include/exclude `NavTerrain` flags; per-area cost |
| `dtPoly` | 3rdparty struct | Polygon header within a tile |
| `NavTerrainFlag` | enum | `GROUND=1`, `MAGMA=2`, `SLIME=4`, `WATER=8`, `EMPTY=16` (and bitwise combinations) |
| `MMapManager` (singleton) | class | Tile cache; `loadMap(mapId)`, `loadMapInstance`, per-instance `dtNavMeshQuery` map |
| `MMapData` (per-map) | struct | `dtNavMesh*` + per-instance `unordered_map<uint32, dtNavMeshQuery*>` + loaded tile set |
| `MmapTileHeader` | struct | On-disk header: `mmapMagic`, `dtVersion`, `mmapVersion`, `size`, `usesLiquids` |
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
| `MMapManager::loadMap(mapId)` | Open `mmaps/{mapid:03}.mmap` header file | file I/O |
| `MMapManager::loadMapData(mapId, tx, ty)` | Load `mmaps/{mapid:03}{tx:02}{ty:02}.mmtile`; `dtNavMesh::addTile` | file I/O, Detour |
| `MMapManager::loadMapInstance(mapId, instanceId)` | Allocate per-instance `dtNavMeshQuery` (Detour query is not thread-safe; one per instance) | Detour `init` |
| `MMapManager::unloadMap(mapId)` / `unloadMapInstance` | Tear down | Detour `removeTile`, `dtFreeNavMeshQuery` |
| `MMapManager::getNavMesh(mapId)` / `getNavMeshQuery(mapId, instanceId)` | Accessors used by `PathGenerator` ctor | — |

---

## 5. Module dependencies

**Depends on:**
- `dep/recastnavigation/Detour/` — entire Detour runtime: `dtNavMesh`, `dtNavMeshQuery`, `dtQueryFilter`, `dtPoly`, `dtVdist`/`dtVlerp` math.
- `MMapManager` (in `src/server/collision/Management/`) — loads `.mmap`/`.mmtile` files from disk; per-map and per-instance Detour query allocation.
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
| `mmaps/{mapid:03}.mmap` | Binary header (`MmapTileHeader`-like) | `MMapManager::loadMap` |
| `mmaps/{mapid:03}{tx:02}{ty:02}.mmtile` | Detour tile (header + polys + verts + dtNavMeshTile blob) | `MMapManager::loadMapData` → `dtNavMesh::addTile` |
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

**Files in `/home/server/rustycore`:**
- `crates/wow-recastdetour/Cargo.toml` — declared dependency stub.
- `crates/wow-recastdetour/src/lib.rs` — **0 lines of actual FFI code** (placeholder).
- *No* `crates/wow-movement/src/path_generator.rs`.
- *No* `mmap` tile loader anywhere.
- *No* Detour FFI bindings (no `cxx`, `bindgen`, or `recastnavigation-sys` integration).

**What's implemented:**
- *Nothing functional.* `wow-recastdetour` is an empty crate listed in the workspace `Cargo.toml` so future code has a place to land. There is no static or dynamic link to Recast/Detour, no header generation, and no test harness.

**What's missing vs C++:**
- **`PathGenerator` class** — 1045 lines of C++ code defining `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `BuildShortcut`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, `AddFarFromPolyFlags`, `ShortenPathUntilDist` — none ported.
- **`PathType` enum / bitfield** — does not exist.
- **`dtNavMesh` / `dtNavMeshQuery` / `dtQueryFilter` FFI** — no Rust wrapper, no safety abstractions, no lifetime model. The Detour C++ runtime is not linked.
- **`MMapManager`** — no tile cache, no `loadMap(mapId)`, no `.mmap`/`.mmtile` reader, no per-instance query allocation.
- **`NavTerrain` flags** — enum does not exist.
- **`mmtile` file format reader** — no parser. The on-disk binary layout (`mmapMagic`, `dtVersion`, `mmapVersion`, `size`, `usesLiquids`, then a Detour tile blob) is not implemented.
- **Filter creation** — no `CreateFilter` / `UpdateFilter`; no creature-type → include-flags mapping.
- **Path normalization** — no Z-clamp via `Map::GetHeight`; no `IsInvalidDestinationZ`.
- **Chase shortening** — no `ShortenPathUntilDist`; chase will overshoot once it lands.
- **Far-from-poly diagnostics** — no `AddFarFromPolyFlags`; no way to detect if a creature got pushed off the navmesh.
- **Constants** — `MAX_PATH_LENGTH=74`, `MAX_POINT_PATH_LENGTH=74`, `SMOOTH_PATH_STEP_SIZE=4.0`, `SMOOTH_PATH_SLOP=0.3`, `INVALID_POLYREF=0` — not declared.
- **Per-instance threading model** — Detour `dtNavMeshQuery` is not thread-safe; the Rust port must allocate one per `MapManager` instance, mirroring C++. No design for this exists.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- The current `wow_ai::wander` linear-tween bypasses path-finding entirely; creatures will path through walls, off cliffs, into lava — visible immediately on any map with terrain.
- Without `BuildShortcut` fallback, missing mmaps will hard-fail rather than degrade gracefully (C++ falls back to straight-line + `PATHFIND_NOT_USING_PATH`).
- No mmap tile coordinate alignment with the VMap grid is documented anywhere — when both load, they need to agree on `tile_x = floor(64 - y / 533.333)` (note WoW's coord flip).
- Detour `dtPolyRef` is `uint64` — Rust must use `u64` and not `u32` (an easy mistake from older docs).
- The C++ `dtQueryFilter` per-area cost table (`m_areaCost[64]`) needs to be initialised in `CreateFilter`; default `1.0` is fine for ground but lava/slime should be punitive (`100.0`+) — replicate or creatures will path through fire.
- `FindSmoothPath` uses `MAX_SMOOTH_PATH_NODES = 74` (matching `MAX_POINT_PATH_LENGTH`); the buffer is stack-allocated in C++. Rust port must size matching.
- Per-instance query (one `dtNavMeshQuery` per `(mapId, instanceId)`) means dungeon raids cannot share a single query object — the Rust `MapManager` design currently has no plumbing for this.

**Tests existing:**
- 0 tests for `PathGenerator` (does not exist).
- 0 tests for mmap tile load (no loader).
- 0 tests for Detour FFI wrapper (no FFI).
- The `wow-recastdetour` crate has no `#[cfg(test)]`.

---

## 9. Migration sub-tasks

Numbered for cross-reference from `MIGRATION_ROADMAP.md` §5. Complexity: **L** (<1h), **M** (1-4h), **H** (4-12h), **XL** (>12h, splitear).

- [ ] **#MOVE-PATH.1** Decide FFI strategy: `recastnavigation-sys` crate from crates.io vs vendored `dep/recastnavigation` + `cxx` bridge. Document choice. (M)
- [ ] **#MOVE-PATH.2** Vendor or link Recast/Detour C++; produce `build.rs` that compiles `Detour/Source/*.cpp` into a static lib. (H)
- [ ] **#MOVE-PATH.3** Generate FFI bindings (`bindgen` or hand-rolled) for `dtNavMesh` (`init`, `addTile`, `removeTile`, `getTileAndPolyByRef`, `getMaxTiles`), `dtNavMeshQuery` (`init`, `findNearestPoly`, `findPath`, `findStraightPath`, `closestPointOnPoly`, `moveAlongSurface`, `getPolyHeight`, `raycast`), `dtQueryFilter`. (XL — split per class)
- [ ] **#MOVE-PATH.4** Safe Rust wrappers: `struct DetourNavMesh(*mut dtNavMesh)` + Drop; `struct DetourNavMeshQuery(*mut dtNavMeshQuery)` + Drop; `struct DetourQueryFilter`. Document `!Send + !Sync` bounds (Detour is not thread-safe). (H)
- [ ] **#MOVE-PATH.5** `MmapTileHeader` struct + `mmtile` file parser: validate magic + version, read tile blob, hand to `dtNavMesh::addTile`. (M)
- [ ] **#MOVE-PATH.6** `.mmap` map header parser: reads `dtNavMeshParams` (origin, tileWidth/Height, maxTiles, maxPolys). (M)
- [ ] **#MOVE-PATH.7** `MMapManager` skeleton: `HashMap<MapId, MMapData>` cache; `load_map(map_id)` + `unload_map`. (H)
- [ ] **#MOVE-PATH.8** Per-instance `dtNavMeshQuery` allocator: `MMapData { mesh: DetourNavMesh, queries: HashMap<InstanceId, DetourNavMeshQuery> }`. (M)
- [ ] **#MOVE-PATH.9** On-demand tile load: `load_map_data(map_id, tx, ty)` reads file lazily; track loaded set per map. (H)
- [ ] **#MOVE-PATH.10** `NavTerrain` flags enum (`GROUND=1`, `MAGMA=2`, `SLIME=4`, `WATER=8`, `EMPTY=16`); poly-area → flag mapping. (L)
- [ ] **#MOVE-PATH.11** Port `PathType` bitflags + constants (`MAX_PATH_LENGTH=74`, `SMOOTH_PATH_STEP_SIZE=4.0`, `SMOOTH_PATH_SLOP=0.3`, `INVALID_POLYREF=0`). (L)
- [ ] **#MOVE-PATH.12** `PathGenerator` skeleton: holds `&MMapManager`, `owner` reference, `polyrefs: [u64; 74]`, `path_points: Vec<Vec3>`, `_type: PathType`, filter. (M)
- [ ] **#MOVE-PATH.13** `PathGenerator::create_filter()` with creature-type → include-flags mapping (ground vs water vs flying vs lava-immune). (M)
- [ ] **#MOVE-PATH.14** Port `BuildShortcut` (no-mmap fallback: 2-point straight line). (L)
- [ ] **#MOVE-PATH.15** Port `BuildPolyPath`: `find_nearest_poly(start)` + `find_nearest_poly(end)` + `find_path(start_poly, end_poly, &filter)`. (H)
- [ ] **#MOVE-PATH.16** Port `BuildPointPath`: dispatch between `findStraightPath` (`_useStraightPath=true`) and `FindSmoothPath`. (H)
- [ ] **#MOVE-PATH.17** Port `FindSmoothPath` + `GetSteerTarget` + `FixupCorridor` (the Detour smooth-path tutorial code, identical math). (XL)
- [ ] **#MOVE-PATH.18** Port `NormalizePath` (Z-clamp via `Map::GetHeight`). (M)
- [ ] **#MOVE-PATH.19** Port `IsInvalidDestinationZ` + `AddFarFromPolyFlags` + `ShortenPathUntilDist`. (M)
- [ ] **#MOVE-PATH.20** Port `Dist3DSqr` / `InRange` / `InRangeYZX` helpers. (L)
- [ ] **#MOVE-PATH.21** Port `CalculatePath(destX, destY, destZ, force_dest)` top-level entry orchestrating all the above. (H)
- [ ] **#MOVE-PATH.22** Port `SetUseStraightPath` / `SetPathLengthLimit` / `SetUseRaycast` setters. (L)
- [ ] **#MOVE-PATH.23** Wire `PathGenerator` into `MoveSplineInit::MoveTo(generatePath=true, ...)` — see [`movement-spline.md`](movement-spline.md) #MOVE-SPL.14. (M)
- [ ] **#MOVE-PATH.24** Snapshot tests: known map + known start/end → expected path waypoints (record from C++ trinity, replay in Rust). (H)
- [ ] **#MOVE-PATH.25** Stress test: 1000 chase paths/sec on Stormwind tile; verify no leaks of `dtNavMeshQuery`. (M)
- [ ] **#MOVE-PATH.26** Config plumbing: `mmap.enabled`, `mmap.allowedMaps` from worldserver.conf. (L)

---

## 10. Regression tests to write

- [ ] Test: `mmtile` parser validates magic `MMAP` + version on a real captured tile blob.
- [ ] Test: `MMapManager::load_map(0)` (Eastern Kingdoms) succeeds and exposes a non-null `dtNavMesh`.
- [ ] Test: `MMapManager` loads tile `(map=0, tx=32, ty=32)` (centre of EK) on demand without preloading.
- [ ] Test: `MMapManager::load_map_instance(map_id=0, instance=0)` allocates a fresh `dtNavMeshQuery`; second call reuses it.
- [ ] Test: `PathGenerator::CalculatePath` with mmap disabled / map not in allowlist → `PATHFIND_SHORTCUT | PATHFIND_NOT_USING_PATH` + path = `[start, end]`.
- [ ] Test: `PathGenerator::CalculatePath` with mmaps loaded → `PATHFIND_NORMAL` + path length ≥ Euclidean distance.
- [ ] Test: `PathGenerator::CalculatePath` across two adjacent tiles → smooth path crosses tile boundary without duplicate points.
- [ ] Test: `PathGenerator::CalculatePath` with start far from any poly → `PATHFIND_FARFROMPOLY_START`.
- [ ] Test: `PathGenerator::CalculatePath` with end on inaccessible cliff → `PATHFIND_INCOMPLETE` (path leads to closest reachable point).
- [ ] Test: `PathGenerator::ShortenPathUntilDist(target=last_point, dist=3.0)` removes segments until `|last - target| ≈ 3.0`.
- [ ] Test: `PathGenerator::IsInvalidDestinationZ` rejects Z values 100+ yards above the polygon's vertical bound.
- [ ] Test: `NormalizePath` clamps each Z within 5 yards of `Map::GetHeight(x, y)`.
- [ ] Test: `CreateFilter` for a swimming creature includes `WATER` and excludes `MAGMA|SLIME`.
- [ ] Test: `CreateFilter` for a flying creature includes `EMPTY` and skips ground checks.
- [ ] Test: `dtQueryFilter` area cost for `MAGMA` ≥ 50.0 (creatures avoid lava).
- [ ] Test: Path length limit `SetPathLengthLimit(50.0)` produces ≤ 13 points (50 / 4.0 = 12.5).
- [ ] Test: Far-tile path (`tile A` → `tile B` neither loaded) triggers on-demand load of both tiles.
- [ ] Test: Concurrent calls from two map instances each use their own `dtNavMeshQuery` (`!Send + !Sync` boundaries hold).
- [ ] Test: Drop semantics: dropping `DetourNavMesh` calls `dtFreeNavMesh`; dropping `DetourNavMeshQuery` calls `dtFreeNavMeshQuery` (LeakSanitizer clean).
- [ ] Test: Snapshot — Stormwind path from (-8949, -132, 84) to (-8949, -50, 84) matches recorded C++ output within 0.1y per waypoint.

---

## 11. Notes / gotchas

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

**Scope.** Cross-checked `/home/server/woltk-trinity-legacy/src/server/game/Movement/PathGenerator.{h,cpp}` (150 + 1045 lines) and the supporting `src/server/collision/Management/MMapManager.{h,cpp}` + `MMapDefines.h` (~80) + `dep/recastnavigation/Detour/` against the Rust workspace at `/home/server/rustycore/crates/`. Note: this WotLK 3.4.3 source tree does **not** ship a separate `PathFinderTerrainInfo.cpp` — terrain queries are inlined into `PathGenerator::GetNavTerrain` and `Map::GetHeight`. Verified by directory listing.

**`wow-recastdetour` is a 0-line placeholder.** The workspace declares `crates/wow-recastdetour/` with a `Cargo.toml` shell, but `src/lib.rs` contains **0 lines of FFI code, 0 bindings, and 0 link to Recast/Detour**. There is no `recastnavigation-sys` dependency, no `cxx` bridge, no `bindgen` invocation, no vendored Detour headers. The C++ runtime (15+ classes across `dtNavMesh`, `dtNavMeshQuery`, `dtQueryFilter`, plus the math helpers) is entirely absent.

**`PathGenerator` class: absent.** The C++ class is **1045 lines** of pathfinding logic — `CalculatePath`, `BuildPolyPath`, `BuildPointPath`, `BuildShortcut`, `FindSmoothPath`, `FixupCorridor`, `GetSteerTarget`, `NormalizePath`, `AddFarFromPolyFlags`, `ShortenPathUntilDist`, `CreateFilter`, `UpdateFilter`, `IsInvalidDestinationZ`, plus 8 geometry helpers. **Rust ships 0 lines.** No struct, no methods, no constants (`MAX_PATH_LENGTH`, `SMOOTH_PATH_STEP_SIZE`, `SMOOTH_PATH_SLOP`, `INVALID_POLYREF`), no `PathType` enum.

**`MMapManager` tile cache: absent.** The on-demand `.mmtile` loader (`mmaps/{mapid:03}{tx:02}{ty:02}.mmtile`), `.mmap` map header reader, per-instance `dtNavMeshQuery` allocator, `MmapTileHeader` magic/version validator — none exist in Rust. There is no on-disk reader for the offline-generated mmap tiles. The runtime cannot load Detour data even if FFI were wired.

**Filter and area cost: absent.** The C++ `dtQueryFilter` setup distinguishes ground/water/magma/slime/empty navigation areas with per-area cost (lava ≈ 100×, water 1.5× for swimming creatures, etc.) and per-creature include flags (flying creatures skip ground; swimmers include water). Rust has none of this — there is no `NavTerrain` enum, no `CreateFilter`, no `UpdateFilter`. Without filter setup, even if Detour were linked, every creature would happily path through lava.

**Threading model: undesigned.** Detour `dtNavMeshQuery` is **not thread-safe**; C++ keys it `(mapId, instanceId)` with one query per dungeon instance. The Rust `MapManager` (recently landed, see `crates/wow-world/src/map_manager.rs`) has 64×64 grids and per-map sharing but **no plumbing for per-instance Detour queries**. Adding navmesh queries naively into the existing `Arc<RwLock<MapManager>>` will either deadlock under concurrent path-find or share unsafe state across instances.

**Worst divergence.** This is the single **largest external-dependency greenfield** in the engine. Beyond the pure code port, the Rust workspace must (a) decide on an FFI strategy (vendored Detour vs `recastnavigation-sys` crate), (b) wire C++ build into Cargo via `build.rs`, (c) generate or write FFI bindings for ~15 Detour classes/functions, (d) build safe Rust wrappers with `Drop` and `!Send + !Sync` correctly applied, (e) port the on-disk `.mmap`/`.mmtile` parser, (f) port the 1045-line `PathGenerator` itself, (g) plumb per-instance queries into `MapManager`, and only then can [`movement-generators.md`](movement-generators.md) (Chase, Follow, Point, Waypoint, Confused, Fleeing) call `PathGenerator::CalculatePath`. Until this lands, every server-driven creature motion either snaps in straight lines (`BuildShortcut` fallback equivalent — the only mode currently working in `wow_ai::wander`) or pathfinding is unavailable, with creatures pathing through walls and off cliffs visible from the first session. Estimated XL across §9 tasks #MOVE-PATH.1 → #MOVE-PATH.26, with #MOVE-PATH.3 (Detour FFI) and #MOVE-PATH.17 (`FindSmoothPath` port) being the biggest individual chunks.
