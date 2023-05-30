use std::{mem::MaybeUninit, ptr::NonNull};

use miners::{
    encoding::{Decode, Encode},
    nbt::List,
};

use crate::containers::{BlockArray49, ByteArray, HalfByteArray, Block49, ReadContainer};

mod util {
    #[inline]
    pub const fn bit_at(val: u16, idx: u8) -> bool {
        debug_assert!((idx <= 0x0f));
        (val >> idx) & 0b1 != 0
    }

    /// # Safety
    /// `src` should be allocated properly, initialised, and no other references should point to it
    pub unsafe fn assign_ref<'a, const N: usize, T>(src: &mut *mut u8) -> NonNull<T> {
        let p = src.cast() as *mut [u8; N];
        *src = src.add(N);
        NonNull::new_unchecked(p.cast())
    }

    /// Creates a buffer with the supplied size.
    pub fn create_buffer(size: usize) -> *mut u8 {
        let mut vec = Vec::<u8>::with_capacity(size);
        let data = vec.as_mut_ptr();
        std::mem::forget(vec);
        data
    }

    macro_rules! getter {
        ($i:ident, $m:ident, $t:ty) => {
            pub fn $i<'a>(&'a self) -> &'a $t {
                // Safety: this is safe because the pointers are valid for the lifetime of self
                unsafe { self.$i.as_ref() }
            }

            pub fn $m<'a>(&'a mut self) -> &'a mut $t {
                // Safety: this is safe because the pointers are valid for the lifetime of self
                unsafe { self.$i.as_mut() }
            }
        };
    }

    macro_rules! opt_getter {
        ($i:ident, $m:ident, $t:ty) => {
            pub fn $i(&self) -> Option<&$t> {
                if let Some(v) = self.$i.as_ref() {
                    // Safety: this is safe because the pointers are valid for the lifetime of self
                    Some(unsafe { v.as_ref() })
                } else {
                    None
                }
            }

            pub fn $m(&mut self) -> Option<&mut $t> {
                if let Some(v) = self.$i.as_mut() {
                    // Safety: this is safe because the pointers are valid for the lifetime of self
                    Some(unsafe { v.as_mut() })
                } else {
                    None
                }
            }
        };
    }

    /// Used to generate reallocate functions for the 0 and 49 protocol versions.
    macro_rules! reallocate_fn {
        ($section_t:ty, | $self:ident, $p:ident, $section:ident | $e:expr) => {
            /// Reallocates the internal buffer extending it with `extend` and returning a reference to the part of the buffer that was just added.
            pub fn reallocate<'a>(&'a mut $self, extend: usize) -> &'a mut [MaybeUninit<u8>] {
                assert!(extend != 0);

                let new = $crate::chunk::util::create_buffer($self.size + extend);

                let mut sections: [Option<$section_t>; 16] = [
                    None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                    None, None,
                ];

                if let Some(buf) = $self.buf {
                    // SAFETY: This is fine because we know self.buf is initialised and new and self.buf don't overlap.
                    unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), new, $self.size) };
                    let mut $p = new;

                    for (i, section) in $self.sections.iter().enumerate() {
                        if let Some($section) = section {
                            let section = $e;
                            sections[i] = Some(section)
                        }
                    }
                }

                let this = Self {
                    // SAFETY: This is safe because we know new isn't a null pointer.
                    buf: unsafe { Some(NonNull::new_unchecked(new)) },
                    size: $self.size + extend,
                    skylight: $self.skylight,
                    sections,
                };

                let old_size = $self.size;
                *$self = this;

                // SAFETY: This is sound because we're using `MaybeUninit<u8>` and we know the memory has been allocated.
                unsafe { std::slice::from_raw_parts_mut(new.add(old_size).cast(), extend) }

            }
        };
    }

    macro_rules! from_reader_fn {
        ($section:ty, | $p:ident, $skylight:ident | $e:expr $(, $add:ident, $t:ty)?) => {
            pub fn from_reader(
                cursor: &mut std::io::Cursor<&[u8]>,
                $skylight: bool,
                bitmask: u16,
                $($add: $t)?
            ) -> miners::encoding::decode::Result<Self> {
                let mut size = 0;
                for i in 0u8..16  {
                    let exists: bool = $crate::chunk::util::bit_at(bitmask, i);
                    if exists {
                        $(let $add: bool = util::bit_at($add, i);)?
                        size += Self::section_size($skylight, $($add)?);
                    }
                }

                let data = {
                    let pos = cursor.position() as usize;
                    let slice = cursor
                        .get_ref()
                        .get(pos..pos + size as usize)
                        .ok_or(miners::encoding::decode::Error::UnexpectedEndOfSlice)?;
                    cursor.set_position((pos + size) as u64);
                    debug_assert_eq!(slice.len(), size);
                    slice
                };

                let buf = util::create_buffer(size);
                // Copy the data over to the buffer

                // Safety: This is safe because the buf is properly allocated, and data is properly initialised for the amount of bytes copied over.
                unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf, size) };

                let mut sections: [Option<$section>; 16] = [
                    None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                    None, None,
                ];
                let mut $p = buf;
                for i in 0u8..16 {
                    let exists: bool = $crate::chunk::util::bit_at(bitmask, i);
                    if exists {
                        $(let $add: bool = util::bit_at($add, i);)?
                        // Safety: This is safe because p is properly allocated and there are no other references pointing to it.
                        let section = $e;
                        sections[i as usize] = Some(section)
                    }
                }
                Ok(Self {
                    // Safety: This is safe because buf isn't a null pointer.
                    buf: unsafe { Some(NonNull::new_unchecked(buf)) },
                    size,
                    $skylight,
                    // Safety: The compiler thinks `sections` is bound by the lifetime of the cursor slice, but it isn't because we copied the data over to a new buffer.
                    sections: unsafe { std::mem::transmute(sections) },
                })
            }
        };
    }

    use std::ptr::NonNull;

    pub(super) use from_reader_fn;
    pub(super) use getter;
    pub(super) use opt_getter;
    pub(super) use reallocate_fn;
}

pub struct ChunkColumn49 {
    buf: Option<NonNull<u8>>,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection49>; 16],
}

impl ChunkColumn49 {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection49> {
        if let Some(ref section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection49> {
        if let Some(ref mut section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }
}

impl ChunkColumn49 {
    /// Parses 1.8 anvil chunk nbt data into a `ChunkColumn49`. This function does not take an entire region file as input, but one of the chunks contained within.
    pub fn from_nbt(nbt: miners::nbt::Compound, skylight: bool) -> Option<Self> {
        //TODO: Fix pointer nonsense
        let nbt = nbt.get("Level")?.as_compound()?;

        let sections_data = {
            let List::Compound(sections) = nbt.get("Sections")?.as_list()? else {
                return None;
            };
            sections
        };

        let mut sections: [Option<ChunkSection49>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        let size: usize = Self::section_size(skylight) * sections_data.len();

        let mut buf = Vec::with_capacity(size);

        let mut p = buf.as_mut_ptr();
        for section in sections_data.iter() {
            //let mut p_offset: usize = 0;
            unsafe {
                let light = section.get("BlockLight")?.as_byte_array()?;
                if light.len() != 2048 {
                    return None;
                }
                buf.extend_from_slice(light);
                //p_offset += 2048;
                let blocks = section.get("Blocks")?.as_byte_array()?;
                if blocks.len() != 4096 {
                    return None;
                }
                let metadata = section.get("Data")?.as_byte_array()?;
                if metadata.len() != 2048 {
                    return None;
                }
                let metadata = <&HalfByteArray<2048>>::from(std::mem::transmute::<*const u8, &[u8; 2048]>(metadata.as_ptr()));
                for i in 0..4096 {
                    let block = Block49::new(blocks[i] as u16, metadata.get(i));
                    buf.extend_from_slice(block.as_slice());
                    //p_offset += 2;
                }
                if skylight {
                    let skylight = section.get("SkyLight")?.as_byte_array()?;
                    if skylight.len() != 2048 {
                        return None;
                    }
                    buf.extend_from_slice(&skylight);
                    //p_offset += 2048
                    
                }
            }
            sections[section.get("Y")?.as_byte()? as usize] = Some(unsafe { ChunkSection49::new(&mut p, skylight) });
            //p = unsafe { p.add(p_offset) }
        }
        Some(Self {
            size,
            sections,
            // Safety: This is safe because buf isn't a null pointer.
            buf: Some(unsafe { NonNull::new_unchecked(buf.as_mut_ptr()) }),
            skylight,
        })
    }

    pub(crate) const fn section_size(skylight: bool) -> usize {
        4096 + 1024 + if skylight { 1024 } else { 0 } + 256
    }

    pub fn insert_section(&mut self, i: usize, skylight: bool) {
        assert!(self.sections[i].is_none());
        let size = Self::section_size(skylight);
        let mut p: *mut u8 = self.reallocate(size).as_mut_ptr().cast();
        // Safety: This is safe because we know p was allocated for size.
        unsafe { p.write_bytes(0, size) }
        // Safety: This is safe because p was allocated and initialised correctly.
        self.sections[i] = unsafe { Some(ChunkSection49::new(&mut p, skylight)) };
    }

    util::reallocate_fn!(ChunkSection49, |self, p, _section| {
        // Safety: This is safe because `p` is properly initialised and allocated inside of the macro.
        unsafe { ChunkSection49::new(&mut p, self.skylight) }
    });

    util::from_reader_fn!(ChunkSection49, |p, skylight| {
        // Safety: This is safe because `p` is properly initialised and allocated inside of the macro.
        unsafe { ChunkSection49::new(&mut p, skylight) }
    });
}

pub struct ChunkSection49 {
    blocks: NonNull<BlockArray49<4096>>,
    light: NonNull<HalfByteArray<2048>>,
    skylight: Option<NonNull<HalfByteArray<2048>>>,
    biomes: NonNull<ByteArray<256>>,
}

impl ChunkSection49 {
    pub(self) unsafe fn new(p: &mut *mut u8, skylight: bool) -> Self {
        ChunkSection49 {
            blocks: util::assign_ref::<4096, BlockArray49<4096>>(p),
            light: util::assign_ref::<2048, HalfByteArray<2048>>(p),
            skylight: if skylight {
                Some(util::assign_ref::<2048, HalfByteArray<2048>>(p))
            } else {
                None
            },
            biomes: util::assign_ref::<256, ByteArray<256>>(p),
        }
    }
}

impl ChunkSection49 {
    util::getter!(blocks, blocks_mut, BlockArray49<4096>);
    util::getter!(light, light_mut, HalfByteArray<2048>);
    util::opt_getter!(skylight, skylight_mut, HalfByteArray<2048>);
    util::getter!(biomes, biomes_mut, ByteArray<256>);
}

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0 {
    buf: Option<NonNull<u8>>,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection0>; 16],
}

impl Encode for ChunkColumn0 {
    // This implementation only writes the chunk data, not the metadata.
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        let mut compression = flate2::write::ZlibEncoder::new(writer, flate2::Compression::fast());
        for section in self.sections.iter().flatten() {
            // TODO: add a way for the user to specify the compression level.
            section.encode(&mut compression)?;
        }
        compression.flush_finish()?;
        Ok(())
    }
}

impl Default for ChunkColumn0 {
    fn default() -> Self {
        Self::new(true)
    }
}

impl ChunkColumn0 {
    /// Constructs a new `ChunkColumn0`, doesn't allocate.
    pub fn new(skylight: bool) -> Self {
        Self {
            buf: None,
            size: 0,
            skylight,
            sections: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            ],
        }
    }

    util::from_reader_fn!(
        ChunkSection0,
        |p, skylight| {
            // Safety: This is safe because `p` is properly initialised and allocated inside of the macro.
            unsafe { ChunkSection0::new(&mut p, skylight, add) }
        },
        add,
        u16
    );

    /// Creates a new section and zero-initialises all of the buffers
    pub fn insert_section(&mut self, section: usize, add: bool) {
        assert!(self.sections[section].is_none());
        let size = Self::section_size(self.skylight, add);
        let mut p: *mut u8 = self.reallocate(size).as_mut_ptr().cast();
        // zero-initialise the buffer
        // SAFETY: This is fine because we know `p` has been allocated for `size`.
        unsafe { p.write_bytes(0, size) };
        // Safety: This is safe because `p` is allocated and initialised correctly.
        self.sections[section] = unsafe { Some(ChunkSection0::new(&mut p, self.skylight, add)) };
    }

    pub fn insert_add(&mut self, section: usize) {
        let p: *mut u8 = self.reallocate(2048).as_mut_ptr().cast();
        // zero-initialise the buffer
        // SAFETY: This is fine because we know `p` has been allocated for `size`.
        unsafe { p.write_bytes(0, 2048) };
        if let Some(section) = &mut self.sections[section] {
            assert!(section.add.is_none());
            // Safety: This is safe because p is allocated and zero-initialised for 2048 bytes.
            section.add = unsafe { Some(NonNull::new_unchecked(p.cast())) }
        } else {
            panic!("chunk section does not exist")
        }
    }

    util::reallocate_fn!(ChunkSection0, |self, p, section| {
        // Safety: This is safe because `p` is properly initialised and allocated inside of the macro.
        unsafe { ChunkSection0::new(&mut p, self.skylight, section.add.is_some()) }
    });

    /// Returns a bool indicating if this column stores sky light data.
    pub fn skylight(&self) -> bool {
        self.skylight
    }

    pub fn construct_add(&self) -> u16 {
        let mut bitmask: u16 = 0;
        for (i, section) in self.sections.iter().enumerate() {
            if let Some(section) = section {
                if section.add.is_some() {
                    // flip the bit corresponding to the section
                    bitmask |= 1 << i
                }
            }
        }
        bitmask
    }

    /// Constructs the primary bitmask for this chunk column.
    pub fn construct_bitmask(&self) -> u16 {
        let mut bitmask: u16 = 0;
        for (i, section) in self.sections.iter().enumerate() {
            if section.is_some() {
                // flip the bit corresponding to the section
                bitmask |= 1 << i
            }
        }
        bitmask
    }

    const fn section_size(skylight: bool, add: bool) -> usize {
        4096 + (2 * 2048) + 256 + if skylight { 2048 } else { 0 } + if add { 2048 } else { 0 }
    }
}

impl<'a> ChunkColumn0 {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection0> {
        if let Some(ref section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection0> {
        if let Some(ref mut section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }
}

impl<'a> Drop for ChunkColumn0 {
    fn drop(&mut self) {
        if let Some(buf) = self.buf {
            // SAFETY: This is fine because the buffer was allocated with `Vec`.
            let vec = unsafe { Vec::<u8>::from_raw_parts(buf.as_ptr(), self.size, self.size) };
            drop(vec)
        }
    }
}

#[repr(C)]
pub struct ChunkSection0 {
    blocks: NonNull<ByteArray<4096>>,
    metadata: NonNull<HalfByteArray<2048>>,
    light: NonNull<HalfByteArray<2048>>,
    skylight: Option<NonNull<HalfByteArray<2048>>>,
    add: Option<NonNull<HalfByteArray<2048>>>,
    biomes: NonNull<ByteArray<256>>,
}

impl ChunkSection0 {
    unsafe fn new(p: &mut *mut u8, skylight: bool, add: bool) -> Self {
        Self {
            blocks: util::assign_ref::<4096, ByteArray<4096>>(p),
            metadata: util::assign_ref::<2048, HalfByteArray<2048>>(p),
            light: util::assign_ref::<2048, HalfByteArray<2048>>(p),
            skylight: if skylight {
                Some(util::assign_ref::<2048, HalfByteArray<2048>>(p))
            } else {
                None
            },
            add: if add {
                Some(util::assign_ref::<2048, HalfByteArray<2048>>(p))
            } else {
                None
            },
            biomes: util::assign_ref::<256, ByteArray<256>>(p),
        }
    }
}

impl Encode for ChunkSection0 {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        // Safety: This is safe because the points are all valid references for the lifetime of self.
        unsafe {
            writer.write_all(self.blocks.as_ref().as_ref())?;
            writer.write_all(self.metadata.as_ref().as_ref())?;
            writer.write_all(self.light.as_ref().as_ref())?;
            if let Some(skylight) = &self.skylight {
                writer.write_all(skylight.as_ref().as_ref())?;
            }
            if let Some(add) = &self.add {
                writer.write_all(add.as_ref().as_ref())?;
            }
            writer.write_all(self.biomes.as_ref().as_ref())?;
            Ok(())
        }
    }
}

impl ChunkSection0 {
    util::getter!(blocks, blocks_mut, ByteArray<4096>);
    util::getter!(metadata, metadata_mut, HalfByteArray<2048>);
    util::getter!(light, light_mut, HalfByteArray<2048>);
    util::opt_getter!(skylight, skylight_mut, HalfByteArray<2048>);
    util::opt_getter!(add, add_mut, HalfByteArray<2048>);
    util::getter!(biomes, biomes_mut, ByteArray<256>);
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
    use super::util::bit_at;

    #[test]
    fn _bit_at() {
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

    mod pv0 {
    use super::super::{ChunkColumn0, util::bit_at};
        #[test]
        fn _from_reader() {
            // first we generate the data
            //TODO: use real data from minecraft
            let bitmask = 0b1011001110110011u16;
            let add = 0b1001001010010010u16;
            let skylight = true;
    
            let mut data = Vec::<u8>::new();
    
            for i in 0u8..16 {
                let exists = bit_at(bitmask, i);
                let add = bit_at(add, i);
                print!("{:b}", add as u8);
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
                if skylight {
                    for i in 0u16..2048 {
                        data.push(i as u8)
                    }
                }
                for i in 0u8..=255 {
                    data.push(i)
                }
            }
            let mut chunk =
                ChunkColumn0::from_reader(&mut std::io::Cursor::new(&data), skylight, bitmask, add)
                    .unwrap();
            drop(data);
            chunk.insert_section(6, false);
        }
    }

    mod pv49 {
        use std::{path::PathBuf, borrow::Cow};

        use miners::{nbt, encoding::Decode};

        use crate::{region, chunk::ChunkColumn49};

        // Temporary nbt decoding method while the main decode implementation is broken.
        fn decode_nbt(buf: &[u8]) -> miners::encoding::decode::Result<(Cow<str>, nbt::Compound)> {
            let mut c = std::io::Cursor::new(buf);
            let tag = nbt::NbtTag::decode(&mut c)?;
            if !matches!(tag, nbt::NbtTag::Compound) {
                return Err(miners::encoding::decode::Error::InvalidId);
            }
            let name = miners::encoding::attrs::Mutf8::decode(&mut c)?.into_inner();
            let compound = nbt::Compound::decode(&mut c)?;
            Ok((name, compound))
        }

        #[test]
        fn _from_nbt() {
            let data = include_bytes!("../test_data/testchunk.nbt");
            let (_, nbt) = decode_nbt(data).unwrap();
            let _chunk = ChunkColumn49::from_nbt(nbt, true).unwrap();
        }
    }
}
