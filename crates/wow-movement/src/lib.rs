pub mod spline;

pub use spline::{
    FacingInfo, MonsterMoveType, MoveSpline, MoveSplineFlag, MoveSplineInitArgs,
    MoveSplineValidationError, SpellEffectExtraData, SplineUpdateResult, compute_fall_elevation,
    compute_fall_time,
};
