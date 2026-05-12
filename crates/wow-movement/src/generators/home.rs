use wow_core::Position;

use crate::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType,
};

pub const UNIT_STATE_STUNNED_LIKE_CPP: u32 = 0x0000_0008;
pub const UNIT_STATE_ROAMING_LIKE_CPP: u32 = 0x0000_0010;
pub const UNIT_STATE_ROOT_LIKE_CPP: u32 = 0x0000_0400;
pub const UNIT_STATE_DISTRACTED_LIKE_CPP: u32 = 0x0000_1000;
pub const UNIT_STATE_EVADE_LIKE_CPP: u32 = 0x0040_0000;
pub const UNIT_STATE_ROAMING_MOVE_LIKE_CPP: u32 = 0x0080_0000;
pub const UNIT_STATE_IGNORE_PATHFINDING_LIKE_CPP: u32 = 0x1000_0000;
pub const UNIT_FLAG_CAN_SWIM_LIKE_CPP: u32 = 0x0000_8000;

pub const UNIT_STATE_HOME_INTERRUPT_MASK_LIKE_CPP: u32 =
    UNIT_STATE_ROOT_LIKE_CPP | UNIT_STATE_STUNNED_LIKE_CPP | UNIT_STATE_DISTRACTED_LIKE_CPP;

pub const UNIT_STATE_ALL_STATE_SUPPORTED_LIKE_CPP: u32 = 0x3fff_ffff;
pub const UNIT_STATE_ALL_ERASABLE_LIKE_CPP: u32 =
    UNIT_STATE_ALL_STATE_SUPPORTED_LIKE_CPP & !UNIT_STATE_IGNORE_PATHFINDING_LIKE_CPP;
pub const HOME_CLEAR_ON_TARGET_MASK_LIKE_CPP: u32 =
    UNIT_STATE_ALL_ERASABLE_LIKE_CPP & !UNIT_STATE_EVADE_LIKE_CPP;
pub const HOME_CLEAR_ON_FINALIZE_MASK_LIKE_CPP: u32 =
    UNIT_STATE_ROAMING_MOVE_LIKE_CPP | UNIT_STATE_EVADE_LIKE_CPP;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HomeUnitSnapshot {
    pub owner_alive: bool,
    pub owner_unit_state: u32,
    pub home_position: Position,
    pub move_spline_finalized: bool,
    pub can_swim_out_of_combat: bool,
    pub is_vehicle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HomeLaunchPlan {
    pub destination: Position,
    pub facing: f32,
    pub walk: bool,
    pub clear_unit_state_mask: u32,
    pub add_unit_state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HomeMovementAction {
    Continue,
    Finished,
    Interrupted,
    Launch(HomeLaunchPlan),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeFinalizeAction {
    pub clear_unit_state_mask: u32,
    pub remove_can_swim_flag: bool,
    pub set_spawn_health: bool,
    pub load_creatures_addon: bool,
    pub load_creatures_sparring_health: bool,
    pub reset_vehicle: bool,
    pub just_reached_home: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HomeMovementGenerator {
    state: MovementGeneratorState,
    last_launch: Option<HomeLaunchPlan>,
    pub set_no_search_assistance_false_calls: u32,
    pub finalize_action: Option<HomeFinalizeAction>,
}

impl HomeMovementGenerator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: MovementGeneratorState {
                mode: MovementGeneratorMode::Default,
                priority: MovementGeneratorPriority::Normal,
                flags: MovementGeneratorFlags::INITIALIZATION_PENDING,
                base_unit_state: UNIT_STATE_ROAMING_LIKE_CPP,
            },
            last_launch: None,
            set_no_search_assistance_false_calls: 0,
            finalize_action: None,
        }
    }

    #[must_use]
    pub const fn last_launch(&self) -> Option<HomeLaunchPlan> {
        self.last_launch
    }

    pub fn initialize_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: HomeUnitSnapshot,
    ) -> HomeMovementAction {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);

        if !owner_exists || !snapshot.owner_alive {
            return HomeMovementAction::Continue;
        }

        self.set_no_search_assistance_false_calls =
            self.set_no_search_assistance_false_calls.saturating_add(1);
        self.set_target_location_like_cpp(snapshot)
    }

    pub fn reset_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: HomeUnitSnapshot,
    ) -> HomeMovementAction {
        self.remove_flag(MovementGeneratorFlags::DEACTIVATED);
        self.initialize_like_cpp(owner_exists, snapshot)
    }

    pub fn set_target_location_like_cpp(
        &mut self,
        snapshot: HomeUnitSnapshot,
    ) -> HomeMovementAction {
        if snapshot.owner_unit_state & UNIT_STATE_HOME_INTERRUPT_MASK_LIKE_CPP != 0 {
            self.add_flag(MovementGeneratorFlags::INTERRUPTED);
            return HomeMovementAction::Interrupted;
        }

        let launch = HomeLaunchPlan {
            destination: snapshot.home_position,
            facing: snapshot.home_position.orientation,
            walk: false,
            clear_unit_state_mask: HOME_CLEAR_ON_TARGET_MASK_LIKE_CPP,
            add_unit_state: UNIT_STATE_ROAMING_MOVE_LIKE_CPP,
        };
        self.last_launch = Some(launch);
        HomeMovementAction::Launch(launch)
    }

    pub fn update_like_cpp(
        &mut self,
        owner_exists: bool,
        snapshot: HomeUnitSnapshot,
    ) -> HomeMovementAction {
        if !owner_exists || !snapshot.owner_alive {
            return HomeMovementAction::Finished;
        }

        if self.has_flag(MovementGeneratorFlags::INTERRUPTED) || snapshot.move_spline_finalized {
            self.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
            return HomeMovementAction::Finished;
        }

        HomeMovementAction::Continue
    }

    pub fn deactivate_like_cpp(&mut self) -> HomeFinalizeAction {
        self.add_flag(MovementGeneratorFlags::DEACTIVATED);
        HomeFinalizeAction {
            clear_unit_state_mask: UNIT_STATE_ROAMING_MOVE_LIKE_CPP,
            remove_can_swim_flag: false,
            set_spawn_health: false,
            load_creatures_addon: false,
            load_creatures_sparring_health: false,
            reset_vehicle: false,
            just_reached_home: false,
        }
    }

    pub fn finalize_like_cpp(
        &mut self,
        active: bool,
        movement_inform: bool,
        snapshot: HomeUnitSnapshot,
    ) -> HomeFinalizeAction {
        self.add_flag(MovementGeneratorFlags::FINALIZED);
        let inform = movement_inform && self.has_flag(MovementGeneratorFlags::INFORM_ENABLED);
        let action = HomeFinalizeAction {
            clear_unit_state_mask: if active {
                HOME_CLEAR_ON_FINALIZE_MASK_LIKE_CPP
            } else {
                0
            },
            remove_can_swim_flag: inform && !snapshot.can_swim_out_of_combat,
            set_spawn_health: inform,
            load_creatures_addon: inform,
            load_creatures_sparring_health: inform,
            reset_vehicle: inform && snapshot.is_vehicle,
            just_reached_home: inform,
        };
        self.finalize_action = Some(action);
        action
    }
}

impl Default for HomeMovementGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl MovementGenerator for HomeMovementGenerator {
    fn state(&self) -> &MovementGeneratorState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut MovementGeneratorState {
        &mut self.state
    }

    fn kind(&self) -> MovementGeneratorType {
        MovementGeneratorType::Home
    }

    fn initialize(&mut self) {
        self.remove_flag(
            MovementGeneratorFlags::INITIALIZATION_PENDING | MovementGeneratorFlags::DEACTIVATED,
        );
        self.add_flag(MovementGeneratorFlags::INITIALIZED);
    }

    fn reset(&mut self) {
        self.remove_flag(MovementGeneratorFlags::DEACTIVATED);
        self.initialize();
    }

    fn update(&mut self, _diff_ms: u32) -> bool {
        false
    }

    fn deactivate(&mut self) {
        self.deactivate_like_cpp();
    }

    fn finalize(&mut self, active: bool, movement_inform: bool) {
        self.finalize_like_cpp(
            active,
            movement_inform,
            HomeUnitSnapshot {
                owner_alive: true,
                owner_unit_state: 0,
                home_position: Position::new(0.0, 0.0, 0.0, 0.0),
                move_spline_finalized: true,
                can_swim_out_of_combat: false,
                is_vehicle: false,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> HomeUnitSnapshot {
        HomeUnitSnapshot {
            owner_alive: true,
            owner_unit_state: 0,
            home_position: Position::new(10.0, 20.0, 30.0, 1.5),
            move_spline_finalized: false,
            can_swim_out_of_combat: false,
            is_vehicle: false,
        }
    }

    #[test]
    fn home_constructor_and_initialize_match_cpp_shape() {
        let mut home = HomeMovementGenerator::new();
        assert_eq!(home.kind(), MovementGeneratorType::Home);
        assert_eq!(home.state().mode, MovementGeneratorMode::Default);
        assert_eq!(home.state().priority, MovementGeneratorPriority::Normal);
        assert_eq!(
            home.state().flags,
            MovementGeneratorFlags::INITIALIZATION_PENDING
        );
        assert_eq!(home.state().base_unit_state, UNIT_STATE_ROAMING_LIKE_CPP);

        let action = home.initialize_like_cpp(true, snapshot());
        assert!(home.has_flag(MovementGeneratorFlags::INITIALIZED));
        assert!(!home.has_flag(MovementGeneratorFlags::INITIALIZATION_PENDING));
        assert_eq!(home.set_no_search_assistance_false_calls, 1);
        assert_eq!(
            action,
            HomeMovementAction::Launch(HomeLaunchPlan {
                destination: Position::new(10.0, 20.0, 30.0, 1.5),
                facing: 1.5,
                walk: false,
                clear_unit_state_mask: HOME_CLEAR_ON_TARGET_MASK_LIKE_CPP,
                add_unit_state: UNIT_STATE_ROAMING_MOVE_LIKE_CPP,
            })
        );
    }

    #[test]
    fn home_set_target_interrupts_root_stunned_or_distracted_like_cpp() {
        for state in [
            UNIT_STATE_ROOT_LIKE_CPP,
            UNIT_STATE_STUNNED_LIKE_CPP,
            UNIT_STATE_DISTRACTED_LIKE_CPP,
        ] {
            let mut home = HomeMovementGenerator::new();
            let mut snap = snapshot();
            snap.owner_unit_state = state;
            assert_eq!(
                home.set_target_location_like_cpp(snap),
                HomeMovementAction::Interrupted
            );
            assert!(home.has_flag(MovementGeneratorFlags::INTERRUPTED));
            assert_eq!(home.last_launch(), None);
        }
    }

    #[test]
    fn home_update_finishes_on_interrupted_or_finalized_spline_like_cpp() {
        let mut home = HomeMovementGenerator::new();
        home.add_flag(MovementGeneratorFlags::INTERRUPTED);
        assert_eq!(
            home.update_like_cpp(true, snapshot()),
            HomeMovementAction::Finished
        );
        assert!(home.has_flag(MovementGeneratorFlags::INFORM_ENABLED));

        let mut home = HomeMovementGenerator::new();
        let mut snap = snapshot();
        snap.move_spline_finalized = true;
        assert_eq!(
            home.update_like_cpp(true, snap),
            HomeMovementAction::Finished
        );
        assert!(home.has_flag(MovementGeneratorFlags::INFORM_ENABLED));

        let mut home = HomeMovementGenerator::new();
        assert_eq!(
            home.update_like_cpp(true, snapshot()),
            HomeMovementAction::Continue
        );
    }

    #[test]
    fn home_deactivate_and_finalize_match_cpp_state_cleanup() {
        let mut home = HomeMovementGenerator::new();
        assert_eq!(
            home.deactivate_like_cpp(),
            HomeFinalizeAction {
                clear_unit_state_mask: UNIT_STATE_ROAMING_MOVE_LIKE_CPP,
                remove_can_swim_flag: false,
                set_spawn_health: false,
                load_creatures_addon: false,
                load_creatures_sparring_health: false,
                reset_vehicle: false,
                just_reached_home: false,
            }
        );
        assert!(home.has_flag(MovementGeneratorFlags::DEACTIVATED));

        home.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
        let mut snap = snapshot();
        snap.is_vehicle = true;
        assert_eq!(
            home.finalize_like_cpp(true, true, snap),
            HomeFinalizeAction {
                clear_unit_state_mask: HOME_CLEAR_ON_FINALIZE_MASK_LIKE_CPP,
                remove_can_swim_flag: true,
                set_spawn_health: true,
                load_creatures_addon: true,
                load_creatures_sparring_health: true,
                reset_vehicle: true,
                just_reached_home: true,
            }
        );
    }

    #[test]
    fn home_finalize_does_not_inform_without_flag_or_when_inactive() {
        let mut home = HomeMovementGenerator::new();
        assert_eq!(
            home.finalize_like_cpp(false, true, snapshot()),
            HomeFinalizeAction {
                clear_unit_state_mask: 0,
                remove_can_swim_flag: false,
                set_spawn_health: false,
                load_creatures_addon: false,
                load_creatures_sparring_health: false,
                reset_vehicle: false,
                just_reached_home: false,
            }
        );

        let mut home = HomeMovementGenerator::new();
        let mut snap = snapshot();
        snap.can_swim_out_of_combat = true;
        home.add_flag(MovementGeneratorFlags::INFORM_ENABLED);
        assert_eq!(
            home.finalize_like_cpp(true, true, snap),
            HomeFinalizeAction {
                clear_unit_state_mask: HOME_CLEAR_ON_FINALIZE_MASK_LIKE_CPP,
                remove_can_swim_flag: false,
                set_spawn_health: true,
                load_creatures_addon: true,
                load_creatures_sparring_health: true,
                reset_vehicle: false,
                just_reached_home: true,
            }
        );
    }
}
