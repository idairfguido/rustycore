# Movement Parsing Audit (#A06)

Fecha: 2026-05-11

## C++ canónico

- `/home/server/woltk-trinity-legacy/src/server/game/Entities/Object/MovementInfo.h`
  - `MovementInfo`: `guid`, `flags`, `flags2`, `flags3`, `pos`, `time`, `transport`, `pitch`, `inertia`, `jump`, `stepUpStartElevation`, `advFlying`, `standingOnGameObjectGUID`.
- `/home/server/woltk-trinity-legacy/src/server/game/Server/Packets/MovementPackets.cpp`
  - `operator>>(ByteBuffer&, MovementInfo&)`, líneas 104-170: orden binario de lectura.
  - `operator<<(ByteBuffer&, MovementInfo const&)`, líneas 25-99: orden binario de escritura.
  - `operator>>(ByteBuffer&, MovementInfo::TransportInfo&)`, líneas 181-198: transporte con bits `hasPrevTime`/`hasVehicleId`.
  - ACKs de movimiento, líneas 800-989: `MoveTeleportAck`, `MovementAck`, `MovementAckMessage`, `MovementSpeedAck`, `MoveKnockBackAck`, `MoveApply/RemoveMovementForceAck`, `MoveSetCollisionHeightAck`, `MoveTimeSkipped`, `MoveSplineDone`.
- `/home/server/woltk-trinity-legacy/src/server/game/Handlers/MovementHandler.cpp`
  - `HandleMovementOpcode`, líneas 312-430: validación de mover, teleport, coordenadas, transport, fall, auras, update position y broadcast.
  - `HandleMoveTeleportAck`, líneas 263-306: near teleport ack, posición destino, zone update, resummon pet y delayed ops.
  - `HandleForceSpeedChangeAck`, líneas 470-539: valida movement info, decrementa counters de speed force, corrige/kickea si la velocidad no coincide.
  - `HandleMoveKnockBackAck`, líneas 552-565: valida, ajusta time, guarda `m_movementInfo` y emite `SMSG_MOVE_UPDATE_KNOCK_BACK`.
  - `HandleMovementAckMessage`, `HandleSetCollisionHeightAck`, `HandleMoveApply/RemoveMovementForceAck`, `HandleMoveSetModMovementForceMagnitudeAck`, líneas 567-666.
  - `HandleMoveSplineDoneOpcode` y `HandleMoveTimeSkippedOpcode`, líneas 668-741.
  - `HandleSetActiveMoverOpcode`, líneas 543-548: solo log si el mover no coincide.
  - `HandleMoveInitActiveMoverComplete`, líneas 810-815: set local flag, transport server time y visibility update.

## Rust auditado

- `crates/wow-packet/src/packets/movement.rs`
  - `MovementInfo::read`, líneas 114-242.
  - `MovementInfo::write`, líneas 246-305.
  - `MoveUpdate`, `MonsterMove`, `SetActiveMover`, `MoveInitActiveMoverComplete`.
  - `MovementAck`, `MovementAckMessage`, `MovementSpeedAck`, `MoveKnockBackAck`, `MoveSetCollisionHeightAck`, `MoveTimeSkipped`, `MoveSplineDone`, `MoveTeleportAck`.
- `crates/wow-world/src/handlers/movement.rs`
  - `handle_movement`, líneas 79-153.
  - `handle_set_active_mover`, líneas 160-181.
  - `handle_move_init_active_mover_complete`, líneas 187-196.
  - ACK handlers para generic ACKs, speed ACKs, knockback, collision height, time skipped, spline done y teleport ack.

## Divergencias

| Feature C++ | Estado Rust | Clasificación | Acción |
|---|---|---|---|
| Orden base `MovementInfo` (`guid`, flags, time, XYZO, pitch, step, remove forces, move index, 8 bits, bloques opcionales) | `MovementInfo::read` sigue el orden C++ para campos base, transport, standing GUID skip, inertia skip, adv flying y fall. | OK parcial | Añadir tests de roundtrip con transport/fall/adv flying para fijar wire format. |
| Escritura `hasFallData = falling flag || fallTime != 0`; `hasFallDirection = MOVEMENTFLAG_FALLING | FALLING_FAR` | Corregido en `#A06.1`: Rust deriva ambos bits de `MovementFlag::FALLING/FALLING_FAR` como C++ y cubre el caso `fallTime=0` con flags de caída. | OK | Mantener test de regresión en `wow-packet::packets::movement`. |
| `standingOnGameObjectGUID` e `inertia` se conservan como `Optional` en C++ | Corregido en `#A06.2`: Rust los representa en `MovementInfo`, los conserva al leer y los vuelve a emitir en `write`. | OK | Mantener test de regresión en `wow-packet::packets::movement`. |
| `TransportInfo` default C++ tiene `seat=-1`, `prevTime=0`, `vehicleId=0`; bits de prev/vehicle dependen de no cero al escribir | Rust modela `prev_time`/`vehicle_id` como `Option`, correcto en wire, pero no normaliza/limpia transport inválido en handler. | OK wire / Missing runtime | Cubrir en `#A06.4` junto a validación de transport. |
| Handler C++ rechaza si player está teletransportándose, GUID no coincide, posición inválida, movespline no finalizada | Corregido parcialmente en `#A06.3`: Rust ya exige GUID cargado/exacto y valida X/Y/Z/orientación con los límites de `GridDefines.h`. Teleport/movespline siguen pendientes porque no hay runtime equivalente completo. | OK parcial | Mantener teleport/movespline bajo Fase 2 Movement/Map runtime. |
| Handler C++ procesa transport: dist > grid, offsets > 75, coordenada world+transport válida, add/remove passenger, reset transport si no aplica | Corregido parcialmente en `#A06.4`: Rust rechaza distancias stale > grid, offsets ±75 y coordenadas world+transport inválidas. Add/remove passenger y reset de transport requieren `TransportBase`/Map runtime real. | OK parcial | Mantener integración real en Transport/Map phase. |
| Handler C++ ajusta tiempo con `AdjustClientMovementTime` antes de guardar/broadcast | Corregido en `#A06.5`: Rust registra requests pendientes, calcula delta con cola circular de 6 muestras y ajusta `MovementInfo.time` antes del update/broadcast. También corrige el timer efectivo C++: 5s solo para la primera sync, luego 10s. | OK | Mantener tests de delta/fallback/timer; `GetReceivedTime` se aproxima con el momento de handler Rust. |
| Handler C++ side effects: fall damage, parachute/flight aura interrupts, pet unsummon, sit-to-stand, under-map damage, jump procs | Corregido parcialmente en `#A06.6.1-#A06.6.4a`: Rust ya remueve auras por flags `LandingOrFlight`/`Jump`, levanta al jugador sentado al moverse, registra hooks para unsummon temporal de pet / jump proc, aplica `HandleFall` con `m_lastFallTime/Z`, guards GM/hover/feather/fly/normal immunity, `SAFE_FALL`, `MODIFY_FALL_DAMAGE_PCT`, god/env immunity y Gust of Wind representados, y ejecuta under-map con `PLAYER_FLAGS_IS_OUT_OF_BOUNDS` representado + `DAMAGE_FALL_TO_VOID`. `MapManager::min_height_like_cpp` usa el fallback C++ `-500.0` hasta que terrain/grid exponga alturas reales. Quedan conexiones runtime Pet/Proc/Aura completas. | OK parcial | `#A06.6.4`: conectar los hooks a runtime real; terrain/grid real sigue en Map/Terrain. |
| `SetActiveMover` C++ solo loguea mismatch si player está en world | Rust loguea warning para mismatch pero no cambia estado, comportamiento aceptable. | OK distinto | Bajar a `trace/debug` si el log molesta en pruebas. |
| `MoveInitActiveMoverComplete` C++ setea local flag, transport server time y actualiza visibility | Corregido en `#A06.7`: Rust lee `Ticks`, setea `PLAYER_LOCAL_FLAG_OVERRIDE_TRANSPORT_SERVER_TIME`, calcula `TransportServerTime = GameTimeMS - Ticks`, emite VALUES update mínimo de `ActivePlayerData::{LocalFlags,TransportServerTime}` y dispara refresh de visibility. | OK | Mantener test de estado; el timing exacto depende del contador monotónico Rust equivalente a `GameTime::GetGameTimeMS`. |
| ACK opcodes genéricos (`CollisionDisable/Enable`, double jump, swim-to-fly, feather fall, root/unroot, gravity, hover, inertia, can fly, turn while falling, ignore movement forces, water walk) | Corregido en `#A06.8a/#A06.8g.1`: Rust registra los opcodes que C++ manda a `HandleMovementAckMessage`, parsea `MovementAck` en orden C++, valida GUID/coord de forma conservadora y ya tiene sanitización representable de flags para las reglas locales/aura-representada de `Player::ValidateMovementInfo`. | OK parcial | Queda conectar la mutación completa de `Player::ValidateMovementInfo` en todos los ACKs cuando exista Unit/Aura/Vehicle runtime real. |
| ACKs de velocidad (`MoveForce*SpeedChangeAck`, turn/pitch rate y `MoveSetModMovementForceMagnitudeAck`) | Corregido en `#A06.8a/#A06.8b/#A06.8c` a nivel parser/handler: Rust lee `MovementSpeedAck`, valida GUID/coord, registra el ACK con speed, mapea opcode a `UnitMoveType`, decrementa counters equivalentes a `m_forced_speed_changes` / `m_movementForceModMagnitudeChanges`, salta ACKs pendientes, compara con tolerancia `0.01`, respeta transport como C++, registra corrección si el cliente va lento, hace kick si declara más velocidad/magnitud y rebroadcasts `SMSG_MOVE_UPDATE_MOD_MOVEMENT_FORCE_MAGNITUDE` para el ACK de movement-force magnitude. | OK parcial | Falta enganchar `Unit::SetSpeedRate` productivo, paquete real de corrección al cliente y `MovementForces::GetModMagnitude()` real cuando exista Unit/MovementForce runtime completo; no fingirlo como cerrado. |
| `MoveKnockBackAck` | Corregido en `#A06.8a/#A06.8c`: Rust lee optional speeds bit como C++, valida mover, ajusta time con `AdjustClientMovementTime`, actualiza posición/tiempo representados y emite `SMSG_MOVE_UPDATE_KNOCK_BACK` al set sin incluir al jugador local. | OK parcial | Queda conectar `Player::ValidateMovementInfo` completo sobre flags cuando exista runtime real. |
| `MoveSetCollisionHeightAck` | Corregido en `#A06.8a`: Rust lee `MovementAck`, `Height`, `MountDisplayID`, `Reason u8` en orden C++ y valida status. | OK parcial | Conectar a collision-height runtime si se requiere más que la validación C++ actual. |
| `MoveTimeSkipped` | Corregido en `#A06.8a/#A06.8c`: Rust salió de `misc` stub, lee `MoverGUID` + `TimeSkipped`, valida mover, suma `time_skipped` al tiempo de movimiento representado y emite `SMSG_MOVE_SKIP_TIME` al set sin incluir al jugador local. | OK | Mantener test de broadcast. |
| `MoveSplineDone` | Corregido en `#A06.8a/#A06.8e` a nivel parser/handler y side effects representables: Rust lee `MovementInfo` + `SplineID`, valida, registra, distingue taxi en progreso, representa teleport multi-map/teleport-node, cleanup final de taxi, remoción de flags taxi/control, dismount representado, reset de fall info y cast representado de Honorless Target 2479 cuando el jugador está hostile PvP. | OK parcial | Falta runtime real `PlayerTaxi`/`FlightPathMovementGenerator`/`MotionMaster`, costes/intermediate nodes, early landing y `SMSG_ON_MONSTER_MOVE` completo; no fingir taxi spline como port completo. |
| `MoveTeleportAck` | Corregido en `#A06.8a/#A06.8f` a nivel parser/handler y side effects representables: Rust lee `MoverGUID`, `AckIndex`, `MoveTime`, mantiene semáforo near y destino representado, ignora ACKs si no hay near teleport o el GUID no coincide, aplica mapa/posición destino, resetea fall info, actualiza zone/area representados, ejecuta ramas PvP/Honorless Target 2479 representadas, registra resummon temporal de pet y `ProcessDelayedOperations`. | OK parcial | Falta runtime real `Player::TeleportTo`, `SMSG_MOVE_TELEPORT`, `SMSG_MOVE_UPDATE_TELEPORT`, access checks, vehicle/transport/duel/BG/arena cleanup, pet/dynobject/areatrigger/cast/aura cleanup y resolver real Map/Terrain de zone/area. |
| ACKs de movement force (`MoveApplyMovementForceAck`, `MoveRemoveMovementForceAck`) | Corregido en `#A06.8c/#A06.8d`: Rust porta `MovementForce` wire (`ID`, origin/direction XYZ, transport id, magnitude, `Unused910`, type de 2 bits), parsea Apply/Remove ACK, valida GUID/coord, ajusta time, registra force id/type y emite `SMSG_MOVE_UPDATE_APPLY_MOVEMENT_FORCE` / `SMSG_MOVE_UPDATE_REMOVE_MOVEMENT_FORCE` al set. | OK parcial | Queda conectar `Player::ValidateMovementInfo` completo sobre flags cuando exista runtime real. |
| `SMSG_ON_MONSTER_MOVE` C++ usa `MovementMonsterSpline` completo con flags, filters, face modes, packed deltas, extras | Corregido parcialmente en `#A06.8h.1/#A06.8h.2/#A06.8h.3a/#A06.8h.3c.1`: Rust ya escribe el DTO wire `MovementMonsterSpline`/`MovementSpline` en orden C++, cubre destino, tolerancia, flags, elapsed/moveTime/fade/mode/transport/seat, counts bit-packed, face normal/spot/target/angle, puntos y packed deltas; `MonsterMoveStop` deja `Move` default y `StopDistanceTolerance=2` como C++. `tick_creatures_sync` vuelve a emitir un `SMSG_ON_MONSTER_MOVE` lineal representado cuando una criatura inicia wander. `crates/wow-movement` ya contiene un primer núcleo real contrastado de `MoveSplineFlag`, `MoveSplineInitArgs`, fall/parabolic math, almacenamiento CatmullRom-compatible, duración, `ComputePosition`, update/finalize y cyclic wrap. Además, `wow-packet` ya modela y serializa los opcionales `SplineFilter`, `SpellEffectExtraData`, `JumpExtraData` y `AnimTierTransition`, y puede mapear un `MoveSpline` real a `MovementMonsterSpline` siguiendo `InitializeSplineData`. | OK parcial | Falta conectar `MoveSpline` a `Unit`/packet runtime, transport transform, `MoveSplineInit::Launch/Stop`, `MotionMaster`, generators, pathgen y `SMSG_FLIGHT_SPLINE_SYNC`. |

## TODOs añadidos al roadmap

- `#A06.1`: corregido; `MovementInfo::write` usa la regla C++ de fall-data/fall-direction.
- `#A06.2`: corregido; `standingOnGameObjectGUID` e `inertia` ya se conservan en Rust.
- `#A06.3`: corregido en validación mínima; quedan guards de teleport/movespline para Fase 2.
- `#A06.4`: validación mínima corregida; integración passenger/reset queda para Transport/Map runtime.
- `#A06.5`: corregido; `AdjustClientMovementTime` y time-sync delta básico portados.
- `#A06.6`: `#A06.6.1-#A06.6.2` corregidos; quedan `#A06.6.3-#A06.6.4` para under-map y conexión real Aura/Pet/Proc.
- `#A06.7`: portar efectos de `MoveInitActiveMoverComplete`.
- `#A06.8a`: corregido; parsers/handlers base para ACKs genéricos, speed ACKs, knockback, collision height, time skipped, spline done y teleport ack.
- `#A06.8c`: corregido; broadcasts ACK portados para knockback, skip-time, movement-force apply/remove y movement-force magnitude.
- `#A06.8d`: corregido; `MovementForce` wire y Apply/Remove ACK quedan parseados, registrados y rebroadcast.
- `#A06.8b`: corregido a nivel representado/handler; quedan explícitos bajo runtime Unit/MovementForce el incremento productivo desde `SetSpeedRate`, el resend de corrección y `MovementForces::GetModMagnitude()` real.
- `#A06.8e`: corregido a nivel representado/handler; queda explícito para Taxi/MotionMaster real el runtime completo de flight paths, costes, early landing y spline packets.
- `#A06.8f`: corregido a nivel representado/handler; queda explícito para Teleport/Map/Movement real el `Player::TeleportTo` completo y packets de near teleport.
- `#A06.8g.1`: corregido a nivel representado; `ValidateMovementInfo` ya muta flags locales/aura-representada como C++ para los casos portables sin Unit/Aura/Vehicle productivo.
- `#A06.8g.2`: pendiente explícito para `ValidateMovementInfo` completo integrado en todos los ACKs con `Unit::HasAuraType`, vehículo fijo, mover controlado y runtime real.
- `#A06.8h.1`: corregido a nivel wire puro; `MonsterMove`/`MonsterMoveStop` ya siguen el layout C++ de `MovementMonsterSpline` para los campos portables sin runtime `MoveSpline` completo.
- `#A06.8h.2`: corregido a nivel representado; el wander de criaturas vuelve a enviar `SMSG_ON_MONSTER_MOVE` lineal con el wire C++ corregido.
- `#A06.8h.3a`: corregido como base de runtime; `crates/wow-movement` existe y porta un núcleo de `MoveSpline` contrastado contra C++ con tests unitarios. No sustituye todavía al shell de `MoveSplineState` ni a `tick_creatures_sync`.
- `#A06.8h.3b.1-#A06.8h.3b.3`: corregidos dentro del núcleo; `Enter_Cycle` reconstruye el path preservando duración, `AnimTierTransition` queda modelado en args/spline y `compute_position_percent` porta la regla `SplineImpl.h::computeIndex`.
- `#A06.8h.3c.1`: corregido para el borde packet-side; `MoveSpline` expone path data C++-like y `wow-packet` serializa opcionales + mapper `MoveSpline -> MovementMonsterSpline`. Quedan `Unit` runtime y flight sync.

## Conclusión

`MovementInfo` y varios ACKs principales ya están fijados contra el wire C++ con tests, y el handler Rust cubre movimiento básico, side effects representables, `MoveInitActiveMoverComplete`, ACK bookkeeping seguro, anticheat de speed representado, sanitización representable de `ValidateMovementInfo`, wire puro + broadcast lineal representado de `SMSG_ON_MONSTER_MOVE`, primer núcleo `MoveSpline` en `wow-movement` con `Enter_Cycle`/anim tier/percent-eval y mapper packet-side `MoveSpline -> MovementMonsterSpline`, side effects representables de taxi `MoveSplineDone` y near teleport ACK. Aun así no es un port completo de `MovementHandler.cpp`: faltan runtime real de Aura/Pet/Proc, movement-force/speed productivo, `ValidateMovementInfo` completo integrado con Unit/Aura/Vehicle, Taxi/MotionMaster/MoveSpline conectado a Unit y `Player::TeleportTo` completo. El módulo queda en estado ⚠️, no ✅.
