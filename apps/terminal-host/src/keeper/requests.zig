//! The Keeper's requests, each applied to the table with its lock held: the Worker's registrations
//! and `status`, and a registered Host's holding requests along the contract's state table.

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
        .worker => return switch (request) {
            .register_host => |process| registerHost(table, process),
            .unregister_host => |process| unregisterHost(table, process, effects),
            .status => |after| status(table, arena, after),
            else => refused(.wrong_role),
        },
        .host => |requester| {
            switch (request) {
                .hold, .mark, .@"resume", .inspect, .claim, .decline, .release => {},
                else => return refused(.wrong_role),
            }
            // A Host forgotten while its request was in flight sends nothing more.
            if (table.findHost(requester) == null) return refused(.not_registered);
            return switch (request) {
                .hold => |hold| holdRequest(table, requester, hold),
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

fn status(table: *Table, arena: std.mem.Allocator, after: ?RandomId) error{OutOfMemory}!Body {
    var start: usize = 0;
    if (after) |last| {
        while (start < table.holding_count and
            !std.mem.lessThan(u8, &last, &table.holdings[start].id)) start += 1;
    }
    const end = @min(table.holding_count, start + message.maximum_status_entries);
    const entries = try arena.alloc(message.HoldingEntry, end - start);
    for (table.holdings[start..end], entries) |*holding, *entry| {
        entry.* = .{
            .holding = holding.id,
            .owner = holding.owner,
            .state = holding.state(),
            .target = if (holding.mark) |mark| mark.target else null,
            .facts = try holding.facts.view(arena),
        };
    }
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
    const next: ?RandomId = if (end < table.holding_count) table.holdings[end - 1].id else null;
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
    const index = position.index;
    std.mem.copyBackwards(
        Holding,
        table.holdings[index + 1 .. table.holding_count + 1],
        table.holdings[index..table.holding_count],
    );
    table.holdings[index] = .{
        .id = hold.holding,
        .owner = requester,
        .duplicates = duplicates,
        .facts = stored,
    };
    table.holding_count += 1;
    return .hold;
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
    var values: [message.Handles.roles.len]u32 = undefined;
    for (holding.duplicates, &values) |duplicate, *value| value.* = @intCast(@intFromPtr(duplicate));
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
        .facts = try holding.facts.view(arena),
    } };
}

fn claimRequest(table: *Table, requester: Process, claim: message.Claim, effects: *Effects) Body {
    const holding = table.find(claim.holding) orelse return refused(.unknown_holding);
    if (consumedBy(holding, claim.mark) != null and holding.owner.eql(requester)) return .claim;
    if (currentMark(holding, claim.mark) == null) return refused(.stale_mark);
    if (!holding.mark.?.target.eql(requester)) return refused(.not_target);
    if (!claim.program.eql(holding.facts.program)) return refused(.identity_mismatch);
    const previous = holding.owner;
    holding.owner = requester;
    holding.facts.terminal = claim.terminal;
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
    effects.duplicates = holding.duplicates;
    std.mem.copyForwards(
        Holding,
        table.holdings[position.index .. table.holding_count - 1],
        table.holdings[position.index + 1 .. table.holding_count],
    );
    table.holding_count -= 1;
    return .release;
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
