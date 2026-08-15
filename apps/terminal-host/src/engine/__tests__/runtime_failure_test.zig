const harness = @import("runtime_harness.zig");
const std = harness.std;
const target_builtin = harness.target_builtin;
const ghostty_vt = harness.ghostty_vt;
const engine_boundary = harness.engine_boundary;
const model_boundary = harness.model_boundary;
const platform_boundary = harness.platform_boundary;
const protocol = harness.protocol;
const semantic_wait = harness.semantic_wait;
const subject = harness.subject;
const terminal_launch = harness.terminal_launch;
const pty_process = harness.pty_process;
const applyScrollbackBytes = harness.applyScrollbackBytes;
const cancelInput = harness.cancelInput;
const clearTestingHooks = harness.clearTestingHooks;
const closeClientInput = harness.closeClientInput;
const commitReaderBatch = harness.commitReaderBatch;
const completionClassificationLoad = harness.completionClassificationLoad;
const createWithOptionsUsingSpawner = harness.createWithOptionsUsingSpawner;
const deinitOwnedProfile = harness.deinitOwnedProfile;
const FirstContainmentAttempt = harness.FirstContainmentAttempt;
const InputReservation = harness.InputReservation;
const InputSource = harness.InputSource;
const initOwnedProfile = harness.initOwnedProfile;
const inputQueueSnapshot = harness.inputQueueSnapshot;
const modifiers = harness.modifiers;
const mouseEncodeEvent = harness.mouseEncodeEvent;
const mouseHasPressedButton = harness.mouseHasPressedButton;
const markNormalCompletionReady = harness.markNormalCompletionReady;
const ProjectionEncodeRequest = harness.ProjectionEncodeRequest;
const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const reserveInput = harness.reserveInput;
const TerminalRuntime = harness.TerminalRuntime;
const runtimeState = harness.runtimeState;
const max_pending_input_bytes = harness.max_pending_input_bytes;
const max_pending_input_entries = harness.max_pending_input_entries;
const max_pending_terminal_reply_bytes = harness.max_pending_terminal_reply_bytes;
const max_writer_slice_bytes = harness.max_writer_slice_bytes;
const terminal_reply_burst_bound = harness.terminal_reply_burst_bound;
const publishInput = harness.publishInput;
const recordReaderFailure = harness.recordReaderFailure;
const recordTerminalReplyFailure = harness.recordTerminalReplyFailure;
const recordWorkerIntegrityFailure = harness.recordWorkerIntegrityFailure;
const recordWriterFailure = harness.recordWriterFailure;
const releaseOutputForShutdown = harness.releaseOutputForShutdown;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const resizePtyWithRollback = harness.resizePtyWithRollback;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const Geometry = harness.Geometry;
const requireProjectionFrame = harness.requireProjectionFrame;
const TestGate = harness.TestGate;
const TestSignal = harness.TestSignal;
const exerciseOwnedProfileAllocations = harness.exerciseOwnedProfileAllocations;
const FailingThreadSpawner = harness.FailingThreadSpawner;
const CountingThreadSpawner = harness.CountingThreadSpawner;
const HookedThreadSpawner = harness.HookedThreadSpawner;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const createRuntimeWithHooksAndCommand = harness.createRuntimeWithHooksAndCommand;
const waitForWriterStop = harness.waitForWriterStop;
const createIdleRuntime = harness.createIdleRuntime;
const createIdleRuntimeWithGeometry = harness.createIdleRuntimeWithGeometry;
const parseTerminalInput = harness.parseTerminalInput;
const WriterPause = harness.WriterPause;
const WriterProbe = harness.WriterProbe;
const RepeatingGate = harness.RepeatingGate;
const BurstProbe = harness.BurstProbe;
const DescendantProbe = harness.DescendantProbe;
const ReaderProbe = harness.ReaderProbe;
const PtyDrainRaceProbe = harness.PtyDrainRaceProbe;
const CanonicalSnapshot = harness.CanonicalSnapshot;
const canonicalSnapshot = harness.canonicalSnapshot;
const expectCanonicalSnapshotEqual = harness.expectCanonicalSnapshotEqual;
const readFrameInt = harness.readFrameInt;
const projectionFrameRowOffset = harness.projectionFrameRowOffset;
const expectIncludedProjectionRows = harness.expectIncludedProjectionRows;
const FrameHyperlinkObservation = harness.FrameHyperlinkObservation;
const observeFrameHyperlinks = harness.observeFrameHyperlinks;
const writeProjectionOutput = harness.writeProjectionOutput;
const expectProjectionRowRevisions = harness.expectProjectionRowRevisions;
const expectProjectionLinkTargets = harness.expectProjectionLinkTargets;
const expectInputQueuesCleared = harness.expectInputQueuesCleared;
const ContainmentProbe = harness.ContainmentProbe;
const FailureReporter = harness.FailureReporter;
const ShutdownProbe = harness.ShutdownProbe;
const OutputCleanupProbe = harness.OutputCleanupProbe;
const ResizeEffectOutcome = harness.ResizeEffectOutcome;
const ResizeProbe = harness.ResizeProbe;
const ResizeObservation = harness.ResizeObservation;
const TerminalDrivenResizeProbe = harness.TerminalDrivenResizeProbe;
const TerminalResizeSerializationProbe = harness.TerminalResizeSerializationProbe;
const ResizeCall = harness.ResizeCall;
const ReaderBatchCall = harness.ReaderBatchCall;
const expectRealPtyQueryReplies = harness.expectRealPtyQueryReplies;
const driveReplyBurstUntilClient = harness.driveReplyBurstUntilClient;
const parseDescendantPid = harness.parseDescendantPid;
const procCommEquals = harness.procCommEquals;
const processAlive = harness.processAlive;

test "prepared export cancellation commits after an uncontained fatal failure" {
    var probe: ContainmentProbe = .{ .pause_first_attempt = false };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    const attachment_id = AttachmentId.fromBytes([_]u8{0x91} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = false, .read = true, .resize = false },
    );
    defer attachment.deinit();
    attachment.commit();
    {
        runtimeState(runtime).terminal_mutex.lock();
        defer runtimeState(runtime).terminal_mutex.unlock();
        try runtimeState(runtime).terminal.printString("prepared cancellation");
    }

    const geometry = runtime.geometry();
    _ = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    const started = (try runtime.startSelectionExport(attachment_id, 1, .plain)).started;
    _ = try runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );
    _ = try runtime.takeSelectionEvent(attachment_id);
    const prepared = (try runtime.prepareSelectionExportCancellation(attachment_id)).?;
    try std.testing.expectEqual(started.byte_length, prepared.cancellation.byte_length);
    try std.testing.expectEqual(started.export_sequence, prepared.cancellation.export_sequence);
    try std.testing.expect(prepared.cancellation.attachment_id.eql(attachment_id));

    recordReaderFailure(runtime);
    try std.testing.expect(!runtime.readableStateAvailable());
    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.plainString(std.testing.allocator),
    );
    {
        runtimeState(runtime).terminal_mutex.lock();
        defer runtimeState(runtime).terminal_mutex.unlock();
        const retained = (try runtimeState(runtime).history.prepareSelectionExportCancellation(attachment_id)).?;
        try std.testing.expectEqualDeep(prepared, retained);
    }

    runtime.commitPreparedSelectionExportCancellation(prepared);
    {
        runtimeState(runtime).terminal_mutex.lock();
        defer runtimeState(runtime).terminal_mutex.unlock();
        try std.testing.expect(
            (try runtimeState(runtime).history.prepareSelectionExportCancellation(attachment_id)) == null,
        );
    }
}
test "terminal reply failure fences only after its coherent reader batch commits" {
    var probe: ReaderProbe = .{ .batch = "coherent-reply\x1b[c" };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer probe.batch_read.release();
    defer probe.batch_finished.release();

    probe.batch_read.wait();
    const reply_reservation = try reserveInput(
        runtime,
        max_pending_terminal_reply_bytes,
        false,
        .terminal,
    );
    _ = reply_reservation;
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();

    probe.batch_read.release();
    probe.batch_finished.wait();
    try std.testing.expectEqual(ReaderBatchOutcome.runtime_failed, probe.observedOutcome().?);
    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(
        subject.RuntimeFailureKind.terminal_reply,
        runtime.primaryFailure().?,
    );

    var committed = try canonicalSnapshot(runtime);
    defer committed.deinit();
    try std.testing.expectEqual(before.revision + 1, committed.revision);
    try std.testing.expect(std.mem.indexOf(u8, committed.text, "coherent-reply") != null);
    try std.testing.expectEqual(@as(u64, 1), runtime.takeOutputBatches());
    waitForWriterStop(runtime);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    probe.batch_finished.release();
    probe.stopped.wait();
    var frozen = try canonicalSnapshot(runtime);
    defer frozen.deinit();
    try std.testing.expectEqual(committed.revision, frozen.revision);
    try std.testing.expectEqualStrings(committed.text, frozen.text);
    try std.testing.expectEqual(@as(usize, 1), probe.readCallCount());
}
test "concurrent engine reporters leave the first containment attempt with the fence winner" {
    var probe: ContainmentProbe = .{ .pause_first_attempt = true };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }
    defer probe.first_attempt.release();

    var winner: FailureReporter = .{ .kind = .reader, .runtime = runtime };
    const winner_thread = try std.Thread.spawn(.{}, FailureReporter.run, .{&winner});
    probe.first_attempt.wait();

    var loser: FailureReporter = .{ .kind = .writer, .runtime = runtime };
    const loser_thread = try std.Thread.spawn(.{}, FailureReporter.run, .{&loser});
    loser.done.wait();
    loser_thread.join();
    const while_pending = probe.counts();

    probe.first_attempt.release();
    winner.done.wait();
    winner_thread.join();

    try std.testing.expectEqual(@as(usize, 1), while_pending.aborts);
    try std.testing.expectEqual(@as(usize, 1), while_pending.contains);
    try std.testing.expectEqual(
        FirstContainmentAttempt.done,
        subject.testing.firstContainmentAttempt(runtime),
    );
    try std.testing.expectEqual(subject.RuntimeFailureKind.pty_read, runtime.primaryFailure().?);
}
test "Host cleanup waits for a pending engine first containment attempt" {
    var probe: ContainmentProbe = .{ .pause_first_attempt = true };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }
    defer probe.first_attempt.release();

    var reporter: FailureReporter = .{ .kind = .reader, .runtime = runtime };
    const reporter_thread = try std.Thread.spawn(.{}, FailureReporter.run, .{&reporter});
    probe.first_attempt.wait();

    var shutdown: ShutdownProbe = .{ .runtime = runtime };
    const shutdown_thread = try std.Thread.spawn(.{}, ShutdownProbe.run, .{&shutdown});
    probe.host_waiting.wait();
    const before_engine_release = probe.counts();

    probe.first_attempt.release();
    reporter.done.wait();
    reporter_thread.join();
    shutdown.done.wait();
    shutdown_thread.join();

    try std.testing.expectEqual(@as(usize, 1), before_engine_release.aborts);
    try std.testing.expectEqual(@as(usize, 1), before_engine_release.contains);
    try std.testing.expectEqual(error.ProcessContainmentFailed, shutdown.observedFailure().?);
    try std.testing.expectEqual(@as(usize, 2), probe.counts().contains);
    try std.testing.expectEqual(
        FirstContainmentAttempt.done,
        subject.testing.firstContainmentAttempt(runtime),
    );
}
test "Host-originated failure fence reserves the first containment attempt for Host" {
    var probe: ContainmentProbe = .{ .pause_first_attempt = false };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    switch (runtime.fenceHostFailure()) {
        .host_won => {},
        .engine_won => return error.ExpectedHostFenceWinner,
    }
    try std.testing.expectEqual(
        FirstContainmentAttempt.host_pending,
        subject.testing.firstContainmentAttempt(runtime),
    );
    try std.testing.expectEqual(@as(usize, 0), probe.counts().contains);

    try std.testing.expectError(error.ProcessContainmentFailed, runtime.shutdown());
    try std.testing.expectEqual(@as(usize, 1), probe.counts().contains);
    try std.testing.expectEqual(
        FirstContainmentAttempt.done,
        subject.testing.firstContainmentAttempt(runtime),
    );
    try std.testing.expect(runtime.primaryFailure() == null);
}
test "output cleanup failure retry closes in-flight reader and writer lanes permanently" {
    const cases = [_]struct {
        abort_fails: bool,
        finish_fails: bool,
    }{
        .{ .abort_fails = false, .finish_fails = true },
        .{ .abort_fails = true, .finish_fails = false },
        .{ .abort_fails = true, .finish_fails = true },
    };

    for (cases) |case| {
        var probe: OutputCleanupProbe = .{
            .abort_fails = case.abort_fails,
            .finish_fails = case.finish_fails,
        };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        var lanes_stopped = false;
        defer {
            if (!lanes_stopped) {
                probe.allowCleanupRetry();
                releaseOutputForShutdown(runtime) catch {};
                probe.reader_batch_in_flight.release();
                probe.writer_slice_copied.release();
                probe.reader_stopped.wait();
                waitForWriterStop(runtime);
            }
            clearTestingHooks(runtime);
            runtime.destroy();
        }

        probe.reader_batch_in_flight.wait();
        const client_bytes = "accepted-writer-slice";
        const client_reservation = try reserveInput(
            runtime,
            client_bytes.len,
            false,
            .client,
        );
        try publishInput(
            runtime,
            client_reservation,
            try std.testing.allocator.dupe(u8, client_bytes),
        );
        probe.writer_slice_copied.wait();

        const reply_bytes = "queued-reply";
        const reply_reservation = try reserveInput(
            runtime,
            reply_bytes.len,
            false,
            .terminal,
        );
        try publishInput(
            runtime,
            reply_reservation,
            try std.testing.allocator.dupe(u8, reply_bytes),
        );

        var before = try canonicalSnapshot(runtime);
        defer before.deinit();
        const before_cleanup = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), before_cleanup.read_calls);
        try std.testing.expectEqual(@as(usize, 1), before_cleanup.writer_copies);
        try std.testing.expectEqual(@as(usize, 0), before_cleanup.writer_starts);
        try std.testing.expectEqual(@as(usize, 0), before_cleanup.write_calls);
        try std.testing.expectEqual(@as(u64, 0), before.output_batches);
        try std.testing.expectEqual(@as(usize, 0), before.written_bytes);
        const queue_snapshot = inputQueueSnapshot(runtime);
        try std.testing.expectEqual(@as(usize, 2), queue_snapshot.entry_count);
        try std.testing.expectEqual(client_bytes.len, queue_snapshot.client_charge.bytes);
        try std.testing.expectEqual(@as(usize, 1), queue_snapshot.client_charge.entries);
        try std.testing.expectEqual(
            reply_bytes.len,
            queue_snapshot.terminal_reply_charge.bytes,
        );
        try std.testing.expectEqual(
            @as(usize, 1),
            queue_snapshot.terminal_reply_charge.entries,
        );

        try std.testing.expectError(error.OutputReleaseFailed, releaseOutputForShutdown(runtime));
        const failed_cleanup = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), failed_cleanup.finishes);
        try std.testing.expectEqual(@as(usize, 1), failed_cleanup.aborts);

        probe.reader_batch_in_flight.release();
        probe.writer_slice_copied.release();
        probe.reader_batch_finished.wait();
        probe.reader_stopped.wait();
        waitForWriterStop(runtime);
        lanes_stopped = true;

        try std.testing.expectEqual(
            ReaderBatchOutcome.output_aborted,
            probe.observedReaderOutcome().?,
        );
        const stopped = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), stopped.read_calls);
        try std.testing.expectEqual(@as(usize, 1), stopped.reader_stops);
        try std.testing.expectEqual(@as(usize, 1), stopped.writer_copies);
        try std.testing.expectEqual(@as(usize, 0), stopped.writer_starts);
        try std.testing.expectEqual(@as(usize, 0), stopped.write_calls);
        try std.testing.expectEqual(@as(usize, 0), stopped.write_bytes);
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
        try expectInputQueuesCleared(runtime);
        try std.testing.expectError(
            error.TerminalExited,
            reserveInput(runtime, 1, false, .client),
        );
        try std.testing.expectError(
            error.TerminalExited,
            reserveInput(runtime, 1, false, .terminal),
        );
        try std.testing.expectError(
            error.TerminalExited,
            runtime.prepareRawInput("closed-after-output-stop"),
        );
        var after_failure = try canonicalSnapshot(runtime);
        defer after_failure.deinit();
        try expectCanonicalSnapshotEqual(before, after_failure);
        switch (runtime.completion().classification) {
            .pending => {},
            .normal, .failed => return error.ExpectedOrdinaryOutputStop,
        }
        try std.testing.expect(!subject.testing.failureFenced(runtime));
        try std.testing.expect(runtime.primaryFailure() == null);

        probe.allowCleanupRetry();
        try releaseOutputForShutdown(runtime);

        const retried = probe.counts();
        try std.testing.expectEqual(@as(usize, 2), retried.finishes);
        try std.testing.expectEqual(@as(usize, 2), retried.aborts);
        try std.testing.expectEqual(@as(usize, 1), retried.read_calls);
        try std.testing.expectEqual(@as(usize, 1), retried.reader_stops);
        try std.testing.expectEqual(@as(usize, 1), retried.writer_copies);
        try std.testing.expectEqual(@as(usize, 0), retried.writer_starts);
        try std.testing.expectEqual(@as(usize, 0), retried.write_calls);
        try std.testing.expectEqual(@as(usize, 0), retried.write_bytes);
        var after_retry = try canonicalSnapshot(runtime);
        defer after_retry.deinit();
        try expectCanonicalSnapshotEqual(before, after_retry);
        try expectInputQueuesCleared(runtime);
        try std.testing.expectError(
            error.TerminalExited,
            runtime.prepareRawInput("closed-after-cleanup-retry"),
        );
        try std.testing.expect(!subject.testing.failureFenced(runtime));
        try std.testing.expect(runtime.primaryFailure() == null);
    }
}
test "output abort that wins before drain publication cannot become natural drain" {
    var probe: PtyDrainRaceProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.before_publication.release();
        probe.reader_stopped.wait();
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    // The reader has observed EOF and passed its outer abort checks, but has
    // not crossed the drain-publication boundary yet.
    probe.before_publication.wait();
    try releaseOutputForShutdown(runtime);
    probe.before_publication.release();
    probe.reader_stopped.wait();

    const completion = runtime.completion();
    try std.testing.expect(!completion.pty_drained);
    switch (completion.classification) {
        .pending => {},
        .normal, .failed => return error.ExpectedPendingCompletion,
    }
}
test "post-resize-effect barrier keeps normal completion pending through commit or compensation" {
    const cases = [_]ResizeEffectOutcome{ .commit, .compensated_error };

    for (cases) |outcome| {
        var probe: ResizeProbe = .{ .outcome = outcome };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            clearTestingHooks(runtime);
            runtime.destroy();
        }

        runtimeState(runtime).input_mutex.lock();
        runtimeState(runtime).writer_stop = true;
        runtimeState(runtime).input_condition.broadcast();
        runtimeState(runtime).input_mutex.unlock();
        waitForWriterStop(runtime);
        const before = runtime.geometry();
        const before_configuration = runtime.configuration();

        var resize_call: ResizeCall = .{ .runtime = runtime };
        const resize_thread = try std.Thread.spawn(.{}, ResizeCall.run, .{&resize_call});
        defer resize_thread.join();
        defer probe.post_effect.release();
        probe.post_effect.wait();

        const at_barrier = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), at_barrier.effects);
        try std.testing.expectEqual(@as(usize, 1), at_barrier.post_effects);
        try std.testing.expect(subject.testing.resizeEffectInFlight(runtime));
        try std.testing.expectEqualDeep(before, runtime.geometry());
        switch (completionClassificationLoad(&runtimeState(runtime).completion_classification)) {
            .pending => {},
            .normal, .failed => return error.ExpectedPendingCompletionAtPostEffectBarrier,
        }

        runtimeState(runtime).exit_result = .{ .exit_code = 0 };
        runtimeState(runtime).child_exited.store(true, .release);
        runtimeState(runtime).pty_drained.store(true, .release);
        markNormalCompletionReady(runtime);
        try std.testing.expect(subject.testing.resizeEffectInFlight(runtime));
        switch (completionClassificationLoad(&runtimeState(runtime).completion_classification)) {
            .pending => {},
            .normal, .failed => return error.NormalCompletionOvertookPostResizeEffectBarrier,
        }

        probe.post_effect.release();
        resize_call.done.wait();
        const expected_geometry: subject.Geometry = switch (outcome) {
            .commit => .{
                .cell_pixel_height = 18,
                .cell_pixel_width = 9,
                .columns = 100,
                .generation = before.generation + 1,
                .revision = before.revision + 1,
                .rows = 30,
            },
            .compensated_error => before,
        };
        switch (outcome) {
            .commit => switch (resize_call.observedResult()) {
                .resized => |revision| try std.testing.expectEqual(
                    expected_geometry.revision,
                    revision,
                ),
                .failed => return error.ExpectedCommittedResize,
            },
            .compensated_error => switch (resize_call.observedResult()) {
                .failed => |err| try std.testing.expectEqual(
                    error.InjectedCompensatedResizeFailure,
                    err,
                ),
                .resized => return error.ExpectedCompensatedResizeFailure,
            },
        }

        const settled_completion = runtime.completion();
        switch (settled_completion.classification) {
            .normal => {},
            .pending, .failed => return error.ExpectedNormalCompletion,
        }
        try std.testing.expect(!subject.testing.resizeEffectInFlight(runtime));
        try std.testing.expectEqual(expected_geometry.revision, settled_completion.final_revision);
        try std.testing.expectEqualDeep(expected_geometry, runtime.geometry());
        try std.testing.expectEqualDeep(before_configuration, runtime.configuration());
        const settled_counts = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), settled_counts.effects);
        try std.testing.expectEqual(@as(usize, 1), settled_counts.post_effects);

        try std.testing.expectError(
            error.TerminalRuntimeUnavailable,
            runtime.prepareRawInput("closed-after-normal-settlement"),
        );
        try std.testing.expectError(
            error.TerminalRuntimeUnavailable,
            runtime.updateConfiguration(.{ .scrollback_bytes = 0 }),
        );
        try std.testing.expectError(
            error.TerminalRuntimeUnavailable,
            runtime.resizeGeometry(101, 31, 10, 20),
        );
        try std.testing.expectEqualDeep(expected_geometry, runtime.geometry());
        try std.testing.expectEqualDeep(before_configuration, runtime.configuration());
        try std.testing.expectEqual(
            expected_geometry.revision,
            runtime.completion().final_revision,
        );
        try std.testing.expect(!subject.testing.resizeEffectInFlight(runtime));
    }
}
test "force termination releases every engine-owned client input reservation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const input = try std.testing.allocator.alloc(u8, 512 * 1024);
    defer std.testing.allocator.free(input);
    @memset(input, 'x');
    try runtime.enqueueInput(input);
    try runtime.forceTerminate();
    try runtime.shutdown();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
}
test "stopped writer rejects reservations even when queue capacity remains" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    runtimeState(runtime).input_mutex.lock();
    runtimeState(runtime).writer_stop = true;
    runtimeState(runtime).input_condition.broadcast();
    runtimeState(runtime).input_mutex.unlock();

    try std.testing.expectError(
        error.TerminalExited,
        reserveInput(runtime, 1, false, .client),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "resize counter exhaustion leaves process and terminal geometry unchanged" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).generation = std.math.maxInt(u64);
    runtimeState(runtime).terminal_mutex.unlock();
    const generation_exhausted = runtime.geometry();
    try std.testing.expectError(
        error.RevisionOverflow,
        runtime.resizeGeometry(81, 25, 2, 3),
    );
    try std.testing.expectEqualDeep(generation_exhausted, runtime.geometry());

    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).generation = 1;
    runtimeState(runtime).revision.store(std.math.maxInt(u64), .monotonic);
    runtimeState(runtime).terminal_mutex.unlock();
    const revision_exhausted = runtime.geometry();
    try std.testing.expectError(
        error.RevisionOverflow,
        runtime.resizeGeometry(81, 25, 2, 3),
    );
    try std.testing.expectEqualDeep(revision_exhausted, runtime.geometry());
    try runtime.forceTerminate();
}
test "runtime can force the retained session after the root child exits" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    const arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "start \"\" /B ping -n 31 127.0.0.1 >nul & ping -n 2 127.0.0.1 >nul",
    };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const finishing = try semantic_wait.childExit(runtime);
    try std.testing.expect(!finishing.pty_drained);

    try runtime.terminate();
    try runtime.forceTerminate();
    try runtime.shutdown();
    const completed = runtime.completion();
    try std.testing.expect(completed.child_exited);
}
