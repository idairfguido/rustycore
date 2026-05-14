//! Vehicle packet handler registrations.
//!
//! C++ refs:
//! - `WorldSession::HandleMoveDismissVehicle`
//! - `WorldSession::HandleRequestVehiclePrevSeat`
//! - `WorldSession::HandleRequestVehicleNextSeat`
//! - `WorldSession::HandleMoveChangeVehicleSeats`
//! - `WorldSession::HandleRequestVehicleSwitchSeat`
//! - `WorldSession::HandleRideVehicleInteract`
//! - `WorldSession::HandleEjectPassenger`
//! - `WorldSession::HandleRequestVehicleExit`

use wow_constants::ClientOpcodes;
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_packet::packets::vehicle::{
    EjectPassenger, MoveChangeVehicleSeats, MoveDismissVehicle, RequestVehicleExit,
    RequestVehicleNextSeat, RequestVehiclePrevSeat, RequestVehicleSwitchSeat, RideVehicleInteract,
};

use crate::session::WorldSession;

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::MoveDismissVehicle,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadSafe,
        handler_name: "handle_move_dismiss_vehicle",
    }
}

impl WorldSession {
    /// C++ `HandleMoveDismissVehicle`.
    ///
    /// Full `Player::ExitVehicle` requires live charm/passenger ownership. Until that runtime
    /// exists, this handler intentionally mirrors the C++ early-return shape by doing nothing
    /// when the session has no represented passenger vehicle.
    pub async fn handle_move_dismiss_vehicle(&mut self, _packet: MoveDismissVehicle) {}

    /// C++ `HandleRequestVehiclePrevSeat`.
    pub async fn handle_request_vehicle_prev_seat(&mut self, _packet: RequestVehiclePrevSeat) {}

    /// C++ `HandleRequestVehicleNextSeat`.
    pub async fn handle_request_vehicle_next_seat(&mut self, _packet: RequestVehicleNextSeat) {}

    /// C++ `HandleMoveChangeVehicleSeats`.
    pub async fn handle_move_change_vehicle_seats(&mut self, _packet: MoveChangeVehicleSeats) {}

    /// C++ `HandleRequestVehicleSwitchSeat`.
    pub async fn handle_request_vehicle_switch_seat(&mut self, _packet: RequestVehicleSwitchSeat) {}

    /// C++ `HandleRideVehicleInteract`.
    pub async fn handle_ride_vehicle_interact(&mut self, _packet: RideVehicleInteract) {}

    /// C++ `HandleEjectPassenger`.
    pub async fn handle_eject_passenger(&mut self, _packet: EjectPassenger) {}

    /// C++ `HandleRequestVehicleExit`.
    pub async fn handle_request_vehicle_exit(&mut self, _packet: RequestVehicleExit) {}
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestVehiclePrevSeat,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_vehicle_prev_seat",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestVehicleNextSeat,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_vehicle_next_seat",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::MoveChangeVehicleSeats,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadSafe,
        handler_name: "handle_move_change_vehicle_seats",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestVehicleSwitchSeat,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_vehicle_switch_seat",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RideVehicleInteract,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_ride_vehicle_interact",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::EjectPassenger,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_eject_passenger",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestVehicleExit,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_vehicle_exit",
    }
}
