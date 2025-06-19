use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::{Seek, Write},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
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

    pub fn set(&self, ph: PartedHash, key: &[u8], val: &[u8]) -> Result<bool> {
        let row = self.header_row(ph.row());

        for (i, s) in row.signs.iter().enumerate() {
            if *s == ph.sign() {
                let desc = row.descriptors[i];
                let (k, _) = self.read(desc)?;

                if k == key {
                    row.descriptors[i] = self.write(key, val)?;
                    return Ok(true);
                }
            }
        }

        for (i, s) in row.signs.iter_mut().enumerate() {
            if *s == PartedHash::INVALID_SIGN {
                *s = ph.sign();
                row.descriptors[i] = self.write(key, val)?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn remove(&mut self, ph: PartedHash, key: &[u8]) -> Result<bool> {
        let row = self.header_row(ph.row());

        for (i, s) in row.signs.iter_mut().enumerate() {
            if *s == ph.sign() {
                let desc = row.descriptors[i];
                let (k, _) = self.read(desc)?;

                if k == key {
                    *s = PartedHash::INVALID_SIGN;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = Result<KV>> + 'a {
        (0..ROWS).map(|r| self.header_row(r)).flat_map(|row| {
            row.signs.iter().enumerate().filter_map(|(i, sig)| {
                if *sig == PartedHash::INVALID_SIGN {
                    return None;
                }
                Some(self.read(row.descriptors[i]))
            })
        })
    }
}

pub struct Store {
    dirpath: PathBuf,
    shards: Vec<ShardFile>,
}

impl Store {
    const MAX_SHARD: u32 = u16::MAX as u32 + 1;

    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dirpath = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dirpath)?;
        let first_shard = ShardFile::open(&dirpath, 0, Self::MAX_SHARD)?;

        Ok(Store {
            dirpath,
            shards: vec![first_shard],
        })
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Buf>> {
        let ph = PartedHash::new(key);

        for shard in self.shards.iter() {
            if ph.shard() < shard.end {
                return shard.get(ph, key);
            }
        }

        unreachable!()
    }

    pub fn remove(&mut self, key: &[u8]) -> Result<bool> {
        let ph = PartedHash::new(key);

        for shard in self.shards.iter_mut() {
            if ph.shard() < shard.end {
                return shard.remove(ph, key);
            }
        }

        unreachable!()
    }

    pub fn split(&mut self, shard_idx: usize) -> Result<()> {
        let removed_shard = self.shards.remove(shard_idx);

        let start = removed_shard.start;
        let end = removed_shard.end;
        let mid = (start + end) / 2;
        println!("splitting [{start}, {end}) to [{start}, {mid}) and [{mid}, {end})");

        let top = ShardFile::open(&self.dirpath, start, mid)?;
        let bottom = ShardFile::open(&self.dirpath, mid, end)?;

        for res in removed_shard.iter() {
            let (key, val) = res?;
            let ph = PartedHash::new(&key);

            if ph.shard() < mid {
                bottom.set(ph, &key, &val)?;
            } else {
                top.set(ph, &key, &val)?;
            }
        }

        std::fs::remove_file(self.dirpath.join(format!("{start}-{end}")))?;

        self.shards.push(bottom);
        self.shards.push(top);
        self.shards.sort_by(|x, y| x.end.cmp(&y.end));

        Ok(())
    }

    pub fn set(&mut self, key: &[u8], val: &[u8]) -> Result<bool> {
        let ph = PartedHash::new(key);

        loop {
            let mut shard_to_split = None;

            for (i, shard) in self.shards.iter_mut().enumerate() {
                if ph.shard() < shard.end {
                    if shard.set(ph, key, val)? {
                        return Ok(true);
                    }
                    shard_to_split = Some(i);
                    break;
                }
            }

            self.split(shard_to_split.unwrap())?;
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = Result<KV>> + 'a {
        self.shards.iter().flat_map(|shard| shard.iter())
    }
}
