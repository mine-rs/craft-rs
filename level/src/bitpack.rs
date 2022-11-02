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
        if bits == 0 || bits > 64 {
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
    pub fn from_data(bits: usize, data: &[u64]) -> Self {
        let mut this = Self::new(data.len(), bits);
        this.data.copy_from_slice(data);
        this
    }

    /// Constructs a new `PackedBits` with data, the data supplied has to not have been packed yet.
    #[inline]
    pub fn from_data_unpacked(bits: usize, data: &[u64]) -> Self {
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
        if i < self.len - 1 {
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
        if i > self.len - 1 {
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
}
