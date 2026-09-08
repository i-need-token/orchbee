const std = @import("std");

const private_ipc = @import("ssh-runtime-private-ipc");

const bridge_buffer_bytes = 8192;
const connect_attempts = 100;
const connect_retry_ns = 50 * std.time.ns_per_ms;
const queued_read_request = switch (@import("builtin").os.tag) {
    .linux => std.c.T.FIONREAD,
    // Darwin sys/filio.h: _IOR('f', 127, int); Zig 0.16 does not expose this constant.
    .macos => 0x4004667f,
    else => @compileError("unsupported SSH Runtime attachment platform"),
};

pub fn attach(
    io: std.Io,
    allocator: std.mem.Allocator,
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
) !void {
    try private_ipc.validateRuntimeDirectory(runtime_directory);
    var directory = try std.Io.Dir.openDirAbsolute(io, runtime_directory, .{
        .follow_symlinks = false,
    });
    defer directory.close(io);
    if (std.c.fchdir(directory.handle) != 0) return error.RuntimeDirectoryUnavailable;
    const endpoint_name = try std.fmt.allocPrint(allocator, "{s}.s", .{runtime_instance_id});
    defer allocator.free(endpoint_name);
    try private_ipc.validateEndpoint(endpoint_name);
    const address = std.Io.net.UnixAddress.init(endpoint_name) catch
        return error.EndpointPathTooLong;
    const stream = try connectRuntime(io, address);
    defer stream.close(io);
    try private_ipc.requirePeerEffectiveUid(stream.socket.handle);

    const stdin_descriptor = std.Io.File.stdin().handle;
    const stdout_descriptor = std.Io.File.stdout().handle;
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    while (true) {
        var descriptors = [_]std.posix.pollfd{
            .{
                .fd = stdin_descriptor,
                .events = std.posix.POLL.IN,
                .revents = 0,
            },
            .{
                .fd = stream.socket.handle,
                .events = std.posix.POLL.IN,
                .revents = 0,
            },
        };
        _ = try std.posix.poll(&descriptors, -1);

        if (!try bridgeStep(&descriptors, stdout_descriptor, &buffer)) return;
    }
}

fn bridgeStep(descriptors: *[2]std.posix.pollfd, stdout_descriptor: std.posix.fd_t, buffer: []u8) !bool {
    if (descriptors[1].revents & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
        try drainQueuedOutput(descriptors[1].fd, stdout_descriptor, buffer);
        return false;
    }
    if (descriptors[1].revents & std.posix.POLL.IN != 0) {
        const length = try std.posix.read(descriptors[1].fd, buffer);
        if (length == 0) return false;
        try writeAll(stdout_descriptor, buffer[0..length]);
    }
    if (descriptors[0].revents & std.posix.POLL.IN != 0) {
        const length = try std.posix.read(descriptors[0].fd, buffer);
        if (length == 0) return false;
        sendAll(descriptors[1].fd, buffer[0..length]) catch |err| {
            if (err == error.ConnectionClosed) {
                // Preserve the write failure even if stdout cannot accept the queued prefix.
                drainQueuedOutput(descriptors[1].fd, stdout_descriptor, buffer) catch {};
            }
            return err;
        };
    }
    if (descriptors[0].revents & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
        return false;
    }
    return true;
}

fn connectRuntime(io: std.Io, address: std.Io.net.UnixAddress) !std.Io.net.Stream {
    for (0..connect_attempts) |attempt| {
        const stream = address.connect(io) catch |err| {
            if (attempt + 1 == connect_attempts) return err;
            std.Io.sleep(io, .fromNanoseconds(connect_retry_ns), .awake) catch {};
            continue;
        };
        return stream;
    }
    unreachable;
}

fn sendAll(socket: std.posix.socket_t, bytes: []const u8) !void {
    var sender = SocketSender{ .socket = socket };
    try sendAllUsing(&sender, bytes);
}

const SendAttempt = struct { count: isize, errno: std.posix.E };

const SocketSender = struct {
    socket: std.posix.socket_t,

    fn send(self: *SocketSender, bytes: []const u8) SendAttempt {
        const sent = std.c.send(self.socket, bytes.ptr, bytes.len, std.posix.MSG.NOSIGNAL);
        return .{ .count = sent, .errno = std.posix.errno(sent) };
    }
};

fn sendAllUsing(sender: anytype, bytes: []const u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        const attempt = sender.send(bytes[offset..]);
        switch (attempt.errno) {
            .SUCCESS => {
                if (attempt.count == 0) return error.ConnectionClosed;
                offset += @intCast(attempt.count);
            },
            .INTR => continue,
            .PIPE, .CONNRESET, .NOTCONN => return error.ConnectionClosed,
            .BADF => return error.InvalidSocket,
            else => |err| return std.posix.unexpectedErrno(err),
        }
    }
}

fn queuedReadLimit(socket: std.posix.socket_t) usize {
    var queued: c_int = 0;
    if (std.c.ioctl(socket, queued_read_request, @intFromPtr(&queued)) < 0 or queued <= 0) return 0;
    return @intCast(queued);
}

fn drainQueuedOutput(socket: std.posix.socket_t, stdout_descriptor: std.posix.fd_t, buffer: []u8) !void {
    try drainQueuedPrefix(socket, stdout_descriptor, queuedReadLimit(socket), buffer);
}

fn drainQueuedPrefix(socket: std.posix.socket_t, stdout_descriptor: std.posix.fd_t, queued: usize, buffer: []u8) !void {
    var remaining = queued;
    // One kernel-reported work cap, which may overcount after partial reads.
    // Later peer writes cannot extend the cap, but can arrive within it.
    while (remaining > 0) {
        const count = std.c.recv(socket, buffer.ptr, @min(remaining, buffer.len), std.posix.MSG.DONTWAIT);
        if (count <= 0) return;
        const length: usize = @intCast(count);
        remaining -= length;
        try writeAll(stdout_descriptor, buffer[0..length]);
    }
}

fn writeAll(descriptor: std.posix.fd_t, bytes: []const u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        const written = std.posix.system.write(
            descriptor,
            bytes[offset..].ptr,
            bytes.len - offset,
        );
        switch (std.posix.errno(written)) {
            .SUCCESS => offset += @intCast(written),
            .INTR => continue,
            .PIPE => return error.ConnectionClosed,
            else => |err| return std.posix.unexpectedErrno(err),
        }
    }
}

const Attachment = @This();

pub const testing = if (@import("builtin").is_test) struct {
    pub const bridge_buffer_bytes = Attachment.bridge_buffer_bytes;
    pub const bridgeStep = Attachment.bridgeStep;
    pub const sendAll = Attachment.sendAll;
    pub const sendAllUsing = Attachment.sendAllUsing;
    pub const SendAttempt = Attachment.SendAttempt;
    pub const queuedReadLimit = Attachment.queuedReadLimit;
    pub const drainQueuedOutput = Attachment.drainQueuedOutput;
    pub const drainQueuedPrefix = Attachment.drainQueuedPrefix;
} else struct {};
