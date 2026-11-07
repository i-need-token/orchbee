const std = @import("std");

const terminal_theme = @import("terminal-host-model").theme;
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const completion_ops = @import("completion.zig");
const failure_ops = @import("failure.zig");
const mode_effects = @import("mode_effects.zig");

pub fn themeDefaults(self: *state.State) contracts.TerminalTheme {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return self.profile.themeDefaults(&self.terminal);
}

pub fn applyTheme(
    self: *state.State,
    next_theme: contracts.TerminalTheme,
) !contracts.ThemeUpdate {
    if (!terminal_theme.valid(next_theme)) return error.InvalidTheme;

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
    if (themeIsActive(self, next_theme)) {
        const update: contracts.ThemeUpdate = .{
            .generation = self.generation,
            .revision = current_revision,
        };
        self.terminal_mutex.unlock();
        return update;
    }
    if (current_revision == std.math.maxInt(u64)) {
        self.terminal_mutex.unlock();
        return error.RevisionOverflow;
    }

    // The notice is queued before the theme installs, because its check compares against the
    // installed scheme. A notice that cannot be queued is dropped instead of failing the terminal:
    // the theme still installs, and a program that queries CSI ? 996 n reads the new scheme.
    mode_effects.colorSchemeChangingLocked(self, next_theme.color_scheme) catch {};
    self.profile.applyTheme(&self.terminal, next_theme);
    const revision = current_revision + 1;
    self.revision.store(revision, .release);
    const update: contracts.ThemeUpdate = .{
        .generation = self.generation,
        .revision = revision,
    };
    self.terminal_mutex.unlock();
    completion_ops.markNormalCompletionReady(self);
    return update;
}

fn themeIsActive(self: *const state.State, expected: terminal_theme.Theme) bool {
    if (!terminal_theme.eql(self.profile.themeDefaults(&self.terminal), expected)) return false;
    if (self.terminal.colors.background.override != null or
        self.terminal.colors.foreground.override != null or
        self.terminal.colors.cursor.override != null or
        !optionalRgbEquals(self.terminal.colors.background.get(), expected.default_background_rgb) or
        !optionalRgbEquals(self.terminal.colors.foreground.get(), expected.default_foreground_rgb) or
        !optionalRgbEqualsOptional(self.terminal.colors.cursor.get(), expected.default_cursor_rgb) or
        self.terminal.colors.palette.mask.count() != 0)
    {
        return false;
    }
    for (self.terminal.colors.palette.current, expected.palette) |actual, color| {
        if (rgbCode(actual) != color) return false;
    }
    return true;
}

fn optionalRgbEquals(actual: ?@import("ghostty-vt").color.RGB, expected: u32) bool {
    const value = actual orelse return false;
    return rgbCode(value) == expected;
}

fn optionalRgbEqualsOptional(
    actual: ?@import("ghostty-vt").color.RGB,
    expected: ?u32,
) bool {
    if (actual == null or expected == null) return actual == null and expected == null;
    return rgbCode(actual.?) == expected.?;
}

fn rgbCode(value: @import("ghostty-vt").color.RGB) u32 {
    return (@as(u32, value.r) << 16) | (@as(u32, value.g) << 8) | value.b;
}
