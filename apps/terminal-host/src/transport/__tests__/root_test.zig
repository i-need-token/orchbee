const std = @import("std");
const builtin = @import("builtin");
const core_boundary = @import("terminal-host-core");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");
const subject = @import("terminal-host-transport");
const host_service = service_boundary.service;
const host_core = core_boundary.state;
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;

const windows_c = if (builtin.os.tag == .windows) @cImport({
    @cInclude("local_ipc_windows.h");
}) else struct {};
const LocalServer = subject.LocalServer;
const PosixServer = subject.testing.PosixServerType;
const WindowsServer = subject.testing.WindowsServerType;
const checkedWindowsBufferLength = subject.testing.checkedWindowsBufferLengthFn;
const checkedWindowsTransfer = subject.testing.checkedWindowsTransferFn;
const readRecord = subject.testing.readRecordFn;
const readRecordWithTimeout = subject.testing.readRecordWithTimeoutFn;
const requireWindowsStatus = subject.testing.requireWindowsStatusFn;
const posix_send_flags = subject.testing.posixSendFlags;
const serveAcceptedConnection = subject.testing.serveAcceptedConnectionFn;
const serveConnection = subject.testing.serveConnectionFn;
const serveConnectionWithHandshakeTimeout = subject.testing.serveConnectionWithHandshakeTimeoutFn;
const serveUntilIdleLoop = subject.testing.serveUntilIdleLoopFn;
const validEndpointId = subject.validEndpointId;
const windowsStatusError = subject.testing.windowsStatusErrorFn;
const writeOutgoing = subject.testing.writeOutgoingFn;

const TestClient = switch (builtin.os.tag) {
    .windows => WindowsTestClient,
    .freebsd, .linux, .macos => PosixTestClient,
    else => void,
};

const TestRuntimeDirectory = switch (builtin.os.tag) {
    .windows => WindowsTestRuntimeDirectory,
    .freebsd, .linux, .macos => PosixTestRuntimeDirectory,
    else => void,
};

const PosixTestRuntimeDirectory = if (builtin.os.tag == .windows) void else struct {
    allocator: std.mem.Allocator,
    directory: std.fs.Dir,
    path_bytes: []u8,

    fn init(allocator: std.mem.Allocator) !PosixTestRuntimeDirectory {
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

    fn deinit(self: *PosixTestRuntimeDirectory) void {
        self.directory.close();
        std.fs.deleteTreeAbsolute(self.path_bytes) catch {};
        self.allocator.free(self.path_bytes);
        self.* = undefined;
    }

    fn path(self: *const PosixTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }

    fn chmod(self: *PosixTestRuntimeDirectory, mode: std.fs.File.Mode) !void {
        try self.directory.chmod(mode);
    }
};

const WindowsTestRuntimeDirectory = if (builtin.os.tag != .windows) void else struct {
    allocator: std.mem.Allocator,
    path_bytes: []u8,
    temporary: std.testing.TmpDir,

    fn init(allocator: std.mem.Allocator) !WindowsTestRuntimeDirectory {
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

    fn deinit(self: *WindowsTestRuntimeDirectory) void {
        self.allocator.free(self.path_bytes);
        self.temporary.cleanup();
        self.* = undefined;
    }

    fn path(self: *const WindowsTestRuntimeDirectory) []const u8 {
        return self.path_bytes;
    }
};

const PosixTestClient = if (builtin.os.tag == .windows) void else struct {
    stream: std.net.Stream,

    fn connect(server: *const PosixServer, _: std.mem.Allocator, _: []const u8) !PosixTestClient {
        return .{ .stream = try std.net.connectUnixSocket(server.endpointPath()) };
    }

    fn close(self: *PosixTestClient) void {
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

const WindowsTestClient = if (builtin.os.tag != .windows) void else struct {
    handle: *anyopaque,

    fn connect(_: *const WindowsServer, allocator: std.mem.Allocator, endpoint_id: []const u8) !WindowsTestClient {
        const endpoint_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, endpoint_id);
        defer allocator.free(endpoint_w);
        var status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR;
        const handle = windows_c.ob_windows_local_test_client_connect(
            endpoint_w.ptr,
            &status,
        ) orelse return windowsStatusError(status);
        return .{ .handle = handle };
    }

    fn close(self: *WindowsTestClient) void {
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

test "endpoint id is exactly lowercase hexadecimal" {
    try std.testing.expect(validEndpointId("0123456789abcdef0123456789abcdef"));
    try std.testing.expect(!validEndpointId("0123456789ABCDEF0123456789ABCDEF"));
    try std.testing.expect(!validEndpointId("short"));
}

test "POSIX connection writes cannot terminate the Host with SIGPIPE" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    try std.testing.expect(posix_send_flags & std.posix.MSG.DONTWAIT != 0);
    try std.testing.expect(posix_send_flags & std.posix.MSG.NOSIGNAL != 0);
}

test "recoverable accept failure still evaluates the idle exit condition" {
    const RecoverableFailureServer = struct {
        calls: usize = 0,

        pub fn serveOne(self: *@This(), _: *host_service.HostService) !void {
            self.calls += 1;
            return error.TestRecoverableAcceptFailure;
        }

        pub fn serveOneBefore(_: *@This(), _: *host_service.HostService, _: u32) !bool {
            return error.TestUnexpectedAccept;
        }
    };
    const recoverable = struct {
        fn check(err: anyerror) bool {
            return err == error.TestRecoverableAcceptFailure;
        }
    }.check;

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var server: RecoverableFailureServer = .{};

    try serveUntilIdleLoop(&server, &service, recoverable);
    try std.testing.expectEqual(@as(usize, 1), server.calls);
}

// THDD-001 evidence: a PeerIdentityMismatch-class accept failure still follows
// the existing recoverable path. It is swallowed by the local-IPC recoverable
// filter, the idle exit condition is still evaluated, and it never escalates to
// a fatal teardown or the retained-cleanup window.
test "THDD-001 PeerIdentityMismatch accept failure stays on the recoverable path without retained-cleanup escalation" {
    const PeerMismatchServer = struct {
        calls: usize = 0,

        pub fn serveOne(self: *@This(), _: *host_service.HostService) !void {
            self.calls += 1;
            return error.PeerIdentityMismatch;
        }

        pub fn serveOneBefore(self: *@This(), _: *host_service.HostService, _: u32) !bool {
            self.calls += 1;
            return error.PeerIdentityMismatch;
        }
    };
    const recoverable = struct {
        fn check(err: anyerror) bool {
            return err == error.PeerIdentityMismatch;
        }
    }.check;

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var server: PeerMismatchServer = .{};

    try serveUntilIdleLoop(&server, &service, recoverable);
    // The recoverable accept failure was absorbed and the loop exited through
    // the idle condition, not through the error (no fatal escalation).
    try std.testing.expectEqual(@as(usize, 1), server.calls);
    try std.testing.expect(service.canStopWhenIdle());
}

test "accept timeouts continue Host maintenance without a broker" {
    const MaintenanceService = struct {
        idle: bool = false,
        maintenance_calls: usize = 0,

        pub fn maintain(self: *@This()) void {
            self.maintenance_calls += 1;
            if (self.maintenance_calls == 2) self.idle = true;
        }

        pub fn canStopWhenIdle(self: *const @This()) bool {
            return self.idle;
        }
    };
    const TimeoutServer = struct {
        calls: usize = 0,

        pub fn serveOne(self: *@This(), _: *MaintenanceService) !void {
            self.calls += 1;
        }

        pub fn serveOneBefore(
            self: *@This(),
            _: *MaintenanceService,
            _: u32,
        ) !bool {
            self.calls += 1;
            return false;
        }
    };
    const recoverable = struct {
        fn check(_: anyerror) bool {
            return false;
        }
    }.check;

    var service: MaintenanceService = .{};
    var server: TimeoutServer = .{};
    try serveUntilIdleLoop(&server, &service, recoverable);
    try std.testing.expectEqual(@as(usize, 2), service.maintenance_calls);
    try std.testing.expectEqual(@as(usize, 2), server.calls);
}

test "Windows IPC transfer adapters reject invalid C boundary lengths" {
    try std.testing.expectEqual(@as(u32, 4096), try checkedWindowsBufferLength(4096));
    if (@sizeOf(usize) > @sizeOf(u32)) {
        try std.testing.expectError(
            error.IpcTransferTooLarge,
            checkedWindowsBufferLength(@as(usize, std.math.maxInt(u32)) + 1),
        );
    }
    try std.testing.expectError(error.ConnectionClosed, checkedWindowsTransfer(0, 8));
    try std.testing.expectError(error.ConnectionClosed, checkedWindowsTransfer(9, 8));
    try std.testing.expectEqual(@as(usize, 8), try checkedWindowsTransfer(8, 8));
}

test "private local endpoint owns one lifetime lock and serves a handshake" {
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const runtime_directory = runtime.path();
    const endpoint_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    var server = try LocalServer.init(
        std.testing.allocator,
        runtime_directory,
        endpoint_id,
    );
    defer server.deinit();
    if (builtin.os.tag != .windows) {
        try std.testing.expectEqual(std.posix.AF.UNIX, server.listener.listen_address.any.family);
    }
    try std.testing.expectError(
        error.AlreadyRunning,
        LocalServer.init(std.testing.allocator, runtime_directory, endpoint_id),
    );

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var thread_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const server_thread = try std.Thread.spawn(.{}, serveTestUntilIdle, .{&thread_context});

    var client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    try expectHandshakeAccepted(&client);
    client.close();
    server_thread.join();
    if (thread_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().connectionCount());
}

test "Windows pipe recovers when a client closes before accept" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "dddddddddddddddddddddddddddddddd";
    var server = try LocalServer.init(
        std.testing.allocator,
        runtime.path(),
        endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();

    var abandoned = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    abandoned.close();
    try std.testing.expectError(error.ConnectionClosed, server.serveOne(&service));

    var context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const server_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&context});
    var replacement = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    try expectHandshakeAccepted(&replacement);
    replacement.close();
    server_thread.join();
    if (context.failure) |failure| return failure;
}

test "malformed local record closes only that connection" {
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "cccccccccccccccccccccccccccccccc";
    var server = try LocalServer.init(
        std.testing.allocator,
        runtime.path(),
        endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();

    var malformed_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const malformed_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&malformed_context});
    var malformed_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var malformed = [_]u8{0} ** protocol.transport_envelope_header_bytes;
    @memcpy(malformed[0..4], "OBTP");
    std.mem.writeInt(u16, malformed[4..6], protocol.transport_envelope_header_bytes, .little);
    std.mem.writeInt(u32, malformed[8..12], std.math.maxInt(u32), .little);
    try malformed_client.writeAll(&malformed);
    malformed_client.close();
    malformed_thread.join();
    if (malformed_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().connectionCount());

    var valid_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    const valid_thread = try std.Thread.spawn(.{}, serveTestConnection, .{&valid_context});
    var valid_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    try expectHandshakeAccepted(&valid_client);
    valid_client.close();
    valid_thread.join();
    if (valid_context.failure) |failure| return failure;
}

test "dispatch resource failure closes only the accepted connection" {
    const ResourceFailureTransport = struct {
        pub fn read(_: *@This(), _: []u8) !usize {
            return error.TestUnexpectedRead;
        }

        pub fn waitReadable(_: *@This()) !bool {
            return error.OutOfMemory;
        }

        pub fn writeAll(_: *@This(), _: []const u8) !void {}
    };

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    const retained_connection: u64 = 9;
    _ = try service.runtime_aggregate.core().connect(
        retained_connection,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    _ = try service.runtime_aggregate.core().createTerminal(
        retained_connection,
        host_core.CreationId.fromBytes([_]u8{2} ** 16),
        host_core.TerminalId.fromBytes([_]u8{3} ** 16),
        [_]u8{4} ** 32,
        80,
        24,
    );
    var transport: ResourceFailureTransport = .{};

    serveAcceptedConnection(&transport, &service);

    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.core().terminalCount());
    try std.testing.expect(
        service.runtime_aggregate.core().findTerminal(host_core.TerminalId.fromBytes([_]u8{3} ** 16)) != null,
    );
}

test "failed response handoff destroys the broker but preserves retained terminals" {
    const hello_payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":1024,"protocol":{"major":0,"minor":0,"patch":4},"rowWindowBytes":1024,"supportedOptionalFeatures":[]}
    ;
    const hello = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.hello,
        .control_json,
        hello_payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(hello);

    const FailingWriteTransport = struct {
        record: []const u8,
        offset: usize = 0,
        writes: usize = 0,

        pub fn read(self: *@This(), bytes: []u8) !usize {
            const length = @min(bytes.len, self.record.len - self.offset);
            @memcpy(bytes[0..length], self.record[self.offset..][0..length]);
            self.offset += length;
            return length;
        }

        pub fn waitReadable(self: *@This()) !bool {
            return self.offset < self.record.len;
        }

        pub fn writeAll(self: *@This(), _: []const u8) !void {
            self.writes += 1;
            return error.InjectedWriteFailure;
        }
    };

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    const setup_connection: u64 = 9;
    _ = try service.runtime_aggregate.core().connect(
        setup_connection,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    _ = try service.runtime_aggregate.core().createTerminal(
        setup_connection,
        host_core.CreationId.fromBytes([_]u8{2} ** 16),
        host_core.TerminalId.fromBytes([_]u8{3} ** 16),
        [_]u8{4} ** 32,
        80,
        24,
    );
    _ = service.runtime_aggregate.core().disconnect(setup_connection);

    var transport: FailingWriteTransport = .{ .record = hello };
    try std.testing.expectError(
        error.InjectedWriteFailure,
        serveConnection(&transport, &service),
    );
    try std.testing.expectEqual(@as(usize, 1), transport.writes);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().connectionCount());
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.core().terminalCount());
}

test "multi-record handoff stops at the first write failure" {
    const PrefixTransport = struct {
        writes: usize = 0,
        first: []const u8 = &.{},

        pub fn writeAll(self: *@This(), bytes: []const u8) !void {
            self.writes += 1;
            if (self.writes == 2) return error.InjectedWriteFailure;
            self.first = bytes;
        }
    };

    const records = [_][]const u8{ "first", "second", "third" };
    var transport: PrefixTransport = .{};
    try std.testing.expectError(
        error.InjectedWriteFailure,
        writeOutgoing(&transport, &records),
    );
    try std.testing.expectEqual(@as(usize, 2), transport.writes);
    try std.testing.expectEqualStrings("first", transport.first);
}

test "partial transport records have a bounded read lifetime" {
    const StalledTransport = struct {
        first_byte_read: bool = false,

        pub fn read(self: *@This(), bytes: []u8) !usize {
            if (self.first_byte_read) return error.TestUnexpectedRead;
            bytes[0] = 'O';
            self.first_byte_read = true;
            return 1;
        }

        pub fn waitReadable(self: *@This()) !bool {
            return !self.first_byte_read;
        }
    };

    var transport: StalledTransport = .{};
    try std.testing.expectError(
        error.ConnectionClosed,
        readRecordWithTimeout(
            std.testing.allocator,
            &transport,
            std.time.ns_per_ms,
        ),
    );
}

test "silent accepted connections have a bounded handshake lifetime" {
    const SilentTransport = struct {
        pub fn read(_: *@This(), _: []u8) !usize {
            return error.TestUnexpectedRead;
        }

        pub fn waitReadable(_: *@This()) !bool {
            return false;
        }

        pub fn writeAll(_: *@This(), _: []const u8) !void {}
    };

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var transport: SilentTransport = .{};
    try std.testing.expectError(
        error.ConnectionClosed,
        serveConnectionWithHandshakeTimeout(
            &transport,
            &service,
            std.time.ns_per_ms,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().connectionCount());
}

test "partial handshakes cannot gate broker-independent cleanup maintenance" {
    const PartialTransport = struct {
        readable_checks: usize = 0,
        supplied_byte: bool = false,

        pub fn read(self: *@This(), bytes: []u8) !usize {
            if (self.supplied_byte) return error.TestUnexpectedRead;
            bytes[0] = 'O';
            self.supplied_byte = true;
            return 1;
        }

        pub fn waitReadable(self: *@This()) !bool {
            self.readable_checks += 1;
            return self.readable_checks == 1;
        }

        pub fn writeAll(_: *@This(), _: []const u8) !void {}
    };

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    const core = service.runtime_aggregate.core();
    const connection_id: host_core.ConnectionId = 7;
    _ = try core.connect(
        connection_id,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    const first_terminal_id = host_core.TerminalId.fromBytes([_]u8{2} ** 16);
    const second_terminal_id = host_core.TerminalId.fromBytes([_]u8{3} ** 16);
    for ([_]host_core.TerminalId{ first_terminal_id, second_terminal_id }, 0..) |terminal_id, index| {
        _ = try core.createTerminal(
            connection_id,
            host_core.CreationId.fromBytes([_]u8{@intCast(index + 4)} ** 16),
            terminal_id,
            [_]u8{@intCast(index + 6)} ** 32,
            80,
            24,
        );
        _ = try core.runtimeFailed(terminal_id, .read_output);
        _ = try core.failureContainmentBlocked(
            terminal_id,
            .contain_process,
            if (index == 0) 0 else std.math.maxInt(i128),
        );
    }

    var transport: PartialTransport = .{};
    try std.testing.expectError(
        error.ConnectionClosed,
        serveConnectionWithHandshakeTimeout(
            &transport,
            &service,
            std.time.ns_per_ms,
        ),
    );
    // The first due terminal remains retryable because this deterministic model
    // has no native runtime, but the fair cleanup cursor proves maintenance ran.
    try std.testing.expectEqual(
        @as(usize, 1),
        host_runtime_aggregate.testing.cleanupRetryScanIndexFn(&service.runtime_aggregate),
    );
}

fn expectHandshakeAccepted(client: *TestClient) !void {
    const hello_payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":1024,"protocol":{"major":0,"minor":0,"patch":4},"rowWindowBytes":1024,"supportedOptionalFeatures":[]}
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

const ServeThreadContext = struct {
    server: *LocalServer,
    service: *host_service.HostService,
    failure: ?anyerror = null,
};

fn serveTestConnection(context: *ServeThreadContext) void {
    context.server.serveOne(context.service) catch |err| {
        context.failure = err;
    };
}

fn serveTestUntilIdle(context: *ServeThreadContext) void {
    context.server.serveUntilIdle(context.service) catch |err| {
        context.failure = err;
    };
}

test "POSIX endpoint rejects a directory visible to another account" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    try runtime.chmod(0o755);
    const runtime_directory = runtime.path();
    try std.testing.expectError(
        error.InsecureRuntimeDirectory,
        LocalServer.init(
            std.testing.allocator,
            runtime_directory,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
    );
}
