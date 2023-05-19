mod bitmap;
mod block;
mod inode;
mod superblock;

use std::{
    io::{Read, Seek, Write},
    marker::PhantomData,
};

use fuser::FileType;

pub use bitmap::*;

pub const METADATA_IN_INODE: usize = 5;
pub const DATA_PER_INODE: u64 = 4096;
pub const MAGIC_SIGNATURE: u16 = 0xEF53;
pub const NULL_BLOCK: u64 = u64::MAX;

pub(crate) trait PermanentIndexed: Sized {
    type Error;
    fn load<D: Read + Seek>(
        block_device: &mut D,
        superblock: &Superblock,
        index: u64,
    ) -> Result<Self, Self::Error>;
    fn flush<D: Write + Seek>(
        &self,
        block_device: &mut D,
        superblock: &Superblock,
    ) -> Result<(), Self::Error>;
}

pub trait AsBitmap {}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Superblock {
    /// Total count of inodes in the filesystem
    pub(crate) inode_count: u64,
    /// Count of free inodes in the filesystem
    pub(crate) inodes_free: u64,
    /// Total count of blocks in the filesystem
    pub(crate) block_count: u64,
    /// Count of free blocks in the filesystem
    pub(crate) blocks_free: u64,
    /// Block size in bytes
    pub(crate) block_size: u32,
    #[doc(hidden)]
    pub(crate) __padding_1: [u8; 20],
    /// Magic signature
    pub(crate) magic: u16,
    #[doc(hidden)]
    pub(crate) __padding_2: [u8; 966],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Inode {
    /// Inode's index
    pub(crate) index: u64,
    /// File mode (permissions)
    pub(crate) mode: u16,
    /// File type
    pub(crate) r#type: FileType,
    /// File size in bytes
    pub(crate) size: u64,
    /// Owner UID
    pub(crate) uid: u32,
    /// Owner GID
    pub(crate) gid: u32,
    /// Last access timestamp in seconds
    pub(crate) atime: u64,
    /// Last metadata modification timestamp in seconds
    pub(crate) ctime: u64,
    /// Last data modification timestamp in seconds
    pub(crate) mtime: u64,
    /// Deletion timestamp in seconds ([`u64::MAX`](core::u64::MAX) if not deleted)
    pub(crate) dtime: u64,
    /// Occupied block count
    pub(crate) block_count: u64,
    /// Raw slice for additional optional metadata
    pub(crate) metadata: [u64; METADATA_IN_INODE],
    #[doc(hidden)]
    pub(crate) __padding_1: [bool; 5],
    /// Index of file's first block. Set to
    /// Every extra block references next in sequence in its first 8 bytes.
    pub(crate) first_block: u64,
    pub(crate) last_block: u64,
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Block's index
    pub(crate) index: u64,
    /// Raw data as bytes
    pub(crate) data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Bitmap<T: AsBitmap> {
    /// Bits mapping to indexes
    pub bitfield: Vec<usize>,
    /// Number of valid indexes
    pub count: u64,
    /// Position
    pub position: u64,
    #[doc(hidden)]
    __type: PhantomData<T>,
}
