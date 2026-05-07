pub mod cell;
pub mod coords;
pub mod spawn;

use std::fmt;

pub use cell::{Cell, CellArea, GridObjectGuids, WorldObjectGuids};
pub use coords::{
    CellCoord, GridCoord, MAX_NUMBER_OF_CELLS, MAX_NUMBER_OF_GRIDS, SIZE_OF_GRID_CELL,
    SIZE_OF_GRIDS, TOTAL_NUMBER_OF_CELLS_PER_MAP, cell_to_grid_local, compute_cell_coord,
    compute_cell_coord_with_offset, compute_grid_coord, compute_grid_coord_simple,
    is_valid_map_coord, normalize_map_coord,
};
pub use spawn::{
    CellSpawnGuids, Difficulty, PersonalSpawnMapKey, SpawnData, SpawnGroupFlags,
    SpawnGroupTemplateData, SpawnId, SpawnMapKey, SpawnObjectType, SpawnPosition, SpawnStore,
};

/// Key used by `MapManager` for world and instance map lookup.
///
/// C++ ref: `Maps/MapManager.h`, `using MapKey = std::pair<uint32, uint32>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MapKey {
    pub map_id: u32,
    pub instance_id: u32,
}

impl MapKey {
    pub const fn new(map_id: u32, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
        }
    }
}

impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapKey({}, {})", self.map_id, self.instance_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_key_keeps_trinity_uint32_shape() {
        let key = MapKey::new(u32::MAX, 42);
        assert_eq!(key.map_id, u32::MAX);
        assert_eq!(key.instance_id, 42);
        assert_eq!(key.to_string(), "MapKey(4294967295, 42)");
    }

    #[test]
    fn map_key_orders_like_std_pair_for_range_queries() {
        let mut keys = [
            MapKey::new(571, 3),
            MapKey::new(0, 0),
            MapKey::new(571, 1),
            MapKey::new(1, 0),
        ];
        keys.sort();

        assert_eq!(
            keys,
            [
                MapKey::new(0, 0),
                MapKey::new(1, 0),
                MapKey::new(571, 1),
                MapKey::new(571, 3),
            ]
        );
    }
}
