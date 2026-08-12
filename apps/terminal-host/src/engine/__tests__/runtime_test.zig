const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const model_boundary = @import("terminal-host-model");
const platform_boundary = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const subject = engine_boundary.runtime;
const terminal_launch = engine_boundary.launch;
const pty_process = platform_boundary.process;

const applyScrollbackBytes = subject.testing.applyScrollbackBytesFn;
const cancelInput = subject.testing.cancelInputFn;
const clearTestingHooks = subject.testing.clearTestingHooks;
const closeClientInput = subject.testing.closeClientInputFn;
const commitReaderBatch = subject.testing.commitReaderBatchFn;
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
// The terminal-reply burst bound the writer enforces before yielding to a
// waiting accepted client entry. It mirrors max_terminal_reply_burst_slices in
// runtime.zig and is the contract value documented in runtime-model.md.
const terminal_reply_burst_bound: usize = 64;
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

fn requireProjectionFrame(
    outcome: subject.ProjectionEncodeOutcome,
) !subject.ProjectionFrame {
    return switch (outcome) {
        .frame => |frame| frame,
        .blocked => error.UnexpectedProjectionBlock,
    };
}

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

const CountingThreadSpawner = struct {
    attempts: usize = 0,

    pub fn spawn(
        self: *CountingThreadSpawner,
        comptime thread_function: fn (*TerminalRuntime) void,
        runtime: *TerminalRuntime,
    ) !std.Thread {
        self.attempts += 1;
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

fn createRuntimeWithHooksAndCommand(
    hooks: RuntimeTestingHooks,
    executable: []const u8,
    arguments: []const []const u8,
) !*TerminalRuntime {
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    return createWithOptionsUsingSpawner(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
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

fn createIdleRuntime(options: subject.Options) !*TerminalRuntime {
    return createIdleRuntimeWithGeometry(80, 24, options);
}

fn createIdleRuntimeWithGeometry(
    columns: u16,
    rows: u16,
    options: subject.Options,
) !*TerminalRuntime {
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
    return TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = arguments,
        .columns = columns,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = rows,
    }, options);
}

fn parseTerminalInput(runtime: *TerminalRuntime, bytes: []const u8) void {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    runtime.stream.nextSlice(bytes);
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

/// A gate that pauses the writer after every slice copy, not just the first.
/// The writer blocks in `pause()` until the test releases it, so tests observe
/// and control each written slice deterministically without sleeps. A
/// generation counter makes `wait()` return only when the writer has parked at
/// a park the caller has not yet observed, so a `wait()` immediately following
/// an `unpause()` cannot return while the writer is still leaving the previous
/// park.
const RepeatingGate = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    generation: usize = 0,
    last_returned: usize = 0,
    release: bool = false,

    fn pause(self: *RepeatingGate) void {
        self.mutex.lock();
        self.release = false;
        self.generation += 1;
        self.condition.broadcast();
        while (!self.release) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn wait(self: *RepeatingGate) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.release or self.generation == self.last_returned) {
            self.condition.wait(&self.mutex);
        }
        self.last_returned = self.generation;
    }

    fn unpause(self: *RepeatingGate) void {
        self.mutex.lock();
        self.release = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

/// Drives the writer one slice at a time and records the first byte of every
/// written slice, so tests can count the terminal-reply slices that precede a
/// client slice and assert the bounded reply burst without any sleeps.
const BurstProbe = struct {
    gate: RepeatingGate = .{},
    // Once set, the writer stops pausing per slice and drains freely so the
    // test can end without a gate deadlock at shutdown.
    detached: std.atomic.Value(bool) = .init(false),
    mutex: std.Thread.Mutex = .{},
    written: [512]u8 = undefined,
    write_count: usize = 0,

    fn fromContext(context: *anyopaque) *BurstProbe {
        return @ptrCast(@alignCast(context));
    }

    fn sliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        if (!self.detached.load(.acquire)) self.gate.pause();
    }

    fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.write_count < self.written.len) {
            self.written[self.write_count] = bytes[0];
        }
        self.write_count += 1;
        return bytes.len;
    }

    fn hooks(self: *BurstProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .write_pty = write,
            .writer_slice_copied = sliceCopied,
        };
    }

    /// First byte of the most recently written slice, or null when no slice has
    /// been written yet.
    fn lastWritten(self: *BurstProbe) ?u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.write_count == 0) return null;
        return self.written[self.write_count - 1];
    }

    /// First bytes of every written slice so far.
    fn writtenBytes(self: *BurstProbe) []const u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.written[0..self.write_count];
    }
};

/// Simulates a root-exit descendant that keeps the PTY slave open: the reader
/// blocks instead of observing EOF, so pty_drained stays false and writer_stop
/// stays false even after child_exited has been published. Releasing the gate
/// returns EOF so the runtime can drain and shut down normally.
const DescendantProbe = struct {
    read_gate: TestGate = .{},
    write_bytes: usize = 0,
    write_calls: usize = 0,
    write_mutex: std.Thread.Mutex = .{},

    fn fromContext(context: *anyopaque) *DescendantProbe {
        return @ptrCast(@alignCast(context));
    }

    fn read(context: *anyopaque, _: []u8) anyerror!usize {
        const self = fromContext(context);
        self.read_gate.pause();
        return 0;
    }

    fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.write_mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.write_mutex.unlock();
        return bytes.len;
    }

    fn hooks(self: *DescendantProbe) RuntimeTestingHooks {
        return .{ .context = self, .read_pty = read, .write_pty = write };
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

const ResizeObservation = struct {
    previous: Geometry,
    next: Geometry,
};

const TerminalDrivenResizeProbe = struct {
    calls: [4]ResizeObservation = undefined,
    count: usize = 0,
    fail: bool = false,
    mutex: std.Thread.Mutex = .{},

    fn fromContext(context: *anyopaque) *TerminalDrivenResizeProbe {
        return @ptrCast(@alignCast(context));
    }

    fn resize(
        context: *anyopaque,
        previous: Geometry,
        next: Geometry,
    ) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.count < self.calls.len);
        self.calls[self.count] = .{ .previous = previous, .next = next };
        self.count += 1;
        if (self.fail) return error.InjectedTerminalDrivenResizeFailure;
    }

    fn hooks(self: *TerminalDrivenResizeProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .resize_pty = resize,
        };
    }

    fn observedCalls(self: *TerminalDrivenResizeProbe) []const ResizeObservation {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.calls[0..self.count];
    }
};

const TerminalResizeSerializationProbe = struct {
    calls: [4]ResizeObservation = undefined,
    count: usize = 0,
    effect_returns: usize = 0,
    first_effect_returned: TestGate = .{},
    mutex: std.Thread.Mutex = .{},
    reader_commit_waiting: TestSignal = .{},

    fn fromContext(context: *anyopaque) *TerminalResizeSerializationProbe {
        return @ptrCast(@alignCast(context));
    }

    fn resize(
        context: *anyopaque,
        previous: Geometry,
        next: Geometry,
    ) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.count < self.calls.len);
        self.calls[self.count] = .{ .previous = previous, .next = next };
        self.count += 1;
    }

    fn effectReturned(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.effect_returns += 1;
        const effect_return = self.effect_returns;
        self.mutex.unlock();
        if (effect_return == 1) self.first_effect_returned.pause();
    }

    fn readerCommitWaiting(context: *anyopaque) void {
        fromContext(context).reader_commit_waiting.notify();
    }

    fn hooks(self: *TerminalResizeSerializationProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .reader_commit_waiting = readerCommitWaiting,
            .resize_effect_returned = effectReturned,
            .resize_pty = resize,
        };
    }

    fn observedCalls(self: *TerminalResizeSerializationProbe) []const ResizeObservation {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.calls[0..self.count];
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

const ReaderBatchCall = struct {
    done: TestSignal = .{},
    mutex: std.Thread.Mutex = .{},
    outcome: ?ReaderBatchOutcome = null,
    runtime: *TerminalRuntime,

    fn run(self: *ReaderBatchCall) void {
        const outcome = commitReaderBatch(
            self.runtime,
            "\x1b[?40h\x1b[?3h",
        );
        self.mutex.lock();
        self.outcome = outcome;
        self.mutex.unlock();
        self.done.notify();
    }

    fn observedOutcome(self: *ReaderBatchCall) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome;
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

// THCR-041 regression: a Windows child that terminates with exit status 259
// (STILL_ACTIVE's numeric value) must classify as a normal engine completion
// carrying exit_code 259 with the child exited, PTY drained, and writer fully
// quiesced. It must never be treated as a process-observation failure. Native
// Windows execution required; compiled for `x86_64-windows-gnu` on non-Windows.
test "Windows child exiting 259 completes normally with exit code 259 (THCR-041)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const executable = try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec");
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
        "\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h" ++
        "\x1b[>4;2m\x1b[=31;1u\x1b[HPRI";
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        primary_bytes,
    ));
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
        (1 << 15) | (1 << 16) | (1 << 17);
    try std.testing.expectEqual(
        @as(u32, primary_interaction),
        try readFrameInt(u32, primary.bytes, 100),
    );

    const alternate_bytes = "\x1b[?1049h" ++
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
}

test "application clear erases and homes the active screen without resetting metadata or modes" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();

    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(
        runtime,
        "\x1b[?1049h\x1b]2;Clear metadata\x1b\\" ++
            "\x1b]7;file:///workspace/clear\x1b\\" ++
            "\x1b[?2004h\x1b[?1004h\x1b[?1002h\x1b[?1006h\x1b[3;4HCLEAR",
    ));
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
            "\x1b[?1004h\x1b[?1003h\x1b[?1016h\x1b[4;5HRESET",
    ));
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
    try std.testing.expectEqual(@as(u32, 0), try readFrameInt(u32, frame.bytes, 100));
}

test "application terminal controls reject counter exhaustion before mutation" {
    const runtime = try createIdleRuntimeWithGeometry(8, 6, .{});
    defer runtime.destroy();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "unchanged"),
    );

    runtime.terminal_mutex.lock();
    runtime.generation = std.math.maxInt(u64);
    runtime.terminal_mutex.unlock();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.clear));
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();
    try expectCanonicalSnapshotEqual(before, after);

    runtime.terminal_mutex.lock();
    runtime.generation = 1;
    runtime.revision.store(std.math.maxInt(u64), .monotonic);
    runtime.terminal_mutex.unlock();
    var revision_before = try canonicalSnapshot(runtime);
    defer revision_before.deinit();
    try std.testing.expectError(error.RevisionOverflow, runtime.applyTerminalControl(.reset));
    var revision_after = try canonicalSnapshot(runtime);
    defer revision_after.deinit();
    try expectCanonicalSnapshotEqual(revision_before, revision_after);
}

test "reader applies terminal-driven 80 and 132 column modes to the PTY before commit" {
    var resize_probe: TerminalDrivenResizeProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

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
    var resize_probe: TerminalResizeSerializationProbe = .{};
    const runtime = try createRuntimeWithHooks(resize_probe.hooks());
    defer runtime.destroy();

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

    try runtime.forceTerminate();
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

fn expectRealPtyQueryReplies(
    columns: u16,
    rows: u16,
    color_scheme: subject.ColorScheme,
    query_shell: []const u8,
    expected_shell: []const u8,
    expected_length: usize,
) !void {
    const script = try std.fmt.allocPrint(
        std.testing.allocator,
        "stty -echo -icanon min 0 time 20; " ++
            "printf '{s}'; " ++
            "response=$(dd bs=1 count={d} 2>/dev/null); " ++
            "expected=$(printf '{s}'); " ++
            "[ \"$response\" = \"$expected\" ]",
        .{ query_shell, expected_length, expected_shell },
    );
    defer std.testing.allocator.free(script);
    const arguments = [_][]const u8{ "-c", script };
    const palette = [_]u32{0} ** 256;
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = columns,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = rows,
    }, .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .profile = .{
            .color_scheme = color_scheme,
            .default_background_rgb = if (color_scheme == .dark) 0x11_11_11 else 0xf5_f5_f5,
            .default_cursor_rgb = null,
            .default_foreground_rgb = if (color_scheme == .dark) 0xff_ff_ff else 0,
            .device_primary_conformance_level = 62,
            .device_primary_features = &.{22},
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "orchbee-ready",
            .palette = &palette,
            .version_response = "orchbee-test 2.4.0",
        },
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

test "real PTY preserves the legacy terminal query and reply corpus" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const query_shell = "\\033[5n\\033[6n" ++
        "\\033[?996n\\033[>q\\005" ++
        "\\033[c\\033[>c\\033[=c" ++
        "\\033P$qm\\033\\\\" ++
        "\\033[1;31m\\033P$qm\\033\\\\" ++
        "\\033[5 q\\033P$q q\\033\\\\" ++
        "\\033[3;20r\\033P$qr\\033\\\\" ++
        "\\033P$qs\\033\\\\" ++
        "\\033[?69h\\033[5;70s\\033P$qs\\033\\\\" ++
        "\\033P$q\"q\\033\\\\";
    const expected = "\x1b[0n\x1b[1;1R" ++
        "\x1b[?997;1n" ++
        "\x1bP>|orchbee-test 2.4.0\x1b\\" ++
        "orchbee-ready" ++
        "\x1b[?62;22c\x1b[>1;0;0c\x1bP!|00000000\x1b\\" ++
        "\x1bP1$r0m\x1b\\" ++
        "\x1bP1$r0;1;31m\x1b\\" ++
        "\x1bP1$r5 q\x1b\\" ++
        "\x1bP1$r3;20r\x1b\\" ++
        "\x1bP0$r\x1b\\" ++
        "\x1bP1$r5;70s\x1b\\" ++
        "\x1bP0$r\x1b\\";
    const expected_shell = "\\033[0n\\033[1;1R" ++
        "\\033[?997;1n" ++
        "\\033P>|orchbee-test 2.4.0\\033\\\\" ++
        "orchbee-ready" ++
        "\\033[?62;22c\\033[>1;0;0c\\033P!|00000000\\033\\\\" ++
        "\\033P1$r0m\\033\\\\" ++
        "\\033P1$r0;1;31m\\033\\\\" ++
        "\\033P1$r5 q\\033\\\\" ++
        "\\033P1$r3;20r\\033\\\\" ++
        "\\033P0$r\\033\\\\" ++
        "\\033P1$r5;70s\\033\\\\" ++
        "\\033P0$r\\033\\\\";
    try expectRealPtyQueryReplies(
        80,
        24,
        .dark,
        query_shell,
        expected_shell,
        expected.len,
    );

    const size_expected = "\x1b[4;32;80t\x1b[6;16;8t\x1b[8;2;10t";
    try expectRealPtyQueryReplies(
        10,
        2,
        .dark,
        "\\033[14t\\033[16t\\033[18t",
        "\\033[4;32;80t\\033[6;16;8t\\033[8;2;10t",
        size_expected.len,
    );

    const light_expected = "\x1b[?997;2n";
    try expectRealPtyQueryReplies(
        80,
        24,
        .light,
        "\\033[?996n",
        "\\033[?997;2n",
        light_expected.len,
    );
}

test "pinned Ghostty OSC color updates and queries share one color state" {
    const Reply = struct {
        var bytes: [256]u8 = undefined;
        var length: usize = 0;

        fn reset() void {
            length = 0;
        }

        fn writePty(
            _: *ghostty_vt.TerminalStream.Handler,
            data: [:0]const u8,
        ) void {
            if (data.len > bytes.len - length) @panic("color reply exceeds test buffer");
            @memcpy(bytes[length..][0..data.len], data);
            length += data.len;
        }
    };
    Reply.reset();

    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.handler.effects.write_pty = Reply.writePty;

    stream.nextSlice(
        "\x1b]10;rgb:d8/de/e9\x1b\\" ++
            "\x1b]11;rgb:10/14/18\x1b\\" ++
            "\x1b]12;rgb:f5/a9/7f\x1b\\" ++
            "\x1b]4;1;rgb:aa/bb/cc\x1b\\",
    );
    try std.testing.expectEqual(@as(usize, 0), Reply.length);

    stream.nextSlice("\x1b]10;?;?;?\x1b\\\x1b]4;1;?\x1b\\");
    try std.testing.expectEqualStrings(
        "\x1b]10;rgb:d8d8/dede/e9e9\x1b\\" ++
            "\x1b]11;rgb:1010/1414/1818\x1b\\" ++
            "\x1b]12;rgb:f5f5/a9a9/7f7f\x1b\\" ++
            "\x1b]4;1;rgb:aaaa/bbbb/cccc\x1b\\",
        Reply.bytes[0..Reply.length],
    );
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

/// Drives the writer one slice at a time, parking after every slice copy so
/// every write is observed deterministically. A terminal reply is replenished
/// after every reply slice, so the reply stream never runs dry while the
/// accepted client entry waits. Returns the number of reply slices written
/// before the client slice, or null when the client entry was never written
/// within `maximum_reply_slices` (unbounded starvation).
fn driveReplyBurstUntilClient(
    runtime: *TerminalRuntime,
    probe: *BurstProbe,
    maximum_reply_slices: usize,
) !?usize {
    var reply_slices: usize = 0;
    while (reply_slices < maximum_reply_slices) {
        probe.gate.unpause();
        probe.gate.wait();
        const first_byte = probe.lastWritten() orelse unreachable;
        if (first_byte == 'a') return reply_slices;
        reply_slices += 1;
        const reply_reservation = try reserveInput(runtime, 3, false, .terminal);
        try publishInput(runtime, reply_reservation, try std.testing.allocator.dupe(u8, "rrr"));
    }
    return null;
}

test "accepted client entry makes bounded progress behind a continuously replenished reply stream" {
    var probe: BurstProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer {
        probe.detached.store(true, .release);
        probe.gate.unpause();
    }

    // A child continuously emits reply-producing queries. Admit one full reply
    // burst and park the writer on it before accepting the client entry, so the
    // client waits behind reply work deterministically.
    for (0..terminal_reply_burst_bound) |_| {
        const reply_reservation = try reserveInput(runtime, 3, false, .terminal);
        try publishInput(runtime, reply_reservation, try std.testing.allocator.dupe(u8, "rrr"));
    }
    probe.gate.wait();
    const client_reservation = try reserveInput(runtime, 5, false, .client);
    try publishInput(runtime, client_reservation, try std.testing.allocator.dupe(u8, "aaaaa"));

    // The reply stream is replenished as the writer drains it. Without the
    // reply burst bound the accepted client entry would starve behind the
    // endless stream; with the bound it is delivered after exactly one burst.
    const before_client = try driveReplyBurstUntilClient(
        runtime,
        &probe,
        4 * terminal_reply_burst_bound,
    );
    if (before_client == null) return error.ClientStarvedBehindReplyStream;
    try std.testing.expectEqual(@as(usize, terminal_reply_burst_bound), before_client.?);
    try std.testing.expectEqual(@as(usize, 5), runtime.takeWrittenBytes());
}

test "terminal reply burst enforces the exact yield bound and replies keep priority within the burst" {
    var probe: BurstProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer {
        probe.detached.store(true, .release);
        probe.gate.unpause();
    }

    // The accepted client entry holds the front FIFO position while a reply
    // stream larger than one burst sits behind it. Reserve it first so the
    // writer parks on a reply slice, then publish it before driving.
    const client_reservation = try reserveInput(runtime, 5, false, .client);
    for (0..2 * terminal_reply_burst_bound) |_| {
        const reply_reservation = try reserveInput(runtime, 3, false, .terminal);
        try publishInput(runtime, reply_reservation, try std.testing.allocator.dupe(u8, "rrr"));
    }
    probe.gate.wait();
    try publishInput(runtime, client_reservation, try std.testing.allocator.dupe(u8, "aaaaa"));

    const before_client = try driveReplyBurstUntilClient(
        runtime,
        &probe,
        4 * terminal_reply_burst_bound,
    );
    if (before_client == null) return error.ClientStarvedBehindReplyStream;
    // Exactly one reply burst precedes the client: the writer yields after the
    // bounded slice budget, not after the whole injected stream.
    try std.testing.expectEqual(@as(usize, terminal_reply_burst_bound), before_client.?);
    // Within the burst every written slice is a terminal reply (priority), and
    // the accepted client entry is delivered once the writer yields.
    const written = probe.writtenBytes();
    for (written[0..terminal_reply_burst_bound]) |byte| {
        try std.testing.expectEqual(@as(u8, 'r'), byte);
    }
    try std.testing.expectEqual(@as(usize, 5), runtime.takeWrittenBytes());
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

test "post-exit client closure rejects a prepared client commit and releases its reservation" {
    const runtime = try createRuntimeWithHooks(.{});
    defer runtime.destroy();

    var prepared = try runtime.prepareTextInput("prepared", std.math.maxInt(usize));
    defer prepared.deinit();
    try std.testing.expectEqual(@as(usize, 8), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 1), runtime.client_input_charge.entries);

    // Root exit closes client admission while writer_stop is still false; the
    // previously reserved publication must fail at its one commit boundary.
    closeClientInput(runtime);
    try std.testing.expectError(error.TerminalExited, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    runtime.input_mutex.lock();
    defer runtime.input_mutex.unlock();
    try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.entries);
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}

test "client input publication linearizes against post-exit client closure" {
    // Order 1: publication acquires input_mutex before closure. It is accepted
    // exactly once and stays as the queue's single published client entry.
    {
        var probe: WriterProbe = .{ .pause_at = .copied };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            probe.copied.release();
            clearTestingHooks(runtime);
            runtime.destroy();
        }
        const first = try reserveInput(runtime, 1, false, .client);
        try publishInput(runtime, first, try std.testing.allocator.dupe(u8, "a"));
        probe.copied.wait();
        closeClientInput(runtime);
        runtime.input_mutex.lock();
        try std.testing.expectEqual(@as(usize, 1), runtime.input_entries.items.len);
        try std.testing.expectEqual(@as(usize, 1), runtime.client_input_charge.bytes);
        try std.testing.expectEqual(@as(usize, 1), runtime.client_input_charge.entries);
        runtime.input_mutex.unlock();
    }

    // Order 2: closure acquires input_mutex before publication. The reservation
    // is removed, its byte and entry charge released, and the owned bytes freed.
    {
        const runtime = try createRuntimeWithHooks(.{});
        defer runtime.destroy();
        const second = try reserveInput(runtime, 1, false, .client);
        closeClientInput(runtime);
        try std.testing.expectError(
            error.TerminalExited,
            publishInput(runtime, second, try std.testing.allocator.dupe(u8, "b")),
        );
        runtime.input_mutex.lock();
        try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
        try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
        try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.entries);
        runtime.input_mutex.unlock();
    }
}

test "client admission closure rejects client commits while terminal replies still publish" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    // A terminal-generated reply reservation still publishes after client
    // admission has closed; only client source entries are gated.
    const reply = try reserveInput(runtime, 1, false, .terminal);
    closeClientInput(runtime);
    try publishInput(runtime, reply, try std.testing.allocator.dupe(u8, "r"));
    probe.copied.wait();
    runtime.input_mutex.lock();
    try std.testing.expectEqual(@as(usize, 1), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 1), runtime.terminal_reply_charge.bytes);
    try std.testing.expectEqual(@as(usize, 1), runtime.terminal_reply_charge.entries);
    runtime.input_mutex.unlock();

    // A zero-byte prepared semantic input shares the same one-phase commit gate.
    var noop = try runtime.prepareFocusInput(false, std.math.maxInt(usize));
    defer noop.deinit();
    try std.testing.expectEqual(@as(usize, 0), noop.byteCount());
    try std.testing.expectError(error.TerminalExited, noop.commit());
    runtime.input_mutex.lock();
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.entries);
    runtime.input_mutex.unlock();
}

test "post-exit descendant keeping the PTY open cannot receive a later committed client input" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: DescendantProbe = .{};
    const arguments = [_][]const u8{ "-c", "sleep 30" };
    const runtime = try createRuntimeWithHooksAndCommand(probe.hooks(), "/bin/sh", &arguments);
    defer {
        probe.read_gate.release();
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    // The root child is still running: reserve a client FIFO position and its bytes.
    const descendant_text = "descendant-input";
    var prepared = try runtime.prepareTextInput(descendant_text, std.math.maxInt(usize));
    defer prepared.deinit();
    try std.testing.expectEqual(descendant_text.len, runtime.pendingInputBytes());

    // Root exit closes client admission; the PTY-holding descendant keeps the
    // writer running, so writer_stop stays false.
    try runtime.terminate();
    runtime.input_mutex.lock();
    while (!runtime.child_exited.load(.acquire)) {
        runtime.input_condition.wait(&runtime.input_mutex);
    }
    const writer_still_open = !runtime.writer_stop;
    runtime.input_mutex.unlock();
    try std.testing.expect(writer_still_open);

    // A preparation committed only after child exit is rejected and its bytes
    // never reach the descendant's PTY.
    try std.testing.expectError(error.TerminalExited, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    runtime.input_mutex.lock();
    try std.testing.expectEqual(@as(usize, 0), runtime.input_entries.items.len);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.client_input_charge.entries);
    runtime.input_mutex.unlock();
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
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
    var attachment = try runtime.prepareAttachment(attachment_id, false, true, false);
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

test "paste encoding requires the reviewed bracketed mode under the terminal lock" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectError(
        error.PasteModeChanged,
        runtime.preparePasteInput("payload", true, std.math.maxInt(usize)),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    parseTerminalInput(runtime, "\x1b[?2004h");
    try std.testing.expectError(
        error.PasteModeChanged,
        runtime.preparePasteInput("payload", false, std.math.maxInt(usize)),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var prepared = try runtime.preparePasteInput("payload", true, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[200~payload\x1b[201~", prepared.bytes.?);
    prepared.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "attachment detach repairs only its accepted key and mouse ownership" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    const first_attachment = AttachmentId.fromBytes([_]u8{0x31} ** 16);
    const second_attachment = AttachmentId.fromBytes([_]u8{0x32} ** 16);
    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        var attachment = try runtime.prepareAttachment(attachment_id, true, false, true);
        attachment.commit();
    }
    parseTerminalInput(runtime, "\x1b[=3;1u\x1b[?1002h\x1b[?1006h");

    const first_key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{ .control = true },
        .text = "a",
        .unshifted_codepoint = 'a',
    };
    const second_key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyB",
        .modifiers = .{ .shift = true },
        .text = "B",
        .unshifted_codepoint = 'b',
    };
    const first_mouse: subject.MouseInput = .{
        .action = .{ .press = .left },
        .modifiers = .{ .control = true },
        .pressed_buttons = .{ .left = true },
        .x = 24,
        .y = 32,
    };
    const second_mouse: subject.MouseInput = .{
        .action = .{ .press = .right },
        .modifiers = .{ .shift = true },
        .pressed_buttons = .{ .right = true },
        .x = 40,
        .y = 48,
    };
    for ([_]struct { AttachmentId, protocol.SemanticInput }{
        .{ first_attachment, .{ .key = first_key } },
        .{ first_attachment, .{ .mouse = first_mouse } },
        .{ second_attachment, .{ .key = second_key } },
        .{ second_attachment, .{ .mouse = second_mouse } },
        // Wheel presses are stateless and must not become detach ownership.
        .{ first_attachment, .{ .mouse = .{
            .action = .{ .press = .four },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 24,
            .y = 32,
        } } },
    }) |entry| {
        var prepared = try runtime.prepareAttachmentSemanticInput(
            entry[0],
            entry[1],
            std.math.maxInt(usize),
        );
        _ = try prepared.commit();
    }

    var expected_first: std.ArrayList(u8) = .empty;
    defer expected_first.deinit(std.testing.allocator);
    var first_key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = first_key.composing,
        .consumed_modifiers = first_key.consumed_modifiers,
        .key = first_key.key,
        .modifiers = first_key.modifiers,
        .text = first_key.text,
        .unshifted_codepoint = first_key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected_first.appendSlice(std.testing.allocator, first_key_release.bytes.?);
    first_key_release.deinit();
    var first_mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = first_mouse.modifiers,
        .pressed_buttons = .{},
        .x = first_mouse.x,
        .y = first_mouse.y,
    }, std.math.maxInt(usize));
    try expected_first.appendSlice(std.testing.allocator, first_mouse_release.bytes.?);
    first_mouse_release.deinit();

    var first_repair = try runtime.prepareAttachmentInputRepair(first_attachment);
    defer first_repair.deinit();
    try std.testing.expectEqualSlices(u8, expected_first.items, first_repair.bytes.?);
    _ = try first_repair.commit();
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        first_attachment,
    ));
    try std.testing.expect(subject.testing.hasAttachmentInputOwnership(
        runtime,
        second_attachment,
    ));

    var expected_second: std.ArrayList(u8) = .empty;
    defer expected_second.deinit(std.testing.allocator);
    var second_key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = second_key.composing,
        .consumed_modifiers = second_key.consumed_modifiers,
        .key = second_key.key,
        .modifiers = second_key.modifiers,
        .text = second_key.text,
        .unshifted_codepoint = second_key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected_second.appendSlice(std.testing.allocator, second_key_release.bytes.?);
    second_key_release.deinit();
    var second_mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .right },
        .modifiers = second_mouse.modifiers,
        .pressed_buttons = .{},
        .x = second_mouse.x,
        .y = second_mouse.y,
    }, std.math.maxInt(usize));
    try expected_second.appendSlice(std.testing.allocator, second_mouse_release.bytes.?);
    second_mouse_release.deinit();

    var second_repair = try runtime.prepareAttachmentInputRepair(second_attachment);
    defer second_repair.deinit();
    try std.testing.expectEqualSlices(u8, expected_second.items, second_repair.bytes.?);
    _ = try second_repair.commit();
    try runtime.forceTerminate();
}

test "prepared attachment input cannot commit after its owner detaches" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x33} ** 16);
    var attachment = try runtime.prepareAttachment(attachment_id, true, false, true);
    attachment.commit();

    var prepared = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyA",
            .modifiers = .{},
            .text = "a",
            .unshifted_codepoint = 'a',
        } },
        std.math.maxInt(usize),
    );
    defer prepared.deinit();

    runtime.detachInput(attachment_id);
    try std.testing.expectError(error.MissingAttachment, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "attachment detach repair uses its precharged input capacity" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }
    const attachment_id = AttachmentId.fromBytes([_]u8{0x34} ** 16);
    var attachment = try runtime.prepareAttachment(attachment_id, true, false, true);
    attachment.commit();
    parseTerminalInput(runtime, "\x1b[=3;1u");

    var press = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyA",
            .modifiers = .{},
            .text = "a",
            .unshifted_codepoint = 'a',
        } },
        std.math.maxInt(usize),
    );
    _ = try press.commit();
    probe.copied.wait();

    const queued_before_fill = runtime.pendingInputBytes();
    const fill_size = subject.testing.pending_input_limit -
        subject.max_key_encoded_bytes - queued_before_fill;
    const fill = try std.testing.allocator.alloc(u8, fill_size);
    defer std.testing.allocator.free(fill);
    @memset(fill, 'x');
    var prepared_fill = try runtime.prepareRawInput(fill);
    _ = try prepared_fill.commit();
    try std.testing.expectError(error.InputBackpressure, runtime.prepareRawInput("x"));

    var repair = try runtime.prepareAttachmentInputRepair(attachment_id);
    defer repair.deinit();
    try std.testing.expect(repair.bytes.?.len > 0);
    _ = try repair.commit();
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    probe.copied.release();
    try runtime.forceTerminate();
}

test "attachment detach repair allocation failure fences the live runtime" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x35} ** 16);
    var attachment = try runtime.prepareAttachment(attachment_id, true, false, true);
    attachment.commit();

    // The disabled mouse mode admits no bytes but still owns the accepted
    // held semantic state. Enabling tracking afterward makes detach encoding
    // allocate without leaving an ordinary client entry for the writer.
    var press = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .mouse = .{
            .action = .{ .press = .left },
            .modifiers = .{},
            .pressed_buttons = .{ .left = true },
            .x = 1,
            .y = 1,
        } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 0), try press.commit());
    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");

    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    const runtime_allocator = runtime.allocator;
    runtime.allocator = failing.allocator();
    runtime.detachInput(attachment_id);
    runtime.allocator = runtime_allocator;

    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(
        subject.RuntimeFailureKind.worker_integrity,
        runtime.primaryFailure().?,
    );
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
}

test "finite mouse coordinates remain bounded across tracking modes and formats" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();

    const maximum_finite: f32 = @bitCast(@as(u32, 0x7f7fffff));
    parseTerminalInput(runtime, "\x1b[?1000h");
    var boundary_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[M#p!", boundary_release.bytes.?);
    boundary_release.deinit();

    const clear_modes =
        "\x1b[?1000l\x1b[?1002l\x1b[?1003l" ++
        "\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";
    const tracking_modes = [_][]const u8{
        "\x1b[?1000h",
        "\x1b[?1002h",
        "\x1b[?1003h",
    };
    const formats = [_][]const u8{
        "",
        "\x1b[?1005h",
        "\x1b[?1006h",
        "\x1b[?1015h",
        "\x1b[?1016h",
    };
    const positions = [_][2]f32{
        .{ maximum_finite, 0 },
        .{ -maximum_finite, 0 },
        .{ 0, maximum_finite },
        .{ 0, -maximum_finite },
    };
    for (tracking_modes) |tracking_mode| {
        for (formats) |format| {
            parseTerminalInput(runtime, clear_modes);
            parseTerminalInput(runtime, tracking_mode);
            parseTerminalInput(runtime, format);
            for (positions) |position| {
                var prepared = try runtime.prepareMouseInput(.{
                    .action = .{ .release = .left },
                    .modifiers = .{},
                    .pressed_buttons = .{},
                    .x = position[0],
                    .y = position[1],
                }, std.math.maxInt(usize));
                try std.testing.expect(prepared.byteCount() > 0);
                try std.testing.expect(prepared.byteCount() <= subject.max_mouse_encoded_bytes);
                prepared.deinit();
            }
        }
    }

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1000h");
    var outside_press = try runtime.prepareMouseInput(.{
        .action = .{ .press = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqual(@as(usize, 0), outside_press.byteCount());
    outside_press.deinit();

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1002h\x1b[?1006h");
    var outside_drag = try runtime.prepareMouseInput(.{
        .action = .{ .motion = .{ .drag = .left } },
        .modifiers = .{},
        .pressed_buttons = .{ .left = true },
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[<32;80;1M", outside_drag.bytes.?);
    outside_drag.deinit();

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1000h\x1b[?1016h");
    var representable_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 650,
        .y = 10,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;650;10m",
        representable_pixels.bytes.?,
    );
    representable_pixels.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "finite mouse normalization is safe at maximum legal pixel geometry" {
    const runtime = try createIdleRuntimeWithGeometry(17, 17, .{
        .cell_pixel_height = 3_855,
        .cell_pixel_width = 3_855,
    });
    defer runtime.destroy();
    const maximum_finite: f32 = @bitCast(@as(u32, 0x7f7fffff));

    parseTerminalInput(runtime, "\x1b[?1000h");
    var grid_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[M#11", grid_release.bytes.?);
    grid_release.deinit();

    parseTerminalInput(runtime, "\x1b[?1016h");
    var positive_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;2147483520;2147483520m",
        positive_pixels.bytes.?,
    );
    positive_pixels.deinit();

    var negative_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = -maximum_finite,
        .y = -maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;-2147483648;-2147483648m",
        negative_pixels.bytes.?,
    );
    negative_pixels.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "maximum key text uses the negotiated encoded-byte limit" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    parseTerminalInput(runtime, "\x1b[>24u");

    const a_text = [_]u8{'a'} ** 4096;
    const a_input: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{},
        .text = &a_text,
        .unshifted_codepoint = 'a',
    };
    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.prepareKeyInput(a_input, 12_293),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var prepared_a = try runtime.prepareKeyInput(a_input, 12_294);
    try std.testing.expectEqual(@as(usize, 12_294), prepared_a.byteCount());
    try std.testing.expectEqual(@as(usize, 12_294), runtime.pendingInputBytes());
    prepared_a.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    const d_text = [_]u8{'d'} ** 4096;
    var prepared_d = try runtime.prepareKeyInput(.{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyD",
        .modifiers = .{},
        .text = &d_text,
        .unshifted_codepoint = 'd',
    }, 16_391);
    try std.testing.expectEqual(@as(usize, 16_391), prepared_d.byteCount());
    prepared_d.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    const runtime_allocator = runtime.allocator;
    runtime.allocator = failing.allocator();
    const allocation_result = runtime.prepareKeyInput(a_input, 12_294);
    runtime.allocator = runtime_allocator;
    try std.testing.expectError(error.OutOfMemory, allocation_result);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}

test "every admitted mouse button has a normal SGR encoding" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");

    const cases = [_]struct {
        button: protocol.MouseButton,
        expected: []const u8,
    }{
        .{ .button = .left, .expected = "\x1b[<0;1;1M" },
        .{ .button = .right, .expected = "\x1b[<2;1;1M" },
        .{ .button = .middle, .expected = "\x1b[<1;1;1M" },
        .{ .button = .four, .expected = "\x1b[<64;1;1M" },
        .{ .button = .five, .expected = "\x1b[<65;1;1M" },
        .{ .button = .six, .expected = "\x1b[<66;1;1M" },
        .{ .button = .seven, .expected = "\x1b[<67;1;1M" },
        .{ .button = .eight, .expected = "\x1b[<128;1;1M" },
        .{ .button = .nine, .expected = "\x1b[<129;1;1M" },
        .{ .button = .ten, .expected = "\x1b[<130;1;1M" },
        .{ .button = .eleven, .expected = "\x1b[<131;1;1M" },
    };
    for (cases) |case| {
        var prepared = try runtime.prepareMouseInput(.{
            .action = .{ .press = case.button },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 0,
            .y = 0,
        }, std.math.maxInt(usize));
        try std.testing.expectEqualSlices(u8, case.expected, prepared.bytes.?);
        prepared.deinit();
    }

    const clear_modes =
        "\x1b[?1000l\x1b[?1002l\x1b[?1003l" ++
        "\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";
    const tracking_modes = [_][]const u8{
        "\x1b[?1000h",
        "\x1b[?1002h",
        "\x1b[?1003h",
    };
    const formats = [_][]const u8{
        "",
        "\x1b[?1005h",
        "\x1b[?1006h",
        "\x1b[?1015h",
        "\x1b[?1016h",
    };
    for (tracking_modes) |tracking_mode| {
        for (formats) |format| {
            parseTerminalInput(runtime, clear_modes);
            parseTerminalInput(runtime, tracking_mode);
            parseTerminalInput(runtime, format);
            for ([_]protocol.MouseButton{ .ten, .eleven }) |button| {
                var prepared = try runtime.prepareMouseInput(.{
                    .action = .{ .press = button },
                    .modifiers = .{},
                    .pressed_buttons = .{},
                    .x = 0,
                    .y = 0,
                }, std.math.maxInt(usize));
                try std.testing.expect(prepared.byteCount() > 0);
                prepared.deinit();
            }
        }
    }

    parseTerminalInput(runtime, clear_modes);
    var suppressed = try runtime.prepareMouseInput(.{
        .action = .{ .press = .ten },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 0,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqual(@as(usize, 0), suppressed.byteCount());
    suppressed.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
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

test "raw runtime creation rejects invalid launches before any thread or process effect" {
    const many_arguments = try std.testing.allocator.alloc([]const u8, 4097);
    defer std.testing.allocator.free(many_arguments);
    @memset(many_arguments, "");
    var many_environment: [1025]pty_process.EnvironmentDelta = undefined;
    @memset(&many_environment, .{ .name = "ORCHBEE_BOUND", .value = null });

    const duplicate_names = if (target_builtin.os.tag == .windows)
        [_]pty_process.EnvironmentDelta{
            .{ .name = "Path", .value = "first" },
            .{ .name = "PATH", .value = "second" },
        }
    else
        [_]pty_process.EnvironmentDelta{
            .{ .name = "ORCHBEE_DUPLICATE", .value = "first" },
            .{ .name = "ORCHBEE_DUPLICATE", .value = "second" },
        };

    const InvalidCase = struct {
        expected_error: anyerror,
        launch: pty_process.Launch,
        options: subject.Options,
    };
    const cases = [_]InvalidCase{
        .{ // NUL in the executable: the base runtime accepted and spawned it
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe\x00name",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // columns exceed the protocol frame bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4097,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 1,
            },
            .options = .{},
        },
        .{ // cells exceed the protocol frame bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4096,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 65,
            },
            .options = .{},
        },
        .{ // pixel extents exceed the unsigned 16-bit PTY limit
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 4096,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 1,
            },
            .options = .{ .cell_pixel_width = 16 },
        },
        .{ // scrollback exceeds the configuration maximum
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{ .scrollback_bytes = 1_000_000_001 },
        },
        .{ // NUL in an argument
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{"argument\x00with-nul"},
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in the current directory
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = "/tmp/orchbee\x00fixture",
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in an environment name
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{.{
                    .name = "ORCHBEE\x00NAME",
                    .value = null,
                }},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // NUL in an environment value
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &.{.{
                    .name = "ORCHBEE_VALUE",
                    .value = "bad\x00value",
                }},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // argument count exceeds the protocol launch bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = many_arguments,
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // environment entry count exceeds the protocol launch bound
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &many_environment,
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // duplicate environment names under the target operating system identity
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .columns = 80,
                .current_directory = null,
                .environment = &duplicate_names,
                .executable = "probe",
                .rows = 24,
            },
            .options = .{},
        },
        .{ // launch and options disagree on the cell-pixel width authority
            .expected_error = error.InvalidLaunch,
            .launch = .{
                .arguments = &.{},
                .cell_pixel_width = 9,
                .cell_pixel_height = 18,
                .columns = 80,
                .current_directory = null,
                .environment = &.{},
                .executable = "probe",
                .rows = 24,
            },
            .options = .{ .cell_pixel_width = 10 },
        },
    };

    for (cases) |case| {
        var spawner: CountingThreadSpawner = .{};
        try std.testing.expectError(
            case.expected_error,
            createWithOptionsUsingSpawner(
                std.testing.allocator,
                case.launch,
                case.options,
                &spawner,
            ),
        );
        try std.testing.expectEqual(@as(usize, 0), spawner.attempts);
    }
}

test "raw runtime creation accepts the exact canonical maxima and reaches the process spawn seam" {
    var spawner: FailingThreadSpawner = .{ .fail_at = 3 };
    // Maximum grid columns, exactly 262144 cells, and a cell-pixel geometry
    // whose total extents stay within the unsigned 16-bit PTY limit.
    try std.testing.expectError(
        error.SpawnFailed,
        createWithOptionsUsingSpawner(std.testing.allocator, .{
            .arguments = &.{},
            .cell_pixel_width = 1,
            .cell_pixel_height = 1,
            .columns = 4096,
            .current_directory = null,
            .environment = &.{},
            .executable = "max-geometry-must-reach-process-spawn",
            .rows = 64,
        }, .{}, &spawner),
    );
    try std.testing.expectEqual(@as(usize, 3), spawner.attempts);
}

test "POSIX PTY child reads TIOCGWINSZ matching the canonical launch geometry" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const environment = [_]terminal_launch.EnvironmentEntry{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    const canonical_launch: terminal_launch.Launch = .{
        .arguments = &.{},
        .configuration = null,
        .current_directory = null,
        .environment = &environment,
        .executable = "probe",
        .geometry = .{
            .cell_pixel_height = 18,
            .cell_pixel_width = 9,
            .columns = 120,
            .rows = 40,
        },
        .profile = .{
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
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee test",
        },
    };
    const validated = try terminal_launch.inspect(std.testing.allocator, canonical_launch);
    var prepared = try validated.prepareRuntime(std.testing.allocator, .{});
    defer prepared.deinit(std.testing.allocator);
    const plan = prepared.process_launch;

    const posix = std.posix;
    var pty = try platform_boundary.testing.pty.Pty.open(.{
        .columns = plan.columns,
        .pixel_height = plan.rows * plan.cell_pixel_height,
        .pixel_width = plan.columns * plan.cell_pixel_width,
        .rows = plan.rows,
    });
    defer pty.deinit();
    try pty.setSize(.{
        .columns = plan.columns,
        .pixel_height = plan.rows * plan.cell_pixel_height,
        .pixel_width = plan.columns * plan.cell_pixel_width,
        .rows = plan.rows,
    });

    const TIOCGWINSZ: c_int = comptime if (target_builtin.os.tag == .windows)
        0
    else switch (target_builtin.os.tag) {
        .macos, .freebsd => 0x40087468,
        .linux => 0x5413,
        else => @compileError("TIOCGWINSZ is not defined for this POSIX target"),
    };
    const report = try posix.pipe2(.{ .CLOEXEC = true });
    defer posix.close(report[0]);
    const child_id = try posix.fork();
    if (child_id == 0) {
        posix.close(report[0]);
        pty.childSetup() catch {
            _ = posix.write(report[1], &.{0}) catch 0;
            posix.exit(1);
        };
        var winsize: posix.winsize = undefined;
        if (std.c.ioctl(posix.STDIN_FILENO, TIOCGWINSZ, @intFromPtr(&winsize)) < 0) {
            _ = posix.write(report[1], &.{0}) catch 0;
            posix.exit(1);
        }
        var bytes: [8]u8 = undefined;
        std.mem.writeInt(u16, bytes[0..2], winsize.row, .little);
        std.mem.writeInt(u16, bytes[2..4], winsize.col, .little);
        std.mem.writeInt(u16, bytes[4..6], winsize.xpixel, .little);
        std.mem.writeInt(u16, bytes[6..8], winsize.ypixel, .little);
        _ = posix.write(report[1], &bytes) catch 0;
        posix.exit(0);
    }
    posix.close(report[1]);
    var result: [8]u8 = undefined;
    var received: usize = 0;
    while (received < result.len) {
        const length = try posix.read(report[0], result[received..]);
        if (length == 0) return error.ChildClosedReportPipe;
        received += length;
    }
    _ = posix.waitpid(child_id, 0);
    const row = std.mem.readInt(u16, result[0..2], .little);
    const column = std.mem.readInt(u16, result[2..4], .little);
    const xpixel = std.mem.readInt(u16, result[4..6], .little);
    const ypixel = std.mem.readInt(u16, result[6..8], .little);
    try std.testing.expectEqual(plan.rows, row);
    try std.testing.expectEqual(plan.columns, column);
    try std.testing.expectEqual(@as(u16, plan.columns * plan.cell_pixel_width), xpixel);
    try std.testing.expectEqual(@as(u16, plan.rows * plan.cell_pixel_height), ypixel);
}

fn parseDescendantPid(text: []const u8) ?std.posix.pid_t {
    const marker = "ORCHBEE-DESC ";
    const marker_index = std.mem.indexOf(u8, text, marker) orelse return null;
    const rest = text[marker_index + marker.len ..];
    const digit_end = std.mem.indexOfAny(u8, rest, " \n\r") orelse rest.len;
    return std.fmt.parseInt(std.posix.pid_t, rest[0..digit_end], 10) catch null;
}

fn procCommEquals(pid: std.posix.pid_t, expected: []const u8) bool {
    var path_buffer: [64]u8 = undefined;
    const path = std.fmt.bufPrint(&path_buffer, "/proc/{d}/comm", .{pid}) catch return false;
    const file = std.fs.openFileAbsolute(path, .{}) catch return false;
    defer file.close();
    var comm_buffer: [64]u8 = undefined;
    const count = file.readAll(&comm_buffer) catch return false;
    var end = count;
    while (end > 0 and (comm_buffer[end - 1] == '\n' or comm_buffer[end - 1] == '\r')) end -= 1;
    return std.mem.eql(u8, comm_buffer[0..end], expected);
}

fn processAlive(pid: std.posix.pid_t) bool {
    std.posix.kill(pid, 0) catch |err| switch (err) {
        error.ProcessNotFound => return false,
        else => return true,
    };
    return true;
}

// THCR-028 regression: a session leader that forks a descendant which closes
// its PTY descriptors and writes a forged ") Z\n" task name must not make the
// engine confirm containment while that descendant is still alive. Before
// disposal the descendant is a live, uncontained session member and the leader
// identity stays retained; shutdown kills the descendant, confirms containment,
// and only then releases the leader identity.
test "Linux session descendant with a forged newline comm stays contained until shutdown kills it (THCR-028)" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .linux) return error.SkipZigTest;

    // The leader forks one background subshell that closes its PTY descriptors,
    // writes the forged "worker) Z\nX" main-thread comm (the embedded newline is
    // not trailing, so /proc/self/comm keeps it), and blocks forever reading
    // /dev/zero; the leader echoes the descendant PID and exits. On the pre-fix
    // parser the single fgets line reads as a non-live 'Z'.
    const script =
        "( printf 'worker) Z\\nX' >/proc/self/comm; exec 0</dev/zero 1>&- 2>&-; read x ) & " ++
        "printf 'ORCHBEE-DESC %s\\n' \"$!\"; exit 0";
    const arguments = [_][]const u8{ "-c", script };
    const runtime = try TerminalRuntime.createWithOptions(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    }, .{});
    defer runtime.destroy();

    const completion = try runtime.waitForCompletion();
    try std.testing.expect(completion.pty_drained);

    var snapshot = try canonicalSnapshot(runtime);
    defer snapshot.deinit();
    const descendant_pid = parseDescendantPid(snapshot.text) orelse
        return error.DescendantPidMissing;
    // Best-effort cleanup: on the pre-fix parser containment is spoof-confirmed,
    // so a failed shutdown would leave the descendant running. Ensure it dies.
    defer {
        if (processAlive(descendant_pid)) {
            std.posix.kill(descendant_pid, std.posix.SIG.KILL) catch {};
        }
    }

    // Wait until the descendant has actually written the forged comm; only that
    // state exercises the spoof on the pre-fix parser.
    var forged_comm = false;
    for (0..1000) |_| {
        if (procCommEquals(descendant_pid, "worker) Z\nX")) {
            forged_comm = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(forged_comm);

    // The descendant is a live, uncontained session member: containment must
    // not be confirmed and the leader identity must remain retained.
    try std.testing.expect(processAlive(descendant_pid));
    try std.testing.expect(!runtime.containment_confirmed.load(.acquire));
    try std.testing.expect(!try runtime.process.processTreeContained());
    try std.testing.expect(!runtime.process.identityReleased());

    // Shutdown force-terminates the session, confirms containment, and only
    // then releases the retained leader identity.
    try runtime.shutdown();
    try std.testing.expect(runtime.containment_confirmed.load(.acquire));
    try std.testing.expect(runtime.process.identityReleased());
    try std.testing.expect(try runtime.process.processTreeContained());

    // The descendant was killed and, once the leader identity is released, is
    // reparented and reaped by init so it is fully gone rather than stopped.
    var descendant_gone = false;
    for (0..1000) |_| {
        if (!processAlive(descendant_pid)) {
            descendant_gone = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(descendant_gone);
}
