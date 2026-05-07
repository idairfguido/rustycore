# R7 L1 Infra Mini-Phase

> Generated: 2026-05-07
> Rule: every L1 claim is contrasted against C++ under `/home/server/woltk-trinity-legacy/src/server/`.

## Scope

This mini-phase closes the R4 `L1_INFRA` gate enough to unblock packet and map work without hiding the full database/DB2 surface:

- `wow-database`: statement ownership, prepared SQL registry, typed DB separation.
- `wow-data`: DB2/WDC4 loaders and hotfix overlay path.
- `world-server`/`wow-world`: runtime consumers use the correct L1 database/store instead of ad-hoc cross-DB reads.

## Tasks

- [x] **#NEXT.L1.INFRA.001.a** Move hotfix control SQL to `HotfixDatabase` and preload hotfix overlays at startup.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Stores.cpp:1539`, `:1609`, `:1661`.
  Rust targets: `crates/wow-database/src/statements/hotfix.rs`, `crates/wow-data/src/hotfix_cache.rs`, `crates/world-server/src/main.rs`, `crates/wow-world/src/handlers/character.rs`.
  Acceptance: `hotfix_data`, `hotfix_blob`, and `hotfix_optional_data` are owned by `HotfixDatabase`; `CMSG_DB_QUERY_BULK` no longer queries `hotfix_blob` via `WorldDatabase`; tests pass for `wow-database`, `wow-data`, and `world-server`.

- [x] **#NEXT.L1.INFRA.001.b** Refine prepared-statement closure by DB/runtime blocker.
  C++ refs: `docs/migration/inventory/r3-database-registry.md`, `/home/server/woltk-trinity-legacy/src/server/database/Database/Implementation/*.cpp`.
  Acceptance: the 1461 prepared statements are split into executable subtasks by owner DB and runtime dependency, with no optimistic “covered” marker unless SQL is registered and tested.
  Result from R3 inventory: `auth_login` has 136 named Rust statements; `world` has 57 named Rust statements; `characters` has 511 missing and 12 named; `hotfixes` had 745 missing before `#NEXT.L1.INFRA.001.a` added the three DB2Manager control statements.

- [x] **#NEXT.L1.INFRA.001.c** Refine DB2 store closure.
  C++ refs: `docs/migration/inventory/r3-database-registry.md`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Stores.cpp`.
  Acceptance: current hand-written stores, hotfix overlay, and deferred generated DB2 stores are explicitly separated so later Maps/Entities tasks cannot assume missing stores exist.
  Result from R3 inventory: 318 DB2 stores are still missing, 5 have file loaders, and 2 are direct SQL hotfix flows.

## Follow-Up Work Items

- [ ] **#NEXT.L1.DB.PREP.CHARACTER** Split the 511 missing C++ `CharacterDatabase` prepared statements into boot/login/save/social/guild/mail/auction/instance batches.
- [ ] **#NEXT.L1.DB.PREP.HOTFIX** Split or generate HotfixDatabase per-table statements from C++ `HotfixDatabase.cpp`/`DB2LoadInfo.h`; `#NEXT.L1.INFRA.001.a` only covers DB2Manager control queries.
- [ ] **#NEXT.L1.DB2.STORES** Define DB2 store implementation order: minimal runtime stores for Maps/Entities first, then full generated 325-store plan.
- [x] **#NEXT.L1.DB.PREP.MAP_SPAWNS** Add map-spawn prepared SQL and the runtime spawn index needed by Maps.
  C++ refs: `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2167`, `:2449`, `:2492`; `/home/server/woltk-trinity-legacy/src/server/game/Globals/AreaTriggerDataStore.cpp:312`.
  Rust targets: `crates/wow-database/src/statements/world.rs`, `crates/wow-map/src/spawn.rs`.
  Acceptance: creature/gameobject/areatrigger spawn SQL is global preload SQL, not per-cell SQL; `SpawnStore` indexes creature/gameobject GUIDs exactly like `ObjectMgr::AddSpawnDataToGrid`, including personal phase variant; static areatriggers use the non-personal `AreaTriggerDataStore` location index.
