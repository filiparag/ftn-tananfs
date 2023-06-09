use super::{helpers::*, FileOperations, RawByteFile, RegularFile};
use crate::filetypes::Directory;
use crate::structs::{Inode, NULL_BLOCK};
use crate::{Error, Filesystem};

use fuser::FileType;
use log::{debug, error};
use std::io::Seek;
use std::sync::{Arc, Mutex};

impl RegularFile {
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
        self.modified = true;
        if self.file.seek(std::io::SeekFrom::Start(offset))? != offset {
            return Err(Error::InsufficientBytes);
        };
        self.inode.atime = timestamp_now();
        self.inode.mtime = timestamp_now();
        self.file.write(data)?;
        Ok(())
    }

    pub fn remove(mut self) -> Result<(), Error> {
        RawByteFile::remove(&self.file.filesystem, self.inode.index)?;
        let mut fs_handle = self.file.filesystem.lock()?;
        fs_handle.release_inode(self.inode.index)?;
        self.removed = true;
        Ok(())
    }
}

impl FileOperations for RegularFile {
    fn new(fs: &Arc<Mutex<Filesystem>>, parent: u64, name: &str, mode: u32) -> Result<Self, Error> {
        let now = timestamp_now();
        let inode = fs.lock()?.acquire_inode()?;
        let file = RawByteFile::new(fs)?;
        Directory::load(&fs, parent)?.add_child(name, inode)?;
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
                metadata: [parent, NULL_BLOCK, NULL_BLOCK, NULL_BLOCK, NULL_BLOCK],
                __padding_1: Default::default(),
                first_block: file.first_block,
                last_block: file.last_block,
            },
            file,
            modified: true,
            removed: false,
        })
    }

    fn load(fs: &Arc<Mutex<Filesystem>>, index: u64) -> Result<Self, Error> {
        let mut fs_handle = fs.lock()?;
        let inode = fs_handle.load_inode(index)?;
        drop(fs_handle);
        let file = RawByteFile::load(fs, inode)?;
        Ok(Self {
            inode,
            file,
            modified: false,
            removed: false,
        })
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.modified = false;
        let index = self.inode.index;
        debug!("Flush regular file {index}");
        self.file.update_inode(&mut self.inode);
        self.inode.mtime = timestamp_now();
        self.inode.block_count = self.file.block_count;
        self.inode.size = self.file.size;
        self.file.filesystem.lock()?.flush_inode(&self.inode)?;
        Ok(())
    }

    fn remove(mut self) -> Result<(), Error> {
        let index = self.inode.index;
        debug!("Remove regular file {index}");
        RawByteFile::remove(&self.file.filesystem, self.inode.index)?;
        Directory::load(&self.file.filesystem, self.inode.metadata[0])?.remove_child(
            crate::filetypes::DirectoryChildIdentifier::Inode(self.inode.index),
        )?;
        let mut fs_handle = self.file.filesystem.lock()?;
        fs_handle.release_inode(self.inode.index)?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for RegularFile {
    fn drop(&mut self) {
        if self.removed || !self.modified {
            return;
        }
        if let Err(e) = self.flush() {
            let index = self.inode.index;
            error!("Error flushing dropped regular file {index}: {e}")
        }
    }
}
