# Migration: Movement / Generators (MotionMaster + 13 generators)

> **C++ canonical path:** `src/server/game/Movement/MotionMaster.{h,cpp}` + `src/server/game/Movement/MovementGenerator.{h,cpp}` + `src/server/game/Movement/MovementGenerators/*.{h,cpp}` + `src/server/game/Movement/AbstractFollower.{h,cpp}` + `src/server/game/Movement/MovementDefines.{h,cpp}`
> **Rust target crate(s):** future `crates/wow-movement/src/motion_master.rs` + `crates/wow-movement/src/generators/`
> **Layer:** L5 sub-module (depends on Spline L5, PathGen L5, Entities L4, AI L6)
> **Status:** ❌ not started — 0 of 13 generators ported, no `MotionMaster`, no trait
> **Audited vs C++:** ✅ complete 2026-05-01
> **Last updated:** 2026-05-01

> Sub-doc of [`movement.md`](movement.md). Cross-links: [`movement-spline.md`](movement-spline.md) (consumed by every generator that drives a `MoveSplineInit`), [`movement-pathgen.md`](movement-pathgen.md) (called by chase/follow/point/waypoint to compute walkable paths), [`common-collision.md`](common-collision.md) (height/LOS clamps used by random/wander), [`ai-base.md`](ai-base.md) (CreatureAI is the primary caller of `MotionMaster::Move*`).

---

## 1. Purpose

Drive **what a Unit decides to do next**: idle, wander, patrol a waypoint path, chase a hostile target, follow a friendly target, flee in fear, return home after evade, fly a taxi, hop along a script-defined spline chain, anchor to a formation leader, or run a one-shot custom spline. `MotionMaster` is the per-Unit stack of `MovementGenerator` instances split into priority slots; the top-of-stack generator owns the Unit's motion until it expires or is interrupted. Every generator ultimately produces a `MoveSpline` (see [`movement-spline.md`](movement-spline.md)) and emits the matching client packet via `MoveSplineInit::Launch`.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Movement/MotionMaster.h` | 246 | Per-Unit slot stack: `_defaultGenerator` + `_generators` multiset, delayed-action queue, public `Move*()` API |
| `src/server/game/Movement/MotionMaster.cpp` | 1376 | Implementation of all `Move*` factories + `Update`/`Add`/`Remove`/`Clear` + `ResolveDelayedActions` |
| `src/server/game/Movement/MovementGenerator.h` | 154 | Abstract base + `MovementGeneratorMedium<T,D>` CRTP + `FactoryHolder` registries (Idle/Random/Waypoint) |
| `src/server/game/Movement/MovementGenerator.cpp` | 61 | Destructor + `GetDebugInfo` |
| `src/server/game/Movement/MovementDefines.h` | 142 | Enums: `MovementGeneratorType` (0..18), `MovementSlot` (DEFAULT/ACTIVE), `MovementGeneratorMode/Priority/Flags`, `RotateDirection`, `ChaseRange`, `ChaseAngle`, `JumpArrivalCastArgs`, `JumpChargeParams` |
| `src/server/game/Movement/MovementDefines.cpp` | 48 | `ChaseAngle` constructor + bound helpers |
| `src/server/game/Movement/AbstractFollower.h` | 36 | Common helper: store target ObjectGuid, detect target moves between updates |
| `src/server/game/Movement/AbstractFollower.cpp` | 31 | Implementation |
| `src/server/game/Movement/MovementGenerators/IdleMovementGenerator.h` | 81 | `IdleMovementGenerator` + `RotateMovementGenerator` + `DistractMovementGenerator` + `AssistanceDistractMovementGenerator` |
| `src/server/game/Movement/MovementGenerators/IdleMovementGenerator.cpp` | 231 | Impls — Idle stops + clears unit state, Rotate ticks orientation, Distract pauses for N ms |
| `src/server/game/Movement/MovementGenerators/RandomMovementGenerator.h` | 58 | `RandomMovementGenerator<Creature>` — wander within radius |
| `src/server/game/Movement/MovementGenerators/RandomMovementGenerator.cpp` | 263 | `_setRandomLocation`, water/oxygen check, delay between hops |
| `src/server/game/Movement/MovementGenerators/WaypointMovementGenerator.h` | 93 | `WaypointMovementGenerator<Creature>` — traverse `waypoint_path`/`script_waypoint` rows |
| `src/server/game/Movement/MovementGenerators/WaypointMovementGenerator.cpp` | 469 | `OnArrived`, `StartMove`, `Pause`/`Resume`, formation hooks, AI script callbacks |
| `src/server/game/Movement/MovementGenerators/ChaseMovementGenerator.h` | 59 | Combat follow with `ChaseRange` + `ChaseAngle` |
| `src/server/game/Movement/MovementGenerators/ChaseMovementGenerator.cpp` | 260 | Reposition logic, predicted target offset, melee leeway |
| `src/server/game/Movement/MovementGenerators/FollowMovementGenerator.h` | 62 | Tail target with offset+angle (non-combat) |
| `src/server/game/Movement/MovementGenerators/FollowMovementGenerator.cpp` | 215 | Pet/companion follow |
| `src/server/game/Movement/MovementGenerators/FleeingMovementGenerator.h` | 67 | Run away from a feared source |
| `src/server/game/Movement/MovementGenerators/FleeingMovementGenerator.cpp` | 282 | Random reachable point in opposite direction, repeat |
| `src/server/game/Movement/MovementGenerators/ConfusedMovementGenerator.h` | 49 | Random short hops (CC effect) |
| `src/server/game/Movement/MovementGenerators/ConfusedMovementGenerator.cpp` | 177 | Bounded short-radius wander; pathfinder validated |
| `src/server/game/Movement/MovementGenerators/HomeMovementGenerator.h` | 41 | Return-to-home after evade |
| `src/server/game/Movement/MovementGenerators/HomeMovementGenerator.cpp` | 158 | Move toward `Creature::GetHomePosition` then despawn-or-restore |
| `src/server/game/Movement/MovementGenerators/PointMovementGenerator.h` | 75 | Move to a fixed `(x,y,z)`, fire `MovementInform(POINT_MOTION_TYPE, id)` on arrival |
| `src/server/game/Movement/MovementGenerators/PointMovementGenerator.cpp` | 227 | Pathing + final orientation handling |
| `src/server/game/Movement/MovementGenerators/FlightPathMovementGenerator.h` | 75 | Taxi flight; integrates `TaxiPathNodes` |
| `src/server/game/Movement/MovementGenerators/FlightPathMovementGenerator.cpp` | 338 | Build node-spline; per-node FlightSplineSync, money deduct, transition map |
| `src/server/game/Movement/MovementGenerators/FormationMovementGenerator.h` | 57 | Anchor offset to formation leader |
| `src/server/game/Movement/MovementGenerators/FormationMovementGenerator.cpp` | 222 | Track leader displacement, recompute slot pos |
| `src/server/game/Movement/MovementGenerators/SplineChainMovementGenerator.h` | 61 | Chained `MoveSplineInit` launches with pauses |
| `src/server/game/Movement/MovementGenerators/SplineChainMovementGenerator.cpp` | 238 | Resume from `SplineChainResumeInfo`, per-link `TimeToNext` |
| `src/server/game/Movement/MovementGenerators/GenericMovementGenerator.h` | 55 | One-shot custom `MoveSplineInit` wrapper |
| `src/server/game/Movement/MovementGenerators/GenericMovementGenerator.cpp` | 99 | Captured initializer lambda + duration |
| `src/server/game/Movement/MovementGenerators/PathMovementBase.h` | 43 | Shared path index/iter helpers |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `MotionMaster` | class | Stack-of-generators per slot for one Unit; entry point for AI/scripts |
| `MotionMaster::DelayedAction` | nested class | Captured `(action, validator, type)` deferred while `MOTIONMASTER_FLAG_UPDATE` set |
| `MotionMasterFlags` | enum | NONE / UPDATE / STATIC_INITIALIZATION_PENDING / INITIALIZATION_PENDING / INITIALIZING |
| `MotionMasterDelayedActionType` | enum | CLEAR / CLEAR_SLOT / CLEAR_MODE / CLEAR_PRIORITY / ADD / REMOVE / REMOVE_TYPE / INITIALIZE |
| `MovementGenerator` | abstract class | Interface: `Initialize/Reset/Update/Deactivate/Finalize/GetMovementGeneratorType/UnitSpeedChanged/Pause/Resume/GetResetPosition` |
| `MovementGeneratorMedium<T,D>` | CRTP template | Typed Update/Reset dispatch (avoids cast in concrete generators) |
| `MovementGeneratorFactory<T>` | template | Registry factory for type-tagged construction |
| `MovementGeneratorCreator` / `IdleMovementFactory` / `RandomMovementFactory` / `WaypointMovementFactory` | factory | Slot registration into `sMovementGeneratorRegistry` |
| `MovementGeneratorType` | enum (uint8) | IDLE=0, RANDOM=1, WAYPOINT=2, MAX_DB=3, CONFUSED=4, CHASE=5, HOME=6, FLIGHT=7, POINT=8, FLEEING=9, DISTRACT=10, ASSISTANCE=11, ASSISTANCE_DISTRACT=12, TIMED_FLEEING=13, FOLLOW=14, ROTATE=15, EFFECT=16, SPLINE_CHAIN=17, FORMATION=18 |
| `MovementSlot` | enum | DEFAULT (idle/random/waypoint) / ACTIVE (chase/flee/point/etc.) |
| `MovementGeneratorMode` | enum | DEFAULT / OVERRIDE |
| `MovementGeneratorPriority` | enum | NONE / NORMAL / HIGHEST |
| `MovementGeneratorFlags` | bitfield (uint16) | INITIALIZATION_PENDING / INITIALIZED / SPEED_UPDATE_PENDING / INTERRUPTED / PAUSED / TIMED_PAUSED / DEACTIVATED / INFORM_ENABLED / FINALIZED / PERSIST_ON_DEATH |
| `MovementGeneratorComparator` | functor | Stable ordering of `_generators` multiset (priority desc, then insertion) |
| `MovementGeneratorDeleter` | functor | unique_ptr deleter — runs destructor with knowledge of ABI boundary |
| `MovementGeneratorInformation` | POD | Debug snapshot `{Type, TargetGUID, TargetName}` |
| `RotateDirection` | enum | LEFT / RIGHT |
| `ChaseRange` | struct | `{MinRange, MinTolerance, MaxRange, MaxTolerance}` |
| `ChaseAngle` | struct | `{RelativeAngle, Tolerance}` + `IsAngleOkay`, `UpperBound`, `LowerBound` |
| `JumpArrivalCastArgs` | struct | `{SpellId, Target}` — cast-on-jump-end |
| `JumpChargeParams` | struct | Speed/MoveTimeInSec union + `JumpGravity` + `Spell/Progress/ParabolicCurveId` |
| `IdleMovementGenerator` | class | Ground-zero generator; never moves; clears unit move state |
| `RotateMovementGenerator` | class | Rotates orientation in place over `_duration` ms |
| `DistractMovementGenerator` | class | Hold facing for `_timer` ms (e.g. quest interaction) |
| `AssistanceDistractMovementGenerator` | class | Like Distract but reset to `Idle` on Finalize |
| `RandomMovementGenerator<Creature>` | template class | Wander within `_wanderDistance` radius; per-hop delay |
| `WaypointMovementGenerator<Creature>` | template class | Traverse `WaypointPath::Nodes`; per-node delay/script/event |
| `FollowMovementGenerator` | class | Owner tails target at `(distance, angle)` non-combat |
| `ChaseMovementGenerator` | class | Combat follow; reposition only when outside `ChaseRange.MaxRange` |
| `FleeingMovementGenerator` | class | Pick reachable point opposite from feared source; repeat |
| `TimedFleeingMovementGenerator` | class | FleeingMG with finite duration |
| `ConfusedMovementGenerator` | class | Short-radius wander (CC effect) |
| `HomeMovementGenerator<Creature>` | template class | Move to `Creature::GetHomePosition`; restore default state on arrival |
| `PointMovementGenerator` | class | Single `(x,y,z)` MoveSpline; fire `MovementInform(POINT_MOTION_TYPE, id)` |
| `FlightPathMovementGenerator` | class | Taxi flight along `TaxiPathNode` rows |
| `FormationMovementGenerator` | class | Anchor at `(range, angle)` from a formation leader |
| `SplineChainMovementGenerator` | class | Chain of `MoveSplineInit` launches with `TimeToNext` pauses |
| `GenericMovementGenerator` | class | Run a captured `std::function<void(MoveSplineInit&)>` once |
| `PathMovementBase<T,P>` | template | Shared path-index iterator helpers (waypoint, splinechain) |
| `AbstractFollower` | helper class | Stores target ObjectGuid + dirty bit on target relocate |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `MotionMaster::Initialize()` / `InitializeDefault()` | Bootstrap default IdleMovementGenerator on slot DEFAULT | `MovementGenerator::Initialize` |
| `MotionMaster::AddToWorld()` | Resolve `STATIC_INITIALIZATION_PENDING` after Unit added to map | `DirectInitialize` |
| `MotionMaster::Update(uint32 diff)` | Tick top generator; pop+finalize on completion; resolve delayed actions | `MovementGenerator::Update`, `Finalize`, `ResolveDelayedActions` |
| `MotionMaster::Add(MovementGenerator*, slot)` | Push generator on slot; defer if `MOTIONMASTER_FLAG_UPDATE` set | `DirectAdd` |
| `MotionMaster::Remove(generator|type, slot)` / `Clear(...)` | Pop+Finalize matching | `Delete`, `MovementGenerator::Finalize` |
| `MotionMaster::PropagateSpeedChange()` | Notify all live generators of speed change | `MovementGenerator::UnitSpeedChanged` |
| `MotionMaster::GetDestination(x,y,z)` | Read final destination of top spline | `MoveSpline::FinalDestination` |
| `MotionMaster::StopOnDeath()` | Pop non-`PERSIST_ON_DEATH` generators | `Clear` |
| `MotionMaster::MoveIdle()` / `MoveTargetedHome()` | Convenience wrappers | `Add(new IdleMovementGenerator)` etc. |
| `MotionMaster::MoveRandom(radius, duration)` | Push `RandomMovementGenerator` on DEFAULT | `Add` |
| `MotionMaster::MoveFollow(target, dist, ChaseAngle, duration, slot)` | Push `FollowMovementGenerator` | `Add` |
| `MotionMaster::MoveChase(target, ChaseRange, ChaseAngle)` | Push `ChaseMovementGenerator` | `Add` |
| `MotionMaster::MovePoint(id, pos, generatePath, finalOrient, speed, mode, closeEnoughDistance)` | Push `PointMovementGenerator` | `MoveSplineInit::MoveTo`, `Launch` |
| `MotionMaster::MoveCloserAndStop(id, target, distance)` | 2D approach until within distance, then stop | `MovePoint` variant |
| `MotionMaster::MoveLand` / `MoveTakeoff` | Push generator with landing/takeoff anim | `MoveSplineInit::SetAnimation` |
| `MotionMaster::MoveCharge(x,y,z,speed,id,...)` / `MoveCharge(PathGenerator&,...)` | Fast linear charge with pre-computed path | `MoveSplineInit::Launch` |
| `MotionMaster::MoveKnockbackFrom(origin, speedXY, speedZ, ...)` | Spell knockback parabolic | `MoveSplineInit::SetParabolic` |
| `MotionMaster::MoveJump` / `MoveJumpTo` / `MoveJumpWithGravity` | Parabolic jump variants (with optional `JumpArrivalCastArgs`) | `MoveSplineInit::SetParabolicVerticalAcceleration` |
| `MotionMaster::MoveCirclePath(x,y,z,radius,clockwise,stepCount)` | Circular orbit | `MoveSplineInit::MovebyPath` |
| `MotionMaster::MoveSmoothPath(pointId, points, size, walk, fly)` | Custom smooth path | `MoveSplineInit::MovebyPath` |
| `MotionMaster::MoveAlongSplineChain(pointId, chainId|chain, walk)` | Push `SplineChainMovementGenerator` | `Add` |
| `MotionMaster::ResumeSplineChain(SplineChainResumeInfo)` | Continue interrupted chain | `Add` |
| `MotionMaster::MoveFall(id)` | Drop straight down using gravity | `MoveSplineInit::SetFall` |
| `MotionMaster::MoveSeekAssistance(x,y,z)` / `MoveSeekAssistanceDistract(timer)` | Combat-assistance flow | `Add` |
| `MotionMaster::MoveTaxiFlight(path, pathnode)` | Push `FlightPathMovementGenerator` | `Add` |
| `MotionMaster::MoveDistract(time, orientation)` | Push `DistractMovementGenerator` | `Add` |
| `MotionMaster::MovePath(pathId|path, repeatable, duration, ...)` | Push `WaypointMovementGenerator` | `Add` |
| `MotionMaster::MoveRotate(id, time, dir)` | Push `RotateMovementGenerator` | `Add` |
| `MotionMaster::MoveFormation(leader, range, angle, p1, p2)` | Push `FormationMovementGenerator` | `Add` |
| `MotionMaster::MoveFleeing(enemy, time)` | Push `FleeingMovementGenerator` (or `TimedFleeingMovementGenerator` if `time>0`) | `Add` |
| `MotionMaster::MoveConfused()` | Push `ConfusedMovementGenerator` | `Add` |
| `MotionMaster::LaunchMoveSpline(initializer, id, priority, type)` | Run captured lambda once, push `GenericMovementGenerator` | `Add` |
| `MotionMaster::CalculateJumpSpeeds(dist, moveType, mult, minH, maxH, &outXY, &outZ)` | Solve parabolic for given distance + height bounds | math |
| `MotionMaster::ResolveDelayedActions()` | Drain `_delayedActions` deque (FIFO) | `DelayedAction::Resolve` |
| `MovementGenerator::Initialize(Unit*)` | Begin generator (called once when pushed) | concrete `DoInitialize` |
| `MovementGenerator::Reset(Unit*)` | Re-enter when previous top finalized | concrete `DoReset` |
| `MovementGenerator::Update(Unit*, diff) -> bool` | Tick; return false to finalize | concrete `DoUpdate` |
| `MovementGenerator::Deactivate(Unit*)` | Called when displaced by higher-priority generator | concrete `DoDeactivate` |
| `MovementGenerator::Finalize(Unit*, active, movementInform)` | Final cleanup; optionally fire `MovementInform` AI hook | concrete `DoFinalize` |
| `MovementGenerator::UnitSpeedChanged()` | Propagate Unit speed to live spline | varies |
| `MovementGenerator::Pause(timer)` / `Resume(timer)` | Pause generator (waypoint, follow) | sets `MOVEMENTGENERATOR_FLAG_PAUSED` |
| `MovementGenerator::GetResetPosition(Unit*, &x,&y,&z)` | Used by Evade flow to compute reset point | varies |
| `AbstractFollower::SetTarget(Unit*)` / `GetTarget()` | Track followed unit by GUID | — |

---

## 5. Module dependencies

**Depends on:**
- [`movement-spline.md`](movement-spline.md) — every generator that emits motion calls `MoveSplineInit::Launch` (defined in `Movement/Spline/MoveSplineInit.h`).
- [`movement-pathgen.md`](movement-pathgen.md) — `Chase`, `Follow`, `Point`, `Waypoint`, `Confused`, `Fleeing` all instantiate `PathGenerator` to compute walkable polylines.
- `Entities/Unit` — `Unit*` owns `MotionMaster`; receives `Unit::SetUnitState`, `ClearUnitState`.
- `Entities/Creature` — `Random/Waypoint/Home<Creature>` are typed templates; `Creature::GetHomePosition`, `Creature::SetHomePosition`, `Creature::IsAIEnabled`.
- `AI` ([`ai-base.md`](ai-base.md)) — `CreatureAI::MovementInform(type, id)` is the callback fired on Finalize when `MOVEMENTGENERATOR_FLAG_INFORM_ENABLED`.
- `Combat` — `ChaseMovementGenerator` activates on `Unit::EngageWithTarget`; `FleeingMovementGenerator` from `Spell::EffectInterruptCast` (fear).
- `Maps` — `Unit::Relocate` writes back to grid; visibility recompute in `Unit::Update`.
- `DataStores` — `TaxiPathStore`, `TaxiPathNodeStore` for `FlightPathMovementGenerator`.
- `Waypoints` — `WaypointManager` SQL loader (sub-module of movement; covered separately if split further).
- `SmartAI/Scripts` — emit `MovePoint`, `MoveChase`, `MoveAlongSplineChain` from boss scripts.
- `Detour` (3rdparty via [`movement-pathgen.md`](movement-pathgen.md)) — indirect through `PathGenerator`.
- [`common-collision.md`](common-collision.md) — wander/random pick must clamp to `Map::GetHeight` and pass LOS via `VMapManager`.

**Depended on by:**
- All `CreatureAI` / `SmartAI` / boss scripts (call `MotionMaster::Move*`).
- `Player` (taxi flight, charge spells, fall recovery).
- `Pets/Vehicles` (follow/chase owner).
- `Battlegrounds/Instances` — boss orchestrators emit `MoveAlongSplineChain` on phase transitions.
- `Spell System` — knockback, charge, blink push generators on victim/caster.

---

## 6. SQL / DB queries (if any)

`MotionMaster` and the generators do not emit SQL directly. Two sources feed them:

| Statement / Source | Purpose | DB |
|---|---|---|
| `SELECT * FROM waypoint_path` (via `WaypointManager`) | Patrol path metadata | world |
| `SELECT * FROM waypoint_path_node` (via `WaypointManager`) | Per-node pos+delay+event | world |
| `SELECT * FROM script_waypoint` | Escort/quest scripted paths | world |
| `SELECT * FROM creature_formations` | `FormationMovementGenerator` leader/follower offsets | world |
| `SELECT * FROM script_spline_chain_meta`, `script_spline_chain_waypoints` | `SplineChainMovementGenerator` static chains | world |
| `SELECT * FROM creature_addon` (`path_id`) | Spawn-level waypoint path attachment | world |
| `SELECT * FROM creature_template_addon` (`path_id`) | Template-level default path | world |
| `SELECT * FROM creature_movement_override` | Override default `MovementType` per spawn | world |

DBC/DB2 stores consumed:

| Store | What it loads | Read by |
|---|---|---|
| `TaxiPathStore` | `TaxiPath.dbc` | `FlightPathMovementGenerator` |
| `TaxiPathNodeStore` | `TaxiPathNode.dbc` | `FlightPathMovementGenerator` |
| `MapStore` (transition map column) | `Map.dbc` | `FlightPathMovementGenerator` (cross-continent flights) |

---

## 7. Wire-protocol packets (if any)

Generators do not own packet writers directly; every motion is serialized through `MoveSplineInit::Launch` (see [`movement-spline.md`](movement-spline.md)). Only `FlightPathMovementGenerator` emits a per-node sync packet:

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `SMSG_FLIGHT_SPLINE_SYNC` (0x2E2B) | S→C | `FlightPathMovementGenerator::DoEventIfAny` after each node |
| `SMSG_ON_MONSTER_MOVE` | S→C | indirectly via every generator's `MoveSplineInit::Launch` |
| `SMSG_MOVE_TELEPORT` | S→C | `HomeMovementGenerator` short-circuit if forced teleport home |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- *None.* No `crates/wow-movement/` exists. There is no `MotionMaster`, no `MovementGenerator` trait, no slot stack, no per-tick generator update.

**What's implemented:**
- A single `wow_ai::wander::pick_wander_destination` linear-tween in `crates/wow-ai/src/` that nudges `WorldCreature::current_pos` between two random points in a per-creature loop. This is **not** a generator — it is a hard-coded substitute that knows nothing about slots, priorities, or finalization callbacks.
- `crates/wow-world/src/handlers/movement.rs` (204 lines) parses ~28 client `CMSG_MOVE_*` opcodes into `MovementInfo` and broadcasts `SMSG_MOVE_UPDATE`, but never drives a creature.

**What's missing vs C++:**
- **0 of 13 concrete generators** — Idle, Random, Waypoint, Confused, Chase, Home, Flight, Point, Fleeing, Distract (+ Assistance, AssistanceDistract), Follow, Rotate, Effect, SplineChain, Formation, Generic — none exist.
- **`MotionMaster` does not exist.** No slot stack, no `Update(diff)`, no `Add/Remove/Clear`, no `_delayedActions` queue, no flag bits.
- **`MovementGenerator` trait does not exist.** No `Initialize/Reset/Update/Deactivate/Finalize`. No `MOVEMENTGENERATOR_FLAG_*` bitfield.
- **`AbstractFollower` helper does not exist.** No follower-target dirty tracking.
- **`ChaseRange` / `ChaseAngle`** structs do not exist.
- **`MovementGeneratorType` enum** is not defined in `wow-constants` (only opcodes are).
- **`MovementInform` AI callback** is not wired — `wow-ai` has no equivalent of `CreatureAI::MovementInform(type, id)`.
- **`PropagateSpeedChange`** — speed changes never propagate to in-flight motion (because no in-flight motion exists).
- **Waypoint loading** — no `WaypointManager`, no SQL reader for `waypoint_path*`, no `script_waypoint`, no `creature_formations`, no `script_spline_chain_meta`.
- **Taxi flight** — no `FlightPathMovementGenerator`, no `TaxiPath.dbc` consumer.
- **Charge / jump / knockback / fall** — no parabolic emitters; `CMSG_MOVE_FALL_LAND` is parsed but no fall damage compute.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- The `wow-ai::wander` linear-tween bypasses `MoveSplineInit::Launch`; clients only see the result via periodic position snapshots, not as a real spline. Once a real `MotionMaster` lands, this wander loop must be replaced by `RandomMovementGenerator` or it will fight the slot stack.
- There is no place in `wow-world` `WorldSession::tick_*` paths that calls a per-Unit `Update(diff)`. Adding `MotionMaster` requires plumbing a tick into the creature update loop in `WorldSession::tick_combat_sync` (or moving to `MapManager`-driven ticks once that migration completes).
- The legacy two-place creature storage (`WorldSession.creatures` HashMap vs `MapManager`) means a `MotionMaster` will need to live on `WorldCreature` inside `MapManager`, not on the per-session copy — wiring on the wrong side will leak motion across phasing or duplicate updates.

**Tests existing:**
- 0 tests for any generator.
- 0 tests for `MotionMaster` (does not exist).
- 0 tests for `ChaseRange`/`ChaseAngle` (do not exist).

---

## 9. Migration sub-tasks

Numbered for cross-reference from `MIGRATION_ROADMAP.md` §5. Complexity: **L** (<1h), **M** (1-4h), **H** (4-12h), **XL** (>12h).

- [ ] **#MOVE-GEN.1** Create `crates/wow-movement/` with module skeleton: `motion_master.rs`, `generator.rs`, `defines.rs`, `generators/{idle,random,waypoint,chase,follow,...}.rs`. (L)
- [ ] **#MOVE-GEN.2** Port `MovementGeneratorType` enum (0..18, identical values). (L)
- [ ] **#MOVE-GEN.3** Port `MovementSlot` (DEFAULT/ACTIVE), `MovementGeneratorMode`, `MovementGeneratorPriority`. (L)
- [ ] **#MOVE-GEN.4** Port `MovementGeneratorFlags` bitflags (`bitflags!` crate already in workspace). (L)
- [ ] **#MOVE-GEN.5** Define `trait MovementGenerator { fn initialize, reset, update, deactivate, finalize, kind, ... }`. (M)
- [ ] **#MOVE-GEN.6** Port `ChaseRange` + `ChaseAngle` structs with `is_angle_okay`, `upper_bound`, `lower_bound`. (L)
- [ ] **#MOVE-GEN.7** Port `MotionMasterFlags` + `MotionMasterDelayedActionType` + `DelayedAction` struct. (L)
- [ ] **#MOVE-GEN.8** Implement `MotionMaster` core: stack per slot (`Vec<Box<dyn MovementGenerator>>`), `update(diff)` with delayed-action draining. (H)
- [ ] **#MOVE-GEN.9** Wire `MotionMaster::add/remove/clear` + flag-gated deferral. (M)
- [ ] **#MOVE-GEN.10** `IdleMovementGenerator` + `RotateMovementGenerator` + `DistractMovementGenerator` + `AssistanceDistractMovementGenerator`. (M)
- [ ] **#MOVE-GEN.11** `PointMovementGenerator` (single-target move + `MovementInform(POINT, id)` callback). (M)
- [ ] **#MOVE-GEN.12** `RandomMovementGenerator` (wander radius + delay + water/oxygen check). (H)
- [ ] **#MOVE-GEN.13** `AbstractFollower` helper + `FollowMovementGenerator`. (M)
- [ ] **#MOVE-GEN.14** `ChaseMovementGenerator` (combat reposition + `ChaseRange`/`ChaseAngle`). (H)
- [ ] **#MOVE-GEN.15** `FleeingMovementGenerator` + `TimedFleeingMovementGenerator`. (M)
- [ ] **#MOVE-GEN.16** `ConfusedMovementGenerator` (CC short hops). (M)
- [ ] **#MOVE-GEN.17** `HomeMovementGenerator` + Evade integration. (M)
- [ ] **#MOVE-GEN.18** `WaypointManager` SQL loader + `WaypointMovementGenerator`. (XL — split: loader L, generator H)
- [ ] **#MOVE-GEN.19** `FormationMovementGenerator` + `creature_formations` loader. (H)
- [ ] **#MOVE-GEN.20** `FlightPathMovementGenerator` + `TaxiPath.dbc`/`TaxiPathNode.dbc` integration + `SMSG_FLIGHT_SPLINE_SYNC`. (XL)
- [ ] **#MOVE-GEN.21** `SplineChainMovementGenerator` + `script_spline_chain_*` loader + `SplineChainResumeInfo`. (H)
- [ ] **#MOVE-GEN.22** `GenericMovementGenerator` (lambda capture for one-shot custom splines). (M)
- [ ] **#MOVE-GEN.23** `MotionMaster::MoveJump` / `MoveCharge` / `MoveKnockbackFrom` / `MoveFall` (parabolic helpers — depend on movement-spline). (H)
- [ ] **#MOVE-GEN.24** Wire `Unit::MovementInform(type, id)` AI callback (cross-link with [`ai-base.md`](ai-base.md)). (M)
- [ ] **#MOVE-GEN.25** Wire per-Unit `MotionMaster::update(diff)` into `MapManager` creature tick. (H)
- [ ] **#MOVE-GEN.26** Replace `wow_ai::wander` linear-tween with `RandomMovementGenerator` push at spawn time. (M)
- [ ] **#MOVE-GEN.27** `MotionMaster::PropagateSpeedChange` + listener registry. (M)

---

## 10. Regression tests to write

- [ ] Test: `MotionMaster::initialize_default` pushes `IdleMovementGenerator` into slot DEFAULT.
- [ ] Test: After `MovePoint` + arrival, top of stack is back to IDLE on slot DEFAULT.
- [ ] Test: `Add` during `update_in_progress` defers to `_delayedActions` and fires after Update.
- [ ] Test: `Clear(slot=ACTIVE)` while a chase is running fires `Finalize(active=true)` then re-enters `Reset` on whatever was below.
- [ ] Test: `RandomMovementGenerator::update` never picks a destination outside `wander_distance` (radius bound).
- [ ] Test: `ChaseMovementGenerator` with `ChaseRange::MaxRange=5` only repositions if target moves > MaxRange away.
- [ ] Test: `ChaseAngle::is_angle_okay` returns true within `±tolerance`, false outside.
- [ ] Test: `WaypointMovementGenerator` with 3 nodes and `delay=2000` ticks: at t=2000ms emits next-node move, at t=arrival fires `MovementInform(WAYPOINT, node_id)`.
- [ ] Test: `FleeingMovementGenerator` with feared source at (0,0,0) picks point with `dot(direction_to_pick, direction_from_fearer) > 0`.
- [ ] Test: `HomeMovementGenerator` finalize calls `Creature::SetHomePosition` no-op if already at home (within tolerance).
- [ ] Test: `FlightPathMovementGenerator` per-node `SMSG_FLIGHT_SPLINE_SYNC` sent at the configured node-arrival.
- [ ] Test: `SplineChainMovementGenerator::ResumeFrom(SplineChainResumeInfo)` continues from `(SplineIndex, PointIndex, TimeToNext)`.
- [ ] Test: `FormationMovementGenerator` with leader at (10,0,0) and offset `(range=5, angle=PI)` puts follower at (5,0,0).
- [ ] Test: `MotionMaster::PropagateSpeedChange` calls `MovementGenerator::UnitSpeedChanged` on every live generator.
- [ ] Test: `StopOnDeath` removes all generators that don't have `MOVEMENTGENERATOR_FLAG_PERSIST_ON_DEATH`.
- [ ] Test: Generator pushed with `MOTION_PRIORITY_HIGHEST` displaces a `NORMAL` priority active even if same type.
- [ ] Test: `AbstractFollower::dirty_bit` flips when target relocates between updates.
- [ ] Test: `ConfusedMovementGenerator` selected destinations all stay within ~8 yard radius (per-hop CC bound).
- [ ] Test: `RotateMovementGenerator` with `direction=LEFT` produces monotonically increasing orientation modulo 2π.
- [ ] Test: `IdleMovementGenerator::initialize` calls `Unit::ClearUnitState(UNIT_STATE_MOVING)`.

---

## 11. Notes / gotchas

- **Delayed actions are not optional.** Calling `Add/Remove/Clear` from inside `MovementGenerator::Update` causes iterator invalidation in C++ unless `MOTIONMASTER_FLAG_UPDATE` is set and the call is queued. Rust safety alone will not save us — the same logic must defer mutation to a `VecDeque<DelayedAction>` drained at end of tick.
- **Slot is not priority.** Slot is *which class of motion* (default/active); priority is a comparator within the same slot. Removing from the wrong slot freezes a creature with no top-of-stack.
- **`MOTION_SLOT_DEFAULT` always autofills with `IdleMovementGenerator`** when emptied. Forgetting this leaves the unit motionless after every `MoveTargetedHome` arrival.
- **`MovementInform` callback fires only when `MOVEMENTGENERATOR_FLAG_INFORM_ENABLED` AND `Finalize(active=true, movementInform=true)`**. Forgetting to set the flag means boss scripts never get notified of waypoint arrivals and stall.
- **`ChaseMovementGenerator` overshoot** — without `PathGenerator::ShortenPathUntilDist`, mobs "stick" to the player. Always shorten chase paths to `ChaseRange.MaxRange - ChaseRange.MaxTolerance`.
- **`FleeingMovementGenerator`** picks a *random reachable* point opposite the feared source. If pathfinding fails repeatedly (e.g. cornered), it falls back to short-radius confused-style hops; this fallback is necessary, not optional.
- **`HomeMovementGenerator`** is the *only* generator that integrates with the Evade flow. Skipping it makes Evade leave creatures stranded.
- **`FlightPathMovementGenerator`** transitions between maps during cross-continent flights (e.g. Stormwind → Theramore). The `Map::dbc` `MapId` column drives the transition, not an in-engine teleport.
- **`SplineChainMovementGenerator`** must persist `SplineChainResumeInfo` on creature respawn — boss scripts expect to resume mid-chain after a wipe.
- **Formation drift** — leader teleport (e.g. taxi) without explicit `FormationMovementGenerator::leader_teleported()` hook desyncs every follower. Hook it.
- **`UnitSpeedChanged`** must propagate even mid-spline — a Bloodlust during chase changes melee leeway. Do not no-op this for live generators.
- **Generators are NOT thread-safe**. They mutate `Unit*` and read MoveSpline. The per-Unit lock model in Rust must be a write-lock for the whole `MotionMaster::update`, not interior mutability.
- **`PathMovementBase`** is intentionally a CRTP base (waypoint + spline_chain) to share index iteration. In Rust this becomes a small `PathCursor` struct embedded in both, not a trait.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class MotionMaster` | `struct MotionMaster { owner: ObjectGuid, default: Box<dyn MovementGenerator>, slots: [Vec<Box<dyn MovementGenerator>>; 2], delayed: VecDeque<DelayedAction>, flags: MotionMasterFlags, base_unit_states: HashMap<u32, *const dyn MovementGenerator> }` | Stack-per-slot |
| `class MovementGenerator` (abstract) | `trait MovementGenerator: Send` | `fn initialize(&mut self, unit: &mut Unit)` etc. |
| `MovementGeneratorMedium<T,D>` (CRTP) | (drop entirely) | Trait + concrete impls suffice; no CRTP needed |
| `FactoryHolder<MovementGenerator,Unit,MovementGeneratorType>` | `inventory::submit!` registration → fn pointer table indexed by `MovementGeneratorType` | Match the existing handler-dispatch pattern |
| `MovementGeneratorType` (enum uint8) | `#[repr(u8)] enum MovementGeneratorType { Idle=0, Random=1, Waypoint=2, ... }` | Identical numeric values |
| `MovementSlot` | `enum MovementSlot { Default, Active }` | — |
| `MovementGeneratorFlags` | `bitflags! struct MovementGeneratorFlags: u16 { ... }` | Identical bit positions |
| `MotionMasterFlags` | `bitflags! struct MotionMasterFlags: u8 { ... }` | — |
| `DelayedAction` | `struct DelayedAction { kind: DelayedActionType, action: Box<dyn FnOnce(&mut MotionMaster)>, validator: Box<dyn Fn() -> bool> }` | Boxed closures |
| `std::deque<DelayedAction>` | `VecDeque<DelayedAction>` | — |
| `std::multiset<MovementGenerator*, Comparator>` | `Vec<Box<dyn MovementGenerator>>` kept sorted by `(priority desc, insertion_idx)` | Multiset is overkill |
| `unique_ptr<MovementGenerator, Deleter>` | `Box<dyn MovementGenerator>` | — |
| `ChaseRange` / `ChaseAngle` | `struct ChaseRange { min: f32, min_tol: f32, max: f32, max_tol: f32 }` / `struct ChaseAngle { relative: f32, tolerance: f32 }` | POD |
| `JumpArrivalCastArgs` | `struct JumpArrivalCastArgs { spell_id: u32, target: ObjectGuid }` | — |
| `JumpChargeParams` (union) | `enum JumpChargeSpec { Speed(f32), MoveTimeSec(f32) }` | Tagged enum replaces union |
| `IdleMovementGenerator` | `struct IdleMovementGenerator;` impl `MovementGenerator` | Stateless |
| `RandomMovementGenerator<Creature>` | `struct RandomMovementGenerator { wander_distance: f32, duration: Option<Duration>, next_hop_at: Instant, current_path: Option<Vec<Vec3>> }` | Drop the `<Creature>` — runtime check |
| `WaypointMovementGenerator<Creature>` | `struct WaypointMovementGenerator { path: Arc<WaypointPath>, current_node: usize, paused_until: Option<Instant>, repeatable: bool, ... }` | Path shared via `Arc` |
| `ChaseMovementGenerator` | `struct ChaseMovementGenerator { target: ObjectGuid, range: ChaseRange, angle: ChaseAngle, follower: AbstractFollower, recompute_at: Instant }` | — |
| `FollowMovementGenerator` | `struct FollowMovementGenerator { target: ObjectGuid, dist: f32, angle: ChaseAngle, follower: AbstractFollower, duration: Option<Duration> }` | — |
| `FleeingMovementGenerator` | `struct FleeingMovementGenerator { fearer: ObjectGuid, until: Option<Instant>, current_path: Option<Vec<Vec3>> }` | — |
| `ConfusedMovementGenerator` | `struct ConfusedMovementGenerator { until: Option<Instant>, next_hop_at: Instant }` | — |
| `HomeMovementGenerator<Creature>` | `struct HomeMovementGenerator { home: Position, arrived: bool }` | — |
| `PointMovementGenerator` | `struct PointMovementGenerator { id: u32, dest: Position, generate_path: bool, final_orient: Option<f32>, speed: Option<f32>, mode: SpeedSelectionMode, close_enough: Option<f32> }` | — |
| `FlightPathMovementGenerator` | `struct FlightPathMovementGenerator { path_nodes: Arc<[TaxiPathNode]>, current_node: usize, money_per_node: u32, transition_map: Option<u32> }` | — |
| `FormationMovementGenerator` | `struct FormationMovementGenerator { leader: ObjectGuid, range: f32, angle: f32, point1: u32, point2: u32 }` | — |
| `SplineChainMovementGenerator` | `struct SplineChainMovementGenerator { chain: Arc<[SplineChainLink]>, current_link: usize, current_point: usize, walk: bool, time_to_next: Duration, point_id: u32 }` | Resume info inline |
| `GenericMovementGenerator` | `struct GenericMovementGenerator { initializer: Box<dyn FnOnce(&mut MoveSplineInit) + Send>, id: u32, kind: MovementGeneratorType }` | Captured lambda |
| `AbstractFollower` | `struct AbstractFollower { target: ObjectGuid, dirty: bool }` | — |
| `RotateDirection` | `enum RotateDirection { Left, Right }` | — |
| `Unit::MovementInform(type, id)` callback | `wow_ai::CreatureAi::movement_inform(&mut self, ty: MovementGeneratorType, id: u32)` | Hook inside [`ai-base.md`](ai-base.md) |

---

*Template version: 1.0 (2026-05-01).* Last updated: 2026-05-01.

---

## 13. Audit (2026-05-01)

**Scope.** Cross-checked `/home/server/woltk-trinity-legacy/src/server/game/Movement/MotionMaster.{h,cpp}` (246 + 1376 lines), `MovementGenerator.{h,cpp}` (154 + 61), `MovementDefines.{h,cpp}` (142 + 48), `AbstractFollower.{h,cpp}` (36 + 31), and all 13 generator pairs under `MovementGenerators/` (3134 lines total of `.cpp`) against the Rust workspace at `/home/server/rustycore/crates/`.

**MotionMaster: absent.** There is **no `MotionMaster` struct anywhere in the workspace**. No slot stack, no per-Unit `update(diff)`, no delayed-action queue, no priority comparator, no `PropagateSpeedChange`, no `StopOnDeath`. The C++ class exposes ~38 public `Move*` factories — Rust ships **0**.

**Generators: 0 of 13 ported.**
- IdleMovementGenerator + Rotate + Distract + AssistanceDistract: **absent** (231 lines C++).
- RandomMovementGenerator: **absent** (263 lines C++) — substituted by a non-equivalent `wow_ai::wander` linear-tween that does not implement the `MovementGenerator` lifecycle.
- WaypointMovementGenerator: **absent** (469 lines C++); no `WaypointManager`, no SQL loader for `waypoint_path`/`waypoint_path_node`.
- ConfusedMovementGenerator: **absent** (177 lines C++).
- ChaseMovementGenerator: **absent** (260 lines C++); `ChaseRange`/`ChaseAngle` structs do not exist.
- HomeMovementGenerator: **absent** (158 lines C++); evade flow has no return-to-home hook.
- FlightPathMovementGenerator: **absent** (338 lines C++); no `TaxiPath.dbc` consumer; `SMSG_FLIGHT_SPLINE_SYNC` opcode is listed in `wow-constants/opcodes.rs` but no writer or caller.
- PointMovementGenerator: **absent** (227 lines C++); no `MovementInform(POINT_MOTION_TYPE, id)` callback path.
- FleeingMovementGenerator + TimedFleeingMovementGenerator: **absent** (282 lines C++).
- FollowMovementGenerator: **absent** (215 lines C++); `AbstractFollower` helper missing.
- FormationMovementGenerator: **absent** (222 lines C++); `creature_formations` table not loaded.
- SplineChainMovementGenerator: **absent** (238 lines C++); `script_spline_chain_*` tables not loaded.
- GenericMovementGenerator: **absent** (99 lines C++).

**Trait + enum surface.** `trait MovementGenerator` does not exist. `MovementGeneratorType` enum does not exist. `MovementSlot`, `MovementGeneratorMode`, `MovementGeneratorPriority`, `MovementGeneratorFlags`, `MotionMasterFlags`, `MotionMasterDelayedActionType` — all absent. `ChaseRange`, `ChaseAngle`, `JumpArrivalCastArgs`, `JumpChargeParams` — all absent.

**AI callback surface.** `Unit::MovementInform(type, id)` (the AI hook every generator calls on Finalize when `MOVEMENTGENERATOR_FLAG_INFORM_ENABLED`) has no Rust counterpart in `wow-ai`. Boss scripts that depend on waypoint/point arrival callbacks have nothing to attach to.

**Worst divergence.** This sub-doc is the single largest greenfield in the engine layer alongside [`movement-spline.md`](movement-spline.md) and [`movement-pathgen.md`](movement-pathgen.md): 13 concrete generator classes plus the `MotionMaster` orchestrator must be ported before any AI script in the codebase can request `MoveChase`, `MovePoint`, `MoveAlongSplineChain`, `MoveTaxiFlight`, `MoveCharge`, `MoveJump`, `MoveFleeing`, `MoveConfused`, `MoveHome`, `MovePath`, or `MoveFormation`. The current `wow_ai::wander` linear-tween is a non-substitute — it lacks lifecycle, slots, priority, deferral, AI callbacks, and pathfinding. Estimated XL across §9 tasks #MOVE-GEN.1 → #MOVE-GEN.27.
