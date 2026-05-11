use wow_entities::{
    ActivePlayerDataUpdate, BagValuesUpdate, ContainerDataUpdate, ItemDataUpdate, ItemValuesUpdate,
    ObjectDataUpdate, PlayerDataUpdate, PlayerValuesUpdate, TYPEID_ACTIVE_PLAYER, TYPEID_CONTAINER,
    TYPEID_ITEM, TYPEID_OBJECT, TYPEID_PLAYER, TYPEID_UNIT, UnitDataUpdate, UnitValuesUpdate,
};
use wow_packet::packets::update::{
    ActivePlayerDataValuesUpdate as PacketActivePlayerDataValuesUpdate, ContainerDataValuesUpdate,
    ItemBonusKeyValuesUpdate, ItemDataValuesDeltaUpdate, ItemEnchantmentValuesUpdate,
    ItemModListValuesUpdate, ItemModValuesUpdate, ObjectDataValuesUpdate,
    PlayerDataValuesDeltaUpdate, SocketedGemValuesUpdate, UnitDataValuesDeltaUpdate, UpdateObject,
    VisibleItemValuesUpdate,
};

const VISIBLE_ITEM_FULL_UPDATE_MASK: u32 = 0x0F;

pub fn player_values_update_to_packet(
    update: &PlayerValuesUpdate,
) -> Option<PlayerDataValuesDeltaUpdate> {
    let mut packet_update = PlayerDataValuesDeltaUpdate {
        changed_object_type_mask: 0,
        ..Default::default()
    };

    if let Some(player_data) = &update.player_data {
        packet_update.changed_object_type_mask |= 1 << TYPEID_PLAYER;
        copy_player_data_update(player_data, &mut packet_update);
    }

    if let Some(object_data) = &update.object_data {
        packet_update.changed_object_type_mask |= 1 << TYPEID_OBJECT;
        packet_update.object_data = Some(object_data_update_to_packet(object_data));
    }

    if let Some(unit_data) = &update.unit_data {
        packet_update.changed_object_type_mask |= 1 << TYPEID_UNIT;
        packet_update.unit_data = Some(unit_data_update_to_packet(unit_data));
    }

    if let Some(active_player_data) = &update.active_player_data {
        packet_update.changed_object_type_mask |= 1 << TYPEID_ACTIVE_PLAYER;
        packet_update.active_player_data =
            Some(active_player_data_update_to_packet(active_player_data));
    }

    (packet_update.changed_object_type_mask != 0).then_some(packet_update)
}

pub fn player_values_update_to_update_object(
    guid: wow_core::ObjectGuid,
    map_id: u16,
    update: &PlayerValuesUpdate,
) -> Option<UpdateObject> {
    player_values_update_to_packet(update)
        .map(|packet_update| UpdateObject::full_player_values_update(guid, map_id, packet_update))
}

pub fn object_values_update_to_packet(
    update: &ObjectDataUpdate,
    changed_object_type_mask: u32,
) -> ObjectDataValuesUpdate {
    object_data_update_to_packet_with_type_mask(update, changed_object_type_mask)
}

pub fn object_values_update_to_update_object(
    guid: wow_core::ObjectGuid,
    map_id: u16,
    update: &ObjectDataUpdate,
    changed_object_type_mask: u32,
) -> UpdateObject {
    UpdateObject::object_values_update(
        guid,
        map_id,
        object_values_update_to_packet(update, changed_object_type_mask),
    )
}

pub fn unit_values_update_to_packet(
    update: &UnitValuesUpdate,
) -> Option<UnitDataValuesDeltaUpdate> {
    let mut packet_update = update
        .unit_data
        .as_ref()
        .map(unit_data_update_to_packet)
        .unwrap_or_default();
    packet_update.changed_object_type_mask = update.changed_object_type_mask;
    if let Some(object_data) = &update.object_data {
        packet_update.object_data = Some(object_data_update_to_packet(object_data));
    }
    update.has_data().then_some(packet_update)
}

pub fn unit_values_update_to_update_object(
    guid: wow_core::ObjectGuid,
    map_id: u16,
    update: &UnitValuesUpdate,
) -> Option<UpdateObject> {
    unit_values_update_to_packet(update)
        .map(|packet_update| UpdateObject::unit_values_update(guid, map_id, packet_update))
}

pub fn item_values_update_to_packet(
    update: &ItemValuesUpdate,
) -> Option<ItemDataValuesDeltaUpdate> {
    let mut packet_update = update
        .item_data
        .as_ref()
        .map(item_data_update_to_packet)
        .unwrap_or_else(empty_item_values_update);
    packet_update.changed_object_type_mask = update.changed_object_type_mask;
    if let Some(object_data) = &update.object_data {
        packet_update.object_data = Some(object_data_update_to_packet(object_data));
    }
    update.has_data().then_some(packet_update)
}

pub fn item_values_update_to_update_object(
    guid: wow_core::ObjectGuid,
    map_id: u16,
    update: &ItemValuesUpdate,
) -> Option<UpdateObject> {
    item_values_update_to_packet(update)
        .map(|packet_update| UpdateObject::full_item_values_update(guid, map_id, packet_update))
}

pub fn bag_values_update_to_packet(update: &BagValuesUpdate) -> Option<ContainerDataValuesUpdate> {
    let mut packet_update = if let Some(container_data) = &update.container_data {
        container_data_update_to_packet(container_data)
    } else {
        ContainerDataValuesUpdate {
            changed_object_type_mask: update.changed_object_type_mask,
            object_data: None,
            item_data: None,
            container_data_mask: 0,
            num_slots: 0,
            slots: [wow_core::ObjectGuid::EMPTY; 36],
        }
    };
    packet_update.changed_object_type_mask = update.changed_object_type_mask;
    if let Some(object_data) = &update.object_data {
        packet_update.object_data = Some(object_data_update_to_packet(object_data));
    }
    if let Some(item_data) = &update.item_data {
        packet_update.item_data = Some(item_data_update_to_packet(item_data));
    }
    update.has_data().then_some(packet_update)
}

pub fn bag_values_update_to_update_object(
    guid: wow_core::ObjectGuid,
    map_id: u16,
    update: &BagValuesUpdate,
) -> Option<UpdateObject> {
    bag_values_update_to_packet(update)
        .map(|packet_update| UpdateObject::container_values_update(guid, map_id, packet_update))
}

fn object_data_update_to_packet(update: &ObjectDataUpdate) -> ObjectDataValuesUpdate {
    object_data_update_to_packet_with_type_mask(update, 1 << TYPEID_OBJECT)
}

fn object_data_update_to_packet_with_type_mask(
    update: &ObjectDataUpdate,
    changed_object_type_mask: u32,
) -> ObjectDataValuesUpdate {
    ObjectDataValuesUpdate {
        changed_object_type_mask,
        object_data_mask: update.mask.blocks().first().copied().unwrap_or(0),
        entry_id: update.values.entry_id,
        dynamic_flags: update.values.dynamic_flags,
        scale: update.values.scale,
    }
}

fn copy_player_data_update(
    update: &PlayerDataUpdate,
    packet_update: &mut PlayerDataValuesDeltaUpdate,
) {
    copy_mask_blocks(update.mask.blocks(), &mut packet_update.player_data_mask);
    packet_update.loot_target_guid = update.values.loot_target_guid;
    packet_update.player_flags = update.values.player_flags;
    packet_update.player_flags_ex = update.values.player_flags_ex;
    packet_update.num_bank_slots = update.values.num_bank_slots;
    packet_update.native_sex = update.values.native_sex;
    packet_update.current_spec_id = update.values.current_spec_id;

    for (dst, src) in packet_update
        .visible_items
        .iter_mut()
        .zip(update.values.visible_items.iter())
    {
        *dst = VisibleItemValuesUpdate {
            visible_item_mask: VISIBLE_ITEM_FULL_UPDATE_MASK,
            item_id: src.item_id,
            appearance_mod_id: src.item_appearance_mod_id,
            item_visual: src.item_visual,
        };
    }
}

fn unit_data_update_to_packet(update: &UnitDataUpdate) -> UnitDataValuesDeltaUpdate {
    let mut packet_update = UnitDataValuesDeltaUpdate::default();
    copy_mask_blocks(update.mask.blocks(), &mut packet_update.unit_data_mask);
    packet_update.health = update.values.health.min(i64::MAX as u64) as i64;
    packet_update.max_health = update.values.max_health.min(i64::MAX as u64) as i64;
    packet_update.display_id = update.values.display_id;
    packet_update.target = update.values.target;
    packet_update.race = update.values.race;
    packet_update.class_id = update.values.class_id;
    packet_update.player_class_id = update.values.player_class_id;
    packet_update.sex = update.values.sex;
    packet_update.display_power = update.values.display_power;
    packet_update.level = update.values.level;
    packet_update.faction_template = update.values.faction_template;
    packet_update.flags = update.values.flags;
    packet_update.flags2 = update.values.flags2;
    packet_update.flags3 = update.values.flags3;
    packet_update.bounding_radius = update.values.bounding_radius;
    packet_update.combat_reach = update.values.combat_reach;
    packet_update.display_scale = update.values.display_scale;
    packet_update.native_display_id = update.values.native_display_id;
    packet_update.native_display_scale = update.values.native_display_scale;
    packet_update.power = update.values.power;
    packet_update.max_power = update.values.max_power;

    for (dst, src) in packet_update
        .virtual_items
        .iter_mut()
        .zip(update.values.virtual_items.iter())
    {
        *dst = VisibleItemValuesUpdate {
            visible_item_mask: VISIBLE_ITEM_FULL_UPDATE_MASK,
            item_id: src.item_id,
            appearance_mod_id: src.item_appearance_mod_id,
            item_visual: src.item_visual,
        };
    }

    packet_update
}

fn item_data_update_to_packet(update: &ItemDataUpdate) -> ItemDataValuesDeltaUpdate {
    let mut spell_charges = [0; 5];
    spell_charges.copy_from_slice(&update.values.spell_charges);

    let mut enchantments = [ItemEnchantmentValuesUpdate::default(); 13];
    for (dst, src) in enchantments
        .iter_mut()
        .zip(update.values.enchantments.iter())
    {
        *dst = ItemEnchantmentValuesUpdate {
            item_enchantment_mask: 0x1F,
            id: src.id,
            duration: src.duration,
            charges: src.charges,
            field_a: src.field_a,
            field_b: src.field_b,
        };
    }

    let modifiers = ItemModListValuesUpdate {
        item_mod_list_mask: 0x01,
        values: update
            .values
            .modifiers
            .iter()
            .enumerate()
            .map(|(index, value)| ItemModValuesUpdate {
                value: *value as i32,
                item_mod_type: index as u8,
            })
            .collect(),
        values_update_mask: None,
    };

    ItemDataValuesDeltaUpdate {
        changed_object_type_mask: 1 << TYPEID_ITEM,
        item_data_mask: mask_to_u64(update.mask.blocks()),
        artifact_powers: update
            .values
            .artifact_powers
            .iter()
            .map(
                |power| wow_packet::packets::update::ArtifactPowerValuesUpdate {
                    artifact_power_id: power.artifact_power_id,
                    purchased_rank: power.purchased_rank,
                    current_rank_with_bonus: power.current_rank_with_bonus,
                },
            )
            .collect(),
        artifact_powers_update_mask: None,
        gems: update
            .values
            .gems
            .iter()
            .map(|gem| {
                let mut bonus_list_ids = [0; 16];
                for (dst, src) in bonus_list_ids.iter_mut().zip(gem.bonus_list_ids.iter()) {
                    *dst = *src;
                }
                SocketedGemValuesUpdate {
                    socketed_gem_mask: 0x07,
                    item_id: gem.item_id,
                    context: gem.context,
                    bonus_list_ids,
                }
            })
            .collect(),
        gems_update_mask: None,
        owner: update.values.owner,
        contained_in: update.values.contained_in,
        creator: update.values.creator,
        gift_creator: update.values.gift_creator,
        stack_count: update.values.stack_count,
        expiration: update.values.expiration,
        dynamic_flags: update.values.dynamic_flags,
        property_seed: update.values.property_seed,
        random_properties_id: update.values.random_properties_id,
        durability: update.values.durability,
        max_durability: update.values.max_durability,
        create_played_time: update.values.create_played_time,
        context: update.values.context,
        create_time: update.values.create_time,
        artifact_xp: update.values.artifact_xp,
        item_appearance_mod_id: update.values.item_appearance_mod_id,
        modifiers,
        dynamic_flags2: update.values.dynamic_flags2,
        item_bonus_key: ItemBonusKeyValuesUpdate {
            item_id: update.values.item_bonus_key.item_id,
            bonus_list_ids: update.values.item_bonus_key.bonus_list_ids.clone(),
        },
        debug_item_level: update.values.debug_item_level,
        spell_charges,
        enchantments,
        ..empty_item_values_update()
    }
}

fn empty_item_values_update() -> ItemDataValuesDeltaUpdate {
    ItemDataValuesDeltaUpdate {
        changed_object_type_mask: 0,
        object_data: None,
        item_data_mask: 0,
        artifact_powers: Vec::new(),
        artifact_powers_update_mask: None,
        gems: Vec::new(),
        gems_update_mask: None,
        owner: wow_core::ObjectGuid::EMPTY,
        contained_in: wow_core::ObjectGuid::EMPTY,
        creator: wow_core::ObjectGuid::EMPTY,
        gift_creator: wow_core::ObjectGuid::EMPTY,
        stack_count: 0,
        expiration: 0,
        dynamic_flags: 0,
        property_seed: 0,
        random_properties_id: 0,
        durability: 0,
        max_durability: 0,
        create_played_time: 0,
        context: 0,
        create_time: 0,
        artifact_xp: 0,
        item_appearance_mod_id: 0,
        modifiers: ItemModListValuesUpdate {
            item_mod_list_mask: 0,
            values: Vec::new(),
            values_update_mask: None,
        },
        dynamic_flags2: 0,
        item_bonus_key: ItemBonusKeyValuesUpdate::default(),
        debug_item_level: 0,
        spell_charges: [0; 5],
        enchantments: [ItemEnchantmentValuesUpdate::default(); 13],
    }
}

fn container_data_update_to_packet(update: &ContainerDataUpdate) -> ContainerDataValuesUpdate {
    ContainerDataValuesUpdate {
        changed_object_type_mask: 1 << TYPEID_CONTAINER,
        object_data: None,
        item_data: None,
        container_data_mask: mask_to_u64(update.mask.blocks()),
        num_slots: update.values.num_slots,
        slots: update.values.slots,
    }
}

fn active_player_data_update_to_packet(
    update: &ActivePlayerDataUpdate,
) -> PacketActivePlayerDataValuesUpdate {
    let mut packet_update = PacketActivePlayerDataValuesUpdate::default();
    copy_mask_blocks(
        update.mask.blocks(),
        &mut packet_update.active_player_data_mask,
    );
    packet_update.coinage = update.values.coinage;
    packet_update.xp = update.values.xp;
    packet_update.next_level_xp = update.values.next_level_xp;
    packet_update.character_points = update.values.character_points;
    packet_update.num_backpack_slots = update.values.num_backpack_slots;
    packet_update
        .inv_slots
        .copy_from_slice(&update.values.inv_slots);
    packet_update.buyback_price = update.values.buyback_price;
    packet_update.buyback_timestamp = update.values.buyback_timestamp;
    packet_update
}

fn mask_to_u64(blocks: &[u32]) -> u64 {
    blocks
        .iter()
        .take(2)
        .enumerate()
        .fold(0u64, |acc, (index, block)| {
            acc | ((*block as u64) << (index * 32))
        })
}

fn copy_mask_blocks<const N: usize>(src: &[u32], dst: &mut [u32; N]) {
    let count = src.len().min(N);
    dst[..count].copy_from_slice(&src[..count]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::ObjectGuid;
    use wow_entities::{
        ACTIVE_PLAYER_DATA_COINAGE_BIT, ACTIVE_PLAYER_DATA_PARENT_BIT, Bag,
        CONTAINER_DATA_NUM_SLOTS_BIT, ITEM_DATA_STACK_COUNT_BIT, Item, PLAYER_DATA_FLAGS_BIT,
        PLAYER_DATA_PARENT_BIT, Player, UNIT_DATA_HEALTH_BIT, UNIT_DATA_PARENT_BIT,
        UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT, UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT, VisibleItemValues,
    };
    use wow_packet::ServerPacket;

    fn mask_has(mask: &[u32], bit: usize) -> bool {
        (mask[bit / 32] & (1 << (bit % 32))) != 0
    }

    fn mask_has_u64(mask: u64, bit: usize) -> bool {
        (mask & (1u64 << bit)) != 0
    }

    #[test]
    fn bridges_active_player_money_update_from_entity_mask() {
        let mut player = Player::new(Some(7), false);
        player.clear_data_changes();
        player.set_money(123_456);

        let update = player.values_update(true);
        let packet_update = player_values_update_to_packet(&update).unwrap();
        let active = packet_update.active_player_data.unwrap();

        assert_eq!(
            packet_update.changed_object_type_mask,
            1 << TYPEID_ACTIVE_PLAYER
        );
        assert!(mask_has(
            &active.active_player_data_mask,
            ACTIVE_PLAYER_DATA_PARENT_BIT
        ));
        assert!(mask_has(
            &active.active_player_data_mask,
            ACTIVE_PLAYER_DATA_COINAGE_BIT
        ));
        assert_eq!(active.coinage, 123_456);
    }

    #[test]
    fn bridges_player_and_active_player_values_without_unit_bits() {
        let mut player = Player::new(Some(7), false);
        player.clear_data_changes();
        player.set_player_flag(0x20);
        player.set_visible_item_slot(
            0,
            Some(VisibleItemValues {
                item_id: 25,
                item_appearance_mod_id: 3,
                item_visual: 4,
            }),
        );
        player.set_money(42);

        let update = player.values_update(true);
        let packet_update = player_values_update_to_packet(&update).unwrap();

        assert_eq!(
            packet_update.changed_object_type_mask,
            (1 << TYPEID_PLAYER) | (1 << TYPEID_ACTIVE_PLAYER)
        );
        assert!(mask_has(
            &packet_update.player_data_mask,
            PLAYER_DATA_PARENT_BIT
        ));
        assert!(mask_has(
            &packet_update.player_data_mask,
            PLAYER_DATA_FLAGS_BIT
        ));
        assert_eq!(packet_update.player_flags, 0x20);
        assert_eq!(packet_update.visible_items[0].visible_item_mask, 0x0F);
        assert_eq!(packet_update.visible_items[0].item_id, 25);
        assert_eq!(
            packet_update.active_player_data.as_ref().unwrap().coinage,
            42
        );
    }

    #[test]
    fn builds_update_object_from_entity_player_values_update() {
        let mut player = Player::new(Some(7), false);
        player
            .unit_mut()
            .world_mut()
            .object_mut()
            .create(ObjectGuid::create_uniq(0x42));
        player.clear_data_changes();
        player.set_money(1234);

        let update = player.values_update(true);
        let packet = player_values_update_to_update_object(player.guid(), 571, &update).unwrap();
        let bytes = packet.to_bytes();

        assert!(!bytes.is_empty());
        assert!(
            bytes
                .windows(1234u64.to_le_bytes().len())
                .any(|window| window == 1234u64.to_le_bytes())
        );
    }

    #[test]
    fn bridges_player_object_data_like_cpp_values_prefix() {
        let mut player = Player::new(Some(7), false);
        player.clear_data_changes();
        player.unit_mut().world_mut().object_mut().set_entry(42);

        let update = player.values_update(true);
        let packet_update = player_values_update_to_packet(&update).unwrap();
        let object_data = packet_update.object_data.unwrap();

        assert_eq!(packet_update.changed_object_type_mask, 1 << TYPEID_OBJECT);
        assert_eq!(object_data.changed_object_type_mask, 1 << TYPEID_OBJECT);
        assert_eq!(object_data.entry_id, 42);
    }

    #[test]
    fn bridges_unit_object_and_unit_values() {
        let mut unit = wow_entities::Unit::new(true);
        unit.world_mut().object_mut().set_entry(99);
        unit.set_max_health(123);
        unit.set_health(123);

        let update = unit.values_update();
        let packet_update = unit_values_update_to_packet(&update).unwrap();

        assert_eq!(
            packet_update.changed_object_type_mask,
            (1 << TYPEID_OBJECT) | (1 << TYPEID_UNIT)
        );
        assert_eq!(packet_update.object_data.unwrap().entry_id, 99);
        assert!(mask_has(
            &packet_update.unit_data_mask,
            UNIT_DATA_PARENT_BIT
        ));
        assert!(mask_has(
            &packet_update.unit_data_mask,
            UNIT_DATA_HEALTH_BIT
        ));
        assert_eq!(packet_update.health, 123);
    }

    #[test]
    fn bridges_item_object_only_and_item_data() {
        let mut item = Item::new(0);
        item.object_mut().set_entry(6948);

        let object_update = item_values_update_to_packet(&item.values_update()).unwrap();
        assert_eq!(object_update.changed_object_type_mask, 1 << TYPEID_OBJECT);
        assert_eq!(object_update.object_data.unwrap().entry_id, 6948);
        assert_eq!(object_update.item_data_mask, 0);

        item.clear_item_data_changes();
        item.object_mut().clear_update_mask(false);
        item.set_count(5);

        let item_update = item_values_update_to_packet(&item.values_update()).unwrap();
        assert_eq!(item_update.changed_object_type_mask, 1 << TYPEID_ITEM);
        assert!(mask_has_u64(
            item_update.item_data_mask,
            ITEM_DATA_STACK_COUNT_BIT
        ));
        assert_eq!(item_update.stack_count, 5);
    }

    #[test]
    fn bridges_bag_container_values_with_item_base() {
        let mut bag = Bag::new(0);
        bag.item_mut().object_mut().set_entry(4242);
        bag.set_bag_size(16);

        let packet_update = bag_values_update_to_packet(&bag.values_update()).unwrap();

        assert_eq!(
            packet_update.changed_object_type_mask,
            (1 << TYPEID_OBJECT) | (1 << TYPEID_CONTAINER)
        );
        assert_eq!(packet_update.object_data.unwrap().entry_id, 4242);
        assert!(mask_has_u64(
            packet_update.container_data_mask,
            CONTAINER_DATA_NUM_SLOTS_BIT
        ));
        assert_eq!(packet_update.num_slots, 16);
    }

    #[test]
    fn bridges_unit_virtual_items_from_player_values_update() {
        let mut player = Player::new(Some(7), false);
        player.clear_data_changes();
        player.unit_mut().set_virtual_item(
            2,
            Some(VisibleItemValues {
                item_id: 25,
                item_appearance_mod_id: 3,
                item_visual: 4,
            }),
        );

        let update = player.values_update(true);
        let packet_update = player_values_update_to_packet(&update).unwrap();
        let unit = packet_update.unit_data.unwrap();

        assert_eq!(packet_update.changed_object_type_mask, 1 << TYPEID_UNIT);
        assert!(mask_has(
            &unit.unit_data_mask,
            UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT
        ));
        assert!(mask_has(
            &unit.unit_data_mask,
            UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT + 2
        ));
        assert_eq!(unit.virtual_items[2].visible_item_mask, 0x0F);
        assert_eq!(unit.virtual_items[2].item_id, 25);
        assert_eq!(unit.virtual_items[2].appearance_mod_id, 3);
        assert_eq!(unit.virtual_items[2].item_visual, 4);
    }

    #[test]
    fn bridges_forced_default_value_deltas() {
        let mut player = Player::new(Some(7), false);
        player.clear_data_changes();
        player.mark_inv_slot_changed(0);
        player.mark_visible_item_slot_changed(0);
        player.unit_mut().mark_virtual_item_changed(0);

        let update = player.values_update(true);
        let packet_update = player_values_update_to_packet(&update).unwrap();
        let active = packet_update.active_player_data.unwrap();
        let unit = packet_update.unit_data.unwrap();

        assert!(mask_has(
            &active.active_player_data_mask,
            wow_entities::ACTIVE_PLAYER_DATA_INV_SLOTS_FIRST_BIT
        ));
        assert!(mask_has(
            &packet_update.player_data_mask,
            wow_entities::PLAYER_DATA_VISIBLE_ITEMS_FIRST_BIT
        ));
        assert!(mask_has(
            &unit.unit_data_mask,
            wow_entities::UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT
        ));
        assert_eq!(active.inv_slots[0], ObjectGuid::EMPTY);
        assert_eq!(packet_update.visible_items[0].item_id, 0);
        assert_eq!(unit.virtual_items[0].item_id, 0);
    }
}
