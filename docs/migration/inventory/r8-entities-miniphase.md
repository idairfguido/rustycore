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

- [x] **#NEXT.R8.ENTITIES.049** Port empty top-level branch of `Player::_StoreItem`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: empty-slot top-level `StoreItem` side effects are represented: `SetCount(count)`, bind-on-store for `BIND_ON_ACQUIRE`, `BIND_QUEST` and `BIND_ON_EQUIP` only when `IsBagPos(pos)`, slot storage, `ContainedIn`/`OwnerGUID`/slot/container reset and `ITEM_CHANGED` transition. Stack merge into an existing `Item*`, clone split storage, bag-contained branch, AddToWorld/update packets, enchant/item durations and obtain spells remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.050** Port empty bag-contained branch of `Player::_StoreItem`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Container/Bag.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/bag.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: `Player::_StoreItem`'s `pBag->StoreItem(slot, pItem, update)` branch is represented for empty bag slots: Player bag storage and `Bag::m_bagslot` stay in sync, child item count/owner/contained/container/slot/state are updated, `BIND_ON_ACQUIRE`/`BIND_QUEST` bind while `BIND_ON_EQUIP` does not bind for bag-contained positions, and the bag item receives `ITEM_CHANGED`. Existing-stack merge, clone split storage, AddToWorld/update packets, duration hooks, obtain spells and persistence remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.051** Port top-level existing-stack merge branch of `Player::_StoreItem`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: top-level existing stack merge is represented with GUID-slot validation, C++ bind-on-store rules on the existing stack, count increment, existing item `ITEM_CHANGED`, incoming item owner assignment, refundable/BOP-tradeable cleanup and `ITEM_REMOVED` transition. Bag-contained existing-stack merge, clone split semantics, world update packets, duration hooks, trade list removal, collection side effects, obtain spells and persistence remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.052** Port bag-contained existing-stack merge branch of `Player::_StoreItem`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Container/Bag.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/bag.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: bag-contained existing stack merge is represented with Player-storage and `Bag` slot GUID validation, C++ bind-on-store rules for bag-contained positions, count increment, existing item `ITEM_CHANGED`, incoming item owner assignment, refundable/BOP-tradeable cleanup and `ITEM_REMOVED` transition; the bag item is not marked changed because C++ does not change `m_bagslot` in this branch. Clone split semantics, world update packets, duration hooks, trade list removal, collection side effects, obtain spells and persistence remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.053** Port `Item::CloneItem` field subset and empty-slot clone storage.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/player.rs`.
  Acceptance: clone storage preserves the source item, creates a separate item with caller-provided GUID, copies entry/context/creator/gift creator/expiration and dynamic flags without `REFUNDABLE`/`BOP_TRADEABLE`, applies the requested count, and then stores the clone through the already ported empty top-level/bag `_StoreItem` branches. C++ ObjectMgr GUID generation, template-backed max-stack/max-durability/effect-charge lookup, `NewItemOrBag` subtype selection, split count allocation, world update packets, duration hooks and persistence remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.054** Port `Player::SplitItem` count allocation into empty storage branches.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: split helpers reject zero, all-count and too-large splits like C++ guard paths, preserve source count/state if destination storage fails, and on success clone the requested count into empty top-level/bag storage while decrementing and marking the source item changed. Full `SplitItem` destination resolution through `CanStoreItem`/`CanBankItem`/`CanEquipItem`, equip/bank-specific side effects, world update packets, duration hooks and persistence remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.055** Port representable `Player::SplitItem` loot/trade guards.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: split helpers reject source items with generated loot before count guards, reject all-count/too-large splits before trade checks, and reject source items marked in-trade without mutating source count/state. Full `TradeData::GetTradeSlotForItem` ownership, packet error mapping and destination validation remain under #NEXT.R8.ENTITIES.013/#008.

- [x] **#NEXT.R8.ENTITIES.056** Port static Player inventory position classifiers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `NULL_BAG`, packed item positions, `IsInventoryPos`, `IsEquipmentPos`, `IsBankPos`, `IsBagPos` and `IsChildEquipmentPos` are represented and tested against C++ slot ranges, including equipped bag slots being equipment, bag contents not being `IsBagPos`, keyring/child-equipment inventory handling and `NULL_SLOT` only matching inventory with `INVENTORY_SLOT_BAG_0`. `IsValidPos`, `CanStoreItem`/`CanBankItem`/`CanEquipItem` and destination vector generation remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.057** Port `Player::IsValidPos` inventory/bag rules.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: `Player::is_valid_pos` and packed-pos wrapper match C++ explicit/non-explicit handling for `NULL_BAG`, `NULL_SLOT`, top-level equipment/profession/bag/reagent slots, backpack slots limited by `GetInventorySlotCount`, bank main/bag/keyring ranges, and registered bag contents limited by bag size. Full `CanStoreItem`/`CanBankItem`/`CanEquipItem` destination vector generation and bank-bag purchase gating remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.058** Port `Item::CanBeMergedPartlyWith` guard subset.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`.
  Rust targets: `crates/wow-entities/src/item.rs`.
  Acceptance: `Item::can_be_merged_partly_with(entry, max_stack_size)` returns C++ `InventoryResult` values for generated-loot, entry mismatch, full-stack and mergeable-stack cases. The method takes the template-derived `entry/max_stack_size` values explicitly until canonical `ItemTemplate` lookup is ported under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.059** Port `ItemCanGoIntoBag` family/subclass rules.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemTemplate.h`.
  Rust targets: `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `ItemStorageTemplate` carries the template fields needed by storage validation and `item_can_go_into_bag` matches C++ regular container, profession/specialized container, reagent container and quiver/ammo-pouch family rules. Full template loading, `IsCraftingReagent` derivation and `CanStoreItem_InSpecificSlot` integration remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.060** Port `Player::CanStoreItem_InSpecificSlot` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `ItemPosCount`, duplicate destination suppression, empty-slot fit checks, existing-stack merge checks, source non-empty-bag/child-equipment guards and specialized bag-family integration match C++ `CanStoreItem_InSpecificSlot` using explicit `Item`/template bridge inputs. The current C++ keyring family check is preserved as written and tested as unreachable (`KEYRING_SLOT_START + KEYRING_SLOT_START - KEYRING_SLOT_END`); full object-registry lookup, template resolver loading, `_InBag`, `_InInventorySlots` and top-level `CanStoreItem` orchestration remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.061** Port `Player::CanStoreItem_InInventorySlots` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: top-level inventory-slot scanning matches C++ skip-slot, reagent-slot omission, source non-empty-bag rejection, move-source-as-empty handling, merge vs empty-slot modes, mergeability filtering, duplicate destination suppression and early return when requested count reaches zero. Full object-registry lookup, template resolver loading, `_InBag` and top-level `CanStoreItem` orchestration remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.062** Port `Player::CanStoreItem_InBag` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: bag-contained scanning matches C++ skip-bag, missing/self-target bag rejection, source non-empty-bag and child-item rejection, bag-template presence, specialized vs regular bag mode, `ItemCanGoIntoBag`, skip-slot, move-source-as-empty handling, merge vs empty-slot modes, mergeability filtering, duplicate destination suppression and early return when count reaches zero. Full object-registry lookup, template resolver loading and top-level `CanStoreItem` orchestration remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.063** Port `Player::CanStoreItem` representable orchestration.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: top-level storage orchestration matches C++ template-missing, source loot/bound, partial/full similar-item limit, specific-slot first pass, specific-bag/inventory pass, general merge pass, keyring/specialized bag pass, non-empty-bag-in-bag rejection, child-equipment free pass, new regular bag direct-equip search, fallback inventory/bag search and `InventoryResult`/`no_space_count` outcomes using explicit bridge inputs for template lookup, bag templates, slot item refs, `CanTakeMoreSimilarItems` and bound-with-player checks. Canonical template resolver, item object registry, real `CanTakeMoreSimilarItems`, BOA/BOP allowed-owner logic and top-level overloads remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.064** Port `Player::CanTakeMoreSimilarItems` representable max-count checks.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DBCEnums.h`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Structure.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: template-missing, generated-loot, no-max, `INT32_MAX` max-count, entry max-count overflow, missing item-limit category entry, `ITEM_LIMIT_CATEGORY_MODE_HAVE` count overflow with offending item id, and `ITEM_LIMIT_CATEGORY_MODE_EQUIP` ignore path match C++ outcomes. `CanStoreItem` now computes similar-item limits through this helper; real inventory/bank/reagent item counting, gem count inclusion and DB2 condition-adjusted `GetItemLimitCategoryQuantity` remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.065** Port representable `GetItemCount` / `GetItemCountWithLimitCategory` counting.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: entry counting matches C++ location filtering for equipment/inventory plus optional bank, skips the source item by GUID and sums stack counts; limit-category counting matches C++ everywhere scan, skips source item and sums counts for templates with the requested limit category. `CanStoreItem` now derives current entry/category counts from explicit stored-item refs. Real object registry traversal, reagent-bank location, socketed gem counting and DB2-backed template lookup remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.066** Port representable `Item::IsBindedNotWith`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.h`.
  Rust targets: `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/player.rs`.
  Acceptance: soulbound, owner GUID, BOP-tradeable allowed-player and account-bound template cases match C++ `Item::IsBindedNotWith`; `CanStoreItem` now calls the item helper instead of receiving a precomputed bound-with-player boolean. Real allowed-GUID ownership set and template resolver-backed `IsBoundAccountWide` remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.067** Port `Player::CanBankItem` representable orchestration.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: bank storage orchestration matches C++ null item/template, generated-loot, bound-owner, currency-token, similar-item limit, bank-bag slot `WrongSlot`/`NoBankSlot`/`CanUseItem`, specific-slot first pass, specific bank bag/main-bank pass, general bank merge pass, special/regular bank bag search, main-bank free-slot search and `BankFull` fallback using explicit bridge inputs for source bag/currency/can-use and templates. Real `Item::IsBag`, `Item::IsCurrencyToken`, `CanUseItem`, object registry and template resolver remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.068** Port `Player::FindEquipSlot` representable destination selection.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: equipment destination selection matches C++ inventory-type mapping, explicit requested-slot/swap handling, free-slot search, offhand rejection while a two-hand item is used, swap replacement by lowest equipped item level, bag slot candidates, dual-wield/titan-grip gating and profession tool/gear routing. The C++ primary-profession gear overwrite behavior is preserved and documented. Real `CanEquipItem`, `CanUseItem`, player skill registry, `GetProfessionSlotFor`, item level resolver and equipped item object registry remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.069** Port `ItemTemplate::CanChangeEquipStateInCombat` representable helper.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemTemplate.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemTemplate.h`.
  Rust targets: `crates/wow-entities/src/item.rs`.
  Acceptance: combat equip-state allowance matches C++ inventory-type exceptions (`Relic`, `Shield`, `Holdable`) and item-class exceptions (`Weapon`, `Projectile`), with other equipment templates rejected. Full `Player::CanEquipItem` runtime combat/arena/casting checks remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.070** Port `Player::CanEquipItem` representable orchestration.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: equipment validation matches C++ null item/template, generated-loot, bound-owner, similar-item limit, not-loading runtime guards, heirloom-level failure hook, `FindEquipSlot`, `CanUseItem`, occupied destination ordering, unique-equipped ignore-slot calculation, quiver/ammo uniqueness, offhand dual-wield/two-hand rules, two-hand offhand-store fallback and destination packing. Real `CanUseItem`, `CanEquipUniqueItem`, equipped object registry, skill/profession registries, spell state, battleground state, weapon-change timer and DB2 heirloom/content tuning remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.071** Port `Player::CanUnequipItem` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: unequip validation matches C++ equipment/bag-position applicability, empty-slot OK path, missing template, generated-loot, charmed, combat/arena restrictions using `CanChangeEquipStateInCombat`, non-empty bag rejection when not swapping and swap allowance. Real equipped object lookup and runtime player combat/battleground/charm state remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.072** Port `Player::CanUseItem(ItemTemplate const*)` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: template use validation matches C++ null template, internal/faction/class/race gates, required skill value/rank, required spell, optional required-level check, holiday lockout, reputation rank, already-known learning effect guard and artifact specialization mismatch. Real template flag extraction, race/class mask resolver, skill/spell/reputation/holiday/artifact stores and object-level `CanUseItem(Item*)` remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.073** Port `Player::CanUseItem(Item*)` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: item use validation matches C++ null item, dead-player `not_loading` guard, missing template, bound-owner, item required-level, template helper call with required-level skip, skill proficiency and heirloom armor morph allowance for Hunter/Shaman mail and Paladin/Warrior plate. Real item required-level/skill extraction, item quality resolver, player class/skill stores and full template resolver remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.074** Port `Player::CanEquipUniqueItem(ItemTemplate const*)` representable validation.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: unique-equipped template validation matches C++ unique-equippable entry/gem checks, except-slot exclusion, missing item-limit category, limit-count overflow, equipped item limit-category overflow and socketed gem limit-category overflow. Real item/gem equipped scans, DB2 condition-adjusted `GetItemLimitCategoryQuantity` and object-level socketed-gem wrapper remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.075** Port `Player::CanEquipUniqueItem(Item*)` representable wrapper.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: object-level unique-equipped validation matches C++ base-template-first ordering, socketed gem traversal, missing gem-template skip, unequipped source gem limit-category count reuse and equipped source gem limit count of one. Real `sObjectMgr` gem template resolution, direct socket field traversal and DB2-backed source gem category counting remain under #NEXT.R8.ENTITIES.013.

- [x] **#NEXT.R8.ENTITIES.076** Port representable `Player::EquipItem` item-storage side effects.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: equip side effects match C++ empty-slot `VisualizeItem` call, item owner/contained/slot/container mutation, visible item update, bind-on-visualize behavior, equipped dynamic flag2, and existing-stack merge path with incoming item refundable/BOP-trade cleanup plus removed/changed states. Enchantment/item durations, set bonuses, item mods, weapon-swap cooldown packet, world add/remove updates, equip cooldown, expertise/rating/titan grip, criteria and average item-level updates remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.077** Port representable `Player::QuickEquipItem` item-storage side effects.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: quick-equip side effects match C++ null-free path through `VisualizeItem`, owner/contained/slot/container mutation, visible item update, item changed state and equipped dynamic flag2. Enchantment/item durations, world add/update packets, titan grip and criteria updates remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.078** Port representable `Player::RemoveItem` storage unlink.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Container/Bag.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: remove-item unlink matches C++ no-item no-op, top-level storage clear, visible equipment clear, equipped dynamic flag2 removal for top-level inventory/bag slots, bag-contained `Bag::RemoveItem` slot clear, item contained/slot/container unlink, and owner preservation. Enchantment/item duration removal, tradeable removal, set bonus/item mod/aura/enchant/rating/titan-grip/average-ilvl updates, packet sends and update-queue handling remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.079** Port representable `Player::MoveItemFromInventory` unlink wrapper.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: move-from-inventory wrapper matches C++ no-item no-op, delegates to `RemoveItem` storage unlink and clears refundable state, refund recipient, paid money and paid extended cost while preserving owner. Quest removed counters, update-queue removal, temporary appearance removal and world destroy/update packets remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.080** Port representable `Player::MoveItemToInventory` post-store finalization.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: move-to-inventory finalization matches C++ original-item-only post-store handling: merged stack results are left untouched, original item owner is corrected to player GUID, and state becomes `ITEM_CHANGED` for character-inventory DB rows or `ITEM_NEW` otherwise. Full `StoreItem(dest, pItem)` multi-destination orchestration, BOP-tradeable registration, quest added counters and obtain criteria remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.081** Port representable `Player::DestroyItem` storage and item cleanup.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Container/Bag.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: destroy-item cleanup matches C++ no-item no-op, top-level storage clear, visible equipment clear, bag-contained `Bag::RemoveItem` slot clear, refundable and BOP-tradeable cleanup, item contained/slot/container unlink, owner preservation and `ITEM_REMOVED` state. The C++ behavior of not explicitly clearing `ITEM_FIELD_FLAG2_EQUIPPED` during destroy is preserved for removed equipped items. Recursive non-empty bag destruction, wrapped gift DB delete, loot-storage cleanup, duration/tradeable/spell/script hooks, quest counters, world packets and average item-level/stat updates remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.082** Port representable `Player::DestroyItemCount(Item*, uint32&)`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: item-pointer destroy-count overload matches C++ null no-op, full-stack path subtracting the item count and delegating to `DestroyItem`, and partial-stack path decrementing item count, zeroing requested count and marking `ITEM_CHANGED`. Quest removed counters and world update packet sends remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.083** Port representable `Player::DestroyItemCount(entry, count)` scan planning.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: destroy-count-by-entry planning matches C++ scan order across inventory, keyring, inventory bag contents, equipment/bag-list, bank, bank bag contents, bank bag-list and child equipment; skips in-trade/nonmatching entries; emits full-stack destroy actions vs partial-stack decrement actions; stops when requested count is reached; and applies `unequip_check` only to full-stack top-level equipment/bag-list and bank-bag-list destroys. Real action execution across object registry, mutable bag refs, quest counters and world packets remains under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.084** Port representable `DestroyZoneLimitedItem` and `DestroyConjuredItems` scan planning.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: filtered destroy planning matches C++ scan order for zone-limited items over inventory, keyring, inventory bag contents and equipment/bag-list, and for conjured items over inventory, inventory bag contents and equipment/bag-list without keyring. Real `IsLimitedToAnotherMapOrZone`, `IsConjuredConsumable`, action execution through `DestroyItem`, map/zone state and world packets remain under #NEXT.R8.ENTITIES.013/#NEXT.R8.ENTITIES.014.

- [x] **#NEXT.R8.ENTITIES.085** Port representable `Player::GetItemByEntry` and `GetItemListByEntry`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: entry lookup and listing use the C++ `ForEachItem` order over equipment, inventory, optional bank and reagent bank; `GetItemByEntry` stops on the first matching entry; `GetItemListByEntry` excludes bank by default and includes it before reagent bank when requested. Real object registry resolution remains bridged by caller-provided `ItemStorageRef` slices until #NEXT.R8.ENTITIES.013 owns canonical `Item*` storage.

- [x] **#NEXT.R8.ENTITIES.086** Port representable `Player::RemoveItemFromBuyBackSlot` item side effects.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: buyback removal now has an object-aware path matching C++ slot clearing, `InvSlots`/buyback price/timestamp reset, current-buyback-slot reuse, item `RemoveFromWorld`, and optional `ITEM_REMOVED` state when `del` is true. Stored-loot removal for `ITEM_FLAG_HAS_LOOT` remains parked until loot storage ownership is canonical.

- [x] **#NEXT.R8.ENTITIES.087** Port representable weapon/titan-grip helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `IsUseEquipedWeapon`, `SetCanTitanGrip`, `IsTwoHandUsed`, `IsUsingTwoHandedWeaponInOneHand` and the decision branch of `CheckTitanGripPenalty` are represented against explicit form/disarm/template/aura inputs. Actual aura lookup, `CastSpell` and `RemoveAurasDueToSpell` remain under spell/aura runtime work.

- [x] **#NEXT.R8.ENTITIES.088** Port representable `Player::SwapItem` preflight guards.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the first `SwapItem` decision block is represented with C++ order: missing source no-op, CHILD item redirect pairs, dead-player error before unequip checks, source unequip check with exact `swap` boolean, bag-in-self and same-slot-from-inside guards, and destination unequip check with exact `swap` boolean. Actual recursive swap execution, validation/store/equip calls, merge/fill, real two-item swaps, bag contents exchange, aura cleanup and loot release remain pending.

- [x] **#NEXT.R8.ENTITIES.089** Port representable `Player::SwapItem` empty-destination move plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the `!pDstItem` branch is represented with C++ destination ordering: inventory calls `CanStoreItem` and moves through `StoreItem` with bank-source quest-added hook, bank calls `CanBankItem` and moves through `BankItem` with quest-removed hook, equipment calls `CanEquipItem` and schedules `AutoUnequipOffhandIfNeed`, and invalid empty destinations return without action. Actual object mutation remains in the existing store/bank/equip/remove primitives until full `SwapItem` orchestration is wired.

- [x] **#NEXT.R8.ENTITIES.090** Port representable `Player::SwapItem` merge/fill plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the occupied non-bag merge/fill attempt is represented with C++ destination validation order, invalid-destination no-op, validation-failure fallthrough to real swap, full-stack merge through inventory/bank/equip destinations, partial fill count math, in-world update flag and refund-info scheduling. Real mutation remains pending in full `SwapItem` orchestration.

- [x] **#NEXT.R8.ENTITIES.091** Port representable `Player::SwapItem` real-swap validation plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: bidirectional real-swap validation is represented in C++ order: source-to-destination checks inventory/bank/equip with `swap=true`, equip destinations chain `CanUnequipItem(eDest,true)`, source-side failures report against source, destination-to-source repeats the same selection and reports against destination, and unclassified positions preserve the C++ no-validation shape. Bag content exchange, remove/store/equip execution, aura cleanup and loot release remain pending.

- [x] **#NEXT.R8.ENTITIES.092** Port representable `Player::SwapItem` bag-content exchange plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the special two-bag exchange branch is represented: it only triggers when both items are bags and exactly one empty bag is outside a bag position, validates every full-bag item against the empty bag, rejects incompatible contents with `BagInBag`, rejects too-small empty bags with `CantSwap`, and plans compacted slot moves in C++ scan order. Actual `Bag::RemoveItem`/`StoreItem` execution remains pending in full `SwapItem` orchestration.

- [x] **#NEXT.R8.ENTITIES.093** Port representable `Player::SwapItem` final real-swap execution plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: final real-swap actions are represented: destination is removed before source with `update=false`, source and destination are re-added to their planned targets, item-dependent aura cleanup is requested when either packed position is a top-level equipment/bag slot, loot release is requested when active loot exists in a moved bag, and `AutoUnequipOffhandIfNeed` is scheduled. Actual object mutation remains pending in full orchestration.

- [x] **#NEXT.R8.ENTITIES.094** Port representable `Player::SwapItem` branch orchestration plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the full `SwapItem` decision order is represented across the already ported phase plans: preflight/no-source/child-redirect/error terminals, empty-destination move terminals, merge/fill terminals, real-swap bidirectional validation, bag-content exchange, and final real-swap execution. Error item ordering matches C++ `SendEquipError` call sites and missing or inconsistent downstream phases are explicit plan results instead of silent success. Actual recursive calls and object mutation remain pending in the full `SwapItem` executor.

- [x] **#NEXT.R8.ENTITIES.095** Port object-aware `Player::AddItemToBuyBackSlot`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/item.rs`.
  Acceptance: buyback insertion now has the C++ object-aware path: selects current/free/oldest slot, removes overwritten buyback item with delete side effects, stores the new item GUID, computes buyback price from template sell price times item count, computes timestamp as `game_time - login_time + 30h`, updates `InvSlots`/buyback masks, and advances `m_currentBuybackSlot` with C++ non-wrapping behavior. Stored-loot removal for overwritten loot containers remains parked until loot storage ownership is canonical.

- [x] **#NEXT.R8.ENTITIES.096** Port representable `Player::SendEquipError` packet shape.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.*`.
  Rust targets: `crates/wow-packet/src/packets/item.rs`.
  Acceptance: `InventoryChangeFailure` now uses canonical `wow-constants::InventoryResult` values instead of the stale local duplicate, preserves C++ bag-result/item GUID/container slot order, writes `Level` for `CantEquipLevelI` and `PurchaseLevelTooLow`, writes `LimitCategory` for the three item-limit-category failures, and writes the auto-equip bind-confirm source/destination context. Player/session convenience wrapper is closed by #NEXT.R8.ENTITIES.112.

- [x] **#NEXT.R8.ENTITIES.097** Port representable `Player::SendBuyError`/`SendSellError` packet shape.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.*`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/ItemDefines.h`.
  Rust targets: `crates/wow-packet/src/packets/misc.rs`, `crates/wow-world/src/handlers/character.rs`.
  Acceptance: `BuyFailed` now carries canonical `BuyResult` values and serializes `Reason` as C++ `uint8`; `SellResponse` now serializes `VendorGUID`, item GUID count, `Reason` as `int32`, then each item GUID, with helper constructors for C++ sell-error responses and current Rust success responses. Existing world buy/sell handlers were updated away from raw reason bytes to canonical error values.

- [x] **#NEXT.R8.ENTITIES.098** Port representable `Player` soulbound tradeable item set.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `Player` now owns the `m_itemSoulboundTradeable` equivalent set, supports C++ `AddTradeableItem`/`RemoveTradeableItem` GUID insertion/removal, and represents `UpdateSoulboundTradeItems` cleanup by removing entries whose item is missing, owned by another player, or whose trade window has expired. The actual `Item::CheckSoulboundTradeExpire` time calculation remains pending with canonical item trade timers.

- [x] **#NEXT.R8.ENTITIES.099** Port representable `Player` item and enchantment duration lists.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `Player` now owns the `m_itemDuration` and `m_enchantDuration` equivalents, ports `AddItemDurations`/`RemoveItemDurations` and `UpdateItemDuration` as C++-ordered actions, ports `AddEnchantmentDurations`, `AddEnchantmentDuration`, `RemoveEnchantmentDurations`, `RemoveEnchantmentDurationsReferences` and `UpdateEnchantTime`, preserves replacement behavior that saves old enchant left-duration back into the item, converts enchant time updates from milliseconds to seconds like C++, and emits explicit actions for expired/missing enchantments. Actual item expiration destruction, script hook dispatch, `ApplyEnchantment` stat effects and packet sending remain pending in canonical runtime wiring.

- [x] **#NEXT.R8.ENTITIES.100** Port representable `Player::RemoveArenaEnchantments`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: arena enchantment cleanup is represented in C++ order: first walks `m_enchantDuration`, keeps duration entries whose enchantment is arena-allowed, clears non-allowed equipped enchantments with stat-removal action, removes stale/zero duration references, then scans backpack slots up to `GetInventorySlotCount()` and equipped inventory bags in slot order to clear non-allowed inventory enchantments. Real `sSpellMgr->IsArenaAllowedEnchancment`, `ApplyEnchantment`, item mutation and packet/update dispatch remain pending in runtime wiring.

- [x] **#NEXT.R8.ENTITIES.101** Port representable duration resend helpers.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`.
  Acceptance: `SendEnchantmentDurations` and `SendItemDurations` are represented over the canonical player duration lists, preserving list order, duplicate entries, and C++ enchant milliseconds-to-seconds conversion for `SendItemEnchantTimeUpdate`. Session packet delivery is closed by #NEXT.R8.ENTITIES.111.

- [x] **#NEXT.R8.ENTITIES.102** Port representable `Player::ApplyEnchantment` guard/duration shape.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `ApplyEnchantment` now has a C++-ordered plan for missing item, non-equipped item, empty enchantment slot, missing enchantment template, condition failure, player level, required skill, prismatic socket requirements, gem skill requirements, broken-item effect suppression, permanent-enchant visible-item update, and `apply_dur` duration add/remove behavior via the canonical enchant-duration list. Actual enchantment effect execution for damage/stat/rating/spell/totem effects remains pending in follow-up subtasks.

- [x] **#NEXT.R8.ENTITIES.103** Port representable `ApplyEnchantment` effect classification.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: the non-broken enchantment effect loop now has C++-ordered action planning for `None`, deferred combat/use spells, damage/totem `GetAttackBySlot` resolution, equip-spell cast/remove-aura actions, resistance/stat modifier placeholders with original amount/arg, explicit no-op enchantment types and unknown display-type reporting. Detailed stat/rating expansion and actual spell/stat runtime execution remain pending.

- [x] **#NEXT.R8.ENTITIES.104** Expand `ApplyEnchantment` stat/rating effect actions.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.h`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Unit/Unit.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `ITEM_ENCHANTMENT_TYPE_STAT` now expands the C++ `ITEM_MOD_*` switch into concrete action lists for mana/health base modifiers, primary stats plus `UpdateStatBuffMod`, individual and combined combat ratings, attack power/ranged attack power, mana regen, armor penetration, spell power, health regen, spell penetration and shield block value. Runtime application to active player fields/unit mods remains pending.

- [x] **#NEXT.R8.ENTITIES.105** Resolve `ApplyEnchantment` random suffix stat/resistance amounts.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Structure.h`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `ITEM_ENCHANTMENT_TYPE_RESISTANCE` and `ITEM_ENCHANTMENT_TYPE_STAT` now follow the C++ zero-amount path by matching `abs(RandomPropertiesID)` to an explicit `ItemRandomSuffix` ref, scanning suffix enchantment slots in order, and calculating `AllocationPct[k] * PropertySeed / 10000` before producing effect actions. Real DB2 store lookup wiring remains pending with the broader DB2 resolver work.

- [x] **#NEXT.R8.ENTITIES.106** Port representable `Player::UpdateSkillEnchantments`.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `UpdateSkillEnchantments` is represented as a C++-ordered plan over top-level `m_items[0..INVENTORY_SLOT_BAG_END)`, each enchantment slot, normal enchantment template lookup with early abort on missing template, required-skill threshold apply/remove transitions, and prismatic socket requirement rechecks for colorless socket gems. Real `ApplyEnchantment` invocation and DB2/socket-color wiring remain pending.

- [x] **#NEXT.R8.ENTITIES.107** Port representable `Player::SendNewItem` packet plan.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `SendNewItem` is represented as a packet plan that preserves the null-item no-op, player/item GUIDs, bag slot and slot-in-bag quantity rule, quest log item id, quantity and quantity-in-inventory, battle pet modifier extraction, pushed/created flags, encounter loot display/dungeon fields, and direct-vs-group broadcast selection with the party loot-log suppression flag. Session/group delivery bridge is closed by #NEXT.R8.ENTITIES.110; concrete call-site replacement remains pending with canonical entity inventory operations.

- [x] **#NEXT.R8.ENTITIES.108** Port `ItemInstance` and `ItemPushResult` packet serialization.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPacketsCommon.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPacketsCommon.h`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.h`.
  Rust targets: `crates/wow-packet/src/packets/item.rs`.
  Acceptance: `ItemInstance`, `ItemModList`, `ItemMod`, `ItemBonuses` and `ItemPushResult` serialize in C++ field order, using packed GUIDs, the same item-bonus presence bit, 6-bit modification count, item bonus payload order, and `ItemPushResult` pushed/created/display/bonus-roll/encounter-loot bit layout. Conversion from `wow-entities` send-new-item plans to session/group dispatch is closed by #NEXT.R8.ENTITIES.110.

- [x] **#NEXT.R8.ENTITIES.109** Close `SendNewItem` item-instance plan gap.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPacketsCommon.cpp`.
  Rust targets: `crates/wow-entities/src/player.rs`, `crates/wow-entities/src/lib.rs`.
  Acceptance: `SendNewItemPlan` now includes the item instance fields C++ gets from `packet.Item.Initialize(item)`: item id, random property seed, random properties id and the non-zero item modifier values with their modifier type ids. This keeps packet conversion lossless for the item-instance data currently represented by Rust entities and feeds the session/group delivery bridge closed by #NEXT.R8.ENTITIES.110.

- [x] **#NEXT.R8.ENTITIES.110** Wire `SendNewItemPlan` conversion and delivery.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.cpp`.
  Rust targets: `crates/wow-world/src/session.rs`, `crates/wow-world/Cargo.toml`.
  Acceptance: `wow-world` can convert a complete `wow-entities::SendNewItemPlan` into the serialized `ItemPushResult` packet, preserving C++ `SendNewItem` fields, item-instance fields and encounter/direct display behavior. Direct delivery sends to the current session; group delivery broadcasts to all registered group members, including the current player, with direct fallback if group state is unavailable. Call-site replacement in concrete loot/quest/vendor flows remains pending where those flows adopt canonical entity inventory operations.

- [x] **#NEXT.R8.ENTITIES.111** Wire item duration update packet delivery.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Entities/Item/Item.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Handlers/ItemHandler.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.h`.
  Rust targets: `crates/wow-packet/src/packets/item.rs`, `crates/wow-world/src/session.rs`.
  Acceptance: `ItemTimeUpdate` and `ItemEnchantTimeUpdate` serialize in C++ field order with packed item/owner GUIDs, and `WorldSession` can deliver `PlayerItemTimeUpdate` plus `PlayerEnchantTimeUpdate` plans while preserving C++ list order through slice helpers.

- [x] **#NEXT.R8.ENTITIES.112** Wire `SendEquipError` session helper.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Entities/Player/Player.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/ItemPackets.h`.
  Rust targets: `crates/wow-packet/src/packets/item.rs`, `crates/wow-world/src/session.rs`.
  Acceptance: `WorldSession::send_equip_error` mirrors C++ `Player::SendEquipError` packet construction for result, optional item GUIDs, level-gated failures and item-limit-category failures. `InventoryChangeFailure` also covers the C++ bind-confirm context fields for callers that need that packet branch.

## Follow-Up Work Items

- [ ] **#NEXT.R8.ENTITIES.003** Bind `wow-map` grid unload actions to real entity methods once Creature/GameObject/Corpse exist.
- [ ] **#NEXT.R8.ENTITIES.005** Port `WorldObject` LOS, terrain-height, transport relocation and visibility-range helpers that require Map/Terrain/Transport integration.
- [ ] **#NEXT.R8.ENTITIES.006** Complete remaining `ObjectAccessor` integration: real `SaveAllPlayers` and wiring to canonical `wow_map::Map` containers instead of bridge storage.
- [ ] **#NEXT.R8.ENTITIES.008** Complete generated update-field sections beyond `ObjectData`: `UnitData`, `PlayerData`, `ActivePlayerData`, `GameObjectData`, `ItemData`, `CorpseData`, `DynamicObjectData`, `AreaTriggerData`, `SceneObjectData`, `ConversationData`, including visibility flag filters and dynamic/optional fields.
- [ ] **#NEXT.R8.ENTITIES.010** Complete `Unit` subsystems beyond base fields: aura hooks, threat/combat manager, SpellHistory, MotionMaster/move spline, charm/minion ownership, vehicle hooks, AI references and runtime power-index implementations for Player/Creature/Pet.
- [ ] **#NEXT.R8.ENTITIES.012** Complete `Player` create/load/login lifecycle: `Player::Create`, `LoadFromDB`, login packet sequencing, world insertion, visibility bootstrap, stats initialization and DB2-backed `GetPowerIndexByClass`.
- [ ] **#NEXT.R8.ENTITIES.013** Complete `Player` inventory/equipment bridge: build on base `Item`/#NEXT.R8.ENTITIES.041, `Bag`/#NEXT.R8.ENTITIES.042, Player storage lookup/#NEXT.R8.ENTITIES.043, `ObjectAccessor` item branch/#NEXT.R8.ENTITIES.044, visible item state/#NEXT.R8.ENTITIES.045, item visible modifier helpers/#NEXT.R8.ENTITIES.047, `VisualizeItem` mutation side effects/#NEXT.R8.ENTITIES.048, empty top-level `_StoreItem`/#NEXT.R8.ENTITIES.049, empty bag-contained `_StoreItem`/#NEXT.R8.ENTITIES.050, top-level existing-stack merge/#NEXT.R8.ENTITIES.051, bag-contained existing-stack merge/#NEXT.R8.ENTITIES.052, clone storage/#NEXT.R8.ENTITIES.053, split count allocation/#NEXT.R8.ENTITIES.054, split guards/#NEXT.R8.ENTITIES.055, position classifiers/#NEXT.R8.ENTITIES.056, valid-position checks/#NEXT.R8.ENTITIES.057, mergeability guard/#NEXT.R8.ENTITIES.058, bag family fit/#NEXT.R8.ENTITIES.059, specific-slot storage validation/#NEXT.R8.ENTITIES.060, inventory-slot scanning/#NEXT.R8.ENTITIES.061, bag scanning/#NEXT.R8.ENTITIES.062, representable `CanStoreItem` orchestration/#NEXT.R8.ENTITIES.063, similar-item max-count validation/#NEXT.R8.ENTITIES.064, item counting/#NEXT.R8.ENTITIES.065, bound-owner validation/#NEXT.R8.ENTITIES.066, bank storage validation/#NEXT.R8.ENTITIES.067, equip slot selection/#NEXT.R8.ENTITIES.068, combat equip-state helper/#NEXT.R8.ENTITIES.069, representable equip validation/#NEXT.R8.ENTITIES.070, unequip validation/#NEXT.R8.ENTITIES.071, template use validation/#NEXT.R8.ENTITIES.072, item use validation/#NEXT.R8.ENTITIES.073, unique-equip template validation/#NEXT.R8.ENTITIES.074, unique-equip object wrapper/#NEXT.R8.ENTITIES.075, equip-item storage side effects/#NEXT.R8.ENTITIES.076, quick-equip storage side effects/#NEXT.R8.ENTITIES.077, remove-item unlink/#NEXT.R8.ENTITIES.078, move-from-inventory unlink/#NEXT.R8.ENTITIES.079, move-to-inventory finalization/#NEXT.R8.ENTITIES.080, destroy-item cleanup/#NEXT.R8.ENTITIES.081, item-pointer destroy-count/#NEXT.R8.ENTITIES.082, entry destroy-count planning/#NEXT.R8.ENTITIES.083, filtered destroy planning/#NEXT.R8.ENTITIES.084, entry lookup/listing/#NEXT.R8.ENTITIES.085, buyback removal object side effects/#NEXT.R8.ENTITIES.086, weapon/titan-grip helpers/#NEXT.R8.ENTITIES.087, swap preflight guards/#NEXT.R8.ENTITIES.088, swap empty-destination move planning/#NEXT.R8.ENTITIES.089, swap merge/fill planning/#NEXT.R8.ENTITIES.090, swap real-swap validation/#NEXT.R8.ENTITIES.091, swap bag-content exchange planning/#NEXT.R8.ENTITIES.092, swap final execution planning/#NEXT.R8.ENTITIES.093, swap branch orchestration/#NEXT.R8.ENTITIES.094, buyback insertion object side effects/#NEXT.R8.ENTITIES.095, equip-error packet shape/#NEXT.R8.ENTITIES.096, buy/sell error packet shape/#NEXT.R8.ENTITIES.097, soulbound tradeable item set/#NEXT.R8.ENTITIES.098, item/enchantment duration lists/#NEXT.R8.ENTITIES.099, arena enchantment cleanup/#NEXT.R8.ENTITIES.100, duration resend helpers/#NEXT.R8.ENTITIES.101, ApplyEnchantment guard/duration shape/#NEXT.R8.ENTITIES.102, ApplyEnchantment effect classification/#NEXT.R8.ENTITIES.103, ApplyEnchantment stat/rating expansion/#NEXT.R8.ENTITIES.104, ApplyEnchantment random suffix amount resolution/#NEXT.R8.ENTITIES.105, UpdateSkillEnchantments planning/#NEXT.R8.ENTITIES.106, SendNewItem packet planning/#NEXT.R8.ENTITIES.107, ItemPushResult serialization/#NEXT.R8.ENTITIES.108, SendNewItem item-instance plan gap/#NEXT.R8.ENTITIES.109, SendNewItem plan delivery bridge/#NEXT.R8.ENTITIES.110, duration update packet delivery/#NEXT.R8.ENTITIES.111 and SendEquipError session helper/#NEXT.R8.ENTITIES.112 to port canonical item object registry, full store/bank/equip destination validation, DB2 resolver stores, collection appearance registration, equip stat/aura/spell side effects and save/load persistence.
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
