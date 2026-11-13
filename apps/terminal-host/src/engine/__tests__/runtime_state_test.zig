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
const scrollbackCompressionIdleIntervalNs = harness.scrollbackCompressionIdleIntervalNs;
const scrollbackMemoryStats = harness.scrollbackMemoryStats;
const tickAnimationLockedAt = harness.tickAnimationLockedAt;
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
const max_pending_application_signals = harness.max_pending_application_signals;
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
const destroyRuntimeWithIdentityRetry = harness.destroyRuntimeWithIdentityRetry;
const createImmediateExitRuntime = harness.createImmediateExitRuntime;
const createIdleRuntimeWithGeometry = harness.createIdleRuntimeWithGeometry;
const createIdleRuntimeWithGeometryAndHooks = harness.createIdleRuntimeWithGeometryAndHooks;
const parseTerminalInput = harness.parseTerminalInput;
const WriterPause = harness.WriterPause;

fn maintainScrollbackCompressionWhenAvailable(
    runtime: *TerminalRuntime,
    now_ns: i128,
) subject.ScrollbackCompressionOutcome {
    for (0..1_024) |_| {
        const outcome = runtime.maintainScrollbackCompression(now_ns);
        if (outcome != .lock_busy) return outcome;
        std.Thread.yield() catch {};
    }
    return .lock_busy;
}

const CommandClockProbe = struct {
    monotonic_values: []const u64,
    next_monotonic: usize = 0,
    wall_milliseconds: u64,

    fn hooks(self: *CommandClockProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .monotonic_time_nanoseconds = monotonicTimeNanoseconds,
            .wall_time_milliseconds = wallTimeMilliseconds,
        };
    }

    fn monotonicTimeNanoseconds(context: *anyopaque) u64 {
        const self: *CommandClockProbe = @ptrCast(@alignCast(context));
        const value = self.monotonic_values[self.next_monotonic];
        self.next_monotonic += 1;
        return value;
    }

    fn wallTimeMilliseconds(context: *anyopaque) u64 {
        const self: *CommandClockProbe = @ptrCast(@alignCast(context));
        return self.wall_milliseconds;
    }
};

const PtyIoGate = struct {
    reader: TestGate = .{},
    runtime: *TerminalRuntime = undefined,

    fn read(context: *anyopaque, buffer: []u8) anyerror!usize {
        const self: *PtyIoGate = @ptrCast(@alignCast(context));
        self.reader.pause();
        return runtimeState(self.runtime).process.read(buffer);
    }

    fn write(_: *anyopaque, bytes: []const u8) anyerror!usize {
        return bytes.len;
    }

    fn hooks(self: *PtyIoGate) RuntimeTestingHooks {
        return .{ .context = self, .read_pty = read, .write_pty = write };
    }
};

const AnimationWakeProbe = struct {
    count: std.atomic.Value(usize) = .init(0),

    fn hooks(self: *AnimationWakeProbe) RuntimeTestingHooks {
        return .{
            .animation_wake_requested = animationWakeRequested,
            .context = self,
        };
    }

    fn animationWakeRequested(context: *anyopaque) void {
        const self: *AnimationWakeProbe = @ptrCast(@alignCast(context));
        _ = self.count.fetchAdd(1, .monotonic);
    }
};

const WriterProbe = harness.WriterProbe;
const RepeatingGate = harness.RepeatingGate;
const BurstProbe = harness.BurstProbe;
const DescendantProbe = harness.DescendantProbe;
const ReaderProbe = harness.ReaderProbe;
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

fn expectAttachmentPreparationUnavailable(
    runtime: *TerminalRuntime,
    attachment_id: AttachmentId,
    capabilities: subject.Capabilities,
) !void {
    var preparation = runtime.prepareAttachment(attachment_id, capabilities) catch |err| {
        try std.testing.expectEqual(error.TerminalRuntimeUnavailable, err);
        return;
    };
    defer preparation.deinit();
    return error.ExpectedAttachmentPreparationFailure;
}

test "platform resize errors restore committed geometry or report an unsafe rollback" {
    const FakePty = struct {
        call_count: usize = 0,
        calls: [2]Geometry = undefined,
        rollback_fails: bool,

        pub fn resizePixels(
            self: *@This(),
            columns: u16,
            rows: u16,
            cell_pixel_width: u16,
            cell_pixel_height: u16,
        ) !void {
            self.calls[self.call_count] = .{
                .cell_pixel_height = cell_pixel_height,
                .cell_pixel_width = cell_pixel_width,
                .columns = columns,
                .rows = rows,
            };
            self.call_count += 1;
            if (self.call_count == 1) return error.PlatformResizeFailed;
            if (self.rollback_fails) return error.PlatformRollbackFailed;
        }
    };
    const previous: Geometry = .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    };
    const next: Geometry = .{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 120,
        .rows = 40,
    };

    var recovered: FakePty = .{ .rollback_fails = false };
    try std.testing.expectError(
        error.PlatformResizeFailed,
        resizePtyWithRollback(&recovered, previous, next),
    );
    try std.testing.expectEqual(@as(usize, 2), recovered.call_count);
    try std.testing.expectEqualDeep(next, recovered.calls[0]);
    try std.testing.expectEqualDeep(previous, recovered.calls[1]);

    var unsafe: FakePty = .{ .rollback_fails = true };
    try std.testing.expectError(
        error.ResizeRollbackFailed,
        resizePtyWithRollback(&unsafe, previous, next),
    );
    try std.testing.expectEqual(@as(usize, 2), unsafe.call_count);
    try std.testing.expectEqualDeep(next, unsafe.calls[0]);
    try std.testing.expectEqualDeep(previous, unsafe.calls[1]);
}
test "semantic modifiers map every named field into Ghostty" {
    const mapped = modifiers(.{
        .shift = true,
        .control = true,
        .alt = true,
        .meta = true,
        .caps_lock = true,
        .num_lock = true,
        .shift_right = true,
        .control_right = true,
        .alt_right = true,
        .meta_right = true,
    });

    try std.testing.expect(mapped.shift);
    try std.testing.expect(mapped.ctrl);
    try std.testing.expect(mapped.alt);
    try std.testing.expect(mapped.super);
    try std.testing.expect(mapped.caps_lock);
    try std.testing.expect(mapped.num_lock);
    try std.testing.expectEqual(ghostty_vt.input.KeyMods.Side{
        .shift = .right,
        .ctrl = .right,
        .alt = .right,
        .super = .right,
    }, mapped.sides);
}
test "mouse drag preserves its active encoder button" {
    const input: subject.MouseInput = .{
        .action = .{ .motion = .{ .drag = .left } },
        .modifiers = .{},
        .pressed_buttons = .{ .left = true },
        .x = 10,
        .y = 20,
    };

    const encoded = mouseEncodeEvent(input);
    try std.testing.expectEqual(ghostty_vt.input.MouseAction.motion, encoded.action);
    try std.testing.expectEqual(ghostty_vt.input.MouseButton.left, encoded.button.?);
    try std.testing.expect(mouseHasPressedButton(input));
}
test "mouse hover encodes without an active button" {
    const input: subject.MouseInput = .{
        .action = .{ .motion = .hover },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 10,
        .y = 20,
    };

    const encoded = mouseEncodeEvent(input);
    try std.testing.expectEqual(ghostty_vt.input.MouseAction.motion, encoded.action);
    try std.testing.expectEqual(@as(?ghostty_vt.input.MouseButton, null), encoded.button);
    try std.testing.expect(!mouseHasPressedButton(input));
}
test "mouse press counts as pressed for encoder options before persistent state changes" {
    const input: subject.MouseInput = .{
        .action = .{ .press = .four },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 10,
        .y = 20,
    };

    try std.testing.expect(mouseHasPressedButton(input));
}
test "dynamic scrollback configuration evicts history and can retain history again" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .max_scrollback_bytes = 1_000_000,
        .rows = 3,
    });
    defer terminal.deinit(std.testing.allocator);
    for (0..100) |_| try terminal.printString("line\n");
    try std.testing.expect(terminal.screens.get(.primary).?.pages.total_rows > terminal.rows);

    const evicted = applyScrollbackBytes(&terminal, 0);
    try std.testing.expect(evicted > 0);
    try std.testing.expectEqual(
        @as(usize, terminal.rows),
        terminal.screens.get(.primary).?.pages.total_rows,
    );

    _ = applyScrollbackBytes(&terminal, 1_000_000);
    for (0..100) |_| try terminal.printString("line\n");
    try std.testing.expect(terminal.screens.get(.primary).?.pages.total_rows > terminal.rows);
}
test "idle scrollback compression is nonblocking bounded and representation-only" {
    // The input exercises production admission but must not introduce an
    // asynchronous PTY echo into the representation-only revision assertions.
    // Hold native startup output until those assertions complete as well.
    var pty_io: PtyIoGate = .{};
    const runtime = try createIdleRuntimeWithGeometryAndHooks(
        80,
        24,
        .{ .scrollback_bytes = 8 * 1024 * 1024 },
        pty_io.hooks(),
    );
    pty_io.runtime = runtime;
    defer destroyRuntimeWithIdentityRetry(runtime);
    defer pty_io.reader.release();
    pty_io.reader.wait();
    const runtime_state = runtimeState(runtime);

    var batch: [64 * 1024]u8 = undefined;
    for (&batch, 0..) |*byte, index| {
        byte.* = if (index % 80 == 79) '\n' else @as(u8, 'a') + @as(u8, @intCast(index % 4));
    }
    // This still spans multiple resident pages while keeping the bounded
    // compression test below the idle child process's test-only lifetime.
    for (0..12) |_| {
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, &batch),
        );
    }

    const revision_before = runtime_state.revision.load(.acquire);
    const generation_before = runtime_state.generation;
    const memory_before = scrollbackMemoryStats(runtime);
    try std.testing.expect(memory_before.resident_pages > 1);

    runtime_state.terminal_mutex.lock();
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.lock_busy,
        runtime.maintainScrollbackCompression(0),
    );
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, 0),
    );
    const original_deadline = runtime_state.scrollback_compression.eligible_at_ns;
    try std.testing.expectEqual(scrollbackCompressionIdleIntervalNs, original_deadline);

    const client_input_observed_at = 100 * std.time.ns_per_ms;
    try runtime.enqueueInput("i");
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, client_input_observed_at),
    );
    const client_input_deadline = client_input_observed_at +
        scrollbackCompressionIdleIntervalNs;
    try std.testing.expectEqual(
        client_input_deadline,
        runtime_state.scrollback_compression.eligible_at_ns,
    );
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, client_input_deadline - 1),
    );

    runtime_state.terminal_mutex.lock();
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.lock_busy,
        runtime.maintainScrollbackCompression(client_input_deadline),
    );
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        client_input_deadline,
        runtime_state.scrollback_compression.eligible_at_ns,
    );

    var now_ns = client_input_deadline;
    var previous_compressed_pages = memory_before.compressed_pages;
    var completed = false;
    for (0..4_096) |_| {
        const outcome = maintainScrollbackCompressionWhenAvailable(runtime, now_ns);
        const memory = scrollbackMemoryStats(runtime);
        try std.testing.expect(memory.compressed_pages >= previous_compressed_pages);
        try std.testing.expect(memory.compressed_pages - previous_compressed_pages <= 1);
        previous_compressed_pages = memory.compressed_pages;
        switch (outcome) {
            .complete => {
                completed = true;
                break;
            },
            .pending, .idle_wait => {},
            .dormant, .lock_busy, .unsupported => return error.UnexpectedCompressionOutcome,
        }
        now_ns += 20 * std.time.ns_per_ms;
    }
    try std.testing.expect(completed);

    const memory_after = scrollbackMemoryStats(runtime);
    try std.testing.expect(memory_after.compressed_pages > memory_before.compressed_pages);
    try std.testing.expect(memory_after.estimated_savings > 0);
    try std.testing.expect(memory_after.estimated_resident_bytes < memory_before.estimated_resident_bytes);
    try std.testing.expectEqual(revision_before, runtime_state.revision.load(.acquire));
    try std.testing.expectEqual(generation_before, runtime_state.generation);
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.dormant,
        maintainScrollbackCompressionWhenAvailable(runtime, now_ns),
    );

    runtime_state.terminal_mutex.lock();
    const compression_activity_before_dormant_output =
        runtime_state.terminal.compressionActivity();
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x07"),
    );
    runtime_state.terminal_mutex.lock();
    const compression_activity_after_dormant_output =
        runtime_state.terminal.compressionActivity();
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        compression_activity_before_dormant_output,
        compression_activity_after_dormant_output,
    );
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.dormant,
        maintainScrollbackCompressionWhenAvailable(runtime, now_ns),
    );

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "activity\n"),
    );
    const changed_revision = runtime_state.revision.load(.acquire);
    try std.testing.expect(changed_revision > revision_before);
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, now_ns),
    );

    runtime_state.terminal_mutex.lock();
    const compression_activity_before_same_line_output =
        runtime_state.terminal.compressionActivity();
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "x"),
    );
    runtime_state.terminal_mutex.lock();
    const compression_activity_after_same_line_output =
        runtime_state.terminal.compressionActivity();
    runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(
        compression_activity_before_same_line_output,
        compression_activity_after_same_line_output,
    );

    const original_activity_deadline = now_ns + scrollbackCompressionIdleIntervalNs;
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, original_activity_deadline),
    );
    const same_line_output_deadline = original_activity_deadline +
        scrollbackCompressionIdleIntervalNs;
    try std.testing.expectEqual(
        same_line_output_deadline,
        runtime_state.scrollback_compression.eligible_at_ns,
    );
    try std.testing.expectEqual(
        subject.ScrollbackCompressionOutcome.idle_wait,
        maintainScrollbackCompressionWhenAvailable(runtime, same_line_output_deadline - 1),
    );
    try std.testing.expectEqual(changed_revision + 1, runtime_state.revision.load(.acquire));
    try std.testing.expectEqual(generation_before, runtime_state.generation);
}
test "visible Kitty animation ticks monotonically advance Host revision and history" {
    const runtime = try createIdleRuntime(.{});
    defer destroyRuntimeWithIdentityRetry(runtime);
    const runtime_state = runtimeState(runtime);

    // Park the autonomous animation worker so this test owns the exact clock.
    runtime_state.terminal_mutex.lock();
    runtime_state.command_timer = null;
    runtime_state.terminal_mutex.unlock();

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b_Ga=T,t=d,f=24,s=1,v=1,i=1,p=1,C=1;/wAA\x1b\\" ++
                "\x1b_Ga=f,i=1,f=24,s=1,v=1;AAD/\x1b\\" ++
                "\x1b_Ga=a,i=1,s=3\x1b\\",
        ),
    );
    const baseline_revision = runtime_state.revision.load(.acquire);
    const baseline_batches = runtime_state.output_batches_since_sync.load(.acquire);
    try std.testing.expectEqual(@as(u64, 0), runtime_state.animation_tick);
    try std.testing.expectEqual(@as(u64, 0), runtime_state.history.animation_tick);

    runtime_state.terminal_mutex.lock();
    const first = try tickAnimationLockedAt(runtime_state, 0);
    runtime_state.terminal_mutex.unlock();
    try std.testing.expect(first.changed);
    try std.testing.expectEqual(@as(?u64, 40), first.next_delay_milliseconds);
    try std.testing.expectEqual(baseline_revision + 1, runtime_state.revision.load(.acquire));
    try std.testing.expectEqual(baseline_batches + 1, runtime_state.output_batches_since_sync.load(.acquire));
    try std.testing.expectEqual(@as(u64, 1), runtime_state.animation_tick);
    try std.testing.expectEqual(@as(u64, 1), runtime_state.history.animation_tick);

    runtime_state.terminal_mutex.lock();
    const early = try tickAnimationLockedAt(runtime_state, 10);
    runtime_state.terminal_mutex.unlock();
    try std.testing.expect(!early.changed);
    try std.testing.expectEqual(@as(?u64, 30), early.next_delay_milliseconds);
    try std.testing.expectEqual(baseline_revision + 1, runtime_state.revision.load(.acquire));
    try std.testing.expectEqual(baseline_batches + 1, runtime_state.output_batches_since_sync.load(.acquire));
    try std.testing.expectEqual(@as(u64, 1), runtime_state.animation_tick);
    try std.testing.expectEqual(@as(u64, 1), runtime_state.history.animation_tick);
}
test "only Kitty graphics mutations request an animation scheduler wake" {
    var probe: AnimationWakeProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer destroyRuntimeWithIdentityRetry(runtime);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "plain text"),
    );
    try std.testing.expectEqual(@as(usize, 0), probe.count.load(.monotonic));

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b_Ga=T,t=d,f=24,s=1,v=1,i=1,p=1,C=1;/wAA\x1b\\" ++
                "\x1b_Ga=f,i=1,f=24,s=1,v=1;AAD/\x1b\\" ++
                "\x1b_Ga=a,i=1,s=3\x1b\\",
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), probe.count.load(.monotonic));
}
test "reader preserves direct Kitty graphics emitted through a real PTY" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &.{},
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer runtime.destroy();

    try runtime.enqueueInput(
        "printf '\\033_Ga=T,t=d,f=24,i=71,p=1,s=1,v=1,c=2,r=1,q=2;/wAA\\033\\\\'\n" ++
            "exit\n",
    );
    _ = try runtime.waitForCompletion();

    const frame = try runtime.encodeProjectionFrame(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{0x31} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0x42} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(@as(u16, 1), summary.graphics.?.image_count);
    try std.testing.expectEqual(@as(u16, 1), summary.graphics.?.placement_count);

    try runtime.shutdown();

    const runtime_state = runtimeState(runtime);
    runtime_state.terminal_mutex.lock();
    defer runtime_state.terminal_mutex.unlock();
    const graphics = &runtime_state.terminal.screens.active.kitty_images;
    try std.testing.expectEqual(@as(usize, 1), graphics.images.count());
    try std.testing.expectEqual(@as(usize, 1), graphics.placements.count());
}
test "projection frames carry paired-zero pixel geometry and still validate" {
    const runtime = try createIdleRuntime(.{});
    defer destroyRuntimeWithIdentityRetry(runtime);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "cell-only output"),
    );
    const frame = try runtime.encodeProjectionFrame(.{
        .attachment_id = AttachmentId.fromBytes([_]u8{0x32} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0x43} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.allocation);
    // The wire validator accepts the paired-zero header as the unknown-pixel
    // representation while the real cell grid stays authoritative.
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(@as(u16, 0), summary.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 0), summary.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 80), summary.columns);
    try std.testing.expectEqual(@as(u16, 24), summary.rows);
}
test "owned terminal profile releases every partial allocation" {
    try std.testing.checkAllAllocationFailures(
        std.testing.allocator,
        exerciseOwnedProfileAllocations,
        .{},
    );
}
test "runtime rejects an incomplete palette before starting a process" {
    try std.testing.expectError(
        error.InvalidTerminalProfile,
        TerminalRuntime.createWithOptions(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = "not-started",
            .rows = 24,
        }, .{ .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &.{22},
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &.{},
            .version_response = "Orchbee",
        } }),
    );
}
test "thread startup failure releases transferred runtime resources" {
    for (0..4) |failure_stage| {
        var spawner: FailingThreadSpawner = .{ .fail_at = failure_stage };
        try std.testing.expectError(
            error.InjectedThreadSpawnFailure,
            createWithOptionsUsingSpawner(std.testing.allocator, .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                // The injected thread error must win because no process launch
                // is attempted until all four workers exist.
                .executable = "thread-startup-must-precede-process-spawn",
                .rows = 24,
            }, .{}, &spawner),
        );
    }
}
test "process spawn failure releases workers waiting at the startup gate" {
    var spawner: FailingThreadSpawner = .{ .fail_at = 4 };
    try std.testing.expectError(
        error.SpawnFailed,
        createWithOptionsUsingSpawner(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = "runtime-process-spawn-must-fail",
            .rows = 24,
        }, .{}, &spawner),
    );
    try std.testing.expectEqual(@as(usize, 4), spawner.attempts);
}
test "unrecoverable worker failures contain the process tree without a connected broker" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const FailureCase = struct {
        expected_error: anyerror,
        expected_kind: subject.RuntimeFailureKind,
        inject: *const fn (*TerminalRuntime) void,
    };
    const cases = [_]FailureCase{
        .{
            .expected_error = error.PtyReadFailed,
            .expected_kind = .pty_read,
            .inject = recordReaderFailure,
        },
        .{
            .expected_error = error.PtyWriteFailed,
            .expected_kind = .pty_write,
            .inject = recordWriterFailure,
        },
        .{
            .expected_error = error.TerminalReplyDropped,
            .expected_kind = .terminal_reply,
            .inject = recordTerminalReplyFailure,
        },
        .{
            .expected_error = error.WorkerIntegrityFailed,
            .expected_kind = .worker_integrity,
            .inject = recordWorkerIntegrityFailure,
        },
    };

    for (cases) |case| {
        const runtime = try TerminalRuntime.create(std.testing.allocator, .{
            .arguments = arguments,
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = 24,
        });
        defer runtime.destroy();

        case.inject(runtime);

        try std.testing.expect(runtimeState(runtime).containment_confirmed.load(.acquire));
        try std.testing.expectEqual(case.expected_kind, runtime.primaryFailure().?);
        try std.testing.expectError(case.expected_error, runtime.waitForCompletion());
        try runtime.shutdown();
    }
}
test "runtime parses PTY output before exposing canonical terminal state" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf '\\033[31mhost\\033[0m:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =host:!value!& exit /B 0",
    };
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

    const client_input = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    try runtime.enqueueInput(client_input);
    const completion = try runtime.waitForCompletion();
    try runtime.shutdown();
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expect(completion.final_revision > 0);
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);
    try std.testing.expectEqual(client_input.len, runtime.takeWrittenBytes());

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "host:orchbee") != null);
}
test "runtime projects explicit product cursor defaults without a profile override" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try expectProjectedCursor(runtime, .block, false, 1);
}
test "cursor escapes override and resets restore the terminal profile defaults" {
    const palette = [_]u32{0} ** 256;
    const runtime = try createIdleRuntime(.{ .profile = .{
        .color_scheme = .dark,
        .default_background_rgb = 0,
        .default_cursor_blinking = true,
        .default_cursor_rgb = null,
        .default_cursor_shape = .underline,
        .default_foreground_rgb = 0xffffff,
        .device_primary_conformance_level = 62,
        .device_primary_features = &.{22},
        .device_secondary_type = 1,
        .device_secondary_firmware_version = 0,
        .device_secondary_rom_cartridge = 0,
        .device_tertiary_unit_id = 0,
        .enquiry_response = "",
        .palette = &palette,
        .version_response = "Orchbee",
    } });
    defer runtime.destroy();

    try expectProjectedCursor(runtime, .underline, true, 1);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[6 q"),
    );
    try expectProjectedCursor(runtime, .bar, false, 2);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1bc"),
    );
    try expectProjectedCursor(runtime, .underline, true, 3);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[2 q"),
    );
    try expectProjectedCursor(runtime, .block, false, 4);
    _ = try runtime.applyTerminalControl(.reset);
    try expectProjectedCursor(runtime, .underline, true, 5);
}
test "Ghostty BEL effects advance one terminal bell sequence per control byte" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectEqual(@as(u64, 0), runtime.bellSequence());
    parseTerminalInput(runtime, "\x07\x07");
    try std.testing.expectEqual(@as(u64, 2), runtime.bellSequence());
    parseTerminalInput(runtime, "text\x07");
    try std.testing.expectEqual(@as(u64, 3), runtime.bellSequence());
    try runtime.forceTerminate();
}

test "private APC queues the reserved bounded application signal channel in order" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]777;notify;orchbee.agent.v1;ignored\x1b\\" ++
            "\x1b]9;also ignored\x1b\\" ++
            "\x1b_unrelated.private;ignored\x1b\\" ++
            "\x1b_orchbee.agent.v1;\x1b\\" ++
            "\x1b_orchbee.agent.v1;***\x1b\\" ++
            "\x1b_orchbee.agent.v1;AA\x1b\\" ++
            "\x1b_orchbee.agent.v1;_w\x1b\\" ++
            "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\" ++
            "\x1b_orchbee.agent.v1;eyJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6MX0\x1b\\",
    ));

    var first = (try runtime.prepareApplicationSignal(std.testing.allocator)).?;
    defer first.deinit();
    try std.testing.expectEqual(@as(u64, 1), first.sequence);
    try std.testing.expectEqualStrings(
        "{\"kind\":\"correlation\",\"version\":1}",
        first.payload,
    );
    runtime.commitApplicationSignal(first);

    var second = (try runtime.prepareApplicationSignal(std.testing.allocator)).?;
    defer second.deinit();
    try std.testing.expectEqual(@as(u64, 2), second.sequence);
    try std.testing.expectEqualStrings(
        "{\"kind\":\"launch\",\"version\":1}",
        second.payload,
    );
    runtime.commitApplicationSignal(second);
    try std.testing.expect((try runtime.prepareApplicationSignal(std.testing.allocator)) == null);

    try runtime.forceTerminate();
}

test "private APC application signal queue preserves admitted predecessors at its bound" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    const input_signal =
        "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\";
    var input: std.ArrayList(u8) = .empty;
    defer input.deinit(std.testing.allocator);
    for (0..max_pending_application_signals + 1) |_| {
        try input.appendSlice(std.testing.allocator, input_signal);
    }
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, input.items),
    );

    for (1..max_pending_application_signals + 1) |expected_sequence| {
        var signal = (try runtime.prepareApplicationSignal(std.testing.allocator)).?;
        defer signal.deinit();
        try std.testing.expectEqual(@as(u64, expected_sequence), signal.sequence);
        runtime.commitApplicationSignal(signal);
    }
    try std.testing.expect((try runtime.prepareApplicationSignal(std.testing.allocator)) == null);

    try runtime.forceTerminate();
}

test "truncated private APC is ignored without consuming the next valid signal" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    var input: std.ArrayList(u8) = .empty;
    defer input.deinit(std.testing.allocator);
    try input.appendSlice(std.testing.allocator, "\x1b_orchbee.agent.v1;");
    for (0..protocol.Limits.terminal_application_signal_bytes * 2) |_| {
        try input.append(std.testing.allocator, 'A');
    }
    try input.appendSlice(
        std.testing.allocator,
        "\x1b\\" ++
            "\x1b_orchbee.agent.v1;eyJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6MX0\x1b\\",
    );
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, input.items));

    var signal = (try runtime.prepareApplicationSignal(std.testing.allocator)).?;
    defer signal.deinit();
    try std.testing.expectEqual(@as(u64, 1), signal.sequence);
    try std.testing.expectEqualStrings("{\"kind\":\"launch\",\"version\":1}", signal.payload);
    runtime.commitApplicationSignal(signal);
    try std.testing.expect((try runtime.prepareApplicationSignal(std.testing.allocator)) == null);

    try runtime.forceTerminate();
}

fn expectProjectedCursor(
    runtime: *TerminalRuntime,
    expected_shape: protocol.CursorShape,
    expected_blinking: bool,
    sequence: u64,
) !void {
    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0xc3} ** 16),
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = HostEpoch.fromBytes([_]u8{0xd4} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = sequence,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(expected_shape, summary.cursor_shape);
    try std.testing.expectEqual(expected_blinking, frame.bytes[97] & 0x02 != 0);
}
test "OSC 52 admits one bounded clipboard write and never exposes clipboard reads" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]52;c;aGVsbG8gZnJvbSB0ZXJtaW5hbA==\x1b\\",
    ));
    const prepared = (try runtime.prepareClipboardWriteRequest()).?;
    try std.testing.expectEqual(@as(u64, 1), prepared.sequence);
    try std.testing.expectEqualStrings("hello from terminal", prepared.text);
    try std.testing.expect((try runtime.prepareClipboardWriteRequest()) == null);
    runtime.rollbackClipboardWriteRequest(prepared);

    const retried = (try runtime.prepareClipboardWriteRequest()).?;
    try std.testing.expectEqual(prepared.sequence, retried.sequence);
    try std.testing.expectEqualStrings(prepared.text, retried.text);
    runtime.commitClipboardWriteRequest(retried);
    try std.testing.expect((try runtime.prepareClipboardWriteRequest()) == null);

    for ([_][]const u8{
        "\x1b]52;c;?\x1b\\",
        "\x1b]52;p;aGVsbG8=\x1b\\",
        "\x1b]52;c;%%%\x1b\\",
        "\x1b]52;c;/w==\x1b\\",
        "\x1b]52;c;AA==\x1b\\",
    }) |sequence| {
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, sequence),
        );
        try std.testing.expect((try runtime.prepareClipboardWriteRequest()) == null);
    }
    try runtime.forceTerminate();
}
test "OSC 52 keeps the first pending request instead of queuing dialogs" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]52;c;Zmlyc3Q=\x1b\\",
    ));
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]52;c;c2Vjb25k\x1b\\",
    ));
    const prepared = (try runtime.prepareClipboardWriteRequest()).?;
    try std.testing.expectEqualStrings("first", prepared.text);
    runtime.commitClipboardWriteRequest(prepared);
    try std.testing.expect((try runtime.prepareClipboardWriteRequest()) == null);
    try runtime.forceTerminate();
}
test "OSC 52 accepts exactly the clipboard text limit and rejects one byte more" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    const exact = try std.testing.allocator.alloc(u8, protocol.Limits.clipboard_text_bytes);
    defer std.testing.allocator.free(exact);
    @memset(exact, 'a');
    const exact_encoded = try std.testing.allocator.alloc(
        u8,
        std.base64.standard.Encoder.calcSize(exact.len),
    );
    defer std.testing.allocator.free(exact_encoded);
    _ = std.base64.standard.Encoder.encode(exact_encoded, exact);
    const exact_sequence = try std.fmt.allocPrint(
        std.testing.allocator,
        "\x1b]52;c;{s}\x1b\\",
        .{exact_encoded},
    );
    defer std.testing.allocator.free(exact_sequence);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, exact_sequence),
    );
    const prepared = (try runtime.prepareClipboardWriteRequest()).?;
    try std.testing.expectEqual(protocol.Limits.clipboard_text_bytes, prepared.text.len);
    try std.testing.expectEqualSlices(u8, exact, prepared.text);
    runtime.commitClipboardWriteRequest(prepared);

    const over = try std.testing.allocator.alloc(u8, protocol.Limits.clipboard_text_bytes + 1);
    defer std.testing.allocator.free(over);
    @memset(over, 'b');
    const over_encoded = try std.testing.allocator.alloc(
        u8,
        std.base64.standard.Encoder.calcSize(over.len),
    );
    defer std.testing.allocator.free(over_encoded);
    _ = std.base64.standard.Encoder.encode(over_encoded, over);
    const over_sequence = try std.fmt.allocPrint(
        std.testing.allocator,
        "\x1b]52;c;{s}\x1b\\",
        .{over_encoded},
    );
    defer std.testing.allocator.free(over_sequence);
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, over_sequence),
    );
    try std.testing.expect((try runtime.prepareClipboardWriteRequest()) == null);
    try runtime.forceTerminate();
}
test "OSC 9 progress projects current state and remove without replay state" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x35} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x36} ** 16);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]9;4;1;42\x1b\\",
    ));
    const active = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(active.allocation);
    const active_summary = try protocol.validateTerminalFrame(std.testing.allocator, active.bytes);
    try std.testing.expectEqual(protocol.TerminalProgressState.set, active_summary.progress.state);
    try std.testing.expectEqual(@as(?u8, 42), active_summary.progress.value);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]9;4;0;\x1b\\",
    ));
    const removed = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(removed.allocation);
    const removed_summary = try protocol.validateTerminalFrame(std.testing.allocator, removed.bytes);
    try std.testing.expectEqual(protocol.TerminalProgressState.remove, removed_summary.progress.state);
    try std.testing.expectEqual(@as(?u8, null), removed_summary.progress.value);
    try runtime.forceTerminate();
}
test "OSC 133 projects only bounded command completion facts with deterministic duration" {
    const samples = [_]u64{ 1 * std.time.ns_per_s, 6 * std.time.ns_per_s + 500 * std.time.ns_per_ms };
    var clock: CommandClockProbe = .{
        .monotonic_values = &samples,
        .wall_milliseconds = 1_786_947_200_000,
    };
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x45} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x46} ** 16);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b]133;D;99\x07"),
    );
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b]133;C\x07"),
    );
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "private output\x1b]133;D;42\x07"),
    );

    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(@as(u16, 4), summary.extension_count);
    const completion = summary.command_completion.?;
    try std.testing.expectEqual(@as(u64, 1), completion.sequence);
    try std.testing.expectEqual(@as(u64, 5_500), completion.duration_milliseconds);
    try std.testing.expectEqual(@as(u64, 1_786_947_200_000), completion.completed_at_milliseconds);
    try std.testing.expectEqual(@as(u8, 42), completion.exit_code);
    try std.testing.expect(std.mem.indexOf(u8, frame.bytes, "private output") == null);
    try runtime.forceTerminate();
}
test "OSC 133 preserves parser order for multiple completions in one reader batch" {
    const samples = [_]u64{
        1 * std.time.ns_per_s,
        2 * std.time.ns_per_s,
        5 * std.time.ns_per_s,
        9 * std.time.ns_per_s,
    };
    var clock: CommandClockProbe = .{
        .monotonic_values = &samples,
        .wall_milliseconds = 1_786_947_201_000,
    };
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b]133;C\x07\x1b]133;D;0\x07\x1b]133;C\x07\x1b]133;D;999\x07",
        ),
    );
    const state = runtimeState(runtime);
    try std.testing.expectEqual(@as(u64, 2), state.command_sequence);
    const completion = state.command_completion.?;
    try std.testing.expectEqual(@as(u64, 2), completion.sequence);
    try std.testing.expectEqual(@as(u64, 4_000), completion.duration_milliseconds);
    try std.testing.expectEqual(@as(u8, 1), completion.exit_code);
    try runtime.forceTerminate();
}

test "shell correlation preserves the exact run and ignores unrelated finish markers" {
    const runtime = try createRuntimeWithHooks(.{});
    defer runtime.destroy();
    const state = runtimeState(runtime);
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b_orchbee.command.v1;ready\x1b\\"));
    try std.testing.expect(state.command_observation.committed.ready);
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b]133;C\x07\x1b_orchbee.command.v1;start;11111111111111111111111111111111\x1b\\"));
    try std.testing.expect(!state.command_observation.committed.ready);
    try std.testing.expect(!state.command_observation.committed.command.?.completed);
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "result\r\n\x1b_orchbee.command.v1;finish;22222222222222222222222222222222;0\x1b\\"));
    try std.testing.expect(!state.command_observation.committed.command.?.completed);
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b_orchbee.command.v1;finish;11111111111111111111111111111111;1\x1b\\"));
    try std.testing.expect(state.command_observation.committed.command.?.completed);
    try std.testing.expectEqual(@as(i32, 1), state.command_observation.committed.command.?.exit_code);
    try runtime.forceTerminate();
}

test "a start marker records when the Host read it and frames publish that time" {
    const samples = [_]u64{ 1, 2, 3, 4 };
    var clock: CommandClockProbe = .{
        .monotonic_values = &samples,
        .wall_milliseconds = 1_786_947_200_123,
    };
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b_orchbee.command.v1;start;11111111111111111111111111111111\x1b\\"));
    const started_at: u64 = 1_786_947_200_123 * std.time.ns_per_ms;
    try std.testing.expectEqual(started_at, runtimeState(runtime).command_observation.committed.command.?.started_at_nanoseconds);
    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0x47} ** 16),
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = HostEpoch.fromBytes([_]u8{0x48} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(started_at, summary.command_observation.?.command.?.started_at_nanoseconds);
    try runtime.forceTerminate();
}
test "Windows child exiting 259 completes normally with exit code 259 (THCR-041)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec");
    defer std.testing.allocator.free(executable);
    const arguments = [_][]const u8{ "/D", "/S", "/C", "exit 259" };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const completion = try runtime.waitForCompletion();
    try runtime.shutdown();
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    try std.testing.expect(completion.writer_stopped);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(
        @as(?subject.RuntimeFailureKind, null),
        runtime.primaryFailure(),
    );
    try std.testing.expectEqual(
        pty_process.ExitResult{ .exit_code = 259 },
        completion.exit_result.?,
    );
}
test "normal completion freezes revision generation geometry and configuration" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const arguments = [_][]const u8{
        "-c",
        "i=0; while [ $i -lt 100 ]; do printf 'line\\n'; i=$((i+1)); done",
    };
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 8,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 3,
    }, .{ .scrollback_bytes = 1_000_000 });
    defer runtime.destroy();

    const settled = try runtime.waitForCompletion();
    const settled_geometry = runtime.geometry();
    const settled_configuration = runtime.configuration();
    try std.testing.expect(settled.pty_drained);
    try std.testing.expectEqual(settled.final_revision, settled_geometry.revision);

    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.updateConfiguration(.{ .scrollback_bytes = 0 }),
    );
    try std.testing.expectError(
        error.TerminalRuntimeUnavailable,
        runtime.resizeGeometry(9, 4, 2, 2),
    );
    try std.testing.expectEqualDeep(settled_geometry, runtime.geometry());
    try std.testing.expectEqualDeep(settled_configuration, runtime.configuration());
    try std.testing.expectEqual(settled.final_revision, runtime.completion().final_revision);
    try runtime.shutdown();
}
test "attachment preparation applies the live gate to every resize capability" {
    const runtime = try createImmediateExitRuntime();
    defer runtime.destroy();
    _ = try runtime.waitForCompletion();

    const read_attachment = AttachmentId.fromBytes([_]u8{0x21} ** 16);
    {
        var preparation = try runtime.prepareAttachment(
            read_attachment,
            .{ .input = false, .read = true, .resize = false },
        );
        defer preparation.deinit();
    }
    try expectAttachmentPreparationUnavailable(
        runtime,
        AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .{ .input = false, .read = false, .resize = true },
    );
    try expectAttachmentPreparationUnavailable(
        runtime,
        AttachmentId.fromBytes([_]u8{0x23} ** 16),
        .{ .input = false, .read = true, .resize = true },
    );
}
test "resize-only attachment preparation reserves no input ownership capacity" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    runtimeState(runtime).input_mutex.lock();
    const capacity_before = runtimeState(runtime).input_ownership.attachments.capacity();
    runtimeState(runtime).input_mutex.unlock();

    var preparation = try runtime.prepareAttachment(
        AttachmentId.fromBytes([_]u8{0x24} ** 16),
        .{ .input = false, .read = false, .resize = true },
    );
    defer preparation.deinit();

    runtimeState(runtime).input_mutex.lock();
    const capacity_after = runtimeState(runtime).input_ownership.attachments.capacity();
    runtimeState(runtime).input_mutex.unlock();
    try std.testing.expectEqual(capacity_before, capacity_after);
}
test "attachment preparation derives reservations from canonical capabilities" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const cases = [_]subject.Capabilities{
        .{ .input = true, .read = false, .resize = false },
        .{ .input = false, .read = true, .resize = false },
        .{ .input = false, .read = false, .resize = true },
        .{ .input = true, .read = true, .resize = false },
        .{ .input = true, .read = false, .resize = true },
        .{ .input = false, .read = true, .resize = true },
        .{ .input = true, .read = true, .resize = true },
    };

    for (cases, 0..) |capabilities, index| {
        var bytes = [_]u8{0x30} ** 16;
        bytes[bytes.len - 1] = @intCast(index);
        const attachment_id = AttachmentId.fromBytes(bytes);
        var preparation = try runtime.prepareAttachment(attachment_id, capabilities);
        try std.testing.expectEqual(capabilities.read, preparation.history_reserved);
        try std.testing.expectEqual(capabilities.input, preparation.input_state_reserved);
        preparation.commit();
        try std.testing.expectEqual(
            capabilities.input,
            subject.testing.hasAttachmentInputOwnership(runtime, attachment_id),
        );
    }
}
test "runtime resets projection resources after encode failure" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
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

    const failed_request: ProjectionEncodeRequest = .{
        .attachment_id = AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0x11} ** 16),
        .maximum_bytes = 0,
        .sequence = 1,
    };
    const attempted_revision = runtime.geometry().revision;
    const blocked = try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        failed_request,
    );
    switch (blocked) {
        .blocked => |failure| {
            try std.testing.expectEqual(attempted_revision, failure.attempted_revision);
            try std.testing.expectEqual(
                subject.ProjectionBlockReason.projection_too_large,
                failure.reason,
            );
        },
        .frame => |frame| {
            std.testing.allocator.free(frame.allocation);
            return error.ExpectedProjectionBlock;
        },
    }
    try std.testing.expect(!runtime.projectionResident());
    try std.testing.expectError(
        error.ProjectionTooLarge,
        runtime.encodeProjectionFrame(failed_request),
    );
    try std.testing.expect(!runtime.projectionResident());

    const frame = try runtime.encodeProjectionFrame(.{
        .attachment_id = failed_request.attachment_id,
        .base_generation = failed_request.base_generation,
        .base_revision = failed_request.base_revision,
        .force_snapshot = failed_request.force_snapshot,
        .host_epoch = failed_request.host_epoch,
        .maximum_bytes = 8 * 1024 * 1024,
        .sequence = 2,
    });
    defer std.testing.allocator.free(frame.allocation);
    try std.testing.expect(frame.bytes.len > 0);
}
test "reader alternate-screen batch publishes one coherent projection generation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    const primary_bytes = "\x1b]2;Orchbee primary\x1b\\" ++
        "\x1b]7;file:///workspace/primary\x1b\\" ++
        "\x1b[4 q\x1b[?25l\x1b[?1h\x1b=" ++
        "\x1b[?2004h\x1b[?1002h\x1b[?1006h" ++
        "\x1b[>4;2m\x1b[=31;1u\x1b[HPRI";
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        primary_bytes,
    ));
    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).terminal.modes.set(.focus_event, true);
    runtimeState(runtime).terminal_mutex.unlock();
    const primary_geometry = runtime.geometry();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x62} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x71} ** 16);
    const primary = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(primary.allocation);
    const primary_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        primary.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, primary_summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.primary, primary_summary.active_screen);
    try std.testing.expectEqualStrings("Orchbee primary", primary_summary.title.?);
    try std.testing.expectEqualStrings(
        "file:///workspace/primary",
        primary_summary.current_directory.?,
    );
    try std.testing.expectEqual(protocol.CursorShape.underline, primary_summary.cursor_shape);
    try std.testing.expectEqual(@as(u8, 0), primary.bytes[97] & 0x03);
    const primary_interaction = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) |
        (1 << 6) | (1 << 9) | (1 << 12) | (1 << 13) | (1 << 14) |
        (1 << 15) | (1 << 16) | (1 << 17) | (1 << 18);
    try std.testing.expectEqual(
        @as(u32, primary_interaction),
        try readFrameInt(u32, primary.bytes, 100),
    );

    const alternate_bytes = "\x1b[?1049h\x1b[?1007l" ++
        "\x1b]2;Orchbee alternate\x1b\\" ++
        "\x1b]7;file:///workspace/alternate\x1b\\" ++
        "\x1b[5 q\x1b[?25h\x1b[?1l\x1b>" ++
        "\x1b[?2004l\x1b[?1004l\x1b[?1002l\x1b[?1006l" ++
        "\x1b[>4;0m\x1b[?1003h\x1b[?1016h\x1b[=3;1u\x1b[HALT";
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        alternate_bytes,
    ));
    const alternate_geometry = runtime.geometry();
    try std.testing.expectEqual(primary_geometry.generation + 1, alternate_geometry.generation);
    try std.testing.expectEqual(primary_geometry.revision + 1, alternate_geometry.revision);

    const alternate = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = primary_summary.generation,
            .base_revision = primary_summary.revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(alternate.allocation);
    const alternate_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        alternate.bytes,
    );
    try std.testing.expectEqual(protocol.FrameKind.snapshot, alternate_summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, alternate_summary.active_screen);
    try std.testing.expectEqualStrings("Orchbee alternate", alternate_summary.title.?);
    try std.testing.expectEqualStrings(
        "file:///workspace/alternate",
        alternate_summary.current_directory.?,
    );
    try std.testing.expectEqual(protocol.CursorShape.bar, alternate_summary.cursor_shape);
    try std.testing.expectEqual(@as(u8, 0x03), alternate.bytes[97] & 0x03);
    const alternate_interaction = (1 << 7) | (1 << 11) | (1 << 13) | (1 << 14);
    try std.testing.expectEqual(
        @as(u32, alternate_interaction),
        try readFrameInt(u32, alternate.bytes, 100),
    );
    try std.testing.expectEqual(alternate_geometry.generation, alternate_summary.generation);
    try std.testing.expectEqual(alternate_geometry.revision, alternate_summary.revision);

    // A mode-only change must publish the new fact without changing screens.
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?1007h"));
    const mode_frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = alternate_summary.generation,
            .base_revision = alternate_summary.revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 3,
        },
    ));
    defer std.testing.allocator.free(mode_frame.allocation);
    const mode_summary = try protocol.validateTerminalFrame(std.testing.allocator, mode_frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.delta, mode_summary.kind);
    try std.testing.expectEqual(alternate_summary.generation, mode_summary.generation);
    try std.testing.expectEqual(alternate_summary.revision + 1, mode_summary.revision);
    try std.testing.expectEqual(@as(u32, alternate_interaction | (1 << 18)), try readFrameInt(u32, mode_frame.bytes, 100));
}
test "terminal metadata events coalesce by revision and replay once per Host connection" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b]2;Background agent\x1b\\" ++
            "\x1b]7;file:///workspace/background-agent\x1b\\",
    ));
    const changed_revision = runtime.geometry().revision;
    var first = (try runtime.prepareMetadataEvent(std.testing.allocator, 41)).?;
    defer first.deinit();
    try std.testing.expectEqual(changed_revision, first.revision);
    try std.testing.expectEqualStrings("Background agent", first.title.?);
    try std.testing.expectEqualStrings(
        "file:///workspace/background-agent",
        first.current_directory.?,
    );
    runtime.commitMetadataEvent(first);
    var duplicate = try runtime.prepareMetadataEvent(std.testing.allocator, 41);
    defer if (duplicate) |*event| event.deinit();
    try std.testing.expect(duplicate == null);

    var replay = (try runtime.prepareMetadataEvent(std.testing.allocator, 42)).?;
    defer replay.deinit();
    try std.testing.expectEqual(changed_revision, replay.revision);
    runtime.commitMetadataEvent(replay);

    const reset = try runtime.applyTerminalControl(.reset);
    var cleared = (try runtime.prepareMetadataEvent(std.testing.allocator, 42)).?;
    defer cleared.deinit();
    try std.testing.expectEqual(reset.revision, cleared.revision);
    try std.testing.expectEqual(@as(?[]const u8, null), cleared.title);
    try std.testing.expectEqual(@as(?[]const u8, null), cleared.current_directory);
}
test "application clear erases and homes the active screen without resetting metadata or modes" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?1049h\x1b]2;Clear metadata\x1b\\" ++
            "\x1b]7;file:///workspace/clear\x1b\\" ++
            "\x1b[?2004h\x1b[?1002h\x1b[?1006h\x1b[3;4HCLEAR",
    ));
    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).terminal.modes.set(.focus_event, true);
    runtimeState(runtime).terminal_mutex.unlock();
    const before = runtime.geometry();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x63} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x72} ** 16);
    const baseline = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(baseline.allocation);
    const baseline_summary = try protocol.validateTerminalFrame(
        std.testing.allocator,
        baseline.bytes,
    );
    const baseline_interaction = try readFrameInt(u32, baseline.bytes, 100);

    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const update = try runtime.applyTerminalControl(.clear);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const after = runtime.geometry();
    try std.testing.expectEqual(before.generation + 1, update.generation);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expectEqual(update.generation, after.generation);
    try std.testing.expectEqual(update.revision, after.revision);
    try std.testing.expectEqual(before.columns, after.columns);
    try std.testing.expectEqual(before.rows, after.rows);
    try std.testing.expectEqual(before.cell_pixel_width, after.cell_pixel_width);
    try std.testing.expectEqual(before.cell_pixel_height, after.cell_pixel_height);

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqual(@as(usize, 0), std.mem.trim(u8, plain, " \r\n\t").len);

    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = attachment_id,
            .base_generation = before.generation,
            .base_revision = before.revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, summary.active_screen);
    try std.testing.expectEqualStrings("Clear metadata", summary.title.?);
    try std.testing.expectEqualStrings("file:///workspace/clear", summary.current_directory.?);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_column);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_row);
    try std.testing.expectEqual(baseline_summary.active_screen, summary.active_screen);
    try std.testing.expectEqual(baseline_interaction, try readFrameInt(u32, frame.bytes, 100));
}
test "application reset returns alternate state to a fresh primary terminal" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "PRIMARY\x1b[?1049h\x1b]2;Reset metadata\x1b\\" ++
            "\x1b]7;file:///workspace/reset\x1b\\" ++
            "\x1b[5 q\x1b[?25l\x1b[?1h\x1b=\x1b[?2004h" ++
            "\x1b[?1003h\x1b[?1016h\x1b]133;A;redraw=1\x07\x1b[4;5HRESET",
    ));
    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).terminal.modes.set(.focus_event, true);
    runtimeState(runtime).terminal_mutex.unlock();
    const before = runtime.geometry();

    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const update = try runtime.applyTerminalControl(.reset);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    const after = runtime.geometry();
    try std.testing.expectEqual(before.generation + 1, update.generation);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expectEqualDeep(before.columns, after.columns);
    try std.testing.expectEqualDeep(before.rows, after.rows);
    try std.testing.expectEqualDeep(before.cell_pixel_width, after.cell_pixel_width);
    try std.testing.expectEqualDeep(before.cell_pixel_height, after.cell_pixel_height);
    try std.testing.expect(!runtime.focusInputWouldEmit());

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqual(@as(usize, 0), std.mem.trim(u8, plain, " \r\n\t").len);

    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0x64} ** 16),
            .base_generation = before.generation,
            .base_revision = before.revision,
            .force_snapshot = false,
            .host_epoch = HostEpoch.fromBytes([_]u8{0x73} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(protocol.ActiveScreen.primary, summary.active_screen);
    try std.testing.expectEqual(@as(?[]const u8, null), summary.title);
    try std.testing.expectEqual(@as(?[]const u8, null), summary.current_directory);
    try std.testing.expectEqual(protocol.CursorShape.block, summary.cursor_shape);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_column);
    try std.testing.expectEqual(@as(u16, 0), summary.cursor_row);
    // A fresh Ghostty terminal enables DEC 1007; all other interaction bits reset.
    try std.testing.expectEqual(@as(u32, 1 << 18), try readFrameInt(u32, frame.bytes, 100));
    if (target_builtin.os.tag == .windows) {
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
            runtime,
            "PS> \x1b]133;P\x07\x1b]133;B\x07",
        ));
        _ = try runtime.resizeGeometry(10, 7, 0, 0);
        const prompt = try runtime.plainString(std.testing.allocator);
        defer std.testing.allocator.free(prompt);
        try std.testing.expectEqualStrings("PS> ", prompt);
    }
}
test "application terminal controls reject counter exhaustion before mutation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "unchanged"),
    );

    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).generation = std.math.maxInt(u64);
    runtimeState(runtime).terminal_mutex.unlock();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.clear));
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();
    try expectCanonicalSnapshotEqual(before, after);

    runtimeState(runtime).terminal_mutex.lock();
    runtimeState(runtime).generation = 1;
    runtimeState(runtime).revision.store(std.math.maxInt(u64), .monotonic);
    runtimeState(runtime).terminal_mutex.unlock();
    var revision_before = try canonicalSnapshot(runtime);
    defer revision_before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.reset));
    var revision_after = try canonicalSnapshot(runtime);
    defer revision_after.deinit();
    try expectCanonicalSnapshotEqual(revision_before, revision_after);
}
test "reader applies terminal-driven 80 and 132 column modes to the PTY before commit" {
    var probe: harness.GatedPtyProbe(TerminalDrivenResizeProbe) = .{ .probe = .{} };
    const resize_probe = &probe.probe;
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        runtime.destroy();
    }
    probe.reader.wait();

    const initial = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 80), initial.columns);
    try std.testing.expectEqual(@as(u16, 24), initial.rows);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?40h\x1b[?3h",
    ));
    const expanded = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 132), expanded.columns);
    try std.testing.expectEqual(initial.rows, expanded.rows);
    try std.testing.expectEqual(initial.cell_pixel_width, expanded.cell_pixel_width);
    try std.testing.expectEqual(initial.cell_pixel_height, expanded.cell_pixel_height);
    try std.testing.expectEqual(initial.generation + 1, expanded.generation);
    try std.testing.expectEqual(initial.revision + 1, expanded.revision);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?3l",
    ));
    const restored = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 80), restored.columns);
    try std.testing.expectEqual(expanded.rows, restored.rows);
    try std.testing.expectEqual(expanded.cell_pixel_width, restored.cell_pixel_width);
    try std.testing.expectEqual(expanded.cell_pixel_height, restored.cell_pixel_height);
    try std.testing.expectEqual(expanded.generation + 1, restored.generation);
    try std.testing.expectEqual(expanded.revision + 1, restored.revision);

    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(initial.columns, calls[0].previous.columns);
    try std.testing.expectEqual(expanded.columns, calls[0].next.columns);
    try std.testing.expectEqual(expanded.columns, calls[1].previous.columns);
    try std.testing.expectEqual(restored.columns, calls[1].next.columns);
    try std.testing.expectEqual(initial.rows, calls[0].next.rows);
    try std.testing.expectEqual(initial.cell_pixel_width, calls[0].next.cell_pixel_width);
    try std.testing.expectEqual(initial.cell_pixel_height, calls[0].next.cell_pixel_height);

    try runtime.forceTerminate();
}
test "runtime resize commits pixel-only transitions between known and unknown" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
    });
    defer destroyRuntimeWithIdentityRetry(runtime);

    const initial = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 9), initial.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 18), initial.cell_pixel_height);

    // Known → unknown at unchanged cells: the explicit {0, 0} pair clears
    // Ghostty pixel extents instead of preserving the measured values.
    const cleared_revision = try runtime.resizeGeometry(80, 24, 0, 0);
    try std.testing.expectEqual(initial.revision + 1, cleared_revision);
    const unknown = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 0), unknown.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 0), unknown.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 80), unknown.columns);
    try std.testing.expectEqual(@as(u16, 24), unknown.rows);
    runtimeState(runtime).terminal_mutex.lock();
    const cleared_width_px = runtimeState(runtime).terminal.width_px;
    const cleared_height_px = runtimeState(runtime).terminal.height_px;
    runtimeState(runtime).terminal_mutex.unlock();
    try std.testing.expectEqual(@as(u32, 0), cleared_width_px);
    try std.testing.expectEqual(@as(u32, 0), cleared_height_px);

    // Unknown → known installs the newly measured pair in the same runtime.
    _ = try runtime.resizeGeometry(80, 24, 10, 20);
    const known = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 10), known.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 20), known.cell_pixel_height);
    runtimeState(runtime).terminal_mutex.lock();
    const restored_width_px = runtimeState(runtime).terminal.width_px;
    const restored_height_px = runtimeState(runtime).terminal.height_px;
    runtimeState(runtime).terminal_mutex.unlock();
    try std.testing.expectEqual(@as(u32, 800), restored_width_px);
    try std.testing.expectEqual(@as(u32, 480), restored_height_px);
}

test "runtime resizeGeometry rejects mixed zero positive pixel pairs" {
    const runtime = try createIdleRuntime(.{});
    defer destroyRuntimeWithIdentityRetry(runtime);
    const initial = runtime.geometry();
    try std.testing.expectError(
        error.InvalidGeometry,
        runtime.resizeGeometry(80, 24, 0, 16),
    );
    try std.testing.expectError(
        error.InvalidGeometry,
        runtime.resizeGeometry(80, 24, 8, 0),
    );
    try std.testing.expectError(
        error.InvalidGeometry,
        runtime.resizeGeometry(0, 24, 8, 16),
    );
    try std.testing.expectEqualDeep(initial, runtime.geometry());
}

test "reader ignores 132 column mode while the enabling private mode is disabled" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    const initial = runtime.geometry();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?3h"),
    );

    const after = runtime.geometry();
    try std.testing.expectEqual(initial.columns, after.columns);
    try std.testing.expectEqual(initial.rows, after.rows);
    try std.testing.expectEqual(initial.generation, after.generation);
    try std.testing.expectEqual(initial.revision + 1, after.revision);
    try std.testing.expectEqual(@as(usize, 0), resize_probe.observedCalls().len);

    try runtime.forceTerminate();
}
test "reader preserves parser order for multiple terminal-driven resizes in one commit" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

    const initial = runtime.geometry();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h\x1b[?3l"),
    );

    const final = runtime.geometry();
    try std.testing.expectEqual(initial.columns, final.columns);
    try std.testing.expectEqual(initial.rows, final.rows);
    try std.testing.expectEqual(initial.generation + 1, final.generation);
    try std.testing.expectEqual(initial.revision + 1, final.revision);
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[1].previous.columns);
    try std.testing.expectEqual(@as(u16, 80), calls[1].next.columns);

    try runtime.forceTerminate();
}
test "terminal-driven PTY resize failure fences before Ghostty mutation" {
    var resize_probe: TerminalDrivenResizeProbe = .{ .fail = true };
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    const initial = runtime.geometry();

    try std.testing.expectEqual(
        ReaderBatchOutcome.runtime_failed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3hdiscarded-after-failure"),
    );

    const after_geometry = runtime.geometry();
    try std.testing.expectEqualDeep(initial, after_geometry);
    try std.testing.expect(runtime.isResizeFenced());
    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(subject.RuntimeFailureKind.resize, runtime.primaryFailure().?);
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 1), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();
    try expectCanonicalSnapshotEqual(before, after);
}
test "attachment resize waits for a terminal-driven reader transaction" {
    var probe: harness.GatedPtyProbe(TerminalResizeSerializationProbe) = .{ .probe = .{} };
    const resize_probe = &probe.probe;
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        runtime.destroy();
    }
    probe.reader.wait();

    var reader_call: ReaderBatchCall = .{ .runtime = runtime };
    const reader_thread = try std.Thread.spawn(.{}, ReaderBatchCall.run, .{&reader_call});
    defer reader_thread.join();
    defer resize_probe.first_effect_returned.release();
    resize_probe.first_effect_returned.wait();

    var resize_call: ResizeCall = .{ .runtime = runtime };
    const resize_thread = try std.Thread.spawn(.{}, ResizeCall.run, .{&resize_call});
    defer resize_thread.join();
    defer resize_probe.first_effect_returned.release();
    resize_probe.reader_commit_waiting.wait();
    try std.testing.expectEqual(@as(usize, 1), resize_probe.observedCalls().len);

    resize_probe.first_effect_returned.release();
    reader_call.done.wait();
    resize_call.done.wait();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, reader_call.observedOutcome().?);
    switch (resize_call.observedResult()) {
        .resized => |revision| try std.testing.expectEqual(@as(u64, 2), revision),
        .failed => |err| return err,
    }
    const calls = resize_probe.observedCalls();
    try std.testing.expectEqual(@as(usize, 2), calls.len);
    try std.testing.expectEqual(@as(u16, 80), calls[0].previous.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[0].next.columns);
    try std.testing.expectEqual(@as(u16, 132), calls[1].previous.columns);
    try std.testing.expectEqual(@as(u16, 100), calls[1].next.columns);
    const final = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 100), final.columns);
    try std.testing.expectEqual(@as(u16, 30), final.rows);

    probe.reader.release();
    try runtime.forceTerminate();
}

const ResizeCursorReplyProbe = struct {
    effect_returned: TestGate = .{},
    reader_resize_waiting: TestSignal = .{},
    reply_written: TestSignal = .{},
    mutex: platform_boundary.thread_sync.Mutex = .{},
    reply: [32]u8 = undefined,
    reply_length: usize = 0,
    write_calls: usize = 0,

    fn fromContext(context: *anyopaque) *@This() {
        return @ptrCast(@alignCast(context));
    }

    fn resize(
        _: *anyopaque,
        _: Geometry,
        _: Geometry,
    ) anyerror!void {}

    fn effectReturned(context: *anyopaque) void {
        fromContext(context).effect_returned.pause();
    }

    fn readerResizeWaiting(context: *anyopaque) void {
        fromContext(context).reader_resize_waiting.notify();
    }

    fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        if (bytes.len > self.reply.len - self.reply_length) return error.ReplyCaptureOverflow;
        @memcpy(self.reply[self.reply_length..][0..bytes.len], bytes);
        self.reply_length += bytes.len;
        self.write_calls += 1;
        self.reply_written.notify();
        return bytes.len;
    }

    pub fn hooks(self: *@This()) RuntimeTestingHooks {
        return .{
            .context = self,
            .reader_resize_waiting = readerResizeWaiting,
            .resize_effect_returned = effectReturned,
            .resize_pty = resize,
            .write_pty = write,
        };
    }

    fn expectReply(self: *@This(), expected: []const u8) !void {
        self.mutex.lock();
        defer self.mutex.unlock();
        try std.testing.expectEqual(@as(usize, 1), self.write_calls);
        try std.testing.expectEqualStrings(expected, self.reply[0..self.reply_length]);
    }

    fn expectNoReply(self: *@This()) !void {
        self.mutex.lock();
        defer self.mutex.unlock();
        try std.testing.expectEqual(@as(usize, 0), self.write_calls);
        try std.testing.expectEqual(@as(usize, 0), self.reply_length);
    }
};

const ResizeCursorTestThreads = struct {
    gate: *TestGate,
    query: ?std.Thread = null,
    resize: ?std.Thread = null,

    fn deinit(self: *@This()) void {
        self.gate.release();
        if (self.query) |thread| thread.join();
        if (self.resize) |thread| thread.join();
    }
};

const CursorQueryCall = struct {
    done: TestSignal = .{},
    mutex: platform_boundary.thread_sync.Mutex = .{},
    outcome: ?ReaderBatchOutcome = null,
    runtime: *TerminalRuntime,

    fn run(self: *@This()) void {
        const outcome = commitReaderBatch(self.runtime, "\x1b[6n");
        self.mutex.lock();
        self.outcome = outcome;
        self.mutex.unlock();
        self.done.notify();
    }

    fn observedOutcome(self: *@This()) ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome.?;
    }
};

test "cursor query read during attachment resize replies from committed geometry" {
    const wait_timeout_ns = 10 * std.time.ns_per_s;
    var gated: harness.GatedPtyProbe(ResizeCursorReplyProbe) = .{ .probe = .{} };
    const probe = &gated.probe;
    const runtime = try createIdleRuntimeWithGeometryAndHooks(80, 24, .{}, gated.hooks());
    gated.runtime = runtime;
    defer {
        gated.reader.release();
        runtime.destroy();
    }
    gated.reader.wait();

    for (0..40) |index| {
        var line_buffer: [32]u8 = undefined;
        const line = try std.fmt.bufPrint(&line_buffer, "HISTORY-{d:0>3}\r\n", .{index});
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, line),
        );
    }
    const before_resize = runtime.geometry();

    var resize_call: ResizeCall = .{ .runtime = runtime };
    var threads: ResizeCursorTestThreads = .{ .gate = &probe.effect_returned };
    defer threads.deinit();
    threads.resize = try std.Thread.spawn(.{}, ResizeCall.run, .{&resize_call});
    try std.testing.expect(probe.effect_returned.waitFor(wait_timeout_ns));

    var query_call: CursorQueryCall = .{ .runtime = runtime };
    threads.query = try std.Thread.spawn(.{}, CursorQueryCall.run, .{&query_call});
    try std.testing.expect(probe.reader_resize_waiting.waitFor(wait_timeout_ns));

    try std.testing.expectEqual(@as(u16, 80), runtime.geometry().columns);
    try std.testing.expectEqual(@as(u16, 24), runtime.geometry().rows);
    try std.testing.expectEqualDeep(before_resize, runtime.geometry());
    try probe.expectNoReply();
    try std.testing.expectEqual(
        @as(usize, 0),
        inputQueueSnapshot(runtime).terminal_reply_charge.bytes,
    );

    probe.effect_returned.release();
    try std.testing.expect(resize_call.done.waitFor(wait_timeout_ns));
    try std.testing.expect(query_call.done.waitFor(wait_timeout_ns));
    try std.testing.expect(probe.reply_written.waitFor(wait_timeout_ns));

    switch (resize_call.observedResult()) {
        .resized => {},
        .failed => |err| return err,
    }
    try std.testing.expectEqual(ReaderBatchOutcome.committed, query_call.observedOutcome());
    const after_query = runtime.geometry();
    try std.testing.expectEqual(@as(u16, 100), after_query.columns);
    try std.testing.expectEqual(@as(u16, 30), after_query.rows);
    try std.testing.expectEqual(before_resize.generation + 1, after_query.generation);
    try std.testing.expectEqual(before_resize.revision + 2, after_query.revision);
    try probe.expectReply("\x1b[30;1R");

    var capture = try runtime.captureScrollbackText(std.testing.allocator, 64 * 1024);
    defer capture.deinit();
    for (0..40) |index| {
        var marker_buffer: [32]u8 = undefined;
        const marker = try std.fmt.bufPrint(&marker_buffer, "HISTORY-{d:0>3}", .{index});
        try std.testing.expectEqual(@as(usize, 1), std.mem.count(u8, capture.text(), marker));
    }
}

test "runtime projection preserves resize and hyperlink transitions" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 4,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 2,
    });
    defer runtime.destroy();

    const attachment_id = AttachmentId.fromBytes([_]u8{0x52} ** 16);
    const host_epoch = HostEpoch.fromBytes([_]u8{0x37} ** 16);
    const hyperlink_target = "https://projection.example/linked";
    const initial_revision = try writeProjectionOutput(runtime, "\x1b[HAA");
    const initial_geometry = runtime.geometry();
    try std.testing.expectEqual(initial_revision, initial_geometry.revision);

    const initial = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    ));
    defer std.testing.allocator.free(initial.allocation);
    const initial_summary = try protocol.validateTerminalFrame(std.testing.allocator, initial.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, initial_summary.kind);
    try std.testing.expectEqual(@as(u16, 4), initial_summary.columns);
    try std.testing.expectEqual(@as(u16, 2), initial_summary.rows);
    try std.testing.expectEqual(@as(u16, 2), initial_summary.row_count);
    try std.testing.expectEqual(initial_revision, initial_summary.revision);
    try expectIncludedProjectionRows(initial.bytes, &.{ 0, 1 });
    try expectProjectionRowRevisions(runtime, &.{ initial_revision, initial_revision });
    const initial_links = try observeFrameHyperlinks(initial.bytes, hyperlink_target);
    try std.testing.expect(!initial_links.any);
    try std.testing.expect(!initial_links.target);

    const linked_revision = try writeProjectionOutput(
        runtime,
        "\x1b[H\x1b]8;;" ++ hyperlink_target ++ "\x07BB\x1b]8;;\x07",
    );
    const delta = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = initial_summary.generation,
            .base_revision = initial_revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 2,
        },
    ));
    defer std.testing.allocator.free(delta.allocation);
    const delta_summary = try protocol.validateTerminalFrame(std.testing.allocator, delta.bytes);
    try std.testing.expectEqual(protocol.FrameKind.delta, delta_summary.kind);
    try std.testing.expectEqual(@as(u16, 4), delta_summary.columns);
    try std.testing.expectEqual(@as(u16, 2), delta_summary.rows);
    try std.testing.expectEqual(@as(u16, 1), delta_summary.row_count);
    try std.testing.expectEqual(initial_revision, delta_summary.base_revision.?);
    try std.testing.expectEqual(linked_revision, delta_summary.revision);
    try expectIncludedProjectionRows(delta.bytes, &.{0});
    try expectProjectionRowRevisions(runtime, &.{ linked_revision, initial_revision });
    try expectProjectionLinkTargets(runtime, &.{hyperlink_target});
    const delta_links = try observeFrameHyperlinks(delta.bytes, hyperlink_target);
    try std.testing.expect(delta_links.any);
    try std.testing.expect(delta_links.target);

    const unlinked_revision = try writeProjectionOutput(runtime, "\x1b[HCC");
    const resized_revision = try runtime.resizeGeometry(5, 3, 8, 16);
    try std.testing.expectEqual(unlinked_revision + 1, resized_revision);
    const resized = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        32,
        .{
            .attachment_id = attachment_id,
            .base_generation = initial_summary.generation,
            .base_revision = linked_revision,
            .force_snapshot = false,
            .host_epoch = host_epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 3,
        },
    ));
    defer std.testing.allocator.free(resized.allocation);
    const resized_summary = try protocol.validateTerminalFrame(std.testing.allocator, resized.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, resized_summary.kind);
    try std.testing.expectEqual(@as(u16, 5), resized_summary.columns);
    try std.testing.expectEqual(@as(u16, 3), resized_summary.rows);
    try std.testing.expectEqual(@as(u16, 3), resized_summary.row_count);
    try std.testing.expectEqual(initial_summary.generation + 1, resized_summary.generation);
    try std.testing.expectEqual(resized_revision, resized_summary.revision);
    try expectIncludedProjectionRows(resized.bytes, &.{ 0, 1, 2 });
    try expectProjectionRowRevisions(
        runtime,
        &.{ resized_revision, resized_revision, resized_revision },
    );
    try expectProjectionLinkTargets(runtime, &.{});
    const resized_links = try observeFrameHyperlinks(resized.bytes, hyperlink_target);
    try std.testing.expect(!resized_links.any);
    try std.testing.expect(!resized_links.target);
}

test "synchronized output suppresses capture only until its monotonic deadline" {
    const samples = [_]u64{
        0,
        500 * std.time.ns_per_ms,
        std.time.ns_per_s,
    };
    var clock: CommandClockProbe = .{
        .monotonic_values = &samples,
        .wall_milliseconds = 0,
    };
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "before\x1b[?2026hafter"),
    );

    const request: ProjectionEncodeRequest = .{
        .attachment_id = AttachmentId.fromBytes([_]u8{0xe1} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0xe2} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    };
    if (runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        request,
    )) |outcome| {
        switch (outcome) {
            .frame => |frame| std.testing.allocator.free(frame.allocation),
            .blocked => {},
        }
        return error.ExpectedSynchronizedOutputSuppression;
    } else |err| {
        try std.testing.expectEqual(error.SynchronizedOutput, err);
    }

    var released_request = request;
    released_request.sequence = 2;
    const released = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        released_request,
    ));
    defer std.testing.allocator.free(released.allocation);
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "beforeafter") != null);
    try runtime.forceTerminate();
}

test "a capture waits only for the paint a batch began by hiding the cursor" {
    const Clock = struct {
        now: u64 = 1_000,

        fn hooks(self: *@This()) RuntimeTestingHooks {
            return .{
                .context = self,
                .monotonic_time_nanoseconds = read,
            };
        }

        fn read(context: *anyopaque) u64 {
            const self: *@This() = @ptrCast(@alignCast(context));
            return self.now;
        }
    };
    var clock: Clock = .{};
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    const timeout_ns: u64 = 2 * std.time.ns_per_ms;
    const elapsed_ns: u64 = 500 * std.time.ns_per_us;
    try std.testing.expectEqual(@as(?u64, null), runtime.projectionPaintRemaining());

    // Hiding opens the wait, and the paint's later pieces do not extend it.
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?25l"));
    clock.now += elapsed_ns;
    try std.testing.expectEqual(@as(?u64, timeout_ns - elapsed_ns), runtime.projectionPaintRemaining());
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "text"));
    try std.testing.expectEqual(@as(?u64, timeout_ns - elapsed_ns), runtime.projectionPaintRemaining());

    // Showing the cursor ends the wait at once.
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?25h"));
    try std.testing.expectEqual(@as(?u64, null), runtime.projectionPaintRemaining());

    // A paint complete within one batch has nothing to wait for.
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?25lmore\x1b[?25h"));
    try std.testing.expectEqual(@as(?u64, null), runtime.projectionPaintRemaining());

    // A program that never shows the cursor pays one timeout, then never again.
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?25l"));
    clock.now += timeout_ns;
    try std.testing.expectEqual(@as(?u64, null), runtime.projectionPaintRemaining());
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "hidden"));
    try std.testing.expectEqual(@as(?u64, null), runtime.projectionPaintRemaining());
    try runtime.forceTerminate();
}

test "reset and successful resize release synchronized output" {
    const samples = [_]u64{ 0, 1, 2 };
    var clock: CommandClockProbe = .{
        .monotonic_values = &samples,
        .wall_milliseconds = 0,
    };
    const runtime = try createRuntimeWithHooks(clock.hooks());
    defer runtime.destroy();
    const base_request: ProjectionEncodeRequest = .{
        .attachment_id = AttachmentId.fromBytes([_]u8{0xe3} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0xe4} ** 16),
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    };

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?2026h",
    ));
    _ = try runtime.resizeGeometry(81, 25, 8, 16);
    const resized = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        base_request,
    ));
    std.testing.allocator.free(resized.allocation);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?2026h",
    ));
    _ = try runtime.applyTerminalControl(.reset);
    var reset_request = base_request;
    reset_request.sequence = 2;
    const reset = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        reset_request,
    ));
    std.testing.allocator.free(reset.allocation);

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?2026h\x1bc",
    ));
    var ris_request = base_request;
    ris_request.sequence = 3;
    const ris = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        ris_request,
    ));
    std.testing.allocator.free(ris.allocation);
    try runtime.forceTerminate();
}

test "prompt-preserving clear removes history and output while retaining the complete logical cursor line" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "old0\r\nold1\r\nold2\r\nold3\r\nold4\r\nold5\r\nold6\r\n" ++
        "\x1b[3;1Habcdefghijklmno\x1b[5;1HREMOVE\x1b[3;4H" ++
        "\x1b]2;Prompt title\x1b\\\x1b]7;file:///prompt\x1b\\\x1b[?2004h"));
    const before = runtime.geometry();
    const update = try runtime.applyTerminalControl(.clear_prompt);
    try std.testing.expectEqual(before.revision + 1, update.revision);
    try std.testing.expectEqual(before.generation + 1, update.generation);
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqualStrings("abcdefgh\nijklmno", std.mem.trim(u8, plain, " \r\n\t"));
    const terminal = &runtimeState(runtime).terminal;
    try std.testing.expectEqual(@as(u16, 0), terminal.screens.active.cursor.y);
    try std.testing.expectEqual(@as(u16, 3), terminal.screens.active.cursor.x);
    try std.testing.expectEqual(@as(usize, 6), terminal.screens.active.pages.total_rows);
    try std.testing.expectEqualStrings("Prompt title", terminal.getTitle().?);
    try std.testing.expectEqualStrings("file:///prompt", terminal.getPwd().?);
    try std.testing.expect(terminal.modes.get(.bracketed_paste));
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}

test "prompt-preserving clear refuses alternate screen without changing content or revision" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?1049hALTERNATE"));
    const before = runtime.geometry();
    const plain_before = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain_before);
    try std.testing.expectError(error.TerminalClearAlternateScreen, runtime.applyTerminalControl(.clear_prompt));
    const plain_after = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain_after);
    try std.testing.expectEqualStrings(plain_before, plain_after);
    const after = runtime.geometry();
    try std.testing.expectEqual(before.revision, after.revision);
    try std.testing.expectEqual(before.generation, after.generation);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
}

test "scrollback text capture owns all history Unicode and unwrapped lines after source destruction" {
    var capture = capture: {
        const runtime = try createIdleRuntimeWithGeometry(8, 4, .{});
        defer runtime.destroy();
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "old0\r\nold1\r\nold2\r\nabcdefghij\r\n你好é\r\nprompt"));
        const before = runtime.geometry();
        const result = try runtime.captureScrollbackText(std.testing.allocator, 4096);
        errdefer {
            var owned = result;
            owned.deinit();
        }
        try std.testing.expectEqualDeep(before, result.geometry);
        try std.testing.expectEqual(.primary, result.screen);
        try std.testing.expect(result.total_rows > before.rows);
        try std.testing.expectEqualDeep(before, runtime.geometry());
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
        try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
        try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, " changed"));
        _ = try runtime.applyTerminalControl(.clear);
        break :capture result;
    };
    defer capture.deinit();
    try std.testing.expectEqualStrings(
        "old0\nold1\nold2\nabcdefghij\n你好é\nprompt",
        std.mem.trim(u8, capture.text(), "\n"),
    );
}

test "scrollback text capture rejects exact byte overflow invalid budgets and allocation failure without mutation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 4, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "你好éabcdefghij"));
    const before = runtime.geometry();
    var reference = try runtime.captureScrollbackText(std.testing.allocator, 4096);
    defer reference.deinit();
    var exact = try runtime.captureScrollbackText(std.testing.allocator, reference.text().len);
    defer exact.deinit();
    try std.testing.expectEqualStrings(reference.text(), exact.text());
    try std.testing.expectError(error.ScrollbackTextTooLarge, runtime.captureScrollbackText(std.testing.allocator, reference.text().len - 1));
    try std.testing.expectError(error.ScrollbackTextTooLarge, runtime.captureScrollbackText(std.testing.allocator, 1));
    try std.testing.expectError(error.InvalidCaptureBudget, runtime.captureScrollbackText(std.testing.allocator, 0));
    try std.testing.expectError(error.InvalidCaptureBudget, runtime.captureScrollbackText(std.testing.allocator, engine_boundary.runtime.max_scrollback_text_bytes + 1));
    try std.testing.expectError(error.OutOfMemory, runtime.captureScrollbackText(std.testing.failing_allocator, 4096));
    try std.testing.expectEqualDeep(before, runtime.geometry());
    var after = try runtime.captureScrollbackText(std.testing.allocator, 4096);
    defer after.deinit();
    try std.testing.expectEqualStrings(reference.text(), after.text());
}

test "scrollback text capture preserves history viewport and selection and reads the active alternate screen" {
    const runtime = try createIdleRuntimeWithGeometry(8, 4, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "old0\r\nold1\r\nold2\r\nold3\r\nold4\r\nprimary\x1b]2;Editor source\x1b\\\x1b[?2004h"));
    const state = runtimeState(runtime);
    const geometry = runtime.geometry();
    const attachment = AttachmentId.fromBytes(.{0x62} ** 16);
    try state.history.attach(attachment);
    const selected = try state.history.applySelectionCommand(&state.terminal, geometry.revision, geometry.generation, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    try std.testing.expectEqual(.active, std.meta.activeTag(selected));
    _ = try state.history.takeSelectionEvent(attachment);
    state.terminal.screens.active.scroll(.{ .delta_row = -1 });
    const viewport = state.terminal.screens.active.pages.pin(.{ .viewport = .{} }).?;
    var primary = try runtime.captureScrollbackText(std.testing.allocator, 4096);
    defer primary.deinit();
    try std.testing.expectEqualDeep(viewport, state.terminal.screens.active.pages.pin(.{ .viewport = .{} }).?);
    try std.testing.expect(state.history.states.items[0].selection_state.active());
    try std.testing.expect((try state.history.takeSelectionEvent(attachment)) == null);
    try std.testing.expectEqualDeep(geometry, runtime.geometry());
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, "\x1b[?1049h\x1b[HALT你好"));
    const alternate_geometry = runtime.geometry();
    var alternate = try runtime.captureScrollbackText(std.testing.allocator, 4096);
    defer alternate.deinit();
    try std.testing.expectEqual(.alternate, alternate.screen);
    try std.testing.expectEqualStrings("ALT你好", std.mem.trim(u8, alternate.text(), "\n"));
    try std.testing.expectEqualDeep(alternate_geometry, runtime.geometry());
    try std.testing.expectEqualStrings("Editor source", state.terminal.getTitle().?);
    try std.testing.expect(state.terminal.modes.get(.bracketed_paste));
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}
