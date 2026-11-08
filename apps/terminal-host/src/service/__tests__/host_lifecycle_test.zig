const harness = @import("service_harness.zig");
const std = harness.std;
const protocol = harness.protocol;
const host_lifecycle = harness.service_boundary.host_lifecycle;
const ConnectionState = harness.ConnectionState;
const HostService = harness.HostService;
const MessageKind = harness.MessageKind;
const expectSingleFailureCode = harness.expectSingleFailureCode;
const identifierText = harness.identifierText;
const serviceCore = harness.serviceCore;
const testCreatePayload = harness.testCreatePayload;
const testHandshake = harness.testHandshake;
const testRequest = harness.testRequest;
const testTerminalTargetPayload = harness.testTerminalTargetPayload;

const State = host_lifecycle.State;

test "control requests only move the Host lifecycle forward" {
    const cases = [_]struct { from: State, request: host_lifecycle.Request, to: State }{
        .{ .from = .ready, .request = .drain, .to = .draining },
        .{ .from = .draining, .request = .drain, .to = .draining },
        .{ .from = .stopping, .request = .drain, .to = .stopping },
        .{ .from = .ready, .request = .exit, .to = .stopping },
        .{ .from = .draining, .request = .exit, .to = .stopping },
        .{ .from = .stopping, .request = .exit, .to = .stopping },
    };
    for (cases) |case| {
        const next = host_lifecycle.transition(case.from, case.request);
        try std.testing.expectEqual(case.to, next);
        // A request is in effect after it is applied, and applying it again changes nothing.
        try std.testing.expect(host_lifecycle.applied(next, case.request));
        try std.testing.expectEqual(next, host_lifecycle.transition(next, case.request));
        try std.testing.expectEqual(case.from == case.to, host_lifecycle.applied(case.from, case.request));
    }
}

test "only a ready Host admits terminals, and a request ends the Host once it retains none" {
    try std.testing.expectEqual(host_lifecycle.TerminalAdmission.open, host_lifecycle.terminalAdmission(.ready));
    try std.testing.expectEqual(host_lifecycle.TerminalAdmission.closed, host_lifecycle.terminalAdmission(.draining));
    try std.testing.expectEqual(host_lifecycle.TerminalAdmission.closed, host_lifecycle.terminalAdmission(.stopping));
    for ([_]usize{ 0, 1 }) |retained| {
        try std.testing.expect(!host_lifecycle.endsNow(.ready, retained));
        try std.testing.expectEqual(retained == 0, host_lifecycle.endsNow(.draining, retained));
        try std.testing.expect(host_lifecycle.endsNow(.stopping, retained));
    }
}

/// A `CreateTerminal` without launch geometry: the Host retains it awaiting start and spawns
/// nothing.
fn deferredCreatePayload(creation_id: []const u8) ![]u8 {
    const immediate = try testCreatePayload(creation_id, "orchbee-terminal-host-deliberately-missing-executable", &.{});
    defer std.testing.allocator.free(immediate);
    var parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, immediate, .{});
    defer parsed.deinit();
    const launch = &parsed.value.object.getPtr("launch").?.object;
    for ([_][]const u8{ "columns", "rows", "cellPixelWidth", "cellPixelHeight" }) |key| {
        try launch.put(parsed.arena.allocator(), key, .null);
    }
    return std.json.Stringify.valueAlloc(std.testing.allocator, parsed.value, .{});
}

fn createTerminal(
    service: *HostService,
    connection: *ConnectionState,
    payload: []const u8,
    correlation: u8,
) !harness.Outgoing {
    return testRequest(service, connection, MessageKind.create_terminal, .control_json, payload, [_]u8{correlation} ** 16);
}

test "a drained Host refuses new terminals, answers retained creations, and ends with its last terminal" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    const retained = try deferredCreatePayload("33333333333333333333333333333333");
    defer std.testing.allocator.free(retained);
    var created = try createTerminal(&service, &connection, retained, 0x71);
    defer created.deinit();
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
    const terminal_id = serviceCore(&service).terminalKeys()[0];

    try std.testing.expectEqual(State.draining, service.applyControlRequest(.drain));
    try std.testing.expectEqual(State.draining, service.candidateFacts().state);
    try std.testing.expect(!service.controlExitDue());

    const another = try deferredCreatePayload("44444444444444444444444444444444");
    defer std.testing.allocator.free(another);
    var refused = try createTerminal(&service, &connection, another, 0x72);
    defer refused.deinit();
    try expectSingleFailureCode(&refused, "host-draining");
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());

    var replayed = try createTerminal(&service, &connection, retained, 0x73);
    defer replayed.deinit();
    try std.testing.expectEqual(@as(usize, 1), replayed.records().len);
    const replay = try protocol.validateTransportEnvelope(replayed.records()[0]);
    try std.testing.expectEqual(MessageKind.create_terminal, replay.kind);
    var terminal_text: [32]u8 = undefined;
    try std.testing.expect(std.mem.indexOf(u8, replay.payload, identifierText(terminal_id, &terminal_text)) != null);
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());

    const target = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(target);
    var disposed = try testRequest(&service, &connection, MessageKind.dispose_terminal, .control_json, target, [_]u8{0x74} ** 16);
    defer disposed.deinit();
    try std.testing.expectEqual(MessageKind.dispose_terminal, (try protocol.validateTransportEnvelope(disposed.records()[0])).kind);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    try std.testing.expect(service.controlExitDue());
}

test "exit ends the Host while it still retains terminals, and a later drain keeps it stopping" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try testHandshake(&service, &connection, "22222222222222222222222222222222");
    const retained = try deferredCreatePayload("33333333333333333333333333333333");
    defer std.testing.allocator.free(retained);
    var created = try createTerminal(&service, &connection, retained, 0x75);
    defer created.deinit();

    try std.testing.expectEqual(State.stopping, service.applyControlRequest(.exit));
    try std.testing.expectEqual(State.stopping, service.applyControlRequest(.drain));
    try std.testing.expectEqual(State.stopping, service.candidateFacts().state);
    try std.testing.expect(service.controlExitDue());
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
}
