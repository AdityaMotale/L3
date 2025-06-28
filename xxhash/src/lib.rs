#![allow(dead_code)]

mod xxhash32;

pub(crate) trait IntoU32 {
    fn into_u32(self) -> u32;
}

impl IntoU32 for u8 {
    #[inline(always)]
    fn into_u32(self) -> u32 {
        self.into()
    }
}

pub(crate) trait IntoU64 {
    fn into_u64(self) -> u64;
}

impl IntoU64 for u8 {
    #[inline(always)]
    fn into_u64(self) -> u64 {
        self.into()
    }
}
