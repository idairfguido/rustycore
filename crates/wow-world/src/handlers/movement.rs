// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Movement packet handlers — CMSG_MOVE_*.
//!
//! All movement opcodes map to the same handler logic:
//!   1. Parse MovementInfo from the packet
//!   2. Validate: GUID must match the player, position must be finite
//!   3. Update server-side player position
//!   4. Broadcast SMSG_MOVE_UPDATE to nearby sessions (TODO: multi-session map)
//!
//! Reference: C# Game/Handlers/MovementHandler.cs

use tracing::{trace, warn};

use wow_constants::ClientOpcodes;
use wow_constants::movement::MovementFlag;
use wow_constants::unit::UnitStandStateType;
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_packet::ServerPacket;
use wow_packet::packets::movement::{
    ClientPlayerMovement, MoveInitActiveMoverComplete, MoveUpdate, MovementInfo, SetActiveMover,
};

use crate::session::{
    SPELL_AURA_INTERRUPT_FLAG_LANDING_OR_FLIGHT_LIKE_CPP, SPELL_AURA_INTERRUPT_FLAG2_JUMP_LIKE_CPP,
    WorldSession,
};

// ── Handler registrations ─────────────────────────────────────────
// All CMSG_MOVE_* share the same handler (ThreadSafe in C#).

macro_rules! register_move {
    ($opcode:ident) => {
        inventory::submit! {
            PacketHandlerEntry {
                opcode: ClientOpcodes::$opcode,
                status: SessionStatus::LoggedIn,
                processing: PacketProcessing::ThreadUnsafe,
                handler_name: concat!("handle_movement_", stringify!($opcode)),
            }
        }
    };
}

register_move!(MoveStartForward);
register_move!(MoveStartBackward);
register_move!(MoveStop);
register_move!(MoveStartStrafeLeft);
register_move!(MoveStartStrafeRight);
register_move!(MoveStopStrafe);
register_move!(MoveStartTurnLeft);
register_move!(MoveStartTurnRight);
register_move!(MoveStopTurn);
register_move!(MoveStartPitchUp);
register_move!(MoveStartPitchDown);
register_move!(MoveStopPitch);
register_move!(MoveSetRunMode);
register_move!(MoveSetWalkMode);
register_move!(MoveHeartbeat);
register_move!(MoveFallLand);
register_move!(MoveFallReset);
register_move!(MoveJump);
register_move!(MoveSetFacing);
register_move!(MoveSetFacingHeartbeat);
register_move!(MoveSetPitch);
register_move!(MoveSetFly);
register_move!(MoveStartAscend);
register_move!(MoveStopAscend);
register_move!(MoveStartDescend);
register_move!(MoveStartSwim);
register_move!(MoveStopSwim);
register_move!(MoveUpdateFallSpeed);

// ── Handler implementation ─────────────────────────────────────────

impl WorldSession {
    /// Handle any CMSG_MOVE_* packet.
    ///
    /// Parses MovementInfo, validates it, updates player position,
    /// and queues a broadcast to nearby players.
    pub async fn handle_movement(&mut self, mut pkt: wow_packet::WorldPacket) {
        let opcode = pkt.client_opcode();
        let mut info = match ClientPlayerMovement::read(&mut pkt) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    account = self.account_id,
                    "Failed to parse movement packet: {e}"
                );
                return;
            }
        };

        let Some(player_guid) = self.player_guid() else {
            warn!(
                account = self.account_id,
                "Movement packet received without loaded player"
            );
            return;
        };

        // C++ rejects any movement packet whose guid does not match the current mover.
        if info.info.guid != player_guid {
            warn!(
                account = self.account_id,
                "Movement GUID mismatch: expected {:?}, got {:?}", player_guid, info.info.guid
            );
            return;
        }

        let pos = info.info.position;
        if !pos.is_valid_map_coord_like_cpp() {
            warn!(
                account = self.account_id,
                "Invalid movement position: {pos:?}"
            );
            return;
        }

        if let Some(transport) = &info.info.transport {
            if self.player_position_like_cpp().is_some_and(|current| {
                pos.distance_2d(&current) > wow_core::Position::GRID_SIZE_LIKE_CPP
            }) {
                trace!(
                    account = self.account_id,
                    "Ignoring stale transport movement after large position delta"
                );
                return;
            }

            if transport.x.abs() > 75.0 || transport.y.abs() > 75.0 || transport.z.abs() > 75.0 {
                trace!(
                    account = self.account_id,
                    "Ignoring movement with invalid transport offset"
                );
                return;
            }

            if !wow_core::Position::new(
                pos.x + transport.x,
                pos.y + transport.y,
                pos.z + transport.z,
                pos.orientation + transport.o,
            )
            .is_valid_map_coord_like_cpp()
            {
                trace!(
                    account = self.account_id,
                    "Ignoring movement with invalid world transport coordinate"
                );
                return;
            }
        }

        self.apply_movement_side_effects_like_cpp(opcode, &info.info);
        info.info.time = self.adjust_client_movement_time_like_cpp(info.info.time);

        // Update server-side player position.
        self.set_player_position_like_cpp(info.info.position);
        // Keep the broadcast registry in sync so chat range checks are accurate.
        self.update_registry_position();
        trace!(
            account = self.account_id,
            x = pos.x,
            y = pos.y,
            z = pos.z,
            "Player moved"
        );

        // Dynamic visibility update: send new creatures/GOs that came into
        // range and remove those that left. Internally throttled to 50 yards.
        self.update_visibility().await;

        // Check area triggers at the new position
        self.check_area_triggers().await;

        // TODO: aggro proximity check re-enable once combat system is stable
        // self.check_creature_aggro().await;

        // Broadcast movement to other players on the same map.
        if let (Some(guid), Some(registry)) = (self.player_guid(), self.player_registry()) {
            use wow_core::ObjectGuid;
            use wow_network::PlayerBroadcastInfo;

            let move_update = MoveUpdate { info: info.info };
            let bytes = move_update.to_bytes();
            let current_map_id = self.player_map_id_like_cpp();

            for entry in registry.iter() {
                let (other_guid, other_info): (&ObjectGuid, &PlayerBroadcastInfo) = entry.pair();
                if *other_guid == guid {
                    continue;
                }
                if other_info.map_id != current_map_id {
                    continue;
                }
                let _ = other_info.send_tx.send(bytes.clone());
            }
        }
    }

    fn apply_movement_side_effects_like_cpp(
        &mut self,
        opcode: Option<ClientOpcodes>,
        info: &MovementInfo,
    ) {
        if matches!(opcode, Some(ClientOpcodes::MoveFallLand)) {
            self.handle_fall_like_cpp(info);
        }

        match opcode {
            Some(ClientOpcodes::MoveFallLand)
            | Some(ClientOpcodes::MoveStartSwim)
            | Some(ClientOpcodes::MoveSetFly) => {
                self.remove_auras_with_interrupt_flags_like_cpp(
                    SPELL_AURA_INTERRUPT_FLAG_LANDING_OR_FLIGHT_LIKE_CPP,
                    0,
                );
            }
            _ => {}
        }

        if matches!(
            opcode,
            Some(ClientOpcodes::MoveSetFly) | Some(ClientOpcodes::MoveSetAdvFly)
        ) {
            self.request_temporary_pet_unsummon_like_cpp();
        }

        if self.player_is_sit_state_like_cpp()
            && info
                .flags
                .intersects(MovementFlag::MASK_MOVING | MovementFlag::MASK_TURNING)
        {
            self.set_player_stand_state_like_cpp(UnitStandStateType::Stand);
        }

        if matches!(opcode, Some(ClientOpcodes::MoveJump)) {
            self.remove_auras_with_interrupt_flags_like_cpp(
                0,
                SPELL_AURA_INTERRUPT_FLAG2_JUMP_LIKE_CPP,
            );
            self.request_jump_proc_like_cpp();
        }

        self.update_fall_information_if_needed_like_cpp(
            info,
            matches!(opcode, Some(ClientOpcodes::MoveFallLand)),
        );
        self.handle_under_map_like_cpp(info);
    }

    /// Handle CMSG_SET_ACTIVE_MOVER — client sets which unit is currently being moved.
    ///
    /// The client sends this after login to establish the active mover GUID.
    /// In single‑player sessions, the mover must be the player's own GUID.
    pub async fn handle_set_active_mover(&mut self, pkt: SetActiveMover) {
        trace!(
            account = self.account_id,
            mover = ?pkt.active_mover,
            "SetActiveMover"
        );

        // Validate: in a single‑player session, the active mover must be
        // the player's own GUID (or empty, meaning "no unit moving").
        if let Some(player_guid) = self.player_guid() {
            if !pkt.active_mover.is_empty() && pkt.active_mover != player_guid {
                warn!(
                    account = self.account_id,
                    "SetActiveMover GUID mismatch: expected {:?}, got {:?}",
                    player_guid,
                    pkt.active_mover
                );
                // Not fatal; the client can be wrong, we just ignore.
            }
        }
    }

    /// Handle CMSG_MOVE_INIT_ACTIVE_MOVER_COMPLETE — client acknowledges active mover ready.
    ///
    /// In C# this updates transport timing flags and triggers visibility update.
    /// For now we just log receipt; transport timing is not yet implemented.
    pub async fn handle_move_init_active_mover_complete(
        &mut self,
        pkt: MoveInitActiveMoverComplete,
    ) {
        trace!(
            account = self.account_id,
            ticks = pkt.ticks,
            "MoveInitActiveMoverComplete"
        );
        self.apply_move_init_active_mover_complete_like_cpp(pkt.ticks);
        self.update_visibility().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{AuraApplication, RepresentedAuraEffectLikeCpp};
    use wow_core::ObjectGuid;

    fn make_session() -> WorldSession {
        let (_pkt_tx, pkt_rx) = flume::bounded(8);
        let (send_tx, _send_rx) = flume::bounded(8);
        WorldSession::new(
            1,
            "MovementTest".into(),
            0,
            2,
            9,
            54261,
            vec![0; 40],
            "esES".into(),
            pkt_rx,
            send_tx,
        )
    }

    fn visible_aura(slot: u8, flags: u32, flags2: u32) -> AuraApplication {
        AuraApplication {
            spell_id: 1000 + i32::from(slot),
            caster_guid: ObjectGuid::EMPTY,
            slot,
            duration_total: 30_000,
            duration_remaining: 30_000,
            stack_count: 1,
            aura_flags: 0x1,
            aura_interrupt_flags: flags,
            aura_interrupt_flags2: flags2,
            represented_effect: None,
            represented_amount: 0,
            represented_multiplier: 1.0,
            applied_at: std::time::Instant::now(),
        }
    }

    fn fall_aura(
        slot: u8,
        effect: RepresentedAuraEffectLikeCpp,
        amount: i32,
        multiplier: f32,
    ) -> AuraApplication {
        AuraApplication {
            represented_effect: Some(effect),
            represented_amount: amount,
            represented_multiplier: multiplier,
            ..visible_aura(slot, 0, 0)
        }
    }

    #[test]
    fn movement_landing_and_jump_remove_cpp_interruptible_auras() {
        let mut session = make_session();
        session.visible_auras.insert(
            1,
            visible_aura(1, SPELL_AURA_INTERRUPT_FLAG_LANDING_OR_FLIGHT_LIKE_CPP, 0),
        );
        session.visible_auras.insert(
            2,
            visible_aura(2, 0, SPELL_AURA_INTERRUPT_FLAG2_JUMP_LIKE_CPP),
        );
        session.visible_auras.insert(3, visible_aura(3, 0, 0));

        session.apply_movement_side_effects_like_cpp(
            Some(ClientOpcodes::MoveFallLand),
            &MovementInfo::default(),
        );
        assert!(!session.visible_auras.contains_key(&1));
        assert!(session.visible_auras.contains_key(&2));
        assert!(session.visible_auras.contains_key(&3));

        session.apply_movement_side_effects_like_cpp(
            Some(ClientOpcodes::MoveJump),
            &MovementInfo::default(),
        );
        assert!(!session.visible_auras.contains_key(&2));
        assert!(session.visible_auras.contains_key(&3));
        assert_eq!(session.movement_jump_proc_requests_like_cpp(), 1);
    }

    #[test]
    fn movement_stands_sitting_player_and_records_flying_pet_unsummon() {
        let mut session = make_session();
        session.set_player_stand_state_like_cpp(UnitStandStateType::SitChair);
        let mut info = MovementInfo::default();
        info.flags = MovementFlag::FORWARD;

        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveSetFly), &info);

        assert_eq!(
            session.player_stand_state_like_cpp(),
            UnitStandStateType::Stand
        );
        assert_eq!(session.temporary_pet_unsummon_requests_like_cpp(), 1);
    }

    #[test]
    fn movement_fall_land_applies_cpp_base_fall_damage_and_updates_fall_info() {
        let mut session = make_session();
        session.set_player_health_like_cpp(1_000, 1_000);
        session.set_fall_information_like_cpp(1_200, 120.0);
        let mut info = MovementInfo::default();
        info.position.z = 100.0;
        info.jump.fall_time = 1_500;

        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);

        let events = session.fall_damage_events_like_cpp();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].damage, 117);
        assert_eq!(events[0].final_damage, 117);
        assert_eq!(session.player_health_like_cpp(), 883);

        let mut harmless = MovementInfo::default();
        harmless.position.z = 99.0;
        harmless.jump.fall_time = 1_600;
        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &harmless);
        assert_eq!(session.fall_damage_events_like_cpp().len(), 1);
    }

    #[test]
    fn movement_fall_damage_applies_cpp_aura_modifiers_and_guards() {
        let mut session = make_session();
        session.set_player_health_like_cpp(1_000, 1_000);
        session.set_fall_information_like_cpp(1_200, 150.0);
        session.visible_auras.insert(
            4,
            fall_aura(4, RepresentedAuraEffectLikeCpp::SafeFall, 10, 1.0),
        );
        session.visible_auras.insert(
            5,
            fall_aura(5, RepresentedAuraEffectLikeCpp::ModifyFallDamagePct, 0, 0.5),
        );
        let mut info = MovementInfo::default();
        info.position.z = 100.0;
        info.jump.fall_time = 1_500;

        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);

        let events = session.fall_damage_events_like_cpp();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].damage, 238);
        assert_eq!(events[0].final_damage, 238);
        assert_eq!(session.player_health_like_cpp(), 762);

        let mut guarded = make_session();
        guarded.set_player_health_like_cpp(1_000, 1_000);
        guarded.set_fall_information_like_cpp(1_200, 150.0);
        guarded.visible_auras.insert(
            6,
            fall_aura(6, RepresentedAuraEffectLikeCpp::FeatherFall, 0, 1.0),
        );
        guarded.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);
        assert!(guarded.fall_damage_events_like_cpp().is_empty());

        let mut god = make_session();
        god.set_player_health_like_cpp(1_000, 1_000);
        god.set_fall_information_like_cpp(1_200, 150.0);
        god.set_player_cheat_god_like_cpp(true);
        god.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);
        assert!(god.fall_damage_events_like_cpp().is_empty());

        let mut gm = make_session();
        gm.set_player_health_like_cpp(1_000, 1_000);
        gm.set_fall_information_like_cpp(1_200, 150.0);
        gm.set_player_game_master_like_cpp(true);
        gm.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);
        assert!(gm.fall_damage_events_like_cpp().is_empty());

        let mut immune = make_session();
        immune.set_player_health_like_cpp(1_000, 1_000);
        immune.set_fall_information_like_cpp(1_200, 150.0);
        immune.set_player_normal_damage_immune_like_cpp(true);
        immune.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);
        assert!(immune.fall_damage_events_like_cpp().is_empty());

        let mut environmental = make_session();
        environmental.set_player_health_like_cpp(1_000, 1_000);
        environmental.set_fall_information_like_cpp(1_200, 150.0);
        environmental.set_player_environmental_damage_immune_like_cpp(true);
        environmental
            .apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveFallLand), &info);
        assert_eq!(environmental.fall_damage_events_like_cpp()[0].damage, 657);
        assert_eq!(
            environmental.fall_damage_events_like_cpp()[0].final_damage,
            0
        );
        assert_eq!(environmental.player_health_like_cpp(), 1_000);
    }

    #[test]
    fn movement_under_map_applies_cpp_void_damage_and_flag() {
        let mut session = make_session();
        session.set_player_health_like_cpp(1_000, 1_000);
        let mut info = MovementInfo::default();
        info.position.z = -501.0;

        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveHeartbeat), &info);

        assert_eq!(session.under_map_damage_events_like_cpp().len(), 1);
        assert_eq!(
            session.under_map_damage_events_like_cpp()[0].min_height,
            crate::map_manager::DEFAULT_MIN_HEIGHT_LIKE_CPP
        );
        assert_eq!(session.player_health_like_cpp(), 0);
        assert!(!session.player_is_alive_like_cpp());
        assert!(session.player_out_of_bounds_like_cpp());

        info.position.z = -499.0;
        session.apply_movement_side_effects_like_cpp(Some(ClientOpcodes::MoveHeartbeat), &info);
        assert!(!session.player_out_of_bounds_like_cpp());
    }

    #[test]
    fn move_init_active_mover_complete_sets_cpp_transport_state() {
        let mut session = make_session();
        let before = WorldSession::game_time_ms_like_cpp();

        session.apply_move_init_active_mover_complete_like_cpp(25);

        assert!(
            session.active_player_local_flags_like_cpp()
                & crate::session::PLAYER_LOCAL_FLAG_OVERRIDE_TRANSPORT_SERVER_TIME_LIKE_CPP
                != 0
        );
        assert!(session.active_player_transport_server_time_like_cpp() >= 0);
        assert!(
            session.active_player_transport_server_time_like_cpp()
                <= WorldSession::game_time_ms_like_cpp() as i32
        );
        assert!(
            session.active_player_transport_server_time_like_cpp()
                >= before.saturating_sub(25) as i32
        );
        assert_eq!(session.movement_visibility_refresh_requests_like_cpp(), 1);
    }
}

// ── Handler registration (SetActiveMover) ────────────────────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::SetActiveMover,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_set_active_mover",
    }
}

// ── Handler registration (MoveInitActiveMoverComplete) ───────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::MoveInitActiveMoverComplete,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_move_init_active_mover_complete",
    }
}
