use std::fmt::Display;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::size_of;

use super::*;
use crate::Error;

pub const MINIMUM_SIZE: u64 = 1024;
pub const BITS_IN_BYTE: u64 = 8;
pub const BYTES_IN_USIZE: u64 = size_of::<usize>() as u64;
pub const BITS_IN_USIZE: u64 = BYTES_IN_USIZE * BITS_IN_BYTE;

impl<T: AsBitmap> Bitmap<T> {
    /// Return empty bitmap with size as power of 2
    fn empty(count: u64, position: u64) -> Self {
        Self {
            bitfield: vec![0; Self::size_in_usize(count)],
            count,
            position,
            __type: PhantomData,
        }
    }

    /// Calculate appropriate size in [`usize`] for bitmap
    /// Minimum size is 1024 bytes, and grows as count's next power of 2
    pub(super) fn size_in_usize(count: u64) -> usize {
        (Self::size_in_bytes(count) / BYTES_IN_USIZE) as usize
    }

    /// Calculate appropriate size in [`u8`] for bitmap
    pub(super) fn size_in_bytes(count: u64) -> u64 {
        let length = count.next_power_of_two() / BITS_IN_BYTE;
        if length >= MINIMUM_SIZE {
            length
        } else {
            MINIMUM_SIZE
        }
    }

    /// Modify occupancy
    pub(crate) fn set(&mut self, index: u64, value: bool) -> Result<(), Error> {
        if index >= self.count {
            return Err(Error::OutOfBounds);
        }
        let row = index / BITS_IN_USIZE;
        let col = index % BITS_IN_USIZE;
        if value {
            let mask = 1usize << col;
            self.bitfield[row as usize] |= mask;
        } else {
            let mask = !(1usize << col);
            self.bitfield[row as usize] &= mask;
        }
        Ok(())
    }

    /// Get occupancy
    pub(crate) fn get(&self, index: u64) -> Result<bool, Error> {
        if index >= self.count {
            return Err(Error::OutOfBounds);
        }
        let row = index / BITS_IN_USIZE;
        let col = index % BITS_IN_USIZE;
        let mask = 1usize << col;
        Ok((self.bitfield[row as usize] & mask) != 0)
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
        let buffer_size = Bitmap::<Inode>::size_in_bytes(self.count) as usize;
        let mut buffer = vec![0u8; buffer_size];
        block_device.read_exact(&mut buffer)?;
        for index in 0..buffer_size {
            let row = index / BITS_IN_BYTE as usize;
            let col = index % BITS_IN_BYTE as usize;
            let chunk = buffer[row];
            let mask = 1u8 << col;
            let bit = (chunk & mask) >> col & 1u8;
            let row = index / BITS_IN_USIZE as usize;
            let col = index % BITS_IN_USIZE as usize;
            let mask = (bit as usize) << col;
            self.bitfield[row] |= mask;
        }
        Ok(())
    }

    /// Flush bitfield to block device
    fn flush_content<D: Write + Seek>(&self, block_device: &mut D) -> Result<(), Error> {
        let buffer_size = Bitmap::<Inode>::size_in_bytes(self.count) as usize;
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
        block_device.write_all(&buffer)?;
        Ok(())
    }

    /// Get index of first empty field starting at `after`
    pub(crate) fn next_free(&self, after: u64) -> Option<u64> {
        let after_chunk = after / BITS_IN_USIZE;
        let after_bit = after % BITS_IN_USIZE;
        for chunk in after_chunk as usize..self.bitfield.len() {
            if self.bitfield[chunk] == usize::MAX {
                continue;
            }
            for bit in after_bit..(BYTES_IN_USIZE * BITS_IN_BYTE) {
                if self.bitfield[chunk] & 1usize << bit == 0 {
                    let index = chunk as u64 * BITS_IN_USIZE + bit;
                    return Some(index);
                }
            }
        }
        None
    }
}

impl Bitmap<Inode> {
    /// Create new bitmap with all inodes inactive
    pub fn new(superblock: &Superblock) -> Self {
        Self::empty(superblock.inode_count, superblock.bitmap_region_start())
    }
}

impl Bitmap<Block> {
    pub fn new(superblock: &Superblock) -> Self {
        Self::empty(
            superblock.block_count,
            superblock.bitmap_region_start()
                + Bitmap::<Inode>::size_in_bytes(superblock.inode_count),
        )
    }
}

impl<T: AsBitmap> Display for Bitmap<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Bitmap {{",)?;
        writeln!(f, "    count: {}", { self.count })?;
        writeln!(f, "    position: {}", { self.position })?;
        writeln!(f, "    bitfield: [")?;
        for bit in 0..self.count {
            if self.get(bit).unwrap() {
                writeln!(f, "        {bit}")?;
            }
        }
        writeln!(f, "    ]")?;
        write!(f, "}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{Bitmap, BITS_IN_USIZE};
    use crate::structs::{Inode, Superblock};

    #[test]
    fn size() {
        assert_eq!(Bitmap::<Inode>::size_in_bytes(0), 1024);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(10), 1024);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(200), 1024);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(1024), 1024);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(8192), 1024);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(8200), 2048);
        assert_eq!(Bitmap::<Inode>::size_in_bytes(100_000), 16384);
    }

    #[test]
    fn empty() {
        let superblock = Superblock::new(100_000, 512);
        let bitmap = Bitmap::<Inode>::new(&superblock);
        assert_eq!(bitmap.bitfield.len(), 128);
        bitmap
            .bitfield
            .iter()
            .for_each(|&chunk| assert_eq!(chunk, 0));
    }

    #[test]
    fn set_and_get() {
        let superblock = Superblock::new(100_000_000, 512);
        let mut bitmap = Bitmap::<Inode>::new(&superblock);
        assert!(bitmap.set(100, true).is_ok());
        assert!(bitmap.set(1000, true).is_ok());
        assert!(bitmap.set(10000, true).is_ok());
        assert!(bitmap.set(30000, true).is_err());
        assert_eq!(bitmap.get(10).unwrap(), false);
        assert_eq!(bitmap.get(99).unwrap(), false);
        assert_eq!(bitmap.get(100).unwrap(), true);
        assert_eq!(bitmap.get(101).unwrap(), false);
        assert_eq!(bitmap.get(999).unwrap(), false);
        assert_eq!(bitmap.get(1000).unwrap(), true);
        assert_eq!(bitmap.get(1001).unwrap(), false);
        assert_eq!(bitmap.get(9999).unwrap(), false);
        assert_eq!(bitmap.get(10000).unwrap(), true);
        assert_eq!(bitmap.get(10001).unwrap(), false);
        assert_eq!(bitmap.get(20000).unwrap(), false);
        assert!(bitmap.get(30000).is_err());
    }

    #[test]
    fn load_and_flush() {
        let superblock = Superblock::new(100_000, 4096);
        let mut bitmap = Bitmap::<Inode>::new(&superblock);
        let mut dev = Cursor::new(vec![0u8; superblock.inode_region_start() as usize]);
        assert!(bitmap.load(&mut dev).is_ok());
        assert!(bitmap.flush(&mut dev).is_ok());
    }

    #[test]
    fn next_free() {
        let superblock = Superblock::new(10_000_000, 512);
        let mut bitmap = Bitmap::<Inode>::new(&superblock);
        for index in 0..BITS_IN_USIZE * 2 {
            assert_eq!(bitmap.get(index).unwrap(), false);
            assert_eq!(bitmap.next_free(index), Some(index));
            assert!(bitmap.set(index, true).is_ok());
        }
    }
}
