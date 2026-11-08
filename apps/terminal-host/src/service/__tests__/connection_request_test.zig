const harness = @import("service_harness.zig");
const serviceAggregate = harness.serviceAggregate;
const serviceCore = harness.serviceCore;
const expectCompositionInvariants = harness.expectCompositionInvariants;
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
const host_response = service_boundary.testing.response;
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
const registeredRequestContract = harness.registeredRequestContract;
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
const testTerminalThemePayload = harness.testTerminalThemePayload;
const SkippedNativeTarget = harness.SkippedNativeTarget;
const createSkippedNativeTarget = harness.createSkippedNativeTarget;
const testTerminalTargetPayload = harness.testTerminalTargetPayload;
const testIdentifier = harness.testIdentifier;
const testId = harness.testId;
const testRequest = harness.testRequest;

test "handshake binds client and negotiates input bytes" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"22222222222222222222222222222222\",\"frameBytes\":1024,\"inputBytes\":2048,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{
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
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    const expected = try host_response.resolveSuccessResponse(
        try registeredRequestContract(MessageKind.hello),
    );
    try std.testing.expectEqual(expected.kind, response.kind);
    try std.testing.expectEqual(@intFromEnum(expected.encoding), response.payload_encoding);
    try std.testing.expect(
        std.mem.indexOf(u8, response.payload, "\"hostVersion\":\"0.1.0\"") != null,
    );
    const active = try activeConnection(&connection);
    try std.testing.expect(serviceCore(&service).connectionClientId(active.connection_id).?.eql(testClientId()));
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
    const unsupported_patch = protocol.current_protocol_patch + 1;
    const cases = [_]struct {
        protocol_patch: u16,
        response_kind: u16,
        failure_detail: []const u8,
    }{
        .{
            .protocol_patch = protocol.current_protocol_patch,
            .response_kind = MessageKind.typed_failure,
            .failure_detail = "\"code\":\"handshake-required\"",
        },
        .{
            .protocol_patch = unsupported_patch,
            .response_kind = MessageKind.protocol_rejected,
            .failure_detail = "\"reason\":\"version-mismatch\"",
        },
    };

    for (cases) |case| {
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
        record[22] = @intFromEnum(protocol.PayloadEncoding.raw_bytes);
        std.mem.writeInt(u16, record[56..58], case.protocol_patch, .little);

        var outgoing = try service.handle(&connection, record);
        defer outgoing.deinit();
        try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
        const response = try protocol.validateTransportEnvelope(outgoing.records()[0]);
        try std.testing.expectEqual(case.response_kind, response.kind);
        try std.testing.expect(
            std.mem.indexOf(u8, response.payload, case.failure_detail) != null,
        );
        try std.testing.expect(connection.shouldClose());
    }
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
test "terminal acknowledgement honors a schema-compatible success reassociation" {
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments: []const []const u8 = if (target_builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "24242424242424242424242424242424",
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
    try testHandshake(&service, &connection, "23232323232323232323232323232323");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x24} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const terminate_payload = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(terminate_payload);
    var reassociated = try registeredRequestContract(MessageKind.terminate_terminal);
    reassociated.success_kind = MessageKind.force_terminate_terminal;
    const correlation_id = [_]u8{0x25} ** 16;

    var outgoing = try testRequestWithContract(
        &service,
        &connection,
        reassociated,
        MessageKind.terminate_terminal,
        terminate_payload,
        correlation_id,
    );
    defer outgoing.deinit();
    const envelope = try expectContractSuccess(&outgoing, reassociated, correlation_id);
    const TerminalAcknowledged = struct {
        terminalId: []const u8,
    };
    const acknowledgement = try control_wire.decode(
        TerminalAcknowledged,
        std.testing.allocator,
        envelope.payload,
    );
    defer acknowledgement.deinit();
    var terminal_buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        identifierText(terminal_id, &terminal_buffer),
        acknowledgement.value.terminalId,
    );
}
test "theme request returns a complete recoverable snapshot with replay and stale outcomes" {
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments: []const []const u8 = if (target_builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const create_payload = try testCreatePayload(
        "34343434343434343434343434343434",
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
    try testHandshake(&service, &connection, "33333333333333333333333333333333");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x34} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const operation_id = model_boundary.identifiers.TerminalThemeOperationId.fromBytes(
        [_]u8{0x35} ** 16,
    );
    const palette = [_]u32{0x22_33_44} ** 256;
    const theme: model_boundary.theme.Theme = .{
        .color_scheme = .light,
        .default_background_rgb = 0xee_ee_ee,
        .default_cursor_rgb = 0x12_34_56,
        .default_foreground_rgb = 0x11_11_11,
        .palette = palette,
    };
    const applied_payload = try testTerminalThemePayload(operation_id, terminal_id, 0, theme);
    defer std.testing.allocator.free(applied_payload);

    var applied_response = try testRequest(
        &service,
        &connection,
        MessageKind.apply_terminal_theme,
        .control_json,
        applied_payload,
        [_]u8{0x36} ** 16,
    );
    defer applied_response.deinit();
    const applied_envelope = try protocol.validateTransportEnvelope(applied_response.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_theme_result, applied_envelope.kind);
    const ThemeResult = struct {
        current: struct {
            operationId: ?[]const u8,
            revision: []const u8,
            terminal: struct {
                colorScheme: []const u8,
                defaultBackgroundRgb: u32,
                defaultCursorRgb: ?u32,
                defaultForegroundRgb: u32,
                palette: []const u32,
            },
        },
        operationId: []const u8,
        terminalId: []const u8,
        transition: []const u8,
    };
    const applied = try control_wire.decode(
        ThemeResult,
        std.testing.allocator,
        applied_envelope.payload,
    );
    defer applied.deinit();
    try std.testing.expectEqualStrings("applied", applied.value.transition);
    try std.testing.expectEqualStrings("1", applied.value.current.revision);
    try std.testing.expectEqualStrings("light", applied.value.current.terminal.colorScheme);
    try std.testing.expectEqual(theme.default_background_rgb, applied.value.current.terminal.defaultBackgroundRgb);
    try std.testing.expectEqual(theme.default_cursor_rgb, applied.value.current.terminal.defaultCursorRgb);
    try std.testing.expectEqual(theme.default_foreground_rgb, applied.value.current.terminal.defaultForegroundRgb);
    try std.testing.expectEqualSlices(u32, &theme.palette, applied.value.current.terminal.palette);

    const after_apply = findRuntime(&service, terminal_id).?.geometry();
    var replay_response = try testRequest(
        &service,
        &connection,
        MessageKind.apply_terminal_theme,
        .control_json,
        applied_payload,
        [_]u8{0x37} ** 16,
    );
    defer replay_response.deinit();
    const replay_envelope = try protocol.validateTransportEnvelope(replay_response.records()[0]);
    try std.testing.expect(std.mem.indexOf(u8, replay_envelope.payload, "\"transition\":\"already-applied\"") != null);
    try std.testing.expectEqualDeep(after_apply, findRuntime(&service, terminal_id).?.geometry());

    const stale_payload = try testTerminalThemePayload(
        model_boundary.identifiers.TerminalThemeOperationId.fromBytes([_]u8{0x38} ** 16),
        terminal_id,
        0,
        model_boundary.theme.default,
    );
    defer std.testing.allocator.free(stale_payload);
    var stale_response = try testRequest(
        &service,
        &connection,
        MessageKind.apply_terminal_theme,
        .control_json,
        stale_payload,
        [_]u8{0x39} ** 16,
    );
    defer stale_response.deinit();
    const stale_envelope = try protocol.validateTransportEnvelope(stale_response.records()[0]);
    try std.testing.expect(std.mem.indexOf(u8, stale_envelope.payload, "\"transition\":\"stale\"") != null);

    var inventory = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        "{\"cursor\":null,\"maximumTerminals\":1}",
        [_]u8{0x3a} ** 16,
    );
    defer inventory.deinit();
    const inventory_envelope = try protocol.validateTransportEnvelope(inventory.records()[0]);
    try std.testing.expect(std.mem.indexOf(u8, inventory_envelope.payload, "\"theme\":{\"operationId\":\"35353535353535353535353535353535\",\"revision\":\"1\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, inventory_envelope.payload, "\"defaultCursorRgb\":1193046") != null);
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
        _ = try serviceCore(&service).connect(active.connection_id, testClientId());

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
            serviceCore(&service).terminalCount(),
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
            serviceCore(&service).connectionCount(),
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
    const defaults_before = serviceCore(&service).terminalDefaults();
    const revision_before = serviceCore(&service).terminalDefaultsRevision();

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
    try std.testing.expectEqualDeep(
        defaults_before,
        serviceCore(&service).terminalDefaults(),
    );
    try std.testing.expectEqual(
        revision_before,
        serviceCore(&service).terminalDefaultsRevision(),
    );
}
test "IPC updates terminal defaults and retained terminal configuration" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
    try std.testing.expectEqual(
        @as(usize, 25_000_000),
        serviceCore(&service).terminalDefaults().scrollback_bytes,
    );

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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    try std.testing.expectEqual(
        @as(usize, 25_000_000),
        serviceCore(&service).findTerminal(terminal_id).?.effectiveConfiguration().scrollback_bytes,
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
    const explicit_terminal_id = serviceCore(&service).terminalKeys()[1];
    try std.testing.expectEqual(
        @as(usize, 5_000_000),
        serviceCore(&service).findTerminal(explicit_terminal_id).?.effectiveConfiguration().scrollback_bytes,
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
        serviceCore(&service).findTerminal(terminal_id).?.effectiveConfiguration().scrollback_bytes,
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
        serviceCore(&service).terminalDefaults().scrollback_bytes,
    );
    try std.testing.expectEqual(
        @as(u64, 1),
        serviceCore(&service).terminalDefaultsRevision(),
    );
    try expectCompositionInvariants(&service, &connection);
}
test "idle stop requires both the final disconnect and an empty terminal registry" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    try std.testing.expect(service.canStopWhenIdle());

    const connection_id: core_module.ConnectionId = 7;
    _ = try serviceCore(&service).connect(
        connection_id,
        core_module.ClientId.fromBytes([_]u8{0x21} ** 16),
    );
    try std.testing.expect(!service.canStopWhenIdle());
    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x23} ** 16);
    _ = try createModel(
        serviceCore(&service),
        connection_id,
        core_module.CreationId.fromBytes([_]u8{0x22} ** 16),
        terminal_id,
        [_]u8{0x24} ** 32,
        80,
        24,
    );
    _ = serviceCore(&service).disconnect(connection_id);
    try std.testing.expect(!service.canStopWhenIdle());

    _ = try serviceCore(&service).connect(
        connection_id,
        core_module.ClientId.fromBytes([_]u8{0x21} ** 16),
    );
    _ = try serviceCore(&service).childExited(terminal_id, .{ .exit_code = 0 });
    _ = try serviceCore(&service).ptyDrained(terminal_id);
    try publishPendingCoreOutcome(&service, terminal_id);
    _ = try serviceCore(&service).disposeTerminal(connection_id, terminal_id);
    _ = serviceCore(&service).disconnect(connection_id);
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
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).connectionCount());
    try std.testing.expect(
        serviceCore(&service).connectionClientId(first_active.connection_id).?.eql(
            core_module.ClientId.fromBytes([_]u8{0x22} ** 16),
        ),
    );

    service.disconnect(&first_connection);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
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
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        testId(core_module.CreationId, 0x74, 0),
        terminal_id,
        [_]u8{0x75} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        first_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        second_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );

    service.disconnect(&connection);
    try std.testing.expect(serviceCore(&service).findAttachment(first_attachment_id) == null);
    try std.testing.expect(serviceCore(&service).findAttachment(second_attachment_id) == null);
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
}
test "broker loss revokes authority immediately and a replacement broker stays serviceable" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    var replacement: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    defer service.disconnect(&replacement);

    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const owner_active = try activeConnection(&owner);
    const terminal_id = testId(core_module.TerminalId, 0x81, 0);
    _ = try createModel(
        serviceCore(&service),
        owner_active.connection_id,
        testId(core_module.CreationId, 0x82, 0),
        terminal_id,
        [_]u8{0x83} ** 32,
        80,
        24,
    );
    const read_attachment = testId(core_module.AttachmentId, 0x84, 0);
    _ = try serviceCore(&service).attach(
        owner_active.connection_id,
        read_attachment,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    _ = try serviceCore(&service).setProjectionDemand(
        owner_active.connection_id,
        read_attachment,
        .active,
    );
    // Input-only attachments are the trigger for THCR-026's quadratic scan.
    for (0..8) |index| {
        _ = try serviceCore(&service).attach(
            owner_active.connection_id,
            testId(core_module.AttachmentId, 0x90, index),
            terminal_id,
            .{ .input = true, .read = false, .resize = false },
            false,
        );
    }

    const aggregate_testing = service_boundary.testing.runtime_aggregate.testing;
    core_module.testing.resetActiveReadDemandScans(serviceCore(&service));
    aggregate_testing.resetProjectionReleaseDecisions(serviceAggregate(&service));

    // Broker loss revokes connection authority immediately and retains the
    // terminal; the bounded teardown performs zero demand-list scans and
    // releases projection resources at most once for the terminal.
    service.disconnect(&owner);
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).attachmentCount());
    try std.testing.expect(serviceCore(&service).findAttachment(read_attachment) == null);
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
    try std.testing.expect(!serviceCore(&service).hasActiveProjectionDemand(terminal_id));
    try std.testing.expectEqual(
        @as(usize, 0),
        core_module.testing.activeReadDemandScans(serviceCore(&service)),
    );
    try std.testing.expect(
        aggregate_testing.projectionReleaseDecisions(serviceAggregate(&service)) <= 1,
    );

    // A bounded maintenance pass completes without disturbing retained state.
    service.maintain();
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).connectionCount());
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);

    // A replacement broker attaches fresh state to the same retained terminal
    // and exercises authority without old cleanup corrupting the new session.
    try testHandshake(&service, &replacement, "22222222222222222222222222222222");
    const replacement_active = try activeConnection(&replacement);
    const new_attachment = testId(core_module.AttachmentId, 0x95, 0);
    _ = try serviceCore(&service).attach(
        replacement_active.connection_id,
        new_attachment,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    _ = try serviceCore(&service).setProjectionDemand(
        replacement_active.connection_id,
        new_attachment,
        .active,
    );
    try std.testing.expect(serviceCore(&service).hasActiveProjectionDemand(terminal_id));
    _ = try serviceCore(&service).authorizeInput(
        replacement_active.connection_id,
        new_attachment,
        1,
    );
    try std.testing.expect(serviceCore(&service).findAttachment(new_attachment) != null);
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
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
    _ = try createModel(
        serviceCore(&service),
        owner_active.connection_id,
        creation_id,
        terminal_id,
        [_]u8{0x44} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
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
    try std.testing.expect(serviceCore(&service).findAttachment(attachment_id) == null);
}
test "detaching a resize owner publishes its observer promotion before acknowledgement" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const owner_active = try activeConnection(&owner);

    const creation_id = core_module.CreationId.fromBytes([_]u8{0x41} ** 16);
    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x42} ** 16);
    const resize_owner_id = core_module.AttachmentId.fromBytes([_]u8{0x43} ** 16);
    const observer_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    _ = try createModel(
        serviceCore(&service),
        owner_active.connection_id,
        creation_id,
        terminal_id,
        [_]u8{0x45} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
        owner_active.connection_id,
        resize_owner_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    _ = try serviceCore(&service).attach(
        owner_active.connection_id,
        observer_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );

    var detached = try testRequest(
        &service,
        &owner,
        MessageKind.detach_terminal,
        .control_json,
        "{\"attachmentId\":\"43434343434343434343434343434343\"}",
        [_]u8{0x46} ** 16,
    );
    defer detached.deinit();

    try std.testing.expectEqual(@as(usize, 2), detached.records().len);
    const promotion = try protocol.validateTransportEnvelope(detached.records()[0]);
    try std.testing.expectEqual(MessageKind.resize_lease_changed, promotion.kind);
    try std.testing.expect(promotion.event);
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"44444444444444444444444444444444\",\"resizeLease\":true,\"terminalId\":\"42424242424242424242424242424242\"}",
        promotion.payload,
    );
    const acknowledgement = try protocol.validateTransportEnvelope(detached.records()[1]);
    try std.testing.expectEqual(MessageKind.detach_terminal, acknowledgement.kind);
    try std.testing.expect(acknowledgement.response);
    try std.testing.expect(serviceCore(&service).findAttachment(observer_id).?.resize_lease);
}
test "claiming the resize lease publishes revoke then grant before the complete response" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const active = try activeConnection(&owner);

    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x42} ** 16);
    const original_owner_id = core_module.AttachmentId.fromBytes([_]u8{0x43} ** 16);
    const claimant_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        core_module.CreationId.fromBytes([_]u8{0x41} ** 16),
        terminal_id,
        [_]u8{0x45} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        original_owner_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        claimant_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );

    const claim_payload = "{\"attachmentId\":\"44444444444444444444444444444444\"}";
    var claimed = try testRequest(
        &service,
        &owner,
        MessageKind.claim_resize_lease,
        .control_json,
        claim_payload,
        [_]u8{0x46} ** 16,
    );
    defer claimed.deinit();

    try std.testing.expectEqual(@as(usize, 3), claimed.records().len);
    const revoked = try protocol.validateTransportEnvelope(claimed.records()[0]);
    try std.testing.expectEqual(MessageKind.resize_lease_changed, revoked.kind);
    try std.testing.expect(revoked.event);
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"43434343434343434343434343434343\",\"resizeLease\":false,\"terminalId\":\"42424242424242424242424242424242\"}",
        revoked.payload,
    );
    const granted = try protocol.validateTransportEnvelope(claimed.records()[1]);
    try std.testing.expectEqual(MessageKind.resize_lease_changed, granted.kind);
    try std.testing.expect(granted.event);
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"44444444444444444444444444444444\",\"resizeLease\":true,\"terminalId\":\"42424242424242424242424242424242\"}",
        granted.payload,
    );
    const response = try protocol.validateTransportEnvelope(claimed.records()[2]);
    try std.testing.expectEqual(MessageKind.attachment_capabilities, response.kind);
    try std.testing.expect(response.response);
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"44444444444444444444444444444444\",\"capabilities\":{\"input\":true,\"read\":true,\"resize\":true},\"capabilityRevision\":\"1\",\"resizeLease\":true,\"terminalId\":\"42424242424242424242424242424242\"}",
        response.payload,
    );
    try std.testing.expect(!serviceCore(&service).findAttachment(original_owner_id).?.resize_lease);
    try std.testing.expect(serviceCore(&service).findAttachment(claimant_id).?.resize_lease);

    var replayed = try testRequest(
        &service,
        &owner,
        MessageKind.claim_resize_lease,
        .control_json,
        claim_payload,
        [_]u8{0x47} ** 16,
    );
    defer replayed.deinit();
    try std.testing.expectEqual(@as(usize, 1), replayed.records().len);
    try std.testing.expectEqual(
        MessageKind.attachment_capabilities,
        (try protocol.validateTransportEnvelope(replayed.records()[0])).kind,
    );
}
test "resize lease claim reservation failure preserves the current owner" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const active = try activeConnection(&owner);

    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x42} ** 16);
    const original_owner_id = core_module.AttachmentId.fromBytes([_]u8{0x43} ** 16);
    const claimant_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        core_module.CreationId.fromBytes([_]u8{0x41} ** 16),
        terminal_id,
        [_]u8{0x45} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        original_owner_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        claimant_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );

    const correlation_id = [_]u8{0x46} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.claim_resize_lease,
        .control_json,
        "{\"attachmentId\":\"44444444444444444444444444444444\"}",
        correlation_id,
        serviceCore(&service).epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    const envelope = try protocol.validateTransportEnvelope(record);
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const maximum_control_record_bytes = protocol.transport_envelope_header_bytes +
        protocol.Limits.control_json_bytes;
    try fillOutgoingLeaving(
        &outgoing,
        serviceCore(&service).epoch,
        maximum_control_record_bytes * 2,
    );
    var dispatcher = RequestDispatcher.init(
        service.allocator,
        &service.runtime_aggregate,
        service.trace_interactions,
        .open,
    );

    try std.testing.expectError(
        error.OutgoingCapacityInvariant,
        dispatcher.dispatch(
            try registeredRequestContract(MessageKind.claim_resize_lease),
            active.connection_id,
            .{
                .input_bytes = active.limits.input_bytes,
                .row_window_bytes = active.limits.row_window_bytes,
            },
            envelope,
            correlation_id,
            &outgoing,
        ),
    );

    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    try std.testing.expect(serviceCore(&service).findAttachment(original_owner_id).?.resize_lease);
    try std.testing.expect(!serviceCore(&service).findAttachment(claimant_id).?.resize_lease);
    try expectCompositionInvariants(&service, &owner);
}
test "resize lease claim event allocation failures return typed failure without mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var owner: ConnectionState = .awaiting_hello;
    defer service.disconnect(&owner);
    try testHandshake(&service, &owner, "22222222222222222222222222222222");
    const active = try activeConnection(&owner);

    const terminal_id = core_module.TerminalId.fromBytes([_]u8{0x42} ** 16);
    const original_owner_id = core_module.AttachmentId.fromBytes([_]u8{0x43} ** 16);
    const claimant_id = core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16);
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        core_module.CreationId.fromBytes([_]u8{0x41} ** 16),
        terminal_id,
        [_]u8{0x45} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        original_owner_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    _ = try serviceCore(&service).attach(
        active.connection_id,
        claimant_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        false,
    );

    const correlation_id = [_]u8{0x46} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.claim_resize_lease,
        .control_json,
        "{\"attachmentId\":\"44444444444444444444444444444444\"}",
        correlation_id,
        serviceCore(&service).epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    const envelope = try protocol.validateTransportEnvelope(record);
    const original_claim_sequence = serviceCore(&service).findAttachment(original_owner_id).?.resize_claim_sequence;
    const claimant_claim_sequence = serviceCore(&service).findAttachment(claimant_id).?.resize_claim_sequence;
    const geometry = serviceCore(&service).findTerminal(terminal_id).?.observedGeometry();

    for ([_]usize{ 3, 4 }) |fail_index| {
        var failing = std.testing.FailingAllocator.init(
            std.testing.allocator,
            .{ .fail_index = fail_index },
        );
        var outgoing = Outgoing.init(failing.allocator());
        defer outgoing.deinit();
        var dispatcher = RequestDispatcher.init(
            service.allocator,
            &service.runtime_aggregate,
            service.trace_interactions,
            .open,
        );

        try dispatcher.dispatch(
            try registeredRequestContract(MessageKind.claim_resize_lease),
            active.connection_id,
            .{
                .input_bytes = active.limits.input_bytes,
                .row_window_bytes = active.limits.row_window_bytes,
            },
            envelope,
            correlation_id,
            &outgoing,
        );

        try expectSingleFailureCode(&outgoing, "resource-exhausted");
        try std.testing.expect((try activeConnection(&owner)).connection_id == active.connection_id);
        try std.testing.expect(serviceCore(&service).findAttachment(original_owner_id).?.resize_lease);
        try std.testing.expect(!serviceCore(&service).findAttachment(claimant_id).?.resize_lease);
        try std.testing.expectEqual(
            original_claim_sequence,
            serviceCore(&service).findAttachment(original_owner_id).?.resize_claim_sequence,
        );
        try std.testing.expectEqual(
            claimant_claim_sequence,
            serviceCore(&service).findAttachment(claimant_id).?.resize_claim_sequence,
        );
        try std.testing.expectEqual(
            geometry,
            serviceCore(&service).findTerminal(terminal_id).?.observedGeometry(),
        );
        try expectCompositionInvariants(&service, &owner);
    }
}
test "current protocol rejects a cursor made stale by disposal and freshly pages exact inventory" {
    const inventory_count = 130;
    const fresh_page_size = 17;
    const TestInventoryTerminal = struct {
        identity: struct {
            terminalId: []const u8,
        },
    };
    const TestInventoryPage = struct {
        inventoryGeneration: []const u8,
        nextCursor: ?InventoryCursorWire,
        terminals: []TestInventoryTerminal,
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
    var terminal_ids: [inventory_count]core_module.TerminalId = undefined;
    terminal_ids[0] = replay_terminal_id;
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        replay_creation_id,
        replay_terminal_id,
        replay_request.value.launch.launch_digest,
        80,
        24,
    );
    for (1..inventory_count) |index| {
        const creation_id = testId(core_module.CreationId, 0x31, index);
        const terminal_id = testId(core_module.TerminalId, 0x32, index);
        terminal_ids[index] = terminal_id;
        _ = try createModel(
            serviceCore(&service),
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
        .{ .ignore_unknown_fields = true },
    );
    defer first_page.deinit();
    try std.testing.expect(first_page.value.terminals.len > 0);
    try std.testing.expect(first_page.value.terminals.len < 128);
    try std.testing.expect(first_page.value.nextCursor != null);
    try std.testing.expectEqual(
        first_page.value.terminals.len,
        try std.fmt.parseUnsigned(usize, first_page.value.nextCursor.?.offset, 10),
    );

    const second_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"cursor\":{{\"inventoryGeneration\":\"{s}\",\"offset\":\"{s}\"}},\"maximumTerminals\":128}}",
        .{
            first_page.value.nextCursor.?.inventoryGeneration,
            first_page.value.nextCursor.?.offset,
        },
    );
    defer std.testing.allocator.free(second_payload);

    _ = try serviceCore(&service).childExited(
        replay_terminal_id,
        .{ .exit_code = 0 },
    );
    _ = try serviceCore(&service).ptyDrained(replay_terminal_id);
    try publishPendingCoreOutcome(&service, replay_terminal_id);
    const generation_before_disposal = serviceCore(&service).inventoryGeneration();
    try std.testing.expectEqual(
        .applied,
        try serviceCore(&service).disposeTerminal(
            active.connection_id,
            replay_terminal_id,
        ),
    );
    try std.testing.expectEqual(
        generation_before_disposal + 1,
        serviceCore(&service).inventoryGeneration(),
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
    try expectSingleFailureCode(&stale_response, "stale-inventory-cursor");

    const fresh_generation = serviceCore(&service).inventoryGeneration();
    var seen = [_]bool{false} ** inventory_count;
    var next_offset: ?usize = 0;
    var page_count: usize = 0;
    var terminal_count: usize = 0;
    while (next_offset) |offset| {
        const fresh_payload = if (offset == 0)
            try std.fmt.allocPrint(
                std.testing.allocator,
                "{{\"cursor\":null,\"maximumTerminals\":{d}}}",
                .{fresh_page_size},
            )
        else
            try std.fmt.allocPrint(
                std.testing.allocator,
                "{{\"cursor\":{{\"inventoryGeneration\":\"{d}\",\"offset\":\"{d}\"}},\"maximumTerminals\":{d}}}",
                .{ fresh_generation, offset, fresh_page_size },
            );
        defer std.testing.allocator.free(fresh_payload);
        var fresh_response = try testRequest(
            &service,
            &connection,
            MessageKind.list_terminals,
            .control_json,
            fresh_payload,
            testIdentifier(0x50, page_count),
        );
        defer fresh_response.deinit();
        try std.testing.expectEqual(@as(usize, 1), fresh_response.records().len);
        const fresh_envelope = try protocol.validateTransportEnvelope(
            fresh_response.records()[0],
        );
        try std.testing.expectEqual(MessageKind.list_terminals, fresh_envelope.kind);
        const fresh_page = try std.json.parseFromSlice(
            TestInventoryPage,
            std.testing.allocator,
            fresh_envelope.payload,
            .{ .ignore_unknown_fields = true },
        );
        defer fresh_page.deinit();
        try std.testing.expectEqual(
            fresh_generation,
            try std.fmt.parseUnsigned(u64, fresh_page.value.inventoryGeneration, 10),
        );
        try std.testing.expect(fresh_page.value.terminals.len > 0);

        for (fresh_page.value.terminals) |terminal| {
            var matched_index: ?usize = null;
            for (terminal_ids, 0..) |expected_terminal_id, index| {
                var terminal_buffer: [32]u8 = undefined;
                if (std.mem.eql(
                    u8,
                    terminal.identity.terminalId,
                    identifierText(expected_terminal_id, &terminal_buffer),
                )) {
                    matched_index = index;
                    break;
                }
            }
            const index = matched_index orelse return error.TestUnexpectedResult;
            try std.testing.expect(index != 0);
            try std.testing.expect(!seen[index]);
            seen[index] = true;
            terminal_count += 1;
        }

        page_count += 1;
        next_offset = if (fresh_page.value.nextCursor) |cursor| cursor_offset: {
            try std.testing.expectEqual(
                fresh_generation,
                try std.fmt.parseUnsigned(u64, cursor.inventoryGeneration, 10),
            );
            const parsed_offset = try std.fmt.parseUnsigned(usize, cursor.offset, 10);
            try std.testing.expect(parsed_offset > offset);
            break :cursor_offset parsed_offset;
        } else null;
    }

    try std.testing.expectEqual(@as(usize, 8), page_count);
    try std.testing.expectEqual(inventory_count - 1, terminal_count);
    try std.testing.expect(!seen[0]);
    for (seen[1..]) |retained_was_seen| {
        try std.testing.expect(retained_was_seen);
    }
}
