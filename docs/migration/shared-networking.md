# Migration: shared/Networking

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/shared/Networking/`
> **Rust target crate(s):** `crates/wow-network/`
> **Layer:** L1
> **Status:** ✅ done (~80%)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Framework genérico de sockets TCP asíncronos basado en CRTP (`Socket<T>`) sobre Boost.ASIO. Provee acceptor (`AsyncAcceptor`), thread pool (`NetworkThread<T>`) y manager singleton (`SocketMgr<T>`) que cualquier socket de aplicación (worldserver, bnetserver, realmserver) parametriza para tener I/O asíncrono uniforme. En Rust se sustituye por Tokio + estructuras propias en `wow-network` (~1716 líneas).

---

## 2. C++ canonical files

| File | Lines (aprox) | Purpose |
|---|---|---|
| `src/server/shared/Networking/Socket.h` | 313 | Template CRTP base; async I/O, read/write queues |
| `src/server/shared/Networking/SocketMgr.h` | 142 | Per-type singleton; acceptor factory + NetworkThread pool |
| `src/server/shared/Networking/AsyncAcceptor.h` | 153 | Acceptor Boost.ASIO con factory callback `OnSocketOpen` |
| `src/server/shared/Networking/NetworkThread.h` | 174 | Pool de threads; per-thread socket storage + update tick |
| `src/server/shared/Networking/SslSocket.h` | 88 | Wrapper SSL (no usado en WoLK; presente para BNet REST) |
| `src/server/shared/Networking/Http/HttpService.h` | ~250 | HTTP service (legacy) |
| `src/server/shared/Networking/Http/HttpSocket.h` | ~250 | HTTP socket subclass |
| `src/server/shared/Networking/Http/BaseHttpSocket.h` | ~250 | HTTP base |
| **TOTAL** | **~1893** | — |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `Socket<T, Stream>` | template class (CRTP) | Base de cualquier socket; lifecycle async I/O |
| `SocketMgr<T>` | template class | Singleton-per-tipo; factoría + start/stop |
| `AsyncAcceptor` | class | Boost.ASIO TCP acceptor con factory callback |
| `NetworkThread<T>` | template class | Worker thread; mantiene `_sockets` y `_newSockets` |
| `SslSocket<T>` | template class | Hereda `Socket`; SSL handshake antes de Start |
| `HttpSocket` | class | Especialización HTTP (legacy, no WoLK) |
| `HttpService` | class | HTTP listener + dispatcher |
| `MessageBuffer` | class (en `Packets/`) | Ring buffer I/O |
| `Trinity::Asio::IoContext` | wrapper | Boost.ASIO context |
| `AsyncCallback` | functor | Read handler tipado |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `Socket::Start()` | Inicia primer async read | `AsyncRead`, `ReadHandler` |
| `Socket::AsyncRead()` | Encola async_read_some | Boost.ASIO |
| `Socket::AsyncReadWithCallback(cb)` | Read con callback custom | Boost.ASIO |
| `Socket::QueuePacket(buffer)` | Encola write | `AsyncProcessQueue` |
| `Socket::CloseSocket()` | Cierre inmediato + `OnClose` hook | shutdown |
| `Socket::DelayedCloseSocket()` | Marca para cerrar al vaciar write queue | flag |
| `Socket::Update()` | Frame tick (escenarios non-IOCP) | `HandleQueue` |
| `Socket::GetRemoteIpAddress()` | Address remota | — |
| `SocketMgr::StartNetwork(io, bind, port, n)` | Inicia listener + thread pool | `AsyncAcceptor`, `CreateThreads` |
| `SocketMgr::StopNetwork()` | Graceful shutdown | acceptor.Close, threads.Stop/Wait |
| `SocketMgr::OnSocketOpen(sock, idx)` | Factory callback (crea socket de app) | `T::ctor`, `Start` |
| `SocketMgr::SelectThreadWithMinConnections()` | Load balancer round-robin | — |
| `AsyncAcceptor::Bind()` / `Close()` | Bind/Close TCP listener | Boost.ASIO |
| `NetworkThread::Start()` / `Stop()` | Lifecycle thread | atomic flag |
| `NetworkThread::AddSocket(sock)` | Registra socket en thread | `_newSockets` |
| `NetworkThread::Update()` | Iter `_sockets`, llama `Socket::Update` | — |

---

## 5. Module dependencies

**Depends on:**
- `Packets/MessageBuffer.h` — ring buffer
- `Logging.h`, `Errors.h`, `Define.h` — primitivos
- **Boost.ASIO** (TCP, async I/O)
- **OpenSSL** (`SslSocket` only)

**Depended on by:**
- `game/Server/WorldSocket` — hereda `Socket`, usa `WorldPacketCrypt`
- `bnetserver/RealmList/...` — `Socket` para auth
- `HTTPServer` (legacy)

---

## 6. SQL / DB queries

N/A — Networking es infraestructura pura, sin queries propias.

---

## 7. Wire-protocol packets

N/A — `WorldSocket` (en `game/Server`) origina opcodes; Networking solo transporta bytes.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-network/src/world_socket.rs` — ~1023 líneas — equivalente a `Socket` + `WorldSocket`
- `crates/wow-network/src/accept.rs` — `start_world_listener()` (≈ `AsyncAcceptor`)
- `crates/wow-network/src/session_mgr.rs` — `SessionManager` (≈ `SocketMgr` para session pool)
- `crates/wow-network/src/lib.rs` — re-exports
- `crates/wow-network/src/player_registry.rs` — registry de jugadores online (no equivalente directo C++)

**What's implemented:**
- Async TCP listener (Tokio)
- Per-client `WorldSocket` con FSM (Uninitialized → ConnectionString → AuthChallenge → EncryptedMode)
- Read/write buffer rings + async handlers
- Account DB lookup + session key derivation
- Integración AES-GCM encryption/decryption
- Header encryption con counter monotonic per direction

**What's missing vs C++:**
- SSL socket (no usado en WoLK pero existe en BNet REST)
- HTTP socket (legacy, no scope)
- Dynamic load balancing (Tokio runtime hace algo equivalente pero distinto)
- IOCP optimization Windows (irrelevante con Tokio)
- Graceful drain on shutdown (actual close inmediato)

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- `ServerCounter` quirk (líneas 65-81 world_socket.rs): Rust pre-set counters porque C# siempre incrementa incluso pre-encryption. **Verificar fidelidad vs TrinityCore**.
- No tests de header encryption round-trip post-auth.
- No monitor de nonce reuse (potencial RIP AES-GCM).

**Tests existing:**
- Sin tests unitarios formales en `wow-network`. Tests integration probablemente en `world-server`.

---

## 9. Migration sub-tasks

- [ ] **#NET.1** Tests unitarios para `WorldSocket` FSM (5 estados, edge cases). (M)
- [ ] **#NET.2** Audit `ServerCounter`/`ClientCounter` matching exacto con C++. (M)
- [ ] **#NET.3** Regression test: header encryption round-trip post-CMSG_AUTH_SESSION. (M)
- [ ] **#NET.4** Implementar graceful shutdown drain (write queue flush antes close). (H)
- [ ] **#NET.5** Benchmark async read/write Rust vs C++ on same hardware. (H)
- [ ] **#NET.6** Load test: 10k concurrent sockets, memory/CPU profile. (H)
- [ ] **#NET.7** Implementar session timeout (30min idle disconnect). (M)
- [ ] **#NET.8** SOCKS5 proxy support para legacy clients. (M, opcional)
- [ ] **#NET.9** Métricas: per-socket latency, packet rate, crypto timing. (M)
- [ ] **#NET.10** Document buffer sizing heuristics (READ_BLOCK_SIZE 4096 vs Rust tuning). (L)
- [ ] **#NET.11** Verificar nonce counter monotónico, alertar en overflow approach (2^60). (L)

---

## 10. Regression tests to write

- [ ] Header encryption toggle (pre/post EnterEncryptedMode)
- [ ] Nonce counter monotonic increase per direction
- [ ] Reconnect mismo IP resetea estado correctamente
- [ ] Graceful close drena write queue antes TCP FIN
- [ ] Load balancer distribuye uniforme entre N threads (si aplica)
- [ ] Concurrent connect: 1000 clients en <30s sin race conditions

---

## 11. Notes / gotchas

1. **ServerCounter quirk:** C# siempre incrementa, incluso para packets sin cifrar. Rust replica. Ver `world_socket.rs` L67-81.
2. **Nonce reuse = RIP AES-GCM:** Per-direction counter NUNCA puede colisionar entre direcciones. Counter overflow tras 2^64 causa IV reuse. Mitigación: session timeout << 2^64 packets (en práctica imposible alcanzar).
3. **Boost.ASIO IOCP vs epoll:** C++ multiplexa según OS; Tokio abstrae ambos.
4. **READ_BLOCK_SIZE = 4096:** Razonable; WoW max packet ~65KB world. Ring buffer ≥ 2× max packet.
5. **Sin graceful drain en C++:** Socket cierra inmediato on error. Rust replica el comportamiento C++; si se quiere mejor, es divergencia consciente.

---

## 12. C++ → Rust mapping

| C++ | Rust | Notas |
|---|---|---|
| `Socket<T, Stream>` (CRTP) | `WorldSocket` struct + Tokio TcpStream | Pattern divergente; mismo resultado |
| `SocketMgr<T>` | `SessionManager` (en `session_mgr.rs`) | Singleton session tracker |
| `AsyncAcceptor` | `start_world_listener()` (en `accept.rs`) | Tokio TcpListener |
| `NetworkThread<T>` | Tokio runtime + tasks | Async executor reemplaza OS threads |
| `Socket::AsyncRead()` | `TcpStream::read()` + poll loop | Tokio nativo |
| `Socket::QueuePacket()` | `flume::Sender<Vec<u8>>` | Async MPMC channel |
| `Socket::Update()` | (no necesario) | Tokio event-driven |
| `MessageBuffer` | `BytesMut` (crate `bytes`) | Same ring semantics |

---

*Template version: 1.0 (2026-05-01).*
