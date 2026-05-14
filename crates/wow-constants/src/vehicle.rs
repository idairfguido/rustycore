//! Vehicle constants ported from C++ `VehicleDefines.h`.

use num_derive::{FromPrimitive, ToPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u16)]
pub enum VehiclePowerType {
    Steam = 61,
    Pyrite = 41,
    Heat = 101,
    Ooze = 121,
    Blood = 141,
    Wrath = 142,
    ArcaneEnergy = 143,
    LifeEnergy = 144,
    SunEnergy = 145,
    SwingVelocity = 146,
    ShadowflameEnergy = 147,
    BluePower = 148,
    PurplePower = 149,
    GreenPower = 150,
    OrangePower = 151,
    Energy2 = 153,
    Arcaneenergy = 161,
    WindPower1 = 162,
    WindPower2 = 163,
    WindPower3 = 164,
    Fuel = 165,
    SunPower = 166,
    TwilightEnergy = 169,
    Venom = 174,
    OrangePower2 = 176,
    ConsumingFlame = 177,
    PyroclasticFrenzy = 178,
    Flashfire = 179,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum VehicleFlag {
    NoStrafe = 0x0000_0001,
    NoJumping = 0x0000_0002,
    FullSpeedTurning = 0x0000_0004,
    AllowPitching = 0x0000_0010,
    FullSpeedPitching = 0x0000_0020,
    CustomPitch = 0x0000_0040,
    AdjustAimAngle = 0x0000_0400,
    AdjustAimPower = 0x0000_0800,
    FixedPosition = 0x0020_0000,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum VehicleSpell {
    RideHardcoded = 46598,
    Parachute = 45472,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive, ToPrimitive)]
#[repr(u8)]
pub enum VehicleExitParameter {
    None = 0,
    Offset = 1,
    Destination = 2,
    Max = 3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicle_defines_match_cpp_values() {
        assert_eq!(VehiclePowerType::Pyrite as u16, 41);
        assert_eq!(VehiclePowerType::Flashfire as u16, 179);
        assert_eq!(VehicleFlag::NoStrafe as u32, 0x0000_0001);
        assert_eq!(VehicleFlag::FixedPosition as u32, 0x0020_0000);
        assert_eq!(VehicleSpell::RideHardcoded as u32, 46598);
        assert_eq!(VehicleSpell::Parachute as u32, 45472);
        assert_eq!(VehicleExitParameter::None as u8, 0);
        assert_eq!(VehicleExitParameter::Max as u8, 3);
    }
}
