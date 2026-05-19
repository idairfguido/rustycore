//! Spawn metadata and cell indexes used by grid loading.
//!
//! C++ references:
//! - `game/Maps/SpawnData.h`
//! - `game/Globals/ObjectMgr.cpp` (`AddSpawnDataToGrid`)
//! - `game/Globals/AreaTriggerDataStore.cpp` (`LoadAreaTriggerSpawns`)

use std::collections::{BTreeMap, BTreeSet};

use crate::coords::compute_cell_coord;
use wow_core::ObjectGuid;

pub type SpawnId = u64;
pub type Difficulty = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SpawnObjectType {
    Creature = 0,
    GameObject = 1,
    AreaTrigger = 2,
}

impl SpawnObjectType {
    pub const fn type_has_data(self) -> bool {
        matches!(self, Self::Creature | Self::GameObject | Self::AreaTrigger)
    }

    pub const fn mask(self) -> u32 {
        1 << self as u8
    }

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Creature),
            1 => Some(Self::GameObject),
            2 => Some(Self::AreaTrigger),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkedRespawnTypeLikeCpp {
    CreatureToCreature = 0,
    CreatureToGameObject = 1,
    GameObjectToGameObject = 2,
    GameObjectToCreature = 3,
}

impl LinkedRespawnTypeLikeCpp {
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::CreatureToCreature),
            1 => Some(Self::CreatureToGameObject),
            2 => Some(Self::GameObjectToGameObject),
            3 => Some(Self::GameObjectToCreature),
            _ => None,
        }
    }

    pub const fn slave_type(self) -> SpawnObjectType {
        match self {
            Self::CreatureToCreature | Self::CreatureToGameObject => SpawnObjectType::Creature,
            Self::GameObjectToGameObject | Self::GameObjectToCreature => {
                SpawnObjectType::GameObject
            }
        }
    }

    pub const fn master_type(self) -> SpawnObjectType {
        match self {
            Self::CreatureToCreature | Self::GameObjectToCreature => SpawnObjectType::Creature,
            Self::CreatureToGameObject | Self::GameObjectToGameObject => {
                SpawnObjectType::GameObject
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkedRespawnRowLikeCpp {
    pub guid: SpawnId,
    pub linked_guid: SpawnId,
    pub link_type: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkedRespawnLoadIssueKindLikeCpp {
    InvalidType,
    MissingSlave,
    MissingMaster,
    NotInstanceableOrMapMismatch,
    DifficultyMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRespawnLoadIssueLikeCpp {
    pub kind: LinkedRespawnLoadIssueKindLikeCpp,
    pub guid: SpawnId,
    pub linked_guid: SpawnId,
    pub link_type: u8,
    pub slave_type: Option<SpawnObjectType>,
    pub master_type: Option<SpawnObjectType>,
    pub slave_map_id: Option<u32>,
    pub master_map_id: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedRespawnLoadReportLikeCpp {
    pub rows: usize,
    pub inserted: usize,
    pub invalid_type: usize,
    pub missing_slave: usize,
    pub missing_master: usize,
    pub not_instanceable_or_map_mismatch: usize,
    pub difficulty_mismatch: usize,
    pub issues: Vec<LinkedRespawnLoadIssueLikeCpp>,
}

impl LinkedRespawnLoadReportLikeCpp {
    pub fn push(&mut self, issue: LinkedRespawnLoadIssueLikeCpp) {
        match issue.kind {
            LinkedRespawnLoadIssueKindLikeCpp::InvalidType => self.invalid_type += 1,
            LinkedRespawnLoadIssueKindLikeCpp::MissingSlave => self.missing_slave += 1,
            LinkedRespawnLoadIssueKindLikeCpp::MissingMaster => self.missing_master += 1,
            LinkedRespawnLoadIssueKindLikeCpp::NotInstanceableOrMapMismatch => {
                self.not_instanceable_or_map_mismatch += 1;
            }
            LinkedRespawnLoadIssueKindLikeCpp::DifficultyMismatch => self.difficulty_mismatch += 1,
        }
        self.issues.push(issue);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedRespawnStoreLikeCpp {
    linked_respawns: BTreeMap<ObjectGuid, ObjectGuid>,
}

impl LinkedRespawnStoreLikeCpp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_like_cpp(
        &mut self,
        guid: ObjectGuid,
        linked_guid: ObjectGuid,
    ) -> Option<ObjectGuid> {
        self.linked_respawns.insert(guid, linked_guid)
    }

    pub fn get_linked_respawn_guid_like_cpp(&self, guid: ObjectGuid) -> ObjectGuid {
        self.linked_respawns
            .get(&guid)
            .copied()
            .unwrap_or(ObjectGuid::EMPTY)
    }

    pub fn len(&self) -> usize {
        self.linked_respawns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.linked_respawns.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpawnGroupFlags(pub u32);

impl SpawnGroupFlags {
    pub const NONE: Self = Self(0x00);
    pub const SYSTEM: Self = Self(0x01);
    pub const COMPATIBILITY_MODE: Self = Self(0x02);
    pub const MANUAL_SPAWN: Self = Self(0x04);
    pub const DYNAMIC_SPAWN_RATE: Self = Self(0x08);
    pub const ESCORTQUESTNPC: Self = Self(0x10);
    pub const DESPAWN_ON_CONDITION_FAILURE: Self = Self(0x20);
    pub const ALL: Self = Self(
        Self::SYSTEM.0
            | Self::COMPATIBILITY_MODE.0
            | Self::MANUAL_SPAWN.0
            | Self::DYNAMIC_SPAWN_RATE.0
            | Self::ESCORTQUESTNPC.0
            | Self::DESPAWN_ON_CONDITION_FAILURE.0,
    );

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }

    pub const fn truncate_to_all(self) -> Self {
        Self(self.0 & Self::ALL.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

impl SpawnPosition {
    pub const fn new(x: f32, y: f32, z: f32, orientation: f32) -> Self {
        Self {
            x,
            y,
            z,
            orientation,
        }
    }
}

pub const SPAWNGROUP_MAP_UNSET: u32 = 0xffff_ffff;

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnGroupTemplateData {
    pub group_id: u32,
    pub name: String,
    pub map_id: u32,
    pub flags: SpawnGroupFlags,
}

impl SpawnGroupTemplateData {
    pub fn default_group() -> Self {
        Self {
            group_id: 0,
            name: "Default Group".to_string(),
            map_id: 0,
            flags: SpawnGroupFlags::SYSTEM,
        }
    }

    pub fn legacy_group() -> Self {
        Self {
            group_id: 1,
            name: "Legacy Group".to_string(),
            map_id: 0,
            flags: SpawnGroupFlags(
                SpawnGroupFlags::SYSTEM.0 | SpawnGroupFlags::COMPATIBILITY_MODE.0,
            ),
        }
    }

    pub const fn is_system(&self) -> bool {
        self.flags.contains(SpawnGroupFlags::SYSTEM)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnGroupActiveChange {
    MissingGroup,
    SystemGroup,
    Toggled,
    ClearedToggle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnGroupRuntimeState {
    toggled_spawn_group_ids: BTreeSet<u32>,
}

impl SpawnGroupRuntimeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_spawn_group_active_like_cpp(
        &mut self,
        group: Option<&SpawnGroupTemplateData>,
        state: bool,
    ) -> SpawnGroupActiveChange {
        let Some(group) = group else {
            return SpawnGroupActiveChange::MissingGroup;
        };
        if group.is_system() {
            return SpawnGroupActiveChange::SystemGroup;
        }

        if state != spawn_group_default_active_like_cpp(group) {
            self.toggled_spawn_group_ids.insert(group.group_id);
            SpawnGroupActiveChange::Toggled
        } else {
            self.toggled_spawn_group_ids.remove(&group.group_id);
            SpawnGroupActiveChange::ClearedToggle
        }
    }

    pub fn is_spawn_group_active_like_cpp(&self, group: Option<&SpawnGroupTemplateData>) -> bool {
        let Some(group) = group else {
            return false;
        };
        if group.is_system() {
            return true;
        }

        self.toggled_spawn_group_ids.contains(&group.group_id)
            != spawn_group_default_active_like_cpp(group)
    }

    pub fn is_toggled(&self, group_id: u32) -> bool {
        self.toggled_spawn_group_ids.contains(&group_id)
    }

    pub fn toggled_spawn_group_ids(&self) -> &BTreeSet<u32> {
        &self.toggled_spawn_group_ids
    }
}

fn spawn_group_default_active_like_cpp(group: &SpawnGroupTemplateData) -> bool {
    !group.flags.contains(SpawnGroupFlags::MANUAL_SPAWN)
}

#[derive(Debug, Clone)]
pub struct SpawnGridLoadStateLikeCpp<'a> {
    spawn_store: &'a SpawnStore,
    spawn_group_state: &'a SpawnGroupRuntimeState,
    respawn_timers: BTreeSet<(SpawnObjectType, SpawnId)>,
    pool_spawned_objects: BTreeSet<(SpawnObjectType, SpawnId)>,
}

impl<'a> SpawnGridLoadStateLikeCpp<'a> {
    pub fn new(spawn_store: &'a SpawnStore, spawn_group_state: &'a SpawnGroupRuntimeState) -> Self {
        Self {
            spawn_store,
            spawn_group_state,
            respawn_timers: BTreeSet::new(),
            pool_spawned_objects: BTreeSet::new(),
        }
    }

    pub fn with_respawn_timers(
        mut self,
        respawn_timers: impl IntoIterator<Item = (SpawnObjectType, SpawnId)>,
    ) -> Self {
        self.respawn_timers.extend(respawn_timers);
        self
    }

    pub fn with_pool_spawned_objects(
        mut self,
        pool_spawned_objects: impl IntoIterator<Item = (SpawnObjectType, SpawnId)>,
    ) -> Self {
        self.pool_spawned_objects.extend(pool_spawned_objects);
        self
    }

    pub fn add_respawn_timer(&mut self, object_type: SpawnObjectType, spawn_id: SpawnId) {
        self.respawn_timers.insert((object_type, spawn_id));
    }

    pub fn add_pool_spawned_object(&mut self, object_type: SpawnObjectType, spawn_id: SpawnId) {
        self.pool_spawned_objects.insert((object_type, spawn_id));
    }

    pub fn should_be_spawned_on_grid_load(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> bool {
        if !object_type.type_has_data() {
            return false;
        }

        // C++ `Map::ShouldBeSpawnedOnGridLoad` checks respawn timers before
        // consulting spawn metadata, spawn group state, or pool state.
        if self.respawn_timers.contains(&(object_type, spawn_id)) {
            return false;
        }

        let Some(spawn_data) = self.spawn_store.spawn_data(object_type, spawn_id) else {
            return false;
        };
        let spawn_group = &spawn_data.spawn_group;
        if !spawn_group.is_system()
            && !self
                .spawn_group_state
                .is_spawn_group_active_like_cpp(Some(spawn_group))
        {
            return false;
        }

        if spawn_data.pool_id != 0 && !self.pool_spawned_objects.contains(&(object_type, spawn_id))
        {
            return false;
        }

        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnData {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    pub map_id: u32,
    pub db_data: bool,
    pub spawn_group: SpawnGroupTemplateData,
    pub id: u32,
    pub spawn_point: SpawnPosition,
    pub phase_use_flags: u8,
    pub phase_id: u32,
    pub phase_group: u32,
    pub terrain_swap_map: i32,
    pub pool_id: u32,
    pub spawn_time_secs: i32,
    pub spawn_difficulties: Vec<Difficulty>,
    pub script_id: u32,
    pub string_id: String,
}

impl SpawnData {
    pub fn cell_id(&self) -> u32 {
        compute_cell_coord(self.spawn_point.x, self.spawn_point.y).get_id()
    }

    pub const fn spawn_group_id(&self) -> u32 {
        self.spawn_group.group_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnGroupMemberRow {
    pub group_id: u32,
    pub spawn_type: u8,
    pub spawn_id: SpawnId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpawnGroupMember {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnGroupApplyIssueKind {
    InvalidType,
    MissingSpawn,
    DuplicateSpawnGroup,
    MissingGroup,
    MapMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnGroupApplyIssue {
    pub kind: SpawnGroupApplyIssueKind,
    pub group_id: u32,
    pub spawn_type: u8,
    pub spawn_id: SpawnId,
    pub existing_group_id: Option<u32>,
    pub group_map_id: Option<u32>,
    pub spawn_map_id: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnGroupApplyReport {
    pub assigned: usize,
    pub invalid_type: usize,
    pub missing_spawn: usize,
    pub duplicate_spawn_group: usize,
    pub missing_group: usize,
    pub map_mismatch: usize,
    pub issues: Vec<SpawnGroupApplyIssue>,
}

impl SpawnGroupApplyReport {
    fn push(&mut self, issue: SpawnGroupApplyIssue) {
        match issue.kind {
            SpawnGroupApplyIssueKind::InvalidType => self.invalid_type += 1,
            SpawnGroupApplyIssueKind::MissingSpawn => self.missing_spawn += 1,
            SpawnGroupApplyIssueKind::DuplicateSpawnGroup => self.duplicate_spawn_group += 1,
            SpawnGroupApplyIssueKind::MissingGroup => self.missing_group += 1,
            SpawnGroupApplyIssueKind::MapMismatch => self.map_mismatch += 1,
        }
        self.issues.push(issue);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellSpawnGuids {
    pub creatures: BTreeSet<SpawnId>,
    pub gameobjects: BTreeSet<SpawnId>,
    pub area_triggers: BTreeSet<SpawnId>,
}

impl CellSpawnGuids {
    pub fn is_empty(&self) -> bool {
        self.creatures.is_empty() && self.gameobjects.is_empty() && self.area_triggers.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpawnMapKey {
    pub map_id: u32,
    pub difficulty: Difficulty,
}

impl SpawnMapKey {
    pub const fn new(map_id: u32, difficulty: Difficulty) -> Self {
        Self { map_id, difficulty }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersonalSpawnMapKey {
    pub map_id: u32,
    pub difficulty: Difficulty,
    pub phase_id: u32,
}

impl PersonalSpawnMapKey {
    pub const fn new(map_id: u32, difficulty: Difficulty, phase_id: u32) -> Self {
        Self {
            map_id,
            difficulty,
            phase_id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpawnStore {
    spawns: BTreeMap<(SpawnObjectType, SpawnId), SpawnData>,
    object_guids: BTreeMap<SpawnMapKey, BTreeMap<u32, CellSpawnGuids>>,
    personal_object_guids: BTreeMap<PersonalSpawnMapKey, BTreeMap<u32, CellSpawnGuids>>,
    spawn_groups_by_map: BTreeMap<u32, BTreeSet<u32>>,
    spawn_group_members: BTreeMap<u32, BTreeSet<SpawnGroupMember>>,
}

impl SpawnStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `ObjectMgr::LoadSpawnGroups`: apply `spawn_group` rows to loaded spawn metadata.
    ///
    /// This is intentionally pure/in-memory. DB loading and map runtime activation stay outside
    /// `wow-map` until the ObjectMgr/world-server wiring slice.
    pub fn apply_spawn_groups_like_cpp(
        &mut self,
        templates: &mut BTreeMap<u32, SpawnGroupTemplateData>,
        rows: impl IntoIterator<Item = SpawnGroupMemberRow>,
    ) -> SpawnGroupApplyReport {
        let mut report = SpawnGroupApplyReport::default();

        for row in rows {
            let Some(object_type) = SpawnObjectType::from_raw(row.spawn_type) else {
                report.push(SpawnGroupApplyIssue {
                    kind: SpawnGroupApplyIssueKind::InvalidType,
                    group_id: row.group_id,
                    spawn_type: row.spawn_type,
                    spawn_id: row.spawn_id,
                    existing_group_id: None,
                    group_map_id: None,
                    spawn_map_id: None,
                });
                continue;
            };

            let key = (object_type, row.spawn_id);
            let Some(spawn) = self.spawns.get_mut(&key) else {
                report.push(SpawnGroupApplyIssue {
                    kind: SpawnGroupApplyIssueKind::MissingSpawn,
                    group_id: row.group_id,
                    spawn_type: row.spawn_type,
                    spawn_id: row.spawn_id,
                    existing_group_id: None,
                    group_map_id: None,
                    spawn_map_id: None,
                });
                continue;
            };

            if spawn.spawn_group.group_id != 0 {
                report.push(SpawnGroupApplyIssue {
                    kind: SpawnGroupApplyIssueKind::DuplicateSpawnGroup,
                    group_id: row.group_id,
                    spawn_type: row.spawn_type,
                    spawn_id: row.spawn_id,
                    existing_group_id: Some(spawn.spawn_group.group_id),
                    group_map_id: None,
                    spawn_map_id: Some(spawn.map_id),
                });
                continue;
            }

            let Some(group_template) = templates.get_mut(&row.group_id) else {
                report.push(SpawnGroupApplyIssue {
                    kind: SpawnGroupApplyIssueKind::MissingGroup,
                    group_id: row.group_id,
                    spawn_type: row.spawn_type,
                    spawn_id: row.spawn_id,
                    existing_group_id: None,
                    group_map_id: None,
                    spawn_map_id: Some(spawn.map_id),
                });
                continue;
            };

            if group_template.map_id == SPAWNGROUP_MAP_UNSET {
                group_template.map_id = spawn.map_id;
                self.spawn_groups_by_map
                    .entry(spawn.map_id)
                    .or_default()
                    .insert(row.group_id);
            } else if group_template.map_id != spawn.map_id && !group_template.is_system() {
                report.push(SpawnGroupApplyIssue {
                    kind: SpawnGroupApplyIssueKind::MapMismatch,
                    group_id: row.group_id,
                    spawn_type: row.spawn_type,
                    spawn_id: row.spawn_id,
                    existing_group_id: None,
                    group_map_id: Some(group_template.map_id),
                    spawn_map_id: Some(spawn.map_id),
                });
                continue;
            }

            spawn.spawn_group = group_template.clone();
            if !group_template.is_system() {
                self.spawn_group_members
                    .entry(row.group_id)
                    .or_default()
                    .insert(SpawnGroupMember {
                        object_type,
                        spawn_id: row.spawn_id,
                    });
            }
            report.assigned += 1;
        }

        report
    }

    /// C++ `_spawnGroupsByMap`: groups whose template map was first resolved by a spawn row.
    pub fn spawn_group_ids_by_map(&self, map_id: u32) -> Option<&BTreeSet<u32>> {
        self.spawn_groups_by_map.get(&map_id)
    }

    /// C++ `_spawnGroupMapStore`: non-system spawn members indexed by group id.
    pub fn spawn_group_members(&self, group_id: u32) -> Option<&BTreeSet<SpawnGroupMember>> {
        self.spawn_group_members.get(&group_id)
    }

    /// C++ `Map::GetSpawnGroupData` map filter shape for future runtime consumers.
    pub fn spawn_group_template_for_map<'a>(
        templates: &'a BTreeMap<u32, SpawnGroupTemplateData>,
        group_id: u32,
        map_id: u32,
    ) -> Option<&'a SpawnGroupTemplateData> {
        let data = templates.get(&group_id)?;
        if data.is_system() || data.map_id == map_id {
            Some(data)
        } else {
            None
        }
    }

    /// Inserts canonical spawn metadata without touching grid indexes.
    ///
    /// C++ `ObjectMgr::LoadCreatures` / `LoadGameObjects` always populate
    /// `_creatureDataStore` / `_gameObjectDataStore` before the `gameEvent`
    /// branch. The event branch only gates `AddCreatureToGrid` /
    /// `AddGameobjectToGrid`; it does not discard metadata.
    pub fn insert_spawn_metadata_like_cpp(&mut self, data: &SpawnData) {
        self.spawns
            .insert((data.object_type, data.spawn_id), data.clone());
    }

    /// C++ `ObjectMgr::AddSpawnDataToGrid` for creature/gameobject spawns.
    pub fn add_object_spawn<F>(&mut self, data: &SpawnData, is_personal_phase: F)
    where
        F: Fn(u32) -> bool,
    {
        match data.object_type {
            SpawnObjectType::Creature | SpawnObjectType::GameObject => {}
            SpawnObjectType::AreaTrigger => {
                self.add_area_trigger_spawn(data);
                return;
            }
        }

        self.insert_spawn_metadata_like_cpp(data);

        let cell_id = data.cell_id();
        if is_personal_phase(data.phase_id) {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = PersonalSpawnMapKey::new(data.map_id, difficulty, data.phase_id);
                let cell = self
                    .personal_object_guids
                    .entry(key)
                    .or_default()
                    .entry(cell_id)
                    .or_default();
                insert_spawn(cell, data.object_type, data.spawn_id);
            }
        } else {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = SpawnMapKey::new(data.map_id, difficulty);
                let cell = self
                    .object_guids
                    .entry(key)
                    .or_default()
                    .entry(cell_id)
                    .or_default();
                insert_spawn(cell, data.object_type, data.spawn_id);
            }
        }
    }

    /// C++ `AreaTriggerDataStore::LoadAreaTriggerSpawns` indexes static area
    /// triggers by map/difficulty/cell only; it does not use ObjectMgr's
    /// personal-phase store.
    pub fn add_area_trigger_spawn(&mut self, data: &SpawnData) {
        debug_assert_eq!(data.object_type, SpawnObjectType::AreaTrigger);
        self.insert_spawn_metadata_like_cpp(data);
        let cell_id = data.cell_id();
        for difficulty in data.spawn_difficulties.iter().copied() {
            let key = SpawnMapKey::new(data.map_id, difficulty);
            self.object_guids
                .entry(key)
                .or_default()
                .entry(cell_id)
                .or_default()
                .area_triggers
                .insert(data.spawn_id);
        }
    }

    pub fn remove_object_spawn<F>(&mut self, data: &SpawnData, is_personal_phase: F)
    where
        F: Fn(u32) -> bool,
    {
        match data.object_type {
            SpawnObjectType::Creature | SpawnObjectType::GameObject => {}
            SpawnObjectType::AreaTrigger => {
                self.remove_area_trigger_spawn(data);
                return;
            }
        }

        self.spawns.remove(&(data.object_type, data.spawn_id));
        let cell_id = data.cell_id();
        if is_personal_phase(data.phase_id) {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = PersonalSpawnMapKey::new(data.map_id, difficulty, data.phase_id);
                if let Some(cells) = self.personal_object_guids.get_mut(&key) {
                    remove_spawn_from_cells(cells, cell_id, data.object_type, data.spawn_id);
                }
            }
        } else {
            for difficulty in data.spawn_difficulties.iter().copied() {
                let key = SpawnMapKey::new(data.map_id, difficulty);
                if let Some(cells) = self.object_guids.get_mut(&key) {
                    remove_spawn_from_cells(cells, cell_id, data.object_type, data.spawn_id);
                }
            }
        }
    }

    pub fn remove_area_trigger_spawn(&mut self, data: &SpawnData) {
        debug_assert_eq!(data.object_type, SpawnObjectType::AreaTrigger);
        self.spawns
            .remove(&(SpawnObjectType::AreaTrigger, data.spawn_id));
        let cell_id = data.cell_id();
        for difficulty in data.spawn_difficulties.iter().copied() {
            let key = SpawnMapKey::new(data.map_id, difficulty);
            if let Some(cells) = self.object_guids.get_mut(&key) {
                remove_spawn_from_cells(
                    cells,
                    cell_id,
                    SpawnObjectType::AreaTrigger,
                    data.spawn_id,
                );
            }
        }
    }

    pub fn cell_object_guids(
        &self,
        map_id: u32,
        difficulty: Difficulty,
        cell_id: u32,
    ) -> Option<&CellSpawnGuids> {
        self.object_guids
            .get(&SpawnMapKey::new(map_id, difficulty))?
            .get(&cell_id)
    }

    pub fn cell_personal_object_guids(
        &self,
        map_id: u32,
        difficulty: Difficulty,
        phase_id: u32,
        cell_id: u32,
    ) -> Option<&CellSpawnGuids> {
        self.personal_object_guids
            .get(&PersonalSpawnMapKey::new(map_id, difficulty, phase_id))?
            .get(&cell_id)
    }

    pub fn has_personal_spawns(&self, map_id: u32, difficulty: Difficulty, phase_id: u32) -> bool {
        self.personal_object_guids
            .contains_key(&PersonalSpawnMapKey::new(map_id, difficulty, phase_id))
    }

    pub fn spawn_data(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<&SpawnData> {
        self.spawns.get(&(object_type, spawn_id))
    }
}

fn insert_spawn(cell: &mut CellSpawnGuids, object_type: SpawnObjectType, spawn_id: SpawnId) {
    match object_type {
        SpawnObjectType::Creature => {
            cell.creatures.insert(spawn_id);
        }
        SpawnObjectType::GameObject => {
            cell.gameobjects.insert(spawn_id);
        }
        SpawnObjectType::AreaTrigger => {
            cell.area_triggers.insert(spawn_id);
        }
    }
}

fn remove_spawn_from_cells(
    cells: &mut BTreeMap<u32, CellSpawnGuids>,
    cell_id: u32,
    object_type: SpawnObjectType,
    spawn_id: SpawnId,
) {
    if let Some(cell) = cells.get_mut(&cell_id) {
        match object_type {
            SpawnObjectType::Creature => {
                cell.creatures.remove(&spawn_id);
            }
            SpawnObjectType::GameObject => {
                cell.gameobjects.remove(&spawn_id);
            }
            SpawnObjectType::AreaTrigger => {
                cell.area_triggers.remove(&spawn_id);
            }
        }
        if cell.is_empty() {
            cells.remove(&cell_id);
        }
    }
}

/// C++ `Map::RespawnInfo` equivalent owned by the map respawn store.
///
/// This is a dependency slice for future live `Map::ProcessRespawns` wiring: it
/// stores and plans respawn timers, but intentionally does not execute PoolMgr,
/// `DoRespawn`, DB persistence/delete, linked-respawn checks, or entity loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespawnInfoLikeCpp {
    pub object_type: SpawnObjectType,
    pub spawn_id: SpawnId,
    pub entry: u32,
    pub respawn_time: i64,
    pub grid_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddRespawnInfoOutcomeLikeCpp {
    Inserted,
    ReplacedExisting,
    RejectedZeroSpawnId,
    RejectedUnsupportedType,
    RejectedExistingSoonerOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckRespawnOutcomeLikeCpp {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRespawnSpawnGroupGuardOutcomeLikeCpp {
    Allowed,
    InactiveSpawnGroupDeletedTimer,
    MissingSpawnData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessRespawnActionLikeCpp {
    UpdatePool {
        pool_id: u32,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    },
    DoRespawn {
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        grid_id: u32,
    },
    DeleteRespawn {
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    },
    RescheduleAndSave {
        info: RespawnInfoLikeCpp,
    },
    InvalidRescheduleNotFuture {
        info: RespawnInfoLikeCpp,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RespawnQueueKey {
    respawn_time: i64,
    spawn_id: SpawnId,
    object_type: SpawnObjectType,
}

impl RespawnQueueKey {
    const fn from_info(info: &RespawnInfoLikeCpp) -> Self {
        Self {
            respawn_time: info.respawn_time,
            spawn_id: info.spawn_id,
            object_type: info.object_type,
        }
    }
}

impl Ord for RespawnQueueKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.respawn_time
            .cmp(&other.respawn_time)
            // Trinity's heap comparator makes larger spawn ids win equal-time ties.
            .then_with(|| other.spawn_id.cmp(&self.spawn_id))
            // Same spawn id can exist for different types; C++ then orders larger type first.
            .then_with(|| other.object_type.cmp(&self.object_type))
    }
}

impl PartialOrd for RespawnQueueKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Default)]
pub struct RespawnStoreLikeCpp {
    creature_respawn_times_by_spawn_id: BTreeMap<SpawnId, RespawnInfoLikeCpp>,
    gameobject_respawn_times_by_spawn_id: BTreeMap<SpawnId, RespawnInfoLikeCpp>,
    respawn_times: BTreeSet<RespawnQueueKey>,
}

impl RespawnStoreLikeCpp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_respawn_info_like_cpp(
        &mut self,
        info: RespawnInfoLikeCpp,
    ) -> AddRespawnInfoOutcomeLikeCpp {
        if info.spawn_id == 0 {
            return AddRespawnInfoOutcomeLikeCpp::RejectedZeroSpawnId;
        }
        if !Self::has_respawn_map_like_cpp(info.object_type) {
            return AddRespawnInfoOutcomeLikeCpp::RejectedUnsupportedType;
        }

        let existing = self
            .get_respawn_info_like_cpp(info.object_type, info.spawn_id)
            .cloned();
        let replaced_existing = if let Some(existing) = existing {
            if info.respawn_time <= existing.respawn_time {
                self.remove_respawn_time_like_cpp(info.object_type, info.spawn_id);
                true
            } else {
                return AddRespawnInfoOutcomeLikeCpp::RejectedExistingSoonerOrEqual;
            }
        } else {
            false
        };

        self.respawn_times.insert(RespawnQueueKey::from_info(&info));
        let Some(by_spawn_id) = self.map_mut_for_type_like_cpp(info.object_type) else {
            self.respawn_times
                .remove(&RespawnQueueKey::from_info(&info));
            return AddRespawnInfoOutcomeLikeCpp::RejectedUnsupportedType;
        };
        by_spawn_id.insert(info.spawn_id, info);

        if replaced_existing {
            AddRespawnInfoOutcomeLikeCpp::ReplacedExisting
        } else {
            AddRespawnInfoOutcomeLikeCpp::Inserted
        }
    }

    pub fn get_respawn_time_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> i64 {
        self.get_respawn_info_like_cpp(object_type, spawn_id)
            .map_or(0, |info| info.respawn_time)
    }

    pub fn get_respawn_info_like_cpp(
        &self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<&RespawnInfoLikeCpp> {
        self.map_for_type_like_cpp(object_type)
            .and_then(|map| map.get(&spawn_id))
    }

    pub fn remove_respawn_time_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
    ) -> Option<RespawnInfoLikeCpp> {
        let info = self
            .map_mut_for_type_like_cpp(object_type)
            .and_then(|map| map.remove(&spawn_id))?;
        self.respawn_times
            .remove(&RespawnQueueKey::from_info(&info));
        Some(info)
    }

    pub fn unload_all_respawn_infos_like_cpp(&mut self) {
        self.respawn_times.clear();
        self.creature_respawn_times_by_spawn_id.clear();
        self.gameobject_respawn_times_by_spawn_id.clear();
    }

    pub fn respawn_timer_keys_like_cpp(
        &self,
    ) -> impl Iterator<Item = (SpawnObjectType, SpawnId)> + '_ {
        self.respawn_times
            .iter()
            .map(|key| (key.object_type, key.spawn_id))
    }

    pub fn process_due_respawns_like_cpp(
        &mut self,
        now: i64,
        mut is_part_of_pool: impl FnMut(SpawnObjectType, SpawnId) -> Option<u32>,
        mut check_respawn: impl FnMut(&mut RespawnInfoLikeCpp) -> CheckRespawnOutcomeLikeCpp,
    ) -> Vec<ProcessRespawnActionLikeCpp> {
        let mut actions = Vec::new();

        while let Some(next_key) = self.respawn_times.iter().next().copied() {
            if now < next_key.respawn_time {
                break;
            }

            let Some(mut info) =
                self.remove_respawn_time_like_cpp(next_key.object_type, next_key.spawn_id)
            else {
                self.respawn_times.remove(&next_key);
                continue;
            };

            if let Some(pool_id) = is_part_of_pool(info.object_type, info.spawn_id) {
                actions.push(ProcessRespawnActionLikeCpp::UpdatePool {
                    pool_id,
                    object_type: info.object_type,
                    spawn_id: info.spawn_id,
                });
                continue;
            }

            match check_respawn(&mut info) {
                CheckRespawnOutcomeLikeCpp::Allowed => {
                    actions.push(ProcessRespawnActionLikeCpp::DoRespawn {
                        object_type: info.object_type,
                        spawn_id: info.spawn_id,
                        grid_id: info.grid_id,
                    });
                }
                CheckRespawnOutcomeLikeCpp::Blocked if info.respawn_time == 0 => {
                    actions.push(ProcessRespawnActionLikeCpp::DeleteRespawn {
                        object_type: info.object_type,
                        spawn_id: info.spawn_id,
                    });
                }
                CheckRespawnOutcomeLikeCpp::Blocked if now < info.respawn_time => {
                    let stored_info = info.clone();
                    let outcome = self.add_respawn_info_like_cpp(info);
                    debug_assert!(matches!(outcome, AddRespawnInfoOutcomeLikeCpp::Inserted));
                    actions
                        .push(ProcessRespawnActionLikeCpp::RescheduleAndSave { info: stored_info });
                }
                CheckRespawnOutcomeLikeCpp::Blocked => {
                    let stored_info = info.clone();
                    let outcome = self.add_respawn_info_like_cpp(info);
                    debug_assert!(matches!(outcome, AddRespawnInfoOutcomeLikeCpp::Inserted));
                    actions.push(ProcessRespawnActionLikeCpp::InvalidRescheduleNotFuture {
                        info: stored_info,
                    });
                    break;
                }
            }
        }

        actions
    }

    const fn has_respawn_map_like_cpp(object_type: SpawnObjectType) -> bool {
        matches!(
            object_type,
            SpawnObjectType::Creature | SpawnObjectType::GameObject
        )
    }

    fn map_for_type_like_cpp(
        &self,
        object_type: SpawnObjectType,
    ) -> Option<&BTreeMap<SpawnId, RespawnInfoLikeCpp>> {
        match object_type {
            SpawnObjectType::Creature => Some(&self.creature_respawn_times_by_spawn_id),
            SpawnObjectType::GameObject => Some(&self.gameobject_respawn_times_by_spawn_id),
            SpawnObjectType::AreaTrigger => None,
        }
    }

    fn map_mut_for_type_like_cpp(
        &mut self,
        object_type: SpawnObjectType,
    ) -> Option<&mut BTreeMap<SpawnId, RespawnInfoLikeCpp>> {
        match object_type {
            SpawnObjectType::Creature => Some(&mut self.creature_respawn_times_by_spawn_id),
            SpawnObjectType::GameObject => Some(&mut self.gameobject_respawn_times_by_spawn_id),
            SpawnObjectType::AreaTrigger => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(object_type: SpawnObjectType, spawn_id: SpawnId, x: f32, y: f32) -> SpawnData {
        SpawnData {
            object_type,
            spawn_id,
            map_id: 571,
            db_data: true,
            spawn_group: SpawnGroupTemplateData::default_group(),
            id: 42,
            spawn_point: SpawnPosition::new(x, y, 1.0, 2.0),
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: -1,
            pool_id: 0,
            spawn_time_secs: 120,
            spawn_difficulties: vec![0, 1],
            script_id: 0,
            string_id: String::new(),
        }
    }

    fn respawn_info(
        object_type: SpawnObjectType,
        spawn_id: SpawnId,
        respawn_time: i64,
    ) -> RespawnInfoLikeCpp {
        RespawnInfoLikeCpp {
            object_type,
            spawn_id,
            entry: 42,
            respawn_time,
            grid_id: 7,
        }
    }

    #[test]
    fn respawn_info_rejects_area_trigger_and_zero_spawn_id_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();

        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::AreaTrigger, 10, 100)),
            AddRespawnInfoOutcomeLikeCpp::RejectedUnsupportedType
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::AreaTrigger, 10),
            0
        );
        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 0, 100)),
            AddRespawnInfoOutcomeLikeCpp::RejectedZeroSpawnId
        );
        assert!(store.respawn_timer_keys_like_cpp().next().is_none());
    }

    #[test]
    fn respawn_info_add_replace_and_later_reject_follow_cpp_ordering() {
        let mut store = RespawnStoreLikeCpp::new();

        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 200)),
            AddRespawnInfoOutcomeLikeCpp::Inserted
        );
        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 250)),
            AddRespawnInfoOutcomeLikeCpp::RejectedExistingSoonerOrEqual
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            200
        );

        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 200)),
            AddRespawnInfoOutcomeLikeCpp::ReplacedExisting
        );
        assert_eq!(
            store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 150)),
            AddRespawnInfoOutcomeLikeCpp::ReplacedExisting
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            150
        );
        assert_eq!(
            store.respawn_timer_keys_like_cpp().collect::<Vec<_>>(),
            vec![(SpawnObjectType::Creature, 10)]
        );
    }

    #[test]
    fn respawn_info_remove_and_unload_keep_maps_and_queue_coherent() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 10, 100));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 20, 90));

        let removed = store.remove_respawn_time_like_cpp(SpawnObjectType::Creature, 10);
        assert_eq!(removed.map(|info| info.spawn_id), Some(10));
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 10),
            0
        );
        assert_eq!(
            store.respawn_timer_keys_like_cpp().collect::<Vec<_>>(),
            vec![(SpawnObjectType::GameObject, 20)]
        );

        store.unload_all_respawn_infos_like_cpp();
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 20),
            0
        );
        assert!(store.respawn_timer_keys_like_cpp().next().is_none());
    }

    #[test]
    fn process_respawns_stops_at_first_future_timer_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 1, 200));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 2, 100));

        let actions = store.process_due_respawns_like_cpp(
            99,
            |_, _| None,
            |_| CheckRespawnOutcomeLikeCpp::Allowed,
        );

        assert!(actions.is_empty());
        assert_eq!(store.respawn_timer_keys_like_cpp().count(), 2);
    }

    #[test]
    fn process_respawns_equal_time_ties_match_cpp_spawn_and_type_priority() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 5, 100));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 5, 100));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 6, 100));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 1, 90));

        let actions = store.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |_| CheckRespawnOutcomeLikeCpp::Allowed,
        );

        assert_eq!(
            actions,
            vec![
                ProcessRespawnActionLikeCpp::DoRespawn {
                    object_type: SpawnObjectType::Creature,
                    spawn_id: 1,
                    grid_id: 7,
                },
                ProcessRespawnActionLikeCpp::DoRespawn {
                    object_type: SpawnObjectType::Creature,
                    spawn_id: 6,
                    grid_id: 7,
                },
                ProcessRespawnActionLikeCpp::DoRespawn {
                    object_type: SpawnObjectType::GameObject,
                    spawn_id: 5,
                    grid_id: 7,
                },
                ProcessRespawnActionLikeCpp::DoRespawn {
                    object_type: SpawnObjectType::Creature,
                    spawn_id: 5,
                    grid_id: 7,
                },
            ]
        );
    }

    #[test]
    fn process_respawns_pool_branch_deletes_before_check_respawn_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 1, 100));
        let mut check_called = false;

        let actions = store.process_due_respawns_like_cpp(
            100,
            |object_type, spawn_id| {
                assert_eq!(object_type, SpawnObjectType::Creature);
                assert_eq!(spawn_id, 1);
                Some(55)
            },
            |_| {
                check_called = true;
                CheckRespawnOutcomeLikeCpp::Allowed
            },
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::UpdatePool {
                pool_id: 55,
                object_type: SpawnObjectType::Creature,
                spawn_id: 1,
            }]
        );
        assert!(!check_called);
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 1),
            0
        );
    }

    #[test]
    fn process_respawns_check_true_deletes_and_plans_do_respawn_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::GameObject, 2, 100));

        let actions = store.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |_| CheckRespawnOutcomeLikeCpp::Allowed,
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::DoRespawn {
                object_type: SpawnObjectType::GameObject,
                spawn_id: 2,
                grid_id: 7,
            }]
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::GameObject, 2),
            0
        );
    }

    #[test]
    fn process_respawns_check_false_zero_deletes_entry_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 3, 100));

        let actions = store.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |info| {
                info.respawn_time = 0;
                CheckRespawnOutcomeLikeCpp::Blocked
            },
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::DeleteRespawn {
                object_type: SpawnObjectType::Creature,
                spawn_id: 3,
            }]
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 3),
            0
        );
    }

    #[test]
    fn process_respawns_check_false_future_reschedules_and_saves_like_cpp() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 4, 100));

        let actions = store.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |info| {
                info.respawn_time = 150;
                CheckRespawnOutcomeLikeCpp::Blocked
            },
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::RescheduleAndSave {
                info: respawn_info(SpawnObjectType::Creature, 4, 150),
            }]
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 4),
            150
        );
    }

    #[test]
    fn process_respawns_invalid_non_future_reschedule_reports_and_does_not_loop() {
        let mut store = RespawnStoreLikeCpp::new();
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 5, 100));
        store.add_respawn_info_like_cpp(respawn_info(SpawnObjectType::Creature, 6, 100));

        let actions = store.process_due_respawns_like_cpp(
            100,
            |_, _| None,
            |info| {
                info.respawn_time = 99;
                CheckRespawnOutcomeLikeCpp::Blocked
            },
        );

        assert_eq!(
            actions,
            vec![ProcessRespawnActionLikeCpp::InvalidRescheduleNotFuture {
                info: respawn_info(SpawnObjectType::Creature, 6, 99),
            }]
        );
        assert_eq!(
            store.get_respawn_time_like_cpp(SpawnObjectType::Creature, 6),
            99
        );
        assert_eq!(store.respawn_timer_keys_like_cpp().count(), 2);
    }

    #[test]
    fn spawn_constants_match_spawn_data_h() {
        assert_eq!(SpawnObjectType::Creature as u8, 0);
        assert_eq!(SpawnObjectType::GameObject as u8, 1);
        assert_eq!(SpawnObjectType::AreaTrigger as u8, 2);
        assert_eq!(SpawnObjectType::Creature.mask(), 0x1);
        assert_eq!(SpawnObjectType::GameObject.mask(), 0x2);
        assert_eq!(SpawnObjectType::AreaTrigger.mask(), 0x4);
        assert_eq!(SpawnGroupFlags::ALL.0, 0x3f);
        assert!(
            SpawnGroupFlags(0xff)
                .truncate_to_all()
                .contains(SpawnGroupFlags::SYSTEM)
        );
    }

    #[test]
    fn object_spawn_store_indexes_creatures_by_map_difficulty_and_cell() {
        let mut store = SpawnStore::new();
        let data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |_| false);

        assert!(
            store
                .cell_object_guids(571, 0, cell_id)
                .unwrap()
                .creatures
                .contains(&100)
        );
        assert!(
            store
                .cell_object_guids(571, 1, cell_id)
                .unwrap()
                .creatures
                .contains(&100)
        );
        assert!(store.cell_object_guids(571, 2, cell_id).is_none());
    }

    #[test]
    fn insert_spawn_metadata_like_cpp_does_not_touch_grid_indexes() {
        let mut store = SpawnStore::new();
        let data = spawn(SpawnObjectType::Creature, 150, 0.0, 0.0);
        let cell_id = data.cell_id();

        store.insert_spawn_metadata_like_cpp(&data);

        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 150)
                .map(|spawn| spawn.spawn_id),
            Some(150)
        );
        assert!(store.cell_object_guids(571, 0, cell_id).is_none());
        assert!(store.cell_object_guids(571, 1, cell_id).is_none());
    }

    #[test]
    fn object_spawn_store_uses_personal_phase_index_for_creatures_and_gameobjects() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::GameObject, 200, 0.0, 0.0);
        data.phase_id = 9001;
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |phase_id| phase_id == 9001);

        assert!(store.cell_object_guids(571, 0, cell_id).is_none());
        assert!(store.has_personal_spawns(571, 0, 9001));
        assert!(
            store
                .cell_personal_object_guids(571, 0, 9001, cell_id)
                .unwrap()
                .gameobjects
                .contains(&200)
        );
    }

    #[test]
    fn area_trigger_store_follows_cpp_non_personal_location_index() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::AreaTrigger, 300, 0.0, 0.0);
        data.phase_id = 9001;
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |phase_id| phase_id == 9001);

        assert!(
            store
                .cell_object_guids(571, 0, cell_id)
                .unwrap()
                .area_triggers
                .contains(&300)
        );
        assert!(
            store
                .cell_personal_object_guids(571, 0, 9001, cell_id)
                .is_none()
        );
    }

    #[test]
    fn removing_spawn_cleans_empty_cell_entry() {
        let mut store = SpawnStore::new();
        let data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        let cell_id = data.cell_id();

        store.add_object_spawn(&data, |_| false);
        assert!(store.cell_object_guids(571, 0, cell_id).is_some());

        store.remove_object_spawn(&data, |_| false);
        assert!(store.cell_object_guids(571, 0, cell_id).is_none());
        assert!(store.cell_object_guids(571, 1, cell_id).is_none());
    }

    fn template(group_id: u32, map_id: u32, flags: SpawnGroupFlags) -> SpawnGroupTemplateData {
        SpawnGroupTemplateData {
            group_id,
            name: format!("group-{group_id}"),
            map_id,
            flags,
        }
    }

    #[test]
    fn spawn_group_defaults_match_cpp_system_unassigned_shape() {
        let default = SpawnGroupTemplateData::default_group();

        assert_eq!(SPAWNGROUP_MAP_UNSET, 0xffff_ffff);
        assert_eq!(default.group_id, 0);
        assert_eq!(default.name, "Default Group");
        assert_eq!(default.map_id, 0);
        assert!(default.is_system());

        let legacy = SpawnGroupTemplateData::legacy_group();
        assert_eq!(legacy.group_id, 1);
        assert_eq!(legacy.map_id, 0);
        assert!(legacy.is_system());
        assert!(legacy.flags.contains(SpawnGroupFlags::COMPATIBILITY_MODE));
    }

    #[test]
    fn spawn_group_system_active_even_when_set_attempt_is_noop() {
        let system = template(10, 571, SpawnGroupFlags::SYSTEM);
        let mut state = SpawnGroupRuntimeState::new();

        assert!(state.is_spawn_group_active_like_cpp(Some(&system)));
        assert_eq!(
            state.set_spawn_group_active_like_cpp(Some(&system), false),
            SpawnGroupActiveChange::SystemGroup
        );
        assert!(!state.is_toggled(10));
        assert!(state.is_spawn_group_active_like_cpp(Some(&system)));
    }

    #[test]
    fn spawn_group_non_manual_defaults_active_and_toggle_matches_cpp() {
        let group = template(10, 571, SpawnGroupFlags::NONE);
        let mut state = SpawnGroupRuntimeState::new();

        assert!(state.is_spawn_group_active_like_cpp(Some(&group)));
        assert_eq!(
            state.set_spawn_group_active_like_cpp(Some(&group), false),
            SpawnGroupActiveChange::Toggled
        );
        assert!(state.is_toggled(10));
        assert!(!state.is_spawn_group_active_like_cpp(Some(&group)));

        assert_eq!(
            state.set_spawn_group_active_like_cpp(Some(&group), true),
            SpawnGroupActiveChange::ClearedToggle
        );
        assert!(!state.is_toggled(10));
        assert!(state.is_spawn_group_active_like_cpp(Some(&group)));
    }

    #[test]
    fn spawn_group_manual_defaults_inactive_and_toggle_matches_cpp() {
        let group = template(10, 571, SpawnGroupFlags::MANUAL_SPAWN);
        let mut state = SpawnGroupRuntimeState::new();

        assert!(!state.is_spawn_group_active_like_cpp(Some(&group)));
        assert_eq!(
            state.set_spawn_group_active_like_cpp(Some(&group), true),
            SpawnGroupActiveChange::Toggled
        );
        assert!(state.is_toggled(10));
        assert!(state.is_spawn_group_active_like_cpp(Some(&group)));

        assert_eq!(
            state.set_spawn_group_active_like_cpp(Some(&group), false),
            SpawnGroupActiveChange::ClearedToggle
        );
        assert!(!state.is_toggled(10));
        assert!(!state.is_spawn_group_active_like_cpp(Some(&group)));
    }

    #[test]
    fn spawn_group_missing_query_is_false_and_set_reports_missing() {
        let mut state = SpawnGroupRuntimeState::new();

        assert!(!state.is_spawn_group_active_like_cpp(None));
        assert_eq!(
            state.set_spawn_group_active_like_cpp(None, true),
            SpawnGroupActiveChange::MissingGroup
        );
    }

    #[test]
    fn should_spawn_on_grid_load_honors_respawn_before_group_and_pool() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(10, 571, SpawnGroupFlags::MANUAL_SPAWN);
        data.pool_id = 55;
        store.add_object_spawn(&data, |_| false);
        let state = SpawnGroupRuntimeState::new();
        let filter = SpawnGridLoadStateLikeCpp::new(&store, &state)
            .with_respawn_timers([(SpawnObjectType::Creature, 100)]);

        assert!(!filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
    }

    #[test]
    fn should_spawn_on_grid_load_rejects_non_system_inactive_group() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(10, 571, SpawnGroupFlags::MANUAL_SPAWN);
        store.add_object_spawn(&data, |_| false);
        let state = SpawnGroupRuntimeState::new();
        let filter = SpawnGridLoadStateLikeCpp::new(&store, &state);

        assert!(!filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
    }

    #[test]
    fn should_spawn_on_grid_load_pool_zero_ignores_pool_selection() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(10, 571, SpawnGroupFlags::NONE);
        data.pool_id = 0;
        store.add_object_spawn(&data, |_| false);
        let state = SpawnGroupRuntimeState::new();
        let filter = SpawnGridLoadStateLikeCpp::new(&store, &state);

        assert!(filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
    }

    #[test]
    fn should_spawn_on_grid_load_pooled_spawn_requires_selected_object() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(10, 571, SpawnGroupFlags::NONE);
        data.pool_id = 55;
        store.add_object_spawn(&data, |_| false);
        let state = SpawnGroupRuntimeState::new();
        let mut filter = SpawnGridLoadStateLikeCpp::new(&store, &state);

        assert!(!filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
        filter.add_pool_spawned_object(SpawnObjectType::Creature, 100);
        assert!(filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
    }

    #[test]
    fn should_spawn_on_grid_load_allows_system_no_respawn_no_pool() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(10, 571, SpawnGroupFlags::SYSTEM);
        store.add_object_spawn(&data, |_| false);
        let state = SpawnGroupRuntimeState::new();
        let filter = SpawnGridLoadStateLikeCpp::new(&store, &state);

        assert!(filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 100));
    }

    #[test]
    fn should_spawn_on_grid_load_missing_metadata_is_false_without_panic() {
        let store = SpawnStore::new();
        let state = SpawnGroupRuntimeState::new();
        let filter = SpawnGridLoadStateLikeCpp::new(&store, &state);

        assert!(!filter.should_be_spawned_on_grid_load(SpawnObjectType::Creature, 999));
    }

    #[test]
    fn apply_spawn_groups_assigns_all_spawn_types_and_preserves_flags() {
        let mut store = SpawnStore::new();
        for (object_type, spawn_id) in [
            (SpawnObjectType::Creature, 100),
            (SpawnObjectType::GameObject, 200),
            (SpawnObjectType::AreaTrigger, 300),
        ] {
            store.add_object_spawn(&spawn(object_type, spawn_id, 0.0, 0.0), |_| false);
        }
        let mut templates = BTreeMap::from([
            (10, template(10, 571, SpawnGroupFlags::MANUAL_SPAWN)),
            (11, template(11, 571, SpawnGroupFlags::DYNAMIC_SPAWN_RATE)),
            (
                12,
                template(12, 571, SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE),
            ),
        ]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: 0,
                    spawn_id: 100,
                },
                SpawnGroupMemberRow {
                    group_id: 11,
                    spawn_type: 1,
                    spawn_id: 200,
                },
                SpawnGroupMemberRow {
                    group_id: 12,
                    spawn_type: 2,
                    spawn_id: 300,
                },
            ],
        );

        assert_eq!(report.assigned, 3);
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .flags,
            SpawnGroupFlags::MANUAL_SPAWN
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::GameObject, 200)
                .unwrap()
                .spawn_group
                .flags,
            SpawnGroupFlags::DYNAMIC_SPAWN_RATE
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::AreaTrigger, 300)
                .unwrap()
                .spawn_group
                .flags,
            SpawnGroupFlags::DESPAWN_ON_CONDITION_FAILURE
        );
        assert!(
            store
                .spawn_group_members(10)
                .unwrap()
                .contains(&SpawnGroupMember {
                    object_type: SpawnObjectType::Creature,
                    spawn_id: 100
                })
        );
        assert!(
            store
                .spawn_group_members(11)
                .unwrap()
                .contains(&SpawnGroupMember {
                    object_type: SpawnObjectType::GameObject,
                    spawn_id: 200
                })
        );
        assert!(
            store
                .spawn_group_members(12)
                .unwrap()
                .contains(&SpawnGroupMember {
                    object_type: SpawnObjectType::AreaTrigger,
                    spawn_id: 300
                })
        );
    }

    #[test]
    fn first_assignment_sets_unset_group_map_and_indexes_group_by_map() {
        let mut store = SpawnStore::new();
        store.add_object_spawn(&spawn(SpawnObjectType::Creature, 100, 0.0, 0.0), |_| false);
        let mut templates = BTreeMap::from([(
            10,
            template(10, SPAWNGROUP_MAP_UNSET, SpawnGroupFlags::NONE),
        )]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [SpawnGroupMemberRow {
                group_id: 10,
                spawn_type: 0,
                spawn_id: 100,
            }],
        );

        assert_eq!(report.assigned, 1);
        assert_eq!(templates.get(&10).unwrap().map_id, 571);
        assert!(store.spawn_group_ids_by_map(571).unwrap().contains(&10));
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .map_id,
            571
        );
    }

    #[test]
    fn system_group_allows_cross_map_without_member_index_but_non_system_skips() {
        let mut store = SpawnStore::new();
        let mut cross_map = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        cross_map.map_id = 571;
        store.add_object_spawn(&cross_map, |_| false);
        let mut blocked = spawn(SpawnObjectType::GameObject, 200, 0.0, 0.0);
        blocked.map_id = 571;
        store.add_object_spawn(&blocked, |_| false);
        let mut templates = BTreeMap::from([
            (10, template(10, 1, SpawnGroupFlags::SYSTEM)),
            (11, template(11, 1, SpawnGroupFlags::NONE)),
        ]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: 0,
                    spawn_id: 100,
                },
                SpawnGroupMemberRow {
                    group_id: 11,
                    spawn_type: 1,
                    spawn_id: 200,
                },
            ],
        );

        assert_eq!(report.assigned, 1);
        assert_eq!(report.map_mismatch, 1);
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .group_id,
            10
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::GameObject, 200)
                .unwrap()
                .spawn_group
                .group_id,
            0
        );
        assert!(store.spawn_group_members(10).is_none());
        assert!(store.spawn_group_members(11).is_none());
    }

    #[test]
    fn duplicate_nonzero_group_is_reported_and_skipped() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = template(20, 571, SpawnGroupFlags::NONE);
        store.add_object_spawn(&data, |_| false);
        let mut templates = BTreeMap::from([(10, template(10, 571, SpawnGroupFlags::NONE))]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [SpawnGroupMemberRow {
                group_id: 10,
                spawn_type: 0,
                spawn_id: 100,
            }],
        );

        assert_eq!(report.assigned, 0);
        assert_eq!(report.duplicate_spawn_group, 1);
        assert_eq!(report.issues[0].existing_group_id, Some(20));
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .group_id,
            20
        );
    }

    #[test]
    fn invalid_type_missing_spawn_and_missing_group_report_without_mutation() {
        let mut store = SpawnStore::new();
        store.add_object_spawn(&spawn(SpawnObjectType::Creature, 100, 0.0, 0.0), |_| false);
        let mut templates = BTreeMap::from([(10, template(10, 571, SpawnGroupFlags::NONE))]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: 99,
                    spawn_id: 100,
                },
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: 1,
                    spawn_id: 999,
                },
                SpawnGroupMemberRow {
                    group_id: 99,
                    spawn_type: 0,
                    spawn_id: 100,
                },
            ],
        );

        assert_eq!(report.assigned, 0);
        assert_eq!(report.invalid_type, 1);
        assert_eq!(report.missing_spawn, 1);
        assert_eq!(report.missing_group, 1);
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .group_id,
            0
        );
        assert!(store.spawn_group_members(10).is_none());
    }

    #[test]
    fn default_group_zero_does_not_trigger_duplicate_and_behaves_unassigned() {
        let mut store = SpawnStore::new();
        let mut data = spawn(SpawnObjectType::Creature, 100, 0.0, 0.0);
        data.spawn_group = SpawnGroupTemplateData::default_group();
        store.add_object_spawn(&data, |_| false);
        let mut templates = BTreeMap::from([(10, template(10, 571, SpawnGroupFlags::NONE))]);

        let report = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [SpawnGroupMemberRow {
                group_id: 10,
                spawn_type: 0,
                spawn_id: 100,
            }],
        );

        assert_eq!(report.assigned, 1);
        assert_eq!(report.duplicate_spawn_group, 0);
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 100)
                .unwrap()
                .spawn_group
                .group_id,
            10
        );
    }
}
