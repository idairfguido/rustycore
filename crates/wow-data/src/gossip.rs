// Copyright (c) 2026 alseif0x
// RustyCore - WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 - https://www.gnu.org/licenses/gpl-3.0.html

//! C++ `ObjectMgr` gossip menu condition attachment primitives.

use std::collections::HashMap;

use anyhow::Result;
use wow_constants::ConditionSourceType;
use wow_database::{WorldDatabase, WorldStatements};

use crate::{ConditionEntriesByTypeStore, ConditionId, ConditionsReference};

#[derive(Debug, Clone, Default)]
pub struct GossipMenu {
    pub menu_id: u32,
    pub text_id: u32,
    pub conditions: ConditionsReference,
}

#[derive(Debug, Clone, Default)]
pub struct GossipMenuItem {
    pub menu_id: u32,
    pub order_index: u32,
    pub conditions: ConditionsReference,
}

#[derive(Debug, Clone, Default)]
pub struct GossipStore {
    menus_by_id: HashMap<u32, Vec<GossipMenu>>,
    items_by_menu_id: HashMap<u32, Vec<GossipMenuItem>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GossipLoadReport {
    pub menu_rows: usize,
    pub menu_item_rows: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GossipConditionAttachmentReport {
    pub attached_condition_count: usize,
    pub missing_menus: Vec<ConditionId>,
    pub missing_menu_items: Vec<ConditionId>,
}

impl GossipStore {
    /// C++ `ObjectMgr::LoadGossipMenu` + condition-key subset of
    /// `ObjectMgr::LoadGossipMenuItems`.
    pub async fn load_like_cpp(db: &WorldDatabase) -> Result<(Self, GossipLoadReport)> {
        let mut store = Self::default();
        let mut report = GossipLoadReport::default();

        let stmt = db.prepare(WorldStatements::SEL_GOSSIP_MENUS);
        let mut result = db.query(&stmt).await?;
        if !result.is_empty() {
            loop {
                store.add_menu_like_cpp(result.read(0), result.read(1));
                report.menu_rows += 1;
                if !result.next_row() {
                    break;
                }
            }
        }

        let stmt = db.prepare(WorldStatements::SEL_GOSSIP_MENU_OPTION_KEYS);
        let mut result = db.query(&stmt).await?;
        if !result.is_empty() {
            loop {
                store.add_menu_item_like_cpp(result.read(0), result.read(1));
                report.menu_item_rows += 1;
                if !result.next_row() {
                    break;
                }
            }
        }

        Ok((store, report))
    }

    pub fn menu_row_count(&self) -> usize {
        self.menus_by_id.values().map(Vec::len).sum()
    }

    pub fn menu_item_row_count(&self) -> usize {
        self.items_by_menu_id.values().map(Vec::len).sum()
    }

    pub fn add_menu_like_cpp(&mut self, menu_id: u32, text_id: u32) {
        self.menus_by_id
            .entry(menu_id)
            .or_default()
            .push(GossipMenu {
                menu_id,
                text_id,
                conditions: ConditionsReference::default(),
            });
    }

    pub fn add_menu_item_like_cpp(&mut self, menu_id: u32, order_index: u32) {
        self.items_by_menu_id
            .entry(menu_id)
            .or_default()
            .push(GossipMenuItem {
                menu_id,
                order_index,
                conditions: ConditionsReference::default(),
            });
    }

    pub fn menus_for_id(&self, menu_id: u32) -> Option<&[GossipMenu]> {
        self.menus_by_id.get(&menu_id).map(Vec::as_slice)
    }

    pub fn menu_items_for_id(&self, menu_id: u32) -> Option<&[GossipMenuItem]> {
        self.items_by_menu_id.get(&menu_id).map(Vec::as_slice)
    }

    /// C++ `ConditionMgr::addToGossipMenus`.
    pub fn attach_gossip_menu_conditions_like_cpp(
        &mut self,
        conditions: &ConditionEntriesByTypeStore,
    ) -> GossipConditionAttachmentReport {
        let mut report = GossipConditionAttachmentReport::default();
        let Some(menu_conditions) =
            conditions.entries_for_source_type_like_cpp(ConditionSourceType::GossipMenu)
        else {
            return report;
        };

        for (id, condition_bucket) in menu_conditions {
            let Some(menus) = self.menus_by_id.get_mut(&id.source_group) else {
                report.missing_menus.push(*id);
                continue;
            };

            for menu in menus {
                if id.source_entry == 0 || menu.text_id == id.source_entry as u32 {
                    menu.conditions = ConditionsReference::new(condition_bucket);
                    report.attached_condition_count += condition_bucket.len();
                }
            }
        }

        report
    }

    /// C++ `ConditionMgr::addToGossipMenuItems`.
    pub fn attach_gossip_menu_item_conditions_like_cpp(
        &mut self,
        conditions: &ConditionEntriesByTypeStore,
    ) -> GossipConditionAttachmentReport {
        let mut report = GossipConditionAttachmentReport::default();
        let Some(item_conditions) =
            conditions.entries_for_source_type_like_cpp(ConditionSourceType::GossipMenuOption)
        else {
            return report;
        };

        for (id, condition_bucket) in item_conditions {
            let Some(items) = self.items_by_menu_id.get_mut(&id.source_group) else {
                report.missing_menu_items.push(*id);
                continue;
            };

            let Some(item) = items
                .iter_mut()
                .find(|item| item.order_index == id.source_entry as u32)
            else {
                report.missing_menu_items.push(*id);
                continue;
            };

            item.conditions = ConditionsReference::new(condition_bucket);
            report.attached_condition_count += condition_bucket.len();
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Condition;
    use wow_constants::ConditionType;

    fn condition(source_type: ConditionSourceType, menu_id: u32, source_entry: i32) -> Condition {
        Condition {
            source_type,
            source_group: menu_id,
            source_entry,
            condition_type: ConditionType::Aura,
            condition_value1: 100,
            ..Condition::default()
        }
    }

    #[test]
    fn gossip_menu_conditions_attach_to_matching_text_or_all_texts_like_cpp() {
        let mut gossip = GossipStore::default();
        gossip.add_menu_like_cpp(7, 10);
        gossip.add_menu_like_cpp(7, 20);
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([
            condition(ConditionSourceType::GossipMenu, 7, 10),
            condition(ConditionSourceType::GossipMenu, 7, 0),
        ]);

        let report = gossip.attach_gossip_menu_conditions_like_cpp(&store);

        assert_eq!(report.attached_condition_count, 3);
        assert!(report.missing_menus.is_empty());
        let menus = gossip.menus_for_id(7).unwrap();
        assert_eq!(menus[0].conditions.upgrade().unwrap().len(), 1);
        assert_eq!(menus[1].conditions.upgrade().unwrap().len(), 1);
    }

    #[test]
    fn gossip_menu_conditions_do_not_report_text_mismatch_when_menu_exists_like_cpp() {
        let mut gossip = GossipStore::default();
        gossip.add_menu_like_cpp(7, 10);
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition(
            ConditionSourceType::GossipMenu,
            7,
            99,
        )]);

        let report = gossip.attach_gossip_menu_conditions_like_cpp(&store);

        assert_eq!(report.attached_condition_count, 0);
        assert!(report.missing_menus.is_empty());
        assert!(gossip.menus_for_id(7).unwrap()[0].conditions.is_expired());
    }

    #[test]
    fn gossip_menu_item_conditions_attach_by_menu_and_order_index_like_cpp() {
        let mut gossip = GossipStore::default();
        gossip.add_menu_item_like_cpp(7, 2);
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition(
            ConditionSourceType::GossipMenuOption,
            7,
            2,
        )]);

        let report = gossip.attach_gossip_menu_item_conditions_like_cpp(&store);

        assert_eq!(report.attached_condition_count, 1);
        assert!(report.missing_menu_items.is_empty());
        assert_eq!(
            gossip.menu_items_for_id(7).unwrap()[0]
                .conditions
                .upgrade()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn gossip_menu_item_conditions_report_missing_item_like_cpp() {
        let mut gossip = GossipStore::default();
        gossip.add_menu_item_like_cpp(7, 2);
        let missing = ConditionId::new(7, 3, 0);
        let store = ConditionEntriesByTypeStore::from_conditions_like_cpp([condition(
            ConditionSourceType::GossipMenuOption,
            7,
            3,
        )]);

        let report = gossip.attach_gossip_menu_item_conditions_like_cpp(&store);

        assert_eq!(report.attached_condition_count, 0);
        assert_eq!(report.missing_menu_items, vec![missing]);
    }
}
