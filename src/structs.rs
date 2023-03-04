#![allow(unused)]

mod bitmap;
mod superblock;

pub use bitmap::*;
pub use superblock::*;

const BITMAP_WORD: u64 = 128;
const DATA_PER_INODE: u64 = 4096;
const MAGIC_SIGNATURE: u16 = 0xEF53;

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
    pub(crate) block_size: u64,
    #[doc(hidden)]
    __padding_1: [u8; 16],
    /// Magic signature
    pub(crate) magic: u16,
    #[doc(hidden)]
    __padding_2: [u8; 966],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Inode {
    /// File mode (permissions)
    pub(crate) mode: u16,
    /// File type
    pub(crate) r#type: u8,
    /// File size in bytes
    pub(crate) size: u64,
    /// Owner UID
    pub(crate) uid: u16,
    /// Owner GID
    pub(crate) gid: u16,
    /// Last access timestamp in seconds
    pub(crate) atime: i64,
    /// Creation timestamp in seconds
    pub(crate) ctime: i64,
    /// Last modification timestamp in seconds
    pub(crate) mtime: i64,
    /// Deletion timestamp in seconds (zero if undeleted)
    pub(crate) dtime: i64,
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
    pub(crate) blocks: [u64; 7],
    #[doc(hidden)]
    __padding_1: [bool; 7],
    /// Block containing continuation of block sequence if
    /// [`extent_sequence`](Self::extent_sequence) is `true`
    pub(crate) blocks_extra: u64,
}

#[derive(Debug, Clone)]
pub struct Bitmap {
    bitfield: Vec<u128>,
    count: u64,
}
