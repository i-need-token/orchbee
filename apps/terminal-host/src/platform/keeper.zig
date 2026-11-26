//! What the Keeper and the Host's Keeper link read from a terminal's platform process: its
//! release-first gate, the table of known conpty builds, on Windows a hold's snapshot of its
//! handles and the operations on a holding's pulled duplicates, and on macOS and Linux descriptor
//! passing and the leader's start.

const builtin = @import("builtin");
const gate_module = @import("keeper_gate.zig");
const process = @import("process.zig");

pub const Gate = gate_module.Gate;
pub const Wake = gate_module.Wake;
pub const maximum_attachments = gate_module.maximum_attachments;
/// Where a Keeper holds this platform's terminals.
pub const supported = gate_module.supported;
pub const builds = @import("conpty_builds.zig");
pub const hold = @import("keeper_hold_windows.zig");
/// The seven handles of a holding as the process that pulls them sees them.
pub const handles = @import("keeper_handles_windows.zig");
/// Descriptor passing, process identity, and PTY checks on macOS and Linux.
pub const native = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("keeper_posix.zig") else struct {};
/// A macOS or Linux terminal's Keeper record and the Keeper's start of its leader.
pub const parent = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("process_keeper_posix.zig") else struct {};
/// A terminal leader's start, which the macOS and Linux Keeper runs for a Host.
pub const fork_launch = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("fork_launch_posix.zig") else struct {};

/// Whether this Host can take a moved terminal: its bundled conpty packs a held console.
pub fn takesMoves() bool {
    return switch (builtin.os.tag) {
        .windows => @import("conpty_windows.zig").packs(),
        .linux, .macos => true,
        else => false,
    };
}

/// The release-first gate of a terminal's process; Windows, macOS and Linux terminals alone are
/// held.
pub fn gate(target: *process.Process) ?*Gate {
    return switch (builtin.os.tag) {
        .windows => &target.termination.keepers,
        // A settled terminal restored from a record has no record of its own.
        .linux, .macos => if (target.inert) null else &target.keeper.gate,
        else => null,
    };
}
