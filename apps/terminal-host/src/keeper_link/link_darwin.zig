//! A Keeper link's macOS side (`link.zig`): it connects over the Keeper's socket, holds each
//! terminal by sending its PTY master with `hold-pty`, starts the leader of each new terminal a
//! creation hands it with `spawn`, reaps each parented leader its terminal asks for, reads the
//! `exited` events the Keeper sends between requests, and ends the parent of every leader it
//! parents before it reports the attachment gone. A master moves with its request, so an
//! unanswered hold leaves nothing to keep.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");
const core = @import("link.zig");
const connect = @import("connect_darwin.zig");
const holds = @import("hold_darwin.zig");
const parenting = @import("parent_darwin.zig");
const move_requests = @import("moves.zig");

const message = keeper_protocol.message;
const wake_events = platform.keeper.darwin.wake_events;
const parent_module = platform.keeper.parent;
const Gate = platform.keeper.Gate;
const Process = platform.process.Process;
const posix = std.posix;

/// How long an idle link waits before it checks the Keeper process again.
const idle_slice_ms = 1000;

pub const Link = core.Link(@This());
pub const Phase = core.Phase;
pub const Held = core.Held;
pub const Queued = holds.Queued;
/// macOS Hosts hold no background programs.
pub const Program = void;
const SpawnCall = parenting.SpawnCall;
const recordOf = parenting.recordOf;

pub const Wake = posix.fd_t;
pub const Connection = connect.Connection;
pub const Watch = connect.Watch;
/// The Keeper's socket path.
pub const Endpoint = []const u8;
/// The descriptors a request sends beside it.
pub const Sent = []const posix.fd_t;
pub const nothing_sent: Sent = &.{};

pub const Own = struct {
    /// The Keeper's socket, owned.
    socket_path: [:0]u8,
    // Guarded by `mutex`.
    /// The Keeper negotiated `parent`, so it starts leaders.
    parent: bool = false,
    /// Creations waiting for their `spawn`, in order.
    spawns: std.ArrayList(*SpawnCall) = .empty,
    /// Creations that may still hand this link a `spawn`; the link is freed only once none does.
    users: u32 = 0,

    pub fn init(allocator: std.mem.Allocator, endpoint: Endpoint) !Own {
        return .{ .socket_path = try allocator.dupeZ(u8, endpoint) };
    }

    pub fn deinit(self: *Own, allocator: std.mem.Allocator) void {
        self.spawns.deinit(allocator);
        allocator.free(self.socket_path);
    }
};

pub fn createWake() ?Wake {
    return wake_events.create();
}

pub fn signal(wake: Wake) void {
    wake_events.signal(wake);
}

pub fn closeWake(wake: Wake) void {
    platform.posix_compat.close(wake);
}

pub fn queuedTerminal(process: *Process, terminal: message.Terminal) Queued {
    return .{ .gate = undefined, .holding = undefined, .process = process, .terminal = terminal };
}

pub fn unheldTerminal(item: Queued) ?[16]u8 {
    return item.terminal.terminal_id;
}

/// Takes a creation's claim on this link, which keeps it from being freed; false when the link
/// starts no leaders.
pub fn pin(link: *Link) bool {
    link.mutex.lock();
    defer link.mutex.unlock();
    if (!link.accepting or !link.own.parent) return false;
    link.own.users += 1;
    return true;
}

pub fn unpin(link: *Link) void {
    link.mutex.lock();
    defer link.mutex.unlock();
    link.own.users -= 1;
    platform.output_wake.signal();
}

/// The creation thread: starts a new terminal's leader through this Keeper, within its reply
/// deadline once the link takes it.
pub fn spawnLeader(link: *Link, plan: *const parent_module.Plan, terminal: message.Terminal) parent_module.Outcome {
    var waiting: SpawnCall = .{ .plan = plan, .terminal = terminal };
    {
        link.mutex.lock();
        defer link.mutex.unlock();
        if (!link.accepting) return .refused;
        link.own.spawns.append(link.allocator, &waiting) catch return .refused;
        wake_events.signal(link.wake);
    }
    return waiting.wait();
}

pub fn open(link: *Link) bool {
    link.connection = connect.open(link.keeper, link.own.socket_path.ptr, .host, core.hello_id, link.watch(), link.allocator) catch return false;
    if (!link.connection.?.pty) return false;
    link.connection.?.events = .{ .context = link, .exited = parenting.onExited };
    link.mutex.lock();
    defer link.mutex.unlock();
    link.own.parent = link.connection.?.parent;
    return true;
}

pub fn serve(link: *Link, detaching: bool) void {
    move_requests.serve(link);
    if (!link.keeperEnded()) serveSpawns(link, detaching);
    if (!link.keeperEnded()) parenting.serveReaps(link);
    if (!link.keeperEnded()) link.serveHolds(detaching);
}

fn nextSpawn(link: *Link) ?*SpawnCall {
    link.mutex.lock();
    defer link.mutex.unlock();
    if (link.own.spawns.items.len == 0) return null;
    return link.own.spawns.orderedRemove(0);
}

fn serveSpawns(link: *Link, detaching: bool) void {
    while (nextSpawn(link)) |pending| {
        pending.complete(if (detaching or link.failed) .refused else parenting.spawn(link, pending));
        if (link.keeperEnded()) return;
    }
}

/// Waits for work, reading what the Keeper sends meanwhile; false when the Keeper ended.
pub fn idle(link: *Link) bool {
    while (true) {
        if (link.keeperEnded()) return false;
        if (link.connection.?.waitIdle(link.watch(), idle_slice_ms)) readEvents(link);
        if (link.woke) return true;
    }
}

/// Reads the Keeper's events; a broken exchange fails the link, and a closed socket after the
/// Keeper ended is its loss, which the next check sees.
fn readEvents(link: *Link) void {
    var arena_state = std.heap.ArenaAllocator.init(link.allocator);
    defer arena_state.deinit();
    link.connection.?.readEvents(arena_state.allocator(), link.watch()) catch |err| switch (err) {
        error.KeeperEnded => {},
        else => if (!link.failed) link.fail(),
    };
}

pub fn hold(link: *Link, item: Queued, detaching: bool) void {
    const prepared = holds.prepare(item) catch {
        item.gate.settle(link.index);
        return link.postUnheld(item, "hold-failed");
    };
    link.settleHold(item, link.callSending(prepared.body, &.{prepared.master}), detaching);
}

pub fn exchange(connection: *Connection, arena: std.mem.Allocator, request: message.Request, sent: Sent, watch: Watch) core.Exchanged {
    const reply = connection.request(arena, request, sent, watch) catch |err| return switch (err) {
        error.KeeperEnded => .keeper_ended,
        error.Unencodable => .unsent,
        else => .broken,
    };
    return .{ .reply = reply };
}

pub fn breakOff(connection: *Connection) void {
    connection.closeStream();
}

/// A released parented leader is the Keeper's to reap once it ends; any other answer leaves this
/// link no longer its parent.
pub fn released(link: *Link, gate: *Gate, outcome: core.Outcome) void {
    const record = recordOf(gate);
    switch (outcome) {
        .reply => |body| if (body == .release) record.releasedBy(link.serial) else record.lostBy(link.serial),
        else => record.lostBy(link.serial),
    }
}

pub fn lostHeld(link: *Link, entry: core.Held) void {
    recordOf(entry.gate).lostBy(link.serial);
}

/// Nothing was sent for a waiting `spawn`, so each creation forks its leader itself; the caller
/// holds `mutex`.
pub fn settleLocked(link: *Link) void {
    for (link.own.spawns.items) |pending| pending.complete(.refused);
    link.own.spawns.clearRetainingCapacity();
}
