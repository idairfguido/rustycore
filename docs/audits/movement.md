# Movement Parsing Audit (#A06)

Fecha: 2026-05-11

## C++ canónico

- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/MovementInfo.h`
  - `MovementInfo`: `guid`, `flags`, `flags2`, `flags3`, `pos`, `time`, `transport`, `pitch`, `inertia`, `jump`, `stepUpStartElevation`, `advFlying`, `standingOnGameObjectGUID`.
- `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/MovementPackets.cpp`
  - `operator>>(ByteBuffer&, MovementInfo&)`, líneas 104-170: orden binario de lectura.
  - `operator<<(ByteBuffer&, MovementInfo const&)`, líneas 25-99: orden binario de escritura.
  - `operator>>(ByteBuffer&, MovementInfo::TransportInfo&)`, líneas 181-198: transporte con bits `hasPrevTime`/`hasVehicleId`.
- `/home/server/woltk-trinity-legacy/src/server/game/Handlers/MovementHandler.cpp`
  - `HandleMovementOpcode`, líneas 312-430: validación de mover, teleport, coordenadas, transport, fall, auras, update position y broadcast.
  - `HandleSetActiveMoverOpcode`, líneas 543-548: solo log si el mover no coincide.
  - `HandleMoveInitActiveMoverComplete`, líneas 810-815: set local flag, transport server time y visibility update.

## Rust auditado

- `crates/wow-packet/src/packets/movement.rs`
  - `MovementInfo::read`, líneas 114-242.
  - `MovementInfo::write`, líneas 246-305.
  - `MoveUpdate`, `MonsterMove`, `SetActiveMover`, `MoveInitActiveMoverComplete`.
- `crates/wow-world/src/handlers/movement.rs`
  - `handle_movement`, líneas 79-153.
  - `handle_set_active_mover`, líneas 160-181.
  - `handle_move_init_active_mover_complete`, líneas 187-196.

## Divergencias

| Feature C++ | Estado Rust | Clasificación | Acción |
|---|---|---|---|
| Orden base `MovementInfo` (`guid`, flags, time, XYZO, pitch, step, remove forces, move index, 8 bits, bloques opcionales) | `MovementInfo::read` sigue el orden C++ para campos base, transport, standing GUID skip, inertia skip, adv flying y fall. | OK parcial | Añadir tests de roundtrip con transport/fall/adv flying para fijar wire format. |
| Escritura `hasFallData = falling flag || fallTime != 0`; `hasFallDirection = MOVEMENTFLAG_FALLING | FALLING_FAR` | Corregido en `#A06.1`: Rust deriva ambos bits de `MovementFlag::FALLING/FALLING_FAR` como C++ y cubre el caso `fallTime=0` con flags de caída. | OK | Mantener test de regresión en `wow-packet::packets::movement`. |
| `standingOnGameObjectGUID` e `inertia` se conservan como `Optional` en C++ | Corregido en `#A06.2`: Rust los representa en `MovementInfo`, los conserva al leer y los vuelve a emitir en `write`. | OK | Mantener test de regresión en `wow-packet::packets::movement`. |
| `TransportInfo` default C++ tiene `seat=-1`, `prevTime=0`, `vehicleId=0`; bits de prev/vehicle dependen de no cero al escribir | Rust modela `prev_time`/`vehicle_id` como `Option`, correcto en wire, pero no normaliza/limpia transport inválido en handler. | OK wire / Missing runtime | Cubrir en `#A06.4` junto a validación de transport. |
| Handler C++ rechaza si player está teletransportándose, GUID no coincide, posición inválida, movespline no finalizada | Rust solo comprueba GUID contra player y finitud XYZ; permite GUID vacío y no valida orientación/map bounds/movespline/teleport state. | Missing / Bug | `#A06.3`: endurecer validación mínima (`guid` debe ser player, `Position::is_valid` equivalente, teleport guard cuando exista estado). |
| Handler C++ procesa transport: dist > grid, offsets > 75, coordenada world+transport válida, add/remove passenger, reset transport si no aplica | Rust acepta transport sin validar y lo rebroadcast. | Missing | `#A06.4`: añadir validación/normalización mínima de transport; integración real queda para Transport/Map phase. |
| Handler C++ ajusta tiempo con `AdjustClientMovementTime` antes de guardar/broadcast | Rust rebroadcast usa tiempo del cliente tal cual. | Missing | `#A06.5`: añadir helper equivalente o TODO explícito ligado a time-sync. |
| Handler C++ side effects: fall damage, parachute/flight aura interrupts, pet unsummon, sit-to-stand, under-map damage, jump procs | Rust solo actualiza posición, visibility, area triggers y broadcast. | Missing | `#A06.6`: dividir side effects por sistemas dependientes (Aura/Spell, Pet, Map min-height, Combat). |
| `SetActiveMover` C++ solo loguea mismatch si player está en world | Rust loguea warning para mismatch pero no cambia estado, comportamiento aceptable. | OK distinto | Bajar a `trace/debug` si el log molesta en pruebas. |
| `MoveInitActiveMoverComplete` C++ setea local flag, transport server time y actualiza visibility | Rust solo loguea. | Missing | `#A06.7`: representar local flag/transport server time cuando ActivePlayerData local flags esté conectado. |
| ACK opcodes (`MoveKnockBackAck`, speed acks, movement force acks, collision height, spline done, time skipped) | Rust registra solo movement básicos, `SetActiveMover` y `MoveInitActiveMoverComplete`; no hay packet structs ni handlers para la mayoría de ACKs. | Missing | `#A06.8`: inventario de opcodes movement ACK y port incremental antes de anticheat/speed control. |
| `SMSG_ON_MONSTER_MOVE` C++ usa `MovementMonsterSpline` completo con flags, filters, face modes, packed deltas, extras | Rust `MonsterMove` es una versión simplificada de un punto y no representa `MoveSpline`. | Missing | Cubrir en Fase 2.1 `MoveSpline real`; no usarlo como port completo. |

## TODOs añadidos al roadmap

- `#A06.1`: corregido; `MovementInfo::write` usa la regla C++ de fall-data/fall-direction.
- `#A06.2`: corregido; `standingOnGameObjectGUID` e `inertia` ya se conservan en Rust.
- `#A06.3`: endurecer validación de movement en handler.
- `#A06.4`: validar/normalizar transport.
- `#A06.5`: portar `AdjustClientMovementTime` o documentar puente temporal con time sync.
- `#A06.6`: dividir side effects de movement por sistemas dependientes.
- `#A06.7`: portar efectos de `MoveInitActiveMoverComplete`.
- `#A06.8`: inventariar y portar ACK movement opcodes.

## Conclusión

`MovementInfo::read` está cerca del wire C++, pero no es auditoría completa porque se pierden campos opcionales y el writer tiene una divergencia en fall data. El handler Rust es funcional para la prueba básica de login/moverse, pero no es un port completo de `MovementHandler.cpp`. El módulo queda en estado ⚠️, no ✅.
