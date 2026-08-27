pub const std = @import("std");
pub const target_builtin = @import("builtin");
pub const ghostty_vt = @import("ghostty-vt");
pub const engine_boundary = @import("terminal-host-engine");
pub const model_boundary = @import("terminal-host-model");
pub const platform_boundary = @import("terminal-host-platform");
pub const protocol = @import("terminal-protocol");
pub const semantic_wait = @import("semantic_wait.zig");
pub const subject = engine_boundary.runtime;
pub const terminal_launch = engine_boundary.launch;
pub const pty_process = platform_boundary.process;
pub const thread_sync = platform_boundary.thread_sync;

pub const applyScrollbackBytes = subject.testing.applyScrollbackBytesFn;
pub const scrollbackCompressionIdleIntervalNs =
    subject.testing.scrollbackCompressionIdleIntervalNs;
pub const scrollbackMemoryStats = subject.testing.scrollbackMemoryStats;
pub const tickAnimationLockedAt = subject.testing.tickAnimationLockedAtFn;
pub const cancelInput = subject.testing.cancelInputFn;
pub const clearTestingHooks = subject.testing.clearTestingHooks;
pub const closeClientInput = subject.testing.closeClientInputFn;
pub const commitReaderBatch = subject.testing.commitReaderBatchFn;
pub const completionClassificationLoad = subject.testing.completionClassificationLoadFn;
pub const createWithOptionsUsingSpawner = subject.testing.createWithOptionsUsingSpawnerFn;
pub const deinitOwnedProfile = subject.testing.deinitOwnedProfileFn;
pub const FirstContainmentAttempt = subject.testing.FirstContainmentAttemptType;
pub const InputReservation = subject.testing.InputReservationType;
pub const InputSource = subject.testing.InputSourceType;
pub const initOwnedProfile = subject.testing.initOwnedProfileFn;
pub const inputQueueSnapshot = subject.testing.inputQueueSnapshot;
pub const modifiers = subject.testing.modifiersFn;
pub const mouseEncodeEvent = subject.testing.mouseEncodeEventFn;
pub const mouseHasPressedButton = subject.testing.mouseHasPressedButtonFn;
pub const markNormalCompletionReady = subject.testing.markNormalCompletionReadyFn;
pub const ProjectionEncodeRequest = subject.ProjectionEncodeRequest;
pub const AttachmentId = subject.AttachmentId;
pub const HostEpoch = subject.HostEpoch;
pub const reserveInput = subject.testing.reserveInputFn;
pub const TerminalRuntime = subject.TerminalRuntime;
pub const runtimeState = subject.testing.stateForTesting;
pub const resolveCloseActivity = subject.testing.resolveCloseActivityFn;
pub const max_pending_input_bytes = subject.testing.pending_input_limit;
pub const max_pending_input_entries = subject.testing.pending_input_entry_limit;
pub const max_pending_terminal_reply_bytes = subject.testing.pending_terminal_reply_limit;
pub const max_writer_slice_bytes = subject.testing.writer_slice_limit;
// The terminal-reply burst bound the writer enforces before yielding to a
// waiting accepted client entry. It mirrors max_terminal_reply_burst_slices in
// runtime/writer.zig and is the contract value documented in runtime-model.md.
pub const terminal_reply_burst_bound: usize = 64;
pub const publishInput = subject.testing.publishInputFn;
pub const recordReaderFailure = subject.testing.recordReaderFailureFn;
pub const recordTerminalReplyFailure = subject.testing.recordTerminalReplyFailure;
pub const recordWorkerIntegrityFailure = subject.testing.recordWorkerIntegrityFailureFn;
pub const recordWriterFailure = subject.testing.recordWriterFailureFn;
pub const releaseOutputForShutdown = subject.testing.releaseOutputForShutdown;
pub const ReaderBatchOutcome = subject.testing.ReaderBatchOutcomeType;
pub const resizePtyWithRollback = subject.testing.resizePtyWithRollbackFn;
pub const RuntimeTestingHooks = subject.testing.RuntimeTestingHooksType;
pub const Geometry = model_boundary.geometry.Geometry;

pub fn requireProjectionFrame(
    outcome: subject.ProjectionEncodeOutcome,
) !subject.ProjectionFrame {
    return switch (outcome) {
        .frame => |frame| frame,
        .blocked => error.UnexpectedProjectionBlock,
    };
}

pub const TestGate = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    reached: bool = false,
    released: bool = false,

    pub fn pause(self: *TestGate) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        while (!self.released) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    pub fn wait(self: *TestGate) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    pub fn release(self: *TestGate) void {
        self.mutex.lock();
        self.released = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

pub const TestSignal = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    reached: bool = false,

    pub fn notify(self: *TestSignal) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    pub fn wait(self: *TestSignal) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }
};

pub fn exerciseOwnedProfileAllocations(allocator: std.mem.Allocator) !void {
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

pub const FailingThreadSpawner = struct {
    fail_at: usize,
    attempts: usize = 0,

    pub fn spawn(
        self: *FailingThreadSpawner,
        comptime thread_function: anytype,
        runtime: anytype,
    ) !std.Thread {
        const attempt = self.attempts;
        self.attempts += 1;
        if (attempt == self.fail_at) return error.InjectedThreadSpawnFailure;
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

pub const CountingThreadSpawner = struct {
    attempts: usize = 0,

    pub fn spawn(
        self: *CountingThreadSpawner,
        comptime thread_function: anytype,
        runtime: anytype,
    ) !std.Thread {
        self.attempts += 1;
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

pub const HookedThreadSpawner = struct {
    hooks: RuntimeTestingHooks,

    pub fn terminalRuntimeTestingHooks(self: *HookedThreadSpawner) RuntimeTestingHooks {
        return self.hooks;
    }

    pub fn spawn(
        _: *HookedThreadSpawner,
        comptime thread_function: anytype,
        runtime: anytype,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

pub fn createRuntimeWithHooks(hooks: RuntimeTestingHooks) !*TerminalRuntime {
    const builtin = @import("builtin");
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    if (builtin.os.tag == .windows) {
        const executable = try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec");
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

pub fn createRuntimeWithHooksAndCommand(
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

pub fn waitForWriterStop(runtime: *TerminalRuntime) void {
    runtimeState(runtime).input_mutex.lock();
    while (!runtimeState(runtime).writer_stopped.load(.acquire)) {
        runtimeState(runtime).input_condition.wait(&runtimeState(runtime).input_mutex);
    }
    runtimeState(runtime).input_mutex.unlock();
}

pub fn createIdleRuntime(options: subject.Options) !*TerminalRuntime {
    return createIdleRuntimeWithGeometry(80, 24, options);
}

/// Completes the production-defined retry path for a transient POSIX reap
/// miss, then destroys the fully quiesced runtime without hiding any other
/// shutdown failure from the test runner.
pub fn destroyRuntimeWithIdentityRetry(runtime: *TerminalRuntime) void {
    runtime.terminate() catch |err| {
        std.debug.panic("terminal runtime graceful termination failed: {s}", .{@errorName(err)});
    };
    for (0..500) |_| {
        if (runtimeState(runtime).child_exited.load(.acquire)) break;
        platform_boundary.monotonic.sleep(10 * std.time.ns_per_ms);
    }

    const maximum_attempts = 8;
    for (0..maximum_attempts) |attempt| {
        runtime.shutdown() catch |err| switch (err) {
            error.ProcessIdentityReleaseFailed => {
                if (attempt + 1 == maximum_attempts) {
                    std.debug.panic("terminal runtime identity release did not converge", .{});
                }
                // Match the production cleanup scheduler's quiet interval so
                // Darwin can drain the prior EVFILT_PROC reference before the
                // next non-reaping observation attempt.
                platform_boundary.monotonic.sleep(std.time.ns_per_s);
                continue;
            },
            else => std.debug.panic("terminal runtime shutdown failed: {s}", .{@errorName(err)}),
        };
        runtime.destroyAfterShutdown();
        return;
    }
    unreachable;
}

pub fn createImmediateExitRuntime() !*TerminalRuntime {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "exit 0" };
    const windows_arguments = [_][]const u8{ "/D", "/Q", "/C", "exit /B 0" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    return TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
}

pub fn createIdleRuntimeWithGeometry(
    columns: u16,
    rows: u16,
    options: subject.Options,
) !*TerminalRuntime {
    return createIdleRuntimeWithGeometryAndHooks(columns, rows, options, .{});
}

pub fn createIdleRuntimeWithGeometryAndHooks(
    columns: u16,
    rows: u16,
    options: subject.Options,
    hooks: RuntimeTestingHooks,
) !*TerminalRuntime {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 120" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 121 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    return createWithOptionsUsingSpawner(std.testing.allocator, .{
        .arguments = arguments,
        .columns = columns,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = rows,
    }, options, &spawner);
}

pub fn parseTerminalInput(runtime: *TerminalRuntime, bytes: []const u8) void {
    runtimeState(runtime).terminal_mutex.lock();
    defer runtimeState(runtime).terminal_mutex.unlock();
    runtimeState(runtime).stream.nextSlice(bytes);
}

pub const WriterPause = enum {
    copied,
    started,
};

pub const WriterProbe = struct {
    accounted: TestSignal = .{},
    copied: TestGate = .{},
    mutex: thread_sync.Mutex = .{},
    pause_at: WriterPause,
    retired: TestSignal = .{},
    started: TestGate = .{},
    write_bytes: usize = 0,
    write_calls: usize = 0,

    pub fn fromContext(context: *anyopaque) *WriterProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn sliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        if (self.pause_at == .copied) self.copied.pause();
    }

    pub fn sliceStarted(context: *anyopaque) void {
        const self = fromContext(context);
        if (self.pause_at == .started) self.started.pause();
    }

    pub fn sliceAccounted(context: *anyopaque) void {
        fromContext(context).accounted.notify();
    }

    pub fn sliceRetired(context: *anyopaque) void {
        fromContext(context).retired.notify();
    }

    pub fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.mutex.unlock();
        return bytes.len;
    }

    pub fn hooks(self: *WriterProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .write_pty = write,
            .writer_slice_accounted = sliceAccounted,
            .writer_slice_copied = sliceCopied,
            .writer_slice_retired = sliceRetired,
            .writer_slice_started = sliceStarted,
        };
    }

    pub fn counts(self: *WriterProbe) struct { bytes: usize, calls: usize } {
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
pub const RepeatingGate = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    generation: usize = 0,
    last_returned: usize = 0,
    release: bool = false,

    pub fn pause(self: *RepeatingGate) void {
        self.mutex.lock();
        self.release = false;
        self.generation += 1;
        self.condition.broadcast();
        while (!self.release) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    pub fn wait(self: *RepeatingGate) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.release or self.generation == self.last_returned) {
            self.condition.wait(&self.mutex);
        }
        self.last_returned = self.generation;
    }

    pub fn unpause(self: *RepeatingGate) void {
        self.mutex.lock();
        self.release = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

/// Drives the writer one slice at a time and records the first byte of every
/// written slice, so tests can count the terminal-reply slices that precede a
/// client slice and assert the bounded reply burst without any sleeps.
pub const BurstProbe = struct {
    gate: RepeatingGate = .{},
    // Once set, the writer stops pausing per slice and drains freely so the
    // test can end without a gate deadlock at shutdown.
    detached: std.atomic.Value(bool) = .init(false),
    mutex: thread_sync.Mutex = .{},
    written: [512]u8 = undefined,
    write_count: usize = 0,

    pub fn fromContext(context: *anyopaque) *BurstProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn sliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        if (!self.detached.load(.acquire)) self.gate.pause();
    }

    pub fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.write_count < self.written.len) {
            self.written[self.write_count] = bytes[0];
        }
        self.write_count += 1;
        return bytes.len;
    }

    pub fn hooks(self: *BurstProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .write_pty = write,
            .writer_slice_copied = sliceCopied,
        };
    }

    /// First byte of the most recently written slice, or null when no slice has
    /// been written yet.
    pub fn lastWritten(self: *BurstProbe) ?u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.write_count == 0) return null;
        return self.written[self.write_count - 1];
    }

    /// First bytes of every written slice so far.
    pub fn writtenBytes(self: *BurstProbe) []const u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.written[0..self.write_count];
    }
};

/// Simulates a root-exit descendant that keeps the PTY slave open: the reader
/// blocks instead of observing EOF, so pty_drained stays false and writer_stop
/// stays false even after child_exited has been published. Releasing the gate
/// returns EOF so the runtime can drain and shut down normally.
pub const DescendantProbe = struct {
    read_gate: TestGate = .{},
    write_bytes: usize = 0,
    write_calls: usize = 0,
    write_mutex: thread_sync.Mutex = .{},

    pub fn fromContext(context: *anyopaque) *DescendantProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn read(context: *anyopaque, _: []u8) anyerror!usize {
        const self = fromContext(context);
        self.read_gate.pause();
        return 0;
    }

    pub fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.write_mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.write_mutex.unlock();
        return bytes.len;
    }

    pub fn hooks(self: *DescendantProbe) RuntimeTestingHooks {
        return .{ .context = self, .read_pty = read, .write_pty = write };
    }
};

pub const ReaderProbe = struct {
    batch: []const u8,
    batch_finished: TestGate = .{},
    batch_read: TestGate = .{},
    mutex: thread_sync.Mutex = .{},
    outcome: ?ReaderBatchOutcome = null,
    read_calls: usize = 0,
    stopped: TestSignal = .{},

    pub fn fromContext(context: *anyopaque) *ReaderProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn read(context: *anyopaque, buffer: []u8) anyerror!usize {
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

    pub fn abortRead(_: *anyopaque) anyerror!void {}

    pub fn batchFinished(context: *anyopaque, outcome: ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.outcome = outcome;
        self.mutex.unlock();
        self.batch_finished.pause();
    }

    pub fn readerStopped(context: *anyopaque) void {
        fromContext(context).stopped.notify();
    }

    pub fn hooks(self: *ReaderProbe) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .context = self,
            .read_pty = read,
            .reader_batch_finished = batchFinished,
            .reader_stopped = readerStopped,
        };
    }

    pub fn observedOutcome(self: *ReaderProbe) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome;
    }

    pub fn readCallCount(self: *ReaderProbe) usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.read_calls;
    }
};

pub const PtyDrainRaceProbe = struct {
    before_publication: TestGate = .{},
    reader_stopped: TestSignal = .{},

    pub fn fromContext(context: *anyopaque) *PtyDrainRaceProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn read(_: *anyopaque, _: []u8) anyerror!usize {
        return 0;
    }

    pub fn noOp(_: *anyopaque) anyerror!void {}

    pub fn publicationStarting(context: *anyopaque) void {
        fromContext(context).before_publication.pause();
    }

    pub fn readerStopped(context: *anyopaque) void {
        fromContext(context).reader_stopped.notify();
    }

    pub fn hooks(self: *PtyDrainRaceProbe) RuntimeTestingHooks {
        return .{
            .abort_read = noOp,
            .context = self,
            .finish_pty_after_child_exit = noOp,
            .pty_drain_publication_starting = publicationStarting,
            .read_pty = read,
            .reader_stopped = readerStopped,
        };
    }
};

pub const CanonicalSnapshot = struct {
    generation: u64,
    output_batches: u64,
    revision: u64,
    text: []const u8,
    total_rows: usize,
    written_bytes: usize,

    pub fn deinit(self: *CanonicalSnapshot) void {
        std.testing.allocator.free(self.text);
        self.* = undefined;
    }
};

pub fn canonicalSnapshot(runtime: *TerminalRuntime) !CanonicalSnapshot {
    runtimeState(runtime).terminal_mutex.lock();
    defer runtimeState(runtime).terminal_mutex.unlock();
    const screen = runtimeState(runtime).terminal.screens.get(runtimeState(runtime).terminal.screens.active_key).?;
    return .{
        .generation = runtimeState(runtime).generation,
        .output_batches = runtimeState(runtime).output_batches_since_sync.load(.acquire),
        .revision = runtimeState(runtime).revision.load(.monotonic),
        .text = try runtimeState(runtime).terminal.plainString(std.testing.allocator),
        .total_rows = screen.pages.total_rows,
        .written_bytes = runtimeState(runtime).written_since_sync.load(.acquire),
    };
}

pub fn expectCanonicalSnapshotEqual(
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

pub fn readFrameInt(comptime Integer: type, bytes: []const u8, offset: usize) !Integer {
    const width = @sizeOf(Integer);
    if (offset > bytes.len -| width) return error.TestUnexpectedFrame;
    return std.mem.readInt(Integer, bytes[offset..][0..width], .little);
}

pub fn projectionFrameRowOffset(bytes: []const u8) !usize {
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

pub fn expectIncludedProjectionRows(bytes: []const u8, expected: []const u16) !void {
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

pub const FrameHyperlinkObservation = struct {
    any: bool,
    target: bool,
};

pub fn observeFrameHyperlinks(
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

pub fn writeProjectionOutput(runtime: *TerminalRuntime, bytes: []const u8) !u64 {
    runtimeState(runtime).terminal_mutex.lock();
    defer runtimeState(runtime).terminal_mutex.unlock();
    runtimeState(runtime).stream.nextSlice(bytes);
    const revision = std.math.add(u64, runtimeState(runtime).revision.load(.monotonic), 1) catch
        return error.TestUnexpectedRevisionOverflow;
    runtimeState(runtime).revision.store(revision, .release);
    return revision;
}

pub fn expectProjectionRowRevisions(runtime: *TerminalRuntime, expected: []const u64) !void {
    runtimeState(runtime).projection_mutex.lock();
    defer runtimeState(runtime).projection_mutex.unlock();
    try std.testing.expectEqualSlices(u64, expected, runtimeState(runtime).projection.row_revisions.items);
}

pub fn expectProjectionLinkTargets(runtime: *TerminalRuntime, expected: []const []const u8) !void {
    runtimeState(runtime).projection_mutex.lock();
    defer runtimeState(runtime).projection_mutex.unlock();
    try std.testing.expectEqual(expected.len, runtimeState(runtime).projection.link_table.values.items.len);
    for (expected, runtimeState(runtime).projection.link_table.values.items) |expected_target, actual_target| {
        try std.testing.expectEqualStrings(expected_target, actual_target);
    }
}

pub fn expectInputQueuesCleared(runtime: *TerminalRuntime) !void {
    const snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), snapshot.entry_count);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.entries);
    try std.testing.expectEqual(@as(usize, 0), snapshot.terminal_reply_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), snapshot.terminal_reply_charge.entries);
}

pub const ContainmentProbe = struct {
    abort_calls: usize = 0,
    contain_calls: usize = 0,
    first_attempt: TestGate = .{},
    host_waiting: TestSignal = .{},
    mutex: thread_sync.Mutex = .{},
    pause_first_attempt: bool,

    pub fn fromContext(context: *anyopaque) *ContainmentProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.abort_calls += 1;
        self.mutex.unlock();
    }

    pub fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.contain_calls += 1;
        const call = self.contain_calls;
        self.mutex.unlock();
        if (self.pause_first_attempt and call == 1) self.first_attempt.pause();
        return error.InjectedContainmentFailure;
    }

    pub fn hostWaiting(context: *anyopaque) void {
        fromContext(context).host_waiting.notify();
    }

    pub fn hooks(self: *ContainmentProbe) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .contain_process_tree = contain,
            .context = self,
            .host_containment_waiting = hostWaiting,
        };
    }

    pub fn counts(self: *ContainmentProbe) struct { aborts: usize, contains: usize } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{ .aborts = self.abort_calls, .contains = self.contain_calls };
    }
};

pub const FailureReporter = struct {
    const Kind = enum { reader, writer };

    done: TestSignal = .{},
    kind: Kind,
    runtime: *TerminalRuntime,

    pub fn run(self: *FailureReporter) void {
        switch (self.kind) {
            .reader => recordReaderFailure(self.runtime),
            .writer => recordWriterFailure(self.runtime),
        }
        self.done.notify();
    }
};

pub const ShutdownProbe = struct {
    done: TestSignal = .{},
    failure: ?anyerror = null,
    mutex: thread_sync.Mutex = .{},
    runtime: *TerminalRuntime,

    pub fn run(self: *ShutdownProbe) void {
        self.runtime.shutdown() catch |err| {
            self.mutex.lock();
            self.failure = err;
            self.mutex.unlock();
            self.done.notify();
            return;
        };
        self.done.notify();
    }

    pub fn observedFailure(self: *ShutdownProbe) ?anyerror {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.failure;
    }
};

pub const OutputCleanupProbe = struct {
    abort_calls: usize = 0,
    abort_fails: bool,
    batch: []const u8 = "cleanup-reader-batch-must-be-discarded",
    finish_calls: usize = 0,
    finish_fails: bool,
    mutex: thread_sync.Mutex = .{},
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

    pub fn fromContext(context: *anyopaque) *OutputCleanupProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.abort_calls += 1;
        const fails = self.abort_fails;
        self.mutex.unlock();
        if (fails) return error.InjectedAbortReadFailure;
    }

    pub fn finishPty(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        self.finish_calls += 1;
        const fails = self.finish_fails;
        self.mutex.unlock();
        if (fails) return error.InjectedFinishPtyFailure;
    }

    pub fn read(context: *anyopaque, buffer: []u8) anyerror!usize {
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

    pub fn readerBatchFinished(context: *anyopaque, outcome: ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.reader_outcome = outcome;
        self.mutex.unlock();
        self.reader_batch_finished.notify();
    }

    pub fn readerStopped(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.reader_stop_notifications += 1;
        self.mutex.unlock();
        self.reader_stopped.notify();
    }

    pub fn writerSliceCopied(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.writer_copy_notifications += 1;
        const notification = self.writer_copy_notifications;
        self.mutex.unlock();
        if (notification == 1) self.writer_slice_copied.pause();
    }

    pub fn writerSliceStarted(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.writer_start_notifications += 1;
        self.mutex.unlock();
    }

    pub fn write(context: *anyopaque, bytes: []const u8) anyerror!usize {
        const self = fromContext(context);
        self.mutex.lock();
        self.write_calls += 1;
        self.write_bytes += bytes.len;
        self.mutex.unlock();
        return bytes.len;
    }

    pub fn hooks(self: *OutputCleanupProbe) RuntimeTestingHooks {
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

    pub fn allowCleanupRetry(self: *OutputCleanupProbe) void {
        self.mutex.lock();
        self.abort_fails = false;
        self.finish_fails = false;
        self.mutex.unlock();
    }

    pub fn counts(self: *OutputCleanupProbe) struct {
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

    pub fn observedReaderOutcome(self: *OutputCleanupProbe) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.reader_outcome;
    }
};

pub const ResizeEffectOutcome = enum {
    commit,
    compensated_error,
};

pub const ResizeProbe = struct {
    effect_calls: usize = 0,
    mutex: thread_sync.Mutex = .{},
    outcome: ResizeEffectOutcome,
    post_effect: TestGate = .{},
    post_effect_notifications: usize = 0,

    pub fn fromContext(context: *anyopaque) *ResizeProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn resize(
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

    pub fn effectReturned(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.post_effect_notifications += 1;
        const notification = self.post_effect_notifications;
        self.mutex.unlock();
        std.debug.assert(notification == 1);
        self.post_effect.pause();
    }

    pub fn hooks(self: *ResizeProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .resize_effect_returned = effectReturned,
            .resize_pty = resize,
        };
    }

    pub fn counts(self: *ResizeProbe) struct { effects: usize, post_effects: usize } {
        self.mutex.lock();
        defer self.mutex.unlock();
        return .{
            .effects = self.effect_calls,
            .post_effects = self.post_effect_notifications,
        };
    }
};

pub const ResizeObservation = struct {
    previous: Geometry,
    next: Geometry,
};

pub const TerminalDrivenResizeProbe = struct {
    calls: [4]ResizeObservation = undefined,
    count: usize = 0,
    fail: bool = false,
    mutex: thread_sync.Mutex = .{},

    pub fn fromContext(context: *anyopaque) *TerminalDrivenResizeProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn resize(
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

    pub fn hooks(self: *TerminalDrivenResizeProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .resize_pty = resize,
        };
    }

    pub fn observedCalls(self: *TerminalDrivenResizeProbe) []const ResizeObservation {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.calls[0..self.count];
    }

    pub fn clearObservedCalls(self: *TerminalDrivenResizeProbe) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.count = 0;
    }
};

pub const TerminalResizeSerializationProbe = struct {
    calls: [4]ResizeObservation = undefined,
    count: usize = 0,
    effect_returns: usize = 0,
    first_effect_returned: TestGate = .{},
    mutex: thread_sync.Mutex = .{},
    reader_commit_waiting: TestSignal = .{},

    pub fn fromContext(context: *anyopaque) *TerminalResizeSerializationProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn resize(
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

    pub fn effectReturned(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        self.effect_returns += 1;
        const effect_return = self.effect_returns;
        self.mutex.unlock();
        if (effect_return == 1) self.first_effect_returned.pause();
    }

    pub fn readerCommitWaiting(context: *anyopaque) void {
        fromContext(context).reader_commit_waiting.notify();
    }

    pub fn hooks(self: *TerminalResizeSerializationProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .reader_commit_waiting = readerCommitWaiting,
            .resize_effect_returned = effectReturned,
            .resize_pty = resize,
        };
    }

    pub fn observedCalls(self: *TerminalResizeSerializationProbe) []const ResizeObservation {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.calls[0..self.count];
    }
};

pub const ResizeCall = struct {
    const Result = union(enum) {
        failed: anyerror,
        resized: u64,
    };

    done: TestSignal = .{},
    mutex: thread_sync.Mutex = .{},
    result: ?Result = null,
    runtime: *TerminalRuntime,

    pub fn run(self: *ResizeCall) void {
        const result: Result = if (self.runtime.resizeGeometry(100, 30, 9, 18)) |revision|
            .{ .resized = revision }
        else |err|
            .{ .failed = err };
        self.mutex.lock();
        self.result = result;
        self.mutex.unlock();
        self.done.notify();
    }

    pub fn observedResult(self: *ResizeCall) Result {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.result.?;
    }
};

pub const ReaderBatchCall = struct {
    done: TestSignal = .{},
    mutex: thread_sync.Mutex = .{},
    outcome: ?ReaderBatchOutcome = null,
    runtime: *TerminalRuntime,

    pub fn run(self: *ReaderBatchCall) void {
        const outcome = commitReaderBatch(
            self.runtime,
            "\x1b[?40h\x1b[?3h",
        );
        self.mutex.lock();
        self.outcome = outcome;
        self.mutex.unlock();
        self.done.notify();
    }

    pub fn observedOutcome(self: *ReaderBatchCall) ?ReaderBatchOutcome {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.outcome;
    }
};

// THCR-041 regression: a Windows child that terminates with exit status 259
// (STILL_ACTIVE's numeric value) must classify as a normal engine completion
// carrying exit_code 259 with the child exited, PTY drained, and writer fully
// quiesced. It must never be treated as a process-observation failure. Native
// Windows execution required; compiled for `x86_64-windows-gnu` on non-Windows.

pub fn expectRealPtyQueryReplies(
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

/// Drives the writer one slice at a time, parking after every slice copy so
/// every write is observed deterministically. A terminal reply is replenished
/// after every reply slice, so the reply stream never runs dry while the
/// accepted client entry waits. Returns the number of reply slices written
/// before the client slice, or null when the client entry was never written
/// within `maximum_reply_slices` (unbounded starvation).
pub fn driveReplyBurstUntilClient(
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

pub fn parseDescendantPid(text: []const u8) ?std.posix.pid_t {
    const marker = "ORCHBEE-DESC ";
    const marker_index = std.mem.indexOf(u8, text, marker) orelse return null;
    const rest = text[marker_index + marker.len ..];
    const digit_end = std.mem.indexOfAny(u8, rest, " \n\r") orelse rest.len;
    return std.fmt.parseInt(std.posix.pid_t, rest[0..digit_end], 10) catch null;
}

pub fn procCommEquals(pid: std.posix.pid_t, expected: []const u8) bool {
    var path_buffer: [64]u8 = undefined;
    const path = std.fmt.bufPrint(&path_buffer, "/proc/{d}/comm", .{pid}) catch return false;
    const io = thread_sync.io();
    const file = std.Io.Dir.openFileAbsolute(io, path, .{}) catch return false;
    defer file.close(io);
    var comm_buffer: [64]u8 = undefined;
    const count = file.readPositionalAll(io, &comm_buffer, 0) catch return false;
    var end = count;
    while (end > 0 and (comm_buffer[end - 1] == '\n' or comm_buffer[end - 1] == '\r')) end -= 1;
    return std.mem.eql(u8, comm_buffer[0..end], expected);
}

pub fn processAlive(pid: std.posix.pid_t) bool {
    std.posix.kill(pid, @enumFromInt(0)) catch |err| switch (err) {
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
