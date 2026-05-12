use wow_core::Position;

use crate::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType, normalize_orientation_like_cpp,
};

pub const CONFUSED_PATH_LENGTH_LIMIT_LIKE_CPP: f32 = 30.0;
pub const CONFUSED_LOS_RETRY_MS_LIKE_CPP: i32 = 200;
pub const CONFUSED_PATH_RETRY_MS_LIKE_CPP: i32 = 100;
pub const CONFUSED_RANDOM_DELAY_MIN_MS_LIKE_CPP: i32 = 800;
pub const CONFUSED_RANDOM_DELAY_MAX_MS_LIKE_CPP: i32 = 1500;
pub const CONFUSED_RANDOM_DISTANCE_SCALE_LIKE_CPP: f32 = 4.0;
pub const CONFUSED_RANDOM_DISTANCE_OFFSET_LIKE_CPP: f32 = 2.0;
pub const UNIT_STATE_CONFUSED_LIKE_CPP: u32 = 0x0000_0800;
pub const UNIT_STATE_CONFUSED_MOVE_LIKE_CPP: u32 = 0x0100_0000;
pub const UNIT_FLAG_CONFUSED_LIKE_CPP: u32 = 0x0040_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfusedUnitSnapshot {
    pub owner_position: Position,
    pub owner_alive: bool,
    pub can_move: bool,
    pub movement_prevented_by_casting: bool,
    pub move_spline_finalized: bool,
    pub has_los_to_destination: bool,
    pub path_result: ConfusedPathResult,
    pub travel_time_ms: i32,
    pub random_delay_ms: i32,
    pub random_distance_roll: f32,
    pub random_angle_roll: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfusedPathResult {
    Success,
    Failed,
    NoPath,
    Shortcut,
    FarFromPoly,
}

impl ConfusedPathResult {
    #[must_use]
    pub const fn is_usable_like_cpp(self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfusedDestinationPlan {
    pub reference: Position,
    pub distance: f32,
    pub angle: f32,
    pub destination: Position,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfusedLaunchPlan {
    pub destination: Position,
    pub path_length_limit: f32,
    pub walk: bool,
    pub timer_ms: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfusedMovementAction {
    Continue,
    Finished,
    StopMoving,
    RetryAfterLosFailure {
        timer_ms: i32,
    },
    RetryAfterPathFailure {
        timer_ms: i32,
        result: ConfusedPathResult,
    },
    Launch(ConfusedLaunchPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfusedFinalizeAction {
    pub remove_confused_flag: bool,
    pub clear_confused_move: bool,
    pub stop_moving: bool,
    pub set_target_to_victim: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfusedMovementGenerator {
    state: MovementGeneratorState,
    timer_ms: i32,
    reference: Position,
    path_allocated: bool,
    last_destination_plan: Option<ConfusedDestinationPlan>,
    pub stop_moving_calls: u32,
    pub set_confused_flag_calls: u32,
    pub remove_confused_flag_calls: u32,
    pub finalize_action: Option<ConfusedFinalizeAction>,
}

impl ConfusedMovementGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Highest,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_CONFUSED_LIKE_CPP,
            },
            timer_ms: 0,
            reference: Position::new(0.0, 0.0, 0.0, 0.0),
            path_allocated: false,
            last_destination_plan: None,
            stop_moving_calls: 0,
            set_confused_flag_calls: 0,
            remove_confused_flag_calls: 0,
            finalize_action: None,
        }
    }

    #[must_use]
    pub const fn timer_ms(&self) -> i32 {
        self.timer_ms
    }

    #[must_use]
    pub const fn reference(&self) -> Position {
        self.reference
    }

    #[must_use]
    pub const fn path_allocated(&self) -> bool {
        self.path_allocated
    }

    #[must_use]
    pub const fn last_destination_plan(&self) -> Option<ConfusedDestinationPlan> {
        self.last_destination_plan
    }

    pub fn initialize_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: ConfusedUnitSnapshot,
    ) -> ConfusedMovementAction {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING
                | MovementGeneratorFlags::TRANSITORY
                | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);

        if !owner_exists || !snapshot.owner_alive {
            return ConfusedMovementAction::Continue;
        }

        self.set_confused_flag_calls = self.set_confused_flag_calls.saturating_add(1);
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
        self.timer_ms = 0;
        self.reference = Position::new(
            snapshot.owner_position.x,
            snapshot.owner_position.y,
            snapshot.owner_position.z,
            0.0,
        );
        self.path_allocated = false;
        ConfusedMovementAction::StopMoving
    }

    pub fn reset_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: ConfusedUnitSnapshot,
    ) -> ConfusedMovementAction {
        self.remove_flag(MovementGeneratorFlags::TRANSITORY | MovementGeneratorFlags::DEACTIVATED);
        self.initialize_like_cpp(owner_exists, snapshot)
    }

    pub fn update_like_cpp(
        &mut self,
        owner_exists: bool,
        diff_ms: u32,
        snapshot: ConfusedUnitSnapshot,
    ) -> ConfusedMovementAction {
        if !owner_exists || !snapshot.owner_alive {
            return ConfusedMovementAction::Finished;
        }

        if !snapshot.can_move || snapshot.movement_prevented_by_casting {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
            self.path_allocated = false;
            return ConfusedMovementAction::StopMoving;
        }

        self.remove_flag(MovementGeneratorFlags::INTERRUPTED);
        self.timer_ms = self.timer_ms.saturating_sub(diff_ms as i32);
        let speed_update_pending = self.has_flag(MovementGeneratorFlags::SPEED_UPDATE_PENDING);
        if (speed_update_pending && !snapshot.move_spline_finalized)
            || (self.timer_ms <= 0 && snapshot.move_spline_finalized)
        {
            self.remove_flag(MovementGeneratorFlags::TRANSITORY);
            return self.launch_next_move_like_cpp(snapshot);
        }

        ConfusedMovementAction::Continue
    }

    pub fn launch_next_move_like_cpp(
        &mut self,
        snapshot: ConfusedUnitSnapshot,
    ) -> ConfusedMovementAction {
        let destination_plan = compute_confused_destination_like_cpp(
            self.reference,
            snapshot.random_distance_roll,
            snapshot.random_angle_roll,
        );
        self.last_destination_plan = Some(destination_plan);

        if !snapshot.has_los_to_destination {
            self.timer_ms = CONFUSED_LOS_RETRY_MS_LIKE_CPP;
            return ConfusedMovementAction::RetryAfterLosFailure {
                timer_ms: self.timer_ms,
            };
        }

        self.path_allocated = true;
        if !snapshot.path_result.is_usable_like_cpp() {
            self.timer_ms = CONFUSED_PATH_RETRY_MS_LIKE_CPP;
            return ConfusedMovementAction::RetryAfterPathFailure {
                timer_ms: self.timer_ms,
                result: snapshot.path_result,
            };
        }

        self.timer_ms = snapshot.travel_time_ms + snapshot.random_delay_ms;
        ConfusedMovementAction::Launch(ConfusedLaunchPlan {
            destination: destination_plan.destination,
            path_length_limit: CONFUSED_PATH_LENGTH_LIMIT_LIKE_CPP,
            walk: true,
            timer_ms: self.timer_ms,
        })
    }

    pub fn deactivate_like_cpp(&mut self) -> ConfusedFinalizeAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        ConfusedFinalizeAction {
            remove_confused_flag: false,
            clear_confused_move: true,
            stop_moving: false,
            set_target_to_victim: false,
        }
    }

    pub fn finalize_player_like_cpp(&mut self, active: bool) -> ConfusedFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let action = ConfusedFinalizeAction {
            remove_confused_flag: active,
            clear_confused_move: false,
            stop_moving: active,
            set_target_to_victim: false,
        };
        if active {
            self.remove_confused_flag_calls = self.remove_confused_flag_calls.saturating_add(1);
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
        }
        self.finalize_action = Some(action);
        action
    }

    pub fn finalize_creature_like_cpp(
        &mut self,
        active: bool,
        has_victim: bool,
    ) -> ConfusedFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let action = ConfusedFinalizeAction {
            remove_confused_flag: active,
            clear_confused_move: active,
            stop_moving: false,
            set_target_to_victim: active && has_victim,
        };
        if active {
            self.remove_confused_flag_calls = self.remove_confused_flag_calls.saturating_add(1);
        }
        self.finalize_action = Some(action);
        action
    }
}

impl Default for ConfusedMovementGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl MovementGenerator for ConfusedMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Confused
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

    fn finalize(&mut self, active: bool, _movement_inform: bool) {
        self.finalize_player_like_cpp(active);
    }

    fn unit_speed_changed(&mut self) {
        self.add_flag(MovementGeneratorFlags::SPEED_UPDATE_PENDING);
    }
}

#[must_use]
pub fn compute_confused_destination_like_cpp(
    reference: Position,
    random_distance_roll: f32,
    random_angle_roll: f32,
) -> ConfusedDestinationPlan {
    let distance = CONFUSED_RANDOM_DISTANCE_SCALE_LIKE_CPP * random_distance_roll
        - CONFUSED_RANDOM_DISTANCE_OFFSET_LIKE_CPP;
    let angle = normalize_orientation_like_cpp(random_angle_roll * std::f32::consts::PI * 2.0);
    let destination = Position::new(
        reference.x + distance * angle.cos(),
        reference.y + distance * angle.sin(),
        reference.z,
        reference.orientation,
    );

    ConfusedDestinationPlan {
        reference,
        distance,
        angle,
        destination,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(owner: Position) -> ConfusedUnitSnapshot {
        ConfusedUnitSnapshot {
            owner_position: owner,
            owner_alive: true,
            can_move: true,
            movement_prevented_by_casting: false,
            move_spline_finalized: true,
            has_los_to_destination: true,
            path_result: ConfusedPathResult::Success,
            travel_time_ms: 300,
            random_delay_ms: CONFUSED_RANDOM_DELAY_MIN_MS_LIKE_CPP,
            random_distance_roll: 1.0,
            random_angle_roll: 0.0,
        }
    }

    #[test]
    fn confused_constructor_and_initialize_match_cpp_shape() {
        let owner = Position::new(10.0, 20.0, 30.0, 1.5);
        let mut confused = ConfusedMovementGenerator::new();

        assert_eq!(confused.kind(), MovementGeneratorType::Confused);
        assert_eq!(confused.state().mode, MovementGeneratorMode::Default);
        assert_eq!(
            confused.state().priority,
            MovementGeneratorPriority::Highest
        );
        assert_eq!(
            confused.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(
            confused.state().base_unit_state,
            UNIT_STATE_CONFUSED_LIKE_CPP
        );
        assert_eq!(UNIT_STATE_CONFUSED_MOVE_LIKE_CPP, 0x0100_0000);
        assert_eq!(UNIT_FLAG_CONFUSED_LIKE_CPP, 0x0040_0000);

        assert_eq!(
            confused.initialize_like_cpp(true, snapshot(owner)),
            ConfusedMovementAction::StopMoving
        );
        assert!(confused.has_flag(MovementGeneratorFlags::INITIALIZED));
        assert!(!confused.has_flag(MovementGeneratorFlags::INITIALIZATION_PENDING));
        assert_eq!(confused.set_confused_flag_calls, 1);
        assert_eq!(confused.stop_moving_calls, 1);
        assert_eq!(confused.reference(), Position::new(10.0, 20.0, 30.0, 0.0));
        assert!(!confused.path_allocated());
    }

    #[test]
    fn confused_destination_uses_cpp_random_distance_and_angle_shape() {
        let reference = Position::new(10.0, 20.0, 30.0, 0.0);

        let positive = compute_confused_destination_like_cpp(reference, 1.0, 0.0);
        assert_eq!(positive.distance, 2.0);
        assert_eq!(positive.angle, 0.0);
        assert_eq!(positive.destination.x, 12.0);
        assert_eq!(positive.destination.y, 20.0);

        let negative = compute_confused_destination_like_cpp(reference, 0.0, 0.25);
        assert_eq!(negative.distance, -2.0);
        assert!((negative.angle - std::f32::consts::FRAC_PI_2).abs() < f32::EPSILON);
        assert!((negative.destination.x - 10.0).abs() < 0.0001);
        assert!((negative.destination.y - 18.0).abs() < 0.0001);
    }

    #[test]
    fn confused_update_retries_los_and_path_failures_like_cpp() {
        let owner = Position::new(10.0, 20.0, 30.0, 0.0);
        let mut confused = ConfusedMovementGenerator::new();
        confused.initialize_like_cpp(true, snapshot(owner));

        let mut no_los = snapshot(owner);
        no_los.has_los_to_destination = false;
        assert_eq!(
            confused.update_like_cpp(true, 1, no_los),
            ConfusedMovementAction::RetryAfterLosFailure {
                timer_ms: CONFUSED_LOS_RETRY_MS_LIKE_CPP,
            }
        );
        assert_eq!(confused.timer_ms(), CONFUSED_LOS_RETRY_MS_LIKE_CPP);
        assert!(!confused.path_allocated());

        let mut no_path = snapshot(owner);
        no_path.path_result = ConfusedPathResult::Shortcut;
        assert_eq!(
            confused.update_like_cpp(true, 200, no_path),
            ConfusedMovementAction::RetryAfterPathFailure {
                timer_ms: CONFUSED_PATH_RETRY_MS_LIKE_CPP,
                result: ConfusedPathResult::Shortcut,
            }
        );
        assert_eq!(confused.timer_ms(), CONFUSED_PATH_RETRY_MS_LIKE_CPP);
        assert!(confused.path_allocated());
    }

    #[test]
    fn confused_update_blocks_relaunches_and_speed_updates_like_cpp() {
        let owner = Position::new(10.0, 20.0, 30.0, 0.0);
        let mut confused = ConfusedMovementGenerator::new();
        confused.initialize_like_cpp(true, snapshot(owner));

        let mut blocked = snapshot(owner);
        blocked.can_move = false;
        assert_eq!(
            confused.update_like_cpp(true, 1, blocked),
            ConfusedMovementAction::StopMoving
        );
        assert!(confused.has_flag(MovementGeneratorFlags::INTERRUPTED));
        assert_eq!(confused.stop_moving_calls, 2);
        assert!(!confused.path_allocated());

        assert!(matches!(
            confused.update_like_cpp(true, 1, snapshot(owner)),
            ConfusedMovementAction::Launch(_)
        ));
        assert!(!confused.has_flag(MovementGeneratorFlags::INTERRUPTED));

        confused.unit_speed_changed();
        let mut moving = snapshot(owner);
        moving.move_spline_finalized = false;
        assert!(matches!(
            confused.update_like_cpp(true, 1, moving),
            ConfusedMovementAction::Launch(_)
        ));
        assert!(!confused.has_flag(MovementGeneratorFlags::SPEED_UPDATE_PENDING));
    }

    #[test]
    fn confused_finalize_specializations_match_player_and_creature_cpp() {
        let mut player = ConfusedMovementGenerator::new();
        assert_eq!(
            player.finalize_player_like_cpp(true),
            ConfusedFinalizeAction {
                remove_confused_flag: true,
                clear_confused_move: false,
                stop_moving: true,
                set_target_to_victim: false,
            }
        );

        let mut creature = ConfusedMovementGenerator::new();
        assert_eq!(
            creature.finalize_creature_like_cpp(true, true),
            ConfusedFinalizeAction {
                remove_confused_flag: true,
                clear_confused_move: true,
                stop_moving: false,
                set_target_to_victim: true,
            }
        );
        assert_eq!(
            creature.deactivate_like_cpp(),
            ConfusedFinalizeAction {
                remove_confused_flag: false,
                clear_confused_move: true,
                stop_moving: false,
                set_target_to_victim: false,
            }
        );
    }

    #[test]
    fn confused_launch_plan_walks_and_uses_cpp_timer_bounds() {
        let owner = Position::new(0.0, 0.0, 0.0, 0.0);
        let mut confused = ConfusedMovementGenerator::new();
        confused.initialize_like_cpp(true, snapshot(owner));

        let action = confused.update_like_cpp(true, 1, snapshot(owner));
        assert_eq!(
            action,
            ConfusedMovementAction::Launch(ConfusedLaunchPlan {
                destination: Position::new(2.0, 0.0, 0.0, 0.0),
                path_length_limit: CONFUSED_PATH_LENGTH_LIMIT_LIKE_CPP,
                walk: true,
                timer_ms: 300 + CONFUSED_RANDOM_DELAY_MIN_MS_LIKE_CPP,
            })
        );
        assert_eq!(CONFUSED_RANDOM_DELAY_MIN_MS_LIKE_CPP, 800);
        assert_eq!(CONFUSED_RANDOM_DELAY_MAX_MS_LIKE_CPP, 1500);
    }
}
