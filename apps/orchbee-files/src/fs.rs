//! The file engine: directory pages and entries, run on helper threads that answer within a
//! deadline. Its callers pass paths that scope and containment already admitted; it decides
//! nothing about which paths a Client may name, and it never logs a path, a name or file content.

mod blocking;
mod listing;
mod metadata;
mod os_error;
mod timer;

#[cfg(test)]
mod tests;
