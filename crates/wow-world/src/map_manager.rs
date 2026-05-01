use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tracing::{debug, info, trace, warn};
use wow_core::{ObjectGuid, Position};
use wow_ai::CreatureState;
use wow_packet::packets::update::CreatureCreateData;

/// Size of a grid cell in yards (64x64 yards like TrinityCore).
pub const GRID_SIZE: f32 = 64.0;

/// Visibility radius in yards (how far a player can see).
pub const VISIBILITY_RADIUS: f32 = 100.0;

/// Default time before a grid unloads if no players are nearby (5 minutes).
pub const DEFAULT_GRID_UNLOAD_TIME: Duration = Duration::from_secs(300);

/// Coordinate of a grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCoord {
    pub x: i16,
    pub y: i16,
}

impl GridCoord {
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    /// Get surrounding coordinates in a 3x3 area (including self).
    pub fn surrounding(&self) -> Vec<GridCoord> {
        let mut coords = Vec::with_capacity(9);
        for dx in -1..=1 {
            for dy in -1..=1 {
                coords.push(GridCoord::new(self.x + dx, self.y + dy));
            }
        }
        coords
    }

    /// Check if another coordinate is within a given range.
    pub fn distance_squared(&self, other: &GridCoord) -> i32 {
        let dx = (self.x - other.x) as i32;
        let dy = (self.y - other.y) as i32;
        dx * dx + dy * dy
    }
}

/// A creature stored in the global map system.
#[derive(Debug, Clone)]
pub struct WorldCreature {
    pub guid: ObjectGuid,
    pub entry: u32,
    pub level: u8,
    pub is_alive: bool,
    pub current_hp: u32,
    pub max_hp: u32,
    pub position: Position,
    pub home_pos: Position,
    pub state: CreatureState,
    pub move_target: Option<Position>,
    pub corpse_despawn_at: Option<Instant>,
    pub npc_flags: u32,
    pub unit_flags: u32,
    pub aggro_radius: f32,
    pub min_dmg: u32,
    pub max_dmg: u32,
    pub create_data: CreatureCreateData,
    // Additional fields migrated from CreatureAI
    pub display_id: u32,
    pub faction: u32,
    pub respawn_time_secs: u64,
    pub move_start: Instant,
    pub move_duration_ms: u32,
    pub spline_id: u32,
    pub wander_timer: Instant,
    pub wander_delay_ms: u64,
    pub combat_target: Option<ObjectGuid>,
    pub last_swing: Instant,
    pub swing_timer_ms: u64,
    pub wander_radius: f32,
    pub death_time: Option<Instant>,
}

impl WorldCreature {
    pub fn new(
        guid: ObjectGuid,
        entry: u32,
        pos: Position,
        hp: u32,
        level: u8,
        min_dmg: u32,
        max_dmg: u32,
        aggro_radius: f32,
        display_id: u32,
        faction: u32,
        npc_flags: u32,
        unit_flags: u32,
    ) -> Self {
        let now = Instant::now();
        let (min_dmg, max_dmg) = if min_dmg == 0 {
            let base = (level as u32) * 3 + 5;
            (base, base + base / 2)
        } else {
            (min_dmg, max_dmg)
        };
        Self {
            guid,
            entry,
            home_pos: pos.clone(),
            position: pos.clone(),
            move_target: None,
            move_start: now,
            move_duration_ms: 0,
            spline_id: 1,
            state: CreatureState::Idle,
            wander_timer: now,
            wander_delay_ms: 8_000,
            current_hp: hp,
            max_hp: hp,
            level,
            min_dmg,
            max_dmg,
            combat_target: None,
            last_swing: now,
            swing_timer_ms: 2_000,
            aggro_radius,
            wander_radius: 5.0,
            is_alive: true,
            death_time: None,
            respawn_time_secs: 30,
            corpse_despawn_at: None,
            npc_flags,
            unit_flags,
            display_id,
            faction,
            create_data: CreatureCreateData {
                guid,
                entry,
                display_id,
                native_display_id: display_id,
                health: hp as i64,
                max_health: hp as i64,
                level,
                faction_template: faction as i32,
                npc_flags: npc_flags as u64,
                unit_flags,
                unit_flags2: 0,
                unit_flags3: 0,
                scale: 1.0,
                unit_class: 1,
                base_attack_time: 2000,
                ranged_attack_time: 0,
                zone_id: 0,
                speed_walk_rate: 1.0,
                speed_run_rate: 1.14286,
            },
        }
    }

    pub fn enter_combat(&mut self, attacker: ObjectGuid) {
        self.state = CreatureState::InCombat;
        self.combat_target = Some(attacker);
        self.move_target = None;
        debug!("Creature {:?} entered combat with {:?}", self.guid, attacker);
    }

    pub fn reset_combat(&mut self) {
        self.state = CreatureState::Returning;
        self.combat_target = None;
        self.current_hp = self.max_hp;
        self.move_target = Some(self.home_pos.clone());
    }

    pub fn take_damage(&mut self, damage: u32) -> bool {
        if !self.is_alive { return false; }
        self.current_hp = self.current_hp.saturating_sub(damage);
        if self.current_hp == 0 {
            self.die();
            return true;
        }
        false
    }

    pub fn die(&mut self) {
        self.is_alive = false;
        self.state = CreatureState::Dead;
        self.combat_target = None;
        self.death_time = Some(Instant::now());
    }

    pub fn can_wander(&self) -> bool {
        self.npc_flags == 0 || (self.npc_flags & 0x80) == 0
    }

    pub fn try_aggro(&mut self, player_guid: ObjectGuid, player_pos: &Position) -> bool {
        if !self.is_alive || self.state == CreatureState::InCombat {
            return false;
        }
        let dist = self.position.distance(player_pos);
        if dist <= self.aggro_radius {
            self.enter_combat(player_guid);
            return true;
        }
        false
    }

    pub fn should_respawn(&self) -> bool {
        if let Some(dt) = self.death_time {
            dt.elapsed().as_secs() >= self.respawn_time_secs
        } else {
            false
        }
    }

    pub fn respawn(&mut self) {
        self.current_hp = self.max_hp;
        self.is_alive = true;
        self.state = CreatureState::Idle;
        self.position = self.home_pos.clone();
        self.move_target = None;
        self.death_time = None;
        self.spline_id += 1;
        self.wander_timer = Instant::now();
    }

    pub fn movement_finished(&self) -> bool {
        if self.move_target.is_none() { return true; }
        self.move_start.elapsed().as_millis() as u32 >= self.move_duration_ms
    }

    pub fn interpolated_position(&self) -> Position {
        let Some(ref dst) = self.move_target else { return self.position.clone(); };
        let elapsed = self.move_start.elapsed().as_millis() as f32;
        let total = self.move_duration_ms as f32;
        if total <= 0.0 { return dst.clone(); }
        let t = (elapsed / total).min(1.0);
        Position::new(
            self.position.x + (dst.x - self.position.x) * t,
            self.position.y + (dst.y - self.position.y) * t,
            self.position.z + (dst.z - self.position.z) * t,
            dst.orientation,
        )
    }

    pub fn begin_move(&mut self, dst: Position) {
        let dist = self.position.distance(&dst);
        let walk_speed = 2.5f32;
        let duration_ms = ((dist / walk_speed) * 1000.0) as u32;
        self.move_target = Some(dst);
        self.move_start = Instant::now();
        self.move_duration_ms = duration_ms.max(500);
        self.spline_id += 1;
    }

    pub fn finish_move(&mut self) {
        if let Some(dst) = self.move_target.take() {
            self.position = dst;
        }
        self.move_duration_ms = 0;
    }

    pub fn can_swing(&self) -> bool {
        self.is_alive
            && self.state == CreatureState::InCombat
            && self.last_swing.elapsed().as_millis() as u64 >= self.swing_timer_ms
    }

    pub fn record_swing(&mut self) {
        self.last_swing = Instant::now();
    }

    pub fn roll_damage(&self) -> u32 {
        if self.min_dmg >= self.max_dmg { return self.min_dmg; }
        let range = self.max_dmg - self.min_dmg;
        let seed = self.last_swing.elapsed().subsec_nanos();
        self.min_dmg + (seed % (range + 1))
    }

    pub fn should_wander(&self) -> bool {
        self.is_alive
            && self.state == CreatureState::Idle
            && self.can_wander()
            && self.wander_timer.elapsed().as_millis() as u64 >= self.wander_delay_ms
    }

    pub fn pick_wander_destination(&mut self) -> Position {
        let seed = self.wander_timer.elapsed().subsec_nanos() as f32;
        let angle = (seed * 0.001) % (2.0 * std::f32::consts::PI);
        let dist = (seed * 0.0001) % self.wander_radius + 1.0;
        let x = self.home_pos.x + angle.cos() * dist;
        let y = self.home_pos.y + angle.sin() * dist;
        let o = angle + std::f32::consts::PI;
        Position::new(x, y, self.home_pos.z, o)
    }

    pub fn reset_wander_timer(&mut self) {
        self.wander_timer = Instant::now();
        let seed = self.wander_timer.elapsed().subsec_nanos() as u64;
        self.wander_delay_ms = 5_000 + (seed % 10_000);
    }
}

/// A grid cell containing creatures and player references.
#[derive(Debug)]
pub struct Grid {
    pub coord: GridCoord,
    pub creatures: HashMap<ObjectGuid, WorldCreature>,
    pub player_guids: HashSet<ObjectGuid>,
    pub last_player_time: Instant,
    pub loaded: bool,
}

impl Grid {
    pub fn new(x: i16, y: i16) -> Self {
        Self {
            coord: GridCoord::new(x, y),
            creatures: HashMap::new(),
            player_guids: HashSet::new(),
            last_player_time: Instant::now(),
            loaded: true,
        }
    }

    pub fn add_creature(&mut self, creature: WorldCreature) -> bool {
        if self.creatures.contains_key(&creature.guid) {
            warn!("Creature {:?} already exists in grid {:?}", creature.guid, self.coord);
            return false;
        }
        self.creatures.insert(creature.guid, creature);
        true
    }

    pub fn remove_creature(&mut self, guid: ObjectGuid) -> bool {
        self.creatures.remove(&guid).is_some()
    }

    pub fn get_creature(&self, guid: ObjectGuid) -> Option<&WorldCreature> {
        self.creatures.get(&guid)
    }

    pub fn get_creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut WorldCreature> {
        self.creatures.get_mut(&guid)
    }

    pub fn player_enter(&mut self, guid: ObjectGuid) {
        self.player_guids.insert(guid);
        self.last_player_time = Instant::now();
    }

    pub fn player_leave(&mut self, guid: ObjectGuid) {
        self.player_guids.remove(&guid);
    }

    pub fn should_unload(&self, timeout: Duration) -> bool {
        self.player_guids.is_empty() && self.last_player_time.elapsed() > timeout
    }

    pub fn creature_count(&self) -> usize {
        self.creatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.creatures.is_empty() && self.player_guids.is_empty()
    }
}

/// An instance of a map (e.g., Eastern Kingdoms instance 0).
#[derive(Debug)]
pub struct MapInstance {
    pub map_id: u16,
    pub instance_id: u32,
    pub grids: HashMap<GridCoord, Grid>,
    pub grid_unload_timeout: Duration,
}

impl MapInstance {
    pub fn new(map_id: u16, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
            grids: HashMap::new(),
            grid_unload_timeout: DEFAULT_GRID_UNLOAD_TIME,
        }
    }

    pub fn get_or_create_grid(&mut self, x: i16, y: i16) -> &mut Grid {
        let coord = GridCoord::new(x, y);
        if !self.grids.contains_key(&coord) {
            let grid = Grid::new(x, y);
            self.grids.insert(coord, grid);
            debug!("Created new grid ({}, {}) for map {} instance {}", x, y, self.map_id, self.instance_id);
        }
        self.grids.get_mut(&coord).unwrap()
    }

    pub fn get_grid(&self, x: i16, y: i16) -> Option<&Grid> {
        self.grids.get(&GridCoord::new(x, y))
    }

    pub fn get_grid_mut(&mut self, x: i16, y: i16) -> Option<&mut Grid> {
        self.grids.get_mut(&GridCoord::new(x, y))
    }

    pub fn remove_grid(&mut self, x: i16, y: i16) -> bool {
        self.grids.remove(&GridCoord::new(x, y)).is_some()
    }

    pub fn add_creature(&mut self, x: i16, y: i16, creature: WorldCreature) -> bool {
        self.get_or_create_grid(x, y).add_creature(creature)
    }

    pub fn remove_creature(&mut self, x: i16, y: i16, guid: ObjectGuid) -> bool {
        if let Some(grid) = self.get_grid_mut(x, y) {
            grid.remove_creature(guid)
        } else {
            false
        }
    }

    pub fn get_creature(&self, x: i16, y: i16, guid: ObjectGuid) -> Option<&WorldCreature> {
        self.get_grid(x, y)?.get_creature(guid)
    }

    pub fn get_creature_mut(&mut self, x: i16, y: i16, guid: ObjectGuid) -> Option<&mut WorldCreature> {
        self.get_grid_mut(x, y)?.get_creature_mut(guid)
    }

    pub fn unload_empty_grids(&mut self) {
        let to_remove: Vec<GridCoord> = self
            .grids
            .iter()
            .filter(|(_, grid)| grid.should_unload(self.grid_unload_timeout))
            .map(|(coord, _)| *coord)
            .collect();

        for coord in to_remove {
            info!("Unloading grid {:?} from map {} (timeout)", coord, self.map_id);
            self.grids.remove(&coord);
        }
    }

    pub fn creature_count(&self) -> usize {
        self.grids.values().map(|g| g.creature_count()).sum()
    }

    pub fn is_grid_loaded(&self, x: i16, y: i16) -> bool {
        self.get_grid(x, y).is_some()
    }
}

/// Global map manager containing all map instances.
#[derive(Debug)]
pub struct MapManager {
    maps: HashMap<(u16, u32), MapInstance>, // (map_id, instance_id) -> MapInstance
}

impl MapManager {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
        }
    }

    pub fn get_or_create_map(&mut self, map_id: u16, instance_id: u32) -> &mut MapInstance {
        let key = (map_id, instance_id);
        if !self.maps.contains_key(&key) {
            let instance = MapInstance::new(map_id, instance_id);
            self.maps.insert(key, instance);
            info!("Created new map instance: map_id={}, instance_id={}", map_id, instance_id);
        }
        self.maps.get_mut(&key).unwrap()
    }

    pub fn get_map(&self, map_id: u16, instance_id: u32) -> Option<&MapInstance> {
        self.maps.get(&(map_id, instance_id))
    }

    pub fn get_map_mut(&mut self, map_id: u16, instance_id: u32) -> Option<&mut MapInstance> {
        self.maps.get_mut(&(map_id, instance_id))
    }

    // Convenience methods that delegate to MapInstance

    pub fn get_grid(&self, map_id: u16, instance_id: u32, x: i16, y: i16) -> Option<&Grid> {
        self.get_map(map_id, instance_id)?.get_grid(x, y)
    }

    pub fn get_grid_mut(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16) -> Option<&mut Grid> {
        self.get_map_mut(map_id, instance_id)?.get_grid_mut(x, y)
    }

    pub fn get_or_create_grid(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16) -> &mut Grid {
        self.get_or_create_map(map_id, instance_id).get_or_create_grid(x, y)
    }

    pub fn add_creature(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, creature: WorldCreature) -> bool {
        self.get_or_create_map(map_id, instance_id).add_creature(x, y, creature)
    }

    pub fn remove_creature(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, guid: ObjectGuid) -> bool {
        if let Some(map) = self.get_map_mut(map_id, instance_id) {
            map.remove_creature(x, y, guid)
        } else {
            false
        }
    }

    pub fn get_creature(&self, map_id: u16, instance_id: u32, x: i16, y: i16, guid: ObjectGuid) -> Option<&WorldCreature> {
        self.get_map(map_id, instance_id)?.get_creature(x, y, guid)
    }

    pub fn get_creature_mut(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, guid: ObjectGuid) -> Option<&mut WorldCreature> {
        self.get_map_mut(map_id, instance_id)?.get_creature_mut(x, y, guid)
    }

    pub fn with_creature_mut<F, R>(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, guid: ObjectGuid, f: F) -> Option<R>
    where
        F: FnOnce(&mut WorldCreature) -> R,
    {
        self.get_map_mut(map_id, instance_id)?.get_grid_mut(x, y)?.get_creature_mut(guid).map(f)
    }

    pub fn player_enter_grid(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, player_guid: ObjectGuid, _pos: Position) {
        let grid = self.get_or_create_grid(map_id, instance_id, x, y);
        grid.player_enter(player_guid);
        debug!("Player {:?} entered grid ({}, {}) in map {}", player_guid, x, y, map_id);
    }

    pub fn player_leave_grid(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16, player_guid: ObjectGuid) {
        if let Some(grid) = self.get_grid_mut(map_id, instance_id, x, y) {
            grid.player_leave(player_guid);
            debug!("Player {:?} left grid ({}, {}) in map {}", player_guid, x, y, map_id);
        }
    }

    pub fn player_move(&mut self, map_id: u16, instance_id: u32, from: (i16, i16), to: (i16, i16), player_guid: ObjectGuid, pos: Position) {
        let (from_x, from_y) = from;
        let (to_x, to_y) = to;

        // Leave old grid
        self.player_leave_grid(map_id, instance_id, from_x, from_y, player_guid);

        // Enter new grid
        self.player_enter_grid(map_id, instance_id, to_x, to_y, player_guid, pos);
    }

    pub fn get_visible_creatures(&self, map_id: u16, instance_id: u32, x: f32, y: f32, _z: f32) -> Vec<WorldCreature> {
        let center_x = world_to_grid_x(x);
        let center_y = world_to_grid_y(y);

        let mut creatures = Vec::new();

        // Get creatures from 3x3 grid area
        for dx in -1..=1 {
            for dy in -1..=1 {
                let grid_x = center_x + dx;
                let grid_y = center_y + dy;

                if let Some(grid) = self.get_grid(map_id, instance_id, grid_x, grid_y) {
                    for creature in grid.creatures.values() {
                        // Optional: Check actual distance for precise visibility
                        let dist = Position::distance(&Position::new(x, y, _z, 0.0), &creature.position);
                        if dist <= VISIBILITY_RADIUS {
                            creatures.push(creature.clone());
                        }
                    }
                }
            }
        }

        creatures
    }

    pub fn unload_distant_grids(&mut self, map_id: u16, instance_id: u32, center_x: i16, center_y: i16, range: i16) {
        if let Some(map) = self.get_map_mut(map_id, instance_id) {
            let to_remove: Vec<GridCoord> = map
                .grids
                .keys()
                .filter(|coord| {
                    let dx = (coord.x - center_x).abs();
                    let dy = (coord.y - center_y).abs();
                    dx > range || dy > range
                })
                .copied()
                .collect();

            for coord in to_remove {
                if let Some(grid) = map.grids.get(&coord) {
                    if grid.should_unload(map.grid_unload_timeout) {
                        info!("Unloading distant grid {:?} from map {}", coord, map_id);
                        map.grids.remove(&coord);
                    }
                }
            }
        }
    }

    pub fn is_grid_loaded(&self, map_id: u16, instance_id: u32, x: i16, y: i16) -> bool {
        self.get_map(map_id, instance_id)
            .map(|m| m.is_grid_loaded(x, y))
            .unwrap_or(false)
    }

    pub fn create_grid(&mut self, map_id: u16, instance_id: u32, x: i16, y: i16) -> &mut Grid {
        self.get_or_create_grid(map_id, instance_id, x, y)
    }

    pub fn creature_count(&self) -> usize {
        self.maps.values().map(|m| m.creature_count()).sum()
    }
}

/// Shared reference type for the MapManager.
pub type SharedMapManager = Arc<RwLock<MapManager>>;

/// Convert world X coordinate to grid X coordinate.
/// Uses floor() to handle negative coordinates correctly.
pub fn world_to_grid_x(world_x: f32) -> i16 {
    (world_x / GRID_SIZE).floor() as i16
}

/// Convert world Y coordinate to grid Y coordinate.
/// Uses floor() to handle negative coordinates correctly.
pub fn world_to_grid_y(world_y: f32) -> i16 {
    (world_y / GRID_SIZE).floor() as i16
}

/// Convert world coordinates to grid coordinates (x, y).
/// Convenience function that returns both coordinates at once.
pub fn world_to_grid_coords(world_x: f32, world_y: f32) -> (i16, i16) {
    (world_to_grid_x(world_x), world_to_grid_y(world_y))
}

/// Convert grid coordinate to world coordinate (center of grid).
pub fn grid_to_world(grid: i16) -> f32 {
    (grid as f32 * GRID_SIZE) + (GRID_SIZE / 2.0)
}

/// Get the world coordinates of a grid's corner.
pub fn grid_corner(grid_x: i16, grid_y: i16) -> (f32, f32) {
    (grid_x as f32 * GRID_SIZE, grid_y as f32 * GRID_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::guid::HighGuid;

    #[test]
    fn test_world_to_grid_positive() {
        assert_eq!(world_to_grid_x(0.0), 0);
        assert_eq!(world_to_grid_x(63.9), 0);
        assert_eq!(world_to_grid_x(64.0), 1);
        assert_eq!(world_to_grid_x(127.9), 1);
        assert_eq!(world_to_grid_x(128.0), 2);
    }

    #[test]
    fn test_world_to_grid_negative() {
        assert_eq!(world_to_grid_x(-0.1), -1);
        assert_eq!(world_to_grid_x(-64.0), -1);
        assert_eq!(world_to_grid_x(-64.1), -2);
        assert_eq!(world_to_grid_x(-127.9), -2);
        assert_eq!(world_to_grid_x(-128.0), -2);
    }

    #[test]
    fn test_world_to_grid_coords() {
        let (x, y) = world_to_grid_coords(100.0, -50.0);
        assert_eq!(x, 1);  // 100 / 64 = 1.56 -> floor = 1
        assert_eq!(y, -1); // -50 / 64 = -0.78 -> floor = -1
    }

    #[test]
    fn test_grid_round_trip() {
        let world_x = 150.5;
        let grid_x = world_to_grid_x(world_x);
        let world_center = grid_to_world(grid_x);
        // Center should be within half grid size
        assert!((world_x - world_center).abs() <= GRID_SIZE / 2.0);
    }

    #[test]
    fn test_creature_add_remove() {
        let mut grid = Grid::new(0, 0);
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid, 1, Position::new(10.0, 10.0, 0.0, 0.0),
            50, 1, 5, 10, 20.0, 0, 35, 0, 0,
        );

        assert!(grid.add_creature(creature.clone()));
        assert_eq!(grid.creature_count(), 1);
        assert!(grid.get_creature(guid).is_some());

        assert!(grid.remove_creature(guid));
        assert_eq!(grid.creature_count(), 0);
        assert!(grid.get_creature(guid).is_none());
    }

    #[test]
    fn test_duplicate_creature_rejected() {
        let mut grid = Grid::new(0, 0);
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid, 1, Position::new(10.0, 10.0, 0.0, 0.0),
            50, 1, 5, 10, 20.0, 0, 35, 0, 0,
        );

        assert!(grid.add_creature(creature.clone()));
        assert!(!grid.add_creature(creature)); // Duplicate should fail
    }

    #[test]
    fn test_player_enter_leave() {
        let mut grid = Grid::new(0, 0);
        let player = ObjectGuid::create_player(1, 1);

        grid.player_enter(player);
        assert!(grid.player_guids.contains(&player));

        grid.player_leave(player);
        assert!(!grid.player_guids.contains(&player));
    }

    #[test]
    fn test_should_unload() {
        let mut grid = Grid::new(0, 0);
        grid.last_player_time = Instant::now() - Duration::from_secs(400);
        assert!(grid.should_unload(Duration::from_secs(300)));
    }

    #[test]
    fn test_should_not_unload_with_player() {
        let mut grid = Grid::new(0, 0);
        let player = ObjectGuid::create_player(1, 1);
        grid.player_enter(player);
        grid.last_player_time = Instant::now() - Duration::from_secs(400);
        assert!(!grid.should_unload(Duration::from_secs(300)));
    }

    #[test]
    fn test_map_manager_create_map() {
        let mut manager = MapManager::new();
        let map = manager.get_or_create_map(0, 0);
        assert_eq!(map.map_id, 0);
        assert_eq!(map.instance_id, 0);
    }

    #[test]
    fn test_add_creature_to_map() {
        let mut manager = MapManager::new();
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid, 1, Position::new(10.0, 10.0, 0.0, 0.0),
            50, 1, 5, 10, 20.0, 0, 35, 0, 0,
        );

        assert!(manager.add_creature(0, 0, 0, 0, creature));
        assert!(manager.get_creature(0, 0, 0, 0, guid).is_some());
    }

    #[test]
    fn test_visible_creatures() {
        let mut manager = MapManager::new();
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid, 1, Position::new(10.0, 10.0, 0.0, 0.0),
            50, 1, 5, 10, 20.0, 0, 35, 0, 0,
        );

        manager.add_creature(0, 0, 0, 0, creature);
        
        // Should find creature at (10, 10)
        let visible = manager.get_visible_creatures(0, 0, 10.0, 10.0, 0.0);
        assert!(!visible.is_empty());
        assert_eq!(visible[0].guid, guid);

        // Should not find creature far away
        let visible = manager.get_visible_creatures(0, 0, 1000.0, 1000.0, 0.0);
        assert!(visible.is_empty());
    }
}