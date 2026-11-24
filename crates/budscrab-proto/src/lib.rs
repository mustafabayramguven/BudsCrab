//! budscrab-proto: Samsung Galaxy Buds Core (SM-R410) RFCOMM protocol.
//!
//! Pure Rust, no I/O, no dependencies. The transport (Android `BluetoothSocket`,
//! BlueZ on Linux, ...) feeds bytes into [`FrameDecoder`] and writes the bytes
//! produced by [`Command::encode`].
//!
//! Protocol knowledge comes from GalaxyBudsClient by Tim Schneeberger
//! (GPL-3.0), <https://github.com/timschneeb/GalaxyBudsClient>.

pub mod frame;
pub mod ids;
pub mod message;
pub mod model;

pub use frame::{crc16, scan, Frame, FrameDecoder, FrameError};
pub use message::{Command, ExtendedStatus, Incoming, Status};
pub use model::*;

/// RFCOMM service UUID used by Buds2 and newer (incl. Buds Core).
pub const SPP_UUID: &str = "2e73a4ad-332d-41fc-90e2-16bef06523f2";
/// Firmware model prefix of the Buds Core.
pub const MODEL_PREFIX: &str = "R410";
