use crate::structs::{Block, NULL_BLOCK};

use super::{BYTES_IN_U16, BYTES_IN_U64};

const EMPTY_BYTE_DATA: u8 = 0;

fn u64_from_bytes(bytes: &[u8]) -> u64 {
    let mut raw = [0; BYTES_IN_U64];
    raw.copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

fn u16_from_bytes(bytes: &[u8]) -> u16 {
    let mut raw = [0; BYTES_IN_U16];
    raw.copy_from_slice(bytes);
    u16::from_le_bytes(raw)
}

fn empty_block(size: u32) -> Vec<u8> {
    let mut empty_block = vec![EMPTY_BYTE_DATA; size as usize];
    empty_block[0..BYTES_IN_U64].copy_from_slice(&NULL_BLOCK.to_le_bytes());
    empty_block
}

pub fn empty_block_data(block: &mut Block, start_offset: usize) -> usize {
    let block_size = block.data.len() as u32;
    let data = &empty_block(block_size)[start_offset..];
    block.data[start_offset..].copy_from_slice(data);
    data.len()
}

pub fn bytes_per_block(size: u32) -> u64 {
    size as u64 - BYTES_IN_U64 as u64
}

pub fn set_next_block(block: &mut Block, next: u64) {
    block.data[0..BYTES_IN_U64].copy_from_slice(&next.to_le_bytes());
}

pub fn get_next_block(block: &Block) -> u64 {
    u64_from_bytes(&block.data[0..BYTES_IN_U64])
}
