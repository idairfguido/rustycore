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

- [ ] **#NEXT.L1.INFRA.001.b** Refine prepared-statement closure by DB/runtime blocker.
  C++ refs: `docs/migration/inventory/r3-database-registry.md`, `/home/server/woltk-trinity-legacy/src/server/database/Database/Implementation/*.cpp`.
  Acceptance: the 1461 prepared statements are split into executable subtasks by owner DB and runtime dependency, with no optimistic “covered” marker unless SQL is registered and tested.

- [ ] **#NEXT.L1.INFRA.001.c** Refine DB2 store closure.
  C++ refs: `docs/migration/inventory/r3-database-registry.md`, `/home/server/woltk-trinity-legacy/src/server/game/DataStores/DB2Stores.cpp`.
  Acceptance: current hand-written stores, hotfix overlay, and deferred generated DB2 stores are explicitly separated so later Maps/Entities tasks cannot assume missing stores exist.
