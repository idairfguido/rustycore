# Migration: worldserver (game server binary)

> **C++ canonical path:** `src/server/worldserver/`
> **Rust target crate(s):** `crates/world-server/`
> **Layer:** binary (executable entry point)
> **Status:** ⚠️ partial (boot, DB init, listener spawn, session-per-connection work; freeze detector + RA + SOAP + CLI thread + `World::Update` tick loop are all missing)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

The "Trinityd" daemon. Hosts the game world: accepts encrypted WoW client TCP connections on **port 8085** (realm) and **port 8086** (instance), runs `WorldSession` per character, ticks `World::Update(diff)` at a fixed minimum-diff cadence, manages the in-memory game state (`MapManager`, `BattlegroundMgr`, `OutdoorPvPMgr`, `InstanceLockMgr`, `TerrainMgr`, scripts), and exposes the GM admin surface (CLI on stdin, optional Remote Access port, optional SOAP).

---

## 2. C++ canonical files

All paths relative to `/home/server/woltk-trinity-legacy/src/server/worldserver/`.

| File | Lines (approx) | Purpose |
|---|---|---|
| `Main.cpp` | 742 | Entry point; bootstraps everything |
| `CommandLine/CliRunnable.h` / `.cpp` | 30 + 130 | `CliThread()` — stdin reader thread; reads commands, forwards to `World::QueueCliCommand` |
| `RemoteAccess/RASession.h` / `.cpp` | 50 + 150 | `RASession` — per-connection RA console (telnet-like); `Send`, `ReadString`, `CheckAccessLevel`, `ProcessCommand` |
| `TCSoap/TCSoap.h` / `.cpp` | 60 + 160 | `TCSoapThread(host, port)` — gSOAP web service for command exec; `SOAPCommand` future-promise wrapper |
| `worldserver.conf.dist` | — | All ~600 config keys (Network.Threads, MaxCoreStuckTime, Console.Enable, Ra.*, SOAP.*, Updates.*, MaxPingTime, RealmID, Expansion, GM.*, lots more) |

`Main.cpp` is the meat. Notable file-local symbols:
- `class FreezeDetector` — watchdog that aborts the process if `World::m_worldLoopCounter` stops advancing for `MaxCoreStuckTime` ms.
- `void WorldUpdateLoop()` — the actual game tick loop.
- `void SignalHandler(...)` — sets `World::StopNow(SHUTDOWN_EXIT_CODE)`.
- `bool StartDB() / void StopDB()` — opens/closes the four DB pools.
- `AsyncAcceptor* StartRaSocketAcceptor(...)` — RA listener.
- `void ClearOnlineAccounts()` — DB cleanup on start/stop.
- `bool LoadRealmInfo()` — pulls this realm's row from the realmlist.

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `FreezeDetector` | class (file-local in Main.cpp) | Holds a `DeadlineTimer`, polls `World::m_worldLoopCounter` every 1 s, aborts if stuck > `MaxCoreStuckTime` ms |
| `RASession` | class : enable_shared_from_this | One per RA telnet client; auth + command exec |
| `SOAPCommand` | class | Holds a `std::promise<void>` for sync command result |
| `m_ServiceStatus` | global int | Win32 service mode (`-1`, `0`, `1`, `2`) |
| `_TRINITY_CORE_CONFIG` / `_DIR` | macro | `"worldserver.conf"` / `"worldserver.conf.d"` |
| `SHUTDOWN_EXIT_CODE` / `ERROR_EXIT_CODE` | constant (in `World.h`) | `0` / `1` exit codes |

Singletons used (not defined here, but instantiated in `main`):
- `sConfigMgr`, `sLog`, `sWorld`, `sScriptMgr`, `sScriptReloadMgr`, `sBattlegroundMgr`, `sOutdoorPvPMgr`, `sMapMgr`, `sTerrainMgr`, `sInstanceLockMgr`, `sSecretMgr`, `sMetric`, `sRealmList`, `sWorldSocketMgr`.

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `int main(int argc, char** argv)` | Bootstrap (described in §11 step-by-step) | All below |
| `WorldUpdateLoop()` | Main game tick: while `!World::IsStopped()`, increment `m_worldLoopCounter`, compute diff, sleep if `diff < MinWorldUpdateTime`, call `sWorld->Update(diff)` | `World::Update`, `getMSTime` |
| `SignalHandler(error, signum)` | On SIGINT/SIGTERM (+ SIGBREAK Windows): `World::StopNow(SHUTDOWN_EXIT_CODE)` — sets the stop flag; the tick loop exits at next iteration | World |
| `FreezeDetector::Handler(weak<FreezeDetector>, error)` | Every 1 s: read `World::m_worldLoopCounter`; if unchanged for > `MaxCoreStuckTime` ms, `ABORT_MSG("World Thread hangs for %u ms")` (deliberate crash so a supervisor restarts the process) | `getMSTime`, World |
| `StartDB()` | Open all four pools: `LoginDatabase`, `CharacterDatabase`, `WorldDatabase`, `HotfixDatabase`; read `RealmID`; call `ClearOnlineAccounts`; insert version info; `sWorld->LoadDBVersion()` | DatabaseLoader, World |
| `StopDB()` | Close in reverse order: Hotfix, World, Character, Login | — |
| `StartRaSocketAcceptor(io)` | Bind+listen on `Ra.IP:Ra.Port` (default 0.0.0.0:3443); `acceptor->AsyncAccept<RASession>()` | AsyncAcceptor |
| `LoadRealmInfo()` | Copy this realm's row out of `sRealmList` into the global `realm` struct | RealmList |
| `ClearOnlineAccounts()` | DB cleanup: `account.online = 0` for accounts with chars on this realm; `characters.online = 0`; reset `character_battleground_data.instanceId` | LoginDatabase, CharacterDatabase |
| `ShutdownCLIThread(thread*)` | Cancel pending stdin read on Windows (via `CancelSynchronousIo` + simulated keypress); `join()` + `delete` on Linux | — |
| `CliThread()` | Reads lines from stdin, posts to `World::QueueCliCommand`; supports `quit`, `exit`, `server *`, etc. | World |
| `RASession::Start()` | TCP connect → request username/password → `CheckAccessLevel` (gmlevel 3+ only) → loop reading commands and feeding to `CliCommandHolder` | World, AccountMgr |
| `TCSoapThread(host, port)` | Spawn gSOAP server, `process_message` per request, set `SOAPCommand` promise on completion | World |

### `main()` step-by-step (TC behaviour)

1. Install `SIGABRT` → `Trinity::AbortHandler` (writes a coredump preamble).
2. `Trinity::VerifyOsVersion()` — refuses to run on too-old Win.
3. `Trinity::Locale::Init()` — sets `setlocale(LC_ALL, "C")`.
4. Parse CLI: `--config`, `--config-dir`, `--update-databases-only`, `--service`, `--help`, `--version`.
5. `GOOGLE_PROTOBUF_VERIFY_VERSION` + RAII shutdown handle.
6. Win32: install/uninstall/run service if requested. Set `timeBeginPeriod(1ms)` for sub-15ms `Sleep` resolution.
7. `sConfigMgr->LoadInitial(configFile, argv)` then `LoadAdditionalDir(configDir)` then `OverrideWithEnvVariablesIfAny()`.
8. **Build a single `boost::asio::io_context`** shared by network, log async, signal handler, freeze detector, metrics.
9. `sLog->RegisterAppender<AppenderDB>(); sLog->Initialize(asyncLogIo)`.
10. `Trinity::Banner::Show("worldserver-daemon", ...)`.
11. `OpenSSLCrypto::threadsSetup(...)` (RAII `threadsCleanup`).
12. `BigNumber seed; seed.SetRand(16*8)` — pre-seed the OpenSSL PRNG so first login isn't slow.
13. `CreatePIDFile(PidFile)` if config sets it.
14. `boost::asio::signal_set signals(*io, SIGINT, SIGTERM)` → `async_wait(SignalHandler)`.
15. **`ThreadPool threadPool(numThreads = ThreadPool config or 1)`** + post `numThreads` workers each calling `io->run()`. RAII handle stops the io_context on drop.
16. `SetProcessPriority(...)` (affinity + high-priority flag).
17. `StartDB()`. RAII `StopDB`. If `--update-databases-only`, exit.
18. `Trinity::Net::ScanLocalNetworks()` — caches local subnets for `realm` IP-routing decisions.
19. `LoginDatabase.DirectPExecute("UPDATE realmlist SET flag = flag | OFFLINE WHERE id = realm.Id")` — set realm offline for boot duration.
20. `sRealmList->Initialize(*io, RealmsStateUpdateDelay)`.
21. `LoadRealmInfo()`.
22. `sMetric->Initialize(realmName, *io, lambda)` — emits `online_players`, `db_queue_*` periodically.
23. `sScriptMgr->SetScriptLoader(AddScripts)`. RAII unloads ScriptMgr + ScriptReloadMgr.
24. `sSecretMgr->Initialize(SECRET_OWNER_WORLDSERVER)`.
25. **`sWorld->SetInitialWorldSettings()` — the big one.** Loads ~150 DB tables, all DB2 stores, scripts, etc.
26. RAII handle that on shutdown unloads BG templates, OutdoorPvP, MapMgr, TerrainMgr, InstanceLockMgr.
27. `if (Ra.Enable) StartRaSocketAcceptor(*io)`.
28. `if (SOAP.Enabled) std::thread(TCSoapThread, ip, port)` (joined by RAII).
29. `sWorldSocketMgr.StartWorldNetwork(*io, BindIP, WorldServerPort, InstanceServerPort, NetworkThreads)`.
30. RAII shutdown: `KickAll`, `UpdateSessions(1)` to flush, `StopNetwork`, `ClearOnlineAccounts`.
31. `LoginDatabase.DirectPExecute(... clear OFFLINE flag ...)`.
32. `if (MaxCoreStuckTime > 0) FreezeDetector::Start(...)`.
33. `sScriptMgr->OnStartup()`.
34. `if (Console.Enable) std::thread(CliThread)` (RAII shutdown).
35. **`WorldUpdateLoop()`** — blocks until shutdown.
36. Cleanup: `ConnectTo::ShutdownEncryption()`, `EnterEncryptedMode::ShutdownEncryption()`, drop `ioContextStopHandle`, drop `threadPool`, `sLog->SetSynchronous()`, `sScriptMgr->OnShutdown()`, set realm OFFLINE, return `World::GetExitCode()`.

---

## 5. Module dependencies

**Depends on:**
- Everything in `src/server/game/` (transitively): `World`, `WorldSocket`, `WorldSocketMgr`, `MapManager`, `TerrainMgr`, `BattlegroundMgr`, `OutdoorPvPMgr`, `InstanceLockMgr`, `ScriptMgr`, `ScriptReloadMgr`, `Metric`
- `src/common/`: `Config`, `Log` (`AppenderDB`), `DatabaseLoader`, `LoginDatabase` / `CharacterDatabase` / `WorldDatabase` / `HotfixDatabase`, `MySQLThreading`, `OpenSSLCrypto`, `SecretMgr`, `IoContext`, `AsyncAcceptor`, `ThreadPool`, `ProcessPriority`, `Realm`, `RealmList`, `IpNetwork`, `Banner`, `BigNumber`, `Locale`
- `boost`: `asio`, `program_options`, `dll`, `filesystem`
- `OpenSSL`, `protobuf`
- `gsoap` (for TCSoap)

**Depended on by:**
- The WoW client (game-state side; the launcher hits bnetserver, in-game session hits worldserver)
- An optional supervisor (systemd / runit) that restarts the binary on freeze-detector abort

---

## 6. SQL / DB queries (if any)

Direct queries in `Main.cpp` (the rest is in modules):

| Statement / Source | Purpose | DB |
|---|---|---|
| `UPDATE realmlist SET flag = flag \| OFFLINE WHERE id = ?` | Mark realm offline at boot / shutdown | login |
| `UPDATE realmlist SET flag = flag & ~OFFLINE, population = 0 WHERE id = ?` | Mark realm online after listener starts | login |
| `UPDATE version SET core_version = ?, core_revision = ?` | One-shot version-info record | world |
| `UPDATE account SET online = 0 WHERE online > 0 AND id IN (SELECT acctid FROM realmcharacters WHERE realmid = ?)` | Cleanup: clear stale online flags | login |
| `UPDATE characters SET online = 0 WHERE online <> 0` | Cleanup: clear stale character online flags | characters |
| `UPDATE character_battleground_data SET instanceId = 0` | BG instance ids are per-restart | characters |

DBC/DB2 stores: not loaded here; loaded by `World::SetInitialWorldSettings` (covered in `world.md`).

DBUpdater (auto-applies pending `.sql` files) is invoked by `DatabaseLoader::Load()` for each pool. See `wow-database/src/updater.rs` for the Rust port.

---

## 7. Wire-protocol packets (if any)

`worldserver` doesn't define opcodes. It opens listeners on:

| Port | Protocol | Used by |
|---|---|---|
| `WorldServerPort` (def. 8085) | Encrypted WoW TCP (header HMAC-SHA256, body AES-GCM) | The WoW 3.4.3.54261 client realm connection |
| `InstanceServerPort` (def. 8086) | Same, but for instance-bound transfers (`SMSG_REDIRECT_CLIENT` / `SMSG_NEW_WORLD`) | Same client, after `ConnectTo` |
| `Ra.Port` (def. 3443) | Plain TCP, telnet-like | `RASession` |
| `SOAP.Port` (def. 7878) | HTTP SOAP | gsoap |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/world-server/src/main.rs` — 818 lines — entry point, async runtime, account-lookup struct, listener spawn, session create

**What's implemented:**
- Tokio async runtime (`#[tokio::main]`), `tracing` subscriber.
- Config loading (`WorldServer.conf` then `WorldServer.conf.dist` fallback).
- Four DB pools opened: login, characters, world, hotfix.
- `wow_database::updater::DbUpdater::populate(...)` + `update(...)` for the auth and characters DBs (and `update` only for world + hotfix). Equivalent to TC's `DatabaseLoader` + `DBUpdater` running auto-update.
- `ObjectGuidGenerator` initialized from `MAX(guid)` in `characters`.
- DB2 / static-data stores loaded into `Arc`s and shared via `SessionResources`: `ItemStore`, `PlayerStatsStore`, `ItemStatsStore`, `HotfixBlobCache`, `SkillStore`, `SpellStore`, `area_trigger_store`, `quest_store`, `QuestXpStore`, `player_xp_table`.
- Realm gamebuild + `Win64AuthSeed` loaded from `realmlist` + `build_info`.
- Realm external/local addresses loaded from `realmlist`.
- `wow_handler::build_dispatch_table()` constructs the per-opcode handler registry (same role as TC's `OpcodeTable::Initialize()`).
- `DbAccountLookup` impl of `wow_network::world_socket::AccountLookup` — resolves `(realm_id, ticket)` → `AccountInfo` (account_id, session_key_bnet, expansion, security, ban flags, locale, OS, …).
- Two listeners spawned: `start_world_listener(realm_addr, ...)` and `start_instance_listener(instance_addr, ...)` — see `wow-network` for the accept-loop implementation.
- Per-accepted-connection `create_session(...)` builds a `WorldSession`, wires all the resource Arcs into it, calls `send_session_init_packets` and runs a 50 ms tick loop:
  - `session.update(50)` — process inbound queued packets
  - `session.process_pending().await` — async DB callbacks
  - `if disconnecting break` else `tokio::time::sleep(50ms)`
- Shutdown via `tokio::select! { ctrl_c => ..., listener_join => ... }`.
- `get_address_for_client` — replicates TC's `Trinity::Net::SelectAddressForClient`-style "loopback or same /24 → local, else external".

**What's missing vs C++:**
- **`World::Update(diff)` global tick loop.** TC's `WorldUpdateLoop` is a single thread that drives **all** gameplay state for the whole server. RustyCore's design is per-session: each session runs its own 50 ms tick. There is **no global `WorldSessionMgr::Update`, no `MapManager::Update`, no global creature ticks aggregated across sessions, no `World::m_worldLoopCounter`**. This is the most fundamental architectural divergence between the two stacks. The `_attic/` README documents the partial migration toward `MapManager`-shared state; the global-tick driver is still future work.
- **`FreezeDetector`** — there is no equivalent. If the runtime hangs, only an external supervisor (systemd `WatchdogSec=`) catches it.
- **`MaxCoreStuckTime` config**: read but ignored.
- **`MinWorldUpdateTime` config**: ignored. The Rust tick is hardcoded `Duration::from_millis(50)`.
- **`World::IsStopped()` / `World::StopNow(code)` / `World::GetExitCode()`**: there is no `World` singleton. Shutdown is just `ctrl_c` → drop listeners. Exit code always 0.
- **DB keep-alive timer (`MaxPingTime`)**: not implemented (see same gap in bnetserver).
- **`AppenderDB`** (logs into `logs.logs` table): not implemented; tracing only goes to stderr / journald.
- **`Banner::Show`**: only a single `info!("RustyCore World Server starting...")`.
- **`OpenSSLCrypto` setup / `BigNumber::SetRand(...)` warmup**: irrelevant (rustls / `getrandom`).
- **`CreatePIDFile`**: missing.
- **`SecretMgr::Initialize(SECRET_OWNER_WORLDSERVER)`**: missing.
- **`ScanLocalNetworks`**: there is `get_address_for_client` which approximates TC's behaviour but only checks `/24` against `realm_local_address`, not the full set of host interfaces.
- **`ClearOnlineAccounts`** at boot: missing — stale `online = 1` rows linger across crashes.
- **`UPDATE realmlist SET flag = flag | OFFLINE`** at boot / `& ~OFFLINE` after listener-up / on shutdown: missing. The realm is "always online" from the realmlist's POV.
- **`LoadRealmInfo`** equivalent: only loads gamebuild/seed/addresses; doesn't populate a global `realm` struct equivalent — addresses are passed via `SessionResources`.
- **`sRealmList->Initialize(io, RealmsStateUpdateDelay)`** background refresh of cross-realm registry: missing. (Single-realm setups don't notice; multi-realm wouldn't work.)
- **`sMetric->Initialize`** — no metrics subsystem.
- **`sScriptMgr->OnStartup() / OnShutdown()`** hooks: missing (the wow-script dispatch infrastructure exists but startup hook isn't wired).
- **`Trinity::Asio::DeadlineTimer`-driven periodic tasks**: none of TC's housekeeping timers are present (DB ping, metric tick, script reload check).
- **CLI thread (`CliThread`)** reading from stdin and dispatching `.commands`: missing. There's no GM console.
- **Remote Access (`Ra.Enable`, port 3443)**: missing.
- **SOAP (`SOAP.Enabled`, port 7878)**: missing.
- **Win32 service mode**: out of scope.
- **`SetProcessPriority` / processor affinity**: out of scope.
- **`ThreadPool` config**: ignored — Tokio handles threading.
- **CLI args (`--config`, `--update-databases-only`, `--version`, `--help`)**: not parsed.
- **`KickAll` + `UpdateSessions(1)` flush at shutdown**: missing — sessions are dropped abruptly.
- **`WorldPackets::Auth::ConnectTo::ShutdownEncryption / EnterEncryptedMode::ShutdownEncryption`** — these tear down the static keys. RustyCore probably has them per-session, so the global tear-down may be a no-op.
- **`sBattlegroundMgr->DeleteAllBattlegrounds()`, `sOutdoorPvPMgr->Die()`, `sMapMgr->UnloadAll()`, `sTerrainMgr.UnloadAll()`, `sInstanceLockMgr.Unload()` in shutdown order**: only a partial `MapManager` exists; rest is missing.
- **`AbortHandler` on `SIGABRT`**: missing.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- The 50 ms-per-session sleep is **independent across sessions** — there's no global "we ticked everyone simultaneously". If a 25-player raid all has slightly different tick offsets, AoE/aura ticks visible to different clients will land on different real-time millisecond boundaries. Likely visible in PvP combat timing.
- Without `MapManager` running its own tick (creature AI / spawn respawn / movement interpolation between sessions), creatures only "update" when *some* session looks at them. Idle creatures freeze.
- `tokio::time::sleep(Duration::from_millis(50))` is the *floor*, not the period. If `session.update(50)` blocks for >50 ms, the next tick is delayed. TC's `MinWorldUpdateTime` enforces a *floor* explicitly; we get the same effective behaviour, but TC also logs a warning if `sleepTime >= MaxCoreStuckTime / 2`.
- Without `sRealmList`, the BNet realm-list response is built from a stale snapshot loaded once at boot; adding a realm row to MySQL won't be picked up.
- Database connection pool sizing: TC opens three pools per database (`SyncPool` + `AsyncPool` + a small one for callbacks). The Rust `wow-database` opens a single `sqlx::Pool<MySql>` per logical DB. Sizing under heavy login load may differ.

**Tests existing:**
- 0 in `world-server` itself; integration tests live in `wow-world` and `wow-network`.

---

## 9. Migration sub-tasks

- [ ] **#WS.1** Implement `World` singleton (or equivalent state holder): `is_stopped() -> bool`, `stop_now(exit_code: i32)`, `get_exit_code() -> i32`, `m_world_loop_counter: AtomicU32`. (M)
- [ ] **#WS.2** Implement a global `WorldUpdateLoop` task that ticks `MapManager` + (eventually) every `WorldSession` from one place, at a `MinWorldUpdateTime` cadence. Increment the loop counter each iteration. (XL — coupled to migrating sessions off per-task ticks; cf. `_attic/` notes)
- [ ] **#WS.3** Implement `FreezeDetector`: `tokio::time::interval(1s)`; reads `m_world_loop_counter`; if unchanged for `MaxCoreStuckTime` ms, `tracing::error!` + `std::process::abort()`. (M)
- [ ] **#WS.4** Implement `ClearOnlineAccounts()` — call at boot and at shutdown; three queries listed in §6. (L)
- [ ] **#WS.5** Implement realmlist OFFLINE flag toggle at boot + listener-ready + shutdown. (L)
- [ ] **#WS.6** DB keep-alive: every `MaxPingTime` minutes, `SELECT 1` against each of the 4 pools. (L)
- [ ] **#WS.7** Implement `AppenderDB` equivalent for `tracing`: a layer that batches log records into `logs.logs` table. Optional. (M)
- [ ] **#WS.8** Add CLI thread: `tokio::task::spawn_blocking` reading stdin, posting commands to a `CliCommandQueue` consumed in the main tick. Wire a small set of commands first (`server info`, `server shutdown`, `account create`). (H)
- [ ] **#WS.9** Implement RA listener (`Ra.Enable`): bind on `Ra.IP:Ra.Port`, per-connection auth (gmlevel ≥ `Ra.MinLevel`), pipe commands into the same `CliCommandQueue`. (H)
- [ ] **#WS.10** Decide on SOAP: drop or implement. The XML/SOAP surface is rarely used today; recommend **drop** with config warning. (L if drop, XL if implement)
- [ ] **#WS.11** Wire `wow_account::secrets::initialize(SecretOwner::WorldServer).await` (depends on `accounts.md` #ACC.6 / new SecretMgr port). (M)
- [ ] **#WS.12** Wire `sRealmList`-equivalent background refresh (`RealmsStateUpdateDelay`): polls `realmlist` periodically, updates an in-memory `Arc<RwLock<Vec<RealmEntry>>>`. Needed once RustyCore supports more than one realm row. (M)
- [ ] **#WS.13** Implement `sMetric` equivalent: emit `online_players`, `db_queue_*` to an OpenTelemetry / Prometheus exporter. (M)
- [ ] **#WS.14** Implement `sScriptMgr->on_startup()` / `on_shutdown()` hooks. (L)
- [ ] **#WS.15** Implement clean shutdown: kick all sessions (send `SMSG_LOGOUT_RESPONSE` then drop), wait up to N seconds for character saves, close listeners, drop registries, close DBs, set realm OFFLINE. (H)
- [ ] **#WS.16** CLI args via `clap`: `--config`, `--config-dir`, `--update-databases-only`, `--version`, `--help`. (L)
- [ ] **#WS.17** PID file (`PidFile` config). (L)
- [ ] **#WS.18** `SIGTERM` handler in addition to `ctrl_c` (`tokio::signal::unix::signal(SIGTERM)`); both should trigger the same shutdown path. (L)
- [ ] **#WS.19** Pre-listener startup banner with build hash, sqlx version, rustls version, DB versions (one log line per connected DB). (L)
- [ ] **#WS.20** Replace per-session `tokio::time::sleep(50ms)` with a `tokio::sync::broadcast` "tick" signal driven by the global `WorldUpdateLoop`. (M, depends on #WS.2)
- [ ] **#WS.21** Connection-pool sizing: expose `LoginDatabaseInfo.{Sync,Async}.PoolSize` config; pass to `sqlx::PoolOptions`. (L)

---

## 10. Regression tests to write

- [ ] Test: `World::is_stopped()` flips to `true` on `SIGINT` and the `WorldUpdateLoop` exits within one tick interval.
- [ ] Test: `FreezeDetector` aborts the process if the global tick counter doesn't advance for > `MaxCoreStuckTime` ms (use a deliberately blocking handler in a test build).
- [ ] Test: `ClearOnlineAccounts` at boot zeroes `account.online` for accounts with characters on the current realm and **only** that realm.
- [ ] Test: realmlist `flag` column has `REALM_FLAG_OFFLINE` set during startup (after `StartDB`, before `WorldUpdateLoop`) and again after shutdown.
- [ ] Test: DB keep-alive ping fires every `MaxPingTime` minutes (use `tokio::time::pause` for time-skipping).
- [ ] Test: with `Console.Enable=true`, sending `server info\n` to stdin produces an info log; `server shutdown\n` triggers `World::stop_now(SHUTDOWN_EXIT_CODE)`.
- [ ] Test: with `Ra.Enable=true`, a TCP connection to `Ra.Port` requires a username + password, validates `gmlevel >= Ra.MinLevel`, then runs the same command surface.
- [ ] Test: shutdown sequence — when SIGINT arrives, every connected session receives `SMSG_LOGOUT_RESPONSE`, character data is saved, then listeners close. Total time bounded by `ShutdownLatencyMax`.
- [ ] Test: `cargo run --bin world-server -- --update-databases-only` runs DBUpdater for all four DBs and exits 0 without binding any port.
- [ ] Test: `cargo run --bin world-server -- --version` prints `RustyCore worldserver <hash>` and exits.
- [ ] Test: with `MaxCoreStuckTime=0`, `FreezeDetector::Start` is not called (TC behaviour: 0 disables).

---

## 11. Notes / gotchas

- **The `World::Update` tick is the single source of truth for game time.** TC's `getMSTime()` is sampled once per loop iteration; everything inside that iteration sees the same `realCurrTime`. Per-session ticks in RustyCore mean each session has its own time. Combat timing, aura tick alignment, AoE radii on moving targets — all have subtle dependencies on a global tick. Plan the migration to a global tick early or expect bugs that "only show up in raids".
- **`MinWorldUpdateTime` default is 1 ms**, but TC uses `getMSTime` which is *millisecond-resolution*. So the loop effectively runs as fast as the OS scheduler allows. `MaxCoreStuckTime` default is 60 seconds.
- **`ABORT_MSG` in `FreezeDetector::Handler` is intentional**: TC wants the process to crash and be restarted by the supervisor. This is more reliable than trying to recover. Replicate this verbatim — `std::process::abort()` (which dumps core), not `std::process::exit(1)`.
- **The freeze-detector polls `World::m_worldLoopCounter`, not "is the io_context running"**. If only the game logic stalls (DB query stuck, infinite loop in a script), but the network thread keeps reading, the freeze detector still catches it because it's gated on the counter. Replicate that property — *don't* gate it on Tokio's I/O.
- **Shutdown order matters**: BattlegroundMgr → OutdoorPvP → MapMgr → TerrainMgr → InstanceLockMgr. Reversing gives use-after-free in TC, where each manager holds raw pointers into the previous. Rust's borrow checker will help you find equivalent ordering bugs at compile time, but the semantic ordering still matters because of background tasks.
- **TC's `Trinity::ThreadPool`** runs N threads against a single `io_context`. **Tokio is not the same** — Tokio's worker count is set at runtime construction, and there's no equivalent of "this thread polls io_context until shutdown". Don't try to expose `Network.Threads` literally.
- **`UPDATE realmlist SET flag = flag | OFFLINE`** at startup tells the bnetserver "don't list this realm to clients". The `flag = flag & ~OFFLINE` after the listener is up reverses it. If you crash between the two, the realm stays OFFLINE in the DB until the next clean run.
- **WoW client behaviour at shutdown**: if you just close the socket, the client shows "connection lost" and may try to reconnect. To trigger the proper "World server is shutting down" UI, you have to send `SMSG_NOTIFICATION_TEXT` or the queued-shutdown packets from `World::ShutdownServ`. Currently RustyCore does neither.
- **DB connection-pool sizing**: under heavy login (server reboot at peak), all four pools see a thundering herd. TC tunes `Login.SynchPool=1, Login.AsyncPool=4`. The default sqlx `max_connections=10` may be too low or too high depending on workload.
- **`character_battleground_data.instanceId = 0`** at boot: if a player was inside a BG when the server crashed, this lets them log back into a fresh location instead of into the dead BG instance.
- **`Console.Enable`** on Linux: the C++ `CliThread` reads from stdin via `fgets`. If the binary is run via `nohup` or in a systemd unit without `StandardInput=tty`, stdin reads return EOF immediately and the thread exits. Replicate this: `tokio::task::spawn_blocking` reading from stdin, gracefully exit on EOF, do **not** trigger shutdown.
- **`AsyncAcceptor`** in TC is a thin wrapper over `boost::asio::tcp::acceptor`. The Rust counterpart is whatever `wow-network` exposes (`start_world_listener`, `start_instance_listener`).
- **All four DB connections must succeed** for the binary to start. Failure to reach `world` or `hotfixes` is fatal. Currently `world-server/main.rs` matches this — `.context(...)?` short-circuits.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `int main(int argc, char** argv)` | `#[tokio::main] async fn main() -> anyhow::Result<()>` | — |
| `boost::asio::io_context` (single, shared) | Tokio multi-thread runtime (implicit) | TC has *one* io_context for everything; Rust just spawns tasks. |
| `Trinity::ThreadPool threadPool(N)` posting `io->run()` | Tokio worker thread count (`tokio::runtime::Builder::worker_threads`) | Don't expose `ThreadPool` config literally. |
| `boost::asio::signal_set signals(io, SIGINT, SIGTERM)` + `async_wait` | `tokio::select! { _ = tokio::signal::ctrl_c() => ..., _ = sigterm.recv() => ..., }` | Both must drive the same shutdown path. |
| `class FreezeDetector { DeadlineTimer; uint32 _worldLoopCounter; }` | `struct FreezeDetector { last_change_ms: AtomicU64, max_stuck_ms: u64 }` polled by a `tokio::time::interval(1s)` task | Increment a global `AtomicU32` from the global tick. |
| `World::m_worldLoopCounter` | `static WORLD_LOOP_COUNTER: AtomicU32` (or field on a `World` singleton) | — |
| `WorldUpdateLoop()` | `async fn world_update_loop(world: Arc<World>)` running until `world.is_stopped()` | The body uses `tokio::time::sleep` for floor enforcement, *not* an `interval` (interval would catch up on missed ticks). |
| `World::StopNow(code)` | `World::stop_now(&self, code: i32)` setting `AtomicBool::store(true)` and `AtomicI32::store(code)` | — |
| `Trinity::Asio::DeadlineTimer` (recursive rearm) | `tokio::time::interval` + `loop { tick().await; ... }` | The recursive `async_wait`-rearm pattern in TC becomes a flat loop. |
| `class RASession` | `async fn ra_session(stream: TcpStream)` task | Plain TCP. Auth + `match cmd { ... }`. |
| `void TCSoapThread(host, port)` | (recommend dropping) | If kept: `axum` server with a SOAP-XML handler. |
| `void CliThread()` | `tokio::task::spawn_blocking(stdin_reader)` + a `tokio::sync::mpsc` channel | Don't `.await` blocking stdin in async code. |
| `sWorld->SetInitialWorldSettings()` | The accumulation of all `*Store::load(...)` calls + handler-table build | Lives in `world.md` — already partly done. |
| `sScriptMgr->OnStartup() / OnShutdown()` | `wow_scripts::lifecycle::{on_startup, on_shutdown}().await` | Both currently no-ops. |
| `sMetric->Initialize(realmName, io, lambda)` | `wow_metric::initialize(realm_name, ||{ emit_periodic() })` (TODO crate) | — |
| `Trinity::Net::ScanLocalNetworks()` | `wow_network::scan_local_networks()` (TODO; only `/24` heuristic right now) | — |
| `LoginDatabase.DirectPExecute("UPDATE realmlist ...", flag, realmId)` | `login_db.direct_execute(&format!("UPDATE realmlist SET ... WHERE id = {realm_id}"))?` | Already used in `load_realm_addresses`. |
| `MySQL::Library_Init() / End()` | (none) | sqlx handles libmysqlclient internally; nothing to call. |
| `BigNumber seed; seed.SetRand(16*8)` | (none) | rustls / `getrandom` seed automatically. |
| `OpenSSLCrypto::threadsSetup(...)` | (none) | rustls. |
| `ABORT_MSG("World Thread hangs ...")` | `tracing::error!(...); std::process::abort();` | `abort()` not `exit(1)` — for coredump. |
| `sLog->Initialize(asyncIo)` + `AppenderDB` | `tracing_subscriber::fmt().init()` (currently no DB sink) | TODO #WS.7. |
| `CreatePIDFile(path)` | `std::fs::write(path, std::process::id().to_string())?` | TODO #WS.17. |
| `boost::program_options::variables_map` | `clap::Parser` | TODO #WS.16. |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.
