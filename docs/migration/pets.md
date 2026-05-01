# Migration: Pets (Hunter/Warlock/Mage/DK guardians, NOT BattlePets)

> **C++ canonical path:** `src/server/game/Entities/Pet/` (`Pet`, `PetStable`, `PetDefines`) + `src/server/game/Handlers/PetHandler.cpp` + `src/server/game/Server/Packets/PetPackets.{h,cpp}` + the stable subset of `src/server/game/Handlers/NPCHandler.cpp` (`HandleRequestStabledPets` / `HandleStablePet` / `HandleStableSwapPet`)
> **Rust target crate(s):** `crates/wow-world/src/pets/` (does not exist — no `Pet` entity, no `PetStable`, no spell book, no auras), `crates/wow-world/src/handlers/pets.rs` (does not exist — only the `handle_request_stabled_pets` info-stub at `handlers/character.rs:3042`), `crates/wow-packet/src/packets/pets.rs` (does not exist — none of the 16+ packet structs are defined), `crates/wow-database/` (no `character_pet`, `character_pet_declinedname`, `pet_aura`, `pet_aura_effect`, `pet_spell`, `pet_spell_cooldown`, `pet_spell_charges` schema or prepared statements). `crates/wow-constants/src/opcodes.rs` carries the opcode constants (~16 of them, see §7).
> **Layer:** L6 (Game systems — depends on Entities/Creature L4, Spells L5, Auras L5, AI L6, Map/Grid L2, Player L4, ObjectAccessor L4; depended on by class-mechanic Scripts (Hunter, Warlock, Mage, DK), Group XP sharing, BG/Arena unsummoning logic)
> **Status:** ❌ not started — opcodes are present in `crates/wow-constants/src/opcodes.rs` (see §7) and `handle_request_stabled_pets` is a logging stub returning nothing; no `Pet` struct, no spell book, no SQL loader, no spawn flow, no AI, no save/load roundtrip, no stable.
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Pets are persistent (hunter) or class-spell-summoned (warlock Imp/Voidwalker/Felhunter/Succubus/Felguard, mage Water Elemental, DK Ghoul, shaman elementals as guardians) controllable Creatures that share their owner's faction and many combat hooks. The Pet module defines the `Pet` entity (subclass of `Guardian` → `Creature`), the per-player `PetStable` (5 active slots + up to 200 stabled hunter pets), the action bar / autocast / react state machine driven by `CharmInfo`, the persistent spell book + auras + cooldowns + charges, the rename/abandon/dismiss/feed/tame mini-flows, and the ~16 opcodes that wire all of it to the client. It also owns the XP→level scaling that keeps a hunter pet within `[owner.level - 5, owner.level]` and the talent tree (3 trees × N talents) that 3.4.3 reintroduced for hunter pets.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `src/server/game/Entities/Pet/PetDefines.h` | 186 | `PetType` (`SUMMON_PET=0`, `HUNTER_PET=1`, `MAX_PET_TYPE=4`), `PetSaveMode` (slot enum incl. `PET_SAVE_AS_CURRENT=-3`, `PET_SAVE_AS_DELETED=-2`, `PET_SAVE_NOT_IN_SLOT=-1`, 0..4 active, 5..205 stabled), `PetStableFlags`, `PetSpellState` (UNCHANGED/CHANGED/NEW/REMOVED), `PetSpellType` (NORMAL/FAMILY/TALENT), `PetActionFeedback` (None/Dead/NoTarget/InvalidTarget/NoPath), `PetTalk`, `PetTameResult` (15 codes), `StableResult` (7 codes), `MAX_ACTIVE_PETS=5`, `MAX_PET_STABLES=200`, `CALL_PET_SPELL_ID=883`, `PET_SUMMONING_DISORIENTATION=32752`, `PET_FOLLOW_DIST=1.0f`, `PET_FOLLOW_ANGLE=π`, `PetStable` class with `ActivePets[5]`, `StabledPets[200]`, `UnslottedPets[]`, `CurrentPetIndex` w/ `0x80000000` mask for unslotted |
| `src/server/game/Entities/Pet/Pet.h` | 167 | `PetSpell` struct (`active`, `state`, `type`), `PetSpellMap = unordered_map<u32, PetSpell>`, `AutoSpellList = vector<u32>`, `Pet : public Guardian` declaration, `HAPPINESS_LEVEL_SIZE = 333000` (hunter pet happiness counter), `m_petType`, `m_duration`, `m_loading`, `m_focusRegenTimer`, `m_groupUpdateMask`, `m_petSpecialization`, `m_declinedname`, `m_autospells`, `m_spells` |
| `src/server/game/Entities/Pet/Pet.cpp` | 1954 | `Pet` ctor/dtor; `AddToWorld` / `RemoveFromWorld`; `GetLoadPetInfo` (resolve which pet to load given `petEntry`/`petnumber`/`forcedSlot`); `LoadPetFromDB` (the big async holder query) → `_LoadAuras` / `_LoadSpells` / `SpellHistory::LoadFromDB`; `SavePetToDB(mode)` with the 17-column INSERT; `FillPetInfo`; `DeleteFromDB(petNumber)` (cascade delete on 7 tables); `setDeathState` override (hunter pet "JUST_DIED" persists, summon pet auto-unsummons); `Update(diff)` (focus regen, duration tick, save timer); `GivePetXP` / `GivePetLevel` / `SynchronizeLevelWithOwner`; `HaveInDiet` (hunter pet feed); `ToggleAutocast`; `LearnPetPassives` / `LearnPetTalent` / `learnSpell` / `unlearnSpell` / `removeSpell` / `addSpell`; `CleanupActionBar`; `GenerateActionBarData` (serialize 10 slots to text for `character_pet.abdata`); `InitPetCreateSpells`; `SetSpecialization` / `LearnSpecializationSpells` / `RemoveSpecializationSpells`; `CastPetAuras` / `CastPetAura` / `IsPetAura`; `_SaveAuras` / `_SaveSpells`; `Create(guidlow, map, entry, petnumber)`; `CreateBaseAtCreature` / `CreateBaseAtCreatureInfo` / `CreateBaseAtTamed` |
| `src/server/game/Handlers/PetHandler.cpp` | 810 | All pet-control opcode handlers: `HandleDismissCritter`, `HandlePetAction` + `HandlePetActionHelper` (the COMMAND_STAY/FOLLOW/ATTACK/ABANDON/MOVE_TO + REACT_PASSIVE/DEFENSIVE/AGGRESSIVE + ACT_DISABLED/PASSIVE/ENABLED switch), `HandlePetStopAttack`, `HandleQueryPetName` + `SendQueryPetNameResponse`, `HandlePetSetAction` (mutate action bar + toggle autocast), `HandlePetRename` (validation + `CheckPetName` + reserved-name + declined-name cases + DB update), `HandlePetAbandon` (hunter pet → `PET_SAVE_AS_DELETED`), `HandlePetSpellAutocastOpcode`, `HandlePetCastSpellOpcode` (the long path for client-driven pet spell cast), `SendPetNameInvalid`, `HandlePetLearnTalent`, `HandleLearnPreviewTalentsPet`, `HandleRequestPetInfo`, `CheckStableMaster` |
| `src/server/game/Handlers/NPCHandler.cpp` (stable subset) | ~150 (of 1500+) | `HandleRequestStabledPets` (lines 406–421, send `SMSG_PET_STABLE_LIST`), `SendPetStableResult(StableResult)` (line 422), `HandleStablePet`, `HandleStableSwapPet`, plus `BuyStableSlot` |
| `src/server/game/Server/Packets/PetPackets.h` | 276 | All `WorldPackets::Pet::*` packet definitions: `DismissCritter`, `RequestPetInfo`, `PetAbandon`, `PetStopAttack`, `PetSpellAutocast`, `PetSpells` (the action-bar+spells dump), `PetStableResult`, `PetLearnedSpells` / `PetUnlearnedSpells`, `PetNameInvalid`, `PetRename` (with optional `DeclinedName`), `PetAction`, `PetSetAction`, `PetCancelAura`, `SetPetSpecialization`, `PetActionFeedback`, `PetActionSound`, `PetTameFailure`, `PetMode`. Sub-structs: `PetSpellCooldown`, `PetSpellHistory`, `PetRenameData` |
| `src/server/game/Server/Packets/PetPackets.cpp` | 206 | Read/Write impls |
| `src/server/game/Entities/Unit/CharmInfo.h` | ~170 | `CharmInfo` (the per-pet action bar + react-state holder), `MAX_UNIT_ACTION_BAR_INDEX=10`, `ACTION_BAR_INDEX_PET_SPELL_START=3`, `..._END=7` (so action slots 0-2 are reactions/commands, 3-7 are spells, 8-9 reserved), `UnitActionBarEntry` packed as `(action: 23 bits) | (type: 8 bits high)`, `MAKE_UNIT_ACTION_BUTTON(A,T)` macro = `A | (T << 23)` |
| `src/server/game/Entities/Unit/UnitDefines.h` | (subset) | `enum ActiveStates : u8 { ACT_PASSIVE=0x01, ACT_DISABLED=0x81, ACT_ENABLED=0xC1, ACT_COMMAND=0x07, ACT_REACTION=0x06, ACT_DECIDE=0x00 }`, `enum ReactStates : u8 { REACT_PASSIVE=0, REACT_DEFENSIVE=1, REACT_AGGRESSIVE=2, REACT_ASSIST=3 }`, `enum CommandStates : u8 { COMMAND_STAY=0, COMMAND_FOLLOW=1, COMMAND_ATTACK=2, COMMAND_ABANDON=3, COMMAND_MOVE_TO=4 }` |
| `src/server/game/AI/CoreAI/PetAI.{h,cpp}` | ~700 | `PetAI` class — `_AttackStart`, `UpdateAI(diff)`, `KilledUnit`, owner-distance leash, autocast trigger, COMMAND_FOLLOW/STAY motion logic. The actual AI tick. |
| `src/server/database/Database/Implementation/CharacterDatabase.cpp` (pet PreparedStatementID block) | ~50 | Pet-related prepared statement registrations (see §6) |

Out-of-tree touchpoints:
- `src/server/game/Entities/Player/Player.cpp` — `Player::SummonPet`, `Player::RemovePet(pet, mode)`, `Player::IsPetNeedBeTemporaryUnsummoned`, `Player::SetTemporaryUnsummonedPetNumber`, `Player::GetPetStable()` (returns `PetStable*`), `Player::GetSummonedBattlePetGUID` (used to coordinate when a critter dismiss must un-summon a battle pet companion).
- `src/server/game/Spells/Auras/SpellAuraEffects.cpp` — `SPELL_AURA_OVERRIDE_PET_SPECS` (used in spec resolution at `LoadPetFromDB`), `SPELL_AURA_MOD_PACIFY` (HandlePetActionHelper COMMAND_ATTACK guard).
- `src/server/game/Spells/SpellHistory.cpp` — `SpellHistory::LoadFromDB<Pet>` / `SaveToDB<Pet>` use `pet_spell_cooldown` and `pet_spell_charges`.
- `src/server/game/Globals/ObjectMgr.cpp` — `ObjectMgr::CheckPetName(name)`, `ObjectMgr::CheckDeclinedNames`, `ObjectMgr::IsReservedName`.
- `src/server/game/Cache/CharacterCache.cpp` — none directly, but the pet number generator is in `ObjectMgr::GenerateLowGuid<HighGuid::Pet>` (also `HighGuid::PetAnimation` is separate).

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Pet` | class (`final`, derives from `Guardian` → `Creature` → `Unit` → `WorldObject`) | The pet entity itself |
| `Guardian` | class | Mid-class between Pet and Creature; holds owner-bonus stats |
| `PetStable` | class | The 5+200+unslotted container kept on `Player`; serialized into `character_pet.slot` |
| `PetStable::PetInfo` | struct | POD record for one stabled/active/unslotted pet (`Name`, `ActionBar`, `PetNumber`, `CreatureId`, `DisplayId`, `Experience`, `Health`, `Mana`, `LastSaveTime`, `CreatedBySpellId`, `SpecializationId`, `Level`, `ReactState`, `Type: PetType`, `WasRenamed`) |
| `PetSpell` | struct | `(active: ActiveStates, state: PetSpellState, type: PetSpellType)` — one entry in the spell book |
| `PetSpellMap` | typedef | `unordered_map<u32 spellId, PetSpell>` |
| `AutoSpellList` | typedef | `vector<u32>` — spells with `ACT_ENABLED` autocast |
| `PetSpellCooldown` | struct | Wire format: `SpellID, Duration, CategoryDuration, ModRate, Category` |
| `PetSpellHistory` | struct | Wire format: `CategoryID, RecoveryTime, ChargeModRate, ConsumedCharges` |
| `PetType` | enum (u8) | `SUMMON_PET=0` (warlock/mage class summons), `HUNTER_PET=1`, `MAX_PET_TYPE=4` |
| `PetSaveMode` | enum (i16) | Slot codes — see §2 row 1 |
| `PetStableFlags` | enum (u8) | `PET_STABLE_ACTIVE=0x1`, `PET_STABLE_INACTIVE=0x2` |
| `PetSpellState` | enum | `UNCHANGED=0`, `CHANGED=1`, `NEW=2`, `REMOVED=3` |
| `PetSpellType` | enum | `NORMAL=0`, `FAMILY=1`, `TALENT=2` |
| `PetActionFeedback` | enum class (u8) | `None=0`, `Dead=1`, `NoTarget=2`, `InvalidTarget=3`, `NoPath=4` — wire-format payload of `SMSG_PET_ACTION_FEEDBACK` |
| `PetTalk` | enum | `PET_TALK_SPECIAL_SPELL=0`, `PET_TALK_ATTACK=1` — used in `Unit::SendPetTalk` |
| `PetTameResult` | enum class (u8) | 15 codes (Ok, InvalidCreature, TooMany, CreatureAlreadyOwned, NotTameable, AnotherSummonActive, UnitsCantTame, NoPetAvailable, InternalError, TooHighLevel, Dead, NotDead, CantControlExotic, InvalidSlot, EliteTooHighLevel) — payload of `SMSG_PET_TAME_FAILURE` |
| `StableResult` | enum class (u8) | `NotEnoughMoney=1`, `InvalidSlot=3`, `StableSuccess=8`, `UnstableSuccess=9`, `BuySlotSuccess=10`, `CantControlExotic=11`, `InternalError=12` — payload of `SMSG_PET_STABLE_RESULT` |
| `ActiveStates` | enum (u8) | Action bar entry "type" (high byte) — `PASSIVE=0x01`, `DISABLED=0x81`, `ENABLED=0xC1`, `COMMAND=0x07`, `REACTION=0x06`, `DECIDE=0x00` |
| `ReactStates` | enum (u8) | `REACT_PASSIVE=0`, `REACT_DEFENSIVE=1`, `REACT_AGGRESSIVE=2`, `REACT_ASSIST=3` |
| `CommandStates` | enum (u8) | `COMMAND_STAY=0`, `COMMAND_FOLLOW=1`, `COMMAND_ATTACK=2`, `COMMAND_ABANDON=3`, `COMMAND_MOVE_TO=4` |
| `CharmInfo` | struct | Action bar + react state + command state + charm-time spell snapshot |
| `UnitActionBarEntry` | struct | Packed `u32 packedData` = `(action_id_23bits) | (active_state_8bits << 23)` |
| `PetRenameData` | struct | Wire: `PetGUID`, `PetNumber: i32`, `NewName: string`, `Optional<DeclinedName>` |
| Constants | — | `MAX_ACTIVE_PETS=5`, `MAX_PET_STABLES=200`, `MAX_UNIT_ACTION_BAR_INDEX=10`, `ACTION_BAR_INDEX_PET_SPELL_START=3`, `ACTION_BAR_INDEX_PET_SPELL_END=7`, `PET_FOLLOW_DIST=1.0f`, `PET_FOLLOW_ANGLE=π`, `HAPPINESS_LEVEL_SIZE=333000`, `CALL_PET_SPELL_ID=883` (Hunter "Call Pet"), `PET_SUMMONING_DISORIENTATION=32752` (5s daze aura applied on summon), `PET_XP_FACTOR=0.05f` (XP earn coefficient — pet gets 5% of mob XP), `UnslottedPetIndexMask=0x80000000` |

WorldPackets sub-namespace `Pet` (defined in `WorldPackets/Pet/PetPackets.h`):

| Packet | Direction | Fields |
|---|---|---|
| `DismissCritter` | C→S | `CritterGUID: ObjectGuid` |
| `RequestPetInfo` | C→S | (empty) |
| `PetAbandon` | C→S | `Pet: ObjectGuid` |
| `PetStopAttack` | C→S | `PetGUID: ObjectGuid` |
| `PetSpellAutocast` | C→S | `PetGUID, SpellID: u32, AutocastEnabled: bool` |
| `PetSpells` | S→C | `PetGUID, _CreatureFamily: u16, Specialization: u16, TimeLimit: u32, ReactState: u8, CommandState: u8, Flag: u8, ActionButtons[10]: i32, Actions: vec<u32>, Cooldowns: vec<PetSpellCooldown>, SpellHistory: vec<PetSpellHistory>` |
| `PetStableResult` | S→C | `Result: u8` (StableResult code) |
| `PetLearnedSpells` / `PetUnlearnedSpells` | S→C | `Spells: vec<u32>` |
| `PetNameInvalid` | S→C | `RenameData: PetRenameData, Result: u8` |
| `PetRename` | C→S | `RenameData: PetRenameData` |
| `PetAction` | C→S | `PetGUID, Action: u32, TargetGUID: ObjectGuid, ActionPosition: Position::XYZ` |
| `PetSetAction` | C→S | `PetGUID, Index: u32, Action: u32` |
| `PetCancelAura` | C→S | `PetGUID, SpellID: i32` |
| `SetPetSpecialization` | S→C | `SpecID: u16` |
| `PetActionFeedback` | S→C | `SpellID: i32, Response: PetActionFeedback (u8)` |
| `PetActionSound` | S→C | `UnitGUID, Action: i32` |
| `PetTameFailure` | S→C | `Result: u8` (PetTameResult code) |
| `PetMode` | S→C | `PetGUID, ReactState: ReactStates, CommandState: CommandStates, Flag: u8` |

NPC-handler stable packets (`WorldPackets::NPC`):

| Packet | Direction | Fields |
|---|---|---|
| `RequestStabledPets` | C→S | `StableMaster: ObjectGuid` |
| `PetStableList` | S→C | `StableMaster: ObjectGuid`, `Pets: vec<{ PetSlot, PetNumber, CreatureID, DisplayID, ExperienceLevel, PetFlags, PetName }>` |
| `StablePet` | C→S | `StableMaster: ObjectGuid` |
| `StableSwapPet` | C→S | `StableMaster: ObjectGuid, PetNumber: u32` |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `Pet::Pet(Player* owner, PetType)` | Construct with owner, sets `m_unitTypeMask |= UNIT_MASK_PET (\| UNIT_MASK_HUNTER_PET)`, calls `InitCharmInfo()` | `Guardian` ctor, `InitCharmInfo` |
| `Pet::AddToWorld()` | Insert into Map's `ObjectsStore<Pet>`, call `Unit::AddToWorld`, init AI, run zone-script `OnCreatureCreate`, reset stuck-on-zone follow flags | `Map::GetObjectsStore`, `AIM_Initialize`, `ZoneScript::OnCreatureCreate`, `CharmInfo::SetIsCommandFollow(false)` etc |
| `Pet::Create(guidlow, map, entry, petnumber)` | Allocate the `ObjectGuid<HighGuid::Pet>`, fetch CreatureTemplate, set basic Unit fields | `Creature::Create`, `Object::_Create` |
| `Pet::CreateBaseAtCreature(creature)` | Used by `HandleSpellLearnSpell` Tame path: copy faction, level, family from creature being tamed | — |
| `Pet::CreateBaseAtCreatureInfo(cinfo, owner)` | Used by `Player::SummonPet` for SUMMON_PET (warlock/mage): build a fresh pet from CreatureTemplate of the summoned-pet entry | — |
| `Pet::CreateBaseAtTamed(cinfo, map)` | Used by `Pet::LoadPetFromDB` for HUNTER_PET when re-summoning from `character_pet`: hp/mana/level taken from DB, not template | — |
| `Pet::GetLoadPetInfo(stable, petEntry, petnumber, slot)` | Static — pick which `PetInfo*` to load given the four kinds of selectors (by petnumber → search all 5+200+unslotted; by slot → look in active/stable arrays; by petEntry → search unslotted; default → first active or first unslotted) | — |
| `Pet::LoadPetFromDB(owner, petEntry, petnumber, current, forcedSlot)` | The big async loader: resolves slot, runs `PetLoadQueryHolder` with 6 prepared SELECTs (declined names, auras, aura effects, spells, cooldowns, charges), creates the entity, calls `InheritPhaseShift`, sets faction/class/scale, calls `InitStatsForLevel` → `SynchronizeLevelWithOwner`, places at owner+follow-angle, sets reactstate, restores hp/mana, fires the after-complete callback that loads auras/spells/spell history, learns passives, casts pet auras | `PetStable::*`, `Map::GenerateLowGuid<HighGuid::Pet>`, `PhasingHandler::InheritPhaseShift`, `_LoadAuras`, `_LoadSpells`, `SpellHistory::LoadFromDB<Pet>`, `LearnPetPassives`, `CastPetAuras`, `Player::SetMinion(this, true)` |
| `Pet::SavePetToDB(mode)` | Build the 17-column INSERT against `character_pet` (after deleting existing), and call `_SaveAuras` + `_SaveSpells` + `SpellHistory::SaveToDB<Pet>` in a transaction. Skips for non-Player owners and non-controlled. Mode-aware: `PET_SAVE_AS_DELETED` → cascade `DeleteFromDB`; `PET_SAVE_AS_CURRENT` → resolves to current active slot index | `_SaveAuras`, `_SaveSpells`, `SpellHistory::SaveToDB<Pet>`, `CharacterDatabase.GetPreparedStatement`, `FillPetInfo` |
| `Pet::FillPetInfo(petInfo, forcedReactState)` | Copy live entity state into a `PetStable::PetInfo` for serialization | — |
| `Pet::DeleteFromDB(petNumber)` (static) | Cascade-delete the 7 pet-related tables in a single transaction | `CHAR_DEL_CHAR_PET_BY_ID`, `CHAR_DEL_CHAR_PET_DECLINEDNAME`, `CHAR_DEL_PET_AURA_EFFECTS`, `CHAR_DEL_PET_AURAS`, `CHAR_DEL_PET_SPELLS`, `CHAR_DEL_PET_SPELL_COOLDOWNS`, `CHAR_DEL_PET_SPELL_CHARGES` |
| `Pet::Update(diff)` | Per-tick: focus regen (`m_focusRegenTimer` decrements then fires `RegenerateAll`), duration tick (`m_duration` for temporary summons; <=0 → unsummon), forced-save timer (every `CONFIG_PET_SAVE_TIMER`, default 60s) | `Creature::Update`, `Unit::Update`, `RegeneratePower`, `Remove(PET_SAVE_AS_CURRENT)` |
| `Pet::setDeathState(state)` | Override: `JUST_DIED` for hunter pet → drop from world but keep slot; for summon pet → call `Remove(PET_SAVE_NOT_IN_SLOT)` | `Creature::setDeathState`, `Unit::setDeathState` |
| `Pet::GivePetXP(xp)` | Hunter only: add to `UnitData::PetExperience`, level up while XP >= next-level threshold (computed from `xp_for_pet_level` table), call `GivePetLevel(newLevel)` | `SetPetExperience`, `GivePetLevel`, `InitStatsForLevel` |
| `Pet::GivePetLevel(level)` | Set level field, recompute stats, send `SMSG_PET_LEVEL_UP` | `InitStatsForLevel`, `LearnPetTalent`-eligibility checks |
| `Pet::SynchronizeLevelWithOwner()` | Hunter pet: clamp to `[owner.level - 5, owner.level]` on summon; summon pet always = owner level | `GivePetLevel`, `InitStatsForLevel` |
| `Pet::HaveInDiet(itemTemplate)` | Hunter pet feed check: bitmask `creatureFamily.PetFoodMask & item.FoodType` | — |
| `Pet::ToggleAutocast(spellInfo, apply)` | Mutate the `PetSpell::active` field + add/remove from `m_autospells` | — |
| `Pet::HasSpell(spell)` (override) | `m_spells.find(spell) != end()` and not `PETSPELL_REMOVED` | — |
| `Pet::LearnPetPassives()` | For HUNTER_PET: add `creatureFamily.spellSet` passive list. For SUMMON_PET: nothing (those come from `pet_levelstats` and `creature_template_addon`) | `addSpell(s, ACT_PASSIVE, NEW, FAMILY)` |
| `Pet::LearnPetTalent(talentId)` | 3.4.3 hunter pet talent system: TalentEntry lookup, validate prereqs, learn the talent's spell, mark `PETSPELL_TALENT` | `addSpell(spell, ACT_DECIDE, NEW, TALENT)` |
| `Pet::CastPetAuras(current)` | Apply persistent talent/Mark-of-the-Wild-style auras owned by the **owner** that affect this pet (from `pet_aura` Trinity DB table → `PetAura` cache) | `CastPetAura` |
| `Pet::CastPetAura(petAura)` | `CastSpell(this, petAura.spellId, true)` if pet level satisfies threshold | — |
| `Pet::IsPetAura(aura)` | Owned-by-this-pet auras (excludes player-cast buffs that simply target the pet) | — |
| `Pet::_LoadAuras(auraResult, effectResult, timediff)` | Restore all rows from `pet_aura` + `pet_aura_effect`, decreasing `remainTime` by `timediff` | `Unit::_AddAura` |
| `Pet::_SaveAuras(trans)` | Persist current Pet auras (excluding NPC-casted, expired, single-target) into `pet_aura` + `pet_aura_effect` | `CHAR_INS_PET_AURA`, `CHAR_INS_PET_AURA_EFFECT` |
| `Pet::_LoadSpells(result)` / `_SaveSpells(trans)` | Load/save `pet_spell` rows | `addSpell(...PETSPELL_UNCHANGED)`, `CHAR_INS_PET_SPELL`/`CHAR_DEL_PET_SPELL_BY_SPELL` |
| `Pet::addSpell(id, active, state, type)` | Mutate `m_spells`, `m_autospells`, fire `SMSG_PET_LEARNED_SPELLS` if state `NEW` | — |
| `Pet::learnSpell(id)` / `learnSpells([])` | Public learn (delegates to addSpell) | — |
| `Pet::unlearnSpell(id, learn_prev, clear_ab)` | Remove from spell book; if `learn_prev` and a previous-rank exists, re-learn it; if `clear_ab` clear from action bar | `removeSpell` |
| `Pet::CleanupActionBar()` | After spell-list mutation, walk action bar slots 0..9 and remove any spell ID the pet no longer knows | `CharmInfo::SetActionBar(slot, 0, ACT_DISABLED)` |
| `Pet::GenerateActionBarData()` | Build the comma-separated text serialized form (for `character_pet.abdata`) — 10 entries of `"<active_state> <action>"` | — |
| `Pet::InitPetCreateSpells()` | Called for SUMMON_PET on first creation: read `creature_template_addon.spell1..8` and learn each | `addSpell(spell, ACT_DECIDE, NEW, NORMAL)` |
| `Pet::SetSpecialization(specId)` | Pet specialization (3.4.3 Beast Mastery added Cunning/Tenacity/Ferocity for hunter pets) — apply spec passives, send `SMSG_SET_PET_SPECIALIZATION` | `LearnSpecializationSpells` |
| `Pet::SetGroupUpdateFlag(flag)` | Mark a `m_groupUpdateMask` bit so the next group-update tick re-broadcasts pet stats | — |
| `WorldSession::HandlePetAction(packet)` | Top-level dispatcher: validate not mounted, resolve pet, validate ownership, check if alive (allow only `SPELL_ATTR0_ALLOW_CAST_WHILE_DEAD` if dead), guard charmed-player edge case, then call `HandlePetActionHelper` for one or many controlled units of same entry | `HandlePetActionHelper` |
| `WorldSession::HandlePetActionHelper(pet, guid1, spellid, flag, guid2, pos)` | The big switch on `flag`: `ACT_COMMAND` → STAY / FOLLOW / ATTACK / ABANDON / MOVE_TO; `ACT_REACTION` → set ReactState; `ACT_DISABLED`/`ACT_PASSIVE`/`ACT_ENABLED` → cast spell with `Spell::CheckPetCast`, send `PetActionFeedback` on failure | `MotionMaster::Clear/MoveIdle/MoveFollow/MovePoint`, `AttackStop`, `PetAI::_AttackStart`, `Spell::Cast`, `SendPetTalk`, `SendPetAIReaction` |
| `WorldSession::HandlePetStopAttack(pkt)` | Validate pet/charmed, call `pet->AttackStop()` | — |
| `WorldSession::HandleQueryPetName(pkt)` / `SendQueryPetNameResponse(guid)` | Build `SMSG_QUERY_PET_NAME_RESPONSE` with name + timestamp + declined names | `CharacterCache`, `pet_decldname` query |
| `WorldSession::HandlePetSetAction(pkt)` | Mutate one action bar slot; if it's an autocast spell, propagate via `Pet::ToggleAutocast` to all controlled pets of same entry | `Pet::ToggleAutocast`, `CharmInfo::SetActionBar` |
| `WorldSession::HandlePetRename(pkt)` | Validate hunter pet, name, reserved-name, declined names; persist via `CHAR_UPD_CHAR_PET_NAME` and (optionally) `CHAR_INS_CHAR_PET_DECLINEDNAME`; clear `UNIT_PET_FLAG_CAN_BE_RENAMED` (one-shot) | `ObjectMgr::CheckPetName`, `ObjectMgr::CheckDeclinedNames` |
| `WorldSession::HandlePetAbandon(pkt)` | Hunter pet → `Player::RemovePet(pet, PET_SAVE_AS_DELETED)` | `Player::RemovePet` |
| `WorldSession::HandlePetSpellAutocastOpcode(pkt)` | `Pet::ToggleAutocast` for matching controlled pets (e.g. multiple totems share state) | — |
| `WorldSession::HandlePetCastSpellOpcode(pkt)` | Client-side pet bar click → `Spell` ctor on the pet, `Spell::prepare` with target | `Spell::prepare`, `Spell::CheckPetCast` |
| `WorldSession::HandleDismissCritter(pkt)` | Dismiss the temporary critter companion (separate from battle pet); if it's the active battle pet companion, also clear `SetBattlePetData(nullptr)` | `TempSummon::UnSummon` |
| `WorldSession::HandlePetLearnTalent(pkt)` | Validate hunter pet + talent points available + talent prereqs; `Pet::LearnPetTalent(talentId)`; deduct one talent point | `Pet::LearnPetTalent` |
| `WorldSession::HandleLearnPreviewTalentsPet(pkt)` | Bulk learn (preview-mode commit): list of `talentId+rank` pairs | — |
| `WorldSession::HandleRequestStabledPets(pkt)` | `CheckStableMaster(npcGuid)`, then build `SMSG_PET_STABLE_LIST` from `PetStable::ActivePets[1..4] + StabledPets[0..199]` (slot 0 is the currently-summoned active pet, omitted from the list visually) | `CheckStableMaster`, `PetStable` |
| `WorldSession::HandleStablePet(pkt)` | Move current pet from `ActivePets[0]` → first empty `StabledPets[i]`; `Player::RemovePet(PET_SAVE_FIRST_STABLE_SLOT + i)`; reply `STABLE_SUCCESS` | `Player::RemovePet`, `SendPetStableResult` |
| `WorldSession::HandleStableSwapPet(pkt)` | Swap currently-active pet with stabled-by-petNumber: ensure exotic-tame allowed if target is exotic, save old, load new | `Pet::LoadPetFromDB` |
| `WorldSession::HandleBuyStableSlot(pkt)` (in NPCHandler.cpp) | Cost is in `CONFIG_CHARGE_FOR_STABLE_SLOTS` × current count; expand `PetStable::StabledPets` capacity by 1; reply `BUY_SLOT_SUCCESS` | — |
| `WorldSession::CheckStableMaster(guid)` | Validate NPC interactable + has `UNIT_NPC_FLAG_STABLEMASTER` + faction reaction sufficient | `Player::GetNPCIfCanInteractWith` |

---

## 5. Module dependencies

**Depends on:**
- `Entities/Creature` — `Pet : public Guardian : public Creature`. Creature template lookup, faction, family, model, sound IDs.
- `Entities/Unit` — `CharmInfo`, `UnitFlags`, `UNIT_FIELD_PET_NUMBER`, `UNIT_FIELD_PETEXPERIENCE`, `UNIT_FIELD_PETNEXTLEVELEXP`, `UNIT_PET_FLAG_CAN_BE_RENAMED` / `..._ABANDONED`.
- `Spells` — `SpellInfo::HasAttribute(SPELL_ATTR0_ALLOW_CAST_WHILE_DEAD)`, `Spell::CheckPetCast`, `Spell::prepare`. `SpellMgr::GetSpellInfo`. `SpellHistory<Pet>` for cooldowns/charges.
- `AI/CoreAI/PetAI` — `PetAI::_AttackStart` (force target switch on COMMAND_ATTACK), `PetAI::UpdateAI` (autocast trigger, owner-distance leash, motion).
- `Movement/MotionMaster` — `MoveIdle`, `MoveFollow(owner, PET_FOLLOW_DIST, angle)`, `MovePoint` (COMMAND_MOVE_TO).
- `Map / Grid` — `Map::GenerateLowGuid<HighGuid::Pet>`, `Map::GetObjectsStore`, `Map::AddToMap`.
- `Player` — `Player::SummonPet`, `Player::RemovePet(pet, mode)`, `Player::GetPetStable()`, `Player::GetPet()` (active controlled pet), `Player::IsPetNeedBeTemporaryUnsummoned`, `Player::SetMinion(this, true)`, `Player::DisablePetControlsOnMount`, `Player::CanTameExoticPets`.
- `ObjectAccessor` — `GetUnit`, `GetCreatureOrPetOrVehicle`, `GetPet`, `FindConnectedPlayer`.
- `ObjectMgr` — `CheckPetName`, `CheckDeclinedNames`, `IsReservedName`, `GetCreatureTemplate`, talent storage.
- `DB2/DBC` — `ChrSpecializationStore` (pet spec resolution), `TalentStore` (pet talents — though 3.4.3 uses a separate `pet_talent_tree` table), `CreatureFamilyStore`, `CreatureFamilyXTalentTree`.
- `Cache/CharacterCache` — pet number generator state.
- `CharacterDatabase` — 7 tables, ~25 prepared statements.
- `World config` — `CONFIG_PET_SAVE_TIMER` (default 60000ms), `CONFIG_RATE_PET_XP_KILL`, `CONFIG_RATE_PET_TALENT_POINTS`, `CONFIG_CHARGE_FOR_STABLE_SLOTS`, `CONFIG_MAX_PET_STABLES` (effective cap).

**Depended on by:**
- `Player::DeleteFromDB` — calls `PetMgr::DeletePetsByOwner` indirectly via `CHAR_DEL_CHAR_PET_BY_OWNER` + cascade.
- `Group / GroupReference` — `GROUP_UPDATE_PET_FULL` flag, pet hp/mana/auras broadcast to party members. `Group::UpdatePlayerOutOfRange` skips pet field set when out of range.
- `Battleground / Arena` — `Pet::Remove(PET_SAVE_NOT_IN_SLOT)` on BG enter for warlock pets; `Pet::RemoveArenaAuras` filter.
- `Spell::SummonAllTotems`, `Spell::EffectSummonPet` (warlock summon spell families), `Spell::EffectTameCreature` (hunter tame).
- Class scripts — `BeastMastery`, `Demonology` (warlock), `FrostMage` (water elemental), `UnholyDeathKnight` (Army of the Dead = many short-duration guardians, **not** persistent pets).
- `Scripts/SmartScripts` — `SMART_ACTION_SUMMON_CREATURE`, `SMART_ACTION_FOLLOW`.

---

## 6. SQL / DB queries (if any)

Schema (3.4.3 character DB):

```sql
CREATE TABLE character_pet (
  id              INT UNSIGNED NOT NULL DEFAULT 0,           -- pet_number (the canonical pet identity, separate from creature entry)
  entry           INT UNSIGNED NOT NULL DEFAULT 0,           -- creature_template.entry
  owner           BIGINT UNSIGNED NOT NULL DEFAULT 0,        -- character.guid (low part of player ObjectGuid)
  modelid         INT UNSIGNED DEFAULT 0,
  CreatedBySpell  INT UNSIGNED NOT NULL DEFAULT 0,
  PetType         TINYINT UNSIGNED NOT NULL DEFAULT 0,       -- PetType enum
  level           SMALLINT UNSIGNED NOT NULL DEFAULT 1,
  exp             INT UNSIGNED NOT NULL DEFAULT 0,
  Reactstate      TINYINT UNSIGNED NOT NULL DEFAULT 0,
  name            VARCHAR(21) NOT NULL DEFAULT 'Pet',
  renamed         TINYINT UNSIGNED NOT NULL DEFAULT 0,       -- 0 = can be renamed, 1 = locked
  slot            SMALLINT NOT NULL DEFAULT -1,              -- PetSaveMode (-1, 0..4, 5..205)
  curhealth       INT UNSIGNED NOT NULL DEFAULT 1,
  curmana         INT UNSIGNED NOT NULL DEFAULT 0,
  savetime        INT UNSIGNED NOT NULL DEFAULT 0,
  abdata          TEXT,                                       -- "type action type action ..." × 10 slots
  specialization  SMALLINT UNSIGNED NOT NULL DEFAULT 0,
  PRIMARY KEY (id), KEY owner(owner), KEY idx_slot(slot)
);

CREATE TABLE character_pet_declinedname (
  id INT UNSIGNED, owner INT UNSIGNED,
  genitive VARCHAR(12), dative VARCHAR(12), accusative VARCHAR(12),
  instrumental VARCHAR(12), prepositional VARCHAR(12),
  PRIMARY KEY (id), KEY owner_key(owner)
);

CREATE TABLE pet_aura (
  guid            INT UNSIGNED,                              -- = pet_number
  casterGuid      BIGINT UNSIGNED,
  spell           INT UNSIGNED,
  effectMask      TINYINT UNSIGNED,
  recalculateMask TINYINT UNSIGNED,
  difficulty      TINYINT UNSIGNED,
  stackCount      TINYINT UNSIGNED,
  maxDuration     INT,
  remainTime      INT,
  remainCharges   TINYINT UNSIGNED,
  PRIMARY KEY (guid, casterGuid, spell, effectMask)
);

CREATE TABLE pet_aura_effect (
  guid INT UNSIGNED, casterGuid BIGINT UNSIGNED, spell INT UNSIGNED,
  effectMask TINYINT UNSIGNED, effectIndex TINYINT UNSIGNED,
  amount INT, baseAmount INT,
  PRIMARY KEY (guid, casterGuid, spell, effectMask, effectIndex)
);

CREATE TABLE pet_spell (guid INT UNSIGNED, spell INT UNSIGNED, active TINYINT UNSIGNED, PRIMARY KEY(guid, spell));
CREATE TABLE pet_spell_cooldown (guid INT UNSIGNED, spell INT UNSIGNED, time INT UNSIGNED, categoryId INT UNSIGNED, categoryEnd INT UNSIGNED, PRIMARY KEY(guid, spell));
CREATE TABLE pet_spell_charges (guid INT UNSIGNED, categoryId INT UNSIGNED, rechargeStart INT UNSIGNED, rechargeEnd INT UNSIGNED, PRIMARY KEY(guid, categoryId, rechargeStart));
```

Prepared statements (CharacterDatabase):

| Statement | Purpose | DB |
|---|---|---|
| `CHAR_SEL_CHAR_PETS` | Load all rows of `character_pet` for an owner — `SELECT id, entry, modelid, level, exp, Reactstate, slot, name, renamed, curhealth, curmana, abdata, savetime, CreatedBySpell, PetType, specialization FROM character_pet WHERE owner = ?` | character |
| `CHAR_SEL_CHAR_PET_IDS` | `SELECT id FROM character_pet WHERE owner = ?` (for delete cascade) | character |
| `CHAR_INS_PET` | The 17-column insert in `Pet::SavePetToDB` | character |
| `CHAR_DEL_CHAR_PET_BY_ID` | `DELETE FROM character_pet WHERE id = ?` | character |
| `CHAR_DEL_CHAR_PET_BY_OWNER` | `DELETE FROM character_pet WHERE owner = ?` (character delete) | character |
| `CHAR_UPD_CHAR_PET_NAME` | `UPDATE character_pet SET name = ?, renamed = 1 WHERE owner = ? AND id = ?` | character |
| `CHAR_UPD_CHAR_PET_SLOT_BY_ID` | `UPDATE character_pet SET slot = ? WHERE owner = ? AND id = ?` (used by stable handlers) | character |
| `CHAR_SEL_PET_DECLINED_NAME` | `SELECT genitive, dative, accusative, instrumental, prepositional FROM character_pet_declinedname WHERE owner = ? AND id = ?` | character |
| `CHAR_INS_CHAR_PET_DECLINEDNAME` | Insert with 5 declension columns | character |
| `CHAR_DEL_CHAR_PET_DECLINEDNAME` / `_BY_OWNER` | Delete declined names | character |
| `CHAR_SEL_PET_AURA` / `CHAR_SEL_PET_AURA_EFFECT` | Load auras + effects (the long column lists in `Pet::_LoadAuras`) | character |
| `CHAR_INS_PET_AURA` / `CHAR_INS_PET_AURA_EFFECT` | Save auras + effects | character |
| `CHAR_DEL_PET_AURAS` / `CHAR_DEL_PET_AURA_EFFECTS` | Cascade delete | character |
| `CHAR_SEL_PET_SPELL` | `SELECT spell, active FROM pet_spell WHERE guid = ?` | character |
| `CHAR_INS_PET_SPELL` | `INSERT INTO pet_spell (guid, spell, active) VALUES (?, ?, ?)` | character |
| `CHAR_DEL_PET_SPELL_BY_SPELL` | `DELETE FROM pet_spell WHERE guid = ? AND spell = ?` | character |
| `CHAR_DEL_PET_SPELLS` | `DELETE FROM pet_spell WHERE guid = ?` | character |
| `CHAR_SEL_PET_SPELL_COOLDOWN` | `SELECT spell, time, categoryId, categoryEnd FROM pet_spell_cooldown WHERE guid = ? AND time > UNIX_TIMESTAMP()` | character |
| `CHAR_INS_PET_SPELL_COOLDOWN` / `CHAR_DEL_PET_SPELL_COOLDOWNS` | Cooldown persistence | character |
| `CHAR_SEL_PET_SPELL_CHARGES` / `CHAR_INS_PET_SPELL_CHARGES` / `CHAR_DEL_PET_SPELL_CHARGES` | Spell-charge persistence (3.4.3 has charge-based abilities) | character |

DB2/DBC stores read by the Pet module:

| Store | What it loads | Read by |
|---|---|---|
| `CreatureFamilyStore` | CreatureFamily.db2 — diet mask, base attack speed, pet talent tree id, pet scale | `Pet::HaveInDiet`, `LearnPetPassives`, talent tree resolution |
| `ChrSpecializationStore` | ChrSpecialization.db2 — pet spec entries (Cunning/Tenacity/Ferocity have ChrSpecialization rows) | `Pet::SetSpecialization`, `LoadPetFromDB` |
| `TalentStore` | Talent.db2 — for `LearnPetTalent` validation | `LearnPetTalent` |
| `CreatureModelDataStore` | for `Pet::SetDisplayId(..., setNative=true)` scale calc | — |
| World DB `creature_template_addon.spell1..8` | Initial spell book for SUMMON_PET on creation | `InitPetCreateSpells` |
| World DB `pet_levelstats` (per family × level) | HP/mana/stat curves | `Pet::InitStatsForLevel` |
| World DB `pet_aura` (Trinity-specific, separate from character DB `pet_aura`) | Owner-cast pet auras (Mark of the Wild–style scaling) | `Pet::CastPetAuras` |
| World DB `pet_name_generation` | `Pet::GenerateName` for hunter pets | — |

---

## 7. Wire-protocol packets (if any)

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_PET_ACTION` (0x348b) | client → server | `WorldSession::HandlePetAction` |
| `CMSG_PET_SET_ACTION` (0x348a) | client → server | `WorldSession::HandlePetSetAction` |
| `CMSG_PET_STOP_ATTACK` (0x348c) | client → server | `WorldSession::HandlePetStopAttack` |
| `CMSG_PET_ABANDON` (0x348d) | client → server | `WorldSession::HandlePetAbandon` |
| `CMSG_PET_CANCEL_AURA` (0x348e) | client → server | `WorldSession::HandlePetCancelAura` |
| `CMSG_PET_SPELL_AUTOCAST` (0x348f) | client → server | `WorldSession::HandlePetSpellAutocastOpcode` |
| `CMSG_REQUEST_PET_INFO` (0x3490) | client → server | `WorldSession::HandleRequestPetInfo` (re-pushes `PetSpells`) |
| `CMSG_PET_RENAME` (0x3686) | client → server | `WorldSession::HandlePetRename` |
| `CMSG_DISMISS_CRITTER` (0x34f9) | client → server | `WorldSession::HandleDismissCritter` |
| `CMSG_LEARN_PREVIEW_TALENTS_PET` (0x3555) | client → server | `WorldSession::HandleLearnPreviewTalentsPet` |
| `CMSG_PET_LEARN_TALENT` (no opcode in 3.4.3 — 3.4.3 uses `CMSG_LEARN_PREVIEW_TALENTS_PET` instead, or a sub-handler from spell-cast of the talent learn spell) | — | (verify against 3.4.3.54261 wire) |
| `SMSG_PET_SPELLS_MESSAGE` (0x2c22) | server → client | `Player::PetSpellInitialize` (after pet load) |
| `SMSG_PET_MODE` (0x2588) | server → client | `Pet::SetReactState`, `CharmInfo::SetCommandState` |
| `SMSG_PET_NAME_INVALID` (no opcode constant in current Rust list — 3.4.3 likely 0x33XX, verify) | server → client | `WorldSession::SendPetNameInvalid` |
| `SMSG_PET_TAME_FAILURE` (0x26b3) | server → client | `Spell::EffectTameCreature` failure |
| `SMSG_PET_ACTION_FEEDBACK` (0x2749) | server → client | `Pet::SendPetActionFeedback` (NoPath/InvalidTarget/Dead) |
| `SMSG_PET_ACTION_SOUND` (0x26a0) | server → client | `Unit::SendPetTalk` |
| `SMSG_PET_LEARNED_SPELLS` / `SMSG_PET_UNLEARNED_SPELLS` | server → client | `Pet::addSpell(state=NEW)` / `removeSpell` |
| `SMSG_SET_PET_SPECIALIZATION` | server → client | `Pet::SetSpecialization` |
| `SMSG_PET_STABLE_LIST` | server → client | `WorldSession::HandleRequestStabledPets` |
| `SMSG_PET_STABLE_RESULT` (0x2593) | server → client | `WorldSession::SendPetStableResult` |
| `SMSG_QUERY_PET_NAME_RESPONSE` | server → client | `WorldSession::SendQueryPetNameResponse` |
| `CMSG_QUERY_PET_NAME` | client → server | `WorldSession::HandleQueryPetName` |
| `CMSG_REQUEST_STABLED_PETS` | client → server | `WorldSession::HandleRequestStabledPets` |
| `CMSG_STABLE_PET` (0x3168) | client → server | `WorldSession::HandleStablePet` |
| `CMSG_UNSTABLE_PET` (0x3169) | client → server | `WorldSession::HandleStableSwapPet` |
| `CMSG_STABLE_SWAP_PET` (0x316a) | client → server | `WorldSession::HandleStableSwapPet` |
| `CMSG_BUY_STABLE_SLOT` (0x316b) | client → server | `WorldSession::HandleBuyStableSlot` |

(There's no separate `Pet`-specific subnamespace for the stable opcodes in TC — they're under `WorldPackets::NPC` because the conversation is with a stable-master NPC.)

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-constants/src/opcodes.rs` — ~16 pet/stable opcode constants present (PetAbandon, PetAction, PetCancelAura, PetRename, PetSetAction, PetSpellAutocast, PetStopAttack, RequestPetInfo, DismissCritter, LearnPreviewTalentsPet, StablePet, StableSwapPet, UnstablePet, BuyStableSlot, RequestStabledPets [in NPC group], plus all the SMSG: PetActionFeedback, PetActionSound, PetMode, PetSpellsMessage, PetStableResult, PetTameFailure). No Rust-side enum mirrors of CommandStates/ReactStates/ActiveStates/PetType/PetSaveMode.
- `crates/wow-world/src/handlers/character.rs:3040–3045` — `handle_request_stabled_pets` is a single-line info-log stub ("RequestStabledPets account {} (stub)"); does not query DB, does not send `SMSG_PET_STABLE_LIST`.
- No `crates/wow-world/src/pets/` directory. No `Pet` struct anywhere. No `PetStable`. No `CharmInfo`. No spell book / aura book / cooldowns / charges.
- No `crates/wow-packet/src/packets/pets.rs`. None of the 16+ packet structs (`PetAction`, `PetSpells`, `PetMode`, `PetStableList`, `PetActionFeedback`, etc.) exist.
- No SQL migrations for `character_pet`, `character_pet_declinedname`, `pet_aura`, `pet_aura_effect`, `pet_spell`, `pet_spell_cooldown`, `pet_spell_charges` in `crates/wow-database/`.
- No prepared-statement entries in `crates/wow-database/src/statements/` for any of the ~25 `CHAR_*_PET*` statements.
- No `PetAI` in `crates/wow-ai/src/`. No autocast logic. No follow-distance leash. No owner-mount disable hook.

**What's implemented:**
- Opcode integers only.

**What's missing vs C++:**
- The `Pet` entity itself (1954 lines of Pet.cpp, 167 of Pet.h, 186 of PetDefines.h).
- `PetStable` (Player-owned 5+200+unslotted container).
- `CharmInfo` action bar + react/command state machine.
- The full PetHandler.cpp dispatcher (810 lines, ~14 handlers).
- All 18+ packet structs in `PetPackets.{h,cpp}` (482 lines combined).
- The 7 SQL tables and ~25 prepared statements.
- Stable handlers (`HandleRequestStabledPets`, `HandleStablePet`, `HandleStableSwapPet`, `HandleBuyStableSlot`).
- `PetAI` (in `crates/wow-ai/src/`).
- Pet talent tree (3.4.3 hunter pet talents — Cunning/Tenacity/Ferocity trees).
- Pet specialization (`Pet::SetSpecialization` + `LearnSpecializationSpells`).
- Pet XP / level-up loop (`GivePetXP` / `GivePetLevel` / `SynchronizeLevelWithOwner`).
- Pet auras (the owner-cast `pet_aura` Trinity DB cache + `CastPetAuras(current)`).
- Diet/feed handling (`HaveInDiet`).
- Save/load roundtrip (`SavePetToDB` / `LoadPetFromDB`'s 6-query async holder).
- Action-bar text serialization (`GenerateActionBarData` / `LoadPetActionBar`).
- Pet name validation (`CheckPetName`, `CheckDeclinedNames`, reserved-name list).
- Hunter-pet rename one-shot lock (`UNIT_PET_FLAG_CAN_BE_RENAMED`).
- Group update flag broadcasting (`SetGroupUpdateFlag` + `GROUP_UPDATE_PET_FULL`).
- Spell history (cooldowns + charges) for pets — `SpellHistory<Pet>::LoadFromDB`/`SaveToDB`.
- BG/Arena unsummon hooks (`Pet::Remove(PET_SAVE_NOT_IN_SLOT)` on enter, restore on exit).
- Owner-mount disable (`Player::DisablePetControlsOnMount(REACT_PASSIVE, COMMAND_FOLLOW)`).
- Critter dismiss (`HandleDismissCritter`) — separate from BattlePets.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- A 3.4.3 client logging in as a Hunter or Warlock with a saved pet will not see its pet appear at all (no spawn flow), and the action bar will be blank because `SMSG_PET_SPELLS_MESSAGE` is never sent.
- Casting a pet-summon spell (Hunter "Call Pet" 883, Warlock "Summon Imp" etc.) will currently produce nothing on the server — `Spell::EffectSummonPet` does not exist in Rust either.
- Pet-related opcodes from the client are currently silently dropped (no match arm exists in the dispatcher for some of them — verify against `crates/wow-handler/src/lib.rs`). For 3.4.3, this means the client doesn't error — it just hangs waiting for `SMSG_PET_SPELLS_MESSAGE` after summon.
- The C# reference at `/home/server/woltk-server-core/Source/` may have an incomplete pet implementation too — confirm before assuming the C# is authoritative for any specific behavior.

**Tests existing:**
- 0.

---

## 9. Migration sub-tasks

Numera los items para poder referenciarlos desde `MIGRATION_ROADMAP.md` sección 5.

Complejidad: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, splitear).

- [ ] **#PETS.1** Define enums in `crates/wow-constants/src/pets.rs`: `PetType` (u8), `PetSaveMode` (i16), `PetStableFlags`, `PetSpellState`, `PetSpellType`, `PetActionFeedback`, `PetTalk`, `PetTameResult`, `StableResult`, `ActiveStates`, `ReactStates`, `CommandStates`. Plus constants `MAX_ACTIVE_PETS=5`, `MAX_PET_STABLES=200`, `MAX_UNIT_ACTION_BAR_INDEX=10`, `ACTION_BAR_INDEX_PET_SPELL_START=3`, `..._END=7`, `PET_FOLLOW_DIST=1.0`, `PET_FOLLOW_ANGLE=π`, `HAPPINESS_LEVEL_SIZE=333000`, `CALL_PET_SPELL_ID=883`, `PET_SUMMONING_DISORIENTATION=32752`, `UNSLOTTED_PET_INDEX_MASK=0x80000000` (M)
- [ ] **#PETS.2** Define `UnitActionBarEntry` packed struct with `MAKE_UNIT_ACTION_BUTTON(action: u32, type: ActiveStates) -> u32 = action | ((type as u32) << 23)` — bit-exact to C++ `UNIT_ACTION_BUTTON_ACTION` mask `0x007FFFFF` and `UNIT_ACTION_BUTTON_TYPE` shift 23 (L)
- [ ] **#PETS.3** Define `CharmInfo` in `crates/wow-world/src/charm_info.rs` with `pet_action_bar: [UnitActionBarEntry; 10]`, `_command_state`, `_old_react_state`, `_pet_number`, `_is_command_attack/_follow/_at_stay/_following/_returning`, `_stay_position: Option<Position>`, plus the InitPossess/Charm/PetActionBar / LoadPetActionBar / BuildActionBar / SetSpellAutocast / ToggleCreatureAutocast / SetActionBar methods (H)
- [ ] **#PETS.4** Define `PetSpell { active: ActiveStates, state: PetSpellState, type_: PetSpellType }` and aliases `PetSpellMap = HashMap<u32, PetSpell>`, `AutoSpellList = Vec<u32>` (L)
- [ ] **#PETS.5** Define `PetStable::PetInfo` struct + `PetStable` with `current_pet_index: Option<u32>` (high bit = unslotted), `active_pets: [Option<PetInfo>; 5]`, `stabled_pets: [Option<PetInfo>; 200]`, `unslotted_pets: Vec<PetInfo>`. Implement `get_current_pet`, `get_current_active_pet_index`, `get_current_unslotted_pet_index`, `set_current_active_pet_index`, `set_current_unslotted_pet_index` (M)
- [ ] **#PETS.6** Define `Pet` entity in `crates/wow-world/src/pets/pet.rs` extending the Creature/Guardian chain: `pet_type`, `duration`, `loading`, `focus_regen_timer`, `group_update_mask`, `pet_specialization`, `declined_name: Option<DeclinedName>`, `spells: PetSpellMap`, `autospells: AutoSpellList` (H)
- [ ] **#PETS.7** Implement `Pet::create(guid_low, map, entry, pet_number)`, `create_base_at_creature(creature)`, `create_base_at_creature_info(cinfo, owner)`, `create_base_at_tamed(cinfo, map)` (H — depends on Creature/Guardian Rust scaffold)
- [ ] **#PETS.8** SQL migrations for the 7 tables (`character_pet`, `character_pet_declinedname`, `pet_aura`, `pet_aura_effect`, `pet_spell`, `pet_spell_cooldown`, `pet_spell_charges`) in `crates/wow-database/migrations/character/` — exact column types and indexes match (M)
- [ ] **#PETS.9** Prepared-statement entries for the ~25 `CHAR_*_PET*` statements in `crates/wow-database/src/statements/character.rs` (M)
- [ ] **#PETS.10** Implement `Pet::load_pet_from_db(owner, pet_entry, pet_number, current, forced_slot) -> bool` — the slot resolution via `get_load_pet_info(stable, ...)`, the async holder query (6 SELECTs in parallel via `tokio::join!`), the entity creation, the after-complete callback that loads auras + spells + spell history + sets specialization + emits `PetSpellInitialize` (XL — split as: get_load_pet_info; query holder; entity scaffold; auras+spells; spec resolution; callback wiring)
- [ ] **#PETS.11** Implement `Pet::save_pet_to_db(mode)` — mode-aware (PET_SAVE_AS_CURRENT → resolve to active slot index, PET_SAVE_AS_DELETED → cascade delete via `delete_from_db`, otherwise INS with the 17-column INSERT) wrapped in a single transaction together with `_save_auras` + `_save_spells` + `SpellHistory<Pet>::save_to_db` (H)
- [ ] **#PETS.12** Implement `Pet::delete_from_db(pet_number)` — cascade across all 7 tables in a single transaction, exact statement order matching C++ (L)
- [ ] **#PETS.13** Implement `Pet::fill_pet_info(petInfo, forced_react_state)` and `Pet::generate_action_bar_data() -> String` / `CharmInfo::load_pet_action_bar(text)` — the text format is space-separated `"<active_state> <action>"` × 10, parser must match TC's `GenerateActionBarData` byte-for-byte (M)
- [ ] **#PETS.14** Implement `Pet::add_spell` / `learn_spell` / `learn_spells` / `learn_spell_high_rank` / `unlearn_spell` / `unlearn_spells` / `remove_spell` / `cleanup_action_bar` — including `SMSG_PET_LEARNED_SPELLS` / `SMSG_PET_UNLEARNED_SPELLS` emission for state == NEW/REMOVED (H)
- [ ] **#PETS.15** Implement `Pet::toggle_autocast(spell_info, apply)` and `Pet::has_spell(spell)` override (L)
- [ ] **#PETS.16** Implement `Pet::init_pet_create_spells()` for SUMMON_PET — read `creature_template_addon.spell1..8` and add each as `(ACT_DECIDE, NEW, NORMAL)` (M)
- [ ] **#PETS.17** Implement `Pet::learn_pet_passives()` for HUNTER_PET — read CreatureFamilyStore + family-spell list, add as `(ACT_PASSIVE, NEW, FAMILY)` (M)
- [ ] **#PETS.18** Implement `Pet::cast_pet_auras(current)` + `Pet::cast_pet_aura(petAura)` + `Pet::is_pet_aura(aura)` — backed by a Trinity-style world DB `pet_aura` cache (separate from character DB `pet_aura` aura-instance table) (M)
- [ ] **#PETS.19** Implement `Pet::set_specialization(specId)` + `learn_specialization_spells` + `remove_specialization_spells(clear_action_bar)` — depends on ChrSpecialization DB2 store integration (M)
- [ ] **#PETS.20** Implement `Pet::give_pet_xp(xp)` + `give_pet_level(level)` + `synchronize_level_with_owner` — including `SMSG_PET_LEVEL_UP` + clamping HUNTER_PET level to `[owner.level - 5, owner.level]` (H)
- [ ] **#PETS.21** Implement `Pet::have_in_diet(item_template) -> bool` — bitmask `creature_family.pet_food_mask & (1 << (item.food_type - 1))` (L — gotcha: subtract 1 from food_type because client encodes it 1-based)
- [ ] **#PETS.22** Implement `Pet::set_death_state(state)` override — hunter pet JUST_DIED keeps the slot but drops from world; summon pet calls `Player::remove_pet(PET_SAVE_NOT_IN_SLOT)`. Also `Pet::update(diff)` (focus regen, duration tick, save timer) (H)
- [ ] **#PETS.23** Define wire packet structs in `crates/wow-packet/src/packets/pets.rs`: `DismissCritter`, `RequestPetInfo`, `PetAbandon`, `PetStopAttack`, `PetSpellAutocast`, `PetSpells` (with `PetSpellCooldown` + `PetSpellHistory` sub-structs and the `[i32; 10]` ActionButtons array), `PetStableResult`, `PetLearnedSpells`, `PetUnlearnedSpells`, `PetRenameData` + `PetNameInvalid` + `PetRename`, `PetAction` (with `TaggedPosition<XYZ>`), `PetSetAction`, `PetCancelAura`, `SetPetSpecialization`, `PetActionFeedback`, `PetActionSound`, `PetTameFailure`, `PetMode` (XL — split as: client packets, server packets, action-bar packed data) (XL)
- [ ] **#PETS.24** NPC stable wire packets in `crates/wow-packet/src/packets/npc.rs`: `RequestStabledPets`, `PetStableList` (with the inner `{ slot, pet_number, creature_id, display_id, level, flags, name }` repeating block), `StablePet`, `StableSwapPet`, `BuyStableSlot` (M)
- [ ] **#PETS.25** Implement `WorldSession::handle_pet_action(packet)` + `handle_pet_action_helper(pet, ...)` — port the entire switch on `flag` (ACT_COMMAND/REACTION/DISABLED/PASSIVE/ENABLED) and the inner switch on `spellid` for COMMAND_*. Includes `IsCommandAttack/Follow/AtStay/Following/Returning` flag bookkeeping on `CharmInfo` (XL — split as: ACT_COMMAND switch; ACT_REACTION; ACT_*ENABLED spell cast; charmed-player edge case)
- [ ] **#PETS.26** Implement `handle_pet_stop_attack`, `handle_pet_set_action`, `handle_pet_cancel_aura`, `handle_pet_spell_autocast`, `handle_request_pet_info`, `handle_dismiss_critter` (M)
- [ ] **#PETS.27** Implement `handle_pet_rename` + `send_pet_name_invalid` — name validation, declined-names check, reserved-name check, transactional INS/DEL/UPD for declined names + UPD for `character_pet.name` + clear `UNIT_PET_FLAG_CAN_BE_RENAMED` (H)
- [ ] **#PETS.28** Implement `handle_pet_abandon` — only HUNTER_PET, calls `Player::remove_pet(pet, PET_SAVE_AS_DELETED)` (L)
- [ ] **#PETS.29** Implement `handle_pet_cast_spell` — port the long path in `PetHandler.cpp` `HandlePetCastSpellOpcode` (Spell ctor on pet, CheckPetCast, auto-turn-to-target on UNIT_NOT_INFRONT) (H)
- [ ] **#PETS.30** Implement `handle_query_pet_name` + `send_query_pet_name_response` (M)
- [ ] **#PETS.31** Implement `handle_pet_learn_talent` + `handle_learn_preview_talents_pet` (XL — depends on Talent DB2 + per-tree talent point pool resolution) (XL)
- [ ] **#PETS.32** Implement stable handlers (`handle_request_stabled_pets`, `handle_stable_pet`, `handle_stable_swap_pet`, `handle_buy_stable_slot`) + `send_pet_stable_result(result)` + `send_pet_stable_list(stable_master_guid)` (H)
- [ ] **#PETS.33** Implement `CheckStableMaster(npc_guid) -> bool` + the `UNIT_NPC_FLAG_STABLEMASTER` constant (L)
- [ ] **#PETS.34** Implement `Player::summon_pet(entry, x, y, z, o, type, duration, ...)` and `Player::remove_pet(pet, mode)` (XL — depends on player owning a `Box<PetStable>` and `current_pet: Option<ObjectGuid>`) (XL)
- [ ] **#PETS.35** Implement `PetAI` in `crates/wow-ai/src/pet_ai.rs` — `_attack_start(target)` (force target switch on COMMAND_ATTACK), `update_ai(diff)` (autocast trigger via `m_autospells`, owner-distance leash, COMMAND_FOLLOW motion), `killed_unit` hook (XL)
- [ ] **#PETS.36** Wire pet-load into character login flow in `handlers/character.rs::handle_player_login` — query `character_pet` for current slot (slot 0 if hunter, slot -1 if warlock-summon), call `Pet::load_pet_from_db(player, 0, current_active_pet_number, true)`, send `PetSpellInitialize` after load (H)
- [ ] **#PETS.37** Wire pet-save into the per-session save loop (every `CONFIG_PET_SAVE_TIMER` ms, currently 60s in C++) — in `Pet::update(diff)` (M)
- [ ] **#PETS.38** Wire BG/Arena unsummon: on map enter, if map type is BG → `Pet::remove(PET_SAVE_NOT_IN_SLOT)` for warlock; on exit, restore (M)
- [ ] **#PETS.39** Wire owner-mount disable: `Player::disable_pet_controls_on_mount(REACT_PASSIVE, COMMAND_FOLLOW)` invoked from `Player::Mount` (L)
- [ ] **#PETS.40** Add config keys `CONFIG_PET_SAVE_TIMER` (60000ms), `CONFIG_RATE_PET_XP_KILL` (1.0), `CONFIG_RATE_PET_TALENT_POINTS` (1.0), `CONFIG_CHARGE_FOR_STABLE_SLOTS` (10 gold), `CONFIG_MAX_PET_STABLES` (200) (L)
- [ ] **#PETS.41** Documentation cross-link: `pets.md` ↔ `entities.md` (Creature/Guardian inheritance) ↔ `ai.md` (PetAI) ↔ `spells.md` (SummonPet / TameCreature spell effects) ↔ `groups.md` (GROUP_UPDATE_PET_FULL flag) (L)

---

## 10. Regression tests to write

- [ ] Test: `PetStable::get_load_pet_info` — by petnumber: finds in active, stable, and unslotted lists. By slot: returns active or stable entry. By petEntry: searches unslotted only. Default: returns first active or first unslotted, never stabled.
- [ ] Test: `UnitActionBarEntry::pack(action=1024, type=ACT_COMMAND)` round-trips through `unpack` byte-for-byte; `MAKE_UNIT_ACTION_BUTTON` matches C++.
- [ ] Test: `Pet::generate_action_bar_data` produces the same string as a known TC fixture for a HUNTER_PET with [Growl, Bite, Dash, Cower] in slots 3-6.
- [ ] Test: `CharmInfo::load_pet_action_bar(string)` is the inverse of `generate_action_bar_data` — corrupted strings (truncated, non-numeric) fall back to default `InitPetActionBar`.
- [ ] Test: HUNTER_PET dies → `Pet::set_death_state(JUST_DIED)` removes from world but keeps slot 0 in stable; SUMMON_PET dies → calls `Player::remove_pet(PET_SAVE_NOT_IN_SLOT)`.
- [ ] Test: `Pet::synchronize_level_with_owner` clamps hunter pet to `[owner.level - 5, owner.level]` when owner.level >= 5; clamps to `[1, owner.level]` otherwise.
- [ ] Test: `Pet::give_pet_xp(xp)` triggers level-up at the right XP threshold (the `MaxXpForLevel` curve from `pet_levelstats`).
- [ ] Test: `HandlePetAction(COMMAND_STAY)` clears motion master, sets `is_at_stay=true` and saves stay position; `COMMAND_FOLLOW` calls `MoveFollow` with `PET_FOLLOW_DIST`; `COMMAND_ATTACK` calls `PetAI::_AttackStart` if pet is creature; `COMMAND_ABANDON` for HUNTER_PET → `RemovePet(PET_SAVE_AS_DELETED)`; for SUMMON_PET → `RemovePet(PET_SAVE_NOT_IN_SLOT)`.
- [ ] Test: `HandlePetAction(REACT_PASSIVE)` calls `AttackStop` (the fallthrough); REACT_DEFENSIVE/AGGRESSIVE just sets state.
- [ ] Test: `HandlePetAction(ACT_ENABLED, spell_id)` rejects if pet doesn't know the spell or spell is passive; rejects if spell targets `TARGET_UNIT_SRC_AREA_ENEMY` etc.; on `SPELL_FAILED_UNIT_NOT_INFRONT` and not possessed/vehicle, server auto-turns the pet to face target.
- [ ] Test: `HandlePetRename` rejects when pet is not HUNTER_PET; rejects when `UNIT_PET_FLAG_CAN_BE_RENAMED` already cleared; rejects on reserved names; persists name + clears flag on success.
- [ ] Test: `HandlePetSetAction(slot=4, spell_id=X, type=ACT_ENABLED)` toggles autocast on for the spell, and propagates to all controlled pets of same entry.
- [ ] Test: `Pet::save_pet_to_db(PET_SAVE_AS_CURRENT)` resolves to the active slot index (0..4); preserves slot value in `character_pet.slot`.
- [ ] Test: `Pet::save_pet_to_db(PET_SAVE_AS_DELETED)` removes auras AND triggers `delete_from_db(pet_number)` cascading across all 7 tables.
- [ ] Test: `Pet::load_pet_from_db` of a hunter pet with saved auras restores `Aura.remainTime` decremented by `now - last_save_time`.
- [ ] Test: `HandleStablePet` moves current active pet (slot 0) to first empty stable slot; `HandleStableSwapPet` swaps current with stabled-by-petnumber and respects `CanTameExoticPets` for exotic pets.
- [ ] Test: `HandleBuyStableSlot` charges `CONFIG_CHARGE_FOR_STABLE_SLOTS × current_count`; reply `BUY_SLOT_SUCCESS`; max stable slots is 200 (`MAX_PET_STABLES`).
- [ ] Test: Wire-format byte sequences for `SMSG_PET_SPELLS_MESSAGE` and `SMSG_PET_MODE` match recorded 3.4.3 captures.

---

## 11. Notes / gotchas

- **Pet identity is `pet_number`, not creature `entry`.** Two hunter pets of the same creature template get distinct pet_numbers — that's the primary key in `character_pet.id` and the ObjectGuid low part for the spawned `Pet`. Don't conflate `entry` (the species) with `pet_number` (the individual).
- **`character_pet.slot` is signed `SMALLINT`** because PET_SAVE_AS_CURRENT=-3 / PET_SAVE_AS_DELETED=-2 / PET_SAVE_NOT_IN_SLOT=-1 are encoded literally. Active slots 0-4, stable slots 5-205.
- **`PetStable::CurrentPetIndex` uses high-bit mask `0x80000000`** to distinguish unslotted (mask set) from active-slot index (mask clear). Mirror this exactly — TC's `GetCurrentActivePetIndex` returns `nullopt` if the high bit is set; `GetCurrentUnslottedPetIndex` returns `Some(index & ~mask)` if set.
- **The action-bar `packedData` bit layout is `(action: bits 0-22) | (type: bits 23-30 = bit 24-31 in C++ macro)`.** TC's macro is `(uint32(A) | (uint32(T) << 23))`, with mask `0x007FFFFF` for action and `0xFF000000 >> 23` (an 8-bit type extracted by `>> 23`). Note the asymmetry: 23 bits of action (0..0x7FFFFF) but the type is the high byte. The nominal 24-bit boundary in some TC docs is misleading — bit 23 is part of type, not action.
- **Hunter pet level clamping is one-sided in some cases.** When the owner gains a level, pet syncs up. When owner is dispelled below pet level (e.g. level-drain mob), pet does NOT sync down — only on summon does the clamp apply.
- **`HandlePetAction` for `COMMAND_ATTACK`**: when the player has multiple controlled pets (e.g. shaman with multiple totems acting as guardians), the helper iterates `m_Controlled` filtered by same `GetEntry()` and same alive status — make sure the Rust port reproduces that behavior (one click attacks all of same entry). In particular `if (GetPlayer()->m_Controlled.size() == 1)` is the fast path; otherwise the loop builds the controlled-pets vector first to avoid mutation-during-iteration when pets get dismissed by the very command.
- **`Pet::SavePetToDB(PET_SAVE_AS_CURRENT)` corner case**: if the player has a temporary unsummoned pet number (e.g. after Lich-King-style mind-control returns), and it differs from the current pet's number, `mode` is mutated to `PET_SAVE_NOT_IN_SLOT` for warlock pets and the save is skipped entirely for hunter pets. This is **easy to miss** and causes "lost pet" bugs.
- **Action bar serialization format `abdata` is fragile.** TC writes 10 entries as `"<u32_active_state> <u32_action> "` (note trailing space) joined directly. Parsing strips trailing whitespace and splits by space — preserving the exact format is necessary for TC clients to show an unchanged action bar after relog.
- **`UNIT_PET_FLAG_CAN_BE_RENAMED` is set on initial tame/summon and cleared after first rename.** It's serialized in `character_pet.renamed` (inverted: 0 = renameable, 1 = locked). Reload restores via `ReplaceAllPetFlags(petInfo->WasRenamed ? CAN_BE_ABANDONED : (CAN_BE_RENAMED | CAN_BE_ABANDONED))`.
- **`Pet::Update` calls `RegeneratePower` for focus** every `PET_FOCUS_REGEN_INTERVAL` (4 seconds in 3.4.3 — verify the exact constant). Hunter pets use `POWER_FOCUS`; warlock pets use `POWER_MANA` and inherit player regen rules.
- **`HandlePetActionHelper`'s `ACT_DISABLED` case is intentionally identical to `ACT_PASSIVE/ENABLED` for spell casting** — the difference is purely cosmetic on the action bar (greyed out vs active). Don't try to optimize that into separate branches.
- **Charm vs summon distinction**: charmed creatures use `Unit::SetCharmedBy` which moves the creature into the player's `m_Controlled` list, populates `CharmInfo` from a snapshot of the creature's spells, and does NOT use `m_petType`. They're handled by `HandlePetActionHelper` via the same opcodes (action bar UI is shared) but `pet->IsPet()` returns false for charmed creatures — many subroutines (RemovePet, FillPetInfo, save) skip them. Mirror that test exactly.
- **`PET_SUMMONING_DISORIENTATION = 32752`** is the 5-second daze applied to a freshly summoned pet so it can't immediately attack. Always cast on `HandleSpellLearnSpell` Tame path and `Player::SummonPet`.
- **Specialization (3.4.3 reintroduction) is not the same as Cataclysm specs.** In WoLK / 3.4.3 Classic, hunter pets can be Cunning/Tenacity/Ferocity which are stored in `character_pet.specialization` and resolved via `ChrSpecialization.db2`. The override aura `SPELL_AURA_OVERRIDE_PET_SPECS` (used by some boss mechanics) is checked at `LoadPetFromDB`.
- **Pet talent reset cost** scales with previous-reset count and player level — the formula lives in `Player::ResetTalentsCost` and is shared with the player talent reset, but it's a separate cost track (`Player::m_resetTalentsCost`).
- **`HandleDismissCritter` can dismiss a battle pet** — see `_player->GetSummonedBattlePetGUID() == pet->GetBattlePetCompanionGUID()` clearing `SetBattlePetData(nullptr)`. For RustyCore (where battle pets are mostly N/A — see `battlepets.md`), this branch is dead code but the critter dismiss itself (`pet->ToTempSummon()->UnSummon()`) must still work for non-battle critters.
- **Async query holder for `LoadPetFromDB`**: TC uses 6 prepared SELECTs run in parallel via `CharacterDatabase.DelayQueryHolder`. The Rust port can use `tokio::join!` on 6 sqlx queries but **must** preserve the after-complete ordering: if `session->GetPlayer() != owner || owner->GetPet() != this`, abort (player relogged or pet replaced mid-load).
- **`character_pet.PetType` uses the 3.4.3 enum that has 4 entries reserved (`MAX_PET_TYPE=4`) but only 0/1 are used in current code.** Don't assume only-2-values when porting; future-compat may add `GUARDIAN_PET=2`, etc.
- **C# reference quality**: the C# server at `/home/server/woltk-server-core/Source/` may have an incomplete pet implementation (the Rust port references it). When in doubt, the Trinity C++ is canonical for protocol/mechanics.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class Pet : public Guardian` | `pub struct Pet { creature: Creature, pet_type: PetType, duration: i32, loading: bool, focus_regen_timer: u32, group_update_mask: u32, pet_specialization: u16, declined_name: Option<DeclinedName>, spells: PetSpellMap, autospells: AutoSpellList }` | No inheritance — composition with `Creature`. Methods that override Creature/Unit virtuals become trait impls on the wrapper |
| `class PetStable` | `pub struct PetStable { pub current_pet_index: Option<u32>, pub active_pets: [Option<PetInfo>; 5], pub stabled_pets: [Option<PetInfo>; 200], pub unslotted_pets: Vec<PetInfo> }` | Direct port; high-bit mask kept as `const UNSLOTTED_PET_INDEX_MASK: u32 = 0x80000000` |
| `PetStable::PetInfo` | `pub struct PetInfo { pub name: String, pub action_bar: String, pub pet_number: u32, pub creature_id: u32, pub display_id: u32, pub experience: u32, pub health: u32, pub mana: u32, pub last_save_time: u32, pub created_by_spell_id: u32, pub specialization_id: u16, pub level: u8, pub react_state: ReactStates, pub type_: PetType, pub was_renamed: bool }` | Default init via `Default` impl |
| `enum PetType : u8` | `#[repr(u8)] pub enum PetType { SummonPet = 0, HunterPet = 1, MaxPetType = 4 }` | Wire-stable |
| `enum PetSaveMode : i16` | `#[repr(i16)] pub enum PetSaveMode { AsDeleted = -2, AsCurrent = -3, FirstActiveSlot = 0, ..., NotInSlot = -1 }` plus const-fns `is_active_pet_slot(slot) -> bool` and `is_stabled_pet_slot(slot) -> bool` | Note negative discriminants — Rust supports them on `i16`-typed enums |
| `enum class PetActionFeedback : u8` | `#[repr(u8)] pub enum PetActionFeedback { None = 0, Dead = 1, NoTarget = 2, InvalidTarget = 3, NoPath = 4 }` | — |
| `enum class PetTameResult : u8` | 15-variant enum, derive `Copy` | — |
| `enum class StableResult : u8` | 7-variant enum | — |
| `enum ActiveStates : u8 { ACT_PASSIVE=0x01, ACT_DISABLED=0x81, ACT_ENABLED=0xC1, ACT_COMMAND=0x07, ACT_REACTION=0x06, ACT_DECIDE=0x00 }` | `#[repr(u8)] pub enum ActiveStates { Passive=0x01, Disabled=0x81, Enabled=0xC1, Command=0x07, Reaction=0x06, Decide=0x00 }` | Wire-stable |
| `enum ReactStates : u8` | `#[repr(u8)] pub enum ReactStates { Passive=0, Defensive=1, Aggressive=2, Assist=3 }` | Wire-stable |
| `enum CommandStates : u8` | `#[repr(u8)] pub enum CommandStates { Stay=0, Follow=1, Attack=2, Abandon=3, MoveTo=4 }` | Wire-stable |
| `MAKE_UNIT_ACTION_BUTTON(A,T)` macro | `pub const fn make_action_button(action: u32, type_: ActiveStates) -> u32 { (action & 0x007F_FFFF) | ((type_ as u32) << 23) }` | Bit-exact; pair with `extract_action(packed) -> u32` and `extract_type(packed) -> ActiveStates` |
| `class CharmInfo` | `pub struct CharmInfo { pet_action_bar: [UnitActionBarEntry; 10], _command_state: CommandStates, _old_react_state: ReactStates, _pet_number: u32, _is_command_attack: bool, _is_command_follow: bool, _is_at_stay: bool, _is_following: bool, _is_returning: bool, _stay_position: Option<Position>, _charm_spells: [UnitActionBarEntry; 4] }` | Plain struct with method bag |
| `Pet::LoadPetFromDB(...)` | `async fn load_pet_from_db(player: &mut Player, pet_entry: u32, pet_number: u32, current: bool, forced_slot: Option<PetSaveMode>) -> Result<bool>` | Use `tokio::join!` for the 6-query holder |
| `Pet::SavePetToDB(mode)` | `async fn save_pet_to_db(&mut self, mode: PetSaveMode) -> Result<()>` | Wrap in `let mut tx = char_db.begin().await?` |
| `Pet::DeleteFromDB(petNumber)` (static) | `async fn delete_from_db(char_db: &Pool, pet_number: u32) -> Result<()>` | Free function in the `pets` module |
| `Pet::Update(diff)` (override) | `fn update(&mut self, diff_ms: u32)` | Called from session tick |
| `Pet::setDeathState(s)` (override) | `fn set_death_state(&mut self, state: DeathState)` | — |
| `Pet::ToggleAutocast(spellInfo, apply)` | `fn toggle_autocast(&mut self, spell_info: &SpellInfo, apply: bool)` | — |
| `Pet::HasSpell(spell)` (override) | `fn has_spell(&self, spell: u32) -> bool` | Trait-method on the `Unit` trait |
| `WorldSession::HandlePetAction(packet)` | `pub async fn handle_pet_action(session: &mut WorldSession, packet: PetAction) -> Result<()>` in `crates/wow-world/src/handlers/pets.rs` | Match other handlers |
| `WorldSession::HandlePetActionHelper(...)` | `async fn handle_pet_action_helper(session: &mut WorldSession, pet: ObjectGuid, guid1: ObjectGuid, spellid: u32, flag: ActiveStates, guid2: ObjectGuid, pos: Position) -> Result<()>` | Private free fn next to `handle_pet_action` |
| `WorldPackets::Pet::*` | `crates/wow-packet/src/packets/pets.rs` structs implementing `ServerPacket`/`ClientPacket` | Pattern from `quests.rs` etc |
| `unordered_map<u32, PetSpell>` | `HashMap<u32, PetSpell>` (or `dashmap::DashMap` if accessed concurrently) | Pet is single-owner — `HashMap` is fine |
| `vector<u32>` (autospells) | `Vec<u32>` | — |
| `UnitActionBarEntry::packedData` | `pub struct UnitActionBarEntry { pub packed_data: u32 }` with helper methods | Internal field; expose `get_action()/get_type()/set_action_and_type` |
| `CharacterDatabasePreparedStatement* CHAR_INS_PET` | `crate::statements::CharStatement::InsPet` + `bind_*` calls | sqlx prepared statements; use `Pet::INSERT_COLUMN_COUNT = 17` |
| `CharacterDatabaseTransaction trans` | `let mut tx = self.char_db.begin().await?` … `tx.commit().await?` | — |
| `PetLoadQueryHolder` (subclass of `CharacterDatabaseQueryHolder`) | `struct PetLoadResult { declined_names: Option<DeclinedNameRow>, auras: Vec<AuraRow>, aura_effects: Vec<AuraEffectRow>, spells: Vec<SpellRow>, cooldowns: Vec<CooldownRow>, charges: Vec<ChargeRow> }` produced by `tokio::join!` | Eliminate the holder pattern entirely; just a struct of joined results |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.
