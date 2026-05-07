//! Database statement definitions.
//!
//! Each database has its own statement enum that maps variant → SQL string.
//! The [`StatementDef`] trait is implemented by each enum.

pub mod character;
pub mod hotfix;
pub mod login;
pub mod world;

pub use character::CharStatements;
pub use hotfix::HotfixStatements;
pub use login::LoginStatements;
pub use world::WorldStatements;

/// Trait implemented by database statement enums.
///
/// Each variant maps to a static SQL string via [`sql()`](Self::sql).
/// The type parameter on [`Database<S>`](crate::Database) ensures that only
/// the correct statement type can be used with each database connection.
pub trait StatementDef: Copy + Send + Sync + 'static {
    /// Get the SQL string for this statement variant.
    ///
    /// Returns an empty string for variants that have no registered SQL
    /// (should not happen in a correctly configured server).
    fn sql(self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_statements_have_sql() {
        // Verify key BNet statements have non-empty SQL
        assert!(!LoginStatements::SEL_REALMLIST.sql().is_empty());
        assert!(!LoginStatements::SEL_BNET_AUTHENTICATION.sql().is_empty());
        assert!(!LoginStatements::SEL_BNET_ACCOUNT_INFO.sql().is_empty());
        assert!(!LoginStatements::UPD_BNET_LAST_LOGIN_INFO.sql().is_empty());
        assert!(!LoginStatements::DEL_EXPIRED_IP_BANS.sql().is_empty());
        assert!(!LoginStatements::INS_ACCOUNT.sql().is_empty());
    }

    #[test]
    fn login_sql_contains_expected_tables() {
        assert!(LoginStatements::SEL_REALMLIST.sql().contains("realmlist"));
        assert!(
            LoginStatements::SEL_BNET_AUTHENTICATION
                .sql()
                .contains("battlenet_accounts")
        );
        assert!(LoginStatements::INS_IP_BANNED.sql().contains("ip_banned"));
        assert!(LoginStatements::SEL_CHECK_PASSWORD.sql().contains("salt"));
    }

    #[test]
    fn login_unregistered_statement_is_empty() {
        // SEL_BNET_ACCOUNT_SALT_BY_ID has no SQL in C# source
        assert!(
            LoginStatements::SEL_BNET_ACCOUNT_SALT_BY_ID
                .sql()
                .is_empty()
        );
    }

    #[test]
    fn world_statements_have_sql() {
        assert!(!WorldStatements::DEL_LINKED_RESPAWN.sql().is_empty());
        assert!(!WorldStatements::SEL_CREATURE_TEMPLATE.sql().is_empty());
        assert!(!WorldStatements::SEL_COMMANDS.sql().is_empty());
        assert!(!WorldStatements::INS_CREATURE.sql().is_empty());
    }

    #[test]
    fn world_sql_contains_expected_tables() {
        assert!(
            WorldStatements::SEL_CREATURE_TEXT
                .sql()
                .contains("creature_text")
        );
        assert!(
            WorldStatements::SEL_SMART_SCRIPTS
                .sql()
                .contains("smart_scripts")
        );
        assert!(WorldStatements::INS_GAME_TELE.sql().contains("game_tele"));
    }

    #[test]
    fn world_unregistered_statement_is_empty() {
        // SEL_GAMEOBJECT_TARGET has no SQL in C# source
        assert!(WorldStatements::SEL_GAMEOBJECT_TARGET.sql().is_empty());
    }

    #[test]
    fn hotfix_control_statements_match_cpp_tables() {
        assert!(
            HotfixStatements::SEL_HOTFIX_DATA
                .sql()
                .contains("hotfix_data")
        );
        assert!(
            HotfixStatements::SEL_HOTFIX_BLOB
                .sql()
                .contains("hotfix_blob")
        );
        assert!(
            HotfixStatements::SEL_HOTFIX_OPTIONAL_DATA
                .sql()
                .contains("hotfix_optional_data")
        );
    }

    #[test]
    fn world_spawn_loader_statements_match_cpp_objectmgr_tables() {
        let creature = WorldStatements::SEL_CREATURE_SPAWNS.sql();
        assert!(creature.starts_with("SELECT creature.guid, id, map"));
        assert!(creature.contains("FROM creature"));
        assert!(creature.contains("LEFT OUTER JOIN game_event_creature"));
        assert!(creature.contains("pool_members.type = 0"));
        assert_eq!(creature.matches('?').count(), 0);

        let gameobject = WorldStatements::SEL_GAMEOBJECT_SPAWNS.sql();
        assert!(gameobject.starts_with("SELECT gameobject.guid, id, map"));
        assert!(gameobject.contains("FROM gameobject"));
        assert!(gameobject.contains("LEFT OUTER JOIN game_event_gameobject"));
        assert!(gameobject.contains("pool_members.type = 1"));
        assert_eq!(gameobject.matches('?').count(), 0);

        let areatrigger = WorldStatements::SEL_AREATRIGGER_SPAWNS.sql();
        assert!(areatrigger.contains("FROM `areatrigger`"));
        assert!(areatrigger.contains("SpawnId, AreaTriggerCreatePropertiesId"));
        assert_eq!(areatrigger.matches('?').count(), 0);
    }

    #[test]
    fn world_spawn_group_statements_match_cpp_objectmgr_tables() {
        assert_eq!(
            WorldStatements::SEL_SPAWN_GROUP_TEMPLATES.sql(),
            "SELECT groupId, groupName, groupFlags FROM spawn_group_template"
        );
        assert_eq!(
            WorldStatements::SEL_SPAWN_GROUP_MEMBERS.sql(),
            "SELECT groupId, spawnType, spawnId FROM spawn_group"
        );
        assert_eq!(
            WorldStatements::SEL_INSTANCE_SPAWN_GROUPS.sql(),
            "SELECT instanceMapId, bossStateId, bossStates, spawnGroupId, flags FROM instance_spawn_groups"
        );
    }

    #[test]
    fn login_sql_has_correct_placeholders() {
        // SEL_IP_INFO has 1 placeholder
        let sql = LoginStatements::SEL_IP_INFO.sql();
        assert_eq!(sql.matches('?').count(), 1);

        // INS_IP_BANNED has 4 placeholders
        let sql = LoginStatements::INS_IP_BANNED.sql();
        assert_eq!(sql.matches('?').count(), 4);

        // INS_ACCOUNT has 7 placeholders
        let sql = LoginStatements::INS_ACCOUNT.sql();
        assert_eq!(sql.matches('?').count(), 7);
    }
}
