use wow_core::Position;

use crate::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType,
};

pub const UNIT_STATE_SPLINE_CHAIN_ROAMING_LIKE_CPP: u32 = 0x0000_0010;
pub const UNIT_STATE_SPLINE_CHAIN_ROAMING_MOVE_LIKE_CPP: u32 = 0x0080_0000;

#[derive(Debug, Clone, PartialEq)]
pub struct SplineChainLink {
    pub points: Vec<Position>,
    pub expected_duration_ms: u32,
    pub time_to_next_ms: u32,
    pub velocity: f32,
}

impl SplineChainLink {
    #[must_use]
    pub fn new(
        points: Vec<Position>,
        expected_duration_ms: u32,
        time_to_next_ms: u32,
        velocity: f32,
    ) -> Self {
        Self {
            points,
            expected_duration_ms,
            time_to_next_ms,
            velocity,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplineChainResumeInfo {
    pub point_id: u32,
    pub chain: Option<Vec<SplineChainLink>>,
    pub is_walk_mode: bool,
    pub spline_index: u8,
    pub point_index: u8,
    pub time_to_next_ms: u32,
}

impl SplineChainResumeInfo {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            point_id: 0,
            chain: None,
            is_walk_mode: false,
            spline_index: 0,
            point_index: 0,
            time_to_next_ms: 0,
        }
    }

    #[must_use]
    pub fn new(
        point_id: u32,
        chain: Vec<SplineChainLink>,
        is_walk_mode: bool,
        spline_index: u8,
        point_index: u8,
        time_to_next_ms: u32,
    ) -> Self {
        Self {
            point_id,
            chain: Some(chain),
            is_walk_mode,
            spline_index,
            point_index,
            time_to_next_ms,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.chain.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplineChainLaunchPlan {
    pub index: u8,
    pub path: Vec<Position>,
    pub move_by_path: bool,
    pub velocity: Option<f32>,
    pub walk: bool,
    pub add_unit_state: u32,
    pub ms_to_next_after_duration_adjustment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplineChainInform {
    pub movement_type: MovementGeneratorType,
    pub point_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SplineChainMovementAction {
    Continue,
    Finished,
    MissingChain,
    CurrentIndexAtEnd,
    InvalidResumePointClamped { clamped_point_index: u8 },
    Launch(SplineChainLaunchPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplineChainFinalizeAction {
    pub clear_roaming_move: bool,
    pub inform: Option<SplineChainInform>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplineChainMovementGenerator {
    state: MovementGeneratorState,
    id: u32,
    chain: Vec<SplineChainLink>,
    walk: bool,
    next_index: u8,
    next_first_wp: u8,
    ms_to_next: u32,
    pub stop_moving_calls: u32,
    pub last_launch: Option<SplineChainLaunchPlan>,
    pub finalize_action: Option<SplineChainFinalizeAction>,
}

impl SplineChainMovementGenerator {
    #[must_use]
    pub fn new(id: u32, chain: Vec<SplineChainLink>, walk: bool) -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_SPLINE_CHAIN_ROAMING_LIKE_CPP,
            },
            id,
            chain,
            walk,
            next_index: 0,
            next_first_wp: 0,
            ms_to_next: 0,
            stop_moving_calls: 0,
            last_launch: None,
            finalize_action: None,
        }
    }

    #[must_use]
    pub fn from_resume_info(info: SplineChainResumeInfo) -> Self {
        let chain = info.chain.unwrap_or_default();
        let mut generator = Self::new(info.point_id, chain, info.is_walk_mode);
        generator.next_index = info.spline_index;
        generator.next_first_wp = info.point_index;
        generator.ms_to_next = info.time_to_next_ms;
        if usize::from(info.spline_index) >= generator.chain.len() {
            generator.add_flag(MovementGeneratorFlags::FINALIZED);
        }
        generator
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn next_index(&self) -> u8 {
        self.next_index
    }

    #[must_use]
    pub const fn next_first_wp(&self) -> u8 {
        self.next_first_wp
    }

    #[must_use]
    pub const fn ms_to_next(&self) -> u32 {
        self.ms_to_next
    }

    #[must_use]
    pub fn chain(&self) -> &[SplineChainLink] {
        &self.chain
    }

    pub fn initialize_like_cpp(
        &mut self,
        actual_duration_ms: Option<u32>,
    ) -> SplineChainMovementAction {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);

        if self.chain.is_empty() {
            return SplineChainMovementAction::MissingChain;
        }

        if usize::from(self.next_index) >= self.chain.len() {
            self.ms_to_next = 0;
            return SplineChainMovementAction::CurrentIndexAtEnd;
        }

        if self.next_first_wp != 0 {
            if self.has_flag(MovementGeneratorFlags::FINALIZED) {
                return SplineChainMovementAction::Finished;
            }

            let link = &self.chain[usize::from(self.next_index)];
            let mut launch_after_clamp = None;
            if usize::from(self.next_first_wp) >= link.points.len() {
                self.next_first_wp = (link.points.len().saturating_sub(1)) as u8;
                launch_after_clamp = Some(SplineChainMovementAction::InvalidResumePointClamped {
                    clamped_point_index: self.next_first_wp,
                });
            }

            let path = link.points[usize::from(self.next_first_wp.saturating_sub(1))..].to_vec();
            let launch = self.send_path_spline_like_cpp(
                self.next_index,
                link.velocity,
                path,
                self.ms_to_next,
                actual_duration_ms,
                link.expected_duration_ms,
            );
            self.next_index = self.next_index.saturating_add(1);
            if usize::from(self.next_index) >= self.chain.len() {
                self.ms_to_next = 0;
            } else if self.ms_to_next == 0 {
                self.ms_to_next = 1;
            }
            self.next_first_wp = 0;

            return launch_after_clamp.unwrap_or(SplineChainMovementAction::Launch(launch));
        }

        self.ms_to_next = self.chain[usize::from(self.next_index)]
            .time_to_next_ms
            .max(1);
        let launch = self.send_spline_for_like_cpp(self.next_index, actual_duration_ms);
        self.next_index = self.next_index.saturating_add(1);
        if usize::from(self.next_index) >= self.chain.len() {
            self.ms_to_next = 0;
        }
        SplineChainMovementAction::Launch(launch)
    }

    pub fn reset_like_cpp(&mut self, actual_duration_ms: Option<u32>) -> SplineChainMovementAction {
        self.remove_flag(MovementGeneratorFlags::DEACTIVATED);
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
        self.initialize_like_cpp(actual_duration_ms)
    }

    pub fn update_like_cpp(
        &mut self,
        owner_exists: bool,
        owner_spline_finalized: bool,
        diff_ms: u32,
        actual_duration_ms: Option<u32>,
    ) -> SplineChainMovementAction {
        if !owner_exists || self.has_flag(MovementGeneratorFlags::FINALIZED) {
            return SplineChainMovementAction::Finished;
        }

        if self.ms_to_next == 0 {
            if owner_spline_finalized {
                self.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
                return SplineChainMovementAction::Finished;
            }
            return SplineChainMovementAction::Continue;
        }

        if self.ms_to_next <= diff_ms {
            self.ms_to_next = self.chain[usize::from(self.next_index)]
                .time_to_next_ms
                .max(1);
            let launch = self.send_spline_for_like_cpp(self.next_index, actual_duration_ms);
            self.next_index = self.next_index.saturating_add(1);
            if usize::from(self.next_index) >= self.chain.len() {
                self.ms_to_next = 0;
            }
            return SplineChainMovementAction::Launch(launch);
        }

        self.ms_to_next -= diff_ms;
        SplineChainMovementAction::Continue
    }

    pub fn deactivate_like_cpp(&mut self) -> SplineChainFinalizeAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        SplineChainFinalizeAction {
            clear_roaming_move: true,
            inform: None,
        }
    }

    pub fn finalize_like_cpp(
        &mut self,
        active: bool,
        movement_inform: bool,
        owner_is_creature_with_ai: bool,
    ) -> SplineChainFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let action = SplineChainFinalizeAction {
            clear_roaming_move: active,
            inform: (movement_inform
                && owner_is_creature_with_ai
                && self.has_flag(MovementGeneratorFlags::INFORM_ENABLED))
            .then_some(SplineChainInform {
                movement_type: MovementGeneratorType::SplineChain,
                point_id: self.id,
            }),
        };
        self.finalize_action = Some(action);
        action
    }

    #[must_use]
    pub fn get_resume_info_like_cpp(
        &self,
        owner_spline_finalized: bool,
        current_spline_idx: u8,
    ) -> SplineChainResumeInfo {
        if self.next_index == 0 {
            return SplineChainResumeInfo::new(
                self.id,
                self.chain.clone(),
                self.walk,
                0,
                0,
                self.ms_to_next,
            );
        }

        if owner_spline_finalized {
            if usize::from(self.next_index) < self.chain.len() {
                return SplineChainResumeInfo::new(
                    self.id,
                    self.chain.clone(),
                    self.walk,
                    self.next_index,
                    0,
                    1,
                );
            }
            return SplineChainResumeInfo::empty();
        }

        SplineChainResumeInfo::new(
            self.id,
            self.chain.clone(),
            self.walk,
            self.next_index.saturating_sub(1),
            current_spline_idx,
            self.ms_to_next,
        )
    }

    fn send_spline_for_like_cpp(
        &mut self,
        index: u8,
        actual_duration_ms: Option<u32>,
    ) -> SplineChainLaunchPlan {
        let link = &self.chain[usize::from(index)];
        self.send_path_spline_like_cpp(
            index,
            link.velocity,
            link.points.clone(),
            self.ms_to_next,
            actual_duration_ms,
            link.expected_duration_ms,
        )
    }

    fn send_path_spline_like_cpp(
        &mut self,
        index: u8,
        velocity: f32,
        path: Vec<Position>,
        mut duration: u32,
        actual_duration_ms: Option<u32>,
        expected_duration_ms: u32,
    ) -> SplineChainLaunchPlan {
        assert!(
            path.len() > 1,
            "SplineChainMovementGenerator::SendPathSpline requires source and destination"
        );

        if let Some(actual) = actual_duration_ms {
            if actual != expected_duration_ms && expected_duration_ms != 0 {
                duration = ((f64::from(actual) / f64::from(expected_duration_ms))
                    * f64::from(duration)) as u32;
            }
        }
        self.ms_to_next = duration;

        let launch = SplineChainLaunchPlan {
            index,
            move_by_path: path.len() > 2,
            path,
            velocity: (velocity > 0.0).then_some(velocity),
            walk: self.walk,
            add_unit_state: UNIT_STATE_SPLINE_CHAIN_ROAMING_MOVE_LIKE_CPP,
            ms_to_next_after_duration_adjustment: duration,
        };
        self.last_launch = Some(launch.clone());
        launch
    }
}

impl MovementGenerator for SplineChainMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::SplineChain
    }

    fn initialize(&mut self) {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);
    }

    fn reset(&mut self) {
        self.remove_flag(MovementGeneratorFlags::DEACTIVATED);
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
        self.initialize();
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        !self.has_flag(MovementGeneratorFlags::FINALIZED)
    }

    fn deactivate(&mut self) {
        self.deactivate_like_cpp();
    }

    fn finalize(&mut self, active: bool, movement_inform: bool) {
        self.finalize_like_cpp(active, movement_inform, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32) -> Position {
        Position::new(x, 0.0, 0.0, 0.0)
    }

    fn link(points: &[f32], expected: u32, next: u32, velocity: f32) -> SplineChainLink {
        SplineChainLink::new(
            points.iter().copied().map(point).collect(),
            expected,
            next,
            velocity,
        )
    }

    fn chain() -> Vec<SplineChainLink> {
        vec![
            link(&[0.0, 10.0, 20.0], 100, 50, 3.5),
            link(&[20.0, 30.0], 200, 70, 0.0),
        ]
    }

    #[test]
    fn spline_chain_constructor_matches_cpp_shape() {
        let generator = SplineChainMovementGenerator::new(42, chain(), true);
        assert_eq!(generator.kind(), MovementGeneratorType::SplineChain);
        assert_eq!(generator.state().mode, MovementGeneratorMode::Default);
        assert_eq!(
            generator.state().priority,
            MovementGeneratorPriority::Normal
        );
        assert_eq!(
            generator.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(
            generator.state().base_unit_state,
            UNIT_STATE_SPLINE_CHAIN_ROAMING_LIKE_CPP
        );
        assert_eq!(generator.id(), 42);
        assert_eq!(generator.next_index(), 0);
        assert_eq!(generator.next_first_wp(), 0);
        assert_eq!(generator.ms_to_next(), 0);
    }

    #[test]
    fn spline_chain_initialize_sends_first_spline_and_adjusts_duration_like_cpp() {
        let mut generator = SplineChainMovementGenerator::new(42, chain(), true);
        let action = generator.initialize_like_cpp(Some(150));
        let SplineChainMovementAction::Launch(launch) = action else {
            panic!("expected launch");
        };
        assert_eq!(launch.index, 0);
        assert!(launch.move_by_path);
        assert_eq!(launch.path, vec![point(0.0), point(10.0), point(20.0)]);
        assert_eq!(launch.velocity, Some(3.5));
        assert!(launch.walk);
        assert_eq!(
            launch.add_unit_state,
            UNIT_STATE_SPLINE_CHAIN_ROAMING_MOVE_LIKE_CPP
        );
        assert_eq!(launch.ms_to_next_after_duration_adjustment, 75);
        assert_eq!(generator.ms_to_next(), 75);
        assert_eq!(generator.next_index(), 1);
    }

    #[test]
    fn spline_chain_resume_starts_partial_spline_and_clamps_invalid_point() {
        let info = SplineChainResumeInfo::new(77, chain(), false, 0, 99, 0);
        let mut generator = SplineChainMovementGenerator::from_resume_info(info);
        let action = generator.initialize_like_cpp(None);
        assert_eq!(
            action,
            SplineChainMovementAction::InvalidResumePointClamped {
                clamped_point_index: 2
            }
        );
        let launch = generator.last_launch.clone().expect("launch");
        assert_eq!(launch.path, vec![point(10.0), point(20.0)]);
        assert_eq!(generator.next_index(), 1);
        assert_eq!(generator.ms_to_next(), 1);
        assert_eq!(generator.next_first_wp(), 0);
    }

    #[test]
    fn spline_chain_update_waits_launches_next_and_finishes_on_final_spline() {
        let mut generator = SplineChainMovementGenerator::new(42, chain(), false);
        generator.initialize_like_cpp(None);
        assert_eq!(
            generator.update_like_cpp(true, false, 49, None),
            SplineChainMovementAction::Continue
        );
        assert_eq!(generator.ms_to_next(), 1);

        let action = generator.update_like_cpp(true, false, 1, Some(100));
        let SplineChainMovementAction::Launch(launch) = action else {
            panic!("expected second launch");
        };
        assert_eq!(launch.index, 1);
        assert!(!launch.move_by_path);
        assert_eq!(launch.velocity, None);
        assert_eq!(generator.next_index(), 2);
        assert_eq!(generator.ms_to_next(), 0);

        assert_eq!(
            generator.update_like_cpp(true, false, 1, None),
            SplineChainMovementAction::Continue
        );
        assert_eq!(
            generator.update_like_cpp(true, true, 1, None),
            SplineChainMovementAction::Finished
        );
        assert!(generator.has_flag(MovementGeneratorFlags::INFORM_ENABLED));
    }

    #[test]
    fn spline_chain_finalize_and_deactivate_match_cpp() {
        let mut generator = SplineChainMovementGenerator::new(9, chain(), false);
        assert_eq!(
            generator.deactivate_like_cpp(),
            SplineChainFinalizeAction {
                clear_roaming_move: true,
                inform: None,
            }
        );
        generator.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
        assert_eq!(
            generator.finalize_like_cpp(true, true, true),
            SplineChainFinalizeAction {
                clear_roaming_move: true,
                inform: Some(SplineChainInform {
                    movement_type: MovementGeneratorType::SplineChain,
                    point_id: 9,
                }),
            }
        );
    }

    #[test]
    fn spline_chain_resume_info_matches_cpp_branches() {
        let mut generator = SplineChainMovementGenerator::new(42, chain(), true);
        assert_eq!(
            generator.get_resume_info_like_cpp(false, 0),
            SplineChainResumeInfo::new(42, chain(), true, 0, 0, 0)
        );

        generator.initialize_like_cpp(None);
        assert_eq!(
            generator.get_resume_info_like_cpp(false, 2),
            SplineChainResumeInfo::new(42, chain(), true, 0, 2, 50)
        );
        assert_eq!(
            generator.get_resume_info_like_cpp(true, 2),
            SplineChainResumeInfo::new(42, chain(), true, 1, 0, 1)
        );

        generator.next_index = 2;
        assert!(generator.get_resume_info_like_cpp(true, 2).is_empty());
    }
}
