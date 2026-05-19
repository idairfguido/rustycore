//! Canonical spawn metadata loader for `world-server` startup.
//!
//! Scope: metadata/index dependency only. This builds an in-memory
//! `wow_map::SpawnStore` from DB rows and applies `spawn_group`; it does not
//! create live entities, activate spawn groups, run respawn/pool logic, or fan
//! out to sessions.
//!
//! C++ anchors used by this module/tests:
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2138-2165`
//!   `ObjectMgr::ParseSpawnDifficulties`.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2167-2242`
//!   `ObjectMgr::LoadCreatures` query fields and default/legacy spawn group.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2413-2485`
//!   game-event gate and `AddSpawnDataToGrid` / `AddCreatureToGrid`.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2492-2618`
//!   `ObjectMgr::LoadGameObjects` query fields, difficulties/event/pool.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2676-2736`
//!   validation tail and `AddGameobjectToGrid`.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/AreaTriggerDataStore.cpp:312-419`
//!   `LoadAreaTriggerSpawns` query/parse/index/default legacy group.
//! - Existing Rust DB statements:
//!   `/home/server/rustycore/crates/wow-database/src/statements/world.rs:467-529`.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Globals/ObjectMgr.cpp:2798-2862`
//!   `ObjectMgr::LoadSpawnGroups` mutates spawn-group template map metadata and indexes
//!   `_spawnGroupsByMap` / `_spawnGroupMapStore` for non-system groups.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2455-2468`
//!   `Map::InitSpawnGroupState` reads `GetSpawnGroupsForMap(GetId())`, resolves each
//!   `GetSpawnGroupData(groupId)`, skips system groups, checks conditions, and toggles the map.
//! - `/home/server/woltk-trinity-legacy/src/server/game/Conditions/ConditionMgr.cpp:1142-1145`
//!   future map-condition consumer entry point; conditions are not evaluated in this loader.

use std::collections::BTreeMap;

use anyhow::Result;
use wow_core::{ObjectGuid, Position, guid::HighGuid};
use wow_database::{WorldDatabase, WorldStatements};
use wow_map::pool::{
    PoolGroupLikeCpp, PoolMemberKindLikeCpp, PoolMgrLikeCpp, PoolObjectLikeCpp,
    PoolTemplateDataLikeCpp,
};
use wow_map::spawn::{
    LinkedRespawnLoadIssueKindLikeCpp, LinkedRespawnLoadIssueLikeCpp,
    LinkedRespawnLoadReportLikeCpp, LinkedRespawnRowLikeCpp, LinkedRespawnTypeLikeCpp,
    SPAWNGROUP_MAP_UNSET, SpawnGroupApplyReport, SpawnGroupMemberRow,
};
use wow_map::{
    Difficulty, LinkedRespawnStoreLikeCpp, SpawnData, SpawnGroupFlags, SpawnGroupTemplateData,
    SpawnId, SpawnObjectType, SpawnPosition, SpawnStore,
};

const DIFFICULTY_NONE_LIKE_CPP: Difficulty = 0;
const PERSONAL_PHASE_FLAG_LIKE_CPP: u32 = 0x8000_0000;
const TRANSPORT_MAP_IDS_REPRESENTED: &[u32] = &[];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpawnKindLoadReport {
    pub rows: usize,
    pub indexed: usize,
    pub skipped_event: usize,
    pub skipped_empty_difficulties: usize,
    pub skipped_missing_map: usize,
    pub skipped_invalid_position: usize,
    pub validation_skipped: usize,
    pub script_id_unresolved: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CanonicalSpawnStoreLoadReport {
    pub creature: SpawnKindLoadReport,
    pub gameobject: SpawnKindLoadReport,
    pub area_trigger: SpawnKindLoadReport,
    pub spawn_group_rows: usize,
    pub spawn_group_apply: SpawnGroupApplyReport,
    pub linked_respawn: LinkedRespawnLoadReportLikeCpp,
    pub pool_mgr: PoolMgrLoadReportLikeCpp,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoolMemberLoadReportLikeCpp {
    pub rows: usize,
    pub loaded: usize,
    pub skipped_missing_spawn: usize,
    pub skipped_missing_template: usize,
    pub skipped_invalid_chance: usize,
    pub skipped_map_mismatch: usize,
    pub skipped_child_id_overflow: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoolMgrLoadReportLikeCpp {
    pub template_rows: usize,
    pub templates_loaded: usize,
    pub creature_members: PoolMemberLoadReportLikeCpp,
    pub gameobject_members: PoolMemberLoadReportLikeCpp,
    pub pool_members: PoolMemberLoadReportLikeCpp,
    pub relation_removals: usize,
    pub map_mismatches: usize,
    pub circular_relations: usize,
    pub empty_pools: usize,
    pub missing_map_after_non_empty: usize,
    pub autospawn_rows: usize,
    pub autospawn_loaded: usize,
    pub autospawn_skipped_empty: usize,
    pub autospawn_skipped_broken: usize,
    pub autospawn_skipped_child: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CanonicalSpawnMetadataLikeCpp {
    spawn_store: SpawnStore,
    spawn_group_templates: BTreeMap<u32, SpawnGroupTemplateData>,
    linked_respawns: LinkedRespawnStoreLikeCpp,
    pool_mgr: PoolMgrLikeCpp,
    creature_runtime_rows: BTreeMap<SpawnId, CreatureSpawnRuntimeRowLikeCpp>,
    gameobject_runtime_rows: BTreeMap<SpawnId, GameObjectSpawnRuntimeRowLikeCpp>,
}

impl CanonicalSpawnMetadataLikeCpp {
    pub fn new(
        spawn_store: SpawnStore,
        spawn_group_templates: BTreeMap<u32, SpawnGroupTemplateData>,
    ) -> Self {
        Self {
            spawn_store,
            spawn_group_templates,
            linked_respawns: LinkedRespawnStoreLikeCpp::new(),
            pool_mgr: PoolMgrLikeCpp::new(),
            creature_runtime_rows: BTreeMap::new(),
            gameobject_runtime_rows: BTreeMap::new(),
        }
    }

    pub fn spawn_store(&self) -> &SpawnStore {
        &self.spawn_store
    }

    pub fn spawn_group_templates(&self) -> &BTreeMap<u32, SpawnGroupTemplateData> {
        &self.spawn_group_templates
    }

    pub fn with_linked_respawns_like_cpp(
        mut self,
        linked_respawns: LinkedRespawnStoreLikeCpp,
    ) -> Self {
        self.linked_respawns = linked_respawns;
        self
    }

    pub fn with_pool_mgr_like_cpp(mut self, pool_mgr: PoolMgrLikeCpp) -> Self {
        self.pool_mgr = pool_mgr;
        self
    }

    pub fn linked_respawns_like_cpp(&self) -> &LinkedRespawnStoreLikeCpp {
        &self.linked_respawns
    }

    pub fn pool_mgr_like_cpp(&self) -> &PoolMgrLikeCpp {
        &self.pool_mgr
    }

    pub fn creature_runtime_row_like_cpp(
        &self,
        spawn_id: SpawnId,
    ) -> Option<&CreatureSpawnRuntimeRowLikeCpp> {
        self.creature_runtime_rows.get(&spawn_id)
    }

    pub fn with_creature_runtime_rows_like_cpp(
        mut self,
        rows: BTreeMap<SpawnId, CreatureSpawnRuntimeRowLikeCpp>,
    ) -> Self {
        self.creature_runtime_rows = rows;
        self
    }

    pub fn gameobject_runtime_row_like_cpp(
        &self,
        spawn_id: SpawnId,
    ) -> Option<&GameObjectSpawnRuntimeRowLikeCpp> {
        self.gameobject_runtime_rows.get(&spawn_id)
    }

    pub fn with_gameobject_runtime_rows_like_cpp(
        mut self,
        rows: BTreeMap<SpawnId, GameObjectSpawnRuntimeRowLikeCpp>,
    ) -> Self {
        self.gameobject_runtime_rows = rows;
        self
    }

    /// C++ shaped dependency for future `Map::InitSpawnGroupState` wiring.
    ///
    /// Mirrors the read side of
    /// `/home/server/woltk-trinity-legacy/src/server/game/Maps/Map.cpp:2455-2468`:
    /// use `GetSpawnGroupsForMap(mapId)` order, then resolve each group through the
    /// `GetSpawnGroupData(groupId)`/map filter shape. Missing maps/templates are runtime-empty,
    /// not panics. This does not evaluate `ConditionMgr` or mutate map-owned runtime toggles.
    pub fn spawn_group_templates_for_map_like_cpp(
        &self,
        map_id: u32,
    ) -> Vec<(u32, &SpawnGroupTemplateData)> {
        self.spawn_store
            .spawn_group_ids_by_map(map_id)
            .into_iter()
            .flat_map(|group_ids| group_ids.iter().copied())
            .filter_map(|group_id| {
                SpawnStore::spawn_group_template_for_map(
                    &self.spawn_group_templates,
                    group_id,
                    map_id,
                )
                .map(|template| (group_id, template))
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnDifficultyParseReport {
    pub invalid_tokens_as_none: usize,
    pub unsupported: Vec<Difficulty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSpawnDifficulties {
    pub difficulties: Vec<Difficulty>,
    pub report: SpawnDifficultyParseReport,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatureSpawnRuntimeRowLikeCpp {
    pub spawn_id: SpawnId,
    pub model_id: u32,
    pub equipment_id: i8,
    pub wander_distance: f32,
    pub curhealth: u32,
    pub curmana: u32,
    pub movement_type: u8,
    pub string_id: String,
    pub spawn_time_secs: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameObjectSpawnRuntimeRowLikeCpp {
    pub spawn_id: SpawnId,
    pub rotation: [f32; 4],
    pub anim_progress: u8,
    pub state: u8,
    pub string_id: String,
    pub spawn_time_secs: i32,
}

#[derive(Debug, Clone)]
struct CreatureSpawnRow {
    spawn_id: SpawnId,
    entry: u32,
    map_id: u32,
    x: f32,
    y: f32,
    z: f32,
    orientation: f32,
    model_id: u32,
    equipment_id: i8,
    spawn_time_secs: i32,
    wander_distance: f32,
    curhealth: u32,
    curmana: u32,
    movement_type: u8,
    spawn_difficulties: String,
    event_entry: i16,
    pool_id: u32,
    phase_use_flags: u8,
    phase_id: u32,
    phase_group: u32,
    terrain_swap_map: i32,
    script_name: String,
    string_id: String,
}

#[derive(Debug, Clone)]
struct GameObjectSpawnRow {
    spawn_id: SpawnId,
    entry: u32,
    map_id: u32,
    x: f32,
    y: f32,
    z: f32,
    orientation: f32,
    rotation: [f32; 4],
    spawn_time_secs: i32,
    anim_progress: u8,
    state: u8,
    spawn_difficulties: String,
    event_entry: i16,
    pool_id: u32,
    phase_use_flags: u8,
    phase_id: u32,
    phase_group: u32,
    terrain_swap_map: i32,
    script_name: String,
    string_id: String,
}

#[derive(Debug, Clone)]
struct AreaTriggerSpawnRow {
    spawn_id: SpawnId,
    create_properties_id: u32,
    map_id: u32,
    spawn_difficulties: String,
    x: f32,
    y: f32,
    z: f32,
    orientation: f32,
    phase_use_flags: u8,
    phase_id: u32,
    phase_group: u32,
    script_name: String,
}

#[derive(Debug, Clone, Copy)]
struct LinkedRespawnDbRow {
    guid: SpawnId,
    linked_guid: SpawnId,
    link_type: u8,
}

#[derive(Debug, Clone, Copy)]
struct PoolTemplateRowLikeCpp {
    entry: u32,
    max_limit: u32,
}

#[derive(Debug, Clone, Copy)]
struct PoolMemberRowLikeCpp {
    spawn_id: u64,
    pool_spawn_id: u32,
    chance: f32,
}

#[derive(Debug, Clone, Copy)]
struct PoolAutospawnCandidateRowLikeCpp {
    pool_entry: u32,
    child_pool_id: u64,
    mother_pool_id: u32,
}

impl From<LinkedRespawnDbRow> for LinkedRespawnRowLikeCpp {
    fn from(row: LinkedRespawnDbRow) -> Self {
        Self {
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
        }
    }
}

pub async fn load_canonical_spawn_store_like_cpp(
    db: &WorldDatabase,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    spawn_group_store: &wow_data::SpawnGroupTemplateStore,
) -> Result<(CanonicalSpawnMetadataLikeCpp, CanonicalSpawnStoreLoadReport)> {
    let mut store = SpawnStore::new();
    let mut creature_runtime_rows = BTreeMap::new();
    let mut gameobject_runtime_rows = BTreeMap::new();
    let mut report = CanonicalSpawnStoreLoadReport::default();

    load_creature_spawns_like_cpp(
        db,
        map_store,
        map_difficulty_store,
        &mut store,
        &mut creature_runtime_rows,
        &mut report,
    )
    .await?;
    load_gameobject_spawns_like_cpp(
        db,
        map_store,
        map_difficulty_store,
        &mut store,
        &mut gameobject_runtime_rows,
        &mut report,
    )
    .await?;
    load_area_trigger_spawns_like_cpp(db, map_store, map_difficulty_store, &mut store, &mut report)
        .await?;

    // C++ `ObjectMgr::LoadLinkedRespawn` runs after creature/gameobject data is canonical.
    let linked_respawns = load_linked_respawns_like_cpp(db, &store, map_store, &mut report).await?;

    // C++ `PoolMgr::LoadFromDB` uses ObjectMgr creature/gameobject spawn data as
    // existence/map truth. This builds only PoolMgr metadata/plans; no live spawn.
    let pool_mgr = load_pool_mgr_like_cpp(db, &store, &mut report).await?;

    let mut templates = spawn_group_templates_for_spawn_store(spawn_group_store);
    let members = load_spawn_group_members_like_cpp(db).await?;
    report.spawn_group_rows = members.len();
    report.spawn_group_apply = store.apply_spawn_groups_like_cpp(&mut templates, members);

    Ok((
        CanonicalSpawnMetadataLikeCpp::new(store, templates)
            .with_linked_respawns_like_cpp(linked_respawns)
            .with_pool_mgr_like_cpp(pool_mgr)
            .with_creature_runtime_rows_like_cpp(creature_runtime_rows)
            .with_gameobject_runtime_rows_like_cpp(gameobject_runtime_rows),
        report,
    ))
}

async fn load_pool_mgr_like_cpp(
    db: &WorldDatabase,
    store: &SpawnStore,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<PoolMgrLikeCpp> {
    let mut mgr = PoolMgrLikeCpp::new();

    let stmt = db.prepare(WorldStatements::SEL_POOL_TEMPLATES);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(mgr);
    }
    loop {
        apply_pool_template_row_like_cpp(
            PoolTemplateRowLikeCpp {
                entry: result.read(0),
                max_limit: result.read(1),
            },
            &mut mgr,
            &mut report.pool_mgr,
        );
        if !result.next_row() {
            break;
        }
    }

    load_pool_member_rows_like_cpp(db, store, PoolMemberKindLikeCpp::Creature, &mut mgr, report)
        .await?;
    load_pool_member_rows_like_cpp(
        db,
        store,
        PoolMemberKindLikeCpp::GameObject,
        &mut mgr,
        report,
    )
    .await?;
    load_pool_member_rows_like_cpp(db, store, PoolMemberKindLikeCpp::Pool, &mut mgr, report)
        .await?;

    apply_pool_map_propagation_like_cpp(&mut mgr, &mut report.pool_mgr);
    apply_pool_final_validation_like_cpp(&mgr, &mut report.pool_mgr);
    load_pool_autospawn_candidates_like_cpp(db, &mut mgr, report).await?;

    Ok(mgr)
}

async fn load_pool_member_rows_like_cpp(
    db: &WorldDatabase,
    store: &SpawnStore,
    kind: PoolMemberKindLikeCpp,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<()> {
    let mut stmt = db.prepare(WorldStatements::SEL_POOL_MEMBERS_BY_TYPE);
    stmt.set_u8(0, kind as u8);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(());
    }

    loop {
        let row = PoolMemberRowLikeCpp {
            spawn_id: result.read(0),
            pool_spawn_id: result.read(1),
            chance: result.read(2),
        };
        match kind {
            PoolMemberKindLikeCpp::Creature | PoolMemberKindLikeCpp::GameObject => {
                apply_pool_spawn_member_row_like_cpp(row, store, kind, mgr, &mut report.pool_mgr);
            }
            PoolMemberKindLikeCpp::Pool => {
                apply_pool_pool_member_row_like_cpp(row, mgr, &mut report.pool_mgr);
            }
        }
        if !result.next_row() {
            break;
        }
    }

    Ok(())
}

async fn load_pool_autospawn_candidates_like_cpp(
    db: &WorldDatabase,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<()> {
    let stmt = db.prepare(WorldStatements::SEL_POOL_AUTOSPAWN_CANDIDATES);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(());
    }

    loop {
        apply_pool_autospawn_candidate_row_like_cpp(
            PoolAutospawnCandidateRowLikeCpp {
                pool_entry: result.read(0),
                child_pool_id: result.try_read(1).unwrap_or(0),
                mother_pool_id: result.try_read(2).unwrap_or(0),
            },
            mgr,
            &mut report.pool_mgr,
        );
        if !result.next_row() {
            break;
        }
    }

    Ok(())
}

fn apply_pool_template_row_like_cpp(
    row: PoolTemplateRowLikeCpp,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    report.template_rows += 1;
    mgr.insert_template_like_cpp(row.entry, PoolTemplateDataLikeCpp::new(row.max_limit, -1));
    report.templates_loaded += 1;
}

fn apply_pool_spawn_member_row_like_cpp(
    row: PoolMemberRowLikeCpp,
    store: &SpawnStore,
    kind: PoolMemberKindLikeCpp,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    let member_report = match kind {
        PoolMemberKindLikeCpp::Creature => &mut report.creature_members,
        PoolMemberKindLikeCpp::GameObject => &mut report.gameobject_members,
        PoolMemberKindLikeCpp::Pool => {
            unreachable!("pool rows use apply_pool_pool_member_row_like_cpp")
        }
    };
    member_report.rows += 1;

    let spawn_type = match kind {
        PoolMemberKindLikeCpp::Creature => SpawnObjectType::Creature,
        PoolMemberKindLikeCpp::GameObject => SpawnObjectType::GameObject,
        PoolMemberKindLikeCpp::Pool => {
            unreachable!("pool rows use apply_pool_pool_member_row_like_cpp")
        }
    };
    let Some(spawn_data) = store.spawn_data(spawn_type, row.spawn_id) else {
        member_report.skipped_missing_spawn += 1;
        return;
    };
    let Some(template) = mgr.templates.get_mut(&row.pool_spawn_id) else {
        member_report.skipped_missing_template += 1;
        return;
    };
    if !(0.0..=100.0).contains(&row.chance) {
        member_report.skipped_invalid_chance += 1;
        return;
    }

    let map_id = match i32::try_from(spawn_data.map_id) {
        Ok(map_id) => map_id,
        Err(_) => {
            member_report.skipped_map_mismatch += 1;
            return;
        }
    };
    if template.map_id == -1 {
        template.map_id = map_id;
    }
    if template.map_id != map_id {
        member_report.skipped_map_mismatch += 1;
        return;
    }

    let max_limit = template.max_limit;
    let group_map = match kind {
        PoolMemberKindLikeCpp::Creature => &mut mgr.creature_groups,
        PoolMemberKindLikeCpp::GameObject => &mut mgr.gameobject_groups,
        PoolMemberKindLikeCpp::Pool => {
            unreachable!("pool rows use apply_pool_pool_member_row_like_cpp")
        }
    };
    let group = group_map
        .entry(row.pool_spawn_id)
        .or_insert_with(|| PoolGroupLikeCpp::with_pool_id(kind, row.pool_spawn_id));
    group.set_pool_id_like_cpp(row.pool_spawn_id);
    group.add_entry_like_cpp(PoolObjectLikeCpp::new(row.spawn_id, row.chance), max_limit);
    let spawn_id = row.spawn_id;
    let _ = mgr.register_spawn_pool_relation_like_cpp(kind, spawn_id, row.pool_spawn_id);
    member_report.loaded += 1;
}

fn apply_pool_pool_member_row_like_cpp(
    row: PoolMemberRowLikeCpp,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    report.pool_members.rows += 1;
    let Ok(child_pool_id) = u32::try_from(row.spawn_id) else {
        report.pool_members.skipped_child_id_overflow += 1;
        return;
    };
    if !mgr.templates.contains_key(&row.pool_spawn_id) {
        report.pool_members.skipped_missing_template += 1;
        return;
    }
    if !mgr.templates.contains_key(&child_pool_id) {
        report.pool_members.skipped_missing_spawn += 1;
        return;
    }
    if row.pool_spawn_id == child_pool_id {
        report.circular_relations += 1;
        report.pool_members.skipped_missing_spawn += 1;
        return;
    }
    if !(0.0..=100.0).contains(&row.chance) {
        report.pool_members.skipped_invalid_chance += 1;
        return;
    }

    let max_limit = mgr
        .templates
        .get(&row.pool_spawn_id)
        .map(|template| template.max_limit)
        .unwrap_or(0);
    let group = mgr.pool_groups.entry(row.pool_spawn_id).or_insert_with(|| {
        PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Pool, row.pool_spawn_id)
    });
    group.set_pool_id_like_cpp(row.pool_spawn_id);
    group.add_entry_like_cpp(
        PoolObjectLikeCpp::new(u64::from(child_pool_id), row.chance),
        max_limit,
    );
    let _ = mgr.register_child_pool_relation_like_cpp(u64::from(child_pool_id), row.pool_spawn_id);
    report.pool_members.loaded += 1;
}

fn apply_pool_map_propagation_like_cpp(
    mgr: &mut PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    let pool_ids = mgr.templates.keys().copied().collect::<Vec<_>>();
    for pool_id in pool_ids {
        let mut checked = std::collections::HashSet::new();
        let mut current = pool_id;
        while let Some(parent) = mgr.child_pool_to_parent.get(&current).copied() {
            let child_map_id = mgr
                .templates
                .get(&current)
                .map_or(-1, |template| template.map_id);
            if child_map_id != -1 {
                if let Some(parent_template) = mgr.templates.get_mut(&parent) {
                    if parent_template.map_id == -1 {
                        parent_template.map_id = child_map_id;
                    }
                    if parent_template.map_id != child_map_id {
                        mgr.remove_child_pool_relation_like_cpp(current, parent);
                        report.map_mismatches += 1;
                        report.relation_removals += 1;
                        report.pool_members.loaded = report.pool_members.loaded.saturating_sub(1);
                        break;
                    }
                }
            }

            checked.insert(current);
            if checked.contains(&parent) {
                mgr.remove_child_pool_relation_like_cpp(current, parent);
                report.circular_relations += 1;
                report.relation_removals += 1;
                report.pool_members.loaded = report.pool_members.loaded.saturating_sub(1);
                break;
            }
            current = parent;
        }
    }
}

fn apply_pool_final_validation_like_cpp(
    mgr: &PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    for (&pool_id, template) in &mgr.templates {
        if mgr.is_empty_like_cpp(pool_id) {
            report.empty_pools += 1;
        } else if template.map_id == -1 {
            report.missing_map_after_non_empty += 1;
        }
    }
}

fn apply_pool_autospawn_candidate_row_like_cpp(
    row: PoolAutospawnCandidateRowLikeCpp,
    mgr: &mut PoolMgrLikeCpp,
    report: &mut PoolMgrLoadReportLikeCpp,
) {
    report.autospawn_rows += 1;
    if mgr.is_empty_like_cpp(row.pool_entry) {
        report.autospawn_skipped_empty += 1;
        return;
    }
    if !mgr.check_pool_like_cpp(row.pool_entry) {
        report.autospawn_skipped_broken += 1;
        return;
    }
    if row.child_pool_id != 0 {
        let _mother_pool_id = row.mother_pool_id;
        report.autospawn_skipped_child += 1;
        return;
    }
    if let Some(template) = mgr.templates.get(&row.pool_entry) {
        mgr.add_auto_spawn_pool_like_cpp(template.map_id, row.pool_entry);
        report.autospawn_loaded += 1;
    }
}

pub fn spawn_group_templates_for_spawn_store(
    store: &wow_data::SpawnGroupTemplateStore,
) -> BTreeMap<u32, SpawnGroupTemplateData> {
    let mut templates = BTreeMap::new();
    for template in store.iter() {
        let map_id = match template.group_id {
            0 | 1 => 0,
            _ => SPAWNGROUP_MAP_UNSET,
        };
        templates.insert(
            template.group_id,
            SpawnGroupTemplateData {
                group_id: template.group_id,
                name: template.name.clone(),
                map_id,
                flags: SpawnGroupFlags(template.flags),
            },
        );
    }

    templates
        .entry(0)
        .or_insert_with(SpawnGroupTemplateData::default_group);
    templates
        .entry(1)
        .or_insert_with(SpawnGroupTemplateData::legacy_group);
    templates
}

async fn load_creature_spawns_like_cpp(
    db: &WorldDatabase,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    store: &mut SpawnStore,
    creature_runtime_rows: &mut BTreeMap<SpawnId, CreatureSpawnRuntimeRowLikeCpp>,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<()> {
    let stmt = db.prepare(WorldStatements::SEL_CREATURE_SPAWNS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(());
    }

    loop {
        let row = CreatureSpawnRow {
            spawn_id: result.read(0),
            entry: result.read(1),
            map_id: result.read(2),
            x: result.read(3),
            y: result.read(4),
            z: result.read(5),
            orientation: result.read(6),
            model_id: result.try_read(7).unwrap_or(0),
            equipment_id: result.try_read(8).unwrap_or(0),
            spawn_time_secs: result.read(9),
            wander_distance: result.try_read(10).unwrap_or(0.0),
            curhealth: result.try_read(12).unwrap_or(0),
            curmana: result.try_read(13).unwrap_or(0),
            movement_type: result.try_read(14).unwrap_or(0),
            spawn_difficulties: result.read(15),
            event_entry: result.try_read(16).unwrap_or(0),
            pool_id: result.try_read(17).unwrap_or(0),
            phase_use_flags: result.read(22),
            phase_id: result.read(23),
            phase_group: result.read(24),
            terrain_swap_map: result.read(25),
            script_name: result.try_read(26).unwrap_or_default(),
            string_id: result.try_read(27).unwrap_or_default(),
        };
        let runtime_row = creature_row_to_runtime_row_like_cpp(&row);
        report.creature.rows += 1;
        if let Some(spawn) = creature_row_to_spawn_data_like_cpp(
            &row,
            map_store,
            map_difficulty_store,
            &mut report.creature,
        ) {
            if row.event_entry != 0 {
                store.insert_spawn_metadata_like_cpp(&spawn);
                creature_runtime_rows.insert(row.spawn_id, runtime_row.clone());
                report.creature.skipped_event += 1;
            } else {
                store.add_object_spawn(&spawn, is_personal_phase_like_cpp_represented);
                creature_runtime_rows.insert(row.spawn_id, runtime_row.clone());
                report.creature.indexed += 1;
            }
        }

        if !result.next_row() {
            break;
        }
    }

    Ok(())
}

async fn load_gameobject_spawns_like_cpp(
    db: &WorldDatabase,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    store: &mut SpawnStore,
    gameobject_runtime_rows: &mut BTreeMap<SpawnId, GameObjectSpawnRuntimeRowLikeCpp>,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<()> {
    let stmt = db.prepare(WorldStatements::SEL_GAMEOBJECT_SPAWNS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(());
    }

    loop {
        let row = GameObjectSpawnRow {
            spawn_id: result.read(0),
            entry: result.read(1),
            map_id: result.read(2),
            x: result.read(3),
            y: result.read(4),
            z: result.read(5),
            orientation: result.read(6),
            rotation: [
                result.read(7),
                result.read(8),
                result.read(9),
                result.read(10),
            ],
            spawn_time_secs: result.read(11),
            anim_progress: result.read(12),
            state: result.read(13),
            spawn_difficulties: result.read(14),
            event_entry: result.try_read(15).unwrap_or(0),
            pool_id: result.try_read(16).unwrap_or(0),
            phase_use_flags: result.read(17),
            phase_id: result.read(18),
            phase_group: result.read(19),
            terrain_swap_map: result.read(20),
            script_name: result.try_read(21).unwrap_or_default(),
            string_id: result.try_read(22).unwrap_or_default(),
        };
        report.gameobject.rows += 1;
        let runtime_row = gameobject_row_to_runtime_row_like_cpp(&row);
        if let Some(spawn) = gameobject_row_to_spawn_data_like_cpp(
            &row,
            map_store,
            map_difficulty_store,
            &mut report.gameobject,
        ) {
            if row.event_entry != 0 {
                store.insert_spawn_metadata_like_cpp(&spawn);
                gameobject_runtime_rows.insert(row.spawn_id, runtime_row.clone());
                report.gameobject.skipped_event += 1;
            } else {
                store.add_object_spawn(&spawn, is_personal_phase_like_cpp_represented);
                gameobject_runtime_rows.insert(row.spawn_id, runtime_row.clone());
                report.gameobject.indexed += 1;
            }
        }

        if !result.next_row() {
            break;
        }
    }

    Ok(())
}

async fn load_area_trigger_spawns_like_cpp(
    db: &WorldDatabase,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    store: &mut SpawnStore,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<()> {
    let stmt = db.prepare(WorldStatements::SEL_AREATRIGGER_SPAWNS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(());
    }

    loop {
        let row = AreaTriggerSpawnRow {
            spawn_id: result.read(0),
            create_properties_id: result.read(1),
            map_id: result.read(3),
            spawn_difficulties: result.read(4),
            x: result.read(5),
            y: result.read(6),
            z: result.read(7),
            orientation: result.read(8),
            phase_use_flags: result.read(9),
            phase_id: result.read(10),
            phase_group: result.read(11),
            script_name: result.try_read(13).unwrap_or_default(),
        };
        report.area_trigger.rows += 1;
        if let Some(spawn) = area_trigger_row_to_spawn_data_like_cpp(
            &row,
            map_store,
            map_difficulty_store,
            &mut report.area_trigger,
        ) {
            store.add_area_trigger_spawn(&spawn);
            report.area_trigger.indexed += 1;
        }

        if !result.next_row() {
            break;
        }
    }

    Ok(())
}

async fn load_linked_respawns_like_cpp(
    db: &WorldDatabase,
    store: &SpawnStore,
    map_store: &wow_data::MapStore,
    report: &mut CanonicalSpawnStoreLoadReport,
) -> Result<LinkedRespawnStoreLikeCpp> {
    let stmt = db.prepare(WorldStatements::SEL_LINKED_RESPAWNS);
    let mut result = db.query(&stmt).await?;
    let mut linked_store = LinkedRespawnStoreLikeCpp::new();
    if result.is_empty() {
        return Ok(linked_store);
    }

    loop {
        let row = LinkedRespawnDbRow {
            guid: result.read(0),
            linked_guid: result.read(1),
            link_type: result.read(2),
        };
        apply_linked_respawn_row_like_cpp(
            row.into(),
            store,
            map_store,
            &mut linked_store,
            &mut report.linked_respawn,
        );

        if !result.next_row() {
            break;
        }
    }

    Ok(linked_store)
}

fn apply_linked_respawn_row_like_cpp(
    row: LinkedRespawnRowLikeCpp,
    store: &SpawnStore,
    map_store: &wow_data::MapStore,
    linked_store: &mut LinkedRespawnStoreLikeCpp,
    report: &mut LinkedRespawnLoadReportLikeCpp,
) {
    report.rows += 1;
    let Some(link_type) = LinkedRespawnTypeLikeCpp::from_raw(row.link_type) else {
        report.push(LinkedRespawnLoadIssueLikeCpp {
            kind: LinkedRespawnLoadIssueKindLikeCpp::InvalidType,
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
            slave_type: None,
            master_type: None,
            slave_map_id: None,
            master_map_id: None,
        });
        return;
    };

    let slave_type = link_type.slave_type();
    let master_type = link_type.master_type();
    let Some(slave) = store.spawn_data(slave_type, row.guid) else {
        report.push(LinkedRespawnLoadIssueLikeCpp {
            kind: LinkedRespawnLoadIssueKindLikeCpp::MissingSlave,
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
            slave_type: Some(slave_type),
            master_type: Some(master_type),
            slave_map_id: None,
            master_map_id: None,
        });
        return;
    };
    let Some(master) = store.spawn_data(master_type, row.linked_guid) else {
        report.push(LinkedRespawnLoadIssueLikeCpp {
            kind: LinkedRespawnLoadIssueKindLikeCpp::MissingMaster,
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
            slave_type: Some(slave_type),
            master_type: Some(master_type),
            slave_map_id: Some(slave.map_id),
            master_map_id: None,
        });
        return;
    };

    if map_store
        .get(master.map_id)
        .is_none_or(|map| !map_entry_instanceable_like_cpp(*map))
        || master.map_id != slave.map_id
    {
        report.push(LinkedRespawnLoadIssueLikeCpp {
            kind: LinkedRespawnLoadIssueKindLikeCpp::NotInstanceableOrMapMismatch,
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
            slave_type: Some(slave_type),
            master_type: Some(master_type),
            slave_map_id: Some(slave.map_id),
            master_map_id: Some(master.map_id),
        });
        return;
    }

    if !spawn_difficulties_intersect_like_cpp(slave, master) {
        report.push(LinkedRespawnLoadIssueLikeCpp {
            kind: LinkedRespawnLoadIssueKindLikeCpp::DifficultyMismatch,
            guid: row.guid,
            linked_guid: row.linked_guid,
            link_type: row.link_type,
            slave_type: Some(slave_type),
            master_type: Some(master_type),
            slave_map_id: Some(slave.map_id),
            master_map_id: Some(master.map_id),
        });
        return;
    }

    linked_store.insert_like_cpp(
        spawn_data_guid_like_cpp(slave),
        spawn_data_guid_like_cpp(master),
    );
    report.inserted += 1;
}

fn spawn_difficulties_intersect_like_cpp(left: &SpawnData, right: &SpawnData) -> bool {
    left.spawn_difficulties
        .iter()
        .any(|difficulty| right.spawn_difficulties.contains(difficulty))
}

fn spawn_data_guid_like_cpp(spawn: &SpawnData) -> ObjectGuid {
    let high = match spawn.object_type {
        SpawnObjectType::Creature => HighGuid::Creature,
        SpawnObjectType::GameObject => HighGuid::GameObject,
        SpawnObjectType::AreaTrigger => HighGuid::AreaTrigger,
    };
    ObjectGuid::create_world_object(
        high,
        0,
        0,
        spawn.map_id as u16,
        0,
        spawn.id,
        spawn.spawn_id as i64,
    )
}

fn map_entry_instanceable_like_cpp(map: wow_data::MapEntry) -> bool {
    matches!(
        map.instance_type,
        wow_data::map::MAP_INSTANCE
            | wow_data::map::MAP_RAID
            | wow_data::map::MAP_BATTLEGROUND
            | wow_data::map::MAP_ARENA
            | wow_data::map::MAP_SCENARIO
    )
}

async fn load_spawn_group_members_like_cpp(db: &WorldDatabase) -> Result<Vec<SpawnGroupMemberRow>> {
    let stmt = db.prepare(WorldStatements::SEL_SPAWN_GROUP_MEMBERS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(Vec::new());
    }

    let mut rows = Vec::new();
    loop {
        rows.push(SpawnGroupMemberRow {
            group_id: result.read(0),
            spawn_type: result.read(1),
            spawn_id: result.read(2),
        });
        if !result.next_row() {
            break;
        }
    }

    Ok(rows)
}

fn creature_row_to_spawn_data_like_cpp(
    row: &CreatureSpawnRow,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    report: &mut SpawnKindLoadReport,
) -> Option<SpawnData> {
    object_row_to_spawn_data_like_cpp(
        SpawnObjectType::Creature,
        row.spawn_id,
        row.entry,
        row.map_id,
        row.x,
        row.y,
        row.z,
        row.orientation,
        row.spawn_time_secs,
        &row.spawn_difficulties,
        row.pool_id,
        row.phase_use_flags,
        row.phase_id,
        row.phase_group,
        row.terrain_swap_map,
        &row.script_name,
        &row.string_id,
        map_store,
        map_difficulty_store,
        report,
    )
}

fn creature_row_to_runtime_row_like_cpp(row: &CreatureSpawnRow) -> CreatureSpawnRuntimeRowLikeCpp {
    CreatureSpawnRuntimeRowLikeCpp {
        spawn_id: row.spawn_id,
        model_id: row.model_id,
        equipment_id: row.equipment_id,
        wander_distance: row.wander_distance,
        curhealth: row.curhealth,
        curmana: row.curmana,
        movement_type: row.movement_type,
        string_id: row.string_id.clone(),
        spawn_time_secs: row.spawn_time_secs,
    }
}

fn gameobject_row_to_runtime_row_like_cpp(
    row: &GameObjectSpawnRow,
) -> GameObjectSpawnRuntimeRowLikeCpp {
    GameObjectSpawnRuntimeRowLikeCpp {
        spawn_id: row.spawn_id,
        rotation: row.rotation,
        anim_progress: row.anim_progress,
        state: row.state,
        string_id: row.string_id.clone(),
        spawn_time_secs: row.spawn_time_secs,
    }
}

fn gameobject_row_to_spawn_data_like_cpp(
    row: &GameObjectSpawnRow,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    report: &mut SpawnKindLoadReport,
) -> Option<SpawnData> {
    object_row_to_spawn_data_like_cpp(
        SpawnObjectType::GameObject,
        row.spawn_id,
        row.entry,
        row.map_id,
        row.x,
        row.y,
        row.z,
        row.orientation,
        row.spawn_time_secs,
        &row.spawn_difficulties,
        row.pool_id,
        row.phase_use_flags,
        row.phase_id,
        row.phase_group,
        row.terrain_swap_map,
        &row.script_name,
        &row.string_id,
        map_store,
        map_difficulty_store,
        report,
    )
}

#[allow(clippy::too_many_arguments)]
fn object_row_to_spawn_data_like_cpp(
    object_type: SpawnObjectType,
    spawn_id: SpawnId,
    entry: u32,
    map_id: u32,
    x: f32,
    y: f32,
    z: f32,
    orientation: f32,
    spawn_time_secs: i32,
    spawn_difficulties: &str,
    pool_id: u32,
    phase_use_flags: u8,
    phase_id: u32,
    phase_group: u32,
    terrain_swap_map: i32,
    script_name: &str,
    string_id: &str,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    report: &mut SpawnKindLoadReport,
) -> Option<SpawnData> {
    if map_store.get(map_id).is_none() {
        report.skipped_missing_map += 1;
        return None;
    }
    if !is_valid_map_coord_like_cpp(x, y, z, orientation) {
        report.skipped_invalid_position += 1;
        return None;
    }

    let is_transport = is_transport_map_like_cpp_represented(map_id);
    let parsed = parse_spawn_difficulties_like_cpp(
        spawn_difficulties,
        map_id,
        is_transport,
        map_difficulty_store,
    );
    if parsed.difficulties.is_empty() {
        report.skipped_empty_difficulties += 1;
        return None;
    }

    report.validation_skipped += 1;
    if !script_name.is_empty() {
        report.script_id_unresolved += 1;
    }

    Some(SpawnData {
        object_type,
        spawn_id,
        map_id,
        db_data: true,
        spawn_group: default_spawn_group_like_cpp(is_transport),
        id: entry,
        spawn_point: SpawnPosition::new(x, y, z, orientation),
        phase_use_flags,
        phase_id,
        phase_group,
        terrain_swap_map,
        pool_id,
        spawn_time_secs,
        spawn_difficulties: parsed.difficulties,
        script_id: 0,
        string_id: string_id.to_string(),
    })
}

fn area_trigger_row_to_spawn_data_like_cpp(
    row: &AreaTriggerSpawnRow,
    map_store: &wow_data::MapStore,
    map_difficulty_store: &wow_data::MapDifficultyStore,
    report: &mut SpawnKindLoadReport,
) -> Option<SpawnData> {
    if map_store.get(row.map_id).is_none() {
        report.skipped_missing_map += 1;
        return None;
    }
    if !is_valid_map_coord_like_cpp(row.x, row.y, row.z, row.orientation) {
        report.skipped_invalid_position += 1;
        return None;
    }

    let parsed = parse_spawn_difficulties_like_cpp(
        &row.spawn_difficulties,
        row.map_id,
        is_transport_map_like_cpp_represented(row.map_id),
        map_difficulty_store,
    );
    if parsed.difficulties.is_empty() {
        report.skipped_empty_difficulties += 1;
        return None;
    }

    report.validation_skipped += 1;
    if !row.script_name.is_empty() {
        report.script_id_unresolved += 1;
    }

    Some(SpawnData {
        object_type: SpawnObjectType::AreaTrigger,
        spawn_id: row.spawn_id,
        map_id: row.map_id,
        db_data: true,
        spawn_group: SpawnGroupTemplateData::legacy_group(),
        id: row.create_properties_id,
        spawn_point: SpawnPosition::new(row.x, row.y, row.z, row.orientation),
        phase_use_flags: row.phase_use_flags,
        phase_id: row.phase_id,
        phase_group: row.phase_group,
        terrain_swap_map: -1,
        pool_id: 0,
        spawn_time_secs: 0,
        spawn_difficulties: parsed.difficulties,
        script_id: 0,
        string_id: String::new(),
    })
}

fn parse_spawn_difficulties_like_cpp(
    difficulty_string: &str,
    map_id: u32,
    is_transport_map: bool,
    map_difficulty_store: &wow_data::MapDifficultyStore,
) -> ParsedSpawnDifficulties {
    let mut difficulties = Vec::new();
    let mut report = SpawnDifficultyParseReport {
        invalid_tokens_as_none: 0,
        unsupported: Vec::new(),
    };

    for token in difficulty_string
        .split(',')
        .filter(|token| !token.is_empty())
    {
        let difficulty = match token.parse::<Difficulty>() {
            Ok(difficulty) => difficulty,
            Err(_) => {
                report.invalid_tokens_as_none += 1;
                DIFFICULTY_NONE_LIKE_CPP
            }
        };

        if !is_transport_map && map_difficulty_store.get(map_id, difficulty).is_none() {
            report.unsupported.push(difficulty);
            continue;
        }

        difficulties.push(difficulty);
    }

    difficulties.sort_unstable();
    ParsedSpawnDifficulties {
        difficulties,
        report,
    }
}

fn default_spawn_group_like_cpp(is_transport_map: bool) -> SpawnGroupTemplateData {
    if is_transport_map {
        SpawnGroupTemplateData::legacy_group()
    } else {
        SpawnGroupTemplateData::default_group()
    }
}

fn is_valid_map_coord_like_cpp(x: f32, y: f32, z: f32, orientation: f32) -> bool {
    Position::new(x, y, z, orientation).is_valid_map_coord_like_cpp()
}

fn is_personal_phase_like_cpp_represented(phase_id: u32) -> bool {
    // C++ checks `PhaseEntryFlags::Personal` via `PhasingHandler::IsPersonalPhase`.
    // Phase DB2 flag lookup is not available in this metadata-only loader yet, so
    // this keeps the predicate isolated and intentionally conservative.
    phase_id & PERSONAL_PHASE_FLAG_LIKE_CPP != 0
}

fn is_transport_map_like_cpp_represented(map_id: u32) -> bool {
    // C++ `ObjectMgr::_transportMaps` is populated while validating
    // GAMEOBJECT_TYPE_MAP_OBJ_TRANSPORT/GARRISON_BUILDING templates. RustyCore
    // has no canonical transport-map store yet; keep the fallback explicit so a
    // later transport-template slice can replace only this predicate.
    TRANSPORT_MAP_IDS_REPRESENTED.contains(&map_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_store(ids: &[u32]) -> wow_data::MapStore {
        wow_data::MapStore::from_entries(ids.iter().copied().map(|id| wow_data::MapEntry {
            id,
            instance_type: 0,
            parent_map_id: -1,
            cosmetic_parent_map_id: -1,
            flags1: 0,
        }))
    }

    fn instanceable_map_store(ids: &[u32]) -> wow_data::MapStore {
        wow_data::MapStore::from_entries(ids.iter().copied().map(|id| wow_data::MapEntry {
            id,
            instance_type: wow_data::map::MAP_INSTANCE,
            parent_map_id: -1,
            cosmetic_parent_map_id: -1,
            flags1: 0,
        }))
    }

    fn map_difficulty_store(entries: &[(u32, Difficulty)]) -> wow_data::MapDifficultyStore {
        wow_data::MapDifficultyStore::from_entries(entries.iter().enumerate().map(
            |(idx, (map_id, difficulty_id))| wow_data::MapDifficultyEntry {
                id: u32::try_from(idx + 1).unwrap_or(u32::MAX),
                map_id: *map_id,
                difficulty_id: *difficulty_id,
                lock_id: 0,
                reset_interval: 0,
                flags: 0,
            },
        ))
    }

    fn creature_row(spawn_id: SpawnId, event_entry: i16, difficulties: &str) -> CreatureSpawnRow {
        CreatureSpawnRow {
            spawn_id,
            entry: 123,
            map_id: 1,
            x: 10.0,
            y: 20.0,
            z: 30.0,
            orientation: 1.0,
            spawn_time_secs: 300,
            model_id: 0,
            equipment_id: 0,
            wander_distance: 0.0,
            curhealth: 0,
            curmana: 0,
            movement_type: 0,
            spawn_difficulties: difficulties.to_string(),
            event_entry,
            pool_id: 0,
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: -1,
            script_name: String::new(),
            string_id: String::new(),
        }
    }

    fn gameobject_row(
        spawn_id: SpawnId,
        event_entry: i16,
        difficulties: &str,
    ) -> GameObjectSpawnRow {
        GameObjectSpawnRow {
            spawn_id,
            entry: 456,
            map_id: 1,
            x: 11.0,
            y: 21.0,
            z: 31.0,
            orientation: 1.0,
            rotation: [0.0, 0.0, 0.0, 1.0],
            spawn_time_secs: 300,
            anim_progress: 100,
            state: 1,
            spawn_difficulties: difficulties.to_string(),
            event_entry,
            pool_id: 0,
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            terrain_swap_map: -1,
            script_name: String::new(),
            string_id: String::new(),
        }
    }

    fn area_trigger_row(spawn_id: SpawnId, difficulties: &str) -> AreaTriggerSpawnRow {
        AreaTriggerSpawnRow {
            spawn_id,
            create_properties_id: 789,
            map_id: 1,
            spawn_difficulties: difficulties.to_string(),
            x: 12.0,
            y: 22.0,
            z: 32.0,
            orientation: 1.0,
            phase_use_flags: 0,
            phase_id: 0,
            phase_group: 0,
            script_name: String::new(),
        }
    }

    #[test]
    fn pool_mgr_loader_skip_order_missing_spawn_before_template_and_chance_like_cpp() {
        let maps = map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut spawn_report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();
        let spawn = creature_row_to_spawn_data_like_cpp(
            &creature_row(100, 0, "0"),
            &maps,
            &difficulties,
            &mut spawn_report,
        )
        .unwrap();
        store.add_object_spawn(&spawn, is_personal_phase_like_cpp_represented);
        let mut mgr = PoolMgrLikeCpp::new();
        let mut report = PoolMgrLoadReportLikeCpp::default();

        apply_pool_spawn_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 999,
                pool_spawn_id: 88,
                chance: 200.0,
            },
            &store,
            PoolMemberKindLikeCpp::Creature,
            &mut mgr,
            &mut report,
        );
        apply_pool_spawn_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 100,
                pool_spawn_id: 88,
                chance: 200.0,
            },
            &store,
            PoolMemberKindLikeCpp::Creature,
            &mut mgr,
            &mut report,
        );
        mgr.insert_template_like_cpp(88, PoolTemplateDataLikeCpp::new(1, -1));
        apply_pool_spawn_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 100,
                pool_spawn_id: 88,
                chance: 200.0,
            },
            &store,
            PoolMemberKindLikeCpp::Creature,
            &mut mgr,
            &mut report,
        );

        assert_eq!(report.creature_members.rows, 3);
        assert_eq!(report.creature_members.skipped_missing_spawn, 1);
        assert_eq!(report.creature_members.skipped_missing_template, 1);
        assert_eq!(report.creature_members.skipped_invalid_chance, 1);
        assert_eq!(report.creature_members.loaded, 0);
    }

    #[test]
    fn pool_mgr_loader_map_propagation_mismatch_and_cycle_removal_like_cpp() {
        let mut propagated = PoolMgrLikeCpp::new();
        let mut report = PoolMgrLoadReportLikeCpp::default();
        propagated.insert_template_like_cpp(1, PoolTemplateDataLikeCpp::new(1, 571));
        propagated.insert_template_like_cpp(2, PoolTemplateDataLikeCpp::new(1, -1));
        apply_pool_pool_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 1,
                pool_spawn_id: 2,
                chance: 0.0,
            },
            &mut propagated,
            &mut report,
        );
        apply_pool_map_propagation_like_cpp(&mut propagated, &mut report);
        assert_eq!(propagated.templates.get(&2).unwrap().map_id, 571);
        assert_eq!(report.relation_removals, 0);

        let mut mismatch = PoolMgrLikeCpp::new();
        let mut mismatch_report = PoolMgrLoadReportLikeCpp::default();
        mismatch.insert_template_like_cpp(10, PoolTemplateDataLikeCpp::new(1, 1));
        mismatch.insert_template_like_cpp(20, PoolTemplateDataLikeCpp::new(1, 2));
        apply_pool_pool_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 10,
                pool_spawn_id: 20,
                chance: 0.0,
            },
            &mut mismatch,
            &mut mismatch_report,
        );
        apply_pool_map_propagation_like_cpp(&mut mismatch, &mut mismatch_report);
        assert!(!mismatch.child_pool_to_parent.contains_key(&10));
        assert_eq!(mismatch_report.map_mismatches, 1);
        assert_eq!(mismatch_report.relation_removals, 1);

        let mut cyclic = PoolMgrLikeCpp::new();
        let mut cycle_report = PoolMgrLoadReportLikeCpp::default();
        cyclic.insert_template_like_cpp(30, PoolTemplateDataLikeCpp::new(1, -1));
        cyclic.insert_template_like_cpp(31, PoolTemplateDataLikeCpp::new(1, -1));
        apply_pool_pool_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 31,
                pool_spawn_id: 30,
                chance: 0.0,
            },
            &mut cyclic,
            &mut cycle_report,
        );
        apply_pool_pool_member_row_like_cpp(
            PoolMemberRowLikeCpp {
                spawn_id: 30,
                pool_spawn_id: 31,
                chance: 0.0,
            },
            &mut cyclic,
            &mut cycle_report,
        );
        apply_pool_map_propagation_like_cpp(&mut cyclic, &mut cycle_report);
        assert_eq!(cycle_report.circular_relations, 1);
        assert_eq!(cycle_report.relation_removals, 1);
        assert_eq!(cyclic.child_pool_to_parent.len(), 1);
    }

    #[test]
    fn pool_mgr_loader_autospawn_skips_empty_broken_and_child_like_cpp() {
        let mut mgr = PoolMgrLikeCpp::new();
        let mut report = PoolMgrLoadReportLikeCpp::default();
        mgr.insert_template_like_cpp(1, PoolTemplateDataLikeCpp::new(1, 0));
        mgr.insert_template_like_cpp(2, PoolTemplateDataLikeCpp::new(1, 0));
        mgr.insert_template_like_cpp(3, PoolTemplateDataLikeCpp::new(1, 0));
        mgr.insert_template_like_cpp(4, PoolTemplateDataLikeCpp::new(1, 0));
        let mut valid = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 1);
        valid.add_entry_like_cpp(PoolObjectLikeCpp::new(101, 0.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 1, valid)
            .unwrap();
        let mut broken = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 3);
        broken.add_entry_like_cpp(PoolObjectLikeCpp::new(301, 50.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 3, broken)
            .unwrap();
        let mut child = PoolGroupLikeCpp::with_pool_id(PoolMemberKindLikeCpp::Creature, 4);
        child.add_entry_like_cpp(PoolObjectLikeCpp::new(401, 0.0), 1);
        mgr.insert_or_replace_group_like_cpp(PoolMemberKindLikeCpp::Creature, 4, child)
            .unwrap();

        apply_pool_autospawn_candidate_row_like_cpp(
            PoolAutospawnCandidateRowLikeCpp {
                pool_entry: 1,
                child_pool_id: 0,
                mother_pool_id: 0,
            },
            &mut mgr,
            &mut report,
        );
        apply_pool_autospawn_candidate_row_like_cpp(
            PoolAutospawnCandidateRowLikeCpp {
                pool_entry: 2,
                child_pool_id: 0,
                mother_pool_id: 0,
            },
            &mut mgr,
            &mut report,
        );
        apply_pool_autospawn_candidate_row_like_cpp(
            PoolAutospawnCandidateRowLikeCpp {
                pool_entry: 3,
                child_pool_id: 0,
                mother_pool_id: 0,
            },
            &mut mgr,
            &mut report,
        );
        apply_pool_autospawn_candidate_row_like_cpp(
            PoolAutospawnCandidateRowLikeCpp {
                pool_entry: 4,
                child_pool_id: 4,
                mother_pool_id: 99,
            },
            &mut mgr,
            &mut report,
        );

        assert_eq!(report.autospawn_rows, 4);
        assert_eq!(report.autospawn_loaded, 1);
        assert_eq!(report.autospawn_skipped_empty, 1);
        assert_eq!(report.autospawn_skipped_broken, 1);
        assert_eq!(report.autospawn_skipped_child, 1);
        assert_eq!(mgr.auto_spawn_pools_for_map_like_cpp(0), &[1]);
    }

    #[test]
    fn linked_respawn_loader_validation_invalid_type_and_missing_master_like_cpp() {
        let maps = instanceable_map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut kind_report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();
        let creature = creature_row_to_spawn_data_like_cpp(
            &creature_row(100, 0, "0"),
            &maps,
            &difficulties,
            &mut kind_report,
        )
        .unwrap();
        store.add_object_spawn(&creature, is_personal_phase_like_cpp_represented);
        let mut linked_store = LinkedRespawnStoreLikeCpp::new();
        let mut report = LinkedRespawnLoadReportLikeCpp::default();

        apply_linked_respawn_row_like_cpp(
            LinkedRespawnRowLikeCpp {
                guid: 100,
                linked_guid: 200,
                link_type: 99,
            },
            &store,
            &maps,
            &mut linked_store,
            &mut report,
        );
        apply_linked_respawn_row_like_cpp(
            LinkedRespawnRowLikeCpp {
                guid: 100,
                linked_guid: 200,
                link_type: LinkedRespawnTypeLikeCpp::CreatureToCreature as u8,
            },
            &store,
            &maps,
            &mut linked_store,
            &mut report,
        );

        assert_eq!(report.rows, 2);
        assert_eq!(report.invalid_type, 1);
        assert_eq!(report.missing_master, 1);
        assert!(linked_store.is_empty());
    }

    #[test]
    fn linked_respawn_loader_validation_difficulty_mismatch_like_cpp() {
        let maps = instanceable_map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0), (1, 1)]);
        let mut kind_report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();
        let slave = creature_row_to_spawn_data_like_cpp(
            &creature_row(100, 0, "0"),
            &maps,
            &difficulties,
            &mut kind_report,
        )
        .unwrap();
        let master = creature_row_to_spawn_data_like_cpp(
            &creature_row(200, 0, "1"),
            &maps,
            &difficulties,
            &mut kind_report,
        )
        .unwrap();
        store.add_object_spawn(&slave, is_personal_phase_like_cpp_represented);
        store.add_object_spawn(&master, is_personal_phase_like_cpp_represented);
        let mut linked_store = LinkedRespawnStoreLikeCpp::new();
        let mut report = LinkedRespawnLoadReportLikeCpp::default();

        apply_linked_respawn_row_like_cpp(
            LinkedRespawnRowLikeCpp {
                guid: 100,
                linked_guid: 200,
                link_type: LinkedRespawnTypeLikeCpp::CreatureToCreature as u8,
            },
            &store,
            &maps,
            &mut linked_store,
            &mut report,
        );

        assert_eq!(report.difficulty_mismatch, 1);
        assert!(linked_store.is_empty());
    }

    #[test]
    fn linked_respawn_loader_validation_valid_creature_to_gameobject_inserts_like_cpp() {
        let maps = instanceable_map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut kind_report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();
        let slave = creature_row_to_spawn_data_like_cpp(
            &creature_row(100, 0, "0"),
            &maps,
            &difficulties,
            &mut kind_report,
        )
        .unwrap();
        let master = gameobject_row_to_spawn_data_like_cpp(
            &gameobject_row(200, 0, "0"),
            &maps,
            &difficulties,
            &mut kind_report,
        )
        .unwrap();
        store.add_object_spawn(&slave, is_personal_phase_like_cpp_represented);
        store.add_object_spawn(&master, is_personal_phase_like_cpp_represented);
        let mut linked_store = LinkedRespawnStoreLikeCpp::new();
        let mut report = LinkedRespawnLoadReportLikeCpp::default();

        apply_linked_respawn_row_like_cpp(
            LinkedRespawnRowLikeCpp {
                guid: 100,
                linked_guid: 200,
                link_type: LinkedRespawnTypeLikeCpp::CreatureToGameObject as u8,
            },
            &store,
            &maps,
            &mut linked_store,
            &mut report,
        );

        assert_eq!(report.inserted, 1);
        assert_eq!(linked_store.len(), 1);
        let slave_guid = spawn_data_guid_like_cpp(&slave);
        let master_guid = spawn_data_guid_like_cpp(&master);
        assert_eq!(
            linked_store.get_linked_respawn_guid_like_cpp(slave_guid),
            master_guid
        );
    }

    #[test]
    fn spawn_difficulty_parser_matches_cpp_token_rules() {
        let difficulties = map_difficulty_store(&[(1, 0), (1, 1)]);
        let parsed = parse_spawn_difficulties_like_cpp("0,1", 1, false, &difficulties);
        assert_eq!(parsed.difficulties, vec![0, 1]);
        assert_eq!(parsed.report.invalid_tokens_as_none, 0);
        assert!(parsed.report.unsupported.is_empty());

        let parsed = parse_spawn_difficulties_like_cpp("bad,1", 1, false, &difficulties);
        assert_eq!(parsed.difficulties, vec![0, 1]);
        assert_eq!(parsed.report.invalid_tokens_as_none, 1);

        let parsed = parse_spawn_difficulties_like_cpp("0,2,1", 1, false, &difficulties);
        assert_eq!(parsed.difficulties, vec![0, 1]);
        assert_eq!(parsed.report.unsupported, vec![2]);

        let parsed = parse_spawn_difficulties_like_cpp("2", 1, true, &difficulties);
        assert_eq!(parsed.difficulties, vec![2]);

        let parsed = parse_spawn_difficulties_like_cpp("", 1, false, &difficulties);
        assert!(parsed.difficulties.is_empty());
    }

    #[test]
    fn creature_row_indexes_only_non_event_rows_like_cpp() {
        let maps = map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();

        let indexed = creature_row_to_spawn_data_like_cpp(
            &creature_row(100, 0, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .expect("non-event creature spawn should convert");
        store.add_object_spawn(&indexed, is_personal_phase_like_cpp_represented);

        let event_managed = creature_row_to_spawn_data_like_cpp(
            &creature_row(101, 7, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .expect("event-managed creature spawn metadata should convert");
        store.insert_spawn_metadata_like_cpp(&event_managed);

        assert!(
            store
                .cell_object_guids(1, 0, indexed.cell_id())
                .is_some_and(|cell| cell.creatures.contains(&100))
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 101)
                .map(|spawn| spawn.spawn_id),
            Some(101)
        );
        assert!(
            store
                .cell_object_guids(1, 0, event_managed.cell_id())
                .is_none_or(|cell| !cell.creatures.contains(&101))
        );
    }

    #[test]
    fn row_conversion_skips_missing_map_and_empty_difficulties() {
        let maps = map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut report = SpawnKindLoadReport::default();

        let mut missing_map = creature_row(200, 0, "0");
        missing_map.map_id = 999;
        assert!(
            creature_row_to_spawn_data_like_cpp(&missing_map, &maps, &difficulties, &mut report)
                .is_none()
        );
        assert_eq!(report.skipped_missing_map, 1);

        assert!(
            creature_row_to_spawn_data_like_cpp(
                &creature_row(201, 0, ""),
                &maps,
                &difficulties,
                &mut report,
            )
            .is_none()
        );
        assert_eq!(report.skipped_empty_difficulties, 1);
    }

    #[test]
    fn templates_and_spawn_group_apply_cover_creature_go_at_and_event_gap() {
        let (template_store, _) = wow_data::SpawnGroupTemplateStore::from_rows_like_cpp([
            wow_data::SpawnGroupTemplateRow {
                group_id: 10,
                name: "custom".to_string(),
                flags: 0,
            },
            wow_data::SpawnGroupTemplateRow {
                group_id: 11,
                name: "manual".to_string(),
                flags: wow_data::spawn_group::SPAWN_GROUP_FLAG_MANUAL_SPAWN_LIKE_CPP,
            },
        ]);
        let mut templates = spawn_group_templates_for_spawn_store(&template_store);
        assert_eq!(templates.get(&0).unwrap().map_id, 0);
        assert_eq!(templates.get(&1).unwrap().map_id, 0);
        assert_eq!(templates.get(&10).unwrap().map_id, SPAWNGROUP_MAP_UNSET);

        let maps = map_store(&[1]);
        let difficulties = map_difficulty_store(&[(1, 0)]);
        let mut report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();

        let creature = creature_row_to_spawn_data_like_cpp(
            &creature_row(300, 0, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();
        let go = gameobject_row_to_spawn_data_like_cpp(
            &gameobject_row(301, 0, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();
        let at = area_trigger_row_to_spawn_data_like_cpp(
            &area_trigger_row(302, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();
        let event_managed = gameobject_row_to_spawn_data_like_cpp(
            &gameobject_row(303, 5, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();

        store.add_object_spawn(&creature, is_personal_phase_like_cpp_represented);
        store.add_object_spawn(&go, is_personal_phase_like_cpp_represented);
        store.add_area_trigger_spawn(&at);
        store.insert_spawn_metadata_like_cpp(&event_managed);

        let apply = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: SpawnObjectType::Creature as u8,
                    spawn_id: 300,
                },
                SpawnGroupMemberRow {
                    group_id: 11,
                    spawn_type: SpawnObjectType::GameObject as u8,
                    spawn_id: 301,
                },
                SpawnGroupMemberRow {
                    group_id: 1,
                    spawn_type: SpawnObjectType::AreaTrigger as u8,
                    spawn_id: 302,
                },
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: SpawnObjectType::GameObject as u8,
                    spawn_id: event_managed.spawn_id,
                },
                SpawnGroupMemberRow {
                    group_id: 10,
                    spawn_type: SpawnObjectType::GameObject as u8,
                    spawn_id: 999,
                },
            ],
        );

        assert_eq!(apply.assigned, 3);
        assert_eq!(apply.missing_spawn, 1);
        assert_eq!(apply.duplicate_spawn_group, 1);
        assert_eq!(templates.get(&0).unwrap().map_id, 0);
        assert_eq!(templates.get(&1).unwrap().map_id, 0);
        assert_eq!(templates.get(&10).unwrap().map_id, 1);
        assert_eq!(templates.get(&11).unwrap().map_id, 1);
        assert!(templates.contains_key(&0));
        assert!(templates.contains_key(&1));
        let metadata = CanonicalSpawnMetadataLikeCpp::new(store.clone(), templates.clone());
        assert_eq!(metadata.spawn_group_templates().get(&10).unwrap().map_id, 1);
        assert!(metadata.spawn_group_templates().contains_key(&0));
        assert!(metadata.spawn_group_templates().contains_key(&1));
        assert_eq!(
            metadata
                .spawn_store()
                .spawn_group_ids_by_map(1)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::Creature, 300)
                .unwrap()
                .spawn_group_id(),
            10
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::GameObject, 301)
                .unwrap()
                .spawn_group_id(),
            11
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::AreaTrigger, 302)
                .unwrap()
                .spawn_group_id(),
            1
        );
        assert_eq!(
            store
                .spawn_data(SpawnObjectType::GameObject, 303)
                .unwrap()
                .spawn_group_id(),
            10
        );
        assert!(
            store
                .cell_object_guids(1, 0, event_managed.cell_id())
                .is_none_or(|cell| !cell.gameobjects.contains(&303))
        );
    }

    #[test]
    fn canonical_spawn_metadata_spawn_group_helper_filters_by_map_and_template_like_cpp() {
        let (template_store, _) = wow_data::SpawnGroupTemplateStore::from_rows_like_cpp([
            wow_data::SpawnGroupTemplateRow {
                group_id: 20,
                name: "map-one-a".to_string(),
                flags: 0,
            },
            wow_data::SpawnGroupTemplateRow {
                group_id: 21,
                name: "map-one-b".to_string(),
                flags: 0,
            },
            wow_data::SpawnGroupTemplateRow {
                group_id: 22,
                name: "map-two".to_string(),
                flags: 0,
            },
        ]);
        let mut templates = spawn_group_templates_for_spawn_store(&template_store);
        let maps = map_store(&[1, 2]);
        let difficulties = map_difficulty_store(&[(1, 0), (2, 0)]);
        let mut report = SpawnKindLoadReport::default();
        let mut store = SpawnStore::new();

        let map_one_a = creature_row_to_spawn_data_like_cpp(
            &creature_row(400, 0, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();
        let map_one_b = gameobject_row_to_spawn_data_like_cpp(
            &gameobject_row(401, 0, "0"),
            &maps,
            &difficulties,
            &mut report,
        )
        .unwrap();
        let mut map_two_row = creature_row(402, 0, "0");
        map_two_row.map_id = 2;
        let map_two =
            creature_row_to_spawn_data_like_cpp(&map_two_row, &maps, &difficulties, &mut report)
                .unwrap();

        store.add_object_spawn(&map_one_a, is_personal_phase_like_cpp_represented);
        store.add_object_spawn(&map_one_b, is_personal_phase_like_cpp_represented);
        store.add_object_spawn(&map_two, is_personal_phase_like_cpp_represented);
        let apply = store.apply_spawn_groups_like_cpp(
            &mut templates,
            [
                SpawnGroupMemberRow {
                    group_id: 21,
                    spawn_type: SpawnObjectType::GameObject as u8,
                    spawn_id: 401,
                },
                SpawnGroupMemberRow {
                    group_id: 20,
                    spawn_type: SpawnObjectType::Creature as u8,
                    spawn_id: 400,
                },
                SpawnGroupMemberRow {
                    group_id: 22,
                    spawn_type: SpawnObjectType::Creature as u8,
                    spawn_id: 402,
                },
            ],
        );
        assert_eq!(apply.assigned, 3);

        // Simulate a future C++-shaped filter miss without panicking: the group id is indexed
        // for the map, but `GetSpawnGroupData`/map filtering no longer returns a matching template.
        templates.get_mut(&21).unwrap().map_id = 2;
        let metadata = CanonicalSpawnMetadataLikeCpp::new(store, templates);

        let map_one_groups = metadata.spawn_group_templates_for_map_like_cpp(1);
        assert_eq!(
            map_one_groups
                .iter()
                .map(|(group_id, template)| (*group_id, template.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(20, "map-one-a")]
        );
        let map_two_groups = metadata.spawn_group_templates_for_map_like_cpp(2);
        assert_eq!(
            map_two_groups
                .iter()
                .map(|(group_id, template)| (*group_id, template.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(22, "map-two")]
        );
        assert!(
            metadata
                .spawn_group_templates_for_map_like_cpp(999)
                .is_empty()
        );
    }
}
