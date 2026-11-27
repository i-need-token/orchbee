//! The file engine: directory pages, entries, read transfers with encoding detection and decoding,
//! encoding guesses, and the uploads whose Puts bring an operation its data. Its callers pass paths
//! that scope already admitted; it decides nothing about which paths a Client may name, and it
//! never logs a path, a name or file content.

mod background;
mod detection;
mod encoding;
mod engine;
mod guessing;
mod listing;
mod reading;
mod timer;
mod uploads;
mod wire;

pub(crate) use engine::Engine;

#[cfg(test)]
mod tests;
