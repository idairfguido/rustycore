//! Map grid lifecycle skeleton.
//!
//! C++ references:
//! - `game/Maps/Map.h`
//! - `game/Maps/Map.cpp`

use std::collections::HashSet;

use crate::cell::Cell;
use crate::coords::{
    CellCoord, GridCoord, MAX_NUMBER_OF_CELLS, MAX_NUMBER_OF_GRIDS, SIZE_OF_GRID_CELL,
    TOTAL_NUMBER_OF_CELLS_PER_MAP, compute_cell_coord,
};
use crate::grid::{GridStateKind, MapGridHost, NGrid, update_grid_state};
use crate::object_grid_loader::{GridSpawnLoadFilter, ObjectGridLoader};
use crate::personal_phase::{MultiPersonalPhaseTracker, PhaseShift};
use crate::spawn::Difficulty;

const GRID_SLOT_COUNT: usize = (MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_GRIDS) as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveObjectKind {
    Player,
    NonPlayer,
}

pub trait TerrainGridLoader {
    fn load_map_and_vmap(&mut self, grid_x: u32, grid_y: u32);
    fn unload_map(&mut self, grid_x: u32, grid_y: u32);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopTerrainGridLoader;

impl TerrainGridLoader for NoopTerrainGridLoader {
    fn load_map_and_vmap(&mut self, _grid_x: u32, _grid_y: u32) {}
    fn unload_map(&mut self, _grid_x: u32, _grid_y: u32) {}
}

pub trait GridLifecycle {
    fn load_grid_objects(&mut self, grid: &mut NGrid, cell: &Cell);
    fn stop_grid_objects(&mut self, grid: &NGrid);
    fn evacuate_grid(&mut self, grid: &mut NGrid);
    fn clean_grid(&mut self, grid: &mut NGrid);
    fn unload_grid_objects(&mut self, grid: &mut NGrid);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopGridLifecycle;

impl GridLifecycle for NoopGridLifecycle {
    fn load_grid_objects(&mut self, _grid: &mut NGrid, _cell: &Cell) {}
    fn stop_grid_objects(&mut self, _grid: &NGrid) {}
    fn evacuate_grid(&mut self, _grid: &mut NGrid) {}
    fn clean_grid(&mut self, _grid: &mut NGrid) {}
    fn unload_grid_objects(&mut self, _grid: &mut NGrid) {}
}

#[derive(Debug)]
pub struct Map<Terrain = NoopTerrainGridLoader, Lifecycle = NoopGridLifecycle> {
    map_id: u32,
    instance_id: u32,
    spawn_mode: Difficulty,
    grid_expiry_ms: i64,
    grid_unload: bool,
    visible_distance: f32,
    grids: Vec<Option<Box<NGrid>>>,
    terrain: Terrain,
    lifecycle: Lifecycle,
    active_cells: HashSet<CellCoord>,
    personal_phase_tracker: MultiPersonalPhaseTracker,
    grid_state_unloaded: bool,
}

impl Map<NoopTerrainGridLoader, NoopGridLifecycle> {
    pub fn new(map_id: u32, instance_id: u32, spawn_mode: Difficulty, grid_expiry_ms: i64) -> Self {
        Self::with_hooks(
            map_id,
            instance_id,
            spawn_mode,
            grid_expiry_ms,
            true,
            100.0,
            NoopTerrainGridLoader,
            NoopGridLifecycle,
        )
    }
}

impl<Terrain, Lifecycle> Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    #[allow(clippy::too_many_arguments)]
    pub fn with_hooks(
        map_id: u32,
        instance_id: u32,
        spawn_mode: Difficulty,
        grid_expiry_ms: i64,
        grid_unload: bool,
        visible_distance: f32,
        terrain: Terrain,
        lifecycle: Lifecycle,
    ) -> Self {
        Self {
            map_id,
            instance_id,
            spawn_mode,
            grid_expiry_ms,
            grid_unload,
            visible_distance,
            grids: std::iter::repeat_with(|| None)
                .take(GRID_SLOT_COUNT)
                .collect(),
            terrain,
            lifecycle,
            active_cells: HashSet::new(),
            personal_phase_tracker: MultiPersonalPhaseTracker::default(),
            grid_state_unloaded: false,
        }
    }

    pub const fn map_id(&self) -> u32 {
        self.map_id
    }

    pub const fn instance_id(&self) -> u32 {
        self.instance_id
    }

    pub const fn spawn_mode(&self) -> Difficulty {
        self.spawn_mode
    }

    pub const fn grid_expiry_ms(&self) -> i64 {
        self.grid_expiry_ms
    }

    pub const fn grid_unload(&self) -> bool {
        self.grid_unload
    }

    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }

    pub fn personal_phase_tracker(&self) -> &MultiPersonalPhaseTracker {
        &self.personal_phase_tracker
    }

    pub fn mark_active_cell(&mut self, cell: CellCoord) {
        assert!(cell.is_coord_valid());
        self.active_cells.insert(cell);
    }

    pub fn unmark_active_cell(&mut self, cell: CellCoord) {
        self.active_cells.remove(&cell);
    }

    pub fn get_ngrid(&self, coord: GridCoord) -> Option<&NGrid> {
        let index = grid_index(coord)?;
        self.grids[index].as_deref()
    }

    pub fn get_ngrid_mut(&mut self, coord: GridCoord) -> Option<&mut NGrid> {
        let index = grid_index(coord)?;
        self.grids[index].as_deref_mut()
    }

    pub fn set_ngrid(&mut self, coord: GridCoord, grid: Option<NGrid>) {
        let index = checked_grid_index(coord);
        self.grids[index] = grid.map(Box::new);
    }

    pub fn is_grid_loaded(&self, coord: GridCoord) -> bool {
        self.get_ngrid(coord)
            .is_some_and(NGrid::grid_object_data_loaded)
    }

    pub fn ensure_grid_created(&mut self, coord: GridCoord) -> bool {
        let index = checked_grid_index(coord);
        if self.grids[index].is_some() {
            return false;
        }

        let mut grid = NGrid::from_coords(
            coord.x_coord as i32,
            coord.y_coord as i32,
            self.grid_expiry_ms,
            self.grid_unload,
        );
        grid.set_state(GridStateKind::Idle);
        self.grids[index] = Some(Box::new(grid));

        let (terrain_x, terrain_y) = terrain_grid_coords(coord);
        self.terrain.load_map_and_vmap(terrain_x, terrain_y);
        true
    }

    pub fn ensure_grid_loaded(&mut self, cell: &Cell) -> bool {
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.ensure_grid_created(coord);
        let index = checked_grid_index(coord);
        let grid = self.grids[index].as_mut().expect("grid was just created");
        if grid.grid_object_data_loaded() {
            return false;
        }

        grid.set_grid_object_data_loaded(true);
        self.lifecycle.load_grid_objects(grid, cell);
        true
    }

    pub fn ensure_grid_loaded_for_active_object(
        &mut self,
        cell: &Cell,
        kind: ActiveObjectKind,
    ) -> bool {
        let loaded_now = self.ensure_grid_loaded(cell);
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.mark_active_cell(cell.cell_coord());

        if matches!(kind, ActiveObjectKind::Player) {
            // Use `ensure_grid_loaded_for_player_phase` when phase-shift state
            // is available; this entry point only has the object kind.
        }

        let active_expiry_ms = (self.grid_expiry_ms as f32 * 0.1) as i64;
        let grid = self.get_ngrid_mut(coord).expect("grid was just loaded");
        if grid.state() != GridStateKind::Active {
            grid.info_mut().reset_time_tracker(active_expiry_ms);
            grid.set_state(GridStateKind::Active);
        }

        loaded_now
    }

    pub fn ensure_grid_loaded_for_player_phase<Filter>(
        &mut self,
        cell: &Cell,
        phase_shift: &PhaseShift,
        loader: &mut ObjectGridLoader<'_, Filter>,
    ) -> bool
    where
        Filter: GridSpawnLoadFilter,
    {
        let loaded_now = self.ensure_grid_loaded(cell);
        let coord = GridCoord::new(cell.grid_x(), cell.grid_y());
        self.mark_active_cell(cell.cell_coord());

        let active_expiry_ms = (self.grid_expiry_ms as f32 * 0.1) as i64;
        let index = checked_grid_index(coord);
        let grid = self.grids[index].as_mut().expect("grid was just loaded");
        self.personal_phase_tracker
            .load_grid(phase_shift, grid, loader);

        if grid.state() != GridStateKind::Active {
            grid.info_mut().reset_time_tracker(active_expiry_ms);
            grid.set_state(GridStateKind::Active);
        }

        loaded_now
    }

    pub fn load_grid(&mut self, x: f32, y: f32) -> bool {
        self.ensure_grid_loaded(&Cell::from_world(x, y))
    }

    pub fn load_grid_for_active_object(&mut self, x: f32, y: f32, kind: ActiveObjectKind) -> bool {
        self.ensure_grid_loaded_for_active_object(&Cell::from_world(x, y), kind)
    }

    pub fn reset_grid_expiry(&self, grid: &mut NGrid, factor: f32) {
        grid.info_mut()
            .reset_time_tracker((self.grid_expiry_ms as f32 * factor) as i64);
    }

    pub fn active_objects_near_grid(&self, grid: &NGrid) -> bool {
        active_cells_near_grid(&self.active_cells, self.visible_distance, grid)
    }

    pub fn unload_grid_at(&mut self, coord: GridCoord, unload_all: bool) -> bool {
        let index = checked_grid_index(coord);
        let Some(mut grid) = self.grids[index].take() else {
            return false;
        };

        if !self.can_unload_grid(&grid, unload_all) {
            self.grids[index] = Some(grid);
            return false;
        }

        self.run_unload_lifecycle(&mut grid, unload_all);
        true
    }

    pub fn update_grid_state_at(&mut self, coord: GridCoord, diff_ms: u32) -> bool {
        let index = checked_grid_index(coord);
        let Some(mut grid) = self.grids[index].take() else {
            return false;
        };

        self.grid_state_unloaded = false;
        update_grid_state(self, &mut grid, diff_ms);
        if self.grid_state_unloaded {
            self.grid_state_unloaded = false;
            true
        } else {
            self.grids[index] = Some(grid);
            false
        }
    }

    fn can_unload_grid(&self, grid: &NGrid, unload_all: bool) -> bool {
        unload_all
            || (grid.world_creature_count_in_ngrid() == 0 && !self.active_objects_near_grid(grid))
    }

    fn run_unload_lifecycle(&mut self, grid: &mut NGrid, unload_all: bool) {
        if !unload_all {
            self.lifecycle.evacuate_grid(grid);
        }

        self.lifecycle.clean_grid(grid);
        self.lifecycle.unload_grid_objects(grid);

        let coord = GridCoord::new(grid.x() as u32, grid.y() as u32);
        let (terrain_x, terrain_y) = terrain_grid_coords(coord);
        self.terrain.unload_map(terrain_x, terrain_y);
    }
}

impl<Terrain, Lifecycle> MapGridHost for Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    fn active_objects_near_grid(&self, grid: &NGrid) -> bool {
        Map::active_objects_near_grid(self, grid)
    }

    fn stop_grid_objects(&mut self, grid: &NGrid) {
        self.lifecycle.stop_grid_objects(grid);
    }

    fn reset_grid_expiry(&mut self, grid: &mut NGrid, factor: f32) {
        Map::reset_grid_expiry(self, grid, factor);
    }

    fn unload_grid(&mut self, grid: &mut NGrid, unload_all: bool) -> bool {
        if !self.can_unload_grid(grid, unload_all) {
            return false;
        }

        self.run_unload_lifecycle(grid, unload_all);
        self.grid_state_unloaded = true;
        true
    }
}

fn grid_index(coord: GridCoord) -> Option<usize> {
    coord
        .is_coord_valid()
        .then_some((coord.x_coord * MAX_NUMBER_OF_GRIDS + coord.y_coord) as usize)
}

fn checked_grid_index(coord: GridCoord) -> usize {
    grid_index(coord).expect("grid coordinates must be within MAX_NUMBER_OF_GRIDS")
}

fn terrain_grid_coords(coord: GridCoord) -> (u32, u32) {
    (
        (MAX_NUMBER_OF_GRIDS - 1) - coord.x_coord,
        (MAX_NUMBER_OF_GRIDS - 1) - coord.y_coord,
    )
}

fn active_cells_near_grid(
    active_cells: &HashSet<CellCoord>,
    visible_distance: f32,
    grid: &NGrid,
) -> bool {
    let mut cell_min = CellCoord::new(
        grid.x() as u32 * MAX_NUMBER_OF_CELLS,
        grid.y() as u32 * MAX_NUMBER_OF_CELLS,
    );
    let mut cell_max = CellCoord::new(
        cell_min.x_coord + MAX_NUMBER_OF_CELLS,
        cell_min.y_coord + MAX_NUMBER_OF_CELLS,
    );
    let cell_range = (visible_distance / SIZE_OF_GRID_CELL).ceil() as u32 + 1;

    cell_min.dec_x(cell_range);
    cell_min.dec_y(cell_range);
    cell_max.inc_x(cell_range);
    cell_max.inc_y(cell_range);

    active_cells.iter().any(|cell| {
        cell_min.x_coord <= cell.x_coord
            && cell.x_coord <= cell_max.x_coord
            && cell_min.y_coord <= cell.y_coord
            && cell.y_coord <= cell_max.y_coord
    })
}

pub fn is_grid_id_loaded<Terrain, Lifecycle>(map: &Map<Terrain, Lifecycle>, grid_id: u32) -> bool
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    map.is_grid_loaded(GridCoord::new(
        grid_id % MAX_NUMBER_OF_GRIDS,
        grid_id / MAX_NUMBER_OF_GRIDS,
    ))
}

pub fn cell_from_grid_center(coord: GridCoord) -> Cell {
    let cell = CellCoord::new(
        coord.x_coord * MAX_NUMBER_OF_CELLS,
        coord.y_coord * MAX_NUMBER_OF_CELLS,
    );
    Cell::from_cell_coord(cell)
}

pub fn cell_from_world(x: f32, y: f32) -> Cell {
    Cell::from_cell_coord(compute_cell_coord(x, y))
}

pub const fn total_cell_count() -> u32 {
    TOTAL_NUMBER_OF_CELLS_PER_MAP * TOTAL_NUMBER_OF_CELLS_PER_MAP
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::ObjectGuid;

    #[derive(Debug, Default)]
    struct RecordingTerrain {
        loads: Vec<(u32, u32)>,
        unloads: Vec<(u32, u32)>,
    }

    impl TerrainGridLoader for RecordingTerrain {
        fn load_map_and_vmap(&mut self, grid_x: u32, grid_y: u32) {
            self.loads.push((grid_x, grid_y));
        }

        fn unload_map(&mut self, grid_x: u32, grid_y: u32) {
            self.unloads.push((grid_x, grid_y));
        }
    }

    #[derive(Debug, Default)]
    struct RecordingLifecycle {
        loads: usize,
        stops: usize,
        evacuates: usize,
        cleans: usize,
        unloads: usize,
    }

    impl GridLifecycle for RecordingLifecycle {
        fn load_grid_objects(&mut self, _grid: &mut NGrid, _cell: &Cell) {
            self.loads += 1;
        }

        fn stop_grid_objects(&mut self, _grid: &NGrid) {
            self.stops += 1;
        }

        fn evacuate_grid(&mut self, _grid: &mut NGrid) {
            self.evacuates += 1;
        }

        fn clean_grid(&mut self, _grid: &mut NGrid) {
            self.cleans += 1;
        }

        fn unload_grid_objects(&mut self, _grid: &mut NGrid) {
            self.unloads += 1;
        }
    }

    fn test_map() -> Map<RecordingTerrain, RecordingLifecycle> {
        Map::with_hooks(
            571,
            7,
            1,
            1000,
            true,
            100.0,
            RecordingTerrain::default(),
            RecordingLifecycle::default(),
        )
    }

    #[test]
    fn map_constructor_starts_with_empty_grid_slots_like_cpp_pointer_array() {
        let map = test_map();

        assert_eq!(map.map_id(), 571);
        assert_eq!(map.instance_id(), 7);
        assert_eq!(map.spawn_mode(), 1);
        assert_eq!(map.grid_expiry_ms(), 1000);
        assert!(map.grid_unload());
        assert_eq!(map.grids.len(), GRID_SLOT_COUNT);
        assert!(map.grids.iter().all(Option::is_none));
    }

    #[test]
    fn ensure_grid_created_sets_idle_grid_and_loads_reversed_terrain_coords() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);

        assert!(map.ensure_grid_created(coord));
        assert!(!map.ensure_grid_created(coord));

        let grid = map.get_ngrid(coord).unwrap();
        assert_eq!(grid.grid_id(), 2 * MAX_NUMBER_OF_GRIDS + 3);
        assert_eq!(grid.state(), GridStateKind::Idle);
        assert!(!grid.grid_object_data_loaded());
        assert_eq!(map.terrain().loads, vec![(61, 60)]);
    }

    #[test]
    fn ensure_grid_loaded_marks_loaded_before_object_loader_hook() {
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(2, 3));

        assert!(map.ensure_grid_loaded(&cell));
        assert!(!map.ensure_grid_loaded(&cell));

        assert!(map.is_grid_loaded(GridCoord::new(2, 3)));
        assert_eq!(map.lifecycle().loads, 1);
    }

    #[test]
    fn active_object_loading_sets_grid_active_and_short_expiry() {
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(2, 3));

        assert!(map.ensure_grid_loaded_for_active_object(&cell, ActiveObjectKind::NonPlayer));

        let grid = map.get_ngrid(GridCoord::new(2, 3)).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        assert_eq!(grid.info().time_tracker().remaining_ms(), 100);
        assert!(map.active_objects_near_grid(grid));
    }

    #[test]
    fn player_phase_loading_invokes_personal_phase_tracker_before_activation() {
        let mut store = crate::spawn::SpawnStore::new();
        let spawn = crate::spawn::SpawnData {
            object_type: crate::spawn::SpawnObjectType::Creature,
            spawn_id: 100,
            map_id: 571,
            db_data: true,
            id: 42,
            spawn_point: crate::spawn::SpawnPosition::new(0.0, 0.0, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id: 9,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![1],
            script_id: 0,
            string_id: String::new(),
        };
        store.add_object_spawn(&spawn, |phase_id| phase_id == 9);
        let corpses = crate::object_grid_loader::CorpseCellStore::new();
        let mut loader =
            crate::object_grid_loader::ObjectGridLoader::new(&store, &corpses, 571, 1, 1, 1);
        let owner = ObjectGuid::create_player(1, 100);
        let phase_shift = crate::personal_phase::PhaseShift::new(
            Some(owner),
            vec![crate::personal_phase::PhaseRef::new(9, true)],
        );
        let mut map = test_map();
        let cell = cell_from_grid_center(GridCoord::new(32, 32));

        assert!(map.ensure_grid_loaded_for_player_phase(&cell, &phase_shift, &mut loader));

        let grid = map.get_ngrid(GridCoord::new(32, 32)).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        assert_eq!(
            grid.get_grid_type(0, 0)
                .unwrap()
                .grid_objects
                .creatures
                .len(),
            1
        );
        assert_eq!(map.personal_phase_tracker().tracker_count(), 1);
    }

    #[test]
    fn unload_grid_refuses_world_creatures_and_active_neighbors_unless_forced() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        let cell = cell_from_grid_center(coord);
        map.ensure_grid_loaded(&cell);
        map.get_ngrid_mut(coord)
            .unwrap()
            .get_grid_type_mut(0, 0)
            .unwrap()
            .world_objects
            .creatures
            .insert(ObjectGuid::new(1, 1));

        assert!(!map.unload_grid_at(coord, false));
        assert!(map.is_grid_loaded(coord));

        assert!(map.unload_grid_at(coord, true));
        assert!(map.get_ngrid(coord).is_none());
        assert_eq!(map.lifecycle().evacuates, 0);
        assert_eq!(map.lifecycle().cleans, 1);
        assert_eq!(map.lifecycle().unloads, 1);
        assert_eq!(map.terrain().unloads, vec![(61, 60)]);
    }

    #[test]
    fn update_grid_state_at_removes_grid_when_removal_unloads_successfully() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        map.ensure_grid_loaded(&cell_from_grid_center(coord));
        map.get_ngrid_mut(coord)
            .unwrap()
            .set_state(GridStateKind::Removal);

        assert!(map.update_grid_state_at(coord, 1001));

        assert!(map.get_ngrid(coord).is_none());
        assert_eq!(map.lifecycle().evacuates, 1);
        assert_eq!(map.lifecycle().cleans, 1);
        assert_eq!(map.lifecycle().unloads, 1);
    }

    #[test]
    fn active_objects_near_grid_matches_cpp_cell_range_expansion() {
        let mut map = test_map();
        let coord = GridCoord::new(10, 10);
        map.ensure_grid_created(coord);
        let grid = map.get_ngrid(coord).unwrap();
        assert!(!map.active_objects_near_grid(grid));

        map.mark_active_cell(CellCoord::new(79, 80));
        let grid = map.get_ngrid(coord).unwrap();
        assert!(map.active_objects_near_grid(grid));

        map.unmark_active_cell(CellCoord::new(79, 80));
        map.mark_active_cell(CellCoord::new(1, 1));
        let grid = map.get_ngrid(coord).unwrap();
        assert!(!map.active_objects_near_grid(grid));
    }

    #[test]
    fn grid_id_loaded_uses_cpp_public_grid_id_decomposition() {
        let mut map = test_map();
        let coord = GridCoord::new(2, 3);
        map.ensure_grid_loaded(&cell_from_grid_center(coord));

        assert!(is_grid_id_loaded(&map, 3 * MAX_NUMBER_OF_GRIDS + 2));
    }
}
