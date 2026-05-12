use crate::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleMovementGenerator {
    state: MovementGeneratorState,
    pub stop_moving_calls: u32,
}

impl Default for IdleMovementGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleMovementGenerator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZED,
                base_unit_state: 0,
            },
            stop_moving_calls: 0,
        }
    }
}

impl MovementGenerator for IdleMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Idle
    }

    fn initialize(&mut self) {
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
    }

    fn reset(&mut self) {
        self.stop_moving_calls = self.stop_moving_calls.saturating_add(1);
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        true
    }

    fn deactivate(&mut self) {}

    fn finalize(&mut self, _active: bool, _movement_inform: bool) {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_movement_generator_matches_cpp_lifecycle_shape() {
        let mut idle = IdleMovementGenerator::new();
        assert_eq!(idle.kind(), MovementGeneratorType::Idle);
        assert_eq!(idle.state().mode, MovementGeneratorMode::Default);
        assert_eq!(idle.state().priority, MovementGeneratorPriority::Normal);
        assert_eq!(idle.state().flags, MovementGeneratorFlags::INITIALIZED);
        assert_eq!(idle.state().base_unit_state, 0);

        idle.initialize();
        idle.reset();
        assert_eq!(idle.stop_moving_calls, 2);
        assert!(idle.update(999));
        idle.deactivate();
        assert!(!idle.has_flag(MovementGeneratorFlags::FINALIZED));
        idle.finalize(true, true);
        assert!(idle.has_flag(MovementGeneratorFlags::FINALIZED));
    }
}
