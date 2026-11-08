const harness = @import("transport_harness.zig");
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
const Served = harness.Served;
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

        pub fn serveOne(self: *@This(), _: *host_service.HostService) !Served {
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
test "THDD-001 PeerIdentityMismatch accept failure stays on the recoverable path without retained-cleanup escalation" {
    const PeerMismatchServer = struct {
        calls: usize = 0,

        pub fn serveOne(self: *@This(), _: *host_service.HostService) !Served {
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

        pub fn controlExitDue(_: *const @This()) bool {
            return false;
        }
    };
    const TimeoutServer = struct {
        calls: usize = 0,

        pub fn serveOne(self: *@This(), _: *MaintenanceService) !Served {
            self.calls += 1;
            return .worker;
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
