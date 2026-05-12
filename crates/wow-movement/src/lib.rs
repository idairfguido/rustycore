pub mod defines;
pub mod generator;
pub mod generators;
pub mod motion_master;
pub mod spline;

pub use defines::{
    CONTACT_DISTANCE_LIKE_CPP, ChaseAngle, ChaseRange, JumpArrivalCastArgs, JumpChargeParams,
    JumpChargeSpec, MovementWalkRunSpeedSelectionMode, RotateDirection,
    normalize_orientation_like_cpp,
};
pub use generator::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType, MovementSlot,
};
pub use generators::{
    AbstractFollower, AbstractFollowerEvent, AssistanceDistractFinalizeAction,
    AssistanceDistractMovementGenerator, AssistanceMovementFinalize, AssistanceMovementGenerator,
    CREATURE_FAMILY_ASSISTANCE_DELAY_MS_LIKE_CPP, DistractFacingSpline, DistractFinalizeAction,
    DistractInitializeAction, DistractMovementGenerator, EVENT_CHARGE_LIKE_CPP,
    EVENT_CHARGE_PREPATH_LIKE_CPP, FOLLOW_CHECK_INTERVAL_MS_LIKE_CPP,
    FOLLOW_RANGE_TOLERANCE_LIKE_CPP, FollowFinalizeAction, FollowLaunchPlan, FollowMovementAction,
    FollowMovementGenerator, FollowMovementInform, FollowUnitSnapshot, GenericArrivalSpell,
    GenericMovementFinalize, GenericMovementGenerator, GenericMovementInform,
    GenericSplineInitializer, IdleMovementGenerator, PointMovementAction, PointMovementFinalize,
    PointMovementGenerator, PointMovementInform, PointMovementLaunch, RotateFacingSpline,
    RotateMovementGenerator, RotateMovementInform, RotateMovementUpdate,
    UNIT_STATE_DISTRACTED_LIKE_CPP, UNIT_STATE_FOLLOW_LIKE_CPP, UNIT_STATE_FOLLOW_MOVE_LIKE_CPP,
    UNIT_STATE_ROAMING_LIKE_CPP, UNIT_STATE_ROAMING_MOVE_LIKE_CPP, UNIT_STATE_ROTATING_LIKE_CPP,
    position_okay_like_cpp, selected_relative_angle_like_cpp,
};
pub use motion_master::{
    DelayedAction, DelayedActionQueue, MotionMaster, MotionMasterDelayedActionType,
    MotionMasterFlags, ResolvedDelayedAction,
};
pub use spline::{
    AnimTierTransition, FacingInfo, JumpSpeeds, MonsterMovePathData, MonsterMoveType, MoveSpline,
    MoveSplineFlag, MoveSplineInit, MoveSplineInitArgs, MoveSplineLaunchError,
    MoveSplineLaunchInput, MoveSplineLaunchResult, MoveSplineStopInput, MoveSplineStopResult,
    MoveSplineValidationError, SpellEffectExtraData, SplineUpdateResult,
    calculate_jump_speeds_like_cpp, compute_fall_elevation, compute_fall_time,
    compute_jump_max_height_like_cpp,
};
