//! What the Keeper and the Host's Keeper link read from a terminal's platform process: its
//! release-first gate, the table of known conpty builds, on Windows a hold's snapshot of its
//! handles and the operations on a holding's pulled duplicates, and on macOS descriptor passing
//! and the leader's start.

const builtin = @import("builtin");
const gate_module = @import("keeper_gate.zig");
const process = @import("process.zig");

pub const Gate = gate_module.Gate;
pub const maximum_attachments = gate_module.maximum_attachments;
pub const builds = @import("conpty_builds.zig");
pub const hold = @import("keeper_hold_windows.zig");
/// The seven handles of a holding as the process that pulls them sees them.
pub const handles = @import("keeper_handles_windows.zig");
/// Descriptor passing, process identity, and PTY checks on macOS.
pub const darwin = if (builtin.os.tag == .macos) @import("keeper_darwin.zig") else struct {};
/// A terminal leader's start, which the macOS Keeper runs for a Host.
pub const fork_launch = if (builtin.os.tag == .macos) @import("fork_launch_posix.zig") else struct {};

/// Whether this Host can take a moved terminal: its bundled conpty packs a held console.
pub fn takesMoves() bool {
    return switch (builtin.os.tag) {
        .windows => @import("conpty_windows.zig").packs(),
        else => false,
    };
}

/// The release-first gate of a terminal's process; Windows terminals alone are ever held.
pub fn gate(target: *process.Process) ?*Gate {
    return switch (builtin.os.tag) {
        .windows => &target.termination.keepers,
        else => null,
    };
}
