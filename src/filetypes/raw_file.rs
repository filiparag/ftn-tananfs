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
    /// Create an empty file with one allocated block
    pub fn new(fs: &Arc<Mutex<Filesystem>>) -> Result<Self, Error> {
        let mut fs_handle = fs.lock()?;
        let first_block = fs_handle.acquire_block()?;
        let mut block = fs_handle.load_block(first_block)?;
        set_next_block(&mut block, NULL_BLOCK);
        fs_handle.flush_block(&block)?;
        let cursor = BlockCursor::new(&fs_handle, (BYTES_IN_U64 as u32, 0));
        Ok(Self {
            first_block,
            block_count: 1,
            size: 0,
            cursor,
            filesystem: fs.clone(),
        })
    }

    /// Create zero-initialized file with specified capacity
    pub fn with_capacity(fs: &Arc<Mutex<Filesystem>>, capacity: u64) -> Result<Self, Error> {
        let mut file = Self::new(fs)?;
        file.extend(capacity)?;
        Ok(file)
    }

    /// Create an empty file with one allocated block
    pub fn load(fs: &Arc<Mutex<Filesystem>>, inode: Inode) -> Result<Self, Error> {
        let fs_handle = fs.lock()?;
        let cursor = BlockCursor::new(&fs_handle, (BYTES_IN_U64 as u32, 0));
        Ok(Self {
            first_block: inode.first_block,
            block_count: inode.block_count,
            size: inode.size,
            cursor,
            filesystem: fs.clone(),
        })
    }

    /// Retrieve file's n-th [Block]
    pub fn get_nth_block(&self, position: u64) -> Result<Block, Error> {
        let mut fs = self.filesystem.lock()?;
        let mut current_block = fs.load_block(self.first_block)?;
        for current_index in 0..=position {
            if current_index == position {
                return Ok(current_block);
            }
            let next_block = get_next_block(&current_block);
            if next_block == NULL_BLOCK {
                return Err(Error::OutOfBounds);
            }
            current_block = fs.load_block(next_block)?;
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
        let mut fs = self.filesystem.lock()?;
        let bytes_per_block = bytes_per_block(fs.superblock.block_size) as usize;
        if buffer.len() < bytes_per_block - self.cursor.padded_byte() {
            buffer.copy_from_slice(
                &current_block.data[self.cursor.byte()..self.cursor.byte() + buffer.len()],
            );
            self.cursor.advance(buffer.len() as u64);
            return Ok(());
        }
        let mut total_read_bytes = 0;
        while total_read_bytes < buffer.len() {
            let next_block = get_next_block(&current_block);
            let read_bytes = usize::min(
                buffer.len() - total_read_bytes,
                bytes_per_block - self.cursor.padded_byte(),
            );
            buffer[total_read_bytes..total_read_bytes + read_bytes].copy_from_slice(
                &current_block.data[self.cursor.byte()..self.cursor.byte() + read_bytes],
            );
            total_read_bytes += read_bytes;
            self.cursor.advance(read_bytes as u64);
            current_block = fs.load_block(next_block)?;
        }
        Ok(())
    }

    /// Write contents of an [u8] buffer into the file  
    /// File will be extended if buffer exceeds its capacity  
    /// Use [seek](Self::seek) to set starting position and adjust buffer's length for end position
    pub fn write(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let mut current_block = self.get_nth_block(self.cursor.block())?;
        let mut fs = self.filesystem.lock()?;
        let bytes_per_block = bytes_per_block(fs.superblock.block_size) as usize;
        if buffer.len() < bytes_per_block - self.cursor.padded_byte() {
            current_block.data[self.cursor.byte()..self.cursor.byte() + buffer.len()]
                .copy_from_slice(buffer);
            self.cursor.advance(buffer.len() as u64);
            if self.cursor.position() > self.size {
                self.size = self.cursor.position();
            }
            return fs.flush_block(&current_block);
        }
        let mut total_written_bytes = 0;
        while total_written_bytes < buffer.len() {
            let next_block = get_next_block(&current_block);
            let write_bytes = usize::min(
                buffer.len() - total_written_bytes,
                bytes_per_block - self.cursor.padded_byte(),
            );
            current_block.data[self.cursor.byte()..self.cursor.byte() + write_bytes]
                .copy_from_slice(&buffer[total_written_bytes..total_written_bytes + write_bytes]);
            total_written_bytes += write_bytes;
            self.cursor.advance(write_bytes as u64);
            if total_written_bytes == buffer.len() {
                fs.flush_block(&current_block)?;
                break;
            } else if next_block == NULL_BLOCK && self.cursor.block() + 1 > self.block_count {
                let new_block = fs.acquire_block()?;
                set_next_block(&mut current_block, new_block);
                fs.flush_block(&current_block)?;
                current_block = fs.load_block(new_block)?;
                empty_block_data(&mut current_block, 0);
                set_next_block(&mut current_block, NULL_BLOCK);
                self.block_count += 1;
            } else {
                fs.flush_block(&current_block)?;
                current_block = fs.load_block(next_block)?;
            }
        }
        if self.cursor.position() > self.size {
            self.size = self.cursor.position();
        }
        Ok(())
    }

    /// Extend the file to a new capacity with trailing zeros  
    /// Seeking cursor's position will be kept
    pub fn extend(&mut self, new_capacity: u64) -> Result<(), Error> {
        if new_capacity < self.size {
            return Err(Error::InsufficientBytes);
        }
        let mut last_block = self.get_nth_block(self.block_count - 1)?;
        let mut fs = self.filesystem.lock()?;
        let previous_cursor = self.cursor.position();
        let capacity_delta = new_capacity - self.size;
        let mut total_allocated_bytes = 0;
        self.cursor.set(self.size);
        let written = empty_block_data(&mut last_block, self.cursor.byte()) as u64;
        total_allocated_bytes += written;
        self.cursor.advance(written);
        if total_allocated_bytes == capacity_delta {
            fs.flush_block(&last_block)?;
            return Ok(());
        }
        let bytes_per_block = bytes_per_block(fs.superblock.block_size);
        while total_allocated_bytes < capacity_delta {
            let next_block = fs.acquire_block()?;
            set_next_block(&mut last_block, next_block);
            fs.flush_block(&last_block)?;
            last_block = fs.load_block(next_block)?;
            empty_block_data(&mut last_block, 0);
            set_next_block(&mut last_block, NULL_BLOCK);
            total_allocated_bytes += bytes_per_block;
            self.block_count += 1;
        }
        fs.flush_block(&last_block)?;
        self.size = new_capacity;
        self.cursor.set(previous_cursor);
        Ok(())
    }

    /// Shrink the file to a new capacity  
    /// Seeking cursor's position will be kept only if it remains inside shrinked file,
    /// otherwise it is set to zero
    pub fn shrink(&mut self, new_capacity: u64) -> Result<(), Error> {
        if new_capacity > self.size {
            return Err(Error::OutOfBounds);
        }
        let previous_cursor = self.cursor.position();
        self.cursor.set(new_capacity);
        if !(self.block_count == 1 && self.cursor.block() == 0) {
            let mut last_block = self.get_nth_block(self.cursor.block())?;
            let mut fs = self.filesystem.lock()?;
            let mut current_block = fs.load_block(get_next_block(&last_block))?;
            while self.block_count > self.cursor.block() + 1 {
                let next_block = get_next_block(&current_block);
                fs.release_block(current_block.index)?;
                self.block_count -= 1;
                if next_block == NULL_BLOCK {
                    break;
                }
                current_block = fs.load_block(next_block)?;
            }
            set_next_block(&mut last_block, NULL_BLOCK);
            fs.flush_block(&last_block)?;
        }
        self.size = new_capacity;
        if new_capacity < previous_cursor {
            self.cursor.reset();
        } else {
            self.cursor.set(previous_cursor);
        }
        Ok(())
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
    fn write_and_read() {
        let dev = Cursor::new(vec![0u8; 100_000]);
        let fs = Filesystem::new(Box::new(dev), 100_000, 512);
        let fs_handle = Arc::new(Mutex::new(fs));
        let mut file = RawByteFile::new(&fs_handle).unwrap();
        let buff = (1..=1000).map(|v| v as u8).collect::<Vec<u8>>();
        _ = file.write(&buff);
        assert_eq![file.size, buff.len() as u64];
        let mut buff1 = vec![0u8; 200];
        _ = file.seek(std::io::SeekFrom::Start(100));
        _ = file.read(&mut buff1);
        assert_eq![&buff[100..300], &buff1[..]];
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
