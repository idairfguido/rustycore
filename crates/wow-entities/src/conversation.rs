use std::collections::HashMap;

use wow_constants::{TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

use crate::{
    CreateObjectFlags, ObjectDataUpdate, UpdateMask, WorldObject,
    update_fields::{CONVERSATION_DATA_BITS, TYPEID_CONVERSATION},
};

pub const CONVERSATION_DATA_PARENT_BIT: usize = 0;
pub const CONVERSATION_DATA_LINES_BIT: usize = 1;
pub const CONVERSATION_DATA_ACTORS_BIT: usize = 2;
pub const CONVERSATION_DATA_LAST_LINE_END_TIME_BIT: usize = 3;

pub const CONVERSATION_DESPAWN_DELAY_MS: i32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ConversationActorType {
    WorldObject = 0,
    TalkingHead = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConversationLine {
    pub conversation_line_id: i32,
    pub start_time: u32,
    pub ui_camera_id: i32,
    pub actor_index: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationActor {
    pub actor_type: u32,
    pub id: i32,
    pub creature_id: u32,
    pub creature_display_info_id: u32,
    pub actor_guid: ObjectGuid,
}

impl Default for ConversationActor {
    fn default() -> Self {
        Self {
            actor_type: ConversationActorType::WorldObject as u32,
            id: 0,
            creature_id: 0,
            creature_display_info_id: 0,
            actor_guid: ObjectGuid::EMPTY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConversationDataValues {
    pub lines: Vec<ConversationLine>,
    pub actors: Vec<ConversationActor>,
    pub last_line_end_time: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationDataUpdate {
    pub mask: UpdateMask,
    pub values: ConversationDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConversationValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub conversation_data: Option<ConversationDataUpdate>,
}

impl ConversationValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    world: WorldObject,
    data: ConversationDataValues,
    conversation_data_changes: UpdateMask,
    stationary_position: Position,
    creator_guid: ObjectGuid,
    duration_ms: i32,
    texture_kit_id: u32,
    line_start_times: HashMap<(u8, i32), i32>,
    last_line_end_times: Vec<i32>,
    is_removed: bool,
    grid_unload_cleanup_before_delete_count: u32,
    grid_unload_delete_requested: bool,
}

impl Conversation {
    pub fn new() -> Self {
        let mut world = WorldObject::new(
            false,
            TypeId::Conversation,
            TypeMask::OBJECT | TypeMask::CONVERSATION,
        );
        world
            .object_mut()
            .create_flags_mut()
            .insert(CreateObjectFlags::STATIONARY | CreateObjectFlags::CONVERSATION);

        Self {
            world,
            data: ConversationDataValues::default(),
            conversation_data_changes: UpdateMask::new(CONVERSATION_DATA_BITS),
            stationary_position: Position::new(0.0, 0.0, 0.0, 0.0),
            creator_guid: ObjectGuid::EMPTY,
            duration_ms: 0,
            texture_kit_id: 0,
            line_start_times: HashMap::new(),
            last_line_end_times: Vec::new(),
            is_removed: false,
            grid_unload_cleanup_before_delete_count: 0,
            grid_unload_delete_requested: false,
        }
    }

    pub const fn world(&self) -> &WorldObject {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut WorldObject {
        &mut self.world
    }

    pub const fn data(&self) -> &ConversationDataValues {
        &self.data
    }

    pub fn conversation_data_changes_mask(&self) -> &UpdateMask {
        &self.conversation_data_changes
    }

    pub fn clear_conversation_data_changes(&mut self) {
        self.conversation_data_changes.reset_all();
    }

    pub const fn cleanup_before_delete_count(&self) -> u32 {
        self.grid_unload_cleanup_before_delete_count
    }

    pub const fn grid_unload_delete_requested(&self) -> bool {
        self.grid_unload_delete_requested
    }

    pub fn set_destroyed_object(&mut self, destroyed: bool) {
        self.world.object_mut().set_destroyed_object(destroyed);
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

    pub const fn stationary_position(&self) -> Position {
        self.stationary_position
    }

    pub fn relocate_stationary_position(&mut self, position: Position) {
        self.stationary_position = position;
    }

    pub const fn creator_guid(&self) -> ObjectGuid {
        self.creator_guid
    }

    pub const fn owner_guid(&self) -> ObjectGuid {
        self.creator_guid()
    }

    pub const fn faction(&self) -> u32 {
        0
    }

    pub const fn duration_ms(&self) -> i32 {
        self.duration_ms
    }

    pub const fn texture_kit_id(&self) -> u32 {
        self.texture_kit_id
    }

    pub const fn is_removed(&self) -> bool {
        self.is_removed
    }

    pub fn line_start_time(&self, locale: u8, line_id: i32) -> Option<i32> {
        self.line_start_times.get(&(locale, line_id)).copied()
    }

    pub fn last_line_end_time(&self, locale: usize) -> Option<i32> {
        self.last_line_end_times.get(locale).copied()
    }

    pub fn set_creator_guid(&mut self, creator_guid: ObjectGuid) {
        self.creator_guid = creator_guid;
    }

    pub fn set_texture_kit_id(&mut self, texture_kit_id: u32) {
        self.texture_kit_id = texture_kit_id;
    }

    pub fn set_duration_ms(&mut self, duration_ms: i32) {
        self.duration_ms = duration_ms;
    }

    pub fn update_duration(&mut self, diff_ms: u32) -> bool {
        if self.duration_ms > diff_ms as i32 {
            self.duration_ms -= diff_ms as i32;
            false
        } else {
            self.remove();
            true
        }
    }

    pub fn remove(&mut self) {
        self.is_removed = true;
    }

    pub fn set_last_line_end_time(&mut self, last_line_end_time: i32) {
        if self.data.last_line_end_time != last_line_end_time {
            self.data.last_line_end_time = last_line_end_time;
            self.mark_conversation_data(CONVERSATION_DATA_LAST_LINE_END_TIME_BIT);
        }
    }

    pub fn set_lines(&mut self, lines: Vec<ConversationLine>) {
        if self.data.lines != lines {
            self.data.lines = lines;
            self.mark_conversation_data(CONVERSATION_DATA_LINES_BIT);
        }
    }

    pub fn add_line(&mut self, line: ConversationLine) {
        self.data.lines.push(line);
        self.mark_conversation_data(CONVERSATION_DATA_LINES_BIT);
    }

    pub fn add_actor_world_object(
        &mut self,
        actor_id: i32,
        actor_index: usize,
        actor_guid: ObjectGuid,
    ) {
        self.set_actor(
            actor_index,
            ConversationActor {
                actor_type: ConversationActorType::WorldObject as u32,
                id: actor_id,
                creature_id: 0,
                creature_display_info_id: 0,
                actor_guid,
            },
        );
    }

    pub fn add_actor_creature(
        &mut self,
        actor_id: i32,
        actor_index: usize,
        actor_type: ConversationActorType,
        creature_id: u32,
        creature_display_info_id: u32,
    ) {
        self.set_actor(
            actor_index,
            ConversationActor {
                actor_type: actor_type as u32,
                id: actor_id,
                creature_id,
                creature_display_info_id,
                actor_guid: ObjectGuid::EMPTY,
            },
        );
    }

    pub fn set_line_start_time(&mut self, locale: u8, line_id: i32, start_time_ms: i32) {
        self.line_start_times
            .insert((locale, line_id), start_time_ms);
    }

    pub fn set_last_line_end_time_for_locale(&mut self, locale: usize, end_time_ms: i32) {
        if self.last_line_end_times.len() <= locale {
            self.last_line_end_times.resize(locale + 1, 0);
        }
        self.last_line_end_times[locale] = end_time_ms;
    }

    pub fn finalize_duration_from_last_line_end_times(&mut self) {
        let last_end = self
            .last_line_end_times
            .iter()
            .copied()
            .max()
            .unwrap_or_default();
        self.set_last_line_end_time(last_end);
        self.duration_ms = last_end + CONVERSATION_DESPAWN_DELAY_MS;
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.world.object().changed_object_type_mask()
            | if self.conversation_data_changes.is_any_set() {
                1 << TYPEID_CONVERSATION
            } else {
                0
            }
    }

    pub fn values_update(&self) -> ConversationValuesUpdate {
        let object_update = self.world.object().values_update();
        ConversationValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: object_update.object_data,
            conversation_data: self.conversation_data_changes.is_any_set().then(|| {
                ConversationDataUpdate {
                    mask: self.conversation_data_changes.clone(),
                    values: self.data.clone(),
                }
            }),
        }
    }

    fn set_actor(&mut self, actor_index: usize, actor: ConversationActor) {
        if self.data.actors.len() <= actor_index {
            self.data
                .actors
                .resize(actor_index + 1, ConversationActor::default());
        }
        if self.data.actors[actor_index] != actor {
            self.data.actors[actor_index] = actor;
            self.mark_conversation_data(CONVERSATION_DATA_ACTORS_BIT);
        }
    }

    fn mark_conversation_data(&mut self, bit: usize) {
        self.conversation_data_changes
            .set(CONVERSATION_DATA_PARENT_BIT);
        self.conversation_data_changes.set(bit);
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::guid::HighGuid;

    fn creator_guid() -> ObjectGuid {
        ObjectGuid::create_global(HighGuid::Player, 0, 1)
    }

    fn actor_guid() -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::Creature, 0, 1, 530, 123, 0, 99)
    }

    #[test]
    fn conversation_constructor_matches_cpp_base_state() {
        let conversation = Conversation::new();

        assert!(!conversation.world().is_world_object());
        assert_eq!(
            conversation.world().object().type_id(),
            TypeId::Conversation
        );
        assert_eq!(
            conversation.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::CONVERSATION
        );
        assert!(
            conversation
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::STATIONARY | CreateObjectFlags::CONVERSATION)
        );
        assert_eq!(conversation.creator_guid(), ObjectGuid::EMPTY);
        assert_eq!(conversation.owner_guid(), ObjectGuid::EMPTY);
        assert_eq!(conversation.faction(), 0);
        assert_eq!(conversation.duration_ms(), 0);
        assert_eq!(conversation.texture_kit_id(), 0);
        assert_eq!(
            conversation.stationary_position(),
            Position::new(0.0, 0.0, 0.0, 0.0)
        );
        assert!(conversation.data().lines.is_empty());
        assert!(conversation.data().actors.is_empty());
        assert_eq!(conversation.data().last_line_end_time, 0);
        assert!(!conversation.is_removed());
    }

    #[test]
    fn conversation_data_setters_mark_cpp_bits() {
        let mut conversation = Conversation::new();
        conversation.set_creator_guid(creator_guid());
        conversation.add_line(ConversationLine {
            conversation_line_id: 7,
            start_time: 100,
            ui_camera_id: 3,
            actor_index: 1,
            flags: 2,
        });
        conversation.add_actor_world_object(11, 1, actor_guid());
        conversation.add_actor_creature(12, 2, ConversationActorType::TalkingHead, 123, 456);
        conversation.set_last_line_end_time(1_000);

        let mask = conversation.conversation_data_changes_mask();
        assert!(mask.is_set(CONVERSATION_DATA_PARENT_BIT));
        assert!(mask.is_set(CONVERSATION_DATA_LINES_BIT));
        assert!(mask.is_set(CONVERSATION_DATA_ACTORS_BIT));
        assert!(mask.is_set(CONVERSATION_DATA_LAST_LINE_END_TIME_BIT));
        assert_eq!(conversation.creator_guid(), creator_guid());
        assert_eq!(conversation.data().actors[1].actor_guid, actor_guid());
        assert_eq!(conversation.data().actors[2].actor_type, 1);
        assert_eq!(conversation.data().actors[2].creature_id, 123);
    }

    #[test]
    fn conversation_duration_and_locale_times_follow_cpp_shape() {
        let mut conversation = Conversation::new();
        conversation.set_line_start_time(0, 7, 250);
        conversation.set_last_line_end_time_for_locale(0, 1_000);
        conversation.set_last_line_end_time_for_locale(1, 1_500);
        conversation.finalize_duration_from_last_line_end_times();

        assert_eq!(conversation.line_start_time(0, 7), Some(250));
        assert_eq!(conversation.last_line_end_time(1), Some(1_500));
        assert_eq!(conversation.data().last_line_end_time, 1_500);
        assert_eq!(
            conversation.duration_ms(),
            1_500 + CONVERSATION_DESPAWN_DELAY_MS
        );

        assert!(!conversation.update_duration(500));
        assert_eq!(conversation.duration_ms(), 11_000);
        assert!(conversation.update_duration(11_000));
        assert!(conversation.is_removed());
    }

    #[test]
    fn conversation_values_update_sets_type_bit() {
        let mut conversation = Conversation::new();
        conversation.set_last_line_end_time(1);

        let update = conversation.values_update();
        assert_eq!(update.changed_object_type_mask, 1 << TYPEID_CONVERSATION);
        assert!(update.object_data.is_none());
        assert!(update.conversation_data.is_some());
    }
}
