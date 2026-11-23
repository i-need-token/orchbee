//! A macOS Keeper's update in place (`schemas/keeper/README.md`, Re-execution). On the Worker's
//! `reexec` it checks the new program against a copy of its state, stops the listener and every
//! admitted connection between frames, and, holding the table's lock from its last snapshot on,
//! writes the state to an unlinked file and executes the new program in its own process, which
//! keeps its children, its registration, and each descriptor the state names. A refusal at any step
//! leaves this image serving as before; only the new image answers the `reexec` that succeeded.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const state = @import("state.zig");
const saved = @import("saved_darwin.zig");
const check = @import("check_darwin.zig");

const message = keeper_protocol.message;
const Body = message.Reply.Body;
const handles = state.handles;
const posix = std.posix;
const c = std.c;
const posix_compat = platform.posix_compat;
const monotonic = platform.monotonic;
const thread_sync = platform.thread_sync;

/// How long the connections have to stop; a frame under way is due within 2 seconds.
const stop_deadline_ns = 3 * std.time.ns_per_s;
const stop_slice_ns = std.time.ns_per_ms;
const program_path_bytes = 4096;

extern "c" fn proc_pidpath(pid: c_int, buffer: [*]u8, size: u32) c_int;

/// The capabilities a connection's `answer` listed.
pub const Capabilities = struct {
    recovery: bool = false,
    programs: bool = false,
    pty: bool = false,
    parent: bool = false,
    reexec: bool = false,
};

/// An admitted connection stopped for an update: what the next image needs to serve it on.
pub const Parked = struct {
    socket: posix.fd_t,
    peer: state.Peer,
    capabilities: Capabilities,
    used: []const [2]u32,
    /// The `reexec` this connection waits on.
    pending: ?u32 = null,
};

pub const Stage = enum { none, checking, stopping };

/// The table's part in an update; guarded by the table's lock.
pub const Freeze = struct {
    stage: Stage = .none,
    listener_parked: bool = false,
    parked: [state.maximum_connections]?Parked = @splat(null),
    /// Wakes each stopped thread once the update ends in this image.
    thaw: thread_sync.Condition = .{},
};

/// What this image serves from, which the next image is executed with.
pub const Endpoint = struct {
    runtime_directory: []const u8,
    endpoint_id: []const u8,
    endpoint_directory: ?[]const u8,
    listener: posix.fd_t,
    lock: posix.fd_t,
    /// This image's program: the next image's way back.
    program: []const u8,
};

var endpoint: ?Endpoint = null;

/// Lets this image update; without it a `reexec` gets `exec-failed`.
pub fn arm(value: Endpoint) void {
    endpoint = value;
}

/// The program this process runs, as an absolute path, or null when it cannot be read.
pub fn ownProgram(allocator: std.mem.Allocator) ?[]u8 {
    var buffer: [program_path_bytes]u8 = undefined;
    const length = proc_pidpath(c.getpid(), &buffer, buffer.len);
    if (length <= 0) return null;
    const path = buffer[0..@intCast(length)];
    if (!message.validProgram(path)) return null;
    return allocator.dupe(u8, path) catch null;
}

/// Stops the connection in `slot` while an update stops connections; returns once it ends here.
pub fn parkConnection(table: *state.Table, slot: usize, parked: Parked) void {
    table.lock();
    defer table.unlock();
    const freeze = &table.extra.freeze;
    if (freeze.stage != .stopping or table.closing(slot)) return;
    freeze.parked[slot] = parked;
    while (freeze.stage == .stopping) freeze.thaw.wait(&table.mutex);
}

/// Stops the listener while an update stops connections; the kernel keeps new connections queued.
pub fn parkListener(table: *state.Table) void {
    table.lock();
    defer table.unlock();
    const freeze = &table.extra.freeze;
    while (freeze.stage == .stopping) {
        freeze.listener_parked = true;
        freeze.thaw.wait(&table.mutex);
    }
    freeze.listener_parked = false;
}

/// Updates the Keeper in place from `program` for the Worker connection in `own_slot`, which `own`
/// describes with its pending `reexec`. Returns only with the refusal that kept this image.
pub fn run(table: *state.Table, arena: std.mem.Allocator, program: []const u8, own_slot: usize, own: Parked) error{OutOfMemory}!Body {
    if (own.peer != .worker) return refused(.wrong_role);
    const armed = endpoint orelse return refused(.exec_failed);
    const program_z = try arena.dupeZ(u8, program);
    const freeze = &table.extra.freeze;
    table.lock();
    if (freeze.stage != .none) {
        table.unlock();
        return refused(.busy);
    }
    if (precondition(table)) |reason| {
        table.unlock();
        return refused(reason);
    }
    freeze.stage = .checking;
    const copy = snapshot(arena, table, armed) catch |err| {
        thaw(table);
        table.unlock();
        return err;
    };
    table.unlock();
    if (!check.passes(program_z, copy)) return ended(table, .check_failed);

    table.lock();
    freeze.stage = .stopping;
    freeze.parked[own_slot] = own;
    for (&table.connections) |*slot| if (slot.in_use) handles.signal(slot.wake.?);
    table.unlock();
    if (!awaitStopped(table)) return ended(table, .busy);

    // From here the lock is held through `execve`: every connection and the listener are stopped,
    // so only the watcher waits, and nothing changes between the snapshot and the next image.
    table.lock();
    defer table.unlock();
    defer thaw(table);
    if (precondition(table)) |reason| return refused(reason);
    const bytes = try snapshot(arena, table, armed);
    execute(arena, table, armed, program_z, bytes);
    return refused(.exec_failed);
}

fn refused(reason: message.Reason) Body {
    return .{ .refused = reason };
}

fn ended(table: *state.Table, reason: message.Reason) Body {
    table.lock();
    defer table.unlock();
    thaw(table);
    return refused(reason);
}

/// Ends the update in this image and lets every stopped thread go on. Called with the lock held.
fn thaw(table: *state.Table) void {
    const freeze = &table.extra.freeze;
    freeze.stage = .none;
    freeze.parked = @splat(null);
    freeze.thaw.broadcast();
}

/// Why the table cannot be carried across now, or null. Called with the lock held.
fn precondition(table: *const state.Table) ?message.Reason {
    if (table.lifecycle != .ready) return .draining;
    if (table.extra.spawning_count > 0) return .busy;
    for (table.holdings[0..table.holding_count]) |*holding| if (holding.mark != null) return .marked;
    for (table.holdings[0..table.holding_count]) |*holding| {
        if (holding.orphaned) return .orphaned;
        const index = table.findHost(holding.owner) orelse return .orphaned;
        if (table.hosts[index].?.ended) return .orphaned;
    }
    return null;
}

fn awaitStopped(table: *state.Table) bool {
    var timer = monotonic.Timer.start() catch return false;
    while (true) {
        table.lock();
        const stopped = allStopped(table);
        table.unlock();
        if (stopped) return true;
        if (timer.read() >= stop_deadline_ns) return false;
        monotonic.sleep(stop_slice_ns);
    }
}

fn allStopped(table: *const state.Table) bool {
    const freeze = &table.extra.freeze;
    if (!freeze.listener_parked or table.pending_connections > 0) return false;
    for (table.connections, 0..) |slot, index| {
        if (slot.in_use and slot.kind != .pending and freeze.parked[index] == null) return false;
    }
    return true;
}

/// The state of the table and of each stopped connection. Called with the lock held.
fn snapshot(arena: std.mem.Allocator, table: *const state.Table, armed: Endpoint) error{OutOfMemory}![]u8 {
    var hosts: std.ArrayList(saved.Host) = .empty;
    for (table.hosts) |slot| if (slot) |host| {
        try hosts.append(arena, .{ .process = try saved.processText(arena, host.process), .ended = host.ended });
    };
    const holdings = try arena.alloc(saved.Holding, table.holding_count);
    for (table.holdings[0..table.holding_count], holdings) |*holding, *out| {
        const parent = switch (holding.content) {
            inline else => |*held| held.parent,
        };
        out.* = .{
            .holding = try saved.idText(arena, holding.id),
            .owner = try saved.processText(arena, holding.owner),
            .claimed = if (holding.claimed) |claimed| .{
                .from = try saved.processText(arena, claimed.from),
                .mark = try saved.idText(arena, claimed.mark),
            } else null,
            .parented = parent.parented,
            .ended = if (parent.ended) |status| saved.exitText(status) else null,
            .reaped = parent.reaped,
            .exitSent = parent.exit_sent,
        };
        switch (holding.content) {
            .pty => |*held| {
                out.master = held.master;
                out.facts = try saved.factsText(arena, try held.facts.view(arena));
            },
            .pipes => |*held| out.pipes = .{
                .facts = try saved.programFactsText(arena, try held.facts.view(arena)),
                .stdout = held.streams[0],
                .stderr = held.streams[1],
            },
        }
    }
    var connections: std.ArrayList(saved.Connection) = .empty;
    for (table.extra.freeze.parked, 0..) |entry, index| if (entry) |parked| {
        var names: std.ArrayList([]const u8) = .empty;
        inline for (@typeInfo(Capabilities).@"struct".fields) |field| {
            if (@field(parked.capabilities, field.name)) try names.append(arena, field.name);
        }
        try connections.append(arena, .{
            .socket = parked.socket,
            .role = @tagName(parked.peer),
            .pid = table.connections[index].pid,
            .host = if (parked.peer == .host) try saved.processText(arena, parked.peer.host) else null,
            .capabilities = names.items,
            .usedIds = parked.used,
            .pending = parked.pending,
        });
    };
    return saved.encode(arena, .{
        .format = saved.format,
        .previous = armed.program,
        .listener = armed.listener,
        .lock = armed.lock,
        .hosts = hosts.items,
        .holdings = holdings,
        .released = table.extra.released[0..table.extra.released_count],
        .connections = connections.items,
    });
}

/// Writes the state and executes `program` with it; returns only when `execve` failed, with every
/// descriptor close-on-exec again.
fn execute(arena: std.mem.Allocator, table: *const state.Table, armed: Endpoint, program: [:0]const u8, bytes: []const u8) void {
    const descriptor = writeState(arena, armed.runtime_directory, bytes) orelse return;
    defer posix_compat.close(descriptor);
    const argv = arguments(arena, .{
        .program = program,
        .endpoint = armed,
        .state = descriptor,
        .previous = armed.program,
        .returned = false,
    }) catch return;
    inheritAll(table, armed, descriptor, true);
    _ = c.execve(program.ptr, argv, @ptrCast(c.environ));
    inheritAll(table, armed, descriptor, false);
}

/// The state file: created in the runtime folder and unlinked at once, so only this process and
/// the image it executes reach it.
fn writeState(arena: std.mem.Allocator, runtime_directory: []const u8, bytes: []const u8) ?posix.fd_t {
    const path = std.fmt.allocPrintSentinel(arena, "{s}/keeper-state-{d}", .{ runtime_directory, c.getpid() }, 0) catch return null;
    // A state file an earlier update left behind is this Keeper's own.
    _ = c.unlink(path.ptr);
    const descriptor = c.open(path.ptr, .{ .ACCMODE = .RDWR, .CREAT = true, .EXCL = true, .CLOEXEC = true, .NOFOLLOW = true }, @as(c_uint, 0o600));
    if (descriptor < 0) return null;
    _ = c.unlink(path.ptr);
    var written: usize = 0;
    while (written < bytes.len) {
        const count = c.write(descriptor, bytes[written..].ptr, bytes.len - written);
        if (count > 0) {
            written += @intCast(count);
        } else if (count < 0 and posix.errno(count) == .INTR) {
            continue;
        } else {
            posix_compat.close(descriptor);
            return null;
        }
    }
    return descriptor;
}

fn inheritAll(table: *const state.Table, armed: Endpoint, descriptor: posix.fd_t, on: bool) void {
    inherit(descriptor, on);
    inherit(armed.listener, on);
    inherit(armed.lock, on);
    for (table.holdings[0..table.holding_count]) |*holding| switch (holding.content) {
        .pty => |held| inherit(held.master, on),
        .pipes => |held| for (held.streams) |stream| if (stream) |read_end| inherit(read_end, on),
    };
    for (table.extra.freeze.parked) |entry| if (entry) |parked| inherit(parked.socket, on);
}

/// Clears close-on-exec on `descriptor`, or sets it again.
pub fn inherit(descriptor: posix.fd_t, on: bool) void {
    _ = posix_compat.fcntl(descriptor, posix.F.SETFD, if (on) 0 else posix.FD_CLOEXEC) catch {};
}

pub const Execution = struct {
    program: [:0]const u8,
    endpoint: Endpoint,
    state: posix.fd_t,
    previous: []const u8,
    returned: bool,
};

/// The arguments of an image executed in place: its `keep` options and the state it takes over.
pub fn arguments(arena: std.mem.Allocator, execution: Execution) error{OutOfMemory}![*:null]const ?[*:0]const u8 {
    const ends = execution.endpoint;
    var list: std.ArrayList(?[*:0]const u8) = .empty;
    try list.appendSlice(arena, &.{ execution.program.ptr, "keep", "--runtime-directory", (try arena.dupeZ(u8, ends.runtime_directory)).ptr, "--endpoint-id", (try arena.dupeZ(u8, ends.endpoint_id)).ptr });
    if (ends.endpoint_directory) |directory| try list.appendSlice(arena, &.{ "--endpoint-directory", (try arena.dupeZ(u8, directory)).ptr });
    const descriptor = try std.fmt.allocPrintSentinel(arena, "{d}", .{execution.state}, 0);
    try list.appendSlice(arena, &.{ "--resume", descriptor.ptr, "--previous", (try arena.dupeZ(u8, execution.previous)).ptr });
    if (execution.returned) try list.append(arena, "--returned");
    try list.append(arena, null);
    return @ptrCast(list.items.ptr);
}
