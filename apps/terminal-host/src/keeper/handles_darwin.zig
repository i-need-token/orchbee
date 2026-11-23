//! The Keeper's macOS process operations: checking a registered Host, the wake that ends one
//! connection's readiness wait, and one watcher thread that sees a registered Host end and a
//! parented leader exit through one `kqueue`. A Host is named by its process id; its watch is an
//! `EVFILT_PROC` filter on that id, which fires once. A leader's exit only wakes the watcher, which
//! also wakes once a second, so the table's waitid poll finds an exit a missed filter would hide.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const darwin = platform.keeper.darwin;
const posix_compat = platform.posix_compat;
const message = keeper_protocol.message;
const posix = std.posix;

/// A connection's wake: a `kqueue` with one user event.
pub const Handle = posix.fd_t;
/// A registered Host, by its process id.
pub const HostHandle = u32;
/// A Host's armed watch, by the process id it watches.
pub const Wait = u32;
/// What the watcher runs for a Host's end, with the context its watch was armed with.
pub const EndCallback = *const fn (context: usize) void;
/// What the watcher runs on any leader's exit, and once a second.
pub const PollCallback = *const fn () void;

/// A Host's watch fires once, and the kernel drops it then.
pub const watch_spent_on_fire = true;
/// Removing a watch never waits, so the table removes it under its lock, before a later
/// registration of the same process id could arm it again.
pub const unwatch_under_lock = true;

pub const OpenError = error{ HandleFailed, IdentityMismatch };

const EVFILT_PROC: i16 = -5;
const EV_ADD: u16 = 0x0001;
const EV_DELETE: u16 = 0x0002;
const EV_ONESHOT: u16 = 0x0010;
const NOTE_EXIT: u32 = 0x8000_0000;
/// A watch's context with this bit set is a leader's, which only wakes the poll.
const leader_tag: usize = @as(usize, 1) << (@bitSizeOf(usize) - 1);
const poll_interval_ns = std.time.ns_per_s;

var watcher_queue: posix.fd_t = -1;

/// Opens the one watch queue and starts its thread; the Keeper calls it once, before it serves.
pub fn startWatcher(on_host_end: EndCallback, on_poll: PollCallback) error{SystemResources}!void {
    const queue = std.c.kqueue();
    if (queue < 0) return error.SystemResources;
    _ = posix_compat.fcntl(queue, posix.F.SETFD, posix.FD_CLOEXEC) catch {};
    watcher_queue = queue;
    const thread = std.Thread.spawn(.{ .stack_size = 128 * 1024 }, watch, .{ on_host_end, on_poll }) catch
        return error.SystemResources;
    thread.detach();
}

fn watch(on_host_end: EndCallback, on_poll: PollCallback) void {
    platform.fatal_record.markThread(.@"keeper-listener");
    var events: [16]std.c.Kevent = undefined;
    const none: [0]std.c.Kevent = .{};
    const interval: std.c.timespec = .{ .sec = @intCast(poll_interval_ns / std.time.ns_per_s), .nsec = 0 };
    while (true) {
        const count = std.c.kevent(watcher_queue, &none, 0, &events, events.len, &interval);
        if (count > 0) for (events[0..@intCast(count)]) |event| {
            if (event.udata & leader_tag == 0) on_host_end(event.udata);
        };
        on_poll();
    }
}

/// Checks a Host the Worker registers: it runs, with the start time the Worker named.
pub fn openHost(process: message.Process) OpenError!HostHandle {
    const started = darwin.processStart(process.pid) orelse return error.HandleFailed;
    if (started != process.created) return error.IdentityMismatch;
    return process.pid;
}

pub fn closeHost(_: HostHandle) void {}

/// Arms the watch that runs the end callback with `context` once `host` ends; null when the
/// process is already gone.
pub fn watchEnd(host: HostHandle, _: EndCallback, context: usize) ?Wait {
    if (!change(host, EV_ADD | EV_ONESHOT, context)) return null;
    return host;
}

/// Removes a watch that has not fired. Never waits.
pub fn unwatch(wait: Wait) void {
    _ = change(wait, EV_DELETE, 0);
}

pub fn unwatchFromCallback(wait: Wait) void {
    unwatch(wait);
}

/// Wakes the poll once `leader`, the Keeper's own child, exits. A leader that already exited is
/// found by the next poll.
pub fn watchLeader(leader: u32) void {
    _ = change(leader, EV_ADD | EV_ONESHOT, leader_tag);
}

fn change(pid: u32, flags: u16, context: usize) bool {
    const changes = [_]std.c.Kevent{.{
        .ident = pid,
        .filter = EVFILT_PROC,
        .flags = flags,
        .fflags = NOTE_EXIT,
        .data = 0,
        .udata = context,
    }};
    var none: [0]std.c.Kevent = .{};
    return std.c.kevent(watcher_queue, &changes, 1, &none, 0, null) == 0;
}

pub fn createWakeEvent() ?Handle {
    return darwin.wake_events.create();
}

pub fn signal(event: Handle) void {
    darwin.wake_events.signal(event);
}

pub fn close(handle: Handle) void {
    posix_compat.close(handle);
}

pub fn closeAll(handles: []const Handle) void {
    for (handles) |handle| close(handle);
}

pub fn currentProcessId() u32 {
    return @intCast(std.c.getpid());
}
