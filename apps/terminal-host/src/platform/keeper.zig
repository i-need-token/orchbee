//! What the Host's Keeper link reads from a terminal's platform process: its release-first gate,
//! the table of known conpty builds, and on Windows a hold's snapshot of its handles.

const builtin = @import("builtin");
const gate_module = @import("keeper_gate.zig");
const process = @import("process.zig");

pub const Gate = gate_module.Gate;
pub const maximum_attachments = gate_module.maximum_attachments;
pub const builds = @import("conpty_builds.zig");
pub const hold = @import("keeper_hold_windows.zig");

/// The release-first gate of a terminal's process; Windows terminals alone are ever held.
pub fn gate(target: *process.Process) ?*Gate {
    return switch (builtin.os.tag) {
        .windows => &target.termination.keepers,
        else => null,
    };
}
