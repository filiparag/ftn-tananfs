mod block_cursor;
mod directory;
mod directory_child;
mod helpers;
mod raw_file;
mod regular_file;

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
    pub(crate) last_block: u64,
    pub(crate) block_count: u64,
    pub(crate) size: u64,
    pub(crate) cursor: BlockCursor,
    pub(crate) filesystem: Arc<Mutex<Filesystem>>,
}

#[derive(Debug, Clone)]
pub struct RegularFile {
    pub(crate) inode: Inode,
    pub(crate) file: RawByteFile,
    pub(crate) modified: bool,
    pub(crate) removed: bool,
}

#[derive(Debug, Clone)]
pub struct DirectoryChild {
    pub(crate) inode: u64,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub enum DirectoryChildIdentifier<'a> {
    Name(&'a str),
    Inode(u64),
}

#[derive(Debug, Clone)]
pub struct Directory {
    pub(crate) inode: Inode,
    pub(crate) file: RawByteFile,
    pub(crate) name: String,
    pub(crate) children: Vec<DirectoryChild>,
    pub(crate) modified: bool,
    pub(crate) removed: bool,
}

#[derive(Debug, Clone)]
pub struct BlockCursor {
    pub(crate) block_size: usize,
    pub(crate) block_padding_front: usize,
    pub(crate) block_padding_back: usize,
    pub(crate) current_block: u64,
    pub(crate) current_byte: usize,
}

pub trait FileOperations
where
    Self: Sized,
{
    fn new(fs: &Arc<Mutex<Filesystem>>, parent: u64, name: &str, mode: u32) -> Result<Self, Error>;
    fn load(fs: &Arc<Mutex<Filesystem>>, index: u64) -> Result<Self, Error>;
    fn flush(&mut self) -> Result<(), Error>;
    fn remove(self) -> Result<(), Error>;
}
