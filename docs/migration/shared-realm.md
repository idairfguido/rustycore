# Migration: shared/Realm

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/shared/Realm/`
> **Rust target crate(s):** `crates/bnet-server/` (`src/realm/mod.rs`)
> **Layer:** L1
> **Status:** ⚠️ partial (~60%)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Modela el conjunto de realms (servidores de juego) que el bnetserver ofrece al cliente WoW. Mantiene en memoria la lista refrescada periódicamente desde la tabla `realmlist` de la DB `auth`, almacena el `build_info` (versiones de cliente con sus `Win64AuthSeed`/`Mac64AuthSeed`), y serializa esa lista a JSON+zlib para los RPCs `JoinRealm` / `GetRealmList` del protocolo Battle.net. También expone `RealmHandle` (region/site/realm) como clave de direccionamiento de realms.

---

## 2. C++ canonical files

| File | Lines | Purpose |
|---|---|---|
| `src/server/shared/Realm/Realm.h` | 102 | `Realm` struct, `RealmHandle`, `RealmFlags`, `RealmType` |
| `src/server/shared/Realm/Realm.cpp` | 61 | `SetName`, `GetAddressForClient`, `GetConfigId`, `RealmHandle` formatters |
| `src/server/shared/Realm/RealmList.h` | 98 | `RealmList` singleton + `RealmBuildInfo` struct |
| `src/server/shared/Realm/RealmList.cpp` | 434 | DB load, periodic refresh, `JoinRealm`, JSON+zlib emission |
| **TOTAL** | **~695** | — |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Realm` | struct | Datos de un realm (Id, Build, addresses, port, name, type, flags…) |
| `Battlenet::RealmHandle` | struct | `(Region:u8, Site:u8, Realm:u32)` — clave compuesta + `GetAddress()` packed `u32` |
| `RealmFlags` | enum | `NONE / VERSION_MISMATCH / OFFLINE / SPECIFYBUILD / RECOMMENDED / NEW / FULL` |
| `RealmType` | enum | `NORMAL=0 / PVP=1 / NORMAL2=4 / RP=6 / RPPVP=8 / FFA_PVP=16` (custom) |
| `RealmBuildInfo` | struct | `Build`, `Major/Minor/BugfixVersion`, `HotfixVersion[4]`, `Win64AuthSeed[16]`, `Mac64AuthSeed[16]` |
| `RealmList` | singleton class | Holder global con `RealmMap = std::map<RealmHandle, Realm>` |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `RealmList::Instance()` | Singleton accessor | — |
| `RealmList::Initialize(io, interval)` | Carga inicial de builds + realms; arma timer | `LoadBuildInfo`, `UpdateRealms` |
| `RealmList::UpdateRealms()` | Refresca desde DB y reprograma timer | `LOGIN_SEL_REALMLIST`, `Resolver::Resolve` |
| `RealmList::GetRealm(handle)` | Lookup thread-safe (shared_lock) | — |
| `RealmList::GetBuildInfo(build)` | Encuentra `RealmBuildInfo` por número de build | linear scan |
| `RealmList::GetRealmEntryJSON(id, build)` | Serializa un `RealmEntry` → "JamJSONRealmEntry:" + zlib | `JSON::Serialize` |
| `RealmList::GetRealmList(build, subRegion)` | Serializa lista entera filtrada por subregion → "JSONRealmListUpdates:" + zlib | `JSON::Serialize` |
| `RealmList::JoinRealm(addr, build, clientAddr, secret, ...)` | Construye respuesta JoinRealm: server IP + 32 bytes ServerSecret + persiste keyData en DB | `LOGIN_UPD_BNET_GAME_ACCOUNT_LOGIN_INFO` |
| `RealmList::WriteSubRegions(resp)` | Escribe set de subregiones a `GetAllValuesForAttributeResponse` | — |
| `Realm::SetName(name)` | Asigna `Name` y `NormalizedName` (sin spaces) | — |
| `Realm::GetAddressForClient(clientAddr)` | Selecciona address externa o local según red del cliente | `Trinity::Net::SelectAddressForClient` |
| `RealmHandle::GetAddress()` | Empaqueta `(Region<<24)|(Site<<16)|Realm` en `u32` | — |
| `RealmHandle::GetAddressString()` | Formato `"{Region}-{Site}-{Realm}"` | — |

---

## 5. Module dependencies

**Depends on:**
- `shared/Database` — `LoginDatabase`, prepared statement `LOGIN_SEL_REALMLIST`, `LOGIN_UPD_BNET_GAME_ACCOUNT_LOGIN_INFO`
- `shared/Networking` — `Trinity::Asio::Resolver`, `DeadlineTimer`, `IoContext`
- `shared/JSON` — `JSON::Serialize` (ProtobufJSON wrapper)
- `proto` — `JSON::RealmList::RealmEntry`, `RealmListUpdates`, `RealmListServerIPAddresses`
- `shared/Crypto` — `Trinity::Crypto::GetRandomBytes<32>` para ServerSecret
- `zlib` — `compress`, `compressBound`

**Depended on by:**
- `bnetserver` — `RealmList::Initialize` en main, RPC handlers `JoinRealm`, `GetAllValuesForAttribute`
- `worldserver` — algunos servicios consultan `RealmList::Instance()` para sub-regions

---

## 6. SQL / DB queries

| Statement / Source | Purpose | DB |
|---|---|---|
| inline `SELECT majorVersion, minorVersion, bugfixVersion, hotfixVersion, build, win64AuthSeed, mac64AuthSeed FROM build_info ORDER BY build ASC` | Carga build_info | auth |
| `LOGIN_SEL_REALMLIST` | Carga `realmlist` table (id, name, address, localAddress, port, icon, flag, timezone, allowedSecurityLevel, population, gamebuild, Region, Battlegroup) | auth |
| `LOGIN_UPD_BNET_GAME_ACCOUNT_LOGIN_INFO` | Persiste keyData (clientSecret + serverSecret) + locale + os + timezoneOffset + accountName tras `JoinRealm` | auth |

---

## 7. Wire-protocol packets

N/A directos. Genera **payloads JSON+zlib** que el RPC bnet incluye en atributos:

| Attribute name | Origin |
|---|---|
| `Param_RealmJoinTicket` | accountName |
| `Param_ServerAddresses` | "JSONRealmListServerIPAddresses:" + JSON + zlib (4-byte uncompressed prefix) |
| `Param_JoinSecret` | 32 bytes random |

Y para realm list updates:
- envelope `"JamJSONRealmEntry:" + json + '\0'` (single realm)
- envelope `"JSONRealmListUpdates:" + json + '\0'` (full list)

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/bnet-server/src/realm/mod.rs` — 392 líneas — cubre ~60% del C++
- `crates/wow-database/src/statements/login.rs` — declara `SEL_REALMLIST`, `SEL_REALMLIST_SECURITY_LEVEL`

**What's implemented:**
- Struct `Realm` con todos los campos de `realmlist`
- `RealmBuildInfo` con seeds Win/Mac (parsed desde hex)
- `RealmManager` con `realms: HashMap<u32, Realm>` + `builds: Vec<RealmBuildInfo>` + `sub_regions: Vec<String>`
- `init_realm_manager` con polling task Tokio
- `load_build_info`, `update_realms` directo SQL
- `get_realm_list_json` con flag `VERSION_MISMATCH` dinámico y `population_state` correcto
- `get_realm_entry_json` con selector loopback / same-/24 / external (similar a `Trinity::Net::SelectAddressForClient`)
- Envelopes correctos: `JSONRealmListUpdates:` / `JSONRealmListServerIPAddresses:` / `JSONRealmCharacterCountList:`
- Compresión zlib con prefijo `u32` little-endian de tamaño descomprimido
- `find_realm_by_address`, `get_build_info`

**What's missing vs C++:**
- **`RealmHandle` no existe** — Rust usa `u32` plano como key, sin descomponer en `(Region, Site, Realm)`. `wowRealmAddress` = `r.id` directo, no packed `(Region<<24)|(Site<<16)|Realm`. Esto compila pero no es format-equivalent al C++ si llegan clientes con region/site distintos.
- **`JoinRealm` flow no existe** — falta la generación de `Param_RealmJoinTicket` / `Param_ServerAddresses` / `Param_JoinSecret` con persistencia DB de keyData. El RPC handler probablemente genera estos atributos en otro sitio (revisar `bnet-server/src/rpc/services/game_utilities.rs`).
- **`WriteSubRegions`** equivalente — falta la integración con `GetAllValuesForAttributeResponse`.
- **`RealmHandle::GetAddress()` / `GetAddressString()`** — sin equivalente.
- **`SetName` + `NormalizedName`** — Rust no normaliza el nombre (sin remover whitespace).
- **`GetMinorMajorBugfixVersionForBuild`** — sin equivalente; lo necesita warden o algún check de versión.
- **`RealmFlags` / `RealmType` enums tipados** — Rust usa `u8` plano (constantes inline `REALM_FLAG_VERSION_MISMATCH = 0x01`).
- **`AllowedSecurityLevel` clamp a `SEC_ADMINISTRATOR`** — falta en Rust.
- **REALM_TYPE_FFA_PVP=16 → REALM_TYPE_PVP=1 normalization** — falta en Rust.
- **Error path para `Resolver::Resolve` falla** — Rust no resuelve hostnames, asume IPs literales.

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- `wow_realm_address: r.id as i32` perdía la información Region/Site/Realm packed que el cliente espera. **Verificar packets reales**.
- `cfg_timezones_id: i32::from(r.timezone)` — C++ hardcodea `1` y mete timezone en `cfg_categories_id`; Rust tiene los dos campos cruzados. Posible bug.
- `cfg_categories_id: 1` — C++ pone `realm.Timezone` aquí; Rust lo tiene al revés.
- Sin `shared_mutex`, Rust usa `parking_lot::RwLock` (vía `state.realm_mgr.write()`) → equivalente.

**Tests existing:**
- 0 tests en `bnet-server/src/realm/`.

---

## 9. Migration sub-tasks

- [ ] **#REALM.1** Implementar `RealmHandle { region, site, realm }` con `get_address() -> u32` y `get_address_string() -> String`. (L)
- [ ] **#REALM.2** Cambiar `RealmManager.realms` a `HashMap<RealmHandle, Realm>` y propagar a calls sites. (M)
- [ ] **#REALM.3** Auditar `cfg_timezones_id` vs `cfg_categories_id` contra `RealmList.cpp:332`. (L)
- [ ] **#REALM.4** Implementar `JoinRealm` flow completo: random ServerSecret + persistir `LOGIN_UPD_BNET_GAME_ACCOUNT_LOGIN_INFO` + atributos respuesta. (M)
- [ ] **#REALM.5** Implementar `set_name` con `NormalizedName` (strip whitespace). (L)
- [ ] **#REALM.6** Implementar `get_minor_major_bugfix_version_for_build` con `binary_search`. (L)
- [ ] **#REALM.7** Tipar `RealmFlags` como `bitflags!` y `RealmType` como enum, con clamp `>= MAX_CLIENT_REALM_TYPE → NORMAL` y FFA_PVP→PVP. (M)
- [ ] **#REALM.8** Resolver hostnames (no solo IPs) con `tokio::net::lookup_host` en `update_realms`. (M)
- [ ] **#REALM.9** Clamp `allowed_security_level` a `SEC_ADMINISTRATOR`. (L)
- [ ] **#REALM.10** Tests: parse build_info, packed address bit-layout, version_mismatch flag toggle. (M)

---

## 10. Regression tests to write

- [ ] `RealmHandle::get_address()` empaqueta `(region<<24)|(site<<16)|realm` exact-match
- [ ] `set_name("Foo Bar")` produce `normalized_name == "FooBar"`
- [ ] Realm con `build != client_build` → emite `flags |= VERSION_MISMATCH` en JSON
- [ ] Realm con `flags & OFFLINE` → emite `population_state = 0`
- [ ] `select_realm_ip_str` para client 127.0.0.1 → local; para client en /24 distinto → external
- [ ] zlib output: 4-byte LE prefix == uncompressed length; flate2 inflates back to identical bytes
- [ ] Concurrent: 1000 readers + 1 writer con `update_realms` no causa race / panic

---

## 11. Notes / gotchas

1. **Region/Site/Realm packing:** WoW client transmite `wowRealmAddress` como `u32` packed. RustyCore actualmente lo trata como ID lineal — funciona para single-region/single-battlegroup pero rompe en multi-region.
2. **Envelope strings con `\0` final:** El C++ usa `json.length() + 1` para incluir el null terminator en la longitud comprimida. Rust replica con `format!("...:{}\0", json)`.
3. **`JamJSONRealmEntry` vs `JSONRealmListUpdates`:** dos serializaciones distintas para el mismo `RealmEntry` proto — el primero es single-shot (refresh de uno), el segundo plural updates.
4. **`HARDCODED_DEVELOPMENT_REALM_CATEGORY_ID = 1`** — `cfgCategoriesId` siempre `1` en C++. Rust mete `r.timezone` ahí; verificar.
5. **`build_info.HotfixVersion`** es `array<char,4>` no null-terminated — Rust lo guarda como `String` (puede tener bytes basura si el DB devuelve >4 chars).
6. **`Realm.AllowedSecurityLevel`:** tipo `AccountTypes` (enum), C++ hace `min(value, SEC_ADMINISTRATOR)`. Rust no clamp.

---

## 12. C++ → Rust mapping

| C++ | Rust | Notas |
|---|---|---|
| `class RealmList` (singleton) | `struct RealmManager` (en `AppState.realm_mgr: Arc<RwLock<RealmManager>>`) | Lifecycle vía `init_realm_manager` |
| `Battlenet::RealmHandle` | (faltante) `struct RealmHandle { region, site, realm }` | TODO #REALM.1 |
| `struct Realm` | `struct Realm` | 1:1 fields |
| `RealmFlags` enum | `const REALM_FLAG_*: u8 = ...` | Sustituir por `bitflags!` |
| `RealmType` enum | (faltante) constantes | Sustituir por enum |
| `RealmBuildInfo` | `struct RealmBuildInfo` | seeds como `Option<Vec<u8>>` |
| `std::map<RealmHandle, Realm>` | `HashMap<u32, Realm>` | TODO migrar a `HashMap<RealmHandle, Realm>` |
| `std::shared_mutex` | `parking_lot::RwLock` | Vía `AppState.realm_mgr` |
| `DeadlineTimer + async_wait` | `tokio::time::interval` + `tokio::spawn` | En `init_realm_manager` |
| `JSON::Serialize(proto)` | `serde_json::to_string(&struct)` | Pure serde, no protobuf |
| `compress()` (zlib) | `flate2::ZlibEncoder` | Mismo formato |
| `Trinity::Crypto::GetRandomBytes<32>` | (faltante para JoinRealm) | TODO #REALM.4 |
| `Trinity::Net::SelectAddressForClient` | `select_realm_ip_str` | Más simple: solo IPv4 + /24 + loopback |

---

*Template version: 1.0 (2026-05-01).*
