# Migration: Loot

> **C++ canonical path:** `src/server/game/Loot/` (+ `src/server/game/Handlers/LootHandler.cpp`)
> **Rust target crate(s):** `crates/wow-loot/` (currently EMPTY — `lib.rs` is 0 bytes; cargo manifest exists but module is unbuilt), `crates/wow-packet/src/packets/loot.rs` (wire), `crates/wow-world/src/handlers/loot.rs` (session handlers)
> **Layer:** L6 (Game systems — depends on Entities/Creature/GameObject L4, Group L6, Conditions L7, Items L4, Player inventory L4)
> **Status:** 🔧 broken (rewrite needed) — only basic FFA copper drop hard-coded by creature level. No loot template tables loaded, no LootStore, no group loot rolls, no master loot, no loot threshold, no quest items, no fishing/skinning/pickpocketing/disenchant/milling/prospecting/mail/spell/gameobject/reference loot, no loot conditions, no `loot_template` reference resolution, no random suffix/property rolls.
> **Audited vs C++:** ✅ audited 2026-05-01 (§13)
> **Last updated:** 2026-05-01

---

## 1. Purpose

The Loot module turns dead creatures, opened gameobjects, fishing nodes, gathering nodes, pickpocket targets, disenchanted items, milled herbs, prospected ores, mail attachments, and spell-spawned items into player-visible item pools. It owns the random-roll engine (`LootStoreItem::Roll`) that consumes drop chances, item references (chained pools), and conditions (`ConditionMgr`); the per-corpse `Loot` lifecycle (open → display → group roll → assign → release); the wire protocol that drives the loot UI; and the persistence of unlooted gameobject/item containers (`LootItemStorage`). Without it, mobs drop nothing and the game has no economy.

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `game/Loot/Loot.cpp` | 1083 | `prefix` |
| `game/Loot/Loot.h` | 378 | `prefix` |
| `game/Loot/LootItemStorage.cpp` | 362 | `prefix` |
| `game/Loot/LootItemStorage.h` | 95 | `prefix` |
| `game/Loot/LootItemType.h` | 29 | `prefix` |
| `game/Loot/LootMgr.cpp` | 1350 | `prefix` |
| `game/Loot/LootMgr.h` | 178 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `src/server/game/Loot/Loot.h` | 378 | `Loot` struct, `LootItem`, `LootItemList`, `LootRoll` (group roll engine), `AELootResult` (auto-loot accumulator), enums (`LootMethod`, `LootType`, `RollType`, `RollVote`, `RollMask`, `LootError`, `LootSlotType`, `LootRollIneligibilityReason`); constants `LOOT_ROLL_TIMEOUT = 1min`, `MAX_NR_LOOT_ITEMS = 18` |
| `src/server/game/Loot/Loot.cpp` | 1083 | `Loot::FillLoot`, `Loot::AddItem`, `Loot::generateMoneyLoot`, `Loot::AutoStore`, `Loot::BuildLootResponse`, `LootItem::AllowedForPlayer`, `LootItem::GetUiTypeForPlayer`, `LootRoll::TryToStart/PlayerVote/UpdateRoll/Finish`, group-vs-personal loot decision tree |
| `src/server/game/Loot/LootMgr.h` | 178 | `LootStoreItem`, `LootStore`, `LootTemplate` (with private `LootGroup`); 12 global `LootStore` instances (Creature, Disenchant, Fishing, Gameobject, Item, Mail, Milling, Pickpocketing, Prospecting, Reference, Skinning, Spell); `GenerateDungeonEncounterPersonalLoot` |
| `src/server/game/Loot/LootMgr.cpp` | 1350 | `LootStore::LoadAndCollectLootIds` (master loader), per-store `LoadLootTemplates_*` functions, `LootTemplate::Process` (random draw), `LootGroup::HasQuestDropForPlayer`, `LootGroup::Roll`, ref-template resolution, `Verify`/`CheckLootRefs`, hot-fix re-load entry points |
| `src/server/game/Loot/LootItemStorage.h` | 95 | `LootItemStorage` singleton — persists in-progress (uncommitted) lootable item containers across server restarts (the bag/quiver/etc. that has live loot rolling) |
| `src/server/game/Loot/LootItemStorage.cpp` | 362 | DB load/save of `item_loot_storage` per-character; `AddNewStoredLoot`, `RemoveStoredLootForContainer`, `RemoveStoredLootItemForContainer`, `LoadStoredLoot` |
| `src/server/game/Loot/LootItemType.h` | 29 | `enum class LootItemType` — `Item`, `Currency` (used in `LootItem.context`) |
| `src/server/game/Handlers/LootHandler.cpp` | 508 | `WorldSession::HandleAutostoreLootItemOpcode`, `HandleLootMoneyOpcode`, `HandleLootOpcode` (CMSG_LOOT_UNIT), `HandleLootReleaseOpcode`, `HandleLootMasterGiveOpcode`, `HandleLootRoll`, `HandleSetLootSpecialization`, `HandleLootList`, plus reply senders `SendLootError`, `SendLootRelease`, `DoLootRelease` |

Out-of-tree touchpoints:
- `src/server/game/Entities/Creature/Creature.cpp` — `Creature::IsLootRecipient`, `m_loot`, `m_personalLoot`, `HasLootRecipient`, `AllLootRemovedFromCorpse`, `GenerateLootForBody` (calls `Loot::FillLoot` with `LootTemplates_Creature`)
- `src/server/game/Entities/GameObject/GameObject.cpp` — `m_loot`, `Use` for chests/herbs/ore/fishing nodes
- `src/server/game/Entities/Player/Player.cpp` — `StoreLootItem`, `SendLoot`, `SendLootError`, `SendLootRelease`, `SendNotifyLootMoneyRemoved`, `SendNotifyLootItemRemoved`, `m_lootRolls`
- `src/server/game/Groups/Group.cpp` — `GetLootMethod`, `SendLootStartRoll`, `CountRollVote`, `EndRoll`, `Player::OnGameObjectLooted` group permission checks, master-loot list selection
- `src/server/game/Conditions/ConditionMgr.cpp` — `ConditionsReference` resolution per `LootStoreItem`

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Loot` | struct | One per lootable corpse/object: `items` vector, `gold`, `unlootedCount`, `roundRobinPlayer`, `loot_type`, `_lootMethod`, `_lootMaster`, `_allowedLooters`, `_rolls` map, `_dungeonEncounterId`. Owns the per-player FFA item map |
| `LootItem` | struct | Materialized drop: `itemid`, `LootListId`, `randomProperties` (suffix/random property), `context`, `count`, `is_looted`, `is_blocked` (rolling), `freeforall`, `is_underthreshold`, `is_counted`, `needs_quest`, `follow_loot_rules`, `allowedGUIDs`, `rollWinnerGUID`, `conditions`. Built from a `LootStoreItem` at draw time |
| `NotNormalLootItem` | struct | Per-player "FFA" or "personal" item view-stub (just `LootListId` + `is_looted`) for items that aren't shared |
| `LootStoreItem` | struct | One row from a `*_loot_template` table: `itemid`, `reference`, `chance`, `lootmode` (16-bit difficulty mask), `needs_quest`, `groupid`, `mincount`, `maxcount`, `conditions` |
| `LootStore` | class | One per loot table type (12 globals). Wraps `m_LootTemplates: unordered_map<uint32, LootTemplate*>`, name + entryName, ratesAllowed flag |
| `LootTemplate` | class | Per-loot-id collection: `LootStoreItemList Entries` (ungrouped) + `LootGroups Groups` (grouped, mutually exclusive). Methods: `Process`, `ProcessPersonalLoot`, `HasDropForPlayer`, `HasQuestDrop`, `HasQuestDropForPlayer` |
| `LootGroup` (private nested in `LootTemplate`) | class | A "mutually exclusive" set: server picks at most one entry by weighted chance. Used for `groupid > 0` rows |
| `LootMode` | uint16 (mask) | Difficulty mask: `LOOT_MODE_DEFAULT = 0x1`, normal/heroic/mythic/raid-difficulty bits 0x2/0x4/0x8/0x10... up to 16 modes. Drop only if `lootmode & active_mode != 0` |
| `LootMethod` | enum u8 | `FREE_FOR_ALL=0`, `ROUND_ROBIN=1`, `MASTER_LOOT=2`, `GROUP_LOOT=3`, `NEED_BEFORE_GREED=4`, `PERSONAL_LOOT=5` |
| `LootType` | enum u8 | `CORPSE=1`, `PICKPOCKETING=2`, `FISHING=3`, `DISENCHANTING=4`, `ITEM=5`, `SKINNING=6`, `GATHERING_NODE=8`, `CHEST=9`, `CORPSE_PERSONAL=14`, `FISHINGHOLE=20`, `INSIGNIA=21`, `FISHING_JUNK=22`, `PROSPECTING=23`, `MILLING=24`. `GetLootTypeForClient` collapses TC-internal types to client-known ones |
| `LootRoll` | class | Group roll engine for one item: `m_rollVoteMap` (player → `PlayerRollVote{Vote, RollNumber}`), `m_voteMask` (allowed roll types), `m_endTime`. Methods: `TryToStart`, `PlayerVote`, `UpdateRoll` (timeout tick), `Finish`, `AllPlayerVoted`. Lives inside `Loot::_rolls` |
| `RollType` | enum | `PASS=0`, `NEED=1`, `GREED=2`, `DISENCHANT=3`, `TRANSMOG=4` |
| `RollVote` | enum class | Strongly-typed mirror including `NotEmitedYet`, `NotValid` |
| `RollMask` | enum | Allowed-vote bitfield computed from `voteMask = NEED if usable + GREED + DISENCHANT if enchanting skill ≥ required` |
| `LootSlotType` | enum | UI marker: `ALLOW_LOOT=0`, `ROLL_ONGOING=1`, `LOCKED=2`, `MASTER=3`, `OWNER=4` |
| `LootError` | enum u8 | Failure codes for SMSG_LOOT_RESPONSE: `DIDNT_KILL=0`, `TOO_FAR=4`, `BAD_FACING=5`, `LOCKED=6`, `NOTSTANDING=8`, `STUNNED=9`, `MASTER_INV_FULL=12`, `MASTER_UNIQUE_ITEM=13`, `ALREADY_PICKPOCKETED=15`, `NO_LOOT=17` |
| `LootRollIneligibilityReason` | enum class u32 | NEED-roll rejection: `UnusableByClass`, `MaxUniqueItemCount`, `CannotBeDisenchanted`, `EnchantingSkillTooLow`, `NeedDisabled`, `OwnBetterItem` |
| `AELootResult` | class | Accumulator for area-effect (auto) loot — multiple corpses opened at once produce a single ordered result with item dedup |
| `LootItemStorage` | singleton | Persists in-progress containers (mining nodes, herb spots, chest GO, item-loot containers) across restart |
| `LootItemType` | enum class u8 | `Item=0`, `Currency=1` |
| `ItemContext` | enum class u8 | Context flags carried from drop → item creation (used for retail bonus IDs; ignored on 3.4.3 server but preserved on wire) |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `Loot::FillLoot(uint32 lootId, LootStore const& store, Player* lootOwner, bool personal, bool noEmptyError, uint16 lootMode, ItemContext context)` | Generate items by drawing from `LootTemplate->Process`; fill `items[]`; apply `lootMode` mask; auto-add quest items if `needs_quest` matches active quest objective | `LootTemplate::Process`, `LootStore::GetLootFor`, `LootItem` ctor (random suffix/property roll), `Loot::AddItem` |
| `Loot::generateMoneyLoot(uint32 minAmount, uint32 maxAmount)` | Roll uniform copper amount; apply rate config (`Rate.Drop.Money`) | `urand`, `sWorld->getRate` |
| `Loot::AddItem(LootStoreItem const& item)` | Roll mincount..maxcount; assign `LootListId`; copy conditions; respect `MAX_NR_LOOT_ITEMS = 18` cap | `urand`, `Item::GenerateItemRandomPropertyId` |
| `Loot::AutoStore(Player*, uint8 bag, uint8 slot, bool broadcast, bool createdByPlayer)` | Auto-loot path: iterate items and try `Player::StoreNewItem`; call `LootItemPickup` per success | `Player::CanStoreNewItem`, `Player::StoreNewItem`, group event |
| `Loot::LootMoney()` | Distribute `gold` across allowed lootees; emits `SMSG_LOOT_MONEY_NOTIFY` to round-robin player & group | `Player::ModifyMoney`, `Group::SendLooter`, money split per `Group::GetMembersCount` |
| `Loot::LootItemInSlot(uint32 lootListId, Player const* player, NotNormalLootItem** ffaItem)` | Resolve "this player picks slot N" — handles FFA item personal slot, master loot owner check, threshold permission | `LootItem::AllowedForPlayer`, `LootItem::HasAllowedLooter` |
| `Loot::BuildLootResponse(WorldPackets::Loot::LootResponse&, Player const*)` | Emit per-player view of the loot window (different items visible to different players for FFA / quest items / master loot) | `LootItem::GetUiTypeForPlayer`, group permission |
| `Loot::Update()` | Tick — drives `LootRoll::UpdateRoll` for each pending roll | iterates `_rolls` |
| `LootItem::AllowedForPlayer(Player const*, Loot const*)` | Multi-rule check: class restriction (`item_template.AllowableClass`), race, quest requirement, condition fulfilment, faction; static overload reused for "would-have-dropped" preview | `Player::CanRollNeedOnItem`, `ConditionMgr::IsObjectMeetingNotGroupedConditions`, `Player::HasQuest` |
| `LootItem::GetUiTypeForPlayer(Player const*, Loot const&) -> Optional<LootSlotType>` | Decide if the slot shows as `ALLOW_LOOT` / `ROLL_ONGOING` / `MASTER` / `LOCKED` / `OWNER` (or absent) for a given viewer | `Loot::GetLootMethod`, master GUID, `is_blocked`, `is_counted` |
| `LootStoreItem::Roll(bool rate)` | Random draw against `chance` (with global rate multiplier) | `urand`, config rate |
| `LootStore::LoadAndCollectLootIds(LootIdSet& ids_set)` | Master query of `<table_name>` rows → `LootStoreItem` → grouped/ungrouped insertion into `LootTemplate` | `WorldDatabase::Query`, `LootStoreItem::IsValid`, `LootTemplate::AddEntry` |
| `LootStore::CheckLootRefs(LootIdSet*)` | Walk every template and recursively verify `reference > 0` rows resolve to existing `LootTemplates_Reference` ids | `LootTemplate::CheckLootRefs` |
| `LootTemplate::Process(Loot& loot, bool rate, uint16 lootMode, uint8 groupId, Player const* personalLooter)` | Walk ungrouped + grouped entries; for refs recursively `Process` the referenced template | `LootStoreItem::Roll`, `LootGroup::Process`, recursion |
| `LootTemplate::ProcessPersonalLoot(...)` | Personal-loot variant: produce one `Loot*` per player whose drop list passed | per-player iteration |
| `LootTemplate::HasQuestDropForPlayer(...)` / `HasQuestDrop(...)` | True if any entry has `needs_quest` matching player's active quest (or any active quest with a matching ITEM objective) | `Quest::IsRequiredItemForObjective` |
| `LootTemplate::HasDropForPlayer(...)` | Used to suppress "this corpse has no loot" message when conditional loot exists | `LootStoreItem` walk |
| `LootTemplate::CopyConditions(LootItem*)` | Replicate conditions on a freshly-rolled item | — |
| `LootRoll::TryToStart(Map*, Loot&, uint32 lootListId, uint16 enchantingSkill)` | Decide if an item must be rolled (above-threshold + group loot method); compute `voteMask` (NEED based on usability; DISENCHANT based on skill); broadcast `SMSG_LOOT_START_ROLL` | `Group::isLootMethod*`, `LootItem::AllowedForPlayer`, `ItemDisenchantLootEntry` lookup |
| `LootRoll::PlayerVote(Player*, RollVote)` | Record vote; if all eligible voted → `Finish` | timestamp tick |
| `LootRoll::UpdateRoll()` | On `m_endTime` reached, treat unvoted as PASS, call `Finish` | timeout tick |
| `LootRoll::Finish(...)` | Determine winner (highest roll within winning vote category Need > Greed > Disenchant); emit `SMSG_LOOT_ROLL_WON`, mark `LootItem::is_looted` if winner is set, deliver item or auto-disenchant | `Player::AutoStoreLoot` |
| `Group::GroupLoot(uint64 playerGUID, Loot* loot, WorldObject* lootedObject)` | For each `is_blocked` item above threshold call `LootRoll::TryToStart` | calls roll engine |
| `Group::MasterLoot(...)` | For master-loot mode, emit `SMSG_MASTER_LOOT_CANDIDATE_LIST` and wait for `CMSG_LOOT_MASTER_GIVE` | candidate list, `_lootMaster` permission |
| `Group::NeedBeforeGreed(...)` | Like GroupLoot but only allow NEED if class can use the item | `Player::CanRollNeedOnItem` |
| `WorldSession::HandleLootOpcode(CMSG_LOOT_UNIT)` | Validate target is alive ⊕ recipient permission, generate `Loot` lazily, emit `SMSG_LOOT_RESPONSE` | `Creature::IsLootRecipient`, `Loot::FillLoot`, `Loot::BuildLootResponse` |
| `WorldSession::HandleAutostoreLootItemOpcode(CMSG_LOOT_ITEM)` | Per-slot pickup: validate via `Loot::LootItemInSlot`, call `Player::StoreLootItem` | `Player::StoreLootItem`, `Loot::NotifyItemRemoved` |
| `WorldSession::HandleLootMoneyOpcode(CMSG_LOOT_MONEY)` | Trigger `Loot::LootMoney` | money distribution |
| `WorldSession::HandleLootMasterGiveOpcode(CMSG_LOOT_MASTER_GIVE)` | Master gives item to specific GUID; permission + uniqueness checks | master loot path |
| `WorldSession::HandleLootRoll(CMSG_LOOT_ROLL)` | Vote on a pending roll | `LootRoll::PlayerVote` |
| `WorldSession::HandleLootReleaseOpcode(CMSG_LOOT_RELEASE)` | Close window; if fully looted call `Creature::AllLootRemovedFromCorpse` to start corpse decay | `Loot::isLooted`, creature flag clear |
| `WorldSession::HandleSetLootMethod(CMSG_SET_LOOT_METHOD)` | Group leader changes loot method/threshold/master | `Group::SetLootMethod` |
| `LootItemStorage::AddNewStoredLoot(Loot*, Player*)` | Persist live container loot | DB write |
| `LootItemStorage::LoadStoredLoot(Item*, Player*)` | Restore on item open after restart | DB read |

---

## 5. Module dependencies

**Depends on:**
- **Entities/Creature** — `Creature::m_loot`, `Creature::IsLootRecipient`, `m_lootRecipient`, `m_lootRecipientGroup`, kill-tap logic, `AllLootRemovedFromCorpse` (sets corpse decay timer)
- **Entities/GameObject** — chests, mining/herb nodes, fishing bobbers, quest containers; `GameObject::m_loot`, `GameObject::Use` triggers `Loot::FillLoot`
- **Entities/Player** — inventory store/take APIs (`StoreNewItem`, `StoreLootItem`, `CanStoreNewItem`), `m_lootRolls` per-player roll tracking, money modifications, `OnLootedObject` quest hook, `SendLootError`, `SendNotifyLootItemRemoved`
- **Entities/Item** — random property/suffix generation (`Item::GenerateItemRandomPropertyId`), bag containers, item template (for `AllowableClass`/`AllowableRace`/`Bonding`)
- **Conditions** — `ConditionsReference` per `LootStoreItem`; `CONDITION_SOURCE_TYPE_LOOT_TEMPLATE`, `CONDITION_SOURCE_TYPE_REFERENCE_LOOT_TEMPLATE`. Recursive resolution mid-roll
- **Group** — `GetLootMethod`, `GetLootThreshold`, `GetMasterLooterGuid`, `Group::GroupLoot`/`MasterLoot`/`NeedBeforeGreed`/`SendLooter`/`CountRollVote`/`EndRoll`. Roll permissions: round-robin owner, master GUID
- **Quests** — `Quest::IsRequiredItemForObjective` so `needs_quest` items only drop for players with matching active objective
- **DBC/DB2** — `ItemDisenchantLoot.dbc` (for disenchant rolls), `ItemRandomProperties.dbc`, `ItemRandomSuffix.dbc`, `MapDifficulty.dbc` (lootMode mapping), `SpellEffect.dbc` (skinning skill)
- **WorldPackets/Loot** — every loot opcode struct
- **ScriptMgr** — `OnLootCreatureLoot`, `OnLootItem`, `OnLootMoney` hooks per script
- **DB rates** (`Rate.Drop.Item.Poor/Normal/Uncommon/Rare/Epic/Legendary/Artifact/Referenced`, `Rate.Drop.Money`)

**Depended on by:**
- **Quests** — quest item drops route through `LootTemplate::HasQuestDropForPlayer`; `ItemAddedQuestCheck` chained from `StoreLootItem`
- **Achievements** — `CRITERIA_TYPE_LOOT_ITEM`, `CRITERIA_TYPE_LOOT_TYPE`, `CRITERIA_TYPE_LOOT_MONEY`
- **AI/Scripts** — many boss scripts call `Loot::AddItem` post-fact for guaranteed drops
- **Mail** — `mail_loot_template` drives mail attachments
- **Battlegrounds** — insignia loot type
- **AHBot** — generates fake auction listings from `LootTemplates_Creature`
- **Calendar/Events** — seasonal loot (Hallow's End candy, etc.) gated by `ConditionMgr` event conditions

---

## 6. SQL / DB queries

### World DB — loot template tables (one per `LootStore` instance)

| Statement / Source | Purpose | DB |
|---|---|---|
| `SELECT entry, item, Reference, Chance, QuestRequired, LootMode, GroupId, MinCount, MaxCount FROM creature_loot_template` | Drops per creature entry | world |
| `SELECT … FROM gameobject_loot_template` | Drops per GO entry (chests, herb/ore nodes) | world |
| `SELECT … FROM reference_loot_template` | Shared sub-tables referenced via `Reference > 0` | world |
| `SELECT … FROM disenchant_loot_template` | Per `DisenchantID` (column on `Item.dbc`) | world |
| `SELECT … FROM fishing_loot_template` | Keyed by area ID (or fishing-hole entry) | world |
| `SELECT … FROM pickpocketing_loot_template` | Keyed by `creature_template.pickpocketloot` | world |
| `SELECT … FROM skinning_loot_template` | Keyed by `creature_template.skinloot` (skinning, gathering, salvage) | world |
| `SELECT … FROM mail_loot_template` | Mail attachments by mail-template id | world |
| `SELECT … FROM milling_loot_template` | Inscription milling, keyed by herb item id | world |
| `SELECT … FROM prospecting_loot_template` | Jewelcrafting prospecting, keyed by ore item id | world |
| `SELECT … FROM item_loot_template` | Bag/lockbox contents, keyed by item id | world |
| `SELECT … FROM spell_loot_template` | Items created by spell effects (random-item spells) | world |
| `SELECT * FROM conditions WHERE SourceTypeOrReferenceId IN (1, 5, 7, 18, 21, 22, 23, 24, 25)` (loot-source types) | Per-loot conditions joined into `LootStoreItem.conditions` | world |

### Character DB — runtime persistence

| Statement (TC enum) | Purpose | DB |
|---|---|---|
| `CHAR_SEL_ITEM_LOOT_ITEMS` | `SELECT containerGUID, itemId, count, itemIndex, followLootRules, freeForAll, isBlocked, isCounted, isUnderThreshold, needsQuest, randomBonusListId, context, bonusListIDs FROM item_loot_items` | characters |
| `CHAR_SEL_ITEM_LOOT_MONEY` | `SELECT containerGUID, money FROM item_loot_money` | characters |
| `CHAR_INS_ITEM_LOOT_ITEMS` | Persist a freshly-dropped container item | characters |
| `CHAR_INS_ITEM_LOOT_MONEY` | Persist money in a container | characters |
| `CHAR_DEL_ITEM_LOOT_ITEMS` (per containerGUID + itemIndex) | Remove looted item slot | characters |
| `CHAR_DEL_ITEM_LOOT_MONEY` (per containerGUID) | Remove money | characters |
| `CHAR_DEL_ITEM_LOOT_ALL_ITEMS` (per containerGUID) | Drop container | characters |
| `CHAR_DEL_NONEXISTENT_ITEM_LOOT` | Cleanup orphaned rows | characters |
| `CHAR_INS_BG_LOOT_LIST` (when applicable for BG insignia) | optional | characters |

### DBC / DB2 stores

| Store | What it loads | Read by |
|---|---|---|
| `ItemDisenchantLootStore` | `ItemDisenchantLoot.dbc` (subclass + quality → DisenchantID) | `LootRoll::GetItemDisenchantLoot`, disenchant-type rolls |
| `ItemRandomPropertiesStore` | `ItemRandomProperties.dbc` | `LootItem` ctor random property generation |
| `ItemRandomSuffixStore` | `ItemRandomSuffix.dbc` | `LootItem` ctor suffix generation |
| `MapDifficultyStore` | difficulty bit → `lootMode` mask | `Loot::FillLoot` |
| `SpellEffectStore` (skinning skill effect) | skinning required-skill values | `Creature::IsSkinnedBy` |

---

## 7. Wire-protocol packets

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_LOOT_UNIT` | client → server | `WorldSession::HandleLootOpcode` (right-click corpse / GO) |
| `CMSG_LOOT_ITEM` | client → server | `HandleAutostoreLootItemOpcode` (one-by-one or AE batch) |
| `CMSG_LOOT_RELEASE` | client → server | `HandleLootReleaseOpcode` (close window) |
| `CMSG_LOOT_MONEY` | client → server | `HandleLootMoneyOpcode` |
| `CMSG_LOOT_ROLL` | client → server | `HandleLootRoll` (Pass / Need / Greed / Disenchant) |
| `CMSG_LOOT_MASTER_GIVE` | client → server | `HandleLootMasterGiveOpcode` (master assigns to GUID) |
| `CMSG_SET_LOOT_METHOD` | client → server | `HandleSetLootMethod` (party leader changes method/threshold/master) |
| `CMSG_OPT_OUT_OF_LOOT` | client → server | `HandleOptOutOfLoot` (player opts out of group rolls) |
| `CMSG_LOOT_LIST` | client → server | request roll list |
| `CMSG_AE_LOOT_TARGETS` / `CMSG_AE_LOOT_TARGET_ACK` | client → server | Area-effect (auto) loot targeting |
| `SMSG_LOOT_RESPONSE` | server → client | `Loot::BuildLootResponse` (window contents) |
| `SMSG_LOOT_RELEASE` (legacy SMSG_LOOT_RELEASE_RESPONSE) | server → client | `WorldSession::SendLootRelease` |
| `SMSG_LOOT_RELEASE_ALL` | server → client | broadcast on bulk close (AE looting end) |
| `SMSG_LOOT_REMOVED` | server → client | `Loot::NotifyItemRemoved` per slot |
| `SMSG_LOOT_MONEY_NOTIFY` | server → client | per-player share notification |
| `SMSG_LOOT_LIST` | server → client | candidate list for area-loot |
| `SMSG_LOOT_ALL_PASSED` | server → client | every roller passed |
| `SMSG_LOOT_ROLL` | server → client | individual vote broadcast |
| `SMSG_LOOT_ROLL_WON` | server → client | winner announcement |
| `SMSG_LOOT_ROLLS_COMPLETE` | server → client | all rolls done |
| `SMSG_START_LOOT_ROLL` | server → client | a roll begins (per item, per recipient) |
| `SMSG_MASTER_LOOT_CANDIDATE_LIST` | server → client | master loot picker UI |
| `SMSG_SET_LOOT_METHOD_FAILED` | server → client | rejection of CMSG_SET_LOOT_METHOD |
| `SMSG_LEGACY_LOOT_RULES` | server → client | toggle for "old rules" personal loot |
| `SMSG_GUILD_ITEM_LOOTED_NOTIFY` | server → client | guild news on epic+ drop |
| `SMSG_AE_LOOT_TARGETS` / `_ACK` | server → client | AE loot ack |

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-loot` | `crate_dir` | 1 | 0 | `exists_empty` | crate exists; no active Rust source lines |
| `crates/wow-packet/src/packets/loot.rs` | `file` | 1 | 210 | `exists_active` | file exists |
| `crates/wow-world/src/handlers/loot.rs` | `file` | 1 | 247 | `exists_active` | file exists |
| `crates/wow-loot/src/lib.rs` | `file` | 1 | 0 | `exists_empty` | file exists but has 0 lines |
| `crates/wow-core` | `crate_dir` | 4 | 1153 | `exists_active` | crate exists |
| `crates/wow-constants` | `crate_dir` | 10 | 5477 | `exists_active` | crate exists |
| `crates/wow-world` | `crate_dir` | 17 | 12778 | `exists_active` | crate exists |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-loot/src/lib.rs` — **0 bytes / empty** — crate exists in `Cargo.toml` (depends on `wow-core`, `wow-constants`, `rand`) but has no code at all. The whole module is unimplemented.
- `crates/wow-packet/src/packets/loot.rs` — 211 lines — packet structs only: `LootUnit`, `LootItemPkt`, `LootItemRequest`, `LootRelease`, `LootItemData`, `LootResponse`, `LootRemoved`, `SLootRelease`, plus the in-memory `CreatureLoot` / `LootEntry` helpers used by the world handler.
- `crates/wow-world/src/handlers/loot.rs` — 247 lines — registers + implements `CMSG_LOOT_UNIT`, `CMSG_LOOT_ITEM`, `CMSG_LOOT_RELEASE`. Storage is `WorldSession::loot_table: HashMap<ObjectGuid, CreatureLoot>` (per-session, lost on disconnect). Loot generation is the placeholder `generate_creature_loot(guid, level, _entry)` returning random copper based on creature level and zero items.

**What's implemented:**
- `CMSG_LOOT_UNIT` → checks creature is dead, generates loot lazily, sends `SMSG_LOOT_RESPONSE` with copper only
- `CMSG_LOOT_ITEM` → marks slot taken, emits `SMSG_LOOT_REMOVED` (no inventory grant — inventory write is `TODO`)
- `CMSG_LOOT_RELEASE` → emits `SMSG_LOOT_RELEASE`, removes loot from session map, schedules 30s corpse decay if fully looted
- Random copper based on `level * 200 + (guid_counter % (level*300+1))` — uses GUID counter as a fake RNG seed (deterministic per kill)

**What's missing vs C++:**
- **Entire `crates/wow-loot/` is empty.** No `LootStore`, no `LootTemplate`, no `LootStoreItem`, no `LootGroup`, no `LootItem`, no `LootMode`, no `Loot` struct, no global instances for the 12 store types
- No `*_loot_template` table loading (creature/gameobject/reference/disenchant/fishing/pickpocketing/skinning/mail/milling/prospecting/item/spell — all 12 missing)
- No reference resolution (`Reference > 0` ignored)
- No drop chance roll (`LootStoreItem::Roll` missing)
- No grouped vs ungrouped entries (`groupid` ignored)
- No `lootMode` difficulty filtering (every roll would be DEFAULT only)
- No quest-only drops (`needs_quest` ignored)
- No `LootStoreItem.conditions` integration (ConditionMgr unwired)
- No loot rate config (`Rate.Drop.Item.*`, `Rate.Drop.Money`)
- No personal loot (`PERSONAL_LOOT`) — every drop is FFA
- No group loot rolls (`LootRoll`, `SMSG_START_LOOT_ROLL`, `CMSG_LOOT_ROLL`, `SMSG_LOOT_ROLL`, `SMSG_LOOT_ROLL_WON`, timeout)
- No master loot (`MASTER_LOOT`, `SMSG_MASTER_LOOT_CANDIDATE_LIST`, `CMSG_LOOT_MASTER_GIVE`)
- No round-robin owner tracking (`roundRobinPlayer`)
- No loot threshold (`is_underthreshold`); every item would be free pickup
- No `LootMethod::NEED_BEFORE_GREED` rules
- No round-robin order
- No `SMSG_LOOT_MONEY_NOTIFY` per-player money split (current copper goes to one looter only because session-scoped)
- No fishing loot (no fishing-bobber GO interaction, no `FISHINGHOLE` fallback, no junk-fishing)
- No pickpocketing (`LOOT_PICKPOCKETING`, `Creature::CanBePickPocketed`, `m_pickpocketLootTime`)
- No skinning (`LOOT_SKINNING`, skinning skill check, corpse-skinned flag)
- No disenchant rolls (`LOOT_DISENCHANTING`, `ItemDisenchantLoot.dbc`)
- No prospecting (`LOOT_PROSPECTING`) / milling (`LOOT_MILLING`) / mail loot (`LOOT_MAIL`) / item loot (`LOOT_ITEM` containers like lockboxes / bags) / spell loot (random-item spells)
- No gameobject loot (chests, mining/herb nodes, fishing bobbers — all `GameObject::Use` paths)
- No random properties / random suffix on dropped items
- No item context / bonus list IDs persistence (placeholder fields exist on wire, never populated)
- No `AELootResult` for area-effect (auto) loot
- No `LootItemStorage` (in-progress containers vanish on restart)
- No `Group::GroupLoot/MasterLoot/NeedBeforeGreed` integration
- No `Player::CanRollNeedOnItem` (class usability check)
- No `LootRollIneligibilityReason` reporting
- No `OPCODE: SetLootMethod`, `OptOutOfLoot`, `LootList`, `MasterLootItem`, `AeLootTargets/Ack` handlers
- No script hooks (`OnLootCreatureLoot`, `OnLootItem`, `OnLootMoney`)
- No `SMSG_LOOT_LIST`, `SMSG_LOOT_ROLLS_COMPLETE`, `SMSG_LOOT_ALL_PASSED`, `SMSG_LOOT_RELEASE_ALL`
- No achievements integration on loot pickup
- No money split on group loot
- No `LootError` reporting (FFA failure, master InvFull, unique item, etc.)
- **`SMSG_LOOT_REMOVED` is sent but inventory is NOT updated** — items vanish from the loot window without entering player bags. Major bug.

**Suspicious / likely divergent (pre-audit hypothesis):**
- `loot_table` is a **per-session** HashMap on `WorldSession`. In TC, `Loot` belongs to the `Creature`/`GameObject`, so multiple players can see the same loot window. Current design forbids group loot from working at all.
- Coin formula `level * 200 + (seed % (level * 300 + 1))` is arbitrary — TC uses `creature_template.mingold..maxgold` direct rows, then applies `Rate.Drop.Money`.
- `corpse_despawn_at = now + 30s` is hardcoded; TC uses `RateCorpseDecayLooted` × `m_corpseDelay` (default 60s for normal mobs, longer for elites/bosses).
- The `taken: bool` flag is server-only; TC uses `is_looted` + `is_blocked` (separating "taken" from "rolling on it"). When rolls land, `is_blocked` clears and `is_looted` is set on the winner side.
- `LootItemData::ui_type` is hardcoded to 0 — should be `LootSlotType::ALLOW_LOOT` only when no roll active and player has permission.
- `LootResponse::loot_method` is hardcoded to 0 (FreeForAll); should reflect group method.
- `LootResponse::threshold` is hardcoded to 2 (Uncommon); should reflect group's chosen threshold.
- No `acquired = false` path with `failure_reason = LootError::*` — current code sends `failure_reason: 2` on a living creature (which is `LOOT_ERROR_NOTSTANDING`), should be `DIDNT_KILL` or `NO_LOOT`.
- Coins are sent in `SMSG_LOOT_RESPONSE.coins` but never consumed by `CMSG_LOOT_MONEY` — unclear if money modifier is ever called.

**Tests existing:**
- 0 unit tests in `wow-loot` (empty crate)
- 0 integration tests for loot in `wow-world`
- 0 tests against any of the 12 loot-template tables

---

## 9. Migration sub-tasks

Numbered for cross-reference from `MIGRATION_ROADMAP.md` §5. Complexity: **L** (<1h), **M** (1-4h), **H** (4-12h), **XL** (>12h, split).

- [ ] **#LOOT.1** Populate `crates/wow-loot/src/lib.rs` skeleton: re-export `LootStore`, `LootTemplate`, `LootStoreItem`, `LootGroup`, `LootItem`, `Loot` (server-side), `LootRoll`, `LootMethod`, `LootType`, `RollType`, `LootError`, `LootSlotType`, `LootMode` (L)
- [ ] **#LOOT.2** Define `LootStoreItem { itemid, reference, chance, lootmode, needs_quest, groupid, mincount, maxcount, conditions }` and `LootStoreItem::roll(rate: f32) -> bool` (L)
- [ ] **#LOOT.3** Define `LootTemplate { entries: Vec<LootStoreItem>, groups: Vec<LootGroup> }` with `process(loot, rate, loot_mode, group_id, personal_looter)` recursive over reference templates (M)
- [ ] **#LOOT.4** Define `LootGroup` with weighted-without-replacement single-pick semantics; `process_with_total_chance` matches TC `LootGroup::Process` (M)
- [ ] **#LOOT.5** Define `LootStore { name, entry_name, rates_allowed, templates: HashMap<u32, LootTemplate> }` and 12 globals: `Creature`, `Disenchant`, `Fishing`, `Gameobject`, `Item`, `Mail`, `Milling`, `Pickpocketing`, `Prospecting`, `Reference`, `Skinning`, `Spell` (M)
- [ ] **#LOOT.6** SQL loaders: one per `*_loot_template` table → populate corresponding store; `Verify` + `CheckLootRefs` (verify `Reference > 0` resolves) (H — XL if including conditions join)
- [ ] **#LOOT.7** Implement `Loot` struct with `items: Vec<LootItem>`, `gold`, `unlooted_count`, `loot_type`, `loot_method`, `loot_master`, `round_robin_player`, `allowed_looters`, `rolls: HashMap<u32, LootRoll>`. Move ownership from session to `Creature::loot` / `GameObject::loot` (H)
- [ ] **#LOOT.8** Implement `Loot::fill_loot(loot_id, store, owner, personal, no_empty_error, loot_mode, context)` calling `LootTemplate::process` (M)
- [ ] **#LOOT.9** Implement `Loot::generate_money_loot(min, max)` honoring `Rate.Drop.Money` config (L)
- [ ] **#LOOT.10** `LootItem::allowed_for_player`: class/race/quest/condition/faction checks; `needs_quest` → only if player has matching active ITEM objective (M, depends on #QUESTS.5/8/9)
- [ ] **#LOOT.11** `LootItem::get_ui_type_for_player` → `LootSlotType` decision tree (M)
- [ ] **#LOOT.12** Hook `Creature` death → `generate_loot_for_body` populating `m_loot` from `LootTemplates_Creature` (M)
- [ ] **#LOOT.13** Hook `GameObject::use` for chest / herb / ore / fishing-bobber / lockbox; per-type LootStore selection + skill check where applicable (M)
- [ ] **#LOOT.14** Replace per-session `loot_table` HashMap with `Creature::m_loot` / `GameObject::m_loot` so multiple sessions share the same Loot view (H — touches creature/gameobject types)
- [ ] **#LOOT.15** `LootRoll` engine: `try_to_start`, `player_vote`, `update_roll` (1-minute timeout), `finish` (Need > Greed > Disenchant ranking, highest roll wins) (H)
- [ ] **#LOOT.16** Group-loot integration: `Group::GroupLoot`/`MasterLoot`/`NeedBeforeGreed` equivalents; `LootMethod` per group; threshold (`item_template.Quality >= threshold` enters roll) (H)
- [ ] **#LOOT.17** Master loot: `SMSG_MASTER_LOOT_CANDIDATE_LIST`, `CMSG_LOOT_MASTER_GIVE`, permission + uniqueness check (M)
- [ ] **#LOOT.18** Round-robin owner: track per-`Loot`; coin distribution to all eligible group members (M)
- [ ] **#LOOT.19** Fishing loot: `CMSG_USE_ITEM` on fishing bobber → roll skill, pick `LootTemplates_Fishing` by area; junk fallback (M)
- [ ] **#LOOT.20** Skinning: corpse flag after death, skill check, `LootTemplates_Skinning` (M)
- [ ] **#LOOT.21** Pickpocketing: rogue stealth+target check, `LootTemplates_Pickpocketing`, cooldown (M)
- [ ] **#LOOT.22** Disenchant: `ItemDisenchantLoot.dbc` lookup → `LootTemplates_Disenchant`; trigger on `ROLL_DISENCHANT` win; min skill 175 to disenchant grouped roll (M)
- [ ] **#LOOT.23** Prospecting / milling: trade-skill spells → consume reagents → `LootTemplates_Prospecting`/`Milling` (M)
- [ ] **#LOOT.24** Mail loot: `mail_loot_template` resolved at mail send/open (M, depends on Mail module)
- [ ] **#LOOT.25** Item loot containers (lockboxes, bags): `LootItemStorage` persistence in `item_loot_items` + `item_loot_money` (H)
- [ ] **#LOOT.26** Random properties / suffix generation (`ItemRandomProperties.dbc`, `ItemRandomSuffix.dbc`) on `LootItem` creation (M)
- [ ] **#LOOT.27** Loot conditions: hook `ConditionMgr::is_object_meeting_not_grouped_conditions` for each `LootStoreItem` (depends on Conditions module) (M)
- [ ] **#LOOT.28** `CMSG_SET_LOOT_METHOD`, `CMSG_OPT_OUT_OF_LOOT`, `CMSG_LOOT_LIST`, `CMSG_LOOT_MONEY`, `CMSG_AE_LOOT_TARGETS/ACK` handlers (M)
- [ ] **#LOOT.29** **CRITICAL: `CMSG_LOOT_ITEM` must add the item to the player's inventory** (currently TODO). `Player::store_loot_item` analogue (M)
- [ ] **#LOOT.30** Replace ad-hoc copper formula with `creature_template.mingold..maxgold` row, applying `Rate.Drop.Money` (L)
- [ ] **#LOOT.31** `LootError` reporting (`SMSG_LOOT_RESPONSE.failure_reason`) for every refusal path (`DIDNT_KILL`, `TOO_FAR`, `LOCKED`, `STUNNED`, `NO_LOOT`, master-only errors) (L)
- [ ] **#LOOT.32** Achievement criteria: `LOOT_ITEM`, `LOOT_TYPE`, `LOOT_MONEY` (M, depends on Achievements)
- [ ] **#LOOT.33** Script hooks: `on_loot_creature`, `on_loot_item`, `on_loot_money` (L)

---

## 10. Regression tests to write

- [ ] Test: `creature_loot_template` fixture row drops `itemid` at exactly `chance%` (statistical: 10k samples → ±2σ)
- [ ] Test: a `Reference > 0` row resolves through `reference_loot_template` and returns its rolled item, never the literal `itemid=0`
- [ ] Test: `LootGroup` with N entries summing to 100% always picks exactly one
- [ ] Test: `LootGroup` with sum < 100% can produce zero items
- [ ] Test: `lootMode = HEROIC (0x2)` skips entries without `lootmode & 0x2`
- [ ] Test: `needs_quest = 1` row drops only for player with matching active ITEM objective
- [ ] Test: `LootItem::allowed_for_player` rejects warrior on a `AllowableClass` mage-only ref
- [ ] Test: `MAX_NR_LOOT_ITEMS = 18` cap enforced — 19th potential drop ignored
- [ ] Test: `Loot::generate_money_loot` produces value in `[min, max]` × `Rate.Drop.Money`
- [ ] Test: looting an item triggers `Player::store_loot_item` and inventory count increments by `LootItem.count`
- [ ] Test: `CMSG_LOOT_RELEASE` after every item taken sets corpse decay and removes lootable flag
- [ ] Test: group loot with `GROUP_LOOT` method + uncommon threshold → uncommon item enters roll, common item is free pickup
- [ ] Test: `LootRoll` with all 5 members PASSing emits `SMSG_LOOT_ALL_PASSED` and item becomes free
- [ ] Test: `LootRoll` after 60s timeout treats unvoted as PASS; winner determined from voted set
- [ ] Test: NEED outranks GREED outranks DISENCHANT in winner selection regardless of roll number
- [ ] Test: master loot — non-master players see `LootSlotType::MASTER`; master sees `ALLOW_LOOT`
- [ ] Test: master-loot give to player with full inventory returns `LOOT_ERROR_MASTER_INV_FULL`
- [ ] Test: pickpocket on already-picked target returns `ALREADY_PICKPOCKETED`
- [ ] Test: round-robin coin split — gold = 600c, 3 group members → each gets 200c
- [ ] Test: FFA item visible only to its player; non-eligible players see slot absent
- [ ] Test: random suffix items get distinct `random_property_id` per drop
- [ ] Test: `LootItemStorage` round-trip — open container, kill server, restart, container retains items
- [ ] Test: condition gate — quest-condition row drops only when player has active quest

---

## 11. Notes / gotchas

- **`LOOT_ROLL_TIMEOUT = 60s`** is wire-implicit; client expects winner before then or it gets confused. Don't extend.
- **`MAX_NR_LOOT_ITEMS = 18`** — the loot window cannot show more than 18 items + 1 money slot (slot 19 reserved). Excess drops are silently dropped on the server side.
- **FFA vs group loot.** `LootItem::freeforall` rows go in a `NotNormalLootItem` per-player map, not the shared `items[]`. Each player sees their own copy with its own `is_looted` flag. A common bug is treating FFA items as global → one player loots, others can't see it anymore.
- **Loot threshold gate.** Items below `Group::GetLootThreshold` (default Uncommon=2) bypass group rolls and become free-pickup. The threshold is per-group and per-item-quality (`item_template.Quality`). Check `LootItem::is_underthreshold` flag before queuing into `LootRoll`.
- **Master loot permission.** Only the `_lootMaster` GUID can send `CMSG_LOOT_MASTER_GIVE`. Validating this on the server is critical — the client sends the GUID but a malicious client can spoof. Cross-check against `Group::GetMasterLooterGuid`.
- **Item link broadcast.** When a NEED roll is won, TC emits `SMSG_LOOT_ROLL_WON` to the entire group, but the *source* of `SMSG_LOOT_REMOVED` is per-player (only those who could see the slot). For above-threshold items, the slot disappears for everyone. For below-threshold (free-pick), only the looter's view updates.
- **`needs_quest` items consume an objective slot.** When loot is generated for a creature, only items whose `needs_quest=1` AND `LootItem::allowed_for_player` matches an active objective are added. Re-evaluate per-player at view time (multiple party members may need the same item; each gets an independent FFA copy).
- **Reference recursion can loop.** TC has an explicit `LootStoreItem::IsValid` + `LootStore::CheckLootRefs` pass at startup that detects cycles. Reproduce or risk infinite recursion at draw time.
- **`lootMode` is a bitmask.** Difficulty selection (10/25 normal vs heroic) maps to 0x1, 0x2, 0x4, 0x8 etc. via `MapDifficulty.dbc`. Multiple bits can be set simultaneously (e.g. RDF dungeons use a special mode).
- **Drop rate config.** `Rate.Drop.Item.Poor`, `Normal`, `Uncommon`, `Rare`, `Epic`, `Legendary`, `Artifact`, `Heirloom`, `Referenced` — multiplies per-quality. `Rate.Drop.Money` multiplies coin amount. Don't hardcode rates.
- **Quest-required gameobjects.** Some quest items ONLY drop from gameobjects with `gameobject_questrelation`. Treat as `gameobject_loot_template` + condition, not creature.
- **Personal loot ≠ FFA.** `PERSONAL_LOOT` (raid finder) generates one independent `Loot*` per player at draw time; `FREE_FOR_ALL` shares one `Loot` but partitions visibility per `freeforall` flag. The two paths must not be conflated.
- **`SMSG_LOOT_RESPONSE` differs per viewer.** Build it via `BuildLootResponse(packet, viewer)`, not once and broadcast. Different players see different items based on FFA, master, threshold, conditions.
- **Skinning is a state transition** on the corpse (`CREATURE_FLAG_LOOTABLE` → `CREATURE_FLAG_SKINNABLE`); it can only happen *after* the looter releases the corpse. Track separate "loot-released" timestamps to know when skinning is unlocked.
- **Pickpocketing** spawns a *separate* `Loot` instance (`LOOT_PICKPOCKETING`) that does not consume the corpse drops; the same creature can be killed and have its real loot rolled. Each pickpocket cooldown is per-creature, not per-player.
- **Loot in containers persists across server restart** via `LootItemStorage`. If you forget the persistence, players who close their bag with rolling items mid-progress lose them at restart.
- **`TC LootItem(LootStoreItem)` ctor generates the random property/suffix** — this happens once at draw, not at view. Multiple viewers see the same suffix.
- **`AELootResult`** — modern auto-loot batches multiple corpses into one window; deduping by item GUID matters because the same dungeon-encounter ID may attribute the same drop to multiple kills.
- **`is_counted`** — internal flag preventing double-decrement of `unlootedCount` when an item is removed via roll loss vs direct pickup.
- **`is_blocked`** — set while a roll is pending; once roll concludes it clears regardless of winner. Don't auto-loot blocked items.
- **`creature_template.skinloot = 0`** still allows skinning if `creature_template.type_flags & CREATURE_TYPEFLAGS_SKINNABLE` is set (skinning skill produces leather scraps from a default fallback).
- **Conditions on loot are LIVE** — condition fulfilment is checked at `Loot::FillLoot` time AND at view time. A player whose quest expires mid-loot-window sees the item disappear.
- **`LootMethod::PERSONAL_LOOT`** in 3.4.3 is mostly LFR/LFD only; classic dungeons use Group Loot or Free For All by default.
- **Coin display rounding.** Wire format is raw copper; the client formats gold/silver/copper itself. Don't pre-divide.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `struct Loot` | `struct Loot` (`crates/wow-loot/src/loot.rs`) | One per Creature/GameObject, NOT per session |
| `struct LootItem` | `struct LootItem` | Same field shape; bitfields → individual `bool` |
| `struct LootStoreItem` | `struct LootStoreItem` | Already row-shaped, easy port |
| `class LootStore` (12 globals) | `struct LootStore` × 12 in `OnceLock<RwLock<LootStore>>` | Avoid raw mutable globals |
| `class LootTemplate` (with private nested `LootGroup`) | `struct LootTemplate { entries: Vec<LootStoreItem>, groups: Vec<LootGroup> }` | Drop nested-class; `LootGroup` is a sibling type |
| `class LootRoll` | `struct LootRoll { vote_map: HashMap<ObjectGuid, PlayerRollVote>, end_time: Instant, … }` | Drive from `Loot::update(diff_ms)` tick |
| `enum LootMethod` (uint8) | `#[repr(u8)] enum LootMethod { Ffa=0, RoundRobin=1, Master=2, Group=3, NeedBeforeGreed=4, Personal=5 }` | — |
| `enum LootType` (uint8) | `#[repr(u8)] enum LootType { Corpse=1, … }` | Skip 0; preserve gaps (7, 10–13, 15–19) |
| `enum RollType` / `enum class RollVote` | unify into `enum RollVote { Pass, Need, Greed, Disenchant, Transmog }` plus `RollState` enum for not-yet-voted | — |
| `EnumFlag<RollMask>` | `bitflags! struct RollMask: u8` | — |
| `Loot::FillLoot(...)` | `fn fill_loot(&mut self, loot_id: u32, store: &LootStore, owner: &Player, personal: bool, no_empty_error: bool, loot_mode: u16, context: ItemContext) -> bool` | Port one-to-one |
| `LootTemplate::Process(...)` recursive | `fn process(&self, loot: &mut Loot, rate: bool, loot_mode: u16, group_id: u8, personal_looter: Option<&Player>, store: &LootStore)` | Pass `store` to resolve refs |
| `LootStoreItem::Roll(bool rate)` | `fn roll(&self, rate: f32) -> bool` | Use `rand::thread_rng()` |
| `std::unordered_map<ObjectGuid, std::unique_ptr<NotNormalLootItemList>>` | `HashMap<ObjectGuid, Vec<NotNormalLootItem>>` | No need for unique_ptr in Rust |
| `Creature::m_loot: std::unique_ptr<Loot>` | `creature.loot: Option<Loot>` | Lazy creation; `take()` on release |
| `LootItemStorage` singleton | `static LOOT_ITEM_STORAGE: OnceLock<RwLock<LootItemStorage>>` | Same pattern as quest pools |
| `WorldPackets::Loot::LootResponse` | `LootResponse` (already in `wow-packet/src/packets/loot.rs`) | Build per-viewer in handler |
| `ConditionsReference` per `LootStoreItem` | `Arc<Vec<Condition>>` or `ConditionRef` (depends on Conditions module shape) | Tied to #LOOT.27 |
| `urand(min, max)` | `rand::Rng::gen_range(min..=max)` | Inclusive range to match TC |

---

*Generated: 2026-05-01. Mark `Audited vs C++ : ✅` only after a side-by-side audit of `Loot::FillLoot` + `LootTemplate::Process` + `LootRoll::Finish` against the Rust port. The current Rust impl is a stub; the real work begins by populating `crates/wow-loot/src/lib.rs`.*

---

## 13. Audit (2026-05-01)

Side-by-side audit of `crates/wow-loot/src/lib.rs` + `crates/wow-world/src/handlers/loot.rs` vs `src/server/game/Loot/{Loot.cpp,LootMgr.cpp}` + `Handlers/LootHandler.cpp` + `Player::StoreLootItem`.

### Flagged divergence — verdict

**`CMSG_LOOT_ITEM` does not add items to inventory — CONFIRMED, silent loss.**
`crates/wow-world/src/handlers/loot.rs:135-167` is `handle_loot_item`. After locating the slot and flipping `entry.taken = true`, the loop pushes the slot into `taken_items`, sends `SMSG_LOOT_REMOVED` to the client, and ends with the literal comment `// TODO: actually add item to player inventory (DB write).` (`loot.rs:165`). There is no call to any `Player::store_loot_item` analogue, no inventory module reference, no DB write, no `Item` row insertion. The slot disappears from the loot window and the item ceases to exist anywhere — both on the wire (sent removal) and in the database. Compare with C++ `LootHandler.cpp::HandleAutostoreLootItemOpcode` → `Player::StoreLootItem` → `Item::CreateItem` + `Player::StoreNewItem` + DB queue. **Total data loss on every loot pickup.**

### Crate state

- `crates/wow-loot/src/lib.rs` — **0 bytes confirmed** (`ls -la` reports `0` size, file is empty). The crate compiles as an empty library; nothing of `Loot.cpp`'s 1083 lines is ported.
- `crates/wow-world/src/handlers/loot.rs` — 247 lines, registers and implements `LootUnit`, `LootItem`, `LootRelease`. No `LootMoney`, no `LootRoll`, no `LootMasterGive`, no `SetLootMethod`, no `OptOutOfLoot`, no `LootList`.
- `WorldSession.loot_table` is a `HashMap<ObjectGuid, CreatureLoot>` per session (loot.rs:99-104), not shared on the creature; group loot impossible by construction.

### Coverage matrix — C++ → Rust

| C++ symbol | Rust | Verdict |
|---|---|---|
| `class Loot`, `LootItem`, `LootStoreItem`, `LootStore`, `LootTemplate`, `LootGroup`, `LootRoll`, `AELootResult`, `LootItemStorage` | none | ❌ all missing |
| 12 `LootStore` globals (Creature/Disenchant/Fishing/Gameobject/Item/Mail/Milling/Pickpocketing/Prospecting/Reference/Skinning/Spell) | none | ❌ |
| `LoadLootTemplates_*` (12 SQL loaders) | none | ❌ |
| `LootStoreItem::Roll` (chance roll) | none | ❌ |
| `LootTemplate::Process` (recursive ref resolution) | none | ❌ |
| `LootGroup` weighted single-pick | none | ❌ |
| `Loot::FillLoot` | replaced by stub `generate_creature_loot` (`loot.rs:229`, `level*200 + seed%(level*300+1)` copper, no items) | ❌ |
| `Loot::generateMoneyLoot` honoring `Rate.Drop.Money` | hardcoded formula above | ❌ |
| `LootItem::AllowedForPlayer` (class/quest/race/condition) | none | ❌ |
| `Loot::BuildLootResponse` per-viewer | one global view, hardcoded `loot_method=0`, `threshold=2`, `ui_type=0` (`loot.rs:88-130`) | ❌ |
| `HandleAutostoreLootItemOpcode` → `StoreLootItem` | mark taken + send removed; **inventory write missing** | ❌ critical |
| `HandleLootMoneyOpcode` | not registered | ❌ |
| `HandleLootMasterGiveOpcode` | not registered | ❌ |
| `HandleLootRoll` (CMSG_LOOT_ROLL) | not registered | ❌ |
| `HandleSetLootMethod`, `HandleOptOutOfLoot`, `HandleLootList` | not registered | ❌ |
| `HandleLootReleaseOpcode` | ✅ `loot.rs:174` (with 30s hardcoded decay vs Trinity `RateCorpseDecayLooted * m_corpseDelay`) |  partial |
| `HandleLootOpcode` (CMSG_LOOT_UNIT) | ✅ `loot.rs:58` | partial (no items, no group view) |
| `LootError` failure codes (15+ values) | only `failure_reason: 2` is ever sent (`loot.rs:85`); comment claims `AlreadyPickedUp` but value `2` in TC enum is `LOOT_ERROR_TOO_FAR`. Wrong code regardless. | ❌ |
| `LootItemStorage` (item-loot persistence) | none | ❌ |
| `ItemRandomProperties.dbc`/`ItemRandomSuffix.dbc` rolls | none | ❌ |

### Other observed bugs

- `loot.rs:85` — `failure_reason: 2` with comment `// LootError::AlreadyPickedUp or similar`. Trinity `LOOT_ERROR_TOO_FAR = 4`, `LOOT_ERROR_LOCKED = 6`, `LOOT_ERROR_NOTSTANDING = 8`. Value 2 doesn't map to any documented code; should be `DIDNT_KILL = 0` for "alive creature".
- `loot.rs:99-104` — loot is generated lazily inside `WorldSession`; if two players try to loot the same corpse, each sees a fresh independent roll — duplicated drops, no FFA semantics.
- `loot.rs:230-235` — random copper formula uses GUID counter as seed → deterministic per-creature instead of per-kill; the same mob always drops the same amount.
- `loot.rs:206-218` — corpse decay is hardcoded `30s`; Trinity uses `RateCorpseDecayLooted` × `Creature::m_corpseDelay` (default 60s normal, longer for elites).

**Verdict:** loot module is essentially a dead stub. The flagged silent-loss bug on `CMSG_LOOT_ITEM` is real and is the most visible symptom of an entire system that does not exist yet. The whole `crates/wow-loot/` crate must be written from scratch (#LOOT.1–#LOOT.33 in §9). Of 33 listed sub-tasks, 1 is partially complete (`HandleLootReleaseOpcode`); the rest are open.
