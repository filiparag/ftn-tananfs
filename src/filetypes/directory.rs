use super::{Directory, DirectoryChild};
use crate::{
    filesystem::Filesystem,
    filetypes::BlockCursor,
    structs::{Block, Inode, PermanentIndexed, NULL_BLOCK},
    Error,
};
use fuser::FileType;
use std::time::SystemTime;

const BYTES_IN_U64: usize = 8;
const BYTES_IN_U16: usize = 2;

impl Directory {
    pub fn new(fs: &mut Filesystem, parent: u64, name: &str, mode: u32) -> Result<Self, Error> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let inode = fs.acquire_inode()?;
        let children_count = 0u64;
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
                block_count: 0,
                blocks: [parent, children_count, 0, 0, 0, 0],
                __padding_1: Default::default(),
                blocks_extra: NULL_BLOCK,
            },
            blocks: Vec::new(),
            name: name.to_owned(),
            children: Vec::new(),
        })
    }

    pub fn flush(&mut self, fs: &mut Filesystem) -> Result<(), Error> {
        // Calculate new size in blocks
        let mut required_bytes = 0;
        required_bytes += BYTES_IN_U16 + self.name.bytes().count();
        for (_, child) in self.children.iter() {
            required_bytes += BYTES_IN_U64 + BYTES_IN_U16 + child.bytes().count();
        }
        dbg![required_bytes];
        // Allocate required blocks
        loop {
            if required_bytes < self.blocks.len() * fs.superblock.block_size as usize {
                break;
            }
            self.blocks.push(Block::new(fs)?);
            required_bytes = if required_bytes < fs.superblock.block_size as usize {
                0
            } else {
                required_bytes - fs.superblock.block_size as usize
            };
        }
        self.inode.block_count = self.blocks.len() as u64;
        self.inode.blocks_extra = self.blocks[0].index;
        // Link blocks
        for i in 0..self.blocks.len() {
            if i < self.blocks.len() - 1 {
                let next = self.blocks[i + 1].index;
                let remainder = self.blocks[i].write_any(0, next)?;
                assert_eq!(remainder.len(), 0)
            }
        }
        let mut cursor = BlockCursor::new(fs, (BYTES_IN_U64 as u32, 0));
        // Write directory name to first block
        self.blocks[cursor.block()].write_any(cursor.byte(), self.name.bytes().count() as u16)?;
        cursor.advance(BYTES_IN_U16);
        // TODO: spillover to next block
        self.blocks[cursor.block()].write_bytes(cursor.byte(), self.name.as_bytes())?;
        cursor.advance(self.name.bytes().count());
        // Write children
        for (inode, name) in self.children.iter() {
            self.blocks[cursor.block()].write_any(cursor.byte(), *inode)?;
            cursor.advance(BYTES_IN_U64);
            self.blocks[cursor.block()].write_any(cursor.byte(), name.bytes().count() as u16)?;
            cursor.advance(BYTES_IN_U16);
            // TODO: spillover to next block
            self.blocks[cursor.block()].write_bytes(cursor.byte(), name.as_bytes())?;
            cursor.advance(name.bytes().count());
        }
        self.inode.flush(&mut fs.device, &fs.superblock)?;
        for block in self.blocks.iter_mut() {
            block.flush(&mut fs.device, &fs.superblock)?;
        }
        Ok(())
    }

    pub fn load(fs: &mut Filesystem, index: u64) -> Result<Self, Error> {
        let inode = Inode::load(&mut fs.device, &fs.superblock, index)?;
        // Load linked blocks
        let mut blocks = Vec::<Block>::with_capacity(inode.block_count as usize);
        let mut block_index = inode.blocks_extra;
        for _ in 0..inode.block_count {
            let b = Block::load(&mut fs.device, &fs.superblock, block_index)?;
            block_index = u64::from_le_bytes(b.data[0..BYTES_IN_U64].try_into()?);
            blocks.push(b);
        }
        let mut cursor = BlockCursor::new(fs, (BYTES_IN_U64 as u32, 0));
        // Load name
        let name_len = u16::from_le_bytes(
            blocks[cursor.block()].data[cursor.byte()..cursor.byte() + BYTES_IN_U16].try_into()?,
        );
        cursor.advance(BYTES_IN_U16);
        let name = std::str::from_utf8(
            &blocks[cursor.block()].data[cursor.byte()..cursor.byte() + name_len as usize],
        )?
        .to_owned();
        cursor.advance(name_len as usize);
        // Load children
        let mut children = Vec::<DirectoryChild>::with_capacity(inode.blocks[1] as usize);
        for _ in 0..children.capacity() {
            let inode = u64::from_le_bytes(
                blocks[cursor.block()].data[cursor.byte()..cursor.byte() + BYTES_IN_U64]
                    .try_into()?,
            );
            cursor.advance(BYTES_IN_U64);
            let name_len = u16::from_le_bytes(
                blocks[cursor.block()].data[cursor.byte()..cursor.byte() + BYTES_IN_U16]
                    .try_into()?,
            );
            cursor.advance(BYTES_IN_U16);
            let name = std::str::from_utf8(
                &blocks[cursor.block()].data[cursor.byte()..cursor.byte() + name_len as usize],
            )?
            .to_owned();
            cursor.advance(name_len as usize);
            children.push((inode, name));
        }
        Ok(Self {
            inode,
            blocks,
            children,
            name,
        })
    }
}
