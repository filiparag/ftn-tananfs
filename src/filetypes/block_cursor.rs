use super::BlockCursor;

use crate::Filesystem;

impl BlockCursor {
    pub fn new(fs: &Filesystem, padding: (u32, u32)) -> Self {
        Self {
            block_size: fs.superblock.block_size as usize,
            block_padding_front: padding.0 as usize,
            block_padding_back: padding.1 as usize,
            current_block: 0,
            current_byte: padding.0 as usize,
        }
    }

    pub fn from(
        fs: &Filesystem,
        starting_block: u64,
        starting_byte: u32,
        padding: (u32, u32),
    ) -> Self {
        Self {
            block_size: fs.superblock.block_size as usize,
            block_padding_front: padding.0 as usize,
            block_padding_back: padding.1 as usize,
            current_block: starting_block as usize,
            current_byte: starting_byte as usize,
        }
    }

    pub fn advance(&mut self, bytes: usize) {
        let padded_block = self.block_size - self.block_padding_front - self.block_padding_back;
        let remaining_bytes = self.block_size - (self.current_byte + self.block_padding_back);
        if bytes < remaining_bytes {
            self.current_byte += bytes;
            return;
        }
        let advance_blocks = (bytes - remaining_bytes) / padded_block + 1;
        let advance_bytes = (bytes - remaining_bytes) % padded_block;
        self.current_block += advance_blocks;
        self.current_byte = self.block_padding_front + advance_bytes;
    }

    pub fn block(&self) -> usize {
        self.current_block as usize
    }

    pub fn byte(&self) -> usize {
        self.current_byte as usize
    }
}

#[cfg(test)]
mod tests {
    use super::BlockCursor;
    use crate::filesystem::Filesystem;
    use std::io::Cursor;

    #[test]
    fn without_overflow() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::new(&fs, (0, 0));
        assert_eq!(cursor.current_block, 0);
        assert_eq!(cursor.current_byte, 0);
        cursor.advance(400);
        assert_eq!(cursor.current_block, 0);
        assert_eq!(cursor.current_byte, 400);
        cursor.advance(111);
        assert_eq!(cursor.current_block, 0);
        assert_eq!(cursor.current_byte, 511);
    }

    #[test]
    fn with_overflow() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::new(&fs, (0, 0));
        cursor.advance(600);
        assert_eq!(cursor.current_block, 1);
        assert_eq!(cursor.current_byte, 88);
        cursor.advance(1000);
        assert_eq!(cursor.current_block, 3);
        assert_eq!(cursor.current_byte, 64);
    }

    #[test]
    fn with_padding() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::from(&fs, 0, 0, (8, 16));
        cursor.advance(500);
        assert_eq!(cursor.current_block, 1);
        assert_eq!(cursor.current_byte, 12);
        cursor.advance(1234);
        assert_eq!(cursor.current_block, 3);
        assert_eq!(cursor.current_byte, 270);
    }
}
