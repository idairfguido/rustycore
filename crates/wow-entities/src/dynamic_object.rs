use wow_constants::{TypeId, TypeMask};
use wow_core::ObjectGuid;

use crate::{
    CreateObjectFlags, ObjectDataUpdate, UpdateMask, WorldObject,
    update_fields::{DYNAMIC_OBJECT_DATA_BITS, TYPEID_DYNAMIC_OBJECT},
};

pub const DYNAMIC_OBJECT_DATA_PARENT_BIT: usize = 0;
pub const DYNAMIC_OBJECT_DATA_CASTER_BIT: usize = 1;
pub const DYNAMIC_OBJECT_DATA_TYPE_BIT: usize = 2;
pub const DYNAMIC_OBJECT_DATA_SPELL_VISUAL_ID_BIT: usize = 3;
pub const DYNAMIC_OBJECT_DATA_SPELL_ID_BIT: usize = 4;
pub const DYNAMIC_OBJECT_DATA_RADIUS_BIT: usize = 5;
pub const DYNAMIC_OBJECT_DATA_CAST_TIME_BIT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DynamicObjectType {
    Portal = 0,
    AreaSpell = 1,
    FarsightFocus = 2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicObjectDataValues {
    pub caster: ObjectGuid,
    pub dynamic_object_type: u8,
    pub spell_visual_id: i32,
    pub spell_id: i32,
    pub radius: f32,
    pub cast_time_ms: u32,
}

impl Default for DynamicObjectDataValues {
    fn default() -> Self {
        Self {
            caster: ObjectGuid::EMPTY,
            dynamic_object_type: 0,
            spell_visual_id: 0,
            spell_id: 0,
            radius: 0.0,
            cast_time_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicObjectDataUpdate {
    pub mask: UpdateMask,
    pub values: DynamicObjectDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicObjectValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub dynamic_object_data: Option<DynamicObjectDataUpdate>,
}

impl DynamicObjectValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicObject {
    world: WorldObject,
    data: DynamicObjectDataValues,
    dynamic_object_data_changes: UpdateMask,
    bound_caster: Option<ObjectGuid>,
    duration_ms: i32,
    aura_bound: bool,
    aura_removed: bool,
    aura_expired: bool,
    represented_aura_update_owner_count: u32,
    removed_aura_pending: bool,
    caster_viewpoint: bool,
    grid_unload_cleanup_before_delete_count: u32,
    grid_unload_delete_requested: bool,
}

impl DynamicObject {
    pub fn new(is_world_object: bool) -> Self {
        let mut world = WorldObject::new(
            is_world_object,
            TypeId::DynamicObject,
            TypeMask::OBJECT | TypeMask::DYNAMIC_OBJECT,
        );
        world
            .object_mut()
            .create_flags_mut()
            .insert(CreateObjectFlags::STATIONARY);

        Self {
            world,
            data: DynamicObjectDataValues::default(),
            dynamic_object_data_changes: UpdateMask::new(DYNAMIC_OBJECT_DATA_BITS),
            bound_caster: None,
            duration_ms: 0,
            aura_bound: false,
            aura_removed: false,
            aura_expired: false,
            represented_aura_update_owner_count: 0,
            removed_aura_pending: false,
            caster_viewpoint: false,
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

    pub const fn data(&self) -> &DynamicObjectDataValues {
        &self.data
    }

    pub fn dynamic_object_data_changes_mask(&self) -> &UpdateMask {
        &self.dynamic_object_data_changes
    }

    pub fn clear_dynamic_object_data_changes(&mut self) {
        self.dynamic_object_data_changes.reset_all();
    }

    pub const fn bound_caster(&self) -> Option<ObjectGuid> {
        self.bound_caster
    }

    pub const fn duration_ms(&self) -> i32 {
        self.duration_ms
    }

    pub const fn has_aura(&self) -> bool {
        self.aura_bound
    }

    pub const fn has_removed_aura_pending_delete(&self) -> bool {
        self.removed_aura_pending
    }

    pub const fn aura_is_removed_like_cpp(&self) -> bool {
        self.aura_removed
    }

    pub const fn aura_is_expired_like_cpp(&self) -> bool {
        self.aura_expired
    }

    pub const fn represented_aura_update_owner_count(&self) -> u32 {
        self.represented_aura_update_owner_count
    }

    pub const fn is_caster_viewpoint(&self) -> bool {
        self.caster_viewpoint
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

    pub const fn caster_guid(&self) -> ObjectGuid {
        self.data.caster
    }

    pub const fn creator_guid(&self) -> ObjectGuid {
        self.caster_guid()
    }

    pub const fn owner_guid(&self) -> ObjectGuid {
        self.caster_guid()
    }

    pub const fn spell_id(&self) -> i32 {
        self.data.spell_id
    }

    pub const fn radius(&self) -> f32 {
        self.data.radius
    }

    pub fn set_caster_guid(&mut self, caster: ObjectGuid) {
        self.set_guid_field(DYNAMIC_OBJECT_DATA_CASTER_BIT, caster, |data| {
            &mut data.caster
        });
    }

    pub fn set_dynamic_object_type(&mut self, dynamic_object_type: DynamicObjectType) {
        self.set_u8_field(
            DYNAMIC_OBJECT_DATA_TYPE_BIT,
            dynamic_object_type as u8,
            |data| &mut data.dynamic_object_type,
        );
    }

    pub fn set_spell_visual_id(&mut self, spell_visual_id: i32) {
        self.set_i32_field(
            DYNAMIC_OBJECT_DATA_SPELL_VISUAL_ID_BIT,
            spell_visual_id,
            |data| &mut data.spell_visual_id,
        );
    }

    pub fn set_spell_id(&mut self, spell_id: i32) {
        self.set_i32_field(DYNAMIC_OBJECT_DATA_SPELL_ID_BIT, spell_id, |data| {
            &mut data.spell_id
        });
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.set_f32_field(DYNAMIC_OBJECT_DATA_RADIUS_BIT, radius, |data| {
            &mut data.radius
        });
    }

    pub fn set_cast_time_ms(&mut self, cast_time_ms: u32) {
        self.set_u32_field(DYNAMIC_OBJECT_DATA_CAST_TIME_BIT, cast_time_ms, |data| {
            &mut data.cast_time_ms
        });
    }

    pub fn set_duration(&mut self, duration_ms: i32) {
        self.duration_ms = duration_ms;
    }

    pub fn delay(&mut self, delay_ms: i32) {
        self.set_duration(self.duration_ms - delay_ms);
    }

    pub fn update_non_aura_duration(&mut self, elapsed_ms: u32) -> bool {
        if self.aura_bound {
            return false;
        }

        if self.duration_ms > elapsed_ms as i32 {
            self.duration_ms -= elapsed_ms as i32;
            false
        } else {
            true
        }
    }

    pub fn set_aura_bound(&mut self) {
        self.aura_bound = true;
        self.aura_removed = false;
        self.aura_expired = false;
        self.represented_aura_update_owner_count = 0;
        self.removed_aura_pending = false;
    }

    pub fn set_aura_removed_like_cpp(&mut self, removed: bool) {
        self.aura_removed = removed;
    }

    pub fn set_aura_expired_like_cpp(&mut self, expired: bool) {
        self.aura_expired = expired;
    }

    pub fn update_aura_bound_like_cpp(&mut self, _elapsed_ms: u32) -> bool {
        if !self.aura_bound {
            return false;
        }

        if !self.aura_removed {
            self.represented_aura_update_owner_count =
                self.represented_aura_update_owner_count.saturating_add(1);
        }

        self.aura_bound && (self.aura_removed || self.aura_expired)
    }

    pub fn remove_aura(&mut self) {
        if self.aura_bound && !self.removed_aura_pending {
            self.aura_bound = false;
            self.aura_removed = false;
            self.aura_expired = false;
            self.removed_aura_pending = true;
        }
    }

    pub fn bind_to_caster(&mut self, caster: ObjectGuid) {
        self.bound_caster = Some(caster);
    }

    pub fn unbind_from_caster(&mut self) {
        self.bound_caster = None;
    }

    pub fn set_caster_viewpoint(&mut self) {
        self.caster_viewpoint = true;
    }

    pub fn remove_caster_viewpoint(&mut self) {
        self.caster_viewpoint = false;
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.world.object().changed_object_type_mask()
            | if self.dynamic_object_data_changes.is_any_set() {
                1 << TYPEID_DYNAMIC_OBJECT
            } else {
                0
            }
    }

    pub fn values_update(&self) -> DynamicObjectValuesUpdate {
        let object_update = self.world.object().values_update();
        DynamicObjectValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: object_update.object_data,
            dynamic_object_data: self.dynamic_object_data_changes.is_any_set().then(|| {
                DynamicObjectDataUpdate {
                    mask: self.dynamic_object_data_changes.clone(),
                    values: self.data,
                }
            }),
        }
    }

    fn set_u32_field(
        &mut self,
        bit: usize,
        value: u32,
        field: impl FnOnce(&mut DynamicObjectDataValues) -> &mut u32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_dynamic_object_data(bit);
        }
    }

    fn set_i32_field(
        &mut self,
        bit: usize,
        value: i32,
        field: impl FnOnce(&mut DynamicObjectDataValues) -> &mut i32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_dynamic_object_data(bit);
        }
    }

    fn set_u8_field(
        &mut self,
        bit: usize,
        value: u8,
        field: impl FnOnce(&mut DynamicObjectDataValues) -> &mut u8,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_dynamic_object_data(bit);
        }
    }

    fn set_f32_field(
        &mut self,
        bit: usize,
        value: f32,
        field: impl FnOnce(&mut DynamicObjectDataValues) -> &mut f32,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_dynamic_object_data(bit);
        }
    }

    fn set_guid_field(
        &mut self,
        bit: usize,
        value: ObjectGuid,
        field: impl FnOnce(&mut DynamicObjectDataValues) -> &mut ObjectGuid,
    ) {
        let target = field(&mut self.data);
        if *target != value {
            *target = value;
            self.mark_dynamic_object_data(bit);
        }
    }

    fn mark_dynamic_object_data(&mut self, bit: usize) {
        self.dynamic_object_data_changes
            .set(DYNAMIC_OBJECT_DATA_PARENT_BIT);
        self.dynamic_object_data_changes.set(bit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::guid::HighGuid;

    fn caster_guid() -> ObjectGuid {
        ObjectGuid::create_global(HighGuid::Player, 0, 1)
    }

    #[test]
    fn dynamic_object_constructor_matches_cpp_base_state() {
        let dyn_object = DynamicObject::new(false);

        assert!(!dyn_object.world().is_world_object());
        assert_eq!(dyn_object.world().object().type_id(), TypeId::DynamicObject);
        assert_eq!(
            dyn_object.world().object().type_mask(),
            TypeMask::OBJECT | TypeMask::DYNAMIC_OBJECT
        );
        assert!(
            dyn_object
                .world()
                .object()
                .create_flags()
                .contains(CreateObjectFlags::STATIONARY)
        );
        assert_eq!(dyn_object.bound_caster(), None);
        assert_eq!(dyn_object.duration_ms(), 0);
        assert!(!dyn_object.has_aura());
        assert!(!dyn_object.has_removed_aura_pending_delete());
        assert!(!dyn_object.aura_is_removed_like_cpp());
        assert!(!dyn_object.aura_is_expired_like_cpp());
        assert_eq!(dyn_object.represented_aura_update_owner_count(), 0);
        assert!(!dyn_object.is_caster_viewpoint());

        let farsight_focus = DynamicObject::new(true);
        assert!(farsight_focus.world().is_world_object());
    }

    #[test]
    fn dynamic_object_data_setters_mark_cpp_bits() {
        let mut dyn_object = DynamicObject::new(false);
        dyn_object.set_caster_guid(caster_guid());
        dyn_object.set_dynamic_object_type(DynamicObjectType::AreaSpell);
        dyn_object.set_spell_visual_id(42);
        dyn_object.set_spell_id(1337);
        dyn_object.set_radius(8.5);
        dyn_object.set_cast_time_ms(123_456);

        let mask = dyn_object.dynamic_object_data_changes_mask();
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_PARENT_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_CASTER_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_TYPE_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_SPELL_VISUAL_ID_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_SPELL_ID_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_RADIUS_BIT));
        assert!(mask.is_set(DYNAMIC_OBJECT_DATA_CAST_TIME_BIT));
        assert_eq!(dyn_object.caster_guid(), caster_guid());
        assert_eq!(dyn_object.creator_guid(), caster_guid());
        assert_eq!(dyn_object.owner_guid(), caster_guid());
        assert_eq!(dyn_object.spell_id(), 1337);
        assert_eq!(dyn_object.radius(), 8.5);
    }

    #[test]
    fn dynamic_object_duration_and_bridge_state_follow_cpp_shape() {
        let mut dyn_object = DynamicObject::new(false);
        dyn_object.set_duration(100);

        assert!(!dyn_object.update_non_aura_duration(40));
        assert_eq!(dyn_object.duration_ms(), 60);
        assert!(dyn_object.update_non_aura_duration(60));

        dyn_object.set_duration(100);
        dyn_object.delay(25);
        assert_eq!(dyn_object.duration_ms(), 75);

        dyn_object.set_aura_bound();
        assert!(dyn_object.has_aura());
        assert!(!dyn_object.update_non_aura_duration(200));
        assert!(!dyn_object.update_aura_bound_like_cpp(200));
        assert_eq!(dyn_object.represented_aura_update_owner_count(), 1);
        dyn_object.remove_aura();
        assert!(!dyn_object.has_aura());
        assert!(dyn_object.has_removed_aura_pending_delete());

        dyn_object.bind_to_caster(caster_guid());
        assert_eq!(dyn_object.bound_caster(), Some(caster_guid()));
        dyn_object.unbind_from_caster();
        assert_eq!(dyn_object.bound_caster(), None);

        dyn_object.set_caster_viewpoint();
        assert!(dyn_object.is_caster_viewpoint());
        dyn_object.remove_caster_viewpoint();
        assert!(!dyn_object.is_caster_viewpoint());
    }

    #[test]
    fn dynamic_object_aura_bound_update_owner_represented_without_duration_decrement_like_cpp() {
        let mut dyn_object = DynamicObject::new(false);
        dyn_object.set_duration(1_000);
        dyn_object.set_aura_bound();

        let expired = dyn_object.update_aura_bound_like_cpp(250);

        assert!(!expired);
        assert!(dyn_object.has_aura());
        assert_eq!(dyn_object.duration_ms(), 1_000);
        assert!(!dyn_object.aura_is_removed_like_cpp());
        assert!(!dyn_object.aura_is_expired_like_cpp());
        assert_eq!(dyn_object.represented_aura_update_owner_count(), 1);
    }

    #[test]
    fn dynamic_object_aura_bound_removed_or_expired_expires_like_cpp() {
        let mut removed = DynamicObject::new(false);
        removed.set_aura_bound();
        removed.set_aura_removed_like_cpp(true);

        assert!(removed.update_aura_bound_like_cpp(250));
        assert_eq!(removed.represented_aura_update_owner_count(), 0);

        let mut expired = DynamicObject::new(false);
        expired.set_aura_bound();
        expired.set_aura_expired_like_cpp(true);

        assert!(expired.update_aura_bound_like_cpp(250));
        assert_eq!(expired.represented_aura_update_owner_count(), 1);
    }

    #[test]
    fn dynamic_object_values_update_sets_type_bit() {
        let mut dyn_object = DynamicObject::new(false);
        dyn_object.set_spell_id(1);

        let update = dyn_object.values_update();
        assert_eq!(update.changed_object_type_mask, 1 << TYPEID_DYNAMIC_OBJECT);
        assert!(update.object_data.is_none());
        assert!(update.dynamic_object_data.is_some());
    }
}
