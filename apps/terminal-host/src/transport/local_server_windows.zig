const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const monotonic = @import("terminal-host-platform").monotonic;
const output_wake = @import("terminal-host-platform").output_wake;
const connection = @import("connection.zig");
const control_lanes = @import("control_lanes.zig");
const serve_loop = @import("serve_loop.zig");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

pub const Server = if (builtin.os.tag != .windows) void else struct {
    handle: *windows_c.ObWindowsLocalServer,
    /// Only the dispatch thread waits on the process-wide output wake; a control lane's instance
    /// waits on its pipe alone.
    observe_output_wake: bool = true,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        return initWithIo(undefined, allocator, runtime_directory, endpoint_id);
    }

    pub fn initWithIo(
        _: std.Io,
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
    ) !Server {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!connection.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
        output_wake.prepare();
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, runtime_directory);
        defer allocator.free(directory_w);
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_server_create(
            directory_w.ptr,
            endpoint_w.ptr,
            &status,
        ) orelse return statusError(status);
        return .{ .handle = handle };
    }

    /// The endpoint with every protection of `init` and another pipe-instance limit, from 1 to
    /// 254, or 255 for no system limit, for a listener that bounds its own instances.
    pub fn initWithInstanceLimit(
        allocator: std.mem.Allocator,
        runtime_directory: []const u8,
        endpoint_id: []const u8,
        instance_limit: u8,
    ) !Server {
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        if (!connection.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
        const directory_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, runtime_directory);
        defer allocator.free(directory_w);
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_server_create_limited(
            directory_w.ptr,
            endpoint_w.ptr,
            instance_limit,
            &status,
        ) orelse return statusError(status);
        return .{ .handle = handle, .observe_output_wake = false };
    }

    pub fn deinit(self: *Server) void {
        windows_c.ob_windows_local_server_destroy(self.handle);
        self.* = undefined;
    }

    /// Waits at most `timeout_ms` for the next client: false after a timeout or for a client that
    /// left before its accept. The listen stays armed across a timeout.
    pub fn acceptBefore(self: *Server, timeout_ms: u32) !bool {
        const status = windows_c.ob_windows_local_server_accept_before(self.handle, timeout_ms);
        return switch (status) {
            windows_c.OB_WINDOWS_IPC_OK => true,
            windows_c.OB_WINDOWS_IPC_ACCEPT_TIMEOUT, windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED => false,
            else => statusError(status),
        };
    }

    /// Moves the client just accepted, before any byte was read or written, to its own instance,
    /// which the caller owns and may serve and close on any thread; this instance then waits for
    /// the next client. On failure the client stays here.
    pub fn takeConnection(self: *Server) !Server {
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_server_take_connection(
            self.handle,
            &status,
        ) orelse return statusError(status);
        return .{ .handle = handle, .observe_output_wake = false };
    }

    /// The connected client's process id, taken from the pipe before any byte is read.
    pub fn clientProcessId(self: *const Server) ?u32 {
        const process_id = windows_c.ob_windows_local_server_client_process_id(self.handle);
        return if (process_id == 0) null else process_id;
    }

    /// Ends the accepted connection; an armed listen stays armed.
    pub fn disconnect(self: *Server) void {
        windows_c.ob_windows_local_server_disconnect(self.handle);
    }

    /// Whether a byte became readable within `timeout_ms`; false also once `wake`, a
    /// self-resetting event, is signalled. The peer's identity is checked at its first byte.
    pub fn waitReadableOrWake(self: *Server, timeout_ms: u32, wake: ?std.os.windows.HANDLE) !bool {
        var readable: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_wait_readable_or_wake(
            self.handle,
            timeout_ms,
            wake,
            null,
            &readable,
        ));
        return readable != 0;
    }

    /// How many bytes the peer has sent that are not read yet.
    pub fn pendingBytes(self: *Server) !u32 {
        var available: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_peek(self.handle, &available));
        return available;
    }

    /// Writes all of `bytes` within `timeout_ns`, or fails with `WriteTimeout`.
    pub fn writeAllWithin(self: *Server, bytes: []const u8, timeout_ns: u64) !void {
        var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
        var deadline = connection.Deadline.init(&timer, timeout_ns) catch return error.ConnectionClosed;
        return self.writeAllBefore(bytes, &deadline);
    }

    pub fn serveUntilIdle(self: *Server, service: *host_service.HostService) !void {
        return serve_loop.serveUntilIdleLoop(self, service, struct {
            fn recoverable(err: anyerror) bool {
                return err == error.ConnectionClosed;
            }
        }.recoverable);
    }

    /// The Win32 code of the last accept failure, 0 when none was recorded.
    pub fn lastSystemCode(self: *const Server) ?u32 {
        return windows_c.ob_windows_local_server_last_error(self.handle);
    }

    /// Replaces the pipe instance after an accept failure; the lifetime mutex stays held. Returns
    /// null once replaced, or the Win32 code of a failed replacement, which keeps the old instance
    /// for the next accept.
    pub fn renewAfterAcceptFailure(self: *Server) ?u32 {
        if (windows_c.ob_windows_local_server_renew(self.handle) == windows_c.OB_WINDOWS_IPC_OK) {
            return null;
        }
        return windows_c.ob_windows_local_server_last_error(self.handle);
    }

    pub fn serveOne(self: *Server, service: *host_service.HostService) !connection.Served {
        try requireStatus(windows_c.ob_windows_local_server_accept(self.handle));
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        var lanes = Lanes.init(.{ .primary = self.handle }, service.allocator);
        return connection.serveAcceptedConnection(self, service, &lanes);
    }

    pub fn serveOneBefore(
        self: *Server,
        service: *host_service.HostService,
        timeout_ms: u32,
    ) !bool {
        const status = windows_c.ob_windows_local_server_accept_before(
            self.handle,
            timeout_ms,
        );
        if (status == windows_c.OB_WINDOWS_IPC_ACCEPT_TIMEOUT) return false;
        try requireStatus(status);
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        var lanes = Lanes.init(.{ .primary = self.handle }, service.allocator);
        _ = connection.serveAcceptedConnection(self, service, &lanes);
        return true;
    }

    pub fn read(self: *Server, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_read(
            self.handle,
            bytes.ptr,
            try checkedBufferLength(bytes.len),
            &read_bytes,
        ));
        return checkedTransfer(read_bytes, bytes.len);
    }

    pub fn writeAll(self: *Server, bytes: []const u8) !void {
        var timer = monotonic.Timer.start() catch return error.ConnectionClosed;
        var deadline = connection.Deadline.init(&timer, connection.record_read_timeout_ns) catch return error.ConnectionClosed;
        self.writeAllBefore(bytes, &deadline) catch return error.ConnectionClosed;
    }

    pub fn writeAllBefore(self: *Server, bytes: []const u8, deadline: *connection.Deadline) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining_ns = deadline.remainingNs();
            if (remaining_ns <= 0) return error.WriteTimeout;
            const timeout_ms: u32 = @intCast(@min(
                std.math.divCeil(u64, remaining_ns, @as(u64, std.time.ns_per_ms)) catch unreachable,
                std.math.maxInt(u32),
            ));
            const remaining = bytes.len - offset;
            var written_bytes: u32 = 0;
            try requireStatus(windows_c.ob_windows_local_server_write_before(
                self.handle,
                bytes[offset..].ptr,
                try checkedBufferLength(remaining),
                timeout_ms,
                &written_bytes,
            ));
            offset += try checkedTransfer(written_bytes, remaining);
        }
    }

    /// Writes part of `bytes`, waiting at most `wait_ms`. Zero means the write is still pending:
    /// the caller passes the same bytes again, or calls `cancelPendingWrite`, before releasing them.
    pub fn writeSome(self: *Server, bytes: []const u8, wait_ms: u32) !usize {
        var written_bytes: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_write_wait(
            self.handle,
            bytes.ptr,
            try checkedBufferLength(bytes.len),
            wait_ms,
            &written_bytes,
        ));
        if (written_bytes > bytes.len) return error.ConnectionClosed;
        return written_bytes;
    }

    pub fn cancelPendingWrite(self: *Server) void {
        windows_c.ob_windows_local_server_cancel_write(self.handle);
    }

    pub fn waitReadable(self: *Server) !bool {
        var readable: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_wait_readable_or_wake(
            self.handle,
            connection.poll_interval_ms,
            if (self.observe_output_wake) output_wake.windowsEvent() else null,
            if (self.observe_output_wake) output_wake.windowsTimer() else null,
            &readable,
        ));
        return readable != 0;
    }
};

const Lanes = control_lanes.ControlLanes(LaneEndpoint);

/// Each control lane listens on its own instance of the endpoint's pipe, created when the Worker
/// session starts and closed when it ends, so no idle instance can take a Worker's connection. A
/// lane waits for a connection until the primary stops its instances, so an idle lane never wakes.
const LaneEndpoint = if (builtin.os.tag != .windows) void else struct {
    primary: *windows_c.ObWindowsLocalServer,

    pub fn armStop(self: *LaneEndpoint) void {
        windows_c.ob_windows_local_server_resume_instances(self.primary);
    }

    pub fn signalStop(self: *LaneEndpoint) void {
        windows_c.ob_windows_local_server_stop_instances(self.primary);
    }

    pub const Lane = struct {
        instance: Server,

        pub fn open(endpoint: *LaneEndpoint) !Lane {
            var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
            const handle = windows_c.ob_windows_local_server_create_instance(
                endpoint.primary,
                &status,
            ) orelse return statusError(status);
            return .{ .instance = .{ .handle = handle, .observe_output_wake = false } };
        }

        pub fn close(self: *Lane) void {
            windows_c.ob_windows_local_server_destroy(self.instance.handle);
        }

        /// Whether a client connected before the lanes stopped; one that left before the listen
        /// is dropped like on the primary instance. Its identity is checked at its first byte.
        pub fn accept(self: *Lane) !bool {
            const status = windows_c.ob_windows_local_server_accept(self.instance.handle);
            return switch (status) {
                windows_c.OB_WINDOWS_IPC_OK => true,
                windows_c.OB_WINDOWS_IPC_ACCEPT_TIMEOUT,
                windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED,
                => false,
                else => statusError(status),
            };
        }

        pub fn transport(self: *Lane) *Server {
            return &self.instance;
        }

        pub fn finish(self: *Lane) void {
            windows_c.ob_windows_local_server_disconnect(self.instance.handle);
        }
    };
};

pub fn statusError(status: c_int) anyerror {
    if (builtin.os.tag != .windows) unreachable;
    return switch (status) {
        windows_c.OB_WINDOWS_IPC_ALREADY_RUNNING => error.AlreadyRunning,
        windows_c.OB_WINDOWS_IPC_INVALID_RUNTIME_DIRECTORY => error.InsecureRuntimeDirectory,
        windows_c.OB_WINDOWS_IPC_PEER_IDENTITY_MISMATCH => error.PeerIdentityMismatch,
        windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED => error.ConnectionClosed,
        windows_c.OB_WINDOWS_IPC_WRITE_TIMEOUT => error.WriteTimeout,
        else => error.WindowsIpcFailure,
    };
}

pub fn requireStatus(status: c_int) !void {
    if (builtin.os.tag != .windows) unreachable;
    if (status != windows_c.OB_WINDOWS_IPC_OK) return statusError(status);
}

pub fn checkedBufferLength(length: usize) !u32 {
    return std.math.cast(u32, length) orelse error.IpcTransferTooLarge;
}

pub fn checkedTransfer(transferred: u32, capacity: usize) !usize {
    if (transferred == 0 or transferred > capacity) return error.ConnectionClosed;
    return transferred;
}
