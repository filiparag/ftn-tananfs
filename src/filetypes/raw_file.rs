use std::{
    io::Seek,
    sync::{Arc, Mutex},
};

use crate::{
    structs::{Block, Inode, NULL_BLOCK},
    Error, Filesystem,
};

use super::{helpers::*, BlockCursor, RawByteFile, BYTES_IN_U64};

impl RawByteFile {
    /// Create an empty file with no allocated blocks
    pub fn new(fs: &Arc<Mutex<Filesystem>>) -> Result<Self, Error> {
        let fs_handle = fs.lock()?;
        let cursor = BlockCursor::new(&fs_handle, (BYTES_IN_U64 as u32, 0));
        Ok(Self {
            first_block: NULL_BLOCK,
            last_block: NULL_BLOCK,
            block_count: 0,
            size: 0,
            cursor,
            filesystem: fs.clone(),
        })
    }

    /// Create zero-initialized file with specified capacity
    pub fn with_capacity(fs: &Arc<Mutex<Filesystem>>, capacity: u64) -> Result<Self, Error> {
        let mut file = Self::new(fs)?;
        file.extend(capacity)?;
        assert_eq!(file.cursor.position(), 0);
        Ok(file)
    }

    /// Load file for given [Inode]
    pub fn load(fs: &Arc<Mutex<Filesystem>>, inode: Inode) -> Result<Self, Error> {
        let fs_handle = fs.lock()?;
        let cursor = BlockCursor::new(&fs_handle, (BYTES_IN_U64 as u32, 0));
        Ok(Self {
            first_block: inode.first_block,
            last_block: inode.last_block,
            block_count: inode.block_count,
            size: inode.size,
            cursor,
            filesystem: fs.clone(),
        })
    }

    /// Bytes per block available for data
    fn bytes_per_block(&self) -> Result<usize, Error> {
        let fs = self.filesystem.lock()?;
        Ok(bytes_per_block(fs.superblock.block_size) as usize)
    }

    /// Retrieve file's n-th [Block]
    pub fn get_nth_block(&self, position: u64) -> Result<Block, Error> {
        let mut fs = self.filesystem.lock()?;
        if self.first_block == NULL_BLOCK {
            return Err(Error::NullBlock);
        }
        // Skip lookup for last block
        if position + 1 == self.block_count {
            return fs.load_block(self.last_block, false);
        }
        // println!("do lookup for {}", position);
        let mut current_block = fs.load_block(self.first_block, false)?;
        for current_index in 0..=position {
            if current_index == position {
                return Ok(current_block);
            }
            let next_block = get_next_block(&current_block);
            if next_block == NULL_BLOCK {
                return Err(Error::OutOfBounds);
            }
            current_block = fs.load_block(next_block, false)?;
        }
        Err(Error::OutOfBounds)
    }

    /// Read contents of the file into an [u8] buffer
    /// Use [seek](Self::seek) to set starting position and adjust buffer's length for end position
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        if buffer.len() as u64 > self.size - self.cursor.position() {
            return Err(Error::OutOfBounds);
        }
        let mut current_block = self.get_nth_block(self.cursor.block())?;
        let mut total_read_bytes = 0;
        while total_read_bytes < buffer.len() {
            let read = read_from_block(
                &mut current_block,
                self.cursor.byte(),
                &mut buffer[total_read_bytes..],
            );
            total_read_bytes += read;
            self.cursor.advance(read as u64);
            if total_read_bytes == buffer.len() {
                break;
            }
            let mut fs_handle = self.filesystem.lock()?;
            let next_block = get_next_block(&current_block);
            current_block = fs_handle.load_block(next_block, false)?;
        }
        Ok(())
    }

    /// Write contents of an [u8] buffer into the file
    /// File will be extended if buffer exceeds its capacity
    /// Use [seek](Self::seek) to set starting position and adjust buffer's length for end position
    pub fn write(&mut self, buffer: &[u8]) -> Result<(), Error> {
        if self.first_block == NULL_BLOCK {
            self.initialize()?;
        }
        let mut current_block = self.get_nth_block(self.cursor.block())?;
        let mut total_written_bytes = 0;
        while total_written_bytes < buffer.len() {
            let written = write_to_block(
                &mut current_block,
                self.cursor.byte(),
                &buffer[total_written_bytes..],
            );
            total_written_bytes += written;
            self.cursor.advance(written as u64);
            if total_written_bytes == buffer.len() {
                break;
            }
            let next_block;
            let mut fs_handle = self.filesystem.lock()?;
            fs_handle.flush_block(&current_block)?;
            drop(fs_handle);
            if get_next_block(&current_block) == NULL_BLOCK {
                next_block = self.append_block()?;
            } else {
                next_block = get_next_block(&current_block);
            }
            let mut fs_handle = self.filesystem.lock()?;
            current_block = fs_handle.load_block(next_block, false)?;
        }
        let mut fs_handle = self.filesystem.lock()?;
        fs_handle.flush_block(&current_block)?;
        if self.cursor.position() > self.size {
            self.size = self.cursor.position();
        }
        Ok(())
    }

    /// Initialize first block if file is empty
    pub fn initialize(&mut self) -> Result<(), Error> {
        let mut fs_handle = self.filesystem.lock()?;
        let index = fs_handle.acquire_block()?;
        let mut block = fs_handle.load_block(index, true)?;
        set_next_block(&mut block, NULL_BLOCK);
        fs_handle.flush_block(&block)?;
        self.first_block = block.index;
        self.last_block = block.index;
        self.block_count = 1;
        self.cursor.reset();
        Ok(())
    }

    /// Append an empty block to file's end
    /// File size and seeking cursor's position will be kept
    /// Needs housekeeping after being called
    fn append_block(&mut self) -> Result<u64, Error> {
        let mut fs_handle = self.filesystem.lock()?;
        let mut old_last_block = fs_handle.load_block(self.last_block, false)?;
        let next_block: u64 = fs_handle.acquire_block()?;
        set_next_block(&mut old_last_block, next_block);
        fs_handle.flush_block(&old_last_block)?;
        let mut new_last_block = fs_handle.load_block(next_block, true)?;
        set_next_block(&mut new_last_block, NULL_BLOCK);
        fs_handle.flush_block(&new_last_block)?;
        self.last_block = next_block;
        self.block_count += 1;
        Ok(next_block)
    }

    /// Extend the file to a new capacity with trailing zeros
    /// Seeking cursor's position will be kept
    pub fn extend(&mut self, new_capacity: u64) -> Result<(), Error> {
        if new_capacity < self.size {
            return Err(Error::InsufficientBytes);
        }
        if self.first_block == NULL_BLOCK {
            self.initialize()?;
        }
        let mut fs_handle = self.filesystem.lock()?;
        let capacity_delta = new_capacity - self.size;
        let bytes_per_block = bytes_per_block(fs_handle.superblock.block_size);
        let mut last_block = fs_handle.load_block(self.last_block, false)?;
        // New capacity fits into existing blocks
        if new_capacity <= self.block_count * bytes_per_block {
            assert_eq!(get_next_block(&last_block), NULL_BLOCK);
            assert!(capacity_delta <= bytes_per_block);
            empty_block_data(
                &mut last_block,
                (fs_handle.superblock.block_size as u64 - capacity_delta) as usize,
            );
            self.size = new_capacity;
            fs_handle.flush_block(&last_block)?;
            return Ok(());
        }
        // New capacity exceeds existing blocks
        let previous_cursor = self.cursor.position();
        self.cursor.set(self.size);
        let written = empty_block_data(&mut last_block, self.cursor.byte()) as u64;
        let mut total_allocated_bytes = written;
        fs_handle.flush_block(&last_block)?;
        drop(fs_handle);
        while total_allocated_bytes < capacity_delta {
            self.append_block()?;
            total_allocated_bytes += bytes_per_block;
        }
        self.size = new_capacity;
        self.cursor.set(previous_cursor);
        Ok(())
    }

    /// Shrink the file to a new capacity
    /// Seeking cursor's position will be kept only if it remains inside shrinked file,
    /// otherwise it is set to zero
    pub fn shrink(&mut self, new_capacity: u64) -> Result<(), Error> {
        if new_capacity == self.size {
            return Ok(());
        }
        if new_capacity > self.size {
            return Err(Error::OutOfBounds);
        }
        let previous_cursor = self.cursor.position();
        self.cursor.set(new_capacity);
        let last_block = self.get_nth_block(self.cursor.block())?;
        let mut fs_handle = self.filesystem.lock()?;
        let block_delta = self.block_count - (self.cursor.block() + 1);
        // Check if blocks have to be released
        if block_delta > 0 {
            let mut current_block = get_next_block(&last_block);
            for _ in 0..block_delta {
                assert_ne!(current_block, NULL_BLOCK);
                let block = fs_handle.load_block(current_block, false)?;
                fs_handle.release_block(block.index)?;
                self.block_count -= 1;
                current_block = get_next_block(&block);
            }
        }
        if new_capacity > 0 {
            self.size = new_capacity;
            self.last_block = last_block.index;
            if new_capacity < previous_cursor {
                self.cursor.reset();
            } else {
                self.cursor.set(previous_cursor);
            }
        } else {
            assert_eq!(self.first_block, last_block.index);
            fs_handle.release_block(self.first_block)?;
            self.block_count -= 1;
            assert_eq!(self.block_count, 0);
            self.size = 0;
            self.first_block = NULL_BLOCK;
            self.last_block = NULL_BLOCK;
            self.cursor.reset();
        }
        Ok(())
    }

    /// Remove file for given [Inode] index
    pub fn remove(fs: &Arc<Mutex<Filesystem>>, inode: u64) -> Result<(), Error> {
        let inode = {
            let mut fs_handle = fs.lock()?;
            fs_handle.load_inode(inode)?
        };
        let mut file = Self::load(fs, inode)?;
        file.shrink(0)?;
        let mut fs_handle = fs.lock()?;
        assert_eq!(file.first_block, NULL_BLOCK);
        fs_handle.release_inode(inode.index)?;
        Ok(())
    }

    /// Update [Inode]'s block pointers
    pub fn update_inode(&self, inode: &mut Inode) {
        inode.first_block = self.first_block;
        inode.last_block = self.last_block;
    }
}

impl Seek for RawByteFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        #[allow(clippy::comparison_chain)]
        Ok(match pos {
            std::io::SeekFrom::Start(bytes) => {
                if bytes > self.size {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "out of bounds",
                    ));
                }
                self.cursor.set(bytes)
            }
            std::io::SeekFrom::End(bytes) => {
                self.cursor.set(self.size);
                if bytes > 0 {
                    if self.cursor.position() + bytes as u64 >= self.size {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "out of bounds",
                        ));
                    }
                    self.cursor.advance(bytes as u64)
                } else if bytes < 0 {
                    if self.cursor.position() as i64 + bytes < 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "out of bounds",
                        ));
                    }
                    self.cursor.regress((-bytes) as u64)
                } else {
                    0
                }
            }
            std::io::SeekFrom::Current(bytes) => {
                if bytes > 0 {
                    if self.cursor.position() + bytes as u64 >= self.size {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "out of bounds",
                        ));
                    }
                    self.cursor.advance(bytes as u64)
                } else if bytes < 0 {
                    if self.cursor.position() as i64 + bytes < 0 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "out of bounds",
                        ));
                    }
                    self.cursor.regress((-bytes) as u64)
                } else {
                    0
                }
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::{Filesystem, RawByteFile};
    use std::{
        io::{Cursor, Seek},
        sync::{Arc, Mutex},
    };

    #[test]
    fn seek_file() {
        let dev = Cursor::new(vec![0u8; 100_000]);
        let fs = Filesystem::new(Box::new(dev), 100_000, 512);
        let fs_handle = Arc::new(Mutex::new(fs));
        let mut file = RawByteFile::with_capacity(&fs_handle, 10_000).unwrap();
        assert_eq![file.seek(std::io::SeekFrom::Start(1_000)).unwrap(), 1_000];
        assert_eq![file.seek(std::io::SeekFrom::Current(111)).unwrap(), 1_111];
        assert_eq![file.seek(std::io::SeekFrom::Current(-50)).unwrap(), 1_061];
        assert_eq![file.seek(std::io::SeekFrom::End(-999)).unwrap(), 9_001];
        assert![file.seek(std::io::SeekFrom::Start(11_000)).is_err()];
        _ = file.seek(std::io::SeekFrom::Start(9_001));
        assert![file.seek(std::io::SeekFrom::Current(1_000)).is_err()];
        assert![file.seek(std::io::SeekFrom::End(11_000)).is_err()];
    }

    #[test]
    fn extend_and_shrink() {
        let dev = Cursor::new(vec![0u8; 100_000]);
        let fs = Filesystem::new(Box::new(dev), 100_000, 512);
        let fs_handle = Arc::new(Mutex::new(fs));
        let mut file = RawByteFile::new(&fs_handle).unwrap();
        assert_eq!(file.block_count, 0);
        _ = file.extend(1024);
        assert_eq!(file.block_count, 3);
        _ = file.extend(90_000);
        assert_eq!(file.block_count, 179);
        _ = file.shrink(80_000);
        assert_eq!(file.block_count, 159);
        _ = file.shrink(2000);
        assert_eq!(file.block_count, 4);
        _ = file.shrink(1800);
        assert_eq!(file.block_count, 4);
        _ = file.shrink(100);
        assert_eq!(file.block_count, 1);
        _ = file.shrink(1);
        assert_eq!(file.block_count, 1);
        _ = file.shrink(0);
        assert_eq!(file.block_count, 0);
        drop(file);
        let mut file = RawByteFile::with_capacity(&fs_handle, 50_000).unwrap();
        assert_eq!(file.block_count, 100);
        _ = file.extend(60_000);
        assert_eq!(file.block_count, 120);
        assert_eq!(file.size, 60_000);
        _ = file.shrink(50_000);
        assert_eq!(file.block_count, 100);
        _ = file.shrink(0);
        drop(file);
        let fs = fs_handle.lock().unwrap();
        let blocks_used = fs.superblock.block_count - fs.superblock.blocks_free;
        assert_eq!(blocks_used, 0);
    }

    #[test]
    fn write_and_read() {
        let dev = Cursor::new(vec![0u8; 20_000_000]);
        let fs = Filesystem::new(Box::new(dev), 20_000_000, 512);
        let fs_handle = Arc::new(Mutex::new(fs));
        for capacity in (0..=1001).step_by(331) {
            for write_buffer in (400..=100_000).step_by(2017) {
                for read_buffer in (201..=write_buffer - 100).step_by(1013) {
                    for seek in (30..50).step_by(1013) {
                        let mut file =
                            RawByteFile::with_capacity(&fs_handle, capacity * 123).unwrap();
                        let buff = (1..=write_buffer)
                            .map(|v| (v / 504 + 1) as u8)
                            .collect::<Vec<u8>>();
                        let mut buff1 = vec![0u8; read_buffer];
                        assert!(buff1.len() <= buff.len());
                        assert!(file.write(&buff).is_ok());
                        assert!(file.seek(std::io::SeekFrom::Start(seek as u64)).is_ok());
                        assert!(file.read(&mut buff1).is_ok());
                        assert_eq![&buff[seek..seek + read_buffer], &buff1[..]];
                        assert!(file.shrink(0).is_ok());
                    }
                }
            }
        }
    }

    #[test]
    fn extend_and_shrink() {
        let dev = Cursor::new(vec![0u8; 100_000]);
        let fs = Filesystem::new(Box::new(dev), 100_000, 512);
        let fs_handle = Arc::new(Mutex::new(fs));
        let mut file = RawByteFile::new(&fs_handle).unwrap();
        let buff = (1..=50).map(|v| v as u8).collect::<Vec<u8>>();
        _ = file.write(&buff);
        _ = file.extend(200);
        let mut buff1 = vec![0u8; 200];
        _ = file.seek(std::io::SeekFrom::Start(0));
        _ = file.read(&mut buff1);
        assert_eq![&buff, &buff1[0..50]];
        assert_eq![&buff1[50..200], [0u8; 150]];
        let mut buff2 = vec![0u8; 30];
        _ = file.shrink(30);
        _ = file.seek(std::io::SeekFrom::Start(0));
        _ = file.read(&mut buff2);
        assert_eq![&buff[0..30], &buff2];
        _ = file.extend(20000);
        _ = file.shrink(15);
        let mut buff3 = vec![0u8; 15];
        _ = file.read(&mut buff3);
        assert_eq![&buff[0..15], &buff3];
    }
}
