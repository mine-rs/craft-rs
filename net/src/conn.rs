use std::ops::{Deref, DerefMut};

use futures_lite::io::{BufReader, BufWriter};
use miners::{net::encoding::Encoder, protocol::Packet};
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::packet::DynPacket;

pub struct Connection {
    inner: miners::net::conn::Connection<
        BufReader<Compat<OwnedReadHalf>>,
        BufWriter<Compat<OwnedWriteHalf>>,
    >,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        let (read, write) = stream.into_split();
        let conn = miners::net::conn::Connection::new(read.compat(), write.compat_write());
        Self { inner: conn }
    }

    pub fn enable_encryption(&mut self, key: &[u8]) -> Result<(), crypto_common::InvalidLength> {
        self.inner.enable_encryption(key, key)
    }

    pub fn split(self) -> (ReadHalf, WriteHalf) {
        let (readinner, writeinner) = self.inner.split();
        (ReadHalf::new(readinner), WriteHalf::new(writeinner))
    }
}

pub struct ReadHalf {
    inner: miners::net::conn::ReadHalf<BufReader<Compat<OwnedReadHalf>>>,
}

impl Deref for ReadHalf {
    type Target = miners::net::conn::ReadHalf<BufReader<Compat<OwnedReadHalf>>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ReadHalf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl ReadHalf {
    pub(super) fn new(
        inner: miners::net::conn::ReadHalf<BufReader<Compat<OwnedReadHalf>>>,
    ) -> Self {
        Self { inner }
    }

    pub async fn read_packet(&mut self) -> miners::encoding::decode::Result<(i32, &[u8])> {
        self.inner.read_encoded().await?.into_packet()
    }
}

pub struct WriteHalf {
    encoder: Encoder,
    inner: miners::net::conn::WriteHalf<BufWriter<Compat<OwnedWriteHalf>>>,
}

impl Deref for WriteHalf {
    type Target = miners::net::conn::WriteHalf<BufWriter<Compat<OwnedWriteHalf>>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for WriteHalf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl WriteHalf {
    pub(super) fn new(
        inner: miners::net::conn::WriteHalf<BufWriter<Compat<OwnedWriteHalf>>>,
    ) -> Self {
        Self {
            encoder: Encoder::new(),
            inner,
        }
    }

    pub async fn write_packet(
        &mut self,
        version: i32,
        packet: impl Packet,
    ) -> miners::encoding::encode::Result<()> {
        self.inner
            .write_packet(version, packet, &mut self.encoder)
            .await
    }

    pub async fn write_dyn_packet(
        &mut self,
        version: i32,
        packet: &dyn DynPacket<Vec<u8>>,
    ) -> miners::encoding::encode::Result<()> {
        self.inner
            .write_dyn_packet(version, packet, &mut self.encoder)
            .await
    }
}
