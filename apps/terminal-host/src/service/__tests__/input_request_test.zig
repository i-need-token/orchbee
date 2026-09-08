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
const registeredRequestContract = harness.registeredRequestContract;
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
const aggregate_harness = @import("runtime_aggregate_harness.zig");

fn expectAttachmentInputCapability(
    outgoing: *const Outgoing,
    attachment_id: core_module.AttachmentId,
    expected_revision: []const u8,
    expected_capabilities: core_module.Capabilities,
) !void {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.attachment_capabilities, envelope.kind);
    const AttachmentCapabilities = struct {
        attachmentId: []const u8,
        capabilities: struct {
            input: bool,
            read: bool,
            resize: bool,
        },
        capabilityRevision: []const u8,
        resizeLease: bool,
        terminalId: []const u8,
    };
    const decoded = try control_wire.decode(
        AttachmentCapabilities,
        std.testing.allocator,
        envelope.payload,
    );
    defer decoded.deinit();
    var attachment_buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        identifierText(attachment_id, &attachment_buffer),
        decoded.value.attachmentId,
    );
    try std.testing.expectEqualStrings(expected_revision, decoded.value.capabilityRevision);
    try std.testing.expectEqual(expected_capabilities.input, decoded.value.capabilities.input);
    try std.testing.expectEqual(expected_capabilities.read, decoded.value.capabilities.read);
    try std.testing.expectEqual(expected_capabilities.resize, decoded.value.capabilities.resize);
    try std.testing.expect(!decoded.value.resizeLease);
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
    try std.testing.expectEqualDeep(
        InputFailure{ .code = "input-mode-changed", .retryable = false },
        inputFailure(.input_mode_changed),
    );
}
test "protocol identifier failures retain their public error code" {
    try std.testing.expectEqualStrings(
        "invalid-identifier",
        protocolErrorCode(error.InvalidIdentifier),
    );
}
test "projection demand gates and repairs shared attachment focus claims" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "74747474747474747474747474747474");
    const active = try activeConnection(&connection);
    const aggregate = serviceAggregate(&service);
    var launch_fixture = try aggregate_harness.validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var create = try aggregate.prepareCreateTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x75, 0),
        validated,
    );
    defer create.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&create)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const first_attachment = core_module.AttachmentId.fromBytes([_]u8{0x76} ** 16);
    const second_attachment = core_module.AttachmentId.fromBytes([_]u8{0x77} ** 16);
    for ([_]core_module.AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        _ = try aggregate.attach(
            active.connection_id,
            attachment_id,
            terminal_id,
            .{ .input = true, .read = true, .resize = false },
            false,
        );
    }
    const runtime = findRuntime(&service, terminal_id).?;
    parseTerminalInput(runtime, "\x1b[?1004h");
    var publication_barrier = try runtime.prepareRawInput("focus publication barrier");
    defer publication_barrier.deinit();

    switch (aggregate.acceptSemanticInput(
        active.connection_id,
        first_attachment,
        1,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    )) {
        .core_rejected => |err| try std.testing.expectEqual(error.InvalidPhase, err),
        .accepted, .runtime_missing, .runtime_rejected => return error.ExpectedPausedFocusRejection,
    }

    for ([_]core_module.AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        try std.testing.expectEqual(
            core_module.TransitionOutcome.applied,
            try aggregate.setProjectionDemand(
                active.connection_id,
                attachment_id,
                .active,
            ),
        );
    }
    for ([_]struct { core_module.AttachmentId, usize }{
        .{ first_attachment, 3 },
        .{ second_attachment, 0 },
    }) |entry| {
        switch (aggregate.acceptSemanticInput(
            active.connection_id,
            entry[0],
            1,
            .{ .focus = .{ .focused = true } },
            std.math.maxInt(usize),
        )) {
            .accepted => |accepted| try std.testing.expectEqual(entry[1], accepted.byte_count),
            .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedFocusAcceptance,
        }
    }
    try std.testing.expect(terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        first_attachment,
    ));
    try std.testing.expect(terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        second_attachment,
    ));

    const published_before_pause = publishedClientInputCount(runtime);
    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try aggregate.setProjectionDemand(
            active.connection_id,
            first_attachment,
            .paused,
        ),
    );
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        first_attachment,
    ));
    try std.testing.expect(terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        second_attachment,
    ));
    try std.testing.expectEqual(published_before_pause, publishedClientInputCount(runtime));

    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try aggregate.setProjectionDemand(
            active.connection_id,
            second_attachment,
            .paused,
        ),
    );
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        second_attachment,
    ));
    try std.testing.expectEqual(
        published_before_pause + 1,
        publishedClientInputCount(runtime),
    );

    for ([_]core_module.AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        _ = try aggregate.setProjectionDemand(
            active.connection_id,
            attachment_id,
            .active,
        );
    }
    try std.testing.expectEqual(
        published_before_pause + 1,
        publishedClientInputCount(runtime),
    );
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        first_attachment,
    ));
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        second_attachment,
    ));

    var stale_focus = aggregate.prepareSemanticInput(
        active.connection_id,
        first_attachment,
        1,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    );
    defer stale_focus.deinit();
    try std.testing.expect(stale_focus.accepted() != null);
    _ = try aggregate.setProjectionDemand(
        active.connection_id,
        first_attachment,
        .paused,
    );
    _ = try aggregate.setProjectionDemand(
        active.connection_id,
        first_attachment,
        .active,
    );
    switch (stale_focus.commit()) {
        .runtime_rejected => |rejection| try std.testing.expectEqual(
            host_input_admission.Rejection.terminal_exited,
            rejection,
        ),
        .accepted, .core_rejected, .runtime_missing => return error.ExpectedStaleFocusRejection,
    }
    switch (aggregate.acceptSemanticInput(
        active.connection_id,
        first_attachment,
        1,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    )) {
        .accepted => |accepted| try std.testing.expectEqual(@as(usize, 3), accepted.byte_count),
        .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedFocusAcceptance,
    }
    _ = try aggregate.setProjectionDemand(
        active.connection_id,
        first_attachment,
        .paused,
    );
}
test "capability revision updates repair one attachment and reject stale input" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "67676767676767676767676767676767");
    const active = try activeConnection(&connection);
    const aggregate = serviceAggregate(&service);
    var launch_fixture = try aggregate_harness.validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var create = try aggregate.prepareCreateTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x68, 0),
        validated,
    );
    defer create.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&create)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x69} ** 16);
    const sibling_attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x6a} ** 16);
    _ = try aggregate.attach(
        active.connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = false },
        false,
    );
    _ = try aggregate.attach(
        active.connection_id,
        sibling_attachment_id,
        terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );
    _ = try aggregate.setProjectionDemand(
        active.connection_id,
        attachment_id,
        .active,
    );
    const runtime = findRuntime(&service, terminal_id).?;
    parseTerminalInput(runtime, "\x1b[=3;1u\x1b[?1004h");

    // Keep every accepted record queued so downgrade and upgrade can prove
    // they neither discard nor replay already admitted client input.
    var publication_barrier = try runtime.prepareRawInput("publication barrier");
    defer publication_barrier.deinit();
    for ([_]struct { core_module.AttachmentId, []const u8, []const u8, u21 }{
        .{ attachment_id, "KeyA", "a", 'a' },
        .{ sibling_attachment_id, "KeyB", "b", 'b' },
    }) |entry| {
        switch (aggregate.acceptSemanticInput(
            active.connection_id,
            entry[0],
            1,
            .{ .key = .{
                .action = .press,
                .composing = false,
                .consumed_modifiers = .{},
                .key = entry[1],
                .modifiers = .{},
                .text = entry[2],
                .unshifted_codepoint = entry[3],
            } },
            std.math.maxInt(usize),
        )) {
            .accepted => |accepted| try std.testing.expectEqual(@as(u64, 1), accepted.capability_revision),
            .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedInputAcceptance,
        }
    }
    try std.testing.expect(terminal_runtime.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(terminal_runtime.testing.hasAttachmentInputOwnership(
        runtime,
        sibling_attachment_id,
    ));
    switch (aggregate.acceptSemanticInput(
        active.connection_id,
        attachment_id,
        1,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    )) {
        .accepted => |accepted| try std.testing.expectEqual(@as(usize, 3), accepted.byte_count),
        .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedFocusAcceptance,
    }
    try std.testing.expect(terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        attachment_id,
    ));

    var raw_payload = [_]u8{0} ** 25;
    @memcpy(raw_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, raw_payload[16..24], 1, .little);
    raw_payload[24] = 'q';
    var queued = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &raw_payload,
        [_]u8{0x6b} ** 16,
    );
    defer queued.deinit();
    try expectInputAcceptedBytes(&queued, attachment_id, 1, "1");
    const published_before_downgrade = publishedClientInputCount(runtime);

    var stale_preparation = aggregate.prepareRawInput(
        active.connection_id,
        attachment_id,
        1,
        "prepared before downgrade",
    );
    defer stale_preparation.deinit();
    try std.testing.expect(stale_preparation.accepted() != null);

    var attachment_buffer: [32]u8 = undefined;
    const attachment_text = identifierText(attachment_id, &attachment_buffer);
    var update_buffer: [160]u8 = undefined;
    const downgrade_payload = try std.fmt.bufPrint(
        &update_buffer,
        "{{\"attachmentId\":\"{s}\",\"capabilityRevision\":\"2\",\"input\":false}}",
        .{attachment_text},
    );
    var downgraded = try testRequest(
        &service,
        &connection,
        MessageKind.update_attachment_input_capability,
        .control_json,
        downgrade_payload,
        [_]u8{0x6c} ** 16,
    );
    defer downgraded.deinit();
    try expectAttachmentInputCapability(
        &downgraded,
        attachment_id,
        "2",
        .{ .input = false, .read = true, .resize = false },
    );
    try std.testing.expectEqual(
        published_before_downgrade + 1,
        publishedClientInputCount(runtime),
    );
    try std.testing.expect(!terminal_runtime.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(terminal_runtime.testing.hasAttachmentInputOwnership(
        runtime,
        sibling_attachment_id,
    ));
    switch (stale_preparation.commit()) {
        .core_rejected => |err| try std.testing.expectEqual(error.StaleCapabilityRevision, err),
        .accepted, .runtime_missing, .runtime_rejected => return error.ExpectedStaleCapabilityRevision,
    }

    var stale_input = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &raw_payload,
        [_]u8{0x6d} ** 16,
    );
    defer stale_input.deinit();
    try expectSingleFailureCode(&stale_input, "stale-capability-revision");
    std.mem.writeInt(u64, raw_payload[16..24], 2, .little);
    var unavailable_input = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &raw_payload,
        [_]u8{0x6e} ** 16,
    );
    defer unavailable_input.deinit();
    try expectSingleFailureCode(&unavailable_input, "capability-unavailable");

    var replayed = try testRequest(
        &service,
        &connection,
        MessageKind.update_attachment_input_capability,
        .control_json,
        downgrade_payload,
        [_]u8{0x6f} ** 16,
    );
    defer replayed.deinit();
    try expectAttachmentInputCapability(
        &replayed,
        attachment_id,
        "2",
        .{ .input = false, .read = true, .resize = false },
    );
    const conflict_payload = try std.fmt.bufPrint(
        &update_buffer,
        "{{\"attachmentId\":\"{s}\",\"capabilityRevision\":\"2\",\"input\":true}}",
        .{attachment_text},
    );
    var conflicted = try testRequest(
        &service,
        &connection,
        MessageKind.update_attachment_input_capability,
        .control_json,
        conflict_payload,
        [_]u8{0x70} ** 16,
    );
    defer conflicted.deinit();
    try expectSingleFailureCode(&conflicted, "capability-revision-conflict");
    const gap_payload = try std.fmt.bufPrint(
        &update_buffer,
        "{{\"attachmentId\":\"{s}\",\"capabilityRevision\":\"4\",\"input\":true}}",
        .{attachment_text},
    );
    var gap = try testRequest(
        &service,
        &connection,
        MessageKind.update_attachment_input_capability,
        .control_json,
        gap_payload,
        [_]u8{0x71} ** 16,
    );
    defer gap.deinit();
    try expectSingleFailureCode(&gap, "stale-capability-revision");

    const before_upgrade = terminal_runtime.testing.inputQueueSnapshot(runtime);
    const upgrade_payload = try std.fmt.bufPrint(
        &update_buffer,
        "{{\"attachmentId\":\"{s}\",\"capabilityRevision\":\"3\",\"input\":true}}",
        .{attachment_text},
    );
    var upgraded = try testRequest(
        &service,
        &connection,
        MessageKind.update_attachment_input_capability,
        .control_json,
        upgrade_payload,
        [_]u8{0x72} ** 16,
    );
    defer upgraded.deinit();
    try expectAttachmentInputCapability(
        &upgraded,
        attachment_id,
        "3",
        .{ .input = true, .read = true, .resize = false },
    );
    const after_upgrade = terminal_runtime.testing.inputQueueSnapshot(runtime);
    try std.testing.expectEqual(before_upgrade.client_charge, after_upgrade.client_charge);
    try std.testing.expectEqual(before_upgrade.entry_count, after_upgrade.entry_count);
    try std.testing.expectEqual(
        before_upgrade.published_client_entries,
        after_upgrade.published_client_entries,
    );
    try std.testing.expectEqual(before_upgrade.repair_charge, after_upgrade.repair_charge);
    try std.testing.expectEqual(
        before_upgrade.input_attachment_count + 1,
        after_upgrade.input_attachment_count,
    );
    try std.testing.expect(terminal_runtime.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(!terminal_runtime.testing.attachmentFocusClaim(
        runtime,
        attachment_id,
    ));

    std.mem.writeInt(u64, raw_payload[16..24], 3, .little);
    var current_input = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &raw_payload,
        [_]u8{0x73} ** 16,
    );
    defer current_input.deinit();
    try expectInputAcceptedBytes(&current_input, attachment_id, 1, "3");
}
test "input-only capability downgrade and replay return an all-false current binding" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "74747474747474747474747474747474");
    const active = try activeConnection(&connection);
    const aggregate = serviceAggregate(&service);
    var launch_fixture = try aggregate_harness.validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var create = try aggregate.prepareCreateTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x75, 0),
        validated,
    );
    defer create.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&create)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x76} ** 16);
    _ = try aggregate.attach(
        active.connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );

    var attachment_buffer: [32]u8 = undefined;
    var update_buffer: [160]u8 = undefined;
    const downgrade_payload = try std.fmt.bufPrint(
        &update_buffer,
        "{{\"attachmentId\":\"{s}\",\"capabilityRevision\":\"2\",\"input\":false}}",
        .{identifierText(attachment_id, &attachment_buffer)},
    );
    for ([_]u8{ 0x77, 0x78 }) |correlation_byte| {
        var response = try testRequest(
            &service,
            &connection,
            MessageKind.update_attachment_input_capability,
            .control_json,
            downgrade_payload,
            [_]u8{correlation_byte} ** 16,
        );
        defer response.deinit();
        try expectAttachmentInputCapability(
            &response,
            attachment_id,
            "2",
            .{ .input = false, .read = false, .resize = false },
        );
    }

    const attachment = serviceCore(&service).findAttachment(attachment_id).?;
    try std.testing.expectEqual(@as(u64, 2), attachment.capability_revision);
    try std.testing.expectEqual(
        core_module.Capabilities{ .input = false, .read = false, .resize = false },
        attachment.capabilities,
    );
}
fn awaitFrameBatch(service: *HostService, connection: *const ConnectionState) !Outgoing {
    const timer = try platform_boundary.monotonic.Timer.start();
    while (timer.read() < 10 * std.time.ns_per_s) {
        var batch = try pollEventsChecked(service, connection);
        if (batch.records().len != 0) return batch;
        batch.deinit();
        // Native output can change generation between capture and commit.
        // That rejected capture publishes nothing; allow the next scheduled
        // attempt while retaining the caller's exact frame and sequence checks.
        platform_boundary.monotonic.sleep(10 * std.time.ns_per_ms);
    }
    return error.SemanticWaitTimeout;
}

test "I/O successful responses use supplied request contracts and publish input once" {
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
        "49494949494949494949494949494949",
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
    try testHandshake(&service, &connection, "48484848484848484848484848484848");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x49} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x4a} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
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
        [_]u8{0x4a} ** 16,
    );
    defer attached.deinit();
    const runtime = findRuntime(&service, terminal_id).?;
    const active = try activeConnection(&connection);
    _ = try serviceAggregate(&service).setProjectionDemand(
        active.connection_id,
        attachment_id,
        .active,
    );

    // This unpublished first FIFO entry prevents the writer from consuming
    // later committed input while the test counts exact publications.
    var publication_barrier = try runtime.prepareRawInput("publication barrier");
    defer publication_barrier.deinit();

    var raw_payload = [_]u8{0} ** 25;
    @memcpy(raw_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, raw_payload[16..24], 1, .little);
    raw_payload[24] = 'r';

    var key_payload = [_]u8{0} ** 45;
    @memcpy(key_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, key_payload[16..24], 1, .little);
    key_payload[24] = 1;
    std.mem.writeInt(u16, key_payload[30..32], 4, .little);
    std.mem.writeInt(u32, key_payload[32..36], 'a', .little);
    std.mem.writeInt(u32, key_payload[36..40], 1, .little);
    @memcpy(key_payload[40..44], "KeyA");
    key_payload[44] = 'a';

    var text_payload = [_]u8{0} ** 29;
    @memcpy(text_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, text_payload[16..24], 1, .little);
    std.mem.writeInt(u32, text_payload[24..28], 1, .little);
    text_payload[28] = 't';

    var mouse_payload = [_]u8{0} ** 40;
    @memcpy(mouse_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, mouse_payload[16..24], 1, .little);
    mouse_payload[24] = 0;
    mouse_payload[25] = 1;
    std.mem.writeInt(u32, mouse_payload[28..32], @bitCast(@as(f32, 8)), .little);
    std.mem.writeInt(u32, mouse_payload[32..36], @bitCast(@as(f32, 16)), .little);
    std.mem.writeInt(u16, mouse_payload[36..38], 1, .little);

    var focus_payload = [_]u8{0} ** 28;
    @memcpy(focus_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, focus_payload[16..24], 1, .little);
    focus_payload[24] = 1;

    var paste_payload = [_]u8{0} ** 33;
    @memcpy(paste_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, paste_payload[16..24], 1, .little);
    std.mem.writeInt(u32, paste_payload[28..32], 1, .little);
    paste_payload[32] = 'p';

    const input_cases = [_]struct {
        kind: u16,
        must_publish: bool,
        payload: []const u8,
    }{
        .{ .kind = MessageKind.write_input, .must_publish = true, .payload = &raw_payload },
        .{ .kind = MessageKind.key_input, .must_publish = true, .payload = &key_payload },
        .{ .kind = MessageKind.text_input, .must_publish = true, .payload = &text_payload },
        .{ .kind = MessageKind.mouse_input, .must_publish = false, .payload = &mouse_payload },
        .{ .kind = MessageKind.focus_input, .must_publish = false, .payload = &focus_payload },
        .{ .kind = MessageKind.paste_input, .must_publish = true, .payload = &paste_payload },
    };
    var input_acceptances: usize = 0;

    var incompatible_contract = try registeredRequestContract(MessageKind.write_input);
    incompatible_contract.success_kind = MessageKind.rows_snapshot;
    const publications_before_incompatible = publishedClientInputCount(runtime);
    try std.testing.expectError(
        error.IncompatibleSuccessResponseEncoding,
        testRequestWithContract(
            &service,
            &connection,
            incompatible_contract,
            MessageKind.write_input,
            &raw_payload,
            [_]u8{0x50} ** 16,
        ),
    );
    try std.testing.expectEqual(
        publications_before_incompatible,
        publishedClientInputCount(runtime),
    );

    for (input_cases, 0..) |input_case, index| {
        const request_contract = try registeredRequestContract(input_case.kind);
        const publications_before = publishedClientInputCount(runtime);
        const correlation_id = testIdentifier(0x4b, index);
        var outgoing = try testRequestWithContract(
            &service,
            &connection,
            request_contract,
            input_case.kind,
            input_case.payload,
            correlation_id,
        );
        defer outgoing.deinit();
        const envelope = try expectContractSuccess(
            &outgoing,
            request_contract,
            correlation_id,
        );
        const InputAccepted = struct {
            acceptedBytes: usize,
            attachmentId: []const u8,
            capabilityRevision: []const u8,
        };
        const accepted = try control_wire.decode(
            InputAccepted,
            std.testing.allocator,
            envelope.payload,
        );
        defer accepted.deinit();
        input_acceptances += 1;
        var expected_attachment_buffer: [32]u8 = undefined;
        try std.testing.expectEqualStrings(
            identifierText(attachment_id, &expected_attachment_buffer),
            accepted.value.attachmentId,
        );
        try std.testing.expectEqualStrings("1", accepted.value.capabilityRevision);
        if (input_case.kind == MessageKind.write_input) {
            try std.testing.expectEqual(
                @as(usize, input_case.payload.len - 24),
                accepted.value.acceptedBytes,
            );
        }
        const publications_after = publishedClientInputCount(runtime);
        if (accepted.value.acceptedBytes == 0) {
            try std.testing.expect(!input_case.must_publish);
            try std.testing.expectEqual(publications_before, publications_after);
        } else {
            try std.testing.expectEqual(publications_before + 1, publications_after);
        }
    }
    try std.testing.expectEqual(input_cases.len, input_acceptances);

    const resize_contract = try registeredRequestContract(MessageKind.resize_terminal);
    var resized = try testRequestWithContract(
        &service,
        &connection,
        resize_contract,
        MessageKind.resize_terminal,
        "{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"cellPixelHeight\":16,\"cellPixelWidth\":8,\"columns\":81,\"rows\":24}",
        [_]u8{0x51} ** 16,
    );
    defer resized.deinit();
    _ = try expectContractSuccess(&resized, resize_contract, [_]u8{0x51} ** 16);

    const demand_contract = try registeredRequestContract(MessageKind.set_projection_demand);
    var demanded = try testRequestWithContract(
        &service,
        &connection,
        demand_contract,
        MessageKind.set_projection_demand,
        "{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"demand\":\"active\"}",
        [_]u8{0x52} ** 16,
    );
    defer demanded.deinit();
    _ = try expectContractSuccess(&demanded, demand_contract, [_]u8{0x52} ** 16);

    try std.testing.expect(serviceAggregate(&service).setNextProjectionAt(terminal_id, 0));
    var first_frame_batch = try awaitFrameBatch(&service, &connection);
    defer first_frame_batch.deinit();
    const first_frame = try onlyTerminalFrame(&first_frame_batch);
    var need_snapshot_buffer: [160]u8 = undefined;
    const need_snapshot_payload = try std.fmt.bufPrint(
        &need_snapshot_buffer,
        "{{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"reason\":\"activity-stale\",\"sequence\":\"{d}\"}}",
        .{first_frame.sequence},
    );
    const need_snapshot_contract = try registeredRequestContract(MessageKind.need_snapshot);
    var snapshot_requested = try testRequestWithContract(
        &service,
        &connection,
        need_snapshot_contract,
        MessageKind.need_snapshot,
        need_snapshot_payload,
        [_]u8{0x53} ** 16,
    );
    defer snapshot_requested.deinit();
    _ = try expectContractSuccess(
        &snapshot_requested,
        need_snapshot_contract,
        [_]u8{0x53} ** 16,
    );

    try std.testing.expect(serviceAggregate(&service).setNextProjectionAt(terminal_id, 0));
    var second_frame_batch = try awaitFrameBatch(&service, &connection);
    defer second_frame_batch.deinit();
    const second_frame = try onlyTerminalFrame(&second_frame_batch);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, second_frame.kind);
    try std.testing.expectEqual(first_frame.sequence + 1, second_frame.sequence);
    var frame_applied_buffer: [256]u8 = undefined;
    const frame_applied_payload = try std.fmt.bufPrint(
        &frame_applied_buffer,
        "{{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{ second_frame.generation, second_frame.revision, second_frame.sequence },
    );
    const frame_applied_contract = try registeredRequestContract(MessageKind.frame_applied);
    var frame_applied = try testRequestWithContract(
        &service,
        &connection,
        frame_applied_contract,
        MessageKind.frame_applied,
        frame_applied_payload,
        [_]u8{0x54} ** 16,
    );
    defer frame_applied.deinit();
    _ = try expectContractSuccess(
        &frame_applied,
        frame_applied_contract,
        [_]u8{0x54} ** 16,
    );
}
test "semantic input service preserves negotiated limits and total mouse encoding" {
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
        "55555555555555555555555555555555",
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
    const client_id = "56565656565656565656565656565656";
    try testHandshakeWithInputBytes(&service, &connection, client_id, 12_293);
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x55} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const first_attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x57} ** 16);
    var first_attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const first_attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(first_attachment_id, &first_attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
    defer std.testing.allocator.free(first_attach_payload);
    var first_attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        first_attach_payload,
        [_]u8{0x57} ** 16,
    );
    defer first_attached.deinit();
    const runtime = findRuntime(&service, terminal_id).?;
    parseTerminalInput(runtime, "\x1b[>24u");

    const key_payload = try std.testing.allocator.alloc(u8, 40 + 4 + 4_096);
    defer std.testing.allocator.free(key_payload);
    @memset(key_payload, 0);
    @memcpy(key_payload[0..16], &first_attachment_id.bytes);
    std.mem.writeInt(u64, key_payload[16..24], 1, .little);
    key_payload[24] = 1;
    std.mem.writeInt(u16, key_payload[30..32], 4, .little);
    std.mem.writeInt(u32, key_payload[32..36], 'a', .little);
    std.mem.writeInt(u32, key_payload[36..40], 4_096, .little);
    @memcpy(key_payload[40..44], "KeyA");
    @memset(key_payload[44..], 'a');

    var rejected = try testRequest(
        &service,
        &connection,
        MessageKind.key_input,
        .raw_bytes,
        key_payload,
        [_]u8{0x58} ** 16,
    );
    defer rejected.deinit();
    try expectSingleFailureCode(&rejected, "limit-exceeded");
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), inputEntryCount(runtime));

    service.disconnect(&connection);
    try testHandshakeWithInputBytes(&service, &connection, client_id, 12_294);
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x59} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
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
        [_]u8{0x59} ** 16,
    );
    defer attached.deinit();
    @memcpy(key_payload[0..16], &attachment_id.bytes);

    // An unpublished first entry keeps the writer from consuming committed
    // entries while this test checks exact service-level queue charges.
    var publication_barrier = try runtime.prepareRawInput("b");
    defer publication_barrier.deinit();
    try std.testing.expectEqual(@as(usize, 1), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 1), inputEntryCount(runtime));

    var accepted_key = try testRequest(
        &service,
        &connection,
        MessageKind.key_input,
        .raw_bytes,
        key_payload,
        [_]u8{0x5a} ** 16,
    );
    defer accepted_key.deinit();
    try expectInputAcceptedBytes(&accepted_key, attachment_id, 12_294, "1");
    try std.testing.expectEqual(@as(usize, 12_295), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 2), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");
    var mouse_payload = [_]u8{0} ** 40;
    @memcpy(mouse_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u64, mouse_payload[16..24], 1, .little);
    mouse_payload[24] = 0;
    mouse_payload[25] = 10;
    std.mem.writeInt(u16, mouse_payload[36..38], 1 << 9, .little);
    var accepted_ten = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5b} ** 16,
    );
    defer accepted_ten.deinit();
    try expectInputAcceptedBytes(&accepted_ten, attachment_id, 11, "1");
    try std.testing.expectEqual(@as(usize, 12_306), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 3), inputEntryCount(runtime));

    mouse_payload[25] = 11;
    std.mem.writeInt(u16, mouse_payload[36..38], 1 << 10, .little);
    var accepted_eleven = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5c} ** 16,
    );
    defer accepted_eleven.deinit();
    try expectInputAcceptedBytes(&accepted_eleven, attachment_id, 11, "1");
    try std.testing.expectEqual(@as(usize, 12_317), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 4), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1006l");
    mouse_payload[24] = 1;
    mouse_payload[25] = 1;
    std.mem.writeInt(u32, mouse_payload[28..32], 0x7f7fffff, .little);
    std.mem.writeInt(u16, mouse_payload[36..38], 0, .little);
    var accepted_boundary = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5d} ** 16,
    );
    defer accepted_boundary.deinit();
    try expectInputAcceptedBytes(&accepted_boundary, attachment_id, 6, "1");
    try std.testing.expectEqual(@as(usize, 12_323), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 5), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1000l");
    mouse_payload[24] = 0;
    mouse_payload[25] = 10;
    std.mem.writeInt(u32, mouse_payload[28..32], 0, .little);
    std.mem.writeInt(u16, mouse_payload[36..38], 1 << 9, .little);
    var accepted_noop = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5e} ** 16,
    );
    defer accepted_noop.deinit();
    try expectInputAcceptedBytes(&accepted_noop, attachment_id, 0, "1");
    try std.testing.expectEqual(@as(usize, 12_323), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 5), inputEntryCount(runtime));
}
test "semantic input preparation commits to a terminal-exited rejection after client admission closes" {
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
        "4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d",
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
    try testHandshake(&service, &connection, "4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x4d} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x4e} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
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
        [_]u8{0x4e} ** 16,
    );
    defer attached.deinit();
    const active = try activeConnection(&connection);
    const runtime = findRuntime(&service, terminal_id).?;

    // A semantic input prepared while the root runs reserves one client FIFO
    // position whose bytes are not yet published.
    var preparation = serviceAggregate(&service).prepareSemanticInput(
        active.connection_id,
        attachment_id,
        1,
        .{ .text = .{ .text = "post-exit" } },
        std.math.maxInt(usize),
    );
    defer preparation.deinit();
    try std.testing.expect(preparation.accepted() != null);
    try std.testing.expectEqual(@as(usize, 9), runtime.pendingInputBytes());

    // Closing runtime client admission after the exit must turn the commit into
    // a non-retryable invalid-phase rejection instead of an acceptance.
    terminal_runtime.testing.closeClientInputFn(runtime);
    switch (preparation.commit()) {
        .runtime_rejected => |rejection| {
            try std.testing.expectEqual(
                host_input_admission.Rejection.terminal_exited,
                rejection,
            );
            try std.testing.expectEqualDeep(
                InputFailure{ .code = "invalid-phase", .retryable = false },
                inputFailure(rejection),
            );
        },
        .accepted, .core_rejected, .runtime_missing => return error.ExpectedPostExitInputRejection,
    }
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), inputEntryCount(runtime));
}
test "InputAccepted stays queue-admission-only while terminal replies queue ahead of the client entry" {
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
        "5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d",
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
    try testHandshake(&service, &connection, "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e");
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x5d} ** 16,
    );
    defer created.deinit();
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x5f} ** 16);
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
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
        [_]u8{0x5f} ** 16,
    );
    defer attached.deinit();
    const runtime = findRuntime(&service, terminal_id).?;

    // An unpublished first client entry parks the writer, so accepted client
    // bytes are provably not delivered while the queue is inspected.
    var publication_barrier = try runtime.prepareRawInput("b");
    defer publication_barrier.deinit();
    try std.testing.expectEqual(@as(usize, 1), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 1), inputEntryCount(runtime));

    // A child continuously emits reply-producing queries ahead of the client.
    parseTerminalInput(runtime, "\x1b[c");
    parseTerminalInput(runtime, "\x1b[c");
    parseTerminalInput(runtime, "\x1b[c");

    // A client WriteInput is admitted to the ordered bounded queue. InputAccepted
    // is queue-admission-only: it reports the exact committed byte count at
    // admission and does not promise the bytes reached the terminal writer, so
    // it is unaffected by the reply burst sitting ahead.
    var input_payload: [24 + 3]u8 = undefined;
    @memset(input_payload[0..16], 0x5f);
    std.mem.writeInt(u64, input_payload[16..24], 1, .little);
    @memcpy(input_payload[24..], "abc");
    var accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &input_payload,
        [_]u8{0x60} ** 16,
    );
    defer accepted.deinit();
    try expectInputAcceptedBytes(&accepted, attachment_id, 3, "1");
    // The acknowledgement committed the client bytes to the queue; nothing has
    // been delivered to the terminal writer yet.
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    try std.testing.expect(runtime.pendingInputBytes() >= 4);
}
