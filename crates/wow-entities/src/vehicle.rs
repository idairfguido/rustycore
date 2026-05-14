use std::collections::BTreeMap;

use wow_constants::TypeId;
use wow_core::{ObjectGuid, Position};

pub const MAX_VEHICLE_SEATS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VehicleFlag {
    NoStrafe = 0x0000_0001,
    NoJumping = 0x0000_0002,
    FullSpeedTurning = 0x0000_0004,
    AllowPitching = 0x0000_0010,
    FullSpeedPitching = 0x0000_0020,
    CustomPitch = 0x0000_0040,
    AdjustAimAngle = 0x0000_0400,
    AdjustAimPower = 0x0000_0800,
    FixedPosition = 0x0020_0000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VehicleExitParameter {
    None = 0,
    Offset = 1,
    Destination = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleStatus {
    None,
    Installed,
    Uninstalling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassengerInfo {
    pub guid: ObjectGuid,
    pub is_uninteractible: bool,
    pub is_gravity_disabled: bool,
}

impl PassengerInfo {
    pub const fn empty() -> Self {
        Self {
            guid: ObjectGuid::EMPTY,
            is_uninteractible: false,
            is_gravity_disabled: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.guid.is_empty()
    }

    pub fn reset(&mut self) {
        *self = Self::empty();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleSeatAddon {
    pub seat_orientation_offset: f32,
    pub exit_parameter_x: f32,
    pub exit_parameter_y: f32,
    pub exit_parameter_z: f32,
    pub exit_parameter_o: f32,
    pub exit_parameter: VehicleExitParameter,
}

impl Default for VehicleSeatAddon {
    fn default() -> Self {
        Self {
            seat_orientation_offset: 0.0,
            exit_parameter_x: 0.0,
            exit_parameter_y: 0.0,
            exit_parameter_z: 0.0,
            exit_parameter_o: 0.0,
            exit_parameter: VehicleExitParameter::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleSeatInfo {
    pub id: u32,
    pub can_enter_or_exit: bool,
    pub usable_by_override: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleSeat {
    pub seat_info: VehicleSeatInfo,
    pub seat_addon: VehicleSeatAddon,
    pub passenger: PassengerInfo,
}

impl VehicleSeat {
    pub const fn new(seat_info: VehicleSeatInfo, seat_addon: VehicleSeatAddon) -> Self {
        Self {
            seat_info,
            seat_addon,
            passenger: PassengerInfo::empty(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.passenger.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleAccessory {
    pub accessory_entry: u32,
    pub is_minion: bool,
    pub summon_time_ms: u32,
    pub seat_id: i8,
    pub summoned_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VehicleTemplate {
    pub despawn_delay_ms: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    base_guid: ObjectGuid,
    base_type_id: TypeId,
    base_position: Position,
    vehicle_id: u32,
    creature_entry: u32,
    usable_seat_num: u32,
    seats: BTreeMap<i8, VehicleSeat>,
    status: VehicleStatus,
    pending_join_events: BTreeMap<ObjectGuid, i8>,
}

impl Vehicle {
    pub fn new(
        base_guid: ObjectGuid,
        base_type_id: TypeId,
        base_position: Position,
        vehicle_id: u32,
        creature_entry: u32,
        seat_defs: impl IntoIterator<Item = (i8, VehicleSeatInfo, VehicleSeatAddon)>,
    ) -> Self {
        let mut seats = BTreeMap::new();
        let mut usable_seat_num = 0;
        for (seat_id, seat_info, seat_addon) in seat_defs.into_iter().take(MAX_VEHICLE_SEATS) {
            if seat_info.can_enter_or_exit {
                usable_seat_num += 1;
            }
            seats.insert(seat_id, VehicleSeat::new(seat_info, seat_addon));
        }

        Self {
            base_guid,
            base_type_id,
            base_position,
            vehicle_id,
            creature_entry,
            usable_seat_num,
            seats,
            status: VehicleStatus::None,
            pending_join_events: BTreeMap::new(),
        }
    }

    pub const fn base_guid(&self) -> ObjectGuid {
        self.base_guid
    }

    pub const fn base_type_id(&self) -> TypeId {
        self.base_type_id
    }

    pub const fn base_position(&self) -> Position {
        self.base_position
    }

    pub fn set_base_position(&mut self, position: Position) {
        self.base_position = position;
    }

    pub const fn vehicle_id(&self) -> u32 {
        self.vehicle_id
    }

    pub const fn creature_entry(&self) -> u32 {
        self.creature_entry
    }

    pub const fn usable_seat_num(&self) -> u32 {
        self.usable_seat_num
    }

    pub const fn status(&self) -> VehicleStatus {
        self.status
    }

    pub fn seats(&self) -> &BTreeMap<i8, VehicleSeat> {
        &self.seats
    }

    pub fn install(&mut self) {
        self.status = VehicleStatus::Installed;
    }

    pub fn uninstall(&mut self) {
        self.status = VehicleStatus::Uninstalling;
        self.remove_all_passengers();
    }

    pub fn has_empty_seat(&self, seat_id: i8) -> bool {
        self.seats.get(&seat_id).is_some_and(VehicleSeat::is_empty)
            && !self.has_pending_event_for_seat(seat_id)
    }

    pub fn passenger(&self, seat_id: i8) -> Option<ObjectGuid> {
        let passenger = self.seats.get(&seat_id)?.passenger.guid;
        (!passenger.is_empty()).then_some(passenger)
    }

    pub fn available_seat_count(&self) -> u8 {
        self.seats
            .values()
            .filter(|seat| {
                seat.is_empty()
                    && (seat.seat_info.can_enter_or_exit || seat.seat_info.usable_by_override)
            })
            .count()
            .min(u8::MAX as usize) as u8
    }

    pub fn add_vehicle_passenger(&mut self, passenger: ObjectGuid, seat_id: i8) -> bool {
        if self.has_pending_event_for_seat(seat_id) {
            return false;
        }
        let Some(seat) = self.seats.get_mut(&seat_id) else {
            return false;
        };
        if !seat.is_empty() {
            return false;
        }

        seat.passenger.guid = passenger;
        true
    }

    pub fn remove_passenger(&mut self, passenger: ObjectGuid) -> Option<i8> {
        for (seat_id, seat) in &mut self.seats {
            if seat.passenger.guid == passenger {
                seat.passenger.reset();
                return Some(*seat_id);
            }
        }
        None
    }

    pub fn remove_all_passengers(&mut self) {
        self.pending_join_events.clear();
        for seat in self.seats.values_mut() {
            seat.passenger.reset();
        }
    }

    pub fn is_vehicle_in_use(&self) -> bool {
        self.seats.values().any(|seat| !seat.is_empty())
    }

    pub fn is_controllable_vehicle(&self) -> bool {
        self.usable_seat_num != 0
    }

    pub fn add_pending_event(&mut self, passenger: ObjectGuid, seat_id: i8) {
        self.pending_join_events.insert(passenger, seat_id);
    }

    pub fn remove_pending_events_for_passenger(&mut self, passenger: ObjectGuid) {
        self.pending_join_events.remove(&passenger);
    }

    pub fn remove_pending_events_for_seat(&mut self, seat_id: i8) {
        self.pending_join_events
            .retain(|_, pending_seat| *pending_seat != seat_id);
    }

    pub fn has_pending_event_for_seat(&self, seat_id: i8) -> bool {
        self.pending_join_events
            .values()
            .any(|pending_seat| *pending_seat == seat_id)
    }

    pub fn next_empty_seat(&self, seat_id: i8, next: bool) -> Option<i8> {
        if !self.seats.contains_key(&seat_id) || self.seats.is_empty() {
            return None;
        }

        let seat_ids: Vec<i8> = self.seats.keys().copied().collect();
        let mut index = seat_ids.iter().position(|known| *known == seat_id)?;
        loop {
            index = if next {
                (index + 1) % seat_ids.len()
            } else if index == 0 {
                seat_ids.len() - 1
            } else {
                index - 1
            };

            let candidate = seat_ids[index];
            if candidate == seat_id {
                return None;
            }
            let seat = self.seats.get(&candidate)?;
            if seat.is_empty()
                && !self.has_pending_event_for_seat(candidate)
                && (seat.seat_info.can_enter_or_exit || seat.seat_info.usable_by_override)
            {
                return Some(candidate);
            }
        }
    }

    pub fn calculate_passenger_position(&self, offset: Position) -> Position {
        calculate_passenger_position(offset, self.base_position)
    }

    pub fn calculate_passenger_offset(&self, global: Position) -> Position {
        calculate_passenger_offset(global, self.base_position)
    }
}

pub fn calculate_passenger_position(offset: Position, transport: Position) -> Position {
    Position::new(
        transport.x + offset.x * transport.orientation.cos()
            - offset.y * transport.orientation.sin(),
        transport.y
            + offset.y * transport.orientation.cos()
            + offset.x * transport.orientation.sin(),
        transport.z + offset.z,
        normalize_orientation(transport.orientation + offset.orientation),
    )
}

pub fn calculate_passenger_offset(global: Position, transport: Position) -> Position {
    let mut x = global.x - transport.x;
    let mut y = global.y - transport.y;
    let z = global.z - transport.z;
    let orientation = normalize_orientation(global.orientation - transport.orientation);

    let inx = x;
    let iny = y;
    let tan = transport.orientation.tan();
    let denom = transport.orientation.cos() + transport.orientation.sin() * tan;
    y = (iny - inx * tan) / denom;
    x = (inx + iny * tan) / denom;

    Position::new(x, y, z, orientation)
}

fn normalize_orientation(mut orientation: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    orientation %= tau;
    if orientation < 0.0 {
        orientation += tau;
    }
    orientation
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::guid::HighGuid;

    fn base_guid() -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 530, 100, 0, 1)
    }

    fn passenger_guid(counter: i64) -> ObjectGuid {
        ObjectGuid::create_global(HighGuid::Player, 0, counter)
    }

    fn seat(id: u32, can_enter_or_exit: bool) -> VehicleSeatInfo {
        VehicleSeatInfo {
            id,
            can_enter_or_exit,
            usable_by_override: false,
        }
    }

    fn vehicle() -> Vehicle {
        Vehicle::new(
            base_guid(),
            TypeId::Unit,
            Position::new(10.0, 20.0, 30.0, 1.0),
            123,
            456,
            [
                (0, seat(1000, true), VehicleSeatAddon::default()),
                (1, seat(1001, false), VehicleSeatAddon::default()),
                (
                    2,
                    VehicleSeatInfo {
                        id: 1002,
                        can_enter_or_exit: false,
                        usable_by_override: true,
                    },
                    VehicleSeatAddon::default(),
                ),
            ],
        )
    }

    #[test]
    fn vehicle_constructor_matches_cpp_base_state() {
        let vehicle = vehicle();

        assert_eq!(vehicle.base_guid(), base_guid());
        assert_eq!(vehicle.base_type_id(), TypeId::Unit);
        assert_eq!(vehicle.vehicle_id(), 123);
        assert_eq!(vehicle.creature_entry(), 456);
        assert_eq!(vehicle.status(), VehicleStatus::None);
        assert_eq!(vehicle.usable_seat_num(), 1);
        assert_eq!(vehicle.seats().len(), 3);
        assert!(vehicle.has_empty_seat(0));
        assert!(!vehicle.has_empty_seat(99));
        assert_eq!(vehicle.available_seat_count(), 2);
        assert!(vehicle.is_controllable_vehicle());
        assert!(!vehicle.is_vehicle_in_use());
    }

    #[test]
    fn install_uninstall_and_passengers_follow_cpp_shape() {
        let mut vehicle = vehicle();
        vehicle.install();
        assert_eq!(vehicle.status(), VehicleStatus::Installed);

        assert!(vehicle.add_vehicle_passenger(passenger_guid(1), 0));
        assert_eq!(vehicle.passenger(0), Some(passenger_guid(1)));
        assert!(vehicle.is_vehicle_in_use());
        assert!(!vehicle.add_vehicle_passenger(passenger_guid(2), 0));
        assert_eq!(vehicle.remove_passenger(passenger_guid(1)), Some(0));
        assert!(vehicle.passenger(0).is_none());

        vehicle.add_pending_event(passenger_guid(3), 0);
        assert!(vehicle.has_pending_event_for_seat(0));
        assert!(!vehicle.has_empty_seat(0));
        vehicle.remove_pending_events_for_passenger(passenger_guid(3));
        assert!(vehicle.has_empty_seat(0));

        vehicle.add_vehicle_passenger(passenger_guid(4), 0);
        vehicle.add_pending_event(passenger_guid(5), 2);
        vehicle.uninstall();
        assert_eq!(vehicle.status(), VehicleStatus::Uninstalling);
        assert!(!vehicle.is_vehicle_in_use());
        assert!(!vehicle.has_pending_event_for_seat(2));
    }

    #[test]
    fn next_empty_seat_skips_occupied_pending_and_unusable() {
        let mut vehicle = vehicle();
        assert!(vehicle.add_vehicle_passenger(passenger_guid(1), 0));
        vehicle.add_pending_event(passenger_guid(2), 2);

        assert_eq!(vehicle.next_empty_seat(0, true), None);
        vehicle.remove_pending_events_for_seat(2);
        assert_eq!(vehicle.next_empty_seat(0, true), Some(2));
    }

    #[test]
    fn transport_position_transforms_match_cpp_formula() {
        let vehicle = vehicle();
        let offset = Position::new(2.0, 3.0, 4.0, 0.5);

        let global = vehicle.calculate_passenger_position(offset);
        let roundtrip = vehicle.calculate_passenger_offset(global);

        assert!((roundtrip.x - offset.x).abs() < 0.0001);
        assert!((roundtrip.y - offset.y).abs() < 0.0001);
        assert!((roundtrip.z - offset.z).abs() < 0.0001);
        assert!((roundtrip.orientation - offset.orientation).abs() < 0.0001);
    }
}
