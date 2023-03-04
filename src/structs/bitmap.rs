use super::*;

impl Bitmap {
    pub(super) fn new(count: u64) -> Self {
        Self {
            bitfield: vec![0; Self::size(count) as usize],
            count,
        }
    }
    pub(super) fn size(count: u64) -> u64 {
        let length = (count / BITMAP_WORD).next_power_of_two();
        if length >= 1024 / BITMAP_WORD {
            length
        } else {
            1024 / BITMAP_WORD
        }
    }
    pub fn new_block_bitmap(superblock: &Superblock) -> Self {
        Self::new(superblock.block_count)
    }
    pub fn new_inode_bitmap(superblock: &Superblock) -> Self {
        Self::new(superblock.inode_count)
    }
    pub fn set(&mut self, index: u64, value: bool) {
        let row = index / BITMAP_WORD;
        let col = index % BITMAP_WORD;
        if value {
            let mask = 1u128 << col;
            self.bitfield[row as usize] |= mask;
        } else {
            let mask = !(1u128 << col);
            self.bitfield[row as usize] &= mask;
        }
    }
    pub fn get(&self, index: u64) -> bool {
        let row = index / BITMAP_WORD;
        let col = index % BITMAP_WORD;
        let mask = 1u128 << col;
        (self.bitfield[row as usize] & mask) != 0
    }
    pub fn len(&self) -> u64 {
        return self.bitfield.len() as u64 * BITMAP_WORD;
    }
}

impl From<&Bitmap> for Vec<u8> {
    fn from(map: &Bitmap) -> Self {
        let mut buffer = Vec::<u8>::with_capacity(map.count as usize);
        for row in map.bitfield.iter() {
            for col in 0..16 {
                let mask = 0xFFu128 << (col * 8);
                let value = (row & mask) >> (col * 8);
                buffer.push(value as u8)
            }
        }
        buffer
    }
}
