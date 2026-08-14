const harness = @import("interaction_harness.zig");
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

test "batch full commits the interaction prefix and retries the current attachment" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x42);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    const geometry = binding.runtime.geometry();
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );

    var scheduler: subject.Scheduler = .{};
    var first = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first.deinit();
    try fillOutgoingLeaving(&first, aggregate.core().epoch, 256);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &first,
        ),
    );
    try std.testing.expectEqual(@as(usize, 3), first.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(first.records()[2])).kind,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.selection_changed,
        (try protocol.validateTransportEnvelope(retry.records()[0])).kind,
    );
}
test "allocator failure rolls back every earlier interaction mutation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x52);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    const geometry = binding.runtime.geometry();
    const sampled_history = (try binding.runtime.takeHistoryEvent(attachment_id)).?;
    var allocation_counter = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{},
    );
    const counting_allocator = allocation_counter.allocator();
    const sampled_payload = try control_encode.historyChanged(
        counting_allocator,
        sampled_history,
    );
    counting_allocator.free(sampled_payload);
    const history_allocation_count = allocation_counter.alloc_index;
    try binding.runtime.rollbackHistoryEvent(attachment_id, sampled_history);
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.startSelectionExport(attachment_id, 1, .plain);

    var scheduler: subject.Scheduler = .{};
    var failed = host_outgoing.Outgoing.init(std.testing.allocator);
    defer failed.deinit();
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        // Let history encoding complete, then fail the first allocation for
        // the later selection record. The scheduler must roll the already
        // taken history event back even though its record reached this batch.
        .{ .fail_index = history_allocation_count },
    );
    try std.testing.expectError(
        error.OutOfMemory,
        scheduler.schedule(
            failing.allocator(),
            &aggregate,
            connection_id,
            &failed,
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), failed.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(failed.records()[0])).kind,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 2), retry.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(retry.records()[0])).kind,
    );
    try std.testing.expectEqual(
        protocol.MessageKind.selection_export_chunk,
        (try protocol.validateTransportEnvelope(retry.records()[1])).kind,
    );
}
test "full cancellation record remains queued until a later batch commits it" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x62);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    const geometry = binding.runtime.geometry();
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.startSelectionExport(attachment_id, 1, .plain);
    _ = try binding.runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );
    try std.testing.expectEqual(
        terminal_runtime.SelectionSnapshot.active,
        std.meta.activeTag((try binding.runtime.takeSelectionEvent(attachment_id)).?),
    );

    var scheduler: subject.Scheduler = .{};
    var full = host_outgoing.Outgoing.init(std.testing.allocator);
    defer full.deinit();
    try fillOutgoingLeaving(&full, aggregate.core().epoch, 64);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &full,
        ),
    );
    try std.testing.expect(
        (try binding.runtime.prepareSelectionExportCancellation(attachment_id)) != null,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    const finished = try protocol.validateTransportEnvelope(retry.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.selection_export_finished, finished.kind);
    try std.testing.expect(std.mem.indexOf(u8, finished.payload, "\"status\":\"cancelled\"") != null);
    try std.testing.expect(
        (try binding.runtime.prepareSelectionExportCancellation(attachment_id)) == null,
    );
}
test "THCR-001 scheduler commits an appended cancellation across fatal containment failure" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: ContainmentFailureProbe = .{};
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    defer probe.allow_success = true;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x70));
    const attachment_id = testId(core_module.AttachmentId, 0x71);
    const binding = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x72),
        testId(core_module.TerminalId, 0x73),
        attachment_id,
        probe.hooks(),
    );
    probe.runtime = binding.runtime;
    const cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        attachment_id,
        1,
    );
    try std.testing.expectEqualDeep(
        cancellation,
        (try pendingCancellation(binding.runtime, attachment_id)).?,
    );

    var barrier: CancellationCommitBarrier = .{ .runtime = binding.runtime };
    var scheduler: subject.Scheduler = .{};
    subject.testing.setSchedulerTestingHooks(&scheduler, barrier.hooks());
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &outgoing,
        ),
    );

    try std.testing.expectEqual(@as(usize, 1), barrier.calls);
    try std.testing.expectEqual(@as(usize, 1), probe.contain_calls);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try expectCancellationRecord(outgoing.records()[0], cancellation);
    try std.testing.expect((try pendingCancellation(binding.runtime, attachment_id)) == null);
    try std.testing.expect(!binding.runtime.readableStateAvailable());
    try std.testing.expectEqual(
        terminal_runtime.RuntimeFailureKind.pty_read,
        binding.runtime.primaryFailure().?,
    );

    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(@as(usize, 2), probe.contain_calls);
    const blocked = aggregate.core().findTerminal(binding.terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.containing_failure, blocked.phase());
    try std.testing.expectEqual(core_module.RuntimeFailure.read_output, blocked.runtimeFailure().?);
    switch (blocked.failureContainment().?) {
        .blocked => |containment| try std.testing.expectEqual(
            core_module.RuntimeCleanupFailure.contain_process,
            containment.cleanup_failure,
        ),
        .stopping => return error.ExpectedBlockedContainment,
    }

    _ = try aggregate.core().failureContainmentBlocked(
        binding.terminal_id,
        .contain_process,
        0,
    );
    probe.allow_success = true;
    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(@as(usize, 3), probe.contain_calls);
    const retained_failure = aggregate.core().findTerminal(binding.terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.failed, retained_failure.phase());
    try std.testing.expectEqual(
        core_module.RuntimeFailure.read_output,
        retained_failure.runtimeFailure().?,
    );
}
test "THCR-001 complete two-cancellation batch has no committed prefix" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var later_probe: ContainmentFailureProbe = .{};
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    defer later_probe.allow_success = true;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x80));
    const first_attachment_id = testId(core_module.AttachmentId, 0x81);
    const first = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x82),
        testId(core_module.TerminalId, 0x83),
        first_attachment_id,
        .{},
    );
    const later_attachment_id = testId(core_module.AttachmentId, 0x84);
    const later = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x85),
        testId(core_module.TerminalId, 0x86),
        later_attachment_id,
        later_probe.hooks(),
    );
    later_probe.runtime = later.runtime;
    const first_cancellation = try queueSelectionExportCancellation(
        first.runtime,
        first_attachment_id,
        11,
    );
    const later_cancellation = try queueSelectionExportCancellation(
        later.runtime,
        later_attachment_id,
        22,
    );

    var barrier: CancellationCommitBarrier = .{ .runtime = later.runtime };
    var scheduler: subject.Scheduler = .{};
    subject.testing.setSchedulerTestingHooks(&scheduler, barrier.hooks());
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &outgoing,
        ),
    );

    try std.testing.expectEqual(@as(usize, 1), barrier.calls);
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    try expectCancellationRecord(outgoing.records()[0], first_cancellation);
    try expectCancellationRecord(outgoing.records()[1], later_cancellation);
    try std.testing.expect((try pendingCancellation(first.runtime, first_attachment_id)) == null);
    try std.testing.expect((try pendingCancellation(later.runtime, later_attachment_id)) == null);
    try std.testing.expect(first.runtime.readableStateAvailable());
    try std.testing.expect(!later.runtime.readableStateAvailable());
}
test "THCR-001 allocator failure preserves every prepared cancellation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const first_attachment_id = testId(core_module.AttachmentId, 0x92);
    const binding = try createReadableRuntime(&aggregate, connection_id, first_attachment_id);
    const second_attachment_id = testId(core_module.AttachmentId, 0x93);
    _ = try aggregate.attach(
        connection_id,
        second_attachment_id,
        binding.terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    const first_cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        first_attachment_id,
        31,
    );
    const second_cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        second_attachment_id,
        32,
    );
    while (try binding.runtime.takeSelectionEvent(first_attachment_id)) |_| {}
    while (try binding.runtime.takeSelectionEvent(second_attachment_id)) |_| {}

    var allocation_counter = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{},
    );
    const counting_allocator = allocation_counter.allocator();
    const sampled_payload = try control_encode.selectionExportFinished(
        counting_allocator,
        first_cancellation,
        subject.SelectionExportStatus.cancelled,
    );
    counting_allocator.free(sampled_payload);
    const cancellation_allocation_count = allocation_counter.alloc_index;

    var scheduler: subject.Scheduler = .{};
    var failed = host_outgoing.Outgoing.init(std.testing.allocator);
    defer failed.deinit();
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = cancellation_allocation_count },
    );
    // The allocating writer maps its backing allocator exhaustion to WriteFailed.
    try std.testing.expectError(
        error.WriteFailed,
        scheduler.schedule(
            failing.allocator(),
            &aggregate,
            connection_id,
            &failed,
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), failed.records().len);
    try expectCancellationRecord(failed.records()[0], first_cancellation);
    try std.testing.expectEqualDeep(
        first_cancellation,
        (try pendingCancellation(binding.runtime, first_attachment_id)).?,
    );
    try std.testing.expectEqualDeep(
        second_cancellation,
        (try pendingCancellation(binding.runtime, second_attachment_id)).?,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &retry,
        ),
    );
    try std.testing.expectEqual(@as(usize, 2), retry.records().len);
    try expectCancellationRecord(retry.records()[0], first_cancellation);
    try expectCancellationRecord(retry.records()[1], second_cancellation);
    try std.testing.expect((try pendingCancellation(binding.runtime, first_attachment_id)) == null);
    try std.testing.expect((try pendingCancellation(binding.runtime, second_attachment_id)) == null);
}
