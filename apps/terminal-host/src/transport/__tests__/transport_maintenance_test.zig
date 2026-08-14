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
    const core = serviceCore(&service);
    const connection_id: host_core.ConnectionId = 7;
    _ = try core.connect(
        connection_id,
        host_core.ClientId.fromBytes([_]u8{1} ** 16),
    );
    const first_terminal_id = host_core.TerminalId.fromBytes([_]u8{2} ** 16);
    const second_terminal_id = host_core.TerminalId.fromBytes([_]u8{3} ** 16);
    for ([_]host_core.TerminalId{ first_terminal_id, second_terminal_id }, 0..) |terminal_id, index| {
        _ = try createModel(
            core,
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
        host_runtime_aggregate.testing.cleanupRetryScanIndexFn(serviceAggregate(&service)),
    );
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
