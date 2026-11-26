//! A terminal that moves between Hosts, as its POSIX process sees it; only macOS and Linux Hosts
//! move or recover terminals. The new Host adopts a moved or recovered terminal's PTY master and
//! leader, which is not its child; the old Host drops a moved terminal and the new Host discards an
//! import, each closing only its own descriptors; and a settled terminal restored from a record
//! keeps no process at all.

const std = @import("std");
const builtin = @import("builtin");
const process_types = @import("process_types.zig");
const pty_module = @import("pty.zig");
const keeper_record = if (builtin.os.tag == .macos or builtin.os.tag == .linux) @import("process_keeper_posix.zig") else struct {};
const Process = @import("process_posix.zig").Process;

const posix_keeper = builtin.os.tag == .macos or builtin.os.tag == .linux;

/// A moved or recovered terminal's PTY master and leader, as the claim checked them.
pub const Held = if (posix_keeper) keeper_record.Held else void;

/// The process record of a moved or recovered terminal: its leader runs on, not this Host's child.
/// On error the caller still owns the master.
pub fn adopt(allocator: std.mem.Allocator, held: Held) !Process {
    if (comptime !posix_keeper) unreachable;
    var pty = try pty_module.Pty.adopt(held.master);
    errdefer {
        pty.master = -1;
        pty.deinit();
    }
    const keeper = try keeper_record.Record.create(allocator, held.master);
    keeper.leader_created = held.leader.created;
    keeper.claims_parent = held.parented;
    return .{
        .child_id = held.leader.pid,
        .allocator = allocator,
        .keeper = keeper,
        .adopted = true,
        .pty = pty,
        .interactive_shell = held.interactive_shell,
        .launch_kind = held.launch_kind,
    };
}

/// Wakes the wait for the leader without its end, so a terminal that leaves this Host joins it.
pub fn stopWait(self: *Process) void {
    if (comptime !posix_keeper) unreachable;
    self.keeper.stop();
}

/// Frees a terminal that leaves this Host, moved or recovered, closing only its own descriptors:
/// its leader runs on, and nothing is reaped or signalled. A leader this Host forked stays its
/// child until it exits, and `launchd` then reaps it.
pub fn drop(self: *Process) void {
    if (comptime !posix_keeper) unreachable;
    if (self.inert) {
        self.* = undefined;
        return;
    }
    self.keeper.destroy(self.allocator);
    self.pty.deinit();
    self.* = undefined;
}

/// The process record of a settled terminal restored from a record: its leader ended on the old
/// Host, so nothing here is open, watched, or ended, and no Keeper holds it.
pub fn settled(launch_kind: process_types.LaunchKind, interactive_shell: bool) Process {
    return .{
        .child_id = 0,
        .allocator = undefined,
        .keeper = undefined,
        .pty = undefined,
        .interactive_shell = interactive_shell,
        .launch_kind = launch_kind,
        .exit_reaped = true,
        .inert = true,
    };
}
