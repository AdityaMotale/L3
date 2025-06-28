#![allow(dead_code)]

const PRIME32_1: u32 = 0x9E3779B1;
const PRIME32_2: u32 = 0x85EBCA77;
const PRIME32_3: u32 = 0xC2B2AE3D;
const PRIME32_4: u32 = 0x27D4EB2F;
const PRIME32_5: u32 = 0x165667B1;

type Lane = u32;
type Lanes = [Lane; 4];
type Bytes = [u8; 16];

// compile time assertion to verify alignment
const _: () = assert!(std::mem::size_of::<u8>() <= std::mem::size_of::<u32>());

const BYTES_IN_LINE: usize = std::mem::size_of::<Bytes>();

#[derive(Clone, PartialEq, Eq)]
struct BufferedData(Lanes);

impl BufferedData {
    const fn new() -> Self {
        Self([0; 4])
    }

    const fn bytes(&self) -> &Bytes {
        unsafe { &*self.0.as_ptr().cast() }
    }

    fn bytes_mut(&mut self) -> &mut Bytes {
        unsafe { &mut *self.0.as_mut_ptr().cast() }
    }
}

impl std::fmt::Debug for BufferedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0).finish()
    }
}

#[cfg(test)]
mod buffer_data_tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_buffered_data_size_and_alignment() {
        assert_eq!(mem::size_of::<BufferedData>(), mem::size_of::<Lanes>());
        assert_eq!(mem::size_of::<Bytes>(), 16);
        assert!(mem::align_of::<u8>() <= mem::align_of::<u32>());
    }

    #[test]
    fn test_debug_format() {
        let mut buf = BufferedData::new();
        buf.0 = [1, 2, 3, 4];
        let debug_str = format!("{:?}", buf);

        assert_eq!(debug_str, "[1, 2, 3, 4]");
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Buffer {
    offset: usize,
    data: BufferedData,
}

impl Buffer {
    #[inline]
    const fn new() -> Self {
        Self {
            offset: 0,
            data: BufferedData::new(),
        }
    }

    fn extend<'a>(&mut self, data: &'a [u8]) -> (Option<&Lanes>, &'a [u8]) {
        if self.offset == 0 {
            return (None, data);
        }

        let bytes = self.data.bytes_mut();
        debug_assert!(self.offset <= bytes.len());

        let empty = &mut bytes[self.offset..];
        let n_to_copy = usize::min(empty.len(), data.len());

        let dst = &mut empty[..n_to_copy];

        let (src, rest) = data.split_at(n_to_copy);

        dst.copy_from_slice(src);
        self.offset += n_to_copy;

        debug_assert!(self.offset <= bytes.len());

        if self.offset == bytes.len() {
            self.offset = 0;

            return (Some(&self.data.0), rest);
        }

        (None, rest)
    }

    #[inline]
    fn set(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        debug_assert_eq!(self.offset, 0);

        let n_to_copy = data.len();

        let bytes = self.data.bytes_mut();
        debug_assert!(n_to_copy < bytes.len());

        bytes[..n_to_copy].copy_from_slice(data);
        self.offset = n_to_copy;
    }

    #[inline]
    fn remaining(&self) -> &[u8] {
        &self.data.bytes()[..self.offset]
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn test_set_and_remaining() {
        let mut buf = Buffer::new();
        let input = &[10, 20, 30];

        buf.set(input);

        assert_eq!(buf.offset, input.len());
        assert_eq!(buf.remaining(), input);
    }

    #[test]
    fn test_set_empty_does_nothing() {
        let mut buf = Buffer::new();
        buf.set(&[]);
        assert_eq!(buf.offset, 0);
        assert_eq!(buf.remaining(), &[]);
    }

    #[test]
    fn test_extend_with_offset_zero_returns_all() {
        let mut buf = Buffer::new();
        let data = &[1, 2, 3, 4];
        let (opt, rest) = buf.extend(data);

        assert!(opt.is_none());
        assert_eq!(rest, data);
        assert_eq!(buf.offset, 0);
    }

    #[test]
    fn test_extend_filling_buffer_and_emitting() {
        let mut buf = Buffer::new();

        // Pre-set offset to simulate partial fill
        buf.set(&[100; 8]);
        let data = &[1u8; 16];

        // should fill remaining 8 bytes, then emit full lane and rest
        let (opt, rest) = buf.extend(data);
        assert!(opt.is_some());

        let lanes = opt.unwrap();

        assert_eq!(lanes[0], u32::from_le_bytes([100; 4]));
        assert_eq!(lanes[1], u32::from_le_bytes([100; 4]));
        assert_eq!(lanes[2], u32::from_le_bytes([1; 4]));
        assert_eq!(lanes[3], u32::from_le_bytes([1; 4]));
        assert_eq!(rest.len(), 8);
        assert_eq!(buf.offset, 0);
    }

    #[test]
    fn test_extend_partial_fill_no_emit() {
        let mut buf = Buffer::new();
        let data = &[2u8; 2];

        buf.set(&[50; 10]);
        let (opt, rest) = buf.extend(data);

        assert!(opt.is_none());
        assert_eq!(rest.len(), 0);
        assert_eq!(buf.offset, 12);
        assert_eq!(buf.remaining().len(), 12);
    }
}
