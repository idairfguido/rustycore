# Migration: game/Handlers

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Handlers/`
> **Rust target crate(s):** `crates/wow-world/src/handlers/` + `crates/wow-handler/`
> **Layer:** L2
> **Status:** ⚠️ partial (~39% coverage)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Procesa packets CMSG del cliente y produce SMSG. ~560 métodos `HandleXxx` distribuidos en 45 archivos C++ (~21.8K líneas). Los handlers son métodos de `WorldSession` en C++; en Rust son funciones libres organizadas por dominio (`character.rs`, `chat.rs`, `movement.rs`, etc.) registradas vía `inventory::submit!` para mapeo opcode → handler.

---

## 2. C++ canonical files (45 handlers)

| File | Lines | Opcodes (~) | Purpose |
|---|---|---|---|
| `AdventureJournalHandler.cpp` | 67 | 1-2 | Adventure journal (legion+, stub) |
| `AdventureMapHandler.cpp` | 40 | 1 | Adventure map (legion+, stub) |
| `AuctionHouseHandler.cpp` | 1124 | 10-12 | Browse, bid, sell |
| `AuthHandler.cpp` | 126 | 3-5 | Session auth, version, addon list |
| `BankHandler.cpp` | 324 | 8-10 | Bank/vault access |
| `BattleGroundHandler.cpp` | 1346 | 15-20 | BG queue, leave, port |
| `BattlenetHandler.cpp` | 88 | 2-3 | BNet roster (legion+) |
| `BattlePetHandler.cpp` | 134 | 5-7 | Pet journal (MoP+) |
| `BlackMarketHandler.cpp` | 158 | 3-5 | Black market (MoP+) |
| `CalendarHandler.cpp` | 575 | 10-15 | Calendar events, RSVP |
| `ChannelHandler.cpp` | 219 | 8-10 | Chat channels join/leave/mod |
| `CharacterHandler.cpp` | 2884 | 25-30 | Char enum/create/delete/login/customize/rename |
| `ChatHandler.cpp` | 830 | 18-22 | Say/yell/whisper/emote/addon |
| `CollectionsHandler.cpp` | 43 | 1-2 | Toy/heirloom (WoD+, stub) |
| `CombatHandler.cpp` | 82 | 3-5 | Attack swing/stop, duel legacy |
| `DuelHandler.cpp` | 106 | 3-5 | Duel accept/decline/forfeit |
| `GarrisonHandler.cpp` | 44 | 1-2 | Garrison (WoD+, stub) |
| `GroupHandler.cpp` | 783 | 15-18 | Party invite/leave, loot, leader |
| `GuildHandler.cpp` | 813 | 18-22 | Guild create/invite/roster/rank |
| `HotfixHandler.cpp` | 126 | 2-3 | DB2 hotfix request (MoP+) |
| `InspectHandler.cpp` | 152 | 3-5 | Inspect player/talents/glyphs |
| `ItemHandler.cpp` | 1220 | 15-18 | Swap, burn, sell, container |
| `LFGHandler.cpp` | 971 | 15-20 | LFG queue/cancel/role/teleport |
| `LootHandler.cpp` | 508 | 8-12 | Loot item/pass/disenchant/money |
| `MailHandler.cpp` | 676 | 10-12 | Mail list/body/send/delete |
| `MiscHandler.cpp` | 1440 | 25-35 | Repop, tutorial, reset, taxi |
| `MovementHandler.cpp` | 816 | 18-25 | Move/jump/fall/teleport/transport |
| `NPCHandler.cpp` | 577 | 10-15 | NPC quest list, gossip, vendor, stable, trainer |
| `PetHandler.cpp` | 810 | 12-18 | Pet command/summon/control |
| `PetitionsHandler.cpp` | 464 | 6-8 | Guild petition sign |
| `QueryHandler.cpp` | 333 | 4-6 | Query creature/GO/quest/NPC text |
| `QuestHandler.cpp` | 849 | 12-16 | Quest accept/complete/abandon |
| `ScenarioHandler.cpp` | 43 | 1-2 | Scenario (MoP+, stub) |
| `SceneHandler.cpp` | 43 | 1-2 | Scene (WoD+, stub) |
| `SkillHandler.cpp` | 117 | 3-5 | Skill change/initial |
| `SocialHandler.cpp` | 184 | 5-8 | Friend/ignore list |
| `SpellHandler.cpp` | 569 | 10-15 | Cast/cancel cast/skill learn |
| `TaxiHandler.cpp` | 239 | 5-7 | Taxi node list, activate |
| `TicketHandler.cpp` | 152 | 3-5 | Bug report, GM ticket |
| `TokenHandler.cpp` | 43 | 1-2 | Token redeem (WoD+, stub) |
| `ToyHandler.cpp` | 102 | 2-3 | Toy list/use (WoD+) |
| `TradeHandler.cpp` | 814 | 12-15 | Trade begin/items/accept/money |
| `TransmogrificationHandler.cpp` | 314 | 4-6 | Transmog appearance |
| `VehicleHandler.cpp` | 196 | 5-7 | Vehicle enter/exit/seat |
| `VoidStorageHandler.cpp` | 249 | 5-7 | Void storage (Cata+) |
| **TOTAL** | **~21,793** | **~260** | **45 archivos, ~560 métodos `HandleXxx`** |

---

## 3. Grouped handler summary (high-level domains)

- **Character / Session (CharacterHandler):** 25-30 opcodes — login, create, customize, rename, faction change, equipment sets
- **Chat (ChatHandler + ChannelHandler):** 25-28 opcodes — say/yell/whisper/channel/emote/addon
- **Movement (MovementHandler):** 18-25 opcodes — move, jump, fall, teleport, transport, heartbeat
- **Combat / Duel (CombatHandler + DuelHandler):** 6-10 opcodes
- **Social (SocialHandler):** 5-8 opcodes — friends, ignore
- **Group / Raid (GroupHandler):** 15-18 opcodes — invite, leave, role, loot
- **Guild (GuildHandler):** 18-22 opcodes — create, roster, bank
- **Quest (QuestHandler):** 12-16 opcodes
- **Items (ItemHandler + BankHandler + VoidStorageHandler):** 35-42 opcodes — swap, equip, bank, void
- **Spell / Skill (SpellHandler + SkillHandler):** 13-20 opcodes
- **NPC / Vendor / Trainer (NPCHandler):** 10-15 opcodes — gossip, list inventory, train
- **Auction (AuctionHouseHandler):** 10-12 opcodes
- **Loot (LootHandler):** 8-12 opcodes
- **Mail (MailHandler):** 10-12 opcodes
- **Battlegrounds (BattleGroundHandler):** 15-20 opcodes
- **LFG (LFGHandler):** 15-20 opcodes
- **Calendar (CalendarHandler):** 10-15 opcodes
- **Misc (MiscHandler):** 25-35 opcodes (taxi, tutorial, reset, time sync, etc.)

---

## 4. Top-level methods of `WorldSession::HandleXxx`

(Demasiados para listar todos; los más críticos):

| Class::Method | Purpose | Calls into |
|---|---|---|
| `WorldSession::HandlePlayerLoginOpcode` | Carga jugador en mundo | DB queries (~10), `Map::AddPlayerToMap`, init packets |
| `WorldSession::HandleCharEnumOpcode` | Lista personajes | `CHAR_SEL_ENUM` |
| `WorldSession::HandleCharCreateOpcode` | Crea personaje | `CharacterDatabase::Insert`, validations |
| `WorldSession::HandleCharDeleteOpcode` | Elimina (soft) | `CHAR_DEL_CHARACTER` |
| `WorldSession::HandleMovementOpcodes` | Multi-opcode movement | `Map::PlayerRelocation`, anti-cheat |
| `WorldSession::HandleCastSpellOpcode` | Cast spell | `Spell::prepare`, `SpellMgr::GetSpellInfo` |
| `WorldSession::HandleQuestgiverAcceptQuestOpcode` | Accept quest | `Player::AddQuestAndCheckCompletion` |
| `WorldSession::HandleAuctionListItems` | Browse AH | `AuctionMgr::BuildListAuctionItems` |
| `WorldSession::HandleSendMail` | Mail send | `MailMgr::Send` |
| `WorldSession::HandleGuildCreate` | Create guild | `GuildMgr::CreateGuild` |
| `WorldSession::HandleAttackSwing` | Auto-attack | `Unit::AttackerStateUpdate` |
| `WorldSession::HandleLoginOpcode` (auth) | CMSG_AUTH_SESSION | SRP6 verify, derive session key |

---

## 5. Module dependencies

**Depends on:**
- `shared/Packets/ByteBuffer.h` — deserialización
- `game/Server/Protocol/Opcodes.h` — enums
- `game/Entities/Player/Player.h` — `m_player` ptr en cada handler
- `game/Entities/Object/Object.h` — `ObjectGuid`, `ObjectAccessor`
- Numerosos managers (`AuctionMgr`, `MailMgr`, `GuildMgr`, `SpellMgr`, `LootMgr`, `QuestMgr`, etc.)
- `DatabaseEnv.h` — prepared statements

**Depended on by:**
- `game/World/WorldSession.h` — todos son métodos de `WorldSession`
- `game/Scripting/ScriptMgr.h` — scripts pueden hookear pre/post handler

---

## 6. SQL / DB queries

100+ prepared statements emitidos por handlers. Ejemplos:

| Handler | Statement | DB |
|---|---|---|
| HandlePlayerLogin | `CHAR_SEL_CHARACTER`, `CHAR_SEL_CHARACTER_AURAS`, etc. | character |
| HandleCharDelete | `CHAR_DEL_CHARACTER` | character |
| HandleGuildCreate | `GUILD_CREATE` | character |
| HandleGuildRoster | `GUILD_SEL_MEMBERS` | character |
| HandleAutoEquipItem | `CHAR_UPD_CHARACTER_ITEM_INSTANCE` | character |
| HandleSendMail | `MAIL_INSERT` | character |
| HandleQuestAccept | `CHAR_UPD_QUEST_STATUS` | character |
| HandleCastSpell | `SPELL_SEL_COOLDOWN` | world |

Sin SQL inline; todos prepared.

---

## 7. Wire-protocol opcodes mapping (C++ vs Rust)

**Resumen ejecutivo:** TrinityCore implementa ~260 opcodes únicos repartidos en 45 handlers. RustyCore tiene ~101 opcodes registrados en 13 handler files = **~39% coverage**.

### Character (30 opcodes C++)

| Opcode | C++ | Rust | Status |
|---|---|---|---|
| `CMSG_ENUM_CHARACTERS` | HandleCharEnum | character::handle_enum_characters | ✅ |
| `CMSG_CREATE_CHARACTER` | HandleCharCreate | character::handle_create_character | ✅ |
| `CMSG_CHAR_DELETE` | HandleCharDelete | character::handle_char_delete | ✅ |
| `CMSG_PLAYER_LOGIN` | HandlePlayerLogin | character::handle_player_login | ✅ |
| `CMSG_LOADING_SCREEN_NOTIFY` | HandleLoadScreen | (stub) | ⚠️ |
| `CMSG_CHARACTER_RENAME_REQUEST` | HandleCharRename | (missing) | ❌ |
| `CMSG_CHAR_CUSTOMIZE` | HandleCharCustomize | (missing) | ❌ |
| `CMSG_CHAR_RACE_OR_FACTION_CHANGE` | HandleCharRaceOrFactionChange | (missing) | ❌ |
| `CMSG_ALTER_APPEARANCE` | HandleAlterAppearance | (missing) | ❌ |
| `CMSG_EQUIPMENT_SET_SAVE` | HandleEquipmentSetSave | (missing) | ❌ |
| ... (~20 más) | ... | (missing) | ❌ |

### Chat (25 opcodes C++)

Mostly OK — Rust cubre say/yell/whisper/channel/guild/party/raid/emote, falta channel mod/owner/password/banlist.

### Movement (20 opcodes C++)

Mostly partial — Rust cubre move forward/back/jump/fall/teleport/heartbeat/ack. Faltan opcodes especializados (transport, vehicle, swim, etc.).

### Combat (8 opcodes)

Rust: `attack_swing`, `attack_stop`. Faltan duel-related (5).

### Guilds (20 opcodes) — **TODO MISSING (0%)**

### Auctions (10 opcodes) — **TODO MISSING (0%)**

### Mail (10 opcodes) — **TODO MISSING (0%)**

### Bank (8 opcodes) — **TODO MISSING (0%)**

### LFG (15 opcodes) — **TODO MISSING (0%)**

### Calendar (10 opcodes) — **TODO MISSING (0%)**

### Battlegrounds (15 opcodes) — **TODO MISSING (0%)**

### Items extras (swap/split/transmog/void) — **TODO MISSING**

---

## 8. Current state in RustyCore

**Handler files in `/home/server/rustycore/crates/wow-world/src/handlers/`:**

| File | Opcodes | Coverage of corresponding C++ |
|---|---|---|
| `character.rs` | 13 | ~40% CharacterHandler |
| `chat.rs` | 15 | ~60% ChatHandler |
| `movement.rs` | 12 | ~60% MovementHandler |
| `combat.rs` | 3 | ~40% CombatHandler |
| `spell.rs` | 8 | ~50% SpellHandler |
| `quest.rs` | 7 | ~50% QuestHandler |
| `group.rs` | 6 | ~40% GroupHandler |
| `loot.rs` | 5 | ~50% LootHandler |
| `social.rs` | 4 | ~65% SocialHandler |
| `inspect.rs` | 2 | ~40% InspectHandler |
| `trainer.rs` | 3 | ~30% NPCHandler (trainer subset) |
| `battlenet.rs` | 2 | ~10% BattlenetHandler |
| `misc.rs` | 15 | ~40% MiscHandler |
| **TOTAL** | **~101** | **~39%** |

**What's implemented well:**
- Character login pipeline (enum → create/delete → login)
- Chat messaging (5 main types)
- Basic movement
- Quest basic flow (accept/complete/cancel/progress)
- Social (friends/ignore)
- Combat auto-attack

**What's missing vs C++:**
- **CharacterHandler extras**: rename, customize, race/faction change, equipment sets, faction at war, tutorial flags, alter appearance
- **GuildHandler**: COMPLETO missing (0/20 opcodes)
- **AuctionHouseHandler**: COMPLETO missing (0/10)
- **MailHandler**: COMPLETO missing (0/10)
- **BankHandler**: COMPLETO missing (0/8)
- **ItemHandler extras**: swap, split, transmog, void storage (faltan ~15 opcodes)
- **LFGHandler**: COMPLETO missing (0/15)
- **CalendarHandler**: COMPLETO missing (0/10)
- **BattleGroundHandler**: COMPLETO missing (0/15)
- **PetHandler**: COMPLETO missing (0/12)
- **TradeHandler**: COMPLETO missing (0/12)
- **VoidStorageHandler**: COMPLETO missing
- **TaxiHandler**: COMPLETO missing
- **CombatHandler extras**: duel handlers
- Specialized: BattlePets, Achievements, Scenarios, Instances, Petitions

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- **Handler signature:** C++ `WorldSession::Handle(Packet&)` vs Rust `fn handle(&mut session, &mut packet)` — verificar paso correcto de session_id/player_guid.
- **Async dispatch:** ¿Handlers Rust son async? ¿Cómo se manejan operaciones DB que en C++ son sincrónicas blocking?
- **Error handling:** C++ frecuentemente silencia (log + return). Rust `Result`. ¿Política consistente?

**Tests existing:**
- ~5-10 integration tests en `crates/wow-world/tests/`
- Necesitan fixtures (mock Session, Player, Items)

---

## 9. Migration sub-tasks

**Character handlers (10 missing)**
- [ ] **#HANDLERS.1** CharacterRenameRequest (M)
- [ ] **#HANDLERS.2** CharCustomize (M)
- [ ] **#HANDLERS.3** CharRaceOrFactionChange (H)
- [ ] **#HANDLERS.4** EquipmentSet save/delete/use (M)
- [ ] **#HANDLERS.5** SetFactionAtWar/Watched/Inactive (L)
- [ ] **#HANDLERS.6** AlterAppearance (barber shop) (M)

**Guild handlers (20 missing)**
- [ ] **#HANDLERS.7** GuildCreate/Invite/Leave/Delete (H)
- [ ] **#HANDLERS.8** GuildRoster/EventLog/Permissions (H)
- [ ] **#HANDLERS.9** GuildBank query/deposit/withdraw/swap (XL — split)

**Auction House (10 missing)**
- [ ] **#HANDLERS.10** AuctionList items/bidder/owner/pending (H)
- [ ] **#HANDLERS.11** AuctionPlaceBid/RemoveItem/SellItem (H)

**Mail (10 missing)**
- [ ] **#HANDLERS.12** GetMailList/Body/SendMail (H)
- [ ] **#HANDLERS.13** MailTakeMoney/TakeItem/Delete/Return (M)

**Bank (8 missing)**
- [ ] **#HANDLERS.14** BankAccessRequest/BankerActivate/BuyBackItem (L)
- [ ] **#HANDLERS.15** BuyBankSlot (L)

**Item extras (10 missing)**
- [ ] **#HANDLERS.16** SwapItem/SwapInvItem/SplitItem (M)
- [ ] **#HANDLERS.17** WrapItem/UnwrapItem (L)
- [ ] **#HANDLERS.18** VoidStorageTransfer/Query (M)
- [ ] **#HANDLERS.19** Transmogrification (M)

**Group extras (6 missing)**
- [ ] **#HANDLERS.20** DisbandGroup/PartySwap/ChangeSubGroup (L)
- [ ] **#HANDLERS.21** SetLootMethod/SetRole (M)

**Spell/Skill (5 missing)**
- [ ] **#HANDLERS.22** SkillLearn/LevelUp (L)
- [ ] **#HANDLERS.23** CancelAutoRepeatSpell (L)

**Duel (4 missing)**
- [ ] **#HANDLERS.24** DuelAccept/Cancel/Decline/Forfeit (M)

**NPC/Vendor extras (8 missing)**
- [ ] **#HANDLERS.25** GossipSelectMultipleOption, SpiritHealerActivate, etc. (L-M)

**Misc (20+ missing)**
- [ ] **#HANDLERS.26** Calendar opcodes completos (H, ~6h)
- [ ] **#HANDLERS.27** TaxiActivate/QueryNodes (L-M)
- [ ] **#HANDLERS.28** TutorialSetFlag/RepopRequest/CorpseMoved (L)
- [ ] **#HANDLERS.29** Battleground queue/leave/port (H)
- [ ] **#HANDLERS.30** BlackMarket/Petition extras (L-M)

**Cross-cutting**
- [ ] **#HANDLERS.31** Audit completo opcodes C++ vs Rust; tabla exhaustiva (H, ~3h investigación)
- [ ] **#HANDLERS.32** Documentar error policy (cuándo silenciar vs propagar) (L)

---

## 10. Regression tests to write

- [ ] Character enum → lista coincide con DB
- [ ] Create character → insertado, enumerable
- [ ] Player login → auras cargadas, equipo correcto, posición persistente
- [ ] Chat broadcast → recibido por nearby players, logged si aplica
- [ ] Party invite → invitee puede aceptar/rechazar, queue correcto
- [ ] Quest accept → marca active, progress=0
- [ ] Item swap → slots cambian, stats jugador actualizados
- [ ] Spell cast → cast time, mana deducido, effects aplicados
- [ ] Duel → ambos en arena, winner/loser correctos
- [ ] Guild create → guild en DB, creator es leader, puede invitar

---

## 11. Notes / gotchas

- **`WorldSession` stateful:** mantiene `Player*`, cache, pending tx. En Rust `struct Session` con sync. Verificar locks.
- **Async ops:** `HandlePlayerLogin` hace ~10 queries seq. ¿Rust async? Cuidado executor blocking.
- **Opcodes vary by expansion:** WoLK 3.4.3 != Cata != MoP. Mantener `wow_constants::ClientOpcodes` sync con cliente target.
- **Bit-packing en CMSGs:** `CMSG_CAST_SPELL` puede mezclar flags bit-packed + bytes planos. Ver `shared-packets.md`.
- **Performance hotspot:** `HandlePlayerLogin` blocking en C++ (cientos ms). Async en Rust requiere cuidado.
- **Security:** Todos los handlers DEBEN chequear `session.player()` antes de tocar player state. Missing check = exploitable.
- **`ASSERT(m_player)`:** C++ a menudo asume non-null sin check. Rust `Option<&Player>` — explicit.
- **Script callbacks:** `ScriptMgr::OnBeforePlayerLogout` etc. son hooks. Rust necesita registry equivalente.

---

## 12. C++ → Rust mapping

| C++ Pattern | Rust Pattern | Notas |
|---|---|---|
| `void WorldSession::HandleXxx(Packet&)` | `async fn handle_xxx(session: &mut Session, pkt: &mut WorldPacket) -> Result<(), HandlerError>` | Registro vía `inventory::submit!` |
| `m_player` member | `session.player()` / `player_mut()` | Posiblemente `Option` |
| `GetPlayer()` null check | `session.player().ok_or(HandlerError::NoPlayer)?` | Early return `?` |
| `SendPacket(packet.Write())` | `session.send_packet(&packet)` | Encapsula serialización |
| `sObjectAccessor->FindPlayer(guid)` | `world.find_player(guid)` | World lookup |
| `CharacterDatabase.PreparedStatement()` | `db.prepare(stmt).execute(params).await?` | Async DB |
| Exception | `Result<T, Error>` con `?` | Match en lugar de try-catch |
| `if (!m_player) return` | `let player = session.player()?` | Idiom Rust |

---

*Template version: 1.0 (2026-05-01).*
