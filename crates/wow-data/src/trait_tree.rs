//! Trait tree DB2 readers.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitCondEntry {
    pub id: u32,
    pub cond_type: i32,
    pub trait_tree_id: u32,
    pub granted_ranks: i32,
    pub quest_id: i32,
    pub achievement_id: i32,
    pub spec_set_id: i32,
    pub trait_node_group_id: i32,
    pub trait_node_id: i32,
    pub trait_currency_id: i32,
    pub spent_amount_required: i32,
    pub flags: i32,
    pub required_level: i32,
    pub free_shared_string_id: i32,
    pub spend_more_shared_string_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitCostEntry {
    pub id: u32,
    pub internal_name: String,
    pub amount: i32,
    pub trait_currency_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitCurrencyEntry {
    pub id: u32,
    pub currency_type: i32,
    pub currency_types_id: i32,
    pub flags: i32,
    pub icon: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitCurrencySourceEntry {
    pub id: u32,
    pub requirement: String,
    pub trait_currency_id: u32,
    pub amount: i32,
    pub quest_id: i32,
    pub achievement_id: i32,
    pub player_level: i32,
    pub trait_node_entry_id: i32,
    pub order_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinitionEntry {
    pub id: u32,
    pub override_name: String,
    pub override_subtext: String,
    pub override_description: String,
    pub spell_id: i32,
    pub override_icon: i32,
    pub overrides_spell_id: i32,
    pub visible_spell_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitDefinitionEffectPointsEntry {
    pub id: u32,
    pub trait_definition_id: u32,
    pub effect_index: i32,
    pub operation_type: i32,
    pub curve_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitEdgeEntry {
    pub id: u32,
    pub visual_style: i32,
    pub left_trait_node_id: u32,
    pub right_trait_node_id: i32,
    pub edge_type: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeEntry {
    pub id: u32,
    pub trait_tree_id: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub node_type: u8,
    pub flags: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeEntryEntry {
    pub id: u32,
    pub trait_definition_id: i32,
    pub max_ranks: i32,
    pub node_entry_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeEntryXTraitCondEntry {
    pub id: u32,
    pub trait_cond_id: i32,
    pub trait_node_entry_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeEntryXTraitCostEntry {
    pub id: u32,
    pub trait_node_entry_id: u32,
    pub trait_cost_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeGroupEntry {
    pub id: u32,
    pub trait_tree_id: u32,
    pub flags: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeGroupXTraitCondEntry {
    pub id: u32,
    pub trait_cond_id: i32,
    pub trait_node_group_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeGroupXTraitCostEntry {
    pub id: u32,
    pub trait_node_group_id: u32,
    pub trait_cost_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeGroupXTraitNodeEntry {
    pub id: u32,
    pub trait_node_group_id: u32,
    pub trait_node_id: i32,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeXTraitCondEntry {
    pub id: u32,
    pub trait_cond_id: i32,
    pub trait_node_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeXTraitCostEntry {
    pub id: u32,
    pub trait_node_id: u32,
    pub trait_cost_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitNodeXTraitNodeEntryEntry {
    pub id: u32,
    pub trait_node_id: u32,
    pub trait_node_entry_id: i32,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitTreeEntry {
    pub id: u32,
    pub trait_system_id: u32,
    pub unused1000_1: i32,
    pub first_trait_node_id: i32,
    pub player_condition_id: i32,
    pub flags: i32,
    pub unused1000_2: f32,
    pub unused1000_3: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTreeLoadoutEntry {
    pub id: u32,
    pub trait_tree_id: u32,
    pub chr_specialization_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTreeLoadoutEntryEntry {
    pub id: u32,
    pub trait_tree_loadout_id: u32,
    pub selected_trait_node_id: i32,
    pub selected_trait_node_entry_id: i32,
    pub num_points: i32,
    pub order_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTreeXTraitCostEntry {
    pub id: u32,
    pub trait_tree_id: u32,
    pub trait_cost_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitTreeXTraitCurrencyEntry {
    pub id: u32,
    pub index: i32,
    pub trait_tree_id: u32,
    pub trait_currency_id: i32,
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

db2_store!(TraitCondStore, TraitCondEntry);
db2_store!(TraitCostStore, TraitCostEntry);
db2_store!(TraitCurrencyStore, TraitCurrencyEntry);
db2_store!(TraitCurrencySourceStore, TraitCurrencySourceEntry);
db2_store!(TraitDefinitionStore, TraitDefinitionEntry);
db2_store!(
    TraitDefinitionEffectPointsStore,
    TraitDefinitionEffectPointsEntry
);
db2_store!(TraitEdgeStore, TraitEdgeEntry);
db2_store!(TraitNodeStore, TraitNodeEntry);
db2_store!(TraitNodeEntryStore, TraitNodeEntryEntry);
db2_store!(TraitNodeEntryXTraitCondStore, TraitNodeEntryXTraitCondEntry);
db2_store!(TraitNodeEntryXTraitCostStore, TraitNodeEntryXTraitCostEntry);
db2_store!(TraitNodeGroupStore, TraitNodeGroupEntry);
db2_store!(TraitNodeGroupXTraitCondStore, TraitNodeGroupXTraitCondEntry);
db2_store!(TraitNodeGroupXTraitCostStore, TraitNodeGroupXTraitCostEntry);
db2_store!(TraitNodeGroupXTraitNodeStore, TraitNodeGroupXTraitNodeEntry);
db2_store!(TraitNodeXTraitCondStore, TraitNodeXTraitCondEntry);
db2_store!(TraitNodeXTraitCostStore, TraitNodeXTraitCostEntry);
db2_store!(TraitNodeXTraitNodeEntryStore, TraitNodeXTraitNodeEntryEntry);
db2_store!(TraitTreeStore, TraitTreeEntry);
db2_store!(TraitTreeLoadoutStore, TraitTreeLoadoutEntry);
db2_store!(TraitTreeLoadoutEntryStore, TraitTreeLoadoutEntryEntry);
db2_store!(TraitTreeXTraitCostStore, TraitTreeXTraitCostEntry);
db2_store!(TraitTreeXTraitCurrencyStore, TraitTreeXTraitCurrencyEntry);

impl TraitCondStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitCond.db2", |id, idx, r| {
            TraitCondEntry {
                id,
                cond_type: r.get_field_i32(idx, 1),
                trait_tree_id: r.get_field_u32(idx, 2),
                granted_ranks: r.get_field_i32(idx, 3),
                quest_id: r.get_field_i32(idx, 4),
                achievement_id: r.get_field_i32(idx, 5),
                spec_set_id: r.get_field_i32(idx, 6),
                trait_node_group_id: r.get_field_i32(idx, 7),
                trait_node_id: r.get_field_i32(idx, 8),
                trait_currency_id: r.get_field_i32(idx, 9),
                spent_amount_required: r.get_field_i32(idx, 10),
                flags: r.get_field_i32(idx, 11),
                required_level: r.get_field_i32(idx, 12),
                free_shared_string_id: r.get_field_i32(idx, 13),
                spend_more_shared_string_id: r.get_field_i32(idx, 14),
            }
        })
    }
}

impl TraitCostStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitCost.db2", |id, idx, r| {
            TraitCostEntry {
                id,
                internal_name: r.get_field_string(idx, 0),
                amount: r.get_field_i32(idx, 2),
                trait_currency_id: r.get_field_i32(idx, 3),
            }
        })
    }
}

impl TraitCurrencyStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitCurrency.db2", |id, idx, r| {
            TraitCurrencyEntry {
                id,
                currency_type: r.get_field_i32(idx, 1),
                currency_types_id: r.get_field_i32(idx, 2),
                flags: r.get_field_i32(idx, 3),
                icon: r.get_field_i32(idx, 4),
            }
        })
    }
}

impl TraitCurrencySourceStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitCurrencySource.db2", |id, idx, r| {
            TraitCurrencySourceEntry {
                id,
                requirement: r.get_field_string(idx, 0),
                trait_currency_id: r.get_field_u32(idx, 2),
                amount: r.get_field_i32(idx, 3),
                quest_id: r.get_field_i32(idx, 4),
                achievement_id: r.get_field_i32(idx, 5),
                player_level: r.get_field_i32(idx, 6),
                trait_node_entry_id: r.get_field_i32(idx, 7),
                order_index: r.get_field_i32(idx, 8),
            }
        })
    }
}

impl TraitDefinitionStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitDefinition.db2", |id, idx, r| {
            TraitDefinitionEntry {
                id,
                override_name: r.get_field_string(idx, 0),
                override_subtext: r.get_field_string(idx, 1),
                override_description: r.get_field_string(idx, 2),
                spell_id: r.get_field_i32(idx, 4),
                override_icon: r.get_field_i32(idx, 5),
                overrides_spell_id: r.get_field_i32(idx, 6),
                visible_spell_id: r.get_field_i32(idx, 7),
            }
        })
    }
}

impl TraitDefinitionEffectPointsStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitDefinitionEffectPoints.db2",
            |id, idx, r| TraitDefinitionEffectPointsEntry {
                id,
                trait_definition_id: r.get_relationship_id(idx).unwrap_or(0),
                effect_index: r.get_field_i32(idx, 2),
                operation_type: r.get_field_i32(idx, 3),
                curve_id: r.get_field_i32(idx, 4),
            },
        )
    }
}

impl TraitEdgeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitEdge.db2", |id, idx, r| {
            TraitEdgeEntry {
                id,
                visual_style: r.get_field_i32(idx, 1),
                left_trait_node_id: r.get_relationship_id(idx).unwrap_or(0),
                right_trait_node_id: r.get_field_i32(idx, 3),
                edge_type: r.get_field_i32(idx, 4),
            }
        })
    }
}

impl TraitNodeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitNode.db2", |id, idx, r| {
            TraitNodeEntry {
                id,
                trait_tree_id: r.get_relationship_id(idx).unwrap_or(0),
                pos_x: r.get_field_i32(idx, 2),
                pos_y: r.get_field_i32(idx, 3),
                node_type: r.get_field_u8(idx, 4),
                flags: r.get_field_i32(idx, 5),
            }
        })
    }
}

impl TraitNodeEntryStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitNodeEntry.db2", |id, idx, r| {
            TraitNodeEntryEntry {
                id,
                trait_definition_id: r.get_field_i32(idx, 1),
                max_ranks: r.get_field_i32(idx, 2),
                node_entry_type: r.get_field_u8(idx, 3),
            }
        })
    }
}

impl TraitNodeEntryXTraitCondStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeEntryXTraitCond.db2",
            |id, idx, r| TraitNodeEntryXTraitCondEntry {
                id,
                trait_cond_id: r.get_field_i32(idx, 1),
                trait_node_entry_id: r.get_relationship_id(idx).unwrap_or(0),
            },
        )
    }
}

impl TraitNodeEntryXTraitCostStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeEntryXTraitCost.db2",
            |id, idx, r| TraitNodeEntryXTraitCostEntry {
                id,
                trait_node_entry_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_cost_id: r.get_field_i32(idx, 2),
            },
        )
    }
}

impl TraitNodeGroupStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitNodeGroup.db2", |id, idx, r| {
            TraitNodeGroupEntry {
                id,
                trait_tree_id: r.get_relationship_id(idx).unwrap_or(0),
                flags: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl TraitNodeGroupXTraitCondStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeGroupXTraitCond.db2",
            |id, idx, r| TraitNodeGroupXTraitCondEntry {
                id,
                trait_cond_id: r.get_field_i32(idx, 1),
                trait_node_group_id: r.get_relationship_id(idx).unwrap_or(0),
            },
        )
    }
}

impl TraitNodeGroupXTraitCostStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeGroupXTraitCost.db2",
            |id, idx, r| TraitNodeGroupXTraitCostEntry {
                id,
                trait_node_group_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_cost_id: r.get_field_i32(idx, 2),
            },
        )
    }
}

impl TraitNodeGroupXTraitNodeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeGroupXTraitNode.db2",
            |id, idx, r| TraitNodeGroupXTraitNodeEntry {
                id,
                trait_node_group_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_node_id: r.get_field_i32(idx, 2),
                index: r.get_field_i32(idx, 3),
            },
        )
    }
}

impl TraitNodeXTraitCondStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitNodeXTraitCond.db2", |id, idx, r| {
            TraitNodeXTraitCondEntry {
                id,
                trait_cond_id: r.get_field_i32(idx, 1),
                trait_node_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl TraitNodeXTraitCostStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitNodeXTraitCost.db2", |id, idx, r| {
            TraitNodeXTraitCostEntry {
                id,
                trait_node_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_cost_id: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl TraitNodeXTraitNodeEntryStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitNodeXTraitNodeEntry.db2",
            |id, idx, r| TraitNodeXTraitNodeEntryEntry {
                id,
                trait_node_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_node_entry_id: r.get_field_i32(idx, 2),
                index: r.get_field_i32(idx, 3),
            },
        )
    }
}

impl TraitTreeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitTree.db2", |id, idx, r| {
            TraitTreeEntry {
                id,
                trait_system_id: r.get_field_u32(idx, 1),
                unused1000_1: r.get_field_i32(idx, 2),
                first_trait_node_id: r.get_field_i32(idx, 3),
                player_condition_id: r.get_field_i32(idx, 4),
                flags: r.get_field_i32(idx, 5),
                unused1000_2: f32_field(r, idx, 6),
                unused1000_3: f32_field(r, idx, 7),
            }
        })
    }
}

impl TraitTreeLoadoutStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitTreeLoadout.db2", |id, idx, r| {
            TraitTreeLoadoutEntry {
                id,
                trait_tree_id: r.get_relationship_id(idx).unwrap_or(0),
                chr_specialization_id: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl TraitTreeLoadoutEntryStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitTreeLoadoutEntry.db2",
            |id, idx, r| TraitTreeLoadoutEntryEntry {
                id,
                trait_tree_loadout_id: r.get_relationship_id(idx).unwrap_or(0),
                selected_trait_node_id: r.get_field_i32(idx, 2),
                selected_trait_node_entry_id: r.get_field_i32(idx, 3),
                num_points: r.get_field_i32(idx, 4),
                order_index: r.get_field_i32(idx, 5),
            },
        )
    }
}

impl TraitTreeXTraitCostStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TraitTreeXTraitCost.db2", |id, idx, r| {
            TraitTreeXTraitCostEntry {
                id,
                trait_tree_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_cost_id: r.get_field_i32(idx, 2),
            }
        })
    }
}

impl TraitTreeXTraitCurrencyStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(
            data_dir,
            locale,
            "TraitTreeXTraitCurrency.db2",
            |id, idx, r| TraitTreeXTraitCurrencyEntry {
                id,
                index: r.get_field_i32(idx, 1),
                trait_tree_id: r.get_relationship_id(idx).unwrap_or(0),
                trait_currency_id: r.get_field_i32(idx, 3),
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

impl_from_entries!(TraitCondStore, TraitCondEntry);
impl_from_entries!(TraitCostStore, TraitCostEntry);
impl_from_entries!(TraitCurrencyStore, TraitCurrencyEntry);
impl_from_entries!(TraitCurrencySourceStore, TraitCurrencySourceEntry);
impl_from_entries!(TraitDefinitionStore, TraitDefinitionEntry);
impl_from_entries!(
    TraitDefinitionEffectPointsStore,
    TraitDefinitionEffectPointsEntry
);
impl_from_entries!(TraitEdgeStore, TraitEdgeEntry);
impl_from_entries!(TraitNodeStore, TraitNodeEntry);
impl_from_entries!(TraitNodeEntryStore, TraitNodeEntryEntry);
impl_from_entries!(TraitNodeEntryXTraitCondStore, TraitNodeEntryXTraitCondEntry);
impl_from_entries!(TraitNodeEntryXTraitCostStore, TraitNodeEntryXTraitCostEntry);
impl_from_entries!(TraitNodeGroupStore, TraitNodeGroupEntry);
impl_from_entries!(TraitNodeGroupXTraitCondStore, TraitNodeGroupXTraitCondEntry);
impl_from_entries!(TraitNodeGroupXTraitCostStore, TraitNodeGroupXTraitCostEntry);
impl_from_entries!(TraitNodeGroupXTraitNodeStore, TraitNodeGroupXTraitNodeEntry);
impl_from_entries!(TraitNodeXTraitCondStore, TraitNodeXTraitCondEntry);
impl_from_entries!(TraitNodeXTraitCostStore, TraitNodeXTraitCostEntry);
impl_from_entries!(TraitNodeXTraitNodeEntryStore, TraitNodeXTraitNodeEntryEntry);
impl_from_entries!(TraitTreeStore, TraitTreeEntry);
impl_from_entries!(TraitTreeLoadoutStore, TraitTreeLoadoutEntry);
impl_from_entries!(TraitTreeLoadoutEntryStore, TraitTreeLoadoutEntryEntry);
impl_from_entries!(TraitTreeXTraitCostStore, TraitTreeXTraitCostEntry);
impl_from_entries!(TraitTreeXTraitCurrencyStore, TraitTreeXTraitCurrencyEntry);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_node_store_uses_cpp_tree_parent_relationship() {
        let store = TraitNodeStore::from_entries([TraitNodeEntry {
            id: 10,
            trait_tree_id: 20,
            pos_x: 1,
            pos_y: 2,
            node_type: 3,
            flags: 4,
        }]);

        assert_eq!(store.get(10).unwrap().trait_tree_id, 20);
    }

    #[test]
    fn load_trait_tree_db2_subbatch_when_fixtures_exist() {
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

        load_if_exists!("TraitCond.db2", TraitCondStore);
        load_if_exists!("TraitCost.db2", TraitCostStore);
        load_if_exists!("TraitCurrency.db2", TraitCurrencyStore);
        load_if_exists!("TraitCurrencySource.db2", TraitCurrencySourceStore);
        load_if_exists!("TraitDefinition.db2", TraitDefinitionStore);
        load_if_exists!(
            "TraitDefinitionEffectPoints.db2",
            TraitDefinitionEffectPointsStore
        );
        load_if_exists!("TraitEdge.db2", TraitEdgeStore);
        load_if_exists!("TraitNode.db2", TraitNodeStore);
        load_if_exists!("TraitNodeEntry.db2", TraitNodeEntryStore);
        load_if_exists!(
            "TraitNodeEntryXTraitCond.db2",
            TraitNodeEntryXTraitCondStore
        );
        load_if_exists!(
            "TraitNodeEntryXTraitCost.db2",
            TraitNodeEntryXTraitCostStore
        );
        load_if_exists!("TraitNodeGroup.db2", TraitNodeGroupStore);
        load_if_exists!(
            "TraitNodeGroupXTraitCond.db2",
            TraitNodeGroupXTraitCondStore
        );
        load_if_exists!(
            "TraitNodeGroupXTraitCost.db2",
            TraitNodeGroupXTraitCostStore
        );
        load_if_exists!(
            "TraitNodeGroupXTraitNode.db2",
            TraitNodeGroupXTraitNodeStore
        );
        load_if_exists!("TraitNodeXTraitCond.db2", TraitNodeXTraitCondStore);
        load_if_exists!("TraitNodeXTraitCost.db2", TraitNodeXTraitCostStore);
        load_if_exists!(
            "TraitNodeXTraitNodeEntry.db2",
            TraitNodeXTraitNodeEntryStore
        );
        load_if_exists!("TraitTree.db2", TraitTreeStore);
        load_if_exists!("TraitTreeLoadout.db2", TraitTreeLoadoutStore);
        load_if_exists!("TraitTreeLoadoutEntry.db2", TraitTreeLoadoutEntryStore);
        load_if_exists!("TraitTreeXTraitCost.db2", TraitTreeXTraitCostStore);
        load_if_exists!("TraitTreeXTraitCurrency.db2", TraitTreeXTraitCurrencyStore);
    }
}
