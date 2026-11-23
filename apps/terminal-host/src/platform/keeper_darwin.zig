//! What the Keeper and the Host's Keeper link need from macOS: a Unix stream socket that carries
//! descriptors by `SCM_RIGHTS`, the peer's process id, a process's start time, a PTY master's slave
//! name, and a wake one descriptor carries. A write attaches the descriptors its message carries to
//! the frame's first byte, and a read keeps the descriptors that arrived until the decoded message
//! takes them. A message takes exactly the count it names; a short count, or control data the
//! kernel truncated, closes every descriptor that arrived, so none outlives a message nobody took.

const std = @import("std");
const monotonic = @import("monotonic.zig");
const posix_compat = @import("posix_compat.zig");

const posix = std.posix;

/// The most descriptors one message carries.
pub const per_message = 64;
pub const maximum_slave_bytes = 128;

extern "c" fn orchbee_send_with_descriptors(socket: c_int, bytes: [*]const u8, length: usize, descriptors: [*]const c_int, count: usize, sent: *usize) c_int;
extern "c" fn orchbee_receive_with_descriptors(socket: c_int, bytes: [*]u8, capacity: usize, descriptors: [*]c_int, descriptor_capacity: usize, read_length: *usize, descriptor_count: *usize, truncated: *c_int) c_int;
extern "c" fn orchbee_peer_process_id(socket: c_int, pid: *i32) c_int;
extern "c" fn orchbee_process_start_microseconds(pid: i32, start: *u64) c_int;
extern "c" fn orchbee_pty_slave_name(master: c_int, name: *[maximum_slave_bytes]u8) c_int;
extern "c" fn orchbee_raise_descriptor_limit() void;
extern "c" fn ioctl(fd: c_int, request: c_ulong, ...) c_int;

const FIONREAD: c_ulong = 0x4004667f;
const EAGAIN: c_int = 35;
const F_DUPFD_CLOEXEC: c_int = 67;

/// The process id of the socket's peer when it connected, or null when it cannot be read.
pub fn peerProcessId(socket: posix.fd_t) ?u32 {
    var pid: i32 = 0;
    if (orchbee_peer_process_id(socket, &pid) != 0 or pid <= 0) return null;
    return @intCast(pid);
}

/// A process's start time in microseconds since 1970, or null when it is gone or unreadable.
pub fn processStart(pid: u32) ?u64 {
    if (pid == 0 or pid > std.math.maxInt(i32)) return null;
    var start: u64 = 0;
    if (orchbee_process_start_microseconds(@intCast(pid), &start) != 0) return null;
    return start;
}

/// Whether `master` is a PTY master whose slave is named `slave`.
pub fn masterOf(master: posix.fd_t, slave: []const u8) bool {
    var name: [maximum_slave_bytes]u8 = undefined;
    if (orchbee_pty_slave_name(master, &name) != 0) return false;
    const length = std.mem.indexOfScalar(u8, &name, 0) orelse return false;
    return std.mem.eql(u8, name[0..length], slave);
}

/// A close-on-exec duplicate of `descriptor`, or null.
pub fn duplicate(descriptor: posix.fd_t) ?posix.fd_t {
    return @intCast(posix_compat.fcntl(descriptor, F_DUPFD_CLOEXEC, 3) catch return null);
}

pub fn raiseDescriptorLimit() void {
    orchbee_raise_descriptor_limit();
}

pub const Stream = struct {
    socket: posix.fd_t,
    /// Descriptors that arrived with bytes no message has taken yet, oldest first.
    arrived: [per_message]posix.fd_t = undefined,
    arrived_count: usize = 0,
    /// More descriptors arrived than fit, or the kernel truncated the control data.
    truncated: bool = false,

    pub fn init(socket: posix.fd_t) Stream {
        return .{ .socket = socket };
    }

    /// Closes the socket and every descriptor no message took.
    pub fn deinit(self: *Stream) void {
        self.discardDescriptors();
        posix_compat.close(self.socket);
        self.* = undefined;
    }

    pub fn clientProcessId(self: *const Stream) ?u32 {
        return peerProcessId(self.socket);
    }

    /// Reads available bytes into `bytes`, keeping the descriptors that arrived with them. A peer's
    /// close is `ConnectionClosed`; zero means nothing was ready.
    pub fn read(self: *Stream, bytes: []u8) error{ConnectionClosed}!usize {
        var room: [per_message]c_int = undefined;
        var length: usize = 0;
        var count: usize = 0;
        var truncated: c_int = 0;
        const status = orchbee_receive_with_descriptors(self.socket, bytes.ptr, bytes.len, &room, room.len, &length, &count, &truncated);
        if (status == EAGAIN) return 0;
        if (status != 0) return error.ConnectionClosed;
        if (truncated != 0) self.truncated = true;
        for (room[0..count]) |descriptor| {
            if (self.arrived_count == self.arrived.len) {
                posix_compat.close(descriptor);
                self.truncated = true;
                continue;
            }
            self.arrived[self.arrived_count] = descriptor;
            self.arrived_count += 1;
        }
        if (length == 0) return error.ConnectionClosed;
        return length;
    }

    /// Moves exactly `into.len` descriptors to the caller, who owns them; false, with every
    /// descriptor that arrived closed, when fewer arrived, more arrived, or some were lost.
    pub fn takeDescriptors(self: *Stream, into: []posix.fd_t) bool {
        if (self.truncated or self.arrived_count != into.len) {
            self.discardDescriptors();
            return false;
        }
        @memcpy(into, self.arrived[0..into.len]);
        self.arrived_count = 0;
        return true;
    }

    /// Closes every descriptor that arrived and no message took.
    pub fn discardDescriptors(self: *Stream) void {
        for (self.arrived[0..self.arrived_count]) |descriptor| posix_compat.close(descriptor);
        self.arrived_count = 0;
        self.truncated = false;
    }

    /// The bytes the socket holds that no read has taken.
    pub fn pendingBytes(self: *Stream) error{ConnectionClosed}!u32 {
        var count: c_int = 0;
        if (ioctl(self.socket, FIONREAD, &count) != 0 or count < 0) return error.ConnectionClosed;
        return @intCast(count);
    }

    /// Waits up to `timeout_ms` until the socket is readable or `wake` signals; true when readable.
    /// A closed or failed socket reads as readable, so the next read reports it.
    pub fn waitReadableOrWake(self: *Stream, timeout_ms: u32, wake: ?posix.fd_t) error{ConnectionClosed}!bool {
        var descriptors = [_]posix.pollfd{
            .{ .fd = self.socket, .events = posix.POLL.IN, .revents = 0 },
            .{ .fd = wake orelse -1, .events = posix.POLL.IN, .revents = 0 },
        };
        const count: usize = if (wake == null) 1 else 2;
        const ready = posix.poll(descriptors[0..count], @intCast(@min(timeout_ms, std.math.maxInt(i32)))) catch
            return error.ConnectionClosed;
        if (ready == 0) return false;
        if (count == 2 and descriptors[1].revents != 0) wake_events.consume(wake.?);
        return descriptors[0].revents != 0;
    }

    pub fn writeAllWithin(self: *Stream, bytes: []const u8, timeout_ns: u64) error{ WriteTimeout, ConnectionClosed }!void {
        return self.writeWithDescriptors(bytes, &.{}, timeout_ns);
    }

    /// Writes all of `bytes` within `timeout_ns`, with `descriptors` attached to the first byte.
    pub fn writeWithDescriptors(self: *Stream, bytes: []const u8, descriptors: []const posix.fd_t, timeout_ns: u64) error{ WriteTimeout, ConnectionClosed }!void {
        if (descriptors.len > per_message) return error.ConnectionClosed;
        var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
        var written: usize = 0;
        var attach = descriptors;
        while (written < bytes.len) {
            var sent: usize = 0;
            const status = orchbee_send_with_descriptors(self.socket, bytes[written..].ptr, bytes.len - written, attach.ptr, attach.len, &sent);
            if (status == 0) {
                written += sent;
                attach = &.{};
                continue;
            }
            if (status != EAGAIN) return error.ConnectionClosed;
            const elapsed = timer.read();
            if (elapsed >= timeout_ns) return error.WriteTimeout;
            var ready = [_]posix.pollfd{.{ .fd = self.socket, .events = posix.POLL.OUT, .revents = 0 }};
            const remaining_ms: i32 = @intCast(@max(1, @min(1000, (timeout_ns - elapsed) / std.time.ns_per_ms)));
            _ = posix.poll(&ready, remaining_ms) catch return error.ConnectionClosed;
            if (ready[0].revents & (posix.POLL.ERR | posix.POLL.HUP | posix.POLL.NVAL) != 0) return error.ConnectionClosed;
        }
    }
};

/// A wake that one descriptor carries: a `kqueue` with one user event, which a poll for reading
/// sees once it is triggered, and which a wait consumes, so it resets itself.
pub const wake_events = struct {
    const EVFILT_USER: i16 = -10;
    const EV_ADD: u16 = 0x0001;
    const EV_CLEAR: u16 = 0x0020;
    const NOTE_TRIGGER: u32 = 0x0100_0000;

    pub fn create() ?posix.fd_t {
        const queue = std.c.kqueue();
        if (queue < 0) return null;
        _ = posix_compat.fcntl(queue, posix.F.SETFD, posix.FD_CLOEXEC) catch {};
        const change = [_]std.c.Kevent{.{ .ident = 1, .filter = EVFILT_USER, .flags = EV_ADD | EV_CLEAR, .fflags = 0, .data = 0, .udata = 0 }};
        var none: [0]std.c.Kevent = .{};
        if (std.c.kevent(queue, &change, 1, &none, 0, null) != 0) {
            posix_compat.close(queue);
            return null;
        }
        return queue;
    }

    pub fn signal(queue: posix.fd_t) void {
        const change = [_]std.c.Kevent{.{ .ident = 1, .filter = EVFILT_USER, .flags = 0, .fflags = NOTE_TRIGGER, .data = 0, .udata = 0 }};
        var none: [0]std.c.Kevent = .{};
        _ = std.c.kevent(queue, &change, 1, &none, 0, null);
    }

    pub fn consume(queue: posix.fd_t) void {
        var events: [1]std.c.Kevent = undefined;
        const none: [0]std.c.Kevent = .{};
        const zero: std.c.timespec = .{ .sec = 0, .nsec = 0 };
        _ = std.c.kevent(queue, &none, 0, &events, 1, &zero);
    }
};

extern "c" fn orchbee_try_observe_process_exit_now(pid: c_int, kind: *c_int, value: *u32) c_int;
const WNOHANG: c_int = 1;

/// How a process ended: its exit code, or the signal that ended it.
pub const Exit = union(enum) { code: u8, signal: u8 };

/// How the Keeper's child `pid` ended, once it has, without reaping it; null while it runs.
pub fn peekExit(pid: u32) ?Exit {
    if (pid == 0 or pid > std.math.maxInt(i32)) return null;
    var kind: c_int = 0;
    var value: u32 = 0;
    if (orchbee_try_observe_process_exit_now(@intCast(pid), &kind, &value) != 0) return null;
    return switch (kind) {
        1 => .{ .code = @truncate(value) },
        else => .{ .signal = @intCast(@max(1, @min(255, value))) },
    };
}

/// Reaps the Keeper's ended child `pid`.
pub fn reap(pid: u32) void {
    if (pid == 0 or pid > std.math.maxInt(i32)) return;
    _ = posix_compat.waitpid(@intCast(pid), WNOHANG);
}
