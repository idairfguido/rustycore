use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateMask {
    bits: usize,
    blocks: Vec<u32>,
    blocks_mask: Vec<u32>,
}

impl UpdateMask {
    pub fn new(bits: usize) -> Self {
        let block_count = bits.div_ceil(32);
        let blocks_mask_count = block_count.div_ceil(32);
        Self {
            bits,
            blocks: vec![0; block_count],
            blocks_mask: vec![0; blocks_mask_count],
        }
    }

    pub fn from_blocks(bits: usize, input: &[u32]) -> Self {
        let mut mask = Self::new(bits);
        for (block, value) in mask.blocks.iter_mut().zip(input.iter().copied()) {
            *block = value;
        }
        mask.rebuild_blocks_mask();
        mask
    }

    pub const fn bits(&self) -> usize {
        self.bits
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn blocks_mask_count(&self) -> usize {
        self.blocks_mask.len()
    }

    pub fn blocks(&self) -> &[u32] {
        &self.blocks
    }

    pub fn blocks_mask(&self) -> &[u32] {
        &self.blocks_mask
    }

    pub fn get_block(&self, index: usize) -> u32 {
        self.blocks[index]
    }

    pub fn get_blocks_mask(&self, index: usize) -> u32 {
        self.blocks_mask[index]
    }

    pub fn is_set(&self, index: usize) -> bool {
        assert!(index < self.bits);
        (self.blocks[block_index(index)] & block_flag(index)) != 0
    }

    pub fn is_any_set(&self) -> bool {
        self.blocks_mask.iter().any(|block_mask| *block_mask != 0)
    }

    pub fn set(&mut self, index: usize) {
        assert!(index < self.bits);
        let block = block_index(index);
        self.blocks[block] |= block_flag(index);
        self.blocks_mask[block_index(block)] |= block_flag(block);
    }

    pub fn reset(&mut self, index: usize) {
        assert!(index < self.bits);
        let block = block_index(index);
        self.blocks[block] &= !block_flag(index);
        if self.blocks[block] == 0 {
            self.blocks_mask[block_index(block)] &= !block_flag(block);
        }
    }

    pub fn reset_all(&mut self) {
        self.blocks.fill(0);
        self.blocks_mask.fill(0);
    }

    pub fn set_all(&mut self) {
        self.blocks.fill(u32::MAX);
        if let Some(last) = self.blocks.last_mut() {
            let used_bits = self.bits % 32;
            if used_bits != 0 {
                *last &= u32::MAX >> (32 - used_bits);
            }
        }
        self.rebuild_blocks_mask();
    }

    fn rebuild_blocks_mask(&mut self) {
        self.blocks_mask.fill(0);
        for (block, value) in self.blocks.iter().enumerate() {
            if *value != 0 {
                self.blocks_mask[block_index(block)] |= block_flag(block);
            }
        }
    }
}

impl BitAndAssign<&UpdateMask> for UpdateMask {
    fn bitand_assign(&mut self, rhs: &UpdateMask) {
        assert_eq!(self.bits, rhs.bits);
        for (left, right) in self.blocks.iter_mut().zip(&rhs.blocks) {
            *left &= *right;
        }
        for (left, right) in self.blocks_mask.iter_mut().zip(&rhs.blocks_mask) {
            *left &= *right;
        }
        self.rebuild_blocks_mask();
    }
}

impl BitOrAssign<&UpdateMask> for UpdateMask {
    fn bitor_assign(&mut self, rhs: &UpdateMask) {
        assert_eq!(self.bits, rhs.bits);
        for (left, right) in self.blocks.iter_mut().zip(&rhs.blocks) {
            *left |= *right;
        }
        for (left, right) in self.blocks_mask.iter_mut().zip(&rhs.blocks_mask) {
            *left |= *right;
        }
    }
}

impl BitAnd for UpdateMask {
    type Output = Self;

    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= &rhs;
        self
    }
}

impl BitOr for UpdateMask {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self::Output {
        self |= &rhs;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectDataValues {
    pub entry_id: i32,
    pub dynamic_flags: u32,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectDataUpdate {
    pub mask: UpdateMask,
    pub values: ObjectDataValues,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValuesUpdate {
    pub changed_object_type_mask: u32,
    pub object_data: Option<ObjectDataUpdate>,
}

impl ValuesUpdate {
    pub const fn empty() -> Self {
        Self {
            changed_object_type_mask: 0,
            object_data: None,
        }
    }

    pub const fn has_data(&self) -> bool {
        self.changed_object_type_mask != 0
    }
}

pub const NUM_CLIENT_OBJECT_TYPES: usize = 14;
pub const OBJECT_DATA_BITS: usize = 4;
pub const ITEM_DATA_BITS: usize = 43;
pub const CONTAINER_DATA_BITS: usize = 39;
pub const UNIT_DATA_BITS: usize = 227;
pub const PLAYER_DATA_BITS: usize = 108;
pub const ACTIVE_PLAYER_DATA_BITS: usize = 1525;
pub const GAME_OBJECT_DATA_BITS: usize = 20;
pub const DYNAMIC_OBJECT_DATA_BITS: usize = 7;
pub const CORPSE_DATA_BITS: usize = 32;
pub const AREA_TRIGGER_DATA_BITS: usize = 20;
pub const SCENE_OBJECT_DATA_BITS: usize = 5;
pub const CONVERSATION_DATA_BITS: usize = 4;
pub const TYPEID_OBJECT: usize = 0;
pub const TYPEID_ITEM: usize = 1;
pub const TYPEID_CONTAINER: usize = 2;
pub const TYPEID_UNIT: usize = 5;
pub const TYPEID_PLAYER: usize = 6;
pub const TYPEID_ACTIVE_PLAYER: usize = 7;
pub const TYPEID_GAME_OBJECT: usize = 8;
pub const TYPEID_DYNAMIC_OBJECT: usize = 9;
pub const TYPEID_CORPSE: usize = 10;
pub const TYPEID_AREA_TRIGGER: usize = 11;
pub const TYPEID_SCENE_OBJECT: usize = 12;
pub const TYPEID_CONVERSATION: usize = 13;
pub const OBJECT_DATA_PARENT_BIT: usize = 0;
pub const OBJECT_DATA_ENTRY_ID_BIT: usize = 1;
pub const OBJECT_DATA_DYNAMIC_FLAGS_BIT: usize = 2;
pub const OBJECT_DATA_SCALE_BIT: usize = 3;

const fn block_index(bit: usize) -> usize {
    bit / 32
}

const fn block_flag(bit: usize) -> u32 {
    1u32 << (bit % 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_mask_block_counts_match_cpp_template_constants() {
        let object_data = UpdateMask::new(OBJECT_DATA_BITS);
        assert_eq!(object_data.block_count(), 1);
        assert_eq!(object_data.blocks_mask_count(), 1);

        let item_data = UpdateMask::new(ITEM_DATA_BITS);
        assert_eq!(item_data.block_count(), 2);
        assert_eq!(item_data.blocks_mask_count(), 1);

        let container_data = UpdateMask::new(CONTAINER_DATA_BITS);
        assert_eq!(container_data.block_count(), 2);
        assert_eq!(container_data.blocks_mask_count(), 1);

        let client_types = UpdateMask::new(NUM_CLIENT_OBJECT_TYPES);
        assert_eq!(client_types.block_count(), 1);
        assert_eq!(client_types.blocks_mask_count(), 1);

        let unit_data = UpdateMask::new(UNIT_DATA_BITS);
        assert_eq!(unit_data.block_count(), 8);
        assert_eq!(unit_data.blocks_mask_count(), 1);

        let player_data = UpdateMask::new(PLAYER_DATA_BITS);
        assert_eq!(player_data.block_count(), 4);
        assert_eq!(player_data.blocks_mask_count(), 1);

        let active_player_data = UpdateMask::new(ACTIVE_PLAYER_DATA_BITS);
        assert_eq!(active_player_data.block_count(), 48);
        assert_eq!(active_player_data.blocks_mask_count(), 2);

        let game_object_data = UpdateMask::new(GAME_OBJECT_DATA_BITS);
        assert_eq!(game_object_data.block_count(), 1);
        assert_eq!(game_object_data.blocks_mask_count(), 1);

        let dynamic_object_data = UpdateMask::new(DYNAMIC_OBJECT_DATA_BITS);
        assert_eq!(dynamic_object_data.block_count(), 1);
        assert_eq!(dynamic_object_data.blocks_mask_count(), 1);

        let corpse_data = UpdateMask::new(CORPSE_DATA_BITS);
        assert_eq!(corpse_data.block_count(), 1);
        assert_eq!(corpse_data.blocks_mask_count(), 1);

        let area_trigger_data = UpdateMask::new(AREA_TRIGGER_DATA_BITS);
        assert_eq!(area_trigger_data.block_count(), 1);
        assert_eq!(area_trigger_data.blocks_mask_count(), 1);

        let scene_object_data = UpdateMask::new(SCENE_OBJECT_DATA_BITS);
        assert_eq!(scene_object_data.block_count(), 1);
        assert_eq!(scene_object_data.blocks_mask_count(), 1);

        let conversation_data = UpdateMask::new(CONVERSATION_DATA_BITS);
        assert_eq!(conversation_data.block_count(), 1);
        assert_eq!(conversation_data.blocks_mask_count(), 1);
    }

    #[test]
    fn update_mask_set_reset_and_blocks_mask_match_cpp_semantics() {
        let mut mask = UpdateMask::new(64);

        mask.set(0);
        mask.set(31);
        mask.set(32);

        assert_eq!(mask.get_block(0), 0x8000_0001);
        assert_eq!(mask.get_block(1), 0x0000_0001);
        assert_eq!(mask.get_blocks_mask(0), 0x0000_0003);
        assert!(mask.is_any_set());

        mask.reset(31);
        assert_eq!(mask.get_block(0), 0x0000_0001);
        assert_eq!(mask.get_blocks_mask(0), 0x0000_0003);

        mask.reset(0);
        assert_eq!(mask.get_block(0), 0);
        assert_eq!(mask.get_blocks_mask(0), 0x0000_0002);
    }

    #[test]
    fn update_mask_set_all_marks_used_blocks_and_masks_unused_tail_bits() {
        let mut mask = UpdateMask::new(35);

        mask.set_all();

        assert_eq!(mask.get_block(0), u32::MAX);
        assert_eq!(mask.get_block(1), 0x0000_0007);
        assert_eq!(mask.get_blocks_mask(0), 0x0000_0003);
    }

    #[test]
    fn update_mask_and_or_recompute_empty_block_masks() {
        let mut left = UpdateMask::new(64);
        left.set(0);
        left.set(32);
        let mut right = UpdateMask::new(64);
        right.set(32);

        let anded = left.clone() & right.clone();
        assert_eq!(anded.get_block(0), 0);
        assert_eq!(anded.get_block(1), 1);
        assert_eq!(anded.get_blocks_mask(0), 0x0000_0002);

        let ored = anded | right;
        assert_eq!(ored.get_block(1), 1);
        assert_eq!(ored.get_blocks_mask(0), 0x0000_0002);
    }
}
