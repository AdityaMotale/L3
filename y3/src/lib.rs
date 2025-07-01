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
    const INTERNAL: u8 = 0b001000; // e.g. '-', '\', '@', '.', '_', '+'
    const DELIM: u8 = 0b010000; // whitespace

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
        enum State {
            Outside,
            InWord { has_lower: bool },
        }

        let mut state = State::Outside;
        let mut token: Vec<u8> = Vec::new();

        for &ch in buf {
            let cls = self.lookup[ch as usize];

            match state {
                // start a new token
                State::Outside => {
                    if cls & (Self::LOWER | Self::UPPER) != 0 {
                        token.clear();
                        token.push(ch);

                        let has_lower = cls & Self::LOWER != 0;

                        state = State::InWord { has_lower };
                    }
                }
                State::InWord { mut has_lower } => {
                    if cls & (Self::LOWER | Self::UPPER | Self::DIGIT | Self::INTERNAL) != 0 {
                        token.push(ch);

                        if cls & Self::LOWER != 0 {
                            has_lower = true;
                        }

                        state = State::InWord { has_lower };
                    } else {
                        if has_lower {
                            self.tokens.push(token);
                            token = Vec::new();
                        }

                        state = State::Outside;
                    }
                }
            }
        }

        // flush on EOF
        if let State::InWord { has_lower } = state {
            if has_lower {
                self.tokens.push(token);
            }
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

        // internal punctuation: allow common email/path chars
        for &b in &[b'-', b'\'', b'@', b'.', b'_', b'+'] {
            t[b as usize] |= Self::INTERNAL;
        }

        // delimiters
        for &b in &[b' ', b'\n', b'\r', b'\t'] {
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
        let expected_tokens = ["Contact", "Onno", "Hommes"];

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
}
