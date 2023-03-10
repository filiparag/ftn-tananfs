use super::*;

use std::io::SeekFrom;

const LENGTH_AS_BYTES: usize = 2;
const COUNT_AS_BYTES: usize = 4;

impl AsBitmap for Block {}

impl Block {
    // fn read_string(&self, start: usize) -> Result<&str, Error> {
    //     let length = u16::from_le_bytes((&self.data[start..start + LENGTH_AS_BYTES]).try_into()?);
    //     let name_raw =
    //         &self.data[start + LENGTH_AS_BYTES..start + LENGTH_AS_BYTES + length as usize];
    //     Ok(std::str::from_utf8(name_raw)?)
    // }

    // fn write_string(&mut self, start: usize, string: &str) {
    //     let length = &(string.len() as u16).to_le_bytes();
    //     self.data[start..start + LENGTH_AS_BYTES].copy_from_slice(length);
    //     self.data[start + LENGTH_AS_BYTES..start + LENGTH_AS_BYTES + string.len()]
    //         .copy_from_slice(string.as_bytes());
    // }

    // pub fn get_filename(&self) -> Result<&str, Error> {
    //     self.read_string(0)
    // }

    // pub fn get_filenames(&self) -> Result<Vec<&str>, Error> {
    //     let count = u32::from_le_bytes((&self.data[0..COUNT_AS_BYTES]).try_into()?);
    //     let mut names = Vec::<&str>::with_capacity(count as usize);
    //     let mut cursor = 0usize;
    //     for name in names.iter_mut() {
    //         *name = self.read_string(cursor)?;
    //         cursor += name.len() + LENGTH_AS_BYTES
    //     }
    //     Ok(names)
    // }

    // pub fn set_filename(&mut self, name: &str) {
    //     self.write_string(0, name)
    // }

    // pub fn set_filenames(&mut self, names: &[&str]) {
    //     let count = &(names.len() as u32).to_le_bytes();
    //     self.data[0..COUNT_AS_BYTES].copy_from_slice(count);
    //     let mut cursor = COUNT_AS_BYTES;
    //     for &name in names {
    //         self.write_string(cursor, name);
    //         cursor += name.len() + LENGTH_AS_BYTES
    //     }
    // }
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
