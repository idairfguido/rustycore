# Migration: Time (GameTime + UpdateTime + WowTime)

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Time/`
> **Rust target crate(s):** `crates/wow-core/` (already houses a partial `GameTime`)
> **Layer:** L0 (foundation — every module that ticks or timestamps something depends on this)
> **Status:** ⚠️ partial (RustyCore has a `GameTime` and `ServerTime` in `wow-core`, but lacks the global singleton-update model, `WowTime`, `UpdateTime`, and `Timezone` integration)
> **Audited vs C++:** ✅ complete (6 small files, ~520 lines)
> **Last updated:** 2026-05-01

---

## 1. Purpose

Three concerns bundled together:

- **`GameTime` namespace** — the single, server-wide "current time" snapshot, refreshed once per main-loop tick by `UpdateGameTimers()`. Keeps `time_t`, `getMSTime()`, `system_clock::time_point`, `steady_clock::time_point`, a cached `localtime_r` `tm`, plus a UTC `WowTime` and a local-zone-shifted `WowTime`. Everything else asks `GameTime::GetGameTime()` instead of calling `time(nullptr)` directly so that all tick-side computations agree on "now".
- **`WowTime`** — the WoW client's packed time format (`year/month/day/weekday/hour/minute/flags/holidayOffset`). Used over the wire for calendar events, mail expiry, auctions, and BroadcastText timestamps. Has comparison, addition of `Seconds`, and `ByteBuffer` serialization.
- **`UpdateTime` / `WorldUpdateTime`** — a tiny accumulator that records the most recent main-loop diff for diagnostic / display purposes (`.server info`, performance tools).

---

## 2. C++ canonical files

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Time/GameTime.h` | 60 | Namespace declaration |
| `src/server/game/Time/GameTime.cpp` | 113 | Static state + accessors + `UpdateGameTimers()` |
| `src/server/game/Time/WowTime.h` | 91 | `WowTime` class with packed-time accessors, `<=>`, `+= Seconds`, ByteBuffer ops |
| `src/server/game/Time/WowTime.cpp` | ~155 | All WowTime impls (read but not pasted here; standard packed-time math) |
| `src/server/game/Time/UpdateTime.h` | 46 | `UpdateTime` base + `WorldUpdateTime` (extern global `sWorldUpdateTime`) |
| `src/server/game/Time/UpdateTime.cpp` | 37 | `UpdateWithDiff(uint32)` and `GetLastUpdateTime()` |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `GameTime` | namespace | Global "current time" snapshot |
| `WowTime` | class | Packed wire-format time |
| `UpdateTime` | class | Last-diff accumulator (base) |
| `WorldUpdateTime` | class | Trivial subclass; one global instance `sWorldUpdateTime` |

State held by `GameTime` (namespace-private, not in the header):

| Variable | Type | Meaning |
|---|---|---|
| `StartTime` | `time_t const` | Server boot time |
| `GameTime` | `time_t` | Current time (Unix seconds) |
| `GameMSTime` | `uint32` | Millisecond counter since process start (`getMSTime()`) |
| `GameTimeSystemPoint` | `SystemTimePoint` | `chrono::system_clock::now()` |
| `GameTimeSteadyPoint` | `TimePoint` | `chrono::steady_clock::now()` |
| `DateTime` | `tm` | `localtime_r(GameTime, ...)` cached |
| `UtcWow` | `WowTime` | WowTime constructed from UTC |
| `Wow` | `WowTime` | UtcWow shifted by system timezone offset |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `GameTime::GetStartTime()` | Server boot timestamp (constant after init) | — |
| `GameTime::GetGameTime()` | Cached `time_t` of "current" tick | — |
| `GameTime::GetGameTimeMS()` | Cached millisecond counter | — |
| `GameTime::GetSystemTime()` | Cached `system_clock::time_point` | — |
| `GameTime::Now()` | Cached `steady_clock::time_point` | — |
| `GameTime::GetTime<Clock>()` | Compile-time clock dispatch (specialized for `system_clock` and `steady_clock`) | — |
| `GameTime::GetUptime()` | `GameTime - StartTime` in seconds | — |
| `GameTime::GetDateAndTime()` | Pointer to cached `tm` | — |
| `GameTime::GetUtcWowTime()` / `GetWowTime()` | Pointers to cached UTC and local `WowTime` | — |
| `GameTime::UpdateGameTimers()` | Re-snapshot all of the above; called once per main-loop iteration | `time()`, `getMSTime()`, `chrono::*::now()`, `localtime_r`, `Trinity::Timezone::GetSystemZoneOffsetAt` |
| `WowTime::GetPackedTime()` / `SetPackedTime(u32)` | Encode/decode the 32-bit wire format | — |
| `WowTime::GetUnixTimeFromUtcTime()` / `SetUtcTimeFromUnixTime(time_t)` | Round-trip with `time_t` | — |
| `WowTime::IsInRange(from, to)` | Range check (used for daily/weekly resets) | — |
| `WowTime::operator+= Seconds` / `+ Seconds` | Time arithmetic | — |
| `WowTime::operator<<` / `>>` (ByteBuffer) | Wire serialization (packed u32 either side) | — |
| `UpdateTime::UpdateWithDiff(u32)` | Store the latest tick diff | — |
| `UpdateTime::GetLastUpdateTime()` | Read it back | — |

---

## 5. Module dependencies

**Depends on:**
- `Common/Timer.h` — `getMSTime()`, `SystemTimePoint`, `TimePoint` typedefs.
- `Common/Timezone.h` — `Trinity::Timezone::GetSystemZoneOffsetAt(SystemTimePoint)`.
- C++ `<ctime>`, `<chrono>`.

**Depended on by:** essentially everything that has a tick or a timestamp:
- All `Update(diff)` methods (indirectly via tick scheduling).
- `Mail` (expiry), `Auction` (bid expiry), `Calendar` (event start), `BattlePets` (cooldowns), `BlackMarket` (auctions), `Quests` (daily/weekly reset), `Achievements` (criteria timestamps).
- `Log` for log-line timestamps.
- `Maps/Map.cpp`, `Maps/MapManager.cpp` — call `UpdateGameTimers()` indirectly via the world tick.
- `World::Update(diff)` — calls `sWorldUpdateTime.UpdateWithDiff(diff)`.

---

## 6. SQL / DB queries (if any)

None. This module never touches the database. It also doesn't read DBC/DB2.

---

## 7. Wire-protocol packets (if any)

No opcodes are owned by this module, but `WowTime`'s packed format is sent inside many opcodes:

- `SMSG_LOGIN_SET_TIME_SPEED` (game time + speed sync on login).
- Calendar event start/end times in `SMSG_CALENDAR_*`.
- Mail expiry timestamps in `SMSG_MAIL_LIST_RESULT`.
- Auction expiry in `SMSG_AUCTION_LIST_RESULT`.
- Many `BroadcastText` timestamps.
- Holiday/event `WowTime` ranges in DB2-derived data.

The `ByteBuffer << WowTime` overload is the canonical encoder for all of these.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-core/src/time.rs` — `pub struct GameTime(u64)` (Unix-seconds wrapper) with `now()`, `from_unix`, `as_secs`, `to_packed`, `has_passed`, `time_until`, `add_secs`. Also `pub struct ServerTime` wrapping `Instant` for elapsed-ms.
- `crates/wow-packet/src/packets/misc.rs:415` uses `wow_core::GameTime::now().to_packed()` for `SMSG_LOGIN_SET_TIME_SPEED`.

**What's implemented:**
- A minimal `GameTime` value type (Unix seconds + crude packed-time encoder).
- A `ServerTime` for "monotonic elapsed since this object was created".

**What's missing vs C++:**
- The **global tick snapshot** — RustyCore's `GameTime::now()` calls `SystemTime::now()` every time, which means two reads in the same tick can disagree by some microseconds. C++'s model is to refresh once per tick and read the cached value everywhere. This affects determinism and (less importantly) performance.
- `WowTime` proper — the existing `to_packed` on `GameTime` is described as approximate ("not accounting for all edge cases"); there's no `WowTime` struct with the documented field-by-field wire layout, no `IsInRange`, no `+= Seconds`, no ByteBuffer serializer.
- `UpdateTime` / `WorldUpdateTime` — there is no global "last tick diff" accumulator.
- `Timezone` integration — UtcWow vs Local Wow is unimplemented; `to_packed()` doesn't even know the local timezone.
- `chrono` integration is split between `SystemTime` and `Instant` but never cached.

**Suspicious / likely divergent:**
- The `to_packed` impl (`time.rs:59-80`) computes month/day with `(days / 30)` integer math — that drifts a couple of days per month vs `localtime_r`. For low-stakes uses (chat timestamp display) it's fine; for calendar event start times it will misfire. Treat as broken until replaced.
- Year encoding in `to_packed` uses `year - 100`, suggesting a 1900-anchored offset. The C++ `WowTime` packed format anchors on year-2000 (the WoW client interprets `year` as "years since 2000"). This is plausibly already wrong.

**Tests existing:**
- ~6 unit tests in `crates/wow-core/src/time.rs` for the existing API surface (line 140+); none of them validate the packed-time encoding against the client's expected values.

---

## 9. Migration sub-tasks

- [ ] **#TIM.1** Add a `WorldClock` (or `GameClock`) struct in `wow-core` that holds the snapshot fields (`unix_secs`, `ms_since_start`, `system_now`, `steady_now`, `local_tm: chrono::DateTime<Local>`, `utc_wow: WowTime`, `local_wow: WowTime`) behind an `Arc<RwLock<...>>` or `ArcSwap`. Add `update_now()` to refresh all of them atomically. (complexity: **M**)
- [ ] **#TIM.2** Wire `WorldClock::update_now()` into the world-server main tick (immediately after `tick.recv()`). Replace ad-hoc `SystemTime::now()` calls in handler/tick code with `clock.unix_secs()` etc. (complexity: **M**)
- [ ] **#TIM.3** Replace the approximate `GameTime::to_packed` with a real `WowTime` struct mirroring `WowTime.h`: `year, month, month_day, week_day, hour, minute, flags, holiday_offset`. Implement `to_packed_u32` / `from_packed_u32` to bit-exact spec. Use `chrono::NaiveDateTime` for the `year/month/day/hour/minute` decomposition; **anchor year on 2000**, not 1970, not 1900. (complexity: **M**)
- [ ] **#TIM.4** Implement `WowTime::is_in_range(from, to)` mirroring the C++ semantics (uses lex-order on the packed fields). Implement `WowTime + Seconds` / `WowTime - Seconds` via round-trip through Unix time. (complexity: **L**)
- [ ] **#TIM.5** Add `WowTime` (de)serialization on top of the existing `wow-packet` `WorldPacket` writer (`write_u32(packed)`, `read_u32 → WowTime`). Replace the misc.rs:415 call with `clock.local_wow().to_packed()`. (complexity: **L**)
- [ ] **#TIM.6** Add `WorldUpdateTime` in `wow-world` (or as a field on the world tick state): `last_diff_ms: AtomicU32`. Update on every tick; expose for `.server info` style commands. (complexity: **L**)
- [ ] **#TIM.7** Local timezone offset: integrate via `chrono::Local` or `chrono-tz`. Document that the offset is computed once per snapshot, not once per server boot — DST transitions during long uptime must update the local `WowTime`. (complexity: **L**)

---

## 10. Regression tests to write

- [ ] `WowTime::to_packed` → `from_packed` round-trips for a battery of dates spanning 2000-2030.
- [ ] `WowTime::to_packed` matches a known fixture from a captured `SMSG_LOGIN_SET_TIME_SPEED` packet (capture from a real client, paste into test).
- [ ] `WowTime + Duration::from_secs(N)` matches `WowTime::set_utc_from_unix(unix + N)` for many N including DST boundaries.
- [ ] `is_in_range`: weekly reset at Tuesday 08:00 UTC excludes Tuesday 07:59:59 and includes Tuesday 08:00:01.
- [ ] `WorldClock::update_now()` is monotonic in `steady_now` even when `system_now` jumps backwards (NTP correction).
- [ ] After `WorldClock::update_now()`, two reads of `unix_secs()` 100 ms apart return the same value (proves snapshot semantics).
- [ ] `WorldUpdateTime::last_diff_ms` reflects the most recent diff.

---

## 11. Notes / gotchas

- **C++ uses three `time_t`s and two `chrono::*::time_point`s in lockstep**. Don't model this with `SystemTime::now()` everywhere — even microsecond skew across the codebase would make race tests non-deterministic.
- **`WowTime` packs into a single `uint32`** — bit layout is documented in `WowTime.cpp`. Year anchor is **year-2000** in the packed encoding (the field stores "years since 2000"). Confirm against `WowTime.cpp` before committing the constant.
- **DST**: `Trinity::Timezone::GetSystemZoneOffsetAt(SystemTimePoint)` returns the offset *at a specific point* (handles DST). The Rust port must do the same — naively cached offset breaks twice a year for European/American servers.
- **`tm` from `localtime_r` is in local time**; the cached `DateTime` is the local view. UTC WowTime is constructed from `time_t` (UTC) via `SetUtcTimeFromUnixTime`, then shifted by `+offset` to derive the local `Wow`. Mirror this — don't construct local first and then "subtract DST" by hand.
- **`sWorldUpdateTime`** is a global. RustyCore should resist that — pass a `&UpdateTime` through the world tick context. There's only one user (`.server info`) and it's optional anyway.
- **`getMSTime()` is `uint32` and wraps every ~49 days**. Trinity's diff math uses `GetMSTimeDiffToNow(old)` to handle the wrap. If you keep the milliseconds-since-start counter, the Rust equivalent should similarly be wraparound-safe — easier path: use `Instant` and only convert to `u32` ms when the wire requires it.
- **Server-process restart resets `StartTime`** but not `GameTime` (which is wall-clock). Any persistence that uses uptime instead of wall-clock breaks across restarts.
- **WoLK 3.4.3 specific**: BroadcastText event timestamps and a few calendar packets use `WowTime` not `time_t`. Get the encoding right or holidays will be off.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `namespace GameTime` (with mutable globals) | `pub struct WorldClock { snapshot: ArcSwap<TimeSnapshot> }` | Immutable snapshot, atomically swapped on tick |
| `time_t GameTime` | `pub fn unix_secs(&self) -> i64` (or u64) | — |
| `uint32 GameMSTime` | `pub fn ms_since_start(&self) -> u32` (wraps) | Or `Duration since start` for safety |
| `SystemTimePoint GameTimeSystemPoint` | `chrono::DateTime<Utc>` | — |
| `TimePoint GameTimeSteadyPoint` | `Instant` | — |
| `tm DateTime` | `chrono::DateTime<Local>` | — |
| `WowTime UtcWow` / `Wow` | `pub struct WowTime { … }` two instances on snapshot | — |
| `class WowTime` | `pub struct WowTime { year: i32, month: i8, … }` | Implement `Ord`, `Add<Duration>`, packed (de)ser |
| `UpdateTime` | `pub struct UpdateTime { last_diff_ms: AtomicU32 }` | Plain field would also work |
| `extern WorldUpdateTime sWorldUpdateTime` | `Arc<UpdateTime>` injected into world tick | No globals |
| `Trinity::Timezone::GetSystemZoneOffsetAt(...)` | `chrono::Local.offset_from_utc_datetime(...)` or `chrono-tz` | Built-in is fine |

---

*Template version: 1.0 (2026-05-01).*
