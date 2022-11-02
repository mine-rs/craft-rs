use crate::bitpack::PackedBits;
use craftrs_block::State;
use std::collections::BTreeMap;

//TODO: Add encoding

//TODO: Add a BiomePalette struct

//TODO: Refactor !

//TODO: Tests !

pub struct StatePalette<S> {
    palette: PaletteImpl<S>,
    data: PackedBits,
}

impl<S: State> StatePalette<S> {
    pub fn new(len: usize, value: S) -> Self {
        Self {
            palette: PaletteImpl::SingleValuePalette(SingleValuePalette { value }),
            data: PackedBits::new(len, 0),
        }
    }

    pub fn with_data(bits: usize, data: PackedBits, palette: &[S]) -> Self {
        match bits {
            0 => Self {
                palette: PaletteImpl::SingleValuePalette(SingleValuePalette { value: palette[0] }),
                data,
            },
            1..=4 => {
                let mut values = Vec::with_capacity(16);
                values.copy_from_slice(palette);
                Self {
                    palette: PaletteImpl::LinearPalette(LinearPalette { bits: 4, values }),
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
                    palette: PaletteImpl::MappedPalette(MappedPalette {
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
            IndexOrBits::Index(i) => self.data.set_unchecked(i, state.into()),
            IndexOrBits::Bits(bits) => {
                match bits {
                    1 => {
                        let palette = PaletteImpl::LinearPalette(LinearPalette {
                            bits: 4,
                            //SAFETY: This is fine because we know the palette isn't global (otherwise it wouldn't need to grow in size)
                            values: self.palette.take_vec(),
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
                            PaletteImpl::SingleValuePalette(_) => {
                                std::hint::unreachable_unchecked()
                            }
                            PaletteImpl::LinearPalette(p) => p,
                            PaletteImpl::MappedPalette(_) => std::hint::unreachable_unchecked(),
                            PaletteImpl::Global => std::hint::unreachable_unchecked(),
                        };
                        let mut indices = BTreeMap::new();
                        for i in 0..palette.values.len() {
                            let value = palette.values[i];
                            indices.insert(value, i);
                        }

                        let values = std::mem::take(&mut palette.values);

                        let palette = PaletteImpl::MappedPalette(MappedPalette {
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
                            PaletteImpl::SingleValuePalette(_) => {
                                std::hint::unreachable_unchecked()
                            }
                            PaletteImpl::LinearPalette(_) => std::hint::unreachable_unchecked(),
                            PaletteImpl::MappedPalette(p) => p,
                            PaletteImpl::Global => std::hint::unreachable_unchecked(),
                        };
                        palette.inner.bits = bits;
                    }
                    9 => {
                        let palette = PaletteImpl::<S>::Global;
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

enum PaletteImpl<S> {
    SingleValuePalette(SingleValuePalette<S>),
    LinearPalette(LinearPalette<S>),
    MappedPalette(MappedPalette<S>),
    Global,
}

impl<S: State> PaletteImpl<S> {
    fn index(&mut self, state: S) -> IndexOrBits {
        match self {
            PaletteImpl::SingleValuePalette(this) => this.index(state),
            PaletteImpl::LinearPalette(this) => this.index(state),
            PaletteImpl::MappedPalette(this) => this.index(state),
            PaletteImpl::Global => IndexOrBits::Index((state.into() as u64) as usize),
        }
    }
    fn state(&self, index: usize) -> S {
        match self {
            PaletteImpl::SingleValuePalette(this) => this.state(index),
            PaletteImpl::LinearPalette(this) => this.state(index),
            PaletteImpl::MappedPalette(this) => this.state(index),
            PaletteImpl::Global => S::from(index),
        }
    }

    /// # Safety
    /// This is only safe if self isn't the varian't `Global`
    unsafe fn take_vec(&mut self) -> Vec<S> {
        match self {
            PaletteImpl::SingleValuePalette(this) => (*this).into(),
            PaletteImpl::LinearPalette(this) => std::mem::take(&mut this.values),
            PaletteImpl::MappedPalette(this) => std::mem::take(&mut this.inner.values),
            PaletteImpl::Global => std::hint::unreachable_unchecked(),
        }
    }
}

// S = block state
pub trait Palette<S>: Into<Vec<S>> {
    fn index(&mut self, state: S) -> IndexOrBits;
    fn state(&self, index: usize) -> S;
}

// TODO: Rename?
pub enum IndexOrBits {
    Index(usize),
    Bits(usize),
}

#[derive(Copy, Clone)]
pub struct SingleValuePalette<S> {
    pub(crate) value: S,
}

impl<S> Into<Vec<S>> for SingleValuePalette<S> {
    fn into(self) -> Vec<S> {
        vec![self.value]
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
                    return IndexOrBits::Index(i);
                }
            }
        }
        let len = self.values.len();
        if self.values.len() < 16 {
            // 2^4 = 16
            self.values.push(state);
            IndexOrBits::Index(len)
        } else {
            IndexOrBits::Bits(5)
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
            Some(v) => IndexOrBits::Index(*v),
            None => {
                let initial_len = self.inner.values.len();
                if self.inner.values.capacity() - initial_len > 0 {
                    self.inner.values.push(state);
                    self.indices.insert(state, self.inner.values.len());
                    IndexOrBits::Index(initial_len)
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
