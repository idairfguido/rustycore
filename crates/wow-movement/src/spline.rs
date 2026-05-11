// C++ movement code stores durations/curve params as int32/float and casts between them.
#![allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]

use std::f32::consts::PI;

use bitflags::bitflags;
use wow_core::{ObjectGuid, Position};

pub const GRAVITY_LIKE_CPP: f32 = 19.291_105;
const TERMINAL_VELOCITY_LIKE_CPP: f32 = 60.148_003;
const TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP: f32 = 7.0;
const MINIMAL_DURATION_MS_LIKE_CPP: i32 = 1;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MoveSplineFlag: u32 {
        const NONE = 0x0000_0000;
        const UNKNOWN_0X1 = 0x0000_0001;
        const UNKNOWN_0X2 = 0x0000_0002;
        const UNKNOWN_0X4 = 0x0000_0004;
        const UNKNOWN_0X8 = 0x0000_0008;
        const FALLING_SLOW = 0x0000_0010;
        const DONE = 0x0000_0020;
        const FALLING = 0x0000_0040;
        const NO_SPLINE = 0x0000_0080;
        const UNKNOWN_0X100 = 0x0000_0100;
        const FLYING = 0x0000_0200;
        const ORIENTATION_FIXED = 0x0000_0400;
        const CATMULLROM = 0x0000_0800;
        const CYCLIC = 0x0000_1000;
        const ENTER_CYCLE = 0x0000_2000;
        const FROZEN = 0x0000_4000;
        const TRANSPORT_ENTER = 0x0000_8000;
        const TRANSPORT_EXIT = 0x0001_0000;
        const UNKNOWN_0X20000 = 0x0002_0000;
        const UNKNOWN_0X40000 = 0x0004_0000;
        const BACKWARD = 0x0008_0000;
        const SMOOTH_GROUND_PATH = 0x0010_0000;
        const CAN_SWIM = 0x0020_0000;
        const UNCOMPRESSED_PATH = 0x0040_0000;
        const UNKNOWN_0X800000 = 0x0080_0000;
        const UNKNOWN_0X1000000 = 0x0100_0000;
        const ANIMATION = 0x0200_0000;
        const PARABOLIC = 0x0400_0000;
        const FADE_OBJECT = 0x0800_0000;
        const STEERING = 0x1000_0000;
        const UNLIMITED_SPEED = 0x2000_0000;
        const UNKNOWN_0X40000000 = 0x4000_0000;
        const UNKNOWN_0X80000000 = 0x8000_0000;

        const MASK_NO_MONSTER_MOVE = Self::DONE.bits();
        const MASK_UNUSED = Self::NO_SPLINE.bits()
            | Self::ENTER_CYCLE.bits()
            | Self::FROZEN.bits()
            | Self::UNKNOWN_0X8.bits()
            | Self::UNKNOWN_0X100.bits()
            | Self::UNKNOWN_0X20000.bits()
            | Self::UNKNOWN_0X40000.bits()
            | Self::UNKNOWN_0X800000.bits()
            | Self::UNKNOWN_0X1000000.bits()
            | Self::FADE_OBJECT.bits()
            | Self::STEERING.bits()
            | Self::UNLIMITED_SPEED.bits()
            | Self::UNKNOWN_0X40000000.bits()
            | Self::UNKNOWN_0X80000000.bits();
    }
}

impl MoveSplineFlag {
    #[must_use]
    pub const fn is_smooth(self) -> bool {
        self.contains(Self::CATMULLROM)
    }

    #[must_use]
    pub const fn is_linear(self) -> bool {
        !self.is_smooth()
    }

    pub fn enable_animation(&mut self) {
        *self = (*self
            & !(Self::FALLING | Self::PARABOLIC | Self::FALLING_SLOW | Self::FADE_OBJECT))
            | Self::ANIMATION;
    }

    pub fn enable_parabolic(&mut self) {
        *self = (*self
            & !(Self::FALLING | Self::ANIMATION | Self::FALLING_SLOW | Self::FADE_OBJECT))
            | Self::PARABOLIC;
    }

    pub fn enable_flying(&mut self) {
        *self = (*self & !Self::FALLING) | Self::FLYING;
    }

    pub fn enable_falling(&mut self) {
        *self = (*self & !(Self::PARABOLIC | Self::ANIMATION | Self::FLYING)) | Self::FALLING;
    }

    pub fn enable_catmull_rom(&mut self) {
        *self = (*self & !Self::SMOOTH_GROUND_PATH) | Self::CATMULLROM;
    }

    pub fn enable_transport_enter(&mut self) {
        *self = (*self & !Self::TRANSPORT_EXIT) | Self::TRANSPORT_ENTER;
    }

    pub fn enable_transport_exit(&mut self) {
        *self = (*self & !Self::TRANSPORT_ENTER) | Self::TRANSPORT_EXIT;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MonsterMoveType {
    #[default]
    Normal = 0,
    FacingSpot = 1,
    FacingTarget = 2,
    FacingAngle = 3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacingInfo {
    pub spot: Position,
    pub target: ObjectGuid,
    pub angle: f32,
    pub kind: MonsterMoveType,
}

impl Default for FacingInfo {
    fn default() -> Self {
        Self {
            spot: Position::ZERO,
            target: ObjectGuid::EMPTY,
            angle: 0.0,
            kind: MonsterMoveType::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpellEffectExtraData {
    pub target: ObjectGuid,
    pub spell_visual_id: u32,
    pub progress_curve_id: u32,
    pub parabolic_curve_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimTierTransition {
    pub tier_transition_id: u32,
    pub anim_tier: u8,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MonsterMovePathData {
    pub points: Vec<Position>,
    pub packed_deltas: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveSplineInitArgs {
    pub path: Vec<Position>,
    pub facing: FacingInfo,
    pub flags: MoveSplineFlag,
    pub path_idx_offset: i32,
    pub velocity: f32,
    pub parabolic_amplitude: f32,
    pub vertical_acceleration: f32,
    pub effect_start_time_percent: f32,
    pub effect_start_time_ms: i32,
    pub spline_id: u32,
    pub initial_orientation: f32,
    pub spell_effect_extra: Option<SpellEffectExtraData>,
    pub anim_tier: Option<AnimTierTransition>,
    pub walk: bool,
    pub has_velocity: bool,
    pub transform_for_transport: bool,
}

impl Default for MoveSplineInitArgs {
    fn default() -> Self {
        Self {
            path: Vec::with_capacity(16),
            facing: FacingInfo::default(),
            flags: MoveSplineFlag::empty(),
            path_idx_offset: 0,
            velocity: 0.0,
            parabolic_amplitude: 0.0,
            vertical_acceleration: 0.0,
            effect_start_time_percent: 0.0,
            effect_start_time_ms: 0,
            spline_id: 0,
            initial_orientation: 0.0,
            spell_effect_extra: None,
            anim_tier: None,
            walk: false,
            has_velocity: false,
            transform_for_transport: true,
        }
    }
}

impl MoveSplineInitArgs {
    #[must_use]
    pub fn with_capacity(path_capacity: usize) -> Self {
        Self {
            path: Vec::with_capacity(path_capacity),
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), MoveSplineValidationError> {
        if self.path.len() <= 1 {
            return Err(MoveSplineValidationError::PathTooShort);
        }
        if self.velocity < 0.01 {
            return Err(MoveSplineValidationError::VelocityTooLow);
        }
        if !(0.0..=1.0).contains(&self.effect_start_time_percent) {
            return Err(MoveSplineValidationError::EffectStartTimePercentOutOfRange);
        }
        self.check_path_lengths()?;
        Ok(())
    }

    fn check_path_lengths(&self) -> Result<(), MoveSplineValidationError> {
        if self.path.len() > 2 || self.facing.kind == MonsterMoveType::Normal {
            for pair in self.path.windows(2) {
                if distance_3d(pair[0], pair[1]) < 0.1 {
                    return Err(MoveSplineValidationError::PathSegmentTooShort);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveSplineValidationError {
    PathTooShort,
    VelocityTooLow,
    EffectStartTimePercentOutOfRange,
    PathSegmentTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplineUpdateResult {
    None = 0x01,
    Arrived = 0x02,
    NextCycle = 0x04,
    NextSegment = 0x08,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct SplineData {
    points: Vec<Position>,
    lengths: Vec<i32>,
    first: i32,
    last: i32,
    cyclic: bool,
    smooth: bool,
}

impl SplineData {
    fn is_empty(&self) -> bool {
        self.first == self.last
    }

    fn duration(&self) -> i32 {
        if self.lengths.is_empty() {
            0
        } else {
            self.lengths[self.last as usize]
        }
    }

    fn segment_duration(&self, index: i32) -> i32 {
        self.lengths[(index + 1) as usize] - self.lengths[index as usize]
    }

    fn point(&self, index: i32) -> Position {
        self.points[index as usize]
    }

    fn final_destination(&self) -> Option<Position> {
        (!self.is_empty()).then(|| self.point(self.last))
    }

    fn current_destination(&self, point_idx: i32) -> Option<Position> {
        (!self.is_empty()).then(|| self.point(point_idx + 1))
    }

    fn compute_index_in_bounds(&self, length: i32) -> i32 {
        let mut index = self.first;
        while index + 1 < self.last && self.lengths[(index + 1) as usize] < length {
            index += 1;
        }
        index
    }

    fn compute_index_percent(&self, t: f32) -> (i32, f32) {
        debug_assert!((0.0..=1.0).contains(&t));
        let length = (t * self.duration() as f32) as i32;
        let index = self.compute_index_in_bounds(length);
        let segment_duration = self.segment_duration(index);
        let u = if segment_duration > 0 {
            (length - self.lengths[index as usize]) as f32 / segment_duration as f32
        } else {
            1.0
        };
        (index, u)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveSpline {
    spline: SplineData,
    facing: FacingInfo,
    id: u32,
    flags: MoveSplineFlag,
    time_passed_ms: i32,
    vertical_acceleration: f32,
    initial_orientation: f32,
    effect_start_time_ms: i32,
    point_idx: i32,
    point_idx_offset: i32,
    velocity: f32,
    spell_effect_extra: Option<SpellEffectExtraData>,
    anim_tier: Option<AnimTierTransition>,
    pub on_transport: bool,
    pub spline_is_facing_only: bool,
}

impl Default for MoveSpline {
    fn default() -> Self {
        Self {
            spline: SplineData::default(),
            facing: FacingInfo::default(),
            id: 0,
            flags: MoveSplineFlag::DONE,
            time_passed_ms: 0,
            vertical_acceleration: 0.0,
            initial_orientation: 0.0,
            effect_start_time_ms: 0,
            point_idx: 0,
            point_idx_offset: 0,
            velocity: 0.0,
            spell_effect_extra: None,
            anim_tier: None,
            on_transport: false,
            spline_is_facing_only: false,
        }
    }
}

impl MoveSpline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(
        &mut self,
        args: &MoveSplineInitArgs,
    ) -> Result<(), MoveSplineValidationError> {
        self.flags = args.flags;
        self.facing = args.facing;
        self.id = args.spline_id;
        self.point_idx_offset = args.path_idx_offset;
        self.initial_orientation = args.initial_orientation;
        self.time_passed_ms = 0;
        self.vertical_acceleration = 0.0;
        self.effect_start_time_ms = 0;
        self.spell_effect_extra = args.spell_effect_extra;
        self.spline_is_facing_only = args.path.len() == 2
            && args.facing.kind != MonsterMoveType::Normal
            && distance_3d(args.path[0], args.path[1]) < 0.1;
        self.velocity = args.velocity;
        self.anim_tier = args.anim_tier;

        if args.flags.contains(MoveSplineFlag::DONE) {
            self.spline = SplineData::default();
            return Ok(());
        }

        args.validate()?;
        self.init_spline(args);

        if args.flags.intersects(
            MoveSplineFlag::PARABOLIC | MoveSplineFlag::ANIMATION | MoveSplineFlag::FADE_OBJECT,
        ) {
            let spline_duration = self.duration_ms();
            self.effect_start_time_ms = (spline_duration as f32 * args.effect_start_time_percent)
                as i32
                + args.effect_start_time_ms;
            if self.effect_start_time_ms > spline_duration {
                self.effect_start_time_ms = spline_duration;
            }

            if args.flags.contains(MoveSplineFlag::PARABOLIC)
                && self.effect_start_time_ms < spline_duration
            {
                if args.parabolic_amplitude != 0.0 {
                    let duration_sec = ms_to_sec(spline_duration - self.effect_start_time_ms);
                    self.vertical_acceleration =
                        args.parabolic_amplitude * 8.0 / (duration_sec * duration_sec);
                } else if args.vertical_acceleration != 0.0 {
                    self.vertical_acceleration = args.vertical_acceleration;
                }
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn initialized(&self) -> bool {
        !self.spline.is_empty()
    }

    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn flags(&self) -> MoveSplineFlag {
        self.flags
    }

    #[must_use]
    pub const fn facing(&self) -> FacingInfo {
        self.facing
    }

    #[must_use]
    pub const fn time_passed_ms(&self) -> i32 {
        self.time_passed_ms
    }

    #[must_use]
    pub fn duration_ms(&self) -> i32 {
        self.spline.duration()
    }

    #[must_use]
    pub const fn current_spline_index(&self) -> i32 {
        self.point_idx
    }

    #[must_use]
    pub const fn velocity(&self) -> f32 {
        self.velocity
    }

    #[must_use]
    pub const fn effect_start_time_ms(&self) -> i32 {
        self.effect_start_time_ms
    }

    #[must_use]
    pub const fn vertical_acceleration(&self) -> f32 {
        self.vertical_acceleration
    }

    #[must_use]
    pub const fn spell_effect_extra(&self) -> Option<SpellEffectExtraData> {
        self.spell_effect_extra
    }

    #[must_use]
    pub const fn anim_tier(&self) -> Option<AnimTierTransition> {
        self.anim_tier
    }

    #[must_use]
    pub fn finalized(&self) -> bool {
        self.flags.contains(MoveSplineFlag::DONE)
    }

    #[must_use]
    pub fn is_cyclic(&self) -> bool {
        self.flags.contains(MoveSplineFlag::CYCLIC)
    }

    #[must_use]
    pub fn final_destination(&self) -> Option<Position> {
        self.spline.final_destination()
    }

    #[must_use]
    pub fn monster_move_path_data(&self) -> MonsterMovePathData {
        let point_count = self.spline.points.len();
        if point_count < 3 {
            return MonsterMovePathData::default();
        }

        let last_idx = point_count - 3;
        if self.flags.contains(MoveSplineFlag::UNCOMPRESSED_PATH) {
            let mut points = Vec::new();
            if self.flags.contains(MoveSplineFlag::CYCLIC) {
                points.push(self.spline.points[1]);
                for index in 0..last_idx {
                    points.push(self.spline.points[index + 1]);
                }
            } else {
                for index in 0..last_idx {
                    points.push(self.spline.points[index + 2]);
                }
            }
            return MonsterMovePathData {
                points,
                packed_deltas: Vec::new(),
            };
        }

        let real_path = &self.spline.points[1..];
        let points = vec![real_path[last_idx]];

        let mut packed_deltas = Vec::new();
        if last_idx > 1 {
            let first = real_path[0];
            let last = real_path[last_idx];
            let middle = Position::xyz(
                first.x.midpoint(last.x),
                first.y.midpoint(last.y),
                first.z.midpoint(last.z),
            );

            for point in &real_path[1..last_idx] {
                packed_deltas.push([middle.x - point.x, middle.y - point.y, middle.z - point.z]);
            }
        }

        MonsterMovePathData {
            points,
            packed_deltas,
        }
    }

    #[must_use]
    pub fn current_destination(&self) -> Option<Position> {
        self.spline.current_destination(self.point_idx)
    }

    #[must_use]
    pub fn current_path_index(&self) -> i32 {
        let mut point = self.point_idx_offset + self.point_idx - self.spline.first
            + i32::from(self.finalized());
        if self.is_cyclic() {
            point %= self.spline.last - self.spline.first;
        }
        point
    }

    pub fn interrupt(&mut self) {
        self.flags.insert(MoveSplineFlag::DONE);
    }

    pub fn finalize(&mut self) {
        self.flags.insert(MoveSplineFlag::DONE);
        self.point_idx = self.spline.last - 1;
        self.time_passed_ms = self.duration_ms();
    }

    pub fn update_state(&mut self, mut diff_ms: i32) -> Vec<SplineUpdateResult> {
        let mut results = Vec::new();
        while diff_ms > 0 {
            let result = self.update_state_once(&mut diff_ms);
            results.push(result);
        }
        results
    }

    #[must_use]
    pub fn compute_position(&self) -> Option<Position> {
        self.compute_position_at(self.time_passed_ms, self.point_idx)
    }

    #[must_use]
    pub fn compute_position_offset(&self, time_offset_ms: i32) -> Option<Position> {
        let time_point = self.time_passed_ms + time_offset_ms;
        if time_point >= self.duration_ms() {
            return self.compute_position_at(self.duration_ms(), self.spline.last - 1);
        }
        if time_point <= 0 {
            return self.compute_position_at(0, self.spline.first);
        }

        let mut point_index = self.point_idx;
        while time_point >= self.spline.lengths[(point_index + 1) as usize] {
            point_index += 1;
        }
        while time_point < self.spline.lengths[point_index as usize] {
            point_index -= 1;
        }
        self.compute_position_at(time_point, point_index)
    }

    #[must_use]
    pub fn compute_position_percent(&self, t: f32) -> Option<Position> {
        if !(0.0..=1.0).contains(&t) || self.spline.is_empty() {
            return None;
        }
        let time_point = (t * self.duration_ms() as f32) as i32;
        let (point_index, u) = self.spline.compute_index_percent(t);
        self.compute_position_with_u(time_point, point_index, u)
    }

    fn update_state_once(&mut self, diff_ms: &mut i32) -> SplineUpdateResult {
        if self.finalized() {
            *diff_ms = 0;
            return SplineUpdateResult::Arrived;
        }

        let segment_time_elapsed =
            self.spline.lengths[(self.point_idx + 1) as usize] - self.time_passed_ms;
        let minimal_diff = (*diff_ms).min(segment_time_elapsed);
        self.time_passed_ms += minimal_diff;
        *diff_ms -= minimal_diff;

        if self.time_passed_ms >= self.spline.lengths[(self.point_idx + 1) as usize] {
            self.point_idx += 1;
            if self.point_idx < self.spline.last {
                return SplineUpdateResult::NextSegment;
            }
            if self.spline.cyclic {
                let old_duration = self.duration_ms();
                self.point_idx = self.spline.first;
                if old_duration > 0 {
                    self.time_passed_ms %= old_duration;
                }

                if self.flags.contains(MoveSplineFlag::ENTER_CYCLE) {
                    self.rebuild_enter_cycle_spline_preserving_duration(old_duration);
                }
                return SplineUpdateResult::NextCycle;
            }

            self.finalize();
            *diff_ms = 0;
            return SplineUpdateResult::Arrived;
        }

        SplineUpdateResult::None
    }

    fn init_spline(&mut self, args: &MoveSplineInitArgs) {
        let mut spline = if args.flags.contains(MoveSplineFlag::CYCLIC) {
            init_cyclic_catmull_storage(
                &args.path,
                args.flags.contains(MoveSplineFlag::CATMULLROM),
                usize::from(self.flags.contains(MoveSplineFlag::ENTER_CYCLE)),
                args.initial_orientation,
            )
        } else {
            init_catmull_storage(
                &args.path,
                args.flags.contains(MoveSplineFlag::CATMULLROM),
                args.initial_orientation,
            )
        };

        if self.flags.contains(MoveSplineFlag::FALLING) {
            let start_z = spline.point(spline.first).z;
            init_lengths(&mut spline, |spline, index| {
                (compute_fall_time(start_z - spline.point(index + 1).z, false) * 1000.0) as i32
            });
        } else {
            let velocity_inv = 1000.0 / args.velocity;
            let mut time = MINIMAL_DURATION_MS_LIKE_CPP;
            init_lengths(&mut spline, |spline, index| {
                time += (segment_length(spline, index) * velocity_inv) as i32;
                time
            });
        }

        if spline.duration() < MINIMAL_DURATION_MS_LIKE_CPP {
            let fallback = if spline.cyclic { 1000 } else { 1 };
            let last = spline.last as usize;
            spline.lengths[last] = fallback;
        }

        self.point_idx = spline.first;
        self.spline = spline;
    }

    fn compute_position_at(&self, time_point: i32, point_index: i32) -> Option<Position> {
        if self.spline.is_empty() {
            return None;
        }

        let segment_time = self.spline.segment_duration(point_index);
        let u = if segment_time > 0 {
            (time_point - self.spline.lengths[point_index as usize]) as f32 / segment_time as f32
        } else {
            1.0
        };

        self.compute_position_with_u(time_point, point_index, u)
    }

    fn compute_position_with_u(
        &self,
        time_point: i32,
        point_index: i32,
        u: f32,
    ) -> Option<Position> {
        if self.spline.is_empty() {
            return None;
        }

        let mut current = if self.spline.smooth {
            evaluate_catmullrom(&self.spline, point_index, u)
        } else {
            evaluate_linear(&self.spline, point_index, u)
        };
        current.orientation = self.initial_orientation;

        if self.flags.contains(MoveSplineFlag::ANIMATION) {
        } else if self.flags.contains(MoveSplineFlag::PARABOLIC) {
            self.compute_parabolic_elevation(time_point, &mut current.z);
        } else if self.flags.contains(MoveSplineFlag::FALLING) {
            self.compute_fall_elevation(time_point, &mut current.z);
        }

        if self.finalized() && self.facing.kind != MonsterMoveType::Normal {
            match self.facing.kind {
                MonsterMoveType::FacingAngle => current.orientation = self.facing.angle,
                MonsterMoveType::FacingSpot => {
                    current.orientation =
                        (self.facing.spot.y - current.y).atan2(self.facing.spot.x - current.x);
                }
                MonsterMoveType::Normal | MonsterMoveType::FacingTarget => {}
            }
        } else {
            if !self.flags.intersects(
                MoveSplineFlag::ORIENTATION_FIXED
                    | MoveSplineFlag::FALLING
                    | MoveSplineFlag::UNKNOWN_0X8,
            ) {
                let derivative = if self.spline.smooth {
                    evaluate_derivative_catmullrom(&self.spline, self.point_idx, u)
                } else {
                    evaluate_derivative_linear(&self.spline, self.point_idx)
                };
                if derivative.x != 0.0 || derivative.y != 0.0 {
                    current.orientation = derivative.y.atan2(derivative.x);
                }
            }
            if self.flags.contains(MoveSplineFlag::BACKWARD) {
                current.orientation -= PI;
            }
        }

        Some(current)
    }

    fn compute_parabolic_elevation(&self, time_point: i32, z: &mut f32) {
        if time_point <= self.effect_start_time_ms {
            return;
        }

        let time_passed = ms_to_sec(time_point - self.effect_start_time_ms);
        let duration = ms_to_sec(self.duration_ms() - self.effect_start_time_ms);
        *z += (duration - time_passed) * 0.5 * self.vertical_acceleration * time_passed;
    }

    fn compute_fall_elevation(&self, time_point: i32, z: &mut f32) {
        let Some(final_destination) = self.final_destination() else {
            return;
        };
        let z_now = self.spline.point(self.spline.first).z
            - compute_fall_elevation(ms_to_sec(time_point), false, 0.0);
        *z = z_now.max(final_destination.z);
    }

    fn rebuild_enter_cycle_spline_preserving_duration(&mut self, old_duration: i32) {
        self.flags.remove(MoveSplineFlag::ENTER_CYCLE);
        if old_duration <= 0 || self.spline.last <= self.spline.first + 2 {
            return;
        }

        let path: Vec<_> = ((self.spline.first + 1)..self.spline.last)
            .map(|index| self.spline.point(index))
            .collect();
        if path.len() <= 1 {
            return;
        }

        let mut args = MoveSplineInitArgs {
            path,
            facing: self.facing,
            flags: self.flags,
            path_idx_offset: self.point_idx_offset,
            velocity: 1.0,
            spline_id: self.id,
            initial_orientation: self.initial_orientation,
            spell_effect_extra: self.spell_effect_extra,
            anim_tier: self.anim_tier,
            has_velocity: true,
            transform_for_transport: self.on_transport,
            ..MoveSplineInitArgs::default()
        };

        if args.validate().is_err() {
            return;
        }

        let mut temp_spline = Self::new();
        if temp_spline.initialize(&args).is_err() {
            return;
        }

        args.velocity = temp_spline.duration_ms() as f32 / old_duration as f32;
        if args.validate().is_ok() {
            self.init_spline(&args);
        }
    }
}

#[must_use]
pub fn compute_fall_time(path_length: f32, is_safe_fall: bool) -> f32 {
    if path_length < 0.0 {
        return 0.0;
    }

    let terminal_safe_fall_length = TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP
        * TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP
        / (2.0 * GRAVITY_LIKE_CPP);
    let terminal_length =
        TERMINAL_VELOCITY_LIKE_CPP * TERMINAL_VELOCITY_LIKE_CPP / (2.0 * GRAVITY_LIKE_CPP);
    let terminal_safe_fall_time = TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP / GRAVITY_LIKE_CPP;
    let terminal_fall_time = TERMINAL_VELOCITY_LIKE_CPP / GRAVITY_LIKE_CPP;

    if is_safe_fall {
        if path_length >= terminal_safe_fall_length {
            (path_length - terminal_safe_fall_length) / TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP
                + terminal_safe_fall_time
        } else {
            (2.0 * path_length / GRAVITY_LIKE_CPP).sqrt()
        }
    } else if path_length >= terminal_length {
        (path_length - terminal_length) / TERMINAL_VELOCITY_LIKE_CPP + terminal_fall_time
    } else {
        (2.0 * path_length / GRAVITY_LIKE_CPP).sqrt()
    }
}

#[must_use]
pub fn compute_fall_elevation(t_passed: f32, is_safe_fall: bool, start_velocity: f32) -> f32 {
    let terminal_velocity = if is_safe_fall {
        TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP
    } else {
        TERMINAL_VELOCITY_LIKE_CPP
    };
    let start_velocity = start_velocity.min(terminal_velocity);
    let terminal_time = if is_safe_fall {
        TERMINAL_SAFE_FALL_VELOCITY_LIKE_CPP / GRAVITY_LIKE_CPP
    } else {
        TERMINAL_VELOCITY_LIKE_CPP / GRAVITY_LIKE_CPP
    } - start_velocity / GRAVITY_LIKE_CPP;

    if t_passed > terminal_time {
        terminal_velocity * (t_passed - terminal_time)
            + start_velocity * terminal_time
            + GRAVITY_LIKE_CPP * terminal_time * terminal_time * 0.5
    } else {
        t_passed * (start_velocity + t_passed * GRAVITY_LIKE_CPP * 0.5)
    }
}

fn init_catmull_storage(path: &[Position], smooth: bool, initial_orientation: f32) -> SplineData {
    let count = path.len();
    let mut points = vec![Position::ZERO; count + 2];
    points[1..=count].copy_from_slice(path);
    points[0] = offset_initial_virtual_point(path[0], initial_orientation);
    points[count + 1] = path[count - 1];
    SplineData {
        points,
        lengths: Vec::new(),
        first: 1,
        last: i32::try_from(count).expect("spline point count fits i32"),
        cyclic: false,
        smooth,
    }
}

fn init_cyclic_catmull_storage(
    path: &[Position],
    smooth: bool,
    cyclic_point: usize,
    initial_orientation: f32,
) -> SplineData {
    let count = path.len();
    let mut points = vec![Position::ZERO; count + 3];
    points[1..=count].copy_from_slice(path);
    points[0] = if cyclic_point == 0 {
        path[count - 1]
    } else {
        offset_initial_virtual_point(path[0], initial_orientation)
    };
    points[count + 1] = path[cyclic_point];
    points[count + 2] = path[cyclic_point + 1];
    SplineData {
        points,
        lengths: Vec::new(),
        first: 1,
        last: i32::try_from(count).expect("spline point count fits i32") + 1,
        cyclic: true,
        smooth,
    }
}

fn init_lengths<F>(spline: &mut SplineData, mut next_length: F)
where
    F: FnMut(&SplineData, i32) -> i32,
{
    spline.lengths.resize(spline.last as usize + 1, 0);
    let mut prev_length = 0;
    for index in spline.first..spline.last {
        let new_length = next_length(spline, index).max(prev_length);
        spline.lengths[(index + 1) as usize] = new_length;
        prev_length = new_length;
    }
}

fn offset_initial_virtual_point(position: Position, orientation: f32) -> Position {
    Position::new(
        position.x - orientation.cos(),
        position.y - orientation.sin(),
        position.z,
        position.orientation,
    )
}

fn segment_length(spline: &SplineData, index: i32) -> f32 {
    if spline.smooth {
        let mut current = spline.point(index);
        let mut length = 0.0;
        for step in 1..=3 {
            let next = evaluate_catmullrom(spline, index, step as f32 / 3.0);
            length += distance_3d(current, next);
            current = next;
        }
        length
    } else {
        distance_3d(spline.point(index), spline.point(index + 1))
    }
}

fn evaluate_linear(spline: &SplineData, index: i32, u: f32) -> Position {
    let start = spline.point(index);
    let end = spline.point(index + 1);
    Position::new(
        start.x + (end.x - start.x) * u,
        start.y + (end.y - start.y) * u,
        start.z + (end.z - start.z) * u,
        start.orientation,
    )
}

fn evaluate_derivative_linear(spline: &SplineData, index: i32) -> Position {
    let start = spline.point(index);
    let end = spline.point(index + 1);
    Position::xyz(end.x - start.x, end.y - start.y, end.z - start.z)
}

fn evaluate_catmullrom(spline: &SplineData, index: i32, t: f32) -> Position {
    let p0 = spline.point(index - 1);
    let p1 = spline.point(index);
    let p2 = spline.point(index + 1);
    let p3 = spline.point(index + 2);
    let t2 = t * t;
    let t3 = t2 * t;
    let x = 0.5
        * ((2.0 * p1.x)
            + (-p0.x + p2.x) * t
            + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
            + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
    let y = 0.5
        * ((2.0 * p1.y)
            + (-p0.y + p2.y) * t
            + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
            + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
    let z = 0.5
        * ((2.0 * p1.z)
            + (-p0.z + p2.z) * t
            + (2.0 * p0.z - 5.0 * p1.z + 4.0 * p2.z - p3.z) * t2
            + (-p0.z + 3.0 * p1.z - 3.0 * p2.z + p3.z) * t3);
    Position::xyz(x, y, z)
}

fn evaluate_derivative_catmullrom(spline: &SplineData, index: i32, t: f32) -> Position {
    let p0 = spline.point(index - 1);
    let p1 = spline.point(index);
    let p2 = spline.point(index + 1);
    let p3 = spline.point(index + 2);
    let t2 = t * t;
    let x = 0.5
        * ((-p0.x + p2.x)
            + 2.0 * (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t
            + 3.0 * (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t2);
    let y = 0.5
        * ((-p0.y + p2.y)
            + 2.0 * (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t
            + 3.0 * (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t2);
    let z = 0.5
        * ((-p0.z + p2.z)
            + 2.0 * (2.0 * p0.z - 5.0 * p1.z + 4.0 * p2.z - p3.z) * t
            + 3.0 * (-p0.z + 3.0 * p1.z - 3.0 * p2.z + p3.z) * t2);
    Position::xyz(x, y, z)
}

fn distance_3d(left: Position, right: Position) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn ms_to_sec(ms: i32) -> f32 {
    ms as f32 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_args() -> MoveSplineInitArgs {
        MoveSplineInitArgs {
            path: vec![Position::xyz(0.0, 0.0, 0.0), Position::xyz(10.0, 0.0, 0.0)],
            velocity: 5.0,
            spline_id: 42,
            ..MoveSplineInitArgs::default()
        }
    }

    #[test]
    fn flags_match_cpp_values_and_mutators() {
        assert_eq!(MoveSplineFlag::DONE.bits(), 0x0000_0020);
        assert_eq!(MoveSplineFlag::FALLING.bits(), 0x0000_0040);
        assert_eq!(MoveSplineFlag::CYCLIC.bits(), 0x0000_1000);
        assert_eq!(MoveSplineFlag::UNCOMPRESSED_PATH.bits(), 0x0040_0000);

        let mut flags = MoveSplineFlag::FALLING | MoveSplineFlag::FALLING_SLOW;
        flags.enable_parabolic();
        assert!(flags.contains(MoveSplineFlag::PARABOLIC));
        assert!(!flags.intersects(MoveSplineFlag::FALLING | MoveSplineFlag::FALLING_SLOW));

        flags.enable_transport_enter();
        flags.enable_transport_exit();
        assert!(flags.contains(MoveSplineFlag::TRANSPORT_EXIT));
        assert!(!flags.contains(MoveSplineFlag::TRANSPORT_ENTER));
    }

    #[test]
    fn fall_math_matches_cpp_constants() {
        assert!((compute_fall_time(10.0, false) - 1.018_208).abs() < 0.000_01);
        assert!((compute_fall_elevation(1.0, false, 0.0) - 9.645_553).abs() < 0.000_01);
        assert!(compute_fall_time(-1.0, false).abs() < f32::EPSILON);
    }

    #[test]
    fn init_args_validate_like_cpp() {
        let mut args = linear_args();
        assert_eq!(args.validate(), Ok(()));

        args.path[1] = Position::xyz(0.05, 0.0, 0.0);
        assert_eq!(
            args.validate(),
            Err(MoveSplineValidationError::PathSegmentTooShort)
        );

        args.facing.kind = MonsterMoveType::FacingAngle;
        assert_eq!(args.validate(), Ok(()));
    }

    #[test]
    fn linear_spline_duration_position_and_finalize_match_cpp_shape() {
        let args = linear_args();
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        assert!(spline.initialized());
        assert_eq!(spline.id(), 42);
        assert_eq!(spline.duration_ms(), 2001);
        assert_eq!(spline.current_path_index(), 0);
        assert_eq!(
            spline.compute_position().unwrap(),
            Position::new(0.0, 0.0, 0.0, 0.0)
        );

        let mid = spline.compute_position_offset(1000).unwrap();
        assert!((mid.x - 4.997_501).abs() < 0.000_1);
        assert!(mid.orientation.abs() < f32::EPSILON);

        assert_eq!(spline.update_state(2001), vec![SplineUpdateResult::Arrived]);
        assert!(spline.finalized());
        assert_eq!(spline.time_passed_ms(), 2001);
        assert_eq!(spline.current_path_index(), 1);
    }

    #[test]
    fn compute_position_percent_uses_cpp_spline_index_rules() {
        let args = MoveSplineInitArgs {
            path: vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(10.0, 10.0, 0.0),
            ],
            velocity: 10.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        let start = spline.compute_position_percent(0.0).unwrap();
        let mid = spline.compute_position_percent(0.5).unwrap();
        let end = spline.compute_position_percent(1.0).unwrap();

        assert!(start.x.abs() < f32::EPSILON);
        assert!(start.y.abs() < f32::EPSILON);
        assert!(mid.x > 9.9);
        assert!(mid.y.abs() < 0.1);
        assert!((end.x - 10.0).abs() < f32::EPSILON);
        assert!((end.y - 10.0).abs() < f32::EPSILON);
        assert!(spline.compute_position_percent(-0.1).is_none());
        assert!(spline.compute_position_percent(1.1).is_none());
    }

    #[test]
    fn cyclic_spline_wraps_without_finalizing() {
        let args = MoveSplineInitArgs {
            path: vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(10.0, 10.0, 0.0),
            ],
            flags: MoveSplineFlag::CYCLIC,
            velocity: 10.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();
        let duration = spline.duration_ms();

        let results = spline.update_state(duration + 1);
        assert!(results.contains(&SplineUpdateResult::NextCycle));
        assert!(!spline.finalized());
        assert_eq!(spline.current_spline_index(), 1);
    }

    #[test]
    fn parabolic_amplitude_uses_cpp_acceleration_formula() {
        let args = MoveSplineInitArgs {
            path: vec![Position::xyz(0.0, 0.0, 0.0), Position::xyz(10.0, 0.0, 0.0)],
            flags: MoveSplineFlag::PARABOLIC,
            velocity: 10.0,
            parabolic_amplitude: 4.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        let mid = spline
            .compute_position_offset(spline.duration_ms() / 2)
            .unwrap();
        assert!(mid.z > 3.9 && mid.z < 4.1);
    }

    #[test]
    fn animation_tier_transition_matches_cpp_effect_start_storage() {
        let mut flags = MoveSplineFlag::PARABOLIC | MoveSplineFlag::FALLING_SLOW;
        flags.enable_animation();
        let args = MoveSplineInitArgs {
            path: vec![Position::xyz(0.0, 0.0, 0.0), Position::xyz(10.0, 0.0, 0.0)],
            flags,
            velocity: 10.0,
            effect_start_time_ms: 250,
            anim_tier: Some(AnimTierTransition {
                tier_transition_id: 77,
                anim_tier: 3,
            }),
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        assert!(spline.flags().contains(MoveSplineFlag::ANIMATION));
        assert!(!spline.flags().intersects(
            MoveSplineFlag::PARABOLIC | MoveSplineFlag::FALLING | MoveSplineFlag::FALLING_SLOW
        ));
        assert_eq!(
            spline.anim_tier(),
            Some(AnimTierTransition {
                tier_transition_id: 77,
                anim_tier: 3
            })
        );
        assert_eq!(spline.effect_start_time_ms(), 250);
    }

    #[test]
    fn cyclic_enter_cycle_rewrites_path_and_preserves_duration_like_cpp() {
        let args = MoveSplineInitArgs {
            path: vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(10.0, 10.0, 0.0),
                Position::xyz(0.0, 10.0, 0.0),
            ],
            flags: MoveSplineFlag::CYCLIC | MoveSplineFlag::ENTER_CYCLE,
            velocity: 10.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();
        let old_duration = spline.duration_ms();
        let old_point_count = spline.spline.points.len();

        let results = spline.update_state(old_duration);

        assert_eq!(results.last(), Some(&SplineUpdateResult::NextCycle));
        assert!(!spline.flags().contains(MoveSplineFlag::ENTER_CYCLE));
        assert_eq!(spline.duration_ms(), old_duration);
        assert!(spline.spline.points.len() < old_point_count);
        assert_eq!(
            spline.spline.point(spline.spline.first),
            Position::xyz(10.0, 0.0, 0.0)
        );
        assert!(!spline.finalized());
    }

    #[test]
    fn monster_move_path_data_compresses_like_cpp_initialize_spline_data() {
        let args = MoveSplineInitArgs {
            path: vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(20.0, 0.0, 0.0),
                Position::xyz(30.0, 0.0, 0.0),
            ],
            velocity: 10.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        let path_data = spline.monster_move_path_data();

        assert_eq!(path_data.points, vec![Position::xyz(30.0, 0.0, 0.0)]);
        assert_eq!(
            path_data.packed_deltas,
            vec![[5.0, 0.0, 0.0], [-5.0, 0.0, 0.0]]
        );
    }

    #[test]
    fn monster_move_path_data_uncompressed_cyclic_matches_cpp_point_rules() {
        let args = MoveSplineInitArgs {
            path: vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(10.0, 10.0, 0.0),
            ],
            flags: MoveSplineFlag::CYCLIC | MoveSplineFlag::UNCOMPRESSED_PATH,
            velocity: 10.0,
            ..MoveSplineInitArgs::default()
        };
        let mut spline = MoveSpline::new();
        spline.initialize(&args).unwrap();

        let path_data = spline.monster_move_path_data();

        assert_eq!(
            path_data.points,
            vec![
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(0.0, 0.0, 0.0),
                Position::xyz(10.0, 0.0, 0.0),
                Position::xyz(10.0, 10.0, 0.0),
            ]
        );
        assert!(path_data.packed_deltas.is_empty());
    }
}
