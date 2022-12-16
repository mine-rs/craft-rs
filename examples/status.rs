/// Implements https://wiki.vg/Server_List_Ping
use std::borrow::Cow;
use std::io::Cursor;

use async_std::net::TcpStream;
use miners::encoding::Decode;
use miners::protocol::netty::status::clientbound::Response0 as Response;
use miners::protocol::netty::status::clientbound::Ping0 as Ping;
use miners::protocol::netty::status::serverbound::Request0 as Request;
use miners::{net::conn::Connection, version::ProtocolVersion};
use miners::protocol::netty::handshaking::serverbound::Handshake0 as Handshake;
use miners::protocol::netty::handshaking::serverbound::NextState0 as NextState;
use miners::protocol::netty::status::clientbound::Ping0 as Pong; // should probably be renamed to pong

const ADDRESS: &'static str = "mc.hypixel.net:25565";
const VERSION: i32 = 47;

#[async_std::main]
async fn main() {
    let version = ProtocolVersion::new(VERSION).unwrap();

    let mut encoder = miners::net::encoding::Encoder::new();
    let stream = TcpStream::connect(ADDRESS).await.unwrap();
    let mut conn = Connection::new(stream.clone(), stream);
    // initiate handshake
    conn.write_half.write_packet(version, Handshake { // it would be nice if we didn't have to access the write/read halves as fields
        protocol_version: VERSION,
        server_address: Cow::Borrowed("mc.hypixel.net"), // we should probably change this to &str
        server_port: 25565,
        next_state: NextState::Status
    }, &mut encoder).await.unwrap(); // We should also add an abstraction (in craftrs.unwrap()) so you don't have to pass around encoders
    // initiate server list ping
    conn.write_half.write_packet(version, Request {}, &mut encoder).await.unwrap(); // Request0 should probably be a marker struct instead so you don't need {}
    // There should be an abstraction (in craftrs.unwrap()) for simpler decoding.
    conn.write_half.flush().await.unwrap(); // we should probably have write_packet and write_packet_unflushed methods
    let (id, data) = conn.read_half.read_encoded().await.unwrap().into_packet().unwrap(); // EOF
    assert_eq!(id, 0x00);
    //assert_eq!(id, Response0::id_for_version(&self, version)); // ID for version should NOT take self
    // decode response
    let resp = Response::decode(&mut Cursor::new(data)).unwrap().data.to_owned(); // We should add a method so we don't need to construct the Cursor ourselves.
    println!("{ADDRESS}:\n\tRESPONSE: {resp}");
    // ping
    let time = std::time::Instant::now();
    let payload = chrono::Utc::now().timestamp();
    conn.write_half.write_packet(version, Ping { time: payload }, &mut encoder).await.unwrap();
    conn.write_half.flush().await.unwrap();
    let (id, data) = conn.read_half.read_encoded().await.unwrap().into_packet().unwrap();
    assert_eq!(id, 0x01);
    let pong = Pong::decode(&mut Cursor::new(data)).unwrap().time;
    assert_eq!(pong, payload);
    let elapsed = time.elapsed().as_millis();
    println!("--------------------------------------------------------------------------------\n\tPING:{elapsed}ms");
}