# Migration: Instances (Dungeons / Raids — Lock + Script)

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Instances/`
> **Rust target crate(s):** `crates/wow-instances/` (NOT YET CREATED — must be added to workspace)
> **Layer:** L7
> **Status:** ❌ not started
> **Audited vs C++:** ✅ audited 2026-05-01 (❌ confirmed)
> **Last updated:** 2026-05-01

---

## 1. Purpose

The Instances module manages **persistent dungeon and raid state** for TrinityCore: per-character / per-group lockouts, per-instance encounter progress (boss states, doors, minions), difficulty (Normal / Heroic, 10/25 raid sizes), instance ID assignment, and weekly/daily reset timers. It is the bridge between the transient `InstanceMap` (created on demand from `MapManager`) and the durable lock data that survives server restarts and cross-character lookups.

Two distinct subsystems live here:
- **`InstanceLockMgr`** — DB-backed persistence of *who is locked to what*. Tracks `(playerGuid, mapId, lockId)` → `InstanceLock` (with `instanceId`, `expiryTime`, `extended`, `data`, `completedEncountersMask`, `entranceWorldSafeLocId`). Handles ID-based vs lock-id-based binding, shared-state for raid-id locks, lock extension, and reset.
- **`InstanceScript`** — Per-map runtime state machine (boss states `NOT_STARTED|IN_PROGRESS|FAIL|DONE|SPECIAL`, door behaviors, minion linking, dungeon encounter ↔ DBC `DungeonEncounterEntry`, persistent script values, achievement criteria hook). Subclassed by every dungeon/raid script (Naxxramas, Ulduar, ICC, etc.).

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `game/Instances/InstanceLockMgr.cpp` | 599 | `prefix` |
| `game/Instances/InstanceLockMgr.h` | 320 | `prefix` |
| `game/Instances/InstanceScript.cpp` | 971 | `prefix` |
| `game/Instances/InstanceScript.h` | 461 | `prefix` |
| `game/Instances/InstanceScriptData.cpp` | 270 | `prefix` |
| `game/Instances/InstanceScriptData.h` | 86 | `prefix` |
| `game/Instances/enuminfo_InstanceScript.cpp` | 76 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

Todas las rutas relativas a `/home/server/woltk-trinity-legacy/`.

| File | Lines (aprox) | Purpose |
|---|---|---|
| `src/server/game/Instances/InstanceLockMgr.h` | 320 | `InstanceLock`, `SharedInstanceLock`, `InstanceLockMgr`, `MapDb2Entries`, `InstanceLockUpdateEvent` |
| `src/server/game/Instances/InstanceLockMgr.cpp` | 599 | DB load/save, `CanJoinInstanceLock`, `FindActiveInstanceLock`, `UpdateInstanceLockForPlayer`, `ResetInstanceLocksForPlayer`, `GetNextResetTime` |
| `src/server/game/Instances/InstanceScript.h` | 461 | `InstanceScript` base class, `BossInfo`, `DoorData`, `MinionData`, `ObjectData`, `EncounterState`, `EncounterDoorBehavior`, `PersistentInstanceScriptValue<T>` |
| `src/server/game/Instances/InstanceScript.cpp` | 971 | Boss state machine, door/minion bookkeeping, save-data serialization, world-state push, combat-resurrection, `SendEncounterStart/End`, `UpdateLfgEncounterState` |
| `src/server/game/Instances/InstanceScriptData.h` | 86 | Persistent value variant wrappers + serialization helpers |
| `src/server/game/Instances/InstanceScriptData.cpp` | 270 | rapidjson-based load/save of instance script blobs (boss states + persistent values) |
| `src/server/game/Maps/InstanceMap.cpp` (related) | n/a | `InstanceMap::CreateInstanceData`, glue to `InstanceLockMgr` (covered partially in `maps.md`) |
| `src/server/game/Maps/MapManager.cpp` (related) | n/a | Instance ID allocation (`GenerateInstanceId`), reset scheduling (`InitInstanceIds`, `DoForAllMapsWithMapId`) |

NOTE: `InstanceSaveMgr` no longer exists as a separate class in this WoLK 3.4.3 fork — its responsibilities were absorbed into `InstanceLockMgr` (plus per-instance state in `InstanceMap` / `InstanceScript`). The reset cron is in `MapManager::Update` + `InstanceLockMgr::GetNextResetTime`.

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `InstanceLockData` | struct | `Data` (script blob string), `CompletedEncountersMask` (u32), `EntranceWorldSafeLocId` (u32) |
| `InstanceLock` | class | Per-player view of a lock: mapId, difficultyId, instanceId, expiryTime, extended, `_isInUse`, `_isNew` |
| `SharedInstanceLockData` | struct (extends `InstanceLockData`) | Authoritative shared state for ID-based locks (`InstanceId`) |
| `SharedInstanceLock` | class (extends `InstanceLock`) | Holds `shared_ptr<SharedInstanceLockData>`; UI shows player view, real instance reads shared |
| `MapDb2Entries` | struct | Pair of `MapEntry*` + `MapDifficultyEntry*`; provides `GetKey()` → `(mapId, lockId)` and `IsInstanceIdBound()` |
| `InstanceLockKey` | typedef | `pair<u32, u32>` = `(MapDifficultyEntry::MapID, MapDifficultyEntry::LockID)` |
| `InstanceLockUpdateEvent` | struct | New data to be merged into a lock (`InstanceId`, `NewData`, `InstanceCompletedEncountersMask`, `CompletedEncounter*`, `EntranceWorldSafeLocId`) |
| `InstanceLocksStatistics` | struct | `InstanceCount`, `PlayerCount` (for GM commands) |
| `InstanceLockMgr` | singleton | Owns all loaded locks; `Load()`, `CanJoinInstanceLock()`, `FindActiveInstanceLock()`, `UpdateInstanceLockForPlayer()`, `ResetInstanceLocksForPlayer()` |
| `InstanceScript` | class (extends `ZoneScript`) | Per-map script base; owns `bosses`, `doors`, `minions`, `_objectGuids`, persistent values, entrance, combat-res |
| `BossInfo` | struct | `state`, `door[Behavior::Max]` (per-behavior guid sets), `minion`, `boundary`, `DungeonEncounters[4]` |
| `DoorInfo` / `MinionInfo` | struct | Back-reference from GO/creature entry → `BossInfo*` + behavior |
| `DoorData` / `MinionData` / `ObjectData` / `DungeonEncounterData` / `BossBoundaryEntry` | struct | Static load data (compile-time arrays from script files) |
| `EncounterState` | enum | `NOT_STARTED=0, IN_PROGRESS=1, FAIL=2, DONE=3, SPECIAL=4, TO_BE_DECIDED=5` |
| `EncounterDoorBehavior` | enum class | `OpenWhenNotInProgress=0, OpenWhenDone=1, OpenWhenInProgress=2, OpenWhenNotDone=3` |
| `EncounterFrameType` | enum | Wire-frame opcode payloads: ENGAGE, DISENGAGE, UPDATE_PRIORITY, ADD_TIMER, ENABLE/UPDATE/DISABLE_OBJECTIVE, COMBAT_RES_LIMIT |
| `UpdateBossStateSaveDataEvent` / `UpdateAdditionalSaveDataEvent` | struct | Inputs to `InstanceScriptData` JSON serializer |
| `PersistentInstanceScriptValueBase` / `PersistentInstanceScriptValue<T>` | template class | Type-erased `variant<int64,double>` wrapper with auto-notify-on-change |
| `INSTANCE_ID_HIGH_MASK` / `_LFG_MASK` / `_NORMAL_MASK` | constexpr | `0x1F440000`, `0x00000001`, `0x00010000` — bitfields encoded into instance IDs |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `InstanceLockMgr::Load()` | Loads `instance` + `character_instance_lock` rows; reconstructs `_temporaryInstanceLocksByPlayer`, `_instanceLocksByPlayer`, `_instanceLockDataById` | `CharacterDatabase`, `sDB2Manager` |
| `InstanceLockMgr::CanJoinInstanceLock(playerGuid, entries, lock)` | Returns `TransferAbortReason` (NONE / ALREADY_COMPLETED_ENCOUNTER / DIFFICULTY / LOCKED_TO_DIFFERENT_INSTANCE / etc.) | `MapDb2Entries::IsInstanceIdBound`, `InstanceLock::IsExpired` |
| `InstanceLockMgr::FindActiveInstanceLock(playerGuid, entries)` | Resolves owner (player guid or group-leader guid) → active lock; honors `extended` for expired entries | private `FindInstanceLock` |
| `InstanceLockMgr::CreateInstanceLockForNewInstance(...)` | Allocates new (ephemeral) lock pre-encounter; goes into `_temporaryInstanceLocksByPlayer` until first boss kill | `GetNextResetTime` |
| `InstanceLockMgr::UpdateInstanceLockForPlayer(trans, ...)` | Promotes temporary → permanent on encounter; writes `character_instance_lock` row; updates encounters mask | `CharacterDatabase`, `UpdateSharedInstanceLock` |
| `InstanceLockMgr::UpdateSharedInstanceLock(trans, event)` | Writes `instance` row for ID-based locks; mutates `SharedInstanceLockData` | `CharacterDatabase` |
| `InstanceLockMgr::UpdateInstanceLockExtensionForPlayer(...)` | Toggles `extended` flag; returns `(oldExpiry, newExpiry)` (next-week boundary) | `GetNextResetTime` |
| `InstanceLockMgr::ResetInstanceLocksForPlayer(...)` | Removes locks matching filter unless `IsInUse()` (active map exists) | iterates `_instanceLocksByPlayer` |
| `InstanceLockMgr::OnSharedInstanceLockDataDelete(instanceId)` | Cleans `instance` row when last shared ref drops | `CharacterDatabase` |
| `InstanceLockMgr::GetNextResetTime(entries)` | Computes next reset `time_point` from `MapDifficulty.ResetInterval` aligned to weekly/daily reset hour from config | system clock |
| `InstanceScript::Create()` | Fresh-state init (no save data) | `SetBossState` |
| `InstanceScript::Load(data)` | rapidjson parse of header + `bosses[]` states + persistent values | `InstanceScriptData::Load` |
| `InstanceScript::GetSaveData()` | rapidjson serialization | `InstanceScriptData::Save` |
| `InstanceScript::SetBossState(id, state)` | State transition + door/minion/spawn-group updates + LFG progress + `InstanceMap::UpdateInstanceLock` | `UpdateDoorState`, `UpdateMinionState`, `UpdateSpawnGroups`, `SendBossKillCredit`, `UpdateLfgEncounterState` |
| `InstanceScript::IsEncounterInProgress()` | Any boss `IN_PROGRESS`? Used by `Map::CannotEnter` to block ports during pulls | linear scan `bosses` |
| `InstanceScript::OnCreatureCreate/Remove`, `OnGameObjectCreate/Remove` | Auto-bind GO/creature to `BossInfo` via entry → `DoorInfo` / `MinionInfo` / `_creatureInfo` / `_gameObjectInfo` | `AddObject`, `AddDoor`, `AddMinion` |
| `InstanceScript::SendEncounterStart/End` | Sends `SMSG_INSTANCE_ENCOUNTER_START / _END` to all players in map | `Map::DoForAllPlayers` |
| `InstanceScript::SendEncounterUnit(type, unit, prio)` | Sends `SMSG_INSTANCE_ENCOUNTER_FRAME` (engage / disengage / update / objective) | `BroadcastWorker` |
| `InstanceScript::DoUpdateWorldState(id, value)` | World-state push to all players in instance | `Map::DoForAllPlayers` |
| `InstanceScript::InitializeCombatResurrections / Use / Reset` | 25-man raid combat-res charge tracker | timer in `Update` |
| `InstanceScript::UpdateLfgEncounterState(bossInfo)` | Sends LFG dungeon-progress update if all encounters of completion mask done | `LFGMgr` |
| `InstanceScript::TriggerGameEvent(...)` | Override of `ZoneScript`; dispatches event to all players in the map | `GameEventMgr` |

---

## 5. Module dependencies

**Depends on:**
- `Maps` — `InstanceMap` is the runtime parent; `InstanceScript::instance` is an `InstanceMap*`. `MapManager::GenerateInstanceId` allocates the persistent ID.
- `DataStores (DB2)` — `MapEntry`, `MapDifficultyEntry`, `DungeonEncounterEntry`, `WorldSafeLocsEntry` (entrance), `LFGDungeonEntry`.
- `Database (CharacterDatabase)` — tables: `instance`, `character_instance_lock`, `gameobject_respawn`, `creature_respawn`, `account_instance_times` (account-wide raid IDs).
- `Groups` — `Group::GetLeaderGUID()` is the lock owner for grouped instances; group reset commands (`Group::ResetInstances`).
- `LFG (LFGMgr)` — `UpdateLfgEncounterState` + dungeon-finished bookkeeping.
- `World/Time` — reset hour from `worldserver.conf`, `GameTime::GetGameTime()`.
- `Phasing` — `InstanceScript::UpdatePhasing()` re-evaluates phase conditions on boss state change.
- `Achievement / Criteria` — `DoUpdateCriteria(CriteriaType, m1, m2, unit)` and `CheckAchievementCriteriaMeet`.
- `ZoneScript` (parent class) — generic per-zone hooks (`OnCreatureCreate`, `ProcessEvent`, `GetData`/`SetData`, `GetGuidData`).
- `rapidjson` — for `InstanceScriptData` load/save.

**Depended on by:**
- `Maps` — `InstanceMap` calls `InstanceScript::Update()` every frame, `IsEncounterInProgress` for block-on-enter, `OnPlayerEnter/Leave`.
- `Player` — `Player::SendRaidInfo()`, `Player::ResetInstances()`, `WorldSession::HandleResetInstancesOpcode` use `InstanceLockMgr::GetInstanceLocksForPlayer`.
- `Group` — group lock binding; raid-leader reset commands.
- All ~120 dungeon/raid script files in `src/server/scripts/{Northrend,Outland,EasternKingdoms,...}` subclass `InstanceScript`.
- `LFGMgr` — checks instance encounter-mask completion to award LFG rewards.
- GM commands `.instance unbind`, `.instance reset`, `.instance stats`.

---

## 6. SQL / DB queries (if any)

### CharacterDatabase tables

| Table | Schema highlights | Purpose |
|---|---|---|
| `instance` | `instanceId PK`, `data TEXT`, `completedEncountersMask INT`, `entranceWorldSafeLocId INT` | Authoritative state for ID-based (raid) locks |
| `character_instance_lock` | `(guid, mapId, lockId) PK`, `instanceId`, `difficulty`, `data`, `completedEncountersMask`, `entranceWorldSafeLocId`, `expiryTime BIGINT`, `extended TINYINT` | Per-character lock view |
| `account_instance_times` | per-account hourly limit on instance enter (3.x: max 5 per hour) | `Player::CheckInstanceCount` |
| `gameobject_respawn` | `(spawnId, mapId, instanceId, respawnTime)` | Per-instance GO respawn (cleared on reset) |
| `creature_respawn` | same shape | Per-instance creature respawn |

### Prepared statements (CharacterDatabase, names from `CharacterDatabase.cpp`)

| Statement | Purpose |
|---|---|
| `CHAR_SEL_INSTANCE` | Load all `instance` rows on startup |
| `CHAR_SEL_CHARACTER_INSTANCE_LOCKS` | Load all `character_instance_lock` rows |
| `CHAR_REP_INSTANCE` | REPLACE INTO `instance` |
| `CHAR_DEL_INSTANCE_BY_INSTANCE` | DELETE FROM `instance` WHERE instanceId = ? |
| `CHAR_REP_CHARACTER_INSTANCE_LOCK` | REPLACE INTO `character_instance_lock` |
| `CHAR_DEL_CHARACTER_INSTANCE_LOCK_BY_GUID` / `_BY_GUID_MAP_DIFFICULTY` | Lock removal (player delete, GM unbind) |
| `CHAR_UPD_CHARACTER_INSTANCE_LOCK_EXTEND` | Toggle `extended` flag |
| `CHAR_INS_GAMEOBJECT_RESPAWN` / `CHAR_DEL_GAMEOBJECT_RESPAWN_INSTANCE` | Per-instance respawn rows |
| `CHAR_INS_CREATURE_RESPAWN` / `CHAR_DEL_CREATURE_RESPAWN_INSTANCE` | same |

### DBC/DB2 stores read

| Store | What it loads | Read by |
|---|---|---|
| `MapStorage` | Map.db2 (`MapEntry`) | `MapDb2Entries` ctor, `IsInstanceIdBound` |
| `MapDifficultyStorage` | MapDifficulty.db2 (`LockID`, `ResetInterval`, `MaxPlayers`) | `GetKey()`, `GetNextResetTime` |
| `DungeonEncounterStorage` | DungeonEncounter.db2 | `LoadDungeonEncounterData`, `GetBossDungeonEncounter` |
| `LFGDungeonsStorage` | LFGDungeons.db2 | `UpdateLfgEncounterState` |
| `WorldSafeLocsStorage` | WorldSafeLocs.db2 | `entranceWorldSafeLocId` resolution |

---

## 7. Wire-protocol packets (if any)

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_RESET_INSTANCES` | client → server | `WorldSession::HandleResetInstancesOpcode` → `Player::ResetInstances` / `Group::ResetInstances` |
| `CMSG_SET_DUNGEON_DIFFICULTY` | client → server | Player ↔ `WorldSession::HandleSetDungeonDifficultyOpcode` (group leader) |
| `CMSG_SET_RAID_DIFFICULTY` | client → server | `HandleSetRaidDifficultyOpcode` |
| `CMSG_INSTANCE_LOCK_RESPONSE` | client → server | Confirm convert-to-permanent on first boss kill |
| `CMSG_REQUEST_RAID_INFO` | client → server | `Player::SendRaidInfo` |
| `CMSG_INSTANCE_INFO` | client → server | (cataclysm+) similar for dungeon list |
| `CMSG_REQUEST_INSTANCE_INFO` | client → server | Build dungeon list (post 4.x) |
| `SMSG_INSTANCE_RESET` | server → client | After successful reset (per-player or per-group) |
| `SMSG_INSTANCE_RESET_FAILED` | server → client | When `IsInUse()` blocks reset |
| `SMSG_RAID_INSTANCE_INFO` | server → client | Periodic raid lock list to client (`Player::SendRaidInfo`) |
| `SMSG_INSTANCE_INFO` | server → client | Dungeon list with extension flags |
| `SMSG_RAID_INSTANCE_MESSAGE` | server → client | Welcome / "instance expires in X" / "you are saved" |
| `SMSG_PENDING_RAID_LOCK` | server → client | Convert-to-permanent confirm prompt on entry |
| `SMSG_INSTANCE_ENCOUNTER_ENGAGE_UNIT` / `_DISENGAGE_UNIT` | server → client | `SendEncounterUnit` |
| `SMSG_INSTANCE_ENCOUNTER_START` / `_END` | server → client | `SendEncounterStart`, `SendEncounterEnd` |
| `SMSG_INSTANCE_ENCOUNTER_CHANGE_PRIORITY` | server → client | nameplate priority |
| `SMSG_INSTANCE_ENCOUNTER_TIMER_START` | server → client | Boss soft-enrage timer |
| `SMSG_INSTANCE_ENCOUNTER_OBJECTIVE_START` / `_UPDATE` / `_COMPLETE` | server → client | Obj progress |
| `SMSG_INSTANCE_ENCOUNTER_IN_COMBAT_RESURRECTION` / `_GAIN_COMBAT_RESURRECTION_CHARGE` | server → client | Combat-res tracking |
| `SMSG_BOSS_KILL_CREDIT` (alias `SMSG_INSTANCE_ENCOUNTER_GAIN_KILL_CREDIT` in some versions) | server → client | `SendBossKillCredit(encounterId)` |
| `SMSG_UPDATE_INSTANCE_OWNERSHIP` | server → client | Notify on instance owner change |
| `SMSG_TRANSFER_ABORTED` | server → client | When `CanJoinInstanceLock` returns non-NONE |

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-instances` | `crate_dir` | 0 | 0 | `missing_declared_path` | declared/proposed target does not exist |
| `crates/wow-world/src/map_manager.rs` | `file` | 1 | 784 | `exists_active` | file exists |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-instances/src/lib.rs` — foundation started in `#NEXT.R8.ENTITIES.303/#NEXT.R8.ENTITIES.304`: the crate exists and ports the pure C++ encounter metadata path for `MAX_DUNGEON_ENCOUNTERS_PER_BOSS`, `EncounterState`, `DungeonEncounterData`, `BossInfo::GetDungeonEncounterForDifficulty`, `InstanceScript::LoadDungeonEncounterData`, `InstanceScript::GetBossDungeonEncounter(uint32)` and the `BossAI::GetBossId()` branch behind `InstanceScript::GetBossDungeonEncounter(Creature const*)`. `#NEXT.R8.ENTITIES.305` preserves optional represented `BossAI::_bossId` on `wow-ai::CreatureAI`, but real script instantiation and runtime instance use remain pending. Full per-instance state, map integration, lock manager, boss transitions, doors/minions, persistence, criteria and LFG delivery remain pending.

**What's implemented:**
- `crates/wow-world/src/map_manager.rs` (per the active WIP commits) provides a `MapManager` global stub with placeholder for `GenerateInstanceId` (must verify), but no lock store and no script dispatch. (See WIP commit `f83c48d82`.)
- World/character DB pool layer can run queries; no statements registered for `instance` / `character_instance_lock`.

**What's missing vs C++:**
- Everything below `InstanceLockMgr` — lock load/save, expiration, extension, reset.
- Everything below `InstanceScript` — boss-state machine, door/minion linking, encounter packets, persistent values, JSON save blob.
- Instance ID allocation policy + free-id reuse, ID masks (`0x1F440000`, `0x00000001`, `0x00010000`).
- Reset cron (per-`MapDifficulty.ResetInterval` + daily/weekly anchor).
- All wire packets (Section 7).
- Group-leader-as-lock-owner rule.
- Account-wide instance-enter rate limit (`account_instance_times`).
- Per-instance respawn purge on reset (`gameobject_respawn` / `creature_respawn` rows tagged with `instanceId`).
- LFG progress hook.
- Combat-res tracker.
- All ~120 dungeon/raid scripts (each a separate Trait/struct in Rust).

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- `MapManager::GenerateInstanceId` in WIP commit may not honor the `INSTANCE_ID_HIGH_MASK | _NORMAL_MASK` bit pattern — verify before persisting any IDs (DB-level break if wrong).
- No code path in current Rust calls anything resembling `CanJoinInstanceLock` — players can be ported into instances without lock validation; risk of phantom lock binding.

**Tests existing:**
- 0 tests in any wow-instances or related crate.

---

## 9. Migration sub-tasks

<!-- REFINE.022:BEGIN task-wbs -->

### R2 Task WBS (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md` + targets verificados en `docs/migration/inventory/r2-rust-targets.tsv`. C++ sigue siendo el oraculo; estas tareas son el suelo de cobertura por archivo, no una prueba de port correcto.

- [ ] **#INSTANCES.WBS.001** Partir y cerrar la migracion auditada de `game/Instances/InstanceLockMgr.cpp`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `needs_split`; C++ file has 599 lines; split by public API, state model, persistence, runtime behavior and tests before implementation. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.002** Cerrar la migracion auditada de `game/Instances/InstanceLockMgr.h`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.h`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.003** Partir y cerrar la migracion auditada de `game/Instances/InstanceScript.cpp`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `needs_split`; C++ file has 971 lines; split by public API, state model, persistence, runtime behavior and tests before implementation. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.004** Cerrar la migracion auditada de `game/Instances/InstanceScript.h`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.005** Cerrar la migracion auditada de `game/Instances/InstanceScriptData.cpp`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScriptData.cpp`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.006** Cerrar la migracion auditada de `game/Instances/InstanceScriptData.h`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScriptData.h`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.
- [ ] **#INSTANCES.WBS.007** Cerrar la migracion auditada de `game/Instances/enuminfo_InstanceScript.cpp`
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/enuminfo_InstanceScript.cpp`
  Rust target: `crates/wow-instances`
  Depends on: #REFINE.020, #REFINE.021; execution order finalized by #REFINE.040
  Acceptance: Rust target compiles; behavior and public contracts are checked against the listed C++ file; unit/golden/integration tests are added or marked n/a with reason; divergences are recorded before closing.
  Notes: `ready_for_small_task`; Single source-file coverage task; split further if C++ review exposes multiple independent behaviors. Assignment basis: prefix.

<!-- REFINE.022:END task-wbs -->

Numera los items para poder referenciarlos desde `MIGRATION_ROADMAP.md` sección 5.

Complejidad: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, splitear).

- [ ] **#INST.1** Create `crates/wow-instances/` crate with `Cargo.toml` (deps on `wow-core`, `wow-constants`, `wow-database`, `wow-data-stores`, `serde_json`); add to workspace (L)
- [ ] **#INST.2** Port `InstanceLockData`, `InstanceLock`, `SharedInstanceLockData`, `SharedInstanceLock` structs (L)
- [ ] **#INST.3** Port `MapDb2Entries` w/ `GetKey()`, `IsInstanceIdBound()` resolving DB2 entries via `wow-data-stores` (M)
- [ ] **#INST.4** Define `InstanceLockKey = (u32 mapId, u32 lockId)` and `EncounterState`, `EncounterDoorBehavior`, `EncounterFrameType` enums in `wow-constants` (L)
- [ ] **#INST.5** Implement `InstanceLockMgr` skeleton: `DashMap<ObjectGuid, HashMap<InstanceLockKey, Arc<InstanceLock>>>` for both `_temporary` and `_main`; weak-map for shared data (M)
- [ ] **#INST.6** Register prepared statements for `instance`, `character_instance_lock` in `wow-database` (SEL/REP/DEL) (M)
- [ ] **#INST.7** Implement `InstanceLockMgr::load()` — async load all rows + reconstruct shared map (H)
- [ ] **#INST.8** Implement `CanJoinInstanceLock` returning `TransferAbortReason` enum mirror (M)
- [ ] **#INST.9** Implement `FindActiveInstanceLock` (player or group-leader guid) honoring extended-but-expired (M)
- [ ] **#INST.10** Implement `CreateInstanceLockForNewInstance` → temporary map (L)
- [ ] **#INST.11** Implement `UpdateInstanceLockForPlayer` (promote temp→perm, REPLACE row, transactional) (H)
- [ ] **#INST.12** Implement `UpdateSharedInstanceLock` for raid-id locks (M)
- [ ] **#INST.13** Implement `UpdateInstanceLockExtensionForPlayer` (toggle + recompute expiry) (M)
- [ ] **#INST.14** Implement `ResetInstanceLocksForPlayer` w/ in-use guard (M)
- [ ] **#INST.15** Implement `OnSharedInstanceLockDataDelete` cleanup (L)
- [ ] **#INST.16** Implement `GetNextResetTime` from `MapDifficulty.ResetInterval` + reset-hour config (M)
- [ ] **#INST.17** Implement instance-id allocator with `INSTANCE_ID_HIGH_MASK`/`_LFG_MASK`/`_NORMAL_MASK` bits + free-id reuse (M)
- [ ] **#INST.18** Implement `account_instance_times` rate-limit (5 per hour) check (M)
- [ ] **#INST.19** Port `BossInfo`, `DoorData`, `DoorInfo`, `MinionData`, `MinionInfo`, `ObjectData`, `DungeonEncounterData`, `BossBoundaryEntry` (M)
- [ ] **#INST.20** Define `InstanceScript` trait + default impl struct (`bosses`, `doors`, `minions`, `_creature_info`, `_go_info`, `_object_guids`, `_persistent_values`, `_entrance_id`, `_combat_res_*`) (H)
- [ ] **#INST.21** Implement `Create`, `Load(json)`, `GetSaveData()` w/ `serde_json` mirroring `InstanceScriptData.cpp` (header + bosses[] + persistent) (H)
- [ ] **#INST.22** Implement `SetBossState(id, state)` w/ door/minion/spawn-group/LFG hooks + save-trigger + `InstanceMap::UpdateInstanceLock` (H)
- [ ] **#INST.23** Implement `OnCreatureCreate/Remove` and `OnGameObjectCreate/Remove` w/ `_creature_info` + `_go_info` lookups (M)
- [ ] **#INST.24** Implement `IsEncounterInProgress`, `IsEncounterCompleted`, `IsEncounterCompletedInMaskByBossId`, `GetEncounterCount` (L)
- [ ] **#INST.25** Implement `UpdateDoorState`, `UpdateMinionState`, `HandleGameObject`, `DoUseDoorOrButton`, `DoCloseDoorOrButton`, `DoRespawnGameObject` (M)
- [ ] **#INST.26** Implement encounter-frame packet senders: `SendEncounterUnit`, `SendEncounterStart`, `SendEncounterEnd` (M)
- [ ] **#INST.27** Implement `SendBossKillCredit` (`SMSG_BOSS_KILL_CREDIT`) (L)
- [ ] **#INST.28** Implement `DoUpdateWorldState`, `DoCastSpellOnPlayers`, `DoRemoveAurasDueToSpellOnPlayers`, `DoUpdateCriteria`, `DoSendNotifyToInstance` (M)
- [ ] **#INST.29** Implement combat-resurrection tracker (`InitializeCombatResurrections`, `Use`, `Reset`, `GetCombatResurrectionChargeInterval`, `Update`) (M)
- [ ] **#INST.30** Implement entrance-location resolver (`SetEntranceLocation`, `Get`, `ComputeEntranceLocationForCompletedEncounters`) (M)
- [ ] **#INST.31** Implement `PersistentInstanceScriptValue<T>` (i64 / f64 variant) w/ change-notify (M)
- [ ] **#INST.32** Implement `MarkAreaTriggerDone` / `IsAreaTriggerDone` set tracking (L)
- [ ] **#INST.33** Implement `UpdateLfgEncounterState` integration (depends on `wow-lfg` doc) (M)
- [ ] **#INST.34** Implement `UpdatePhasing` integration (depends on `phasing.md`) (M)
- [ ] **#INST.35** Implement `Player::SendRaidInfo` → `SMSG_RAID_INSTANCE_INFO` builder (M)
- [ ] **#INST.36** Implement `WorldSession::HandleResetInstancesOpcode` (`CMSG_RESET_INSTANCES`) → call `ResetInstanceLocksForPlayer` or `Group::ResetInstances` (M)
- [ ] **#INST.37** Implement `SMSG_INSTANCE_RESET` / `_FAILED` / `SMSG_RAID_INSTANCE_MESSAGE` packet senders (M)
- [ ] **#INST.38** Implement `SMSG_PENDING_RAID_LOCK` + `CMSG_INSTANCE_LOCK_RESPONSE` round-trip (M)
- [ ] **#INST.39** Implement instance respawn purge on reset (`gameobject_respawn` / `creature_respawn` rows by `instanceId`) (M)
- [ ] **#INST.40** Wire `InstanceMap::Update(diff)` → `InstanceScript::Update` + combat-res tick (L)
- [ ] **#INST.41** Audit & fix `MapManager::GenerateInstanceId` in WIP `map_manager.rs` to honor the bit-mask scheme (L)
- [ ] **#INST.42** Add GM commands `.instance unbind`, `.instance reset`, `.instance stats` (M)
- [ ] **#INST.43** Plumb `CanJoinInstanceLock` into the teleport / portal / `MapManager::PlayerCannotEnter` path (H)

---

## 10. Regression tests to write

<!-- REFINE.024:BEGIN tests-required -->

### R2 Tests required (generated)

> Fuente: cobertura C++ asignada y targets Rust verificados. Estos gates son obligatorios para cerrar tareas WBS; `n/a` solo vale con razon explicita y referencia C++/producto.

| Gate | Required coverage | Acceptance |
|---|---|---|
| `#INSTANCES.TEST.001 / unit` | Unit tests for pure data structures, parsers, state transitions and edge cases directly ported from C++ invariants. C++ scope: 7 files / 2783 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h`. Rust target: `workspace / target pending`. | `cargo test --workspace` passes for the touched target(s); every migrated behavior has focused tests or an explicit documented n/a. |
| `#INSTANCES.TEST.002 / golden` | Golden/fixture tests derived from C++ packet bytes, SQL rows, config defaults, DB2 records or deterministic algorithm outputs. C++ scope: 7 files / 2783 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h`. Rust target: `workspace / target pending`. | Golden fixtures are checked in or generated by a documented harness; Rust output matches C++ semantics byte-for-byte where wire/data format is involved. |
| `#INSTANCES.TEST.003 / integration` | Integration tests for startup/load paths, database access, registry wiring and cross-crate behavior. C++ scope: 7 files / 2783 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h`. Rust target: `workspace / target pending`. | The module can be loaded/exercised through its real Rust service boundary without panics, missing handlers or silent default-success paths. |
| `#INSTANCES.TEST.004 / e2e` | Client/bot or scripted runtime scenario when the module affects login, world session, packets, entities, maps, gameplay or content. C++ scope: 7 files / 2783 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h`. Rust target: `workspace / target pending`. | Bot/client scenario covers the user-visible path, or the doc marks E2E n/a with a concrete product-scope reason before implementation closes. |

<!-- REFINE.024:END tests-required -->

- [ ] Test: lock-id-based vs instance-id-based binding produce different `InstanceLockKey` and different DB persistence shape.
- [ ] Test: `GetNextResetTime` returns stable answer for daily / 3-day / weekly `ResetInterval` regardless of caller wall-clock seconds (anchor on reset-hour).
- [ ] Test: temporary lock (no boss killed) is purged on player logout / map destroy without writing `character_instance_lock`.
- [ ] Test: first boss kill promotes temp → perm and writes both `instance` row (for ID-based) and `character_instance_lock` row.
- [ ] Test: `CanJoinInstanceLock` returns `TRANSFER_ABORT_LOCKED_TO_DIFFERENT_INSTANCE` when player has lock to instanceId X but tries to enter Y of same `(mapId, lockId)`.
- [ ] Test: `extended=true` lets player join a lock that is past `_expiryTime` but not past `effectiveExpiryTime` (= next reset).
- [ ] Test: `ResetInstanceLocksForPlayer` skips locks where `IsInUse()` (active map) and reports them in `locksFailedToReset`.
- [ ] Test: `InstanceScript::SetBossState(id, DONE)` flips door states per `EncounterDoorBehavior` and updates `completedEncountersMask`.
- [ ] Test: `Load(GetSaveData())` round-trips boss states + persistent values losslessly (JSON stable).
- [ ] Test: `IsEncounterInProgress()` returns true iff any boss in `IN_PROGRESS`.
- [ ] Test: `account_instance_times` blocks the 6th distinct enter within 1 hour.
- [ ] Test: shared-state instance lock — two players in same group see the same `completedEncountersMask` even if one lock row is older.
- [ ] Test: `gameobject_respawn` rows for `instanceId=N` are deleted when the instance is reset.

---

## 11. Notes / gotchas

<!-- REFINE.025:BEGIN product-scope -->

### R2 Product scope / exclusions (generated)

> Fuente: cabecera del doc + inventario C++ asignado. Ninguna marca de alcance elimina C++ del backlog: solo define si se implementa, se sustituye por idiom Rust o se desactiva explicitamente para producto.

| Scope | Decision | C++ retained | Evidence |
|---|---|---|---|
| `active_port_scope` | Full C++ surface remains in migration scope; no product exclusion recorded. | 7 files / 2783 lines; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h` | `crates/wow-instances/` exists with the #NEXT.R8.ENTITIES.303 encounter-metadata foundation; full module still mostly pending |

<!-- REFINE.025:END product-scope -->

<!-- REFINE.023:BEGIN known-divergences -->

### R2 Known divergences / bugs (generated)

> Fuente: C++ asignado en `cpp-files-by-module.md` + target Rust verificado en `r2-rust-targets.tsv`. Esto enumera divergencias estructurales conocidas; no sustituye la auditoria funcional contra C++ antes de cerrar tareas.

| ID | Rust evidence | C++ evidence | Status | Notes |
|---|---|---|---|---|
| `#INSTANCES.DIV.001` | `crates/wow-instances` | 7 C++ files / 2783 lines assigned; refs: `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceLockMgr.cpp`, `/home/server/woltk-trinity-legacy/src/server/game/Instances/InstanceScript.h` | `foundation_started` | Declared Rust target exists as of #NEXT.R8.ENTITIES.303; broad C++ surface remains to port. |

<!-- REFINE.023:END known-divergences -->

- **Instance ID is *not* the same as `mapId`.** `mapId` identifies the dungeon (e.g., 533 = Naxxramas); `instanceId` is a server-allocated global counter that goes into the instance map key alongside `mapId`. The bits encode kind: `INSTANCE_ID_HIGH_MASK = 0x1F440000`, `_LFG_MASK = 0x00000001`, `_NORMAL_MASK = 0x00010000`. Get this wrong and DB rows from prior runs will collide.
- **Lock owner = group leader if grouped, else self.** Crucial — when a player leaves the group, their lock record persists referencing the *old* group-leader's guid as owner, but their own `character_instance_lock.guid` is themselves. The two-table split (`instance` shared + `character_instance_lock` per-player) exists exactly to handle this.
- **Two lock states.** A temporary lock exists from instance-creation until first boss kill; it is *not* persisted. Don't write to DB on every player enter — only on first encounter completion.
- **The `data` blob is rapidjson, not binary.** `InstanceScript::GetSaveData()` produces JSON with shape `{ "Header": "...", "BossStates": [...], "AdditionalData": { ... } }`. Header is set by `SetHeaders()` and used as a versioning string — old script versions write a different header and are rejected on load.
- **`MAX_DUNGEON_ENCOUNTERS_PER_BOSS = 4`** because some bosses have separate `DungeonEncounter` entries per difficulty (10N, 25N, 10H, 25H).
- **Reset cron is in `MapManager::Update`, not `InstanceLockMgr`.** The lock manager only knows *next* reset time per (mapId, lockId); the actual purge of locks + respawn rows + active-instance tear-down is driven by `MapManager`.
- **`character_instance_lock.expiryTime` is unix timestamp (uint64), not a relative offset.** All lock comparisons use absolute wall clock.
- **`account_instance_times`** — Blizzard's anti-grief: a single account can enter at most 5 distinct instance IDs per 1-hour rolling window. This is enforced in `Player::CheckInstanceCount` and queries `account_instance_times`. Easy to forget — leads to "Too many instances" client errors at edge cases.
- **`ASSERT(map->IsDungeon())`** in `InstanceScript` ctor — only instance maps. Battlegrounds and outdoor maps have their own subclasses.
- **Re-entry into in-progress instance.** `IsEncounterInProgress() == true` blocks new players from entering — sends `TRANSFER_ABORT_ZONE_IN_COMBAT`. `OnPlayerEnter` only fires for already-permitted entries.
- **`SendEncounterStart`** must be called *before* the first boss state goes `IN_PROGRESS`, otherwise the client UI does not draw the boss frame. Order matters.
- **Combat-res charges** are 1 by default in 25-man, recharge `interval_ms` on a timer; `_combatResurrectionTimerStarted` only flips on first encounter engage.
- **rapidjson dependency.** In Rust, prefer `serde_json` and a stable schema struct — *do not* duplicate the C++ pointer-walking style.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class InstanceLockMgr` (singleton) | `pub struct InstanceLockMgr` + `OnceLock<InstanceLockMgr>` global | `sInstanceLockMgr` macro → `wow_instances::lock_mgr()` |
| `std::shared_mutex _locksMutex` | `tokio::sync::RwLock<>` or `parking_lot::RwLock` | Reads dominate; writes only on boss kills/resets |
| `std::unordered_map<ObjectGuid, PlayerLockMap>` | `DashMap<ObjectGuid, HashMap<InstanceLockKey, Arc<InstanceLock>>>` | DashMap because access is sharded by playerGuid |
| `std::weak_ptr<SharedInstanceLockData>` | `Weak<SharedInstanceLockData>` | Shared raid lock state ref |
| `std::unique_ptr<InstanceLock>` | `Arc<InstanceLock>` (or `Box` if single-owner) | Multiple readers possible from group members |
| `std::chrono::system_clock::time_point` | `std::time::SystemTime` or `chrono::DateTime<Utc>` | Pick one project-wide |
| `Optional<uint32>` | `Option<u32>` | trivial |
| `std::variant<int64, double>` | `enum PersistentValue { Int(i64), Float(f64) }` | Same shape |
| `class InstanceScript : public ZoneScript` | `pub trait InstanceScript: ZoneScript` + `pub struct InstanceScriptBase` providing default fields | No virtual inheritance; trait + composition |
| `BossInfo::door[Behavior::Max]` (array of GuidSet) | `[HashSet<ObjectGuid>; EncounterDoorBehavior::COUNT]` | Fixed-size array |
| `EncounterState` enum | `#[repr(u8)] enum EncounterState { NotStarted=0, InProgress=1, Fail=2, Done=3, Special=4, ToBeDecided=5 }` | u8 wire-compatible |
| `rapidjson` save/load | `serde_json::Value` + `#[derive(Serialize, Deserialize)]` schema struct | Strict schema with `header` field for versioning |
| `PersistentInstanceScriptValue<T>` | `pub struct PersistentValue<T>` w/ `Drop`-on-script-detach + change-notify channel | Use observer pattern, not raw pointer registry |
| `void Update(uint32 diff)` | `fn update(&mut self, diff: Duration)` | All `uint32` ms → `Duration` |
| `WorldPacket` builders | `wow-shared-packets` packet structs | Already exists in repo |
| Global `INSTANCE_ID_HIGH_MASK` etc. | `pub const INSTANCE_ID_HIGH_MASK: u32 = 0x1F44_0000;` in `wow-constants` | trivial port |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.

---

## 13. Audit (2026-05-01)

❌ confirmado. Auditado contra `/home/server/rustycore/crates/`.

**Hallazgos clave:**
- No existe `crates/wow-instances/`. Búsqueda de `INSTANCE_ID_HIGH_MASK | _LFG_MASK | _NORMAL_MASK | 0x1F44 | 0x1f44` en todo el workspace: **0 resultados**. La hipótesis de la sección 8 queda confirmada — *no hay generador de instance IDs todavía*, ni con el bitfield correcto ni con uno secuencial: simplemente el `instance_id: u32` se pasa como parámetro por callers en `crates/wow-world/src/map_manager.rs:466` (`get_or_create_map(map_id, instance_id)`).
- Los 2 únicos call-sites de `add_creature(0,0,0,0,…)` están en tests del propio `map_manager.rs` (líneas 761, 774). Sin caller real, la pregunta "qué bits usan los IDs" todavía no se ha decidido.
- `InstanceLockMgr` análogo: 0 código. Tablas `instance`, `character_instance_lock`, `account_instance_times` no aparecen en `crates/wow-database/src/`. Cero prepared statements.
- 0 handlers para `CMSG_RESET_INSTANCES`, `CMSG_INSTANCE_LOCK_RESPONSE`, `CMSG_REQUEST_RAID_INFO`. 0 builders para `SMSG_RAID_INSTANCE_INFO`, `SMSG_PENDING_RAID_LOCK`, `SMSG_INSTANCE_ENCOUNTER_*`, `SMSG_INSTANCE_RESET*`.

**Riesgo de UI hang silencioso:**
- ⚠️ **`CMSG_REQUEST_RAID_INFO` no está registrado** en `wow-handler` ni stubbed en `misc.rs`. El cliente al hacer `Shift-O → Raid` espera `SMSG_RAID_INSTANCE_INFO`; sin handler ni stub, el packet se descarta en el dispatcher (registro inventario) y la pestaña queda *forever-loading*. Si y cuando se cree wow-instances, este es el primer opcode a registrar (incluso si solo devuelve 0 locks).
- Bajo riesgo de UI hang en el resto del flujo: el cliente nunca llega a "set difficulty" o "reset instance" porque el botón "Enter Dungeon" requiere primero un raid info populado.

**Acción:** dejar `❌ not started` en el badge. El módulo puede dormir hasta después de Maps + WorldStateMgr + DataStores estén verdes. Cuando arranque, **prioridad #INST.41** (auditar `GenerateInstanceId` antes de persistir IDs) sigue válida porque ahora *no existe* y hay que crearla con los bits correctos desde el inicio — no parchearla después.
