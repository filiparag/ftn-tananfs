use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Read;
use std::io::Seek;
use std::io::Write;

use crate::structs::*;
use crate::Error;

pub trait BlockDevice: Read + Write + Seek + Debug {}

#[derive(Debug, Default)]
struct Cache {
    inodes: BTreeMap<u64, Inode>,
    blocks: BTreeMap<u64, Block>,
}

#[derive(Debug)]
pub struct Filesystem {
    superblock: Superblock,
    inodes: Bitmap<Inode>,
    blocks: Bitmap<Block>,
    device: Box<dyn BlockDevice>,
    cache: Cache,
}

impl Filesystem {
    pub(crate) fn new(device: Box<dyn BlockDevice>, capacity: u64, block_size: u32) -> Self {
        let superblock = Superblock::new(capacity, block_size);
        Self {
            superblock,
            inodes: Bitmap::<Inode>::new(&superblock),
            blocks: Bitmap::<Block>::new(&superblock),
            device: device,
            cache: Cache::default(),
        }
    }

    /// Load filesystem from a block device
    pub(crate) fn load(device: Box<dyn BlockDevice>, block_size: u32) -> Result<Self, Error> {
        let mut device = device;
        let superblock = Superblock::load(&mut device, block_size)?;
        let mut bitmaps = (
            Bitmap::<Inode>::new(&superblock),
            Bitmap::<Block>::new(&superblock),
        );
        bitmaps.0.load(&mut device)?;
        bitmaps.1.load(&mut device)?;
        Ok(Self {
            superblock,
            inodes: bitmaps.0,
            blocks: bitmaps.1,
            device,
            cache: Cache::default(),
        })
    }

    /// Flush filesystem changes to its block device
    pub(crate) fn flush(&mut self) -> Result<(), Error> {
        for (_, block) in &self.cache.blocks {
            block.flush(&mut self.device, &self.superblock)?;
        }
        for (_, inode) in &self.cache.inodes {
            inode.flush(&mut self.device, &self.superblock)?;
        }
        self.superblock.flush(&mut self.device)?;
        self.inodes.flush(&mut self.device)?;
        self.blocks.flush(&mut self.device)?;
        Ok(())
    }

    /// Get index of first empty inode
    pub(crate) fn acquire_inode(&mut self) -> Result<u64, Error> {
        if let Some(index) = self.inodes.next_free(0) {
            self.superblock.inodes_free -= 1;
            self.inodes.set(index, true)?;
            Ok(index)
        } else {
            Err(Error::OutOfMemory)
        }
    }

    /// Release inode at index
    pub(crate) fn release_inode(&mut self, index: u64) -> Result<(), Error> {
        if self.inodes.get(index)? {
            self.superblock.inodes_free += 1;
            self.inodes.set(index, false)?;
            Ok(())
        } else {
            Err(Error::DoubleRelease)
        }
    }

    /// Get index of first empty block
    pub(crate) fn acquire_block(&mut self) -> Result<u64, Error> {
        if let Some(index) = self.blocks.next_free(0) {
            self.superblock.blocks_free -= 1;
            self.blocks.set(index, true)?;
            Ok(index)
        } else {
            Err(Error::OutOfMemory)
        }
    }

    /// Release inode at block
    pub(crate) fn release_block(&mut self, index: u64) -> Result<(), Error> {
        if self.blocks.get(index)? {
            self.superblock.blocks_free += 1;
            self.blocks.set(index, false)?;
            Ok(())
        } else {
            Err(Error::DoubleRelease)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{BlockDevice, Filesystem};

    impl BlockDevice for Cursor<Vec<u8>> {}

    #[test]
    fn load_and_flush() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let mut fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        assert![fs.acquire_block().is_ok()];
        assert!(fs.flush().is_ok());
        let dev = fs.device;
        let fs = Filesystem::load(dev, 512).unwrap();
        assert_eq![fs.blocks.get(0).unwrap(), true];
        assert_eq![fs.superblock.block_count - fs.superblock.blocks_free, 1];
    }

    #[test]
    fn acquire_and_release_inode() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let mut fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        assert_eq![fs.acquire_inode().unwrap(), 0];
        assert_eq![fs.acquire_inode().unwrap(), 1];
        assert_eq![fs.acquire_inode().unwrap(), 2];
        assert![fs.release_inode(1).is_ok()];
        assert![fs.release_inode(1).is_err()];
        assert_eq![fs.acquire_inode().unwrap(), 1];
        assert_eq![fs.acquire_inode().unwrap(), 3];
    }

    #[test]
    fn acquire_and_release_block() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let mut fs = Filesystem::new(Box::new(dev), 10_000_000, 4096);
        assert_eq![fs.acquire_block().unwrap(), 0];
        assert_eq![fs.acquire_block().unwrap(), 1];
        assert_eq![fs.acquire_block().unwrap(), 2];
        assert![fs.release_block(0).is_ok()];
        assert![fs.release_block(0).is_err()];
        assert_eq![fs.acquire_block().unwrap(), 0];
        assert_eq![fs.acquire_block().unwrap(), 3];
    }
}