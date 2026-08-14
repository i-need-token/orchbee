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
        inputFailure(.paste_mode_changed),
    );
}
test "protocol identifier failures retain their public error code" {
    try std.testing.expectEqualStrings(
        "invalid-identifier",
        protocolErrorCode(error.InvalidIdentifier),
    );
}
test "I/O successful responses use supplied request contracts and publish input once" {
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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

    // This unpublished first FIFO entry prevents the writer from consuming
    // later committed input while the test counts exact publications.
    var publication_barrier = try runtime.prepareRawInput("publication barrier");
    defer publication_barrier.deinit();

    var raw_payload = [_]u8{0} ** 17;
    @memcpy(raw_payload[0..16], &attachment_id.bytes);
    raw_payload[16] = 'r';

    var key_payload = [_]u8{0} ** 37;
    @memcpy(key_payload[0..16], &attachment_id.bytes);
    key_payload[16] = 1;
    std.mem.writeInt(u16, key_payload[22..24], 4, .little);
    std.mem.writeInt(u32, key_payload[24..28], 'a', .little);
    std.mem.writeInt(u32, key_payload[28..32], 1, .little);
    @memcpy(key_payload[32..36], "KeyA");
    key_payload[36] = 'a';

    var text_payload = [_]u8{0} ** 21;
    @memcpy(text_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u32, text_payload[16..20], 1, .little);
    text_payload[20] = 't';

    var mouse_payload = [_]u8{0} ** 32;
    @memcpy(mouse_payload[0..16], &attachment_id.bytes);
    mouse_payload[16] = 0;
    mouse_payload[17] = 1;
    std.mem.writeInt(u32, mouse_payload[20..24], @bitCast(@as(f32, 8)), .little);
    std.mem.writeInt(u32, mouse_payload[24..28], @bitCast(@as(f32, 16)), .little);
    std.mem.writeInt(u16, mouse_payload[28..30], 1, .little);

    var focus_payload = [_]u8{0} ** 20;
    @memcpy(focus_payload[0..16], &attachment_id.bytes);
    focus_payload[16] = 1;

    var paste_payload = [_]u8{0} ** 25;
    @memcpy(paste_payload[0..16], &attachment_id.bytes);
    std.mem.writeInt(u32, paste_payload[20..24], 1, .little);
    paste_payload[24] = 'p';

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
    for (input_cases, 0..) |input_case, index| {
        const request_contract = try syntheticIoRequestContract(
            input_case.kind,
            MessageKind.ping,
        );
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
        if (input_case.kind == MessageKind.write_input) {
            try std.testing.expectEqual(
                @as(usize, input_case.payload.len - 16),
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

    const resize_contract = try syntheticIoRequestContract(
        MessageKind.resize_terminal,
        MessageKind.ping,
    );
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

    const demand_contract = try syntheticIoRequestContract(
        MessageKind.set_projection_demand,
        MessageKind.ping,
    );
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
    var first_frame_batch = try service.pollEvents(&connection);
    defer first_frame_batch.deinit();
    const first_frame = try onlyTerminalFrame(&first_frame_batch);
    var need_snapshot_buffer: [160]u8 = undefined;
    const need_snapshot_payload = try std.fmt.bufPrint(
        &need_snapshot_buffer,
        "{{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"reason\":\"base-mismatch\",\"sequence\":\"{d}\"}}",
        .{first_frame.sequence},
    );
    const need_snapshot_contract = try syntheticIoRequestContract(
        MessageKind.need_snapshot,
        MessageKind.ping,
    );
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
    var second_frame_batch = try service.pollEvents(&connection);
    defer second_frame_batch.deinit();
    const second_frame = try onlyTerminalFrame(&second_frame_batch);
    var frame_applied_buffer: [256]u8 = undefined;
    const frame_applied_payload = try std.fmt.bufPrint(
        &frame_applied_buffer,
        "{{\"attachmentId\":\"4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{ second_frame.generation, second_frame.revision, second_frame.sequence },
    );
    const frame_applied_contract = try syntheticIoRequestContract(
        MessageKind.frame_applied,
        MessageKind.ping,
    );
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
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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

    const key_payload = try std.testing.allocator.alloc(u8, 32 + 4 + 4_096);
    defer std.testing.allocator.free(key_payload);
    @memset(key_payload, 0);
    @memcpy(key_payload[0..16], &first_attachment_id.bytes);
    key_payload[16] = 1;
    std.mem.writeInt(u16, key_payload[22..24], 4, .little);
    std.mem.writeInt(u32, key_payload[24..28], 'a', .little);
    std.mem.writeInt(u32, key_payload[28..32], 4_096, .little);
    @memcpy(key_payload[32..36], "KeyA");
    @memset(key_payload[36..], 'a');

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
    try expectInputAcceptedBytes(&accepted_key, attachment_id, 12_294);
    try std.testing.expectEqual(@as(usize, 12_295), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 2), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");
    var mouse_payload = [_]u8{0} ** 32;
    @memcpy(mouse_payload[0..16], &attachment_id.bytes);
    mouse_payload[16] = 0;
    mouse_payload[17] = 10;
    std.mem.writeInt(u16, mouse_payload[28..30], 1 << 9, .little);
    var accepted_ten = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5b} ** 16,
    );
    defer accepted_ten.deinit();
    try expectInputAcceptedBytes(&accepted_ten, attachment_id, 11);
    try std.testing.expectEqual(@as(usize, 12_306), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 3), inputEntryCount(runtime));

    mouse_payload[17] = 11;
    std.mem.writeInt(u16, mouse_payload[28..30], 1 << 10, .little);
    var accepted_eleven = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5c} ** 16,
    );
    defer accepted_eleven.deinit();
    try expectInputAcceptedBytes(&accepted_eleven, attachment_id, 11);
    try std.testing.expectEqual(@as(usize, 12_317), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 4), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1006l");
    mouse_payload[16] = 1;
    mouse_payload[17] = 1;
    std.mem.writeInt(u32, mouse_payload[20..24], 0x7f7fffff, .little);
    std.mem.writeInt(u16, mouse_payload[28..30], 0, .little);
    var accepted_boundary = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5d} ** 16,
    );
    defer accepted_boundary.deinit();
    try expectInputAcceptedBytes(&accepted_boundary, attachment_id, 6);
    try std.testing.expectEqual(@as(usize, 12_323), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 5), inputEntryCount(runtime));

    parseTerminalInput(runtime, "\x1b[?1000l");
    mouse_payload[16] = 0;
    mouse_payload[17] = 10;
    std.mem.writeInt(u32, mouse_payload[20..24], 0, .little);
    std.mem.writeInt(u16, mouse_payload[28..30], 1 << 9, .little);
    var accepted_noop = try testRequest(
        &service,
        &connection,
        MessageKind.mouse_input,
        .raw_bytes,
        &mouse_payload,
        [_]u8{0x5e} ** 16,
    );
    defer accepted_noop.deinit();
    try expectInputAcceptedBytes(&accepted_noop, attachment_id, 0);
    try std.testing.expectEqual(@as(usize, 12_323), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 5), inputEntryCount(runtime));
}
test "semantic input preparation commits to a terminal-exited rejection after client admission closes" {
    const windows_executable: ?[]u8 = if (target_builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
    var input_payload: [16 + 3]u8 = undefined;
    @memset(input_payload[0..16], 0x5f);
    @memcpy(input_payload[16..], "abc");
    var accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        &input_payload,
        [_]u8{0x60} ** 16,
    );
    defer accepted.deinit();
    try expectInputAcceptedBytes(&accepted, attachment_id, 3);
    // The acknowledgement committed the client bytes to the queue; nothing has
    // been delivered to the terminal writer yet.
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    try std.testing.expect(runtime.pendingInputBytes() >= 4);
}
