use miners::encoding::{Decode, Encode};

use crate::containers::{half_byte_array, ByteArray};

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
pub struct ChunkSection<S, B> {
    pub block_count: u16,
    pub states: S,
    pub biomes: B,
}

impl<S: Encode, B: Encode> Encode
    for ChunkSection<S, B>
{
    fn encode(&self, writer: &mut impl std::io::Write) -> miners::encoding::encode::Result<()> {
        self.block_count.encode(writer)?;
        self.states.encode(writer)?;
        self.biomes.encode(writer)
    }
}

impl<S: for<'a> Decode<'a>, B: for<'a> Decode<'a>> Decode<'_>
    for ChunkSection<S, B>
{
    fn decode(cursor: &mut std::io::Cursor<&'_ [u8]>) -> miners::encoding::decode::Result<Self> {
        Ok(Self {
            block_count: u16::decode(cursor)?,
            states: S::decode(cursor)?,
            biomes: B::decode(cursor)?,
        })
    }
}
