const harness = @import("service_harness.zig");
const serviceAggregate = harness.serviceAggregate;
const serviceCore = harness.serviceCore;
const std = harness.std;
const target_builtin = harness.target_builtin;
const control_boundary = harness.control_boundary;
const core_boundary = harness.core_boundary;
const engine_boundary = harness.engine_boundary;
const model_boundary = harness.model_boundary;
const platform_boundary = harness.platform_boundary;
const protocol = harness.protocol;
const semantic_wait = harness.semantic_wait;
const service_boundary = harness.service_boundary;
const subject = harness.subject;
const control_decode = harness.control_decode;
const control_wire = harness.control_wire;
const core_module = harness.core_module;
const pty_process = harness.pty_process;
const terminal_configuration = harness.terminal_configuration;
const terminal_history = harness.terminal_history;
const terminal_runtime = harness.terminal_runtime;
const host_input_admission = harness.host_input_admission;
const host_outgoing = harness.host_outgoing;
const WireIdentifier = harness.WireIdentifier;
const ConnectionState = harness.ConnectionState;
const HostService = harness.HostService;
const Outgoing = harness.Outgoing;
const EnvironmentWire = harness.EnvironmentWire;
const InputFailure = harness.InputFailure;
const InventoryCursorWire = harness.InventoryCursorWire;
const MessageKind = harness.MessageKind;
const OutcomePublicationHandoff = harness.OutcomePublicationHandoff;
const RequestDispatcher = harness.RequestDispatcher;
const TerminalConfigurationWire = harness.TerminalConfigurationWire;
const beginShutdown = harness.beginShutdown;
const commitOutcomePublicationHandoff = harness.commitOutcomePublicationHandoff;
const createModel = harness.createModel;
const identifierText = harness.identifierText;
const inputFailure = harness.inputFailure;
const protocolErrorCode = harness.protocolErrorCode;
const scheduleEgress = harness.scheduleEgress;
const setNextProjection = harness.setNextProjection;
const synchronizeRuntimeLifecycle = harness.synchronizeRuntimeLifecycle;
const findRuntime = harness.findRuntime;
const activeConnection = harness.activeConnection;
const expectSingleFailureCode = harness.expectSingleFailureCode;
const expectInputAcceptedBytes = harness.expectInputAcceptedBytes;
const parseTerminalInput = harness.parseTerminalInput;
const inputEntryCount = harness.inputEntryCount;
const testClientId = harness.testClientId;
const testActiveConnectionState = harness.testActiveConnectionState;
const retainFailedTerminal = harness.retainFailedTerminal;
const publishPendingCoreOutcome = harness.publishPendingCoreOutcome;
const fillOutgoingLeaving = harness.fillOutgoingLeaving;
const disposeSkippedTargetThroughTransport = harness.disposeSkippedTargetThroughTransport;
const TerminationBarrierGate = harness.TerminationBarrierGate;
const TerminationBarrierSignal = harness.TerminationBarrierSignal;
const aggregate_module = harness.aggregate_module;
const PausingTerminationDispatcher = harness.PausingTerminationDispatcher;
const NoopTerminationFencer = harness.NoopTerminationFencer;
const NoopTerminationContainment = harness.NoopTerminationContainment;
const TerminationRequestOutcome = harness.TerminationRequestOutcome;
const driveIdentityReleasedTerminationBarrier = harness.driveIdentityReleasedTerminationBarrier;
const syntheticIoRequestContract = harness.syntheticIoRequestContract;
const testRequestWithContract = harness.testRequestWithContract;
const expectContractSuccess = harness.expectContractSuccess;
const publishedClientInputCount = harness.publishedClientInputCount;
const onlyTerminalFrame = harness.onlyTerminalFrame;
const testHandshake = harness.testHandshake;
const testHandshakeWithInputBytes = harness.testHandshakeWithInputBytes;
const testHandshakeResponseKind = harness.testHandshakeResponseKind;
const testHandshakeResponseKindWithInputBytes = harness.testHandshakeResponseKindWithInputBytes;
const testCreatePayload = harness.testCreatePayload;
const testCreatePayloadWithConfiguration = harness.testCreatePayloadWithConfiguration;
const testAttachPayload = harness.testAttachPayload;
const testRawInputPayload = harness.testRawInputPayload;
const SkippedNativeTarget = harness.SkippedNativeTarget;
const createSkippedNativeTarget = harness.createSkippedNativeTarget;
const testTerminalTargetPayload = harness.testTerminalTargetPayload;
const testIdentifier = harness.testIdentifier;
const testId = harness.testId;
const testRequest = harness.testRequest;

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
        for (serviceCore(&service).terminalValues()) |session| {
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    try semantic_wait.runtimeText(
        findRuntime(&service, terminal_id).?,
        std.testing.allocator,
        "ready",
    );
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
        serviceCore(&service).findTerminal(terminal_id).?.terminationRequest(),
    );

    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        serviceCore(&service).findTerminal(terminal_id).?.phase(),
    );
}
test "graceful termination after identity release never signals a reused session" {
    if (@import("builtin").os.tag == .windows) return error.SkipZigTest;
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    const arguments = [_][]const u8{ "-c", "sleep 30" };
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    try driveIdentityReleasedTerminationBarrier(
        &service,
        &connection,
        terminal_id,
        aggregate_module.TerminationMode.graceful,
    );
}
test "force termination after identity release never signals a reused session" {
    if (@import("builtin").os.tag == .windows) return error.SkipZigTest;
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "22222222222222222222222222222222");

    const arguments = [_][]const u8{ "-c", "sleep 30" };
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    try driveIdentityReleasedTerminationBarrier(
        &service,
        &connection,
        terminal_id,
        aggregate_module.TerminationMode.force,
    );
}
test "malformed request receives a correlated typed failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try serviceCore(&service).connect(active.connection_id, testClientId());
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.create_terminal,
        .control_json,
        "{",
        correlation,
        serviceCore(&service).epoch.bytes,
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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    service.disconnect(&connection);
}
test "stale HostEpoch receives a correlated failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try serviceCore(&service).connect(active.connection_id, testClientId());
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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    service.disconnect(&connection);
}
test "unknown optional request is skipped without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try serviceCore(&service).connect(active.connection_id, testClientId());
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.ping,
        .control_json,
        "{}",
        correlation,
        serviceCore(&service).epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    std.mem.writeInt(u16, record[20..22], 200, .little);
    std.mem.writeInt(u16, record[6..8], 1, .little);

    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    service.disconnect(&connection);
}
test "registered message roles and encodings fail before command dispatch" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection = testActiveConnectionState();
    const active = try activeConnection(&connection);
    _ = try serviceCore(&service).connect(active.connection_id, testClientId());

    const output_kind_record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.ping,
        .control_json,
        "{}",
        [_]u8{0x61} ** 16,
        serviceCore(&service).epoch.bytes,
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
        serviceCore(&service).epoch.bytes,
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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const retained_runtime = findRuntime(&service, terminal_id).?;

    service.disconnect(&first_connection);
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
    try std.testing.expect(findRuntime(&service, terminal_id) == retained_runtime);
    try std.testing.expectEqual(
        core_module.TerminationRequest.none,
        serviceCore(&service).findTerminal(terminal_id).?.terminationRequest(),
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
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
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
        serviceCore(&service).findTerminal(terminal_id).?.observedGeometry().columns,
    );
    try std.testing.expectEqual(
        @as(u16, 30),
        serviceCore(&service).findTerminal(terminal_id).?.observedGeometry().rows,
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

    var exit_events = try semantic_wait.terminalExits(
        &service,
        &second_connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
    var observed_exit_event = false;
    for (exit_events.records()) |event_record| {
        const event = try protocol.validateTransportEnvelope(event_record);
        observed_exit_event = observed_exit_event or event.kind == MessageKind.terminal_exited;
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, serviceCore(&service).findTerminal(terminal_id).?.phase());
    try std.testing.expect(observed_exit_event);
    try std.testing.expect(
        serviceCore(&service).findTerminal(terminal_id).?.observedRevision() >= 2,
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
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) == null);
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];

    var first_final_revision: u64 = 0;
    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
    for (exit_events.records()) |event_record| {
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
    try std.testing.expectEqual(core_module.TerminalPhase.exited, serviceCore(&service).findTerminal(terminal_id).?.phase());
    try std.testing.expect(first_final_revision > 0);
    const settled = serviceCore(&service).findTerminal(terminal_id).?;
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
    const retained = serviceCore(&service).findTerminal(terminal_id).?;
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
