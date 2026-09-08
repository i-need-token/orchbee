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

const TestSocketPair = struct {
    descriptors: [2]std.posix.socket_t,

    fn init() !TestSocketPair {
        var pair: TestSocketPair = undefined;
        if (std.c.socketpair(std.posix.AF.UNIX, std.posix.SOCK.STREAM, 0, &pair.descriptors) != 0)
            return error.SocketPairFailed;
        errdefer pair.deinit();
        const capacity: c_int = 131_072;
        for (pair.descriptors) |descriptor| {
            try std.posix.setsockopt(descriptor, std.posix.SOL.SOCKET, std.posix.SO.SNDBUF, std.mem.asBytes(&capacity));
            try std.posix.setsockopt(descriptor, std.posix.SOL.SOCKET, std.posix.SO.RCVBUF, std.mem.asBytes(&capacity));
        }
        return pair;
    }

    fn closePeer(self: *TestSocketPair) void {
        _ = std.c.close(self.descriptors[1]);
        self.descriptors[1] = -1;
    }

    fn deinit(self: *TestSocketPair) void {
        for (self.descriptors) |descriptor| {
            if (descriptor >= 0) _ = std.c.close(descriptor);
        }
    }

    fn expectOutput(self: *TestSocketPair, expected: []const u8) !void {
        var received: [bridge_buffer_bytes * 3]u8 = undefined;
        var used: usize = 0;
        while (used < received.len) {
            const count = std.c.recv(self.descriptors[1], received[used..].ptr, received.len - used, std.posix.MSG.DONTWAIT);
            if (count <= 0) break;
            used += @intCast(count);
        }
        try std.testing.expectEqualSlices(u8, expected, received[0..used]);
    }
};

test "terminal send failure forwards the queued split response after a stale poll snapshot" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var input = try TestSocketPair.init();
    defer input.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    try sendAll(input.descriptors[1], "hello");
    var descriptors = [_]std.posix.pollfd{
        .{ .fd = input.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
        .{ .fd = peer.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
    };
    _ = try std.posix.poll(&descriptors, 0);
    try std.testing.expect(descriptors[0].revents & std.posix.POLL.IN != 0);
    try std.testing.expectEqual(@as(i16, 0), descriptors[1].revents);
    const response = "header" ++ "b" ** (bridge_buffer_bytes * 2 + 17);
    try sendAll(peer.descriptors[1], response[0..6]);
    try sendAll(peer.descriptors[1], response[6..]);
    peer.closePeer();
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try std.testing.expectError(error.ConnectionClosed, bridgeStep(&descriptors, output.descriptors[0], &buffer));
    try output.expectOutput(response);
}

test "socket hangup forwards queued output larger than one bridge buffer" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var input = try TestSocketPair.init();
    defer input.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    const response = "h" ** (bridge_buffer_bytes * 2 + 31);
    try sendAll(peer.descriptors[1], response);
    peer.closePeer();
    var descriptors = [_]std.posix.pollfd{
        .{ .fd = input.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
        .{ .fd = peer.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
    };
    _ = try std.posix.poll(&descriptors, 0);
    try std.testing.expect(descriptors[1].revents & std.posix.POLL.HUP != 0);
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try std.testing.expect(!try bridgeStep(&descriptors, output.descriptors[0], &buffer));
    try output.expectOutput(response);
}

test "a half-closed peer cannot extend the captured output prefix with later writes" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    try std.testing.expectEqual(@as(c_int, 0), std.c.shutdown(peer.descriptors[1], std.c.SHUT.RD));
    const prefix = "p" ** (bridge_buffer_bytes + 23);
    try sendAll(peer.descriptors[1], prefix);
    const queued = queuedReadLimit(peer.descriptors[0]);
    try std.testing.expectEqual(prefix.len, queued);
    try sendAll(peer.descriptors[1], "later-one");
    try sendAll(peer.descriptors[1], "later-two");
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try drainQueuedPrefix(peer.descriptors[0], output.descriptors[0], queued, &buffer);
    try output.expectOutput(prefix);
    var later: [32]u8 = undefined;
    const length = std.c.recv(peer.descriptors[0], &later, later.len, std.posix.MSG.DONTWAIT);
    try std.testing.expectEqual(@as(isize, 18), length);
    try std.testing.expectEqualStrings("later-onelater-two", later[0..@intCast(length)]);
}

test "empty or unavailable queued data terminates without fabricating output" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try drainQueuedOutput(peer.descriptors[0], output.descriptors[0], &buffer);
    // An unavailable prefix ends immediately even if its captured bound is nonzero.
    try drainQueuedPrefix(peer.descriptors[0], output.descriptors[0], 1, &buffer);
    peer.closePeer();
    try drainQueuedPrefix(peer.descriptors[0], output.descriptors[0], 1, &buffer);
    try drainQueuedOutput(-1, output.descriptors[0], &buffer);
    try drainQueuedPrefix(-1, output.descriptors[0], 1, &buffer);
    try output.expectOutput("");
}

test "an ordinary live read keeps its single-buffer behavior" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var input = try TestSocketPair.init();
    defer input.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    const response = "l" ** (bridge_buffer_bytes + 1);
    try sendAll(peer.descriptors[1], response);
    var descriptors = [_]std.posix.pollfd{
        .{ .fd = input.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
        .{ .fd = peer.descriptors[0], .events = std.posix.POLL.IN, .revents = 0 },
    };
    _ = try std.posix.poll(&descriptors, 0);
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try std.testing.expect(try bridgeStep(&descriptors, output.descriptors[0], &buffer));
    try output.expectOutput(response[0..bridge_buffer_bytes]);
    var remaining: [2]u8 = undefined;
    const length = std.c.recv(peer.descriptors[0], &remaining, remaining.len, std.posix.MSG.DONTWAIT);
    try std.testing.expectEqual(@as(isize, 1), length);
    try std.testing.expectEqual(@as(u8, 'l'), remaining[0]);
}

test "a partial-read overcount is a finite cap that may include newly arriving bytes" {
    var peer = try TestSocketPair.init();
    defer peer.deinit();
    var output = try TestSocketPair.init();
    defer output.deinit();
    const original = "o" ** (bridge_buffer_bytes + 1);
    try sendAll(peer.descriptors[1], original);
    var buffer: [bridge_buffer_bytes]u8 = undefined;
    try std.testing.expectEqual(bridge_buffer_bytes, try std.posix.read(peer.descriptors[0], &buffer));
    const limit = queuedReadLimit(peer.descriptors[0]);
    // Kernels may report either the unread byte or the partially consumed queued buffer.
    try std.testing.expect(limit >= 1 and limit <= original.len);
    const arriving = "n" ** (bridge_buffer_bytes + 11);
    try sendAll(peer.descriptors[1], arriving[0 .. limit + 10]);
    try drainQueuedPrefix(peer.descriptors[0], output.descriptors[0], limit, &buffer);
    var expected: [bridge_buffer_bytes + 1]u8 = @splat('n');
    expected[0] = 'o';
    try output.expectOutput(expected[0..limit]);
    var remainder: [12]u8 = undefined;
    const length = std.c.recv(peer.descriptors[0], &remainder, remainder.len, std.posix.MSG.DONTWAIT);
    try std.testing.expectEqual(@as(isize, 11), length);
    try std.testing.expectEqualStrings("n" ** 11, remainder[0..11]);
}

const ScriptedSender = struct {
    attempts: []const SendAttempt,
    calls: usize = 0,
    offered: [4]usize = @splat(0),
    sent: [8]u8 = undefined,
    sent_count: usize = 0,

    fn send(self: *ScriptedSender, bytes: []const u8) SendAttempt {
        const attempt = self.attempts[self.calls];
        self.offered[self.calls] = bytes.len;
        self.calls += 1;
        if (attempt.count > 0) {
            const count: usize = @intCast(attempt.count);
            @memcpy(self.sent[self.sent_count..][0..count], bytes[0..count]);
            self.sent_count += count;
        }
        return attempt;
    }
};

test "interrupted sends retry only the unsent suffix" {
    var sender = ScriptedSender{ .attempts = &.{
        .{ .count = 2, .errno = .SUCCESS },
        .{ .count = -1, .errno = .INTR },
        .{ .count = 3, .errno = .SUCCESS },
    } };
    try sendAllUsing(&sender, "hello");
    try std.testing.expectEqualSlices(usize, &.{ 5, 3, 3 }, sender.offered[0..sender.calls]);
    try std.testing.expectEqualStrings("hello", sender.sent[0..sender.sent_count]);
}

test "terminal and other send errors remain distinct failures" {
    for ([_]std.posix.E{ .PIPE, .CONNRESET, .NOTCONN }) |errno| {
        var sender = ScriptedSender{ .attempts = &.{.{ .count = -1, .errno = errno }} };
        try std.testing.expectError(error.ConnectionClosed, sendAllUsing(&sender, "hello"));
        try std.testing.expectEqual(@as(usize, 1), sender.calls);
    }
    var invalid = ScriptedSender{ .attempts = &.{.{ .count = -1, .errno = .BADF }} };
    try std.testing.expectError(error.InvalidSocket, sendAllUsing(&invalid, "hello"));
    try std.testing.expectEqual(@as(usize, 1), invalid.calls);
    var zero = ScriptedSender{ .attempts = &.{.{ .count = 0, .errno = .SUCCESS }} };
    try std.testing.expectError(error.ConnectionClosed, sendAllUsing(&zero, "hello"));
}
