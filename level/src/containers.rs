use std::{mem::transmute, ops::Deref};

use miners::encoding::{Decode, Encode};

pub mod bitpack;
pub mod palette;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ByteArray<'a, const N: usize>(&'a [u8; N]);

impl<'a, const N: usize> Into<&'a [u8; N]> for ByteArray<'a, N> {
    fn into(self) -> &'a [u8; N] {
        self.0
    }
}

impl<const N: usize> Encode for ByteArray<'_, N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.0.as_ref()).map_err(From::from)
    }
}

impl<'dec, const N: usize> Decode<'dec> for ByteArray<'_, N> {
    fn decode(cursor: &mut std::io::Cursor<&'dec [u8]>) -> miners::encoding::decode::Result<Self> {
        let slice = decode_slice::<N>(cursor)?;
        // SAFETY: This is safe because we created the ptr from a slice that we know has a len of RLEN
        let data: &[u8; N] = unsafe { (slice.as_ptr().cast() as *const [u8; N]).as_ref().unwrap() };
        //let this = unsafe { Box::new(data) };
        Ok(Self(data))
    }
}

unsafe impl<const N: usize> ReadContainer<N, u8> for ByteArray<'_, N> {
    unsafe fn get_unchecked(&self, i: usize) -> u8 {
        *self.0.get_unchecked(i)
    }
}

#[repr(transparent)]
pub struct ByteArrayMut<'a, const N: usize>(&'a mut [u8; N]);

impl<'a, const N: usize> Deref for ByteArrayMut<'a, N> {
    type Target = ByteArray<'a, N>;

    fn deref(&self) -> &Self::Target {
        // SAFETY: This is fine because the types have the same layout
        unsafe { transmute(self) }
    }
}

impl<const N: usize> Encode for ByteArrayMut<'_, N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        self.0.encode(writer)
    }
}

unsafe impl<const N: usize> ReadContainer<N, u8> for ByteArrayMut<'_, N> {
    unsafe fn get_unchecked(&self, i: usize) -> u8 {
        self.deref().get_unchecked(i)
    }
}

unsafe impl<const N: usize> WriteContainer<N, u8> for ByteArrayMut<'_, N> {
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

pub use crate::half_byte_array;

// We have to do this weird macro stuff because rust doesn't allow use to use const expressions with const params.
/// A macro to access the HalfByteArray struct.
#[macro_export]
macro_rules! half_byte_array {
    ($f:ident) => {
        $crate::containers::__private::HalfByteArray::$f
    };
    ($l:lifetime, $len:literal) => {
        $crate::containers::__private::HalfByteArray<$l, {$len}, {$len/2}>
    };
}

#[macro_export]
macro_rules! half_byte_array_mut {
    ($f:ident) => {
        $crate::containers::__private::HalfByteArrayMut::$f
    };
    ($l:lifetime, $len:literal) => {
        $crate::containers::__private::HalfByteArrayMut<$l, {$len}, {$len/2}>
    };
}

pub mod __private {
    use std::{mem::transmute, ops::Deref};

    use miners::encoding::{Decode, Encode};

    use super::{ReadContainer, WriteContainer};

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub struct HalfByteArray<'a, const LEN: usize, const RLEN: usize>(&'a [u8; RLEN]);

    impl<'a, const LEN: usize, const RLEN: usize> Into<&'a [u8; RLEN]>
        for HalfByteArray<'a, LEN, RLEN>
    {
        fn into(self) -> &'a [u8; RLEN] {
            self.0
        }
    }

    impl<const LEN: usize, const RLEN: usize> Encode for HalfByteArray<'_, LEN, RLEN> {
        fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
            writer.write_all(self.0.as_ref()).map_err(From::from)
        }
    }

    impl<'dec, const LEN: usize, const RLEN: usize> Decode<'dec> for HalfByteArray<'_, LEN, RLEN> {
        fn decode(
            cursor: &mut std::io::Cursor<&'dec [u8]>,
        ) -> miners::encoding::decode::Result<Self> {
            let slice = super::decode_slice::<RLEN>(cursor)?;
            // SAFETY: This is safe because we created the ptr from a slice that we know has a len of RLEN
            let data: &[u8; RLEN] = unsafe { (slice.as_ptr().cast() as *const [u8; RLEN]).as_ref().unwrap() };
            //let this = unsafe { Box::new(data) };
            Ok(Self(data))
        }
    }

    unsafe impl<const LEN: usize, const RLEN: usize> ReadContainer<{ LEN }, u8>
        for HalfByteArray<'_, LEN, RLEN>
    {
        unsafe fn get_unchecked(&self, i: usize) -> u8 {
            let byte = *self.0.get_unchecked(i / 2);
            if i % 2 == 0 {
                (byte & 0xf0) >> 4
            } else {
                byte & 0x0f
            }
        }
    }

    #[repr(transparent)]
    pub struct HalfByteArrayMut<'a, const LEN: usize, const RLEN: usize>(&'a mut [u8; RLEN]);

    unsafe impl<const LEN: usize, const RLEN: usize> ReadContainer<{ LEN }, u8>
        for HalfByteArrayMut<'_, LEN, RLEN>
    {
        unsafe fn get_unchecked(&self, i: usize) -> u8 {
            self.deref().get_unchecked(i)
        }
    }

    impl<'a, const LEN: usize, const RLEN: usize> Deref for HalfByteArrayMut<'a, LEN, RLEN> {
        type Target = HalfByteArray<'a, LEN, RLEN>;

        fn deref(&self) -> &Self::Target {
            // SAFETY: This is fine because the types have the exact same layout
            unsafe { transmute(self) }
        }
    }

    unsafe impl<const LEN: usize, const RLEN: usize> WriteContainer<{ LEN }, u8>
        for HalfByteArrayMut<'_, LEN, RLEN>
    {
        unsafe fn set_unchecked(&mut self, i: usize, v: u8) {
            let byte = self.0.get_unchecked_mut(i / 2);
            if i % 2 == 0 {
                *byte = *byte & (v << 4)
            } else {
                *byte = *byte & v
            }
        }
    }
}

pub unsafe trait ReadContainer<const N: usize, V> /*Encode + for<'dec> Decode<'dec>*/ {
    fn get(&self, i: usize) -> V {
        if i >= N {
            panic!("out of bounds")
        }
        //SAFETY: This is safe because we know i is in bounds.
        unsafe { self.get_unchecked(i) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn get_unchecked(&self, i: usize) -> V;
}

pub unsafe trait WriteContainer<const N: usize, V>: ReadContainer<N, V> {
    fn set(&mut self, i: usize, v: V) {
        if i >= N {
            panic!("out of bounds")
        }
        // SAFETY: This is sound because we just checked the bounds
        unsafe { self.set_unchecked(i, v) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn set_unchecked(&mut self, i: usize, v: V);

    fn swap(&mut self, i: usize, v: V) -> V {
        if i >= N {
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

unsafe impl<const N: usize, T: palette::PaletteContainer<N>> ReadContainer<N, u16> for T {
    unsafe fn get_unchecked(&self, i: usize) -> u16 {
        self.get_unchecked(i)
    }
}

unsafe impl<const N: usize, T: palette::PaletteContainer<N>> WriteContainer<N, u16> for T {
    unsafe fn set_unchecked(&mut self, i: usize, v: u16) {
        self.set_unchecked(i, v)
    }
}
