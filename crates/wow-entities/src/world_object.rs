use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use wow_constants::{TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

use crate::EntityObject;

pub const MAPID_INVALID: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldLocation {
    map_id: u32,
    position: Position,
}

impl Default for WorldLocation {
    fn default() -> Self {
        Self::new(MAPID_INVALID, 0.0, 0.0, 0.0, 0.0)
    }
}

impl WorldLocation {
    pub fn new(map_id: u32, x: f32, y: f32, z: f32, orientation: f32) -> Self {
        Self {
            map_id,
            position: Position::new(x, y, z, normalize_orientation(orientation)),
        }
    }

    pub const fn map_id(&self) -> u32 {
        self.map_id
    }

    pub const fn position(&self) -> Position {
        self.position
    }

    pub fn world_relocate(&mut self, map_id: u32, position: Position) {
        self.map_id = map_id;
        self.position = normalized_position(position);
    }

    pub fn relocate(&mut self, position: Position) {
        self.position = normalized_position(position);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseShift {
    phases: BTreeSet<u32>,
}

impl PhaseShift {
    pub fn from_phases(phases: impl IntoIterator<Item = u32>) -> Self {
        Self {
            phases: phases.into_iter().collect(),
        }
    }

    pub fn insert(&mut self, phase_id: u32) {
        self.phases.insert(phase_id);
    }

    pub fn clear(&mut self) {
        self.phases.clear();
    }

    pub fn can_see(&self, other: &Self) -> bool {
        self.phases.is_empty()
            || other.phases.is_empty()
            || self.phases.iter().any(|phase| other.phases.contains(phase))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldObject {
    object: EntityObject,
    location: WorldLocation,
    instance_id: u32,
    has_current_map: bool,
    phase_shift: PhaseShift,
    suppressed_phase_shift: PhaseShift,
    db_phase: i32,
    name: String,
    is_active: bool,
    is_far_visible: bool,
    is_world_object: bool,
    zone_id: u32,
    area_id: u32,
    combat_reach: f32,
    current_cell: Option<(u32, u32)>,
}

impl WorldObject {
    pub fn new(is_world_object: bool, type_id: TypeId, type_mask: TypeMask) -> Self {
        Self {
            object: EntityObject::new(type_id, type_mask),
            location: WorldLocation::default(),
            instance_id: 0,
            has_current_map: false,
            phase_shift: PhaseShift::default(),
            suppressed_phase_shift: PhaseShift::default(),
            db_phase: 0,
            name: String::new(),
            is_active: false,
            is_far_visible: false,
            is_world_object,
            zone_id: 0,
            area_id: 0,
            combat_reach: 0.0,
            current_cell: None,
        }
    }

    pub const fn object(&self) -> &EntityObject {
        &self.object
    }

    pub fn object_mut(&mut self) -> &mut EntityObject {
        &mut self.object
    }

    pub const fn guid(&self) -> ObjectGuid {
        self.object.guid()
    }

    pub const fn map_id(&self) -> u32 {
        self.location.map_id()
    }

    pub const fn instance_id(&self) -> u32 {
        self.instance_id
    }

    pub const fn position(&self) -> Position {
        self.location.position()
    }

    pub fn relocate(&mut self, position: Position) {
        self.location.relocate(position);
    }

    pub fn world_relocate(&mut self, map_id: u32, position: Position) {
        self.location.world_relocate(map_id, position);
        self.object.bind_map(map_id, self.instance_id);
    }

    pub fn set_map(&mut self, map_id: u32, instance_id: u32) -> Result<(), MapBindingError> {
        if self.object.is_in_world() {
            return Err(MapBindingError::ObjectInWorld);
        }

        if self.has_current_map {
            if self.map_id() == map_id && self.instance_id == instance_id {
                return Ok(());
            }
            return Err(MapBindingError::AlreadyBound {
                old_map_id: self.map_id(),
                old_instance_id: self.instance_id,
                new_map_id: map_id,
                new_instance_id: instance_id,
            });
        }

        self.has_current_map = true;
        self.location.map_id = map_id;
        self.instance_id = instance_id;
        self.object.bind_map(map_id, instance_id);
        Ok(())
    }

    pub fn reset_map(&mut self) -> Result<(), MapBindingError> {
        if !self.has_current_map {
            return Err(MapBindingError::NoCurrentMap);
        }
        if self.object.is_in_world() {
            return Err(MapBindingError::ObjectInWorld);
        }

        self.has_current_map = false;
        self.current_cell = None;
        self.object.set_grid_presence(false);
        Ok(())
    }

    pub const fn has_current_map(&self) -> bool {
        self.has_current_map
    }

    pub const fn phase_shift(&self) -> &PhaseShift {
        &self.phase_shift
    }

    pub fn phase_shift_mut(&mut self) -> &mut PhaseShift {
        &mut self.phase_shift
    }

    pub const fn suppressed_phase_shift(&self) -> &PhaseShift {
        &self.suppressed_phase_shift
    }

    pub const fn db_phase(&self) -> i32 {
        self.db_phase
    }

    pub fn set_db_phase(&mut self, db_phase: i32) {
        self.db_phase = db_phase;
    }

    pub fn in_same_phase(&self, other: &Self) -> bool {
        self.phase_shift.can_see(&other.phase_shift)
    }

    pub fn set_current_cell(&mut self, cell_x: u32, cell_y: u32) {
        self.current_cell = Some((cell_x, cell_y));
        self.object.set_grid_presence(true);
    }

    pub fn clear_current_cell(&mut self) {
        self.current_cell = None;
        self.object.set_grid_presence(false);
    }

    pub const fn current_cell(&self) -> Option<(u32, u32)> {
        self.current_cell
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub const fn is_far_visible(&self) -> bool {
        self.is_far_visible
    }

    pub fn set_far_visible(&mut self, far_visible: bool) {
        self.is_far_visible = far_visible;
    }

    pub const fn is_world_object(&self) -> bool {
        self.is_world_object
    }

    pub const fn zone_id(&self) -> u32 {
        self.zone_id
    }

    pub const fn area_id(&self) -> u32 {
        self.area_id
    }

    pub fn set_zone_and_area(&mut self, zone_id: u32, area_id: u32) {
        self.zone_id = zone_id;
        self.area_id = area_id;
    }

    pub const fn combat_reach(&self) -> f32 {
        self.combat_reach
    }

    pub fn set_combat_reach(&mut self, combat_reach: f32) {
        self.combat_reach = combat_reach.max(0.0);
    }

    pub fn exact_distance(&self, other: &Self) -> f32 {
        self.position().distance(&other.position())
    }

    pub fn exact_distance_2d(&self, other: &Self) -> f32 {
        self.position().distance_2d(&other.position())
    }

    pub fn distance(&self, other: &Self) -> f32 {
        (self.exact_distance(other) - self.combat_reach - other.combat_reach).max(0.0)
    }

    pub fn distance_to_position(&self, position: Position) -> f32 {
        (self.position().distance(&position) - self.combat_reach).max(0.0)
    }

    pub fn distance_2d(&self, other: &Self) -> f32 {
        (self.exact_distance_2d(other) - self.combat_reach - other.combat_reach).max(0.0)
    }

    pub fn distance_z(&self, other: &Self) -> f32 {
        ((self.position().z - other.position().z).abs() - self.combat_reach - other.combat_reach)
            .max(0.0)
    }

    pub fn is_in_map(&self, other: &Self) -> bool {
        self.object.is_in_world()
            && other.object.is_in_world()
            && self.has_current_map
            && other.has_current_map
            && self.map_id() == other.map_id()
            && self.instance_id == other.instance_id
    }

    pub fn is_within_dist(
        &self,
        other: &Self,
        dist: f32,
        is_3d: bool,
        include_own_radius: bool,
        include_target_radius: bool,
    ) -> bool {
        let mut max_dist = dist;
        if include_own_radius {
            max_dist += self.combat_reach;
        }
        if include_target_radius {
            max_dist += other.combat_reach;
        }

        if is_3d {
            self.position().distance_sq(&other.position()) < max_dist * max_dist
        } else {
            self.position().distance_2d_sq(&other.position()) < max_dist * max_dist
        }
    }

    pub fn is_within_dist_in_map(&self, other: &Self, dist: f32, is_3d: bool) -> bool {
        self.is_in_map(other)
            && self.in_same_phase(other)
            && self.is_within_dist(other, dist, is_3d, true, true)
    }

    pub fn absolute_angle_to_position(&self, position: Position) -> f32 {
        normalize_orientation(
            (position.y - self.position().y).atan2(position.x - self.position().x),
        )
    }

    pub fn absolute_angle_to(&self, other: &Self) -> f32 {
        self.absolute_angle_to_position(other.position())
    }

    pub fn to_absolute_angle(&self, relative_angle: f32) -> f32 {
        normalize_orientation(relative_angle + self.position().orientation)
    }

    pub fn to_relative_angle(&self, absolute_angle: f32) -> f32 {
        normalize_orientation(absolute_angle - self.position().orientation)
    }

    pub fn relative_angle_to_position(&self, position: Position) -> f32 {
        self.to_relative_angle(self.absolute_angle_to_position(position))
    }

    pub fn relative_angle_to(&self, other: &Self) -> f32 {
        self.relative_angle_to_position(other.position())
    }

    pub fn has_in_arc(&self, arc: f32, target: &Self, border: f32) -> bool {
        if std::ptr::eq(self, target) {
            return true;
        }

        let arc = normalize_orientation(arc);
        let mut angle = self.relative_angle_to(target);
        if angle > PI {
            angle -= TAU;
        }

        let left_border = -(arc / border);
        let right_border = arc / border;
        left_border <= angle && angle <= right_border
    }

    pub fn is_in_front(&self, target: &Self, arc: f32) -> bool {
        self.has_in_arc(arc, target, 2.0)
    }

    pub fn is_in_back(&self, target: &Self, arc: f32) -> bool {
        !self.has_in_arc(TAU - arc, target, 2.0)
    }

    pub fn has_position_in_line(&self, position: Position, obj_size: f32, width: f32) -> bool {
        if !self.has_position_in_arc(PI, position, 2.0) {
            return false;
        }

        let width = width + obj_size;
        let angle = self.relative_angle_to_position(position);
        angle.sin().abs() * self.position().distance_2d(&position) < width
    }

    pub fn has_in_line(&self, target: &Self, width: f32) -> bool {
        self.has_position_in_line(target.position(), target.combat_reach(), width)
    }

    pub fn has_position_in_arc(&self, arc: f32, position: Position, border: f32) -> bool {
        let arc = normalize_orientation(arc);
        let mut angle = self.relative_angle_to_position(position);
        if angle > PI {
            angle -= TAU;
        }

        let left_border = -(arc / border);
        let right_border = arc / border;
        left_border <= angle && angle <= right_border
    }

    pub fn is_within_box(
        &self,
        center: Position,
        x_radius: f32,
        y_radius: f32,
        z_radius: f32,
    ) -> bool {
        let rotation = TAU - center.orientation;
        let sin = rotation.sin();
        let cos = rotation.cos();
        let position = self.position();

        let box_dist_x = position.x - center.x;
        let box_dist_y = position.y - center.y;
        let rot_x = center.x + box_dist_x * cos - box_dist_y * sin;
        let rot_y = center.y + box_dist_y * cos + box_dist_x * sin;

        let dx = rot_x - center.x;
        let dy = rot_y - center.y;
        let dz = position.z - center.z;
        dx.abs() <= x_radius && dy.abs() <= y_radius && dz.abs() <= z_radius
    }

    pub fn is_within_double_vertical_cylinder(
        &self,
        center: Position,
        radius: f32,
        height: f32,
    ) -> bool {
        self.position().distance_2d_sq(&center) < radius * radius
            && (self.position().z - center.z).abs() <= height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapBindingError {
    ObjectInWorld,
    AlreadyBound {
        old_map_id: u32,
        old_instance_id: u32,
        new_map_id: u32,
        new_instance_id: u32,
    },
    NoCurrentMap,
}

fn normalized_position(mut position: Position) -> Position {
    position.orientation = normalize_orientation(position.orientation);
    position
}

fn normalize_orientation(mut orientation: f32) -> f32 {
    orientation %= TAU;
    if orientation < 0.0 {
        orientation += TAU;
    }
    orientation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_location_defaults_match_cpp_world_location() {
        let location = WorldLocation::default();

        assert_eq!(location.map_id(), MAPID_INVALID);
        assert_eq!(location.position(), Position::ZERO);
    }

    #[test]
    fn world_object_constructor_matches_cpp_base_state() {
        let object = WorldObject::new(true, TypeId::Unit, TypeMask::UNIT);

        assert_eq!(object.object().type_id(), TypeId::Unit);
        assert_eq!(object.object().type_mask(), TypeMask::UNIT);
        assert_eq!(object.map_id(), MAPID_INVALID);
        assert_eq!(object.instance_id(), 0);
        assert!(!object.has_current_map());
        assert!(object.is_world_object());
        assert!(!object.is_active());
        assert!(!object.is_far_visible());
        assert_eq!(object.zone_id(), 0);
        assert_eq!(object.area_id(), 0);
        assert_eq!(object.db_phase(), 0);
        assert_eq!(object.combat_reach(), 0.0);
        assert_eq!(object.current_cell(), None);
    }

    #[test]
    fn relocate_normalizes_orientation_like_cpp_position() {
        let mut object = WorldObject::new(false, TypeId::GameObject, TypeMask::GAME_OBJECT);

        object.world_relocate(571, Position::new(1.0, 2.0, 3.0, -1.0));

        assert_eq!(object.map_id(), 571);
        assert!((object.position().orientation - (TAU - 1.0)).abs() < 0.0001);
        assert_eq!(object.object().map_id(), Some(571));
    }

    #[test]
    fn set_map_and_reset_map_follow_cpp_binding_rules() {
        let mut object = WorldObject::new(false, TypeId::Corpse, TypeMask::CORPSE);

        assert_eq!(object.set_map(1, 10), Ok(()));
        assert_eq!(object.map_id(), 1);
        assert_eq!(object.instance_id(), 10);
        assert!(object.has_current_map());
        assert_eq!(object.set_map(1, 10), Ok(()));
        assert!(matches!(
            object.set_map(2, 10),
            Err(MapBindingError::AlreadyBound { .. })
        ));

        object.object_mut().add_to_world();
        assert_eq!(object.reset_map(), Err(MapBindingError::ObjectInWorld));
        object.object_mut().remove_from_world();

        assert_eq!(object.reset_map(), Ok(()));
        assert!(!object.has_current_map());
        assert_eq!(object.map_id(), 1);
        assert_eq!(object.instance_id(), 10);
    }

    #[test]
    fn distance_helpers_subtract_combat_reach_and_clamp_zero() {
        let mut a = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut b = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        a.relocate(Position::xyz(0.0, 0.0, 0.0));
        b.relocate(Position::xyz(3.0, 4.0, 12.0));
        a.set_combat_reach(1.5);
        b.set_combat_reach(2.0);

        assert!((a.exact_distance(&b) - 13.0).abs() < 0.001);
        assert!((a.distance(&b) - 9.5).abs() < 0.001);
        assert!((a.distance_2d(&b) - 1.5).abs() < 0.001);
        assert!((a.distance_z(&b) - 8.5).abs() < 0.001);

        b.relocate(Position::xyz(1.0, 0.0, 0.0));
        assert_eq!(a.distance(&b), 0.0);
    }

    #[test]
    fn within_dist_uses_cpp_strict_less_than_and_radius_options() {
        let mut a = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut b = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        a.relocate(Position::xyz(0.0, 0.0, 0.0));
        b.relocate(Position::xyz(3.0, 4.0, 0.0));

        assert!(!a.is_within_dist(&b, 5.0, true, false, false));
        assert!(a.is_within_dist(&b, 5.01, true, false, false));

        a.set_combat_reach(1.0);
        b.set_combat_reach(1.0);
        assert!(a.is_within_dist(&b, 3.1, true, true, true));
        assert!(!a.is_within_dist(&b, 3.1, true, false, false));
    }

    #[test]
    fn within_dist_in_map_requires_world_map_and_phase() {
        let mut a = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut b = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        a.set_map(530, 1).unwrap();
        b.set_map(530, 1).unwrap();
        a.object_mut().add_to_world();
        b.object_mut().add_to_world();
        a.relocate(Position::xyz(0.0, 0.0, 0.0));
        b.relocate(Position::xyz(1.0, 0.0, 0.0));

        a.phase_shift_mut().insert(10);
        b.phase_shift_mut().insert(20);
        assert!(!a.is_within_dist_in_map(&b, 2.0, true));

        b.phase_shift_mut().insert(10);
        assert!(a.is_within_dist_in_map(&b, 2.0, true));
    }

    #[test]
    fn angle_helpers_match_cpp_position_relative_angle_semantics() {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        object.relocate(Position::new(0.0, 0.0, 0.0, PI / 2.0));

        assert!(
            (object.absolute_angle_to_position(Position::xyz(1.0, 0.0, 0.0)) - 0.0).abs() < 0.0001
        );
        assert!((object.to_absolute_angle(PI / 2.0) - PI).abs() < 0.0001);
        assert!((object.to_relative_angle(0.0) - (TAU - PI / 2.0)).abs() < 0.0001);
        assert!(
            (object.relative_angle_to_position(Position::xyz(0.0, 1.0, 0.0)) - 0.0).abs() < 0.0001
        );
    }

    #[test]
    fn arc_front_back_and_line_helpers_match_cpp_boundaries() {
        let mut source = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut front = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut side = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut back = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        source.relocate(Position::new(0.0, 0.0, 0.0, 0.0));
        front.relocate(Position::xyz(1.0, 0.0, 0.0));
        side.relocate(Position::xyz(0.0, 1.0, 0.0));
        back.relocate(Position::xyz(-1.0, 0.0, 0.0));

        assert!(source.has_in_arc(PI, &source, 2.0));
        assert!(source.is_in_front(&front, PI));
        assert!(source.is_in_front(&side, PI));
        assert!(!source.is_in_front(&back, PI));
        assert!(source.is_in_back(&back, PI));
        assert!(!source.is_in_back(&front, PI));

        front.relocate(Position::xyz(10.0, 1.0, 0.0));
        assert!(source.has_in_line(&front, 2.0));
        front.relocate(Position::xyz(10.0, 3.0, 0.0));
        assert!(!source.has_in_line(&front, 2.0));
    }

    #[test]
    fn box_and_double_vertical_cylinder_match_cpp_geometry() {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let center = Position::new(0.0, 0.0, 5.0, PI / 2.0);

        object.relocate(Position::xyz(0.5, 1.5, 5.5));
        assert!(object.is_within_box(center, 2.0, 1.0, 1.0));

        object.relocate(Position::xyz(1.5, 1.5, 5.5));
        assert!(!object.is_within_box(center, 2.0, 1.0, 1.0));

        object.relocate(Position::xyz(3.0, 4.0, 8.0));
        assert!(!object.is_within_double_vertical_cylinder(center, 5.0, 3.0));
        object.relocate(Position::xyz(3.0, 3.9, 8.0));
        assert!(object.is_within_double_vertical_cylinder(center, 5.0, 3.0));
    }
}
