const std = @import("std");

const private_ipc = @import("ssh-runtime-private-ipc");

const bridge_buffer_bytes = 8192;
const connect_attempts = 100;
const connect_retry_ns = 50 * std.time.ns_per_ms;

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

        if (descriptors[1].revents & std.posix.POLL.IN != 0) {
            const length = try std.posix.read(stream.socket.handle, &buffer);
            if (length == 0) return;
            try writeAll(stdout_descriptor, buffer[0..length]);
        }
        if (descriptors[1].revents & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return;
        }

        if (descriptors[0].revents & std.posix.POLL.IN != 0) {
            const length = try std.posix.read(stdin_descriptor, &buffer);
            if (length == 0) return;
            try sendAll(stream.socket.handle, buffer[0..length]);
        }
        if (descriptors[0].revents & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return;
        }
    }
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
    var offset: usize = 0;
    while (offset < bytes.len) {
        const sent = std.c.send(
            socket,
            bytes[offset..].ptr,
            bytes.len - offset,
            std.posix.MSG.NOSIGNAL,
        );
        if (sent <= 0) return error.ConnectionClosed;
        offset += @intCast(sent);
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
