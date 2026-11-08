const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const monotonic = @import("terminal-host-platform").monotonic;
const output_wake = @import("terminal-host-platform").output_wake;
const posix_compat = @import("terminal-host-platform").posix_compat;
const thread_sync = @import("terminal-host-platform").thread_sync;
const connection = @import("connection.zig");
const control_lanes = @import("control_lanes.zig");

pub const endpoint_security = @import("posix_endpoint_security.zig");

pub const send_flags = if (builtin.os.tag == .windows)
    0
else
    std.posix.MSG.DONTWAIT | std.posix.MSG.NOSIGNAL;

const Connection = if (builtin.os.tag == .windows) void else struct {
    io: std.Io,
    stream: std.Io.net.Stream,
    /// Only the dispatch thread waits on the process-wide output wake; a control lane's
    /// connection waits on its socket alone.
    observe_output_wake: bool = true,

    pub fn read(self: *Connection, bytes: []u8) !usize {
        return std.posix.read(self.stream.socket.handle, bytes);
    }

    pub fn writeAll(self: *Connection, bytes: []const u8) !void {
        var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
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
                .fd = self.stream.socket.handle,
                .events = std.posix.POLL.OUT,
                .revents = 0,
            }};
            if (try std.posix.poll(&descriptors, timeout_ms) == 0) continue;
            const result = descriptors[0].revents;
            if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
                return error.ConnectionClosed;
            }
            const written = posix_compat.send(
                self.stream.socket.handle,
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
        var descriptors = [_]std.posix.pollfd{
            .{
                .fd = self.stream.socket.handle,
                .events = std.posix.POLL.IN,
                .revents = 0,
            },
            .{
                .fd = output_wake.readDescriptor(),
                .events = std.posix.POLL.IN,
                .revents = 0,
            },
        };
        const observed = if (self.observe_output_wake) descriptors[0..] else descriptors[0..1];
        const timeout_ms = if (self.observe_output_wake)
            output_wake.pollTimeoutMs(connection.poll_interval_ms)
        else
            connection.poll_interval_ms;
        if (try std.posix.poll(observed, timeout_ms) == 0) {
            return false;
        }
        if (self.observe_output_wake and descriptors[1].revents != 0) output_wake.drain();
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
    directory: std.Io.Dir,
    io: std.Io,
    listener: std.Io.net.Server,
    lock_file: std.Io.File,
    lock_name: []u8,
    socket_name: []u8,
    socket_path: []u8,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        return initWithIo(thread_sync.io(), allocator, runtime_directory, endpoint_id);
    }

    pub fn initWithIo(
        io: std.Io,
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!connection.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
        output_wake.prepare();

        const directory = try endpoint_security.prepareRuntimeDirectory(io, runtime_directory);
        errdefer directory.close(io);

        const lock_name = try endpoint_security.lockName(allocator, endpoint_id);
        errdefer allocator.free(lock_name);
        var lock_file = try endpoint_security.acquireLock(
            io,
            directory,
            lock_name,
            std.c.geteuid(),
        );
        errdefer lock_file.close(io);

        const socket_name = try endpoint_security.endpointName(allocator, endpoint_id);
        errdefer allocator.free(socket_name);
        const socket_path = try std.fs.path.join(
            allocator,
            &.{ runtime_directory, socket_name },
        );
        errdefer allocator.free(socket_path);
        directory.deleteFile(io, socket_name) catch |err| switch (err) {
            error.FileNotFound => {},
            else => return err,
        };
        errdefer directory.deleteFile(io, socket_name) catch {};

        const original_directory = try std.Io.Dir.cwd().openDir(io, ".", .{});
        defer original_directory.close(io);
        try posix_compat.fchdir(directory.handle);
        var restored_directory = false;
        defer if (!restored_directory) posix_compat.fchdir(original_directory.handle) catch {};
        const address = std.Io.net.UnixAddress.init(socket_name) catch
            return error.EndpointPathTooLong;
        var listener = try address.listen(io, .{ .kernel_backlog = 8 });
        errdefer listener.deinit(io);
        try endpoint_security.secureEndpoint(
            directory,
            socket_name,
            std.c.geteuid(),
        );
        try posix_compat.fchdir(original_directory.handle);
        restored_directory = true;

        return .{
            .allocator = allocator,
            .directory = directory,
            .io = io,
            .listener = listener,
            .lock_file = lock_file,
            .lock_name = lock_name,
            .socket_name = socket_name,
            .socket_path = socket_path,
        };
    }

    pub fn deinit(self: *Server) void {
        self.listener.deinit(self.io);
        self.directory.deleteFile(self.io, self.socket_name) catch {};
        self.lock_file.close(self.io);
        self.directory.close(self.io);
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

    pub fn serveOne(self: *Server, service: *host_service.HostService) !connection.Served {
        return self.acceptAndServe(service);
    }

    pub fn serveOneBefore(
        self: *Server,
        service: *host_service.HostService,
        timeout_ms: u32,
    ) !bool {
        var descriptors = [_]std.posix.pollfd{.{
            .fd = self.listener.socket.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        }};
        if (try std.posix.poll(&descriptors, @intCast(timeout_ms)) == 0) return false;
        const result = descriptors[0].revents;
        if (result & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            return error.ConnectionClosed;
        }
        if (result & std.posix.POLL.IN == 0) return false;
        _ = try self.acceptAndServe(service);
        return true;
    }

    fn acceptAndServe(self: *Server, service: *host_service.HostService) !connection.Served {
        const accepted = try self.listener.accept(self.io);
        defer accepted.close(self.io);
        try endpoint_security.requirePeerEffectiveUid(
            accepted.socket.handle,
            std.c.geteuid(),
        );
        var transport: Connection = .{ .io = self.io, .stream = accepted };
        var lanes = Lanes.init(.{ .server = self }, service.allocator);
        return connection.serveAcceptedConnection(&transport, service, &lanes);
    }
};

const Lanes = control_lanes.ControlLanes(LaneEndpoint);

/// The control lanes share the endpoint's listener; one lane at a time polls and accepts, so an
/// accept never blocks a lane that lost the race for a connection. A lane rechecks its stop flag
/// after every poll, so stopping needs no wake.
const LaneEndpoint = if (builtin.os.tag == .windows) void else struct {
    server: *Server,
    accept_lock: thread_sync.Mutex = .{},

    pub fn armStop(_: *LaneEndpoint) void {}

    pub fn signalStop(_: *LaneEndpoint) void {}

    pub const Lane = struct {
        endpoint: *LaneEndpoint,
        accepted: ?Connection = null,

        pub fn open(endpoint: *LaneEndpoint) !Lane {
            return .{ .endpoint = endpoint };
        }

        pub fn close(_: *Lane) void {}

        /// Whether a same-user connection is ready within one poll; another user's connection is
        /// closed. The peer credential check needs no byte, and the candidate's first frame keeps
        /// its own deadline.
        pub fn accept(self: *Lane) !bool {
            const timeout_ms: u32 = control_lanes.accept_poll_ms;
            const io = thread_sync.io();
            const stream = stream: {
                // A lane that finds another lane polling waits once and rechecks its stop flag.
                if (!self.endpoint.accept_lock.tryLock()) {
                    monotonic.sleep(@as(u64, timeout_ms) * std.time.ns_per_ms);
                    return false;
                }
                defer self.endpoint.accept_lock.unlock();
                var descriptors = [_]std.posix.pollfd{.{
                    .fd = self.endpoint.server.listener.socket.handle,
                    .events = std.posix.POLL.IN,
                    .revents = 0,
                }};
                if (try std.posix.poll(&descriptors, @intCast(timeout_ms)) == 0) return false;
                if (descriptors[0].revents & std.posix.POLL.IN == 0) return false;
                break :stream try self.endpoint.server.listener.accept(io);
            };
            endpoint_security.requirePeerEffectiveUid(stream.socket.handle, std.c.geteuid()) catch {
                stream.close(io);
                return false;
            };
            self.accepted = .{ .io = io, .stream = stream, .observe_output_wake = false };
            return true;
        }

        pub fn transport(self: *Lane) *Connection {
            return &self.accepted.?;
        }

        pub fn finish(self: *Lane) void {
            const accepted = self.accepted orelse return;
            accepted.stream.close(accepted.io);
            self.accepted = null;
        }
    };
};
