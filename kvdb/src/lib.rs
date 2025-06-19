use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::Seek,
    path::Path,
};

use memmap::{MmapMut, MmapOptions};
use siphasher::sip::SipHasher24;

pub type Result<T> = std::io::Result<T>;
pub type Buf = Vec<u8>;
pub type KV = (Buf, Buf);

const WIDTH: usize = 512;
const ROWS: usize = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PartedHash(u64);

impl PartedHash {
    const INVALID_SIGN: u32 = 0;

    pub fn new(buf: &[u8]) -> Self {
        PartedHash(SipHasher24::new().hash(buf))
    }

    pub fn sign(&self) -> u32 {
        if self.0 as u32 == Self::INVALID_SIGN {
            0x12345678
        } else {
            self.0 as u32
        }
    }

    pub fn row(&self) -> usize {
        (self.0 as usize >> 32) % ROWS
    }

    pub fn shard(&self) -> u32 {
        (self.0 >> 48) as u32
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Descriptor {
    offset: u32,
    klen: u16,
    vlen: u16,
}

#[repr(C)]
pub struct ShardRow {
    signs: [u32; WIDTH],
    descriptors: [Descriptor; WIDTH],
}

#[repr(C)]
pub struct ShardHeader {
    rows: [ShardRow; ROWS],
}

pub struct ShardFile {
    start: u32,
    end: u32,
    file: RefCell<File>,
    mmap: MmapMut,
}

impl ShardFile {
    const HEADER_SIZE: u64 = size_of::<ShardHeader>() as u64;

    pub fn open(dirpath: impl AsRef<Path>, start: u32, end: u32) -> Result<Self> {
        let filepath = dirpath.as_ref().join(format!("{start}-{end}"));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(filepath)?;

        file.set_len(Self::HEADER_SIZE)?;
        file.seek(std::io::SeekFrom::End(0))?;
        let mmap = unsafe {
            MmapOptions::new()
                .len(Self::HEADER_SIZE as usize)
                .map_mut(&file)
        }?;

        Ok(Self {
            start,
            end,
            file: RefCell::new(file),
            mmap,
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(4, 4);
    }
}
