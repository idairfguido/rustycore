# Migration: Entities / Conversation

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Entities/Conversation/`
> **Rust target crate(s):** `crates/wow-world/`, `crates/wow-data/`, `crates/wow-constants/`
> **Layer:** L4 (sub-modules)
> **Status:** ❌ not started — **n/a for WoLK 3.4** (post-WoLK feature; carry stub only if needed)
> **Audited vs C++:** ⚠️ partial (header-level audit only)
> **Last updated:** 2026-05-01

---

## 1. Purpose

`Conversation` is the in-world cinematic-conversation entity introduced in **Battle for Azeroth (8.x)** and folded back into the legacy TC tree. It owns a list of "actors" (NPCs by GUID or by creature/display id), per-line start/end timing per locale, and a `_textureKitId` for client visuals. The client renders it as a popup-ish in-world dialogue between scripted speakers (think the cinematic conversations in BfA quests). **It does not exist in the WotLK 3.4.3.54261 retail client.** Including in TC's WoLK fork is a backport artifact — the high-guid type and TYPEID are reserved but no live data uses them.

---

## 2. C++ canonical files

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Entities/Conversation/Conversation.h` | 102 | `Conversation` class (final, WorldObject + GridObject) |
| `src/server/game/Entities/Conversation/Conversation.cpp` | 399 | Create, Update, Start, AddActor, line timing |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Conversation` | class (final) | Cinematic conversation entity |
| `ConversationActorType` | enum class (forward decl) | `WorldObject` vs `CreatureActor` typing |
| `UF::ConversationData` | UF struct | Wire data (lines, actors, lastLineEndTime, progress) |

---

## 4. Critical public methods

| Symbol | Purpose | Calls into |
|---|---|---|
| `CreateConversation(entry, creator, pos, privateOwner, spellInfo, autoStart)` | Static factory | `Create`, `Start` |
| `Start()` | Begin playback (server timer-driven) | broadcast UpdateObject |
| `Update(uint32 diff)` | Tick conversation playback; remove when last line ends | `Remove` |
| `AddActor(actorId, actorIdx, actorGuid)` | Bind world-object actor to slot | UpdateField mutation |
| `AddActor(actorId, actorIdx, type, creatureId, displayId)` | Bind static creature/display actor | — |
| `GetLineStartTime(locale, lineId)` / `GetLineEndTime` / `GetLastLineEndTime` | Per-locale timing queries | `_lineStartTimes`, `_lastLineEndTimes` |
| `GetActorUnit(actorIdx)` / `GetActorCreature(actorIdx)` | Resolve slot to entity | MapManager lookup |
| `GetTextureKitId()` / `GetDuration()` / `GetScriptId()` | Read-only accessors | — |
| `Remove()` | Despawn | `RemoveFromWorld` |

---

## 5. Module dependencies

**Depends on:**
- `WorldObject` / `GridObject` (entity base)
- `ConversationDataStore` (templates loaded from `conversation_template`, `conversation_actors`, `conversation_line` DB tables)
- `ConversationLine.dbc` / `ConversationLineStore` (per-line locale text + duration; **does not exist in 3.4 client DBCs**)
- `Map` (AddToMap)
- Scripts (per-id cinematic logic)

**Depended on by:**
- Quest scripts (post-WoLK)
- Spell effects with `EFFECT_CREATE_CONVERSATION` (post-WoLK)
- Private object visibility (`privateObjectOwner` GUID — only that player sees it)

---

## 6. SQL / DB queries

| Statement / Source | Purpose | DB |
|---|---|---|
| `conversation_template` | Per-id template (textureKit, duration) | world (post-WoLK schema) |
| `conversation_actors` | Actor slot definitions | world |
| `conversation_line_template` | Per-line scripted properties | world |

DBC stores (post-WoLK):

| Store | What it loads | Read by |
|---|---|---|
| `ConversationLineStore` | `ConversationLine.db2` | line duration / text |

**Note:** None of these tables/DB2 files exist in a stock WoLK 3.4.3 install. Including them requires a backport schema.

---

## 7. Wire-protocol packets

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `SMSG_UPDATE_OBJECT` (with Conversation block) | S → C | spawn (post-WoLK clients) |
| `SMSG_CONVERSATION_LINE_STARTED` | S → C | per-line trigger | (`crates/wow-constants/src/opcodes.rs`: `ConversationLineStarted = 0x3546`) |

**Caveat:** the 3.4.3.54261 retail client does **not** know how to render `SMSG_CONVERSATION_LINE_STARTED`; sending it is a no-op or a desync risk. The opcode constant in `wow-constants` is enumerated for completeness only.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-constants/src/object.rs` — `TypeId::Conversation = 13`, `HighGuid::Conversation = 7`
- `crates/wow-core/src/guid.rs` — `HighGuid::Conversation`, `is_conversation`, type mapping
- `crates/wow-constants/src/opcodes.rs` — `ConversationLineStarted = 0x3546`
- `crates/wow-packet/src/packets/update.rs` — Conversation block bit hardcoded to `false` in object update mask
- **0 lines** of Conversation entity logic.

**What's implemented:** type-id, GUID type, opcode constant. Update bit reserved to `false`.

**What's missing vs C++:** entire 399-line `Conversation.cpp`. Not a priority for 3.4 — see status note.

**Suspicious / likely divergent:** none — feature does not exist on the target client.

**Tests existing:** 0.

---

## 9. Migration sub-tasks

- [ ] **#CONV.1** **Decision gate:** confirm whether any 3.4 backport content (custom server scripts) needs Conversation. If no — close as `n/a` and stop. If yes — proceed. (L)
- [ ] **#CONV.2** Port `ConversationActorType` enum to `wow-constants` (L)
- [ ] **#CONV.3** Define `Conversation` entity struct in `wow-world/src/entities/conversation.rs` (M)
- [ ] **#CONV.4** Implement `create` / `start` / `update_tick` / `remove` (M)
- [ ] **#CONV.5** Implement `add_actor` (both overloads) (L)
- [ ] **#CONV.6** Per-locale line-timing storage (`HashMap<(Locale, i32), Duration>`) (L)
- [ ] **#CONV.7** Conversation DB schema + loader in `wow-data` (M, post-WoLK schema design)
- [ ] **#CONV.8** `ConversationData` UF block + flip create/update bit (M)
- [ ] **#CONV.9** Wire `SMSG_CONVERSATION_LINE_STARTED` sender (only if client supports — verify in sniffs) (L)

**Recommendation:** mark this module **`n/a`** in the master roadmap until a concrete user need is identified for the 3.4 fork.

---

## 10. Regression tests to write

(Only relevant if the decision gate in #CONV.1 says yes.)

- [ ] Test: `add_actor` populates the correct slot index in the UF actor array
- [ ] Test: `update_tick` past `last_line_end_time` calls `remove`
- [ ] Test: per-locale line timing returns correct end-time for a given lineId
- [ ] Test: private-object owner GUID restricts visibility (only that player sees the entity in update broadcasts)

---

## 11. Notes / gotchas

- **WoLK 3.4.3 retail client cannot render conversations.** The structural code (TYPEID, HighGuid, opcode constant) is in TC's WoLK fork as dead infrastructure. Spawning a Conversation entity on a real 3.4 client will at best be ignored, at worst desync the world state.
- This module is the strongest "n/a" candidate of the seven entity sub-types in this batch. Recommend documenting and parking, not implementing.
- The C# legacy reference at `/home/server/woltk-server-core/Source/` likely **does not** have a Conversation class either — confirm before starting any port work.
- Private-object owner pattern is shared with SceneObject (single-player visibility). If both end up needed, factor that filter into a shared `PrivateObjectFilter` utility.
- `_lineStartTimes` is keyed by `(LocaleConstant, lineId)` — Rust would use `HashMap<(Locale, i32), Duration>`; locale is `enum Locale` (already exists somewhere in `wow-core` for chat).

---

## 12. C++ → Rust mapping

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class Conversation : WorldObject` | `struct Conversation` (composition) | if implemented |
| `Milliseconds _duration` | `Duration` | `std::time::Duration` |
| `unordered_map<pair<Locale, i32>, Milliseconds>` | `HashMap<(Locale, i32), Duration>` | direct |
| `array<Milliseconds, TOTAL_LOCALES>` | `[Duration; TOTAL_LOCALES]` | direct |
| `enum class ConversationActorType` | `enum ConversationActorType { WorldObject, Creature }` | direct |
| `Position _stationaryPosition` | `Position` field | from `wow-core` |

---

## 13. §13 Audit (vs `/home/server/woltk-trinity-legacy/`)

| C++ symbol | Found in Rust | File | Verdict |
|---|---|---|---|
| `class Conversation` | no | — | ❌ missing (and likely n/a) |
| `enum class ConversationActorType` | no | — | ❌ missing |
| `Conversation::CreateConversation` | no | — | ❌ missing |
| `Conversation::AddActor` | no | — | ❌ missing |
| `Conversation::Update` / `Start` / `Remove` | no | — | ❌ missing |
| `_lineStartTimes` per-locale timing | no | — | ❌ missing |
| `UF::ConversationData` UF block | no (bit hardcoded false) | `crates/wow-packet/src/packets/update.rs` | ❌ missing |
| `TypeId::Conversation = 13` | yes | `crates/wow-constants/src/object.rs` | ✅ present (constant only) |
| `HighGuid::Conversation = 7` | yes | `crates/wow-core/src/guid.rs` | ✅ present (constant only) |
| `SMSG_CONVERSATION_LINE_STARTED` opcode | yes (constant) | `crates/wow-constants/src/opcodes.rs` | ⚠️ enumerated, no sender (and client likely won't render) |

**Verdict:** ❌ not started — **and recommended to stay that way for the 3.4 fork.** Surface coverage ≈ 0% (constants reserved). Mark `n/a` in master roadmap pending explicit user requirement.
