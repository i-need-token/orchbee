//! Orchbee's closed release records: strict JSON reading, the version and digest tokens the records
//! admit, and the release store's active-version pointer with its fallback record. Nothing here
//! reads a file, takes a lock or starts a process; callers own every location and effect.
mod fallback;
mod pointer;
mod strict_json;
mod tokens;

pub use fallback::{
    ClientWindow, FALLBACK_FAILURES, FALLBACK_WINDOW_MILLISECONDS, Fallback, Move, SessionWindow,
    client_failure_helper, valid_activation_id,
};
pub use pointer::{Entry, Pointer};
pub use strict_json::{check_extensions, closed_object, parse_object};
pub use tokens::{lowercase_sha256, valid_version};
