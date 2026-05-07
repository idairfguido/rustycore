# Migration: Entities / Vehicle

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/game/Entities/Vehicle/`
> **Rust target crate(s):** `crates/wow-world/` (entity logic), `crates/wow-data/` (DBC), `crates/wow-constants/` (flags/opcodes)
> **Layer:** L4 (sub-modules)
> **Status:** ❌ not started
> **Audited vs C++:** ⚠️ partial (header-level audit only)
> **Last updated:** 2026-05-01

---

## 1. Purpose

`Vehicle` is the seat-aware controller attached to a `Unit` that has a vehicle kit (DBC `VehicleEntry`). It implements `TransportBase`, manages the `SeatMap` of passengers (player or creature), schedules delayed join events for spell-driven mounting, owns the accessory list (gunners, decorative passengers, multi-passenger NPCs like Wintergrasp siege engines, Oculus drakes, Ulduar vehicles), and routes passenger position updates back to the global frame.

---

## 2. C++ canonical files

<!-- REFINE.020:BEGIN canonical-file-coverage -->

### R2 canonical file coverage (generated)

> Fuente: `docs/migration/inventory/cpp-files-by-module.md`; C++ canonico: `/home/server/woltk-trinity-legacy/src/server/`. No valida que Rust este correcto.

| C++ file | Lines | Assignment basis |
|---|---:|---|
| `game/Entities/Vehicle/Vehicle.cpp` | 995 | `prefix` |
| `game/Entities/Vehicle/Vehicle.h` | 144 | `prefix` |
| `game/Entities/Vehicle/VehicleDefines.h` | 203 | `prefix` |

<!-- REFINE.020:END canonical-file-coverage -->

| File | Lines | Purpose |
|---|---|---|
| `src/server/game/Entities/Vehicle/Vehicle.h` | 144 | `Vehicle` class def; final, inherits `TransportBase`; `VehicleJoinEvent` |
| `src/server/game/Entities/Vehicle/Vehicle.cpp` | 995 | Install/Uninstall/Reset, AddVehiclePassenger, RemovePassenger, accessory install, immunity application |
| `src/server/game/Entities/Vehicle/VehicleDefines.h` | 203 | `PowerType`, `VehicleFlags`, `VehicleSpells`, `PassengerInfo`, `VehicleSeat`, `VehicleSeatAddon`, `VehicleAccessory`, `VehicleTemplate`, `SeatMap`, `TransportBase` |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Vehicle` | class (final, `TransportBase`) | Per-Unit seat controller |
| `VehicleJoinEvent` | class (BasicEvent) | Delayed mount completion |
| `TransportBase` | abstract class | Shared base for Vehicle + Transport (offset/global xform) |
| `VehicleSeat` | struct | One seat: `SeatInfo`, `SeatAddon`, `Passenger` |
| `VehicleSeatAddon` | struct | DB-driven seat orientation/exit overrides |
| `VehicleAccessory` | struct | Auto-summoned passenger spec |
| `VehicleTemplate` | struct | Per-entry despawn delay |
| `PassengerInfo` | struct | Guid + IsUninteractible + IsGravityDisabled |
| `PowerType` | enum | Vehicle power resources (Steam, Pyrite, Heat, Ooze, Blood, etc.) |
| `VehicleFlags` | enum | NO_STRAFE, NO_JUMPING, FULLSPEEDTURNING, ALLOW_PITCHING, FIXED_POSITION, etc. |
| `VehicleSpells` | enum | `RIDE_HARDCODED = 46598`, `PARACHUTE = 45472` |
| `VehicleExitParameters` | enum class | None / Offset / Dest |
| `Vehicle::Status` | private enum | NONE / INSTALLED / UNINSTALLING |

---

## 4. Critical public methods

| Symbol | Purpose | Calls into |
|---|---|---|
| `Vehicle(Unit*, VehicleEntry const*, uint32 creatureEntry)` | Construct from DBC | `Unit::SetVehicle` |
| `Install()` / `Uninstall()` | Lifecycle gates (status transitions) | `ApplyAllImmunities`, `RemoveAllPassengers` |
| `Reset(bool evading)` | Reset on creature respawn / evade | `InstallAllAccessories` |
| `InstallAllAccessories(bool evading)` | Spawn accessory creatures from `VehicleAccessoryContainer` | `InstallAccessory` |
| `InstallAccessory(entry, seatId, minion, type, summonTime)` | Summon one accessory NPC into a seat | `Map::SummonCreature`, `AddVehiclePassenger` |
| `AddVehiclePassenger(Unit*, int8 seatId)` | Schedule `VehicleJoinEvent`; -1 picks first empty | `GetNextEmptySeat`, `EventMgr` |
| `RemovePassenger(WorldObject*)` | Remove from seat, fire scripts, clear control aura | `Unit::RemoveAurasByType` |
| `RelocatePassengers()` | Push transport-frame moves to global | `TransportBase::CalculatePassengerPosition` |
| `GetSeatForPassenger(Unit const*)` | Lookup seat DBC entry | `SeatMap` find |
| `HasEmptySeat(int8)` / `GetAvailableSeatCount()` | Capacity queries | — |
| `GetNextEmptySeat(int8, bool next)` | Iterate seats wrap-around | — |
| `IsControllableVehicle()` | Any seat with `VEHICLE_SEAT_FLAG_CAN_CONTROL` | — |
| `GetDespawnDelay()` | From `VehicleTemplate` | — |
| `VehicleJoinEvent::Execute` | Two-phase apply on next tick (validates still alive, places passenger) | `Vehicle::AddPassenger` |

---

## 5. Module dependencies

**Depends on:**
- `Unit` (`_me` is the underlying unit; aura type 236 `SPELL_AURA_CONTROL_VEHICLE` drives mounts)
- `Map` (passenger summons, position updates)
- `SpellAuras` (`Aura::AURA_REMOVE_BY_*` paths trigger Uninstall)
- `DBCStores` (`VehicleEntry`, `VehicleSeatEntry`)
- `EventProcessor` (delayed `VehicleJoinEvent`)
- `ObjectMgr` (`VehicleAccessoryContainer`, `VehicleTemplateContainer`)

**Depended on by:**
- `Unit` (owns `Vehicle*` via `m_vehicleKit`)
- `WorldSession::HandleRequestVehicleExit`, `HandleRequestVehicleSwitchSeat`, `HandleEjectPassenger` (`VehicleHandler.cpp`)
- `MovementHandler` (transport frame in MOVEMENT_INFO)
- Scripts (siege engines, Ulduar/ICC vehicle bosses)

---

## 6. SQL / DB queries

`Vehicle.cpp` does not query directly. Tables are loaded by `ObjectMgr`/`SpellMgr`:

| Statement / Source | Purpose | DB |
|---|---|---|
| `vehicle_template` | per-entry despawn delay | world |
| `vehicle_template_accessory` | accessories by vehicle entry | world |
| `vehicle_accessory` | accessories by creature spawn | world |
| `vehicle_seat_addon` | seat orientation/exit overrides | world |

DBC stores:

| Store | What it loads | Read by |
|---|---|---|
| `VehicleStore` | `Vehicle.dbc` (kit definition, flags, seats[8]) | `Vehicle` ctor |
| `VehicleSeatStore` | `VehicleSeat.dbc` (per-seat flags, attachment offsets, exit) | `Vehicle::AddVehiclePassenger` |

---

## 7. Wire-protocol packets

| Opcode | Direction | Sent/Received in |
|---|---|---|
| `CMSG_REQUEST_VEHICLE_EXIT` | C → S | `WorldSession::HandleRequestVehicleExit` |
| `CMSG_REQUEST_VEHICLE_PREV_SEAT` / `_NEXT_SEAT` / `_SWITCH_SEAT` | C → S | seat shuffle |
| `CMSG_RIDE_VEHICLE_INTERACT` | C → S | NPC vehicle entry |
| `CMSG_EJECT_PASSENGER` | C → S | driver ejects passenger |
| `CMSG_MOVE_DISMISS_VEHICLE` | C → S | full dismount |
| `CMSG_MOVE_CHANGE_VEHICLE_SEATS` | C → S | client-side seat swap ack |
| `SMSG_CONTROL_VEHICLE` (on board) | S → C | via aura `SPELL_AURA_CONTROL_VEHICLE` |
| `SMSG_SET_VEHICLE_REC_ID` / `SMSG_MOVE_SET_VEHICLE_REC_ID` (+ ack) | S → C | switch vehicle kit while seated |

(See `crates/wow-constants/src/opcodes.rs` — opcode IDs already defined: `RequestVehicleExit`, `RequestVehicleSwitchSeat`, `MoveDismissVehicle`, `RideVehicleInteract`, `SetVehicleRecId`, etc.)

---

## 8. Current state in RustyCore

<!-- REFINE.021:BEGIN rust-target-coverage -->

### R2 Rust target coverage (generated)

> Fuente: cabecera `Rust target crate(s)` y seccion 8 del doc; verificado contra `/home/server/rustycore`. Esto solo valida existencia/estado del target Rust, no correccion funcional contra C++.

| Rust target | Kind | Rust files | Lines | Status | Notes |
|---|---|---:|---:|---|---|
| `crates/wow-world` | `crate_dir` | 17 | 12778 | `exists_active` | crate exists |
| `crates/wow-data` | `crate_dir` | 11 | 3505 | `exists_active` | crate exists |
| `crates/wow-constants` | `crate_dir` | 10 | 5477 | `exists_active` | crate exists |
| `crates/wow-constants/src/object.rs` | `file` | 1 | 173 | `exists_active` | file exists |
| `crates/wow-constants/src/opcodes.rs` | `file` | 1 | 1642 | `exists_active` | file exists |
| `crates/wow-packet/src/packets/movement.rs` | `file` | 1 | 461 | `exists_active` | file exists |

<!-- REFINE.021:END rust-target-coverage -->

**Files in `/home/server/rustycore`:**
- `crates/wow-constants/src/object.rs` — `TypeId::Vehicle = 9` defined; no flags
- `crates/wow-constants/src/opcodes.rs` — vehicle opcodes enumerated, no handlers
- `crates/wow-packet/src/packets/movement.rs` — `TransportInfo { vehicle_id: Option<i32> }` plumbed through `MOVEMENT_INFO`
- **0 lines** of `Vehicle` entity logic.

**What's implemented:** type-id constant + opcode constants + a vehicle_id field on movement info.

**What's missing vs C++:** entire 995-line `Vehicle.cpp` — seat map, passenger add/remove, accessory summon, transport-frame xform, immunities, despawn timer, join event scheduling.

**Suspicious / likely divergent:** none — nothing exists to diverge.

**Tests existing:** 0.

---

## 9. Migration sub-tasks

- [ ] **#VEH.1** Define `VehicleEntry` / `VehicleSeatEntry` DBC readers in `wow-data` (L)
- [ ] **#VEH.2** Port `VehicleFlags`, `PowerType`, `VehicleExitParameters`, `VehicleSpells` to `wow-constants` (L)
- [ ] **#VEH.3** Define `VehicleSeat`, `VehicleSeatAddon`, `VehicleAccessory`, `VehicleTemplate`, `PassengerInfo` POD types in `wow-world` (L)
- [ ] **#VEH.4** Port `TransportBase` xform (`CalculatePassengerPosition`/`Offset`) as free fns in `wow-math` (L)
- [ ] **#VEH.5** Implement `Vehicle` struct + `Install/Uninstall/Reset` lifecycle (M)
- [ ] **#VEH.6** Implement `AddVehiclePassenger`/`RemovePassenger`/`HasEmptySeat`/`GetNextEmptySeat` (M)
- [ ] **#VEH.7** Implement `InstallAllAccessories` + `InstallAccessory` (depends on TempSummon) (M)
- [ ] **#VEH.8** Implement delayed join (`VehicleJoinEvent` analog) on session/map tick (M)
- [ ] **#VEH.9** Wire vehicle opcodes (Exit/SwitchSeat/Eject/Dismiss) into `wow-handler` (M)
- [ ] **#VEH.10** Apply immunities (`ApplyAllImmunities` from VehicleEntry flags) (L)
- [ ] **#VEH.11** Hook `RelocatePassengers` into MapManager tick (M)

---

## 10. Regression tests to write

- [ ] Test: empty vehicle has correct `GetAvailableSeatCount` from DBC seat array
- [ ] Test: `AddVehiclePassenger(seatId=-1)` picks first empty in DBC index order
- [ ] Test: passenger removal clears seat and fires control-aura removal path
- [ ] Test: `CalculatePassengerPosition(offset, transO)` round-trips with `CalculatePassengerOffset`
- [ ] Test: accessory install summons N creatures matching `vehicle_accessory` rows
- [ ] Test: `IsControllableVehicle` true iff any seat has `CAN_CONTROL` flag

---

## 11. Notes / gotchas

- Aura 236 `SPELL_AURA_CONTROL_VEHICLE` is the single drive of mount/dismount — `Vehicle::AddVehiclePassenger` is **always** invoked from aura apply, never directly from a CMSG. Don't write a "mount handler" — write an aura effect.
- `VehicleJoinEvent` exists because aura apply happens mid-spell-cast and you cannot complete the seat reservation synchronously without re-entering the spell system. Mirror this: defer the actual seat write by one tick.
- `_status` (NONE/INSTALLED/UNINSTALLING) gates against double-uninstall during script callbacks. Mirror with an enum, not a bool.
- WoLK 3.4: `VehicleSeat.dbc` flags include `CAN_ENTER_OR_EXIT`, `CAN_CONTROL`, `CAN_ATTACK`, `UNCONTROLLED`, `KICKABLE_BY_HOSTILE`. Don't drop `UNCONTROLLED` — used by Oculus drakes.
- Hardcoded spell `46598` (RIDE_HARDCODED) is the generic dummy ride aura applied when scripts force-mount.

---

## 12. C++ → Rust mapping

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class Vehicle : TransportBase` | `struct Vehicle` + `impl TransportBase for Vehicle` | trait, not class |
| `Unit* _me` | `ObjectGuid` + lookup via `MapManager` | avoid back-pointer |
| `SeatMap = std::map<int8, VehicleSeat>` | `BTreeMap<i8, VehicleSeat>` | seat order matters |
| `VehicleEntry const*` | `&'static VehicleEntry` from DBC store | — |
| `VehicleJoinEvent : BasicEvent` | enqueue on map's `pending_events: Vec<DelayedEvent>` | tick-deferred |
| `GuidSet vehiclePlayers` | `HashSet<ObjectGuid>` | parking_lot or per-map lock |
| `std::list<VehicleJoinEvent*> _pendingJoinEvents` | `Vec<VehicleJoinEvent>` | small N |
| `Status _status` enum | `enum VehicleStatus { None, Installed, Uninstalling }` | direct |

---

## 13. §13 Audit (vs `/home/server/woltk-trinity-legacy/`)

| C++ symbol | Found in Rust | File | Verdict |
|---|---|---|---|
| `class Vehicle` | no | — | ❌ missing |
| `class VehicleJoinEvent` | no | — | ❌ missing |
| `class TransportBase` | no | — | ❌ missing |
| `enum VehicleFlags` | no | — | ❌ missing |
| `enum PowerType` (vehicle powers) | no | — | ❌ missing |
| `struct VehicleSeat` / `VehicleSeatAddon` / `VehicleAccessory` / `VehicleTemplate` / `PassengerInfo` | no | — | ❌ missing |
| `enum VehicleExitParameters` | no | — | ❌ missing |
| `VEHICLE_SPELL_RIDE_HARDCODED` (46598) | no | — | ❌ missing |
| `TypeId::Vehicle = 9` | yes | `crates/wow-constants/src/object.rs` | ✅ present (constant only) |
| `CMSG_REQUEST_VEHICLE_*` opcodes | yes (constants) | `crates/wow-constants/src/opcodes.rs` | ⚠️ enumerated, no handler |
| `MOVEMENT_INFO::Transport.vehicle_id` | yes | `crates/wow-packet/src/packets/movement.rs` | ✅ wire-format only |

**Verdict:** ❌ not started. Surface coverage ≈ 1% (constants and a movement-info field). No entity, no seats, no accessories, no handlers.
