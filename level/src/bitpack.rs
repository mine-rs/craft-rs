use std::ops::Deref;

use miners::encoding::Encode;

pub(crate) mod byteorder;

#[repr(transparent)]
pub struct PackedBitsNE<const N: usize> {
    inner: PackedBits<N, byteorder::NativeEndian>,
}

impl<const N: usize> From<PackedBits<N, byteorder::NativeEndian>> for PackedBitsNE<N> {
    fn from(inner: PackedBits<N, byteorder::NativeEndian>) -> Self {
        Self {
            inner
        }
    }
}

impl<const N: usize> Encode for PackedBitsNE<N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        for i in &self.data {
            i.encode(writer)?;
        }
        Ok(())
    }
}

impl<const N: usize> AsRef<[u64]> for PackedBitsNE<N> {
    fn as_ref(&self) -> &[u64] {
        // SAFETY: This is fine because the `NativeEndian` struct has the same layout as `u64`
        unsafe { std::mem::transmute(self.data.as_slice()) }
    }
}

impl<const N: usize> Deref for PackedBitsNE<N> {
    type Target = PackedBits<N, byteorder::NativeEndian>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[repr(transparent)]
pub struct PackedBitsBE<const N: usize> {
    inner: PackedBits<N, byteorder::BigEndian>,
}

impl<const N: usize> From<PackedBits<N, byteorder::BigEndian>> for PackedBitsBE<N> {
    fn from(inner: PackedBits<N, byteorder::BigEndian>) -> Self {
        Self {
            inner
        }
    }
}

impl<const N: usize> Encode for PackedBitsBE<N> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.as_ref()).map_err(From::from)
    }
}

impl<const N: usize> AsRef<[u8]> for PackedBitsBE<N> {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: This is fine because the `BigEndian` struct has the same layout as a `u64`
        // and there are 8 bytes in a `u64` so the length is multiplied by 8.
        unsafe {
            std::slice::from_raw_parts(std::mem::transmute(self.data.as_ptr()), self.data.len() * 8)
        }
    }
}

impl<const N: usize> Deref for PackedBitsBE<N> {
    type Target = PackedBits<N, byteorder::BigEndian>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Default, Clone)]
pub struct PackedBits<const N: usize, B: byteorder::ByteOrderedU64> {
    pub(crate) bits: usize,
    mask: u64,
    vpe: usize, // values per element in the vector
    data: Vec<B>,
}

pub struct PackedBitsIter<const N: usize, B: byteorder::ByteOrderedU64> {
    inner: PackedBits<N, B>,
    index: usize,
}
impl<const N: usize, B: byteorder::ByteOrderedU64> Iterator for PackedBitsIter<N, B> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.inner.get(self.index);
        self.index += 1;
        v
    }
}

impl<const N: usize, B: byteorder::ByteOrderedU64> IntoIterator for PackedBits<N, B> {
    type Item = u64;
    type IntoIter = PackedBitsIter<N, B>;
    fn into_iter(self) -> Self::IntoIter {
        PackedBitsIter {
            index: 0,
            inner: self,
        }
    }
}

//impl<const N: usize, B: byteorder::ByteOrderedU64> AsRef<Vec<u64>> for PackedBits<N, B> {
//    fn as_ref(&self) -> &Vec<u64> {
//        &self.data
//    }
//}

impl<const N: usize, B: byteorder::ByteOrderedU64> PackedBits<N, B> {
    /// Constructs a new `PackedBits`, panics if `bits` is equal to zero or if bits is greater than 64.
    #[inline]
    pub fn new(bits: usize) -> Self {
        if bits == 0 || bits > 32 {
            panic!("invalid amount of bits")
        }
        Self::new_unchecked(bits)
    }

    #[inline]
    pub fn new_unchecked(bits: usize) -> Self {
        let vpe = 64 / bits;
        let rlen = (N + vpe - 1) / vpe; // The real length of the vec
        Self {
            bits,
            mask: ((((1 as u64) << bits) - 1) as u64).rotate_right(bits as u32),
            data: vec![B::from_ne(0); rlen],
            vpe: 64 / bits,
        }
    }

    /// Constructs a new `PackedBits` with data, the data supplied has to already be packed.
    #[inline]
    #[allow(dead_code)] // this will be useful for encoding/decoding
    pub fn with_data(bits: usize, data: &[u64]) -> Self {
        let mut this = Self::new(bits);
        // SAFETY: This is fine because we know `B` has the same size as `u64`
        let data = unsafe { std::mem::transmute(data) };
        this.data.copy_from_slice(data);
        this
    }

    /// Constructs a new `PackedBits` with data, the data supplied has to not have been packed yet.
    #[inline]
    #[allow(dead_code)] // this will be useful for encoding/decoding
    pub fn with_data_unpacked(bits: usize, data: &[u64]) -> Self {
        let mut this = Self::new(bits);
        for i in 0..data.len() {
            this.set(i, data[i]);
        }
        this
    }

    #[inline]
    fn calculate_index(&self, i: usize) -> (usize, u64, usize) {
        let vi = i / self.vpe; // vec index
        let bo = i % self.vpe * self.bits; // bit offset
        let bits = self.mask >> bo;
        (vi, bits, bo)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<u64> {
        if i >= N {
            return None;
        }
        // SAFETY: This is fine because we already checked that the index is within bounds.
        unsafe { Some(self.get_unchecked(i)) }
    }

    /// # Safety
    /// This is safe as long as `i` is within bounds.
    #[inline]
    pub unsafe fn get_unchecked(&self, i: usize) -> u64 {
        let (vi, bits, bo) = self.calculate_index(i);
        let num = self.data.get_unchecked(vi).to_ne();
        (((num & bits) << bo) as u64).rotate_left(self.bits as u32)
    }

    #[inline]
    pub fn set(&mut self, i: usize, v: u64) {
        if i >= N {
            panic!("out of bounds")
        }
        // SAFETY: This is fine because we already checked that the index is within bounds.
        unsafe { self.set_unchecked(i, v) }
    }

    #[inline]
    pub unsafe fn set_unchecked(&mut self, i: usize, v: u64) {
        let (vi, bits, bo) = self.calculate_index(i);
        let element = self.data.get_unchecked_mut(vi);
        // convert the endianness for usage
        let mut num = element.to_ne();
        num &= !bits; // set the value to zero
        num |= v.rotate_right(self.bits as u32) >> bo;
        // convert the endianness for storage
        *element = B::from_ne(num);
    }

    #[inline]
    pub fn swap(&mut self, i: usize, v: u64) -> Option<u64> {
        let val = self.get(i)?;
        //SAFETY: This is fine because the self.get call already checked bounds.
        unsafe { self.set_unchecked(i, v) };
        Some(val)
    }

    #[inline]
    pub unsafe fn swap_unchecked(&mut self, i: usize, v: u64) -> u64 {
        let val = self.get_unchecked(i);
        self.set_unchecked(i, v);
        val
    }

    pub fn change_bits(&mut self, bits: usize) {
        let mut new = Self::new_unchecked(bits);
        for i in 0..N {
            // SAFETY: We know this is sound because 1. the lenghts are the same, and 2. the for loop makes sure `i` is in bounds
            unsafe { new.set_unchecked(i, self.get_unchecked(i)) }
        }
        *self = new;
    }
}
#[cfg(test)]
mod tests {
    use super::byteorder;
    use super::PackedBits;

    #[test]
    fn bitpack() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let new_data = vec![7, 6, 5, 4, 3, 2, 1, 0];
        let mut packedbits = PackedBits::<8, byteorder::NativeEndian>::with_data_unpacked(3, &data);
        for bits in 3..=32 {
            for i in 0..8 {
                assert_eq!(packedbits.get(i).unwrap(), data[i]);
                packedbits.set(i, new_data[i]);
                assert_eq!(packedbits.get(i).unwrap(), new_data[i]);
                packedbits.set(i, data[i])
            }

            if bits == 32 {
                break;
            }

            packedbits.change_bits(bits + 1);
        }
    }
}
