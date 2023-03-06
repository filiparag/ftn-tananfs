use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::size_of;

use super::*;
use crate::Error;

const MINIMUM_SIZE: u64 = 1024;
const BITS_IN_BYTE: u64 = size_of::<u8>() as u64;
const BYTES_IN_USIZE: u64 = size_of::<usize>() as u64;
const BITS_IN_USIZE: u64 = BYTES_IN_USIZE * BITS_IN_BYTE;

impl<T> Bitmap<T> {
    /// Return empty bitmap with size as power of 2
    fn empty(count: u64, position: u64) -> Self {
        Self {
            bitfield: vec![0; Self::size(count)],
            count,
            position,
            __type: PhantomData,
        }
    }

    /// Calculate appropriate size in [`usize`] for bitmap
    /// Minimum size is 1024 bytes, and grows as count's next power of 2
    pub(super) fn size(count: u64) -> usize {
        let length = count.next_power_of_two();
        if length >= MINIMUM_SIZE {
            length as usize / BITS_IN_USIZE as usize
        } else {
            (MINIMUM_SIZE / BITS_IN_USIZE) as usize
        }
    }

    /// Modify occupancy
    pub(crate) fn set(&mut self, index: u64, value: bool) {
        let row = index / BITS_IN_USIZE;
        let col = index % BITS_IN_USIZE;
        if value {
            let mask = 1usize << col;
            self.bitfield[row as usize] |= mask;
        } else {
            let mask = !(1usize << col);
            self.bitfield[row as usize] &= mask;
        }
    }

    /// Get occupancy
    pub(crate) fn get(&self, index: u64) -> bool {
        let row = index / BITS_IN_USIZE;
        let col = index % BITS_IN_USIZE;
        let mask = 1usize << col;
        (self.bitfield[row as usize] & mask) != 0
    }

    /// Load bitmap from block device
    pub(crate) fn load<D: Read + Seek>(&mut self, block_device: &mut D) -> Result<(), Error> {
        block_device.seek(SeekFrom::Start(self.position))?;
        self.load_content(block_device)?;
        Ok(())
    }

    /// Flush bitmap to block device
    pub(crate) fn flush<D: Write + Seek>(&self, block_device: &mut D) -> Result<(), Error> {
        block_device.seek(SeekFrom::Start(self.position))?;
        self.flush_content(block_device)?;
        Ok(())
    }

    /// Load bitfield from block device
    fn load_content<D: Read + Seek>(&mut self, block_device: &mut D) -> Result<(), Error> {
        let buffer_size = Bitmap::<Inode>::size(self.count) * BYTES_IN_USIZE as usize;
        let mut buffer = vec![0u8; buffer_size as usize];
        block_device.read_exact(&mut buffer)?;
        for index in 0..buffer_size * BITS_IN_BYTE as usize {
            let row = index / BITS_IN_BYTE as usize;
            let col = index % BITS_IN_BYTE as usize;
            let chunk = buffer[row as usize];
            let mask = 1u8 << col;
            let bit = (chunk & mask) >> col & 1u8;
            let row = index / BITS_IN_USIZE as usize;
            let col = index % BITS_IN_USIZE as usize;
            let mask = (bit as usize) << col;
            self.bitfield[row as usize] |= mask;
        }
        Ok(())
    }

    /// Flush bitfield to block device
    fn flush_content<D: Write + Seek>(&self, block_device: &mut D) -> Result<(), Error> {
        let buffer_size = Bitmap::<Inode>::size(self.count) * BYTES_IN_USIZE as usize;
        let mut buffer = vec![0u8; buffer_size];
        for index in 0..self.count {
            let row = index / BITS_IN_USIZE;
            let col = index % BITS_IN_USIZE;
            let chunk = self.bitfield[row as usize];
            let mask = 1usize << col;
            let bit = (chunk & mask) >> col & 1usize;
            let row = index / BITS_IN_BYTE;
            let col = index % BITS_IN_BYTE;
            let mask = (bit as u8) << col;
            buffer[row as usize] |= mask;
        }
        block_device.write(&buffer)?;
        Ok(())
    }
}

impl Bitmap<Inode> {
    /// Create new bitmap with all inodes inactive
    pub fn new(superblock: &Superblock) -> Self {
        Self::empty(
            superblock.inode_count,
            superblock.bitmap_region_start() as u64,
        )
    }
}

impl Bitmap<Block> {
    pub fn new(superblock: &Superblock) -> Self {
        Self::empty(
            superblock.block_count,
            superblock.bitmap_region_start() + Bitmap::<Inode>::size(superblock.inode_count) as u64,
        )
    }
}
