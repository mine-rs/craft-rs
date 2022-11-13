use crate::{palette::{StatePaletteContainer, BiomePaletteContainer}, bitpack::PackedBits};
pub struct ChunkColumn {
    pub motion_blocking: PackedBits<256>, // len = 256 bits = 9
    pub sections: Vec<Option<ChunkSection>>,
}

pub struct ChunkSection {
    pub block_count: u16,
    pub states: StatePaletteContainer<4096>, // 16*16*16 = 4096
    pub biomes: BiomePaletteContainer<64>, // 4*4*4 = 64
}