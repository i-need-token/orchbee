const harness = @import("transport_harness.zig");
const serviceAggregate = harness.serviceAggregate;
const serviceCore = harness.serviceCore;
const std = harness.std;
const builtin = harness.builtin;
const core_boundary = harness.core_boundary;
const protocol = harness.protocol;
const service_boundary = harness.service_boundary;
const subject = harness.subject;
const host_service = harness.host_service;
const host_core = harness.host_core;
const host_runtime_aggregate = harness.host_runtime_aggregate;
const monotonic = harness.monotonic;
const windows_c = harness.windows_c;
const LocalServer = harness.LocalServer;
const PosixServer = harness.PosixServer;
const WindowsServer = harness.WindowsServer;
const checkedWindowsBufferLength = harness.checkedWindowsBufferLength;
const checkedWindowsTransfer = harness.checkedWindowsTransfer;
const createModel = harness.createModel;
const readRecord = harness.readRecord;
const readRecordWithTimeout = harness.readRecordWithTimeout;
const requireWindowsStatus = harness.requireWindowsStatus;
const posix_send_flags = harness.posix_send_flags;
const serveAcceptedConnection = harness.serveAcceptedConnection;
const serveConnection = harness.serveConnection;
const serveConnectionWithHandshakeTimeout = harness.serveConnectionWithHandshakeTimeout;
const serveUntilIdleLoop = harness.serveUntilIdleLoop;
const validEndpointId = harness.validEndpointId;
const windowsStatusError = harness.windowsStatusError;
const writeOutgoing = harness.writeOutgoing;
const TestClient = harness.TestClient;
const TestRuntimeDirectory = harness.TestRuntimeDirectory;
const PosixTestRuntimeDirectory = harness.PosixTestRuntimeDirectory;
const WindowsTestRuntimeDirectory = harness.WindowsTestRuntimeDirectory;
const PosixTestClient = harness.PosixTestClient;
const WindowsTestClient = harness.WindowsTestClient;
const expectHandshakeAccepted = harness.expectHandshakeAccepted;
const ServeThreadContext = harness.ServeThreadContext;
const serveTestConnection = harness.serveTestConnection;
const serveTestUntilIdle = harness.serveTestUntilIdle;

const ThreadJoinGuard = struct {
    thread: ?std.Thread,

    fn init(thread: std.Thread) ThreadJoinGuard {
        return .{ .thread = thread };
    }

    fn join(self: *ThreadJoinGuard) void {
        const thread = self.thread orelse return;
        self.thread = null;
        thread.join();
    }
};

const WindowsReadinessFixture = if (builtin.os.tag != .windows) void else struct {
    runtime: WindowsTestRuntimeDirectory,
    server: WindowsServer,
    client: WindowsTestClient,
    server_handle: ?*windows_c.ObWindowsLocalServer,
    endpoint_id: []const u8,
    client_open: bool = true,
    connection_open: bool = true,

    fn init(endpoint_id: []const u8, authentication_byte: u8) !@This() {
        var runtime = try WindowsTestRuntimeDirectory.init(std.testing.allocator);
        errdefer runtime.deinit();
        var server = try WindowsServer.init(
            std.testing.allocator,
            runtime.path(),
            endpoint_id,
        );
        errdefer server.deinit();
        var client = try WindowsTestClient.connect(
            &server,
            std.testing.allocator,
            endpoint_id,
        );
        errdefer client.close();
        const server_handle: ?*windows_c.ObWindowsLocalServer = @ptrCast(server.handle);
        try client.writeAll(&.{authentication_byte});
        try requireWindowsStatus(windows_c.ob_windows_local_server_accept_before(
            server_handle,
            1_000,
        ));
        var authenticated: [1]u8 = undefined;
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_read(
            server_handle,
            &authenticated,
            authenticated.len,
            &read_bytes,
        ));
        try std.testing.expectEqual(@as(u32, 1), read_bytes);
        try std.testing.expectEqual(authentication_byte, authenticated[0]);
        return .{
            .runtime = runtime,
            .server = server,
            .client = client,
            .server_handle = server_handle,
            .endpoint_id = endpoint_id,
        };
    }

    fn deinit(self: *@This()) void {
        self.disconnectServer();
        self.closeClient();
        self.server.deinit();
        self.runtime.deinit();
        self.* = undefined;
    }

    fn closeClient(self: *@This()) void {
        if (!self.client_open) return;
        self.client.close();
        self.client_open = false;
    }

    fn disconnectServer(self: *@This()) void {
        if (!self.connection_open) return;
        windows_c.ob_windows_local_server_disconnect(self.server_handle);
        self.connection_open = false;
    }

    fn reconnect(self: *@This(), authentication_byte: u8) !void {
        var accept_context: WindowsAccept = .{
            .server_handle = self.server_handle,
        };
        var accept_thread = ThreadJoinGuard.init(try std.Thread.spawn(
            .{},
            WindowsAccept.run,
            .{&accept_context},
        ));
        defer accept_thread.join();
        self.client = try WindowsTestClient.connect(
            &self.server,
            std.testing.allocator,
            self.endpoint_id,
        );
        self.client_open = true;
        try self.client.writeAll(&.{authentication_byte});
        accept_thread.join();
        try requireWindowsStatus(accept_context.status);
        self.connection_open = true;
        var authenticated: [1]u8 = undefined;
        const read_bytes = try self.readOnce(&authenticated);
        try std.testing.expectEqual(@as(usize, 1), read_bytes);
        try std.testing.expectEqual(authentication_byte, authenticated[0]);
    }

    fn waitReadable(self: *@This(), timeout_ms: u32) !bool {
        var readable: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_wait_readable(
            self.server_handle,
            timeout_ms,
            &readable,
        ));
        return readable != 0;
    }

    fn readOnce(self: *@This(), buffer: []u8) !usize {
        var read_bytes: u32 = 0;
        try requireWindowsStatus(windows_c.ob_windows_local_server_read(
            self.server_handle,
            buffer.ptr,
            try checkedWindowsBufferLength(buffer.len),
            &read_bytes,
        ));
        return checkedWindowsTransfer(read_bytes, buffer.len);
    }

    fn readExact(self: *@This(), buffer: []u8) !void {
        var offset: usize = 0;
        while (offset < buffer.len) {
            if (!try self.waitReadable(1_000)) return error.TestReadTimeout;
            offset += try self.readOnce(buffer[offset..]);
        }
    }
};

const WindowsAccept = if (builtin.os.tag != .windows) void else struct {
    server_handle: ?*windows_c.ObWindowsLocalServer,
    status: c_int = windows_c.OB_WINDOWS_IPC_SYSTEM_ERROR,

    fn run(self: *@This()) void {
        self.status = windows_c.ob_windows_local_server_accept_before(
            self.server_handle,
            5_000,
        );
    }
};

const DelayedWindowsWrite = if (builtin.os.tag != .windows) void else struct {
    client: *WindowsTestClient,
    bytes: []const u8,
    failure: ?anyerror = null,

    fn run(self: *@This()) void {
        monotonic.sleep(10 * std.time.ns_per_ms);
        self.client.writeAll(self.bytes) catch |err| {
            self.failure = err;
        };
    }
};

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
        try std.testing.expect(server.listener.socket.handle >= 0);
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
    var client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var client_open = true;
    var server_thread = ThreadJoinGuard.init(
        std.Thread.spawn(.{}, serveTestUntilIdle, .{&thread_context}) catch |err| {
            client.close();
            return err;
        },
    );
    defer server_thread.join();
    defer if (client_open) client.close();
    try expectHandshakeAccepted(&client);
    client.close();
    client_open = false;
    server_thread.join();
    if (thread_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
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
    var server_thread = ThreadJoinGuard.init(
        try std.Thread.spawn(.{}, serveTestConnection, .{&context}),
    );
    defer server_thread.join();
    var replacement = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var replacement_open = true;
    defer if (replacement_open) replacement.close();
    try expectHandshakeAccepted(&replacement);
    replacement.close();
    replacement_open = false;
    server_thread.join();
    if (context.failure) |failure| return failure;
}
test "Windows pipe requires a first byte within the accept deadline" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    var server = try LocalServer.init(
        std.testing.allocator,
        runtime.path(),
        endpoint_id,
    );
    defer server.deinit();
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();

    var silent = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var silent_open = true;
    defer if (silent_open) silent.close();
    try std.testing.expect(!try server.serveOneBefore(&service, 20));
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
    silent.close();
    silent_open = false;

    var context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    var server_thread = ThreadJoinGuard.init(
        try std.Thread.spawn(.{}, serveTestConnection, .{&context}),
    );
    defer server_thread.join();
    var replacement = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var replacement_open = true;
    defer if (replacement_open) replacement.close();
    try expectHandshakeAccepted(&replacement);
    replacement.close();
    replacement_open = false;
    server_thread.join();
    if (context.failure) |failure| return failure;
}
test "Windows pipe delivers its authenticated first byte before peer EOF" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "ffffffffffffffffffffffffffffffff";
    var server = try LocalServer.init(
        std.testing.allocator,
        runtime.path(),
        endpoint_id,
    );
    defer server.deinit();
    const server_handle: ?*windows_c.ObWindowsLocalServer = @ptrCast(server.handle);

    var client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var client_open = true;
    defer if (client_open) client.close();
    try client.writeAll("O");
    try requireWindowsStatus(windows_c.ob_windows_local_server_accept_before(
        server_handle,
        1_000,
    ));
    client.close();
    client_open = false;

    var available: u32 = 0;
    try requireWindowsStatus(windows_c.ob_windows_local_server_peek(
        server_handle,
        &available,
    ));
    try std.testing.expectEqual(@as(u32, 1), available);
    try requireWindowsStatus(windows_c.ob_windows_local_server_peek(
        server_handle,
        &available,
    ));
    try std.testing.expectEqual(@as(u32, 1), available);
    var byte: [1]u8 = undefined;
    var read_bytes: u32 = 0;
    try requireWindowsStatus(windows_c.ob_windows_local_server_read(
        server_handle,
        &byte,
        byte.len,
        &read_bytes,
    ));
    try std.testing.expectEqual(@as(u32, 1), read_bytes);
    try std.testing.expectEqual(@as(u8, 'O'), byte[0]);
    try std.testing.expectEqual(
        windows_c.OB_WINDOWS_IPC_CONNECTION_CLOSED,
        windows_c.ob_windows_local_server_peek(server_handle, &available),
    );
    windows_c.ob_windows_local_server_disconnect(server_handle);

    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    var server_thread = ThreadJoinGuard.init(
        try std.Thread.spawn(.{}, serveTestConnection, .{&context}),
    );
    defer server_thread.join();
    var replacement = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var replacement_open = true;
    defer if (replacement_open) replacement.close();
    try expectHandshakeAccepted(&replacement);
    replacement.close();
    replacement_open = false;
    server_thread.join();
    if (context.failure) |failure| return failure;
}
test "Windows readiness preserves a byte that arrives after a timeout" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "11111111111111111111111111111111",
        'A',
    );
    defer fixture.deinit();

    try std.testing.expect(!try fixture.waitReadable(1));
    try fixture.client.writeAll("late");
    try std.testing.expect(try fixture.waitReadable(1_000));
    var received: [4]u8 = undefined;
    try fixture.readExact(&received);
    try std.testing.expectEqualStrings("late", &received);
}
test "Windows readiness wakes when a client writes during the wait" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "22222222222222222222222222222222",
        'A',
    );
    defer fixture.deinit();

    try std.testing.expect(!try fixture.waitReadable(1));
    var delayed_write: DelayedWindowsWrite = .{
        .client = &fixture.client,
        .bytes = "wake",
    };
    var writer = ThreadJoinGuard.init(try std.Thread.spawn(
        .{},
        DelayedWindowsWrite.run,
        .{&delayed_write},
    ));
    defer writer.join();
    try std.testing.expect(try fixture.waitReadable(1_000));
    writer.join();
    if (delayed_write.failure) |failure| return failure;
    var received: [4]u8 = undefined;
    try fixture.readExact(&received);
    try std.testing.expectEqualStrings("wake", &received);
}
test "Windows pending readiness wakes with EOF when the client closes" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "66666666666666666666666666666666",
        'A',
    );
    defer fixture.deinit();

    try std.testing.expect(!try fixture.waitReadable(1));
    fixture.closeClient();
    try std.testing.expectError(
        error.ConnectionClosed,
        fixture.waitReadable(1_000),
    );
}
test "Windows disconnect cancels and drains a pending readiness read" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "33333333333333333333333333333333",
        'A',
    );
    defer fixture.deinit();

    try std.testing.expect(!try fixture.waitReadable(1));
    fixture.disconnectServer();
    fixture.closeClient();
    try fixture.reconnect('B');
    try fixture.client.writeAll("fresh");
    var received: [5]u8 = undefined;
    try fixture.readExact(&received);
    try std.testing.expectEqualStrings("fresh", &received);
}
test "Windows reconnect discards a completed readiness byte from the old client" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "44444444444444444444444444444444",
        'A',
    );
    defer fixture.deinit();

    try std.testing.expect(!try fixture.waitReadable(1));
    try fixture.client.writeAll("stale");
    try std.testing.expect(try fixture.waitReadable(1_000));
    fixture.disconnectServer();
    fixture.closeClient();
    try fixture.reconnect('B');
    try fixture.client.writeAll("new");
    var received: [3]u8 = undefined;
    try fixture.readExact(&received);
    try std.testing.expectEqualStrings("new", &received);
}
test "Windows repeated readiness timeouts retain one read and bulk-read queued bytes" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try WindowsReadinessFixture.init(
        "55555555555555555555555555555555",
        'A',
    );
    defer fixture.deinit();

    for (0..4) |_| try std.testing.expect(!try fixture.waitReadable(1));
    var available: u32 = 99;
    try requireWindowsStatus(windows_c.ob_windows_local_server_peek(
        fixture.server_handle,
        &available,
    ));
    try std.testing.expectEqual(@as(u32, 0), available);

    var expected: [4_096]u8 = undefined;
    for (&expected, 0..) |*byte, index| byte.* = @intCast(index % 251);
    try fixture.client.writeAll(&expected);

    var received: [4_096]u8 = undefined;
    const readiness_byte_count = try fixture.readOnce(received[0..]);
    try std.testing.expectEqual(@as(usize, 1), readiness_byte_count);
    try std.testing.expect(try fixture.waitReadable(1_000));
    const queued_byte_count = try fixture.readOnce(received[1..]);
    try std.testing.expectEqual(expected.len - 1, queued_byte_count);
    try std.testing.expectEqualSlices(u8, &expected, &received);
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
    var malformed_thread = ThreadJoinGuard.init(
        try std.Thread.spawn(.{}, serveTestConnection, .{&malformed_context}),
    );
    defer malformed_thread.join();
    var malformed_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var malformed_client_open = true;
    defer if (malformed_client_open) malformed_client.close();
    var malformed = [_]u8{0} ** protocol.transport_envelope_header_bytes;
    @memcpy(malformed[0..4], "OBTP");
    std.mem.writeInt(u16, malformed[4..6], protocol.transport_envelope_header_bytes, .little);
    std.mem.writeInt(u32, malformed[8..12], std.math.maxInt(u32), .little);
    try malformed_client.writeAll(&malformed);
    var unexpected_response: [1]u8 = undefined;
    if (builtin.os.tag == .windows) {
        try std.testing.expectError(
            error.ConnectionClosed,
            malformed_client.read(&unexpected_response),
        );
    } else {
        try std.testing.expectEqual(
            @as(usize, 0),
            try malformed_client.read(&unexpected_response),
        );
    }
    malformed_client.close();
    malformed_client_open = false;
    malformed_thread.join();
    if (malformed_context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());

    var valid_context: ServeThreadContext = .{
        .server = &server,
        .service = &service,
    };
    var valid_thread = ThreadJoinGuard.init(
        try std.Thread.spawn(.{}, serveTestConnection, .{&valid_context}),
    );
    defer valid_thread.join();
    var valid_client = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var valid_client_open = true;
    defer if (valid_client_open) valid_client.close();
    try expectHandshakeAccepted(&valid_client);
    valid_client.close();
    valid_client_open = false;
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
    _ = try serviceCore(&service).connect(
        retained_connection,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    _ = try createModel(
        serviceCore(&service),
        retained_connection,
        host_core.CreationId.fromBytes([_]u8{2} ** 16),
        host_core.TerminalId.fromBytes([_]u8{3} ** 16),
        [_]u8{4} ** 32,
        80,
        24,
    );
    var transport: ResourceFailureTransport = .{};

    serveAcceptedConnection(&transport, &service);

    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
    try std.testing.expect(
        serviceCore(&service).findTerminal(host_core.TerminalId.fromBytes([_]u8{3} ** 16)) != null,
    );
}
test "failed response handoff destroys the broker but preserves retained terminals" {
    const hello_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"22222222222222222222222222222222\",\"frameBytes\":1024,\"inputBytes\":1024,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":1024,\"supportedOptionalFeatures\":[]}}",
        .{
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            protocol.current_protocol_patch,
        },
    );
    defer std.testing.allocator.free(hello_payload);
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
    _ = try serviceCore(&service).connect(
        setup_connection,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    _ = try createModel(
        serviceCore(&service),
        setup_connection,
        host_core.CreationId.fromBytes([_]u8{2} ** 16),
        host_core.TerminalId.fromBytes([_]u8{3} ** 16),
        [_]u8{4} ** 32,
        80,
        24,
    );
    _ = serviceCore(&service).disconnect(setup_connection);

    var transport: FailingWriteTransport = .{ .record = hello };
    try std.testing.expectError(
        error.InjectedWriteFailure,
        serveConnection(&transport, &service),
    );
    try std.testing.expectEqual(@as(usize, 1), transport.writes);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
}
