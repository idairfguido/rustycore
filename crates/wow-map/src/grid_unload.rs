//! Grid unload helper pass.
//!
//! C++ references:
//! - `game/Grids/ObjectGridLoader.h`
//! - `game/Grids/ObjectGridLoader.cpp`
//! - `game/Maps/Map.cpp::UnloadGrid`

use wow_core::ObjectGuid;
use wow_entities::{
    AreaTrigger, Conversation, Corpse, Creature, DynamicObject, GameObject, SceneObject,
};

use crate::cell::GridObjectGuids;
use crate::grid::NGrid;
use crate::map::GridLifecycle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridObjectKind {
    Creature,
    GameObject,
    DynamicObject,
    Corpse,
    AreaTrigger,
    SceneObject,
    Conversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridUnloadAction {
    RemoveAllDynObjects(ObjectGuid),
    RemoveAllAreaTriggers(ObjectGuid),
    CombatStop(ObjectGuid),
    CreatureRespawnRelocation(ObjectGuid),
    GameObjectRespawnRelocation(ObjectGuid),
    SetDestroyedObject(GridObjectKind, ObjectGuid),
    CleanupsBeforeDelete(GridObjectKind, ObjectGuid),
    DeleteObject(GridObjectKind, ObjectGuid),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GuidGridUnloadLifecycle {
    actions: Vec<GridUnloadAction>,
}

impl GuidGridUnloadLifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn actions(&self) -> &[GridUnloadAction] {
        &self.actions
    }

    pub fn into_actions(self) -> Vec<GridUnloadAction> {
        self.actions
    }
}

impl GridLifecycle for GuidGridUnloadLifecycle {
    fn load_grid_objects(&mut self, _grid: &mut NGrid, _cell: &crate::cell::Cell) {}

    fn stop_grid_objects(&mut self, grid: &NGrid) {
        object_grid_stoper(grid, &mut self.actions);
    }

    fn evacuate_grid(&mut self, grid: &mut NGrid) {
        object_grid_evacuator(grid, &mut self.actions);
    }

    fn clean_grid(&mut self, grid: &mut NGrid) {
        object_grid_cleaner(grid, &mut self.actions);
    }

    fn unload_grid_objects(&mut self, grid: &mut NGrid) {
        object_grid_unloader(grid, &mut self.actions);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridUnloadApplyOutcome {
    Applied,
    MissingEntity,
    UnsupportedKind,
}

pub trait GridUnloadEntityStore {
    fn creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut Creature>;
    fn game_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut GameObject>;
    fn dynamic_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut DynamicObject>;
    fn corpse_mut(&mut self, guid: ObjectGuid) -> Option<&mut Corpse>;
    fn area_trigger_mut(&mut self, guid: ObjectGuid) -> Option<&mut AreaTrigger>;
    fn scene_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut SceneObject>;
    fn conversation_mut(&mut self, guid: ObjectGuid) -> Option<&mut Conversation>;
}

pub fn apply_grid_unload_action<S>(
    store: &mut S,
    action: GridUnloadAction,
) -> GridUnloadApplyOutcome
where
    S: GridUnloadEntityStore + ?Sized,
{
    match action {
        GridUnloadAction::RemoveAllDynObjects(guid) => store
            .creature_mut(guid)
            .map(|creature| creature.remove_all_dyn_objects())
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridUnloadAction::RemoveAllAreaTriggers(guid) => store
            .creature_mut(guid)
            .map(|creature| creature.remove_all_area_triggers())
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridUnloadAction::CombatStop(guid) => store
            .creature_mut(guid)
            .map(|creature| {
                if creature.is_in_combat() {
                    creature.combat_stop();
                }
            })
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridUnloadAction::CreatureRespawnRelocation(guid) => store
            .creature_mut(guid)
            .map(|creature| creature.request_respawn_relocation_from_grid_unload())
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridUnloadAction::GameObjectRespawnRelocation(guid) => store
            .game_object_mut(guid)
            .map(|game_object| game_object.request_respawn_relocation_from_grid_unload())
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridUnloadAction::SetDestroyedObject(kind, guid) => {
            apply_grid_unload_object_kind(store, kind, guid, |entity| {
                entity.set_destroyed_object(true);
            })
        }
        GridUnloadAction::CleanupsBeforeDelete(kind, guid) => {
            apply_grid_unload_object_kind(store, kind, guid, |entity| {
                entity.cleanup_before_delete();
            })
        }
        GridUnloadAction::DeleteObject(kind, guid) => {
            apply_grid_unload_object_kind(store, kind, guid, |entity| {
                entity.request_delete_from_grid_unload();
            })
        }
    }
}

pub fn apply_grid_unload_actions<S>(
    store: &mut S,
    actions: impl IntoIterator<Item = GridUnloadAction>,
) -> Vec<GridUnloadApplyOutcome>
where
    S: GridUnloadEntityStore + ?Sized,
{
    actions
        .into_iter()
        .map(|action| apply_grid_unload_action(store, action))
        .collect()
}

trait GridUnloadEntity {
    fn set_destroyed_object(&mut self, destroyed: bool);
    fn cleanup_before_delete(&mut self);
    fn request_delete_from_grid_unload(&mut self);
}

impl GridUnloadEntity for Creature {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for GameObject {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for Corpse {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for DynamicObject {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for AreaTrigger {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for SceneObject {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

impl GridUnloadEntity for Conversation {
    fn set_destroyed_object(&mut self, destroyed: bool) {
        self.set_destroyed_object(destroyed);
    }

    fn cleanup_before_delete(&mut self) {
        self.cleanup_before_delete();
    }

    fn request_delete_from_grid_unload(&mut self) {
        self.request_delete_from_grid_unload();
    }
}

fn apply_grid_unload_object_kind<S>(
    store: &mut S,
    kind: GridObjectKind,
    guid: ObjectGuid,
    apply: impl FnOnce(&mut dyn GridUnloadEntity),
) -> GridUnloadApplyOutcome
where
    S: GridUnloadEntityStore + ?Sized,
{
    match kind {
        GridObjectKind::Creature => store
            .creature_mut(guid)
            .map(|creature| apply(creature))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::GameObject => store
            .game_object_mut(guid)
            .map(|game_object| apply(game_object))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::DynamicObject => store
            .dynamic_object_mut(guid)
            .map(|dynamic_object| apply(dynamic_object))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::Corpse => store
            .corpse_mut(guid)
            .map(|corpse| apply(corpse))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::AreaTrigger => store
            .area_trigger_mut(guid)
            .map(|area_trigger| apply(area_trigger))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::SceneObject => store
            .scene_object_mut(guid)
            .map(|scene_object| apply(scene_object))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
        GridObjectKind::Conversation => store
            .conversation_mut(guid)
            .map(|conversation| apply(conversation))
            .map_or(GridUnloadApplyOutcome::MissingEntity, |_| {
                GridUnloadApplyOutcome::Applied
            }),
    }
}

pub fn object_grid_stoper(grid: &NGrid, actions: &mut Vec<GridUnloadAction>) {
    grid.visit_all_grids(|cell| {
        for guid in &cell.grid_objects.creatures {
            actions.push(GridUnloadAction::RemoveAllDynObjects(*guid));
            actions.push(GridUnloadAction::RemoveAllAreaTriggers(*guid));
            actions.push(GridUnloadAction::CombatStop(*guid));
        }
    });
}

pub fn object_grid_evacuator(grid: &NGrid, actions: &mut Vec<GridUnloadAction>) {
    grid.visit_all_grids(|cell| {
        for guid in &cell.grid_objects.creatures {
            actions.push(GridUnloadAction::CreatureRespawnRelocation(*guid));
        }

        for guid in &cell.grid_objects.gameobjects {
            actions.push(GridUnloadAction::GameObjectRespawnRelocation(*guid));
        }
    });
}

pub fn object_grid_cleaner(grid: &NGrid, actions: &mut Vec<GridUnloadAction>) {
    grid.visit_all_grids(|cell| {
        for_grid_object(
            cell.grid_objects.creatures.iter().copied(),
            GridObjectKind::Creature,
            actions,
        );
        for_grid_object(
            cell.grid_objects.gameobjects.iter().copied(),
            GridObjectKind::GameObject,
            actions,
        );
        for_grid_object(
            cell.grid_objects.dynamic_objects.iter().copied(),
            GridObjectKind::DynamicObject,
            actions,
        );
        for_grid_object(
            cell.grid_objects.corpses.iter().copied(),
            GridObjectKind::Corpse,
            actions,
        );
        for_grid_object(
            cell.grid_objects.area_triggers.iter().copied(),
            GridObjectKind::AreaTrigger,
            actions,
        );
        for_grid_object(
            cell.grid_objects.scene_objects.iter().copied(),
            GridObjectKind::SceneObject,
            actions,
        );
        for_grid_object(
            cell.grid_objects.conversations.iter().copied(),
            GridObjectKind::Conversation,
            actions,
        );
    });
}

fn for_grid_object<I>(guids: I, kind: GridObjectKind, actions: &mut Vec<GridUnloadAction>)
where
    I: IntoIterator<Item = ObjectGuid>,
{
    for guid in guids {
        actions.push(GridUnloadAction::SetDestroyedObject(kind, guid));
        actions.push(GridUnloadAction::CleanupsBeforeDelete(kind, guid));
    }
}

pub fn object_grid_unloader(grid: &mut NGrid, actions: &mut Vec<GridUnloadAction>) {
    grid.visit_all_grids_mut(|cell| {
        unload_guid_set(
            &mut cell.grid_objects.creatures,
            GridObjectKind::Creature,
            actions,
        );
        unload_guid_set(
            &mut cell.grid_objects.gameobjects,
            GridObjectKind::GameObject,
            actions,
        );
        unload_guid_set(
            &mut cell.grid_objects.dynamic_objects,
            GridObjectKind::DynamicObject,
            actions,
        );
        unload_guid_set(
            &mut cell.grid_objects.area_triggers,
            GridObjectKind::AreaTrigger,
            actions,
        );
        unload_guid_set(
            &mut cell.grid_objects.scene_objects,
            GridObjectKind::SceneObject,
            actions,
        );
        unload_guid_set(
            &mut cell.grid_objects.conversations,
            GridObjectKind::Conversation,
            actions,
        );
        cell.grid_objects.corpses.clear();
    });
}

fn unload_guid_set(
    guids: &mut std::collections::HashSet<ObjectGuid>,
    kind: GridObjectKind,
    actions: &mut Vec<GridUnloadAction>,
) {
    for guid in guids.drain() {
        actions.push(GridUnloadAction::CleanupsBeforeDelete(kind, guid));
        actions.push(GridUnloadAction::DeleteObject(kind, guid));
    }
}

pub fn grid_object_count(grid_objects: &GridObjectGuids) -> usize {
    grid_objects.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::NGrid;
    use std::collections::HashMap;
    use wow_core::guid::HighGuid;
    use wow_entities::CorpseType;

    #[derive(Default)]
    struct TestGridUnloadStore {
        creatures: HashMap<ObjectGuid, Creature>,
        game_objects: HashMap<ObjectGuid, GameObject>,
        dynamic_objects: HashMap<ObjectGuid, DynamicObject>,
        corpses: HashMap<ObjectGuid, Corpse>,
        area_triggers: HashMap<ObjectGuid, AreaTrigger>,
        scene_objects: HashMap<ObjectGuid, SceneObject>,
        conversations: HashMap<ObjectGuid, Conversation>,
    }

    impl GridUnloadEntityStore for TestGridUnloadStore {
        fn creature_mut(&mut self, guid: ObjectGuid) -> Option<&mut Creature> {
            self.creatures.get_mut(&guid)
        }

        fn game_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut GameObject> {
            self.game_objects.get_mut(&guid)
        }

        fn dynamic_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut DynamicObject> {
            self.dynamic_objects.get_mut(&guid)
        }

        fn corpse_mut(&mut self, guid: ObjectGuid) -> Option<&mut Corpse> {
            self.corpses.get_mut(&guid)
        }

        fn area_trigger_mut(&mut self, guid: ObjectGuid) -> Option<&mut AreaTrigger> {
            self.area_triggers.get_mut(&guid)
        }

        fn scene_object_mut(&mut self, guid: ObjectGuid) -> Option<&mut SceneObject> {
            self.scene_objects.get_mut(&guid)
        }

        fn conversation_mut(&mut self, guid: ObjectGuid) -> Option<&mut Conversation> {
            self.conversations.get_mut(&guid)
        }
    }

    fn guid(kind: HighGuid, counter: i64) -> ObjectGuid {
        ObjectGuid::create_world_object(kind, 0, 1, 571, 1, counter as u32, counter)
    }

    #[test]
    fn stoper_emits_creature_only_combat_cleanup_actions() {
        let creature = guid(HighGuid::Creature, 1);
        let gameobject = guid(HighGuid::GameObject, 2);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let cell = grid.get_grid_type_mut(0, 0).unwrap();
        cell.grid_objects.creatures.insert(creature);
        cell.grid_objects.gameobjects.insert(gameobject);

        let mut actions = Vec::new();
        object_grid_stoper(&grid, &mut actions);

        assert_eq!(
            actions,
            vec![
                GridUnloadAction::RemoveAllDynObjects(creature),
                GridUnloadAction::RemoveAllAreaTriggers(creature),
                GridUnloadAction::CombatStop(creature),
            ]
        );
    }

    #[test]
    fn evacuator_emits_creature_and_gameobject_respawn_relocation_actions() {
        let creature = guid(HighGuid::Creature, 1);
        let gameobject = guid(HighGuid::GameObject, 2);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let cell = grid.get_grid_type_mut(0, 0).unwrap();
        cell.grid_objects.creatures.insert(creature);
        cell.grid_objects.gameobjects.insert(gameobject);

        let mut actions = Vec::new();
        object_grid_evacuator(&grid, &mut actions);

        assert_eq!(
            actions,
            vec![
                GridUnloadAction::CreatureRespawnRelocation(creature),
                GridUnloadAction::GameObjectRespawnRelocation(gameobject),
            ]
        );
    }

    #[test]
    fn cleaner_marks_and_cleans_every_grid_object_type_in_place() {
        let creature = guid(HighGuid::Creature, 1);
        let corpse = guid(HighGuid::Corpse, 2);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let cell = grid.get_grid_type_mut(0, 0).unwrap();
        cell.grid_objects.creatures.insert(creature);
        cell.grid_objects.corpses.insert(corpse);

        let mut actions = Vec::new();
        object_grid_cleaner(&grid, &mut actions);

        assert_eq!(
            actions,
            vec![
                GridUnloadAction::SetDestroyedObject(GridObjectKind::Creature, creature),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature),
                GridUnloadAction::SetDestroyedObject(GridObjectKind::Corpse, corpse),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Corpse, corpse),
            ]
        );
    }

    #[test]
    fn unloader_deletes_non_corpse_grid_objects_and_clears_grid_sets() {
        let creature = guid(HighGuid::Creature, 1);
        let corpse = guid(HighGuid::Corpse, 2);
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        let cell = grid.get_grid_type_mut(0, 0).unwrap();
        cell.grid_objects.creatures.insert(creature);
        cell.grid_objects.corpses.insert(corpse);

        let mut actions = Vec::new();
        object_grid_unloader(&mut grid, &mut actions);

        assert_eq!(
            actions,
            vec![
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature),
                GridUnloadAction::DeleteObject(GridObjectKind::Creature, creature),
            ]
        );
        assert!(grid.get_grid_type(0, 0).unwrap().grid_objects.is_empty());
    }

    #[test]
    fn apply_set_destroyed_marks_real_creature() {
        let creature_guid = guid(HighGuid::Creature, 1);
        let mut creature = Creature::new(false);
        creature
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(creature_guid);
        let mut store = TestGridUnloadStore::default();
        store.creatures.insert(creature_guid, creature);

        let outcome = apply_grid_unload_action(
            &mut store,
            GridUnloadAction::SetDestroyedObject(GridObjectKind::Creature, creature_guid),
        );

        assert_eq!(outcome, GridUnloadApplyOutcome::Applied);
        assert!(
            store
                .creatures
                .get(&creature_guid)
                .unwrap()
                .unit()
                .world()
                .object()
                .is_destroyed_object()
        );
    }

    #[test]
    fn apply_set_destroyed_marks_real_gameobject() {
        let go_guid = guid(HighGuid::GameObject, 2);
        let mut go = GameObject::new();
        go.world_mut().object_mut().create(go_guid);
        let mut store = TestGridUnloadStore::default();
        store.game_objects.insert(go_guid, go);

        let outcome = apply_grid_unload_action(
            &mut store,
            GridUnloadAction::SetDestroyedObject(GridObjectKind::GameObject, go_guid),
        );

        assert_eq!(outcome, GridUnloadApplyOutcome::Applied);
        assert!(
            store
                .game_objects
                .get(&go_guid)
                .unwrap()
                .world()
                .object()
                .is_destroyed_object()
        );
    }

    #[test]
    fn apply_set_destroyed_marks_real_corpse() {
        let corpse_guid = guid(HighGuid::Corpse, 3);
        let mut corpse = Corpse::new_at(CorpseType::Bones, 10);
        corpse.world_mut().object_mut().create(corpse_guid);
        let mut store = TestGridUnloadStore::default();
        store.corpses.insert(corpse_guid, corpse);

        let outcome = apply_grid_unload_action(
            &mut store,
            GridUnloadAction::SetDestroyedObject(GridObjectKind::Corpse, corpse_guid),
        );

        assert_eq!(outcome, GridUnloadApplyOutcome::Applied);
        assert!(
            store
                .corpses
                .get(&corpse_guid)
                .unwrap()
                .world()
                .object()
                .is_destroyed_object()
        );
    }

    #[test]
    fn apply_stoper_actions_remove_creature_owned_dynamic_objects_and_area_triggers() {
        let creature_guid = guid(HighGuid::Creature, 4);
        let dynamic_object_guid = guid(HighGuid::DynamicObject, 5);
        let area_trigger_guid = guid(HighGuid::AreaTrigger, 6);
        let mut creature = Creature::new(false);
        creature.register_dynamic_object(dynamic_object_guid);
        creature.register_area_trigger(area_trigger_guid);
        let mut store = TestGridUnloadStore::default();
        store.creatures.insert(creature_guid, creature);

        let outcomes = apply_grid_unload_actions(
            &mut store,
            [
                GridUnloadAction::RemoveAllDynObjects(creature_guid),
                GridUnloadAction::RemoveAllAreaTriggers(creature_guid),
            ],
        );

        assert_eq!(outcomes, vec![GridUnloadApplyOutcome::Applied; 2]);
        let creature = store.creatures.get(&creature_guid).unwrap();
        assert!(creature.dynamic_objects().is_empty());
        assert_eq!(
            creature.removed_dynamic_objects_from_grid_unload(),
            &[dynamic_object_guid]
        );
        assert!(creature.area_triggers().is_empty());
        assert_eq!(
            creature.removed_area_triggers_from_grid_unload(),
            &[area_trigger_guid]
        );
    }

    #[test]
    fn apply_cleanup_and_delete_are_represented_without_panics() {
        let creature_guid = guid(HighGuid::Creature, 4);
        let mut creature = Creature::new(false);
        creature.unit_mut().world_mut().set_current_cell(11, 12);
        let go_guid = guid(HighGuid::GameObject, 5);
        let mut go = GameObject::new();
        go.world_mut().set_current_cell(13, 14);
        let dynamic_object_guid = guid(HighGuid::DynamicObject, 6);
        let mut dynamic_object = DynamicObject::new(false);
        dynamic_object.world_mut().set_current_cell(15, 16);
        let corpse_guid = guid(HighGuid::Corpse, 6);
        let mut corpse = Corpse::new_at(CorpseType::Bones, 10);
        corpse.set_cell_coord(17, 18);
        corpse.world_mut().set_current_cell(17, 18);
        let area_trigger_guid = guid(HighGuid::AreaTrigger, 7);
        let mut area_trigger = AreaTrigger::new();
        area_trigger.world_mut().set_current_cell(19, 20);
        let scene_object_guid = guid(HighGuid::SceneObject, 8);
        let mut scene_object = SceneObject::new();
        scene_object.world_mut().set_current_cell(21, 22);
        let conversation_guid = guid(HighGuid::Conversation, 9);
        let mut conversation = Conversation::new();
        conversation.world_mut().set_current_cell(23, 24);

        let mut store = TestGridUnloadStore::default();
        store.creatures.insert(creature_guid, creature);
        store.game_objects.insert(go_guid, go);
        store
            .dynamic_objects
            .insert(dynamic_object_guid, dynamic_object);
        store.corpses.insert(corpse_guid, corpse);
        store.area_triggers.insert(area_trigger_guid, area_trigger);
        store.scene_objects.insert(scene_object_guid, scene_object);
        store.conversations.insert(conversation_guid, conversation);

        let outcomes = apply_grid_unload_actions(
            &mut store,
            [
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature_guid),
                GridUnloadAction::DeleteObject(GridObjectKind::Creature, creature_guid),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::GameObject, go_guid),
                GridUnloadAction::DeleteObject(GridObjectKind::GameObject, go_guid),
                GridUnloadAction::CleanupsBeforeDelete(
                    GridObjectKind::DynamicObject,
                    dynamic_object_guid,
                ),
                GridUnloadAction::DeleteObject(GridObjectKind::DynamicObject, dynamic_object_guid),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Corpse, corpse_guid),
                GridUnloadAction::DeleteObject(GridObjectKind::Corpse, corpse_guid),
                GridUnloadAction::CleanupsBeforeDelete(
                    GridObjectKind::AreaTrigger,
                    area_trigger_guid,
                ),
                GridUnloadAction::DeleteObject(GridObjectKind::AreaTrigger, area_trigger_guid),
                GridUnloadAction::CleanupsBeforeDelete(
                    GridObjectKind::SceneObject,
                    scene_object_guid,
                ),
                GridUnloadAction::DeleteObject(GridObjectKind::SceneObject, scene_object_guid),
                GridUnloadAction::CleanupsBeforeDelete(
                    GridObjectKind::Conversation,
                    conversation_guid,
                ),
                GridUnloadAction::DeleteObject(GridObjectKind::Conversation, conversation_guid),
            ],
        );

        assert_eq!(outcomes, vec![GridUnloadApplyOutcome::Applied; 14]);
        let creature = store.creatures.get(&creature_guid).unwrap();
        assert_eq!(creature.cleanup_before_delete_count(), 1);
        assert!(creature.grid_unload_delete_requested());
        assert_eq!(creature.unit().world().current_cell(), None);
        let go = store.game_objects.get(&go_guid).unwrap();
        assert_eq!(go.cleanup_before_delete_count(), 1);
        assert!(go.grid_unload_delete_requested());
        assert_eq!(go.world().current_cell(), None);
        let dynamic_object = store.dynamic_objects.get(&dynamic_object_guid).unwrap();
        assert_eq!(dynamic_object.cleanup_before_delete_count(), 1);
        assert!(dynamic_object.grid_unload_delete_requested());
        assert_eq!(dynamic_object.world().current_cell(), None);
        let corpse = store.corpses.get(&corpse_guid).unwrap();
        assert_eq!(corpse.cleanup_before_delete_count(), 1);
        assert!(corpse.grid_unload_delete_requested());
        assert_eq!(corpse.cell_coord(), None);
        assert_eq!(corpse.world().current_cell(), None);
        let area_trigger = store.area_triggers.get(&area_trigger_guid).unwrap();
        assert_eq!(area_trigger.cleanup_before_delete_count(), 1);
        assert!(area_trigger.grid_unload_delete_requested());
        assert_eq!(area_trigger.world().current_cell(), None);
        let scene_object = store.scene_objects.get(&scene_object_guid).unwrap();
        assert_eq!(scene_object.cleanup_before_delete_count(), 1);
        assert!(scene_object.grid_unload_delete_requested());
        assert_eq!(scene_object.world().current_cell(), None);
        let conversation = store.conversations.get(&conversation_guid).unwrap();
        assert_eq!(conversation.cleanup_before_delete_count(), 1);
        assert!(conversation.grid_unload_delete_requested());
        assert_eq!(conversation.world().current_cell(), None);
    }

    #[test]
    fn lifecycle_runs_cpp_unload_order_for_normal_unload() {
        let creature = guid(HighGuid::Creature, 1);
        let mut lifecycle = GuidGridUnloadLifecycle::new();
        let mut grid = NGrid::from_coords(32, 32, 1000, true);
        grid.get_grid_type_mut(0, 0)
            .unwrap()
            .grid_objects
            .creatures
            .insert(creature);

        lifecycle.evacuate_grid(&mut grid);
        lifecycle.clean_grid(&mut grid);
        lifecycle.unload_grid_objects(&mut grid);

        assert_eq!(
            lifecycle.actions(),
            &[
                GridUnloadAction::CreatureRespawnRelocation(creature),
                GridUnloadAction::SetDestroyedObject(GridObjectKind::Creature, creature),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature),
                GridUnloadAction::CleanupsBeforeDelete(GridObjectKind::Creature, creature),
                GridUnloadAction::DeleteObject(GridObjectKind::Creature, creature),
            ]
        );
        assert!(grid.get_grid_type(0, 0).unwrap().grid_objects.is_empty());
    }
}
