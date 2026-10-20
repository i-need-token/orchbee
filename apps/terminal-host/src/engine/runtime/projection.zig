const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const read_commit_guard = @import("read_commit_guard.zig");
const failure_ops = @import("failure.zig");
const mode_effects = @import("mode_effects.zig");
const process_io_ops = @import("process_io.zig");

pub fn bellSequence(self: *state.State) u64 {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return self.bell_sequence;
}

pub fn acquireReadCommitGuard(
    self: *state.State,
    expected_close_activity_epoch: u64,
    expected_close_activity_epoch_exhausted: bool,
    expected_generation: u64,
    expected_read_commit_epoch: u64,
) contracts.ProjectionEncodeError!read_commit_guard.Internal {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.read_commit_checkpoint) |notify| {
            notify(self.testing_hooks.context.?, .before_lock);
        }
    }
    self.projection_mutex.lock();
    errdefer self.projection_mutex.unlock();
    self.terminal_mutex.lock();
    errdefer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    if (self.close_activity_epoch != expected_close_activity_epoch or
        self.close_activity_epoch_exhausted != expected_close_activity_epoch_exhausted or
        self.generation != expected_generation or
        self.read_commit_epoch != expected_read_commit_epoch)
    {
        return error.TerminalRuntimeUnavailable;
    }
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.read_commit_checkpoint) |notify| {
            notify(self.testing_hooks.context.?, .locked);
        }
    }
    return .{ .runtime = self };
}

pub fn plainString(self: *state.State, allocator: std.mem.Allocator) ![]const u8 {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    return self.terminal.plainString(allocator);
}

pub fn captureScrollbackText(
    self: *state.State,
    allocator: std.mem.Allocator,
    maximum_bytes: usize,
) !contracts.ScrollbackTextCapture {
    if (maximum_bytes == 0 or maximum_bytes > contracts.max_scrollback_text_bytes) {
        return error.InvalidCaptureBudget;
    }
    // Reserve storage before locking. One extra byte distinguishes an exact fit.
    const storage = try allocator.alloc(u8, maximum_bytes + 1);
    errdefer allocator.free(storage);
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
    const terminal = &self.terminal;
    var writer: std.Io.Writer = .fixed(storage);
    var formatter: ghostty_vt.formatter.ScreenFormatter = .init(
        terminal.screens.active,
        .{ .emit = .plain, .trim = true, .unwrap = true },
    );
    formatter.format(&writer) catch return error.ScrollbackTextTooLarge;
    if (writer.end > maximum_bytes) return error.ScrollbackTextTooLarge;
    return .{
        .allocator = allocator,
        .storage = storage,
        .byte_length = writer.end,
        .geometry = .{
            .cell_pixel_height = self.cell_pixel_height,
            .cell_pixel_width = self.cell_pixel_width,
            .columns = terminal.cols,
            .generation = self.generation,
            .revision = self.revision.load(.monotonic),
            .rows = terminal.rows,
        },
        .screen = if (terminal.screens.active_key == .primary) .primary else .alternate,
        .total_rows = terminal.screens.active.pages.total_rows,
    };
}

pub fn geometry(self: *state.State) contracts.Geometry {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = self.terminal.cols,
        .generation = self.generation,
        .revision = self.revision.load(.monotonic),
        .rows = self.terminal.rows,
    };
}

pub fn projectionCounters(self: *state.State) contracts.ProjectionCounters {
    self.projection_mutex.lock();
    defer self.projection_mutex.unlock();
    return self.projection.counters;
}

pub fn nextProjectionAt(self: *const state.State) i128 {
    return self.next_projection_at_ns;
}

pub fn setNextProjectionAt(self: *state.State, value: i128) void {
    self.next_projection_at_ns = value;
}

pub fn nextCloseActivitySampleAt(self: *const state.State) i128 {
    return self.next_close_activity_sample_at_ns;
}

pub fn setNextCloseActivitySampleAt(self: *state.State, value: i128) void {
    self.next_close_activity_sample_at_ns = value;
}

pub fn projectionResident(self: *state.State) bool {
    self.projection_mutex.lock();
    defer self.projection_mutex.unlock();
    return self.projection.isResident();
}

pub fn resolveCloseActivity(
    platform_command_active: ?bool,
    semantic_prompt_seen: bool,
    cursor_at_prompt: bool,
) contracts.TerminalCloseActivity {
    const command_active = platform_command_active orelse return .unknown;
    if (command_active) return .active;
    if (semantic_prompt_seen) return if (cursor_at_prompt) .idle else .active;
    return .idle;
}

/// Close admission and published inventory share one fresh process sample plus
/// the committed semantic reader phase. Accepted Enter submissions remain
/// pending until a later C marker acknowledges them; only a ready prompt with
/// no pending or partial input is idle.
pub fn sampleCloseActivityForClose(self: *state.State) contracts.TerminalCloseActivity {
    return (sampleCloseActivity(self) catch return .unknown).activity;
}

/// Samples the independent process observation without holding a runtime
/// mutex, then resolves it with Ghostty's committed prompt phase and accepted
/// input accounting. This is the activity published in inventory for UI Close
/// preflight as well as the final Host termination guard. Activity-affecting
/// input invalidates older evidence; a later ready prompt restores idle only
/// after all accepted submissions are acknowledged and no partial input stays.
pub fn sampleCloseActivity(
    self: *state.State,
) contracts.ProjectionEncodeError!contracts.CloseActivityRefresh {
    self.projection_mutex.lock();
    if (self.close_activity_epoch_exhausted) {
        const activity = self.close_activity;
        self.projection_mutex.unlock();
        return .{ .activity = activity, .changed = false };
    }
    self.projection_mutex.unlock();

    const platform_command_active: ?bool = process_io_ops.hasActiveCommand(self) catch null;

    self.projection_mutex.lock();
    defer self.projection_mutex.unlock();
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);

    const prompt_seen = self.terminal.screens.active.semantic_prompt.seen;
    const at_prompt = prompt_seen and self.terminal.cursorIsAtPrompt();
    var next_activity = resolveCloseActivity(
        platform_command_active,
        prompt_seen,
        at_prompt,
    );
    const prompt_phase = self.close_prompt_phase;
    if (next_activity == .idle) {
        // C begins command execution. This catches in-process shell builtins
        // even when the platform observer sees only the interactive shell.
        if (prompt_phase == .command_active) {
            next_activity = .active;
        } else if (!at_prompt or prompt_phase != .input_ready) {
            next_activity = .unknown;
        } else {
            self.input_mutex.lock();
            const pending_lines = self.close_input_lines_submitted > self.close_input_lines_consumed;
            const unresolved_input = self.close_input_lines_exhausted or
                self.close_input_partial_line or
                self.input_queue.client_charge.entries != 0;
            self.input_mutex.unlock();
            if (pending_lines or unresolved_input) next_activity = .unknown;
        }
    }

    const previous_activity = self.close_activity;
    if (previous_activity != next_activity and
        !advanceCloseActivityEpochLocked(self))
    {
        next_activity = .unknown;
    }
    const changed = previous_activity != next_activity;
    self.close_activity = next_activity;
    return .{ .activity = next_activity, .changed = changed };
}

/// Commits the projection half of an activity-affecting input transaction.
/// The caller holds projection_mutex then input_mutex and has already committed
/// the input queue publication (or accepted a zero-byte semantic input).
pub fn invalidateCloseActivityAfterInputLocked(self: *state.State) void {
    self.close_activity = .unknown;
    _ = advanceCloseActivityEpochLocked(self);
}

/// Advances the presentation epoch while projection_mutex is held. Once the
/// counter is exhausted the runtime remains permanently unknown; saturating is
/// safe because no later frame can carry idle or active evidence at that epoch.
fn advanceCloseActivityEpochLocked(self: *state.State) bool {
    if (self.close_activity_epoch_exhausted) return false;
    self.close_activity_epoch = std.math.add(
        u64,
        self.close_activity_epoch,
        1,
    ) catch {
        self.close_activity_epoch_exhausted = true;
        self.close_activity = .unknown;
        return false;
    };
    return true;
}

pub fn releaseProjectionResources(self: *state.State) void {
    self.projection_mutex.lock();
    defer self.projection_mutex.unlock();
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    self.projection.releaseResources();
}

pub fn encodeProjectionFrame(
    self: *state.State,
    request: contracts.ProjectionEncodeRequest,
) !contracts.ProjectionFrame {
    return switch (try encodeProjectionFrameOwned(self, self.allocator, 0, request)) {
        .frame => |frame| frame,
        .blocked => |blocked| switch (blocked.reason) {
            .capture_allocation_failed => error.CaptureAllocationFailed,
            .projection_too_large => error.ProjectionTooLarge,
        },
    };
}

pub fn encodeProjectionFrameOwned(
    self: *state.State,
    output_allocator: std.mem.Allocator,
    output_prefix_bytes: usize,
    request: contracts.ProjectionEncodeRequest,
) contracts.ProjectionEncodeError!contracts.ProjectionEncodeOutcome {
    self.projection_mutex.lock();
    defer self.projection_mutex.unlock();

    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.projection_capture_starting) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
    self.terminal_mutex.lock();
    failure_ops.ensureReadableStateAvailableLocked(self) catch |err| {
        self.terminal_mutex.unlock();
        return err;
    };
    if (mode_effects.projectionSuppressedLocked(self)) {
        self.terminal_mutex.unlock();
        return error.SynchronizedOutput;
    }
    const revision = self.revision.load(.monotonic);
    const generation = self.generation;
    const read_commit_epoch = self.read_commit_epoch;
    const progress = self.progress;
    const command_completion = self.command_completion;
    self.command_observation.refresh(&self.terminal) catch {
        self.terminal_mutex.unlock();
        return .{ .blocked = .{ .attempted_revision = revision, .reason = .capture_allocation_failed } };
    };
    const command_observation = self.command_observation.committed;
    const close_activity = self.close_activity;
    const close_activity_epoch = self.close_activity_epoch;
    const close_activity_epoch_exhausted = self.close_activity_epoch_exhausted;
    if (self.projection.needsCapture(revision, generation)) {
        self.projection.graphics.animation_tick = self.animation_tick;
        self.projection.beginCapture(
            &self.terminal,
            revision,
            generation,
            self.cell_pixel_width,
            self.cell_pixel_height,
        ) catch |err| {
            self.terminal_mutex.unlock();
            self.projection.resetAfterCaptureFailure();
            return .{ .blocked = .{
                .attempted_revision = revision,
                .reason = projectionBlockReason(err),
            } };
        };
        self.terminal_mutex.unlock();
        self.projection.endCapture();
    } else {
        self.terminal_mutex.unlock();
    }
    var projection_request = request;
    projection_request.close_activity = close_activity;
    projection_request.progress = progress;
    projection_request.command_completion = command_completion;
    projection_request.command_observation = command_observation;
    var frame = self.projection.encodeOwned(
        output_allocator,
        output_prefix_bytes,
        projection_request,
    ) catch |err| {
        // Encoding is the final fallible step of a capture. If the frame
        // cannot be materialized (including aggregate-size or OOM
        // failures), discard the capture-owned link/title/row cache before
        // the scheduler marks the attachment blocked. Retaining it here
        // would make a failed capture permanently resident until an
        // unrelated future capture happened to clean it up.
        self.projection.resetAfterCaptureFailure();
        return .{ .blocked = .{
            .attempted_revision = revision,
            .reason = projectionBlockReason(err),
        } };
    };
    std.debug.assert(frame.revision == revision);
    std.debug.assert(frame.generation == generation);
    frame.read_commit_epoch = read_commit_epoch;
    frame.close_activity_epoch = close_activity_epoch;
    frame.close_activity_epoch_exhausted = close_activity_epoch_exhausted;
    return .{ .frame = frame };
}

pub fn projectionBlockReason(err: anyerror) contracts.ProjectionBlockReason {
    return switch (err) {
        error.ProjectionTooLarge => .projection_too_large,
        else => .capture_allocation_failed,
    };
}
