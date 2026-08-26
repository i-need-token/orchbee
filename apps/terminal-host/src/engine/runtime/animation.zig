const std = @import("std");

const thread_sync = @import("terminal-host-platform").thread_sync;
const failure_ops = @import("failure.zig");
const state = @import("state.zig");

const nanoseconds_per_millisecond = std.time.ns_per_ms;

pub const TickResult = struct {
    changed: bool,
    next_delay_milliseconds: ?u64,
};

/// Wakes the animation scheduler after a committed Kitty resource or active-
/// screen change. The caller owns terminal_mutex, keeping the notification
/// ordered with the canonical terminal mutation that made animation possible.
pub fn requestWakeLocked(self: *state.State) void {
    if (comptime @import("builtin").is_test) {
        if (self.testing_hooks.animation_wake_requested) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
    self.animation_condition.broadcast();
}

/// Advances Ghostty's active-screen animation clock. The caller owns
/// terminal_mutex; a displayed-frame change becomes one Host revision and one
/// monotonic animation tick regardless of how many images advance together.
pub fn tickLockedAt(self: *state.State, now_milliseconds: u64) error{CounterExhausted}!TickResult {
    const storage = &self.terminal.screens.active.kitty_images;
    const previous_generation = storage.generation;
    const next_delay = storage.animationTick(thread_sync.io(), now_milliseconds);
    const changed = storage.generation != previous_generation;
    if (!changed) return .{
        .changed = false,
        .next_delay_milliseconds = next_delay,
    };

    const revision = self.revision.load(.monotonic);
    if (revision == std.math.maxInt(u64) or
        self.animation_tick == std.math.maxInt(u64) or
        self.output_batches_since_sync.load(.monotonic) == std.math.maxInt(u64))
    {
        return error.CounterExhausted;
    }
    self.animation_tick += 1;
    self.revision.store(revision + 1, .release);
    self.history.animation_tick = self.animation_tick;
    self.history.noteMutation(&self.terminal, revision + 1, self.generation);
    _ = self.output_batches_since_sync.fetchAdd(1, .release);
    return .{
        .changed = true,
        .next_delay_milliseconds = next_delay,
    };
}

pub fn animationThread(self: *state.State) void {
    if (!self.startup.await()) return;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();

    while (true) {
        while ((self.reader_commit_in_progress or self.resize_effect_in_flight) and
            !self.failure_fenced and !self.output_abort_requested.load(.acquire))
        {
            self.terminal_condition.wait(&self.terminal_mutex);
        }
        if (self.failure_fenced or self.output_abort_requested.load(.acquire)) return;

        const timer = if (self.command_timer) |*value| value else {
            self.animation_condition.wait(&self.terminal_mutex);
            continue;
        };
        const now_milliseconds = timer.read() / nanoseconds_per_millisecond;
        const result = tickLockedAt(self, now_milliseconds) catch {
            const won = failure_ops.closeFailureFenceLocked(self, .worker_integrity);
            self.terminal_mutex.unlock();
            if (won) failure_ops.finishFailureFence(self);
            self.terminal_mutex.lock();
            return;
        };

        if (result.next_delay_milliseconds) |delay_ms| {
            const delay_ns = std.math.mul(u64, delay_ms, nanoseconds_per_millisecond) catch
                std.math.maxInt(u64);
            self.animation_condition.timedWait(&self.terminal_mutex, delay_ns) catch {};
        } else {
            self.animation_condition.wait(&self.terminal_mutex);
        }
    }
}
