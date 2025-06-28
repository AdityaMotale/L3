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
mod tests {
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
