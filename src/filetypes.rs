mod block_cursor;
mod directory;
use crate::{
    filesystem::Filesystem,
    structs::{Block, Inode},
    Error,
};

pub trait File: Sized {
    fn new(fs: &mut Filesystem, parent: u64) -> Result<Self, Error>;
}


#[derive(Debug, Clone)]
pub struct Directory {
    pub(crate) inode: Inode,
    pub(crate) blocks: Vec<Block>,
    pub(crate) name: String,
    pub(crate) children: Vec<DirectoryChild>,
}

#[derive(Debug, Clone)]
pub struct BlockCursor {
    pub(crate) block_size: usize,
    pub(crate) block_padding_front: usize,
    pub(crate) block_padding_back: usize,
    pub(crate) current_block: usize,
    pub(crate) current_byte: usize,
}
