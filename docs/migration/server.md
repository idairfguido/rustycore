# Migration: game/Server

> **C++ canonical path:** `src/server/game/Server/`
> **Rust target crate(s):** `crates/wow-network/`, `crates/wow-world/`
> **Layer:** L1 — Network, Session lifecycle, Packet I/O
> **Status:** ⚠️ partial (WorldSocket handshake ✅, WorldSession dispatch partial)
> **Audited vs C++:** ⚠️ partial (handshake complete, session state diverging)
> **Last updated:** 2026-05-01

---

## 1. Purpose

The Server module in TrinityCore manages the per-client connection lifecycle: TCP handshake with encryption setup (HMAC-SHA256 challenge-response, AES-GCM encryption), user authentication, and packet dispatch to game handlers. `WorldSession` is the *per-character session state* (not the Rust async connection); `WorldSocket` is the encrypted TCP transport; `WorldSocketMgr` pools and manages active connections.

---

## 2. C++ canonical files

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
