use std::marker::PhantomData;

use miners::encoding::{Decode, Encode};

use crate::containers::{half_byte_array, ByteArray, DataContainer};

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkSection0<'a> {
    pub blocks: ByteArray<'a, 4096>,
    pub metadata: half_byte_array!('a, 4096),
    pub light: half_byte_array!('a, 4096),
    pub sky_light: Option<half_byte_array!('a, 4096)>,
    pub add: Option<half_byte_array!('a, 4096)>,
    pub biomes: Box<half_byte_array!('a, 4096)>,
}

/// A 16 * 16 * 16 section of a chunk.
pub struct ChunkSection<SV, BV, S: DataContainer<4096, SV>, B: DataContainer<64, BV>> {
    pub block_count: u16,
    pub states: S,
    pub biomes: B,
    __marker: PhantomData<SV>,
    __marker2: PhantomData<BV>,
}

impl<SV, BV, S: DataContainer<4096, SV>, B: DataContainer<64, BV>> Encode
    for ChunkSection<SV, BV, S, B>
{
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        self.block_count.encode(writer)?;
        self.states.encode(writer)?;
        self.biomes.encode(writer)
    }
}

impl<SV, BV, S: DataContainer<4096, SV>, B: DataContainer<64, BV>> Decode<'_>
    for ChunkSection<SV, BV, S, B>
{
    fn decode(cursor: &mut std::io::Cursor<&'_ [u8]>) -> miners::encoding::decode::Result<Self> {
        Ok(Self {
            block_count: u16::decode(cursor)?,
            states: S::decode(cursor)?,
            biomes: B::decode(cursor)?,
            __marker: PhantomData,
            __marker2: PhantomData,
        })
    }
}
