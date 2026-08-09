const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const model_boundary = @import("terminal-host-model");
const platform_boundary = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const subject = engine_boundary.runtime;
const pty_process = platform_boundary.process;

const applyScrollbackBytes = subject.testing.applyScrollbackBytesFn;
const cancelInput = subject.testing.cancelInputFn;
const clearTestingHooks = subject.testing.clearTestingHooks;
const closeClientInput = subject.testing.closeClientInputFn;
const CompletionClassification = subject.testing.CompletionClassificationType;
const completionClassificationLoad = subject.testing.completionClassificationLoadFn;
const createWithOptionsUsingSpawner = subject.testing.createWithOptionsUsingSpawnerFn;
const deinitOwnedProfile = subject.testing.deinitOwnedProfileFn;
const InputEntry = subject.testing.InputEntryType;
const FirstContainmentAttempt = subject.testing.FirstContainmentAttemptType;
const InputReservation = subject.testing.InputReservationType;
const InputSource = subject.testing.InputSourceType;
const initOwnedProfile = subject.testing.initOwnedProfileFn;
const modifiers = subject.testing.modifiersFn;
const mouseEncodeEvent = subject.testing.mouseEncodeEventFn;
const mouseHasPressedButton = subject.testing.mouseHasPressedButtonFn;
const nextWritableInputEntry = subject.testing.nextWritableInputEntryFn;
const markCompletionFailure = subject.testing.markCompletionFailureFn;
const markCompletionNormal = subject.testing.markCompletionNormalFn;
const markNormalCompletionReady = subject.testing.markNormalCompletionReadyFn;
const ProjectionEncodeRequest = subject.ProjectionEncodeRequest;
const AttachmentId = subject.AttachmentId;
const HostEpoch = subject.HostEpoch;
const reserveInput = subject.testing.reserveInputFn;
const TerminalRuntime = subject.TerminalRuntime;
const max_pending_input_bytes = subject.testing.pending_input_limit;
const max_pending_input_entries = subject.testing.pending_input_entry_limit;
const max_pending_terminal_reply_bytes = subject.testing.pending_terminal_reply_limit;
const max_writer_slice_bytes = subject.testing.writer_slice_limit;
const publishInput = subject.testing.publishInputFn;
const recordReaderFailure = subject.testing.recordReaderFailureFn;
const recordTerminalReplyFailure = subject.testing.recordTerminalReplyFailure;
const recordWorkerIntegrityFailure = subject.testing.recordWorkerIntegrityFailureFn;
const recordWriterFailure = subject.testing.recordWriterFailureFn;
const releaseOutputForShutdown = subject.testing.releaseOutputForShutdown;
const ReaderBatchOutcome = subject.testing.ReaderBatchOutcomeType;
const resizePtyWithRollback = subject.testing.resizePtyWithRollbackFn;
const RuntimeTestingHooks = subject.testing.RuntimeTestingHooksType;
const Geometry = model_boundary.geometry.Geometry;

const TestGate = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,
    released: bool = false,

    fn pause(self: *TestGate) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        while (!self.released) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn wait(self: *TestGate) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn release(self: *TestGate) void {
        self.mutex.lock();
        self.released = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

const TestSignal = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,

    fn notify(self: *TestSignal) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn wait(self: *TestSignal) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }
};

test "runtime completion classification is terminal and preserves the first outcome" {
    var classification: CompletionClassification = .{};
    switch (completionClassificationLoad(&classification)) {
        .pending => {},
        .normal, .failed => return error.ExpectedPendingCompletion,
    }

    try std.testing.expect(markCompletionNormal(&classification));
    switch (completionClassificationLoad(&classification)) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }

    markCompletionFailure(&classification, .pty_read);
    switch (completionClassificationLoad(&classification)) {
        .normal => {},
        .pending, .failed => return error.ExpectedTerminalNormalCompletion,
    }
    try std.testing.expect(!markCompletionNormal(&classification));

    var failed_classification: CompletionClassification = .{};
    markCompletionFailure(&failed_classification, .pty_read);
    markCompletionFailure(&failed_classification, .pty_write);
    switch (completionClassificationLoad(&failed_classification)) {
        .failed => |failure| try std.testing.expectEqual(
            subject.RuntimeFailureKind.pty_read,
            failure,
        ),
        .pending, .normal => return error.ExpectedFirstFailure,
    }
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
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .max_scrollback = 1_000_000,
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

fn exerciseOwnedProfileAllocations(allocator: std.mem.Allocator) !void {
    const features = [_]u16{ 1, 22, 52 };
    const palette = [_]u32{0} ** 256;
    var profile = try initOwnedProfile(allocator, .{
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
        .enquiry_response = "orchbee-enquiry",
        .palette = &palette,
        .version_response = "orchbee-version",
    });
    defer deinitOwnedProfile(&profile, allocator);
}

const FailingThreadSpawner = struct {
    fail_at: usize,
    attempts: usize = 0,

    pub fn spawn(
        self: *FailingThreadSpawner,
        comptime thread_function: fn (*TerminalRuntime) void,
        runtime: *TerminalRuntime,
    ) !std.Thread {
        const attempt = self.attempts;
        self.attempts += 1;
        if (attempt == self.fail_at) return error.InjectedThreadSpawnFailure;
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

const HookedThreadSpawner = struct {
    hooks: RuntimeTestingHooks,

    pub fn terminalRuntimeTestingHooks(self: *HookedThreadSpawner) RuntimeTestingHooks {
        return self.hooks;
    }

    pub fn spawn(
        _: *HookedThreadSpawner,
        comptime thread_function: fn (*TerminalRuntime) void,
        runtime: *TerminalRuntime,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

fn createRuntimeWithHooks(hooks: RuntimeTestingHooks) !*TerminalRuntime {
    const builtin = @import("builtin");
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    if (builtin.os.tag == .windows) {
        const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
        defer std.testing.allocator.free(executable);
        const arguments = [_][]const u8{
            "/D",
            "/S",
            "/C",
            "more",
        };
        return createWithOptionsUsingSpawner(std.testing.allocator, .{
            .arguments = &arguments,
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = executable,
            .rows = 24,
        }, .{}, &spawner);
    }
    return createWithOptionsUsingSpawner(std.testing.allocator, .{
        .arguments = &.{},
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/cat",
        .rows = 24,
    }, .{}, &spawner);
}

fn waitForWriterStop(runtime: *TerminalRuntime) void {
    runtime.input_mutex.lock();
    while (!runtime.writer_stopped.load(.acquire)) {
        runtime.input_condition.wait(&runtime.input_mutex);
    }
    runtime.input_mutex.unlock();
}

const WriterPause = enum {
    copied,
    started,
};

const WriterProbe = struct {
    copied: TestGate = .{},
    mutex: std.Thread.Mutex = .{},
    pause_at: WriterPause,
    started: TestGate = .{},
    write_bytes: usize = 0,
    write_calls: usize = 0,

    fn fromContext(context: *anyopaque) *WriterProbe {
        return @ptrCast(@alignCast(context));
    }

    fn sliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        if (self.pause_at == .copied) self.copied.pause();
    }

    fn sliceStarted(context: *anyopaque) void {
        const self = fromContext(context);
        if (self.pause_at == .started) self.started.pause();
    }

    fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.mutex.unlock();
        return bytes.len;
    }

    fn hooks(self: *WriterProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .write_pty = write,
            .writer_slice_copied = sliceCopied,
            .writer_slice_started = sliceStarted,
        };
    }

    fn counts(self: *WriterProbe) struct { bytes: usize, calls: usize } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .bytes = self.write_bytes, .calls = self.write_calls };
    }
};

const ReaderProbe = struct {
    batch: []const u8,
    batch_finished: TestGate = .{},
    batch_read: TestGate = .{},
    mutex: std.Thread.Mutex = .{},
    outcome: ?ReaderBatchOutcome = null,
    read_calls: usize = 0,
    stopped: TestSignal = .{},

    fn fromContext(context: *anyopaque) *ReaderProbe {
        return @ptrCast(@alignCast(context));
    }

    fn read(context: *anyopaque, buffer: []u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.read_calls += 1;
        const call = self.read_calls;
        self.mutex.unlock();
        if (call != 1) return error.UnexpectedSecondRead;
        std.debug.assert(self.batch.len <= buffer.len);
        @memcpy(buffer[0..self.batch.len], self.batch);
        self.batch_read.pause();
        return self.batch.len;
    }

    fn abortRead(_: *anyopaque) anyerror!void {}

    fn batchFinished(context: *anyopaque, outcome: ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.outcome = outcome;
        self.mutex.unlock();
        self.batch_finished.pause();
    }

    fn readerStopped(context: *anyopaque) void {
        fromContext(context).stopped.notify();
    }

    fn hooks(self: *ReaderProbe) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .context = self,
            .read_pty = read,
            .reader_batch_finished = batchFinished,
            .reader_stopped = readerStopped,
        };
    }

    fn observedOutcome(self: *ReaderProbe) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome;
    }

    fn readCallCount(self: *ReaderProbe) usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.read_calls;
    }
};

const CanonicalSnapshot = struct {
    generation: u64,
    output_batches: u64,
    revision: u64,
    text: []const u8,
    total_rows: usize,
    written_bytes: usize,

    fn deinit(self: *CanonicalSnapshot) void {
        std.testing.allocator.free(self.text);
        self.* = undefined;
    }
};

fn canonicalSnapshot(runtime: *TerminalRuntime) !CanonicalSnapshot {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    const screen = runtime.terminal.screens.get(runtime.terminal.screens.active_key).?;
    return .{
        .generation = runtime.generation,
        .output_batches = runtime.output_batches_since_sync.load(.acquire),
        .revision = runtime.revision.load(.monotonic),
        .text = try runtime.terminal.plainString(std.testing.allocator),
        .total_rows = screen.pages.total_rows,
        .written_bytes = runtime.written_since_sync.load(.acquire),
    };
}

fn expectCanonicalSnapshotEqual(
    expected: CanonicalSnapshot,
    actual: CanonicalSnapshot,
) !void {
    try std.testing.expectEqual(expected.generation, actual.generation);
    try std.testing.expectEqual(expected.output_batches, actual.output_batches);
    try std.testing.expectEqual(expected.revision, actual.revision);
    try std.testing.expectEqualStrings(expected.text, actual.text);
    try std.testing.expectEqual(expected.total_rows, actual.total_rows);
    try std.testing.expectEqual(expected.written_bytes, actual.written_bytes);
}

fn readFrameInt(comptime Integer: type, bytes: []const u8, offset: usize) !Integer {
    const width = @sizeOf(Integer);
    if (offset > bytes.len -| width) return error.TestUnexpectedFrame;
    return std.mem.readInt(Integer, bytes[offset..][0..width], .little);
}

fn projectionFrameRowOffset(bytes: []const u8) !usize {
    const string_count = try readFrameInt(u32, bytes, 124);
    const style_count = try readFrameInt(u16, bytes, 128);
    var offset: usize = protocol.terminal_frame_header_bytes;
    for (0..string_count) |_| {
        const length = try readFrameInt(u32, bytes, offset);
        offset = std.math.add(usize, offset, 4 + @as(usize, length)) catch
            return error.TestUnexpectedFrame;
        if (offset > bytes.len) return error.TestUnexpectedFrame;
    }
    return std.math.add(usize, offset, @as(usize, style_count) * 20) catch
        error.TestUnexpectedFrame;
}

fn expectIncludedProjectionRows(bytes: []const u8, expected: []const u16) !void {
    const columns = try readFrameInt(u16, bytes, 88);
    const row_count = try readFrameInt(u16, bytes, 130);
    try std.testing.expectEqual(expected.len, row_count);
    var offset = try projectionFrameRowOffset(bytes);
    for (expected) |expected_row| {
        try std.testing.expectEqual(expected_row, try readFrameInt(u16, bytes, offset));
        offset = std.math.add(usize, offset, 8 + @as(usize, columns) * 8) catch
            return error.TestUnexpectedFrame;
    }
}

const FrameHyperlinkObservation = struct {
    any: bool,
    target: bool,
};

fn observeFrameHyperlinks(
    bytes: []const u8,
    expected_target: []const u8,
) !FrameHyperlinkObservation {
    const string_count = try readFrameInt(u32, bytes, 124);
    const style_count = try readFrameInt(u16, bytes, 128);
    var target_index: ?u32 = null;
    var offset: usize = protocol.terminal_frame_header_bytes;
    for (0..string_count) |index| {
        const length = try readFrameInt(u32, bytes, offset);
        offset = std.math.add(usize, offset, 4) catch return error.TestUnexpectedFrame;
        if (offset > bytes.len -| length) return error.TestUnexpectedFrame;
        if (std.mem.eql(u8, bytes[offset..][0..length], expected_target)) {
            target_index = @intCast(index);
        }
        offset += length;
    }

    var observation: FrameHyperlinkObservation = .{ .any = false, .target = false };
    for (0..style_count) |_| {
        const hyperlink_index = try readFrameInt(u32, bytes, offset + 12);
        if (hyperlink_index != std.math.maxInt(u32)) {
            observation.any = true;
            if (target_index != null and hyperlink_index == target_index.?) {
                observation.target = true;
            }
        }
        offset = std.math.add(usize, offset, 20) catch return error.TestUnexpectedFrame;
    }
    return observation;
}

fn writeProjectionOutput(runtime: *TerminalRuntime, bytes: []const u8) !u64 {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    runtime.stream.nextSlice(bytes);
    const revision = std.math.add(u64, runtime.revision.load(.monotonic), 1) catch
        return error.TestUnexpectedRevisionOverflow;
    runtime.revision.store(revision, .release);
    return revision;
}

fn expectProjectionRowRevisions(runtime: *TerminalRuntime, expected: []const u64) !void {
    runtime.projection_mutex.lock();
    defer runtime.projection_mutex.unlock();
    try std.testing.expectEqualSlices(u64, expected, runtime.projection.row_revisions.items);
}

fn expectProjectionLinkTargets(runtime: *TerminalRuntime, expected: []const []const u8) !void {
    runtime.projection_mutex.lock();
    defer runtime.projection_mutex.unlock();
    try std.testing.expectEqual(expected.len, runtime.projection.link_table.values.items.len);
    for (expected, runtime.projection.link_table.values.items) |expected_target, actual_target| {
        try std.testing.expectEqualStrings(expected_target, actual_target);
    }
}

fn expectInputQueuesCleared(runtime: *TerminalRuntime) !void {
    runtime.input_mutex.lock();
    defer runtime.input_mutex.unlock();
    try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.entries);
    try std.testing.expectEqual(@as(usize, 0), runtime.terminal_reply_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.terminal_reply_charge.entries);
}

const ContainmentProbe = struct {
    abort_calls: usize = 0,
    contain_calls: usize = 0,
    first_attempt: TestGate = .{},
    host_waiting: TestSignal = .{},
    mutex: std.Thread.Mutex = .{},
    pause_first_attempt: bool,

    fn fromContext(context: *anyopaque) *ContainmentProbe {
        return @ptrCast(@alignCast(context));
    }

    fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.abort_calls += 1;
        self.mutex.unlock();
    }

    fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.contain_calls += 1;
        const call = self.contain_calls;
        self.mutex.unlock();
        if (self.pause_first_attempt and call == 1) self.first_attempt.pause();
        return error.InjectedContainmentFailure;
    }

    fn hostWaiting(context: *anyopaque) void {
        fromContext(context).host_waiting.notify();
    }

    fn hooks(self: *ContainmentProbe) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .contain_process_tree = contain,
            .context = self,
            .host_containment_waiting = hostWaiting,
        };
    }

    fn counts(self: *ContainmentProbe) struct { aborts: usize, contains: usize } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .aborts = self.abort_calls, .contains = self.contain_calls };
    }
};

const FailureReporter = struct {
    const Kind = enum { reader, writer };

    done: TestSignal = .{},
    kind: Kind,
    runtime: *TerminalRuntime,

    fn run(self: *FailureReporter) void {
        switch (self.kind) {
            .reader => recordReaderFailure(self.runtime),
            .writer => recordWriterFailure(self.runtime),
        }
        self.done.notify();
    }
};

const ShutdownProbe = struct {
    done: TestSignal = .{},
    failure: ?anyerror = null,
    mutex: std.Thread.Mutex = .{},
    runtime: *TerminalRuntime,

    fn run(self: *ShutdownProbe) void {
        self.runtime.shutdown() catch |err| {
            self.mutex.lock();
            self.failure = err;
            self.mutex.unlock();
            self.done.notify();
            return;
        };
        self.done.notify();
    }

    fn observedFailure(self: *ShutdownProbe) ?anyerror {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.failure;
    }
};

const OutputCleanupProbe = struct {
    abort_calls: usize = 0,
    abort_fails: bool,
    batch: []const u8 = "cleanup-reader-batch-must-be-discarded",
    finish_calls: usize = 0,
    finish_fails: bool,
    mutex: std.Thread.Mutex = .{},
    read_calls: usize = 0,
    reader_batch_finished: TestSignal = .{},
    reader_batch_in_flight: TestGate = .{},
    reader_outcome: ?ReaderBatchOutcome = null,
    reader_stop_notifications: usize = 0,
    reader_stopped: TestSignal = .{},
    write_bytes: usize = 0,
    write_calls: usize = 0,
    writer_copy_notifications: usize = 0,
    writer_slice_copied: TestGate = .{},
    writer_start_notifications: usize = 0,

    fn fromContext(context: *anyopaque) *OutputCleanupProbe {
        return @ptrCast(@alignCast(context));
    }

    fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.abort_calls += 1;
        const fails = self.abort_fails;
        self.mutex.unlock();
        if (fails) return error.InjectedAbortReadFailure;
    }

    fn finishPty(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.finish_calls += 1;
        const fails = self.finish_fails;
        self.mutex.unlock();
        if (fails) return error.InjectedFinishPtyFailure;
    }

    fn read(context: *anyopaque, buffer: []u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.read_calls += 1;
        const call = self.read_calls;
        self.mutex.unlock();
        if (call != 1) return error.UnexpectedSecondRead;
        std.debug.assert(self.batch.len <= buffer.len);
        @memcpy(buffer[0..self.batch.len], self.batch);
        self.reader_batch_in_flight.pause();
        return self.batch.len;
    }

    fn readerBatchFinished(context: *anyopaque, outcome: ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.reader_outcome = outcome;
        self.mutex.unlock();
        self.reader_batch_finished.notify();
    }

    fn readerStopped(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.reader_stop_notifications += 1;
        self.mutex.unlock();
        self.reader_stopped.notify();
    }

    fn writerSliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.writer_copy_notifications += 1;
        const notification = self.writer_copy_notifications;
        self.mutex.unlock();
        if (notification == 1) self.writer_slice_copied.pause();
    }

    fn writerSliceStarted(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.writer_start_notifications += 1;
        self.mutex.unlock();
    }

    fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.mutex.unlock();
        return bytes.len;
    }

    fn hooks(self: *OutputCleanupProbe) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .context = self,
            .finish_pty_after_child_exit = finishPty,
            .read_pty = read,
            .reader_batch_finished = readerBatchFinished,
            .reader_stopped = readerStopped,
            .write_pty = write,
            .writer_slice_copied = writerSliceCopied,
            .writer_slice_started = writerSliceStarted,
        };
    }

    fn allowCleanupRetry(self: *OutputCleanupProbe) void {
        self.mutex.lock();
        self.abort_fails = false;
        self.finish_fails = false;
        self.mutex.unlock();
    }

    fn counts(self: *OutputCleanupProbe) struct {
        aborts: usize,
        finishes: usize,
        read_calls: usize,
        reader_stops: usize,
        write_bytes: usize,
        write_calls: usize,
        writer_copies: usize,
        writer_starts: usize,
    } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{
            .aborts = self.abort_calls,
            .finishes = self.finish_calls,
            .read_calls = self.read_calls,
            .reader_stops = self.reader_stop_notifications,
            .write_bytes = self.write_bytes,
            .write_calls = self.write_calls,
            .writer_copies = self.writer_copy_notifications,
            .writer_starts = self.writer_start_notifications,
        };
    }

    fn observedReaderOutcome(self: *OutputCleanupProbe) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.reader_outcome;
    }
};

const ResizeEffectOutcome = enum {
    commit,
    compensated_error,
};

const ResizeProbe = struct {
    effect_calls: usize = 0,
    mutex: std.Thread.Mutex = .{},
    outcome: ResizeEffectOutcome,
    post_effect: TestGate = .{},
    post_effect_notifications: usize = 0,

    fn fromContext(context: *anyopaque) *ResizeProbe {
        return @ptrCast(@alignCast(context));
    }

    fn resize(
        context: *anyopaque,
        _: Geometry,
        _: Geometry,
    ) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.effect_calls += 1;
        const call = self.effect_calls;
        self.mutex.unlock();
        std.debug.assert(call == 1);
        if (self.outcome == .compensated_error) {
            return error.InjectedCompensatedResizeFailure;
        }
    }

    fn effectReturned(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.post_effect_notifications += 1;
        const notification = self.post_effect_notifications;
        self.mutex.unlock();
        std.debug.assert(notification == 1);
        self.post_effect.pause();
    }

    fn hooks(self: *ResizeProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .resize_effect_returned = effectReturned,
            .resize_pty = resize,
        };
    }

    fn counts(self: *ResizeProbe) struct { effects: usize, post_effects: usize } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{
            .effects = self.effect_calls,
            .post_effects = self.post_effect_notifications,
        };
    }
};

const ResizeCall = struct {
    const Result = union(enum) {
        failed: anyerror,
        resized: u64,
    };

    done: TestSignal = .{},
    mutex: std.Thread.Mutex = .{},
    result: ?Result = null,
    runtime: *TerminalRuntime,

    fn run(self: *ResizeCall) void {
        const result: Result = if (self.runtime.resizeGeometry(100, 30, 9, 18)) |revision|
            .{ .resized = revision }
        else |err|
            .{ .failed = err };
        self.mutex.lock();
        self.result = result;
        self.mutex.unlock();
        self.done.notify();
    }

    fn observedResult(self: *ResizeCall) Result {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.result.?;
    }
};

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
    for (0..3) |failure_stage| {
        var spawner: FailingThreadSpawner = .{ .fail_at = failure_stage };
        try std.testing.expectError(
            error.InjectedThreadSpawnFailure,
            createWithOptionsUsingSpawner(std.testing.allocator, .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                // The injected thread error must win because no process launch
                // is attempted until all three workers exist.
                .executable = "thread-startup-must-precede-process-spawn",
                .rows = 24,
            }, .{}, &spawner),
        );
    }
}

test "process spawn failure releases workers waiting at the startup gate" {
    var spawner: FailingThreadSpawner = .{ .fail_at = 3 };
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
    try std.testing.expectEqual(@as(usize, 3), spawner.attempts);
}

test "unrecoverable worker failures contain the process tree without a connected broker" {
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

        try std.testing.expect(runtime.containment_confirmed.load(.acquire));
        try std.testing.expectEqual(case.expected_kind, runtime.primaryFailure().?);
        try std.testing.expectError(case.expected_error, runtime.waitForCompletion());
        try runtime.shutdown();
    }
}

test "runtime parses PTY output before exposing canonical terminal state" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
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
        "set /p value=& <nul set /p =host:!value!",
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

test "runtime resets projection resources after encode failure" {
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

    const failed_request: ProjectionEncodeRequest = .{
        .attachment_id = AttachmentId.fromBytes([_]u8{0x22} ** 16),
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = HostEpoch.fromBytes([_]u8{0x11} ** 16),
        .maximum_bytes = 0,
        .sequence = 1,
    };
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

test "runtime projection preserves resize and hyperlink transitions" {
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

    const initial = try runtime.encodeProjectionFrameOwned(std.testing.allocator, 32, .{
        .attachment_id = attachment_id,
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = host_epoch,
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 1,
    });
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
    const delta = try runtime.encodeProjectionFrameOwned(std.testing.allocator, 32, .{
        .attachment_id = attachment_id,
        .base_generation = initial_summary.generation,
        .base_revision = initial_revision,
        .force_snapshot = false,
        .host_epoch = host_epoch,
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 2,
    });
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
    const resized = try runtime.encodeProjectionFrameOwned(std.testing.allocator, 32, .{
        .attachment_id = attachment_id,
        .base_generation = initial_summary.generation,
        .base_revision = linked_revision,
        .force_snapshot = false,
        .host_epoch = host_epoch,
        .maximum_bytes = protocol.Limits.frame_bytes,
        .sequence = 3,
    });
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

test "terminal generated replies are excluded from client input accounting" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const arguments = [_][]const u8{
        "-c",
        "stty -echo -icanon min 9 time 20; printf '\\033[c'; " ++
            "response=$(dd bs=9 count=1 2>/dev/null); " ++
            "expected=$(printf '\\033[?62;22c'); [ \"$response\" = \"$expected\" ]",
    };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer runtime.destroy();

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const completion = runtime.completion();
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}

test "root child exit rejects new client input without stopping terminal replies" {
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

    closeClientInput(runtime);
    try std.testing.expectError(
        error.TerminalExited,
        reserveInput(runtime, 1, false, .client),
    );
    const reply = try reserveInput(runtime, 1, false, .terminal);
    try publishInput(runtime, reply, try std.testing.allocator.dupe(u8, "x"));
    runtime.input_mutex.lock();
    try std.testing.expect(!runtime.writer_stop);
    runtime.input_mutex.unlock();
    try runtime.forceTerminate();
}

test "runtime queue applies bounded backpressure before allocation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 1" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 3 127.0.0.1 >nul" };
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

    const bytes = try std.testing.allocator.alloc(u8, max_pending_input_bytes + 1);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'x');
    try std.testing.expectError(error.InputBackpressure, runtime.enqueueInput(bytes));
    try runtime.terminate();
}

test "terminal replies preempt client slices without reordering either source" {
    var first_client = [_]u8{'a'};
    var reply = [_]u8{'r'};
    var second_client = [_]u8{'b'};
    var entries = [_]InputEntry{
        .{
            .bytes = first_client[0..],
            .identifier = 1,
            .reserved_bytes = first_client.len,
            .source = .client,
        },
        .{
            .bytes = reply[0..],
            .identifier = 2,
            .reserved_bytes = reply.len,
            .source = .terminal,
        },
        .{
            .bytes = second_client[0..],
            .identifier = 3,
            .reserved_bytes = second_client.len,
            .source = .client,
        },
    };

    try std.testing.expectEqual(@as(?usize, 1), nextWritableInputEntry(&entries));

    entries[1].bytes = null;
    try std.testing.expectEqual(@as(?usize, null), nextWritableInputEntry(&entries));

    entries[1].source = .client;
    try std.testing.expectEqual(@as(?usize, 0), nextWritableInputEntry(&entries));

    entries[0].bytes = null;
    try std.testing.expectEqual(@as(?usize, null), nextWritableInputEntry(&entries));
}

test "discarding prepared input releases its ordered queue reservation" {
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

    var prepared = try runtime.prepareRawInput("abc");
    try std.testing.expectEqual(@as(usize, 3), prepared.byteCount());
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    prepared.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "prepared input losing the failure fence is unavailable and releases its reservation" {
    const runtime = try createRuntimeWithHooks(.{});
    defer runtime.destroy();

    var prepared = try runtime.prepareRawInput("prepared");
    defer prepared.deinit();
    try std.testing.expectEqual(@as(usize, 8), runtime.pendingInputBytes());

    recordReaderFailure(runtime);
    try std.testing.expectError(error.TerminalRuntimeUnavailable, prepared.commit());
    waitForWriterStop(runtime);

    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    try std.testing.expectEqual(subject.RuntimeFailureKind.pty_read, runtime.primaryFailure().?);
}

test "client and reply writer slices obey the final failure-fence ordering" {
    const Case = struct {
        expected_calls: usize,
        pause_at: WriterPause,
        source: InputSource,
    };
    const cases = [_]Case{
        .{ .expected_calls = 0, .pause_at = .copied, .source = .client },
        .{ .expected_calls = 1, .pause_at = .started, .source = .client },
        .{ .expected_calls = 0, .pause_at = .copied, .source = .terminal },
        .{ .expected_calls = 1, .pause_at = .started, .source = .terminal },
    };
    const bytes = try std.testing.allocator.alloc(u8, max_writer_slice_bytes + 17);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'w');

    for (cases) |case| {
        var probe: WriterProbe = .{ .pause_at = case.pause_at };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer runtime.destroy();
        defer probe.copied.release();
        defer probe.started.release();

        const reservation = try reserveInput(runtime, bytes.len, false, case.source);
        try publishInput(
            runtime,
            reservation,
            try std.testing.allocator.dupe(u8, bytes),
        );
        switch (case.pause_at) {
            .copied => probe.copied.wait(),
            .started => probe.started.wait(),
        }

        recordReaderFailure(runtime);
        switch (case.pause_at) {
            .copied => probe.copied.release(),
            .started => probe.started.release(),
        }
        waitForWriterStop(runtime);

        const counts = probe.counts();
        try std.testing.expectEqual(case.expected_calls, counts.calls);
        try std.testing.expectEqual(
            case.expected_calls * max_writer_slice_bytes,
            counts.bytes,
        );
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
        const expected_client_bytes = if (case.source == .client)
            case.expected_calls * max_writer_slice_bytes
        else
            0;
        try std.testing.expectEqual(expected_client_bytes, runtime.takeWrittenBytes());
    }
}

test "an already-read PTY batch losing terminal_mutex cannot mutate canonical state" {
    var probe: ReaderProbe = .{ .batch = "must-not-commit" };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer probe.batch_read.release();
    defer probe.batch_finished.release();

    probe.batch_read.wait();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();

    recordReaderFailure(runtime);
    probe.batch_read.release();
    probe.batch_finished.wait();
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();

    try std.testing.expectEqual(ReaderBatchOutcome.failure_fenced, probe.observedOutcome().?);
    try std.testing.expectEqual(before.revision, after.revision);
    try std.testing.expectEqual(before.generation, after.generation);
    try std.testing.expectEqual(before.total_rows, after.total_rows);
    try std.testing.expectEqualStrings(before.text, after.text);
    try std.testing.expectEqual(@as(u64, 0), runtime.takeOutputBatches());

    probe.batch_finished.release();
    probe.stopped.wait();
    try std.testing.expectEqual(@as(usize, 1), probe.readCallCount());
}

test "a PTY batch winning terminal_mutex commits once and freezes at the later fence" {
    var probe: ReaderProbe = .{ .batch = "reader-winner" };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer probe.batch_read.release();
    defer probe.batch_finished.release();

    probe.batch_read.wait();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    probe.batch_read.release();
    probe.batch_finished.wait();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, probe.observedOutcome().?);

    var committed = try canonicalSnapshot(runtime);
    defer committed.deinit();
    try std.testing.expectEqual(before.revision + 1, committed.revision);
    try std.testing.expect(std.mem.indexOf(u8, committed.text, "reader-winner") != null);

    recordReaderFailure(runtime);
    probe.batch_finished.release();
    probe.stopped.wait();
    var frozen = try canonicalSnapshot(runtime);
    defer frozen.deinit();

    try std.testing.expectEqual(committed.revision, frozen.revision);
    try std.testing.expectEqual(committed.generation, frozen.generation);
    try std.testing.expectEqual(committed.total_rows, frozen.total_rows);
    try std.testing.expectEqualStrings(committed.text, frozen.text);
    try std.testing.expectEqual(@as(u64, 1), runtime.takeOutputBatches());
    try std.testing.expectEqual(@as(usize, 1), probe.readCallCount());
}

test "prepared export cancellation commits after an uncontained fatal failure" {
    var probe: ContainmentProbe = .{ .pause_first_attempt = false };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    const attachment_id = AttachmentId.fromBytes([_]u8{0x91} ** 16);
    var attachment = try runtime.prepareAttachment(attachment_id, false, true);
    defer attachment.deinit();
    attachment.commit();
    {
        runtime.terminal_mutex.lock();
        defer runtime.terminal_mutex.unlock();
        try runtime.terminal.printString("prepared cancellation");
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
        runtime.terminal_mutex.lock();
        defer runtime.terminal_mutex.unlock();
        const retained = (try runtime.history.prepareSelectionExportCancellation(attachment_id)).?;
        try std.testing.expectEqualDeep(prepared, retained);
    }

    runtime.commitPreparedSelectionExportCancellation(prepared);
    {
        runtime.terminal_mutex.lock();
        defer runtime.terminal_mutex.unlock();
        try std.testing.expect(
            (try runtime.history.prepareSelectionExportCancellation(attachment_id)) == null,
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
        runtime.input_mutex.lock();
        {
            defer runtime.input_mutex.unlock();
            try std.testing.expectEqual(@as(usize, 2), runtime.input_entries.items.len);
            try std.testing.expectEqual(client_bytes.len, runtime.client_input_charge.bytes);
            try std.testing.expectEqual(@as(usize, 1), runtime.client_input_charge.entries);
            try std.testing.expectEqual(reply_bytes.len, runtime.terminal_reply_charge.bytes);
            try std.testing.expectEqual(@as(usize, 1), runtime.terminal_reply_charge.entries);
        }

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

test "post-resize-effect barrier keeps normal completion pending through commit or compensation" {
    const cases = [_]ResizeEffectOutcome{ .commit, .compensated_error };

    for (cases) |outcome| {
        var probe: ResizeProbe = .{ .outcome = outcome };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            clearTestingHooks(runtime);
            runtime.destroy();
        }

        runtime.input_mutex.lock();
        runtime.writer_stop = true;
        runtime.input_condition.broadcast();
        runtime.input_mutex.unlock();
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
        switch (completionClassificationLoad(&runtime.completion_classification)) {
            .pending => {},
            .normal, .failed => return error.ExpectedPendingCompletionAtPostEffectBarrier,
        }

        runtime.exit_result = .{ .exit_code = 0 };
        runtime.child_exited.store(true, .release);
        runtime.pty_drained.store(true, .release);
        markNormalCompletionReady(runtime);
        try std.testing.expect(subject.testing.resizeEffectInFlight(runtime));
        switch (completionClassificationLoad(&runtime.completion_classification)) {
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

test "zero-byte semantic encodings do not retain queue metadata" {
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

    for (0..max_pending_input_entries + 1) |_| {
        try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    }
    runtime.input_mutex.lock();
    try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.terminal_reply_charge.bytes);
    runtime.input_mutex.unlock();

    var entry_reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&entry_reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const entry_reply = try reserveInput(runtime, 1, false, .terminal);
    cancelInput(runtime, entry_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    for (entry_reservations) |reservation| cancelInput(runtime, reservation.identifier);
    const byte_reservation = try reserveInput(runtime, max_pending_input_bytes, false, .client);
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const byte_reply = try reserveInput(
        runtime,
        max_pending_terminal_reply_bytes,
        false,
        .terminal,
    );
    cancelInput(runtime, byte_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    cancelInput(runtime, byte_reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "semantic input limits apply to encoded terminal bytes" {
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

    try std.testing.expectEqual(@as(usize, 3), try runtime.enqueueTextInputWithLimit("abc", 3));
    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.enqueueTextInputWithLimit("abcd", 3),
    );
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInputWithLimit(true, 0));
    try runtime.forceTerminate();
}

test "input queue bounds entry metadata independently of byte charge" {
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

    var reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    for (reservations) |reservation| cancelInput(runtime, reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
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

    runtime.input_mutex.lock();
    runtime.writer_stop = true;
    runtime.input_condition.broadcast();
    runtime.input_mutex.unlock();

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

    runtime.terminal_mutex.lock();
    runtime.generation = std.math.maxInt(u64);
    runtime.terminal_mutex.unlock();
    const generation_exhausted = runtime.geometry();
    try std.testing.expectError(
        error.RevisionOverflow,
        runtime.resizeGeometry(81, 25, 2, 3),
    );
    try std.testing.expectEqualDeep(generation_exhausted, runtime.geometry());

    runtime.terminal_mutex.lock();
    runtime.generation = 1;
    runtime.revision.store(std.math.maxInt(u64), .monotonic);
    runtime.terminal_mutex.unlock();
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

    var observed_finishing_order = false;
    for (0..600) |_| {
        const completion = runtime.completion();
        if (completion.child_exited) {
            observed_finishing_order = !completion.pty_drained;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(observed_finishing_order);

    try runtime.terminate();
    try runtime.forceTerminate();
    try runtime.shutdown();
    const completed = runtime.completion();
    try std.testing.expect(completed.child_exited);
}

test "ordered input reservations prevent overtaking and cancellation leaves no gap" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "read value; printf '<%s>' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =^<!value!^>",
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

    const first = try reserveInput(runtime, 1, false, .client);
    const cancelled = try reserveInput(runtime, 1, false, .client);
    const third = try reserveInput(runtime, 2, false, .client);
    try publishInput(runtime, third, try std.testing.allocator.dupe(u8, if (builtin.os.tag == .windows) "C\r" else "C\n"));
    cancelInput(runtime, cancelled.identifier);
    std.Thread.sleep(10 * std.time.ns_per_ms);
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    try publishInput(runtime, first, try std.testing.allocator.dupe(u8, "A"));

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "<AC>") != null);
}
