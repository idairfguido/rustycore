# Migration: proto (Battle.net RPC protobuf)

> **C++ canonical path:** `src/server/proto/`
> **Rust target crate(s):** `crates/wow-proto/`
> **Layer:** L1 (infrastructure — consumed by `bnet-server`, `wow-network`, `wow-handler`)
> **Status:** ⚠️ partial — 11 of ~50 services + the two real `.proto` sources (Login, RealmList) are not yet ported
> **Audited vs C++:** ⚠️ partial
> **Last updated:** 2026-05-01

---

## 1. Purpose

This module owns the **Battle.net RPC wire format** and its supporting infrastructure: protobuf message types, the abstract `ServiceBase` dispatch interface, and the catalogue of Battle.net error codes. Battle.net is the connection layer the WoW 3.4.3 client uses for authentication, game-account selection, realm listing, friends, presence, club (guild/community), reporting, voice, etc.; it is a length-prefixed binary RPC where each request carries a `Header` protobuf identifying the target service (by 32-bit hash) and method id, followed by a service-specific payload.

The C++ tree contains both the **canonical service definitions** (a couple of `.proto` files plus a large body of pre-generated `.pb.{h,cc}` artefacts whose original `.proto` sources live outside the public Trinity tree — they're Blizzard's `bgs.low` protocol surface) and the **base classes for service implementation** (`ServiceBase`, error code enum). RustyCore re-derives this surface using the `prost` crate and a small set of recovered `.proto` files, plus hand-written service hash + status code constants.

---

## 2. C++ canonical files

Todas las rutas relativas a `/home/server/woltk-trinity-legacy/`.

| File | Lines (aprox) | Purpose |
|---|---|---|
| `src/server/proto/CMakeLists.txt` | 50 | Builds `proto` target — collects all `.pb.{cc,h}` + `ServiceBase.cpp` |
| `src/server/proto/ServiceBase.h` | 66 | `class ServiceBase` — abstract dispatch superclass for every RPC service. `CallServerMethod`, `SendRequest`, `SendResponse`, logging helpers |
| `src/server/proto/ServiceBase.cpp` | 69 | Logging helpers + continuation factory |
| `src/server/proto/BattlenetRpcErrorCodes.h` | **671** | `enum BattlenetRpcErrorCode : uint32` — ~330 named error codes (`ERROR_OK=0`, `ERROR_DENIED=3`, `ERROR_GAME_ACCOUNT_BANNED`, `ERROR_WOW_SERVICES_*` family, etc.) |
| `src/server/proto/PrecompiledHeaders/protoPCH.h` | <100 | PCH stub |
| `src/server/proto/Login/Login.proto` | 84 | **Real `.proto` source.** JSON-over-HTTP login form: `LoginForm`, `SrpLoginChallenge`, `LoginResult`, `GameAccountList`, etc. |
| `src/server/proto/Login/Login.pb.{cc,h}` | (gen) | Generated artefacts |
| `src/server/proto/RealmList/RealmList.proto` | 83 | **Real `.proto` source.** Realm browser ticket and update messages: `RealmListTicketIdentity`, `ClientInformation`, `RealmEntry`, `RealmState`, `RealmListUpdates`, `RealmListServerIPAddresses`, `IPAddress` |
| `src/server/proto/RealmList/RealmList.pb.{cc,h}` | (gen) | Generated artefacts |
| `src/server/proto/Client/club_service.proto` | 293 | **Real `.proto` source.** `service ClubService` — the only RPC service whose `.proto` is checked in. Methods like `CreateClub`, `JoinClub`, `LeaveClub`, `GetClub`, `SubscribeClub`, etc. |
| `src/server/proto/Client/club_listener.proto` | 121 | **Real `.proto` source.** `service ClubListener` — server-pushed notifications mirror of ClubService |
| `src/server/proto/Client/*.pb.{cc,h}` | (gen) | **53 distinct services**, all generated from `.proto` files **not present** in this tree (Blizzard internal `bgs.low` definitions). Includes account, attribute, authentication, challenge, channel, club_*, connection, content_handle, embed, entity, ets, event_view, friends, game_utilities, invitation, message, notification, presence, profanity_filter_config, report, resource, role, rpc_config, rpc_types, semantic_version, user_manager, voice |
| `src/server/proto/Client/api/` | (gen) | `field_options.pb.{cc,h}`, `message_options.pb.{cc,h}`, `method_options.pb.{cc,h}`, `range.pb.{cc,h}`, `register_method_types.pb.{cc,h}`, `routing.pb.{cc,h}`, `service_options.pb.{cc,h}` — protobuf custom-options metadata used by the codegen |
| `src/server/proto/Client/global_extensions/` | (gen) | (additional codegen extensions) |

**The asymmetry to note:** only **4 `.proto` source files** are checked in (`Login`, `RealmList`, `club_service`, `club_listener`). The other ~50 services exist only as their pre-generated `.pb.cc` / `.pb.h` artefacts. To extend the Rust port beyond what's already there, one must either re-derive the `.proto` schema by reverse-engineering the `.pb.h` (manageable for small services, painful for the full set) or obtain Blizzard's authoritative `bgs.low` protos out-of-band.

---

## 3. Classes / Structs / Enums

### From `ServiceBase.h`

| Symbol | Kind | Purpose |
|---|---|---|
| `ServiceBase` | abstract class | Common base for every generated `*Service` class. Holds `service_hash_`, declares `CallServerMethod` (dispatcher), `SendRequest`/`SendResponse` (transport), and logging helpers |

### From `BattlenetRpcErrorCodes.h`

| Symbol | Kind | Purpose |
|---|---|---|
| `BattlenetRpcErrorCode` | enum uint32 | ~330 named error codes covering: `ERROR_OK`, generic errors (`ERROR_INTERNAL/TIMED_OUT/DENIED/NOT_EXISTS/INVALID_ARGS/NOT_IMPLEMENTED/...`), authentication (`ERROR_AUTHENTICATION_*`), game account state (`ERROR_GAME_ACCOUNT_*`), parental controls, geographic restrictions, WoW-specific (`ERROR_WOW_SERVICES_*`: realm down, banned, account suspended, ...), club/community errors, friends errors, presence, reporting, voice, etc. |

### From `Login/Login.proto`

| Symbol | Kind | Purpose |
|---|---|---|
| `ErrorResponse` | message | Empty error placeholder |
| `FormType` | enum | `LOGIN_FORM = 1` |
| `FormInput` | message | `input_id`, `type`, `label`, `max_length` — one form field |
| `FormInputs` | message | List of form fields + SRP6 url/javascript |
| `FormInputValue` | message | Submitted value pair |
| `LoginForm` | message | Submitted form payload (`platform_id`, `program_id`, `version`, `inputs[]`) |
| `SrpLoginChallenge` | message | SRP6 challenge from server (`version`, `iterations`, `modulus`, `generator`, `hash_function`, `username`, `salt`, `public_B`) |
| `AuthenticationState` | enum | `LOGIN / LEGAL / AUTHENTICATOR / DONE` |
| `LoginResult` | message | Final auth response with state, session token, account info |
| `LoginRefreshResult` | message | Token refresh |
| `GameAccountInfo` | message | Per-game-account metadata |
| `GameAccountList` | message | List of game accounts on the BNet account |

### From `RealmList/RealmList.proto`

| Symbol | Kind | Purpose |
|---|---|---|
| `RealmListTicketIdentity` | message | `gameAccountID`, `gameAccountRegion` |
| `ClientVersion` | message | `versionMajor/Minor/Revision/Build` |
| `ClientInformation` | message | `platform`, `buildVariant`, `type`, `timeZone`, `currentTime`, `textLocale`, `audioLocale`, `versionDataBuild`, `version`, `secret`, `clientArch`, `systemVersion`, `platformType`, `systemArch` |
| `RealmListTicketClientInformation` | message | Wrapper around `ClientInformation` |
| `RealmCharacterCountEntry` | message | `wowRealmAddress` + `count` |
| `RealmCharacterCountList` | message | Repeated `RealmCharacterCountEntry` |
| `RealmEntry` | message | One realm row: address, timezone, population state, category, version, flags, name, configs |
| `RealmState` | message | Realm up/down state |
| `RealmListUpdates` | message | Repeated `RealmState` |
| `IPAddress` | message | IPv4/IPv6 address |
| `RealmIPAddressFamily` | message | One IP family |
| `RealmListServerIPAddresses` | message | Repeated `RealmIPAddressFamily` |

### From `Client/club_service.proto` and `club_listener.proto`

| Symbol | Kind | Purpose |
|---|---|---|
| `service ClubService` | service | All "client → server" club operations (CRUD on clubs, members, invitations, streams, messages, roles, bans, tags, ranges) |
| `service ClubListener` | service | Server-pushed notifications mirroring the same domain |

### Implicit (in `bgs.low` generated artefacts)

The generated `.pb.h` set defines hundreds of message types and a similar number of service methods. Spot-check from filenames:

- `account_service.pb.h` — `AccountService` (account info queries, link types, GameAccount info)
- `account_types.pb.h` — `AccountId`, `GameAccountHandle`, etc.
- `attribute_types.pb.h` — `Attribute`, `Variant` (key→value generic payload)
- `authentication_service.pb.h` / `authentication_listener` — login flow
- `challenge_service.pb.h` — captcha-like challenges (web-pin, sms, etc.)
- `channel_types.pb.h` — chat channel metadata
- `club_*` — community/club system (most widely-implemented BNet service after auth)
- `connection_service.pb.h` — `Connect`, `Bind`, `KeepAlive`, `RequestDisconnect`
- `friends_service.pb.h` / `friends_types.pb.h` — friend lists, invitations
- `game_utilities_service.pb.h` — generic `ClientRequest` / `ClientResponse` used by realm list flow + many other ad-hoc commands
- `invitation_types.pb.h` — generic invitation envelope (party, friend, club)
- `message_types.pb.h` — generic message envelope
- `notification_types.pb.h` — generic notification envelope
- `presence_service.pb.h` / `presence_listener.pb.h` — online/idle/AFK + rich presence
- `profanity_filter_config.pb.h` — chat profanity rules
- `report_service.pb.h` / `report_types.pb.h` — abuse reporting
- `resource_service.pb.h` — resource (file blob) handle lookup
- `rpc_types.pb.h` — `Header`, `ProcessId`, `NoData`, `EntityId`, `MethodOptions`
- `semantic_version.pb.h` — version triples
- `user_manager_service.pb.h` — block list, ignore list
- `voice_types.pb.h` — voice channel metadata

---

## 4. Critical public methods / functions

### `ServiceBase`

| Symbol | Purpose | Calls into |
|---|---|---|
| `virtual void CallServerMethod(uint32 token, uint32 methodId, MessageBuffer)` | Top-level dispatch. Generated subclasses override and switch on `methodId` | per-method `Handle*` virtual |
| `virtual void SendRequest(uint32 hash, uint32 method, Message*)` | Send fire-and-forget RPC | transport |
| `virtual void SendRequest(uint32 hash, uint32 method, Message*, std::function<void(MessageBuffer)>)` | Send + register continuation | transport + token registry |
| `virtual void SendResponse(uint32 hash, uint32 method, uint32 token, uint32 status)` | Send error response | transport |
| `virtual void SendResponse(uint32 hash, uint32 method, uint32 token, Message*)` | Send success response | transport |
| `CreateServerContinuation(token, methodId, methodName, outputDescriptor)` | Build callback closure that serialises the response into the right outbound envelope | logging + `SendResponse` |
| `LogCallServerMethod`, `LogCallClientMethod`, `LogUnimplementedServerMethod`, `LogInvalidMethod`, `LogFailedParsingRequest`, `LogDisallowedMethod` | Standardised logging | `TC_LOG_*` macros |
| `GetServiceHash() const -> uint32` | Identify which service this instance handles | — |

### Generated services (pattern)

Each generated `XxxService` exposes:

- `static char const* GetName()` — service name string for logging
- `static uint32 GetServiceHash()` — usually `HashFnv1a(name)`
- `void XxxMethod(MethodInputType const&, std::function<void(MethodOutputType const&)>)` — server-side handler stub
- `void XxxMethod(MethodInputType const&)` — fire-and-forget client-side caller

---

## 5. Module dependencies

**Depends on:**
- `protobuf` (Google library, `MessageLite`, `Message`)
- `MessageBuffer` (TC's network buffer wrapper, in `src/server/shared/Networking/`)
- `Define.h` (for `uint32` typedefs and `TC_PROTO_API` export macro)
- (transitively) `boost::asio` callbacks via `MessageBuffer`

**Depended on by:**
- `bnet-server` — the entire executable is a thin shell over a few `*Service` subclasses
- `worldserver` — uses `BattlenetRpcErrorCode` for some error reporting back to the BNet realm-list flow
- `WorldServerSession` (for the realm-list ticket) — uses `RealmListTicketIdentity`, `RealmListTicketClientInformation`, `ClientInformation`
- `RealmList` server (separate component in TC) — uses `RealmEntry`, `RealmState`, `RealmListServerIPAddresses`, `IPAddress`
- Login flow — uses `Login.pb.h` types, the JSON-over-HTTP form layer
- Account / Friends / Club / Presence services (all the BNet RPC handlers) — use the corresponding generated types

---

## 6. SQL / DB queries (if any)

The proto module itself issues **no** SQL. It defines the wire types used by upstream BNet RPC handlers, which then query `auth` / `bnet_account` tables; those queries live in the consumer code (Account/Friends/Club services), not here.

No DBC/DB2 stores are owned here either.

---

## 7. Wire-protocol packets (if any)

The entire module **is** the wire protocol — but for **Battle.net RPC**, not the WoW world-protocol opcodes. There are no `OPCODE_*` symbols here; instead, the unit of transmission is:

```
[ Header (varint-length-prefixed protobuf) ][ Payload (length-prefixed protobuf) ]
```

The `Header` is `bgs.protocol.Header` (in `rpc_types.pb.h`), with fields:
- `service_id` (uint8 — local) or `service_hash` (uint32 — global)
- `method_id` (uint32)
- `token` (uint32 — request/response correlation)
- `status` (uint32 — error code on responses; absent on requests)
- `size` (uint32 — payload size)

| "Opcode" | Direction | Sent/Received in |
|---|---|---|
| `service_hash = AUTHENTICATION_SERVICE` (0x0DECFC01), method 1 = `Logon` | client → server | `AuthenticationService::Logon` |
| `service_hash = CONNECTION_SERVICE` (0x65446991), method 1 = `Connect` | client → server | `ConnectionService::Connect` |
| `service_hash = CHALLENGE_SERVICE`, method 3 = `OnChallenge*` | server → client | `ChallengeService` |
| `service_hash = ACCOUNT_SERVICE` (0x62DA0891) | client → server | `AccountService` |
| `service_hash = GAME_UTILITIES_SERVICE` (0x3FC1274D), method 1 = `ProcessClientRequest` | client → server | Realm list / generic command channel |
| `service_hash = AUTHENTICATION_LISTENER` (0x71240E35) | server → client | Logon push notifications |
| `service_hash = ACCOUNT_LISTENER` (0x54DFDA17) | server → client | Account info changes |
| `service_hash = FRIENDS_SERVICE` (0xA3DDB1BD) / `FRIENDS_LISTENER` (0x6F259A13) | both | Friends |
| `service_hash = PRESENCE_SERVICE` (0xFA0796FF) / `PRESENCE_LISTENER` (0x890AB85F) | both | Presence |
| `service_hash = REPORT_SERVICE` (0x7CAF61C9) / `REPORT_SERVICE_V2` (0x3A4218FB) | client → server | Abuse reports |
| `service_hash = RESOURCES_SERVICE` (0xECBE75BA) | client → server | Blob fetch |
| `service_hash = USER_MANAGER_SERVICE` (0x3E19268A) / `USER_MANAGER_LISTENER` (0xBC872C22) | both | Block lists |
| `Login.proto` payloads | HTTP POST/JSON (separate transport) | The *Login form* leg of the auth flow — JSON over HTTPS, **not** binary RPC |
| `RealmList.proto` payloads | nested inside `GameUtilities.ClientRequest` `Attribute`s | Realm list ticket request/response |

---

## 8. Current state in RustyCore

The Rust port is in `crates/wow-proto`, using `prost` + `prost-build` to compile a curated subset of `.proto` files at build time.

**Files in `/home/server/rustycore`:**
- `crates/wow-proto/Cargo.toml` — `prost` dep + `prost-build` build-dep
- `crates/wow-proto/build.rs` — compiles **11 `.proto` files** under `proto/bgs/low/pb/client/`
- `crates/wow-proto/src/lib.rs` — `pub mod bgs::protocol::{authentication, connection, challenge, game_utilities, account}::v1`, hand-written `service_hash` constants, hand-written `status` constants, `RESPONSE_SERVICE_ID = 0xFE`, 6 round-trip tests
- `crates/wow-proto/proto/bgs/low/pb/client/`
  - `rpc_types.proto` — `Header`, `ProcessId`, `NoData`, etc.
  - `entity_types.proto` — `EntityId`
  - `attribute_types.proto` — `Attribute`, `Variant`
  - `content_handle_types.proto` — content handle types
  - `semantic_version.proto` — version triples
  - `authentication_service.proto` — Logon flow
  - `connection_service.proto` — Connect/Bind/Disconnect
  - `challenge_service.proto` — captcha challenges
  - `game_utilities_service.proto` — Realm list channel
  - `account_types.proto` + `account_service.proto` — account info

**What's implemented:**
- Core RPC envelope (`Header`, `ProcessId`, `NoData`, `EntityId`, `Attribute`, `Variant`)
- 5 services as Rust modules: `authentication.v1`, `connection.v1`, `challenge.v1`, `game_utilities.v1`, `account.v1`
- Service-hash constants for **15 services** (5 implemented + 10 declared but unimplemented: `FRIENDS_SERVICE/LISTENER`, `PRESENCE_SERVICE/LISTENER`, `REPORT_SERVICE/V2`, `RESOURCES_SERVICE`, `USER_MANAGER_SERVICE/LISTENER`, `AUTHENTICATION_LISTENER`, `CHALLENGE_LISTENER`, `ACCOUNT_LISTENER`)
- A minimal `status` mod with `OK`, `ERROR_INTERNAL`, `ERROR_DENIED`, `ERROR_NO_GAME_ACCOUNT`, `ERROR_WOW_SERVICES_GAME_ACCOUNT_LOCKED`, `ERROR_GAME_ACCOUNT_BANNED`, `ERROR_GAME_ACCOUNT_SUSPENDED` — **only 7 of ~330** error codes from `BattlenetRpcErrorCodes.h`
- 6 unit tests covering Header round-trip, EntityId round-trip, LogonRequest encode, ConnectRequest encode, ClientRequest with Attributes, service hash constants

**What's missing vs C++:**
- **No `Login.proto` port.** The HTTP/JSON login form layer is entirely absent — `bnet-server`'s `LoginForm` / `SrpLoginChallenge` / `LoginResult` / `GameAccountList` / `AuthenticationState` types must be hand-written elsewhere or the form posts re-implemented from scratch
- **No `RealmList.proto` port.** Realm-list responses are likely hand-rolled in `bnet-server`'s realm-list handler. The 12 messages (`RealmEntry`, `RealmState`, `ClientInformation`, etc.) need to be ported to ensure binary parity with retail clients
- **No `club_service.proto` / `club_listener.proto` port.** The entire community/club system is unreachable; clients that try to query clubs will hit unimplemented method ids
- **No port of the unrecovered C++ `.pb.h` set:** account-listener, friends_service/types/listener, presence_service/types/listener, report_service/types(/V2), resource_service, user_manager_service/types/listener, channel_types, club_* (~25 files), embed_types, ets_types, event_view_types, invitation_types, message_types, notification_types, profanity_filter_config, role_types, rpc_config, voice_types. To port them you must first **recover the `.proto` schema** by reading the `.pb.h` files and reverse-engineering, or obtain Blizzard's official `bgs.low` schema
- **`BattlenetRpcErrorCodes.h` is barely ported** (~7 of ~330 codes). Many codes are needed for accurate error reporting (e.g. `ERROR_GAME_ACCOUNT_BANNED_PERMANENTLY`, `ERROR_AUTHENTICATION_*` family for login error parity)
- **No `ServiceBase` analogue.** The Rust side dispatches services ad-hoc per-service. The C++ pattern of a base class with `CallServerMethod` virtual + standard logging helpers is not replicated. This is fine if the dispatch table is small, but as more services are added, refactoring to a trait + `inventory` pattern (mirroring `wow-handler`) would help
- **No `MethodOptions` / `ServiceOptions` / `FieldOptions` / `MessageOptions` custom-options ports.** These are used by the C++ codegen to attach metadata (e.g. `(method_options).disallow_dynamic_method`) and only matter if you want full schema fidelity. Not blocking
- **`prost` does not generate service stubs** by default — it only generates messages. The Rust side has to wire dispatch by hand (currently done in `bnet-server`). C++ has the `*Service` generated classes for free
- **No re-export of Login/RealmList types from the `lib.rs`** even if hand-written, no exposed module exists yet

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- The two-tier hash mapping (`service_id` byte for local registration, `service_hash` for global identification) is implemented in the Rust side — but the convention "use `service_id` for sender-known recipients, `service_hash` for the first message" needs verification against a wire capture
- The `RESPONSE_SERVICE_ID = 0xFE` magic is referenced but it's not clear whether the bnet-server uses it consistently when dispatching responses
- The Login HTTP/JSON portion is hand-written somewhere (likely in `bnet-server`'s REST / Axum routes); risk of deviation from `Login.proto` field semantics — e.g. `required` vs `optional` field handling, JSON encoding conventions (`proto2 → JSON` rules vs ad-hoc)
- The realm-list flow stuffs proto messages inside `GameUtilities.ClientRequest`'s `Attribute` list, with `Attribute.name` keys like `Command_RealmListRequest_v1`. The Rust side covers `Attribute` but the **command-name conventions** (`Command_*_v1`, `Param_*`, `Param_RealmListBase_v1`, etc.) are only soft-validated; client-side validation is strict, so a typo silently breaks realm browsing
- 7 of 330 status codes ⇒ many error paths are reported as `ERROR_INTERNAL` instead of the precise code — observable client-side as a generic error popup instead of the specific localised message

**Tests existing:**
- 6 unit tests in `wow-proto/src/lib.rs` (Header, EntityId, LogonRequest, ConnectRequest, ClientRequest+Attribute, service-hash constants)
- No integration tests against a real client capture (.pcap)
- No tests on `account_service.proto` types (despite being compiled)
- No tests for the (missing) Login form or RealmList types

---

## 9. Migration sub-tasks

Numera los items para poder referenciarlos desde `MIGRATION_ROADMAP.md` sección 5.

Complejidad: **L** (low, <1h), **M** (med, 1-4h), **H** (high, 4-12h), **XL** (>12h, splitear).

### Phase A — port the two recovered `.proto` sources (Login + RealmList)

- [ ] **#PROTO.1** Copy `Login.proto` into `crates/wow-proto/proto/json/` (or similar), add to `build.rs`, expose as `pub mod json::login`. (L)
- [ ] **#PROTO.2** Hand-port realm-list types from `RealmList.proto` into `crates/wow-proto/proto/bgs/low/pb/client/realmlist_types.proto` (or similar). Add to `build.rs`, expose as `pub mod realmlist::v1`. (M — schema is straight-forward, ~12 messages)
- [ ] **#PROTO.3** Add a round-trip test for `LoginForm` and `RealmEntry` against captured wire bytes from a real client. (M — needs capture)
- [ ] **#PROTO.4** Verify `bnet-server`'s realm-list handler uses the new types instead of hand-rolled byte builders. (M)

### Phase B — port BattlenetRpcErrorCode in full

- [ ] **#PROTO.5** Generate a Rust `enum BattlenetRpcErrorCode` from `BattlenetRpcErrorCodes.h` (script: parse the C header, emit `pub const ERROR_FOO: u32 = 0x...` set or a `#[repr(u32)] enum`). (M)
- [ ] **#PROTO.6** Replace all `0x...` literal status codes in `bnet-server` with the named constants. (L)

### Phase C — port club service (only `.proto` source not yet ported)

- [ ] **#PROTO.7** Copy `club_service.proto` and `club_listener.proto` into the build, expose under `pub mod club::v1`. (M — 414 lines combined; ~30 message types, ~25 RPC methods)
- [ ] **#PROTO.8** Decide whether to implement the club service handlers or stub them. (M-decision)

### Phase D — recover and port the remaining ~50 services from `.pb.h`

This is the long-tail effort. Each service can be tackled independently.

For each `xxx.pb.h` without a checked-in `.proto`:

- [ ] **#PROTO.9** Reverse-engineer the `.proto` schema from `xxx.pb.h` (pattern: each `class` in the header maps to one `message`, each `enum` to one `enum`, field types/numbers from `*_field_number` constants and tag-prefix bytes in the descriptor pool). (M-H per service)

Concrete sub-tasks (one per service group):

- [ ] **#PROTO.10** `friends_types.proto` + `friends_service.proto` (friends list, invitations). (H)
- [ ] **#PROTO.11** `presence_types.proto` + `presence_service.proto` + `presence_listener.proto`. (H)
- [ ] **#PROTO.12** `account_listener.proto` (server push for account updates). (M)
- [ ] **#PROTO.13** `report_types.proto` + `report_service.proto` (+v2). (M)
- [ ] **#PROTO.14** `resource_service.proto`. (L)
- [ ] **#PROTO.15** `user_manager_types.proto` + `user_manager_service.proto` + `user_manager_listener.proto`. (M)
- [ ] **#PROTO.16** `channel_types.proto`. (L)
- [ ] **#PROTO.17** `notification_types.proto` + `message_types.proto` + `invitation_types.proto`. (M)
- [ ] **#PROTO.18** `profanity_filter_config.proto`. (L)
- [ ] **#PROTO.19** `role_types.proto`. (L)
- [ ] **#PROTO.20** `voice_types.proto`. (L)
- [ ] **#PROTO.21** `embed_types.proto` + `ets_types.proto` + `event_view_types.proto` (rich text + embeds for chat). (M)
- [ ] **#PROTO.22** Club domain (~25 files): `club_core/_enum/_type/_member/_member_id/_membership_*/_invitation/_listener/_notification/_request/_role/_service/_stream/_tag/_types/_ban/_name_generator/_range_set`. (XL — split per concept)

### Phase E — `ServiceBase` analogue

- [ ] **#PROTO.23** Define a `ServiceTrait` (or use `inventory`) for service-hash-based dispatch in `wow-network` or `wow-handler` mirror. (M)
- [ ] **#PROTO.24** Add structured logging helpers (`log_call_server_method`, `log_unimplemented`, etc.) to standardise output across services. (L)
- [ ] **#PROTO.25** Add a `disallow_dynamic_method` enforcement pass (mirrors C++ `LogDisallowedMethod`). (L)

### Phase F — protobuf custom options (low priority)

- [ ] **#PROTO.26** If/when needed, port `field_options`, `message_options`, `method_options`, `service_options`, `range`, `register_method_types`, `routing` from `Client/api/`. (M — only useful if you need schema-level metadata; not required for wire compatibility)

### Phase G — testing

- [ ] **#PROTO.27** Capture a full BNet handshake `.pcap` against a real client and add round-trip tests for every message we port. (M — once)
- [ ] **#PROTO.28** Fuzz prost decoders with malformed BNet packets to ensure no panics. (M)

---

## 10. Regression tests to write

- [ ] Test: `Header { service_id, method_id, token, service_hash, size }` round-trip equals capture from a real client `Logon` packet
- [ ] Test: `LogonRequest { program: "WoW", platform: "Wn64", locale: "enUS", ... }` encodes to bytes byte-identical to a TC `bnet-server` capture
- [ ] Test: `ConnectRequest` encodes to bytes that the WoW 3.4.3 client accepts (tested by booting it against the bnet-server)
- [ ] Test: `RealmEntry { wowRealmAddress, name, flags, ... }` survives round-trip and `ClientInformation` matches a 3.4.3.54261 capture
- [ ] Test: `Attribute { name: "Command_RealmListRequest_v1", value: Variant { string_value: ... } }` is the exact byte sequence the realm-list flow expects
- [ ] Test: every defined `service_hash::*` constant equals the FNV-1a hash of the canonical service name string
- [ ] Test: `BattlenetRpcErrorCode::ERROR_OK == 0`, `ERROR_DENIED == 3`, `ERROR_GAME_ACCOUNT_BANNED == 0x0002000B` — discriminant parity with `BattlenetRpcErrorCodes.h`
- [ ] Test: `LoginForm` JSON encoding matches the Battle.net JSON-over-HTTPS form layer (the JSON shape, not protobuf binary)
- [ ] Test: `SrpLoginChallenge` populated by the bnet-server with version=1, iterations=15000 (or the wowchad value) and a valid SRP6 modulus matches a TC reference

---

## 11. Notes / gotchas

- **Two transports.** Battle.net runs over **two** transports in parallel: (a) JSON-over-HTTPS for the *Login form* leg (uses `Login.proto` types, `proto2` syntax with `optimize_for = CODE_SIZE`), and (b) length-prefixed binary RPC over TLS for everything else (uses the `bgs.low` services). Don't conflate them — `bnet-server` exposes both on different ports (1119 binary RPC, 8081 REST/JSON).
- **`service_hash` vs `service_id`.** The first frame on a binary connection registers a service via `service_hash` (32-bit FNV-1a of the service name); subsequent frames refer to it by a small `service_id` byte. Both fields are in `Header`. `RESPONSE_SERVICE_ID = 0xFE` is the magic id for response messages.
- **Token correlation.** Each request has a `token` (uint32) that the response copies back. `bnet-server` must keep a `HashMap<token, oneshot::Sender<...>>` to route replies. Easy to forget when bridging async ↔ sync.
- **`proto2` with `optimize_for = CODE_SIZE`.** The C++ `.proto` files use proto2 syntax (`required` / `optional` are explicit). prost uses proto2 fine, but `required` fields become `Option<T>` in Rust and you must check `.is_some()` at decode-time or your code panics on `.unwrap()`. Lint for this.
- **Attribute / Variant generic envelope.** `bgs.protocol.Attribute { name: string, value: Variant }` and `Variant { bool_value, int_value, float_value, string_value, blob_value, message_value, fourcc_value, uint_value }` — used as a key/value bag for `GameUtilities.ClientRequest`. Realm list, realm-list-character-counts, profanity, etc. are all *transported as attribute lists inside ClientRequest*. The `name` strings are the de-facto API surface (`Command_RealmListRequest_v1`, `Param_RealmListBase_v1`, etc.) — typos there silently break the client.
- **Hash collisions.** Service hashes are 32-bit FNV-1a; collisions are theoretically possible but the canonical service name set is small and curated. Don't invent new services without checking for collision.
- **`message_value` in `Variant` is `bytes`, not nested message.** Holds a serialised inner protobuf — decode separately. This is how realm-list responses carry `RealmListUpdates` etc.
- **`profanity_filter_config.pb.h`** is the chat-profanity rules table; it ships as part of the protocol but is rarely used in private servers. Low porting priority.
- **Recovering `.proto` from `.pb.h`** is doable: each generated header has a `descriptor_pool_` table at the bottom that contains the binary `FileDescriptorProto`. Run `protoc --decode_raw < descriptor_bytes` to recover schemas. Tedious but reliable.
- **Custom field options** like `(method_options).disallow_dynamic_method` and `(service_options).inbound` are used to gate which methods accept dynamic dispatch and which require strict registration. You can ignore these for the Rust port unless you need exact-equivalent dispatch semantics; just hard-code the rule.
- **Login flow secret.** `RealmListTicketClientInformation.info.secret` is a 16-byte client nonce that gets HMAC'd into the world-server handshake. Don't strip it during protobuf decoding — it's load-bearing for `wow-crypto` HMAC validation in the world-server handover.
- **Don't blindly copy `bgs.low/pb/client/`** into the Rust crate — it's already pruned to what's actually used. If you need to add services, add them deliberately with the rationale documented (which client opcode triggers them?).
- **Versioning.** Battle.net protocol versions iterate independently from WoW client versions. The 3.4.3 client uses `bgs.low` from a specific era; later expansions add new services & methods. If you copy a `.pb.h` from a newer Trinity branch, it may not match what the 3.4.3 client expects. Stick to this tree's `.pb.h` set.

---

## 12. C++ → Rust mapping (high-level)

| C++ Symbol | Rust Equivalent | Notes |
|---|---|---|
| `class ServiceBase` | `trait Service { fn service_hash() -> u32; async fn call_server_method(token: u32, method_id: u32, buf: BytesMut) -> Result<Bytes>; }` | Or use `inventory` static registration, mirroring `wow-handler::PacketHandlerEntry` |
| `enum BattlenetRpcErrorCode` | `#[repr(u32)] pub enum RpcError` (or `pub mod status { pub const ERROR_OK: u32 = 0; ... }`) | Discriminants must match C++ exactly |
| `google::protobuf::Message` | `prost::Message` | — |
| `MessageBuffer` | `bytes::BytesMut` (input) / `Vec<u8>` (output) | — |
| `std::function<void(MessageBuffer)>` continuation | `oneshot::Sender<Bytes>` or `Box<dyn FnOnce(...)>` | async-friendly version |
| Generated `class XxxService` (per `.pb.h`) | Module `pub mod xxx::v1 { /* messages */ }` + a separate `XxxServiceHandler` trait/impl | prost gives messages, dispatch is hand-written |
| `service_hash_` field | `const SERVICE_HASH: u32` on the trait impl | — |
| `Header` proto | `bgs::protocol::Header` (already done) | — |
| `bgs::protocol::Attribute`, `Variant` | `bgs::protocol::{Attribute, Variant}` (already done) | The key-value envelope pattern is the same |
| `Login.proto` (proto2 + `optimize_for=CODE_SIZE`) | prost-compiled module `json::login` | proto2 → `Option<T>` fields |
| `RealmList.proto` | prost-compiled module `realmlist::v1` | — |
| `EnumUtils` reflection (over `Header.METHOD_ID` etc.) | Hand-written `enum Method { Logon, Verify, ... }` per service | — |
| `LogCallServerMethod`, `LogFailedParsingRequest`, etc. | `tracing::info!`, `tracing::error!` macros with structured fields | Use `wow-logging` |
| `MethodOptions { disallow_dynamic_method = true }` | Compile-time const + runtime check in dispatch | Soft-enforce |

---

*Template version: 1.0 (2026-05-01).* Cuando se rellene, actualizar header de status y `Last updated`.
