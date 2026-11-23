//! What a macOS Keeper link's hold names: the terminal the dispatch thread queued and the
//! `hold-pty` that sends its master. The descriptor moves with the request, so nothing outlives an
//! unanswered hold here: a master the Keeper never received closes with the socket.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const message = keeper_protocol.message;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;

/// A hold the dispatch thread queued; its gate slot keeps the master open until the link settles
/// it.
pub const Queued = struct {
    gate: *Gate,
    holding: [16]u8,
    process: *Process,
    terminal: message.Terminal,
};

/// A hold request and the descriptor it sends.
pub const Prepared = struct {
    body: message.Request.Body,
    master: std.posix.fd_t,
};

/// The `hold-pty` of `item`'s terminal. Fails when its leader's start time or its slave's name
/// could not be read at spawn.
pub fn prepare(item: Queued) error{HoldFailed}!Prepared {
    const process = item.process;
    const record = process.keeper;
    const slave = record.slaveName() orelse return error.HoldFailed;
    if (record.leader_created == 0 or process.child_id <= 0) return error.HoldFailed;
    return .{
        .body = .{ .hold_pty = .{
            .holding = item.holding,
            .facts = .{
                .terminal = item.terminal,
                .leader = .{ .pid = @intCast(process.child_id), .created = record.leader_created },
                .slave = slave,
                .launch = process.launch_kind.wireName(),
            },
        } },
        .master = process.pty.master,
    };
}
