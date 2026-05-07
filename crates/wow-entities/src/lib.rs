//! Canonical entity model.
//!
//! C++ refs:
//! - `game/Entities/Object/Object.h`
//! - `game/Entities/Object/Object.cpp`
//! - `game/Entities/Object/ObjectGuid.h`

mod object;
mod object_accessor;
mod update_fields;
mod world_object;

pub use object::{CreateObjectFlags, EntityObject, EntityObjectState, ObjectChangedFields};
pub use object_accessor::{
    AccessorObjectKind, AccessorPlayer, MapObjectRecord, ObjectAccessor, ObjectAccessorError,
    normalize_player_name,
};
pub use update_fields::{
    NUM_CLIENT_OBJECT_TYPES, OBJECT_DATA_BITS, OBJECT_DATA_DYNAMIC_FLAGS_BIT,
    OBJECT_DATA_ENTRY_ID_BIT, OBJECT_DATA_PARENT_BIT, OBJECT_DATA_SCALE_BIT, ObjectDataUpdate,
    ObjectDataValues, TYPEID_OBJECT, UpdateMask, ValuesUpdate,
};
pub use world_object::{MAPID_INVALID, MapBindingError, PhaseShift, WorldLocation, WorldObject};
