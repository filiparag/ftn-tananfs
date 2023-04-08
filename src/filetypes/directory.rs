use super::{helpers::*, RawByteFile};
use super::{Directory, DirectoryChild};
use crate::structs::{Inode, NULL_BLOCK};
use crate::{Error, Filesystem};

use fuser::FileType;
use std::sync::{Arc, Mutex};

impl Directory {
    pub fn new(
        fs: &Arc<Mutex<Filesystem>>,
        parent: u64,
        name: &str,
        mode: u32,
    ) -> Result<Self, Error> {
        let now = timestamp_now();
        let inode = fs.lock()?.acquire_inode()?;
        let children_count = 0u64;
        let file = RawByteFile::new(fs)?;
        Ok(Self {
            inode: Inode {
                index: inode,
                mode: mode as u16,
                r#type: FileType::Directory,
                size: 0,
                uid: 0,
                gid: 0,
                atime: now,
                ctime: now,
                mtime: now,
                dtime: u64::MAX,
                block_count: 1,
                metadata: [
                    parent,
                    children_count,
                    name.as_bytes().len() as u64,
                    NULL_BLOCK,
                    NULL_BLOCK,
                    NULL_BLOCK,
                ],
                __padding_1: Default::default(),
                first_block: file.first_block,
            },
            file,
            name: name.to_owned(),
            children: Vec::new(),
            filesystem: fs.clone(),
        })
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.file.cursor.reset();
        self.file.write(self.name.as_bytes())?;
        for child in self.children.iter() {
            child.flush(&mut self.file)?;
        }
        self.inode.mtime = timestamp_now();
        self.inode.block_count = self.file.block_count;
        self.inode.size = self.file.cursor.position();
        self.inode.metadata[1] = self.children.len() as u64;
        self.inode.metadata[2] = self.name.as_bytes().len() as u64;
        self.filesystem.lock()?.flush_inode(&self.inode)?;
        Ok(())
    }

    pub fn load(fs: &Arc<Mutex<Filesystem>>, index: u64) -> Result<Self, Error> {
        let mut fs_handle = fs.lock()?;
        let inode = fs_handle.load_inode(index)?;
        let children_count = inode.metadata[1];
        let name_len = inode.metadata[2] as usize;
        drop(fs_handle);
        let mut file = RawByteFile::load(fs, inode)?;
        let name = read_string(&mut file, name_len)?;
        let mut children = Vec::<DirectoryChild>::with_capacity(children_count as usize);
        for _ in 0..children_count {
            children.push(DirectoryChild::read(&mut file)?);
        }
        Ok(Self {
            inode,
            file,
            name,
            children,
            filesystem: fs.clone(),
        })
    }

    pub fn remove(mut self) -> Result<(), Error> {
        self.file.shrink(0)?;
        let mut fs_handle = self.filesystem.lock()?;
        fs_handle.release_block(self.inode.first_block)?;
        fs_handle.release_inode(self.inode.index)?;
        Ok(())
    }
}
