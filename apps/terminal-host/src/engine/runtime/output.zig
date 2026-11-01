const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const output_wake = @import("terminal-host-platform").output_wake;
const terminal_geometry = @import("terminal-host-model").geometry;
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const animation_ops = @import("animation.zig");
const failure_ops = @import("failure.zig");
const feedback_ops = @import("feedback.zig");
const process_io_ops = @import("process_io.zig");
const resize_ops = @import("resize.zig");
const metadata_ops = @import("metadata.zig");
const mode_effects = @import("mode_effects.zig");
const scrollback_compression_ops = @import("scrollback_compression.zig");
const terminal_reply = @import("terminal_reply.zig");

pub fn writePtyEffect(handler: *ghostty_vt.TerminalStream.Handler, bytes: []const u8) void {
    const self = runtimeFromHandler(handler);
    // The pinned Ghostty size callback serves CSI 14t/16t/18t and mode 2048
    // through one interface, so it cannot withhold only the pixel reports.
    // XTWINOPS replies are emitted as single writes on this host-owned seam;
    // while pixels are unknown the exact 14t/16t reply shapes are dropped
    // rather than answered with fabricated values. Cell geometry stays
    // authoritative: 18t and mode-2048 reports continue with real cells.
    if ((self.cell_pixel_width == 0 or self.cell_pixel_height == 0) and
        isPixelSizeReply(bytes)) return;
    terminal_reply.enqueueLocked(self, bytes) catch {
        self.reply_failure_pending = true;
    };
}

/// Matches one complete XTWINOPS pixel-size reply: `ESC [ 4 ; h ; w t` (the
/// CSI 14t response) or `ESC [ 6 ; h ; w t` (the CSI 16t response). Only exact
/// whole-write matches are suppressed; ordinary program output and every other
/// reply shape pass through untouched.
fn isPixelSizeReply(bytes: []const u8) bool {
    if (bytes.len < "\x1b[4;0;0t".len) return false;
    if (bytes[bytes.len - 1] != 't') return false;
    if (!std.mem.startsWith(u8, bytes, "\x1b[4;") and
        !std.mem.startsWith(u8, bytes, "\x1b[6;")) return false;
    var fields = std.mem.splitScalar(u8, bytes[4 .. bytes.len - 1], ';');
    const height = fields.next() orelse return false;
    const width = fields.next() orelse return false;
    if (fields.next() != null) return false;
    return digitsOnly(height) and digitsOnly(width);
}

fn digitsOnly(field: []const u8) bool {
    if (field.len == 0) return false;
    for (field) |byte| if (!std.ascii.isDigit(byte)) return false;
    return true;
}

pub fn bellEffect(handler: *ghostty_vt.TerminalStream.Handler) void {
    const self = runtimeFromHandler(handler);
    if (self.bell_sequence == std.math.maxInt(u64)) {
        self.bell_sequence_overflow_pending = true;
        return;
    }
    self.bell_sequence += 1;
}

pub fn metadataChangedEffect(handler: *ghostty_vt.TerminalStream.Handler) void {
    metadata_ops.markChangedLocked(runtimeFromHandler(handler));
}

pub fn metadataResetEffect(handler: *terminal_stream.Handler) void {
    metadata_ops.markChangedLocked(runtimeFromHostHandler(handler));
}

pub fn modeChangedEffect(
    handler: *terminal_stream.Handler,
    mode: ghostty_vt.Mode,
    enabled: bool,
) bool {
    const self = runtimeFromHostHandler(handler);
    if (mode == .enable_mode_3 and !restoreViewportGeometryEffect(handler)) {
        return false;
    }
    mode_effects.streamModeChangedLocked(self, mode, enabled) catch {
        self.reply_failure_pending = true;
    };
    return true;
}

pub fn fullResetEffect(handler: *terminal_stream.Handler) void {
    _ = mode_effects.resetLocked(runtimeFromHostHandler(handler));
}

pub fn colorSchemeEffect(
    handler: *ghostty_vt.TerminalStream.Handler,
) ?ghostty_vt.device_status.ColorScheme {
    return runtimeFromHandler(handler).profile.reportedColorScheme();
}

pub fn deviceAttributesEffect(handler: *ghostty_vt.TerminalStream.Handler) contracts.DeviceAttributes {
    return runtimeFromHandler(handler).profile.deviceAttributes();
}

pub fn enquiryEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
    return runtimeFromHandler(handler).profile.enquiry_response;
}

pub fn columnModeEffect(handler: *terminal_stream.Handler, enabled: bool) bool {
    const self = runtimeFromHostHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    const terminal = handler.terminal_handler.terminal;
    const target_columns: u16 = if (enabled) 132 else 80;
    const next_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = target_columns,
        .rows = terminal.rows,
    };
    return terminalDrivenGeometryEffect(
        self,
        terminal,
        next_geometry,
        .{ .column_mode = enabled },
    );
}

const TerminalGeometryMutation = union(enum) {
    column_mode: bool,
    viewport,
};

fn restoreViewportGeometryEffect(handler: *terminal_stream.Handler) bool {
    const self = runtimeFromHostHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    return terminalDrivenGeometryEffect(
        self,
        handler.terminal_handler.terminal,
        self.viewport_geometry,
        .viewport,
    );
}

fn terminalDrivenGeometryEffect(
    self: *state.State,
    terminal: *ghostty_vt.Terminal,
    next_geometry: terminal_geometry.Geometry,
    mutation: TerminalGeometryMutation,
) bool {
    const previous_geometry: terminal_geometry.Geometry = .{
        .cell_pixel_height = self.cell_pixel_height,
        .cell_pixel_width = self.cell_pixel_width,
        .columns = terminal.cols,
        .rows = terminal.rows,
    };
    const geometry_changes = !terminal_geometry.eql(previous_geometry, next_geometry);

    if (geometry_changes) {
        std.debug.assert(!self.resize_effect_in_flight);
        self.resize_effect_in_flight = true;
        self.terminal_mutex.unlock();
        const resize_result = resize_ops.resizePtyEffect(self, previous_geometry, next_geometry);
        resize_ops.notifyResizeEffectReturned(self);
        self.terminal_mutex.lock();
        resize_result catch {
            self.resize_fenced.store(true, .release);
            self.terminal_resize_failure_pending = true;
            resize_ops.finishResizeEffectLocked(self);
            return false;
        };
    }

    applyTerminalGeometryMutation(self, terminal, next_geometry, mutation) catch {
        self.resize_fenced.store(true, .release);
        if (geometry_changes) {
            self.terminal_mutex.unlock();
            resize_ops.restorePtyGeometry(self, next_geometry, previous_geometry) catch {};
            self.terminal_mutex.lock();
            resize_ops.finishResizeEffectLocked(self);
        }
        self.terminal_resize_failure_pending = true;
        return false;
    };

    self.terminal_driven_resize_target = next_geometry;
    if (geometry_changes) {
        mode_effects.geometryCommittedLocked(self) catch {
            self.reply_failure_pending = true;
        };
        resize_ops.finishResizeEffectLocked(self);
    }
    return true;
}

fn applyTerminalGeometryMutation(
    self: *state.State,
    terminal: *ghostty_vt.Terminal,
    next_geometry: terminal_geometry.Geometry,
    mutation: TerminalGeometryMutation,
) !void {
    switch (mutation) {
        .column_mode => |enabled| try terminal.deccolm(
            terminal.screens.active.alloc,
            if (enabled) .@"132_cols" else .@"80_cols",
        ),
        .viewport => {
            try terminal.resize(
                self.allocator,
                .{
                    .cell_size_px = .{
                        .height = next_geometry.cell_pixel_height,
                        .width = next_geometry.cell_pixel_width,
                    },
                    .cols = next_geometry.columns,
                    .rows = next_geometry.rows,
                },
            );
            self.cell_pixel_width = next_geometry.cell_pixel_width;
            self.cell_pixel_height = next_geometry.cell_pixel_height;
        },
    }
}

pub fn sizeEffect(
    handler: *ghostty_vt.TerminalStream.Handler,
) ?ghostty_vt.size_report.Size {
    const self = runtimeFromHandler(handler);
    return .{
        .cell_height = self.cell_pixel_height,
        .cell_width = self.cell_pixel_width,
        .columns = self.terminal.cols,
        .rows = self.terminal.rows,
    };
}

pub fn versionEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
    return runtimeFromHandler(handler).profile.version_response;
}

/// Admits one PTY read while preserving shutdown and failure ordering.
fn beginReaderRead(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    return !self.failure_fenced and !self.output_abort_requested.load(.acquire);
}

pub fn notifyReaderBatchFinished(
    self: *state.State,
    outcome: contracts.ReaderBatchOutcome,
) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.reader_batch_finished) |notify| {
            notify(self.testing_hooks.context.?, outcome);
        }
    }
}

pub fn notifyReaderStopped(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.reader_stopped) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn notifyPtyDrainPublicationStarting(self: *state.State) void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.pty_drain_publication_starting) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn commitReaderBatch(
    self: *state.State,
    bytes: []const u8,
) contracts.ReaderBatchOutcome {
    return commitObservedReaderBatch(self, bytes);
}

fn commitObservedReaderBatch(
    self: *state.State,
    bytes: []const u8,
) contracts.ReaderBatchOutcome {
    self.terminal_mutex.lock();
    while (self.resize_effect_in_flight) {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.reader_resize_waiting) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
        self.terminal_condition.wait(&self.terminal_mutex);
    }
    if (self.failure_fenced) {
        self.terminal_mutex.unlock();
        notifyReaderBatchFinished(self, .failure_fenced);
        return .failure_fenced;
    }
    if (self.output_abort_requested.load(.acquire)) {
        self.terminal_mutex.unlock();
        notifyReaderBatchFinished(self, .output_aborted);
        return .output_aborted;
    }

    const current_revision = self.revision.load(.monotonic);
    if (current_revision == std.math.maxInt(u64) or
        self.generation == std.math.maxInt(u64))
    {
        const won = failure_ops.closeFailureFenceLocked(self, .worker_integrity);
        self.terminal_mutex.unlock();
        notifyReaderBatchFinished(self, .runtime_failed);
        if (won) failure_ops.finishFailureFence(self);
        return .runtime_failed;
    }

    std.debug.assert(!self.reader_commit_in_progress);
    self.reader_commit_in_progress = true;
    std.debug.assert(!self.reply_failure_pending);
    std.debug.assert(!self.bell_sequence_overflow_pending);
    std.debug.assert(!self.feedback_sequence_overflow_pending);
    std.debug.assert(!self.mouse_input_epoch_exhausted);
    std.debug.assert(!self.terminal_resize_failure_pending);
    std.debug.assert(self.terminal_driven_resize_target == null);
    self.stream.handler.beginBatch();
    feedback_ops.beginReaderBatch(self);
    const previous_screen = self.terminal.screens.active_key;
    const previous_graphics_generation = self.terminal.screens.active.kitty_images.generation;
    const previous_columns = self.terminal.cols;
    const previous_rows = self.terminal.rows;
    const previous_cell_pixel_width = self.cell_pixel_width;
    const previous_cell_pixel_height = self.cell_pixel_height;
    // Ghostty's 1049 path can reset the active screen without changing
    // its key (for example, enabling it twice). Isolate the terminal clear
    // marker for this batch so same-key replacement still invalidates
    // history while preserving a clear already pending for the renderer.
    const previous_clear = self.terminal.flags.dirty.clear;
    self.terminal.flags.dirty.clear = false;
    const cursor_was_visible = self.terminal.modes.get(.cursor_visible);
    self.stream.nextSlice(bytes);
    mode_effects.noteCursorVisibilityLocked(self, cursor_was_visible);
    const screen_reset = self.terminal.flags.dirty.clear;
    self.terminal.flags.dirty.clear = previous_clear or screen_reset;
    const geometry_changed = previous_columns != self.terminal.cols or
        previous_rows != self.terminal.rows or
        previous_cell_pixel_width != self.cell_pixel_width or
        previous_cell_pixel_height != self.cell_pixel_height;
    const terminal_resize_target = self.terminal_driven_resize_target;
    self.terminal_driven_resize_target = null;
    const terminal_geometry_unsynchronized = geometry_changed and
        (terminal_resize_target == null or
            terminal_resize_target.?.columns != self.terminal.cols or
            terminal_resize_target.?.rows != self.terminal.rows or
            terminal_resize_target.?.cell_pixel_width != self.cell_pixel_width or
            terminal_resize_target.?.cell_pixel_height != self.cell_pixel_height);
    const terminal_resize_failed = self.terminal_resize_failure_pending or
        terminal_geometry_unsynchronized;
    self.terminal_resize_failure_pending = false;
    if (terminal_resize_failed) {
        std.debug.assert(!self.resize_effect_in_flight);
        self.resize_fenced.store(true, .release);
        self.reply_failure_pending = false;
        self.bell_sequence_overflow_pending = false;
        self.feedback_sequence_overflow_pending = false;
        feedback_ops.abortReaderBatch(self);
        const won = failure_ops.closeFailureFenceLocked(self, .resize);
        finishReaderCommitLocked(self);
        self.terminal_mutex.unlock();
        notifyReaderBatchFinished(self, .runtime_failed);
        if (won) failure_ops.finishFailureFence(self);
        return .runtime_failed;
    }
    const screen_changed = previous_screen != self.terminal.screens.active_key or
        screen_reset;
    const graphics_changed = screen_changed or
        previous_graphics_generation != self.terminal.screens.active.kitty_images.generation;
    if (screen_changed or geometry_changed) self.generation += 1;
    const revision = current_revision + 1;
    self.revision.store(revision, .release);
    scrollback_compression_ops.noteInteractionLocked(self);
    metadata_ops.commitRevisionLocked(self, revision);
    if (screen_changed) {
        self.history.screenChanged(&self.terminal);
    } else if (geometry_changed) {
        self.history.generationChanged(&self.terminal, revision, self.generation);
    } else {
        self.history.noteMutation(&self.terminal, revision, self.generation);
    }
    _ = self.output_batches_since_sync.fetchAdd(1, .release);

    const reply_failed = self.reply_failure_pending;
    self.reply_failure_pending = false;
    if (reply_failed) feedback_ops.abortReaderBatch(self) else feedback_ops.commitReaderBatch(self);
    const bell_sequence_overflowed = self.bell_sequence_overflow_pending;
    self.bell_sequence_overflow_pending = false;
    const feedback_sequence_overflowed = self.feedback_sequence_overflow_pending;
    self.feedback_sequence_overflow_pending = false;
    const mouse_input_epoch_exhausted = self.mouse_input_epoch_exhausted;
    const failure_won = if (reply_failed)
        failure_ops.closeFailureFenceLocked(self, .terminal_reply)
    else if (bell_sequence_overflowed)
        failure_ops.closeFailureFenceLocked(self, .worker_integrity)
    else if (feedback_sequence_overflowed)
        failure_ops.closeFailureFenceLocked(self, .worker_integrity)
    else if (mouse_input_epoch_exhausted)
        failure_ops.closeFailureFenceLocked(self, .worker_integrity)
    else
        false;
    finishReaderCommitLocked(self);
    if (graphics_changed) animation_ops.requestWakeLocked(self);
    self.terminal_mutex.unlock();

    const outcome: contracts.ReaderBatchOutcome = if (reply_failed or
        bell_sequence_overflowed or feedback_sequence_overflowed or
        mouse_input_epoch_exhausted)
        .runtime_failed
    else
        .committed;
    notifyReaderBatchFinished(self, outcome);
    if (failure_won) failure_ops.finishFailureFence(self);
    return outcome;
}

pub fn finishReaderCommitLocked(self: *state.State) void {
    std.debug.assert(self.reader_commit_in_progress);
    self.reader_commit_in_progress = false;
    self.terminal_condition.broadcast();
}

pub fn publishPtyDrained(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    // This is the natural-drain linearization point. Shutdown publishes
    // the abort request before waiting for terminal_mutex, so an abort
    // that wins first cannot later be represented as normal PTY drain.
    if (self.failure_fenced or self.output_abort_requested.load(.acquire)) {
        return false;
    }
    self.pty_drained.store(true, .release);
    self.writer_stop = true;
    self.input_condition.broadcast();
    return true;
}

pub fn readerThread(self: *state.State) void {
    defer notifyReaderStopped(self);
    if (!self.startup.await()) return;
    var buffer: [contracts.max_pty_read_batch_bytes]u8 = undefined;
    while (true) {
        if (!beginReaderRead(self)) return;
        const length = process_io_ops.readPty(self, &buffer) catch {
            if (self.output_abort_requested.load(.acquire)) return;
            failure_ops.recordReaderFailure(self);
            return;
        };
        if (length == 0) {
            if (self.output_abort_requested.load(.acquire)) return;
            break;
        }
        if (commitObservedReaderBatch(self, buffer[0..length]) != .committed) return;
        output_wake.signal();
    }
    if (self.output_abort_requested.load(.acquire)) return;
    notifyPtyDrainPublicationStarting(self);
    _ = publishPtyDrained(self);
    output_wake.signal();
}

pub fn runtimeFromHandler(handler: *ghostty_vt.TerminalStream.Handler) *state.State {
    const host_handler: *terminal_stream.Handler = @fieldParentPtr("terminal_handler", handler);
    return runtimeFromHostHandler(host_handler);
}

pub fn runtimeFromHostHandler(handler: *terminal_stream.Handler) *state.State {
    const stream: *terminal_stream.Stream = @fieldParentPtr("handler", handler);
    return @alignCast(@fieldParentPtr("stream", stream));
}
