# Migration: Crypto

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/common/Cryptography/`
> **Rust target crate(s):** `crates/wow-crypto/`
> **Layer:** L1
> **Status:** ✅ done (~95%)
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Implementa criptografía de TrinityCore: SRP6 (auth), AES-128-GCM (packet encryption con tag de 12 bytes — variante WoW, no estándar), HMAC-SHA1/SHA256, RSA, Ed25519 y derivación de claves vía `SessionKeyGenerator`. WoLK 3.4.3 específicamente usa SRP6-Grunt + AES-128-GCM + HMAC-SHA256 para session keys. Rust traduce todo a `wow-crypto` con dependencia de OpenSSL/RustCrypto crates.

---

## 2. C++ canonical files

| File | Lines | Purpose |
|---|---|---|
| `src/common/Cryptography/Authentication/SRP6.h` | 242 | SRP6 base + GruntSRP6 (WoLK) + BnetSRP6 v1/v2 |
| `src/common/Cryptography/Authentication/SRP6.cpp` | ~800 | Verify, evidence, KDF |
| `src/common/Cryptography/AES.h` | 52 | AES-128-GCM wrapper (EVP_CIPHER_CTX) |
| `src/common/Cryptography/AES.cpp` | 87 | AES init, encrypt/decrypt, integrity |
| `src/common/Cryptography/Authentication/WorldPacketCrypt.h` | 44 | Packet-level encrypt state (ciphers + counters) |
| `src/common/Cryptography/Authentication/WorldPacketCrypt.cpp` | ~250 | Encrypt/decrypt con nonce per direction |
| `src/common/Cryptography/HMAC.h` | 140 | Template `HMAC<Hash>` (SHA1, SHA256) |
| `src/common/Cryptography/SessionKeyGenerator.h` | 61 | KDF de session key SRP a múltiples claves |
| `src/common/Cryptography/BigNumber.h` | ~470 | Wrapper OpenSSL BN_* |
| `src/common/Cryptography/BigNumber.cpp` | ~450 | Mul/mod/power/import/export |
| `src/common/Cryptography/CryptoHash.h` | 146 | SHA1/256/512 EVP wrappers |
| `src/common/Cryptography/CryptoRandom.h` | 46 | RAND_bytes init |
| `src/common/Cryptography/RSA.h` | 113 | RSA sign/verify (EVP_PKEY) |
| `src/common/Cryptography/Ed25519.h` | 69 | Ed25519 sign/verify |
| `src/common/Cryptography/ARC4.h` | 58 | RC4 stream (legacy, deprecated) |
| `src/common/Cryptography/Argon2.h` | 44 | Argon2 password hashing (BNet v2 opt) |
| `src/common/Cryptography/TOTP.h` | 37 | TOTP 2FA |
| `src/common/Cryptography/CryptoConstants.h` | 37 | Magic seeds y constantes |
| `src/common/Cryptography/OpenSSLCrypto.h` | 36 | OpenSSL init/cleanup |
| **TOTAL** | **~2933** | — |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `SRP6` | abstract class | Interfaz SRP-6 base |
| `GruntSRP6` | class | WoLK SRP6 (N/g/k Blizzard, SHA-1, 256-bit) |
| `BnetSRP6Base` | class | BNet SRP6 base (v1 SHA1, v2 SHA256) |
| `BnetSRP6v1Base` / `BnetSRP6v2Base` | class | BNet variants (1 iter SHA-256 / 15k iter PBKDF/Argon2 SHA-512) |
| `WorldPacketCrypt` | class | AES + counters per direction |
| `AES` | class | AES-128-GCM EVP wrapper |
| `BigNumber` | class | Arbitrary precision int (BN_*) |
| `GenericHMAC<HashImpl>` | template class | HMAC any hash |
| `SessionKeyGenerator<Hash>` | template class | KDF para múltiples claves desde session key |
| `CryptoHash` (SHA1/256/512) | template class | Hashes EVP_MD wrappers |
| `RSA`, `Ed25519` | class | Signature algos |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `SRP6::VerifyClientEvidence(A, M1)` → `Optional<BigNumber>` | Valida CMSG_AUTH_SESSION proof | `DoVerifyClientEvidence` |
| `GruntSRP6::CalculateServerEvidence(A, M1, K)` → `BigNumber` | Genera M2 (server proof) | SHA1 interleave |
| `SRP6::CheckCredentials(user, pwd)` | Verifica vs salt/verifier stored | `CalculateX`, `CalculateVerifier` |
| `SRP6::MakeRegistrationData<Impl>(user, pwd)` | Genera (salt, verifier) registro | `CalculateVerifier` |
| `WorldPacketCrypt::Init(key)` | Init AES bidirec + counters | `AES::Init` |
| `WorldPacketCrypt::DecryptRecv(data, len, tag)` | Descifra cliente→server | `AES::Process`, counter++ |
| `WorldPacketCrypt::EncryptSend(data, len, tag)` | Cifra server→cliente | `AES::Process`, counter++ |
| `WorldPacketCrypt::PeekDecryptRecv(data, len)` | Descifra sin tag verify | `AES::ProcessNoIntegrityCheck` |
| `AES::Init(key)` / `Process(iv, data, len, tag)` | EVP setup + encrypt/decrypt | OpenSSL EVP_Cipher* |
| `BigNumber::ModPow(exp, mod)` → `BigNumber` | g^b mod N | OpenSSL BN_mod_exp |
| `BigNumber::operator*(const)` | Multiply | OpenSSL BN_mod_mul |
| `BigNumber::ToByteArray<N>(le)` | Export endian | memcpy |
| `GenericHMAC::GetDigestOf(seed, data)` → `Digest` | Compute HMAC | EVP_DigestSign |
| `SessionKeyGenerator::Generate(buf, sz)` | Fill buf con material | fill_up loop |

---

## 5. Module dependencies

**Depends on:**
- `BigNumber.h`, `CryptoHash.h`, `CryptoRandom.h`, `Define.h`, `Errors.h`
- **OpenSSL** (libssl, libcrypto): EVP_*, BN_*, RAND_*
- **Argon2** (opt, BNet v2 2FA)

**Depended on by:**
- `game/Server/WorldSocket` — `WorldPacketCrypt`, SRP6 verify
- `bnetserver/Authentication` — BnetSRP6 v1/v2
- `Account` service — session key storage en tabla `account.sessionkey`

---

## 6. SQL / DB queries

| Statement / Field | Purpose | DB |
|---|---|---|
| `account.salt` (32 bytes) | SRP6 salt | auth |
| `account.verifier` (256 bytes) | SRP6 verifier (hex) | auth |
| `account.sessionkey` (40 bytes hex) | Session key derivada (SHA-256) | auth |
| `SELECT verifier, salt FROM account WHERE id=?` | Pre-auth lookup | auth |
| `UPDATE account SET sessionkey=?, last_ip=? WHERE id=?` | Almacena session key post-auth | auth |

DBC/DB2: ninguno (Crypto es algorítmico puro).

---

## 7. Wire-protocol packets

Crypto NO origina packets; `WorldSocket` los emite. Crypto cifra:

| Opcode | Originador | Cifrado | Notas |
|---|---|---|---|
| `CMSG_AUTH_SESSION` | Client | NO yet | Handshake phase |
| `SMSG_AUTH_RESPONSE` | Server | NO yet | — |
| `SMSG_ENTER_ENCRYPTED_MODE` | Server | NO yet (Ed25519 signed) | Activa cifrado |
| `CMSG_ENTER_ENCRYPTED_MODE_ACK` | Client | NO yet | Ack |
| **All subsequent** | Both | **AES-128-GCM (12-byte tag)** | Cifrado |

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-crypto/src/srp6.rs` — SRP6 N/g/k WoLK, X/S/v calc, M1/M2 verify
- `crates/wow-crypto/src/bnet_srp6.rs` — BNetSRP6 v1/v2
- `crates/wow-crypto/src/world_crypt.rs` — AES-128-GCM con tag 12 bytes, counter per direction
- `crates/wow-crypto/src/session_key.rs` — SessionKeyGenerator SHA1/SHA256
- `crates/wow-crypto/src/hmac_utils.rs` — HMAC SHA1/SHA256
- `crates/wow-crypto/src/ed25519ctx.rs` — Ed25519 wrapper
- `crates/wow-crypto/src/sarc4.rs` — RC4 legacy
- `crates/wow-crypto/src/rsa_sign.rs` — RSA sign/verify

**What's implemented:**
- SRP6 completo (verify client evidence, compute server evidence, registration data)
- BNetSRP6 v1 + v2 (SHA-256, SHA-512 con iteraciones)
- AES-128-GCM con tag 12 bytes (NO el 16 estándar) — ✅ confirma
- Per-direction counter (server/client) en nonce
- SessionKeyGenerator split-half KDF
- HMAC-SHA1, HMAC-SHA256
- Ed25519 sign/verify

**What's missing vs C++:**
- **Vector tests** SRP6 — no hay tests vs salida C++ reference
- **AES nonce reuse detection** — sin auditoría de counter overflow approach
- **Argon2** — no implementado (opt para BNet v2 2FA, low priority)
- **TOTP** — no implementado (2FA, low priority)
- **ARC4** — implementado pero deprecated, no usar

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- **Nonce format:** ¿`[counter LE | suffix LE]` vs C++ `[counter LE | suffix BE]`? **CRÍTICO VERIFICAR**.
- **HMAC key ordering:** orden estricto en derivación de DEC_KEY/ENC_KEY/HMAC keys; verificar.
- **SRP6 prime constants:** N exact hex `894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7`?
- **CalculateX:** `SHA1(salt || SHA1(user:pass))` — orden de concat exacto?
- **Ed25519 key:** `ENTER_ENCRYPTED_MODE_PRIVATE_KEY` (world_socket.rs:81) hardcoded; en producción real ¿viene de `build_info` table?

**Tests existing:**
- Algunos tests de round-trip pero sin vectors cross-impl
- `srp6.rs`: sin tests unitarios formales

---

## 9. Migration sub-tasks

- [ ] **#CRYPTO.1** Vector tests SRP6: dado `(user, pwd)` → comparar `(salt, verifier, A, M1, M2)` vs C++ reference output. (H)
- [ ] **#CRYPTO.2** Auditar nonce format: `[counter LE | suffix LE]` o BE — verificar contra C++ `WorldPacketCrypt::Encrypt`. (M)
- [ ] **#CRYPTO.3** Auditar HMAC-SHA256 key derivation order vs C++ exacto. (M)
- [ ] **#CRYPTO.4** Verificar SRP6 N, g, k = Blizzard wotlk_classic exactos. (L)
- [ ] **#CRYPTO.5** Test AES round-trip: encrypt header → decrypt → match plaintext. (M)
- [ ] **#CRYPTO.6** Counter overflow detection: warn si counter > 2^60. (L)
- [ ] **#CRYPTO.7** Test SessionKeyGenerator: seed → 32/40 bytes match C++ output. (M)
- [ ] **#CRYPTO.8** Documentar nonce construction (12 bytes) en world_crypt.rs comment. (L)
- [ ] **#CRYPTO.9** BNet SRP6: tests v1 SHA-256 iteration vs v2 PBKDF/Argon2. (M)
- [ ] **#CRYPTO.10** RSA signature verify: integrar pub key de `build_info` table, verificar EnterEncryptedMode. (H)
- [ ] **#CRYPTO.11** Ed25519 key: investigar si hardcoded vs DB-driven y unificar. (M)
- [ ] **#CRYPTO.12** Benchmark SRP6 verify vs C++. (M)
- [ ] **#CRYPTO.13** Argon2 PBKDF (opt para 2FA BNet v2). (M, low prio)
- [ ] **#CRYPTO.14** Fuzz testing: invalid SRP6 A values (0, N, N+1, neg mod). (H)
- [ ] **#CRYPTO.15** Documentar magic seeds (AUTH_CHECK_SEED, SESSION_KEY_SEED). (L)

---

## 10. Regression tests to write

- [ ] SRP6 verifier generation matches C++ bit-for-bit
- [ ] AES-GCM nonce counter increments monotonic
- [ ] 12-byte GCM tag (no 16) enforced
- [ ] Nonce per-direction (server vs client suffix)
- [ ] SessionKeyGenerator split-half output match C++
- [ ] HMAC-SHA256 key order (DEC, ENC, DEC_HMAC, ENC_HMAC)
- [ ] SRP6 evidence M1 rejection on invalid A (0 or N)
- [ ] Packet encryption round-trip
- [ ] Counter overflow warning approach 2^60

---

## 11. Notes / gotchas

1. **SRP6 safe primes:** N, g, k son específicos Blizzard WoLK. Hex N debe ser `894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7` exacto. Cualquier desviación → todos los logins rotos.
2. **HMAC key order STRICT:** SessionKeyGenerator splits seed mitades, hashea cada una, produce o1/o2. Order matters; replay detection depende de eso.
3. **AES-GCM 12-byte tag:** Standard GCM = 16. WoW = 12. Crate `aes-gcm` soporta `Tag<U12>`. ✅ Rust ya usa eso.
4. **Nonce reuse = RIP:** Counter overflow tras 2^64 packets. En práctica imposible (el server reiniciaría primero), pero merece warning en logs si se acerca a 2^60.
5. **Ed25519 key hardcoded en `world_socket.rs:81`:** En TrinityCore real esto viene de tabla `build_info` per build. Investigar si simplificación o regresión.
6. **ARC4 deprecated:** SRP6 Auth en WoLK NO usa RC4. Solo legacy code path; ignorar.
7. **BNet vs WoW:** `GruntSRP6` = WoLK world auth. `BnetSRP6 v1/v2` = Battle.net login. **Completamente separados** — no confundir.
8. **PBKDF vs Argon2:** BNet v2 usa iterations (no Argon2 en TrinityCore legacy). Argon2 es future-proof, opcional.

---

## 12. C++ → Rust mapping

| C++ | Rust | Notas |
|---|---|---|
| `GruntSRP6` (class) | `srp6::Srp6Params` + funciones (proc style) | — |
| `BigNumber` | `num_bigint::BigUint` | Direct replacement |
| `HMAC<SHA1/SHA256>` | `hmac_utils::HmacSha1/256` | Wrapper |
| `SessionKeyGenerator<SHA1/256>` | `SessionKeyGenerator{,256}` | Direct |
| `WorldPacketCrypt` | `WorldCrypt` (en `world_crypt.rs`) | aes-gcm crate, tag 12B |
| `AES::Process()` | `WorldCrypt::encrypt_packet()` / `decrypt_packet()` | Integrado |
| `uint64 _clientCounter` | `client_counter: u64` | — |
| `IV [12]` | `nonce: [u8; 12]` | counter+suffix |
| `Tag [12]` | `tag: [u8; 12]` | aes-gcm output |
| `CryptoHash SHA1` | `sha1::Sha1` (digest crate) | Extern |
| `CryptoRandom` | `rand::thread_rng()` | Extern |
| `RSA sign/verify` | `wow-network::rsa_sign` (separate module) | — |
| `Ed25519` | `ed25519_dalek` + `ed25519ctx` wrapper | — |

---

*Template version: 1.0 (2026-05-01).*
