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

test "retained exited terminals still project and export selection" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
    const runtime = findRuntime(&service, terminal_id).?;

    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
    const exited = serviceCore(&service).findTerminal(terminal_id).?;
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
    var projection_events = try pollEventsChecked(&service, &connection);
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
    var export_events = try pollEventsChecked(&service, &connection);
    defer export_events.deinit();
    for (export_events.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        chunked = chunked or envelope.kind == MessageKind.selection_export_chunk;
    }
    try std.testing.expect(chunked);
}
test "two visible attachments produce one real PTY focus interval" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "esc=$(printf '\\033'); printf '\\033[?1004hfocus-ready'; " ++
            "IFS= read -r value; " ++
            "if [ \"$value\" = \"${esc}[O${esc}[I${esc}[O\" ]; " ++
            "then printf 'focus-bytes:1b5b4f1b5b491b5b4f'; " ++
            "else printf 'focus-bytes:unexpected'; fi",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/Q",
        "/V:ON",
        "/S",
        "/C",
        "for /F \"delims=#\" %a in ('\"prompt #$E# & for %b in (1) do rem\"') " ++
            "do @set \"ESC=%a\" & <nul set /p =!ESC![?1004hfocus-ready & " ++
            "set /p value= & if \"!value!\"==\"!ESC![O!ESC![I!ESC![O\" " ++
            "(<nul set /p =focus-bytes:1b5b4f1b5b491b5b4f) else " ++
            "(<nul set /p =focus-bytes:unexpected)",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    var launch = aggregate_harness.validMissingExecutableLaunch();
    launch.executable = executable;
    launch.arguments = arguments;
    const validated = try aggregate_harness.terminal_launch.inspect(
        std.testing.allocator,
        launch,
    );

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "91919191919191919191919191919191");
    const active = try activeConnection(&connection);
    const aggregate = serviceAggregate(&service);
    var create = try aggregate.prepareCreateTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x92, 0),
        validated,
    );
    defer create.deinit();
    const terminal_id = switch (try aggregate.commitPreparedTerminal(&create)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const first_attachment = core_module.AttachmentId.fromBytes([_]u8{0x93} ** 16);
    const second_attachment = core_module.AttachmentId.fromBytes([_]u8{0x94} ** 16);
    for ([_]core_module.AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        _ = try aggregate.attach(
            active.connection_id,
            attachment_id,
            terminal_id,
            .{ .input = true, .read = true, .resize = false },
            false,
        );
        _ = try aggregate.setProjectionDemand(
            active.connection_id,
            attachment_id,
            .active,
        );
    }
    const runtime = findRuntime(&service, terminal_id).?;
    try semantic_wait.runtimeText(runtime, std.testing.allocator, "focus-ready");

    for ([_]struct { core_module.AttachmentId, bool, usize }{
        .{ first_attachment, true, 3 },
        .{ second_attachment, true, 0 },
        .{ first_attachment, false, 0 },
    }) |step| {
        switch (aggregate.acceptSemanticInput(
            active.connection_id,
            step[0],
            1,
            .{ .focus = .{ .focused = step[1] } },
            std.math.maxInt(usize),
        )) {
            .accepted => |accepted| try std.testing.expectEqual(step[2], accepted.byte_count),
            .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedFocusAcceptance,
        }
    }
    _ = try aggregate.setProjectionDemand(
        active.connection_id,
        second_attachment,
        .paused,
    );
    const line_end: []const u8 = if (builtin.os.tag == .windows) "\r\n" else "\n";
    switch (aggregate.acceptRawInput(
        active.connection_id,
        first_attachment,
        1,
        line_end,
    )) {
        .accepted => {},
        .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedInputAcceptance,
    }

    try semantic_wait.runtimeText(
        runtime,
        std.testing.allocator,
        "focus-bytes:1b5b4f1b5b491b5b4f",
    );
    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
}
test "two PTY runtimes route input independently and isolate spawn failure" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 1), serviceAggregate(&service).runtimeCount());
    const first_terminal_id = serviceCore(&service).terminalKeys()[0];

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
    try std.testing.expectEqual(@as(usize, 1), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 1), serviceAggregate(&service).runtimeCount());
    try std.testing.expect(serviceCore(&service).findTerminal(first_terminal_id) != null);

    var second_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        second_create_payload,
        [_]u8{0x53} ** 16,
    );
    defer second_created.deinit();
    try std.testing.expectEqual(@as(usize, 2), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 2), serviceAggregate(&service).runtimeCount());
    const second_terminal_id = serviceCore(&service).terminalKeys()[1];

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

    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{ first_terminal_id, second_terminal_id },
    );
    defer exit_events.deinit();
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        serviceCore(&service).findTerminal(first_terminal_id).?.phase(),
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        serviceCore(&service).findTerminal(second_terminal_id).?.phase(),
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
    try std.testing.expect(serviceCore(&service).findTerminal(first_terminal_id) == null);
    try std.testing.expect(findRuntime(&service, first_terminal_id) == null);
    try std.testing.expect(serviceCore(&service).findTerminal(second_terminal_id) != null);
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
    try std.testing.expectEqual(@as(usize, 0), serviceCore(&service).terminalCount());
    try std.testing.expectEqual(@as(usize, 0), serviceAggregate(&service).runtimeCount());
}
test "current protocol activates a lazy live projection and accepts semantic text" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
        .environmentOverlay = &.{},
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
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
    var demand_events = try pollEventsChecked(&service, &connection);
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
        serviceCore(&service).findAttachment(core_module.AttachmentId.fromBytes([_]u8{0x44} ** 16)).?.projectionInFlight() == null,
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

    try std.testing.expect(serviceAggregate(&service).setNextProjectionAt(terminal_id, 0));
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
    var resume_events = try pollEventsChecked(&service, &connection);
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
    const text_payload = try std.testing.allocator.alloc(u8, 28 + input_text.len);
    defer std.testing.allocator.free(text_payload);
    @memset(text_payload[0..16], 0x44);
    std.mem.writeInt(u64, text_payload[16..24], 1, .little);
    std.mem.writeInt(u32, text_payload[24..28], @intCast(input_text.len), .little);
    @memcpy(text_payload[28..], input_text);
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

    var exit_events = try semantic_wait.terminalExits(
        &service,
        &connection,
        &.{terminal_id},
    );
    defer exit_events.deinit();
    try std.testing.expectEqual(core_module.TerminalPhase.exited, serviceCore(&service).findTerminal(terminal_id).?.phase());
    const counters_after_exit = runtime.projectionCounters();
    try std.testing.expectEqual(counters_before_pause.captures, counters_after_exit.captures);
    try std.testing.expectEqual(counters_before_pause.encodes, counters_after_exit.encodes);
}
test "scheduler emits one uncorrelated cancellation for an invalidated export" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
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

    try semantic_wait.runtimeText(runtime, std.testing.allocator, "selected");

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

    var events = try pollEventsChecked(&service, &connection);
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

    var later_events = try pollEventsChecked(&service, &connection);
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
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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
    const terminal_id = serviceCore(&service).terminalKeys()[0];
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

    try semantic_wait.runtimeText(runtime, std.testing.allocator, "selected");

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

    var export_events = try pollEventsChecked(&service, &connection);
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
