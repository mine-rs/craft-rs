use std::mem::transmute;

use miners::encoding::{Decode, Encode};

pub mod bitpack;
pub mod palette;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ByteArray<const N: usize>([u8; N]);

impl<'a, const N: usize> From<&'a [u8; N]> for &'a ByteArray<N> {
    fn from(value: &'a [u8; N]) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { transmute(value) }
    }
}

impl<'a, const N: usize> From<&'a ByteArray<N>> for &'a [u8; N] {
    fn from(value: &'a ByteArray<N>) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { transmute(value) }
    }
}

impl<'a, const N: usize> From<&'a mut [u8; N]> for &'a mut ByteArray<N> {
    fn from(value: &'a mut [u8; N]) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { transmute(value) }
    }
}

impl<'a, const N: usize> From<&'a mut ByteArray<N>> for &'a mut [u8; N] {
    fn from(value: &'a mut ByteArray<N>) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { transmute(value) }
    }
}

impl<const N: usize> Encode for ByteArray<N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.0.as_ref()).map_err(From::from)
    }
}

impl<'dec, const N: usize> Decode<'dec> for &ByteArray<N> {
    fn decode(cursor: &mut std::io::Cursor<&'dec [u8]>) -> miners::encoding::decode::Result<Self> {
        let slice = decode_slice::<N>(cursor)?;
        // SAFETY: This is safe because we created the ptr from a slice that we know has a len of RLEN
        let data: &[u8; N] = unsafe { &*(slice.as_ptr().cast() as *const [u8; N]) };
        //let this = unsafe { Box::new(data) };
        Ok(Self::from(data))
    }
}

// SAFETY: This is fine because we uphold all of the invariants
unsafe impl<const N: usize> ReadContainer<u8> for ByteArray<N> {
    const N: usize = N;
    unsafe fn get_unchecked(&self, i: usize) -> u8 {
        *self.0.get_unchecked(i)
    }
}

// SAFETY: This is fine because we uphold all of the invariants
unsafe impl<const N: usize> WriteContainer<u8> for ByteArray<N> {
    unsafe fn set_unchecked(&mut self, i: usize, v: u8) {
        *self.0.get_unchecked_mut(i) = v
    }
}

#[inline]
fn decode_slice<'dec, const N: usize>(
    cursor: &mut std::io::Cursor<&'dec [u8]>,
) -> miners::encoding::decode::Result<&'dec [u8]> {
    let pos = cursor.position() as usize;
    let slice = cursor
        .get_ref()
        .get(pos..pos + N as usize)
        .ok_or(miners::encoding::decode::Error::UnexpectedEndOfSlice)?;
    cursor.set_position((pos + N) as u64);
    debug_assert_eq!(slice.len(), N);
    Ok(slice)
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct HalfByteArray<const RLEN: usize>([u8; RLEN]);

impl<'a, const RLEN: usize> From<&'a [u8; RLEN]> for &'a HalfByteArray<RLEN> {
    fn from(value: &'a [u8; RLEN]) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { std::mem::transmute(value) }
    }
}

impl<'a, const RLEN: usize> From<&'a HalfByteArray<RLEN>> for &'a [u8; RLEN] {
    fn from(value: &'a HalfByteArray<RLEN>) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { std::mem::transmute(value) }
    }
}

impl<'a, const RLEN: usize> From<&'a mut [u8; RLEN]> for &'a mut HalfByteArray<RLEN> {
    fn from(value: &'a mut [u8; RLEN]) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { std::mem::transmute(value) }
    }
}

impl<'a, const RLEN: usize> From<&'a mut HalfByteArray<RLEN>> for &'a mut [u8; RLEN] {
    fn from(value: &'a mut HalfByteArray<RLEN>) -> Self {
        // SAFETY: This is fine because ByteArray is repr(transparent)
        unsafe { std::mem::transmute(value) }
    }
}

impl<const RLEN: usize> Encode for HalfByteArray<RLEN> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.0.as_ref()).map_err(From::from)
    }
}

impl<'dec, const RLEN: usize> Decode<'dec> for &'dec HalfByteArray<RLEN> {
    fn decode(cursor: &mut std::io::Cursor<&'dec [u8]>) -> miners::encoding::decode::Result<Self> {
        let slice = decode_slice::<RLEN>(cursor)?;
        // SAFETY: This is safe because we created the ptr from a slice that we know has a len of RLEN
        let data: &[u8; RLEN] = unsafe { &*(slice.as_ptr().cast() as *const [u8; RLEN]) };
        Ok(Self::from(data))
    }
}

// SAFETY: This is fine because we uphold all of the invariants
unsafe impl<const RLEN: usize> ReadContainer<u8> for HalfByteArray<RLEN> {
    const N: usize = RLEN * 2;

    unsafe fn get_unchecked(&self, i: usize) -> u8 {
        let byte = *self.0.get_unchecked(i / 2);
        if i % 2 == 0 {
            (byte & 0xf0) >> 4
        } else {
            byte & 0x0f
        }
    }
}

// SAFETY: This is fine because we uphold all of the invariants
unsafe impl<const RLEN: usize> WriteContainer<u8> for HalfByteArray<RLEN> {
    fn set(&mut self, i: usize, v: u8) {
        if i >= RLEN / 2 + RLEN % 2 {
            panic!("out of bounds")
        }
        // SAFETY: This is fine because we just checked the bounds
        unsafe { self.set_unchecked(i, v) }
    }

    unsafe fn set_unchecked(&mut self, i: usize, v: u8) {
        let byte = self.0.get_unchecked_mut(i / 2);
        if i % 2 == 0 {
            *byte = *byte & (v << 4)
        } else {
            *byte = *byte & v
        }
    }
}

pub unsafe trait ReadContainer<V> {
    const N: usize;

    fn get(&self, i: usize) -> V {
        if i >= Self::N {
            panic!("out of bounds")
        }
        //SAFETY: This is safe because we know i is in bounds.
        unsafe { self.get_unchecked(i) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn get_unchecked(&self, i: usize) -> V;
}

pub unsafe trait WriteContainer<V>: ReadContainer<V> {
    fn set(&mut self, i: usize, v: V) {
        if i >= Self::N {
            panic!("out of bounds")
        }
        // SAFETY: This is sound because we just checked the bounds
        unsafe { self.set_unchecked(i, v) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn set_unchecked(&mut self, i: usize, v: V);

    fn swap(&mut self, i: usize, v: V) -> V {
        if i >= Self::N {
            panic!("out of bounds")
        }
        //SAFETY: This is safe because we just checked the bounds.
        unsafe { self.swap_unchecked(i, v) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds
    unsafe fn swap_unchecked(&mut self, i: usize, v: V) -> V {
        let val = self.get_unchecked(i);
        self.set_unchecked(i, v);
        val
    }
}

// SAFETY: This is safe because implementing PaletteContainer has the same invariants as ReadContainer/WriteContainer
unsafe impl<T: palette::PaletteContainer> ReadContainer<u16> for T {
    const N: usize = Self::N;
    unsafe fn get_unchecked(&self, i: usize) -> u16 {
        self.get_unchecked(i)
    }
}

// SAFETY: This is safe because implementing PaletteContainer has the same invariants as ReadContainer/WriteContainer
unsafe impl<T: palette::PaletteContainer> WriteContainer<u16> for T {
    unsafe fn set_unchecked(&mut self, i: usize, v: u16) {
        self.set_unchecked(i, v)
    }
}
