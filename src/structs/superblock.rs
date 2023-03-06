use std::io::{Read, Seek, SeekFrom, Write};

use super::*;
use crate::Error;

impl Superblock {
    pub fn new(capacity: u64, block_size: u32) -> Self {
        let capacity = Self::usable_capacity(capacity, block_size as u64);
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

    pub(super) fn usable_capacity(capacity: u64, block_size: u64) -> u64 {
        let boot_sector = block_size;
        let inode_count = capacity / DATA_PER_INODE;
        let block_count = capacity / block_size;
        capacity
            - boot_sector
            - std::mem::size_of::<Self>() as u64
            - Bitmap::<Inode>::size(inode_count) as u64
            - Bitmap::<Block>::size(block_count) as u64
    }

    pub(super) fn bitmap_region_start(&self) -> u64 {
        let boot_sector = self.block_size as u64;
        boot_sector + std::mem::size_of::<Self>() as u64
    }

    pub(super) fn inode_region_start(&self) -> u64 {
        self.bitmap_region_start()
            + Bitmap::<Inode>::size(self.inode_count) as u64
            + Bitmap::<Block>::size(self.block_count) as u64
    }

    pub(super) fn block_region_start(&self) -> u64 {
        self.inode_region_start() + std::mem::size_of::<Inode>() as u64 * self.inode_count
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

impl From<&Superblock> for &[u8] {
    fn from(data: &Superblock) -> Self {
        let raw = data as *const Superblock as *const u8;
        unsafe { core::slice::from_raw_parts(raw, std::mem::size_of::<Superblock>()) }
    }
}
