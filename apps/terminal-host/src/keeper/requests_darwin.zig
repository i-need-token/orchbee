//! The macOS Keeper's requests that depend on what a holding holds: a Host's `hold-pty` of a PTY
//! master it forked a leader on and `hold-pipes` of a background program it started, its `spawn`
//! of a leader and `spawn-program` of a program the Keeper parents, the owner's `reap` of an ended
//! parented leader, and the inspection, claim, and release of a holding. A spawn applies in three
//! steps: it reserves its holding under the lock, forks outside it, and inserts the holding under
//! the lock again, so no fork waits with the lock held.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const content = @import("content_darwin.zig");
const handles = @import("handles_darwin.zig");
const requests = @import("requests.zig");
const state = @import("state.zig");

const darwin = platform.keeper.darwin;
const fork_launch = platform.keeper.fork_launch;
const posix_compat = platform.posix_compat;
const message = keeper_protocol.message;
const pty = message.pty;
const Process = message.Process;
const RandomId = message.RandomId;
const Reason = message.Reason;
const Body = message.Reply.Body;
const Table = state.Table;
const Holding = state.Holding;
const Effects = state.Effects;
const refused = requests.refused;
const posix = std.posix;

/// Whether this Keeper serves a request of `kind` at all; a macOS Keeper holds no Win32 handles.
pub fn serves(kind: message.RequestKind) bool {
    return kind != .hold and kind != .hold_program;
}

/// Whether a registered Host may send `request` beyond the requests every Keeper serves. A spawn
/// never reaches the table's apply: its connection runs it in steps.
pub fn hostRequest(request: message.Request.Body) bool {
    return request == .hold_pty or request == .hold_pipes or request == .reap;
}

pub fn apply(table: *Table, _: std.mem.Allocator, requester: Process, request: message.Request.Body, effects: *Effects) error{OutOfMemory}!Body {
    return switch (request) {
        .hold_pty => |hold| holdPty(table, requester, hold, effects),
        .hold_pipes => |hold| holdPipes(table, requester, hold, effects),
        .reap => |holding| reap(table, requester, holding),
        else => unreachable,
    };
}

/// A Host's hold of the master of a terminal whose leader it forked: the master must be a PTY
/// master with the recorded slave, and the leader must run with the recorded start time.
fn holdPty(table: *Table, requester: Process, hold: pty.HoldPty, effects: *Effects) Body {
    const position = table.search(hold.holding);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.state() == .held and holding.content == .pty and !holding.content.pty.parent.parented) return .hold_pty;
        return refused(.holding_exists);
    }
    if (admits(table, hold.holding)) |reason| return refused(reason);
    const stored = content.StoredPtyFacts.store(hold.facts) orelse return refused(.identity_mismatch);
    if (effects.descriptors.count != 1) return refused(.handle_failed);
    const master = effects.descriptors.received[0];
    if (!darwin.masterOf(master, hold.facts.slave)) return refused(.identity_mismatch);
    if (darwin.processStart(hold.facts.leader.pid) != hold.facts.leader.created) return refused(.identity_mismatch);
    requests.insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .pty = .{ .master = effects.descriptors.take(0), .facts = stored, .parent = .{ .parented = false } } },
    });
    return .hold_pty;
}

/// A Host's hold of a background program it started itself: each descriptor must be a pipe's read
/// end, and the program must run with the recorded start time.
fn holdPipes(table: *Table, requester: Process, hold: pty.HoldPipes, effects: *Effects) Body {
    const position = table.search(hold.holding);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.state() == .held and holding.content == .pipes and !holding.content.pipes.parent.parented) return .hold_pipes;
        return refused(.holding_exists);
    }
    if (admits(table, hold.holding)) |reason| return refused(reason);
    const count = hold.capture.count();
    if (effects.descriptors.count != count) return refused(.handle_failed);
    for (effects.descriptors.received[0..count]) |descriptor| if (!darwin.pipeEnd(descriptor, false)) return refused(.identity_mismatch);
    if (darwin.processStart(hold.facts.process.pid) != hold.facts.process.created) return refused(.identity_mismatch);
    requests.insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .pipes = .{
            .streams = takeStreams(&effects.descriptors, hold.capture, 1),
            .facts = .store(hold.facts),
            .parent = .{ .parented = false },
        } },
    });
    return .hold_pipes;
}

/// Why a new holding cannot be made now, or null when it can.
fn admits(table: *Table, id: RandomId) ?Reason {
    if (table.extra.reserved(id)) return .holding_exists;
    if (table.lifecycle != .ready) return .draining;
    if (table.holding_count + table.extra.spawning_count >= message.maximum_holdings) return .capacity;
    return null;
}

/// Takes the read end of each captured stream from `descriptors`, where each stream sends `stride`
/// descriptors, its read end first.
fn takeStreams(descriptors: *content.Descriptors, capture: pty.Capture, stride: usize) [2]?posix.fd_t {
    var streams: [2]?posix.fd_t = .{ null, null };
    var at: usize = 0;
    for ([_]bool{ capture.stdout, capture.stderr }, &streams) |captured, *stream| if (captured) {
        stream.* = descriptors.take(at);
        at += stride;
    };
    return streams;
}

/// The owner's reap of its ended parented leader, once it has contained the leader's session.
fn reap(table: *Table, requester: Process, id: RandomId) Body {
    const holding = table.find(id) orelse return refused(.unknown_holding);
    if (!holding.owner.eql(requester)) return refused(.not_owner);
    const held = holding.content.parent();
    if (!held.parented or held.reaped) return .reap;
    if (held.ended == null) return refused(.running);
    darwin.reap(holding.content.leader().pid);
    held.reaped = true;
    return .reap;
}

/// The target's view of a quiesced holding; the reply carries a duplicate of the master, or of
/// each captured stream's read end.
pub fn inspect(holding: *Holding, arena: std.mem.Allocator, effects: *Effects) error{OutOfMemory}!Body {
    switch (holding.content) {
        .pty => |*held| {
            if (!effects.descriptors.sendDuplicate(held.master)) return refused(.handle_failed);
            return .{ .inspect_pty = .{ .facts = try held.facts.view(arena), .parented = held.parent.parented, .ended = held.parent.ended } };
        },
        .pipes => |*held| {
            for (held.streams) |stream| if (stream) |descriptor| if (!effects.descriptors.sendDuplicate(descriptor)) {
                effects.descriptors.closeSent();
                return refused(.handle_failed);
            };
            return .{ .inspect_pipes = .{
                .facts = try held.facts.view(arena),
                .capture = held.capture(),
                .parented = held.parent.parented,
                .ended = held.parent.ended,
            } };
        },
    }
}

/// A claim names the leader, and for a terminal the claimant's terminal; the new owner is sent
/// `exited` for a leader that already ended.
pub fn claim(holding: *Holding, request: message.Claim) ?Reason {
    if (!request.program.eql(holding.content.leader())) return .identity_mismatch;
    switch (holding.content) {
        .pty => |*held| held.facts.terminal = request.terminal orelse return .identity_mismatch,
        .pipes => if (request.terminal != null) return .identity_mismatch,
    }
    holding.content.parent().exit_sent = false;
    return null;
}

/// Before a holding leaves the table: its ended parented leader is reaped now, a running one once
/// it ends.
pub fn released(table: *Table, holding: *Holding) ?Reason {
    const held = holding.content.parent();
    if (!held.parented or held.reaped) return null;
    const leader = holding.content.leader().pid;
    if (held.ended != null) {
        darwin.reap(leader);
        held.reaped = true;
        return null;
    }
    if (!table.extra.reapLater(leader)) return .capacity;
    return null;
}

/// Runs a Host's `spawn` without the lock held; `descriptors` holds the master, the slave, and the
/// standard input when sent. The Keeper keeps only the master.
pub fn spawn(table: *Table, arena: std.mem.Allocator, peer: state.Peer, request: pty.Spawn, descriptors: *content.Descriptors) error{OutOfMemory}!Body {
    const requester = switch (peer) {
        .host => |process| process,
        .worker => return refused(.wrong_role),
    };
    if (descriptors.count != request.descriptorCount()) return refused(.handle_failed);
    var stored = content.StoredPtyFacts.store(.{
        .terminal = request.facts.terminal,
        .leader = .{ .pid = 0, .created = 0 },
        .slave = request.facts.slave,
        .launch = request.facts.launch,
    }) orelse return refused(.identity_mismatch);
    const master = descriptors.received[0];
    if (!darwin.masterOf(master, request.facts.slave)) return refused(.identity_mismatch);
    const streams: fork_launch.Streams = .{ .pty = .{
        .master = master,
        .slave = descriptors.received[1],
        .input = if (request.input) descriptors.received[2] else null,
    } };
    const plan = try launchPlan(arena, request.launch, streams);
    const started = switch (try startHolding(table, requester, request.holding, .spawn, plan)) {
        .started => |process| process,
        .refused => |body| return body,
    };
    stored.leader = started.process;
    requests.insert(table, started.position, .{
        .id = request.holding,
        .owner = requester,
        .content = .{ .pty = .{ .master = descriptors.take(0), .facts = stored, .parent = .{ .parented = true } } },
        .orphaned = started.orphaned,
    });
    table.unlock();
    return .{ .spawn = started.process };
}

/// Runs a Host's `spawn-program` without the lock held; `descriptors` holds each captured stream's
/// read end and then its write end. The Keeper keeps the read ends, and the program's standard
/// input is the null device.
pub fn spawnProgram(table: *Table, arena: std.mem.Allocator, peer: state.Peer, request: pty.SpawnProgram, descriptors: *content.Descriptors) error{OutOfMemory}!Body {
    const requester = switch (peer) {
        .host => |process| process,
        .worker => return refused(.wrong_role),
    };
    const count = request.capture.count();
    if (descriptors.count != 2 * count) return refused(.handle_failed);
    for (0..count) |stream| {
        if (!darwin.pipeEnd(descriptors.received[2 * stream], false)) return refused(.identity_mismatch);
        if (!darwin.pipeEnd(descriptors.received[2 * stream + 1], true)) return refused(.identity_mismatch);
    }
    const null_device = posix_compat.openNullDevice() catch return refused(.spawn_failed);
    defer posix_compat.close(null_device);
    var outputs: [2]posix.fd_t = .{ null_device, null_device };
    var at: usize = 0;
    for ([_]bool{ request.capture.stdout, request.capture.stderr }, &outputs) |captured, *output| if (captured) {
        output.* = descriptors.received[at + 1];
        at += 2;
    };
    const plan = try launchPlan(arena, request.launch, .{ .pipes = .{ .input = null_device, .output = outputs[0], .errors = outputs[1] } });
    const started = switch (try startHolding(table, requester, request.holding, .spawn_program, plan)) {
        .started => |process| process,
        .refused => |body| return body,
    };
    requests.insert(table, started.position, .{
        .id = request.holding,
        .owner = requester,
        .content = .{ .pipes = .{
            .streams = takeStreams(descriptors, request.capture, 2),
            .facts = .store(.{ .process = started.process, .program_id = request.facts.program_id, .label = request.facts.label }),
            .parent = .{ .parented = true },
        } },
        .orphaned = started.orphaned,
    });
    table.unlock();
    return .{ .spawn_program = started.process };
}

const Started = union(enum) {
    /// The leader runs and the lock is held; the caller inserts the holding at `position`.
    started: struct { process: Process, position: usize, orphaned: bool },
    /// The reply that ends the request, with the lock released.
    refused: Body,
};

/// Reserves the holding under the lock, starts the leader outside it, and takes the lock again.
fn startHolding(table: *Table, requester: Process, id: RandomId, kind: message.RequestKind, plan: fork_launch.Plan) error{OutOfMemory}!Started {
    table.lock();
    const early = reserve(table, requester, id, kind);
    table.unlock();
    if (early) |body| return .{ .refused = body };

    const leader = start(plan);

    table.lock();
    table.extra.unreserve(id);
    const refusal: ?Body = refusal: {
        const process = leader orelse break :refusal refused(.spawn_failed);
        const host = table.findHost(requester) orelse {
            // A reservation keeps its Host registered, so this cannot happen; reap the leader later.
            _ = table.extra.reapLater(process.pid);
            break :refusal refused(.not_registered);
        };
        if (table.lifecycle == .stopping) {
            _ = table.extra.reapLater(process.pid);
            break :refusal refused(.draining);
        }
        return .{ .started = .{ .process = process, .position = table.search(id).index, .orphaned = table.hosts[host].?.ended } };
    };
    table.unlock();
    return .{ .refused = refusal.? };
}

/// Reserves a spawn's holding, or returns the reply that ends the request: a repeat of a completed
/// spawn of the same kind answers its leader again.
fn reserve(table: *Table, requester: Process, id: RandomId, kind: message.RequestKind) ?Body {
    if (table.findHost(requester) == null) return refused(.not_registered);
    const position = table.search(id);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.content.parent().parented) switch (holding.content) {
            .pty => if (kind == .spawn) return .{ .spawn = holding.content.leader() },
            .pipes => if (kind == .spawn_program) return .{ .spawn_program = holding.content.leader() },
        };
        return refused(.holding_exists);
    }
    if (admits(table, id)) |reason| return refused(reason);
    if (!table.extra.reserve(.{ .holding = id, .owner = requester })) return refused(.capacity);
    return null;
}

/// Starts the leader and reads its identity, or returns null with no child left behind.
fn start(plan: fork_launch.Plan) ?Process {
    const child = fork_launch.start(plan) catch return null;
    const pid: u32 = @intCast(child);
    const created = darwin.processStart(pid) orelse {
        posix.kill(child, posix.SIG.KILL) catch {};
        _ = posix_compat.waitpid(child, 0);
        return null;
    };
    handles.watchLeader(pid);
    return .{ .pid = pid, .created = created };
}

fn launchPlan(arena: std.mem.Allocator, launch: pty.Launch, streams: fork_launch.Streams) error{OutOfMemory}!fork_launch.Plan {
    const argv = try arena.allocSentinel(?[*:0]const u8, launch.arguments.len + 1, null);
    argv[0] = (try arena.dupeZ(u8, launch.executable)).ptr;
    for (launch.arguments, 1..) |argument, index| argv[index] = (try arena.dupeZ(u8, argument)).ptr;
    const environment = try arena.allocSentinel(?[*:0]const u8, launch.environment.len, null);
    for (launch.environment, environment) |entry, *slot| slot.* = (try arena.dupeZ(u8, entry)).ptr;
    const candidates = try arena.alloc([*:0]const u8, launch.candidates.len);
    for (launch.candidates, candidates) |candidate, *slot| slot.* = (try arena.dupeZ(u8, candidate)).ptr;
    return .{
        .executable = launch.executable,
        .argv = argv,
        .environment = environment,
        .candidates = candidates,
        .directory = launch.directory,
        .streams = streams,
    };
}
