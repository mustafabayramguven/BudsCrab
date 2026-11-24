//! budscrab-core: everything between the socket and the UI.
//!
//! Kotlin owns the Android `BluetoothSocket` and the UI; this crate owns the
//! protocol state machine, custom gestures, battery history, Wi-Fi profiles
//! and diagnostics. The API is synchronous and returns `CoreEvent`s.

uniffi::setup_scaffolding!();

pub mod session;
pub mod types;

pub use session::{decode_hex, Session};
pub use types::*;

#[cfg(test)]
mod tests;
