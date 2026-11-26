//! The Keeper's process operations on macOS and Linux: checking a registered Host, the wake that ends
//! one connection's readiness wait, and one watcher thread that sees a registered Host end and a
//! parented leader exit. A Host is named by its process id, and so is its watch, which fires once:
//! on macOS an `EVFILT_PROC` filter on one `kqueue`, on Linux a pidfd the thread polls. A leader's
//! exit only wakes the watcher, which also wakes once a second, so the table's waitid poll finds an
//! exit a missed watch would hide.

const std = @import("std");
const builtin = @import("builtin");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

const native = platform.keeper.native;
const posix_compat = platform.posix_compat;
const thread_sync = platform.thread_sync;
const message = keeper_protocol.message;
const posix = std.posix;

/// A connection's wake (see `platform.keeper.native.wake_events`).
pub const Handle = posix.fd_t;
/// A registered Host, by its process id.
pub const HostHandle = u32;
/// A Host's armed watch, by the process id it watches.
pub const Wait = u32;
/// What the watcher runs for a Host's end, with the context its watch was armed with.
pub const EndCallback = *const fn (context: usize) void;
/// What the watcher runs on any leader's exit, and once a second.
pub const PollCallback = *const fn () void;

/// A Host's watch fires once, and is dropped then.
pub const watch_spent_on_fire = true;
/// Removing a watch never waits, so the table removes it under its lock, before a later
/// registration of the same process id could arm it again.
pub const unwatch_under_lock = true;

pub const OpenError = error{ HandleFailed, IdentityMismatch };

/// A watch's context with this bit set is a leader's, which only wakes the poll.
const leader_tag: usize = @as(usize, 1) << (@bitSizeOf(usize) - 1);
const poll_interval_ms = 1000;
/// The most watches one wait reports; the others report on the next.
const fired_per_wait = 16;

const watcher = if (builtin.os.tag == .linux) pidfd_watcher else kqueue_watcher;

/// Opens the watcher and starts its thread; the Keeper calls it once, before it serves.
pub fn startWatcher(on_host_end: EndCallback, on_poll: PollCallback) error{SystemResources}!void {
    try watcher.open();
    const thread = std.Thread.spawn(platform.thread_stack.sized(128 * 1024), watch, .{ on_host_end, on_poll }) catch
        return error.SystemResources;
    thread.detach();
}

fn watch(on_host_end: EndCallback, on_poll: PollCallback) void {
    platform.fatal_record.markThread(.@"keeper-listener");
    var fired: [fired_per_wait]usize = undefined;
    while (true) {
        const count = watcher.wait(&fired);
        for (fired[0..count]) |context| {
            if (context & leader_tag == 0) on_host_end(context);
        }
        on_poll();
    }
}

/// Checks a Host the Worker registers: it runs, with the start time the Worker named.
pub fn openHost(process: message.Process) OpenError!HostHandle {
    const started = native.processStart(process.pid) orelse return error.HandleFailed;
    if (started != process.created) return error.IdentityMismatch;
    return process.pid;
}

pub fn closeHost(_: HostHandle) void {}

/// Arms the watch that runs the end callback with `context` once `host` ends; null when the
/// process is already gone.
pub fn watchEnd(host: HostHandle, _: EndCallback, context: usize) ?Wait {
    if (!watcher.add(host, context)) return null;
    return host;
}

/// Removes a watch that has not fired. Never waits.
pub fn unwatch(wait: Wait) void {
    watcher.remove(wait);
}

pub fn unwatchFromCallback(wait: Wait) void {
    unwatch(wait);
}

/// Wakes the poll once `leader`, the Keeper's own child, exits. A leader that already exited is
/// found by the next poll.
pub fn watchLeader(leader: u32) void {
    _ = watcher.add(leader, leader_tag);
}

/// The macOS watcher: one `kqueue`, where a watch is an `EVFILT_PROC` filter on its process id,
/// which the kernel drops once it fires. A second watch of a process id replaces the first.
const kqueue_watcher = struct {
    const EVFILT_PROC: i16 = -5;
    const EV_ADD: u16 = 0x0001;
    const EV_DELETE: u16 = 0x0002;
    const EV_ONESHOT: u16 = 0x0010;
    const NOTE_EXIT: u32 = 0x8000_0000;

    var queue: posix.fd_t = -1;

    fn open() error{SystemResources}!void {
        const created = std.c.kqueue();
        if (created < 0) return error.SystemResources;
        _ = posix_compat.fcntl(created, posix.F.SETFD, posix.FD_CLOEXEC) catch {};
        queue = created;
    }

    /// Waits up to the poll interval; the contexts of the watches that fired go to `fired`.
    fn wait(fired: *[fired_per_wait]usize) usize {
        var events: [fired_per_wait]std.c.Kevent = undefined;
        const none: [0]std.c.Kevent = .{};
        const interval: std.c.timespec = .{ .sec = poll_interval_ms / std.time.ms_per_s, .nsec = 0 };
        const count = std.c.kevent(queue, &none, 0, &events, events.len, &interval);
        if (count <= 0) return 0;
        for (events[0..@intCast(count)], fired[0..@intCast(count)]) |event, *context| context.* = event.udata;
        return @intCast(count);
    }

    fn add(pid: u32, context: usize) bool {
        return change(pid, EV_ADD | EV_ONESHOT, context);
    }

    fn remove(pid: u32) void {
        _ = change(pid, EV_DELETE, 0);
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
        return std.c.kevent(queue, &changes, 1, &none, 0, null) == 0;
    }
};

/// The Linux watcher: each watch holds a pidfd, which reads ready once its process ends, and the
/// thread polls them all with a wake that a new watch signals, so the next poll takes it. A watch is
/// dropped once it fires, and a second watch of a process id replaces the first's context, as on
/// macOS. The thread matches what a poll reported by each watch's own number, not its descriptor: a
/// pidfd a removal closed during the poll can come back as another watch's, which that watch's
/// next poll then reports.
const pidfd_watcher = struct {
    const linux = std.os.linux;
    /// A watch for every Host slot, and for every holding's leader and its program's process.
    const capacity = 2 * message.maximum_hosts + 2 * message.maximum_holdings;
    const Watch = struct { pid: u32, descriptor: posix.fd_t, context: usize, number: u64 };

    var mutex: thread_sync.Mutex = .{};
    var watches: [capacity]Watch = undefined;
    var count: usize = 0;
    var next_number: u64 = 0;
    var wake: posix.fd_t = -1;
    /// The watcher thread's own: what one poll waits on, the wake first, and the number of the
    /// watch each other descriptor belongs to.
    var polled: [capacity + 1]posix.pollfd = undefined;
    var numbers: [capacity + 1]u64 = undefined;

    fn open() error{SystemResources}!void {
        wake = native.wake_events.create() orelse return error.SystemResources;
    }

    /// Waits up to the poll interval; the contexts of the watches that fired go to `fired`.
    fn wait(fired: *[fired_per_wait]usize) usize {
        mutex.lock();
        polled[0] = .{ .fd = wake, .events = posix.POLL.IN, .revents = 0 };
        for (watches[0..count], polled[1 .. count + 1], numbers[1 .. count + 1]) |armed, *descriptor, *number| {
            descriptor.* = .{ .fd = armed.descriptor, .events = posix.POLL.IN, .revents = 0 };
            number.* = armed.number;
        }
        const length = count + 1;
        mutex.unlock();
        _ = posix.poll(polled[0..length], poll_interval_ms) catch {
            // Waits on the wake alone rather than turn at once.
            _ = posix.poll(polled[0..1], poll_interval_ms) catch {};
            return 0;
        };
        if (polled[0].revents != 0) native.wake_events.consume(wake);
        mutex.lock();
        defer mutex.unlock();
        var taken: usize = 0;
        for (polled[1..length], numbers[1..length]) |descriptor, number| {
            if (descriptor.revents == 0) continue;
            if (taken == fired.len) break;
            const index = numbered(number) orelse continue;
            posix_compat.close(watches[index].descriptor);
            fired[taken] = watches[index].context;
            taken += 1;
            drop(index);
        }
        return taken;
    }

    fn add(pid: u32, context: usize) bool {
        if (pid == 0 or pid > std.math.maxInt(i32)) return false;
        const opened = linux.pidfd_open(@intCast(pid), 0);
        if (linux.errno(opened) != .SUCCESS) return false;
        const descriptor: posix.fd_t = @intCast(opened);
        mutex.lock();
        defer mutex.unlock();
        if (watching(pid)) |index| {
            watches[index].context = context;
            posix_compat.close(descriptor);
            return true;
        }
        if (count == capacity) {
            posix_compat.close(descriptor);
            return false;
        }
        watches[count] = .{ .pid = pid, .descriptor = descriptor, .context = context, .number = next_number };
        next_number += 1;
        count += 1;
        native.wake_events.signal(wake);
        return true;
    }

    fn remove(pid: u32) void {
        mutex.lock();
        defer mutex.unlock();
        const index = watching(pid) orelse return;
        posix_compat.close(watches[index].descriptor);
        drop(index);
    }

    fn watching(pid: u32) ?usize {
        for (watches[0..count], 0..) |armed, index| if (armed.pid == pid) return index;
        return null;
    }

    fn numbered(number: u64) ?usize {
        for (watches[0..count], 0..) |armed, index| if (armed.number == number) return index;
        return null;
    }

    fn drop(index: usize) void {
        count -= 1;
        watches[index] = watches[count];
    }
};

pub fn createWakeEvent() ?Handle {
    return native.wake_events.create();
}

pub fn signal(event: Handle) void {
    native.wake_events.signal(event);
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
