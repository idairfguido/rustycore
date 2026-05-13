// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ConditionMgr` data rows.

use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use anyhow::Result;
use num_traits::FromPrimitive;
use tracing::info;
use wow_constants::{ConditionSourceType, ConditionType, TypeId, TypeMask};
use wow_database::{WorldDatabase, WorldStatements};

pub const GRID_MAP_TYPE_MASK_CORPSE: u32 = 0x01;
pub const GRID_MAP_TYPE_MASK_CREATURE: u32 = 0x02;
pub const GRID_MAP_TYPE_MASK_DYNAMIC_OBJECT: u32 = 0x04;
pub const GRID_MAP_TYPE_MASK_GAME_OBJECT: u32 = 0x08;
pub const GRID_MAP_TYPE_MASK_PLAYER: u32 = 0x10;
pub const GRID_MAP_TYPE_MASK_AREA_TRIGGER: u32 = 0x20;
pub const GRID_MAP_TYPE_MASK_SCENE_OBJECT: u32 = 0x40;
pub const GRID_MAP_TYPE_MASK_CONVERSATION: u32 = 0x80;
pub const GRID_MAP_TYPE_MASK_ALL: u32 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ConditionId {
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
}

impl ConditionId {
    pub const fn new(source_group: u32, source_entry: i32, source_id: u32) -> Self {
        Self {
            source_group,
            source_entry,
            source_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub source_type: ConditionSourceType,
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
    pub else_group: u32,
    pub condition_type: ConditionType,
    pub condition_value1: u32,
    pub condition_value2: u32,
    pub condition_value3: u32,
    pub condition_string_value1: String,
    pub error_type: u32,
    pub error_text_id: u32,
    pub reference_id: u32,
    pub script_id: u32,
    pub condition_target: u8,
    pub negative_condition: bool,
}

impl Default for Condition {
    fn default() -> Self {
        Self {
            source_type: ConditionSourceType::None,
            source_group: 0,
            source_entry: 0,
            source_id: 0,
            else_group: 0,
            condition_type: ConditionType::None,
            condition_value1: 0,
            condition_value2: 0,
            condition_value3: 0,
            condition_string_value1: String::new(),
            error_type: 0,
            error_text_id: 0,
            reference_id: 0,
            script_id: 0,
            condition_target: 0,
            negative_condition: false,
        }
    }
}

impl Condition {
    /// C++ `Condition::isLoaded`.
    pub const fn is_loaded_like_cpp(&self) -> bool {
        self.condition_type as u32 > ConditionType::None as u32
            || self.reference_id != 0
            || self.script_id != 0
    }

    pub const fn id_like_cpp(&self) -> ConditionId {
        ConditionId::new(self.source_group, self.source_entry, self.source_id)
    }

    /// C++ `Condition::GetMaxAvailableConditionTargets`.
    pub const fn max_available_condition_targets_like_cpp(&self) -> u32 {
        match self.source_type {
            ConditionSourceType::Spell
            | ConditionSourceType::SpellImplicitTarget
            | ConditionSourceType::CreatureTemplateVehicle
            | ConditionSourceType::VehicleSpell
            | ConditionSourceType::SpellClickEvent
            | ConditionSourceType::GossipMenu
            | ConditionSourceType::GossipMenuOption
            | ConditionSourceType::SmartEvent
            | ConditionSourceType::NpcVendor
            | ConditionSourceType::SpellProc => 2,
            _ => 1,
        }
    }

    /// C++ `Condition::ToString`.
    pub fn to_string_like_cpp(&self, ext: bool) -> String {
        let mut text = format!(
            "[Condition SourceType: {} ({})",
            self.source_type as u32,
            condition_source_type_name_like_cpp(self.source_type)
        );

        if condition_source_can_have_group_set_like_cpp(self.source_type) {
            text.push_str(&format!(", SourceGroup: {}", self.source_group));
        }

        text.push_str(&format!(", SourceEntry: {}", self.source_entry));

        if condition_source_can_have_id_set_like_cpp(self.source_type) {
            text.push_str(&format!(", SourceId: {}", self.source_id));
        }

        if ext {
            text.push_str(&format!(
                ", ConditionType: {} ({})",
                self.condition_type as u32,
                condition_type_name_like_cpp(self.condition_type)
            ));
        }

        text.push(']');
        text
    }

    /// C++ `Condition::GetSearcherTypeMaskForCondition`.
    pub fn get_searcher_type_mask_for_condition_like_cpp(&self) -> u32 {
        if self.negative_condition {
            return GRID_MAP_TYPE_MASK_ALL;
        }

        match self.condition_type {
            ConditionType::None
            | ConditionType::ZoneId
            | ConditionType::ActiveEvent
            | ConditionType::InstanceInfo
            | ConditionType::MapId
            | ConditionType::AreaId
            | ConditionType::NearCreature
            | ConditionType::NearGameObject
            | ConditionType::DistanceTo
            | ConditionType::WorldState
            | ConditionType::PhaseId
            | ConditionType::RealmAchievement
            | ConditionType::TerrainSwap
            | ConditionType::DifficultyId
            | ConditionType::ScenarioStep => GRID_MAP_TYPE_MASK_ALL,
            ConditionType::Aura
            | ConditionType::Class
            | ConditionType::Race
            | ConditionType::Level
            | ConditionType::RelationTo
            | ConditionType::ReactionTo
            | ConditionType::Alive
            | ConditionType::HpVal
            | ConditionType::HpPct
            | ConditionType::UnitState
            | ConditionType::InWater
            | ConditionType::StandState
            | ConditionType::Charmed => GRID_MAP_TYPE_MASK_CREATURE | GRID_MAP_TYPE_MASK_PLAYER,
            ConditionType::Item
            | ConditionType::ItemEquipped
            | ConditionType::ReputationRank
            | ConditionType::Achievement
            | ConditionType::Team
            | ConditionType::Skill
            | ConditionType::QuestRewarded
            | ConditionType::QuestTaken
            | ConditionType::QuestComplete
            | ConditionType::QuestNone
            | ConditionType::Spell
            | ConditionType::DrunkenState
            | ConditionType::Title
            | ConditionType::Gender
            | ConditionType::DailyQuestDone
            | ConditionType::PetType
            | ConditionType::Taxi
            | ConditionType::QuestState
            | ConditionType::QuestObjectiveProgress
            | ConditionType::GameMaster
            | ConditionType::BattlePetCount
            | ConditionType::SceneInProgress
            | ConditionType::PlayerCondition => GRID_MAP_TYPE_MASK_PLAYER,
            ConditionType::ObjectEntryGuid | ConditionType::ObjectEntryGuidLegacy => {
                match self.condition_value1 {
                    value if value == TypeId::Unit as u32 => GRID_MAP_TYPE_MASK_CREATURE,
                    value if value == TypeId::Player as u32 => GRID_MAP_TYPE_MASK_PLAYER,
                    value if value == TypeId::GameObject as u32 => GRID_MAP_TYPE_MASK_GAME_OBJECT,
                    value if value == TypeId::Corpse as u32 => GRID_MAP_TYPE_MASK_CORPSE,
                    value if value == TypeId::AreaTrigger as u32 => GRID_MAP_TYPE_MASK_AREA_TRIGGER,
                    _ => 0,
                }
            }
            ConditionType::TypeMask | ConditionType::TypeMaskLegacy => {
                let condition_mask = TypeMask::from_bits_truncate(self.condition_value1);
                let mut mask = 0;
                if condition_mask.intersects(TypeMask::UNIT) {
                    mask |= GRID_MAP_TYPE_MASK_CREATURE | GRID_MAP_TYPE_MASK_PLAYER;
                }
                if condition_mask.intersects(TypeMask::PLAYER) {
                    mask |= GRID_MAP_TYPE_MASK_PLAYER;
                }
                if condition_mask.intersects(TypeMask::GAME_OBJECT) {
                    mask |= GRID_MAP_TYPE_MASK_GAME_OBJECT;
                }
                if condition_mask.intersects(TypeMask::CORPSE) {
                    mask |= GRID_MAP_TYPE_MASK_CORPSE;
                }
                if condition_mask.intersects(TypeMask::AREA_TRIGGER) {
                    mask |= GRID_MAP_TYPE_MASK_AREA_TRIGGER;
                }
                mask
            }
            ConditionType::CreatureType => GRID_MAP_TYPE_MASK_CREATURE,
            ConditionType::PrivateObject => GRID_MAP_TYPE_MASK_ALL & !GRID_MAP_TYPE_MASK_PLAYER,
            ConditionType::SpawnMaskDeprecated | ConditionType::StringId | ConditionType::Max => {
                panic!(
                    "Condition::GetSearcherTypeMaskForCondition - missing condition handling for {:?}",
                    self.condition_type
                )
            }
        }
    }
}

/// C++ `ConditionMgr::CanHaveSourceGroupSet`.
pub const fn condition_source_can_have_group_set_like_cpp(
    source_type: ConditionSourceType,
) -> bool {
    matches!(
        source_type,
        ConditionSourceType::CreatureLootTemplate
            | ConditionSourceType::DisenchantLootTemplate
            | ConditionSourceType::FishingLootTemplate
            | ConditionSourceType::GameObjectLootTemplate
            | ConditionSourceType::ItemLootTemplate
            | ConditionSourceType::MailLootTemplate
            | ConditionSourceType::MillingLootTemplate
            | ConditionSourceType::PickpocketingLootTemplate
            | ConditionSourceType::ProspectingLootTemplate
            | ConditionSourceType::ReferenceLootTemplate
            | ConditionSourceType::SkinningLootTemplate
            | ConditionSourceType::SpellLootTemplate
            | ConditionSourceType::GossipMenu
            | ConditionSourceType::GossipMenuOption
            | ConditionSourceType::VehicleSpell
            | ConditionSourceType::SpellImplicitTarget
            | ConditionSourceType::SpellClickEvent
            | ConditionSourceType::SmartEvent
            | ConditionSourceType::NpcVendor
            | ConditionSourceType::Phase
            | ConditionSourceType::Graveyard
            | ConditionSourceType::AreaTrigger
            | ConditionSourceType::TrainerSpell
            | ConditionSourceType::ObjectIdVisibility
            | ConditionSourceType::ReferenceCondition
    )
}

/// C++ `ConditionMgr::CanHaveSourceIdSet`.
pub const fn condition_source_can_have_id_set_like_cpp(source_type: ConditionSourceType) -> bool {
    matches!(source_type, ConditionSourceType::SmartEvent)
}

pub const fn condition_source_type_name_like_cpp(source_type: ConditionSourceType) -> &'static str {
    match source_type {
        ConditionSourceType::None => "None",
        ConditionSourceType::CreatureLootTemplate => "Creature Loot",
        ConditionSourceType::DisenchantLootTemplate => "Disenchant Loot",
        ConditionSourceType::FishingLootTemplate => "Fishing Loot",
        ConditionSourceType::GameObjectLootTemplate => "GameObject Loot",
        ConditionSourceType::ItemLootTemplate => "Item Loot",
        ConditionSourceType::MailLootTemplate => "Mail Loot",
        ConditionSourceType::MillingLootTemplate => "Milling Loot",
        ConditionSourceType::PickpocketingLootTemplate => "Pickpocketing Loot",
        ConditionSourceType::ProspectingLootTemplate => "Prospecting Loot",
        ConditionSourceType::ReferenceLootTemplate => "Reference Loot",
        ConditionSourceType::SkinningLootTemplate => "Skinning Loot",
        ConditionSourceType::SpellLootTemplate => "Spell Loot",
        ConditionSourceType::SpellImplicitTarget => "Spell Impl. Target",
        ConditionSourceType::GossipMenu => "Gossip Menu",
        ConditionSourceType::GossipMenuOption => "Gossip Menu Option",
        ConditionSourceType::CreatureTemplateVehicle => "Creature Vehicle",
        ConditionSourceType::Spell => "Spell Expl. Target",
        ConditionSourceType::SpellClickEvent => "Spell Click Event",
        ConditionSourceType::QuestAvailable => "Quest Available",
        ConditionSourceType::VehicleSpell => "Vehicle Spell",
        ConditionSourceType::SmartEvent => "SmartScript",
        ConditionSourceType::NpcVendor => "Npc Vendor",
        ConditionSourceType::SpellProc => "Spell Proc",
        ConditionSourceType::TerrainSwap => "Terrain Swap",
        ConditionSourceType::Phase => "Phase",
        ConditionSourceType::Graveyard => "Graveyard",
        ConditionSourceType::AreaTrigger => "AreaTrigger",
        ConditionSourceType::ConversationLine => "ConversationLine",
        ConditionSourceType::AreaTriggerClientTriggered => "AreaTrigger Client Triggered",
        ConditionSourceType::TrainerSpell => "Trainer Spell",
        ConditionSourceType::ObjectIdVisibility => "Object Visibility (by ID)",
        ConditionSourceType::SpawnGroup => "Spawn Group",
        ConditionSourceType::ReferenceCondition => "Reference",
        ConditionSourceType::Max => "Unknown",
    }
}

pub const fn condition_type_name_like_cpp(condition_type: ConditionType) -> &'static str {
    match condition_type {
        ConditionType::None => "None",
        ConditionType::Aura => "Aura",
        ConditionType::Item => "Item Stored",
        ConditionType::ItemEquipped => "Item Equipped",
        ConditionType::ZoneId => "Zone",
        ConditionType::ReputationRank => "Reputation",
        ConditionType::Team => "Team",
        ConditionType::Skill => "Skill",
        ConditionType::QuestRewarded => "Quest Rewarded",
        ConditionType::QuestTaken => "Quest Taken",
        ConditionType::DrunkenState => "Drunken",
        ConditionType::WorldState => "WorldState",
        ConditionType::ActiveEvent => "Active Event",
        ConditionType::InstanceInfo => "Instance Info",
        ConditionType::QuestNone => "Quest None",
        ConditionType::Class => "Class",
        ConditionType::Race => "Race",
        ConditionType::Achievement => "Achievement",
        ConditionType::Title => "Title",
        ConditionType::SpawnMaskDeprecated => "SpawnMask",
        ConditionType::Gender => "Gender",
        ConditionType::UnitState => "Unit State",
        ConditionType::MapId => "Map",
        ConditionType::AreaId => "Area",
        ConditionType::CreatureType => "CreatureType",
        ConditionType::Spell => "Spell Known",
        ConditionType::PhaseId => "Phase",
        ConditionType::Level => "Level",
        ConditionType::QuestComplete => "Quest Completed",
        ConditionType::NearCreature => "Near Creature",
        ConditionType::NearGameObject => "Near GameObject",
        ConditionType::ObjectEntryGuidLegacy | ConditionType::ObjectEntryGuid => {
            "Object Entry or Guid"
        }
        ConditionType::TypeMaskLegacy | ConditionType::TypeMask => "Object TypeMask",
        ConditionType::RelationTo => "Relation",
        ConditionType::ReactionTo => "Reaction",
        ConditionType::DistanceTo => "Distance",
        ConditionType::Alive => "Alive",
        ConditionType::HpVal => "Health Value",
        ConditionType::HpPct => "Health Pct",
        ConditionType::RealmAchievement => "Realm Achievement",
        ConditionType::InWater => "In Water",
        ConditionType::TerrainSwap => "Terrain Swap",
        ConditionType::StandState => "Sit/stand state",
        ConditionType::DailyQuestDone => "Daily Quest Completed",
        ConditionType::Charmed => "Charmed",
        ConditionType::PetType => "Pet type",
        ConditionType::Taxi => "On Taxi",
        ConditionType::QuestState => "Quest state mask",
        ConditionType::QuestObjectiveProgress => "Quest objective progress",
        ConditionType::DifficultyId => "Map Difficulty",
        ConditionType::GameMaster => "Is Gamemaster",
        ConditionType::BattlePetCount => "BattlePet Species Learned",
        ConditionType::ScenarioStep => "On Scenario Step",
        ConditionType::SceneInProgress => "Scene In Progress",
        ConditionType::PlayerCondition => "Player Condition",
        ConditionType::PrivateObject => "Private Object",
        ConditionType::StringId => "String ID",
        ConditionType::Max => "Unknown",
    }
}

pub type ConditionContainer = Vec<Condition>;
pub type ConditionsByEntryMap = HashMap<ConditionId, Arc<ConditionContainer>>;

#[derive(Debug, Clone, Default)]
pub struct ConditionsReference {
    conditions: Weak<ConditionContainer>,
}

impl ConditionsReference {
    pub fn new(conditions: &Arc<ConditionContainer>) -> Self {
        Self {
            conditions: Arc::downgrade(conditions),
        }
    }

    pub fn upgrade(&self) -> Option<Arc<ConditionContainer>> {
        self.conditions.upgrade()
    }

    pub fn is_expired(&self) -> bool {
        self.conditions.strong_count() == 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConditionEntriesByTypeStore {
    entries: HashMap<ConditionSourceType, ConditionsByEntryMap>,
}

impl ConditionEntriesByTypeStore {
    pub fn from_conditions_like_cpp(conditions: impl IntoIterator<Item = Condition>) -> Self {
        let mut store = Self::default();
        for condition in conditions {
            store.add_condition_like_cpp(condition);
        }
        store
    }

    pub fn add_condition_like_cpp(&mut self, condition: Condition) {
        self.entries
            .entry(condition.source_type)
            .or_default()
            .entry(condition.id_like_cpp())
            .or_insert_with(|| Arc::new(Vec::new()));

        let bucket = self
            .entries
            .get_mut(&condition.source_type)
            .and_then(|by_id| by_id.get_mut(&condition.id_like_cpp()))
            .expect("condition bucket must exist after insertion");
        Arc::make_mut(bucket).push(condition);
    }

    pub fn conditions_for_like_cpp(
        &self,
        source_type: ConditionSourceType,
        id: ConditionId,
    ) -> Option<&Arc<ConditionContainer>> {
        self.entries
            .get(&source_type)
            .and_then(|by_id| by_id.get(&id))
    }

    pub fn reference_for_like_cpp(
        &self,
        source_type: ConditionSourceType,
        id: ConditionId,
    ) -> Option<ConditionsReference> {
        self.conditions_for_like_cpp(source_type, id)
            .map(ConditionsReference::new)
    }

    pub fn bucket_count(&self) -> usize {
        self.entries.values().map(HashMap::len).sum()
    }

    pub fn condition_count(&self) -> usize {
        self.entries
            .values()
            .flat_map(HashMap::values)
            .map(|conditions| conditions.len())
            .sum()
    }

    /// C++ `ConditionMgr::GetSearcherTypeMaskForConditionList`.
    pub fn get_searcher_type_mask_for_condition_list_like_cpp(
        &self,
        conditions: &[Condition],
    ) -> u32 {
        if conditions.is_empty() {
            return GRID_MAP_TYPE_MASK_ALL;
        }

        let mut else_group_searcher_type_masks = std::collections::BTreeMap::<u32, u32>::new();
        for condition in conditions {
            assert!(
                condition.is_loaded_like_cpp(),
                "ConditionMgr::GetSearcherTypeMaskForConditionList - not yet loaded condition found in list"
            );

            let group_mask = else_group_searcher_type_masks
                .entry(condition.else_group)
                .or_insert(GRID_MAP_TYPE_MASK_ALL);
            if *group_mask == 0 {
                continue;
            }

            if condition.reference_id != 0 {
                let reference_conditions = self
                    .conditions_for_like_cpp(
                        ConditionSourceType::ReferenceCondition,
                        ConditionId::new(condition.reference_id, 0, 0),
                    )
                    .expect(
                        "ConditionMgr::GetSearcherTypeMaskForConditionList - incorrect reference",
                    );
                *group_mask &= self.get_searcher_type_mask_for_condition_list_like_cpp(
                    reference_conditions.as_slice(),
                );
            } else {
                *group_mask &= condition.get_searcher_type_mask_for_condition_like_cpp();
            }
        }

        else_group_searcher_type_masks
            .values()
            .fold(0, |mask, group_mask| mask | group_mask)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionDbRowLikeCpp {
    pub source_type_or_reference_id: i32,
    pub source_group: u32,
    pub source_entry: i32,
    pub source_id: u32,
    pub else_group: u32,
    pub condition_type_or_reference: i32,
    pub condition_target: u8,
    pub condition_value1: u32,
    pub condition_value2: u32,
    pub condition_value3: u32,
    pub condition_string_value1: String,
    pub negative_condition: bool,
    pub error_type: u32,
    pub error_text_id: u32,
    pub script_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionRowSkipReason {
    SelfReference(i32),
    InvalidConditionType(i32),
    InvalidSourceType(i32),
    SourceGroupNotAllowed {
        source_type: ConditionSourceType,
        source_group: u32,
    },
    SourceIdNotAllowed {
        source_type: ConditionSourceType,
        source_id: u32,
    },
    ConditionTargetOutOfRange {
        source_type: ConditionSourceType,
        condition_target: u8,
        max_available_targets: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedConditionRow {
    pub row: ConditionDbRowLikeCpp,
    pub reason: ConditionRowSkipReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConditionLoadReport {
    pub conditions: Vec<Condition>,
    pub skipped: Vec<SkippedConditionRow>,
}

impl ConditionLoadReport {
    pub fn parsed_count(&self) -> usize {
        self.conditions.len()
    }

    pub fn into_store_like_cpp(self) -> ConditionEntriesByTypeStore {
        ConditionEntriesByTypeStore::from_conditions_like_cpp(self.conditions)
    }
}

/// C++ `ConditionMgr::LoadConditions` row-to-`Condition` conversion before source/type validation.
pub fn parse_condition_row_like_cpp(
    row: ConditionDbRowLikeCpp,
    mut script_id_for_name: impl FnMut(&str) -> u32,
) -> Result<Condition, SkippedConditionRow> {
    let mut condition = Condition {
        source_group: row.source_group,
        source_entry: row.source_entry,
        source_id: row.source_id,
        else_group: row.else_group,
        condition_target: row.condition_target,
        condition_value1: row.condition_value1,
        condition_value2: row.condition_value2,
        condition_value3: row.condition_value3,
        condition_string_value1: row.condition_string_value1.clone(),
        negative_condition: row.negative_condition,
        error_type: row.error_type,
        error_text_id: row.error_text_id,
        script_id: script_id_for_name(&row.script_name),
        ..Condition::default()
    };

    if row.condition_type_or_reference >= 0 {
        condition.condition_type = ConditionType::from_i32(row.condition_type_or_reference)
            .ok_or_else(|| SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::InvalidConditionType(
                    row.condition_type_or_reference,
                ),
            })?;
    }

    if row.source_type_or_reference_id >= 0 {
        condition.source_type = ConditionSourceType::from_i32(row.source_type_or_reference_id)
            .ok_or_else(|| SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::InvalidSourceType(row.source_type_or_reference_id),
            })?;
    }

    if row.condition_type_or_reference < 0 {
        if row.condition_type_or_reference == row.source_type_or_reference_id {
            return Err(SkippedConditionRow {
                row: row.clone(),
                reason: ConditionRowSkipReason::SelfReference(row.source_type_or_reference_id),
            });
        }

        condition.reference_id = u32::try_from(-row.condition_type_or_reference).unwrap_or(0);
    }

    if row.source_type_or_reference_id < 0 {
        condition.source_type = ConditionSourceType::ReferenceCondition;
        condition.source_group = u32::try_from(-row.source_type_or_reference_id).unwrap_or(0);
    }

    Ok(condition)
}

pub fn normalize_loaded_condition_shape_like_cpp(
    condition: &mut Condition,
) -> Result<(), ConditionRowSkipReason> {
    if condition.source_group != 0
        && !condition_source_can_have_group_set_like_cpp(condition.source_type)
    {
        return Err(ConditionRowSkipReason::SourceGroupNotAllowed {
            source_type: condition.source_type,
            source_group: condition.source_group,
        });
    }

    if condition.source_id != 0 && !condition_source_can_have_id_set_like_cpp(condition.source_type)
    {
        return Err(ConditionRowSkipReason::SourceIdNotAllowed {
            source_type: condition.source_type,
            source_id: condition.source_id,
        });
    }

    if condition.error_type != 0 && condition.source_type != ConditionSourceType::Spell {
        condition.error_type = 0;
    }

    if condition.error_text_id != 0 && condition.error_type == 0 {
        condition.error_text_id = 0;
    }

    if condition.reference_id == 0 {
        let max_available_targets = condition.max_available_condition_targets_like_cpp();
        if u32::from(condition.condition_target) >= max_available_targets {
            return Err(ConditionRowSkipReason::ConditionTargetOutOfRange {
                source_type: condition.source_type,
                condition_target: condition.condition_target,
                max_available_targets,
            });
        }
    }

    Ok(())
}

pub fn parse_condition_rows_like_cpp(
    rows: impl IntoIterator<Item = ConditionDbRowLikeCpp>,
    mut script_id_for_name: impl FnMut(&str) -> u32,
) -> ConditionLoadReport {
    let mut report = ConditionLoadReport::default();
    for row in rows {
        match parse_condition_row_like_cpp(row.clone(), &mut script_id_for_name) {
            Ok(mut condition) => match normalize_loaded_condition_shape_like_cpp(&mut condition) {
                Ok(()) => report.conditions.push(condition),
                Err(reason) => report.skipped.push(SkippedConditionRow { row, reason }),
            },
            Err(skipped) => report.skipped.push(skipped),
        }
    }
    report
}

pub async fn load_condition_rows_like_cpp(
    db: &WorldDatabase,
    script_id_for_name: impl FnMut(&str) -> u32,
) -> Result<ConditionLoadReport> {
    let stmt = db.prepare(WorldStatements::SEL_CONDITIONS);
    let mut result = db.query(&stmt).await?;
    if result.is_empty() {
        return Ok(ConditionLoadReport::default());
    }

    let mut rows = Vec::new();
    loop {
        rows.push(ConditionDbRowLikeCpp {
            source_type_or_reference_id: result.read(0),
            source_group: result.read(1),
            source_entry: result.read(2),
            source_id: result.read(3),
            else_group: result.read(4),
            condition_type_or_reference: result.read(5),
            condition_target: result.read(6),
            condition_value1: result.read(7),
            condition_value2: result.read(8),
            condition_value3: result.read(9),
            condition_string_value1: result.read_string(10),
            negative_condition: result.read(11),
            error_type: result.read(12),
            error_text_id: result.read(13),
            script_name: result.read_string(14),
        });

        if !result.next_row() {
            break;
        }
    }

    let report = parse_condition_rows_like_cpp(rows, script_id_for_name);
    info!(
        "Parsed {} conditions rows ({} skipped before validation)",
        report.parsed_count(),
        report.skipped.len()
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition_row(
        source_type: ConditionSourceType,
        condition_type: ConditionType,
    ) -> ConditionDbRowLikeCpp {
        ConditionDbRowLikeCpp {
            source_type_or_reference_id: source_type as i32,
            source_group: 0,
            source_entry: 0,
            source_id: 0,
            else_group: 0,
            condition_type_or_reference: condition_type as i32,
            condition_target: 0,
            condition_value1: 0,
            condition_value2: 0,
            condition_value3: 0,
            condition_string_value1: String::new(),
            negative_condition: false,
            error_type: 0,
            error_text_id: 0,
            script_name: String::new(),
        }
    }

    #[test]
    fn condition_default_matches_cpp_constructor() {
        let condition = Condition::default();

        assert_eq!(condition.source_type, ConditionSourceType::None);
        assert_eq!(condition.source_group, 0);
        assert_eq!(condition.source_entry, 0);
        assert_eq!(condition.source_id, 0);
        assert_eq!(condition.else_group, 0);
        assert_eq!(condition.condition_type, ConditionType::None);
        assert_eq!(condition.condition_value1, 0);
        assert_eq!(condition.condition_value2, 0);
        assert_eq!(condition.condition_value3, 0);
        assert!(condition.condition_string_value1.is_empty());
        assert_eq!(condition.error_type, 0);
        assert_eq!(condition.error_text_id, 0);
        assert_eq!(condition.reference_id, 0);
        assert_eq!(condition.script_id, 0);
        assert_eq!(condition.condition_target, 0);
        assert!(!condition.negative_condition);
        assert!(!condition.is_loaded_like_cpp());
    }

    #[test]
    fn condition_is_loaded_matches_cpp() {
        let mut condition = Condition {
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        assert!(condition.is_loaded_like_cpp());

        condition.condition_type = ConditionType::None;
        condition.reference_id = 42;
        assert!(condition.is_loaded_like_cpp());

        condition.reference_id = 0;
        condition.script_id = 7;
        assert!(condition.is_loaded_like_cpp());
    }

    #[test]
    fn source_group_and_id_flags_match_cpp() {
        assert!(condition_source_can_have_group_set_like_cpp(
            ConditionSourceType::CreatureLootTemplate,
        ));
        assert!(condition_source_can_have_group_set_like_cpp(
            ConditionSourceType::SpellClickEvent,
        ));
        assert!(condition_source_can_have_group_set_like_cpp(
            ConditionSourceType::ReferenceCondition,
        ));
        assert!(!condition_source_can_have_group_set_like_cpp(
            ConditionSourceType::QuestAvailable,
        ));

        assert!(condition_source_can_have_id_set_like_cpp(
            ConditionSourceType::SmartEvent,
        ));
        assert!(!condition_source_can_have_id_set_like_cpp(
            ConditionSourceType::SpellClickEvent,
        ));
    }

    #[test]
    fn max_available_condition_targets_matches_cpp_source_groups() {
        assert_eq!(
            Condition {
                source_type: ConditionSourceType::SpellClickEvent,
                ..Condition::default()
            }
            .max_available_condition_targets_like_cpp(),
            2
        );
        assert_eq!(
            Condition {
                source_type: ConditionSourceType::SmartEvent,
                ..Condition::default()
            }
            .max_available_condition_targets_like_cpp(),
            2
        );
        assert_eq!(
            Condition {
                source_type: ConditionSourceType::Phase,
                ..Condition::default()
            }
            .max_available_condition_targets_like_cpp(),
            1
        );
    }

    #[test]
    fn condition_to_string_matches_cpp_shape() {
        let condition = Condition {
            source_type: ConditionSourceType::SmartEvent,
            source_group: 8,
            source_entry: -7,
            source_id: 9,
            condition_type: ConditionType::ObjectEntryGuid,
            ..Condition::default()
        };

        assert_eq!(
            condition.to_string_like_cpp(false),
            "[Condition SourceType: 22 (SmartScript), SourceGroup: 8, SourceEntry: -7, SourceId: 9]"
        );
        assert_eq!(
            condition.to_string_like_cpp(true),
            "[Condition SourceType: 22 (SmartScript), SourceGroup: 8, SourceEntry: -7, SourceId: 9, ConditionType: 51 (Object Entry or Guid)]"
        );
    }

    #[test]
    fn condition_to_string_handles_reference_and_private_object_name() {
        let condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: 55,
            condition_type: ConditionType::PrivateObject,
            ..Condition::default()
        };

        assert_eq!(
            condition.to_string_like_cpp(true),
            "[Condition SourceType: 34 (Reference), SourceGroup: 55, SourceEntry: 0, ConditionType: 57 (Private Object)]"
        );
    }

    #[test]
    fn condition_searcher_type_mask_matches_cpp_direct_cases() {
        assert_eq!(
            Condition {
                condition_type: ConditionType::None,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_ALL
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::Aura,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_CREATURE | GRID_MAP_TYPE_MASK_PLAYER
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::Item,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_PLAYER
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::CreatureType,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_CREATURE
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::PrivateObject,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_ALL & !GRID_MAP_TYPE_MASK_PLAYER
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::Team,
                negative_condition: true,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_ALL
        );
    }

    #[test]
    fn condition_searcher_type_mask_object_entry_and_type_mask_match_cpp() {
        assert_eq!(
            Condition {
                condition_type: ConditionType::ObjectEntryGuid,
                condition_value1: TypeId::Unit as u32,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_CREATURE
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::ObjectEntryGuid,
                condition_value1: TypeId::Player as u32,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_PLAYER
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::ObjectEntryGuid,
                condition_value1: TypeId::GameObject as u32,
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_GAME_OBJECT
        );
        assert_eq!(
            Condition {
                condition_type: ConditionType::TypeMask,
                condition_value1: (TypeMask::UNIT | TypeMask::GAME_OBJECT).bits(),
                ..Condition::default()
            }
            .get_searcher_type_mask_for_condition_like_cpp(),
            GRID_MAP_TYPE_MASK_CREATURE
                | GRID_MAP_TYPE_MASK_PLAYER
                | GRID_MAP_TYPE_MASK_GAME_OBJECT
        );
    }

    #[test]
    fn condition_searcher_type_mask_list_ands_groups_ors_else_groups_and_expands_refs() {
        let reference_condition = Condition {
            source_type: ConditionSourceType::ReferenceCondition,
            source_group: 77,
            condition_type: ConditionType::Item,
            ..Condition::default()
        };
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([reference_condition]);
        let conditions = vec![
            Condition {
                else_group: 0,
                condition_type: ConditionType::Aura,
                ..Condition::default()
            },
            Condition {
                else_group: 0,
                reference_id: 77,
                ..Condition::default()
            },
            Condition {
                else_group: 1,
                condition_type: ConditionType::CreatureType,
                ..Condition::default()
            },
        ];

        assert_eq!(
            store.get_searcher_type_mask_for_condition_list_like_cpp(&conditions),
            GRID_MAP_TYPE_MASK_PLAYER | GRID_MAP_TYPE_MASK_CREATURE
        );
    }

    #[test]
    fn condition_id_matches_cpp_key_fields() {
        let condition = Condition {
            source_group: 12,
            source_entry: -45,
            source_id: 67,
            ..Condition::default()
        };

        assert_eq!(condition.id_like_cpp(), ConditionId::new(12, -45, 67));
    }

    #[test]
    fn parse_condition_row_maps_positive_source_and_type_like_cpp() {
        let condition = parse_condition_row_like_cpp(
            ConditionDbRowLikeCpp {
                source_type_or_reference_id: ConditionSourceType::Phase as i32,
                source_group: 12,
                source_entry: 34,
                source_id: 0,
                else_group: 2,
                condition_type_or_reference: ConditionType::Aura as i32,
                condition_target: 1,
                condition_value1: 100,
                condition_value2: 2,
                condition_value3: 3,
                condition_string_value1: String::from("string"),
                negative_condition: true,
                error_type: 4,
                error_text_id: 5,
                script_name: String::from("script"),
            },
            |name| u32::from(name == "script") * 77,
        )
        .unwrap();

        assert_eq!(condition.source_type, ConditionSourceType::Phase);
        assert_eq!(condition.condition_type, ConditionType::Aura);
        assert_eq!(condition.source_group, 12);
        assert_eq!(condition.source_entry, 34);
        assert_eq!(condition.else_group, 2);
        assert_eq!(condition.condition_target, 1);
        assert_eq!(condition.condition_value1, 100);
        assert_eq!(condition.condition_value2, 2);
        assert_eq!(condition.condition_value3, 3);
        assert_eq!(condition.condition_string_value1, "string");
        assert!(condition.negative_condition);
        assert_eq!(condition.error_type, 4);
        assert_eq!(condition.error_text_id, 5);
        assert_eq!(condition.script_id, 77);
    }

    #[test]
    fn parse_condition_row_maps_reference_template_like_cpp() {
        let condition = parse_condition_row_like_cpp(
            ConditionDbRowLikeCpp {
                source_type_or_reference_id: -500,
                source_group: 9,
                source_entry: 8,
                source_id: 7,
                else_group: 0,
                condition_type_or_reference: -(ConditionType::Aura as i32),
                condition_target: 0,
                condition_value1: 0,
                condition_value2: 0,
                condition_value3: 0,
                condition_string_value1: String::new(),
                negative_condition: false,
                error_type: 0,
                error_text_id: 0,
                script_name: String::new(),
            },
            |_| 0,
        )
        .unwrap();

        assert_eq!(
            condition.source_type,
            ConditionSourceType::ReferenceCondition
        );
        assert_eq!(condition.source_group, 500);
        assert_eq!(condition.source_entry, 8);
        assert_eq!(condition.source_id, 7);
        assert_eq!(condition.reference_id, ConditionType::Aura as u32);
        assert_eq!(condition.condition_type, ConditionType::None);
    }

    #[test]
    fn parse_condition_row_skips_self_reference_like_cpp() {
        let row = ConditionDbRowLikeCpp {
            source_type_or_reference_id: -42,
            source_group: 0,
            source_entry: 0,
            source_id: 0,
            else_group: 0,
            condition_type_or_reference: -42,
            condition_target: 0,
            condition_value1: 0,
            condition_value2: 0,
            condition_value3: 0,
            condition_string_value1: String::new(),
            negative_condition: false,
            error_type: 0,
            error_text_id: 0,
            script_name: String::new(),
        };

        let skipped = parse_condition_row_like_cpp(row, |_| 0).unwrap_err();

        assert_eq!(skipped.reason, ConditionRowSkipReason::SelfReference(-42));
    }

    #[test]
    fn parse_condition_rows_applies_cpp_source_group_and_id_shape_validation() {
        let mut invalid_group =
            condition_row(ConditionSourceType::QuestAvailable, ConditionType::Aura);
        invalid_group.source_group = 5;
        let mut invalid_id = condition_row(ConditionSourceType::Phase, ConditionType::Aura);
        invalid_id.source_id = 9;
        let report = parse_condition_rows_like_cpp([invalid_group, invalid_id], |_| 0);

        assert_eq!(report.conditions.len(), 0);
        assert_eq!(
            report.skipped[0].reason,
            ConditionRowSkipReason::SourceGroupNotAllowed {
                source_type: ConditionSourceType::QuestAvailable,
                source_group: 5,
            }
        );
        assert_eq!(
            report.skipped[1].reason,
            ConditionRowSkipReason::SourceIdNotAllowed {
                source_type: ConditionSourceType::Phase,
                source_id: 9,
            }
        );
    }

    #[test]
    fn parse_condition_rows_validates_condition_target_for_non_references_like_cpp() {
        let mut invalid = condition_row(ConditionSourceType::Phase, ConditionType::Aura);
        invalid.condition_target = 1;
        let mut valid = condition_row(ConditionSourceType::SpellClickEvent, ConditionType::Aura);
        valid.condition_target = 1;
        let mut reference = condition_row(ConditionSourceType::Phase, ConditionType::Aura);
        reference.condition_type_or_reference = -77;
        reference.condition_target = 2;

        let report = parse_condition_rows_like_cpp([invalid, valid, reference], |_| 0);

        assert_eq!(report.conditions.len(), 2);
        assert_eq!(
            report.conditions[0].source_type,
            ConditionSourceType::SpellClickEvent
        );
        assert_eq!(report.conditions[1].reference_id, 77);
        assert_eq!(
            report.skipped[0].reason,
            ConditionRowSkipReason::ConditionTargetOutOfRange {
                source_type: ConditionSourceType::Phase,
                condition_target: 1,
                max_available_targets: 1,
            }
        );
    }

    #[test]
    fn parse_condition_rows_normalizes_error_fields_like_cpp() {
        let mut non_spell = condition_row(ConditionSourceType::Phase, ConditionType::Aura);
        non_spell.error_type = 7;
        non_spell.error_text_id = 8;
        let mut spell_with_error = condition_row(ConditionSourceType::Spell, ConditionType::Aura);
        spell_with_error.error_type = 7;
        spell_with_error.error_text_id = 8;
        let mut spell_without_error =
            condition_row(ConditionSourceType::Spell, ConditionType::Aura);
        spell_without_error.error_text_id = 8;

        let report = parse_condition_rows_like_cpp(
            [non_spell, spell_with_error, spell_without_error],
            |_| 0,
        );

        assert_eq!(report.conditions.len(), 3);
        assert_eq!(report.conditions[0].error_type, 0);
        assert_eq!(report.conditions[0].error_text_id, 0);
        assert_eq!(report.conditions[1].error_type, 7);
        assert_eq!(report.conditions[1].error_text_id, 8);
        assert_eq!(report.conditions[2].error_type, 0);
        assert_eq!(report.conditions[2].error_text_id, 0);
    }

    #[test]
    fn condition_entries_store_groups_by_source_type_and_id_like_cpp() {
        let first = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 7,
            source_entry: 20,
            source_id: 0,
            condition_type: ConditionType::Aura,
            condition_value1: 100,
            ..Condition::default()
        };
        let second = Condition {
            condition_value1: 101,
            ..first.clone()
        };
        let other = Condition {
            source_type: ConditionSourceType::TerrainSwap,
            source_group: 7,
            source_entry: 20,
            source_id: 0,
            condition_type: ConditionType::MapId,
            ..Condition::default()
        };

        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            first.clone(),
            second.clone(),
            other,
        ]);

        assert_eq!(store.bucket_count(), 2);
        assert_eq!(store.condition_count(), 3);
        let phase_bucket = store
            .conditions_for_like_cpp(ConditionSourceType::Phase, first.id_like_cpp())
            .unwrap();
        assert_eq!(phase_bucket.as_slice(), &[first, second]);
    }

    #[test]
    fn conditions_reference_expires_after_store_reload_like_cpp() {
        let condition = Condition {
            source_type: ConditionSourceType::Phase,
            source_group: 7,
            source_entry: 20,
            condition_type: ConditionType::Aura,
            ..Condition::default()
        };
        let reference = {
            let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition.clone()]);
            store
                .reference_for_like_cpp(ConditionSourceType::Phase, condition.id_like_cpp())
                .unwrap()
        };

        assert!(reference.upgrade().is_none());
        assert!(reference.is_expired());
    }
}
