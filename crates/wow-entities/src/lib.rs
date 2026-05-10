//! Canonical entity model.
//!
//! C++ refs:
//! - `game/Entities/Object/Object.h`
//! - `game/Entities/Object/Object.cpp`
//! - `game/Entities/Object/ObjectGuid.h`

mod area_trigger;
mod bag;
mod conversation;
mod corpse;
mod creature;
mod dynamic_object;
mod game_object;
mod item;
mod object;
mod object_accessor;
mod pet;
mod player;
mod scene_object;
mod totem;
mod transport;
mod unit;
mod unit_subsystems;
mod update_fields;
mod vehicle;
mod world_object;

pub use area_trigger::{
    AREA_TRIGGER_DATA_BOUNDS_RADIUS_2D_BIT, AREA_TRIGGER_DATA_CASTER_BIT,
    AREA_TRIGGER_DATA_CREATING_EFFECT_GUID_BIT, AREA_TRIGGER_DATA_DECAL_PROPERTIES_ID_BIT,
    AREA_TRIGGER_DATA_DURATION_BIT, AREA_TRIGGER_DATA_EXTRA_SCALE_CURVE_BIT,
    AREA_TRIGGER_DATA_ORBIT_PATH_TARGET_BIT, AREA_TRIGGER_DATA_OVERRIDE_MOVE_CURVE_X_BIT,
    AREA_TRIGGER_DATA_OVERRIDE_MOVE_CURVE_Y_BIT, AREA_TRIGGER_DATA_OVERRIDE_MOVE_CURVE_Z_BIT,
    AREA_TRIGGER_DATA_OVERRIDE_SCALE_CURVE_BIT, AREA_TRIGGER_DATA_PARENT_BIT,
    AREA_TRIGGER_DATA_SPELL_FOR_VISUALS_BIT, AREA_TRIGGER_DATA_SPELL_ID_BIT,
    AREA_TRIGGER_DATA_SPELL_VISUAL_ID_BIT, AREA_TRIGGER_DATA_TIME_TO_TARGET_BIT,
    AREA_TRIGGER_DATA_TIME_TO_TARGET_EXTRA_SCALE_BIT, AREA_TRIGGER_DATA_TIME_TO_TARGET_POS_BIT,
    AREA_TRIGGER_DATA_TIME_TO_TARGET_SCALE_BIT, AREA_TRIGGER_DATA_VISUAL_ANIM_BIT,
    AREA_TRIGGER_FLAG_IS_SERVER_SIDE, AreaTrigger, AreaTriggerDataUpdate, AreaTriggerDataValues,
    AreaTriggerId, AreaTriggerShapeType, AreaTriggerValuesUpdate, ScaleCurveValues,
    VisualAnimValues,
};
pub use bag::{
    Bag, BagCreateError, BagCreateInfo, BagValuesUpdate, CONTAINER_DATA_NUM_SLOTS_BIT,
    CONTAINER_DATA_PARENT_BIT, CONTAINER_DATA_SLOTS_FIRST_BIT, CONTAINER_DATA_SLOTS_PARENT_BIT,
    ContainerDataUpdate, ContainerDataValues, MAX_BAG_SIZE, NULL_SLOT,
};
pub use conversation::{
    CONVERSATION_DATA_ACTORS_BIT, CONVERSATION_DATA_LAST_LINE_END_TIME_BIT,
    CONVERSATION_DATA_LINES_BIT, CONVERSATION_DATA_PARENT_BIT, CONVERSATION_DESPAWN_DELAY_MS,
    Conversation, ConversationActor, ConversationActorType, ConversationDataUpdate,
    ConversationDataValues, ConversationLine, ConversationValuesUpdate,
};
pub use corpse::{
    CORPSE_BONES_EXPIRE_SECS, CORPSE_DATA_CLASS_BIT, CORPSE_DATA_DISPLAY_ID_BIT,
    CORPSE_DATA_DYNAMIC_FLAGS_BIT, CORPSE_DATA_FACTION_TEMPLATE_BIT, CORPSE_DATA_FLAGS_BIT,
    CORPSE_DATA_GUILD_GUID_BIT, CORPSE_DATA_ITEMS_FIRST_BIT, CORPSE_DATA_ITEMS_PARENT_BIT,
    CORPSE_DATA_OWNER_BIT, CORPSE_DATA_PARENT_BIT, CORPSE_DATA_PARTY_GUID_BIT,
    CORPSE_DATA_RACE_ID_BIT, CORPSE_DATA_SEX_BIT, CORPSE_DYNFLAG_LOOTABLE, CORPSE_ITEMS,
    CORPSE_RESURRECTABLE_EXPIRE_SECS, Corpse, CorpseDataUpdate, CorpseDataValues, CorpseType,
    CorpseValuesUpdate,
};
pub use creature::{
    CREATURE_NOPATH_EVADE_TIME_MS, CREATURE_REGEN_INTERVAL_MS, CREATURE_TAPPERS_SOFT_CAP, Creature,
    CreatureAiOwnershipState, CreatureAiState, CreatureCreateLifecycleRecord,
    CreatureLifecycleMetadata, CreatureLifecyclePlan, CreatureLifecycleStats,
    CreatureLifecycleStep, CreatureLoadFromDbLifecycleRecord, CreatureModelDimensions,
    CreatureRuntimeAction, CreatureRuntimeEvadeReason, CreatureRuntimePlan, CreatureRuntimeState,
    CreatureRuntimeUpdateContext, CreatureSpawnLifecycleRecord, CreatureTemplateLifecycleRecord,
    DEFAULT_BOUNDARY_CHECK_TIME_MS, DEFAULT_CORPSE_DELAY_SECS, DEFAULT_MONSTER_SIGHT_DISTANCE,
    DEFAULT_RESPAWN_DELAY_SECS, LOOT_MODE_DEFAULT, MAX_CREATURE_SPELLS, MovementGeneratorType,
    ReactState,
};
pub use dynamic_object::{
    DYNAMIC_OBJECT_DATA_CAST_TIME_BIT, DYNAMIC_OBJECT_DATA_CASTER_BIT,
    DYNAMIC_OBJECT_DATA_PARENT_BIT, DYNAMIC_OBJECT_DATA_RADIUS_BIT,
    DYNAMIC_OBJECT_DATA_SPELL_ID_BIT, DYNAMIC_OBJECT_DATA_SPELL_VISUAL_ID_BIT,
    DYNAMIC_OBJECT_DATA_TYPE_BIT, DynamicObject, DynamicObjectDataUpdate, DynamicObjectDataValues,
    DynamicObjectType, DynamicObjectValuesUpdate,
};
pub use game_object::{
    DEFAULT_GAMEOBJECT_RESPAWN_DELAY_SECS, GAME_OBJECT_DATA_ART_KIT_BIT,
    GAME_OBJECT_DATA_CUSTOM_PARAM_BIT, GAME_OBJECT_DATA_DISPLAY_ID_BIT,
    GAME_OBJECT_DATA_FACTION_TEMPLATE_BIT, GAME_OBJECT_DATA_FLAGS_BIT, GAME_OBJECT_DATA_LEVEL_BIT,
    GAME_OBJECT_DATA_PARENT_BIT, GAME_OBJECT_DATA_PERCENT_HEALTH_BIT, GAME_OBJECT_DATA_STATE_BIT,
    GAME_OBJECT_DATA_TYPE_ID_BIT, GAMEOBJECT_DATA_CHEST_DUNGEON_ENCOUNTER,
    GAMEOBJECT_DATA_CHEST_LOOT, GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT,
    GAMEOBJECT_DATA_CHEST_PUSH_LOOT, GAMEOBJECT_DATA_CHEST_USE_GROUP_LOOT_RULES,
    GAMEOBJECT_LOOT_MODE_DEFAULT, GAMEOBJECT_TYPE_CHEST, GAMEOBJECT_TYPE_FISHING_HOLE,
    GAMEOBJECT_TYPE_GATHERING_NODE, GameObject, GameObjectDataUpdate, GameObjectDataValues,
    GameObjectLootSource, GameObjectTemplateData, GameObjectValuesUpdate, GoState, LootState,
    MAX_GAMEOBJECT_DATA,
};
pub use item::{
    APPEARANCE_MODIFIER_SLOT_BY_SPEC, ArtifactPower, BOP_TRADEABLE_DURATION_SECS,
    EQUIPMENT_SLOT_BACK, EQUIPMENT_SLOT_BODY, EQUIPMENT_SLOT_CHEST, EQUIPMENT_SLOT_END,
    EQUIPMENT_SLOT_FEET, EQUIPMENT_SLOT_FINGER1, EQUIPMENT_SLOT_FINGER2, EQUIPMENT_SLOT_HANDS,
    EQUIPMENT_SLOT_HEAD, EQUIPMENT_SLOT_LEGS, EQUIPMENT_SLOT_MAINHAND, EQUIPMENT_SLOT_NECK,
    EQUIPMENT_SLOT_OFFHAND, EQUIPMENT_SLOT_RANGED, EQUIPMENT_SLOT_SHOULDERS, EQUIPMENT_SLOT_TABARD,
    EQUIPMENT_SLOT_TRINKET1, EQUIPMENT_SLOT_TRINKET2, EQUIPMENT_SLOT_WAIST, EQUIPMENT_SLOT_WRISTS,
    ILLUSION_MODIFIER_SLOT_BY_SPEC, INVENTORY_SLOT_BAG_0, ITEM_DATA_ARTIFACT_POWERS_BIT,
    ITEM_DATA_ARTIFACT_XP_BIT, ITEM_DATA_BASE_ALLOWED_MASK, ITEM_DATA_CONTAINED_IN_BIT,
    ITEM_DATA_CONTEXT_BIT, ITEM_DATA_CREATE_PLAYED_TIME_BIT, ITEM_DATA_CREATE_TIME_BIT,
    ITEM_DATA_CREATOR_BIT, ITEM_DATA_DEBUG_ITEM_LEVEL_BIT, ITEM_DATA_DURABILITY_BIT,
    ITEM_DATA_DYNAMIC_FLAGS_BIT, ITEM_DATA_DYNAMIC_FLAGS2_BIT, ITEM_DATA_ENCHANTMENT_FIRST_BIT,
    ITEM_DATA_ENCHANTMENT_PARENT_BIT, ITEM_DATA_EXPIRATION_BIT, ITEM_DATA_GEMS_BIT,
    ITEM_DATA_GIFT_CREATOR_BIT, ITEM_DATA_ITEM_APPEARANCE_MOD_ID_BIT, ITEM_DATA_ITEM_BONUS_KEY_BIT,
    ITEM_DATA_MAX_DURABILITY_BIT, ITEM_DATA_MODIFIERS_BIT, ITEM_DATA_OWNER_ALLOWED_MASK,
    ITEM_DATA_OWNER_BIT, ITEM_DATA_PARENT_BIT, ITEM_DATA_PROPERTY_SEED_BIT,
    ITEM_DATA_RANDOM_PROPERTIES_ID_BIT, ITEM_DATA_SPELL_CHARGES_FIRST_BIT,
    ITEM_DATA_SPELL_CHARGES_PARENT_BIT, ITEM_DATA_STACK_COUNT_BIT, ITEM_MODIFIER_COUNT, Item,
    ItemBonusKey, ItemCreateInfo, ItemDataUpdate, ItemDataValues, ItemEnchantment,
    ItemStateTransition, ItemStorageTemplate, ItemValuesUpdate, MAX_ENCHANTMENT_SLOT,
    MAX_INSPECTED_ENCHANTMENT_SLOT, MAX_ITEM_SPELLS, MAX_SPECIALIZATIONS,
    PROFESSION_SLOT_COOKING_GEAR1, PROFESSION_SLOT_COOKING_TOOL, PROFESSION_SLOT_END,
    PROFESSION_SLOT_FISHING_GEAR1, PROFESSION_SLOT_FISHING_GEAR2, PROFESSION_SLOT_FISHING_TOOL,
    PROFESSION_SLOT_MAX_COUNT, PROFESSION_SLOT_PROFESSION1_GEAR1,
    PROFESSION_SLOT_PROFESSION1_GEAR2, PROFESSION_SLOT_PROFESSION1_TOOL,
    PROFESSION_SLOT_PROFESSION2_GEAR1, PROFESSION_SLOT_PROFESSION2_GEAR2,
    PROFESSION_SLOT_PROFESSION2_TOOL, PROFESSION_SLOT_START,
    SECONDARY_APPEARANCE_MODIFIER_SLOT_BY_SPEC, SocketedGem, item_can_go_into_bag,
};
pub use object::{CreateObjectFlags, EntityObject, EntityObjectState, ObjectChangedFields};
pub use object_accessor::{
    AccessorObjectKind, AccessorObjectRef, AccessorPlayer, MapObjectRecord, ObjectAccessor,
    ObjectAccessorError, ObjectAccessorMapSource, PlayerSaveError, PlayerSaveSink,
    normalize_player_name,
};
pub use pet::{
    ActiveState, HAPPINESS_LEVEL_SIZE, MAX_ACTIVE_PETS, MAX_PET_STABLES,
    PET_FOCUS_REGEN_INTERVAL_MS, PET_XP_FACTOR, Pet, PetLoadSelection, PetSaveMode, PetSpell,
    PetSpellState, PetSpellType, PetStable, PetStableInfo, PetType,
};
pub use player::{
    ACTIVE_PLAYER_DATA_BUYBACK_PARENT_BIT, ACTIVE_PLAYER_DATA_BUYBACK_PRICE_FIRST_BIT,
    ACTIVE_PLAYER_DATA_BUYBACK_TIMESTAMP_FIRST_BIT, ACTIVE_PLAYER_DATA_CHARACTER_POINTS_BIT,
    ACTIVE_PLAYER_DATA_COINAGE_BIT, ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT,
    ACTIVE_PLAYER_DATA_INV_SLOTS_PARENT_BIT, ACTIVE_PLAYER_DATA_NEXT_LEVEL_XP_BIT,
    ACTIVE_PLAYER_DATA_NUM_BACKPACK_SLOTS_BIT, ACTIVE_PLAYER_DATA_PARENT_BIT,
    ACTIVE_PLAYER_DATA_XP_BIT, ActivePlayerDataUpdate, ActivePlayerDataValues,
    ApplyEnchantmentArgs, ApplyEnchantmentBaseMod, ApplyEnchantmentCombatRating,
    ApplyEnchantmentDurationAction, ApplyEnchantmentEffectAction, ApplyEnchantmentEffectKind,
    ApplyEnchantmentEffectRef, ApplyEnchantmentGemRequirementRef, ApplyEnchantmentPlan,
    ApplyEnchantmentRandomSuffixRef, ApplyEnchantmentResult, ApplyEnchantmentSkipReason,
    ApplyEnchantmentSocketContext, ApplyEnchantmentTemplateRef, ApplyEnchantmentUnitMod,
    ApplyEnchantmentUnitModifier, ArenaEnchantmentItemRef, BANK_SLOT_BAG_END, BANK_SLOT_BAG_START,
    BANK_SLOT_ITEM_END, BANK_SLOT_ITEM_START, BUYBACK_SLOT_COUNT, BUYBACK_SLOT_END,
    BUYBACK_SLOT_START, BagTemplateRef, CHILD_EQUIPMENT_SLOT_END, CHILD_EQUIPMENT_SLOT_START,
    CLASS_HUNTER, CLASS_PALADIN, CLASS_SHAMAN, CLASS_WARRIOR, CanBankItemArgs, CanEquipItemArgs,
    CanEquipItemOutcome, CanEquipUniqueItemArgs, CanEquipUniqueItemTemplateArgs, CanStoreItemArgs,
    CanStoreItemOutcome, CanTakeMoreSimilarItemsArgs, CanTakeMoreSimilarItemsOutcome,
    CanUnequipItemArgs, CanUseItemArgs, CanUseItemTemplateArgs, DestroyFilteredItemAction,
    DestroyFilteredItemRef, DestroyItemCountAction, DestroyItemCountItemRef, DestroyItemCountPlan,
    EquipItemObjectOutcome, EquippedGemRef, FindEquipSlotArgs, INVENTORY_DEFAULT_SIZE,
    INVENTORY_SLOT_BAG_END, INVENTORY_SLOT_BAG_START, INVENTORY_SLOT_ITEM_END,
    INVENTORY_SLOT_ITEM_START, ITEM_LIMIT_CATEGORY_MODE_EQUIP, ITEM_LIMIT_CATEGORY_MODE_HAVE,
    ItemDurationRef, ItemLimitCategoryTemplate, ItemPosCount, ItemSearchCallbackResult,
    ItemSearchLocation, ItemSlotRef, ItemStorageRef, KEYRING_SLOT_END, KEYRING_SLOT_START,
    MAX_MONEY_AMOUNT, NULL_BAG, PLAYER_DATA_CURRENT_SPEC_ID_BIT, PLAYER_DATA_FLAGS_BIT,
    PLAYER_DATA_FLAGS_EX_BIT, PLAYER_DATA_LOOT_TARGET_GUID_BIT, PLAYER_DATA_NATIVE_SEX_BIT,
    PLAYER_DATA_NUM_BANK_SLOTS_BIT, PLAYER_DATA_PARENT_BIT, PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT,
    PLAYER_DATA_VISIBLE_ITEMS_PARENT_BIT, PLAYER_SLOT_END, Player, PlayerAchievementCriteriaRecord,
    PlayerAchievementRecord, PlayerActionButtonRecord, PlayerBagStorage,
    PlayerBattlegroundQueueRecord, PlayerBattlegroundState, PlayerCreateLifecycleRecord,
    PlayerCurrencyRecord, PlayerDataUpdate, PlayerDataValues, PlayerDbLoadLifecycleRecord,
    PlayerEnchantDuration, PlayerEnchantDurationItemRef, PlayerEnchantTimeUpdate,
    PlayerGameplayLoadPlan, PlayerGameplayLoadRecord, PlayerGameplayLoadStep, PlayerGameplayState,
    PlayerGroupState, PlayerGuildState, PlayerInventoryStorage, PlayerItemTimeUpdate,
    PlayerKnownSpellRecord, PlayerLifecycleMetadata, PlayerLifecyclePower,
    PlayerLoginLifecyclePlan, PlayerLoginLifecycleStep, PlayerMailRecord, PlayerPowerIndexResolver,
    PlayerQuestGameplayState, PlayerQuestObjectiveProgress, PlayerQuestStatusRecord,
    PlayerRandomBattlegroundState, PlayerReputationRecord, PlayerRestState, PlayerSkillRecord,
    PlayerSocialState, PlayerSpellChargeRecord, PlayerSpellCooldownRecord, PlayerSpellLoadState,
    PlayerStorageError, PlayerTalentRecord, PlayerTaxiState, PlayerValuesUpdate,
    PlayerWorldInsertionState, REAGENT_BAG_SLOT_END, REAGENT_BAG_SLOT_START,
    RemoveArenaEnchantmentAction, SKILL_MAIL, SKILL_PLATE_MAIL, SendNewItemArgs,
    SendNewItemDelivery, SendNewItemDisplayText, SendNewItemInstancePlan, SendNewItemModifier,
    SendNewItemPlan, SendNewItemTemplateRef, SkillEnchantmentItemRef, SkillEnchantmentTemplateRef,
    SocketedGemUniqueRef, SoulboundTradeableItemRef, SwapBagItemMove, SwapBagItemRef, SwapBagRef,
    SwapItemBagExchangePlan, SwapItemBagExchangeResult, SwapItemEmptyDestinationPlan,
    SwapItemEmptyDestinationResult, SwapItemErrorItemOrder, SwapItemMergeFillPlan,
    SwapItemMergeFillResult, SwapItemMissingPhase, SwapItemOrchestrationPlan,
    SwapItemOrchestrationResult, SwapItemPreflightItem, SwapItemPreflightPlan,
    SwapItemPreflightResult, SwapItemRealSwapExecutionPlan, SwapItemRealSwapTarget,
    SwapItemRealSwapValidationPlan, SwapItemRealSwapValidationResult,
    SwapItemRealSwapValidationSubject, TEAM_ALLIANCE_ID, TEAM_HORDE_ID, TEAM_OTHER,
    TitanGripPenaltyAction, UpdateEnchantTimeAction, UpdateItemDurationAction,
    UpdateSkillEnchantmentAction, UpdateSkillEnchantmentReason, VisibleItemValues, is_bag_pos,
    is_bank_packed_pos, is_bank_pos, is_child_equipment_packed_pos, is_child_equipment_pos,
    is_equipment_packed_pos, is_equipment_pos, is_inventory_packed_pos, is_inventory_pos,
    make_item_pos,
};
pub use scene_object::{
    SCENE_OBJECT_DATA_CREATED_BY_BIT, SCENE_OBJECT_DATA_PARENT_BIT,
    SCENE_OBJECT_DATA_RND_SEED_VAL_BIT, SCENE_OBJECT_DATA_SCENE_TYPE_BIT,
    SCENE_OBJECT_DATA_SCRIPT_PACKAGE_ID_BIT, SceneObject, SceneObjectDataUpdate,
    SceneObjectDataValues, SceneObjectValuesUpdate, SceneType,
};
pub use totem::{
    MAX_TOTEM_SLOT, SUMMON_SLOT_ANY_TOTEM, SUMMON_SLOT_TOTEM, SUMMON_SLOT_TOTEM_2,
    SUMMON_SLOT_TOTEM_3, SUMMON_SLOT_TOTEM_4, SpellAuraKind, SpellEffectKind, Totem,
    TotemCreatedPacket, TotemType, TotemUpdateOutcome, UNIT_MASK_CONTROLABLE_GUARDIAN,
    UNIT_MASK_GUARDIAN, UNIT_MASK_HUNTER_PET, UNIT_MASK_MINION, UNIT_MASK_PET, UNIT_MASK_SUMMON,
    UNIT_MASK_TOTEM,
};
pub use transport::{
    GAMEOBJECT_TYPE_MAP_OBJ_TRANSPORT, GO_DYNFLAG_LO_STOPPED, Transport, TransportCreateInfo,
    TransportMovementState, TransportPassengerSet, TransportPathEvent, TransportPathLeg,
    TransportPathSegment, TransportTemplate,
};
pub use unit::{
    BASE_MAXDAMAGE, BASE_MINDAMAGE, BASE_MOVE_SPEED, DEFAULT_PLAYER_DISPLAY_SCALE, MAX_ATTACK,
    MAX_MOVE_TYPE, MAX_POWERS, MAX_POWERS_PER_CLASS, UNIT_DATA_BOUNDING_RADIUS_BIT,
    UNIT_DATA_CLASS_ID_BIT, UNIT_DATA_COMBAT_REACH_BIT, UNIT_DATA_DISPLAY_ID_BIT,
    UNIT_DATA_DISPLAY_POWER_BIT, UNIT_DATA_DISPLAY_SCALE_BIT, UNIT_DATA_FACTION_TEMPLATE_BIT,
    UNIT_DATA_FLAGS_BIT, UNIT_DATA_FLAGS2_BIT, UNIT_DATA_FLAGS3_BIT, UNIT_DATA_HEALTH_BIT,
    UNIT_DATA_LEVEL_BIT, UNIT_DATA_MAX_HEALTH_BIT, UNIT_DATA_MAX_POWER_FIRST_BIT,
    UNIT_DATA_NATIVE_DISPLAY_ID_BIT, UNIT_DATA_NATIVE_DISPLAY_SCALE_BIT, UNIT_DATA_PARENT_BIT,
    UNIT_DATA_PLAYER_CLASS_ID_BIT, UNIT_DATA_POWER_FIRST_BIT, UNIT_DATA_POWER_PARENT_BIT,
    UNIT_DATA_RACE_BIT, UNIT_DATA_SEX_BIT, UNIT_DATA_TARGET_BIT, Unit, UnitDataUpdate,
    UnitDataValues, UnitValuesUpdate,
};
pub use unit_subsystems::{
    AiSubsystem, AppliedAuraRef, AuraRef, AuraSubsystem, CURRENT_FIRST_NON_MELEE_SPELL,
    CURRENT_MAX_SPELL, CombatSubsystem, ControlSubsystem, CurrentSpellRef, CurrentSpellSlot,
    MotionSubsystem, MoveSplineState, MovementGeneratorKind, OwnedAuraRef, SpellChargeState,
    SpellCooldown, SpellHistory, SpellSubsystem, UnitSubsystems, VehicleKitState, VehicleSubsystem,
};
pub use update_fields::{
    ACTIVE_PLAYER_DATA_BITS, AREA_TRIGGER_DATA_BITS, CONTAINER_DATA_BITS, CONVERSATION_DATA_BITS,
    CORPSE_DATA_BITS, DYNAMIC_OBJECT_DATA_BITS, GAME_OBJECT_DATA_BITS, ITEM_DATA_BITS,
    NUM_CLIENT_OBJECT_TYPES, OBJECT_DATA_BITS, OBJECT_DATA_DYNAMIC_FLAGS_BIT,
    OBJECT_DATA_ENTRY_ID_BIT, OBJECT_DATA_PARENT_BIT, OBJECT_DATA_SCALE_BIT, ObjectDataUpdate,
    ObjectDataValues, PLAYER_DATA_BITS, SCENE_OBJECT_DATA_BITS, TYPEID_ACTIVE_PLAYER,
    TYPEID_AREA_TRIGGER, TYPEID_CONTAINER, TYPEID_CONVERSATION, TYPEID_CORPSE,
    TYPEID_DYNAMIC_OBJECT, TYPEID_GAME_OBJECT, TYPEID_ITEM, TYPEID_OBJECT, TYPEID_PLAYER,
    TYPEID_SCENE_OBJECT, TYPEID_UNIT, UNIT_DATA_BITS, UpdateFieldDescriptor,
    UpdateFieldDescriptorKind, UpdateFieldSectionKind, UpdateFieldSectionMetadata,
    UpdateFieldSectionUpdate, UpdateFieldVisibilityFlags, UpdateMask, ValuesUpdate,
    ValuesUpdateSections, allowed_mask_for_visibility, base_allowed_mask_for_section,
    extra_allowed_mask_for_visibility, filter_disallowed_fields,
};
pub use vehicle::{
    MAX_VEHICLE_SEATS, PassengerInfo, Vehicle, VehicleAccessory, VehicleExitParameter, VehicleFlag,
    VehicleSeat, VehicleSeatAddon, VehicleSeatInfo, VehicleStatus, VehicleTemplate,
    calculate_passenger_offset, calculate_passenger_position,
};
pub use world_object::{
    DEFAULT_HEIGHT_SEARCH, DEFAULT_VISIBILITY_DISTANCE, DEFAULT_VISIBILITY_INSTANCE,
    INVALID_HEIGHT, LineOfSightEndpoint, LineOfSightOptions, LineOfSightQuery, MAPID_INVALID,
    MAX_HEIGHT, MAX_VISIBILITY_DISTANCE, MapBindingError, PhaseShift, SIGHT_RANGE_UNIT,
    WorldLocation, WorldObject, WorldObjectEnvironment, WorldObjectHeightQuery,
    Z_OFFSET_FIND_HEIGHT,
};
