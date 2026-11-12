const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_configuration = @import("terminal-host-model").configuration;
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const metadata_ops = @import("metadata.zig");
const mode_effects = @import("mode_effects.zig");
const program_status_feed = @import("program_status_feed.zig");

pub fn applyTerminalControl(
    self: *state.State,
    operation: contracts.TerminalControlOperation,
) !contracts.TerminalControlUpdate {
    self.terminal_mutex.lock();
    failure_ops.ensureRuntimeWorkAvailableLocked(self) catch |err| {
        self.terminal_mutex.unlock();
        return err;
    };
    if (self.child_exited.load(.acquire)) {
        self.terminal_mutex.unlock();
        return error.TerminalExited;
    }
    const current_revision = self.revision.load(.monotonic);
    if (operation == .clear_prompt and self.terminal.screens.active_key == .alternate) {
        self.terminal_mutex.unlock();
        return error.TerminalClearAlternateScreen;
    }
    if (self.generation == std.math.maxInt(u64) or
        current_revision == std.math.maxInt(u64))
    {
        self.terminal_mutex.unlock();
        return error.RevisionOverflow;
    }
    if (operation == .reset and !mode_effects.canAdvanceMouseInputEpochLocked(self)) {
        _ = mode_effects.advanceMouseInputEpochLocked(self);
        const won = failure_ops.closeFailureFenceLocked(self, .worker_integrity);
        self.terminal_mutex.unlock();
        if (won) failure_ops.finishFailureFence(self);
        return error.TerminalRuntimeUnavailable;
    }

    const had_metadata = self.terminal.getTitle() != null or self.terminal.getPwd() != null;
    switch (operation) {
        .clear => {
            self.terminal.eraseDisplay(.complete, false);
            self.terminal.setCursorPos(1, 1);
        },
        // ConPTY keeps its own buffer and places later output by its coordinates, so it
        // clears too, and the screen keeps exactly the row ConPTY keeps.
        .clear_prompt => clearPromptLocked(self, self.process.clearConsoleKeepingCursorRow()),
        .reset => {
            self.terminal.fullReset();
            std.debug.assert(mode_effects.resetLocked(self));
            program_status_feed.resetLocked(self);
            terminal_stream.restoreResetDefaults(&self.terminal);
            self.profile.applyCursorDefaults(&self.terminal);
        },
    }
    self.generation += 1;
    const revision = current_revision + 1;
    self.revision.store(revision, .release);
    if (operation == .reset) metadata_ops.commitResetLocked(self, revision, had_metadata);
    self.history.screenChanged(&self.terminal);
    const update: contracts.TerminalControlUpdate = .{
        .generation = self.generation,
        .revision = revision,
    };
    self.terminal_mutex.unlock();
    completion_ops.markNormalCompletionReady(self);
    return update;
}

// Preserve the entire cursor logical line, including continuations on either side, or only the
// cursor row when the console behind the child kept only that row.
// Pins and cursor are reloaded by Ghostty's erase operations, under terminal_mutex.
fn clearPromptLocked(self: *state.State, cursor_row_only: bool) void {
    const terminal = &self.terminal;
    const screen = terminal.screens.active;
    var first = screen.cursor.y;
    while (!cursor_row_only and first > 0) {
        const pin = screen.pages.pin(.{ .active = .{ .y = first } }).?;
        if (!pin.rowAndCell().row.wrap_continuation) break;
        first -= 1;
    }
    var last = screen.cursor.y;
    while (!cursor_row_only and last + 1 < terminal.rows) {
        const pin = screen.pages.pin(.{ .active = .{ .y = last } }).?;
        if (!pin.rowAndCell().row.wrap) break;
        last += 1;
    }
    screen.clearSelection();
    if (last + 1 < terminal.rows) {
        screen.clearRows(.{ .active = .{ .y = last + 1 } }, null, false);
    }
    screen.eraseHistory(null);
    if (first > 0) screen.eraseActive(first - 1);
    const top = screen.pages.pin(.{ .active = .{} }).?.rowAndCell().row;
    top.wrap_continuation = false;
    if (cursor_row_only) top.wrap = false;
    screen.scroll(.active);
    if (comptime @TypeOf(screen.kitty_images) != void) {
        screen.kitty_images.delete(terminal.io(), screen.alloc, terminal, .{ .all = true });
    }
    terminal.flags.dirty.clear = true;
}

pub fn configuration(self: *state.State) terminal_configuration.Configuration {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return .{ .scrollback_bytes = self.scrollback_bytes };
}

pub fn updateConfiguration(
    self: *state.State,
    next_configuration: terminal_configuration.Configuration,
) !contracts.ConfigurationUpdate {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (next_configuration.scrollback_bytes == self.scrollback_bytes) return .{
        .generation = self.generation,
        .history_rows_evicted = 0,
        .revision = self.revision.load(.monotonic),
    };
    if (!terminal_configuration.valid(next_configuration)) return error.InvalidConfiguration;

    const primary = self.terminal.screens.get(.primary).?;
    const may_evict = next_configuration.scrollback_bytes < self.scrollback_bytes and
        primary.pages.total_rows > self.terminal.rows;
    if (may_evict and (self.generation == std.math.maxInt(u64) or
        self.revision.load(.monotonic) == std.math.maxInt(u64)))
    {
        return error.RevisionOverflow;
    }

    const history_rows_evicted = applyScrollbackBytes(
        &self.terminal,
        next_configuration.scrollback_bytes,
    );
    self.scrollback_bytes = next_configuration.scrollback_bytes;
    if (history_rows_evicted == 0) return .{
        .generation = self.generation,
        .history_rows_evicted = 0,
        .revision = self.revision.load(.monotonic),
    };

    const next_generation = std.math.add(u64, self.generation, 1) catch unreachable;
    const next_revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch unreachable;
    self.generation = next_generation;
    self.revision.store(next_revision, .release);
    self.history.screenChanged(&self.terminal);
    return .{
        .generation = next_generation,
        .history_rows_evicted = history_rows_evicted,
        .revision = next_revision,
    };
}

pub fn applyScrollbackBytes(terminal: *ghostty_vt.Terminal, scrollback_bytes: usize) u64 {
    const primary = terminal.screens.get(.primary).?;
    const previous_total_rows = primary.pages.total_rows;
    primary.no_scrollback = scrollback_bytes == 0;
    primary.pages.setMaxBytes(scrollback_bytes);
    if (scrollback_bytes == 0) primary.pages.eraseHistory(null);
    return @intCast(previous_total_rows - primary.pages.total_rows);
}
