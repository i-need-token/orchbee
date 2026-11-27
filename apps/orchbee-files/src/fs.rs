//! The file engine: directory pages and entries, and read transfers with encoding detection, run
//! on helper threads that answer within a deadline. Its callers pass paths that scope and containment already admitted; it decides
//! nothing about which paths a Client may name, and it never logs a path, a name or file content.

mod blocking;
mod detection;
mod encoding;
mod engine;
mod guessing;
mod listing;
mod metadata;
mod os_error;
mod reading;
mod timer;
mod wire;

#[cfg(test)]
mod tests;
