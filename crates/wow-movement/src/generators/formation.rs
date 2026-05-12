use wow_core::{ObjectGuid, Position};

use crate::{
    AbstractFollower, MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode,
    MovementGeneratorPriority, MovementGeneratorState, MovementGeneratorType,
    normalize_orientation_like_cpp,
};

pub const FORMATION_MOVEMENT_INTERVAL_MS_LIKE_CPP: i32 = 1_200;
pub const FORMATION_PREDICTED_SPLINE_SECONDS_LIKE_CPP: f32 = 1.65;
pub const UNIT_STATE_FOLLOW_FORMATION_LIKE_CPP: u32 = 0x0008_0000;
pub const UNIT_STATE_FOLLOW_FORMATION_MOVE_LIKE_CPP: u32 = 0x2000_0000;
pub const UNIT_STATE_FORMATION_NOT_MOVE_LIKE_CPP: u32 = 0x0000_0409;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationUnitSnapshot {
    pub owner_alive: bool,
    pub owner_unit_state: u32,
    pub owner_position: Position,
    pub owner_movespline_finalized: bool,
    pub owner_has_formation_move: bool,
    pub movement_prevented_by_casting: bool,
    pub target_exists: bool,
    pub target_position: Position,
    pub target_orientation: f32,
    pub target_movespline_finalized: bool,
    pub target_spline_id: u32,
    pub target_spline_destination: Position,
    pub target_spline_velocity: f32,
    pub target_walk_speed: f32,
    pub target_is_creature: bool,
    pub formation_leader_current_waypoint: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationLaunchPlan {
    pub destination: Position,
    pub velocity: f32,
    pub add_unit_state: u32,
    pub predicted_destination: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationMovementInform {
    pub movement_type: MovementGeneratorType,
    pub point_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationArrivalAction {
    pub clear_formation_move: bool,
    pub facing: f32,
    pub inform: FormationMovementInform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormationMovementAction {
    Continue,
    Finished,
    StopMoving,
    Arrived(FormationArrivalAction),
    Launch(FormationLaunchPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormationFinalizeAction {
    pub clear_formation_move: bool,
    pub inform: Option<FormationMovementInform>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormationMovementGenerator {
    state: MovementGeneratorState,
    follower: AbstractFollower,
    range: f32,
    angle: f32,
    point1: u32,
    point2: u32,
    last_leader_spline_id: u32,
    has_predicted_destination: bool,
    last_leader_position: Position,
    next_move_timer_ms: i32,
    pub stop_moving_calls: u32,
    pub last_launch: Option<FormationLaunchPlan>,
    pub finalize_action: Option<FormationFinalizeAction>,
}

impl FormationMovementGenerator {
    #[must_use]
    pub fn new(leader: ObjectGuid, range: f32, angle: f32, point1: u32, point2: u32) -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_FOLLOW_FORMATION_LIKE_CPP,
            },
            follower: AbstractFollower::new(leader),
            range,
            angle,
            point1,
            point2,
            last_leader_spline_id: 0,
            has_predicted_destination: false,
            last_leader_position: Position::new(0.0, 0.0, 0.0, 0.0),
            next_move_timer_ms: 0,
            stop_moving_calls: 0,
            last_launch: None,
            finalize_action: None,
        }
    }

    #[must_use]
    pub const fn leader(&self) -> Option<ObjectGuid> {
        self.follower.target()
    }

    #[must_use]
    pub const fn angle(&self) -> f32 {
        self.angle
    }

    #[must_use]
    pub const fn last_leader_spline_id(&self) -> u32 {
        self.last_leader_spline_id
    }

    #[must_use]
    pub const fn has_predicted_destination(&self) -> bool {
        self.has_predicted_destination
    }

    #[must_use]
    pub const fn next_move_timer_ms(&self) -> i32 {
        self.next_move_timer_ms
    }

    pub fn initialize_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: FormationUnitSnapshot,
    ) -> FormationMovementAction {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING
                | MovementGeneratorFlags::TRANSITORY
                | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);

        if !owner_exists || !snapshot.owner_alive {
            return FormationMovementAction::Finished;
        }

        if snapshot.owner_unit_state & UNIT_STATE_FORMATION_NOT_MOVE_LIKE_CPP != 0
            || snapshot.movement_prevented_by_casting
        {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
            return FormationMovementAction::StopMoving;
        }

        self.next_move_timer_ms = 0;
        FormationMovementAction::Continue
    }

    pub fn reset_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: FormationUnitSnapshot,
    ) -> FormationMovementAction {
        self.remove_flag(MovementGeneratorFlags::TRANSITORY | MovementGeneratorFlags::DEACTIVATED);
        self.initialize_like_cpp(owner_exists, snapshot)
    }

    pub fn update_like_cpp(
        &mut self,
        owner_exists: bool,
        diff_ms: u32,
        snapshot: FormationUnitSnapshot,
    ) -> FormationMovementAction {
        if !owner_exists || !snapshot.target_exists {
            return FormationMovementAction::Finished;
        }

        if snapshot.owner_unit_state & UNIT_STATE_FORMATION_NOT_MOVE_LIKE_CPP != 0
            || snapshot.movement_prevented_by_casting
        {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
            self.next_move_timer_ms = 0;
            self.has_predicted_destination = false;
            return FormationMovementAction::StopMoving;
        }

        if snapshot.target_movespline_finalized
            && snapshot.target_spline_id == self.last_leader_spline_id
            && self.has_predicted_destination
        {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
            self.next_move_timer_ms = 0;
            self.has_predicted_destination = false;
            return FormationMovementAction::StopMoving;
        }

        if !snapshot.target_movespline_finalized
            && snapshot.target_spline_id != self.last_leader_spline_id
        {
            self.maybe_flip_angle_like_cpp(snapshot);
            let launch = self.launch_movement_like_cpp(snapshot);
            self.last_leader_spline_id = snapshot.target_spline_id;
            return FormationMovementAction::Launch(launch);
        }

        self.next_move_timer_ms = self.next_move_timer_ms.saturating_sub(diff_ms as i32);
        if self.next_move_timer_ms <= 0 {
            self.next_move_timer_ms = FORMATION_MOVEMENT_INTERVAL_MS_LIKE_CPP;
            if self.last_leader_position != snapshot.target_position {
                let launch = self.launch_movement_like_cpp(snapshot);
                return FormationMovementAction::Launch(launch);
            }
        }

        if snapshot.owner_has_formation_move && snapshot.owner_movespline_finalized {
            return FormationMovementAction::Arrived(FormationArrivalAction {
                clear_formation_move: true,
                facing: snapshot.target_orientation,
                inform: self.inform(),
            });
        }

        FormationMovementAction::Continue
    }

    pub fn launch_movement_like_cpp(
        &mut self,
        snapshot: FormationUnitSnapshot,
    ) -> FormationLaunchPlan {
        let mut relative_angle = 0.0;
        if !snapshot.target_movespline_finalized {
            relative_angle = relative_angle_like_cpp(
                snapshot.target_position,
                snapshot.target_spline_destination,
            );
        }

        let mut destination = snapshot.target_position;
        let mut velocity = 0.0;
        if !snapshot.target_movespline_finalized {
            velocity = snapshot.target_spline_velocity;
            let travel_distance = velocity * FORMATION_PREDICTED_SPLINE_SECONDS_LIKE_CPP;
            destination = move_position_like_cpp(destination, travel_distance, relative_angle);
            destination =
                move_position_like_cpp(destination, self.range, self.angle + relative_angle);
            let distance = distance_like_cpp(snapshot.owner_position, destination);
            let velocity_mod = (distance / travel_distance.max(f32::EPSILON)).min(1.5);
            velocity *= velocity_mod;
            self.has_predicted_destination = true;
        } else {
            destination =
                move_position_like_cpp(destination, self.range, self.angle + relative_angle);
            self.has_predicted_destination = false;
        }

        if velocity == 0.0 {
            velocity = snapshot.target_walk_speed;
        }

        self.last_leader_position = snapshot.target_position;
        self.remove_flag(MovementGeneratorFlags::INTERRUPTED);
        let launch = FormationLaunchPlan {
            destination,
            velocity,
            add_unit_state: UNIT_STATE_FOLLOW_FORMATION_MOVE_LIKE_CPP,
            predicted_destination: self.has_predicted_destination,
        };
        self.last_launch = Some(launch);
        launch
    }

    pub fn deactivate_like_cpp(&mut self) -> FormationFinalizeAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        FormationFinalizeAction {
            clear_formation_move: true,
            inform: None,
        }
    }

    pub fn finalize_like_cpp(
        &mut self,
        active: bool,
        movement_inform: bool,
    ) -> FormationFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let action = FormationFinalizeAction {
            clear_formation_move: active,
            inform: (movement_inform && self.has_flag(MovementGeneratorFlags::INFORM_ENABLED))
                .then_some(self.inform()),
        };
        self.finalize_action = Some(action);
        action
    }

    fn maybe_flip_angle_like_cpp(&mut self, snapshot: FormationUnitSnapshot) {
        if self.point1 == 0 || !snapshot.target_is_creature {
            return;
        }
        let Some(current_waypoint) = snapshot.formation_leader_current_waypoint else {
            return;
        };
        if u32::from(current_waypoint) == self.point1 || u32::from(current_waypoint) == self.point2
        {
            self.angle = std::f32::consts::PI * 2.0 - self.angle;
        }
    }

    const fn inform(&self) -> FormationMovementInform {
        FormationMovementInform {
            movement_type: MovementGeneratorType::Formation,
            point_id: 0,
        }
    }
}

impl MovementGenerator for FormationMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Formation
    }

    fn initialize(&mut self) {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING
                | MovementGeneratorFlags::TRANSITORY
                | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);
    }

    fn reset(&mut self) {
        self.remove_flag(MovementGeneratorFlags::TRANSITORY | MovementGeneratorFlags::DEACTIVATED);
        self.initialize();
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        true
    }

    fn deactivate(&mut self) {
        self.deactivate_like_cpp();
    }

    fn finalize(&mut self, active: bool, movement_inform: bool) {
        self.finalize_like_cpp(active, movement_inform);
    }
}

#[must_use]
pub fn move_position_like_cpp(position: Position, distance: f32, angle: f32) -> Position {
    let angle = normalize_orientation_like_cpp(angle);
    Position::new(
        position.x + distance * angle.cos(),
        position.y + distance * angle.sin(),
        position.z,
        position.orientation,
    )
}

fn relative_angle_like_cpp(from: Position, to: Position) -> f32 {
    normalize_orientation_like_cpp((to.y - from.y).atan2(to.x - from.x) - from.orientation)
}

fn distance_like_cpp(left: Position, right: Position) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guid(counter: i64) -> ObjectGuid {
        ObjectGuid::create_uniq(counter)
    }

    fn snapshot() -> FormationUnitSnapshot {
        FormationUnitSnapshot {
            owner_alive: true,
            owner_unit_state: 0,
            owner_position: Position::new(0.0, 0.0, 0.0, 0.0),
            owner_movespline_finalized: true,
            owner_has_formation_move: false,
            movement_prevented_by_casting: false,
            target_exists: true,
            target_position: Position::new(10.0, 0.0, 0.0, 0.0),
            target_orientation: 0.0,
            target_movespline_finalized: true,
            target_spline_id: 1,
            target_spline_destination: Position::new(20.0, 0.0, 0.0, 0.0),
            target_spline_velocity: 4.0,
            target_walk_speed: 2.5,
            target_is_creature: true,
            formation_leader_current_waypoint: None,
        }
    }

    #[test]
    fn formation_constructor_and_initialize_match_cpp_shape() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 11, 22);
        assert_eq!(formation.kind(), MovementGeneratorType::Formation);
        assert_eq!(formation.leader(), Some(guid(7)));
        assert_eq!(formation.state().mode, MovementGeneratorMode::Default);
        assert_eq!(
            formation.state().priority,
            MovementGeneratorPriority::Normal
        );
        assert_eq!(
            formation.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(
            formation.state().base_unit_state,
            UNIT_STATE_FOLLOW_FORMATION_LIKE_CPP
        );

        assert_eq!(
            formation.initialize_like_cpp(true, snapshot()),
            FormationMovementAction::Continue
        );
        assert!(formation.has_flag(MovementGeneratorFlags::INITIALIZED));
        assert!(!formation.has_flag(MovementGeneratorFlags::INITIALIZATION_PENDING));
        assert_eq!(formation.next_move_timer_ms(), 0);
    }

    #[test]
    fn formation_initialize_and_update_stop_when_owner_cannot_move() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 0, 0);
        let mut snap = snapshot();
        snap.owner_unit_state = UNIT_STATE_FORMATION_NOT_MOVE_LIKE_CPP;
        assert_eq!(
            formation.initialize_like_cpp(true, snap),
            FormationMovementAction::StopMoving
        );
        assert!(formation.has_flag(MovementGeneratorFlags::INTERRUPTED));
        assert_eq!(formation.stop_moving_calls, 1);

        snap.owner_unit_state = 0;
        formation.has_predicted_destination = true;
        formation.last_leader_spline_id = snap.target_spline_id;
        assert_eq!(
            formation.update_like_cpp(true, 1, snap),
            FormationMovementAction::StopMoving
        );
        assert_eq!(formation.stop_moving_calls, 2);
        assert!(!formation.has_predicted_destination());
    }

    #[test]
    fn formation_launch_stationary_leader_uses_shape_and_walk_speed() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 0, 0);
        let launch = formation.launch_movement_like_cpp(snapshot());
        assert_eq!(
            launch,
            FormationLaunchPlan {
                destination: Position::new(15.0, 0.0, 0.0, 0.0),
                velocity: 2.5,
                add_unit_state: UNIT_STATE_FOLLOW_FORMATION_MOVE_LIKE_CPP,
                predicted_destination: false,
            }
        );
        assert_eq!(formation.last_leader_position, snapshot().target_position);
    }

    #[test]
    fn formation_launch_moving_leader_predicts_destination_and_velocity_mod() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 0, 0);
        let mut snap = snapshot();
        snap.target_movespline_finalized = false;
        snap.target_spline_velocity = 4.0;
        snap.target_spline_destination = Position::new(20.0, 0.0, 0.0, 0.0);
        let launch = formation.launch_movement_like_cpp(snap);
        assert!(launch.predicted_destination);
        assert_eq!(launch.destination, Position::new(21.6, 0.0, 0.0, 0.0));
        assert_eq!(launch.velocity, 4.0 * 1.5);
        assert!(formation.has_predicted_destination());
    }

    #[test]
    fn formation_update_launches_on_new_leader_spline_and_flips_angle_at_configured_points() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 1.0, 11, 22);
        formation.initialize_like_cpp(true, snapshot());
        let mut snap = snapshot();
        snap.target_movespline_finalized = false;
        snap.target_spline_id = 9;
        snap.formation_leader_current_waypoint = Some(11);
        let action = formation.update_like_cpp(true, 1, snap);
        assert!(matches!(action, FormationMovementAction::Launch(_)));
        assert_eq!(formation.last_leader_spline_id(), 9);
        assert!((formation.angle() - (std::f32::consts::PI * 2.0 - 1.0)).abs() < 0.0001);
    }

    #[test]
    fn formation_periodic_launch_and_arrival_inform_match_cpp() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 0, 0);
        formation.initialize_like_cpp(true, snapshot());
        formation.last_leader_position = Position::new(0.0, 0.0, 0.0, 0.0);
        let action = formation.update_like_cpp(
            true,
            FORMATION_MOVEMENT_INTERVAL_MS_LIKE_CPP as u32,
            snapshot(),
        );
        assert!(matches!(action, FormationMovementAction::Launch(_)));

        let mut snap = snapshot();
        snap.owner_has_formation_move = true;
        snap.owner_movespline_finalized = true;
        formation.last_leader_position = snap.target_position;
        assert_eq!(
            formation.update_like_cpp(true, 1, snap),
            FormationMovementAction::Arrived(FormationArrivalAction {
                clear_formation_move: true,
                facing: 0.0,
                inform: FormationMovementInform {
                    movement_type: MovementGeneratorType::Formation,
                    point_id: 0,
                },
            })
        );
    }

    #[test]
    fn formation_deactivate_and_finalize_match_cpp() {
        let mut formation = FormationMovementGenerator::new(guid(7), 5.0, 0.0, 0, 0);
        assert_eq!(
            formation.deactivate_like_cpp(),
            FormationFinalizeAction {
                clear_formation_move: true,
                inform: None,
            }
        );

        formation.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
        assert_eq!(
            formation.finalize_like_cpp(true, true),
            FormationFinalizeAction {
                clear_formation_move: true,
                inform: Some(FormationMovementInform {
                    movement_type: MovementGeneratorType::Formation,
                    point_id: 0,
                }),
            }
        );
    }
}
