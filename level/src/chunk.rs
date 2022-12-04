use std::marker::PhantomData;

use miners::encoding::{Decode, Encode};

use crate::containers::{DataContainer, ByteArray, HalfByteArray};

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, S> {
    pub sections: [Option<S>; N],
}

pub struct ChunkSection0 {
    pub blocks: ByteArray<4096>,
    pub metadata: HalfByteArray,
    pub light: HalfByteArray,
    pub sky_light: Option<HalfByteArray>,
    pub add: Option<HalfByteArray>,
    pub biomes: Box<HalfByteArray>,
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
