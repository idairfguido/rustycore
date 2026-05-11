use wow_entities::{
    ActivePlayerDataUpdate, PlayerDataUpdate, PlayerValuesUpdate, TYPEID_ACTIVE_PLAYER,
    TYPEID_PLAYER,
};
use wow_packet::packets::update::{
    ActivePlayerDataValuesUpdate as PacketActivePlayerDataValuesUpdate,
    PlayerDataValuesDeltaUpdate, UpdateObject, VisibleItemValuesUpdate,
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
        PLAYER_DATA_PARENT_BIT, Player, VisibleItemValues,
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
}
