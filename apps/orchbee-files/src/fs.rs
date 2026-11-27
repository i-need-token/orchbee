//! The file engine: directory pages, entries, read transfers with encoding detection and decoding,
//! and encoding guesses. Its callers pass paths that scope already admitted; it decides nothing
//! about which paths a Client may name, and it never logs a path, a name or file content.

mod background;
mod detection;
mod encoding;
mod engine;
mod guessing;
mod listing;
mod reading;
mod timer;
mod wire;

pub(crate) use engine::Engine;

#[cfg(test)]
mod tests;
