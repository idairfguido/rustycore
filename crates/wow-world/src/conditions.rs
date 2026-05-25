// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! Runtime side of C++ `ConditionMgr` evaluation context.

use std::sync::{Arc, OnceLock};

use num_traits::FromPrimitive;
use parking_lot::RwLock;
use wow_constants::MAX_CONDITION_TARGETS;
use wow_constants::{
    ComparisonType, ConditionInstanceInfo, ConditionSourceType, ConditionType, RelationType,
    TypeId, TypeMask, UnitStandStateType,
};
use wow_data::{
    Condition, ConditionEntriesByTypeStore, ConditionId, NpcSpellClickStoreLikeCpp,
    PlayerConditionContextLikeCpp, PlayerConditionStore, SPELL_CLICK_USER_FRIEND_LIKE_CPP,
    SPELL_CLICK_USER_PARTY_LIKE_CPP, SPELL_CLICK_USER_RAID_LIKE_CPP, SpellClickInfoLikeCpp,
    UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP, is_player_meeting_condition_like_cpp,
};
use wow_entities::WorldObject;
use wow_loot::{LootStoreItemContext, condition_source_type_for_loot_store_kind_like_cpp};

pub const QUEST_STATUS_NONE_LIKE_CPP: u8 = 0;
pub const QUEST_STATUS_COMPLETE_LIKE_CPP: u8 = 1;
pub const QUEST_STATUS_INCOMPLETE_LIKE_CPP: u8 = 3;
pub const QUEST_STATUS_FAILED_LIKE_CPP: u8 = 5;
pub const QUEST_STATUS_REWARDED_LIKE_CPP: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellClickRequirementContextLikeCpp {
    pub clicker_is_player: bool,
    pub clicker_is_friendly_to_summoner: bool,
    pub clicker_is_in_raid_with_summoner: bool,
    pub clicker_is_in_party_with_summoner: bool,
}

static CONDITION_MGR_STORE_LIKE_CPP: OnceLock<RwLock<Option<Arc<ConditionEntriesByTypeStore>>>> =
    OnceLock::new();

fn condition_mgr_store_slot_like_cpp() -> &'static RwLock<Option<Arc<ConditionEntriesByTypeStore>>>
{
    CONDITION_MGR_STORE_LIKE_CPP.get_or_init(|| RwLock::new(None))
}

/// Install the process-wide C++ `sConditionMgr` condition store.
///
/// This keeps the access pattern close to C++ while storing the actual data in an `Arc`, so a
/// future reload can atomically replace the active store without changing call sites.
pub fn set_condition_mgr_store_like_cpp(store: Arc<ConditionEntriesByTypeStore>) {
    *condition_mgr_store_slot_like_cpp().write() = Some(store);
}

/// Return the active C++ `sConditionMgr` store, if startup loaded it.
pub fn condition_mgr_store_like_cpp() -> Option<Arc<ConditionEntriesByTypeStore>> {
    condition_mgr_store_slot_like_cpp().read().as_ref().cloned()
}

/// Clear the process-wide condition store. Used by tests and future reload wiring.
pub fn clear_condition_mgr_store_like_cpp() {
    *condition_mgr_store_slot_like_cpp().write() = None;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConditionMapRef {
    pub map_id: u32,
    pub instance_id: u32,
}

impl ConditionMapRef {
    pub const fn new(map_id: u32, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
        }
    }
}

#[derive(Debug)]
pub struct ConditionSourceInfo<'a> {
    pub condition_targets: [Option<&'a WorldObject>; MAX_CONDITION_TARGETS],
    pub unit_targets: [Option<ConditionUnitSnapshot>; MAX_CONDITION_TARGETS],
    pub unit_aura_targets: [Option<&'a [ConditionAuraEffectSnapshot]>; MAX_CONDITION_TARGETS],
    pub unit_relation_targets: [Option<&'a [ConditionUnitRelationSnapshot]>; MAX_CONDITION_TARGETS],
    pub nearby_creature_targets:
        [Option<&'a [ConditionNearbyCreatureSnapshot]>; MAX_CONDITION_TARGETS],
    pub nearby_gameobject_targets:
        [Option<&'a [ConditionNearbyGameObjectSnapshot]>; MAX_CONDITION_TARGETS],
    pub player_targets: [Option<ConditionPlayerSnapshot>; MAX_CONDITION_TARGETS],
    pub player_quest_targets: [Option<ConditionPlayerQuestSnapshot<'a>>; MAX_CONDITION_TARGETS],
    pub player_progression_targets:
        [Option<ConditionPlayerProgressionSnapshot<'a>>; MAX_CONDITION_TARGETS],
    pub player_condition_contexts:
        [Option<PlayerConditionContextLikeCpp<'a>>; MAX_CONDITION_TARGETS],
    pub player_condition_store: Option<&'a PlayerConditionStore>,
    pub spawn_id_targets: [Option<u64>; MAX_CONDITION_TARGETS],
    pub private_object_targets: [bool; MAX_CONDITION_TARGETS],
    pub string_id_targets: [Option<&'a [&'a str]>; MAX_CONDITION_TARGETS],
    pub realm_achievement_ids: &'a [u32],
    pub map_state: Option<ConditionMapStateSnapshot<'a>>,
    pub condition_map: Option<ConditionMapRef>,
    pub last_failed_condition: Option<&'a Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionUnitSnapshot {
    pub level: u32,
    pub health: u64,
    pub max_health: u64,
    pub class_mask: u32,
    pub race: u8,
    pub creature_type: Option<u32>,
    pub is_alive: bool,
    pub is_charmed: bool,
    pub in_water: bool,
    pub unit_state: u32,
    pub stand_state: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionAuraEffectSnapshot {
    pub spell_id: u32,
    pub effect_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionUnitRelationSnapshot {
    pub to_target_index: usize,
    pub in_party: bool,
    pub in_raid_or_party: bool,
    pub owned_by: bool,
    pub passenger_of: bool,
    pub created_by: bool,
    pub reaction: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionNearbyCreatureSnapshot {
    pub entry: u32,
    pub distance: f32,
    pub is_alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionNearbyGameObjectSnapshot {
    pub entry: u32,
    pub distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionPlayerSnapshot {
    pub team: u32,
    pub native_gender: u32,
    pub drunken_state: u32,
    pub can_be_game_master: bool,
    pub is_game_master: bool,
    pub pet_type: Option<u32>,
    pub is_in_flight: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionQuestStatusSnapshot {
    pub quest_id: u32,
    pub status: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionQuestObjectiveProgressSnapshot {
    pub quest_id: u32,
    pub objective_id: u32,
    pub counter: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionPlayerQuestSnapshot<'a> {
    pub statuses: &'a [ConditionQuestStatusSnapshot],
    pub objective_progress: &'a [ConditionQuestObjectiveProgressSnapshot],
    pub rewarded_quest_ids: &'a [u32],
    pub daily_quest_ids: &'a [u32],
}

impl ConditionPlayerQuestSnapshot<'_> {
    fn quest_status_like_cpp(self, quest_id: u32) -> u8 {
        self.statuses
            .iter()
            .find(|status| status.quest_id == quest_id)
            .map(|status| status.status)
            .or_else(|| {
                self.is_quest_rewarded_like_cpp(quest_id)
                    .then_some(QUEST_STATUS_REWARDED_LIKE_CPP)
            })
            .unwrap_or(QUEST_STATUS_NONE_LIKE_CPP)
    }

    fn is_quest_rewarded_like_cpp(self, quest_id: u32) -> bool {
        self.rewarded_quest_ids.contains(&quest_id)
    }

    fn is_daily_quest_done_like_cpp(self, quest_id: u32) -> bool {
        self.daily_quest_ids.contains(&quest_id)
    }

    fn quest_is_in_log_like_cpp(self, quest_id: u32) -> bool {
        !matches!(
            self.quest_status_like_cpp(quest_id),
            QUEST_STATUS_NONE_LIKE_CPP | QUEST_STATUS_REWARDED_LIKE_CPP
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionItemCountSnapshot {
    pub item_id: u32,
    pub count: u32,
    pub bank_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionSkillSnapshot {
    pub skill_id: u32,
    pub base_value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionBattlePetCountSnapshot {
    pub species_id: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionReputationSnapshot {
    pub faction_id: u32,
    pub rank: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionPlayerProgressionSnapshot<'a> {
    pub items: &'a [ConditionItemCountSnapshot],
    pub equipped_item_or_gem_ids: &'a [u32],
    pub skills: &'a [ConditionSkillSnapshot],
    pub spell_ids: &'a [u32],
    pub achievement_ids: &'a [u32],
    pub reputations: &'a [ConditionReputationSnapshot],
    pub title_ids: &'a [u32],
    pub battle_pet_counts: &'a [ConditionBattlePetCountSnapshot],
    pub active_scene_ids: &'a [u32],
}

impl ConditionPlayerProgressionSnapshot<'_> {
    fn has_item_count_like_cpp(self, item_id: u32, required_count: u32, check_bank: bool) -> bool {
        self.items
            .iter()
            .find(|item| item.item_id == item_id)
            .is_some_and(|item| {
                let count = if check_bank {
                    item.count.saturating_add(item.bank_count)
                } else {
                    item.count
                };
                count >= required_count
            })
    }

    fn has_item_or_gem_equipped_like_cpp(self, item_id: u32) -> bool {
        self.equipped_item_or_gem_ids.contains(&item_id)
    }

    fn has_skill_base_value_like_cpp(self, skill_id: u32, required_value: u32) -> bool {
        self.skills
            .iter()
            .find(|skill| skill.skill_id == skill_id)
            .is_some_and(|skill| skill.base_value >= required_value)
    }

    fn has_spell_like_cpp(self, spell_id: u32) -> bool {
        self.spell_ids.contains(&spell_id)
    }

    fn has_achievement_like_cpp(self, achievement_id: u32) -> bool {
        self.achievement_ids.contains(&achievement_id)
    }

    fn has_reputation_rank_like_cpp(self, faction_id: u32, rank_mask: u32) -> bool {
        self.reputations
            .iter()
            .find(|reputation| reputation.faction_id == faction_id)
            .is_some_and(|reputation| ((1_u32 << reputation.rank) & rank_mask) != 0)
    }

    fn has_title_like_cpp(self, title_id: u32) -> bool {
        self.title_ids.contains(&title_id)
    }

    fn battle_pet_count_like_cpp(self, species_id: u32) -> u32 {
        self.battle_pet_counts
            .iter()
            .find(|pet| pet.species_id == species_id)
            .map(|pet| pet.count)
            .unwrap_or(0)
    }

    fn has_active_scene_like_cpp(self, scene_id: u32) -> bool {
        self.active_scene_ids.contains(&scene_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionMapDataSnapshot {
    pub id: u32,
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionWorldStateSnapshot {
    pub id: u32,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionMapStateSnapshot<'a> {
    pub active_event_ids: &'a [u32],
    pub world_states: &'a [ConditionWorldStateSnapshot],
    pub difficulty_id: u32,
    pub instance_data: &'a [ConditionMapDataSnapshot],
    pub instance_data64: &'a [ConditionMapDataSnapshot],
    pub boss_states: &'a [ConditionMapDataSnapshot],
    pub scenario_step_id: Option<u32>,
}

impl ConditionMapStateSnapshot<'_> {
    fn world_state_value_like_cpp(self, world_state_id: u32) -> i32 {
        self.world_states
            .iter()
            .find(|world_state| world_state.id == world_state_id)
            .map(|world_state| world_state.value)
            .unwrap_or(0)
    }

    fn instance_data_value_like_cpp(
        self,
        instance_info: ConditionInstanceInfo,
        id: u32,
    ) -> Option<u64> {
        let values = match instance_info {
            ConditionInstanceInfo::Data => self.instance_data,
            ConditionInstanceInfo::Data64 => self.instance_data64,
            ConditionInstanceInfo::BossState => self.boss_states,
            ConditionInstanceInfo::GuidData => return None,
        };
        values
            .iter()
            .find(|data| data.id == id)
            .map(|data| data.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionMeetResult {
    Evaluated(bool),
    Unsupported,
}

impl ConditionMeetResult {
    pub const fn value(self) -> Option<bool> {
        match self {
            Self::Evaluated(value) => Some(value),
            Self::Unsupported => None,
        }
    }
}

impl<'a> ConditionSourceInfo<'a> {
    /// C++ `ConditionSourceInfo(WorldObject const*, WorldObject const*, WorldObject const*)`.
    pub fn from_targets(
        target0: Option<&'a WorldObject>,
        target1: Option<&'a WorldObject>,
        target2: Option<&'a WorldObject>,
    ) -> Self {
        let condition_targets = [target0, target1, target2];
        let condition_map = condition_targets
            .iter()
            .flatten()
            .next()
            .map(|target| ConditionMapRef::new(target.map_id(), target.instance_id()));

        Self {
            condition_targets,
            unit_targets: [None; MAX_CONDITION_TARGETS],
            unit_aura_targets: [None; MAX_CONDITION_TARGETS],
            unit_relation_targets: [None; MAX_CONDITION_TARGETS],
            nearby_creature_targets: [None; MAX_CONDITION_TARGETS],
            nearby_gameobject_targets: [None; MAX_CONDITION_TARGETS],
            player_targets: [None; MAX_CONDITION_TARGETS],
            player_quest_targets: [None; MAX_CONDITION_TARGETS],
            player_progression_targets: [None; MAX_CONDITION_TARGETS],
            player_condition_contexts: [None; MAX_CONDITION_TARGETS],
            player_condition_store: None,
            spawn_id_targets: [None; MAX_CONDITION_TARGETS],
            private_object_targets: [false; MAX_CONDITION_TARGETS],
            string_id_targets: [None; MAX_CONDITION_TARGETS],
            realm_achievement_ids: &[],
            map_state: None,
            condition_map,
            last_failed_condition: None,
        }
    }

    /// C++ `ConditionSourceInfo(Map const*)`.
    pub const fn from_map(condition_map: ConditionMapRef) -> Self {
        Self {
            condition_targets: [None; MAX_CONDITION_TARGETS],
            unit_targets: [None; MAX_CONDITION_TARGETS],
            unit_aura_targets: [None; MAX_CONDITION_TARGETS],
            unit_relation_targets: [None; MAX_CONDITION_TARGETS],
            nearby_creature_targets: [None; MAX_CONDITION_TARGETS],
            nearby_gameobject_targets: [None; MAX_CONDITION_TARGETS],
            player_targets: [None; MAX_CONDITION_TARGETS],
            player_quest_targets: [None; MAX_CONDITION_TARGETS],
            player_progression_targets: [None; MAX_CONDITION_TARGETS],
            player_condition_contexts: [None; MAX_CONDITION_TARGETS],
            player_condition_store: None,
            spawn_id_targets: [None; MAX_CONDITION_TARGETS],
            private_object_targets: [false; MAX_CONDITION_TARGETS],
            string_id_targets: [None; MAX_CONDITION_TARGETS],
            realm_achievement_ids: &[],
            map_state: None,
            condition_map: Some(condition_map),
            last_failed_condition: None,
        }
    }

    pub fn set_unit_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionUnitSnapshot,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.unit_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_unit_aura_target_snapshot(
        &mut self,
        target_index: usize,
        aura_effects: &'a [ConditionAuraEffectSnapshot],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.unit_aura_targets[target_index] = Some(aura_effects);
        }
    }

    pub fn set_unit_relation_target_snapshot(
        &mut self,
        target_index: usize,
        relations: &'a [ConditionUnitRelationSnapshot],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.unit_relation_targets[target_index] = Some(relations);
        }
    }

    pub fn set_nearby_creature_target_snapshot(
        &mut self,
        target_index: usize,
        creatures: &'a [ConditionNearbyCreatureSnapshot],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.nearby_creature_targets[target_index] = Some(creatures);
        }
    }

    pub fn set_nearby_gameobject_target_snapshot(
        &mut self,
        target_index: usize,
        gameobjects: &'a [ConditionNearbyGameObjectSnapshot],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.nearby_gameobject_targets[target_index] = Some(gameobjects);
        }
    }

    pub fn set_player_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionPlayerSnapshot,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.player_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_player_quest_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionPlayerQuestSnapshot<'a>,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.player_quest_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_player_progression_target_snapshot(
        &mut self,
        target_index: usize,
        snapshot: ConditionPlayerProgressionSnapshot<'a>,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.player_progression_targets[target_index] = Some(snapshot);
        }
    }

    pub fn set_player_condition_context(
        &mut self,
        target_index: usize,
        context: PlayerConditionContextLikeCpp<'a>,
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.player_condition_contexts[target_index] = Some(context);
        }
    }

    pub fn set_player_condition_store(&mut self, store: &'a PlayerConditionStore) {
        self.player_condition_store = Some(store);
    }

    pub fn set_realm_achievement_ids(&mut self, achievement_ids: &'a [u32]) {
        self.realm_achievement_ids = achievement_ids;
    }

    pub fn set_map_state_snapshot(&mut self, map_state: ConditionMapStateSnapshot<'a>) {
        self.map_state = Some(map_state);
    }

    pub fn set_spawn_id_target_snapshot(&mut self, target_index: usize, spawn_id: u64) {
        if target_index < MAX_CONDITION_TARGETS {
            self.spawn_id_targets[target_index] = Some(spawn_id);
        }
    }

    pub fn set_private_object_target_snapshot(&mut self, target_index: usize, is_private: bool) {
        if target_index < MAX_CONDITION_TARGETS {
            self.private_object_targets[target_index] = is_private;
        }
    }

    pub fn set_string_id_target_snapshot(
        &mut self,
        target_index: usize,
        string_ids: &'a [&'a str],
    ) {
        if target_index < MAX_CONDITION_TARGETS {
            self.string_id_targets[target_index] = Some(string_ids);
        }
    }

    pub fn mark_failed_like_cpp(&mut self, condition: &'a Condition) {
        self.last_failed_condition = Some(condition);
    }
}

fn compare_values_u64_like_cpp(comparison_type: u32, left: u64, right: u64) -> bool {
    match ComparisonType::from_u32(comparison_type) {
        Some(ComparisonType::Eq) => left == right,
        Some(ComparisonType::High) => left > right,
        Some(ComparisonType::Low) => left < right,
        Some(ComparisonType::HighEq) => left >= right,
        Some(ComparisonType::LowEq) => left <= right,
        _ => false,
    }
}

fn compare_values_f32_like_cpp(comparison_type: u32, left: f32, right: f32) -> bool {
    match ComparisonType::from_u32(comparison_type) {
        Some(ComparisonType::Eq) => left == right,
        Some(ComparisonType::High) => left > right,
        Some(ComparisonType::Low) => left < right,
        Some(ComparisonType::HighEq) => left >= right,
        Some(ComparisonType::LowEq) => left <= right,
        _ => false,
    }
}

fn is_player_object_like_cpp(object: &WorldObject) -> bool {
    matches!(
        object.object().type_id(),
        TypeId::Player | TypeId::ActivePlayer
    )
}

fn is_unit_object_like_cpp(object: &WorldObject) -> bool {
    matches!(
        object.object().type_id(),
        TypeId::Unit | TypeId::Player | TypeId::ActivePlayer
    ) || object
        .object()
        .type_mask()
        .intersects(TypeMask::UNIT | TypeMask::PLAYER | TypeMask::ACTIVE_PLAYER)
}

fn unit_stand_state_is_sit_like_cpp(stand_state: u32) -> bool {
    stand_state == UnitStandStateType::Sit as u32
        || stand_state == UnitStandStateType::SitChair as u32
        || stand_state == UnitStandStateType::SitLowChair as u32
        || stand_state == UnitStandStateType::SitMediumChair as u32
        || stand_state == UnitStandStateType::SitHighChair as u32
}

fn unit_stand_state_is_stand_like_cpp(stand_state: u32) -> bool {
    !unit_stand_state_is_sit_like_cpp(stand_state)
        && stand_state != UnitStandStateType::Sleep as u32
        && stand_state != UnitStandStateType::Kneel as u32
}

/// Ported subset of C++ `Condition::Meets`.
///
/// Returns `Unsupported` for condition types whose exact C++ state is not yet represented by
/// the Rust entity/DB2/runtime layers.
pub fn condition_meets_basic_like_cpp<'a>(
    condition: &'a Condition,
    source_info: &mut ConditionSourceInfo<'a>,
    mut is_in_area_like_cpp: impl FnMut(u32, u32) -> bool,
) -> ConditionMeetResult {
    if usize::from(condition.condition_target) >= MAX_CONDITION_TARGETS {
        return ConditionMeetResult::Evaluated(false);
    }

    let mut cond_meets = false;
    let mut needs_object = false;

    match condition.condition_type {
        ConditionType::None => cond_meets = true,
        ConditionType::MapId => {
            cond_meets = source_info
                .condition_map
                .is_some_and(|map| map.map_id == condition.condition_value1);
        }
        ConditionType::RealmAchievement => {
            cond_meets = source_info
                .realm_achievement_ids
                .contains(&condition.condition_value1);
        }
        ConditionType::ActiveEvent => {
            cond_meets = source_info.map_state.is_some_and(|map_state| {
                map_state
                    .active_event_ids
                    .contains(&condition.condition_value1)
            });
        }
        ConditionType::WorldState => {
            cond_meets = source_info.map_state.is_some_and(|map_state| {
                map_state.world_state_value_like_cpp(condition.condition_value1)
                    == condition.condition_value2 as i32
            });
        }
        ConditionType::DifficultyId => {
            cond_meets = source_info
                .map_state
                .is_some_and(|map_state| map_state.difficulty_id == condition.condition_value1);
        }
        ConditionType::InstanceInfo => {
            let Some(instance_info) = ConditionInstanceInfo::from_u32(condition.condition_value3)
            else {
                return ConditionMeetResult::Evaluated(false);
            };
            cond_meets = source_info.map_state.is_some_and(|map_state| {
                map_state
                    .instance_data_value_like_cpp(instance_info, condition.condition_value1)
                    .is_some_and(|value| value == u64::from(condition.condition_value2))
            });
        }
        ConditionType::ScenarioStep => {
            cond_meets = source_info.map_state.is_some_and(|map_state| {
                map_state.scenario_step_id == Some(condition.condition_value1)
            });
        }
        _ => needs_object = true,
    }

    let target_index = usize::from(condition.condition_target);
    let object = source_info.condition_targets[target_index];
    if needs_object && object.is_none() {
        return ConditionMeetResult::Evaluated(false);
    }

    if let Some(object) = object {
        let unit = source_info.unit_targets[target_index];
        let unit_auras =
            source_info.unit_aura_targets[target_index].filter(|_| is_unit_object_like_cpp(object));
        let unit_relations = source_info.unit_relation_targets[target_index]
            .filter(|_| is_unit_object_like_cpp(object));
        let is_player = is_player_object_like_cpp(object);
        let player = source_info.player_targets[target_index].filter(|_| is_player);
        let player_quests = source_info.player_quest_targets[target_index].filter(|_| is_player);
        let player_progression =
            source_info.player_progression_targets[target_index].filter(|_| is_player);
        let player_condition_context =
            source_info.player_condition_contexts[target_index].filter(|_| is_player);
        match condition.condition_type {
            ConditionType::Aura => {
                if let Some(aura_effects) = unit_auras {
                    cond_meets = aura_effects.iter().any(|aura| {
                        aura.spell_id == condition.condition_value1
                            && aura.effect_index == condition.condition_value2
                    });
                }
            }
            ConditionType::ZoneId => cond_meets = object.zone_id() == condition.condition_value1,
            ConditionType::AreaId => {
                cond_meets = is_in_area_like_cpp(object.area_id(), condition.condition_value1);
            }
            ConditionType::Class => {
                if let Some(unit) = unit {
                    cond_meets = (unit.class_mask & condition.condition_value1) != 0;
                }
            }
            ConditionType::Team => {
                if let Some(player) = player {
                    cond_meets = player.team == condition.condition_value1;
                }
            }
            ConditionType::Race => {
                if let Some(unit) = unit {
                    cond_meets = unit.race != 0
                        && condition.condition_value1 & (1_u32 << u32::from(unit.race - 1)) != 0;
                }
            }
            ConditionType::Gender => {
                if let Some(player) = player {
                    cond_meets = player.native_gender == condition.condition_value1;
                }
            }
            ConditionType::Item => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_item_count_like_cpp(
                        condition.condition_value1,
                        condition.condition_value2,
                        condition.condition_value3 != 0,
                    );
                }
            }
            ConditionType::ItemEquipped => {
                if let Some(progression) = player_progression {
                    cond_meets =
                        progression.has_item_or_gem_equipped_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::Achievement => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_achievement_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::ReputationRank => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_reputation_rank_like_cpp(
                        condition.condition_value1,
                        condition.condition_value2,
                    );
                }
            }
            ConditionType::Skill => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_skill_base_value_like_cpp(
                        condition.condition_value1,
                        condition.condition_value2,
                    );
                }
            }
            ConditionType::QuestRewarded => {
                if let Some(quests) = player_quests {
                    cond_meets = quests.is_quest_rewarded_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::QuestTaken => {
                if let Some(quests) = player_quests {
                    cond_meets = quests.quest_status_like_cpp(condition.condition_value1)
                        == QUEST_STATUS_INCOMPLETE_LIKE_CPP;
                }
            }
            ConditionType::QuestComplete => {
                if let Some(quests) = player_quests {
                    cond_meets = quests.quest_status_like_cpp(condition.condition_value1)
                        == QUEST_STATUS_COMPLETE_LIKE_CPP
                        && !quests.is_quest_rewarded_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::QuestNone => {
                if let Some(quests) = player_quests {
                    cond_meets = quests.quest_status_like_cpp(condition.condition_value1)
                        == QUEST_STATUS_NONE_LIKE_CPP;
                }
            }
            ConditionType::Spell => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_spell_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::Level => {
                if let Some(unit) = unit {
                    cond_meets = compare_values_u64_like_cpp(
                        condition.condition_value2,
                        u64::from(unit.level),
                        u64::from(condition.condition_value1),
                    );
                }
            }
            ConditionType::ObjectEntryGuid | ConditionType::ObjectEntryGuidLegacy => {
                let type_id = object.object().type_id();
                if type_id as u32 == condition.condition_value1 {
                    cond_meets = condition.condition_value2 == 0
                        || object.object().entry() == condition.condition_value2;

                    if condition.condition_value3 != 0
                        && matches!(type_id, TypeId::Unit | TypeId::GameObject)
                    {
                        cond_meets &=
                            source_info.spawn_id_targets[target_index].is_some_and(|spawn_id| {
                                spawn_id == u64::from(condition.condition_value3)
                            });
                    }
                }
            }
            ConditionType::TypeMask | ConditionType::TypeMaskLegacy => {
                cond_meets = object
                    .object()
                    .is_type(TypeMask::from_bits_truncate(condition.condition_value1));
            }
            ConditionType::DistanceTo => {
                if let Some(to_object) = usize::try_from(condition.condition_value1)
                    .ok()
                    .filter(|target| *target < MAX_CONDITION_TARGETS)
                    .and_then(|target| source_info.condition_targets[target])
                {
                    cond_meets = compare_values_f32_like_cpp(
                        condition.condition_value3,
                        object.distance(to_object),
                        condition.condition_value2 as f32,
                    );
                }
            }
            ConditionType::NearCreature => {
                if let Some(creatures) = source_info.nearby_creature_targets[target_index] {
                    let alive_required = condition.condition_value3 == 0;
                    cond_meets = creatures.iter().any(|creature| {
                        creature.entry == condition.condition_value1
                            && creature.distance <= condition.condition_value2 as f32
                            && (!alive_required || creature.is_alive)
                    });
                }
            }
            ConditionType::NearGameObject => {
                if let Some(gameobjects) = source_info.nearby_gameobject_targets[target_index] {
                    cond_meets = gameobjects.iter().any(|gameobject| {
                        gameobject.entry == condition.condition_value1
                            && gameobject.distance <= condition.condition_value2 as f32
                    });
                }
            }
            ConditionType::RelationTo => {
                let Some(relation_type) = RelationType::from_u32(condition.condition_value2) else {
                    return ConditionMeetResult::Unsupported;
                };

                if let Some(to_target_index) = usize::try_from(condition.condition_value1)
                    .ok()
                    .filter(|target| *target < MAX_CONDITION_TARGETS)
                {
                    if let Some(to_object) = source_info.condition_targets[to_target_index]
                        && is_unit_object_like_cpp(object)
                        && is_unit_object_like_cpp(to_object)
                    {
                        cond_meets = match relation_type {
                            RelationType::SelfRelation => std::ptr::eq(object, to_object),
                            RelationType::InParty => unit_relations
                                .and_then(|relations| {
                                    relations.iter().find(|relation| {
                                        relation.to_target_index == to_target_index
                                    })
                                })
                                .is_some_and(|relation| relation.in_party),
                            RelationType::InRaidOrParty => unit_relations
                                .and_then(|relations| {
                                    relations.iter().find(|relation| {
                                        relation.to_target_index == to_target_index
                                    })
                                })
                                .is_some_and(|relation| relation.in_raid_or_party),
                            RelationType::OwnedBy => unit_relations
                                .and_then(|relations| {
                                    relations.iter().find(|relation| {
                                        relation.to_target_index == to_target_index
                                    })
                                })
                                .is_some_and(|relation| relation.owned_by),
                            RelationType::PassengerOf => unit_relations
                                .and_then(|relations| {
                                    relations.iter().find(|relation| {
                                        relation.to_target_index == to_target_index
                                    })
                                })
                                .is_some_and(|relation| relation.passenger_of),
                            RelationType::CreatedBy => unit_relations
                                .and_then(|relations| {
                                    relations.iter().find(|relation| {
                                        relation.to_target_index == to_target_index
                                    })
                                })
                                .is_some_and(|relation| relation.created_by),
                            RelationType::Max => false,
                        };
                    }
                }
            }
            ConditionType::ReactionTo => {
                if let Some(to_target_index) = usize::try_from(condition.condition_value1)
                    .ok()
                    .filter(|target| *target < MAX_CONDITION_TARGETS)
                {
                    if let Some(to_object) = source_info.condition_targets[to_target_index]
                        && is_unit_object_like_cpp(object)
                        && is_unit_object_like_cpp(to_object)
                        && let Some(relation) = unit_relations.and_then(|relations| {
                            relations
                                .iter()
                                .find(|relation| relation.to_target_index == to_target_index)
                        })
                    {
                        cond_meets =
                            ((1_u32 << relation.reaction) & condition.condition_value2) != 0;
                    }
                }
            }
            ConditionType::PhaseId => {
                cond_meets = object
                    .phase_shift()
                    .has_phase_like_cpp(condition.condition_value1);
            }
            ConditionType::TerrainSwap => {
                cond_meets = object
                    .phase_shift()
                    .has_visible_map_id_like_cpp(condition.condition_value1);
            }
            ConditionType::Alive => {
                if let Some(unit) = unit {
                    cond_meets = unit.is_alive;
                }
            }
            ConditionType::HpVal => {
                if let Some(unit) = unit {
                    cond_meets = compare_values_u64_like_cpp(
                        condition.condition_value2,
                        unit.health,
                        u64::from(condition.condition_value1),
                    );
                }
            }
            ConditionType::HpPct => {
                if let Some(unit) = unit {
                    let health_pct = if unit.max_health == 0 {
                        0.0
                    } else {
                        (unit.health as f32 / unit.max_health as f32) * 100.0
                    };
                    cond_meets = compare_values_f32_like_cpp(
                        condition.condition_value2,
                        health_pct,
                        condition.condition_value1 as f32,
                    );
                }
            }
            ConditionType::UnitState => {
                if let Some(unit) = unit {
                    cond_meets = (unit.unit_state & condition.condition_value1) != 0;
                }
            }
            ConditionType::InWater => {
                if let Some(unit) = unit {
                    cond_meets = unit.in_water;
                }
            }
            ConditionType::CreatureType => {
                if let Some(unit) = unit
                    && object.object().type_id() == TypeId::Unit
                    && let Some(creature_type) = unit.creature_type
                {
                    cond_meets = creature_type == condition.condition_value1;
                }
            }
            ConditionType::StandState => {
                if let Some(unit) = unit {
                    cond_meets = if condition.condition_value1 == 0 {
                        unit.stand_state == condition.condition_value2
                    } else if condition.condition_value2 == 0 {
                        unit_stand_state_is_stand_like_cpp(unit.stand_state)
                    } else if condition.condition_value2 == 1 {
                        unit_stand_state_is_sit_like_cpp(unit.stand_state)
                    } else {
                        false
                    };
                }
            }
            ConditionType::DrunkenState => {
                if let Some(player) = player {
                    cond_meets = player.drunken_state >= condition.condition_value1;
                }
            }
            ConditionType::GameMaster => {
                if let Some(player) = player {
                    cond_meets = if condition.condition_value1 == 1 {
                        player.can_be_game_master
                    } else {
                        player.is_game_master
                    };
                }
            }
            ConditionType::PetType => {
                if let Some(player) = player
                    && let Some(pet_type) = player.pet_type
                {
                    cond_meets = ((1_u32 << pet_type) & condition.condition_value1) != 0;
                }
            }
            ConditionType::Taxi => {
                if let Some(player) = player {
                    cond_meets = player.is_in_flight;
                }
            }
            ConditionType::Title => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_title_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::BattlePetCount => {
                if let Some(progression) = player_progression {
                    cond_meets = compare_values_u64_like_cpp(
                        condition.condition_value3,
                        u64::from(
                            progression.battle_pet_count_like_cpp(condition.condition_value1),
                        ),
                        u64::from(condition.condition_value2),
                    );
                }
            }
            ConditionType::SceneInProgress => {
                if let Some(progression) = player_progression {
                    cond_meets = progression.has_active_scene_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::PlayerCondition => {
                let Some(store) = source_info.player_condition_store else {
                    return ConditionMeetResult::Unsupported;
                };
                let Some(context) = player_condition_context else {
                    return ConditionMeetResult::Unsupported;
                };
                if let Some(player_condition) = store.get(condition.condition_value1) {
                    cond_meets = is_player_meeting_condition_like_cpp(player_condition, &context);
                }
            }
            ConditionType::DailyQuestDone => {
                if let Some(quests) = player_quests {
                    cond_meets = quests.is_daily_quest_done_like_cpp(condition.condition_value1);
                }
            }
            ConditionType::QuestState => {
                if let Some(quests) = player_quests {
                    let quest_status = quests.quest_status_like_cpp(condition.condition_value1);
                    cond_meets = ((condition.condition_value2 & (1 << QUEST_STATUS_NONE_LIKE_CPP))
                        != 0
                        && quest_status == QUEST_STATUS_NONE_LIKE_CPP)
                        || ((condition.condition_value2 & (1 << QUEST_STATUS_COMPLETE_LIKE_CPP))
                            != 0
                            && quest_status == QUEST_STATUS_COMPLETE_LIKE_CPP)
                        || ((condition.condition_value2 & (1 << QUEST_STATUS_INCOMPLETE_LIKE_CPP))
                            != 0
                            && quest_status == QUEST_STATUS_INCOMPLETE_LIKE_CPP)
                        || ((condition.condition_value2 & (1 << QUEST_STATUS_FAILED_LIKE_CPP))
                            != 0
                            && quest_status == QUEST_STATUS_FAILED_LIKE_CPP)
                        || ((condition.condition_value2 & (1 << QUEST_STATUS_REWARDED_LIKE_CPP))
                            != 0
                            && quests.is_quest_rewarded_like_cpp(condition.condition_value1));
                }
            }
            ConditionType::QuestObjectiveProgress => {
                if let Some(quests) = player_quests
                    && let Some(progress) = quests.objective_progress.iter().find(|progress| {
                        progress.objective_id == condition.condition_value1
                            && quests.quest_is_in_log_like_cpp(progress.quest_id)
                    })
                {
                    cond_meets = progress.counter == condition.condition_value3 as i32;
                }
            }
            ConditionType::Charmed => {
                if let Some(unit) = unit {
                    cond_meets = unit.is_charmed;
                }
            }
            ConditionType::PrivateObject => {
                cond_meets = source_info.private_object_targets[target_index];
            }
            ConditionType::StringId => {
                if matches!(object.object().type_id(), TypeId::Unit | TypeId::GameObject)
                    && let Some(string_ids) = source_info.string_id_targets[target_index]
                {
                    cond_meets = string_ids
                        .iter()
                        .any(|string_id| *string_id == condition.condition_string_value1.as_str());
                }
            }
            ConditionType::None | ConditionType::MapId | ConditionType::RealmAchievement => {}
            ConditionType::ActiveEvent
            | ConditionType::InstanceInfo
            | ConditionType::WorldState
            | ConditionType::DifficultyId
            | ConditionType::ScenarioStep => {}
            _ => return ConditionMeetResult::Unsupported,
        }
    }

    if condition.negative_condition {
        cond_meets = !cond_meets;
    }

    if !cond_meets {
        source_info.mark_failed_like_cpp(condition);
    }

    ConditionMeetResult::Evaluated(cond_meets)
}

/// C++ `ConditionMgr::IsObjectMeetToConditions`.
pub fn is_object_meet_to_conditions_like_cpp<'a>(
    source_info: &mut ConditionSourceInfo<'a>,
    conditions: &'a [Condition],
    condition_store: &'a ConditionEntriesByTypeStore,
    mut meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if conditions.is_empty() {
        return true;
    }

    is_object_meet_to_condition_list_like_cpp(source_info, conditions, condition_store, &mut meets)
}

fn is_object_meet_to_condition_list_like_cpp<'a, F>(
    source_info: &mut ConditionSourceInfo<'a>,
    conditions: &'a [Condition],
    condition_store: &'a ConditionEntriesByTypeStore,
    meets: &mut F,
) -> bool
where
    F: FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
{
    let mut else_group_store = std::collections::BTreeMap::<u32, bool>::new();

    for condition in conditions {
        if !condition.is_loaded_like_cpp() {
            continue;
        }

        let group_passed = else_group_store.entry(condition.else_group).or_insert(true);
        if !*group_passed {
            continue;
        }

        if condition.reference_id != 0 {
            if let Some(reference_conditions) = condition_store.conditions_for_like_cpp(
                ConditionSourceType::ReferenceCondition,
                ConditionId::new(condition.reference_id, 0, 0),
            ) && !is_object_meet_to_condition_list_like_cpp(
                source_info,
                reference_conditions.as_slice(),
                condition_store,
                meets,
            ) {
                *group_passed = false;
            }
        } else if !meets(condition, source_info) {
            *group_passed = false;
        }
    }

    else_group_store.values().any(|passed| *passed)
}

/// C++ `ConditionMgr::IsObjectMeetingNotGroupedConditions`.
pub fn is_object_meeting_not_grouped_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    source_type: ConditionSourceType,
    entry: u32,
    source_info: &mut ConditionSourceInfo<'a>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if (source_type as u32) > ConditionSourceType::None as u32
        && (source_type as u32) < ConditionSourceType::Max as u32
    {
        if let Some(conditions) = condition_store
            .conditions_for_like_cpp(source_type, ConditionId::new(0, entry as i32, 0))
        {
            return is_object_meet_to_conditions_like_cpp(
                source_info,
                conditions.as_slice(),
                condition_store,
                meets,
            );
        }
    }

    true
}

/// C++ `ConditionMgr::IsMapMeetingNotGroupedConditions` for spawn-group map conditions.
///
/// This is a map-only evaluation helper for `CONDITION_SOURCE_TYPE_SPAWN_GROUP` buckets keyed as
/// `{ source_group = 0, source_entry = spawn_group_id, source_id = 0 }`. It deliberately does not
/// mutate `wow_map::Map` or execute `SpawnGroupSpawn`/`SpawnGroupDespawn`; future live callers must
/// pass the returned bool into the map-side planner/executor.
pub fn is_spawn_group_meeting_map_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    spawn_group_id: u32,
    condition_map: ConditionMapRef,
    map_state: Option<ConditionMapStateSnapshot<'a>>,
    realm_achievement_ids: &'a [u32],
) -> bool {
    let mut source_info = ConditionSourceInfo::from_map(condition_map);
    if let Some(map_state) = map_state {
        source_info.set_map_state_snapshot(map_state);
    }
    source_info.set_realm_achievement_ids(realm_achievement_ids);

    is_object_meeting_not_grouped_conditions_like_cpp(
        condition_store,
        ConditionSourceType::SpawnGroup,
        spawn_group_id,
        &mut source_info,
        |condition, source_info| {
            condition_meets_basic_like_cpp(condition, source_info, |_area, _zone| false)
                .value()
                .unwrap_or(false)
        },
    )
}

/// C++ `ConditionMgr::HasConditionsForNotGroupedEntry`.
pub fn has_conditions_for_not_grouped_entry_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    source_type: ConditionSourceType,
    entry: u32,
) -> bool {
    (source_type as u32) > ConditionSourceType::None as u32
        && (source_type as u32) < ConditionSourceType::Max as u32
        && condition_store
            .conditions_for_like_cpp(source_type, ConditionId::new(0, entry as i32, 0))
            .is_some()
}

/// C++ `ConditionMgr::IsObjectMeetingSpellClickConditions`.
pub fn is_object_meeting_spell_click_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
    clicker: Option<&'a WorldObject>,
    target: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::SpellClickEvent,
        ConditionId::new(creature_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(clicker, target, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::HasConditionsForSpellClickEvent`.
pub fn has_conditions_for_spell_click_event_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
) -> bool {
    condition_store
        .conditions_for_like_cpp(
            ConditionSourceType::SpellClickEvent,
            ConditionId::new(creature_id, spell_id as i32, 0),
        )
        .is_some()
}

/// C++ `SpellClickInfo::IsFitToRequirements`.
pub fn spell_click_info_is_fit_to_requirements_like_cpp(
    info: &SpellClickInfoLikeCpp,
    context: SpellClickRequirementContextLikeCpp,
) -> bool {
    if !context.clicker_is_player {
        return true;
    }

    match info.user_type {
        SPELL_CLICK_USER_FRIEND_LIKE_CPP => context.clicker_is_friendly_to_summoner,
        SPELL_CLICK_USER_RAID_LIKE_CPP => context.clicker_is_in_raid_with_summoner,
        SPELL_CLICK_USER_PARTY_LIKE_CPP => context.clicker_is_in_party_with_summoner,
        _ => true,
    }
}

/// C++ `Player::CanSeeSpellClickOn`.
pub fn can_see_spell_click_on_like_cpp<'a>(
    spell_click_store: &NpcSpellClickStoreLikeCpp,
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_entry: u32,
    creature_npc_flags: u64,
    clicker: Option<&'a WorldObject>,
    target: Option<&'a WorldObject>,
    requirement_context: SpellClickRequirementContextLikeCpp,
    mut meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if (creature_npc_flags & UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP) == 0 {
        return false;
    }

    let click_bounds = spell_click_store.spell_click_info_map_bounds_like_cpp(creature_entry);
    if click_bounds.is_empty() {
        return false;
    }

    for click_info in click_bounds {
        if !spell_click_info_is_fit_to_requirements_like_cpp(click_info, requirement_context) {
            return false;
        }

        if is_object_meeting_spell_click_conditions_like_cpp(
            condition_store,
            creature_entry,
            click_info.spell_id,
            clicker,
            target,
            &mut meets,
        ) {
            return true;
        }
    }

    false
}

/// C++ `ConditionMgr::IsObjectMeetingVehicleSpellConditions`.
pub fn is_object_meeting_vehicle_spell_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    spell_id: u32,
    player: Option<&'a WorldObject>,
    vehicle: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::VehicleSpell,
        ConditionId::new(creature_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, vehicle, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingSmartEventConditions`.
pub fn is_object_meeting_smart_event_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    entry_or_guid: i64,
    event_id: u32,
    source_type: u32,
    unit: Option<&'a WorldObject>,
    base_object: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::SmartEvent,
        ConditionId::new(event_id + 1, entry_or_guid as i32, source_type),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(unit, base_object, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingVendorItemConditions`.
pub fn is_object_meeting_vendor_item_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    creature_id: u32,
    item_id: u32,
    player: Option<&'a WorldObject>,
    vendor: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::NpcVendor,
        ConditionId::new(creature_id, item_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, vendor, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::GetConditionsForAreaTrigger`.
pub fn conditions_for_area_trigger_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    area_trigger_id: u32,
    is_server_side: bool,
) -> Option<&[Condition]> {
    condition_store
        .conditions_for_like_cpp(
            ConditionSourceType::AreaTrigger,
            ConditionId::new(area_trigger_id, i32::from(is_server_side), 0),
        )
        .map(|conditions| conditions.as_slice())
}

/// C++ `ConditionMgr::IsObjectMeetingTrainerSpellConditions`.
pub fn is_object_meeting_trainer_spell_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    trainer_id: u32,
    spell_id: u32,
    player: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::TrainerSpell,
        ConditionId::new(trainer_id, spell_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(player, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `ConditionMgr::IsObjectMeetingVisibilityByObjectIdConditions`.
pub fn is_object_meeting_visibility_by_object_id_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    object_type: u32,
    entry: u32,
    seer: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        ConditionSourceType::ObjectIdVisibility,
        ConditionId::new(object_type, entry as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(seer, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

/// C++ `LootTemplate::LinkConditions` + `LootItem::AllowedForPlayer` condition check.
///
/// Rust keeps loot items immutable and passes `LootStoreItemContext` during fill; this resolves the
/// same condition bucket C++ links into `LootStoreItem::conditions` and evaluates it with the looter
/// as target0.
pub fn is_loot_store_item_meeting_conditions_like_cpp<'a>(
    condition_store: &'a ConditionEntriesByTypeStore,
    context: LootStoreItemContext,
    looter: Option<&'a WorldObject>,
    meets: impl FnMut(&'a Condition, &mut ConditionSourceInfo<'a>) -> bool,
) -> bool {
    let Some(source_type) = ConditionSourceType::from_u32(
        condition_source_type_for_loot_store_kind_like_cpp(context.store_kind) as u32,
    ) else {
        return true;
    };

    if let Some(conditions) = condition_store.conditions_for_like_cpp(
        source_type,
        ConditionId::new(context.entry, context.item.item_id as i32, 0),
    ) {
        let mut source_info = ConditionSourceInfo::from_targets(looter, None, None);
        return is_object_meet_to_conditions_like_cpp(
            &mut source_info,
            conditions.as_slice(),
            condition_store,
            meets,
        );
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_constants::{ConditionType, PhaseFlags, TypeId, TypeMask};
    use wow_core::Position;
    use wow_data::{PlayerConditionContextLikeCpp, PlayerConditionEntry, PlayerConditionStore};
    use wow_loot::{LootStoreItem, LootStoreItemContext, LootStoreKind};

    fn world_object(map_id: u32, instance_id: u32) -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Unit, TypeMask::UNIT);
        object.set_map(map_id, instance_id).unwrap();
        object
    }

    fn player_object(map_id: u32, instance_id: u32) -> WorldObject {
        let mut object = WorldObject::new(false, TypeId::Player, TypeMask::PLAYER | TypeMask::UNIT);
        object.set_map(map_id, instance_id).unwrap();
        object
    }

    #[test]
    fn condition_source_info_uses_first_non_null_target_map_like_cpp() {
        let target1 = world_object(571, 2);
        let target2 = world_object(1, 9);

        let info = ConditionSourceInfo::from_targets(None, Some(&target1), Some(&target2));

        assert_eq!(info.condition_targets[0].map(WorldObject::map_id), None);
        assert_eq!(
            info.condition_targets[1].map(WorldObject::map_id),
            Some(571)
        );
        assert_eq!(info.condition_map, Some(ConditionMapRef::new(571, 2)));
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn condition_source_info_map_constructor_matches_cpp() {
        let info = ConditionSourceInfo::from_map(ConditionMapRef::new(530, 7));

        assert!(info.condition_targets.iter().all(Option::is_none));
        assert_eq!(info.condition_map, Some(ConditionMapRef::new(530, 7)));
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn condition_source_info_tracks_last_failed_condition_like_cpp() {
        let condition = Condition::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        info.mark_failed_like_cpp(&condition);

        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &condition
        ));
    }

    #[test]
    fn global_condition_mgr_store_can_be_installed_and_replaced_like_cpp_reload_foundation() {
        clear_condition_mgr_store_like_cpp();
        assert!(condition_mgr_store_like_cpp().is_none());

        let first = Arc::new(ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::Phase,
                source_entry: 10,
                condition_type: ConditionType::None,
                ..Condition::default()
            },
        ]));
        set_condition_mgr_store_like_cpp(Arc::clone(&first));
        assert_eq!(
            condition_mgr_store_like_cpp()
                .as_ref()
                .map(|store| store.bucket_count()),
            Some(1)
        );

        let second = Arc::new(ConditionEntriesByTypeStore::default());
        set_condition_mgr_store_like_cpp(Arc::clone(&second));
        assert!(Arc::ptr_eq(
            &condition_mgr_store_like_cpp().expect("condition store installed"),
            &second
        ));

        clear_condition_mgr_store_like_cpp();
    }

    #[test]
    fn basic_condition_meets_map_zone_area_and_negative_like_cpp() {
        let mut target = world_object(571, 2);
        target.set_zone_and_area(67, 123);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        let active_events = [77];
        let world_states = [ConditionWorldStateSnapshot { id: 88, value: -5 }];
        let instance_data = [ConditionMapDataSnapshot { id: 1, value: 42 }];
        let instance_data64 = [ConditionMapDataSnapshot { id: 2, value: 84 }];
        let boss_states = [ConditionMapDataSnapshot { id: 3, value: 2 }];
        info.set_map_state_snapshot(ConditionMapStateSnapshot {
            active_event_ids: &active_events,
            world_states: &world_states,
            difficulty_id: 23,
            instance_data: &instance_data,
            instance_data64: &instance_data64,
            boss_states: &boss_states,
            scenario_step_id: Some(99),
        });

        let map_condition = Condition {
            condition_type: ConditionType::MapId,
            condition_value1: 571,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&map_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let zone_condition = Condition {
            condition_type: ConditionType::ZoneId,
            condition_value1: 67,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&zone_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let active_event_condition = Condition {
            condition_type: ConditionType::ActiveEvent,
            condition_value1: 77,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&active_event_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let world_state_condition = Condition {
            condition_type: ConditionType::WorldState,
            condition_value1: 88,
            condition_value2: (-5_i32) as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&world_state_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let difficulty_condition = Condition {
            condition_type: ConditionType::DifficultyId,
            condition_value1: 23,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&difficulty_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let instance_data_condition = Condition {
            condition_type: ConditionType::InstanceInfo,
            condition_value1: 1,
            condition_value2: 42,
            condition_value3: ConditionInstanceInfo::Data as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&instance_data_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let instance_data64_condition = Condition {
            condition_type: ConditionType::InstanceInfo,
            condition_value1: 2,
            condition_value2: 84,
            condition_value3: ConditionInstanceInfo::Data64 as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&instance_data64_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let boss_state_condition = Condition {
            condition_type: ConditionType::InstanceInfo,
            condition_value1: 3,
            condition_value2: 2,
            condition_value3: ConditionInstanceInfo::BossState as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&boss_state_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let scenario_step_condition = Condition {
            condition_type: ConditionType::ScenarioStep,
            condition_value1: 99,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&scenario_step_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let area_condition = Condition {
            condition_type: ConditionType::AreaId,
            condition_value1: 999,
            negative_condition: true,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&area_condition, &mut info, |current, required| {
                current == 123 && required == 999
            }),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &area_condition
        ));
    }

    #[test]
    fn basic_condition_meets_object_type_entry_mask_and_phasing_like_cpp() {
        let mut target = world_object(571, 2);
        target.object_mut().set_entry(1001);
        target
            .phase_shift_mut()
            .add_phase_like_cpp(55, PhaseFlags::NONE, 1);
        target.phase_shift_mut().add_visible_map_id_like_cpp(609, 1);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_spawn_id_target_snapshot(0, 42);
        info.set_private_object_target_snapshot(0, true);
        info.set_string_id_target_snapshot(0, &["template-id", "spawn-id", "script-id"]);

        let entry_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&entry_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let spawn_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            condition_value3: 42,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&spawn_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let private_condition = Condition {
            condition_type: ConditionType::PrivateObject,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&private_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let string_id_condition = Condition {
            condition_type: ConditionType::StringId,
            condition_string_value1: "spawn-id".to_string(),
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&string_id_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let mask_condition = Condition {
            condition_type: ConditionType::TypeMask,
            condition_value1: TypeMask::UNIT.bits(),
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&mask_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let phase_condition = Condition {
            condition_type: ConditionType::PhaseId,
            condition_value1: 55,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&phase_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let terrain_condition = Condition {
            condition_type: ConditionType::TerrainSwap,
            condition_value1: 609,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&terrain_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );
    }

    #[test]
    fn basic_condition_meets_distance_to_uses_cpp_target_and_combat_reach_distance() {
        let mut target0 = world_object(571, 2);
        let mut target1 = world_object(571, 2);
        target0.relocate(Position::xyz(0.0, 0.0, 0.0));
        target1.relocate(Position::xyz(3.0, 4.0, 0.0));
        target0.set_combat_reach(1.0);
        target1.set_combat_reach(1.0);

        let mut info = ConditionSourceInfo::from_targets(Some(&target0), Some(&target1), None);
        let condition = Condition {
            condition_type: ConditionType::DistanceTo,
            condition_value1: 1,
            condition_value2: 3,
            condition_value3: ComparisonType::LowEq as u32,
            ..Condition::default()
        };

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );
    }

    #[test]
    fn basic_condition_meets_relation_self_like_cpp() {
        let target = world_object(571, 2);
        let other = world_object(571, 2);
        let self_condition = Condition {
            condition_type: ConditionType::RelationTo,
            condition_value1: 1,
            condition_value2: RelationType::SelfRelation as u32,
            ..Condition::default()
        };

        let mut same_info = ConditionSourceInfo::from_targets(Some(&target), Some(&target), None);
        assert_eq!(
            condition_meets_basic_like_cpp(&self_condition, &mut same_info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let mut other_info = ConditionSourceInfo::from_targets(Some(&target), Some(&other), None);
        assert_eq!(
            condition_meets_basic_like_cpp(&self_condition, &mut other_info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );

        let party_condition = Condition {
            condition_type: ConditionType::RelationTo,
            condition_value1: 1,
            condition_value2: RelationType::InParty as u32,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&party_condition, &mut other_info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_relation_reaction_and_nearby_snapshots_like_cpp() {
        let target = world_object(571, 2);
        let other = world_object(571, 2);
        let relations = [ConditionUnitRelationSnapshot {
            to_target_index: 1,
            in_party: true,
            in_raid_or_party: true,
            owned_by: true,
            passenger_of: true,
            created_by: true,
            reaction: 4,
        }];
        let nearby_creatures = [
            ConditionNearbyCreatureSnapshot {
                entry: 1001,
                distance: 10.0,
                is_alive: true,
            },
            ConditionNearbyCreatureSnapshot {
                entry: 1002,
                distance: 5.0,
                is_alive: false,
            },
        ];
        let nearby_gameobjects = [ConditionNearbyGameObjectSnapshot {
            entry: 2001,
            distance: 12.0,
        }];
        let mut info = ConditionSourceInfo::from_targets(Some(&target), Some(&other), None);
        info.set_unit_relation_target_snapshot(0, &relations);
        info.set_nearby_creature_target_snapshot(0, &nearby_creatures);
        info.set_nearby_gameobject_target_snapshot(0, &nearby_gameobjects);

        let conditions = vec![
            Condition {
                condition_type: ConditionType::RelationTo,
                condition_value1: 1,
                condition_value2: RelationType::InParty as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::RelationTo,
                condition_value1: 1,
                condition_value2: RelationType::OwnedBy as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::RelationTo,
                condition_value1: 1,
                condition_value2: RelationType::PassengerOf as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::RelationTo,
                condition_value1: 1,
                condition_value2: RelationType::CreatedBy as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::ReactionTo,
                condition_value1: 1,
                condition_value2: 1 << 4,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::NearCreature,
                condition_value1: 1001,
                condition_value2: 10,
                condition_value3: 0,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::NearCreature,
                condition_value1: 1002,
                condition_value2: 5,
                condition_value3: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::NearGameObject,
                condition_value1: 2001,
                condition_value2: 12,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }
    }

    #[test]
    fn basic_condition_meets_unit_snapshot_class_race_level_and_life_like_cpp() {
        let target = world_object(571, 2);
        let aura_effects = [ConditionAuraEffectSnapshot {
            spell_id: 17,
            effect_index: 1,
        }];
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 70,
                health: 750,
                max_health: 1000,
                class_mask: 1 << (2 - 1),
                race: 4,
                creature_type: Some(7),
                is_alive: true,
                is_charmed: true,
                in_water: true,
                unit_state: 0x20,
                stand_state: 8,
            },
        );
        info.set_unit_aura_target_snapshot(0, &aura_effects);

        let conditions = vec![
            Condition {
                condition_type: ConditionType::Aura,
                condition_value1: 17,
                condition_value2: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Class,
                condition_value1: 1 << (2 - 1),
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Race,
                condition_value1: 1 << (4 - 1),
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Level,
                condition_value1: 60,
                condition_value2: ComparisonType::HighEq as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Alive,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::HpVal,
                condition_value1: 700,
                condition_value2: ComparisonType::High as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::HpPct,
                condition_value1: 75,
                condition_value2: ComparisonType::Eq as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::UnitState,
                condition_value1: 0x20,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::InWater,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::CreatureType,
                condition_value1: 7,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Charmed,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::StandState,
                condition_value1: 0,
                condition_value2: UnitStandStateType::Kneel as u32,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }
    }

    #[test]
    fn basic_condition_meets_player_snapshot_branches_like_cpp() {
        let target = player_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_player_target_snapshot(
            0,
            ConditionPlayerSnapshot {
                team: 469,
                native_gender: 1,
                drunken_state: 2,
                can_be_game_master: true,
                is_game_master: false,
                pet_type: Some(2),
                is_in_flight: true,
            },
        );

        let conditions = vec![
            Condition {
                condition_type: ConditionType::Team,
                condition_value1: 469,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Gender,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::DrunkenState,
                condition_value1: 2,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::GameMaster,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::PetType,
                condition_value1: 1 << 2,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Taxi,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }
    }

    #[test]
    fn basic_condition_meets_player_progression_snapshot_branches_like_cpp() {
        let target = player_object(571, 2);
        let items = [ConditionItemCountSnapshot {
            item_id: 100,
            count: 2,
            bank_count: 3,
        }];
        let equipped_item_or_gem_ids = [200];
        let skills = [ConditionSkillSnapshot {
            skill_id: 300,
            base_value: 75,
        }];
        let spell_ids = [400];
        let achievement_ids = [500];
        let reputations = [ConditionReputationSnapshot {
            faction_id: 550,
            rank: 5,
        }];
        let title_ids = [600];
        let battle_pet_counts = [ConditionBattlePetCountSnapshot {
            species_id: 700,
            count: 4,
        }];
        let active_scene_ids = [900];
        let realm_achievement_ids = [800];
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_player_progression_target_snapshot(
            0,
            ConditionPlayerProgressionSnapshot {
                items: &items,
                equipped_item_or_gem_ids: &equipped_item_or_gem_ids,
                skills: &skills,
                spell_ids: &spell_ids,
                achievement_ids: &achievement_ids,
                reputations: &reputations,
                title_ids: &title_ids,
                battle_pet_counts: &battle_pet_counts,
                active_scene_ids: &active_scene_ids,
            },
        );
        info.set_realm_achievement_ids(&realm_achievement_ids);

        let conditions = vec![
            Condition {
                condition_type: ConditionType::Item,
                condition_value1: 100,
                condition_value2: 5,
                condition_value3: 1,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::ItemEquipped,
                condition_value1: 200,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Skill,
                condition_value1: 300,
                condition_value2: 75,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Spell,
                condition_value1: 400,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Achievement,
                condition_value1: 500,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::ReputationRank,
                condition_value1: 550,
                condition_value2: 1 << 5,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::Title,
                condition_value1: 600,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::BattlePetCount,
                condition_value1: 700,
                condition_value2: 4,
                condition_value3: ComparisonType::HighEq as u32,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::RealmAchievement,
                condition_value1: 800,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::SceneInProgress,
                condition_value1: 900,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }

        let without_bank = Condition {
            condition_type: ConditionType::Item,
            condition_value1: 100,
            condition_value2: 5,
            condition_value3: 0,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&without_bank, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_player_quest_snapshot_branches_like_cpp() {
        let target = player_object(571, 2);
        let statuses = [
            ConditionQuestStatusSnapshot {
                quest_id: 10,
                status: QUEST_STATUS_INCOMPLETE_LIKE_CPP,
            },
            ConditionQuestStatusSnapshot {
                quest_id: 20,
                status: QUEST_STATUS_COMPLETE_LIKE_CPP,
            },
            ConditionQuestStatusSnapshot {
                quest_id: 30,
                status: QUEST_STATUS_FAILED_LIKE_CPP,
            },
        ];
        let objective_progress = [ConditionQuestObjectiveProgressSnapshot {
            quest_id: 10,
            objective_id: 900,
            counter: 4,
        }];
        let rewarded_quest_ids = [40];
        let daily_quest_ids = [50];
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_player_quest_target_snapshot(
            0,
            ConditionPlayerQuestSnapshot {
                statuses: &statuses,
                objective_progress: &objective_progress,
                rewarded_quest_ids: &rewarded_quest_ids,
                daily_quest_ids: &daily_quest_ids,
            },
        );

        let conditions = vec![
            Condition {
                condition_type: ConditionType::QuestTaken,
                condition_value1: 10,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestComplete,
                condition_value1: 20,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestNone,
                condition_value1: 999,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestRewarded,
                condition_value1: 40,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestState,
                condition_value1: 30,
                condition_value2: 1 << QUEST_STATUS_FAILED_LIKE_CPP,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestState,
                condition_value1: 40,
                condition_value2: 1 << QUEST_STATUS_REWARDED_LIKE_CPP,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::QuestObjectiveProgress,
                condition_value1: 900,
                condition_value3: 4,
                ..Condition::default()
            },
            Condition {
                condition_type: ConditionType::DailyQuestDone,
                condition_value1: 50,
                ..Condition::default()
            },
        ];

        for condition in &conditions {
            assert_eq!(
                condition_meets_basic_like_cpp(condition, &mut info, |_, _| false),
                ConditionMeetResult::Evaluated(true),
                "{condition:?}"
            );
        }

        let rewarded_is_not_complete = Condition {
            condition_type: ConditionType::QuestComplete,
            condition_value1: 40,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&rewarded_is_not_complete, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_stand_state_modes_like_cpp() {
        let target = world_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 1,
                health: 1,
                max_health: 1,
                class_mask: 1,
                race: 1,
                creature_type: None,
                is_alive: true,
                is_charmed: false,
                in_water: false,
                unit_state: 0,
                stand_state: UnitStandStateType::SitChair as u32,
            },
        );

        let sit_mode = Condition {
            condition_type: ConditionType::StandState,
            condition_value1: 1,
            condition_value2: 1,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&sit_mode, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let stand_mode = Condition {
            condition_type: ConditionType::StandState,
            condition_value1: 1,
            condition_value2: 0,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&stand_mode, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_creature_type_requires_creature_like_cpp() {
        let target = player_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_unit_target_snapshot(
            0,
            ConditionUnitSnapshot {
                level: 1,
                health: 1,
                max_health: 1,
                class_mask: 1,
                race: 1,
                creature_type: Some(7),
                is_alive: true,
                is_charmed: false,
                in_water: false,
                unit_state: 0,
                stand_state: UnitStandStateType::Stand as u32,
            },
        );

        let condition = Condition {
            condition_type: ConditionType::CreatureType,
            condition_value1: 7,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn basic_condition_meets_unit_branches_fail_without_unit_snapshot_like_cpp_tounit_null() {
        let target = world_object(571, 2);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        let condition = Condition {
            condition_type: ConditionType::Alive,
            ..Condition::default()
        };

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(std::ptr::eq(
            info.last_failed_condition.unwrap(),
            &condition
        ));
    }

    #[test]
    fn basic_condition_meets_missing_object_returns_false_before_negative_like_cpp() {
        let condition = Condition {
            condition_type: ConditionType::ZoneId,
            condition_value1: 67,
            negative_condition: true,
            ..Condition::default()
        };
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| true),
            ConditionMeetResult::Evaluated(false)
        );
        assert!(info.last_failed_condition.is_none());
    }

    #[test]
    fn basic_condition_meets_reports_unsupported_for_unrepresented_cpp_state() {
        let mut target = world_object(571, 2);
        target.object_mut().set_entry(1001);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);

        let spawn_condition = Condition {
            condition_type: ConditionType::ObjectEntryGuid,
            condition_value1: TypeId::Unit as u32,
            condition_value2: 1001,
            condition_value3: 77,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&spawn_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );

        let unrepresented_condition = Condition {
            condition_type: ConditionType::PlayerCondition,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&unrepresented_condition, &mut info, |_, _| false),
            ConditionMeetResult::Unsupported
        );
    }

    #[test]
    fn basic_condition_meets_player_condition_delegates_to_db2_evaluator_like_cpp() {
        let target = player_object(571, 2);
        let store = PlayerConditionStore::from_entries([PlayerConditionEntry {
            id: 970,
            race_mask: 1 << 0,
            class_mask: 1 << 1,
            gender: 1,
            ..PlayerConditionEntry::default()
        }]);
        let mut info = ConditionSourceInfo::from_targets(Some(&target), None, None);
        info.set_player_condition_store(&store);
        info.set_player_condition_context(
            0,
            PlayerConditionContextLikeCpp {
                race: 1,
                class_mask: 1 << 1,
                gender: 1,
                native_gender: 0,
                ..Default::default()
            },
        );

        let condition = Condition {
            condition_type: ConditionType::PlayerCondition,
            condition_value1: 970,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(true)
        );

        let missing_condition = Condition {
            condition_type: ConditionType::PlayerCondition,
            condition_value1: 971,
            ..Condition::default()
        };
        assert_eq!(
            condition_meets_basic_like_cpp(&missing_condition, &mut info, |_, _| false),
            ConditionMeetResult::Evaluated(false)
        );
    }

    #[test]
    fn object_meet_conditions_uses_cpp_else_group_or_of_and() {
        let conditions = vec![
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 2,
                ..Condition::default()
            },
            Condition {
                else_group: 1,
                condition_type: ConditionType::Aura,
                condition_value1: 3,
                ..Condition::default()
            },
        ];
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| condition.condition_value1 != 2,
        );

        assert!(passed);
    }

    #[test]
    fn object_meet_conditions_short_circuits_failed_group_like_cpp() {
        let conditions = vec![
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 1,
                ..Condition::default()
            },
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                condition_value1: 2,
                ..Condition::default()
            },
        ];
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);
        let mut checked = Vec::new();

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| {
                checked.push(condition.condition_value1);
                false
            },
        );

        assert!(!passed);
        assert_eq!(checked, vec![1]);
    }

    #[test]
    fn object_meet_conditions_expands_reference_conditions_like_cpp() {
        let reference_condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: 55,
            condition_type: ConditionType::Aura,
            condition_value1: 7,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([reference_condition]);
        let conditions = vec![Condition {
            condition_type: ConditionType::None,
            reference_id: 55,
            ..Condition::default()
        }];
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        let passed = is_object_meet_to_conditions_like_cpp(
            &mut info,
            &conditions,
            &store,
            |condition, _| condition.condition_value1 == 7,
        );

        assert!(passed);
    }

    #[test]
    fn not_grouped_conditions_missing_bucket_passes_like_cpp() {
        let store = ConditionEntriesByTypeStore::default();
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert!(is_object_meeting_not_grouped_conditions_like_cpp(
            &store,
            ConditionSourceType::Phase,
            42,
            &mut info,
            |_, _| false,
        ));
    }

    #[test]
    fn not_grouped_conditions_uses_zero_source_group_and_id_like_cpp() {
        let condition = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 0,
            source_entry: 42,
            source_id: 0,
            condition_type: ConditionType::Aura,
            condition_value1: 10,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let mut info = ConditionSourceInfo::from_targets(None, None, None);

        assert!(is_object_meeting_not_grouped_conditions_like_cpp(
            &store,
            ConditionSourceType::Phase,
            42,
            &mut info,
            |condition, _| condition.condition_value1 == 10,
        ));
    }

    #[test]
    fn has_not_grouped_conditions_uses_cpp_zero_group_entry_key() {
        let condition = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 0,
            source_entry: -1,
            source_id: 0,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);

        assert!(has_conditions_for_not_grouped_entry_like_cpp(
            &store,
            ConditionSourceType::Phase,
            u32::MAX,
        ));
        assert!(!has_conditions_for_not_grouped_entry_like_cpp(
            &store,
            ConditionSourceType::None,
            u32::MAX,
        ));
    }

    fn spawn_group_condition(
        spawn_group_id: u32,
        condition_type: ConditionType,
        value1: u32,
        value2: u32,
        value3: u32,
    ) -> Condition {
        Condition {
            source_type: ConditionSourceType::SpawnGroup,
            source_group: 0,
            source_entry: spawn_group_id as i32,
            source_id: 0,
            condition_type,
            condition_value1: value1,
            condition_value2: value2,
            condition_value3: value3,
            ..Condition::default()
        }
    }

    #[test]
    fn spawn_group_meeting_map_conditions_missing_bucket_passes_like_cpp() {
        let store = ConditionEntriesByTypeStore::default();

        assert!(is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            123,
            ConditionMapRef::new(571, 1),
            None,
            &[],
        ));
    }

    #[test]
    fn spawn_group_meeting_map_conditions_map_id_matches_cpp_bucket_key() {
        let spawn_group_id = 123;
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([spawn_group_condition(
            spawn_group_id,
            ConditionType::MapId,
            571,
            0,
            0,
        )]);

        assert!(is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            None,
            &[],
        ));
        assert!(!is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(530, 1),
            None,
            &[],
        ));
    }

    #[test]
    fn spawn_group_meeting_map_conditions_world_state_snapshot_matches_cpp() {
        let spawn_group_id = 124;
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([spawn_group_condition(
            spawn_group_id,
            ConditionType::WorldState,
            77,
            42,
            0,
        )]);
        let matching_world_states = [ConditionWorldStateSnapshot { id: 77, value: 42 }];
        let non_matching_world_states = [ConditionWorldStateSnapshot { id: 77, value: 7 }];
        let matching_state = ConditionMapStateSnapshot {
            active_event_ids: &[],
            world_states: &matching_world_states,
            difficulty_id: 0,
            instance_data: &[],
            instance_data64: &[],
            boss_states: &[],
            scenario_step_id: None,
        };
        let non_matching_state = ConditionMapStateSnapshot {
            active_event_ids: &[],
            world_states: &non_matching_world_states,
            difficulty_id: 0,
            instance_data: &[],
            instance_data64: &[],
            boss_states: &[],
            scenario_step_id: None,
        };

        assert!(is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            Some(matching_state),
            &[],
        ));
        assert!(!is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            Some(non_matching_state),
            &[],
        ));
    }

    #[test]
    fn spawn_group_meeting_map_conditions_realm_achievement_ids_match_cpp() {
        let spawn_group_id = 125;
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([spawn_group_condition(
            spawn_group_id,
            ConditionType::RealmAchievement,
            9001,
            0,
            0,
        )]);

        assert!(is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            None,
            &[9001],
        ));
        assert!(!is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            None,
            &[42],
        ));
    }

    #[test]
    fn spawn_group_meeting_map_conditions_map_only_unsupported_type_fails_without_panic() {
        let spawn_group_id = 126;
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([spawn_group_condition(
            spawn_group_id,
            ConditionType::Aura,
            1234,
            0,
            0,
        )]);

        assert!(!is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            None,
            &[],
        ));
    }

    #[test]
    fn spawn_group_meeting_map_conditions_reference_conditions_are_expanded_like_cpp() {
        let spawn_group_id = 127;
        let passing_reference_id = 700;
        let failing_reference_id = 701;
        let passing_spawn_group_condition = Condition {
            reference_id: passing_reference_id,
            ..spawn_group_condition(spawn_group_id, ConditionType::None, 0, 0, 0)
        };
        let failing_spawn_group_condition = Condition {
            reference_id: failing_reference_id,
            ..spawn_group_condition(spawn_group_id + 1, ConditionType::None, 0, 0, 0)
        };
        let passing_reference_condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: passing_reference_id,
            condition_type: ConditionType::MapId,
            condition_value1: 571,
            ..Condition::default()
        };
        let failing_reference_condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: failing_reference_id,
            condition_type: ConditionType::MapId,
            condition_value1: 530,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            passing_spawn_group_condition,
            failing_spawn_group_condition,
            passing_reference_condition,
            failing_reference_condition,
        ]);

        assert!(is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id,
            ConditionMapRef::new(571, 1),
            None,
            &[],
        ));
        assert!(!is_spawn_group_meeting_map_conditions_like_cpp(
            &store,
            spawn_group_id + 1,
            ConditionMapRef::new(571, 1),
            None,
            &[],
        ));
    }

    #[test]
    fn spell_click_conditions_use_cpp_key_and_target_order() {
        let condition = Condition {
            source_type: ConditionSourceType::SpellClickEvent,
            source_group: 123,
            source_entry: -1,
            source_id: 0,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let clicker = world_object(571, 1);
        let target = world_object(1, 2);

        assert!(has_conditions_for_spell_click_event_like_cpp(
            &store,
            123,
            u32::MAX,
        ));
        assert!(is_object_meeting_spell_click_conditions_like_cpp(
            &store,
            123,
            u32::MAX,
            Some(&clicker),
            Some(&target),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &clicker)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &target)
            },
        ));
    }

    #[test]
    fn can_see_spell_click_requires_flag_and_loaded_rows_like_cpp() {
        let spell_click_store = NpcSpellClickStoreLikeCpp::from_rows_like_cpp(
            [wow_data::NpcSpellClickRowLikeCpp {
                npc_entry: 123,
                spell_id: 456,
                cast_flags: 0,
                user_type: wow_data::SPELL_CLICK_USER_ANY_LIKE_CPP,
            }],
            |entry| entry == 123,
            |spell| spell == 456,
        );
        let condition_store = ConditionEntriesByTypeStore::default();
        let context = SpellClickRequirementContextLikeCpp {
            clicker_is_player: true,
            clicker_is_friendly_to_summoner: false,
            clicker_is_in_raid_with_summoner: false,
            clicker_is_in_party_with_summoner: false,
        };

        assert!(!can_see_spell_click_on_like_cpp(
            &spell_click_store,
            &condition_store,
            123,
            0,
            None,
            None,
            context,
            |_, _| true,
        ));
        assert!(!can_see_spell_click_on_like_cpp(
            &spell_click_store,
            &condition_store,
            124,
            UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP,
            None,
            None,
            context,
            |_, _| true,
        ));
        assert!(can_see_spell_click_on_like_cpp(
            &spell_click_store,
            &condition_store,
            123,
            UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP,
            None,
            None,
            context,
            |_, _| false,
        ));
    }

    #[test]
    fn can_see_spell_click_stops_on_first_failed_requirement_like_cpp() {
        let spell_click_store = NpcSpellClickStoreLikeCpp::from_rows_like_cpp(
            [
                wow_data::NpcSpellClickRowLikeCpp {
                    npc_entry: 123,
                    spell_id: 456,
                    cast_flags: 0,
                    user_type: SPELL_CLICK_USER_PARTY_LIKE_CPP,
                },
                wow_data::NpcSpellClickRowLikeCpp {
                    npc_entry: 123,
                    spell_id: 457,
                    cast_flags: 0,
                    user_type: wow_data::SPELL_CLICK_USER_ANY_LIKE_CPP,
                },
            ],
            |entry| entry == 123,
            |spell| matches!(spell, 456 | 457),
        );
        let condition_store = ConditionEntriesByTypeStore::default();
        let context = SpellClickRequirementContextLikeCpp {
            clicker_is_player: true,
            clicker_is_friendly_to_summoner: true,
            clicker_is_in_raid_with_summoner: true,
            clicker_is_in_party_with_summoner: false,
        };
        let mut condition_calls = 0;

        assert!(!can_see_spell_click_on_like_cpp(
            &spell_click_store,
            &condition_store,
            123,
            UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP,
            None,
            None,
            context,
            |_, _| {
                condition_calls += 1;
                true
            },
        ));
        assert_eq!(condition_calls, 0);
    }

    #[test]
    fn can_see_spell_click_uses_spell_conditions_until_one_passes_like_cpp() {
        let spell_click_store = NpcSpellClickStoreLikeCpp::from_rows_like_cpp(
            [
                wow_data::NpcSpellClickRowLikeCpp {
                    npc_entry: 123,
                    spell_id: 456,
                    cast_flags: 0,
                    user_type: SPELL_CLICK_USER_FRIEND_LIKE_CPP,
                },
                wow_data::NpcSpellClickRowLikeCpp {
                    npc_entry: 123,
                    spell_id: 457,
                    cast_flags: 0,
                    user_type: SPELL_CLICK_USER_RAID_LIKE_CPP,
                },
            ],
            |entry| entry == 123,
            |spell| matches!(spell, 456 | 457),
        );
        let condition_store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::SpellClickEvent,
                source_group: 123,
                source_entry: 456,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::SpellClickEvent,
                source_group: 123,
                source_entry: 457,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
        ]);
        let clicker = player_object(571, 1);
        let target = world_object(571, 1);
        let context = SpellClickRequirementContextLikeCpp {
            clicker_is_player: true,
            clicker_is_friendly_to_summoner: true,
            clicker_is_in_raid_with_summoner: true,
            clicker_is_in_party_with_summoner: false,
        };
        let mut seen_spells = Vec::new();

        assert!(can_see_spell_click_on_like_cpp(
            &spell_click_store,
            &condition_store,
            123,
            UNIT_NPC_FLAG_SPELLCLICK_LIKE_CPP,
            Some(&clicker),
            Some(&target),
            context,
            |condition, source_info| {
                seen_spells.push(condition.source_entry as u32);
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &clicker)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &target)
                    && condition.source_entry == 457
            },
        ));
        assert_eq!(seen_spells, vec![456, 457]);
    }

    #[test]
    fn vehicle_vendor_and_trainer_conditions_match_cpp_keys_and_targets() {
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::VehicleSpell,
                source_group: 10,
                source_entry: 20,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::NpcVendor,
                source_group: 30,
                source_entry: 40,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::TrainerSpell,
                source_group: 50,
                source_entry: 60,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
        ]);
        let player = world_object(571, 1);
        let other = world_object(571, 1);

        assert!(is_object_meeting_vehicle_spell_conditions_like_cpp(
            &store,
            10,
            20,
            Some(&player),
            Some(&other),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &player)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &other)
            },
        ));
        assert!(is_object_meeting_vendor_item_conditions_like_cpp(
            &store,
            30,
            40,
            Some(&player),
            Some(&other),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &player)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &other)
            },
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            50,
            60,
            Some(&player),
            |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &player),
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            30,
            40,
            Some(&player),
            |_, _| false,
        ));
    }

    #[test]
    fn smart_event_and_visibility_conditions_match_cpp_composite_keys() {
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            Condition {
                source_type: ConditionSourceType::SmartEvent,
                source_group: 8,
                source_entry: -7,
                source_id: 9,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                source_type: ConditionSourceType::ObjectIdVisibility,
                source_group: 11,
                source_entry: -1,
                source_id: 0,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
        ]);
        let unit = world_object(571, 1);
        let base = world_object(571, 1);

        assert!(is_object_meeting_smart_event_conditions_like_cpp(
            &store,
            -7,
            7,
            9,
            Some(&unit),
            Some(&base),
            |_, source_info| {
                std::ptr::eq(source_info.condition_targets[0].unwrap(), &unit)
                    && std::ptr::eq(source_info.condition_targets[1].unwrap(), &base)
            },
        ));
        assert!(
            is_object_meeting_visibility_by_object_id_conditions_like_cpp(
                &store,
                11,
                u32::MAX,
                Some(&unit),
                |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &unit),
            )
        );
    }

    #[test]
    fn area_trigger_lookup_uses_server_side_as_cpp_source_entry() {
        let client_condition = Condition {
            source_type: ConditionSourceType::AreaTrigger,
            source_group: 77,
            source_entry: 0,
            condition_type: ConditionType::Aura,
            condition_value1: 10,
            ..Condition::default()
        };
        let server_condition = Condition {
            source_type: ConditionSourceType::AreaTrigger,
            source_group: 77,
            source_entry: 1,
            condition_type: ConditionType::Aura,
            condition_value1: 20,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            client_condition,
            server_condition,
        ]);

        assert_eq!(
            conditions_for_area_trigger_like_cpp(&store, 77, false).unwrap()[0].condition_value1,
            10
        );
        assert_eq!(
            conditions_for_area_trigger_like_cpp(&store, 77, true).unwrap()[0].condition_value1,
            20
        );
    }

    #[test]
    fn loot_store_item_conditions_use_cpp_store_entry_item_key_and_looter_target() {
        let condition = Condition {
            source_type: ConditionSourceType::CreatureLootTemplate,
            source_group: 500,
            source_entry: 6948,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition]);
        let looter = world_object(571, 1);
        let context = LootStoreItemContext {
            store_kind: LootStoreKind::Creature,
            entry: 500,
            item: LootStoreItem {
                item_id: 6948,
                reference: 0,
                chance: 100.0,
                needs_quest: false,
                loot_mode: 1,
                group_id: 0,
                min_count: 1,
                max_count: 1,
            },
        };

        assert!(is_loot_store_item_meeting_conditions_like_cpp(
            &store,
            context,
            Some(&looter),
            |_, source_info| std::ptr::eq(source_info.condition_targets[0].unwrap(), &looter),
        ));

        let missing_item_context = LootStoreItemContext {
            item: LootStoreItem {
                item_id: 6949,
                ..context.item
            },
            ..context
        };
        assert!(is_loot_store_item_meeting_conditions_like_cpp(
            &store,
            missing_item_context,
            Some(&looter),
            |_, _| false,
        ));
    }

    #[test]
    fn specialized_condition_lookups_default_to_true_when_missing_like_cpp() {
        let store = ConditionEntriesByTypeStore::default();

        assert!(is_object_meeting_spell_click_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_vehicle_spell_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_smart_event_conditions_like_cpp(
            &store,
            1,
            2,
            3,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_vendor_item_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            None,
            |_, _| false,
        ));
        assert!(is_object_meeting_trainer_spell_conditions_like_cpp(
            &store,
            1,
            2,
            None,
            |_, _| false,
        ));
        assert!(
            is_object_meeting_visibility_by_object_id_conditions_like_cpp(
                &store,
                1,
                2,
                None,
                |_, _| false,
            )
        );
        assert!(conditions_for_area_trigger_like_cpp(&store, 1, false).is_none());
    }
}
