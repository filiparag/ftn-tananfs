use super::*;

impl Superblock {
    pub fn new(capacity: u64, block_size: u64) -> Self {
        let capacity = Self::usable_capacity(capacity, block_size);
        let inode_count = capacity / DATA_PER_INODE;
        let block_count = capacity / block_size;
        Self {
            inode_count,
            inodes_free: inode_count,
            block_count,
            blocks_free: block_count,
            block_size,
            __padding_1: [0; 16],
            magic: MAGIC_SIGNATURE,
            __padding_2: [0; 966],
        }
    }
    pub(super) fn usable_capacity(capacity: u64, block_size: u64) -> u64 {
        let boot_sector = block_size;
        let inode_count = capacity / DATA_PER_INODE;
        let block_count = capacity / block_size;
        capacity
            - boot_sector
            - std::mem::size_of::<Superblock>() as u64
            - Bitmap::size(inode_count)
            - Bitmap::size(block_count)
    }
}

impl From<&Superblock> for &[u8] {
    fn from(data: &Superblock) -> Self {
        let data = data as *const Superblock as *const u8;
        unsafe { core::slice::from_raw_parts(data, std::mem::size_of::<Superblock>()) }
    }
}
