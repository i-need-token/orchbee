pub const std = @import("std");
pub const target_builtin = @import("builtin");
pub const control_boundary = @import("terminal-host-control");
pub const core_boundary = @import("terminal-host-core");
pub const engine_boundary = @import("terminal-host-engine");
pub const model_boundary = @import("terminal-host-model");
pub const platform_boundary = @import("terminal-host-platform");
pub const protocol = @import("terminal-protocol");
pub const semantic_wait = @import("semantic_wait.zig");
pub const deterministic_schedule = @import("deterministic_schedule.zig");
pub const service_boundary = @import("terminal-host-service");
pub const subject = service_boundary.service;
pub const control_decode = control_boundary.decode;
pub const control_wire = control_boundary.testing.wire;
pub const core_module = core_boundary.state;
pub const pty_process = platform_boundary.process;
pub const terminal_configuration = model_boundary.configuration;
pub const terminal_history = engine_boundary.history;
pub const terminal_runtime = engine_boundary.runtime;
pub const runtimeState = terminal_runtime.testing.stateForTesting;
pub const host_input_admission = service_boundary.testing.input_admission;
pub const host_composition_observation = @import("composition_observation.zig");
pub const host_composition_reference = @import("composition_reference.zig");
pub const host_composition_trace = @import("composition_trace.zig");
pub const host_outgoing = service_boundary.testing.outgoing;
pub const host_response = service_boundary.testing.response;

pub const WireIdentifier = [16]u8;

pub const ConnectionState = subject.ConnectionState;
pub const HostService = subject.HostService;
pub const Outgoing = host_outgoing.Outgoing;
pub const EnvironmentWire = control_wire.Environment;
pub const InputFailure = subject.testing.InputFailureType;
pub const InventoryCursorWire = control_wire.InventoryCursor;
pub const MessageKind = protocol.MessageKind;
pub const OutcomePublicationHandoff = subject.testing.OutcomePublicationHandoffType;
pub const RequestDispatcher = subject.testing.RequestDispatcherType;
pub const TerminalConfigurationWire = control_wire.TerminalConfiguration;
pub const beginShutdown = subject.testing.beginShutdownFn;
pub const commitOutcomePublicationHandoff = subject.testing.commitOutcomePublicationHandoffFn;
pub const createModel = core_module.testing.createTerminal;
pub const identifierText = subject.testing.identifierTextFn;
pub const inputFailure = subject.testing.inputFailureFn;
pub const protocolErrorCode = subject.testing.protocolErrorCodeFn;
pub const scheduleEgress = subject.testing.scheduleEgressFn;
pub const setNextProjection = subject.testing.setNextProjectionFn;
pub const synchronizeRuntimeLifecycle = subject.testing.synchronizeRuntimeLifecycleFn;

pub fn serviceAggregate(service: *HostService) *aggregate_module.HostRuntimeAggregate {
    return &service.runtime_aggregate;
}

pub fn serviceCore(service: *HostService) *core_module.HostCore {
    return serviceAggregate(service).core();
}

pub fn findRuntime(
    service: *HostService,
    terminal_id: core_module.TerminalId,
) ?*terminal_runtime.TerminalRuntime {
    return serviceAggregate(service).runtimeFor(terminal_id);
}

pub fn expectCompositionInvariants(
    service: *HostService,
    connection: ?*const ConnectionState,
) !void {
    try host_composition_observation.checkInvariants(
        std.testing.allocator,
        service,
        connection,
    );
}

pub fn pollEventsChecked(
    service: *HostService,
    connection: *const ConnectionState,
) !Outgoing {
    var outgoing = try service.pollEvents(connection);
    errdefer outgoing.deinit();
    try expectCompositionInvariants(service, connection);
    return outgoing;
}

pub fn activeConnection(connection: *const ConnectionState) !*const ConnectionState.Active {
    return switch (connection.*) {
        .active => |*active| active,
        .awaiting_hello, .closing => error.ExpectedActiveConnection,
    };
}

pub fn expectSingleFailureCode(outgoing: *const Outgoing, expected_code: []const u8) !void {
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

pub fn expectInputAcceptedBytes(
    outgoing: *const Outgoing,
    attachment_id: core_module.AttachmentId,
    expected_bytes: usize,
) !void {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.input_accepted, envelope.kind);
    const InputAccepted = struct {
        acceptedBytes: usize,
        attachmentId: []const u8,
    };
    const accepted = try control_wire.decode(
        InputAccepted,
        std.testing.allocator,
        envelope.payload,
    );
    defer accepted.deinit();
    try std.testing.expectEqual(expected_bytes, accepted.value.acceptedBytes);
    var attachment_buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        identifierText(attachment_id, &attachment_buffer),
        accepted.value.attachmentId,
    );
}

pub fn parseTerminalInput(runtime: *terminal_runtime.TerminalRuntime, bytes: []const u8) void {
    runtimeState(runtime).terminal_mutex.lock();
    defer runtimeState(runtime).terminal_mutex.unlock();
    runtimeState(runtime).stream.nextSlice(bytes);
}

pub fn inputEntryCount(runtime: *terminal_runtime.TerminalRuntime) usize {
    return terminal_runtime.testing.inputQueueSnapshot(runtime).entry_count;
}

pub fn testClientId() core_module.ClientId {
    return core_module.ClientId.fromBytes([_]u8{0x22} ** 16);
}

pub fn testActiveConnectionState() ConnectionState {
    return .{ .active = .{
        .connection_id = 1,
        .limits = .{
            .frame_bytes = 1024,
            .input_bytes = 1024,
            .row_window_bytes = 1024,
        },
    } };
}

pub fn retainFailedTerminal(
    service: *HostService,
    connection_id: core_module.ConnectionId,
    namespace: u8,
) !core_module.TerminalId {
    const terminal_id = testId(core_module.TerminalId, namespace, 0);
    _ = try createModel(
        service.runtime_aggregate.core(),
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

pub fn publishPendingCoreOutcome(
    service: *HostService,
    terminal_id: core_module.TerminalId,
) !void {
    const terminal = service.runtime_aggregate.core().findTerminal(terminal_id) orelse
        return error.ExpectedTerminal;
    const pending = terminal.pendingOutcome() orelse return error.ExpectedPendingOutcome;
    const token = switch (pending) {
        .exited => |outcome| outcome.token,
        .failed => |outcome| outcome.token,
    };
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        service.runtime_aggregate.core().markOutcomePublished(token),
    );
    try std.testing.expectEqual(
        core_module.TransitionOutcome.idempotent,
        service.runtime_aggregate.core().markOutcomePublished(token),
    );
}

pub fn fillOutgoingLeaving(
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

// THCR-027 regression: DisposeTerminal on an already-blocked cleanup phase
// flattened the exact cleanup phase into the generic cleanup-incomplete code.
// Each blocked phase must answer one retryable typed failure with its exact
// cleanup-* code and retain the terminal for the independent maintenance retry.

// THCR-027 boundary: while containment is still stopping there is no retained
// failing cleanup phase, so disposal keeps the generic incomplete-containment
// response and the terminal remains retained.

// THCR-036 regression: with 64 model-only fillers and one native target at
// registry index 64, the bounded 64-entry fair scan skips the target when the
// cursor starts at zero. The engine's published normal completion is therefore
// never linearized into Core before DisposeTerminal. The repair performs an
// exact-terminal synchronization pass before the disposal phase decision, so
// the settled target reaches the retryable outcome-publication-pending barrier
// instead of being rejected as invalid-phase against a stale running lifecycle.

// THCR-036 transport-level confirmation: the exact-terminal synchronization
// pass makes the fully buffered DisposeTerminal outcome independent of the
// fair-scan cursor. Running the same skipped-target setup with the cursor
// outside the target's slice and with the cursor advanced so the bounded pass
// covers the target produces identical transport-level results: the retryable
// publication barrier, the single outcome event, then the disposal
// acknowledgement and removal from both registry indexes.

pub fn disposeSkippedTargetThroughTransport(cursor: usize) !void {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "39393939393939393939393939393939");
    const active = try activeConnection(&connection);
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
    // Advancing the fair cursor makes the bounded pass cover the target before
    // the request; leaving it at zero keeps the target outside that pass.
    if (cursor == 64) service.runtime_aggregate.synchronizeRuntimes();

    const dispose_payload = try testTerminalTargetPayload(target.terminal_id);
    defer std.testing.allocator.free(dispose_payload);
    var rejected = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x7b} ** 16,
    );
    defer rejected.deinit();
    try expectSingleFailureCode(&rejected, "outcome-publication-pending");
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(target.terminal_id) != null);

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

    var disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x7c} ** 16,
    );
    defer disposed.deinit();
    try std.testing.expectEqual(@as(usize, 1), disposed.records().len);
    try std.testing.expectEqual(
        MessageKind.dispose_terminal,
        (try protocol.validateTransportEnvelope(disposed.records()[0])).kind,
    );
    try std.testing.expectEqual(@as(usize, 0), service.runtime_aggregate.runtimeCount());
    try std.testing.expect(service.runtime_aggregate.core().findTerminal(target.terminal_id) == null);
}

// THCR-026 regression: broker loss must revoke attachment authority
// immediately, keep retained terminals, finish old-session cleanup in a
// bounded teardown, and leave a replacement broker fully serviceable.

pub const TerminationBarrierGate = deterministic_schedule.Gate;
pub const TerminationBarrierSignal = deterministic_schedule.Signal;

pub const aggregate_module = service_boundary.testing.runtime_aggregate;

// THCR-006 regression: dispatch pauses after admission observed a pending
// engine, then the wait worker confirms the empty session and reaps the leader.
// The resumed real terminate/forceTerminate must be an idempotent no-op that
// never consults the now-reused numeric session identifier.
pub const PausingTerminationDispatcher = struct {
    entered: *TerminationBarrierSignal,
    gate: *TerminationBarrierGate,
    runtime: ?*terminal_runtime.TerminalRuntime = null,
    mode: ?aggregate_module.TerminationMode = null,

    pub fn dispatch(
        self: *@This(),
        runtime: *terminal_runtime.TerminalRuntime,
        mode: aggregate_module.TerminationMode,
    ) !void {
        self.runtime = runtime;
        self.mode = mode;
        self.entered.notify();
        self.gate.pause();
        switch (mode) {
            .graceful => try runtime.terminate(),
            .force => try runtime.forceTerminate(),
        }
    }
};

pub const NoopTerminationFencer = struct {
    pub fn fence(
        _: *@This(),
        runtime: *terminal_runtime.TerminalRuntime,
    ) terminal_runtime.HostFailureFenceResult {
        return runtime.fenceHostFailure();
    }
};

pub const NoopTerminationContainment = struct {
    pub fn contain(
        _: *@This(),
        _: *core_module.HostCore,
        _: core_module.TerminalId,
        _: *terminal_runtime.TerminalRuntime,
    ) !void {}
};

pub const TerminationRequestOutcome = struct {
    result: ?core_module.TransitionOutcome = null,
    failure: ?anyerror = null,

    pub fn run(
        self: *@This(),
        aggregate: *aggregate_module.HostRuntimeAggregate,
        connection_id: core_module.ConnectionId,
        terminal_id: core_module.TerminalId,
        mode: aggregate_module.TerminationMode,
        dispatcher: anytype,
        fencer: anytype,
        containment: anytype,
    ) void {
        self.result = aggregate_module.testing.requestTerminationUsingFn(
            aggregate,
            connection_id,
            terminal_id,
            mode,
            dispatcher,
            fencer,
            containment,
        ) catch |err| blk: {
            self.failure = err;
            break :blk null;
        };
    }
};

/// Drives one termination-mode variant of the THCR-006 Host barrier: the
/// initial synchronization observes a pending engine, the request is admitted
/// for dispatch, and while dispatch is paused the wait worker completes
/// empty-session confirmation and reaps the session leader. The resumed signal
/// must be an idempotent no-op; the numeric SID is never consulted again.
pub fn driveIdentityReleasedTerminationBarrier(
    service: *HostService,
    connection: *ConnectionState,
    terminal_id: core_module.TerminalId,
    mode: aggregate_module.TerminationMode,
) !void {
    const runtime = findRuntime(service, terminal_id) orelse return error.ExpectedRuntime;
    var entered = TerminationBarrierSignal{};
    var gate = TerminationBarrierGate{};
    var dispatcher: PausingTerminationDispatcher = .{ .entered = &entered, .gate = &gate };
    var fencer: NoopTerminationFencer = .{};
    var containment: NoopTerminationContainment = .{};
    var outcome: TerminationRequestOutcome = .{};
    const active = try activeConnection(connection);

    var request_thread: ?std.Thread = try std.Thread.spawn(.{}, TerminationRequestOutcome.run, .{
        &outcome,
        &service.runtime_aggregate,
        active.connection_id,
        terminal_id,
        mode,
        &dispatcher,
        &fencer,
        &containment,
    });
    defer {
        gate.release();
        if (request_thread) |thread| thread.join();
    }

    // Admission observed the running engine and is pausing before the signal.
    entered.wait();
    try std.testing.expectEqual(mode, dispatcher.mode.?);
    try std.testing.expectEqual(
        terminal_runtime.CompletionState.pending,
        runtime.completion().classification,
    );

    // Complete the wait-worker path: empty-session confirmation and reap.
    // The test drives the ORIGINAL owned session to exit; this is the signal
    // the retained identity is allowed to receive.
    try runtimeState(runtime).process.forceTerminate();
    _ = try runtime.waitForCompletion();
    try std.testing.expect(runtimeState(runtime).process.identityReleased());

    // The numeric SID is free and may be rebound to a foreign session. Resume
    // dispatch: the real terminate/forceTerminate must not consult it.
    const signals_before = runtimeState(runtime).process.signalsIssued();
    gate.release();
    request_thread.?.join();
    request_thread = null;

    try std.testing.expect(outcome.failure == null);
    try std.testing.expectEqual(core_module.TransitionOutcome.applied, outcome.result.?);
    try std.testing.expectEqual(signals_before, runtimeState(runtime).process.signalsIssued());

    // The terminal settles with its eventual normal exit.
    service.runtime_aggregate.synchronizeTerminal(terminal_id);
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.runtime_aggregate.core().findTerminal(terminal_id).?.phase(),
    );
    // A replayed request after settlement is idempotent and never dispatches:
    // the gate is already released, so a (regression) dispatch would return
    // immediately but still fail the idempotent assertion below.
    var replay_outcome: TerminationRequestOutcome = .{};
    var replay_dispatcher: PausingTerminationDispatcher = .{ .entered = &entered, .gate = &gate };
    const replay_thread = try std.Thread.spawn(.{}, TerminationRequestOutcome.run, .{
        &replay_outcome,
        &service.runtime_aggregate,
        active.connection_id,
        terminal_id,
        mode,
        &replay_dispatcher,
        &fencer,
        &containment,
    });
    replay_thread.join();
    try std.testing.expectEqual(core_module.TransitionOutcome.idempotent, replay_outcome.result.?);
    try std.testing.expect(replay_outcome.failure == null);
}

pub fn registeredRequestContract(request_kind: u16) !protocol.RequestMessageContract {
    const registered = protocol.messageContract(request_kind) orelse
        return error.ExpectedRegisteredRequest;
    return registered.request orelse error.ExpectedRegisteredRequest;
}

pub fn testRequestWithContract(
    service: *HostService,
    connection: *ConnectionState,
    request_contract: protocol.RequestMessageContract,
    kind: u16,
    payload: []const u8,
    correlation_id: WireIdentifier,
) !Outgoing {
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        kind,
        request_contract.payload_encoding,
        payload,
        correlation_id,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    const envelope = try protocol.validateTransportEnvelope(record);
    const active = try activeConnection(connection);
    service.runtime_aggregate.synchronizeRuntimes();
    var outgoing = Outgoing.init(std.testing.allocator);
    errdefer outgoing.deinit();
    var dispatcher = RequestDispatcher.init(
        service.allocator,
        &service.runtime_aggregate,
        service.trace_interactions,
    );
    try dispatcher.dispatch(
        request_contract,
        active.connection_id,
        .{
            .input_bytes = active.limits.input_bytes,
            .row_window_bytes = active.limits.row_window_bytes,
        },
        envelope,
        correlation_id,
        &outgoing,
    );
    try expectCompositionInvariants(service, connection);
    return outgoing;
}

pub fn expectContractSuccess(
    outgoing: *const Outgoing,
    request_contract: protocol.RequestMessageContract,
    correlation_id: WireIdentifier,
) !protocol.EnvelopeSummary {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    const success = try host_response.resolveSuccessResponse(request_contract);
    try std.testing.expect(envelope.response);
    try std.testing.expect(!envelope.event);
    try std.testing.expectEqualSlices(u8, &correlation_id, &envelope.correlation_id.?);
    try std.testing.expectEqual(success.kind, envelope.kind);
    try std.testing.expectEqual(
        @intFromEnum(success.encoding),
        envelope.payload_encoding,
    );
    return envelope;
}

pub fn publishedClientInputCount(runtime: *terminal_runtime.TerminalRuntime) usize {
    return terminal_runtime.testing.inputQueueSnapshot(runtime).published_client_entries;
}

pub fn onlyTerminalFrame(outgoing: *const Outgoing) !protocol.FrameSummary {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expect(
        envelope.kind == MessageKind.snapshot_frame or
            envelope.kind == MessageKind.delta_frame,
    );
    return protocol.validateTerminalFrame(std.testing.allocator, envelope.payload);
}

pub fn testHandshake(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    return testHandshakeWithInputBytes(service, connection, client_id, 2_048);
}

pub fn testHandshakeWithInputBytes(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
    input_bytes: usize,
) !void {
    const hello = try host_response.resolveSuccessResponse(
        try registeredRequestContract(MessageKind.hello),
    );
    try std.testing.expectEqual(
        hello.kind,
        try testHandshakeResponseKindWithInputBytes(
            service,
            connection,
            client_id,
            input_bytes,
        ),
    );
}

pub fn testHandshakeResponseKind(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !u16 {
    return testHandshakeResponseKindWithInputBytes(service, connection, client_id, 2_048);
}

pub fn testHandshakeResponseKindWithInputBytes(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
    input_bytes: usize,
) !u16 {
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"{s}\",\"frameBytes\":1048576,\"inputBytes\":{d},\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{
            client_id,
            input_bytes,
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
    try expectCompositionInvariants(service, connection);
    return (try protocol.validateTransportEnvelope(outgoing.records()[0])).kind;
}

pub fn testCreatePayload(
    creation_id: []const u8,
    executable: []const u8,
    arguments: []const []const u8,
) ![]u8 {
    return testCreatePayloadWithConfiguration(creation_id, executable, arguments, null);
}

pub fn testCreatePayloadWithConfiguration(
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

pub fn testAttachPayload(
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

pub fn testTerminalThemePayload(
    operation_id: model_boundary.identifiers.TerminalThemeOperationId,
    terminal_id: core_module.TerminalId,
    expected_revision: u64,
    theme: model_boundary.theme.Theme,
) ![]u8 {
    var operation_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    var revision_buffer: [20]u8 = undefined;
    const revision = try std.fmt.bufPrint(&revision_buffer, "{d}", .{expected_revision});
    var writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer writer.deinit();
    try std.json.Stringify.value(.{
        .expectedThemeRevision = revision,
        .operationId = identifierText(operation_id, &operation_buffer),
        .terminal = .{
            .colorScheme = @tagName(theme.color_scheme),
            .defaultBackgroundRgb = theme.default_background_rgb,
            .defaultCursorRgb = theme.default_cursor_rgb,
            .defaultForegroundRgb = theme.default_foreground_rgb,
            .palette = &theme.palette,
        },
        .terminalId = identifierText(terminal_id, &terminal_buffer),
    }, .{}, &writer.writer);
    return writer.toOwnedSlice();
}

pub fn testRawInputPayload(
    attachment_id: core_module.AttachmentId,
    input: []const u8,
) ![]u8 {
    const payload = try std.testing.allocator.alloc(u8, attachment_id.bytes.len + input.len);
    @memcpy(payload[0..attachment_id.bytes.len], &attachment_id.bytes);
    @memcpy(payload[attachment_id.bytes.len..], input);
    return payload;
}

pub const SkippedNativeTarget = struct {
    creation_id: core_module.CreationId,
    terminal_id: core_module.TerminalId,
};

/// THCR-036 setup: 64 model-only filler sessions followed by one native
/// target. Registry keys are insertion-ordered, so the target sits at index 64
/// and stays outside the bounded 64-entry fair background slice while the
/// cursor starts at zero.
pub fn createSkippedNativeTarget(
    service: *HostService,
    connection_id: core_module.ConnectionId,
    executable: []const u8,
    arguments: []const []const u8,
) !SkippedNativeTarget {
    for (0..64) |offset| {
        _ = try createModel(
            service.runtime_aggregate.core(),
            connection_id,
            testId(core_module.CreationId, 0x71, offset),
            testId(core_module.TerminalId, 0x72, offset),
            [_]u8{0x73} ** 32,
            80,
            24,
        );
    }
    const creation_id = testId(core_module.CreationId, 0x74, 0);
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    const validated = try engine_boundary.launch.inspect(std.testing.allocator, .{
        .arguments = arguments,
        .configuration = null,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .geometry = .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 80,
            .rows = 24,
        },
        .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &features,
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee test",
        },
    });
    var preparation = try service.runtime_aggregate.prepareCreateTerminal(
        connection_id,
        creation_id,
        validated,
    );
    defer preparation.deinit();
    const terminal_id = switch (try service.runtime_aggregate.commitPreparedTerminal(&preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    return .{ .creation_id = creation_id, .terminal_id = terminal_id };
}

pub fn testTerminalTargetPayload(terminal_id: core_module.TerminalId) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
}

pub fn testIdentifier(namespace: u8, index: usize) WireIdentifier {
    var result = [_]u8{0} ** 16;
    result[0] = namespace;
    std.mem.writeInt(u64, result[8..16], @intCast(index + 1), .little);
    return result;
}

pub fn testId(comptime Id: type, namespace: u8, index: usize) Id {
    return Id.fromBytes(testIdentifier(namespace, index));
}

pub fn testRequest(
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
    var outgoing = try service.handle(connection, record);
    errdefer outgoing.deinit();
    try expectCompositionInvariants(service, connection);
    return outgoing;
}
