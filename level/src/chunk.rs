use std::{
    mem::{ManuallyDrop, MaybeUninit},
};

use miners::encoding::{Decode, Encode};

use crate::{
    containers::{half_byte_array, ByteArray, ByteArrayMut, __private::HalfByteArrayMut},
    half_byte_array_mut,
};

#[inline]
const fn get_bit_as_bool(bitmask: u16, n: u8) -> bool {
    let n = n & 0b1111;
    let v = ((bitmask & (1 << n) as u16) >> n as u16) as u8;
    // Here we check that v is 0 or 1, if it is any other value then turing it into a bool is UB
    debug_assert!(v == 0 || v == 1);

    // SAFETY: This is safe because we know (bitmask & (1 << n)) >> n) will always be 1 or 0
    unsafe { std::mem::transmute(n) }
}

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0<'a> {
    buf: *mut u8,
    size: usize,
    bitmask: u16,
    add: u16,
    sky_light: u16,
    sections: [MaybeUninit<ChunkSection0<'a>>; 16],
}

impl<'a> ChunkColumn0<'a> {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection0<'a>> {
        if get_bit_as_bool(self.bitmask, section as u8) {
            // SAFETY: We know the section will be initialised because we checked the bitmask
            Some(unsafe { self.sections[section].assume_init_ref() })
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection0<'a>> {
        if get_bit_as_bool(self.bitmask, section as u8) {
            // SAFETY: We know the section will be initialised because we checked the bitmask
            Some(unsafe { self.sections[section].assume_init_mut() })
        } else {
            None
        }
    }

    /// Gets the add bitmask
    pub fn add(&self) -> u16 {
        self.add
    }

    /// Gets the sky light bitmask
    pub fn sky_light(&self) -> u16 {
        self.sky_light
    }

    pub fn sky_light_of_section(&self, section: usize) -> Option<&half_byte_array_mut!('a, 4096)>{
        if let Some(s) = self.section(section) {
            // SAFETY: This is safe because we provide the correct bitmap and section index
            unsafe { s.sky_light(self.sky_light, section as u8) }
        } else {
            None
        }
    }

    pub fn sky_light_of_section_mut(&mut self, section: usize) -> Option<&mut half_byte_array_mut!('a, 4096)>{
        let sky_light = self.sky_light;
        if let Some(s) = self.section_mut(section) {
            // SAFETY: This is safe because we provide the correct bitmap and section index
            unsafe { s.sky_light_mut(sky_light, section as u8) }
        } else {
            None
        }
    }

    pub fn add_of_section(&self, section: usize) -> Option<&half_byte_array_mut!('a, 4096)>{
        if let Some(s) = self.section(section) {
            // SAFETY: This is safe because we provide the correct bitmap and section index
            unsafe { s.add(self.add, section as u8) }
        } else {
            None
        }
    }

    pub fn add_of_section_mut(&mut self, section: usize) -> Option<&mut half_byte_array_mut!('a, 4096)>{
        let add = self.add;
        if let Some(s) = self.section_mut(section) {
            // SAFETY: This is safe because we provide the correct bitmap and section index
            unsafe { s.add_mut(add, section as u8) }
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
            let exists: bool = get_bit_as_bool(bitmask, i);
            let add: bool = get_bit_as_bool(bitmask, i);
            let sky_light: bool = get_bit_as_bool(bitmask, i);
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
            let exists = get_bit_as_bool(bitmask, i);
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

                let add: bool = get_bit_as_bool(bitmask, i);
                let sky_light: bool = get_bit_as_bool(bitmask, i);        

                let section = ChunkSection0 {
                    blocks: unsafe {
                        ByteArrayMut::from(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().blocks,
                        ))
                    },
                    metadata: unsafe {
                        HalfByteArrayMut::from(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().metadata,
                        ))
                    },
                    light: unsafe {
                        HalfByteArrayMut::from(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().light,
                        ))
                    },
                    sky_light: if sky_light {
                        MaybeUninit::new(
                            unsafe {
                                HalfByteArrayMut::from(new_field(
                                    &mut p,
                                    sections[i as usize].decode.assume_init().sky_light.assume_init(),
                                ))
                            }
                        )
                    } else {
                        MaybeUninit::uninit()
                    },
                    add: if add {
                        MaybeUninit::new(
                            unsafe {
                                HalfByteArrayMut::from(new_field(
                                    &mut p,
                                    sections[i as usize].decode.assume_init().add.assume_init(),
                                ))
                            }
                        )
                    } else {
                        MaybeUninit::uninit()
                    },
                    biomes: unsafe {
                        HalfByteArrayMut::from(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().biomes,
                        ))
                    },
                };
                sections[i as usize] = ChunkSection {
                    mutable: ManuallyDrop::new(section),
                };
            }
        }
        // SAFETY: This is fine because ChunkSection0 and ChunkSection0Decode have the same type layout
        Ok(Self { buf: data, size, bitmask, add, sky_light, sections: unsafe { std::mem::transmute(sections) } })
    }
}

#[repr(C)]
pub struct ChunkSection0<'a> {
    pub blocks: ByteArrayMut<'a, 4096>,
    pub metadata: half_byte_array_mut!('a, 4096),
    pub light: half_byte_array_mut!('a, 4096),
    pub sky_light: MaybeUninit<half_byte_array_mut!('a, 4096)>,
    pub add: MaybeUninit<half_byte_array_mut!('a, 4096)>,
    pub biomes: half_byte_array_mut!('a, 4096),
}

impl<'a> ChunkSection0<'a> {
    /// # Safety
    /// This method is only safe if you know you provide the right bitmask and it has not been tampered with.
    /// You must also verify that `i` corresponds to this chunk section.
    pub unsafe fn sky_light(&self, bitmask: u16, i: u8) -> Option<&half_byte_array_mut!('a, 4096)> {
        if get_bit_as_bool(bitmask, i) {
            Some(self.sky_light.assume_init_ref())
        } else {
            None
        }
    }

    /// # Safety
    /// See `get_sky_light`.
    pub unsafe fn sky_light_mut(&mut self, bitmask: u16, i: u8) -> Option<&mut half_byte_array_mut!('a, 4096)> {
        if get_bit_as_bool(bitmask, i) {
            Some(self.sky_light.assume_init_mut())
        } else {
            None
        }
    }

    /// # Safety
    /// See `get_sky_light`.
    pub unsafe fn add(&self, bitmask: u16, i: u8) -> Option<&half_byte_array_mut!('a, 4096)> {
        if get_bit_as_bool(bitmask, i) {
            Some(self.add.assume_init_ref())
        } else {
            None
        }
    }
    
    /// # Safety
    /// See `get_sky_light`.
    pub unsafe fn add_mut(&mut self, bitmask: u16, i: u8) -> Option<&mut half_byte_array_mut!('a, 4096)> {
        if get_bit_as_bool(bitmask, i) {
            Some(self.add.assume_init_mut())
        } else {
            None
        }
    }
}

/// This is only used internally for Decoding
#[derive(Copy, Clone)]
#[repr(C)]
struct ChunkSection0Decode<'a> {
    pub blocks: ByteArray<'a, 4096>,
    pub metadata: half_byte_array!('a, 4096),
    pub light: half_byte_array!('a, 4096),
    pub sky_light: MaybeUninit<half_byte_array!('a, 4096)>,
    pub add: MaybeUninit<half_byte_array!('a, 4096)>,
    pub biomes: half_byte_array!('a, 4096),
}

impl ChunkSection0Decode<'_> {
    pub fn from_reader(
        cursor: &mut std::io::Cursor<&'_ [u8]>,
        sky_light: bool,
        add: bool,
    ) -> miners::encoding::decode::Result<Self> {
        Ok(Self {
            blocks: ByteArray::decode(cursor)?,
            metadata: half_byte_array!(decode)(cursor)?,
            light: half_byte_array!(decode)(cursor)?,
            sky_light: if sky_light {
                MaybeUninit::new(half_byte_array!(decode)(cursor)?)
            } else {
                MaybeUninit::uninit()
            },
            add: if add {
                MaybeUninit::new(half_byte_array!(decode)(cursor)?)
            } else {
                MaybeUninit::uninit()
            },
            biomes: half_byte_array!(decode)(cursor)?,
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
    use super::{get_bit_as_bool, ChunkColumn0};

    #[test]
    fn t_get_bit_as_bool() {
        let bitmask = 0b1010101010101010u16;
        for i in 0u8..16 {
            let bit = get_bit_as_bool(bitmask, i);
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
            let exists = get_bit_as_bool(bitmask, i);
            let add = get_bit_as_bool(add, i);
            print!("{:b}", add as u8);
            let sky_light = get_bit_as_bool(bitmask, i);
            if exists {
                for i in 0u16..4096 {
                    data.push(i as u8);
                    data.push(((i & 0xf0) >> 8 ) as u8)
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

        let _chunk = ChunkColumn0::from_reader(&mut std::io::Cursor::new(&data), bitmask, add, sky_light).unwrap();
    }
}