#![allow(dead_code)]

use memmap::Mmap;
use std::{fs::File, io, path::PathBuf};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

const BUFFER_SIZE: usize = 1024 * 32; // 16 Kib
const CHUNK_SIZE: usize = 16; // 16 bytes (used for SIMD)

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

    pub fn get_chunk(&mut self) -> Option<[u8; CHUNK_SIZE]> {
        if self.pos == self.len {
            return None;
        }

        let end = (self.pos + CHUNK_SIZE).min(self.len);
        let slice = match &self.src {
            SrcType::InMem(buf) => &buf[self.pos..end],
            SrcType::Mmap(mmap) => &mmap[self.pos..end],
        };

        let mut chunk = [0u8; CHUNK_SIZE];

        unsafe {
            std::ptr::copy_nonoverlapping(slice.as_ptr(), chunk.as_mut_ptr(), slice.len());
        }

        self.pos = end;
        Some(chunk)
    }
}

pub struct Tokenizer;

impl Tokenizer {
    pub fn tokenize(path: &PathBuf) -> io::Result<Vec<u8>> {
        const SPACE: u8 = b' ';
        let mut tokens: Vec<u8> = Vec::with_capacity(BUFFER_SIZE * 2);
        let mut src_reader = SrcReader::new(&path)?;

        while let Some(buf) = src_reader.get_chunk() {
            let mut output = [0u8; 16];

            unsafe {
                Self::replace_delims_16_simple(buf.as_ptr(), output.as_mut_ptr());
            }

            tokens.extend_from_slice(&output);
        }

        Ok(tokens)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn replace_delims_16_simple(input: *const u8, output: *mut u8) {
        let orig = _mm_loadu_si128(input as *const __m128i);

        let v_nl = _mm_cmpeq_epi8(orig, _mm_set1_epi8(b'\n' as i8));
        let v_cr = _mm_cmpeq_epi8(orig, _mm_set1_epi8(b'\r' as i8));
        let v_tab = _mm_cmpeq_epi8(orig, _mm_set1_epi8(b'\t' as i8));
        let v_dash = _mm_cmpeq_epi8(orig, _mm_set1_epi8(b'-' as i8));
        let v_us = _mm_cmpeq_epi8(orig, _mm_set1_epi8(b'_' as i8));
        let mask1 = _mm_or_si128(v_nl, v_cr);
        let mask2 = _mm_or_si128(v_tab, v_dash);
        let mask = _mm_or_si128(_mm_or_si128(mask1, mask2), v_us);

        let space = _mm_set1_epi8(b' ' as i8);
        let result = _mm_blendv_epi8(orig, space, mask);

        _mm_storeu_si128(output as *mut __m128i, result);
    }
}

#[cfg(test)]
mod token_tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_simple_text() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write(b"# Contact Hommes EMAIL").unwrap();

        let expected_tokens = ["#", "Contact", "Hommes", "EMAIL"];
        let path = temp_file.path().to_path_buf();

        let tokens = Tokenizer::tokenize(&path).unwrap();

        let mut token: Vec<u8> = Vec::new();
        let mut idx: usize = 0;

        for &t in tokens.iter() {
            if t != b' ' {
                token.push(t);

                continue;
            }

            let tok = String::from_utf8(token.clone()).unwrap();
            assert_eq!(&tok, expected_tokens[idx]);

            token.clear();
            idx += 1;
        }
    }
}
#[cfg(test)]
mod reader_tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn get_file_size(path: &PathBuf) -> usize {
        let file = File::open(path).unwrap();
        file.metadata().unwrap().len() as usize
    }

    #[test]
    fn test_chunk_size_input() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = b"Contact OH Homme";

        // make sure data is of chunk len
        assert!(
            data.len() == CHUNK_SIZE,
            "input data must be of size CHUNK_SIZE, (INVALID INPUT)",
        );

        temp_file.write_all(data).unwrap();

        let path = temp_file.path().to_path_buf();
        let file_size = get_file_size(&path);

        let mut sr = SrcReader::new(&path).unwrap();
        let buf = sr.get_chunk().expect("expected one chunk for tiny file");

        assert_eq!(buf.len(), CHUNK_SIZE);
        assert_eq!(&buf[..file_size], data);
        assert!(buf[file_size..].iter().all(|&b| b == 0));
        assert!(sr.get_chunk().is_none());
    }

    #[test]
    fn test_large_file_read() {
        let path = PathBuf::from("./ex_files/large.txt");
        let file_size = get_file_size(&path);

        let mut sr = SrcReader::new(&path).unwrap();
        let mut num_chunks = 0;

        while let Some(buf) = sr.get_chunk() {
            assert_eq!(buf.len(), CHUNK_SIZE);
            num_chunks += 1;
        }

        let covered = num_chunks * CHUNK_SIZE;

        assert!(
            covered >= file_size,
            "covered={} should be ≥ file_size={}",
            covered,
            file_size
        );
        assert!(
            covered < file_size + CHUNK_SIZE,
            "covered={} should be < file_size+CHUNK_SIZE={}",
            covered,
            file_size + CHUNK_SIZE
        );
    }
}
