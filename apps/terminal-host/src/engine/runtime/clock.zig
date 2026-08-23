const target_builtin = @import("builtin");
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
