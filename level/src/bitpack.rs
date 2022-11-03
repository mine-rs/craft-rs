pub struct PackedBits {
    len: usize,
    pub(crate) bits: usize,
    mask: u64,
    vpe: usize, // values per element in the vector
    data: Vec<u64>,
}

pub struct PackedBitsIter {
    inner: PackedBits,
    index: usize,
}
impl Iterator for PackedBitsIter {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.inner.get(self.index);
        self.index += 1;
        v
    }
}

impl IntoIterator for PackedBits {
    type Item = u64;
    type IntoIter = PackedBitsIter;
    fn into_iter(self) -> Self::IntoIter {
        PackedBitsIter {
            index: 0,
            inner: self,
        }
    }
}

impl AsRef<Vec<u64>> for PackedBits {
    fn as_ref(&self) -> &Vec<u64> {
        &self.data
    }
}

impl PackedBits {
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    /// Constructs a new `PackedBits`, panics if `bits` is equal to zero or if bits is greater than 64.
    #[inline]
    pub fn new(len: usize, bits: usize) -> Self {
        if bits == 0 || bits > 32 {
            panic!("invalid amount of bits")
        }
        Self::new_unchecked(len, bits)
    }

    #[inline]
    pub fn new_unchecked(len: usize, bits: usize) -> Self {
        let vpe = 64 / bits;
        let rlen = (len + vpe - 1) / vpe; // The real length of the vec
        Self {
            len,
            bits,
            mask: (1 << bits) - 1,
            data: vec![0; rlen],
            vpe: 64 / bits,
        }
    }

    /// Constructs a new `PackedBits` with data, the data supplied has to already be packed.
    #[inline]
    pub fn with_data(bits: usize, data: &[u64]) -> Self {
        let mut this = Self::new(data.len(), bits);
        this.data.copy_from_slice(data);
        this
    }

    /// Constructs a new `PackedBits` with data, the data supplied has to not have been packed yet.
    #[inline]
    pub fn with_data_unpacked(bits: usize, data: &[u64]) -> Self {
        let mut this = Self::new(data.len(), bits);
        for i in 0..data.len() {
            this.set(i, data[i]);
        }
        this
    }

    #[inline]
    fn calculate_index(&self, i: usize) -> (usize, u64, usize) {
        let vi = i / self.vpe; // vec index
        let bo = i % self.vpe * self.bits; // bit offset
        let bits = self.mask << bo;
        (vi, bits, bo)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<u64> {
        if i >= self.len  {
            return None;
        }
        // SAFETY: This is fine because we already checked that the index is within bounds.
        unsafe { Some(self.get_unchecked(i)) }
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, i: usize) -> u64 {
        let (vi, bits, bo) = self.calculate_index(i);
        (self.data.get_unchecked(vi) & bits) >> bo
    }

    #[inline]
    pub fn set(&mut self, i: usize, v: u64) {
        if i >= self.len {
            panic!("out of bounds")
        }
        // SAFETY: This is fine because we already checked that the index is within bounds.
        unsafe { self.set_unchecked(i, v) }
    }

    #[inline]
    pub unsafe fn set_unchecked(&mut self, i: usize, v: u64) {
        let (vi, bits, bi) = self.calculate_index(i);
        let num = self.data.get_unchecked_mut(vi);
        *num &= !bits;
        *num |= v << bi;
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
        let mut new = Self::new_unchecked(self.len, bits);
        for i in 0..self.len {
            // SAFETY: We know this is sound because 1. the lenghts are the same, and 2. the for loop makes sure `i` is in bounds
            unsafe {
                new.set_unchecked(i, self.get_unchecked(i))
            }
        }
        *self = new;
    }
}
#[cfg(test)]
mod tests {
    use super::PackedBits;

    #[test]
    fn bitpack() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let new_data = vec![7, 6, 5, 4, 3, 2, 1, 0];
        let mut packedbits = PackedBits::with_data_unpacked(3, &data);
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

            packedbits.change_bits(bits+1);
        }
    }
}