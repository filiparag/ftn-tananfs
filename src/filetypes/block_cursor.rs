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
            current_block: starting_block,
            current_byte: starting_byte as usize,
        }
    }

    pub fn current(&self) -> u64 {
        let padded_block =
            (self.block_size - self.block_padding_front - self.block_padding_back) as u64;
        padded_block * self.current_block + (self.current_byte - self.block_padding_front) as u64
    }

    pub fn advance(&mut self, bytes: u64) -> u64 {
        let padded_block = self.block_size - self.block_padding_front - self.block_padding_back;
        let remaining_bytes = self.block_size - (self.current_byte + self.block_padding_back);
        if bytes < remaining_bytes as u64 {
            self.current_byte += bytes as usize;
            return self.current();
        }
        let advance_blocks = (bytes - remaining_bytes as u64) / (padded_block as u64) + 1;
        let advance_bytes = (bytes - remaining_bytes as u64) % (padded_block as u64);
        self.current_block += advance_blocks;
        self.current_byte = self.block_padding_front + advance_bytes as usize;
        self.current()
    }

    pub fn regress(&mut self, bytes: u64) -> u64 {
        let padded_block = self.block_size - self.block_padding_front - self.block_padding_back;
        let remaining_bytes = self.current_byte - self.block_padding_front;
        if bytes < remaining_bytes as u64 {
            self.current_byte -= bytes as usize;
            return self.current();
        }
        let regress_blocks = (bytes - remaining_bytes as u64) / (padded_block as u64) + 1;
        let regress_bytes = (bytes - remaining_bytes as u64) % (padded_block as u64);
        if regress_blocks <= self.current_block {
            self.current_block -= regress_blocks;
            self.current_byte = self.block_size - self.block_padding_back - regress_bytes as usize;
        } else {
            self.current_block = 0;
            self.current_byte = 0;
        }
        self.current()
    }

    pub fn set(&mut self, bytes: u64) -> u64 {
        self.reset();
        self.advance(bytes)
    }

    pub fn reset(&mut self) {
        self.current_block = 0;
        self.current_byte = self.block_padding_front;
    }

    pub fn block(&self) -> u64 {
        self.current_block
    }

    pub fn byte(&self) -> usize {
        self.current_byte
    }

    pub fn padded_byte(&self) -> usize {
        self.current_byte - self.block_padding_front
    }

    pub fn position(&self) -> u64 {
        if self.current_block == 0 {
            self.padded_byte() as u64
        } else {
            let padded_block =
                (self.block_size - self.block_padding_front - self.block_padding_back) as u64;
            self.current_block * padded_block + self.padded_byte() as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockCursor;
    use crate::filesystem::Filesystem;
    use std::io::Cursor;

    #[test]
    fn advance_without_overflow() {
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
    fn regress_without_overflow() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::from(&fs, 10, 200, (0, 0));
        assert_eq!(cursor.current_block, 10);
        assert_eq!(cursor.current_byte, 200);
        cursor.regress(100);
        assert_eq!(cursor.current_block, 10);
        assert_eq!(cursor.current_byte, 100);
        cursor.regress(55);
        assert_eq!(cursor.current_block, 10);
        assert_eq!(cursor.current_byte, 45);
    }

    #[test]
    fn advance_with_overflow() {
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
    fn regress_with_overflow() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::from(&fs, 10, 200, (0, 0));
        assert_eq!(cursor.current_block, 10);
        assert_eq!(cursor.current_byte, 200);
        cursor.regress(400);
        assert_eq!(cursor.current_block, 9);
        assert_eq!(cursor.current_byte, 312);
        cursor.regress(111);
        assert_eq!(cursor.current_block, 9);
        assert_eq!(cursor.current_byte, 201);
        cursor.regress(512);
        assert_eq!(cursor.current_block, 8);
        assert_eq!(cursor.current_byte, 201);
        cursor.regress(4000);
        assert_eq!(cursor.current_block, 0);
        assert_eq!(cursor.current_byte, 297);
        cursor.regress(1000);
        assert_eq!(cursor.current_block, 0);
        assert_eq!(cursor.current_byte, 0);
    }

    #[test]
    fn advance_with_padding() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::from(&fs, 0, 0, (8, 16));
        cursor.advance(500);
        assert_eq!(cursor.current_block, 1);
        assert_eq!(cursor.current_byte, 12);
        cursor.advance(1234);
        assert_eq!(cursor.current_block, 3);
        assert_eq!(cursor.current_byte, 270);
        cursor.advance(225);
        assert_eq!(cursor.current_block, 3);
        assert_eq!(cursor.current_byte, 495);
        cursor.advance(2);
        assert_eq!(cursor.current_block, 4);
        assert_eq!(cursor.current_byte, 9);
    }

    #[test]
    fn regress_with_padding() {
        let dev = Cursor::new(vec![0u8; 10_000_000]);
        let fs = Filesystem::new(Box::new(dev), 10_000_000, 512);
        let mut cursor = BlockCursor::from(&fs, 10, 200, (8, 16));
        assert_eq!(cursor.current_block, 10);
        assert_eq!(cursor.current_byte, 200);
        cursor.regress(400);
        assert_eq!(cursor.current_block, 9);
        assert_eq!(cursor.current_byte, 288);
        cursor.regress(111);
        assert_eq!(cursor.current_block, 9);
        assert_eq!(cursor.current_byte, 177);
        cursor.regress(488);
        assert_eq!(cursor.current_block, 8);
        assert_eq!(cursor.current_byte, 177);
    }
}
