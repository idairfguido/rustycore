use std::collections::BTreeSet;

use wow_core::{ObjectGuid, Position};

use crate::{
    CreateObjectFlags, GameObject, GoState, WorldObject, calculate_passenger_offset,
    calculate_passenger_position,
};

pub const GAMEOBJECT_TYPE_MAP_OBJ_TRANSPORT: u8 = 15;
pub const GO_DYNFLAG_LO_STOPPED: u32 = 0x0040;
pub const TRANSPORT_POSITION_UPDATE_DELAY_MS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransportMovementState {
    Moving = 0,
    WaitingOnPauseWaypoint = 1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportPathSegment {
    pub segment_end_arrival_timestamp_ms: u32,
    pub delay_ms: u32,
    pub distance_from_leg_start_at_end: f64,
}

impl Default for TransportPathSegment {
    fn default() -> Self {
        Self {
            segment_end_arrival_timestamp_ms: 0,
            delay_ms: 0,
            distance_from_leg_start_at_end: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransportPathEvent {
    pub timestamp_ms: u32,
    pub event_id: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransportPathLeg {
    pub map_id: u32,
    pub start_timestamp_ms: u32,
    pub duration_ms: u32,
    pub segments: Vec<TransportPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransportTemplate {
    pub total_path_time_ms: u32,
    pub speed: f64,
    pub acceleration_rate: f64,
    pub acceleration_time: f64,
    pub acceleration_distance: f64,
    pub path_legs: Vec<TransportPathLeg>,
    pub events: Vec<TransportPathEvent>,
    pub map_ids: BTreeSet<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPassengerSet {
    Dynamic,
    Static,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportCreateInfo {
    pub entry: u32,
    pub display_id: u32,
    pub object_scale: f32,
    pub name: String,
    pub period_ms: u32,
    pub path_progress_ms: u32,
    pub allow_stopping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportUpdateLikeCpp {
    pub diff_ms: u32,
    pub now_ms: u64,
    pub old_path_progress_ms: u32,
    pub new_path_progress_ms: u32,
    pub timer_ms: Option<u32>,
    pub period_ms: u32,
    pub allow_stopping: bool,
    pub old_cycle_id: Option<u32>,
    pub new_cycle_id: Option<u32>,
    pub cycle_reset: bool,
    pub path_progress_for_client_updated: bool,
    pub position_update_timer_before_ms: u32,
    pub position_update_timer_after_ms: u32,
    pub position_update_timer_passed: bool,
    pub movement_state_before: TransportMovementState,
    pub movement_state_after: TransportMovementState,
    pub just_stopped: bool,
    pub stopped_dynflag_represented: bool,
    pub expected_map_id: Option<u32>,
    pub current_map_id: u32,
    pub expected_map_matches_current_map: bool,
    pub position_update_due: bool,
    pub position_update_represented: bool,
    pub compute_position_not_represented: bool,
    pub ai_update_not_represented: bool,
    pub script_update_not_represented: bool,
    pub events_trigger_not_represented: bool,
    pub unsupported_no_period: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transport {
    game_object: GameObject,
    template: Option<TransportTemplate>,
    movement_state: TransportMovementState,
    allow_stopping: bool,
    events_to_trigger: Vec<bool>,
    current_path_leg: usize,
    request_stop_timestamp_ms: Option<u32>,
    path_progress_ms: u32,
    position_change_timer_ms: u32,
    passengers: BTreeSet<ObjectGuid>,
    static_passengers: BTreeSet<ObjectGuid>,
    delayed_add_model: bool,
}

impl Transport {
    pub fn new() -> Self {
        let mut game_object = GameObject::new();
        game_object
            .world_mut()
            .object_mut()
            .create_flags_mut()
            .insert(CreateObjectFlags::SERVER_TIME);

        Self {
            game_object,
            template: None,
            movement_state: TransportMovementState::Moving,
            // Conservative default for template-less tests/scaffolds: allow stop requests
            // to be represented unless DB-backed create info says otherwise.
            allow_stopping: true,
            events_to_trigger: Vec::new(),
            current_path_leg: 0,
            request_stop_timestamp_ms: None,
            path_progress_ms: 0,
            position_change_timer_ms: 0,
            passengers: BTreeSet::new(),
            static_passengers: BTreeSet::new(),
            delayed_add_model: false,
        }
    }

    pub fn with_template(template: TransportTemplate) -> Self {
        let mut transport = Self::new();
        transport.events_to_trigger = vec![true; template.events.len()];
        transport.set_period(template.total_path_time_ms);
        transport.template = Some(template);
        transport
    }

    pub const fn game_object(&self) -> &GameObject {
        &self.game_object
    }

    pub fn game_object_mut(&mut self) -> &mut GameObject {
        &mut self.game_object
    }

    pub const fn world(&self) -> &WorldObject {
        self.game_object.world()
    }

    pub fn world_mut(&mut self) -> &mut WorldObject {
        self.game_object.world_mut()
    }

    pub const fn template(&self) -> Option<&TransportTemplate> {
        self.template.as_ref()
    }

    pub const fn movement_state(&self) -> TransportMovementState {
        self.movement_state
    }

    pub fn set_movement_state(&mut self, movement_state: TransportMovementState) {
        self.movement_state = movement_state;
    }

    pub const fn allow_stopping(&self) -> bool {
        self.allow_stopping
    }

    pub fn set_allow_stopping(&mut self, allow_stopping: bool) {
        self.allow_stopping = allow_stopping;
    }

    pub const fn current_path_leg(&self) -> usize {
        self.current_path_leg
    }

    pub fn set_current_path_leg(&mut self, current_path_leg: usize) {
        self.current_path_leg = current_path_leg;
    }

    pub const fn request_stop_timestamp_ms(&self) -> Option<u32> {
        self.request_stop_timestamp_ms
    }

    pub const fn path_progress_ms(&self) -> u32 {
        self.path_progress_ms
    }

    pub fn set_path_progress_ms(&mut self, path_progress_ms: u32) {
        self.path_progress_ms = path_progress_ms;
        let period = self.get_transport_period();
        if period != 0 {
            self.game_object
                .set_path_progress_for_client(path_progress_ms as f32 / period as f32);
        }
    }

    pub const fn position_change_timer_ms(&self) -> u32 {
        self.position_change_timer_ms
    }

    pub fn set_position_change_timer_ms(&mut self, timer_ms: u32) {
        self.position_change_timer_ms = timer_ms;
    }

    pub fn passengers(&self) -> &BTreeSet<ObjectGuid> {
        &self.passengers
    }

    pub fn static_passengers(&self) -> &BTreeSet<ObjectGuid> {
        &self.static_passengers
    }

    pub const fn delayed_add_model(&self) -> bool {
        self.delayed_add_model
    }

    pub fn set_delayed_add_model_to_map(&mut self) {
        self.delayed_add_model = true;
    }

    pub fn clear_delayed_add_model_to_map(&mut self) {
        self.delayed_add_model = false;
    }

    pub fn events_to_trigger(&self) -> &[bool] {
        &self.events_to_trigger
    }

    pub fn mark_event_triggered(&mut self, event_index: usize) -> bool {
        let Some(event) = self.events_to_trigger.get_mut(event_index) else {
            return false;
        };

        *event = false;
        true
    }

    pub fn initialize_created_state(&mut self, info: TransportCreateInfo) {
        self.game_object
            .world_mut()
            .object_mut()
            .set_scale(info.object_scale);
        self.game_object
            .world_mut()
            .object_mut()
            .set_entry(info.entry);
        self.game_object.set_display_id(info.display_id);
        self.allow_stopping = info.allow_stopping;
        self.game_object.set_go_state(if info.allow_stopping {
            GoState::Active
        } else {
            GoState::Ready
        });
        self.game_object
            .set_go_type(GAMEOBJECT_TYPE_MAP_OBJ_TRANSPORT);
        self.game_object.world_mut().set_name(info.name);
        self.set_period(info.period_ms);
        self.set_path_progress_ms(info.path_progress_ms);
    }

    pub fn add_passenger(&mut self, passenger: ObjectGuid) -> bool {
        if !self.world().object().is_in_world() {
            return false;
        }

        self.passengers.insert(passenger)
    }

    pub fn add_static_passenger(&mut self, passenger: ObjectGuid) -> bool {
        self.static_passengers.insert(passenger)
    }

    pub fn remove_passenger(&mut self, passenger: ObjectGuid) -> Option<TransportPassengerSet> {
        if self.passengers.remove(&passenger) {
            Some(TransportPassengerSet::Dynamic)
        } else if self.static_passengers.remove(&passenger) {
            Some(TransportPassengerSet::Static)
        } else {
            None
        }
    }

    pub fn unload_static_passengers(&mut self) -> Vec<ObjectGuid> {
        let passengers = self.static_passengers.iter().copied().collect();
        self.static_passengers.clear();
        passengers
    }

    pub fn cleanup_before_delete(&mut self) -> Vec<ObjectGuid> {
        let mut removed = self.unload_static_passengers();
        removed.extend(self.passengers.iter().copied());
        self.passengers.clear();
        removed
    }

    pub fn set_period(&mut self, period_ms: u32) {
        self.game_object.set_level(period_ms);
    }

    pub fn get_transport_period(&self) -> u32 {
        self.game_object.data().level.max(0) as u32
    }

    pub const fn get_timer(&self) -> u32 {
        self.path_progress_ms
    }

    pub fn request_stop_at_next_pause(&mut self, next_pause_timestamp_ms: u32) {
        let period = self.get_transport_period();
        self.request_stop_timestamp_ms = Some(if period == 0 {
            next_pause_timestamp_ms
        } else {
            (self.path_progress_ms / period) * period + next_pause_timestamp_ms
        });
    }

    pub fn enable_movement(&mut self) {
        self.request_stop_timestamp_ms = None;
        self.game_object.set_go_state(GoState::Active);
        self.game_object
            .world_mut()
            .object_mut()
            .remove_dynamic_flag(GO_DYNFLAG_LO_STOPPED);
    }

    pub fn update_position(&mut self, position: Position) {
        self.world_mut().relocate(position);
    }

    pub fn get_expected_map_id(&self) -> Option<u32> {
        self.template
            .as_ref()?
            .path_legs
            .get(self.current_path_leg)
            .map(|leg| leg.map_id)
    }

    /// Bounded local-state representation of TrinityCore `Transport::Update(uint32 diff)`.
    ///
    /// This deliberately does not run AI/scripts, `GameEvents`, real
    /// `TransportInfo::ComputePosition`, `TeleportTransport`, static passenger DB
    /// spawning/removal, passenger relocation, packet fanout, or grid/model movement.
    pub fn update_like_cpp(
        &mut self,
        diff_ms: u32,
        now_ms: u64,
        current_map_id: u32,
    ) -> TransportUpdateLikeCpp {
        let period_ms = self.get_transport_period();
        let old_path_progress_ms = self.path_progress_ms;
        let movement_state_before = self.movement_state;
        let position_update_timer_before_ms = self.position_change_timer_ms;
        let expected_map_id = self.get_expected_map_id();
        let expected_map_matches_current_map = expected_map_id == Some(current_map_id);

        if period_ms == 0 {
            return TransportUpdateLikeCpp {
                diff_ms,
                now_ms,
                old_path_progress_ms,
                new_path_progress_ms: old_path_progress_ms,
                timer_ms: None,
                period_ms,
                allow_stopping: self.allow_stopping,
                old_cycle_id: None,
                new_cycle_id: None,
                cycle_reset: false,
                path_progress_for_client_updated: false,
                position_update_timer_before_ms,
                position_update_timer_after_ms: position_update_timer_before_ms,
                position_update_timer_passed: false,
                movement_state_before,
                movement_state_after: self.movement_state,
                just_stopped: false,
                stopped_dynflag_represented: false,
                expected_map_id,
                current_map_id,
                expected_map_matches_current_map,
                position_update_due: false,
                position_update_represented: false,
                compute_position_not_represented: false,
                ai_update_not_represented: true,
                script_update_not_represented: true,
                events_trigger_not_represented: false,
                unsupported_no_period: true,
            };
        }

        let position_timer_after_update_ms = self.position_change_timer_ms.saturating_sub(diff_ms);
        let position_update_timer_passed = position_timer_after_update_ms == 0;
        self.position_change_timer_ms = position_timer_after_update_ms;

        let old_cycle_id = old_path_progress_ms / period_ms;
        let stop_request_within_tick = self.allow_stopping
            && self
                .request_stop_timestamp_ms
                .is_some_and(|request| request <= old_path_progress_ms.saturating_add(diff_ms));

        let new_path_progress_ms = if !self.allow_stopping {
            now_ms.min(u64::from(u32::MAX)) as u32
        } else if let Some(request_stop_timestamp_ms) = self.request_stop_timestamp_ms {
            if request_stop_timestamp_ms > old_path_progress_ms.saturating_add(diff_ms) {
                old_path_progress_ms.saturating_add(diff_ms)
            } else {
                request_stop_timestamp_ms
            }
        } else {
            old_path_progress_ms.saturating_add(diff_ms)
        };

        self.set_path_progress_ms(new_path_progress_ms);
        let new_cycle_id = new_path_progress_ms / period_ms;
        let cycle_reset = old_cycle_id != new_cycle_id;
        if cycle_reset {
            self.events_to_trigger.fill(true);
        }

        let just_stopped = movement_state_before == TransportMovementState::Moving
            && stop_request_within_tick
            && self.request_stop_timestamp_ms == Some(new_path_progress_ms);
        let stopped_dynflag_represented = just_stopped;
        if just_stopped {
            self.movement_state = TransportMovementState::WaitingOnPauseWaypoint;
            self.game_object.set_go_state(GoState::Ready);
            self.game_object
                .world_mut()
                .object_mut()
                .set_dynamic_flag(GO_DYNFLAG_LO_STOPPED);
        }

        let moving_or_just_stopped =
            self.movement_state == TransportMovementState::Moving || just_stopped;
        let position_update_timer_reset =
            position_update_timer_passed && expected_map_matches_current_map;
        if position_update_timer_reset {
            self.position_change_timer_ms = TRANSPORT_POSITION_UPDATE_DELAY_MS;
        }
        let position_update_due = position_update_timer_reset && moving_or_just_stopped;

        TransportUpdateLikeCpp {
            diff_ms,
            now_ms,
            old_path_progress_ms,
            new_path_progress_ms,
            timer_ms: Some(new_path_progress_ms % period_ms),
            period_ms,
            allow_stopping: self.allow_stopping,
            old_cycle_id: Some(old_cycle_id),
            new_cycle_id: Some(new_cycle_id),
            cycle_reset,
            path_progress_for_client_updated: true,
            position_update_timer_before_ms,
            position_update_timer_after_ms: self.position_change_timer_ms,
            position_update_timer_passed,
            movement_state_before,
            movement_state_after: self.movement_state,
            just_stopped,
            stopped_dynflag_represented,
            expected_map_id,
            current_map_id,
            expected_map_matches_current_map,
            position_update_due,
            position_update_represented: position_update_due,
            compute_position_not_represented: position_update_due,
            ai_update_not_represented: true,
            script_update_not_represented: true,
            events_trigger_not_represented: cycle_reset,
            unsupported_no_period: false,
        }
    }

    pub fn calculate_passenger_position(&self, offset: Position) -> Position {
        calculate_passenger_position(offset, self.world().position())
    }

    pub fn calculate_passenger_offset(&self, global: Position) -> Position {
        calculate_passenger_offset(global, self.world().position())
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{TypeId, TypeMask};
    use wow_core::guid::HighGuid;

    fn transport_guid() -> ObjectGuid {
        ObjectGuid::create_transport(HighGuid::Transport, 100)
    }

    fn passenger_guid(counter: i64) -> ObjectGuid {
        ObjectGuid::create_global(HighGuid::Player, 0, counter)
    }

    fn transport_template_for_map(map_id: u32, period_ms: u32) -> TransportTemplate {
        TransportTemplate {
            total_path_time_ms: period_ms,
            path_legs: vec![TransportPathLeg {
                map_id,
                start_timestamp_ms: 0,
                duration_ms: period_ms,
                segments: vec![],
            }],
            ..TransportTemplate::default()
        }
    }

    #[test]
    fn transport_constructor_matches_cpp_base_state() {
        let transport = Transport::new();

        assert_eq!(transport.world().object().type_id(), TypeId::GameObject);
        assert_eq!(
            transport.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::GAME_OBJECT
        );
        assert!(!transport.world().is_world_object());
        assert!(
            transport
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::SERVER_TIME)
        );
        assert!(
            transport
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::STATIONARY)
        );
        assert!(
            transport
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::ROTATION)
        );
        assert_eq!(transport.movement_state(), TransportMovementState::Moving);
        assert_eq!(transport.current_path_leg(), 0);
        assert_eq!(transport.request_stop_timestamp_ms(), None);
        assert_eq!(transport.path_progress_ms(), 0);
        assert_eq!(transport.get_timer(), 0);
        assert_eq!(transport.position_change_timer_ms(), 0);
        assert!(transport.allow_stopping());
        assert!(transport.passengers().is_empty());
        assert!(transport.static_passengers().is_empty());
        assert!(!transport.delayed_add_model());
    }

    #[test]
    fn initialize_created_state_sets_cpp_transport_gameobject_fields() {
        let mut transport = Transport::new();
        transport.world_mut().object_mut().create(transport_guid());

        transport.initialize_created_state(TransportCreateInfo {
            entry: 42,
            display_id: 1000,
            object_scale: 1.5,
            name: "Deeprun Tram".to_string(),
            period_ms: 120_000,
            path_progress_ms: 30_000,
            allow_stopping: false,
        });

        assert_eq!(transport.world().object().guid(), transport_guid());
        assert_eq!(transport.world().object().entry(), 42);
        assert_eq!(transport.world().object().scale(), 1.5);
        assert_eq!(transport.game_object().data().display_id, 1000);
        assert_eq!(transport.game_object().data().type_id, 15);
        assert_eq!(transport.game_object().data().level, 120_000);
        assert_eq!(transport.game_object().data().state, GoState::Ready as i8);
        assert!(!transport.allow_stopping());
        assert_eq!(transport.world().name(), "Deeprun Tram");
        assert_eq!(transport.path_progress_ms(), 30_000);
        assert_eq!(
            transport.world().object().dynamic_flags() >> 16,
            (0.25 * 65_535.0) as u32
        );
    }

    #[test]
    fn passenger_sets_follow_cpp_dynamic_and_static_shape() {
        let mut transport = Transport::new();
        assert!(!transport.add_passenger(passenger_guid(1)));

        transport.world_mut().object_mut().add_to_world();
        assert!(transport.add_passenger(passenger_guid(1)));
        assert!(!transport.add_passenger(passenger_guid(1)));
        assert!(transport.add_static_passenger(passenger_guid(2)));
        assert_eq!(
            transport.remove_passenger(passenger_guid(1)),
            Some(TransportPassengerSet::Dynamic)
        );
        assert_eq!(
            transport.remove_passenger(passenger_guid(2)),
            Some(TransportPassengerSet::Static)
        );
        assert_eq!(transport.remove_passenger(passenger_guid(3)), None);

        transport.add_passenger(passenger_guid(4));
        transport.add_static_passenger(passenger_guid(5));
        let removed = transport.cleanup_before_delete();
        assert_eq!(removed.len(), 2);
        assert!(transport.passengers().is_empty());
        assert!(transport.static_passengers().is_empty());
    }

    #[test]
    fn template_state_tracks_events_period_and_expected_map() {
        let template = TransportTemplate {
            total_path_time_ms: 60_000,
            path_legs: vec![
                TransportPathLeg {
                    map_id: 0,
                    start_timestamp_ms: 0,
                    duration_ms: 20_000,
                    segments: vec![],
                },
                TransportPathLeg {
                    map_id: 571,
                    start_timestamp_ms: 20_000,
                    duration_ms: 40_000,
                    segments: vec![],
                },
            ],
            events: vec![TransportPathEvent {
                timestamp_ms: 10_000,
                event_id: 7,
            }],
            ..TransportTemplate::default()
        };
        let mut transport = Transport::with_template(template);

        assert_eq!(transport.get_transport_period(), 60_000);
        assert_eq!(transport.get_expected_map_id(), Some(0));
        transport.set_current_path_leg(1);
        assert_eq!(transport.get_expected_map_id(), Some(571));
        assert_eq!(transport.events_to_trigger(), &[true]);
        assert!(transport.mark_event_triggered(0));
        assert_eq!(transport.events_to_trigger(), &[false]);
    }

    #[test]
    fn movement_stop_request_matches_cpp_period_math() {
        let mut transport = Transport::new();
        transport.set_period(1000);
        transport.set_path_progress_ms(2500);
        transport.request_stop_at_next_pause(750);
        assert_eq!(transport.request_stop_timestamp_ms(), Some(2750));

        transport
            .world_mut()
            .object_mut()
            .set_dynamic_flag(GO_DYNFLAG_LO_STOPPED);
        transport.enable_movement();
        assert_eq!(transport.request_stop_timestamp_ms(), None);
        assert_eq!(transport.game_object().data().state, GoState::Active as i8);
        assert!(
            !transport
                .world()
                .object()
                .has_dynamic_flag(GO_DYNFLAG_LO_STOPPED)
        );
    }

    #[test]
    fn transport_update_initial_expired_position_timer_runs_first_tick_like_cpp() {
        let mut transport = Transport::with_template(transport_template_for_map(571, 1_000));
        transport.set_path_progress_ms(100);

        let first_update = transport.update_like_cpp(1, 10_000, 571);

        assert_eq!(first_update.old_path_progress_ms, 100);
        assert_eq!(first_update.new_path_progress_ms, 101);
        assert_eq!(first_update.timer_ms, Some(101));
        assert_eq!(first_update.position_update_timer_before_ms, 0);
        assert!(first_update.position_update_timer_passed);
        assert_eq!(first_update.position_update_timer_after_ms, 200);
        assert!(first_update.position_update_due);
        assert!(first_update.position_update_represented);
        assert_eq!(transport.position_change_timer_ms(), 200);

        let second_update = transport.update_like_cpp(1, 10_001, 571);

        assert_eq!(second_update.new_path_progress_ms, 102);
        assert_eq!(second_update.position_update_timer_before_ms, 200);
        assert!(!second_update.position_update_timer_passed);
        assert_eq!(second_update.position_update_timer_after_ms, 199);
        assert!(!second_update.position_update_due);
        assert_eq!(transport.position_change_timer_ms(), 199);
    }

    #[test]
    fn transport_update_allow_stopping_counts_down_timer_like_cpp() {
        let mut transport = Transport::new();
        transport.set_period(1_000);
        transport.set_path_progress_ms(100);
        transport.set_position_change_timer_ms(150);

        let update = transport.update_like_cpp(75, 10_000, 571);

        assert_eq!(update.old_path_progress_ms, 100);
        assert_eq!(update.new_path_progress_ms, 175);
        assert_eq!(update.timer_ms, Some(175));
        assert_eq!(transport.path_progress_ms(), 175);
        assert_eq!(update.position_update_timer_before_ms, 150);
        assert!(!update.position_update_timer_passed);
        assert_eq!(update.position_update_timer_after_ms, 75);
        assert_eq!(transport.position_change_timer_ms(), 75);
        assert!(!update.position_update_due);
        assert!(!update.unsupported_no_period);
    }

    #[test]
    fn transport_update_stop_request_inside_tick_marks_just_stopped_like_cpp() {
        let mut transport = Transport::new();
        transport.set_period(1_000);
        transport.set_path_progress_ms(900);
        transport.request_stop_at_next_pause(950);

        let update = transport.update_like_cpp(100, 10_000, 571);

        assert_eq!(update.new_path_progress_ms, 950);
        assert!(update.just_stopped);
        assert!(update.stopped_dynflag_represented);
        assert_eq!(
            update.movement_state_after,
            TransportMovementState::WaitingOnPauseWaypoint
        );
        assert_eq!(transport.game_object().data().state, GoState::Ready as i8);
        assert!(
            transport
                .world()
                .object()
                .has_dynamic_flag(GO_DYNFLAG_LO_STOPPED)
        );
    }

    #[test]
    fn transport_update_non_allow_stopping_uses_now_ms_like_cpp() {
        let mut transport = Transport::new();
        transport.set_period(1_000);
        transport.set_path_progress_ms(200);
        transport.set_allow_stopping(false);

        let update = transport.update_like_cpp(50, 12_345, 571);

        assert_eq!(update.new_path_progress_ms, 12_345);
        assert_eq!(update.timer_ms, Some(345));
        assert!(!update.allow_stopping);
        assert_eq!(transport.path_progress_ms(), 12_345);
    }

    #[test]
    fn transport_update_period_zero_is_safe_noop_like_cpp_unsupported() {
        let mut transport = Transport::new();
        transport.set_period(0);
        transport.set_path_progress_ms(200);
        transport.set_position_change_timer_ms(150);

        let update = transport.update_like_cpp(100, 12_345, 571);

        assert!(update.unsupported_no_period);
        assert_eq!(update.new_path_progress_ms, 200);
        assert_eq!(update.timer_ms, None);
        assert_eq!(transport.path_progress_ms(), 200);
        assert_eq!(transport.position_change_timer_ms(), 150);
        assert!(!update.path_progress_for_client_updated);
    }

    #[test]
    fn transport_position_transforms_match_cpp_formula() {
        let mut transport = Transport::new();
        transport.update_position(Position::new(10.0, 20.0, 30.0, 1.0));
        let offset = Position::new(2.0, 3.0, 4.0, 0.5);

        let global = transport.calculate_passenger_position(offset);
        let roundtrip = transport.calculate_passenger_offset(global);

        assert!((roundtrip.x - offset.x).abs() < 0.0001);
        assert!((roundtrip.y - offset.y).abs() < 0.0001);
        assert!((roundtrip.z - offset.z).abs() < 0.0001);
        assert!((roundtrip.orientation - offset.orientation).abs() < 0.0001);
    }
}
