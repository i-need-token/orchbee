//! The Keeper's requests, each applied to the table with its lock held: the Worker's registrations,
//! `status`, and recovery requests, and a registered Host's holding requests along the contract's
//! state table. A program holding follows the same table as a terminal's; only its hold, its
//! inspection, and its claim differ.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const handles = @import("handles_windows.zig");
const state = @import("state.zig");

const message = keeper_protocol.message;
const Process = message.Process;
const RandomId = message.RandomId;
const Reason = message.Reason;
const Body = message.Reply.Body;
const Table = state.Table;
const Holding = state.Holding;
const Effects = state.Effects;

/// Applies one request from `peer` and returns its reply body; slices live in `arena`.
pub fn apply(
    table: *Table,
    arena: std.mem.Allocator,
    peer: state.Peer,
    request: message.Request.Body,
    effects: *Effects,
) error{OutOfMemory}!Body {
    switch (peer) {
        .worker => |worker| return switch (request) {
            .register_host => |process| registerHost(table, process),
            .unregister_host => |process| unregisterHost(table, process, effects),
            .status => |after| status(table, arena, after, worker.programs),
            .assign => |assign| assignRequest(table, assign),
            .discard => |holding| discardRequest(table, holding, effects),
            else => refused(.wrong_role),
        },
        .host => |requester| {
            switch (request) {
                .hold, .hold_program, .mark, .@"resume", .inspect, .claim, .decline, .release => {},
                else => return refused(.wrong_role),
            }
            // A Host forgotten while its request was in flight sends nothing more.
            if (table.findHost(requester) == null) return refused(.not_registered);
            return switch (request) {
                .hold => |hold| holdRequest(table, requester, hold),
                .hold_program => |hold| holdProgramRequest(table, requester, hold),
                .mark => |mark| markRequest(table, requester, mark),
                .@"resume" => |marked| resumeRequest(table, requester, marked),
                .inspect => |marked| try inspectRequest(table, arena, requester, marked),
                .claim => |claim| claimRequest(table, requester, claim, effects),
                .decline => |marked| declineRequest(table, requester, marked),
                .release => |holding| releaseRequest(table, requester, holding, effects),
                else => unreachable,
            };
        },
    }
}

fn registerHost(table: *Table, process: Process) Body {
    if (table.findHost(process) != null) return .register_host;
    if (table.lifecycle != .ready) return refused(.draining);
    if (table.registered_hosts >= message.maximum_hosts) return refused(.capacity);
    const index = for (table.hosts, 0..) |slot, index| {
        if (slot == null) break index;
    } else return refused(.capacity);
    const handle = handles.openHost(process) catch |err| return refused(switch (err) {
        error.HandleFailed => .handle_failed,
        error.IdentityMismatch => .identity_mismatch,
    });
    const context = (@as(usize, table.host_generations[index]) << 16) | index;
    const wait = handles.watchEnd(handle, state.hostEnded, context) orelse {
        handles.close(handle);
        return refused(.handle_failed);
    };
    table.hosts[index] = .{ .process = process, .handle = handle, .wait = wait };
    table.registered_hosts += 1;
    return .register_host;
}

fn unregisterHost(table: *Table, process: Process, effects: *Effects) Body {
    const index = table.findHost(process) orelse return .unregister_host;
    for (table.holdings[0..table.holding_count]) |*holding| {
        if (holding.owner.eql(process)) return refused(.holding_exists);
        if (holding.mark) |mark| if (mark.target.eql(process)) return refused(.holding_exists);
    }
    for (table.holdings[0..table.holding_count]) |*holding| {
        if (holding.claimed) |claimed| if (claimed.from.eql(process)) {
            holding.claimed = null;
        };
    }
    for (&table.connections) |*slot| {
        if (slot.in_use and slot.kind == .host and slot.pid == process.pid) {
            slot.closing = true;
            handles.signal(slot.wake.?);
        }
    }
    effects.host = table.forget(index);
    return .unregister_host;
}

/// One page of holdings after `after`; program holdings only for a Worker that negotiated
/// `programs`, since another reads every entry as a terminal's.
fn status(table: *Table, arena: std.mem.Allocator, after: ?RandomId, programs: bool) error{OutOfMemory}!Body {
    var start: usize = 0;
    if (after) |last| {
        while (start < table.holding_count and
            !std.mem.lessThan(u8, &last, &table.holdings[start].id)) start += 1;
    }
    var listed: std.ArrayList(message.HoldingEntry) = try .initCapacity(arena, message.maximum_status_entries);
    var more = false;
    for (table.holdings[start..table.holding_count]) |*holding| {
        if (!programs and holding.content == .program) continue;
        if (listed.items.len == message.maximum_status_entries) {
            more = true;
            break;
        }
        listed.appendAssumeCapacity(.{
            .holding = holding.id,
            .owner = holding.owner,
            .state = holding.state(),
            .target = if (holding.mark) |mark| mark.target else null,
            .facts = try holding.content.view(arena),
        });
    }
    const entries = listed.items;
    var hosts: ?[]message.HostEntry = null;
    if (after == null) {
        const list = try arena.alloc(message.HostEntry, table.registered_hosts);
        var count: usize = 0;
        for (table.hosts) |slot| if (slot) |host| {
            list[count] = .{ .process = host.process, .ended = host.ended };
            count += 1;
        };
        hosts = list[0..count];
    }
    const next: ?RandomId = if (more) entries[entries.len - 1].holding else null;
    return .{ .status = .{ .holdings = entries, .hosts = hosts, .next = next } };
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
    const stored = state.StoredFacts.store(hold.facts) orelse return refused(.identity_mismatch);
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
    insert(table, position.index, .{
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
    insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .program = .{ .duplicates = duplicates, .facts = .store(hold.facts) } },
    });
    return .hold_program;
}

fn insert(table: *Table, index: usize, holding: Holding) void {
    std.mem.copyBackwards(
        Holding,
        table.holdings[index + 1 .. table.holding_count + 1],
        table.holdings[index..table.holding_count],
    );
    table.holdings[index] = holding;
    table.holding_count += 1;
}

fn markRequest(table: *Table, requester: Process, mark: message.Mark) Body {
    const holding = table.find(mark.holding) orelse return refused(.unknown_holding);
    if (!holding.owner.eql(requester)) return refused(.not_owner);
    if (holding.mark) |current| {
        if (std.mem.eql(u8, &current.id, &mark.mark) and current.target.eql(mark.target)) return .mark;
        return refused(.marked);
    }
    if (mark.target.eql(requester)) return refused(.not_registered);
    const target = table.findHost(mark.target) orelse return refused(.not_registered);
    if (table.hosts[target].?.ended) return refused(.not_registered);
    holding.mark = .{ .id = mark.mark, .target = mark.target };
    return .mark;
}

fn resumeRequest(table: *Table, requester: Process, marked: message.Marked) Body {
    const holding = table.find(marked.holding) orelse return refused(.unknown_holding);
    if (consumedBy(holding, marked.mark)) |claimed| {
        if (claimed.from.eql(requester)) return .{ .@"resume" = .claimed };
    }
    if (!holding.owner.eql(requester)) return refused(.not_owner);
    if (holding.mark) |current| {
        if (!std.mem.eql(u8, &current.id, &marked.mark)) return refused(.stale_mark);
        holding.mark = null;
    }
    return .{ .@"resume" = .resumed };
}

fn inspectRequest(
    table: *Table,
    arena: std.mem.Allocator,
    requester: Process,
    marked: message.Marked,
) error{OutOfMemory}!Body {
    const holding = table.find(marked.holding) orelse return refused(.unknown_holding);
    if (currentMark(holding, marked.mark) == null) return refused(.stale_mark);
    if (!holding.mark.?.target.eql(requester)) return refused(.not_target);
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

fn claimRequest(table: *Table, requester: Process, claim: message.Claim, effects: *Effects) Body {
    const holding = table.find(claim.holding) orelse return refused(.unknown_holding);
    if (consumedBy(holding, claim.mark) != null and holding.owner.eql(requester)) return .claim;
    if (currentMark(holding, claim.mark) == null) return refused(.stale_mark);
    if (!holding.mark.?.target.eql(requester)) return refused(.not_target);
    // A terminal's claim names the claimant's terminal; a program's names none.
    switch (holding.content) {
        .terminal => |*terminal| {
            const identity = claim.terminal orelse return refused(.identity_mismatch);
            if (!claim.program.eql(terminal.facts.program)) return refused(.identity_mismatch);
            terminal.facts.terminal = identity;
        },
        .program => |*program| {
            if (claim.terminal != null or !claim.program.eql(program.facts.process)) return refused(.identity_mismatch);
        },
    }
    const previous = holding.owner;
    holding.owner = requester;
    holding.claimed = .{ .from = previous, .mark = claim.mark };
    holding.mark = null;
    holding.orphaned = false;
    effects.host = table.forgetIfEndedAndUnnamed(previous);
    return .claim;
}

fn declineRequest(table: *Table, requester: Process, marked: message.Marked) Body {
    const holding = table.find(marked.holding) orelse return refused(.unknown_holding);
    if (currentMark(holding, marked.mark)) |current| {
        if (!current.target.eql(requester)) return refused(.not_target);
        // A quiesced holding returns to its owner; an orphaned one stays orphaned.
        holding.mark = null;
        return .{ .decline = .declined };
    }
    if (consumedBy(holding, marked.mark) != null and holding.owner.eql(requester)) {
        return .{ .decline = .claimed };
    }
    return .{ .decline = .declined };
}

fn releaseRequest(table: *Table, requester: Process, id: RandomId, effects: *Effects) Body {
    const position = table.search(id);
    if (!position.found) return .release;
    const holding = &table.holdings[position.index];
    if (!holding.owner.eql(requester)) return refused(.not_owner);
    if (holding.mark != null) return refused(.marked);
    effects.released = holding.content;
    std.mem.copyForwards(
        Holding,
        table.holdings[position.index .. table.holding_count - 1],
        table.holdings[position.index + 1 .. table.holding_count],
    );
    table.holding_count -= 1;
    return .release;
}

/// Recovery: the Worker names the live Host that may claim an orphaned holding, under a mark it
/// mints, while no live Host may still claim it.
fn assignRequest(table: *Table, assign: message.Mark) Body {
    const holding = table.find(assign.holding) orelse return refused(.unknown_holding);
    if (!holding.orphaned) return refused(.not_orphaned);
    if (holding.mark) |current| {
        if (std.mem.eql(u8, &current.id, &assign.mark) and current.target.eql(assign.target)) return .assign;
        if (liveHost(table, current.target)) return refused(.marked);
    }
    if (!liveHost(table, assign.target)) return refused(.not_registered);
    holding.mark = .{ .id = assign.mark, .target = assign.target };
    return .assign;
}

/// Closes an orphaned holding's duplicates, which ends its program once no other process holds its
/// Job, while no live Host may still claim it.
fn discardRequest(table: *Table, id: RandomId, effects: *Effects) Body {
    const position = table.search(id);
    if (!position.found) return .discard;
    const holding = &table.holdings[position.index];
    if (!holding.orphaned) return refused(.not_orphaned);
    if (holding.mark) |current| if (liveHost(table, current.target)) return refused(.marked);
    const owner = holding.owner;
    effects.released = holding.content;
    std.mem.copyForwards(
        Holding,
        table.holdings[position.index .. table.holding_count - 1],
        table.holdings[position.index + 1 .. table.holding_count],
    );
    table.holding_count -= 1;
    effects.host = table.forgetIfEndedAndUnnamed(owner);
    return .discard;
}

/// Whether `process` is a registered Host that has not ended.
fn liveHost(table: *const Table, process: Process) bool {
    const index = table.findHost(process) orelse return false;
    return !table.hosts[index].?.ended;
}

fn refused(reason: Reason) Body {
    return .{ .refused = reason };
}

fn currentMark(holding: *const Holding, id: RandomId) ?state.Mark {
    const mark = holding.mark orelse return null;
    return if (std.mem.eql(u8, &mark.id, &id)) mark else null;
}

fn consumedBy(holding: *const Holding, id: RandomId) ?state.Claimed {
    const claimed = holding.claimed orelse return null;
    return if (std.mem.eql(u8, &claimed.mark, &id)) claimed else null;
}
