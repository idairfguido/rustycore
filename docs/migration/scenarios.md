# Migration: Scenarios

> **C++ canonical path:** `src/server/game/Scenarios/`
> **Rust target crate(s):** *N/A para WoLK 3.4.x* — placeholder en `crates/wow-world/src/scenarios/` (skeleton only)
> **Layer:** L7
> **Status:** ❌ not started — **N/A para 3.4.x classic**
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Scenarios son *3-player adventures* introducidos en **Mists of Pandaria (5.0, 2012)**. Son instancias mini-historia para compositions arbitrarios (sin requerir tank/healer/dps), con sistema propio de criteria-tree y POI. **No aplican a Wrath of the Lich King 3.4.x** (Blizzard no los retroportó al cliente clásico). El módulo existe en este fork como código *upstream-merged* pero **muerto**: ninguna `lfg_dungeon_template` con `type==SCENARIO`, ninguna entrada en `LFGDungeons.dbc` 3.4.x, ningún cliente WoLK envía `CMSG_SCENARIO_*`.

> **Conclusión:** documentar como esqueleto stub-only. NO se debe implementar para 3.4.3. Se conserva la doc para que la próxima sesión no investigue desde cero.

---

## 2. C++ canonical files

Todas las rutas relativas a `/home/server/woltk-trinity-legacy/`.

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Scenarios/Scenario.h` | 111 | `Scenario : CriteriaHandler`, `ScenarioStepState` enum |
| `src/server/game/Scenarios/Scenario.cpp` | 362 | Implementación: step transitions, criteria updates, packet builders |
| `src/server/game/Scenarios/InstanceScenario.h` | 37 | `InstanceScenario : Scenario` (variante para `InstanceMap`) |
| `src/server/game/Scenarios/InstanceScenario.cpp` | 112 | DB persistence de criteria progress |
| `src/server/game/Scenarios/ScenarioMgr.h` | 127 | Singleton, `ScenarioData`, `ScenarioDBData`, `ScenarioPOI`, enums |
| `src/server/game/Scenarios/ScenarioMgr.cpp` | 224 | `LoadDB2Data`, `LoadDBData`, `LoadScenarioPOI`, `CreateInstanceScenario` |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Scenario` | class (extends `CriteriaHandler`) | Base scenario instance, tracks current step + step states |
| `InstanceScenario` | class (extends `Scenario`) | Scenario asociado a un `InstanceMap` (con DB save) |
| `ScenarioMgr` | singleton | Registry de `ScenarioData`, `ScenarioPOI`, mapping (mapId, difficulty) → scenario A/H |
| `ScenarioStepState` | enum | INVALID=0, NOT_STARTED=1, IN_PROGRESS=2, DONE=3 |
| `ScenarioType` | enum | SCENARIO=0, CHALLENGE_MODE=1, SOLO=2, DUNGEON=10 |
| `ScenarioData` | struct | `Entry` (DB2 row) + `Steps: map<uint8, ScenarioStepEntry*>` |
| `ScenarioDBData` | struct | `MapID`, `DifficultyID`, `Scenario_A`, `Scenario_H` (per-faction routing) |
| `ScenarioPOI` | struct | `BlobIndex`, `MapID`, `UiMapID`, `Priority`, `Flags`, `WorldEffectID`, `PlayerConditionID`, `NavigationPlayerConditionID`, `Points: vec<ScenarioPOIPoint>` |
| `ScenarioPOIPoint` | struct | `X`, `Y`, `Z` (int32) |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `ScenarioMgr::Instance()` | Singleton accessor | — |
| `ScenarioMgr::CreateInstanceScenario(InstanceMap*, TeamId)` | Lookup `_scenarioDBData[(mapId, diff)]`, devuelve scenario A o H según team | `_scenarioData` |
| `ScenarioMgr::LoadDBData()` | Lee tabla `scenarios` (mapId, difficulty, scenario_A, scenario_H) | `WorldDatabase` |
| `ScenarioMgr::LoadDB2Data()` | Carga `Scenario.db2` + `ScenarioStep.db2` | `sScenarioStore`, `sScenarioStepStore` |
| `ScenarioMgr::LoadScenarioPOI()` | Carga `scenario_poi` + `scenario_poi_points` | `WorldDatabase` |
| `ScenarioMgr::GetScenarioPOIs(criteriaTreeID)` | Lookup POIs por criteria-tree | — |
| `Scenario::Reset()` | Resetea step a `GetFirstStep`, limpia step states | `SetStep`, `Reset` (CriteriaHandler) |
| `Scenario::SetStep(ScenarioStepEntry const*)` | Activa step, marca anteriores como DONE, broadcast `SMSG_SCENARIO_STATE` | `SendScenarioState` |
| `Scenario::CompleteStep(step)` | Step → DONE, advance to next o `CompleteScenario` | `SetStep`, `CompleteScenario` |
| `Scenario::CompleteScenario()` | Final step DONE → broadcast completion, hooks reward | — |
| `Scenario::OnPlayerEnter(Player*)` | Insert en `_players`, send state | `SendScenarioState` |
| `Scenario::OnPlayerExit(Player*)` | Remove de `_players` | — |
| `Scenario::IsComplete()` | Todos los steps en DONE | — |
| `Scenario::GetStepState(step)` | Lookup `_stepStates[step]` | — |
| `Scenario::SendScenarioState(Player*)` | `SMSG_SCENARIO_STATE` con criteria progress + bonus objectives | `BuildScenarioStateFor` |
| `Scenario::SendBootPlayer(Player*)` | `SMSG_SCENARIO_BOOT_PLAYER` | — |

---

## 5. Module dependencies

**Depends on:**
- `CriteriaHandler` (achievements/criteria infra)
- `Map` / `InstanceMap`
- DB2: `sScenarioStore`, `sScenarioStepStore`, `sCriteriaStore`
- WorldDatabase: `scenarios`, `scenario_poi`, `scenario_poi_points`
- `WorldPackets::Scenario::*` (ScenarioState, BootPlayer, POIs)
- `Player` — para criteria progress per-player

**Depended on by:**
- `InstanceMap::SetInstanceScenario` — instance binding
- `Player::SendInitialPacketsBeforeAddToMap` — manda scenario state al entrar
- `LFGMgr` (en clientes MoP+) — `LFG_TYPE_RANDOM` con scenario flag

---

## 6. SQL / DB queries

| Statement / Source | Purpose | DB |
|---|---|---|
| `SELECT mapId, difficulty, scenario_A, scenario_H FROM scenarios` | Routing per faction | world |
| `SELECT * FROM scenario_poi` | POI metadata | world |
| `SELECT * FROM scenario_poi_points` | POI vertices | world |

**DB2 stores leídos:**

| Store | What it loads | Read by |
|---|---|---|
| `sScenarioStore` | Scenario.db2 | `ScenarioMgr::LoadDB2Data` |
| `sScenarioStepStore` | ScenarioStep.db2 | `ScenarioMgr::LoadDB2Data` |

> **En 3.4.x estos DB2 NO existen en el cliente clásico**, las tablas SQL no se llenan, y el loader sale temprano con count=0.

---

## 7. Wire-protocol packets

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `SMSG_SCENARIO_STATE` | server → client | `Scenario::SendScenarioState` (MoP+ only) |
| `SMSG_SCENARIO_PROGRESS_UPDATE` | server → client | `SendCriteriaUpdate` |
| `SMSG_SCENARIO_COMPLETED` | server → client | `CompleteScenario` |
| `SMSG_SCENARIO_BOOT_PLAYER` | server → client | `Scenario::SendBootPlayer` |
| `SMSG_SCENARIO_POIS` | server → client | Response a request POI |
| `CMSG_QUERY_SCENARIO_POI` | client → server | `HandleQueryScenarioPOI` |

> **Cliente WoLK 3.4.3 NO envía `CMSG_QUERY_SCENARIO_POI` ni decodifica los SMSG_SCENARIO_*.** Sus opcode IDs probablemente conflictúan con otros opcodes WoLK.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- *NINGUNO*. No existe ni stub ni placeholder.

**What's implemented:**
- Nada.

**What's missing vs C++:**
- Todo. **Pero no se debe implementar para 3.4.x.** El esfuerzo de portar este módulo (~970 líneas C++ + DB2 stores + criteria infra) no genera beneficio funcional para un cliente WoLK clásico.

**Suspicious / likely divergent:**
- N/A — el cliente nunca dispara este código.

**Tests existing:**
- 0 tests. No se requieren.

---

## 9. Migration sub-tasks (ESQUELETO ONLY)

Complejidad: **L** (<1h), **M** (1-4h), **H** (4-12h).

> **Política recomendada:** ejecutar #SCN.1–#SCN.3 sólo cuando el equipo decida soportar un cliente MoP+ (5.x). Para 3.4.x, marcar el módulo como **WONT-IMPLEMENT** y dejar el directorio inexistente.

- [ ] **#SCN.1** Crear placeholder `crates/wow-world/src/scenarios/mod.rs` con un único item `pub const SCENARIOS_NOT_SUPPORTED_IN_3_4_3: bool = true;` y un comentario explicando por qué (L)
- [ ] **#SCN.2** Documentar en `crates/wow-world/src/lfg/mod.rs` que `LfgQueueType::Scenario=3` está **disabled** (L)
- [ ] **#SCN.3** Asegurar que los handlers de opcodes `CMSG_QUERY_SCENARIO_POI` (si llegan a llegar) responden con paquete vacío sin panic (L)
- [ ] **#SCN.4** *(futuro, sólo si se soporta cliente MoP+)* Definir `Scenario`, `InstanceScenario`, `ScenarioMgr`, todos los enums (`ScenarioStepState`, `ScenarioType`) y structs (M)
- [ ] **#SCN.5** *(futuro)* Loader `load_db_data()` desde tabla `scenarios` (M)
- [ ] **#SCN.6** *(futuro)* Loader `load_db2_data()` desde `Scenario.db2` + `ScenarioStep.db2` — **requiere cliente MoP+** (M)
- [ ] **#SCN.7** *(futuro)* Loader `load_scenario_poi()` desde `scenario_poi` + `scenario_poi_points` (M)
- [ ] **#SCN.8** *(futuro)* Implementar `Scenario::set_step`, `complete_step`, `complete_scenario` con state-machine (H)
- [ ] **#SCN.9** *(futuro)* Integración con `CriteriaHandler` para tracking de progreso por jugador (H)
- [ ] **#SCN.10** *(futuro)* Builders SMSG_SCENARIO_* y handler `handle_query_scenario_poi` (M)

---

## 10. Regression tests to write

- [ ] Test: en startup con cliente 3.4.x, `ScenarioMgr::LoadDB2Data()` no loaded (count=0) sin error de log spam
- [ ] Test: `CMSG_QUERY_SCENARIO_POI` (si el cliente alguna vez lo envía) responde con vacío sin panic
- [ ] Test: `LfgQueueType::Scenario` no está expuesto en `system_info` enviado al cliente
- [ ] *(futuro)* Test: round-trip `Scenario.db2` parsing
- [ ] *(futuro)* Test: state machine NOT_STARTED → IN_PROGRESS → DONE → next step

---

## 11. Notes / gotchas

- **Decisión arquitectónica:** Scenarios son **post-WoLK**. El cliente 3.4.3 no tiene UI para ellos, no envía opcodes, no parsea SMSG. Implementarlos sería trabajo muerto.
- TrinityCore master rama mainline tiene Scenarios completos para Shadowlands+. Este fork legacy heredó el código pero NUNCA lo activó (no hay registros en tablas SQL ni DBC del cliente).
- `enum LfgQueueType { ..., LFG_QUEUE_SCENARIO=3 }` existe en `LFG.h` pero ningún path de matchmaking lo usa para 3.4.x.
- Si en el futuro alguien quiere portar el server a un cliente MoP+ (5.4.8 por ejemplo), este módulo **será obligatorio** y la implementación es no-trivial (~3 días-persona): requiere `CriteriaHandler` portado, DB2 loaders, packet builders, instance binding, y POI rendering.
- `InstanceScenario` interactúa con `InstanceMap::SetInstanceScenario` — si en el futuro se implementa, hay que coordinar con `instance.md`.
- POI system es independiente del scenario en sí: los POIs son metadata para minimap/world map; en 3.4.x el equivalente es `quest_poi` (que **sí** se usa).
- **NO confundir Scenarios con LFR o con Challenge Mode dungeons** — son sistemas separados:
  - `LFG_TYPE_RANDOM` + `flag SEASONAL`: WoLK Random Heroic Daily ✓ activo
  - `LFG_QUEUE_LFR=2`: 25-man Looking For Raid ✗ MoP+
  - `LFG_QUEUE_SCENARIO=3`: 3-man Scenarios ✗ MoP+
  - `SCENARIO_TYPE_CHALLENGE_MODE=1`: Challenge Mode dungeons ✗ MoP+
- Recomendación: este doc se mantiene como **referencia histórica + esqueleto**. No abrir tarea de implementación hasta que haya un caso de uso confirmado.

---

## 12. C++ → Rust mapping (high-level, FUTURE-USE only)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class Scenario : CriteriaHandler` | `struct Scenario` con campo `criteria_handler: CriteriaHandler` | Composition, no inheritance |
| `class InstanceScenario : Scenario` | `struct InstanceScenario { base: Scenario, instance_id: u32 }` | — |
| `class ScenarioMgr` (singleton) | `static SCENARIO_MGR: OnceCell<ScenarioMgr>` | — |
| `enum ScenarioStepState` | `#[repr(u8)] enum ScenarioStepState { Invalid=0, NotStarted=1, InProgress=2, Done=3 }` | — |
| `enum ScenarioType` | `#[repr(u8)] enum ScenarioType { Scenario=0, ChallengeMode=1, Solo=2, Dungeon=10 }` | — |
| `ScenarioData { Entry, Steps: map<u8, ScenarioStepEntry*> }` | `struct ScenarioData { entry: ScenarioEntry, steps: BTreeMap<u8, ScenarioStepEntry> }` | — |
| `ScenarioPOI` con `vector<ScenarioPOIPoint>` | `struct ScenarioPOI { ..., points: Vec<ScenarioPOIPoint> }` | — |
| `unordered_map<pair<uint32, uint8>, ScenarioDBData>` | `HashMap<(u32, u8), ScenarioDBData>` | — |
| `void OnPlayerEnter(Player*)` | `async fn on_player_enter(&self, player: &Player)` | — |
| `bool IsComplete() const` | `pub fn is_complete(&self) -> bool` | — |

---

*Template version: 1.0 (2026-05-01).* **Status:** documento de referencia; implementación N/A para WoLK 3.4.x.
