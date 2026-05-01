# Migration Index — TrinityCore wotlk_classic → RustyCore

> Índice maestro de los docs de migración por módulo. Cada doc sigue `_TEMPLATE.md`.
> Si un módulo no aparece aquí o no tiene su `.md`, **es bug del índice** — añadirlo.

**Cómo usar:**
- Para entender cualquier sistema: abrir su doc → secciones 1-7 son C++; 8 es estado actual; 9-12 son plan de trabajo.
- Para auditar un módulo: ya lo está auditando este doc. Update sección 8 + columna "Audited" cuando termines.
- Para empezar a implementar: leer el doc del módulo + dependencias listadas en sección 5.

**Reglas:**
- No modificar código del módulo X sin auditar `<modulo X>.md` primero (ADR-007 del ROADMAP).
- Si encuentras divergencia C++ vs Rust no documentada: añadirla al doc en sección 8 ("Suspicious") + abrir issue/TODO en sección 9.

---

## Tabla maestra

Leyenda Status: ❌ not started / ⚠️ partial / ✅ done / 🔧 broken.
Leyenda Audit: ❌/⚠️/✅.
Layer: L0–L8 según `MIGRATION_ROADMAP.md` § 2.

### L0–L1 — Foundation & Infrastructure

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L0 | Globals | `src/server/game/Globals/` | [globals.md](globals.md) | ⚠️ | ❌ |
| L0 | Time | `src/server/game/Time/` | [time.md](time.md) | ⚠️ | ❌ |
| L0 | Texts | `src/server/game/Texts/` | [texts.md](texts.md) | ⚠️ | ❌ |
| L0 | Tools | `src/server/game/Tools/` | [tools.md](tools.md) | ⚠️ | ❌ |
| L0 | Miscellaneous | `src/server/game/Miscellaneous/` | [miscellaneous.md](miscellaneous.md) | ⚠️ | ❌ |
| L1 | shared/Networking | `src/server/shared/Networking/` | [shared-networking.md](shared-networking.md) | ✅ | ❌ |
| L1 | shared/Packets | `src/server/shared/Packets/` | [shared-packets.md](shared-packets.md) | ✅ | ❌ |
| L1 | Crypto (SRP6/AES-GCM/HMAC) | `src/server/shared/Cryptography/` + `src/common/Cryptography/` | [crypto.md](crypto.md) | ✅ | ⚠️ |
| L1 | shared/Realm | `src/server/shared/Realm/` | [shared-realm.md](shared-realm.md) | ⚠️ | ❌ |
| L1 | shared/Secrets | `src/server/shared/Secrets/` | [shared-secrets.md](shared-secrets.md) | ✅ | ❌ |
| L1 | shared/DataStores | `src/server/shared/DataStores/` | [shared-datastores.md](shared-datastores.md) | ✅ | ❌ |
| L1 | shared/Dynamic | `src/server/shared/Dynamic/` | [shared-dynamic.md](shared-dynamic.md) | n/a | ❌ |
| L1 | shared/JSON | `src/server/shared/JSON/` | [shared-json.md](shared-json.md) | n/a | ❌ |
| L1 | proto/ | `src/server/proto/` | [proto.md](proto.md) | ✅ | ❌ |
| L1 | game/DataStores | `src/server/game/DataStores/` | [datastores.md](datastores.md) | ⚠️ | ❌ |
| L1 | game/Storages | `src/server/game/Storages/` | [storages.md](storages.md) | ⚠️ | ❌ |
| L1 | game/Cache | `src/server/game/Cache/` | [cache.md](cache.md) | ⚠️ | ❌ |
| L1 | game/Accounts | `src/server/game/Accounts/` | [accounts.md](accounts.md) | ⚠️ | ❌ |
| L1 | game/Services | `src/server/game/Services/` | [services.md](services.md) | ⚠️ | ❌ |
| L1 | game/Support | `src/server/game/Support/` | [support.md](support.md) | ❌ | ❌ |

### L2 — Packets & Dispatch

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L2 | game/Handlers | `src/server/game/Handlers/` | [handlers.md](handlers.md) | ⚠️ | ❌ |
| L2 | game/Server | `src/server/game/Server/` | [server.md](server.md) | ✅ | ❌ |

### L3 — World & Maps

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L3 | Maps | `src/server/game/Maps/` | [maps.md](maps.md) | 🔧 | ⚠️ |
| L3 | Grids | `src/server/game/Grids/` | [grids.md](grids.md) | 🔧 | ⚠️ |
| L3 | World | `src/server/game/World/` | [world.md](world.md) | ⚠️ | ❌ |
| L3 | Phasing | `src/server/game/Phasing/` | [phasing.md](phasing.md) | ❌ | ❌ |

### L4 — Entities

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L4 | Entities | `src/server/game/Entities/` | [entities.md](entities.md) | ⚠️ | ❌ |
| L4 | Entities/Pet | `src/server/game/Entities/Pet/` | [pets.md](pets.md) | ⚠️ | ❌ |
| L4 | Entities/Item (Inventory) | `src/server/game/Entities/Item/` + Player inventory | [inventory.md](inventory.md) | ⚠️ | ❌ |

> Nota: Entities es un mega-módulo con ~16 sub-tipos (Object, WorldObject, Unit, Player, Creature, GameObject, Pet, DynamicObject, AreaTrigger, Conversation, Corpse, Vehicle, Transport, SceneObject, Totem, Item). Si su doc supera ~500 líneas, splitear en `entities-<subtipo>.md`.

### L5 — Engines

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L5 | Movement | `src/server/game/Movement/` | [movement.md](movement.md) | ⚠️ | ❌ |
| L5 | Combat | `src/server/game/Combat/` | [combat.md](combat.md) | ⚠️ | ❌ |
| L5 | Spells | `src/server/game/Spells/` | [spells.md](spells.md) | ⚠️ | ❌ |
| L5 | AI | `src/server/game/AI/` | [ai.md](ai.md) | ⚠️ | ❌ |

### L6 — Game systems

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L6 | Quests | `src/server/game/Quests/` | [quests.md](quests.md) | ⚠️ | ❌ |
| L6 | Loot | `src/server/game/Loot/` | [loot.md](loot.md) | ⚠️ | ❌ |
| L6 | Chat | `src/server/game/Chat/` | [chat.md](chat.md) | ⚠️ | ❌ |
| L6 | Social (friends/ignore) | `src/server/game/Social/` | [social.md](social.md) | ⚠️ | ❌ |
| L6 | Groups | `src/server/game/Groups/` | [groups.md](groups.md) | ⚠️ | ❌ |
| L6 | Guilds | `src/server/game/Guilds/` | [guilds.md](guilds.md) | ❌ | ❌ |
| L6 | Mails | `src/server/game/Mails/` | [mails.md](mails.md) | ❌ | ❌ |
| L6 | AuctionHouse | `src/server/game/AuctionHouse/` | [auctionhouse.md](auctionhouse.md) | ❌ | ❌ |
| L6 | AuctionHouseBot | `src/server/game/AuctionHouseBot/` | [auctionhousebot.md](auctionhousebot.md) | ❌ | ❌ |
| L6 | Calendar | `src/server/game/Calendar/` | [calendar.md](calendar.md) | ❌ | ❌ |
| L6 | Achievements | `src/server/game/Achievements/` | [achievements.md](achievements.md) | ❌ | ❌ |
| L6 | Reputation | `src/server/game/Reputation/` | [reputation.md](reputation.md) | ❌ | ❌ |
| L6 | Skills | `src/server/game/Skills/` | [skills.md](skills.md) | ⚠️ | ❌ |
| L6 | BlackMarket | `src/server/game/BlackMarket/` | [blackmarket.md](blackmarket.md) | ❌ | ❌ |
| L6 | BattlePets | `src/server/game/BattlePets/` | [battlepets.md](battlepets.md) | n/a (post-WoLK) | ❌ |
| L6 | Petitions | `src/server/game/Petitions/` | [petitions.md](petitions.md) | ❌ | ❌ |
| L6 | Pools | `src/server/game/Pools/` | [pools.md](pools.md) | ❌ | ❌ |
| L6 | Conditions | `src/server/game/Conditions/` | [conditions.md](conditions.md) | ❌ | ❌ |

### L7 — Instances, BG, Arenas, PvP

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L7 | Instances | `src/server/game/Instances/` | [instances.md](instances.md) | ❌ | ❌ |
| L7 | Battlegrounds | `src/server/game/Battlegrounds/` | [battlegrounds.md](battlegrounds.md) | ❌ | ❌ |
| L7 | Battlefield | `src/server/game/Battlefield/` | [battlefield.md](battlefield.md) | ❌ | ❌ |
| L7 | OutdoorPvP | `src/server/game/OutdoorPvP/` | [outdoorpvp.md](outdoorpvp.md) | ❌ | ❌ |
| L7 | DungeonFinding | `src/server/game/DungeonFinding/` | [dungeonfinding.md](dungeonfinding.md) | ❌ | ❌ |
| L7 | Scenarios | `src/server/game/Scenarios/` | [scenarios.md](scenarios.md) | n/a (post-WoLK) | ❌ |

### L8 — Content & service

| Layer | Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|---|
| L8 | Scripting | `src/server/game/Scripting/` | [scripting.md](scripting.md) | ❌ | ❌ |
| L8 | scripts/ (content) | `src/server/scripts/` | [scripts.md](scripts.md) | ❌ | ❌ |
| L8 | Warden | `src/server/game/Warden/` | [warden.md](warden.md) | ❌ | ❌ |
| L8 | Events | `src/server/game/Events/` | [events.md](events.md) | ❌ | ❌ |
| L8 | Weather | `src/server/game/Weather/` | [weather.md](weather.md) | ❌ | ❌ |
| L8 | Achievements scripts | (parte de Achievements) | (en achievements.md) | — | — |

### Binarios

| Module | C++ path | Doc | Status | Audit |
|---|---|---|---|---|
| bnetserver | `src/server/bnetserver/` | [bnetserver.md](bnetserver.md) | ✅ | ❌ |
| worldserver | `src/server/worldserver/` | [worldserver.md](worldserver.md) | ✅ | ❌ |

---

## Conteo

- **Total módulos enumerados:** 64 (incluye sub-módulos `pets`, `inventory`, `social`, `crypto` y los dos binarios `bnetserver` / `worldserver`).
- **Docs creados:** 64/64 — primera ronda completa.
- **Docs auditados vs C++:** 0/64 — pendiente Fase A (auditoría sistemática módulo a módulo).

> Nota: el estado por columna refleja la **implementación Rust actual**, no la calidad del doc. Un módulo `❌` con doc completo significa "documentado, pero el código Rust no existe o es una stub". Tras auditar, actualizar la columna **Audit** a ⚠️ (divergencias documentadas) o ✅ (audit cerrada, sin gaps abiertos).

---

## Histórico

| Fecha | Cambio |
|---|---|
| 2026-05-01 | Índice creado, 45 módulos enumerados, 0 docs todavía |
| 2026-05-01 | Primera ronda completa: 64/64 docs con plantilla de 12 secciones, plan de migración por módulo, gotchas y mapping C++→Rust. Audit aún 0/64. |

---

*Mantener actualizada esta tabla cada vez que se cree o modifique un doc de migración.*
