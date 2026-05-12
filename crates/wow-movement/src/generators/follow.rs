use wow_core::{ObjectGuid, Position};

use crate::{
    ChaseAngle, MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode,
    MovementGeneratorPriority, MovementGeneratorState, MovementGeneratorType,
    normalize_orientation_like_cpp,
};

pub const FOLLOW_RANGE_TOLERANCE_LIKE_CPP: f32 = 1.0;
pub const FOLLOW_CHECK_INTERVAL_MS_LIKE_CPP: i32 = 100;
pub const UNIT_STATE_FOLLOW_LIKE_CPP: u32 = 0x0000_0200;
pub const UNIT_STATE_FOLLOW_MOVE_LIKE_CPP: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractFollowerEvent {
    FollowerRemoved(ObjectGuid),
    FollowerAdded(ObjectGuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AbstractFollower {
    target: Option<ObjectGuid>,
}

impl AbstractFollower {
    #[must_use]
    pub const fn new(target: ObjectGuid) -> Self {
        Self {
            target: Some(target),
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<ObjectGuid> {
        self.target
    }

    pub fn set_target_like_cpp(
        &mut self,
        target: Option<ObjectGuid>,
    ) -> Vec<AbstractFollowerEvent> {
        if self.target == target {
            return Vec::new();
        }

        let mut events = Vec::with_capacity(2);
        if let Some(previous) = self.target {
            events.push(AbstractFollowerEvent::FollowerRemoved(previous));
        }
        self.target = target;
        if let Some(next) = self.target {
            events.push(AbstractFollowerEvent::FollowerAdded(next));
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowUnitSnapshot {
    pub owner_position: Position,
    pub target_position: Position,
    pub owner_combat_reach: f32,
    pub target_combat_reach: f32,
    pub owner_alive: bool,
    pub target_in_world: bool,
    pub can_move: bool,
    pub movement_prevented_by_casting: bool,
    pub owner_has_follow_move: bool,
    pub owner_movespline_finalized: bool,
    pub target_is_walking: bool,
    pub owner_is_pet_of_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowLaunchPlan {
    pub desired_range: f32,
    pub desired_relative_angle: f32,
    pub target_is_walking: bool,
    pub allow_shortcut: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowMovementInform {
    pub movement_type: MovementGeneratorType,
    pub target_counter: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FollowMovementAction {
    Continue,
    Finished,
    StopMoving,
    StopMovingAndInform(FollowMovementInform),
    ClearFollowMoveAndInform(FollowMovementInform),
    Launch(FollowLaunchPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FollowFinalizeAction {
    pub clear_follow_move: bool,
    pub update_pet_speed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowMovementGenerator {
    state: MovementGeneratorState,
    follower: AbstractFollower,
    range: f32,
    angle: ChaseAngle,
    check_timer_ms: i32,
    duration_ms: Option<i32>,
    last_target_position: Option<Position>,
    pub stop_moving_calls: u32,
    pub pet_speed_update_calls: u32,
    pub finalize_action: Option<FollowFinalizeAction>,
}

impl FollowMovementGenerator {
    #[must_use]
    pub const fn new(
        target: ObjectGuid,
        range: f32,
        angle: ChaseAngle,
        duration_ms: Option<i32>,
    ) -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_FOLLOW_LIKE_CPP,
            },
            follower: AbstractFollower::new(target),
            range,
            angle,
            check_timer_ms: FOLLOW_CHECK_INTERVAL_MS_LIKE_CPP,
            duration_ms,
            last_target_position: None,
            stop_moving_calls: 0,
            pet_speed_update_calls: 0,
            finalize_action: None,
        }
    }

    #[must_use]
    pub const fn target(&self) -> Option<ObjectGuid> {
        self.follower.target()
    }

    #[must_use]
    pub const fn range(&self) -> f32 {
        self.range
    }

    #[must_use]
    pub const fn angle(&self) -> ChaseAngle {
        self.angle
    }

    #[must_use]
    pub const fn duration_ms(&self) -> Option<i32> {
        self.duration_ms
    }

    #[must_use]
    pub const fn check_timer_ms(&self) -> i32 {
        self.check_timer_ms
    }

    pub fn set_target_like_cpp(
        &mut self,
        target: Option<ObjectGuid>,
    ) -> Vec<AbstractFollowerEvent> {
        self.follower.set_target_like_cpp(target)
    }

    pub fn initialize_like_cpp(&mut self) {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED | MovementGeneratorFlags::INFORM_ENABLED);
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
        self.update_pet_speed_like_cpp();
        self.last_target_position = None;
    }

    pub fn reset_like_cpp(&mut self) {
        self.remove_flag(MovementGeneratorFlags::DEACTIVATED);
        self.initialize_like_cpp();
    }

    pub fn update_like_cpp(
        &mut self,
        owner_exists: bool,
        target_exists: bool,
        diff_ms: u32,
        snapshot: FollowUnitSnapshot,
    ) -> FollowMovementAction {
        if !owner_exists || !snapshot.owner_alive {
            return FollowMovementAction::Finished;
        }

        let Some(target) = self.target() else {
            return FollowMovementAction::Finished;
        };
        if !target_exists || !snapshot.target_in_world {
            return FollowMovementAction::Finished;
        }

        if let Some(duration_ms) = self.duration_ms.as_mut() {
            *duration_ms = duration_ms.saturating_sub(diff_ms as i32);
            if *duration_ms <= 0 {
                self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
                return FollowMovementAction::StopMovingAndInform(self.inform_for(target));
            }
        }

        if !snapshot.can_move || snapshot.movement_prevented_by_casting {
            self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
            self.last_target_position = None;
            return FollowMovementAction::StopMoving;
        }

        self.check_timer_ms = self.check_timer_ms.saturating_sub(diff_ms as i32);
        if self.check_timer_ms <= 0 {
            self.check_timer_ms = FOLLOW_CHECK_INTERVAL_MS_LIKE_CPP;
            if self.has_flag(MovementGeneratorFlags::INFORM_ENABLED)
                && position_okay_like_cpp(snapshot, self.range, Some(self.angle))
            {
                self.remove_flag(MovementGeneratorFlags::INFORM_ENABLED);
                self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
                self.last_target_position = None;
                return FollowMovementAction::StopMovingAndInform(self.inform_for(target));
            }
        }

        if snapshot.owner_has_follow_move && snapshot.owner_movespline_finalized {
            self.remove_flag(MovementGeneratorFlags::INFORM_ENABLED);
            self.last_target_position = None;
            return FollowMovementAction::ClearFollowMoveAndInform(self.inform_for(target));
        }

        if self
            .last_target_position
            .is_none_or(|position| position_distance_sq(position, snapshot.target_position) > 0.0)
        {
            self.last_target_position = Some(snapshot.target_position);
            if snapshot.owner_has_follow_move
                || !position_okay_like_cpp(
                    snapshot,
                    self.range + FOLLOW_RANGE_TOLERANCE_LIKE_CPP,
                    None,
                )
            {
                self.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
                return FollowMovementAction::Launch(FollowLaunchPlan {
                    desired_range: self.range,
                    desired_relative_angle: selected_relative_angle_like_cpp(
                        snapshot.owner_position,
                        snapshot.target_position,
                        self.angle,
                    ),
                    target_is_walking: snapshot.target_is_walking,
                    allow_shortcut: snapshot.owner_is_pet_of_target,
                });
            }
        }

        FollowMovementAction::Continue
    }

    pub fn deactivate_like_cpp(&mut self) -> FollowFinalizeAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        self.remove_flag(
            MovementGeneratorFlags::TRANSITORY | MovementGeneratorFlags::INFORM_ENABLED,
        );
        FollowFinalizeAction {
            clear_follow_move: true,
            update_pet_speed: false,
        }
    }

    pub fn finalize_like_cpp(&mut self, active: bool) -> FollowFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let action = FollowFinalizeAction {
            clear_follow_move: active,
            update_pet_speed: active,
        };
        if active {
            self.update_pet_speed_like_cpp();
        }
        self.finalize_action = Some(action);
        action
    }

    fn update_pet_speed_like_cpp(&mut self) {
        self.pet_speed_update_calls = self.pet_speed_update_calls.saturating_add(1);
    }

    fn inform_for(&self, target: ObjectGuid) -> FollowMovementInform {
        FollowMovementInform {
            movement_type: MovementGeneratorType::Follow,
            target_counter: target.counter() as u32,
        }
    }
}

impl MovementGenerator for FollowMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Follow
    }

    fn initialize(&mut self) {
        self.initialize_like_cpp();
    }

    fn reset(&mut self) {
        self.reset_like_cpp();
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        true
    }

    fn deactivate(&mut self) {
        self.deactivate_like_cpp();
    }

    fn finalize(&mut self, active: bool, _movement_inform: bool) {
        self.finalize_like_cpp(active);
    }

    fn unit_speed_changed(&mut self) {
        self.last_target_position = None;
    }
}

#[must_use]
pub fn position_okay_like_cpp(
    snapshot: FollowUnitSnapshot,
    range: f32,
    angle: Option<ChaseAngle>,
) -> bool {
    let max_distance = snapshot.owner_combat_reach + snapshot.target_combat_reach + range;
    if position_distance_sq(snapshot.owner_position, snapshot.target_position)
        > max_distance * max_distance
    {
        return false;
    }

    angle.is_none_or(|angle| {
        angle.is_angle_okay(relative_angle_like_cpp(
            snapshot.target_position,
            snapshot.owner_position,
        ))
    })
}

#[must_use]
pub fn selected_relative_angle_like_cpp(
    owner_position: Position,
    target_position: Position,
    desired_angle: ChaseAngle,
) -> f32 {
    let current = relative_angle_like_cpp(target_position, owner_position);
    if desired_angle.is_angle_okay(current) {
        return current;
    }

    let diff_upper = normalize_orientation_like_cpp(current - desired_angle.upper_bound());
    let diff_lower = normalize_orientation_like_cpp(desired_angle.lower_bound() - current);
    if diff_upper < diff_lower {
        desired_angle.upper_bound()
    } else {
        desired_angle.lower_bound()
    }
}

fn relative_angle_like_cpp(from: Position, to: Position) -> f32 {
    normalize_orientation_like_cpp((to.y - from.y).atan2(to.x - from.x) - from.orientation)
}

fn position_distance_sq(left: Position, right: Position) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    dx * dx + dy * dy + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guid(counter: i64) -> ObjectGuid {
        ObjectGuid::create_uniq(counter)
    }

    fn snapshot(owner: Position, target: Position) -> FollowUnitSnapshot {
        FollowUnitSnapshot {
            owner_position: owner,
            target_position: target,
            owner_combat_reach: 0.5,
            target_combat_reach: 0.5,
            owner_alive: true,
            target_in_world: true,
            can_move: true,
            movement_prevented_by_casting: false,
            owner_has_follow_move: false,
            owner_movespline_finalized: false,
            target_is_walking: false,
            owner_is_pet_of_target: false,
        }
    }

    #[test]
    fn abstract_follower_set_target_emits_remove_add_like_cpp() {
        let mut follower = AbstractFollower::new(guid(1));
        assert_eq!(follower.set_target_like_cpp(Some(guid(1))), Vec::new());
        assert_eq!(
            follower.set_target_like_cpp(Some(guid(2))),
            vec![
                AbstractFollowerEvent::FollowerRemoved(guid(1)),
                AbstractFollowerEvent::FollowerAdded(guid(2)),
            ]
        );
        assert_eq!(
            follower.set_target_like_cpp(None),
            vec![AbstractFollowerEvent::FollowerRemoved(guid(2))]
        );
    }

    #[test]
    fn follow_constructor_and_initialize_match_cpp_shape() {
        let mut follow = FollowMovementGenerator::new(
            guid(7),
            3.0,
            ChaseAngle::with_tolerance(0.0, 0.5),
            Some(1_000),
        );
        assert_eq!(follow.kind(), MovementGeneratorType::Follow);
        assert_eq!(follow.target(), Some(guid(7)));
        assert_eq!(follow.range(), 3.0);
        assert_eq!(follow.duration_ms(), Some(1_000));
        assert_eq!(follow.check_timer_ms(), FOLLOW_CHECK_INTERVAL_MS_LIKE_CPP);
        assert_eq!(follow.state().mode, MovementGeneratorMode::Default);
        assert_eq!(follow.state().priority, MovementGeneratorPriority::Normal);
        assert_eq!(
            follow.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(follow.state().base_unit_state, UNIT_STATE_FOLLOW_LIKE_CPP);

        follow.initialize_like_cpp();
        assert_eq!(follow.stop_moving_calls, 1);
        assert_eq!(follow.pet_speed_update_calls, 1);
        assert!(follow.has_flag(MovementGeneratorFlags::INITIALIZED));
        assert!(follow.has_flag(MovementGeneratorFlags::INFORM_ENABLED));
        assert!(!follow.has_flag(MovementGeneratorFlags::INITIALIZATION_PENDING));
    }

    #[test]
    fn follow_update_finishes_when_duration_passes_like_cpp() {
        let mut follow = FollowMovementGenerator::new(
            guid(7),
            3.0,
            ChaseAngle::with_tolerance(0.0, 0.5),
            Some(10),
        );
        let action = follow.update_like_cpp(
            true,
            true,
            11,
            snapshot(
                Position::new(0.0, 0.0, 0.0, 0.0),
                Position::new(1.0, 0.0, 0.0, 0.0),
            ),
        );
        assert_eq!(
            action,
            FollowMovementAction::StopMovingAndInform(FollowMovementInform {
                movement_type: MovementGeneratorType::Follow,
                target_counter: 7,
            })
        );
        assert_eq!(follow.stop_moving_calls, 1);
    }

    #[test]
    fn follow_update_stops_when_in_position_and_launches_when_out_of_range_like_cpp() {
        let mut follow =
            FollowMovementGenerator::new(guid(7), 3.0, ChaseAngle::with_tolerance(0.0, 0.5), None);
        follow.initialize_like_cpp();
        let in_range = snapshot(
            Position::new(2.0, 0.0, 0.0, 0.0),
            Position::new(0.0, 0.0, 0.0, 0.0),
        );
        assert_eq!(
            follow.update_like_cpp(true, true, 100, in_range),
            FollowMovementAction::StopMovingAndInform(FollowMovementInform {
                movement_type: MovementGeneratorType::Follow,
                target_counter: 7,
            })
        );
        assert!(!follow.has_flag(MovementGeneratorFlags::INFORM_ENABLED));

        let mut far = snapshot(
            Position::new(10.0, 0.0, 0.0, 0.0),
            Position::new(0.0, 0.0, 0.0, 0.0),
        );
        far.target_is_walking = true;
        far.owner_is_pet_of_target = true;
        assert_eq!(
            follow.update_like_cpp(true, true, 1, far),
            FollowMovementAction::Launch(FollowLaunchPlan {
                desired_range: 3.0,
                desired_relative_angle: 0.0,
                target_is_walking: true,
                allow_shortcut: true,
            })
        );
        assert!(follow.has_flag(MovementGeneratorFlags::INFORM_ENABLED));
    }

    #[test]
    fn follow_update_handles_blocked_and_spline_finalized_like_cpp() {
        let mut follow =
            FollowMovementGenerator::new(guid(7), 3.0, ChaseAngle::with_tolerance(0.0, 0.5), None);
        let mut blocked = snapshot(
            Position::new(10.0, 0.0, 0.0, 0.0),
            Position::new(0.0, 0.0, 0.0, 0.0),
        );
        blocked.can_move = false;
        assert_eq!(
            follow.update_like_cpp(true, true, 1, blocked),
            FollowMovementAction::StopMoving
        );

        let mut arrived = blocked;
        arrived.can_move = true;
        arrived.owner_has_follow_move = true;
        arrived.owner_movespline_finalized = true;
        assert_eq!(
            follow.update_like_cpp(true, true, 1, arrived),
            FollowMovementAction::ClearFollowMoveAndInform(FollowMovementInform {
                movement_type: MovementGeneratorType::Follow,
                target_counter: 7,
            })
        );
    }

    #[test]
    fn follow_deactivate_finalize_and_speed_change_match_cpp_flags() {
        let mut follow =
            FollowMovementGenerator::new(guid(7), 3.0, ChaseAngle::with_tolerance(0.0, 0.5), None);
        follow.initialize_like_cpp();
        follow.unit_speed_changed();
        let deactivate = follow.deactivate_like_cpp();
        assert_eq!(
            deactivate,
            FollowFinalizeAction {
                clear_follow_move: true,
                update_pet_speed: false,
            }
        );
        assert!(follow.has_flag(MovementGeneratorFlags::DEACTIVATED));
        assert!(!follow.has_flag(MovementGeneratorFlags::INFORM_ENABLED));

        let finalize = follow.finalize_like_cpp(true);
        assert_eq!(
            finalize,
            FollowFinalizeAction {
                clear_follow_move: true,
                update_pet_speed: true,
            }
        );
        assert_eq!(follow.pet_speed_update_calls, 2);
    }
}
