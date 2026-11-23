//! What a macOS holding holds: a terminal's PTY master with the facts its hold or spawn recorded,
//! and, for a leader the Keeper parented, how it ended. The table also keeps the leaders whose
//! holdings were released while they ran, which the Keeper reaps once they end, and the holdings a
//! `spawn` reserved while its leader starts outside the lock.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const state = @import("state.zig");

const darwin = platform.keeper.darwin;
const posix_compat = platform.posix_compat;
const message = keeper_protocol.message;
const pty = message.pty;
const Process = message.Process;
const RandomId = message.RandomId;
const posix = std.posix;
const launch_capacity = handshake.message.maximum_name_bytes;

pub const StoredPtyFacts = struct {
    terminal: message.Terminal,
    leader: Process,
    slave: [pty.maximum_slave_bytes]u8,
    slave_length: u8,
    launch: [launch_capacity]u8,
    launch_length: u8,

    pub fn store(facts: pty.PtyFacts) ?StoredPtyFacts {
        if (facts.slave.len > pty.maximum_slave_bytes or facts.launch.len > launch_capacity) return null;
        var stored: StoredPtyFacts = .{
            .terminal = facts.terminal,
            .leader = facts.leader,
            .slave = undefined,
            .slave_length = @intCast(facts.slave.len),
            .launch = undefined,
            .launch_length = @intCast(facts.launch.len),
        };
        @memcpy(stored.slave[0..facts.slave.len], facts.slave);
        @memcpy(stored.launch[0..facts.launch.len], facts.launch);
        return stored;
    }

    pub fn view(self: *const StoredPtyFacts, arena: std.mem.Allocator) error{OutOfMemory}!pty.PtyFacts {
        return .{
            .terminal = self.terminal,
            .leader = self.leader,
            .slave = try arena.dupe(u8, self.slave[0..self.slave_length]),
            .launch = try arena.dupe(u8, self.launch[0..self.launch_length]),
        };
    }
};

pub const PtyContent = struct {
    master: posix.fd_t,
    facts: StoredPtyFacts,
    /// The Keeper started the leader, so it is the leader's parent and alone may reap it.
    parented: bool,
    /// How a parented leader ended; the Keeper keeps it unreaped until its owner's `reap`.
    ended: ?pty.ExitStatus = null,
    reaped: bool = false,
    /// The owner was sent `exited` for this end.
    exit_sent: bool = false,
};

pub const Content = union(enum) {
    pty: PtyContent,

    pub fn close(self: *const Content) void {
        posix_compat.close(self.pty.master);
    }

    pub fn view(self: *const Content, arena: std.mem.Allocator) error{OutOfMemory}!message.HoldingFacts {
        const held = &self.pty;
        return .{ .pty = .{ .facts = try held.facts.view(arena), .parented = held.parented, .ended = held.ended } };
    }

    /// A Worker's `status` lists PTY holdings only to a Worker that negotiated `pty`.
    pub fn listedTo(_: *const Content, worker: anytype) bool {
        return worker.pty;
    }
};

pub const Reservation = struct { holding: RandomId, owner: Process };

pub const TableExtra = struct {
    /// Leaders whose holdings were released while they ran.
    released: [2 * message.maximum_holdings]u32 = undefined,
    released_count: usize = 0,
    /// Holdings a `spawn` reserved; each counts toward the holding bound until it is inserted.
    spawning: [state.maximum_host_connections]Reservation = undefined,
    spawning_count: usize = 0,

    /// Whether a `spawn` of `process` is in flight, so the Host stays registered for its holding.
    pub fn reserves(self: *const TableExtra, process: Process) bool {
        for (self.spawning[0..self.spawning_count]) |reservation| {
            if (reservation.owner.eql(process)) return true;
        }
        return false;
    }

    pub fn reserved(self: *const TableExtra, holding: RandomId) bool {
        for (self.spawning[0..self.spawning_count]) |reservation| {
            if (std.mem.eql(u8, &reservation.holding, &holding)) return true;
        }
        return false;
    }

    pub fn reserve(self: *TableExtra, reservation: Reservation) bool {
        if (self.spawning_count == self.spawning.len) return false;
        self.spawning[self.spawning_count] = reservation;
        self.spawning_count += 1;
        return true;
    }

    pub fn unreserve(self: *TableExtra, holding: RandomId) void {
        for (self.spawning[0..self.spawning_count], 0..) |reservation, index| {
            if (!std.mem.eql(u8, &reservation.holding, &holding)) continue;
            self.spawning[index] = self.spawning[self.spawning_count - 1];
            self.spawning_count -= 1;
            return;
        }
    }

    /// Reaps `leader` once it ends; false when the list is full.
    pub fn reapLater(self: *TableExtra, leader: u32) bool {
        if (self.released_count == self.released.len) return false;
        self.released[self.released_count] = leader;
        self.released_count += 1;
        return true;
    }
};

/// The descriptors a request carried, in the order its message names them. A request keeps one by
/// taking it; `release` closes the rest after the lock.
pub const Descriptors = struct {
    received: [3]posix.fd_t = undefined,
    count: usize = 0,
    /// A duplicate the reply carries, which the connection closes once it wrote the reply.
    send: ?posix.fd_t = null,

    pub fn take(self: *Descriptors, index: usize) posix.fd_t {
        const descriptor = self.received[index];
        self.received[index] = -1;
        return descriptor;
    }

    pub fn release(self: *Descriptors) void {
        for (self.received[0..self.count]) |descriptor| if (descriptor >= 0) posix_compat.close(descriptor);
        self.count = 0;
    }
};

/// Records each parented leader that ended since the last poll, and wakes its owner's connections
/// to send `exited`; reaps each released leader that ended. Called with the lock held.
pub fn pollLeaders(table: *state.Table) void {
    for (table.holdings[0..table.holding_count]) |*holding| {
        const held = &holding.content.pty;
        if (!held.parented or held.ended != null) continue;
        const exit = darwin.peekExit(held.facts.leader.pid) orelse continue;
        held.ended = switch (exit) {
            .code => |code| .{ .code = code },
            .signal => |number| .{ .signal = number },
        };
        held.exit_sent = false;
        table.wakeHostConnections(holding.owner.pid);
    }
    const extra = &table.extra;
    var index: usize = 0;
    while (index < extra.released_count) {
        const leader = extra.released[index];
        if (darwin.peekExit(leader) == null) {
            index += 1;
            continue;
        }
        darwin.reap(leader);
        extra.released[index] = extra.released[extra.released_count - 1];
        extra.released_count -= 1;
    }
}

/// Takes up to `into.len` ends of `owner`'s parented leaders that it was not yet sent, marking
/// each sent. Called with the lock held.
pub fn takeExits(table: *state.Table, owner: Process, into: []pty.Exited) usize {
    var count: usize = 0;
    for (table.holdings[0..table.holding_count]) |*holding| {
        if (count == into.len) break;
        const held = &holding.content.pty;
        const status = held.ended orelse continue;
        if (held.exit_sent or !holding.owner.eql(owner)) continue;
        held.exit_sent = true;
        into[count] = .{ .holding = holding.id, .status = status };
        count += 1;
    }
    return count;
}
