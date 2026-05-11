use wow_constants::{TypeId, TypeMask};
use wow_core::{ObjectGuid, Position};

use crate::{
    CreateObjectFlags, ObjectDataUpdate, UpdateMask, WorldObject,
    update_fields::{SCENE_OBJECT_DATA_BITS, TYPEID_SCENE_OBJECT},
};

pub const SCENE_OBJECT_DATA_PARENT_BIT: usize = 0;
pub const SCENE_OBJECT_DATA_SCRIPT_PACKAGE_ID_BIT: usize = 1;
pub const SCENE_OBJECT_DATA_RND_SEED_VAL_BIT: usize = 2;
pub const SCENE_OBJECT_DATA_CREATED_BY_BIT: usize = 3;
pub const SCENE_OBJECT_DATA_SCENE_TYPE_BIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SceneType {
    Normal = 0,
    PetBattle = 1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneObjectDataValues {
    pub script_package_id: i32,
    pub rnd_seed_val: u32,
    pub created_by: ObjectGuid,
    pub scene_type: u32,
}

impl Default for SceneObjectDataValues {
    fn default() -> Self {
        Self {
            script_package_id: 0,
            rnd_seed_val: 0,
            created_by: ObjectGuid::EMPTY,
            scene_type: SceneType::Normal as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneObjectDataUpdate {
    pub mask: UpdateMask,
    pub values: SceneObjectDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneObjectValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub scene_object_data: Option<SceneObjectDataUpdate>,
}

impl SceneObjectValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject {
    world: WorldObject,
    data: SceneObjectDataValues,
    scene_object_data_changes: UpdateMask,
    stationary_position: Position,
    created_by_spell_cast: ObjectGuid,
    grid_unload_cleanup_before_delete_count: u32,
    grid_unload_delete_requested: bool,
}

impl SceneObject {
    pub fn new() -> Self {
        let mut world = WorldObject::new(
            false,
            TypeId::SceneObject,
            TypeMask::OBJECT | TypeMask::SCENE_OBJECT,
        );
        world
            .object_mut()
            .create_flags_mut()
            .insert(CreateObjectFlags::STATIONARY | CreateObjectFlags::SCENE_OBJECT);

        Self {
            world,
            data: SceneObjectDataValues::default(),
            scene_object_data_changes: UpdateMask::new(SCENE_OBJECT_DATA_BITS),
            stationary_position: Position::new(0.0, 0.0, 0.0, 0.0),
            created_by_spell_cast: ObjectGuid::EMPTY,
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

    pub const fn data(&self) -> &SceneObjectDataValues {
        &self.data
    }

    pub fn scene_object_data_changes_mask(&self) -> &UpdateMask {
        &self.scene_object_data_changes
    }

    pub fn clear_scene_object_data_changes(&mut self) {
        self.scene_object_data_changes.reset_all();
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

    pub const fn created_by_spell_cast(&self) -> ObjectGuid {
        self.created_by_spell_cast
    }

    pub fn set_created_by_spell_cast(&mut self, cast_id: ObjectGuid) {
        self.created_by_spell_cast = cast_id;
    }

    pub const fn creator_guid(&self) -> ObjectGuid {
        self.data.created_by
    }

    pub const fn owner_guid(&self) -> ObjectGuid {
        self.data.created_by
    }

    pub const fn faction(&self) -> u32 {
        0
    }

    pub fn should_be_removed(&self, creator_exists: bool, linked_aura_exists: bool) -> bool {
        if !creator_exists {
            return true;
        }

        !self.created_by_spell_cast.is_empty() && !linked_aura_exists
    }

    pub fn set_script_package_id(&mut self, script_package_id: i32) {
        self.set_i32_field(
            SCENE_OBJECT_DATA_SCRIPT_PACKAGE_ID_BIT,
            script_package_id,
            |data| &mut data.script_package_id,
        );
    }

    pub fn set_rnd_seed_val(&mut self, rnd_seed_val: u32) {
        self.set_u32_field(SCENE_OBJECT_DATA_RND_SEED_VAL_BIT, rnd_seed_val, |data| {
            &mut data.rnd_seed_val
        });
    }

    pub fn set_created_by(&mut self, created_by: ObjectGuid) {
        self.set_guid_field(SCENE_OBJECT_DATA_CREATED_BY_BIT, created_by, |data| {
            &mut data.created_by
        });
    }

    pub fn set_scene_type(&mut self, scene_type: SceneType) {
        self.set_u32_field(
            SCENE_OBJECT_DATA_SCENE_TYPE_BIT,
            scene_type as u32,
            |data| &mut data.scene_type,
        );
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.world.object().changed_object_type_mask()
            | if self.scene_object_data_changes.is_any_set() {
                1 << TYPEID_SCENE_OBJECT
            } else {
                0
            }
    }

    pub fn values_update(&self) -> SceneObjectValuesUpdate {
        let object_update = self.world.object().values_update();
        SceneObjectValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: object_update.object_data,
            scene_object_data: self.scene_object_data_changes.is_any_set().then(|| {
                SceneObjectDataUpdate {
                    mask: self.scene_object_data_changes.clone(),
                    values: self.data,
                }
            }),
        }
    }

    fn set_u32_field(
        &mut self,
        bit: usize,
        value: u32,
        field: impl FnOnce(&mut SceneObjectDataValues) -> &mut u32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_scene_object_data(bit);
        }
    }

    fn set_i32_field(
        &mut self,
        bit: usize,
        value: i32,
        field: impl FnOnce(&mut SceneObjectDataValues) -> &mut i32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_scene_object_data(bit);
        }
    }

    fn set_guid_field(
        &mut self,
        bit: usize,
        value: ObjectGuid,
        field: impl FnOnce(&mut SceneObjectDataValues) -> &mut ObjectGuid,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_scene_object_data(bit);
        }
    }

    fn mark_scene_object_data(&mut self, bit: usize) {
        self.scene_object_data_changes
            .set(SCENE_OBJECT_DATA_PARENT_BIT);
        self.scene_object_data_changes.set(bit);
    }
}

impl Default for SceneObject {
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

    fn cast_guid() -> ObjectGuid {
        ObjectGuid::create_world_object(HighGuid::Spell, 0, 1, 530, 123, 0, 99)
    }

    #[test]
    fn scene_object_constructor_matches_cpp_base_state() {
        let scene_object = SceneObject::new();

        assert!(!scene_object.world().is_world_object());
        assert_eq!(scene_object.world().object().type_id(), TypeId::SceneObject);
        assert_eq!(
            scene_object.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::SCENE_OBJECT
        );
        assert!(
            scene_object
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::STATIONARY | CreateObjectFlags::SCENE_OBJECT)
        );
        assert_eq!(
            scene_object.stationary_position(),
            Position::new(0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(scene_object.created_by_spell_cast(), ObjectGuid::EMPTY);
        assert_eq!(scene_object.creator_guid(), ObjectGuid::EMPTY);
        assert_eq!(scene_object.owner_guid(), ObjectGuid::EMPTY);
        assert_eq!(scene_object.faction(), 0);
    }

    #[test]
    fn scene_object_data_setters_mark_cpp_bits() {
        let mut scene_object = SceneObject::new();
        scene_object.set_script_package_id(123);
        scene_object.set_rnd_seed_val(456);
        scene_object.set_created_by(creator_guid());
        scene_object.set_scene_type(SceneType::PetBattle);

        let mask = scene_object.scene_object_data_changes_mask();
        assert!(mask.is_set(SCENE_OBJECT_DATA_PARENT_BIT));
        assert!(mask.is_set(SCENE_OBJECT_DATA_SCRIPT_PACKAGE_ID_BIT));
        assert!(mask.is_set(SCENE_OBJECT_DATA_RND_SEED_VAL_BIT));
        assert!(mask.is_set(SCENE_OBJECT_DATA_CREATED_BY_BIT));
        assert!(mask.is_set(SCENE_OBJECT_DATA_SCENE_TYPE_BIT));
        assert_eq!(scene_object.creator_guid(), creator_guid());
        assert_eq!(scene_object.owner_guid(), creator_guid());
    }

    #[test]
    fn scene_object_removal_predicate_matches_cpp_shape() {
        let mut scene_object = SceneObject::new();
        assert!(scene_object.should_be_removed(false, false));
        assert!(!scene_object.should_be_removed(true, false));

        scene_object.set_created_by_spell_cast(cast_guid());
        assert!(scene_object.should_be_removed(true, false));
        assert!(!scene_object.should_be_removed(true, true));
    }

    #[test]
    fn scene_object_values_update_sets_type_bit() {
        let mut scene_object = SceneObject::new();
        scene_object.set_script_package_id(1);

        let update = scene_object.values_update();
        assert_eq!(update.changed_object_type_mask, 1 << TYPEID_SCENE_OBJECT);
        assert!(update.object_data.is_none());
        assert!(update.scene_object_data.is_some());
    }
}
