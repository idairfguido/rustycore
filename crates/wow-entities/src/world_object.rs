use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::{PI, TAU};

use wow_constants::{TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

use crate::{
    EntityObject,
    vehicle::{calculate_passenger_offset, calculate_passenger_position},
};

pub const MAPID_INVALID: u32 = u32::MAX;

/// TrinityCore `MAX_VISIBILITY_DISTANCE` (`SIZE_OF_GRIDS`).
pub const MAX_VISIBILITY_DISTANCE: f32 = 533.3333;
/// TrinityCore `SIGHT_RANGE_UNIT`.
pub const SIGHT_RANGE_UNIT: f32 = 50.0;
/// TrinityCore normal visibility distance.
pub const DEFAULT_VISIBILITY_DISTANCE: f32 = 100.0;
/// TrinityCore default instance/cinematic visibility distance.
pub const DEFAULT_VISIBILITY_INSTANCE: f32 = 170.0;
/// TrinityCore invalid terrain height sentinel.
pub const INVALID_HEIGHT: f32 = -100_000.0;
/// TrinityCore `MAX_HEIGHT` sentinel for unconstrained height search.
pub const MAX_HEIGHT: f32 = 100_000.0;
/// TrinityCore `DEFAULT_HEIGHT_SEARCH`.
pub const DEFAULT_HEIGHT_SEARCH: f32 = 50.0;
/// TrinityCore `Z_OFFSET_FIND_HEIGHT`.
pub const Z_OFFSET_FIND_HEIGHT: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldObjectHeightQuery {
    pub vmap: bool,
    pub distance_to_search: f32,
}

impl Default for WorldObjectHeightQuery {
    fn default() -> Self {
        Self {
            vmap: true,
            distance_to_search: DEFAULT_HEIGHT_SEARCH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineOfSightOptions {
    pub check_dynamic: bool,
}

/// Endpoint passed to the LOS bridge.
///
/// TrinityCore adjusts object LOS endpoints with `GetCollisionHeight()` and
/// `GetHitSpherePointFor(...)` before calling `Map::isInLineOfSight`. RustyCore does not yet
/// have canonical collision-height / hit-sphere ownership in `wow-entities`, so current
/// endpoint constructors intentionally expose raw positions and mark both adjustments as not
/// applied. Keep this metadata explicit until the canonical collision model can fill it in.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct LineOfSightEndpoint {
    pub position: Position,
    pub collision_height_adjusted: bool,
    pub hit_sphere_adjusted: bool,
}

impl LineOfSightEndpoint {
    pub const fn raw_position(position: Position) -> Self {
        Self {
            position,
            collision_height_adjusted: false,
            hit_sphere_adjusted: false,
        }
    }
}

/// Query passed from `WorldObject` LOS helpers to the map/terrain bridge.
///
/// `target` is populated for `is_within_los_in_map`, allowing a future map bridge to reproduce
/// TrinityCore's target-aware hit-sphere endpoint adjustment without changing the trait shape.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct LineOfSightQuery<'a> {
    pub source: &'a WorldObject,
    pub target: Option<&'a WorldObject>,
    pub from: LineOfSightEndpoint,
    pub to: LineOfSightEndpoint,
    pub options: LineOfSightOptions,
}

impl<'a> LineOfSightQuery<'a> {
    pub fn raw_to_position(
        source: &'a WorldObject,
        position: Position,
        options: LineOfSightOptions,
    ) -> Self {
        Self {
            source,
            target: None,
            from: LineOfSightEndpoint::raw_position(source.position()),
            to: LineOfSightEndpoint::raw_position(position),
            options,
        }
    }

    pub fn raw_to_object(
        source: &'a WorldObject,
        target: &'a WorldObject,
        options: LineOfSightOptions,
    ) -> Self {
        Self {
            source,
            target: Some(target),
            from: LineOfSightEndpoint::raw_position(source.position()),
            to: LineOfSightEndpoint::raw_position(target.position()),
            options,
        }
    }
}

/// Bridge for `WorldObject` helpers whose C++ implementation delegates to `Map`/terrain.
///
/// This keeps `wow-entities` independent from `wow-map` while preserving the represented C++
/// call shape. LOS is still a partial bridge: endpoint collision-height and hit-sphere
/// adjustment are documented in `LineOfSightEndpoint` metadata but not computed until canonical
/// collision data exists.
pub trait WorldObjectEnvironment {
    fn map_id(&self) -> u32;
    fn instance_id(&self) -> u32;
    fn visibility_range(&self) -> f32;

    fn visibility_override(&self, _object: &WorldObject) -> Option<f32> {
        None
    }

    fn creature_sight_distance(&self, _object: &WorldObject) -> Option<f32> {
        None
    }

    fn player_on_cinematic(&self, _object: &WorldObject) -> bool {
        false
    }

    fn line_of_sight(&self, _query: LineOfSightQuery<'_>) -> bool;

    fn map_height(
        &self,
        _object: &WorldObject,
        x: f32,
        y: f32,
        z: f32,
        query: WorldObjectHeightQuery,
    ) -> f32;

    fn floor_z(&self, _object: &WorldObject, position: Position, max_search_dist: f32) -> f32;
}

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
pub struct VisibleMapIdRef {
    references: i32,
}

impl VisibleMapIdRef {
    pub const fn references(&self) -> i32 {
        self.references
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiMapPhaseIdRef {
    references: i32,
}

impl UiMapPhaseIdRef {
    pub const fn references(&self) -> i32 {
        self.references
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseShift {
    phases: BTreeSet<u32>,
    visible_map_ids: BTreeMap<u32, VisibleMapIdRef>,
    ui_map_phase_ids: BTreeMap<u32, UiMapPhaseIdRef>,
}

impl PhaseShift {
    pub fn from_phases(phases: impl IntoIterator<Item = u32>) -> Self {
        Self {
            phases: phases.into_iter().collect(),
            visible_map_ids: BTreeMap::new(),
            ui_map_phase_ids: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, phase_id: u32) {
        self.phases.insert(phase_id);
    }

    pub fn clear(&mut self) {
        self.phases.clear();
        self.visible_map_ids.clear();
        self.ui_map_phase_ids.clear();
    }

    pub fn add_visible_map_id_like_cpp(&mut self, visible_map_id: u32, references: i32) -> bool {
        let inserted = !self.visible_map_ids.contains_key(&visible_map_id);
        let entry = self
            .visible_map_ids
            .entry(visible_map_id)
            .or_insert(VisibleMapIdRef { references: 0 });
        entry.references += references;
        inserted
    }

    pub fn remove_visible_map_id_like_cpp(&mut self, visible_map_id: u32) -> bool {
        let Some(entry) = self.visible_map_ids.get_mut(&visible_map_id) else {
            return false;
        };
        entry.references -= 1;
        if entry.references == 0 {
            self.visible_map_ids.remove(&visible_map_id);
            return true;
        }
        false
    }

    pub fn has_visible_map_id_like_cpp(&self, visible_map_id: u32) -> bool {
        self.visible_map_ids.contains_key(&visible_map_id)
    }

    pub fn visible_map_id_count_like_cpp(&self) -> usize {
        self.visible_map_ids.len()
    }

    pub fn visible_map_ids_like_cpp(&self) -> impl Iterator<Item = u32> + '_ {
        self.visible_map_ids.keys().copied()
    }

    pub fn visible_map_id_ref_like_cpp(&self, visible_map_id: u32) -> Option<&VisibleMapIdRef> {
        self.visible_map_ids.get(&visible_map_id)
    }

    pub fn add_ui_map_phase_id_like_cpp(&mut self, ui_map_phase_id: u32, references: i32) -> bool {
        let inserted = !self.ui_map_phase_ids.contains_key(&ui_map_phase_id);
        let entry = self
            .ui_map_phase_ids
            .entry(ui_map_phase_id)
            .or_insert(UiMapPhaseIdRef { references: 0 });
        entry.references += references;
        inserted
    }

    pub fn remove_ui_map_phase_id_like_cpp(&mut self, ui_map_phase_id: u32) -> bool {
        let Some(entry) = self.ui_map_phase_ids.get_mut(&ui_map_phase_id) else {
            return false;
        };
        entry.references -= 1;
        if entry.references == 0 {
            self.ui_map_phase_ids.remove(&ui_map_phase_id);
            return true;
        }
        false
    }

    pub fn has_ui_map_phase_id_like_cpp(&self, ui_map_phase_id: u32) -> bool {
        self.ui_map_phase_ids.contains_key(&ui_map_phase_id)
    }

    pub fn ui_map_phase_ids_like_cpp(&self) -> impl Iterator<Item = u32> + '_ {
        self.ui_map_phase_ids.keys().copied()
    }

    pub fn ui_map_phase_id_ref_like_cpp(&self, ui_map_phase_id: u32) -> Option<&UiMapPhaseIdRef> {
        self.ui_map_phase_ids.get(&ui_map_phase_id)
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
    static_floor_z: f32,
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
            static_floor_z: INVALID_HEIGHT,
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

    pub const fn static_floor_z(&self) -> f32 {
        self.static_floor_z
    }

    pub fn set_static_floor_z(&mut self, static_floor_z: f32) {
        self.static_floor_z = static_floor_z;
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

    pub fn get_visibility_range(&self, environment: &impl WorldObjectEnvironment) -> f32 {
        if environment.visibility_override(self).is_some() && !self.is_player() {
            environment
                .visibility_override(self)
                .unwrap_or(environment.visibility_range())
        } else if self.is_far_visible() && !self.is_player() {
            MAX_VISIBILITY_DISTANCE
        } else {
            environment.visibility_range()
        }
    }

    pub fn get_sight_range(
        &self,
        target: Option<&WorldObject>,
        environment: &impl WorldObjectEnvironment,
    ) -> f32 {
        if self.is_unit() {
            if self.is_player() {
                if let Some(target) = target {
                    if let Some(override_range) = environment.visibility_override(target) {
                        if !target.is_player() {
                            return override_range;
                        }
                    }
                    if target.is_far_visible() && !target.is_player() {
                        return MAX_VISIBILITY_DISTANCE;
                    }
                }

                if environment.player_on_cinematic(self) {
                    DEFAULT_VISIBILITY_INSTANCE
                } else {
                    environment.visibility_range()
                }
            } else {
                environment
                    .creature_sight_distance(self)
                    .unwrap_or(SIGHT_RANGE_UNIT)
            }
        } else if self.object.type_id() == TypeId::DynamicObject && self.is_active() {
            environment.visibility_range()
        } else {
            0.0
        }
    }

    pub fn is_within_los(
        &self,
        position: Position,
        environment: &impl WorldObjectEnvironment,
        options: LineOfSightOptions,
    ) -> bool {
        if !self.object.is_in_world() {
            return true;
        }

        if !self.is_in_environment(environment) {
            return false;
        }

        environment.line_of_sight(LineOfSightQuery::raw_to_position(self, position, options))
    }

    pub fn is_within_los_in_map(
        &self,
        other: &Self,
        environment: &impl WorldObjectEnvironment,
        options: LineOfSightOptions,
    ) -> bool {
        self.is_in_map(other)
            && self.is_in_environment(environment)
            && environment.line_of_sight(LineOfSightQuery::raw_to_object(self, other, options))
    }

    pub fn get_map_height(
        &self,
        environment: &impl WorldObjectEnvironment,
        x: f32,
        y: f32,
        z: f32,
        query: WorldObjectHeightQuery,
    ) -> f32 {
        if !self.is_in_environment(environment) {
            return INVALID_HEIGHT;
        }

        let query_z = if z == MAX_HEIGHT {
            z
        } else {
            z + Z_OFFSET_FIND_HEIGHT
        };
        environment.map_height(self, x, y, query_z, query)
    }

    pub fn update_ground_position_z(
        &self,
        environment: &impl WorldObjectEnvironment,
        x: f32,
        y: f32,
        z: f32,
        hover_offset: f32,
    ) -> f32 {
        let new_z = self.get_map_height(environment, x, y, z, WorldObjectHeightQuery::default());
        if new_z > INVALID_HEIGHT {
            new_z + if self.is_unit() { hover_offset } else { 0.0 }
        } else {
            z
        }
    }

    pub fn get_floor_z(&self, environment: &impl WorldObjectEnvironment) -> f32 {
        if !self.object.is_in_world() || !self.is_in_environment(environment) {
            return self.static_floor_z;
        }

        let position = Position::new(
            self.position().x,
            self.position().y,
            self.position().z + Z_OFFSET_FIND_HEIGHT,
            self.position().orientation,
        );
        self.static_floor_z
            .max(environment.floor_z(self, position, DEFAULT_HEIGHT_SEARCH))
    }

    pub fn transport_global_position_from_offset(
        &self,
        transport_position: Position,
        passenger_offset: Position,
    ) -> Position {
        calculate_passenger_position(passenger_offset, transport_position)
    }

    pub fn transport_offset_from_position(&self, transport_position: Position) -> Position {
        calculate_passenger_offset(self.position(), transport_position)
    }

    pub fn relocate_on_transport(
        &mut self,
        transport_position: Position,
        passenger_offset: Position,
    ) -> Position {
        let global =
            self.transport_global_position_from_offset(transport_position, passenger_offset);
        self.relocate(global);
        global
    }

    fn is_in_environment(&self, environment: &impl WorldObjectEnvironment) -> bool {
        self.has_current_map
            && self.map_id() == environment.map_id()
            && self.instance_id == environment.instance_id()
    }

    fn is_player(&self) -> bool {
        matches!(self.object.type_id(), TypeId::Player | TypeId::ActivePlayer)
            || self
                .object
                .type_mask()
                .intersects(TypeMask::PLAYER | TypeMask::ACTIVE_PLAYER)
    }

    fn is_unit(&self) -> bool {
        self.object
            .type_mask()
            .intersects(TypeMask::UNIT | TypeMask::PLAYER | TypeMask::ACTIVE_PLAYER)
            || matches!(
                self.object.type_id(),
                TypeId::Unit | TypeId::Player | TypeId::ActivePlayer
            )
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
    use std::cell::Cell;

    use super::*;

    #[derive(Debug, Clone)]
    struct TestEnvironment {
        map_id: u32,
        instance_id: u32,
        visibility_range: f32,
        visibility_override: Option<f32>,
        creature_sight_distance: Option<f32>,
        cinematic: bool,
        los: bool,
        los_calls: Cell<usize>,
        height: f32,
        floor: f32,
    }

    impl Default for TestEnvironment {
        fn default() -> Self {
            Self {
                map_id: 571,
                instance_id: 1,
                visibility_range: DEFAULT_VISIBILITY_DISTANCE,
                visibility_override: None,
                creature_sight_distance: None,
                cinematic: false,
                los: true,
                los_calls: Cell::new(0),
                height: INVALID_HEIGHT,
                floor: INVALID_HEIGHT,
            }
        }
    }

    impl WorldObjectEnvironment for TestEnvironment {
        fn map_id(&self) -> u32 {
            self.map_id
        }

        fn instance_id(&self) -> u32 {
            self.instance_id
        }

        fn visibility_range(&self) -> f32 {
            self.visibility_range
        }

        fn visibility_override(&self, _object: &WorldObject) -> Option<f32> {
            self.visibility_override
        }

        fn creature_sight_distance(&self, _object: &WorldObject) -> Option<f32> {
            self.creature_sight_distance
        }

        fn player_on_cinematic(&self, _object: &WorldObject) -> bool {
            self.cinematic
        }

        fn line_of_sight(&self, query: LineOfSightQuery<'_>) -> bool {
            self.los_calls.set(self.los_calls.get() + 1);
            assert!(!query.from.collision_height_adjusted);
            assert!(!query.from.hit_sphere_adjusted);
            assert!(!query.to.collision_height_adjusted);
            assert!(!query.to.hit_sphere_adjusted);
            self.los
        }

        fn map_height(
            &self,
            _object: &WorldObject,
            _x: f32,
            _y: f32,
            z: f32,
            _query: WorldObjectHeightQuery,
        ) -> f32 {
            if (z - (10.0 + Z_OFFSET_FIND_HEIGHT)).abs() < 0.001 {
                self.height
            } else {
                INVALID_HEIGHT
            }
        }

        fn floor_z(&self, _object: &WorldObject, position: Position, _max_search_dist: f32) -> f32 {
            if (position.z - (10.0 + Z_OFFSET_FIND_HEIGHT)).abs() < 0.001 {
                self.floor
            } else {
                INVALID_HEIGHT
            }
        }
    }

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
        assert_eq!(object.static_floor_z(), INVALID_HEIGHT);
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
    fn phase_shift_visible_map_ids_reference_count_like_cpp() {
        let mut phase_shift = PhaseShift::default();

        assert!(phase_shift.add_visible_map_id_like_cpp(609, 1));
        assert!(!phase_shift.add_visible_map_id_like_cpp(609, 1));
        assert!(phase_shift.has_visible_map_id_like_cpp(609));
        assert_eq!(phase_shift.visible_map_id_count_like_cpp(), 1);
        assert_eq!(
            phase_shift
                .visible_map_id_ref_like_cpp(609)
                .map(VisibleMapIdRef::references),
            Some(2)
        );
        assert_eq!(
            phase_shift.visible_map_ids_like_cpp().collect::<Vec<_>>(),
            vec![609]
        );

        assert!(!phase_shift.remove_visible_map_id_like_cpp(609));
        assert_eq!(
            phase_shift
                .visible_map_id_ref_like_cpp(609)
                .map(VisibleMapIdRef::references),
            Some(1)
        );
        assert!(phase_shift.remove_visible_map_id_like_cpp(609));
        assert!(!phase_shift.has_visible_map_id_like_cpp(609));
        assert!(!phase_shift.remove_visible_map_id_like_cpp(609));
    }

    #[test]
    fn phase_shift_clear_removes_visible_map_ids_like_cpp() {
        let mut phase_shift = PhaseShift::from_phases([10]);
        phase_shift.add_visible_map_id_like_cpp(609, 1);
        phase_shift.add_ui_map_phase_id_like_cpp(42, 1);

        phase_shift.clear();

        assert!(!phase_shift.has_visible_map_id_like_cpp(609));
        assert!(phase_shift.visible_map_ids_like_cpp().next().is_none());
        assert!(!phase_shift.has_ui_map_phase_id_like_cpp(42));
        assert!(phase_shift.ui_map_phase_ids_like_cpp().next().is_none());
        assert!(phase_shift.can_see(&PhaseShift::from_phases([20])));
    }

    #[test]
    fn phase_shift_ui_map_phase_ids_reference_count_like_cpp() {
        let mut phase_shift = PhaseShift::default();

        assert!(phase_shift.add_ui_map_phase_id_like_cpp(42, 1));
        assert!(!phase_shift.add_ui_map_phase_id_like_cpp(42, 1));
        assert!(phase_shift.has_ui_map_phase_id_like_cpp(42));
        assert_eq!(
            phase_shift
                .ui_map_phase_id_ref_like_cpp(42)
                .map(UiMapPhaseIdRef::references),
            Some(2)
        );
        assert_eq!(
            phase_shift.ui_map_phase_ids_like_cpp().collect::<Vec<_>>(),
            vec![42]
        );

        assert!(!phase_shift.remove_ui_map_phase_id_like_cpp(42));
        assert_eq!(
            phase_shift
                .ui_map_phase_id_ref_like_cpp(42)
                .map(UiMapPhaseIdRef::references),
            Some(1)
        );
        assert!(phase_shift.remove_ui_map_phase_id_like_cpp(42));
        assert!(!phase_shift.has_ui_map_phase_id_like_cpp(42));
        assert!(!phase_shift.remove_ui_map_phase_id_like_cpp(42));
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

    #[test]
    fn world_object_visibility_range_uses_override_far_visible_then_map_range() {
        let environment = TestEnvironment {
            visibility_range: 123.0,
            ..TestEnvironment::default()
        };
        let mut object = WorldObject::new(false, TypeId::GameObject, TypeMask::GAME_OBJECT);
        assert_eq!(object.get_visibility_range(&environment), 123.0);

        object.set_far_visible(true);
        assert_eq!(
            object.get_visibility_range(&environment),
            MAX_VISIBILITY_DISTANCE
        );

        let environment = TestEnvironment {
            visibility_override: Some(222.0),
            ..environment
        };
        assert_eq!(object.get_visibility_range(&environment), 222.0);

        let mut player = WorldObject::new(false, TypeId::Player, TypeMask::PLAYER | TypeMask::UNIT);
        player.set_far_visible(true);
        assert_eq!(player.get_visibility_range(&environment), 123.0);
    }

    #[test]
    fn world_object_sight_range_matches_representable_cpp_cases() {
        let environment = TestEnvironment {
            visibility_range: 140.0,
            creature_sight_distance: Some(80.0),
            ..TestEnvironment::default()
        };
        let player = WorldObject::new(false, TypeId::Player, TypeMask::PLAYER | TypeMask::UNIT);
        let creature = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let unit = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut dyn_object =
            WorldObject::new(false, TypeId::DynamicObject, TypeMask::DYNAMIC_OBJECT);
        dyn_object.set_active(true);

        assert_eq!(player.get_sight_range(None, &environment), 140.0);
        assert_eq!(creature.get_sight_range(None, &environment), 80.0);
        assert_eq!(
            unit.get_sight_range(None, &TestEnvironment::default()),
            SIGHT_RANGE_UNIT
        );
        assert_eq!(dyn_object.get_sight_range(None, &environment), 140.0);

        let mut target = WorldObject::new(false, TypeId::GameObject, TypeMask::GAME_OBJECT);
        target.set_far_visible(true);
        assert_eq!(
            player.get_sight_range(Some(&target), &environment),
            MAX_VISIBILITY_DISTANCE
        );

        let cinematic = TestEnvironment {
            visibility_range: 140.0,
            cinematic: true,
            ..TestEnvironment::default()
        };
        assert_eq!(
            player.get_sight_range(None, &cinematic),
            DEFAULT_VISIBILITY_INSTANCE
        );
    }

    #[test]
    fn world_object_los_prefilters_map_not_phase_and_uses_partial_raw_endpoint_bridge() {
        let environment = TestEnvironment {
            los: false,
            ..TestEnvironment::default()
        };
        let mut source = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        let mut target = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);

        assert!(source.is_within_los(
            Position::xyz(1.0, 0.0, 0.0),
            &environment,
            LineOfSightOptions::default()
        ));
        assert_eq!(environment.los_calls.get(), 0);

        source.set_map(571, 1).unwrap();
        target.set_map(571, 2).unwrap();
        source.object_mut().add_to_world();
        target.object_mut().add_to_world();
        assert!(!source.is_within_los_in_map(&target, &environment, LineOfSightOptions::default()));
        assert_eq!(environment.los_calls.get(), 0);

        target.object_mut().remove_from_world();
        target.reset_map().unwrap();
        target.set_map(571, 1).unwrap();
        target.object_mut().add_to_world();
        source.phase_shift_mut().insert(10);
        target.phase_shift_mut().insert(20);
        assert!(!source.in_same_phase(&target));
        assert!(!source.is_within_los_in_map(&target, &environment, LineOfSightOptions::default()));
        assert_eq!(environment.los_calls.get(), 1);

        let environment = TestEnvironment {
            los: true,
            ..environment
        };
        assert!(source.is_within_los_in_map(&target, &environment, LineOfSightOptions::default()));
        assert_eq!(environment.los_calls.get(), 2);
    }

    #[test]
    fn world_object_height_floor_and_ground_update_use_map_bridge() {
        let environment = TestEnvironment {
            height: 20.0,
            floor: 30.0,
            ..TestEnvironment::default()
        };
        let mut unit = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        unit.set_map(571, 1).unwrap();
        unit.object_mut().add_to_world();
        unit.relocate(Position::xyz(1.0, 2.0, 10.0));

        assert_eq!(
            unit.get_map_height(
                &environment,
                1.0,
                2.0,
                10.0,
                WorldObjectHeightQuery::default()
            ),
            20.0
        );
        assert_eq!(
            unit.update_ground_position_z(&environment, 1.0, 2.0, 10.0, 1.25),
            21.25
        );

        unit.set_static_floor_z(25.0);
        assert_eq!(unit.get_floor_z(&environment), 30.0);

        let environment = TestEnvironment {
            height: INVALID_HEIGHT,
            floor: INVALID_HEIGHT,
            ..environment
        };
        assert_eq!(
            unit.update_ground_position_z(&environment, 1.0, 2.0, 10.0, 1.25),
            10.0
        );
        assert_eq!(unit.get_floor_z(&environment), 25.0);
    }

    #[test]
    fn world_object_transport_relocation_roundtrips_offset_and_global_position() {
        let transport = Position::new(100.0, 200.0, 10.0, PI / 2.0);
        let offset = Position::new(4.0, -2.0, 3.0, PI / 4.0);
        let mut passenger = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);

        let global = passenger.relocate_on_transport(transport, offset);
        assert_eq!(passenger.position(), global);

        let roundtrip = passenger.transport_offset_from_position(transport);
        assert!((roundtrip.x - offset.x).abs() < 0.0001);
        assert!((roundtrip.y - offset.y).abs() < 0.0001);
        assert!((roundtrip.z - offset.z).abs() < 0.0001);
        assert!((roundtrip.orientation - offset.orientation).abs() < 0.0001);
    }
}
