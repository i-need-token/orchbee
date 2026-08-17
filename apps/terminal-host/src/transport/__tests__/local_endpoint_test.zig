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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());

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
    const hello_payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":1024,"protocol":{"major":0,"minor":0,"patch":9},"rowWindowBytes":1024,"supportedOptionalFeatures":[]}
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
