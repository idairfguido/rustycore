// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Quest system handlers.
//!
//! Implements:
//!   CMSG_QUEST_GIVER_STATUS_QUERY  → SMSG_QUEST_GIVER_STATUS
//!   CMSG_QUEST_GIVER_HELLO         → SMSG_QUEST_GIVER_QUEST_LIST_MESSAGE
//!   CMSG_QUEST_GIVER_QUERY_QUEST   → SMSG_QUEST_GIVER_QUEST_DETAILS
//!   CMSG_QUEST_GIVER_ACCEPT_QUEST  → save to DB + SMSG_QUEST_GIVER_QUEST_COMPLETE
//!   CMSG_QUEST_LOG_REMOVE_QUEST    → remove from DB
//!   CMSG_QUERY_QUEST_INFO          → SMSG_QUERY_QUEST_INFO_RESPONSE
//!
//! C# ref: Game/Handlers/QuestHandler.cs

use std::sync::Arc;
use tracing::{debug, info, warn};
use wow_constants::unit::NPCFlags1;
use wow_constants::{ClientOpcodes, InventoryResult};
use wow_core::ObjectGuid;
use wow_data::DISABLE_TYPE_QUEST;
use wow_handler::{PacketHandlerEntry, PacketProcessing, SessionStatus};
use wow_network::SessionCommand;
use wow_network::player_registry::{
    SendRepeatableTurnInRequestItemsLikeCppCommand, SetQuestSharingInfoAndSendDetailsCommand,
};
use wow_packet::packets::query::{
    QueryQuestCompletionNpcs, QuestCompletionNpc, QuestCompletionNpcResponse,
};
use wow_packet::packets::quest::{
    PushQuestToParty, QueryQuestInfoResponse, QuestConfirmAccept, QuestGiverOfferReward,
    QuestGiverQuestComplete, QuestGiverRequestItems, QuestGiverStatus, QuestObjectiveInfo,
    QuestPushResult, QuestPushResultResponse, QuestRewardsBlock, QuestUpdateComplete,
    WorldQuestUpdateResponse, quest_giver_status, quest_push_reason,
};
use wow_packet::{ClientPacket, ServerPacket};

use crate::conditions::{
    QUEST_STATUS_COMPLETE_LIKE_CPP, QUEST_STATUS_FAILED_LIKE_CPP, QUEST_STATUS_INCOMPLETE_LIKE_CPP,
    QUEST_STATUS_NONE_LIKE_CPP,
};
use crate::session::{
    RepresentedPushQuestToPartyOutcomeLikeCpp, RepresentedPushQuestToPartyOutcomeReasonLikeCpp,
    RepresentedQuestConfirmAcceptLikeCpp, RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp,
    RepresentedQuestPushResultResponseLikeCpp, SeasonalQuestStatusDbRowLikeCpp, WorldSession,
};

pub(crate) const QUEST_FLAGS_AUTO_COMPLETE_LIKE_CPP: u32 = 0x0001_0000;
pub(crate) const QUEST_FLAGS_SHARABLE_LIKE_CPP: u32 = 0x0000_0008;
pub(crate) const QUEST_PUSH_REASON_INVALID_LIKE_CPP: u8 = 1;
pub(crate) const QUEST_PUSH_REASON_INVALID_TO_RECIPIENT_LIKE_CPP: u8 = 2;
pub(crate) const QUEST_PUSH_REASON_BUSY_LIKE_CPP: u8 = 5;
pub(crate) const QUEST_PUSH_REASON_DEAD_LIKE_CPP: u8 = 6;
pub(crate) const QUEST_PUSH_REASON_DEAD_TO_RECIPIENT_LIKE_CPP: u8 = 7;
pub(crate) const QUEST_PUSH_REASON_LOG_FULL_LIKE_CPP: u8 = 8;
pub(crate) const QUEST_PUSH_REASON_LOG_FULL_TO_RECIPIENT_LIKE_CPP: u8 = 9;
pub(crate) const QUEST_PUSH_REASON_ON_QUEST_LIKE_CPP: u8 = 10;
pub(crate) const QUEST_PUSH_REASON_ON_QUEST_TO_RECIPIENT_LIKE_CPP: u8 = 11;
pub(crate) const QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP: u8 = 12;
pub(crate) const QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP: u8 = 13;
pub(crate) const QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP: u8 = 20;
pub(crate) const QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP: u8 = 21;
pub(crate) const QUEST_PUSH_REASON_LOW_LEVEL_LIKE_CPP: u8 = 22;
pub(crate) const QUEST_PUSH_REASON_LOW_LEVEL_TO_RECIPIENT_LIKE_CPP: u8 = 23;
pub(crate) const QUEST_PUSH_REASON_HIGH_LEVEL_LIKE_CPP: u8 = 24;
pub(crate) const QUEST_PUSH_REASON_HIGH_LEVEL_TO_RECIPIENT_LIKE_CPP: u8 = 25;
pub(crate) const QUEST_PUSH_REASON_CLASS_LIKE_CPP: u8 = 26;
pub(crate) const QUEST_PUSH_REASON_CLASS_TO_RECIPIENT_LIKE_CPP: u8 = 27;
pub(crate) const QUEST_PUSH_REASON_RACE_LIKE_CPP: u8 = 28;
pub(crate) const QUEST_PUSH_REASON_RACE_TO_RECIPIENT_LIKE_CPP: u8 = 29;
pub(crate) const QUEST_PUSH_REASON_LOW_FACTION_LIKE_CPP: u8 = 30;
pub(crate) const QUEST_PUSH_REASON_LOW_FACTION_TO_RECIPIENT_LIKE_CPP: u8 = 31;
pub(crate) const QUEST_PUSH_REASON_EXPANSION_LIKE_CPP: u8 = 32;
pub(crate) const QUEST_PUSH_REASON_EXPANSION_TO_RECIPIENT_LIKE_CPP: u8 = 33;
pub(crate) const QUEST_PUSH_REASON_SUCCESS_LIKE_CPP: u8 = 0;

fn player_race_or_class_mask_like_cpp(id: u8) -> u32 {
    if id == 0 {
        return 0;
    }

    1_u32
        .checked_shl(u32::from(id.saturating_sub(1)))
        .unwrap_or(0)
}

fn represented_satisfy_quest_dependent_previous_quests_failed_like_cpp(
    quest_store: &wow_data::quest::QuestStore,
    quest: &wow_data::quest::QuestTemplate,
    receiver_rewarded_quests: &std::collections::HashSet<u32>,
) -> bool {
    if quest.dependent_previous_quests.is_empty() {
        return false;
    }

    for &prev_id in &quest.dependent_previous_quests {
        let Some(previous_quest) = quest_store.get(prev_id) else {
            // C++ ASSERTs because ObjectMgr validates this at startup. Rust fails closed
            // as the prerequisite branch rather than panicking in the sender loop.
            return true;
        };

        if receiver_rewarded_quests.contains(&prev_id) {
            if previous_quest.exclusive_group >= 0 {
                return false;
            }

            for exclusive_quest_id in quest_store
                .quests
                .values()
                .filter(|candidate| candidate.exclusive_group == previous_quest.exclusive_group)
                .map(|candidate| candidate.id)
            {
                if exclusive_quest_id != prev_id
                    && !receiver_rewarded_quests.contains(&exclusive_quest_id)
                {
                    return true;
                }
            }

            return false;
        }
    }

    true
}

fn represented_satisfy_quest_dependent_breadcrumb_quests_failed_like_cpp(
    quest: &wow_data::quest::QuestTemplate,
    receiver_active_quest_statuses: &std::collections::HashMap<u32, u8>,
) -> bool {
    quest
        .dependent_breadcrumb_quests
        .iter()
        .any(|breadcrumb_quest_id| {
            matches!(
                receiver_active_quest_statuses
                    .get(breadcrumb_quest_id)
                    .copied(),
                Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP)
                    | Some(QUEST_STATUS_COMPLETE_LIKE_CPP)
                    | Some(QUEST_STATUS_FAILED_LIKE_CPP)
            )
        })
}

fn represented_can_take_quest_after_expansion_like_cpp(
    quest_store: &wow_data::quest::QuestStore,
    quest: &wow_data::quest::QuestTemplate,
    receiver: &wow_network::PlayerBroadcastInfo,
) -> bool {
    // C++ anchor: `Player::CanTakeQuest`, Player.cpp:14093-14102, after the
    // push handler has already emitted dedicated messages for class/race/level,
    // reputation, prerequisite, daily/DF, and expansion gates. This bounded
    // helper keeps the remaining represented `false` cases that TrinityCore
    // groups under `QuestPushReason::Invalid` at the final CanTakeQuest gate.
    // Explicitly out of scope for this represented-partial slice: DisableMgr,
    // skill, timed, weekly/monthly, ConditionMgr, and full seasonal runtime.
    let receiver_status = receiver
        .active_quest_statuses
        .get(&quest.id)
        .copied()
        .unwrap_or(QUEST_STATUS_NONE_LIKE_CPP);
    if receiver.rewarded_quests.contains(&quest.id) || receiver_status != QUEST_STATUS_NONE_LIKE_CPP
    {
        return false;
    }

    if quest.exclusive_group <= 0 {
        return true;
    }

    for peer_quest in quest_store
        .quests
        .values()
        .filter(|candidate| candidate.exclusive_group == quest.exclusive_group)
    {
        if peer_quest.id == quest.id {
            continue;
        }

        if peer_quest.is_df_quest_like_cpp() && receiver.df_quests.contains(&peer_quest.id) {
            return false;
        }
        if peer_quest.is_daily_like_cpp()
            && receiver.daily_quests_completed.contains(&peer_quest.id)
        {
            return false;
        }

        if receiver
            .active_quest_statuses
            .get(&peer_quest.id)
            .copied()
            .unwrap_or(QUEST_STATUS_NONE_LIKE_CPP)
            != QUEST_STATUS_NONE_LIKE_CPP
        {
            return false;
        }

        if !(quest.is_repeatable() && peer_quest.is_repeatable())
            && receiver.rewarded_quests.contains(&peer_quest.id)
        {
            return false;
        }
    }

    true
}

fn represented_quest_completion_npc_response_like_cpp(
    quest_store: &wow_data::quest::QuestStore,
    raw_quest_ids: &[i32],
) -> Vec<QuestCompletionNpc> {
    raw_quest_ids
        .iter()
        .filter_map(|&raw_quest_id| {
            let quest_id = u32::try_from(raw_quest_id).ok()?;
            if quest_store.get(quest_id).is_none() {
                return None;
            }

            let mut npcs = Vec::new();
            for creature_entry in quest_store.creature_ender_entries_for_quest_like_cpp(quest_id) {
                let Ok(entry) = i32::try_from(creature_entry) else {
                    debug!(
                        quest_id,
                        creature_entry,
                        "QueryQuestCompletionNPCs: creature entry exceeds signed i32 response field"
                    );
                    continue;
                };
                npcs.push(entry);
            }

            for go_entry in quest_store.gameobject_ender_entries_for_quest_like_cpp(quest_id) {
                npcs.push((go_entry | 0x8000_0000) as i32);
            }

            Some(QuestCompletionNpc {
                quest_id: raw_quest_id,
                npcs,
            })
        })
        .collect()
}

// ── Handler registrations ────────────────────────────────────────────────────

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverStatusQuery,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_status_query",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverHello,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_hello",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverQueryQuest,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_query_quest",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverAcceptQuest,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_accept_quest",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestLogRemoveQuest,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_log_remove_quest",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueryQuestInfo,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_query_quest_info",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QueryQuestCompletionNpcs,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_query_quest_completion_npcs",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverRequestReward,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_request_reward",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverCompleteQuest,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_complete_quest",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverChooseReward,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_choose_reward",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestGiverCloseQuest,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_quest_giver_close_quest",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::RequestWorldQuestUpdate,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_request_world_quest_update",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestConfirmAccept,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_quest_confirm_accept",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::QuestPushResult,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_quest_push_result",
    }
}

inventory::submit! {
    PacketHandlerEntry {
        opcode: ClientOpcodes::PushQuestToParty,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::ThreadUnsafe,
        handler_name: "handle_push_quest_to_party",
    }
}

// ── Handler implementations ──────────────────────────────────────────────────

/// TrinityCore `MAX_QUEST_LOG_SIZE`; explicit quest-log slots are 0..24.
pub(crate) const MAX_QUEST_LOG_SIZE_LIKE_CPP: u8 = 25;

impl WorldSession {
    /// CMSG_QUEST_GIVER_STATUS_QUERY — returns the quest status icon for an NPC.
    /// C# ref: QuestHandler.HandleQuestGiverStatusQuery
    pub async fn handle_quest_giver_status_query(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverStatusQuery: failed to read GUID");
                return;
            }
        };

        let Some(source) = self.represented_quest_giver_status_query_source_like_cpp(guid) else {
            debug!(
                account = self.account_id,
                ?guid,
                "QuestGiverStatusQuery: represented ObjectAccessor mask UNIT|GAMEOBJECT miss"
            );
            return;
        };
        let status = self.get_represented_quest_giver_status_like_cpp(source);

        debug!(
            account = self.account_id,
            ?guid,
            source_entry = source.entry(),
            source_kind = source.kind_name(),
            status = status,
            "QuestGiverStatus represented source resolved"
        );

        self.send_packet(&QuestGiverStatus { guid, status });
    }

    /// CMSG_QUEST_GIVER_HELLO — player right-clicks a quest NPC.
    /// Opens the represented quest list dialog for an interactable questgiver Creature.
    /// C++ refs:
    /// - `WorldSession::HandleQuestgiverHelloOpcode`, `QuestHandler.cpp:76-103`.
    /// - `Player::PrepareQuestMenu`, `Player.cpp:13947-14004`.
    /// Remaining represented gaps: fake-death aura removal, movement pause/home position,
    /// `AI()->OnGossipHello`, `PrepareGossipMenu`, `SendPreparedGossip` / auto-open / PlayerTalkClass.
    pub async fn handle_quest_giver_hello(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverHello: failed to read GUID");
                return;
            }
        };

        let Some(access) =
            self.represented_npc_can_interact_with_like_cpp(guid, NPCFlags1::QUEST_GIVER.bits())
        else {
            debug!(
                account = self.account_id,
                ?guid,
                "QuestGiverHello: NPC not found or not interactable as questgiver"
            );
            return;
        };

        if self.use_represented_creature_questgiver_like_cpp(guid, access.entry) {
            debug!(
                account = self.account_id,
                creature_entry = access.entry,
                "QuestGiverHello represented Creature questgiver seam consumed"
            );
        }
    }

    /// CMSG_QUEST_GIVER_QUERY_QUEST — player clicks a quest name in the list.
    /// Shows full quest details (objectives, rewards) before accepting.
    /// C# ref: QuestHandler.HandleQuestGiverQueryQuest
    pub async fn handle_quest_giver_query_quest(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverQueryQuest: failed to read GUID");
                return;
            }
        };
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);
        let _resend_offer: bool = pkt.read_uint8().unwrap_or(0) != 0;

        let _ = self.send_represented_quest_giver_query_quest_like_cpp(guid, quest_id);
    }

    /// CMSG_QUEST_GIVER_ACCEPT_QUEST — player clicks "Accept" in the quest details dialog.
    /// Saves quest to characters DB and confirms to the client.
    /// C# ref: QuestHandler.HandleQuestGiverAcceptQuest
    pub async fn handle_quest_giver_accept_quest(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverAcceptQuest: failed to read GUID");
                return;
            }
        };
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);
        let _start_cheat: bool = pkt.read_uint8().unwrap_or(0) != 0;

        // Validate represented C++ source/relation before any quest-log mutation or DB save.
        // C++ HandleQuestgiverAcceptQuestOpcode closes gossip and clears sharing info on
        // failure; this represented slice intentionally models that as no packet/no mutation.
        let quest_store = match &self.quest_store {
            Some(s) => Arc::clone(s),
            None => return,
        };
        if !self.represented_quest_giver_accept_source_allows_quest_like_cpp(
            guid,
            quest_id,
            &quest_store,
        ) {
            debug!(
                account = self.account_id,
                ?guid,
                quest_id,
                "AcceptQuest: represented source/relation guard rejected quest"
            );
            return;
        }
        if quest_store.get(quest_id).is_none() {
            warn!(
                account = self.account_id,
                quest_id, "AcceptQuest: unknown quest"
            );
            return;
        }

        // Full eligibility check: SatisfyQuestStatus + PrevQuestId + race/class/level
        // C# ref: Player.CanTakeQuest(quest, true)
        if let Some(quest) = quest_store.get(quest_id) {
            if !self.can_take_quest(quest) {
                warn!(
                    account = self.account_id,
                    quest_id,
                    race = self.player_race_like_cpp(),
                    class = self.player_class_like_cpp(),
                    level = self.player_level_like_cpp(),
                    "AcceptQuest: player does not meet requirements (CanTakeQuest failed)"
                );
                return;
            }
        }

        // C++ Player::AddQuest uses FindQuestSlot(0) over explicit QuestLog slots.
        let Some(slot) = self.first_free_quest_slot_like_cpp() else {
            warn!(account = self.account_id, "Quest log full");
            return;
        };

        // Build objective counts (one slot per objective)
        let obj_count = quest_store.get(quest_id).map_or(0, |q| q.objectives.len());

        // Add to local state
        self.player_quests.insert(
            quest_id,
            PlayerQuestStatus {
                quest_id,
                status: 1, // Incomplete
                explored: false,
                objective_counts: vec![0; obj_count],
                slot,
            },
        );

        // Save to DB
        self.save_quest_to_db(quest_id, 1).await;
        self.sync_player_registry_state_like_cpp();

        info!(account = self.account_id, quest_id, "Quest accepted");

        // Notify client — quest added popup
        self.send_packet(&QuestGiverQuestComplete {
            quest_id,
            xp: 0,
            money: 0,
            skill_points: 0,
            use_quest_reward_currency: false,
        });
    }

    /// CMSG_QUEST_GIVER_CLOSE_QUEST — acknowledged client close for auto-accept quest flow.
    /// C++ ref: `WorldSession::HandleQuestgiverCloseQuest`, `QuestHandler.cpp:591-601`.
    /// Represented seam only: records local `ScriptMgr::OnQuestAcknowledgeAutoAccept` evidence.
    pub async fn handle_quest_giver_close_quest(&mut self, mut pkt: wow_packet::WorldPacket) {
        let quest_id = match pkt.read_uint32() {
            Ok(quest_id) => quest_id,
            Err(error) => {
                warn!(
                    account = self.account_id,
                    ?error,
                    "QuestGiverCloseQuest: failed to read QuestID"
                );
                return;
            }
        };

        let _ = self.acknowledge_auto_accept_quest_like_cpp(quest_id);
    }

    pub(crate) fn acknowledge_auto_accept_quest_like_cpp(&mut self, quest_id: u32) -> bool {
        // C++ order: FindQuestSlot(QuestID), then GetQuestTemplate(QuestID), then
        // ScriptMgr::OnQuestAcknowledgeAutoAccept(player, quest).
        if self.find_quest_slot_like_cpp(quest_id).is_none() {
            debug!(
                account = self.account_id,
                quest_id, "QuestGiverCloseQuest: represented active quest log miss"
            );
            return false;
        }

        let Some(quest_store) = &self.quest_store else {
            debug!(
                account = self.account_id,
                quest_id, "QuestGiverCloseQuest: missing represented quest store"
            );
            return false;
        };

        if quest_store.get(quest_id).is_none() {
            debug!(
                account = self.account_id,
                quest_id, "QuestGiverCloseQuest: represented quest template miss"
            );
            return false;
        }

        self.represented_auto_accept_acknowledged_quests_like_cpp
            .push(quest_id);
        true
    }

    /// CMSG_REQUEST_WORLD_QUEST_UPDATE — current Trinity 3.4.3 handler sends an empty response.
    /// C++ refs: `WorldSession::HandleRequestWorldQuestUpdate`, `QuestHandler.cpp:780-788`;
    /// `RequestWorldQuestUpdate::Read`, `QuestPackets.h:655-661` (`Read() { }`, no payload consumption).
    pub async fn handle_request_world_quest_update(&mut self, _pkt: wow_packet::WorldPacket) {
        self.send_packet(&WorldQuestUpdateResponse {
            updates: Vec::new(),
        });
    }

    async fn add_quest_confirm_accept_local_state_like_cpp(
        &mut self,
        quest: &wow_data::quest::QuestTemplate,
    ) -> bool {
        let Some(slot) = self.first_free_quest_slot_like_cpp() else {
            return false;
        };

        self.player_quests.insert(
            quest.id,
            PlayerQuestStatus {
                quest_id: quest.id,
                status: QUEST_STATUS_INCOMPLETE_LIKE_CPP,
                explored: false,
                objective_counts: vec![0; quest.objectives.len()],
                slot,
            },
        );
        self.save_quest_to_db(quest.id, QUEST_STATUS_INCOMPLETE_LIKE_CPP)
            .await;
        self.sync_player_registry_state_like_cpp();
        true
    }

    /// CMSG_QUEST_CONFIRM_ACCEPT — confirm accepting a shared quest.
    ///
    /// C++ anchor: `WorldSession::HandleQuestConfirmAccept`, `QuestHandler.cpp:499-531`.
    /// Represented-partial: validates against session-local pending sharing state, clears before
    /// quest-template lookup like C++, then records safe represented post-template gates.
    /// No-source-item quests and source-item no-grant branches consume only local quest-log insertion
    /// + Character DB status save + PlayerRegistry snapshot sync from `Player::AddQuest`. Real
    /// `StoreNewItem`/`SendNewItem`, criteria/completion, timed/PvP, scripts, and `SendQuestUpdate`
    /// packet fanout remain explicit no-mutation boundaries.
    pub async fn handle_quest_confirm_accept(&mut self, mut pkt: wow_packet::WorldPacket) {
        let packet = match QuestConfirmAccept::read(&mut pkt) {
            Ok(packet) => packet,
            Err(error) => {
                warn!(
                    account = self.account_id,
                    ?error,
                    "QuestConfirmAccept: failed to read signed QuestID"
                );
                return;
            }
        };

        let parsed_quest_id = packet.quest_id as u32;
        let Some(pending) = self.represented_pending_quest_sharing_like_cpp() else {
            debug!(
                account = self.account_id,
                raw_quest_id = packet.quest_id,
                parsed_quest_id,
                "QuestConfirmAccept: no represented pending shared quest"
            );
            return;
        };

        if pending.quest_id != parsed_quest_id {
            debug!(
                account = self.account_id,
                pending_quest_id = pending.quest_id,
                raw_quest_id = packet.quest_id,
                parsed_quest_id,
                "QuestConfirmAccept: represented pending quest id mismatch; pending state preserved"
            );
            return;
        }

        self.clear_represented_pending_quest_sharing_like_cpp();

        let Some(quest_store) = &self.quest_store else {
            debug!(
                account = self.account_id,
                parsed_quest_id,
                "QuestConfirmAccept: pending cleared before missing quest store like C++ order"
            );
            return;
        };

        let Some(quest) = quest_store.get(parsed_quest_id).cloned() else {
            debug!(
                account = self.account_id,
                parsed_quest_id,
                "QuestConfirmAccept: pending cleared before missing quest template like C++ order"
            );
            return;
        };

        let receiver_guid = self.player_guid();
        let record = |session: &mut WorldSession,
                      reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp,
                      can_add_source_item_unrepresented: bool,
                      can_add_source_item_result: Option<InventoryResult>,
                      add_quest_runtime_unrepresented: bool,
                      source_spell_unrepresented: bool,
                      represented_source_spell_id: Option<u32>,
                      represented_source_spell_self_casts: u8| {
            session.record_represented_quest_confirm_accept_like_cpp(
                RepresentedQuestConfirmAcceptLikeCpp {
                    receiver_guid,
                    sender_guid_before_clear: pending.sender_guid,
                    quest_id: parsed_quest_id,
                    raw_quest_id: packet.quest_id,
                    reason,
                    object_accessor_unrepresented: true,
                    party_runtime_unrepresented: true,
                    can_add_source_item_unrepresented,
                    can_add_source_item_result,
                    add_quest_runtime_unrepresented,
                    source_spell_unrepresented,
                    represented_source_spell_id,
                    represented_source_spell_self_casts,
                },
            );
        };

        let Some(player_registry) = self.player_registry().map(Arc::clone) else {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerMissing,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        };

        let Some(sender_snapshot) = player_registry
            .get(&pending.sender_guid)
            .map(|entry| entry.clone())
        else {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerMissing,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        };

        let Some(receiver_guid) = receiver_guid else {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::NotInSameRaid,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        };

        let Some(group_registry) = self.group_registry().map(Arc::clone) else {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::NotInSameRaid,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        };

        let same_represented_group = group_registry.iter().any(|entry| {
            let members = &entry.value().members;
            members.contains(&receiver_guid) && members.contains(&pending.sender_guid)
        });
        if !same_represented_group {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::NotInSameRaid,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        }

        let sender_active_status = sender_snapshot
            .active_quest_statuses
            .get(&parsed_quest_id)
            .copied();
        if !matches!(
            sender_active_status,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP | QUEST_STATUS_COMPLETE_LIKE_CPP)
        ) {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerNotActiveQuest,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        }

        if !self.can_take_quest(&quest) {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanTakeQuestFailed,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        }

        if self.first_free_quest_slot_like_cpp().is_none() {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestLogFull,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        }

        if quest.source_item_id > 0 {
            let Some(source_item_template) = self.item_storage_template(quest.source_item_id)
            else {
                let source_item_result = InventoryResult::ItemNotFound;
                self.send_equip_error(source_item_result, None, None, 0, 0);
                record(
                    self,
                    RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemFailed,
                    false,
                    Some(source_item_result),
                    false,
                    false,
                    None,
                    0,
                );
                return;
            };

            if source_item_template.item_limit_category != 0 {
                record(
                    self,
                    RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemLimitCategoryUnrepresented,
                    true,
                    None,
                    false,
                    false,
                    None,
                    0,
                );
                return;
            }

            let source_item_count = quest.source_item_count.max(1);
            let source_item_result = self
                .plan_store_new_direct_inventory_item(quest.source_item_id, source_item_count)
                .map(|(result, _dest, _no_space_count)| result)
                .unwrap_or(InventoryResult::ItemNotFound);

            if !matches!(
                source_item_result,
                InventoryResult::Ok | InventoryResult::ItemMaxCount
            ) {
                self.send_equip_error(
                    source_item_result,
                    None,
                    None,
                    0,
                    u32::from(source_item_template.item_limit_category),
                );
                record(
                    self,
                    RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemFailed,
                    false,
                    Some(source_item_result),
                    false,
                    false,
                    None,
                    0,
                );
                return;
            }

            let source_item_no_grant_reason = if self
                .item_template_start_quest_id(quest.source_item_id)
                .is_some_and(|start_quest_id| start_quest_id == quest.id as i32)
            {
                Some(RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverGiveQuestSourceItemStartQuestNoGrant)
            } else if source_item_result == InventoryResult::ItemMaxCount {
                Some(RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverGiveQuestSourceItemMaxCountNoGrant)
            } else {
                None
            };

            if let Some(source_item_no_grant_reason) = source_item_no_grant_reason {
                if !self
                    .add_quest_confirm_accept_local_state_like_cpp(&quest)
                    .await
                {
                    record(
                        self,
                        RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestLogFull,
                        false,
                        None,
                        false,
                        false,
                        None,
                        0,
                    );
                    return;
                }

                let represented_source_spell_id =
                    (quest.source_spell_id > 0).then_some(quest.source_spell_id);
                let represented_source_spell_self_casts = u8::from(quest.source_spell_id > 0) * 2;
                record(
                    self,
                    source_item_no_grant_reason,
                    false,
                    Some(source_item_result),
                    false,
                    false,
                    represented_source_spell_id,
                    represented_source_spell_self_casts,
                );
                return;
            }

            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::GiveQuestSourceItemStoreNewItemUnrepresented,
                false,
                Some(source_item_result),
                true,
                quest.source_spell_id > 0,
                None,
                0,
            );
            return;
        }

        if !self
            .add_quest_confirm_accept_local_state_like_cpp(&quest)
            .await
        {
            record(
                self,
                RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestLogFull,
                false,
                None,
                false,
                false,
                None,
                0,
            );
            return;
        }

        let represented_source_spell_id =
            (quest.source_spell_id > 0).then_some(quest.source_spell_id);
        let represented_source_spell_self_casts = u8::from(quest.source_spell_id > 0) * 2;
        record(
            self,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverAddQuestLocalStateRepresented,
            false,
            None,
            false,
            false,
            represented_source_spell_id,
            represented_source_spell_self_casts,
        );
    }

    /// CMSG_PUSH_QUEST_TO_PARTY — sender-side bounded quest share preflight.
    ///
    /// C++ anchors:
    /// - `Opcodes.cpp:746`: `STATUS_LOGGEDIN`, `PROCESS_THREADUNSAFE`, `HandlePushQuestToParty`.
    /// - `QuestPackets.cpp:658-661`: packet reads one `uint32 QuestID`.
    /// - `QuestHandler.cpp:603-756`: template lookup, `CanShareQuest`, quest-pool active,
    ///   group presence, then receiver iteration.
    ///
    /// Represented-partial: this records sender-local evidence only. It never mutates DB/maps,
    /// never sets receiver pending sharing, and never fans out packets to other sessions.
    /// If the session has no real `player_guid`, Rust records the existing evidence only and
    /// does not fabricate an empty sender GUID for `SMSG_QUEST_PUSH_RESULT`.
    pub async fn handle_push_quest_to_party(&mut self, mut pkt: wow_packet::WorldPacket) {
        let packet = match PushQuestToParty::read(&mut pkt) {
            Ok(packet) => packet,
            Err(error) => {
                warn!(
                    account = self.account_id,
                    ?error,
                    "PushQuestToParty: failed to read QuestID"
                );
                return;
            }
        };

        let Some(quest_store) = self.quest_store.as_ref().map(Arc::clone) else {
            debug!(
                account = self.account_id,
                quest_id = packet.quest_id,
                "PushQuestToParty: missing QuestStore, silent return like missing ObjectMgr template path"
            );
            return;
        };

        let Some(quest) = quest_store.get(packet.quest_id) else {
            debug!(
                account = self.account_id,
                quest_id = packet.quest_id,
                "PushQuestToParty: missing quest template, silent return like C++"
            );
            return;
        };

        let sender_guid = self.player_guid();
        if !self.represented_can_share_quest_like_cpp(quest) {
            self.send_push_quest_result_to_sender_if_available_like_cpp(
                sender_guid,
                quest_push_reason::NOT_ALLOWED,
            );
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotAllowed,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: false,
                },
            );
            return;
        }

        let Some(quest_pool_store) = self.quest_pool_store.as_ref().map(Arc::clone) else {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::QuestPoolActiveCheckUnrepresented,
                    quest_pool_active_check_unrepresented: true,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: false,
                },
            );
            return;
        };

        if !quest_pool_store.is_quest_active_like_cpp(packet.quest_id) {
            self.send_push_quest_result_to_sender_if_available_like_cpp(
                sender_guid,
                quest_push_reason::NOT_DAILY,
            );
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotDaily,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: false,
                },
            );
            return;
        }

        if self.group_guid.is_none() {
            self.send_push_quest_result_to_sender_if_available_like_cpp(
                sender_guid,
                quest_push_reason::NOT_IN_PARTY,
            );
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotInParty,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: false,
                },
            );
            return;
        }

        let Some(group_guid) = self.group_guid else {
            return;
        };

        let Some(group_registry) = self.group_registry().map(Arc::clone) else {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason:
                        RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: true,
                    receiver_fanout_unrepresented: true,
                },
            );
            return;
        };

        let Some(player_registry) = self.player_registry().map(Arc::clone) else {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason:
                        RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: true,
                    receiver_fanout_unrepresented: true,
                },
            );
            return;
        };

        let Some(group_info) = group_registry.get(&group_guid).map(|entry| entry.clone()) else {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason:
                        RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: true,
                    receiver_fanout_unrepresented: true,
                },
            );
            return;
        };

        let receiver_snapshots = group_info
            .members
            .iter()
            .copied()
            .filter(|member_guid| Some(*member_guid) != sender_guid)
            .filter_map(|member_guid| {
                player_registry
                    .get(&member_guid)
                    .map(|receiver| (member_guid, receiver.clone()))
            })
            .collect::<Vec<_>>();

        if receiver_snapshots.is_empty() {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: true,
                },
            );
            return;
        }

        let mut blocked_by_unsupported_success_path = false;
        for (receiver_guid, receiver) in receiver_snapshots {
            if receiver.pending_quest_sharing.is_some() {
                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_BUSY_LIKE_CPP,
                    String::new(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverBusy,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if !receiver.is_alive {
                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_DEAD_LIKE_CPP,
                    String::new(),
                );
                if let Some(sender_guid) = sender_guid {
                    let _ = receiver.send_tx.send(
                        QuestPushResultResponse {
                            sender_guid,
                            result: QUEST_PUSH_REASON_DEAD_TO_RECIPIENT_LIKE_CPP,
                            quest_title: quest.log_title.clone(),
                        }
                        .to_bytes(),
                    );
                }
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverDead,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if receiver.rewarded_quests.contains(&packet.quest_id) {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason:
                            RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverAlreadyDone,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if let Some(status) = receiver
                .active_quest_statuses
                .get(&packet.quest_id)
                .copied()
            {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                if status == QUEST_STATUS_INCOMPLETE_LIKE_CPP
                    || status == QUEST_STATUS_COMPLETE_LIKE_CPP
                {
                    self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                        receiver_guid,
                        QUEST_PUSH_REASON_ON_QUEST_LIKE_CPP,
                        String::new(),
                    );
                    let _ = receiver.send_tx.send(
                        QuestPushResultResponse {
                            sender_guid: sender_guid_for_receiver_packet,
                            result: QUEST_PUSH_REASON_ON_QUEST_TO_RECIPIENT_LIKE_CPP,
                            quest_title: quest.log_title.clone(),
                        }
                        .to_bytes(),
                    );
                    self.record_represented_push_quest_to_party_outcome_like_cpp(
                        RepresentedPushQuestToPartyOutcomeLikeCpp {
                            sender_guid,
                            quest_id: packet.quest_id,
                            target_guid: Some(receiver_guid),
                            reason:
                                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverOnQuest,
                            quest_pool_active_check_unrepresented: false,
                            group_runtime_unrepresented: false,
                            receiver_fanout_unrepresented: false,
                        },
                    );
                    continue;
                }
            }

            // C++ `Player::SatisfyQuestLog(false)` checks `FindQuestSlot(0) <
            // MAX_QUEST_LOG_SIZE`; this represented cross-session seam uses
            // the receiver snapshot derived from `WorldSession.player_quests`
            // slots via `sync_player_registry_state_like_cpp()`.
            if receiver.active_quest_statuses.len() >= MAX_QUEST_LOG_SIZE_LIKE_CPP as usize {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_LOG_FULL_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_LOG_FULL_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverLogFull,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            // C++ `Player::SatisfyQuestDay(quest, false)` immediately follows
            // `SatisfyQuestLog(false)` in `WorldSession::HandlePushQuestToParty`.
            // Non-daily/non-DF quests pass this gate; already-completed daily
            // quests and represented DF quests send the same AlreadyDone pair
            // as the earlier rewarded/onquest branch.
            let already_satisfied_quest_day_like_cpp = if quest.is_df_quest_like_cpp() {
                receiver.df_quests.contains(&packet.quest_id)
            } else if quest.is_daily_like_cpp() {
                receiver.daily_quests_completed.contains(&packet.quest_id)
            } else {
                false
            };

            if already_satisfied_quest_day_like_cpp {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            // C++ then evaluates `Player::SatisfyQuestMinLevel(quest, false)`
            // followed by `SatisfyQuestMaxLevel(quest, false)`.  Receiver
            // `level` is a derived cross-session snapshot synchronized from
            // the receiver `WorldSession`, never source-of-truth in reverse.
            if quest.min_level > 0 && i32::from(receiver.level) < quest.min_level {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_LOW_LEVEL_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_LOW_LEVEL_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMinLevelLowLevel,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if quest.max_level > 0 && receiver.level > quest.max_level {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_HIGH_LEVEL_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_HIGH_LEVEL_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMaxLevelHighLevel,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            // C++ order then evaluates `Player::SatisfyQuestClass(quest, false)`
            // followed by `SatisfyQuestRace(quest, false)`. Receiver class/race
            // are read-only `PlayerRegistry` snapshots derived from the receiver
            // `WorldSession`; never sync registry state back into the session.
            let receiver_class_mask = player_race_or_class_mask_like_cpp(receiver.class);
            if quest.allowable_classes != 0 && (quest.allowable_classes & receiver_class_mask) == 0
            {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_CLASS_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_CLASS_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestClassWrongClass,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            let receiver_race_mask = u64::from(player_race_or_class_mask_like_cpp(receiver.race));
            if quest.allowable_races != 0 && (quest.allowable_races & receiver_race_mask) == 0 {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_RACE_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_RACE_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestRaceWrongRace,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            let receiver_reputation_standing_like_cpp = |faction_id: u32| -> i32 {
                receiver
                    .reputation_standings
                    .iter()
                    .find_map(|(stored_faction_id, standing)| {
                        (*stored_faction_id == faction_id).then_some(*standing)
                    })
                    .unwrap_or(0)
            };

            let reputation_failure_reason = if quest.required_min_rep_faction != 0
                && receiver_reputation_standing_like_cpp(quest.required_min_rep_faction)
                    < quest.required_min_rep_value
            {
                Some(RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationLowFaction)
            } else if quest.required_max_rep_faction != 0
                && receiver_reputation_standing_like_cpp(quest.required_max_rep_faction)
                    >= quest.required_max_rep_value
            {
                Some(RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationHighFaction)
            } else {
                None
            };

            if let Some(reason) = reputation_failure_reason {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_LOW_FACTION_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_LOW_FACTION_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            // C++ `Player::SatisfyQuestDependentQuests(quest, false)` preserves this
            // sub-gate order: PreviousQuest, DependentPreviousQuests,
            // BreadcrumbQuest, DependentBreadcrumbQuests. Expansion, CanTakeQuest,
            // success fanout, SetQuestSharingInfo, details, and auto-accept stay
            // deliberately unsupported after these represented prerequisites.
            let previous_quest_prerequisite_failed = if quest.prev_quest_id > 0 {
                !receiver
                    .rewarded_quests
                    .contains(&quest.prev_quest_id.unsigned_abs())
            } else if quest.prev_quest_id < 0 {
                receiver
                    .active_quest_statuses
                    .get(&quest.prev_quest_id.unsigned_abs())
                    .copied()
                    != Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP)
            } else {
                false
            };

            let mut prerequisite_failure_reason =
                previous_quest_prerequisite_failed.then_some(
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite,
                );

            if prerequisite_failure_reason.is_none()
                && represented_satisfy_quest_dependent_previous_quests_failed_like_cpp(
                    &quest_store,
                    quest,
                    &receiver.rewarded_quests,
                )
            {
                prerequisite_failure_reason = Some(
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite,
                );
            }

            if prerequisite_failure_reason.is_none() && quest.breadcrumb_for_quest_id != 0 {
                // C++ `SatisfyQuestBreadcrumbQuest` depends on
                // `CanTakeQuest(target,false)`. Do not fake it here; keep the
                // success path blocked until real/represented CanTakeQuest is available.
                blocked_by_unsupported_success_path = true;
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: true,
                    },
                );
                continue;
            }

            if prerequisite_failure_reason.is_none()
                && represented_satisfy_quest_dependent_breadcrumb_quests_failed_like_cpp(
                    quest,
                    &receiver.active_quest_statuses,
                )
            {
                prerequisite_failure_reason = Some(
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentBreadcrumbQuestsPrerequisite,
                );
            }

            if let Some(reason) = prerequisite_failure_reason {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if i32::from(receiver.active_expansion) < quest.expansion {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_EXPANSION_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_EXPANSION_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestExpansionRequiredExpansion,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if !represented_can_take_quest_after_expansion_like_cpp(&quest_store, quest, &receiver)
            {
                let Some(sender_guid_for_receiver_packet) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    continue;
                };

                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_INVALID_LIKE_CPP,
                    String::new(),
                );
                let _ = receiver.send_tx.send(
                    QuestPushResultResponse {
                        sender_guid: sender_guid_for_receiver_packet,
                        result: QUEST_PUSH_REASON_INVALID_TO_RECIPIENT_LIKE_CPP,
                        quest_title: quest.log_title.clone(),
                    }
                    .to_bytes(),
                );
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverCanTakeQuestInvalid,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            if quest.is_turn_in_like_cpp()
                && quest.is_repeatable()
                && !quest.is_daily_or_weekly_like_cpp()
            {
                let Some(sender_guid_for_receiver_command) = sender_guid else {
                    blocked_by_unsupported_success_path = true;
                    self.record_represented_push_quest_to_party_outcome_like_cpp(
                        RepresentedPushQuestToPartyOutcomeLikeCpp {
                            sender_guid,
                            quest_id: packet.quest_id,
                            target_guid: Some(receiver_guid),
                            reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPromptCommandFailed,
                            quest_pool_active_check_unrepresented: false,
                            group_runtime_unrepresented: false,
                            receiver_fanout_unrepresented: true,
                        },
                    );
                    continue;
                };

                let command = SessionCommand::SendRepeatableTurnInRequestItemsLikeCpp(
                    SendRepeatableTurnInRequestItemsLikeCppCommand {
                        sender_guid: sender_guid_for_receiver_command,
                        quest: quest.clone(),
                    },
                );

                if receiver.command_tx.try_send(command).is_err() {
                    blocked_by_unsupported_success_path = true;
                    self.record_represented_push_quest_to_party_outcome_like_cpp(
                        RepresentedPushQuestToPartyOutcomeLikeCpp {
                            sender_guid,
                            quest_id: packet.quest_id,
                            target_guid: Some(receiver_guid),
                            reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPromptCommandFailed,
                            quest_pool_active_check_unrepresented: false,
                            group_runtime_unrepresented: false,
                            receiver_fanout_unrepresented: true,
                        },
                    );
                    continue;
                }

                // C++ `HandlePushQuestToParty` sends Success to the sender before the
                // repeatable turn-in `SendQuestGiverRequestItems` receiver side effect.
                // Rust has an extra fallible queue hop, so emit represented Success only
                // after the receiver command has been accepted.
                self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                    receiver_guid,
                    QUEST_PUSH_REASON_SUCCESS_LIKE_CPP,
                    String::new(),
                );

                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPrompted,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: false,
                    },
                );
                continue;
            }

            let Some(sender_guid_for_receiver_command) = sender_guid else {
                blocked_by_unsupported_success_path = true;
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverQuestDetailsPromptCommandFailed,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: true,
                    },
                );
                continue;
            };

            let command = SessionCommand::SetQuestSharingInfoAndSendDetails(
                SetQuestSharingInfoAndSendDetailsCommand {
                    sender_guid: sender_guid_for_receiver_command,
                    quest: quest.clone(),
                },
            );

            if receiver.command_tx.try_send(command).is_err() {
                blocked_by_unsupported_success_path = true;
                self.record_represented_push_quest_to_party_outcome_like_cpp(
                    RepresentedPushQuestToPartyOutcomeLikeCpp {
                        sender_guid,
                        quest_id: packet.quest_id,
                        target_guid: Some(receiver_guid),
                        reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverQuestDetailsPromptCommandFailed,
                        quest_pool_active_check_unrepresented: false,
                        group_runtime_unrepresented: false,
                        receiver_fanout_unrepresented: true,
                    },
                );
                continue;
            }

            self.send_push_quest_result_to_sender_with_title_if_available_like_cpp(
                receiver_guid,
                QUEST_PUSH_REASON_SUCCESS_LIKE_CPP,
                String::new(),
            );
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: Some(receiver_guid),
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: false,
                },
            );
        }

        if blocked_by_unsupported_success_path {
            self.record_represented_push_quest_to_party_outcome_like_cpp(
                RepresentedPushQuestToPartyOutcomeLikeCpp {
                    sender_guid,
                    quest_id: packet.quest_id,
                    target_guid: sender_guid,
                    reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented,
                    quest_pool_active_check_unrepresented: false,
                    group_runtime_unrepresented: false,
                    receiver_fanout_unrepresented: true,
                },
            );
        }
    }

    fn represented_can_share_quest_like_cpp(&self, quest: &wow_data::quest::QuestTemplate) -> bool {
        quest.flags & QUEST_FLAGS_SHARABLE_LIKE_CPP != 0
            && self.player_quests.contains_key(&quest.id)
    }

    fn send_push_quest_result_to_sender_if_available_like_cpp(
        &self,
        sender_guid: Option<ObjectGuid>,
        result: u8,
    ) {
        if let Some(sender_guid) = sender_guid {
            self.send_packet(&QuestPushResultResponse {
                sender_guid,
                result,
                quest_title: String::new(),
            });
        }
    }

    fn send_push_quest_result_to_sender_with_title_if_available_like_cpp(
        &self,
        sender_guid: ObjectGuid,
        result: u8,
        quest_title: String,
    ) {
        self.send_packet(&QuestPushResultResponse {
            sender_guid,
            result,
            quest_title,
        });
    }

    /// CMSG_QUEST_PUSH_RESULT — response to a shared quest prompt.
    ///
    /// C++ anchor: `WorldSession::HandleQuestPushResult`, `QuestHandler.cpp:758-767`.
    /// Represented-partial: session-local pending sharing state is cleared like C++;
    /// matching sender responses are recorded as evidence because full `ObjectAccessor::FindPlayer`
    /// and party sender packet fanout are not represented in this bounded slice.
    pub async fn handle_quest_push_result(&mut self, mut pkt: wow_packet::WorldPacket) {
        let packet = match QuestPushResult::read(&mut pkt) {
            Ok(packet) => packet,
            Err(error) => {
                warn!(
                    account = self.account_id,
                    ?error,
                    "QuestPushResult: failed to read SenderGUID/QuestID/Result"
                );
                return;
            }
        };

        let Some(pending) = self.represented_pending_quest_sharing_like_cpp() else {
            debug!(
                account = self.account_id,
                sender_guid = ?packet.sender_guid,
                quest_id = packet.quest_id,
                result = packet.result,
                "QuestPushResult: no represented pending shared quest"
            );
            return;
        };

        self.clear_represented_pending_quest_sharing_like_cpp();

        if pending.sender_guid != packet.sender_guid {
            self.record_represented_quest_push_result_sender_mismatch_like_cpp();
            debug!(
                account = self.account_id,
                pending_sender_guid = ?pending.sender_guid,
                packet_sender_guid = ?packet.sender_guid,
                "QuestPushResult: represented sender mismatch, pending state cleared"
            );
            return;
        }

        let Some(receiver_guid) = self.player_guid() else {
            debug!(
                account = self.account_id,
                sender_guid = ?packet.sender_guid,
                "QuestPushResult: represented sender matched but no local receiver guid is available"
            );
            return;
        };

        self.record_represented_quest_push_result_response_like_cpp(
            RepresentedQuestPushResultResponseLikeCpp {
                receiver_guid,
                sender_guid: packet.sender_guid,
                parsed_quest_id: packet.quest_id,
                pending_quest_id: pending.quest_id,
                result: packet.result,
            },
        );
    }

    /// CMSG_QUEST_LOG_REMOVE_QUEST — abandon quest-log slot.

    /// Represented-partial seam: explicit QuestLog slot lookup + local active quest removal/DB delete.
    /// Remaining gaps: source-item gates/cleanup, no-abandon-once-begun, timed/PvP state,
    /// personal summons, quest tracker DB, ScriptMgr callbacks, and criteria update evidence.
    pub async fn handle_quest_log_remove_quest(&mut self, mut pkt: wow_packet::WorldPacket) {
        let slot = match pkt.read_uint8() {
            Ok(slot) => slot,
            Err(error) => {
                warn!(
                    account = self.account_id,
                    ?error,
                    "QuestLogRemoveQuest: failed to read Entry"
                );
                return;
            }
        };

        debug!(
            account = self.account_id,
            slot, "QuestLogRemoveQuest: represented slot-backed abandon request"
        );

        if slot >= MAX_QUEST_LOG_SIZE_LIKE_CPP {
            debug!(
                account = self.account_id,
                slot, "QuestLogRemoveQuest: slot outside MAX_QUEST_LOG_SIZE"
            );
            return;
        }

        let Some(qid) = self.get_quest_slot_quest_id_like_cpp(slot) else {
            debug!(
                account = self.account_id,
                slot,
                "QuestLogRemoveQuest: valid slot empty; criteria update remains an explicit gap"
            );
            return;
        };

        self.player_quests.remove(&qid);
        self.delete_quest_from_db(qid).await;
        self.sync_player_registry_state_like_cpp();
        info!(
            account = self.account_id,
            quest_id = qid,
            slot,
            "Quest abandoned via represented explicit quest-log slot"
        );
    }

    pub(crate) fn first_free_quest_slot_like_cpp(&self) -> Option<u8> {
        (0..MAX_QUEST_LOG_SIZE_LIKE_CPP)
            .find(|&slot| !self.quest_slot_has_active_entry_like_cpp(slot))
    }

    fn quest_slot_has_active_entry_like_cpp(&self, slot: u8) -> bool {
        // C++ `QuestSlotOffset` stores the quest id independently from the status fields;
        // represented active slots are COMPLETE or INCOMPLETE only for this bounded helper.
        slot < MAX_QUEST_LOG_SIZE_LIKE_CPP
            && self.player_quests.values().any(|status| {
                status.slot == slot
                    && matches!(
                        status.status,
                        QUEST_STATUS_COMPLETE_LIKE_CPP | QUEST_STATUS_INCOMPLETE_LIKE_CPP
                    )
            })
    }

    pub(crate) fn get_quest_slot_quest_id_like_cpp(&self, slot: u8) -> Option<u32> {
        if slot >= MAX_QUEST_LOG_SIZE_LIKE_CPP {
            return None;
        }

        let mut matching_quest_id = None;
        for status in self.player_quests.values().filter(|status| {
            status.slot == slot
                && matches!(
                    status.status,
                    QUEST_STATUS_COMPLETE_LIKE_CPP | QUEST_STATUS_INCOMPLETE_LIKE_CPP
                )
        }) {
            if matching_quest_id.is_some() {
                return None;
            }

            matching_quest_id = Some(status.quest_id);
        }

        matching_quest_id
    }

    pub(crate) fn find_quest_slot_like_cpp(&self, quest_id: u32) -> Option<u8> {
        self.player_quests.get(&quest_id).and_then(|status| {
            (status.slot < MAX_QUEST_LOG_SIZE_LIKE_CPP
                && (status.status == 1 || status.status == 2))
                .then_some(status.slot)
        })
    }

    pub(crate) fn quest_log_create_entries_like_cpp(&self) -> Vec<(u32, u32, i64, [u16; 24])> {
        (0..MAX_QUEST_LOG_SIZE_LIKE_CPP)
            .map(|slot| {
                let Some(quest_id) = self.get_quest_slot_quest_id_like_cpp(slot) else {
                    return (0, 0, 0, [0; 24]);
                };
                let Some(qs) = self.player_quests.get(&quest_id) else {
                    return (0, 0, 0, [0; 24]);
                };

                let state_flags: u32 = if qs.status == 2 { 1 } else { 0 };
                let mut obj_progress = [0u16; 24];
                for (i, &count) in qs.objective_counts.iter().enumerate().take(24) {
                    obj_progress[i] = count.min(u16::MAX as i32) as u16;
                }
                (qs.quest_id, state_flags, 0i64, obj_progress)
            })
            .collect()
    }

    /// CMSG_QUERY_QUEST_INFO — client asks for full quest template data by ID.
    /// Used to populate the quest log and tooltip.
    /// C# ref: QuestHandler.HandleQueryQuestInfo
    pub async fn handle_query_quest_info(&mut self, mut pkt: wow_packet::WorldPacket) {
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);
        let _guid = pkt.read_packed_guid(); // requester GUID (usually player)

        let quest_store = match &self.quest_store {
            Some(s) => Arc::clone(s),
            None => {
                self.send_packet(&QueryQuestInfoResponse {
                    quest_id,
                    allow: false,
                    ..Default::default()
                });
                return;
            }
        };

        match quest_store.get(quest_id) {
            None => {
                self.send_packet(&QueryQuestInfoResponse {
                    quest_id,
                    allow: false,
                    ..Default::default()
                });
            }
            Some(quest) => {
                let objectives: Vec<QuestObjectiveInfo> = quest
                    .objectives
                    .iter()
                    .map(|obj| QuestObjectiveInfo {
                        id: obj.id,
                        obj_type: obj.obj_type,
                        storage_index: obj.storage_index,
                        object_id: obj.object_id,
                        amount: obj.amount,
                        flags: obj.flags,
                        flags2: obj.flags2,
                        progress_bar_weight: obj.progress_bar_weight,
                        description: obj.description.clone(),
                    })
                    .collect();

                self.send_packet(&QueryQuestInfoResponse {
                    quest_id,
                    allow: true,
                    quest_type: quest.quest_type,
                    quest_level: quest.quest_level,
                    quest_max_scaling_level: quest.quest_max_scaling_level,
                    min_level: quest.min_level,
                    quest_sort_id: quest.quest_sort_id,
                    quest_info_id: quest.quest_info_id,
                    suggested_group_num: quest.suggested_group_num,
                    reward_next_quest: quest.reward_next_quest,
                    reward_xp_difficulty: quest.reward_xp_difficulty,
                    reward_money_difficulty: quest.reward_money_difficulty,
                    flags: quest.flags,
                    flags_ex: quest.flags_ex,
                    flags_ex2: quest.flags_ex2,
                    reward_items: quest.reward_items,
                    reward_amounts: quest.reward_amounts,
                    reward_display_spell: quest.reward_display_spell,
                    reward_spell: quest.reward_spell,
                    objectives,
                    log_title: quest.log_title.clone(),
                    log_description: quest.log_description.clone(),
                    quest_description: quest.quest_description.clone(),
                    area_description: quest.area_description.clone(),
                    quest_completion_log: quest.quest_completion_log.clone(),
                });
            }
        }
    }

    /// CMSG_QUERY_QUEST_COMPLETION_NPCS — client asks for Creature/GO quest enders.
    /// C++ refs:
    /// - `WorldSession::HandleQueryQuestCompletionNPCs`, QueryHandler.cpp:252-278.
    /// - `QuestCompletionNPCResponse::Write`, QueryPackets.cpp:451-462.
    pub async fn handle_query_quest_completion_npcs(&mut self, query: QueryQuestCompletionNpcs) {
        let quests = self
            .quest_store
            .as_deref()
            .map_or_else(Vec::new, |quest_store| {
                represented_quest_completion_npc_response_like_cpp(quest_store, &query.quest_ids)
            });

        self.send_packet(&QuestCompletionNpcResponse { quests });
    }

    /// CMSG_QUEST_GIVER_REQUEST_REWARD — player talks to NPC to turn in a completed quest.
    /// C# ref: QuestHandler.HandleQuestgiverRequestReward
    /// Sent when player right-clicks a quest-ender NPC and has the quest in Complete status.
    /// Server responds with SMSG_QUEST_GIVER_OFFER_REWARD_MESSAGE (reward selection dialog).
    pub async fn handle_quest_giver_request_reward(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverRequestReward: failed to read GUID");
                return;
            }
        };
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);

        let quest_store = match &self.quest_store {
            Some(s) => Arc::clone(s),
            None => return,
        };
        let quest = match quest_store.get(quest_id) {
            Some(q) => q.clone(),
            None => {
                warn!(
                    account = self.account_id,
                    quest_id, "RequestReward: unknown quest"
                );
                return;
            }
        };

        if self.is_quest_disabled_like_cpp(quest_id) {
            debug!(
                account = self.account_id,
                quest_id, "RequestReward: quest disabled"
            );
            return;
        }

        if quest.flags & QUEST_FLAGS_AUTO_COMPLETE_LIKE_CPP == 0
            && !self.represented_quest_giver_involved_source_allows_quest_like_cpp(
                guid,
                quest_id,
                &quest_store,
            )
        {
            debug!(
                account = self.account_id,
                ?guid,
                quest_id,
                "RequestReward: represented involved source rejected"
            );
            return;
        }

        // C++: if (_player->CanCompleteQuest(questID)) _player->CompleteQuest(questID)
        // We check if all objectives are done; if so, upgrade status to Complete (2).
        let is_complete = self
            .player_quests
            .get(&quest_id)
            .map_or(false, |qs| qs.status == 2);

        if !is_complete {
            // Objectives not finished — silently ignore
            // (C# would send SMSG_QUEST_GIVER_REQUEST_ITEMS instead)
            debug!(
                account = self.account_id,
                quest_id, "RequestReward: quest not complete"
            );
            return;
        }

        // Build rewards block for the offer-reward dialog
        let mut rewards = QuestRewardsBlock::default();
        rewards.money = quest.reward_money_difficulty as i32;
        for i in 0..4 {
            rewards.items[i] = (quest.reward_items[i], quest.reward_amounts[i]);
        }
        for i in 0..3 {
            rewards.display_spells[i] = quest.reward_display_spell[i];
        }
        rewards.completion_spell = quest.reward_spell as i32;
        // Populate choice items for the dialog
        for i in 0..6 {
            rewards.choice_items[i] = (
                quest.reward_choice_items[i].0,
                quest.reward_choice_items[i].1,
            );
        }

        // C#: SendQuestGiverOfferReward(quest, questGiverGUID, true)
        self.send_packet(&QuestGiverOfferReward {
            giver_guid: guid,
            quest_id,
            quest_flags: [quest.flags, quest.flags_ex, quest.flags_ex2],
            suggested_party_members: quest.suggested_group_num,
            rewards,
            title: quest.log_title.clone(),
            reward_text: quest.quest_completion_log.clone(),
            auto_launched: false,
        });
    }

    /// CMSG_QUEST_GIVER_COMPLETE_QUEST — player talks to quest-ender NPC.
    /// If objectives are done: show reward dialog. Else: show "still need X" dialog.
    /// C# ref: QuestHandler.HandleQuestGiverCompleteQuest
    pub async fn handle_quest_giver_complete_quest(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("QuestGiverCompleteQuest: failed to read GUID");
                return;
            }
        };
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);
        let from_script: bool = pkt.read_bit().unwrap_or(false);

        let quest_store = match &self.quest_store {
            Some(s) => Arc::clone(s),
            None => return,
        };

        let quest = match quest_store.get(quest_id) {
            Some(q) => q,
            None => {
                warn!(
                    account = self.account_id,
                    quest_id, "QuestGiverCompleteQuest: unknown quest"
                );
                return;
            }
        };

        if self.is_quest_disabled_like_cpp(quest_id) {
            debug!(
                account = self.account_id,
                quest_id, "QuestGiverCompleteQuest: quest disabled"
            );
            return;
        }

        if quest.flags & QUEST_FLAGS_AUTO_COMPLETE_LIKE_CPP == 0 {
            if from_script
                || !self.represented_quest_giver_involved_source_allows_quest_like_cpp(
                    guid,
                    quest_id,
                    &quest_store,
                )
            {
                debug!(
                    account = self.account_id,
                    ?guid,
                    quest_id,
                    from_script,
                    "QuestGiverCompleteQuest: represented involved source rejected"
                );
                return;
            }
        } else if !from_script || self.player_guid() != Some(guid) {
            debug!(
                account = self.account_id,
                ?guid,
                quest_id,
                from_script,
                "QuestGiverCompleteQuest: auto-complete source is not script/player"
            );
            return;
        }

        // Check if player has the quest active
        if !self.has_quest(quest_id) {
            debug!(
                account = self.account_id,
                quest_id, "Player doesn't have quest"
            );
            return;
        }

        // Build rewards block
        let mut rewards = QuestRewardsBlock::default();
        rewards.money = quest.reward_money_difficulty as i32;
        for i in 0..4 {
            rewards.items[i] = (quest.reward_items[i], quest.reward_amounts[i]);
        }
        for i in 0..3 {
            rewards.display_spells[i] = quest.reward_display_spell[i];
        }
        rewards.completion_spell = quest.reward_spell as i32;

        // Check if all objectives are done — C# GetQuestStatus == QuestStatus.Complete
        let is_complete = self
            .player_quests
            .get(&quest_id)
            .map_or(false, |qs| qs.status == 2);

        if !is_complete {
            // Not all objectives done — send "you still need X" dialog
            // C# ref: SendQuestGiverRequestItems(quest, guid, canComplete=false, false)
            self.send_packet(&QuestGiverRequestItems {
                giver_guid: guid,
                giver_creature_id: i32::try_from(guid.entry()).unwrap_or(0),
                quest_id,
                comp_emote_delay: 0,
                comp_emote_type: 0,
                quest_flags: [quest.flags, quest.flags_ex, quest.flags_ex2],
                suggested_party_members: quest.suggested_group_num,
                money_to_get: 0,
                collect: Vec::new(),
                currency: Vec::new(),
                status_flags: 0xFD,
                title: quest.log_title.clone(),
                completion_text: quest.area_description.clone(),
                auto_launched: false,
            });
            return;
        }

        // All objectives done — show offer reward dialog
        self.send_packet(&QuestGiverOfferReward {
            giver_guid: guid,
            quest_id,
            quest_flags: [quest.flags, quest.flags_ex, quest.flags_ex2],
            suggested_party_members: quest.suggested_group_num,
            rewards,
            title: quest.log_title.clone(),
            reward_text: quest.quest_completion_log.clone(),
            auto_launched: false,
        });
    }

    /// CMSG_QUEST_GIVER_CHOOSE_REWARD — player clicks "Complete Quest" in reward dialog.
    /// Gives XP, gold, items. Removes quest from active log.
    /// C# ref: QuestHandler.HandleQuestGiverChooseReward
    pub async fn handle_quest_giver_choose_reward(&mut self, mut pkt: wow_packet::WorldPacket) {
        let guid = match pkt.read_packed_guid() {
            Ok(g) => g,
            Err(_) => {
                warn!("ChooseReward: failed to read GUID");
                return;
            }
        };
        let quest_id: u32 = pkt.read_uint32().unwrap_or(0);
        let choice_item_id: u32 = pkt.read_uint32().unwrap_or(0);
        let _loot_item_type: u32 = pkt.read_uint32().unwrap_or(0); // 0=Item, 1=Currency

        let quest_store = match &self.quest_store {
            Some(s) => Arc::clone(s),
            None => return,
        };
        let quest = match quest_store.get(quest_id) {
            Some(q) => q.clone(),
            None => {
                warn!(
                    account = self.account_id,
                    quest_id, "ChooseReward: unknown quest"
                );
                return;
            }
        };

        if self.is_quest_disabled_like_cpp(quest_id) {
            debug!(
                account = self.account_id,
                quest_id, "ChooseReward: quest disabled"
            );
            return;
        }

        // SatisfyQuestStatus — C# HandleQuestgiverChooseReward line ~370
        // Player must have the quest active AND it must be complete (status=2)
        let quest_status = self.player_quests.get(&quest_id).map(|qs| qs.status);
        match quest_status {
            Some(2) => {} // Complete — ok
            Some(1) => {
                warn!(
                    account = self.account_id,
                    quest_id, "ChooseReward: quest not complete yet"
                );
                return;
            }
            _ => {
                warn!(
                    account = self.account_id,
                    quest_id, "ChooseReward: player doesn't have quest"
                );
                return;
            }
        }

        // Validate choice item — C# HandleQuestgiverChooseReward lines 255-310
        // If client sends a non-zero choice item, it must be in reward_choice_items.
        if choice_item_id != 0 {
            let valid = quest
                .reward_choice_items
                .iter()
                .any(|(item_id, _qty)| *item_id == choice_item_id);
            if !valid {
                warn!(
                    account = self.account_id,
                    quest_id,
                    choice_item_id,
                    "ChooseReward: choice item not valid for this quest (possible exploit)"
                );
                return;
            }
        }

        // C++ HandleQuestgiverChooseRewardOpcode keeps `object = _player` for auto-complete,
        // but non-auto-complete quests must resolve the packet source as an involved
        // Unit/GameObject and pass CanInteractWithQuestGiver before RewardQuest mutates state.
        // This represented-partial slice intentionally keeps the existing bounded choice-item
        // validation only; QuestPackageItems and CurrencyTypes DB2 validation remain gaps.
        if quest.flags & QUEST_FLAGS_AUTO_COMPLETE_LIKE_CPP == 0
            && !self.represented_quest_giver_involved_source_allows_quest_like_cpp(
                guid,
                quest_id,
                &quest_store,
            )
        {
            debug!(
                account = self.account_id,
                ?guid,
                quest_id,
                "ChooseReward: represented involved source rejected"
            );
            return;
        }

        // Give gold reward
        let money = quest.reward_money_difficulty;
        if money > 0 {
            self.set_player_gold_like_cpp(self.player_gold_like_cpp().saturating_add(money as u64));
            self.save_player_gold().await;
        }

        let xp = self.calculate_quest_xp(quest.reward_xp_difficulty, quest.quest_level);

        // RewardQuest — C# Player.RewardQuest:
        // 1. Remove from active quest log
        // 2. Mark as rewarded (status=3) in DB and in memory
        // Non-repeatable quests go into rewarded_quests; repeatable quests stay removed
        self.player_quests.remove(&quest_id);
        if !quest.is_repeatable() {
            self.rewarded_quests.insert(quest_id);
            self.save_quest_to_db(quest_id, 3).await; // status=3 = Rewarded
        } else {
            self.delete_quest_from_db(quest_id).await;
        }
        self.sync_player_registry_state_like_cpp();

        info!(
            account = self.account_id,
            quest_id,
            xp,
            gold = money,
            repeatable = quest.is_repeatable(),
            "Quest rewarded"
        );

        // C++ Player::SendQuestReward calls sGameEventMgr->HandleQuestComplete(questId)
        // before building/sending the reward packet. This bounded bridge reports
        // failures explicitly and never blocks the reward flow indefinitely.
        let game_event_outcome = self
            .notify_game_event_quest_complete_like_cpp(quest_id)
            .await;
        debug!(
            account = self.account_id,
            quest_id,
            outcome = ?game_event_outcome,
            "Represented C++ GameEventMgr::HandleQuestComplete notification after quest reward"
        );

        // SMSG_QUEST_GIVER_QUEST_COMPLETE — reward popup with XP/gold
        self.send_packet(&QuestGiverQuestComplete {
            quest_id,
            xp,
            money,
            skill_points: 0,
            use_quest_reward_currency: false,
        });

        // SMSG_QUEST_UPDATE_COMPLETE — removes from quest log UI
        self.send_packet(&QuestUpdateComplete { quest_id });

        // Give XP reward — C# Player.RewardQuest → GiveXP
        if xp > 0 {
            let player_guid = self
                .player_guid()
                .unwrap_or(wow_core::ObjectGuid::new(0, 0));
            self.give_xp(xp, player_guid, false).await;
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Resolves CMSG_QUEST_GIVER_STATUS_QUERY through the represented equivalent of
    /// C++ `ObjectAccessor::GetObjectByTypeMask(*_player, guid, TYPEMASK_UNIT | TYPEMASK_GAMEOBJECT)`.
    /// Missing canonical objects and unsupported Player/Item/other GUID types fail closed with no packet.
    pub(crate) fn represented_quest_giver_status_query_source_like_cpp(
        &self,
        guid: wow_core::ObjectGuid,
    ) -> Option<RepresentedQuestGiverStatusSourceLikeCpp> {
        if guid.is_any_type_creature() {
            // C++ TYPEID_UNIT branch also checks Creature::IsHostileTo before computing
            // dialog status. Exact faction/hostility is not represented here yet; a
            // resolved canonical Creature is treated as non-hostile only for this
            // bounded represented status calculation.
            let access = self.canonical_creature_access_like_cpp(guid)?;
            return Some(RepresentedQuestGiverStatusSourceLikeCpp::Creature {
                entry: access.entry,
            });
        }

        if guid.is_game_object() {
            let access = self.canonical_gameobject_access_like_cpp(guid)?;
            return Some(RepresentedQuestGiverStatusSourceLikeCpp::GameObject {
                entry: access.entry,
            });
        }

        None
    }

    /// Bounded representation of C++ `Player::GetQuestDialogStatus(Object const*)`.
    /// Creature sources use Creature starter/ender relations; GameObject sources use
    /// GO starter/ender relations. Full AI dialog status, ConditionMgr, event overlays
    /// and important/daily/trivial/future/repeatable/covenant/legendary/POI bit flags
    /// remain documented migration gaps for this slice.
    pub(crate) fn get_represented_quest_giver_status_like_cpp(
        &self,
        source: RepresentedQuestGiverStatusSourceLikeCpp,
    ) -> u64 {
        let Some(store) = &self.quest_store else {
            return quest_giver_status::NONE;
        };

        let has_turn_in = match source {
            RepresentedQuestGiverStatusSourceLikeCpp::Creature { entry } => store
                .quests_for_ender(entry)
                .iter()
                .any(|q| self.completed_quest_can_reward_status_like_cpp(q.id)),
            RepresentedQuestGiverStatusSourceLikeCpp::GameObject { entry } => store
                .quests_for_gameobject_ender(entry)
                .iter()
                .any(|q| self.completed_quest_can_reward_status_like_cpp(q.id)),
        };

        if has_turn_in {
            return quest_giver_status::CAN_REWARD;
        }

        let has_available = match source {
            RepresentedQuestGiverStatusSourceLikeCpp::Creature { entry } => store
                .quests_for_starter(entry)
                .iter()
                .any(|q| self.can_take_quest(q)),
            RepresentedQuestGiverStatusSourceLikeCpp::GameObject { entry } => store
                .quests_for_gameobject_starter(entry)
                .iter()
                .any(|q| self.can_take_quest(q)),
        };

        if has_available {
            quest_giver_status::AVAILABLE
        } else {
            quest_giver_status::NONE
        }
    }

    fn completed_quest_can_reward_status_like_cpp(&self, quest_id: u32) -> bool {
        !self.is_quest_disabled_like_cpp(quest_id)
            && self
                .player_quests
                .get(&quest_id)
                .is_some_and(|qs| qs.status == 2)
    }

    /// Check if the player currently has an active quest with the given ID.
    pub fn has_quest(&self, quest_id: u32) -> bool {
        self.player_quests.contains_key(&quest_id)
    }

    /// Full eligibility check before accepting a quest.
    /// C# ref: Player.CanTakeQuest (SatisfyQuestStatus + SatisfyQuestRace/Class/Level + PrevQuest)
    pub fn can_take_quest(&self, quest: &wow_data::quest::QuestTemplate) -> bool {
        if self.is_quest_disabled_like_cpp(quest.id) {
            debug!(
                account = self.account_id,
                quest_id = quest.id,
                "CanTakeQuest: quest disabled"
            );
            return false;
        }

        // SatisfyQuestStatus — C# lines 1624-1654
        // If quest is already rewarded (non-repeatable), cannot take again.
        if self.rewarded_quests.contains(&quest.id) && !quest.is_repeatable() {
            debug!(
                account = self.account_id,
                quest_id = quest.id,
                "CanTakeQuest: already rewarded"
            );
            return false;
        }
        // If quest is already active, cannot accept again.
        if self.player_quests.contains_key(&quest.id) {
            debug!(
                account = self.account_id,
                quest_id = quest.id,
                "CanTakeQuest: already active"
            );
            return false;
        }

        // SatisfyQuestSeasonal — C++ Player::SatisfyQuestSeasonal
        if quest.is_seasonal_like_cpp() && !self.seasonal_quests_like_cpp.is_empty() {
            if let Some(bucket) = self
                .seasonal_quests_like_cpp
                .get(&quest.event_id_for_quest_like_cpp())
            {
                if !bucket.is_empty() && bucket.contains_key(&quest.id) {
                    debug!(
                        account = self.account_id,
                        quest_id = quest.id,
                        event_id = quest.event_id_for_quest_like_cpp(),
                        "CanTakeQuest: seasonal quest cooldown"
                    );
                    return false;
                }
            }
        }

        // SatisfyQuestPreviousQuest — C# lines 1415-1440
        // prev_quest_id > 0 → previous quest must have been rewarded
        // prev_quest_id < 0 → previous quest must be currently active (Incomplete)
        if quest.prev_quest_id != 0 {
            let prev_id = quest.prev_quest_id.unsigned_abs();
            if quest.prev_quest_id > 0 {
                if !self.rewarded_quests.contains(&prev_id) {
                    debug!(
                        account = self.account_id,
                        quest_id = quest.id,
                        prev_id,
                        "CanTakeQuest: prev quest not rewarded"
                    );
                    return false;
                }
            } else {
                // negative: prev quest must be active
                let active = self
                    .player_quests
                    .get(&prev_id)
                    .map_or(false, |qs| qs.status == 1);
                if !active {
                    debug!(
                        account = self.account_id,
                        quest_id = quest.id,
                        prev_id,
                        "CanTakeQuest: negative prev quest not active"
                    );
                    return false;
                }
            }
        }

        // SatisfyQuestRace + SatisfyQuestClass + SatisfyQuestLevel
        quest.is_available_for(
            self.player_race_like_cpp(),
            self.player_class_like_cpp(),
            self.player_level_like_cpp(),
        )
    }

    fn is_quest_disabled_like_cpp(&self, quest_id: u32) -> bool {
        self.disable_mgr().is_some_and(|disable_mgr| {
            disable_mgr.is_disabled_for_like_cpp(DISABLE_TYPE_QUEST, quest_id, None, 0, None)
        })
    }

    /// Save quest status to the characters database.
    async fn save_quest_to_db(&self, quest_id: u32, status: u8) {
        use wow_database::CharStatements;

        let guid = match self.player_guid() {
            Some(g) => g.counter() as u32,
            None => return,
        };
        let char_db = match self.char_db() {
            Some(db) => Arc::clone(db),
            None => return,
        };

        let mut stmt = char_db.prepare(CharStatements::INS_CHAR_QUEST_STATUS);
        stmt.set_u32(0, guid);
        stmt.set_u32(1, quest_id);
        stmt.set_u8(2, status);
        stmt.set_u8(3, status); // ON DUPLICATE KEY UPDATE status
        stmt.set_u8(4, 0); // explored

        if let Err(e) = char_db.execute(&stmt).await {
            warn!(
                account = self.account_id,
                quest_id, "Failed to save quest status: {e}"
            );
        }
    }

    /// Delete a quest from the characters database (abandon).
    async fn delete_quest_from_db(&self, quest_id: u32) {
        use wow_database::CharStatements;

        let guid = match self.player_guid() {
            Some(g) => g.counter() as u32,
            None => return,
        };
        let char_db = match self.char_db() {
            Some(db) => Arc::clone(db),
            None => return,
        };

        let mut stmt = char_db.prepare(CharStatements::DEL_CHAR_QUEST_STATUS);
        stmt.set_u32(0, guid);
        stmt.set_u32(1, quest_id);

        if let Err(e) = char_db.execute(&stmt).await {
            warn!(
                account = self.account_id,
                quest_id, "Failed to delete quest: {e}"
            );
        }
    }

    /// Load all active quests for this player from the characters DB.
    pub(crate) async fn load_player_quests(&mut self) {
        use wow_database::CharStatements;

        let guid = match self.player_guid() {
            Some(g) => g.counter() as u32,
            None => return,
        };
        let char_db = match self.char_db() {
            Some(db) => Arc::clone(db),
            None => return,
        };

        let mut stmt = char_db.prepare(CharStatements::SEL_CHAR_QUEST_STATUS);
        stmt.set_u32(0, guid);

        let result = match char_db.query(&stmt).await {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    account = self.account_id,
                    "Failed to load quest status: {e}"
                );
                return;
            }
        };

        self.player_quests.clear();
        self.rewarded_quests.clear();

        let mut next_active_slot: u8 = 0;

        if !result.is_empty() {
            let mut result = result;
            loop {
                let quest_id: u32 = result.try_read::<u32>(0).unwrap_or(0);
                let status: u8 = result.try_read::<u8>(1).unwrap_or(0);
                let explored: bool = result.try_read::<u8>(2).unwrap_or(0) != 0;

                if status == 3 {
                    // Rewarded (C# QuestStatus.Rewarded / m_RewardedQuests)
                    // Non-repeatable quests cannot be re-taken once rewarded.
                    self.rewarded_quests.insert(quest_id);
                } else if next_active_slot < MAX_QUEST_LOG_SIZE_LIKE_CPP {
                    // Active (1=Incomplete) or complete-but-not-turned-in (2=Complete).
                    // C++ _LoadQuestStatus assigns sequential visible slots in DB row order
                    // because the character DB status row has no persisted quest-log slot.
                    let slot = next_active_slot;
                    next_active_slot = next_active_slot.saturating_add(1);
                    let obj_count = self
                        .quest_store
                        .as_ref()
                        .and_then(|s| s.get(quest_id))
                        .map_or(0, |q| q.objectives.len());
                    self.player_quests.insert(
                        quest_id,
                        PlayerQuestStatus {
                            quest_id,
                            status,
                            explored,
                            objective_counts: vec![0; obj_count],
                            slot,
                        },
                    );
                }

                if !result.next_row() {
                    break;
                }
            }
        }

        let mut seasonal_stmt = char_db.prepare(CharStatements::SEL_CHAR_QUEST_STATUS_SEASONAL);
        seasonal_stmt.set_u32(0, guid);

        let seasonal_rows = match char_db.query(&seasonal_stmt).await {
            Ok(result) => {
                let mut rows = Vec::new();
                if !result.is_empty() {
                    let mut result = result;
                    loop {
                        let quest_id = result.try_read::<u32>(0).unwrap_or_else(|| {
                            warn!(
                                account = self.account_id,
                                "Failed to read seasonal quest id"
                            );
                            0
                        });
                        let event_id = result.try_read::<u32>(1).unwrap_or_else(|| {
                            warn!(
                                account = self.account_id,
                                quest_id, "Failed to read seasonal quest event id"
                            );
                            u32::MAX
                        });
                        let completed_time = result.try_read::<i64>(2).unwrap_or_else(|| {
                            warn!(
                                account = self.account_id,
                                quest_id, event_id, "Failed to read seasonal quest completedTime"
                            );
                            -1
                        });
                        rows.push(SeasonalQuestStatusDbRowLikeCpp {
                            quest_id,
                            event_id,
                            completed_time,
                        });

                        if !result.next_row() {
                            break;
                        }
                    }
                }
                rows
            }
            Err(e) => {
                warn!(
                    account = self.account_id,
                    "Failed to load seasonal quest status: {e}"
                );
                Vec::new()
            }
        };

        let quest_store = self.quest_store.as_ref().map(Arc::clone);
        let quest_v2_store = self.quest_v2_store.as_ref().map(Arc::clone);
        let seasonal_outcome = self.load_seasonal_quest_status_like_cpp(
            seasonal_rows,
            quest_store.as_deref(),
            quest_v2_store.as_deref(),
        );

        if seasonal_outcome.skipped_no_quest_store > 0
            || seasonal_outcome.skipped_missing_quest > 0
            || seasonal_outcome.skipped_event_out_of_range > 0
            || seasonal_outcome.skipped_negative_completed_time > 0
            || seasonal_outcome.completed_bit_skipped_no_quest_v2_store > 0
            || seasonal_outcome.completed_bit_skipped_zero_unique_bit > 0
            || seasonal_outcome.completed_bit_no_change_or_noop > 0
        {
            warn!(
                account = self.account_id,
                rows_seen = seasonal_outcome.rows_seen,
                skipped_no_quest_store = seasonal_outcome.skipped_no_quest_store,
                skipped_missing_quest = seasonal_outcome.skipped_missing_quest,
                skipped_event_out_of_range = seasonal_outcome.skipped_event_out_of_range,
                skipped_negative_completed_time = seasonal_outcome.skipped_negative_completed_time,
                completed_bit_skipped_no_quest_v2_store =
                    seasonal_outcome.completed_bit_skipped_no_quest_v2_store,
                completed_bit_skipped_zero_unique_bit =
                    seasonal_outcome.completed_bit_skipped_zero_unique_bit,
                completed_bit_no_change_or_noop = seasonal_outcome.completed_bit_no_change_or_noop,
                "Skipped seasonal quest status rows during login load"
            );
        }

        info!(
            account = self.account_id,
            active = self.player_quests.len(),
            rewarded = self.rewarded_quests.len(),
            seasonal_inserted = seasonal_outcome.inserted,
            seasonal_replaced = seasonal_outcome.replaced,
            seasonal_completed_bit_set = seasonal_outcome.completed_bit_set,
            seasonal_completed_bit_skipped_no_quest_v2_store =
                seasonal_outcome.completed_bit_skipped_no_quest_v2_store,
            seasonal_completed_bit_skipped_zero_unique_bit =
                seasonal_outcome.completed_bit_skipped_zero_unique_bit,
            seasonal_completed_bit_no_change_or_noop =
                seasonal_outcome.completed_bit_no_change_or_noop,
            "Loaded player quests"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepresentedQuestGiverStatusSourceLikeCpp {
    Creature { entry: u32 },
    GameObject { entry: u32 },
}

impl RepresentedQuestGiverStatusSourceLikeCpp {
    fn entry(self) -> u32 {
        match self {
            Self::Creature { entry } | Self::GameObject { entry } => entry,
        }
    }

    fn kind_name(self) -> &'static str {
        match self {
            Self::Creature { .. } => "Creature",
            Self::GameObject { .. } => "GameObject",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::InventoryItem;
    use wow_constants::{InventoryType, ItemBondingType, ItemClass, ItemContext};
    use wow_core::guid::HighGuid;
    use wow_core::{ObjectGuid, Position};
    use wow_data::quest::{
        QUEST_FLAGS_DAILY_LIKE_CPP, QUEST_ITEM_DROP_COUNT, QUEST_REWARD_CHOICES_COUNT,
        QUEST_REWARD_DISPLAY_SPELL_COUNT, QUEST_REWARD_ITEM_COUNT,
        QUEST_SPECIAL_FLAGS_DF_QUEST_LIKE_CPP, QuestObjective, QuestPoolMemberRowLikeCpp,
        QuestPoolSavedActiveRowLikeCpp, QuestPoolStoreLikeCpp, QuestStore, QuestTemplate,
    };
    use wow_data::{ItemRecord, ItemSparseTemplateEntry, ItemStatsStore, ItemStore};
    use wow_network::{GroupInfo, GroupRegistry, PendingInvites, PlayerRegistry};
    use wow_packet::WorldPacket;
    use wow_packet::packets::item::InventoryChangeFailure;

    fn make_session() -> (WorldSession, flume::Receiver<Vec<u8>>) {
        let (_pkt_tx, pkt_rx) = flume::bounded(8);
        let (send_tx, send_rx) = flume::bounded(8);
        let mut session = WorldSession::new(
            1,
            "QuestStatusTest".into(),
            0,
            2,
            9,
            54261,
            vec![0; 40],
            "enUS".into(),
            pkt_rx,
            send_tx,
        );
        session.set_player_guid(Some(ObjectGuid::create_player(1, 42)));
        session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        session.set_player_position_like_cpp(Position::new(10.0, 0.0, 0.0, 0.0));
        (session, send_rx)
    }

    fn quest_template(id: u32) -> QuestTemplate {
        QuestTemplate {
            id,
            quest_type: 2,
            quest_level: 1,
            quest_max_scaling_level: 0,
            min_level: 1,
            quest_sort_id: 0,
            quest_info_id: 0,
            suggested_group_num: 0,
            reward_next_quest: 0,
            reward_xp_difficulty: 0,
            reward_xp_multiplier: 1.0,
            reward_money_difficulty: 0,
            reward_money_multiplier: 1.0,
            reward_bonus_money: 0,
            reward_display_spell: [0; QUEST_REWARD_DISPLAY_SPELL_COUNT],
            reward_spell: 0,
            reward_honor: 0,
            source_item_id: 0,
            source_item_count: 0,
            source_spell_id: 0,
            expansion: 0,
            flags: 0,
            flags_ex: 0,
            flags_ex2: 0,
            special_flags: 0,
            event_id_for_quest: 0,
            reward_items: [0; QUEST_REWARD_ITEM_COUNT],
            reward_amounts: [0; QUEST_REWARD_ITEM_COUNT],
            item_drop: [0; QUEST_ITEM_DROP_COUNT],
            item_drop_quantity: [0; QUEST_ITEM_DROP_COUNT],
            log_title: format!("Quest {id}"),
            log_description: String::new(),
            quest_description: String::new(),
            area_description: String::new(),
            quest_completion_log: String::new(),
            objectives: Vec::new(),
            allowable_races: 0,
            allowable_classes: 0,
            max_level: 0,
            prev_quest_id: 0,
            next_quest_id: 0,
            exclusive_group: 0,
            breadcrumb_for_quest_id: 0,
            dependent_previous_quests: Vec::new(),
            dependent_breadcrumb_quests: Vec::new(),
            required_min_rep_faction: 0,
            required_min_rep_value: 0,
            required_max_rep_faction: 0,
            required_max_rep_value: 0,
            reward_choice_items: [(0, 0); QUEST_REWARD_CHOICES_COUNT],
        }
    }

    fn store_with_quests(ids: &[u32]) -> QuestStore {
        QuestStore::from_quests_like_cpp(ids.iter().copied().map(quest_template))
    }

    fn quest_template_with_objective_count(id: u32, objective_count: usize) -> QuestTemplate {
        let mut quest = quest_template(id);
        quest.objectives = (0..objective_count)
            .map(|index| QuestObjective {
                id: id * 10 + index as u32,
                quest_id: id,
                obj_type: 0,
                order: index as u8,
                storage_index: index as i8,
                object_id: 1000 + index as i32,
                amount: 1,
                flags: 0,
                flags2: 0,
                progress_bar_weight: 0.0,
                description: String::new(),
            })
            .collect();
        quest
    }

    fn store_with_sharable_quest_objectives(id: u32, objective_count: usize) -> QuestStore {
        let mut quest = quest_template_with_objective_count(id, objective_count);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn quest_template_with_source_item(
        id: u32,
        source_item_id: u32,
        source_item_count: u32,
        source_spell_id: u32,
    ) -> QuestTemplate {
        let mut quest = quest_template(id);
        quest.source_item_id = source_item_id;
        quest.source_item_count = source_item_count;
        quest.source_spell_id = source_spell_id;
        quest
    }

    fn store_with_source_item_quest(
        quest_id: u32,
        source_item_id: u32,
        source_item_count: u32,
        source_spell_id: u32,
    ) -> QuestStore {
        QuestStore::from_quests_like_cpp([quest_template_with_source_item(
            quest_id,
            source_item_id,
            source_item_count,
            source_spell_id,
        )])
    }

    fn install_source_item_template(
        session: &mut WorldSession,
        entry: u32,
        stackable: i32,
        max_count: u32,
    ) {
        install_source_item_template_with_start_quest_and_limit_category(
            session, entry, stackable, max_count, 0, 0,
        );
    }

    fn install_source_item_template_with_start_quest(
        session: &mut WorldSession,
        entry: u32,
        stackable: i32,
        max_count: u32,
        start_quest_id: i32,
    ) {
        install_source_item_template_with_start_quest_and_limit_category(
            session,
            entry,
            stackable,
            max_count,
            start_quest_id,
            0,
        );
    }

    fn install_source_item_template_with_limit_category(
        session: &mut WorldSession,
        entry: u32,
        stackable: i32,
        max_count: u32,
        limit_category: u16,
    ) {
        install_source_item_template_with_start_quest_and_limit_category(
            session,
            entry,
            stackable,
            max_count,
            0,
            limit_category,
        );
    }

    fn install_source_item_template_with_start_quest_and_limit_category(
        session: &mut WorldSession,
        entry: u32,
        stackable: i32,
        max_count: u32,
        start_quest_id: i32,
        limit_category: u16,
    ) {
        session.set_item_store(Arc::new(ItemStore::from_records([ItemRecord {
            id: entry,
            class_id: ItemClass::Consumable as u8,
            subclass_id: 0,
            material: 0,
            inventory_type: InventoryType::NonEquip as i8,
            sheathe_type: 0,
            random_select: 0,
            random_suffix_group_id: 0,
        }])));
        session.set_item_stats_store(Arc::new(ItemStatsStore::from_sparse_templates([(
            entry,
            ItemSparseTemplateEntry {
                flags: [0, 0, 0, 0],
                bag_family: 0,
                start_quest_id,
                stackable,
                max_count: i32::try_from(max_count).unwrap_or(i32::MAX),
                lock_id: 0,
                required_reputation_rank: 0,
                sell_price: 0,
                buy_price: 0,
                vendor_stack_count: 1,
                price_variance: 1.0,
                price_random_value: 1.0,
                max_durability: 0,
                limit_category,
                instance_bound: 0,
                zone_bound: [0, 0],
                required_reputation_faction: 0,
                allowable_class: -1,
                required_expansion: 0,
                bonding: ItemBondingType::None as u8,
                container_slots: 0,
                inventory_type: InventoryType::NonEquip as i8,
            },
        )])));
    }

    fn insert_direct_inventory_item(
        session: &mut WorldSession,
        player_guid: ObjectGuid,
        slot: u8,
        entry: u32,
        count: u32,
        db_guid: u64,
    ) {
        let item_guid = ObjectGuid::create_item(1, db_guid as i64);
        session.insert_inventory_item_like_cpp(
            slot,
            InventoryItem {
                guid: item_guid,
                entry_id: entry,
                db_guid,
                inventory_type: None,
            },
        );
        let item = session.make_inventory_item_object(
            item_guid,
            entry,
            player_guid,
            count,
            0,
            ItemContext::None,
            slot,
        );
        session.insert_inventory_item_object(item);
    }

    #[test]
    fn query_quest_completion_builds_creature_then_masked_go_entries_like_cpp() {
        let mut store = store_with_quests(&[77]);
        store.ender_quests.entry(1234).or_default().push(77);
        store.ender_quests.entry(12).or_default().push(77);
        store
            .gameobject_ender_quests
            .entry(0x5678)
            .or_default()
            .push(77);

        let response = represented_quest_completion_npc_response_like_cpp(&store, &[77]);

        assert_eq!(response.len(), 1);
        assert_eq!(response[0].quest_id, 77);
        assert_eq!(response[0].npcs, vec![12, 1234, 0x8000_5678u32 as i32]);
    }

    #[test]
    fn query_quest_completion_skips_negative_missing_and_oversized_creature_entries_like_cpp() {
        let mut store = store_with_quests(&[5]);
        store
            .ender_quests
            .entry(i32::MAX as u32 + 1)
            .or_default()
            .push(5);
        store
            .gameobject_ender_quests
            .entry(u32::MAX)
            .or_default()
            .push(5);

        let response = represented_quest_completion_npc_response_like_cpp(&store, &[-1, 999, 5]);

        assert_eq!(response.len(), 1);
        assert_eq!(response[0].quest_id, 5);
        assert_eq!(response[0].npcs, vec![-1]);
    }

    fn creature_guid(entry: u32, counter: i64) -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 571, 0, entry, counter)
    }

    fn gameobject_guid(entry: u32, counter: i64) -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::GameObject, 0, 1, 571, 0, entry, counter)
    }

    fn insert_creature(manager: &mut wow_map::MapManager, guid: ObjectGuid, entry: u32) {
        let mut creature = wow_entities::Creature::new(false);
        creature.unit_mut().world_mut().object_mut().create(guid);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .set_entry(entry);
        creature.unit_mut().world_mut().set_map(571, 0).unwrap();
        creature
            .unit_mut()
            .world_mut()
            .relocate(Position::new(10.0, 0.0, 0.0, 0.0));
        creature.unit_mut().set_level(80);
        creature.set_ai_identity_runtime(1, 35, NPCFlags1::QUEST_GIVER.bits(), 0);
        manager
            .create_world_map(571, 0)
            .map_mut()
            .insert_map_object_record(
                wow_entities::MapObjectRecord::new_creature(creature).unwrap(),
            )
            .unwrap();
    }

    fn insert_gameobject(manager: &mut wow_map::MapManager, guid: ObjectGuid, entry: u32) {
        let mut gameobject = wow_entities::GameObject::new();
        gameobject.world_mut().object_mut().create(guid);
        gameobject.world_mut().object_mut().set_entry(entry);
        gameobject.world_mut().set_map(571, 0).unwrap();
        gameobject
            .world_mut()
            .relocate(Position::new(10.0, 0.0, 0.0, 0.0));
        gameobject.world_mut().object_mut().add_to_world();
        manager
            .create_world_map(571, 0)
            .map_mut()
            .insert_map_object_record(
                wow_entities::MapObjectRecord::new_game_object(gameobject).unwrap(),
            )
            .unwrap();
    }

    fn attach_map_manager(session: &mut WorldSession, manager: wow_map::MapManager) {
        session.set_canonical_map_manager(Arc::new(std::sync::Mutex::new(manager)));
    }

    async fn run_status_query(session: &mut WorldSession, guid: ObjectGuid) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&guid);
        session.handle_quest_giver_status_query(pkt).await;
    }

    fn add_active_quest(session: &mut WorldSession, quest_id: u32) {
        let slot = session.first_free_quest_slot_like_cpp().unwrap_or(0);
        add_active_quest_in_slot(session, quest_id, slot);
    }

    fn add_active_quest_in_slot(session: &mut WorldSession, quest_id: u32, slot: u8) {
        add_active_quest_in_slot_with_status(
            session,
            quest_id,
            slot,
            QUEST_STATUS_INCOMPLETE_LIKE_CPP,
        );
    }

    fn add_active_quest_in_slot_with_status(
        session: &mut WorldSession,
        quest_id: u32,
        slot: u8,
        status: u8,
    ) {
        session.player_quests.insert(
            quest_id,
            PlayerQuestStatus {
                quest_id,
                status,
                explored: false,
                objective_counts: Vec::new(),
                slot,
            },
        );
    }

    async fn run_close_quest(session: &mut WorldSession, quest_id: u32) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_uint32(quest_id);
        session.handle_quest_giver_close_quest(pkt).await;
    }

    async fn run_remove_quest_slot(session: &mut WorldSession, slot: u8) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_uint8(slot);
        session.handle_quest_log_remove_quest(pkt).await;
    }

    async fn run_request_world_quest_update(session: &mut WorldSession) {
        session
            .handle_request_world_quest_update(WorldPacket::new_empty())
            .await;
    }

    async fn run_quest_confirm_accept(session: &mut WorldSession, quest_id: i32) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_int32(quest_id);
        session.handle_quest_confirm_accept(pkt).await;
    }

    async fn run_quest_push_result(
        session: &mut WorldSession,
        sender_guid: ObjectGuid,
        quest_id: u32,
        result: u8,
    ) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&sender_guid);
        pkt.write_uint32(quest_id);
        pkt.write_uint8(result);
        session.handle_quest_push_result(pkt).await;
    }

    async fn run_push_quest_to_party(session: &mut WorldSession, quest_id: u32) {
        let mut pkt = WorldPacket::new_empty();
        pkt.write_uint32(quest_id);
        session.handle_push_quest_to_party(pkt).await;
    }

    fn store_with_sharable_quest(id: u32) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn store_with_sharable_quest_levels(id: u32, min_level: i32, max_level: u8) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.min_level = min_level;
        quest.max_level = max_level;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn store_with_sharable_quest_class_race(
        id: u32,
        allowable_classes: u32,
        allowable_races: u64,
    ) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.allowable_classes = allowable_classes;
        quest.allowable_races = allowable_races;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn store_with_sharable_quest_reputation(
        id: u32,
        min_faction: u32,
        min_value: i32,
        max_faction: u32,
        max_value: i32,
    ) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.required_min_rep_faction = min_faction;
        quest.required_min_rep_value = min_value;
        quest.required_max_rep_faction = max_faction;
        quest.required_max_rep_value = max_value;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn store_with_sharable_quest_previous(id: u32, prev_quest_id: i32) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.prev_quest_id = prev_quest_id;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn store_with_daily_sharable_quests(ids: &[u32]) -> QuestStore {
        let quests = ids.iter().map(|id| {
            let mut quest = quest_template(*id);
            quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP | QUEST_FLAGS_DAILY_LIKE_CPP;
            quest
        });
        QuestStore::from_quests_like_cpp(quests)
    }

    fn store_with_df_sharable_quest(id: u32) -> QuestStore {
        let mut quest = quest_template(id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.special_flags |= QUEST_SPECIAL_FLAGS_DF_QUEST_LIKE_CPP;
        QuestStore::from_quests_like_cpp([quest])
    }

    fn quest_pool_store_with_active_saved(
        quest_store: &QuestStore,
        pool_id: u32,
        member_quest_ids: &[u32],
        active_quest_ids: &[u32],
    ) -> QuestPoolStoreLikeCpp {
        QuestPoolStoreLikeCpp::from_rows_like_cpp(
            quest_store,
            member_quest_ids
                .iter()
                .enumerate()
                .map(|(pool_index, quest_id)| QuestPoolMemberRowLikeCpp {
                    quest_id: *quest_id,
                    pool_id,
                    pool_index: pool_index as u32,
                    num_active: Some(active_quest_ids.len() as u32),
                }),
            active_quest_ids
                .iter()
                .map(|quest_id| QuestPoolSavedActiveRowLikeCpp {
                    pool_id,
                    quest_id: *quest_id,
                }),
        )
    }

    fn recv_world_quest_update_count(send_rx: &flume::Receiver<Vec<u8>>) -> u32 {
        let bytes = send_rx
            .try_recv()
            .expect("world quest update response packet");
        assert_eq!(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            wow_constants::ServerOpcodes::WorldQuestUpdateResponse as u16
        );
        let mut pkt = WorldPacket::from_bytes(&bytes[2..]);
        pkt.read_uint32().unwrap()
    }

    fn recv_push_quest_result_response(
        send_rx: &flume::Receiver<Vec<u8>>,
    ) -> (ObjectGuid, u8, String) {
        let bytes = send_rx
            .try_recv()
            .expect("quest push result response packet");
        assert_eq!(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            wow_constants::ServerOpcodes::QuestPushResult as u16
        );
        let mut pkt = WorldPacket::from_bytes(&bytes[2..]);
        let sender_guid = pkt.read_packed_guid().unwrap();
        let result = pkt.read_uint8().unwrap();
        let title_len = pkt.read_bits(9).unwrap() as usize;
        let quest_title = pkt.read_string(title_len).unwrap();
        assert_eq!(pkt.remaining(), 0);
        assert!(send_rx.try_recv().is_err());
        (sender_guid, result, quest_title)
    }

    fn recv_quest_giver_quest_details_contains_quest_id(
        send_rx: &flume::Receiver<Vec<u8>>,
        quest_id: u32,
    ) {
        let bytes = send_rx
            .try_recv()
            .expect("quest giver quest details packet");
        assert_eq!(
            wow_packet::WorldPacket::from_bytes(&bytes).server_opcode(),
            Some(wow_constants::ServerOpcodes::QuestGiverQuestDetails)
        );
        assert!(
            bytes
                .windows(4)
                .any(|window| window == quest_id.to_le_bytes())
        );
        assert!(send_rx.try_recv().is_err());
    }

    fn recv_quest_giver_request_items_like_cpp(
        send_rx: &flume::Receiver<Vec<u8>>,
        quest_id: u32,
    ) -> (Vec<(i32, i32, u32)>, bool) {
        let bytes = send_rx
            .try_recv()
            .expect("quest giver request items packet");
        let mut pkt = wow_packet::WorldPacket::from_bytes(&bytes);
        assert_eq!(
            pkt.server_opcode(),
            Some(wow_constants::ServerOpcodes::QuestGiverRequestItems)
        );
        pkt.skip_opcode();
        let _giver_guid = pkt.read_packed_guid().expect("giver guid");
        let giver_creature_id = pkt.read_int32().expect("giver creature id");
        assert_eq!(pkt.read_int32().expect("quest id"), quest_id as i32);
        let _comp_emote_delay = pkt.read_int32().expect("comp emote delay");
        let _comp_emote_type = pkt.read_int32().expect("comp emote type");
        for _ in 0..3 {
            let _ = pkt.read_uint32().expect("quest flags");
        }
        let _suggested_party_members = pkt.read_int32().expect("suggested party members");
        let _money_to_get = pkt.read_int32().expect("money to get");
        let collect_count = pkt.read_int32().expect("collect count");
        let currency_count = pkt.read_int32().expect("currency count");
        let _status_flags = pkt.read_int32().expect("status flags");
        let mut collect = Vec::new();
        for _ in 0..collect_count {
            collect.push((
                pkt.read_int32().expect("collect object id"),
                pkt.read_int32().expect("collect amount"),
                pkt.read_uint32().expect("collect flags"),
            ));
        }
        for _ in 0..currency_count {
            let _currency_id = pkt.read_int32().expect("currency id");
            let _currency_amount = pkt.read_int32().expect("currency amount");
        }
        let auto_launched = pkt.read_bit().expect("auto launched bit");
        assert_eq!(
            pkt.read_int32().expect("repeated giver creature id"),
            giver_creature_id
        );
        assert_eq!(
            pkt.read_uint32()
                .expect("conditional completion text count"),
            0
        );
        assert!(send_rx.try_recv().is_err());
        (collect, auto_launched)
    }

    fn recv_quest_giver_offer_reward_contains_quest_id(
        send_rx: &flume::Receiver<Vec<u8>>,
        quest_id: u32,
    ) {
        let bytes = send_rx.try_recv().expect("quest giver offer reward packet");
        assert_eq!(
            wow_packet::WorldPacket::from_bytes(&bytes).server_opcode(),
            Some(wow_constants::ServerOpcodes::QuestGiverOfferRewardMessage)
        );
        assert!(
            bytes
                .windows(4)
                .any(|window| window == quest_id.to_le_bytes())
        );
        assert!(send_rx.try_recv().is_err());
    }

    fn assert_success_command_queued_like_cpp(
        sender_rx: &flume::Receiver<Vec<u8>>,
        receiver_rx: &flume::Receiver<Vec<u8>>,
        receiver_session: &mut WorldSession,
        receiver_guid: ObjectGuid,
        sender_guid: ObjectGuid,
        quest_id: u32,
    ) {
        assert_eq!(
            recv_push_quest_result_response(sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_SUCCESS_LIKE_CPP,
                String::new()
            )
        );
        assert!(receiver_rx.try_recv().is_err());
        let commands = receiver_session.drain_session_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            SessionCommand::SetQuestSharingInfoAndSendDetails(command) => {
                assert_eq!(command.sender_guid, sender_guid);
                assert_eq!(command.quest.id, quest_id);
            }
            other => panic!("unexpected session command: {other:?}"),
        }
    }

    fn recv_status(send_rx: &flume::Receiver<Vec<u8>>) -> (ObjectGuid, u64) {
        let bytes = send_rx.try_recv().expect("quest giver status packet");
        assert_eq!(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            wow_constants::ServerOpcodes::QuestGiverStatus as u16
        );
        let mut pkt = WorldPacket::from_bytes(&bytes[2..]);
        let guid = pkt.read_packed_guid().unwrap();
        let status = pkt.read_uint64().unwrap();
        (guid, status)
    }

    fn recv_status_multiple(send_rx: &flume::Receiver<Vec<u8>>) -> Vec<(ObjectGuid, u64)> {
        let bytes = send_rx
            .try_recv()
            .expect("quest giver status multiple packet");
        assert_eq!(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            wow_constants::ServerOpcodes::QuestGiverStatusMultiple as u16
        );
        let mut pkt = WorldPacket::from_bytes(&bytes[2..]);
        let count = pkt.read_int32().unwrap();
        assert!(count >= 0);
        let mut statuses = Vec::new();
        for _ in 0..count {
            statuses.push((pkt.read_packed_guid().unwrap(), pkt.read_uint64().unwrap()));
        }
        statuses
    }

    fn mark_visible(session: &mut WorldSession, guid: ObjectGuid) {
        session.client_visible_guids_like_cpp.insert(guid);
    }

    fn mark_visible_gameobject_questgiver(session: &mut WorldSession, guid: ObjectGuid) {
        let mut state = crate::session::RepresentedGameObjectUseState::default();
        state.go_type = Some(wow_entities::GAMEOBJECT_TYPE_QUESTGIVER as u8);
        session
            .represented_gameobject_use_states
            .insert(guid, state);
        mark_visible(session, guid);
    }

    fn assert_confirm_accept_outcome(
        session: &WorldSession,
        receiver_guid: Option<ObjectGuid>,
        sender_guid: ObjectGuid,
        quest_id: u32,
        raw_quest_id: i32,
        reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp,
    ) {
        let success_boundary = matches!(
            reason,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::AddQuestRuntimeUnrepresented
        );
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid,
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id,
                reason,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: None,
                add_quest_runtime_unrepresented: success_boundary,
                source_spell_unrepresented: false,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
    }

    fn install_confirm_accept_sender_snapshot(
        session: &mut WorldSession,
        sender_guid: ObjectGuid,
        quest_id: u32,
        same_group: bool,
        sender_active_status: Option<u8>,
    ) -> (WorldSession, flume::Receiver<Vec<u8>>) {
        let player_registry = Arc::new(PlayerRegistry::default());
        session.set_player_registry(Arc::clone(&player_registry));
        session.set_loaded_player_name_like_cpp("Receiver".to_string());
        session.register_in_player_registry();

        let (mut sender_session, sender_rx) = make_session();
        sender_session.set_player_guid(Some(sender_guid));
        sender_session.set_loaded_player_name_like_cpp("Sender".to_string());
        sender_session.set_player_registry(player_registry);
        if let Some(status) = sender_active_status {
            add_active_quest_in_slot_with_status(&mut sender_session, quest_id, 0, status);
        }
        sender_session.register_in_player_registry();
        sender_session.sync_player_registry_state_like_cpp();

        let group_registry = Arc::new(GroupRegistry::default());
        let mut group = GroupInfo::new(sender_guid);
        if same_group {
            if let Some(receiver_guid) = session.player_guid() {
                group.add_member(receiver_guid);
            }
        }
        let group_guid = group.group_guid;
        group_registry.insert(group_guid, group);
        session.set_group_registry(group_registry, Arc::new(PendingInvites::default()));

        (sender_session, sender_rx)
    }

    #[tokio::test]
    async fn quest_confirm_accept_short_packet_does_not_clear_pending_state_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 81);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7001);

        session
            .handle_quest_confirm_accept(WorldPacket::from_bytes(&[0x59, 0x1B, 0x00]))
            .await;

        assert_eq!(
            session.represented_pending_quest_sharing_like_cpp(),
            Some(crate::session::RepresentedPendingQuestSharingLikeCpp {
                sender_guid,
                quest_id: 7001,
            })
        );
        assert!(
            session
                .represented_quest_confirm_accepts_like_cpp()
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_no_pending_valid_packet_is_noop_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[7002])));

        run_quest_confirm_accept(&mut session, 7002).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(
            session
                .represented_quest_confirm_accepts_like_cpp()
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_mismatch_preserves_pending_state_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 82);
        session.set_quest_store(Arc::new(store_with_quests(&[7003])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7003);

        run_quest_confirm_accept(&mut session, 7004).await;

        assert_eq!(
            session.represented_pending_quest_sharing_like_cpp(),
            Some(crate::session::RepresentedPendingQuestSharingLikeCpp {
                sender_guid,
                quest_id: 7003,
            })
        );
        assert!(
            session
                .represented_quest_confirm_accepts_like_cpp()
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_match_missing_template_clears_without_evidence_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 83);
        session.set_quest_store(Arc::new(store_with_quests(&[7005])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7006);

        run_quest_confirm_accept(&mut session, 7006).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(
            session
                .represented_quest_confirm_accepts_like_cpp()
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_match_template_records_original_player_missing_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 84);
        let receiver_guid = ObjectGuid::create_player(1, 42);
        session.set_quest_store(Arc::new(store_with_quests(&[7007])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7007);

        run_quest_confirm_accept(&mut session, 7007).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(receiver_guid),
            sender_guid,
            7007,
            7007,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerMissing,
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_negative_raw_id_compares_as_u32_bit_pattern_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 85);
        let quest_id = u32::MAX;
        session.set_quest_store(Arc::new(store_with_quests(&[quest_id])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);

        run_quest_confirm_accept(&mut session, -1).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            quest_id,
            -1,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerMissing,
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_sender_exists_not_same_group_records_not_in_same_raid_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 86);
        session.set_quest_store(Arc::new(store_with_quests(&[7008])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7008);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            7008,
            false,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, 7008).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            7008,
            7008,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::NotInSameRaid,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_same_group_sender_not_active_records_original_not_active_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 87);
        session.set_quest_store(Arc::new(store_with_quests(&[7009])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7009);
        let (_sender_session, sender_rx) =
            install_confirm_accept_sender_snapshot(&mut session, sender_guid, 7009, true, None);

        run_quest_confirm_accept(&mut session, 7009).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            7009,
            7009,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::OriginalPlayerNotActiveQuest,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_same_group_sender_active_can_take_failed_records_receiver_gate_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 88);
        let quest_id = 7010;
        session.set_quest_store(Arc::new(store_with_quests(&[quest_id])));
        session.rewarded_quests.insert(quest_id);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            quest_id,
            quest_id as i32,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanTakeQuestFailed,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_can_take_ok_log_full_records_can_add_log_full_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 89);
        let quest_id = 7011;
        session.set_quest_store(Arc::new(store_with_quests(&[quest_id])));
        for slot in 0..MAX_QUEST_LOG_SIZE_LIKE_CPP {
            add_active_quest_in_slot_with_status(
                &mut session,
                80_000 + u32::from(slot),
                slot,
                QUEST_STATUS_COMPLETE_LIKE_CPP,
            );
        }
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_COMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            quest_id,
            quest_id as i32,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestLogFull,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_no_source_side_effects_adds_local_quest_state_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 90);
        let quest_id = 7012;
        session.set_quest_store(Arc::new(store_with_sharable_quest_objectives(quest_id, 3)));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        let status = session
            .player_quests
            .get(&quest_id)
            .expect("receiver quest log should receive bounded local AddQuest state");
        assert_eq!(status.quest_id, quest_id);
        assert_eq!(status.status, QUEST_STATUS_INCOMPLETE_LIKE_CPP);
        assert!(!status.explored);
        assert_eq!(status.objective_counts, vec![0, 0, 0]);
        assert_eq!(status.slot, 0);
        let registry = session.player_registry().expect("test installs registry");
        let snapshot = registry
            .get(&receiver_guid)
            .expect("receiver snapshot should sync after quest insertion");
        assert_eq!(
            snapshot.active_quest_statuses.get(&quest_id),
            Some(&QUEST_STATUS_INCOMPLETE_LIKE_CPP)
        );
        assert_eq!(
            snapshot.active_quest_objective_counts.get(&quest_id),
            Some(&vec![0, 0, 0])
        );
        assert_confirm_accept_outcome(
            &session,
            Some(receiver_guid),
            sender_guid,
            quest_id,
            quest_id as i32,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverAddQuestLocalStateRepresented,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_first_free_slot_skips_occupied_slot_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 190);
        let occupied_quest_id = 8000;
        let quest_id = 70120;
        session.set_quest_store(Arc::new(store_with_sharable_quest_objectives(quest_id, 1)));
        add_active_quest_in_slot_with_status(
            &mut session,
            occupied_quest_id,
            0,
            QUEST_STATUS_INCOMPLETE_LIKE_CPP,
        );
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        let occupied_status = session
            .player_quests
            .get(&occupied_quest_id)
            .expect("pre-existing quest should remain in slot 0");
        assert_eq!(occupied_status.slot, 0);
        let status = session
            .player_quests
            .get(&quest_id)
            .expect("accepted quest should be inserted into first free slot");
        assert_eq!(status.slot, 1);
        assert_eq!(status.status, QUEST_STATUS_INCOMPLETE_LIKE_CPP);
        let registry = session.player_registry().expect("test installs registry");
        let snapshot = registry
            .get(&receiver_guid)
            .expect("receiver snapshot should sync after quest insertion");
        assert_eq!(
            snapshot.active_quest_statuses.get(&quest_id),
            Some(&QUEST_STATUS_INCOMPLETE_LIKE_CPP)
        );
        assert_confirm_accept_outcome(
            &session,
            Some(receiver_guid),
            sender_guid,
            quest_id,
            quest_id as i32,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverAddQuestLocalStateRepresented,
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_spell_records_two_self_casts_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 191);
        let quest_id = 70121;
        let mut quest = quest_template_with_objective_count(quest_id, 2);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.source_spell_id = 12_345;
        session.set_quest_store(Arc::new(QuestStore::from_quests_like_cpp([quest])));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        let status = session
            .player_quests
            .get(&quest_id)
            .expect("source-spell-only quest should still insert represented local AddQuest state");
        assert_eq!(status.quest_id, quest_id);
        assert_eq!(status.status, QUEST_STATUS_INCOMPLETE_LIKE_CPP);
        assert!(!status.explored);
        assert_eq!(status.objective_counts, vec![0, 0]);
        assert_eq!(status.slot, 0);
        let registry = session.player_registry().expect("test installs registry");
        let snapshot = registry
            .get(&receiver_guid)
            .expect("receiver snapshot should sync after source-spell quest insertion");
        assert_eq!(
            snapshot.active_quest_statuses.get(&quest_id),
            Some(&QUEST_STATUS_INCOMPLETE_LIKE_CPP)
        );
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverAddQuestLocalStateRepresented,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: None,
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: Some(12_345),
                represented_source_spell_self_casts: 2,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_start_quest_no_grant_adds_local_state_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 90);
        let quest_id = 7012;
        let source_item_id = 9000;
        let source_spell_id = 12_344;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            2,
            source_spell_id,
        )));
        install_source_item_template_with_start_quest(
            &mut session,
            source_item_id,
            20,
            0,
            quest_id as i32,
        );
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(session.player_quests.contains_key(&quest_id));
        assert_eq!(
            session
                .represented_inventory_item_counts_like_cpp()
                .get(&source_item_id)
                .copied()
                .unwrap_or(0),
            0
        );
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverGiveQuestSourceItemStartQuestNoGrant,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: Some(InventoryResult::Ok),
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: Some(source_spell_id),
                represented_source_spell_self_casts: 2,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_with_space_records_store_new_item_unrepresented_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 91);
        let quest_id = 7013;
        let source_item_id = 9001;
        let source_spell_id = 12_346;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            2,
            source_spell_id,
        )));
        install_source_item_template(&mut session, source_item_id, 20, 0);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(!session.player_quests.contains_key(&quest_id));
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::GiveQuestSourceItemStoreNewItemUnrepresented,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: Some(InventoryResult::Ok),
                add_quest_runtime_unrepresented: true,
                source_spell_unrepresented: true,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_zero_count_normalizes_to_one_and_fails_full_inventory_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 96);
        let quest_id = 7018;
        let source_item_id = 9005;
        let filler_item_id = 9105;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            0,
            0,
        )));
        install_source_item_template(&mut session, source_item_id, 1, 0);
        for slot in 35..59 {
            insert_direct_inventory_item(
                &mut session,
                receiver_guid,
                slot,
                filler_item_id,
                1,
                91_000 + u64::from(slot),
            );
        }
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(!session.player_quests.contains_key(&quest_id));
        let outcomes = session.represented_quest_confirm_accepts_like_cpp();
        assert_eq!(outcomes.len(), 1);
        let outcome = &outcomes[0];
        assert_eq!(outcome.receiver_guid, Some(receiver_guid));
        assert_eq!(outcome.sender_guid_before_clear, sender_guid);
        assert_eq!(outcome.quest_id, quest_id);
        assert_eq!(outcome.raw_quest_id, quest_id as i32);
        assert_eq!(
            outcome.reason,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemFailed
        );
        assert!(!outcome.add_quest_runtime_unrepresented);
        let source_item_result = outcome
            .can_add_source_item_result
            .expect("zero ProvidedItemCount must normalize to one and reach planner failure");
        assert_ne!(source_item_result, InventoryResult::Ok);
        assert_ne!(source_item_result, InventoryResult::ItemMaxCount);
        assert_eq!(
            send_rx.try_recv().unwrap(),
            InventoryChangeFailure::error(source_item_result).to_bytes()
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_at_max_count_allows_can_add_gate_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 92);
        let quest_id = 7014;
        let source_item_id = 9002;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            1,
            0,
        )));
        install_source_item_template(&mut session, source_item_id, 20, 1);
        insert_direct_inventory_item(&mut session, receiver_guid, 23, source_item_id, 1, 9002);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert!(session.player_quests.contains_key(&quest_id));
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverGiveQuestSourceItemMaxCountNoGrant,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: Some(InventoryResult::ItemMaxCount),
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_missing_source_item_proto_fails_can_add_gate_like_cpp() {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 93);
        let quest_id = 7015;
        let source_item_id = 9003;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            1,
            0,
        )));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason:
                    RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemFailed,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: false,
                can_add_source_item_result: Some(InventoryResult::ItemNotFound),
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
        assert_eq!(
            send_rx.try_recv().unwrap(),
            InventoryChangeFailure::error(InventoryResult::ItemNotFound).to_bytes()
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_limit_category_blocks_at_unrepresented_boundary_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let receiver_guid = session.player_guid().unwrap();
        let sender_guid = ObjectGuid::create_player(1, 95);
        let quest_id = 7017;
        let source_item_id = 9004;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            1,
            0,
        )));
        install_source_item_template_with_limit_category(&mut session, source_item_id, 20, 0, 44);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(!session.player_quests.contains_key(&quest_id));
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(receiver_guid),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemLimitCategoryUnrepresented,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: true,
                can_add_source_item_result: None,
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_source_item_start_quest_still_respects_can_add_gate_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 97);
        let quest_id = 7019;
        let source_item_id = 9006;
        session.set_quest_store(Arc::new(store_with_source_item_quest(
            quest_id,
            source_item_id,
            1,
            0,
        )));
        install_source_item_template_with_start_quest_and_limit_category(
            &mut session,
            source_item_id,
            20,
            0,
            quest_id as i32,
            44,
        );
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(!session.player_quests.contains_key(&quest_id));
        assert_eq!(
            session.represented_quest_confirm_accepts_like_cpp(),
            &[RepresentedQuestConfirmAcceptLikeCpp {
                receiver_guid: Some(ObjectGuid::create_player(1, 42)),
                sender_guid_before_clear: sender_guid,
                quest_id,
                raw_quest_id: quest_id as i32,
                reason: RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverCanAddQuestSourceItemLimitCategoryUnrepresented,
                object_accessor_unrepresented: true,
                party_runtime_unrepresented: true,
                can_add_source_item_unrepresented: true,
                can_add_source_item_result: None,
                add_quest_runtime_unrepresented: false,
                source_spell_unrepresented: false,
                represented_source_spell_id: None,
                represented_source_spell_self_casts: 0,
            }]
        );
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_confirm_accept_without_source_item_does_not_overclaim_source_gate_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 94);
        let quest_id = 7016;
        session.set_quest_store(Arc::new(store_with_source_item_quest(quest_id, 0, 0, 0)));
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, quest_id);
        let (_sender_session, sender_rx) = install_confirm_accept_sender_snapshot(
            &mut session,
            sender_guid,
            quest_id,
            true,
            Some(QUEST_STATUS_INCOMPLETE_LIKE_CPP),
        );

        run_quest_confirm_accept(&mut session, quest_id as i32).await;

        assert_confirm_accept_outcome(
            &session,
            Some(ObjectGuid::create_player(1, 42)),
            sender_guid,
            quest_id,
            quest_id as i32,
            RepresentedQuestConfirmAcceptOutcomeReasonLikeCpp::ReceiverAddQuestLocalStateRepresented,
        );
        assert!(session.player_quests.contains_key(&quest_id));
        assert!(send_rx.try_recv().is_err());
        assert!(sender_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_push_short_packet_does_not_clear_pending_state_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 77);
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7001);

        session
            .handle_quest_push_result(WorldPacket::from_bytes(&[0x00]))
            .await;

        assert_eq!(
            session.represented_pending_quest_sharing_like_cpp(),
            Some(crate::session::RepresentedPendingQuestSharingLikeCpp {
                sender_guid,
                quest_id: 7001,
            })
        );
        assert!(
            session
                .represented_quest_push_result_responses_like_cpp()
                .is_empty()
        );
        assert_eq!(
            session.represented_quest_push_result_sender_mismatch_count_like_cpp(),
            0
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_push_no_pending_valid_packet_is_noop_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 78);

        run_quest_push_result(&mut session, sender_guid, 7002, 3).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(
            session
                .represented_quest_push_result_responses_like_cpp()
                .is_empty()
        );
        assert_eq!(
            session.represented_quest_push_result_sender_mismatch_count_like_cpp(),
            0
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_push_pending_sender_match_clears_and_records_response_evidence_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = ObjectGuid::create_player(1, 79);
        let receiver_guid = session.player_guid().unwrap();
        session.set_represented_pending_quest_sharing_like_cpp(sender_guid, 7003);

        run_quest_push_result(&mut session, sender_guid, 8003, 6).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert_eq!(
            session.represented_quest_push_result_responses_like_cpp(),
            &[RepresentedQuestPushResultResponseLikeCpp {
                receiver_guid,
                sender_guid,
                parsed_quest_id: 8003,
                pending_quest_id: 7003,
                result: 6,
            }]
        );
        assert_eq!(
            session.represented_quest_push_result_sender_mismatch_count_like_cpp(),
            0
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_push_pending_sender_mismatch_clears_without_response_evidence_like_cpp() {
        let (mut session, send_rx) = make_session();
        let pending_sender_guid = ObjectGuid::create_player(1, 80);
        let packet_sender_guid = ObjectGuid::create_player(1, 81);
        session.set_represented_pending_quest_sharing_like_cpp(pending_sender_guid, 7004);

        run_quest_push_result(&mut session, packet_sender_guid, 7004, 4).await;

        assert_eq!(session.represented_pending_quest_sharing_like_cpp(), None);
        assert!(
            session
                .represented_quest_push_result_responses_like_cpp()
                .is_empty()
        );
        assert_eq!(
            session.represented_quest_push_result_sender_mismatch_count_like_cpp(),
            1
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[test]
    fn quest_push_inventory_registration_and_dispatcher_contract_like_cpp() {
        let entry = inventory::iter::<PacketHandlerEntry>
            .into_iter()
            .find(|entry| entry.opcode == ClientOpcodes::QuestPushResult)
            .expect("QuestPushResult handler registration");

        assert_eq!(entry.status, SessionStatus::LoggedIn);
        assert_eq!(entry.processing, PacketProcessing::ThreadUnsafe);
        assert_eq!(entry.handler_name, "handle_quest_push_result");
        assert!(include_str!("../session.rs").contains("ClientOpcodes::QuestPushResult =>"));
        assert!(include_str!("../session.rs").contains("self.handle_quest_push_result(pkt).await"));
    }

    #[test]
    fn quest_push_packet_parser_reads_sender_quest_id_result_in_cpp_order() {
        let sender_guid = ObjectGuid::create_player(1, 82);
        let mut pkt = WorldPacket::new_empty();
        pkt.write_packed_guid(&sender_guid);
        pkt.write_uint32(7005);
        pkt.write_uint8(9);

        let parsed = QuestPushResult::read(&mut pkt).expect("valid QuestPushResult");

        assert_eq!(parsed.sender_guid, sender_guid);
        assert_eq!(parsed.quest_id, 7005);
        assert_eq!(parsed.result, 9);
    }

    #[tokio::test]
    async fn push_quest_to_party_malformed_packet_records_no_evidence_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_sharable_quest(7101)));
        add_active_quest(&mut session, 7101);

        session
            .handle_push_quest_to_party(WorldPacket::from_bytes(&[0x9F, 0x34, 0x00]))
            .await;

        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .is_empty()
        );
        assert!(
            session
                .represented_pending_quest_sharing_like_cpp()
                .is_none()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn push_quest_to_party_missing_quest_template_returns_silently_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[7102])));

        run_push_quest_to_party(&mut session, 7103).await;

        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn push_quest_to_party_unshareable_or_not_in_log_records_not_allowed_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = session.player_guid();
        session.set_quest_store(Arc::new(store_with_sharable_quest(7104)));

        run_push_quest_to_party(&mut session, 7104).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7104,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotAllowed,
                quest_pool_active_check_unrepresented: false,
                group_runtime_unrepresented: false,
                receiver_fanout_unrepresented: false,
            }]
        );
        assert_eq!(
            recv_push_quest_result_response(&send_rx),
            (
                sender_guid.expect("test session has player guid"),
                quest_push_reason::NOT_ALLOWED,
                String::new()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_shareable_sender_without_pool_store_still_blocks_before_group_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let sender_guid = session.player_guid();
        session.set_quest_store(Arc::new(store_with_sharable_quest(7105)));
        add_active_quest(&mut session, 7105);

        run_push_quest_to_party(&mut session, 7105).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7105,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::QuestPoolActiveCheckUnrepresented,
                quest_pool_active_check_unrepresented: true,
                group_runtime_unrepresented: false,
                receiver_fanout_unrepresented: false,
            }]
        );
        assert!(
            session
                .represented_pending_quest_sharing_like_cpp()
                .is_none()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn push_quest_to_party_inactive_pooled_quest_records_not_daily_before_group_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = session.player_guid();
        let quest_store = store_with_daily_sharable_quests(&[7106, 7107]);
        let quest_pool_store =
            quest_pool_store_with_active_saved(&quest_store, 77, &[7106, 7107], &[7107]);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7106);
        session.group_guid = Some(99);

        run_push_quest_to_party(&mut session, 7106).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7106,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotDaily,
                quest_pool_active_check_unrepresented: false,
                group_runtime_unrepresented: false,
                receiver_fanout_unrepresented: false,
            }]
        );
        assert_eq!(
            recv_push_quest_result_response(&send_rx),
            (
                sender_guid.expect("test session has player guid"),
                quest_push_reason::NOT_DAILY,
                String::new()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_active_pooled_quest_passes_pool_check_to_not_in_party_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = session.player_guid();
        let quest_store = store_with_daily_sharable_quests(&[7108, 7109]);
        let quest_pool_store =
            quest_pool_store_with_active_saved(&quest_store, 78, &[7108, 7109], &[7108]);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7108);

        run_push_quest_to_party(&mut session, 7108).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7108,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::NotInParty,
                quest_pool_active_check_unrepresented: false,
                group_runtime_unrepresented: false,
                receiver_fanout_unrepresented: false,
            }]
        );
        assert_eq!(
            recv_push_quest_result_response(&send_rx),
            (
                sender_guid.expect("test session has player guid"),
                quest_push_reason::NOT_IN_PARTY,
                String::new()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_non_pooled_quest_passes_pool_check_to_group_boundary_like_cpp() {
        let (mut session, send_rx) = make_session();
        let sender_guid = session.player_guid();
        let quest_store = store_with_sharable_quest(7110);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7110);
        session.group_guid = Some(99);

        run_push_quest_to_party(&mut session, 7110).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7110,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented,
                quest_pool_active_check_unrepresented: false,
                group_runtime_unrepresented: true,
                receiver_fanout_unrepresented: true,
            }]
        );
        assert!(send_rx.try_recv().is_err());
    }

    fn install_represented_party(
        session: &mut WorldSession,
        sender_guid: ObjectGuid,
        receiver_guid: ObjectGuid,
    ) -> (Arc<PlayerRegistry>, WorldSession, flume::Receiver<Vec<u8>>) {
        let player_registry = Arc::new(PlayerRegistry::default());
        let (mut receiver_session, receiver_rx) = make_session();
        receiver_session.set_player_guid(Some(receiver_guid));
        receiver_session.set_loaded_player_name_like_cpp("Receiver".to_string());
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.set_player_position_like_cpp(Position::new(11.0, 0.0, 0.0, 0.0));
        receiver_session.set_player_registry(Arc::clone(&player_registry));
        receiver_session.register_in_player_registry();

        let group_registry = Arc::new(GroupRegistry::default());
        let mut group = GroupInfo::new(sender_guid);
        group.add_member(receiver_guid);
        let group_guid = group.group_guid;
        group_registry.insert(group_guid, group);

        session.group_guid = Some(group_guid);
        session.set_player_registry(player_registry.clone());
        session.set_group_registry(group_registry, Arc::new(PendingInvites::default()));
        (player_registry, receiver_session, receiver_rx)
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_on_quest_emits_on_quest_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 43);
        let quest_store = store_with_sharable_quest(7111);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7111);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        add_active_quest(&mut receiver_session, 7111);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, 7111).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_ON_QUEST_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_ON_QUEST_TO_RECIPIENT_LIKE_CPP,
                "Quest 7111".to_string()
            )
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_rewarded_emits_already_done_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 44);
        let quest_store = store_with_sharable_quest(7112);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7112);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.rewarded_quests.insert(7112);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, 7112).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7112".to_string()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_log_full_emits_log_full_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 144);
        let shared_quest_id = 7116;
        let quest_store = store_with_sharable_quest(shared_quest_id);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        for slot in 0..MAX_QUEST_LOG_SIZE_LIKE_CPP {
            add_active_quest_in_slot(&mut receiver_session, 8000 + u32::from(slot), slot);
        }
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOG_FULL_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOG_FULL_TO_RECIPIENT_LIKE_CPP,
                "Quest 7116".to_string()
            )
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                ))
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverLogFull
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_daily_completed_emits_already_done_pair_like_cpp()
    {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 244);
        let shared_quest_id = 7117;
        let quest_store = store_with_daily_sharable_quests(&[shared_quest_id]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session
            .set_represented_daily_quest_completed_like_cpp_for_test(shared_quest_id, true);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7117".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                outcome.reason,
                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone
            ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_df_completed_emits_already_done_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 245);
        let shared_quest_id = 7118;
        let quest_store = store_with_df_sharable_quest(shared_quest_id);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_represented_df_quest_like_cpp_for_test(shared_quest_id, true);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7118".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                outcome.reason,
                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone
            ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_non_daily_non_df_ignores_unrelated_daily_snapshot_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 246);
        let shared_quest_id = 7119;
        let quest_store = store_with_sharable_quest(shared_quest_id);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_represented_daily_quest_completed_like_cpp_for_test(9001, true);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                outcome.reason,
                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
            ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                outcome.reason,
                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone
            ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_low_level_emits_low_level_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 248);
        let shared_quest_id = 7121;
        let quest_store = store_with_sharable_quest_levels(shared_quest_id, 10, 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_level_like_cpp(4);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOW_LEVEL_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOW_LEVEL_TO_RECIPIENT_LIKE_CPP,
                "Quest 7121".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMinLevelLowLevel
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_high_level_emits_high_level_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 249);
        let shared_quest_id = 7122;
        let quest_store = store_with_sharable_quest_levels(shared_quest_id, 1, 40);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_level_like_cpp(80);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_HIGH_LEVEL_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_HIGH_LEVEL_TO_RECIPIENT_LIKE_CPP,
                "Quest 7122".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMaxLevelHighLevel
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_receiver_max_level_zero_does_not_block_high_level_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 250);
        let shared_quest_id = 7123;
        let quest_store = store_with_sharable_quest_levels(shared_quest_id, 1, 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_level_like_cpp(80);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMaxLevelHighLevel
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_wrong_class_emits_class_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 251);
        let shared_quest_id = 7124;
        let quest_store = store_with_sharable_quest_class_race(shared_quest_id, 1 << (2 - 1), 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.sync_player_registry_state_like_cpp();
        assert_eq!(
            player_registry
                .get(&receiver_guid)
                .expect("receiver snapshot")
                .class,
            1
        );

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_CLASS_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_CLASS_TO_RECIPIENT_LIKE_CPP,
                "Quest 7124".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestClassWrongClass
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_wrong_race_emits_race_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 252);
        let shared_quest_id = 7125;
        let quest_store = store_with_sharable_quest_class_race(shared_quest_id, 0, 1 << (2 - 1));
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.sync_player_registry_state_like_cpp();
        assert_eq!(
            player_registry
                .get(&receiver_guid)
                .expect("receiver snapshot")
                .race,
            1
        );

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_RACE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_RACE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7125".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestRaceWrongRace
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_receiver_class_precedes_race_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 253);
        let shared_quest_id = 7126;
        let quest_store =
            store_with_sharable_quest_class_race(shared_quest_id, 1 << (2 - 1), 1 << (2 - 1));
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_CLASS_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_CLASS_TO_RECIPIENT_LIKE_CPP,
                "Quest 7126".to_string()
            )
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestRaceWrongRace
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_zero_class_and_race_masks_do_not_block_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 254);
        let shared_quest_id = 7127;
        let quest_store = store_with_sharable_quest_class_race(shared_quest_id, 0, 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestClassWrongClass
                        | RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestRaceWrongRace
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_low_min_reputation_emits_low_faction_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 255);
        let shared_quest_id = 7128;
        let quest_store = store_with_sharable_quest_reputation(shared_quest_id, 72, 100, 0, 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        player_registry
            .get_mut(&receiver_guid)
            .expect("receiver snapshot")
            .reputation_standings = vec![(72, 99)];
        receiver_session.sync_player_registry_state_like_cpp();
        player_registry
            .get_mut(&receiver_guid)
            .expect("receiver snapshot")
            .reputation_standings = vec![(72, 99)];

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOW_FACTION_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOW_FACTION_TO_RECIPIENT_LIKE_CPP,
                "Quest 7128".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationLowFaction)));
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_equal_max_reputation_emits_low_faction_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 256);
        let shared_quest_id = 7129;
        let quest_store = store_with_sharable_quest_reputation(shared_quest_id, 0, 0, 72, 100);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();
        player_registry
            .get_mut(&receiver_guid)
            .expect("receiver snapshot")
            .reputation_standings = vec![(72, 100)];

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOW_FACTION_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOW_FACTION_TO_RECIPIENT_LIKE_CPP,
                "Quest 7129".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationHighFaction)));
    }

    #[tokio::test]
    async fn push_quest_to_party_zero_reputation_factions_do_not_block_with_missing_snapshot_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 257);
        let shared_quest_id = 7130;
        let quest_store = store_with_sharable_quest_reputation(shared_quest_id, 0, 999, 0, -1);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationLowFaction | RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationHighFaction)));
    }

    #[tokio::test]
    async fn push_quest_to_party_positive_prev_missing_rewarded_emits_prerequisite_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 259);
        let shared_quest_id = 7132;
        let quest_store = store_with_sharable_quest_previous(shared_quest_id, 9001);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7132".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
    }

    #[tokio::test]
    async fn push_quest_to_party_positive_prev_rewarded_passes_to_unrepresented_boundary_like_cpp()
    {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 260);
        let shared_quest_id = 7133;
        let quest_store = store_with_sharable_quest_previous(shared_quest_id, 9002);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.rewarded_quests.insert(9002);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_negative_prev_missing_active_incomplete_emits_prerequisite_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 261);
        let shared_quest_id = 7134;
        let quest_store = store_with_sharable_quest_previous(shared_quest_id, -9003);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7134".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_negative_prev_active_incomplete_passes_to_unrepresented_boundary_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 262);
        let shared_quest_id = 7135;
        let quest_store = store_with_sharable_quest_previous(shared_quest_id, -9004);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        add_active_quest(&mut receiver_session, 9004);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_dependent_previous_missing_rewarded_emits_prerequisite_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 470);
        let shared_quest_id = 7607;
        let prev_id = 9607;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut previous_quest = quest_template(prev_id);
        previous_quest.next_quest_id = shared_quest_id;
        previous_quest.exclusive_group = 0;
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, previous_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7607".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_dependent_previous_rewarded_nonnegative_group_passes_to_unrepresented_boundary_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 471);
        let shared_quest_id = 7608;
        let prev_id = 9608;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut previous_quest = quest_template(prev_id);
        previous_quest.next_quest_id = shared_quest_id;
        previous_quest.exclusive_group = 0;
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, previous_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.rewarded_quests.insert(prev_id);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_dependent_previous_negative_exclusive_group_requires_all_other_members_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 472);
        let shared_quest_id = 7609;
        let prev_id = 9609;
        let sibling_id = 9610;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut previous_quest = quest_template(prev_id);
        previous_quest.next_quest_id = shared_quest_id;
        previous_quest.exclusive_group = -90;
        let mut sibling_quest = quest_template(sibling_id);
        sibling_quest.exclusive_group = -90;
        let quest_store =
            QuestStore::from_quests_like_cpp([shared_quest, previous_quest, sibling_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.rewarded_quests.insert(prev_id);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7609".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite)));

        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut previous_quest = quest_template(prev_id);
        previous_quest.next_quest_id = shared_quest_id;
        previous_quest.exclusive_group = -90;
        let mut sibling_quest = quest_template(sibling_id);
        sibling_quest.exclusive_group = -90;
        let quest_store =
            QuestStore::from_quests_like_cpp([shared_quest, previous_quest, sibling_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.rewarded_quests.insert(prev_id);
        receiver_session.rewarded_quests.insert(sibling_id);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_dependent_breadcrumb_active_status_emits_prerequisite_and_absent_passes_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 473);
        let shared_quest_id = 7610;
        let breadcrumb_id = 9611;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut breadcrumb_quest = quest_template(breadcrumb_id);
        breadcrumb_quest.breadcrumb_for_quest_id = shared_quest_id as i32;
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, breadcrumb_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        add_active_quest_in_slot_with_status(
            &mut receiver_session,
            breadcrumb_id,
            2,
            QUEST_STATUS_FAILED_LIKE_CPP,
        );
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7610".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentBreadcrumbQuestsPrerequisite)));

        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        let mut breadcrumb_quest = quest_template(breadcrumb_id);
        breadcrumb_quest.breadcrumb_for_quest_id = shared_quest_id as i32;
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, breadcrumb_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_success_command_queued_like_cpp(
            &sender_rx,
            &receiver_rx,
            &mut receiver_session,
            receiver_guid,
            sender_guid,
            shared_quest_id,
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentBreadcrumbQuestsPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_breadcrumb_for_quest_remains_unrepresented_without_prerequisite_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 474);
        let shared_quest_id = 7611;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        shared_quest.breadcrumb_for_quest_id = 9991;
        let target_quest = quest_template(9991);
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, target_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert!(sender_rx.try_recv().is_err());
        assert!(receiver_rx.try_recv().is_err());
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentBreadcrumbQuestsPrerequisite | RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDependentPreviousQuestsPrerequisite | RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_reputation_precedes_previous_prerequisite_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 263);
        let shared_quest_id = 7136;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.required_min_rep_faction = 72;
        quest.required_min_rep_value = 100;
        quest.prev_quest_id = 9005;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();
        player_registry
            .get_mut(&receiver_guid)
            .expect("receiver snapshot")
            .reputation_standings = vec![(72, 99)];

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOW_FACTION_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOW_FACTION_TO_RECIPIENT_LIKE_CPP,
                "Quest 7136".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationLowFaction)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite)));
    }

    #[tokio::test]
    async fn push_quest_to_party_class_precedes_reputation_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 258);
        let shared_quest_id = 7131;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.allowable_classes = 1 << (2 - 1);
        quest.required_min_rep_faction = 72;
        quest.required_min_rep_value = 100;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_loaded_player_identity_like_cpp(571, 1, 1, 80, 0);
        receiver_session.sync_player_registry_state_like_cpp();
        player_registry
            .get_mut(&receiver_guid)
            .expect("receiver snapshot")
            .reputation_standings = vec![(72, -42000)];

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_CLASS_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_CLASS_TO_RECIPIENT_LIKE_CPP,
                "Quest 7131".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestClassWrongClass)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestReputationLowFaction)));
    }

    #[tokio::test]
    async fn push_quest_to_party_daily_precedes_low_level_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 251);
        let shared_quest_id = 7124;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP | QUEST_FLAGS_DAILY_LIKE_CPP;
        quest.min_level = 80;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_level_like_cpp(1);
        receiver_session
            .set_represented_daily_quest_completed_like_cpp_for_test(shared_quest_id, true);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_ALREADY_DONE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7124".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestMinLevelLowLevel
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_receiver_level_snapshot_syncs_from_world_session_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 252);
        let shared_quest_id = 7125;
        let quest_store = store_with_sharable_quest_levels(shared_quest_id, 20, 0);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        assert_eq!(
            player_registry.get(&receiver_guid).map(|info| info.level),
            Some(80)
        );
        receiver_session.set_player_level_like_cpp(19);
        receiver_session.sync_player_registry_state_like_cpp();
        assert_eq!(
            player_registry.get(&receiver_guid).map(|info| info.level),
            Some(19)
        );

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOW_LEVEL_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOW_LEVEL_TO_RECIPIENT_LIKE_CPP,
                "Quest 7125".to_string()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_log_full_precedes_daily_completed_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 247);
        let shared_quest_id = 7120;
        let quest_store = store_with_daily_sharable_quests(&[shared_quest_id]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        for slot in 0..MAX_QUEST_LOG_SIZE_LIKE_CPP {
            add_active_quest_in_slot(&mut receiver_session, 8100 + u32::from(slot), slot);
        }
        receiver_session
            .set_represented_daily_quest_completed_like_cpp_for_test(shared_quest_id, true);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_LOG_FULL_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_LOG_FULL_TO_RECIPIENT_LIKE_CPP,
                "Quest 7120".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverLogFull
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                outcome.reason,
                RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestDayAlreadyDone
            ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_low_receiver_expansion_emits_expansion_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 248);
        let shared_quest_id = 7121;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.expansion = 2;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.expansion = 1;
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_EXPANSION_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_EXPANSION_TO_RECIPIENT_LIKE_CPP,
                "Quest 7121".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestExpansionRequiredExpansion
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_success_prompts_receiver_details_and_sets_pending_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 249);
        let shared_quest_id = 7122;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.expansion = 2;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.expansion = 2;
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_SUCCESS_LIKE_CPP,
                String::new()
            )
        );
        assert!(receiver_rx.try_recv().is_err());
        let commands = receiver_session.drain_session_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            SessionCommand::SetQuestSharingInfoAndSendDetails(command) => {
                assert_eq!(command.sender_guid, sender_guid);
                assert_eq!(command.quest.id, shared_quest_id);
            }
            other => panic!("unexpected session command: {other:?}"),
        }
        receiver_session
            .session_command_tx()
            .try_send(commands.into_iter().next().expect("command"))
            .expect("requeue command for processing");
        receiver_session
            .process_represented_session_commands_like_cpp()
            .await;
        assert_eq!(
            receiver_session.represented_pending_quest_sharing_like_cpp(),
            Some(crate::session::RepresentedPendingQuestSharingLikeCpp {
                sender_guid,
                quest_id: shared_quest_id,
            })
        );
        recv_quest_giver_quest_details_contains_quest_id(&receiver_rx, shared_quest_id);
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| outcome.target_guid == Some(receiver_guid)
                    && matches!(
                        outcome.reason,
                        RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                    )
                    && !outcome.receiver_fanout_unrepresented)
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestExpansionRequiredExpansion
                        | RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_repeatable_turn_in_success_prompts_request_items_without_pending_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 611);
        let shared_quest_id = 76110;
        let mut quest = quest_template(shared_quest_id);
        quest.quest_type = 0;
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.special_flags |= 0x0000_0001;
        quest.objectives.push(QuestObjective {
            id: 1,
            quest_id: shared_quest_id,
            obj_type: 1,
            order: 0,
            storage_index: 0,
            object_id: 49211,
            amount: 3,
            flags: 0xA5,
            flags2: 0,
            progress_bar_weight: 0.0,
            description: String::new(),
        });
        let quest_for_assertion = quest.clone();
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_SUCCESS_LIKE_CPP,
                String::new()
            )
        );
        assert!(receiver_rx.try_recv().is_err());
        let commands = receiver_session.drain_session_commands();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            SessionCommand::SendRepeatableTurnInRequestItemsLikeCpp(command) => {
                assert_eq!(command.sender_guid, sender_guid);
                assert_eq!(command.quest.id, shared_quest_id);
            }
            other => panic!("unexpected session command: {other:?}"),
        }
        receiver_session
            .session_command_tx()
            .try_send(commands.into_iter().next().expect("command"))
            .expect("requeue command for processing");
        receiver_session
            .process_represented_session_commands_like_cpp()
            .await;

        assert_eq!(
            receiver_session.represented_pending_quest_sharing_like_cpp(),
            None
        );
        let (collect, auto_launched) =
            recv_quest_giver_request_items_like_cpp(&receiver_rx, shared_quest_id);
        assert_eq!(collect, vec![(49211, 3, 0xA5)]);
        assert!(auto_launched);
        assert!(
            !receiver_session
                .can_complete_repeatable_quest_represented_bounded_like_cpp(&quest_for_assertion)
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(
            |outcome| outcome.target_guid == Some(receiver_guid)
                && matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPrompted
                )
                && !outcome.receiver_fanout_unrepresented
        ));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(
            |outcome| outcome.target_guid == Some(receiver_guid)
                && matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted
                )
        ));
    }

    #[tokio::test]
    async fn push_quest_to_party_repeatable_turn_in_command_queue_failure_sends_no_success_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 612);
        let shared_quest_id = 76120;
        let mut quest = quest_template(shared_quest_id);
        quest.quest_type = 0;
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.special_flags |= 0x0000_0001;
        quest.objectives.push(QuestObjective {
            id: 1,
            quest_id: shared_quest_id,
            obj_type: 1,
            order: 0,
            storage_index: 0,
            object_id: 49212,
            amount: 3,
            flags: 0xA5,
            flags2: 0,
            progress_bar_weight: 0.0,
            description: String::new(),
        });
        let quest_for_dummy_commands = quest.clone();
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.sync_player_registry_state_like_cpp();

        for _ in 0..256 {
            receiver_session
                .session_command_tx()
                .try_send(SessionCommand::SetQuestSharingInfoAndSendDetails(
                    SetQuestSharingInfoAndSendDetailsCommand {
                        sender_guid,
                        quest: quest_for_dummy_commands.clone(),
                    },
                ))
                .expect("fill receiver command queue fixture");
        }

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert!(sender_rx.try_recv().is_err());
        assert!(receiver_rx.try_recv().is_err());
        assert_eq!(receiver_session.drain_session_commands().len(), 256);
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(
            |outcome| outcome.target_guid == Some(receiver_guid)
                && matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPromptCommandFailed
                )
                && outcome.receiver_fanout_unrepresented
        ));
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(
            |outcome| outcome.target_guid == Some(sender_guid)
                && matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverEligibilityUnrepresented
                )
                && outcome.receiver_fanout_unrepresented
        ));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(
            |outcome| outcome.target_guid == Some(receiver_guid)
                && matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverRepeatableTurnInRequestItemsPrompted
                )
        ));
    }

    #[tokio::test]
    async fn push_quest_to_party_receiver_unknown_status_after_expansion_emits_invalid_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 609);
        let shared_quest_id = 76090;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.expansion = 2;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.expansion = 2;
        add_active_quest_in_slot_with_status(&mut receiver_session, shared_quest_id, 2, 0xFE);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_INVALID_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_INVALID_TO_RECIPIENT_LIKE_CPP,
                "Quest 76090".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| outcome.target_guid == Some(receiver_guid) && matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverCanTakeQuestInvalid)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| outcome.target_guid == Some(receiver_guid) && matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
    }

    #[tokio::test]
    async fn push_quest_to_party_receiver_positive_exclusive_group_active_peer_emits_invalid_pair_like_cpp()
     {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 610);
        let shared_quest_id = 76091;
        let peer_quest_id = 76092;
        let mut shared_quest = quest_template(shared_quest_id);
        shared_quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        shared_quest.exclusive_group = 609;
        let mut peer_quest = quest_template(peer_quest_id);
        peer_quest.exclusive_group = 609;
        let quest_store = QuestStore::from_quests_like_cpp([shared_quest, peer_quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        add_active_quest(&mut receiver_session, peer_quest_id);
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_INVALID_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_INVALID_TO_RECIPIENT_LIKE_CPP,
                "Quest 76091".to_string()
            )
        );
        assert!(session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| outcome.target_guid == Some(receiver_guid) && matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverCanTakeQuestInvalid)));
        assert!(!session.represented_push_quest_to_party_outcomes_like_cpp().iter().any(|outcome| outcome.target_guid == Some(receiver_guid) && matches!(outcome.reason, RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSuccessQuestDetailsPrompted)));
    }

    #[tokio::test]
    async fn push_quest_to_party_prerequisite_precedes_expansion_gate_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 250);
        let shared_quest_id = 7123;
        let mut quest = quest_template(shared_quest_id);
        quest.flags |= QUEST_FLAGS_SHARABLE_LIKE_CPP;
        quest.prev_quest_id = 9001;
        quest.expansion = 2;
        let quest_store = QuestStore::from_quests_like_cpp([quest]);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, shared_quest_id);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.expansion = 1;
        receiver_session.sync_player_registry_state_like_cpp();

        run_push_quest_to_party(&mut session, shared_quest_id).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_PREREQUISITE_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_PREREQUISITE_TO_RECIPIENT_LIKE_CPP,
                "Quest 7123".to_string()
            )
        );
        assert!(
            session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestPreviousQuestPrerequisite
                ))
        );
        assert!(
            !session
                .represented_push_quest_to_party_outcomes_like_cpp()
                .iter()
                .any(|outcome| matches!(
                    outcome.reason,
                    RepresentedPushQuestToPartyOutcomeReasonLikeCpp::ReceiverSatisfyQuestExpansionRequiredExpansion
                ))
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_busy_emits_sender_only_busy_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 45);
        let quest_store = store_with_sharable_quest(7113);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7113);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session
            .set_represented_pending_quest_sharing_like_cpp(ObjectGuid::create_player(1, 77), 9000);

        run_push_quest_to_party(&mut session, 7113).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_BUSY_LIKE_CPP,
                String::new()
            )
        );
        assert!(receiver_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_dead_emits_dead_pair_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 46);
        let quest_store = store_with_sharable_quest(7114);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7114);
        let (_player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_alive_like_cpp(false);

        run_push_quest_to_party(&mut session, 7114).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_DEAD_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_DEAD_TO_RECIPIENT_LIKE_CPP,
                "Quest 7114".to_string()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_grouped_receiver_dead_observes_runtime_under_map_sync_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid().expect("test sender guid");
        let receiver_guid = ObjectGuid::create_player(1, 146);
        let quest_store = store_with_sharable_quest(7114);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7114);
        let (player_registry, mut receiver_session, receiver_rx) =
            install_represented_party(&mut session, sender_guid, receiver_guid);
        receiver_session.set_player_health_like_cpp(1_000, 1_000);
        let mut movement_info = wow_packet::packets::movement::MovementInfo::default();
        movement_info.position.z = -501.0;

        let event = receiver_session.handle_under_map_like_cpp(&movement_info);

        assert!(event.is_some());
        assert!(!receiver_session.player_is_alive_like_cpp());
        assert!(
            !player_registry
                .get(&receiver_guid)
                .expect("receiver registry snapshot")
                .is_alive
        );

        run_push_quest_to_party(&mut session, 7114).await;

        assert_eq!(
            recv_push_quest_result_response(&sender_rx),
            (
                receiver_guid,
                QUEST_PUSH_REASON_DEAD_LIKE_CPP,
                String::new()
            )
        );
        assert_eq!(
            recv_push_quest_result_response(&receiver_rx),
            (
                sender_guid,
                QUEST_PUSH_REASON_DEAD_TO_RECIPIENT_LIKE_CPP,
                "Quest 7114".to_string()
            )
        );
    }

    #[tokio::test]
    async fn push_quest_to_party_missing_group_registry_keeps_explicit_blocker_like_cpp() {
        let (mut session, sender_rx) = make_session();
        let sender_guid = session.player_guid();
        let quest_store = store_with_sharable_quest(7115);
        let quest_pool_store = QuestPoolStoreLikeCpp::from_rows_like_cpp(&quest_store, [], []);
        session.set_quest_store(Arc::new(quest_store));
        session.set_quest_pool_store(Arc::new(quest_pool_store));
        add_active_quest(&mut session, 7115);
        session.group_guid = Some(1234);

        run_push_quest_to_party(&mut session, 7115).await;

        assert_eq!(
            session.represented_push_quest_to_party_outcomes_like_cpp(),
            &[RepresentedPushQuestToPartyOutcomeLikeCpp {
                sender_guid,
                quest_id: 7115,
                target_guid: sender_guid,
                reason: RepresentedPushQuestToPartyOutcomeReasonLikeCpp::GroupRuntimeUnrepresented,
                quest_pool_active_check_unrepresented: false,
                group_runtime_unrepresented: true,
                receiver_fanout_unrepresented: true,
            }]
        );
        assert!(sender_rx.try_recv().is_err());
    }

    #[test]
    fn push_quest_to_party_registration_and_dispatch_are_wired_like_cpp() {
        let entry = inventory::iter::<PacketHandlerEntry>
            .into_iter()
            .find(|entry| entry.opcode == ClientOpcodes::PushQuestToParty)
            .expect("PushQuestToParty handler registration");

        assert_eq!(entry.status, SessionStatus::LoggedIn);
        assert_eq!(entry.processing, PacketProcessing::ThreadUnsafe);
        assert_eq!(entry.handler_name, "handle_push_quest_to_party");
        assert!(include_str!("../session.rs").contains("ClientOpcodes::PushQuestToParty =>"));
        assert!(
            include_str!("../session.rs").contains("self.handle_push_quest_to_party(pkt).await")
        );
    }

    #[tokio::test]
    async fn request_world_quest_update_empty_payload_sends_empty_response_like_cpp() {
        let (mut session, send_rx) = make_session();

        run_request_world_quest_update(&mut session).await;

        assert_eq!(recv_world_quest_update_count(&send_rx), 0);
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn request_world_quest_update_with_payload_ignores_bytes_and_sends_empty_response_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let mut pkt = WorldPacket::new_empty();
        pkt.write_uint8(1);

        session.handle_request_world_quest_update(pkt).await;

        assert_eq!(recv_world_quest_update_count(&send_rx), 0);
        assert!(send_rx.try_recv().is_err());
    }

    #[test]
    fn request_world_quest_update_inventory_entry_matches_cpp_status_and_processing() {
        let entry = inventory::iter::<PacketHandlerEntry>
            .into_iter()
            .find(|entry| entry.opcode == ClientOpcodes::RequestWorldQuestUpdate)
            .expect("RequestWorldQuestUpdate handler registration");

        assert_eq!(entry.status, SessionStatus::LoggedIn);
        assert_eq!(entry.processing, PacketProcessing::ThreadUnsafe);
        assert_eq!(entry.handler_name, "handle_request_world_quest_update");
    }

    #[tokio::test]
    async fn quest_giver_status_query_missing_noncanonical_guid_sends_no_packet_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[1001])));
        attach_map_manager(&mut session, wow_map::MapManager::default());

        run_status_query(&mut session, creature_guid(9001, 1)).await;

        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_giver_status_query_unsupported_player_or_item_guid_sends_no_packet_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[1001])));
        attach_map_manager(&mut session, wow_map::MapManager::default());

        run_status_query(&mut session, ObjectGuid::create_player(1, 99)).await;
        run_status_query(&mut session, ObjectGuid::create_item(1, 100)).await;

        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_giver_status_query_canonical_creature_starter_sends_available_like_cpp() {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[1001]);
        store.starter_quests.entry(9001).or_default().push(1001);
        session.set_quest_store(Arc::new(store));
        let guid = creature_guid(9001, 1);
        let mut manager = wow_map::MapManager::default();
        insert_creature(&mut manager, guid, 9001);
        attach_map_manager(&mut session, manager);

        run_status_query(&mut session, guid).await;

        assert_eq!(recv_status(&send_rx), (guid, quest_giver_status::AVAILABLE));
    }

    #[tokio::test]
    async fn quest_giver_status_query_canonical_creature_completed_ender_sends_can_reward_like_cpp()
    {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[1002]);
        store.ender_quests.entry(9002).or_default().push(1002);
        session.set_quest_store(Arc::new(store));
        session.player_quests.insert(
            1002,
            PlayerQuestStatus {
                quest_id: 1002,
                status: 2,
                explored: false,
                objective_counts: Vec::new(),
                slot: 0,
            },
        );
        let guid = creature_guid(9002, 2);
        let mut manager = wow_map::MapManager::default();
        insert_creature(&mut manager, guid, 9002);
        attach_map_manager(&mut session, manager);

        run_status_query(&mut session, guid).await;

        assert_eq!(
            recv_status(&send_rx),
            (guid, quest_giver_status::CAN_REWARD)
        );
    }

    #[tokio::test]
    async fn quest_giver_status_query_canonical_gameobject_starter_uses_go_relation_like_cpp() {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[1003]);
        assert!(store.insert_gameobject_starter_relation_like_cpp(9103, 1003));
        session.set_quest_store(Arc::new(store));
        let guid = gameobject_guid(9103, 3);
        let mut manager = wow_map::MapManager::default();
        insert_gameobject(&mut manager, guid, 9103);
        attach_map_manager(&mut session, manager);

        run_status_query(&mut session, guid).await;

        assert_eq!(recv_status(&send_rx), (guid, quest_giver_status::AVAILABLE));
    }

    #[tokio::test]
    async fn quest_giver_status_query_canonical_gameobject_completed_ender_uses_go_relation_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[1004]);
        assert!(store.insert_gameobject_ender_relation_like_cpp(9104, 1004));
        session.set_quest_store(Arc::new(store));
        session.player_quests.insert(
            1004,
            PlayerQuestStatus {
                quest_id: 1004,
                status: 2,
                explored: false,
                objective_counts: Vec::new(),
                slot: 0,
            },
        );
        let guid = gameobject_guid(9104, 4);
        let mut manager = wow_map::MapManager::default();
        insert_gameobject(&mut manager, guid, 9104);
        attach_map_manager(&mut session, manager);

        run_status_query(&mut session, guid).await;

        assert_eq!(
            recv_status(&send_rx),
            (guid, quest_giver_status::CAN_REWARD)
        );
    }

    #[tokio::test]
    async fn quest_giver_status_query_gameobject_ignores_creature_relation_for_same_entry_like_cpp()
    {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[1005]);
        store.starter_quests.entry(9105).or_default().push(1005);
        store.ender_quests.entry(9105).or_default().push(1005);
        session.set_quest_store(Arc::new(store));
        session.player_quests.insert(
            1005,
            PlayerQuestStatus {
                quest_id: 1005,
                status: 2,
                explored: false,
                objective_counts: Vec::new(),
                slot: 0,
            },
        );
        let guid = gameobject_guid(9105, 5);
        let mut manager = wow_map::MapManager::default();
        insert_gameobject(&mut manager, guid, 9105);
        attach_map_manager(&mut session, manager);

        run_status_query(&mut session, guid).await;

        assert_eq!(recv_status(&send_rx), (guid, quest_giver_status::NONE));
    }

    #[tokio::test]
    async fn quest_giver_status_multiple_empty_visible_set_sends_zero_count_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[2001])));
        attach_map_manager(&mut session, wow_map::MapManager::default());

        session.handle_quest_giver_status_multiple_query().await;

        assert!(recv_status_multiple(&send_rx).is_empty());
    }

    #[tokio::test]
    async fn quest_giver_status_multiple_visible_canonical_creature_starter_sends_available_like_cpp()
     {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[2002]);
        store.starter_quests.entry(9202).or_default().push(2002);
        session.set_quest_store(Arc::new(store));
        let guid = creature_guid(9202, 202);
        let mut manager = wow_map::MapManager::default();
        insert_creature(&mut manager, guid, 9202);
        attach_map_manager(&mut session, manager);
        mark_visible(&mut session, guid);

        session.handle_quest_giver_status_multiple_query().await;

        assert_eq!(
            recv_status_multiple(&send_rx),
            vec![(guid, quest_giver_status::AVAILABLE)]
        );
    }

    #[tokio::test]
    async fn quest_giver_status_multiple_visible_gameobject_starter_uses_go_relation_like_cpp() {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[2003]);
        assert!(store.insert_gameobject_starter_relation_like_cpp(9203, 2003));
        store.starter_quests.entry(9203).or_default().push(2999);
        session.set_quest_store(Arc::new(store));
        let guid = gameobject_guid(9203, 203);
        let mut manager = wow_map::MapManager::default();
        insert_gameobject(&mut manager, guid, 9203);
        attach_map_manager(&mut session, manager);
        mark_visible_gameobject_questgiver(&mut session, guid);

        session.handle_quest_giver_status_multiple_query().await;

        assert_eq!(
            recv_status_multiple(&send_rx),
            vec![(guid, quest_giver_status::AVAILABLE)]
        );
    }

    #[tokio::test]
    async fn quest_giver_status_multiple_skips_missing_player_item_and_non_questgiver_go_like_cpp()
    {
        let (mut session, send_rx) = make_session();
        let mut store = store_with_quests(&[2004]);
        store.starter_quests.entry(9204).or_default().push(2004);
        assert!(store.insert_gameobject_starter_relation_like_cpp(9204, 2004));
        session.set_quest_store(Arc::new(store));
        let accepted_guid = creature_guid(9204, 204);
        let missing_guid = creature_guid(9204, 205);
        let player_guid = ObjectGuid::create_player(1, 204);
        let item_guid = ObjectGuid::create_item(1, 204);
        let non_questgiver_go = gameobject_guid(9204, 206);
        let mut manager = wow_map::MapManager::default();
        insert_creature(&mut manager, accepted_guid, 9204);
        insert_gameobject(&mut manager, non_questgiver_go, 9204);
        attach_map_manager(&mut session, manager);
        for guid in [
            accepted_guid,
            missing_guid,
            player_guid,
            item_guid,
            non_questgiver_go,
        ] {
            mark_visible(&mut session, guid);
        }
        let mut state = crate::session::RepresentedGameObjectUseState::default();
        state.go_type = Some(wow_entities::GAMEOBJECT_TYPE_CHEST as u8);
        session
            .represented_gameobject_use_states
            .insert(non_questgiver_go, state);

        session.handle_quest_giver_status_multiple_query().await;

        assert_eq!(
            recv_status_multiple(&send_rx),
            vec![(accepted_guid, quest_giver_status::AVAILABLE)]
        );
    }

    #[tokio::test]
    async fn quest_giver_close_active_existing_template_records_acknowledge_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[5901])));
        add_active_quest(&mut session, 5901);

        run_close_quest(&mut session, 5901).await;

        assert_eq!(
            session.represented_auto_accept_acknowledged_quests_like_cpp,
            vec![5901]
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_giver_close_missing_active_quest_records_no_acknowledge_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[5902])));

        run_close_quest(&mut session, 5902).await;

        assert!(
            session
                .represented_auto_accept_acknowledged_quests_like_cpp
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_giver_close_missing_template_records_no_acknowledge_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[5904])));
        add_active_quest(&mut session, 5903);

        run_close_quest(&mut session, 5903).await;

        assert!(
            session
                .represented_auto_accept_acknowledged_quests_like_cpp
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_giver_close_short_packet_records_no_acknowledge_and_sends_no_packet_like_cpp() {
        let (mut session, send_rx) = make_session();
        session.set_quest_store(Arc::new(store_with_quests(&[5905])));
        add_active_quest(&mut session, 5905);

        session
            .handle_quest_giver_close_quest(WorldPacket::from_bytes(&[0x05, 0x17]))
            .await;

        assert!(
            session
                .represented_auto_accept_acknowledged_quests_like_cpp
                .is_empty()
        );
        assert!(send_rx.try_recv().is_err());
    }

    #[test]
    fn quest_giver_close_inventory_registration_matches_dispatch_contract_like_cpp() {
        let entry = inventory::iter::<PacketHandlerEntry>
            .into_iter()
            .find(|entry| entry.opcode == ClientOpcodes::QuestGiverCloseQuest)
            .expect("QuestGiverCloseQuest handler registration");

        assert_eq!(entry.status, SessionStatus::LoggedIn);
        assert_eq!(entry.processing, PacketProcessing::Inplace);
        assert_eq!(entry.handler_name, "handle_quest_giver_close_quest");
    }

    #[tokio::test]
    async fn quest_log_remove_short_packet_does_not_remove_like_cpp() {
        let (mut session, send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5911, 0);

        session
            .handle_quest_log_remove_quest(WorldPacket::from_bytes(&[]))
            .await;

        assert!(session.player_quests.contains_key(&5911));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(0), Some(5911));
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_log_remove_slot_outside_max_does_not_remove_like_cpp() {
        let (mut session, send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5912, 0);

        run_remove_quest_slot(&mut session, 25).await;

        assert!(session.player_quests.contains_key(&5912));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(0), Some(5912));
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_log_remove_valid_slot_removes_only_that_slot_like_cpp() {
        let (mut session, send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 880_001, 7);
        add_active_quest_in_slot(&mut session, 17, 3);

        run_remove_quest_slot(&mut session, 7).await;

        assert!(!session.player_quests.contains_key(&880_001));
        assert!(session.player_quests.contains_key(&17));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(7), None);
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(3), Some(17));
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_log_remove_empty_valid_slot_does_not_remove_other_quest_like_cpp() {
        let (mut session, send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5914, 4);

        run_remove_quest_slot(&mut session, 3).await;

        assert!(session.player_quests.contains_key(&5914));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(4), Some(5914));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(3), None);
        assert!(send_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quest_log_remove_duplicate_slot_fails_closed_and_removes_none_like_cpp() {
        let (mut session, send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5915, 2);
        add_active_quest_in_slot(&mut session, 5916, 2);

        run_remove_quest_slot(&mut session, 2).await;

        assert!(session.player_quests.contains_key(&5915));
        assert!(session.player_quests.contains_key(&5916));
        assert_eq!(session.get_quest_slot_quest_id_like_cpp(2), None);
        assert_eq!(session.first_free_quest_slot_like_cpp(), Some(0));
        assert!(send_rx.try_recv().is_err());
    }

    #[test]
    fn quest_log_create_entries_preserve_explicit_slot_holes_like_cpp() {
        let (mut session, _send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5916, 9);
        add_active_quest_in_slot(&mut session, 5915, 2);

        let entries = session.quest_log_create_entries_like_cpp();

        assert_eq!(entries.len(), MAX_QUEST_LOG_SIZE_LIKE_CPP as usize);
        assert_eq!(entries[0], (0, 0, 0, [0; 24]));
        assert_eq!(entries[2].0, 5915);
        assert_eq!(entries[9].0, 5916);
    }

    #[test]
    fn quest_log_create_entries_duplicate_slot_is_empty_fail_closed_like_cpp() {
        let (mut session, _send_rx) = make_session();
        add_active_quest_in_slot(&mut session, 5915, 2);
        add_active_quest_in_slot(&mut session, 5916, 2);

        let entries = session.quest_log_create_entries_like_cpp();

        assert_eq!(entries.len(), MAX_QUEST_LOG_SIZE_LIKE_CPP as usize);
        assert_eq!(entries[2], (0, 0, 0, [0; 24]));
        assert!(session.player_quests.contains_key(&5915));
        assert!(session.player_quests.contains_key(&5916));
    }

    #[test]
    fn quest_log_remove_inventory_registration_and_dispatcher_contract_like_cpp() {
        let entry = inventory::iter::<PacketHandlerEntry>
            .into_iter()
            .find(|entry| entry.opcode == ClientOpcodes::QuestLogRemoveQuest)
            .expect("QuestLogRemoveQuest handler registration");

        assert_eq!(entry.status, SessionStatus::LoggedIn);
        assert_eq!(entry.processing, PacketProcessing::Inplace);
        assert_eq!(entry.handler_name, "handle_quest_log_remove_quest");
        assert!(include_str!("../session.rs").contains("ClientOpcodes::QuestLogRemoveQuest =>"));
        assert!(
            include_str!("../session.rs").contains("self.handle_quest_log_remove_quest(pkt).await")
        );
    }
}

// ── PlayerQuestStatus ────────────────────────────────────────────────────────

/// Tracks one active quest for a player.
#[derive(Debug, Clone)]
pub struct PlayerQuestStatus {
    pub quest_id: u32,
    /// 0=None, 1=Incomplete, 2=Complete, 3=Failed
    pub status: u8,
    pub explored: bool,
    /// Progress per objective (indexed by objective.storage_index).
    /// value = current count toward the required amount.
    pub objective_counts: Vec<i32>,
    /// Represented TrinityCore QuestStatusData::Slot / ActivePlayerData::QuestLog index.
    pub slot: u8,
}
