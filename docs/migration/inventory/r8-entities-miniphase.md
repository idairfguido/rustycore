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

- [x] **#NEXT.R8.ENTITIES.004** Port `ObjectAccessor` base lookup API.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectAccessor.h`, `ObjectAccessor.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp::normalizePlayerName`.
  Rust targets: `crates/wow-entities/src/object_accessor.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: global player holder supports add/remove/find-connected/find-in-world by GUID and normalized name; same-map `GetPlayer` semantics are represented; `GetWorldObject`, `GetObjectByTypeMask`, unit/creature/pet/gameobject/corpse/dynamicobject/areatrigger/sceneobject/conversation dispatch follows C++ high-GUID/type-mask branches, including C++'s `GetObjectByTypeMask` corpse branch returning null; map-local objects are stored through a bridge store until canonical `wow_map::Map` owns real entity containers.

- [x] **#NEXT.R8.ENTITIES.007** Port update-field delta foundation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateMask.h`, `UpdateField.h`, `UpdateFields.h`, `UpdateFields.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp`.
  Rust targets: `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/object.rs`, `crates/wow-packet/src/packets/update.rs`.
  Acceptance: `UpdateMask` block/block-mask behavior is represented; `EntityObject` emits `UF::ObjectData` values update masks using C++ bit positions 0..3; `UpdateObject` can serialize an ObjectData-only VALUES update in the C++ `Object::PrepareValuesUpdateBuffer`/`ObjectData::WriteUpdate` shape; creature health VALUES updates no longer write the create-only `UpdateFieldFlag` byte.

- [x] **#NEXT.R8.ENTITIES.009** Port `Unit` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.h`, `Unit.cpp`, `UnitDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/unit.rs`, `crates/wow-entities/src/lib.rs`, `crates/wow-entities/src/update_fields.rs`.
  Acceptance: base `Unit` constructor state matches C++ for type id/mask, movement update flag, attack/speed/weapon defaults, death state and unit state; health/max-health clamps follow C++; power setters use a derived power-index bridge and clamp current power to max; display, native display, level, faction, bounding radius and combat reach update `UF::UnitData` bit positions; `UnitValuesUpdate` sets the `TYPEID_UNIT` object-type bit.

- [x] **#NEXT.R8.ENTITIES.011** Port `Player` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/StatSystem.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/unit.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Player` constructor retags `Unit` to `TYPEID_PLAYER`/`TYPEMASK_PLAYER`, stores session bridge, uses C++ hit chance/team/active/control/whisper defaults, exposes race/class/gender/native gender and target selection setters, ports PlayerData flags/loot/spec/bank setters, ports ActivePlayerData money/XP/backpack/inv-slot field masks, and splits `PlayerData` vs `ActivePlayerData` values updates for self vs other receivers.

- [x] **#NEXT.R8.ENTITIES.015** Port `Creature` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Creature/Creature.h`, `Creature.cpp`, `CreatureData.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/StatSystem.cpp`, `UnitDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Movement/MovementDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Miscellaneous/SharedDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/World/World.cpp`.
  Rust targets: `crates/wow-entities/src/creature.rs`, `crates/wow-entities/src/unit.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Creature` constructor remains `TYPEID_UNIT`/`TYPEMASK_UNIT`, preserves C++ defaults for respawn/corpse timers, regen, react state, idle movement, assistance flags, spell slots, loot mode, sight/combat distance and temp-world-object state; `Creature::GetPowerIndex` semantics are represented; faction/display setters update `UnitData`, with model dimensions passed explicitly until ObjectMgr template/model stores are canonical.

- [x] **#NEXT.R8.ENTITIES.019** Port `GameObject` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/GameObject/GameObject.h`, `GameObject.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Miscellaneous/SharedDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/game_object.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `GameObject` constructor matches C++ type id/mask and stationary/rotation create flags; respawn/despawn/restock/cooldown, loot state/unit guid, spawned-by-default, spell/spawn ids, packed rotation, loot mode, respawn compatibility and stationary position defaults are represented; `UF::GameObjectData` bit masks cover display, flags, faction, level, state, type, percent health, art kit and custom param; values update sets `TYPEID_GAMEOBJECT`.

- [x] **#NEXT.R8.ENTITIES.022** Port `Corpse` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Corpse/Corpse.h`, `Corpse.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Miscellaneous/SharedDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/corpse.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Corpse` constructor matches C++ type id/mask, `WorldObject(type != CORPSE_BONES)`, stationary create flag and ghost time/type state; `CorpseData` dynamic flags, owner/party/guild, display/race/class/sex/flags/faction/items setters use C++ bit positions; expiry thresholds for bones and resurrectable corpses match C++; values update sets `TYPEID_CORPSE`.

- [x] **#NEXT.R8.ENTITIES.024** Port `DynamicObject` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/DynamicObject/DynamicObject.h`, `DynamicObject.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/dynamic_object.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `DynamicObject` constructor matches C++ type id/mask, `WorldObject(isWorldObject)`, stationary create flag and duration/aura/caster/viewpoint null state; `DynamicObjectType` enum and `DynamicObjectData` caster/type/spell visual/spell id/radius/cast-time setters use C++ bit positions; non-aura duration ticking follows C++; values update sets `TYPEID_DYNAMICOBJECT`.

- [x] **#NEXT.R8.ENTITIES.026** Port `AreaTrigger` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/AreaTrigger/AreaTrigger.h`, `AreaTrigger.cpp`, `AreaTriggerTemplate.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/area_trigger.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `AreaTrigger` constructor matches C++ type id/mask, `WorldObject(false)`, stationary and area-trigger create flags, spawn/target/aura/stationary-position/duration/time/removal/movement/template bridge defaults; permanent duration is sent as zero; runtime duration updates do not mark the update mask; scalar `AreaTriggerData`, simple scale-curve constants and `VisualAnim` use C++ bit positions; values update sets `TYPEID_AREATRIGGER`.

## Follow-Up Work Items

- [ ] **#NEXT.R8.ENTITIES.003** Bind `wow-map` grid unload actions to real entity methods once Creature/GameObject/Corpse exist.
- [ ] **#NEXT.R8.ENTITIES.005** Port `WorldObject` LOS, terrain-height, transport, visibility-range and facing/arc helpers that require Map/Terrain/Transport integration.
- [ ] **#NEXT.R8.ENTITIES.006** Complete `ObjectAccessor` branches that require real `Player`: item lookup via inventory, real `SaveAllPlayers`, and wiring to canonical `wow_map::Map` containers instead of bridge storage.
- [ ] **#NEXT.R8.ENTITIES.008** Complete generated update-field sections beyond `ObjectData`: `UnitData`, `PlayerData`, `ActivePlayerData`, `GameObjectData`, `ItemData`, `CorpseData`, `DynamicObjectData`, `AreaTriggerData`, `SceneObjectData`, `ConversationData`, including visibility flag filters and dynamic/optional fields.
- [ ] **#NEXT.R8.ENTITIES.010** Complete `Unit` subsystems beyond base fields: aura hooks, threat/combat manager, SpellHistory, MotionMaster/move spline, charm/minion ownership, vehicle hooks, AI references and runtime power-index implementations for Player/Creature/Pet.
- [ ] **#NEXT.R8.ENTITIES.012** Complete `Player` create/load/login lifecycle: `Player::Create`, `LoadFromDB`, login packet sequencing, world insertion, visibility bootstrap, stats initialization and DB2-backed `GetPowerIndexByClass`.
- [ ] **#NEXT.R8.ENTITIES.013** Complete `Player` inventory/equipment bridge: real `Item` containers, equipment slots, visible items, `InvSlots`, buyback, `ObjectAccessor` `TYPEMASK_ITEM`, and save/load persistence.
- [ ] **#NEXT.R8.ENTITIES.014** Complete `Player` gameplay state: quests, skills, spells/actions, taxi, social, mail, group/guild, battleground/arena queues, reputation, achievements, cooldowns and rest state.
- [ ] **#NEXT.R8.ENTITIES.016** Complete `Creature` create/load/template lifecycle: `Creature::Create`, `CreateCreatureFromDB`, `LoadFromDB`, creature template/difficulty/model refs, spawn data, equipment, level/stat selection and map insertion.
- [ ] **#NEXT.R8.ENTITIES.017** Complete `Creature` runtime lifecycle: update loop, death/corpse/respawn transitions, forced despawn, evade/combat cleanup, loot owner/tap list, reputation, pickpocket and grid unload bindings.
- [ ] **#NEXT.R8.ENTITIES.018** Move real AI ownership from `wow-ai`/`wow-world::WorldCreature` bridge into canonical `Creature`/Map refs without mixing entity state into session.
- [ ] **#NEXT.R8.ENTITIES.020** Complete `GameObject` create/load/template lifecycle: `GameObject::Create`, `CreateGameObjectFromDB`, template/addon refs, rotations, model/collision creation, spawn data, map insertion and respawn compatibility.
- [ ] **#NEXT.R8.ENTITIES.021** Complete `GameObject` runtime lifecycle: update loop, loot/use state machine, door/button/trap/chest/fishing/destructible behavior, cooldown/restock, despawn/respawn persistence and grid unload bindings.
- [ ] **#NEXT.R8.ENTITIES.023** Complete `Corpse` create/load/persistence lifecycle: player-owned corpse creation, DB save/load/delete, character cache invalidation, phasing, loot object and map registration.
- [ ] **#NEXT.R8.ENTITIES.025** Complete `DynamicObject` create/add-to-map/update runtime: caster map inheritance, GUID creation, phase inheritance, caster registration, Aura ownership/removal, SpellInfo lookup, farsight viewpoint, transport passenger offset and map relocation.
- [ ] **#NEXT.R8.ENTITIES.027** Complete `AreaTrigger` create/load/update runtime: AreaTriggerDataStore templates/spawns, GUID creation, phase inheritance, static spawn store, AI selection, shape search, unit enter/exit actions, splines/orbit/attached movement, server-side visibility, transport offset and map relocation.
