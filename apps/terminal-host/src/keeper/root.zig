//! The Keeper, `orchbee-terminal-host keep`: one process per Session that holds duplicates of its
//! terminals' handles for the Hosts the Worker registers and arbitrates each move between two
//! Hosts, as `schemas/keeper/README.md` defines. It exists on Windows, macOS and Linux.

const builtin = @import("builtin");

pub const supported = builtin.os.tag == .windows or builtin.os.tag == .macos or builtin.os.tag == .linux;
pub const service = if (supported) @import("service.zig") else struct {};
/// A macOS or Linux Keeper's update in place: the check mode and the image that takes the state
/// over.
pub const check = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("check_posix.zig") else struct {};
pub const resume_state = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("resume_posix.zig") else struct {};
