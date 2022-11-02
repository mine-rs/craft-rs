// TODO: Add actual functions to the traits
pub trait Block {}

pub trait State: Eq + Copy + Ord + Into<u64> + From<usize> {}
