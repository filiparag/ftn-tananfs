use std::fmt::Debug;
use std::io::{Read, Seek, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use log::{debug, info, warn};

use crate::structs::*;
use crate::Error;

mod cache;
mod fuse;

use cache::Cache;

pub trait BlockDevice: Read + Write + Seek + Debug {}

impl BlockDevice for std::fs::File {}

pub const DIRTY_PAGE_MAX_SECONDS: Duration = Duration::from_millis(1000);
pub const LRU_MAX_ENTRIES: usize = 131072;

#[derive(Debug)]
pub struct Filesystem {
    pub(crate) superblock: Superblock,
    pub(crate) inodes: Bitmap<Inode>,
    pub(crate) blocks: Bitmap<Block>,
    pub(crate) device: Box<dyn BlockDevice>,
    pub(crate) cache: Cache,
    pub(crate) last_flush: Option<Instant>,
}

#[derive(Debug)]
pub struct FuseFs {
    pub(crate) filesystem: Arc<Mutex<Filesystem>>,
}

impl FuseFs {
    fn fs_handle(&self) -> Result<MutexGuard<Filesystem>, Error> {
        if let Ok(fs) = self.filesystem.lock() {
            Ok(fs)
        } else {
            Err(Error::ThreadSync)
        }
    }
}

impl Filesystem {
    pub(crate) fn new(device: Box<dyn BlockDevice>, capacity: u64, block_size: u32) -> Self {
        let superblock = Superblock::new(capacity, block_size);
        assert!(block_size.is_power_of_two() && (512..=8192).contains(&block_size));
        Self {
            superblock,
            inodes: Bitmap::<Inode>::new(&superblock),
            blocks: Bitmap::<Block>::new(&superblock),
            device,
            cache: Cache::default(),
            last_flush: None,
        }
    }

    /// Returns block size of an existing filesystem on `device` by checking magic signature
    pub(crate) fn detect_existing(device: &mut dyn BlockDevice) -> Result<Option<u32>, Error> {
        for pow in 9..=13 {
            let block_size = u64::pow(2, pow);
            device.seek(std::io::SeekFrom::Start(block_size + 0x38))?;
            let mut buffer = [0u8; 2];
            device.read_exact(&mut buffer)?;
            if u16::from_le_bytes(buffer) == MAGIC_SIGNATURE {
                info!("Detected existing filesystem with block size {block_size}");
                return Ok(Some(block_size as u32));
            }
        }
        Ok(None)
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
            last_flush: None,
        })
    }

    /// Flush filesystem changes to cache and periodically call [`Self::force_flush`]
    pub(crate) fn flush(&mut self) -> Result<(), Error> {
        debug!("Invoking filesystem flush");
        if let Some(last) = self.last_flush {
            if Instant::now().duration_since(last) < DIRTY_PAGE_MAX_SECONDS {
                return Ok(());
            }
        }
        self.force_flush()?;
        Ok(())
    }

    /// Force flush filesystem changes to its block device
    pub(crate) fn force_flush(&mut self) -> Result<(), Error> {
        info!("Flushing filesystem to disk");
        self.flush_cache()?;
        self.superblock.flush(&mut self.device)?;
        self.inodes.flush(&mut self.device)?;
        self.blocks.flush(&mut self.device)?;
        self.last_flush = Some(Instant::now());
        Ok(())
    }

    fn flush_cache(&mut self) -> Result<(), Error> {
        debug!("Flushing cache to disk");
        self.cache.prune()?;
        for inode in self.cache.inodes.values_mut() {
            if inode.modified {
                inode.value.flush(&mut self.device, &self.superblock)?;
                inode.modified = false;
            }
        }
        for block in self.cache.blocks.values_mut() {
            if block.modified {
                block.value.flush(&mut self.device, &self.superblock)?;
                block.modified = false;
            }
        }
        Ok(())
    }

    /// Get index of first empty inode
    pub(crate) fn acquire_inode(&mut self) -> Result<u64, Error> {
        if let Some(index) = self.inodes.next_free(0) {
            debug!("Acquire inode {index}");
            if index >= self.superblock.inode_count {
                return Err(Error::OutOfMemory);
            }
            self.superblock.inodes_free -= 1;
            self.inodes.set(index, true)?;
            self.flush()?;
            Ok(index)
        } else {
            Err(Error::OutOfMemory)
        }
    }

    /// Release inode at index
    pub(crate) fn release_inode(&mut self, index: u64) -> Result<(), Error> {
        if self.inodes.get(index)? {
            debug!("Release inode {index}");
            self.superblock.inodes_free += 1;
            self.inodes.set(index, false)?;
            self.flush()?;
            Ok(())
        } else {
            Err(Error::DoubleRelease)
        }
    }

    /// Get index of first empty block
    pub(crate) fn acquire_block(&mut self) -> Result<u64, Error> {
        if let Some(index) = self.blocks.next_free(0) {
            if index >= self.superblock.block_count {
                return Err(Error::OutOfMemory);
            }
            debug!("Acquire block {index}");
            warn!("ACQUIRE BLOCK {index}");
            self.superblock.blocks_free -= 1;
            self.blocks.set(index, true)?;
            self.flush()?;
            Ok(index)
        } else {
            Err(Error::OutOfMemory)
        }
    }

    /// Release inode at block
    pub(crate) fn release_block(&mut self, index: u64) -> Result<(), Error> {
        if self.blocks.get(index)? {
            debug!("Release block {index}");
            self.superblock.blocks_free += 1;
            self.blocks.set(index, false)?;
            self.flush()?;
            Ok(())
        } else {
            Err(Error::DoubleRelease)
        }
    }

    /// Load inode with index
    pub(crate) fn load_inode(&mut self, index: u64) -> Result<Inode, Error> {
        if !self.inodes.get(index)? {
            return Err(Error::OutOfBounds);
        }
        debug!("Load inode {index}");
        if let Some(inode) = self.cache.get_inode(index) {
            Ok(inode)
        } else {
            let inode = Inode::load(&mut self.device, &self.superblock, index)?;
            self.cache.set_inode(&inode);
            Ok(inode)
        }
    }

    /// Load block with index.
    /// If `empty` is true, skip loading data and return zero-initialized block
    /// Next block pointer is also cleared, has to be set manually
    pub(crate) fn load_block(&mut self, index: u64, empty: bool) -> Result<Block, Error> {
        if !self.blocks.get(index)? {
            return Err(Error::OutOfBounds);
        }
        if empty {
            debug!("Load empty block {index}");
            let block = Block::with_index(self, index)?;
            self.cache.set_block(&block);
            return Ok(block);
        }
        debug!("Load block {index}");
        if let Some(block) = self.cache.get_block(index) {
            Ok(block)
        } else {
            let block = Block::load(&mut self.device, &self.superblock, index)?;
            self.cache.set_block(&block);
            Ok(block)
        }
    }

    /// Flush inode
    pub(crate) fn flush_inode(&mut self, inode: &Inode) -> Result<(), Error> {
        let index = inode.index;
        debug!("Flush inode {index}");
        self.cache.set_inode(inode);
        self.flush()?;
        Ok(())
    }

    /// Flush block
    pub(crate) fn flush_block(&mut self, block: &Block) -> Result<(), Error> {
        debug!("Flush block {}", &block.index);
        self.cache.set_block(block);
        self.flush()?;
        Ok(())
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
        for index in 4..fs.superblock.inode_count {
            assert_eq![fs.acquire_inode().unwrap(), index];
        }
        for index in 4..fs.superblock.inode_count {
            assert![fs.release_inode(index).is_ok()];
        }
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
        for index in 4..fs.superblock.block_count {
            assert_eq![fs.acquire_block().unwrap(), index];
        }
        for index in 4..fs.superblock.block_count {
            assert![fs.release_block(index).is_ok()];
        }
    }
}
