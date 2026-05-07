//! Canonical entity model.
//!
//! C++ refs:
//! - `game/Entities/Object/Object.h`
//! - `game/Entities/Object/Object.cpp`
//! - `game/Entities/Object/ObjectGuid.h`

mod object;
mod world_object;

pub use object::{CreateObjectFlags, EntityObject, EntityObjectState, ObjectChangedFields};
pub use world_object::{MAPID_INVALID, MapBindingError, PhaseShift, WorldLocation, WorldObject};
