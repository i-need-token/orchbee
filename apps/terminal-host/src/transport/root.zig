const builtin = @import("builtin");

const connection = @import("connection.zig");
const control_lanes = @import("control_lanes.zig");
const posix = @import("local_server_posix.zig");
const windows = @import("local_server_windows.zig");

pub const LocalServer = switch (builtin.os.tag) {
    .windows => windows.Server,
    .freebsd, .linux, .macos => posix.Server,
    else => @compileError("terminal-host has no local IPC implementation for this target"),
};

pub const validEndpointId = connection.validEndpointId;

pub const testing = if (builtin.is_test) struct {
    pub const checkedWindowsBufferLengthFn = windows.checkedBufferLength;
    pub const checkedWindowsTransferFn = windows.checkedTransfer;
    pub const PosixServerType = posix.Server;
    pub const readRecordFn = connection.readRecord;
    pub const readRecordWithTimeoutFn = connection.readRecordWithTimeout;
    pub const requireWindowsStatusFn = windows.requireStatus;
    pub const posixSendFlags = posix.send_flags;
    pub const posixCompat = @import("terminal-host-platform").posix_compat;
    pub const posixEndpointSecurity = posix.endpoint_security;
    pub const ServedType = connection.Served;
    /// Serves an accepted test transport without control lanes.
    pub fn serveAcceptedConnectionFn(transport: anytype, service: anytype) void {
        var lanes: control_lanes.None = .{};
        _ = connection.serveAcceptedConnection(transport, service, &lanes);
    }
    pub const serveConnectionFn = connection.serveConnection;
    pub const serveConnectionWithHandshakeTimeoutFn = connection.serveConnectionWithHandshakeTimeout;
    pub const serveUntilIdleLoopFn = connection.serveUntilIdleLoop;
    pub const writeOutgoingFn = connection.writeOutgoing;
    pub const WindowsServerType = windows.Server;
    pub const windowsStatusErrorFn = windows.statusError;
} else struct {};
