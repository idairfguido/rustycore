# Migration: game/World

> **C++ canonical path:** `src/server/game/World/`
> **Rust target crate(s):** `crates/world-server/`, `crates/wow-network/`
> **Layer:** L0 — Global state, update loop, server lifecycle
> **Status:** ❌ not started (core event loop skeleton exists; config/timers not ported)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

The World module is TrinityCore's **global game state manager**. It owns and orchestrates:
- Server configuration (bool/int/float config values, rates, limits)
- Global timers and periodic maintenance tasks (uptime, auctions, corpse cleanup, character deletion, database pings)
- World state variables (MOTD, shutdown state, player count, server status messages)
- Session broadcasting (send message to all players, shutdown notifications)
- MapManager update loop coordination
- Integration point for all managers (AuctionHouseMgr, BattlegroundMgr, GuildMgr, etc.)

The World tick is the heartbeat: it calls Update() on all subsystems every 10–50ms, ensuring consistent ordering and coordination.

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/src/server/game/World/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `World.h` | 934 | Global singleton; config enums, timer list, manager ptrs, broadcast methods |
| `World.cpp` | 3971 | Initialization, LoadConfigSettings, Update loop, SQL queries |
| `WorldStates/WorldStateMgr.h` | ~150 | PersistentWorldVariable storage (quest state, event flags) |
| `WorldStates/WorldStateMgr.cpp` | ~300 | Load/save world state from DB |
| `WorldStates/WorldStateDefines.h` | ~100 | WorldState variable IDs (WS_ALLIANCE_CONTROLLED_etc) |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `World` | singleton class | Global game state; config, timers, broadcaster, shutdown orchestration |
| `ServerMessageType` | enum | Shutdown/restart/broadcast message types (SERVER_MSG_SHUTDOWN_TIME, etc.) |
| `ShutdownMask` | enum | Restart vs Idle vs Force shutdown flags |
| `WorldTimers` | enum | Timer IDs (WUPDATE_AUCTIONS, WUPDATE_CORPSES, etc.) — indices into m_timers array |
| `WorldBoolConfigs` | enum | Config bool keys (CONFIG_DURABILITY_LOSS_IN_PVP, CONFIG_ADDON_CHANNEL, etc.) |
| `WorldIntConfigs` | enum | Config int keys (CONFIG_COMPRESSION, CONFIG_INTERVAL_SAVE, etc.) |
| `WorldFloatConfigs` | enum | Config float keys (CONFIG_SIGHT_MONSTER, CONFIG_LISTEN_RANGE_*, etc.) |
| `PersistentWorldVariable` | struct | (from WorldStateMgr) int32 variable stored in DB |
| `WorldStateManager` | class | Load/persist PersistentWorldVariable instances |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `World::World()` | Constructor; setup empty config, timers, rate values | (none, static init only) |
| `World::SetInitialWorldSettings()` | Load all config, DB data, spawn managers | LoadConfigSettings, sObjectMgr, sMapMgr |
| `World::LoadConfigSettings(bool reload)` | Parse config file, populate m_*_configs arrays | sConfigMgr, SetIntConfig, SetFloatConfig |
| `World::Update(uint32 diff)` | Main server tick; advance timers, dispatch maintenance | UpdateSessions, sMapMgr->Update, all manager->Update |
| `World::UpdateSessions(uint32 diff)` | Tick all WorldSession instances | WorldSessionMgr::Update |
| `World::SendWorldText(uint32, ...)` | Send localized text to all players | SendGlobalText |
| `World::SendGlobalText(const char*, WorldSession*)` | Broadcast raw text (optional self exclude) | WorldSession::SendPacket for each |
| `World::SendServerMessage(ServerMessageType, string)` | Send system message (shutdown, restart, etc.) | WorldPackets::Misc::ServerMessage packet |
| `World::SendGlobalMessage(WorldPacket const*, WorldSession*, Team)` | Broadcast WorldPacket to all (optional team filter) | iterator over sessions |
| `World::ShutdownServ(uint32 time, uint32 options, uint8 exitcode)` | Schedule server shutdown in N seconds with reason | m_stopEvent, m_ExitCode |
| `World::ShutdownMsg(bool, Player*)` | Announce shutdown to players or specific player | SendGlobalText |
| `World::KickAll()` | Disconnect all players and destroy sessions | WorldSessionMgr::KickAll |
| `World::setIntConfig(WorldIntConfigs, uint32)` | Update int config value (may trigger manager rescaling) | m_int_configs[], manager callbacks |
| `World::setBoolConfig(WorldBoolConfigs, bool)` | Update bool config value | m_bool_configs[] |
| `World::SetPersistentWorldVariable()` | Persist int32 world state to DB | WorldStateManager |
| `World::LoadPersistentWorldVariables()` | Load world state from DB on startup | WorldStateManager |

---

## 5. Module dependencies

**Depends on:**
- `MapManager` — Update all loaded maps and their grids per tick
- `ObjectMgr` — creature/gameobject/spell/item templates; faction, quest data
- `WorldSessionMgr` — tick all sessions, broadcast messages
- `Config` (sConfigMgr) — read world config from file
- `DatabaseEnv` — LoginDatabase, CharacterDatabase, WorldDatabase for queries
- `ScriptMgr` — hook events (startup, shutdown, config reload)
- `BattlegroundMgr` — update arenas, queue announcements
- `AuctionHouseMgr` — periodic auction expiry checks
- `GuildMgr` — auto-save guilds, reset perks
- `GroupMgr` — auto-save groups
- `AuctionHouseBot` — restock vendor listings
- `GameEventMgr` — check for event transitions
- `PoolMgr` — respawn pool creatures/objects
- `OutdoorPvPMgr` — update Wintergrasp, Tol Barad capture state
- `BattlefieldMgr` — outdoor PvP manager
- `LFGMgr` (Looking For Group) — queue updates
- `CharacterCache`, `CreatureTextMgr`, `TaxiPathGraph`, etc. — various lookups
- `SmartScriptMgr`, `CreatureAIRegistry` — AI behavior loading
- `WardenCheckMgr` — anti-cheat scans
- `Realm` (local) — realm info (name, type, zone, language)
- `GameTime` — time checks (events, auto-save intervals)

**Depended on by:**
- `main()` — World singleton is instantiated and ticked every server frame
- `ScriptMgr` — listens to World events
- All game systems via singleton accessor `sWorld`
- CLI commands — query player count, shutdown, broadcast messages

---

## 6. SQL / DB queries (if any)

World module issues numerous queries during init and periodic maintenance.

| Statement / Source | Purpose | DB |
|---|---|---|
| `LOGIN_SEL_REALMLIST_SECURITY_LEVEL` | Load realm security setting | login |
| `UPDATE realmlist SET icon=?, timezone=?` | Update realm type/timezone in DB | login |
| `INSERT INTO uptime VALUES(realm_id, start_time, 0, revision)` | Log server startup | login |
| Auction cleanup queries | Remove expired auctions | world |
| Guild bank log cleanup | Prune old guild transaction logs | character |
| Corpse cleanup | Delete abandoned corpses after N hours | character |
| Character deletion cleanup | Remove chars pending deletion | character |
| Database ping | Keep connection alive | any (SELECT 1) |

Timers that trigger DB maintenance:
- `WUPDATE_AUCTIONS` (60s) — expire old auctions
- `WUPDATE_UPTIME` (10min) — ping databases, log uptime
- `WUPDATE_CORPSES` (20min) — despawn old corpses
- `WUPDATE_CLEANDB` (hourly) — clean orphaned character data
- `WUPDATE_DELETECHARS` (daily) — finalize character deletions
- `WUPDATE_PINGDB` (30min) — keep DB connection warm

---

## 7. Wire-protocol packets (if any)

World module doesn't directly send packets; instead, it calls methods on sessions or managers that send packets. Notable broadcast scenarios:

| Scenario | Packet(s) | Sent via |
|---|---|---|
| Server message (shutdown countdown, MOTD, content ready) | `ServerMessagePacket` | `World::SendServerMessage()` → per-session broadcast |
| Global text chat | `ChatMessage` | `World::SendGlobalText()` |
| Guild/party announcements | `PartyAlert`, `GuildAlert` | manager-specific methods |
| Ping/keepalive | `CMSG_PING` / `PONG` | WorldSocket periodically |
| Auction expiry notification | `AuctionClosedNotification` | AuctionHouseMgr |
| Instance reset warning | `InstanceReset` | InstanceLockMgr |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/world-server/src/main.rs` — ~500 lines — TCP listener, session spawning, basic event loop skeleton
- `crates/wow-world/src/lib.rs` — module exports (session, map_manager, handlers)
- `crates/wow-world/src/map_manager.rs` — ~600 lines — MapManager equivalent (map loading, grid tick, NPC update)

**What's implemented:**
- ✅ Async event loop (tokio runtime)
- ✅ TCP listener accepting client connections
- ✅ Session creation per authenticated client
- ✅ Skeleton MapManager with update ticking
- ✅ Basic GUID generation per realm
- ⚠️ Partial map update logic (not all systems hooked)

**What's missing vs C++:**
- ❌ World singleton with config management (bool/int/float configs)
- ❌ Periodic timer system (WUPDATE_* timers not implemented)
- ❌ Server shutdown orchestration (graceful countdown, kick all players)
- ❌ MOTD, server status messages, shutdown announcements
- ❌ Global text/message broadcasting
- ❌ PersistentWorldVariable persistence (world state variables)
- ❌ Integration with managers: AuctionHouseMgr, GuildMgr, BattlegroundMgr, etc. — no lifecycle hooks
- ❌ Config hot-reload support
- ❌ Database maintenance tasks (corpse cleanup, character deletion, ping)
- ❌ Uptime logging to DB
- ❌ Realm info queries (realm name, type, zone, language)
- ❌ Game event orchestration
- ❌ Player count tracking and limits enforcement
- ❌ Security level per-realm loading from DB

**Suspicious / likely divergent (pre-audit hypothesis):**
- **No config system**: C++ World has massive `m_bool_configs[]`, `m_int_configs[]`, `m_float_configs[]` arrays. Rust version would need env vars or TOML file parser. Risk: **all behavior locked to compile-time constants; can't adjust gameplay without rebuild**.
- **Update loop ordering**: C++ has specific order in `World::Update()`: sessions first, then MapManager, then managers. Rust might update async without order guarantee. Risk: **race conditions, desync between player positions and NPC moves**.
- **Timer precision**: C++ uses IntervalTimer with millisecond precision. Rust likely uses coarse wall-clock ticks. Risk: **maintenance tasks fire at wrong time, auctions expire early, uptime log drifts**.
- **Shutdown is not orderly**: C++ `ShutdownServ()` waits, broadcasts countdowns, saves all, then exits. Rust might just kill the runtime. Risk: **data loss, player disconnects without save, orphaned records in DB**.
- **No session registry**: World doesn't own sessions (WorldSessionMgr does), but needs to broadcast to them. If WorldSessionMgr not yet ported, broadcast is a no-op. Risk: **players never get global messages, shutdown announcements don't reach clients**.

**Tests existing:**
- 0 tests for World logic
- Integration tests in CI check server startup only

---

## 9. Migration sub-tasks

- [ ] **#WORLD.1** Create World singleton with config arrays (bool/int/float); load from TOML or environment; expose setter methods with hot-reload hook (complejidad: M)
- [ ] **#WORLD.2** Implement periodic timer system: `IntervalTimer` array for WUPDATE_* timers; tick all timers in update loop, dispatch callbacks (complejidad: M)
- [ ] **#WORLD.3** Port server shutdown orchestration: `ShutdownServ()` method with N-second countdown, broadcast shutdown messages every 10s, save all players/guilds, graceful exit (complejidad: M)
- [ ] **#WORLD.4** Implement session registry integration: lookup sessions by account_id, broadcast messages (text, system packets) to all/filtered sessions (complejidad: L)
- [ ] **#WORLD.5** Implement global message methods: `SendWorldText()`, `SendGlobalMessage()`, `SendServerMessage()` using session broadcaster (complejidad: L)
- [ ] **#WORLD.6** Port PersistentWorldVariable system: load/save int32 world state vars from DB, persist on change (complejidad: L)
- [ ] **#WORLD.7** Integrate all managers into World::Update(): call AuctionHouseMgr, GuildMgr, BattlegroundMgr, etc. with ordered update sequence (complejidad: H)
- [ ] **#WORLD.8** Implement database maintenance timers: corpse cleanup, character deletion, uptime logging, DB ping (complejidad: M)
- [ ] **#WORLD.9** Load realm info from DB on startup: realm name, type, timezone, language, security level (complejidad: L)
- [ ] **#WORLD.10** Implement player count tracking + max player enforcement; track online count, reject login if at limit (complejidad: L)

---

## 10. Regression tests to write

- [ ] Test: World::Update() ticks all timers correctly (one timer fires, others don't)
- [ ] Test: Server shutdown: countdown broadcasts message every 10s, then saves all, then exits
- [ ] Test: Config hot-reload: change int value, World respects new value immediately
- [ ] Test: SendGlobalMessage broadcast reaches all N sessions exactly once
- [ ] Test: SendGlobalMessage with team filter only reaches team members
- [ ] Test: Session lifecycle: new login increments player count, logout decrements
- [ ] Test: Max player limit: 100 players online, 101st login rejected
- [ ] Test: Corpse cleanup timer fires every 20min, expires corpses > 2 hours old
- [ ] Test: Auction expiry timer fires every 60s, removes expired auctions from DB
- [ ] Test: Uptime logging timer fires every 10min, updates login.uptime table
- [ ] Test: PersistentWorldVariable change persists to DB on next flush

---

## 11. Notes / gotchas

1. **World is a singleton** (`sWorld` global). C++ uses `World* sWorld = nullptr` declared globally, initialized in `main()`. Rust must use `lazy_static!` or `once_cell::sync::Lazy` for thread-safe access. **Do NOT use `Mutex` around the whole World** — it will serialize all updates and kill performance.

2. **Update order is critical**. C++ `World::Update()` has explicit order:
   - Tick all sessions (WorldSessionMgr)
   - Tick MapManager (all maps, grids, creatures, objects)
   - Tick managers (Auction, Battlefield, Battleground, etc.)
   - Update world timers (check if WUPDATE_* timers expired, dispatch cleanup)
   
   If Rust does this out of order (e.g., maps before sessions), players might move into loaded maps before sessions are ticked, causing race conditions.

3. **Timer precision matters**. Auctions expire in milliseconds. If a timer drifts by seconds, auctions could last too long or expire too soon. Use `Instant::now()` consistently, not system time.

4. **Shutdown must be graceful**. C++ `ShutdownServ()` sets `m_stopEvent = true`, then the main loop checks this flag. While flag is true, it waits for all sessions to disconnect, saves all data, then exits. If Rust just kills the tokio runtime, character data is lost. Must implement a clean shutdown signal.

5. **Session affinity with broadcast**: when World broadcasts (e.g., SendGlobalMessage), it iterates WorldSessionMgr's session pool. If a session is being destroyed concurrently, iterator might panic. Use Arc<Mutex<Session>> and lock all session access consistently.

6. **Config reload is tricky**. Some config changes require manager restart (e.g., CONFIG_INTERVAL_MAPUPDATE changes MapManager tick interval). Others are just value swaps (CONFIG_DURABILITY_LOSS_IN_PVP). C++ calls manager-specific callbacks on config change. Rust needs similar hook system.

7. **Database pings keep connections alive**. MySQL connections timeout after 8 hours of inactivity. The timer `WUPDATE_PINGDB` fires every 30 minutes with a `SELECT 1` query to prevent timeout. Without this, long-running servers disconnect from DB and crash.

8. **Corpse cleanup can't be instant**. C++ waits N hours (config `CharacterDatabaseCleanerInterval`; default 3600s = 1 hour for deaths < 2 hours). Client expects corpses to linger; instant cleanup breaks UI.

9. **Account-level vs character-level limits**. C++ tracks per-account max sessions and global max player limit. If account tries to log in 2 characters at once (abnormal), either new login fails or old one is kicked. Must enforce in SessionRegistry before creating new WorldSession.

10. **Realm info is static after startup**. C++ loads realm data once in SetInitialWorldSettings. If you change realm name in DB later, server won't see it until restart. Rust should do the same to avoid locks.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class World` | `struct World` (or `struct GameState` to avoid keyword conflict) | singleton; no inheritance |
| `World* sWorld` | `lazy_static!(static ref WORLD: World = ...)` | thread-safe lazy init |
| `std::array<IntervalTimer, WUPDATE_COUNT>` | `Vec<IntervalTimer>` or `[IntervalTimer; WUPDATE_COUNT]` | fixed-size timer array |
| `uint32 m_bool_configs[BOOL_CONFIG_VALUE_COUNT]` | `[bool; BOOL_CONFIG_VALUE_COUNT]` | flat bool array |
| `uint32 m_int_configs[INT_CONFIG_VALUE_COUNT]` | `[u32; INT_CONFIG_VALUE_COUNT]` | flat int array |
| `float m_float_configs[FLOAT_CONFIG_VALUE_COUNT]` | `[f32; FLOAT_CONFIG_VALUE_COUNT]` | flat float array |
| `std::unordered_map<uint32, WorldSession*>` | `HashMap<u32, Arc<Mutex<WorldSession>>>` | session pool (via WorldSessionMgr) |
| `void World::Update(uint32)` | `fn update(&mut self, diff_ms: u32)` | main tick dispatcher |
| `void World::SendGlobalMessage(WorldPacket const*)` | `fn broadcast_packet(&self, packet: &WorldPacket)` | iterate sessions, send |
| `void World::ShutdownServ(...)` | `async fn shutdown(&mut self, delay_sec: u32)` | tokio-aware async shutdown |
| `IntervalTimer` (C++) | `struct IntervalTimer { interval: u32, passed: u32 }` | manual timer tracking |
| `WorldStateManager` | planned `struct PersistentWorldState` | world variable persistence |

---

## 13. Integration with Maps.md and Handlers.md

**Link to Maps:** World ticks MapManager once per update with the same `diff_ms`. MapManager then ticks all loaded maps. See `docs/migration/maps.md` for map update order and creature AI tick.

**Link to Handlers:** When World broadcasts a message (e.g., shutdown announcement), it sends a WorldPacket. Handlers then process SMSG_SERVER_MESSAGE in WorldSession. See `docs/migration/handlers.md` for opcode dispatch details.

**Link to Entities/Player:** When player logs in, MapManager spawns a Player object on the world. World doesn't own the Player directly; it's owned by WorldSession (which got it from ObjectAccessor). But World updates the session, which ticks player's spell casting, movement, etc.

---

*Template version: 1.0. Status: ❌ not started — skeleton exists, major subsystems not ported.*
