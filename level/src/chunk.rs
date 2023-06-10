use std::ptr::NonNull;

use bumpalo::Bump;
use miners::{
    encoding::{Decode, Encode},
    nbt::List,
};

use crate::containers::{Block47, BlockArray47, ByteArray, HalfByteArray, ReadContainer};

use self::util::{create, impl_clone};

mod util {
    //TODO: rename
    macro_rules! create {
        ($t:ty, $buf:ident, $data:ident) => {{
            let slice = $buf.alloc_slice_copy(&$data[..std::mem::size_of::<$t>()]);
            *$data = &$data[std::mem::size_of::<$t>()..];
            NonNull::new(<&mut $t>::try_from(slice).unwrap()).unwrap()
        }};
    }

    #[inline]
    pub const fn bit_at(val: u16, idx: u8) -> bool {
        debug_assert!((idx <= 0x0f));
        (val >> idx) & 0b1 != 0
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

    macro_rules! void {
        ($_:tt, $e:expr) => {
            $e
        }
    }

    /// Used to implement clone for the 0 and 47 protocol versions
    macro_rules! impl_clone {
        ($column_t:ty, $section_t:ty $(, $marker:tt)?) => {
            impl Clone for $column_t {
                fn clone(&self) -> Self {
                    let buf = Bump::with_capacity(self.size);
                    let mut sections: [Option<$section_t>; 16] = [
                        None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                        None, None,
                    ];
                    for (i, section) in self
                        .sections
                        .iter()
                        .enumerate()
                        .flat_map(|(i, section)| section.into_iter().map(move |x| (i, x)))
                    {
                        sections[i] = Some(<$section_t>::from_slices(
                            buf.alloc_slice_copy(section.blocks().as_ref()),
                            $(util::void!($marker, buf.alloc_slice_copy(section.metadata().as_ref())),)?
                            buf.alloc_slice_copy(section.light().as_ref()),
                            section.skylight().map(|v| buf.alloc_slice_copy(v.as_ref())),
                            $(util::void!($marker, section.add().map(|v| buf.alloc_slice_copy(v.as_ref()))),)?
                        ))
                    }
            
                    let biomes = NonNull::new(
                        <&mut ByteArray<256>>::try_from(
                            buf.alloc_slice_copy(unsafe { self.biomes.as_ref() }.as_ref()),
                        )
                        .unwrap(),
                    )
                    .unwrap();
            
                    Self {
                        buf,
                        size: self.size,
                        skylight: self.skylight,
                        sections,
                        biomes,
                    }
                }
            }
        };
    }

    macro_rules! from_reader_fn {
        ($section:ty, | $buf:ident, $data:ident, $skylight:ident | $e:expr $(, $add:ident, $t:ty)?) => {
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

                let mut $data = {
                    let pos = cursor.position() as usize;
                    let slice = cursor
                        .get_ref()
                        .get(pos..pos + size as usize)
                        .ok_or(miners::encoding::decode::Error::UnexpectedEndOfSlice)?;
                    cursor.set_position((pos + size) as u64);
                    debug_assert_eq!(slice.len(), size);
                    slice
                };

                let $buf = Bump::with_capacity(size);
                // Copy the data over to the buffer

                let mut sections: [Option<$section>; 16] = [
                    None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                    None, None,
                ];

                for i in 0u8..16 {
                    let exists: bool = $crate::chunk::util::bit_at(bitmask, i);
                    if exists {
                        $(let $add: bool = util::bit_at($add, i);)?
                        // Safety: This is safe because p is properly allocated and there are no other references pointing to it.
                        let section = $e;
                        sections[i as usize] = Some(section)
                    }
                }

                let biomes = NonNull::new(<&mut ByteArray<256>>::try_from($buf.alloc_slice_copy($data)).unwrap()).unwrap();

                Ok(Self {
                    // Safety: This is safe because buf isn't a null pointer.
                    $buf,
                    size,
                    $skylight,
                    // Safety: The compiler thinks `sections` is bound by the lifetime of the cursor slice, but it isn't because we copied the data over to a new buffer.
                    sections: unsafe { std::mem::transmute(sections) },
                    biomes,
                })
            }
        };
    }

    pub(super) use create;
    pub(super) use from_reader_fn;
    pub(super) use getter;
    pub(super) use impl_clone;
    pub(super) use opt_getter;
    pub(super) use void;
}

pub struct ChunkColumn0 {
    buf: Bump,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection0>; 16],
    biomes: NonNull<ByteArray<256>>,
}

impl_clone!(ChunkColumn0, ChunkSection0, ());

/*
impl Clone for ChunkColumn0 {
    fn clone(&self) -> Self {
        let buf = Bump::with_capacity(self.size);
        let mut sections: [Option<ChunkSection0>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        for (i, section) in self
            .sections
            .iter()
            .enumerate()
            .flat_map(|(i, section)| section.into_iter().map(move |x| (i, x)))
        {
            sections[i] = Some(ChunkSection0::from_slices(
                buf.alloc_slice_copy(section.blocks().as_ref()),
                buf.alloc_slice_copy(section.metadata().as_ref()),
                buf.alloc_slice_copy(section.light().as_ref()),
                section.skylight().map(|v| buf.alloc_slice_copy(v.as_ref())),
                section.add().map(|v| buf.alloc_slice_copy(v.as_ref())),
            ))
        }

        let biomes = NonNull::new(
            <&mut ByteArray<256>>::try_from(
                buf.alloc_slice_copy(unsafe { self.biomes.as_ref() }.as_ref()),
            )
            .unwrap(),
        )
        .unwrap();

        Self {
            buf,
            size: self.size,
            skylight: self.skylight,
            sections,
            biomes,
        }
    }
}
*/

// Safety: This is safe because no data races can occur since the mutable pointers are only written to when &mut self is passed.
unsafe impl Sync for ChunkColumn0 {}
// Safety: ^
unsafe impl Send for ChunkColumn0 {}

/*
impl Default for ChunkColumn0 {
    fn default() -> Self {
        Self::new(true)
    }
}
*/

impl Encode for ChunkColumn0 {
    // This implementation only writes the chunk data, not the metadata.
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        let mut compression = flate2::write::ZlibEncoder::new(writer, flate2::Compression::fast());
        for section in self.sections.iter().flatten() {
            // TODO: add a way for the user to specify the compression level.
            section.encode(&mut compression)?;
        }
        unsafe { self.biomes.as_ref().encode(&mut compression)? };
        compression.flush_finish()?;
        Ok(())
    }
}

impl ChunkColumn0 {
    /*
    /// Constructs a new `ChunkColumn0`, doesn't allocate.
    pub fn new(skylight: bool) -> Self {
        Self {
            buf: Bump::new(),
            size: 0,
            skylight,
            sections: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            ],
            biomes: todo!()
        }
    }*/

    util::from_reader_fn!(
        ChunkSection0,
        |buf, data, skylight| {
            ChunkSection0::new(&buf, &mut data, skylight, add)
        },
        add,
        u16
    );

    /// Creates a new section and zero-initialises all of the buffers
    pub fn insert_section(&mut self, section: usize, add: bool) {
        assert!(self.sections[section].is_none());
        let size = Self::section_size(self.skylight, add);
        self.buf.alloc_slice_fill_with(size, |_| 0);
        self.sections[section] = Some(ChunkSection0::new_zeroed(&self.buf, self.skylight));
        self.size += size;
    }

    pub fn insert_add(&mut self, section: usize) {
        if let Some(section) = &mut self.sections[section] {
            assert!(section.add.is_none());
            section.add = Some(NonNull::new(
                <&mut HalfByteArray<2048>>::try_from(self.buf.alloc_slice_fill_with(2048, |_| 0))
                    .unwrap(),
            ))
            .unwrap();
            self.size += 2048
        } else {
            panic!("chunk section does not exist")
        }
    }

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
        use std::mem::size_of;
        size_of::<ByteArray<4096>>()
            + size_of::<HalfByteArray<2048>>()
            + size_of::<HalfByteArray<2048>>()
            + size_of::<HalfByteArray<2048>>()
            + if skylight {
                size_of::<HalfByteArray<2048>>()
            } else {
                0
            }
            + if add {
                size_of::<HalfByteArray<2048>>()
            } else {
                0
            }
            + size_of::<ByteArray<256>>()
        //4096 + (2 * 2048) + 256 + if skylight { 2048 } else { 0 } + if add { 2048 } else { 0 }
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

#[repr(C)]
pub struct ChunkSection0 {
    blocks: NonNull<ByteArray<4096>>,
    metadata: NonNull<HalfByteArray<2048>>,
    light: NonNull<HalfByteArray<2048>>,
    skylight: Option<NonNull<HalfByteArray<2048>>>,
    add: Option<NonNull<HalfByteArray<2048>>>,
}

impl ChunkSection0 {
    fn new(buf: &Bump, data: &mut &[u8], skylight: bool, add: bool) -> Self {
        Self {
            blocks: create!(ByteArray<4096>, buf, data),
            metadata: create!(HalfByteArray<2048>, buf, data),
            light: create!(HalfByteArray<2048>, buf, data),
            skylight: if skylight {
                Some(create!(HalfByteArray<2048>, buf, data))
            } else {
                None
            },
            add: if add {
                Some(create!(HalfByteArray<2048>, buf, data))
            } else {
                None
            },
        }
    }

    fn from_slices(
        blocks: &mut [u8],
        metadata: &mut [u8],
        light: &mut [u8],
        skylight: Option<&mut [u8]>,
        add: Option<&mut [u8]>,
    ) -> Self {
        Self {
            blocks: NonNull::new(<&mut ByteArray<4096>>::try_from(blocks).unwrap()).unwrap(),
            metadata: NonNull::new(<&mut HalfByteArray<2048>>::try_from(metadata).unwrap())
                .unwrap(),
            light: NonNull::new(<&mut HalfByteArray<2048>>::try_from(light).unwrap()).unwrap(),
            skylight: skylight
                .map(|v| NonNull::new(<&mut HalfByteArray<2048>>::try_from(v).unwrap()).unwrap()),
            add: add
                .map(|v| NonNull::new(<&mut HalfByteArray<2048>>::try_from(v).unwrap()).unwrap()),
        }
    }

    fn new_zeroed(buf: &Bump, skylight: bool) -> Self {
        Self {
            blocks: NonNull::new(
                <&mut ByteArray<4096>>::try_from(buf.alloc_slice_fill_with(4096, |_| 0)).unwrap(),
            )
            .unwrap(),
            metadata: NonNull::new(
                <&mut HalfByteArray<2048>>::try_from(buf.alloc_slice_fill_with(2048, |_| 0))
                    .unwrap(),
            )
            .unwrap(),
            light: NonNull::new(
                <&mut HalfByteArray<2048>>::try_from(buf.alloc_slice_fill_with(2048, |_| 0))
                    .unwrap(),
            )
            .unwrap(),
            skylight: if skylight {
                Some(
                    NonNull::new(
                        <&mut HalfByteArray<2048>>::try_from(
                            buf.alloc_slice_fill_with(2048, |_| 0),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                )
            } else {
                None
            },
            add: None,
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
}

pub struct ChunkColumn47 {
    buf: Bump,
    size: usize,
    skylight: bool,
    sections: [Option<ChunkSection47>; 16],
    biomes: NonNull<ByteArray<256>>,
}

impl_clone!(ChunkColumn47, ChunkSection47);

// Safety: This is safe because no data races can occur since the mutable pointers are only written to when &mut self is passed.
unsafe impl Sync for ChunkColumn47 {}
// Safety: ^
unsafe impl Send for ChunkColumn47 {}

impl Encode for ChunkColumn47 {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        for section in self.sections.iter().flatten() {
            section.encode(writer)?;
            unsafe { self.biomes.as_ref().encode(writer)? };
        }
        Ok(())
    }
}

impl ChunkColumn47 {
    /// Gets a reference to the section if it exists.
    pub fn section(&self, section: usize) -> Option<&ChunkSection47> {
        if let Some(ref section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }

    /// Gets a mutable reference to the section if it exists.
    pub fn section_mut(&mut self, section: usize) -> Option<&mut ChunkSection47> {
        if let Some(ref mut section) = self.sections[section] {
            Some(section)
        } else {
            None
        }
    }

    pub fn primary_bitmap(&self) -> u16 {
        let mut bitmap = 0u16;
        for (i, _) in self
            .sections
            .iter()
            .enumerate()
            .flat_map(|(i, section)| section.into_iter().map(move |x| (i, x)))
        {
            bitmap |= 1 << i
        }
        bitmap
    }
}

impl ChunkColumn47 {
    util::from_reader_fn!(ChunkSection47, |buf, data, skylight| ChunkSection47::new(&buf, &mut data, skylight));
/*
    pub fn from_reader(
        cursor: &mut std::io::Cursor<&[u8]>,
        skylight: bool,
        bitmask: u16,
    ) -> miners::encoding::decode::Result<Self> {
        let mut size = 0;
        for i in 0u8..16 {
            let exists: bool = util::bit_at(bitmask, i);
            if exists {
                size += Self::section_size(skylight)
            }
        }

        let mut data = {
            let pos = cursor.position() as usize;
            let slice = cursor
                .get_ref()
                .get(pos..pos + size as usize)
                .ok_or(miners::encoding::decode::Error::UnexpectedEndOfSlice)?;
            cursor.set_position((pos + size) as u64);
            debug_assert_eq!(slice.len(), size);
            slice
        };

        let buf = Bump::new();

        let mut sections: [Option<ChunkSection47>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];

        for i in 0..16 {
            let exists = util::bit_at(bitmask, i as u8);
            if exists {
                sections[i as usize] = Some(ChunkSection47::new(&buf, &mut data, skylight))
            }
        }

        let biomes =
            NonNull::new(<&mut ByteArray<256>>::try_from(buf.alloc_slice_copy(data)).unwrap())
                .unwrap();

        Ok(Self {
            buf,
            size,
            skylight,
            sections,
            biomes,
        })
    }
*/
    /// Parses 1.8 anvil chunk nbt data into a `ChunkColumn49`. This function does not take an entire region file as input, but one of the chunks contained within.
    pub fn from_nbt(nbt: &miners::nbt::Compound, skylight: bool) -> Option<Self> {
        //TODO: Return Result and not Option.
        let nbt = nbt.get("Level")?.as_compound()?;

        let sections_data = {
            let List::Compound(sections) = nbt.get("Sections")?.as_list()? else {
                return None;
            };
            sections
        };

        let mut sections: [Option<ChunkSection47>; 16] = [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
            None, None,
        ];
        let size: usize = Self::section_size(skylight) * sections_data.len();
        let buf = Bump::with_capacity(size);

        for section in sections_data.iter() {
            let light = section.get("BlockLight")?.as_byte_array()?;
            if light.len() != 2048 {
                return None;
            }
            let light = buf.alloc_slice_copy(light.as_ref());

            let blocks = section.get("Blocks")?.as_byte_array()?;
            if blocks.len() != 8192 {
                return None;
            }
            let blocks = buf.alloc_slice_copy(blocks.as_ref());

            let metadata = section.get("Data")?.as_byte_array()?;
            if metadata.len() != 2048 {
                return None;
            }
            //let metadata = <&HalfByteArray<2048>>::from(std::mem::transmute::<*const u8, &[u8; 2048]>(metadata.as_ptr()));
            let metadata: &[u8; 2048] = metadata[..2048].try_into().unwrap();
            let metadata = <&HalfByteArray<2048>>::from(metadata);

            buf.alloc_slice_fill_with(4096, |i| Block47::new(blocks[i] as u16, metadata.get(i)));

            let skylight = if skylight {
                let skylight = section.get("SkyLight")?.as_byte_array()?;
                if skylight.len() != 2048 {
                    return None;
                }
                Some(buf.alloc_slice_copy(skylight))
            } else {
                None
            };

            *sections.get_mut(section.get("Y")?.as_byte()? as usize)? =
                Some(ChunkSection47::from_slices(blocks, light, skylight));
        }

        let biomes = nbt.get("Biomes")?.as_byte_array()?;
        if biomes.len() != 256 {
            return None;
        }
        let biomes =
            NonNull::new(<&mut ByteArray<256>>::try_from(buf.alloc_slice_copy(biomes)).unwrap())
                .unwrap();

        Some(Self {
            size,
            sections,
            // Safety: This is safe because buf isn't a null pointer.
            buf,
            skylight,
            biomes,
        })
    }

    pub(crate) const fn section_size(skylight: bool) -> usize {
        use std::mem::size_of;
        size_of::<BlockArray47<4096>>()
            + size_of::<HalfByteArray<2048>>()
            + if skylight {
                size_of::<HalfByteArray<2048>>()
            } else {
                0
            }
            + size_of::<ByteArray<256>>()
    }

    pub fn insert_section(&mut self, i: usize, skylight: bool) {
        assert!(self.sections[i].is_none());
        let size = Self::section_size(skylight);
        self.buf.alloc_slice_fill_with(size, |_| 0);
        self.sections[i] = Some(ChunkSection47::new_zeroed(&self.buf, self.skylight));
    }
}

#[derive(Debug)]
pub struct ChunkSection47 {
    blocks: NonNull<BlockArray47<4096>>,
    light: NonNull<HalfByteArray<2048>>,
    skylight: Option<NonNull<HalfByteArray<2048>>>,
    //biomes: NonNull<ByteArray<256>>,
}

impl Encode for ChunkSection47 {
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        unsafe {
            writer.write_all(self.blocks.as_ref().as_ref())?;
            writer.write_all(self.light.as_ref().as_ref())?;
            if let Some(skylight) = &self.skylight {
                writer.write_all(skylight.as_ref().as_ref())?;
            }
            //writer.write_all(self.biomes.as_ref().as_ref())?;
            Ok(())
        }
    }
}

impl ChunkSection47 {
    pub(self) fn new(buf: &bumpalo::Bump, data: &mut &[u8], skylight: bool) -> Self {
        ChunkSection47 {
            blocks: create!(BlockArray47<4096>, buf, data),
            light: create!(HalfByteArray<2048>, buf, data),
            skylight: if skylight {
                Some(create!(HalfByteArray<2048>, buf, data))
            } else {
                None
            },
            //biomes: create!(ByteArray<256>, buf, data),
        }
    }

    pub(self) fn new_zeroed(buf: &Bump, skylight: bool) -> Self {
        ChunkSection47 {
            blocks: NonNull::new(
                <&mut BlockArray47<4096>>::try_from(buf.alloc_slice_fill_with(8192, |_| 0))
                    .unwrap(),
            )
            .unwrap(),
            light: NonNull::new(
                <&mut HalfByteArray<2048>>::try_from(buf.alloc_slice_fill_with(2048, |_| 0))
                    .unwrap(),
            )
            .unwrap(),
            skylight: if skylight {
                Some(
                    NonNull::new(
                        <&mut HalfByteArray<2048>>::try_from(
                            buf.alloc_slice_fill_with(2048, |_| 0),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                )
            } else {
                None
            },
            //biomes: NonNull::new(<&mut ByteArray<256>>::try_from(buf.alloc_slice_fill_with(4096, |_| 0)).unwrap()).unwrap(),
        }
    }

    pub(self) fn from_slices(
        blocks: &mut [u8],
        light: &mut [u8],
        skylight: Option<&mut [u8]>,
    ) -> Self {
        Self {
            blocks: NonNull::new(<&mut BlockArray47<4096>>::try_from(blocks).unwrap()).unwrap(),
            light: NonNull::new(<&mut HalfByteArray<2048>>::try_from(light).unwrap()).unwrap(),
            skylight: skylight
                .map(|v| NonNull::new(<&mut HalfByteArray<2048>>::try_from(v).unwrap()).unwrap()),
            //biomes: NonNull::new(<&mut ByteArray<256>>::try_from(biomes).unwrap()).unwrap(),
        }
    }
}

impl ChunkSection47 {
    util::getter!(blocks, blocks_mut, BlockArray47<4096>);
    util::getter!(light, light_mut, HalfByteArray<2048>);
    util::opt_getter!(skylight, skylight_mut, HalfByteArray<2048>);
    //util::getter!(biomes, biomes_mut, ByteArray<256>);
}

/*
/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}
*/

/// A 16 * 16 * 16 section of a chunk.
/*
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
*/
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
        use super::super::{util::bit_at, ChunkColumn0};
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
        use std::io::Cursor;

        use miners::{
            encoding::{Decode, Encode},
            nbt,
        };

        use crate::chunk::ChunkColumn47;

        #[test]
        fn _from_nbt() {
            let data = include_bytes!("../test_data/testchunk.nbt");
            let nbt = nbt::Nbt::decode(&mut Cursor::new(data)).unwrap();
            let chunk = ChunkColumn47::from_nbt(&nbt, true).unwrap();
            //let clone = chunk.clone();
            let mut buf = Vec::new();
            chunk.encode(&mut buf).unwrap();
            panic!()
        }
    }
}
