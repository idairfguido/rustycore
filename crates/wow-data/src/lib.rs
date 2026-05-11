// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Game data file readers (DB2/WDC4).

pub mod area_trigger;
pub mod chr_specialization;
pub mod currency;
pub mod dungeon_encounter;
pub mod hotfix_cache;
pub mod import_price;
pub mod item;
pub mod item_appearance;
pub mod item_class;
pub mod item_currency_cost;
pub mod item_disenchant_loot;
pub mod item_extended_cost;
pub mod item_modified_appearance;
pub mod item_price_base;
pub mod item_random_enchantment;
pub mod item_random_properties;
pub mod item_random_suffix;
pub mod item_stats;
pub mod lock;
pub mod map;
pub mod player_power;
pub mod player_stats;
pub mod quest;
pub mod quest_xp;
pub mod rand_prop_points;
pub mod skill;
pub mod spell;
pub mod spell_item_enchantment;
pub mod wdc4;

pub use area_trigger::{
    AreaTriggerData, AreaTriggerStore, AreaTriggerTeleport, TriggerShape, load_area_triggers,
};
pub use chr_specialization::{ChrSpecializationEntry, ChrSpecializationStore};
pub use currency::{CurrencyTypesEntry, CurrencyTypesStore};
pub use dungeon_encounter::{DungeonEncounterEntry, DungeonEncounterStore};
pub use hotfix_cache::{
    HotfixBlobCache, HotfixId, HotfixRecord, HotfixRecordStatus, build_hotfix_blob_cache,
    hotfix_locale_mask,
};
pub use import_price::{
    ImportPriceArmorEntry, ImportPriceArmorStore, ImportPriceQualityEntry, ImportPriceQualityStore,
    ImportPriceShieldEntry, ImportPriceShieldStore, ImportPriceStores, ImportPriceWeaponEntry,
    ImportPriceWeaponStore,
};
pub use item::{ItemRecord, ItemStore};
pub use item_appearance::{ItemAppearanceEntry, ItemAppearanceStore};
pub use item_class::{ItemClassEntry, ItemClassStore};
pub use item_currency_cost::{ItemCurrencyCostEntry, ItemCurrencyCostStore};
pub use item_disenchant_loot::{ItemDisenchantLootEntry, ItemDisenchantLootStore};
pub use item_extended_cost::{
    ItemExtendedCostEntry, ItemExtendedCostStore, MAX_ITEM_EXT_COST_CURRENCIES,
    MAX_ITEM_EXT_COST_ITEMS,
};
pub use item_modified_appearance::{ItemModifiedAppearanceEntry, ItemModifiedAppearanceStore};
pub use item_price_base::{ItemPriceBaseEntry, ItemPriceBaseStore};
pub use item_random_enchantment::{
    ItemRandomEnchantmentTemplateEntry, ItemRandomEnchantmentTemplateStore,
};
pub use item_random_properties::{ItemRandomPropertiesEntry, ItemRandomPropertiesStore};
pub use item_random_suffix::{ItemRandomSuffixEntry, ItemRandomSuffixStore};
pub use item_stats::{
    ItemRandomPropertyTemplateEntry, ItemSparseTemplateEntry, ItemStatEntry, ItemStatsStore,
};
pub use lock::{LockEntry, LockStore};
pub use map::{MapDifficultyEntry, MapDifficultyStore, MapEntry, MapStore};
pub use player_power::{
    ClassPowerIndexRecord, Db2PlayerPowerIndexResolver, PlayerClassPowerIndexStore,
};
pub use player_stats::{PlayerLevelStats, PlayerStatsStore};
pub use rand_prop_points::{RandPropPointsEntry, RandPropPointsStore};
pub use skill::{SkillInfoEntry, SkillStore};
pub use spell::{SpellInfo, SpellStore};
pub use spell_item_enchantment::{SpellItemEnchantmentEntry, SpellItemEnchantmentStore};
