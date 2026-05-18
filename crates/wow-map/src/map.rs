//! Map grid lifecycle skeleton.
//!
//! C++ references:
//! - `game/Maps/Map.h`
//! - `game/Maps/Map.cpp`

use std::collections::{HashMap, HashSet};

use crate::cell::{Cell, GridObjectGuids, WorldObjectGuids, calculate_cell_area_like_cpp};
use crate::coords::{
    CellCoord, GridCoord, MAX_NUMBER_OF_CELLS, MAX_NUMBER_OF_GRIDS, SIZE_OF_GRID_CELL,
    TOTAL_NUMBER_OF_CELLS_PER_MAP, compute_cell_coord, is_valid_map_coord_2d,
};
use crate::grid::{GridStateKind, MapGridHost, NGrid, update_grid_state};
use crate::object_grid_loader::{GridSpawnLoadFilter, ObjectGridLoader};
use crate::personal_phase::{MultiPersonalPhaseTracker, PhaseShift};
use crate::spawn::{Difficulty, SpawnGroupFlags, SpawnObjectType};
use wow_core::{ObjectGuid, Position};
use wow_entities::{
    AccessorObjectKind, CombatBeginContextLikeCpp, CombatSubsystem, Creature, GameObject,
    MAX_VISIBILITY_DISTANCE, MapBindingError, MapObjectRecord, ObjectAccessorError,
    ObjectAccessorMapSource, ObjectNotifyFlags, Player, Unit, WorldObject,
};

const GRID_SLOT_COUNT: usize = (MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_GRIDS) as usize;

#[derive(Clone, Copy)]
struct CombatUnitSnapshotLikeCpp<'a> {
    guid: ObjectGuid,
    unit: &'a Unit,
    game_master_player: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveObjectKind {
    Player,
    NonPlayer,
}

impl From<AccessorObjectKind> for ActiveObjectKind {
    fn from(kind: AccessorObjectKind) -> Self {
        match kind {
            AccessorObjectKind::Player => Self::Player,
            _ => Self::NonPlayer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicRespawnScalingConfig {
    pub creature_rate: f64,
    pub creature_minimum_secs: u32,
    pub gameobject_rate: f64,
    pub gameobject_minimum_secs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicRespawnScalingNoopReason {
    DynamicModeDisabled,
    UnsupportedMode,
    BattlegroundOrArena,
    UnsupportedSpawnType,
    MissingSpawnMetadata,
    MissingDynamicSpawnRateFlag,
    MissingZonePlayerCount,
    ZeroZonePlayers,
    AdjustFactorAtLeastOne,
    DelayAtOrBelowMinimum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicRespawnScalingOutcome {
    pub delay_secs: u32,
    pub noop_reason: Option<DynamicRespawnScalingNoopReason>,
}

impl DynamicRespawnScalingOutcome {
    pub const fn unchanged(delay_secs: u32, reason: DynamicRespawnScalingNoopReason) -> Self {
        Self {
            delay_secs,
            noop_reason: Some(reason),
        }
    }

    pub const fn scaled(delay_secs: u32) -> Self {
        Self {
            delay_secs,
            noop_reason: None,
        }
    }

    pub const fn was_scaled(self) -> bool {
        self.noop_reason.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicRespawnScalingContext {
    pub mode: u32,
    pub spawn_type: Option<SpawnObjectType>,
    pub spawn_metadata_present: bool,
    pub spawn_group_flags: Option<SpawnGroupFlags>,
    pub is_battleground_or_arena: bool,
    pub zone_player_count: Option<u32>,
    pub config: DynamicRespawnScalingConfig,
}

/// Rust equivalent of C++ `Map::ApplyDynamicModeRespawnScaling`.
///
/// C++ anchors:
/// - `GameObject.cpp:1665-1672` calls this before persisting GO respawn time.
/// - `Map.cpp:2242-2284` contains the dynamic respawn guards and formula.
/// - `Map.h:657-660` declares the map helper.
///
/// This helper is pure because RustyCore does not yet own the canonical map
/// spawn-metadata and zone-player-count stores needed by a `Map` method. Future
/// GameObject runtime wiring must pass canonical metadata/counts into this
/// function; this function must not read or mutate session-local fallback state.
pub fn apply_dynamic_mode_respawn_scaling_like_cpp(
    respawn_delay_secs: u32,
    context: DynamicRespawnScalingContext,
) -> DynamicRespawnScalingOutcome {
    if context.mode == 0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::DynamicModeDisabled,
        );
    }

    if context.mode != 1 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedMode,
        );
    }

    if context.is_battleground_or_arena {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::BattlegroundOrArena,
        );
    }

    let Some(spawn_type) = context.spawn_type else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );
    };

    if !matches!(
        spawn_type,
        SpawnObjectType::Creature | SpawnObjectType::GameObject
    ) {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );
    }

    if !context.spawn_metadata_present {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    }

    let Some(spawn_group_flags) = context.spawn_group_flags else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    };

    if !spawn_group_flags.contains(SpawnGroupFlags::DYNAMIC_SPAWN_RATE) {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingDynamicSpawnRateFlag,
        );
    }

    let Some(player_count) = context.zone_player_count else {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::MissingZonePlayerCount,
        );
    };

    if player_count == 0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::ZeroZonePlayers,
        );
    }

    let (rate, time_minimum) = match spawn_type {
        SpawnObjectType::Creature => (
            context.config.creature_rate,
            context.config.creature_minimum_secs,
        ),
        SpawnObjectType::GameObject => (
            context.config.gameobject_rate,
            context.config.gameobject_minimum_secs,
        ),
        SpawnObjectType::AreaTrigger => {
            return DynamicRespawnScalingOutcome::unchanged(
                respawn_delay_secs,
                DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
            );
        }
    };

    let adjust_factor = rate / f64::from(player_count);
    if adjust_factor >= 1.0 {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::AdjustFactorAtLeastOne,
        );
    }

    if respawn_delay_secs <= time_minimum {
        return DynamicRespawnScalingOutcome::unchanged(
            respawn_delay_secs,
            DynamicRespawnScalingNoopReason::DelayAtOrBelowMinimum,
        );
    }

    let scaled = (f64::from(respawn_delay_secs) * adjust_factor).ceil() as u32;
    DynamicRespawnScalingOutcome::scaled(scaled.max(time_minimum))
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
    map_objects: HashMap<ObjectGuid, MapObjectRecord>,
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
            map_objects: HashMap::new(),
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

    pub const fn visibility_range(&self) -> f32 {
        self.visible_distance
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

    pub fn map_object_count(&self) -> usize {
        self.map_objects.len()
    }

    pub fn insert_map_object(
        &mut self,
        kind: AccessorObjectKind,
        object: WorldObject,
    ) -> Result<Option<MapObjectRecord>, MapObjectStoreError> {
        let record = MapObjectRecord::new(kind, object)?;
        self.insert_map_object_record(record)
    }

    pub fn insert_map_object_record(
        &mut self,
        record: MapObjectRecord,
    ) -> Result<Option<MapObjectRecord>, MapObjectStoreError> {
        self.validate_map_object(record.object())?;
        Ok(self.map_objects.insert(record.object().guid(), record))
    }

    pub fn add_to_map_like_cpp(
        &mut self,
        kind: AccessorObjectKind,
        mut object: WorldObject,
    ) -> Result<AddToMapOutcome, AddToMapError> {
        if object.object().is_in_world() {
            let guid = object.guid();
            let cell = Cell::from_world(object.position().x, object.position().y);
            let previous = self.insert_map_object(kind, object)?;
            return Ok(AddToMapOutcome {
                guid,
                cell: cell.cell_coord(),
                grid: GridCoord::new(cell.grid_x(), cell.grid_y()),
                inserted: previous.is_none(),
                already_in_world: true,
                grid_created: false,
                grid_loaded: false,
                inserted_into_cell: false,
            });
        }

        let prevalidated_record =
            MapObjectRecord::new(kind, object.clone()).map_err(MapObjectStoreError::from)?;
        self.validate_map_object(prevalidated_record.object())?;

        let position = object.position();
        if !is_valid_map_coord_2d(position.x, position.y) {
            return Err(AddToMapError::InvalidCoordinates {
                guid: object.guid(),
                x: position.x,
                y: position.y,
            });
        }

        let cell = Cell::from_world(position.x, position.y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());
        let guid = object.guid();
        let active_object = is_active_object_like_cpp(kind, &object);
        let grid_loaded = if active_object {
            self.ensure_grid_loaded_for_active_object(&cell, kind.into())
        } else {
            false
        };
        let grid_created = if active_object {
            false
        } else {
            self.ensure_grid_created(grid)
        };

        {
            let ngrid = self
                .get_ngrid_mut(grid)
                .expect("Map::AddToMap must have created or loaded the target grid");
            let local_cell = ngrid
                .get_grid_type_mut(cell.cell_x(), cell.cell_y())
                .expect("cell coordinates must be local to target grid");
            insert_object_guid_in_cell_like_cpp(local_cell, kind, object.is_world_object(), guid);
        }

        object.set_current_cell(cell.cell_x(), cell.cell_y());
        object.object_mut().add_to_world();
        object.object_mut().set_is_new_object(true);
        // Rust does not emit visibility here yet; keep the flag lifecycle identical to
        // C++ `Map::AddToMap` after `UpdateObjectVisibilityOnCreate()` returns.
        object.object_mut().set_is_new_object(false);

        let previous = self.insert_map_object(kind, object)?;
        Ok(AddToMapOutcome {
            guid,
            cell: cell.cell_coord(),
            grid,
            inserted: previous.is_none(),
            already_in_world: false,
            grid_created,
            grid_loaded,
            inserted_into_cell: true,
        })
    }

    pub fn remove_map_object(&mut self, guid: ObjectGuid) -> Option<MapObjectRecord> {
        self.map_objects.remove(&guid)
    }

    pub fn remove_from_map_like_cpp(
        &mut self,
        guid: ObjectGuid,
        delete_from_world: bool,
    ) -> Result<RemoveFromMapOutcome, RemoveFromMapError> {
        let record = self
            .remove_map_object(guid)
            .ok_or(RemoveFromMapError::ObjectNotFound { guid })?;
        let kind = record.kind();
        let mut object = record.into_object();
        let was_in_world = object.object().is_in_world();
        let was_active = is_active_object_like_cpp(kind, &object);
        let cell = Cell::from_world(object.position().x, object.position().y);
        let grid = GridCoord::new(cell.grid_x(), cell.grid_y());

        object.object_mut().remove_from_world();
        let removed_from_cell = remove_object_guid_from_cell_like_cpp(
            self,
            grid,
            &cell,
            kind,
            object.is_world_object(),
            guid,
        );
        if was_active {
            self.unmark_active_cell(cell.cell_coord());
        }

        object.clear_current_cell();
        object.reset_map().map_err(RemoveFromMapError::ResetMap)?;

        Ok(RemoveFromMapOutcome {
            guid,
            cell: cell.cell_coord(),
            grid,
            was_in_world,
            was_active,
            removed_from_cell,
            delete_from_world,
            object: if delete_from_world {
                None
            } else {
                Some(object)
            },
        })
    }

    pub fn relocate_map_object_like_cpp(
        &mut self,
        guid: ObjectGuid,
        new_position: Position,
    ) -> Result<MapObjectRelocationOutcome, MapObjectRelocationError> {
        if !is_valid_map_coord_2d(new_position.x, new_position.y) {
            return Err(MapObjectRelocationError::InvalidCoordinates {
                guid,
                x: new_position.x,
                y: new_position.y,
            });
        }

        let record = self
            .map_object_record(guid)
            .ok_or(MapObjectRelocationError::ObjectNotFound { guid })?;
        let kind = record.kind();
        let old_position = record.object().position();
        let old_cell = Cell::from_world(old_position.x, old_position.y);
        let new_cell = Cell::from_world(new_position.x, new_position.y);
        let old_grid = GridCoord::new(old_cell.grid_x(), old_cell.grid_y());
        let new_grid = GridCoord::new(new_cell.grid_x(), new_cell.grid_y());
        let diff_cell = old_cell.diff_cell(&new_cell);
        let diff_grid = old_cell.diff_grid(&new_cell);

        if !diff_cell && !diff_grid {
            let mut record = self
                .remove_map_object(guid)
                .expect("record was just observed");
            record.object_mut().relocate(new_position);
            self.insert_map_object_record(record)
                .map_err(MapObjectRelocationError::Store)?;
            return Ok(MapObjectRelocationOutcome {
                guid,
                old_cell: old_cell.cell_coord(),
                new_cell: new_cell.cell_coord(),
                old_grid,
                new_grid,
                moved_between_cells: false,
                loaded_grid: false,
                created_grid: false,
                relocated: true,
                blocked_by_unloaded_grid: false,
            });
        }

        let active_object = is_active_object_like_cpp(kind, record.object());
        let loaded_grid = if diff_grid && active_object {
            self.ensure_grid_loaded_for_active_object(&new_cell, kind.into())
        } else {
            false
        };
        let created_grid = if diff_grid && !active_object {
            if !self.is_grid_loaded(new_grid) {
                return Ok(MapObjectRelocationOutcome {
                    guid,
                    old_cell: old_cell.cell_coord(),
                    new_cell: new_cell.cell_coord(),
                    old_grid,
                    new_grid,
                    moved_between_cells: false,
                    loaded_grid: false,
                    created_grid: false,
                    relocated: false,
                    blocked_by_unloaded_grid: true,
                });
            }
            self.ensure_grid_created(new_grid)
        } else {
            false
        };

        let mut record = self
            .remove_map_object(guid)
            .expect("record was just observed");
        let object_is_world_object = record.object().is_world_object();
        let removed = remove_object_guid_from_cell_like_cpp(
            self,
            old_grid,
            &old_cell,
            kind,
            object_is_world_object,
            guid,
        );
        debug_assert!(removed, "relocated object should have been in its old cell");
        {
            let ngrid = self
                .get_ngrid_mut(new_grid)
                .expect("relocation target grid must be loaded or created");
            let local_cell = ngrid
                .get_grid_type_mut(new_cell.cell_x(), new_cell.cell_y())
                .expect("cell coordinates must be local to target grid");
            insert_object_guid_in_cell_like_cpp(local_cell, kind, object_is_world_object, guid);
        }
        record.object_mut().relocate(new_position);
        record
            .object_mut()
            .set_current_cell(new_cell.cell_x(), new_cell.cell_y());
        self.insert_map_object_record(record)
            .map_err(MapObjectRelocationError::Store)?;

        Ok(MapObjectRelocationOutcome {
            guid,
            old_cell: old_cell.cell_coord(),
            new_cell: new_cell.cell_coord(),
            old_grid,
            new_grid,
            moved_between_cells: true,
            loaded_grid,
            created_grid,
            relocated: true,
            blocked_by_unloaded_grid: false,
        })
    }

    pub fn nearby_cell_guids_like_cpp(&self, x: f32, y: f32, radius: f32) -> NearbyCellGuids {
        if !is_valid_map_coord_2d(x, y) {
            return NearbyCellGuids::default();
        }

        let area = calculate_cell_area_like_cpp(x, y, radius);
        let mut result = NearbyCellGuids::default();
        for cell_x in area.low_bound.x_coord..=area.high_bound.x_coord {
            for cell_y in area.low_bound.y_coord..=area.high_bound.y_coord {
                result.visited_cells += 1;
                let cell = Cell::from_cell_coord(CellCoord::new(cell_x, cell_y));
                let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
                else {
                    continue;
                };
                let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                    continue;
                };
                result.merge_world(&local_cell.world_objects);
                result.merge_grid(&local_cell.grid_objects);
            }
        }

        result
    }

    pub fn visit_nearby_cells_of_like_cpp(
        &self,
        centers: impl IntoIterator<Item = NearbyCellVisitCenter>,
    ) -> NearbyCellVisitPlan {
        let mut marked_cells = HashSet::new();
        let mut marked_cells_in_visit_order = Vec::new();
        let mut nearby = NearbyCellGuids::default();
        let mut skipped_missing_centers = Vec::new();
        let mut skipped_invalid_position_centers = Vec::new();

        for center in centers {
            let Some(object) = self.map_object(center.guid) else {
                skipped_missing_centers.push(center.guid);
                continue;
            };
            let position = object.position();
            if !is_valid_map_coord_2d(position.x, position.y) {
                skipped_invalid_position_centers.push(center.guid);
                continue;
            }

            let area =
                calculate_cell_area_like_cpp(position.x, position.y, center.activation_radius);
            for cell_x in area.low_bound.x_coord..=area.high_bound.x_coord {
                for cell_y in area.low_bound.y_coord..=area.high_bound.y_coord {
                    let cell_coord = CellCoord::new(cell_x, cell_y);
                    if !marked_cells.insert(cell_coord) {
                        continue;
                    }

                    marked_cells_in_visit_order.push(cell_coord);
                    nearby.visited_cells += 1;
                    let cell = Cell::from_cell_coord(cell_coord);
                    let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y()))
                    else {
                        continue;
                    };
                    let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                        continue;
                    };
                    nearby.merge_world(&local_cell.world_objects);
                    nearby.merge_grid(&local_cell.grid_objects);
                }
            }
        }

        NearbyCellVisitPlan {
            marked_cells: marked_cells_in_visit_order,
            nearby,
            skipped_missing_centers,
            skipped_invalid_position_centers,
        }
    }

    pub fn object_update_plan_for_nearby_like_cpp(
        &self,
        nearby: &NearbyCellGuids,
        diff_ms: u32,
    ) -> ObjectUpdatePlan {
        let mut update_guids = Vec::new();
        for guid in nearby
            .world
            .creatures
            .iter()
            .chain(nearby.world.dynamic_objects.iter())
            .chain(nearby.grid.creatures.iter())
            .chain(nearby.grid.gameobjects.iter())
            .chain(nearby.grid.dynamic_objects.iter())
            .chain(nearby.grid.area_triggers.iter())
            .chain(nearby.grid.scene_objects.iter())
            .chain(nearby.grid.conversations.iter())
        {
            if self
                .map_object(*guid)
                .is_some_and(|object| object.object().is_in_world())
            {
                update_guids.push(*guid);
            }
        }

        update_guids.sort();
        update_guids.dedup();
        ObjectUpdatePlan {
            diff_ms,
            update_guids,
        }
    }

    pub fn map_update_visit_plan_like_cpp(
        &self,
        sources: impl IntoIterator<Item = MapUpdatePlayerSources>,
        active_non_player_guids: impl IntoIterator<Item = ObjectGuid>,
        transport_guids: impl IntoIterator<Item = ObjectGuid>,
        diff_ms: u32,
    ) -> MapUpdateVisitPlan {
        let mut session_update_players = Vec::new();
        let mut player_update_guids = Vec::new();
        let mut nearby_visit_centers = Vec::new();
        let mut saw_player_source = false;

        for source in sources {
            saw_player_source = true;
            if !self.object_is_in_world(source.player_guid) {
                continue;
            }

            session_update_players.push(source.player_guid);
            player_update_guids.push(source.player_guid);
            nearby_visit_centers.push(source.player_guid);

            if let Some(viewpoint) = source.viewpoint_guid
                && self.object_is_in_world(viewpoint)
            {
                nearby_visit_centers.push(viewpoint);
            }

            push_in_world_guids(
                self,
                &mut nearby_visit_centers,
                source.far_combat_unit_guids,
            );
            push_in_world_guids(
                self,
                &mut nearby_visit_centers,
                source.far_aura_caster_guids,
            );
            push_in_world_guids(self, &mut nearby_visit_centers, source.far_summon_guids);
        }

        let mut saw_active_non_player_source = false;
        for guid in active_non_player_guids {
            saw_active_non_player_source = true;
            if self.object_is_in_world(guid) {
                nearby_visit_centers.push(guid);
            }
        }

        let mut transport_update_guids = Vec::new();
        for guid in transport_guids {
            if self.map_object(guid).is_some() {
                transport_update_guids.push(guid);
            }
        }

        sort_dedup(&mut session_update_players);
        sort_dedup(&mut player_update_guids);
        sort_dedup(&mut nearby_visit_centers);
        sort_dedup(&mut transport_update_guids);
        let process_relocation_notifies = saw_player_source || saw_active_non_player_source;

        MapUpdateVisitPlan {
            diff_ms,
            session_update_players,
            player_update_guids,
            nearby_visit_centers,
            transport_update_guids,
            process_relocation_notifies,
        }
    }

    pub fn process_relocation_notifies_plan_like_cpp(
        &mut self,
        marked_cells: impl IntoIterator<Item = CellCoord>,
        diff_ms: u32,
        visibility_notify_period_ms: i64,
    ) -> RelocationNotifyProcessPlan {
        let marked_cells: HashSet<_> = marked_cells.into_iter().collect();
        let mut delayed_relocation_cells = Vec::new();
        let mut reset_notify_cells = Vec::new();
        let mut reset_timer_grids = Vec::new();
        let mut expired_active_grids = Vec::new();

        for grid_x in 0..MAX_NUMBER_OF_GRIDS {
            for grid_y in 0..MAX_NUMBER_OF_GRIDS {
                let coord = GridCoord::new(grid_x, grid_y);
                let Some(grid) = self.get_ngrid_mut(coord) else {
                    continue;
                };
                if grid.state() != GridStateKind::Active {
                    continue;
                }

                grid.info_mut()
                    .relocation_timer_mut()
                    .tracker_update(diff_ms);
                if !grid.info().relocation_timer().tracker_passed() {
                    continue;
                }

                expired_active_grids.push(coord);
                delayed_relocation_cells
                    .extend(marked_cells_in_grid_like_cpp(coord, &marked_cells));
            }
        }

        for coord in &expired_active_grids {
            let Some(grid) = self.get_ngrid_mut(*coord) else {
                continue;
            };
            if grid.state() != GridStateKind::Active {
                continue;
            }
            if !grid.info().relocation_timer().tracker_passed() {
                continue;
            }

            grid.info_mut()
                .relocation_timer_mut()
                .tracker_reset(diff_ms, visibility_notify_period_ms);
            reset_timer_grids.push(*coord);
            reset_notify_cells.extend(marked_cells_in_grid_like_cpp(*coord, &marked_cells));
        }

        RelocationNotifyProcessPlan {
            diff_ms,
            delayed_relocation_cells,
            reset_notify_cells,
            reset_timer_grids,
        }
    }

    pub fn process_relocation_notifies_like_cpp(
        &mut self,
        marked_cells: impl IntoIterator<Item = CellCoord>,
        diff_ms: u32,
        visibility_notify_period_ms: i64,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> ProcessRelocationNotifiesOutcome {
        let process_plan = self.process_relocation_notifies_plan_like_cpp(
            marked_cells,
            diff_ms,
            visibility_notify_period_ms,
        );
        let delayed_plan = self.delayed_unit_relocation_for_cells_like_cpp(
            process_plan.delayed_relocation_cells.iter().copied(),
            invalid_non_self_viewpoints,
        );
        let reset_outcome = self
            .reset_notify_flags_for_cells_like_cpp(process_plan.reset_notify_cells.iter().copied());

        ProcessRelocationNotifiesOutcome {
            process_plan,
            delayed_plan,
            reset_outcome,
        }
    }

    pub fn reset_notify_flags_for_cells_like_cpp(
        &mut self,
        cells: impl IntoIterator<Item = CellCoord>,
    ) -> ResetNotifyFlagsOutcome {
        let mut reset_player_guids = Vec::new();
        let mut reset_creature_guids = Vec::new();
        let mut missing_guids = Vec::new();

        for cell_coord in cells {
            let cell = Cell::from_cell_coord(cell_coord);
            let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y())) else {
                continue;
            };
            let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
                continue;
            };

            reset_player_guids.extend(local_cell.world_objects.players.iter().copied());
            reset_creature_guids.extend(local_cell.grid_objects.creatures.iter().copied());
            reset_creature_guids.extend(local_cell.world_objects.creatures.iter().copied());
        }

        sort_dedup(&mut reset_player_guids);
        sort_dedup(&mut reset_creature_guids);

        for guid in reset_player_guids
            .iter()
            .chain(reset_creature_guids.iter())
            .copied()
        {
            let Some(record) = self.map_objects.get_mut(&guid) else {
                missing_guids.push(guid);
                continue;
            };
            record.object_mut().object_mut().reset_all_notifies();
        }

        ResetNotifyFlagsOutcome {
            reset_player_guids,
            reset_creature_guids,
            missing_guids,
        }
    }

    pub fn delayed_unit_relocation_for_cells_like_cpp(
        &self,
        cells: impl IntoIterator<Item = CellCoord>,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> DelayedUnitRelocationForCellsPlan {
        let invalid_non_self_viewpoints: HashSet<_> =
            invalid_non_self_viewpoints.into_iter().collect();
        let mut cell_plans = Vec::new();

        for cell_coord in cells {
            let nearby = self.exact_cell_guids_like_cpp(cell_coord);
            let creatures_needing_notify = nearby
                .world
                .creatures
                .iter()
                .chain(nearby.grid.creatures.iter())
                .copied()
                .filter(|guid| self.object_needs_notify_visibility(*guid));
            let player_viewpoints_needing_notify = nearby
                .world
                .players
                .iter()
                .copied()
                .filter(|guid| self.object_needs_notify_visibility(*guid));

            let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(
                &nearby,
                creatures_needing_notify,
                player_viewpoints_needing_notify,
                invalid_non_self_viewpoints.iter().copied(),
            );
            if !plan.creature_relocations.is_empty()
                || !plan.player_relocations.is_empty()
                || !plan.skipped_invalid_viewpoints.is_empty()
            {
                cell_plans.push(DelayedUnitRelocationCellPlan { cell_coord, plan });
            }
        }

        DelayedUnitRelocationForCellsPlan { cell_plans }
    }

    pub fn delayed_unit_relocation_visibility_plans_like_cpp(
        &self,
        delayed_plan: &DelayedUnitRelocationForCellsPlan,
        player_contexts: impl IntoIterator<Item = DelayedPlayerRelocationContext>,
        creature_contexts: impl IntoIterator<Item = DelayedCreatureRelocationContext>,
    ) -> DelayedUnitRelocationVisibilityPlans {
        let player_contexts: HashMap<_, _> = player_contexts
            .into_iter()
            .map(|context| (context.player_guid, context))
            .collect();
        let creature_contexts: HashMap<_, _> = creature_contexts
            .into_iter()
            .map(|context| (context.creature_guid, context))
            .collect();
        let mut creature_plans = Vec::new();
        let mut player_plans = Vec::new();
        let mut skipped_missing_sources = Vec::new();
        let mut skipped_invalid_source_positions = Vec::new();
        let mut missing_player_contexts = Vec::new();

        for cell_plan in &delayed_plan.cell_plans {
            for creature_guid in &cell_plan.plan.creature_relocations {
                let Some(creature) = self.map_object(*creature_guid) else {
                    skipped_missing_sources.push(*creature_guid);
                    continue;
                };
                let position = creature.position();
                if !is_valid_map_coord_2d(position.x, position.y) {
                    skipped_invalid_source_positions.push(*creature_guid);
                    continue;
                }

                let nearby = self.nearby_cell_guids_like_cpp(
                    position.x,
                    position.y,
                    MAX_VISIBILITY_DISTANCE + creature.combat_reach(),
                );
                let player_seers_needing_notify = nearby
                    .world
                    .players
                    .iter()
                    .copied()
                    .filter(|guid| self.object_needs_notify_visibility(*guid));
                let creatures_needing_notify = nearby
                    .world
                    .creatures
                    .iter()
                    .chain(nearby.grid.creatures.iter())
                    .copied()
                    .filter(|guid| self.object_needs_notify_visibility(*guid));
                let source_creature_alive = creature_contexts
                    .get(creature_guid)
                    .is_none_or(|context| context.source_creature_alive);
                let visibility_plan = CreatureRelocationVisibilityPlan::from_nearby_like_cpp(
                    *creature_guid,
                    source_creature_alive,
                    &nearby,
                    player_seers_needing_notify,
                    creatures_needing_notify,
                );
                creature_plans.push(CreatureDelayedRelocationVisibilityPlan {
                    creature_guid: *creature_guid,
                    cell_coord: cell_plan.cell_coord,
                    nearby,
                    visibility_plan,
                });
            }

            for player_guid in &cell_plan.plan.player_relocations {
                let Some(context) = player_contexts.get(player_guid) else {
                    missing_player_contexts.push(*player_guid);
                    continue;
                };
                let Some(viewpoint) = self.map_object(context.viewpoint_guid) else {
                    skipped_missing_sources.push(context.viewpoint_guid);
                    continue;
                };
                let position = viewpoint.position();
                if !is_valid_map_coord_2d(position.x, position.y) {
                    skipped_invalid_source_positions.push(context.viewpoint_guid);
                    continue;
                }

                let nearby = self.nearby_cell_guids_like_cpp(
                    position.x,
                    position.y,
                    MAX_VISIBILITY_DISTANCE + viewpoint.combat_reach(),
                );
                let visibility_plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
                    *player_guid,
                    context.previous_client_guids.iter().copied(),
                    &nearby,
                    context.relocated_for_ai,
                );
                player_plans.push(PlayerDelayedRelocationVisibilityPlan {
                    player_guid: *player_guid,
                    viewpoint_guid: context.viewpoint_guid,
                    cell_coord: cell_plan.cell_coord,
                    nearby,
                    visibility_plan,
                });
            }
        }

        sort_dedup(&mut skipped_missing_sources);
        sort_dedup(&mut skipped_invalid_source_positions);
        sort_dedup(&mut missing_player_contexts);

        DelayedUnitRelocationVisibilityPlans {
            creature_plans,
            player_plans,
            skipped_missing_sources,
            skipped_invalid_source_positions,
            missing_player_contexts,
        }
    }

    pub fn process_map_object_move_list_like_cpp(
        &mut self,
        entries: impl IntoIterator<Item = MapObjectMoveListEntry>,
    ) -> MapObjectMoveListPlan {
        let mut plan = MapObjectMoveListPlan::default();

        for entry in entries {
            let Some(record) = self.map_object_record(entry.guid) else {
                plan.skipped_other_map_or_missing.push(entry.guid);
                continue;
            };
            if record.kind() != entry.kind {
                plan.skipped_kind_mismatch.push(entry.guid);
                continue;
            }

            if entry.move_state != MapObjectCellMoveState::Active {
                plan.reset_inactive_or_none.push(entry.guid);
                continue;
            }

            if !record.object().object().is_in_world() {
                plan.skipped_not_in_world.push(entry.guid);
                continue;
            }

            match self.relocate_map_object_like_cpp(entry.guid, entry.new_position) {
                Ok(outcome) if outcome.relocated => {
                    plan.relocated.push(entry.guid);
                    continue;
                }
                Ok(outcome) if outcome.blocked_by_unloaded_grid => {}
                Ok(_) => {}
                Err(MapObjectRelocationError::InvalidCoordinates { .. }) => {
                    plan.failed_invalid_position.push(entry.guid);
                    continue;
                }
                Err(MapObjectRelocationError::ObjectNotFound { .. }) => {
                    plan.skipped_other_map_or_missing.push(entry.guid);
                    continue;
                }
                Err(MapObjectRelocationError::Record(_) | MapObjectRelocationError::Store(_)) => {
                    plan.failed_store.push(entry.guid);
                    continue;
                }
            }

            match entry.kind {
                AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
                    if let Some(respawn_position) = entry.respawn_position
                        && self
                            .relocate_map_object_like_cpp(entry.guid, respawn_position)
                            .is_ok_and(|outcome| outcome.relocated)
                    {
                        plan.respawn_relocated.push(entry.guid);
                        continue;
                    }

                    if entry.kind == AccessorObjectKind::Pet || entry.is_pet {
                        plan.pet_removed.push(entry.guid);
                    } else {
                        plan.remove_from_world.push(entry.guid);
                    }
                }
                AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
                    if let Some(respawn_position) = entry.respawn_position
                        && self
                            .relocate_map_object_like_cpp(entry.guid, respawn_position)
                            .is_ok_and(|outcome| outcome.relocated)
                    {
                        plan.respawn_relocated.push(entry.guid);
                        continue;
                    }

                    plan.remove_from_world.push(entry.guid);
                }
                AccessorObjectKind::DynamicObject | AccessorObjectKind::AreaTrigger => {
                    plan.blocked_unloaded_grid.push(entry.guid);
                }
                AccessorObjectKind::Player
                | AccessorObjectKind::Corpse
                | AccessorObjectKind::SceneObject
                | AccessorObjectKind::Conversation => {
                    plan.unsupported_kind.push(entry.guid);
                }
            }
        }

        plan
    }

    pub fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord> {
        self.map_objects.get(&guid)
    }

    pub fn map_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_record(guid).map(MapObjectRecord::object)
    }

    fn object_is_in_world(&self, guid: ObjectGuid) -> bool {
        self.map_object(guid)
            .is_some_and(|object| object.object().is_in_world())
    }

    fn object_needs_notify_visibility(&self, guid: ObjectGuid) -> bool {
        self.map_object(guid).is_some_and(|object| {
            object
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        })
    }

    fn exact_cell_guids_like_cpp(&self, cell_coord: CellCoord) -> NearbyCellGuids {
        let mut nearby = NearbyCellGuids::default();
        let cell = Cell::from_cell_coord(cell_coord);
        let Some(grid) = self.get_ngrid(GridCoord::new(cell.grid_x(), cell.grid_y())) else {
            return nearby;
        };
        let Some(local_cell) = grid.get_grid_type(cell.cell_x(), cell.cell_y()) else {
            return nearby;
        };

        nearby.visited_cells = 1;
        nearby.merge_world(&local_cell.world_objects);
        nearby.merge_grid(&local_cell.grid_objects);
        nearby
    }

    pub fn map_object_by_kind(
        &self,
        guid: ObjectGuid,
        allowed: &[AccessorObjectKind],
    ) -> Option<&WorldObject> {
        let record = self.map_object_record(guid)?;
        allowed.contains(&record.kind()).then_some(record.object())
    }

    pub fn get_creature(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Creature])
    }

    pub fn get_typed_creature(&self, guid: ObjectGuid) -> Option<&Creature> {
        let record = self.map_object_record(guid)?;
        if record.kind() != AccessorObjectKind::Creature {
            return None;
        }
        record.creature()
    }

    pub fn get_typed_creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut Creature> {
        let record = self.map_objects.get_mut(&guid)?;
        if record.kind() != AccessorObjectKind::Creature {
            return None;
        }
        record.creature_mut()
    }

    pub fn get_pet(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Pet])
    }

    pub fn get_game_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(
            guid,
            &[
                AccessorObjectKind::GameObject,
                AccessorObjectKind::Transport,
            ],
        )
    }

    pub fn get_typed_game_object(&self, guid: ObjectGuid) -> Option<&GameObject> {
        let record = self.map_object_record(guid)?;
        if !matches!(
            record.kind(),
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport
        ) {
            return None;
        }
        record.game_object()
    }

    pub fn get_typed_game_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut GameObject> {
        let record = self.map_objects.get_mut(&guid)?;
        if !matches!(
            record.kind(),
            AccessorObjectKind::GameObject | AccessorObjectKind::Transport
        ) {
            return None;
        }
        record.game_object_mut()
    }

    pub fn get_typed_player(&self, guid: ObjectGuid) -> Option<&Player> {
        let record = self.map_object_record(guid)?;
        if record.kind() != AccessorObjectKind::Player {
            return None;
        }
        record.player()
    }

    pub fn get_typed_player_mut(&mut self, guid: ObjectGuid) -> Option<&mut Player> {
        let record = self.map_objects.get_mut(&guid)?;
        if record.kind() != AccessorObjectKind::Player {
            return None;
        }
        record.player_mut()
    }

    fn combat_unit_snapshot_like_cpp(
        &self,
        guid: ObjectGuid,
    ) -> Option<CombatUnitSnapshotLikeCpp<'_>> {
        if let Some(player) = self.get_typed_player(guid) {
            return Some(CombatUnitSnapshotLikeCpp {
                guid,
                unit: player.unit(),
                game_master_player: player.is_game_master_like_cpp(),
            });
        }
        self.get_typed_creature(guid)
            .map(|creature| CombatUnitSnapshotLikeCpp {
                guid,
                unit: creature.unit(),
                game_master_player: false,
            })
    }

    fn combat_begin_context_like_cpp(
        &self,
        owner: CombatUnitSnapshotLikeCpp<'_>,
        target: CombatUnitSnapshotLikeCpp<'_>,
    ) -> CombatBeginContextLikeCpp {
        let owner_world = owner.unit.world();
        let target_world = target.unit.world();
        CombatBeginContextLikeCpp {
            same_unit: owner.guid == target.guid,
            attacker_in_world: owner_world.object().is_in_world(),
            victim_in_world: target_world.object().is_in_world(),
            attacker_alive: owner.unit.is_alive(),
            victim_alive: target.unit.is_alive(),
            same_map: owner_world.is_in_map(target_world),
            same_phase: owner_world.in_same_phase(target_world),
            attacker_unit_state: owner.unit.unit_state(),
            victim_unit_state: target.unit.unit_state(),
            attacker_combat_disallowed: owner.unit.subsystems().combat.combat_disallowed,
            victim_combat_disallowed: target.unit.subsystems().combat.combat_disallowed,
            relation_represented: false,
            attacker_is_friendly_to_victim: false,
            victim_is_friendly_to_attacker: false,
            attacker_or_owner_player_is_game_master: owner.game_master_player,
            victim_or_owner_player_is_game_master: target.game_master_player,
        }
    }

    pub fn typed_combat_unit_guids_like_cpp(&self) -> Vec<ObjectGuid> {
        self.map_objects
            .iter()
            .filter_map(|(guid, record)| {
                matches!(
                    record.kind(),
                    AccessorObjectKind::Player | AccessorObjectKind::Creature
                )
                .then_some(*guid)
            })
            .collect()
    }

    pub fn revalidate_all_combat_refs_like_cpp(&mut self) -> Vec<(ObjectGuid, ObjectGuid)> {
        let owner_guids = self.typed_combat_unit_guids_like_cpp();
        let mut invalid = Vec::new();

        for owner_guid in owner_guids {
            let Some(owner) = self.combat_unit_snapshot_like_cpp(owner_guid) else {
                continue;
            };
            let refs: Vec<_> = owner
                .unit
                .subsystems()
                .combat
                .pve_refs
                .keys()
                .chain(owner.unit.subsystems().combat.pvp_refs.keys())
                .copied()
                .collect();

            for target_guid in refs {
                let Some(target) = self.combat_unit_snapshot_like_cpp(target_guid) else {
                    invalid.push((owner_guid, target_guid));
                    continue;
                };
                if !CombatSubsystem::can_begin_combat_like_cpp(
                    self.combat_begin_context_like_cpp(owner, target),
                ) {
                    invalid.push((owner_guid, target_guid));
                }
            }
        }

        for (owner_guid, target_guid) in &invalid {
            if let Some(owner) = self.get_typed_player_mut(*owner_guid) {
                owner
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*target_guid);
            } else if let Some(owner) = self.get_typed_creature_mut(*owner_guid) {
                owner
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*target_guid);
            }

            if let Some(target) = self.get_typed_player_mut(*target_guid) {
                target
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*owner_guid);
            } else if let Some(target) = self.get_typed_creature_mut(*target_guid) {
                target
                    .unit_mut()
                    .subsystems_mut()
                    .combat
                    .purge_combat_ref_like_cpp(*owner_guid);
            }
        }

        invalid
    }

    pub fn get_transport(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Transport])
    }

    pub fn get_dynamic_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::DynamicObject])
    }

    pub fn get_area_trigger(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::AreaTrigger])
    }

    pub fn get_corpse(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Corpse])
    }

    pub fn get_scene_object(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::SceneObject])
    }

    pub fn get_conversation(&self, guid: ObjectGuid) -> Option<&WorldObject> {
        self.map_object_by_kind(guid, &[AccessorObjectKind::Conversation])
    }

    fn validate_map_object(&self, object: &WorldObject) -> Result<(), MapObjectStoreError> {
        if object.map_id() == self.map_id && object.instance_id() == self.instance_id {
            return Ok(());
        }

        Err(MapObjectStoreError::WrongMap {
            guid: object.guid(),
            expected_map_id: self.map_id,
            expected_instance_id: self.instance_id,
            actual_map_id: object.map_id(),
            actual_instance_id: object.instance_id(),
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapObjectStoreError {
    InvalidRecord(ObjectAccessorError),
    WrongMap {
        guid: ObjectGuid,
        expected_map_id: u32,
        expected_instance_id: u32,
        actual_map_id: u32,
        actual_instance_id: u32,
    },
}

impl From<ObjectAccessorError> for MapObjectStoreError {
    fn from(error: ObjectAccessorError) -> Self {
        Self::InvalidRecord(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddToMapOutcome {
    pub guid: ObjectGuid,
    pub cell: CellCoord,
    pub grid: GridCoord,
    pub inserted: bool,
    pub already_in_world: bool,
    pub grid_created: bool,
    pub grid_loaded: bool,
    pub inserted_into_cell: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddToMapError {
    InvalidCoordinates { guid: ObjectGuid, x: f32, y: f32 },
    Store(MapObjectStoreError),
}

impl From<MapObjectStoreError> for AddToMapError {
    fn from(error: MapObjectStoreError) -> Self {
        Self::Store(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoveFromMapOutcome {
    pub guid: ObjectGuid,
    pub cell: CellCoord,
    pub grid: GridCoord,
    pub was_in_world: bool,
    pub was_active: bool,
    pub removed_from_cell: bool,
    pub delete_from_world: bool,
    pub object: Option<WorldObject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveFromMapError {
    ObjectNotFound { guid: ObjectGuid },
    ResetMap(MapBindingError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapObjectRelocationOutcome {
    pub guid: ObjectGuid,
    pub old_cell: CellCoord,
    pub new_cell: CellCoord,
    pub old_grid: GridCoord,
    pub new_grid: GridCoord,
    pub moved_between_cells: bool,
    pub loaded_grid: bool,
    pub created_grid: bool,
    pub relocated: bool,
    pub blocked_by_unloaded_grid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapObjectRelocationError {
    ObjectNotFound { guid: ObjectGuid },
    InvalidCoordinates { guid: ObjectGuid, x: f32, y: f32 },
    Record(ObjectAccessorError),
    Store(MapObjectStoreError),
}

#[derive(Debug, Clone, Default)]
pub struct NearbyCellGuids {
    pub world: WorldObjectGuids,
    pub grid: GridObjectGuids,
    pub visited_cells: usize,
}

impl NearbyCellGuids {
    pub fn is_empty(&self) -> bool {
        self.world.is_empty() && self.grid.is_empty()
    }

    pub fn len(&self) -> usize {
        self.world.len() + self.grid.len()
    }

    pub fn all_guids(&self) -> HashSet<ObjectGuid> {
        let mut guids = HashSet::with_capacity(self.len());
        guids.extend(self.world.players.iter().copied());
        guids.extend(self.world.creatures.iter().copied());
        guids.extend(self.world.corpses.iter().copied());
        guids.extend(self.world.dynamic_objects.iter().copied());
        guids.extend(self.grid.gameobjects.iter().copied());
        guids.extend(self.grid.creatures.iter().copied());
        guids.extend(self.grid.dynamic_objects.iter().copied());
        guids.extend(self.grid.corpses.iter().copied());
        guids.extend(self.grid.area_triggers.iter().copied());
        guids.extend(self.grid.scene_objects.iter().copied());
        guids.extend(self.grid.conversations.iter().copied());
        guids
    }

    fn merge_world(&mut self, other: &WorldObjectGuids) {
        self.world.players.extend(other.players.iter().copied());
        self.world.creatures.extend(other.creatures.iter().copied());
        self.world.corpses.extend(other.corpses.iter().copied());
        self.world
            .dynamic_objects
            .extend(other.dynamic_objects.iter().copied());
    }

    fn merge_grid(&mut self, other: &GridObjectGuids) {
        self.grid
            .gameobjects
            .extend(other.gameobjects.iter().copied());
        self.grid.creatures.extend(other.creatures.iter().copied());
        self.grid
            .dynamic_objects
            .extend(other.dynamic_objects.iter().copied());
        self.grid.corpses.extend(other.corpses.iter().copied());
        self.grid
            .area_triggers
            .extend(other.area_triggers.iter().copied());
        self.grid
            .scene_objects
            .extend(other.scene_objects.iter().copied());
        self.grid
            .conversations
            .extend(other.conversations.iter().copied());
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearbyCellVisitCenter {
    pub guid: ObjectGuid,
    pub activation_radius: f32,
}

#[derive(Debug, Clone, Default)]
pub struct NearbyCellVisitPlan {
    pub marked_cells: Vec<CellCoord>,
    pub nearby: NearbyCellGuids,
    pub skipped_missing_centers: Vec<ObjectGuid>,
    pub skipped_invalid_position_centers: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerRelocationVisibilityPlan {
    pub visible_guids: HashSet<ObjectGuid>,
    pub out_of_range_guids: HashSet<ObjectGuid>,
    pub reciprocal_player_updates: HashSet<ObjectGuid>,
    pub ai_relocation_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl PlayerRelocationVisibilityPlan {
    pub fn from_nearby_like_cpp(
        player_guid: ObjectGuid,
        previous_client_guids: impl IntoIterator<Item = ObjectGuid>,
        nearby: &NearbyCellGuids,
        relocated_for_ai: bool,
    ) -> Self {
        let visible_guids = nearby.all_guids();
        let mut out_of_range_guids: HashSet<_> = previous_client_guids.into_iter().collect();
        out_of_range_guids.remove(&player_guid);

        let mut reciprocal_player_updates = HashSet::new();
        let mut ai_relocation_checks = Vec::new();
        for guid in &visible_guids {
            out_of_range_guids.remove(guid);

            if guid.is_player() && *guid != player_guid {
                reciprocal_player_updates.insert(*guid);
            } else if relocated_for_ai && guid.is_any_type_creature() {
                ai_relocation_checks.push((*guid, player_guid));
            }
        }

        for guid in &out_of_range_guids {
            if guid.is_player() {
                reciprocal_player_updates.insert(*guid);
            }
        }

        Self {
            visible_guids,
            out_of_range_guids,
            reciprocal_player_updates,
            ai_relocation_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreatureRelocationVisibilityPlan {
    pub player_visibility_updates: HashSet<ObjectGuid>,
    pub ai_relocation_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl CreatureRelocationVisibilityPlan {
    pub fn from_nearby_like_cpp(
        creature_guid: ObjectGuid,
        source_creature_alive: bool,
        nearby: &NearbyCellGuids,
        player_seers_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        creatures_needing_notify: impl IntoIterator<Item = ObjectGuid>,
    ) -> Self {
        let player_seers_needing_notify: HashSet<_> =
            player_seers_needing_notify.into_iter().collect();
        let creatures_needing_notify: HashSet<_> = creatures_needing_notify.into_iter().collect();
        let mut player_visibility_updates = HashSet::new();
        let mut ai_relocation_checks = Vec::new();

        for player in &nearby.world.players {
            if !player_seers_needing_notify.contains(player) {
                player_visibility_updates.insert(*player);
            }
            ai_relocation_checks.push((creature_guid, *player));
        }

        if source_creature_alive {
            for creature in nearby_creature_guids_excluding(nearby, creature_guid) {
                ai_relocation_checks.push((creature_guid, creature));
                if !creatures_needing_notify.contains(&creature) {
                    ai_relocation_checks.push((creature, creature_guid));
                }
            }
        }

        Self {
            player_visibility_updates,
            ai_relocation_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelayedUnitRelocationPlan {
    pub creature_relocations: Vec<ObjectGuid>,
    pub player_relocations: Vec<ObjectGuid>,
    pub skipped_invalid_viewpoints: Vec<ObjectGuid>,
}

impl DelayedUnitRelocationPlan {
    pub fn from_nearby_like_cpp(
        nearby: &NearbyCellGuids,
        creatures_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        player_viewpoints_needing_notify: impl IntoIterator<Item = ObjectGuid>,
        invalid_non_self_viewpoints: impl IntoIterator<Item = ObjectGuid>,
    ) -> Self {
        let creatures_needing_notify: HashSet<_> = creatures_needing_notify.into_iter().collect();
        let player_viewpoints_needing_notify: HashSet<_> =
            player_viewpoints_needing_notify.into_iter().collect();
        let invalid_non_self_viewpoints: HashSet<_> =
            invalid_non_self_viewpoints.into_iter().collect();

        let mut creature_relocations: Vec<_> = nearby
            .world
            .creatures
            .iter()
            .chain(nearby.grid.creatures.iter())
            .copied()
            .filter(|guid| creatures_needing_notify.contains(guid))
            .collect();
        creature_relocations.sort();
        creature_relocations.dedup();

        let mut player_relocations = Vec::new();
        let mut skipped_invalid_viewpoints = Vec::new();
        let mut players: Vec<_> = nearby.world.players.iter().copied().collect();
        players.sort();
        for player in players {
            if !player_viewpoints_needing_notify.contains(&player) {
                continue;
            }

            if invalid_non_self_viewpoints.contains(&player) {
                skipped_invalid_viewpoints.push(player);
            } else {
                player_relocations.push(player);
            }
        }

        Self {
            creature_relocations,
            player_relocations,
            skipped_invalid_viewpoints,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelayedUnitRelocationForCellsPlan {
    pub cell_plans: Vec<DelayedUnitRelocationCellPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedUnitRelocationCellPlan {
    pub cell_coord: CellCoord,
    pub plan: DelayedUnitRelocationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedPlayerRelocationContext {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: ObjectGuid,
    pub previous_client_guids: Vec<ObjectGuid>,
    pub relocated_for_ai: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelayedCreatureRelocationContext {
    pub creature_guid: ObjectGuid,
    pub source_creature_alive: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DelayedUnitRelocationVisibilityPlans {
    pub creature_plans: Vec<CreatureDelayedRelocationVisibilityPlan>,
    pub player_plans: Vec<PlayerDelayedRelocationVisibilityPlan>,
    pub skipped_missing_sources: Vec<ObjectGuid>,
    pub skipped_invalid_source_positions: Vec<ObjectGuid>,
    pub missing_player_contexts: Vec<ObjectGuid>,
}

#[derive(Debug, Clone)]
pub struct CreatureDelayedRelocationVisibilityPlan {
    pub creature_guid: ObjectGuid,
    pub cell_coord: CellCoord,
    pub nearby: NearbyCellGuids,
    pub visibility_plan: CreatureRelocationVisibilityPlan,
}

#[derive(Debug, Clone)]
pub struct PlayerDelayedRelocationVisibilityPlan {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: ObjectGuid,
    pub cell_coord: CellCoord,
    pub nearby: NearbyCellGuids,
    pub visibility_plan: PlayerRelocationVisibilityPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AIRelocationPlan {
    pub creature_unit_checks: Vec<(ObjectGuid, ObjectGuid)>,
}

impl AIRelocationPlan {
    pub fn from_nearby_like_cpp(
        unit_guid: ObjectGuid,
        unit_is_creature: bool,
        nearby: &NearbyCellGuids,
    ) -> Self {
        let nearby_creatures = nearby_creature_guids_excluding(nearby, unit_guid);
        let mut creature_unit_checks = Vec::with_capacity(if unit_is_creature {
            nearby_creatures.len() * 2
        } else {
            nearby_creatures.len()
        });

        for creature in nearby_creatures {
            creature_unit_checks.push((creature, unit_guid));
            if unit_is_creature {
                creature_unit_checks.push((unit_guid, creature));
            }
        }

        Self {
            creature_unit_checks,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectUpdatePlan {
    pub diff_ms: u32,
    pub update_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapUpdatePlayerSources {
    pub player_guid: ObjectGuid,
    pub viewpoint_guid: Option<ObjectGuid>,
    pub far_combat_unit_guids: Vec<ObjectGuid>,
    pub far_aura_caster_guids: Vec<ObjectGuid>,
    pub far_summon_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapUpdateVisitPlan {
    pub diff_ms: u32,
    pub session_update_players: Vec<ObjectGuid>,
    pub player_update_guids: Vec<ObjectGuid>,
    pub nearby_visit_centers: Vec<ObjectGuid>,
    pub transport_update_guids: Vec<ObjectGuid>,
    pub process_relocation_notifies: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelocationNotifyProcessPlan {
    pub diff_ms: u32,
    pub delayed_relocation_cells: Vec<CellCoord>,
    pub reset_notify_cells: Vec<CellCoord>,
    pub reset_timer_grids: Vec<GridCoord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessRelocationNotifiesOutcome {
    pub process_plan: RelocationNotifyProcessPlan,
    pub delayed_plan: DelayedUnitRelocationForCellsPlan,
    pub reset_outcome: ResetNotifyFlagsOutcome,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResetNotifyFlagsOutcome {
    pub reset_player_guids: Vec<ObjectGuid>,
    pub reset_creature_guids: Vec<ObjectGuid>,
    pub missing_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapObjectCellMoveState {
    None,
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapObjectMoveListEntry {
    pub guid: ObjectGuid,
    pub kind: AccessorObjectKind,
    pub move_state: MapObjectCellMoveState,
    pub new_position: Position,
    pub respawn_position: Option<Position>,
    pub is_pet: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapObjectMoveListPlan {
    pub relocated: Vec<ObjectGuid>,
    pub respawn_relocated: Vec<ObjectGuid>,
    pub remove_from_world: Vec<ObjectGuid>,
    pub pet_removed: Vec<ObjectGuid>,
    pub blocked_unloaded_grid: Vec<ObjectGuid>,
    pub reset_inactive_or_none: Vec<ObjectGuid>,
    pub skipped_not_in_world: Vec<ObjectGuid>,
    pub skipped_other_map_or_missing: Vec<ObjectGuid>,
    pub skipped_kind_mismatch: Vec<ObjectGuid>,
    pub failed_invalid_position: Vec<ObjectGuid>,
    pub failed_store: Vec<ObjectGuid>,
    pub unsupported_kind: Vec<ObjectGuid>,
}

fn is_active_object_like_cpp(kind: AccessorObjectKind, object: &WorldObject) -> bool {
    kind == AccessorObjectKind::Player || object.is_active()
}

fn push_in_world_guids<Terrain, Lifecycle>(
    map: &Map<Terrain, Lifecycle>,
    target: &mut Vec<ObjectGuid>,
    guids: impl IntoIterator<Item = ObjectGuid>,
) where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    target.extend(
        guids
            .into_iter()
            .filter(|guid| map.object_is_in_world(*guid)),
    );
}

fn sort_dedup(guids: &mut Vec<ObjectGuid>) {
    guids.sort();
    guids.dedup();
}

fn marked_cells_in_grid_like_cpp(
    grid: GridCoord,
    marked_cells: &HashSet<CellCoord>,
) -> Vec<CellCoord> {
    let cell_min_x = grid.x_coord * MAX_NUMBER_OF_CELLS;
    let cell_min_y = grid.y_coord * MAX_NUMBER_OF_CELLS;
    let cell_max_x = cell_min_x + MAX_NUMBER_OF_CELLS;
    let cell_max_y = cell_min_y + MAX_NUMBER_OF_CELLS;
    let mut cells = Vec::new();

    for x in cell_min_x..cell_max_x {
        for y in cell_min_y..cell_max_y {
            let cell = CellCoord::new(x, y);
            if marked_cells.contains(&cell) {
                cells.push(cell);
            }
        }
    }

    cells
}

fn nearby_creature_guids_excluding(
    nearby: &NearbyCellGuids,
    excluded: ObjectGuid,
) -> Vec<ObjectGuid> {
    let mut nearby_creatures: Vec<_> = nearby
        .world
        .creatures
        .iter()
        .chain(nearby.grid.creatures.iter())
        .copied()
        .filter(|guid| *guid != excluded)
        .collect();
    nearby_creatures.sort();
    nearby_creatures.dedup();
    nearby_creatures
}

fn insert_object_guid_in_cell_like_cpp(
    cell: &mut Cell,
    kind: AccessorObjectKind,
    is_world_object: bool,
    guid: ObjectGuid,
) {
    match kind {
        AccessorObjectKind::Player => {
            cell.world_objects.players.insert(guid);
        }
        AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
            if is_world_object {
                cell.world_objects.creatures.insert(guid);
            } else {
                cell.grid_objects.creatures.insert(guid);
            }
        }
        AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
            cell.grid_objects.gameobjects.insert(guid);
        }
        AccessorObjectKind::DynamicObject => {
            if is_world_object {
                cell.world_objects.dynamic_objects.insert(guid);
            } else {
                cell.grid_objects.dynamic_objects.insert(guid);
            }
        }
        AccessorObjectKind::AreaTrigger => {
            cell.grid_objects.area_triggers.insert(guid);
        }
        AccessorObjectKind::Corpse => {
            if is_world_object {
                cell.world_objects.corpses.insert(guid);
            } else {
                cell.grid_objects.corpses.insert(guid);
            }
        }
        AccessorObjectKind::SceneObject => {
            cell.grid_objects.scene_objects.insert(guid);
        }
        AccessorObjectKind::Conversation => {
            cell.grid_objects.conversations.insert(guid);
        }
    }
}

fn remove_object_guid_from_cell_like_cpp<Terrain, Lifecycle>(
    map: &mut Map<Terrain, Lifecycle>,
    grid: GridCoord,
    cell: &Cell,
    kind: AccessorObjectKind,
    is_world_object: bool,
    guid: ObjectGuid,
) -> bool
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    let Some(ngrid) = map.get_ngrid_mut(grid) else {
        return false;
    };
    let Some(local_cell) = ngrid.get_grid_type_mut(cell.cell_x(), cell.cell_y()) else {
        return false;
    };

    match kind {
        AccessorObjectKind::Player => local_cell.world_objects.players.remove(&guid),
        AccessorObjectKind::Creature | AccessorObjectKind::Pet => {
            if is_world_object {
                local_cell.world_objects.creatures.remove(&guid)
            } else {
                local_cell.grid_objects.creatures.remove(&guid)
            }
        }
        AccessorObjectKind::GameObject | AccessorObjectKind::Transport => {
            local_cell.grid_objects.gameobjects.remove(&guid)
        }
        AccessorObjectKind::DynamicObject => {
            if is_world_object {
                local_cell.world_objects.dynamic_objects.remove(&guid)
            } else {
                local_cell.grid_objects.dynamic_objects.remove(&guid)
            }
        }
        AccessorObjectKind::AreaTrigger => local_cell.grid_objects.area_triggers.remove(&guid),
        AccessorObjectKind::Corpse => {
            if is_world_object {
                local_cell.world_objects.corpses.remove(&guid)
            } else {
                local_cell.grid_objects.corpses.remove(&guid)
            }
        }
        AccessorObjectKind::SceneObject => local_cell.grid_objects.scene_objects.remove(&guid),
        AccessorObjectKind::Conversation => local_cell.grid_objects.conversations.remove(&guid),
    }
}

impl<Terrain, Lifecycle> ObjectAccessorMapSource for Map<Terrain, Lifecycle>
where
    Terrain: TerrainGridLoader,
    Lifecycle: GridLifecycle,
{
    fn map_id(&self) -> u32 {
        self.map_id
    }

    fn instance_id(&self) -> u32 {
        self.instance_id
    }

    fn map_object_record(&self, guid: ObjectGuid) -> Option<&MapObjectRecord> {
        self.map_objects.get(&guid)
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
    use wow_constants::{DeathState, TypeId, TypeMask};
    use wow_core::{ObjectGuid, Position, guid::HighGuid};
    use wow_entities::{AccessorObjectRef, Creature, ObjectAccessor, ObjectNotifyFlags, Player};

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

    fn dynamic_respawn_context(
        spawn_type: Option<SpawnObjectType>,
    ) -> DynamicRespawnScalingContext {
        DynamicRespawnScalingContext {
            mode: 1,
            spawn_type,
            spawn_metadata_present: true,
            spawn_group_flags: Some(SpawnGroupFlags::DYNAMIC_SPAWN_RATE),
            is_battleground_or_arena: false,
            zone_player_count: Some(4),
            config: DynamicRespawnScalingConfig {
                creature_rate: 1.0,
                creature_minimum_secs: 30,
                gameobject_rate: 1.5,
                gameobject_minimum_secs: 60,
            },
        }
    }

    fn assert_dynamic_respawn_noop(
        context: DynamicRespawnScalingContext,
        reason: DynamicRespawnScalingNoopReason,
    ) {
        let outcome = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);
        assert_eq!(outcome.delay_secs, 120);
        assert_eq!(outcome.noop_reason, Some(reason));
        assert!(!outcome.was_scaled());
    }

    #[test]
    fn dynamic_respawn_bg_or_arena_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.is_battleground_or_arena = true;

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::BattlegroundOrArena,
        );
    }

    #[test]
    fn dynamic_respawn_unsupported_type_and_missing_metadata_do_not_scale() {
        assert_dynamic_respawn_noop(
            dynamic_respawn_context(Some(SpawnObjectType::AreaTrigger)),
            DynamicRespawnScalingNoopReason::UnsupportedSpawnType,
        );

        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.spawn_metadata_present = false;
        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::MissingSpawnMetadata,
        );
    }

    #[test]
    fn dynamic_respawn_without_dynamic_spawn_rate_flag_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.spawn_group_flags = Some(SpawnGroupFlags::NONE);

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::MissingDynamicSpawnRateFlag,
        );
    }

    #[test]
    fn dynamic_respawn_missing_or_zero_players_do_not_scale() {
        let mut missing = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        missing.zone_player_count = None;
        assert_dynamic_respawn_noop(
            missing,
            DynamicRespawnScalingNoopReason::MissingZonePlayerCount,
        );

        let mut zero = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        zero.zone_player_count = Some(0);
        assert_dynamic_respawn_noop(zero, DynamicRespawnScalingNoopReason::ZeroZonePlayers);
    }

    #[test]
    fn dynamic_respawn_adjust_factor_at_least_one_does_not_scale() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.zone_player_count = Some(1);
        context.config.gameobject_rate = 1.0;

        assert_dynamic_respawn_noop(
            context,
            DynamicRespawnScalingNoopReason::AdjustFactorAtLeastOne,
        );
    }

    #[test]
    fn dynamic_respawn_delay_at_or_below_minimum_does_not_scale() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        let outcome = apply_dynamic_mode_respawn_scaling_like_cpp(60, context);

        assert_eq!(outcome.delay_secs, 60);
        assert_eq!(
            outcome.noop_reason,
            Some(DynamicRespawnScalingNoopReason::DelayAtOrBelowMinimum)
        );
    }

    #[test]
    fn dynamic_respawn_gameobject_ceil_scales_and_clamps_to_minimum() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        let scaled = apply_dynamic_mode_respawn_scaling_like_cpp(241, context);

        assert_eq!(scaled.delay_secs, 91);
        assert!(scaled.was_scaled());

        let clamped = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);
        assert_eq!(clamped.delay_secs, 60);
        assert!(clamped.was_scaled());
    }

    #[test]
    fn dynamic_respawn_creature_uses_creature_rate_and_minimum() {
        let context = dynamic_respawn_context(Some(SpawnObjectType::Creature));
        let scaled = apply_dynamic_mode_respawn_scaling_like_cpp(120, context);

        assert_eq!(scaled.delay_secs, 30);
        assert!(scaled.was_scaled());
    }

    #[test]
    fn dynamic_respawn_unsupported_mode_is_safe_noop() {
        let mut context = dynamic_respawn_context(Some(SpawnObjectType::GameObject));
        context.mode = 2;

        assert_dynamic_respawn_noop(context, DynamicRespawnScalingNoopReason::UnsupportedMode);
    }

    fn guid(high: HighGuid, counter: i64) -> ObjectGuid {
        if high == HighGuid::Player {
            ObjectGuid::create_global(high, 0, counter)
        } else if high == HighGuid::Transport {
            ObjectGuid::create_transport(high, counter)
        } else {
            ObjectGuid::create_world_object(high, 0, 1, 571, 7, 100, counter)
        }
    }

    fn world_object(high: HighGuid, map_id: u32, instance_id: u32, in_world: bool) -> WorldObject {
        let type_id = guid(high, 1).type_id();
        let type_mask = match type_id {
            wow_core::guid::TypeId::Player => TypeMask::PLAYER,
            wow_core::guid::TypeId::Unit => TypeMask::UNIT,
            wow_core::guid::TypeId::GameObject => TypeMask::GAME_OBJECT,
            wow_core::guid::TypeId::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            wow_core::guid::TypeId::Corpse => TypeMask::CORPSE,
            wow_core::guid::TypeId::AreaTrigger => TypeMask::AREA_TRIGGER,
            wow_core::guid::TypeId::SceneObject => TypeMask::SCENE_OBJECT,
            wow_core::guid::TypeId::Conversation => TypeMask::CONVERSATION,
            _ => TypeMask::OBJECT,
        };
        let mut object = WorldObject::new(false, convert_type_id(type_id), type_mask);
        object.object_mut().create(guid(high, 1));
        object.set_map(map_id, instance_id).unwrap();
        object.relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            object.object_mut().add_to_world();
        }
        object
    }

    fn world_object_with_counter(
        high: HighGuid,
        counter: i64,
        map_id: u32,
        instance_id: u32,
        in_world: bool,
    ) -> WorldObject {
        let object_guid = guid(high, counter);
        let type_id = object_guid.type_id();
        let type_mask = match type_id {
            wow_core::guid::TypeId::Player => TypeMask::PLAYER,
            wow_core::guid::TypeId::Unit => TypeMask::UNIT,
            wow_core::guid::TypeId::GameObject => TypeMask::GAME_OBJECT,
            wow_core::guid::TypeId::DynamicObject => TypeMask::DYNAMIC_OBJECT,
            wow_core::guid::TypeId::Corpse => TypeMask::CORPSE,
            wow_core::guid::TypeId::AreaTrigger => TypeMask::AREA_TRIGGER,
            wow_core::guid::TypeId::SceneObject => TypeMask::SCENE_OBJECT,
            wow_core::guid::TypeId::Conversation => TypeMask::CONVERSATION,
            _ => TypeMask::OBJECT,
        };
        let mut object = WorldObject::new(false, convert_type_id(type_id), type_mask);
        object.object_mut().create(object_guid);
        object.set_map(map_id, instance_id).unwrap();
        object.relocate(Position::xyz(1.0, 2.0, 3.0));
        if in_world {
            object.object_mut().add_to_world();
        }
        object
    }

    fn convert_type_id(type_id: wow_core::guid::TypeId) -> TypeId {
        match type_id {
            wow_core::guid::TypeId::Object => TypeId::Object,
            wow_core::guid::TypeId::Item => TypeId::Item,
            wow_core::guid::TypeId::Container => TypeId::Container,
            wow_core::guid::TypeId::AzeriteEmpoweredItem => TypeId::AzeriteEmpoweredItem,
            wow_core::guid::TypeId::AzeriteItem => TypeId::AzeriteItem,
            wow_core::guid::TypeId::Unit => TypeId::Unit,
            wow_core::guid::TypeId::Player => TypeId::Player,
            wow_core::guid::TypeId::ActivePlayer => TypeId::ActivePlayer,
            wow_core::guid::TypeId::GameObject => TypeId::GameObject,
            wow_core::guid::TypeId::DynamicObject => TypeId::DynamicObject,
            wow_core::guid::TypeId::Corpse => TypeId::Corpse,
            wow_core::guid::TypeId::AreaTrigger => TypeId::AreaTrigger,
            wow_core::guid::TypeId::SceneObject => TypeId::SceneObject,
            wow_core::guid::TypeId::Conversation => TypeId::Conversation,
        }
    }

    #[test]
    fn map_constructor_starts_with_empty_grid_slots_like_cpp_pointer_array() {
        let map = test_map();

        assert_eq!(map.map_id(), 571);
        assert_eq!(map.instance_id(), 7);
        assert_eq!(map.spawn_mode(), 1);
        assert_eq!(map.grid_expiry_ms(), 1000);
        assert!(map.grid_unload());
        assert_eq!(map.visibility_range(), 100.0);
        assert_eq!(map.grids.len(), GRID_SLOT_COUNT);
        assert!(map.grids.iter().all(Option::is_none));
    }

    #[test]
    fn map_object_store_inserts_finds_typed_objects_and_removes_by_guid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let gameobject = world_object(HighGuid::GameObject, 571, 7, true);
        let creature_guid = creature.guid();
        let gameobject_guid = gameobject.guid();

        assert!(
            map.insert_map_object(AccessorObjectKind::Creature, creature)
                .unwrap()
                .is_none()
        );
        assert!(
            map.insert_map_object(AccessorObjectKind::GameObject, gameobject)
                .unwrap()
                .is_none()
        );

        assert_eq!(map.map_object_count(), 2);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().guid(),
            creature_guid
        );
        assert_eq!(
            map.get_game_object(gameobject_guid).unwrap().guid(),
            gameobject_guid
        );
        assert!(map.get_game_object(creature_guid).is_none());

        assert_eq!(
            map.remove_map_object(creature_guid)
                .unwrap()
                .object()
                .guid(),
            creature_guid
        );
        assert!(map.get_creature(creature_guid).is_none());
        assert_eq!(map.map_object_count(), 1);
    }

    #[test]
    fn map_object_store_can_hold_typed_gameobject_entity_like_cpp() {
        let mut map = test_map();
        let mut gameobject = GameObject::new();
        let guid = guid(HighGuid::GameObject, 77);
        gameobject.world_mut().object_mut().create(guid);
        gameobject.world_mut().object_mut().set_entry(123);
        gameobject.world_mut().set_map(571, 7).unwrap();
        gameobject
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        gameobject.set_created_by(ObjectGuid::create_player(1, 42));

        map.insert_map_object_record(MapObjectRecord::new_game_object(gameobject).unwrap())
            .unwrap();

        assert_eq!(map.get_game_object(guid).unwrap().guid(), guid);
        assert_eq!(
            map.get_typed_game_object(guid).unwrap().owner_guid(),
            ObjectGuid::create_player(1, 42)
        );
    }

    #[test]
    fn map_object_store_can_hold_typed_creature_entity_like_cpp() {
        let mut map = test_map();
        let mut creature = Creature::new(false);
        let guid = guid(HighGuid::Creature, 78);
        creature.unit_mut().world_mut().object_mut().create(guid);
        creature.unit_mut().world_mut().object_mut().set_entry(321);
        creature.unit_mut().world_mut().set_map(571, 7).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        creature.unit_mut().world_mut().object_mut().add_to_world();
        creature.unit_mut().set_level(42);

        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        assert_eq!(map.get_creature(guid).unwrap().guid(), guid);
        assert_eq!(
            map.get_typed_creature(guid).unwrap().unit().data().level,
            42
        );
        map.get_typed_creature_mut(guid)
            .unwrap()
            .unit_mut()
            .set_level(43);
        assert_eq!(
            map.get_typed_creature(guid).unwrap().unit().data().level,
            43
        );
    }

    #[test]
    fn map_object_store_can_hold_typed_player_entity_like_cpp() {
        let mut map = test_map();
        let mut player = Player::new(Some(7), false);
        let player_guid = guid(HighGuid::Player, 42);
        let victim_guid = guid(HighGuid::Creature, 77);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(player_guid);
        player.unit_mut().world_mut().set_map(571, 7).unwrap();
        player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        player.unit_mut().world_mut().object_mut().add_to_world();
        player.unit_mut().set_attacking(Some(victim_guid));

        map.insert_map_object_record(MapObjectRecord::new_player(player).unwrap())
            .unwrap();

        assert_eq!(map.map_object(player_guid).unwrap().guid(), player_guid);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .unit()
                .attacking(),
            Some(victim_guid)
        );
        map.get_typed_player_mut(player_guid)
            .unwrap()
            .unit_mut()
            .set_attacking(None);
        assert_eq!(
            map.get_typed_player(player_guid)
                .unwrap()
                .unit()
                .attacking(),
            None
        );
    }

    #[test]
    fn map_revalidates_all_typed_combat_refs_like_cpp_multi_owner_sweep() {
        let mut map = test_map();
        let alive_player_guid = guid(HighGuid::Player, 501);
        let dead_player_guid = guid(HighGuid::Player, 502);
        let creature_guid = guid(HighGuid::Creature, 503);

        let mut alive_player = Player::new(Some(7), false);
        alive_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(alive_player_guid);
        alive_player.unit_mut().world_mut().set_map(571, 7).unwrap();
        alive_player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(10.0, 20.0, 30.0));
        alive_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();

        let mut dead_player = Player::new(Some(7), false);
        dead_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(dead_player_guid);
        dead_player.unit_mut().world_mut().set_map(571, 7).unwrap();
        dead_player
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(11.0, 20.0, 30.0));
        dead_player
            .unit_mut()
            .world_mut()
            .object_mut()
            .add_to_world();
        dead_player.unit_mut().set_death_state(DeathState::Dead);

        let mut creature = Creature::new(false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(creature_guid);
        creature.unit_mut().world_mut().set_map(571, 7).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::xyz(12.0, 20.0, 30.0));
        creature.unit_mut().world_mut().object_mut().add_to_world();

        map.insert_map_object_record(MapObjectRecord::new_player(alive_player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_player(dead_player).unwrap())
            .unwrap();
        map.insert_map_object_record(MapObjectRecord::new_creature(creature).unwrap())
            .unwrap();

        map.get_typed_player_mut(alive_player_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(creature_guid, false, false);
        map.get_typed_creature_mut(creature_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(alive_player_guid, false, false);
        map.get_typed_player_mut(dead_player_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(creature_guid, false, false);
        map.get_typed_creature_mut(creature_guid)
            .unwrap()
            .unit_mut()
            .subsystems_mut()
            .combat
            .set_in_combat_with(dead_player_guid, false, false);

        let invalid = map.revalidate_all_combat_refs_like_cpp();

        assert!(invalid.contains(&(dead_player_guid, creature_guid)));
        assert!(invalid.contains(&(creature_guid, dead_player_guid)));
        assert!(
            map.get_typed_player(alive_player_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(creature_guid)
        );
        assert!(
            map.get_typed_creature(creature_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(alive_player_guid)
        );
        assert!(
            !map.get_typed_player(dead_player_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(creature_guid)
        );
        assert!(
            !map.get_typed_creature(creature_guid)
                .unwrap()
                .unit()
                .subsystems()
                .combat
                .is_in_combat_with(dead_player_guid)
        );
    }

    #[test]
    fn map_object_store_rejects_records_from_other_map_or_instance() {
        let mut map = test_map();
        let other_map_creature = world_object(HighGuid::Creature, 530, 7, true);
        let other_instance_creature = world_object(HighGuid::Creature, 571, 8, true);

        assert!(matches!(
            map.insert_map_object(AccessorObjectKind::Creature, other_map_creature),
            Err(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                expected_instance_id: 7,
                actual_map_id: 530,
                actual_instance_id: 7,
                ..
            })
        ));
        assert!(matches!(
            map.insert_map_object(AccessorObjectKind::Creature, other_instance_creature),
            Err(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                expected_instance_id: 7,
                actual_map_id: 571,
                actual_instance_id: 8,
                ..
            })
        ));
        assert_eq!(map.map_object_count(), 0);
    }

    #[test]
    fn object_accessor_can_consult_map_owned_object_store() {
        let accessor = ObjectAccessor::default();
        let mut map = test_map();
        let context = world_object(HighGuid::Player, 571, 7, true);
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();

        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();

        assert_eq!(
            accessor
                .get_world_object_from_map_source(&context, &map, creature_guid)
                .unwrap()
                .guid(),
            creature_guid
        );
        assert!(matches!(
            accessor.get_object_ref_by_type_mask_from_map_source(
                &context,
                &map,
                creature_guid,
                TypeMask::UNIT
            ),
            Some(AccessorObjectRef::WorldObject(object)) if object.guid() == creature_guid
        ));
    }

    #[test]
    fn add_to_map_like_cpp_creates_grid_marks_world_and_stores_grid_object() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.inserted);
        assert!(!outcome.already_in_world);
        assert!(outcome.grid_created);
        assert!(!outcome.grid_loaded);
        assert!(outcome.inserted_into_cell);

        let stored = map.get_creature(guid).unwrap();
        assert!(stored.object().is_in_world());
        assert!(stored.object().is_in_grid());
        assert!(!stored.object().is_new_object());
        assert_eq!(
            stored.current_cell(),
            Some((
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS
            ))
        );

        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.grid_objects.creatures.contains(&guid));
        assert!(!cell.world_objects.creatures.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_active_world_object_loads_grid_and_world_container() {
        let mut map = test_map();
        let mut object = WorldObject::new(true, TypeId::DynamicObject, TypeMask::DYNAMIC_OBJECT);
        object.object_mut().create(guid(HighGuid::DynamicObject, 2));
        object.set_map(571, 7).unwrap();
        object.relocate(Position::xyz(20.0, 20.0, 3.0));
        object.set_active(true);
        let guid = object.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::DynamicObject, object)
            .unwrap();

        assert!(outcome.grid_loaded);
        assert!(!outcome.grid_created);
        assert!(map.is_grid_loaded(outcome.grid));
        assert_eq!(map.lifecycle().loads, 1);
        let grid = map.get_ngrid(outcome.grid).unwrap();
        assert_eq!(grid.state(), GridStateKind::Active);
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.world_objects.dynamic_objects.contains(&guid));
        assert!(!cell.grid_objects.dynamic_objects.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_player_is_active_even_without_runtime_active_flag() {
        let mut map = test_map();
        let player = world_object(HighGuid::Player, 571, 7, false);
        let guid = player.guid();

        let outcome = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();

        assert_eq!(outcome.guid, guid);
        assert!(outcome.grid_loaded);
        assert!(!outcome.grid_created);
        assert!(map.is_grid_loaded(outcome.grid));
        let grid = map.get_ngrid(outcome.grid).unwrap();
        let cell = grid
            .get_grid_type(
                outcome.cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(cell.world_objects.players.contains(&guid));
    }

    #[test]
    fn add_to_map_like_cpp_rejects_invalid_coordinates_before_grid_mutation() {
        let mut map = test_map();
        let mut creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        creature.relocate(Position::xyz(f32::NAN, 0.0, 0.0));

        assert!(matches!(
            map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature),
            Err(AddToMapError::InvalidCoordinates { guid: actual, .. }) if actual == guid
        ));
        assert_eq!(map.map_object_count(), 0);
        assert!(map.terrain().loads.is_empty());
    }

    #[test]
    fn add_to_map_like_cpp_rejects_wrong_map_before_grid_mutation() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 530, 7, false);

        assert!(matches!(
            map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature),
            Err(AddToMapError::Store(MapObjectStoreError::WrongMap {
                expected_map_id: 571,
                actual_map_id: 530,
                ..
            }))
        ));
        assert_eq!(map.map_object_count(), 0);
        assert!(map.terrain().loads.is_empty());
    }

    #[test]
    fn remove_from_map_like_cpp_removes_store_cell_and_resets_object_binding() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        assert!(map.get_creature(guid).is_some());

        let removed = map.remove_from_map_like_cpp(guid, false).unwrap();

        assert_eq!(removed.guid, guid);
        assert_eq!(removed.cell, added.cell);
        assert!(removed.was_in_world);
        assert!(!removed.was_active);
        assert!(removed.removed_from_cell);
        assert!(!removed.delete_from_world);
        assert!(map.get_creature(guid).is_none());

        let grid = map.get_ngrid(removed.grid).unwrap();
        let cell = grid
            .get_grid_type(
                removed.cell.x_coord % MAX_NUMBER_OF_CELLS,
                removed.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(!cell.grid_objects.creatures.contains(&guid));

        let object = removed.object.unwrap();
        assert!(!object.object().is_in_world());
        assert!(!object.object().is_in_grid());
        assert!(!object.has_current_map());
        assert_eq!(object.current_cell(), None);
    }

    #[test]
    fn remove_from_map_like_cpp_can_delete_object_and_reports_missing_guid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let removed = map.remove_from_map_like_cpp(guid, true).unwrap();
        assert!(removed.delete_from_world);
        assert!(removed.object.is_none());
        assert_eq!(map.map_object_count(), 0);

        assert_eq!(
            map.remove_from_map_like_cpp(guid, false),
            Err(RemoveFromMapError::ObjectNotFound { guid })
        );
    }

    #[test]
    fn relocate_map_object_like_cpp_same_cell_only_updates_position() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(2.0, 3.0, 4.0))
            .unwrap();

        assert!(outcome.relocated);
        assert!(!outcome.moved_between_cells);
        assert_eq!(outcome.old_cell, added.cell);
        assert_eq!(outcome.new_cell, added.cell);
        assert_eq!(
            map.get_creature(guid).unwrap().position(),
            Position::xyz(2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn relocate_map_object_like_cpp_moves_between_cells_in_same_grid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        let new_position = Position::xyz(90.0, 20.0, 5.0);

        let outcome = map
            .relocate_map_object_like_cpp(guid, new_position)
            .unwrap();

        assert!(outcome.relocated);
        assert!(outcome.moved_between_cells);
        assert_eq!(outcome.old_grid, outcome.new_grid);
        assert_eq!(map.get_creature(guid).unwrap().position(), new_position);
        assert_eq!(
            map.get_creature(guid).unwrap().current_cell(),
            Some((
                outcome.new_cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.new_cell.y_coord % MAX_NUMBER_OF_CELLS
            ))
        );

        let old_grid = map.get_ngrid(added.grid).unwrap();
        let old_cell = old_grid
            .get_grid_type(
                added.cell.x_coord % MAX_NUMBER_OF_CELLS,
                added.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(!old_cell.grid_objects.creatures.contains(&guid));

        let new_cell = old_grid
            .get_grid_type(
                outcome.new_cell.x_coord % MAX_NUMBER_OF_CELLS,
                outcome.new_cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(new_cell.grid_objects.creatures.contains(&guid));
    }

    #[test]
    fn relocate_map_object_like_cpp_blocks_normal_object_to_unloaded_grid() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let guid = creature.guid();
        let added = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(700.0, 20.0, 5.0))
            .unwrap();

        assert!(!outcome.relocated);
        assert!(outcome.blocked_by_unloaded_grid);
        assert_eq!(
            map.get_creature(guid).unwrap().position(),
            Position::xyz(1.0, 2.0, 3.0)
        );
        let old_grid = map.get_ngrid(added.grid).unwrap();
        let old_cell = old_grid
            .get_grid_type(
                added.cell.x_coord % MAX_NUMBER_OF_CELLS,
                added.cell.y_coord % MAX_NUMBER_OF_CELLS,
            )
            .unwrap();
        assert!(old_cell.grid_objects.creatures.contains(&guid));
    }

    #[test]
    fn relocate_map_object_like_cpp_active_object_loads_new_grid_and_moves() {
        let mut map = test_map();
        let mut object = WorldObject::new(true, TypeId::DynamicObject, TypeMask::DYNAMIC_OBJECT);
        object.object_mut().create(guid(HighGuid::DynamicObject, 3));
        object.set_map(571, 7).unwrap();
        object.relocate(Position::xyz(20.0, 20.0, 3.0));
        object.set_active(true);
        let guid = object.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::DynamicObject, object)
            .unwrap();

        let outcome = map
            .relocate_map_object_like_cpp(guid, Position::xyz(700.0, 20.0, 5.0))
            .unwrap();

        assert!(outcome.relocated);
        assert!(outcome.moved_between_cells);
        assert_ne!(outcome.old_grid, outcome.new_grid);
        assert!(outcome.loaded_grid);
        assert!(map.is_grid_loaded(outcome.new_grid));
        assert_eq!(
            map.get_dynamic_object(guid).unwrap().position(),
            Position::xyz(700.0, 20.0, 5.0)
        );
    }

    #[test]
    fn nearby_cell_guids_like_cpp_visits_existing_cells_without_loading_grids() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object(HighGuid::GameObject, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();

        let nearby = map.nearby_cell_guids_like_cpp(0.0, 0.0, 70.0);

        assert_eq!(nearby.visited_cells, 16);
        assert_eq!(nearby.len(), 2);
        assert!(nearby.grid.creatures.contains(&creature_guid));
        assert!(nearby.grid.gameobjects.contains(&gameobject_guid));
        assert_eq!(map.terrain().loads.len(), 1);

        let far = map.nearby_cell_guids_like_cpp(700.0, 700.0, 0.0);
        assert_eq!(far.visited_cells, 1);
        assert!(far.is_empty());
        assert_eq!(map.terrain().loads.len(), 1);
    }

    #[test]
    fn nearby_cell_guids_like_cpp_rejects_invalid_center_without_visits() {
        let map = test_map();
        let nearby = map.nearby_cell_guids_like_cpp(f32::NAN, 0.0, 100.0);

        assert_eq!(nearby.visited_cells, 0);
        assert!(nearby.is_empty());
    }

    #[test]
    fn visit_nearby_cells_of_like_cpp_marks_cells_once_and_collects_objects() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let viewpoint = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let viewpoint_guid = viewpoint.guid();
        let creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let creature_guid = creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, viewpoint)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();

        let plan = map.visit_nearby_cells_of_like_cpp([
            NearbyCellVisitCenter {
                guid: player_guid,
                activation_radius: 0.0,
            },
            NearbyCellVisitCenter {
                guid: viewpoint_guid,
                activation_radius: 0.0,
            },
        ]);

        assert_eq!(plan.marked_cells.len(), 1);
        assert_eq!(plan.nearby.visited_cells, 1);
        assert!(plan.nearby.world.players.contains(&player_guid));
        assert!(plan.nearby.grid.creatures.contains(&viewpoint_guid));
        assert!(plan.nearby.grid.creatures.contains(&creature_guid));
    }

    #[test]
    fn visit_nearby_cells_of_like_cpp_skips_missing_and_invalid_centers() {
        let mut map = test_map();
        let mut invalid_center = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let invalid_guid = invalid_center.guid();
        invalid_center.relocate(Position::xyz(f32::NAN, 0.0, 0.0));
        map.insert_map_object(AccessorObjectKind::Player, invalid_center)
            .unwrap();
        let missing = guid(HighGuid::Player, 9);

        let plan = map.visit_nearby_cells_of_like_cpp([
            NearbyCellVisitCenter {
                guid: invalid_guid,
                activation_radius: 100.0,
            },
            NearbyCellVisitCenter {
                guid: missing,
                activation_radius: 100.0,
            },
        ]);

        assert!(plan.marked_cells.is_empty());
        assert!(plan.nearby.is_empty());
        assert_eq!(plan.skipped_invalid_position_centers, vec![invalid_guid]);
        assert_eq!(plan.skipped_missing_centers, vec![missing]);
    }

    #[test]
    fn player_relocation_visibility_plan_matches_cpp_visible_and_out_of_range_shape() {
        let player = guid(HighGuid::Player, 1);
        let other_player = guid(HighGuid::Player, 2);
        let old_player = guid(HighGuid::Player, 3);
        let creature = guid(HighGuid::Creature, 4);
        let old_creature = guid(HighGuid::Creature, 5);
        let gameobject = guid(HighGuid::GameObject, 6);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player);
        nearby.world.players.insert(other_player);
        nearby.grid.creatures.insert(creature);
        nearby.grid.gameobjects.insert(gameobject);

        let plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
            player,
            [other_player, old_player, old_creature],
            &nearby,
            true,
        );

        assert!(plan.visible_guids.contains(&player));
        assert!(plan.visible_guids.contains(&other_player));
        assert!(plan.visible_guids.contains(&creature));
        assert!(plan.visible_guids.contains(&gameobject));
        assert_eq!(
            plan.out_of_range_guids,
            HashSet::from([old_player, old_creature])
        );
        assert_eq!(
            plan.reciprocal_player_updates,
            HashSet::from([other_player, old_player])
        );
        assert_eq!(plan.ai_relocation_checks, vec![(creature, player)]);
    }

    #[test]
    fn player_relocation_visibility_plan_skips_ai_when_not_relocated_for_ai() {
        let player = guid(HighGuid::Player, 1);
        let creature = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature);

        let plan = PlayerRelocationVisibilityPlan::from_nearby_like_cpp(
            player,
            [creature],
            &nearby,
            false,
        );

        assert!(plan.out_of_range_guids.is_empty());
        assert!(plan.ai_relocation_checks.is_empty());
    }

    #[test]
    fn creature_relocation_visibility_plan_matches_cpp_player_and_creature_visits() {
        let source = guid(HighGuid::Creature, 1);
        let player_visible = guid(HighGuid::Player, 2);
        let player_needs_notify = guid(HighGuid::Player, 3);
        let creature_normal = guid(HighGuid::Creature, 4);
        let creature_needs_notify = guid(HighGuid::Creature, 5);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player_visible);
        nearby.world.players.insert(player_needs_notify);
        nearby.grid.creatures.insert(source);
        nearby.grid.creatures.insert(creature_normal);
        nearby.grid.creatures.insert(creature_needs_notify);

        let plan = CreatureRelocationVisibilityPlan::from_nearby_like_cpp(
            source,
            true,
            &nearby,
            [player_needs_notify],
            [creature_needs_notify],
        );

        assert_eq!(
            plan.player_visibility_updates,
            HashSet::from([player_visible])
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, player_visible))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, player_needs_notify))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, creature_normal))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(creature_normal, source))
        );
        assert!(
            plan.ai_relocation_checks
                .contains(&(source, creature_needs_notify))
        );
        assert!(
            !plan
                .ai_relocation_checks
                .contains(&(creature_needs_notify, source))
        );
    }

    #[test]
    fn creature_relocation_visibility_plan_skips_creature_visits_when_source_dead() {
        let source = guid(HighGuid::Creature, 1);
        let player = guid(HighGuid::Player, 2);
        let creature = guid(HighGuid::Creature, 3);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player);
        nearby.grid.creatures.insert(creature);

        let plan =
            CreatureRelocationVisibilityPlan::from_nearby_like_cpp(source, false, &nearby, [], []);

        assert_eq!(plan.player_visibility_updates, HashSet::from([player]));
        assert_eq!(plan.ai_relocation_checks, vec![(source, player)]);
    }

    #[test]
    fn delayed_unit_relocation_plan_selects_only_units_needing_notify_like_cpp() {
        let creature_notify = guid(HighGuid::Creature, 1);
        let creature_normal = guid(HighGuid::Creature, 2);
        let world_creature_notify = guid(HighGuid::Creature, 3);
        let player_notify = guid(HighGuid::Player, 4);
        let player_normal = guid(HighGuid::Player, 5);
        let player_invalid_viewpoint = guid(HighGuid::Player, 6);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature_notify);
        nearby.grid.creatures.insert(creature_normal);
        nearby.world.creatures.insert(world_creature_notify);
        nearby.world.players.insert(player_notify);
        nearby.world.players.insert(player_normal);
        nearby.world.players.insert(player_invalid_viewpoint);

        let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(
            &nearby,
            [creature_notify, world_creature_notify],
            [player_notify, player_invalid_viewpoint],
            [player_invalid_viewpoint],
        );

        assert_eq!(
            plan.creature_relocations,
            vec![creature_notify, world_creature_notify]
        );
        assert_eq!(plan.player_relocations, vec![player_notify]);
        assert_eq!(
            plan.skipped_invalid_viewpoints,
            vec![player_invalid_viewpoint]
        );
    }

    #[test]
    fn delayed_unit_relocation_plan_deduplicates_creatures_from_world_and_grid_sets() {
        let creature = guid(HighGuid::Creature, 1);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(creature);
        nearby.world.creatures.insert(creature);

        let plan = DelayedUnitRelocationPlan::from_nearby_like_cpp(&nearby, [creature], [], []);

        assert_eq!(plan.creature_relocations, vec![creature]);
        assert!(plan.player_relocations.is_empty());
    }

    #[test]
    fn delayed_unit_relocation_for_cells_like_cpp_reads_notify_flags_from_map_store() {
        let mut map = test_map();
        let creature_notify = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_notify_guid = creature_notify.guid();
        let creature_normal = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let player_notify = world_object_with_counter(HighGuid::Player, 3, 571, 7, false);
        let player_notify_guid = player_notify.guid();
        let player_invalid = world_object_with_counter(HighGuid::Player, 4, 571, 7, false);
        let player_invalid_guid = player_invalid.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature_notify)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature_normal)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_notify)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_invalid)
            .unwrap();
        for guid in [
            creature_notify_guid,
            player_notify_guid,
            player_invalid_guid,
        ] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], [player_invalid_guid]);

        assert_eq!(plan.cell_plans.len(), 1);
        assert_eq!(plan.cell_plans[0].cell_coord, cell);
        assert_eq!(
            plan.cell_plans[0].plan.creature_relocations,
            vec![creature_notify_guid]
        );
        assert_eq!(
            plan.cell_plans[0].plan.player_relocations,
            vec![player_notify_guid]
        );
        assert_eq!(
            plan.cell_plans[0].plan.skipped_invalid_viewpoints,
            vec![player_invalid_guid]
        );
    }

    #[test]
    fn process_relocation_notifies_like_cpp_selects_delayed_before_resetting_flags() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let player = world_object_with_counter(HighGuid::Player, 2, 571, 7, false);
        let player_guid = player.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap()
            .cell;
        let active_cell = Cell::from_cell_coord(cell);
        let active_grid = GridCoord::new(active_cell.grid_x(), active_cell.grid_y());
        map.get_ngrid_mut(active_grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap();
        for guid in [creature_guid, player_guid] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let outcome = map.process_relocation_notifies_like_cpp(
            [cell],
            1000,
            1000,
            std::iter::empty::<ObjectGuid>(),
        );

        assert_eq!(outcome.process_plan.delayed_relocation_cells, vec![cell]);
        assert_eq!(outcome.process_plan.reset_notify_cells, vec![cell]);
        assert_eq!(outcome.process_plan.reset_timer_grids, vec![active_grid]);
        assert_eq!(outcome.delayed_plan.cell_plans.len(), 1);
        assert_eq!(
            outcome.delayed_plan.cell_plans[0].plan.creature_relocations,
            vec![creature_guid]
        );
        assert_eq!(
            outcome.delayed_plan.cell_plans[0].plan.player_relocations,
            vec![player_guid]
        );
        assert_eq!(outcome.reset_outcome.reset_player_guids, vec![player_guid]);
        assert_eq!(
            outcome.reset_outcome.reset_creature_guids,
            vec![creature_guid]
        );
        assert!(
            !map.map_object(creature_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            !map.map_object(player_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
    }

    #[test]
    fn delayed_unit_relocation_visibility_plans_use_cpp_max_visibility_visits() {
        let mut map = test_map();
        let source_creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let source_creature_guid = source_creature.guid();
        let other_creature = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let other_creature_guid = other_creature.guid();
        let notified_creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let notified_creature_guid = notified_creature.guid();
        let player_notify = world_object_with_counter(HighGuid::Player, 4, 571, 7, false);
        let player_notify_guid = player_notify.guid();
        let player_normal = world_object_with_counter(HighGuid::Player, 5, 571, 7, false);
        let player_normal_guid = player_normal.guid();
        let old_player = guid(HighGuid::Player, 6);
        let old_creature = guid(HighGuid::Creature, 7);

        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Creature, source_creature)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, other_creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, notified_creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_notify)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Player, player_normal)
            .unwrap();
        for guid in [
            source_creature_guid,
            notified_creature_guid,
            player_notify_guid,
        ] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let delayed_plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], []);
        let plans = map.delayed_unit_relocation_visibility_plans_like_cpp(
            &delayed_plan,
            [DelayedPlayerRelocationContext {
                player_guid: player_notify_guid,
                viewpoint_guid: player_notify_guid,
                previous_client_guids: vec![old_player, old_creature],
                relocated_for_ai: true,
            }],
            [DelayedCreatureRelocationContext {
                creature_guid: source_creature_guid,
                source_creature_alive: true,
            }],
        );

        assert_eq!(plans.creature_plans.len(), 2);
        let source_plan = plans
            .creature_plans
            .iter()
            .find(|plan| plan.creature_guid == source_creature_guid)
            .unwrap();
        assert_eq!(source_plan.cell_coord, cell);
        assert!(
            source_plan
                .visibility_plan
                .player_visibility_updates
                .contains(&player_normal_guid)
        );
        assert!(
            !source_plan
                .visibility_plan
                .player_visibility_updates
                .contains(&player_notify_guid)
        );
        assert!(
            source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(source_creature_guid, other_creature_guid))
        );
        assert!(
            source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(other_creature_guid, source_creature_guid))
        );
        assert!(
            !source_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(notified_creature_guid, source_creature_guid))
        );

        assert_eq!(plans.player_plans.len(), 1);
        let player_plan = &plans.player_plans[0];
        assert_eq!(player_plan.player_guid, player_notify_guid);
        assert_eq!(player_plan.viewpoint_guid, player_notify_guid);
        assert!(
            player_plan
                .visibility_plan
                .out_of_range_guids
                .contains(&old_player)
        );
        assert!(
            player_plan
                .visibility_plan
                .out_of_range_guids
                .contains(&old_creature)
        );
        assert!(
            player_plan
                .visibility_plan
                .ai_relocation_checks
                .contains(&(source_creature_guid, player_notify_guid))
        );
    }

    #[test]
    fn delayed_unit_relocation_visibility_plans_report_missing_player_contexts_like_cpp_gap() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap()
            .cell;
        map.map_objects
            .get_mut(&player_guid)
            .unwrap()
            .object_mut()
            .object_mut()
            .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);

        let delayed_plan = map.delayed_unit_relocation_for_cells_like_cpp([cell], []);
        let plans = map.delayed_unit_relocation_visibility_plans_like_cpp(
            &delayed_plan,
            std::iter::empty::<DelayedPlayerRelocationContext>(),
            std::iter::empty::<DelayedCreatureRelocationContext>(),
        );

        assert!(plans.player_plans.is_empty());
        assert_eq!(plans.missing_player_contexts, vec![player_guid]);
    }

    #[test]
    fn ai_relocation_plan_for_player_checks_nearby_creatures_against_source_unit() {
        let player = guid(HighGuid::Player, 1);
        let world_creature = guid(HighGuid::Creature, 2);
        let grid_creature = guid(HighGuid::Creature, 3);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(world_creature);
        nearby.grid.creatures.insert(grid_creature);

        let plan = AIRelocationPlan::from_nearby_like_cpp(player, false, &nearby);

        assert_eq!(
            plan.creature_unit_checks,
            vec![(world_creature, player), (grid_creature, player)]
        );
    }

    #[test]
    fn ai_relocation_plan_for_creature_checks_both_cpp_directions() {
        let source = guid(HighGuid::Creature, 1);
        let other = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.grid.creatures.insert(source);
        nearby.grid.creatures.insert(other);

        let plan = AIRelocationPlan::from_nearby_like_cpp(source, true, &nearby);

        assert_eq!(
            plan.creature_unit_checks,
            vec![(other, source), (source, other)]
        );
    }

    #[test]
    fn ai_relocation_plan_deduplicates_world_grid_creatures_and_skips_self_worker_noop() {
        let source = guid(HighGuid::Creature, 1);
        let other = guid(HighGuid::Creature, 2);
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(source);
        nearby.grid.creatures.insert(source);
        nearby.world.creatures.insert(other);
        nearby.grid.creatures.insert(other);

        let plan = AIRelocationPlan::from_nearby_like_cpp(source, false, &nearby);

        assert_eq!(plan.creature_unit_checks, vec![(other, source)]);
    }

    #[test]
    fn object_update_plan_for_nearby_like_cpp_selects_in_world_updateable_objects_only() {
        let mut map = test_map();
        let player = world_object(HighGuid::Player, 571, 7, true);
        let player_guid = player.guid();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();
        let gameobject = world_object(HighGuid::GameObject, 571, 7, true);
        let gameobject_guid = gameobject.guid();
        let dynamic_not_in_world = world_object(HighGuid::DynamicObject, 571, 7, false);
        let dynamic_guid = dynamic_not_in_world.guid();
        let missing_conversation = guid(HighGuid::Conversation, 9);
        map.insert_map_object(AccessorObjectKind::Player, player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::DynamicObject, dynamic_not_in_world)
            .unwrap();

        let mut nearby = NearbyCellGuids::default();
        nearby.world.players.insert(player_guid);
        nearby.grid.creatures.insert(creature_guid);
        nearby.grid.gameobjects.insert(gameobject_guid);
        nearby.grid.dynamic_objects.insert(dynamic_guid);
        nearby.grid.conversations.insert(missing_conversation);

        let plan = map.object_update_plan_for_nearby_like_cpp(&nearby, 42);

        assert_eq!(plan.diff_ms, 42);
        assert_eq!(plan.update_guids, vec![creature_guid, gameobject_guid]);
    }

    #[test]
    fn object_update_plan_for_nearby_like_cpp_deduplicates_world_and_grid_objects() {
        let mut map = test_map();
        let creature = world_object(HighGuid::Creature, 571, 7, true);
        let creature_guid = creature.guid();
        map.insert_map_object(AccessorObjectKind::Creature, creature)
            .unwrap();
        let mut nearby = NearbyCellGuids::default();
        nearby.world.creatures.insert(creature_guid);
        nearby.grid.creatures.insert(creature_guid);

        let plan = map.object_update_plan_for_nearby_like_cpp(&nearby, 1);

        assert_eq!(plan.update_guids, vec![creature_guid]);
    }

    #[test]
    fn map_update_visit_plan_like_cpp_filters_sources_by_cpp_in_world_guards() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, true);
        let player_guid = player.guid();
        let offline_player = world_object_with_counter(HighGuid::Player, 2, 571, 7, false);
        let offline_player_guid = offline_player.guid();
        let viewpoint = world_object_with_counter(HighGuid::Creature, 3, 571, 7, true);
        let viewpoint_guid = viewpoint.guid();
        let far_combat = world_object_with_counter(HighGuid::Creature, 4, 571, 7, true);
        let far_combat_guid = far_combat.guid();
        let offline_aura = world_object_with_counter(HighGuid::Creature, 5, 571, 7, false);
        let offline_aura_guid = offline_aura.guid();
        let active_non_player = world_object_with_counter(HighGuid::DynamicObject, 6, 571, 7, true);
        let active_non_player_guid = active_non_player.guid();
        let transport = world_object_with_counter(HighGuid::Transport, 7, 571, 7, false);
        let transport_guid = transport.guid();

        map.insert_map_object(AccessorObjectKind::Player, player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Player, offline_player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, viewpoint)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, far_combat)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, offline_aura)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::DynamicObject, active_non_player)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Transport, transport)
            .unwrap();

        let plan = map.map_update_visit_plan_like_cpp(
            [
                MapUpdatePlayerSources {
                    player_guid,
                    viewpoint_guid: Some(viewpoint_guid),
                    far_combat_unit_guids: vec![far_combat_guid],
                    far_aura_caster_guids: vec![offline_aura_guid],
                    far_summon_guids: vec![],
                },
                MapUpdatePlayerSources {
                    player_guid: offline_player_guid,
                    viewpoint_guid: Some(far_combat_guid),
                    far_combat_unit_guids: vec![viewpoint_guid],
                    far_aura_caster_guids: vec![],
                    far_summon_guids: vec![],
                },
            ],
            [active_non_player_guid, offline_aura_guid],
            [transport_guid],
            50,
        );

        assert_eq!(plan.diff_ms, 50);
        assert_eq!(plan.session_update_players, vec![player_guid]);
        assert_eq!(plan.player_update_guids, vec![player_guid]);
        assert_eq!(plan.transport_update_guids, vec![transport_guid]);
        assert_eq!(
            plan.nearby_visit_centers
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([
                player_guid,
                viewpoint_guid,
                far_combat_guid,
                active_non_player_guid
            ])
        );
        assert!(plan.process_relocation_notifies);
    }

    #[test]
    fn map_update_visit_plan_like_cpp_processes_relocation_notifies_only_for_players_or_active_non_players()
     {
        let mut map = test_map();
        let transport = world_object_with_counter(HighGuid::Transport, 7, 571, 7, false);
        let transport_guid = transport.guid();
        map.insert_map_object(AccessorObjectKind::Transport, transport)
            .unwrap();

        let plan = map.map_update_visit_plan_like_cpp(
            std::iter::empty::<MapUpdatePlayerSources>(),
            std::iter::empty::<ObjectGuid>(),
            [transport_guid],
            1,
        );

        assert_eq!(plan.transport_update_guids, vec![transport_guid]);
        assert!(!plan.process_relocation_notifies);
    }

    #[test]
    fn process_relocation_notifies_plan_like_cpp_waits_for_active_grid_timer() {
        let mut map = test_map();
        let grid = GridCoord::new(2, 3);
        map.ensure_grid_created(grid);
        map.get_ngrid_mut(grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        let marked = CellCoord::new(2 * MAX_NUMBER_OF_CELLS, 3 * MAX_NUMBER_OF_CELLS);

        let plan = map.process_relocation_notifies_plan_like_cpp([marked], 999, 1000);

        assert!(plan.delayed_relocation_cells.is_empty());
        assert!(plan.reset_notify_cells.is_empty());
        assert!(plan.reset_timer_grids.is_empty());
    }

    #[test]
    fn process_relocation_notifies_plan_like_cpp_visits_marked_cells_and_resets_timer() {
        let mut map = test_map();
        let active_grid = GridCoord::new(2, 3);
        let idle_grid = GridCoord::new(4, 5);
        map.ensure_grid_created(active_grid);
        map.ensure_grid_created(idle_grid);
        map.get_ngrid_mut(active_grid)
            .unwrap()
            .set_state(GridStateKind::Active);
        map.get_ngrid_mut(idle_grid)
            .unwrap()
            .set_state(GridStateKind::Idle);
        let marked_a = CellCoord::new(2 * MAX_NUMBER_OF_CELLS, 3 * MAX_NUMBER_OF_CELLS);
        let marked_b = CellCoord::new(2 * MAX_NUMBER_OF_CELLS + 1, 3 * MAX_NUMBER_OF_CELLS);
        let marked_idle = CellCoord::new(4 * MAX_NUMBER_OF_CELLS, 5 * MAX_NUMBER_OF_CELLS);

        let plan = map.process_relocation_notifies_plan_like_cpp(
            [marked_b, marked_idle, marked_a],
            1000,
            1000,
        );

        assert_eq!(plan.diff_ms, 1000);
        assert_eq!(plan.delayed_relocation_cells, vec![marked_a, marked_b]);
        assert_eq!(plan.reset_notify_cells, vec![marked_a, marked_b]);
        assert_eq!(plan.reset_timer_grids, vec![active_grid]);
        assert_eq!(
            map.get_ngrid(active_grid)
                .unwrap()
                .info()
                .relocation_timer()
                .expire_time_ms(),
            1000
        );
    }

    #[test]
    fn reset_notify_flags_for_cells_like_cpp_resets_only_players_and_creatures() {
        let mut map = test_map();
        let player = world_object_with_counter(HighGuid::Player, 1, 571, 7, false);
        let player_guid = player.guid();
        let creature = world_object_with_counter(HighGuid::Creature, 2, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 3, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        let player_cell = map
            .add_to_map_like_cpp(AccessorObjectKind::Player, player)
            .unwrap()
            .cell;
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        for guid in [player_guid, creature_guid, gameobject_guid] {
            map.map_objects
                .get_mut(&guid)
                .unwrap()
                .object_mut()
                .object_mut()
                .add_to_notify(ObjectNotifyFlags::VISIBILITY_CHANGED);
        }

        let outcome = map.reset_notify_flags_for_cells_like_cpp([player_cell]);

        assert_eq!(outcome.reset_player_guids, vec![player_guid]);
        assert_eq!(outcome.reset_creature_guids, vec![creature_guid]);
        assert!(outcome.missing_guids.is_empty());
        assert!(
            !map.map_object(player_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            !map.map_object(creature_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
        assert!(
            map.map_object(gameobject_guid)
                .unwrap()
                .object()
                .is_need_notify(ObjectNotifyFlags::VISIBILITY_CHANGED)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_relocates_active_entries_and_resets_inactive() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 2, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(5.0, 5.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: gameobject_guid,
                kind: AccessorObjectKind::GameObject,
                move_state: MapObjectCellMoveState::Inactive,
                new_position: Position::xyz(6.0, 6.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
        ]);

        assert_eq!(plan.relocated, vec![creature_guid]);
        assert_eq!(plan.reset_inactive_or_none, vec![gameobject_guid]);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().position(),
            Position::xyz(5.0, 5.0, 3.0)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_uses_respawn_or_removal_fallbacks() {
        let mut map = test_map();
        let creature = world_object_with_counter(HighGuid::Creature, 1, 571, 7, false);
        let creature_guid = creature.guid();
        let gameobject = world_object_with_counter(HighGuid::GameObject, 2, 571, 7, false);
        let gameobject_guid = gameobject.guid();
        let pet = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let pet_guid = pet.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, creature)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::GameObject, gameobject)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::Creature, pet)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: Some(Position::xyz(2.0, 2.0, 3.0)),
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: gameobject_guid,
                kind: AccessorObjectKind::GameObject,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: pet_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: true,
            },
        ]);

        assert_eq!(plan.respawn_relocated, vec![creature_guid]);
        assert_eq!(plan.remove_from_world, vec![gameobject_guid]);
        assert_eq!(plan.pet_removed, vec![pet_guid]);
        assert_eq!(
            map.get_creature(creature_guid).unwrap().position(),
            Position::xyz(2.0, 2.0, 3.0)
        );
    }

    #[test]
    fn process_map_object_move_list_like_cpp_blocks_dynamic_and_skips_not_in_world() {
        let mut map = test_map();
        let dynamic = world_object_with_counter(HighGuid::DynamicObject, 1, 571, 7, false);
        let dynamic_guid = dynamic.guid();
        let area_trigger = world_object_with_counter(HighGuid::AreaTrigger, 2, 571, 7, false);
        let area_trigger_guid = area_trigger.guid();
        let offline_creature = world_object_with_counter(HighGuid::Creature, 3, 571, 7, false);
        let offline_creature_guid = offline_creature.guid();
        map.add_to_map_like_cpp(AccessorObjectKind::DynamicObject, dynamic)
            .unwrap();
        map.add_to_map_like_cpp(AccessorObjectKind::AreaTrigger, area_trigger)
            .unwrap();
        map.insert_map_object(AccessorObjectKind::Creature, offline_creature)
            .unwrap();

        let plan = map.process_map_object_move_list_like_cpp([
            MapObjectMoveListEntry {
                guid: dynamic_guid,
                kind: AccessorObjectKind::DynamicObject,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: area_trigger_guid,
                kind: AccessorObjectKind::AreaTrigger,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(700.0, 20.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
            MapObjectMoveListEntry {
                guid: offline_creature_guid,
                kind: AccessorObjectKind::Creature,
                move_state: MapObjectCellMoveState::Active,
                new_position: Position::xyz(2.0, 2.0, 3.0),
                respawn_position: None,
                is_pet: false,
            },
        ]);

        assert_eq!(
            plan.blocked_unloaded_grid,
            vec![dynamic_guid, area_trigger_guid]
        );
        assert_eq!(plan.skipped_not_in_world, vec![offline_creature_guid]);
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
            spawn_group: crate::spawn::SpawnGroupTemplateData::default_group(),
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
