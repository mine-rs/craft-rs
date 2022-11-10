//! # craft-rs
//!
//! A project aimed towards providing utilities atop mine-rs to aid in building
//! Clients and Servers for Minecraft.
#[cfg(feature = "auth")]
pub use craftrs_auth as auth;
#[cfg(feature = "level")]
pub use craftrs_level as level;
#[cfg(feature = "net")]
pub use craftrs_net as net;

