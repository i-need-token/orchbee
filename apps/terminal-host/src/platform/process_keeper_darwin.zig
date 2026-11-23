//! What a macOS terminal's Keeper links read and change: its release-first gate, the facts a
//! `hold-pty` names, and, when a Keeper started its leader, that parent's state. The record lives
//! apart from the process value, so a pointer to it stays valid from the spawn on, however the
//! process value moves.
//!
//! A parented leader's exit reaches the Host as the Keeper's `exited`, and its reap is the Keeper's
//! `reap`; both run on the link that parents it, which the terminal's threads wake through the
//! gate. Once that link released the holding or ended, the Keeper or `launchd` reaps the leader
//! and the Host sees its end only by its start time.

const std = @import("std");
const keeper_darwin = @import("keeper_darwin.zig");
const monotonic = @import("monotonic.zig");
const thread_sync = @import("thread_sync.zig");
const process_types = @import("process_types.zig");
const Gate = @import("keeper_gate.zig").Gate;

const ExitResult = process_types.ExitResult;

/// How often a leader no link parents any more is checked for its end.
const end_poll_ns = 50 * std.time.ns_per_ms;

/// A Keeper that starts a terminal's leader for the Host and parents it.
pub const Spawn = struct {
    context: *anyopaque,
    spawn: *const fn (context: *anyopaque, plan: *const Plan) Outcome,
};

/// The leader as the Host prepared it, and the descriptors the Keeper receives.
pub const Plan = struct {
    record: *Record,
    master: std.posix.fd_t,
    slave: std.posix.fd_t,
    input: ?std.posix.fd_t,
    executable: []const u8,
    argv: [*:null]const ?[*:0]const u8,
    environment: [*:null]const ?[*:0]const u8,
    candidates: []const [*:0]const u8,
    directory: ?[]const u8,
    launch_kind: process_types.LaunchKind,
};

pub const Outcome = union(enum) {
    /// The Keeper started and parents this leader; the record is parented and held.
    started: std.posix.pid_t,
    /// The Keeper started nothing, so the Host forks the leader itself.
    refused,
    /// The child could not run the executable.
    spawn_failed,
    /// The Keeper may have started the leader; the terminal fails.
    unanswered,
};

pub const Parent = enum {
    /// The Host forked the leader and reaps it.
    none,
    /// A Keeper parents the leader and keeps it until the Host's `reap`.
    parented,
    /// The parent's holding was released; the Keeper reaps the leader once it ends.
    released,
    /// The parent's link ended or failed; another process reaps the leader.
    lost,
};

pub const Record = struct {
    gate: Gate = .{},
    /// The leader's start time in microseconds since 1970; zero when it could not be read, and
    /// then no Keeper holds the terminal.
    leader_created: u64 = 0,
    slave: [keeper_darwin.maximum_slave_bytes]u8 = undefined,
    slave_length: usize = 0,

    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    // Guarded by `mutex`.
    parent: Parent = .none,
    /// The attachment whose Keeper parents the leader; only its link reaps it or ends the parent.
    parent_serial: u64 = 0,
    exit: ?ExitResult = null,
    reap: enum { idle, requested, done } = .idle,

    /// A record for the PTY whose master is `master`; its slave name is read now.
    pub fn create(allocator: std.mem.Allocator, master: std.posix.fd_t) !*Record {
        const record = try allocator.create(Record);
        record.* = .{};
        if (keeper_darwin.slaveName(master, &record.slave)) |name| record.slave_length = name.len;
        return record;
    }

    /// Frees the record once no link touches its gate.
    pub fn destroy(self: *Record, allocator: std.mem.Allocator) void {
        std.debug.assert(self.gate.idle());
        allocator.destroy(self);
    }

    /// Records the start time of a leader the Host forked, read once it runs.
    pub fn started(self: *Record, leader: std.posix.pid_t) void {
        self.leader_created = keeper_darwin.processStart(@intCast(leader)) orelse 0;
    }

    /// The slave's name, or null when it could not be read.
    pub fn slaveName(self: *const Record) ?[]const u8 {
        if (self.slave_length == 0) return null;
        return self.slave[0..self.slave_length];
    }

    /// The link thread of attachment `serial`: its Keeper started this leader, which it parents.
    pub fn parentedBy(self: *Record, serial: u64, created: u64) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.leader_created = created;
        self.parent = .parented;
        self.parent_serial = serial;
    }

    pub fn isParented(self: *Record) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.parent != .none;
    }

    /// The link thread: the parent sent `exited`.
    pub fn exited(self: *Record, status: ExitResult) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.parent != .parented or self.exit != null) return;
        self.exit = status;
        self.condition.broadcast();
    }

    /// The link thread of attachment `serial`: whether the terminal waits for its `reap`.
    pub fn reapRequestedOf(self: *Record, serial: u64) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.parent == .parented and self.parent_serial == serial and self.reap == .requested;
    }

    /// The parent's link: the `reap` was answered, or the Keeper will reap nothing for this Host.
    pub fn reapedBy(self: *Record, serial: u64) void {
        self.settle(serial, .parented);
    }

    /// The parent's link: the holding was released, so the Keeper reaps the leader once it ends.
    pub fn releasedBy(self: *Record, serial: u64) void {
        self.settle(serial, .released);
    }

    /// The parent's link ended or failed.
    pub fn lostBy(self: *Record, serial: u64) void {
        self.settle(serial, .lost);
    }

    fn settle(self: *Record, serial: u64, parent: Parent) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.parent != .parented or self.parent_serial != serial) return;
        self.parent = parent;
        if (self.reap == .requested) self.reap = .done;
        self.condition.broadcast();
    }

    /// The wait thread: the parented leader's exit, once the parent sent it. A leader no link
    /// parents any more is watched by its start time and ends with an unknown status.
    pub fn awaitExit(self: *Record, leader: std.posix.pid_t) ExitResult {
        {
            self.mutex.lock();
            defer self.mutex.unlock();
            while (self.exit == null and self.parent == .parented) self.condition.wait(&self.mutex);
            if (self.exit) |status| return status;
        }
        while (keeper_darwin.processStart(@intCast(leader)) == self.leader_created) monotonic.sleep(end_poll_ns);
        return .{ .unknown = 0 };
    }

    /// The identity release: asks the parent to reap the leader and waits until it did, or until
    /// no link parents it. Nothing is left to reap for a leader whose parent is gone.
    pub fn awaitReap(self: *Record) void {
        self.mutex.lock();
        if (self.parent != .parented or self.reap == .done) return self.mutex.unlock();
        self.reap = .requested;
        self.mutex.unlock();
        self.gate.wakeActive();
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.parent == .parented and self.reap == .requested) self.condition.wait(&self.mutex);
    }
};
