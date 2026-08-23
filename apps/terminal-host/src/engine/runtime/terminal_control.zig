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
    if (self.generation == std.math.maxInt(u64) or
        current_revision == std.math.maxInt(u64))
    {
        self.terminal_mutex.unlock();
        return error.RevisionOverflow;
    }

    const had_metadata = self.terminal.getTitle() != null or self.terminal.getPwd() != null;
    switch (operation) {
        .clear => {
            self.terminal.eraseDisplay(.complete, false);
            self.terminal.setCursorPos(1, 1);
        },
        .reset => {
            self.terminal.fullReset();
            mode_effects.resetLocked(self);
            terminal_stream.restoreColorDefaults(&self.terminal);
            self.profile.applyCursorDefaults(&self.terminal);
            self.stream.handler.terminal_handler.default_cursor = true;
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
    primary.pages.explicit_max_size = scrollback_bytes;

    while (primary.pages.total_rows > terminal.rows and
        (scrollback_bytes == 0 or primary.pages.page_size > primary.pages.maxSize()))
    {
        const history_rows = primary.pages.total_rows - terminal.rows;
        const first_page_rows = primary.pages.pages.first.?.data.size.rows;
        const rows_to_erase = @min(history_rows, first_page_rows);
        primary.pages.eraseHistory(.{ .history = .{ .y = rows_to_erase - 1 } });
    }
    return @intCast(previous_total_rows - primary.pages.total_rows);
}
