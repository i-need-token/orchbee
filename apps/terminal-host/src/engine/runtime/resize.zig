const std = @import("std");
const target_builtin = @import("builtin");
const terminal_geometry = @import("terminal-host-model").geometry;
const state = @import("state.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const mode_effects = @import("mode_effects.zig");

pub fn resize(self: *state.State, columns: u16, rows: u16) !void {
    self.terminal_mutex.lock();
    const cell_pixel_width = self.cell_pixel_width;
    const cell_pixel_height = self.cell_pixel_height;
    self.terminal_mutex.unlock();
    _ = try resizeGeometry(self, columns, rows, cell_pixel_width, cell_pixel_height);
}

pub fn resizeGeometry(
    self: *state.State,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) !u64 {
    self.terminal_mutex.lock();
    failure_ops.ensureRuntimeWorkAvailableLocked(self) catch |err| {
        self.terminal_mutex.unlock();
        return err;
    };
    if (self.resize_effect_in_flight) {
        self.terminal_mutex.unlock();
        return error.TerminalRuntimeUnavailable;
    }
    const old_columns = self.terminal.cols;
    const old_rows = self.terminal.rows;
    const cells_changed = old_columns != columns or old_rows != rows;
    const pixels_changed = self.cell_pixel_width != cell_pixel_width or
        self.cell_pixel_height != cell_pixel_height;
    const next_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .columns = columns,
        .rows = rows,
    };
    if (!cells_changed and !pixels_changed) {
        // A granted owner is required to submit one measured geometry. Even
        // when canonical geometry already matches, that successful request
        // replaces a stale viewport target left by DECCOLM.
        self.viewport_geometry = next_geometry;
        const revision = self.revision.load(.acquire);
        self.terminal_mutex.unlock();
        return revision;
    }
    if (self.generation == std.math.maxInt(u64) or
        self.revision.load(.monotonic) == std.math.maxInt(u64))
    {
        self.terminal_mutex.unlock();
        return error.RevisionOverflow;
    }
    const previous_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = old_columns,
        .rows = old_rows,
    };
    self.resize_effect_in_flight = true;
    self.terminal_mutex.unlock();

    const resize_effect_result = resizePtyEffect(self, previous_geometry, next_geometry);
    // This notification exposes the post-platform-effect interval to
    // deterministic tests. The effect has returned, its result has not
    // been consumed, and no engine state mutex is held. Restore effects
    // deliberately bypass this one-shot initial-effect seam.
    notifyResizeEffectReturned(self);
    resize_effect_result catch |err| {
        if (err == error.ResizeRollbackFailed) {
            fenceResizeFailure(self);
            return err;
        }
        self.terminal_mutex.lock();
        const failure_fenced = self.failure_fenced;
        finishResizeEffectLocked(self);
        self.terminal_mutex.unlock();
        if (failure_fenced) return error.TerminalRuntimeUnavailable;
        completion_ops.markNormalCompletionReady(self);
        return err;
    };

    self.terminal_mutex.lock();
    if (self.failure_fenced) {
        self.terminal_mutex.unlock();
        const compensation = compensateResizeEffect(
            self,
            previous_geometry,
            next_geometry,
        );
        if (compensation.rollback_failed) return error.ResizeRollbackFailed;
        return error.TerminalRuntimeUnavailable;
    }
    // Attachment and parsed terminal requests share this one geometry
    // effect gate. PTY output and configuration may advance while this
    // attachment effect is in flight, but no second geometry effect may
    // overtake its commit.
    std.debug.assert(self.terminal.cols == old_columns);
    std.debug.assert(self.terminal.rows == old_rows);
    std.debug.assert(self.cell_pixel_width == previous_geometry.cell_pixel_width);
    std.debug.assert(self.cell_pixel_height == previous_geometry.cell_pixel_height);
    const next_generation = std.math.add(u64, self.generation, 1) catch {
        self.terminal_mutex.unlock();
        const compensation = compensateResizeEffect(
            self,
            previous_geometry,
            next_geometry,
        );
        if (compensation.rollback_failed) return error.ResizeRollbackFailed;
        if (compensation.failure_fenced) return error.TerminalRuntimeUnavailable;
        return error.RevisionOverflow;
    };
    const revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch {
        self.terminal_mutex.unlock();
        const compensation = compensateResizeEffect(
            self,
            previous_geometry,
            next_geometry,
        );
        if (compensation.rollback_failed) return error.ResizeRollbackFailed;
        if (compensation.failure_fenced) return error.TerminalRuntimeUnavailable;
        return error.RevisionOverflow;
    };
    if (cells_changed) {
        self.terminal.resize(self.allocator, .{
            .cell_size_px = .{
                .height = cell_pixel_height,
                .width = cell_pixel_width,
            },
            .cols = columns,
            .rows = rows,
        }) catch |err| {
            self.resize_fenced.store(true, .release);
            const won = failure_ops.closeFailureFenceLocked(self, .resize);
            self.terminal_mutex.unlock();
            const rollback_failed = rollback: {
                restorePtyGeometry(
                    self,
                    next_geometry,
                    previous_geometry,
                ) catch break :rollback true;
                break :rollback false;
            };
            self.terminal_mutex.lock();
            finishResizeEffectLocked(self);
            self.terminal_mutex.unlock();
            if (won) failure_ops.finishFailureFence(self);
            if (rollback_failed) return error.ResizeRollbackFailed;
            return err;
        };
    }
    self.cell_pixel_width = cell_pixel_width;
    self.cell_pixel_height = cell_pixel_height;
    self.viewport_geometry = next_geometry;
    self.generation = next_generation;
    self.revision.store(revision, .release);
    self.history.generationChanged(&self.terminal, revision, self.generation);
    mode_effects.geometryCommittedLocked(self) catch {
        const won = failure_ops.closeFailureFenceLocked(self, .terminal_reply);
        finishResizeEffectLocked(self);
        self.terminal_mutex.unlock();
        if (won) failure_ops.finishFailureFence(self);
        return error.TerminalRuntimeUnavailable;
    };
    finishResizeEffectLocked(self);
    self.terminal_mutex.unlock();
    completion_ops.markNormalCompletionReady(self);
    return revision;
}

pub fn fenceResizeFailure(self: *state.State) void {
    self.terminal_mutex.lock();
    self.resize_fenced.store(true, .release);
    const won = failure_ops.closeFailureFenceLocked(self, .resize);
    if (self.resize_effect_in_flight) finishResizeEffectLocked(self);
    self.terminal_mutex.unlock();
    if (won) failure_ops.finishFailureFence(self);
}

const ResizeCompensation = struct {
    failure_fenced: bool,
    rollback_failed: bool,
};

pub fn compensateResizeEffect(
    self: *state.State,
    previous_geometry: terminal_geometry.Geometry,
    next_geometry: terminal_geometry.Geometry,
) ResizeCompensation {
    const rollback_failed = rollback: {
        restorePtyGeometry(self, next_geometry, previous_geometry) catch
            break :rollback true;
        break :rollback false;
    };
    self.terminal_mutex.lock();
    var won = false;
    if (rollback_failed) {
        self.resize_fenced.store(true, .release);
        won = failure_ops.closeFailureFenceLocked(self, .resize);
    }
    finishResizeEffectLocked(self);
    const failure_fenced = self.failure_fenced;
    self.terminal_mutex.unlock();
    if (won) failure_ops.finishFailureFence(self);
    if (!failure_fenced) completion_ops.markNormalCompletionReady(self);
    return .{
        .failure_fenced = failure_fenced,
        .rollback_failed = rollback_failed,
    };
}

pub fn finishResizeEffectLocked(self: *state.State) void {
    std.debug.assert(self.resize_effect_in_flight);
    self.resize_effect_in_flight = false;
    self.terminal_condition.broadcast();
}

pub fn notifyResizeEffectReturned(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.resize_effect_returned) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn resizePtyEffect(
    self: *state.State,
    previous_geometry: terminal_geometry.Geometry,
    next_geometry: terminal_geometry.Geometry,
) !void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.resize_pty) |resize_pty| {
            return resize_pty(
                self.testing_hooks.context.?,
                previous_geometry,
                next_geometry,
            );
        }
    }
    try resizePtyWithRollback(&self.process, previous_geometry, next_geometry);
}

pub fn restorePtyGeometry(
    self: *state.State,
    current_geometry: terminal_geometry.Geometry,
    previous_geometry: terminal_geometry.Geometry,
) !void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.resize_pty) |resize_pty| {
            return resize_pty(
                self.testing_hooks.context.?,
                current_geometry,
                previous_geometry,
            );
        }
    }
    try self.process.resizePixels(
        previous_geometry.columns,
        previous_geometry.rows,
        previous_geometry.cell_pixel_width,
        previous_geometry.cell_pixel_height,
    );
}

pub fn resizePtyWithRollback(
    process: anytype,
    previous: terminal_geometry.Geometry,
    next: terminal_geometry.Geometry,
) !void {
    process.resizePixels(
        next.columns,
        next.rows,
        next.cell_pixel_width,
        next.cell_pixel_height,
    ) catch |resize_error| {
        // Platform adapters do not promise that an error is side-effect free.
        // A successful compensation re-establishes the last committed
        // geometry; otherwise the caller must fence the terminal engine.
        process.resizePixels(
            previous.columns,
            previous.rows,
            previous.cell_pixel_width,
            previous.cell_pixel_height,
        ) catch return error.ResizeRollbackFailed;
        return resize_error;
    };
}
