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

        // Linux-specific optimization only for large files
        #[cfg(target_os = "linux")]
        if file_size > LARGE_FILE_THRESHOLD {
            Self::advise_sequential(&file).ok();
        }

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
        let mut j: usize = 0;

        for (i, ch) in buf.iter().enumerate() {
            if self.lookup[*ch as usize] == 1 {
                self.tokens.push(buf[j..i].to_vec());

                // we do not need to include the ith byte, it'll be a delimeter
                j = i + 1;
            }
        }
    }

    fn build_lookup() -> [u8; 256] {
        let mut t = [0u8; 256];

        for &b in b" \t\r\n.,;:()[]{}<>\"'`" {
            t[b as usize] = 1;
        }

        t
    }

    #[cfg(target_os = "linux")]
    fn advise_sequential(file: &File) -> std::io::Result<()> {
        let fd = file.as_raw_fd();
        let res = unsafe { posix_fadvise(fd, 0, 0, POSIX_FADV_SEQUENTIAL) };
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_large_file() {
        let mut y3 = Y3::new("dict.txt");

        let n = y3.tokenize().unwrap();

        assert_eq!(n, 3864798);
        assert_eq!(y3.tokens.len(), 370287);
    }

    #[test]
    fn test_small_file() {
        let mut y3 = Y3::new("asm.txt");

        let n = y3.tokenize().unwrap();

        assert_eq!(n, 13703);
        assert_eq!(y3.tokens.len(), 3492);
    }
}
