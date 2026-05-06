# Migration Roadmap — TrinityCore (wotlk_classic) → RustyCore (Rust)

> Plan operativo para migrar **todo** TrinityCore C++ a Rust. Este documento es la fuente de verdad: prioridad, orden, estado actual y TODO list. Se actualiza al cierre de cada fase.

**Repos de referencia:**
- C++ origen: `/home/server/woltk-trinity-legacy` (TrinityCore branch `wotlk_classic`)
- Rust destino: `/home/server/rustycore` (este repo, GitHub `alseif0x/rustycore`)
- C# legacy: `/home/server/woltk-server-core/Source/` (referencia secundaria, mismo modelo)

**Reglas inviolables:**

1. **Antes de implementar** cualquier sistema, leer su contraparte C++ en TrinityCore. Nunca improvisar a oído. Lecciones del bridge MapManager fallido (`_attic/`) costaron 176 errores de compilación.
2. **Antes de extender** cualquier sistema ya migrado, **auditarlo contra C++**. Lo que está marcado ✅/⚠️ en este documento puede tener bugs, divergencias o piezas que faltan respecto al C++. **Nada se da por bueno hasta auditoría**. Un sistema "implementado" sin auditar es un riesgo, no una ventaja.

---

## 1. Visión general

### 1.1 Topología C++ que hay que migrar

TrinityCore expone tres binarios y ~58 módulos en `src/server/`:

**Binarios:**
- `bnetserver` — autenticación Battle.net (BNet protobuf, SRP6, REST)
- `worldserver` — servidor de juego (sockets WoW, dispatch, todos los sistemas)
- (`scripts` se compila como librería linkada al worldserver)

**Capas (`src/server/`):**
```
shared/      Networking, Packets, Realm, Secrets, DataStores, Dynamic, JSON
game/        50 subdirectorios — todo el contenido del juego
proto/       definiciones protobuf BNet
database/    capa SQL común a todos los servidores
scripts/     contenido scripteado (bosses, instancias, NPCs)
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

Basado en la auditoría cruzada de los Explore agents (commits `c65858b` y previo). **Importante**: la columna `% migrado` es **estimación de superficie**; mientras la columna `Auditado vs C++` no esté ✅, el % es solo orientativo y puede esconder bugs/divergencias significativas.

Leyenda **Auditado vs C++**: ❌ no auditado / ⚠️ auditoría parcial / ✅ contrastado completo.

| Capa | % migrado | Auditado vs C++ | Nota |
|---|---|---|---|
| **Foundation** (core/constants/config/logging/math/collections) | ~95% | ❌ | Estable, pocas adiciones |
| **Crypto** (SRP6, AES-GCM, HMAC, BNet keys) | ~90% | ❌ | Falta limpieza menor |
| **Database** (SQLx + statements) | ~70% | ❌ | Updater básico, faltan muchos statements |
| **DBC/DB2** (Wdc4, hotfix cache) | ~85% | ❌ | Lectura OK, falta granularidad de algunos stores |
| **Network** (BNet TCP+TLS, WorldSocket, encryption) | ~85% | ❌ | Estable |
| **Packets** (serialización, dispatch) | ~75% | ❌ | ~150 packets cubiertos, faltan muchos |
| **Maps & Grids** | **🔧 15%** | ⚠️ (auditado, conclusión: rehacer) | Sandbox aislado; necesita rehacer fiel a TrinityCore |
| **Entities** (Object/WorldObject/Unit/Player/Creature) | ~30% | ❌ | Player en sesión, no como entidad jerárquica |
| **Movement** (parsing CMSG_MOVE_*, server pos update) | ~40% | ❌ | Sin pathfinding, sin spline real |
| **Combat** (auto-attack server-authoritative) | ~25% | ❌ | Sin spells, sin school resistances, sin threat real |
| **Spells & Auras** | ~20% | ❌ | Cast basic + cooldown, sin scripting de efectos |
| **AI** (CreatureAI states) | ~15% | ❌ | Idle/Walk/Combat; sin SmartAI, sin ScriptedAI |
| **Quests** (Phase 1+2 done) | ~50% | ❌ | Kill objectives + accept/abandon/complete |
| **Inventory** (equipped + bags partial) | ~35% | ❌ | Falta bank, mail, durability, transmog |
| **Loot** (templates + drop) | ~40% | ❌ | Falta loot rules en grupo |
| **Social** (friends/ignore/inspect/who) | ~50% | ❌ | Falta canales globales |
| **Group/Raid** (party invite/leave) | ~40% | ❌ | Sin loot rules, sin ready check, sin marker |
| **Chat** (say/yell/whisper/emote) | ~60% | ❌ | Falta canales y BG/guild advanced |
| **Trainer/Vendor/Gossip** | ~50% | ❌ | Vendor real, trainer parcial, gossip stub |
| **Instances/Dungeons** | 0% | n/a | Sin instance lock, sin difficulty, sin script |
| **Battlegrounds/Arenas** | 0% | n/a | Solo stubs handler |
| **Achievements** | 0% | n/a | Crate scaffold vacío |
| **Auctions/Mail/Calendar** | 0% | n/a | Solo stubs handler |
| **Scripts** (bosses, NPCs, gossip) | 0% | n/a | Crate scaffold vacío |
| **PvP** (rated, conquest, honor, OutdoorPvP) | 0% | n/a | Sin honor, sin honor PvP zones |
| **GM/Commands** (.tele, .level, etc.) | 0% | n/a | No empezado |
| **Phases** (PhaseMgr, group/personal phasing) | 0% | n/a | Solo SMSG_PHASE_SHIFT_CHANGE estático |
| **Conditions** (drop conditions, gossip cond) | 0% | n/a | No empezado |
| **Talents/Glyphs** | 0% | n/a | Solo SMSG_UPDATE_TALENT_DATA estático |
| **LFG / DungeonFinding** | 0% | n/a | Solo DF_GET_SYSTEM_INFO stub |
| **Transports / Vehicles** | 0% | n/a | Sin MOTransport, sin Vehicle |
| **Pathfinding (Recast/Detour)** | 5% | ❌ | Crate `wow-recastdetour` scaffold FFI, no usado |
| **Warden / Anticheat** | 0% | n/a | No empezado |
| **Pet system (hunter)** | 0% | n/a | No empezado |

**Total ponderado**: ~25% migrado **sin auditar**. Solo Maps tiene auditoría parcial (la que hicimos hoy con los Explore agents). El resto puede tener desde "perfecto" hasta "muy roto" — no lo sabemos hasta auditarlo.

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

## 3. Estado por módulo (matriz completa)

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
| `Grids/Grid` + `NGrid` | (no existe) | ❌ | falta separación NGrid (8×8 cells) vs Cell |
| `Grids/Cell` (8×8 dentro de NGrid) | (no existe) | ❌ | sin esto la visibilidad es 8× más gruesa |
| `Grids/GridStates` (Active/Idle/Removal) | (no existe) | ❌ | sin máquina de estados, grids no se descargan |
| `Grids/ObjectGridLoader` | (no existe) | ❌ | sin lazy load DB → grid |
| `Maps/MapUpdater` (thread pool por map) | (no existe) | ❌ | actualmente todo serializa por RwLock global |
| `Maps/TerrainMgr` + `GridMap` | wow-map (vacío) | ❌ | no hay carga de mapas .map de cliente |
| `Maps/MapReference` / `MapRefManager` | (no existe) | ❌ | iteración de jugadores en map |
| `Phasing/PhaseMgr` | (no existe) | ❌ | personal/group phases |
| `Maps/SpawnData` | (no existe) | ❌ | unified spawn descriptors |

### L4 Entities

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Entities/Object/Object` (base) | wow-core (parcial) | ⚠️ | GUID OK, falta jerarquía polimórfica |
| `Entities/Object/WorldObject` | (mezclado en session) | ⚠️ | posición, mapa, fases, eventos |
| `Entities/Unit/Unit` | (no existe formal) | ❌ | health/power/stats/auras/threat — disperso en session |
| `Entities/Player/Player` | wow-world/session.rs | ⚠️ | mezclado con sesión, no como entidad |
| `Entities/Creature/Creature` | wow-ai/CreatureAI | ⚠️ | duplicado entre wow-ai y map_manager |
| `Entities/GameObject/GameObject` | wow-world (parcial) | ⚠️ | falta lifecycle, scripts |
| `Entities/Pet/Pet` | (no existe) | ❌ | hunter pets |
| `Entities/DynamicObject` | (no existe) | ❌ | DynObjects de spells AoE |
| `Entities/AreaTrigger/AreaTrigger` | wow-data/area_trigger | ⚠️ | datos sí, lógica no |
| `Entities/Conversation` | (no existe) | ❌ | quest text dialogues |
| `Entities/Corpse` | (no existe) | ❌ | corpses persistentes |
| `Entities/Vehicle` | (no existe) | ❌ | sistema de vehículos |
| `Entities/Transport` (MO) | (no existe) | ❌ | barcos, dirigibles |
| `Entities/SceneObject` | (no existe) | ❌ | escenas cinematicas |
| `Entities/Totem` | (no existe) | ❌ | totems chamán |

### L5 Engines: Movement, Combat, Spells, AI

| Módulo C++ | Crate Rust | Estado | Pendiente |
|---|---|---|---|
| `Movement/MovementInfo` | wow-packet | ✅ | parsing OK |
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
2. Spawn de Explore agent con prompt estructurado: "lee X.cpp y resume invariantes runtime; cita líneas".
3. Leer el código Rust correspondiente.
4. Producir tabla de divergencias.
5. Para cada divergencia: clasificar como **bug** (Rust diverge mal) / **missing** (Rust no implementa) / **extra** (Rust hace de más) / **OK** (divergencia aceptable, ej. idiom Rust).
6. Para bugs y missing críticos: añadir TODO en sección 5.
7. Commit `docs(audit): audit <modulo>` con el mini-informe.

### Fase 0 — Fundación de Maps (rehacer L3) — *🔧 prioridad inmediata*

> El bloqueante de TODO lo demás. Sin Map/Grid/Cell correctos, ni entidades, ni AI, ni multi-player escalable.

- **0.1** `wow-map`: constantes (`SIZE_OF_GRIDS=533.3333`, `MAX_NUMBER_OF_GRIDS=64`, `MAX_NUMBER_OF_CELLS=8`), tipos `GridCoord`, `CellCoord`, `MapKey`, conversión `compute_grid_coord(x,y)` / `compute_cell_coord(x,y)`. Tests unitarios contra valores de TrinityCore.
- **0.2** `wow-map`: estructura `Map` (id, instance, matriz 64×64 NGrid), `NGrid` (8×8 Cell + GridInfo + estado), `Cell` (containers tipados).
- **0.3** `wow-map`: máquina de estados `GridState` (`Invalid`/`Active`/`Idle`/`Removal`) con transiciones temporizadas (referencia: `GridStates.cpp`).
- **0.4** `wow-map`: `ObjectGridLoader` — carga lazy de criaturas/GO/AreaTrigger desde DB cuando una cell pasa a Active. Necesita statement nuevo `SEL_CREATURE_BY_MAP_GRID`.
- **0.5** `wow-map`: `MapManager` singleton con `i_maps: HashMap<MapKey, Map>`, `create_map`, `find_map`, `do_for_all_maps` con `RwLock` (read = lookups, write = create/destroy). Update loop single-threaded por map.
- **0.6** `wow-map`: `MapUpdater` — pool de threads para distribuir updates de Maps (opcional pero recomendado).
- **0.7** Migrar `crates/wow-world/src/map_manager.rs` actual: tirar a la basura, retener solo los 12 tests como regresión (re-adaptados).
- **0.8** `world-server/main.rs`: integrar el nuevo `MapManager`, arrancar update loop.
- **0.9** Migrar handlers que tocan `self.creatures` (43 sitios): orden loot → combat → trainer → misc → session ticks → character.rs. Cada handler en su commit.
- **0.10** Quitar campos legacy `creatures`/`visible_creatures` de `WorldSession`.

### Fase 1 — Entidades canónicas (L4)

- **1.1** `wow-entities` (crate nuevo): trait `Object` (base) → `WorldObject` → `Unit` → `Player` / `Creature`. Polimorfismo con enum o trait objects (decidir basado en perf).
- **1.2** Mover IA pura de `wow-ai` a la struct `Creature` (state, timers, target). Limpiar duplicación con `WorldCreature`.
- **1.3** Refactor `WorldSession` para que `player` sea `Player` referencia entidad, no datos sueltos en la sesión.
- **1.4** `GameObject`, `Corpse`, `DynamicObject`, `AreaTrigger`, `Pet` como entidades.
- **1.5** Update fields (TrinityCore `UpdateFields.h`): unified delta updates por tipo de entidad.

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
- **8.2** Migrar el grueso de `scripts/` (~3000 archivos C++ → idioms Rust) — esto es el 50%+ del trabajo total. Probablemente automatizado con un transpilador semi-asistido.
- **8.3** Chat commands GM (.tele, .gm, .level, .item, .additem, .lookup...).
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
- [ ] **#A06** Auditar **Movement parsing** (`wow-packet/movement.rs`, handlers/movement.rs) vs `src/server/game/Movement/PacketBuilder` + handlers.
- [ ] **#A07** Auditar **Combat** (`wow-combat` + handlers) vs `src/server/game/Combat/`. Damage roll, miss tables, hit info.
- [ ] **#A08** Auditar **Spells** (`wow-spell`) vs `src/server/game/Spells/`. Spell flow, casting, effects subset.
- [ ] **#A09** Auditar **Quests** (handlers/quest.rs, wow-data/quest) vs `src/server/game/Quests/`. Eligibility, kill credit, completion, reward.
- [ ] **#A10** Auditar **Inventory** (handlers/character.rs partes inventario) vs `src/server/game/Entities/Player/PlayerStorage.cpp`.
- [ ] **#A11** Auditar **Loot** (`wow-loot`) vs `src/server/game/Loot/LootMgr.cpp`. Drop chance, condition support.
- [ ] **#A12** Auditar **Chat** (`wow-chat`) vs `src/server/game/Chat/`. Mensaje broadcast, silenciamiento, anti-spam.
- [ ] **#A13** Auditar **Social** (`wow-social`) vs `src/server/game/Handlers/SocialHandler.cpp`.
- [ ] **#A14** Auditar **Group** vs `src/server/game/Groups/`.
- [ ] **#A15** Auditar **Trainer/Vendor/Gossip** (handlers) vs `src/server/game/Handlers/NPCHandler.cpp`.

### Inmediato (Fase 0 — Maps rewrite)

- [x] **#001** `wow-map`: módulo `coords.rs` con constantes y `compute_grid_coord` / `compute_cell_coord`. Tests vs `GridDefines.h`. Cerrado en `crates/wow-map/src/coords.rs` contra `GridDefines.h`.
- [ ] **#002** `wow-map`: `MapKey { map_id: u16, instance_id: u32 }`.
- [ ] **#003** `wow-map`: `Cell` struct con containers tipados (`HashMap<ObjectGuid, CreatureRef>`, similar para GO/AT).
- [ ] **#004** `wow-map`: `NGrid` (8×8 `Cell` + `GridInfo` con timer).
- [ ] **#005** `wow-map`: `GridState` enum + `update(state, &mut NGrid, &Map, diff)` para cada estado (referenciar `GridStates.cpp` línea por línea).
- [ ] **#006** `wow-map`: `Map` con `i_grids: [[Option<Box<NGrid>>; 64]; 64]`, `add_player`, `remove_player`, `update(diff)`, `load_grid`, `unload_grid`, `ensure_grid_loaded`.
- [ ] **#007** `wow-map`: `MapManager` singleton con `RwLock`, `create_map`, `find_map`, `do_for_all_maps`, `update`.
- [ ] **#008** `wow-database`: nuevo statement `SEL_CREATURE_BY_MAP_CELL` (params: map, grid_x, grid_y, cell_x, cell_y) o por grid completo.
- [ ] **#009** `wow-map`: `ObjectGridLoader::load_n(grid)` — carga creatures/GO/AT desde DB para todas las cells del grid. Replicar `ObjectGridLoader.cpp::LoadN`.
- [ ] **#010** `wow-map`: integrar terreno/`GridMap` (carga de archivos .map del cliente) — quizá diferir si WoLK Classic no requiere LoS exacto al inicio.
- [ ] **#011** `wow-map`: tests integration: spawnea criatura, mueve player, verifica visibilidad por cells correcta (NO por grid).
- [ ] **#012** Limpiar `crates/wow-world/src/map_manager.rs`: tirar implementación, retener `WorldCreature`-equivalente como tipo en wow-map (rename `Creature`).
- [ ] **#013** `world-server/main.rs`: arrancar `MapManager` global + spawn task `update_loop()` (interval 100ms).
- [ ] **#014** Migrar `handlers/loot.rs` a usar `MapManager::find_creature(guid)` en vez de `self.creatures`.
- [ ] **#015** Migrar `handlers/combat.rs` (3 sitios).
- [ ] **#016** Migrar `handlers/trainer.rs` (1 sitio).
- [ ] **#017** Migrar `handlers/misc.rs` (2 sitios).
- [ ] **#018** Migrar `session.rs::tick_creatures_sync` a iterar criaturas via Map del player.
- [ ] **#019** Migrar `session.rs::tick_combat_sync`.
- [ ] **#020** Migrar `session.rs::send_nearby_creatures` → desaparece, sustituida por load on-demand del MapManager.
- [ ] **#021** Migrar `handlers/character.rs::update_creature_visibility` a usar visitor pattern del MapManager (15+ sitios).
- [ ] **#022** Quitar campos `creatures`/`visible_creatures` de `WorldSession`. Borrar el `_attic/` (ya no aporta).

### Inmediato siguiente (Fase 1 — Entidades canónicas)

- [ ] **#023** `wow-entities` (crate nuevo): trait `Object` con métodos comunes (guid, type, position, map).
- [ ] **#024** `wow-entities`: `WorldObject` extiende `Object` (orientation, facing, distance_to).
- [ ] **#025** `wow-entities`: `Unit` extiende `WorldObject` (health, power, stats, faction, auras).
- [ ] **#026** `wow-entities`: `Player` extiende `Unit` (account, char data, inventory, quests, skills).
- [ ] **#027** `wow-entities`: `Creature` extiende `Unit` (template, AI ref, spawn point, respawn timer).
- [ ] **#028** Mover `wow-ai::CreatureAI` a `wow-entities::Creature` (eliminar duplicación con `wow-map::Creature`).
- [ ] **#029** Refactor `WorldSession` para tener `player: Option<Player>` en vez de campos sueltos.
- [ ] **#030** Update fields delta (TrinityCore `UpdateFields.h`): generador de paquete UpdateObject por entidad.

> Tras cerrar #030, el roadmap continúa con Fase 2 (Movement) y siguientes según la sección 4.

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
| **Lo "✅ done" actual tiene bugs/divergencias vs C++ que no hemos detectado** | Alta | Alto | Fase A (auditoría obligatoria por módulo) antes de extender. Tabla de divergencias en `docs/audits/<modulo>.md`. Hasta que un módulo no esté auditado, su columna "Auditado vs C++" sigue ❌ y se trata con sospecha. |
| Auditar todo costaría tanto como reescribirlo | Media | Medio | Las auditorías se priorizan: módulos críticos (network, crypto, packets, maps) primero; los de menor superficie y baja prioridad pueden auditarse "just-in-time" antes de extender. |
| Scope creep entre fases (querer hacer L5 antes de L3 estable) | Alta | Alto | Esta hoja de ruta es vinculante. No se salta orden sin acuerdo explícito. |
| Implementación parcial que parezca completa (ej. spell engine que solo cubre 5 efectos) | Media | Medio | Tests por feature concreta. Marcar ⚠️ en lugar de ✅ hasta cobertura ≥ 90%. |
| Acoplamiento accidental entre crates (wow-map dependiendo de wow-world) | Baja | Alto | Disciplina de capas. wow-map no conoce sesiones, solo entidades. |
| Pathfinding (Detour) incompleto bloquea AI | Media | Medio | Hacer movement waypoint sin pathfinding primero; Detour es Fase 2.2. |
| `scripts/` (3000 archivos) bloquea cualquier contenido scripteado | Alta | Alto | Aceptar que la mayoría de bosses/instancias no funcionan hasta Fase 8. Priorizar SmartAI (data-driven) que cubre ~50% sin scripting. |
| Performance: `Arc<RwLock<MapManager>>` global serializa todo | Alta | Alto | Resolver en Fase 0.6 (MapUpdater pool). Si no resuelve, considerar one-Arc-per-Map en lugar de un Arc global. |
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

---

*Actualizar este archivo al cerrar cada fase. Sin documento actualizado, no se considera la fase cerrada.*
