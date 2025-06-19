use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::{Seek, Write},
    os::unix::fs::FileExt,
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

    pub fn header_row(&self, r: usize) -> &mut ShardRow {
        &mut unsafe { &mut *(self.mmap.as_ptr() as *const ShardHeader as *mut ShardHeader) }.rows[r]
    }

    pub fn read(&self, desc: Descriptor) -> Result<KV> {
        let mut k = vec![0u8; desc.klen as usize];
        let mut v = vec![0u8; desc.vlen as usize];
        let f = self.file.borrow();

        f.read_exact_at(&mut k, desc.offset as u64)?;
        f.read_exact_at(&mut v, desc.offset as u64 + desc.klen as u64)?;

        Ok((k, v))
    }

    pub fn write(&self, k: &[u8], v: &[u8]) -> Result<Descriptor> {
        let mut f = self.file.borrow_mut();
        let offset = f.stream_position()?;

        f.write_all(k)?;
        f.write_all(v)?;

        Ok(Descriptor {
            offset: offset as u32,
            klen: k.len() as u16,
            vlen: v.len() as u16,
        })
    }

    pub fn get(&self, ph: PartedHash, key: &[u8]) -> Result<Option<Buf>> {
        let row = self.header_row(ph.row());

        for (i, s) in row.signs.iter().enumerate() {
            if *s == ph.sign() {
                let desc = row.descriptors[i];
                let (k, v) = self.read(desc)?;

                if k == key {
                    return Ok(Some(v));
                }
            }
        }

        Ok(None)
    }

    pub fn set(&self, ph: PartedHash, k: &[u8], v: &[u8]) -> Result<bool> {
        let row = self.header_row(ph.row());

        Ok(false)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(4, 4);
    }
}
