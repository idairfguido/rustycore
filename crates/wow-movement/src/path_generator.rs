use bitflags::bitflags;
use wow_core::Position;

pub const MAX_PATH_LENGTH_LIKE_CPP: usize = 74;
pub const MAX_POINT_PATH_LENGTH_LIKE_CPP: usize = 74;
pub const SMOOTH_PATH_STEP_SIZE_LIKE_CPP: f32 = 4.0;
pub const SMOOTH_PATH_SLOP_LIKE_CPP: f32 = 0.3;
pub const VERTEX_SIZE_LIKE_CPP: usize = 3;
pub const INVALID_POLYREF_LIKE_CPP: u64 = 0;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct PathType: u8 {
        const BLANK = 0x00;
        const NORMAL = 0x01;
        const SHORTCUT = 0x02;
        const INCOMPLETE = 0x04;
        const NOPATH = 0x08;
        const NOT_USING_PATH = 0x10;
        const SHORT = 0x20;
        const FARFROMPOLY_START = 0x40;
        const FARFROMPOLY_END = 0x80;
        const FARFROMPOLY = Self::FARFROMPOLY_START.bits() | Self::FARFROMPOLY_END.bits();
    }
}

#[derive(Debug, Clone)]
pub struct PathGenerator {
    path_poly_refs: [u64; MAX_PATH_LENGTH_LIKE_CPP],
    poly_length: usize,
    path_points: Vec<Position>,
    path_type: PathType,
    use_straight_path: bool,
    force_destination: bool,
    point_path_limit: usize,
    use_raycast: bool,
    start_position: Position,
    end_position: Position,
    actual_end_position: Position,
}

impl Default for PathGenerator {
    fn default() -> Self {
        Self {
            path_poly_refs: [INVALID_POLYREF_LIKE_CPP; MAX_PATH_LENGTH_LIKE_CPP],
            poly_length: 0,
            path_points: Vec::new(),
            path_type: PathType::BLANK,
            use_straight_path: false,
            force_destination: false,
            point_path_limit: MAX_POINT_PATH_LENGTH_LIKE_CPP,
            use_raycast: false,
            start_position: Position::ZERO,
            end_position: Position::ZERO,
            actual_end_position: Position::ZERO,
        }
    }
}

impl PathGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn path_type(&self) -> PathType {
        self.path_type
    }

    #[must_use]
    pub fn path_points(&self) -> &[Position] {
        &self.path_points
    }

    #[must_use]
    pub fn start_position(&self) -> Position {
        self.start_position
    }

    #[must_use]
    pub fn end_position(&self) -> Position {
        self.end_position
    }

    #[must_use]
    pub fn actual_end_position(&self) -> Position {
        self.actual_end_position
    }

    #[must_use]
    pub fn poly_length(&self) -> usize {
        self.poly_length
    }

    #[must_use]
    pub fn point_path_limit(&self) -> usize {
        self.point_path_limit
    }

    #[must_use]
    pub fn use_straight_path(&self) -> bool {
        self.use_straight_path
    }

    #[must_use]
    pub fn use_raycast(&self) -> bool {
        self.use_raycast
    }

    #[must_use]
    pub fn force_destination(&self) -> bool {
        self.force_destination
    }

    pub fn set_use_straight_path(&mut self, use_straight_path: bool) {
        self.use_straight_path = use_straight_path;
    }

    pub fn set_use_raycast(&mut self, use_raycast: bool) {
        self.use_raycast = use_raycast;
    }

    pub fn set_path_length_limit(&mut self, distance: f32) {
        let point_limit = if distance.is_sign_negative() {
            0
        } else {
            (distance / SMOOTH_PATH_STEP_SIZE_LIKE_CPP) as usize
        };
        self.point_path_limit = point_limit.min(MAX_POINT_PATH_LENGTH_LIKE_CPP);
    }

    pub fn calculate_without_navmesh_like_cpp(
        &mut self,
        start: Position,
        destination: Position,
        force_destination: bool,
    ) -> bool {
        if !start.is_valid_map_coord_like_cpp() || !destination.is_valid_map_coord_like_cpp() {
            return false;
        }

        self.set_start_position(start);
        self.set_end_position(destination);
        self.force_destination = force_destination;
        self.build_shortcut();
        self.path_type = PathType::NORMAL | PathType::NOT_USING_PATH;
        true
    }

    pub fn apply_detour_path_like_cpp(
        &mut self,
        start: Position,
        destination: Position,
        actual_end: Position,
        points: impl IntoIterator<Item = Position>,
        poly_refs: &[u64],
        path_type: PathType,
        force_destination: bool,
    ) {
        self.clear();
        self.set_start_position(start);
        self.end_position = destination;
        self.actual_end_position = actual_end;
        self.force_destination = force_destination;
        self.path_type = path_type;

        for (index, poly_ref) in poly_refs
            .iter()
            .copied()
            .take(MAX_PATH_LENGTH_LIKE_CPP)
            .enumerate()
        {
            self.path_poly_refs[index] = poly_ref;
            self.poly_length += 1;
        }

        self.path_points.extend(points);
    }

    #[must_use]
    pub fn path_length(&self) -> f32 {
        self.path_points
            .windows(2)
            .map(|points| distance_3d(points[0], points[1]))
            .sum()
    }

    pub fn shorten_path_until_dist_like_cpp(
        &mut self,
        target: Position,
        dist: f32,
        mut has_line_of_sight_from: impl FnMut(Position) -> bool,
    ) -> bool {
        if self.path_type == PathType::BLANK || self.path_points.len() < 2 {
            return false;
        }

        let dist_sq = dist * dist;
        if distance_3d_sq(self.path_points[0], target) < dist_sq {
            return false;
        }

        let last = self.path_points[self.path_points.len() - 1];
        if distance_3d_sq(last, target) >= dist_sq {
            return false;
        }

        let mut index = self.path_points.len() - 1;
        loop {
            if distance_3d_sq(self.path_points[index - 1], target) >= dist_sq {
                break;
            }

            if !has_line_of_sight_from(self.path_points[index - 1]) {
                self.path_points.truncate(index + 1);
                return true;
            }

            index -= 1;
            if index == 0 {
                self.path_points[0] = self.path_points[1];
                self.path_points.truncate(2);
                return true;
            }
        }

        let current = self.path_points[index];
        let previous = self.path_points[index - 1];
        let current_to_target = distance_3d(current, target);
        let offset = dist - current_to_target;
        self.path_points[index] = move_towards(current, previous, offset);
        self.path_points.truncate(index + 1);
        true
    }

    pub fn add_far_from_poly_flags(&mut self, start_far_from_poly: bool, end_far_from_poly: bool) {
        if start_far_from_poly {
            self.path_type.insert(PathType::FARFROMPOLY_START);
        }
        if end_far_from_poly {
            self.path_type.insert(PathType::FARFROMPOLY_END);
        }
    }

    pub fn normalize_path_like_cpp(&mut self, mut update_allowed_z: impl FnMut(Position) -> f32) {
        for point in &mut self.path_points {
            point.z = update_allowed_z(*point);
        }
    }

    #[must_use]
    pub fn is_invalid_destination_z_like_cpp(&self, target: Position) -> bool {
        (target.z - self.actual_end_position.z) > 5.0
    }

    pub fn clear(&mut self) {
        self.poly_length = 0;
        self.path_poly_refs = [INVALID_POLYREF_LIKE_CPP; MAX_PATH_LENGTH_LIKE_CPP];
        self.path_points.clear();
    }

    fn set_start_position(&mut self, point: Position) {
        self.start_position = point;
    }

    fn set_end_position(&mut self, point: Position) {
        self.actual_end_position = point;
        self.end_position = point;
    }

    fn build_shortcut(&mut self) {
        self.clear();
        self.path_points
            .extend([self.start_position, self.actual_end_position]);
        self.path_type = PathType::SHORTCUT;
    }
}

#[must_use]
pub fn in_range_like_cpp(first: Position, second: Position, range: f32, height: f32) -> bool {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    let dz = first.z - second.z;
    (dx * dx + dy * dy) < range * range && dz.abs() < height
}

#[must_use]
pub fn in_range_yzx_like_cpp(
    first: [f32; VERTEX_SIZE_LIKE_CPP],
    second: [f32; VERTEX_SIZE_LIKE_CPP],
    range: f32,
    height: f32,
) -> bool {
    let dx = second[0] - first[0];
    let dy = second[1] - first[1];
    let dz = second[2] - first[2];
    (dx * dx + dz * dz) < range * range && dy.abs() < height
}

#[must_use]
pub fn distance_3d_sq(left: Position, right: Position) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    let dz = left.z - right.z;
    dx * dx + dy * dy + dz * dz
}

#[must_use]
pub fn distance_3d(left: Position, right: Position) -> f32 {
    distance_3d_sq(left, right).sqrt()
}

fn move_towards(from: Position, to: Position, distance: f32) -> Position {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dz = to.z - from.z;
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length <= f32::EPSILON {
        return from;
    }

    let scale = distance / length;
    Position::new(
        from.x + dx * scale,
        from.y + dy * scale,
        from.z + dz * scale,
        from.orientation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: f32, y: f32, z: f32) -> Position {
        Position::xyz(x, y, z)
    }

    #[test]
    fn path_type_and_constants_match_cpp() {
        assert_eq!(MAX_PATH_LENGTH_LIKE_CPP, 74);
        assert_eq!(MAX_POINT_PATH_LENGTH_LIKE_CPP, 74);
        assert_eq!(SMOOTH_PATH_STEP_SIZE_LIKE_CPP, 4.0);
        assert_eq!(SMOOTH_PATH_SLOP_LIKE_CPP, 0.3);
        assert_eq!(VERTEX_SIZE_LIKE_CPP, 3);
        assert_eq!(INVALID_POLYREF_LIKE_CPP, 0);

        assert_eq!(PathType::BLANK.bits(), 0x00);
        assert_eq!(PathType::NORMAL.bits(), 0x01);
        assert_eq!(PathType::SHORTCUT.bits(), 0x02);
        assert_eq!(PathType::INCOMPLETE.bits(), 0x04);
        assert_eq!(PathType::NOPATH.bits(), 0x08);
        assert_eq!(PathType::NOT_USING_PATH.bits(), 0x10);
        assert_eq!(PathType::SHORT.bits(), 0x20);
        assert_eq!(PathType::FARFROMPOLY_START.bits(), 0x40);
        assert_eq!(PathType::FARFROMPOLY_END.bits(), 0x80);
        assert_eq!(
            PathType::FARFROMPOLY,
            PathType::FARFROMPOLY_START | PathType::FARFROMPOLY_END
        );
    }

    #[test]
    fn options_and_geometry_helpers_match_cpp_shape() {
        let mut path = PathGenerator::new();
        assert_eq!(path.point_path_limit(), MAX_POINT_PATH_LENGTH_LIKE_CPP);
        path.set_path_length_limit(50.0);
        assert_eq!(path.point_path_limit(), 12);
        path.set_path_length_limit(1_000.0);
        assert_eq!(path.point_path_limit(), MAX_POINT_PATH_LENGTH_LIKE_CPP);
        path.set_use_straight_path(true);
        path.set_use_raycast(true);
        assert!(path.use_straight_path());
        assert!(path.use_raycast());

        assert!(in_range_like_cpp(
            pos(0.0, 0.0, 0.0),
            pos(3.0, 4.0, 0.5),
            6.0,
            1.0
        ));
        assert!(!in_range_like_cpp(
            pos(0.0, 0.0, 0.0),
            pos(3.0, 4.0, 2.0),
            6.0,
            1.0
        ));
        assert!(in_range_yzx_like_cpp(
            [0.0, 0.5, 0.0],
            [3.0, 0.0, 4.0],
            6.0,
            1.0
        ));
        assert_eq!(distance_3d_sq(pos(0.0, 0.0, 0.0), pos(1.0, 2.0, 2.0)), 9.0);
    }

    #[test]
    fn calculate_without_navmesh_builds_cpp_shortcut_degraded_path() {
        let mut path = PathGenerator::new();
        assert!(path.calculate_without_navmesh_like_cpp(
            pos(1.0, 2.0, 3.0),
            pos(4.0, 6.0, 3.0),
            false
        ));

        assert_eq!(path.start_position(), pos(1.0, 2.0, 3.0));
        assert_eq!(path.end_position(), pos(4.0, 6.0, 3.0));
        assert_eq!(path.actual_end_position(), pos(4.0, 6.0, 3.0));
        assert_eq!(
            path.path_type(),
            PathType::NORMAL | PathType::NOT_USING_PATH
        );
        assert_eq!(
            path.path_points(),
            &[pos(1.0, 2.0, 3.0), pos(4.0, 6.0, 3.0)]
        );
        assert_eq!(path.path_length(), 5.0);
        assert_eq!(path.poly_length(), 0);
    }

    #[test]
    fn calculate_without_navmesh_rejects_invalid_coords_like_cpp() {
        let mut path = PathGenerator::new();
        assert!(!path.calculate_without_navmesh_like_cpp(
            pos(0.0, 0.0, 0.0),
            Position::new(f32::NAN, 0.0, 0.0, 0.0),
            false
        ));
        assert_eq!(path.path_type(), PathType::BLANK);
        assert!(path.path_points().is_empty());
    }

    #[test]
    fn apply_detour_path_writes_cpp_pathgenerator_state() {
        let mut path = PathGenerator::new();
        path.calculate_without_navmesh_like_cpp(pos(9.0, 9.0, 9.0), pos(10.0, 10.0, 10.0), false);

        path.apply_detour_path_like_cpp(
            pos(1.0, 2.0, 3.0),
            pos(7.0, 8.0, 9.0),
            pos(7.0, 8.0, 9.5),
            [pos(1.0, 2.0, 3.0), pos(4.0, 5.0, 6.0), pos(7.0, 8.0, 9.5)],
            &[11, 22],
            PathType::NORMAL,
            true,
        );

        assert_eq!(path.start_position(), pos(1.0, 2.0, 3.0));
        assert_eq!(path.end_position(), pos(7.0, 8.0, 9.0));
        assert_eq!(path.actual_end_position(), pos(7.0, 8.0, 9.5));
        assert_eq!(path.path_type(), PathType::NORMAL);
        assert!(path.force_destination());
        assert_eq!(path.poly_length(), 2);
        assert_eq!(
            path.path_points(),
            &[pos(1.0, 2.0, 3.0), pos(4.0, 5.0, 6.0), pos(7.0, 8.0, 9.5)]
        );
    }

    #[test]
    fn add_far_from_poly_flags_sets_bitmask_like_cpp() {
        let mut path = PathGenerator::new();
        path.calculate_without_navmesh_like_cpp(pos(0.0, 0.0, 0.0), pos(1.0, 0.0, 0.0), false);
        path.add_far_from_poly_flags(true, false);
        assert!(path.path_type().contains(PathType::FARFROMPOLY_START));
        assert!(!path.path_type().contains(PathType::FARFROMPOLY_END));
        path.add_far_from_poly_flags(false, true);
        assert!(path.path_type().contains(PathType::FARFROMPOLY));
    }

    #[test]
    fn normalize_path_and_invalid_destination_z_match_cpp_shape() {
        let mut path = PathGenerator::new();
        path.calculate_without_navmesh_like_cpp(pos(0.0, 0.0, 10.0), pos(5.0, 0.0, 20.0), false);

        path.normalize_path_like_cpp(|point| if point.x < 1.0 { 7.0 } else { 12.0 });
        assert_eq!(
            path.path_points(),
            &[pos(0.0, 0.0, 7.0), pos(5.0, 0.0, 12.0)]
        );

        assert!(!path.is_invalid_destination_z_like_cpp(pos(5.0, 0.0, 25.0)));
        assert!(path.is_invalid_destination_z_like_cpp(pos(5.0, 0.0, 25.1)));
        assert!(!path.is_invalid_destination_z_like_cpp(pos(5.0, 0.0, 1.0)));
    }

    #[test]
    fn shorten_path_until_dist_matches_cpp_segment_trim_shape() {
        let mut path = PathGenerator::new();
        path.calculate_without_navmesh_like_cpp(pos(0.0, 0.0, 0.0), pos(10.0, 0.0, 0.0), false);
        path.path_points = vec![pos(0.0, 0.0, 0.0), pos(5.0, 0.0, 0.0), pos(10.0, 0.0, 0.0)];

        assert!(path.shorten_path_until_dist_like_cpp(pos(10.0, 0.0, 0.0), 3.0, |_| true));

        assert_eq!(
            path.path_points(),
            &[pos(0.0, 0.0, 0.0), pos(5.0, 0.0, 0.0), pos(7.0, 0.0, 0.0)]
        );
    }

    #[test]
    fn shorten_path_until_dist_preserves_last_los_valid_path_like_cpp() {
        let mut path = PathGenerator::new();
        path.calculate_without_navmesh_like_cpp(pos(0.0, 0.0, 0.0), pos(10.0, 0.0, 0.0), false);
        path.path_points = vec![
            pos(0.0, 0.0, 0.0),
            pos(5.0, 0.0, 0.0),
            pos(8.0, 0.0, 0.0),
            pos(10.0, 0.0, 0.0),
        ];

        assert!(path.shorten_path_until_dist_like_cpp(pos(10.0, 0.0, 0.0), 3.0, |_| false));

        assert_eq!(
            path.path_points(),
            &[
                pos(0.0, 0.0, 0.0),
                pos(5.0, 0.0, 0.0),
                pos(8.0, 0.0, 0.0),
                pos(10.0, 0.0, 0.0)
            ]
        );
    }
}
