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
    /// C++ `HOTFIX_SEL_MOUNT`.
    SEL_MOUNT,
    /// C++ `HOTFIX_SEL_MOUNT_CAPABILITY`.
    SEL_MOUNT_CAPABILITY,
    /// C++ `HOTFIX_SEL_MOUNT_TYPE_X_CAPABILITY`.
    SEL_MOUNT_TYPE_X_CAPABILITY,
    /// C++ `HOTFIX_SEL_MOUNT_X_DISPLAY`.
    SEL_MOUNT_X_DISPLAY,
    /// C++ `HOTFIX_SEL_CREATURE_DISPLAY_INFO`.
    SEL_CREATURE_DISPLAY_INFO,
    /// C++ `HOTFIX_SEL_CREATURE_MODEL_DATA`.
    SEL_CREATURE_MODEL_DATA,
    /// C++ `HOTFIX_SEL_VEHICLE`.
    SEL_VEHICLE,
    /// C++ `HOTFIX_SEL_VEHICLE_SEAT`.
    SEL_VEHICLE_SEAT,
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
            Self::SEL_MOUNT => concat!(
                "SELECT Name, SourceText, Description, ID, MountTypeID, Flags, SourceTypeEnum, SourceSpellID, ",
                "PlayerConditionID, MountFlyRideHeight, UiModelSceneID FROM mount WHERE VerifiedBuild > 0"
            ),
            Self::SEL_MOUNT_CAPABILITY => concat!(
                "SELECT ID, Flags, ReqRidingSkill, ReqAreaID, ReqSpellAuraID, ReqSpellKnownID, ModSpellAuraID, ",
                "ReqMapID FROM mount_capability WHERE VerifiedBuild > 0"
            ),
            Self::SEL_MOUNT_TYPE_X_CAPABILITY => {
                "SELECT ID, MountTypeID, MountCapabilityID, OrderIndex FROM mount_type_x_capability WHERE VerifiedBuild > 0"
            }
            Self::SEL_MOUNT_X_DISPLAY => {
                "SELECT ID, CreatureDisplayInfoID, PlayerConditionID, MountID FROM mount_x_display WHERE VerifiedBuild > 0"
            }
            Self::SEL_CREATURE_DISPLAY_INFO => concat!(
                "SELECT ID, ModelID, SoundID, SizeClass, CreatureModelScale, CreatureModelAlpha, BloodID, ",
                "ExtendedDisplayInfoID, NPCSoundID, ParticleColorID, PortraitCreatureDisplayInfoID, ",
                "PortraitTextureFileDataID, ObjectEffectPackageID, AnimReplacementSetID, Flags, ",
                "StateSpellVisualKitID, PlayerOverrideScale, PetInstanceScale, UnarmedWeaponType, ",
                "MountPoofSpellVisualKitID, DissolveEffectID, Gender, DissolveOutEffectID, CreatureModelMinLod, ",
                "TextureVariationFileDataID1, TextureVariationFileDataID2, TextureVariationFileDataID3, ",
                "TextureVariationFileDataID4 FROM creature_display_info WHERE VerifiedBuild > 0"
            ),
            Self::SEL_CREATURE_MODEL_DATA => concat!(
                "SELECT ID, GeoBox1, GeoBox2, GeoBox3, GeoBox4, GeoBox5, GeoBox6, Flags, FileDataID, ",
                "BloodID, FootprintTextureID, FootprintTextureLength, FootprintTextureWidth, FootprintParticleScale, ",
                "FoleyMaterialID, FootstepCameraEffectID, DeathThudCameraEffectID, SoundID, SizeClass, ",
                "CollisionWidth, CollisionHeight, WorldEffectScale, CreatureGeosetDataID, HoverHeight, ",
                "AttachedEffectScale, ModelScale, MissileCollisionRadius, MissileCollisionPush, MissileCollisionRaise, ",
                "MountHeight, OverrideLootEffectScale, OverrideNameScale, OverrideSelectionRadius, TamedPetBaseScale ",
                "FROM creature_model_data WHERE VerifiedBuild > 0"
            ),
            Self::SEL_VEHICLE => concat!(
                "SELECT ID, Flags, FlagsB, TurnSpeed, PitchSpeed, PitchMin, PitchMax, MouseLookOffsetPitch, ",
                "CameraFadeDistScalarMin, CameraFadeDistScalarMax, CameraPitchOffset, FacingLimitRight, ",
                "FacingLimitLeft, CameraYawOffset, VehicleUIIndicatorID, MissileTargetingID, VehiclePOITypeID, ",
                "UiLocomotionType, SeatID1, SeatID2, SeatID3, SeatID4, SeatID5, SeatID6, SeatID7, SeatID8, ",
                "PowerDisplayID1, PowerDisplayID2, PowerDisplayID3 FROM vehicle WHERE VerifiedBuild > 0"
            ),
            Self::SEL_VEHICLE_SEAT => concat!(
                "SELECT ID, AttachmentOffsetX, AttachmentOffsetY, AttachmentOffsetZ, CameraOffsetX, CameraOffsetY, ",
                "CameraOffsetZ, Flags, FlagsB, FlagsC, AttachmentID, EnterPreDelay, EnterSpeed, EnterGravity, ",
                "EnterMinDuration, EnterMaxDuration, EnterMinArcHeight, EnterMaxArcHeight, EnterAnimStart, ",
                "EnterAnimLoop, RideAnimStart, RideAnimLoop, RideUpperAnimStart, RideUpperAnimLoop, ExitPreDelay, ",
                "ExitSpeed, ExitGravity, ExitMinDuration, ExitMaxDuration, ExitMinArcHeight, ExitMaxArcHeight, ",
                "ExitAnimStart, ExitAnimLoop, ExitAnimEnd, VehicleEnterAnim, VehicleEnterAnimBone, VehicleExitAnim, ",
                "VehicleExitAnimBone, VehicleRideAnimLoop, VehicleRideAnimLoopBone, PassengerAttachmentID, ",
                "PassengerYaw, PassengerPitch, PassengerRoll, VehicleEnterAnimDelay, VehicleExitAnimDelay, ",
                "VehicleAbilityDisplay, EnterUISoundID, ExitUISoundID, UiSkinFileDataID, UiSkin, CameraEnteringDelay, ",
                "CameraEnteringDuration, CameraExitingDelay, CameraExitingDuration, CameraPosChaseRate, ",
                "CameraFacingChaseRate, CameraEnteringZoom, CameraSeatZoomMin, CameraSeatZoomMax, EnterAnimKitID, ",
                "RideAnimKitID, ExitAnimKitID, VehicleEnterAnimKitID, VehicleRideAnimKitID, VehicleExitAnimKitID, ",
                "CameraModeID FROM vehicle_seat WHERE VerifiedBuild > 0"
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
