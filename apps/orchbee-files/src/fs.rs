//! The file engine: directory pages, entries, read transfers with encoding detection and decoding,
//! encoding guesses, the uploads whose Puts bring an operation its data, and the watches whose
//! changes reach `fs.subscribe`. Its callers pass paths that scope already admitted; it decides
//! nothing about which paths a Client may name, and it never logs a path, a name or file content.

mod background;
mod detection;
mod encoding;
mod engine;
mod guessing;
mod listing;
mod reading;
mod timer;
mod uploads;
mod watch;
mod wire;

pub(crate) use engine::Engine;

#[cfg(test)]
mod tests;
