const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const monotonic = @import("terminal-host-platform").monotonic;
const output_wake = @import("terminal-host-platform").output_wake;
const connection = @import("connection.zig");
const control_lanes = @import("control_lanes.zig");

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

    pub fn deinit(self: *Server) void {
        windows_c.ob_windows_local_server_destroy(self.handle);
        self.* = undefined;
    }

    pub fn serveUntilIdle(self: *Server, service: *host_service.HostService) !void {
        return connection.serveUntilIdleLoop(self, service, struct {
            fn recoverable(err: anyerror) bool {
                return err == error.ConnectionClosed;
            }
        }.recoverable);
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
