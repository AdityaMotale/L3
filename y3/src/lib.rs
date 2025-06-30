#![allow(dead_code)]

use std::{
    fs::File,
    io::{BufReader, Read},
};

#[cfg(target_os = "linux")]
use {libc::posix_fadvise, libc::POSIX_FADV_SEQUENTIAL, std::os::unix::io::AsRawFd};

const BUFFER_SIZE: usize = 1024 * 64; // 32 Kib
const SMALL_FILE_THRESHOLD: usize = 16 * 1024; // 16 KiB
const LARGE_FILE_THRESHOLD: usize = 1 * 1024 * 1024; // 1 MiB

pub struct Y3 {
    file: String,
    tokens: Vec<Vec<u8>>,
    lookup: [u8; 256],
}

impl Y3 {
    pub fn new(path: &str) -> Self {
        Self {
            file: path.to_owned(),
            tokens: Vec::new(),
            lookup: Self::build_lookup(),
        }
    }

    pub fn tokenize(&mut self) -> std::io::Result<usize> {
        let metadata = std::fs::metadata(&self.file)?;
        let file_size = metadata.len() as usize;

        // Handle small files efficiently
        if file_size <= SMALL_FILE_THRESHOLD {
            let content = std::fs::read(&self.file)?;
            self.process_chunks(&content);

            return Ok(content.len());
        }

        let file = File::open(&self.file)?;

        // Linux-specific optimization
        #[cfg(target_os = "linux")]
        Self::advise_sequential(&file);

        let mut total_bytes = 0usize;
        let mut buffer = vec![0u8; BUFFER_SIZE];
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);

        loop {
            let bytes_read = reader.read(&mut buffer)?;

            if bytes_read == 0 {
                break;
            }

            self.process_chunks(&buffer);
            total_bytes += bytes_read;
        }

        Ok(total_bytes)
    }

    fn process_chunks(&mut self, buf: &[u8]) {
        let mut i = false;
        let mut j = false;

        let mut t1: Vec<u8> = Vec::new();
        let mut t2: Vec<u8> = Vec::new();

        for &ch in buf.iter() {
            // We got the valid character
            if Self::is_ascii_alpha(ch) {
                if j {
                    t1.extend_from_slice(&t2);

                    t2.clear();
                    j = false;
                }

                t1.push(ch);
                i = true;

                continue;
            }

            if self.lookup[ch as usize] == 1 {
                if i {
                    self.tokens.push(t1.clone());

                    t1.clear();
                    t2.clear();
                }

                i = false;
                j = false;
            }

            // we got invalid char (non alpha chars)
            // only allowed if surrounded by valid chars
            if i {
                t2.push(ch);
                j = true;
            }
        }

        // consider remaining chars
        if i {
            self.tokens.push(t1.clone());
        }
    }

    #[inline]
    fn build_lookup() -> [u8; 256] {
        let mut t = [0u8; 256];

        t[9] = 1; // \t
        t[10] = 1; // \n
        t[13] = 1; // \r
        t[32] = 1; // SPACE

        t
    }

    #[cfg(target_os = "linux")]
    #[inline]
    fn advise_sequential(file: &File) {
        let res = unsafe { posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_SEQUENTIAL) };

        debug_assert_eq!(res, 0, "`posix_fadvise` returned an error");
    }

    #[inline]
    fn is_ascii_alpha(c: u8) -> bool {
        ((c & !0x20).wrapping_sub(b'A')) < 26
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_large_file() {
        let mut y3 = Y3::new("dict.txt");

        let n = y3.tokenize().unwrap();

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
    }

    #[test]
    #[ignore]
    fn test_small_file() {
        let mut y3 = Y3::new("asm.txt");

        let n = y3.tokenize().unwrap();

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
    }

    #[test]
    fn test_tiny_file() {
        let mut y3 = Y3::new("tiny.txt");
        let n = y3.tokenize().unwrap();
        let expected_tokens = ["Contact", "Onno", "Hommes", "ohommes@cmu.edu"];

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);

        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
        }
    }
}
