const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const connection = @import("connection.zig");

pub const endpoint_security = @import("posix_endpoint_security.zig");

pub const send_flags = if (builtin.os.tag == .windows)
    0
else
    std.posix.MSG.DONTWAIT | std.posix.MSG.NOSIGNAL;

const Connection = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

    pub fn read(self: *Connection, bytes: []u8) !usize {
        return self.stream.read(bytes);
    }

    pub fn writeAll(self: *Connection, bytes: []const u8) !void {
        var timer = std.time.Timer.start() catch return error.ConnectionClosed;
        var deadline = connection.Deadline.init(&timer, connection.record_read_timeout_ns) catch return error.ConnectionClosed;
        self.writeAllBefore(bytes, &deadline) catch return error.ConnectionClosed;
    }

    pub fn writeAllBefore(self: *Connection, bytes: []const u8, deadline: *connection.Deadline) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining_ns = deadline.remainingNs();
            if (remaining_ns <= 0) return error.WriteTimeout;
            const timeout_ms: i32 = @intCast(@min(
                std.math.divCeil(u64, remaining_ns, @as(u64, std.time.ns_per_ms)) catch unreachable,
                @as(u64, connection.poll_interval_ms),
            ));
            var descriptors = [_]std.posix.pollfd{.{
                .fd = self.stream.handle,
                .events = std.posix.POLL.OUT,
                .revents = 0,
            }};
            if (try std.posix.poll(&descriptors, timeout_ms) == 0) continue;
            const result = descriptors[0].revents;
            if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
                return error.ConnectionClosed;
            }
            const written = std.posix.send(
                self.stream.handle,
                bytes[offset..],
                send_flags,
            ) catch |err| switch (err) {
                error.WouldBlock => continue,
                else => return error.ConnectionClosed,
            };
            if (written == 0) return error.ConnectionClosed;
            offset += written;
        }
    }

    pub fn waitReadable(self: *Connection) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.stream.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, connection.poll_interval_ms) == 0) return false;
        const result = descriptors[0].revents;
        if (result & std.posix.POLL.IN != 0) return true;
        if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ConnectionClosed;
        }
        return false;
    }
};

pub const Server = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    listener: std.net.Server,
    lock_file: std.fs.File,
    lock_name: []u8,
    socket_name: []u8,
    socket_path: []u8,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!connection.validEndpointId(endpoint_id)) return error.InvalidEndpointId;

        var directory = try endpoint_security.prepareRuntimeDirectory(runtime_directory);
        errdefer directory.close();

        const lock_name = try endpoint_security.lockName(allocator, endpoint_id);
        errdefer allocator.free(lock_name);
        var lock_file = try endpoint_security.acquireLock(
            directory,
            lock_name,
            std.posix.geteuid(),
        );
        errdefer lock_file.close();

        const socket_name = try endpoint_security.endpointName(allocator, endpoint_id);
        errdefer allocator.free(socket_name);
        const socket_path = try std.fs.path.join(
            allocator,
            &.{ runtime_directory, socket_name },
        );
        errdefer allocator.free(socket_path);
        directory.deleteFile(socket_name) catch |err| switch (err) {
            error.FileNotFound => {},
            else => return err,
        };
        errdefer directory.deleteFile(socket_name) catch {};

        var original_directory = try std.fs.cwd().openDir(".", .{});
        defer original_directory.close();
        try std.posix.fchdir(directory.fd);
        var restored_directory = false;
        defer if (!restored_directory) std.posix.fchdir(original_directory.fd) catch {};
        const address = std.net.Address.initUnix(socket_name) catch return error.EndpointPathTooLong;
        var listener = try address.listen(.{ .kernel_backlog = 8 });
        errdefer listener.deinit();
        try endpoint_security.secureEndpoint(
            directory,
            socket_name,
            std.posix.geteuid(),
        );
        try std.posix.fchdir(original_directory.fd);
        restored_directory = true;

        return .{
            .allocator = allocator,
            .directory = directory,
            .listener = listener,
            .lock_file = lock_file,
            .lock_name = lock_name,
            .socket_name = socket_name,
            .socket_path = socket_path,
        };
    }

    pub fn deinit(self: *Server) void {
        self.listener.deinit();
        self.directory.deleteFile(self.socket_name) catch {};
        self.lock_file.close();
        self.directory.close();
        self.allocator.free(self.socket_path);
        self.allocator.free(self.socket_name);
        self.allocator.free(self.lock_name);
        self.* = undefined;
    }

    pub fn endpointPath(self: *const Server) []const u8 {
        return self.socket_path;
    }

    pub fn serveUntilIdle(self: *Server, service: *host_service.HostService) !void {
        return connection.serveUntilIdleLoop(self, service, struct {
            fn recoverable(err: anyerror) bool {
                return err == error.PeerIdentityMismatch;
            }
        }.recoverable);
    }

    pub fn serveOne(self: *Server, service: *host_service.HostService) !void {
        try self.acceptAndServe(service);
    }

    pub fn serveOneBefore(
        self: *Server,
        service: *host_service.HostService,
        timeout_ms: u32,
    ) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.listener.stream.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, @intCast(timeout_ms)) == 0) return false;
        const result = descriptors[0].revents;
        if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ConnectionClosed;
        }
        if (result & std.posix.POLL.IN == 0) return false;
        try self.acceptAndServe(service);
        return true;
    }

    fn acceptAndServe(self: *Server, service: *host_service.HostService) !void {
        var accepted = try self.listener.accept();
        defer accepted.stream.close();
        try endpoint_security.requirePeerEffectiveUid(
            accepted.stream.handle,
            std.posix.geteuid(),
        );
        var transport: Connection = .{ .stream = accepted.stream };
        connection.serveAcceptedConnection(&transport, service);
    }
};
