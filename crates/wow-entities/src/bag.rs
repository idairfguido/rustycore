use wow_constants::{ItemContext, TypeId, TypeMask};
use wow_core::ObjectGuid;

use crate::{
    Item, ItemDataUpdate, ObjectDataUpdate, UpdateMask,
    update_fields::{CONTAINER_DATA_BITS, TYPEID_CONTAINER},
};

pub const MAX_BAG_SIZE: usize = 36;
pub const NULL_SLOT: u8 = 255;

pub const CONTAINER_DATA_PARENT_BIT: usize = 0;
pub const CONTAINER_DATA_NUM_SLOTS_BIT: usize = 1;
pub const CONTAINER_DATA_SLOTS_PARENT_BIT: usize = 2;
pub const CONTAINER_DATA_SLOTS_FIRST_BIT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerDataValues {
    pub num_slots: u32,
    pub slots: [ObjectGuid; MAX_BAG_SIZE],
}

impl Default for ContainerDataValues {
    fn default() -> Self {
        Self {
            num_slots: 0,
            slots: [ObjectGuid::EMPTY; MAX_BAG_SIZE],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BagCreateInfo {
    pub guid: ObjectGuid,
    pub item_id: u32,
    pub context: ItemContext,
    pub owner: Option<ObjectGuid>,
    pub max_durability: u32,
    pub container_slots: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagCreateError {
    TooManySlots { requested: u32, max: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerDataUpdate {
    pub mask: UpdateMask,
    pub values: ContainerDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BagValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
    pub item_data: Option<ItemDataUpdate>,
    pub container_data: Option<ContainerDataUpdate>,
}

impl BagValuesUpdate {
    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bag {
    item: Item,
    data: ContainerDataValues,
    container_data_changes: UpdateMask,
    bag_slots: [Option<ObjectGuid>; MAX_BAG_SIZE],
}

impl Default for Bag {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Bag {
    pub fn new(last_played_time_update: i64) -> Self {
        let mut item = Item::new(last_played_time_update);
        item.object_mut().set_type(
            TypeId::Container,
            TypeMask::OBJECT | TypeMask::ITEM | TypeMask::CONTAINER,
        );

        Self {
            item,
            data: ContainerDataValues::default(),
            container_data_changes: UpdateMask::new(CONTAINER_DATA_BITS),
            bag_slots: [None; MAX_BAG_SIZE],
        }
    }

    pub const fn item(&self) -> &Item {
        &self.item
    }

    pub fn item_mut(&mut self) -> &mut Item {
        &mut self.item
    }

    pub const fn data(&self) -> &ContainerDataValues {
        &self.data
    }

    pub fn container_data_changes_mask(&self) -> &UpdateMask {
        &self.container_data_changes
    }

    pub fn clear_container_data_changes(&mut self) {
        self.container_data_changes.reset_all();
    }

    pub fn try_initialize_created_state(
        &mut self,
        create: BagCreateInfo,
    ) -> Result<(), BagCreateError> {
        if create.container_slots > MAX_BAG_SIZE as u32 {
            return Err(BagCreateError::TooManySlots {
                requested: create.container_slots,
                max: MAX_BAG_SIZE as u32,
            });
        }

        self.item.object_mut().create(create.guid);
        self.item.object_mut().set_entry(create.item_id);
        self.item.object_mut().set_scale(1.0);

        if let Some(owner) = create.owner {
            self.item.set_owner_guid(owner);
            self.item.set_contained_in(owner);
        }

        self.item.set_max_durability(create.max_durability);
        self.item.set_durability(create.max_durability);
        self.item.set_count(1);
        self.item.set_context(create.context);
        self.set_bag_size(create.container_slots);
        self.clear_slots();

        Ok(())
    }

    pub fn bag_size(&self) -> u32 {
        self.data.num_slots
    }

    pub fn set_bag_size(&mut self, num_slots: u32) {
        assert!(num_slots <= MAX_BAG_SIZE as u32);
        if self.data.num_slots != num_slots {
            self.data.num_slots = num_slots;
            self.mark_container_data(CONTAINER_DATA_NUM_SLOTS_BIT);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bag_slots
            .iter()
            .take(self.bag_size() as usize)
            .all(Option::is_none)
    }

    pub fn free_slots(&self) -> u32 {
        self.bag_slots
            .iter()
            .take(self.bag_size() as usize)
            .filter(|slot| slot.is_none())
            .count() as u32
    }

    pub fn item_by_pos(&self, slot: u8) -> Option<ObjectGuid> {
        if u32::from(slot) < self.bag_size() {
            self.bag_slots[slot as usize]
        } else {
            None
        }
    }

    pub fn slot_by_item_guid(&self, guid: ObjectGuid) -> u8 {
        self.bag_slots
            .iter()
            .take(self.bag_size() as usize)
            .position(|slot| *slot == Some(guid))
            .map(|slot| slot as u8)
            .unwrap_or(NULL_SLOT)
    }

    pub fn store_item(&mut self, slot: u8, item: &mut Item) {
        assert!((slot as usize) < MAX_BAG_SIZE);
        if item.object().guid() == self.item.object().guid() {
            return;
        }

        let item_guid = item.object().guid();
        self.bag_slots[slot as usize] = Some(item_guid);
        self.set_slot_guid(slot as usize, item_guid);
        item.set_contained_in(self.item.object().guid());
        item.set_owner_guid(self.item.owner_guid());
        item.set_container_guid_and_slot(self.item.object().guid(), self.item.slot());
        item.set_slot(slot);
    }

    pub fn remove_item(&mut self, slot: u8) -> Option<ObjectGuid> {
        assert!((slot as usize) < MAX_BAG_SIZE);
        let removed = self.bag_slots[slot as usize].take();
        self.set_slot_guid(slot as usize, ObjectGuid::EMPTY);
        removed
    }

    pub fn clear_slots(&mut self) {
        for slot in 0..MAX_BAG_SIZE {
            self.bag_slots[slot] = None;
            self.set_slot_guid(slot, ObjectGuid::EMPTY);
        }
    }

    pub fn changed_object_type_mask(&self) -> u32 {
        self.item.changed_object_type_mask()
            | if self.container_data_changes.is_any_set() {
                1 << TYPEID_CONTAINER
            } else {
                0
            }
    }

    pub fn values_update(&self) -> BagValuesUpdate {
        let item_update = self.item.values_update();
        BagValuesUpdate {
            changed_object_type_mask: self.changed_object_type_mask(),
            object_data: item_update.object_data,
            item_data: item_update.item_data,
            container_data: self
                .container_data_changes
                .is_any_set()
                .then(|| ContainerDataUpdate {
                    mask: self.container_data_changes.clone(),
                    values: self.data,
                }),
        }
    }

    fn set_slot_guid(&mut self, slot: usize, guid: ObjectGuid) {
        assert!(slot < MAX_BAG_SIZE);
        if self.data.slots[slot] != guid {
            self.data.slots[slot] = guid;
            self.mark_container_data_array(
                CONTAINER_DATA_SLOTS_PARENT_BIT,
                CONTAINER_DATA_SLOTS_FIRST_BIT,
                slot,
            );
        }
    }

    fn mark_container_data(&mut self, bit: usize) {
        self.container_data_changes.set(CONTAINER_DATA_PARENT_BIT);
        self.container_data_changes.set(bit);
    }

    fn mark_container_data_array(
        &mut self,
        parent_bit: usize,
        first_element_bit: usize,
        index: usize,
    ) {
        self.container_data_changes.set(parent_bit);
        self.container_data_changes.set(first_element_bit + index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::ObjectGuid;

    #[test]
    fn bag_constructor_matches_cpp_base_state() {
        let bag = Bag::new(10);

        assert_eq!(bag.item().object().type_id(), TypeId::Container);
        assert_eq!(
            bag.item().object().type_mask(),
            TypeMask::OBJECT | TypeMask::ITEM | TypeMask::CONTAINER
        );
        assert_eq!(bag.bag_size(), 0);
        assert!(bag.is_empty());
        assert_eq!(bag.free_slots(), 0);
        assert_eq!(
            bag.slot_by_item_guid(ObjectGuid::create_item(1, 1)),
            NULL_SLOT
        );
        assert!(!bag.container_data_changes_mask().is_any_set());
    }

    #[test]
    fn initialize_created_state_follows_bag_create_without_template_lookup() {
        let owner = ObjectGuid::create_player(1, 42);
        let guid = ObjectGuid::create_item(1, 90);
        let mut bag = Bag::default();

        bag.try_initialize_created_state(BagCreateInfo {
            guid,
            item_id: 4500,
            context: ItemContext::Vendor,
            owner: Some(owner),
            max_durability: 12,
            container_slots: 20,
        })
        .unwrap();

        assert_eq!(bag.item().object().guid(), guid);
        assert_eq!(bag.item().object().entry(), 4500);
        assert_eq!(bag.item().data().owner, owner);
        assert_eq!(bag.item().data().contained_in, owner);
        assert_eq!(bag.item().data().max_durability, 12);
        assert_eq!(bag.item().data().durability, 12);
        assert_eq!(bag.item().data().stack_count, 1);
        assert_eq!(bag.item().data().context, ItemContext::Vendor as i32);
        assert_eq!(bag.bag_size(), 20);
        assert_eq!(bag.free_slots(), 20);
        assert!(bag.is_empty());
        assert!(
            bag.container_data_changes_mask()
                .is_set(CONTAINER_DATA_NUM_SLOTS_BIT)
        );
    }

    #[test]
    fn too_many_container_slots_is_rejected_like_cpp_create_template_guard() {
        let mut bag = Bag::default();
        let result = bag.try_initialize_created_state(BagCreateInfo {
            guid: ObjectGuid::create_item(1, 1),
            item_id: 1,
            context: ItemContext::None,
            owner: None,
            max_durability: 0,
            container_slots: MAX_BAG_SIZE as u32 + 1,
        });

        assert_eq!(
            result,
            Err(BagCreateError::TooManySlots {
                requested: MAX_BAG_SIZE as u32 + 1,
                max: MAX_BAG_SIZE as u32
            })
        );
    }

    #[test]
    fn store_and_remove_item_updates_container_and_child_item_like_cpp() {
        let owner = ObjectGuid::create_player(1, 42);
        let mut bag = Bag::default();
        bag.try_initialize_created_state(BagCreateInfo {
            guid: ObjectGuid::create_item(1, 10),
            item_id: 100,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            container_slots: 4,
        })
        .unwrap();
        bag.item_mut().set_slot(20);

        let mut child = Item::default();
        child.initialize_created_state(crate::ItemCreateInfo {
            guid: ObjectGuid::create_item(1, 11),
            item_id: 200,
            context: ItemContext::None,
            owner: Some(owner),
            max_durability: 0,
            expiration: 0,
            spell_charges: [0; crate::MAX_ITEM_SPELLS],
        });
        child.clear_item_data_changes();

        bag.clear_container_data_changes();
        bag.store_item(2, &mut child);

        assert_eq!(bag.item_by_pos(2), Some(child.object().guid()));
        assert_eq!(bag.slot_by_item_guid(child.object().guid()), 2);
        assert_eq!(bag.free_slots(), 3);
        assert!(!bag.is_empty());
        assert_eq!(child.data().contained_in, bag.item().object().guid());
        assert_eq!(child.data().owner, owner);
        assert_eq!(child.container_guid(), bag.item().object().guid());
        assert_eq!(child.bag_slot(), 20);
        assert_eq!(child.slot(), 2);
        assert!(
            bag.container_data_changes_mask()
                .is_set(CONTAINER_DATA_SLOTS_PARENT_BIT)
        );
        assert!(
            bag.container_data_changes_mask()
                .is_set(CONTAINER_DATA_SLOTS_FIRST_BIT + 2)
        );

        let removed = bag.remove_item(2);
        assert_eq!(removed, Some(child.object().guid()));
        assert_eq!(bag.item_by_pos(2), None);
        assert_eq!(bag.data().slots[2], ObjectGuid::EMPTY);
    }

    #[test]
    fn values_update_sets_container_type_bit() {
        let mut bag = Bag::default();

        bag.set_bag_size(1);
        let update = bag.values_update();

        assert!(update.has_data());
        assert_eq!(
            update.changed_object_type_mask & (1 << TYPEID_CONTAINER),
            1 << TYPEID_CONTAINER
        );
        assert!(update.container_data.is_some());
    }
}
