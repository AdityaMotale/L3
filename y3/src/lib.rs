#![allow(dead_code)]

use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use memmap::Mmap;

const BUFFER_SIZE: usize = 1024 * 16; // 16 Kib

enum SrcType {
    InMem(Vec<u8>),
    Mmap(Mmap),
}

pub struct SrcReader {
    src: SrcType,
    len: usize,
    pos: usize,
}

impl SrcReader {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let mut file = File::open(&path)?;
        let file_size = file.metadata()?.len() as usize;

        if file_size <= BUFFER_SIZE {
            let mut buf = Vec::with_capacity(file_size);
            let size = file.read_to_end(&mut buf)?;

            Ok(Self {
                src: SrcType::InMem(buf),
                len: size,
                pos: 0,
            })
        } else {
            let mmap = unsafe { Mmap::map(&file)? };

            Ok(Self {
                len: mmap.len(),
                src: SrcType::Mmap(mmap),
                pos: 0,
            })
        }
    }

    pub fn get_chunk(&mut self) -> Option<&[u8]> {
        match &self.src {
            SrcType::InMem(buf) => {
                if self.pos == self.len {
                    return None;
                }

                let end = (self.pos + BUFFER_SIZE).min(self.len);
                let slice = &buf[self.pos..end];

                self.pos = end;

                Some(slice)
            }
            SrcType::Mmap(mmap) => {
                if self.pos == self.len {
                    return None;
                }

                let end = (self.pos + BUFFER_SIZE).min(self.len);
                let slice = &mmap[self.pos..end];

                self.pos = end;

                Some(slice)
            }
        }
    }
}

#[cfg(test)]
mod reader_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn get_file_size(path: &PathBuf) -> usize {
        let file = File::open(path).unwrap();

        file.metadata().unwrap().len() as usize
    }

    #[test]
    fn test_tiny_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"# Contact: Onno Hommes EMAIL <ohommes@cmu.edu>.")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let file_size = get_file_size(&path);

        let mut sr = SrcReader::new(path).unwrap();

        // before reading the chunk
        assert_eq!(sr.pos, 0, "start position should be zero");
        assert_eq!(sr.len, file_size, "reader length should equal file size");

        // read the only chunk
        let buf = sr.get_chunk().expect("expected one chunk from tiny file");

        assert_eq!(
            buf.len(),
            file_size,
            "chunk length should equal entire file size"
        );

        // after reading the chunk
        assert_eq!(sr.pos, file_size, "pos should now equal file size");
        assert!(sr.get_chunk().is_none(), "reader should stay at EOF");
    }

    #[test]
    fn test_large_file() {
        let path = PathBuf::from("./ex_files/large.txt");
        let file_size = get_file_size(&path);

        let mut sr = SrcReader::new(path).unwrap();

        // before reading the chunk
        assert_eq!(sr.pos, 0, "start position should be zero");
        assert_eq!(sr.len, file_size, "reader length should equal file size");

        let mut accumulated = 0;

        while let Some(buf) = sr.get_chunk() {
            assert!(!buf.is_empty(), "chunk should never be empty");
            assert!(
                buf.len() <= BUFFER_SIZE,
                "chunk.len() = {}, but BUFFER_SIZE = {}",
                buf.len(),
                BUFFER_SIZE
            );

            accumulated += buf.len();
        }

        assert_eq!(sr.pos, file_size, "pos should now equal file size");
        assert!(sr.get_chunk().is_none(), "reader should stay at EOF");
        assert_eq!(
            accumulated, file_size,
            "sum of all chunk lengths should equal file size"
        );
    }
}
