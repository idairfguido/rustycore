pub mod spline;

pub use spline::{
    AnimTierTransition, FacingInfo, MonsterMovePathData, MonsterMoveType, MoveSpline,
    MoveSplineFlag, MoveSplineInitArgs, MoveSplineValidationError, SpellEffectExtraData,
    SplineUpdateResult, compute_fall_elevation, compute_fall_time,
};
