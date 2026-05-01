# Migration: Scripts (content scripts directory)

> **C++ canonical path:** `src/server/scripts/` (the content-side script library: bosses, instances, quests, NPCs, gossip, spells, area triggers, GM commands, world events, holiday events, OutdoorPvP, Battlefield WG, pet AIs, custom scripts).
> **Rust target crate(s):** `crates/wow-scripts/` (content) + lateral crates (`wow-spell` for `spell_*` scripts, `wow-pvp`/`wow-battleground` for OutdoorPvP & Battlefield, `wow-pet` for Pet scripts, etc.). The framework that scripts plug into lives in `crates/wow-script/` and is covered by `scripting.md`.
> **Layer:** L8 (content layer — depends on **everything** below it: scripting framework L7, instance/BG/OPvP L7, spells L5, AI L5, conditions L7, loot L6, quests L6, all entity types L4, …).
> **Status:** ❌ not started — `crates/wow-scripts/src/lib.rs` is **0 bytes**. **No** boss AI, **no** instance script, **no** spell script, **no** GM command, **no** holiday event, **no** OutdoorPvP, **no** Wintergrasp, **no** quest helper exists in Rust. This is the largest single migration surface in the entire project: ~725 `.cpp` files and ~294,137 lines (it dwarfs every other subsystem in line count).
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

`src/server/scripts/` holds the **content** layer of TrinityCore: everything that makes the game *be a game* rather than an inert simulation. Every boss encounter, every instance state machine, every WoLK quest helper NPC, every gossip flowchart, every holiday vendor swap, every `.gm fly` chat command, every Wintergrasp tower interaction, every special spell script (death-grip, kill command, polymorph trigger, etc.) is implemented here. Files derive from base classes defined in `src/server/game/Scripting/ScriptMgr.h` (covered by `scripting.md`); each file ends with an `AddSC_*` registration function. The aggregate `ScriptLoader.cpp.in.cmake` template wires every `AddSC_*` declaration into a single `AddScripts()` entry point that `ScriptMgr::Initialize()` invokes at startup.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/src/server/scripts/`. Sizes are in `.cpp` files (.h are tiny instance-data accessors).

| Subdirectory | `.cpp` count | Purpose |
|---|---|---|
| `Battlefield/` | 2 (+1 `.h`, +1 loader) | `BattlefieldWG` — Wintergrasp implementation. The only content-side Battlefield in WoLK. |
| `Commands/` | 42 + loader | Every `.gm`, `.cheat`, `.tele`, `.npc`, `.account`, `.ban`, `.gobject`, etc. GM chat command (one file per `cs_*.cpp` topic). |
| `Custom/` | 0 + loader | Empty — placeholder for site-local custom scripts. |
| `EasternKingdoms/` | 173 | Old-world (1.x) and BC zones in EK + their dungeons/raids: Karazhan, Sunwell Plateau, ZulAman, ZulGurub, Magisters' Terrace, Scarlet Monastery/Enclave, Stratholme, Scholomance, Shadowfang Keep, Deadmines, Gnomeregan, The Stockade, Sunken Temple, Uldaman, Blackrock Mountain (BRD/BWL/MC/UBRS/LBRS), 16 zone files. |
| `Events/` | 13 (+1 loader) | The 12 holiday/world events scripted on the content side: brewfest, childrens_week, darkmoon_faire, fireworks_show, hallows_end, love_is_in_the_air, lunar_festival, midsummer, operation_gnomeregan, pilgrims_bounty, winter_veil, zalazane_fall. (See `events.md` for the GameEventMgr scheduler that drives them.) |
| `Kalimdor/` | 173 | Old-world Kalimdor + BC: Onyxia's Lair, Temple/Ruins of Ahn'Qiraj, Dire Maul, Maraudon, Wailing Caverns, Razorfen Downs/Kraul, Blackfathom Deeps, Ragefire Chasm, ZulFarrak, Caverns of Time (Old Hillsbrad, Black Morass, CoS, Hyjal, Culling, Drak'Tharon, Battle for Mt. Hyjal), 18 zone files. |
| `Maelstrom/` | 2 + 1 subdir | Cataclysm-leakage (Stonecore, Deepholm, Kezan). Almost entirely vestigial in WoLK. |
| `Northrend/` | 169 | The WoLK content. **The single most important directory** for this expansion: Icecrown Citadel, Ulduar, Trial of the Crusader / Champion, Naxxramas, all 5-mans (Azjol-Nerub, Ahnkahet, Drak'Tharon Keep, Frozen Halls, Gundrak, Nexus, Eye of Eternity, Oculus, Utgarde Keep/Pinnacle, Violet Hold), Vault of Archavon, Chamber of Aspects, Onyxia (re-tuned), Isle of Conquest BG, plus 10 Northrend zone files (Borean Tundra, Dalaran, Dragonblight, Grizzly Hills, Howling Fjord, Icecrown, Sholazar Basin, Storm Peaks, Wintergrasp, Zul'Drak). |
| `OutdoorPvP/` | 5 (HP, NA, SI, TF, ZM) + loader | The five Outland outdoor PvP zones (Hellfire Peninsula, Nagrand, Silithus, Terokkar Forest, Zangarmarsh). |
| `Outland/` | 91 | BC content: Black Temple, Coilfang Reservoir (SP/SH/SV/UB), Tempest Keep (Eye/Mech/Bot/Arc), Hellfire Citadel (HFR/SH/BF/MT), Gruul's Lair, Auchindoun (AC/SH/MT/SE), 6 zone files, two Outland world bosses (Doomlord Kazzak, Doomwalker). |
| `Pet/` | 6 (+ loader) | Class-specific pet AI overrides: DK ghoul/army-of-the-dead, hunter pets, mage water elemental, priest shadowfiend, shaman elementals, generic guardians. |
| `Spells/` | 15 (+ loader) | The big one: per-class scripted spells (`spell_dk.cpp`, `spell_druid.cpp`, …, `spell_warrior.cpp`), plus `spell_generic.cpp`, `spell_item.cpp`, `spell_quest.cpp`, `spell_pet.cpp`. Each file packs hundreds of `class spell_xxx : public SpellScript` / `AuraScript` definitions implementing per-spell scripted behavior (proc handlers, custom target selection, conditional damage, etc.). |
| `World/` | 15 | Cross-zone scripts: `npcs_special.cpp` (huge — innkeepers, banker, guards, helpers), `npc_professions.cpp`, `npc_guard.cpp`, `boss_emerald_dragons.cpp`, `go_scripts.cpp` (generic gameobject use), `item_scripts.cpp`, `areatrigger_scripts.cpp`, `achievement_scripts.cpp`, `chat_log.cpp`, `action_ip_logger.cpp`, `boosted_xp.cpp`, `duel_reset.cpp`, `conversation_scripts.cpp`, `scene_scripts.cpp`. |

### Headline file sizes (Northrend / WoLK content)

| File | Lines | Notes |
|---|---|---|
| `Northrend/IcecrownCitadel/boss_the_lich_king.cpp` | 2815 | The 25H final boss. Multi-phase, Frostmourne Room sub-encounter, Tirion intervention. |
| `Northrend/IcecrownCitadel/boss_icecrown_gunship_battle.cpp` | 2243 | Gunship battle — vehicles, jet packs, 4 NPC factions. |
| `Northrend/IcecrownCitadel/icecrown_citadel.cpp` | 1596 | Trash + helper NPCs (gauntlets, plagueworks helpers, blood-prince council servants, wing teleports). |
| `Northrend/IcecrownCitadel/sindragosa.cpp` | 1526 | 3-phase boss with Frost Beacons / Ice Tomb / unchained magic. |
| `Northrend/IcecrownCitadel/boss_sister_svalna.cpp` | 1487 | Crimson Hall (intermission encounter on the path to Blood-Queen). |
| `Northrend/IcecrownCitadel/boss_professor_putricide.cpp` | 1488 | Plague wing final boss — abomination/ooze phases, table interactions. |
| `Northrend/IcecrownCitadel/instance_icecrown_citadel.cpp` | 1420 | The full ICC `InstanceScript` state machine. |
| `Northrend/IcecrownCitadel/boss_blood_prince_council.cpp` | 1341 | Trio fight (Keleseth/Taldaram/Valanar). |
| `Northrend/IcecrownCitadel/boss_valithria_dreamwalker.cpp` | 1284 | Healing-target encounter. |
| `Northrend/IcecrownCitadel/boss_deathbringer_saurfang.cpp` | 1257 | Marks of the Fallen Champion mechanic. |
| `Northrend/IcecrownCitadel/boss_lady_deathwhisper.cpp` | 1081 | Adds + mind control phase. |
| `Northrend/IcecrownCitadel/boss_rotface.cpp` | 796 | Slime spray, ooze flood. |
| `Northrend/IcecrownCitadel/boss_blood_queen_lana_thel.cpp` | 778 | Vampiric bite chain. |
| `Northrend/IcecrownCitadel/boss_lord_marrowgar.cpp` | 697 | First boss — bone spike graveyard. |
| `Northrend/IcecrownCitadel/boss_festergut.cpp` | 457 | Plague wing first boss — inhale stacks. |
| `Northrend/IcecrownCitadel/go_icecrown_citadel_teleport.cpp` | 121 | Teleport pads. |
| **ICC subtotal** | **20,387** | Just one raid. |

Naxxramas and Ulduar have similarly heavy footprints (Naxx: 16 boss files spread across 4 wings; Ulduar: 12 main bosses plus the Halls of Stone/Lightning sub-instances under the same dir).

### Linker glue

| File | Purpose |
|---|---|
| `ScriptLoader.cpp.in.cmake` | CMake-templated source: `@TRINITY_SCRIPTS_FORWARD_DECL@` expands to ~3000 `void AddSC_xxx();` forward decls; `@TRINITY_SCRIPTS_INVOKE@` expands to the matching invocation list inside `AddScripts()`. |
| `ScriptLoader.h` | Declares `AddScripts()` and the per-loader `void AddXxxScripts()` aggregators (one per top-level subdir). |
| `<expansion>_script_loader.cpp` (one per dir, e.g. `northrend_script_loader.cpp`) | Hand-written aggregator that lists every `AddSC_*` for that directory and calls them all. |

---

## 3. Classes / Structs / Enums

There is no single canonical class hierarchy here — every file defines its own bosses. The recurring patterns are:

| Pattern | Kind | Purpose |
|---|---|---|
| `class boss_<name> : public CreatureScript { CreatureAI* GetAI(Creature*) override; class boss_<name>AI : public BossAI { … }; }` | Boss script | Standard one-encounter file. `BossAI` (in `wow-ai`) supplies `Reset/JustEngagedWith/JustDied/EnterEvadeMode/SummonedCreatureDies` plus the `events`/`summons` helpers. |
| `class instance_<dungeon> : public InstanceMapScript { InstanceScript* GetInstanceScript(InstanceMap*) override; struct instance_<dungeon>_InstanceMapScript : public InstanceScript { … }; }` | Instance script | The state machine for a 5-man/raid: tracks boss states, GUIDs of doors/teleporters/event NPCs, encounter progress, achievement criteria, save/load. |
| `class spell_<name> : public SpellScriptLoader { SpellScript* GetSpellScript() override; class spell_<name>_SpellScript : public SpellScript { void Register() override { … } }; }` | Spell script | Per-spell hook bag. Use `BeforeCast`/`OnCast`/`AfterCast`/`OnEffectHitTarget`/`OnHit`/`OnCheckCast`. |
| `class spell_<name>_aura : public AuraScript { … }` | Aura script | Periodic/proc/dispel hooks: `OnEffectApply`, `OnEffectPeriodic`, `OnEffectRemove`, `OnProc`, `AfterDispel`. |
| `class npc_<name> : public CreatureScript` | Quest/escort NPC | Simple gossip, `OnQuestAccept`, escort, helper. |
| `class go_<name> : public GameObjectScript` | GameObject script | `OnGossipHello`, `OnGossipSelect`, `OnUse`, `OnLootStateChanged`. |
| `class at_<name> : public AreaTriggerScript` | Area trigger | `OnTrigger(player, areaTriggerEntry, entered)`. |
| `class achievement_<name> : public AchievementCriteriaScript` | Achievement criterion | `OnCheck` for `MODIFIER_TREE_TYPE_REQUIRED_SCRIPT`. |
| `class <bg>_<name> : public BattlegroundMapScript` / `BattlegroundScript` | BG glue | Less common — most BG state lives in `Battleground` subclasses under `src/server/game/Battlegrounds/Zones/`. |
| `class outdoorpvp_<zone> : public OutdoorPvPScript` | OPvP factory | Returns `OutdoorPvP*` for HP/NA/SI/TF/ZM. |
| `using <X>CommandScript : public CommandScript` | Chat command | `GetCommands()` returns nested `ChatCommandTable` for `.foo`, `.foo bar`, etc. |

The full list of WoLK 3.4.3 `*Script` base classes (the things scripts derive from) is enumerated in `scripting.md` §3.

---

## 4. Critical public methods / functions

Per-file. Below are recurring contract methods in the boss-AI pattern (the most common shape):

| Symbol | Purpose | Calls into |
|---|---|---|
| `BossAI::Reset()` | Wipe state, schedule pre-pull events, restore phase to default | `events.Reset`, `summons.DespawnAll` |
| `BossAI::JustEngagedWith(Unit* who)` | Encounter start: announce yell, schedule `events.ScheduleEvent`, set encounter to IN_PROGRESS | `instance->SetBossState(<id>, IN_PROGRESS)` |
| `BossAI::JustDied(Unit* killer)` | Encounter end: yell, despawn adds, set to DONE, fire achievement criteria | `instance->SetBossState(<id>, DONE)`, `DoCastAOE`, achievement triggers |
| `BossAI::EnterEvadeMode(EvadeReason)` | Wipe path: rewind all timers, despawn summons, set encounter to FAIL, reset doors | `summons.DespawnAll`, `instance->SetBossState(<id>, FAIL)` |
| `BossAI::UpdateAI(uint32 diff)` | Per-tick: drive `events.Update(diff)`, dispatch `events.ExecuteEvent()` switch, call `DoMeleeAttackIfReady` | `events.ExecuteEvent`, spell casts, movement |
| `InstanceScript::OnGameObjectCreate(GameObject*)` | Cache GUIDs of doors/event objects | `ObjectGuid` storage in instance |
| `InstanceScript::OnCreatureCreate(Creature*)` | Cache GUIDs of bosses/event NPCs | as above |
| `InstanceScript::SetBossState(uint32 id, EncounterState state)` | Drive door open/close, achievement checkpoints, save to DB | `Door::DoUseDoorOrButton`, `SaveToDB` |
| `InstanceScript::ReadSaveDataMore(istream&)` / `WriteSaveDataMore(ostream&)` | Per-instance persistence (custom flags beyond boss states) | `instance.data` row |
| `SpellScript::Register()` | Hook bag setup; called once per `SpellScript*` | `OnEffectHitTarget += SpellEffectFn(...)`, etc. |
| `AuraScript::Register()` | Same for auras | `OnEffectApply += AuraEffectApplyFn(...)`, etc. |
| `void AddSC_<file>()` | Registration: `new boss_lord_marrowgar();` `new RegisterSpellScript(spell_xxx);` etc. | Constructor side-effect → `ScriptRegistry<T>::AddScript` |

There is no single facade — each `*Script` ctor does its own `ScriptRegistry` insert.

---

## 5. Module dependencies

**Depends on:**
- `crates/wow-script/` (the `*Script` traits + registry) — see `scripting.md`.
- `crates/wow-ai/` (`BossAI`, `ScriptedAI`, `EventMap`, `SummonList`, `TaskScheduler`).
- `crates/wow-spell/` (for `SpellScript`/`AuraScript` and the spell engine).
- `crates/wow-instance/` and instance/dungeon-finder code (for `InstanceScript`).
- `crates/wow-pvp/`, `crates/wow-battleground/`, `crates/wow-outdoorpvp/` (for BG/OPvP/Battlefield).
- `crates/wow-pet/` (for Pet/* scripts).
- `crates/wow-conditions/` (for AchievementCriteriaScript & ConditionScript).
- `crates/wow-loot/`, `crates/wow-quest/`, `crates/wow-chat/` (for many NPC/quest/gossip/chat scripts).
- DB2 stores (achievement, criteria, spell, item, areatrigger, faction, …).

**Depended on by:**
- Nothing internal. Scripts are leaves; the framework calls into them, not vice versa. They are the **last** thing to migrate, after every dependency is stable.

---

## 6. SQL / DB queries (if any)

Individual scripts rarely emit raw queries. Two recurring exceptions:
- `InstanceScript::SaveToDB` / `ReadSaveDataMore` use `instance` table rows under the hood (handled by core, not script).
- A handful of holiday-event scripts and some larger instance scripts query `world_state`, `world` reference tables, or use `WorldDatabase.PQuery` for one-off lookups (e.g. `Naxxramas` checks `creature_template` for adds).

DBC/DB2 stores read indirectly (via `sObjectMgr`, `sSpellMgr`, `sAchievementMgr`):

| Store | What it loads | Read by |
|---|---|---|
| `SpellMgr` (Spell.db2 + spell_dbc, spell_proc, spell_target_position, …) | Spell metadata referenced by `SpellScript`/`AuraScript` | Every `Spells/*.cpp` |
| `AchievementMgr` (Achievement.db2, Criteria.db2, ModifierTree.db2) | Achievement criteria for `AchievementCriteriaScript` | `World/achievement_scripts.cpp`, instance scripts |
| `ObjectMgr` (creature_template, creature_template_addon, gameobject_template, areatrigger, areatrigger_scripts, conditions) | Per-entry script binding | Every script that resolves a `ScriptName` |
| `MapMgr` (Map.db2, MapDifficulty.db2) | Difficulty selection for raid/heroic versions | Most boss scripts (`IsHeroic()`, `IsTenMan()`, `Is25ManRaid()`) |

---

## 7. Wire-protocol packets (if any)

Scripts emit hundreds of packets. Highlights of the recurring wire-protocol surface:

| Opcode | Direction | Sent by |
|---|---|---|
| `SMSG_PLAY_SOUND` | server → client | Boss scripts, holiday event scripts (yells, mood SFX) |
| `SMSG_PLAY_OBJECT_SOUND` | server → client | Object-anchored sound (gunship horns, ICC throne hum) |
| `SMSG_TEXT_EMOTE` | server → client | Monster yells/emotes (`Talk(SAY_AGGRO)`) |
| `SMSG_CHAT` (variants) | server → client | Boss yells via `BroadcastText` |
| `SMSG_SPELL_GO` / `SMSG_SPELL_START` | server → client | Cast packets from `me->CastSpell` |
| `SMSG_AURA_UPDATE` (and friends) | server → client | Aura application via `AuraScript` |
| `SMSG_GAME_OBJECT_RESET_STATE` / `_CUSTOM_ANIM` | server → client | `GameObject::SetGoState`, animation triggers |
| `SMSG_AREA_TRIGGER_MESSAGE` | server → client | `at_*` scripts |
| `SMSG_GOSSIP_MESSAGE` / `SMSG_GOSSIP_POI` | server → client | Quest/gossip NPCs |
| `SMSG_QUEST_*` | server → client | Quest helper NPCs |
| `SMSG_RAID_INSTANCE_MESSAGE` / `SMSG_RAID_INSTANCE_INFO` | server → client | Instance bind, lockout |
| `SMSG_INSTANCE_ENCOUNTER_*` | server → client | Pull/wipe/end notifications |
| `SMSG_WEATHER` | server → client | Weather event scripts (love-is-in-air haze, hallow's-end smoke) |
| `CMSG_AREATRIGGER` | client → server | Triggers `AreaTriggerScript::OnTrigger` |
| `CMSG_GOSSIP_HELLO` / `CMSG_GOSSIP_SELECT_OPTION` | client → server | NPC gossip |
| `CMSG_QUEST_GIVER_ACCEPT_QUEST` | client → server | Triggers `ItemScript::OnQuestAccept`, quest scripts |

There is no opcode used **only** by scripts — they reuse the entire game protocol.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-scripts/src/lib.rs` — **0 lines** (empty).
- `crates/wow-scripts/Cargo.toml` — depends on `wow-script`, `wow-core`, `wow-constants`. Nothing else.
- No subdirectory structure (no `northrend/`, no `events/`, no `spells/`, no `commands/`).

**What's implemented:** Nothing.

**What's missing vs C++:** The full content layer. Every boss, every instance, every quest helper, every spell script, every GM command, every holiday event, every OutdoorPvP zone, every pet AI, every Wintergrasp interaction. ~725 `.cpp` files / ~294k LOC.

**Suspicious / likely divergent (hypothesis pre-audit):**
- The Rust port will almost certainly **not** mirror the 1-file-per-encounter C++ structure verbatim. Expect a smaller "core encounters" set first (maybe ICC + Naxxramas + Ulduar bosses for WoLK relevance, plus Wintergrasp), then triage everything else.
- Many of the BC and old-world scripts (Karazhan, ZulAman, Sunwell, every Outland 5-man, every old-world dungeon) are functionally **dead content** for a WoLK 3.4.3 server's usage profile (max-level players ignore them entirely), so triage will deprioritize them in practice. They still need scripts to satisfy `creature_template.ScriptName` references during DB load — minimal stubs may suffice.
- `Spells/spell_*.cpp` files are individually large (1000+ lines each). They're the **most reused content** because every class-spell scripted behavior (e.g. shaman's "Fire Nova" exclusion of totems, hunter's "Kill Command" pet trigger, paladin's "Beacon of Light" healing redirection) lives there. **Migrating these is harder than migrating bosses** because they couple tightly with the spell engine in `wow-spell` (which is itself partial) — no shortcut.
- `World/npcs_special.cpp` is a grab-bag of ~3000+ lines covering innkeepers, generic banker, scryer/aldor faction switchers, generic guards, mailbox helpers, vendors, and dozens of one-off NPCs. Splitting it into Rust modules is unavoidable.

**Tests existing:** None.

---

## 9. Migration sub-tasks

Numbering: `#SCRIPTS.N`. Complexity: **L** (<1h), **M** (1–4h), **H** (4–12h), **XL** (>12h, split further). This list is intentionally long; expect to split each XL further at execution time.

### Phase A — scaffolding

- [ ] **#SCRIPTS.1** Create the `wow-scripts` directory layout: `northrend/`, `eastern_kingdoms/`, `kalimdor/`, `outland/`, `events/`, `spells/`, `commands/`, `world/`, `pet/`, `outdoor_pvp/`, `battlefield/`, `custom/`. One `mod.rs` per. (L)
- [ ] **#SCRIPTS.2** Add a `register_all` entry point that the world-server calls on init — the Rust analog of `AddScripts()`. With `inventory::submit!` it's mostly auto-triggered, but you still need the `mod` chain to compile in. (M)
- [ ] **#SCRIPTS.3** Define a stub helper module for the recurring "scripted boss" shape: a `BossAi` adapter wrapping `wow-ai::BossAI` that exposes `register_boss!(name, struct)`. (H)

### Phase B — Northrend (raids, then 5-mans, then zones)

#### Icecrown Citadel (`northrend/icecrown_citadel/`)

- [ ] **#SCRIPTS.10** `instance_icecrown_citadel.rs` — the `InstanceScript` (1420 lines C++). State machine, GUID cache, 12 boss states, achievement criteria. (XL — split into ICC.10a/10b/10c)
- [ ] **#SCRIPTS.11** `boss_lord_marrowgar.rs` (697 lines). Bone Spike Graveyard, Whirlwind, Coldflame, Bone Storm. (H)
- [ ] **#SCRIPTS.12** `boss_lady_deathwhisper.rs` (1081 lines). Adds, MC, mana shield. (XL)
- [ ] **#SCRIPTS.13** `boss_icecrown_gunship_battle.rs` (2243 lines). Vehicles, jet packs, dual-faction. (XL — split per faction + per role)
- [ ] **#SCRIPTS.14** `boss_deathbringer_saurfang.rs` (1257 lines). Mark of the Fallen Champion, Blood Beasts. (XL)
- [ ] **#SCRIPTS.15** `boss_festergut.rs` (457 lines). Inhale Blight, gas spore. (H)
- [ ] **#SCRIPTS.16** `boss_rotface.rs` (796 lines). Slime spray, ooze flood. (XL)
- [ ] **#SCRIPTS.17** `boss_professor_putricide.rs` (1488 lines). Three phases, abomination, choking gas. (XL)
- [ ] **#SCRIPTS.18** `boss_blood_prince_council.rs` (1341 lines). Trio fight. (XL)
- [ ] **#SCRIPTS.19** `boss_blood_queen_lana_thel.rs` (778 lines). Vampiric bite chain. (XL)
- [ ] **#SCRIPTS.20** `boss_valithria_dreamwalker.rs` (1284 lines). Healing-target encounter. (XL)
- [ ] **#SCRIPTS.21** `boss_sindragosa.rs` (1526 lines). Frost Beacons, Ice Tomb, Mystic Buffet. (XL)
- [ ] **#SCRIPTS.22** `boss_the_lich_king.rs` (2815 lines). 4 phases + Frostmourne Room. (XL — split 22a–22e per phase)
- [ ] **#SCRIPTS.23** `boss_sister_svalna.rs` (1487 lines). Crimson Hall path. (XL)
- [ ] **#SCRIPTS.24** `icecrown_citadel.rs` shared trash & helpers (1596 lines). Gauntlet, plagueworks, council servants, wing teleporters. (XL — split per wing)
- [ ] **#SCRIPTS.25** `go_icecrown_citadel_teleport.rs` (121 lines). (L)

#### Ulduar (`northrend/ulduar/`) — 12 boss files + instance + Halls of Stone + Halls of Lightning

- [ ] **#SCRIPTS.30** `instance_ulduar.rs`. (XL)
- [ ] **#SCRIPTS.31** Flame Leviathan vehicle encounter. (XL)
- [ ] **#SCRIPTS.32** Razorscale, Ignis, XT-002, Iron Council, Kologarn, Auriaya, Hodir, Thorim, Freya, Mimiron, General Vezax, Yogg-Saron, Algalon. (XL each → ~13 sub-tasks)
- [ ] **#SCRIPTS.33** Halls of Stone (3 bosses + tribunal event + instance script). (XL → ~5 sub-tasks)
- [ ] **#SCRIPTS.34** Halls of Lightning (4 bosses + instance script). (XL → ~5 sub-tasks)

#### Naxxramas (`northrend/naxxramas/`) — 16 boss files + instance

- [ ] **#SCRIPTS.40** `instance_naxxramas.rs`. (H)
- [ ] **#SCRIPTS.41** Arachnid quarter: Anub'rekhan, Faerlina, Maexxna. (H each → 3 sub-tasks)
- [ ] **#SCRIPTS.42** Plague quarter: Noth, Heigan, Loatheb. (H each → 3)
- [ ] **#SCRIPTS.43** Construct quarter: Patchwerk, Grobbulus, Gluth, Thaddius. (H each → 4; Thaddius is XL)
- [ ] **#SCRIPTS.44** Military quarter: Razuvious, Gothik, Four Horsemen. (H each → 3; Horsemen XL)
- [ ] **#SCRIPTS.45** Frostwyrm Lair: Sapphiron, Kel'Thuzad. (XL each → 2)
- [ ] **#SCRIPTS.46** `naxxramas.rs` shared. (M)

#### Crusaders' Coliseum (`northrend/crusaders_coliseum/`)

- [ ] **#SCRIPTS.50** Trial of the Champion (4 fights + instance). (XL → 5 sub-tasks)
- [ ] **#SCRIPTS.51** Trial of the Crusader: Northrend Beasts, Jaraxxus, Faction Champions, Twin Val'kyr, Anub'arak + instance. (XL → 6 sub-tasks)

#### Northrend 5-mans

- [ ] **#SCRIPTS.55** Azjol-Nerub (Krik'thir, Hadronox, Anub'arak + instance). (XL → 4)
- [ ] **#SCRIPTS.56** Ahn'kahet (5 bosses + instance + 1 helper). (XL → 7)
- [ ] **#SCRIPTS.57** Drak'Tharon Keep (Trollgore, Novos, King Dred, Tharon'ja + instance). (XL → 5)
- [ ] **#SCRIPTS.58** Frozen Halls (Forge of Souls, Pit of Saron, Halls of Reflection — 3 instance scripts × 3-4 bosses). (XL → ~12)
- [ ] **#SCRIPTS.59** Gundrak (5 bosses + instance). (XL → 6)
- [ ] **#SCRIPTS.60** Nexus 5-man + Eye of Eternity 25 (Malygos) + Oculus. (XL → ~10)
- [ ] **#SCRIPTS.61** Utgarde Keep + Utgarde Pinnacle. (XL → ~10)
- [ ] **#SCRIPTS.62** Violet Hold (random boss pool of 6 + instance). (XL → 8)
- [ ] **#SCRIPTS.63** Vault of Archavon (4 bosses + instance). (XL → 5)
- [ ] **#SCRIPTS.64** Chamber of Aspects (Onyxia 3.4 retuned, Obsidian Sanctum, Ruby Sanctum). (XL → 8+)
- [ ] **#SCRIPTS.65** Isle of Conquest BG. (XL)

#### Northrend zones (10 files)

- [ ] **#SCRIPTS.70** `zone_borean_tundra.rs`. (M-H per zone, ~depends on number of NPCs scripted)
- [ ] **#SCRIPTS.71** `zone_dalaran.rs`. (M)
- [ ] **#SCRIPTS.72** `zone_dragonblight.rs`. (M-H)
- [ ] **#SCRIPTS.73** `zone_grizzly_hills.rs`. (M)
- [ ] **#SCRIPTS.74** `zone_howling_fjord.rs`. (M-H)
- [ ] **#SCRIPTS.75** `zone_icecrown.rs`. (H)
- [ ] **#SCRIPTS.76** `zone_sholazar_basin.rs`. (M-H)
- [ ] **#SCRIPTS.77** `zone_storm_peaks.rs`. (M-H)
- [ ] **#SCRIPTS.78** `zone_wintergrasp.rs` (mostly references to `Battlefield/BattlefieldWG`). (M)
- [ ] **#SCRIPTS.79** `zone_zuldrak.rs`. (M-H)

### Phase C — Outland (BC content; lower priority for WoLK server)

- [ ] **#SCRIPTS.100** Black Temple — Illidan, Akama, the 8 mid-bosses. (XL → ~10)
- [ ] **#SCRIPTS.101** Sunwell Plateau (technically EK dir) — Kil'jaeden, M'uru, Brutallus, Felmyst, Eredar Twins, Kalecgos. (XL → ~10)
- [ ] **#SCRIPTS.102** Hyjal Summit — Anetheron, Kaz'rogal, Azgalor, Rage Winterchill, Archimonde. (XL → 7)
- [ ] **#SCRIPTS.103** Karazhan — Attumen, Moroes, Maiden, Opera, Curator, Aran, Netherspite, Nightbane, Prince. (XL → ~12)
- [ ] **#SCRIPTS.104** Tempest Keep — The Eye (Kael'thas), Mech, Bot, Arc. (XL → ~12)
- [ ] **#SCRIPTS.105** Coilfang — SP/SH/SV/UB. (XL → ~12)
- [ ] **#SCRIPTS.106** Hellfire Citadel — HFR/SH/BF/MT. (XL → ~12)
- [ ] **#SCRIPTS.107** Auchindoun — AC/SH/MT/SE. (XL → ~12)
- [ ] **#SCRIPTS.108** Gruul's Lair, Magtheridon's Lair. (XL → 4)
- [ ] **#SCRIPTS.109** Doomwalker, Doomlord Kazzak (world bosses). (M each)
- [ ] **#SCRIPTS.110** ZulAman, Zul'Gurub. (XL → ~12)
- [ ] **#SCRIPTS.111** Magisters' Terrace. (XL → 5)
- [ ] **#SCRIPTS.112** Outland zones (6 files). (M-H each → 6)
- [ ] **#SCRIPTS.113** OutdoorPvP HP/NA/SI/TF/ZM (5 zones). (H each → 5)

### Phase D — old world (very low priority)

- [ ] **#SCRIPTS.130** Eastern Kingdoms 16 zone files. (M each → 16)
- [ ] **#SCRIPTS.131** EK dungeons: Deadmines, Gnomeregan, Stockade, ShadowfangKeep, Stratholme, Scholomance, Karazhan (counted above), Scarlet Monastery, Scarlet Enclave, Sunken Temple, Uldaman, Blackrock Mountain wings (UBRS/LBRS/MC/BWL/BRD). (XL each)
- [ ] **#SCRIPTS.140** Kalimdor 18 zone files. (M each → 18)
- [ ] **#SCRIPTS.141** Kalimdor dungeons: Onyxia, AQ20/AQ40, Dire Maul, Maraudon, Wailing Caverns, Razorfen Downs/Kraul, Blackfathom Deeps, Ragefire Chasm, Zul'Farrak, all 7 Caverns of Time instances. (XL each)

### Phase E — `Spells/` (one giant per-class file each; biggest engineering surface in this whole list)

- [ ] **#SCRIPTS.200** `spell_dk.rs`. (XL — every DK spell with custom logic: Death Coil, Death Grip, Strangulate, Anti-Magic Shell, Bone Shield, Death and Decay, Frost Strike, Howling Blast, Icy Touch, Mind Freeze, Obliterate, Plague Strike, Rune Strike, Scourge Strike, Unbreakable Armor, etc.)
- [ ] **#SCRIPTS.201** `spell_druid.rs`. (XL)
- [ ] **#SCRIPTS.202** `spell_hunter.rs`. (XL)
- [ ] **#SCRIPTS.203** `spell_mage.rs`. (XL)
- [ ] **#SCRIPTS.204** `spell_paladin.rs`. (XL)
- [ ] **#SCRIPTS.205** `spell_priest.rs`. (XL)
- [ ] **#SCRIPTS.206** `spell_rogue.rs`. (XL)
- [ ] **#SCRIPTS.207** `spell_shaman.rs`. (XL)
- [ ] **#SCRIPTS.208** `spell_warlock.rs`. (XL)
- [ ] **#SCRIPTS.209** `spell_warrior.rs`. (XL)
- [ ] **#SCRIPTS.210** `spell_pet.rs`. (XL)
- [ ] **#SCRIPTS.211** `spell_generic.rs`. (XL — non-class one-off spells; the largest of this set)
- [ ] **#SCRIPTS.212** `spell_item.rs`. (XL — trinkets, consumables)
- [ ] **#SCRIPTS.213** `spell_quest.rs`. (XL — quest reward and quest-step spells)

### Phase F — `Commands/` (GM chat commands)

One sub-task per file (42 total). Each is L–M.

- [ ] **#SCRIPTS.300** `cs_account.rs`. (M)
- [ ] **#SCRIPTS.301** `cs_achievement.rs`. (M)
- [ ] **#SCRIPTS.302** `cs_ahbot.rs`. (M)
- [ ] **#SCRIPTS.303** `cs_arena.rs`. (M)
- [ ] **#SCRIPTS.304** `cs_ban.rs`. (M)
- [ ] **#SCRIPTS.305** `cs_battlenet_account.rs`. (M)
- [ ] **#SCRIPTS.306** `cs_bf.rs` (battlefield). (L)
- [ ] **#SCRIPTS.307** `cs_cast.rs`. (M)
- [ ] **#SCRIPTS.308** `cs_character.rs`. (M)
- [ ] **#SCRIPTS.309** `cs_cheat.rs`. (M)
- [ ] **#SCRIPTS.310** `cs_debug.rs`. (M)
- [ ] **#SCRIPTS.311** `cs_deserter.rs`. (L)
- [ ] **#SCRIPTS.312** `cs_disable.rs`. (L)
- [ ] **#SCRIPTS.313** `cs_event.rs`. (L)
- [ ] **#SCRIPTS.314** `cs_gm.rs`. (M)
- [ ] **#SCRIPTS.315** `cs_go.rs`. (M)
- [ ] **#SCRIPTS.316** `cs_gobject.rs`. (M)
- [ ] **#SCRIPTS.317** `cs_group.rs`. (L)
- [ ] **#SCRIPTS.318** `cs_guild.rs`. (M)
- [ ] **#SCRIPTS.319** `cs_honor.rs`. (L)
- [ ] **#SCRIPTS.320** `cs_instance.rs`. (M)
- [ ] **#SCRIPTS.321** `cs_learn.rs`. (M)
- [ ] **#SCRIPTS.322** `cs_lfg.rs`. (M)
- [ ] **#SCRIPTS.323** `cs_list.rs`. (M)
- [ ] **#SCRIPTS.324** `cs_lookup.rs`. (M)
- [ ] **#SCRIPTS.325** `cs_message.rs`. (L)
- [ ] **#SCRIPTS.326** `cs_misc.rs`. (M)
- [ ] **#SCRIPTS.327** `cs_mmaps.rs`. (M)
- [ ] **#SCRIPTS.328** `cs_modify.rs`. (M)
- [ ] **#SCRIPTS.329** `cs_npc.rs`. (M)
- [ ] **#SCRIPTS.330** `cs_pet.rs`. (L)
- [ ] **#SCRIPTS.331** `cs_quest.rs`. (M)
- [ ] **#SCRIPTS.332** `cs_rbac.rs`. (M)
- [ ] **#SCRIPTS.333** `cs_reload.rs`. (M)
- [ ] **#SCRIPTS.334** `cs_reset.rs`. (M)
- [ ] **#SCRIPTS.335** `cs_scene.rs`. (L)
- [ ] **#SCRIPTS.336** `cs_send.rs`. (L)
- [ ] **#SCRIPTS.337** `cs_server.rs`. (M)
- [ ] **#SCRIPTS.338** `cs_tele.rs`. (L)
- [ ] **#SCRIPTS.339** `cs_ticket.rs`. (M)
- [ ] **#SCRIPTS.340** `cs_titles.rs`. (L)
- [ ] **#SCRIPTS.341** `cs_wp.rs` (waypoints). (M)

### Phase G — `World/` (cross-zone shared scripts)

- [ ] **#SCRIPTS.400** `npcs_special.rs` (3000+ lines C++). **Split per-NPC group**. (XL)
- [ ] **#SCRIPTS.401** `npc_professions.rs`. (H)
- [ ] **#SCRIPTS.402** `npc_guard.rs`. (M)
- [ ] **#SCRIPTS.403** `boss_emerald_dragons.rs` (Ysondre, Lethon, Emeriss, Taerar — world bosses). (XL → 5)
- [ ] **#SCRIPTS.404** `go_scripts.rs` (generic gameobjects). (H)
- [ ] **#SCRIPTS.405** `item_scripts.rs`. (M)
- [ ] **#SCRIPTS.406** `areatrigger_scripts.rs`. (H)
- [ ] **#SCRIPTS.407** `achievement_scripts.rs`. (H)
- [ ] **#SCRIPTS.408** `chat_log.rs`, `action_ip_logger.rs`, `boosted_xp.rs`, `duel_reset.rs`. (L each → 4)
- [ ] **#SCRIPTS.409** `conversation_scripts.rs`, `scene_scripts.rs`. (L each)

### Phase H — `Pet/`, `Battlefield/`, `Custom/`

- [ ] **#SCRIPTS.500** `pet_dk.rs`, `pet_hunter.rs`, `pet_mage.rs`, `pet_priest.rs`, `pet_shaman.rs`, `pet_generic.rs`. (M each → 6)
- [ ] **#SCRIPTS.501** `BattlefieldWG.rs` — Wintergrasp full implementation. (XL — coupled to `wow-battlefield`)
- [ ] **#SCRIPTS.502** `Custom/` — empty placeholder. (—)

### Phase I — `Events/` (holiday content scripts)

Covered in `events.md`. Cross-reference: each event file in `scripts/Events/` is **content** that depends on the **scheduler** in `src/server/game/Events/GameEventMgr` which `events.md` documents.

- [ ] **#SCRIPTS.600** `events/brewfest.rs`, `childrens_week.rs`, `darkmoon_faire.rs`, `fireworks_show.rs`, `hallows_end.rs`, `love_is_in_the_air.rs`, `lunar_festival.rs`, `midsummer.rs`, `operation_gnomeregan.rs`, `pilgrims_bounty.rs`, `winter_veil.rs`, `zalazane_fall.rs`. (H each → 12)

---

## 10. Regression tests to write

These are encounter-level acceptance tests; one per major sub-system. **All depend on `#SCRIPTING.*` framework being live first**.

- [ ] Test: an `inventory::submit!` registered `boss_lord_marrowgar` is reachable via `MapManager` for a creature whose template `ScriptName="boss_lord_marrowgar"` and produces a `Box<dyn CreatureAI>` on demand.
- [ ] Test: `instance_icecrown_citadel` `set_boss_state(MARROWGAR, DONE)` opens the door GUID it cached during `on_creature_create`.
- [ ] Test: ICC instance state survives a server restart (re-load via `instance` row).
- [ ] Test: a `spell_dk` aura (Bone Shield) intercepts melee damage as expected.
- [ ] Test: GM command `.gm fly on` toggles `MOVEMENTFLAG_CAN_FLY` and emits the matching opcode to the issuing player.
- [ ] Test: `at_<some_trigger>` fires once on entry and zero times on exit (with `entered=true`).
- [ ] Test: Wintergrasp battle starts on schedule, bestows the Vault of Archavon access aura, and tears it down on end.
- [ ] Test: a Northrend zone NPC (e.g. quest helper) responds to gossip with the right options for a player with the matching quest state.
- [ ] Test: holiday event `winter_veil` swaps the Greatfather Winter NPC vendor when active and reverts when inactive (couples with `events.md`).
- [ ] Test: `boss_emerald_dragons.rs` Ysondre spawn rotation respects the world-event window (couples with `events.md`).

---

## 11. Notes / gotchas

- **This is the longest tail in the project.** Don't try to enumerate sub-tasks at full granularity up-front; the list above is shaped to be a **triage menu** rather than a roadmap. Pick the WoLK-relevant content (Northrend raids + 5-mans + zones, Wintergrasp, Spells/spell_*) and treat the rest as "needed only for DB integrity" (creature_template script names must resolve to *something* even if it's a no-op stub).
- **Dependency order is brutal.** A boss script can't compile until its `BossAI` parent (`wow-ai`), its summon helpers (`wow-ai::SummonList`), its event scheduler (`wow-ai::EventMap`), its instance script base (`wow-instance::InstanceScript`), and most of `wow-spell` are usable. Do **not** start scripts until those are stable enough not to keep churning underneath you.
- **`Spells/spell_*.cpp` is the single biggest blocker for actual gameplay.** If `spell_dk.cpp` / `spell_warrior.cpp` etc. are unmigrated, every DK and warrior is non-functional at endgame. Prioritize Phase E in parallel with Phase B raids — not after.
- C++ instance scripts use `std::ostringstream`/`std::istringstream` for `WriteSaveDataMore`/`ReadSaveDataMore`. The Rust port should use a small `serde` envelope (or fixed-format text mirror) — pick one early because all instance scripts inherit from it.
- Many boss scripts use `Talk(SAY_X)` which under the hood reads from `creature_text` table. The text table is data; the IDs (`SAY_AGGRO`, `SAY_DEATH`, `EMOTE_X`) are constants per-encounter — preserve the constant names for grep-ability.
- C++ frequently pattern: `if (instance && instance->GetData(DATA_X) == DONE) { … }`. The Rust shape will most cleanly use an `enum BossId` indexing into a `[EncounterState; N]` array on the instance script — see how `wow-instance` (when it lands) settles this.
- WoLK-specific: many old-world (Vanilla/BC) instance scripts in this tree also reference `LFG`/`Random Dungeon Finder` mechanics that are post-WoLK. Carefully audit which scripts assume LFG behavior (the `cs_lfg.cpp` admin command and a few instance scripts).
- **Don't port everything.** A minimal viable WoLK 3.4.3 server can ship with: ICC + Naxx + Ulduar + ToC bosses, all WoLK 5-mans, Vault of Archavon, Onyxia 3.4, Wintergrasp, a few hundred high-traffic spell scripts (DK/Pally + every PvP-relevant proc), and ~50 GM commands. Everything else can stub-load.
- The C++ `ScriptLoader.cpp.in.cmake` template auto-generates the master `AddScripts()` function. Rust avoids this by having `inventory::submit!` register at link time inside each module — but you must explicitly `mod` every submodule into the crate root, or the linker will dead-strip the registrations. **This is the same trap as the packet handlers**: forgetting `mod foo;` makes the script silently invisible.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class boss_xxx : public CreatureScript { class boss_xxx_AI : public BossAI { … } }` | `pub struct BossXxxAi { … }` impl `BossAi` trait + `register_creature_ai!(BossXxxAi)` | One file per encounter; flat module structure — drop the nested `_AI` class. |
| `class instance_xxx : public InstanceMapScript` | `pub struct InstanceXxx { state: InstanceState, … }` impl `InstanceScript` trait | One file per dungeon. |
| `class spell_xxx : public SpellScriptLoader` | `pub fn spell_xxx() -> SpellScriptDescriptor { … }` plus `register_spell_script!` | Coupled to `wow-spell`. |
| `class npc_xxx : public CreatureScript` | `pub struct NpcXxxAi { … }` + `register_creature_ai!` | NPC scripts are ordinary creature scripts; the `npc_` prefix is style. |
| `class go_xxx : public GameObjectScript` | `pub struct GoXxx { … }` + `register_game_object_ai!` | — |
| `class at_xxx : public AreaTriggerScript` | `pub struct AtXxx;` + `register_area_trigger!` | — |
| `class xxx_CommandScript : public CommandScript { GetCommands() { return … } }` | a `pub fn xxx_commands() -> CommandTable` + `register_commands!` macro | The chat command builder API is a cross-cutting decision (also see `chat.md` and `scripting.md` #SCRIPTING.17). |
| `void AddSC_xxx()` | (none — `inventory::submit!` replaces this) | Aggregator function disappears. |
| `<expansion>_script_loader.cpp` (e.g. `northrend_script_loader.cpp`) | `crates/wow-scripts/src/northrend/mod.rs` listing `pub mod icecrown_citadel; pub mod ulduar; …` | Same intent (compile-link aggregation) but driven by `mod` declarations. |
| `Talk(SAY_X)` / `creature_text` table | `talk!(self, "SAY_X")` macro that resolves to a `creature_text` row at runtime | Preserve text-id constants. |
| `events.ScheduleEvent(EVENT_X, 5s)` | `self.events.schedule(EventId::X, Duration::from_secs(5))` | `wow-ai::EventMap` design is upstream. |
| `me->CastSpell(target, SPELL_X)` | `unit.cast_spell(target, SpellId::X)` | Through `wow-spell`. |

---

*Template version: 1.0 (2026-05-01).*
