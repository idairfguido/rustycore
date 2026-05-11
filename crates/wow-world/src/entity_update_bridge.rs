use wow_entities::{
    ActivePlayerDataUpdate, PlayerDataUpdate, PlayerValuesUpdate, TYPEID_ACTIVE_PLAYER,
    TYPEID_PLAYER, TYPEID_UNIT, UnitDataUpdate,
};
use wow_packet::packets::update::{
    ActivePlayerDataValuesUpdate as PacketActivePlayerDataValuesUpdate,
    PlayerDataValuesDeltaUpdate, UnitDataValuesDeltaUpdate, UpdateObject, VisibleItemValuesUpdate,
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

fn copy_mask_blocks<const N: usize>(src: &[u32], dst: &mut [u32; N]) {
    let count = src.len().min(N);
    dst[..count].copy_from_slice(&src[..count]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use wow_core::ObjectGuid;
    use wow_entities::{
        ACTIVE_PLAYER_DATA_COINAGE_BIT, ACTIVE_PLAYER_DATA_PARENT_BIT, PLAYER_DATA_FLAGS_BIT,
        PLAYER_DATA_PARENT_BIT, Player, UNIT_DATA_VIRTUAL_ITEMS_FIRST_BIT,
        UNIT_DATA_VIRTUAL_ITEMS_PARENT_BIT, VisibleItemValues,
    };
    use wow_packet::ServerPacket;

    fn mask_has(mask: &[u32], bit: usize) -> bool {
        (mask[bit / 32] & (1 << (bit % 32))) != 0
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
}
