//! Bounded fake peers and controlled transport clocks for the owning application's tests.
//!
//! The module compiles for this crate's unit tests and, through the `test-support` feature, for
//! the application's development dependency only. Release builds never enable that feature, so
//! these constructors never widen the production transport boundary.
pub mod handshake;
pub mod multiplex_peer;
pub mod server;
#[cfg(windows)]
pub(crate) mod windows_endpoints;

pub use super::{wake_fault_io, wake_ready_io};
