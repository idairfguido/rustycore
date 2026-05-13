// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! ConditionMgr constants ported from TrinityCore `ConditionMgr.h` and `Util.h`.

use num_derive::{FromPrimitive, ToPrimitive};

/// C++ `ConditionTypes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum ConditionType {
    None = 0,
    Aura = 1,
    Item = 2,
    ItemEquipped = 3,
    ZoneId = 4,
    ReputationRank = 5,
    Team = 6,
    Skill = 7,
    QuestRewarded = 8,
    QuestTaken = 9,
    DrunkenState = 10,
    WorldState = 11,
    ActiveEvent = 12,
    InstanceInfo = 13,
    QuestNone = 14,
    Class = 15,
    Race = 16,
    Achievement = 17,
    Title = 18,
    SpawnMaskDeprecated = 19,
    Gender = 20,
    UnitState = 21,
    MapId = 22,
    AreaId = 23,
    CreatureType = 24,
    Spell = 25,
    PhaseId = 26,
    Level = 27,
    QuestComplete = 28,
    NearCreature = 29,
    NearGameObject = 30,
    ObjectEntryGuidLegacy = 31,
    TypeMaskLegacy = 32,
    RelationTo = 33,
    ReactionTo = 34,
    DistanceTo = 35,
    Alive = 36,
    HpVal = 37,
    HpPct = 38,
    RealmAchievement = 39,
    InWater = 40,
    TerrainSwap = 41,
    StandState = 42,
    DailyQuestDone = 43,
    Charmed = 44,
    PetType = 45,
    Taxi = 46,
    QuestState = 47,
    QuestObjectiveProgress = 48,
    DifficultyId = 49,
    GameMaster = 50,
    ObjectEntryGuid = 51,
    TypeMask = 52,
    BattlePetCount = 53,
    ScenarioStep = 54,
    SceneInProgress = 55,
    PlayerCondition = 56,
    PrivateObject = 57,
    StringId = 58,
    Max = 59,
}

/// C++ `ConditionSourceType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum ConditionSourceType {
    None = 0,
    CreatureLootTemplate = 1,
    DisenchantLootTemplate = 2,
    FishingLootTemplate = 3,
    GameObjectLootTemplate = 4,
    ItemLootTemplate = 5,
    MailLootTemplate = 6,
    MillingLootTemplate = 7,
    PickpocketingLootTemplate = 8,
    ProspectingLootTemplate = 9,
    ReferenceLootTemplate = 10,
    SkinningLootTemplate = 11,
    SpellLootTemplate = 12,
    SpellImplicitTarget = 13,
    GossipMenu = 14,
    GossipMenuOption = 15,
    CreatureTemplateVehicle = 16,
    Spell = 17,
    SpellClickEvent = 18,
    QuestAvailable = 19,
    VehicleSpell = 21,
    SmartEvent = 22,
    NpcVendor = 23,
    SpellProc = 24,
    TerrainSwap = 25,
    Phase = 26,
    Graveyard = 27,
    AreaTrigger = 28,
    ConversationLine = 29,
    AreaTriggerClientTriggered = 30,
    TrainerSpell = 31,
    ObjectIdVisibility = 32,
    SpawnGroup = 33,
    ReferenceCondition = 34,
    Max = 35,
}

impl ConditionSourceType {
    pub const MAX_DB_ALLOWED: u32 = Self::ReferenceCondition as u32;
}

/// C++ `RelationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum RelationType {
    SelfRelation = 0,
    InParty = 1,
    InRaidOrParty = 2,
    OwnedBy = 3,
    PassengerOf = 4,
    CreatedBy = 5,
    Max = 6,
}

/// C++ `InstanceInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum ConditionInstanceInfo {
    Data = 0,
    GuidData = 1,
    BossState = 2,
    Data64 = 3,
}

/// C++ `MAX_CONDITION_TARGETS`.
pub const MAX_CONDITION_TARGETS: usize = 3;

/// C++ `ComparisionType` from `Util.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum ComparisonType {
    Eq = 0,
    High = 1,
    Low = 2,
    HighEq = 3,
    LowEq = 4,
    Max = 5,
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::FromPrimitive;

    #[test]
    fn condition_type_values_match_cpp() {
        assert_eq!(ConditionType::None as u32, 0);
        assert_eq!(ConditionType::SpawnMaskDeprecated as u32, 19);
        assert_eq!(ConditionType::ObjectEntryGuidLegacy as u32, 31);
        assert_eq!(ConditionType::TypeMaskLegacy as u32, 32);
        assert_eq!(ConditionType::StringId as u32, 58);
        assert_eq!(ConditionType::Max as u32, 59);
        assert_eq!(ConditionType::from_u32(20), Some(ConditionType::Gender));
    }

    #[test]
    fn condition_source_type_values_match_cpp() {
        assert_eq!(ConditionSourceType::None as u32, 0);
        assert_eq!(ConditionSourceType::QuestAvailable as u32, 19);
        assert_eq!(ConditionSourceType::VehicleSpell as u32, 21);
        assert_eq!(ConditionSourceType::TerrainSwap as u32, 25);
        assert_eq!(ConditionSourceType::Phase as u32, 26);
        assert_eq!(ConditionSourceType::SpawnGroup as u32, 33);
        assert_eq!(ConditionSourceType::MAX_DB_ALLOWED, 34);
        assert_eq!(
            ConditionSourceType::ReferenceCondition as u32,
            ConditionSourceType::MAX_DB_ALLOWED
        );
        assert_eq!(ConditionSourceType::Max as u32, 35);
        assert_eq!(ConditionSourceType::from_u32(20), None);
    }

    #[test]
    fn auxiliary_condition_values_match_cpp() {
        assert_eq!(RelationType::SelfRelation as u32, 0);
        assert_eq!(RelationType::CreatedBy as u32, 5);
        assert_eq!(RelationType::Max as u32, 6);
        assert_eq!(ConditionInstanceInfo::Data as u32, 0);
        assert_eq!(ConditionInstanceInfo::Data64 as u32, 3);
        assert_eq!(MAX_CONDITION_TARGETS, 3);
        assert_eq!(ComparisonType::Eq as u32, 0);
        assert_eq!(ComparisonType::LowEq as u32, 4);
        assert_eq!(ComparisonType::Max as u32, 5);
    }
}
