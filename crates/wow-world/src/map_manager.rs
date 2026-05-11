use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};
use wow_constants::WeaponAttackType;
use wow_core::{ObjectGuid, Position};
use wow_entities::{Creature, CreatureAiState};
use wow_packet::packets::update::CreatureCreateData;

/// Size of a grid cell in yards (64x64 yards like TrinityCore).
pub const GRID_SIZE: f32 = 64.0;

/// Visibility radius in yards (how far a player can see).
pub const VISIBILITY_RADIUS: f32 = 100.0;

/// Default time before a grid unloads if no players are nearby (5 minutes).
pub const DEFAULT_GRID_UNLOAD_TIME: Duration = Duration::from_secs(300);

/// TrinityCore `TerrainInfo::GetMinHeight` fallback when no terrain grid is loaded.
///
/// Real grid-backed min-height data belongs to the terrain/map-data port; exposing
/// the fallback here lets movement preserve the C++ under-map branch without
/// inventing terrain values.
pub const DEFAULT_MIN_HEIGHT_LIKE_CPP: f32 = -500.0;

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
    /// Canonical creature entity. Runtime/AI ownership lives here.
    pub creature: Creature,
    /// Packet-create bridge retained for update-object construction.
    pub create_data: CreatureCreateData,
    clock_started_at: Instant,
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
        let (min_dmg, max_dmg) = if min_dmg == 0 {
            let base = (level as u32) * 3 + 5;
            (base, base + base / 2)
        } else {
            (min_dmg, max_dmg)
        };

        let mut creature = Creature::new(false);
        creature.unit_mut().world_mut().object_mut().create(guid);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .set_entry(entry);
        let _ = creature.unit_mut().world_mut().set_map(0, 0);
        creature.set_ai_position(pos);
        creature.set_ai_home_position(pos);
        creature.unit_mut().set_level(level);
        creature.unit_mut().set_max_health(u64::from(hp));
        creature.unit_mut().set_health(u64::from(hp));
        creature.set_display_id(display_id, true, None);
        creature.set_faction(faction);
        creature.unit_mut().set_weapon_damage(
            WeaponAttackType::BaseAttack,
            min_dmg as f32,
            max_dmg as f32,
        );
        {
            let ai = creature.ai_ownership_mut();
            ai.aggro_radius = aggro_radius;
            ai.wander_radius = 5.0;
            ai.respawn_time_secs = 30;
            ai.npc_flags = npc_flags;
            ai.unit_flags = unit_flags;
            ai.display_id = display_id;
            ai.faction = faction;
            ai.min_damage = min_dmg;
            ai.max_damage = max_dmg;
        }

        let create_data = CreatureCreateData {
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
        };

        Self::from_canonical(creature, create_data)
    }

    pub fn from_canonical(creature: Creature, create_data: CreatureCreateData) -> Self {
        Self {
            creature,
            create_data,
            clock_started_at: Instant::now(),
        }
    }

    pub(crate) fn now_ms(&self) -> u64 {
        self.clock_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64
    }

    pub fn guid(&self) -> ObjectGuid {
        self.creature.ai_guid()
    }

    pub fn entry(&self) -> u32 {
        self.creature.ai_entry()
    }

    pub fn position(&self) -> Position {
        self.creature.ai_position()
    }

    pub fn home_position(&self) -> Position {
        self.creature.ai_home_position()
    }

    pub fn is_alive(&self) -> bool {
        self.creature.ai_is_alive()
    }

    pub fn current_hp(&self) -> u32 {
        self.creature.ai_current_health().min(u64::from(u32::MAX)) as u32
    }

    pub fn max_hp(&self) -> u32 {
        self.creature.ai_max_health().min(u64::from(u32::MAX)) as u32
    }

    pub fn level(&self) -> u8 {
        self.creature.ai_level()
    }

    pub fn npc_flags(&self) -> u32 {
        self.creature.ai_ownership().npc_flags
    }

    pub fn unit_flags(&self) -> u32 {
        self.creature.ai_ownership().unit_flags
    }

    pub fn display_id(&self) -> u32 {
        self.creature.ai_ownership().display_id
    }

    pub fn faction(&self) -> u32 {
        self.creature.ai_ownership().faction
    }

    pub fn min_dmg(&self) -> u32 {
        self.creature.ai_ownership().min_damage
    }

    pub fn max_dmg(&self) -> u32 {
        self.creature.ai_ownership().max_damage
    }

    pub fn loot_id(&self) -> u32 {
        self.creature.ai_ownership().loot_id
    }

    pub fn gold_min(&self) -> u32 {
        self.creature.ai_ownership().gold_min
    }

    pub fn gold_max(&self) -> u32 {
        self.creature.ai_ownership().gold_max
    }

    pub fn boss_id(&self) -> Option<u32> {
        self.creature.ai_ownership().boss_id
    }

    pub fn dungeon_encounter_id(&self) -> u32 {
        self.creature.ai_ownership().dungeon_encounter_id
    }

    pub fn state(&self) -> CreatureAiState {
        self.creature.ai_state()
    }

    pub fn move_target(&self) -> Option<Position> {
        self.creature.ai_ownership().move_target
    }

    pub fn spline_id(&self) -> u32 {
        self.creature.ai_ownership().spline_id
    }

    pub fn corpse_despawn_at(&self) -> Option<Instant> {
        self.creature
            .ai_ownership()
            .corpse_despawn_at_ms
            .map(|ms| self.clock_started_at + Duration::from_millis(ms))
    }

    pub fn set_corpse_despawn_at(&mut self, when: Option<Instant>) {
        let now_ms = self.now_ms();
        let at_ms = when.map(|instant| {
            if instant <= self.clock_started_at {
                0
            } else if instant <= Instant::now() {
                now_ms
            } else {
                now_ms.saturating_add(
                    instant
                        .duration_since(Instant::now())
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                )
            }
        });
        self.creature.set_ai_corpse_despawn_at(at_ms);
    }

    pub fn enter_combat(&mut self, attacker: ObjectGuid) {
        self.creature.enter_ai_combat(attacker);
        debug!(
            "Creature {:?} entered combat with {:?}",
            self.guid(),
            attacker
        );
    }

    pub fn reset_combat(&mut self) {
        self.creature.reset_ai_combat(self.now_ms());
    }

    pub fn take_damage(&mut self, damage: u32) -> bool {
        self.creature.take_ai_damage(damage, self.now_ms())
    }

    pub fn die(&mut self) {
        self.creature.mark_ai_dead(self.now_ms());
    }

    pub fn can_wander(&self) -> bool {
        self.creature.can_ai_wander()
    }

    pub fn try_aggro(&mut self, player_guid: ObjectGuid, player_pos: &Position) -> bool {
        self.creature.try_ai_aggro(player_guid, player_pos)
    }

    pub fn should_respawn(&self) -> bool {
        self.creature.should_ai_respawn(self.now_ms())
    }

    pub fn respawn(&mut self) {
        self.creature.respawn_ai(self.now_ms());
    }

    pub fn movement_finished(&self) -> bool {
        self.creature
            .ai_ownership()
            .move_target
            .map(|_| {
                self.now_ms()
                    .saturating_sub(self.creature.ai_ownership().move_start_ms)
                    >= u64::from(self.creature.ai_ownership().move_duration_ms)
            })
            .unwrap_or(true)
    }

    pub fn interpolated_position(&self) -> Position {
        let Some(dst) = self.creature.ai_ownership().move_target else {
            return self.position();
        };
        let elapsed =
            self.now_ms()
                .saturating_sub(self.creature.ai_ownership().move_start_ms) as f32;
        let total = self.creature.ai_ownership().move_duration_ms as f32;
        if total <= 0.0 {
            return dst;
        }
        let src = self.position();
        let t = (elapsed / total).min(1.0);
        Position::new(
            src.x + (dst.x - src.x) * t,
            src.y + (dst.y - src.y) * t,
            src.z + (dst.z - src.z) * t,
            dst.orientation,
        )
    }

    pub fn begin_move(&mut self, dst: Position) {
        let dist = self.position().distance(&dst);
        let walk_speed = 2.5f32;
        let duration_ms = ((dist / walk_speed) * 1000.0) as u32;
        let now_ms = self.now_ms();
        let ai = self.creature.ai_ownership_mut();
        ai.move_target = Some(dst);
        ai.move_start_ms = now_ms;
        ai.move_duration_ms = duration_ms.max(500);
        ai.spline_id = ai.spline_id.saturating_add(1);
    }

    pub fn finish_move(&mut self) {
        if let Some(dst) = self.creature.ai_ownership_mut().move_target.take() {
            self.creature.set_ai_position(dst);
        }
        self.creature.ai_ownership_mut().move_duration_ms = 0;
    }

    pub fn can_swing(&self) -> bool {
        self.is_alive()
            && self.state() == CreatureAiState::InCombat
            && self
                .now_ms()
                .saturating_sub(self.creature.ai_ownership().last_swing_ms)
                >= self.creature.ai_ownership().swing_timer_ms
    }

    pub fn record_swing(&mut self) {
        self.creature.ai_ownership_mut().last_swing_ms = self.now_ms();
    }

    pub fn roll_damage(&self) -> u32 {
        let min_dmg = self.min_dmg();
        let max_dmg = self.max_dmg();
        if min_dmg >= max_dmg {
            return min_dmg;
        }
        let range = max_dmg - min_dmg;
        let seed = (self.now_ms() as u32).wrapping_add(self.spline_id());
        min_dmg + (seed % (range + 1))
    }

    pub fn should_wander(&self) -> bool {
        self.is_alive()
            && self.state() == CreatureAiState::Idle
            && self.can_wander()
            && self
                .now_ms()
                .saturating_sub(self.creature.ai_ownership().move_start_ms)
                >= self.creature.ai_ownership().wander_delay_ms
    }

    pub fn pick_wander_destination(&mut self) -> Position {
        let seed = self.now_ms() as f32;
        let angle = (seed * 0.001) % (2.0 * std::f32::consts::PI);
        let radius = self.creature.ai_ownership().wander_radius.max(1.0);
        let dist = (seed * 0.0001) % radius + 1.0;
        let home = self.home_position();
        let x = home.x + angle.cos() * dist;
        let y = home.y + angle.sin() * dist;
        let o = angle + std::f32::consts::PI;
        Position::new(x, y, home.z, o)
    }

    pub fn reset_wander_timer(&mut self) {
        let now_ms = self.now_ms();
        let ai = self.creature.ai_ownership_mut();
        ai.move_start_ms = now_ms;
        ai.wander_delay_ms = 5_000 + (now_ms % 10_000);
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
        if self.creatures.contains_key(&creature.guid()) {
            warn!(
                "Creature {:?} already exists in grid {:?}",
                creature.guid(),
                self.coord
            );
            return false;
        }
        self.creatures.insert(creature.guid(), creature);
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
            debug!(
                "Created new grid ({}, {}) for map {} instance {}",
                x, y, self.map_id, self.instance_id
            );
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

    pub fn get_creature_mut(
        &mut self,
        x: i16,
        y: i16,
        guid: ObjectGuid,
    ) -> Option<&mut WorldCreature> {
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
            info!(
                "Unloading grid {:?} from map {} (timeout)",
                coord, self.map_id
            );
            self.grids.remove(&coord);
        }
    }

    pub fn creature_count(&self) -> usize {
        self.grids.values().map(|g| g.creature_count()).sum()
    }

    pub fn is_grid_loaded(&self, x: i16, y: i16) -> bool {
        self.get_grid(x, y).is_some()
    }

    pub fn min_height_like_cpp(&self, _x: f32, _y: f32) -> f32 {
        DEFAULT_MIN_HEIGHT_LIKE_CPP
    }
}

/// Global map manager containing all map instances.
#[derive(Debug)]
pub struct MapManager {
    maps: HashMap<(u16, u32), MapInstance>, // (map_id, instance_id) -> MapInstance
    free_instance_ids: Vec<bool>,
    next_instance_id: u32,
}

impl MapManager {
    pub fn new() -> Self {
        let mut manager = Self {
            maps: HashMap::new(),
            free_instance_ids: Vec::new(),
            next_instance_id: 1,
        };
        manager.init_instance_ids_from_max(0);
        manager
    }

    pub fn init_instance_ids_from_max(&mut self, max_existing_instance_id: u32) {
        self.next_instance_id = 1;
        self.free_instance_ids = vec![true; max_existing_instance_id.saturating_add(2) as usize];
        self.free_instance_ids[0] = false;
    }

    pub fn register_instance_id(&mut self, instance_id: u32) {
        let index = instance_id as usize;
        if index >= self.free_instance_ids.len() {
            self.free_instance_ids.resize(index.saturating_add(2), true);
        }

        self.free_instance_ids[index] = false;

        if self.next_instance_id == instance_id {
            self.next_instance_id = self.next_instance_id.saturating_add(1);
        }
    }

    pub fn generate_instance_id(&mut self) -> Option<u32> {
        if self.next_instance_id == u32::MAX {
            return None;
        }

        let new_instance_id = self.next_instance_id;
        let index = new_instance_id as usize;
        if index >= self.free_instance_ids.len() {
            self.free_instance_ids.resize(index.saturating_add(1), true);
        }
        self.free_instance_ids[index] = false;

        let search_start = self.next_instance_id.saturating_add(1) as usize;
        if let Some(next_free_offset) = self.free_instance_ids[search_start..]
            .iter()
            .position(|is_free| *is_free)
        {
            self.next_instance_id = (search_start + next_free_offset) as u32;
        } else {
            self.next_instance_id = self.free_instance_ids.len() as u32;
            self.free_instance_ids.push(true);
        }

        Some(new_instance_id)
    }

    pub fn free_instance_id(&mut self, instance_id: u32) {
        if instance_id == 0 {
            if self.free_instance_ids.is_empty() {
                self.init_instance_ids_from_max(0);
            } else {
                self.free_instance_ids[0] = false;
            }
            return;
        }

        let index = instance_id as usize;
        if index >= self.free_instance_ids.len() {
            self.free_instance_ids.resize(index.saturating_add(2), true);
        }

        self.next_instance_id = self.next_instance_id.min(instance_id);
        self.free_instance_ids[index] = true;
        self.free_instance_ids[0] = false;
    }

    pub fn get_or_create_map(&mut self, map_id: u16, instance_id: u32) -> &mut MapInstance {
        let key = (map_id, instance_id);
        if !self.maps.contains_key(&key) {
            let instance = MapInstance::new(map_id, instance_id);
            self.maps.insert(key, instance);
            info!(
                "Created new map instance: map_id={}, instance_id={}",
                map_id, instance_id
            );
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

    pub fn get_grid_mut(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
    ) -> Option<&mut Grid> {
        self.get_map_mut(map_id, instance_id)?.get_grid_mut(x, y)
    }

    pub fn get_or_create_grid(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
    ) -> &mut Grid {
        self.get_or_create_map(map_id, instance_id)
            .get_or_create_grid(x, y)
    }

    pub fn add_creature(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        creature: WorldCreature,
    ) -> bool {
        self.get_or_create_map(map_id, instance_id)
            .add_creature(x, y, creature)
    }

    pub fn remove_creature(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        guid: ObjectGuid,
    ) -> bool {
        if let Some(map) = self.get_map_mut(map_id, instance_id) {
            map.remove_creature(x, y, guid)
        } else {
            false
        }
    }

    pub fn get_creature(
        &self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        guid: ObjectGuid,
    ) -> Option<&WorldCreature> {
        self.get_map(map_id, instance_id)?.get_creature(x, y, guid)
    }

    pub fn get_creature_mut(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        guid: ObjectGuid,
    ) -> Option<&mut WorldCreature> {
        self.get_map_mut(map_id, instance_id)?
            .get_creature_mut(x, y, guid)
    }

    pub fn find_creature(
        &self,
        map_id: u16,
        instance_id: u32,
        guid: ObjectGuid,
    ) -> Option<&WorldCreature> {
        let map = self.get_map(map_id, instance_id)?;
        map.grids.values().find_map(|grid| grid.get_creature(guid))
    }

    pub fn find_creature_mut(
        &mut self,
        map_id: u16,
        instance_id: u32,
        guid: ObjectGuid,
    ) -> Option<&mut WorldCreature> {
        let map = self.get_map_mut(map_id, instance_id)?;
        map.grids
            .values_mut()
            .find_map(|grid| grid.get_creature_mut(guid))
    }

    pub fn creature_guids(&self, map_id: u16, instance_id: u32) -> Vec<ObjectGuid> {
        self.get_map(map_id, instance_id)
            .map(|map| {
                map.grids
                    .values()
                    .flat_map(|grid| grid.creatures.keys().copied())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remove_creature_any(
        &mut self,
        map_id: u16,
        instance_id: u32,
        guid: ObjectGuid,
    ) -> Option<WorldCreature> {
        let map = self.get_map_mut(map_id, instance_id)?;
        map.grids
            .values_mut()
            .find_map(|grid| grid.creatures.remove(&guid))
    }

    pub fn with_creature_mut<F, R>(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        guid: ObjectGuid,
        f: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut WorldCreature) -> R,
    {
        self.get_map_mut(map_id, instance_id)?
            .get_grid_mut(x, y)?
            .get_creature_mut(guid)
            .map(f)
    }

    pub fn player_enter_grid(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        player_guid: ObjectGuid,
        _pos: Position,
    ) {
        let grid = self.get_or_create_grid(map_id, instance_id, x, y);
        grid.player_enter(player_guid);
        debug!(
            "Player {:?} entered grid ({}, {}) in map {}",
            player_guid, x, y, map_id
        );
    }

    pub fn player_leave_grid(
        &mut self,
        map_id: u16,
        instance_id: u32,
        x: i16,
        y: i16,
        player_guid: ObjectGuid,
    ) {
        if let Some(grid) = self.get_grid_mut(map_id, instance_id, x, y) {
            grid.player_leave(player_guid);
            debug!(
                "Player {:?} left grid ({}, {}) in map {}",
                player_guid, x, y, map_id
            );
        }
    }

    pub fn player_move(
        &mut self,
        map_id: u16,
        instance_id: u32,
        from: (i16, i16),
        to: (i16, i16),
        player_guid: ObjectGuid,
        pos: Position,
    ) {
        let (from_x, from_y) = from;
        let (to_x, to_y) = to;

        // Leave old grid
        self.player_leave_grid(map_id, instance_id, from_x, from_y, player_guid);

        // Enter new grid
        self.player_enter_grid(map_id, instance_id, to_x, to_y, player_guid, pos);
    }

    pub fn get_visible_creatures(
        &self,
        map_id: u16,
        instance_id: u32,
        x: f32,
        y: f32,
        _z: f32,
    ) -> Vec<WorldCreature> {
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
                        let dist =
                            Position::distance(&Position::new(x, y, _z, 0.0), &creature.position());
                        if dist <= VISIBILITY_RADIUS {
                            creatures.push(creature.clone());
                        }
                    }
                }
            }
        }

        creatures
    }

    pub fn unload_distant_grids(
        &mut self,
        map_id: u16,
        instance_id: u32,
        center_x: i16,
        center_y: i16,
        range: i16,
    ) {
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

    pub fn min_height_like_cpp(&self, map_id: u16, instance_id: u32, x: f32, y: f32) -> f32 {
        self.get_map(map_id, instance_id)
            .map(|m| m.min_height_like_cpp(x, y))
            .unwrap_or(DEFAULT_MIN_HEIGHT_LIKE_CPP)
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
        assert_eq!(x, 1); // 100 / 64 = 1.56 -> floor = 1
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
            guid,
            1,
            Position::new(10.0, 10.0, 0.0, 0.0),
            50,
            1,
            5,
            10,
            20.0,
            0,
            35,
            0,
            0,
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
            guid,
            1,
            Position::new(10.0, 10.0, 0.0, 0.0),
            50,
            1,
            5,
            10,
            20.0,
            0,
            35,
            0,
            0,
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
    fn instance_id_allocator_generates_lowest_free_id_like_cpp() {
        let mut manager = MapManager::new();

        assert_eq!(manager.generate_instance_id(), Some(1));
        assert_eq!(manager.generate_instance_id(), Some(2));
        assert_eq!(manager.generate_instance_id(), Some(3));

        manager.free_instance_id(2);
        assert_eq!(manager.generate_instance_id(), Some(2));
        assert_eq!(manager.generate_instance_id(), Some(4));
    }

    #[test]
    fn instance_id_allocator_registers_loaded_ids_in_order_like_cpp() {
        let mut manager = MapManager::new();
        manager.init_instance_ids_from_max(5);

        manager.register_instance_id(1);
        manager.register_instance_id(2);
        manager.register_instance_id(4);

        assert_eq!(manager.generate_instance_id(), Some(3));
        assert_eq!(manager.generate_instance_id(), Some(5));
        assert_eq!(manager.generate_instance_id(), Some(6));
    }

    #[test]
    fn instance_id_allocator_keeps_zero_reserved_like_cpp() {
        let mut manager = MapManager::new();

        manager.free_instance_id(0);

        assert_eq!(manager.generate_instance_id(), Some(1));
    }

    #[test]
    fn test_add_creature_to_map() {
        let mut manager = MapManager::new();
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid,
            1,
            Position::new(10.0, 10.0, 0.0, 0.0),
            50,
            1,
            5,
            10,
            20.0,
            0,
            35,
            0,
            0,
        );

        assert!(manager.add_creature(0, 0, 0, 0, creature));
        assert!(manager.get_creature(0, 0, 0, 0, guid).is_some());
    }

    #[test]
    fn map_manager_uses_canonical_creature_guid_position_and_runtime() {
        let mut manager = MapManager::new();
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid,
            1,
            Position::new(10.0, 10.0, 0.0, 0.0),
            50,
            2,
            5,
            10,
            20.0,
            100,
            14,
            0,
            0,
        );

        assert!(manager.add_creature(0, 0, 0, 0, creature));
        let stored = manager
            .find_creature(0, 0, guid)
            .expect("canonical creature stored");
        assert_eq!(stored.guid(), guid);
        assert_eq!(stored.position(), Position::new(10.0, 10.0, 0.0, 0.0));
        assert_eq!(stored.current_hp(), 50);

        manager
            .find_creature_mut(0, 0, guid)
            .expect("canonical creature mutable")
            .take_damage(25);
        let stored = manager
            .find_creature(0, 0, guid)
            .expect("canonical creature stored");
        assert_eq!(stored.current_hp(), 25);
        assert_eq!(stored.creature.unit().data().health, 25);
    }

    #[test]
    fn test_visible_creatures() {
        let mut manager = MapManager::new();
        let guid = ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 0, 0, 1, 12345);
        let creature = WorldCreature::new(
            guid,
            1,
            Position::new(10.0, 10.0, 0.0, 0.0),
            50,
            1,
            5,
            10,
            20.0,
            0,
            35,
            0,
            0,
        );

        manager.add_creature(0, 0, 0, 0, creature);

        // Should find creature at (10, 10)
        let visible = manager.get_visible_creatures(0, 0, 10.0, 10.0, 0.0);
        assert!(!visible.is_empty());
        assert_eq!(visible[0].guid(), guid);

        // Should not find creature far away
        let visible = manager.get_visible_creatures(0, 0, 1000.0, 1000.0, 0.0);
        assert!(visible.is_empty());
    }
}
