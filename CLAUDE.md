# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

RustyCore — WoW Wrath of the Lich King Classic (3.4.3.54261) private server emulator written in Rust. Migration from the C++ TrinityCore-derived server at `/home/server/woltk-trinity-legacy`. Rust 1.85, edition 2024. ~29-crate Cargo workspace.

The repo lives on GitHub at `https://github.com/alseif0x/rustycore.git` (remote `origin`). There is also an older archived clone at `/home/server/rustycore_ARCHIVED_20260312/` — a parallel iteration with different work, not authoritative.

## Build / test

`protoc` lives at a non-standard path on this machine. Always use the env var:

```bash
PROTOC=/home/cdmonio/.local/protoc/bin/protoc cargo build --workspace
PROTOC=/home/cdmonio/.local/protoc/bin/protoc cargo test --workspace
PROTOC=/home/cdmonio/.local/protoc/bin/protoc cargo build --workspace --release   # ~10 MB binaries
```

Per-crate is much faster while iterating:

```bash
cargo check -p wow-world
cargo test -p wow-world --lib
cargo test -p wow-world --lib -- some_test_name
```

Lints (`Cargo.toml`): `clippy::all` + `pedantic` warn, several casts/docs allowed. `unsafe_code` is `warn`. Release profile: `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`. Current state: `cargo check` 0 errors; `cargo test --workspace` 393 passed.

## Runtime

Two binaries:

- **`bnet-server`** — Battle.net auth: TCP+TLS on **1119**, REST (Axum) on **8081**. Reads `BNetServer.conf`, `bnet_cert.pem`, `bnet_key.pem`, `bnet_fullchain.pem`.
- **`world-server`** — game server: TCP on **8085** / **8086**. Reads `WorldServer.conf`.

Both depend on MariaDB 10.6+ with four databases: `auth`, `characters`, `world`, `hotfixes`. Connection details live in the `.conf` files (gitignored). Production domain: `wowchad.work.gd` (Let's Encrypt).

## Architecture

The workspace is layered foundation → infrastructure → ECS/game → executables. Crates depend downward only.

**Foundation**: `wow-core` (GUID, Position, Time), `wow-constants` (opcodes, game constants), `wow-config`, `wow-logging` (tracing), `wow-crypto` (SRP6, AES-GCM, HMAC-SHA256), `wow-math` (glam), `wow-collections`.

**Infrastructure**: `wow-database` (SQLx + MariaDB, prepared statement registry, schema `updater.rs`), `wow-network` (Tokio sockets, `PlayerRegistry`/`GroupRegistry`/`PendingInvites` shared registries, `SessionManager`, `world_socket` handshake), `wow-proto` (BNet protobuf via prost), `wow-data` (DB2/DBC: `Wdc4Reader`, `HotfixBlobCache`, plus `quest.rs` / `quest_xp.rs` for quest data and `player_stats.rs` / `item_stats.rs` for stat math).

**Game core**: `wow-ecs` (hecs), `wow-world` (the bulk of game logic — `WorldSession`, packet handlers, the new `MapManager`), `wow-map`, `wow-spell`, `wow-combat`, `wow-ai`, `wow-loot`, `wow-chat`, `wow-social`, `wow-pvp`, `wow-achievement`, `wow-script`(s).

**Packets / dispatch**: `wow-packet` (serialization, bit-packing primitives in `world_packet.rs`: `read_bit`, `write_bit`, `flush_bits`, packed GUID; `update.rs` is the big one — `UpdateObject` / `CreatureCreateData` / `PlayerCreateData`; `quest.rs` covers questgiver/log packets), `wow-handler` (dispatch table). FFI: `wow-recastdetour` (pathfinding, scaffolded).

**Executables**: `bnet-server`, `world-server`.

### Packet handler dispatch (read this before adding handlers)

The world server uses static registration via the `inventory` crate. **Two-step dispatch is mandatory**: a handler runs only if it both (a) has a `match` arm in the dispatcher and (b) registers a `PacketHandlerEntry` via `inventory::submit!`. Forgetting `submit!` silently drops the opcode even if the match arm exists. Each `PacketHandlerEntry` declares an opcode, a `SessionStatus` (`Authed` / `LoggedIn` / `Transfer` / `LoggedInOrRecentlyLogout`), and a `PacketProcessing` mode. See `crates/wow-handler/src/lib.rs`.

Handler modules in `crates/wow-world/src/handlers/`:
- `character.rs` — login, char select, char create/delete, played time
- `combat.rs` — attack swing/stop, sheathe, combat tick, XP on kill, level-up
- `loot.rs` — loot unit/item/release
- `chat.rs` — say/yell/whisper/emote/channel broadcast
- `movement.rs` — all CMSG_MOVE_* opcodes
- `quest.rs` — full questgiver flow, accept/abandon, kill objectives, complete, rewards
- `spell.rs` — cast, cancel, cooldown, aura updates
- `trainer.rs` — list, buy spell
- `social.rs` — friends, ignore, contact list, who, inspect
- `group.rs` — invite, accept, leave, party broadcasts
- `misc.rs` — ~40 misc handlers (selection, area trigger teleport, cemetery, taxi status, time sync, etc.)
- `battlenet.rs` — BattlenetRequest plumbing

### `MapManager` — landed module, integration pending

`crates/wow-world/src/map_manager.rs` (~890 lines, 12 unit tests) defines a global, shared world model: `Arc<RwLock<MapManager>>` with 64×64 grids and a single `WorldCreature` per `ObjectGuid` seen by every session on the same map. The intent is for all `WorldSession`s to read/write the same creature state instead of each keeping its own `HashMap`.

**The module is defined and unit-tested in isolation but not yet wired into the live tick path.** `WorldSession` still owns its per-session creature view via the legacy `creatures: HashMap<ObjectGuid, CreatureAI>` field. Migrating session methods to `MapManager` is future work.

`crates/wow-world/_attic/` (renamed `.rs.txt` so cargo skips it) holds ~22 files from a previous integration attempt that produced 176 compile errors when written against `CreatureCreateData` field names that never existed (`entry_id`, `faction`, `position`, `current_hp`, `max_hp`). The real names are `entry`, `faction_template`, `health`, `max_health`. Read `_attic/README.md` first when:
- you need to know which 40+ stub handlers the previous attempt enumerated
- you're about to migrate a session method and want to see what was tried (`migrated_session_methods.rs.txt`, `character_migration.rs.txt`)

Treat `_attic/` as a brief from a colleague who tried and wrote down what didn't work — don't re-introduce content mechanically.

### Patterns to follow

- **Collect-then-send**: build `Vec<Vec<u8>>` of serialized packets first, then send, to avoid borrow conflicts on `&mut self`.
- **`send_packet` vs `send_tx`**: inside tick methods (`tick_combat_sync`, creature ticks, etc.) use `send_tx.send(pkt.to_bytes())` — `send_packet` will double-borrow.
- **`Position` fields are `.x .y .z .orientation`** — not `.o`.
- **`use wow_packet::ClientPacket;`** must be imported explicitly in each handler module that decodes a packet; the trait does not auto-import.
- Prefer `parking_lot` / `dashmap` over std equivalents (already in workspace deps).

### Migration status documents

`MIGRATION_STATUS.md` (root) is a feature-implementation snapshot from late February 2026 — useful for "is X already done" but stale: it doesn't mention the quest system, XP/levelup, aura expiry, spell cooldown, or area-trigger teleports that have landed since.

### Reference implementations

The C++ server at `/home/server/woltk-trinity-legacy` is the canonical reference for this port. Do not trust existing Rust code, prior AI changes, or migration docs as correctness sources until they are contrasted against C++.

The C# server at `/home/server/woltk-server-core/Source/` is historical/reference material only. It can help locate concepts, but C++ wins for protocol layouts, database field order, packet semantics, and mechanics.

## Repo conventions / gitignore quirks

The `.gitignore` excludes a lot of agent/workflow files that **do exist locally and contain useful project context** but are not in the public repo: `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, `PLAN.md`, `MIGRATION_STATUS.md`, `IDENTITY.md`, `INVENTORY.md`, `HEARTBEAT.md`, `TOOLS.md`, plus the `memory/`, `skills/`, `wiki/`, `.claude/`, `.agent(s)/`, `.openclaw/`, `.trae/` directories. Read them when you need project history or domain knowledge — just don't commit them.

`*.pem`, `BNetServer.conf`, `WorldServer.conf`, and the `world-server` / `bnet-server` binaries at the repo root are gitignored and contain credentials/keys — never stage them.
