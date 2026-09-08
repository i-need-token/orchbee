const std = @import("std");
const attachment = @import("ssh-runtime-attachment").testing;
const bridge_buffer_bytes = attachment.bridge_buffer_bytes;
const bridgeStep = attachment.bridgeStep;
const sendAll = attachment.sendAll;
const sendAllUsing = attachment.sendAllUsing;
const SendAttempt = attachment.SendAttempt;
const queuedReadLimit = attachment.queuedReadLimit;
const drainQueuedOutput = attachment.drainQueuedOutput;
const drainQueuedPrefix = attachment.drainQueuedPrefix;

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

    pub fn send(self: *ScriptedSender, bytes: []const u8) SendAttempt {
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
