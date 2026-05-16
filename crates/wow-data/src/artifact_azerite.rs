//! Artifact and Azerite DB2 readers.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactEntry {
    pub id: u32,
    pub name: String,
    pub ui_texture_kit_id: u16,
    pub ui_name_color: i32,
    pub ui_bar_overlay_color: i32,
    pub ui_bar_background_color: i32,
    pub chr_specialization_id: u16,
    pub flags: u8,
    pub artifact_category_id: u8,
    pub ui_model_scene_id: u32,
    pub spell_visual_kit_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactAppearanceEntry {
    pub id: u32,
    pub name: String,
    pub artifact_appearance_set_id: u16,
    pub display_index: u8,
    pub unlock_player_condition_id: u32,
    pub item_appearance_modifier_id: u8,
    pub ui_swatch_color: i32,
    pub ui_model_saturation: f32,
    pub ui_model_opacity: f32,
    pub override_shapeshift_form_id: u8,
    pub override_shapeshift_display_id: u32,
    pub ui_item_appearance_id: u32,
    pub ui_alt_item_appearance_id: u32,
    pub flags: u8,
    pub ui_camera_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAppearanceSetEntry {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub display_index: u8,
    pub ui_camera_id: u16,
    pub alt_hand_ui_camera_id: u16,
    pub forge_attachment_override: i8,
    pub flags: u8,
    pub artifact_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCategoryEntry {
    pub id: u32,
    pub xp_mult_currency_id: i16,
    pub xp_mult_curve_id: i16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactPowerEntry {
    pub id: u32,
    pub display_pos: [f32; 2],
    pub artifact_id: u8,
    pub max_purchasable_rank: u8,
    pub label: i32,
    pub flags: u8,
    pub tier: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPowerLinkEntry {
    pub id: u32,
    pub power_a: u16,
    pub power_b: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPowerPickerEntry {
    pub id: u32,
    pub player_condition_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactPowerRankEntry {
    pub id: u32,
    pub rank_index: u8,
    pub spell_id: i32,
    pub item_bonus_list_id: u16,
    pub aura_points_override: f32,
    pub artifact_power_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactQuestXpEntry {
    pub id: u32,
    pub difficulty: [u32; 10],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactTierEntry {
    pub id: u32,
    pub artifact_tier: u32,
    pub max_num_traits: u32,
    pub max_artifact_knowledge: u32,
    pub knowledge_player_condition: u32,
    pub minimum_empower_knowledge: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactUnlockEntry {
    pub id: u32,
    pub power_id: u32,
    pub power_rank: u8,
    pub item_bonus_list_id: u16,
    pub player_condition_id: u32,
    pub artifact_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteEmpoweredItemEntry {
    pub id: u32,
    pub item_id: i32,
    pub azerite_tier_unlock_set_id: u32,
    pub azerite_power_set_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteEssenceEntry {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub spec_set_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteEssencePowerEntry {
    pub id: u32,
    pub source_alliance: String,
    pub source_horde: String,
    pub azerite_essence_id: i32,
    pub tier: u8,
    pub major_power_description: i32,
    pub minor_power_description: i32,
    pub major_power_actual: i32,
    pub minor_power_actual: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteItemEntry {
    pub id: u32,
    pub item_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteItemMilestonePowerEntry {
    pub id: u32,
    pub required_level: i32,
    pub azerite_power_id: i32,
    pub milestone_type: i32,
    pub auto_unlock: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzeriteKnowledgeMultiplierEntry {
    pub id: u32,
    pub multiplier: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteLevelInfoEntry {
    pub id: u32,
    pub base_experience_to_next_level: u64,
    pub minimum_experience_to_next_level: u64,
    pub item_level: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeritePowerEntry {
    pub id: u32,
    pub spell_id: i32,
    pub item_bonus_list_id: i32,
    pub spec_set_id: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeritePowerSetMemberEntry {
    pub id: u32,
    pub azerite_power_set_id: u32,
    pub azerite_power_id: i32,
    pub class: i32,
    pub tier: u8,
    pub order_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteTierUnlockEntry {
    pub id: u32,
    pub item_creation_context: u8,
    pub tier: u8,
    pub azerite_level: u8,
    pub azerite_tier_unlock_set_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzeriteTierUnlockSetEntry {
    pub id: u32,
    pub flags: i32,
}

macro_rules! db2_store {
    ($store:ident, $entry:ty) => {
        pub struct $store {
            entries: HashMap<u32, $entry>,
        }

        impl $store {
            pub fn from_entries(entries: impl IntoIterator<Item = $entry>) -> Self {
                Self {
                    entries: entries.into_iter().map(|entry| (entry.id, entry)).collect(),
                }
            }

            pub fn get(&self, id: u32) -> Option<&$entry> {
                self.entries.get(&id)
            }

            pub fn len(&self) -> usize {
                self.entries.len()
            }

            pub fn is_empty(&self) -> bool {
                self.entries.is_empty()
            }
        }
    };
}

db2_store!(ArtifactStore, ArtifactEntry);
db2_store!(ArtifactAppearanceStore, ArtifactAppearanceEntry);
db2_store!(ArtifactAppearanceSetStore, ArtifactAppearanceSetEntry);
db2_store!(ArtifactCategoryStore, ArtifactCategoryEntry);
db2_store!(ArtifactPowerStore, ArtifactPowerEntry);
db2_store!(ArtifactPowerLinkStore, ArtifactPowerLinkEntry);
db2_store!(ArtifactPowerPickerStore, ArtifactPowerPickerEntry);
db2_store!(ArtifactPowerRankStore, ArtifactPowerRankEntry);
db2_store!(ArtifactQuestXpStore, ArtifactQuestXpEntry);
db2_store!(ArtifactTierStore, ArtifactTierEntry);
db2_store!(ArtifactUnlockStore, ArtifactUnlockEntry);
db2_store!(AzeriteEmpoweredItemStore, AzeriteEmpoweredItemEntry);
db2_store!(AzeriteEssenceStore, AzeriteEssenceEntry);
db2_store!(AzeriteEssencePowerStore, AzeriteEssencePowerEntry);
db2_store!(AzeriteItemStore, AzeriteItemEntry);
db2_store!(
    AzeriteItemMilestonePowerStore,
    AzeriteItemMilestonePowerEntry
);
db2_store!(
    AzeriteKnowledgeMultiplierStore,
    AzeriteKnowledgeMultiplierEntry
);
db2_store!(AzeriteLevelInfoStore, AzeriteLevelInfoEntry);
db2_store!(AzeritePowerStore, AzeritePowerEntry);
db2_store!(AzeritePowerSetMemberStore, AzeritePowerSetMemberEntry);
db2_store!(AzeriteTierUnlockStore, AzeriteTierUnlockEntry);
db2_store!(AzeriteTierUnlockSetStore, AzeriteTierUnlockSetEntry);

impl ArtifactStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "Artifact.db2", |id, idx, r| {
            ArtifactEntry {
                id,
                name: r.get_field_string(idx, 0),
                ui_texture_kit_id: r.get_field_u16(idx, 2),
                ui_name_color: r.get_field_i32(idx, 3),
                ui_bar_overlay_color: r.get_field_i32(idx, 4),
                ui_bar_background_color: r.get_field_i32(idx, 5),
                chr_specialization_id: r.get_field_u16(idx, 6),
                flags: r.get_field_u8(idx, 7),
                artifact_category_id: r.get_field_u8(idx, 8),
                ui_model_scene_id: r.get_field_u32(idx, 9),
                spell_visual_kit_id: r.get_field_u32(idx, 10),
            }
        })
    }
}

impl ArtifactAppearanceStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactAppearance.db2", |id, idx, r| {
            ArtifactAppearanceEntry {
                id,
                name: r.get_field_string(idx, 0),
                artifact_appearance_set_id: r.get_relationship_id(idx).unwrap_or(0) as u16,
                display_index: r.get_field_u8(idx, 3),
                unlock_player_condition_id: r.get_field_u32(idx, 4),
                item_appearance_modifier_id: r.get_field_u8(idx, 5),
                ui_swatch_color: r.get_field_i32(idx, 6),
                ui_model_saturation: f32_field(r, idx, 7),
                ui_model_opacity: f32_field(r, idx, 8),
                override_shapeshift_form_id: r.get_field_u8(idx, 9),
                override_shapeshift_display_id: r.get_field_u32(idx, 10),
                ui_item_appearance_id: r.get_field_u32(idx, 11),
                ui_alt_item_appearance_id: r.get_field_u32(idx, 12),
                flags: r.get_field_u8(idx, 13),
                ui_camera_id: r.get_field_u16(idx, 14),
            }
        })
    }
}

impl ArtifactAppearanceSetStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "ArtifactAppearanceSet.db2",
            |id, idx, r| ArtifactAppearanceSetEntry {
                id,
                name: r.get_field_string(idx, 0),
                description: r.get_field_string(idx, 1),
                display_index: r.get_field_u8(idx, 3),
                ui_camera_id: r.get_field_u16(idx, 4),
                alt_hand_ui_camera_id: r.get_field_u16(idx, 5),
                forge_attachment_override: r.get_field_i8(idx, 6),
                flags: r.get_field_u8(idx, 7),
                artifact_id: r.get_relationship_id(idx).unwrap_or(0),
            },
        )
    }
}

impl ArtifactCategoryStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactCategory.db2", |id, idx, r| {
            ArtifactCategoryEntry {
                id,
                xp_mult_currency_id: r.get_field_i16(idx, 0),
                xp_mult_curve_id: r.get_field_i16(idx, 1),
            }
        })
    }
}

impl ArtifactPowerStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactPower.db2", |id, idx, r| {
            ArtifactPowerEntry {
                id,
                display_pos: [
                    f32_array_element(r, idx, 0, 0),
                    f32_array_element(r, idx, 0, 1),
                ],
                artifact_id: r.get_relationship_id(idx).unwrap_or(0) as u8,
                max_purchasable_rank: r.get_field_u8(idx, 3),
                label: r.get_field_i32(idx, 4),
                flags: r.get_field_u8(idx, 5),
                tier: r.get_field_u8(idx, 6),
            }
        })
    }
}

impl ArtifactPowerLinkStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactPowerLink.db2", |id, idx, r| {
            ArtifactPowerLinkEntry {
                id,
                power_a: r.get_field_u16(idx, 0),
                power_b: r.get_field_u16(idx, 1),
            }
        })
    }
}

impl ArtifactPowerPickerStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactPowerPicker.db2", |id, idx, r| {
            ArtifactPowerPickerEntry {
                id,
                player_condition_id: r.get_field_u32(idx, 0),
            }
        })
    }
}

impl ArtifactPowerRankStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactPowerRank.db2", |id, idx, r| {
            ArtifactPowerRankEntry {
                id,
                rank_index: r.get_field_u8(idx, 0),
                spell_id: r.get_field_i32(idx, 1),
                item_bonus_list_id: r.get_field_u16(idx, 2),
                aura_points_override: f32_field(r, idx, 3),
                artifact_power_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl ArtifactQuestXpStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactQuestXP.db2", |id, idx, r| {
            ArtifactQuestXpEntry {
                id,
                difficulty: std::array::from_fn(|i| r.get_array_element(idx, 0, i, 32)),
            }
        })
    }
}

impl ArtifactTierStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactTier.db2", |id, idx, r| {
            ArtifactTierEntry {
                id,
                artifact_tier: r.get_field_u32(idx, 0),
                max_num_traits: r.get_field_u32(idx, 1),
                max_artifact_knowledge: r.get_field_u32(idx, 2),
                knowledge_player_condition: r.get_field_u32(idx, 3),
                minimum_empower_knowledge: r.get_field_u32(idx, 4),
            }
        })
    }
}

impl ArtifactUnlockStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "ArtifactUnlock.db2", |id, idx, r| {
            ArtifactUnlockEntry {
                id,
                power_id: r.get_field_u32(idx, 0),
                power_rank: r.get_field_u8(idx, 1),
                item_bonus_list_id: r.get_field_u16(idx, 2),
                player_condition_id: r.get_field_u32(idx, 3),
                artifact_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl AzeriteEmpoweredItemStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "AzeriteEmpoweredItem.db2",
            |id, idx, r| AzeriteEmpoweredItemEntry {
                id,
                item_id: r.get_field_i32(idx, 0),
                azerite_tier_unlock_set_id: r.get_field_u32(idx, 1),
                azerite_power_set_id: r.get_field_u32(idx, 2),
            },
        )
    }
}

impl AzeriteEssenceStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeriteEssence.db2", |id, idx, r| {
            AzeriteEssenceEntry {
                id,
                name: r.get_field_string(idx, 0),
                description: r.get_field_string(idx, 1),
                spec_set_id: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl AzeriteEssencePowerStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeriteEssencePower.db2", |id, idx, r| {
            AzeriteEssencePowerEntry {
                id,
                source_alliance: r.get_field_string(idx, 0),
                source_horde: r.get_field_string(idx, 1),
                azerite_essence_id: r.get_field_i32(idx, 2),
                tier: r.get_field_u8(idx, 3),
                major_power_description: r.get_field_i32(idx, 4),
                minor_power_description: r.get_field_i32(idx, 5),
                major_power_actual: r.get_field_i32(idx, 6),
                minor_power_actual: r.get_field_i32(idx, 7),
            }
        })
    }
}

impl AzeriteItemStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeriteItem.db2", |id, idx, r| {
            AzeriteItemEntry {
                id,
                item_id: r.get_field_i32(idx, 0),
            }
        })
    }
}

impl AzeriteItemMilestonePowerStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "AzeriteItemMilestonePower.db2",
            |id, idx, r| AzeriteItemMilestonePowerEntry {
                id,
                required_level: r.get_field_i32(idx, 0),
                azerite_power_id: r.get_field_i32(idx, 1),
                milestone_type: r.get_field_i32(idx, 2),
                auto_unlock: r.get_field_i32(idx, 3),
            },
        )
    }
}

impl AzeriteKnowledgeMultiplierStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "AzeriteKnowledgeMultiplier.db2",
            |id, idx, r| AzeriteKnowledgeMultiplierEntry {
                id,
                multiplier: f32_field(r, idx, 0),
            },
        )
    }
}

impl AzeriteLevelInfoStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeriteLevelInfo.db2", |id, idx, r| {
            AzeriteLevelInfoEntry {
                id,
                base_experience_to_next_level: r.get_field_i64(idx, 0) as u64,
                minimum_experience_to_next_level: r.get_field_i64(idx, 1) as u64,
                item_level: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl AzeritePowerStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeritePower.db2", |id, idx, r| {
            AzeritePowerEntry {
                id,
                spell_id: r.get_field_i32(idx, 0),
                item_bonus_list_id: r.get_field_i32(idx, 1),
                spec_set_id: r.get_field_i32(idx, 2),
                flags: r.get_field_i32(idx, 3),
            }
        })
    }
}

impl AzeritePowerSetMemberStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "AzeritePowerSetMember.db2",
            |id, idx, r| AzeritePowerSetMemberEntry {
                id,
                azerite_power_set_id: r.get_relationship_id(idx).unwrap_or(0),
                azerite_power_id: r.get_field_i32(idx, 1),
                class: r.get_field_i32(idx, 2),
                tier: r.get_field_u8(idx, 3),
                order_index: r.get_field_i32(idx, 4),
            },
        )
    }
}

impl AzeriteTierUnlockStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AzeriteTierUnlock.db2", |id, idx, r| {
            AzeriteTierUnlockEntry {
                id,
                item_creation_context: r.get_field_u8(idx, 0),
                tier: r.get_field_u8(idx, 1),
                azerite_level: r.get_field_u8(idx, 2),
                azerite_tier_unlock_set_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl AzeriteTierUnlockSetStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "AzeriteTierUnlockSet.db2",
            |id, idx, r| AzeriteTierUnlockSetEntry {
                id,
                flags: r.get_field_i32(idx, 0),
            },
        )
    }
}

fn load_store<T, S>(
    data_dir: &str,
    locale: &str,
    file_name: &str,
    mut read: impl FnMut(u32, usize, &Wdc4Reader) -> T,
) -> Result<S>
where
    S: FromEntries<T>,
{
    let path = Path::new(data_dir).join("dbc").join(locale).join(file_name);
    let reader =
        Wdc4Reader::open(&path).with_context(|| format!("failed to open {}", path.display()))?;

    let mut entries = Vec::with_capacity(reader.total_count());
    for (id, idx) in reader.iter_records() {
        entries.push(read(id, idx, &reader));
    }

    let store = S::from_entries(entries);
    info!("Loaded {} rows from {}", store.len(), path.display());
    Ok(store)
}

fn f32_field(reader: &Wdc4Reader, record_idx: usize, field: usize) -> f32 {
    f32::from_bits(reader.get_field_u32(record_idx, field))
}

fn f32_array_element(
    reader: &Wdc4Reader,
    record_idx: usize,
    field: usize,
    array_index: usize,
) -> f32 {
    f32::from_bits(reader.get_array_element(record_idx, field, array_index, 32))
}

trait FromEntries<T> {
    fn from_entries(entries: impl IntoIterator<Item = T>) -> Self;
    fn len(&self) -> usize;
}

macro_rules! impl_from_entries {
    ($store:ident, $entry:ty) => {
        impl FromEntries<$entry> for $store {
            fn from_entries(entries: impl IntoIterator<Item = $entry>) -> Self {
                Self::from_entries(entries)
            }

            fn len(&self) -> usize {
                self.len()
            }
        }
    };
}

impl_from_entries!(ArtifactStore, ArtifactEntry);
impl_from_entries!(ArtifactAppearanceStore, ArtifactAppearanceEntry);
impl_from_entries!(ArtifactAppearanceSetStore, ArtifactAppearanceSetEntry);
impl_from_entries!(ArtifactCategoryStore, ArtifactCategoryEntry);
impl_from_entries!(ArtifactPowerStore, ArtifactPowerEntry);
impl_from_entries!(ArtifactPowerLinkStore, ArtifactPowerLinkEntry);
impl_from_entries!(ArtifactPowerPickerStore, ArtifactPowerPickerEntry);
impl_from_entries!(ArtifactPowerRankStore, ArtifactPowerRankEntry);
impl_from_entries!(ArtifactQuestXpStore, ArtifactQuestXpEntry);
impl_from_entries!(ArtifactTierStore, ArtifactTierEntry);
impl_from_entries!(ArtifactUnlockStore, ArtifactUnlockEntry);
impl_from_entries!(AzeriteEmpoweredItemStore, AzeriteEmpoweredItemEntry);
impl_from_entries!(AzeriteEssenceStore, AzeriteEssenceEntry);
impl_from_entries!(AzeriteEssencePowerStore, AzeriteEssencePowerEntry);
impl_from_entries!(AzeriteItemStore, AzeriteItemEntry);
impl_from_entries!(
    AzeriteItemMilestonePowerStore,
    AzeriteItemMilestonePowerEntry
);
impl_from_entries!(
    AzeriteKnowledgeMultiplierStore,
    AzeriteKnowledgeMultiplierEntry
);
impl_from_entries!(AzeriteLevelInfoStore, AzeriteLevelInfoEntry);
impl_from_entries!(AzeritePowerStore, AzeritePowerEntry);
impl_from_entries!(AzeritePowerSetMemberStore, AzeritePowerSetMemberEntry);
impl_from_entries!(AzeriteTierUnlockStore, AzeriteTierUnlockEntry);
impl_from_entries!(AzeriteTierUnlockSetStore, AzeriteTierUnlockSetEntry);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_quest_xp_store_preserves_cpp_difficulty_array() {
        let store = ArtifactQuestXpStore::from_entries([ArtifactQuestXpEntry {
            id: 1,
            difficulty: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        }]);

        assert_eq!(store.get(1).unwrap().difficulty[9], 9);
    }

    #[test]
    fn load_artifact_azerite_db2_subbatch_when_fixtures_exist() {
        let data_dir = "/home/server/woltk-server-core/Data";
        let locale = "esES";
        let dbc_dir = Path::new(data_dir).join("dbc").join(locale);
        if !dbc_dir.exists() {
            eprintln!(
                "Skipping test: DB2 fixture directory not found at {}",
                dbc_dir.display()
            );
            return;
        }

        macro_rules! load_if_exists {
            ($file:literal, $store:ty) => {
                if dbc_dir.join($file).exists() {
                    let _store = <$store>::load(data_dir, locale)
                        .unwrap_or_else(|error| panic!("failed to load {}: {error:#}", $file));
                }
            };
        }

        load_if_exists!("Artifact.db2", ArtifactStore);
        load_if_exists!("ArtifactAppearance.db2", ArtifactAppearanceStore);
        load_if_exists!("ArtifactAppearanceSet.db2", ArtifactAppearanceSetStore);
        load_if_exists!("ArtifactCategory.db2", ArtifactCategoryStore);
        load_if_exists!("ArtifactPower.db2", ArtifactPowerStore);
        load_if_exists!("ArtifactPowerLink.db2", ArtifactPowerLinkStore);
        load_if_exists!("ArtifactPowerPicker.db2", ArtifactPowerPickerStore);
        load_if_exists!("ArtifactPowerRank.db2", ArtifactPowerRankStore);
        load_if_exists!("ArtifactQuestXP.db2", ArtifactQuestXpStore);
        load_if_exists!("ArtifactTier.db2", ArtifactTierStore);
        load_if_exists!("ArtifactUnlock.db2", ArtifactUnlockStore);
        load_if_exists!("AzeriteEmpoweredItem.db2", AzeriteEmpoweredItemStore);
        load_if_exists!("AzeriteEssence.db2", AzeriteEssenceStore);
        load_if_exists!("AzeriteEssencePower.db2", AzeriteEssencePowerStore);
        load_if_exists!("AzeriteItem.db2", AzeriteItemStore);
        load_if_exists!(
            "AzeriteItemMilestonePower.db2",
            AzeriteItemMilestonePowerStore
        );
        load_if_exists!(
            "AzeriteKnowledgeMultiplier.db2",
            AzeriteKnowledgeMultiplierStore
        );
        load_if_exists!("AzeriteLevelInfo.db2", AzeriteLevelInfoStore);
        load_if_exists!("AzeritePower.db2", AzeritePowerStore);
        load_if_exists!("AzeritePowerSetMember.db2", AzeritePowerSetMemberStore);
        load_if_exists!("AzeriteTierUnlock.db2", AzeriteTierUnlockStore);
        load_if_exists!("AzeriteTierUnlockSet.db2", AzeriteTierUnlockSetStore);
    }
}
