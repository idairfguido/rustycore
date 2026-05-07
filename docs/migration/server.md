# Migration: game/Server

> **C++ canonical path:** `src/server/game/Server/`
> **Rust target crate(s):** `crates/wow-network/`, `crates/wow-world/`
> **Layer:** L1 — Network, Session lifecycle, Packet I/O
> **Status:** ⚠️ partial (WorldSocket handshake ✅, opcode dispatch ~23% coverage)
> **Audited vs C++:** ⚠️ audited 2026-05-01 — large coverage gap, processing-mode mismatches widespread
> **Last updated:** 2026-05-01

---

## 1. Purpose

The Server module in TrinityCore manages the per-client connection lifecycle: TCP handshake with encryption setup (HMAC-SHA256 challenge-response, AES-GCM encryption), user authentication, and packet dispatch to game handlers. `WorldSession` is the *per-character session state* (not the Rust async connection); `WorldSocket` is the encrypted TCP transport; `WorldSocketMgr` pools and manages active connections.

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `game/Server/Packet.cpp` | 49 | `prefix` |
| `game/Server/Packet.h` | 80 | `prefix` |
| `game/Server/Packets/AccountPackets.cpp` | 29 | `prefix` |
| `game/Server/Packets/AccountPackets.h` | 69 | `prefix` |
| `game/Server/Packets/AchievementPackets.cpp` | 251 | `prefix` |
| `game/Server/Packets/AchievementPackets.h` | 297 | `prefix` |
| `game/Server/Packets/AddonPackets.cpp` | 41 | `prefix` |
| `game/Server/Packets/AddonPackets.h` | 41 | `prefix` |
| `game/Server/Packets/AdventureJournalPackets.cpp` | 54 | `prefix` |
| `game/Server/Packets/AdventureJournalPackets.h` | 66 | `prefix` |
| `game/Server/Packets/AdventureMapPackets.cpp` | 29 | `prefix` |
| `game/Server/Packets/AdventureMapPackets.h` | 39 | `prefix` |
| `game/Server/Packets/AllPackets.h` | 92 | `prefix` |
| `game/Server/Packets/AreaTriggerPackets.cpp` | 101 | `prefix` |
| `game/Server/Packets/AreaTriggerPackets.h` | 91 | `prefix` |
| `game/Server/Packets/ArenaTeamPackets.cpp` | 82 | `prefix` |
| `game/Server/Packets/ArenaTeamPackets.h` | 127 | `prefix` |
| `game/Server/Packets/ArtifactPackets.cpp` | 71 | `prefix` |
| `game/Server/Packets/ArtifactPackets.h` | 105 | `prefix` |
| `game/Server/Packets/AuctionHousePackets.cpp` | 772 | `prefix` |
| `game/Server/Packets/AuctionHousePackets.h` | 642 | `prefix` |
| `game/Server/Packets/AuthenticationPackets.cpp` | 366 | `prefix` |
| `game/Server/Packets/AuthenticationPackets.h` | 317 | `prefix` |
| `game/Server/Packets/AzeritePackets.cpp` | 71 | `prefix` |
| `game/Server/Packets/AzeritePackets.h` | 110 | `prefix` |
| `game/Server/Packets/BankPackets.cpp` | 63 | `prefix` |
| `game/Server/Packets/BankPackets.h` | 112 | `prefix` |
| `game/Server/Packets/BattlePayPackets.cpp` | 25 | `prefix` |
| `game/Server/Packets/BattlePayPackets.h` | 47 | `prefix` |
| `game/Server/Packets/BattlePetPackets.cpp` | 210 | `prefix` |
| `game/Server/Packets/BattlePetPackets.h` | 270 | `prefix` |
| `game/Server/Packets/BattlegroundPackets.cpp` | 523 | `prefix` |
| `game/Server/Packets/BattlegroundPackets.h` | 717 | `prefix` |
| `game/Server/Packets/BattlenetPackets.cpp` | 90 | `prefix` |
| `game/Server/Packets/BattlenetPackets.h` | 110 | `prefix` |
| `game/Server/Packets/BlackMarketPackets.cpp` | 108 | `prefix` |
| `game/Server/Packets/BlackMarketPackets.h` | 126 | `prefix` |
| `game/Server/Packets/CalendarPackets.cpp` | 512 | `prefix` |
| `game/Server/Packets/CalendarPackets.h` | 590 | `prefix` |
| `game/Server/Packets/ChannelPackets.cpp` | 196 | `prefix` |
| `game/Server/Packets/ChannelPackets.h` | 198 | `prefix` |
| `game/Server/Packets/CharacterPackets.cpp` | 727 | `prefix` |
| `game/Server/Packets/CharacterPackets.h` | 835 | `prefix` |
| `game/Server/Packets/ChatPackets.cpp` | 358 | `prefix` |
| `game/Server/Packets/ChatPackets.h` | 396 | `prefix` |
| `game/Server/Packets/ClientConfigPackets.cpp` | 76 | `prefix` |
| `game/Server/Packets/ClientConfigPackets.h` | 102 | `prefix` |
| `game/Server/Packets/CollectionPackets.cpp` | 25 | `prefix` |
| `game/Server/Packets/CollectionPackets.h` | 49 | `prefix` |
| `game/Server/Packets/CombatLogPackets.cpp` | 488 | `prefix` |
| `game/Server/Packets/CombatLogPackets.h` | 364 | `prefix` |
| `game/Server/Packets/CombatLogPacketsCommon.cpp` | 195 | `prefix` |
| `game/Server/Packets/CombatLogPacketsCommon.h` | 160 | `prefix` |
| `game/Server/Packets/CombatPackets.cpp` | 166 | `prefix` |
| `game/Server/Packets/CombatPackets.h` | 240 | `prefix` |
| `game/Server/Packets/CraftingPacketsCommon.cpp` | 62 | `prefix` |
| `game/Server/Packets/CraftingPacketsCommon.h` | 60 | `prefix` |
| `game/Server/Packets/DuelPackets.cpp` | 79 | `prefix` |
| `game/Server/Packets/DuelPackets.h` | 127 | `prefix` |
| `game/Server/Packets/EquipmentSetPackets.cpp` | 132 | `prefix` |
| `game/Server/Packets/EquipmentSetPackets.h` | 111 | `prefix` |
| `game/Server/Packets/EventPackets.cpp` | 28 | `prefix` |
| `game/Server/Packets/EventPackets.h` | 49 | `prefix` |
| `game/Server/Packets/GameObjectPackets.cpp` | 103 | `prefix` |
| `game/Server/Packets/GameObjectPackets.h` | 169 | `prefix` |
| `game/Server/Packets/GarrisonPackets.cpp` | 484 | `prefix` |
| `game/Server/Packets/GarrisonPackets.h` | 443 | `prefix` |
| `game/Server/Packets/GuildPackets.cpp` | 1047 | `prefix` |
| `game/Server/Packets/GuildPackets.h` | 1307 | `prefix` |
| `game/Server/Packets/HotfixPackets.cpp` | 127 | `prefix` |
| `game/Server/Packets/HotfixPackets.h` | 102 | `prefix` |
| `game/Server/Packets/InspectPackets.cpp` | 247 | `prefix` |
| `game/Server/Packets/InspectPackets.h` | 198 | `prefix` |
| `game/Server/Packets/InstancePackets.cpp` | 168 | `prefix` |
| `game/Server/Packets/InstancePackets.h` | 251 | `prefix` |
| `game/Server/Packets/ItemPackets.cpp` | 363 | `prefix` |
| `game/Server/Packets/ItemPackets.h` | 538 | `prefix` |
| `game/Server/Packets/ItemPacketsCommon.cpp` | 267 | `prefix` |
| `game/Server/Packets/ItemPacketsCommon.h` | 139 | `prefix` |
| `game/Server/Packets/LFGPackets.cpp` | 497 | `prefix` |
| `game/Server/Packets/LFGPackets.h` | 559 | `prefix` |
| `game/Server/Packets/LFGPacketsCommon.cpp` | 42 | `prefix` |
| `game/Server/Packets/LFGPacketsCommon.h` | 49 | `prefix` |
| `game/Server/Packets/LootPackets.cpp` | 248 | `prefix` |
| `game/Server/Packets/LootPackets.h` | 322 | `prefix` |
| `game/Server/Packets/MailPackets.cpp` | 305 | `prefix` |
| `game/Server/Packets/MailPackets.h` | 244 | `prefix` |
| `game/Server/Packets/MiscPackets.cpp` | 832 | `prefix` |
| `game/Server/Packets/MiscPackets.h` | 1052 | `prefix` |
| `game/Server/Packets/MovementPackets.cpp` | 1097 | `prefix` |
| `game/Server/Packets/MovementPackets.h` | 728 | `prefix` |
| `game/Server/Packets/MythicPlusPacketsCommon.cpp` | 130 | `prefix` |
| `game/Server/Packets/MythicPlusPacketsCommon.h` | 107 | `prefix` |
| `game/Server/Packets/NPCPackets.cpp` | 291 | `prefix` |
| `game/Server/Packets/NPCPackets.h` | 327 | `prefix` |
| `game/Server/Packets/PacketUtilities.cpp` | 62 | `prefix` |
| `game/Server/Packets/PacketUtilities.h` | 322 | `prefix` |
| `game/Server/Packets/PartyPackets.cpp` | 790 | `prefix` |
| `game/Server/Packets/PartyPackets.h` | 774 | `prefix` |
| `game/Server/Packets/PerksProgramPacketsCommon.cpp` | 38 | `prefix` |
| `game/Server/Packets/PerksProgramPacketsCommon.h` | 42 | `prefix` |
| `game/Server/Packets/PetPackets.cpp` | 206 | `prefix` |
| `game/Server/Packets/PetPackets.h` | 276 | `prefix` |
| `game/Server/Packets/PetitionPackets.cpp` | 192 | `prefix` |
| `game/Server/Packets/PetitionPackets.h` | 246 | `prefix` |
| `game/Server/Packets/QueryPackets.cpp` | 534 | `prefix` |
| `game/Server/Packets/QueryPackets.h` | 452 | `prefix` |
| `game/Server/Packets/QuestPackets.cpp` | 823 | `prefix` |
| `game/Server/Packets/QuestPackets.h` | 776 | `prefix` |
| `game/Server/Packets/ReferAFriendPackets.cpp` | 30 | `prefix` |
| `game/Server/Packets/ReferAFriendPackets.h` | 40 | `prefix` |
| `game/Server/Packets/ReputationPackets.cpp` | 70 | `prefix` |
| `game/Server/Packets/ReputationPackets.h` | 89 | `prefix` |
| `game/Server/Packets/ScenarioPackets.cpp` | 136 | `prefix` |
| `game/Server/Packets/ScenarioPackets.h` | 125 | `prefix` |
| `game/Server/Packets/ScenePackets.cpp` | 56 | `prefix` |
| `game/Server/Packets/ScenePackets.h` | 87 | `prefix` |
| `game/Server/Packets/SocialPackets.cpp` | 148 | `prefix` |
| `game/Server/Packets/SocialPackets.h` | 170 | `prefix` |
| `game/Server/Packets/SpellPackets.cpp` | 1042 | `prefix` |
| `game/Server/Packets/SpellPackets.h` | 1090 | `prefix` |
| `game/Server/Packets/SystemPackets.cpp` | 278 | `prefix` |
| `game/Server/Packets/SystemPackets.h` | 244 | `prefix` |
| `game/Server/Packets/TalentPackets.cpp` | 179 | `prefix` |
| `game/Server/Packets/TalentPackets.h` | 202 | `prefix` |
| `game/Server/Packets/TaxiPackets.cpp` | 78 | `prefix` |
| `game/Server/Packets/TaxiPackets.h` | 130 | `prefix` |
| `game/Server/Packets/TicketPackets.cpp` | 392 | `prefix` |
| `game/Server/Packets/TicketPackets.h` | 315 | `prefix` |
| `game/Server/Packets/TokenPackets.cpp` | 55 | `prefix` |
| `game/Server/Packets/TokenPackets.h` | 84 | `prefix` |
| `game/Server/Packets/TotemPackets.cpp` | 46 | `prefix` |
| `game/Server/Packets/TotemPackets.h` | 69 | `prefix` |
| `game/Server/Packets/ToyPackets.cpp` | 57 | `prefix` |
| `game/Server/Packets/ToyPackets.h` | 71 | `prefix` |
| `game/Server/Packets/TradePackets.cpp` | 138 | `prefix` |
| `game/Server/Packets/TradePackets.h` | 207 | `prefix` |
| `game/Server/Packets/TraitPackets.cpp` | 68 | `prefix` |
| `game/Server/Packets/TraitPackets.h` | 106 | `prefix` |
| `game/Server/Packets/TraitPacketsCommon.cpp` | 137 | `prefix` |
| `game/Server/Packets/TraitPacketsCommon.h` | 67 | `prefix` |
| `game/Server/Packets/TransmogrificationPackets.cpp` | 53 | `prefix` |
| `game/Server/Packets/TransmogrificationPackets.h` | 69 | `prefix` |
| `game/Server/Packets/VehiclePackets.cpp` | 69 | `prefix` |
| `game/Server/Packets/VehiclePackets.h` | 149 | `prefix` |
| `game/Server/Packets/VoidStoragePackets.cpp` | 106 | `prefix` |
| `game/Server/Packets/VoidStoragePackets.h` | 138 | `prefix` |
| `game/Server/Packets/WardenPackets.cpp` | 29 | `prefix` |
| `game/Server/Packets/WardenPackets.h` | 39 | `prefix` |
| `game/Server/Packets/WhoPackets.cpp` | 135 | `prefix` |
| `game/Server/Packets/WhoPackets.h` | 121 | `prefix` |
| `game/Server/Packets/WorldStatePackets.cpp` | 52 | `prefix` |
| `game/Server/Packets/WorldStatePackets.h` | 64 | `prefix` |
| `game/Server/Protocol/Opcodes.cpp` | 2280 | `prefix` |
| `game/Server/Protocol/Opcodes.h` | 2274 | `prefix` |
| `game/Server/Protocol/PacketLog.cpp` | 153 | `prefix` |
| `game/Server/Protocol/PacketLog.h` | 64 | `prefix` |
| `game/Server/WorldPacket.h` | 98 | `prefix` |
| `game/Server/WorldSession.cpp` | 1596 | `prefix` |
| `game/Server/WorldSession.h` | 2125 | `prefix` |
| `game/Server/WorldSocket.cpp` | 1083 | `prefix` |
| `game/Server/WorldSocket.h` | 178 | `prefix` |
| `game/Server/WorldSocketMgr.cpp` | 157 | `prefix` |
| `game/Server/WorldSocketMgr.h` | 72 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

All paths relative to `/home/server/woltk-trinity-legacy/src/server/game/Server/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `WorldSocket.h` | 178 | Socket lifecycle, header parsing, encryption state |
| `WorldSocket.cpp` | 1079 | Handshake, CMSG_AUTH_SESSION validation, packet crypto |
| `WorldSession.h` | 2125 | Per-character session state, packet handlers registry, player/guild/group state |
| `WorldSession.cpp` | 1596 | Session initialization, handler dispatch, player loading/logout |
| `WorldSocketMgr.h` | 72 | Connection manager, session pool, broadcaster |
| `WorldSocketMgr.cpp` | 157 | Session lifecycle, broadcast, queue management |
| `Packet.h` | 80 | Packet binary header, serialization |
| `Packet.cpp` | 49 | Packet I/O helpers |
| `WorldPacket.h` | 98 | WorldPacket struct (opcode + data buffer) |
| `Protocol/Opcodes.h` | — | Opcode enums (CMSG_*, SMSG_*) |
| `Protocol/Opcodes.cpp` | — | Opcode name strings |
| `Protocol/PacketLog.h` | — | Debug packet logging |
| `Protocol/PacketLog.cpp` | — | Network traffic capture |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `WorldSocket` | class | Per-client TCP connection; owns encryption state, packet recv/send |
| `WorldSession` | class | Per-character session; owns player, inventory, quests, auras, spell casting |
| `WorldSessionMgr` | class | Pool of all active WorldSession instances |
| `WorldPacket` | struct | Wire format: opcode (uint16) + payload bytes |
| `PacketHeader` | struct (packed) | Size (4 bytes) + Tag[12] for header encryption |
| `IncomingPacketHeader` | struct (packed) | Extends PacketHeader; adds EncryptedOpcode (2 bytes) |
| `EncryptablePacket` | class | WorldPacket wrapper with _encrypt flag for send queue |
| `OpcodeHandler` | class (impl in Opcodes.cpp) | Dispatch entry: opcode → handler function ptr |
| `ConnectionType` | enum | NORMAL, INSTANCE, PATCH, etc. |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `WorldSocket::Start()` | Begin handshake (send WORLD OF WARCRAFT CONNECTION string) | `InitializeHandler()`, `ReadHandler()` |
| `WorldSocket::Update()` | Process pending send queue | `WritePacketToBuffer()`, encryption |
| `WorldSocket::SendPacket(WorldPacket const&)` | Queue packet for send | `WritePacketToBuffer()`, `CompressPacket()` |
| `WorldSocket::HandleAuthSession()` | Validate CMSG_AUTH_SESSION digest + create WorldSession | DB lookup, `WorldPacket` parsing |
| `WorldSession::WorldSession(...)` | Constructor; init state for new character session | Database queries |
| `WorldSession::SetPlayer(Player*)` | Link Player object to session | Player, ObjectAccessor |
| `WorldSession::SendPacket(WorldPacket const*)` | Send packet to client via socket | `WorldSocket::SendPacket()` |
| `WorldSession::Update(uint32 diff)` | Process inbound packet queue per tick | Handler dispatch table |
| `WorldSession::ProcessQueryCallback()` | Handle async DB query results (login, equipment, etc.) | CharacterDatabase, LoginDatabase |
| `WorldSession::HandleCharEnumOpcode()` | List characters for account | CharacterDatabase, PlayerNameStore |
| `WorldSession::HandlePlayerLoginOpcode()` | Load character into world | Player constructor, MapManager, ObjectAccessor |
| `WorldSession::HandlePlayerLogout()` | Save character, unload player | Player::SaveToDB() |
| `WorldSessionMgr::AddSession()` | Register new WorldSession in pool | broadcaster |
| `WorldSessionMgr::SendGlobalText()` | Broadcast text to all sessions | iterator over session pool |
| `WorldSessionMgr::Update(uint32 diff)` | Tick all active sessions | `WorldSession::Update()` |
| `WorldSessionMgr::KickAll()` | Disconnect all players for shutdown | iterator, `WorldSocket::Close()` |

---

## 5. Module dependencies

**Depends on:**
- `Player` — stores character data, inventory, spells, quest state; owned by WorldSession
- `WorldSocket` — receives/sends encrypted packets; owned by WorldSession
- `ObjectMgr` — player/creature/gameobject templates, character lookup
- `CharacterDatabase`, `LoginDatabase` — account info, character data, item cache
- `MapManager` — spawn player on map on login
- `ObjectAccessor` — find player by GUID in world
- `ItemTemplate` — item stats for inventory validation
- `SpellMgr` — spell info, cast times, cooldowns
- `Guild`, `Group` — social state queried/updated on session init
- `AuctionHouseMgr`, `MailMgr` — async queries during login
- `BattlegroundMgr`, `InstanceLockMgr` — validate instance access
- `Warden` — client security scanning
- `RBAC` (Role-Based Access Control) — permission checks

**Depended on by:**
- `World` — ticks all sessions in update loop; broadcasts messages
- `ScriptMgr` — event hooks on session events
- Packet handlers (hundreds of opcodes) — call back into WorldSession methods
- `RemoteCommandMgr`, `ChatMgr` — send admin messages to sessions

---

## 6. SQL / DB queries (if any)

WorldSession issues prepared statements heavily during initialization and gameplay.

| Statement / Source | Purpose | DB |
|---|---|---|
| `SEL_ACCOUNT_DATA` | Load UI config data (keybinds, layouts) | character |
| `SEL_TUTORIALS` | Load tutorial flags (quest helper tooltips) | character |
| `LOGIN_SEL_ACCOUNT_TOYS` | Load account-wide toy unlocks | login |
| `LOGIN_SEL_BATTLE_PETS` | Load BattlePets (summons) | login |
| `LOGIN_SEL_ACCOUNT_MOUNTS` | Load account-wide mount unlocks | login |
| `LOGIN_SEL_ACCOUNT_HEIRLOOMS` | Load heirloom unlocks + transmog | login |
| `LOGIN_SEL_BNET_ITEM_APPEARANCES` | Load transmog wardrobe | login |
| `UPD_ACCOUNT_ONLINE` | Mark account online on login | character |
| `REP_ACCOUNT_DATA` | Save UI config on logout | character |
| `REP_PLAYER_ACCOUNT_DATA` | Save per-character config (field 124-255) | character |
| `INS/UPD_TUTORIALS` | Save tutorial flags | character |

WorldSocket also validates during handshake:
- `SEL_ACCOUNT_INFO_BY_NAME` (LoginDatabase) — account ID, session key, security level, ban status
- `SEL_IP_BANNED` — reject if IP banned
- `SEL_ACCOUNT_BANNED` — reject if account banned

---

## 7. Wire-protocol packets (if any)

Server module handles ALL opcode dispatch. Listed by category:

**Authentication:**
| Opcode | Direction | Handled in |
|---|---|---|
| `CMSG_AUTH_SESSION` | client → server | `WorldSocket::HandleAuthSession()` |
| `CMSG_AUTH_CONTINUED_SESSION` | client → server | `WorldSocket::HandleAuthContinuedSession()` |
| `CMSG_PING` | client → server | `WorldSocket::HandlePing()` |
| `SMSG_AUTH_CHALLENGE` | server → client | `WorldSocket::HandleSendAuthSession()` |
| `SMSG_AUTH_RESPONSE` | server → client | `WorldSocket::HandleAuthSession()` callback |
| `SMSG_ENTER_ENCRYPTED_MODE` | server → client | signed with Ed25519 |

**Character Management:**
| Opcode | Direction | Handled in |
|---|---|---|
| `CMSG_ENUM_CHARACTERS` | client → server | `WorldSession::HandleCharEnumOpcode()` |
| `CMSG_CHAR_CREATE` | client → server | `WorldSession::HandleCharCreateOpcode()` |
| `CMSG_CHAR_DELETE` | client → server | `WorldSession::HandleCharDeleteOpcode()` |
| `CMSG_PLAYER_LOGIN` | client → server | `WorldSession::HandlePlayerLoginOpcode()` |
| `CMSG_LOGOUT_REQUEST` | client → server | `WorldSession::HandleLogoutRequest()` |

**Movement / Position:**
| Opcode | Direction | Handled in |
|---|---|---|
| `CMSG_MOVEMENT_*` (numerous) | client → server | `WorldSession` movement validators |
| `SMSG_MONSTER_MOVE` | server → client | creature/object motion |

**Spells / Combat:**
| Opcode | Direction | Handled in |
|---|---|---|
| `CMSG_CAST_SPELL` | client → server | spell validation, cooldown check |
| `CMSG_CANCEL_CAST` | client → server | cancel in-flight cast |
| `SMSG_SPELL_FAILURE`, `SMSG_CAST_FAILED` | server → client | error responses |

**Chat / Social:**
| Opcode | Direction | Handled in |
|---|---|---|
| `CMSG_MESSAGECHAT` | client → server | chat broadcast |
| `CMSG_FRIEND_*`, `CMSG_IGNORE_*` | client → server | social lists |

(Hundreds more; full list in `Protocol/Opcodes.h`.)

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-network` | `crate_dir` | 6 | 1716 | `exists_active` | crate exists |
| `crates/wow-world` | `crate_dir` | 17 | 12778 | `exists_active` | crate exists |
| `crates/wow-network/src/world_socket.rs` | `file` | 1 | 1023 | `exists_active` | file exists |
| `crates/wow-network/src/session_mgr.rs` | `file` | 1 | 188 | `exists_active` | file exists |
| `crates/wow-world/src/session.rs` | `file` | 1 | 3138 | `exists_active` | file exists |
| `crates/world-server/src/main.rs` | `file` | 1 | 818 | `exists_active` | file exists |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-network/src/world_socket.rs` — ~600 lines — handshake, encryption, packet I/O
- `crates/wow-network/src/session_mgr.rs` — session linking for instance transfer
- `crates/wow-world/src/session.rs` — 3138 lines — WorldSession equivalent (packet dispatch, spell casting, inventory, creature visibility, aura tracking, quest state)
- `crates/world-server/src/main.rs` — ~500 lines — listener, account lookup, session spawning

**What's implemented:**
- ✅ WorldSocket handshake (AUTH_CHALLENGE → AUTH_SESSION → EnterEncryptedMode)
- ✅ HMAC-SHA256 digest validation (challenge-response authentication)
- ✅ AES-128-GCM encryption/decryption of all packets post-auth
- ✅ Ed25519 signing of EnterEncryptedMode packet
- ✅ Packet header parsing (size, tag, opcode)
- ✅ Packet dispatch to handler registry (Rust dispatch_table)
- ✅ Basic character login/logout (Player object creation, MapManager integration)
- ✅ Inventory tracking (HashMap<slot, InventoryItem>)
- ✅ Spell casting state machine (SpellCastState with timer)
- ✅ Visible aura tracking (HashMap<slot, AuraApplication>)
- ✅ Creature/gameobject visibility (HashSet<visible_guids>)
- ✅ Time sync (TimeSyncRequest counter)
- ⚠️ Partial packet handlers (login, logout, movement, spell cast, query)

**What's missing vs C++:**
- ❌ Warden anti-cheat integration
- ❌ Guild/Group state in session (Guild, Group classes not ported yet)
- ❌ BattlePets, Achievements, Calendar handlers
- ❌ RBAC permission checks in handlers
- ❌ Transmogrification (outfit saving)
- ❌ Social list (friends, ignore, blocked)
- ❌ Petition submission
- ❌ Auction house bidding/selling from session
- ❌ Mail system state
- ❌ Stable (pet) management
- ❌ Bank slot expansion
- ❌ Comprehensive error handling for all 400+ opcodes

**Suspicious / likely divergent (pre-audit hypothesis):**
- **Creature visibility divergence**: C++ WorldSession owns a `CreatureVisibilitySet` (per cell, per range); Rust `visible_creatures: HashSet<ObjectGuid>` is a flat set. No grid-based visibility culling in Rust yet. Risk: **clients see wrong creatures, performance cliff on dense maps**.
- **Aura slot collision**: C++ uses `std::array<AuraApplication, 255>` with slot management; Rust uses `HashMap<u8, AuraApplication>`. Risk: **aura slot number mismatches on client, visual bugs**.
- **Inventory desync**: C++ Item is a full class with links to ItemTemplate; Rust InventoryItem has only basic fields. Risk: **item stats sent to client differ from server state**.
- **Spell cooldown tracking**: C++ WorldSession has `SpellCooldownMap` (spell_id → cooldown_time); Rust has `last_spell_cast_time_per_spell` (spell_id → Instant). Missing cooldown_expiry for shared cooldowns. Risk: **player can cast too frequently**.
- **Movement validator**: C++ has extensive `MovementInfo` validation (gravity, speed hacks, teleport cheat detection); Rust has minimal validation. Risk: **position exploits**.

**Tests existing:**
- 0 direct tests in wow-world or wow-network (integration tests only in CI)
- No unit tests for crypto, packet parsing, or session state transitions

---

## 9. Migration sub-tasks

- [ ] **#SERVER.1** Implement `WorldSessionMgr` equivalent: `SessionRegistry` struct holding all active sessions with broadcasting + session lookup by account_id (complejidad: M)
- [ ] **#SERVER.2** Port all 400+ opcode handlers from C++ packet classes; prioritize common ones (login, movement, chat, spells, items) over rare ones (complejidad: XL)
- [ ] **#SERVER.3** Implement grid-based creature/gameobject visibility culling: replace flat HashSet with grid-based proximity tracking using existing MapManager grid (complejidad: H)
- [ ] **#SERVER.4** Add RBAC permission checks to session state; load permissions on login, validate in handler dispatch (complejidad: M)
- [ ] **#SERVER.5** Implement shared cooldown group tracking for spells (cooldown_id → expiry_time) (complejidad: L)
- [ ] **#SERVER.6** Port Warden anti-cheat module: link with `wow_warden` crate or write minimal client-challenge subset (complejidad: H)
- [ ] **#SERVER.7** Fix aura slot management: use `std::array<Option<AuraApplication>, 255>` or range-tree to prevent collisions (complejidad: M)
- [ ] **#SERVER.8** Implement Guild/Group state in session; link with group_registry and guild lookup (complejidad: M)
- [ ] **#SERVER.9** Implement BattlePets, Achievements, Calendar, Heirlooms handlers; query datastores on login (complejidad: H)
- [ ] **#SERVER.10** Comprehensive movement validator: speed checks, gravity, teleport sanity (complejidad: H)

---

## 10. Regression tests to write

- [ ] Test: `WorldSocket` handshake with valid HMAC → session created with correct account_id
- [ ] Test: `WorldSocket` handshake with invalid HMAC → AUTH_RESPONSE_FAILED sent
- [ ] Test: Packet encryption/decryption round-trip (plaintext → AES-GCM → decrypt → plaintext)
- [ ] Test: Character enum returns exact set of characters owned by account
- [ ] Test: Player login creates Player object on correct map with correct position
- [ ] Test: Spell cast state machine: init cast → tick to completion → SpellGo sent
- [ ] Test: Aura application fills slot 0-254 without collision
- [ ] Test: Creature visibility: login spawns nearby creatures, leave range despawns them
- [ ] Test: Movement validator rejects position delta > speed * dt
- [ ] Test: Inventory slot validation: equipped items have correct slot, bag items in valid range
- [ ] Test: Guild/Group sync on login: session reflects current guild/party state

---

## 11. Notes / gotchas

1. **WoLK 3.4.3 auth is challenge-response**, not token-based. The client must compute HMAC-SHA256(session_key, AUTH_CHECK_SEED + challenge_data) and send back the digest. If the digest doesn't match server calculation, connection is rejected. See `WorldSocket::HandleAuthSession()` in C++ for seed constants.

2. **EnterEncryptedMode is signed with Ed25519**. After validation, server sends this packet signed; client verifies signature before enabling AES-GCM. Missing or wrong signature → connection drops before encryption.

3. **Packet header compression**: packets > 256 bytes are optionally zlib-compressed if `CONFIG_COMPRESSION` is enabled. The header size includes uncompressed size. Rust implementation must check `CompressionEnabled()` flag.

4. **Session affinity**: a player can have only one active session at a time. Logging in again auto-closes the old session. C++ checks this in `WorldSocketMgr::AddSession()`.

5. **Grid visibility is critical for performance**. WoW's 533×533 map cells are checked against player's range (usually 100 yards). Flat HashSet iteration on login is O(n) and will lag servers with thousands of creatures. Must implement grid-based culling ASAP.

6. **Creature AI is per-creature**, not per-session. The C++ `WorldSession` doesn't own AI; AI ticks on MapManager update. Rust `creatures: HashMap<GUID, CreatureAI>` is wrong — it duplicates AI per session viewing. This will cause memory explosion. Must refactor to share CreatureAI from MapManager via Arc.

7. **Aura slot conflicts** in C++ are prevented by the `Aura::ModStackAmount()` logic; two auras of same spell_id go to same slot. Rust HashMap doesn't enforce this. Will cause client visual bugs (aura buffs disappearing) if not fixed.

8. **Logout timer**: on CMSG_LOGOUT_REQUEST, session enters a 20-second countdown. If player moves or casts during countdown, abort logout. C++ has explicit `m_logoutTimer`; Rust has `logout_time: Option<Instant>`.

9. **ASSERT on null Player**: many C++ handlers start with `Player* player = GetPlayer(); ASSERT(player);` — they crash the server if called without a loaded player. Rust must handle this gracefully (return error packet, don't panic).

10. **Packet header tag decryption**: the tag[12] field is encrypted with the first 12 bytes of AES keystream, not part of the opcode. Must be decrypted *before* trying to parse opcode. Wrong order → opcode garbage.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class WorldSocket` | `struct WorldSocket` in world_socket.rs | async tokio TcpStream; no inheritance |
| `WorldSocket::SendPacket()` | `world_socket.send_tx.send(bytes)` | channel-based async send |
| `class WorldSession` | `struct WorldSession` in session.rs | per-player state; no inheritance |
| `WorldSession::Update()` | `session.update(&mut self, diff_ms)` | poll packet queue, dispatch handlers |
| `WorldSession::SendPacket()` | `session.send_tx.send(packet)` | channel back to socket layer |
| `std::unordered_map<uint32, WorldSession*>` | `HashMap<u32, Arc<Mutex<WorldSession>>>` | shared mutable access |
| `WorldSessionMgr` | planned `SessionRegistry` struct | broadcast, lookup, lifecycle |
| `void (WorldSession::*)(WorldPacket*)` | `fn(&mut WorldSession, packet: &WorldPacket)` | handler function ptr → closure |
| `Player* m_player` | `Option<Arc<Player>>` | optional loaded character |
| `Spell* m_currentSpell[CURRENT_MAX_SPELL]` | `Option<SpellCastState>` | single active cast tracked |
| `std::set<ObjectGuid> m_creatures_visible` | `HashSet<ObjectGuid>` → *replace with grid-based* | divergence: must refactor |
| `Aura* m_auras[TOTAL_AURAS]` | `HashMap<u8, AuraApplication>` → *replace with array* | slot collision risk |
| `Item* m_items[EQUIPMENT_SLOT_END]` | `HashMap<u8, InventoryItem>` | okay for now; validate on send |

---

*Template version: 1.0. Status: ⚠️ partial — handshake & basic dispatch done; visibility & slot management divergent.*

---

## 13. Audit (2026-05-01)

### Scope

Cross-checked the Rust opcode dispatch layer (`crates/wow-handler/src/lib.rs` + `inventory::submit!`
sites in `crates/wow-world/src/handlers/*.rs` + match block in `crates/wow-world/src/session.rs:1411-1920`)
against the C++ TrinityCore wotlk_classic backport `Opcodes.cpp` (882 `DEFINE_HANDLER` entries; 621 with
real handler bodies, the remainder being newer-expansion stubs that resolve to `Handle_NULL` /
`STATUS_UNHANDLED` and are not relevant to a 3.4.3 server).

### Coverage stats

| Metric | Count |
|---|---|
| C++ `CMSG_*` opcodes defined in `Opcodes.cpp` | 882 |
| C++ active handlers (excluding `Handle_NULL` + `Handle_EarlyProccess`) | 621 |
| Rust `ClientOpcodes` enum constants in `wow-constants/src/opcodes.rs` | 663 |
| Rust `inventory::submit!` registrations (incl. `register_move!` macro expansions) | 154 |
| Match arms in `session.rs` dispatch block | 143 |
| **Overlap** (active C++ ∩ Rust registered) | **145** |
| Rust-registered with no corresponding C++ active handler (false positives) | 0 |
| **C++-active without Rust registration (gap)** | **476** |
| Rust-only opcode constants (`BuyStableSlot`, `Max`) without C++ handler | 2 |

Overall coverage of the C++ active-handler surface in Rust: **~23%** (145/621). The remaining 476
opcodes have no Rust `PacketHandlerEntry` and will fall through `dispatch_table.get()` returning
`None` in `session.rs:1382`, producing only an `info!` log line and silently dropping the packet.

### Status / processing mismatches

**Six SessionStatus mismatches** (Rust enum vs `STATUS_*` in C++):

| Opcode | C++ status | Rust status | Site |
|---|---|---|---|
| `CMSG_LOADING_SCREEN_NOTIFY` | `STATUS_AUTHED` | `LoggedIn` | `handlers/misc.rs` |
| `CMSG_QUERY_CREATURE` | `STATUS_LOGGEDIN` | `Authed` | `handlers/character.rs` |
| `CMSG_QUERY_GAME_OBJECT` | `STATUS_LOGGEDIN` | `Authed` | `handlers/character.rs` |
| `CMSG_QUERY_NPC_TEXT` | `STATUS_LOGGEDIN` | `Authed` | `handlers/character.rs` |
| `CMSG_QUERY_PLAYER_NAMES` | `STATUS_LOGGEDIN` | `Authed` | `handlers/character.rs` |
| `CMSG_QUERY_REALM_NAME` | `STATUS_LOGGEDIN` | `Authed` | `handlers/character.rs` |

The `CMSG_QUERY_*` cluster is registered as `Authed` but C++ requires `LoggedIn`. This means a
valid pre-login client could trigger DB lookups via these handlers — a small abuse surface but
behaviourally divergent.

**54 PacketProcessing mismatches** (after granting RustyCore's binary `Inplace`/`ThreadUnsafe`
split the leeway that C++'s `PROCESS_THREADSAFE` is acceptable as either). Pattern:

- Most mismatches are `cpp=PROCESS_THREADUNSAFE rust=Inplace` — Rust runs handlers in the socket
  thread that C++ explicitly demanded run on the per-session worker. Several of these (e.g.
  `CMSG_AUCTION_LIST_*`, `CMSG_CALENDAR_*`, `CMSG_CHAT_JOIN_CHANNEL`, `CMSG_HOTFIX_REQUEST`,
  `CMSG_GUILD_BANK_REMAINING_WITHDRAW_MONEY_QUERY`, `CMSG_LOGOUT_REQUEST`,
  `CMSG_LOGOUT_CANCEL`) issue DB reads/writes; running them inplace risks blocking the I/O thread.
- A second cluster is `cpp=PROCESS_INPLACE rust=ThreadUnsafe` (`CMSG_AREA_TRIGGER`,
  `CMSG_AUTO_EQUIP_ITEM`, `CMSG_AUTO_STORE_BAG_ITEM`, `CMSG_BUY_ITEM`, `CMSG_DESTROY_ITEM`,
  `CMSG_LIST_INVENTORY`, the `CMSG_QUEST_GIVER_*` group, `CMSG_QUERY_QUEST_INFO`). These are
  actually fine functionally — running them on the worker is stricter than C++ — but they will
  show up as latency outliers vs the reference.

### Two-step dispatch invariant

The dispatcher requires both (a) an `inventory::submit!` entry and (b) a match arm to actually
execute a handler. Without (a), `dispatch_table.get(&opcode)` returns `None` and the function
returns at `session.rs:1390` before the match executes. Without (b), the wildcard `_ => {}`
branch (`session.rs:1902-1919`) just emits a `trace!` line and does nothing.

| Class | Count | Severity |
|---|---|---|
| Match arm without `inventory::submit!` (hard silent drop) | 0 | — (clean) |
| `inventory::submit!` without match arm (log-only no-op) | **11** | **High** |

The 11 log-only no-ops are functionally dead handlers that look registered but never run:

- `Emote`, `SendTextEmote` — `handlers/chat.rs`
- `QueryQuestInfo`, `QuestGiverAcceptQuest`, `QuestGiverChooseReward`,
  `QuestGiverCompleteQuest`, `QuestGiverQueryQuest`, `QuestGiverRequestReward`,
  `QuestLogRemoveQuest` — `handlers/quest.rs`
- `TrainerBuySpell` — `handlers/trainer.rs`
- `WorldPortResponse` — `handlers/misc.rs`

These are silent gameplay regressions: the handler bodies exist (so tests against the function
directly may pass) but the dispatcher never reaches them.

### Other findings

- **One duplicate registration**: `ClientOpcodes::TrainerList` is registered in both
  `handlers/character.rs` and `handlers/trainer.rs`. `inventory` does not de-duplicate; the table
  builder will keep whichever `inventory::iter` yields last, producing a non-deterministic winner
  across builds.
- **Unknown-opcode handling parity**: C++ `OpcodeTable::operator[]` returns null and the dispatcher
  drops with `LogUnprocessedTail`; Rust returns at `session.rs:1373` after an `info!` log. Behaviour
  matches (log + drop, no error response sent to client). No regression vs C++.
- **Arity sample (10 opcodes, fields read in order)**:
  - `CMSG_CHAR_DELETE`: C++ reads `Guid` ↔ Rust reads packed GUID — **match**.
  - `CMSG_LOOT_UNIT`: C++ reads `Unit` (ObjectGuid) ↔ Rust reads packed GUID — **match**.
  - `CMSG_TAXI_NODE_STATUS_QUERY`: C++ reads `UnitGUID` ↔ Rust reads packed GUID — **match**.
  - `CMSG_CHAT_MESSAGE_RAID`: C++ reads `Language(int32) + Bits(11) + IsSecure(bit) + text`;
    Rust reads same fields in same order — **match**.
  - `CMSG_AREA_TRIGGER`: C++ reads `int32 AreaTriggerID + bit Entered + bit FromClient`;
    Rust reads `u32 trigger_id` and stops — **MISMATCH**, drops 2 trailing bits. Server cannot
    distinguish entry/exit triggers.
  - `CMSG_MOVE_SET_FLY`, `CMSG_AUCTION_LIST_BIDDER_ITEMS`, `CMSG_LOADING_SCREEN_NOTIFY`,
    `CMSG_COMMERCE_TOKEN_GET_LOG`, `CMSG_QUEST_GIVER_QUERY_QUEST` — not deeply inspected, but
    dispatch flow exists.

The 1-in-10 arity defect rate, extrapolated, suggests a non-trivial number of similar truncated
reads in the registered handlers — most likely in opcodes that gained bit-packed fields between
classic-WotLK and the modern protocol the C++ backport actually uses.

### Recommended sub-tasks

- [ ] **#SERVER.AUDIT.1** Migrate the 11 log-only no-ops out of the `_ => {}` fall-through:
      add explicit `ClientOpcodes::Foo => self.handle_foo(pkt).await,` arms in
      `session.rs:1411-1920` for each registered-but-unmatched opcode (Emote, SendTextEmote,
      WorldPortResponse, the 7 QuestGiver/Quest opcodes, TrainerBuySpell). Complejidad: S.
- [ ] **#SERVER.AUDIT.2** Resolve the duplicate `TrainerList` registration: keep the
      `handlers/trainer.rs` entry, remove the `handlers/character.rs` one, and add a
      `cargo test`-time invariant that scans `inventory::iter` for duplicate opcodes.
      Complejidad: S.
- [ ] **#SERVER.AUDIT.3** Fix the 6 `SessionStatus` mismatches (`CMSG_QUERY_*` cluster + the
      reverse on `CMSG_LOADING_SCREEN_NOTIFY`). Complejidad: S.
- [ ] **#SERVER.AUDIT.4** Reconcile the 54 `PacketProcessing` mismatches. The right move is
      probably to introduce a third `Inplace`/`ThreadSafe`/`ThreadUnsafe` variant and migrate the
      handlers DB-touching while running inplace to `ThreadUnsafe`. Complejidad: M.
- [ ] **#SERVER.AUDIT.5** Triage the 476 unregistered C++-active opcodes by frequency: the
      hot path (login, movement, item, spell, inventory, chat) appears mostly covered; the
      gap is dominated by guild bank, calendar, LFG, auction-house variants, void storage,
      petitions, garrison-era opcodes that may legitimately be deferred. Catalog into
      "must-port" / "P2 backlog" / "skip (modern-only)" lists. Complejidad: M.
- [ ] **#SERVER.AUDIT.6** Add the missing `Entered` + `FromClient` bit reads to
      `CMSG_AREA_TRIGGER` decoder; sweep the rest of `handlers/misc.rs` and `handlers/character.rs`
      for similar truncated reads against C++ `*Packets.cpp::Read()` bodies. Complejidad: M.
- [ ] **#SERVER.AUDIT.7** Add a build-time test that walks `inventory::iter::<PacketHandlerEntry>`
      and asserts each opcode either has a match arm in `session.rs` or is explicitly listed in
      a `#[allow(dead_handler)]` set, eliminating the silent-no-op class of bugs. Complejidad: M.

### Audit confidence

Coverage stats and two-step-dispatch findings: **high confidence** (mechanical comparison of
parsed registrations against parsed match arms). Status/processing mismatch counts: **high
confidence** (string compare with documented C++ → Rust mapping; the
`PROCESS_THREADSAFE`/`Inplace` ambiguity is explicitly accounted for). Arity findings:
**moderate confidence** (10-sample manual spot check, not exhaustive). The 476 missing-handler
gap is well-supported but the practical importance of each missing opcode varies wildly and
would need its own pass.

