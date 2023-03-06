#![allow(unused)]

mod bitmap;
mod inode;
mod superblock;

use std::{
    io::{Read, Seek, Write},
    marker::PhantomData,
};

use fuser::FileType;

const BITMAP_WORD: u64 = std::mem::size_of::<usize>() as u64;
const BITMAP_MIN_SIZE: u64 = 1024;
const DATA_PER_INODE: u64 = 4096;
const MAGIC_SIGNATURE: u16 = 0xEF53;

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
    __padding_1: [u8; 20],
    /// Magic signature
    pub(crate) magic: u16,
    #[doc(hidden)]
    __padding_2: [u8; 966],
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
    /// File is stored in contiguous blocks
    /// starting from [`blocks`](Self::blocks)`[0]`
    /// and ending after [`block_count`](Self::block_count) blocks
    pub(crate) block_range: bool,
    /// Block occupation exceeds [`blocks`](Self::blocks) size, and
    /// is continued in extent sequence residing in
    /// block [`blocks_extra`](Self::blocks_extra)
    pub(crate) extent_sequence: bool,
    /// Beginning of sequence of blocks containing file's data
    pub(crate) blocks: [u64; 6],
    #[doc(hidden)]
    __padding_1: [bool; 3],
    /// Block containing continuation of block sequence if
    /// [`extent_sequence`](Self::extent_sequence) is `true`
    pub(crate) blocks_extra: u64,
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Block's index
    pub(crate) index: u64,
    /// Raw data as bytes
    pub(crate) data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Bitmap<T> {
    /// Bits mapping to indexes
    pub bitfield: Vec<usize>,
    /// Number of valid indexes
    pub count: u64,
    /// Position
    pub position: u64,
    #[doc(hidden)]
    __type: PhantomData<T>,
}
