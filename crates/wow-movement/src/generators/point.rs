use std::f32::consts::PI;

use wow_core::{ObjectGuid, Position};

use crate::{
    MoveSpline, MoveSplineInit, MoveSplineLaunchError, MoveSplineLaunchInput,
    MoveSplineLaunchResult, MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode,
    MovementGeneratorPriority, MovementGeneratorState, MovementGeneratorType,
    MovementWalkRunSpeedSelectionMode, SpellEffectExtraData, UNIT_STATE_ROAMING_LIKE_CPP,
};

pub const UNIT_STATE_ROAMING_MOVE_LIKE_CPP: u32 = 0x0080_0000;
pub const EVENT_CHARGE_LIKE_CPP: u32 = 1003;
pub const EVENT_CHARGE_PREPATH_LIKE_CPP: u32 = 1005;
pub const CREATURE_FAMILY_ASSISTANCE_DELAY_MS_LIKE_CPP: u32 = 1_500;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointMovementLaunch {
    pub mark_roaming_move: bool,
    pub signal_formation_movement: bool,
    pub launch_result: Option<MoveSplineLaunchResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointMovementAction {
    Continue,
    StopMoving,
    StopMovingAndContinue,
    RelaunchSpline,
    Finished,
    MarkRoamingMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMovementInform {
    pub movement_type: MovementGeneratorType,
    pub movement_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMovementFinalize {
    pub clear_roaming_move: bool,
    pub inform: Option<PointMovementInform>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssistanceMovementFinalize {
    pub clear_roaming_move: bool,
    pub set_no_call_assistance_false: bool,
    pub call_assistance: bool,
    pub move_seek_assistance_distract_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointMovementGenerator {
    state: MovementGeneratorState,
    movement_id: u32,
    destination: Position,
    speed: Option<f32>,
    generate_path: bool,
    final_orientation: Option<f32>,
    face_target: Option<ObjectGuid>,
    spell_effect_extra: Option<SpellEffectExtraData>,
    speed_selection_mode: MovementWalkRunSpeedSelectionMode,
    close_enough_distance: Option<f32>,
    pub last_launch: Option<PointMovementLaunch>,
    pub last_launch_error: Option<MoveSplineLaunchError>,
    pub finalize_result: Option<PointMovementFinalize>,
}

impl PointMovementGenerator {
    #[must_use]
    pub const fn new(movement_id: u32, destination: Position, generate_path: bool) -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_ROAMING_LIKE_CPP,
            },
            movement_id,
            destination,
            speed: None,
            generate_path,
            final_orientation: None,
            face_target: None,
            spell_effect_extra: None,
            speed_selection_mode: MovementWalkRunSpeedSelectionMode::Default,
            close_enough_distance: None,
            last_launch: None,
            last_launch_error: None,
            finalize_result: None,
        }
    }

    #[must_use]
    pub const fn movement_id(&self) -> u32 {
        self.movement_id
    }

    #[must_use]
    pub const fn destination(&self) -> Position {
        self.destination
    }

    #[must_use]
    pub const fn generate_path(&self) -> bool {
        self.generate_path
    }

    #[must_use]
    pub const fn speed(&self) -> Option<f32> {
        self.speed
    }

    #[must_use]
    pub const fn final_orientation(&self) -> Option<f32> {
        self.final_orientation
    }

    #[must_use]
    pub const fn face_target(&self) -> Option<ObjectGuid> {
        self.face_target
    }

    #[must_use]
    pub const fn spell_effect_extra(&self) -> Option<SpellEffectExtraData> {
        self.spell_effect_extra
    }

    #[must_use]
    pub const fn speed_selection_mode(&self) -> MovementWalkRunSpeedSelectionMode {
        self.speed_selection_mode
    }

    #[must_use]
    pub const fn close_enough_distance(&self) -> Option<f32> {
        self.close_enough_distance
    }

    #[must_use]
    pub const fn with_speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }

    #[must_use]
    pub const fn with_final_orientation(mut self, final_orientation: f32) -> Self {
        self.final_orientation = Some(final_orientation);
        self
    }

    #[must_use]
    pub const fn with_face_target(mut self, face_target: ObjectGuid) -> Self {
        self.face_target = Some(face_target);
        self
    }

    #[must_use]
    pub const fn with_spell_effect_extra(
        mut self,
        spell_effect_extra: SpellEffectExtraData,
    ) -> Self {
        self.spell_effect_extra = Some(spell_effect_extra);
        self
    }

    #[must_use]
    pub const fn with_speed_selection_mode(
        mut self,
        speed_selection_mode: MovementWalkRunSpeedSelectionMode,
    ) -> Self {
        self.speed_selection_mode = speed_selection_mode;
        self
    }

    #[must_use]
    pub const fn with_close_enough_distance(mut self, close_enough_distance: f32) -> Self {
        self.close_enough_distance = Some(close_enough_distance);
        self
    }

    pub fn initialize_with_spline_like_cpp(
        &mut self,
        can_move: bool,
        movement_prevented_by_casting: bool,
        move_spline: &mut MoveSpline,
        launch_input: MoveSplineLaunchInput,
        spline_id: u32,
    ) -> Result<PointMovementLaunch, MoveSplineLaunchError> {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING
                | MovementGeneratorFlags::TRANSITORY
                | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);

        if self.movement_id == EVENT_CHARGE_PREPATH_LIKE_CPP {
            let launch = PointMovementLaunch {
                mark_roaming_move: true,
                signal_formation_movement: false,
                launch_result: None,
            };
            self.last_launch = Some(launch);
            return Ok(launch);
        }

        if !can_move || movement_prevented_by_casting {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            return Ok(PointMovementLaunch {
                mark_roaming_move: false,
                signal_formation_movement: false,
                launch_result: None,
            });
        }

        self.launch_direct_like_cpp(move_spline, launch_input, spline_id)
    }

    pub fn reset_with_spline_like_cpp(
        &mut self,
        can_move: bool,
        movement_prevented_by_casting: bool,
        move_spline: &mut MoveSpline,
        launch_input: MoveSplineLaunchInput,
        spline_id: u32,
    ) -> Result<PointMovementLaunch, MoveSplineLaunchError> {
        self.remove_flag(MovementGeneratorFlags::TRANSITORY | MovementGeneratorFlags::DEACTIVATED);
        self.initialize_with_spline_like_cpp(
            can_move,
            movement_prevented_by_casting,
            move_spline,
            launch_input,
            spline_id,
        )
    }

    pub fn update_with_spline_like_cpp(
        &mut self,
        owner_exists: bool,
        can_move: bool,
        movement_prevented_by_casting: bool,
        move_spline: &MoveSpline,
    ) -> PointMovementAction {
        if !owner_exists {
            return PointMovementAction::Finished;
        }

        if self.movement_id == EVENT_CHARGE_PREPATH_LIKE_CPP {
            if move_spline.finalized() {
                self.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
                return PointMovementAction::Finished;
            }
            return PointMovementAction::Continue;
        }

        if !can_move || movement_prevented_by_casting {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            return PointMovementAction::StopMovingAndContinue;
        }

        if (self.has_flag(MovementGeneratorFlags::INTERRUPTED) && move_spline.finalized())
            || (self.has_flag(MovementGeneratorFlags::SPEED_UPDATE_PENDING)
                && !move_spline.finalized())
        {
            self.remove_flag(
                MovementGeneratorFlags::INTERRUPTED | MovementGeneratorFlags::SPEED_UPDATE_PENDING,
            );
            return PointMovementAction::RelaunchSpline;
        }

        if move_spline.finalized() {
            self.remove_flag(MovementGeneratorFlags::TRANSITORY);
            self.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
            return PointMovementAction::Finished;
        }

        PointMovementAction::Continue
    }

    pub fn deactivate_like_cpp(&mut self) -> PointMovementAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        PointMovementAction::StopMoving
    }

    pub fn finalize_with_owner_like_cpp(
        &mut self,
        active: bool,
        movement_inform: bool,
        owner_is_creature: bool,
    ) -> PointMovementFinalize {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let inform = (movement_inform
            && self.has_flag(MovementGeneratorFlags::INFORM_ENABLED)
            && owner_is_creature)
            .then_some(PointMovementInform {
                movement_type: MovementGeneratorType::Point,
                movement_id: if self.movement_id == EVENT_CHARGE_PREPATH_LIKE_CPP {
                    EVENT_CHARGE_LIKE_CPP
                } else {
                    self.movement_id
                },
            });
        let result = PointMovementFinalize {
            clear_roaming_move: active,
            inform,
        };
        self.finalize_result = Some(result);
        result
    }

    fn launch_direct_like_cpp(
        &mut self,
        move_spline: &mut MoveSpline,
        launch_input: MoveSplineLaunchInput,
        spline_id: u32,
    ) -> Result<PointMovementLaunch, MoveSplineLaunchError> {
        let mut init = MoveSplineInit::new(spline_id);
        let destination = self.direct_destination_like_cpp(launch_input.current_position);
        init.move_to(destination);
        if let Some(speed) = self.speed {
            init.set_velocity(speed);
        }
        if let Some(face_target) = self.face_target {
            init.set_facing_target_with_angle(face_target, 0.0);
        }
        if let Some(spell_effect_extra) = self.spell_effect_extra {
            init.set_spell_effect_extra_data(spell_effect_extra);
        }
        if let Some(final_orientation) = self.final_orientation {
            init.set_facing_angle(final_orientation);
        }
        match self.speed_selection_mode {
            MovementWalkRunSpeedSelectionMode::Default => {}
            MovementWalkRunSpeedSelectionMode::ForceRun => init.set_walk(false),
            MovementWalkRunSpeedSelectionMode::ForceWalk => init.set_walk(true),
        }
        match init.launch(move_spline, launch_input) {
            Ok(result) => {
                let launch = PointMovementLaunch {
                    mark_roaming_move: true,
                    signal_formation_movement: true,
                    launch_result: Some(result),
                };
                self.last_launch = Some(launch);
                self.last_launch_error = None;
                Ok(launch)
            }
            Err(error) => {
                self.last_launch = None;
                self.last_launch_error = Some(error);
                Err(error)
            }
        }
    }

    fn direct_destination_like_cpp(&self, owner_position: Position) -> Position {
        let Some(close_enough_distance) = self.close_enough_distance else {
            return self.destination;
        };
        let dx = self.destination.x - owner_position.x;
        let dy = self.destination.y - owner_position.y;
        let dz = self.destination.z - owner_position.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance <= f32::EPSILON {
            return self.destination;
        }
        let offset = close_enough_distance.min(distance);
        let angle = dy.atan2(dx) + PI;
        Position::new(
            self.destination.x + offset * angle.cos(),
            self.destination.y + offset * angle.sin(),
            self.destination.z,
            self.destination.orientation,
        )
    }
}

impl MovementGenerator for PointMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Point
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
        self.initialize();
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        true
    }

    fn deactivate(&mut self) {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
    }

    fn finalize(&mut self, active: bool, movement_inform: bool) {
        self.finalize_with_owner_like_cpp(active, movement_inform, false);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssistanceMovementGenerator {
    inner: PointMovementGenerator,
    pub finalize_result: Option<AssistanceMovementFinalize>,
}

impl AssistanceMovementGenerator {
    #[must_use]
    pub const fn new(movement_id: u32, destination: Position) -> Self {
        Self {
            inner: PointMovementGenerator::new(movement_id, destination, true),
            finalize_result: None,
        }
    }

    #[must_use]
    pub const fn movement_id(&self) -> u32 {
        self.inner.movement_id()
    }

    pub fn finalize_with_owner_like_cpp(
        &mut self,
        active: bool,
        movement_inform: bool,
        owner_is_creature: bool,
        owner_is_alive: bool,
    ) -> AssistanceMovementFinalize {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let should_assist = movement_inform
            && self.has_flag(MovementGeneratorFlags::INFORM_ENABLED)
            && owner_is_creature;
        let result = AssistanceMovementFinalize {
            clear_roaming_move: active,
            set_no_call_assistance_false: should_assist,
            call_assistance: should_assist,
            move_seek_assistance_distract_ms: (should_assist && owner_is_alive)
                .then_some(CREATURE_FAMILY_ASSISTANCE_DELAY_MS_LIKE_CPP),
        };
        self.finalize_result = Some(result);
        result
    }
}

impl MovementGenerator for AssistanceMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        self.inner.state()
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        self.inner.state_mut()
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Assistance
    }

    fn initialize(&mut self) {
        self.inner.initialize();
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn update(&mut self, diff_ms: u32) -> bool {
        self.inner.update(diff_ms)
    }

    fn deactivate(&mut self) {
        self.inner.deactivate();
    }

    fn finalize(&mut self, active: bool, movement_inform: bool) {
        self.finalize_with_owner_like_cpp(active, movement_inform, false, false);
    }
}

#[cfg(test)]
mod tests {
    use wow_constants::movement::MovementFlag;

    use super::*;

    fn launch_input() -> MoveSplineLaunchInput {
        MoveSplineLaunchInput {
            current_position: Position::new(0.0, 0.0, 0.0, 0.0),
            active_spline_position: None,
            movement_flags: MovementFlag::empty(),
            selected_speed: 4.0,
            run_speed: 7.0,
            assistance_speed_factor: 1.0,
            on_transport: false,
        }
    }

    #[test]
    fn point_movement_generator_matches_cpp_constructor_shape() {
        let target = ObjectGuid::create_uniq(0x55);
        let spell = SpellEffectExtraData {
            target,
            spell_visual_id: 1,
            progress_curve_id: 2,
            parabolic_curve_id: 3,
        };
        let point = PointMovementGenerator::new(42, Position::new(10.0, 0.0, 0.0, 0.25), true)
            .with_speed(8.0)
            .with_final_orientation(1.0)
            .with_face_target(target)
            .with_spell_effect_extra(spell)
            .with_speed_selection_mode(MovementWalkRunSpeedSelectionMode::ForceWalk)
            .with_close_enough_distance(2.0);
        assert_eq!(point.kind(), MovementGeneratorType::Point);
        assert_eq!(point.movement_id(), 42);
        assert_eq!(point.state().mode, MovementGeneratorMode::Default);
        assert_eq!(point.state().priority, MovementGeneratorPriority::Normal);
        assert_eq!(
            point.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(point.state().base_unit_state, UNIT_STATE_ROAMING_LIKE_CPP);
        assert!(point.generate_path());
        assert_eq!(point.speed(), Some(8.0));
        assert_eq!(point.final_orientation(), Some(1.0));
        assert_eq!(point.face_target(), Some(target));
        assert_eq!(point.spell_effect_extra(), Some(spell));
        assert_eq!(
            point.speed_selection_mode(),
            MovementWalkRunSpeedSelectionMode::ForceWalk
        );
        assert_eq!(point.close_enough_distance(), Some(2.0));
    }

    #[test]
    fn point_initialize_launches_direct_spline_and_applies_close_enough_like_cpp() {
        let mut point = PointMovementGenerator::new(42, Position::new(10.0, 0.0, 0.0, 0.0), false)
            .with_speed(8.0)
            .with_close_enough_distance(2.0)
            .with_final_orientation(1.5)
            .with_speed_selection_mode(MovementWalkRunSpeedSelectionMode::ForceWalk);
        let mut spline = MoveSpline::new();
        let launch = point
            .initialize_with_spline_like_cpp(true, false, &mut spline, launch_input(), 77)
            .expect("launch");
        assert!(launch.mark_roaming_move);
        assert!(launch.signal_formation_movement);
        assert!(point.has_flag(MovementGeneratorFlags::INITIALIZED));
        let destination = spline.final_destination().expect("destination");
        assert!((destination.x - 8.0).abs() < 0.000_01);
        assert!((destination.y - 0.0).abs() < 0.000_01);
        assert_eq!(spline.id(), 77);
        assert_eq!(spline.velocity(), 8.0);
        assert!(spline.facing().angle > 1.49 && spline.facing().angle < 1.51);
    }

    #[test]
    fn point_initialize_handles_prepath_and_blocked_owner_like_cpp() {
        let mut prepath = PointMovementGenerator::new(
            EVENT_CHARGE_PREPATH_LIKE_CPP,
            Position::new(1.0, 0.0, 0.0, 0.0),
            true,
        );
        let mut spline = MoveSpline::new();
        let launch = prepath
            .initialize_with_spline_like_cpp(true, false, &mut spline, launch_input(), 77)
            .expect("prepath");
        assert_eq!(
            launch,
            PointMovementLaunch {
                mark_roaming_move: true,
                signal_formation_movement: false,
                launch_result: None,
            }
        );
        assert!(!spline.initialized());

        let mut blocked = PointMovementGenerator::new(42, Position::new(1.0, 0.0, 0.0, 0.0), false);
        let launch = blocked
            .initialize_with_spline_like_cpp(false, false, &mut spline, launch_input(), 77)
            .expect("blocked");
        assert!(!launch.mark_roaming_move);
        assert!(blocked.has_flag(MovementGeneratorFlags::INTERRUPTED));
    }

    #[test]
    fn point_update_and_finalize_match_cpp_inform_rules() {
        let mut point = PointMovementGenerator::new(
            EVENT_CHARGE_PREPATH_LIKE_CPP,
            Position::new(1.0, 0.0, 0.0, 0.0),
            true,
        );
        let spline = MoveSpline::new();
        assert_eq!(
            point.update_with_spline_like_cpp(true, true, false, &spline),
            PointMovementAction::Finished
        );
        assert!(point.has_flag(MovementGeneratorFlags::INFORM_ENABLED));
        let finalize = point.finalize_with_owner_like_cpp(true, true, true);
        assert_eq!(
            finalize,
            PointMovementFinalize {
                clear_roaming_move: true,
                inform: Some(PointMovementInform {
                    movement_type: MovementGeneratorType::Point,
                    movement_id: EVENT_CHARGE_LIKE_CPP,
                }),
            }
        );

        let mut interrupted =
            PointMovementGenerator::new(42, Position::new(1.0, 0.0, 0.0, 0.0), false);
        assert_eq!(
            interrupted.update_with_spline_like_cpp(true, false, false, &spline),
            PointMovementAction::StopMovingAndContinue
        );
        assert!(interrupted.has_flag(MovementGeneratorFlags::INTERRUPTED));
    }

    #[test]
    fn assistance_finalize_matches_cpp_side_effect_plan() {
        let mut assistance =
            AssistanceMovementGenerator::new(1009, Position::new(1.0, 2.0, 3.0, 0.0));
        assistance.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
        let result = assistance.finalize_with_owner_like_cpp(true, true, true, true);
        assert_eq!(
            result,
            AssistanceMovementFinalize {
                clear_roaming_move: true,
                set_no_call_assistance_false: true,
                call_assistance: true,
                move_seek_assistance_distract_ms: Some(
                    CREATURE_FAMILY_ASSISTANCE_DELAY_MS_LIKE_CPP
                ),
            }
        );
        assert_eq!(assistance.kind(), MovementGeneratorType::Assistance);
    }
}
