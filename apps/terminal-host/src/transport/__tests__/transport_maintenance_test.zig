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
const handshake = harness.handshake;
const workerOffer = harness.workerOffer;
const ServeThreadContext = harness.ServeThreadContext;
const serveTestConnection = harness.serveTestConnection;
const serveTestUntilIdle = harness.serveTestUntilIdle;

/// Counts maintenance passes: each pass reads the cleanup clock once while a terminal is retained.
const MaintenancePasses = struct {
    count: usize = 0,

    fn read(clock: *host_runtime_aggregate.testing.CleanupClockType) i128 {
        const self: *MaintenancePasses = @ptrCast(@alignCast(clock.context.?));
        self.count += 1;
        return 0;
    }
};

const WorkerStall = enum { reading, mid_send };

/// A Worker that completes its Hello and then stalls: it takes no more Host records, or it stops
/// partway through a record of its own. The Host's `exit` arrives at its `exit_at`th wait.
const StalledWorker = struct {
    input: []const u8,
    stall: WorkerStall,
    service: *host_service.HostService,
    passes: *const MaintenancePasses,
    exit_at: usize,
    offset: usize = 0,
    records_taken: usize = 0,
    waits: usize = 0,
    passes_at_first_wait: usize = 0,
    cancelled: bool = false,

    pub fn read(self: *StalledWorker, bytes: []u8) !usize {
        const length = @min(bytes.len, self.input.len - self.offset);
        @memcpy(bytes[0..length], self.input[self.offset..][0..length]);
        self.offset += length;
        return length;
    }

    pub fn waitReadable(self: *StalledWorker) !bool {
        if (self.offset < self.input.len) return true;
        self.wait();
        return false;
    }

    /// Only the handshake answer is written whole; session records go through `writeSome`.
    pub fn writeAll(_: *StalledWorker, bytes: []const u8) !void {
        if (!std.mem.startsWith(u8, bytes, handshake.frame.magic)) return error.TestUnexpectedWrite;
    }

    pub fn writeSome(self: *StalledWorker, bytes: []const u8, _: u32) !usize {
        // The Hello reply is taken; a Worker that stopped reading takes nothing after it.
        if (self.stall == .reading and self.records_taken != 0) {
            self.wait();
            return 0;
        }
        self.records_taken += 1;
        return bytes.len;
    }

    pub fn cancelPendingWrite(self: *StalledWorker) void {
        self.cancelled = true;
    }

    fn wait(self: *StalledWorker) void {
        self.waits += 1;
        if (self.waits == 1) self.passes_at_first_wait = self.passes.count;
        if (self.waits == self.exit_at) _ = self.service.applyControlRequest(.exit);
    }
};

fn expectStalledWorkerEndsOnExit(stall: WorkerStall) !void {
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    const setup_connection: host_core.ConnectionId = 9;
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
    var passes: MaintenancePasses = .{};
    host_runtime_aggregate.testing.setCleanupClockForTesting(
        serviceAggregate(&service),
        MaintenancePasses.read,
        &passes,
    );

    const offer = try handshake.encode.initiatorFrame(std.testing.allocator, .{ .offer = workerOffer() });
    defer std.testing.allocator.free(offer);
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
    const ping = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.ping,
        .control_json,
        "{}",
        [_]u8{0x5b} ** 16,
        serviceAggregate(&service).hostEpoch().bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(ping);
    const sent = switch (stall) {
        // The whole ping, whose answer the Worker never takes.
        .reading => ping,
        // Part of the ping's header, and then nothing.
        .mid_send => ping[0..30],
    };
    const input = try std.mem.concat(std.testing.allocator, u8, &.{ offer, hello, sent });
    defer std.testing.allocator.free(input);

    var worker: StalledWorker = .{
        .input = input,
        .stall = stall,
        .service = &service,
        .passes = &passes,
        .exit_at = 5,
    };
    try std.testing.expectError(error.ConnectionClosed, serveConnection(&worker, &service));
    // The session ends in the step `exit` arrived in, with no deadline involved.
    try std.testing.expectEqual(worker.exit_at, worker.waits);
    // Every wait for the stalled Worker maintained the retained terminal.
    try std.testing.expect(passes.count - worker.passes_at_first_wait >= worker.exit_at - 1);
    // Only a write left pending is cancelled, which ends the stream partway.
    try std.testing.expectEqual(stall == .reading, worker.cancelled);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
}

test "a Worker that stops reading holds its session until exit cancels the pending write, and terminals are maintained meanwhile" {
    try expectStalledWorkerEndsOnExit(.reading);
}

test "a Worker that stops partway through a record holds its session until exit, and terminals are maintained meanwhile" {
    try expectStalledWorkerEndsOnExit(.mid_send);
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

const rule_account = "S-1-5-21-1-2-3-1001";
const rule_private_dacl = "D:P(A;OICI;FA;;;" ++ rule_account ++ ")(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

fn expectPrivateRule(comptime descriptor: []const u8, comptime admitted: bool) !void {
    const answer = windows_c.ob_windows_local_test_private_descriptor(
        std.unicode.utf8ToUtf16LeStringLiteral(descriptor),
        std.unicode.utf8ToUtf16LeStringLiteral(rule_account),
    );
    if (answer != @intFromBool(admitted)) {
        std.debug.print("{s}: expected admitted={any}, got {d}\n", .{ descriptor, admitted, answer });
        return error.TestUnexpectedResult;
    }
}

// A real elevated token cannot be made in a test, so owner and DACL inputs stand in for the owners
// an ordinary session, an elevated session and an installer give a runtime directory, and for the
// accounts that must never own or reach it. The rule is the Rust runtime's private admission.
test "Windows endpoint admits a runtime directory only when the account, SYSTEM or Administrators own and reach it" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const trusted = .{ rule_account, "SY", "BA" };
    const others = .{ "S-1-5-21-1-2-3-1002", "BU", "AU", "WD" };
    inline for (trusted) |owner| {
        try expectPrivateRule("O:" ++ owner ++ rule_private_dacl, true);
        // Deny entries only take access away, and an empty DACL grants no one anything.
        try expectPrivateRule("O:" ++ owner ++ "D:P(D;;FA;;;WD)(A;;FA;;;" ++ rule_account ++ ")", true);
        try expectPrivateRule("O:" ++ owner ++ "D:P", true);
        inline for (others) |other| {
            try expectPrivateRule("O:" ++ owner ++ rule_private_dacl ++ "(A;;FA;;;" ++ other ++ ")", false);
            // An entry that only children inherit still lets that account into what the Host creates.
            try expectPrivateRule("O:" ++ owner ++ rule_private_dacl ++ "(A;OICIIO;FA;;;" ++ other ++ ")", false);
        }
        // A missing DACL grants everyone everything; an entry of another kind cannot be judged.
        try expectPrivateRule("O:" ++ owner ++ "D:NO_ACCESS_CONTROL", false);
        try expectPrivateRule("O:" ++ owner, false);
        try expectPrivateRule(
            "O:" ++ owner ++ rule_private_dacl ++ "(OA;;FA;00000000-0000-0000-0000-000000000001;;" ++ rule_account ++ ")",
            false,
        );
    }
    inline for (others) |owner| {
        try expectPrivateRule("O:" ++ owner ++ rule_private_dacl, false);
    }
}

// No elevated token exists in a test, so the levels a Host can have stand in for one: High and
// System label the pipe and mutex High, which the system parses as a descriptor; Medium Plus,
// Medium, Low and Untrusted keep the plain user DACL they had before the label existed.
test "Windows endpoint labels its pipe and mutex High only for a High or System Host" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    for ([_]u32{ 0x3000, 0x4000 }) |level| {
        try std.testing.expectEqual(@as(c_int, 1), windows_c.ob_windows_local_test_endpoint_label(level));
    }
    for ([_]u32{ 0x2100, 0x2000, 0x1000, 0 }) |level| {
        try std.testing.expectEqual(@as(c_int, 0), windows_c.ob_windows_local_test_endpoint_label(level));
    }
}
