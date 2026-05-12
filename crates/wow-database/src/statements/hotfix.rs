//! Hotfix database prepared statement definitions.
//!
//! These correspond to the `hotfixes` database used by C++ `DB2Manager`.

use super::StatementDef;

/// Prepared statements for the hotfix database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum HotfixStatements {
    /// C++ `HOTFIX_SEL_AREA_TABLE`.
    SEL_AREA_TABLE,
    /// `DB2Manager::LoadHotfixData`.
    SEL_HOTFIX_DATA,
    /// `DB2Manager::LoadHotfixBlob`.
    SEL_HOTFIX_BLOB,
    /// `DB2Manager::LoadHotfixOptionalData`.
    SEL_HOTFIX_OPTIONAL_DATA,
    /// C++ `HOTFIX_SEL_PHASE`.
    SEL_PHASE,
    /// C++ `HOTFIX_SEL_PHASE_X_PHASE_GROUP`.
    SEL_PHASE_X_PHASE_GROUP,
    /// C++ `HOTFIX_SEL_UI_MAP_X_MAP_ART`.
    SEL_UI_MAP_X_MAP_ART,
}

impl StatementDef for HotfixStatements {
    fn sql(self) -> &'static str {
        match self {
            Self::SEL_AREA_TABLE => concat!(
                "SELECT ID, ZoneName, AreaName, ContinentID, ParentAreaID, AreaBit, SoundProviderPref, ",
                "SoundProviderPrefUnderwater, AmbienceID, UwAmbience, ZoneMusic, UwZoneMusic, ExplorationLevel, IntroSound, UwIntroSound, FactionGroupMask, ",
                "AmbientMultiplier, MountFlags, PvpCombatWorldStateID, WildBattlePetLevelMin, WildBattlePetLevelMax, WindSettingsID, Flags1, Flags2, ",
                "LiquidTypeID1, LiquidTypeID2, LiquidTypeID3, LiquidTypeID4 FROM area_table WHERE VerifiedBuild > 0"
            ),
            Self::SEL_HOTFIX_DATA => {
                "SELECT Id, UniqueId, TableHash, RecordId, Status FROM hotfix_data ORDER BY Id"
            }
            Self::SEL_HOTFIX_BLOB => {
                "SELECT TableHash, RecordId, locale, `Blob` FROM hotfix_blob ORDER BY TableHash"
            }
            Self::SEL_HOTFIX_OPTIONAL_DATA => {
                "SELECT TableHash, RecordId, locale, `Key`, `Data` FROM hotfix_optional_data ORDER BY TableHash"
            }
            Self::SEL_PHASE => "SELECT ID, Flags FROM phase WHERE VerifiedBuild > 0",
            Self::SEL_PHASE_X_PHASE_GROUP => {
                "SELECT ID, PhaseID, PhaseGroupID FROM phase_x_phase_group WHERE VerifiedBuild > 0"
            }
            Self::SEL_UI_MAP_X_MAP_ART => {
                "SELECT ID, PhaseID, UiMapArtID, UiMapID FROM ui_map_x_map_art WHERE VerifiedBuild > 0"
            }
        }
    }
}
