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

- [x] **#NEXT.R8.ENTITIES.028** Port `SceneObject` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/SceneObject/SceneObject.h`, `SceneObject.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/scene_object.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `SceneObject` constructor matches C++ type id/mask, `WorldObject(false)`, stationary and scene-object create flags, stationary position and created-by-spell-cast bridge; `ShouldBeRemoved` predicate shape is represented; `SceneObjectData` script package, random seed, created-by and scene type setters use C++ bit positions; values update sets `TYPEID_SCENEOBJECT`.

- [x] **#NEXT.R8.ENTITIES.030** Port `Conversation` base state and core setters.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Conversation/Conversation.h`, `Conversation.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Globals/ConversationDataStore.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/conversation.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Conversation` constructor matches C++ type id/mask, `WorldObject(false)`, stationary and conversation create flags, creator/duration/texture/stationary-position defaults; lines, actors and last-line-end-time use C++ `ConversationData` bits; actor world-object/talking-head variants match C++ field shape; max last-line-end-time plus 10s despawn delay is represented; values update sets `TYPEID_CONVERSATION`.

- [x] **#NEXT.R8.ENTITIES.032** Port `Totem` base state and core rules.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Totem/Totem.h`, `Totem.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Creature/TemporarySummon.h`, `TemporarySummon.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.h`, `/home/server/woltk-trinity-legacy/src/server/game/Miscellaneous/SharedDefines.h`.
  Rust targets: `crates/wow-entities/src/totem.rs`, `crates/wow-entities/src/creature.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Totem` remains a `Creature`/`Unit` shape with `UNIT_MASK_SUMMON|UNIT_MASK_MINION|UNIT_MASK_TOTEM`; owner/summoner bridge, properties slot, totem type and duration defaults match C++; inherited spell slots back `GetSpell(slot)`; passive/active init-summon rules, update duration/owner-alive unsummon shape, delayed unsummon bridge, totem-created packet slot offset and positive/aura immunity special cases are represented.

- [x] **#NEXT.R8.ENTITIES.034** Port `Pet` base state, spell map and stable slot helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Pet/Pet.h`, `Pet.cpp`, `PetDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Creature/TemporarySummon.h`, `TemporarySummon.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/UnitDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Creature/CreatureData.h`.
  Rust targets: `crates/wow-entities/src/pet.rs`, `crates/wow-entities/src/totem.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Pet` constructor matches C++ `Guardian(nullptr, owner, true)` shape with `UNIT_MASK_SUMMON|MINION|GUARDIAN|PET|CONTROLABLE_GUARDIAN` and hunter-pet mask branch; name, pet type, duration, loading, removed, focus regen timer, group update mask and specialization defaults match C++; pet spell map/autospells and autocast toggles follow `PetSpell` field shape; `PetSaveMode` active/stable ranges and `GetLoadPetInfo` priority order are represented; pet XP factor is recorded.

- [x] **#NEXT.R8.ENTITIES.036** Port `Vehicle` base kit and seat helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Vehicle/Vehicle.h`, `Vehicle.cpp`, `VehicleDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Structure.h`.
  Rust targets: `crates/wow-entities/src/vehicle.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Vehicle` is represented as a kit attached to a unit GUID/type/position, not as an independent object; vehicle id, creature entry, status, usable seat count, seats, passenger info, seat addon, accessory and template structures match C++ shape; passenger add/remove/remove-all and pending join-event seat checks are represented; `TransportBase::CalculatePassengerPosition/Offset` formulas are ported and round-trip tested.

- [x] **#NEXT.R8.ENTITIES.038** Port `Transport` base state and passenger-set helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Transport/Transport.h`, `Transport.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Maps/TransportMgr.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/GameObject/GameObject.h`, `GameObject.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Miscellaneous/SharedDefines.h`.
  Rust targets: `crates/wow-entities/src/transport.rs`, `crates/wow-entities/src/game_object.rs`, `crates/wow-entities/src/object.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Transport` is represented as a `GameObject`/map-object transport with `SERVER_TIME|STATIONARY|ROTATION` create flags; movement state, current path leg, request-stop timestamp, path progress, position-change timer, delayed-add-model flag, event trigger bitset shape and dynamic/static passenger GUID sets match C++ base fields; `GetTransportPeriod`/`SetPeriod` use `GameObjectData::Level`; path-progress-for-client encodes into high dynamic-flag bits; passenger add/remove/cleanup/unload helpers and `TransportBase::CalculatePassengerPosition/Offset` formulas are represented and tested.

- [x] **#NEXT.R8.ENTITIES.040** Port pure `WorldObject`/`Position` geometry helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Position.h`, `Position.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Object.cpp`.
  Rust targets: `crates/wow-entities/src/world_object.rs`.
  Acceptance: absolute/relative angle conversion, `HasInArc`, `isInFront`, `isInBack`, `HasInLine`, rotated `IsWithinBox` and double vertical cylinder checks match C++ pure math semantics and are tested; LOS, terrain height, transport relocation and visibility-range hooks remain pending because they require canonical `Map`/terrain/transport ownership.

- [x] **#NEXT.R8.ENTITIES.041** Port `Item` base state and `UF::ItemData` masks.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`, `Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemTemplate.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemDefines.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`, `UpdateFields.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPacketsCommon.h`.
  Rust targets: `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`, `crates/wow-constants/src/item.rs`.
  Acceptance: base `Item` is represented as `Object`/`TYPEID_ITEM`/`TYPEMASK_ITEM`, not `WorldObject`; constructor state, slot/container bridge, update queue state, trade/refund/text fields, core `Create` initialization shape, dynamic item flags/flags2, stack/durability/expiration/context/appearance, spell charges, enchantments, item bonus key and `UF::ItemData` 43-bit masks are ported and tested. Template lookup, Bag/Container, DB save/load, item update packet serializers and Player inventory ownership remain pending under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.042** Port `Bag` base state and `UF::ContainerData` masks.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Container/Bag.h`, `Bag.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`, `UpdateFields.cpp`.
  Rust targets: `crates/wow-entities/src/bag.rs`, `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/update_fields.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: base `Bag` retags `Item` to `TYPEID_CONTAINER`/`TYPEMASK_CONTAINER`, preserves `MAX_BAG_SIZE=36`, owns a GUID slot bridge for `m_bagslot`, ports `ContainerData::NumSlots` and `Slots[36]` bit positions, rejects templates with too many slots, stores/removes child `Item` state like C++ `StoreItem`/`RemoveItem`, and emits values updates with `TYPEID_CONTAINER`. Real `Item*` ownership/destruction, DB save/load recursion, Player inventory indexes and packet serializers remain pending under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.043** Port `Player` inventory storage lookup bridge.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `m_items[PLAYER_SLOT_END=141]`, `m_currentBuybackSlot`, storage slot constants, `ForEachItem` search locations, top-level/bag/reagent/bank GUID storage, `GetItemByPos`, packed-pos lookup, `GetBagByPos`, `GetItemByGuid`, buyback slots and `ActivePlayerData::{InvSlots,BuybackPrice,BuybackTimestamp}` masks are represented and tested against C++ lookup rules. Actual `Item*` ownership, item mutation side effects, equip spell/aura/stat application, visible item data, DB persistence and packet serializers remain pending under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.044** Wire `ObjectAccessor` `TYPEMASK_ITEM` lookup branch.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectAccessor.cpp`, `ObjectAccessor.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/object_accessor.rs`, `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `GetObjectByTypeMask` item semantics are represented: item GUID lookup only works when `typemask` contains `TYPEMASK_ITEM` and the context object is a player; the branch delegates to Player inventory storage. Because C++ returns `Object*` and Rust `Item` is not a `WorldObject`, a new `AccessorObjectRef::{WorldObject,Item}` API carries item hits while the legacy `get_object_by_type_mask` remains world-object-only. Real `Player` ownership storage, item object registry and packet serializers remain #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.045** Port `Player` visible item slot state.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/Updates/UpdateFields.h`, `UpdateFields.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `UF::VisibleItem` value shape (`ItemID`, `ItemAppearanceModID`, `ItemVisual`), `PlayerData::VisibleItems[19]` array bits (`61`, `62..80`), `SetVisibleItemSlot` clear/set semantics and the equipment-slot branch of `VisualizeItem` are represented and tested. C++ template-dependent `Item::GetVisibleEntry/GetVisibleAppearanceModId/GetVisibleItemVisual`, BoE/BoA binding, real `Item*` ownership side effects and final nested update-field packet serializers remain pending under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.046** Fix `InventoryType` bridge against C++ signed DB2 field and bag slots.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Structure.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemTemplate.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-data/src/item.rs`, `crates/wow-packet/src/packets/item.rs`, `crates/wow-world/src/handlers/character.rs`.
  Acceptance: `ItemEntry::InventoryType` is treated as signed at the DB2 boundary, negative and zero values do not map to equipment slots, `-1` no longer wraps to `INVENTORY_SLOT_BAG_0=255`, and `INVTYPE_BAG=18` maps to C++ equipped bag slots `30..33` instead of an equipment display slot. Existing flat `wow-world` inventory remains a temporary bridge until canonical `Player`/`Item` ownership replaces it under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.047** Port `Item` visible transmog/enchant modifier helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `Item.h`, `ItemDefines.h`.
  Rust targets: `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `AppearanceModifierSlotBySpec`, `IllusionModifierSlotBySpec`, `SecondaryAppearanceModifierSlotBySpec`, `GetModifier`/`SetModifier`, `GetVisibleEntry`, `GetVisibleAppearanceModId`, `GetVisibleEnchantmentId`, `GetVisibleItemVisual` and secondary appearance precedence are represented and tested. DB2 resolver stores for `ItemModifiedAppearance` and `SpellItemEnchantment` remain explicit caller-provided bridges until canonical `wow-data` stores are ported under DataStores/#NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.048** Port `Player::VisualizeItem` item mutation side effects.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: `VisualizeItem` binding behavior for `BIND_ON_EQUIP`, `BIND_ON_ACQUIRE` and `BIND_QUEST`, top-level inventory slot storage, item `ContainedIn`/`OwnerGUID`/slot/container reset and `ITEM_CHANGED` transition are represented and tested. Collection appearance registration, equip stat/aura/spell application, canonical item object registry, DB persistence and packet serializers remain under #NEXT.R8.ENTITIES.013/#008.

## Follow-Up Work Items

- [ ] **#NEXT.R8.ENTITIES.003** Bind `wow-map` grid unload actions to real entity methods once Creature/GameObject/Corpse exist.
- [ ] **#NEXT.R8.ENTITIES.005** Port `WorldObject` LOS, terrain-height, transport relocation and visibility-range helpers that require Map/Terrain/Transport integration.
- [ ] **#NEXT.R8.ENTITIES.006** Complete remaining `ObjectAccessor` integration: real `SaveAllPlayers` and wiring to canonical `wow_map::Map` containers instead of bridge storage.
- [ ] **#NEXT.R8.ENTITIES.008** Complete generated update-field sections beyond `ObjectData`: `UnitData`, `PlayerData`, `ActivePlayerData`, `GameObjectData`, `ItemData`, `CorpseData`, `DynamicObjectData`, `AreaTriggerData`, `SceneObjectData`, `ConversationData`, including visibility flag filters and dynamic/optional fields.
- [ ] **#NEXT.R8.ENTITIES.010** Complete `Unit` subsystems beyond base fields: aura hooks, threat/combat manager, SpellHistory, MotionMaster/move spline, charm/minion ownership, vehicle hooks, AI references and runtime power-index implementations for Player/Creature/Pet.
- [ ] **#NEXT.R8.ENTITIES.012** Complete `Player` create/load/login lifecycle: `Player::Create`, `LoadFromDB`, login packet sequencing, world insertion, visibility bootstrap, stats initialization and DB2-backed `GetPowerIndexByClass`.
- [ ] **#NEXT.R8.ENTITIES.013** Complete `Player` inventory/equipment bridge: build on base `Item`/#NEXT.R8.ENTITIES.041, `Bag`/#NEXT.R8.ENTITIES.042, Player storage lookup/#NEXT.R8.ENTITIES.043, `ObjectAccessor` item branch/#NEXT.R8.ENTITIES.044, visible item state/#NEXT.R8.ENTITIES.045, item visible modifier helpers/#NEXT.R8.ENTITIES.047 and `VisualizeItem` mutation side effects/#NEXT.R8.ENTITIES.048 to port canonical item object registry, DB2 resolver stores, collection appearance registration, equip stat/aura/spell side effects and save/load persistence.
- [ ] **#NEXT.R8.ENTITIES.014** Complete `Player` gameplay state: quests, skills, spells/actions, taxi, social, mail, group/guild, battleground/arena queues, reputation, achievements, cooldowns and rest state.
- [ ] **#NEXT.R8.ENTITIES.016** Complete `Creature` create/load/template lifecycle: `Creature::Create`, `CreateCreatureFromDB`, `LoadFromDB`, creature template/difficulty/model refs, spawn data, equipment, level/stat selection and map insertion.
- [ ] **#NEXT.R8.ENTITIES.017** Complete `Creature` runtime lifecycle: update loop, death/corpse/respawn transitions, forced despawn, evade/combat cleanup, loot owner/tap list, reputation, pickpocket and grid unload bindings.
- [ ] **#NEXT.R8.ENTITIES.018** Move real AI ownership from `wow-ai`/`wow-world::WorldCreature` bridge into canonical `Creature`/Map refs without mixing entity state into session.
- [ ] **#NEXT.R8.ENTITIES.020** Complete `GameObject` create/load/template lifecycle: `GameObject::Create`, `CreateGameObjectFromDB`, template/addon refs, rotations, model/collision creation, spawn data, map insertion and respawn compatibility.
- [ ] **#NEXT.R8.ENTITIES.021** Complete `GameObject` runtime lifecycle: update loop, loot/use state machine, door/button/trap/chest/fishing/destructible behavior, cooldown/restock, despawn/respawn persistence and grid unload bindings.
- [ ] **#NEXT.R8.ENTITIES.023** Complete `Corpse` create/load/persistence lifecycle: player-owned corpse creation, DB save/load/delete, character cache invalidation, phasing, loot object and map registration.
- [ ] **#NEXT.R8.ENTITIES.025** Complete `DynamicObject` create/add-to-map/update runtime: caster map inheritance, GUID creation, phase inheritance, caster registration, Aura ownership/removal, SpellInfo lookup, farsight viewpoint, transport passenger offset and map relocation.
- [ ] **#NEXT.R8.ENTITIES.027** Complete `AreaTrigger` create/load/update runtime: AreaTriggerDataStore templates/spawns, GUID creation, phase inheritance, static spawn store, AI selection, shape search, unit enter/exit actions, splines/orbit/attached movement, server-side visibility, transport offset and map relocation.
- [ ] **#NEXT.R8.ENTITIES.029** Complete `SceneObject` create/map/update runtime: SceneTemplate lookup, GUID creation, private object owner, phase inheritance, random seed time source, map insertion, creator/aura lookup and removal scheduling.
- [ ] **#NEXT.R8.ENTITIES.031** Complete `Conversation` create/start/update runtime: ConversationDataStore templates, conditions, actor fill visitor, line locale timings, private owner locale, script hooks, map insertion, actor unit/creature lookup and removal scheduling.
- [ ] **#NEXT.R8.ENTITIES.033** Complete `TempSummon`/`Minion`/`Totem` runtime: SummonProperties, owner slots, usable totem slot selection, model lookup by spell/race, `SMSG_TOTEM_CREATED`, spell casting, CombatStop, aura removal from owner/group, cooldown event and map removal scheduling.
- [ ] **#NEXT.R8.ENTITIES.035** Complete `Pet` create/load/save/update runtime: pet GUID/create from DB/tamed creature, stable persistence, action bar, XP/level sync, stats, auras/cooldowns/charges, specialization/talents/passives, PetAI/charm info, group updates and map/object-store insertion.
- [ ] **#NEXT.R8.ENTITIES.037** Complete `Vehicle` runtime: DB2 vehicle/seat lookup, npc flags, install/uninstall/reset scripts, accessories, control auras, immunities, passenger relocation/exit, pending join events, despawn delay and integration with Unit movement/transport state.
- [ ] **#NEXT.R8.ENTITIES.039** Complete `Transport` runtime: `TransportMgr` template loading/path generation, `Transport::Create`, path update/event triggering, map transition teleport/hide behavior, static creature/GameObject passenger spawning, summon passenger path, passenger movement-info transport offsets, script hooks, model/collision update and map/grid integration.
