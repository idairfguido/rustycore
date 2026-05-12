pub mod defines;
pub mod generator;
pub mod generators;
pub mod motion_master;
pub mod spline;

pub use defines::{
    CONTACT_DISTANCE_LIKE_CPP, ChaseAngle, ChaseRange, JumpArrivalCastArgs, JumpChargeParams,
    JumpChargeSpec, RotateDirection, normalize_orientation_like_cpp,
};
pub use generator::{
    MovementGenerator, MovementGeneratorFlags, MovementGeneratorMode, MovementGeneratorPriority,
    MovementGeneratorState, MovementGeneratorType, MovementSlot,
};
pub use generators::{
    AssistanceDistractFinalizeAction, AssistanceDistractMovementGenerator, DistractFacingSpline,
    DistractFinalizeAction, DistractInitializeAction, DistractMovementGenerator,
    IdleMovementGenerator, RotateFacingSpline, RotateMovementGenerator, RotateMovementInform,
    RotateMovementUpdate, UNIT_STATE_DISTRACTED_LIKE_CPP, UNIT_STATE_ROTATING_LIKE_CPP,
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
