#![allow(dead_code)]

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

const BUFFER_SIZE: usize = 1024 * 32; // 32 Kib

pub struct Y3 {
    file: String,
}

impl Y3 {
    pub fn new(path: &str) -> Self {
        Self {
            file: path.to_owned(),
        }
    }

    pub fn tokenize(&self) -> std::io::Result<usize> {
        let file = File::open(&self.file)?;
        let mut totale_bytes = 0usize;
        let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);

        loop {
            let buf = reader.fill_buf()?;
            let n = buf.len();

            if n == 0 {
                break;
            }

            totale_bytes += n;
            reader.consume(n);
        }

        Ok(totale_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_read() {
        let y3 = Y3::new("asm.txt");

        let n = y3.tokenize().unwrap();

        assert_eq!(n, 13695);
    }
}
