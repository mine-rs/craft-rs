use std::{mem::MaybeUninit, ptr::NonNull};

use miners::encoding::{Decode, Encode};

use crate::containers::{ByteArray, HalfByteArray};

#[inline]
const fn bit_at(val: u16, idx: u8) -> bool {
    debug_assert!((idx <= 0x0f));
    (val >> idx) & 0b1 != 0
}

const fn section_size_pv0(sky_light: bool, add: bool) -> usize {
    4096 + (2 * 2048) + 256 + if sky_light {
        2048
    } else {
        0
    } + if add {
        2048
    } else {
        0
    }
} 
/// # Safety
/// dst should be allocated properly, initialised, and no other references should point to it
unsafe fn assign_ref<'a, const N: usize, T: From<&'a mut [u8; N]>>(
    dst: &mut *mut u8,
) -> T {
    let p = dst.cast() as *mut [u8; N];
    *dst = dst.add(N);
    (&mut *p).into()
}
/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0<'a> {
    buf: Option<NonNull<u8>>,
    size: usize,
    sections: [Option<ChunkSection0<'a>>; 16],
}

impl Default for ChunkColumn0<'_> {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! insert_opt_field {
    ($i:ident, $f:ident) => {
        pub fn $f(&mut self, section: usize) {
            let p: *mut u8 = self.reallocate(2048).as_mut_ptr().cast();
            // zero-initialise the buffer
            // SAFETY: This is fine because we know `p` has been allocated for `size`.
            unsafe { p.write_bytes(0, 2048) };
            if let Some(section)  = &mut self.sections[section] {
                assert!(section.$i.is_none());
                section.$i = unsafe { Some((&mut *(p.cast() as *mut [u8; 2048])).into()) }
            } else {
                panic!("chunk section does not exist")
            }
        }
    };
}

impl ChunkColumn0<'_> {
    /// Constructs a new `ChunkColumn0`, doesn't allocate.
    pub fn new() -> Self {
        Self {
            buf: None,
            size: 0,
            sections: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            ],
        }
    }

    /// Creates a new section and zero-initialises all of the buffers
    pub fn insert_section(&mut self, section: usize, add: bool, sky_light: bool) {
        assert!(self.sections[section].is_none()); 
        let size = section_size_pv0(sky_light, add);
        let mut p: *mut u8 = self.reallocate(size).as_mut_ptr().cast();
        // zero-initialise the buffer
        // SAFETY: This is fine because we know `p` has been allocated for `size`.
        unsafe { p.write_bytes(0, size) };
        self.sections[section] = Some(ChunkSection0 {
            blocks: unsafe { assign_ref(&mut p) },
            metadata: unsafe { assign_ref(&mut p) },
            light: unsafe { assign_ref(&mut p) },
            sky_light: if sky_light {
                Some(unsafe { assign_ref(&mut p) })
            } else {
                None
            },
            add: if add {
                Some(unsafe { assign_ref(&mut p) })
            } else {
                None
            },
            biomes: unsafe { assign_ref(&mut p) },
        })
    }

    insert_opt_field!(sky_light, insert_sky_light);
    insert_opt_field!(add, insert_add);

    /// Reallocates the internal buffer extending it with `extend` and returning a reference to the part of the buffer that was just added.
    pub fn reallocate<'a>(&'a mut self, extend: usize) -> &mut [MaybeUninit<u8>] {
        assert!(extend != 0);
        
        let mut vec = Vec::<u8>::with_capacity(self.size + extend);
        let new = vec.as_mut_ptr();
        std::mem::forget(vec);

        let mut sections: [Option<ChunkSection0>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];

        if let Some(buf) = self.buf {
            // SAFETY: This is fine because we know self.buf is initialised and new and self.buf don't overlap.
            unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), new, self.size) };
            let mut p = new;

            for i in 0..16 {
                if let Some(old_section) = &self.sections[i] {
                    let section = Some(ChunkSection0 {
                        // SAFETY: We know dst is allocated, initialised and no other references point to it so this is fine.
                        blocks: unsafe { assign_ref(&mut p) },
                        // SAFETY: See safety comment for `blocks`.
                        metadata: unsafe { assign_ref(&mut p) },
                        // SAFETY: See safety comment for `blocks`.
                        light: unsafe { assign_ref(&mut p) },
                        // SAFETY: See safety comment for `blocks`.
                        sky_light: if old_section.sky_light.is_some() {
                            // SAFETY: See safety comment for `blocks`.
                            Some(unsafe { assign_ref(&mut p) })
                        } else {
                            None
                        },
                        add: if old_section.add.is_some() {
                            // SAFETY: See safety comment for `blocks`.
                            Some(unsafe { assign_ref(&mut p) })
                        } else {
                            None
                        },
                        // SAFETY: See safety comment for `blocks`.
                        biomes: unsafe { assign_ref(&mut p) },
                    });
                    sections[i] = section;
                }
            }
        }
        let this = Self {
            // SAFETY: This is safe because we know new isn't a null pointer.
            buf: unsafe { Some(NonNull::new_unchecked(new)) },
            size: self.size + extend,
            sections,
        };

        let old_size = self.size;
        *self = this;

        // SAFETY: This is sound because we're using `MaybeUninit<u8>` and we know the memory has been allocated.
        unsafe { std::slice::from_raw_parts_mut(new.add(old_size).cast(), extend) }
    }
}

impl<'a> ChunkColumn0<'a> {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection0<'a>> {
        if let Some(ref section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection0<'a>> {
        if let Some(ref mut section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }
}

impl<'a> Drop for ChunkColumn0<'a> {
    fn drop(&mut self) {
        if let Some(buf) = self.buf {
            // SAFETY: This is fine because the buffer was allocated with `Vec`.
            let vec = unsafe { Vec::<u8>::from_raw_parts(buf.as_ptr(), self.size, self.size) };
            drop(vec)
        }
    }
}

impl<'a> ChunkColumn0<'a> {
    pub fn from_reader(
        cursor: &mut std::io::Cursor<&'a [u8]>,
        bitmask: u16,
        add: u16,
        sky_light: u16,
    ) -> miners::encoding::decode::Result<Self> {
        let mut decode_sections: [Option<ChunkSection0Decode>; 16] = [None; 16];

        let mut size = 0;
        // create sections according to the bitmask
        for i in 0u8..16 {
            let exists: bool = bit_at(bitmask, i);
            if exists {
                let add: bool = bit_at(add, i);
                let sky_light: bool = bit_at(sky_light, i);
                decode_sections[i as usize] =
                    Some(ChunkSection0Decode::from_reader(cursor, sky_light, add)?);
                size += section_size_pv0(sky_light, add);
            }
        }

        let mut vec = Vec::<u8>::with_capacity(size);
        let data = vec.as_mut_ptr();
        std::mem::forget(vec);

        let mut sections: [Option<ChunkSection0>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];

        // loop through the sections
        let mut p = data;
        for i in 0u8..16 {
            if let Some(section) = decode_sections[i as usize] {
                #[inline]
                // TODO: come up with a better name
                /// # Safety
                /// dst should be allocated properly and no other references should point to it
                unsafe fn new_field<'a, const N: usize, T: Into<&'a [u8; N]>>(
                    dst: &mut *mut u8,
                    src: T,
                ) -> &'a mut [u8; N] {
                    let p = dst.cast() as *mut [u8; N];
                    p.copy_from_nonoverlapping(Into::<&[u8; N]>::into(src), 1);
                    *dst = dst.add(N);
                    &mut *p
                }

                let section = ChunkSection0 {
                    // SAFETY: This is fine because we know dst (p) was properly allocated and there are no references to it.
                    // (a pointer is not a reference)
                    blocks: unsafe { (new_field(&mut p, section.blocks)).into() },
                    // SAFETY: See safety comment for `blocks`
                    metadata: unsafe { (new_field(&mut p, section.metadata)).into() },
                    // SAFETY: See safety comment for `blocks`
                    light: unsafe { (new_field(&mut p, section.light)).into() },
                    sky_light: if let Some(v) = section.sky_light {
                        Some(
                            // SAFETY: See safety comment for `blocks`
                            unsafe { (new_field(&mut p, v)).into() },
                        )
                    } else {
                        None
                    },
                    add: if let Some(v) = section.add {
                        Some(
                            // SAFETY: See safety comment for `blocks`
                            unsafe { (new_field(&mut p, v)).into() },
                        )
                    } else {
                        None
                    },
                    // SAFETY: See safety comment for `blocks`
                    biomes: unsafe { (new_field(&mut p, section.biomes)).into() },
                };
                sections[i as usize] = Some(section);
            }
        }
        // SAFETY: This is fine because ChunkSection0 and ChunkSection0Decode have the same type layout
        Ok(Self {
            // SAFETY: This is fine because we know data is not null
            buf: unsafe { Some(NonNull::new_unchecked(data)) },
            size,
            // SAFETY: This is fine because we know both union fields have the exact same layout.
            sections,
        })
    }
}

#[repr(C)]
pub struct ChunkSection0<'a> {
    pub(crate) blocks: &'a mut ByteArray<4096>,
    pub(crate) metadata: &'a mut HalfByteArray<2048>,
    pub(crate) light: &'a mut HalfByteArray<2048>,
    pub(crate) sky_light: Option<&'a mut HalfByteArray<2048>>,
    pub(crate) add: Option<&'a mut HalfByteArray<2048>>,
    pub(crate) biomes: &'a mut ByteArray<256>,
}

macro_rules! getter {
    ($i:ident, $m:ident, $t:ty) => {
        pub fn $i(&self ) -> &&mut$t {
            &self.$i
        }

        pub fn $m(&mut self ) -> &mut $t {
            self.$i
        }    
    };
}

macro_rules! opt_getter {
    ($i:ident, $m:ident, $t:ty) => {
        pub fn $i(&self ) -> &Option<&mut$t> {
            &self.$i
        }

        pub fn $m(&mut self ) -> Option<&mut $t> {
            if let Some(v) = self.$i.as_mut() {
                Some(v)
            } else {
                None
            }
        }    
    };
}

impl ChunkSection0<'_> {
    getter!(blocks, blocks_mut, ByteArray<4096>);
    getter!(metadata, metadata_mut, HalfByteArray<2048>);
    getter!(light, light_mut, HalfByteArray<2048>);
    opt_getter!(sky_light, sky_light_mut, HalfByteArray<2048>);
    opt_getter!(add, add_mut, HalfByteArray<2048>);
    getter!(biomes, biomes_mut, ByteArray<256>);
}

/// This is only used internally for Decoding
#[derive(Copy, Clone)]
#[repr(C)]
struct ChunkSection0Decode<'a> {
    pub blocks: &'a ByteArray<4096>,
    pub metadata: &'a HalfByteArray<2048>,
    pub light: &'a HalfByteArray<2048>,
    pub sky_light: Option<&'a HalfByteArray<2048>>,
    pub add: Option<&'a HalfByteArray<2048>>,
    pub biomes: &'a ByteArray<256>,
}

impl<'a> ChunkSection0Decode<'a> {
    pub fn from_reader(
        cursor: &mut std::io::Cursor<&'a [u8]>,
        sky_light: bool,
        add: bool,
    ) -> miners::encoding::decode::Result<Self> {
        Ok(Self {
            blocks: <&ByteArray<4096>>::decode(cursor)?,
            metadata: <&HalfByteArray<2048>>::decode(cursor)?,
            light: <&HalfByteArray<2048>>::decode(cursor)?,
            sky_light: if sky_light {
                Some(<&HalfByteArray<2048>>::decode(cursor)?)
            } else {
                None
            },
            add: if add {
                Some(<&HalfByteArray<2048>>::decode(cursor)?)
            } else {
                None
            },
            biomes: <&ByteArray<256>>::decode(cursor)?,
        })
    }
}

/// A 16 * 16 * 16 section of a chunk.
pub struct ChunkSection<S, B> {
    pub block_count: u16,
    pub states: S,
    pub biomes: B,
}

impl<S: Encode, B: Encode> Encode for ChunkSection<S, B> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        self.block_count.encode(writer)?;
        self.states.encode(writer)?;
        self.biomes.encode(writer)
    }
}

impl<S: for<'a> Decode<'a>, B: for<'a> Decode<'a>> Decode<'_> for ChunkSection<S, B> {
    fn decode(cursor: &mut std::io::Cursor<&'_ [u8]>) -> miners::encoding::decode::Result<Self> {
        Ok(Self {
            block_count: u16::decode(cursor)?,
            states: S::decode(cursor)?,
            biomes: B::decode(cursor)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{bit_at, ChunkColumn0};

    #[test]
    fn t_bit_at() {
        let bitmask = 0b1010101010101010u16;
        for i in 0u8..16 {
            let bit = bit_at(bitmask, i);
            if i % 2 == 0 && bit {
                panic!("{i}th bit should be 0!")
            }
            if i % 2 == 1 && !bit {
                panic!("bit {i} should be 1!")
            }
        }
    }

    #[test]
    fn pv0() {
        // first we generate the data
        //TODO: use real data from minecraft
        let bitmask = 0b1011001110110011u16;
        let add = 0b1001001010010010u16;
        let sky_light = 0b0010000100100001u16;

        let mut data = Vec::<u8>::new();

        for i in 0u8..16 {
            let exists = bit_at(bitmask, i);
            let add = bit_at(add, i);
            print!("{:b}", add as u8);
            let sky_light = bit_at(bitmask, i);
            if exists {
                for i in 0u16..4096 {
                    data.push(i as u8);
                    data.push(((i & 0xf0) >> 8) as u8)
                }
            }
            if add {
                for i in 0u16..2048 {
                    data.push(i as u8)
                }
            }
            if sky_light {
                for i in 0u16..2048 {
                    data.push(i as u8)
                }
            }
            for i in 0u8..=255 {
                data.push(i)
            }
        }

        let mut chunk =
            ChunkColumn0::from_reader(&mut std::io::Cursor::new(&data), bitmask, add, sky_light)
                .unwrap();
        chunk.insert_section(6, false, false)
    }
}
