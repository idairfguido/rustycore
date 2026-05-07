//! Canonical entity model.
//!
//! C++ refs:
//! - `game/Entities/Object/Object.h`
//! - `game/Entities/Object/Object.cpp`
//! - `game/Entities/Object/ObjectGuid.h`

mod object;

pub use object::{CreateObjectFlags, EntityObject, EntityObjectState, ObjectChangedFields};
