use std::mem::{ManuallyDrop, MaybeUninit};

use miners::encoding::{Decode, Encode};

use crate::{
    containers::{HalfByteArray, ByteArray},
};

#[inline]
const fn bit_at(val: u16, idx: u8) -> bool {
    debug_assert!(!(idx > 0x0f));
    (val >> idx) & 0b1 != 0
}

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0<'a> {
    buf: *mut u8,
    size: usize,
    bitmask: u16,
    sections: [MaybeUninit<ChunkSection0<'a>>; 16],
}


impl<'a> ChunkColumn0<'a> {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection0<'a>> {
        if bit_at(self.bitmask, section as u8) {
            // SAFETY: We know the section will be initialised because we checked the bitmask
            Some(unsafe { self.sections[section].assume_init_ref() })
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection0<'a>> {
        if bit_at(self.bitmask, section as u8) {
            // SAFETY: We know the section will be initialised because we checked the bitmask
            Some(unsafe { self.sections[section].assume_init_mut() })
        } else {
            None
        }
    }
}


impl<'a> Drop for ChunkColumn0<'a> {
    fn drop(&mut self) {
        let vec = unsafe { Vec::<u8>::from_raw_parts(self.buf, self.size, self.size) };
        drop(vec)
    }
}

impl ChunkColumn0<'_> {
    pub fn from_reader(
        cursor: &mut std::io::Cursor<&'_ [u8]>,
        bitmask: u16,
        add: u16,
        sky_light: u16,
    ) -> miners::encoding::decode::Result<Self> {
        union ChunkSection<'a> {
            decode: MaybeUninit<ChunkSection0Decode<'a>>,
            // wrap it in ManuallyDrop even though it does not need dropping because it can't implement `Copy`
            mutable: ManuallyDrop<ChunkSection0<'a>>,
        }
        // SAFETY: We have to do this weird thing because ChunkSection doesn't implement Copy, it is completely safe though as the union fields have the same layout
        let mut sections: [ChunkSection; 16] =
            unsafe { std::mem::transmute([MaybeUninit::<ChunkSection0Decode>::uninit(); 16]) };

        let mut nsections = 0;
        let mut nadd = 0;
        let mut nsky_light = 0;
        // create sections according to the bitmask
        for i in 0u8..16 {
            let exists: bool = bit_at(bitmask, i);
            let add: bool = bit_at(add, i);
            let sky_light: bool = bit_at(sky_light, i);
            if exists {
                sections[i as usize] = ChunkSection {
                    decode: MaybeUninit::new(ChunkSection0Decode::from_reader(
                        cursor, sky_light, add,
                    )?),
                };
                if add {
                    nadd += 1;
                }
                if sky_light {
                    nsky_light += 1
                }
                nsections += 1;
            }
        }
        const MINIMUM_SECTION_SIZE: usize = 4096 + (3 * 2048);
        let size = (nsections * MINIMUM_SECTION_SIZE) + (nsky_light * 2048) + (nadd * 2048);
        let mut vec = Vec::<u8>::with_capacity(size);
        let data = vec.as_mut_ptr();
        std::mem::forget(vec);

        // loop through the sections
        let mut p = data;
        for i in 0u8..16 {
            let exists = bit_at(bitmask, i);
            if exists {
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

                let add: bool = bit_at(add, i);
                let sky_light: bool = bit_at(sky_light, i);

                let section = ChunkSection0 {
                    // SAFETY: This is fine because we know dst (p) was properly allocated and there are no references to it.
                    // (a pointer is not a reference)
                    blocks: unsafe {
                        (new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().blocks,
                        )).into()
                    },
                    // SAFETY: See safety comment for `blocks`
                    metadata: unsafe {
                        (new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().metadata,
                        )).into()
                    },
                    // SAFETY: See safety comment for `blocks`
                    light: unsafe {
                        (new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().light,
                        )).into()
                    },
                    sky_light: if sky_light {
                        Some(
                            // SAFETY: See safety comment for `blocks`
                            unsafe {
                                (new_field(
                                    &mut p,
                                    sections[i as usize]
                                        .decode
                                        .assume_init()
                                        .sky_light.unwrap_unchecked(),
                                )).into()
                            },
                        )
                    } else {
                        None
                    },
                    add: if add {
                        Some(
                            // SAFETY: See safety comment for `blocks`
                            unsafe {
                                (new_field(
                                    &mut p,
                                    sections[i as usize].decode.assume_init().add.unwrap_unchecked(),
                                )).into()
                            },
                        )
                    } else {
                        None
                    },
                    // SAFETY: See safety comment for `blocks`
                    biomes: unsafe {
                        (new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().biomes,
                        )).into()
                    },
                };
                sections[i as usize] = ChunkSection {
                    mutable: ManuallyDrop::new(section),
                };
            }
        }
        // SAFETY: This is fine because ChunkSection0 and ChunkSection0Decode have the same type layout
        Ok(Self {
            buf: data,
            size,
            bitmask,
            // SAFETY: This is fine because we know both union fields have the exact same layout.
            sections: unsafe { std::mem::transmute(sections) },
        })
    }
}

#[repr(C)]
pub struct ChunkSection0<'a> {
    pub blocks: &'a mut ByteArray<4096>,
    pub metadata: &'a mut HalfByteArray<2048>,
    pub light: &'a mut HalfByteArray<2048>,
    pub sky_light: Option<&'a mut HalfByteArray<2048>>,
    pub add: Option<&'a mut HalfByteArray<2048>>,
    pub biomes: &'a mut HalfByteArray<2048>,
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
    pub biomes: &'a HalfByteArray<2048>,
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
            biomes: <&HalfByteArray<2048>>::decode(cursor)?,
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
    fn t_get_bit_as_bool() {
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
            for i in 0u16..2048 {
                data.push(i as u8)
            }
        }

        let _chunk =
            ChunkColumn0::from_reader(&mut std::io::Cursor::new(&data), bitmask, add, sky_light)
                .unwrap();
    }
}
