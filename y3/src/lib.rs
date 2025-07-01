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
    const EMAIL_CHAR: u8 = 0b010000; // '@' and '.'
    const URL_CHAR: u8 = 0b100000; // ':' and '/'

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
            self.process_chunks(&buffer[..bytes_read]);
            total_bytes += bytes_read;
        }

        Ok(total_bytes)
    }

    fn process_chunks(&mut self, buf: &[u8]) {
        let mut start = 0;
        let mut in_token = false;
        let mut saw_lower = false;
        let mut last_cls = 0;
        let mut in_email = false;
        let mut in_url = false;

        for (i, &ch) in buf.iter().enumerate() {
            let cls = self.lookup[ch as usize];

            // Check for URL pattern (protocol://)
            if !in_url
                && !in_email
                && ch == b':'
                && i + 2 < buf.len()
                && buf[i + 1] == b'/'
                && buf[i + 2] == b'/'
            {
                // Look back to see if we have a protocol
                if self.looks_like_url_protocol(&buf[..i]) {
                    in_url = true;
                    in_token = false;
                    saw_lower = false;
                    continue;
                }
            }

            // Skip tokenization if we're in a URL
            if in_url {
                // End URL on whitespace or other delimiters
                if cls & Self::DELIM != 0
                    || (ch != b'/'
                        && ch != b':'
                        && ch != b'.'
                        && ch != b'-'
                        && ch != b'_'
                        && ch != b'?'
                        && ch != b'&'
                        && ch != b'='
                        && ch != b'#'
                        && ch != b'%'
                        && cls & (Self::LOWER | Self::UPPER | Self::DIGIT) == 0)
                {
                    in_url = false;
                    continue;
                }
                continue;
            }

            // Check for email pattern
            if ch == b'@' && in_token && !in_url {
                // Look ahead for domain pattern
                if self.looks_like_email_domain(&buf[i..]) {
                    in_email = true;
                    continue;
                }
            }

            // Skip tokenization if we're in an email
            if in_email {
                // End email on whitespace or end of valid email chars
                if cls & (Self::LOWER | Self::UPPER | Self::DIGIT | Self::EMAIL_CHAR) == 0
                    && ch != b'-'
                    && ch != b'_'
                {
                    in_token = false;
                    in_email = false;
                    saw_lower = false;
                }
                continue;
            }

            // 1. Non-alpha-numeric → always ends token
            if cls & (Self::LOWER | Self::UPPER | Self::DIGIT) == 0 {
                if in_token && saw_lower {
                    self.tokens.push(buf[start..i].to_vec());
                }
                in_token = false;
                saw_lower = false;
                continue;
            }

            // 2. Digit → ends current token only if not followed by a lowercase letter
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
        if in_token && saw_lower && !in_email && !in_url {
            self.tokens.push(buf[start..].to_vec());
        }
    }

    #[inline]
    fn looks_like_url_protocol(&self, buf: &[u8]) -> bool {
        // Look backwards from current position to find start of potential protocol
        let mut start = buf.len();

        // Find the start of the current word (alphanumeric sequence)
        while start > 0 {
            let ch = buf[start - 1];
            let cls = self.lookup[ch as usize];
            if cls & (Self::LOWER | Self::UPPER | Self::DIGIT) == 0 {
                break;
            }
            start -= 1;
        }

        if start >= buf.len() {
            return false;
        }

        let protocol = &buf[start..];

        // Check for common protocols
        matches!(
            protocol,
            b"http" | b"https" | b"ftp" | b"ftps" | b"file" | b"mailto" | b"ssh" | b"git"
        )
    }

    #[inline]
    fn looks_like_email_domain(&self, buf: &[u8]) -> bool {
        // Simple heuristic: @ followed by alphanumeric chars, then a dot, then more alphanumeric
        let mut i = 1; // Skip the '@'
        let mut found_dot = false;
        let mut chars_after_dot = 0;

        while i < buf.len() && i < 50 {
            // Reasonable email length limit
            let ch = buf[i];
            let cls = self.lookup[ch as usize];

            if ch == b'.' {
                if found_dot || i == 1 {
                    // Multiple dots or dot right after @
                    return false;
                }
                found_dot = true;
                chars_after_dot = 0;
            } else if cls & (Self::LOWER | Self::UPPER | Self::DIGIT) != 0 || ch == b'-' {
                if found_dot {
                    chars_after_dot += 1;
                }
            } else {
                // End of potential email
                break;
            }
            i += 1;
        }

        found_dot && chars_after_dot >= 2 // At least 2 chars after dot (like ".com")
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

        // email chars
        for &b in &[b'@', b'.'] {
            t[b as usize] |= Self::EMAIL_CHAR;
        }

        // url chars
        for &b in &[b':', b'/'] {
            t[b as usize] |= Self::URL_CHAR;
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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_tiny_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"# Contact: Onno Hommes EMAIL <ohommes@cmu.edu>.")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
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
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(
                b"camelCase PascalCase snake_case SCREAMING_SNAKE_CASE Camel_Snake_Case kebab-case UPPERCASE lowercase",
            )
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
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
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"_private __private_var maxSize methodName_expectedResult() NullUser.getName() IEnumerable {\"user_name\": \"Alice\"} $temp1 EMPLOYEE-RECORD")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
        let n = y3.tokenize().unwrap();

        let expected_tokens = [
            "private",
            "private",
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
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"#2 #A 123 IIab I2ab Iab IName StateI fileIO car5 ab5y CHATGpt myParser5X")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
        let n = y3.tokenize().unwrap();

        let expected_tokens = [
            "Iab", "I2ab", "Iab", "Name", "State", "file", "car", "ab5y", "Gpt", "my", "Parser",
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
    fn test_email_detection() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"Contact john@example.com or name@domain.org for help with parseEmail function")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
        let n = y3.tokenize().unwrap();

        let expected_tokens = [
            "Contact", "or", "for", "help", "with", "parse", "Email", "function",
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
    fn test_url_detection() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"Visit https://example.com or http://test.org for more info about parseUrl function")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
        let n = y3.tokenize().unwrap();

        let expected_tokens = [
            "Visit", "or", "for", "more", "info", "about", "parse", "Url", "function",
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
    fn test_mixed_urls_and_emails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write(b"Contact support@company.com or visit https://company.com/help for urlHelper function")
            .unwrap();

        let path = temp_file.path().to_path_buf();
        let mut y3 = Y3::new(&path.to_str().unwrap());
        let n = y3.tokenize().unwrap();

        let expected_tokens = ["Contact", "or", "visit", "for", "url", "Helper", "function"];

        assert_ne!(n, 0);
        assert_ne!(y3.tokens.len(), 0);
        assert_eq!(expected_tokens.len(), y3.tokens.len());

        for (i, t) in y3.tokens.iter().enumerate() {
            let token = String::from_utf8(t.clone()).unwrap();
            assert_eq!(&token, expected_tokens[i]);
        }
    }
}
