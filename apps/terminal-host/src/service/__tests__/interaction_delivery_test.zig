const harness = @import("interaction_harness.zig");
const serviceAggregate = harness.serviceAggregate;
const serviceCore = harness.serviceCore;
const std = harness.std;
const target_builtin = harness.target_builtin;
const control_boundary = harness.control_boundary;
const core_boundary = harness.core_boundary;
const engine_boundary = harness.engine_boundary;
const protocol = harness.protocol;
const service_boundary = harness.service_boundary;
const core_module = harness.core_module;
const control_encode = harness.control_encode;
const control_wire = harness.control_wire;
const host_outgoing = harness.host_outgoing;
const host_response = service_boundary.testing.response;
const host_runtime_aggregate = harness.host_runtime_aggregate;
const subject = harness.subject;
const service_subject = harness.service_subject;
const terminal_launch = harness.terminal_launch;
const terminal_runtime = harness.terminal_runtime;
const commitReaderBatch = harness.commitReaderBatch;
const createModel = harness.createModel;
const createWithOptionsUsingSpawner = harness.createWithOptionsUsingSpawner;
const recordReaderFailure = harness.recordReaderFailure;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const SchedulerTestingHooks = harness.SchedulerTestingHooks;
const features = harness.features;
const palette = harness.palette;
const RecordingAllocator = harness.RecordingAllocator;
const testId = harness.testId;
const testLaunch = harness.testLaunch;
const testLaunchWithGeometry = harness.testLaunchWithGeometry;
const TestRuntime = harness.TestRuntime;
const createReadableRuntime = harness.createReadableRuntime;
const createReadableRuntimeWithGeometry = harness.createReadableRuntimeWithGeometry;
const HookedThreadSpawner = harness.HookedThreadSpawner;
const createHookedReadableRuntime = harness.createHookedReadableRuntime;
const ContainmentFailureProbe = harness.ContainmentFailureProbe;
const CancellationCommitBarrier = harness.CancellationCommitBarrier;
const queueSelectionExportCancellation = harness.queueSelectionExportCancellation;
const pendingCancellation = harness.pendingCancellation;
const expectCancellationRecord = harness.expectCancellationRecord;
const retainRows = harness.retainRows;
const requestRenderRows = harness.requestRenderRows;
const fillOutgoingLeaving = harness.fillOutgoingLeaving;
const expectBellRecord = harness.expectBellRecord;
const HistoryChangedSummary = harness.HistoryChangedSummary;
const historyChangedSummary = harness.historyChangedSummary;
const dispatchHistoryObserved = harness.dispatchHistoryObserved;
const dispatchFrameApplied = harness.dispatchFrameApplied;
const dispatchRowsApplied = harness.dispatchRowsApplied;
const dispatchControlAcknowledgement = harness.dispatchControlAcknowledgement;
const dispatchSelectionExportChunkApplied = harness.dispatchSelectionExportChunkApplied;
const exportChunkInBatch = harness.exportChunkInBatch;
const expectSameKindAcknowledgementSuccess = harness.expectSameKindAcknowledgementSuccess;
const expectExportChunkAcknowledgementFinished = harness.expectExportChunkAcknowledgementFinished;

test "interaction rejects an incompatible success codec before request parsing" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    var dispatcher = service_subject.testing.RequestDispatcherType.init(
        std.testing.allocator,
        &aggregate,
        false,
    );
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var request = protocol.messageContract(protocol.MessageKind.read_rows).?.request.?;
    request.success_kind = protocol.MessageKind.ping;
    const correlation_id = [_]u8{0x19} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.read_rows,
        request.payload_encoding,
        "{}",
        correlation_id,
        aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    const envelope = try protocol.validateTransportEnvelope(record);

    try std.testing.expectError(
        error.IncompatibleSuccessResponseEncoding,
        dispatcher.dispatch(
            request,
            1,
            .{
                .input_bytes = protocol.Limits.input_bytes,
                .row_window_bytes = protocol.Limits.row_window_bytes,
            },
            envelope,
            correlation_id,
            &outgoing,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}

test "readable interaction capability distinguishes authorization from runtime ownership" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x22);
    const readable_attachment_id = testId(core_module.AttachmentId, 0x23);
    const input_attachment_id = testId(core_module.AttachmentId, 0x24);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x20),
    );
    _ = try createModel(
        aggregate.core(),
        connection_id,
        testId(core_module.CreationId, 0x21),
        terminal_id,
        [_]u8{0x25} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().attach(
        connection_id,
        readable_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    _ = try aggregate.core().attach(
        connection_id,
        input_attachment_id,
        terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );

    const controller = subject.Controller.init(&aggregate);
    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        controller.historyAccess(connection_id, readable_attachment_id),
    );
    try std.testing.expectError(
        error.InteractionAccessUnavailable,
        controller.historyAccess(connection_id, input_attachment_id),
    );
    try std.testing.expectError(
        error.InteractionAccessUnavailable,
        controller.historyAccess(connection_id + 1, readable_attachment_id),
    );

    _ = try aggregate.core().detach(connection_id, readable_attachment_id);
    _ = try aggregate.core().detach(connection_id, input_attachment_id);
    _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try aggregate.core().ptyDrained(terminal_id);
    const pending = aggregate.core().findTerminal(terminal_id).?.pendingOutcome() orelse
        return error.ExpectedPendingOutcome;
    _ = aggregate.core().markOutcomePublished(switch (pending) {
        .exited => |outcome| outcome.token,
        .failed => |outcome| outcome.token,
    });
    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}
test "empty interaction scheduler completes without producing a batch" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    var scheduler: subject.Scheduler = .{};
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(std.testing.allocator, &aggregate, 1, &outgoing),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}
test "bell delivery is per attachment, ordered, and baselined at admission" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const first_attachment_id = testId(core_module.AttachmentId, 0x81);
    const second_attachment_id = testId(core_module.AttachmentId, 0x82);
    const binding = try createReadableRuntime(&aggregate, connection_id, first_attachment_id);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(binding.runtime, "\x07\x07"),
    );
    _ = try aggregate.attach(
        connection_id,
        second_attachment_id,
        binding.terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );

    var scheduler: subject.Scheduler = .{};
    var first = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &first);
    try std.testing.expectEqual(@as(usize, 1), first.records().len);
    try expectBellRecord(first.records()[0], first_attachment_id, 1);

    var second = host_outgoing.Outgoing.init(std.testing.allocator);
    defer second.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &second);
    try std.testing.expectEqual(@as(usize, 1), second.records().len);
    try expectBellRecord(second.records()[0], first_attachment_id, 2);

    var drained = host_outgoing.Outgoing.init(std.testing.allocator);
    defer drained.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &drained);
    try std.testing.expectEqual(@as(usize, 0), drained.records().len);

    _ = try aggregate.detach(connection_id, first_attachment_id);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(binding.runtime, "\x07"),
    );
    var after_detach = host_outgoing.Outgoing.init(std.testing.allocator);
    defer after_detach.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &after_detach,
    );
    try std.testing.expectEqual(@as(usize, 1), after_detach.records().len);
    try expectBellRecord(after_detach.records()[0], second_attachment_id, 3);
}
test "a full outgoing batch retries the same bell sequence" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x83);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(binding.runtime, "\x07"),
    );

    var scheduler: subject.Scheduler = .{};
    var full = host_outgoing.Outgoing.init(std.testing.allocator);
    defer full.deinit();
    try fillOutgoingLeaving(&full, aggregate.core().epoch, 64);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &full),
    );
    try std.testing.expectEqual(@as(usize, 2), full.records().len);

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &retry);
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    try expectBellRecord(retry.records()[0], attachment_id, 1);
}
test "OSC 52 delivery retries transactionally and selects only one readable attachment" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const first_attachment_id = testId(core_module.AttachmentId, 0x84);
    const second_attachment_id = testId(core_module.AttachmentId, 0x85);
    const reconnect_attachment_id = testId(core_module.AttachmentId, 0x86);
    const binding = try createReadableRuntime(&aggregate, connection_id, first_attachment_id);
    _ = try aggregate.attach(
        connection_id,
        second_attachment_id,
        binding.terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(binding.runtime, "\x1b]52;c;b25jZQ==\x1b\\"),
    );

    var scheduler: subject.Scheduler = .{};
    var full = host_outgoing.Outgoing.init(std.testing.allocator);
    defer full.deinit();
    try fillOutgoingLeaving(&full, aggregate.core().epoch, 64);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &full),
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &retry);
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    const envelope = try protocol.validateTransportEnvelope(retry.records()[0]);
    try std.testing.expectEqual(
        protocol.MessageKind.terminal_clipboard_write_request,
        envelope.kind,
    );
    const Event = struct {
        attachmentId: []const u8,
        sequence: []const u8,
        text: []const u8,
    };
    const event = try control_wire.decode(Event, std.testing.allocator, envelope.payload);
    defer event.deinit();
    var first_buffer: [32]u8 = undefined;
    var second_buffer: [32]u8 = undefined;
    const routed_to_first = std.mem.eql(
        u8,
        event.value.attachmentId,
        control_encode.identifierText(first_attachment_id, &first_buffer),
    );
    const routed_to_second = std.mem.eql(
        u8,
        event.value.attachmentId,
        control_encode.identifierText(second_attachment_id, &second_buffer),
    );
    try std.testing.expect(routed_to_first or routed_to_second);
    try std.testing.expectEqualStrings("1", event.value.sequence);
    try std.testing.expectEqualStrings("once", event.value.text);

    var drained = host_outgoing.Outgoing.init(std.testing.allocator);
    defer drained.deinit();
    _ = try scheduler.schedule(std.testing.allocator, &aggregate, connection_id, &drained);
    try std.testing.expectEqual(@as(usize, 0), drained.records().len);

    _ = try aggregate.attach(
        connection_id,
        reconnect_attachment_id,
        binding.terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    var after_reconnect = host_outgoing.Outgoing.init(std.testing.allocator);
    defer after_reconnect.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &after_reconnect,
    );
    try std.testing.expectEqual(@as(usize, 0), after_reconnect.records().len);
}
test "selection capability binds identity and provisional export rollback restores sequence" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x32);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    const interaction = try subject.Controller.init(&aggregate).selectionAccess(
        connection_id,
        attachment_id,
    );
    const geometry = binding.runtime.geometry();
    _ = try interaction.applySelectionCommand(.{
        .command = .{ .set_range = .{
            .end = .{ .column = 0, .row = 0 },
            .rectangle = false,
            .start = .{ .column = 0, .row = 0 },
        } },
        .generation = geometry.generation,
        .revision = geometry.revision,
    });

    var provisional = try interaction.startSelectionExport(7, .plain);
    try std.testing.expectEqual(@as(u64, 7), provisional.result().started.export_sequence);
    provisional.deinit();

    var retried = try interaction.startSelectionExport(7, .plain);
    try std.testing.expectEqual(@as(u64, 7), retried.result().started.export_sequence);
    retried.commit();
    retried.deinit();
    _ = try interaction.cancelSelectionExport(7);
}
test "read rows service boundary maps oversized render geometry to limit exceeded" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const valid_row_count: u16 = 3_640;
    const invalid_row_count: u16 = valid_row_count + 1;
    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x3a);
    const binding = try createReadableRuntimeWithGeometry(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
        8,
        24,
        8,
        18,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    try retainRows(binding.runtime, invalid_row_count);
    const geometry = binding.runtime.geometry();

    var oversized = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        1,
        invalid_row_count,
        0x41,
    );
    defer oversized.deinit();
    try std.testing.expectEqual(@as(usize, 1), oversized.records().len);
    const failure_envelope = try protocol.validateTransportEnvelope(oversized.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.typed_failure, failure_envelope.kind);
    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const failure = try control_wire.decode(
        Failure,
        std.testing.allocator,
        failure_envelope.payload,
    );
    defer failure.deinit();
    try std.testing.expectEqualStrings("limit-exceeded", failure.value.code);

    var adjacent = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        1,
        valid_row_count,
        0x42,
    );
    defer adjacent.deinit();
    try std.testing.expectEqual(@as(usize, 1), adjacent.records().len);
    const rows_envelope = try protocol.validateTransportEnvelope(adjacent.records()[0]);
    const expected = try host_response.resolveSuccessResponse(
        protocol.messageContract(protocol.MessageKind.read_rows).?.request.?,
    );
    try std.testing.expectEqual(expected.kind, rows_envelope.kind);
    try std.testing.expectEqual(@intFromEnum(expected.encoding), rows_envelope.payload_encoding);
    const summary = try protocol.validateTerminalRowWindow(
        std.testing.allocator,
        rows_envelope.payload,
    );
    try std.testing.expectEqual(valid_row_count, summary.row_count);
    try std.testing.expectEqual(@as(u64, 1), summary.sequence);
}
test "read rows service boundary allocates for encoded bytes instead of the negotiated maximum" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var recording = RecordingAllocator.init(std.testing.allocator);
    var service = service_subject.HostService.init(recording.allocator());
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x3b);
    const binding = try createReadableRuntimeWithGeometry(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
        1,
        1,
        8,
        16,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    const geometry = binding.runtime.geometry();
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"{d}\",\"maximumBytes\":{d},\"origin\":{{\"kind\":\"absolute\",\"row\":\"0\"}},\"purpose\":\"search\",\"requestSequence\":\"1\",\"revision\":\"{d}\",\"revisionPolicy\":\"exact\",\"rowCount\":1}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            geometry.generation,
            protocol.Limits.row_window_bytes,
            geometry.revision,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.read_rows,
        .control_json,
        payload,
        [_]u8{0x43} ** 16,
        serviceCore(&service).epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);

    recording.reset();
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();

    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    const expected = try host_response.resolveSuccessResponse(
        protocol.messageContract(protocol.MessageKind.read_rows).?.request.?,
    );
    try std.testing.expectEqual(expected.kind, envelope.kind);
    try std.testing.expectEqual(@intFromEnum(expected.encoding), envelope.payload_encoding);
    try std.testing.expectEqual(@as(usize, 124), envelope.payload.len);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, envelope.payload);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u16, 1), summary.columns);
    try std.testing.expectEqual(@as(u16, 1), summary.row_count);
    try std.testing.expectEqual(@as(u32, 0), summary.entry_count);
    try std.testing.expect(recording.alloc_request_count > 0);
    try std.testing.expectEqual(@as(usize, 0), recording.row_window_request_count);
    try std.testing.expect(recording.max_requested < protocol.Limits.row_window_bytes);
}
