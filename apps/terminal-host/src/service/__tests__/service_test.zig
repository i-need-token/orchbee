const std = @import("std");
const target_builtin = @import("builtin");
const control_boundary = @import("terminal-host-control");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const model_boundary = @import("terminal-host-model");
const platform_boundary = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");
const subject = service_boundary.service;
const control_decode = control_boundary.decode;
const control_wire = control_boundary.wire;
const core_module = core_boundary.state;
const pty_process = platform_boundary.process;
const terminal_configuration = model_boundary.configuration;
const terminal_history = engine_boundary.history;
const terminal_runtime = engine_boundary.runtime;
const host_outgoing = service_boundary.testing.outgoing;

const WireIdentifier = [16]u8;

const ConnectionState = subject.ConnectionState;
const HostService = subject.HostService;
const Outgoing = host_outgoing.Outgoing;
const EnvironmentWire = control_wire.Environment;
const InputFailure = subject.testing.InputFailureType;
const InventoryCursorWire = control_wire.InventoryCursor;
const MessageKind = protocol.MessageKind;
const OutcomePublicationHandoff = subject.testing.OutcomePublicationHandoffType;
const TerminalConfigurationWire = control_wire.TerminalConfiguration;
const beginShutdown = subject.testing.beginShutdownFn;
const commitOutcomePublicationHandoff = subject.testing.commitOutcomePublicationHandoffFn;
const identifierText = subject.testing.identifierTextFn;
const inputFailure = subject.testing.inputFailureFn;
const protocolErrorCode = subject.testing.protocolErrorCodeFn;
const scheduleEgress = subject.testing.scheduleEgressFn;
const synchronizeRuntimeLifecycle = subject.testing.synchronizeRuntimeLifecycleFn;

fn findRuntime(
    service: *HostService,
    terminal_id: core_module.TerminalId,
) ?*terminal_runtime.TerminalRuntime {
    return service.runtime_aggregate.runtimeFor(terminal_id);
}

fn activeConnection(connection: *const ConnectionState) !*const ConnectionState.Active {
    return switch (connection.*) {
        .active => |*active| active,
        .awaiting_hello, .closing => error.ExpectedActiveConnection,
    };
}

fn expectSingleFailureCode(outgoing: *const Outgoing, expected_code: []const u8) !void {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, envelope.kind);
    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const failure = try control_wire.decode(Failure, std.testing.allocator, envelope.payload);
    defer failure.deinit();
    try std.testing.expectEqualStrings(expected_code, failure.value.code);
}

fn testClientId() core_module.ClientId {
    return core_module.ClientId.fromBytes([_]u8{0x22} ** 16);
}

fn testActiveConnectionState() ConnectionState {
    return .{ .active = .{
        .connection_id = 1,
        .limits = .{
            .frame_bytes = 1024,
            .input_bytes = 1024,
            .row_window_bytes = 1024,
        },
    } };
}

fn retainFailedTerminal(
    service: *HostService,
    connection_id: core_module.ConnectionId,
    namespace: u8,
) !core_module.TerminalId {
    const terminal_id = testId(core_module.TerminalId, namespace, 0);
    _ = try service.runtime_aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, namespace, 1),
        terminal_id,
        [_]u8{namespace} ** 32,
        80,
        24,
    );
    _ = try service.runtime_aggregate.core().synchronizeRevision(terminal_id, 7);
    _ = try service.runtime_aggregate.core().runtimeFailed(terminal_id, .read_output);
    _ = try service.runtime_aggregate.core().failureContained(
        terminal_id,
        .{ .observed = .{ .exit_code = 9 } },
    );
    return terminal_id;
}

fn fillOutgoingLeaving(
    outgoing: *Outgoing,
    host_epoch: core_module.HostEpoch,
    remaining_bytes: usize,
) !void {
    const header_bytes = protocol.transport_envelope_header_bytes;
    const first_payload_bytes = protocol.Limits.frame_bytes;
    const first_record_bytes = header_bytes + first_payload_bytes;
    const second_payload_bytes = host_outgoing.testing.byte_limit -
        remaining_bytes - first_record_bytes - header_bytes;
    const payload = try std.testing.allocator.alloc(u8, first_payload_bytes);
    defer std.testing.allocator.free(payload);
    @memset(payload, 0);
    switch (try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        payload,
        .{ .event = host_epoch },
    )) {
        .appended => {},
        .full => return error.ExpectedFillerAppend,
    }
    switch (try outgoing.append(
        MessageKind.snapshot_frame,
        .terminal_frame,
        payload[0..second_payload_bytes],
        .{ .event = host_epoch },
    )) {
        .appended => {},
        .full => return error.ExpectedFillerAppend,
    }
}

test "HostService rejects dispatch after shutdown begins" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;

    try std.testing.expect(beginShutdown(&service));
    try std.testing.expect(!beginShutdown(&service));
    try std.testing.expectError(
        error.ServiceShuttingDown,
        service.pollEvents(&connection),
    );
    try std.testing.expectError(
        error.ServiceShuttingDown,
        service.handle(&connection, &[_]u8{}),
    );
}

test "dispose cleanup failures preserve their exact owner phase" {
    const cleanupFailureDetails = subject.testing.cleanupFailureDetailsFn;
    const cases = [_]struct {
        failure: core_module.RuntimeCleanupFailure,
        code: []const u8,
    }{
        .{ .failure = .contain_process, .code = "cleanup-contain-process" },
        .{ .failure = .release_output, .code = "cleanup-release-output" },
        .{ .failure = .release_process_identity, .code = "cleanup-release-process-identity" },
    };
    for (cases) |case| {
        const details = cleanupFailureDetails(case.failure);
        try std.testing.expectEqualStrings(case.code, details.code);
        try std.testing.expect(details.retryable);
    }
}

test "failed terminal outcome is emitted once with typed process evidence" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "31313131313131313131313131313131");
    const active = try activeConnection(&connection);
    const terminal_id = try retainFailedTerminal(&service, active.connection_id, 0x31);

    var outgoing = try service.pollEvents(&connection);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_failed, envelope.kind);
    const parsed = try std.json.parseFromSlice(
        struct {
            failure: struct {
                kind: []const u8,
                stage: []const u8,
            },
            finalRevision: []const u8,
            process: struct {
                kind: []const u8,
                result: struct { code: u32, kind: []const u8 },
            },
        },
        std.testing.allocator,
        envelope.payload,
        .{ .ignore_unknown_fields = true },
    );
    defer parsed.deinit();
    try std.testing.expectEqualStrings("pty-read", parsed.value.failure.kind);
    try std.testing.expectEqualStrings("read-output", parsed.value.failure.stage);
    try std.testing.expectEqualStrings("7", parsed.value.finalRevision);
    try std.testing.expectEqualStrings("observed", parsed.value.process.kind);
    try std.testing.expectEqual(@as(u32, 9), parsed.value.process.result.code);
    try std.testing.expectEqualStrings("exit-code", parsed.value.process.result.kind);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id).?.pendingOutcome() == null);

    var duplicate = try service.pollEvents(&connection);
    defer duplicate.deinit();
    try std.testing.expectEqual(@as(usize, 0), duplicate.records().len);
}

test "full outgoing batch leaves a failed outcome pending for the next handoff" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id: core_module.ConnectionId = 1;
    _ = try service.runtime_aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x32, 0),
    );
    const terminal_id = try retainFailedTerminal(&service, connection_id, 0x32);

    var full = Outgoing.init(std.testing.allocator);
    defer full.deinit();
    for (0..host_outgoing.testing.record_limit) |_| {
        switch (try full.append(
            MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = service.runtime_aggregate.core().epoch },
        )) {
            .appended => {},
            .full => return error.ExpectedFillerAppend,
        }
    }
    var blocked_handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &full, &blocked_handoff);
    commitOutcomePublicationHandoff(&blocked_handoff, service.runtime_aggregate.core());
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id).?.pendingOutcome() != null);

    var retry = Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    var retry_handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &retry, &retry_handoff);
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    try std.testing.expectEqual(
        MessageKind.terminal_failed,
        (try protocol.validateTransportEnvelope(retry.records()[0])).kind,
    );
    commitOutcomePublicationHandoff(&retry_handoff, service.runtime_aggregate.core());
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id).?.pendingOutcome() == null);
}

test "settled outcome handoff publishes once and rejects revision advancement" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id: core_module.ConnectionId = 1;
    _ = try service.runtime_aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x33, 0),
    );
    const terminal_id = testId(core_module.TerminalId, 0x33, 1);
    _ = try service.runtime_aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0x33, 2),
        terminal_id,
        [_]u8{0x33} ** 32,
        80,
        24,
    );
    _ = try service.runtime_aggregate.core().synchronizeRevision(terminal_id, 1);
    _ = try service.runtime_aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try service.runtime_aggregate.core().ptyDrained(terminal_id);

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &outgoing, &handoff);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expectError(
        error.InvalidPhase,
        service.runtime_aggregate.core().synchronizeRevision(terminal_id, 2),
    );
    commitOutcomePublicationHandoff(&handoff, service.runtime_aggregate.core());
    commitOutcomePublicationHandoff(&handoff, service.runtime_aggregate.core());
    const terminal = service.runtime_aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(@as(u64, 1), terminal.settledFinalRevision().?);
    try std.testing.expect(terminal.pendingOutcome() == null);
}

test "full projection batches cannot starve interaction scheduling" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const arguments = [_][]const u8{ "-c", "read value" };
    const create_payload = try testCreatePayload(
        "41414141414141414141414141414141",
        "/bin/sh",
        &arguments,
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "42424242424242424242424242424242");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x43} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":false,\"read\":true,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x45} ** 16,
    );
    defer attached.deinit();

    const active = try activeConnection(&connection);
    const runtime = findRuntime(&service, terminal_id).?;
    _ = try service.runtime_aggregate.core().setProjectionDemand(
        active.connection_id,
        attachment_id,
        .active,
    );
    try runtime.setHistoryDemand(attachment_id, .viewing_history);
    service.next_egress_priority = .projection;
    try std.testing.expect(service.runtime_aggregate.setNextProjectionAt(terminal_id, 0));

    {
        var projection_first = Outgoing.init(std.testing.allocator);
        defer projection_first.deinit();
        try fillOutgoingLeaving(&projection_first, service.runtime_aggregate.core().epoch, 512);
        try scheduleEgress(&service, active, &projection_first);
        try std.testing.expectEqual(@as(usize, 2), projection_first.records().len);
    }
    {
        var interaction_first = Outgoing.init(std.testing.allocator);
        defer interaction_first.deinit();
        try fillOutgoingLeaving(&interaction_first, service.runtime_aggregate.core().epoch, 512);
        try scheduleEgress(&service, active, &interaction_first);
        try std.testing.expectEqual(@as(usize, 3), interaction_first.records().len);
        const interaction = try protocol.validateTransportEnvelope(
            interaction_first.records()[2],
        );
        try std.testing.expectEqual(MessageKind.history_changed, interaction.kind);
    }
}

test "input failure metadata is consistent across raw and semantic ingress" {
    try std.testing.expectEqualDeep(
        InputFailure{ .code = "input-backpressure", .retryable = true },
        inputFailure(.backpressure),
    );
    try std.testing.expectEqualDeep(
        InputFailure{ .code = "resource-exhausted", .retryable = true },
        inputFailure(.resource_exhausted),
    );
    try std.testing.expectEqualDeep(
        InputFailure{ .code = "invalid-phase", .retryable = false },
        inputFailure(.terminal_exited),
    );
    try std.testing.expectEqualDeep(
        InputFailure{ .code = "invalid-field", .retryable = false },
        inputFailure(.invalid_input),
    );
}

test "protocol identifier failures retain their public error code" {
    try std.testing.expectEqualStrings(
        "invalid-identifier",
        protocolErrorCode(error.InvalidIdentifier),
    );
}

test "handshake binds client and negotiates input bytes" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    const payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":2048,"protocol":{"major":0,"minor":0,"patch":2},"rowWindowBytes":4096,"supportedOptionalFeatures":[]}
    ;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.hello_accepted, response.kind);
    try std.testing.expect(
        std.mem.indexOf(u8, response.payload, "\"hostVersion\":\"0.1.0\"") != null,
    );
    const active = try activeConnection(&connection);
    try std.testing.expect(service.runtime_aggregate.core().connectionClientId(active.connection_id).?.eql(testClientId()));
    try std.testing.expectEqual(@as(usize, 2048), active.limits.input_bytes);
    service.disconnect(&connection);
}

test "handshake rejects a different exact protocol without compatibility negotiation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    const unsupported_patch = protocol.current_protocol_patch + 1;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"22222222222222222222222222222222\",\"frameBytes\":1024,\"inputBytes\":2048,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            unsupported_patch,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    std.mem.writeInt(u16, record[56..58], unsupported_patch, .little);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.protocol_rejected, response.kind);
    try std.testing.expect(std.mem.indexOf(u8, response.payload, "\"reason\":\"version-mismatch\"") != null);
    try std.testing.expect(connection.shouldClose());
}

test "handshake rejects an unsupported envelope version before parsing its payload" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        "{\"futureHelloShape\":true}",
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    const unsupported_patch = protocol.current_protocol_patch + 1;
    std.mem.writeInt(u16, record[56..58], unsupported_patch, .little);

    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.protocol_rejected, response.kind);
    try std.testing.expect(
        std.mem.indexOf(u8, response.payload, "\"reason\":\"version-mismatch\"") != null,
    );
    try std.testing.expect(connection.shouldClose());
}

test "pre-handshake role failures do not publish the HostEpoch" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        "{}",
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    std.mem.writeInt(u16, record[6..8], 2, .little);

    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, response.kind);
    try std.testing.expect(response.host_epoch == null);
    try std.testing.expect(connection.shouldClose());
}

test "active control decode failures retain distinct public codes" {
    const invalid_utf8 = [_]u8{0xff};
    const cases = [_]struct {
        code: []const u8,
        payload: []const u8,
    }{
        .{ .code = "invalid-utf8", .payload = &invalid_utf8 },
        .{ .code = "invalid-json", .payload = "{" },
        .{ .code = "invalid-field", .payload = "{\"extra\":true}" },
    };

    for (cases) |case| {
        var service = HostService.init(std.testing.allocator);
        defer service.deinit();
        var connection = testActiveConnectionState();
        const active = try activeConnection(&connection);
        _ = try service.runtime_aggregate.core().connect(active.connection_id, testClientId());

        var outgoing = try testRequest(
            &service,
            &connection,
            MessageKind.ping,
            .control_json,
            case.payload,
            [_]u8{0x30} ** 16,
        );
        defer outgoing.deinit();
        try expectSingleFailureCode(&outgoing, case.code);
        try std.testing.expect(!connection.shouldClose());
        try std.testing.expectEqual(
            @as(usize, 0),
            service.runtime_aggregate.core().terminalCount(),
        );
        service.disconnect(&connection);
    }
}

test "malformed Hello decode failures retain distinct public codes and close" {
    const invalid_utf8 = [_]u8{0xff};
    const cases = [_]struct {
        code: []const u8,
        payload: []const u8,
    }{
        .{ .code = "invalid-utf8", .payload = &invalid_utf8 },
        .{ .code = "invalid-json", .payload = "{" },
        .{ .code = "invalid-field", .payload = "{}" },
    };

    for (cases) |case| {
        var service = HostService.init(std.testing.allocator);
        defer service.deinit();
        var connection: ConnectionState = .awaiting_hello;
        const record = try protocol.encodeTransportEnvelope(
            std.testing.allocator,
            MessageKind.hello,
            .control_json,
            case.payload,
            null,
            null,
            false,
            false,
        );
        defer std.testing.allocator.free(record);

        var outgoing = try service.handle(&connection, record);
        defer outgoing.deinit();
        try expectSingleFailureCode(&outgoing, case.code);
        try std.testing.expect(connection.shouldClose());
        try std.testing.expectEqual(
            @as(usize, 0),
            service.runtime_aggregate.core().connectionCount(),
        );
    }
}

test "SetTerminalDefaults token coercion is rejected before mutation" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");
    const defaults_before = service.terminal_defaults;
    const revision_before = service.terminal_defaults_revision;

    var outgoing = try testRequest(
        &service,
        &connection,
        MessageKind.set_terminal_defaults,
        .control_json,
        "{\"configuration\":{\"scrollbackBytes\":\"25000000\"}}",
        [_]u8{0x31} ** 16,
    );
    defer outgoing.deinit();
    try expectSingleFailureCode(&outgoing, "invalid-field");
    try std.testing.expectEqualDeep(defaults_before, service.terminal_defaults);
    try std.testing.expectEqual(revision_before, service.terminal_defaults_revision);
}

test "IPC updates terminal defaults and retained terminal configuration" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "read value" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "set /P value=" };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    var defaults = try testRequest(
        &service,
        &connection,
        MessageKind.set_terminal_defaults,
        .control_json,
        "{\"configuration\":{\"scrollbackBytes\":25000000}}",
        [_]u8{0x31} ** 16,
    );
    defer defaults.deinit();
    const defaults_envelope = try protocol.validateTransportEnvelope(defaults.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_defaults_accepted, defaults_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, defaults_envelope.payload, "\"revision\":\"2\"") != null);
    try std.testing.expectEqual(@as(usize, 25_000_000), service.terminal_defaults.scrollback_bytes);

    const create_payload = try testCreatePayload(
        "33333333333333333333333333333333",
        executable,
        arguments,
    );
    defer std.testing.allocator.free(create_payload);
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x32} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    try std.testing.expectEqual(
        @as(usize, 25_000_000),
        service.runtime_aggregate.core().findTerminal(terminal_id).?.effectiveConfiguration().scrollback_bytes,
    );
    try std.testing.expectEqual(
        @as(usize, 25_000_000),
        findRuntime(&service, terminal_id).?.configuration().scrollback_bytes,
    );

    const explicit_create_payload = try testCreatePayloadWithConfiguration(
        "44444444444444444444444444444444",
        executable,
        arguments,
        .{ .scrollbackBytes = 5_000_000 },
    );
    defer std.testing.allocator.free(explicit_create_payload);
    var explicitly_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        explicit_create_payload,
        [_]u8{0x35} ** 16,
    );
    defer explicitly_created.deinit();
    const explicit_terminal_id = service.runtime_aggregate.core().terminalKeys()[1];
    try std.testing.expectEqual(
        @as(usize, 5_000_000),
        service.runtime_aggregate.core().findTerminal(explicit_terminal_id).?.effectiveConfiguration().scrollback_bytes,
    );
    try std.testing.expectEqual(
        @as(usize, 5_000_000),
        findRuntime(&service, explicit_terminal_id).?.configuration().scrollback_bytes,
    );

    var terminal_buffer: [32]u8 = undefined;
    const update_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"configuration\":{{\"scrollbackBytes\":0}},\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(update_payload);
    var updated = try testRequest(
        &service,
        &connection,
        MessageKind.update_terminal_configuration,
        .control_json,
        update_payload,
        [_]u8{0x33} ** 16,
    );
    defer updated.deinit();
    const updated_envelope = try protocol.validateTransportEnvelope(updated.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_configuration_accepted, updated_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, updated_envelope.payload, "\"configurationRevision\":\"2\"") != null);
    try std.testing.expectEqual(
        @as(usize, 0),
        service.runtime_aggregate.core().findTerminal(terminal_id).?.effectiveConfiguration().scrollback_bytes,
    );
    try std.testing.expectEqual(@as(usize, 0), findRuntime(&service, terminal_id).?.configuration().scrollback_bytes);

    var inventory = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        "{\"cursor\":null,\"maximumTerminals\":128}",
        [_]u8{0x34} ** 16,
    );
    defer inventory.deinit();
    const inventory_envelope = try protocol.validateTransportEnvelope(inventory.records()[0]);
    try std.testing.expect(std.mem.indexOf(u8, inventory_envelope.payload, "\"scrollbackBytes\":0") != null);
    try std.testing.expect(std.mem.indexOf(u8, inventory_envelope.payload, "\"configurationRevision\":\"2\"") != null);
}

test "reply reservation failure cannot commit terminal defaults" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    // The parser owns allocation 0. The two Outgoing view arrays own 1 and 2,
    // so allocation 3 is the record storage that must exist before mutation.
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 3 },
    );
    const service_allocator = service.allocator;
    service.allocator = failing.allocator();
    const request = testRequest(
        &service,
        &connection,
        MessageKind.set_terminal_defaults,
        .control_json,
        "{\"configuration\":{\"scrollbackBytes\":25000000}}",
        [_]u8{0x39} ** 16,
    );
    service.allocator = service_allocator;

    try std.testing.expectError(error.OutOfMemory, request);
    try std.testing.expectEqual(
        (terminal_configuration.Configuration{}).scrollback_bytes,
        service.terminal_defaults.scrollback_bytes,
    );
    try std.testing.expectEqual(@as(u64, 1), service.terminal_defaults_revision);
}

test "idle stop requires both the final disconnect and an empty terminal registry" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    try std.testing.expect(service.canStopWhenIdle());

    const connection_id: core_module.ConnectionId = 7;
    _ = try service.runtime_aggregate.core().connect(
        connection_id,
        core_module.ClientId.fromBytes([_]u8{0x21} ** 16),
    );
    try std.testing.expect(!service.canStopWhenIdle());
    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x23} ** 16);
    _ = try service.runtime_aggregate.core().createTerminal(
        connection_id,
        core_module.CreationId.fromBytes([_]u8{0x22} ** 16),
        terminal_id,
        [_]u8{0x24} ** 32,
        80,
        24,
    );
    _ = service.runtime_aggregate.core().disconnect(connection_id);
    try std.testing.expect(!service.canStopWhenIdle());

    _ = try service.runtime_aggregate.core().connect(
        connection_id,
        core_module.ClientId.fromBytes([_]u8{0x21} ** 16),
    );
    _ = try service.runtime_aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try service.runtime_aggregate.core().ptyDrained(terminal_id);
    _ = try service.runtime_aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = service.runtime_aggregate.core().disconnect(connection_id);
    try std.testing.expect(service.canStopWhenIdle());
}

test "a second broker is rejected until the active broker disconnects" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var first_connection: ConnectionState = .awaiting_hello;
    var second_connection: ConnectionState = .awaiting_hello;
    var replacement_connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&first_connection);
    defer service.disconnect(&second_connection);
    defer service.disconnect(&replacement_connection);

    try testHandshake(&service, &first_connection, "22222222222222222222222222222222");
    const first_active = try activeConnection(&first_connection);
    const first_connection_id = first_active.connection_id;
    try std.testing.expectEqual(
        MessageKind.typed_failure,
        try testHandshakeResponseKind(
            &service,
            &second_connection,
            "22222222222222222222222222222222",
        ),
    );
    try std.testing.expect(second_connection.shouldClose());
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.core().connectionCount());
    try std.testing.expect(
        service.runtime_aggregate.core().connectionClientId(first_active.connection_id).?.eql(
            core_module.ClientId.fromBytes([_]u8{0x22} ** 16),
        ),
    );

    service.disconnect(&first_connection);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().connectionCount());
    try testHandshake(
        &service,
        &replacement_connection,
        "22222222222222222222222222222222",
    );
    const replacement_active = try activeConnection(&replacement_connection);
    try std.testing.expectEqual(first_connection_id + 1, replacement_active.connection_id);
}

test "disconnect removes every broker attachment and preserves retained terminals" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);

    try testHandshake(&service, &connection, "22222222222222222222222222222222");
    const active = try activeConnection(&connection);
    const terminal_id = testId(core_module.TerminalId, 0x71, 0);
    const first_attachment_id = testId(core_module.AttachmentId, 0x72, 0);
    const second_attachment_id = testId(core_module.AttachmentId, 0x73, 0);
    _ = try service.runtime_aggregate.core().createTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x74, 0),
        terminal_id,
        [_]u8{0x75} ** 32,
        80,
        24,
    );
    _ = try service.runtime_aggregate.core().attach(
        active.connection_id,
        first_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    _ = try service.runtime_aggregate.core().attach(
        active.connection_id,
        second_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );

    service.disconnect(&connection);
    try std.testing.expect(service.runtime_aggregate.core().findAttachment(first_attachment_id) == null);
    try std.testing.expect(service.runtime_aggregate.core().findAttachment(second_attachment_id) == null);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id) != null);
}

test "detach retry is idempotent within one broker session" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const owner_active = try activeConnection(&owner);

    const creation_id = core_module.CreationId.fromBytes([_]u8{0x41} ** 16);
    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x42} ** 16);
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x43} ** 16);
    _ = try service.runtime_aggregate.core().createTerminal(
        owner_active.connection_id,
        creation_id,
        terminal_id,
        [_]u8{0x44} ** 32,
        80,
        24,
    );
    _ = try service.runtime_aggregate.core().attach(
        owner_active.connection_id,
        attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );

    const detach_payload =
        \\{"attachmentId":"43434343434343434343434343434343"}
    ;
    inline for (.{ @as(u8, 0x45), @as(u8, 0x46) }) |correlation_byte| {
        var detached = try testRequest(
            &service,
            &owner,
            MessageKind.detach_terminal,
            .control_json,
            detach_payload,
            [_]u8{correlation_byte} ** 16,
        );
        defer detached.deinit();
        try std.testing.expectEqual(
            MessageKind.detach_terminal,
            (try protocol.validateTransportEnvelope(detached.records()[0])).kind,
        );
    }
    try std.testing.expect(service.runtime_aggregate.core().findAttachment(attachment_id) == null);
}

test "current protocol pages a stable inventory and rejects stale cursors" {
    const TestInventoryPage = struct {
        inventoryGeneration: []const u8,
        nextCursor: ?InventoryCursorWire,
        terminals: []std.json.Value,
    };

    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try testHandshake(&service, &connection, "22222222222222222222222222222222");
    const active = try activeConnection(&connection);

    const replay_creation_id = testId(core_module.CreationId, 0x31, 0);
    const replay_terminal_id = testId(core_module.TerminalId, 0x32, 0);
    var replay_creation_buffer: [32]u8 = undefined;
    const replay_payload = try testCreatePayload(
        identifierText(replay_creation_id, &replay_creation_buffer),
        "orchbee-terminal-host-deliberately-missing-executable",
        &.{},
    );
    defer std.testing.allocator.free(replay_payload);
    var replay_request = try control_decode.decodeCreateTerminal(
        std.testing.allocator,
        replay_payload,
    );
    defer replay_request.deinit();
    _ = try service.runtime_aggregate.core().createTerminal(
        active.connection_id,
        replay_creation_id,
        replay_terminal_id,
        replay_request.value.launch.launch_digest,
        80,
        24,
    );
    for (1..130) |index| {
        const creation_id = testId(core_module.CreationId, 0x31, index);
        const terminal_id = testId(core_module.TerminalId, 0x32, index);
        _ = try service.runtime_aggregate.core().createTerminal(
            active.connection_id,
            creation_id,
            terminal_id,
            [_]u8{@intCast(index + 1)} ** 32,
            80,
            24,
        );
    }

    var first_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        "{\"cursor\":null,\"maximumTerminals\":128}",
        [_]u8{0x41} ** 16,
    );
    defer first_response.deinit();
    try std.testing.expectEqual(@as(usize, 1), first_response.records().len);
    const first_envelope = try protocol.validateTransportEnvelope(first_response.records()[0]);
    const first_page = try std.json.parseFromSlice(
        TestInventoryPage,
        std.testing.allocator,
        first_envelope.payload,
        .{},
    );
    defer first_page.deinit();
    try std.testing.expectEqual(@as(usize, 128), first_page.value.terminals.len);
    try std.testing.expect(first_page.value.nextCursor != null);
    try std.testing.expectEqualStrings("128", first_page.value.nextCursor.?.offset);

    const second_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"cursor\":{{\"inventoryGeneration\":\"{s}\",\"offset\":\"{s}\"}},\"maximumTerminals\":128}}",
        .{
            first_page.value.nextCursor.?.inventoryGeneration,
            first_page.value.nextCursor.?.offset,
        },
    );
    defer std.testing.allocator.free(second_payload);
    var second_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        second_payload,
        [_]u8{0x42} ** 16,
    );
    defer second_response.deinit();
    const second_envelope = try protocol.validateTransportEnvelope(second_response.records()[0]);
    const second_page = try std.json.parseFromSlice(
        TestInventoryPage,
        std.testing.allocator,
        second_envelope.payload,
        .{},
    );
    defer second_page.deinit();
    try std.testing.expectEqual(@as(usize, 2), second_page.value.terminals.len);
    try std.testing.expect(second_page.value.nextCursor == null);
    try std.testing.expectEqualStrings(
        first_page.value.inventoryGeneration,
        second_page.value.inventoryGeneration,
    );

    _ = try service.runtime_aggregate.core().createTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x31, 130),
        testId(core_module.TerminalId, 0x32, 130),
        [_]u8{0xee} ** 32,
        80,
        24,
    );
    var stale_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        second_payload,
        [_]u8{0x43} ** 16,
    );
    defer stale_response.deinit();
    const stale_envelope = try protocol.validateTransportEnvelope(stale_response.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, stale_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, stale_envelope.payload, "stale-inventory-cursor") != null);
}

test "force termination rejects an invalid payload" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    var response = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        "{}",
        [_]u8{0x41} ** 16,
    );
    defer response.deinit();
    try std.testing.expectEqual(@as(usize, 1), response.records().len);
    const envelope = try protocol.validateTransportEnvelope(response.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, envelope.payload, "invalid-field") != null);
}

test "current protocol force-terminates every retained POSIX session member" {
    if (@import("builtin").os.tag == .windows) return error.SkipZigTest;
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        for (service.runtime_aggregate.core().terminalValues()) |session| {
            if (session.runtime()) |engine| engine.forceTerminate() catch {};
        }
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    const arguments = [_][]const u8{
        "-c",
        "trap '' HUP; set -m; printf ready; while :; do sleep 600 & wait; done",
    };
    const create_payload = try testCreatePayload(
        "33333333333333333333333333333333",
        "/bin/sh",
        &arguments,
    );
    defer std.testing.allocator.free(create_payload);
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x42} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    var ready = false;
    for (0..400) |_| {
        const plain = try findRuntime(&service, terminal_id).?.plainString(std.testing.allocator);
        defer std.testing.allocator.free(plain);
        if (std.mem.indexOf(u8, plain, "ready") != null) {
            ready = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(ready);
    const target = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(target);

    var initial_force = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        target,
        [_]u8{0x43} ** 16,
    );
    defer initial_force.deinit();
    try std.testing.expectEqual(
        MessageKind.force_terminate_terminal,
        (try protocol.validateTransportEnvelope(initial_force.records()[0])).kind,
    );

    var graceful = try testRequest(
        &service,
        &connection,
        MessageKind.terminate_terminal,
        .control_json,
        target,
        [_]u8{0x44} ** 16,
    );
    defer graceful.deinit();
    try std.testing.expectEqual(
        MessageKind.terminate_terminal,
        (try protocol.validateTransportEnvelope(graceful.records()[0])).kind,
    );

    var forced = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        target,
        [_]u8{0x45} ** 16,
    );
    defer forced.deinit();
    try std.testing.expectEqual(
        MessageKind.force_terminate_terminal,
        (try protocol.validateTransportEnvelope(forced.records()[0])).kind,
    );
    try std.testing.expectEqual(
        core_module.TerminationRequest.force,
        service.runtime_aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.runtime_aggregate.core().findTerminal(terminal_id).?.phase() == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.runtime_aggregate.core().findTerminal(terminal_id).?.phase(),
    );
}

test "malformed request receives a correlated typed failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try service.runtime_aggregate.core().connect(active.connection_id, testClientId());
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.create_terminal,
        .control_json,
        "{",
        correlation,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, response.kind);
    try std.testing.expectEqualSlices(u8, &correlation, &response.correlation_id.?);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    service.disconnect(&connection);
}

test "stale HostEpoch receives a correlated failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try service.runtime_aggregate.core().connect(active.connection_id, testClientId());
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.list_terminals,
        .control_json,
        "{}",
        correlation,
        [_]u8{0x44} ** 16,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, response.kind);
    try std.testing.expectEqualSlices(u8, &correlation, &response.correlation_id.?);
    try std.testing.expect(std.mem.indexOf(u8, response.payload, "stale-epoch") != null);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    service.disconnect(&connection);
}

test "unknown optional request is skipped without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try service.runtime_aggregate.core().connect(active.connection_id, testClientId());
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.ping,
        .control_json,
        "{}",
        correlation,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    std.mem.writeInt(u16, record[20..22], 200, .little);
    std.mem.writeInt(u16, record[6..8], 1, .little);

    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    service.disconnect(&connection);
}

test "registered message roles and encodings fail before command dispatch" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try service.runtime_aggregate.core().connect(active.connection_id, testClientId());

    const output_kind_record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.ping,
        .control_json,
        "{}",
        [_]u8{0x61} ** 16,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(output_kind_record);
    std.mem.writeInt(u16, output_kind_record[20..22], MessageKind.terminal_started, .little);
    var output_kind_response = try service.handle(&connection, output_kind_record);
    defer output_kind_response.deinit();
    try std.testing.expectEqual(@as(usize, 1), output_kind_response.records().len);
    const output_kind_envelope = try protocol.validateTransportEnvelope(
        output_kind_response.records()[0],
    );
    try std.testing.expect(std.mem.indexOf(
        u8,
        output_kind_envelope.payload,
        "invalid-envelope-role",
    ) != null);

    const wrong_encoding_record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.ping,
        .control_json,
        "{}",
        [_]u8{0x62} ** 16,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(wrong_encoding_record);
    wrong_encoding_record[22] = @intFromEnum(protocol.PayloadEncoding.raw_bytes);
    var wrong_encoding_response = try service.handle(&connection, wrong_encoding_record);
    defer wrong_encoding_response.deinit();
    try std.testing.expectEqual(@as(usize, 1), wrong_encoding_response.records().len);
    const wrong_encoding_envelope = try protocol.validateTransportEnvelope(
        wrong_encoding_response.records()[0],
    );
    try std.testing.expect(std.mem.indexOf(
        u8,
        wrong_encoding_envelope.payload,
        "invalid-field",
    ) != null);
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    service.disconnect(&connection);
}

test "invalid transport framing closes dispatch without a response" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    var record = [_]u8{0} ** protocol.transport_envelope_header_bytes;
    @memcpy(record[0..4], "NOPE");
    try std.testing.expectError(
        error.InvalidTransportRecord,
        service.handle(&connection, &record),
    );
}

test "real PTY lifecycle survives disconnect and an idempotent create retry" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf 'ready:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =ready:& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "33333333333333333333333333333333",
        executable,
        arguments,
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var first_connection: ConnectionState = .awaiting_hello;
    var second_connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&first_connection);
        service.disconnect(&second_connection);
        service.deinit();
    }

    try testHandshake(&service, &first_connection, "22222222222222222222222222222222");
    const create_correlation = [_]u8{0x51} ** 16;
    var created = try testRequest(
        &service,
        &first_connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        create_correlation,
    );
    defer created.deinit();
    try std.testing.expectEqual(@as(usize, 2), created.records().len);
    try std.testing.expectEqual(MessageKind.create_terminal, (try protocol.validateTransportEnvelope(created.records()[0])).kind);
    try std.testing.expectEqual(MessageKind.terminal_started, (try protocol.validateTransportEnvelope(created.records()[1])).kind);
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const retained_runtime = findRuntime(&service, terminal_id).?;

    service.disconnect(&first_connection);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id) != null);
    try std.testing.expect(findRuntime(&service, terminal_id) == retained_runtime);
    try std.testing.expectEqual(
        core_module.TerminationRequest.none,
        service.runtime_aggregate.core().findTerminal(terminal_id).?.terminationRequest(),
    );

    try testHandshake(&service, &second_connection, "66666666666666666666666666666666");
    var retried = try testRequest(
        &service,
        &second_connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x52} ** 16,
    );
    defer retried.deinit();
    try std.testing.expectEqual(@as(usize, 1), retried.records().len);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id) != null);
    try std.testing.expect(findRuntime(&service, terminal_id) == retained_runtime);

    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &second_connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x53} ** 16,
    );
    defer attached.deinit();
    try std.testing.expectEqual(MessageKind.attachment_capabilities, (try protocol.validateTransportEnvelope(attached.records()[0])).kind);

    const resize_payload =
        \\{"attachmentId":"44444444444444444444444444444444","cellPixelHeight":16,"cellPixelWidth":8,"columns":100,"rows":30}
    ;
    var resized = try testRequest(
        &service,
        &second_connection,
        MessageKind.resize_terminal,
        .control_json,
        resize_payload,
        [_]u8{0x54} ** 16,
    );
    defer resized.deinit();
    try std.testing.expectEqual(MessageKind.resize_accepted, (try protocol.validateTransportEnvelope(resized.records()[0])).kind);
    try std.testing.expectEqual(
        @as(u16, 100),
        service.runtime_aggregate.core().findTerminal(terminal_id).?.observedGeometry().columns,
    );
    try std.testing.expectEqual(
        @as(u16, 30),
        service.runtime_aggregate.core().findTerminal(terminal_id).?.observedGeometry().rows,
    );

    const input_text: []const u8 = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    var input_payload: [16 + input_text.len]u8 = undefined;
    @memset(input_payload[0..16], 0x44);
    @memcpy(input_payload[16..], input_text);
    var input = try testRequest(
        &service,
        &second_connection,
        MessageKind.write_input,
        .raw_bytes,
        &input_payload,
        [_]u8{0x55} ** 16,
    );
    defer input.deinit();
    try std.testing.expectEqual(MessageKind.input_accepted, (try protocol.validateTransportEnvelope(input.records()[0])).kind);

    var observed_exit_event = false;
    for (0..400) |_| {
        var events = try service.pollEvents(&second_connection);
        for (events.records()) |event_record| {
            const event = try protocol.validateTransportEnvelope(event_record);
            observed_exit_event = observed_exit_event or event.kind == MessageKind.terminal_exited;
        }
        events.deinit();
        if (service.runtime_aggregate.core().findTerminal(terminal_id).?.phase() == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, service.runtime_aggregate.core().findTerminal(terminal_id).?.phase());
    try std.testing.expect(observed_exit_event);
    try std.testing.expect(
        service.runtime_aggregate.core().findTerminal(terminal_id).?.observedRevision() >= 2,
    );

    const plain = try findRuntime(&service, terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "ready:orchbee") != null);

    var dispose_buffer: [32]u8 = undefined;
    const dispose_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &dispose_buffer)},
    );
    defer std.testing.allocator.free(dispose_payload);
    var disposed = try testRequest(
        &service,
        &second_connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x56} ** 16,
    );
    defer disposed.deinit();
    try std.testing.expect(findRuntime(&service, terminal_id) == null);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(terminal_id) == null);
}

test "post-exit configuration is rejected without revising or republishing outcome" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    const arguments = [_][]const u8{
        "-c",
        "i=0; while [ $i -lt 100 ]; do printf 'line\\n'; i=$((i+1)); done",
    };
    const create_payload = try testCreatePayloadWithConfiguration(
        "77777777777777777777777777777777",
        "/bin/sh",
        &arguments,
        .{ .scrollbackBytes = 1_000_000 },
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "88888888888888888888888888888888");

    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x71} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];

    var first_final_revision: u64 = 0;
    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        defer events.deinit();
        for (events.records()) |event_record| {
            const event = try protocol.validateTransportEnvelope(event_record);
            if (event.kind != MessageKind.terminal_exited) continue;
            const parsed = try std.json.parseFromSlice(
                struct { finalRevision: []const u8 },
                std.testing.allocator,
                event.payload,
                .{ .ignore_unknown_fields = true },
            );
            defer parsed.deinit();
            first_final_revision = try std.fmt.parseUnsigned(u64, parsed.value.finalRevision, 10);
        }
        if (service.runtime_aggregate.core().findTerminal(terminal_id).?.phase() == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, service.runtime_aggregate.core().findTerminal(terminal_id).?.phase());
    try std.testing.expect(first_final_revision > 0);
    const settled = service.runtime_aggregate.core().findTerminal(terminal_id).?;
    const settled_configuration = settled.effectiveConfiguration();
    const settled_configuration_revision = settled.configurationRevision();
    try std.testing.expectEqual(first_final_revision, settled.settledFinalRevision().?);

    var terminal_buffer: [32]u8 = undefined;
    const update_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"configuration\":{{\"scrollbackBytes\":0}},\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(update_payload);
    var updated = try testRequest(
        &service,
        &connection,
        MessageKind.update_terminal_configuration,
        .control_json,
        update_payload,
        [_]u8{0x72} ** 16,
    );
    defer updated.deinit();

    try std.testing.expectEqual(@as(usize, 1), updated.records().len);
    const rejection = try protocol.validateTransportEnvelope(updated.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, rejection.kind);
    try std.testing.expect(std.mem.indexOf(u8, rejection.payload, "invalid-phase") != null);
    const retained = service.runtime_aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(first_final_revision, retained.observedRevision());
    try std.testing.expectEqual(first_final_revision, retained.settledFinalRevision().?);
    try std.testing.expectEqualDeep(settled_configuration, retained.effectiveConfiguration());
    try std.testing.expectEqual(settled_configuration_revision, retained.configurationRevision());

    var later_events = try service.pollEvents(&connection);
    defer later_events.deinit();
    for (later_events.records()) |event_record| {
        const event = try protocol.validateTransportEnvelope(event_record);
        try std.testing.expect(event.kind != MessageKind.terminal_exited);
    }
}

test "retained exited terminals still project and export selection" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "printf 'selected'" };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/S",
        "/C",
        "<nul set /p =selected",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "81818181818181818181818181818181",
        executable,
        arguments,
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "82828282828282828282828282828282");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x83} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.runtime_aggregate.core().findTerminal(terminal_id).?.phase() == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    const exited = service.runtime_aggregate.core().findTerminal(terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.exited, exited.phase());

    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x84} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":false,\"read\":true,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x85} ** 16,
    );
    defer attached.deinit();

    var demand_attachment_buffer: [32]u8 = undefined;
    const demand_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"demand\":\"active\"}}",
        .{identifierText(attachment_id, &demand_attachment_buffer)},
    );
    defer std.testing.allocator.free(demand_payload);
    var demanded = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        demand_payload,
        [_]u8{0x86} ** 16,
    );
    defer demanded.deinit();
    try std.testing.expectEqual(@as(usize, 1), demanded.records().len);
    try std.testing.expectEqual(
        MessageKind.projection_demand_accepted,
        (try protocol.validateTransportEnvelope(demanded.records()[0])).kind,
    );
    var projection_events = try service.pollEvents(&connection);
    defer projection_events.deinit();
    var projected = false;
    for (projection_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind != MessageKind.snapshot_frame) continue;
        const frame = try protocol.validateTerminalFrame(
            std.testing.allocator,
            envelope.payload,
        );
        try std.testing.expectEqual(exited.observedRevision(), frame.revision);
        projected = true;
    }
    try std.testing.expect(projected);

    const geometry = runtime.geometry();
    const selection = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    try std.testing.expectEqual(
        terminal_runtime.SelectionSnapshot.active,
        std.meta.activeTag(selection),
    );
    var export_attachment_buffer: [32]u8 = undefined;
    const export_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"1\",\"format\":\"plain\"}}",
        .{identifierText(attachment_id, &export_attachment_buffer)},
    );
    defer std.testing.allocator.free(export_payload);
    var exported = try testRequest(
        &service,
        &connection,
        MessageKind.selection_export,
        .control_json,
        export_payload,
        [_]u8{0x87} ** 16,
    );
    defer exported.deinit();
    var started = false;
    var chunked = false;
    for (exported.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        started = started or envelope.kind == MessageKind.selection_export_started;
        chunked = chunked or envelope.kind == MessageKind.selection_export_chunk;
    }
    try std.testing.expect(started);
    try std.testing.expect(!chunked);
    var export_events = try service.pollEvents(&connection);
    defer export_events.deinit();
    for (export_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        chunked = chunked or envelope.kind == MessageKind.selection_export_chunk;
    }
    try std.testing.expect(chunked);
}

test "two PTY runtimes route input independently and isolate spawn failure" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const first_posix_arguments = [_][]const u8{
        "-c",
        "printf 'first-ready:'; read value; printf 'first-%s' \"$value\"",
    };
    const second_posix_arguments = [_][]const u8{
        "-c",
        "printf 'second-ready:'; read value; printf 'second-%s' \"$value\"",
    };
    const first_windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =first-ready:& set /p value=& <nul set /p =first-!value!",
    };
    const second_windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =second-ready:& set /p value=& <nul set /p =second-!value!",
    };
    const first_arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &first_windows_arguments
    else
        &first_posix_arguments;
    const second_arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &second_windows_arguments
    else
        &second_posix_arguments;

    const first_create_payload = try testCreatePayload(
        "31313131313131313131313131313131",
        executable,
        first_arguments,
    );
    defer std.testing.allocator.free(first_create_payload);
    const failed_create_payload = try testCreatePayload(
        "32323232323232323232323232323232",
        "orchbee-terminal-host-deliberately-missing-executable",
        &.{},
    );
    defer std.testing.allocator.free(failed_create_payload);
    const second_create_payload = try testCreatePayload(
        "33333333333333333333333333333333",
        executable,
        second_arguments,
    );
    defer std.testing.allocator.free(second_create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    var first_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        first_create_payload,
        [_]u8{0x51} ** 16,
    );
    defer first_created.deinit();
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.runtimeCount());
    const first_terminal_id = service.runtime_aggregate.core().terminalKeys()[0];

    var failed_create = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        failed_create_payload,
        [_]u8{0x52} ** 16,
    );
    defer failed_create.deinit();
    try std.testing.expectEqual(@as(usize, 1), failed_create.records().len);
    const failed_envelope = try protocol.validateTransportEnvelope(failed_create.records()[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, failed_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, failed_envelope.payload, "spawn-failed") != null);
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 1), service.runtime_aggregate.runtimeCount());
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(first_terminal_id) != null);

    var second_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        second_create_payload,
        [_]u8{0x53} ** 16,
    );
    defer second_created.deinit();
    try std.testing.expectEqual(@as(usize, 2), service.runtime_aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 2), service.runtime_aggregate.runtimeCount());
    const second_terminal_id = service.runtime_aggregate.core().terminalKeys()[1];

    const first_attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    const second_attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x55} ** 16);
    const first_attach_payload = try testAttachPayload(first_attachment_id, first_terminal_id);
    defer std.testing.allocator.free(first_attach_payload);
    const second_attach_payload = try testAttachPayload(second_attachment_id, second_terminal_id);
    defer std.testing.allocator.free(second_attach_payload);
    var first_attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        first_attach_payload,
        [_]u8{0x54} ** 16,
    );
    defer first_attached.deinit();
    var second_attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        second_attach_payload,
        [_]u8{0x55} ** 16,
    );
    defer second_attached.deinit();

    const first_input: []const u8 = if (builtin.os.tag == .windows) "alpha\r\n" else "alpha\n";
    const second_input: []const u8 = if (builtin.os.tag == .windows) "beta\r\n" else "beta\n";
    const first_input_payload = try testRawInputPayload(first_attachment_id, first_input);
    defer std.testing.allocator.free(first_input_payload);
    const second_input_payload = try testRawInputPayload(second_attachment_id, second_input);
    defer std.testing.allocator.free(second_input_payload);
    var first_accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        first_input_payload,
        [_]u8{0x56} ** 16,
    );
    defer first_accepted.deinit();
    var second_accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        second_input_payload,
        [_]u8{0x57} ** 16,
    );
    defer second_accepted.deinit();

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.runtime_aggregate.core().findTerminal(first_terminal_id).?.phase() == .exited and
            service.runtime_aggregate.core().findTerminal(second_terminal_id).?.phase() == .exited)
        {
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.runtime_aggregate.core().findTerminal(first_terminal_id).?.phase(),
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.runtime_aggregate.core().findTerminal(second_terminal_id).?.phase(),
    );
    const first_plain = try findRuntime(&service, first_terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(first_plain);
    const second_plain = try findRuntime(&service, second_terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(second_plain);
    try std.testing.expect(std.mem.indexOf(u8, first_plain, "first-alpha") != null);
    try std.testing.expect(std.mem.indexOf(u8, first_plain, "second-beta") == null);
    try std.testing.expect(std.mem.indexOf(u8, second_plain, "second-beta") != null);
    try std.testing.expect(std.mem.indexOf(u8, second_plain, "first-alpha") == null);

    const first_dispose_payload = try testTerminalTargetPayload(first_terminal_id);
    defer std.testing.allocator.free(first_dispose_payload);
    var first_disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        first_dispose_payload,
        [_]u8{0x58} ** 16,
    );
    defer first_disposed.deinit();
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(first_terminal_id) == null);
    try std.testing.expect(findRuntime(&service, first_terminal_id) == null);
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(second_terminal_id) != null);
    try std.testing.expect(findRuntime(&service, second_terminal_id) != null);

    const second_dispose_payload = try testTerminalTargetPayload(second_terminal_id);
    defer std.testing.allocator.free(second_dispose_payload);
    var second_disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        second_dispose_payload,
        [_]u8{0x59} ** 16,
    );
    defer second_disposed.deinit();
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.core().terminalCount());
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.runtimeCount());
}

test "current protocol activates a lazy live projection and accepts semantic text" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf 'ready:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =ready:& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const environment = [_]EnvironmentWire{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    var create_writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer create_writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = "33333333333333333333333333333333",
        .launch = .{
            .arguments = arguments,
            .cellPixelHeight = 16,
            .cellPixelWidth = 8,
            .columns = 20,
            .configuration = @as(?TerminalConfigurationWire, null),
            .currentDirectory = @as(?[]const u8, null),
            .environment = &environment,
            .executable = executable,
            .rows = 4,
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
    }, .{}, &create_writer.writer);
    const create_payload = try create_writer.toOwnedSlice();
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x61} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;
    try std.testing.expectEqual(@as(u64, 0), runtime.projectionCounters().render_state_creations);
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x62} ** 16,
    );
    defer attached.deinit();
    try std.testing.expectEqual(MessageKind.attachment_capabilities, (try protocol.validateTransportEnvelope(attached.records()[0])).kind);
    try std.testing.expectEqual(@as(u64, 0), runtime.projectionCounters().render_state_creations);

    const demand_payload =
        \\{"attachmentId":"44444444444444444444444444444444","demand":"active"}
    ;
    var demanded = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        demand_payload,
        [_]u8{0x63} ** 16,
    );
    defer demanded.deinit();
    try std.testing.expectEqual(@as(usize, 1), demanded.records().len);
    try std.testing.expectEqual(MessageKind.projection_demand_accepted, (try protocol.validateTransportEnvelope(demanded.records()[0])).kind);
    var demand_events = try service.pollEvents(&connection);
    defer demand_events.deinit();
    try std.testing.expectEqual(@as(usize, 1), demand_events.records().len);
    const frame_envelope = try protocol.validateTransportEnvelope(demand_events.records()[0]);
    try std.testing.expectEqual(MessageKind.snapshot_frame, frame_envelope.kind);
    const frame = try protocol.validateTerminalFrame(std.testing.allocator, frame_envelope.payload);
    try std.testing.expectEqual(@as(u16, 20), frame.columns);
    try std.testing.expectEqual(@as(u16, 4), frame.rows);
    try std.testing.expectEqual(@as(u16, 8), frame.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 16), frame.cell_pixel_height);
    try std.testing.expectEqual(@as(u64, 1), runtime.projectionCounters().render_state_creations);

    var frame_ack_payload_buffer: [256]u8 = undefined;
    const frame_ack_payload = try std.fmt.bufPrint(
        &frame_ack_payload_buffer,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{ frame.generation, frame.revision, frame.sequence },
    );
    const pause_payload =
        \\{"attachmentId":"44444444444444444444444444444444","demand":"paused"}
    ;
    var paused = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        pause_payload,
        [_]u8{0x66} ** 16,
    );
    defer paused.deinit();
    try std.testing.expect(!runtime.projectionResident());
    try std.testing.expect(
        service.runtime_aggregate.core().findAttachment(core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16)).?.projectionInFlight() == null,
    );

    var acknowledged = try testRequest(
        &service,
        &connection,
        MessageKind.frame_applied,
        .control_json,
        frame_ack_payload,
        [_]u8{0x64} ** 16,
    );
    defer acknowledged.deinit();
    try std.testing.expectEqual(MessageKind.frame_applied, (try protocol.validateTransportEnvelope(acknowledged.records()[0])).kind);

    try std.testing.expect(service.runtime_aggregate.setNextProjectionAt(terminal_id, 0));
    var resumed = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        demand_payload,
        [_]u8{0x67} ** 16,
    );
    defer resumed.deinit();
    try std.testing.expectEqual(@as(usize, 1), resumed.records().len);
    var resume_events = try service.pollEvents(&connection);
    defer resume_events.deinit();
    try std.testing.expectEqual(@as(usize, 1), resume_events.records().len);
    const resumed_envelope = try protocol.validateTransportEnvelope(resume_events.records()[0]);
    try std.testing.expectEqual(MessageKind.snapshot_frame, resumed_envelope.kind);
    const resumed_frame = try protocol.validateTerminalFrame(std.testing.allocator, resumed_envelope.payload);
    try std.testing.expectEqual(frame.sequence + 1, resumed_frame.sequence);
    try std.testing.expectEqual(@as(u64, 2), runtime.projectionCounters().render_state_creations);

    var paused_again = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        pause_payload,
        [_]u8{0x68} ** 16,
    );
    defer paused_again.deinit();
    try std.testing.expect(!runtime.projectionResident());
    const counters_before_pause = runtime.projectionCounters();

    const input_text: []const u8 = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    const text_payload = try std.testing.allocator.alloc(u8, 20 + input_text.len);
    defer std.testing.allocator.free(text_payload);
    @memset(text_payload[0..16], 0x44);
    std.mem.writeInt(u32, text_payload[16..20], @intCast(input_text.len), .little);
    @memcpy(text_payload[20..], input_text);
    var accepted = try testRequest(
        &service,
        &connection,
        MessageKind.text_input,
        .raw_bytes,
        text_payload,
        [_]u8{0x65} ** 16,
    );
    defer accepted.deinit();
    try std.testing.expectEqual(MessageKind.input_accepted, (try protocol.validateTransportEnvelope(accepted.records()[0])).kind);

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.runtime_aggregate.core().findTerminal(terminal_id).?.phase() == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, service.runtime_aggregate.core().findTerminal(terminal_id).?.phase());
    const counters_after_exit = runtime.projectionCounters();
    try std.testing.expectEqual(counters_before_pause.captures, counters_after_exit.captures);
    try std.testing.expectEqual(counters_before_pause.encodes, counters_after_exit.encodes);
}

test "scheduler emits one uncorrelated cancellation for an invalidated export" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "printf 'selected'; read value" };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =selected& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "77777777777777777777777777777777",
        executable,
        arguments,
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "66666666666666666666666666666666");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x71} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x72} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x73} ** 16,
    );
    defer attached.deinit();

    var output_ready = false;
    for (0..400) |_| {
        const output = try runtime.plainString(std.testing.allocator);
        defer std.testing.allocator.free(output);
        if (std.mem.indexOf(u8, output, "selected") != null) {
            output_ready = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(output_ready);

    const geometry = runtime.geometry();
    const snapshot = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    try std.testing.expectEqual(terminal_runtime.SelectionSnapshot.active, std.meta.activeTag(snapshot));
    const started = (try runtime.startSelectionExport(
        attachment_id,
        1,
        .plain,
    )).started;
    _ = try runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );

    var expected_attachment_buffer: [32]u8 = undefined;
    const expected_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"status\":\"cancelled\"}}",
        .{
            identifierText(attachment_id, &expected_attachment_buffer),
            started.byte_length,
            started.export_sequence,
        },
    );
    defer std.testing.allocator.free(expected_payload);

    var events = try service.pollEvents(&connection);
    defer events.deinit();
    var finished_count: usize = 0;
    var chunk_count: usize = 0;
    for (events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind == MessageKind.selection_export_finished) {
            finished_count += 1;
            try std.testing.expectEqualSlices(u8, expected_payload, envelope.payload);
            try std.testing.expect(envelope.correlation_id == null);
            try std.testing.expect(!envelope.response);
            try std.testing.expect(envelope.event);
        } else if (envelope.kind == MessageKind.selection_export_chunk) {
            chunk_count += 1;
        }
    }
    try std.testing.expectEqual(@as(usize, 1), finished_count);
    try std.testing.expectEqual(@as(usize, 0), chunk_count);

    var later_events = try service.pollEvents(&connection);
    defer later_events.deinit();
    for (later_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        try std.testing.expect(envelope.kind != MessageKind.selection_export_finished);
        try std.testing.expect(envelope.kind != MessageKind.selection_export_chunk);
    }
}

test "selection export request emits pending cancellation before a newer export starts" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "printf 'selected'; read value" };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =selected& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "79797979797979797979797979797979",
        executable,
        arguments,
    );
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "68686868686868686868686868686868");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x78} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.runtime_aggregate.core().terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x79} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x7a} ** 16,
    );
    defer attached.deinit();

    var output_ready = false;
    for (0..400) |_| {
        const output = try runtime.plainString(std.testing.allocator);
        defer std.testing.allocator.free(output);
        if (std.mem.indexOf(u8, output, "selected") != null) {
            output_ready = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(output_ready);

    const initial_geometry = runtime.geometry();
    _ = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = initial_geometry.generation,
        .revision = initial_geometry.revision,
    });
    const first_started = (try runtime.startSelectionExport(
        attachment_id,
        1,
        .plain,
    )).started;
    _ = try runtime.resizeGeometry(
        initial_geometry.columns + 1,
        initial_geometry.rows,
        initial_geometry.cell_pixel_width,
        initial_geometry.cell_pixel_height,
    );

    const current_geometry = runtime.geometry();
    const snapshot = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = current_geometry.generation,
        .revision = current_geometry.revision,
    });
    try std.testing.expectEqual(terminal_runtime.SelectionSnapshot.active, std.meta.activeTag(snapshot));

    var export_attachment_buffer: [32]u8 = undefined;
    const export_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"2\",\"format\":\"plain\"}}",
        .{identifierText(attachment_id, &export_attachment_buffer)},
    );
    defer std.testing.allocator.free(export_payload);
    const export_correlation = [_]u8{0x7b} ** 16;
    var exported = try testRequest(
        &service,
        &connection,
        MessageKind.selection_export,
        .control_json,
        export_payload,
        export_correlation,
    );
    defer exported.deinit();

    var expected_attachment_buffer: [32]u8 = undefined;
    const expected_cancellation_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"1\",\"status\":\"cancelled\"}}",
        .{
            identifierText(attachment_id, &expected_attachment_buffer),
            first_started.byte_length,
        },
    );
    defer std.testing.allocator.free(expected_cancellation_payload);

    var cancellation_position: ?usize = null;
    var started_position: ?usize = null;
    var chunk_count: usize = 0;
    for (exported.records(), 0..) |record, index| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind == MessageKind.selection_export_finished) {
            try std.testing.expectEqualSlices(u8, expected_cancellation_payload, envelope.payload);
            try std.testing.expect(envelope.correlation_id == null);
            try std.testing.expect(!envelope.response);
            try std.testing.expect(envelope.event);
            try std.testing.expect(cancellation_position == null);
            cancellation_position = index;
        } else if (envelope.kind == MessageKind.selection_export_started) {
            try std.testing.expectEqualSlices(u8, &export_correlation, &envelope.correlation_id.?);
            try std.testing.expect(std.mem.indexOf(u8, envelope.payload, "\"exportSequence\":\"2\"") != null);
            try std.testing.expect(envelope.response);
            try std.testing.expect(!envelope.event);
            try std.testing.expect(started_position == null);
            started_position = index;
        } else if (envelope.kind == MessageKind.selection_export_chunk) {
            try std.testing.expectEqual(
                @intFromEnum(protocol.PayloadEncoding.raw_bytes),
                envelope.payload_encoding,
            );
            try std.testing.expect(envelope.payload.len >= 32);
            try std.testing.expectEqual(@as(u64, 2), std.mem.readInt(u64, envelope.payload[16..24], .little));
            chunk_count += 1;
        }
    }
    try std.testing.expect(cancellation_position != null);
    try std.testing.expect(started_position != null);
    try std.testing.expect(cancellation_position.? < started_position.?);
    try std.testing.expectEqual(@as(usize, 0), chunk_count);

    var export_events = try service.pollEvents(&connection);
    defer export_events.deinit();
    for (export_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind != MessageKind.selection_export_chunk) continue;
        try std.testing.expectEqual(
            @intFromEnum(protocol.PayloadEncoding.raw_bytes),
            envelope.payload_encoding,
        );
        try std.testing.expect(envelope.payload.len >= 32);
        try std.testing.expectEqual(
            @as(u64, 2),
            std.mem.readInt(u64, envelope.payload[16..24], .little),
        );
        chunk_count += 1;
    }
    try std.testing.expectEqual(@as(usize, 1), chunk_count);

    var applied_attachment_buffer: [32]u8 = undefined;
    const applied_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"chunkSequence\":0,\"exportSequence\":\"2\"}}",
        .{identifierText(attachment_id, &applied_attachment_buffer)},
    );
    defer std.testing.allocator.free(applied_payload);
    const applied_correlation = [_]u8{0x7c} ** 16;
    var applied = try testRequest(
        &service,
        &connection,
        MessageKind.selection_export_chunk_applied,
        .control_json,
        applied_payload,
        applied_correlation,
    );
    defer applied.deinit();

    var acknowledged = false;
    var completed = false;
    for (applied.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind == MessageKind.selection_export_chunk_applied) {
            try std.testing.expectEqualSlices(u8, &applied_correlation, &envelope.correlation_id.?);
            try std.testing.expect(envelope.response);
            try std.testing.expect(!envelope.event);
            acknowledged = true;
        } else if (envelope.kind == MessageKind.selection_export_finished) {
            try std.testing.expect(std.mem.indexOf(u8, envelope.payload, "\"exportSequence\":\"2\"") != null);
            try std.testing.expect(std.mem.indexOf(u8, envelope.payload, "\"status\":\"completed\"") != null);
            try std.testing.expect(envelope.correlation_id == null);
            try std.testing.expect(!envelope.response);
            try std.testing.expect(envelope.event);
            completed = true;
        }
    }
    try std.testing.expect(acknowledged);
    try std.testing.expect(completed);

    const after_completion_geometry = runtime.geometry();
    _ = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = after_completion_geometry.generation,
        .revision = after_completion_geometry.revision,
    });
    const unannounced_started = (try runtime.startSelectionExport(
        attachment_id,
        3,
        .plain,
    )).started;
    _ = try runtime.resizeGeometry(
        after_completion_geometry.columns + 1,
        after_completion_geometry.rows,
        after_completion_geometry.cell_pixel_width,
        after_completion_geometry.cell_pixel_height,
    );
    const restored_selection_geometry = runtime.geometry();
    _ = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = restored_selection_geometry.generation,
        .revision = restored_selection_geometry.revision,
    });

    var failed_export_attachment_buffer: [32]u8 = undefined;
    const failed_export_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"4\",\"format\":\"plain\"}}",
        .{identifierText(attachment_id, &failed_export_attachment_buffer)},
    );
    defer std.testing.allocator.free(failed_export_payload);
    // The control parser needs one root allocation; fail the first handoff allocation.
    var handoff_allocator = std.testing.FailingAllocator.init(std.testing.allocator, .{ .fail_index = 1 });
    const service_allocator = service.allocator;
    service.allocator = handoff_allocator.allocator();
    const failed_request = testRequest(
        &service,
        &connection,
        MessageKind.selection_export,
        .control_json,
        failed_export_payload,
        [_]u8{0x7d} ** 16,
    );
    service.allocator = service_allocator;
    try std.testing.expectError(error.OutOfMemory, failed_request);

    const restored_cancellation = (try runtime.takeSelectionExportCancellation(attachment_id)).?;
    try std.testing.expectEqual(@as(u64, 3), restored_cancellation.export_sequence);
    try std.testing.expectEqual(unannounced_started.byte_length, restored_cancellation.byte_length);
    try std.testing.expect((try runtime.takeSelectionExportCancellation(attachment_id)) == null);
    try std.testing.expect((try runtime.nextSelectionExportChunk(
        std.testing.allocator,
        attachment_id,
    )) == null);
    const retried_handoff = try runtime.startSelectionExport(
        attachment_id,
        4,
        .plain,
    );
    try std.testing.expectEqual(@as(u64, 4), retried_handoff.started.export_sequence);
    _ = try runtime.cancelSelectionExport(attachment_id, 4);
}

fn testHandshake(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    try std.testing.expectEqual(
        MessageKind.hello_accepted,
        try testHandshakeResponseKind(service, connection, client_id),
    );
}

fn testHandshakeResponseKind(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !u16 {
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"{s}\",\"frameBytes\":1048576,\"inputBytes\":2048,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{
            client_id,
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            protocol.current_protocol_patch,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    return (try protocol.validateTransportEnvelope(outgoing.records()[0])).kind;
}

fn testCreatePayload(
    creation_id: []const u8,
    executable: []const u8,
    arguments: []const []const u8,
) ![]u8 {
    return testCreatePayloadWithConfiguration(creation_id, executable, arguments, null);
}

fn testCreatePayloadWithConfiguration(
    creation_id: []const u8,
    executable: []const u8,
    arguments: []const []const u8,
    configuration: ?TerminalConfigurationWire,
) ![]u8 {
    const environment = [_]EnvironmentWire{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    var writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = creation_id,
        .launch = .{
            .arguments = arguments,
            .cellPixelHeight = 16,
            .cellPixelWidth = 8,
            .columns = 80,
            .configuration = configuration,
            .currentDirectory = @as(?[]const u8, null),
            .environment = &environment,
            .executable = executable,
            .rows = 24,
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
    }, .{}, &writer.writer);
    return writer.toOwnedSlice();
}

fn testAttachPayload(
    attachment_id: core_module.AttachmentId,
    terminal_id: core_module.TerminalId,
) ![]u8 {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
}

fn testRawInputPayload(
    attachment_id: core_module.AttachmentId,
    input: []const u8,
) ![]u8 {
    const payload = try std.testing.allocator.alloc(u8, attachment_id.bytes.len + input.len);
    @memcpy(payload[0..attachment_id.bytes.len], &attachment_id.bytes);
    @memcpy(payload[attachment_id.bytes.len..], input);
    return payload;
}

fn testTerminalTargetPayload(terminal_id: core_module.TerminalId) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
}

fn testIdentifier(namespace: u8, index: usize) WireIdentifier {
    var result = [_]u8{0} ** 16;
    result[0] = namespace;
    std.mem.writeInt(u64, result[8..16], @intCast(index + 1), .little);
    return result;
}

fn testId(comptime Id: type, namespace: u8, index: usize) Id {
    return Id.fromBytes(testIdentifier(namespace, index));
}

fn testRequest(
    service: *HostService,
    connection: *ConnectionState,
    kind: u16,
    encoding: protocol.PayloadEncoding,
    payload: []const u8,
    correlation_id: WireIdentifier,
) !Outgoing {
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        kind,
        encoding,
        payload,
        correlation_id,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    return service.handle(connection, record);
}
