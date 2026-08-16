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

test "THCR-013 service schedules distinct wire sequences and the exact acknowledgement unblocks later mutation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x16);
    const binding = try createReadableRuntime(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    var scheduler: subject.Scheduler = .{};

    var first = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        serviceAggregate(&service),
        connection_id,
        &first,
    );
    try std.testing.expectEqual(@as(usize, 1), first.records().len);
    const first_event = try historyChangedSummary(first.records()[0]);
    try std.testing.expectEqual(@as(u64, 1), first_event.sequence);
    var ack1 = try dispatchHistoryObserved(
        &service,
        &connection,
        attachment_id,
        first_event.sequence,
        first_event.revision,
        first_event.generation,
        0x61,
    );
    defer ack1.deinit();

    // Leave and re-enter viewing history without a terminal mutation, so the
    // next event carries the same revision/generation identity.
    try binding.runtime.setHistoryDemand(attachment_id, .following_bottom);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);

    var second = host_outgoing.Outgoing.init(std.testing.allocator);
    defer second.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        serviceAggregate(&service),
        connection_id,
        &second,
    );
    try std.testing.expectEqual(@as(usize, 1), second.records().len);
    const second_event = try historyChangedSummary(second.records()[0]);
    try std.testing.expectEqual(first_event.sequence + 1, second_event.sequence);
    try std.testing.expectEqual(first_event.revision, second_event.revision);
    try std.testing.expectEqual(first_event.generation, second_event.generation);
    var ack2 = try dispatchHistoryObserved(
        &service,
        &connection,
        attachment_id,
        second_event.sequence,
        second_event.revision,
        second_event.generation,
        0x62,
    );
    defer ack2.deinit();

    // Mutate and confirm the scheduler emits the next history event after the
    // exact second-sequence acknowledgement.
    std.debug.assert(commitReaderBatch(binding.runtime, "x") == .committed);
    var third = host_outgoing.Outgoing.init(std.testing.allocator);
    defer third.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        serviceAggregate(&service),
        connection_id,
        &third,
    );
    try std.testing.expectEqual(@as(usize, 1), third.records().len);
    const third_event = try historyChangedSummary(third.records()[0]);
    try std.testing.expectEqual(second_event.sequence + 1, third_event.sequence);
    try std.testing.expectEqual(second_event.revision + 1, third_event.revision);
}
test "THCR-029 frame late duplicate request returns the same-kind success response" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x1d);
    _ = try createReadableRuntime(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    _ = try serviceAggregate(&service).setProjectionDemand(connection_id, attachment_id, .active);
    const first = try serviceCore(&service).commitFrame(
        attachment_id,
        null,
        1,
        1,
        .snapshot,
    );
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    _ = try serviceCore(&service).frameApplied(
        connection_id,
        attachment_id,
        1,
        1,
        1,
    );
    const second = try serviceCore(&service).commitFrame(
        attachment_id,
        1,
        2,
        1,
        .delta,
    );
    try std.testing.expectEqual(@as(u64, 2), second.sequence);

    // Replaying frame 1's acknowledgement while frame 2 is in flight must be
    // an idempotent no-op returned as the registered frame_applied success.
    var replayed = try dispatchFrameApplied(
        &service,
        &connection,
        attachment_id,
        1,
        1,
        1,
        0x21,
    );
    defer replayed.deinit();
    try expectSameKindAcknowledgementSuccess(
        &replayed,
        protocol.MessageKind.frame_applied,
        0x21,
    );
    try std.testing.expectEqual(
        @as(u64, 2),
        serviceCore(&service).findAttachment(attachment_id).?.projectionInFlight().?.frame.sequence,
    );

    var applied = try dispatchFrameApplied(
        &service,
        &connection,
        attachment_id,
        2,
        2,
        1,
        0x22,
    );
    defer applied.deinit();
    try expectSameKindAcknowledgementSuccess(
        &applied,
        protocol.MessageKind.frame_applied,
        0x22,
    );
}
test "THCR-029 rows late duplicate request returns the same-kind success response" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x1e);
    const binding = try createReadableRuntime(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
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

    var first = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        1,
        1,
        0x23,
    );
    defer first.deinit();
    try std.testing.expectEqual(
        protocol.MessageKind.rows_snapshot,
        (try protocol.validateTransportEnvelope(first.records()[0])).kind,
    );
    var applied_first = try dispatchRowsApplied(
        &service,
        &connection,
        attachment_id,
        1,
        geometry.revision,
        geometry.generation,
        0x24,
    );
    defer applied_first.deinit();
    try expectSameKindAcknowledgementSuccess(
        &applied_first,
        protocol.MessageKind.rows_applied,
        0x24,
    );

    var second = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        2,
        1,
        0x25,
    );
    defer second.deinit();
    try std.testing.expectEqual(
        protocol.MessageKind.rows_snapshot,
        (try protocol.validateTransportEnvelope(second.records()[0])).kind,
    );

    // Replaying window 1's acknowledgement while window 2 is pending must be
    // an idempotent no-op returned as the registered rows_applied success.
    var replayed = try dispatchRowsApplied(
        &service,
        &connection,
        attachment_id,
        1,
        geometry.revision,
        geometry.generation,
        0x26,
    );
    defer replayed.deinit();
    try expectSameKindAcknowledgementSuccess(
        &replayed,
        protocol.MessageKind.rows_applied,
        0x26,
    );

    var applied_second = try dispatchRowsApplied(
        &service,
        &connection,
        attachment_id,
        2,
        geometry.revision,
        geometry.generation,
        0x27,
    );
    defer applied_second.deinit();
    try expectSameKindAcknowledgementSuccess(
        &applied_second,
        protocol.MessageKind.rows_applied,
        0x27,
    );
}
test "THCR-029 export late duplicate request returns the same-kind success response" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x1f);
    const binding = try createReadableRuntime(
        serviceAggregate(&service),
        connection_id,
        attachment_id,
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
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .{ .set_range = .{
            .end = .{ .column = 0, .row = 0 },
            .rectangle = false,
            .start = .{ .column = 0, .row = 0 },
        } },
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = (try binding.runtime.startSelectionExport(
        attachment_id,
        7,
        .plain,
    )).started;

    var first_batch = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first_batch.deinit();
    var scheduler: subject.Scheduler = .{};
    _ = try scheduler.schedule(
        std.testing.allocator,
        serviceAggregate(&service),
        connection_id,
        &first_batch,
    );
    try std.testing.expectEqual(@as(u32, 0), (try exportChunkInBatch(&first_batch, 7)).?);
    var completed_seven = try dispatchSelectionExportChunkApplied(
        &service,
        &connection,
        attachment_id,
        7,
        0,
        0x28,
    );
    defer completed_seven.deinit();
    try expectExportChunkAcknowledgementFinished(
        &completed_seven,
        0x28,
    );

    _ = (try binding.runtime.startSelectionExport(
        attachment_id,
        8,
        .plain,
    )).started;
    var second_batch = host_outgoing.Outgoing.init(std.testing.allocator);
    defer second_batch.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        serviceAggregate(&service),
        connection_id,
        &second_batch,
    );
    try std.testing.expectEqual(@as(u32, 0), (try exportChunkInBatch(&second_batch, 8)).?);

    // Replaying export 7's final chunk acknowledgement while export 8 is in
    // flight must be an idempotent no-op with the registered same-kind success
    // and no new finished event.
    var replayed_seven = try dispatchSelectionExportChunkApplied(
        &service,
        &connection,
        attachment_id,
        7,
        0,
        0x29,
    );
    defer replayed_seven.deinit();
    try expectSameKindAcknowledgementSuccess(
        &replayed_seven,
        protocol.MessageKind.selection_export_chunk_applied,
        0x29,
    );
    try std.testing.expectEqual(@as(usize, 1), replayed_seven.records().len);

    var applied_eight = try dispatchSelectionExportChunkApplied(
        &service,
        &connection,
        attachment_id,
        8,
        0,
        0x2a,
    );
    defer applied_eight.deinit();
    try expectExportChunkAcknowledgementFinished(
        &applied_eight,
        0x2a,
    );
}
