use std::io::{Read, Seek, SeekFrom, Write};

use super::*;
use crate::Error;

impl Superblock {
    pub fn new(capacity: u64, block_size: u32) -> Self {
        debug_assert!(block_size.next_power_of_two() == block_size);
        let capacity = Self::usable_capacity(capacity, block_size);
        let inode_count = capacity / DATA_PER_INODE;
        let block_count = capacity / block_size as u64;
        Self {
            inode_count,
            inodes_free: inode_count,
            block_count,
            blocks_free: block_count,
            block_size,
            __padding_1: [0; 20],
            magic: MAGIC_SIGNATURE,
            __padding_2: [0; 966],
        }
    }

    pub(crate) fn load<D: Read + Seek>(
        block_device: &mut D,
        block_size: u32,
    ) -> Result<Self, Error> {
        let position = block_size as u64;
        block_device.seek(SeekFrom::Start(position))?;
        let mut superblock_raw = [0u8; std::mem::size_of::<Self>() / std::mem::size_of::<u8>()];
        block_device.read_exact(&mut superblock_raw)?;
        Ok(unsafe { *(superblock_raw.as_ptr() as *const Self) })
    }

    pub(crate) fn flush<D: Write + Seek>(&self, block_device: &mut D) -> Result<(), Error> {
        let position = self.block_size as u64;
        block_device.seek(SeekFrom::Start(position))?;
        let superblock_raw = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        };
        block_device.write_all(superblock_raw)?;
        Ok(())
    }

    pub(super) fn usable_capacity(capacity: u64, block_size: u32) -> u64 {
        debug_assert!(capacity > block_size as u64);
        let block_size = block_size as u64;
        let boot_sector = block_size;
        let superblock = std::mem::size_of::<Self>() as u64;
        let inode = std::mem::size_of::<Inode>() as u64;
        let after_superblock = capacity - boot_sector as u64 - superblock;
        let max_inodes = after_superblock / DATA_PER_INODE;
        let max_blocks = (after_superblock - max_inodes * inode) / block_size;
        let bitmaps = (Bitmap::<Inode>::size_in_bytes(max_inodes)
            + Bitmap::<Block>::size_in_bytes(max_blocks)) as u64;
        let align = |byte| Self::align_to_block_start(byte, block_size as u32);
        let before_blocks = align(boot_sector + superblock + bitmaps + max_inodes * inode);
        debug_assert!(capacity > before_blocks);
        (capacity / block_size) * block_size - before_blocks
    }

    pub(super) fn align_to_block_start(position: u64, block_size: u32) -> u64 {
        let block_size = block_size as u64;
        if position % block_size == 0 {
            position
        } else {
            let padding = block_size - (position % block_size);
            position + padding
        }
    }

    pub(super) fn align(&self, position: u64) -> u64 {
        Self::align_to_block_start(position, self.block_size)
    }

    pub(super) fn bitmap_region_start(&self) -> u64 {
        let boot_sector = self.block_size as u64;
        boot_sector + std::mem::size_of::<Self>() as u64
    }

    pub(super) fn inode_region_start(&self) -> u64 {
        let byte = self.bitmap_region_start()
            + Bitmap::<Inode>::size_in_bytes(self.inode_count)
            + Bitmap::<Block>::size_in_bytes(self.block_count);
        Self::align_to_block_start(byte, self.block_size)
    }

    pub(super) fn block_region_start(&self) -> u64 {
        let byte =
            self.inode_region_start() + std::mem::size_of::<Inode>() as u64 * self.inode_count;
        Self::align_to_block_start(byte, self.block_size)
    }

    pub(super) fn block_region_end(&self) -> u64 {
        self.block_region_start() + self.block_size as u64 * self.block_count
    }

    pub(super) fn inode_position(&self, index: u64) -> Result<u64, Error> {
        let position = self.inode_region_start() + index * std::mem::size_of::<Inode>() as u64;
        if position < self.block_region_start() {
            Ok(position)
        } else {
            Err(Error::OutOfBounds)
        }
    }

    pub(super) fn block_position(&self, index: u64) -> Result<u64, Error> {
        let position = self.block_region_start() + index * self.block_size as u64;
        if position < self.block_region_end() {
            Ok(position)
        } else {
            Err(Error::OutOfBounds)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::structs::{Bitmap, Block, Inode};

    use super::Superblock;

    #[test]
    fn size() {
        assert_eq!(std::mem::size_of::<Superblock>(), 1024);
    }

    #[test]
    fn usable_capacity() {
        for block_exp in 9..=14 {
            let block_size = 1u64 << block_exp;
            assert_eq!(
                Superblock::usable_capacity(1_000_000, block_size as u32) % block_size,
                0
            );
            assert_eq!(
                Superblock::usable_capacity(10_000_000, block_size as u32) % block_size,
                0
            );
            assert_eq!(
                Superblock::usable_capacity(1_000_000_000, block_size as u32) % block_size,
                0
            );
        }
    }

    #[test]
    fn align_position() {
        let superblock = Superblock::new(2_048_000, 512);
        assert_eq!(superblock.align(3000), 3072);
        assert_eq!(superblock.align(4000), 4096);
        assert_eq!(superblock.align(5000), 5120);
        assert_eq!(superblock.align(5500), 5632);
    }

    #[test]
    fn regions() {
        for block_exp in 9..=14 {
            let block_size = 1u64 << block_exp;
            let superblock = Superblock::new(100_000_000, block_size as u32);
            assert_eq!(
                superblock.bitmap_region_start(),
                block_size + std::mem::size_of::<Superblock>() as u64
            );
            let inodes = block_size
                + std::mem::size_of::<Superblock>() as u64
                + (Bitmap::<Inode>::size_in_bytes(superblock.inode_count)
                    + Bitmap::<Block>::size_in_bytes(superblock.block_count))
                    as u64;
            assert_eq!(superblock.inode_region_start(), superblock.align(inodes));
            let blocks = inodes + superblock.inode_count * std::mem::size_of::<Inode>() as u64;
            assert_eq!(superblock.block_region_start(), superblock.align(blocks));
            assert_eq!(
                superblock.block_region_end(),
                superblock.align(blocks) + superblock.block_count * block_size
            );
        }
    }
}
