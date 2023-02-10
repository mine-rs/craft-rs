use std::{mem::MaybeUninit, ptr::NonNull};

use miners::encoding::{Decode, Encode};

use crate::containers::{BlockArray49, ByteArray, HalfByteArray};

mod util {
    #[inline]
    pub const fn bit_at(val: u16, idx: u8) -> bool {
        debug_assert!((idx <= 0x0f));
        (val >> idx) & 0b1 != 0
    }

    /// # Safety
    /// `src` should be allocated properly, initialised, and no other references should point to it
    pub unsafe fn assign_ref<'a, const N: usize, T: From<&'a mut [u8; N]>>(src: &mut *mut u8) -> T {
        let p = src.cast() as *mut [u8; N];
        *src = src.add(N);
        (&mut *p).into()
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
            pub fn $i(&self) -> &&mut$t {
                &self.$i
            }
    
            pub fn $m(&mut self) -> &mut $t {
                self.$i
            }
        };
    }

    macro_rules! opt_getter {
        ($i:ident, $m:ident, $t:ty) => {
            pub fn $i(&self) -> Option<&$t> {
                if let Some(v) = self.$i.as_ref() {
                    Some(v)
                } else {
                    None
                }
            }
    
            pub fn $m(&mut self) -> Option<&mut $t> {
                if let Some(v) = self.$i.as_mut() {
                    Some(v)
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

    pub(super) use getter;
    pub(super) use opt_getter;
    pub(super) use reallocate_fn;
    pub(super) use from_reader_fn;    
}

pub struct ChunkColumn49<'a> {
    buf: Option<NonNull<u8>>,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection49<'a>>; 16],
}

impl ChunkColumn49<'_> {
    util::reallocate_fn!(ChunkSection49, |self, p, _section | {unsafe { ChunkSection49::new(&mut p, self.skylight) } });
}

impl ChunkColumn49<'_> {
    const fn section_size(skylight: bool) -> usize {
        4096 + 1024 + if skylight { 1024 } else { 0 } + 256
    }

    util::from_reader_fn!(ChunkSection49, | p, skylight | unsafe { ChunkSection49::new(&mut p, skylight) });
}

#[repr(C)]
pub struct ChunkSection49<'a> {
    blocks: &'a mut BlockArray49<4096>,
    light: &'a mut HalfByteArray<2048>,
    skylight: Option<&'a mut HalfByteArray<2048>>,
    biomes: &'a mut ByteArray<256>,
}

impl ChunkSection49<'_> {
    pub(self) unsafe fn new(p: &mut *mut u8, skylight: bool) -> Self {
        ChunkSection49 { blocks: util::assign_ref(p), light: util::assign_ref(p), skylight: if skylight {Some(util::assign_ref(p))} else {None}, biomes: util::assign_ref(p) }
    }
}

impl<'a> ChunkSection49<'a> {
    util::getter!(blocks, blocks_mut, BlockArray49<4096>);
    util::getter!(light, light_mut, HalfByteArray<2048>);
    util::opt_getter!(skylight, skylight_mut, HalfByteArray<2048>);
    util::getter!(biomes, biomes_mut, ByteArray<256>);
}

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkColumn0<'a> {
    buf: Option<NonNull<u8>>,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection0<'a>>; 16],
}

impl Encode for ChunkColumn0<'_> {
    // This implementation only writes the chunk data, not the metadata.
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        let mut compression = flate2::write::ZlibEncoder::new(writer, flate2::Compression::fast());
        for section in &self.sections {
            if let Some(section) = section {
                // TODO: add a way for the user to specify the compression level.
                section.encode(&mut compression)?;
            }
        }
        compression.flush_finish()?;
        Ok(())
    }
}

impl Default for ChunkColumn0<'_> {
    fn default() -> Self {
        Self::new(true)
    }
}

impl ChunkColumn0<'_> {
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
    
    util::from_reader_fn!(ChunkSection0, | p, skylight | unsafe { ChunkSection0::new(&mut p, skylight, add) }, add, u16);

    /// Creates a new section and zero-initialises all of the buffers
    pub fn insert_section(&mut self, section: usize, add: bool) {
        assert!(self.sections[section].is_none());
        let size = Self::section_size(self.skylight, add);
        let mut p: *mut u8 = self.reallocate(size).as_mut_ptr().cast();
        // zero-initialise the buffer
        // SAFETY: This is fine because we know `p` has been allocated for `size`.
        unsafe { p.write_bytes(0, size) };
        self.sections[section] = Some(ChunkSection0 {
            blocks: unsafe { util::assign_ref(&mut p) },
            metadata: unsafe { util::assign_ref(&mut p) },
            light: unsafe { util::assign_ref(&mut p) },
            skylight: if self.skylight {
                Some(unsafe { util::assign_ref(&mut p) })
            } else {
                None
            },
            add: if add {
                Some(unsafe { util::assign_ref(&mut p) })
            } else {
                None
            },
            biomes: unsafe { util::assign_ref(&mut p) },
        })
    }

    pub fn insert_add(&mut self, section: usize) {
        let p: *mut u8 = self.reallocate(2048).as_mut_ptr().cast();
        // zero-initialise the buffer
        // SAFETY: This is fine because we know `p` has been allocated for `size`.
        unsafe { p.write_bytes(0, 2048) };
        if let Some(section) = &mut self.sections[section] {
            assert!(section.add.is_none());
            section.add = unsafe { Some((&mut *(p.cast() as *mut [u8; 2048])).into()) }
        } else {
            panic!("chunk section does not exist")
        }
    }

    util::reallocate_fn!(ChunkSection0, |self, p, section| unsafe { ChunkSection0::new(&mut p, self.skylight, section.add.is_some()) });

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

#[repr(C)]
pub struct ChunkSection0<'a> {
    blocks: &'a mut ByteArray<4096>,
    metadata: &'a mut HalfByteArray<2048>,
    light: &'a mut HalfByteArray<2048>,
    skylight: Option<&'a mut HalfByteArray<2048>>,
    add: Option<&'a mut HalfByteArray<2048>>,
    biomes: &'a mut ByteArray<256>,
}

impl ChunkSection0<'_> {
    unsafe fn new(p: &mut *mut u8, skylight: bool, add: bool) -> Self {
        Self {
            blocks: util::assign_ref(p),
            metadata: util::assign_ref(p) ,
            light: util::assign_ref(p) ,
            skylight: if skylight {
                Some(util::assign_ref(p))
            } else {
                None
            },
            add: if add {
                Some(util::assign_ref(p))
            } else {
                None
            },
            biomes: util::assign_ref(p) ,
        }
    }
}

impl Encode for ChunkSection0<'_> {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        writer.write_all(self.blocks.as_ref())?;
        writer.write_all(self.metadata.as_ref())?;
        writer.write_all(self.light.as_ref())?;
        if let Some(skylight) = &self.skylight {
            writer.write_all(skylight.as_ref())?;
        }
        if let Some(add) = &self.add {
            writer.write_all(add.as_ref())?;
        }
        writer.write_all(self.biomes.as_ref())?;
        Ok(())
    }
}

impl ChunkSection0<'_> {
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
    use super::{util::bit_at, ChunkColumn0};

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
