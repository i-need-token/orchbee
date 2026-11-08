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
const serveTestUntilIdle = harness.serveTestUntilIdle;
const workerOffer = harness.workerOffer;
const protocol = harness.protocol;
const readRecord = harness.readRecord;
const host_lifecycle = harness.service_boundary.host_lifecycle;

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

        pub fn controlExitDue(_: *const @This()) bool {
            return false;
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

/// Serves one scripted candidate on the dispatch thread, with no Worker session, and decodes the
/// Host's reply into `arena`.
fn serveScripted(
    arena: std.mem.Allocator,
    service: *host_service.HostService,
    initiator: message.InitiatorMessage,
) !message.AcceptorMessage {
    const frame = try handshake.encode.initiatorFrame(std.testing.allocator, initiator);
    defer std.testing.allocator.free(frame);
    var transport: ScriptedTransport = .{ .input = frame };
    defer transport.deinit();
    try serveConnection(&transport, service);
    return handshake.decode.acceptorFrame(arena, try arena.dupe(u8, transport.output.items));
}

fn controlRequest(kind: message.RequestKind) message.InitiatorMessage {
    return .{ .request = .{ .kind = kind, .environment = candidate_admission.environment } };
}

test "drain and exit before the Worker arrives take effect on the dispatch thread" {
    var service = host_service.HostService.init(std.testing.allocator);
    defer service.deinit();
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();

    const drained = try serveScripted(arena, &service, controlRequest(.drain));
    try std.testing.expect(drained == .drain_result);
    try std.testing.expectEqual(message.ProcessState.draining, drained.drain_result.state);
    // A drained Host that retains no terminal ends without waiting for a Worker.
    try std.testing.expect(service.controlExitDue());

    const offered = try serveScripted(arena, &service, .{ .offer = workerOffer() });
    try std.testing.expect(offered == .rejection);
    try std.testing.expectEqual(message.RejectionReason.stopping, offered.rejection.reason);

    const exited = try serveScripted(arena, &service, controlRequest(.exit));
    try std.testing.expect(exited == .exit_result);
    const queried = try serveScripted(arena, &service, controlRequest(.query));
    try std.testing.expectEqual(message.ProcessState.stopping, queried.query_result.processes[0].state);
}

/// Sends one control request on its own connection while the Worker session holds the dispatch
/// thread.
fn requestOnControlConnection(
    arena: std.mem.Allocator,
    server: *LocalServer,
    endpoint_id: []const u8,
    kind: message.RequestKind,
) !message.AcceptorMessage {
    var client = try TestClient.connect(server, std.testing.allocator, endpoint_id);
    defer client.close();
    return exchangeCandidate(arena, &client, controlRequest(kind));
}

/// Sends one OBTP request on the Worker connection and returns its correlated response, skipping
/// the events the Host interleaves.
fn workerRequest(
    worker: *TestClient,
    epoch: [16]u8,
    kind: u16,
    payload: []const u8,
    correlation: u8,
) ![]u8 {
    const correlation_id = [_]u8{correlation} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        kind,
        .control_json,
        payload,
        correlation_id,
        epoch,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    try worker.writeAll(record);
    for (0..64) |_| {
        const response = try readRecord(std.testing.allocator, worker);
        errdefer std.testing.allocator.free(response);
        const envelope = try protocol.validateTransportEnvelope(response);
        if (envelope.response and envelope.correlation_id != null and
            std.mem.eql(u8, &envelope.correlation_id.?, &correlation_id))
        {
            return response;
        }
        std.testing.allocator.free(response);
    }
    return error.TestResponseMissing;
}

/// A terminal without launch geometry: the Host retains it awaiting start and spawns nothing.
fn deferredCreatePayload(creation_id: []const u8) ![]u8 {
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    const no_entries = [_]u16{};
    return std.json.Stringify.valueAlloc(std.testing.allocator, .{
        .creationId = creation_id,
        .environmentOverlay = &no_entries,
        .launch = .{
            .arguments = &[_][]const u8{},
            .cellPixelHeight = @as(?u16, null),
            .cellPixelWidth = @as(?u16, null),
            .columns = @as(?u16, null),
            .configuration = @as(?u8, null),
            .currentDirectory = @as(?[]const u8, null),
            .environment = &no_entries,
            .executable = "orchbee-terminal-host-deliberately-missing-executable",
            .rows = @as(?u16, null),
            .terminalProfile = .{
                .colorScheme = "dark",
                .defaultBackgroundRgb = 0,
                .defaultCursorRgb = @as(?u32, null),
                .defaultForegroundRgb = 0xffffff,
                .deviceAttributes = .{
                    .primaryConformanceLevel = 62,
                    .primaryFeatures = &features,
                    .secondaryDeviceType = 1,
                    .secondaryFirmwareVersion = 0,
                    .secondaryRomCartridge = 0,
                    .tertiaryUnitId = 0,
                },
                .enquiryResponse = "",
                .palette = &palette,
                .versionResponse = "Orchbee test",
            },
        },
    }, .{});
}

/// Creates a retained terminal over the Worker connection and returns its `terminalId` text,
/// allocated in `arena`.
fn createDeferredTerminal(
    arena: std.mem.Allocator,
    worker: *TestClient,
    epoch: [16]u8,
    creation_id: []const u8,
    correlation: u8,
) ![]const u8 {
    const payload = try deferredCreatePayload(creation_id);
    defer std.testing.allocator.free(payload);
    const response = try workerRequest(worker, epoch, protocol.MessageKind.create_terminal, payload, correlation);
    defer std.testing.allocator.free(response);
    const envelope = try protocol.validateTransportEnvelope(response);
    try std.testing.expectEqual(protocol.MessageKind.create_terminal, envelope.kind);
    const identity = try std.json.parseFromSliceLeaky(
        struct { terminalId: []const u8 },
        arena,
        envelope.payload,
        .{ .ignore_unknown_fields = true, .allocate = .alloc_always },
    );
    return identity.terminalId;
}

/// Reads until the Host closes the Worker connection, which the test itself keeps open.
fn expectHostClosed(worker: *TestClient) !void {
    for (0..64) |_| {
        const record = readRecord(std.testing.allocator, worker) catch return;
        std.testing.allocator.free(record);
    }
    return error.TestHostKeptConnection;
}

/// A Host running its serve loop on a test thread, as its dispatch thread does, with one admitted
/// Worker.
const LiveSession = struct {
    runtime: TestRuntimeDirectory,
    server: LocalServer,
    service: host_service.HostService,
    context: ServeThreadContext,
    thread: ?std.Thread,
    worker: TestClient,
    worker_open: bool,
    epoch: [16]u8,

    const endpoint_id = "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd";

    /// Admits the Worker; `self` must stay in place until `deinit`.
    fn start(self: *LiveSession) !void {
        self.runtime = try TestRuntimeDirectory.init(std.testing.allocator);
        errdefer self.runtime.deinit();
        self.server = try LocalServer.init(std.testing.allocator, self.runtime.path(), endpoint_id);
        errdefer self.server.deinit();
        self.service = host_service.HostService.init(std.testing.allocator);
        errdefer self.service.deinit();
        self.epoch = serviceCore(&self.service).epoch.bytes;
        self.context = .{ .server = &self.server, .service = &self.service };
        self.thread = try std.Thread.spawn(.{}, serveTestUntilIdle, .{&self.context});
        errdefer self.joinHost() catch {};
        self.worker = try TestClient.connect(&self.server, std.testing.allocator, endpoint_id);
        self.worker_open = true;
        errdefer self.worker.close();
        try expectHandshakeAccepted(&self.worker);
    }

    /// Waits for the serve loop to return, as the Host process does before its teardown.
    fn joinHost(self: *LiveSession) !void {
        if (self.thread) |thread| thread.join();
        self.thread = null;
        if (self.context.failure) |failure| return failure;
    }

    fn deinit(self: *LiveSession) void {
        if (self.worker_open) self.worker.close();
        if (self.thread) |thread| thread.join();
        self.service.deinit();
        self.server.deinit();
        self.runtime.deinit();
    }
};

test "a drain on a control connection closes terminal admission and ends the Host with its last terminal" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var live: LiveSession = undefined;
    try live.start();
    defer live.deinit();
    const terminal_id = try createDeferredTerminal(arena, &live.worker, live.epoch, "33333333333333333333333333333333", 0x81);

    const drained = try requestOnControlConnection(arena, &live.server, LiveSession.endpoint_id, .drain);
    try std.testing.expect(drained == .drain_result);
    try std.testing.expectEqual(message.ProcessState.draining, drained.drain_result.state);

    // The reply waited for the dispatch thread to apply the drain, so the Worker's next creation
    // is already refused.
    const another = try deferredCreatePayload("44444444444444444444444444444444");
    defer std.testing.allocator.free(another);
    const refused = try workerRequest(&live.worker, live.epoch, protocol.MessageKind.create_terminal, another, 0x82);
    defer std.testing.allocator.free(refused);
    const failure = try protocol.validateTransportEnvelope(refused);
    try std.testing.expectEqual(protocol.MessageKind.typed_failure, failure.kind);
    try std.testing.expect(std.mem.indexOf(u8, failure.payload, "\"host-draining\"") != null);

    const queried = try requestOnControlConnection(arena, &live.server, LiveSession.endpoint_id, .query);
    try std.testing.expectEqual(message.ProcessState.draining, queried.query_result.processes[0].state);

    const target = try std.fmt.allocPrint(arena, "{{\"terminalId\":\"{s}\"}}", .{terminal_id});
    const disposed = try workerRequest(&live.worker, live.epoch, protocol.MessageKind.dispose_terminal, target, 0x83);
    defer std.testing.allocator.free(disposed);
    try std.testing.expectEqual(protocol.MessageKind.dispose_terminal, (try protocol.validateTransportEnvelope(disposed)).kind);

    // With its last terminal gone, the Host ends the session the Worker keeps open, and then
    // ends itself instead of accepting again.
    try expectHostClosed(&live.worker);
    try live.joinHost();
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&live.service).terminalCount());
    try std.testing.expectEqual(host_lifecycle.State.draining, live.service.candidateFacts().state);
    try std.testing.expect(live.service.controlExitDue());
}

test "an exit on a control connection ends the Worker session and the Host while terminals remain" {
    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    var live: LiveSession = undefined;
    try live.start();
    defer live.deinit();
    _ = try createDeferredTerminal(arena, &live.worker, live.epoch, "33333333333333333333333333333333", 0x84);

    const exited = try requestOnControlConnection(arena, &live.server, LiveSession.endpoint_id, .exit);
    try std.testing.expect(exited == .exit_result);

    try expectHostClosed(&live.worker);
    try live.joinHost();
    // The Host ends with the terminal still retained; its teardown, here `deinit`, ends it.
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&live.service).terminalCount());
    try std.testing.expectEqual(host_lifecycle.State.stopping, live.service.candidateFacts().state);
    try std.testing.expect(live.service.controlExitDue());
}
