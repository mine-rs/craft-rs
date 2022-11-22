/// # Safety
/// This trait is safe to implement as long as the struct has the same data layout as `u64`
pub unsafe trait ByteOrderedU64: Copy + Clone + Default {
    // used for en/decoding
    fn bo_to_be(v: u64) -> u64;
    fn be_to_bo(v: u64) -> u64;
    // used for using the value internally
    fn to_ne(self) -> u64;
    fn from_ne(v: u64) -> Self;
}

#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct BigEndian(u64);

// SAFETY: This is fine because the struct is `repr(transparent)`
unsafe impl ByteOrderedU64 for BigEndian {
    #[inline(always)]
    fn be_to_bo(v: u64) -> u64 {
        v
    }

    #[inline(always)]
    fn bo_to_be(v: u64) -> u64 {
        v
    }

    fn to_ne(self) -> u64 {
        self.0.swap_bytes()
    }

    fn from_ne(v: u64) -> Self {
        Self(v.swap_bytes())
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct NativeEndian(u64);

// SAFETY: This is fine because the struct is `repr(transparent)`
unsafe impl ByteOrderedU64 for NativeEndian {
    fn bo_to_be(v: u64) -> u64 {
        v.to_be()
    }
    fn be_to_bo(v: u64) -> u64 {
        #[cfg(target_endian="little")]
        {
            v.swap_bytes()
        }
        #[cfg(target_endian="big")]
        {
            v
        }
    }
    fn to_ne(self) -> u64 {
        self.0
    }
    fn from_ne(v: u64) -> Self {
        Self(v)
    }
}