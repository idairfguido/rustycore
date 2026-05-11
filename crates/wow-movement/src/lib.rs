pub mod spline;

pub use spline::{
    AnimTierTransition, FacingInfo, JumpSpeeds, MonsterMovePathData, MonsterMoveType, MoveSpline,
    MoveSplineFlag, MoveSplineInit, MoveSplineInitArgs, MoveSplineLaunchError,
    MoveSplineLaunchInput, MoveSplineLaunchResult, MoveSplineStopInput, MoveSplineStopResult,
    MoveSplineValidationError, SpellEffectExtraData, SplineUpdateResult,
    calculate_jump_speeds_like_cpp, compute_fall_elevation, compute_fall_time,
    compute_jump_max_height_like_cpp,
};
