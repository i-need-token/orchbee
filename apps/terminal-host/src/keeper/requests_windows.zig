//! The Windows Keeper's requests that depend on what a holding holds: a Host's hold of a terminal's
//! seven handles or of a background program's, and the inspection and claim of each.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const content = @import("content_windows.zig");
const handles = @import("handles_windows.zig");
const requests = @import("requests.zig");
const state = @import("state.zig");

const message = keeper_protocol.message;
const Process = message.Process;
const Reason = message.Reason;
const Body = message.Reply.Body;
const Table = state.Table;
const Holding = state.Holding;
const Effects = state.Effects;
const refused = requests.refused;

/// Whether this Keeper serves a request of `kind` at all; a Windows Keeper serves every kind its
/// connection negotiated.
pub fn serves(_: message.RequestKind) bool {
    return true;
}

/// Whether a registered Host may send `request` beyond the requests every Keeper serves.
pub fn hostRequest(request: message.Request.Body) bool {
    return request == .hold or request == .hold_program;
}

pub fn apply(table: *Table, _: std.mem.Allocator, requester: Process, request: message.Request.Body, _: *Effects) error{OutOfMemory}!Body {
    return switch (request) {
        .hold => |hold| holdRequest(table, requester, hold),
        .hold_program => |hold| holdProgramRequest(table, requester, hold),
        else => unreachable,
    };
}

fn holdRequest(table: *Table, requester: Process, hold: message.Hold) Body {
    const position = table.search(hold.holding);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.state() == .held) return .hold;
        return refused(.holding_exists);
    }
    if (table.lifecycle != .ready) return refused(.draining);
    if (table.holding_count >= message.maximum_holdings) return refused(.capacity);
    const stored = content.StoredFacts.store(hold.facts) orelse return refused(.identity_mismatch);
    const host = table.hosts[table.findHost(requester).?].?;
    const duplicates = handles.pull(host.handle, hold.handles) catch return refused(.handle_failed);
    // `inspect` names each duplicate by a value below 2^32, as the wire carries it.
    for (duplicates) |duplicate| if (@intFromPtr(duplicate) > std.math.maxInt(u32)) {
        handles.closeAll(&duplicates);
        return refused(.handle_failed);
    };
    if (!handles.matches(duplicates, hold.facts)) {
        handles.closeAll(&duplicates);
        return refused(.identity_mismatch);
    }
    requests.insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .terminal = .{ .duplicates = duplicates, .facts = stored } },
    });
    return .hold;
}

/// A Host's hold of a background program: the Keeper pulls its Job, its process, and the read end
/// of each captured stream, and checks them as a terminal's hold checks its own.
fn holdProgramRequest(table: *Table, requester: Process, hold: message.HoldProgram) Body {
    const position = table.search(hold.holding);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.state() == .held and holding.content == .program) return .hold_program;
        return refused(.holding_exists);
    }
    if (table.lifecycle != .ready) return refused(.draining);
    if (table.holding_count >= message.maximum_holdings) return refused(.capacity);
    const host = table.hosts[table.findHost(requester).?].?;
    const duplicates = handles.pullProgram(host.handle, hold.handles) catch return refused(.handle_failed);
    // `inspect` names each duplicate by a value below 2^32, as the wire carries it.
    if (duplicates.values() == null) {
        duplicates.close();
        return refused(.handle_failed);
    }
    if (!handles.programMatches(duplicates, hold.facts)) {
        duplicates.close();
        return refused(.identity_mismatch);
    }
    requests.insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .program = .{ .duplicates = duplicates, .facts = .store(hold.facts) } },
    });
    return .hold_program;
}

/// The target's view of a quiesced holding: the value of each duplicate and the recorded facts.
pub fn inspect(holding: *Holding, arena: std.mem.Allocator, _: *Effects) error{OutOfMemory}!Body {
    switch (holding.content) {
        .terminal => |*terminal| {
            var values: [message.Handles.roles.len]u32 = undefined;
            for (terminal.duplicates, &values) |duplicate, *value| value.* = @intCast(@intFromPtr(duplicate));
            return .{ .inspect = .{
                .handles = .{
                    .job = values[0],
                    .program = values[1],
                    .console = values[2],
                    .signal = values[3],
                    .reference = values[4],
                    .input = values[5],
                    .output = values[6],
                },
                .facts = try terminal.facts.view(arena),
            } };
        },
        .program => |*program| {
            // The hold checked that every value fits.
            const values = program.duplicates.values().?;
            return .{ .inspect_program = .{
                .handles = .{
                    .job = values.job,
                    .process = values.process,
                    .stdout = values.streams[0],
                    .stderr = values.streams[1],
                },
                .facts = try program.facts.view(arena),
            } };
        },
    }
}

/// Checks a claim against what the holding holds and records the claimant's terminal: a terminal's
/// claim names the claimant's terminal; a program's names none.
pub fn claim(holding: *Holding, request: message.Claim) ?Reason {
    switch (holding.content) {
        .terminal => |*terminal| {
            const identity = request.terminal orelse return .identity_mismatch;
            if (!request.program.eql(terminal.facts.program)) return .identity_mismatch;
            terminal.facts.terminal = identity;
        },
        .program => |*program| {
            if (request.terminal != null or !request.program.eql(program.facts.process)) return .identity_mismatch;
        },
    }
    return null;
}

/// A released or discarded holding needs nothing more than its duplicates closed.
pub fn released(_: *Table, _: *Holding) ?Reason {
    return null;
}
