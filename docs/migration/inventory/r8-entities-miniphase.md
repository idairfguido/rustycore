# R8 Entities Mini-Phase

> Generated: 2026-05-07
> Rule: every Entities claim is contrasted against `/home/server/woltk-trinity-legacy/src/server/game/Entities/`.

## Closed Tasks

- [x] **#NEXT.R8.ENTITIES.001** Create `wow-entities` crate and base `EntityObject`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h`, `Object.cpp`, `ObjectGuid.h`, `Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/lib.rs`, `crates/wow-entities/src/object.rs`, `Cargo.toml`.
  Acceptance: default constructor state matches C++ `Object::Object`; `_Create`, `AddToWorld`, `RemoveFromWorld`, `SetEntry`, `SetObjectScale`, dynamic flag helpers, type id/mask checks, new/destroyed/update state and clear-update-mask behavior are represented and tested; `map_id`/`instance_id`/`in_grid` are explicit Rust bridge state for canonical map ownership, not claimed as C++ `Object` fields.

- [x] **#NEXT.R8.ENTITIES.002** Port `WorldObject` base state and distance helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.h`, `Object.cpp`, `Position.h`.
  Rust targets: `crates/wow-entities/src/world_object.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `WorldLocation` default map id and orientation normalization match C++; `WorldObject` tracks map/instance binding, current cell, world-object flag, active/far-visible flags, zone/area, DB phase and minimal phase shift; distance/range helpers subtract combat reach, clamp visible distance to zero and use C++ strict `< dist²` checks.

## Follow-Up Work Items

- [ ] **#NEXT.R8.ENTITIES.003** Bind `wow-map` grid unload actions to real entity methods once Creature/GameObject/Corpse exist.
- [ ] **#NEXT.R8.ENTITIES.004** Port `ObjectAccessor` equivalent for global player lookup and map-local object lookup.
- [ ] **#NEXT.R8.ENTITIES.005** Port `WorldObject` LOS, terrain-height, transport, visibility-range and facing/arc helpers that require Map/Terrain/Transport integration.
