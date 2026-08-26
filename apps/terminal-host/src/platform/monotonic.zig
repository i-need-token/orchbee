const std = @import("std");

const thread_sync = @import("thread_sync.zig");

/// Compatibility boundary for elapsed-time measurements after Zig 0.16 moved
/// monotonic clocks behind `std.Io`. Values are relative to construction and
/// therefore cannot be affected by wall-clock changes.
pub const Timer = struct {
    io_impl: std.Io,
    started_at: std.Io.Timestamp,

    pub const Error = error{TimerUnsupported};

    pub fn start() Error!Timer {
        return startWithIo(thread_sync.io());
    }

    pub fn startWithIo(io_impl: std.Io) Error!Timer {
        const resolution = std.Io.Clock.awake.resolution(io_impl) catch
            return error.TimerUnsupported;
        if (resolution.nanoseconds <= 0) return error.TimerUnsupported;
        return .{
            .io_impl = io_impl,
            .started_at = .now(io_impl, .awake),
        };
    }

    pub fn read(self: *const Timer) u64 {
        const elapsed = self.started_at.durationTo(.now(self.io_impl, .awake));
        if (elapsed.nanoseconds <= 0) return 0;
        return std.math.cast(u64, elapsed.nanoseconds) orelse std.math.maxInt(u64);
    }
};

pub fn sleep(nanoseconds: u64) void {
    sleepWithIo(thread_sync.io(), nanoseconds);
}

pub fn sleepWithIo(io_impl: std.Io, nanoseconds: u64) void {
    std.Io.sleep(
        io_impl,
        .fromNanoseconds(@intCast(nanoseconds)),
        .awake,
    ) catch {};
}
