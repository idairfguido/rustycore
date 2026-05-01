# Migration: shared/Packets

> **C++ canonical path:** `/home/server/woltk-trinity-legacy/src/server/shared/Packets/`
> **Rust target crate(s):** `crates/wow-packet/`
> **Layer:** L0
> **Status:** ⚠️ partial
> **Audited vs C++:** ❌ not audited
> **Last updated:** 2026-05-01

---

## 1. Purpose

Infraestructura de serialización/deserialización de paquetes binarios del protocolo WoW 3.4.3. Provee `ByteBuffer` (contenedor genérico para read/write de primitivos con bit-packing) y `WorldPacket` (encapsula opcode + payload). Todos los ~560 handlers de sesión dependen de él.

---

## 2. C++ canonical files

| File | Lines (aprox) | Purpose |
|---|---|---|
| `src/server/shared/Packets/ByteBuffer.h` | ~800 | Clase ByteBuffer: read<T>/write<T>, bit-packing, posición lectura/escritura, excepciones |
| `src/server/shared/Packets/ByteBuffer.cpp` | ~300 | Conversión endian, manejo excepciones, métodos string |
| `src/server/shared/Packets/MessageBuffer.h` | ~150 | Ring buffer pooling para I/O |

---

## 3. Classes / Structs / Enums

| Symbol | Kind | Purpose |
|---|---|---|
| `ByteBuffer` | class | Contenedor con read/write tipado + bit-packing |
| `ByteBufferException` | class | Base excepción |
| `ByteBufferPositionException` | class | Read/write past end |
| `ByteBufferInvalidValueException` | class | Conversión tipo inválida |
| `WorldPacket` | class | Packet con opcode (u16 LE) + payload ByteBuffer |
| `MessageBuffer` | class | Ring buffer I/O |

---

## 4. Critical public methods / functions

| Symbol | Purpose | Calls into |
|---|---|---|
| `ByteBuffer::append<T>(T)` | Añade primitivo (u8/16/32/64, i*, float, double) | `EndianConvert`, `append(uint8*, size)` |
| `ByteBuffer::read<T>()` | Lee primitivo; avanza `_rpos` | `ResetBitPos`, conversión endian |
| `ByteBuffer::WriteBit(bool)` | Escribe 1 bit en `_curbitval`; auto-flush si lleno | `_curbitval`, `_bitpos` |
| `ByteBuffer::ReadBit()` | Lee 1 bit; recarga `_curbitval` | — |
| `ByteBuffer::WriteBits(value, n)` | Escribe N bits | loop WriteBit |
| `ByteBuffer::ReadBits(n)` | Lee N bits | loop ReadBit |
| `ByteBuffer::FlushBits()` | Finaliza bit-pack actual; escribe byte pendiente | `append(uint8*)` |
| `ByteBuffer::ResetBitPos()` | Reset bit pos pre-read byte | `_bitpos = 8` |
| `ByteBuffer::rpos()` / `wpos()` | Get/set read/write position | — |
| `ByteBuffer::bitwpos()` | Get bit position | — |
| `ByteBuffer::PutBits(pos, n, val)` | Reemplaza bits en posición (patch) | — |
| `ByteBuffer::operator<<(T)` / `operator>>(T&)` | Append/read con operadores | — |
| `ByteBuffer::ReadString()` / `ReadCString()` | Lee string null-terminated | — |
| `ByteBuffer::ReadPackedGuid()` / `WritePackedGuid()` | Compression GUID variable-length WoLK | — |

---

## 5. Module dependencies

**Depends on:**
- `shared/Define.h` (uint8/16/32/64, TC_SHARED_API)
- `shared/ByteConverter.h` (`EndianConvert` little-endian)
- STL: `std::vector<uint8>`, `std::string`, `std::array`

**Depended on by:**
- **Todos los ~560 packet handlers** (`WorldSession::HandleXxx`)
- `game/Server/Protocol/Opcodes.h` — enum opcodes en primer u16
- `game/World/WorldSession.h` — constructor WorldPacket
- Todos los crates Rust de packets

---

## 6. SQL / DB queries

N/A — serialización pura, sin queries.

---

## 7. Wire-protocol packets

Define el formato genérico:

```text
[Header]
  u16 LE  — Opcode
[Payload]
  variable — formato específico por opcode
```

Bit-packing: `WriteBits` puede mezclarse con bytes planos en mismo packet; `FlushBits()` obligatorio antes de cambiar modo.

---

## 8. Current state in RustyCore

**Files in `/home/server/rustycore`:**
- `crates/wow-packet/src/world_packet.rs` — ~400 líneas (`WorldPacket` con `BitBuf` state)
- `crates/wow-packet/src/header.rs` — `PacketHeader` (16 bytes: size + GCM tag)
- `crates/wow-packet/src/compression.rs` — Compresión > 1024 bytes
- `crates/wow-packet/src/lib.rs` — Traits `ClientPacket` y `ServerPacket`
- `crates/wow-packet/src/packets/mod.rs` — Inventario módulos por dominio (aura, auth, battlenet, character, chat, combat, gossip, inspect, item, loot, misc, movement, query, quest, party, social, spell, trainer, update)

**What's implemented:**
- `new_empty()`, `new_server()`, `new_client()` — construcción
- `read<T>()`, `write<T>()` — primitivos (u8/u16/u32/u64, i*, f32/f64)
- `read_string()`, `write_string()` — null-terminated
- `read_bits(n)`, `write_bits(val, n)` — bit-packing
- `flush_bits()` — finalizar
- Traits `ClientPacket` / `ServerPacket`
- ~50+ packet types definidos por dominio

**What's missing vs C++:**
- Hierarquía completa de excepciones — Rust usa `Result<T, PacketError>` (idiom diferente, OK)
- `ReadPackedGuid` / `WritePackedGuid` — compresión GUID variable-length (algunos packets sí lo tienen, otros no — auditar)
- Variantes de string (`ReadCString` con trim) — Rust usa `read_string` genérico
- `append_bytes` / `read_bytes` — copiar bloques sin conversión

**Suspicious / likely divergent (hipótesis pre-auditoría):**
- **Bit-packing order:** C++ `_bitpos` decrece (8→0), Rust `wbit_pos` incrementa (0→8). Lógica inversible pero requiere auditoría explícita.
- **String encoding:** WoLK 3.4.3 inconsistente: algunos opcodes null-terminated, otros length-prefixed. Auditoría caso por caso.
- **Float encoding:** ¿endian igual en todos los casos?

**Tests existing:**
- ~10 tests primitivos en `crates/wow-packet/tests/`
- Necesitan tests bit-packing (sequences read_bits/write_bits cross-impl)

---

## 9. Migration sub-tasks

- [ ] **#PACKETS.1** Verificar bit-packing: secuencia C++ → leer en Rust → comparar resultado byte-a-byte. (H)
- [ ] **#PACKETS.2** Implementar `ReadPackedGuid` / `WritePackedGuid` (variable-length GUID compression). (M)
- [ ] **#PACKETS.3** Auditar string encoding por opcode (null-term vs length-prefix). (H, ~30+ opcodes)
- [ ] **#PACKETS.4** Implementar `append_bytes` / `read_bytes` (copiar sin conversión tipos). (L)
- [ ] **#PACKETS.5** Garantizar `PacketError` cubre casos `ByteBufferException` C++. (L)
- [ ] **#PACKETS.6** Benchmark serialización Rust vs C++ en packet típico 512 bytes. (M)
- [ ] **#PACKETS.7** Auditar IEEE-754 float encoding (corner cases NaN, Inf, -0). (L)

---

## 10. Regression tests to write

- [ ] Serializar+deserializar primitivo u32 → resultado ≡ original
- [ ] Bit-packing: write 4+3+2 bits = 1 byte 0xAB, releer y verificar
- [ ] String null-terminated: write "hello\0", read, comparar
- [ ] Read past end → `Err(PacketError::ReadPastEnd)`
- [ ] Float IEEE-754: write f32(3.14), read, comparar con tolerancia
- [ ] PackedGuid (cuando se implemente): bytes esperados para GUID 0x0150000000ABCDEF
- [ ] Mixed mode: bytes plain + bits + bytes plain con FlushBits intermedio

---

## 11. Notes / gotchas

- **Bit-packing en WoLK 3.4.3 es delicado:** algunos opcodes mezclan bytes planos + bit-packed. Llamar `FlushBits()` explícito antes de cambiar modo.
- **Endianness:** little-endian en mayoría; algunos casos especiales (GUIDs comprimidos). Ver `ByteConverter.h` C++.
- **`MessageBuffer`:** C++ tiene pooling. Rust usa `BytesMut` directamente con capacidad pre-asignada.
- **Default size:** C++ `DEFAULT_SIZE = 0x1000` (4 KB). Rust 64 bytes, crece dinámico. Puede afectar perf packets grandes.
- **Position rewind:** `rpos(new)` salta. Cuidado: NO resetea estado bits.
- **ASSERT:** C++ excepción on read past end. Rust `Result`. Handlers deben manejar `Err`.

---

## 12. C++ → Rust mapping

| C++ | Rust | Notas |
|---|---|---|
| `ByteBuffer` | `WorldPacket` (en `world_packet.rs`) | `BytesMut` + estados bit |
| `uint8/16/32/64` | `u8/u16/u32/u64` | Conversión endian integrada |
| `float` / `double` | `f32` / `f64` | IEEE-754, conversión endian |
| `std::string_view` | `&str` o `String` | Bytes + null term |
| `WriteBit(bool)` | `write_bit(bool)` | Mismo state |
| `ReadBit()` | `read_bit()` | Mismo state |
| `WriteBits(v, n)` | `write_bits(v, n)` | Loop |
| `ReadBits(n)` | `read_bits(n)` | Loop, retorna u32 |
| `FlushBits()` | `flush_bits()` | Finaliza bit-pack |
| `ResetBitPos()` | `reset_bit_pos()` | — |
| `rpos()` / `wpos()` | `read_pos()` / `write_pos()` | — |
| `ByteBufferException` family | `Result<T, PacketError>` | Match patterns |

---

*Template version: 1.0 (2026-05-01).*
