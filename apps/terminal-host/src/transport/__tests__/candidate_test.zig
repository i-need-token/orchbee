const harness = @import("transport_harness.zig");
const std = harness.std;
const builtin = harness.builtin;
const candidate_admission = harness.candidate_admission;
const handshake = harness.handshake;
const host_service = harness.host_service;
const LocalServer = harness.LocalServer;
const Served = harness.Served;
const ServeThreadContext = harness.ServeThreadContext;
const TestClient = harness.TestClient;
const TestRuntimeDirectory = harness.TestRuntimeDirectory;
const exchangeCandidate = harness.exchangeCandidate;
const expectHandshakeAccepted = harness.expectHandshakeAccepted;
const serveConnection = harness.serveConnection;
const serveTestConnection = harness.serveTestConnection;
const serveUntilIdleLoop = harness.serveUntilIdleLoop;
const serviceCore = harness.serviceCore;
const workerOffer = harness.workerOffer;

const message = handshake.message;

/// Feeds one prepared input, then behaves as a peer that closed, and keeps every byte the Host
/// writes.
const ScriptedTransport = struct {
    input: []const u8,
    offset: usize = 0,
    output: std.ArrayList(u8) = .empty,

    fn deinit(self: *ScriptedTransport) void {
        self.output.deinit(std.testing.allocator);
    }

    pub fn read(self: *ScriptedTransport, bytes: []u8) !usize {
        const length = @min(bytes.len, self.input.len - self.offset);
        @memcpy(bytes[0..length], self.input[self.offset..][0..length]);
        self.offset += length;
        return length;
    }

    pub fn waitReadable(self: *ScriptedTransport) !bool {
        if (self.offset < self.input.len) return true;
        return error.ConnectionClosed;
    }

    pub fn writeAll(self: *ScriptedTransport, bytes: []const u8) !void {
        try self.output.appendSlice(std.testing.allocator, bytes);
    }
};

fn expectOfferRefused(offer: message.Offer, reason: message.RejectionReason) !void {
    const frame = try handshake.encode.initiatorFrame(std.testing.allocator, .{ .offer = offer });
    defer std.testing.allocator.free(frame);
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var transport: ScriptedTransport = .{ .input = frame };
    defer transport.deinit();

    try serveConnection(&transport, &service);

    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const reply = try handshake.decode.acceptorFrame(arena.allocator(), transport.output.items);
    try std.testing.expect(reply == .rejection);
    try std.testing.expectEqual(reason, reply.rejection.reason);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
}

test "the Host refuses an offer from another environment or another protocol build" {
    var foreign_environment = workerOffer();
    foreign_environment.environment = "ob-test-0000000000000000";
    foreign_environment.fingerprint = "0" ** 64;
    if (std.mem.eql(u8, candidate_admission.environment, foreign_environment.environment)) {
        foreign_environment.environment = "ob-test-ffffffffffffffff";
    }
    try expectOfferRefused(foreign_environment, .environment_mismatch);

    const own = workerOffer();
    const foreign_fingerprint = if (std.mem.eql(u8, own.fingerprint orelse "", "0" ** 64))
        "1" ** 64
    else
        "0" ** 64;
    if (own.fingerprint != null) {
        var foreign_build = own;
        foreign_build.fingerprint = foreign_fingerprint;
        try expectOfferRefused(foreign_build, .fingerprint_mismatch);
    }
}

test "a control request before the Worker arrives keeps the Host waiting for it" {
    const IdleService = struct {
        idle: bool = false,

        pub fn maintain(self: *@This()) void {
            self.idle = true;
        }

        pub fn canStopWhenIdle(self: *const @This()) bool {
            return self.idle;
        }
    };
    const QueryFirstServer = struct {
        accepted: usize = 0,

        pub fn serveOne(self: *@This(), _: *IdleService) !Served {
            self.accepted += 1;
            return if (self.accepted == 1) .control_request else .worker;
        }

        pub fn serveOneBefore(_: *@This(), _: *IdleService, _: u32) !bool {
            return false;
        }
    };
    const never_recoverable = struct {
        fn check(_: anyerror) bool {
            return false;
        }
    }.check;

    var service: IdleService = .{};
    var server: QueryFirstServer = .{};
    try serveUntilIdleLoop(&server, &service, never_recoverable);
    try std.testing.expectEqual(@as(usize, 2), server.accepted);
}

test "control connections are answered while a Worker session is live" {
    var runtime = try TestRuntimeDirectory.init(std.testing.allocator);
    defer runtime.deinit();
    const endpoint_id = "abababababababababababababababab";
    var server = try LocalServer.init(std.testing.allocator, runtime.path(), endpoint_id);
    defer server.deinit();
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var context: ServeThreadContext = .{ .server = &server, .service = &service };
    var server_thread: ?std.Thread = try std.Thread.spawn(.{}, serveTestConnection, .{&context});
    defer if (server_thread) |thread| thread.join();

    var worker = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
    var worker_open = true;
    defer if (worker_open) worker.close();
    try expectHandshakeAccepted(&worker);

    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    {
        var second = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
        defer second.close();
        const reply = try exchangeCandidate(arena.allocator(), &second, .{ .offer = workerOffer() });
        try std.testing.expect(reply == .rejection);
        try std.testing.expectEqual(message.RejectionReason.capacity, reply.rejection.reason);
    }
    {
        var query = try TestClient.connect(&server, std.testing.allocator, endpoint_id);
        defer query.close();
        const reply = try exchangeCandidate(arena.allocator(), &query, .{ .request = .{
            .kind = .query,
            .environment = candidate_admission.environment,
        } });
        try std.testing.expect(reply == .query_result);
        const processes = reply.query_result.processes;
        try std.testing.expectEqual(@as(usize, 1), processes.len);
        try std.testing.expectEqual(message.ProcessRole.terminal_host, processes[0].role);
        try std.testing.expectEqualStrings(candidate_admission.product_version, processes[0].product_version);
        try std.testing.expectEqualStrings(candidate_admission.environment, processes[0].environment);
        try std.testing.expectEqual(message.ProcessState.ready, processes[0].state);
    }

    worker.close();
    worker_open = false;
    server_thread.?.join();
    server_thread = null;
    if (context.failure) |failure| return failure;
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
}
