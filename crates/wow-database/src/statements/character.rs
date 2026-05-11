//! Character database prepared statement definitions.
//!
//! These correspond to the `characters` database and the C# `CharStatements` enum.

use super::StatementDef;

/// Prepared statements for the character database.
///
/// Covers character list, creation, deletion, and login operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum CharStatements {
    /// SELECT c.guid, c.name, c.race, c.class, c.gender, c.level, c.zone, c.map,
    /// c.position_x, c.position_y, c.position_z, IFNULL(gm.guildid, 0), c.playerFlags,
    /// c.at_login, c.equipmentCache, c.lastLoginBuild
    /// FROM characters AS c LEFT JOIN guild_member AS gm ON c.guid = gm.guid
    /// WHERE c.account = ? AND c.deleteInfos_Name IS NULL
    SEL_ENUM,

    /// SELECT name FROM characters WHERE name = ?
    SEL_CHECK_NAME,

    /// SELECT COUNT(*) FROM characters WHERE account = ?
    SEL_SUM_CHARS,

    /// INSERT INTO characters (guid, account, name, race, class, gender, level, money,
    /// zone, map, position_x, position_y, position_z, orientation,
    /// taximask, createTime, createMode, playerFlags, at_login,
    /// health, power1, lastLoginBuild) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
    INS_CHARACTER,

    /// INSERT INTO character_customizations (guid, chrCustomizationOptionID,
    /// chrCustomizationChoiceID) VALUES (?,?,?)
    INS_CHAR_CUSTOMIZATION,

    /// DELETE FROM characters WHERE guid = ?
    DEL_CHARACTER,

    /// SELECT guid, account, name, race, class, gender, level, zone, map,
    /// position_x, position_y, position_z, orientation, playerFlags, at_login
    /// FROM characters WHERE guid = ?
    SEL_CHARACTER,

    /// UPDATE characters SET online = 1 WHERE guid = ?
    UPD_CHAR_ONLINE,

    /// UPDATE characters SET online = 0 WHERE guid = ?
    UPD_CHAR_OFFLINE,

    /// SELECT guid, account FROM characters WHERE guid = ? AND account = ?
    SEL_CHAR_DEL_CHECK,

    /// SELECT MAX(guid) FROM characters
    SEL_MAX_GUID,

    /// SELECT ci.slot, ii.itemEntry, ci.item, ii.count, ii.durability, ii.context,
    /// ii.flags, ii.playedTime, ir.paidMoney, ir.paidExtendedCost
    /// FROM character_inventory ci
    /// JOIN item_instance ii ON ci.item = ii.guid
    /// LEFT JOIN item_refund_instance ir ON ir.item_guid = ci.item AND ir.player_guid = ci.guid
    /// WHERE ci.guid = ? AND ci.bag = 0
    SEL_CHAR_EQUIPMENT,

    /// UPDATE character_inventory SET slot = ? WHERE guid = ? AND item = ?
    UPD_CHAR_INVENTORY_SLOT,

    /// DELETE FROM character_inventory WHERE guid = ? AND item = ?
    DEL_CHAR_INVENTORY_ITEM,

    /// SELECT skill FROM character_skills WHERE guid = ?
    SEL_CHARACTER_SKILLS,

    /// SELECT spell, active, disabled FROM character_spell WHERE guid = ?
    SEL_CHARACTER_SPELL,

    /// SELECT Currency, Quantity, WeeklyQuantity, TrackedQuantity,
    /// IncreasedCapQuantity, EarnedQuantity, Flags FROM character_currency
    /// WHERE CharacterGuid = ?
    SEL_PLAYER_CURRENCY,

    /// UPDATE character_currency SET Quantity = ?, WeeklyQuantity = ?,
    /// TrackedQuantity = ?, IncreasedCapQuantity = ?, EarnedQuantity = ?,
    /// Flags = ? WHERE CharacterGuid = ? AND Currency = ?
    UPD_PLAYER_CURRENCY,

    /// REPLACE INTO character_currency (CharacterGuid, Currency, Quantity,
    /// WeeklyQuantity, TrackedQuantity, IncreasedCapQuantity, EarnedQuantity, Flags)
    /// VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    REP_PLAYER_CURRENCY,

    /// SELECT button, action, type FROM character_action
    /// WHERE guid = ? AND spec = ? AND traitConfigId = ? ORDER BY button
    SEL_CHARACTER_ACTIONS_SPEC,

    /// INSERT INTO character_action (guid, spec, traitConfigId, button, action, type)
    /// VALUES (?, 0, 0, ?, ?, ?)
    INS_CHARACTER_ACTION,

    /// UPDATE characters SET totaltime = ?, leveltime = ? WHERE guid = ?
    UPD_CHAR_PLAYED_TIME,

    /// SELECT instanceId, releaseTime FROM account_instance_times WHERE accountId = ?
    SEL_ACCOUNT_INSTANCELOCKTIMES,
    /// DELETE FROM account_instance_times WHERE accountId = ?
    DEL_ACCOUNT_INSTANCE_LOCK_TIMES,
    /// INSERT INTO account_instance_times (accountId, instanceId, releaseTime) VALUES (?, ?, ?)
    INS_ACCOUNT_INSTANCE_LOCK_TIMES,

    /// SELECT instance rows used by C++ InstanceLockMgr::Load.
    SEL_INSTANCE,
    /// SELECT character_instance_lock rows used by C++ InstanceLockMgr::Load.
    SEL_CHARACTER_INSTANCE_LOCK,
    /// DELETE FROM character_instance_lock WHERE guid = ? AND mapId = ? AND lockId = ?
    DEL_CHARACTER_INSTANCE_LOCK,
    /// DELETE FROM character_instance_lock WHERE guid = ?
    DEL_CHARACTER_INSTANCE_LOCK_BY_GUID,
    /// INSERT INTO character_instance_lock C++ lock persistence row.
    INS_CHARACTER_INSTANCE_LOCK,
    /// UPDATE character_instance_lock SET extended = ? WHERE guid = ? AND mapId = ? AND lockId = ?
    UPD_CHARACTER_INSTANCE_LOCK_EXTENSION,
    /// UPDATE character_instance_lock SET expiryTime = ?, extended = 0 WHERE guid = ? AND mapId = ? AND lockId = ?
    UPD_CHARACTER_INSTANCE_LOCK_FORCE_EXPIRE,
    /// DELETE FROM instance WHERE instanceId = ?
    DEL_INSTANCE,
    /// INSERT INTO instance (instanceId, data, completedEncountersMask, entranceWorldSafeLocId) VALUES (?, ?, ?, ?)
    INS_INSTANCE,
    /// SELECT type, spawnId, respawnTime FROM respawn WHERE mapId = ? AND instanceId = ?
    SEL_RESPAWNS,
    /// REPLACE INTO respawn (type, spawnId, respawnTime, mapId, instanceId) VALUES (?, ?, ?, ?, ?)
    REP_RESPAWN,
    /// DELETE FROM respawn WHERE type = ? AND spawnId = ? AND mapId = ? AND instanceId = ?
    DEL_RESPAWN,
    /// DELETE FROM respawn WHERE mapId = ? AND instanceId = ?
    DEL_ALL_RESPAWNS,

    // Quest status
    SEL_CHAR_QUEST_STATUS,
    INS_CHAR_QUEST_STATUS,
    DEL_CHAR_QUEST_STATUS,

    /// UPDATE characters SET money = ? WHERE guid = ?
    UPD_CHAR_MONEY,
    /// UPDATE characters SET xp = ? WHERE guid = ?
    UPD_CHAR_XP,
    /// UPDATE characters SET level = ?, xp = ? WHERE guid = ?
    UPD_CHAR_LEVEL,

    /// SELECT MAX(guid) FROM item_instance
    SEL_MAX_ITEM_GUID,

    /// INSERT INTO item_instance (guid, itemEntry, owner_guid, count, durability, enchantments, charges)
    /// VALUES (?, ?, ?, ?, ?, '', '')
    INS_ITEM_INSTANCE,

    /// INSERT INTO item_instance preserving generated loot random property/context metadata.
    INS_ITEM_INSTANCE_WITH_RANDOM_CONTEXT,

    /// INSERT INTO item_instance with the C++ Item::CloneItem persisted field subset.
    INS_ITEM_INSTANCE_CLONE,

    /// UPDATE item_instance SET count = ? WHERE guid = ?
    UPD_ITEM_INSTANCE_COUNT,

    /// UPDATE item_instance SET flags = ? WHERE guid = ?
    UPD_ITEM_INSTANCE_FLAGS,

    /// SELECT entry, flags FROM character_gifts WHERE item_guid = ?
    SEL_CHARACTER_GIFT_BY_ITEM,

    /// DELETE FROM character_gifts WHERE item_guid = ?
    DEL_GIFT,

    /// UPDATE item_instance after opening a wrapped gift.
    UPD_ITEM_INSTANCE_OPEN_GIFT,

    /// INSERT INTO character_inventory (guid, bag, slot, item) VALUES (?, 0, ?, ?)
    INS_CHAR_INVENTORY,

    /// DELETE FROM item_instance WHERE guid = ?
    DEL_ITEM_INSTANCE,

    /// SELECT paidMoney, paidExtendedCost FROM item_refund_instance
    /// WHERE item_guid = ? AND player_guid = ? LIMIT 1
    SEL_ITEM_REFUNDS,

    /// SELECT bag_ci.slot, ci.slot, ii.itemEntry, ci.item, ii.count, ii.durability, ii.context,
    /// ii.flags, ii.playedTime, ir.paidMoney, ir.paidExtendedCost
    /// FROM character_inventory ci
    /// JOIN character_inventory bag_ci ON bag_ci.guid = ci.guid AND bag_ci.item = ci.bag
    /// JOIN item_instance ii ON ci.item = ii.guid
    /// LEFT JOIN item_refund_instance ir ON ir.item_guid = ci.item AND ir.player_guid = ci.guid
    /// WHERE ci.guid = ? AND bag_ci.bag = 0 AND bag_ci.slot >= 30 AND bag_ci.slot < 34
    SEL_CHAR_BAG_CONTENTS,

    /// DELETE FROM item_refund_instance WHERE item_guid = ?
    DEL_ITEM_REFUND_INSTANCE,

    /// DELETE FROM item_loot_money WHERE container_id = ?
    DEL_ITEMCONTAINER_MONEY,

    /// DELETE FROM item_loot_items WHERE container_id = ?
    DEL_ITEMCONTAINER_ITEMS,

    /// DELETE FROM item_loot_items WHERE container_id = ? AND item_id = ? AND item_count = ? AND item_index = ?
    DEL_ITEMCONTAINER_ITEM,

    /// SELECT money FROM item_loot_money WHERE container_id = ?
    SEL_ITEMCONTAINER_MONEY,

    /// INSERT INTO item_loot_money (container_id, money) VALUES (?, ?)
    INS_ITEMCONTAINER_MONEY,

    /// SELECT item_loot_items rows for one container_id.
    SEL_ITEMCONTAINER_ITEMS,

    /// INSERT INTO item_loot_items with Trinity's stored item loot shape.
    INS_ITEMCONTAINER_ITEMS,

    /// INSERT INTO item_refund_instance (item_guid, player_guid, paidMoney, paidExtendedCost)
    /// VALUES (?, ?, ?, ?)
    INS_ITEM_REFUND_INSTANCE,

    /// INSERT IGNORE INTO character_spell (guid, spell, active, disabled) VALUES (?, ?, 1, 0)
    INS_CHARACTER_SPELL,
}

impl StatementDef for CharStatements {
    fn sql(self) -> &'static str {
        match self {
            Self::SEL_ENUM => {
                "SELECT c.guid, c.name, c.race, c.class, c.gender, c.level, c.zone, c.map, \
                 c.position_x, c.position_y, c.position_z, IFNULL(gm.guildid, 0), c.playerFlags, \
                 c.at_login, c.equipmentCache, c.lastLoginBuild \
                 FROM characters AS c \
                 LEFT JOIN guild_member AS gm ON c.guid = gm.guid \
                 WHERE c.account = ? AND c.deleteInfos_Name IS NULL"
            }
            Self::SEL_CHECK_NAME => "SELECT name FROM characters WHERE name = ?",
            Self::SEL_SUM_CHARS => "SELECT COUNT(*) FROM characters WHERE account = ?",
            Self::INS_CHARACTER => {
                "INSERT INTO characters (guid, account, name, race, class, gender, level, money, \
                 zone, map, position_x, position_y, position_z, orientation, \
                 taximask, createTime, createMode, playerFlags, at_login, \
                 health, power1, lastLoginBuild) \
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
            }
            Self::INS_CHAR_CUSTOMIZATION => {
                "INSERT INTO character_customizations (guid, chrCustomizationOptionID, \
                 chrCustomizationChoiceID) VALUES (?,?,?)"
            }
            Self::DEL_CHARACTER => "DELETE FROM characters WHERE guid = ?",
            Self::SEL_CHARACTER => {
                "SELECT guid, account, name, race, class, gender, level, zone, map, \
                 position_x, position_y, position_z, orientation, playerFlags, at_login, \
                 totaltime, leveltime, money, xp \
                 FROM characters WHERE guid = ?"
            }
            Self::UPD_CHAR_ONLINE => "UPDATE characters SET online = 1 WHERE guid = ?",
            Self::UPD_CHAR_OFFLINE => "UPDATE characters SET online = 0 WHERE guid = ?",
            Self::SEL_CHAR_DEL_CHECK => {
                "SELECT guid, account FROM characters WHERE guid = ? AND account = ?"
            }
            Self::SEL_MAX_GUID => "SELECT MAX(guid) FROM characters",
            Self::SEL_CHAR_EQUIPMENT => {
                "SELECT ci.slot, ii.itemEntry, ci.item, ii.count, ii.durability, ii.context, \
                 ii.flags, ii.playedTime, ir.paidMoney, ir.paidExtendedCost \
                 FROM character_inventory ci \
                 JOIN item_instance ii ON ci.item = ii.guid \
                 LEFT JOIN item_refund_instance ir \
                   ON ir.item_guid = ci.item AND ir.player_guid = ci.guid \
                 WHERE ci.guid = ? AND ci.bag = 0"
            }
            Self::UPD_CHAR_INVENTORY_SLOT => {
                "UPDATE character_inventory SET slot = ? WHERE guid = ? AND item = ?"
            }
            Self::DEL_CHAR_INVENTORY_ITEM => {
                "DELETE FROM character_inventory WHERE guid = ? AND item = ?"
            }
            Self::SEL_CHARACTER_SKILLS => "SELECT skill FROM character_skills WHERE guid = ?",
            Self::SEL_CHARACTER_SPELL => {
                "SELECT spell, active, disabled FROM character_spell WHERE guid = ?"
            }
            Self::SEL_PLAYER_CURRENCY => {
                "SELECT Currency, Quantity, WeeklyQuantity, TrackedQuantity, \
                 IncreasedCapQuantity, EarnedQuantity, Flags \
                 FROM character_currency WHERE CharacterGuid = ?"
            }
            Self::UPD_PLAYER_CURRENCY => {
                "UPDATE character_currency SET Quantity = ?, WeeklyQuantity = ?, \
                 TrackedQuantity = ?, IncreasedCapQuantity = ?, EarnedQuantity = ?, Flags = ? \
                 WHERE CharacterGuid = ? AND Currency = ?"
            }
            Self::REP_PLAYER_CURRENCY => {
                "REPLACE INTO character_currency \
                 (CharacterGuid, Currency, Quantity, WeeklyQuantity, TrackedQuantity, \
                  IncreasedCapQuantity, EarnedQuantity, Flags) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            }
            Self::SEL_CHARACTER_ACTIONS_SPEC => {
                "SELECT button, action, type FROM character_action \
                 WHERE guid = ? AND spec = ? AND traitConfigId = ? ORDER BY button"
            }
            Self::INS_CHARACTER_ACTION => {
                "INSERT INTO character_action (guid, spec, traitConfigId, button, action, type) \
                 VALUES (?, 0, 0, ?, ?, ?)"
            }
            Self::UPD_CHAR_PLAYED_TIME => {
                "UPDATE characters SET totaltime = ?, leveltime = ? WHERE guid = ?"
            }
            Self::SEL_ACCOUNT_INSTANCELOCKTIMES => {
                "SELECT instanceId, releaseTime FROM account_instance_times WHERE accountId = ?"
            }
            Self::DEL_ACCOUNT_INSTANCE_LOCK_TIMES => {
                "DELETE FROM account_instance_times WHERE accountId = ?"
            }
            Self::INS_ACCOUNT_INSTANCE_LOCK_TIMES => {
                "INSERT INTO account_instance_times (accountId, instanceId, releaseTime) VALUES (?, ?, ?)"
            }
            Self::SEL_INSTANCE => {
                "SELECT instanceId, data, completedEncountersMask, entranceWorldSafeLocId FROM instance"
            }
            Self::SEL_CHARACTER_INSTANCE_LOCK => {
                "SELECT guid, mapId, lockId, instanceId, difficulty, data, completedEncountersMask, \
                 entranceWorldSafeLocId, expiryTime, extended FROM character_instance_lock ORDER BY instanceId"
            }
            Self::DEL_CHARACTER_INSTANCE_LOCK => {
                "DELETE FROM character_instance_lock WHERE guid = ? AND mapId = ? AND lockId = ?"
            }
            Self::DEL_CHARACTER_INSTANCE_LOCK_BY_GUID => {
                "DELETE FROM character_instance_lock WHERE guid = ?"
            }
            Self::INS_CHARACTER_INSTANCE_LOCK => {
                "INSERT INTO character_instance_lock \
                 (guid, mapId, lockId, instanceId, difficulty, data, completedEncountersMask, \
                  entranceWorldSafeLocId, expiryTime, extended) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            }
            Self::UPD_CHARACTER_INSTANCE_LOCK_EXTENSION => {
                "UPDATE character_instance_lock SET extended = ? WHERE guid = ? AND mapId = ? AND lockId = ?"
            }
            Self::UPD_CHARACTER_INSTANCE_LOCK_FORCE_EXPIRE => {
                "UPDATE character_instance_lock SET expiryTime = ?, extended = 0 WHERE guid = ? AND mapId = ? AND lockId = ?"
            }
            Self::DEL_INSTANCE => "DELETE FROM instance WHERE instanceId = ?",
            Self::INS_INSTANCE => {
                "INSERT INTO instance (instanceId, data, completedEncountersMask, entranceWorldSafeLocId) VALUES (?, ?, ?, ?)"
            }
            Self::SEL_RESPAWNS => {
                "SELECT type, spawnId, respawnTime FROM respawn WHERE mapId = ? AND instanceId = ?"
            }
            Self::REP_RESPAWN => {
                "REPLACE INTO respawn (type, spawnId, respawnTime, mapId, instanceId) VALUES (?, ?, ?, ?, ?)"
            }
            Self::DEL_RESPAWN => {
                "DELETE FROM respawn WHERE type = ? AND spawnId = ? AND mapId = ? AND instanceId = ?"
            }
            Self::DEL_ALL_RESPAWNS => "DELETE FROM respawn WHERE mapId = ? AND instanceId = ?",
            Self::UPD_CHAR_XP => "UPDATE characters SET xp = ? WHERE guid = ?",
            Self::UPD_CHAR_LEVEL => "UPDATE characters SET level = ?, xp = ? WHERE guid = ?",
            Self::UPD_CHAR_MONEY => "UPDATE characters SET money = ? WHERE guid = ?",
            Self::SEL_MAX_ITEM_GUID => "SELECT MAX(guid) FROM item_instance",
            Self::INS_ITEM_INSTANCE => {
                "INSERT INTO item_instance \
                 (guid, itemEntry, owner_guid, creatorGuid, giftCreatorGuid, count, \
                  durability, enchantments, charges, flags, randomPropertiesId, \
                  randomPropertiesSeed, context) \
                 VALUES (?, ?, ?, 0, 0, ?, ?, '', '', 0, 0, 0, 0)"
            }
            Self::INS_ITEM_INSTANCE_WITH_RANDOM_CONTEXT => {
                "INSERT INTO item_instance \
                 (guid, itemEntry, owner_guid, creatorGuid, giftCreatorGuid, count, \
                  durability, enchantments, charges, flags, randomPropertiesId, \
                  randomPropertiesSeed, context) \
                 VALUES (?, ?, ?, 0, 0, ?, ?, '', '', 0, ?, ?, ?)"
            }
            Self::INS_ITEM_INSTANCE_CLONE => {
                "INSERT INTO item_instance \
                 (guid, itemEntry, owner_guid, creatorGuid, giftCreatorGuid, count, \
                  duration, charges, flags, durability, playedTime, randomPropertiesId, \
                  randomPropertiesSeed, context) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            }
            Self::UPD_ITEM_INSTANCE_COUNT => "UPDATE item_instance SET count = ? WHERE guid = ?",
            Self::UPD_ITEM_INSTANCE_FLAGS => "UPDATE item_instance SET flags = ? WHERE guid = ?",
            Self::SEL_CHARACTER_GIFT_BY_ITEM => {
                "SELECT entry, flags FROM character_gifts WHERE item_guid = ?"
            }
            Self::DEL_GIFT => "DELETE FROM character_gifts WHERE item_guid = ?",
            Self::UPD_ITEM_INSTANCE_OPEN_GIFT => {
                "UPDATE item_instance SET itemEntry = ?, giftCreatorGuid = 0, flags = ?, durability = ? WHERE guid = ?"
            }
            Self::INS_CHAR_INVENTORY => {
                "INSERT INTO character_inventory (guid, bag, slot, item) VALUES (?, 0, ?, ?)"
            }
            Self::DEL_ITEM_INSTANCE => "DELETE FROM item_instance WHERE guid = ?",
            Self::SEL_CHAR_BAG_CONTENTS => {
                "SELECT bag_ci.slot, ci.slot, ii.itemEntry, ci.item, ii.count, ii.durability, ii.context, \
                 ii.flags, ii.playedTime, ir.paidMoney, ir.paidExtendedCost \
                 FROM character_inventory ci \
                 JOIN character_inventory bag_ci \
                   ON bag_ci.guid = ci.guid AND bag_ci.item = ci.bag \
                 JOIN item_instance ii ON ci.item = ii.guid \
                 LEFT JOIN item_refund_instance ir \
                   ON ir.item_guid = ci.item AND ir.player_guid = ci.guid \
                 WHERE ci.guid = ? AND bag_ci.bag = 0 AND ((bag_ci.slot >= 30 AND bag_ci.slot < 34) OR \
                 (bag_ci.slot >= 87 AND bag_ci.slot < 94) OR \
                 (bag_ci.slot >= 34 AND bag_ci.slot < 35))"
            }
            Self::SEL_ITEM_REFUNDS => {
                "SELECT paidMoney, paidExtendedCost \
                 FROM item_refund_instance WHERE item_guid = ? AND player_guid = ? LIMIT 1"
            }
            Self::DEL_ITEM_REFUND_INSTANCE => {
                "DELETE FROM item_refund_instance WHERE item_guid = ?"
            }
            Self::DEL_ITEMCONTAINER_MONEY => "DELETE FROM item_loot_money WHERE container_id = ?",
            Self::DEL_ITEMCONTAINER_ITEMS => "DELETE FROM item_loot_items WHERE container_id = ?",
            Self::DEL_ITEMCONTAINER_ITEM => {
                "DELETE FROM item_loot_items WHERE container_id = ? AND item_id = ? AND item_count = ? AND item_index = ?"
            }
            Self::SEL_ITEMCONTAINER_MONEY => {
                "SELECT money FROM item_loot_money WHERE container_id = ? LIMIT 1"
            }
            Self::INS_ITEMCONTAINER_MONEY => {
                "INSERT INTO item_loot_money (container_id, money) VALUES (?, ?)"
            }
            Self::SEL_ITEMCONTAINER_ITEMS => {
                "SELECT item_id, item_count, item_index, follow_rules, ffa, blocked, counted, under_threshold, needs_quest, random_properties_id, random_properties_seed, context \
                 FROM item_loot_items WHERE container_id = ? ORDER BY item_index"
            }
            Self::INS_ITEMCONTAINER_ITEMS => {
                "INSERT INTO item_loot_items \
                 (container_id, item_id, item_count, item_index, follow_rules, ffa, blocked, counted, under_threshold, needs_quest, random_properties_id, random_properties_seed, context) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            }
            Self::INS_ITEM_REFUND_INSTANCE => {
                "INSERT INTO item_refund_instance \
                 (item_guid, player_guid, paidMoney, paidExtendedCost) \
                 VALUES (?, ?, ?, ?)"
            }
            Self::INS_CHARACTER_SPELL => {
                "INSERT IGNORE INTO character_spell (guid, spell, active, disabled) VALUES (?, ?, 1, 0)"
            }
            Self::SEL_CHAR_QUEST_STATUS => {
                "SELECT quest, status, explored FROM character_queststatus WHERE guid = ?"
            }
            Self::INS_CHAR_QUEST_STATUS => {
                "INSERT INTO character_queststatus (guid, quest, status, explored, acceptTime, endTime) \
                 VALUES (?, ?, ?, 0, UNIX_TIMESTAMP(), 0) \
                 ON DUPLICATE KEY UPDATE status = ?, explored = ?"
            }
            Self::DEL_CHAR_QUEST_STATUS => {
                "DELETE FROM character_queststatus WHERE guid = ? AND quest = ?"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_statements_have_sql() {
        assert!(!CharStatements::SEL_ENUM.sql().is_empty());
        assert!(!CharStatements::SEL_CHECK_NAME.sql().is_empty());
        assert!(!CharStatements::SEL_SUM_CHARS.sql().is_empty());
        assert!(!CharStatements::INS_CHARACTER.sql().is_empty());
        assert!(!CharStatements::INS_CHAR_CUSTOMIZATION.sql().is_empty());
        assert!(!CharStatements::DEL_CHARACTER.sql().is_empty());
        assert!(!CharStatements::SEL_CHARACTER.sql().is_empty());
        assert!(!CharStatements::UPD_CHAR_ONLINE.sql().is_empty());
        assert!(!CharStatements::UPD_CHAR_OFFLINE.sql().is_empty());
        assert!(!CharStatements::SEL_CHAR_DEL_CHECK.sql().is_empty());
        assert!(!CharStatements::SEL_MAX_GUID.sql().is_empty());
        assert!(!CharStatements::SEL_PLAYER_CURRENCY.sql().is_empty());
        assert!(!CharStatements::UPD_PLAYER_CURRENCY.sql().is_empty());
        assert!(!CharStatements::REP_PLAYER_CURRENCY.sql().is_empty());
        assert!(!CharStatements::UPD_CHAR_PLAYED_TIME.sql().is_empty());
        assert!(!CharStatements::SEL_CHARACTER_INSTANCE_LOCK.sql().is_empty());
        assert!(!CharStatements::INS_CHARACTER_INSTANCE_LOCK.sql().is_empty());
        assert!(!CharStatements::INS_INSTANCE.sql().is_empty());
        assert!(!CharStatements::SEL_RESPAWNS.sql().is_empty());
        assert!(!CharStatements::REP_RESPAWN.sql().is_empty());
        assert!(!CharStatements::DEL_RESPAWN.sql().is_empty());
        assert!(!CharStatements::DEL_ALL_RESPAWNS.sql().is_empty());
    }

    #[test]
    fn char_sql_contains_expected_tables() {
        assert!(CharStatements::SEL_ENUM.sql().contains("characters"));
        assert!(CharStatements::INS_CHARACTER.sql().contains("characters"));
        assert!(
            CharStatements::INS_CHAR_CUSTOMIZATION
                .sql()
                .contains("character_customizations")
        );
        assert!(CharStatements::DEL_CHARACTER.sql().contains("characters"));
        assert!(
            CharStatements::SEL_CHARACTER_INSTANCE_LOCK
                .sql()
                .contains("character_instance_lock")
        );
        assert!(CharStatements::SEL_INSTANCE.sql().contains("instance"));
        assert!(
            CharStatements::SEL_ACCOUNT_INSTANCELOCKTIMES
                .sql()
                .contains("account_instance_times")
        );
        assert!(CharStatements::SEL_RESPAWNS.sql().contains("respawn"));
        assert!(CharStatements::DEL_ALL_RESPAWNS.sql().contains("respawn"));
    }

    #[test]
    fn char_sql_has_correct_placeholders() {
        // SEL_ENUM has 1 placeholder (account id)
        assert_eq!(CharStatements::SEL_ENUM.sql().matches('?').count(), 1);
        // SEL_ENUM should select equipmentCache and lastLoginBuild
        assert!(CharStatements::SEL_ENUM.sql().contains("equipmentCache"));
        assert!(CharStatements::SEL_ENUM.sql().contains("lastLoginBuild"));
        // SEL_CHECK_NAME has 1 placeholder
        assert_eq!(CharStatements::SEL_CHECK_NAME.sql().matches('?').count(), 1);
        // SEL_SUM_CHARS has 1 placeholder
        assert_eq!(CharStatements::SEL_SUM_CHARS.sql().matches('?').count(), 1);
        // INS_CHARACTER has 22 placeholders
        assert_eq!(CharStatements::INS_CHARACTER.sql().matches('?').count(), 22);
        // INS_CHAR_CUSTOMIZATION has 3 placeholders
        assert_eq!(
            CharStatements::INS_CHAR_CUSTOMIZATION
                .sql()
                .matches('?')
                .count(),
            3
        );
        // DEL_CHARACTER has 1 placeholder
        assert_eq!(CharStatements::DEL_CHARACTER.sql().matches('?').count(), 1);
        // SEL_CHARACTER has 1 placeholder
        assert_eq!(CharStatements::SEL_CHARACTER.sql().matches('?').count(), 1);
        // SEL_CHAR_DEL_CHECK has 2 placeholders
        assert_eq!(
            CharStatements::SEL_CHAR_DEL_CHECK
                .sql()
                .matches('?')
                .count(),
            2
        );
        // Player currency save/load statements mirror C++ CharacterDatabase.cpp.
        assert_eq!(
            CharStatements::SEL_PLAYER_CURRENCY
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::UPD_PLAYER_CURRENCY
                .sql()
                .matches('?')
                .count(),
            8
        );
        assert_eq!(
            CharStatements::REP_PLAYER_CURRENCY
                .sql()
                .matches('?')
                .count(),
            8
        );
        assert_eq!(
            CharStatements::SEL_CHAR_EQUIPMENT
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::INS_ITEM_INSTANCE_WITH_RANDOM_CONTEXT
                .sql()
                .matches('?')
                .count(),
            8
        );
        assert_eq!(
            CharStatements::INS_ITEM_INSTANCE_CLONE
                .sql()
                .matches('?')
                .count(),
            14
        );
        assert_eq!(
            CharStatements::UPD_ITEM_INSTANCE_FLAGS
                .sql()
                .matches('?')
                .count(),
            2
        );
        assert_eq!(
            CharStatements::SEL_CHARACTER_GIFT_BY_ITEM
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(CharStatements::DEL_GIFT.sql().matches('?').count(), 1);
        assert_eq!(
            CharStatements::UPD_ITEM_INSTANCE_OPEN_GIFT
                .sql()
                .matches('?')
                .count(),
            4
        );
        assert_eq!(
            CharStatements::SEL_ITEM_REFUNDS.sql().matches('?').count(),
            2
        );
        assert_eq!(
            CharStatements::SEL_CHAR_BAG_CONTENTS
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::DEL_ITEM_REFUND_INSTANCE
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::DEL_ITEMCONTAINER_MONEY
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::DEL_ITEMCONTAINER_ITEMS
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::DEL_ITEMCONTAINER_ITEM
                .sql()
                .matches('?')
                .count(),
            4
        );
        assert_eq!(
            CharStatements::SEL_ITEMCONTAINER_MONEY
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::INS_ITEMCONTAINER_MONEY
                .sql()
                .matches('?')
                .count(),
            2
        );
        assert_eq!(
            CharStatements::SEL_ITEMCONTAINER_ITEMS
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::INS_ITEMCONTAINER_ITEMS
                .sql()
                .matches('?')
                .count(),
            13
        );
        assert_eq!(
            CharStatements::INS_ITEM_REFUND_INSTANCE
                .sql()
                .matches('?')
                .count(),
            4
        );
        assert_eq!(
            CharStatements::SEL_ACCOUNT_INSTANCELOCKTIMES
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::DEL_ACCOUNT_INSTANCE_LOCK_TIMES
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::INS_ACCOUNT_INSTANCE_LOCK_TIMES
                .sql()
                .matches('?')
                .count(),
            3
        );
        assert_eq!(CharStatements::SEL_INSTANCE.sql().matches('?').count(), 0);
        assert_eq!(
            CharStatements::SEL_CHARACTER_INSTANCE_LOCK
                .sql()
                .matches('?')
                .count(),
            0
        );
        assert_eq!(
            CharStatements::DEL_CHARACTER_INSTANCE_LOCK
                .sql()
                .matches('?')
                .count(),
            3
        );
        assert_eq!(
            CharStatements::DEL_CHARACTER_INSTANCE_LOCK_BY_GUID
                .sql()
                .matches('?')
                .count(),
            1
        );
        assert_eq!(
            CharStatements::INS_CHARACTER_INSTANCE_LOCK
                .sql()
                .matches('?')
                .count(),
            10
        );
        assert_eq!(
            CharStatements::UPD_CHARACTER_INSTANCE_LOCK_EXTENSION
                .sql()
                .matches('?')
                .count(),
            4
        );
        assert_eq!(
            CharStatements::UPD_CHARACTER_INSTANCE_LOCK_FORCE_EXPIRE
                .sql()
                .matches('?')
                .count(),
            4
        );
        assert_eq!(CharStatements::DEL_INSTANCE.sql().matches('?').count(), 1);
        assert_eq!(CharStatements::INS_INSTANCE.sql().matches('?').count(), 4);
        assert_eq!(CharStatements::SEL_RESPAWNS.sql().matches('?').count(), 2);
        assert_eq!(CharStatements::REP_RESPAWN.sql().matches('?').count(), 5);
        assert_eq!(CharStatements::DEL_RESPAWN.sql().matches('?').count(), 4);
        assert_eq!(
            CharStatements::DEL_ALL_RESPAWNS.sql().matches('?').count(),
            2
        );
    }
}
