# Migration: Texts (CreatureTextMgr + ChatTextBuilder)

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Texts/`
> **Rust target crate(s):** `crates/wow-chat/` for the senders/builders, `crates/wow-database/` for the loader, `crates/wow-world/` for the per-creature repeat-tracking glue
> **Layer:** L0 (foundation — every boss talk, NPC yell, scripted whisper goes through this)
> **Status:** ❌ not started
> **Audited vs C++:** ✅ complete (3 files, ~825 lines total)
> **Last updated:** 2026-05-01

---

## 1. Purpose

`CreatureTextMgr` is the indirection layer between scripts/AI and the chat wire format. Scripts call `sCreatureTextMgr->SendChat(creature, textGroup, …)` with a *symbolic* group id (e.g. "boss-aggro = group 0"); the manager picks one text from that group at random (weighted by probability, avoiding immediate repeats), localizes it per-recipient, plays the associated sound/emote, and routes the resulting `SMSG_CHAT` packet to the right audience (range / area / zone / map / world / personal whisper). Every boss yell, every quest-giver line, every "Ow!" emote in a script flows through this. `ChatTextBuilder` provides the polymorphic builder objects that emit the correct `WorldPackets::Chat::Chat` from various source kinds (broadcast text, custom string, format-string, creature-text).

---

## 2. C++ canonical files

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Texts/CreatureTextMgr.h` | 131 | Public API: `CreatureTextEntry`, `CreatureTextMap`, `CreatureTextMgr` singleton, `CreatureTextRange`, `SoundKitPlayType` |
| `src/server/game/Texts/CreatureTextMgr.cpp` | 446 | Loader (`LoadCreatureTexts`, `LoadCreatureTextLocales`), `SendChat`, range broadcasters, `GetLocalizedChatString` |
| `src/server/game/Texts/CreatureTextMgrImpl.h` | 165 | Templated `SendChatPacket<Builder>` + `CreatureTextLocalizer` (one packet cached per `LocaleConstant` per send) |
| `src/server/game/Texts/ChatTextBuilder.h` | 125 | `ChatPacketSender` + four builder kinds: `BroadcastTextBuilder`, `CustomChatTextBuilder`, `TrinityStringChatBuilder`, `CreatureTextTextBuilder` |
| `src/server/game/Texts/ChatTextBuilder.cpp` | ~110 | Builder operator() implementations (not read but inferred from header) |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `CreatureTextMgr` | class (singleton) | Registry + dispatch |
| `CreatureTextEntry` | struct | One row of `creature_text` (creature, group, id, text, type, lang, prob, emote, duration, sound, soundPlayType, broadcastTextId, range) |
| `CreatureTextLocale` | struct | `vector<string> Text[locale]` — translated variants |
| `CreatureTextId` | struct | `(entry, textGroup, textId)` triple — key for the locale map; has `<=>` for ordering |
| `CreatureTextRange` | enum | `NORMAL=0`, `AREA=1`, `ZONE=2`, `MAP=3`, `WORLD=4`, `PERSONAL=5` |
| `SoundKitPlayType` | enum class | `Normal=0`, `ObjectSound=1` |
| `CreatureTextHolder` | typedef | `unordered_map<u8 groupId, vector<CreatureTextEntry>>` |
| `CreatureTextMap` | typedef | `unordered_map<u32 creatureEntry, CreatureTextHolder>` |
| `LocaleCreatureTextMap` | typedef | `map<CreatureTextId, CreatureTextLocale>` |
| `Trinity::ChatPacketSender` | class | Holds an untranslated + lazily-translated `WorldPackets::Chat::Chat`; `operator()(Player const*)` sends the right one |
| `Trinity::BroadcastTextBuilder` | class | Wraps a `BroadcastText.db2` id + gender; `operator()(LocaleConstant)` returns a fresh `ChatPacketSender` |
| `Trinity::CustomChatTextBuilder` | class | Wraps an explicit `string_view` + language |
| `Trinity::TrinityStringChatBuilder` | class | Wraps a `trinity_string` ID with `va_list` args |
| `Trinity::CreatureTextTextBuilder` | class | Wraps `(speaker, gender, msgType, group, id, language)` for replays of cached creature text |
| `CreatureTextLocalizer<Builder>` | template | Per-send object that caches one `ChatPacketSender` per locale-index |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `CreatureTextMgr::LoadCreatureTexts()` | Read `creature_text` (prepared `WORLD_SEL_CREATURE_TEXT`); validate Sound/Emote/Type/Lang/BroadcastTextId/Range against DB2 stores; populate `mTextMap` | DB2 stores, `LanguageMgr` |
| `CreatureTextMgr::LoadCreatureTextLocales()` | Read `creature_text_locale` into `mLocaleTextMap` | `ObjectMgr::AddLocaleString` |
| `CreatureTextMgr::SendChat(source, textGroup, whisperTarget?, msgType?, language?, range?, sound?, playType?, team?, gmOnly?, srcPlr?)` | Pick a random non-recently-said entry weighted by `probability`; resolve sound (override or BroadcastText-derived), emote, range; build a `CreatureTextTextBuilder`; call `SendChatPacket`; mark the id as said via `Creature::SetTextRepeatId`; **returns the `duration` field** (used by AI to schedule the next line) | `Containers::SelectRandomWeightedContainerElement`, `SendSound`, `SendEmote`, `SendChatPacket`, `Creature::GetTextRepeatGroup`/`SetTextRepeatId`/`ClearTextRepeatGroup` |
| `CreatureTextMgr::TextExist(entry, group)` | Existence probe used by AI sanity asserts | — |
| `CreatureTextMgr::GetLocalizedChatString(entry, gender, group, id, locale)` | Resolve the translated string for arbitrary entry/group/id (used outside the chat dispatch — e.g. spell logs) | `DB2Manager::GetBroadcastTextValue`, `ObjectMgr::GetLocaleString` |
| `CreatureTextMgr::SendSound(source, sound, msgType, whisperTarget?, range?, team?, gmOnly?, broadcastTextId?, playType?)` | Send `SMSG_PLAY_SOUND` or `SMSG_PLAY_OBJECT_SOUND` to the same audience as the chat would go | `SendNonChatPacket` |
| `CreatureTextMgr::SendEmote(unit, emote)` | Trigger a visual emote (`HandleEmoteCommand`) | `Unit::HandleEmoteCommand` |
| `CreatureTextMgr::SendChatPacket<Builder>(source, builder, msgType, …)` (template) | Generic dispatcher: instantiates `CreatureTextLocalizer<Builder>`, switches on range, calls the worker against the correct player set | `Cell::VisitWorldObjects`, `Group::BroadcastWorker`, `Map::GetPlayers`, `World::GetAllSessions` |
| `CreatureTextMgr::SendNonChatPacket(...)` | Same range-dispatch logic but for arbitrary packets (sound/object-sound) | same as above |
| `static GetRangeForChatType(msgType)` | YELL/EMOTE/SAY → distance from `CONFIG_LISTEN_RANGE_*` | `World::getFloatConfig` |
| `Trinity::ChatPacketSender::operator()(Player const*)` | Decide translated vs untranslated based on `LanguageMgr` + recipient skill, send the right cached packet | — |
| `Trinity::*Builder::operator()(LocaleConstant)` | Build and return a fresh `ChatPacketSender` for the requested locale | — |

---

## 5. Module dependencies

**Depends on:**
- `Database` — prepared `WORLD_SEL_CREATURE_TEXT`, plain query for locales.
- `DataStores/DB2Stores` — `sBroadcastTextStore`, `sSoundKitStore`, `sEmotesStore` for validity checks; `DB2Manager::GetBroadcastTextValue` for translation.
- `Globals/LanguageMgr` — language existence + skill-based translation gating.
- `Globals/ObjectMgr` — `GetScriptId`, `AddLocaleString`, `GetLocaleString`.
- `Server/Packets/ChatPackets` — `WorldPackets::Chat::Chat`.
- `Server/Packets/MiscPackets` — `WorldPackets::Misc::PlaySound`, `PlayObjectSound`.
- `Grids/Cell` + `GridNotifiers::PlayerDistWorker` — TEXT_RANGE_NORMAL distance broadcast.
- `Maps/Map` — `GetPlayers()` for AREA/ZONE/MAP ranges.
- `Server/World` — `GetAllSessions`, listen-range floats.
- `Groups/Group` — `BroadcastWorker` for `CHAT_MSG_MONSTER_PARTY`.

**Depended on by:**
- All `wow-script` boss/quest scripts — every `Talk(group)` call in C++ becomes a `CreatureTextMgr::SendChat`.
- AI: `SmartAI` / `ScriptedAI` use `Talk` extensively; the `duration` return value drives `Talk()`'s sleep-then-next behaviour.
- `Creature` itself — owns `m_textRepeat`, used to avoid saying the same line twice in a row.

---

## 6. SQL / DB queries (if any)

| Statement / Source | Purpose | DB |
|---|---|---|
| `WORLD_SEL_CREATURE_TEXT` (prepared) | All rows of `creature_text` | world |
| Inline `SELECT … FROM creature_text_locale` | Localized variants | world |

DBC/DB2 stores consumed:

| Store | What it loads | Read by |
|---|---|---|
| `sBroadcastTextStore` | `BroadcastText.db2` | LoadCreatureTexts (validity), SendChat (sound override), GetLocalizedChatString (translation) |
| `sSoundKitStore` | `SoundKit.db2` | LoadCreatureTexts (validity) |
| `sEmotesStore` | `Emotes.db2` | LoadCreatureTexts (validity) |

---

## 7. Wire-protocol packets (if any)

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `SMSG_CHAT` | server → client | All `SendChat` paths via `WorldPackets::Chat::Chat` |
| `SMSG_PLAY_SOUND` | server → client | `SendSound` (Normal play type) |
| `SMSG_PLAY_OBJECT_SOUND` | server → client | `SendSound` (ObjectSound play type) |
| `SMSG_EMOTE` | server → client | `SendEmote` via `Unit::HandleEmoteCommand` |

This module *only* sends; nothing client-originated routes here.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-database/src/statements/world.rs:14` declares the prepared statement constant `SEL_CREATURE_TEXT` with the right column list (`SELECT CreatureID, GroupID, ID, Text, Type, Language, Probability, Emote, Duration, Sound, SoundPlayType, BroadcastTextId, TextRange FROM creature_text` at line 132). No loader, no struct, no `SendChat`, no builders.
- No `creature_text.rs`, no `text_mgr.rs` anywhere.
- Boss/scripted talks elsewhere in the codebase are stubbed out or short-circuit-routed through ad-hoc `SMSG_CHAT` constructions.

**What's implemented:** the prepared statement string. That's it.

**What's missing vs C++:**
- `CreatureTextMgr` registry and loader.
- `CreatureTextLocale` table loader.
- The weighted-random + repeat-avoidance `SendChat` pick logic.
- The four range broadcasters (AREA/ZONE/MAP/WORLD/PERSONAL) and the cell-visit fallback for NORMAL.
- Per-locale packet caching (`CreatureTextLocalizer`).
- `ChatPacketSender` and the four builder kinds.
- `Creature` text-repeat memory (the `m_textRepeat` field needs to live on the `WorldCreature` in `MapManager`).

**Suspicious / likely divergent:** any current Rust scripted boss talk uses a different code path that won't share repeat-avoidance with `CreatureTextMgr` once the mgr exists. Audit needed at integration time.

**Tests existing:** none.

---

## 9. Migration sub-tasks

- [ ] **#TXT.1** Add `CreatureTextEntry` struct + `CreatureTextRange`/`SoundKitPlayType` enums in `wow-chat`. Implement `CreatureTextMgr::load(pool)` reading via `SEL_CREATURE_TEXT`. Validate Sound/Emote/Lang/Type/BroadcastTextId/Range with the same warn-and-default behaviour. (complexity: **M**)
- [ ] **#TXT.2** Add `CreatureTextLocale` and the `(entry, group, id) → Vec<String> per-locale` loader. (complexity: **L**)
- [ ] **#TXT.3** Implement `SendChat`: weighted pick (`rand` + `Vec<f32>` probabilities), repeat-avoidance via a `text_repeat: HashMap<u8 group, Vec<u8 ids>>` field on `WorldCreature`. Return the `duration` ms. (complexity: **M**)
- [ ] **#TXT.4** Define a `Builder` trait in `wow-chat` and the four impls (`BroadcastTextBuilder`, `CustomChatTextBuilder`, `RustyStringChatBuilder`, `CreatureTextTextBuilder`). Implement `ChatPacketSender` with per-locale lazy translation. (complexity: **M**)
- [ ] **#TXT.5** Implement the range dispatcher (generic over `Builder`): NORMAL = grid-cell visit through `MapManager`; AREA/ZONE/MAP = filter `Map`'s player list; WORLD = iterate `PlayerRegistry`; PERSONAL = single `whisperTarget`; `MONSTER_PARTY` = group-broadcast. (complexity: **H** — touches many subsystems)
- [ ] **#TXT.6** Implement `SendSound`/`SendEmote` and `SendNonChatPacket` (mirror dispatcher logic for arbitrary `Vec<u8>` payload, no `Builder`). (complexity: **M**)
- [ ] **#TXT.7** Implement `GetLocalizedChatString(entry, gender, group, id, locale)` for use outside SendChat (logs, debugging). (complexity: **L**)

---

## 10. Regression tests to write

- [ ] Loader: a `creature_text` row with a non-existent `Sound` is loaded with `sound = 0` and an error logged.
- [ ] `SendChat` weighted pick with seed S over 10K iterations matches the C++ histogram for a fixture group.
- [ ] Repeat avoidance: after saying every id in a group, the `text_repeat` set clears and the cycle repeats — never returns `id = N` twice in a row except after `ClearTextRepeatGroup`.
- [ ] `duration` return value matches the picked entry's `duration` column.
- [ ] Range = ZONE: a player in a different zone on the same map does NOT receive the packet; a player in the same zone on a different map (impossible, but sanity) is filtered.
- [ ] `MONSTER_PARTY` whisper targeting a player in a group sends to all group members; targeting a soloer sends to that soloer only (NOT to the wider area).
- [ ] BroadcastText resolution: when `BroadcastTextId` is set, the recipient's locale-specific string is used; the gender variant is selected by the *speaker's* gender.
- [ ] Translation gating: a player without the required language skill receives the untranslated (gibberish) variant; with the skill, the translated one.

---

## 11. Notes / gotchas

- **The `duration` return value drives boss talk pacing**: `BossAI::Talk(group)` schedules its next line based on it. Don't change the return type or hide the field — many scripts implicitly depend on it.
- **`CreatureTextMgr` drives ALL boss talks, NPC yells, scripted whispers**. Any "talk" in TrinityCore code goes through `Creature::Talk(group)` which delegates here. There is essentially no other path. This is the single most-trafficked text system in the server.
- **`text_repeat` lives on the `Creature` (per-instance), not on the `CreatureTextMgr` (per-entry)**. Two instances of the same boss in two different copies of the same instance do *not* share repeat memory. Mirror that — put it on `WorldCreature`, not on the mgr.
- **`CHAT_MSG_ADDON` + `LANG_ADDON` are sentinels for "use the value from the DB row"**. `SendChat`'s default args mean "let the DB decide". Don't replace them with `Option::None` semantics — many callers pass `CHAT_MSG_ADDON` deliberately.
- **`CreatureTextLocalizer` caches one `ChatPacketSender` per `LocaleConstant`** (currently 12+1 locales) per send call. Important when broadcasting to hundreds of players in TEXT_RANGE_WORLD — without the cache, every recipient re-builds the packet.
- **`CHAT_MSG_MONSTER_WHISPER` and `CHAT_MSG_RAID_BOSS_WHISPER` short-circuit the range** when `range == TEXT_RANGE_NORMAL` — they go to the whisper target only. The `range != NORMAL` path falls through to the standard dispatcher and ignores `whisperTarget`. Easy to get backwards.
- **`creature_text_locale.Locale = enUS` rows are skipped** at load time (the base text in `creature_text` is already enUS). Replicate or you'll have duplicate-string ambiguity.
- **WoLK 3.4.3 specific**: `BroadcastText.db2` has a `SoundKitID[2]` column (male/female). The C++ uses `[GetGender() == GENDER_FEMALE ? 1 : 0]`. Female-voiced bosses are real; don't lose this.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class CreatureTextMgr` (singleton) | `pub struct CreatureTextMgr` + `Arc<CreatureTextMgr>` injected | Avoid global; pass through registries |
| `struct CreatureTextEntry` | `#[derive(Clone)] pub struct CreatureTextEntry` | One per row |
| `unordered_map<u32, unordered_map<u8, vector<…>>>` | `HashMap<u32, HashMap<u8, Vec<CreatureTextEntry>>>` | Triple-nested OK |
| `template<class Builder> SendChatPacket(...)` | generic `fn send_chat_packet<B: ChatBuilder>(...)` | Trait `ChatBuilder { fn build(&self, locale) -> ChatPacketSender; }` |
| `Trinity::ChatPacketSender` | `pub struct ChatPacketSender { untranslated: Vec<u8>, translated: OnceCell<Vec<u8>>, … }` | Lazy translation via `OnceCell` |
| `Cell::VisitWorldObjects(source, worker, dist)` | iterate the 3×3 grid window in `MapManager` filtering by distance | `MapManager` already exposes a similar primitive |
| `Group::BroadcastWorker(localizer)` | `GroupRegistry::for_each_member(group_guid, |player_guid| { … })` | Existing primitive in `wow-network` |
| `World::GetAllSessions()` | `PlayerRegistry::iter()` | Existing |
| `Creature::GetTextRepeatGroup(group)` / `SetTextRepeatId` / `ClearTextRepeatGroup` | three methods on `WorldCreature` | Lives in `crates/wow-world/src/map_manager.rs` |

---

*Template version: 1.0 (2026-05-01).*
