//! What a macOS or Linux Keeper link's hold names: the terminal or background program the dispatch
//! thread queued, and the `hold-pty` that sends its master or the `hold-pipes` that sends its
//! captured streams' read ends. The descriptors move with the request, so nothing outlives an
//! unanswered hold here: a descriptor the Keeper never received closes with the socket.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const message = keeper_protocol.message;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;

/// A hold the dispatch thread queued; its gate slot keeps what it names open until the link
/// settles it.
pub const Queued = struct {
    gate: *Gate,
    holding: [16]u8,
    subject: union(enum) {
        terminal: struct { process: *Process, terminal: message.Terminal },
        program: Program,
    },
};

/// A background program offered for a hold: the read end of each captured stream, and its facts,
/// whose label it borrows from the program.
pub const Program = struct {
    streams: [2]?std.posix.fd_t,
    facts: message.ProgramFacts,
};

/// A hold request and the descriptors it sends.
pub const Prepared = struct {
    body: message.Request.Body,
    descriptors: [2]std.posix.fd_t = undefined,
    count: usize = 0,

    pub fn sent(self: *const Prepared) []const std.posix.fd_t {
        return self.descriptors[0..self.count];
    }
};

/// The hold of what `item` names. A terminal's fails when its leader's start time or its slave's
/// name could not be read at spawn.
pub fn prepare(item: Queued) error{HoldFailed}!Prepared {
    switch (item.subject) {
        .terminal => |terminal| {
            const process = terminal.process;
            const record = process.keeper;
            const slave = record.slaveName() orelse return error.HoldFailed;
            if (record.leader_created == 0 or process.child_id <= 0) return error.HoldFailed;
            return .{
                .body = .{ .hold_pty = .{
                    .holding = item.holding,
                    .facts = .{
                        .terminal = terminal.terminal,
                        .leader = .{ .pid = @intCast(process.child_id), .created = record.leader_created },
                        .slave = slave,
                        .launch = process.launch_kind.wireName(),
                    },
                } },
                .descriptors = .{ process.pty.master, undefined },
                .count = 1,
            };
        },
        .program => |program| {
            var prepared: Prepared = .{ .body = .{ .hold_pipes = .{
                .holding = item.holding,
                .facts = program.facts,
                .capture = .{ .stdout = program.streams[0] != null, .stderr = program.streams[1] != null },
            } } };
            for (program.streams) |stream| if (stream) |descriptor| {
                prepared.descriptors[prepared.count] = descriptor;
                prepared.count += 1;
            };
            return prepared;
        },
    }
}
