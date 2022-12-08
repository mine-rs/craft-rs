use std::{
    mem::{ManuallyDrop, MaybeUninit},
};

use miners::encoding::{Decode, Encode};

use crate::{
    containers::{half_byte_array, ByteArray, ByteArrayMut},
    half_byte_array_mut,
};

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0 {
    bitmask: u16,
    add: u16,
    sky_light: u16,
    sections: [MaybeUninit<ChunkSection0<'static>>; 16],
}

#[inline]
const fn get_bit_as_bool(bitmask: u16, n: u8) -> bool {
    // SAFETY: This is safe because we know (bitmask & (1 << n)) >> n) will always be 1 or 0
    let n = n & 0b1111;

    ((bitmask & (1 << n)) >> n) != 0
}

impl ChunkColumn0 {
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
        println!("{MINIMUM_SECTION_SIZE}");
        let size = (nsections * MINIMUM_SECTION_SIZE) + (nsky_light * 2048) + (nadd * 2048);
        println!("{size}");
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
                unsafe fn new_field<'a, const N: usize, T: Into<&'a [u8; N]>>(
                    dst: &mut *mut u8,
                    src: T,
                ) -> *mut [u8; N] {
                    let p = dst.cast() as *mut [u8; N];
                    p.copy_from_nonoverlapping(Into::<&[u8; N]>::into(src), 1);
                    *dst = dst.add(N);
                    p
                }

                let add: bool = get_bit_as_bool(bitmask, i);
                let sky_light: bool = get_bit_as_bool(bitmask, i);        

                let section = ChunkSection0 {
                    blocks: unsafe {
                        std::mem::transmute(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().blocks,
                        ))
                    },
                    metadata: unsafe {
                        std::mem::transmute(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().metadata,
                        ))
                    },
                    light: unsafe {
                        std::mem::transmute(new_field(
                            &mut p,
                            sections[i as usize].decode.assume_init().light,
                        ))
                    },
                    sky_light: if sky_light {
                        MaybeUninit::new(
                            unsafe {
                                std::mem::transmute(new_field(
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
                                std::mem::transmute(new_field(
                                    &mut p,
                                    sections[i as usize].decode.assume_init().add.assume_init(),
                                ))
                            }
                        )
                    } else {
                        MaybeUninit::uninit()
                    },
                    biomes: unsafe {
                        std::mem::transmute(new_field(
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
        Ok(Self { bitmask, add, sky_light, sections: unsafe { std::mem::transmute(sections) } })
    }
}

struct ChunkSection0<'a> {
    pub blocks: ByteArrayMut<'a, 4096>,
    pub metadata: half_byte_array_mut!('a, 4096),
    pub light: half_byte_array_mut!('a, 4096),
    pub sky_light: MaybeUninit<half_byte_array_mut!('a, 4096)>,
    pub add: MaybeUninit<half_byte_array_mut!('a, 4096)>,
    pub biomes: half_byte_array_mut!('a, 4096),
}

/// This is only used internally for Decoding
#[derive(Copy, Clone)]
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

        let chunk = ChunkColumn0::from_reader(&mut std::io::Cursor::new(&data), bitmask, add, sky_light).unwrap();
    }
}