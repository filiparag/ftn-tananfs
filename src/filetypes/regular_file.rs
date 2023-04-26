use super::{helpers::*, RawByteFile, RegularFile};
use crate::structs::{Inode, NULL_BLOCK};
use crate::{Error, Filesystem};

use fuser::FileType;
use std::io::Seek;
use std::sync::{Arc, Mutex};

impl RegularFile {
    pub fn new(fs: &Arc<Mutex<Filesystem>>, parent: u64, mode: u32) -> Result<Self, Error> {
        let now = timestamp_now();
        let inode = fs.lock()?.acquire_inode()?;
        let file = RawByteFile::new(fs)?;
        Ok(Self {
            inode: Inode {
                index: inode,
                mode: mode as u16,
                r#type: FileType::RegularFile,
                size: 0,
                uid: 0,
                gid: 0,
                atime: now,
                ctime: now,
                mtime: now,
                dtime: u64::MAX,
                block_count: 1,
                metadata: [
                    parent, NULL_BLOCK, NULL_BLOCK, NULL_BLOCK, NULL_BLOCK, NULL_BLOCK,
                ],
                __padding_1: Default::default(),
                first_block: file.first_block,
            },
            file,
        })
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.inode.mtime = timestamp_now();
        self.inode.block_count = self.file.block_count;
        self.inode.size = self.file.size;
        self.file.filesystem.lock()?.flush_inode(&self.inode)?;
        Ok(())
    }

    pub fn load(fs: &Arc<Mutex<Filesystem>>, index: u64) -> Result<Self, Error> {
        let mut fs_handle = fs.lock()?;
        let inode = fs_handle.load_inode(index)?;
        drop(fs_handle);
        let file = RawByteFile::load(fs, inode)?;
        Ok(Self { inode, file })
    }

    pub fn read(&mut self, offset: u64, size: u64) -> Result<Vec<u8>, Error> {
        if self.file.seek(std::io::SeekFrom::Start(offset))? != offset {
            return Err(Error::InsufficientBytes);
        };
        let lookahead_size = self.file.size - self.file.cursor.current();
        let mut buffer;
        if size > lookahead_size {
            buffer = vec![0; lookahead_size as usize];
        } else {
            buffer = vec![0; size as usize];
        }
        self.inode.atime = timestamp_now();
        self.file.read(&mut buffer)?;
        Ok(buffer)
    }

    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<(), Error> {
        if self.file.seek(std::io::SeekFrom::Start(offset))? != offset {
            return Err(Error::InsufficientBytes);
        };
        self.inode.atime = timestamp_now();
        self.inode.mtime = timestamp_now();
        self.file.write(data)?;
        Ok(())
    }
}

impl Drop for RegularFile {
    fn drop(&mut self) {
        self.flush().expect("failed to flush dropped file")
    }
}
