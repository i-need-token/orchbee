const std = @import("std");
const target_builtin = @import("builtin");
const thread_sync = @import("terminal-host-platform").thread_sync;
const state = @import("state.zig");

/// Samples the runtime-owned monotonic clock domain. Tests may replace the
/// sample source, while production uses the timer created with the runtime.
pub fn monotonicTimeNanoseconds(self: *state.State) ?u64 {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.monotonic_time_nanoseconds) |read| {
            return read(self.testing_hooks.context.?);
        }
    }
    if (self.command_timer) |*timer| return timer.read();
    return null;
}

/// Samples real time as Unix nanoseconds for facts compared with other processes' clocks.
/// Tests may replace the millisecond source.
pub fn wallTimeNanoseconds(self: *state.State) ?u64 {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.wall_time_milliseconds) |read| {
            return std.math.mul(u64, read(self.testing_hooks.context.?), std.time.ns_per_ms) catch null;
        }
    }
    return std.math.cast(
        u64,
        std.Io.Timestamp.now(thread_sync.io(), .real).toNanoseconds(),
    );
}
