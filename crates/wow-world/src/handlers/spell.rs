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
use wow_constants::{ClientOpcodes, InventoryResult, ItemFlags};
use wow_entities::INVENTORY_SLOT_BAG_0;
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_packet::packets::loot::{CreatureLoot, LootItemData, LootResponse};
use wow_packet::packets::spell::{
    CastFailed, CastSpellRequest, OpenItem, SpellCastVisual, SpellStartPkt, SpellTargetData,
};
use wow_packet::ClientPacket;

use crate::session::WorldSession;

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
            let coins = match stored_money {
                Some(money) => money,
                None => {
                    let (min_money, max_money) = self
                        .load_item_template_addon_money_loot_like_cpp(item.entry_id)
                        .await;
                    generate_money_loot_like_cpp(min_money, max_money, &mut rand::thread_rng())
                }
            };
            self.loot_table.insert(item.guid, CreatureLoot {
                loot_guid: item.guid,
                coins,
                items: Vec::new(),
                looted_by_player: false,
            });

            if stored_money.is_none() && coins > 0 {
                self.save_new_stored_item_money_like_cpp(item.guid, coins)
                    .await;
            }
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

    async fn save_new_stored_item_money_like_cpp(&self, item_guid: wow_core::ObjectGuid, money: u32) {
        let Some(char_db) = self.char_db().map(Arc::clone) else {
            return;
        };

        let mut tx = SqlTransaction::new();

        let mut del_money = char_db.prepare(CharStatements::DEL_ITEMCONTAINER_MONEY);
        del_money.set_u64(0, item_guid.counter() as u64);
        tx.append(del_money);

        let mut ins_money = char_db.prepare(CharStatements::INS_ITEMCONTAINER_MONEY);
        ins_money.set_u64(0, item_guid.counter() as u64);
        ins_money.set_u32(1, money);
        tx.append(ins_money);

        if let Err(err) = char_db.commit_transaction(tx).await {
            warn!(
                item_guid = item_guid.counter(),
                money,
                error = %err,
                "failed to save stored item loot money"
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

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::generate_money_loot_like_cpp;

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
}
