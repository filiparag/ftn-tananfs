use bytemuck::Pod;
use std::{fmt::Display, io::SeekFrom};

use super::*;
use crate::{filesystem::Filesystem, Error};

const LENGTH_AS_BYTES: usize = 2;
const COUNT_AS_BYTES: usize = 4;

impl AsBitmap for Block {}

impl Block {
    pub fn new(fs: &mut Filesystem) -> Result<Self, Error> {
        let index = fs.acquire_block()?;
        Ok(Self {
            index,
            data: vec![0; fs.superblock.block_size as usize],
        })
    }

    /// Serialize any data to bytes and return ones exceeding Block's capacity
    pub fn write_any<T: Pod>(&mut self, position: usize, data: T) -> Result<Vec<u8>, Error> {
        let data_raw = bytemuck::bytes_of(&data);
        if position + data_raw.len() < self.data.len() {
            self.data[position..position + data_raw.len()].copy_from_slice(data_raw);
            Ok(vec![])
        } else {
            let end = position + data_raw.len() - self.data.len();
            self.data[position..].copy_from_slice(&data_raw[..end]);
            Ok(data_raw[end..].to_vec())
        }
    }

    /// Write bytes to bytes and return ones exceeding Block's capacity
    pub fn write_bytes<'a>(&mut self, position: usize, data: &'a [u8]) -> Result<&'a [u8], Error> {
        if position + data.len() < self.data.len() {
            self.data[position..position + data.len()].copy_from_slice(data);
            Ok(&[])
        } else {
            let end = position + data.len() - self.data.len();
            self.data[position..].copy_from_slice(&data[..end]);
            Ok(&data[end..])
        }
    }
}

impl PermanentIndexed for Block {
    type Error = crate::Error;

    fn load<D: Read + Seek>(
        block_device: &mut D,
        superblock: &Superblock,
        index: u64,
    ) -> Result<Self, Self::Error> {
        let position = superblock.block_position(index)?;
        block_device.seek(SeekFrom::Start(position))?;
        let mut block_raw = vec![0u8; superblock.block_size as usize];
        block_device.read_exact(&mut block_raw)?;
        Ok(Self {
            data: block_raw,
            index,
        })
    }

    fn flush<D: Write + Seek>(
        &self,
        block_device: &mut D,
        superblock: &Superblock,
    ) -> Result<(), Self::Error> {
        let position = superblock.block_position(self.index)?;
        block_device.seek(SeekFrom::Start(position))?;
        block_device.write_all(&self.data)?;
        Ok(())
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.data == other.data
    }
}

impl Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Block {{")?;
        writeln!(f, "    index: {}", { self.index })?;
        writeln!(f, "    data: [")?;
        write!(f, "         ")?;
        for column in 0..16 {
            write!(f, "{column:3}")?;
        }
        writeln!(f)?;
        for (index, byte) in self.data.iter().enumerate() {
            if index % 16 == 0 {
                write!(f, "     {index:3}:")?;
            }
            if byte.is_ascii_alphanumeric() {
                write!(f, "  {}", *byte as char)?;
            } else if *byte == 0 {
                write!(f, "  ·")?;
            } else {
                write!(f, " {byte:02X}")?;
            }

            if index % 16 == 15 {
                writeln!(f)?;
            }
        }
        writeln!(f, "    ]")?;
        write!(f, "}}")?;
        Ok(())
    }
}
