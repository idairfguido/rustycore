// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Loot packet handlers — CMSG_LOOT_UNIT, CMSG_LOOT_ITEM, CMSG_LOOT_RELEASE.
//!
//! Reference: C# Game/Handlers/LootHandler.cs

use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use wow_constants::{ClientOpcodes, InventoryResult, ItemContext};
use wow_core::ObjectGuid;
use wow_database::{CharStatements, SqlTransaction};
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_entities::INVENTORY_SLOT_BAG_0;
use wow_packet::packets::item::{
    ItemExpirePurchaseRefund, ItemInstance, ItemModList, ItemPushResult,
    ItemPushResultDisplayType,
};
use wow_packet::packets::loot::{
    CreatureLoot, LootItemData, LootItemPkt, LootMoney, LootMoneyNotify, LootRelease, LootRemoved,
    LootResponse, LootUnit, SLootRelease,
};
use wow_packet::packets::update::{ItemCreateData, UpdateObject};
use wow_packet::ClientPacket;

use crate::session::{InventoryItem, WorldSession};

// ── Handler registrations ─────────────────────────────────────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LootUnit,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_loot_unit",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LootItem,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_loot_item",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LootMoney,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_loot_money",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LootRelease,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_loot_release",
    }
}

// ── Handler implementations ───────────────────────────────────────

impl WorldSession {
    /// CMSG_LOOT_UNIT — player right-clicks a dead creature to loot it.
    pub async fn handle_loot_unit(&mut self, mut pkt: wow_packet::WorldPacket) {
        let req = match LootUnit::read(&mut pkt) {
            Ok(r) => r,
            Err(e) => { warn!("Bad LootUnit: {e}"); return; }
        };

        let player_guid = match self.player_guid {
            Some(g) => g,
            None => return,
        };

        debug!(account = self.account_id, target = ?req.unit, "CMSG_LOOT_UNIT");

        // Check creature exists and is dead.
        let creature = match self.creatures.get(&req.unit) {
            Some(c) => c,
            None => {
                warn!("LootUnit: creature {:?} not found", req.unit);
                return;
            }
        };

        if creature.is_alive {
            // Can't loot a living creature.
            let response = LootResponse {
                owner: player_guid,
                loot_obj: req.unit,
                failure_reason: 2, // LootError::AlreadyPickedUp or similar
                acquire_reason: 0,
                loot_method: 0,
                threshold: 2,
                coins: 0,
                items: vec![],
                acquired: false,
                ae_looting: false,
            };
            self.send_packet(&response);
            return;
        }

        // Generate or retrieve existing loot.
        if !self.loot_table.contains_key(&req.unit) {
            let loot = generate_creature_loot(req.unit, creature.level, creature.entry);
            self.loot_table.insert(req.unit, loot);
        }

        let loot = self.loot_table.get(&req.unit).unwrap();
        let coins = loot.coins;
        let items: Vec<LootItemData> = loot.items.iter()
            .filter(|e| !e.taken)
            .map(|e| LootItemData {
                loot_list_id: e.loot_list_id,
                ui_type: 0,
                quantity: e.quantity,
                item_id: e.item_id as i32,
                item_context: 0,
                bonus_list_ids: vec![],
                can_loot: true,
            })
            .collect();

        let response = LootResponse {
            owner: player_guid,
            loot_obj: req.unit,
            failure_reason: 0, // No error
            acquire_reason: 0,
            loot_method: 0,   // FreeForAll
            threshold: 2,
            coins,
            items,
            acquired: true,
            ae_looting: false,
        };
        self.set_active_loot_guid(req.unit);
        self.send_packet(&response);
    }

    /// CMSG_LOOT_ITEM — player clicks to take a specific item from the loot.
    pub async fn handle_loot_item(&mut self, mut pkt: wow_packet::WorldPacket) {
        let req = match LootItemPkt::read(&mut pkt) {
            Ok(r) => r,
            Err(e) => { warn!("Bad LootItem: {e}"); return; }
        };

        let player_guid = match self.player_guid { Some(g) => g, None => return };

        let mut taken_items: Vec<(ObjectGuid, u8, u32, u32)> = Vec::new();
        let mut item_release: Vec<ObjectGuid> = Vec::new();

        for loot_req in &req.requests {
            let Some(entry) = self
                .loot_table
                .get(&loot_req.object)
                .and_then(|loot| {
                    loot.items
                        .iter()
                        .find(|entry| entry.loot_list_id == loot_req.loot_list_id && !entry.taken)
                        .cloned()
                })
            else {
                self.send_equip_error(InventoryResult::LootGone, None, None, 0, 0);
                continue;
            };

            if !self
                .store_direct_loot_item_like_cpp(entry.item_id, entry.quantity)
                .await
            {
                continue;
            }

            if loot_req.object.is_item() {
                self.delete_stored_item_loot_item_like_cpp(
                    loot_req.object,
                    entry.item_id,
                    entry.quantity,
                    entry.loot_list_id,
                )
                .await;
            }

            if let Some(loot) = self.loot_table.get_mut(&loot_req.object) {
                if let Some(entry) = loot
                    .items
                    .iter_mut()
                    .find(|entry| entry.loot_list_id == loot_req.loot_list_id && !entry.taken)
                {
                    entry.taken = true;
                    taken_items.push((
                        loot_req.object,
                        entry.loot_list_id,
                        entry.item_id,
                        entry.quantity,
                    ));
                }

                if loot_req.object.is_item() && loot_is_looted_like_cpp(loot) {
                    item_release.push(loot_req.object);
                }
            }
        }

        for (loot_obj, list_id, item_id, quantity) in taken_items {
            let removed = LootRemoved {
                owner: player_guid,
                loot_obj,
                loot_list_id: list_id,
            };
            self.send_packet(&removed);
            debug!(
                account = self.account_id,
                item = item_id,
                quantity,
                "Looted item"
            );
        }

        item_release.sort_by_key(|guid| guid.counter());
        item_release.dedup();
        for loot_guid in item_release {
            self.loot_table.remove(&loot_guid);
            self.send_packet(&SLootRelease {
                unit: loot_guid,
                loot_obj: loot_guid,
            });
            self.destroy_fully_looted_direct_item(loot_guid).await;
        }
    }

    /// CMSG_LOOT_MONEY — player takes money from the current loot view.
    pub async fn handle_loot_money(&mut self, mut pkt: wow_packet::WorldPacket) {
        let req = match LootMoney::read(&mut pkt) {
            Ok(r) => r,
            Err(e) => {
                warn!("Bad LootMoney: {e}");
                return;
            }
        };

        let player_guid = match self.player_guid {
            Some(guid) => guid,
            None => return,
        };

        debug!(
            account = self.account_id,
            is_soft_interact = req.is_soft_interact,
            "CMSG_LOOT_MONEY"
        );

        let mut money_by_loot: Vec<(ObjectGuid, u32)> = Vec::new();
        let mut item_release: Vec<ObjectGuid> = Vec::new();

        for (loot_guid, loot) in self.loot_table.iter_mut() {
            if loot.coins == 0 {
                continue;
            }

            let money = loot.coins;
            loot.coins = 0;
            money_by_loot.push((*loot_guid, money));

            if loot_guid.is_item() && loot_is_looted_like_cpp(loot) {
                item_release.push(*loot_guid);
            }
        }

        if money_by_loot.is_empty() {
            return;
        }

        let total_money = money_by_loot
            .iter()
            .fold(0u64, |total, (_, money)| total.saturating_add(u64::from(*money)));
        self.player_gold = self.player_gold.saturating_add(total_money);
        self.save_player_gold().await;

        for (_, money) in &money_by_loot {
            self.send_packet(&LootMoneyNotify {
                money: u64::from(*money),
                money_mod: 0,
                sole_looter: true,
            });
        }

        for (loot_guid, _) in &money_by_loot {
            if loot_guid.is_item() {
                self.delete_stored_item_money_like_cpp(*loot_guid).await;
            }
        }

        for loot_guid in item_release {
            self.loot_table.remove(&loot_guid);
            self.send_packet(&SLootRelease {
                unit: loot_guid,
                loot_obj: loot_guid,
            });
            self.destroy_fully_looted_direct_item(loot_guid).await;
        }

        let _ = player_guid;
    }

    /// CMSG_LOOT_RELEASE — player closes the loot window.
    ///
    /// C# ref: `LootHandler.DoLootRelease` (creature branch):
    ///   if loot.IsLooted() && creature.IsFullyLooted() → RemoveDynamicFlag(Lootable)
    ///   → creature.AllLootRemovedFromCorpse() → sets `m_corpseRemoveTime = now + decay`
    pub async fn handle_loot_release(&mut self, mut pkt: wow_packet::WorldPacket) {
        let req = match LootRelease::read(&mut pkt) {
            Ok(r) => r,
            Err(e) => { warn!("Bad LootRelease: {e}"); return; }
        };

        debug!(account = self.account_id, unit = ?req.unit, "CMSG_LOOT_RELEASE");

        // Check if loot is fully taken (all items picked up).
        // Coins are auto-consumed when the loot window opens (sent in LootResponse),
        // so we only check items here.
        // C# ref: `loot.IsLooted()` → no more non-taken items.
        let fully_looted = self.loot_table
            .get(&req.unit)
            .map(loot_is_looted_like_cpp)
            .unwrap_or(true); // If no entry at all, treat as fully looted.

        let player_guid = match self.player_guid { Some(g) => g, None => return };

        // Acknowledge the release to the client.
        let release = SLootRelease {
            unit: req.unit,
            loot_obj: req.unit,
        };
        self.send_packet(&release);

        if req.unit.is_item() && !fully_looted {
            self.clear_active_loot_guid_if(req.unit);
            return;
        }

        // Remove loot entry from memory once the represented loot is consumed.
        self.loot_table.remove(&req.unit);
        self.clear_active_loot_guid_if(req.unit);

        if req.unit.is_item() && fully_looted {
            self.destroy_fully_looted_direct_item(req.unit).await;
            return;
        }

        // Start corpse despawn timer if fully looted.
        // C# uses `RateCorpseDecayLooted` config × `m_corpseDelay` (default 60s).
        // We use a simple 30s fixed decay.
        if fully_looted {
            if let Some(creature) = self.creatures.get_mut(&req.unit) {
                if !creature.is_alive && creature.corpse_despawn_at.is_none() {
                    const CORPSE_DECAY_SECS: u64 = 30;
                    creature.corpse_despawn_at =
                        Some(Instant::now() + Duration::from_secs(CORPSE_DECAY_SECS));
                    info!(
                        "Creature {:?} (entry {}) fully looted — despawning in {}s",
                        req.unit, creature.entry, CORPSE_DECAY_SECS
                    );
                }
            }
        }

        let _ = player_guid;
    }

    async fn delete_stored_item_money_like_cpp(&self, item_guid: ObjectGuid) {
        let Some(char_db) = self.char_db().map(Arc::clone) else {
            return;
        };

        let mut stmt = char_db.prepare(CharStatements::DEL_ITEMCONTAINER_MONEY);
        stmt.set_u64(0, item_guid.counter() as u64);
        if let Err(e) = char_db.execute(&stmt).await {
            warn!(
                item_guid = item_guid.counter(),
                error = %e,
                "failed to delete stored item loot money"
            );
        }
    }

    async fn delete_stored_item_loot_item_like_cpp(
        &self,
        item_guid: ObjectGuid,
        item_id: u32,
        count: u32,
        loot_list_id: u8,
    ) {
        let Some(char_db) = self.char_db().map(Arc::clone) else {
            return;
        };

        let mut stmt = char_db.prepare(CharStatements::DEL_ITEMCONTAINER_ITEM);
        stmt.set_u64(0, item_guid.counter() as u64);
        stmt.set_u32(1, item_id);
        stmt.set_u32(2, count);
        stmt.set_u32(3, u32::from(loot_list_id));
        if let Err(e) = char_db.execute(&stmt).await {
            warn!(
                item_guid = item_guid.counter(),
                item_id,
                count,
                loot_list_id,
                error = %e,
                "failed to delete stored item loot row"
            );
        }
    }

    async fn store_direct_loot_item_like_cpp(&mut self, item_id: u32, count: u32) -> bool {
        let Some(player_guid) = self.player_guid else {
            return false;
        };
        let Some(char_db) = self.char_db().map(Arc::clone) else {
            return false;
        };
        let Some((store_result, store_dest, _)) =
            self.plan_store_new_direct_inventory_item(item_id, count)
        else {
            self.send_equip_error(InventoryResult::ItemNotFound, None, None, 0, 0);
            return false;
        };
        if store_result != InventoryResult::Ok {
            self.send_equip_error(store_result, None, None, 0, 0);
            return false;
        }

        let mut planned_existing_counts = Vec::<(u8, ObjectGuid, u64, u32, u32)>::new();
        let mut planned_new_stacks = Vec::<PlannedLootNewStack>::new();

        for dest in store_dest {
            let bag = (dest.pos >> 8) as u8;
            let slot = (dest.pos & 0x00FF) as u8;
            if bag != u8::from(INVENTORY_SLOT_BAG_0) {
                self.send_equip_error(InventoryResult::InvFull, None, None, 0, 0);
                return false;
            }

            let max_stack = self
                .item_storage_template(item_id)
                .map(|template| template.max_stack_size)
                .unwrap_or(1)
                .max(1);

            if let Some(existing) = self.inventory_items.get(&slot) {
                let Some(existing_object) = self.inventory_item_objects.get(&existing.guid) else {
                    self.send_equip_error(InventoryResult::ItemNotFound, None, None, 0, 0);
                    return false;
                };
                let base_count = planned_existing_counts
                    .iter()
                    .find(|(planned_slot, ..)| *planned_slot == slot)
                    .map(|(_, _, _, new_count, _)| *new_count)
                    .unwrap_or_else(|| existing_object.count());
                let new_count = base_count.saturating_add(dest.count);
                if existing.entry_id != item_id || new_count > max_stack {
                    self.send_equip_error(InventoryResult::InvFull, None, None, 0, 0);
                    return false;
                }
                if let Some(existing_plan) = planned_existing_counts
                    .iter_mut()
                    .find(|(planned_slot, ..)| *planned_slot == slot)
                {
                    existing_plan.3 = new_count;
                    existing_plan.4 = existing_plan.4.saturating_add(dest.count);
                } else {
                    planned_existing_counts.push((
                        slot,
                        existing.guid,
                        existing.db_guid,
                        new_count,
                        dest.count,
                    ));
                }
                continue;
            }

            if let Some(stack) = planned_new_stacks
                .iter_mut()
                .find(|stack| stack.slot == slot)
            {
                if stack.entry_id == item_id && stack.count.saturating_add(dest.count) <= max_stack {
                    stack.count = stack.count.saturating_add(dest.count);
                    continue;
                }
                self.send_equip_error(InventoryResult::InvFull, None, None, 0, 0);
                return false;
            }

            planned_new_stacks.push(PlannedLootNewStack {
                slot,
                entry_id: item_id,
                count: dest.count,
                max_durability: self.item_template_max_durability(item_id),
            });
        }

        let mut tx = SqlTransaction::new();
        for &(_, _, db_guid, new_count, _) in &planned_existing_counts {
            let mut upd_count = char_db.prepare(CharStatements::UPD_ITEM_INSTANCE_COUNT);
            upd_count.set_u32(0, new_count);
            upd_count.set_u64(1, db_guid);
            tx.append(upd_count);
        }

        let realm_id = self.realm_id();
        let mut created_new_stacks = Vec::new();
        if !planned_new_stacks.is_empty() {
            let max_guid_stmt = char_db.prepare(CharStatements::SEL_MAX_ITEM_GUID);
            let mut next_item_guid = match char_db.query(&max_guid_stmt).await {
                Ok(r) => r.try_read::<u64>(0).unwrap_or(0) + 1,
                Err(_) => 1,
            };

            for stack in &planned_new_stacks {
                let db_guid = next_item_guid;
                next_item_guid += 1;
                let item_guid = ObjectGuid::create_item(realm_id, db_guid as i64);

                let mut ins_item = char_db.prepare(CharStatements::INS_ITEM_INSTANCE);
                ins_item.set_u64(0, db_guid);
                ins_item.set_u32(1, stack.entry_id);
                ins_item.set_u64(2, player_guid.counter() as u64);
                ins_item.set_u32(3, stack.count);
                ins_item.set_u32(4, stack.max_durability);
                tx.append(ins_item);

                let mut ins_inv = char_db.prepare(CharStatements::INS_CHAR_INVENTORY);
                ins_inv.set_u64(0, player_guid.counter() as u64);
                ins_inv.set_u8(1, stack.slot);
                ins_inv.set_u64(2, db_guid);
                tx.append(ins_inv);

                created_new_stacks.push((stack.clone(), db_guid, item_guid));
            }
        }

        if let Err(e) = char_db.commit_transaction(tx).await {
            warn!("LootItem: store transaction failed: {e}");
            self.send_equip_error(InventoryResult::InvFull, None, None, 0, 0);
            return false;
        }

        for &(_, item_guid, _, new_count, _) in &planned_existing_counts {
            if let Some(item) = self.inventory_item_objects.get_mut(&item_guid) {
                item.set_count(new_count);
            }
        }

        for (stack, db_guid, item_guid) in &created_new_stacks {
            self.inventory_items.insert(
                stack.slot,
                InventoryItem {
                    guid: *item_guid,
                    entry_id: stack.entry_id,
                    db_guid: *db_guid,
                    inventory_type: self.item_template_inventory_type(stack.entry_id),
                },
            );
            let item_object = self.make_inventory_item_object(
                *item_guid,
                stack.entry_id,
                player_guid,
                stack.count,
                stack.max_durability,
                ItemContext::None,
                stack.slot,
            );
            self.insert_inventory_item_object(item_object);
        }
        self.sync_object_accessor_player();

        let map_id = self.current_map_id;
        if !created_new_stacks.is_empty() {
            let item_creates = created_new_stacks
                .iter()
                .map(|(stack, _, item_guid)| ItemCreateData {
                    item_guid: *item_guid,
                    entry_id: stack.entry_id as i32,
                    owner_guid: player_guid,
                    contained_in: player_guid,
                    stack_count: stack.count,
                    durability: stack.max_durability,
                    max_durability: stack.max_durability,
                })
                .collect();
            self.send_packet(&UpdateObject::create_items(item_creates, map_id));
        }

        for &(slot, item_guid, _, new_count, added_count) in &planned_existing_counts {
            self.send_packet(&UpdateObject::item_stack_count_update(
                item_guid, map_id, new_count,
            ));
            self.send_loot_item_push_result(
                player_guid,
                item_guid,
                item_id,
                slot,
                added_count,
                new_count,
                false,
            );
        }

        for (stack, _, item_guid) in &created_new_stacks {
            self.send_loot_item_push_result(
                player_guid,
                *item_guid,
                stack.entry_id,
                stack.slot,
                stack.count,
                stack.count,
                false,
            );
        }

        if !created_new_stacks.is_empty() {
            self.send_packet(&UpdateObject::player_values_update(
                player_guid,
                map_id,
                created_new_stacks
                    .iter()
                    .map(|(stack, _, item_guid)| (stack.slot, *item_guid))
                    .collect(),
                Vec::new(),
                Vec::new(),
            ));
        }

        true
    }

    fn send_loot_item_push_result(
        &self,
        player_guid: ObjectGuid,
        item_guid: ObjectGuid,
        item_id: u32,
        slot: u8,
        quantity: u32,
        quantity_in_inventory: u32,
        created: bool,
    ) {
        self.send_packet(&ItemPushResult {
            player_guid,
            slot: u8::from(INVENTORY_SLOT_BAG_0),
            slot_in_bag: i32::from(slot),
            item: ItemInstance {
                item_id: item_id as i32,
                random_properties_seed: 0,
                random_properties_id: 0,
                item_bonus: None,
                modifications: ItemModList { values: Vec::new() },
            },
            quest_log_item_id: 0,
            quantity: quantity as i32,
            quantity_in_inventory: quantity_in_inventory as i32,
            dungeon_encounter_id: 0,
            battle_pet_species_id: 0,
            battle_pet_breed_id: 0,
            battle_pet_breed_quality: 0,
            battle_pet_level: 0,
            item_guid,
            pushed: false,
            display_text: ItemPushResultDisplayType::Normal,
            created,
            is_bonus_roll: false,
            is_encounter_loot: false,
        });
    }

    async fn destroy_fully_looted_direct_item(&mut self, item_guid: ObjectGuid) {
        let player_guid = match self.player_guid {
            Some(guid) => guid,
            None => return,
        };

        let runtime_item = self.inventory_item_objects.get(&item_guid).cloned();
        let (bag, slot) = match runtime_item.as_ref() {
            Some(item) => (item.bag_slot(), item.slot()),
            None => return,
        };

        let Some(item) = self.get_inventory_item_by_pos(bag, slot) else {
            return;
        };

        let char_db = match self.char_db() {
            Some(db) => Arc::clone(db),
            None => return,
        };

        let mut tx = SqlTransaction::new();
        let should_expire_refund = runtime_item
            .as_ref()
            .is_some_and(|item_object| item_object.is_refundable());
        if should_expire_refund {
            let mut del_refund = char_db.prepare(CharStatements::DEL_ITEM_REFUND_INSTANCE);
            del_refund.set_u64(0, item.db_guid);
            tx.append(del_refund);
        }

        let mut del_inv = char_db.prepare(CharStatements::DEL_CHAR_INVENTORY_ITEM);
        del_inv.set_u64(0, player_guid.counter() as u64);
        del_inv.set_u64(1, item.db_guid);
        tx.append(del_inv);

        let mut del_item = char_db.prepare(CharStatements::DEL_ITEM_INSTANCE);
        del_item.set_u64(0, item.db_guid);
        tx.append(del_item);

        if let Err(e) = char_db.commit_transaction(tx).await {
            warn!("LootRelease: delete fully looted item failed: {e}");
            return;
        }

        self.remove_fully_looted_runtime_item(bag, slot, item.guid);

        if should_expire_refund {
            self.send_packet(&ItemExpirePurchaseRefund { item_guid: item.guid });
        }

        // Player-values update and stat refresh only apply to top-level slots.
        if bag == INVENTORY_SLOT_BAG_0 {
            let mut visible_item_changes = Vec::new();
            let mut virtual_item_changes = Vec::new();
            if (slot as usize) < 19 {
                visible_item_changes.push((slot, 0i32, 0u16, 0u16));
            }
            if slot >= 15 && slot <= 17 {
                virtual_item_changes.push((slot - 15, 0i32, 0u16, 0u16));
            }

            self.send_packet(&UpdateObject::player_values_update(
                player_guid,
                self.current_map_id,
                vec![(slot, ObjectGuid::EMPTY)],
                visible_item_changes,
                virtual_item_changes,
            ));

            if slot < 19 {
                self.send_stat_update();
            }
        }
    }
}

// ── Loot generation ───────────────────────────────────────────────

/// Generate loot for a dead creature.
///
/// For now: random coins based on level. Item drops TODO (needs loot template DB query).
fn generate_creature_loot(creature_guid: ObjectGuid, level: u8, _entry: u32) -> CreatureLoot {
    // Coin drop: roughly (level * 2) to (level * 5) copper, converted to silver/gold.
    // 1 gold = 10000 copper, 1 silver = 100 copper.
    let base = level as u32;
    // Use GUID counter as a cheap seed for randomness.
    let seed = creature_guid.counter() as u32;
    let copper = base * 200 + (seed % (base * 300 + 1));
    let coins = copper; // Sent as raw copper to client.

    // TODO: query creature_loot_template from world DB for item drops.
    let items = vec![];

    CreatureLoot {
        loot_guid: creature_guid,
        coins,
        items,
        looted_by_player: false,
    }
}

fn loot_is_looted_like_cpp(loot: &CreatureLoot) -> bool {
    loot.coins == 0 && loot.items.iter().all(|entry| entry.taken)
}

#[derive(Debug, Clone)]
struct PlannedLootNewStack {
    slot: u8,
    entry_id: u32,
    count: u32,
    max_durability: u32,
}

#[cfg(test)]
mod tests {
    use super::loot_is_looted_like_cpp;
    use wow_core::ObjectGuid;
    use wow_packet::packets::loot::{CreatureLoot, LootEntry};

    #[test]
    fn loot_is_looted_requires_no_money_and_no_unlooted_items_like_cpp() {
        let mut loot = CreatureLoot {
            loot_guid: ObjectGuid::EMPTY,
            coins: 1,
            items: vec![],
            looted_by_player: false,
        };
        assert!(!loot_is_looted_like_cpp(&loot));

        loot.coins = 0;
        loot.items.push(LootEntry {
            loot_list_id: 0,
            item_id: 25,
            quantity: 1,
            taken: false,
        });
        assert!(!loot_is_looted_like_cpp(&loot));

        loot.items[0].taken = true;
        assert!(loot_is_looted_like_cpp(&loot));
    }
}
