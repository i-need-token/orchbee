//! The macOS and Linux Keeper's endpoint: a Host's Unix socket, with its private folder, same-user
//! check, and lifetime lock, from which the listener accepts each connection as a socket that
//! carries descriptors.

const std = @import("std");
const platform = @import("terminal-host-platform");
const local_ipc = @import("terminal-host-transport");

const posix_compat = platform.posix_compat;
const posix = std.posix;
const Stream = platform.keeper.native.Stream;

pub const Server = struct {
    inner: local_ipc.LocalServer,
    accepted: ?posix.fd_t = null,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        return .{ .inner = try local_ipc.LocalServer.initInEndpointDirectory(
            platform.thread_sync.io(),
            allocator,
            runtime_directory,
            endpoint_directory,
            endpoint_id,
        ) };
    }

    /// The endpoint an earlier image of this Keeper left open across `execve`.
    pub fn adopt(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_directory: []const u8,
        endpoint_id: []const u8,
        listener_socket: posix.fd_t,
        lock: posix.fd_t,
    ) !Server {
        return .{ .inner = try local_ipc.LocalServer.adoptInEndpointDirectory(
            platform.thread_sync.io(),
            allocator,
            runtime_directory,
            endpoint_directory,
            endpoint_id,
            listener_socket,
            lock,
        ) };
    }

    /// The listening socket and the lifetime lock, which an update carries to the next image.
    pub fn endpointDescriptors(self: *const Server) struct { listener: posix.fd_t, lock: posix.fd_t } {
        return .{ .listener = self.inner.listenerSocket(), .lock = self.inner.lock_file.handle };
    }

    /// Accepts one same-user connection within `timeout_ms`; another user's is closed unread.
    pub fn acceptBefore(self: *Server, timeout_ms: u32) !bool {
        var descriptors = [_]posix.pollfd{.{ .fd = self.inner.listenerSocket(), .events = posix.POLL.IN, .revents = 0 }};
        if (try posix.poll(&descriptors, @intCast(timeout_ms)) == 0) return false;
        if (descriptors[0].revents & (posix.POLL.ERR | posix.POLL.HUP | posix.POLL.NVAL) != 0) return error.ConnectionClosed;
        const socket = self.inner.acceptSocket() catch |err| switch (err) {
            error.PeerIdentityMismatch => return false,
            else => return err,
        };
        _ = posix_compat.fcntl(socket, posix.F.SETFD, posix.FD_CLOEXEC) catch {};
        self.accepted = socket;
        return true;
    }

    pub fn takeConnection(self: *Server) !Stream {
        const socket = self.accepted orelse return error.ConnectionClosed;
        self.accepted = null;
        return Stream.init(socket);
    }

    pub fn disconnect(self: *Server) void {
        if (self.accepted) |socket| posix_compat.close(socket);
        self.accepted = null;
    }

    pub fn lastSystemCode(_: *const Server) ?u32 {
        return null;
    }

    pub fn renewAfterAcceptFailure(_: *Server) ?u32 {
        return null;
    }
};
