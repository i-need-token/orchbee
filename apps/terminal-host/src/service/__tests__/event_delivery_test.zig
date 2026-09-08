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
const commitReaderBatch = harness.commitReaderBatch;
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
const pollEventsChecked = harness.pollEventsChecked;
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
test "DisposeTerminal returns the exact cleanup code for every blocked containment phase" {
    const cases = [_]struct {
        failure: core_module.RuntimeCleanupFailure,
        code: []const u8,
    }{
        .{ .failure = .contain_process, .code = "cleanup-contain-process" },
        .{ .failure = .release_output, .code = "cleanup-release-output" },
        .{ .failure = .release_process_identity, .code = "cleanup-release-process-identity" },
    };
    for (cases, 0..) |case, index| {
        var service = HostService.init(std.testing.allocator);
        defer service.deinit();
        var connection: ConnectionState = .awaiting_hello;
        defer service.disconnect(&connection);
        try testHandshake(&service, &connection, "22222222222222222222222222222222");
        const active = try activeConnection(&connection);
        const terminal_id = testId(core_module.TerminalId, 0x61, index);
        _ = try createModel(
            serviceCore(&service),
            active.connection_id,
            testId(core_module.CreationId, 0x62, index),
            terminal_id,
            [_]u8{0x63} ** 32,
            80,
            24,
        );
        _ = try serviceCore(&service).runtimeFailed(terminal_id, .read_output);
        // A never-due retry deadline keeps the blocked phase stable so the
        // request cannot race the independent maintenance retry.
        _ = try serviceCore(&service).failureContainmentBlocked(
            terminal_id,
            case.failure,
            std.math.maxInt(i128),
        );

        const payload = try testTerminalTargetPayload(terminal_id);
        defer std.testing.allocator.free(payload);
        var rejected = try testRequest(
            &service,
            &connection,
            MessageKind.dispose_terminal,
            .control_json,
            payload,
            testIdentifier(0x64, index),
        );
        defer rejected.deinit();
        try std.testing.expectEqual(@as(usize, 1), rejected.records().len);
        const envelope = try protocol.validateTransportEnvelope(rejected.records()[0]);
        try std.testing.expectEqual(MessageKind.typed_failure, envelope.kind);
        const Failure = struct {
            code: []const u8,
            message: []const u8,
            retryable: bool,
        };
        const failure = try control_wire.decode(
            Failure,
            std.testing.allocator,
            envelope.payload,
        );
        defer failure.deinit();
        try std.testing.expectEqualStrings(case.code, failure.value.code);
        try std.testing.expect(failure.value.retryable);
        try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
    }
}
test "DisposeTerminal keeps incomplete-containment while cleanup is stopping" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try testHandshake(&service, &connection, "22222222222222222222222222222222");
    const active = try activeConnection(&connection);
    const terminal_id = testId(core_module.TerminalId, 0x65, 0);
    _ = try createModel(
        serviceCore(&service),
        active.connection_id,
        testId(core_module.CreationId, 0x66, 0),
        terminal_id,
        [_]u8{0x67} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).runtimeFailed(terminal_id, .read_output);

    const payload = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(payload);
    var rejected = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        payload,
        [_]u8{0x68} ** 16,
    );
    defer rejected.deinit();
    try expectSingleFailureCode(&rejected, "cleanup-incomplete");
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);
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

    var outgoing = try pollEventsChecked(&service, &connection);
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
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id).?.pendingOutcome() == null);

    var duplicate = try pollEventsChecked(&service, &connection);
    defer duplicate.deinit();
    try std.testing.expectEqual(@as(usize, 0), duplicate.records().len);
}
test "private APC application signals cross the Host event scheduler once" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const arguments = [_][]const u8{ "-c", "read value" };
    const create_payload = try testCreatePayload(
        "71717171717171717171717171717171",
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
    try testHandshake(&service, &connection, "72727272727272727272727272727272");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x73} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;
    try std.testing.expectEqual(
        @as(terminal_runtime.testing.ReaderBatchOutcomeType, .committed),
        commitReaderBatch(
            runtime,
            "\x1b_orchbee.agent.v3;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjozfQ\x1b\\",
        ),
    );

    var outgoing = try pollEventsChecked(&service, &connection);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_application_signal, envelope.kind);
    const Signal = struct {
        channel: []const u8,
        identity: struct {
            creationId: []const u8,
            hostEpoch: []const u8,
            terminalId: []const u8,
        },
        payload: []const u8,
        sequence: []const u8,
    };
    const signal = try control_wire.decode(Signal, std.testing.allocator, envelope.payload);
    defer signal.deinit();
    try std.testing.expectEqualStrings("orchbee.agent.v1", signal.value.channel);
    try std.testing.expectEqualStrings(
        "{\"kind\":\"correlation\",\"version\":3}",
        signal.value.payload,
    );
    try std.testing.expectEqualStrings("1", signal.value.sequence);
    try std.testing.expect((try runtime.prepareApplicationSignal(std.testing.allocator)) == null);
}
test "full outgoing batch leaves a failed outcome pending for the next handoff" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection = testActiveConnectionState();
    const connection_id: core_module.ConnectionId = 1;
    _ = try serviceCore(&service).connect(
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
            .{ .event = serviceCore(&service).epoch },
        )) {
            .appended => {},
            .full => return error.ExpectedFillerAppend,
        }
    }
    var blocked_handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &full, &blocked_handoff);
    commitOutcomePublicationHandoff(&blocked_handoff, serviceAggregate(&service));
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id).?.pendingOutcome() != null);
    try std.testing.expectError(
        error.OutcomePublicationPending,
        serviceCore(&service).disposeTerminal(connection_id, terminal_id),
    );
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) != null);

    var retry = try pollEventsChecked(&service, &connection);
    defer retry.deinit();
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    const event = try protocol.validateTransportEnvelope(retry.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_failed, event.kind);
    var terminal_buffer: [32]u8 = undefined;
    try std.testing.expect(
        std.mem.indexOf(
            u8,
            event.payload,
            identifierText(terminal_id, &terminal_buffer),
        ) != null,
    );
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id).?.pendingOutcome() == null);
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try serviceCore(&service).disposeTerminal(connection_id, terminal_id),
    );
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        try serviceCore(&service).disposeTerminal(connection_id, terminal_id),
    );
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) == null);
}
test "DisposeTerminal retains a native exit until its poll handoff commits" {
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "exit 0" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
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
    try testHandshake(&service, &connection, "35353535353535353535353535353535");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x36} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const retained_runtime = findRuntime(&service, terminal_id).?;
    const completion = try retained_runtime.waitForCompletion();
    try std.testing.expectEqual(
        terminal_runtime.CompletionState.normal,
        completion.classification,
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        serviceCore(&service).findTerminal(terminal_id).?.phase(),
    );

    const dispose_payload = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(dispose_payload);
    const generation_before = serviceCore(&service).inventoryGeneration();
    var rejected = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x37} ** 16,
    );
    defer rejected.deinit();
    try expectSingleFailureCode(&rejected, "outcome-publication-pending");
    const rejection_envelope = try protocol.validateTransportEnvelope(rejected.records()[0]);
    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const failure = try control_wire.decode(
        Failure,
        std.testing.allocator,
        rejection_envelope.payload,
    );
    defer failure.deinit();
    try std.testing.expect(failure.value.retryable);
    try std.testing.expectEqual(generation_before, serviceCore(&service).inventoryGeneration());
    try std.testing.expect(findRuntime(&service, terminal_id) == retained_runtime);
    try std.testing.expectEqual(@as(usize, 1), serviceAggregate(&service).runtimeCount());
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id).?.pendingOutcome() != null);

    var events = try pollEventsChecked(&service, &connection);
    defer events.deinit();
    try std.testing.expectEqual(@as(usize, 1), events.records().len);
    const exit_event = try protocol.validateTransportEnvelope(events.records()[0]);
    try std.testing.expectEqual(MessageKind.terminal_exited, exit_event.kind);
    var terminal_buffer: [32]u8 = undefined;
    try std.testing.expect(
        std.mem.indexOf(
            u8,
            exit_event.payload,
            identifierText(terminal_id, &terminal_buffer),
        ) != null,
    );
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id).?.pendingOutcome() == null);
    try std.testing.expect(findRuntime(&service, terminal_id) == retained_runtime);

    var duplicate_events = try pollEventsChecked(&service, &connection);
    defer duplicate_events.deinit();
    for (duplicate_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        // ConPTY may publish its shell title after the exit event. That
        // independent metadata notification must not look like a duplicate exit.
        try std.testing.expectEqual(MessageKind.terminal_metadata_changed, envelope.kind);
    }

    var disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x38} ** 16,
    );
    defer disposed.deinit();
    try std.testing.expectEqual(@as(usize, 1), disposed.records().len);
    try std.testing.expectEqual(
        MessageKind.dispose_terminal,
        (try protocol.validateTransportEnvelope(disposed.records()[0])).kind,
    );
    try std.testing.expectEqual(@as(usize, 0), serviceAggregate(&service).runtimeCount());
    try std.testing.expect(serviceCore(&service).findTerminal(terminal_id) == null);

    var repeated = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x39} ** 16,
    );
    defer repeated.deinit();
    try std.testing.expectEqual(@as(usize, 1), repeated.records().len);
    try std.testing.expectEqual(
        MessageKind.dispose_terminal,
        (try protocol.validateTransportEnvelope(repeated.records()[0])).kind,
    );
    try std.testing.expectEqual(@as(usize, 0), serviceAggregate(&service).runtimeCount());
}
test "DisposeTerminal settles a skipped exact target through HostService" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "37373737373737373737373737373737");
    const active = try activeConnection(&connection);
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "exit 0" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
    const arguments: []const []const u8 = if (target_builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const target = try createSkippedNativeTarget(
        &service,
        active.connection_id,
        executable,
        arguments,
    );
    const completion = try findRuntime(&service, target.terminal_id).?.waitForCompletion();
    try std.testing.expectEqual(
        terminal_runtime.CompletionState.normal,
        completion.classification,
    );
    // The engine has published normal completion, but the bounded background
    // scan has not visited this target, so Core still observes running.
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        serviceCore(&service).findTerminal(target.terminal_id).?.phase(),
    );

    const dispose_payload = try testTerminalTargetPayload(target.terminal_id);
    defer std.testing.allocator.free(dispose_payload);
    var rejected = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x75} ** 16,
    );
    defer rejected.deinit();
    // The exact pass settles the target, so disposal reaches the retryable
    // publication barrier rather than a stale invalid-phase rejection.
    try expectSingleFailureCode(&rejected, "outcome-publication-pending");
    const rejection_envelope = try protocol.validateTransportEnvelope(rejected.records()[0]);
    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const failure = try control_wire.decode(
        Failure,
        std.testing.allocator,
        rejection_envelope.payload,
    );
    defer failure.deinit();
    try std.testing.expect(failure.value.retryable);
    try std.testing.expect(serviceCore(&service).findTerminal(target.terminal_id) != null);

    // The bounded outcome scan also skips this target once, so poll until the
    // settled outcome event is handed off and its publication commits.
    var exit_event_handed_off = false;
    for (0..4) |_| {
        var events = try pollEventsChecked(&service, &connection);
        defer events.deinit();
        for (events.records()) |record| {
            const envelope = try protocol.validateTransportEnvelope(record);
            if (envelope.kind == MessageKind.terminal_exited) {
                exit_event_handed_off = true;
            }
        }
        if (exit_event_handed_off) break;
    }
    try std.testing.expect(exit_event_handed_off);
    try std.testing.expect(serviceCore(&service).findTerminal(target.terminal_id) != null);

    var disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x76} ** 16,
    );
    defer disposed.deinit();
    try std.testing.expectEqual(@as(usize, 1), disposed.records().len);
    try std.testing.expectEqual(
        MessageKind.dispose_terminal,
        (try protocol.validateTransportEnvelope(disposed.records()[0])).kind,
    );
    try std.testing.expectEqual(@as(usize, 0), serviceAggregate(&service).runtimeCount());
    try std.testing.expect(serviceCore(&service).findTerminal(target.terminal_id) == null);
    try std.testing.expect(!core_module.testing.containsCreation(
        serviceCore(&service),
        target.creation_id,
    ));
}
test "dispose transport behavior is independent of the fair-scan cursor" {
    try disposeSkippedTargetThroughTransport(0);
    try disposeSkippedTargetThroughTransport(64);
}
test "one event lifecycle poll consumes one bounded runtime and cleanup scan" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id: core_module.ConnectionId = 1;
    _ = try serviceCore(&service).connect(
        connection_id,
        testId(core_module.ClientId, 0x34, 0),
    );
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "exit 0" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
    const arguments = if (target_builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const target = try createSkippedNativeTarget(
        &service,
        connection_id,
        executable,
        arguments,
    );
    _ = try findRuntime(&service, target.terminal_id).?.waitForCompletion();
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        serviceCore(&service).findTerminal(target.terminal_id).?.phase(),
    );

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &outgoing, &handoff);

    // One 64-terminal runtime slice leaves index 64 for the next poll. The
    // cleanup selector independently inspects exactly its 16-terminal budget.
    try std.testing.expectEqual(
        core_module.TerminalPhase.running,
        serviceCore(&service).findTerminal(target.terminal_id).?.phase(),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(
        @as(usize, 16),
        aggregate_module.testing.cleanupRetryScanIndexFn(serviceAggregate(&service)),
    );
}
test "settled outcome handoff publishes once and rejects revision advancement" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    const connection_id: core_module.ConnectionId = 1;
    _ = try serviceCore(&service).connect(
        connection_id,
        testId(core_module.ClientId, 0x33, 0),
    );
    const terminal_id = testId(core_module.TerminalId, 0x33, 1);
    _ = try createModel(
        serviceCore(&service),
        connection_id,
        testId(core_module.CreationId, 0x33, 2),
        terminal_id,
        [_]u8{0x33} ** 32,
        80,
        24,
    );
    _ = try serviceCore(&service).synchronizeRevision(terminal_id, 1);
    _ = try serviceCore(&service).childExited(terminal_id, .{ .exit_code = 0 });
    _ = try serviceCore(&service).ptyDrained(terminal_id);

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var handoff: OutcomePublicationHandoff = .{};
    try synchronizeRuntimeLifecycle(&service, &outgoing, &handoff);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expectError(
        error.InvalidPhase,
        serviceCore(&service).synchronizeRevision(terminal_id, 2),
    );
    commitOutcomePublicationHandoff(&handoff, serviceAggregate(&service));
    commitOutcomePublicationHandoff(&handoff, serviceAggregate(&service));
    const terminal = serviceCore(&service).findTerminal(terminal_id).?;
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
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
    _ = try serviceCore(&service).setProjectionDemand(
        active.connection_id,
        attachment_id,
        .active,
    );
    try runtime.setHistoryDemand(attachment_id, .viewing_history);
    setNextProjection(&service);
    try std.testing.expect(serviceAggregate(&service).setNextProjectionAt(terminal_id, 0));

    {
        var projection_first = Outgoing.init(std.testing.allocator);
        defer projection_first.deinit();
        try fillOutgoingLeaving(&projection_first, serviceCore(&service).epoch, 512);
        try scheduleEgress(&service, active, &projection_first);
        try std.testing.expectEqual(@as(usize, 2), projection_first.records().len);
    }
    {
        var interaction_first = Outgoing.init(std.testing.allocator);
        defer interaction_first.deinit();
        try fillOutgoingLeaving(&interaction_first, serviceCore(&service).epoch, 512);
        try scheduleEgress(&service, active, &interaction_first);
        try std.testing.expectEqual(@as(usize, 3), interaction_first.records().len);
        const interaction = try protocol.validateTransportEnvelope(
            interaction_first.records()[2],
        );
        try std.testing.expectEqual(MessageKind.history_changed, interaction.kind);
    }
}
