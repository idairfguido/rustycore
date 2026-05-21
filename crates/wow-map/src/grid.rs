//! NGrid and grid-state primitives.
//!
//! C++ references:
//! - `game/Grids/NGrid.h`
//! - `game/Grids/NGrid.cpp`
//! - `game/Grids/GridStates.cpp`

use crate::cell::Cell;
use crate::coords::{CellCoord, MAX_NUMBER_OF_CELLS, MAX_NUMBER_OF_GRIDS};

pub const DEFAULT_VISIBILITY_NOTIFY_PERIOD: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeTracker {
    remaining_ms: i64,
}

impl TimeTracker {
    pub const fn new(expiry_ms: i64) -> Self {
        Self {
            remaining_ms: expiry_ms,
        }
    }

    pub fn reset(&mut self, interval_ms: i64) {
        self.remaining_ms = interval_ms;
    }

    pub fn update(&mut self, diff_ms: u32) {
        self.remaining_ms -= i64::from(diff_ms);
    }

    pub const fn passed(self) -> bool {
        self.remaining_ms <= 0
    }

    pub const fn remaining_ms(self) -> i64 {
        self.remaining_ms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodicTimer {
    period_ms: i64,
    expire_time_ms: i64,
}

impl PeriodicTimer {
    pub const fn new(period_ms: i64, start_time_ms: i64) -> Self {
        Self {
            period_ms,
            expire_time_ms: start_time_ms,
        }
    }

    pub fn update(&mut self, diff_ms: u32) -> bool {
        self.expire_time_ms -= i64::from(diff_ms);
        if self.expire_time_ms > 0 {
            false
        } else {
            self.reset_after_update(diff_ms);
            true
        }
    }

    pub const fn period_ms(self) -> i64 {
        self.period_ms
    }

    pub const fn expire_time_ms(self) -> i64 {
        self.expire_time_ms
    }

    pub fn tracker_update(&mut self, diff_ms: u32) {
        self.expire_time_ms -= i64::from(diff_ms);
    }

    pub const fn tracker_passed(self) -> bool {
        self.expire_time_ms <= 0
    }

    pub fn tracker_reset(&mut self, diff_ms: u32, period_ms: i64) {
        self.expire_time_ms += period_ms.max(i64::from(diff_ms));
    }

    fn reset_after_update(&mut self, diff_ms: u32) {
        self.tracker_reset(diff_ms, self.period_ms);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridInfo {
    timer: TimeTracker,
    relocation_timer: PeriodicTimer,
    unload_active_lock_count: u16,
    unload_explicit_lock: bool,
}

impl Default for GridInfo {
    fn default() -> Self {
        Self::new(0, true)
    }
}

impl GridInfo {
    pub fn new(expiry_ms: i64, unload: bool) -> Self {
        Self {
            timer: TimeTracker::new(expiry_ms),
            relocation_timer: PeriodicTimer::new(0, DEFAULT_VISIBILITY_NOTIFY_PERIOD),
            unload_active_lock_count: 0,
            unload_explicit_lock: !unload,
        }
    }

    pub const fn time_tracker(&self) -> TimeTracker {
        self.timer
    }

    pub const fn unload_lock(&self) -> bool {
        self.unload_active_lock_count > 0 || self.unload_explicit_lock
    }

    pub const fn unload_active_lock_count(&self) -> u16 {
        self.unload_active_lock_count
    }

    pub fn set_unload_explicit_lock(&mut self, on: bool) {
        self.unload_explicit_lock = on;
    }

    pub fn inc_unload_active_lock(&mut self) {
        self.unload_active_lock_count = self.unload_active_lock_count.saturating_add(1);
    }

    pub fn dec_unload_active_lock(&mut self) {
        if self.unload_active_lock_count > 0 {
            self.unload_active_lock_count -= 1;
        }
    }

    pub fn reset_time_tracker(&mut self, interval_ms: i64) {
        self.timer.reset(interval_ms);
    }

    pub fn update_time_tracker(&mut self, diff_ms: u32) {
        self.timer.update(diff_ms);
    }

    pub const fn relocation_timer(&self) -> PeriodicTimer {
        self.relocation_timer
    }

    pub fn relocation_timer_mut(&mut self) -> &mut PeriodicTimer {
        &mut self.relocation_timer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GridStateKind {
    Invalid = 0,
    Active = 1,
    Idle = 2,
    Removal = 3,
}

#[derive(Debug, Clone)]
pub struct NGrid {
    grid_id: u32,
    info: GridInfo,
    x: i32,
    y: i32,
    state: GridStateKind,
    cells: [[Cell; MAX_NUMBER_OF_CELLS as usize]; MAX_NUMBER_OF_CELLS as usize],
    grid_object_data_loaded: bool,
}

impl NGrid {
    pub fn new(grid_id: u32, x: i32, y: i32, expiry_ms: i64, unload: bool) -> Self {
        Self {
            grid_id,
            info: GridInfo::new(expiry_ms, unload),
            x,
            y,
            state: GridStateKind::Invalid,
            cells: std::array::from_fn(|cell_x| {
                std::array::from_fn(|cell_y| {
                    let map_cell_x = (x as u32) * MAX_NUMBER_OF_CELLS + cell_x as u32;
                    let map_cell_y = (y as u32) * MAX_NUMBER_OF_CELLS + cell_y as u32;
                    Cell::from_cell_coord(CellCoord::new(map_cell_x, map_cell_y))
                })
            }),
            grid_object_data_loaded: false,
        }
    }

    pub fn from_coords(x: i32, y: i32, expiry_ms: i64, unload: bool) -> Self {
        let grid_id = (x as u32) * MAX_NUMBER_OF_GRIDS + y as u32;
        Self::new(grid_id, x, y, expiry_ms, unload)
    }

    pub const fn grid_id(&self) -> u32 {
        self.grid_id
    }

    pub const fn state(&self) -> GridStateKind {
        self.state
    }

    pub fn set_state(&mut self, state: GridStateKind) {
        self.state = state;
    }

    pub const fn x(&self) -> i32 {
        self.x
    }

    pub const fn y(&self) -> i32 {
        self.y
    }

    pub const fn grid_object_data_loaded(&self) -> bool {
        self.grid_object_data_loaded
    }

    pub fn set_grid_object_data_loaded(&mut self, loaded: bool) {
        self.grid_object_data_loaded = loaded;
    }

    pub const fn info(&self) -> &GridInfo {
        &self.info
    }

    pub fn info_mut(&mut self) -> &mut GridInfo {
        &mut self.info
    }

    pub fn get_grid_type(&self, x: u32, y: u32) -> Option<&Cell> {
        self.cells.get(x as usize)?.get(y as usize)
    }

    pub fn get_grid_type_mut(&mut self, x: u32, y: u32) -> Option<&mut Cell> {
        self.cells.get_mut(x as usize)?.get_mut(y as usize)
    }

    pub fn visit_all_grids<F>(&self, mut visitor: F)
    where
        F: FnMut(&Cell),
    {
        for x in 0..MAX_NUMBER_OF_CELLS as usize {
            for y in 0..MAX_NUMBER_OF_CELLS as usize {
                visitor(&self.cells[x][y]);
            }
        }
    }

    pub fn visit_all_grids_mut<F>(&mut self, mut visitor: F)
    where
        F: FnMut(&mut Cell),
    {
        for x in 0..MAX_NUMBER_OF_CELLS as usize {
            for y in 0..MAX_NUMBER_OF_CELLS as usize {
                visitor(&mut self.cells[x][y]);
            }
        }
    }

    pub fn player_count_in_ngrid(&self) -> usize {
        let mut count = 0usize;
        self.visit_all_grids(|cell| {
            count += cell.world_objects.players.len();
        });
        count
    }

    pub fn world_creature_count_in_ngrid(&self) -> usize {
        let mut count = 0usize;
        self.visit_all_grids(|cell| {
            count += cell.world_objects.creatures.len();
        });
        count
    }
}

pub trait MapGridHost {
    fn active_objects_near_grid(&self, grid: &NGrid) -> bool;
    fn stop_grid_objects(&mut self, grid: &NGrid);
    fn reset_grid_expiry(&mut self, grid: &mut NGrid, factor: f32);
    fn unload_grid(&mut self, grid: &mut NGrid, unload_all: bool) -> bool;
}

pub fn update_grid_state<H: MapGridHost>(host: &mut H, grid: &mut NGrid, diff_ms: u32) {
    match grid.state() {
        GridStateKind::Invalid => {}
        GridStateKind::Active => update_active_grid(host, grid, diff_ms),
        GridStateKind::Idle => update_idle_grid(host, grid),
        GridStateKind::Removal => update_removal_grid(host, grid, diff_ms),
    }
}

fn update_active_grid<H: MapGridHost>(host: &mut H, grid: &mut NGrid, diff_ms: u32) {
    grid.info_mut().update_time_tracker(diff_ms);
    if grid.info().time_tracker().passed() {
        if grid.player_count_in_ngrid() == 0 && !host.active_objects_near_grid(grid) {
            host.stop_grid_objects(grid);
            grid.set_state(GridStateKind::Idle);
        } else {
            host.reset_grid_expiry(grid, 0.1);
        }
    }
}

fn update_idle_grid<H: MapGridHost>(host: &mut H, grid: &mut NGrid) {
    host.reset_grid_expiry(grid, 1.0);
    grid.set_state(GridStateKind::Removal);
}

fn update_removal_grid<H: MapGridHost>(host: &mut H, grid: &mut NGrid, diff_ms: u32) {
    if !grid.info().unload_lock() {
        grid.info_mut().update_time_tracker(diff_ms);
        if grid.info().time_tracker().passed() && !host.unload_grid(grid, false) {
            host.reset_grid_expiry(grid, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::ObjectGuid;

    #[derive(Default)]
    struct TestHost {
        active_near: bool,
        unload_result: bool,
        stop_calls: usize,
        reset_calls: Vec<f32>,
        unload_calls: usize,
    }

    impl MapGridHost for TestHost {
        fn active_objects_near_grid(&self, _grid: &NGrid) -> bool {
            self.active_near
        }

        fn stop_grid_objects(&mut self, _grid: &NGrid) {
            self.stop_calls += 1;
        }

        fn reset_grid_expiry(&mut self, grid: &mut NGrid, factor: f32) {
            grid.info_mut().reset_time_tracker((1000.0 * factor) as i64);
            self.reset_calls.push(factor);
        }

        fn unload_grid(&mut self, _grid: &mut NGrid, _unload_all: bool) -> bool {
            self.unload_calls += 1;
            self.unload_result
        }
    }

    #[test]
    fn grid_state_values_match_ngrid_h() {
        assert_eq!(GridStateKind::Invalid as u8, 0);
        assert_eq!(GridStateKind::Active as u8, 1);
        assert_eq!(GridStateKind::Idle as u8, 2);
        assert_eq!(GridStateKind::Removal as u8, 3);
    }

    #[test]
    fn grid_info_unload_locks_match_cpp_semantics() {
        let mut info = GridInfo::new(500, true);
        assert!(!info.unload_lock());
        assert_eq!(info.time_tracker().remaining_ms(), 500);
        assert_eq!(info.relocation_timer().period_ms(), 0);
        assert_eq!(
            info.relocation_timer().expire_time_ms(),
            DEFAULT_VISIBILITY_NOTIFY_PERIOD
        );

        info.inc_unload_active_lock();
        assert!(info.unload_lock());
        info.dec_unload_active_lock();
        assert!(!info.unload_lock());
        info.dec_unload_active_lock();
        assert!(!info.unload_lock());

        let info = GridInfo::new(500, false);
        assert!(info.unload_lock());
    }

    #[test]
    fn periodic_timer_matches_timer_h_tracker_semantics() {
        let mut timer = PeriodicTimer::new(0, 100);

        timer.tracker_update(40);
        assert!(!timer.tracker_passed());
        assert_eq!(timer.expire_time_ms(), 60);

        timer.tracker_update(70);
        assert!(timer.tracker_passed());
        assert_eq!(timer.expire_time_ms(), -10);

        timer.tracker_reset(20, DEFAULT_VISIBILITY_NOTIFY_PERIOD);
        assert_eq!(timer.expire_time_ms(), 990);

        timer.tracker_reset(1500, DEFAULT_VISIBILITY_NOTIFY_PERIOD);
        assert_eq!(timer.expire_time_ms(), 2490);
    }

    #[test]
    fn ngrid_initializes_like_cpp_constructor() {
        let grid = NGrid::from_coords(2, 3, 1000, true);
        assert_eq!(grid.grid_id(), 2 * MAX_NUMBER_OF_GRIDS + 3);
        assert_eq!(grid.x(), 2);
        assert_eq!(grid.y(), 3);
        assert_eq!(grid.state(), GridStateKind::Invalid);
        assert!(!grid.grid_object_data_loaded());

        let cell = grid.get_grid_type(1, 2).unwrap();
        assert_eq!(cell.grid_x(), 2);
        assert_eq!(cell.grid_y(), 3);
        assert_eq!(cell.cell_x(), 1);
        assert_eq!(cell.cell_y(), 2);
        assert!(grid.get_grid_type(MAX_NUMBER_OF_CELLS, 0).is_none());
    }

    #[test]
    fn active_state_moves_to_idle_when_timer_passed_and_grid_empty() {
        let mut host = TestHost::default();
        let mut grid = NGrid::from_coords(1, 1, 10, true);
        grid.set_state(GridStateKind::Active);

        update_grid_state(&mut host, &mut grid, 11);

        assert_eq!(grid.state(), GridStateKind::Idle);
        assert_eq!(host.stop_calls, 1);
        assert!(host.reset_calls.is_empty());
    }

    #[test]
    fn active_state_resets_expiry_when_players_or_active_objects_exist() {
        let mut host = TestHost {
            active_near: true,
            ..Default::default()
        };
        let mut grid = NGrid::from_coords(1, 1, 10, true);
        grid.set_state(GridStateKind::Active);
        grid.get_grid_type_mut(0, 0)
            .unwrap()
            .world_objects
            .players
            .insert(ObjectGuid::new(1, 1));

        update_grid_state(&mut host, &mut grid, 11);

        assert_eq!(grid.state(), GridStateKind::Active);
        assert_eq!(host.reset_calls, vec![0.1]);
        assert_eq!(host.stop_calls, 0);
    }

    #[test]
    fn idle_state_moves_to_removal_after_reset() {
        let mut host = TestHost::default();
        let mut grid = NGrid::from_coords(1, 1, 10, true);
        grid.set_state(GridStateKind::Idle);

        update_grid_state(&mut host, &mut grid, 1);

        assert_eq!(grid.state(), GridStateKind::Removal);
        assert_eq!(host.reset_calls, vec![1.0]);
    }

    #[test]
    fn removal_state_resets_when_unload_is_deferred() {
        let mut host = TestHost::default();
        let mut grid = NGrid::from_coords(1, 1, 10, true);
        grid.set_state(GridStateKind::Removal);

        update_grid_state(&mut host, &mut grid, 11);

        assert_eq!(host.unload_calls, 1);
        assert_eq!(host.reset_calls, vec![1.0]);
    }

    #[test]
    fn removal_state_does_not_tick_while_unload_locked() {
        let mut host = TestHost::default();
        let mut grid = NGrid::from_coords(1, 1, 10, true);
        grid.set_state(GridStateKind::Removal);
        grid.info_mut().set_unload_explicit_lock(true);

        update_grid_state(&mut host, &mut grid, 11);

        assert_eq!(host.unload_calls, 0);
        assert!(host.reset_calls.is_empty());
        assert_eq!(grid.info().time_tracker().remaining_ms(), 10);
    }
}
