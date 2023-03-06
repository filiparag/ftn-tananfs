use super::*;

use fuser::{FileAttr, FileType};
use std::{
    io::{Read, Seek, SeekFrom, Write},
    time::{Duration, UNIX_EPOCH},
};

impl Inode {
    pub fn attrs(&self, superblock: &Superblock) -> FileAttr {
        FileAttr {
            ino: self.index,
            size: self.size,
            blocks: self.block_count,
            atime: UNIX_EPOCH + Duration::from_secs(self.atime),
            mtime: UNIX_EPOCH + Duration::from_secs(self.mtime),
            ctime: UNIX_EPOCH + Duration::from_secs(self.ctime),
            crtime: UNIX_EPOCH + Duration::from_secs(self.ctime),
            kind: FileType::BlockDevice,
            perm: self.mode,
            nlink: 0, // unimplemented
            uid: self.uid,
            gid: self.gid,
            rdev: 0, // unimplemented
            blksize: superblock.block_size as u32,
            flags: 0, // unimplemented
        }
    }
}

impl AsBitmap for Inode {}

impl PermanentIndexed for Inode {
    type Error = crate::Error;

    fn load<D: Read + Seek>(
        block_device: &mut D,
        superblock: &Superblock,
        index: u64,
    ) -> Result<Self, Self::Error> {
        let position = superblock.inode_position(index)?;
        block_device.seek(SeekFrom::Start(position))?;
        let mut inode_raw = [0u8; std::mem::size_of::<Self>() / std::mem::size_of::<u8>()];
        block_device.read_exact(&mut inode_raw)?;
        Ok(unsafe { *(inode_raw.as_ptr() as *const Self) })
    }

    fn flush<D: Write + Seek>(
        &self,
        block_device: &mut D,
        superblock: &Superblock,
    ) -> Result<(), Self::Error> {
        let position = superblock.inode_position(self.index)?;
        block_device.seek(SeekFrom::Start(position))?;
        let inode_raw = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        };
        block_device.write_all(inode_raw)?;
        Ok(())
    }
}

impl Default for Inode {
    fn default() -> Self {
        Self {
            index: 0,
            mode: 0,
            r#type: FileType::RegularFile,
            size: 0,
            uid: 0,
            gid: 0,
            atime: 0,
            ctime: 0,
            mtime: 0,
            dtime: 0,
            block_count: 0,
            block_range: false,
            extent_sequence: false,
            blocks: [0; 6],
            __padding_1: Default::default(),
            blocks_extra: 0,
        }
    }
}
