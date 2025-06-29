#![allow(dead_code)]

use std::{fs::File, io::Read, path::PathBuf};

pub struct Y3 {
    file: PathBuf,
}

impl Y3 {
    pub fn new(path: PathBuf) -> Self {
        Self { file: path }
    }

    pub fn tokenize(&self) -> std::io::Result<usize> {
        let mut file = File::open(&self.file)?;
        let mut buffer = [0u8; 1024];

        let mut totale_bytes = 0usize;

        loop {
            let n = file.read(&mut buffer)?;

            if n == 0 {
                break;
            }

            totale_bytes += n;
        }

        Ok(totale_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_read() {
        let y3 = Y3::new(PathBuf::from("asm.txt"));

        let n = y3.tokenize().unwrap();

        assert_eq!(n, 13695);
    }
}
