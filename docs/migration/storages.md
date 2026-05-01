# Migration: Storages (in-memory snapshot containers — currently just WhoListStorage)

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Storages/`
> **Rust target crate(s):** `crates/wow-social/` or `crates/wow-world/` (no dedicated crate needed)
> **Layer:** L1 (infrastructure — periodically-rebuilt cache of online-player state)
> **Status:** ❌ not started
> **Audited vs C++:** ✅ complete (2 files, 154 lines total)
> **Last updated:** 2026-05-01

---

## 1. Purpose

Tiny module — *one* class. `Storages/` is a parallel folder to `DataStores/` for things that are also "global look-up tables" but are populated from runtime state (online players) rather than static client data files. Today it contains only `WhoListStorage`: a periodically-rebuilt snapshot of every connected, world-loaded, non-loading player, used to answer the `/who` command without iterating the global player map under lock for every query. The folder is named `Storages/` (plural) because the historical intent was for more such caches (`PendingMailStorage`, `MarketStorage`, etc.) to land here, but in the current TrinityCore tree only the who-list survives.

This doc could reasonably be merged into `social.md` or `chat.md`, but a separate doc is cheaper than burying the rebuild semantics inside an unrelated module.

---

## 2. C++ canonical files

Todas las rutas relativas a `/home/server/woltk-trinity-legacy/`.

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Storages/WhoListStorage.h` | 86 | `WhoListPlayerInfo` POD-ish entry + `WhoListStorageMgr` (Meyers singleton) |
| `src/server/game/Storages/WhoListStorage.cpp` | 68 | `instance()` + `Update()` (the only mutator) |

That is the entire module.

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `WhoListPlayerInfo` | class | Snapshot of one online player. 15 fields: `_guid: ObjectGuid`, `_team: uint32`, `_security: AccountTypes`, `_level: uint8`, `_class: uint8`, `_race: uint8`, `_zoneid: uint32`, `_gender: uint8`, `_visible: bool`, `_gamemaster: bool`, `_widePlayerName: wstring` (lower-cased), `_wideGuildName: wstring` (lower-cased), `_playerName: string` (display case), `_guildName: string` (display case), `_guildguid: ObjectGuid` |
| `WhoListInfoVector` | typedef | `std::vector<WhoListPlayerInfo>` |
| `WhoListStorageMgr` | class | Singleton; one private member: `WhoListInfoVector _whoListStorage` |

The class has only getters — no public mutator. The vector is replaced wholesale by `Update()`.

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `WhoListStorageMgr::instance()` | Meyers singleton accessor (`#define sWhoListStorageMgr WhoListStorageMgr::instance()`) | — |
| `WhoListStorageMgr::Update()` | Rebuild the snapshot. Walks `ObjectAccessor::GetPlayers()` (the `HashMapHolder<Player>` global map). For each player: skip if not on a map (`FindMap == nullptr`) or if their session is still in the loading screen. Read name/guild via `Player::GetName` and `GuildMgr::GetGuildNameById(player.guildId)`. Convert both to wstring + lowercase via `Utf8toWStr` + `wstrToLower`. Emplace a `WhoListPlayerInfo`. Replaces `_whoListStorage` wholesale (initial `clear()` + `reserve(player_count + 1)`). | `ObjectAccessor::GetPlayers`, `Player::Get*`, `GuildMgr::GetGuildNameById`, `Player::IsVisible`/`IsGameMaster`, `Player::GetTeam`/`GetSecurity`/`GetLevel`/`GetClass`/`GetRace`/`GetZoneId`/`GetNativeGender`, `Utf8toWStr`, `wstrToLower` |
| `WhoListStorageMgr::GetWhoList() const -> WhoListInfoVector const&` | Read-only access. Caller iterates linearly to apply `/who` filters | — |
| `WhoListPlayerInfo::Get*` (15 trivial getters) | Field access | — |

---

## 5. Module dependencies

**Depends on:**
- `Server/World` — `sWorld->GetPlayerCount()` for vector reservation.
- `Entities/Object/ObjectAccessor` — `GetPlayers()` returns the global `HashMapHolder<Player>::MapType const&`.
- `Entities/Player` — every getter on `Player` listed above; `IsVisible`, `IsGameMaster`, `GetGuild`, `GetGuildId`.
- `Server/WorldSession` — `GetSecurity()`, `PlayerLoading()` (skip-while-loading filter).
- `Guilds/GuildMgr` — `GetGuildNameById(guildId)`. Also `Guild::GetGUID()` for the guild GUID field.
- `ObjectGuid` — typed GUIDs.
- `Util` — `Utf8toWStr`, `wstrToLower`.
- `World/World` — referenced via `sWorld` macro.

**Depended on by:**
- `Handlers/CharacterHandler` (or `Handlers/MiscHandler`) — `WorldSession::HandleWhoOpcode` walks `sWhoListStorageMgr->GetWhoList()` applying the request's name/guild/zone/race/class/level filters and packages matches into `SMSG_WHO`.
- The world's main tick (`World::Update`) calls `Update()` periodically — typical interval is `5 * IN_MILLISECONDS` (5 s) configured by `CONFIG_WHO_LIST_UPDATE_INTERVAL`-equivalent. Snapshots stale by up to 5 s are acceptable for `/who`.

No other consumers — this is exclusively a `/who` accelerator.

---

## 6. SQL / DB queries (if any)

None. Pure in-memory; sources from runtime state.

| Statement / Source | Purpose | DB |
|---|---|---|
| (none) | — | — |

No DBC/DB2 stores either.

---

## 7. Wire-protocol packets (if any)

The module itself does not emit/receive packets; its consumer does:

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_WHO` | client → server | `WorldSession::HandleWhoOpcode` (consumer of `GetWhoList`) |
| `SMSG_WHO` | server → client | `WorldSession::HandleWhoOpcode` (consumer of `GetWhoList`) |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- None. Searches for `WhoList`, `WhoListStorage`, `who_list` across `crates/` return zero hits.
- `PlayerRegistry` (in `wow-network`, ~referenced from `CLAUDE.md`) is a related primitive — it tracks online sessions — but it is not the same shape. The who-list cache is a denormalized read-only snapshot containing fields (level, class, zone, guild name, lowercase wide name) that aren't necessarily on the registry entry. Even if they were, hitting the live registry per `/who` query under any contention would lock-bottleneck — the explicit snapshot pattern exists exactly to avoid that.

**What's implemented:** nothing.

**What's missing vs C++:** the entire module. As long as `CMSG_WHO` is unimplemented or stubbed, the gap is invisible. As soon as a meaningful `/who` lands, the implementation will need either (a) the snapshot pattern, or (b) a non-locking iterator over `PlayerRegistry`. Pick (a) — TrinityCore picked it for a reason and the cost is one Vec rebuilt every 5 seconds.

**Suspicious / likely divergent:** N/A.

**Tests existing:** none.

---

## 9. Migration sub-tasks

Numera los items para poder referenciarlos desde `MIGRATION_ROADMAP.md` sección 5.

Complejidad: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, splitear).

- [ ] **#STR.1** Define `pub struct WhoListPlayerInfo` mirroring the C++ class. Use owned `String` (UTF-8) for `player_name`/`guild_name` and pre-lowercased `String` for `wide_player_name_lower`/`wide_guild_name_lower`. WoW `wstrToLower` on UTF-16 is *not* the same as `to_lowercase` on UTF-8 for some Unicode codepoints — for the WoLK 3.4.3 client this rarely matters (Latin-1 supplement only) but document the divergence. (complexity: **L**)
- [ ] **#STR.2** Define `pub struct WhoListStorage { snapshot: ArcSwap<Vec<WhoListPlayerInfo>> }` (use `arc_swap` for lock-free reads — already a workspace-aligned pattern). Provide `pub fn snapshot(&self) -> Arc<Vec<WhoListPlayerInfo>>`. (complexity: **L**)
- [ ] **#STR.3** Implement `WhoListStorage::update(&self, player_registry: &PlayerRegistry, guild_mgr: &GuildMgr)`. Walk the registry, skip sessions still loading (`PlayerLoading()` equivalent — likely a flag on `WorldSession`), build a fresh `Vec` with `with_capacity(player_count + 1)`, replace `snapshot`. Match TrinityCore's filters: `FindMap == None` → skip; `PlayerLoading() == true` → skip. (complexity: **M**)
- [ ] **#STR.4** Wire periodic invocation: every 5 seconds in the world tick (`World::Update` analogue). Make the interval a config knob `wholist_update_interval_ms` defaulting to 5000. (complexity: **L**)
- [ ] **#STR.5** Implement `CMSG_WHO` handler that consumes the snapshot, applies the filter (name substring, guild substring, level range, race mask, class mask, zone list, server-string mask), caps at 49 results, packages into `SMSG_WHO`. (complexity: **M** — packet shape; the snapshot read is trivial.)
- [ ] **#STR.6** Lower-case wide-string handling: provide a `whostr::wstrtolower(input: &str) -> String` helper. For accuracy parity with TC, mirror `Utf8toWStr` (UTF-8 → UTF-16 LE) + `wstrToLower` (per-`wchar` `towlower`); a pure UTF-8 `to_lowercase` will be ≈99 % equivalent for Latin-script names but diverges for some German/French sharp-s and Greek edge cases. Acceptable hack: pure UTF-8 lower with a TODO. (complexity: **L**)

---

## 10. Regression tests to write

- [ ] After `update()` with N online players in a fixture registry (some loading, some not on a map), `snapshot().len() == count(online ∧ on_map ∧ !loading)`.
- [ ] Names are stored in *both* display case (`player_name`) and lowercase (`wide_player_name_lower`).
- [ ] Two consecutive `update()` calls without state change produce vectors of equal length and content (idempotence).
- [ ] `update()` reservation is exact: `Vec::capacity() == len()` after build (asserts the `reserve(count + 1)` + immediate emplace pattern is preserved — minor perf concern).
- [ ] Concurrent `snapshot()` reads during an `update()` write never see partial state (use `loom` or stress test).
- [ ] Filter pipeline: an online level-80-warrior in zone Stormwind matches a `/who` request for "level 80 warrior stormwind"; the same player loading does not.
- [ ] Lower-case match: a player named "Áéíóú" matches a search for "áéíóú" after lowercasing.

---

## 11. Notes / gotchas

- **`Update()` is intentionally non-incremental**: the cost of detecting per-player deltas would dwarf the cost of a full rebuild. Don't try to make this incremental — it's 5-second-stale-by-design.
- **Lower-case both the player name and the guild name as wide-strings**. The C++ converts UTF-8 → UTF-16 → lower → keeps the wide form. The `/who` filter compares wide-string to wide-string. If you simplify to UTF-8 lower in Rust, document that searches for non-ASCII names may differ by a handful of codepoints from C++.
- **`_widePlayerName` is wide and lower; `_playerName` is UTF-8 and display-case** (used for the response packet). Same dual representation for guild. The Rust port should preserve the dual form to avoid converting per-`/who` query.
- **`PlayerLoading()` filter**: a session that has authenticated but is mid-`SMSG_LOGIN_VERIFY_WORLD` should not appear in `/who`. The C++ uses `WorldSession::PlayerLoading()` which is true between `HandlePlayerLogin` and the post-`SMSG_INITIAL_SPELLS`-or-equivalent point. Mirror this in Rust.
- **Guild GUID may be empty**: only set if `Player::GetGuild()` returns non-null. C++ assigns `ObjectGuid()` (zero) otherwise. Rust should use `Option<ObjectGuid>` or zero-guid sentinel.
- **`AccountTypes` (security level)**: `_security` is included so `/who` can hide GM accounts from non-GM searchers. The visibility filter is in the *consumer* (`HandleWhoOpcode`), not in `Update()` — every player goes into the snapshot regardless of GM status.
- **No tests in TrinityCore**: this module has no unit tests in the C++ tree either. The semantics are tiny enough that a regression here would be obvious from `/who` behaving wrong.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class WhoListStorageMgr` (singleton) | `pub struct WhoListStorage` + `Arc<WhoListStorage>` injected through `WorldServer` | Avoid global; pass through DI |
| `class WhoListPlayerInfo` | `pub struct WhoListPlayerInfo { … }` (#[derive(Clone)]) | All public fields or all getters — TC's choice of getters is C++-idiomatic; Rust public fields are fine for a POD |
| `WhoListInfoVector (typedef vector)` | `Vec<WhoListPlayerInfo>` | Wrapped in `ArcSwap` for lock-free reads |
| `Update()` | `update(&self, registry: &PlayerRegistry, guilds: &GuildMgr)` | Take both refs explicitly; no global state |
| `instance()` (Meyers singleton) | `OnceLock<Arc<WhoListStorage>>` if a global is required | Prefer DI |
| `std::wstring` (lower-cased) | `String` (UTF-8 lower-cased) | Mirror TC's wide-then-lower if exact behavior needed |
| `Utf8toWStr + wstrToLower` | `str::to_lowercase()` | Approximation; diverges for some Unicode codepoints — see gotcha |
| `ObjectGuid _guildguid` (default-constructed = zero) | `Option<ObjectGuid>` | Or `ObjectGuid::EMPTY` constant |
| `_security: AccountTypes` | `_security: AccountType` (enum from `wow-constants`) | — |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.
