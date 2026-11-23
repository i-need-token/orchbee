//! What a Keeper link's hold names: the terminal or background program the dispatch thread queued,
//! the request that holds it, and this Host's own duplicates of its handles, which keep the values
//! the request names valid until the Keeper answered and, for an unanswered hold, until it ends.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const message = keeper_protocol.message;
const Gate = platform.keeper.Gate;
const hold_snapshot = platform.keeper.hold;
const handles_module = platform.keeper.handles;
const Process = platform.process.Process;

const windows = std.os.windows;
const HANDLE = windows.HANDLE;

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

/// A background program offered for a hold: its Job, its process, the read end of each captured
/// stream, and its facts, whose label it borrows from the program.
pub const Program = struct {
    job: HANDLE,
    process: HANDLE,
    streams: [2]?HANDLE,
    facts: message.ProgramFacts,
};

/// The duplicates an unanswered hold named, which the Keeper may still pull.
pub const Retained = union(enum) {
    terminal: hold_snapshot.Snapshot,
    program: handles_module.ProgramDuplicates,

    pub fn close(self: *Retained) void {
        switch (self.*) {
            .terminal => |*snapshot| snapshot.close(),
            .program => |duplicates| duplicates.close(),
        }
    }
};

/// A hold request and the duplicates its values name.
pub const Prepared = struct {
    body: message.Request.Body,
    retained: Retained,
};

/// Takes this Host's duplicates of what `item` names and the request that holds them.
pub fn prepare(item: Queued) error{ SystemConsole, HoldFailed }!Prepared {
    switch (item.subject) {
        .terminal => |terminal| {
            var snapshot = try hold_snapshot.take(terminal.process);
            const handles = handleValues(snapshot.handles) orelse {
                snapshot.close();
                return error.HoldFailed;
            };
            const own = platform.keeper.builds.own;
            return .{ .retained = .{ .terminal = snapshot }, .body = .{ .hold = .{
                .holding = item.holding,
                .handles = handles,
                .facts = .{
                    .terminal = terminal.terminal,
                    .program = .{ .pid = snapshot.program.pid, .created = snapshot.program.created },
                    .console = .{ .pid = snapshot.console.pid, .created = snapshot.console.created },
                    .console_build = own.console_sha256,
                    .signal_format = own.signal_format,
                    .launch = launchName(snapshot.launch_kind),
                },
            } } };
        },
        .program => |program| {
            const duplicates = duplicateProgram(program) catch return error.HoldFailed;
            const values = duplicates.values() orelse {
                duplicates.close();
                return error.HoldFailed;
            };
            return .{ .retained = .{ .program = duplicates }, .body = .{ .hold_program = .{
                .holding = item.holding,
                .handles = .{
                    .job = values.job,
                    .process = values.process,
                    .stdout = values.streams[0],
                    .stderr = values.streams[1],
                },
                .facts = program.facts,
            } } };
        },
    }
}

/// This Host's own duplicates of a program's handles.
fn duplicateProgram(program: Program) error{HandleFailed}!handles_module.ProgramDuplicates {
    const job = try handles_module.duplicateOwn(program.job);
    errdefer handles_module.close(job);
    const process = try handles_module.duplicateOwn(program.process);
    errdefer handles_module.close(process);
    var streams: [2]?HANDLE = .{ null, null };
    errdefer for (streams) |stream| if (stream) |handle| handles_module.close(handle);
    for (program.streams, &streams) |source, *stream| if (source) |handle| {
        stream.* = try handles_module.duplicateOwn(handle);
    };
    return .{ .job = job, .process = process, .streams = streams };
}

/// The wire values of the snapshot's duplicates; null when one exceeds 2^32, which the Keeper
/// would refuse.
fn handleValues(handles: [hold_snapshot.role_count]HANDLE) ?message.Handles {
    var values: [hold_snapshot.role_count]u32 = undefined;
    for (handles, &values) |handle, *value| value.* = std.math.cast(u32, @intFromPtr(handle)) orelse return null;
    return .{
        .job = values[0],
        .program = values[1],
        .console = values[2],
        .signal = values[3],
        .reference = values[4],
        .input = values[5],
        .output = values[6],
    };
}

fn launchName(kind: platform.process.LaunchKind) []const u8 {
    return switch (kind) {
        inline else => |tag| comptime blk: {
            const text = @tagName(tag);
            var spelled: [text.len]u8 = undefined;
            for (text, &spelled) |byte, *out| out.* = if (byte == '_') '-' else byte;
            const final = spelled;
            break :blk &final;
        },
    };
}
