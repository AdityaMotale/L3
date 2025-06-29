#![allow(dead_code)]

use crate::{IntoU32, IntoU64};
use std::hash::BuildHasher;

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

const BYTES_IN_LANE: usize = std::mem::size_of::<Bytes>();

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

#[derive(Clone, PartialEq)]
struct Accumulator(Lanes);

impl Accumulator {
    #[inline]
    const fn new(seed: u32) -> Self {
        Self([
            seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2),
            seed.wrapping_add(PRIME32_2),
            seed,
            seed.wrapping_sub(PRIME32_1),
        ])
    }

    #[inline]
    fn write(&mut self, lanes: Lanes) {
        let [acc1, acc2, acc3, acc4] = &mut self.0;
        let [l1, l2, l3, l4] = lanes;

        *acc1 = Self::round(*acc1, l1.to_le());
        *acc2 = Self::round(*acc2, l2.to_le());
        *acc3 = Self::round(*acc3, l3.to_le());
        *acc4 = Self::round(*acc4, l4.to_le());
    }

    #[inline]
    fn write_many<'d>(&mut self, mut data: &'d [u8]) -> &'d [u8] {
        while let Some((chunk, rest)) = data.split_first_chunk::<BYTES_IN_LANE>() {
            let lanes = unsafe { chunk.as_ptr().cast::<Lanes>().read_unaligned() };
            self.write(lanes);
            data = rest;
        }

        data
    }

    #[inline]
    const fn finish(&self) -> u32 {
        let [acc1, acc2, acc3, acc4] = self.0;

        let acc1 = acc1.rotate_left(1);
        let acc2 = acc2.rotate_left(7);
        let acc3 = acc3.rotate_left(12);
        let acc4 = acc4.rotate_left(18);

        acc1.wrapping_add(acc2)
            .wrapping_add(acc3)
            .wrapping_add(acc4)
    }

    #[inline]
    const fn round(mut acc: u32, lane: u32) -> u32 {
        acc = acc.wrapping_add(lane.wrapping_mul(PRIME32_2));
        acc = acc.rotate_left(13);
        acc.wrapping_mul(PRIME32_1)
    }
}

impl std::fmt::Debug for Accumulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [acc1, acc2, acc3, acc4] = self.0;

        f.debug_struct("Accumulator")
            .field("acc1", &acc1)
            .field("acc2", &acc2)
            .field("acc3", &acc3)
            .field("acc4", &acc4)
            .finish()
    }
}

#[cfg(test)]
mod accumulator_tests {
    use super::*;

    #[test]
    fn test_accumulator_new() {
        let seed = 42;
        let acc = Accumulator::new(seed);

        assert_eq!(
            acc.0[0],
            seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2)
        );
        assert_eq!(acc.0[1], seed.wrapping_add(PRIME32_2));
        assert_eq!(acc.0[2], seed);
        assert_eq!(acc.0[3], seed.wrapping_sub(PRIME32_1));
    }

    #[test]
    fn test_round_consistency() {
        let acc = Accumulator::round(1, 2);
        let mut exp = 1u32.wrapping_add(2u32.wrapping_mul(PRIME32_2));
        exp = exp.rotate_left(13).wrapping_mul(PRIME32_1);

        assert_eq!(acc, exp);
    }

    #[test]
    fn test_write_and_finish() {
        let mut acc = Accumulator::new(0);
        acc.write([1, 2, 3, 4]);
        let hash = acc.finish();

        assert!(hash <= u32::MAX);
    }

    #[test]
    fn test_write_many_exact_chunks() {
        let mut acc = Accumulator::new(0);
        let mut data = vec![];

        for i in 0..32u8 {
            data.push(i);
        }

        let rest = acc.write_many(&data);

        assert!(rest.is_empty());
    }

    #[test]
    fn test_write_many_with_remainder() {
        let mut acc = Accumulator::new(0);
        let mut data = vec![];

        for i in 0..(BYTES_IN_LANE as u8 + 3) {
            data.push(i);
        }

        let rest = acc.write_many(&data);

        assert_eq!(rest.len(), 3);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hasher {
    seed: u32,
    length: u64,
    accumulator: Accumulator,
    buffer: Buffer,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::with_seed(0)
    }
}

impl Hasher {
    /// Hash all data at once and get a 32-bit hash value
    #[must_use]
    #[inline]
    pub fn oneshot(seed: u32, data: &[u8]) -> u32 {
        let len = data.len().into_u64();

        let mut accumulator = Accumulator::new(seed);
        let data = accumulator.write_many(data);

        Self::finish_with(seed, len, &accumulator, data)
    }

    /// Construct the hasher with initial seed
    #[must_use]
    pub const fn with_seed(seed: u32) -> Self {
        Self {
            seed,
            length: 0,
            accumulator: Accumulator::new(seed),
            buffer: Buffer::new(),
        }
    }

    /// The seed used to create this hasher
    pub const fn seed(&self) -> u32 {
        self.seed
    }

    /// The total no. of bytes hashed
    pub const fn total_len(&self) -> u64 {
        self.length
    }

    /// The total no. of bytes hashed, truncated to 32.
    ///
    /// For the full 64-bit count use [`total_len`](Self::total_len)
    pub const fn total_len_32(&self) -> u32 {
        self.length as u32
    }

    /// Returns the hash value for the input data so far.
    #[must_use]
    #[inline]
    pub fn finish_32(&self) -> u32 {
        Self::finish_with(
            self.seed,
            self.length,
            &self.accumulator,
            self.buffer.remaining(),
        )
    }

    #[inline]
    #[must_use]
    fn finish_with(seed: u32, len: u64, accumulator: &Accumulator, mut data: &[u8]) -> u32 {
        let mut acc = if len < BYTES_IN_LANE.into_u64() {
            seed.wrapping_add(PRIME32_5)
        } else {
            accumulator.finish()
        };

        acc += len as u32;

        while let Some((chunk, rest)) = data.split_first_chunk() {
            let lane = u32::from_ne_bytes(*chunk).to_le();

            acc = acc.wrapping_add(lane.wrapping_mul(PRIME32_3));
            acc = acc.rotate_left(17).wrapping_mul(PRIME32_4);

            data = rest;
        }

        for &byte in data {
            let lane = byte.into_u32();

            acc = acc.wrapping_add(lane.wrapping_mul(PRIME32_5));
            acc = acc.rotate_left(11).wrapping_mul(PRIME32_1);
        }

        acc ^= acc >> 15;
        acc = acc.wrapping_mul(PRIME32_2);
        acc ^= acc >> 13;
        acc = acc.wrapping_mul(PRIME32_3);
        acc ^= acc >> 16;

        acc
    }
}

impl core::hash::Hasher for Hasher {
    #[inline]
    fn write(&mut self, data: &[u8]) {
        let len = data.len().into_u64();

        let (buf_lanes, data) = self.buffer.extend(data);

        if let Some(&lanes) = buf_lanes {
            self.accumulator.write(lanes);
        }

        let data = self.accumulator.write_many(data);

        self.buffer.set(data);
        self.length += len;
    }

    #[inline]
    fn finish(&self) -> u64 {
        Hasher::finish_32(self).into()
    }
}

#[derive(Clone)]
pub struct State(u32);

impl State {
    /// Constructs the hasher w/ an initial seed.
    pub fn with_seed(seed: u32) -> Self {
        Self(seed)
    }
}

impl BuildHasher for State {
    type Hasher = Hasher;

    fn build_hasher(&self) -> Self::Hasher {
        Hasher::with_seed(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{
        array,
        hash::{BuildHasherDefault, Hasher as _},
    };
    use std::collections::HashMap;

    const _TRAITS: () = {
        const fn is_clone<T: Clone>() {}
        is_clone::<Hasher>();
        is_clone::<State>();
    };

    const EMPTY_BYTES: [u8; 0] = [];

    #[test]
    fn ingesting_byte_by_byte_is_equivalent_to_large_chunks() {
        let bytes = [0; 32];

        let mut byte_by_byte = Hasher::with_seed(0);
        for byte in bytes.chunks(1) {
            byte_by_byte.write(byte);
        }
        let byte_by_byte = byte_by_byte.finish();

        let mut one_chunk = Hasher::with_seed(0);
        one_chunk.write(&bytes);
        let one_chunk = one_chunk.finish();

        assert_eq!(byte_by_byte, one_chunk);
    }

    #[test]
    fn hash_of_nothing_matches_c_implementation() {
        let mut hasher = Hasher::with_seed(0);
        hasher.write(&EMPTY_BYTES);
        assert_eq!(hasher.finish(), 0x02cc_5d05);
    }

    #[test]
    fn hash_of_single_byte_matches_c_implementation() {
        let mut hasher = Hasher::with_seed(0);
        hasher.write(&[42]);
        assert_eq!(hasher.finish(), 0xe0fe_705f);
    }

    #[test]
    fn hash_of_multiple_bytes_matches_c_implementation() {
        let mut hasher = Hasher::with_seed(0);
        hasher.write(b"Hello, world!\0");
        assert_eq!(hasher.finish(), 0x9e5e_7e93);
    }

    #[test]
    fn hash_of_multiple_chunks_matches_c_implementation() {
        let bytes: [u8; 100] = array::from_fn(|i| i as u8);
        let mut hasher = Hasher::with_seed(0);
        hasher.write(&bytes);
        assert_eq!(hasher.finish(), 0x7f89_ba44);
    }

    #[test]
    fn hash_with_different_seed_matches_c_implementation() {
        let mut hasher = Hasher::with_seed(0x42c9_1977);
        hasher.write(&EMPTY_BYTES);
        assert_eq!(hasher.finish(), 0xd6bf_8459);
    }

    #[test]
    fn hash_with_different_seed_and_multiple_chunks_matches_c_implementation() {
        let bytes: [u8; 100] = array::from_fn(|i| i as u8);
        let mut hasher = Hasher::with_seed(0x42c9_1977);
        hasher.write(&bytes);
        assert_eq!(hasher.finish(), 0x6d2f_6c17);
    }

    #[test]
    fn hashes_with_different_offsets_are_the_same() {
        let bytes = [0x7c; 4096];
        let expected = Hasher::oneshot(0, &[0x7c; 64]);

        let the_same = bytes
            .windows(64)
            .map(|w| {
                let mut hasher = Hasher::with_seed(0);
                hasher.write(w);
                hasher.finish_32()
            })
            .all(|h| h == expected);
        assert!(the_same);
    }

    #[ignore]
    #[test]
    fn length_overflows_32bit() {
        // Hash 4.3 billion (4_300_000_000) bytes, which overflows a u32.
        let bytes200: [u8; 200] = array::from_fn(|i| i as _);

        let mut hasher = Hasher::with_seed(0);
        for _ in 0..(4_300_000_000 / bytes200.len()) {
            hasher.write(&bytes200);
        }

        assert_eq!(hasher.total_len(), 0x0000_0001_004c_cb00);
        assert_eq!(hasher.total_len_32(), 0x004c_cb00);

        // compared against the C implementation
        assert_eq!(hasher.finish(), 0x1522_4ca7);
    }

    #[test]
    fn can_be_used_in_a_hashmap_with_a_default_seed() {
        let mut hash: HashMap<_, _, BuildHasherDefault<Hasher>> = Default::default();
        hash.insert(42, "the answer");
        assert_eq!(hash.get(&42), Some(&"the answer"));
    }
}
