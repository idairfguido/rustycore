//! Coordinate math for the TrinityCore Map/Grid/Cell hierarchy.
//!
//! C++ reference: `/home/server/woltk-trinity-legacy/src/server/game/Grids/GridDefines.h`.

/// Number of cells in one NGrid axis.
pub const MAX_NUMBER_OF_CELLS: u32 = 8;

/// Number of NGrids in one Map axis.
pub const MAX_NUMBER_OF_GRIDS: u32 = 64;

/// Width/height of one NGrid in world yards.
pub const SIZE_OF_GRIDS: f32 = 533.3333;

/// Grid ID of the map center.
pub const CENTER_GRID_ID: u32 = MAX_NUMBER_OF_GRIDS / 2;

/// World offset to a grid center.
pub const CENTER_GRID_OFFSET: f32 = SIZE_OF_GRIDS / 2.0;

/// Width/height of one cell inside an NGrid.
pub const SIZE_OF_GRID_CELL: f32 = SIZE_OF_GRIDS / MAX_NUMBER_OF_CELLS as f32;

/// Cell ID of the map center.
pub const CENTER_GRID_CELL_ID: u32 = MAX_NUMBER_OF_CELLS * MAX_NUMBER_OF_GRIDS / 2;

/// World offset to a cell center.
pub const CENTER_GRID_CELL_OFFSET: f32 = SIZE_OF_GRID_CELL / 2.0;

/// Total cells in one map axis.
pub const TOTAL_NUMBER_OF_CELLS_PER_MAP: u32 = MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_CELLS;

/// Full map width/height in world yards.
pub const MAP_SIZE: f32 = SIZE_OF_GRIDS * MAX_NUMBER_OF_GRIDS as f32;

/// Half map width/height in world yards.
pub const MAP_HALFSIZE: f32 = MAP_SIZE / 2.0;

/// 2D coordinate pair bounded by `LIMIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoordPair<const LIMIT: u32> {
    pub x_coord: u32,
    pub y_coord: u32,
}

impl<const LIMIT: u32> CoordPair<LIMIT> {
    pub const fn new(x_coord: u32, y_coord: u32) -> Self {
        Self { x_coord, y_coord }
    }

    pub fn dec_x(&mut self, val: u32) {
        self.x_coord = self.x_coord.saturating_sub(val);
    }

    pub fn inc_x(&mut self, val: u32) {
        self.x_coord = self.x_coord.saturating_add(val).min(LIMIT - 1);
    }

    pub fn dec_y(&mut self, val: u32) {
        self.y_coord = self.y_coord.saturating_sub(val);
    }

    pub fn inc_y(&mut self, val: u32) {
        self.y_coord = self.y_coord.saturating_add(val).min(LIMIT - 1);
    }

    pub fn is_coord_valid(self) -> bool {
        self.x_coord < LIMIT && self.y_coord < LIMIT
    }

    pub fn normalize(&mut self) -> &mut Self {
        self.x_coord = self.x_coord.min(LIMIT - 1);
        self.y_coord = self.y_coord.min(LIMIT - 1);
        self
    }

    pub fn get_id(self) -> u32 {
        self.y_coord * LIMIT + self.x_coord
    }
}

/// NGrid coordinates in `[0, MAX_NUMBER_OF_GRIDS)`.
pub type GridCoord = CoordPair<MAX_NUMBER_OF_GRIDS>;

/// Map-wide cell coordinates in `[0, TOTAL_NUMBER_OF_CELLS_PER_MAP)`.
pub type CellCoord = CoordPair<TOTAL_NUMBER_OF_CELLS_PER_MAP>;

fn compute_pair<const LIMIT: u32>(
    x: f32,
    y: f32,
    center_offset: f32,
    size: f32,
    center_val: u32,
) -> CoordPair<LIMIT> {
    let x_offset = (f64::from(x) - f64::from(center_offset)) / f64::from(size);
    let y_offset = (f64::from(y) - f64::from(center_offset)) / f64::from(size);

    let x_val = (x_offset + f64::from(center_val) + 0.5) as i32;
    let y_val = (y_offset + f64::from(center_val) + 0.5) as i32;

    CoordPair::new(x_val as u32, y_val as u32)
}

/// Convert world coordinates to TrinityCore `GridCoord`.
pub fn compute_grid_coord(x: f32, y: f32) -> GridCoord {
    compute_pair(x, y, CENTER_GRID_OFFSET, SIZE_OF_GRIDS, CENTER_GRID_ID)
}

/// Convert world coordinates using TrinityCore's simplified terrain formula.
pub fn compute_grid_coord_simple(x: f32, y: f32) -> GridCoord {
    let gx = (CENTER_GRID_ID as f32 - x / SIZE_OF_GRIDS) as i32;
    let gy = (CENTER_GRID_ID as f32 - y / SIZE_OF_GRIDS) as i32;
    let max_grid = MAX_NUMBER_OF_GRIDS as i32 - 1;
    GridCoord::new((max_grid - gx) as u32, (max_grid - gy) as u32)
}

/// Convert world coordinates to TrinityCore map-wide `CellCoord`.
pub fn compute_cell_coord(x: f32, y: f32) -> CellCoord {
    compute_pair(
        x,
        y,
        CENTER_GRID_CELL_OFFSET,
        SIZE_OF_GRID_CELL,
        CENTER_GRID_CELL_ID,
    )
}

/// Convert world coordinates to `CellCoord` and return the intra-cell offset.
pub fn compute_cell_coord_with_offset(x: f32, y: f32) -> (CellCoord, f32, f32) {
    let x_offset =
        (f64::from(x) - f64::from(CENTER_GRID_CELL_OFFSET)) / f64::from(SIZE_OF_GRID_CELL);
    let y_offset =
        (f64::from(y) - f64::from(CENTER_GRID_CELL_OFFSET)) / f64::from(SIZE_OF_GRID_CELL);

    let x_val = (x_offset + f64::from(CENTER_GRID_CELL_ID) + 0.5) as i32;
    let y_val = (y_offset + f64::from(CENTER_GRID_CELL_ID) + 0.5) as i32;

    let x_off = ((x_offset - f64::from(x_val) + f64::from(CENTER_GRID_CELL_ID))
        * f64::from(SIZE_OF_GRID_CELL)) as f32;
    let y_off = ((y_offset - f64::from(y_val) + f64::from(CENTER_GRID_CELL_ID))
        * f64::from(SIZE_OF_GRID_CELL)) as f32;

    (CellCoord::new(x_val as u32, y_val as u32), x_off, y_off)
}

/// Return the parent grid and local 0..7 cell coordinates.
pub fn cell_to_grid_local(cell: CellCoord) -> (GridCoord, u32, u32) {
    let grid = GridCoord::new(
        cell.x_coord / MAX_NUMBER_OF_CELLS,
        cell.y_coord / MAX_NUMBER_OF_CELLS,
    );
    let cell_x = cell.x_coord % MAX_NUMBER_OF_CELLS;
    let cell_y = cell.y_coord % MAX_NUMBER_OF_CELLS;
    (grid, cell_x, cell_y)
}

pub fn normalize_map_coord(c: &mut f32) {
    let limit = MAP_HALFSIZE - 0.5;
    if *c > limit {
        *c = limit;
    } else if *c < -limit {
        *c = -limit;
    }
}

pub fn is_valid_map_coord(c: f32) -> bool {
    c.is_finite() && c.abs() <= MAP_HALFSIZE - 0.5
}

pub fn is_valid_map_coord_2d(x: f32, y: f32) -> bool {
    is_valid_map_coord(x) && is_valid_map_coord(y)
}

pub fn is_valid_map_coord_3d(x: f32, y: f32, z: f32) -> bool {
    is_valid_map_coord_2d(x, y) && is_valid_map_coord(z)
}

pub fn is_valid_map_coord_4d(x: f32, y: f32, z: f32, o: f32) -> bool {
    is_valid_map_coord_3d(x, y, z) && o.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_trinity_grid_defines() {
        assert_eq!(MAX_NUMBER_OF_CELLS, 8);
        assert_eq!(MAX_NUMBER_OF_GRIDS, 64);
        assert!((SIZE_OF_GRIDS - 533.3333).abs() < f32::EPSILON);
        assert_eq!(CENTER_GRID_ID, 32);
        assert_eq!(CENTER_GRID_CELL_ID, 256);
        assert_eq!(TOTAL_NUMBER_OF_CELLS_PER_MAP, 512);
        assert!((SIZE_OF_GRID_CELL - 66.666_66).abs() < 0.0001);
    }

    #[test]
    fn coord_pair_matches_trinity_helpers() {
        let mut pair = GridCoord::new(2, 62);
        pair.dec_x(5);
        pair.inc_y(5);
        assert_eq!(pair, GridCoord::new(0, 63));

        pair.inc_x(99);
        pair.dec_y(99);
        assert_eq!(pair, GridCoord::new(63, 0));
        assert!(pair.is_coord_valid());
        assert_eq!(GridCoord::new(1, 2).get_id(), 129);
    }

    #[test]
    fn compute_grid_coord_matches_trinity_formula() {
        assert_eq!(compute_grid_coord(0.0, 0.0), GridCoord::new(32, 32));
        assert_eq!(
            compute_grid_coord(MAP_HALFSIZE - 0.5, MAP_HALFSIZE - 0.5),
            GridCoord::new(63, 63)
        );
        assert_eq!(
            compute_grid_coord(-(MAP_HALFSIZE - 0.5), -(MAP_HALFSIZE - 0.5)),
            GridCoord::new(0, 0)
        );
    }

    #[test]
    fn compute_cell_coord_and_grid_local_match_trinity_formula() {
        let cell = compute_cell_coord(0.0, 0.0);
        assert_eq!(cell, CellCoord::new(256, 256));
        assert_eq!(cell_to_grid_local(cell), (GridCoord::new(32, 32), 0, 0));

        let cell = compute_cell_coord(SIZE_OF_GRID_CELL, SIZE_OF_GRID_CELL);
        assert_eq!(cell, CellCoord::new(257, 257));
        assert_eq!(cell_to_grid_local(cell), (GridCoord::new(32, 32), 1, 1));
    }

    #[test]
    fn compute_cell_coord_with_offset_matches_trinity_formula() {
        let (cell, x_off, y_off) = compute_cell_coord_with_offset(0.0, 0.0);
        assert_eq!(cell, CellCoord::new(256, 256));
        assert!((x_off + CENTER_GRID_CELL_OFFSET).abs() < 0.0001);
        assert!((y_off + CENTER_GRID_CELL_OFFSET).abs() < 0.0001);
    }

    #[test]
    fn map_coord_validation_matches_trinity_bounds() {
        let limit = MAP_HALFSIZE - 0.5;
        assert!(is_valid_map_coord(limit));
        assert!(is_valid_map_coord(-limit));
        assert!(!is_valid_map_coord(limit + 0.01));
        assert!(!is_valid_map_coord(f32::NAN));

        let mut coord = MAP_HALFSIZE + 10.0;
        normalize_map_coord(&mut coord);
        assert_eq!(coord, limit);
    }
}
