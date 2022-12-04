use miners::encoding::{Decode, Encode};

pub mod bitpack;
pub mod palette;

#[derive(Clone)]
pub struct ByteArray<const N: usize>(Box<[u8; N]>);

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

impl<const N: usize> Encode for ByteArray<N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.0.as_ref()).map_err(From::from)
    }
}

impl<'dec, const N: usize> Decode<'dec> for ByteArray<N> {
    fn decode(cursor: &mut std::io::Cursor<&'dec [u8]>) -> miners::encoding::decode::Result<Self> {
        let slice = decode_slice::<N>(cursor)?;
        let data = (slice as *const [u8]).cast();
        // SAFETY: This is safe because we created the ptr from a slice that we know has a len of RLEN
        let this = unsafe { Box::new(*data) };
        Ok(Self(this))
    }
}

pub use crate::half_byte_array;

// We have to do this weird macro stuff because rust doesn't allow use to use const expressions with const params.
/// A macro to access the HalfByteArray struct.
#[macro_export]
macro_rules! half_byte_array {
    ($l:lifetime, $len:literal) => {
        $crate::containers::__private::HalfByteArray<$l, {$len}, {$len/2}>
    };
}

pub mod __private {
    use std::borrow::Cow;

    use miners::encoding::{Decode, Encode};

    use super::DataContainer;

    #[derive(Clone)]
    #[repr(transparent)]
    pub struct HalfByteArray<'a, const LEN: usize, const RLEN: usize>(Cow<'a, [u8; RLEN]>);

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
            let data = unsafe { *(slice as *const [u8]).cast() };
            //let this = unsafe { Box::new(data) };
            Ok(Self(Cow::Borrowed(data)))
        }
    }

    impl<const LEN: usize, const RLEN: usize> HalfByteArray<'_, LEN, RLEN> {
        pub fn new() -> Self {
            Self(Cow::Owned([0; RLEN]))
        }
    }

    unsafe impl<const LEN: usize, const RLEN: usize> DataContainer<{ LEN }, u8>
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

        unsafe fn set_unchecked(&mut self, i: usize, v: u8) {
            self.0 = self.0.to_owned();
            let byte = self.0.to_mut().get_unchecked_mut(i / 2);
            if i % 2 == 0 {
                *byte = *byte & (v << 4)
            } else {
                *byte = *byte & v
            }
        }
    }
}

pub unsafe trait DataContainer<const N: usize, V>: Encode + for<'dec> Decode<'dec> {
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

unsafe impl<const N: usize, T: palette::PaletteContainer<N> + Encode + for<'dec> Decode<'dec>>
    DataContainer<N, u16> for T
{
    unsafe fn get_unchecked(&self, i: usize) -> u16 {
        self.get_unchecked(i)
    }
    unsafe fn set_unchecked(&mut self, i: usize, v: u16) {
        self.set_unchecked(i, v)
    }
}
