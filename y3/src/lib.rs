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
    const LOWER: u8 = 0b000001;
    const UPPER: u8 = 0b000010;
    const DIGIT: u8 = 0b000100;
    const DELIM: u8 = 0b001000; // whitespaces, '_', '-', etc.

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
        let mut start = 0;
        let mut in_token = false;
        let mut saw_lower = false;
        let mut last_cls = 0;

        for (i, &ch) in buf.iter().enumerate() {
            let cls = self.lookup[ch as usize];

            // 1. Non-alpha-numeric → always ends token
            if cls & (Self::LOWER | Self::UPPER | Self::DIGIT) == 0 {
                if in_token && saw_lower {
                    self.tokens.push(buf[start..i].to_vec());
                }
                in_token = false;
                saw_lower = false;
                continue;
            }

            // 2. Digit → ends current token *only* if not followed by a lowercase letter
            if cls & Self::DIGIT != 0 {
                let next_is_lower = buf
                    .get(i + 1)
                    .map(|&b| self.lookup[b as usize] & Self::LOWER != 0)
                    .unwrap_or(false);

                if !next_is_lower {
                    if in_token && saw_lower {
                        self.tokens.push(buf[start..i].to_vec());
                    }
                    in_token = false;
                    saw_lower = false;
                    continue;
                }
            }

            // 3. Start new token
            if !in_token {
                start = i;
                in_token = true;
                saw_lower = cls & Self::LOWER != 0;
            } else {
                // Uppercase after lowercase → split (e.g., "FileIO" → "File")
                if cls & Self::UPPER != 0 && saw_lower {
                    self.tokens.push(buf[start..i].to_vec());
                    start = i;
                    saw_lower = false;
                }
                // PascalCase boundary: UPPER followed by LOWER (e.g., "IOFile")
                else if last_cls & Self::UPPER != 0 && cls & Self::LOWER != 0 {
                    if saw_lower {
                        self.tokens.push(buf[start..i - 1].to_vec());
                    }
                    start = i - 1;
                    saw_lower = true;
                }
                // mark saw_lower
                else if cls & Self::LOWER != 0 {
                    saw_lower = true;
                }
            }

            last_cls = cls;
        }

        // Final flush
        if in_token && saw_lower {
            self.tokens.push(buf[start..].to_vec());
        }
    }

    #[inline]
    fn build_lookup() -> [u8; 256] {
        let mut t = [0u8; 256];

        // small letters
        for b in b'a'..=b'z' {
            t[b as usize] |= Self::LOWER;
        }

        // big letters
        for b in b'A'..=b'Z' {
            t[b as usize] |= Self::UPPER;
        }

        // digits
        for b in b'0'..=b'9' {
            t[b as usize] |= Self::DIGIT;
        }

        // delimiters
        for &b in &[b' ', b'\n', b'\r', b'\t', b'-', b'_'] {
            t[b as usize] |= Self::DELIM;
        }

        t
    }

    #[cfg(target_os = "linux")]
    #[inline]
    fn advise_sequential(file: &File) {
        let res = unsafe { posix_fadvise(file.as_raw_fd(), 0, 0, POSIX_FADV_SEQUENTIAL) };

        debug_assert_eq!(res, 0, "`posix_fadvise` returned an error");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_large_file() {
        let mut y3 = Y3::new("./ex_files/large.txt");

        let n = y3.tokenize().unwrap();

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
    }

    #[test]
    #[ignore]
    fn test_small_file() {
        let mut y3 = Y3::new("./ex_files/small.txt");

        let n = y3.tokenize().unwrap();

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
    }

    #[test]
    fn test_tiny_file() {
        let mut y3 = Y3::new("./ex_files/tiny.txt");
        let n = y3.tokenize().unwrap();
        let expected_tokens = ["Contact", "Onno", "Hommes", "ohommes", "cmu", "edu"];

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
        }
    }

    #[test]
    fn test_various_cases() {
        let mut y3 = Y3::new("./ex_files/cases.txt");
        let n = y3.tokenize().unwrap();
        let expected_tokens = [
            "camel",
            "Case",
            "Pascal",
            "Case",
            "snake",
            "case",
            "Camel",
            "Snake",
            "Case",
            "kebab",
            "case",
            "lowercase",
        ];

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);

        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
        }
    }

    #[test]
    fn test_code_format_cases() {
        let mut y3 = Y3::new("./ex_files/exp_cases.txt");
        let n = y3.tokenize().unwrap();
        let expected_tokens = [
            "private",
            "priave",
            "var",
            "max",
            "Size",
            "method",
            "Name",
            "expected",
            "Result",
            "Null",
            "User",
            "get",
            "Name",
            "Enumerable",
            "user",
            "name",
            "Alice",
            "temp",
        ];

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
        }
    }

    #[test]
    fn test_random_cases() {
        let mut y3 = Y3::new("./ex_files/rand_cases.txt");
        let n = y3.tokenize().unwrap();
        let expected_tokens = [
            "Iab", "I2ab", "Iab", "Name", "State", "file", "car", "ab5y", "Gpt", "my", "Parser",
        ];

        for t in y3.tokens.iter() {
            let token = String::from_utf8(t.clone()).unwrap();

            println!("{token}");
        }

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();

            assert_eq!(&token, expected_tokens[i]);
        }
    }
}
