use miners::net::encoding::Encoder;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub fn request_encoder() -> EncoderGuard {
    POOL.lock().request_encoder()
}

static POOL: Lazy<Arc<Mutex<Pool>>> = Lazy::new(|| Arc::new(Mutex::new(Pool::new(16))));

struct Pool {
    encoders: Vec<Encoder>,
}

impl Pool {
    fn new(initial_capacity: usize) -> Self {
        Self {
            encoders: Vec::with_capacity(initial_capacity),
        }
    }

    fn request_encoder(&mut self) -> EncoderGuard {
        self.encoders
            .pop()
            .map_or(EncoderGuard::default(), |v| EncoderGuard::new(v))
    }

    fn return_encoder(&mut self, encoder: Encoder) {
        self.encoders.push(encoder)
    }
}

#[derive(Default)]
pub struct EncoderGuard(ManuallyDrop<Encoder>);

impl EncoderGuard {
    pub fn new(encoder: Encoder) -> Self {
        Self(ManuallyDrop::new(encoder))
    }
}

impl Drop for EncoderGuard {
    fn drop(&mut self) {
        let mut pool = POOL.lock();
        pool.return_encoder(unsafe { ManuallyDrop::take(&mut self.0) });
    }
}

impl Deref for EncoderGuard {
    type Target = Encoder;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for EncoderGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
