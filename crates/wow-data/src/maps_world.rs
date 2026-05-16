//! Map/world DB2 readers missing from the C++ `DB2Stores.cpp` inventory.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::wdc4::Wdc4Reader;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Db2Position2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Db2Position3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaGroupMemberEntry {
    pub id: u32,
    pub area_id: u16,
    pub area_group_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AreaTriggerDb2Entry {
    pub id: u32,
    pub message: String,
    pub pos: Db2Position3,
    pub continent_id: i16,
    pub phase_use_flags: i8,
    pub phase_id: i16,
    pub phase_group_id: i16,
    pub radius: f32,
    pub box_length: f32,
    pub box_width: f32,
    pub box_height: f32,
    pub box_yaw: f32,
    pub shape_type: i8,
    pub shape_id: i16,
    pub area_trigger_action_set_id: i16,
    pub flags: i8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightEntry {
    pub id: u32,
    pub game_coords: Db2Position3,
    pub game_falloff_start: f32,
    pub game_falloff_end: f32,
    pub continent_id: i16,
    pub light_params_id: [u16; 8],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiquidTypeEntry {
    pub id: u32,
    pub name: String,
    pub texture: [String; 6],
    pub flags: u16,
    pub sound_bank: u8,
    pub sound_id: u32,
    pub spell_id: u32,
    pub max_darken_depth: f32,
    pub fog_darken_intensity: f32,
    pub amb_darken_intensity: f32,
    pub dir_darken_intensity: f32,
    pub light_id: u16,
    pub particle_scale: f32,
    pub particle_movement: u8,
    pub particle_tex_slots: u8,
    pub material_id: u8,
    pub minimap_static_col: i32,
    pub frame_count_texture: [u8; 6],
    pub color: [i32; 2],
    pub float_values: [f32; 18],
    pub int_values: [u32; 4],
    pub coefficient: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapChallengeModeEntry {
    pub id: u32,
    pub name: String,
    pub map_id: u16,
    pub flags: u8,
    pub expansion_level: u32,
    pub required_world_state_id: i32,
    pub criteria_count: [i16; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxiNodesDb2Entry {
    pub id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxiPathEntry {
    pub id: u32,
    pub from_taxi_node: u16,
    pub to_taxi_node: u16,
    pub cost: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaxiPathNodeEntry {
    pub id: u32,
    pub loc: Db2Position3,
    pub path_id: u16,
    pub node_index: i32,
    pub continent_id: u16,
    pub flags: i32,
    pub delay: u32,
    pub arrival_event_id: u32,
    pub departure_event_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportAnimationEntry {
    pub id: u32,
    pub pos: Db2Position3,
    pub sequence_id: u8,
    pub time_index: u32,
    pub transport_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransportRotationEntry {
    pub id: u32,
    pub rot: [f32; 4],
    pub time_index: u32,
    pub game_objects_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiMapEntry {
    pub id: u32,
    pub name: String,
    pub parent_ui_map_id: i32,
    pub flags: i32,
    pub system: i8,
    pub map_type: u8,
    pub bounty_set_id: i32,
    pub bounty_display_location: u32,
    pub visibility_player_condition_id_2: i32,
    pub visibility_player_condition_id: i32,
    pub help_text_position: i8,
    pub bkg_atlas_id: i32,
    pub alternate_ui_map_group: u32,
    pub content_tuning_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiMapAssignmentEntry {
    pub id: u32,
    pub ui_min: Db2Position2,
    pub ui_max: Db2Position2,
    pub region: [Db2Position3; 2],
    pub ui_map_id: i32,
    pub order_index: i32,
    pub map_id: i32,
    pub area_id: i32,
    pub wmo_doodad_placement_id: i32,
    pub wmo_group_id: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiMapLinkEntry {
    pub id: u32,
    pub ui_min: Db2Position2,
    pub ui_max: Db2Position2,
    pub parent_ui_map_id: i32,
    pub order_index: i32,
    pub child_ui_map_id: i32,
    pub override_highlight_file_data_id: i32,
    pub override_highlight_atlas_id: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WmoAreaTableEntry {
    pub id: u32,
    pub area_name: String,
    pub wmo_id: u16,
    pub name_set_id: u8,
    pub wmo_group_id: i32,
    pub sound_provider_pref: u8,
    pub sound_provider_pref_underwater: u8,
    pub ambience_id: u16,
    pub uw_ambience: u16,
    pub zone_music: u16,
    pub uw_zone_music: u32,
    pub intro_sound: u16,
    pub uw_intro_sound: u16,
    pub area_table_id: u16,
    pub flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldEffectEntry {
    pub id: u32,
    pub quest_feedback_effect_id: u32,
    pub when_to_display: u8,
    pub target_type: u8,
    pub target_asset: i32,
    pub player_condition_id: u32,
    pub combat_condition_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldMapOverlayEntry {
    pub id: u32,
    pub ui_map_art_id: u32,
    pub texture_width: u16,
    pub texture_height: u16,
    pub offset_x: i32,
    pub offset_y: i32,
    pub hit_rect_top: i32,
    pub hit_rect_bottom: i32,
    pub hit_rect_left: i32,
    pub hit_rect_right: i32,
    pub player_condition_id: u32,
    pub flags: u32,
    pub area_id: [u32; 4],
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

            pub fn entries(&self) -> impl Iterator<Item = &$entry> {
                self.entries.values()
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

db2_store!(AreaGroupMemberStore, AreaGroupMemberEntry);
db2_store!(AreaTriggerDb2Store, AreaTriggerDb2Entry);
db2_store!(LightStore, LightEntry);
db2_store!(LiquidTypeStore, LiquidTypeEntry);
db2_store!(MapChallengeModeStore, MapChallengeModeEntry);
db2_store!(TaxiNodesDb2Store, TaxiNodesDb2Entry);
db2_store!(TaxiPathStore, TaxiPathEntry);
db2_store!(TaxiPathNodeStore, TaxiPathNodeEntry);
db2_store!(TransportAnimationStore, TransportAnimationEntry);
db2_store!(TransportRotationStore, TransportRotationEntry);
db2_store!(UiMapStore, UiMapEntry);
db2_store!(UiMapAssignmentStore, UiMapAssignmentEntry);
db2_store!(UiMapLinkStore, UiMapLinkEntry);
db2_store!(WmoAreaTableStore, WmoAreaTableEntry);
db2_store!(WorldEffectStore, WorldEffectEntry);
db2_store!(WorldMapOverlayStore, WorldMapOverlayEntry);

impl AreaGroupMemberStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AreaGroupMember.db2", |id, idx, r| {
            AreaGroupMemberEntry {
                id,
                area_id: r.get_field_u16(idx, 0),
                // C++ field `AreaGroupID` is the DB2 parent/relationship id.
                area_group_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl AreaTriggerDb2Store {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "AreaTrigger.db2", |id, idx, r| {
            AreaTriggerDb2Entry {
                id,
                message: r.get_field_string(idx, 0),
                pos: pos3_array(r, idx, 1),
                continent_id: r.get_field_i16(idx, 3),
                phase_use_flags: r.get_field_i8(idx, 4),
                phase_id: r.get_field_i16(idx, 5),
                phase_group_id: r.get_field_i16(idx, 6),
                radius: f32_field(r, idx, 7),
                box_length: f32_field(r, idx, 8),
                box_width: f32_field(r, idx, 9),
                box_height: f32_field(r, idx, 10),
                box_yaw: f32_field(r, idx, 11),
                shape_type: r.get_field_i8(idx, 12),
                shape_id: r.get_field_i16(idx, 13),
                area_trigger_action_set_id: r.get_field_i16(idx, 14),
                flags: r.get_field_i8(idx, 15),
            }
        })
    }
}

impl LightStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "Light.db2", |id, idx, r| LightEntry {
            id,
            game_coords: pos3_array(r, idx, 0),
            game_falloff_start: f32_field(r, idx, 1),
            game_falloff_end: f32_field(r, idx, 2),
            continent_id: r.get_field_i16(idx, 3),
            light_params_id: std::array::from_fn(|i| r.get_array_u16(idx, 4, i)),
        })
    }
}

impl LiquidTypeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "LiquidType.db2", |id, idx, r| {
            let texture = std::array::from_fn(|i| r.get_field_string(idx, 1 + i));
            let frame_count_texture =
                std::array::from_fn(|i| r.get_array_element(idx, 16, i, 8) as u8);
            let color = std::array::from_fn(|i| r.get_array_element(idx, 17, i, 32) as i32);
            let float_values =
                std::array::from_fn(|i| f32::from_bits(r.get_array_element(idx, 18, i, 32)));
            let int_values = std::array::from_fn(|i| r.get_array_element(idx, 19, i, 32));
            let coefficient =
                std::array::from_fn(|i| f32::from_bits(r.get_array_element(idx, 20, i, 32)));
            LiquidTypeEntry {
                id,
                name: r.get_field_string(idx, 0),
                texture,
                flags: r.get_field_u16(idx, 2),
                sound_bank: r.get_field_u8(idx, 3),
                sound_id: r.get_field_u32(idx, 4),
                spell_id: r.get_field_u32(idx, 5),
                max_darken_depth: f32_field(r, idx, 6),
                fog_darken_intensity: f32_field(r, idx, 7),
                amb_darken_intensity: f32_field(r, idx, 8),
                dir_darken_intensity: f32_field(r, idx, 9),
                light_id: r.get_field_u16(idx, 10),
                particle_scale: f32_field(r, idx, 11),
                particle_movement: r.get_field_u8(idx, 12),
                particle_tex_slots: r.get_field_u8(idx, 13),
                material_id: r.get_field_u8(idx, 14),
                minimap_static_col: r.get_field_i32(idx, 15),
                frame_count_texture,
                color,
                float_values,
                int_values,
                coefficient,
            }
        })
    }
}

impl MapChallengeModeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "MapChallengeMode.db2", |id, idx, r| {
            MapChallengeModeEntry {
                id,
                name: r.get_field_string(idx, 0),
                map_id: r.get_field_u16(idx, 2),
                flags: r.get_field_u8(idx, 3),
                expansion_level: r.get_field_u32(idx, 4),
                required_world_state_id: r.get_field_i32(idx, 5),
                criteria_count: [
                    r.get_array_i16(idx, 6, 0),
                    r.get_array_i16(idx, 6, 1),
                    r.get_array_i16(idx, 6, 2),
                ],
            }
        })
    }
}

impl TaxiNodesDb2Store {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TaxiNodes.db2", |id, _idx, _r| {
            TaxiNodesDb2Entry { id }
        })
    }
}

impl TaxiPathStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TaxiPath.db2", |id, idx, r| {
            TaxiPathEntry {
                id,
                from_taxi_node: r.get_field_u16(idx, 1),
                to_taxi_node: r.get_field_u16(idx, 2),
                cost: r.get_field_u32(idx, 3),
            }
        })
    }
}

impl TaxiPathNodeStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TaxiPathNode.db2", |id, idx, r| {
            TaxiPathNodeEntry {
                id,
                loc: pos3_array(r, idx, 0),
                path_id: r.get_field_u16(idx, 2),
                node_index: r.get_field_i32(idx, 3),
                continent_id: r.get_field_u16(idx, 4),
                flags: r.get_field_i32(idx, 5),
                delay: r.get_field_u32(idx, 6),
                arrival_event_id: r.get_field_u32(idx, 7),
                departure_event_id: r.get_field_u32(idx, 8),
            }
        })
    }
}

impl TransportAnimationStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TransportAnimation.db2", |id, idx, r| {
            TransportAnimationEntry {
                id,
                pos: pos3_array(r, idx, 0),
                sequence_id: r.get_field_u8(idx, 1),
                time_index: r.get_field_u32(idx, 2),
                transport_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl TransportRotationStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "TransportRotation.db2", |id, idx, r| {
            TransportRotationEntry {
                id,
                rot: [
                    f32::from_bits(r.get_array_element(idx, 0, 0, 32)),
                    f32::from_bits(r.get_array_element(idx, 0, 1, 32)),
                    f32::from_bits(r.get_array_element(idx, 0, 2, 32)),
                    f32::from_bits(r.get_array_element(idx, 0, 3, 32)),
                ],
                time_index: r.get_field_u32(idx, 1),
                game_objects_id: r.get_relationship_id(idx).unwrap_or(0),
            }
        })
    }
}

impl UiMapStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "UiMap.db2", |id, idx, r| UiMapEntry {
            id,
            name: r.get_field_string(idx, 0),
            parent_ui_map_id: r.get_field_i32(idx, 2),
            flags: r.get_field_i32(idx, 3),
            system: r.get_field_i8(idx, 4),
            map_type: r.get_field_u8(idx, 5),
            bounty_set_id: r.get_field_i32(idx, 6),
            bounty_display_location: r.get_field_u32(idx, 7),
            visibility_player_condition_id_2: r.get_field_i32(idx, 8),
            visibility_player_condition_id: r.get_field_i32(idx, 9),
            help_text_position: r.get_field_i8(idx, 10),
            bkg_atlas_id: r.get_field_i32(idx, 11),
            alternate_ui_map_group: r.get_field_u32(idx, 12),
            content_tuning_id: r.get_field_u32(idx, 13),
        })
    }
}

impl UiMapAssignmentStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "UiMapAssignment.db2", |id, idx, r| {
            UiMapAssignmentEntry {
                id,
                ui_min: pos2_array(r, idx, 0),
                ui_max: pos2_array(r, idx, 1),
                region: [
                    pos3_array_offset(r, idx, 2, 0),
                    pos3_array_offset(r, idx, 2, 3),
                ],
                ui_map_id: r.get_field_i32(idx, 4),
                order_index: r.get_field_i32(idx, 5),
                map_id: r.get_field_i32(idx, 6),
                area_id: r.get_field_i32(idx, 7),
                wmo_doodad_placement_id: r.get_field_i32(idx, 8),
                wmo_group_id: r.get_field_i32(idx, 9),
            }
        })
    }
}

impl UiMapLinkStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "UiMapLink.db2", |id, idx, r| {
            UiMapLinkEntry {
                id,
                ui_min: pos2_array(r, idx, 0),
                ui_max: pos2_array(r, idx, 1),
                parent_ui_map_id: r.get_field_i32(idx, 3),
                order_index: r.get_field_i32(idx, 4),
                child_ui_map_id: r.get_field_i32(idx, 5),
                override_highlight_file_data_id: r.get_field_i32(idx, 6),
                override_highlight_atlas_id: r.get_field_i32(idx, 7),
                flags: r.get_field_i32(idx, 8),
            }
        })
    }
}

impl WmoAreaTableStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "WMOAreaTable.db2", |id, idx, r| {
            WmoAreaTableEntry {
                id,
                area_name: r.get_field_string(idx, 0),
                wmo_id: r.get_field_u16(idx, 2),
                name_set_id: r.get_field_u8(idx, 3),
                wmo_group_id: r.get_field_i32(idx, 4),
                sound_provider_pref: r.get_field_u8(idx, 5),
                sound_provider_pref_underwater: r.get_field_u8(idx, 6),
                ambience_id: r.get_field_u16(idx, 7),
                uw_ambience: r.get_field_u16(idx, 8),
                zone_music: r.get_field_u16(idx, 9),
                uw_zone_music: r.get_field_u32(idx, 10),
                intro_sound: r.get_field_u16(idx, 11),
                uw_intro_sound: r.get_field_u16(idx, 12),
                area_table_id: r.get_field_u16(idx, 13),
                flags: r.get_field_u8(idx, 14),
            }
        })
    }
}

impl WorldEffectStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "WorldEffect.db2", |id, idx, r| {
            WorldEffectEntry {
                id,
                quest_feedback_effect_id: r.get_field_u32(idx, 0),
                when_to_display: r.get_field_u8(idx, 1),
                target_type: r.get_field_u8(idx, 2),
                target_asset: r.get_field_i32(idx, 3),
                player_condition_id: r.get_field_u32(idx, 4),
                combat_condition_id: r.get_field_u16(idx, 5),
            }
        })
    }
}

impl WorldMapOverlayStore {
    pub fn load(data_dir: &str, locale: &str) -> Result<Self> {
        load_store(data_dir, locale, "WorldMapOverlay.db2", |id, idx, r| {
            WorldMapOverlayEntry {
                id,
                ui_map_art_id: r.get_field_u32(idx, 1),
                texture_width: r.get_field_u16(idx, 2),
                texture_height: r.get_field_u16(idx, 3),
                offset_x: r.get_field_i32(idx, 4),
                offset_y: r.get_field_i32(idx, 5),
                hit_rect_top: r.get_field_i32(idx, 6),
                hit_rect_bottom: r.get_field_i32(idx, 7),
                hit_rect_left: r.get_field_i32(idx, 8),
                hit_rect_right: r.get_field_i32(idx, 9),
                player_condition_id: r.get_field_u32(idx, 10),
                flags: r.get_field_u32(idx, 11),
                area_id: [
                    r.get_array_element(idx, 12, 0, 32),
                    r.get_array_element(idx, 12, 1, 32),
                    r.get_array_element(idx, 12, 2, 32),
                    r.get_array_element(idx, 12, 3, 32),
                ],
            }
        })
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

impl_from_entries!(AreaGroupMemberStore, AreaGroupMemberEntry);
impl_from_entries!(AreaTriggerDb2Store, AreaTriggerDb2Entry);
impl_from_entries!(LightStore, LightEntry);
impl_from_entries!(LiquidTypeStore, LiquidTypeEntry);
impl_from_entries!(MapChallengeModeStore, MapChallengeModeEntry);
impl_from_entries!(TaxiNodesDb2Store, TaxiNodesDb2Entry);
impl_from_entries!(TaxiPathStore, TaxiPathEntry);
impl_from_entries!(TaxiPathNodeStore, TaxiPathNodeEntry);
impl_from_entries!(TransportAnimationStore, TransportAnimationEntry);
impl_from_entries!(TransportRotationStore, TransportRotationEntry);
impl_from_entries!(UiMapStore, UiMapEntry);
impl_from_entries!(UiMapAssignmentStore, UiMapAssignmentEntry);
impl_from_entries!(UiMapLinkStore, UiMapLinkEntry);
impl_from_entries!(WmoAreaTableStore, WmoAreaTableEntry);
impl_from_entries!(WorldEffectStore, WorldEffectEntry);
impl_from_entries!(WorldMapOverlayStore, WorldMapOverlayEntry);

fn f32_field(reader: &Wdc4Reader, idx: usize, field: usize) -> f32 {
    f32::from_bits(reader.get_field_u32(idx, field))
}

fn pos2_array(reader: &Wdc4Reader, idx: usize, field: usize) -> Db2Position2 {
    Db2Position2 {
        x: f32::from_bits(reader.get_array_element(idx, field, 0, 32)),
        y: f32::from_bits(reader.get_array_element(idx, field, 1, 32)),
    }
}

fn pos3_array(reader: &Wdc4Reader, idx: usize, field: usize) -> Db2Position3 {
    pos3_array_offset(reader, idx, field, 0)
}

fn pos3_array_offset(
    reader: &Wdc4Reader,
    idx: usize,
    field: usize,
    first_array_index: usize,
) -> Db2Position3 {
    Db2Position3 {
        x: f32::from_bits(reader.get_array_element(idx, field, first_array_index, 32)),
        y: f32::from_bits(reader.get_array_element(idx, field, first_array_index + 1, 32)),
        z: f32::from_bits(reader.get_array_element(idx, field, first_array_index + 2, 32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxi_path_store_indexes_by_id_like_cpp_lookup_entry() {
        let store = TaxiPathStore::from_entries([TaxiPathEntry {
            id: 7,
            from_taxi_node: 1,
            to_taxi_node: 2,
            cost: 345,
        }]);

        assert_eq!(store.get(7).unwrap().cost, 345);
        assert!(store.get(8).is_none());
    }

    #[test]
    fn ui_map_assignment_keeps_cpp_order_index_and_region_fields() {
        let store = UiMapAssignmentStore::from_entries([UiMapAssignmentEntry {
            id: 11,
            ui_min: Db2Position2 { x: 0.1, y: 0.2 },
            ui_max: Db2Position2 { x: 0.9, y: 0.8 },
            region: [
                Db2Position3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                Db2Position3 {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                },
            ],
            ui_map_id: 100,
            order_index: 3,
            map_id: 571,
            area_id: 67,
            wmo_doodad_placement_id: -1,
            wmo_group_id: -1,
        }]);

        let entry = store.get(11).unwrap();
        assert_eq!(entry.order_index, 3);
        assert_eq!(entry.region[1].z, 6.0);
    }

    #[test]
    fn liquid_type_store_preserves_cpp_arrays() {
        let store = LiquidTypeStore::from_entries([LiquidTypeEntry {
            id: 3,
            name: "water".to_string(),
            texture: std::array::from_fn(|i| format!("tex{i}")),
            flags: 1,
            sound_bank: 2,
            sound_id: 3,
            spell_id: 4,
            max_darken_depth: 5.0,
            fog_darken_intensity: 6.0,
            amb_darken_intensity: 7.0,
            dir_darken_intensity: 8.0,
            light_id: 9,
            particle_scale: 10.0,
            particle_movement: 11,
            particle_tex_slots: 12,
            material_id: 13,
            minimap_static_col: 14,
            frame_count_texture: [1, 2, 3, 4, 5, 6],
            color: [7, 8],
            float_values: [0.5; 18],
            int_values: [9, 10, 11, 12],
            coefficient: [1.0, 2.0, 3.0, 4.0],
        }]);

        let entry = store.get(3).unwrap();
        assert_eq!(entry.texture[5], "tex5");
        assert_eq!(entry.frame_count_texture[4], 5);
        assert_eq!(entry.coefficient[3], 4.0);
    }

    #[test]
    fn load_maps_world_db2_batch_when_fixtures_exist() {
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

        load_if_exists!("AreaGroupMember.db2", AreaGroupMemberStore);
        load_if_exists!("AreaTrigger.db2", AreaTriggerDb2Store);
        load_if_exists!("Light.db2", LightStore);
        load_if_exists!("LiquidType.db2", LiquidTypeStore);
        load_if_exists!("MapChallengeMode.db2", MapChallengeModeStore);
        load_if_exists!("TaxiNodes.db2", TaxiNodesDb2Store);
        load_if_exists!("TaxiPath.db2", TaxiPathStore);
        load_if_exists!("TaxiPathNode.db2", TaxiPathNodeStore);
        load_if_exists!("TransportAnimation.db2", TransportAnimationStore);
        load_if_exists!("TransportRotation.db2", TransportRotationStore);
        load_if_exists!("UiMap.db2", UiMapStore);
        load_if_exists!("UiMapAssignment.db2", UiMapAssignmentStore);
        load_if_exists!("UiMapLink.db2", UiMapLinkStore);
        load_if_exists!("WMOAreaTable.db2", WmoAreaTableStore);
        load_if_exists!("WorldEffect.db2", WorldEffectStore);
        load_if_exists!("WorldMapOverlay.db2", WorldMapOverlayStore);
    }
}
