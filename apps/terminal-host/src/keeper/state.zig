//! The Keeper's table: its holdings, the Hosts the Worker registered, and the open connections of
//! each kind. One lock guards all of it and is held across no I/O and no wait, so requests apply one
//! at a time across every connection and a claim and a resume of one mark never both succeed
//! (TH-034). A Host handle the table forgets goes back to the caller, which releases it after the
//! lock is released, because unregistering its wait can wait for a callback that takes the lock.

const std = @import("std");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const thread_sync = platform.thread_sync;
const monotonic = platform.monotonic;
const handles = @import("handles_windows.zig");
const requests = @import("requests.zig");

const message = keeper_protocol.message;
const Process = message.Process;
const RandomId = message.RandomId;
const Reason = message.Reason;
const Body = message.Reply.Body;

pub const maximum_host_connections = 256;
/// Connections not yet admitted, or carrying a control request.
pub const maximum_pending_connections = 8;
pub const maximum_connections = maximum_host_connections + 1 + maximum_pending_connections;
/// Twice the registration bound, so a slot is free whenever fewer than 256 Hosts are registered,
/// even while forgotten Hosts' handles are still being released.
const host_slots = 2 * message.maximum_hosts;
const launch_capacity = handshake.message.maximum_name_bytes;

pub const Lifecycle = enum { ready, draining, stopping };

/// Who sent a request: the Worker, or the registered Host whose connection it arrived on.
pub const Peer = union(enum) {
    worker: Worker,
    host: Process,
};

pub const Worker = struct {
    /// The Worker's connection negotiated `programs`, so `status` lists program holdings to it.
    programs: bool = false,
};

/// What a request leaves for its caller to release after the lock: the duplicates of a released
/// holding, and the handle and wait of a forgotten Host.
pub const Effects = struct {
    released: ?Content = null,
    host: ?HostRelease = null,

    pub fn release(self: *const Effects) void {
        if (self.released) |*content| content.close();
        if (self.host) |host| host.release(true);
    }
};

pub const HostRelease = struct {
    handle: handles.Handle,
    wait: ?handles.Handle,

    /// `blocking` waits for an end callback that already started; the callback itself passes
    /// false, since it cannot wait for itself.
    pub fn release(self: HostRelease, blocking: bool) void {
        if (self.wait) |wait| if (blocking) handles.unwatch(wait) else handles.unwatchFromCallback(wait);
        handles.close(self.handle);
    }
};

pub const Admission = union(enum) {
    admitted: Peer,
    refused: Reason,
};

pub const ConnectionKind = enum { pending, worker, host };

const ConnectionSlot = struct {
    in_use: bool = false,
    kind: ConnectionKind = .pending,
    pid: u32 = 0,
    /// Set when the connection must end: its Host was unregistered, or the Keeper is stopping.
    closing: bool = false,
    wake: ?handles.Handle = null,
};

pub const Mark = struct { id: RandomId, target: Process };
pub const Claimed = struct { from: Process, mark: RandomId };

pub const StoredFacts = struct {
    terminal: message.Terminal,
    program: Process,
    console: Process,
    console_build: [32]u8,
    signal_format: u16,
    launch: [launch_capacity]u8,
    launch_length: u8,

    pub fn store(facts: message.Facts) ?StoredFacts {
        if (facts.launch.len > launch_capacity) return null;
        var stored: StoredFacts = .{
            .terminal = facts.terminal,
            .program = facts.program,
            .console = facts.console,
            .console_build = facts.console_build,
            .signal_format = facts.signal_format,
            .launch = undefined,
            .launch_length = @intCast(facts.launch.len),
        };
        @memcpy(stored.launch[0..facts.launch.len], facts.launch);
        return stored;
    }

    pub fn view(self: *const StoredFacts, arena: std.mem.Allocator) error{OutOfMemory}!message.Facts {
        return .{
            .terminal = self.terminal,
            .program = self.program,
            .console = self.console,
            .console_build = self.console_build,
            .signal_format = self.signal_format,
            .launch = try arena.dupe(u8, self.launch[0..self.launch_length]),
        };
    }
};

pub const StoredProgramFacts = struct {
    process: Process,
    program_id: [16]u8,
    label: [message.maximum_label_bytes]u8,
    label_length: u16,

    /// Facts the decoder checked, so the label fits.
    pub fn store(facts: message.ProgramFacts) StoredProgramFacts {
        var stored: StoredProgramFacts = .{
            .process = facts.process,
            .program_id = facts.program_id,
            .label = undefined,
            .label_length = @intCast(facts.label.len),
        };
        @memcpy(stored.label[0..facts.label.len], facts.label);
        return stored;
    }

    pub fn view(self: *const StoredProgramFacts, arena: std.mem.Allocator) error{OutOfMemory}!message.ProgramFacts {
        return .{
            .process = self.process,
            .program_id = self.program_id,
            .label = try arena.dupe(u8, self.label[0..self.label_length]),
        };
    }
};

/// What a holding holds, with the Keeper's duplicates of its handles: a terminal, or a
/// background program a Host runs without one.
pub const Content = union(enum) {
    terminal: struct { duplicates: handles.Duplicates, facts: StoredFacts },
    program: struct { duplicates: handles.ProgramDuplicates, facts: StoredProgramFacts },

    pub fn close(self: *const Content) void {
        switch (self.*) {
            .terminal => |*terminal| handles.closeAll(&terminal.duplicates),
            .program => |*program| program.duplicates.close(),
        }
    }

    pub fn view(self: *const Content, arena: std.mem.Allocator) error{OutOfMemory}!message.HoldingFacts {
        return switch (self.*) {
            .terminal => |*terminal| .{ .terminal = try terminal.facts.view(arena) },
            .program => |*program| .{ .program = try program.facts.view(arena) },
        };
    }
};

pub const Holding = struct {
    id: RandomId,
    owner: Process,
    content: Content,
    mark: ?Mark = null,
    /// The Host this holding was claimed from and the mark that claim consumed.
    claimed: ?Claimed = null,
    /// The owner has ended.
    orphaned: bool = false,

    pub fn state(self: *const Holding) message.HoldingState {
        if (self.orphaned) return .orphaned;
        return if (self.mark != null) .quiesced else .held;
    }
};

pub const Host = struct {
    process: Process,
    handle: handles.Handle,
    wait: ?handles.Handle,
    ended: bool = false,
};

pub const Table = struct {
    mutex: thread_sync.Mutex = .{},
    lifecycle: Lifecycle = .ready,
    /// Ascending by holding id.
    holdings: [message.maximum_holdings]Holding = undefined,
    holding_count: usize = 0,
    hosts: [host_slots]?Host = @splat(null),
    /// Bumped whenever a slot is forgotten, so a late end callback for its earlier Host does nothing.
    host_generations: [host_slots]u16 = @splat(0),
    registered_hosts: usize = 0,
    connections: [maximum_connections]ConnectionSlot = @splat(.{}),
    pending_connections: usize = 0,
    host_connections: usize = 0,
    worker_connected: bool = false,
    /// The clock the idle span is counted on, read under the lock.
    clock: monotonic.Timer,
    /// When the Keeper last became idle: stamped at the change that made it idle, and null while it
    /// is not, so the span starts there even when no listener pass observed that change.
    idle_since_ns: ?u64 = null,

    /// Creates each connection slot's wake event, and starts the idle span: the Keeper starts idle.
    pub fn init(self: *Table) error{SystemResources}!void {
        self.* = .{ .clock = monotonic.Timer.start() catch return error.SystemResources };
        errdefer self.deinit();
        for (&self.connections) |*slot| {
            slot.wake = handles.createWakeEvent() orelse return error.SystemResources;
        }
        self.settleIdle();
    }

    pub fn deinit(self: *Table) void {
        for (&self.connections) |*slot| if (slot.wake) |wake| handles.close(wake);
    }

    pub fn lock(self: *Table) void {
        self.mutex.lock();
    }

    pub fn unlock(self: *Table) void {
        self.mutex.unlock();
    }

    // Connections, called with the lock held.

    /// Takes a slot for a connection just accepted from `pid`, or null when it is beyond the bounds.
    pub fn reservePending(self: *Table, pid: u32) ?usize {
        if (self.lifecycle == .stopping or self.pending_connections >= maximum_pending_connections) return null;
        for (&self.connections, 0..) |*slot, index| {
            if (slot.in_use) continue;
            slot.* = .{ .in_use = true, .kind = .pending, .pid = pid, .wake = slot.wake };
            self.pending_connections += 1;
            return index;
        }
        return null;
    }

    pub fn wakeEvent(self: *Table, slot: usize) handles.Handle {
        return self.connections[slot].wake.?;
    }

    pub fn closing(self: *Table, slot: usize) bool {
        return self.connections[slot].closing or self.lifecycle == .stopping;
    }

    /// Admits `hello` on a pending connection, or returns why it is refused.
    pub fn admit(self: *Table, slot_index: usize, role: message.Role) Admission {
        const slot = &self.connections[slot_index];
        std.debug.assert(slot.kind == .pending);
        switch (role) {
            .worker => {
                if (self.findHostByPid(slot.pid) != null) return .{ .refused = .wrong_role };
                if (self.worker_connected) return .{ .refused = .capacity };
                self.worker_connected = true;
                self.settleIdle();
                slot.kind = .worker;
                self.pending_connections -= 1;
                return .{ .admitted = .{ .worker = .{} } };
            },
            .host => {
                const index = self.findHostByPid(slot.pid) orelse return .{ .refused = .not_registered };
                const host = self.hosts[index].?;
                if (host.ended) return .{ .refused = .not_registered };
                if (self.host_connections >= maximum_host_connections) return .{ .refused = .capacity };
                self.host_connections += 1;
                slot.kind = .host;
                self.pending_connections -= 1;
                return .{ .admitted = .{ .host = host.process } };
            },
            .unknown => return .{ .refused = .wrong_role },
        }
    }

    pub fn releaseConnection(self: *Table, slot_index: usize) void {
        const slot = &self.connections[slot_index];
        switch (slot.kind) {
            .pending => self.pending_connections -= 1,
            .worker => self.worker_connected = false,
            .host => self.host_connections -= 1,
        }
        slot.* = .{ .wake = slot.wake };
        self.settleIdle();
    }

    // Lifecycle, called with the lock held.

    /// Whether the Keeper holds nothing, has no Host registered, and has no Worker connected.
    pub fn idle(self: *const Table) bool {
        return self.holding_count == 0 and self.registered_hosts == 0 and !self.worker_connected;
    }

    /// Whether the Keeper has been idle for at least `span_ns`, from the change that made it idle.
    pub fn idleFor(self: *const Table, span_ns: u64) bool {
        const since = self.idle_since_ns orelse return false;
        return self.clock.read() -| since >= span_ns;
    }

    /// Stamps the change that made the Keeper idle, or clears the stamp while it is not. Each change
    /// to what `idle` reads calls this under the lock, so the stamp never lags behind a change.
    fn settleIdle(self: *Table) void {
        if (!self.idle()) {
            self.idle_since_ns = null;
        } else if (self.idle_since_ns == null) {
            self.idle_since_ns = self.clock.read();
        }
    }

    pub fn holdingCount(self: *const Table) u32 {
        return @intCast(self.holding_count);
    }

    pub fn drain(self: *Table) void {
        if (self.lifecycle == .ready) self.lifecycle = .draining;
    }

    /// Stops admitting anything, closes every duplicate, and wakes every connection to end it.
    /// Closing the last handle of a kill-on-close Job ends its programs, as at a confirmed stop.
    pub fn stop(self: *Table) void {
        self.lifecycle = .stopping;
        for (self.holdings[0..self.holding_count]) |*holding| holding.content.close();
        self.holding_count = 0;
        for (&self.connections) |*slot| if (slot.in_use) {
            slot.closing = true;
            handles.signal(slot.wake.?);
        };
        self.settleIdle();
    }

    // Requests, called with the lock held; `requests.zig` applies each.

    /// Applies one request from `peer` and returns its reply body; slices live in `arena`.
    pub fn apply(
        self: *Table,
        arena: std.mem.Allocator,
        peer: Peer,
        request: message.Request.Body,
        effects: *Effects,
    ) error{OutOfMemory}!Body {
        defer self.settleIdle();
        return requests.apply(self, arena, peer, request, effects);
    }

    // Hosts.

    /// A registered Host's end: its holdings are orphaned, what holdings remember of claims from it
    /// is cleared, and it stays registered, as ended, while a holding names it as owner.
    fn hostEndedLocked(self: *Table, index: usize, generation: u16) ?HostRelease {
        if (self.host_generations[index] != generation) return null;
        const host = if (self.hosts[index]) |*host| host else return null;
        if (host.ended) return null;
        host.ended = true;
        for (self.holdings[0..self.holding_count]) |*holding| {
            if (holding.owner.eql(host.process)) holding.orphaned = true;
            if (holding.claimed) |claimed| if (claimed.from.eql(host.process)) {
                holding.claimed = null;
            };
        }
        return self.forgetIfEndedAndUnnamed(host.process);
    }

    /// Forgets `process` once it has ended and no holding names it as owner.
    pub fn forgetIfEndedAndUnnamed(self: *Table, process: Process) ?HostRelease {
        const index = self.findHost(process) orelse return null;
        if (!self.hosts[index].?.ended) return null;
        for (self.holdings[0..self.holding_count]) |*holding| {
            if (holding.owner.eql(process)) return null;
        }
        return self.forget(index);
    }

    pub fn forget(self: *Table, index: usize) HostRelease {
        const host = self.hosts[index].?;
        self.hosts[index] = null;
        self.host_generations[index] +%= 1;
        self.registered_hosts -= 1;
        self.settleIdle();
        return .{ .handle = host.handle, .wait = host.wait };
    }

    pub fn findHost(self: *const Table, process: Process) ?usize {
        for (self.hosts, 0..) |slot, index| if (slot) |host| {
            if (host.process.eql(process)) return index;
        };
        return null;
    }

    fn findHostByPid(self: *const Table, pid: u32) ?usize {
        for (self.hosts, 0..) |slot, index| if (slot) |host| {
            if (host.process.pid == pid) return index;
        };
        return null;
    }

    // Holdings.

    pub const Position = struct { index: usize, found: bool };

    /// Where `id` is, or where it would go, in the ascending holdings.
    pub fn search(self: *const Table, id: RandomId) Position {
        var low: usize = 0;
        var high: usize = self.holding_count;
        while (low < high) {
            const middle = low + (high - low) / 2;
            switch (std.mem.order(u8, &self.holdings[middle].id, &id)) {
                .lt => low = middle + 1,
                .gt => high = middle,
                .eq => return .{ .index = middle, .found = true },
            }
        }
        return .{ .index = low, .found = false };
    }

    pub fn find(self: *Table, id: RandomId) ?*Holding {
        const position = self.search(id);
        return if (position.found) &self.holdings[position.index] else null;
    }
};

/// The one table of this process, which Host-end callbacks reach.
var active: ?*Table = null;

pub fn activate(table: *Table) void {
    active = table;
}

/// Runs on a thread-pool thread once a registered Host ends.
pub fn hostEnded(context: ?*anyopaque, _: u8) callconv(.winapi) void {
    const table = active orelse return;
    const value = @intFromPtr(context);
    table.lock();
    const forgotten = table.hostEndedLocked(value & 0xffff, @truncate(value >> 16));
    table.unlock();
    if (forgotten) |host| host.release(false);
}
