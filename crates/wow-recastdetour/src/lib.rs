use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

pub const MMAP_MAGIC_LIKE_CPP: u32 = 0x4d4d_4150;
pub const MMAP_VERSION_LIKE_CPP: u32 = 15;
pub const MMAP_TILE_HEADER_SIZE_LIKE_CPP: usize = 20;

pub const DT_POLYREF64_LIKE_CPP: bool = true;
pub const DT_SALT_BITS_LIKE_CPP: u32 = 12;
pub const DT_TILE_BITS_LIKE_CPP: u32 = 21;
pub const DT_POLY_BITS_LIKE_CPP: u32 = 31;
pub const DT_NAVMESH_MAGIC_LIKE_CPP: u32 = 0x444e_4156;
pub const DT_NAVMESH_VERSION_LIKE_CPP: u32 = 7;
pub const DT_NAVMESH_STATE_MAGIC_LIKE_CPP: u32 = 0x444e_4d53;
pub const DT_NAVMESH_STATE_VERSION_LIKE_CPP: u32 = 1;
pub const DT_EXT_LINK_LIKE_CPP: u16 = 0x8000;
pub const DT_NULL_LINK_LIKE_CPP: u32 = 0xffff_ffff;
pub const DT_OFFMESH_CON_BIDIR_LIKE_CPP: u32 = 1;
pub const DT_MAX_AREAS_LIKE_CPP: usize = 64;
pub const DT_TILE_FREE_DATA_LIKE_CPP: i32 = 0x01;
pub const DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP: usize = 28;

pub const NAV_AREA_EMPTY_LIKE_CPP: u8 = 0;
pub const NAV_AREA_GROUND_LIKE_CPP: u8 = 11;
pub const NAV_AREA_GROUND_STEEP_LIKE_CPP: u8 = 10;
pub const NAV_AREA_WATER_LIKE_CPP: u8 = 9;
pub const NAV_AREA_MAGMA_SLIME_LIKE_CPP: u8 = 8;
pub const NAV_AREA_MAX_VALUE_LIKE_CPP: u8 = NAV_AREA_GROUND_LIKE_CPP;
pub const NAV_AREA_MIN_VALUE_LIKE_CPP: u8 = NAV_AREA_MAGMA_SLIME_LIKE_CPP;
pub const NAV_AREA_ALL_MASK_LIKE_CPP: u8 = 0x3f;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct NavTerrainFlag: u16 {
        const EMPTY = 0x00;
        const GROUND = 1 << (NAV_AREA_MAX_VALUE_LIKE_CPP - NAV_AREA_GROUND_LIKE_CPP);
        const GROUND_STEEP = 1 << (NAV_AREA_MAX_VALUE_LIKE_CPP - NAV_AREA_GROUND_STEEP_LIKE_CPP);
        const WATER = 1 << (NAV_AREA_MAX_VALUE_LIKE_CPP - NAV_AREA_WATER_LIKE_CPP);
        const MAGMA_SLIME = 1 << (NAV_AREA_MAX_VALUE_LIKE_CPP - NAV_AREA_MAGMA_SLIME_LIKE_CPP);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmapTileHeader {
    pub mmap_magic: u32,
    pub dt_version: u32,
    pub mmap_version: u32,
    pub size: u32,
    pub uses_liquids: bool,
    pub padding: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetourNavMeshParams {
    pub origin: [f32; 3],
    pub tile_width: f32,
    pub tile_height: f32,
    pub max_tiles: i32,
    pub max_polys: i32,
}

impl DetourNavMeshParams {
    pub fn parse(bytes: &[u8]) -> Result<Self, DetourNavMeshParamsError> {
        if bytes.len() < DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP {
            return Err(DetourNavMeshParamsError::TooShort {
                actual: bytes.len(),
                expected: DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP,
            });
        }

        Ok(Self {
            origin: [read_f32(bytes, 0), read_f32(bytes, 4), read_f32(bytes, 8)],
            tile_width: read_f32(bytes, 12),
            tile_height: read_f32(bytes, 16),
            max_tiles: read_i32(bytes, 20),
            max_polys: read_i32(bytes, 24),
        })
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP] {
        let mut bytes = [0; DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP];
        bytes[0..4].copy_from_slice(&self.origin[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&self.origin[1].to_le_bytes());
        bytes[8..12].copy_from_slice(&self.origin[2].to_le_bytes());
        bytes[12..16].copy_from_slice(&self.tile_width.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.tile_height.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.max_tiles.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.max_polys.to_le_bytes());
        bytes
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DetourNavMeshParamsError {
    #[error("Detour navmesh params are too short: got {actual} bytes, expected {expected}")]
    TooShort { actual: usize, expected: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MMapData {
    pub nav_mesh_params: DetourNavMeshParams,
    pub loaded_tile_refs: HashMap<u32, u64>,
}

impl MMapData {
    #[must_use]
    pub fn new(nav_mesh_params: DetourNavMeshParams) -> Self {
        Self {
            nav_mesh_params,
            loaded_tile_refs: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadUnsafeMapData {
    pub map_id: u32,
    pub child_map_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MMapManager {
    loaded_mmaps: HashMap<u32, Option<MMapData>>,
    parent_map_data: HashMap<u32, u32>,
    loaded_tiles: u32,
    thread_safe_environment: bool,
}

impl Default for MMapManager {
    fn default() -> Self {
        Self {
            loaded_mmaps: HashMap::new(),
            parent_map_data: HashMap::new(),
            loaded_tiles: 0,
            thread_safe_environment: true,
        }
    }
}

impl MMapManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize_thread_unsafe<I>(&mut self, map_data: I)
    where
        I: IntoIterator<Item = ThreadUnsafeMapData>,
    {
        self.loaded_mmaps.clear();
        self.parent_map_data.clear();
        self.loaded_tiles = 0;

        for data in map_data {
            self.loaded_mmaps.entry(data.map_id).or_insert(None);

            for child_map_id in data.child_map_ids {
                self.parent_map_data.insert(child_map_id, data.map_id);
            }
        }

        self.thread_safe_environment = false;
    }

    pub fn load_map_data(
        &mut self,
        base_path: impl AsRef<Path>,
        map_id: u32,
    ) -> Result<bool, MMapManagerError> {
        if let Some(data) = self.loaded_mmaps.get(&map_id) {
            if data.is_some() {
                return Ok(true);
            }
        } else if !self.thread_safe_environment {
            return Err(MMapManagerError::InvalidMapInThreadUnsafe { map_id });
        } else {
            self.loaded_mmaps.insert(map_id, None);
        }

        let path = map_file_path_like_cpp(base_path, map_id);
        let bytes = fs::read(&path).map_err(|source| MMapManagerError::ReadMapFile {
            path: path.clone(),
            source,
        })?;
        let nav_mesh_params =
            DetourNavMeshParams::parse(&bytes).map_err(MMapManagerError::BadMapParams)?;

        self.loaded_mmaps
            .insert(map_id, Some(MMapData::new(nav_mesh_params)));

        Ok(true)
    }

    #[must_use]
    pub fn get_mmap_data(&self, map_id: u32) -> Option<&MMapData> {
        self.loaded_mmaps.get(&map_id).and_then(Option::as_ref)
    }

    pub fn unload_map(&mut self, map_id: u32) -> bool {
        let Some(data) = self.loaded_mmaps.get_mut(&map_id) else {
            return false;
        };

        let Some(loaded) = data.take() else {
            return false;
        };

        self.loaded_tiles = self
            .loaded_tiles
            .saturating_sub(loaded.loaded_tile_refs.len() as u32);
        true
    }

    #[must_use]
    pub fn get_nav_mesh_params(&self, map_id: u32) -> Option<DetourNavMeshParams> {
        self.loaded_mmaps
            .get(&map_id)
            .and_then(Option::as_ref)
            .map(|data| data.nav_mesh_params)
    }

    #[must_use]
    pub fn parent_map_id(&self, child_map_id: u32) -> Option<u32> {
        self.parent_map_data.get(&child_map_id).copied()
    }

    #[must_use]
    pub fn get_loaded_tiles_count(&self) -> u32 {
        self.loaded_tiles
    }

    #[must_use]
    pub fn get_loaded_maps_count(&self) -> u32 {
        self.loaded_mmaps.len() as u32
    }

    #[must_use]
    pub fn is_thread_safe_environment(&self) -> bool {
        self.thread_safe_environment
    }
}

#[derive(Debug, Error)]
pub enum MMapManagerError {
    #[error("invalid map id {map_id} passed after thread-unsafe initialization")]
    InvalidMapInThreadUnsafe { map_id: u32 },
    #[error("failed to read mmap file {path:?}: {source}")]
    ReadMapFile { path: PathBuf, source: io::Error },
    #[error("bad mmap params: {0}")]
    BadMapParams(DetourNavMeshParamsError),
}

impl MmapTileHeader {
    #[must_use]
    pub const fn new(dt_version: u32) -> Self {
        Self {
            mmap_magic: MMAP_MAGIC_LIKE_CPP,
            dt_version,
            mmap_version: MMAP_VERSION_LIKE_CPP,
            size: 0,
            uses_liquids: true,
            padding: [0; 3],
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, MmapTileHeaderError> {
        if bytes.len() < MMAP_TILE_HEADER_SIZE_LIKE_CPP {
            return Err(MmapTileHeaderError::TooShort {
                actual: bytes.len(),
                expected: MMAP_TILE_HEADER_SIZE_LIKE_CPP,
            });
        }

        let header = Self {
            mmap_magic: read_u32(bytes, 0),
            dt_version: read_u32(bytes, 4),
            mmap_version: read_u32(bytes, 8),
            size: read_u32(bytes, 12),
            uses_liquids: bytes[16] != 0,
            padding: [bytes[17], bytes[18], bytes[19]],
        };

        if header.mmap_magic != MMAP_MAGIC_LIKE_CPP {
            return Err(MmapTileHeaderError::BadMagic {
                actual: header.mmap_magic,
                expected: MMAP_MAGIC_LIKE_CPP,
            });
        }

        if header.mmap_version != MMAP_VERSION_LIKE_CPP {
            return Err(MmapTileHeaderError::BadMmapVersion {
                actual: header.mmap_version,
                expected: MMAP_VERSION_LIKE_CPP,
            });
        }

        Ok(header)
    }

    pub fn validate_dt_version(&self, expected_dt_version: u32) -> Result<(), MmapTileHeaderError> {
        if self.dt_version != expected_dt_version {
            return Err(MmapTileHeaderError::BadDetourVersion {
                actual: self.dt_version,
                expected: expected_dt_version,
            });
        }

        Ok(())
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; MMAP_TILE_HEADER_SIZE_LIKE_CPP] {
        let mut bytes = [0; MMAP_TILE_HEADER_SIZE_LIKE_CPP];
        bytes[0..4].copy_from_slice(&self.mmap_magic.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.dt_version.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.mmap_version.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.size.to_le_bytes());
        bytes[16] = u8::from(self.uses_liquids);
        bytes[17..20].copy_from_slice(&self.padding);
        bytes
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum MmapTileHeaderError {
    #[error("mmap tile header is too short: got {actual} bytes, expected {expected}")]
    TooShort { actual: usize, expected: usize },
    #[error("bad mmap magic: got 0x{actual:08x}, expected 0x{expected:08x}")]
    BadMagic { actual: u32, expected: u32 },
    #[error("bad mmap version: got {actual}, expected {expected}")]
    BadMmapVersion { actual: u32, expected: u32 },
    #[error("bad Detour navmesh version: got {actual}, expected {expected}")]
    BadDetourVersion { actual: u32, expected: u32 },
}

#[must_use]
pub const fn pack_tile_id_like_cpp(x: i32, y: i32) -> u32 {
    ((x as u32) << 16) | (y as u32 & 0xffff)
}

#[must_use]
pub fn map_file_name_like_cpp(map_id: u32) -> String {
    format!("mmaps/{map_id:04}.mmap")
}

#[must_use]
pub fn map_file_path_like_cpp(base_path: impl AsRef<Path>, map_id: u32) -> PathBuf {
    base_path.as_ref().join(map_file_name_like_cpp(map_id))
}

#[must_use]
pub fn tile_file_name_like_cpp(map_id: u32, x: i32, y: i32) -> String {
    format!("mmaps/{map_id:04}{x:02}{y:02}.mmtile")
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmap_constants_and_nav_flags_match_cpp() {
        assert_eq!(MMAP_MAGIC_LIKE_CPP, 0x4d4d_4150);
        assert_eq!(MMAP_VERSION_LIKE_CPP, 15);
        assert_eq!(MMAP_TILE_HEADER_SIZE_LIKE_CPP, 20);

        assert!(DT_POLYREF64_LIKE_CPP);
        assert_eq!(DT_SALT_BITS_LIKE_CPP, 12);
        assert_eq!(DT_TILE_BITS_LIKE_CPP, 21);
        assert_eq!(DT_POLY_BITS_LIKE_CPP, 31);
        assert_eq!(DT_NAVMESH_MAGIC_LIKE_CPP, 0x444e_4156);
        assert_eq!(DT_NAVMESH_VERSION_LIKE_CPP, 7);
        assert_eq!(DT_NAVMESH_STATE_MAGIC_LIKE_CPP, 0x444e_4d53);
        assert_eq!(DT_NAVMESH_STATE_VERSION_LIKE_CPP, 1);
        assert_eq!(DT_EXT_LINK_LIKE_CPP, 0x8000);
        assert_eq!(DT_NULL_LINK_LIKE_CPP, 0xffff_ffff);
        assert_eq!(DT_OFFMESH_CON_BIDIR_LIKE_CPP, 1);
        assert_eq!(DT_MAX_AREAS_LIKE_CPP, 64);
        assert_eq!(DT_TILE_FREE_DATA_LIKE_CPP, 1);
        assert_eq!(DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP, 28);

        assert_eq!(NAV_AREA_EMPTY_LIKE_CPP, 0);
        assert_eq!(NAV_AREA_GROUND_LIKE_CPP, 11);
        assert_eq!(NAV_AREA_GROUND_STEEP_LIKE_CPP, 10);
        assert_eq!(NAV_AREA_WATER_LIKE_CPP, 9);
        assert_eq!(NAV_AREA_MAGMA_SLIME_LIKE_CPP, 8);
        assert_eq!(NAV_AREA_ALL_MASK_LIKE_CPP, 0x3f);

        assert_eq!(NavTerrainFlag::EMPTY.bits(), 0x00);
        assert_eq!(NavTerrainFlag::GROUND.bits(), 0x01);
        assert_eq!(NavTerrainFlag::GROUND_STEEP.bits(), 0x02);
        assert_eq!(NavTerrainFlag::WATER.bits(), 0x04);
        assert_eq!(NavTerrainFlag::MAGMA_SLIME.bits(), 0x08);
    }

    #[test]
    fn detour_nav_mesh_params_round_trips_cpp_layout() {
        let params = DetourNavMeshParams {
            origin: [-17_066.666, -17_066.666, -2_000.0],
            tile_width: 533.3333,
            tile_height: 533.3333,
            max_tiles: 4_096,
            max_polys: 32_768,
        };

        let bytes = params.to_bytes();
        assert_eq!(bytes.len(), DT_NAV_MESH_PARAMS_SIZE_LIKE_CPP);
        assert_eq!(DetourNavMeshParams::parse(&bytes), Ok(params));
        assert_eq!(
            DetourNavMeshParams::parse(&bytes[..27]),
            Err(DetourNavMeshParamsError::TooShort {
                actual: 27,
                expected: 28,
            })
        );
    }

    #[test]
    fn mmap_tile_header_round_trips_cpp_layout() {
        let header = MmapTileHeader {
            mmap_magic: MMAP_MAGIC_LIKE_CPP,
            dt_version: 7,
            mmap_version: MMAP_VERSION_LIKE_CPP,
            size: 123_456,
            uses_liquids: true,
            padding: [0, 0, 0],
        };

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), MMAP_TILE_HEADER_SIZE_LIKE_CPP);
        assert_eq!(MmapTileHeader::parse(&bytes), Ok(header));
        assert_eq!(
            MmapTileHeader::parse(&bytes)
                .unwrap()
                .validate_dt_version(7),
            Ok(())
        );
    }

    #[test]
    fn mmap_tile_header_rejects_cpp_load_failures() {
        assert_eq!(
            MmapTileHeader::parse(&[0; 19]),
            Err(MmapTileHeaderError::TooShort {
                actual: 19,
                expected: 20,
            })
        );

        let mut bad_magic = MmapTileHeader::new(7).to_bytes();
        bad_magic[0] = 0;
        assert!(matches!(
            MmapTileHeader::parse(&bad_magic),
            Err(MmapTileHeaderError::BadMagic { .. })
        ));

        let mut bad_version = MmapTileHeader::new(7).to_bytes();
        bad_version[8..12].copy_from_slice(&14_u32.to_le_bytes());
        assert!(matches!(
            MmapTileHeader::parse(&bad_version),
            Err(MmapTileHeaderError::BadMmapVersion { .. })
        ));

        let header = MmapTileHeader::new(7);
        assert_eq!(
            header.validate_dt_version(8),
            Err(MmapTileHeaderError::BadDetourVersion {
                actual: 7,
                expected: 8,
            })
        );
    }

    #[test]
    fn mmap_manager_small_helpers_match_cpp() {
        assert_eq!(pack_tile_id_like_cpp(0x12, 0x34), 0x0012_0034);
        assert_eq!(map_file_name_like_cpp(571), "mmaps/0571.mmap");
        assert_eq!(
            map_file_path_like_cpp("/srv/wow", 571),
            std::path::PathBuf::from("/srv/wow/mmaps/0571.mmap")
        );
        assert_eq!(
            tile_file_name_like_cpp(571, 32, 48),
            "mmaps/05713248.mmtile"
        );
    }

    #[test]
    fn mmap_manager_loads_map_params_and_caches_like_cpp() {
        let root = unique_test_dir("mmap-manager-loads-map-params");
        std::fs::create_dir_all(root.join("mmaps")).unwrap();

        let params = DetourNavMeshParams {
            origin: [1.0, 2.0, 3.0],
            tile_width: 533.3333,
            tile_height: 533.3333,
            max_tiles: 128,
            max_polys: 16_384,
        };
        std::fs::write(root.join("mmaps/0001.mmap"), params.to_bytes()).unwrap();

        let mut manager = MMapManager::new();
        assert!(manager.is_thread_safe_environment());
        assert_eq!(manager.get_loaded_maps_count(), 0);
        assert!(matches!(manager.load_map_data(&root, 1), Ok(true)));
        assert!(matches!(manager.load_map_data(&root, 1), Ok(true)));
        assert_eq!(manager.get_loaded_maps_count(), 1);
        assert_eq!(manager.get_loaded_tiles_count(), 0);
        assert_eq!(manager.get_nav_mesh_params(1), Some(params));
        assert_eq!(manager.get_mmap_data(1).unwrap().loaded_tile_refs.len(), 0);
        assert!(manager.unload_map(1));
        assert!(!manager.unload_map(1));
        assert_eq!(manager.get_loaded_maps_count(), 1);
        assert_eq!(manager.get_nav_mesh_params(1), None);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mmap_manager_thread_unsafe_preloads_allowed_map_ids_like_cpp() {
        let root = unique_test_dir("mmap-manager-thread-unsafe");
        std::fs::create_dir_all(root.join("mmaps")).unwrap();

        let params = DetourNavMeshParams {
            origin: [10.0, 20.0, 30.0],
            tile_width: 533.3333,
            tile_height: 533.3333,
            max_tiles: 256,
            max_polys: 32_768,
        };
        std::fs::write(root.join("mmaps/0571.mmap"), params.to_bytes()).unwrap();

        let mut manager = MMapManager::new();
        manager.initialize_thread_unsafe([ThreadUnsafeMapData {
            map_id: 571,
            child_map_ids: vec![609],
        }]);

        assert!(!manager.is_thread_safe_environment());
        assert_eq!(manager.get_loaded_maps_count(), 1);
        assert_eq!(manager.get_nav_mesh_params(571), None);
        assert_eq!(manager.parent_map_id(609), Some(571));
        assert!(matches!(manager.load_map_data(&root, 571), Ok(true)));
        assert_eq!(manager.get_nav_mesh_params(571), Some(params));
        assert!(matches!(
            manager.load_map_data(&root, 1),
            Err(MMapManagerError::InvalidMapInThreadUnsafe { map_id: 1 })
        ));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mmap_manager_keeps_placeholder_after_missing_file_like_cpp() {
        let root = unique_test_dir("mmap-manager-missing-file");
        let mut manager = MMapManager::new();

        assert!(matches!(
            manager.load_map_data(&root, 999),
            Err(MMapManagerError::ReadMapFile { .. })
        ));
        assert_eq!(manager.get_loaded_maps_count(), 1);
        assert_eq!(manager.get_nav_mesh_params(999), None);
    }

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rustycore-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }
}
