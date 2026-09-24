const harness = @import("runtime_harness.zig");
const std = harness.std;
const subject = harness.subject;
const cancelInput = harness.cancelInput;
const commitReaderBatch = harness.commitReaderBatch;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const max_pending_terminal_reply_bytes = harness.max_pending_terminal_reply_bytes;
const reserveInput = harness.reserveInput;
const runtimeState = harness.runtimeState;
const Geometry = harness.Geometry;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const TerminalDrivenResizeProbe = harness.TerminalDrivenResizeProbe;
const TerminalRuntime = harness.TerminalRuntime;
const WriterProbe = harness.WriterProbe;

const GatedPtyProbe = harness.GatedPtyProbe;

fn viewportGeometry(runtime: *TerminalRuntime) Geometry {
    const runtime_state = runtimeState(runtime);
    runtime_state.terminal_mutex.lock();
    defer runtime_state.terminal_mutex.unlock();
    return runtime_state.viewport_geometry;
}

fn expectGeometry(expected: Geometry, actual: anytype) !void {
    try std.testing.expectEqual(expected.columns, actual.columns);
    try std.testing.expectEqual(expected.rows, actual.rows);
    try std.testing.expectEqual(expected.cell_pixel_width, actual.cell_pixel_width);
    try std.testing.expectEqual(expected.cell_pixel_height, actual.cell_pixel_height);
}

test "mode 40 restores the resize-owner viewport across every mode mutation path" {
    var probe: GatedPtyProbe(TerminalDrivenResizeProbe) = .{ .probe = .{} };
    const resize_probe = &probe.probe;
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        runtime.destroy();
    }
    probe.reader.wait();

    const launch = runtime.geometry();
    try expectGeometry(.{
        .cell_pixel_height = launch.cell_pixel_height,
        .cell_pixel_width = launch.cell_pixel_width,
        .columns = launch.columns,
        .rows = launch.rows,
    }, viewportGeometry(runtime));

    _ = try runtime.resizeGeometry(100, 30, 9, 18);
    const viewport: Geometry = .{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 100,
        .rows = 30,
    };
    try expectGeometry(viewport, runtime.geometry());
    try expectGeometry(viewport, viewportGeometry(runtime));

    const before_enable = runtime.geometry();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h"),
    );
    const after_enable = runtime.geometry();
    try expectGeometry(viewport, after_enable);
    try std.testing.expectEqual(before_enable.generation, after_enable.generation);
    try std.testing.expectEqual(before_enable.revision + 1, after_enable.revision);
    try std.testing.expectEqual(@as(usize, 1), resize_probe.observedCalls().len);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?3h"),
    );
    const reset_expanded = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), reset_expanded.columns);
    try std.testing.expectEqual(viewport.rows, reset_expanded.rows);
    try std.testing.expectEqual(viewport.cell_pixel_width, reset_expanded.cell_pixel_width);
    try std.testing.expectEqual(viewport.cell_pixel_height, reset_expanded.cell_pixel_height);
    try expectGeometry(viewport, viewportGeometry(runtime));

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40l"),
    );
    try expectGeometry(viewport, runtime.geometry());
    try expectGeometry(viewport, viewportGeometry(runtime));

    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 3), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 100), calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 100), calls[1].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[1].next.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[2].previous.columns);
    try expectGeometry(viewport, calls[2].next);

    // Re-setting an already-enabled mode and restoring a saved enabled mode
    // both apply the viewport target after a DECCOLM geometry change.
    resize_probe.clearObservedCalls();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    try std.testing.expectEqual(@as(u16, 132), runtime.geometry().columns);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h"),
    );
    try expectGeometry(viewport, runtime.geometry());
    try std.testing.expectEqual(@as(usize, 2), resize_probe.observedCalls().len);

    resize_probe.clearObservedCalls();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40s\x1b[?3h"),
    );
    try std.testing.expectEqual(@as(u16, 132), runtime.geometry().columns);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40r"),
    );
    try expectGeometry(viewport, runtime.geometry());
    try std.testing.expectEqual(@as(usize, 2), resize_probe.observedCalls().len);

    // A single reader batch preserves DECCOLM-before-restore effect ordering
    // while publishing only one canonical revision and generation.
    resize_probe.clearObservedCalls();
    const before_batch = runtime.geometry();

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h\x1b[?40l"),
    );
    const after_batch = runtime.geometry();
    try expectGeometry(viewport, after_batch);
    try std.testing.expectEqual(before_batch.revision + 1, after_batch.revision);
    try std.testing.expectEqual(before_batch.generation + 1, after_batch.generation);
    const batch_calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), batch_calls.len);
    try std.testing.expectEqual(@as(u16, 132), batch_calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 100), batch_calls[1].next.columns);

    // A successful owner resize to the current canonical geometry refreshes
    // the viewport without a platform effect or a geometry revision.
    _ = try runtime.resizeGeometry(
        launch.columns,
        launch.rows,
        launch.cell_pixel_width,
        launch.cell_pixel_height,
    );
    resize_probe.clearObservedCalls();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    const expanded = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), expanded.columns);
    try std.testing.expectEqual(@as(u16, 80), viewportGeometry(runtime).columns);

    const unchanged_revision = try runtime.resizeGeometry(
        expanded.columns,
        expanded.rows,
        expanded.cell_pixel_width,
        expanded.cell_pixel_height,
    );
    try std.testing.expectEqual(expanded.revision, unchanged_revision);
    const replacement = viewportGeometry(runtime);
    try std.testing.expectEqual(@as(u16, 132), replacement.columns);
    try std.testing.expectEqual(@as(usize, 1), resize_probe.observedCalls().len);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40l"),
    );
    const restored = runtime.geometry();
    try expectGeometry(replacement, restored);
    try std.testing.expectEqual(expanded.generation, restored.generation);
    try std.testing.expectEqual(expanded.revision + 1, restored.revision);
    try std.testing.expectEqual(@as(usize, 1), resize_probe.observedCalls().len);
    try std.testing.expectEqual(launch.rows, restored.rows);

    try runtime.forceTerminate();
}

test "mode 40 PTY restore failure fences before Ghostty geometry mutation" {
    var probe: GatedPtyProbe(TerminalDrivenResizeProbe) = .{ .probe = .{} };
    const resize_probe = &probe.probe;
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        runtime.destroy();
    }
    probe.reader.wait();
    _ = try runtime.resizeGeometry(100, 30, 9, 18);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    const expanded = runtime.geometry();
    resize_probe.fail = true;

    try std.testing.expectEqual(
        ReaderBatchOutcome.runtime_failed,
        commitReaderBatch(runtime, "\x1b[?40ldiscarded-after-failure"),
    );
    try expectGeometry(.{
        .cell_pixel_height = expanded.cell_pixel_height,
        .cell_pixel_width = expanded.cell_pixel_width,
        .columns = expanded.columns,
        .rows = expanded.rows,
    }, runtime.geometry());
    try std.testing.expect(runtime.isResizeFenced());
    try std.testing.expectEqual(subject.RuntimeFailureKind.resize, runtime.primaryFailure().?);
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 3), calls.len);
    try std.testing.expectEqual(@as(u16, 132), calls[2].previous.columns);
    try std.testing.expectEqual(@as(u16, 100), calls[2].next.columns);
}

test "mode 2048 reports exact mode 40 geometry and fences a dropped restore report" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    // Native startup mode effects must not consume the reply quota before
    // the deliberately dropped restore report exercises the failure fence.
    probe.reader.wait();
    var expected: [128]u8 = undefined;

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2048h"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    ));
    _ = try runtime.resizeGeometry(100, 30, 9, 18);
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    ));

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    ));
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40l"),
    );
    const restored = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 100), restored.columns);
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, restored),
    ));

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    ));
    _ = try reserveInput(runtime, max_pending_terminal_reply_bytes, false, .terminal);
    try std.testing.expectEqual(
        ReaderBatchOutcome.runtime_failed,
        commitReaderBatch(runtime, "\x1b[?40l"),
    );
    try std.testing.expectEqual(subject.RuntimeFailureKind.terminal_reply, runtime.primaryFailure().?);
    try std.testing.expectEqual(@as(u16, 100), runtime.geometry().columns);
}

test "pixel size queries have no reply while cell queries still answer under unknown pixels" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    const geometry = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 0), geometry.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 0), geometry.cell_pixel_height);

    // CSI 14t and 16t carry only pixel facts; with no measured pair the host
    // emits no reply rather than reporting zero as if it were a measurement.
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[14t"),
    );
    try expectNoQueuedTerminalBytes(runtime);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[16t"),
    );
    try expectNoQueuedTerminalBytes(runtime);

    // CSI 18t stays authoritative for the real cell grid.
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[18t"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[8;24;80t"));

    // Mode 2048 reports the real cells with honest zero pixel extents.
    var expected: [128]u8 = undefined;
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2048h"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    ));
}

fn expectNoQueuedTerminalBytes(runtime: *TerminalRuntime) !void {
    const runtime_state = runtimeState(runtime);
    runtime_state.input_mutex.lock();
    defer runtime_state.input_mutex.unlock();
    try std.testing.expect(!runtime_state.input_queue.hasWritableEntry());
}

fn expectNextQueuedTerminalBytes(runtime: *TerminalRuntime, expected: []const u8) !u64 {
    var buffer: [256]u8 = undefined;
    const runtime_state = runtimeState(runtime);
    runtime_state.input_mutex.lock();
    defer runtime_state.input_mutex.unlock();
    if (!runtime_state.input_queue.hasWritableEntry()) return error.ExpectedQueuedInput;
    const slice = runtime_state.input_queue.copyNextWriterSlice(&buffer);
    try std.testing.expectEqual(harness.InputSource.terminal, slice.source);
    try std.testing.expectEqualStrings(expected, buffer[0..slice.length]);
    return slice.identifier;
}

fn inBandSizeReport(buffer: []u8, geometry: anytype) ![]const u8 {
    return std.fmt.bufPrint(buffer, "\x1b[48;{d};{d};{d};{d}t", .{
        geometry.rows,
        geometry.columns,
        @as(u64, geometry.rows) * geometry.cell_pixel_height,
        @as(u64, geometry.columns) * geometry.cell_pixel_width,
    });
}
