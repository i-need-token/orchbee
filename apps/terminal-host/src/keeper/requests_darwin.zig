//! The macOS Keeper's requests that depend on what a holding holds: a Host's `hold-pty` of a PTY
//! master it forked a leader on, its `spawn` of a leader the Keeper parents, the owner's `reap` of
//! an ended parented leader, and the inspection, claim, and release of a PTY holding. A `spawn`
//! applies in three steps: it reserves its holding under the lock, forks outside it, and inserts
//! the holding under the lock again, so no fork waits with the lock held.

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

/// Whether a registered Host may send `request` beyond the requests every Keeper serves. `spawn`
/// never reaches the table's apply: its connection runs it in steps.
pub fn hostRequest(request: message.Request.Body) bool {
    return request == .hold_pty or request == .reap;
}

pub fn apply(table: *Table, _: std.mem.Allocator, requester: Process, request: message.Request.Body, effects: *Effects) error{OutOfMemory}!Body {
    return switch (request) {
        .hold_pty => |hold| holdPty(table, requester, hold, effects),
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
        if (holding.owner.eql(requester) and holding.state() == .held and !holding.content.pty.parented) return .hold_pty;
        return refused(.holding_exists);
    }
    if (table.extra.reserved(hold.holding)) return refused(.holding_exists);
    if (table.lifecycle != .ready) return refused(.draining);
    if (table.holding_count + table.extra.spawning_count >= message.maximum_holdings) return refused(.capacity);
    const stored = content.StoredPtyFacts.store(hold.facts) orelse return refused(.identity_mismatch);
    if (effects.descriptors.count != 1) return refused(.handle_failed);
    const master = effects.descriptors.received[0];
    if (!darwin.masterOf(master, hold.facts.slave)) return refused(.identity_mismatch);
    if (darwin.processStart(hold.facts.leader.pid) != hold.facts.leader.created) return refused(.identity_mismatch);
    requests.insert(table, position.index, .{
        .id = hold.holding,
        .owner = requester,
        .content = .{ .pty = .{ .master = effects.descriptors.take(0), .facts = stored, .parented = false } },
    });
    return .hold_pty;
}

/// The owner's reap of its ended parented leader, once it has contained the leader's session.
fn reap(table: *Table, requester: Process, id: RandomId) Body {
    const holding = table.find(id) orelse return refused(.unknown_holding);
    if (!holding.owner.eql(requester)) return refused(.not_owner);
    const held = &holding.content.pty;
    if (!held.parented or held.reaped) return .reap;
    if (held.ended == null) return refused(.running);
    darwin.reap(held.facts.leader.pid);
    held.reaped = true;
    return .reap;
}

/// The target's view of a quiesced PTY holding; the reply carries a duplicate of the master.
pub fn inspect(holding: *Holding, arena: std.mem.Allocator, effects: *Effects) error{OutOfMemory}!Body {
    const held = &holding.content.pty;
    effects.descriptors.send = darwin.duplicate(held.master) orelse return refused(.handle_failed);
    return .{ .inspect_pty = .{ .facts = try held.facts.view(arena), .parented = held.parented, .ended = held.ended } };
}

/// A PTY holding's claim names the leader and the claimant's terminal; the new owner is sent
/// `exited` for a leader that already ended.
pub fn claim(holding: *Holding, request: message.Claim) ?Reason {
    const held = &holding.content.pty;
    const identity = request.terminal orelse return .identity_mismatch;
    if (!request.program.eql(held.facts.leader)) return .identity_mismatch;
    held.facts.terminal = identity;
    held.exit_sent = false;
    return null;
}

/// Before a holding leaves the table: its ended parented leader is reaped now, a running one once
/// it ends.
pub fn released(table: *Table, holding: *Holding) ?Reason {
    const held = &holding.content.pty;
    if (!held.parented or held.reaped) return null;
    const leader = held.facts.leader.pid;
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
    const plan = try launchPlan(arena, request.launch, master, descriptors.received[1], if (request.input) descriptors.received[2] else null);

    table.lock();
    const early = reserve(table, requester, request.holding);
    table.unlock();
    if (early) |body| return body;

    const leader = start(plan);

    table.lock();
    defer table.unlock();
    table.extra.unreserve(request.holding);
    const started = leader orelse return refused(.spawn_failed);
    const host = table.findHost(requester) orelse {
        // A reservation keeps its Host registered, so this cannot happen; reap the leader later.
        _ = table.extra.reapLater(started.pid);
        return refused(.not_registered);
    };
    if (table.lifecycle == .stopping) return refused(.draining);
    stored.leader = started;
    const position = table.search(request.holding);
    requests.insert(table, position.index, .{
        .id = request.holding,
        .owner = requester,
        .content = .{ .pty = .{ .master = descriptors.take(0), .facts = stored, .parented = true } },
        .orphaned = table.hosts[host].?.ended,
    });
    return .{ .spawn = started };
}

/// Reserves a spawn's holding, or returns the reply that ends the request: a repeat of a completed
/// spawn answers its leader again.
fn reserve(table: *Table, requester: Process, id: RandomId) ?Body {
    if (table.findHost(requester) == null) return refused(.not_registered);
    const position = table.search(id);
    if (position.found) {
        const holding = &table.holdings[position.index];
        if (holding.owner.eql(requester) and holding.content.pty.parented) return .{ .spawn = holding.content.pty.facts.leader };
        return refused(.holding_exists);
    }
    if (table.extra.reserved(id)) return refused(.holding_exists);
    if (table.lifecycle != .ready) return refused(.draining);
    if (table.holding_count + table.extra.spawning_count >= message.maximum_holdings) return refused(.capacity);
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

fn launchPlan(
    arena: std.mem.Allocator,
    launch: pty.Launch,
    master: posix.fd_t,
    slave: posix.fd_t,
    input: ?posix.fd_t,
) error{OutOfMemory}!fork_launch.Plan {
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
        .master = master,
        .slave = slave,
        .input = input,
    };
}
