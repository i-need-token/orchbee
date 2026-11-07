const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_input_encoding = @import("../input_encoding.zig");
const engine_profile = @import("../profile.zig");
const terminal_theme = @import("terminal-host-model").theme;
const clock = @import("clock.zig");
const process_io_ops = @import("process_io.zig");
const state = @import("state.zig");
const terminal_reply = @import("terminal_reply.zig");

pub const synchronized_output_timeout_ns = std.time.ns_per_s;
/// How long a capture waits for a paint that began by hiding the cursor. ConPTY brackets each
/// paint this way and writes its pieces separately, so a frame taken after the first piece
/// shows nothing but a missing cursor and pushes the text to a later frame.
pub const cursor_paint_timeout_ns = 2 * std.time.ns_per_ms;

pub fn streamModeChangedLocked(
    self: *state.State,
    mode: ghostty_vt.Mode,
    enabled: bool,
) !void {
    if (isMouseMode(mode)) _ = advanceMouseInputEpochLocked(self);
    switch (mode) {
        .synchronized_output => updateSynchronizedOutputLocked(self, enabled),
        .focus_event => if (enabled) try reportFocusLocked(self),
        // The pinned Ghostty handler reports committed geometry when mode
        // 2048 is enabled. Host-driven geometry commits still use
        // `geometryCommittedLocked` below.
        .in_band_size_reports => {},
        .linefeed => {},
        else => {},
    }
}

pub fn resetLocked(self: *state.State) bool {
    clearSynchronizedOutputLocked(self);
    return advanceMouseInputEpochLocked(self);
}

pub fn canAdvanceMouseInputEpochLocked(self: *const state.State) bool {
    return !self.mouse_input_epoch_exhausted and
        self.mouse_input_epoch != std.math.maxInt(u64);
}

/// Advances the mouse-input validity boundary. The caller holds
/// terminal_mutex; taking input_mutex establishes the shared publication
/// order observed by prepared commits and writer starts.
pub fn advanceMouseInputEpochLocked(self: *state.State) bool {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.mouse_input_epoch_exhausted) return false;
    if (self.mouse_input_epoch == std.math.maxInt(u64)) {
        self.mouse_input_epoch_exhausted = true;
        self.input_condition.broadcast();
        return false;
    }
    self.mouse_input_epoch += 1;
    self.input_condition.broadcast();
    return true;
}

/// Repairs only a confirmed local-shell prompt. A foreground command or a
/// failed process observation preserves canonical application mouse state.
pub fn recoverMouseTrackingAtPromptLocked(self: *state.State) void {
    if (!mouseConfigurationActiveLocked(self)) return;
    if (process_io_ops.hasActiveCommand(self) catch return) return;
    if (!advanceMouseInputEpochLocked(self)) return;
    inline for (mouse_modes) |mode| self.terminal.modes.set(mode, false);
    self.terminal.flags.mouse_event = .none;
    self.terminal.flags.mouse_format = .x10;
}

/// Applies the non-parser half of a successful geometry commit. The caller
/// holds terminal_mutex and has already installed the exact new geometry.
pub fn geometryCommittedLocked(self: *state.State) !void {
    clearSynchronizedOutputLocked(self);
    if (self.terminal.modes.get(.in_band_size_reports)) {
        try reportSizeLocked(self);
    }
}

/// Queues the scheme report that a program which enabled mode 2031 needs when the effective
/// appearance changes. A palette-only edit, or a program that has not enabled 2031, queues nothing.
/// The caller holds terminal_mutex and has not installed `scheme` yet, because the check compares
/// against the installed scheme. A notice that cannot be queued is dropped by the caller, and the
/// theme installs anyway.
pub fn colorSchemeChangingLocked(self: *state.State, scheme: terminal_theme.ColorScheme) !void {
    if (self.profile.color_scheme == scheme) return;
    if (!self.terminal.modes.get(.report_color_scheme)) return;
    var buffer: [ghostty_vt.device_status.max_color_scheme_report_encode_size]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&buffer);
    try ghostty_vt.device_status.encodeColorSchemeReport(
        &writer,
        engine_profile.reportedColorSchemeFor(scheme),
    );
    try terminal_reply.enqueueLocked(self, writer.buffered());
}

/// Follows one reader batch. Only the visible-to-hidden edge opens the wait: a program that keeps
/// its cursor hidden never pays for it, and showing the cursor ends it at once.
pub fn noteCursorVisibilityLocked(self: *state.State, was_visible: bool) void {
    if (self.terminal.modes.get(.cursor_visible)) {
        self.cursor_paint_deadline_ns = null;
        return;
    }
    if (!was_visible) return;
    const now = clock.monotonicTimeNanoseconds(self) orelse return;
    self.cursor_paint_deadline_ns = std.math.add(u64, now, cursor_paint_timeout_ns) catch null;
}

/// Nanoseconds a new capture should still wait for the open paint, or null when none is open.
pub fn cursorPaintRemainingLocked(self: *state.State) ?u64 {
    const deadline = self.cursor_paint_deadline_ns orelse return null;
    const now = clock.monotonicTimeNanoseconds(self) orelse {
        self.cursor_paint_deadline_ns = null;
        return null;
    };
    if (now >= deadline) {
        self.cursor_paint_deadline_ns = null;
        return null;
    }
    return deadline - now;
}

/// Returns true while a synchronized-output interval still suppresses frame
/// capture. Deadline expiry is an Engine-owned canonical mutation so it
/// advances the content revision before the released projection is captured.
pub fn projectionSuppressedLocked(self: *state.State) bool {
    if (!self.terminal.modes.get(.synchronized_output)) {
        self.synchronized_output_deadline_ns = null;
        return false;
    }
    const deadline = self.synchronized_output_deadline_ns orelse {
        clearSynchronizedOutputLocked(self);
        return false;
    };
    const now = clock.monotonicTimeNanoseconds(self) orelse {
        clearSynchronizedOutputLocked(self);
        return false;
    };
    if (now < deadline) return true;

    clearSynchronizedOutputLocked(self);
    const revision = self.revision.load(.monotonic);
    if (revision != std.math.maxInt(u64)) {
        const next_revision = revision + 1;
        self.revision.store(next_revision, .release);
        self.history.noteMutation(&self.terminal, next_revision, self.generation);
    }
    return false;
}

fn updateSynchronizedOutputLocked(self: *state.State, enabled: bool) void {
    if (!enabled) {
        self.synchronized_output_deadline_ns = null;
        return;
    }
    const now = clock.monotonicTimeNanoseconds(self) orelse {
        clearSynchronizedOutputLocked(self);
        return;
    };
    self.synchronized_output_deadline_ns = std.math.add(
        u64,
        now,
        synchronized_output_timeout_ns,
    ) catch {
        clearSynchronizedOutputLocked(self);
        return;
    };
}

fn clearSynchronizedOutputLocked(self: *state.State) void {
    self.terminal.modes.set(.synchronized_output, false);
    self.synchronized_output_deadline_ns = null;
}

const mouse_modes = [_]ghostty_vt.Mode{
    .mouse_event_x10,
    .mouse_event_normal,
    .mouse_event_button,
    .mouse_event_any,
    .mouse_format_utf8,
    .mouse_format_sgr,
    .mouse_format_urxvt,
    .mouse_format_sgr_pixels,
};

fn isMouseMode(mode: ghostty_vt.Mode) bool {
    return switch (mode) {
        .mouse_event_x10,
        .mouse_event_normal,
        .mouse_event_button,
        .mouse_event_any,
        .mouse_format_utf8,
        .mouse_format_sgr,
        .mouse_format_urxvt,
        .mouse_format_sgr_pixels,
        => true,
        else => false,
    };
}

fn mouseConfigurationActiveLocked(self: *const state.State) bool {
    if (self.terminal.flags.mouse_event != .none or
        self.terminal.flags.mouse_format != .x10) return true;
    inline for (mouse_modes) |mode| {
        if (self.terminal.modes.get(mode)) return true;
    }
    return false;
}

fn reportFocusLocked(self: *state.State) !void {
    var buffer: [terminal_input_encoding.max_focus_bytes]u8 = undefined;
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    const encoded = try terminal_input_encoding.encodeFocus(
        &buffer,
        self.input_ownership.focusedCount() != 0,
    );
    try terminal_reply.enqueueInputLocked(self, encoded);
}

fn reportSizeLocked(self: *state.State) !void {
    var buffer: [128]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&buffer);
    try ghostty_vt.size_report.encode(&writer, .mode_2048, .{
        .cell_height = self.cell_pixel_height,
        .cell_width = self.cell_pixel_width,
        .columns = self.terminal.cols,
        .rows = self.terminal.rows,
    });
    try terminal_reply.enqueueLocked(self, writer.buffered());
}
