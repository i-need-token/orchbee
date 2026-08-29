const std = @import("std");

const private_ipc = @import("ssh-runtime-private-ipc");

pub const maximum_chunk_bytes = 65_536;
const connect_attempts = 100;
const connect_retry_ns = 50 * std.time.ns_per_ms;

const SetupStatus = enum(u8) {
    ready = 0,
    unavailable = 1,
};

pub const ReadResult = union(enum) {
    pending,
    ready,
    data: usize,
    unavailable,
    closed,
};

pub const Worker = struct {
    attachment_generation: u64,
    channel_id: u32,
    child: std.process.Child,
    setup_complete: bool = false,

    pub fn spawn(
        io: std.Io,
        executable_path: []const u8,
        attachment_generation: u64,
        channel_id: u32,
        terminal_host_executable_path: []const u8,
        terminal_host_runtime_directory: []const u8,
        terminal_host_endpoint_id: []const u8,
    ) !Worker {
        var child = try std.process.spawn(io, .{
            .argv = &.{
                executable_path,
                "terminal-host-relay-worker",
                "--terminal-host-executable",
                terminal_host_executable_path,
                "--terminal-host-runtime-directory",
                terminal_host_runtime_directory,
                "--terminal-host-endpoint-id",
                terminal_host_endpoint_id,
            },
            .stdin = .pipe,
            .stdout = .pipe,
            .stderr = .ignore,
        });
        errdefer child.kill(io);
        const stdin = child.stdin orelse return error.WorkerPipeUnavailable;
        const stdout = child.stdout orelse return error.WorkerPipeUnavailable;
        try setNonblocking(stdin.handle);
        try setNonblocking(stdout.handle);
        return .{
            .attachment_generation = attachment_generation,
            .channel_id = channel_id,
            .child = child,
        };
    }

    pub fn inputDescriptor(self: *const Worker) std.posix.fd_t {
        return self.child.stdin.?.handle;
    }

    pub fn outputDescriptor(self: *const Worker) std.posix.fd_t {
        return self.child.stdout.?.handle;
    }

    pub fn readAvailable(self: *Worker, destination: []u8) !ReadResult {
        if (destination.len == 0) return .pending;
        const stdout = self.child.stdout orelse return error.WorkerPipeUnavailable;
        const length = std.posix.read(stdout.handle, destination) catch |err| switch (err) {
            error.WouldBlock => return .pending,
            else => return err,
        };
        if (length == 0) return .closed;
        if (self.setup_complete) return .{ .data = length };
        const status = std.enums.fromInt(SetupStatus, destination[0]) orelse
            return error.InvalidWorkerStatus;
        if (status == .unavailable) return .unavailable;
        self.setup_complete = true;
        if (length == 1) return .ready;
        std.mem.copyForwards(u8, destination[0 .. length - 1], destination[1..length]);
        return .{ .data = length - 1 };
    }

    pub fn writeAvailable(self: *Worker, bytes: []const u8) !usize {
        if (bytes.len == 0) return 0;
        const stdin = self.child.stdin orelse return error.WorkerPipeUnavailable;
        while (true) {
            const written = std.posix.system.write(stdin.handle, bytes.ptr, bytes.len);
            switch (std.posix.errno(written)) {
                .SUCCESS => return @intCast(written),
                .INTR => continue,
                .AGAIN => return 0,
                .PIPE => return error.ConnectionClosed,
                else => |err| return std.posix.unexpectedErrno(err),
            }
        }
    }

    pub fn terminate(self: *Worker, io: std.Io) void {
        self.child.kill(io);
    }
};

pub fn runWorker(
    io: std.Io,
    allocator: std.mem.Allocator,
    terminal_host_executable_path: []const u8,
    terminal_host_runtime_directory: []const u8,
    terminal_host_endpoint_id: []const u8,
) !void {
    var stream = connectOrLaunch(
        io,
        allocator,
        terminal_host_executable_path,
        terminal_host_runtime_directory,
        terminal_host_endpoint_id,
    ) catch {
        try writeStatus(.unavailable);
        return;
    };
    defer stream.close(io);
    try writeStatus(.ready);
    try bridge(io, stream.socket.handle);
}

fn connectOrLaunch(
    io: std.Io,
    allocator: std.mem.Allocator,
    terminal_host_executable_path: []const u8,
    terminal_host_runtime_directory: []const u8,
    terminal_host_endpoint_id: []const u8,
) !std.Io.net.Stream {
    try private_ipc.validateRuntimeDirectory(terminal_host_runtime_directory);
    var directory = try std.Io.Dir.openDirAbsolute(io, terminal_host_runtime_directory, .{
        .follow_symlinks = false,
    });
    defer directory.close(io);
    if (std.c.fchdir(directory.handle) != 0) return error.RuntimeDirectoryUnavailable;

    const endpoint_name = try std.fmt.allocPrint(
        allocator,
        "{s}.s",
        .{terminal_host_endpoint_id},
    );
    defer allocator.free(endpoint_name);
    if (connectExisting(io, endpoint_name)) |stream| return stream else |err| switch (err) {
        error.FileNotFound => {},
        else => return err,
    }

    var child = try std.process.spawn(io, .{
        .argv = &.{
            terminal_host_executable_path,
            "serve",
            "--runtime-directory",
            terminal_host_runtime_directory,
            "--endpoint-id",
            terminal_host_endpoint_id,
        },
        .stdin = .ignore,
        .stdout = .ignore,
        .stderr = .ignore,
    });
    _ = &child;

    for (0..connect_attempts) |_| {
        if (connectExisting(io, endpoint_name)) |stream| return stream else |_| {}
        std.Io.sleep(io, .fromNanoseconds(connect_retry_ns), .awake) catch {};
    }
    return error.TerminalHostUnavailable;
}

fn connectExisting(io: std.Io, endpoint_name: []const u8) !std.Io.net.Stream {
    try private_ipc.validateEndpoint(endpoint_name);
    const address = std.Io.net.UnixAddress.init(endpoint_name) catch
        return error.EndpointPathTooLong;
    const stream = try address.connect(io);
    errdefer stream.close(io);
    try private_ipc.requirePeerEffectiveUid(stream.socket.handle);
    return stream;
}

fn bridge(io: std.Io, host_socket: std.posix.socket_t) !void {
    _ = io;
    const stdin_descriptor = std.Io.File.stdin().handle;
    const stdout_descriptor = std.Io.File.stdout().handle;
    var buffer: [maximum_chunk_bytes]u8 = undefined;
    while (true) {
        var descriptors = [_]std.posix.pollfd{
            .{ .fd = stdin_descriptor, .events = std.posix.POLL.IN, .revents = 0 },
            .{ .fd = host_socket, .events = std.posix.POLL.IN, .revents = 0 },
        };
        _ = try std.posix.poll(&descriptors, -1);

        if (descriptors[1].revents & std.posix.POLL.IN != 0) {
            const length = try std.posix.read(host_socket, &buffer);
            if (length == 0) return;
            try writeAll(stdout_descriptor, buffer[0..length]);
        }
        if (descriptors[1].revents &
            (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0)
        {
            return;
        }
        if (descriptors[0].revents & std.posix.POLL.IN != 0) {
            const length = try std.posix.read(stdin_descriptor, &buffer);
            if (length == 0) return;
            try sendAll(host_socket, buffer[0..length]);
        }
        if (descriptors[0].revents &
            (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0)
        {
            return;
        }
    }
}

fn writeStatus(status: SetupStatus) !void {
    try writeAll(std.Io.File.stdout().handle, &.{@intFromEnum(status)});
}

fn setNonblocking(descriptor: std.posix.fd_t) !void {
    const flags = std.posix.system.fcntl(descriptor, std.posix.F.GETFL);
    if (flags == -1) return error.NonblockingUnavailable;
    switch (std.posix.errno(std.posix.system.fcntl(
        descriptor,
        std.posix.F.SETFL,
        @as(u32, @intCast(flags)) | @as(u32, @bitCast(std.posix.O{ .NONBLOCK = true })),
    ))) {
        .SUCCESS => {},
        else => |err| return std.posix.unexpectedErrno(err),
    }
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
