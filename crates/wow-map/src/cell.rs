//! Cell-level storage for map objects.
//!
//! C++ reference:
//! - `Grids/Cells/Cell.h` for grid/cell decomposition and `nocreate`.
//! - `Grids/Grid.h` for separate world-object and grid-object containers.

use std::collections::HashSet;

use wow_core::ObjectGuid;

use crate::coords::{
    CellCoord, GridCoord, MAX_NUMBER_OF_CELLS, cell_to_grid_local, compute_cell_coord,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellArea {
    pub low_bound: CellCoord,
    pub high_bound: CellCoord,
}

impl CellArea {
    pub const fn new(low_bound: CellCoord, high_bound: CellCoord) -> Self {
        Self {
            low_bound,
            high_bound,
        }
    }

    pub fn is_empty(self) -> bool {
        self.low_bound == self.high_bound
    }

    pub fn resize_borders(self, begin_cell: &mut CellCoord, end_cell: &mut CellCoord) {
        *begin_cell = self.low_bound;
        *end_cell = self.high_bound;
    }
}

pub fn calculate_cell_area_like_cpp(x: f32, y: f32, radius: f32) -> CellArea {
    if radius <= 0.0 {
        let mut center = compute_cell_coord(x, y);
        center.normalize();
        return CellArea::new(center, center);
    }

    let mut low = compute_cell_coord(x - radius, y - radius);
    let mut high = compute_cell_coord(x + radius, y + radius);
    low.normalize();
    high.normalize();
    CellArea::new(low, high)
}

#[derive(Debug, Clone, Default)]
pub struct WorldObjectGuids {
    pub players: HashSet<ObjectGuid>,
    pub creatures: HashSet<ObjectGuid>,
    pub corpses: HashSet<ObjectGuid>,
    pub dynamic_objects: HashSet<ObjectGuid>,
}

impl WorldObjectGuids {
    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
            && self.creatures.is_empty()
            && self.corpses.is_empty()
            && self.dynamic_objects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.players.len() + self.creatures.len() + self.corpses.len() + self.dynamic_objects.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct GridObjectGuids {
    pub gameobjects: HashSet<ObjectGuid>,
    pub creatures: HashSet<ObjectGuid>,
    pub dynamic_objects: HashSet<ObjectGuid>,
    pub corpses: HashSet<ObjectGuid>,
    pub area_triggers: HashSet<ObjectGuid>,
    pub scene_objects: HashSet<ObjectGuid>,
    pub conversations: HashSet<ObjectGuid>,
}

impl GridObjectGuids {
    pub fn is_empty(&self) -> bool {
        self.gameobjects.is_empty()
            && self.creatures.is_empty()
            && self.dynamic_objects.is_empty()
            && self.corpses.is_empty()
            && self.area_triggers.is_empty()
            && self.scene_objects.is_empty()
            && self.conversations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.gameobjects.len()
            + self.creatures.len()
            + self.dynamic_objects.len()
            + self.corpses.len()
            + self.area_triggers.len()
            + self.scene_objects.len()
            + self.conversations.len()
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    grid: GridCoord,
    cell_x: u32,
    cell_y: u32,
    no_create: bool,
    pub world_objects: WorldObjectGuids,
    pub grid_objects: GridObjectGuids,
}

impl Default for Cell {
    fn default() -> Self {
        Self::from_cell_coord(CellCoord::new(0, 0))
    }
}

impl Cell {
    pub fn from_cell_coord(cell_coord: CellCoord) -> Self {
        let (grid, cell_x, cell_y) = cell_to_grid_local(cell_coord);
        Self {
            grid,
            cell_x,
            cell_y,
            no_create: false,
            world_objects: WorldObjectGuids::default(),
            grid_objects: GridObjectGuids::default(),
        }
    }

    pub fn from_world(x: f32, y: f32) -> Self {
        Self::from_cell_coord(compute_cell_coord(x, y))
    }

    pub fn compute(&self) -> (u32, u32) {
        (
            self.grid.x_coord * MAX_NUMBER_OF_CELLS + self.cell_x,
            self.grid.y_coord * MAX_NUMBER_OF_CELLS + self.cell_y,
        )
    }

    pub fn diff_cell(&self, other: &Self) -> bool {
        self.cell_x != other.cell_x || self.cell_y != other.cell_y
    }

    pub fn diff_grid(&self, other: &Self) -> bool {
        self.grid != other.grid
    }

    pub fn cell_x(&self) -> u32 {
        self.cell_x
    }

    pub fn cell_y(&self) -> u32 {
        self.cell_y
    }

    pub fn grid_x(&self) -> u32 {
        self.grid.x_coord
    }

    pub fn grid_y(&self) -> u32 {
        self.grid.y_coord
    }

    pub fn no_create(&self) -> bool {
        self.no_create
    }

    pub fn set_no_create(&mut self) {
        self.no_create = true;
    }

    pub fn cell_coord(&self) -> CellCoord {
        CellCoord::new(
            self.grid.x_coord * MAX_NUMBER_OF_CELLS + self.cell_x,
            self.grid.y_coord * MAX_NUMBER_OF_CELLS + self.cell_y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_decomposes_map_wide_cell_coord_like_trinity_cell() {
        let cell = Cell::from_cell_coord(CellCoord::new(257, 258));

        assert_eq!(cell.grid_x(), 32);
        assert_eq!(cell.grid_y(), 32);
        assert_eq!(cell.cell_x(), 1);
        assert_eq!(cell.cell_y(), 2);
        assert_eq!(cell.compute(), (257, 258));
        assert_eq!(cell.cell_coord(), CellCoord::new(257, 258));
    }

    #[test]
    fn cell_from_world_uses_trinity_cell_coord() {
        let cell = Cell::from_world(0.0, 0.0);

        assert_eq!(cell.grid_x(), 32);
        assert_eq!(cell.grid_y(), 32);
        assert_eq!(cell.cell_x(), 0);
        assert_eq!(cell.cell_y(), 0);
    }

    #[test]
    fn cell_diff_methods_match_cell_h_semantics() {
        let base = Cell::from_cell_coord(CellCoord::new(256, 256));
        let same_grid = Cell::from_cell_coord(CellCoord::new(257, 256));
        let other_grid = Cell::from_cell_coord(CellCoord::new(264, 256));

        assert!(base.diff_cell(&same_grid));
        assert!(!base.diff_grid(&same_grid));
        assert!(base.diff_grid(&other_grid));
    }

    #[test]
    fn no_create_flag_is_separate_from_coordinates() {
        let mut cell = Cell::from_cell_coord(CellCoord::new(256, 256));
        assert!(!cell.no_create());

        cell.set_no_create();
        assert!(cell.no_create());
        assert_eq!(cell.cell_coord(), CellCoord::new(256, 256));
    }

    #[test]
    fn calculate_cell_area_matches_cpp_bounds_and_normalization() {
        assert_eq!(
            calculate_cell_area_like_cpp(0.0, 0.0, 0.0),
            CellArea::new(CellCoord::new(256, 256), CellCoord::new(256, 256))
        );

        let area = calculate_cell_area_like_cpp(0.0, 0.0, 70.0);
        assert_eq!(area.low_bound, CellCoord::new(254, 254));
        assert_eq!(area.high_bound, CellCoord::new(257, 257));

        let edge = calculate_cell_area_like_cpp(-50_000.0, -50_000.0, 100.0);
        assert_eq!(edge.low_bound, CellCoord::new(511, 511));
        assert_eq!(edge.high_bound, CellCoord::new(511, 511));
    }

    #[test]
    fn object_guid_sets_keep_world_and_grid_objects_separate() {
        let mut cell = Cell::from_cell_coord(CellCoord::new(256, 256));
        let player = ObjectGuid::new(1, 1);
        let gameobject = ObjectGuid::new(2, 2);

        cell.world_objects.players.insert(player);
        cell.grid_objects.gameobjects.insert(gameobject);

        assert_eq!(cell.world_objects.len(), 1);
        assert_eq!(cell.grid_objects.len(), 1);
        assert!(cell.world_objects.players.contains(&player));
        assert!(cell.grid_objects.gameobjects.contains(&gameobject));
    }
}
