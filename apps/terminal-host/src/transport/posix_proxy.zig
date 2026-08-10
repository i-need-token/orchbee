const std = @import("std");

pub const buffer_bytes = 64 * 1024;
const send_flags = std.posix.MSG.DONTWAIT | std.posix.MSG.NOSIGNAL;

const FixedBuffer = struct {
    bytes: [buffer_bytes]u8 = undefined,
    end: usize = 0,
    start: usize = 0,

    fn empty(self: *const FixedBuffer) bool {
        return self.start == self.end;
    }

    fn readable(self: *FixedBuffer) []u8 {
        return self.bytes[self.start..self.end];
    }

    fn writable(self: *FixedBuffer) []u8 {
        if (self.end == self.bytes.len and self.start > 0) {
            std.mem.copyForwards(u8, self.bytes[0 .. self.end - self.start], self.readable());
            self.end -= self.start;
            self.start = 0;
        }
        return self.bytes[self.end..];
    }

    fn commit(self: *FixedBuffer, length: usize) !void {
        if (length > self.bytes.len - self.end) return error.ProxyBufferOverflow;
        self.end += length;
    }

    fn consume(self: *FixedBuffer, length: usize) !void {
        if (length > self.end - self.start) return error.ProxyBufferUnderflow;
        self.start += length;
        if (self.start == self.end) {
            self.start = 0;
            self.end = 0;
        }
    }
};

pub fn run(socket: std.posix.socket_t, input: std.posix.fd_t, output: std.posix.fd_t) !void {
    try setNonBlocking(socket);
    try setNonBlocking(input);
    try setNonBlocking(output);

    var to_socket: FixedBuffer = .{};
    var to_output: FixedBuffer = .{};
    var input_open = true;
    var socket_read_open = true;
    var socket_write_open = true;

    while (socket_read_open or !to_output.empty()) {
        if (!input_open and socket_write_open and to_socket.empty()) {
            std.posix.shutdown(socket, .send) catch |failure| switch (failure) {
                error.SocketNotConnected, error.ConnectionResetByPeer => {},
                else => return failure,
            };
            socket_write_open = false;
        }

        const socket_read_events: i16 = if (socket_read_open and to_output.writable().len > 0)
            std.posix.POLL.IN
        else
            0;
        const socket_write_events: i16 = if (socket_write_open and !to_socket.empty())
            std.posix.POLL.OUT
        else
            0;
        var descriptors = [_]std.posix.pollfd{
            .{
                .fd = input,
                .events = if (input_open and to_socket.writable().len > 0) std.posix.POLL.IN else 0,
                .revents = 0,
            },
            .{
                .fd = socket,
                .events = socket_read_events | socket_write_events,
                .revents = 0,
            },
            .{
                .fd = output,
                .events = if (!to_output.empty()) std.posix.POLL.OUT else 0,
                .revents = 0,
            },
        };
        _ = try std.posix.poll(&descriptors, -1);

        if (descriptors[2].revents & std.posix.POLL.OUT != 0) {
            const written = std.posix.write(output, to_output.readable()) catch |failure| switch (failure) {
                error.WouldBlock => continue,
                else => return error.ProxyOutputClosed,
            };
            try to_output.consume(written);
        }
        if (descriptors[2].revents & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ProxyOutputClosed;
        }

        if (descriptors[1].revents & std.posix.POLL.OUT != 0) {
            const written = std.posix.send(socket, to_socket.readable(), send_flags) catch |failure| switch (failure) {
                error.WouldBlock => continue,
                error.BrokenPipe, error.ConnectionResetByPeer => return error.ConnectionClosed,
                else => return failure,
            };
            if (written == 0 and !to_socket.empty()) return error.ConnectionClosed;
            try to_socket.consume(written);
        }
        if (descriptors[1].revents & std.posix.POLL.IN != 0) {
            const target = to_output.writable();
            const length = std.posix.recv(socket, target, std.posix.MSG.DONTWAIT) catch |failure| switch (failure) {
                error.WouldBlock => continue,
                error.ConnectionResetByPeer => 0,
                else => return failure,
            };
            if (length == 0) {
                socket_read_open = false;
                input_open = false;
            } else {
                try to_output.commit(length);
            }
        }
        if (descriptors[1].revents & (std.posix.POLL.ERR | std.posix.POLL.NVAL) != 0) {
            socket_read_open = false;
            input_open = false;
        }
        if (descriptors[1].revents & std.posix.POLL.HUP != 0 and
            descriptors[1].revents & std.posix.POLL.IN == 0)
        {
            socket_read_open = false;
            input_open = false;
        }

        if (descriptors[0].revents & std.posix.POLL.IN != 0) {
            const target = to_socket.writable();
            const length = std.posix.read(input, target) catch |failure| switch (failure) {
                error.WouldBlock => continue,
                else => return failure,
            };
            if (length == 0) {
                input_open = false;
            } else {
                try to_socket.commit(length);
            }
        }
        if (descriptors[0].revents & (std.posix.POLL.ERR | std.posix.POLL.NVAL) != 0) {
            input_open = false;
        }
        if (descriptors[0].revents & std.posix.POLL.HUP != 0 and
            descriptors[0].revents & std.posix.POLL.IN == 0)
        {
            input_open = false;
        }
    }
}

fn setNonBlocking(descriptor: std.posix.fd_t) !void {
    const flags = try std.posix.fcntl(descriptor, std.posix.F.GETFL, 0);
    const nonblocking = @as(usize, 1) << @bitOffsetOf(std.posix.O, "NONBLOCK");
    _ = try std.posix.fcntl(descriptor, std.posix.F.SETFL, flags | nonblocking);
}
