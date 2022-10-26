pub use miners::packet::Packet;

pub trait PacketBuilderExt: Packet {
    type Data;
    fn new(data: Self::Data) -> Self;
}

// TODO: Create the PacketData types and implement PacketBuilderExt for
