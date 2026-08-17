const std = @import("std");
const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const read_commit_guard = @import("read_commit_guard.zig");
const failure_ops = @import("failure.zig");

pub fn bellSequence(self: *state.State) u64 {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return self.bell_sequence;
}

pub fn acquireReadCommitGuard(
    self: *state.State,
) contracts.ProjectionEncodeError!read_commit_guard.Internal {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.read_commit_checkpoint) |notify| {
            notify(self.testing_hooks.context.?, .before_lock);
        }
    }
    self.terminal_mutex.lock();
    errdefer self.terminal_mutex.unlock();
    try failure_ops.ensureReadableStateAvailableLocked(self);
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
    if (platform_command_active == true) return .active;
    if (semantic_prompt_seen) return if (cursor_at_prompt) .idle else .active;
    if (platform_command_active == false) return .idle;
    return .unknown;
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

    const platform_command_active = self.process.hasActiveCommand() catch null;

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
    const revision = self.revision.load(.monotonic);
    const generation = self.generation;
    const semantic_prompt_seen = self.terminal.screens.active.semantic_prompt.seen;
    const progress = self.progress;
    const command_completion = self.command_completion;
    const close_activity = resolveCloseActivity(
        platform_command_active,
        semantic_prompt_seen,
        semantic_prompt_seen and self.terminal.cursorIsAtPrompt(),
    );
    if (self.projection.needsCapture(revision, generation)) {
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
    const frame = self.projection.encodeOwned(
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
    return .{ .frame = frame };
}

pub fn projectionBlockReason(err: anyerror) contracts.ProjectionBlockReason {
    return switch (err) {
        error.ProjectionTooLarge => .projection_too_large,
        else => .capture_allocation_failed,
    };
}
