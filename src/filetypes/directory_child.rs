use crate::{
    filetypes::{helpers::*, BYTES_IN_U16, BYTES_IN_U64},
    Error,
};

use super::{DirectoryChild, RawByteFile};

impl DirectoryChild {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < BYTES_IN_U64 + BYTES_IN_U16 {
            return Err(Error::InsufficientBytes);
        }
        let mut inode = [0; BYTES_IN_U64];
        let mut name_length = [0; BYTES_IN_U16];
        inode.copy_from_slice(&bytes[0..BYTES_IN_U64]);
        name_length.copy_from_slice(&bytes[BYTES_IN_U64..BYTES_IN_U64 + BYTES_IN_U16]);
        let inode = u64::from_be_bytes(inode);
        let name_length = u16::from_be_bytes(name_length) as usize;
        if bytes.len() < BYTES_IN_U64 + BYTES_IN_U16 + name_length {
            return Err(Error::InsufficientBytes);
        }
        let name = std::str::from_utf8(
            &bytes[BYTES_IN_U64 + BYTES_IN_U16..BYTES_IN_U64 + BYTES_IN_U16 + name_length],
        )?
        .to_owned();
        Ok(Self { inode, name })
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![0; BYTES_IN_U64 + BYTES_IN_U16 + self.name.len()];
        bytes[0..BYTES_IN_U64].copy_from_slice(&self.inode.to_be_bytes());
        bytes[BYTES_IN_U64..BYTES_IN_U64 + BYTES_IN_U16]
            .copy_from_slice(&(self.name.len() as u16).to_be_bytes());
        bytes[BYTES_IN_U64 + BYTES_IN_U16..].copy_from_slice(self.name.as_bytes());
        bytes
    }

    pub fn read(file: &mut RawByteFile) -> Result<Self, Error> {
        let inode = read_u64(file)?;
        let name = read_sized_string(file)?;
        Ok(Self { inode, name })
    }

    pub fn flush(&self, file: &mut RawByteFile) -> Result<(), Error> {
        file.write(&self.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::DirectoryChild;

    #[test]
    fn byte_conversion() {
        let dc = DirectoryChild {
            inode: 420,
            name: "foobar.exe".into(),
        };
        let bytes = dc.as_bytes();
        let dc1 = DirectoryChild::from_bytes(&bytes).unwrap();
        assert_eq!(dc1.inode, dc.inode);
        assert_eq!(dc1.name, dc.name);
    }
}
