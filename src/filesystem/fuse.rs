use fuser::FileType;
use log::{debug, error, info, warn};
use std::time::Duration;

use crate::{
    error::Error,
    filesystem::ROOT_INODE,
    filetypes::{Directory, FileOperations, RegularFile},
};

use super::FuseFs;

impl fuser::Filesystem for FuseFs {
    fn init(
        &mut self,
        _req: &fuser::Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        info!("Initializing filesystem");
        if self.fs_handle()?.inodes.get(ROOT_INODE)? {
            debug!("Reusing existing root directory");
        } else {
            self.fs_handle()?.inodes.set(0, true)?;
            debug!(
                "Skipped inode 0, current is {}",
                self.fs_handle()?.inodes.next_free(0).unwrap()
            );
            Directory::new(&self.filesystem, ROOT_INODE, "root", 0o750)?;
            info!("Root directory created");
        }
        self.fs_handle()?.force_flush()?;
        debug!("Success");
        Ok(())
    }

    fn access(&mut self, _req: &fuser::Request<'_>, ino: u64, mask: i32, reply: fuser::ReplyEmpty) {
        info!("Accessing inode {ino} with mask {mask}");
        reply.ok();
        debug!("Success");
    }

    fn readdir(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: fuser::ReplyDirectory,
    ) {
        info!("Reading directory {ino} with offset {offset}");
        let inner = || -> Result<(), Error> {
            match Directory::load(&self.filesystem, ino) {
                Ok(dir) => {
                    if offset == 0 {
                        let parent = dir.inode.metadata[0];
                        let _ = reply.add(ino, 0, fuser::FileType::Directory, ".");
                        let _ = reply.add(parent, 1, fuser::FileType::Directory, "..");
                        debug!("Listed parent and self inode");
                    }
                    for (index, child) in dir.children.iter().skip(offset as usize).enumerate() {
                        let inode = self.fs_handle()?.load_inode(ino)?;
                        debug!("Listed child inode {}", child.name);
                        if reply.add(ino, offset + index as i64 + 3, inode.r#type, &child.name) {
                            debug!("Buffer full");
                            break;
                        }
                    }
                    reply.ok();
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn lookup(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        reply: fuser::ReplyEntry,
    ) {
        info!("Lookup {name:?} in directory with inode {parent}");
        let inner = || -> Result<(), Error> {
            let dir = Directory::load(&self.filesystem, parent)?;
            let name = name.to_string_lossy();
            match dir.get_child_inode(crate::filetypes::DirectoryChildIdentifier::Name(&name)) {
                Ok(child) => {
                    drop(dir);
                    let inode = self.fs_handle()?.load_inode(child)?;
                    let attrs = inode.attrs(&self.fs_handle()?.superblock);
                    reply.entry(&Duration::from_secs(0), &attrs, 0);
                    debug!("Loaded attributes");
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn rmdir(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Remove directory {name:?} with parent {parent}");
        let inner = || -> Result<(), Error> {
            let mut dir = Directory::load(&self.filesystem, parent)?;
            let name = name.to_str().unwrap();
            if let Err(e) = dir.remove_child(crate::filetypes::DirectoryChildIdentifier::Name(name))
            {
                warn!("Error: {e}");
                reply.error(e.into());
                Ok(())
            } else {
                reply.ok();
                debug!("Success");
                Ok(())
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn read(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: fuser::ReplyData,
    ) {
        info!("Read {size} bytes from file {ino:?} with offset {offset}");
        let inner = || -> Result<(), Error> {
            match RegularFile::load(&self.filesystem, ino) {
                Ok(mut file) => {
                    let data = file.read(offset as u64, size as u64)?;
                    reply.data(&data);
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn write(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: fuser::ReplyWrite,
    ) {
        info!(
            "Write {} bytes to file {ino:?} with offset {offset}",
            data.len()
        );
        let inner = || -> Result<(), Error> {
            match RegularFile::load(&self.filesystem, ino) {
                Ok(mut file) => {
                    file.write(offset as u64, data)?;
                    reply.written(data.len() as u32);
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn fallocate(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        length: i64,
        mode: i32,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Allocate {length} bytes in file {ino:?} at offset {offset}");
        let inner = || -> Result<(), Error> {
            match RegularFile::load(&self.filesystem, ino) {
                Ok(mut file) => {
                    let size = file.file.size as i64;
                    let new_size = size - offset + length;
                    if new_size > size {
                        file.file.extend(new_size as u64)?;
                    } else {
                        file.file.shrink(new_size as u64)?;
                    }
                    file.modified = true;
                    file.inode.mode = mode as u16;
                    reply.ok();
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn getattr(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyAttr) {
        info!("Get attributes for inode {ino}");
        let inner = || -> Result<(), Error> {
            let inode = match self.fs_handle()?.load_inode(ino) {
                Ok(inode) => inode,
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    return Ok(());
                }
            };
            let attrs = inode.attrs(&self.fs_handle()?.superblock);
            reply.attr(&Duration::from_secs(0), &attrs);
            debug!("Success");
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn setattr(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        flags: Option<u32>,
        reply: fuser::ReplyAttr,
    ) {
        info!("Set attributes for inode {ino}");
        let inner = || -> Result<(), Error> {
            let mut inode = match self.fs_handle()?.load_inode(ino) {
                Ok(inode) => inode,
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    return Ok(());
                }
            };
            if let Some(mode) = mode {
                debug!("Setting mode to {mode:0o}");
                inode.mode = mode as u16;
            }
            if let Some(uid) = uid {
                debug!("Setting uid to {uid}");
                inode.uid = uid;
            }
            if let Some(gid) = gid {
                debug!("Setting gid to {gid}");
                inode.gid = gid;
            }
            if let Some(flags) = flags {
                debug!("Setting flags to {flags}");
            }
            self.fs_handle()?.flush_inode(&inode)?;
            debug!("Flushing inode");
            reply.attr(
                &Duration::new(0, 0),
                &inode.attrs(&self.fs_handle()?.superblock),
            );
            debug!("Success");
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn open(&mut self, _req: &fuser::Request<'_>, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
        info!("Open file {ino}");
        let inner = || -> Result<(), Error> {
            match self.fs_handle()?.load_inode(ino) {
                Ok(inode) => {
                    if inode.r#type == FileType::RegularFile {
                        reply.opened(0, fuser::consts::FOPEN_DIRECT_IO);
                        debug!("Success");
                        Ok(())
                    } else {
                        warn!("Unable to open non-regular file");
                        reply.error(libc::EACCES);
                        Ok(())
                    }
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn opendir(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _flags: i32,
        reply: fuser::ReplyOpen,
    ) {
        info!("Open directory {ino}");
        let inner = || -> Result<(), Error> {
            match self.fs_handle()?.load_inode(ino) {
                Ok(inode) => {
                    if inode.r#type == FileType::Directory {
                        reply.opened(0, fuser::consts::FOPEN_DIRECT_IO);
                        debug!("Success");
                        Ok(())
                    } else {
                        warn!("Unable to open file as a directory");
                        reply.error(libc::EACCES);
                        Ok(())
                    }
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn mknod(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: fuser::ReplyEntry,
    ) {
        info!("Make node {name:?} in parent directory {parent}");
        let inner = || -> Result<(), Error> {
            let name = name.to_str().unwrap();
            match RegularFile::new(&self.filesystem, parent, name, mode) {
                Ok(file) => {
                    reply.entry(
                        &Duration::from_secs(0),
                        &file.inode.attrs(&self.fs_handle()?.superblock),
                        0,
                    );
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn mkdir(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        mode: u32,
        _umask: u32,
        reply: fuser::ReplyEntry,
    ) {
        info!("Make directory {name:?} in parent directory {parent}");
        let inner = || -> Result<(), Error> {
            let name = name.to_str().unwrap();
            match Directory::new(&self.filesystem, parent, name, mode) {
                Ok(dir) => {
                    reply.entry(
                        &Duration::from_secs(0),
                        &dir.inode.attrs(&self.fs_handle()?.superblock),
                        0,
                    );
                    debug!("Success");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn unlink(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Unlink {name:?} from parent directory {parent}");
        let inner = || -> Result<(), Error> {
            let name = name.to_str().unwrap();
            match Directory::load(&self.filesystem, parent) {
                Ok(mut dir) => {
                    match dir.remove_child(crate::filetypes::DirectoryChildIdentifier::Name(name)) {
                        Err(e) => reply.error(e.into()),
                        Ok(_) => {
                            reply.ok();
                            debug!("Success");
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn destroy(&mut self) {
        info!("Destroying filesystem");
        let inner = || -> Result<(), Error> {
            self.fs_handle()?.force_flush()?;
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn rename(
        &mut self,
        _req: &fuser::Request<'_>,
        parent: u64,
        name: &std::ffi::OsStr,
        newparent: u64,
        newname: &std::ffi::OsStr,
        _flags: u32,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Rename {name:?} to {newname:?}");
        let inner = || -> Result<(), Error> {
            let name = name.to_str().unwrap();
            let new_name = newname.to_str().unwrap();
            match Directory::load(&self.filesystem, parent)?.transfer_child(
                crate::filetypes::DirectoryChildIdentifier::Name(name),
                newparent,
                new_name,
            ) {
                Ok(()) => {
                    reply.ok();
                    Ok(())
                }
                Err(e) => {
                    warn!("Error: {e}");
                    reply.error(e.into());
                    Ok(())
                }
            }
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn flush(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        _lock_owner: u64,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Filesystem flush requested for inode {ino}");
        let inner = || -> Result<(), Error> {
            match self.fs_handle()?.flush() {
                Ok(()) => {
                    debug!("Success");
                    reply.ok();
                }
                Err(e) => {
                    error!("Error: {e}");
                    reply.error(e.into());
                }
            }
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn fsync(
        &mut self,
        _req: &fuser::Request<'_>,
        ino: u64,
        _fh: u64,
        _datasync: bool,
        reply: fuser::ReplyEmpty,
    ) {
        info!("Filesystem flush requested for inode {ino}");
        let inner = || -> Result<(), Error> {
            match self.fs_handle()?.force_flush() {
                Ok(()) => {
                    debug!("Success");
                    reply.ok();
                }
                Err(e) => {
                    error!("Error: {e}");
                    reply.error(e.into());
                }
            }
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }

    fn statfs(&mut self, _req: &fuser::Request<'_>, _ino: u64, reply: fuser::ReplyStatfs) {
        info!("Get filesystem statistics");
        let inner = || -> Result<(), Error> {
            let spb = &self.fs_handle()?.superblock;
            let padded_block_size = spb.block_size - 8;
            reply.statfs(
                spb.block_count,
                spb.blocks_free,
                spb.blocks_free,
                spb.inode_count - spb.inodes_free,
                spb.inodes_free,
                padded_block_size,
                u16::MAX as u32,
                padded_block_size,
            );
            Ok(())
        };
        inner().unwrap_or_else(|e| error!("Unexpected error: {e}"));
    }
}
