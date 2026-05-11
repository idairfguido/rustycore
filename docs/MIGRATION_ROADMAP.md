# Migration Roadmap — TrinityCore (wotlk_classic) → RustyCore (Rust)

> Plan operativo para migrar **todo** TrinityCore C++ a Rust. Este documento es la fuente de verdad para prioridad, orden y TODO list. El inventario de estado por módulo vive en `docs/migration/_INDEX.md`. Se actualiza al cierre de cada fase.

**Repos de referencia:**
- C++ origen: `/home/server/woltk-trinity-legacy` (TrinityCore branch `wotlk_classic`)
- Rust destino: `/home/server/rustycore` (este repo, GitHub `alseif0x/rustycore`)
- C# legacy: `/home/server/woltk-server-core/Source/` (referencia secundaria, mismo modelo)

**Reglas inviolables:**

1. **Antes de implementar** cualquier sistema, leer su contraparte C++ en TrinityCore. Nunca improvisar a oído. Lecciones del bridge MapManager fallido (`_attic/`) costaron 176 errores de compilación.
2. **Antes de extender** cualquier sistema ya migrado, **auditarlo contra C++**. Lo que está marcado ✅/⚠️ en este documento puede tener bugs, divergencias o piezas que faltan respecto al C++. **Nada se da por bueno hasta auditoría**. Un sistema "implementado" sin auditar es un riesgo, no una ventaja.
3. Los docs creados por agentes anteriores son útiles como índice, pero no son prueba de corrección. Cada task se valida contra C++ en el momento de ejecutarla.

### Revisión del plan 2026-05-07

Contraste realizado contra el árbol C++ real en `/home/server/woltk-trinity-legacy/src/server/`:

- Inventario top-level correcto: C++ contiene `bnetserver`, `worldserver`, `database`, `proto`, `shared`, `game` y `scripts`.
- `game/` contiene 49 subdirectorios funcionales si se ignora `PrecompiledHeaders`; el plan cubre todos por módulo o como parte de `Entities`/`Scripts`.
- `shared/` contiene 7 módulos reales si se ignora `PrecompiledHeaders`: `DataStores`, `Dynamic`, `JSON`, `Networking`, `Packets`, `Realm`, `Secrets`.
- `scripts/` no es un bloque genérico solamente: tiene `Commands`, `Spells`, `Battlefield`, `Events`, `OutdoorPvP`, `World` y scripts por continente/expansión. La fase de contenido debe mantener esa subdivisión cuando llegue.
- La matriz histórica de este roadmap estaba más optimista que `_INDEX.md`. Desde esta revisión, `_INDEX.md` manda para status/audit; este roadmap manda para orden de ejecución.
- La Fase 0 necesitaba afinarse: en C++ `ObjectGridLoader` no consulta directamente cada tabla por celda. Carga GUIDs preclasificados por `ObjectMgr`/`AreaTriggerDataStore` (`GetCellObjectGuids`, `GetAreaTriggersForMapAndCell`) a partir de `SpawnData`, difficulty, personal phases y respawn state. La cola inmediata se ajusta para no implementar un loader Rust incorrecto.
- R6 corrige el siguiente paso operativo: antes de continuar la Fase 0/L3 Maps, se ejecuta `#NEXT.L0.CONFIG.001` (L0 config parity / startup config schema). Maps no se descarta; queda bloqueado por los gates L0/L1/L2 definidos en R4.

---

## 1. Visión general

### 1.1 Topología C++ que hay que migrar

TrinityCore expone dos binarios ejecutables principales, una librería de scripts linkada al worldserver y 64 módulos documentados en `docs/migration/_INDEX.md`:

**Binarios / librerías:**
- `bnetserver` — autenticación Battle.net (BNet protobuf, SRP6, REST)
- `worldserver` — servidor de juego (sockets WoW, dispatch, todos los sistemas)
- (`scripts` se compila como librería linkada al worldserver)

**Capas (`src/server/`):**
```
shared/      Networking, Packets, Realm, Secrets, DataStores, Dynamic, JSON
game/        49 subdirectorios funcionales (ignorando PrecompiledHeaders)
proto/       definiciones protobuf BNet
database/    capa SQL común a todos los servidores
scripts/     contenido scripteado (commands, spells, continentes, world, events, PvP)
```

**game/** se subdivide en (alfabético, sin agrupar):
```
Accounts          Conditions       Loot          Pools         Spells
Achievements      DataStores       Mails         Quests        Storages
AI                DungeonFinding   Maps          Reputation    Support
AuctionHouse      Entities         Miscellaneous Scenarios     Texts
AuctionHouseBot   Events           Movement      Scripting     Time
Battlefield       Globals          OutdoorPvP    Server        Tools
Battlegrounds     Grids            Petitions     Services      Warden
BattlePets        Groups           Phasing       Skills        Weather
BlackMarket       Guilds           ─             ─             World
Cache             Handlers         ─             ─
Calendar          Instances        ─             ─
Chat              ─                ─             ─
Combat            ─                ─             ─
```

### 1.2 Topología Rust actual (29 crates)

```
crates/
  bnet-server       wow-database      wow-pvp
  world-server      wow-ecs           wow-recastdetour
  wow-achievement   wow-handler       wow-script
  wow-ai            wow-logging       wow-scripts
  wow-chat          wow-loot          wow-social
  wow-collections   wow-map           wow-spell
  wow-combat        wow-math          wow-world
  wow-config        wow-network
  wow-constants     wow-packet
  wow-core          wow-proto
  wow-crypto        wow-data
```

### 1.3 Métrica de avance

La métrica de estado por módulo se mantiene en `docs/migration/_INDEX.md`. No duplicar porcentajes antiguos aquí: ya demostraron quedarse obsoletos y optimistas.

Estado operativo tras el primer barrido de auditoría:

- Total módulos enumerados en `_INDEX.md`: 64.
- Docs por módulo: 64/64.
- Ningún módulo se considera `done` de forma plena contra C++; las marcas `✅` en docs antiguos deben tratarse como sospechosas si no tienen contraste de líneas C++ y tests.
- Estimación global útil para planificación: servidor funcional de forma muy parcial; no usar porcentajes altos heredados como criterio de prioridad.

---

## 2. Capas y dependencias

Grafo de dependencias (← lee como "X depende de Y"):

```
                 ┌─────────────────┐
                 │  L0 Foundation  │  core, constants, config, logging, math, collections
                 └────────┬────────┘
                          │
                 ┌────────┴────────┐
                 │  L1 Infra       │  crypto, database, network, proto, data (DB2/DBC)
                 └────────┬────────┘
                          │
                 ┌────────┴────────┐
                 │  L2 Packets     │  packet, handler (dispatch table)
                 └────────┬────────┘
                          │
                 ┌────────┴────────┐
                 │  L3 World/Maps  │  Map, MapManager, Grid, Cell, ObjectGridLoader  ◄── 🔧 rehacer
                 └────────┬────────┘
                          │
                 ┌────────┴────────────────┐
                 │  L4 Entities            │  Object/WorldObject/Unit/Player/Creature/GameObject
                 └────────┬────────────────┘
                          │
       ┌──────────────────┼──────────────────┬──────────────────┐
       │                  │                  │                  │
   ┌───┴─────┐      ┌────┴─────┐       ┌────┴─────┐      ┌────┴─────┐
   │ L5      │      │ L5       │       │ L5       │      │ L5       │
   │ Movement│      │ Combat   │       │ Spells   │      │ AI       │
   │ Path    │      │ Damage   │       │ Auras    │      │ Smart    │
   └───┬─────┘      └────┬─────┘       └────┬─────┘      └────┬─────┘
       └──────────────────┴──────────────────┴──────────────────┘
                                  │
                          ┌───────┴────────┐
                          │  L6 Game Systems│  Quests, Loot, Inventory, Social,
                          │                 │  Group, Chat, Vendor, Trainer, Mail,
                          │                 │  Auction, Calendar, Achievements,
                          │                 │  Reputation, Skills, Talents
                          └───────┬─────────┘
                                  │
                          ┌───────┴─────────┐
                          │  L7 Battlegrounds│  BG, Arena, OutdoorPvP, Battlefield,
                          │  Instances       │  Instance lock, Difficulty,
                          │  Phasing         │  PhaseMgr, Conditions
                          └───────┬─────────┘
                                  │
                          ┌───────┴─────────┐
                          │  L8 Content     │  Scripts (bosses, NPCs, instances)
                          │                 │  GM commands, Warden, LFG
                          └─────────────────┘
```

**Regla de oro**: una capa solo se considera "trabajable" cuando la inferior está al menos en estado **estable** (compila + tests). No se puede tocar L7 si L4 (entidades) está incompleto.

---

## 3. Estado por módulo (snapshot heredado)

> **No usar esta tabla para decidir si algo está correcto o terminado.** Se conserva como mapa visual de módulos, pero el estado operativo/auditado vive en `docs/migration/_INDEX.md`. Las marcas `✅` de esta sección son heredadas y no equivalen a "port completo contra C++".

Leyenda:
- ✅ done — implementado y tests verdes, cubre el 90%+ de la superficie C++
- ⚠️ partial — implementado parcialmente, falta funcionalidad significativa
- 🔧 broken — implementado pero diseño incorrecto, hay que rehacer
- ❌ missing — no empezado

### L0 Foundation

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Globals` | wow-core | ✅ | — |
| `Time` | wow-core | ✅ | — |
| `Miscellaneous` | wow-core / wow-collections | ✅ | — |
| `Texts` (string formatting) | wow-core | ⚠️ | i18n, broadcast text |

### L1 Infrastructure

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `shared/Networking` | wow-network | ✅ | — |
| `shared/Secrets` | wow-crypto | ✅ | — |
| `shared/DataStores` (DBC) | wow-data | ✅ | — |
| `game/DataStores` (cliente DB2) | wow-data | ⚠️ | varios stores: WMOAreaTable, AreaTable, MapDifficulty |
| `game/Storages` (server-side stores) | wow-data | ⚠️ | varios pendientes |
| `database/` | wow-database | ⚠️ | falta updater de schema, muchos prepared statements |
| `proto/` (BNet protobuf) | wow-proto | ✅ | — |
| `Cache` | wow-data | ⚠️ | hotfix cache OK, falta player cache |

### L2 Packets & Dispatch

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `shared/Packets` (header, encryption) | wow-packet | ✅ | — |
| `Handlers/` (138+ handlers) | wow-handler + wow-world/handlers | ⚠️ | ~75% packets cubiertos; faltan muchos opcodes |

### L3 World/Maps — 🔧 NÚCLEO DE REWRITE

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Maps/Map` | wow-world/map_manager.rs | 🔧 | sin Cell anidado, sin máquina de estados, sin lifecycle |
| `Maps/MapManager` | wow-world/map_manager.rs | 🔧 | singleton OK pero sin update loop, sin DoForAllMaps con lock |
| `Grids/Grid` + `NGrid` | wow-map (parcial) | ❌ | falta NGrid/GridInfo/GridState; `Cell` base ya existe |
| `Grids/Cell` (8×8 dentro de NGrid) | wow-map::cell | ⚠️ | Cell base y GUID containers; falta visitante/entidades reales |
| `Grids/GridStates` (Active/Idle/Removal) | (no existe) | ❌ | sin máquina de estados, grids no se descargan |
| `Grids/ObjectGridLoader` | (no existe) | ❌ | sin lazy load DB → grid |
| `Maps/MapUpdater` (thread pool por map) | (no existe) | ❌ | actualmente todo serializa por RwLock global |
| `Maps/TerrainMgr` + `GridMap` | wow-map (coords/cell parcial) | ❌ | no hay carga de mapas .map/vmap/mmaps de cliente |
| `Maps/MapReference` / `MapRefManager` | (no existe) | ❌ | iteración de jugadores en map |
| `Phasing/PhaseMgr` | (no existe) | ❌ | personal/group phases |
| `Maps/SpawnData` | (no existe) | ❌ | unified spawn descriptors |

### L4 Entities

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Entities/Object/Object` (base) | wow-core (parcial) | ⚠️ | GUID OK, falta jerarquía polimórfica |
| `Entities/Object/WorldObject` | (mezclado en session) | ⚠️ | posición, mapa, fases, eventos |
| `Entities/Unit/Unit` | (no existe formal) | ❌ | health/power/stats/auras/threat — disperso en session |
| `Entities/Player/Player` | wow-entities + wow-world/session.rs | ⚠️ | base entidad iniciado; login/inventario/quests siguen mezclados en sesión |
| `Entities/Creature/Creature` | wow-entities + wow-ai/map_manager legacy | ⚠️ | base entidad iniciado; AI/spawn/loot siguen duplicados fuera |
| `Entities/GameObject/GameObject` | wow-entities + wow-world parcial | ⚠️ | base entidad iniciado; lifecycle/scripts siguen fuera |
| `Entities/Pet/Pet` | wow-entities | ⚠️ | base entidad iniciado; Create/Load/DB/AI pendiente |
| `Entities/DynamicObject` | wow-entities | ⚠️ | base entidad iniciado; Aura/Spell/Map runtime pendiente |
| `Entities/AreaTrigger/AreaTrigger` | wow-entities + wow-data/area_trigger | ⚠️ | base entidad iniciado; templates/runtime/actions pendientes |
| `Entities/Conversation` | wow-entities | ⚠️ | base entidad iniciado; data store/start/runtime pendiente |
| `Entities/Corpse` | wow-entities | ⚠️ | base entidad iniciado; create/load/persistence pendiente |
| `Entities/Vehicle` | wow-entities | ⚠️ | base kit/seats iniciado; auras/events/accessories pendiente |
| `Entities/Transport` (MO) | wow-entities | ⚠️ | base transport iniciado; TransportMgr/path/runtime pendiente |
| `Entities/SceneObject` | wow-entities | ⚠️ | base entidad iniciado; create/map/aura removal pendiente |
| `Entities/Totem` | wow-entities | ⚠️ | base entidad iniciado; TempSummon/Minion runtime pendiente |
| `Entities/Item` | wow-entities | ⚠️ | base Item+Bag+Player storage/ObjectAccessor lookup/visible item state iniciado; InventoryType y visible modifier helpers corregidos; ownership/DB/runtime pendiente |

### L5 Engines: Movement, Combat, Spells, AI

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Movement/MovementInfo` | wow-packet | ⚠️ | lectura base cercana a C++; writer fall-data y campos opcionales pendientes (#A06) |
| `Movement/MoveSpline` | (no existe) | ❌ | spline real con control points |
| `Movement/MovementGenerator` | (no existe) | ❌ | random/waypoint/follow/escort |
| `Movement/PathGenerator` (Detour) | wow-recastdetour | ❌ | crate scaffold, FFI no conectado |
| `Movement/spline/MoveSplineInit` | (no existe) | ❌ | constructor de splines server-side |
| `Combat/CombatManager` | wow-combat | ⚠️ | auto-attack OK, falta swap target / threat |
| `Combat/ThreatManager` | (no existe) | ❌ | sistema de aggro real |
| `Combat/Damage` (school, resistance, mitigation) | wow-combat | ⚠️ | physical OK, falta schools mágicas |
| `Spells/Spell` (engine de cast) | wow-spell | ⚠️ | cast OK, falta projectile, channel real |
| `Spells/SpellMgr` | wow-spell | ⚠️ | parcial |
| `Spells/SpellEffects` (151 efectos) | wow-spell | ⚠️ | DAMAGE/HEAL/AURA básicos, faltan ~140 |
| `Spells/Auras/AuraEffect` | wow-spell | ⚠️ | aura básico, falta periodic real |
| `Spells/SpellHistory` (cooldowns) | wow-world | ⚠️ | cooldowns visibles, falta GCD per-school |
| `AI/CreatureAI` (interfaz base) | wow-ai | ⚠️ | sí pero monolítica |
| `AI/SmartAI` (data-driven) | (no existe) | ❌ | smart_scripts table |
| `AI/ScriptedAI` (boss scripting) | wow-script | ❌ | crate vacío |
| `AI/PetAI` | (no existe) | ❌ | hunter/warlock pets |
| `AI/CombatAI` | (no existe) | ❌ | helper genérico para mobs |

### L6 Game Systems

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Quests/QuestDef` + `QuestMgr` | wow-data + handlers/quest | ⚠️ | accept/complete OK; falta quest pool, daily/weekly, escort, repeatable |
| `Loot/LootMgr` | wow-loot | ⚠️ | drops básicos; falta group rules, conditions, master loot |
| `Loot/LootPackets` | wow-packet | ✅ | — |
| `Skills` | wow-data | ⚠️ | tabla SkillLineAbility OK; falta skill gain, profession recipes |
| `Reputation/ReputationMgr` | (no existe) | ❌ | factions, paragon, exalted bonuses |
| `Chat/Chat` (channels) | wow-chat | ⚠️ | say/yell/whisper OK; falta global channels (Trade/General/LFG) |
| `Mails/MailMgr` | (no existe) | ❌ | sistema de correo COD/items |
| `AuctionHouse/AuctionMgr` | (no existe) | ❌ | listing, bidding, expiración |
| `AuctionHouseBot/` | (no existe) | ❌ | bot que compra/vende |
| `BlackMarket/` | (no existe) | ❌ | subastas especiales |
| `Calendar/CalendarMgr` | (no existe) | ❌ | eventos del calendario |
| `Achievements/AchievementMgr` | wow-achievement (vacío) | ❌ | criterios + progreso |
| `Groups/Group` | wow-social | ⚠️ | invite/accept/leave; falta loot rules, ready check, role check |
| `Guilds/Guild` | (no existe) | ❌ | guild bank, MOTD, ranks, achievements |
| `Petitions/Petition` | (no existe) | ❌ | charter para guilds/arenas |
| `Pools/PoolMgr` | (no existe) | ❌ | spawn pools (rotación de NPCs raros) |
| `Conditions/ConditionMgr` | (no existe) | ❌ | condiciones para drops, gossip, spells |
| `BattlePets/BattlePetMgr` | (no existe) | ❌ | sistema de mascotas combatientes (fuera de WoLK) |
| `OutdoorPvP/OutdoorPvP` (WG, EP, etc.) | wow-pvp (vacío) | ❌ | zonas PvP de mundo abierto |
| `Battlefield/Battlefield` (Wintergrasp) | wow-pvp | ❌ | WG es batalla de zona programada |

### L7 Instances, BG, Arenas, Phasing

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Instances/InstanceLockMgr` | (no existe) | ❌ | bloqueo de jugador a instancia |
| `Instances/InstanceScript` | (no existe) | ❌ | API de scripting de instancia |
| `Instances/InstanceSaveMgr` | (no existe) | ❌ | persistencia entre sesiones |
| `Battlegrounds/Battleground*` (WSG, AB, EotS, etc.) | wow-pvp | ❌ | colas, mapas, capturas |
| `Battlegrounds/ArenaTeamMgr` | wow-pvp | ❌ | rated arena |
| `Phasing/PhaseMgr` | (no existe) | ❌ | personal/group/spell phases |
| `Scenarios/Scenario*` | (no existe) | ❌ | escenarios de 3 jugadores (post-Cata) |
| `DungeonFinding/LFGMgr` | (no existe) | ❌ | cola dungeon |

### L8 Content + Service

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Scripting/ScriptMgr` | wow-script (vacío) | ❌ | API para scripts (boss, instance, gossip) |
| `scripts/` (~3000 scripts C++) | wow-scripts (vacío) | ❌ | content scripts — el grueso del trabajo total |
| `Chat/ChatCommands` (.tele, .gm, etc.) | (no existe) | ❌ | comandos GM |
| `Warden/Warden*` | (no existe) | ❌ | anticheat client-side |
| `Server/WorldSocket` | wow-network | ✅ | — |
| `Server/World` (loop principal) | world-server/main.rs | ⚠️ | tick loop OK, falta orquestación de Maps |
| `Support/Ticket` (GM tickets) | (no existe) | ❌ | sistema de tickets |
| `Accounts/AccountMgr` | wow-database | ⚠️ | login OK, falta gestión de cuenta GM |
| `Services/AccountService`, `BattlepayService` | wow-network | ⚠️ | BNet endpoints |
| `Weather/WeatherMgr` | (no existe) | ❌ | clima dinámico por zona |
| `Events/GameEventMgr` | (no existe) | ❌ | eventos temporales (Hallow's End, etc.) |
| `Cache` (player cache para queries) | (no existe) | ❌ | nombre→guid cache |

---

## 4. Fases de migración (orden ejecutable)

Cada fase es un commit (o pequeño grupo de commits) mergeable a `main` con `cargo check` + `cargo test` verdes. **No se salta a la siguiente sin la anterior cerrada.**

### Fase R — Refinamiento WBS completo (precondición)

> Antes de seguir implementando, convertir el plan en una estructura de tareas/subtareas verificable contra C++. El procedimiento completo vive en `docs/migration/refinement-plan.md`.

- **R.1** Inventariar el árbol C++ completo: archivos, handlers/opcodes, SQL, DB2/DBC, config, entidades y scripts.
- **R.2** Actualizar cada módulo de `docs/migration/*.md` con WBS granular: task IDs, C++ refs, dependencias, aceptación y tests.
- **R.3** Crear registros transversales para opcodes, SQL, update fields, runtime managers y scripts.
- **R.4** Revisar dependencias y gates de implementación; reordenar roadmap si C++ demuestra que el orden actual es incorrecto.
- **R.5** Hacer gap audit: ningún archivo/opcode/script relevante de C++ queda sin dueño o exclusión explícita.
- **R.6** Elegir la siguiente mini-fase solo cuando tenga C++ refs y tests definidos.

### Fase A — Auditoría obligatoria de lo existente (precondición)

> Esta fase **se ejecuta en paralelo** con Fase 0 y siguientes; cada módulo se audita **antes** de extenderlo. No bloquea Fase 0 (Maps ya está auditado y la conclusión es "rehacer"). Bloquea Fase 1+ porque Entities depende de saber qué hay realmente en `wow-core`/`wow-world`.

**Objetivo:** para cada módulo marcado ✅/⚠️ en sección 3, producir un mini-informe `docs/audits/<modulo>.md` con:

- Archivos C++ canónicos del módulo (cite líneas).
- Archivos Rust correspondientes y resumen de qué hacen.
- **Tabla de divergencias**: feature C++ → estado Rust → ¿bug? ¿missing? ¿extra? ¿correcto pero distinto y aceptable?
- TODO list de fixes específicos del módulo (pueden añadirse a la sección 5).
- Cambia la columna "Auditado vs C++" del módulo de ❌ → ⚠️ (si parcial) o ✅ (completo).

**Orden recomendado de auditoría** (por dependencias y por probabilidad de bugs):

- **A.1** Maps & Grids (✅ ya hecho, conclusión: rehacer en Fase 0)
- **A.2** Packets & Dispatch (alta superficie, alta probabilidad de divergencias en wire format)
- **A.3** Network (BNet handshake, WorldSocket encryption)
- **A.4** Crypto (SRP6, AES-GCM, HMAC) — cifrado roto = todo se cae
- **A.5** Database (statements, transacciones, prepared)
- **A.6** Foundation (GUID, Position, Time)
- **A.7** Movement (parsing y validación)
- **A.8** Combat (damage calc, miss table)
- **A.9** Spells & Auras
- **A.10** Quests
- **A.11** Inventory
- **A.12** Loot
- **A.13** Chat
- **A.14** Social, Group/Raid
- **A.15** Trainer/Vendor/Gossip
- **A.16** Resto

**Cómo auditar (proceso por módulo):**

1. Localizar archivos C++ de referencia (`/home/server/woltk-trinity-legacy/src/server/`).
2. Leer directamente los `.h/.cpp` relevantes y citar símbolos/archivos concretos. Si se usa cualquier agente auxiliar, su salida solo sirve como pista y se verifica manualmente contra C++.
3. Leer el código Rust correspondiente.
4. Producir tabla de divergencias.
5. Para cada divergencia: clasificar como **bug** (Rust diverge mal) / **missing** (Rust no implementa) / **extra** (Rust hace de más) / **OK** (divergencia aceptable, ej. idiom Rust).
6. Para bugs y missing críticos: añadir TODO en sección 5.
7. Commit `docs(audit): audit <modulo>` con el mini-informe.

### Fase 0 — Fundación de Maps (rehacer L3) — *🔧 siguiente tras gates L0-L2*

> El bloqueante de TODO lo demás. Sin Map/Grid/Cell correctos, ni entidades, ni AI, ni multi-player escalable.
> Nota R6: esta fase no se reanuda hasta cerrar `#NEXT.L0.CONFIG.001`.

- **0.1** `wow-map`: constantes (`SIZE_OF_GRIDS=533.3333`, `MAX_NUMBER_OF_GRIDS=64`, `MAX_NUMBER_OF_CELLS=8`), tipos `GridCoord`, `CellCoord`, `MapKey`, conversión `compute_grid_coord(x,y)` / `compute_cell_coord(x,y)`. Tests unitarios contra `GridDefines.h`. **Cerrado #001-#003.**
- **0.2** `wow-map`: `GridInfo`, `GridStateKind` y `NGrid` 8×8 `Cell`, siguiendo `NGrid.h` y `GridStates.cpp`.
- **0.3** `wow-map`: `Map` skeleton con `EnsureGridCreated`, `EnsureGridLoaded`, `LoadGridObjects`, `ResetGridExpiry`, `CanUnload`, `Update`, sin handlers todavía.
- **0.4** Spawn stores previos al loader: `SpawnData`, creature/gameobject/areatrigger spawn stores por `(map, difficulty, cell_id)` como C++ `ObjectMgr::AddSpawnDataToGrid` y `AreaTriggerDataStore`.
- **0.5** `ObjectGridLoader`: carga lazy desde esos stores, no desde queries ad hoc por celda; incluir corpses, respawn state y personal phase hooks como pendientes explícitos.
- **0.6** `MapManager` + `MapUpdater`: `i_maps`, `create_map`, `find_map`, `update`, `destroy_map`, delayed update y pool opcional según `MapManager.cpp`.
- **0.7** Integración worldserver: arrancar update loop global, reemplazar `crates/wow-world/src/map_manager.rs` legacy y migrar handlers que tocan `self.creatures`.
- **0.8** Quitar campos legacy `creatures`/`visible_creatures` de `WorldSession`.

### Fase 1 — Entidades canónicas (L4)

- **1.1** `wow-entities`: base `Object` / `WorldObject` con map/cell/phase/current cell y GUID typing contrastado contra `Entities/Object/`.
- **1.2** `ObjectAccessor`/map stored object registry equivalente a `Globals/ObjectAccessor.*` y `MapStoredObjectTypesContainer`; sin esto los handlers acabarán reintroduciendo lookups por sesión.
- **1.3** Update fields temprano (`Entities/Object/Updates/` + `UpdateFields.h`): masks/deltas por tipo antes de expandir Player/Unit, para no seguir generando full re-create.
- **1.4** `Unit`, `Player`, `Creature`, `GameObject`, `Corpse`, `DynamicObject`, `AreaTrigger`, `Pet`, `Transport`, `Vehicle`, `SceneObject`, `Conversation`, `Totem`; `Taxi` se trata como soporte de Player/Transport, no como sistema suelto.
- **1.5** Mover IA pura de `wow-ai` a `Creature`/AI refs sin mezclar comportamiento con sesión.
- **1.6** Refactor `WorldSession` para que `player` sea una referencia/controlador de entidad, no la entidad completa.

### Fase 2 — Movement & Pathfinding (L5)

- **2.1** `wow-movement` (renombrar/extender `wow-recastdetour`): MoveSpline real con control points.
- **2.2** Pathfinding: bindings FFI a Detour reales (no stubs), cargar navmesh `.mmtile` del cliente.
- **2.3** `MovementGenerator`: Idle / Random / Waypoint / Follow / Confused / Fleeing.
- **2.4** Server-side movement validation (anticheat básico): velocidad, jump, teleport range.

### Fase 3 — Combat & Threat (L5)

- **3.1** `wow-combat`: school resistances, miss/dodge/parry/block tablas reales por nivel.
- **3.2** `ThreatManager` per-Unit: tabla de threat, switch target, taunt.
- **3.3** Damage events: `SMSG_ATTACKER_STATE_UPDATE` con todos los campos (school mask, hit info, blocked, absorbed, resisted).
- **3.4** XP/Honor del kill (interactúa con L6 Quests para kill credit).

### Fase 4 — Spells & Auras (L5)

- **4.1** `wow-spell`: SpellEffect handlers para los 151 effects (al menos 30 más comunes en WoLK: damage, heal, aura, summon, teleport, charge...).
- **4.2** Aura periódica real (DoT/HoT con tick interval).
- **4.3** Channeled spells (mind flay, drain life).
- **4.4** Projectile spells (arrow, fireball con velocity).
- **4.5** GCD per-school + spell history persistente entre sesiones.

### Fase 5 — AI escalable (L5)

- **5.1** `wow-ai`: trait `CreatureAI` con métodos `update_ai`, `enter_combat`, `kill_unit`, `damage_taken`, `move_in_line_of_sight`.
- **5.2** `SmartAI` data-driven (lee `smart_scripts` de world DB).
- **5.3** `ScriptedAI` interfaz para scripts C++/Rust (boss en instancia).
- **5.4** `PetAI`, `CombatAI` genéricos.

### Fase 6 — Game systems pendientes (L6)

> Cada uno es un sub-proyecto. Orden por dependencias y prioridad de jugabilidad.

- **6.1** Inventory completo: bags, bank, durability, transmog, soulbound rules.
- **6.2** Chat channels globales (Trade, General, LookingForGroup) — depende de PhaseMgr para área.
- **6.3** Reputation: factions, paragon, repmod buffs.
- **6.4** Mail: items, COD, expiración, attachment limits.
- **6.5** Quest features avanzadas: pool, daily/weekly, escort, repeatable, area quests.
- **6.6** Achievements + criterios + persistencia.
- **6.7** Group: loot rules (FFA/group/master), ready check, role check, raid markers.
- **6.8** Guilds completas: bank, MOTD, ranks, perks, achievements.
- **6.9** Auction House + AHBot.
- **6.10** Calendar + events.
- **6.11** Black Market.

### Fase 7 — Instances, BG, Arenas, Phasing (L7)

- **7.1** Instance lock + difficulty + map switch flow (ConnectTo en realm separado).
- **7.2** InstanceScript trait + persistencia de estado.
- **7.3** Phasing: PhaseMgr por player y por área.
- **7.4** Conditions engine.
- **7.5** Battlegrounds (4-5 BGs WoLK: WSG, AB, EotS, AV, SotA, IoC) — colas, mapa, captura.
- **7.6** Arenas: rated, skirmish, conquista.
- **7.7** OutdoorPvP zones (WG, EP, HP, TF).
- **7.8** Battlefield (Wintergrasp como caso especial).
- **7.9** LFGMgr: cola dungeon finder.

### Fase 8 — Content & Service (L8)

- **8.1** ScriptMgr API: registro de scripts, hooks (boss, gossip, instance, npc, item, spell, area).
- **8.2** Migrar `scripts/Commands` primero para GM tooling mínimo (`.tele`, `.gm`, `.level`, `.item`, `.additem`, `.lookup...`), porque acelera la validación runtime de todo lo anterior.
- **8.3** Migrar `scripts/Spells`, `scripts/World`, `scripts/Events`, `scripts/Battlefield`, `scripts/OutdoorPvP` y scripts por zona/continente en bloques separados. No tratar `scripts/` como una masa única.
- **8.4** Warden (opcional, anticheat client-side).
- **8.5** Weather, GameEvents, Tickets, AccountMgr GM.

---

## 5. TODO list operativo (próximas 40+ acciones, ordenadas)

> Esta es la cola accionable. Cada ítem tiene un commit/PR esperado. Marcar `[x]` al cerrar.

### Auditorías iniciales (Fase A) — paralelas a Fase 0

> Cada auditoría produce `docs/audits/<modulo>.md` con tabla de divergencias y TODOs específicos.

- [ ] **#A01** Auditar **Packets & Dispatch** (`wow-packet`, `wow-handler`, `wow-world/handlers/`) vs `src/server/shared/Packets/` + `Handlers/`. ¿Wire format correcto? ¿Bit-packing fiel? ¿Opcodes en sync con cliente 3.4.3.54261?
- [ ] **#A02** Auditar **Network/WorldSocket** (`wow-network`) vs `src/server/Server/WorldSocket.cpp` + `WorldSocketMgr`. Encryption flow, header bytes, dispatch.
- [ ] **#A03** Auditar **Crypto** (`wow-crypto`) vs `src/server/shared/Cryptography/`. SRP6 idéntico al usado por cliente, AES-GCM nonce construction, HMAC-SHA256 keys.
- [ ] **#A04** Auditar **Database** (`wow-database`) vs `src/server/database/`. Statements registrados, prepared, transacciones, escapeo.
- [ ] **#A05** Auditar **Foundation** (`wow-core`) vs `src/server/game/Globals/` + `src/server/shared/`. GUID encoding, Position math, Time.
- [x] **#A06** Auditar **Movement parsing** (`wow-packet/movement.rs`, handlers/movement.rs) vs `src/server/game/Server/Packets/MovementPackets.*`, `Entities/Object/MovementInfo.h` y `Handlers/MovementHandler.cpp`. Resultado: `docs/audits/movement.md`; `MovementInfo::read` está cerca del wire C++, pero el writer y handler quedan en ⚠️ con subtareas:
  - [x] **#A06.1** Corregir `MovementInfo::write` para que `hasFallData = falling flags || fallTime != 0` y `hasFallDirection = falling flags`, como `MovementPackets.cpp`.
  - [x] **#A06.2** Representar `standingOnGameObjectGUID` e `inertia` en `MovementInfo`; no descartarlos al leer ni forzarlos a `false` al escribir.
  - [x] **#A06.3** Endurecer validación mínima de `handle_movement`: GUID cargado y exactamente el jugador actual, más `Position::is_valid_map_coord_like_cpp()` para X/Y/Z/orientación; los guards de teleport/movespline quedan ligados al runtime Movement/Map cuando existan equivalentes.
  - [x] **#A06.4** Validar transport sin integración `TransportBase` completa: offset ±75, distancia > grid tras teleport y world coord con transport. El reset/add-remove real de passenger queda para Transport/Map runtime.
  - [x] **#A06.5** Portar `AdjustClientMovementTime`: cola circular de 6 muestras, pendientes por sequence, delta filtrado por latencia y ajuste de `MovementInfo.time` antes de rebroadcast.
  - [ ] **#A06.6** Separar side effects C++ de movement: fall damage, aura interrupts, pet unsummon, sit-to-stand, under-map damage y jump procs.
    - [x] **#A06.6.1** Portar side effects representables hoy: remover auras `LandingOrFlight`/`Jump`, sentar-a-stand en movimiento y registrar hooks de unsummon temporal de pet / jump proc.
    - [x] **#A06.6.2** Portar base de `Player::HandleFall` / `UpdateFallInformationIfNeed`: `m_lastFallTime/Z`, umbral 14.57, fórmula C++ de daño, clamp a max health y aplicación al estado representado de salud/vida.
    - [x] **#A06.6.3** Portar under-map damage: `MapManager::min_height_like_cpp` expone el fallback C++ `TerrainInfo::GetMinHeight == -500.0` cuando no hay grid, `handle_movement` marca `PLAYER_FLAGS_IS_OUT_OF_BOUNDS` representado, aplica `DAMAGE_FALL_TO_VOID` por vida máxima y mata si el daño ambiental no lo hizo. La altura real de terrain/grid y el hook Battleground siguen en las tareas Map/Terrain/Battleground, no se finge aquí.
    - [x] **#A06.6.4a** Portar los modificadores/guards de aura usados por `Player::HandleFall`: `HOVER`, `FEATHER_FALL`, `FLY`, inmunidad normal, `SAFE_FALL`, `MODIFY_FALL_DAMAGE_PCT`, `CHEAT_GOD`, inmunidad ambiental y aura 43621 (Gust of Wind) sobre el modelo de aura representado actual.
    - [ ] **#A06.6.4** Conectar hooks registrados a Pet runtime y Proc/Aura runtime completos: `UnsummonPetTemporaryIfAny`, `Unit::ProcSkillsAndAuras(PROC_FLAG_JUMP)` y aura runtime real en vez de `visible_auras` representado.
  - [x] **#A06.7** Portar efectos de `MoveInitActiveMoverComplete`: lee `Ticks`, setea `PLAYER_LOCAL_FLAG_OVERRIDE_TRANSPORT_SERVER_TIME`, calcula `TransportServerTime = GameTimeMS - Ticks`, emite VALUES update mínimo de `ActivePlayerData::{LocalFlags,TransportServerTime}` y refresca visibility como C++.
  - [ ] **#A06.8** Inventariar y portar ACK movement opcodes (`KnockBack`, speed, force, collision height, spline done, time skipped).
    - [x] **#A06.8a** Portar parsers/handlers base contrastados con C++ para ACKs genéricos (`HandleMovementAckMessage`), speed ACKs, `MoveKnockBackAck`, `MoveSetCollisionHeightAck`, `MoveTimeSkipped`, `MoveSplineDone` y `MoveTeleportAck`: Rust valida GUID/coord, ajusta time en knockback, suma `TimeSkipped` al tiempo de movimiento representado y registra eventos para los runtime gaps.
    - [x] **#A06.8b** Portar counters y anticheat representados de `HandleForceSpeedChangeAck` / `HandleMoveSetModMovementForceMagnitudeAck`: Rust mapea opcode a `UnitMoveType`, mantiene `m_forced_speed_changes` / `m_movementForceModMagnitudeChanges` equivalentes, compara contra `playerBaseMoveSpeed * rate` con tolerancia `0.01`, respeta transport como C++, registra corrección si el cliente va lento y kick si declara más velocidad/magnitud. El `SetSpeedRate` productivo, el paquete de corrección al cliente y `MovementForces::GetModMagnitude()` real quedan bajo `#A06.8g`/runtime Unit-MovementForce porque hoy no existe ese runtime completo.
    - [x] **#A06.8c** Emitir broadcasts C++ pendientes para ACKs portados: `SMSG_MOVE_UPDATE_KNOCK_BACK`, `SMSG_MOVE_SKIP_TIME`, `SMSG_MOVE_UPDATE_APPLY_MOVEMENT_FORCE`, `SMSG_MOVE_UPDATE_REMOVE_MOVEMENT_FORCE`, `SMSG_MOVE_UPDATE_MOD_MOVEMENT_FORCE_MAGNITUDE`, excluyendo al jugador local como C++.
    - [x] **#A06.8d** Portar `MovementForce` wire (`MoveApplyMovementForceAck`/`MoveRemoveMovementForceAck`): GUID, origin/direction XYZ, transport id, magnitude, `Unused910` y type de 2 bits; handlers validan GUID/coord, ajustan time y registran force id/type.
    - [x] **#A06.8e** Integrar side effects representables de `MoveSplineDone` con Taxi/MotionMaster: Rust distingue vuelo taxi en progreso, registra branch sin `FLIGHT_MOTION_TYPE`, representa teleport multi-map/teleport-node actualizando mapa/posición, limpia destino final como `CleanupAfterTaxiFlight`, remueve flags taxi/control, desmonta estado representado, resetea fall info y registra cast de Honorless Target 2479 si `pvpInfo.IsHostile`. El runtime real `PlayerTaxi`, `FlightPathMovementGenerator`, `SMSG_ON_MONSTER_MOVE` completo, costes/intermediate nodes y early landing quedan para la fase Taxi/MotionMaster porque hoy no existe esa base completa.
    - [x] **#A06.8f** Integrar side effects representables de `MoveTeleportAck` con near-teleport: Rust mantiene semáforo near y destino representado, ignora ACKs si no hay teleport o si el GUID no coincide, aplica mapa/posición destino, resetea fall info, actualiza zone/area representados, ejecuta ramas PvP/Honorless Target 2479 representadas, registra resummon temporal de pet y `ProcessDelayedOperations`. El runtime real `Player::TeleportTo`, `SMSG_MOVE_TELEPORT`, `SMSG_MOVE_UPDATE_TELEPORT`, access checks, vehicle/transport/duel/BG/arena cleanup y Map/Terrain zone resolver quedan para la fase Teleport/Map/Movement completa.
    - [ ] **#A06.8g** Reemplazar la validación conservadora de ACKs por `Player::ValidateMovementInfo` completo sobre flags cuando Unit/Aura/Vehicle runtime esté disponible.
      - [x] **#A06.8g.1** Portar la sanitización representable de `Player::ValidateMovementInfo`: Rust muta `MovementInfo.flags` en los ACK paths portados como C++ para `ROOT` sin vehículo fijo, pares incompatibles (`ASCENDING/DESCENDING`, `LEFT/RIGHT`, `STRAFE_LEFT/STRAFE_RIGHT`, `PITCH_UP/PITCH_DOWN`, `FORWARD/BACKWARD`), `HOVER`, `WATER_WALK`, `FALLING_SLOW`, `FLYING/CAN_FLY`, `FALLING` con `DISABLE_GRAVITY/CAN_FLY` y `SPLINE_ELEVATION` según `stepUpStartElevation`.
      - [ ] **#A06.8g.2** Conectar `ValidateMovementInfo` completo e integrado en todos los ACK/broadcasts cuando existan `Unit::HasAuraType`, `VehicleInfo::VEHICLE_FLAG_FIXED_POSITION`, mover controlado, `SPELL_AURA_FLY` vs `SPELL_AURA_MOD_INCREASE_MOUNTED_FLIGHT_SPEED` real y runtime Aura/Vehicle productivo. No dar esta pieza por cerrada con el modelo representado.
    - [ ] **#A06.8h** Portar `SMSG_ON_MONSTER_MOVE` / `MovementMonsterSpline` contra `MovementPackets.cpp` y `MoveSpline`.
      - [x] **#A06.8h.1** Corregir el wire puro de `MonsterMove`/`MonsterMoveStop`: Rust escribe `MovementMonsterSpline` + `MovementSpline` en el orden C++ (`ID`, `Destination`, `CrzTeleport`, `StopDistanceTolerance`, flags/elapsed/moveTime/fade/mode/transport/seat, counts bit-packed, face payload, puntos y packed deltas) y corrige `Done = 0x20`/stop sin flags de move.
      - [x] **#A06.8h.2** Reactivar el envío lineal representado desde `tick_creatures_sync` usando el builder C++-like ya testeado: cuando una criatura inicia wander representado, Rust emite `SMSG_ON_MONSTER_MOVE` con `MovementMonsterSpline` lineal, spline id incrementado, duración C++-like mínima 500ms y punto destino único.
      - [ ] **#A06.8h.3** Implementar `MoveSpline`/`MotionMaster` real: CatmullRom, cyclic, parabolic/fall, spline filters, spell visual extra data, transport motion, pathgen y lifecycle completo.
        - [x] **#A06.8h.3a** Crear `crates/wow-movement` y portar el núcleo contrastado de `MoveSplineFlag`, `MonsterMoveType`, `FacingInfo`, `SpellEffectExtraData`, `MoveSplineInitArgs::Validate`, `computeFallTime`, `computeFallElevation`, almacenamiento CatmullRom-compatible, duración por segmentos, `MoveSpline::Initialize`, `ComputePosition`, parabolic/fall elevation, `updateState`, cyclic wrap y `Finalize` contra `MoveSplineFlag.h`, `MovementTypedefs.h`, `MovementUtil.cpp`, `MoveSplineInitArgs.h`, `Spline.cpp` y `MoveSpline.cpp`.
        - [ ] **#A06.8h.3b** Completar el núcleo `MoveSpline` pendiente: rewrite de `Enter_Cycle` preservando duración C++, `AnimTierTransition`, curvas DB2 de `SpellEffectExtraData`, Bezier si resulta necesario por callers, y fixtures adicionales contra C++.
          - [x] **#A06.8h.3b.1** Portar `Enter_Cycle` de `MoveSpline::_updateState`: al completar el primer ciclo, Rust elimina el primer vértice de la ruta y reconstruye el spline preservando la duración anterior como C++.
          - [x] **#A06.8h.3b.2** Portar `AnimTierTransition` en `MoveSplineInitArgs`/`MoveSpline` y conservar `effect_start_time` para el futuro mapper de `MovementSpline::AnimTierTransition`.
          - [x] **#A06.8h.3b.3** Portar el helper de `SplineImpl.h::computeIndex/evaluate_percent` como `compute_position_percent`, con reglas C++ para `t=1.0`.
          - [ ] **#A06.8h.3b.4** Conectar curvas DB2 de `SpellEffectExtraData::{ProgressCurveId,ParabolicCurveId}` y ampliar fixtures contra C++ cuando exista store/mapper real.
        - [ ] **#A06.8h.3c** Conectar el `MoveSpline` real a `Unit`/`MoveSplineState` y mapper `MoveSpline -> MovementMonsterSpline`, incluyendo extras packet (`SplineFilter`, `SpellEffectExtraData`, `JumpExtraData`, `AnimTierTransition`) y `SMSG_FLIGHT_SPLINE_SYNC` cíclico.
          - [x] **#A06.8h.3c.1** Portar el mapper packet-side contrastado con `MovementPackets.cpp::MonsterMove::InitializeSplineData`: Rust expone path data C++-like desde `MoveSpline`, serializa opcionales `SplineFilter`/`SpellEffectExtraData`/`JumpExtraData`/`AnimTierTransition` en el orden C++ y construye `MovementMonsterSpline` con flags, face, duración, fade, puntos y packed deltas.
          - [ ] **#A06.8h.3c.2** Sustituir el `MoveSplineState` representado de `Unit`/criaturas por `wow_movement::MoveSpline` real y usar el mapper anterior en broadcasts.
          - [ ] **#A06.8h.3c.3** Portar `SMSG_FLIGHT_SPLINE_SYNC` y el sync cíclico asociado.
        - [ ] **#A06.8h.3d** Implementar `MoveSplineInit::Launch/Stop`: current-position chaining, sustitución de `path[0]`, selección/clamp de velocidad, flags de movimiento, transport-local transform, `MonsterMove` broadcast y stop con tolerancia 2.
        - [ ] **#A06.8h.3e** Implementar `MotionMaster` real y primeros generators (`Generic`, `Point`, `MoveJump`, `MoveCharge`, `MoveFall`, `MoveCirclePath`) antes de pathgen completo.
        - [ ] **#A06.8h.3f** Implementar pathgen/Detour o política de fallback explícita contrastada con `PathGenerator.cpp`; no cerrar `#A06.8h.3` mientras `wow-recastdetour` siga vacío.
- [ ] **#A07** Auditar **Combat** (`wow-combat` + handlers) vs `src/server/game/Combat/`. Damage roll, miss tables, hit info.
- [ ] **#A08** Auditar **Spells** (`wow-spell`) vs `src/server/game/Spells/`. Spell flow, casting, effects subset.
- [ ] **#A09** Auditar **Quests** (handlers/quest.rs, wow-data/quest) vs `src/server/game/Quests/`. Eligibility, kill credit, completion, reward.
- [ ] **#A10** Auditar **Inventory** (handlers/character.rs partes inventario) vs `src/server/game/Entities/Player/PlayerStorage.cpp`.
- [ ] **#A11** Auditar **Loot** (`wow-loot`) vs `src/server/game/Loot/LootMgr.cpp`. Drop chance, condition support.
- [ ] **#A12** Auditar **Chat** (`wow-chat`) vs `src/server/game/Chat/`. Mensaje broadcast, silenciamiento, anti-spam.
- [ ] **#A13** Auditar **Social** (`wow-social`) vs `src/server/game/Handlers/SocialHandler.cpp`.
- [ ] **#A14** Auditar **Group** vs `src/server/game/Groups/`.
- [ ] **#A15** Auditar **Trainer/Vendor/Gossip** (handlers) vs `src/server/game/Handlers/NPCHandler.cpp`.

### Refinamiento completo (Fase R)

- [ ] **#REFINE.001** Congelar features nuevas hasta refinar la siguiente mini-fase completa.
- [x] **#REFINE.010** Inventario árbol C++ `src/server` en `docs/migration/inventory/cpp-server-tree.md`.
- [x] **#REFINE.011** Inventario C++ por archivo y módulo en `docs/migration/inventory/cpp-files-by-module.md`.
- [x] **#REFINE.012** Inventario handlers/opcodes en `docs/migration/inventory/cpp-handlers-opcodes.md`.
- [x] **#REFINE.013** Inventario SQL/prepared statements en `docs/migration/inventory/cpp-sql-prepared.md`.
- [x] **#REFINE.014** Inventario DB2/DBC/hotfix stores en `docs/migration/inventory/cpp-dbc-db2-stores.md`.
- [x] **#REFINE.015** Inventario config world/bnet en `docs/migration/inventory/cpp-config-keys.md`.
- [x] **#REFINE.016** Inventario entity types en `docs/migration/inventory/cpp-entity-types.md`.
- [x] **#REFINE.017** Inventario `scripts/*` en `docs/migration/inventory/cpp-scripts-tree.md`.
- [x] **#REFINE.020** Cobertura canonica de ficheros C++ en cada doc de modulo.
- [x] **#REFINE.021** Rust target exacto por cada doc de modulo (`docs/migration/inventory/r2-rust-targets.md`).
- [x] **#REFINE.022** WBS granular por cada doc de modulo (`docs/migration/inventory/r2-task-wbs.md`).
- [x] **#REFINE.023** Divergencias/bugs conocidos con evidencia C++ (`docs/migration/inventory/r2-known-divergences.md`).
- [x] **#REFINE.024** Tests required por modulo (`docs/migration/inventory/r2-tests-required.md`).
- [x] **#REFINE.025** Sistemas post-WoLK/desactivados marcados sin omision silenciosa (`docs/migration/inventory/r2-product-scope.md`).
- [x] **#REFINE.030** Registros transversales de opcodes, SQL, update fields, managers, scripts y harness (`docs/migration/inventory/r3-cross-registry-summary.md`).
- [x] **#REFINE.040** DAG de dependencias y gates por fase (`docs/migration/inventory/r4-dependency-gate-summary.md`).
- [x] **#REFINE.050** Gap audit de archivos/opcodes/SQL/scripts (`docs/migration/inventory/r5-gap-audit.md`).
- [x] **#REFINE.060** Selección de la siguiente mini-fase lista para implementación (`docs/migration/inventory/r6-next-miniphase.md`).

### Inmediato (R6 — L0 config parity)

- [x] **#NEXT.L0.CONFIG.001** Ejecutar `docs/migration/inventory/r6-next-miniphase.md`: nombres canonicos `worldserver.conf`/`bnetserver.conf`, parsing semicolonado `*DatabaseInfo`, overlays `.conf.d`, override `TC_*`, y consumo de startup world/bnet contra C++. Cerrado en código, incluido `#NEXT.L0.CONFIG.REMOVE_LEGACY_DB_SUBKEYS`.
- [x] **#NEXT.L0.CONFIG.002** Portar `WorldBoolConfigs`/`WorldFloatConfigs`/`WorldIntConfigs`/`WorldInt64Configs` contra `World.cpp`.
  Estado: cerrado con `#NEXT.L0.CONFIG.002.a` registry/defaults, `#NEXT.L0.CONFIG.002.b` validaciones C++ y `#NEXT.L0.CONFIG.002.c` wiring runtime.

### Inmediato (R7 — L1 infra gate)

- [ ] **#NEXT.L1.INFRA.001** Ejecutar `docs/migration/inventory/r7-l1-infra-miniphase.md`: database/prepared + DB2/hotfix gate contra C++.
  Estado: `#NEXT.L1.INFRA.001.a` cerrado; `#NEXT.L1.INFRA.001.b/c` refinados; siguen `#NEXT.L1.DB.PREP.CHARACTER`, `#NEXT.L1.DB.PREP.HOTFIX` y `#NEXT.L1.DB2.STORES`.

### Inmediato (R7 — L2 packets/dispatch gate)

- [x] **#NEXT.L2.DISPATCH.001** Ejecutar `docs/migration/inventory/r7-l2-packets-miniphase.md`: restaurar metadata C++ de dispatch para opcodes tocados (`PROCESS_THREADSAFE`, duplicados y variantes `TimeSyncResponse*`).
- [ ] **#NEXT.L2.DISPATCH.002** Generar/auditar tabla completa de metadata de opcodes cliente desde `Opcodes.cpp`.
- [ ] **#NEXT.L2.PACKET.WIRE.001** Dividir auditoría wire de parsers/serializers por ruta login-to-world.

### Inmediato (Fase 0 — Maps rewrite)

- [x] **#001** `wow-map`: módulo `coords.rs` con constantes y `compute_grid_coord` / `compute_cell_coord`. Tests vs `GridDefines.h`. Cerrado en `crates/wow-map/src/coords.rs` contra `GridDefines.h`.
- [x] **#002** `wow-map`: `MapKey { map_id: u32, instance_id: u32 }`, matching C++ `std::pair<uint32, uint32>`.
- [x] **#003** `wow-map`: `Cell` struct con containers tipados por GUID para world/grid objects; referencias reales quedan para NGrid/entities.
- [x] **#004** `wow-map`: `GridInfo` + `GridStateKind` from `NGrid.h`: time tracker, relocation timer period, unload active lock, explicit unload lock, loaded flag semantics. Cerrado en `crates/wow-map/src/grid.rs`.
- [x] **#005** `wow-map`: `NGrid` (8×8 `Cell`) from `NGrid.h`: grid id `x * MAX_NUMBER_OF_GRIDS + y`, x/y, state, `is_grid_object_data_loaded`, `get_grid_type`, `visit_grid`, `visit_all_grids`, world-object count by type. Cerrado en `crates/wow-map/src/grid.rs`.
- [x] **#006** `wow-map`: `GridState` update functions from `GridStates.cpp`: Invalid no-op, Active → Idle when no players/active objects, Idle → Removal, Removal → unload if no lock. Implementado con `MapGridHost` para mantenerlo testeable antes de full `Map`.
- [x] **#007** `wow-map`: `Map` skeleton from `Map.cpp`: `i_grids[64][64]`, `ensure_grid_created`, `ensure_grid_loaded`, `ensure_grid_loaded_for_active_object`, `load_grid_objects`, `reset_grid_expiry`, `active_objects_near_grid`, `unload_grid`. Cerrado en `crates/wow-map/src/map.rs` con hooks explícitos para terrain/object lifecycle.
- [x] **#008** `wow-map`/`wow-data`: `SpawnData` and spawn-store model from `Maps/SpawnData.h` + `ObjectMgr::AddSpawnDataToGrid`: creature/gameobject spawn ids indexed by `(map_id, difficulty, cell_id)` plus personal phase variant `(map_id, difficulty, phase_id, cell_id)`; areatriggers follow C++ `AreaTriggerDataStore` by `(map_id, difficulty, cell_id)` only. Cerrado en `crates/wow-map/src/spawn.rs`.
- [x] **#009** `wow-database`: prepared statements/loaders for creature, gameobject and areatrigger spawn data. Do not implement a per-cell loader query as the canonical model; C++ preloads stores and `ObjectGridLoader` consumes GUID sets. Cerrado con `SEL_CREATURE_SPAWNS`, `SEL_GAMEOBJECT_SPAWNS`, `SEL_AREATRIGGER_SPAWNS` y spawn-group statements contra `ObjectMgr.cpp`/`AreaTriggerDataStore.cpp`.
- [x] **#010** `wow-map`: `ObjectGridLoader::load_n(grid)` from `ObjectGridLoader.cpp`: iterate all 8×8 cells, load creature/gameobject/areatrigger GUIDs from stores, load corpses from map corpse store, set current cell, add to world/grid containers. Cerrado a nivel GUID/container en `crates/wow-map/src/object_grid_loader.rs`; `LoadFromDB`, `MapObject::SetCurrentCell` y `AddToWorld` reales quedan ligados a `#023` entidades canónicas.
- [x] **#010a** `wow-map`: `MultiPersonalPhaseTracker` grid hook from `PersonalPhaseTracker.cpp`: player-triggered grid loading loads personal creature/gameobject spawns once per owner/grid/phase, unload removes grid tracking, owner phase changes mark missing phases for delayed deletion. Cerrado en `crates/wow-map/src/personal_phase.rs` y conectado a `Map::ensure_grid_loaded_for_player_phase`.
- [x] **#011** `wow-map`: grid unload helpers from `ObjectGridLoader.cpp`: `ObjectGridStoper`, `ObjectGridEvacuator`, `ObjectGridCleaner`, `ObjectGridUnloader` traversal/order over grid containers. Cerrado como action pass GUID/container en `crates/wow-map/src/grid_unload.rs`; concrete `Creature::CombatStop`, dynobject/areatrigger cleanup, respawn relocation, `CleanupsBeforeDelete` and deletion effects remain tied to `#023` canonical entities.
- [x] **#012** `wow-map`: terrain hooks from `Map::EnsureGridCreated`: grid coordinate flip `(63 - x, 63 - y)` and `TerrainMgr::LoadMapAndVMap`; keep actual vmap/mmaps loading behind a trait if assets are not ready. Cerrado con `TerrainGridLoader`.
- [x] **#013** `wow-map`: tests integration: spawn store → `EnsureGridLoaded` → `ObjectGridLoader::load_n`; verify cell-level placement, grid state transitions and no grid-size regression. Cerrado con `SpawnGridLifecycle` y tests de `Map::ensure_grid_loaded`.
- [x] **#014** `wow-map`: `MapManager` structural skeleton from `MapManager.h/.cpp`: ordered `i_maps`, `create_world_map`/`create_map_entry`, `find_map`, `do_for_all_maps`, `do_for_all_maps_with_map_id`, serial `update`, `destroy_map`, instance id allocation/free, scheduled script counter. Cerrado en `crates/wow-map/src/manager.rs`; `CreateMap(Player*)` branching for BG/dungeon/group/instance locks remains pending until those types exist.
- [ ] **#014a** `wow-map`/`wow-world`: bind `MapManager::CreateMap(uint32, Player*)` decision tree against real `Player`, `Group`, `InstanceLockMgr`, `Battleground`, `MapEntry`/DB2 difficulty data and recent instance tracking.
- [x] **#015** `wow-map`: `MapUpdater` API/fallback from `MapUpdater.cpp`: `activate`, `deactivate`, `activated`, `schedule_update`, `wait`; wired into `MapManager::update`. Cerrado como inline deterministic fallback in `crates/wow-map/src/manager.rs`.
- [ ] **#015a** `wow-map`: real `MapUpdater` worker pool equivalent to C++ `ProducerConsumerQueue<MapUpdateRequest*>` + worker threads, if/when maps become independently mutable/sendable enough to update safely in parallel.
- [x] **#016** `world-server/main.rs`: arrancar `MapManager` global + update loop. Cerrado con `wow_map::MapManager` canónico inicializado desde `GridCleanUpDelay`, `MapUpdateInterval` y `MapUpdate.Threads`, y task global que llama `MapManager::update(diff)` como `World::Update -> sMapMgr->Update(diff)` en C++.
- [ ] **#016a** `wow-world`/`world-server`: eliminar los ticks de mundo session-local como fuente de verdad (`WorldSession::tick_creatures_sync`, `tick_combat_sync`, visibilidad/aura ligada a entidades) cuando existan entidades canónicas y `ObjectAccessor`; no cerrar como port completo hasta que esos ticks pasen por Map/Entity.
- [ ] **#017** Limpiar `crates/wow-world/src/map_manager.rs`: reemplazar implementación legacy por el nuevo `wow-map`; retener tests útiles solo si siguen contrastados contra C++.
- [ ] **#018** Migrar `handlers/loot.rs` a lookups de criatura/GO vía Map/ObjectAccessor equivalente, no `self.creatures`.
- [ ] **#019** Migrar `handlers/combat.rs` y `session.rs::tick_combat_sync` al Map/Entity model.
- [ ] **#020** Migrar `handlers/trainer.rs`, `handlers/misc.rs` y query/use GO al Map/Entity model.
- [ ] **#021** Migrar `session.rs::tick_creatures_sync`, `send_nearby_creatures` y `handlers/character.rs::update_creature_visibility` a visitors/cell queries del Map.
- [ ] **#022** Quitar campos legacy `creatures`/`visible_creatures` de `WorldSession`; borrar `_attic/` solo cuando sus tests/avisos útiles estén integrados o descartados explícitamente.

### Inmediato siguiente (Fase 1 — Entidades canónicas)

- [x] **#023** `wow-entities`: crate/module boundary and base `Object` from `Entities/Object/Object.*`: guid, type id, map id, entry, update flags, in-world/grid state. Cerrado con crate `wow-entities` y `EntityObject` base contrastado contra `Object.h`, `Object.cpp`, `ObjectGuid.h`; `map_id`/`in_grid` quedan como bridge Rust explícito para ownership canónico de mapas.
- [x] **#023a** `wow-entities`/`wow-map`: bind `grid_unload` actions to real entity methods: `Creature::RemoveAllDynObjects`, `Creature::RemoveAllAreaTriggers`, `Creature::CombatStop`, creature/GO respawn relocation, `SetDestroyedObject`, `CleanupsBeforeDelete`, and object deletion. Cerrado contra `ObjectGridLoader.cpp`: stoper/evacuator/cleaner/unloader order is preserved; Creature-owned dynamic objects and area triggers drain like C++ `Unit::RemoveAll*`; all C++ grid-cleaned object kinds (`Creature`, `GameObject`, `DynamicObject`, `Corpse`, `AreaTrigger`, `SceneObject`, `Conversation`) now apply represented cleanup/delete state.
- [ ] **#023z** Smoke-test follow-ups from the 2026-05-11 live-client probe: document or quiet non-blocking `TactKey.db2` `DbQueryBulk` misses after contrasting C++ `sTactKeyStore`/hotfix behavior with local extracted DB2 rows; implement `SMSG_RATED_PVP_INFO` when `CMSG_REQUEST_RATED_PVP_INFO` is received in `LoggedIn` state, keeping the C++ `STATUS_LOGGEDIN` rejection before login; keep external invalid connection/reset logs as operational noise unless a C++-parity issue is reproduced.
- [x] **#024** `wow-entities`: `WorldObject` from `Entities/Object/WorldObject.*`: position/orientation, current cell, map pointer/key, phase shift, distance/facing helpers. Cerrado como base `WorldObject`/`WorldLocation`: posición con orientación normalizada, map/instance binding, current-cell bridge, phase-shift mínimo y helpers de distancia/rango contrastados contra `Position.h` y `Object.cpp`; helpers puros de ángulo/arc/line/box cerrados en `#024a`; LOS, terreno, transportes y visibility ranges quedan en subtareas posteriores.
- [x] **#024a** `wow-entities`: pure `Position`/`WorldObject` geometry helpers from `Position.h`, `Position.cpp` and `Object.cpp`: absolute/relative angle conversion, `HasInArc`, `isInFront`, `isInBack`, `HasInLine`, rotated box and double vertical cylinder checks; no LOS/terrain/Map behavior is faked here.
- [x] **#025** `wow-world`/`wow-entities`: `ObjectAccessor` equivalent from `Globals/ObjectAccessor.*`: global player lookup plus map-local object lookup APIs for Creature/GO/Corpse/DynamicObject/AreaTrigger/SceneObject/Conversation/Pet. Cerrado como API base en `wow-entities::ObjectAccessor`: player global by GUID/name, connected vs in-world lookup, same-map player lookup, map-local dispatch by GUID high type and `TypeMask`, incluyendo la rama C++ de corpse/null en `GetObjectByTypeMask`.
- [x] **#025a** `wow-entities`/`wow-map`: conectar `ObjectAccessor` al `wow_map::Map` canónico en vez de mantener un store bridge interno. Cerrado contra `ObjectAccessor.cpp`: los objetos map-locales se resuelven mediante `ObjectAccessorMapSource`/`Map`, `ObjectAccessor` conserva sólo el registro global de players y los helpers map-locales sin source quedan deprecados para no ocultar el requisito de map canónico.
- [ ] **#025b** `wow-entities`/`wow-world`: `ObjectAccessor::SaveAllPlayers()` con persistencia real equivalente a C++ `Player::SaveToDB()` para cada player registrado. El shape está en `save_all_players_with`, pero el cierre completo depende de `Player` runtime/persistencia canónica (`#028`).
- [ ] **#026** `wow-packet`/`wow-entities`: Update fields delta from `Entities/Object/Updates/` and `UpdateFields.h`; stop relying on full re-create as normal update path. Refinado: base `UpdateMask` + writer VALUES de `ObjectData`, `DynamicObjectData`, `SceneObjectData`, `ConversationData`, `GameObjectData`, `CorpseData`, `AreaTriggerData`, `ItemData`, `ContainerData`, `UnitData`, `PlayerData` y `ActivePlayerData` cerrados; bridge `wow-entities` -> `wow-packet` iniciado para `PlayerData`/`ActivePlayerData`; sigue pendiente integrar callsites y cubrir los demás tipos/Unit/Object sin gaps.
- [x] **#026a** `wow-entities`/`wow-packet`: foundation for update-field deltas. Cerrado con `wow_entities::UpdateMask`, `EntityObject::values_update()`, writer `UpdateObject::object_values_update`, y corrección contrastada de `CreatureHealthUpdate` VALUES para no escribir byte `UpdateFieldFlag` de create.
- [x] **#026b** `wow-packet`: `UF::DynamicObjectData::WriteUpdate` VALUES serializer. Cerrado contra `UpdateFields.cpp`: escribe máscara de 7 bits, flush y campos en orden C++ `Caster`, `Type`, `SpellXSpellVisualID`, `SpellID`, `Radius`, `CastTime`, con bloque `UpdateObject::dynamic_object_values_update`.
- [x] **#026c** `wow-packet`: `UF::SceneObjectData::WriteUpdate` VALUES serializer. Cerrado contra `UpdateFields.cpp`: escribe máscara de 5 bits, flush y campos en orden C++ `ScriptPackageID`, `RndSeedVal`, `CreatedBy`, `SceneType`, con bloque `UpdateObject::scene_object_values_update`.
- [x] **#026e** `wow-packet`: `UF::ConversationData::WriteUpdate` VALUES serializer contra `UpdateFields.cpp`, incluyendo `Lines`, `Actors` `DynamicUpdateField` masks y `LastLineEndTime`. Cerrado con bloque `UpdateObject::conversation_values_update`: máscara de 4 bits, tamaño/serialización de `Lines`, máscara dinámica explícita o completa de `Actors`, escritura solo de actores marcados y `LastLineEndTime` en orden C++.
- [x] **#026d** `wow-packet`: serializers VALUES con arrays/nested para `CorpseData`, `GameObjectData` y `AreaTriggerData`. Cerrado contra `UpdateFields.cpp`: `GameObjectData` cubre `StateWorldEffectIDs`, `EnableDoodadSets`, `WorldEffects` y campos escalares en orden C++; `CorpseData` cubre `Customizations`, campos base y `Items[19]`; `AreaTriggerData` cubre `ScaleCurve`, `VisualAnim`, GUIDs y escalares con orden nested C++.
- [x] **#026f** `wow-packet`: serializers VALUES completos para `UF::ItemData` y `UF::ContainerData`. Cerrado contra `UpdateFields.cpp`/`ItemPacketsCommon.cpp`: máscaras de bloques de 2 bits, `ArtifactPowers`, `Gems`, `ItemModList`, `ItemBonusKey`, `SpellCharges`, `Enchantment[13]`, `NumSlots` y `Slots[36]` en orden C++.
- [x] **#026g** `wow-packet`: serializer VALUES completo para `UF::UnitData::WriteUpdate`. Cerrado contra `UpdateFields.cpp`: máscaras de 8 bloques, dinámicos `StateWorldEffectIDs`/`PassiveSpells`/`WorldEffects`/`ChannelObjects`, `UnitChannel`, campos escalares, GUIDs, `VirtualItems`, `NpcFlags`, power/regen, stats, resistencias y costes se escriben en orden C++.
- [x] **#026h** `wow-packet`: serializer VALUES completo para `UF::PlayerData::WriteUpdate`. Cerrado contra `UpdateFields.cpp` y `MythicPlusPacketsCommon.cpp`: máscaras de 4 bloques, bit `IsQuestLogChangesMaskSkipped=false`, `Customizations`, `ArenaCooldowns`, `VisualItemReplacements`, campos escalares/GUIDs, `DungeonScoreSummary`, `PartyType`, `QuestLog`, `VisibleItems`, `AvgItemLevel` y `Field_3120` en orden C++.
- [x] **#026i** `wow-packet`: serializer VALUES completo para `UF::ActivePlayerData::WriteUpdate`, incluyendo máscaras de 48 bloques, `SkillInfo`, inventario/buyback, dinámicos de quests/titles/toys/transmog/traits, research y PVP info. Cerrado en `#026i5`; queda fuera de este ítem el bridge `wow-entities` para poblar/emitir los deltas reales.
- [x] **#026i1** `wow-packet`: `UF::ActivePlayerData::WriteUpdate` runtime/common path contrastado contra `UpdateFields.cpp`: cabecera de 48 bloques (`group0` u32 + `group1` 16 bits), `Coinage`, `InvSlots[141]`, `BuybackPrice/BuybackTimestamp`, parent 0 expertise, parent 38 stats, `SpellCritPercentage`/`ModDamageDonePos` y `CombatRatings` en orden C++.
- [x] **#026i2** `wow-packet`: nested `UF::SkillInfo::WriteUpdate` contrastado contra `UpdateFields.cpp`: máscara de 57 bloques (`group0` u32 + `group1` 25 bits), bloques activos y arrays `SkillLineID`, `SkillStep`, `SkillRank`, `SkillStartingRank`, `SkillMaxRank`, `SkillTempBonus`, `SkillPermBonus` en el loop C++ de 256 entradas.
- [x] **#026i3** `wow-packet`: nested simple writers de `ActivePlayerData` contrastados contra `UpdateFields.cpp`: `UF::Research::WriteUpdate`, `UF::RestInfo::WriteUpdate` y `UF::PVPInfo::WriteUpdate` con máscaras/flushes/orden de escalares C++.
- [x] **#026i4** `wow-packet`: nested dynamic writers de `ActivePlayerData` contrastados contra `UpdateFields.cpp`: `CharacterRestriction`, `SpellPctModByLabel`, `SpellFlatModByLabel`, `CategoryCooldownMod`, `WeeklySpellUse`, `CompletedProject`, `ResearchHistory`, `TraitEntry`, `TraitConfig`, `StablePetInfo` y `StableInfo` con máscaras dinámicas, strings y condicionales por `Type` en orden C++.
- [x] **#026i5** `wow-packet`: writer completo `ActivePlayerDataValuesUpdate` + `UpdateObject::full_active_player_values_update` contrastado contra `UF::ActivePlayerData::WriteUpdate`: orden global C++ de masks, dos fases de dynamic masks, parent-102/PetStable bit, arrays tardíos (`QuestCompleted`, glyphs) y `PvpInfo` final. Corregida divergencia detectada en writer runtime: `Coinage` ahora activa también parent bit `0` como C++.
- [x] **#026j** `wow-world`/`wow-packet`: bridge inicial `PlayerValuesUpdate` -> VALUES packet para `PlayerData` y `ActivePlayerData`. Cerrado contra el modelo C++ de bloque VALUES combinado por `changedObjectTypeMask`: `wow-packet` permite anexar `ActivePlayerData` dentro del bloque Player, y `wow-world::entity_update_bridge` copia máscaras/valores desde `wow-entities` sin añadir dependencia inversa. Queda pendiente `#026k`: usar este bridge en los callsites runtime y añadir Object/Unit/otros tipos.
- [x] **#026k** `wow-entities`/`wow-world`: integrar el bridge en callsites runtime de inventario/dinero/buyback sin perder `UnitData::VirtualItems`. Cerrado con `WorldSession::send_player_values_update_from_entity_bridge`: construye snapshot entity-side, aplica cambios/markers, emite `PlayerValuesUpdate` por el bridge, y reemplaza los callsites runtime que usaban `player_values_update`, `player_money_update` y `player_values_buyback_update`.
- [x] **#026l** `wow-world`: extender `entity_update_bridge` para traducir deltas entity-side de `Object`, `Unit`, `Item` y `Bag/Container` hacia los serializers de `wow-packet`. Contrastado contra los `BuildValuesUpdate*` C++ de `Object.cpp`, `Player.cpp`, `Item.cpp` y `Bag.cpp`: `ObjectData` viaja como prefijo opcional dentro de Unit/Item/Container/Player, y el `changedObjectTypeMask` preserva combinaciones Object+Unit, Object+Item y Object+Container. Queda pendiente conectar callsites runtime no-player y ampliar el bridge a GameObject/Corpse/DynamicObject/AreaTrigger/SceneObject/Conversation.
- [x] **#026m** `wow-world`: completar bridge entity-side para `GameObject`, `Corpse`, `DynamicObject`, `AreaTrigger`, `SceneObject` y `Conversation`. Contrastado contra `UpdateFields.cpp`/`BuildValuesUpdate*`: cada tipo conserva `ObjectData` opcional como prefijo y copia máscaras/campos en el orden C++; para nested simples que `wow-entities` aún no desglosa (`ScaleCurve`, `VisualAnim`, actores) el bridge emite máscara completa cuando el padre cambia, equivalente al path C++ con `ignoreNestedChangesMask=true`. Queda pendiente conectar callsites runtime no-player.
- [ ] **#027** `wow-entities`: `Unit` from `Entities/Unit/`: health, power, faction, flags, aura hooks, threat hooks. Refinado: base `Unit` state/setters cerrado en `#027a`, current spell slots en `#027b`, `SpellHistory` en `#027c`, `MotionMaster` en `#027d`, charm/control en `#027e`, threat/combat en `#027f`, aura hooks en `#027g` y movespline/AI bridge en `#027h`; los gaps restantes son de integración runtime con Spell/Aura, Movement, AI, Map y packets, no de estado estructural `wow-entities`.
- [x] **#027a** `wow-entities`: base `Unit` state from `Unit.*` and `UF::UnitData`: constructor state, movement update flag, death/unit state, health/max-health clamps, power index bridge, display/level/faction/reach fields and UnitData masks.
- [x] **#027b** `wow-entities`: `Unit` current spell slot lifecycle from `Unit.h`/`Unit.cpp`: `CURRENT_*` slot ids, `m_currentSpells` bridge, `SetCurrentCastSpell`, `InterruptSpell`, `FinishSpell`, `InterruptNonMeleeSpells`, `FindCurrentSpellBySpellId`, Auto Shot id `75`, delayed/instant/interruptible guards and channel `ALLOW_ACTIONS_DURING_CHANNEL` breakage. Full Spell cancel/finish packet side effects remain under the later Spell pipeline port, but the Unit-side ownership and interruption semantics are now represented and tested.
- [x] **#027c** `wow-entities`: `SpellHistory` structural state from `Spells/SpellHistory.*` and `Unit::_spellHistory`: spell/category cooldown entries, on-hold month marker, category index, update expiry, cooldown modification/reset, charge-category recharge queues, school lockouts, global cooldowns and before/after duel snapshot restoration. DB2/aura-derived duration calculation, packet emission and DB persistence remain explicit gaps for the later Spell pipeline and persistence passes.
- [x] **#027d** `wow-entities`: `MotionMaster` structural state from `MovementDefines.h`, `MovementGenerator.h` and `MotionMaster.*`: Trinity movement generator ids, default/active slots, mode/priority ordering, generator flags, target/id/duration metadata, base unit state reference counts, `MoveIdle`/point/follow/charge helpers, active clear/remove and `StopOnDeath` persist-on-death handling. Real path generation, spline launch callbacks, movement informs and map collision remain under the later movement runtime pass.
- [x] **#027e** `wow-entities`: `Unit` charm/control ownership state from `Unit.h`, `Unit.cpp`, `CharmInfo.h` and `SharedDefines.h`: owner/minion GUID bridge, `m_SummonSlot[MAX_SUMMON_SLOT]`, pet/totem summon slot constants, charmer/charmed/controlled GUID sets, `m_ControlledByPlayer`, `CharmType`, `CharmInfo` action-bar/charm-spell state, old faction/walk snapshot, direct mover pointers, shared vision helpers, `SetCharm` controller-side removal predicate and `SetCharmedBy`/`RemoveCharmedBy` target-side guard/state transitions. Aura packets, faction application, MotionMaster mutations, AI hooks and player possession packets remain explicit runtime gaps for the later spell/aura/movement/AI integration passes.
- [x] **#027f** `wow-entities`: `ThreatManager`/`CombatManager` structural state from `Combat/ThreatManager.*`, `Combat/CombatManager.*` and `Unit.*`: threat refs with base/temp amount, online/suppressed/offline and taunt/detaunt ordering, current/fixate victim selection with C++ 110%/130% shape, reverse `threatenedByMe` refs, threat update interval, PvE/PvP combat refs, PvP 5s timeout, suppression and all-combat clear helpers. Real `CanBeginCombat` world/map/phase/faction checks, aura-derived suppression/modifiers, redirect registry, packet emission, AI callbacks and bidirectional map-owned ref wiring remain under later combat/runtime integration passes.
- [x] **#027g** `wow-entities`: `Unit` aura hook state from `Unit.h`, `Unit.cpp`, `SharedDefines.h` and `SpellDefines.h`: owned/applied/visible/removed aura containers, visible update queue, interruptible aura list with interrupt masks, aura-state application including per-caster mask behavior, proc depth/chain counters and diminishing-return group/level/18s reset state. Real `Aura`/`AuraApplication` object lifetimes, `SpellInfo` stack/exclusive rules, aura effect handlers, criteria updates, interrupt-ignore rules, channel interruption, packet emission and spell script hooks remain explicit gaps for the later Spell/Aura pipeline.
- [x] **#027h** `wow-entities`: `Unit` movespline/AI bridge state from `Unit.*`, `MoveSpline.*`, `MoveSplineInit.*` and Unit AI stack helpers: movespline finalized/cyclic/on-transport flags, progress/duration/id, destination/velocity metadata, update/finalize/interrupt helpers, AI active/stack lock semantics, update tick bookkeeping and scheduled-change bridge around charm transitions. Real spline math/path generation, movement packet sync, `MovementInfo` flags, `UnitAI` trait object ownership and callbacks remain under Movement/AI runtime phases.
- [ ] **#028** `wow-entities`: `Player` from `Entities/Player/`: account/session link, inventory refs, quests, skills, taxi state. Refinado: base `Player` state/setters cerrado en `#028a`, base `Item` state cerrado en `#028b`, base `Bag` state cerrado en `#028c`, storage lookup cerrado en `#028d`, ObjectAccessor item branch cerrado en `#028e`, visible item state cerrado en `#028f`, InventoryType bridge cerrado en `#028g` y visible modifier helpers cerrado en `#028h`; siguen pendientes create/load/login, inventario real ownership, DB2 resolver stores, binding/equipment side effects, quests, skills, taxi, social, mail, group/guild, battleground and persistence.
- [x] **#028a** `wow-entities`: base `Player` state from `Player.*`, `StatSystem.cpp::Player::GetPowerIndex` and `UF::{PlayerData,ActivePlayerData}`: constructor type id/mask, session bridge, hit chance defaults, whisper accept permission branch, race/class/gender/native gender setters, selection target, flags, loot GUID, bank/backpack counts, XP, money clamp and PlayerData/ActivePlayerData masks.
- [x] **#028b** `wow-entities`: base `Item` state from `Item.*`, `ItemTemplate.h`, `ItemDefines.h`, `UF::ItemData` and `ItemPacketsCommon::ItemBonusKey`: constructor type id/mask, Object-only shape, create-state bridge, owner/contained/creator fields, slot/container/update/refund/text/trade state, dynamic item flags/flags2, stack/durability/expiration/context/appearance, spell charges, enchantments, item bonus key and ItemData masks.
- [x] **#028c** `wow-entities`: base `Bag` state from `Item/Container/Bag.*` and `UF::ContainerData`: `Item` retag to `TYPEID_CONTAINER`/`TYPEMASK_CONTAINER`, `MAX_BAG_SIZE`, bag slot GUID bridge, `NumSlots`, `Slots[36]`, template slot-count guard, StoreItem/RemoveItem child state updates and ContainerData masks.
- [x] **#028d** `wow-entities`: `Player` storage lookup bridge from `Player.h` and `Player.cpp`: `m_items[141]`, slot constants, `ForEachItem` locations, `GetItemByPos`, packed-pos lookup, `GetBagByPos`, `GetItemByGuid`, buyback slot rotation and `ActivePlayerData` InvSlots/Buyback masks; `PlayerStorage.cpp` is not present in this C++ checkout, storage lives in `Player.cpp`.
- [x] **#028e** `wow-entities`: `ObjectAccessor::GetObjectByTypeMask` item branch from `ObjectAccessor.cpp`: `TYPEMASK_ITEM` only resolves for player context and delegates to player inventory lookup; Rust exposes `AccessorObjectRef::Item` because `Item` is not a `WorldObject`.
- [x] **#028f** `wow-entities`: `Player` visible item slot state from `Player.cpp`, `Player.h` and `UF::{PlayerData,VisibleItem}`: `VisibleItems[19]`, `ItemID`, `ItemAppearanceModID`, `ItemVisual`, PlayerData array bits `61` and `62..80`, `SetVisibleItemSlot` clear/set behavior and equipment-slot `VisualizeItem` bridge. Template-derived item display values, BoE/BoA binding, real ownership side effects and nested packet serializers remain under `#028`/`#026`.
- [x] **#028g** `wow-data`/`wow-packet`/`wow-world`: InventoryType bridge from `DB2Structure.h::ItemEntry`, `ItemTemplate.h::InventoryType` and `Player.h` slot ranges: signed `int8 InventoryType` no longer wraps negative values to `255`, `INVTYPE_NON_EQUIP=0` maps to no slot, and `INVTYPE_BAG=18` maps to equipped bag slots `30..33`.
- [x] **#028h** `wow-entities`: `Item` visible modifier helpers from `Item.cpp`, `Item.h` and `ItemDefines.h`: item modifier storage, `AppearanceModifierSlotBySpec`, `IllusionModifierSlotBySpec`, `SecondaryAppearanceModifierSlotBySpec`, `GetVisibleEntry`, `GetVisibleAppearanceModId`, `GetVisibleEnchantmentId`, `GetVisibleItemVisual` and secondary appearance precedence. DB2 resolver stores remain explicit bridges until `wow-data` ports `ItemModifiedAppearance` and `SpellItemEnchantment`.
- [ ] **#029** `wow-entities`: `Creature` + `GameObject` from their C++ dirs: template refs, spawn data, respawn timer, AI ref, GO state. Refinado: base `Creature` state cerrado en `#029a` y base `GameObject` state cerrado en `#029b`; siguen pendientes `Creature::Create/LoadFromDB`, template/difficulty refs, AI ownership, loot, corpse/respawn lifecycle and GameObject create/template/model/use lifecycle.
- [x] **#029a** `wow-entities`: base `Creature` state from `Creature.*`, `CreatureData.h`, `UnitDefines.h`, `MovementDefines.h`, `SharedDefines.h`, `World.cpp` config defaults and `StatSystem.cpp::Creature::GetPowerIndex`: constructor defaults, respawn/corpse timers, react state, movement type, spells, loot mode, monster sight default, display/model dimension bridge, faction setter and creature power-index semantics.
- [x] **#029b** `wow-entities`: base `GameObject` state from `GameObject.*`, `SharedDefines.h` and `UF::GameObjectData`: constructor type id/mask, stationary/rotation create flags, respawn/despawn/restock/cooldown state, loot state/unit guid, spawned-by-default, spell/spawn ids, packed rotation, loot mode, stationary position, respawn compatibility flag and GameObjectData setters/masks.
- [x] **#029c** `wow-entities`: base `Corpse` state from `Corpse.*`, `SharedDefines.h` and `UF::CorpseData`: constructor type id/mask, `WorldObject(type != CORPSE_BONES)`, stationary flag, ghost time/type/cell bridge, dynamic flags, owner/party/guild, display/race/class/sex/flags/faction/item setters, corpse expiry thresholds and CorpseData masks.
- [ ] **#030** `wow-entities`: remaining map-stored object types: `DynamicObject`, `AreaTrigger`, `Pet`, `Transport`, `Vehicle`, `SceneObject`, `Conversation`, `Totem`; mark post-WoLK-only behavior explicitly when C++ has stubs. Refinado: base `DynamicObject` state cerrado en `#030a`, base `AreaTrigger` state cerrado en `#030b`, base `SceneObject` state cerrado en `#030c`, base `Conversation` state cerrado en `#030d`, base `Totem` state cerrado en `#030e`, base `Pet` state cerrado en `#030f`, base `Vehicle` kit cerrado en `#030g` y base `Transport` state cerrado en `#030h`; siguen pendientes DynamicObject create/add-to-map/update runtime, AreaTrigger templates/create/load/update/search/actions/AI, SceneObject create/map/aura removal, ConversationDataStore/Start/runtime, TempSummon/Minion/Pet runtime, Vehicle auras/events/accessories/immunities, TransportMgr/path runtime/static passenger spawning/teleport/update integration, Aura/Spell ownership, caster registration, farsight viewpoint and transport/map relocation.
- [x] **#030a** `wow-entities`: base `DynamicObject` state from `DynamicObject.*` and `UF::DynamicObjectData`: constructor type id/mask, `WorldObject(isWorldObject)`, stationary flag, duration/aura/caster/viewpoint bridge state, dynamic-object type enum, caster/spell visual/spell id/radius/cast-time setters and DynamicObjectData masks.
- [x] **#030b** `wow-entities`: base `AreaTrigger` state from `AreaTrigger.*`, `AreaTriggerTemplate.h` and `UF::AreaTriggerData`: constructor type id/mask, `WorldObject(false)`, stationary/area-trigger create flags, spawn/target/aura/stationary-position/duration/time/removal/movement/template bridge state, duration semantics for permanent triggers, scalar AreaTriggerData setters, basic scale-curve constants and VisualAnim mask bridge.
- [x] **#030c** `wow-entities`: base `SceneObject` state from `SceneObject.*` and `UF::SceneObjectData`: constructor type id/mask, `WorldObject(false)`, stationary/scene-object create flags, stationary position, created-by-spell-cast bridge, removal predicate shape and SceneObjectData script package/random seed/created-by/type masks.
- [x] **#030d** `wow-entities`: base `Conversation` state from `Conversation.*`, `ConversationDataStore.h` and `UF::ConversationData`: constructor type id/mask, `WorldObject(false)`, stationary/conversation create flags, creator/duration/texture/stationary-position state, line start/end time bridges, actor/line data, 10s despawn delay and ConversationData masks.
- [x] **#030e** `wow-entities`: base `Totem` state from `Totem.*`, `TemporarySummon.*`, `Unit.h` and `SharedDefines.h`: `Creature/Minion` type shape, `UNIT_MASK_SUMMON|MINION|TOTEM`, owner/summoner bridge, totem duration/type, inherited spell slots, init-summon passive/secondary spell rules, update/unsummon duration shape, totem-created packet slot offset and immunity predicate special cases.
- [x] **#030f** `wow-entities`: base `Pet` state from `Pet.*`, `PetDefines.h`, `TemporarySummon.*`, `UnitDefines.h` and `CreatureData.h`: `Guardian`/`Creature` world-object shape, unit type masks including hunter/controlable guardian, owner/type/duration/loading/removed/focus/group/specialization state, pet spell/autospell maps, stable slot helpers/load selection priority and pet XP factor.
- [x] **#030g** `wow-entities`: base `Vehicle` kit from `Vehicle.*` and `VehicleDefines.h`: base unit GUID/type/position bridge, vehicle id/creature entry, status machine, seats/passenger info/addons/accessories/template structs, usable/available seat counting, pending join-event bridge, passenger add/remove/remove-all and `TransportBase` passenger position/offset formulas.
- [x] **#030h** `wow-entities`: base `Transport` state from `Transport.*`, `TransportMgr.h`, `GameObject.*` and `SharedDefines.h`: `GameObject` shape with `SERVER_TIME|STATIONARY|ROTATION`, map-object-transport type, movement state, path leg/event/template structs, period/timer/path-progress client dynamic-flag encoding, dynamic/static passenger GUID sets, cleanup/unload shape, movement stop request bridge and `TransportBase` passenger position/offset formulas.
- [x] **#031** `wow-world`/`wow-entities`: mover el runtime de criaturas usado por sesión a `WorldCreature`/`Creature` map-owned, contrastado contra C++ `Creature::Update`, `Creature::AIM_Create/AIM_Initialize` y `Unit::AIUpdateTick`. `WorldSession` deja de usar `HashMap<Guid, wow_ai::CreatureAI>` en runtime; combate, aggro, loot/gossip/vendor/taxi, respawn/corpse y visibilidad mutan o consultan el `MapManager`. Metadatos de loot/boss que estaban en `CreatureAI` pasan a `CreatureAiOwnershipState`. Queda solo un puente `cfg(test)` para tests legacy hasta limpiar fixtures.
- [x] **#032** Refactor `WorldSession` para tener player entity handle/controlador en vez de campos sueltos.
  - [x] **#032a** Crear `SessionPlayerController` como equivalente incremental de C++ `WorldSession::_player`/`SetPlayer`/`GetPlayer`: GUID, nombre, mapa, posición, raza, clase, nivel y sexo quedan sincronizados desde login/movement/teleport/logout, y los registros compartidos (`PlayerRegistry`/`ObjectAccessor`) leen por el controlador cuando existe.
  - [x] **#032b** Migrar runtime de jugador al controlador/player entity.
    - [x] **#032b1** Dinero, XP/NextLevelXP, selección, spells y `_currencyStorage` quedan reflejados en `SessionPlayerController` con getters/setters tipo C++ `Player::{GetMoney,SetMoney,GetXP,SetXP,SetSelection,GetSpellMap,_currencyStorage}`; login/trainer/loot/quest/registry usan la ruta canónica donde ya aplica.
    - [x] **#032b2** Migrar `m_items`/buyback/inventory item objects al controlador/player entity sin depender de `WorldSession` como owner de inventario.
      - [x] **#032b2a** Crear `SessionPlayerInventoryRuntime` dentro del controlador y alimentar desde login/logout/clear; snapshots de valores, `PlayerRegistry`, `ObjectAccessor`, `GetItemByPos` representado y planes directos de store/unequip leen por esta ruta canónica cuando existe.
      - [x] **#032b2b** Cambiar mutaciones directas de `character`/`loot`/`spell` sobre `inventory_items`, `buyback_items` e `inventory_item_objects` a mutadores canónicos sincronizados con `SessionPlayerInventoryRuntime`, contrastado contra C++ `Player::_StoreItem`, `QuickEquipItem`, `AddItemToBuyBackSlot` y `RemoveItemFromBuyBackSlot`.
      - [x] **#032b2c** Invertir el owner efectivo: los mutadores operan sobre `SessionPlayerInventoryRuntime` cuando existe controlador y los campos heredados de `WorldSession` quedan como espejo/compatibilidad hasta su retirada en `#032d`.
  - [x] **#032c** Cambiar handlers directos (`character`, `loot`, `trainer`, `spell`, `quest`, `chat`, `group`) a getters/mutators del controlador, contrastando cada bloque contra `Player`/`WorldSession` C++.
    - [x] **#032c1** `chat`/`group`: reemplazar lecturas directas de GUID/nombre/mapa/posición por getters del controlador, equivalente a C++ `WorldSession::GetPlayer()->GetGUID()/GetName()/GetMapId()/GetPosition()`.
    - [x] **#032c2** `quest`/`spell`: reemplazar raza/clase/nivel/género/spells/inventario por getters canónicos y cubrir condiciones/quest availability.
    - [x] **#032c3** `loot`: reemplazar GUID/mapa/posición/nivel/clase/inventario por getters canónicos en roll/master-loot/store paths.
    - [x] **#032c4** `character`/`trainer`: reemplazar gold/currency/player metadata/inventory directos por mutadores/getters canónicos y cerrar residuos de login/logout.
  - [x] **#032d** Retirar o hacer privados los campos heredados cuando dejen de ser fuente runtime, dejando solo puentes `cfg(test)` si aún son necesarios.
    - [x] **#032d1** Cerrar residuos fuera de la lista original (`combat`, `social`, `misc`, `spell`, `loot`) que todavía leían GUID/posición/item runtime directamente en vez de C++ `GetPlayer()`/`GetItemByGuid()`.
    - [x] **#032d2** Encapsular accesos internos de `session.rs` que todavía usan campos legacy como fallback/runtime y separar claramente bootstrap/test de runtime canónico.
    - [x] **#032d3** Reducir visibilidad de campos heredados o moverlos detrás de helpers `cfg(test)` cuando ya no haya consumidores productivos directos: los campos legacy de player runtime quedan privados en `WorldSession`; login/bootstrap y loot spec pasan por helpers C++-like, y los tests de handlers ya no escriben/leen esos campos directamente.

> Tras cerrar #032, el roadmap continúa con Fase 2 (Movement) y siguientes según la sección 4.

---

## 6. Criterios de "done" por fase

Una fase se considera cerrada cuando:

1. **Todos los TODO de la fase marcados `[x]`**.
2. **`cargo check --workspace` 0 errores**, sin warnings nuevos.
3. **`cargo test --workspace` todos los tests verdes**, incluyendo nuevos tests de la fase.
4. **Tests de regresión runtime**: el server arranca, login OK, un personaje entra al mundo y puede moverse + combatir + alguna mecánica de la fase recién implementada.
5. **Documentación actualizada**: este `MIGRATION_ROADMAP.md` con la sección 3 (matriz) actualizada al nuevo % migrado, y `CLAUDE.md` con cualquier nueva convención.
6. **Sin `// TODO` ni `unimplemented!()` ni `todo!()` en el código de la fase** (excepto claramente marcados como pendientes de la siguiente fase).
7. **Commit limpio en `main`** (no en rama feature, dado que trabajamos en solitario — ver ADR sobre solo-developer workflow).

---

## 7. Riesgos y mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|---|---|---|---|
| Re-introducir el bug del bridge fallido (improvisar contra structs imaginarios) | Media | Alto | Memory `feedback_always_read_cpp.md`. Antes de cada implementación, leer el `.cpp` correspondiente. Citar línea en commit. |
| Confiar en docs/agentes previos como si fueran C++ | Alta | Alto | Los docs son índice, no oracle. Cada task requiere contraste directo con C++ y, si toca wire/runtime, test específico. |
| **Lo "✅ done" actual tiene bugs/divergencias vs C++ que no hemos detectado** | Alta | Alto | Fase A (auditoría obligatoria por módulo) antes de extender. Tabla de divergencias en `docs/audits/<modulo>.md`. Hasta que un módulo no esté auditado, su columna "Auditado vs C++" sigue ❌ y se trata con sospecha. |
| Auditar todo costaría tanto como reescribirlo | Media | Medio | Las auditorías se priorizan: módulos críticos (network, crypto, packets, maps) primero; los de menor superficie y baja prioridad pueden auditarse "just-in-time" antes de extender. |
| Scope creep entre fases (querer hacer L5 antes de L3 estable) | Alta | Alto | Esta hoja de ruta es vinculante. No se salta orden sin acuerdo explícito. |
| Implementación parcial que parezca completa (ej. spell engine que solo cubre 5 efectos) | Media | Medio | Tests por feature concreta. Marcar ⚠️ en lugar de ✅ hasta cobertura ≥ 90%. |
| Acoplamiento accidental entre crates (wow-map dependiendo de wow-world) | Baja | Alto | Disciplina de capas. wow-map no conoce sesiones, solo entidades. |
| Pathfinding (Detour) incompleto bloquea AI | Media | Medio | Hacer movement waypoint sin pathfinding primero; Detour es Fase 2.2. |
| `scripts/` (3000 archivos) bloquea cualquier contenido scripteado | Alta | Alto | Aceptar que la mayoría de bosses/instancias no funcionan hasta Fase 8. Priorizar SmartAI (data-driven) que cubre ~50% sin scripting. |
| Performance: `Arc<RwLock<MapManager>>` global serializa todo | Alta | Alto | Resolver en Fase 0.6 (MapUpdater pool). Si no resuelve, considerar one-Arc-per-Map en lugar de un Arc global. |
| Implementar spawn loading con SQL directo por celda y saltarse `ObjectMgr`/`SpawnData` | Media | Alto | Fase 0 ahora separa spawn stores (#008-#009) de `ObjectGridLoader` (#010), igual que C++ preclasifica GUIDs por map/difficulty/cell. |
| Tests de regresión runtime cuestan tiempo | Media | Bajo | Aceptar y planificar — son los que de verdad demuestran "done". |
| El cliente WoLK 3.4.3 hace cosas no documentadas | Media | Medio | El C++ TrinityCore es la fuente de verdad. Si no aclara, capturar paquetes con `wow-data/pcap` (pendiente). |

---

## 8. Decisiones de arquitectura (ADRs)

### ADR-001: Solo-developer workflow

Trabajamos directamente sobre `main`. **No PRs** (no hay reviewer). Cada commit debe pasar `cargo check + test` antes de pushear. Ramas feature solo para experimentos arriesgados.

### ADR-002: Capas estrictas de crates

`wow-map` no conoce `wow-world::WorldSession`. Las dependencias solo van hacia abajo. Si un crate de capa N necesita algo de capa N+1, se mueve a un trait en capa N o se reorganiza.

### ADR-003: Tests por feature, no por línea

Los tests deben demostrar invariantes de TrinityCore (ej. "un grid en estado Idle pasa a Removal después de 60s sin actividad"), no porcentaje de cobertura.

### ADR-004: Comentarios `// C++ ref:`

Cuando una función traduce código C++, citar archivo y línea: `// C++ ref: Map.cpp:441 (AddPlayerToMap, ASSERT player->GetMap() == this)`. Facilita revisar la migración.

### ADR-005: Cero `unsafe` salvo FFI

Solo `unsafe` permitido en crates de FFI (`wow-recastdetour`). Aislar y documentar.

### ADR-006: SQL prepared statements en `wow-database/statements/`

No SQL inline en handlers. Toda query como `StatementDef` registrado. Facilita auditoría y prevención de inyección.

### ADR-007: Auditoría obligatoria antes de extender

Ningún módulo se considera "trustworthy" hasta tener auditoría vs C++ documentada en `docs/audits/<modulo>.md`. Antes de añadir features a un módulo, ejecutar (o verificar que existe) la auditoría correspondiente. Lo "✅ done" sin auditar es deuda técnica latente.

Las auditorías son commits `docs(audit): ...` separados; no se mezclan con código nuevo.

---

## 9. Glosario rápido

- **NGrid** — el contenedor de 8×8 cells. 64×64 NGrids forman un Map.
- **Cell** — la unidad de visibilidad/carga. ~66 yardas. Granularidad para spawn de mobs.
- **Active object** — entidad que mantiene grids cargados (player, criatura en combate, summons activos).
- **Visibility range** — distancia máxima a la que el cliente ve entidades (~100 yardas en WoLK).
- **PhaseMask** — bitmask de fases; un objeto solo es visible si su phase ∩ player phase ≠ 0.
- **Hotfix** — cambio de DB2 aplicado en runtime sin reinicio (TrinityCore: `hotfix_data` table).

---

## 10. Histórico de cambios al roadmap

| Fecha | Cambio | Commit |
|---|---|---|
| 2026-05-01 | Creación inicial del documento | (este commit) |
| 2026-05-01 | Añadido Fase A (auditoría obligatoria), columna "Auditado vs C++" en matriz, ADR-007, riesgo "lo existente puede tener bugs" | (este commit) |
| 2026-05-07 | Revisión manual del plan contra el árbol C++: `_INDEX.md` pasa a ser inventario de estado, Fase 0 se ajusta a `NGrid.h`/`GridStates.cpp`/`ObjectGridLoader.cpp`/`SpawnData.h`, Fase 1 adelanta `ObjectAccessor` y UpdateFields, Fase 8 separa `scripts/Commands` del contenido masivo. | pendiente |
| 2026-05-07 | Añadida Fase R de refinamiento WBS completo antes de continuar implementación. | pendiente |
| 2026-05-07 | Cerrada R6: la siguiente mini-fase implementable es `#NEXT.L0.CONFIG.001` antes de reanudar Maps/L3. | pendiente |

---

*Actualizar este archivo al cerrar cada fase. Sin documento actualizado, no se considera la fase cerrada.*
