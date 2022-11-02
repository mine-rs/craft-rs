use crate::bitpack::PackedBits;
use craftrs_block::State;
use std::{collections::BTreeMap, mem::MaybeUninit};

//TODO: Add encoding

//TODO: Add a BiomePalette struct

//TODO: Refactor !

//TODO: Tests !


//TODO: Use a union instead of an enum
pub struct BiomePaletteContainer<S> {
    palette: BiomePaletteImpl<S>,
    data: MaybeUninit<PackedBits>,
    // This is used to only later initialise the `PackedBits`
    len: usize, 
}

impl<S: State> BiomePaletteContainer<S> {
    pub fn new(len: usize, value: S) -> Self {
        Self { palette: BiomePaletteImpl::SingleValuePalette(SingleValuePalette {value }), data: MaybeUninit::uninit(), len }
    }

    pub fn get(&self, i: usize) -> Option<S> {
        match &self.palette {
            BiomePaletteImpl::SingleValuePalette(p) => {
                if i == 0 {
                    return Some(p.value)
                }
                panic!("out of bounds")
            }
            BiomePaletteImpl::LinearPalette(p) => {
                //SAFETY: We know this is safe as data is constructed when a LinearPalette is created.
                let data = unsafe { self.data.assume_init_ref() };
                Some(p.state(data.get(i)? as usize))
            }
        }
    }

    pub fn set(&mut self, i: usize, state: S) {
        if i > self.len-1 {
            panic!("out of bounds")
        }
        //SAFETY: This is fine becuase we checked that the index is in bounds 
        unsafe {
            self.set_unchecked(i, state)
        }
    }

    pub unsafe fn set_unchecked(&mut self, i: usize, state: S) {
        loop {
            match &mut self.palette {
                BiomePaletteImpl::SingleValuePalette(p) => {
                    if p.value == state {
                        return
                    };
                    self.palette = BiomePaletteImpl::LinearPalette(LinearPalette {
                        bits: 1,
                        values: Vec::with_capacity(2),
                    });
                    self.data = MaybeUninit::new(PackedBits::new(self.len, 1))
                },
                BiomePaletteImpl::LinearPalette(p) => {
                    loop {
                        match p.index(state) {
                            IndexOrBits::Index(index) => {
                                let data = self.data.assume_init_mut();
                                data.set_unchecked(i, index as u64)
                            }, 
                            IndexOrBits::Bits(bits) => {
                                let mut packedbits = PackedBits::new(self.len, bits);
                                for i in 0..self.data.assume_init_ref().len() {
                                    packedbits.set_unchecked(i, packedbits.get_unchecked(i));
                                }
                                p.values.reserve_exact(2usize.pow(bits as u32) / 2)
                            },
                        }
                    }
                },
            }
        }

    }
}

// TODO: Use a union instead of an enum
pub struct StatePaletteContainer<S> {
    palette: StatePaletteImpl<S>,
    data: PackedBits,
}

impl<S: State> StatePaletteContainer<S> {
    pub fn new(len: usize, value: S) -> Self {
        Self {
            palette: StatePaletteImpl::SingleValuePalette(SingleValuePalette { value }),
            data: PackedBits::new(len, 0),
        }
    }

    pub fn with_data(bits: usize, data: PackedBits, palette: &[S]) -> Self {
        match bits {
            0 => Self {
                palette: StatePaletteImpl::SingleValuePalette(SingleValuePalette { value: palette[0] }),
                data,
            },
            1..=4 => {
                let mut values = Vec::with_capacity(16);
                values.copy_from_slice(palette);
                Self {
                    palette: StatePaletteImpl::LinearPalette(LinearPalette { bits: 4, values }),
                    data,
                }
            }
            5..=8 => {
                let mut values = Vec::with_capacity(32);
                let mut indices = BTreeMap::new();
                for i in 0..palette.len() {
                    indices.insert(palette[i], i);
                    values.push(palette[i]);
                }
                Self {
                    palette: StatePaletteImpl::MappedPalette(MappedPalette {
                        indices,
                        inner: LinearPalette { values, bits },
                    }),
                    data,
                }
            }
            _ => todo!(),
        }
    }

    pub fn get(&self, i: usize) -> Option<S> {
        Some(self.palette.state(self.data.get(i)? as usize))
    }

    pub unsafe fn get_unchecked(&self, i: usize) -> S {
        self.palette.state(self.data.get_unchecked(i) as usize)
    }

    //TODO: triple check if this is correct
    pub fn set(&mut self, i: usize, state: S) {
        if i > self.data.len() - 1 {
            panic!("out of bounds")
        }
        // SAFETY: We just checked that is in bounds
        unsafe { self.set_unchecked(i, state) }
    }

    /// Safe as long as `i` is within bounds.
    pub unsafe fn set_unchecked(&mut self, i: usize, state: S) {
        match self.palette.index(state) {
            IndexOrBits::Index(v) => self.data.set_unchecked(i, v as u64),
            IndexOrBits::Bits(bits) => {
                match bits {
                    1 => {
                        let palette = match self.palette {
                            StatePaletteImpl::SingleValuePalette(p) => p,
                            StatePaletteImpl::LinearPalette(_) => std::hint::unreachable_unchecked(),
                            StatePaletteImpl::MappedPalette(_) => std::hint::unreachable_unchecked(),
                            StatePaletteImpl::Global => std::hint::unreachable_unchecked()
                        };
                        // SAFETY: we know it's not `Global` so this is fine
                        let mut values = Vec::with_capacity(16);
                        values.push(palette.value);
                        let palette = StatePaletteImpl::LinearPalette(LinearPalette {
                            bits: 4,
                            values
                        });

                        let mut new = Self {
                            data: PackedBits::new(self.data.len(), bits),
                            palette,
                        };
                        //SAFETY: This is sound because we know it is in bounds as we specified the len.
                        for i in 0..self.data.len() {
                            new.data.set_unchecked(i, self.data.get_unchecked(i))
                        }

                        *self = new
                    }

                    5 => {
                        // SAFETY: This is fine because the bits will only be 5 when the palette was a `LinearPalette`
                        let palette = match &mut self.palette {
                            StatePaletteImpl::SingleValuePalette(_) => {
                                std::hint::unreachable_unchecked()
                            }
                            StatePaletteImpl::LinearPalette(p) => p,
                            StatePaletteImpl::MappedPalette(_) => std::hint::unreachable_unchecked(),
                            StatePaletteImpl::Global => std::hint::unreachable_unchecked(),
                        };
                        let mut indices = BTreeMap::new();
                        for i in 0..palette.values.len() {
                            let value = palette.values[i];
                            indices.insert(value, i);
                        }

                        let values = std::mem::take(&mut palette.values);

                        let palette = StatePaletteImpl::MappedPalette(MappedPalette {
                            indices,
                            inner: LinearPalette { values, bits },
                        });

                        let mut new = Self {
                            data: PackedBits::new(self.data.len(), bits),
                            palette,
                        };
                        //SAFETY: This is sound because we know it is in bounds as we specified the len.

                        for i in 0..self.data.len() {
                            new.data.set_unchecked(i, self.data.get_unchecked(i))
                        }
                        *self = new
                    }
                    6..=8 => {
                        // SAFETY: This is fine because the bits will only be 6..8 when the palette was a `MappedPalette`
                        let mut palette = match &mut self.palette {
                            StatePaletteImpl::SingleValuePalette(_) => {
                                std::hint::unreachable_unchecked()
                            }
                            StatePaletteImpl::LinearPalette(_) => std::hint::unreachable_unchecked(),
                            StatePaletteImpl::MappedPalette(p) => p,
                            StatePaletteImpl::Global => std::hint::unreachable_unchecked(),
                        };
                        palette.inner.bits = bits;
                    }
                    9 => {
                        let palette = StatePaletteImpl::<S>::Global;
                        let mut new = Self {
                            data: PackedBits::new(self.data.len(), 15),
                            palette,
                        };

                        for i in 0..self.data.len() {
                            //SAFETY: Because of the way the for loop is defined, i will alwasys be in bounds.
                            let state = self.get_unchecked(i);
                            new.data.set(i, state.into())
                        }
                        *self = new
                    }
                    // This is sound because we know bits can only be 5, 6..=8, or 9.
                    _ => std::hint::unreachable_unchecked(),
                }
                self.set_unchecked(i, state)
            }
        }
    }
}

enum StatePaletteImpl<S> {
    SingleValuePalette(SingleValuePalette<S>),
    LinearPalette(LinearPalette<S>),
    MappedPalette(MappedPalette<S>),
    Global,
}

impl<S: State> StatePaletteImpl<S> {
    fn index(&mut self, state: S) -> IndexOrBits {
        match self {
            StatePaletteImpl::SingleValuePalette(this) => this.index(state),
            StatePaletteImpl::LinearPalette(this) => this.index(state),
            StatePaletteImpl::MappedPalette(this) => this.index(state),
            StatePaletteImpl::Global => IndexOrBits::Index(state.into() as u64),
        }
    }
    fn state(&self, index: usize) -> S {
        match self {
            StatePaletteImpl::SingleValuePalette(this) => this.state(index),
            StatePaletteImpl::LinearPalette(this) => this.state(index),
            StatePaletteImpl::MappedPalette(this) => this.state(index),
            StatePaletteImpl::Global => S::from(index),
        }
    }
}

enum BiomePaletteImpl<S> {
    SingleValuePalette(SingleValuePalette<S>),
    LinearPalette(LinearPalette<S>)
}

// S = block state
pub trait Palette<S>: Into<Vec<S>> {
    fn index(&mut self, state: S) -> IndexOrBits;
    fn state(&self, index: usize) -> S;
}

// TODO: Rename?
pub enum IndexOrBits {
    Index(u64),
    Bits(usize),
}

#[derive(Copy, Clone)]
pub struct SingleValuePalette<S> {
    pub(crate) value: S,
}

impl<S> Into<Vec<S>> for SingleValuePalette<S> {
    fn into(self) -> Vec<S> {
        let mut vec = Vec::with_capacity(2);
        vec.push(self.value);
        vec
    }
}

impl<S: State> Palette<S> for SingleValuePalette<S> {
    fn index(&mut self, state: S) -> IndexOrBits {
        if self.value == state {
            IndexOrBits::Index(0)
        } else {
            IndexOrBits::Bits(1)
        }
    }

    fn state(&self, index: usize) -> S {
        if index == 0 {
            self.value
        } else {
            panic!("index out of bounds")
        }
    }
}

pub struct LinearPalette<S> {
    pub(crate) values: Vec<S>,
    pub(crate) bits: usize,
}

impl<S> Into<Vec<S>> for LinearPalette<S> {
    fn into(self) -> Vec<S> {
        self.values
    }
}

impl<S: State> Palette<S> for LinearPalette<S> {
    fn index(&mut self, state: S) -> IndexOrBits {
        for i in 0..self.values.len() {
            // SAFETY: This is fine because i can only be in bounds due to the for loop.
            unsafe {
                if *self.values.get_unchecked(i) == state {
                    return IndexOrBits::Index(i as u64);
                }
            }
        }
        let len = self.values.len();
        if self.values.capacity() - self.values.len() > 0 {
            self.values.push(state);
            IndexOrBits::Index(len as u64)
        } else {
            IndexOrBits::Bits(self.bits+1)
        }
    }

    #[inline]
    fn state(&self, index: usize) -> S {
        self.values[index]
    }
}

/// This makes the `index` method faster at the cost of memory usage.
pub struct MappedPalette<S> {
    pub(crate) indices: BTreeMap<S, usize>,
    pub(crate) inner: LinearPalette<S>,
}

impl<S> Into<Vec<S>> for MappedPalette<S> {
    fn into(self) -> Vec<S> {
        self.inner.values
    }
}

impl<S: State> Palette<S> for MappedPalette<S> {
    fn index(&mut self, state: S) -> IndexOrBits {
        match self.indices.get(&state) {
            Some(v) => IndexOrBits::Index(*v as u64),
            None => {
                let initial_len = self.inner.values.len();
                if self.inner.values.capacity() - initial_len > 0 {
                    self.inner.values.push(state);
                    self.indices.insert(state, self.inner.values.len());
                    IndexOrBits::Index(initial_len as u64)
                } else {
                    IndexOrBits::Bits(self.inner.bits + 1)
                }
            }
        }
    }

    fn state(&self, index: usize) -> S {
        self.inner.state(index)
    }
}
