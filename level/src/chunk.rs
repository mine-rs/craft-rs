use std::borrow::Cow;

use crate::palette::{BiomePaletteContainer, StatePaletteContainer};

/// A chunk column, not including heightmaps
pub struct ChunkColumn<const N: usize, B: super::bitpack::byteorder::ByteOrderedU64> {
    //pub motion_blocking: PackedBits<256>, // len = 256 bits = 9
    pub sections: [Option<ChunkSection<B>>; N],
}

/// A 16 * 16 * 16 section of a chunk.
pub struct ChunkSection<B: super::bitpack::byteorder::ByteOrderedU64> {
    pub block_count: u16,
    pub states: StatePaletteContainer<4096, B>, // 16*16*16 = 4096
    pub biomes: BiomePaletteContainer<64, B>,   // 4*4*4 = 64
}

pub trait BlockDataContainer<B: super::bitpack::byteorder::ByteOrderedU64, V>: DataContainer<4096, V> {
    fn new<'a>(data: Cow<'a, [u8]>, version: miners::version::ProtocolVersion) -> Self;
}

pub trait ChunkDataContainer<const N: usize, V>: DataContainer<N, V> {
    fn new<'a>(data: Cow<'a, [u8]>, version: miners::version::ProtocolVersion) -> Self;
}

pub unsafe trait DataContainer<const N: usize, V> {
    fn get(&self, i: usize) -> V {
        if i >= N {
            panic!("out of bounds")
        }
        //SAFETY: This is safe because we know i is in bounds.
        unsafe { self.get_unchecked(i) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn get_unchecked(&self, i: usize) -> V;

    fn set(&mut self, i: usize, v: V) {
        if i >= N {
            panic!("out of bounds")
        }
        // SAFETY: This is sound because we just checked the bounds
        unsafe { self.set_unchecked(i, v) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds.
    unsafe fn set_unchecked(&mut self, i: usize, v: V);

    fn swap(&mut self, i: usize, v: V) -> V {
        if i >= N {
            panic!("out of bounds")
        }
        //SAFETY: This is safe because we just checked the bounds.
        unsafe { self.swap_unchecked(i, v) }
    }

    /// # Safety
    /// This method is safe as long as `i` is within bounds
    unsafe fn swap_unchecked(&mut self, i: usize, v: V) -> V {
        let val = self.get_unchecked(i);
        self.set_unchecked(i, v);
        val
    }
}

unsafe impl<const N: usize, T: super::palette::PaletteContainer<N>> DataContainer<N, u64> for T {
    unsafe fn get_unchecked(&self, i: usize) -> u64 {
        self.get_unchecked(i)
    }
    unsafe fn set_unchecked(&mut self, i: usize, v: u64) {
        self.set_unchecked(i, v)
    }
}
