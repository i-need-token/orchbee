pub const std = @import("std");
pub const builtin = @import("builtin");
pub const core_boundary = @import("terminal-host-core");
pub const protocol = @import("terminal-protocol");
pub const service_boundary = @import("terminal-host-service");
pub const subject = @import("terminal-host-transport");
pub const host_service = service_boundary.service;
pub const host_core = core_boundary.state;
pub const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;

pub const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};
pub const LocalServer = subject.LocalServer;
pub const PosixServer = subject.testing.PosixServerType;
pub const WindowsServer = subject.testing.WindowsServerType;
pub const checkedWindowsBufferLength = subject.testing.checkedWindowsBufferLengthFn;
pub const checkedWindowsTransfer = subject.testing.checkedWindowsTransferFn;
pub const createModel = host_core.testing.createTerminal;
pub const readRecord = subject.testing.readRecordFn;
pub const readRecordWithTimeout = subject.testing.readRecordWithTimeoutFn;
pub const requireWindowsStatus = subject.testing.requireWindowsStatusFn;
pub const posix_send_flags = subject.testing.posixSendFlags;
pub const serveAcceptedConnection = subject.testing.serveAcceptedConnectionFn;
pub const serveConnection = subject.testing.serveConnectionFn;
pub const serveConnectionWithHandshakeTimeout = subject.testing.serveConnectionWithHandshakeTimeoutFn;
pub const serveUntilIdleLoop = subject.testing.serveUntilIdleLoopFn;
pub const validEndpointId = subject.validEndpointId;
pub const windowsStatusError = subject.testing.windowsStatusErrorFn;
pub const writeOutgoing = subject.testing.writeOutgoingFn;

pub fn serviceAggregate(
    service: *host_service.HostService,
) *host_runtime_aggregate.HostRuntimeAggregate {
    return &service.runtime_aggregate;
}

pub fn serviceCore(service: *host_service.HostService) *host_core.HostCore {
    return serviceAggregate(service).core();
}

pub const TestClient = switch (builtin.os.tag) {
    .windows => WindowsTestClient,
    .freebsd, .linux, .macos => PosixTestClient,
    else => void,
};

pub const TestRuntimeDirectory = switch (builtin.os.tag) {
    .windows => WindowsTestRuntimeDirectory,
    .freebsd, .linux, .macos => PosixTestRuntimeDirectory,
    else => void,
};

pub const PosixTestRuntimeDirectory = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    path_bytes: []u8,

    pub fn init(allocator: std.mem.Allocator) !PosixTestRuntimeDirectory {
        var random: [8]u8 = undefined;
        std.crypto.random.bytes(&random);
        const encoded = std.fmt.bytesToHex(random, .lower);
        const path_bytes = try std.fmt.allocPrint(allocator, "/tmp/ob-{s}", .{encoded});
        errdefer allocator.free(path_bytes);
        try std.posix.mkdir(path_bytes, 0o700);
        errdefer std.fs.deleteDirAbsolute(path_bytes) catch {};
        const directory = try std.fs.openDirAbsolute(path_bytes, .{ .no_follow = true });
        return .{
            .allocator = allocator,
            .directory = directory,
            .path_bytes = path_bytes,
        };
    }

    pub fn deinit(self: *PosixTestRuntimeDirectory) void {
        self.directory.close();
        std.fs.deleteTreeAbsolute(self.path_bytes) catch {};
        self.allocator.free(self.path_bytes);
        self.* = undefined;
    }

    pub fn path(self: *const PosixTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }

    pub fn chmod(self: *PosixTestRuntimeDirectory, mode: std.fs.File.Mode) !void {
        try self.directory.chmod(mode);
    }
};

pub const WindowsTestRuntimeDirectory = if (builtin.os.tag != .windows) void else struct {
    allocator: std.mem.Allocator,
    path_bytes: []u8,
    temporary: std.testing.TmpDir,

    pub fn init(allocator: std.mem.Allocator) !WindowsTestRuntimeDirectory {
        var temporary = std.testing.tmpDir(.{});
        errdefer temporary.cleanup();
        const path_bytes = try temporary.parent_dir.realpathAlloc(
            allocator,
            &temporary.sub_path,
        );
        errdefer allocator.free(path_bytes);
        const path_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path_bytes);
        defer allocator.free(path_w);
        try requireWindowsStatus(windows_c.ob_windows_local_test_make_directory_private(path_w.ptr));
        return .{
            .allocator = allocator,
            .path_bytes = path_bytes,
            .temporary = temporary,
        };
    }

    pub fn deinit(self: *WindowsTestRuntimeDirectory) void {
        self.allocator.free(self.path_bytes);
        self.temporary.cleanup();
        self.* = undefined;
    }

    pub fn path(self: *const WindowsTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }
};

pub const PosixTestClient = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

    pub fn connect(server: *const PosixServer, _: std.mem.Allocator, _: []const u8) !PosixTestClient {
        return .{ .stream = try std.net.connectUnixSocket(server.endpointPath()) };
    }

    pub fn close(self: *PosixTestClient) void {
        self.stream.close();
    }

    pub fn read(self: *PosixTestClient, bytes: []u8) !usize {
        return self.stream.read(bytes);
    }

    pub fn waitReadable(_: *PosixTestClient) !bool {
        return true;
    }

    pub fn writeAll(self: *PosixTestClient, bytes: []const u8) !void {
        try self.stream.writeAll(bytes);
    }
};

pub const WindowsTestClient = if (builtin.os.tag != .windows) void else struct {
    handle: *anyopaque,

    pub fn connect(_: *const WindowsServer, allocator: std.mem.Allocator, endpoint_id: []const u8) !WindowsTestClient {
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_test_client_connect(
            endpoint_w.ptr,
            &status,
        ) orelse return windowsStatusError(status);
        return .{ .handle = handle };
    }

    pub fn close(self: *WindowsTestClient) void {
        windows_c.ob_windows_local_test_client_close(self.handle);
    }

    pub fn read(self: *WindowsTestClient, bytes: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_test_client_read(
            self.handle,
            bytes.ptr,
            try checkedWindowsBufferLength(bytes.len),
            &read_bytes,
        ));
        return checkedWindowsTransfer(read_bytes, bytes.len);
    }

    pub fn waitReadable(_: *WindowsTestClient) !bool {
        return true;
    }

    pub fn writeAll(self: *WindowsTestClient, bytes: []const u8) !void {
        var offset: usize = 0;
        while (offset < bytes.len) {
            const remaining = bytes.len - offset;
            var written_bytes: u32 = 0;
            try requireWindowsStatus(windows_c.ob_windows_local_test_client_write(
                self.handle,
                bytes[offset..].ptr,
                try checkedWindowsBufferLength(remaining),
                &written_bytes,
            ));
            offset += try checkedWindowsTransfer(written_bytes, remaining);
        }
    }
};

// THDD-001 evidence: a PeerIdentityMismatch-class accept failure still follows
// the existing recoverable path. It is swallowed by the local-IPC recoverable
// filter, the idle exit condition is still evaluated, and it never escalates to
// a fatal teardown or the retained-cleanup window.

pub fn expectHandshakeAccepted(client: *TestClient) !void {
    const hello_payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":1024,"protocol":{"major":0,"minor":0,"patch":8},"rowWindowBytes":1024,"supportedOptionalFeatures":[]}
    ;
    const hello = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        1,
        .control_json,
        hello_payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(hello);
    try client.writeAll(hello);
    const response_record = try readRecord(std.testing.allocator, client);
    defer std.testing.allocator.free(response_record);
    const response = try protocol.validateTransportEnvelope(response_record);
    try std.testing.expectEqual(@as(u16, 2), response.kind);
}

pub const ServeThreadContext = struct {
    server: *LocalServer,
    service: *host_service.HostService,
    failure: ?anyerror = null,
};

pub fn serveTestConnection(context: *ServeThreadContext) void {
    context.server.serveOne(context.service) catch |err| {
        context.failure = err;
    };
}

pub fn serveTestUntilIdle(context: *ServeThreadContext) void {
    context.server.serveUntilIdle(context.service) catch |err| {
        context.failure = err;
    };
}
