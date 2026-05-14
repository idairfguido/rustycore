// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ConditionMgr::LoadConditions` downstream attachment pass.

use std::collections::HashSet;

use wow_constants::ConditionSourceType;

use crate::{
    ConditionEntriesByTypeStore, GossipConditionAttachmentReport, GossipStore,
    GraveyardConditionAttachmentReport, GraveyardStore, PhaseConditionAttachmentReport,
    PhaseInfoStore,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConditionAttachmentReportLikeCpp {
    pub gossip_menus: GossipConditionAttachmentReport,
    pub gossip_menu_items: GossipConditionAttachmentReport,
    pub spell_click_aura_spell_ids: HashSet<u32>,
    pub deferred_spell_implicit_target_condition_count: usize,
    pub phases: PhaseConditionAttachmentReport,
    pub graveyards: GraveyardConditionAttachmentReport,
}

/// C++ `ConditionMgr::LoadConditions` post-store source-type attachment pass.
///
/// This applies the downstream builders already represented in `wow-data`. Spell implicit target
/// conditions are validated and normalized during the external validation pass, then counted here
/// until Rust exposes the runtime `SpellEffectInfo::ImplicitTargetConditions` attachment surface.
pub fn attach_loaded_conditions_like_cpp(
    condition_store: &ConditionEntriesByTypeStore,
    mut gossip_store: Option<&mut GossipStore>,
    mut phase_info_store: Option<&mut PhaseInfoStore>,
    mut graveyard_store: Option<&mut GraveyardStore>,
) -> ConditionAttachmentReportLikeCpp {
    let gossip_menus = gossip_store
        .as_deref_mut()
        .map(|store| store.attach_gossip_menu_conditions_like_cpp(condition_store))
        .unwrap_or_default();
    let gossip_menu_items = gossip_store
        .as_deref_mut()
        .map(|store| store.attach_gossip_menu_item_conditions_like_cpp(condition_store))
        .unwrap_or_default();

    let spell_click_aura_spell_ids =
        condition_store.spells_used_in_spell_click_conditions_like_cpp();
    let deferred_spell_implicit_target_condition_count = condition_store
        .entries_for_source_type_like_cpp(ConditionSourceType::SpellImplicitTarget)
        .into_iter()
        .flat_map(|entries| entries.values())
        .map(|conditions| conditions.len())
        .sum();

    let phases = phase_info_store
        .as_deref_mut()
        .map(|store| store.attach_phase_conditions_like_cpp(condition_store))
        .unwrap_or_default();
    let graveyards = graveyard_store
        .as_deref_mut()
        .map(|store| store.attach_graveyard_conditions_like_cpp(condition_store))
        .unwrap_or_default();

    ConditionAttachmentReportLikeCpp {
        gossip_menus,
        gossip_menu_items,
        spell_click_aura_spell_ids,
        deferred_spell_implicit_target_condition_count,
        phases,
        graveyards,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AreaTableEntry, Condition, GraveyardZoneRow, PhaseEntry, PhaseInfoStore, PhaseStore,
    };
    use wow_constants::ConditionType;

    fn condition(
        source_type: ConditionSourceType,
        source_group: u32,
        source_entry: i32,
        condition_type: ConditionType,
        value1: u32,
    ) -> Condition {
        Condition {
            source_type,
            source_group,
            source_entry,
            condition_type,
            condition_value1: value1,
            ..Condition::default()
        }
    }

    #[test]
    fn loaded_condition_attachment_pass_matches_cpp_ordered_builders() {
        let conditions = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            condition(
                ConditionSourceType::GossipMenu,
                7,
                10,
                ConditionType::Aura,
                100,
            ),
            condition(
                ConditionSourceType::GossipMenuOption,
                7,
                2,
                ConditionType::Aura,
                101,
            ),
            condition(
                ConditionSourceType::SpellClickEvent,
                1,
                2,
                ConditionType::Aura,
                200,
            ),
            condition(
                ConditionSourceType::SpellImplicitTarget,
                1,
                123,
                ConditionType::Aura,
                201,
            ),
            condition(
                ConditionSourceType::Phase,
                55,
                100,
                ConditionType::Aura,
                202,
            ),
            condition(
                ConditionSourceType::Graveyard,
                100,
                99,
                ConditionType::Team,
                469,
            ),
        ]);

        let mut gossip = GossipStore::default();
        gossip.add_menu_like_cpp(7, 10);
        gossip.add_menu_item_like_cpp(7, 2);

        let area_store = crate::AreaTableStore::from_entries([AreaTableEntry {
            id: 100,
            parent_area_id: 0,
        }]);
        let phase_store = PhaseStore::from_entries([PhaseEntry { id: 55, flags: 0 }]);
        let mut phases = PhaseInfoStore::from_phase_store_like_cpp(&phase_store);
        phases.load_area_phases_from_rows_like_cpp(&area_store, &phase_store, [(100, 55)]);

        let mut graveyards = GraveyardStore::default();
        graveyards.load_graveyard_zones_from_rows_like_cpp(
            [GraveyardZoneRow {
                safe_loc_id: 99,
                ghost_zone_id: 100,
            }],
            |_| true,
            |_| true,
        );

        let report = attach_loaded_conditions_like_cpp(
            &conditions,
            Some(&mut gossip),
            Some(&mut phases),
            Some(&mut graveyards),
        );

        assert_eq!(report.gossip_menus.attached_condition_count, 1);
        assert_eq!(report.gossip_menu_items.attached_condition_count, 1);
        assert_eq!(report.spell_click_aura_spell_ids, HashSet::from([200]));
        assert_eq!(report.deferred_spell_implicit_target_condition_count, 1);
        assert_eq!(report.phases.attached_condition_count, 1);
        assert_eq!(report.graveyards.attached_condition_count, 1);
    }
}
