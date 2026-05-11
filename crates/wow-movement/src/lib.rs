pub mod spline;

pub use spline::{
    AnimTierTransition, FacingInfo, MonsterMovePathData, MonsterMoveType, MoveSpline,
    MoveSplineFlag, MoveSplineInit, MoveSplineInitArgs, MoveSplineLaunchError,
    MoveSplineLaunchInput, MoveSplineLaunchResult, MoveSplineStopInput, MoveSplineStopResult,
    MoveSplineValidationError, SpellEffectExtraData, SplineUpdateResult, compute_fall_elevation,
    compute_fall_time,
};
