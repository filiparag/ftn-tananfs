use super::{BYTES_IN_U16, BYTES_IN_U64};

pub fn u64_from_bytes(bytes: &[u8]) -> u64 {
    let mut raw = [0; BYTES_IN_U64];
    raw.copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

pub fn u16_from_bytes(bytes: &[u8]) -> u16 {
    let mut raw = [0; BYTES_IN_U16];
    raw.copy_from_slice(bytes);
    u16::from_le_bytes(raw)
}
