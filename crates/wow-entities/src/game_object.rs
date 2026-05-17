use wow_constants::{TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

use crate::{
    CreateObjectFlags, ObjectChangedFields, ObjectDataUpdate, UpdateMask, WorldObject,
    update_fields::{GAME_OBJECT_DATA_BITS, TYPEID_GAME_OBJECT},
};

pub const DEFAULT_GAMEOBJECT_RESPAWN_DELAY_SECS: u32 = 300;
pub const GAMEOBJECT_LOOT_MODE_DEFAULT: u16 = 0x1;
pub const GAMEOBJECT_TYPE_DOOR: u32 = 0;
pub const GAMEOBJECT_TYPE_BUTTON: u32 = 1;
pub const GAMEOBJECT_TYPE_QUESTGIVER: u32 = 2;
pub const GAMEOBJECT_TYPE_CHEST: u32 = 3;
pub const GAMEOBJECT_TYPE_BINDER: u32 = 4;
pub const GAMEOBJECT_TYPE_GENERIC: u32 = 5;
pub const GAMEOBJECT_TYPE_TRAP: u32 = 6;
pub const GAMEOBJECT_TYPE_CHAIR: u32 = 7;
pub const GAMEOBJECT_TYPE_SPELL_FOCUS: u32 = 8;
pub const GAMEOBJECT_TYPE_TEXT: u32 = 9;
pub const GAMEOBJECT_TYPE_GOOBER: u32 = 10;
pub const GAMEOBJECT_TYPE_AREADAMAGE: u32 = 12;
pub const GAMEOBJECT_TYPE_CAMERA: u32 = 13;
pub const GAMEOBJECT_TYPE_MAP_OBJECT: u32 = 14;
pub const GAMEOBJECT_TYPE_FISHING_NODE: u32 = 17;
pub const GAMEOBJECT_TYPE_RITUAL: u32 = 18;
pub const GAMEOBJECT_TYPE_MAILBOX: u32 = 19;
pub const GAMEOBJECT_TYPE_SPELLCASTER: u32 = 22;
pub const GAMEOBJECT_TYPE_MEETINGSTONE: u32 = 23;
pub const GAMEOBJECT_TYPE_FLAGSTAND: u32 = 24;
pub const GAMEOBJECT_TYPE_FISHING_HOLE: u32 = 25;
pub const GAMEOBJECT_TYPE_FLAGDROP: u32 = 26;
pub const GAMEOBJECT_TYPE_MINI_GAME: u32 = 27;
pub const GAMEOBJECT_TYPE_AURA_GENERATOR: u32 = 30;
pub const GAMEOBJECT_TYPE_DUNGEON_DIFFICULTY: u32 = 31;
pub const GAMEOBJECT_TYPE_BARBER_CHAIR: u32 = 32;
pub const GAMEOBJECT_TYPE_DESTRUCTIBLE_BUILDING: u32 = 33;
pub const GAMEOBJECT_TYPE_GUILD_BANK: u32 = 34;
pub const GAMEOBJECT_TYPE_NEW_FLAG: u32 = 36;
pub const GAMEOBJECT_TYPE_NEW_FLAG_DROP: u32 = 37;
pub const GAMEOBJECT_TYPE_CAPTURE_POINT: u32 = 42;
pub const GAMEOBJECT_TYPE_ITEM_FORGE: u32 = 47;
pub const GAMEOBJECT_TYPE_UI_LINK: u32 = 48;
pub const GAMEOBJECT_TYPE_GATHERING_NODE: u32 = 50;

pub const GO_DYNFLAG_LO_NO_INTERACT: u32 = 0x0080;

pub const MAX_GAMEOBJECT_DATA: usize = 35;
pub const GAMEOBJECT_DATA_CHEST_LOOT: usize = 1;
pub const GAMEOBJECT_DATA_CHEST_RESTOCK_TIME: usize = 2;
pub const GAMEOBJECT_DATA_CHEST_CONSUMABLE: usize = 3;
pub const GAMEOBJECT_DATA_CHEST_TRIGGERED_EVENT: usize = 6;
pub const GAMEOBJECT_DATA_CHEST_LINKED_TRAP: usize = 7;
pub const GAMEOBJECT_DATA_CHEST_USE_GROUP_LOOT_RULES: usize = 15;
pub const GAMEOBJECT_DATA_CHEST_DUNGEON_ENCOUNTER: usize = 25;
pub const GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT: usize = 30;
pub const GAMEOBJECT_DATA_CHEST_PUSH_LOOT: usize = 33;
pub const GAMEOBJECT_DATA_GATHERING_NODE_DESPAWN_DELAY: usize = 6;
pub const GAMEOBJECT_DATA_GATHERING_NODE_TRIGGERED_EVENT: usize = 7;
pub const GAMEOBJECT_DATA_GATHERING_NODE_XP_DIFFICULTY: usize = 13;
pub const GAMEOBJECT_DATA_GATHERING_NODE_SPELL: usize = 14;
pub const GAMEOBJECT_DATA_GATHERING_NODE_MAX_LOOTS: usize = 18;
pub const GAMEOBJECT_DATA_GATHERING_NODE_LINKED_TRAP: usize = 20;

pub const GO_FLAG_IN_USE: u32 = 0x0000_0001;
pub const GO_FLAG_IN_MULTI_USE: u32 = 0x0020_0000;
pub const GAME_OBJECT_DATA_PARENT_BIT: usize = 0;
pub const GAME_OBJECT_DATA_DISPLAY_ID_BIT: usize = 4;
pub const GAME_OBJECT_DATA_CREATED_BY_BIT: usize = 9;
pub const GAME_OBJECT_DATA_FLAGS_BIT: usize = 11;
pub const GAME_OBJECT_DATA_FACTION_TEMPLATE_BIT: usize = 13;
pub const GAME_OBJECT_DATA_LEVEL_BIT: usize = 14;
pub const GAME_OBJECT_DATA_STATE_BIT: usize = 15;
pub const GAME_OBJECT_DATA_TYPE_ID_BIT: usize = 16;
pub const GAME_OBJECT_DATA_PERCENT_HEALTH_BIT: usize = 17;
pub const GAME_OBJECT_DATA_ART_KIT_BIT: usize = 18;
pub const GAME_OBJECT_DATA_CUSTOM_PARAM_BIT: usize = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum GoState {
    Active = 0,
    Ready = 1,
    Destroyed = 2,
    TransportActive = 24,
    TransportStopped = 25,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LootState {
    NotReady = 0,
    Ready = 1,
    Activated = 2,
    JustDeactivated = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameObjectLootSource {
    pub loot_id: u32,
    pub use_group_loot_rules: bool,
    pub dungeon_encounter_id: u32,
    pub personal_loot_id: u32,
    pub push_loot_id: u32,
    pub triggered_event_id: u32,
    pub linked_trap_entry: u32,
    pub chest_restock_time_secs: u32,
    pub chest_consumable: bool,
}

impl GameObjectLootSource {
    pub const fn is_empty(&self) -> bool {
        self.loot_id == 0 && self.personal_loot_id == 0 && self.push_loot_id == 0
    }

    pub const fn open_loot_id_like_cpp(&self) -> u32 {
        if self.loot_id != 0 {
            self.loot_id
        } else {
            self.personal_loot_id
        }
    }

    pub const fn has_open_loot_like_cpp(&self) -> bool {
        self.open_loot_id_like_cpp() != 0
    }

    pub const fn is_personal_encounter_loot_like_cpp(&self) -> bool {
        self.loot_id == 0 && self.personal_loot_id != 0 && self.dungeon_encounter_id != 0
    }

    pub const fn should_autostore_push_loot_like_cpp(&self) -> bool {
        self.loot_id == 0 && self.push_loot_id != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameObjectTemplateData {
    pub go_type: u32,
    pub data: [u32; MAX_GAMEOBJECT_DATA],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GatheringNodeUseSource {
    pub loot_id: u32,
    pub despawn_delay_secs: u32,
    pub triggered_event_id: u32,
    pub xp_difficulty: u32,
    pub spell_id: u32,
    pub max_loots: u32,
    pub linked_trap_entry: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrapUseSource {
    pub spell_id: u32,
    pub charges: u32,
    pub cooldown_secs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChairUseSource {
    pub chair_slots: u32,
    pub chair_height: u32,
    pub triggered_event_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BarberChairUseSource {
    pub chair_height: u32,
    pub sit_anim_kit: u32,
    pub customization_scope: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiLinkUseSource {
    pub ui_link_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemForgeUseSource {
    pub condition_id: u32,
    pub forge_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapturePointUseSource {
    pub capture_time_ms: u32,
    pub world_state_id: u32,
    pub contested_event_horde: u32,
    pub contested_event_alliance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlagStandUseSource {
    pub pickup_spell_id: u32,
    pub return_aura_id: u32,
    pub return_spell_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlagDropUseSource {
    pub event_id: u32,
    pub pickup_spell_id: u32,
    pub expire_duration_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NewFlagUseSource {
    pub pickup_spell_id: u32,
    pub expire_duration_ms: u32,
    pub respawn_time_ms: u32,
    pub flag_drop_entry: u32,
    pub exclusive_category: i32,
    pub world_state_id: u32,
    pub return_on_defender_interact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NewFlagDropUseSource {
    pub spawn_vignette_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RitualUseSource {
    pub casters_required: u32,
    pub spell_id: u32,
    pub anim_spell_id: u32,
    pub persistent: bool,
    pub caster_target_spell_id: u32,
    pub caster_target_spell_targets: u32,
    pub casters_grouped: bool,
    pub no_target_check: bool,
    pub allow_unfriendly_cross_faction_party: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeetingStoneUseSource {
    pub area_id: u32,
    pub prevent_unfriendly_outside_instances: bool,
    pub content_tuning_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuestgiverUseSource {
    pub gossip_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpellcasterUseSource {
    pub spell_id: u32,
    pub charges: u32,
    pub party_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CameraUseSource {
    pub cinematic_id: u32,
    pub event_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GooberUseSource {
    pub lock_id: u32,
    pub quest_id: u32,
    pub event_id: u32,
    pub auto_close_ms: u32,
    pub custom_anim: u32,
    pub consumable: bool,
    pub page_id: u32,
    pub spell_id: u32,
    pub linked_trap_entry: u32,
    pub gossip_id: u32,
    pub allow_multi_interact: bool,
    pub player_cast: bool,
}

impl GameObjectTemplateData {
    pub const fn new(go_type: u32, data: [u32; MAX_GAMEOBJECT_DATA]) -> Self {
        Self { go_type, data }
    }

    pub const fn get_loot_id_like_cpp(&self) -> u32 {
        match self.go_type {
            GAMEOBJECT_TYPE_CHEST
            | GAMEOBJECT_TYPE_FISHING_HOLE
            | GAMEOBJECT_TYPE_GATHERING_NODE => self.data[GAMEOBJECT_DATA_CHEST_LOOT],
            _ => 0,
        }
    }

    pub const fn get_condition_id1_like_cpp(&self) -> u32 {
        let index = match self.go_type {
            GAMEOBJECT_TYPE_DOOR => 7,
            GAMEOBJECT_TYPE_BUTTON => 9,
            GAMEOBJECT_TYPE_QUESTGIVER => 10,
            GAMEOBJECT_TYPE_CHEST => 17,
            GAMEOBJECT_TYPE_GENERIC => 6,
            GAMEOBJECT_TYPE_TRAP => 15,
            GAMEOBJECT_TYPE_CHAIR => 4,
            GAMEOBJECT_TYPE_SPELL_FOCUS => 8,
            GAMEOBJECT_TYPE_TEXT => 4,
            GAMEOBJECT_TYPE_GOOBER => 22,
            GAMEOBJECT_TYPE_CAMERA => 4,
            GAMEOBJECT_TYPE_RITUAL => 8,
            GAMEOBJECT_TYPE_MAILBOX => 0,
            GAMEOBJECT_TYPE_SPELLCASTER => 5,
            GAMEOBJECT_TYPE_FLAGSTAND => 8,
            GAMEOBJECT_TYPE_AURA_GENERATOR => 3,
            GAMEOBJECT_TYPE_GUILD_BANK => 0,
            GAMEOBJECT_TYPE_NEW_FLAG => 4,
            GAMEOBJECT_TYPE_ITEM_FORGE => 0,
            GAMEOBJECT_TYPE_GATHERING_NODE => 11,
            _ => return 0,
        };
        self.data[index]
    }

    pub const fn get_interact_radius_override_like_cpp(&self) -> u32 {
        let index = match self.go_type {
            GAMEOBJECT_TYPE_DOOR => 12,
            GAMEOBJECT_TYPE_BUTTON => 10,
            GAMEOBJECT_TYPE_QUESTGIVER => 12,
            GAMEOBJECT_TYPE_CHEST => 9,
            GAMEOBJECT_TYPE_BINDER => 0,
            GAMEOBJECT_TYPE_GENERIC => 9,
            GAMEOBJECT_TYPE_TRAP => 21,
            GAMEOBJECT_TYPE_CHAIR => 5,
            GAMEOBJECT_TYPE_SPELL_FOCUS => 9,
            GAMEOBJECT_TYPE_TEXT => 6,
            GAMEOBJECT_TYPE_GOOBER => 33,
            GAMEOBJECT_TYPE_AREADAMAGE => 8,
            GAMEOBJECT_TYPE_CAMERA => 5,
            GAMEOBJECT_TYPE_FISHING_NODE => 0,
            GAMEOBJECT_TYPE_RITUAL => 9,
            GAMEOBJECT_TYPE_MAILBOX => 1,
            GAMEOBJECT_TYPE_SPELLCASTER => 8,
            GAMEOBJECT_TYPE_MEETINGSTONE => 3,
            GAMEOBJECT_TYPE_FLAGSTAND => 13,
            GAMEOBJECT_TYPE_FISHING_HOLE => 5,
            GAMEOBJECT_TYPE_FLAGDROP => 10,
            GAMEOBJECT_TYPE_AURA_GENERATOR => 7,
            GAMEOBJECT_TYPE_DUNGEON_DIFFICULTY => 11,
            GAMEOBJECT_TYPE_BARBER_CHAIR => 3,
            GAMEOBJECT_TYPE_DESTRUCTIBLE_BUILDING => 27,
            GAMEOBJECT_TYPE_GUILD_BANK => 1,
            GAMEOBJECT_TYPE_NEW_FLAG => 14,
            GAMEOBJECT_TYPE_NEW_FLAG_DROP => 2,
            GAMEOBJECT_TYPE_GATHERING_NODE => 24,
            _ => return 0,
        };
        self.data[index]
    }

    pub const fn get_lock_id_like_cpp(&self) -> u32 {
        let index = match self.go_type {
            GAMEOBJECT_TYPE_DOOR => 1,
            GAMEOBJECT_TYPE_BUTTON => 1,
            GAMEOBJECT_TYPE_QUESTGIVER => 0,
            GAMEOBJECT_TYPE_CHEST => 0,
            GAMEOBJECT_TYPE_TRAP => 0,
            GAMEOBJECT_TYPE_GOOBER => 0,
            GAMEOBJECT_TYPE_AREADAMAGE => 0,
            GAMEOBJECT_TYPE_CAMERA => 0,
            GAMEOBJECT_TYPE_FLAGSTAND => 0,
            GAMEOBJECT_TYPE_FISHING_HOLE => 4,
            GAMEOBJECT_TYPE_FLAGDROP => 0,
            GAMEOBJECT_TYPE_NEW_FLAG => 0,
            GAMEOBJECT_TYPE_NEW_FLAG_DROP => 0,
            GAMEOBJECT_TYPE_GATHERING_NODE => 3,
            _ => return 0,
        };
        self.data[index]
    }

    pub const fn is_usable_mounted_like_cpp(&self) -> bool {
        let index = match self.go_type {
            GAMEOBJECT_TYPE_MAILBOX => return true,
            GAMEOBJECT_TYPE_BARBER_CHAIR => return false,
            GAMEOBJECT_TYPE_QUESTGIVER => 8,
            GAMEOBJECT_TYPE_TEXT => 3,
            GAMEOBJECT_TYPE_GOOBER => 17,
            GAMEOBJECT_TYPE_SPELLCASTER => 3,
            GAMEOBJECT_TYPE_UI_LINK => 1,
            _ => return false,
        };

        self.data[index] != 0
    }

    pub const fn get_no_damage_immune_like_cpp(&self) -> u32 {
        let index = match self.go_type {
            GAMEOBJECT_TYPE_DOOR => 3,
            GAMEOBJECT_TYPE_BUTTON => 4,
            GAMEOBJECT_TYPE_QUESTGIVER => 5,
            GAMEOBJECT_TYPE_CHEST => {
                return if self.data[22] == 0 { 1 } else { 0 };
            }
            GAMEOBJECT_TYPE_GOOBER => 11,
            GAMEOBJECT_TYPE_FLAGSTAND => 5,
            GAMEOBJECT_TYPE_FLAGDROP => 3,
            _ => return 0,
        };

        self.data[index]
    }

    pub const fn get_cooldown_like_cpp(&self) -> u32 {
        match self.go_type {
            GAMEOBJECT_TYPE_TRAP => self.data[5],
            GAMEOBJECT_TYPE_GOOBER => self.data[6],
            _ => 0,
        }
    }

    pub const fn get_auto_close_time_like_cpp(&self) -> u32 {
        match self.go_type {
            GAMEOBJECT_TYPE_DOOR | GAMEOBJECT_TYPE_BUTTON => self.data[2],
            GAMEOBJECT_TYPE_TRAP => self.data[6],
            GAMEOBJECT_TYPE_GOOBER => self.data[3],
            _ => 0,
        }
    }

    pub const fn trap_use_source_like_cpp(&self) -> Option<TrapUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_TRAP {
            return None;
        }

        Some(TrapUseSource {
            spell_id: self.data[3],
            charges: self.data[4],
            cooldown_secs: self.data[5],
        })
    }

    pub const fn chair_use_source_like_cpp(&self) -> Option<ChairUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_CHAIR {
            return None;
        }

        Some(ChairUseSource {
            chair_slots: self.data[0],
            chair_height: self.data[1],
            triggered_event_id: self.data[3],
        })
    }

    pub const fn barber_chair_use_source_like_cpp(&self) -> Option<BarberChairUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_BARBER_CHAIR {
            return None;
        }

        Some(BarberChairUseSource {
            chair_height: self.data[0],
            sit_anim_kit: self.data[2],
            customization_scope: self.data[4],
        })
    }

    pub const fn ui_link_use_source_like_cpp(&self) -> Option<UiLinkUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_UI_LINK {
            return None;
        }

        Some(UiLinkUseSource {
            ui_link_type: self.data[0],
        })
    }

    pub const fn item_forge_use_source_like_cpp(&self) -> Option<ItemForgeUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_ITEM_FORGE {
            return None;
        }

        Some(ItemForgeUseSource {
            condition_id: self.data[0],
            forge_type: self.data[5],
        })
    }

    pub const fn capture_point_use_source_like_cpp(&self) -> Option<CapturePointUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_CAPTURE_POINT {
            return None;
        }

        Some(CapturePointUseSource {
            capture_time_ms: self.data[0],
            world_state_id: self.data[10],
            contested_event_horde: self.data[11],
            contested_event_alliance: self.data[14],
        })
    }

    pub const fn flag_stand_use_source_like_cpp(&self) -> Option<FlagStandUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_FLAGSTAND {
            return None;
        }

        Some(FlagStandUseSource {
            pickup_spell_id: self.data[1],
            return_aura_id: self.data[3],
            return_spell_id: self.data[4],
        })
    }

    pub const fn flag_drop_use_source_like_cpp(&self) -> Option<FlagDropUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_FLAGDROP {
            return None;
        }

        Some(FlagDropUseSource {
            event_id: self.data[1],
            pickup_spell_id: self.data[2],
            expire_duration_ms: self.data[6],
        })
    }

    pub const fn questgiver_use_source_like_cpp(&self) -> Option<QuestgiverUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_QUESTGIVER {
            return None;
        }

        Some(QuestgiverUseSource {
            gossip_id: self.data[3],
        })
    }

    pub const fn ritual_use_source_like_cpp(&self) -> Option<RitualUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_RITUAL {
            return None;
        }

        Some(RitualUseSource {
            casters_required: self.data[0],
            spell_id: self.data[1],
            anim_spell_id: self.data[2],
            persistent: self.data[3] != 0,
            caster_target_spell_id: self.data[4],
            caster_target_spell_targets: self.data[5],
            casters_grouped: self.data[6] != 0,
            no_target_check: self.data[7] != 0,
            allow_unfriendly_cross_faction_party: self.data[10] != 0,
        })
    }

    pub const fn meeting_stone_use_source_like_cpp(&self) -> Option<MeetingStoneUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_MEETINGSTONE {
            return None;
        }

        Some(MeetingStoneUseSource {
            area_id: self.data[2],
            prevent_unfriendly_outside_instances: self.data[4] != 0,
            content_tuning_id: 0,
        })
    }

    pub const fn new_flag_use_source_like_cpp(&self) -> Option<NewFlagUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_NEW_FLAG {
            return None;
        }

        Some(NewFlagUseSource {
            pickup_spell_id: self.data[1],
            expire_duration_ms: self.data[7],
            respawn_time_ms: self.data[8],
            flag_drop_entry: self.data[9],
            exclusive_category: self.data[10] as i32,
            world_state_id: self.data[11],
            return_on_defender_interact: self.data[12] != 0,
        })
    }

    pub const fn new_flag_drop_use_source_like_cpp(&self) -> Option<NewFlagDropUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_NEW_FLAG_DROP {
            return None;
        }

        Some(NewFlagDropUseSource {
            spawn_vignette_id: self.data[1],
        })
    }

    pub const fn spellcaster_use_source_like_cpp(&self) -> Option<SpellcasterUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_SPELLCASTER {
            return None;
        }

        Some(SpellcasterUseSource {
            spell_id: self.data[0],
            charges: self.data[1],
            party_only: self.data[2] != 0,
        })
    }

    pub const fn spell_focus_linked_trap_like_cpp(&self) -> u32 {
        if self.go_type != GAMEOBJECT_TYPE_SPELL_FOCUS {
            return 0;
        }

        self.data[2]
    }

    pub const fn camera_use_source_like_cpp(&self) -> Option<CameraUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_CAMERA {
            return None;
        }

        Some(CameraUseSource {
            cinematic_id: self.data[1],
            event_id: self.data[2],
        })
    }

    pub const fn goober_use_source_like_cpp(&self) -> Option<GooberUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_GOOBER {
            return None;
        }

        Some(GooberUseSource {
            lock_id: self.data[0],
            quest_id: self.data[1],
            event_id: self.data[2],
            auto_close_ms: self.data[3],
            custom_anim: self.data[4],
            consumable: self.data[5] != 0,
            page_id: self.data[7],
            spell_id: self.data[10],
            linked_trap_entry: self.data[12],
            gossip_id: self.data[19],
            allow_multi_interact: self.data[20] != 0,
            player_cast: self.data[23] != 0,
        })
    }

    pub const fn chest_loot_source_like_cpp(&self) -> Option<GameObjectLootSource> {
        if self.go_type != GAMEOBJECT_TYPE_CHEST {
            return None;
        }

        Some(GameObjectLootSource {
            loot_id: self.get_loot_id_like_cpp(),
            use_group_loot_rules: self.data[GAMEOBJECT_DATA_CHEST_USE_GROUP_LOOT_RULES] != 0,
            dungeon_encounter_id: self.data[GAMEOBJECT_DATA_CHEST_DUNGEON_ENCOUNTER],
            personal_loot_id: self.data[GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT],
            push_loot_id: self.data[GAMEOBJECT_DATA_CHEST_PUSH_LOOT],
            triggered_event_id: self.data[GAMEOBJECT_DATA_CHEST_TRIGGERED_EVENT],
            linked_trap_entry: self.data[GAMEOBJECT_DATA_CHEST_LINKED_TRAP],
            chest_restock_time_secs: self.data[GAMEOBJECT_DATA_CHEST_RESTOCK_TIME],
            chest_consumable: self.data[GAMEOBJECT_DATA_CHEST_CONSUMABLE] != 0,
        })
    }

    pub const fn gathering_node_use_source_like_cpp(&self) -> Option<GatheringNodeUseSource> {
        if self.go_type != GAMEOBJECT_TYPE_GATHERING_NODE {
            return None;
        }

        Some(GatheringNodeUseSource {
            loot_id: self.get_loot_id_like_cpp(),
            despawn_delay_secs: self.data[GAMEOBJECT_DATA_GATHERING_NODE_DESPAWN_DELAY],
            triggered_event_id: self.data[GAMEOBJECT_DATA_GATHERING_NODE_TRIGGERED_EVENT],
            xp_difficulty: self.data[GAMEOBJECT_DATA_GATHERING_NODE_XP_DIFFICULTY],
            spell_id: self.data[GAMEOBJECT_DATA_GATHERING_NODE_SPELL],
            max_loots: self.data[GAMEOBJECT_DATA_GATHERING_NODE_MAX_LOOTS],
            linked_trap_entry: self.data[GAMEOBJECT_DATA_GATHERING_NODE_LINKED_TRAP],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameObjectDataValues {
    pub display_id: i32,
    pub created_by: ObjectGuid,
    pub flags: u32,
    pub faction_template: i32,
    pub level: i32,
    pub state: i8,
    pub type_id: i8,
    pub percent_health: u8,
    pub art_kit: u32,
    pub custom_param: u32,
}

impl Default for GameObjectDataValues {
    fn default() -> Self {
        Self {
            display_id: 0,
            created_by: ObjectGuid::EMPTY,
            flags: 0,
            faction_template: 0,
            level: 0,
            state: GoState::Active as i8,
            type_id: 0,
            percent_health: 0,
            art_kit: 0,
            custom_param: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameObjectDataUpdate {
    pub mask: UpdateMask,
    pub values: GameObjectDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameObjectValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub game_object_data: Option<GameObjectDataUpdate>,
}

impl GameObjectValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameObject {
    world: WorldObject,
    data: GameObjectDataValues,
    game_object_data_changes: UpdateMask,
    spell_id: u32,
    respawn_time: i64,
    respawn_delay_time: u32,
    despawn_delay: u32,
    despawn_respawn_time: u32,
    restock_time: i64,
    loot_state: LootState,
    loot_state_unit_guid: ObjectGuid,
    spawned_by_default: bool,
    use_times: u32,
    cooldown_time: i64,
    prev_go_state: GoState,
    packed_rotation: i64,
    spawn_id: u64,
    loot_mode: u16,
    respawn_compatibility_mode: bool,
    anim_kit_id: u16,
    world_effect_id: u32,
    stationary_position: Position,
    grid_unload_cleanup_before_delete_count: u32,
    grid_unload_delete_requested: bool,
    grid_unload_respawn_relocation_requested: bool,
}

impl GameObject {
    pub fn new() -> Self {
        let mut world = WorldObject::new(
            false,
            TypeId::GameObject,
            TypeMask::OBJECT | TypeMask::GAME_OBJECT,
        );
        world
            .object_mut()
            .create_flags_mut()
            .insert(CreateObjectFlags::STATIONARY | CreateObjectFlags::ROTATION);

        Self {
            world,
            data: GameObjectDataValues::default(),
            game_object_data_changes: UpdateMask::new(GAME_OBJECT_DATA_BITS),
            spell_id: 0,
            respawn_time: 0,
            respawn_delay_time: DEFAULT_GAMEOBJECT_RESPAWN_DELAY_SECS,
            despawn_delay: 0,
            despawn_respawn_time: 0,
            restock_time: 0,
            loot_state: LootState::NotReady,
            loot_state_unit_guid: ObjectGuid::EMPTY,
            spawned_by_default: true,
            use_times: 0,
            cooldown_time: 0,
            prev_go_state: GoState::Active,
            packed_rotation: 0,
            spawn_id: 0,
            loot_mode: GAMEOBJECT_LOOT_MODE_DEFAULT,
            respawn_compatibility_mode: false,
            anim_kit_id: 0,
            world_effect_id: 0,
            stationary_position: Position::new(0.0, 0.0, 0.0, 0.0),
            grid_unload_cleanup_before_delete_count: 0,
            grid_unload_delete_requested: false,
            grid_unload_respawn_relocation_requested: false,
        }
    }

    pub const fn world(&self) -> &WorldObject {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut WorldObject {
        &mut self.world
    }

    pub const fn data(&self) -> &GameObjectDataValues {
        &self.data
    }

    pub fn game_object_data_changes_mask(&self) -> &UpdateMask {
        &self.game_object_data_changes
    }

    pub fn clear_game_object_data_changes(&mut self) {
        self.game_object_data_changes.reset_all();
    }

    pub const fn spell_id(&self) -> u32 {
        self.spell_id
    }

    pub fn set_spell_id(&mut self, spell_id: u32) {
        self.spell_id = spell_id;
        if spell_id != 0 {
            self.spawned_by_default = false;
        }
    }

    pub const fn respawn_time(&self) -> i64 {
        self.respawn_time
    }

    pub fn set_respawn_time(&mut self, respawn_time: i64) {
        self.respawn_time = respawn_time;
    }

    pub const fn respawn_delay_time(&self) -> u32 {
        self.respawn_delay_time
    }

    pub fn set_respawn_delay_time(&mut self, delay: u32) {
        self.respawn_delay_time = delay;
    }

    pub const fn despawn_delay(&self) -> u32 {
        self.despawn_delay
    }

    pub const fn despawn_respawn_time(&self) -> u32 {
        self.despawn_respawn_time
    }

    pub const fn restock_time(&self) -> i64 {
        self.restock_time
    }

    pub const fn loot_state(&self) -> LootState {
        self.loot_state
    }

    pub const fn loot_state_unit_guid(&self) -> ObjectGuid {
        self.loot_state_unit_guid
    }

    pub fn set_loot_state(&mut self, state: LootState, unit: Option<ObjectGuid>) {
        self.loot_state = state;
        self.loot_state_unit_guid = if state == LootState::Activated {
            unit.unwrap_or(ObjectGuid::EMPTY)
        } else {
            ObjectGuid::EMPTY
        };
    }

    pub const fn spawned_by_default(&self) -> bool {
        self.spawned_by_default
    }

    pub fn set_spawned_by_default(&mut self, spawned: bool) {
        self.spawned_by_default = spawned;
    }

    pub const fn use_times(&self) -> u32 {
        self.use_times
    }

    pub const fn cooldown_time(&self) -> i64 {
        self.cooldown_time
    }

    pub fn set_cooldown_time(&mut self, cooldown_time: i64) {
        self.cooldown_time = cooldown_time;
    }

    pub const fn prev_go_state(&self) -> GoState {
        self.prev_go_state
    }

    pub const fn packed_rotation(&self) -> i64 {
        self.packed_rotation
    }

    pub const fn spawn_id(&self) -> u64 {
        self.spawn_id
    }

    pub fn set_spawn_id(&mut self, spawn_id: u64) {
        self.spawn_id = spawn_id;
    }

    pub const fn loot_mode(&self) -> u16 {
        self.loot_mode
    }

    pub fn reset_loot_mode(&mut self) {
        self.loot_mode = GAMEOBJECT_LOOT_MODE_DEFAULT;
    }

    pub const fn respawn_compatibility_mode(&self) -> bool {
        self.respawn_compatibility_mode
    }

    pub fn set_respawn_compatibility_mode(&mut self, enabled: bool) {
        self.respawn_compatibility_mode = enabled;
    }

    pub const fn anim_kit_id(&self) -> u16 {
        self.anim_kit_id
    }

    pub const fn world_effect_id(&self) -> u32 {
        self.world_effect_id
    }

    pub const fn stationary_position(&self) -> Position {
        self.stationary_position
    }

    pub const fn cleanup_before_delete_count(&self) -> u32 {
        self.grid_unload_cleanup_before_delete_count
    }

    pub const fn grid_unload_delete_requested(&self) -> bool {
        self.grid_unload_delete_requested
    }

    pub const fn grid_unload_respawn_relocation_requested(&self) -> bool {
        self.grid_unload_respawn_relocation_requested
    }

    pub fn set_destroyed_object(&mut self, destroyed: bool) {
        self.world.object_mut().set_destroyed_object(destroyed);
    }

    pub fn request_respawn_relocation_from_grid_unload(&mut self) {
        self.grid_unload_respawn_relocation_requested = true;
    }

    pub fn cleanup_before_delete(&mut self) {
        self.grid_unload_cleanup_before_delete_count = self
            .grid_unload_cleanup_before_delete_count
            .saturating_add(1);
    }

    pub fn request_delete_from_grid_unload(&mut self) {
        self.grid_unload_delete_requested = true;
        self.world.clear_current_cell();
    }

    pub fn set_display_id(&mut self, display_id: u32) {
        self.set_i32_field(GAME_OBJECT_DATA_DISPLAY_ID_BIT, display_id as i32, |data| {
            &mut data.display_id
        });
    }

    pub fn set_faction(&mut self, faction: u32) {
        self.set_i32_field(
            GAME_OBJECT_DATA_FACTION_TEMPLATE_BIT,
            faction as i32,
            |data| &mut data.faction_template,
        );
    }

    pub fn set_go_state(&mut self, state: GoState) {
        self.set_i8_field(GAME_OBJECT_DATA_STATE_BIT, state as i8, |data| {
            &mut data.state
        });
    }

    pub fn set_go_type(&mut self, type_id: u8) {
        self.set_i8_field(GAME_OBJECT_DATA_TYPE_ID_BIT, type_id as i8, |data| {
            &mut data.type_id
        });
    }

    pub fn set_flags(&mut self, flags: u32) {
        self.set_u32_field(GAME_OBJECT_DATA_FLAGS_BIT, flags, |data| &mut data.flags);
    }

    pub fn set_level(&mut self, level: u32) {
        self.set_i32_field(GAME_OBJECT_DATA_LEVEL_BIT, level as i32, |data| {
            &mut data.level
        });
    }

    pub fn set_percent_health(&mut self, percent_health: u8) {
        self.set_u8_field(
            GAME_OBJECT_DATA_PERCENT_HEALTH_BIT,
            percent_health,
            |data| &mut data.percent_health,
        );
    }

    pub fn set_art_kit(&mut self, art_kit: u32) {
        self.set_u32_field(GAME_OBJECT_DATA_ART_KIT_BIT, art_kit, |data| {
            &mut data.art_kit
        });
    }

    pub fn set_custom_param(&mut self, custom_param: u32) {
        self.set_u32_field(GAME_OBJECT_DATA_CUSTOM_PARAM_BIT, custom_param, |data| {
            &mut data.custom_param
        });
    }

    pub fn set_created_by(&mut self, created_by: ObjectGuid) {
        self.set_guid_field(GAME_OBJECT_DATA_CREATED_BY_BIT, created_by, |data| {
            &mut data.created_by
        });
    }

    pub const fn owner_guid(&self) -> ObjectGuid {
        self.data.created_by
    }

    pub fn set_path_progress_for_client(&mut self, progress: f32) {
        let had_dynamic_flags_change = self
            .world
            .object()
            .changed_fields()
            .contains(ObjectChangedFields::DYNAMIC_FLAGS);
        let path_progress = (progress.clamp(0.0, 1.0) * 65_535.0) as u32;
        let dynamic_flags = (self.world.object().dynamic_flags() & 0xFFFF) | (path_progress << 16);

        if had_dynamic_flags_change {
            self.world
                .object_mut()
                .replace_all_dynamic_flags(dynamic_flags);
        } else {
            self.world
                .object_mut()
                .replace_all_dynamic_flags_suppressed(dynamic_flags);
        }
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.world.object().changed_object_type_mask()
            | if self.game_object_data_changes.is_any_set() {
                1 << TYPEID_GAME_OBJECT
            } else {
                0
            }
    }

    pub fn values_update(&self) -> GameObjectValuesUpdate {
        let object_update = self.world.object().values_update();
        GameObjectValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: object_update.object_data,
            game_object_data: self.game_object_data_changes.is_any_set().then(|| {
                GameObjectDataUpdate {
                    mask: self.game_object_data_changes.clone(),
                    values: self.data,
                }
            }),
        }
    }

    fn set_u32_field(
        &mut self,
        bit: usize,
        value: u32,
        field: impl FnOnce(&mut GameObjectDataValues) -> &mut u32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_game_object_data(bit);
        }
    }

    fn set_i32_field(
        &mut self,
        bit: usize,
        value: i32,
        field: impl FnOnce(&mut GameObjectDataValues) -> &mut i32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_game_object_data(bit);
        }
    }

    fn set_i8_field(
        &mut self,
        bit: usize,
        value: i8,
        field: impl FnOnce(&mut GameObjectDataValues) -> &mut i8,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_game_object_data(bit);
        }
    }

    fn set_u8_field(
        &mut self,
        bit: usize,
        value: u8,
        field: impl FnOnce(&mut GameObjectDataValues) -> &mut u8,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_game_object_data(bit);
        }
    }

    fn set_guid_field(
        &mut self,
        bit: usize,
        value: ObjectGuid,
        field: impl FnOnce(&mut GameObjectDataValues) -> &mut ObjectGuid,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_game_object_data(bit);
        }
    }

    fn mark_game_object_data(&mut self, bit: usize) {
        self.game_object_data_changes
            .set(GAME_OBJECT_DATA_PARENT_BIT);
        self.game_object_data_changes.set(bit);
    }
}

impl Default for GameObject {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameobject_constructor_matches_cpp_base_state() {
        let go = GameObject::new();

        assert_eq!(go.world().object().type_id(), TypeId::GameObject);
        assert_eq!(
            go.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::GAME_OBJECT
        );
        assert!(!go.world().is_world_object());
        assert!(
            go.world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::STATIONARY)
        );
        assert!(
            go.world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::ROTATION)
        );
        assert_eq!(go.respawn_time(), 0);
        assert_eq!(
            go.respawn_delay_time(),
            DEFAULT_GAMEOBJECT_RESPAWN_DELAY_SECS
        );
        assert_eq!(go.despawn_delay(), 0);
        assert_eq!(go.despawn_respawn_time(), 0);
        assert_eq!(go.restock_time(), 0);
        assert_eq!(go.loot_state(), LootState::NotReady);
        assert_eq!(go.loot_state_unit_guid(), ObjectGuid::EMPTY);
        assert!(go.spawned_by_default());
        assert_eq!(go.use_times(), 0);
        assert_eq!(go.spell_id(), 0);
        assert_eq!(go.cooldown_time(), 0);
        assert_eq!(go.prev_go_state(), GoState::Active);
        assert_eq!(go.packed_rotation(), 0);
        assert_eq!(go.spawn_id(), 0);
        assert_eq!(go.loot_mode(), GAMEOBJECT_LOOT_MODE_DEFAULT);
        assert!(!go.respawn_compatibility_mode());
        assert_eq!(go.anim_kit_id(), 0);
        assert_eq!(go.world_effect_id(), 0);
        assert_eq!(go.stationary_position(), Position::new(0.0, 0.0, 0.0, 0.0));
        assert_eq!(go.cleanup_before_delete_count(), 0);
        assert!(!go.grid_unload_delete_requested());
        assert!(!go.grid_unload_respawn_relocation_requested());
        assert!(!go.game_object_data_changes_mask().is_any_set());
    }

    #[test]
    fn gameobject_template_get_loot_id_matches_cpp_switch() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[GAMEOBJECT_DATA_CHEST_LOOT] = 44;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).get_loot_id_like_cpp(),
            44
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_FISHING_HOLE, data).get_loot_id_like_cpp(),
            44
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_GATHERING_NODE, data)
                .get_loot_id_like_cpp(),
            44
        );
        assert_eq!(
            GameObjectTemplateData::new(2, data).get_loot_id_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_condition_id1_matches_cpp_switch() {
        let cases = [
            (GAMEOBJECT_TYPE_DOOR, 7),
            (GAMEOBJECT_TYPE_BUTTON, 9),
            (GAMEOBJECT_TYPE_QUESTGIVER, 10),
            (GAMEOBJECT_TYPE_CHEST, 17),
            (GAMEOBJECT_TYPE_GENERIC, 6),
            (GAMEOBJECT_TYPE_TRAP, 15),
            (GAMEOBJECT_TYPE_CHAIR, 4),
            (GAMEOBJECT_TYPE_SPELL_FOCUS, 8),
            (GAMEOBJECT_TYPE_TEXT, 4),
            (GAMEOBJECT_TYPE_GOOBER, 22),
            (GAMEOBJECT_TYPE_CAMERA, 4),
            (GAMEOBJECT_TYPE_RITUAL, 8),
            (GAMEOBJECT_TYPE_MAILBOX, 0),
            (GAMEOBJECT_TYPE_SPELLCASTER, 5),
            (GAMEOBJECT_TYPE_FLAGSTAND, 8),
            (GAMEOBJECT_TYPE_AURA_GENERATOR, 3),
            (GAMEOBJECT_TYPE_GUILD_BANK, 0),
            (GAMEOBJECT_TYPE_NEW_FLAG, 4),
            (GAMEOBJECT_TYPE_ITEM_FORGE, 0),
            (GAMEOBJECT_TYPE_GATHERING_NODE, 11),
        ];

        for (go_type, index) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            data[index] = 7_000 + go_type;
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_condition_id1_like_cpp(),
                7_000 + go_type
            );
        }

        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 999;
        assert_eq!(
            GameObjectTemplateData::new(25, data).get_condition_id1_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_interact_radius_override_matches_cpp_switch() {
        let cases = [
            (GAMEOBJECT_TYPE_DOOR, 12),
            (GAMEOBJECT_TYPE_BUTTON, 10),
            (GAMEOBJECT_TYPE_QUESTGIVER, 12),
            (GAMEOBJECT_TYPE_CHEST, 9),
            (GAMEOBJECT_TYPE_BINDER, 0),
            (GAMEOBJECT_TYPE_GENERIC, 9),
            (GAMEOBJECT_TYPE_TRAP, 21),
            (GAMEOBJECT_TYPE_CHAIR, 5),
            (GAMEOBJECT_TYPE_SPELL_FOCUS, 9),
            (GAMEOBJECT_TYPE_TEXT, 6),
            (GAMEOBJECT_TYPE_GOOBER, 33),
            (GAMEOBJECT_TYPE_AREADAMAGE, 8),
            (GAMEOBJECT_TYPE_CAMERA, 5),
            (GAMEOBJECT_TYPE_FISHING_NODE, 0),
            (GAMEOBJECT_TYPE_RITUAL, 9),
            (GAMEOBJECT_TYPE_MAILBOX, 1),
            (GAMEOBJECT_TYPE_SPELLCASTER, 8),
            (GAMEOBJECT_TYPE_MEETINGSTONE, 3),
            (GAMEOBJECT_TYPE_FLAGSTAND, 13),
            (GAMEOBJECT_TYPE_FISHING_HOLE, 5),
            (GAMEOBJECT_TYPE_FLAGDROP, 10),
            (GAMEOBJECT_TYPE_AURA_GENERATOR, 7),
            (GAMEOBJECT_TYPE_DUNGEON_DIFFICULTY, 11),
            (GAMEOBJECT_TYPE_BARBER_CHAIR, 3),
            (GAMEOBJECT_TYPE_DESTRUCTIBLE_BUILDING, 27),
            (GAMEOBJECT_TYPE_GUILD_BANK, 1),
            (GAMEOBJECT_TYPE_NEW_FLAG, 14),
            (GAMEOBJECT_TYPE_NEW_FLAG_DROP, 2),
            (GAMEOBJECT_TYPE_GATHERING_NODE, 24),
        ];

        for (go_type, index) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            data[index] = 10_000 + go_type;
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_interact_radius_override_like_cpp(),
                10_000 + go_type
            );
        }

        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 999;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_MAP_OBJECT, data)
                .get_interact_radius_override_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_lock_id_matches_cpp_switch() {
        let cases = [
            (GAMEOBJECT_TYPE_DOOR, 1),
            (GAMEOBJECT_TYPE_BUTTON, 1),
            (GAMEOBJECT_TYPE_QUESTGIVER, 0),
            (GAMEOBJECT_TYPE_CHEST, 0),
            (GAMEOBJECT_TYPE_TRAP, 0),
            (GAMEOBJECT_TYPE_GOOBER, 0),
            (GAMEOBJECT_TYPE_AREADAMAGE, 0),
            (GAMEOBJECT_TYPE_CAMERA, 0),
            (GAMEOBJECT_TYPE_FLAGSTAND, 0),
            (GAMEOBJECT_TYPE_FISHING_HOLE, 4),
            (GAMEOBJECT_TYPE_FLAGDROP, 0),
            (GAMEOBJECT_TYPE_NEW_FLAG, 0),
            (GAMEOBJECT_TYPE_NEW_FLAG_DROP, 0),
            (GAMEOBJECT_TYPE_GATHERING_NODE, 3),
        ];

        for (go_type, index) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            data[index] = 20_000 + go_type;
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_lock_id_like_cpp(),
                20_000 + go_type
            );
        }

        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 999;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_MAP_OBJECT, data).get_lock_id_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_usable_mounted_matches_cpp_switch() {
        assert!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_MAILBOX, [0; MAX_GAMEOBJECT_DATA])
                .is_usable_mounted_like_cpp()
        );
        assert!(
            !GameObjectTemplateData::new(GAMEOBJECT_TYPE_BARBER_CHAIR, [1; MAX_GAMEOBJECT_DATA])
                .is_usable_mounted_like_cpp()
        );

        let cases = [
            (GAMEOBJECT_TYPE_QUESTGIVER, 8),
            (GAMEOBJECT_TYPE_TEXT, 3),
            (GAMEOBJECT_TYPE_GOOBER, 17),
            (GAMEOBJECT_TYPE_SPELLCASTER, 3),
            (GAMEOBJECT_TYPE_UI_LINK, 1),
        ];

        for (go_type, index) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            assert!(
                !GameObjectTemplateData::new(go_type, data).is_usable_mounted_like_cpp(),
                "type {go_type} should default to not usable mounted"
            );
            data[index] = 1;
            assert!(
                GameObjectTemplateData::new(go_type, data).is_usable_mounted_like_cpp(),
                "type {go_type} should read allowMounted from data[{index}]"
            );
        }

        assert!(
            !GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, [1; MAX_GAMEOBJECT_DATA])
                .is_usable_mounted_like_cpp()
        );
    }

    #[test]
    fn gameobject_template_no_damage_immune_matches_cpp_switch() {
        let cases = [
            (GAMEOBJECT_TYPE_DOOR, 3),
            (GAMEOBJECT_TYPE_BUTTON, 4),
            (GAMEOBJECT_TYPE_QUESTGIVER, 5),
            (GAMEOBJECT_TYPE_GOOBER, 11),
            (GAMEOBJECT_TYPE_FLAGSTAND, 5),
            (GAMEOBJECT_TYPE_FLAGDROP, 3),
        ];

        for (go_type, index) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_no_damage_immune_like_cpp(),
                0
            );
            data[index] = 7;
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_no_damage_immune_like_cpp(),
                7
            );
        }

        let mut chest = [0; MAX_GAMEOBJECT_DATA];
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, chest)
                .get_no_damage_immune_like_cpp(),
            1
        );
        chest[22] = 1;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, chest)
                .get_no_damage_immune_like_cpp(),
            0
        );

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_TEXT, [1; MAX_GAMEOBJECT_DATA])
                .get_no_damage_immune_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_cooldown_matches_cpp_switch() {
        let mut trap = [0; MAX_GAMEOBJECT_DATA];
        trap[5] = 12;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_TRAP, trap).get_cooldown_like_cpp(),
            12
        );

        let mut goober = [0; MAX_GAMEOBJECT_DATA];
        goober[6] = 34;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_GOOBER, goober).get_cooldown_like_cpp(),
            34
        );

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, [99; MAX_GAMEOBJECT_DATA])
                .get_cooldown_like_cpp(),
            0
        );
    }

    #[test]
    fn gameobject_template_auto_close_time_matches_cpp_switch() {
        let cases = [
            (GAMEOBJECT_TYPE_DOOR, 2, 11),
            (GAMEOBJECT_TYPE_BUTTON, 2, 22),
            (GAMEOBJECT_TYPE_TRAP, 6, 33),
            (GAMEOBJECT_TYPE_GOOBER, 3, 44),
        ];

        for (go_type, index, value) in cases {
            let mut data = [0; MAX_GAMEOBJECT_DATA];
            data[index] = value;
            assert_eq!(
                GameObjectTemplateData::new(go_type, data).get_auto_close_time_like_cpp(),
                value
            );
        }

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, [99; MAX_GAMEOBJECT_DATA])
                .get_auto_close_time_like_cpp(),
            0
        );
    }

    #[test]
    fn trap_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[3] = 123;
        data[4] = 1;
        data[5] = 9;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_TRAP, data).trap_use_source_like_cpp(),
            Some(TrapUseSource {
                spell_id: 123,
                charges: 1,
                cooldown_secs: 9,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).trap_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn chair_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 3;
        data[1] = 2;
        data[3] = 77;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHAIR, data).chair_use_source_like_cpp(),
            Some(ChairUseSource {
                chair_slots: 3,
                chair_height: 2,
                triggered_event_id: 77,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).chair_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn barber_chair_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 2;
        data[2] = 345;
        data[4] = 9;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_BARBER_CHAIR, data)
                .barber_chair_use_source_like_cpp(),
            Some(BarberChairUseSource {
                chair_height: 2,
                sit_anim_kit: 345,
                customization_scope: 9,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHAIR, data)
                .barber_chair_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn ui_link_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 3;
        data[6] = 99;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_UI_LINK, data)
                .ui_link_use_source_like_cpp(),
            Some(UiLinkUseSource { ui_link_type: 3 })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).ui_link_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn item_forge_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 77;
        data[5] = 4;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_ITEM_FORGE, data)
                .item_forge_use_source_like_cpp(),
            Some(ItemForgeUseSource {
                condition_id: 77,
                forge_type: 4,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .item_forge_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn capture_point_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 60_000;
        data[10] = 123;
        data[11] = 456;
        data[14] = 789;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CAPTURE_POINT, data)
                .capture_point_use_source_like_cpp(),
            Some(CapturePointUseSource {
                capture_time_ms: 60_000,
                world_state_id: 123,
                contested_event_horde: 456,
                contested_event_alliance: 789,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .capture_point_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn battleground_flag_use_sources_use_cpp_data_indices() {
        let mut stand = [0; MAX_GAMEOBJECT_DATA];
        stand[1] = 111;
        stand[3] = 333;
        stand[4] = 444;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_FLAGSTAND, stand)
                .flag_stand_use_source_like_cpp(),
            Some(FlagStandUseSource {
                pickup_spell_id: 111,
                return_aura_id: 333,
                return_spell_id: 444,
            })
        );

        let mut drop = [0; MAX_GAMEOBJECT_DATA];
        drop[1] = 222;
        drop[2] = 333;
        drop[6] = 666;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_FLAGDROP, drop)
                .flag_drop_use_source_like_cpp(),
            Some(FlagDropUseSource {
                event_id: 222,
                pickup_spell_id: 333,
                expire_duration_ms: 666,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, drop)
                .flag_drop_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn questgiver_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[3] = 42;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_QUESTGIVER, data)
                .questgiver_use_source_like_cpp(),
            Some(QuestgiverUseSource { gossip_id: 42 })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .questgiver_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn ritual_and_meeting_stone_use_sources_use_cpp_data_indices() {
        let mut ritual = [0; MAX_GAMEOBJECT_DATA];
        ritual[0] = 3;
        ritual[1] = 62330;
        ritual[2] = 111;
        ritual[3] = 1;
        ritual[4] = 222;
        ritual[5] = 2;
        ritual[6] = 1;
        ritual[7] = 1;
        ritual[10] = 1;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_RITUAL, ritual)
                .ritual_use_source_like_cpp(),
            Some(RitualUseSource {
                casters_required: 3,
                spell_id: 62330,
                anim_spell_id: 111,
                persistent: true,
                caster_target_spell_id: 222,
                caster_target_spell_targets: 2,
                casters_grouped: true,
                no_target_check: true,
                allow_unfriendly_cross_faction_party: true,
            })
        );

        let mut meeting_stone = [0; MAX_GAMEOBJECT_DATA];
        meeting_stone[2] = 345;
        meeting_stone[4] = 1;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_MEETINGSTONE, meeting_stone)
                .meeting_stone_use_source_like_cpp(),
            Some(MeetingStoneUseSource {
                area_id: 345,
                prevent_unfriendly_outside_instances: true,
                content_tuning_id: 0,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, ritual).ritual_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn new_flag_use_sources_use_cpp_data_indices() {
        let mut flag = [0; MAX_GAMEOBJECT_DATA];
        flag[1] = 111;
        flag[7] = 777;
        flag[8] = 888;
        flag[9] = 999;
        flag[10] = u32::MAX;
        flag[11] = 1111;
        flag[12] = 1;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_NEW_FLAG, flag)
                .new_flag_use_source_like_cpp(),
            Some(NewFlagUseSource {
                pickup_spell_id: 111,
                expire_duration_ms: 777,
                respawn_time_ms: 888,
                flag_drop_entry: 999,
                exclusive_category: -1,
                world_state_id: 1111,
                return_on_defender_interact: true,
            })
        );

        let mut drop = [0; MAX_GAMEOBJECT_DATA];
        drop[1] = 222;
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_NEW_FLAG_DROP, drop)
                .new_flag_drop_use_source_like_cpp(),
            Some(NewFlagDropUseSource {
                spawn_vignette_id: 222,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, drop).new_flag_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn spellcaster_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 1234;
        data[1] = 7;
        data[2] = 1;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_SPELLCASTER, data)
                .spellcaster_use_source_like_cpp(),
            Some(SpellcasterUseSource {
                spell_id: 1234,
                charges: 7,
                party_only: true,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .spellcaster_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn spell_focus_linked_trap_uses_cpp_data_index() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[2] = 987;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_SPELL_FOCUS, data)
                .spell_focus_linked_trap_like_cpp(),
            987
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .spell_focus_linked_trap_like_cpp(),
            0
        );
    }

    #[test]
    fn camera_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[1] = 1234;
        data[2] = 55;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CAMERA, data).camera_use_source_like_cpp(),
            Some(CameraUseSource {
                cinematic_id: 1234,
                event_id: 55,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).camera_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn goober_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[0] = 99;
        data[1] = 101;
        data[2] = 202;
        data[3] = 303;
        data[4] = 4;
        data[5] = 1;
        data[7] = 707;
        data[10] = 1010;
        data[12] = 1212;
        data[19] = 1919;
        data[20] = 1;
        data[23] = 1;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_GOOBER, data).goober_use_source_like_cpp(),
            Some(GooberUseSource {
                lock_id: 99,
                quest_id: 101,
                event_id: 202,
                auto_close_ms: 303,
                custom_anim: 4,
                consumable: true,
                page_id: 707,
                spell_id: 1010,
                linked_trap_entry: 1212,
                gossip_id: 1919,
                allow_multi_interact: true,
                player_cast: true,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data).goober_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn chest_loot_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[GAMEOBJECT_DATA_CHEST_LOOT] = 10;
        data[GAMEOBJECT_DATA_CHEST_RESTOCK_TIME] = 60;
        data[GAMEOBJECT_DATA_CHEST_CONSUMABLE] = 1;
        data[GAMEOBJECT_DATA_CHEST_TRIGGERED_EVENT] = 40;
        data[GAMEOBJECT_DATA_CHEST_LINKED_TRAP] = 50;
        data[GAMEOBJECT_DATA_CHEST_USE_GROUP_LOOT_RULES] = 1;
        data[GAMEOBJECT_DATA_CHEST_DUNGEON_ENCOUNTER] = 1234;
        data[GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT] = 20;
        data[GAMEOBJECT_DATA_CHEST_PUSH_LOOT] = 30;

        let source = GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
            .chest_loot_source_like_cpp()
            .expect("chest templates expose a chest loot source");

        assert_eq!(
            source,
            GameObjectLootSource {
                loot_id: 10,
                use_group_loot_rules: true,
                dungeon_encounter_id: 1234,
                personal_loot_id: 20,
                push_loot_id: 30,
                triggered_event_id: 40,
                linked_trap_entry: 50,
                chest_restock_time_secs: 60,
                chest_consumable: true,
            }
        );
        assert!(!source.is_empty());
        assert_eq!(source.open_loot_id_like_cpp(), 10);
        assert!(source.has_open_loot_like_cpp());
        assert!(!source.is_personal_encounter_loot_like_cpp());
        assert!(!source.should_autostore_push_loot_like_cpp());

        data[GAMEOBJECT_DATA_CHEST_LOOT] = 0;
        data[GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT] = 0;
        let push_source = GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
            .chest_loot_source_like_cpp()
            .expect("chest templates expose a chest loot source");
        assert!(!push_source.is_empty());
        assert!(!push_source.has_open_loot_like_cpp());
        assert!(!push_source.is_personal_encounter_loot_like_cpp());
        assert!(push_source.should_autostore_push_loot_like_cpp());

        data[GAMEOBJECT_DATA_CHEST_PERSONAL_LOOT] = 25;
        let personal_encounter_source = GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
            .chest_loot_source_like_cpp()
            .expect("chest templates expose a chest loot source");
        assert_eq!(personal_encounter_source.open_loot_id_like_cpp(), 25);
        assert!(personal_encounter_source.has_open_loot_like_cpp());
        assert!(personal_encounter_source.is_personal_encounter_loot_like_cpp());
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_FISHING_HOLE, data)
                .chest_loot_source_like_cpp(),
            None
        );
    }

    #[test]
    fn gathering_node_use_source_uses_cpp_data_indices() {
        let mut data = [0; MAX_GAMEOBJECT_DATA];
        data[GAMEOBJECT_DATA_CHEST_LOOT] = 10;
        data[GAMEOBJECT_DATA_GATHERING_NODE_DESPAWN_DELAY] = 15;
        data[GAMEOBJECT_DATA_GATHERING_NODE_TRIGGERED_EVENT] = 20;
        data[GAMEOBJECT_DATA_GATHERING_NODE_XP_DIFFICULTY] = 5;
        data[GAMEOBJECT_DATA_GATHERING_NODE_SPELL] = 30;
        data[GAMEOBJECT_DATA_GATHERING_NODE_MAX_LOOTS] = 3;
        data[GAMEOBJECT_DATA_GATHERING_NODE_LINKED_TRAP] = 40;

        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_GATHERING_NODE, data)
                .gathering_node_use_source_like_cpp(),
            Some(GatheringNodeUseSource {
                loot_id: 10,
                despawn_delay_secs: 15,
                triggered_event_id: 20,
                xp_difficulty: 5,
                spell_id: 30,
                max_loots: 3,
                linked_trap_entry: 40,
            })
        );
        assert_eq!(
            GameObjectTemplateData::new(GAMEOBJECT_TYPE_CHEST, data)
                .gathering_node_use_source_like_cpp(),
            None
        );
    }

    #[test]
    fn gameobject_data_setters_mark_cpp_bits() {
        let mut go = GameObject::new();

        go.set_display_id(1234);
        go.set_faction(35);
        go.set_go_state(GoState::Ready);
        go.set_go_type(3);
        go.set_flags(0x20);
        go.set_level(70);
        go.set_percent_health(80);
        go.set_art_kit(4);
        go.set_custom_param(99);
        let owner = ObjectGuid::create_player(1, 42);
        go.set_created_by(owner);

        assert_eq!(go.data().display_id, 1234);
        assert_eq!(go.owner_guid(), owner);
        assert_eq!(go.data().created_by, owner);
        assert_eq!(go.data().faction_template, 35);
        assert_eq!(go.data().state, GoState::Ready as i8);
        assert_eq!(go.data().type_id, 3);
        assert_eq!(go.data().flags, 0x20);
        assert_eq!(go.data().level, 70);
        assert_eq!(go.data().percent_health, 80);
        assert_eq!(go.data().art_kit, 4);
        assert_eq!(go.data().custom_param, 99);
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_PARENT_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_DISPLAY_ID_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_CREATED_BY_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_FACTION_TEMPLATE_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_STATE_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_TYPE_ID_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_FLAGS_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_LEVEL_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_PERCENT_HEALTH_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_ART_KIT_BIT)
        );
        assert!(
            go.game_object_data_changes_mask()
                .is_set(GAME_OBJECT_DATA_CUSTOM_PARAM_BIT)
        );
    }

    #[test]
    fn loot_state_tracks_activating_unit_only_for_activated_state() {
        let mut go = GameObject::new();
        let unit = ObjectGuid::new(7, 11);

        go.set_loot_state(LootState::Activated, Some(unit));
        assert_eq!(go.loot_state(), LootState::Activated);
        assert_eq!(go.loot_state_unit_guid(), unit);

        go.set_loot_state(LootState::Ready, Some(unit));
        assert_eq!(go.loot_state(), LootState::Ready);
        assert_eq!(go.loot_state_unit_guid(), ObjectGuid::EMPTY);
    }

    #[test]
    fn spell_id_and_spawn_fields_match_cpp_base_behaviour() {
        let mut go = GameObject::new();

        go.set_spell_id(123);
        go.set_spawn_id(99);
        go.set_respawn_delay_time(45);
        go.set_respawn_time(1000);
        go.set_spawned_by_default(true);
        go.set_cooldown_time(77);
        go.set_respawn_compatibility_mode(true);

        assert_eq!(go.spell_id(), 123);
        assert_eq!(go.spawn_id(), 99);
        assert_eq!(go.respawn_delay_time(), 45);
        assert_eq!(go.respawn_time(), 1000);
        assert!(go.spawned_by_default());
        assert_eq!(go.cooldown_time(), 77);
        assert!(go.respawn_compatibility_mode());
    }

    #[test]
    fn path_progress_for_client_preserves_cpp_dynamic_flag_change_state() {
        let mut go = GameObject::new();

        go.set_path_progress_for_client(0.5);
        assert_eq!(go.world().object().dynamic_flags() >> 16, 32_767);
        assert!(
            !go.world()
                .object()
                .changed_fields()
                .contains(ObjectChangedFields::DYNAMIC_FLAGS)
        );

        go.world_mut().object_mut().set_dynamic_flag(0x4);
        go.set_path_progress_for_client(1.0);
        assert_eq!(go.world().object().dynamic_flags() & 0xFFFF, 0x4);
        assert_eq!(go.world().object().dynamic_flags() >> 16, 65_535);
        assert!(
            go.world()
                .object()
                .changed_fields()
                .contains(ObjectChangedFields::DYNAMIC_FLAGS)
        );
    }

    #[test]
    fn values_update_sets_gameobject_object_type_bit() {
        let mut go = GameObject::new();

        go.set_display_id(1234);
        let update = go.values_update();

        assert!(update.has_data());
        assert_eq!(update.changed_object_type_mask, 1 << TYPEID_GAME_OBJECT);
        let game_object_data = update.game_object_data.unwrap();
        assert_eq!(game_object_data.values.display_id, 1234);
        assert!(
            game_object_data
                .mask
                .is_set(GAME_OBJECT_DATA_DISPLAY_ID_BIT)
        );
    }

    #[test]
    fn gameobject_grid_unload_helpers_apply_represented_state() {
        let mut go = GameObject::new();
        go.world_mut().set_current_cell(3, 4);

        go.set_destroyed_object(true);
        go.request_respawn_relocation_from_grid_unload();
        go.cleanup_before_delete();
        go.request_delete_from_grid_unload();

        assert!(go.world().object().is_destroyed_object());
        assert!(go.grid_unload_respawn_relocation_requested());
        assert_eq!(go.cleanup_before_delete_count(), 1);
        assert!(go.grid_unload_delete_requested());
        assert_eq!(go.world().current_cell(), None);
        assert!(!go.world().object().is_in_grid());
    }
}
