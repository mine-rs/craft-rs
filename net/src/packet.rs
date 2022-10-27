pub use miners::packet::{Packet, DynPacket};

pub trait PacketData<W> {
    fn to_packet(&self, version: i32) -> dyn DynPacket<W>;
}


// TODO: Create the PacketData types and implement PacketBuilderExt for them
