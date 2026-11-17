//! The Keeper, `orchbee-terminal-host keep`: one process per Session that holds duplicates of its
//! terminals' handles for the Hosts the Worker registers and arbitrates each move between two
//! Hosts, as `schemas/keeper/README.md` defines. It exists on Windows only.

const builtin = @import("builtin");

pub const supported = builtin.os.tag == .windows;
pub const service = if (supported) @import("service.zig") else struct {};
