//! Orchbee's closed release records: strict JSON reading, the version and digest tokens the records
//! admit, and the release store's active-version pointer. Nothing here reads a file, takes a lock or
//! starts a process; callers own every location and effect.
mod pointer;
mod strict_json;
mod tokens;

pub use pointer::{Entry, Pointer};
pub use strict_json::{check_extensions, closed_object, parse_object};
pub use tokens::{lowercase_sha256, valid_version};
