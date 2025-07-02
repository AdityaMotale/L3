#![allow(dead_code)]

use memmap::Mmap;
use std::{fs::File, io, path::PathBuf};

const BUFFER_SIZE: usize = 1024 * 32; // 16 Kib

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
    pub fn new(path: &PathBuf) -> io::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let file_size = metadata.len() as usize;

        if file_size <= BUFFER_SIZE {
            let buf = std::fs::read(path)?;

            Ok(Self {
                src: SrcType::InMem(buf),
                len: file_size,
                pos: 0,
            })
        } else {
            let file = File::open(&path)?;
            let mmap = unsafe { Mmap::map(&file)? };

            Ok(Self {
                src: SrcType::Mmap(mmap),
                len: file_size,
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

pub struct Tokenizer {
    lookup: [u8; 256],
}

impl Tokenizer {
    const DELIM: u8 = 0b001000; // whitespaces, '_', '-', etc.

    pub fn new() -> Self {
        Self {
            lookup: Self::build_lookup(),
        }
    }

    pub fn tokenize(&self, path: &PathBuf) -> io::Result<Vec<Vec<u8>>> {
        let mut tokens: Vec<Vec<u8>> = Vec::with_capacity(128);
        let mut src_reader = SrcReader::new(&path)?;

        let mut token: Vec<u8> = vec![0u8; 128];
        let mut i: usize = 0;

        while let Some(buf) = src_reader.get_chunk() {
            for &ch in buf {
                if self.lookup[ch as usize] & Self::DELIM != 0 {
                    tokens.push(token[0..i].to_vec());

                    i = 0;
                    continue;
                }

                token[i] = ch;
                i += 1;
            }

            if i != 0 {
                tokens.push(token[0..i].to_vec());

                i = 0;
            }
        }

        Ok(tokens)
    }

    #[inline]
    fn build_lookup() -> [u8; 256] {
        let mut t = [0u8; 256];

        // delimiters
        for &b in &[b' ', b'\n', b'\r', b'\t', b'-', b'_'] {
            t[b as usize] |= Self::DELIM;
        }

        t
    }
}

#[cfg(test)]
mod token_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_tiny_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write(b"# Contact Hommes EMAIL").unwrap();

        let expected_tokens = ["#", "Contact", "Hommes", "EMAIL"];
        let path = temp_file.path().to_path_buf();
        let tokenizer = Tokenizer::new();

        let tokens = tokenizer.tokenize(&path).unwrap();

        assert_eq!(tokens.len(), expected_tokens.len());

        for (i, t) in tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
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

        let mut sr = SrcReader::new(&path).unwrap();

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

        let mut sr = SrcReader::new(&path).unwrap();

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
