# Migration: Commands (GM/dev `.commands`)

> **C++ canonical path:** `src/server/scripts/Commands/` + `src/server/game/Chat/ChatCommands/` + `src/server/worldserver/CommandLine/` + `src/server/worldserver/RemoteAccess/` + `src/server/worldserver/TCSoap/`
> **Rust target crate(s):** *(none yet — `wow-script` and `wow-scripts` are empty placeholders; world-server has no CLI thread; bnet-server has no SOAP)*
> **Layer:** L8 — service / scripting (sits on top of L7 handlers; depends on Accounts/RBAC L1, Chat L6, every gameplay subsystem)
> **Status:** ❌ not started — **confirmed via audit 2026-05-01** (zero references to `ChatCommandTable`, `ChatCommandBuilder`, `RBAC_PERM_COMMAND_*`, `.tele`, `.gm`, `.additem`, `CliRunnable`, `RASession`, or SOAP in any Rust crate; `wow-script*/src/lib.rs` are 0 LOC; the `chat.rs` handler does not even peek at `text[0] == '.'`)
> **Audited vs C++:** ✅ complete — every absence reconfirmed against the workspace
> **Last updated:** 2026-05-01

---

## 1. Purpose

The "commands" subsystem is the entire **GM / dev / RA / SOAP administration surface** of the server. Every `.command` typed by a GM in chat (`. `-prefix or `!`-prefix), every line typed at the server's stdin console, every command piped over the Remote Administration TCP socket, and every `<executeCommand>` SOAP body all funnel through the same dispatcher (`Trinity::ChatCommands::TryExecuteCommand`) which walks a tree of `ChatCommandNode`s, checks the caller's RBAC permission, parses typed arguments, and invokes a static handler. Without this layer there is **no admin testing path**: no `.tele`, no `.gm on`, no `.additem`, no `.npc spawn`, no `.reload`, no `.account create`, no `.ban`. Every QA workflow, every dev-time spawn-and-debug loop, and every live-ops moderation action depends on it.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/`.

### 2a. Dispatcher / framework

| File | Lines (approx) | Purpose |
|---|---|---|
| `src/server/game/Chat/Chat.h` | 168 | `ChatHandler`, `CliHandler`, `AddonChannelCommandHandler` class hierarchy; `SendSysMessage` / `PSendSysMessage` API |
| `src/server/game/Chat/Chat.cpp` | 795 | `ChatHandler::ParseCommands` (.- and !-prefix gate), `_ParseCommands` (delegates to `TryExecuteCommand`), per-handler `GetTrinityString`, `getSelectedPlayer/Unit/Object/Creature` helpers, `extractKeyFromLink` hyperlink scrubber |
| `src/server/game/Chat/ChatCommands/ChatCommand.h` | 280 | `ChatCommandBuilder`, `ChatCommandTable`, `ChatCommandNode`, `CommandInvoker` (variadic-template arg consumer), `Console::Yes/No` enum |
| `src/server/game/Chat/ChatCommands/ChatCommand.cpp` | 482 | `LoadCommandMap` (aggregates from `ScriptMgr::GetChatCommands()` + `world.command` SQL table for help text), `TryExecuteCommand`, `SendCommandHelpFor`, `GetAutoCompletionsFor`, prefix-matching `FilteredCommandListIterator`, `LogCommandUsage` (audit trail to `gm` log channel) |
| `src/server/game/Chat/ChatCommands/ChatCommandArgs.h` | 338 | `ArgInfo<T>` template specializations: how to parse `uint32`, `float`, `Hyperlink<...>`, `Optional<T>`, `Variant<...>`, `PlayerIdentifier`, `AccountIdentifier`, `GameTele const*`, etc. from a `string_view` |
| `src/server/game/Chat/ChatCommands/ChatCommandArgs.cpp` | 136 | Default `ArgInfo` impls + error-message strings |
| `src/server/game/Chat/ChatCommands/ChatCommandTags.h` | 326 | Tag types: `Tail` (consume rest), `WTail` (whitespace-tail), `QuotedString`, `Hyperlink<LinkTag>`, `ExactSequence<"foo">`, `AchievementId`, etc. |
| `src/server/game/Chat/ChatCommands/ChatCommandTags.cpp` | 155 | Tag arg parsers |
| `src/server/game/Chat/ChatCommands/ChatCommandHelpers.h` | 133 | Localized error templates |
| `src/server/game/Chat/ChatCommands/ChatCommandHelpers.cpp` | 30 | (helpers impl) |
| `src/server/game/Chat/Hyperlinks.h` | 549 | `ParseSingleHyperlink`, `LinkValidator<T>`, link-data POD structs (`AchievementLinkData`, `ItemLinkData`, `QuestLinkData`, ~25 link types) |
| `src/server/game/Chat/Hyperlinks.cpp` | 730 | Hyperlink validation; `IsCreatureNameValid`, `IsItemNameValid`, `IsSpellNameValid`, every `LinkValidator<...>::IsTextValid` specialization |
| `src/server/game/Chat/HyperlinkTags.cpp` | (varies) | One `LINKDATA(name, types...)` macro per supported `|H<tag>:...|h[...]|h` link type |

### 2b. CommandScripts — one per top-level subtree (42 files)

`src/server/scripts/Commands/cs_*.cpp`. Each defines a `class <name>_commandscript : public CommandScript { ChatCommandTable GetCommands() const override { ... } static bool Handle...Command(ChatHandler*, args...){...} }` and a free function `AddSC_<name>_commandscript()`. Loader: `cs_script_loader.cpp` (108 LOC) wires all 42 into `AddCommandScripts()` called by world startup.

| File | Lines | Top-level command(s) — visible to GMs as `.<name>` |
|---|---|---|
| `cs_account.cpp` | 1026 | `.account`, `.account create/delete/lock/onlinelist/password/set/addon/email`, `.bnetaccount …` (when feature-gated) |
| `cs_achievement.cpp` | 67 | `.achievement add` |
| `cs_ahbot.cpp` | 208 | `.ahbot items/ratio/rebuild/reload/status` |
| `cs_arena.cpp` | 251 | `.arena create/disband/info/lookup/rename/captain` |
| `cs_ban.cpp` | 765 | `.ban`, `.unban`, `.baninfo`, `.banlist` (each with `account`/`character`/`ip`/`playeraccount` subs) |
| `cs_battlenet_account.cpp` | 390 | `.bnetaccount create/lock/password/link/unlink/listgameaccounts` |
| `cs_bf.cpp` | 141 | `.bf start/stop/switch/timer/enable` (battlefield = Wintergrasp/TolBarad) |
| `cs_cast.cpp` | 228 | `.cast`, `.cast back/dist/self/target/dest` |
| `cs_character.cpp` | 968 | `.character customize/changefaction/changerace/level/rename/reputation/titles/erase/deleted (delete/list/restore/old)`, `.levelup`, `.pdump load/write` |
| `cs_cheat.cpp` | 254 | `.cheat casttime/cooldown/explore/god/power/status/taxi/waterwalk` |
| `cs_debug.cpp` | 1799 | `.debug …` ~80 sub-commands: anim, arena, bg, boundary, combat, conversation, dummy, entervehicle, getitemstate, getitemvalue, hostil, instance, itemexpire, lootrecipient, los, mod32value, moveflags, neargraveyard, objectcount, opcode, packet, pathing, phase, play, pvp, raidreset, scene, send, setbit, setvalue, setvid, spawnvehicle, spell, stealth, summon, threat, tradestatus, transport, transportRotation, uistate, update, vehicle, visibility, wsexpression, ... |
| `cs_deserter.cpp` | 187 | `.deserter bg/instance add/remove (player/account)` |
| `cs_disable.cpp` | 354 | `.disable add/remove (achievement_criteria/battleground/map/mmap/outdoorpvp/quest/spell/vmap)` |
| `cs_event.cpp` | 190 | `.event info/activelist/start/stop` |
| `cs_gm.cpp` | 246 | `.gm`, `.gm chat/fly/ingame/list/visible` |
| `cs_go.cpp` | 623 | `.go xyz/zonexy/grid/graveyard/creature/object/quest/taxinode/ticket/instance/bugticket/complaintticket/suggestionticket/areatrigger/offset` |
| `cs_gobject.cpp` | 636 | `.gobject add/delete/info/move/near/relocate/set/spawngroup/despawngroup/target/turn/activate/state/info` |
| `cs_group.cpp` | 531 | `.group disband/leader/list/remove/summon/join/set assistant/maintank/mainassist`, `.lfg ...` partial |
| `cs_guild.cpp` | 309 | `.guild create/delete/invite/uninvite/rank/info` |
| `cs_honor.cpp` | 120 | `.honor add/addkill/update` |
| `cs_instance.cpp` | 269 | `.instance listbinds/unbind/stats/savedata/setbossstate/getbossstate` |
| `cs_learn.cpp` | 481 | `.learn`, `.unlearn`, `.learn all/all_lang/all_my_class/all_my_pettalents/all_my_spells/all_my_talents/all_recipes/debug` |
| `cs_lfg.cpp` | 182 | `.lfg player/group/queue/clean/options` |
| `cs_list.cpp` | 731 | `.list creature/item/object/auras/mail/scenes/spawnpoints/respawns` |
| `cs_lookup.cpp` | 1551 | `.lookup area/creature/event/faction/item (id)/itemset/object/quest (id)/skill/spell/spell id/taxinode/tele/title/map (id)/player (account/email/ip)` |
| `cs_message.cpp` | 257 | `.announce`, `.notify`, `.gmannounce`, `.gmnameannounce`, `.gmnotify`, `.nameannounce`, `.whispers`, `.bcaster`, `.spectator` |
| `cs_misc.cpp` | 2687 | `.additem`, `.additemset`, `.appear`, `.summon`, `.commands`, `.demorph`, `.die`, `.damage`, `.fearme`, `.freeze`, `.unfreeze`, `.gps`, `.guid`, `.hidearea`, `.kick`, `.linkgrave`, `.maxskill`, `.mute`, `.unmute`, `.movegens`, `.neargrave`, `.password`, `.pinfo`, `.playall`, `.respawn`, `.revive`, `.repairitems`, `.saveall`, `.setskill`, `.showarea`, `.start`, `.unaura`, `.wchange`, `.bindsight`, `.unbindsight`, `.combatstop`, `.cooldown`, `.distance`, `.getmovespeed`, … (the historical "miscellaneous" dumping ground — ~60 commands) |
| `cs_mmaps.cpp` | 308 | `.mmap loadedtiles/path/loc/stats/testarea/test` |
| `cs_modify.cpp` | 1056 | `.modify hp/mana/rage/runicpower/energy/money/speed (run/walk/swim/fly/all/backwards)/scale/bit/faction/spell/talentpoints/skill/standstate/morph/phase/gender/drunk/repair/honor/rep/arenapoints/xp/currency` |
| `cs_npc.cpp` | 1435 | `.npc add (item/temp/move/formation)/delete (item)/info/listloot/near/playemote/say/textemote/whisper/yell/follow (stop)/set (entry/factionid/flag/level/movetype/spawndist/spawntime/data/phase/link/model)/spawngroup/despawngroup/showloot/evade` |
| `cs_pet.cpp` | 209 | `.pet create/learn/unlearn/level` |
| `cs_quest.cpp` | 303 | `.quest add/complete/objective complete/remove/reward` |
| `cs_rbac.cpp` | 300 | `.rbac account permission list/grant/deny/revoke`, `.rbac list permissions/roles` |
| `cs_reload.cpp` | 1168 | `.reload …` ~120 sub-commands, one per loadable table (`creature_template`, `quest_template`, `gameobject_template`, `spell_proc`, `gossip_menu`, `command`, `rbac`, `points_of_interest`, `game_event`, `auctions`, `achievements`, `disables`, `creature_text`, `mail_loot_template`, `prospecting_loot_template`, …) — each is a thin wrapper around a `sObjectMgr->Load*()` |
| `cs_reset.cpp` | 317 | `.reset achievements/honor/level/spells/stats/talents/all` |
| `cs_scene.cpp` | 120 | `.scene debug/play/playpackage/cancel` |
| `cs_send.cpp` | 266 | `.send mail/items/money/message` |
| `cs_server.cpp` | 530 | `.server info/motd/set (motd/closed/loglevel)/shutdown (cancel/idlerestart/restart)/exit/idlerestart/restart/shutdown` (force variants), `.server debug` |
| `cs_tele.cpp` | 390 | `.tele`, `.tele add`, `.tele del`, `.tele name <player> <loc>`, `.tele name npc id/guid/name`, `.tele group` |
| `cs_ticket.cpp` | 431 | `.ticket bug/complaint/suggestion (assign/close/closedlist/comment/delete/list/unassign/view)`, `.ticket reset all/bug/complaint/suggestion` |
| `cs_titles.cpp` | 201 | `.titles add/remove/setmask/current` |
| `cs_wp.cpp` | 508 | `.wp add/event/load/modify/unload/reload/show` (waypoint editor for creature paths) |
| `cs_script_loader.cpp` | 108 | Forward-declares + calls the 42 `AddSC_*_commandscript()` registrars |

**Total: 23 101 LOC of command handlers + ~4 000 LOC of dispatcher/parser/hyperlink framework ≈ 27 KLOC.**

### 2c. Console + RA + SOAP entry points

| File | Lines | Purpose |
|---|---|---|
| `src/server/worldserver/CommandLine/CliRunnable.cpp` | 180 | `CliThread` reads stdin via `readline`/`linenoise`, calls `CliHandler::ParseCommands` → same `TryExecuteCommand` (with `IsConsole() == true`, RBAC bypassed for console-allowed commands) |
| `src/server/worldserver/CommandLine/CliRunnable.h` | 29 | Thread interface |
| `src/server/worldserver/RemoteAccess/RASession.h` | 56 | TLS-less TCP shell — accepts `username/password`, authenticates against `account` + RBAC `RBAC_PERM_EMAIL_CONFIRM_FOR_PASS_CHANGE`-style perms, then loops on `\n`-delimited commands |
| `src/server/worldserver/RemoteAccess/RASession.cpp` | 192 | Reads line, calls a `CliHandler`-like wrapper, writes response back to socket; per-line audit log to `ra` channel |
| `src/server/worldserver/TCSoap/TCSoap.h` | ~70 | gSOAP-generated `SOAPCommand` struct + `process_message` glue |
| `src/server/worldserver/TCSoap/TCSoap.cpp` | ~150 | Parses XML-RPC `<executeCommand><command>…</command><username>…</username><password>…</password></executeCommand>`, authenticates, runs via `CliHandler::ParseCommands`, returns the captured `SendSysMessage` buffer in the response |

> Per the brief: RA + SOAP are **bnetserver-adjacent in spirit (auth) but live in the worldserver process** — the actual command execution always reuses the same `ChatCommandNode::TryExecuteCommand`. The migration mention is brief because once the in-game dispatcher exists, plugging a stdin loop or a TCP socket on top is plumbing.

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `ChatHandler` | class (in-game) | Wraps a `WorldSession*`. `ParseCommands(text)` is the per-message entrypoint (called from `WorldSession::HandleMessagechatOpcode` after detecting a `.` or `!` prefix). Owns `m_session`, `m_sentErrorMessage`, `getSelected*` helpers. |
| `CliHandler` | class : ChatHandler | Console variant — `m_session == nullptr`, `IsConsole()==true`; `GetTrinityString` reads the configured DBC locale instead of the per-session locale. |
| `AddonChannelCommandHandler` | class : ChatHandler | Receives commands from the addon channel (binary `0x05` chat type), strips the addon prefix, then delegates. |
| `Trinity::ChatCommands::ChatCommandBuilder` | struct | The literal entry the user writes inside `ChatCommandTable` — `{ "name", &handler, RBAC_PERM_*, Console::Yes/No }` or `{ "name", subTable }`. Stored as a `std::variant<InvokerEntry, SubCommandEntry>`. |
| `Trinity::ChatCommands::ChatCommandTable` | typedef | `std::vector<ChatCommandBuilder>`. |
| `Trinity::Impl::ChatCommands::ChatCommandNode` | class | Built tree node — keyed by `string_view` (case-insensitive), holds the `CommandInvoker`, `CommandPermissions`, help text, and a `std::map` of sub-nodes. The actual runtime lookup uses `ChatSubCommandMap` (top-level + nested). |
| `CommandInvoker` | struct | Type-erased `bool(*)(ChatHandler*, string_view)` plus a `void*` to the typed handler. The wrapper unpacks the `string_view` into a tuple of typed args via `ArgInfo<T>::TryConsume` and `std::apply`s the real handler. |
| `CommandPermissions` | struct | Pair of `RBACPermissions` + `Console::Yes/No` (whether stdin/RA/SOAP can run it without a player). |
| `Console` | enum class : bool | `Yes` / `No`. Console `No` = needs a `WorldSession` (handler dereferences `handler->GetSession()->GetPlayer()` somewhere). |
| `Trinity::ChatCommands::Tail` | tag struct | Consume the rest of the input as a free string. |
| `Trinity::ChatCommands::WTail` | tag struct | Whitespace-preserving tail (used for `.announce`, `.send mail` body). |
| `Trinity::ChatCommands::QuotedString` | tag struct | Reads a `"..."`-quoted token. |
| `Trinity::ChatCommands::Hyperlink<LinkTag>` | template | Parses `|c…|H<tag>:<data>|h[<text>]|h|r` and yields the typed link payload (e.g. `Hyperlink<ItemLinkData>`). |
| `Trinity::ChatCommands::ExactSequence<"…">` | template | Literal-keyword arg (e.g. `"on"` / `"off"`). |
| `Trinity::ChatCommands::PlayerIdentifier` | class | "Either a `|Hplayer:Name|h…|h` link, a `\"Quoted Name\"`, or a bare word"; resolves online → offline (`CharacterCache`) → fail. Used by ~half of all GM commands. |
| `Trinity::ChatCommands::AccountIdentifier` | class | Same idea for accounts (name or numeric id). |
| `Trinity::ChatCommands::ChatCommandResult` | typedef | `Optional<string_view>` carrying either the unconsumed tail or an error message. |
| `Trinity::Hyperlinks::HyperlinkInfo` | struct | `{ tail, color, tag, data, text }` after `ParseSingleHyperlink`. |
| `Trinity::Hyperlinks::*LinkData` | structs (~25) | `AchievementLinkData`, `ItemLinkData`, `QuestLinkData`, `SpellLinkData`, `BattlePetLinkData`, `EnchantLinkData`, `GarrFollowerLinkData`, … one per supported link type. |
| `rbac::RBACPermissions` | enum (~880 values, ~440 of them `_COMMAND_*`) | Per-command permission token. Loaded from `rbac_permissions` + linked-to-roles via `rbac_linked_permissions` + per-account-overrides via `rbac_account_permissions`. |
| `CommandScript` | abstract class (`ScriptObject` derivative) | Abstract base every `cs_*.cpp` inherits. Single virtual: `ChatCommandTable GetCommands() const`. Registered globally in `ScriptMgr::_scripts<CommandScript>`. |
| `ScriptMgr::GetChatCommands()` | function | Walks every registered `CommandScript`, concatenates their `GetCommands()` results into one big `ChatCommandTable`. Called once by `ChatCommandNode::LoadCommandMap` at startup and on `.reload commands`. |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `ChatHandler::ParseCommands(string_view text)` | Message-entry gate. Rejects unless `text` starts with `.` or `!`, length ≥ 2, second char ≠ first, second char ≠ delimiter. Strips prefix, delegates to `_ParseCommands`. | `_ParseCommands` |
| `ChatHandler::_ParseCommands(string_view)` | Wraps `Trinity::ChatCommands::TryExecuteCommand`. On failure, suppresses output for non-GM unless `RBAC_PERM_COMMANDS_NOTIFY_COMMAND_NOT_FOUND_ERROR`; otherwise sends `LANG_CMD_INVALID`. | `TryExecuteCommand`, `PSendSysMessage` |
| `ChatCommandNode::TryExecuteCommand(handler, cmdStr)` | Tokenize command string by `' '` (`COMMAND_DELIMITER`), walk the sub-command map case-insensitively with prefix matching, complain on ambiguity, invoke the leaf node's `_invoker(handler, oldTail)` with the unconsumed tail. Logs success via `LogCommandUsage`. | `FilteredCommandListIterator`, `LogCommandUsage`, `SendCommandHelp` |
| `ChatCommandNode::SendCommandHelpFor(handler, cmdStr)` | Implements `.help <cmd>` — walks the same tree, prints `LANG_CMD_HELP_GENERIC` + sub-commands. | `SendCommandHelp` |
| `ChatCommandNode::GetAutoCompletionsFor(handler, cmdStr)` | Used by `CliRunnable` for tab-completion. Returns vector of full token paths. | `FilteredCommandListIterator` |
| `ChatCommandNode::LoadCommandMap()` | Build the global `COMMAND_MAP` once at startup: call `sScriptMgr->GetChatCommands()`, then merge per-command help text from the `world.command` SQL table (overrides `LANG_*` strings if an entry exists). Called again by `.reload commands`. | `ScriptMgr::GetChatCommands`, `WorldDatabase.Query(WORLD_SEL_COMMANDS)` |
| `ChatCommandNode::LoadFromBuilder(builder)` | One node — copy invoker/perms/help from an `InvokerEntry`, or recurse into a `SubCommandEntry`. Asserts no duplicate blank ("") sub-command. | (recursive) |
| `ChatCommandNode::IsInvokerVisible(handler)` | True if the handler has the required RBAC permission **and** (we're in-game OR the command is `Console::Yes`). | `Player::HasPermission` (in-game) — always true for console |
| `ChatCommandNode::ResolveNames(name)` | Post-load pass: backfill each node's `_name` to the dot-joined full path (`tele.name.npc.id`) for help-message printing. | (recursive) |
| `LogCommandUsage(session, perm, cmdStr)` | Audit-trail GM activity to the `gm` log channel — captures account, char name, position, map, area, zone, target. Skipped if account is a normal player and the perm is in the player role's linked-perm set. | `sLog->OutCommand` |
| `Trinity::Hyperlinks::ParseSingleHyperlink(str)` | Validates `|c<8 hex>|H<tag>:<data>|h[<text>]|h|r` shape and returns a `HyperlinkInfo`. Mostly used to sanitize player-typed links *before* DB lookup. | (no calls) |
| `Trinity::Hyperlinks::ValidateLinks(str)` | Walks every hyperlink in a chat string and runs `LinkValidator<T>::IsTextValid` / `IsColorValid` for each (so a player can't fabricate a fake "+2 to Stamina" item). | per-tag `IsCreatureNameValid`, `IsItemNameValid`, … |
| `ArgInfo<T>::TryConsume(value, handler, args)` | The whole typed-arg system. ~30 specializations: integers, floats, `std::string`, `Tail`, `QuotedString`, `Hyperlink<…>`, `Optional<…>`, `Variant<…>`, `PlayerIdentifier`, etc. | (per-T parsing) |
| `ChatHandler::SendSysMessage(string)` / `(uint32 entry)` | Send a system chat packet to the GM. Splits on `\n`. The `uint32` overload looks up `LANG_*` via `GetTrinityString` (locale-aware: per-session locale in-game, `sObjectMgr->GetTrinityStringForDBCLocale` for console). | `WorldPackets::Chat::Chat::Initialize`, `m_session->SendPacket` |
| `ChatHandler::PSendSysMessage(LANG_*, args...)` | `Trinity::StringFormat`'d variant. The single most-called function inside every command handler — typed args, then dump output to the GM. | `SendSysMessage` |
| `ChatHandler::needReportToTarget(target)` | Used to decide whether to mirror an action message ("[GM] teleported you") to the target's chat window. | `Player::IsGameMaster`, security checks |
| `ChatHandler::HasLowerSecurity(target, guid)` | Prevents GM-level-3 from `.kick`/`.ban`/`.appear`-ing GM-level-4. The single most-overlooked check. | `AccountMgr::GetSecurity` |
| `ChatHandler::extractKeyFromLink(text, …)` / `extractPlayerNameFromLink(text)` | Pulls a numeric id (`item:12345`) or player name out of a click-pasted hyperlink. Used pervasively. | `ParseSingleHyperlink` |
| `CliRunnable::ThreadFunc()` | stdin loop in worldserver: `while (sWorld->IsStopped()==false) { line = readline(); CliHandler{}.ParseCommands(line); }`. Uses `linenoise` for tab-completion via `GetAutoCompletionsFor`. | `CliHandler::ParseCommands` |
| `RASession::HandleAuthentication()` then `Process()` | TCP socket: send `Username:`, read line, send `Password:`, read line, `AccountMgr::CheckPassword` + RBAC `RBAC_PERM_COMMANDS_USE_REMOTEACCESS`. On success loop on lines, run via captured `CliHandler`. | `AccountMgr::CheckPassword`, `CliHandler::ParseCommands` |
| `process_message(soap*)` (TCSoap) | Parse `<executeCommand>`, auth, run command, capture `SendSysMessage` into a `std::string`, return as response body. | `CliHandler::ParseCommands` |

---

## 5. Module dependencies

**Depends on:**

- **`Accounts/RBAC`** — every leaf node has a `RBACPermissions` and `Player::HasPermission(perm)` is the gate (cross-ref `accounts.md`). Without RBAC, the dispatcher cannot authorize.
- **`Chat/Chat.cpp`** — `ChatHandler::ParseCommands` is invoked from `WorldSession::HandleMessagechatOpcode` whenever the user-typed text begins with `.` or `!`. Depends on the `Chat` module to format `SMSG_MESSAGECHAT` system replies (cross-ref `chat.md`).
- **`Chat/Hyperlinks`** — typed-arg parsing for any `Hyperlink<T>` cell (e.g. `.lookup item [Item:12345]`). Validation also sanity-checks player-pasted links inside the message body before any handler runs.
- **`Globals/ObjectMgr`** — `GetTrinityString(entry, locale)`; loads the `trinity_string` table with localized command output strings; `GameTele` storage for `.tele <name>`; `PlayerNameMapHolder` for `PlayerIdentifier`.
- **`Scripting/ScriptMgr`** — `sScriptMgr->GetChatCommands()` is the registration aggregator. Every `cs_*.cpp` inherits `CommandScript` (a `ScriptObject` subclass) and is collected at startup.
- **`Logging/Log`** — `sLog->OutCommand(...)` writes the GM audit trail.
- **DB2/DBC stores** — many handlers (`.lookup achievement`, `.lookup item`, `.go zonexy`, `.npc info`) read directly from `sCreatureTemplateStore`, `sItemSparseStore`, `sAreaTableStore`, `sMapStore`, etc.
- **WorldDatabase, CharacterDatabase, LoginDatabase** — `.account create`, `.ban`, `.character deleted`, `.tele add`, `.reload <table>` all run direct SQL through prepared statements.
- **The entire game logic** — every `Player::*`, `Creature::*`, `Spell::Cast`, `Map::*`, `Quest*`, `Battleground::*`, `Guild::*`, `MailDraft::*`, `Pet::*` is reachable from some `.command`. This is what makes `commands` an L8 "leaf" — it transitively depends on virtually every module below.

**Depended on by:**

- `worldserver` main — instantiates `CliThread` and the SOAP dispatcher.
- Live ops / GM workflow — every moderation action (`.ban`, `.mute`, `.kick`) and every dev-time iteration (`.tele <named loc>`, `.additem 12345`, `.npc spawn`, `.reload spell_proc`) routes through here.
- QA — there is no out-of-band test harness in TC; QA writes scripts that pipe `.commands` over RA/SOAP.

---

## 6. SQL / DB queries (if any)

The dispatcher itself owns one query; the 42 handlers collectively run several hundred (every prepared statement in `World*Statements.cpp` whose name mentions `_COMMAND_` or `_GM_`).

Dispatcher-level:

| Statement / Source | Purpose | DB |
|---|---|---|
| `WORLD_SEL_COMMANDS` (`SELECT name, help FROM command`) | Help-text override per-command | world |

Per-handler representative samples (each `cs_*.cpp` has its own; full enumeration is the implementation's problem, not the doc's):

| Statement | Source | Purpose | DB |
|---|---|---|---|
| `LOGIN_INS_ACCOUNT`, `LOGIN_DEL_ACCOUNT`, `LOGIN_UPD_ACCOUNT`, `LOGIN_UPD_LOGON` (~30 of these) | `cs_account.cpp` | `.account create/delete/lock/password` | login (auth) |
| `LOGIN_INS_ACCOUNT_BANNED`, `LOGIN_INS_IP_BANNED`, `LOGIN_DEL_ACCOUNT_BANNED`, `LOGIN_SEL_BANS` | `cs_ban.cpp` | `.ban`, `.unban`, `.baninfo`, `.banlist` | login |
| `CHAR_DEL_CHARACTER`, `CHAR_SEL_DELETED_CHARS`, `CHAR_UPD_RESTORE_DELETED_CHAR` | `cs_character.cpp` | `.character erase/deleted (list/restore)` | characters |
| `WORLD_INS_GAME_TELE`, `WORLD_DEL_GAME_TELE` | `cs_tele.cpp` | `.tele add/del` | world |
| `WORLD_INS_CREATURE`, `WORLD_DEL_CREATURE`, `WORLD_UPD_CREATURE_SPAWN_DIST`, `WORLD_UPD_CREATURE_SPAWN_TIME` | `cs_npc.cpp` | `.npc add/delete/set spawndist/spawntime` | world |
| `WORLD_INS_GAMEOBJECT`, `WORLD_DEL_GAMEOBJECT` | `cs_gobject.cpp` | `.gobject add/delete` | world |
| `LOGIN_INS_RBAC_ACCOUNT_PERMISSION`, `LOGIN_DEL_RBAC_ACCOUNT_PERMISSION` | `cs_rbac.cpp` | `.rbac account permission grant/deny/revoke` | login |
| `WORLD_INS_WAYPOINT_DATA`, `WORLD_DEL_WAYPOINT_DATA`, `WORLD_UPD_WAYPOINT_DATA_POS` | `cs_wp.cpp` | `.wp add/modify/del` | world |
| `WORLD_INS_GUILD`, `CHAR_DEL_GUILD`, `CHAR_INS_GUILD_MEMBER` | `cs_guild.cpp` | `.guild create/delete/invite` | world+characters |
| (~120 more) | `cs_reload.cpp` | each `.reload <table>` doesn't *itself* re-issue SELECTs; it calls `sObjectMgr->Load*()` which does. The handler is just a router. | world (mostly) |

DBC/DB2 stores read by command handlers (representative):

| Store | What it loads | Read by |
|---|---|---|
| `sCreatureDisplayInfoStore`, `sCreatureModelDataStore` | Display / model data | `.morph`, `.npc info`, `.lookup creature` |
| `sItemSparseStore`, `sItemStore` | Items | `.additem`, `.lookup item`, `.modify scale`, hyperlink validation |
| `sSpellNameStore`, `SpellMgr->GetSpellInfo` | Spells | `.cast`, `.learn`, `.lookup spell`, `.aura` |
| `sAchievementStore` | Achievements | `.achievement add`, `.lookup achievement` |
| `sAreaTableStore`, `sMapStore` | Areas + maps | `.go zonexy`, `.gps`, `.lookup map`, `.tele add`, `LogCommandUsage` audit |
| `sFactionStore`, `sFactionTemplateStore` | Reputation | `.modify faction`, `.character reputation`, `.lookup faction` |
| `sQuestV2Store` (and `ObjectMgr::GetQuestTemplate`) | Quests | `.quest add/complete/remove`, `.lookup quest` |
| `sGameEventStore`-equivalent (`GameEventMgr`) | Events | `.event start/stop/info` |
| `sGameTeleStore` (`ObjectMgr`) | `.tele` named locations | `.tele`, `.tele del` |
| `sLanguageStore` (`trinity_string`) | Localized command output | every `PSendSysMessage(LANG_*)` |

---

## 7. Wire-protocol packets (if any)

Commands are not a wire protocol per se. They are **content carried inside `CMSG_MESSAGECHAT*`** when the leading character is `.` or `!`. The reply path is `SMSG_MESSAGECHAT` with `CHAT_MSG_SYSTEM`, `LANG_UNIVERSAL`, blank sender (`nullptr` GUID and name), one packet per `\n`-split line.

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_MESSAGECHAT_*` (Say, Yell, Party, Guild, Whisper, …) | client → server | `WorldSession::HandleMessagechatOpcode` (or per-channel variant) inspects the body, sees `text[0] == '.'`, and instead of routing to the `Chat` broadcaster passes the body to `ChatHandler::ParseCommands`. |
| `SMSG_MESSAGECHAT` with `CHAT_MSG_SYSTEM` / `CHAT_MSG_RAID_BOSS_EMOTE` (for `.bcaster`) / `CHAT_MSG_RAID_WARNING` (for `.gmnotify`) | server → client | `ChatHandler::SendSysMessage` (one per `\n` line), `WorldPackets::Chat::Chat::Initialize`. |
| `SMSG_MESSAGECHAT` with `CHAT_MSG_GUILD_ACHIEVEMENT` and various other types | server → client | Specific handlers (`.gmannounce`, `.announce`, `.gmnameannounce`, etc.) build a different `ChatMsg` value. |

**Console / RA / SOAP** do not use any opcode — they read/write raw text on a TCP socket / stdin / XML body. The same `SendSysMessage` path is captured into a `std::string` buffer instead of being serialized into a `WorldPacket`.

**Hyperlink wire format** (consumed when parsing arguments and when validating chat content):

```
|c<8-hex AABBGGRR>|H<tag>:<data>|h[<text>]|h|r
```

Examples seen as command arguments:

```
.additem |cffa335ee|Hitem:19019:0:0:0:0:0:0:0:80|h[Thunderfury, Blessed Blade of the Windseeker]|h|r 1
.lookup spell |cff71d5ff|Hspell:133|h[Fireball]|h|r
.appear |Hplayer:Arthas|h[Arthas]|h|r
.kick |Hplayer:Bot01|h[Bot01]|h|r reason text
```

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**

- `crates/wow-script/src/lib.rs` — **0 LOC** (empty placeholder).
- `crates/wow-scripts/src/lib.rs` — **0 LOC** (empty placeholder).
- `crates/wow-handler/src/lib.rs` — 116 LOC. Defines `PacketHandlerEntry` for the `inventory::submit!` static-registration pattern used for **opcode** handlers. Has nothing to do with `.commands`.
- `crates/wow-chat/src/lib.rs` — **0 LOC**.
- `crates/wow-world/src/handlers/chat.rs` — 413 LOC. Handles `CMSG_CHAT_MESSAGE_SAY/YELL/PARTY/GUILD/RAID/WHISPER/EMOTE/...`; broadcasts to nearby players. **Does not check `text[0] == '.'`. Does not call any command dispatcher. Does not import any "command" symbol.** A GM typing `.tele` today gets it broadcast as ordinary `Say` chat to nearby players.
- `crates/world-server/src/main.rs` — server entry point. **No CLI thread. No stdin loop. No `tokio::io::stdin` reader. No RA listener. No SOAP listener.**
- `crates/bnet-server/src/...` — only handles BNet auth + REST. **No SOAP endpoint.**

**What's implemented:**

- *Nothing.* The chat opcode handlers exist and accept text, but text starting with `.` is broadcast as chat, not parsed as a command.

**What's missing vs C++:**

- The entire `Trinity::ChatCommands` dispatcher (`ChatCommandNode`, `ChatCommandTable`, `ChatCommandBuilder`, `CommandInvoker`, `ArgInfo<T>` parsers, `Tail`/`QuotedString`/`Hyperlink<T>`/`Variant<T>`/`Optional<T>` tag types, prefix matching, ambiguity reporting, autocompletion).
- The `ChatHandler` / `CliHandler` / `AddonChannelCommandHandler` triad and the `ParseCommands` gate (`.` / `!` prefix detection, the second-char-not-equal-first sanity check).
- The `Hyperlinks::ParseSingleHyperlink` + `LinkValidator<T>` system (~25 link types). Without this, no command can accept a click-pasted `[Item:12345]` argument. **Also blocks chat-spoofing protection** — players can craft fake "+1000 Sta" item links.
- `RBACPermissions` enum (~440 `_COMMAND_*` values out of ~880 total). Every handler has a permission token; the dispatcher refuses to invoke without it.
- The `LogCommandUsage` audit pipeline → `sLog->OutCommand` → `gm` log channel. Live-ops compliance hard requirement.
- All 42 `cs_*.cpp` content scripts (23 KLOC of handlers).
- `world.command` table loader (per-command help-text overrides). Schema: `name VARCHAR(255), help TEXT`.
- `trinity_string` / `acore_string` localized output table loader. Used by every `LANG_*` lookup; enables locale-aware command output.
- `CliRunnable` (stdin reader thread for the worldserver process) — required for ops to type at the live server console.
- `RASession` (RA TCP socket: auth + per-line dispatch).
- `TCSoap` / SOAP XML-RPC endpoint.
- `tab completion` via `linenoise` / `readline` (depends on `GetAutoCompletionsFor`).

**Suspicious / likely divergent (hipótesis pre-auditoría):**

- N/A — there is no Rust implementation to be divergent. The hypothesis pre-audit is "100% absent". Confirmed in §13.

**Tests existing:**

- 0 tests touching commands, command dispatch, hyperlinks, RBAC permissions, or RA/SOAP.

---

## 9. Migration sub-tasks

Numbered for cross-reference from `MIGRATION_ROADMAP.md`. Complexity: **L** <1h, **M** 1-4h, **H** 4-12h, **XL** >12h (split).

The order matters — a working `.tele` is the smallest end-to-end demo; everything else compounds on the dispatcher.

- [ ] **#CMD.1** Decide on the typed-argument parser strategy in Rust. Recommended: a derive-macro on the handler `fn` (or a `#[command(...)]` attribute) that generates the `ArgInfo` consumers from the function signature, so `fn handle_tele_add(h: &mut ChatHandler, name: String) -> bool` produces a parser that consumes a `Tail` into `name`. Alternative: hand-rolled `nom`-style combinators per arg type. **Locked-in choice required before #CMD.4.** Complejidad: **M**.
- [ ] **#CMD.2** Implement `wow-chat::commands::dispatcher` — `CommandNode { name, invoker, permission, console, help, sub: BTreeMap<UniCase<String>, CommandNode> }`, case-insensitive prefix match, ambiguity detection (mirror `FilteredCommandListIterator`), recursive `try_execute(handler, cmdStr)`. Complejidad: **H**.
- [ ] **#CMD.3** Implement `ChatHandler` analogue in `wow-chat` — wrapper over `WorldSession`, `send_sys_message(string)`, `psend_sys_message(LANG_id, args...)`, `get_selected_player/unit/object/creature`, `has_lower_security`. Hook into `wow-world/src/handlers/chat.rs`: detect `.` / `!` prefix in `ChatMessageSay`/`Whisper`/etc. *before* broadcasting; on hit, route into the dispatcher and **swallow the broadcast**. Complejidad: **M**.
- [ ] **#CMD.4** Port `ArgInfo` for the basic types: `u8/i8/u16/i16/u32/i32/u64/i64`, `f32/f64`, `bool` (`on`/`off`/`1`/`0`), `String` (single token), `Tail`, `QuotedString`, `Optional<T>`, `Variant<A,B,...>`, `ExactSequence<&'static str>`. Complejidad: **H**.
- [ ] **#CMD.5** Port hyperlink parsing: `parse_single_hyperlink(str) -> HyperlinkInfo`, then per-link-type structs (`ItemLinkData`, `SpellLinkData`, `PlayerLinkData`, `QuestLinkData`, `AchievementLinkData`, …). Wire `Hyperlink<T>` arg-type. Complejidad: **H** (split per link family if it grows).
- [ ] **#CMD.6** Port `LinkValidator<T>::IsTextValid` for the same set, sourced from DB2 stores (`wow-data`). This is the chat anti-spoof barrier; not optional even for read-only commands. Complejidad: **H**.
- [ ] **#CMD.7** Port `RBACPermissions` enum (mirror cs `RBAC.h` byte-for-byte: same numeric values so DB rows are interchangeable). Wire `Player::has_permission(perm)` → `WorldSession.account.rbac.has_permission(perm)`. Cross-ref `accounts.md` #ACC.8-12. Complejidad: **M** (but blocked on accounts.md migration starting).
- [ ] **#CMD.8** Implement `ChatCommandNode::load_command_map` registrar. Crate-level decision: use the `inventory::submit!` static-registration pattern already in use for opcode handlers (`PacketHandlerEntry`) so `cs_*.rs` files can each just `inventory::submit! { CommandRegistration { name: "tele.add", invoker: ..., permission: TELE_ADD, console: false, help: LANG_COMMAND_TELE_ADD_HELP } }`. Avoids a giant `register_all()` switchboard. Complejidad: **M**.
- [ ] **#CMD.9** Localized strings: load the `trinity_string` table at startup → in-memory `HashMap<u32, [String; LOCALE_COUNT]>`; `get_trinity_string(entry, locale)` resolution. Cross-ref `texts.md`. Complejidad: **M**.
- [ ] **#CMD.10** Per-command help-text override: load `world.command` table at startup; merge into the command tree post-construction (mirrors `LoadCommandMap` second pass). Complejidad: **L**.
- [ ] **#CMD.11** `LogCommandUsage` audit pipeline: dedicated `tracing` target `gm` (or a separate appender) capturing `account_id, char_name, char_guid, x, y, z, map, area, zone, target, command_text`. Complejidad: **L**.
- [ ] **#CMD.12** First handler set — `cs_tele.rs` (`.tele`, `.tele add`, `.tele del`, `.tele name`, `.tele group`). Smallest meaningful demo; exercises the whole pipeline including the `GameTele` lookup. Complejidad: **M**.
- [ ] **#CMD.13** `cs_gm.rs` — `.gm`, `.gm chat/fly/ingame/list/visible`. Required to enable GM mode, which gates many other commands. Complejidad: **M**.
- [ ] **#CMD.14** `cs_misc.rs` core subset — `.additem`, `.appear`, `.summon`, `.commands` (the `.commands` command itself: lists what the caller can run), `.gps`, `.kick`, `.die`, `.revive`, `.unaura`. The dev-iteration baseline. Complejidad: **H** (split: ~60 commands; do the 10 most-used first). Depends on `Player`/`Unit`/`Map` mutator APIs in `wow-world` already existing.
- [ ] **#CMD.15** `cs_npc.rs` core subset — `.npc add`, `.npc delete`, `.npc info`, `.npc near`, `.npc set level`, `.npc set faction`. Required for spawning content during dev. Complejidad: **H**.
- [ ] **#CMD.16** `cs_gobject.rs` core subset — `.gobject add`, `.gobject delete`, `.gobject info`, `.gobject near`, `.gobject move`. Complejidad: **H**.
- [ ] **#CMD.17** `cs_modify.rs` — money, hp, mana, scale, speed, faction, gender. Needed to manipulate state for testing. Complejidad: **H**.
- [ ] **#CMD.18** `cs_lookup.rs` — read-only catalog queries (item/spell/quest/area/creature/object/skill/event/faction/player/title/map/tele/taxinode). 1.5 KLOC in C++. Each is a database/DB2 scan with localization; share a "search-and-paginate" helper. Complejidad: **XL** (split per kind).
- [ ] **#CMD.19** `cs_reload.rs` — every `.reload <table>` thunks to a `sObjectMgr->Load*()` call. In Rust we currently have no live-reload story for any table; first land per-table reload primitives in the relevant crates, *then* expose them as commands. Complejidad: **XL** (~120 sub-commands; do the 10 most-changed tables first: `creature_template`, `quest_template`, `gameobject_template`, `spell_proc`, `gossip_menu`, `disables`, `creature_text`, `mail_loot_template`, `points_of_interest`, `command`).
- [ ] **#CMD.20** `cs_account.rs` + `cs_ban.rs` + `cs_character.rs` — the "live ops" tier. Depends on accounts.md / characters DB / ban tables. Complejidad: **XL** (split per-file per-PR).
- [ ] **#CMD.21** Remaining content handlers — `cs_achievement`, `cs_arena`, `cs_ahbot`, `cs_bf`, `cs_cast`, `cs_cheat`, `cs_debug`, `cs_deserter`, `cs_disable`, `cs_event`, `cs_go`, `cs_group`, `cs_guild`, `cs_honor`, `cs_instance`, `cs_learn`, `cs_lfg`, `cs_list`, `cs_message`, `cs_mmaps`, `cs_pet`, `cs_quest`, `cs_rbac`, `cs_reset`, `cs_scene`, `cs_send`, `cs_server`, `cs_ticket`, `cs_titles`, `cs_wp`, `cs_battlenet_account`. Each blocks on its own subsystem being live. Complejidad: **XL** (do not do in one PR).
- [ ] **#CMD.22** `CliRunnable` analogue: `tokio::spawn` reading lines from `tokio::io::stdin` (or `rustyline` for tab-completion via `get_auto_completions_for`), instantiate a console-mode `ChatHandler` (no `WorldSession`, no `Player`), call dispatcher with `console = true`. Complejidad: **M**.
- [ ] **#CMD.23** RA TCP listener: bind a configured port (`Ra.IP`, `Ra.Port`), per-connection `Username:`/`Password:` prompt, `AccountMgr::check_password` + RBAC `RBAC_PERM_COMMANDS_USE_REMOTEACCESS`, then loop dispatching lines with output captured back to the socket. Complejidad: **H** (TLS optional but recommended).
- [ ] **#CMD.24** SOAP endpoint: HTTP listener (use `axum`, already in `bnet-server`), `POST /` with XML body `<executeCommand><command>...</command><username>...</username><password>...</password></executeCommand>`, dispatch, return the captured `SendSysMessage` buffer in `<result>`. Complejidad: **M** (a small subset of full gSOAP, since we control both endpoints).
- [ ] **#CMD.25** `world.command` table override-merge **invalidation** on `.reload commands` — wire it through.
- [ ] **#CMD.26** Decide GM-level vs RBAC migration: TC has both (`gmlevel` numeric on `account`, plus the modern RBAC permissions). Most handlers test RBAC; some legacy ones still call `AccountMgr::IsPlayerAccount`. Mirror the dual model exactly to keep DB row compatibility, or commit to RBAC-only (with a migration script). Complejidad: **M** (decision-only). Cross-ref `accounts.md` §13.

---

## 10. Regression tests to write

Each test should produce a Rust output byte-equal (ignoring locale-dependent timestamps) to the C++ output for the same input.

- [ ] Test: `ChatHandler::ParseCommands(text)` accepts `.tele`, `!tele` (synonyms); rejects `..tele`, `. tele`, `tele`, empty, single-char.
- [ ] Test: case-insensitivity — `.TELE`, `.Tele`, `.tEle` all dispatch.
- [ ] Test: prefix matching — `.te` resolves uniquely if `.tele` is the only `te*` top-level; `.te add` resolves to `tele add`. Ambiguous `.b` (with both `ban` and `bf` registered) prints the `LANG_CMD_AMBIGUOUS` list with both.
- [ ] Test: empty-name blank sub-command — `.tele` alone (with no further args) hits the `{ "", HandleTeleCommand, … }` invoker, not the help screen.
- [ ] Test: RBAC denial — caller without `RBAC_PERM_COMMAND_TELE_ADD` typing `.tele add foo` does **not** invoke the handler and does **not** leak the command's existence in tab completion (`is_invoker_visible == false`).
- [ ] Test: console-only — `.tele add` (which is `Console::No`) invoked from a `CliHandler` is rejected with help; `.tele del` (`Console::Yes`) succeeds.
- [ ] Test: `Hyperlink<ItemLinkData>` arg parses `|cffa335ee|Hitem:19019:...|h[Thunderfury…]|h|r` and rejects malformed bracket/closer. Item id matches the link payload.
- [ ] Test: `LinkValidator<ItemLinkData>::IsTextValid` rejects an item link whose `[text]` doesn't match any locale of the item's name.
- [ ] Test: `PlayerIdentifier` parses `Arthas`, `"Arthas"`, `|Hplayer:Arthas|h[Arthas]|h|r` to the same identity; resolves online → offline → fail.
- [ ] Test: `Optional<T>` — `.npc set level` with no level prints help; with a level invokes; no panic on EOF mid-parse.
- [ ] Test: `Tail` consumes the rest as a single string including spaces (`.announce Hello, world!` → `"Hello, world!"`).
- [ ] Test: typed-arg failure produces a localized `LANG_CMDPARSER_*` error (not a panic).
- [ ] Test: `.commands` lists exactly the subset visible to the caller's RBAC.
- [ ] Test: `LogCommandUsage` writes one line to the `gm` log target with all expected fields; not written when caller is a normal player and the perm is in the player role.
- [ ] Test: `world.command` help-text override replaces the default `LANG_*` text.
- [ ] Test: `trinity_string` locale resolution — same `LANG_COMMAND_TP_ADDED` returns different text for `LOCALE_enUS` vs `LOCALE_esES` (only if the rows exist; falls back to `LOCALE_enUS` if not).
- [ ] Test: tab-completion (`get_auto_completions_for("te")` returns `["tele", "tele add", "tele del", …]` in stable order — only the entries the caller's RBAC permits).
- [ ] Test: end-to-end — a `WorldSession` receiving `CMSG_CHAT_MESSAGE_SAY` with body `".tele 1"` does *not* see a `SMSG_MESSAGECHAT` broadcast to nearby players, but does receive an `SMSG_MESSAGECHAT(SYSTEM)` with the `LANG_*` confirmation message.

---

## 11. Notes / gotchas

- **The `.commands` command itself enumerates all commands the caller can execute, filtered by RBAC** — so getting RBAC wrong means GMs see commands they can't run, or worse, can't see commands they can run. Test `.commands` early and often.
- **RA / SOAP do not bypass RBAC** — they re-authenticate against the same account table. `RBAC_PERM_COMMANDS_USE_REMOTEACCESS` is the gatekeeper. There is also `RBAC_PERM_COMMANDS_PINFO_CHECK_PERSONAL_DATA` (the GDPR-flavoured opt-in for `.pinfo` to expose email/last-IP).
- **`Console::Yes/No` is the wrong abstraction post-2017** — many commands are *technically* Console-safe (no `WorldSession*` deref) but operationally GM-only because they need a target selection. Don't blindly mirror — cross-check the handler body.
- **Audit log is a compliance hard-requirement** for paid private servers and for any server that takes player reports. `LogCommandUsage` writes to the `gm` channel by default; rotate it out of the main log file. Skipping a command (e.g. for high-frequency `.gps`) is the wrong fix — filter on the appender, not the call.
- **Hyperlink validation is a security perimeter, not a UX nicety.** A player can craft `|cff00ff00|Hitem:1|h[+9999 Stamina Boots]|h|r` and post it to General; without `LinkValidator` the receiver's client renders it as a real link. Once a GM `.appear`s the spammer they may be "examining" a fake item that runs another exploit. TC has had at least three CVE-like reports here historically.
- **`PlayerIdentifier` is the silent footgun** — it does an offline-DB lookup on miss, which means typing `.appear nonexistentplayer` runs a synchronous `CHAR_SEL_*` query under the chat-handler lock. Audit all callsites for blocking I/O when porting.
- **Argument tokens are split on `' '` (single ASCII space) only** — tabs, multiple spaces, or non-breaking spaces don't tokenize. Mirror this even if it feels wrong.
- **Empty-name builders (`{ "", handler, ... }`) act as the "default" handler when only the parent name is typed and no sub-command matches.** A duplicate empty-name builder is a hard `ASSERT` in C++. Mirror this; don't make it a soft warning.
- **The `!`-prefix is meant for "addon channel" commands** (an addon sends a binary message containing the command). It's almost never typed by humans — they use `.`. But it must be accepted.
- **Argument for "`.gm fly` is not the same as `.cheat fly`":** `.gm fly` toggles a session flag; `.cheat fly` toggles a cheat-mode flag stored on `Player`. They affect different code paths. Don't merge.
- **`.reload commands` exists and is critical** — without it, GMs have to bounce the server to pick up new help-text rows. Wire #CMD.25 from day one.
- **Don't try to migrate `cs_debug.cpp` (1799 LOC, ~80 sub-commands).** Almost all of it is dev-only test-harness for things RustyCore hasn't ported yet (boundary visualization, conversation playback, packet send/recv test, vehicle attachment). Port maybe 5 (the "where am I" / `.debug los`-style) and stub the rest.
- **Console-mode `GetTrinityString` reads the *server's* DBC locale**, not a per-session locale (which doesn't exist on the console). Mirror this.
- **TC has both legacy (`char const*`) and new (typed-tuple) handler signatures, with a `[[deprecated]]` builder for the former.** Skip the legacy form entirely in Rust — there's no codebase to be bidirectionally compatible with.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class ChatHandler` | `struct ChatHandler<'s>` borrowing `&'s mut WorldSession` (in `wow-chat::handler`) | No inheritance. `CliHandler` and `AddonChannelCommandHandler` become enum variants over the session source. |
| `ChatHandler::SendSysMessage(string_view)` | `ChatHandler::send_sys_message(&mut self, msg: &str)` | Splits on `\n`, sends one packet per line. |
| `ChatHandler::PSendSysMessage(LANG_id, args...)` | `ChatHandler::psend_sys_message(&mut self, key: TStr, args: …)` via a `format!`-style helper or `tracing::info!`-style macro | Use `&'static str` interning for `LANG_*` keys; resolve via `trinity_string` at format time. |
| `Trinity::ChatCommands::ChatCommandTable` | `&'static [CommandBuilder]` collected via `inventory::collect!` | Static registration to match the existing `PacketHandlerEntry` pattern in `wow-handler`. |
| `ChatCommandBuilder` | `pub struct CommandBuilder { name: &'static str, invoker: CommandInvoker, perm: RbacPerm, console: bool, help: TStr }` | One stable form, no deprecated overloads. |
| `Trinity::Impl::ChatCommands::ChatCommandNode` | `struct CommandNode { name: String, invoker: Option<Invoker>, perm: RbacPerm, console: bool, help: HelpText, sub: BTreeMap<UniCase<String>, CommandNode> }` | `BTreeMap<UniCase, _>` for case-insensitive iteration with stable order. |
| `CommandInvoker` (type-erased) | `type Invoker = fn(&mut ChatHandler, &str) -> bool` plus a per-command **derive-macro**-generated wrapper that parses args | The macro hides the `ArgInfo` plumbing from handler authors. |
| `ArgInfo<T>::TryConsume` | `trait ConsumeArg { fn consume<'a>(input: &'a str, h: &ChatHandler) -> Result<(Self, &'a str), ParseError>; }` per type | Derive impls for `i32`, `u32`, `f32`, `bool`, `String`. |
| `Trinity::ChatCommands::Tail` / `WTail` / `QuotedString` | newtype wrappers `Tail(String)`, `QuotedString(String)` with `ConsumeArg` impls | — |
| `Trinity::ChatCommands::Hyperlink<T>` | `struct Hyperlink<T> { color: u32, data: T, text: String }` with `ConsumeArg` impl that calls `parse_single_hyperlink` then dispatches per-tag | — |
| `Trinity::ChatCommands::Optional<T>` | `Option<T>` with `ConsumeArg` impl that tries-and-rolls-back | — |
| `Trinity::ChatCommands::Variant<A, B, ...>` | An enum with a derive macro generating the alternative-tries | Or just `serde`-style untagged. |
| `Trinity::ChatCommands::ExactSequence<"on">` | `unit struct OnLiteral;` with `ConsumeArg` matching exactly `"on"` | Use a const-generic `ExactSeq<const S: &'static str>` if 2024 const generics allow. |
| `PlayerIdentifier` | `struct PlayerIdentifier { guid: ObjectGuid, name: String, is_online: bool }` resolving via `PlayerRegistry` then `CharacterCache` | Cross-ref `globals.md`'s `CharacterCache` migration. |
| `AccountIdentifier` | `struct AccountIdentifier { id: u32, name: String }` resolving via `AccountMgr` (cross-ref `accounts.md`) | — |
| `Trinity::Hyperlinks::HyperlinkInfo` | `struct HyperlinkInfo<'a> { tail: &'a str, color: u32, tag: &'a str, data: &'a str, text: &'a str }` | `&'a str` lifetime carries from the input. |
| `Trinity::Hyperlinks::ParseSingleHyperlink` | free fn `parse_single_hyperlink(s: &str) -> Option<HyperlinkInfo<'_>>` | — |
| `LinkValidator<T>` | `trait ValidateLink { fn validate_text(&self, ...) -> bool; fn validate_color(&self, ...) -> bool; }` per-link-data impls | Stores backed by `wow-data` DB2 readers. |
| `rbac::RBACPermissions` enum | `#[repr(u32)] enum RbacPerm { ... }` mirroring numeric values from `RBAC.h` byte-for-byte | Required for DB row compatibility (`rbac_account_permissions.permissionId`). |
| `class CommandScript` (`ScriptObject` derivative) | (no class) — `inventory::submit! { CommandRegistration { ... } }` calls in each `cs_*.rs` | Avoids the abstract-base / virtual-method dance entirely. |
| `ScriptMgr::GetChatCommands` | `inventory::iter::<CommandRegistration>` fold-construct into the global `CommandNode` tree | Lazy-init on first dispatch; invalidate on `.reload commands`. |
| `WorldDatabase.Query(WORLD_SEL_COMMANDS)` | A `SqlxStatement::CommandHelpOverrides` in `wow-database::statements::world` | Loaded at startup *after* the static tree is built, in `LoadCommandMap` second pass. |
| `sLog->OutCommand(account, fmt, ...)` | `tracing::info!(target: "gm", account, char_name, x, y, z, map, area, zone, target, "{}", cmd)` | Configure a separate appender for the `gm` target in `wow-logging`. |
| `CliRunnable::ThreadFunc` | `tokio::spawn(async move { let mut rl = rustyline::Editor::new()?; loop { match rl.readline(">> ") { Ok(line) => dispatcher.execute(&console_handler, &line).await, … } })` | `rustyline` for tab-completion via `get_auto_completions_for`. |
| `RASession` | `tokio::net::TcpListener` + per-connection `auth_then_loop` task | Optional TLS — recommend `rustls`. |
| `process_message(soap*)` (TCSoap) | `axum::Router::route("/", post(soap_execute))` parsing the XML body manually (or with `quick-xml`) | We control both ends; no need for full gSOAP. |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.

---

## 13. Audit (2026-05-01)

**Method:**

```bash
# 1. Confirm zero command-framework symbols anywhere in Rust.
grep -rn 'ChatCommandTable\|ChatCommandBuilder\|ChatCommandNode\|CommandInvoker\|TryExecuteCommand\|ParseCommands\|RBAC_PERM_COMMAND_\|RBACPermissions\|ChatHandler\|CliHandler\|CliRunnable\|RASession\|TCSoap\|process_message\|ParseSingleHyperlink\|LinkValidator\|HyperlinkInfo' crates/

# 2. Confirm zero "user types a dot-command" handling in the chat path.
grep -rn '\\.tele\\|\\.gm\\b\\|\\.additem\\|\\.npc\\|\\.gobject\\|\\.commands\\b\\|text\\.starts_with\\|text\\[0\\]\\b' crates/wow-world/src/handlers/chat.rs

# 3. Verify the script crates are empty placeholders.
wc -l crates/wow-script/src/lib.rs crates/wow-scripts/src/lib.rs crates/wow-chat/src/lib.rs

# 4. Confirm no stdin / RA / SOAP listener in either binary.
grep -rn 'tokio::io::stdin\\|rustyline\\|linenoise\\|RASession\\|RemoteAccess\\|TcpListener.*\\(8087\\|7878\\|3443\\)\\|axum.*soap\\|/executeCommand' crates/world-server/ crates/bnet-server/

# 5. Reread the chat handler in full to confirm no dispatch path.
read crates/wow-world/src/handlers/chat.rs
```

**Verdicts on flagged absences:**

1. **Command dispatcher (`Trinity::ChatCommands`) — CONFIRMED ABSENT.** Zero matches across `crates/` for `ChatCommandTable`, `ChatCommandBuilder`, `ChatCommandNode`, `CommandInvoker`, `TryExecuteCommand`. The single occurrence of "ChatCommand" in the workspace is inside `docs/migration/scripts.md` and `docs/migration/chat.md`, both of which are this very migration plan flagging the gap. `wow-chat/src/lib.rs` is 0 LOC. `wow-script/src/lib.rs` and `wow-scripts/src/lib.rs` are 0 LOC each.
2. **`.command` parse hook — CONFIRMED ABSENT.** `crates/wow-world/src/handlers/chat.rs` (lines 1-413) processes `CMSG_CHAT_MESSAGE_SAY/YELL/PARTY/GUILD/RAID/RAID_WARNING/INSTANCE_CHAT/WHISPER/EMOTE`. None of them inspects the first byte of the body for `.` or `!`. A GM typing `.tele` today gets it broadcast as ordinary `Say` chat to nearby players, with the leading dot included — a comedy gold demo. The handler does not import any "command", "ChatHandler", or "dispatcher" symbol.
3. **`RBAC_PERM_COMMAND_*` enum (~440 values) — CONFIRMED ABSENT.** Cross-referenced with `accounts.md` §13: the entire `RBACPermissions` enum is unrepresented in Rust, including all 440-odd `_COMMAND_*` values. `rbac_account_permissions` SQL prepared statements exist as string declarations but are never called. There is no `Player::has_permission(perm)` method; gameplay checks short-circuit to `true` or hard-code a numeric `gmlevel`.
4. **`Trinity::Hyperlinks` — CONFIRMED ABSENT.** No `parse_single_hyperlink`, `HyperlinkInfo`, `LinkValidator`, or `*LinkData` struct anywhere. **Security implication:** chat messages forwarded between players (already implemented in `handlers/chat.rs`) pass through completely unsanitized — a malicious player can spoof item links in any channel today. This is a P1 chat-protocol gap independent of the command system, but it's solved by the same parser, so it's flagged here for combined fix.
5. **`CliRunnable` / stdin reader — CONFIRMED ABSENT.** `crates/world-server/src/main.rs` and `crates/bnet-server/src/main.rs` neither spawn a stdin task nor import `tokio::io::stdin`. There is no console; the only way to administer a running RustyCore today is via direct DB writes + restart. The existing `worldserver.md` §8 and §11 already flag `CliThread` and `CliRunnable.cpp` (130 lines) as missing under task `#WS.8`; this commands.md cross-references that as `#CMD.22`.
6. **`RASession` (Remote Administration TCP) — CONFIRMED ABSENT.** No TCP listener on port 3443 or any RA-related symbol. `crates/world-server/src/main.rs` only handles game-client TCP on 8085/8086. This is a tracked gap under `#CMD.23`.
7. **SOAP endpoint — CONFIRMED ABSENT.** No `axum`-based or other HTTP listener for `/executeCommand` in `crates/world-server/`. The `bnet-server` crate has an `axum` REST handler for BNet auth but no SOAP route. Tracked as `#CMD.24`.
8. **`trinity_string` / `acore_string` localization table loader — CONFIRMED ABSENT.** No `LANG_*` constants enumerated, no `get_trinity_string(entry, locale)` resolution, no DB load of the `trinity_string` table. Cross-ref `texts.md` (which already documents the gap from the texts-table angle); this commands.md depends on its migration as `#CMD.9`.
9. **The 42 `cs_*.cpp` content scripts — CONFIRMED ABSENT.** No file in `crates/wow-scripts/`, `crates/wow-script/`, or anywhere else implements any GM command. There are 0 GM commands available; the count is exact, not approximate.

**Other findings during the audit:**

- The `inventory` crate (used by `wow-handler::PacketHandlerEntry` for static opcode-handler registration) is exactly the right primitive for command registration — a parallel `CommandRegistration` collection in each `cs_*.rs` would mirror the C++ `CommandScript` aggregator with no reflection or boilerplate. Recommended for `#CMD.8`.
- The `ChatMessage` packet decoder in `wow-packet::packets::chat` already exposes the message body as `String`. The dispatch hook (`#CMD.3`) is a 5-line patch to `handle_chat_say` (and the parallel handlers): peek `text.starts_with('.') || text.starts_with('!')` *before* broadcast, and return without broadcasting on hit.
- `parking_lot` and `dashmap` are already in workspace deps — use them for `COMMAND_MAP` (read-mostly: built once at startup, swapped on `.reload commands`).
- `unicase` is **not** in workspace deps. Add it for `UniCase<String>` in the sub-command map keys (case-insensitive Unicode-aware matching is required to mirror C++ `StringCompareLessI_T`).
- `MIGRATION_ROADMAP.md` already has line items at §5.8.3 ("Chat commands GM (.tele, .gm, .level, .item, .additem, .lookup...)") and §5.8.4 referencing this gap, plus the table at line 315 (`Chat/ChatCommands` → "(no existe) → ❌ → comandos GM"). Numbering is already consistent with this document's `#CMD.*` scheme.
- The `globals.md` migration doc defines a `GameTele` struct as a `.tele` GM command target type (confirmed at line 57 of that file). When `#CMD.12` lands, it will be the first consumer of that struct.
- **Critical operational impact:** without commands, the only way to test a feature in dev is to (a) write a unit test for it, or (b) edit a SQL row + restart the server + log in + perform the action manually. There is no `.spawn`, no `.summon`, no `.kick myself for testing reconnect`. Every QA workflow described in the C# legacy README ("`.tele <loc>` to verify map load", "`.npc add 12345` to verify combat ai", "`.additem 19019` to verify equipment slot") is impossible today. This is the **#1 productivity bottleneck** for further migration; recommend prioritizing #CMD.1-12 (dispatcher + `cs_tele` + `cs_gm` + `cs_misc` core) ahead of any further gameplay work.

**Status verdict:** ❌ not started (no change). All 9 flagged absences are real and total. Recommended migration order (compressed): #CMD.1 (decide arg-parser strategy) → #CMD.7 (RBAC enum, blocked on `accounts.md` #ACC.8) → #CMD.2 + #CMD.3 + #CMD.4 (dispatcher + `ChatHandler` + basic `ConsumeArg`) → #CMD.8 (registration via `inventory`) → #CMD.9 + #CMD.10 (string tables) → #CMD.12 (`cs_tele.rs` — first end-to-end demo) → #CMD.13 (`cs_gm.rs` — toggles GM mode) → #CMD.14 narrow (`cs_misc.rs` top 10) → #CMD.5 + #CMD.6 (hyperlinks: do *before* widening to handlers that take typed link args) → #CMD.11 (audit log) → #CMD.22 (stdin loop). Do **not** attempt #CMD.18 (`cs_lookup`) or #CMD.19 (`cs_reload`) until at minimum the first 8 above are stable; both are XL and will hide regressions in the dispatcher under their volume.
