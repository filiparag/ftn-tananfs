use std::{
    collections::{BTreeMap, BinaryHeap},
    time::Instant,
};

use crate::{
    error::Error,
    structs::{Block, Inode, PermanentIndexed},
};

use super::{Filesystem, LRU_MAX_ENTRIES};

#[derive(Debug, Default)]
pub struct Cache {
    pub(super) inodes: BTreeMap<u64, CacheLine<Inode>>,
    pub(super) blocks: BTreeMap<u64, CacheLine<Block>>,
}

#[derive(Debug)]
pub struct CacheLine<T: Clone> {
    pub(super) value: T,
    pub(super) modified: bool,
    pub(super) atime: Instant,
}

#[derive(Debug)]
pub enum LruLine {
    Inode(Instant, u64),
    Block(Instant, u64),
}

impl LruLine {
    fn duration(&self) -> u64 {
        let last = match self {
            Self::Inode(d, _) => *d,
            Self::Block(d, _) => *d,
        };
        (Instant::now() - last).as_millis() as u64
    }
}

impl Cache {
    pub fn prune(&mut self) -> Result<(), Error> {
        let mut lru = BinaryHeap::<LruLine>::with_capacity(self.inodes.len() + self.blocks.len());
        self.inodes
            .values()
            .filter(|v| !v.modified)
            .for_each(|v| lru.push(v.lru_line()));
        self.blocks
            .values()
            .filter(|v| !v.modified)
            .for_each(|v| lru.push(v.lru_line()));
        lru.into_sorted_vec()
            .iter()
            .skip(LRU_MAX_ENTRIES)
            .for_each(|item| match *item {
                LruLine::Inode(_, index) => _ = self.inodes.remove(&index),
                LruLine::Block(_, index) => _ = self.blocks.remove(&index),
            });
        Ok(())
    }

    pub fn get_inode(&mut self, index: u64) -> Option<Inode> {
        if let Some(line) = self.inodes.get_mut(&index) {
            line.atime = Instant::now();
            Some(line.get().clone())
        } else {
            None
        }
    }

    pub fn get_block(&mut self, index: u64) -> Option<Block> {
        if let Some(line) = self.blocks.get_mut(&index) {
            line.atime = Instant::now();
            Some(line.get().clone())
        } else {
            None
        }
    }

    pub fn set_inode(&mut self, inode: &Inode) {
        let index = inode.index;
        if let Some(line) = self.inodes.get_mut(&index) {
            line.update(&inode);
        } else {
            self.inodes.insert(index, CacheLine::new(inode));
        }
    }

    pub fn set_block(&mut self, block: &Block) {
        let index = block.index;
        if let Some(line) = self.blocks.get_mut(&index) {
            line.update(&block);
        } else {
            self.blocks.insert(index, CacheLine::new(block));
        }
    }
}

impl<T: Clone + PartialEq> CacheLine<T> {
    pub fn new(value: &T) -> Self {
        Self {
            value: value.clone(),
            modified: false,
            atime: Instant::now(),
        }
    }

    pub fn get(&mut self) -> &T {
        self.atime = Instant::now();
        &self.value
    }

    pub fn update(&mut self, value: &T) {
        if &self.value != value {
            self.atime = Instant::now();
            self.modified = true;
            self.value = value.clone()
        }
    }
}

impl CacheLine<Inode> {
    fn lru_line(&self) -> LruLine {
        LruLine::Inode(self.atime, self.value.index)
    }
}

impl CacheLine<Block> {
    fn flush(&mut self, fs: &mut Filesystem) -> Result<(), Error> {
        self.modified = false;
        self.value.flush(&mut fs.device, &fs.superblock)
    }

    fn lru_line(&self) -> LruLine {
        LruLine::Block(self.atime, self.value.index)
    }
}

impl PartialEq for LruLine {
    fn eq(&self, other: &Self) -> bool {
        self.duration() == other.duration()
    }
}

impl PartialOrd for LruLine {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        if self.duration() > other.duration() {
            Some(Greater)
        } else {
            Some(Less)
        }
    }
}

impl Eq for LruLine {}

impl Ord for LruLine {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).expect("unexpected none")
    }
}
