const std = @import("std");
const builtin = @import("builtin");

const host_service = @import("terminal-host-service").service;
const monotonic = @import("terminal-host-platform").monotonic;
const connection = @import("connection.zig");

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};

pub const Server = if (builtin.os.tag != .windows) void else struct {
    handle: *windows_c.ObWindowsLocalServer,

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
                return err == error.ConnectionClosed or err == error.PeerIdentityMismatch;
            }
        }.recoverable);
    }

    pub fn serveOne(self: *Server, service: *host_service.HostService) !void {
        try requireStatus(windows_c.ob_windows_local_server_accept(self.handle));
        defer windows_c.ob_windows_local_server_disconnect(self.handle);
        connection.serveAcceptedConnection(self, service);
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
        connection.serveAcceptedConnection(self, service);
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
        var available: u32 = 0;
        try requireStatus(windows_c.ob_windows_local_server_peek(
            self.handle,
            &available,
        ));
        if (available > 0) return true;
        monotonic.sleep(connection.poll_interval_ms * std.time.ns_per_ms);
        return false;
    }
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
