// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Spell cast handlers — CMSG_CAST_SPELL, CMSG_CANCEL_CAST, CMSG_CANCEL_CHANNELLING.
//!
//! Phase 2 ("efectos mecánicos"): 
//!   1. Parse CMSG_CAST_SPELL.
//!   2. Validate known spell + cooldown.
//!   3. If cast_time > 0: send SMSG_SPELL_START, store active_spell_cast, wait.
//!   4. When cast completes: execute_spell() → apply effects + cooldown.
//!   5. If instant: execute immediately.
//!
//! Future phases will add:
//!   - Movement cancellation (CMSG_MOVE_* while casting)
//!   - Channelling spells (tick-based damage)
//!   - Interrupts & silences
//!
//! Reference: C# Game/Handlers/SpellHandler.cs, Game/Spells/Spell.cs

use std::sync::Arc;

use rand::Rng;
use tracing::{debug, info, warn};

use wow_database::{CharStatements, SqlTransaction, WorldStatements};
use wow_constants::{BagFamilyMask, ClientOpcodes, InventoryResult, ItemFlags};
use wow_entities::INVENTORY_SLOT_BAG_0;
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_packet::packets::loot::{CreatureLoot, LootEntry, LootItemData, LootResponse};
use wow_packet::packets::spell::{
    CastFailed, CastSpellRequest, OpenItem, SpellCastVisual, SpellStartPkt, SpellTargetData,
};
use wow_packet::ClientPacket;

use crate::session::WorldSession;

const LOOT_MODE_DEFAULT_LIKE_CPP: u16 = 1;
const MAX_NR_LOOT_ITEMS_LIKE_CPP: usize = 18;
const MAX_LOOT_REFERENCE_FRAMES_LIKE_CPP: u32 = 64;

// ── Handler registrations ─────────────────────────────────────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CastSpell,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_cast_spell",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CancelCast,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_cancel_cast",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CancelChannelling,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_cancel_channelling",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::OpenItem,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_open_item",
    }
}

// ── Handler implementations ───────────────────────────────────────

impl WorldSession {
    /// Handle `CMSG_CAST_SPELL` (0x329C).
    ///
    /// Phase 2: cast timers + cooldowns + mechanical effects.
    /// 
    /// Flow:
    /// 1. Validate spell is known.
    /// 2. Validate cooldown.
    /// 3. If cast_time > 0: initiate cast (SMSG_SPELL_START), wait for tick_active_spell_cast().
    /// 4. If instant: execute immediately.
    pub async fn handle_cast_spell(&mut self, mut pkt: wow_packet::WorldPacket) {
        let player_guid = match self.player_guid {
            Some(g) => g,
            None => {
                warn!("handle_cast_spell: no player_guid");
                return;
            }
        };

        let req = match CastSpellRequest::read(&mut pkt) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    account = self.account_id,
                    "Failed to parse CMSG_CAST_SPELL: {e}"
                );
                return;
            }
        };

        let spell_id = req.spell_id;
        let cast_id = req.cast_id;

        debug!(
            account = self.account_id,
            spell_id = spell_id,
            cast_id = ?cast_id,
            target = ?req.target.unit,
            "CMSG_CAST_SPELL"
        );

        // ── Validation: Known spell ─────────────────────────────────────
        if !self.known_spells.contains(&spell_id) {
            warn!(
                account = self.account_id,
                spell_id = spell_id,
                "Cast attempt for unknown spell"
            );
            self.send_packet(&CastFailed {
                cast_id,
                spell_id,
                reason: 2,    // SpellCastResult::NotKnown
                fail_arg1: 0,
                fail_arg2: 0,
            });
            return;
        }

        // ── Get spell info ──────────────────────────────────────────────
        let spell_info: &wow_data::SpellInfo = match &self.spell_store {
            Some(store) => match store.get(spell_id) {
                Some(info) => info,
                None => {
                    warn!(
                        account = self.account_id,
                        spell_id = spell_id,
                        "Spell not found in store"
                    );
                    self.send_packet(&CastFailed {
                        cast_id,
                        spell_id,
                        reason: 2,
                        fail_arg1: 0,
                        fail_arg2: 0,
                    });
                    return;
                }
            },
            None => {
                warn!(account = self.account_id, "No spell store available");
                self.send_packet(&CastFailed {
                    cast_id,
                    spell_id,
                    reason: 2,
                    fail_arg1: 0,
                    fail_arg2: 0,
                });
                return;
            }
        };

        // ── Validation: Cooldown ────────────────────────────────────────
        // Check global cooldown (GCD)
        if let Some(last_cast) = self.last_spell_cast_time {
            let cooldown_ms = spell_info.effective_cooldown_ms();
            let elapsed_ms = last_cast.elapsed().as_millis() as u32;
            
            if elapsed_ms < cooldown_ms {
                debug!(
                    account = self.account_id,
                    spell_id = spell_id,
                    remaining_ms = cooldown_ms - elapsed_ms,
                    "Spell on global cooldown"
                );
                self.send_packet(&CastFailed {
                    cast_id,
                    spell_id,
                    reason: 10,   // SpellCastResult::NotReady
                    fail_arg1: 0,
                    fail_arg2: 0,
                });
                return;
            }
        }
        
        // Check per-spell cooldown
        if spell_info.recovery_time_ms > 0 {
            if let Some(last_spell_cast) = self.last_spell_cast_time_per_spell.get(&spell_id) {
                let elapsed_ms = last_spell_cast.elapsed().as_millis() as u32;
                let cooldown_ms = spell_info.recovery_time_ms;
                
                if elapsed_ms < cooldown_ms {
                    debug!(
                        account = self.account_id,
                        spell_id = spell_id,
                        remaining_ms = cooldown_ms - elapsed_ms,
                        "Spell on per-spell cooldown"
                    );
                    self.send_packet(&CastFailed {
                        cast_id,
                        spell_id,
                        reason: 10,   // SpellCastResult::NotReady
                        fail_arg1: 0,
                        fail_arg2: 0,
                    });
                    return;
                }
            }
        }

        // ── Build target ────────────────────────────────────────────────
        let (target_flags, target_guid) = if !req.target.unit.is_empty() {
            (0x2u32, req.target.unit)  // SpellCastTargetFlags::Unit
        } else {
            (0x2u32, player_guid)      // self-target
        };

        let spell_target = SpellTargetData {
            flags: target_flags,
            unit: target_guid,
            item: wow_core::ObjectGuid::EMPTY,
        };

        // ── Initiate cast or execute immediately ─────────────────────────
        if spell_info.has_cast_time() {
            // Cast with delay — send SMSG_SPELL_START and store state
            debug!(
                account = self.account_id,
                spell_id = spell_id,
                cast_time_ms = spell_info.cast_time_ms,
                "Starting cast with timer"
            );

            let start_pkt = SpellStartPkt {
                caster: player_guid,
                cast_id,
                spell_id,
                visual: SpellCastVisual {
                    spell_visual_id: req.visual.spell_visual_id,
                    script_visual_id: 0,
                },
                target: spell_target,
                cast_time_ms: spell_info.cast_time_ms,
            };
            self.send_packet(&start_pkt);

            // Store active cast state
            self.active_spell_cast = Some(crate::session::SpellCastState {
                spell_id,
                target_guid,
                cast_id,
                cast_start_time: std::time::Instant::now(),
                cast_time_ms: spell_info.cast_time_ms,
                spell_visual: SpellCastVisual {
                    spell_visual_id: req.visual.spell_visual_id,
                    script_visual_id: 0,
                },
            });

            info!(
                account = self.account_id,
                spell_id = spell_id,
                "Cast initiated ({}ms cast time)",
                spell_info.cast_time_ms
            );
        } else {
            // Instant cast — execute immediately
            debug!(
                account = self.account_id,
                spell_id = spell_id,
                "Instant cast, executing immediately"
            );
            if let Err(e) = self.execute_spell(spell_id, target_guid).await {
                warn!(account = self.account_id, "Instant spell execution failed: {}", e);
                return;
            }

            info!(
                account = self.account_id,
                spell_id = spell_id,
                "Instant spell executed"
            );
        }
    }

    /// Handle `CMSG_OPEN_ITEM`.
    ///
    /// This ports Trinity's initial validation and fails closed until item loot
    /// storage/generation is represented in Rust.
    pub async fn handle_open_item(&mut self, mut pkt: wow_packet::WorldPacket) {
        let open = match OpenItem::read(&mut pkt) {
            Ok(open) => open,
            Err(e) => {
                warn!(account = self.account_id, "Failed to parse CMSG_OPEN_ITEM: {e}");
                return;
            }
        };

        debug!(
            account = self.account_id,
            slot = open.slot,
            pack_slot = open.pack_slot,
            "CMSG_OPEN_ITEM"
        );

        if open.slot != INVENTORY_SLOT_BAG_0 {
            self.send_equip_error(InventoryResult::InternalBagError, None, None, 0, 0);
            return;
        }

        let Some(item) = self.inventory_items.get(&open.pack_slot).cloned() else {
            self.send_equip_error(InventoryResult::ItemNotFound, None, None, 0, 0);
            return;
        };

        let Some(flags) = self.item_template_flags(item.entry_id) else {
            self.send_equip_error(InventoryResult::ItemNotFound, Some(item.guid), None, 0, 0);
            return;
        };

        if !flags.contains(ItemFlags::HAS_LOOT) {
            self.send_equip_error(InventoryResult::ClientLockedOut, Some(item.guid), None, 0, 0);
            return;
        }

        if !self.loot_table.contains_key(&item.guid) {
            let stored_money = self.load_stored_item_money_like_cpp(item.guid).await;
            let stored_items = self.load_stored_item_items_like_cpp(item.guid).await;
            let loaded_stored_loot = stored_money.is_some() || stored_items.is_some();
            let (coins, items) = if loaded_stored_loot {
                (stored_money.unwrap_or(0), stored_items.unwrap_or_default())
            } else {
                let coins = {
                    let (min_money, max_money) = self
                        .load_item_template_addon_money_loot_like_cpp(item.entry_id)
                        .await;
                    generate_money_loot_like_cpp(min_money, max_money, &mut rand::thread_rng())
                };
                let items = self
                    .generate_item_loot_template_entries_like_cpp(item.entry_id)
                    .await;
                (coins, items)
            };
            if !loaded_stored_loot && (coins > 0 || !items.is_empty()) {
                self.save_new_stored_item_loot_like_cpp(item.guid, coins, &items)
                    .await;
            }

            self.loot_table.insert(item.guid, CreatureLoot {
                loot_guid: item.guid,
                coins,
                items,
                looted_by_player: false,
            });
        }

        if let Some(item_object) = self.inventory_item_objects.get_mut(&item.guid) {
            item_object.set_loot_generated(true);
        }

        let Some(loot) = self.loot_table.get(&item.guid) else {
            self.send_equip_error(InventoryResult::ClientLockedOut, Some(item.guid), None, 0, 0);
            return;
        };

        let items: Vec<LootItemData> = loot
            .items
            .iter()
            .filter(|entry| !entry.taken)
            .map(|entry| LootItemData {
                loot_list_id: entry.loot_list_id,
                ui_type: 0,
                quantity: entry.quantity,
                item_id: entry.item_id as i32,
                item_context: 0,
                bonus_list_ids: vec![],
                can_loot: true,
            })
            .collect();

        self.send_packet(&LootResponse {
            owner: item.guid,
            loot_obj: loot.loot_guid,
            failure_reason: 0,
            acquire_reason: 0,
            loot_method: 0,
            threshold: 2,
            coins: loot.coins,
            items,
            acquired: true,
            ae_looting: false,
        });
    }

    async fn load_item_template_addon_money_loot_like_cpp(&self, item_entry: u32) -> (u32, u32) {
        let Some(world_db) = self.world_db() else {
            return (0, 0);
        };

        let mut stmt = world_db.prepare(WorldStatements::SEL_ITEM_TEMPLATE_ADDON_MONEY_LOOT);
        stmt.set_u32(0, item_entry);

        match world_db.query(&stmt).await {
            Ok(result) if !result.is_empty() => {
                let min_money = result.try_read::<u32>(0).unwrap_or(0);
                let max_money = result.try_read::<u32>(1).unwrap_or(0);
                (min_money, max_money)
            }
            Ok(_) => (0, 0),
            Err(err) => {
                warn!(
                    item_entry,
                    error = %err,
                    "failed to load item_template_addon money loot"
                );
                (0, 0)
            }
        }
    }

    async fn generate_item_loot_template_entries_like_cpp(
        &self,
        item_entry: u32,
    ) -> Vec<LootEntry> {
        let mut loot_items = Vec::new();
        let mut frames = Vec::new();
        let rows = self
            .load_loot_template_rows_like_cpp(LootTemplateTable::Item, item_entry)
            .await;
        frames.push(LootTemplateFrame {
            rows,
            index: 0,
            group_id: 0,
            groups_enqueued: false,
        });

        let mut processed_frames = 0u32;
        while let Some(mut frame) = frames.pop() {
            if frame.group_id != 0 {
                let mut rng = rand::thread_rng();
                if let Some(row) = roll_group_loot_row_like_cpp(
                    &frame.rows,
                    frame.group_id,
                    |item_id| self.item_storage_template(item_id).is_some(),
                    &mut rng,
                ) {
                    add_loot_template_row_item_like_cpp(&mut loot_items, &row, |item_id| {
                        self.item_storage_template(item_id)
                            .map(|template| template.max_stack_size)
                            .unwrap_or(1)
                    });
                }
                continue;
            }

            if frame.index >= frame.rows.len() {
                if !frame.groups_enqueued {
                    frame.groups_enqueued = true;
                    let mut groups: Vec<u8> = frame
                        .rows
                        .iter()
                        .filter(|row| row.reference == 0 && row.group_id != 0)
                        .map(|row| row.group_id)
                        .collect();
                    groups.sort_unstable();
                    groups.dedup();

                    if !groups.is_empty() {
                        let rows = frame.rows.clone();
                        frames.push(frame);
                        for group_id in groups.into_iter().rev() {
                            frames.push(LootTemplateFrame {
                                rows: rows.clone(),
                                index: 0,
                                group_id,
                                groups_enqueued: true,
                            });
                        }
                    }
                }
                continue;
            }
            let row = frame.rows[frame.index].clone();
            frame.index += 1;
            frames.push(frame);

            if row.loot_mode & LOOT_MODE_DEFAULT_LIKE_CPP == 0 {
                continue;
            }

            if row.group_id != 0 && row.reference == 0 {
                continue;
            }

            let mut rng = rand::thread_rng();
            if row.reference > 0 {
                if !loot_template_reference_row_can_roll_like_cpp(
                    row.reference,
                    row.chance,
                    row.loot_mode,
                    row.min_count,
                ) {
                    continue;
                }
                if row.chance < 100.0 && rng.gen_range(0.0f32..100.0f32) >= row.chance {
                    continue;
                }

                let reference_rows = self
                    .load_loot_template_rows_like_cpp(LootTemplateTable::Reference, row.reference)
                    .await;
                for _ in 0..row.max_count {
                    frames.push(LootTemplateFrame {
                        rows: reference_rows.clone(),
                        index: 0,
                        group_id: row.group_id,
                        groups_enqueued: false,
                    });
                }
                processed_frames = processed_frames.saturating_add(1);
                if processed_frames > MAX_LOOT_REFERENCE_FRAMES_LIKE_CPP {
                    warn!(
                        item_entry,
                        reference = row.reference,
                        "stopped item loot reference processing after safety cap"
                    );
                    break;
                }
                continue;
            }

            if !loot_template_plain_row_can_roll_like_cpp(
                row.item_id,
                row.chance,
                row.needs_quest,
                row.loot_mode,
                row.min_count,
                row.max_count,
                self.item_storage_template(row.item_id).is_some(),
            ) {
                continue;
            }
            if row.chance < 100.0 && rng.gen_range(0.0f32..100.0f32) >= row.chance {
                continue;
            }
            add_loot_template_row_item_like_cpp(&mut loot_items, &row, |item_id| {
                self.item_storage_template(item_id)
                    .map(|template| template.max_stack_size)
                    .unwrap_or(1)
            });
        }

        loot_items
    }

    async fn load_loot_template_rows_like_cpp(
        &self,
        table: LootTemplateTable,
        entry: u32,
    ) -> Vec<LootTemplateRow> {
        let Some(world_db) = self.world_db() else {
            return Vec::new();
        };

        let statement = match table {
            LootTemplateTable::Item => WorldStatements::SEL_ITEM_LOOT_TEMPLATE_ROWS,
            LootTemplateTable::Reference => WorldStatements::SEL_REFERENCE_LOOT_TEMPLATE_ROWS,
        };
        let mut stmt = world_db.prepare(statement);
        stmt.set_u32(0, entry);

        let mut result = match world_db.query(&stmt).await {
            Ok(result) => result,
            Err(err) => {
                warn!(
                    entry,
                    table = table.name(),
                    error = %err,
                    "failed to load loot template rows"
                );
                return Vec::new();
            }
        };

        let mut rows = Vec::new();
        if result.is_empty() {
            return rows;
        }

        loop {
            rows.push(LootTemplateRow {
                item_id: result.try_read::<u32>(0).unwrap_or(0),
                reference: result.try_read::<u32>(1).unwrap_or(0),
                chance: result.try_read::<f32>(2).unwrap_or(0.0),
                needs_quest: result.try_read::<bool>(3).unwrap_or(false),
                loot_mode: result.try_read::<u16>(4).unwrap_or(0),
                group_id: result.try_read::<u8>(5).unwrap_or(0),
                min_count: result.try_read::<u8>(6).unwrap_or(0),
                max_count: result.try_read::<u8>(7).unwrap_or(0),
            });

            if !result.next_row() {
                break;
            }
        }

        rows
    }

    async fn load_stored_item_money_like_cpp(&self, item_guid: wow_core::ObjectGuid) -> Option<u32> {
        let char_db = self.char_db().map(Arc::clone)?;

        let mut stmt = char_db.prepare(CharStatements::SEL_ITEMCONTAINER_MONEY);
        stmt.set_u64(0, item_guid.counter() as u64);

        match char_db.query(&stmt).await {
            Ok(result) if !result.is_empty() => result.try_read::<u32>(0),
            Ok(_) => None,
            Err(err) => {
                warn!(
                    item_guid = item_guid.counter(),
                    error = %err,
                    "failed to load stored item loot money"
                );
                None
            }
        }
    }

    async fn load_stored_item_items_like_cpp(
        &self,
        item_guid: wow_core::ObjectGuid,
    ) -> Option<Vec<LootEntry>> {
        let char_db = self.char_db().map(Arc::clone)?;

        let mut stmt = char_db.prepare(CharStatements::SEL_ITEMCONTAINER_ITEMS);
        stmt.set_u64(0, item_guid.counter() as u64);

        let mut result = match char_db.query(&stmt).await {
            Ok(result) if !result.is_empty() => result,
            Ok(_) => return None,
            Err(err) => {
                warn!(
                    item_guid = item_guid.counter(),
                    error = %err,
                    "failed to load stored item loot rows"
                );
                return None;
            }
        };

        let mut items = Vec::new();
        loop {
            let item_id = result.try_read::<u32>(0).unwrap_or(0);
            let count = result.try_read::<u32>(1).unwrap_or(0);
            let item_index = result.try_read::<u32>(2).unwrap_or(u32::MAX);
            let _follow_rules = result.try_read::<bool>(3).unwrap_or(false);
            let _ffa = result.try_read::<bool>(4).unwrap_or(false);
            let blocked = result.try_read::<bool>(5).unwrap_or(false);
            let _counted = result.try_read::<bool>(6).unwrap_or(false);
            let _under_threshold = result.try_read::<bool>(7).unwrap_or(false);
            let needs_quest = result.try_read::<bool>(8).unwrap_or(false);
            let random_properties_id = result.try_read::<i32>(9).unwrap_or(0);
            let random_properties_seed = result.try_read::<i32>(10).unwrap_or(0);
            let context = result.try_read::<u8>(11).unwrap_or(0);

            if stored_item_row_can_load_like_cpp_representable(
                item_id,
                count,
                item_index,
                blocked,
                needs_quest,
                random_properties_id,
                random_properties_seed,
                context,
                self.item_storage_template(item_id).is_some(),
            ) {
                items.push(LootEntry {
                    loot_list_id: item_index as u8,
                    item_id,
                    quantity: count,
                    taken: false,
                });
            }

            if !result.next_row() {
                break;
            }
        }

        Some(items)
    }

    async fn save_new_stored_item_loot_like_cpp(
        &self,
        item_guid: wow_core::ObjectGuid,
        money: u32,
        items: &[LootEntry],
    ) {
        let Some(char_db) = self.char_db().map(Arc::clone) else {
            return;
        };

        let mut tx = SqlTransaction::new();

        if money > 0 {
            let mut del_money = char_db.prepare(CharStatements::DEL_ITEMCONTAINER_MONEY);
            del_money.set_u64(0, item_guid.counter() as u64);
            tx.append(del_money);

            let mut ins_money = char_db.prepare(CharStatements::INS_ITEMCONTAINER_MONEY);
            ins_money.set_u64(0, item_guid.counter() as u64);
            ins_money.set_u32(1, money);
            tx.append(ins_money);
        }

        let mut del_items = char_db.prepare(CharStatements::DEL_ITEMCONTAINER_ITEMS);
        del_items.set_u64(0, item_guid.counter() as u64);
        tx.append(del_items);

        for item in items {
            let template = self.item_storage_template(item.item_id);
            if !stored_loot_item_should_persist_like_cpp(
                template.is_some(),
                template.map(|t| t.bag_family).unwrap_or(BagFamilyMask::NONE),
            ) {
                continue;
            }

            let flags = template.map(|t| t.flags).unwrap_or(ItemFlags::empty());

            let mut ins_item = char_db.prepare(CharStatements::INS_ITEMCONTAINER_ITEMS);
            ins_item.set_u64(0, item_guid.counter() as u64);
            ins_item.set_u32(1, item.item_id);
            ins_item.set_u32(2, item.quantity);
            ins_item.set_u32(3, u32::from(item.loot_list_id));
            ins_item.set_bool(4, true);
            ins_item.set_bool(5, flags.contains(ItemFlags::MULTI_DROP));
            ins_item.set_bool(6, false);
            ins_item.set_bool(7, false);
            ins_item.set_bool(8, false);
            ins_item.set_bool(9, false);
            ins_item.set_i32(10, 0);
            ins_item.set_i32(11, 0);
            ins_item.set_u8(12, 0);
            tx.append(ins_item);
        }

        if let Err(err) = char_db.commit_transaction(tx).await {
            warn!(
                item_guid = item_guid.counter(),
                money,
                error = %err,
                "failed to save stored item loot rows"
            );
        }
    }

    /// Handle `CMSG_CANCEL_CAST` — player cancels an in-progress cast.
    pub async fn handle_cancel_cast(&mut self, _pkt: wow_packet::WorldPacket) {
        // TODO: Phase 3 — implement cancel cast
    }

    /// Handle `CMSG_CANCEL_CHANNELLING` — player stops a channelled spell.
    pub async fn handle_cancel_channelling(&mut self, _pkt: wow_packet::WorldPacket) {
        // TODO: Phase 3 — implement cancel channelling
    }
}

fn stored_loot_item_should_persist_like_cpp(template_exists: bool, bag_family: BagFamilyMask) -> bool {
    if !template_exists {
        return false;
    }
    !bag_family.contains(BagFamilyMask::CURRENCY_TOKENS)
}

fn generate_money_loot_like_cpp<R: Rng + ?Sized>(
    min_amount: u32,
    max_amount: u32,
    rng: &mut R,
) -> u32 {
    if max_amount == 0 {
        return 0;
    }

    if max_amount <= min_amount {
        return max_amount;
    }

    if max_amount - min_amount < 32_700 {
        return rng.gen_range(min_amount..=max_amount);
    }

    rng.gen_range((min_amount >> 8)..=(max_amount >> 8)) << 8
}

fn loot_template_plain_row_can_roll_like_cpp(
    item_id: u32,
    chance: f32,
    needs_quest: bool,
    loot_mode: u16,
    min_count: u8,
    max_count: u8,
    item_exists: bool,
) -> bool {
    if item_id == 0 || !item_exists || min_count == 0 || max_count < min_count {
        return false;
    }

    if needs_quest {
        return false;
    }

    if chance == 0.0 || (chance != 0.0 && chance < 0.000001) {
        return false;
    }

    loot_mode & LOOT_MODE_DEFAULT_LIKE_CPP != 0
}

fn loot_template_reference_row_can_roll_like_cpp(
    reference: u32,
    chance: f32,
    loot_mode: u16,
    min_count: u8,
) -> bool {
    reference != 0
        && min_count != 0
        && chance != 0.0
        && loot_mode & LOOT_MODE_DEFAULT_LIKE_CPP != 0
}

fn loot_template_group_row_can_roll_like_cpp(
    item_id: u32,
    chance: f32,
    needs_quest: bool,
    loot_mode: u16,
    min_count: u8,
    max_count: u8,
    item_exists: bool,
) -> bool {
    if item_id == 0 || !item_exists || min_count == 0 || max_count < min_count || needs_quest {
        return false;
    }

    if chance != 0.0 && chance < 0.000001 {
        return false;
    }

    loot_mode & LOOT_MODE_DEFAULT_LIKE_CPP != 0
}

fn roll_group_loot_row_like_cpp<R, F>(
    rows: &[LootTemplateRow],
    group_id: u8,
    item_exists: F,
    rng: &mut R,
) -> Option<LootTemplateRow>
where
    R: Rng + ?Sized,
    F: Fn(u32) -> bool,
{
    let possible: Vec<&LootTemplateRow> = rows
        .iter()
        .filter(|row| {
            row.group_id == group_id
                && row.reference == 0
                && loot_template_group_row_can_roll_like_cpp(
                    row.item_id,
                    row.chance,
                    row.needs_quest,
                    row.loot_mode,
                    row.min_count,
                    row.max_count,
                    item_exists(row.item_id),
                )
        })
        .collect();

    let explicitly_chanced: Vec<&LootTemplateRow> = possible
        .iter()
        .copied()
        .filter(|row| row.chance != 0.0)
        .collect();
    if !explicitly_chanced.is_empty() {
        let mut roll = rng.gen_range(0.0f32..100.0f32);
        for row in explicitly_chanced {
            if row.chance >= 100.0 {
                return Some((*row).clone());
            }
            roll -= row.chance;
            if roll < 0.0 {
                return Some((*row).clone());
            }
        }
    }

    let equal_chanced: Vec<&LootTemplateRow> = possible
        .iter()
        .copied()
        .filter(|row| row.chance == 0.0)
        .collect();
    if equal_chanced.is_empty() {
        None
    } else {
        Some((*equal_chanced[rng.gen_range(0..equal_chanced.len())]).clone())
    }
}

fn stored_item_row_can_load_like_cpp_representable(
    item_id: u32,
    count: u32,
    item_index: u32,
    blocked: bool,
    needs_quest: bool,
    random_properties_id: i32,
    random_properties_seed: i32,
    context: u8,
    item_exists: bool,
) -> bool {
    item_id != 0
        && item_exists
        && count != 0
        && item_index <= u32::from(u8::MAX)
        && !blocked
        && !needs_quest
        && random_properties_id == 0
        && random_properties_seed == 0
        && context == 0
}

#[derive(Debug, Clone)]
struct LootTemplateRow {
    item_id: u32,
    reference: u32,
    chance: f32,
    needs_quest: bool,
    loot_mode: u16,
    group_id: u8,
    min_count: u8,
    max_count: u8,
}

#[derive(Debug)]
struct LootTemplateFrame {
    rows: Vec<LootTemplateRow>,
    index: usize,
    group_id: u8,
    groups_enqueued: bool,
}

#[derive(Debug, Clone, Copy)]
enum LootTemplateTable {
    Item,
    Reference,
}

impl LootTemplateTable {
    fn name(self) -> &'static str {
        match self {
            Self::Item => "item_loot_template",
            Self::Reference => "reference_loot_template",
        }
    }
}

fn add_loot_item_stacks_like_cpp(
    loot_items: &mut Vec<LootEntry>,
    item_id: u32,
    mut count: u32,
    max_stack_size: u32,
) {
    while count > 0 && loot_items.len() < MAX_NR_LOOT_ITEMS_LIKE_CPP {
        let quantity = count.min(max_stack_size);
        loot_items.push(LootEntry {
            loot_list_id: loot_items.len() as u8,
            item_id,
            quantity,
            taken: false,
        });
        count = count.saturating_sub(max_stack_size);
    }
}

fn add_loot_template_row_item_like_cpp<F>(
    loot_items: &mut Vec<LootEntry>,
    row: &LootTemplateRow,
    max_stack_size: F,
) where
    F: Fn(u32) -> u32,
{
    let mut rng = rand::thread_rng();
    let rolled_count = rng.gen_range(u32::from(row.min_count)..=u32::from(row.max_count));
    add_loot_item_stacks_like_cpp(
        loot_items,
        row.item_id,
        rolled_count,
        max_stack_size(row.item_id).max(1),
    );
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use wow_constants::BagFamilyMask;

    use super::generate_money_loot_like_cpp;
    use super::{
        add_loot_item_stacks_like_cpp, loot_template_plain_row_can_roll_like_cpp,
        loot_template_group_row_can_roll_like_cpp,
        loot_template_reference_row_can_roll_like_cpp,
        roll_group_loot_row_like_cpp, LootTemplateRow,
        stored_item_row_can_load_like_cpp_representable,
        stored_loot_item_should_persist_like_cpp,
    };

    #[test]
    fn item_money_loot_generation_matches_cpp_boundary_branches() {
        let mut rng = StdRng::seed_from_u64(0xC0FFEE);

        assert_eq!(generate_money_loot_like_cpp(0, 0, &mut rng), 0);
        assert_eq!(generate_money_loot_like_cpp(120, 100, &mut rng), 100);
        assert_eq!(generate_money_loot_like_cpp(100, 100, &mut rng), 100);

        let small_range = generate_money_loot_like_cpp(100, 200, &mut rng);
        assert!((100..=200).contains(&small_range));

        let wide_range = generate_money_loot_like_cpp(1_000, 100_000, &mut rng);
        assert_eq!(wide_range & 0xFF, 0);
        assert!((((1_000 >> 8) << 8)..=((100_000 >> 8) << 8)).contains(&wide_range));
    }

    #[test]
    fn plain_item_loot_template_validation_matches_cpp_basic_guards() {
        assert!(loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, false, 1, 1, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            0, 100.0, false, 1, 1, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 0.0, false, 1, 1, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, true, 1, 1, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, false, 0, 1, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, false, 1, 0, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, false, 1, 4, 3, true
        ));
        assert!(!loot_template_plain_row_can_roll_like_cpp(
            25, 100.0, false, 1, 1, 3, false
        ));
    }

    #[test]
    fn add_loot_item_stacks_caps_like_cpp_max_nr_loot_items() {
        let mut loot_items = Vec::new();
        add_loot_item_stacks_like_cpp(&mut loot_items, 25, 45, 20);
        assert_eq!(loot_items.len(), 3);
        assert_eq!(loot_items[0].quantity, 20);
        assert_eq!(loot_items[1].quantity, 20);
        assert_eq!(loot_items[2].quantity, 5);
        assert_eq!(loot_items[2].loot_list_id, 2);

        let mut capped = Vec::new();
        add_loot_item_stacks_like_cpp(&mut capped, 25, 100, 1);
        assert_eq!(capped.len(), 18);
        assert_eq!(capped[17].loot_list_id, 17);
    }

    #[test]
    fn reference_loot_template_validation_matches_cpp_basic_guards() {
        assert!(loot_template_reference_row_can_roll_like_cpp(10, 100.0, 1, 1));
        assert!(loot_template_reference_row_can_roll_like_cpp(10, 0.0000001, 1, 1));
        assert!(!loot_template_reference_row_can_roll_like_cpp(0, 100.0, 1, 1));
        assert!(!loot_template_reference_row_can_roll_like_cpp(10, 0.0, 1, 1));
        assert!(!loot_template_reference_row_can_roll_like_cpp(10, 100.0, 0, 1));
        assert!(!loot_template_reference_row_can_roll_like_cpp(10, 100.0, 1, 0));
    }

    #[test]
    fn grouped_loot_template_roll_matches_cpp_explicit_then_equal_order() {
        assert!(loot_template_group_row_can_roll_like_cpp(
            25, 0.0, false, 1, 1, 1, true
        ));
        assert!(!loot_template_group_row_can_roll_like_cpp(
            25, 0.0000001, false, 1, 1, 1, true
        ));

        let rows = vec![
            LootTemplateRow {
                item_id: 25,
                reference: 0,
                chance: 100.0,
                needs_quest: false,
                loot_mode: 1,
                group_id: 1,
                min_count: 1,
                max_count: 1,
            },
            LootTemplateRow {
                item_id: 26,
                reference: 0,
                chance: 0.0,
                needs_quest: false,
                loot_mode: 1,
                group_id: 1,
                min_count: 1,
                max_count: 1,
            },
        ];
        let mut rng = StdRng::seed_from_u64(0xBEEF);
        let selected = roll_group_loot_row_like_cpp(&rows, 1, |_| true, &mut rng).unwrap();
        assert_eq!(selected.item_id, 25);

        let equal_rows = vec![
            LootTemplateRow { chance: 0.0, item_id: 25, ..rows[0].clone() },
            LootTemplateRow { chance: 0.0, item_id: 26, ..rows[1].clone() },
        ];
        let mut rng = StdRng::seed_from_u64(0xBEEF);
        let selected = roll_group_loot_row_like_cpp(&equal_rows, 1, |_| true, &mut rng).unwrap();
        assert!([25, 26].contains(&selected.item_id));
    }

    #[test]
    fn stored_item_row_loader_keeps_only_currently_representable_rows() {
        assert!(stored_item_row_can_load_like_cpp_representable(
            25, 2, 7, false, false, 0, 0, 0, true
        ));
        assert!(!stored_item_row_can_load_like_cpp_representable(
            25, 2, 256, false, false, 0, 0, 0, true
        ));
        assert!(!stored_item_row_can_load_like_cpp_representable(
            25, 2, 7, true, false, 0, 0, 0, true
        ));
        assert!(!stored_item_row_can_load_like_cpp_representable(
            25, 2, 7, false, false, 12, 0, 0, true
        ));
    }

    #[test]
    fn stored_loot_item_persistence_skips_missing_template_and_currency_tokens_like_cpp() {
        // template missing -> no persist
        assert!(!stored_loot_item_should_persist_like_cpp(
            false,
            BagFamilyMask::NONE
        ));

        // normal template -> persist
        assert!(stored_loot_item_should_persist_like_cpp(
            true,
            BagFamilyMask::NONE
        ));

        // currency token -> no persist (C++ ItemTemplate::IsCurrencyToken)
        assert!(!stored_loot_item_should_persist_like_cpp(
            true,
            BagFamilyMask::CURRENCY_TOKENS
        ));

        // currency token combined with other families still no persist
        assert!(!stored_loot_item_should_persist_like_cpp(
            true,
            BagFamilyMask::CURRENCY_TOKENS | BagFamilyMask::HERBS
        ));
    }
}
