use super::{helpers::*, DirectoryChildIdentifier, FileOperations, RawByteFile, RegularFile};
use super::{Directory, DirectoryChild};
use crate::filesystem::ROOT_INODE;
use crate::structs::{Inode, NULL_BLOCK};
use crate::{Error, Filesystem};

use fuser::FileType;
use log::{debug, error};
use std::sync::{Arc, Mutex};

impl Directory {
    pub fn get_child_inode(&self, child: DirectoryChildIdentifier) -> Result<u64, Error> {
        Ok(match child {
            DirectoryChildIdentifier::Name(name) => {
                match self.children.iter().find(|c| c.name == name) {
                    Some(child) => child.inode,
                    None => return Err(Error::NotFound),
                }
            }
            DirectoryChildIdentifier::Inode(index) => index,
        })
    }

    pub fn add_child(&mut self, name: &str, inode: u64) -> Result<(), Error> {
        self.modified = true;
        let index = self.inode.index;
        debug!(
            "Add child {name} with inode {inode} to directory {} with inode {index}",
            self.name
        );
        let child = DirectoryChild {
            inode,
            name: name.to_owned(),
        };
        if !self.children.contains(&child) {
            self.children.push(child);
            Ok(())
        } else {
            Err(Error::NameOrInodeDuplicate)
        }
    }

    pub fn remove_empty(mut self) -> Result<(), Error> {
        let index = self.inode.index;
        debug!("Remove empty directory {} with inode {index}", self.name);
        if !self.children.is_empty() {
            return Err(Error::DirectoryNotEmpty);
        }
        RawByteFile::remove(&self.file.filesystem, self.inode.index)?;
        let mut fs_handle = self.file.filesystem.lock()?;
        fs_handle.release_inode(self.inode.index)?;
        self.removed = true;
        Ok(())
    }

    pub fn transfer_child(
        &mut self,
        child: DirectoryChildIdentifier,
        new_parent: u64,
        new_name: &str,
    ) -> Result<(), Error> {
        let index = self.inode.index;
        let child = self.get_child_inode(child)?;
        debug!(
            "Transfer child with inode {child} from directory with inode {index} to {new_parent}"
        );
        if new_parent == self.inode.index {
            match self.children.iter_mut().find(|c| c.inode == child) {
                Some(child) => child.name = new_name.into(),
                None => unreachable!(),
            }
        } else {
            let mut new_parent = Directory::load(&self.file.filesystem, new_parent)?;
            new_parent.add_child(new_name, child)?;
            self.children.retain(|c| c.inode != child);
        }
        self.modified = true;
        Ok(())
    }

    pub fn remove_child(&mut self, child: DirectoryChildIdentifier) -> Result<(), Error> {
        self.modified = true;
        let index = self.inode.index;
        let child = self.get_child_inode(child)?;
        debug!(
            "Remove child with inode {index} from directory {} with inode {index}",
            self.name
        );
        let inode = self.file.filesystem.lock()?.load_inode(child)?;
        match inode.r#type {
            FileType::RegularFile => {
                RegularFile::load(&self.file.filesystem, inode.index)?.remove()?;
            }
            FileType::Directory => {
                Directory::load(&self.file.filesystem, inode.index)?.remove_empty()?;
            }
            _ => return Err(Error::NullBlock),
        }
        self.children.retain(|c| c.inode != inode.index);
        Ok(())
    }
}

impl FileOperations for Directory {
    fn new(fs: &Arc<Mutex<Filesystem>>, parent: u64, name: &str, mode: u32) -> Result<Self, Error> {
        let now = timestamp_now();
        let inode = fs.lock()?.acquire_inode()?;
        let children_count = 0u64;
        let file = RawByteFile::new(fs)?;
        if parent == ROOT_INODE && inode == ROOT_INODE {
            debug!("Root directory, skip adding to parent");
        } else {
            Directory::load(fs, parent)?.add_child(name, inode)?;
        }
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
                ],
                __padding_1: Default::default(),
                first_block: file.first_block,
                last_block: file.last_block,
            },
            file,
            name: name.to_owned(),
            children: Vec::new(),
            modified: true,
            removed: false,
        })
    }

    fn load(fs: &Arc<Mutex<Filesystem>>, index: u64) -> Result<Self, Error> {
        debug!("Load directory with inode {index}");
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
            modified: false,
            removed: false,
        })
    }

    fn flush(&mut self) -> Result<(), Error> {
        let index = self.inode.index;
        debug!("Flush directory {} with inode {index}", self.name);
        self.file.cursor.reset();
        self.file.write(self.name.as_bytes())?;
        for child in self.children.iter() {
            child.flush(&mut self.file)?;
        }
        self.file.update_inode(&mut self.inode);
        self.inode.mtime = timestamp_now();
        self.inode.block_count = self.file.block_count;
        self.inode.size = self.file.cursor.position();
        self.inode.metadata[1] = self.children.len() as u64;
        self.inode.metadata[2] = self.name.as_bytes().len() as u64;
        self.file.filesystem.lock()?.flush_inode(&self.inode)?;
        self.modified = false;
        Ok(())
    }

    fn remove(mut self) -> Result<(), Error> {
        let index = self.inode.index;
        debug!(
            "Recursively remove directory {} with inode {index}",
            self.name
        );
        self.modified = true;
        let mut fs_handle = self.file.filesystem.lock()?;
        for child in &self.children {
            let inode = fs_handle.load_inode(child.inode)?;
            match inode.r#type {
                FileType::RegularFile => {
                    RegularFile::load(&self.file.filesystem, inode.index)?.remove()?;
                }
                FileType::Directory => {
                    Directory::load(&self.file.filesystem, inode.index)?.remove()?;
                }
                _ => unreachable!(),
            }
        }
        drop(fs_handle);
        self.remove_empty()?;
        Ok(())
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if self.removed || !self.modified {
            return;
        }
        if let Err(e) = self.flush() {
            let index = self.inode.index;
            error!("Error flushing dropped directory {index}: {e}")
        }
    }
}
