// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Handlers for miscellaneous client opcodes:
//! SetSelection, AreaTrigger, RequestCemeteryList,
//! TaxiNodeStatusQuery, ChatJoinChannel.

use tracing::{debug, info, warn};
use wow_constants::{ClientOpcodes, ItemExtendedCostFlags};
use wow_database::{SqlTransaction, WorldStatements};
use wow_entities::{
    GAMEOBJECT_TYPE_FISHING_HOLE, GAMEOBJECT_TYPE_GATHERING_NODE, GameObjectTemplateData,
    MAX_GAMEOBJECT_DATA,
};
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_packet::ClientPacket;
use wow_packet::packets::instance::{
    InstanceInfo, InstanceLockInfo, InstanceLockResponse, InstanceReset, InstanceResetFailed,
    PendingRaidLock,
};
use wow_packet::packets::item::{
    GetItemPurchaseData, ItemPurchaseContents, ItemPurchaseRefundCurrency, ItemPurchaseRefundItem,
    SetItemPurchaseData,
};
use wow_packet::packets::misc::{RequestCemeteryListResponse, TaxiNodeStatusPkt};

// ── inventory registrations ───────────────────────────────────────────────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::SetSelection,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_set_selection",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::AreaTrigger,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_area_trigger",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::WorldPortResponse,
        status: SessionStatus::Transfer,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_world_port_response",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestCemeteryList,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_cemetery_list",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::TaxiNodeStatusQuery,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadSafe,
        handler_name: "handle_taxi_node_status_query",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ChatJoinChannel,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_chat_join_channel",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueryTime,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_query_time",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueryNextMailTime,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_query_next_mail_time",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LoadingScreenNotify,
        status: SessionStatus::Authed,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_loading_screen_notify",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ViolenceLevel,
        status: SessionStatus::Authed,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_violence_level",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::OverrideScreenFlash,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_override_screen_flash",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueuedMessagesEnd,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_queued_messages_end",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ChatUnregisterAllAddonPrefixes,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_chat_unregister_all_addon_prefixes",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::SetActionBarToggles,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_set_action_bar_toggles",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::SaveCufProfiles,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_save_cuf_profiles",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GuildSetAchievementTracking,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_guild_set_achievement_tracking",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GetItemPurchaseData,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_get_item_purchase_data",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestForcedReactions,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_forced_reactions",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestBattlefieldStatus,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_battlefield_status",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestRatedPvpInfo,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_rated_pvp_info",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestPvpRewards,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_pvp_rewards",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::DfGetSystemInfo,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_df_get_system_info",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::DfGetJoinStatus,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_df_get_join_status",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CalendarGetNumPending,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_calendar_get_num_pending",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GmTicketGetCaseStatus,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_gm_ticket_get_case_status",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GuildBankRemainingWithdrawMoneyQuery,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_guild_bank_remaining_withdraw_money_query",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::BattlePetRequestJournal,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_battle_pet_request_journal",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ArenaTeamRoster,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_arena_team_roster",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestRaidInfo,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_raid_info",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ResetInstances,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_reset_instances",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::InstanceLockResponse,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_instance_lock_response",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestConquestFormulaConstants,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_conquest_formula_constants",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestLfgListBlacklist,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_lfg_list_blacklist",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::LfgListGetStatus,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_lfg_list_get_status",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GetAccountCharacterList,
        status: SessionStatus::Authed,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_get_account_character_list",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CancelTrade,
        status: SessionStatus::LoggedInOrRecentlyLogout,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_cancel_trade",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ReportClientVariables,
        status: SessionStatus::Authed,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_report_client_variables",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ReportEnabledAddons,
        status: SessionStatus::Authed,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_report_enabled_addons",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::ReportKeybindingExecutionCounts,
        status: SessionStatus::Authed,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_report_keybinding_execution_counts",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueryCountdownTimer,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_request_countdown_timer",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CalendarGet,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_calendar_get",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CloseInteraction,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_close_interaction",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::AuctionListBidderItems,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_auction_list_bidder_items",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::AuctionListOwnerItems,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_auction_list_owner_items",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::AuctionListPendingSales,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_auction_list_pending_sales",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::CommerceTokenGetLog,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_commerce_token_get_log",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GameObjUse,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_game_obj_use",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::GameObjReportUse,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_game_obj_report_use",
    }
}

// ── Handler implementations ───────────────────────────────────────────────────

pub(crate) fn item_purchase_contents_from_extended_cost(
    extended_cost: &wow_data::item_extended_cost::ItemExtendedCostEntry,
    money: u64,
) -> ItemPurchaseContents {
    let mut contents = ItemPurchaseContents {
        money,
        ..Default::default()
    };

    for i in 0..5 {
        contents.items[i] = ItemPurchaseRefundItem {
            item_id: extended_cost.item_id[i] as i32,
            item_count: extended_cost.item_count[i] as i32,
        };

        let season_earned = match i {
            0 => extended_cost
                .flags
                .contains(ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_1),
            1 => extended_cost
                .flags
                .contains(ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_2),
            2 => extended_cost
                .flags
                .contains(ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_3),
            3 => extended_cost
                .flags
                .contains(ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_4),
            4 => extended_cost
                .flags
                .contains(ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_5),
            _ => false,
        };
        if !season_earned {
            contents.currencies[i] = ItemPurchaseRefundCurrency {
                currency_id: extended_cost.currency_id[i] as i32,
                currency_count: extended_cost.currency_count[i] as i32,
            };
        }
    }

    contents
}

impl crate::session::WorldSession {
    /// CMSG_SET_SELECTION — player selected a new target.
    /// C# ref: MiscHandler.HandleSetSelection → player.SetSelection(guid)
    pub async fn handle_set_selection(&mut self, mut pkt: wow_packet::WorldPacket) {
        let target_guid = pkt
            .read_packed_guid()
            .unwrap_or(wow_core::ObjectGuid::EMPTY);
        self.set_selection_guid_like_cpp(Some(target_guid));
        info!(
            "SetSelection: account {} → {:?}",
            self.account_id, target_guid
        );
    }

    /// CMSG_WORLD_PORT_RESPONSE — client confirms it has loaded the new map.
    /// C# ref: MovementHandler.HandleMoveWorldportAck
    /// Sent after SMSG_TRANSFER_PENDING + SMSG_SUSPEND_TOKEN.
    /// We respond with SMSG_NEW_WORLD + SMSG_RESUME_TOKEN and resend world objects.
    pub async fn handle_world_port_response(&mut self, _pkt: wow_packet::WorldPacket) {
        use wow_packet::packets::misc::{NewWorld, ResumeToken};

        let Some((new_map, new_pos)) = self.pending_teleport.take() else {
            warn!(
                "WorldPortResponse from account {} but no pending teleport",
                self.account_id
            );
            self.set_state(crate::session::SessionState::LoggedIn);
            return;
        };

        info!(
            account = self.account_id,
            "WorldPortResponse: completing teleport to map {} ({:.2}, {:.2}, {:.2})",
            new_map,
            new_pos.x,
            new_pos.y,
            new_pos.z
        );

        // Update internal state
        self.set_player_map_position_like_cpp(new_map as u16, new_pos);
        self.update_registry_position();

        // SMSG_NEW_WORLD — place player in new world
        self.send_packet(&NewWorld {
            map_id: new_map,
            pos: new_pos,
            reason: 0,
        });

        // SMSG_RESUME_TOKEN — resume movement processing
        self.send_packet(&ResumeToken {
            sequence_index: 1,
            reason: 1,
        });

        // Back to LoggedIn — handler dispatch resumes
        self.set_state(crate::session::SessionState::LoggedIn);

        // Resend nearby world objects at new position
        self.send_nearby_creatures(new_map as u16, &new_pos, 0)
            .await;
        self.send_nearby_gameobjects(new_map as u16, &new_pos, 0)
            .await;
    }

    /// CMSG_AREA_TRIGGER — player entered an area trigger.
    /// C# ref: MiscHandler.HandleAreaTrigger
    pub async fn handle_area_trigger(&mut self, mut pkt: wow_packet::WorldPacket) {
        let trigger_id: u32 = pkt.read_uint32().unwrap_or(0);

        info!(
            "AreaTrigger: account {} trigger_id={}",
            self.account_id, trigger_id
        );

        // Lookup in area trigger store
        if let Some(store) = self.area_trigger_store() {
            if let Some(trigger) = store.get_trigger(trigger_id) {
                info!(
                    "AreaTrigger {} detected at map {} pos ({}, {}, {})",
                    trigger_id, trigger.map_id, trigger.pos.x, trigger.pos.y, trigger.pos.z
                );

                if let Some(ref teleport) = trigger.teleport {
                    let target_map = teleport.target_map;
                    let target_pos = teleport.target_position;
                    info!(
                        "AreaTrigger {} → teleport to map {} ({:.2}, {:.2}, {:.2})",
                        trigger_id, target_map, target_pos.x, target_pos.y, target_pos.z
                    );
                    self.teleport_to(target_map, target_pos).await;
                }
            } else {
                debug!("Unknown area trigger ID {}", trigger_id);
            }
        } else {
            debug!("Area trigger store not available");
        }
    }

    /// CMSG_REQUEST_CEMETERY_LIST — client asks for graveyards in zone.
    /// C# ref: CharacterHandler.HandleRequestCemeteryList
    /// Returns empty list until graveyard data is implemented.
    pub async fn handle_request_cemetery_list(&mut self, mut pkt: wow_packet::WorldPacket) {
        let is_gossip: bool = pkt.read_uint8().unwrap_or(0) != 0;
        self.send_packet(&RequestCemeteryListResponse::empty(is_gossip));
    }

    /// CMSG_TAXI_NODE_STATUS_QUERY — client asks status of a taxi NPC.
    ///
    /// C# ref: `TaxiHandler.SendTaxiStatus`:
    ///   0 = None (no node found), 1 = Learned, 2 = Unlearned, 3 = NotEligible.
    ///
    /// Without a full taxi mask we default to:
    ///   - NPCFlags includes FlightMaster (0x2000) → `Unlearned` (2)
    ///     so the taxi icon shows as available.
    ///   - Otherwise → `None` (0).
    pub async fn handle_taxi_node_status_query(&mut self, mut pkt: wow_packet::WorldPacket) {
        let unit_guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("TaxiNodeStatusQuery: failed to read unit GUID");
                return;
            }
        };

        const NPC_FLAG_FLIGHT_MASTER: u32 = 0x2000;
        let is_flight_master = self
            .mutate_world_creature(unit_guid, |creature| {
                creature.npc_flags() & NPC_FLAG_FLIGHT_MASTER != 0
            })
            .unwrap_or(false);

        // TaxiNodeStatus: 0=None, 1=Learned, 2=Unlearned, 3=NotEligible
        let status: u8 = if is_flight_master { 2 } else { 0 };

        debug!(
            account = self.account_id,
            ?unit_guid,
            status,
            "TaxiNodeStatusQuery"
        );
        self.send_packet(&TaxiNodeStatusPkt { unit_guid, status });
    }

    /// CMSG_CHAT_JOIN_CHANNEL — player joins a chat channel.
    /// C# ref: ChannelHandler.HandleJoinChannel
    /// Stubbed until ChannelManager is implemented.
    pub async fn handle_chat_join_channel(&mut self, _pkt: wow_packet::WorldPacket) {
        // TODO: parse channel packet and join via ChannelManager.
        // Packet structure (bit-packed): channel_id u32, has_voice bit,
        // name_len bits(7), pass_len bits(7), channel_name, password.
    }

    // ── QueryTime ─────────────────────────────────────────────────────────────

    /// CMSG_QUERY_TIME — client requests current server time.
    /// C# ref: QueryHandler.HandleQueryTime → SendQueryTimeResponse
    pub async fn handle_query_time(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        use wow_packet::packets::misc::QueryTimeResponse;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        self.send_packet(&QueryTimeResponse { current_time: ts });
    }

    // ── QueryNextMailTime ──────────────────────────────────────────────────────

    /// CMSG_QUERY_NEXT_MAIL_TIME — client asks when next mail arrives.
    /// C# ref: MailHandler.HandleQueryNextMailTime
    /// Returns "no mail" (-1.0) until mail system is implemented.
    pub async fn handle_query_next_mail_time(&mut self) {
        use wow_packet::packets::misc::MailQueryNextTimeResult;
        self.send_packet(&MailQueryNextTimeResult::no_mail());
    }

    // ── Silent-ignore stubs ────────────────────────────────────────────────────
    // These opcodes are sent by the client at login but require no server
    // response at this stage (UI state, client-side settings, system queries
    // that return empty data until the respective subsystems are implemented).

    pub async fn handle_loading_screen_notify(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_violence_level(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_override_screen_flash(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_queued_messages_end(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_chat_unregister_all_addon_prefixes(
        &mut self,
        _pkt: wow_packet::WorldPacket,
    ) {
        self.registered_addon_prefixes.clear();
        self.filter_addon_messages = false;
    }
    pub async fn handle_set_action_bar_toggles(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_save_cuf_profiles(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_guild_set_achievement_tracking(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_get_item_purchase_data(&mut self, mut pkt: wow_packet::WorldPacket) {
        let request = match GetItemPurchaseData::read(&mut pkt) {
            Ok(request) => request,
            Err(e) => {
                warn!("GetItemPurchaseData parse failed: {e}");
                return;
            }
        };
        let Some(player_guid) = self.player_guid() else {
            return;
        };
        let current_total_played_time = self.total_played_time.saturating_add(
            self.login_time
                .map(|login_time| login_time.elapsed().as_secs() as u32)
                .unwrap_or(0),
        );

        let Some(packet) = (|| {
            let item = self
                .inventory_item_objects_like_cpp()
                .get(&request.item_guid)?;
            if !item.is_refundable() || item.refund_recipient() != player_guid {
                return None;
            }

            let played_time = item.played_time(i64::from(current_total_played_time));
            if played_time > 2 * 60 * 60 {
                return None;
            }

            let extended_cost = self
                .item_extended_cost_store()
                .and_then(|store| store.get(item.paid_extended_cost()))?;
            let contents =
                item_purchase_contents_from_extended_cost(extended_cost, item.paid_money());
            Some(SetItemPurchaseData {
                item_guid: request.item_guid,
                contents,
                flags: 0,
                purchase_time: current_total_played_time.saturating_sub(played_time),
            })
        })() else {
            debug!(
                "GetItemPurchaseData ignored for non-refundable or unknown item {:?}",
                request.item_guid
            );
            return;
        };

        self.send_packet(&packet);
    }
    pub async fn handle_request_forced_reactions(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_request_battlefield_status(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_request_rated_pvp_info(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_request_pvp_rewards(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_df_get_system_info(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_df_get_join_status(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_calendar_get_num_pending(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_gm_ticket_get_case_status(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_guild_bank_remaining_withdraw_money_query(
        &mut self,
        _pkt: wow_packet::WorldPacket,
    ) {
    }
    pub async fn handle_battle_pet_request_journal(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_arena_team_roster(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_request_raid_info(&mut self, _pkt: wow_packet::WorldPacket) {
        let locks = match (self.player_guid(), self.instance_lock_mgr.as_ref()) {
            (Some(player_guid), Some(instance_lock_mgr)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                instance_lock_mgr
                    .read()
                    .map(|mgr| {
                        let map_store = self.map_store().map(|store| store.as_ref());
                        let map_difficulty_store =
                            self.map_difficulty_store().map(|store| store.as_ref());
                        mgr.get_raid_info_locks_for_player_at(
                            player_guid,
                            now,
                            wow_instances::ResetSchedule::default(),
                            |map_id, difficulty_id| {
                                let map = map_store?.get(map_id)?;
                                let map_difficulty =
                                    map_difficulty_store?.get(map_id, difficulty_id)?;
                                Some(wow_instances::MapDb2Entries {
                                    map_id,
                                    difficulty_id,
                                    lock_id: u32::from(map_difficulty.lock_id),
                                    reset_interval: match map_difficulty.reset_interval {
                                        1 => wow_instances::MapDifficultyResetInterval::Daily,
                                        2 => wow_instances::MapDifficultyResetInterval::Weekly,
                                        _ => wow_instances::MapDifficultyResetInterval::Anytime,
                                    },
                                    is_flex_locking: map.is_flex_locking(),
                                    is_using_encounter_locks: map_difficulty
                                        .is_using_encounter_locks(),
                                })
                            },
                        )
                    })
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        };

        self.send_packet(&InstanceInfo {
            locks: locks
                .into_iter()
                .map(|lock| InstanceLockInfo {
                    instance_id: lock.instance_id,
                    map_id: lock.map_id,
                    difficulty_id: lock.difficulty_id,
                    time_remaining: lock.time_remaining,
                    completed_mask: lock.completed_mask,
                    locked: lock.locked,
                    extended: lock.extended,
                })
                .collect(),
        });
    }

    /// C++ `WorldSession::HandleResetInstancesOpcode`.
    pub async fn handle_reset_instances(&mut self, _pkt: wow_packet::WorldPacket) {
        let Some(player_guid) = self.player_guid() else {
            return;
        };

        if self
            .map_store()
            .and_then(|store| store.get(u32::from(self.player_map_id_like_cpp())))
            .is_some_and(|map| map.instance_type != 0)
        {
            return;
        }

        let reset_owner_guid = if let Some(group_guid) = self.group_guid {
            let Some(group_registry) = self.group_registry() else {
                return;
            };
            let Some(group) = group_registry.get(&group_guid) else {
                return;
            };
            if group.leader_guid != player_guid {
                return;
            }
            group.leader_guid
        } else {
            player_guid
        };

        let Some(instance_lock_mgr) = self.instance_lock_mgr.as_ref().cloned() else {
            return;
        };

        let mut tx = SqlTransaction::new();
        let reset_result = {
            let mut mgr = match instance_lock_mgr.write() {
                Ok(mgr) => mgr,
                Err(_) => return,
            };
            let entries_by_key = mgr
                .player_lock_map_difficulties(reset_owner_guid)
                .into_iter()
                .filter_map(|(map_id, difficulty_id)| {
                    let map = self.map_store()?.get(map_id)?;
                    let map_difficulty = self.map_difficulty_store()?.get(map_id, difficulty_id)?;
                    let entries = wow_instances::MapDb2Entries {
                        map_id,
                        difficulty_id,
                        lock_id: u32::from(map_difficulty.lock_id),
                        reset_interval: match map_difficulty.reset_interval {
                            1 => wow_instances::MapDifficultyResetInterval::Daily,
                            2 => wow_instances::MapDifficultyResetInterval::Weekly,
                            _ => wow_instances::MapDifficultyResetInterval::Anytime,
                        },
                        is_flex_locking: map.is_flex_locking(),
                        is_using_encounter_locks: map_difficulty.is_using_encounter_locks(),
                    };
                    Some((entries.key(), entries))
                })
                .collect::<std::collections::HashMap<_, _>>();

            mgr.reset_instance_locks_for_player_tx_at(
                &mut tx,
                reset_owner_guid,
                None,
                None,
                &entries_by_key,
                wow_instances::ResetSchedule::default(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0),
            )
        };

        if !tx.is_empty() {
            if let Some(char_db) = self.char_db()
                && let Err(err) = char_db.commit_transaction(tx).await
            {
                warn!(
                    account = self.account_id,
                    player_guid = ?reset_owner_guid,
                    error = ?err,
                    "failed to commit CMSG_RESET_INSTANCES lock reset transaction"
                );
                return;
            }
        }

        for lock in reset_result.reset {
            self.send_packet(&InstanceReset {
                map_id: lock.map_id,
            });
        }

        for lock in reset_result.failed_to_reset {
            self.send_packet(&InstanceResetFailed {
                map_id: lock.map_id,
                reset_failed_reason: 0,
            });
        }
    }

    /// C++ `WorldSession::HandleInstanceLockResponse`.
    pub async fn handle_instance_lock_response(&mut self, mut pkt: wow_packet::WorldPacket) {
        let Ok(response) = InstanceLockResponse::read(&mut pkt) else {
            return;
        };

        let Some(pending_bind) = self.pending_bind.take() else {
            info!(
                account = self.account_id,
                player_guid = ?self.player_guid(),
                "InstanceLockResponse without pending bind"
            );
            return;
        };

        if response.accept_lock {
            self.represented_confirmed_pending_binds
                .push(pending_bind.instance_id);
        } else {
            self.represented_repop_at_graveyard_count =
                self.represented_repop_at_graveyard_count.saturating_add(1);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn send_pending_raid_lock_like_cpp(
        &mut self,
        instance_id: u32,
        completed_mask: u32,
        extending: bool,
        warning_only: bool,
    ) {
        self.send_packet(&PendingRaidLock {
            time_until_lock: 60_000,
            completed_mask,
            extending,
            warning_only,
        });

        if !warning_only {
            self.pending_bind = Some(crate::session::RepresentedPendingBind {
                instance_id,
                time_until_lock_ms: 60_000,
            });
        }
    }

    pub async fn handle_request_conquest_formula_constants(
        &mut self,
        _pkt: wow_packet::WorldPacket,
    ) {
    }
    pub async fn handle_request_lfg_list_blacklist(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_lfg_list_get_status(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_get_account_character_list(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_cancel_trade(&mut self, _pkt: wow_packet::WorldPacket) {
        // C++ calls Player::TradeCancel(true) only when a player is present.
        // Full trade state is not ported yet; no active trade means no response.
    }
    pub async fn handle_report_client_variables(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_report_enabled_addons(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_report_keybinding_execution_counts(
        &mut self,
        _pkt: wow_packet::WorldPacket,
    ) {
    }
    pub async fn handle_request_countdown_timer(&mut self, _pkt: wow_packet::WorldPacket) {}
    pub async fn handle_calendar_get(&mut self, _pkt: wow_packet::WorldPacket) {}

    // ── Auction house list stubs ──────────────────────────────────────────────

    /// CMSG_AUCTION_LIST_BIDDER_ITEMS — list items bid on.
    /// Returns empty list until AH system is implemented.
    pub async fn handle_auction_list_bidder_items(&mut self, _pkt: wow_packet::WorldPacket) {
        use wow_packet::packets::misc::AuctionListBidderItemsResult;
        self.send_packet(&AuctionListBidderItemsResult);
    }

    /// CMSG_AUCTION_LIST_OWNER_ITEMS — list items the player put up for auction.
    /// Returns empty list until AH system is implemented.
    pub async fn handle_auction_list_owner_items(&mut self, _pkt: wow_packet::WorldPacket) {
        use wow_packet::packets::misc::AuctionListOwnerItemsResult;
        self.send_packet(&AuctionListOwnerItemsResult);
    }

    /// CMSG_AUCTION_LIST_PENDING_SALES — list pending sales / completed auctions.
    /// Returns empty list until AH system is implemented.
    pub async fn handle_auction_list_pending_sales(&mut self, _pkt: wow_packet::WorldPacket) {
        use wow_packet::packets::misc::AuctionListPendingSalesResult;
        self.send_packet(&AuctionListPendingSalesResult);
    }

    /// CMSG_COMMERCE_TOKEN_GET_LOG — WoW Token transaction log.
    /// Not implemented; silently ignored.
    pub async fn handle_commerce_token_get_log(&mut self, _pkt: wow_packet::WorldPacket) {}

    // ── Game object interaction ───────────────────────────────────────────────

    /// CMSG_GAME_OBJ_USE — player interacts with a world game object.
    /// C++ ref: `GameObject::Use` dispatches by `GameObjectTemplate::type`.
    pub async fn handle_game_obj_use(&mut self, mut pkt: wow_packet::WorldPacket) {
        let gameobject_guid = match pkt.read_packed_guid() {
            Ok(guid) => guid,
            Err(e) => {
                warn!("GameObjUse: failed to read gameobject guid: {e}");
                return;
            }
        };

        if !gameobject_guid.is_game_object() || !self.visible_gameobjects.contains(&gameobject_guid)
        {
            return;
        }

        let Some(world_db) = self.world_db().cloned() else {
            return;
        };
        let mut stmt = world_db.prepare(WorldStatements::SEL_GAMEOBJECT_TEMPLATE_BY_ENTRY);
        stmt.set_u32(0, gameobject_guid.entry());
        let result = match world_db.query(&stmt).await {
            Ok(result) => result,
            Err(e) => {
                warn!(
                    entry = gameobject_guid.entry(),
                    "GameObjUse: failed to query gameobject template: {e}"
                );
                return;
            }
        };
        if result.is_empty() {
            return;
        }

        let go_type = result.try_read::<u32>(1).unwrap_or(0);
        let mut data = [0_u32; MAX_GAMEOBJECT_DATA];
        for (index, value) in data.iter_mut().enumerate() {
            *value = result
                .try_read::<i32>(8 + index)
                .and_then(|raw| u32::try_from(raw).ok())
                .unwrap_or(0);
        }

        let template = GameObjectTemplateData::new(go_type, data);
        if let Some(source) = template.chest_loot_source_like_cpp() {
            if source.is_empty() {
                return;
            }

            self.open_represented_gameobject_chest_like_cpp(gameobject_guid, source)
                .await;
            return;
        }

        let loot_id = template.get_loot_id_like_cpp();
        match go_type {
            GAMEOBJECT_TYPE_FISHING_HOLE if loot_id != 0 => {
                self.open_represented_fishing_hole_like_cpp(gameobject_guid, loot_id)
                    .await;
            }
            GAMEOBJECT_TYPE_GATHERING_NODE => {
                if let Some(source) = template.gathering_node_use_source_like_cpp() {
                    self.open_represented_gathering_node_like_cpp(gameobject_guid, source)
                        .await;
                }
            }
            _ => {
                debug!(
                    account = self.account_id,
                    guid = ?gameobject_guid,
                    go_type,
                    "GameObjUse: represented gameobject use type is not ported yet"
                );
            }
        }
    }

    /// CMSG_GAME_OBJ_REPORT_USE — client reports a game object use event.
    /// C# ref: SpellHandler.HandleGameobjectReportUse → UpdateCriteria
    pub async fn handle_game_obj_report_use(&mut self, _pkt: wow_packet::WorldPacket) {}

    /// CMSG_CLOSE_INTERACTION — player closed an NPC interaction window.
    /// C# ref: MiscHandler.HandleCloseInteraction → resets interaction data.
    pub async fn handle_close_interaction(&mut self, _pkt: wow_packet::WorldPacket) {
        // TODO: reset PlayerTalkClass interaction data and stable master.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use wow_constants::ServerOpcodes;
    use wow_core::{ObjectGuid, Position};
    use wow_data::{MapDifficultyEntry, MapDifficultyStore, MapEntry, MapStore};
    use wow_packet::WorldPacket;

    #[test]
    fn item_purchase_contents_skip_season_earned_currency_like_cpp() {
        let extended_cost = wow_data::item_extended_cost::ItemExtendedCostEntry {
            id: 1,
            required_arena_rating: 0,
            arena_bracket: 0,
            flags: ItemExtendedCostFlags::REQUIRE_SEASON_EARNED_2,
            min_faction_id: 0,
            min_reputation: 0,
            required_achievement: 0,
            item_id: [100, 0, 0, 0, 0],
            item_count: [2, 0, 0, 0, 0],
            currency_id: [390, 391, 0, 0, 0],
            currency_count: [5, 7, 0, 0, 0],
        };

        let contents = item_purchase_contents_from_extended_cost(&extended_cost, 123);
        assert_eq!(contents.money, 123);
        assert_eq!(contents.items[0].item_id, 100);
        assert_eq!(contents.items[0].item_count, 2);
        assert_eq!(contents.currencies[0].currency_id, 390);
        assert_eq!(contents.currencies[0].currency_count, 5);
        assert_eq!(contents.currencies[1].currency_id, 0);
        assert_eq!(contents.currencies[1].currency_count, 0);
    }

    fn make_session() -> (crate::session::WorldSession, flume::Receiver<Vec<u8>>) {
        let (_pkt_tx, pkt_rx) = flume::bounded(8);
        let (send_tx, send_rx) = flume::bounded(8);
        (
            crate::session::WorldSession::new(
                1,
                "TestAccount".into(),
                0,
                2,
                9,
                54261,
                vec![0; 40],
                "enUS".into(),
                pkt_rx,
                send_tx,
            ),
            send_rx,
        )
    }

    #[tokio::test]
    async fn reset_instances_handler_resets_player_lock_and_sends_cpp_success_packet() {
        let (mut session, send_rx) = make_session();
        let player_guid = ObjectGuid::create_player(1, 42);
        let entries = wow_instances::MapDb2Entries {
            map_id: 631,
            difficulty_id: 4,
            lock_id: 10,
            reset_interval: wow_instances::MapDifficultyResetInterval::Weekly,
            is_flex_locking: true,
            is_using_encounter_locks: false,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut mgr = wow_instances::InstanceLockMgr::default();
        mgr.update_instance_lock_for_player_at(
            player_guid,
            &entries,
            wow_instances::InstanceLockUpdateEvent {
                instance_id: 100,
                new_data: String::new(),
                instance_completed_encounters_mask: 0,
                completed_encounter_bit: None,
                entrance_world_safe_loc_id: None,
            },
            wow_instances::ResetSchedule::default(),
            now,
        );

        session.set_player_guid(Some(player_guid));
        session.set_player_map_position_like_cpp(0, Position::ZERO);
        session.set_map_store(Arc::new(MapStore::from_entries([
            MapEntry {
                id: 0,
                instance_type: 0,
                flags1: 0,
            },
            MapEntry {
                id: 631,
                instance_type: 2,
                flags1: wow_data::map::MAP_FLAG_FLEXIBLE_RAID_LOCKING,
            },
        ])));
        session.set_map_difficulty_store(Arc::new(MapDifficultyStore::from_entries([
            MapDifficultyEntry {
                id: 1,
                map_id: 631,
                difficulty_id: 4,
                lock_id: 10,
                reset_interval: 2,
                flags: 0,
            },
        ])));
        let mgr = Arc::new(std::sync::RwLock::new(mgr));
        session.set_instance_lock_mgr(Arc::clone(&mgr));

        session
            .handle_reset_instances(WorldPacket::from_bytes(&[]))
            .await;

        let sent = send_rx.try_recv().unwrap();
        assert_eq!(
            u16::from_le_bytes([sent[0], sent[1]]),
            ServerOpcodes::InstanceReset as u16
        );
        assert_eq!(&sent[2..], &[0x77, 0x02, 0x00, 0x00]);
        assert!(
            mgr.read()
                .unwrap()
                .find_active_instance_lock_at(player_guid, &entries, now)
                .is_none()
        );
    }

    #[test]
    fn send_pending_raid_lock_sets_pending_bind_like_cpp_for_stop_prompt() {
        let (mut session, send_rx) = make_session();

        session.send_pending_raid_lock_like_cpp(77, 0xA5, true, false);

        let sent = send_rx.try_recv().unwrap();
        assert_eq!(
            u16::from_le_bytes([sent[0], sent[1]]),
            ServerOpcodes::PendingRaidLock as u16
        );
        assert_eq!(
            session.pending_bind,
            Some(crate::session::RepresentedPendingBind {
                instance_id: 77,
                time_until_lock_ms: 60_000,
            })
        );
    }

    #[test]
    fn send_pending_raid_lock_warning_only_does_not_set_pending_bind_like_cpp() {
        let (mut session, _send_rx) = make_session();

        session.send_pending_raid_lock_like_cpp(77, 0xA5, false, true);

        assert!(session.pending_bind.is_none());
    }

    #[tokio::test]
    async fn instance_lock_response_accept_confirms_and_clears_pending_bind_like_cpp() {
        let (mut session, _send_rx) = make_session();
        session.pending_bind = Some(crate::session::RepresentedPendingBind {
            instance_id: 77,
            time_until_lock_ms: 60_000,
        });

        session
            .handle_instance_lock_response(WorldPacket::from_bytes(&[0x80]))
            .await;

        assert!(session.pending_bind.is_none());
        assert_eq!(session.represented_confirmed_pending_binds, vec![77]);
        assert_eq!(session.represented_repop_at_graveyard_count, 0);
    }

    #[tokio::test]
    async fn instance_lock_response_decline_repops_and_clears_pending_bind_like_cpp() {
        let (mut session, _send_rx) = make_session();
        session.pending_bind = Some(crate::session::RepresentedPendingBind {
            instance_id: 77,
            time_until_lock_ms: 60_000,
        });

        session
            .handle_instance_lock_response(WorldPacket::from_bytes(&[0x00]))
            .await;

        assert!(session.pending_bind.is_none());
        assert!(session.represented_confirmed_pending_binds.is_empty());
        assert_eq!(session.represented_repop_at_graveyard_count, 1);
    }
}
