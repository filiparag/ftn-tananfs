mod block_cursor;
mod directory;
mod directory_child;
mod helpers;
mod raw_file;

use std::sync::{Arc, Mutex};

use crate::{filesystem::Filesystem, structs::Inode, Error};

const BYTES_IN_U64: usize = 8;
const BYTES_IN_U16: usize = 2;

pub trait File: Sized {
    fn new(fs: &mut Filesystem, parent: u64) -> Result<Self, Error>;
}

#[derive(Debug, Clone)]
pub struct RawByteFile {
    pub(crate) first_block: u64,
    pub(crate) block_count: u64,
    pub(crate) size: u64,
    pub(crate) cursor: BlockCursor,
    pub(crate) filesystem: Arc<Mutex<Filesystem>>,
}


#[derive(Debug, Clone)]
pub struct DirectoryChild {
    pub(crate) inode: u64,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub struct Directory {
    pub(crate) inode: Inode,
    pub(crate) file: RawByteFile,
    pub(crate) name: String,
    pub(crate) children: Vec<DirectoryChild>,
}

#[derive(Debug, Clone)]
pub struct BlockCursor {
    pub(crate) block_size: usize,
    pub(crate) block_padding_front: usize,
    pub(crate) block_padding_back: usize,
    pub(crate) current_block: u64,
    pub(crate) current_byte: usize,
}
