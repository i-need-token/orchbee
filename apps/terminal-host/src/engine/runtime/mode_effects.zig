const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_input_encoding = @import("../input_encoding.zig");
const clock = @import("clock.zig");
const state = @import("state.zig");
const terminal_reply = @import("terminal_reply.zig");

pub const synchronized_output_timeout_ns = std.time.ns_per_s;

pub fn streamModeChangedLocked(
    self: *state.State,
    mode: ghostty_vt.Mode,
    enabled: bool,
) !void {
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

pub fn resetLocked(self: *state.State) void {
    clearSynchronizedOutputLocked(self);
}

/// Applies the non-parser half of a successful geometry commit. The caller
/// holds terminal_mutex and has already installed the exact new geometry.
pub fn geometryCommittedLocked(self: *state.State) !void {
    clearSynchronizedOutputLocked(self);
    if (self.terminal.modes.get(.in_band_size_reports)) {
        try reportSizeLocked(self);
    }
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
