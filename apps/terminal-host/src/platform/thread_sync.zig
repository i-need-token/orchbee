const std = @import("std");

/// Returns the process-wide blocking I/O implementation used only for the
/// futex operations behind the Host's explicitly spawned worker threads.
/// These workers are not `std.Io` tasks, so synchronization is deliberately
/// uncancelable and does not consume an I/O concurrency slot.
pub fn io() std.Io {
    return std.Io.Threaded.global_single_threaded.io();
}

pub const Mutex = struct {
    raw: std.Io.Mutex = .init,

    pub fn lock(self: *Mutex) void {
        self.raw.lockUncancelable(io());
    }

    pub fn unlock(self: *Mutex) void {
        self.raw.unlock(io());
    }
};

pub const Condition = struct {
    raw: std.Io.Condition = .init,

    pub fn wait(self: *Condition, mutex: *Mutex) void {
        self.raw.waitUncancelable(io(), &mutex.raw);
    }

    pub fn timedWait(self: *Condition, mutex: *Mutex, timeout_ns: u64) !void {
        return waitTimeout(
            &self.raw,
            &mutex.raw,
            .{ .duration = .{
                .clock = .awake,
                .raw = .fromNanoseconds(timeout_ns),
            } },
        );
    }

    pub fn signal(self: *Condition) void {
        self.raw.signal(io());
    }

    pub fn broadcast(self: *Condition) void {
        self.raw.broadcast(io());
    }
};

fn waitTimeout(
    condition: *std.Io.Condition,
    mutex: *std.Io.Mutex,
    timeout: std.Io.Timeout,
) (std.Io.Cancelable || std.Io.Timeout.Error)!void {
    const io_impl = io();
    const deadline = timeout.toDeadline(io_impl);
    var epoch = condition.epoch.load(.acquire);

    {
        const previous = condition.state.fetchAdd(
            .{ .waiters = 1, .signals = 0 },
            .monotonic,
        );
        std.debug.assert(previous.waiters < std.math.maxInt(u16));
    }

    mutex.unlock(io_impl);
    defer mutex.lockUncancelable(io_impl);

    while (true) {
        const result = io_impl.futexWaitTimeout(
            u32,
            &condition.epoch.raw,
            epoch,
            deadline,
        );
        epoch = condition.epoch.load(.acquire);

        {
            var previous = condition.state.load(.monotonic);
            while (previous.signals > 0) {
                previous = condition.state.cmpxchgWeak(
                    previous,
                    .{
                        .waiters = previous.waiters - 1,
                        .signals = previous.signals - 1,
                    },
                    .acquire,
                    .monotonic,
                ) orelse return;
            }
        }

        result catch |err| {
            const previous = condition.state.fetchSub(
                .{ .waiters = 1, .signals = 0 },
                .monotonic,
            );
            std.debug.assert(previous.waiters > 0);
            return err;
        };
        switch (deadline) {
            .none => {},
            .deadline => |value| if (value.untilNow(io_impl).raw.nanoseconds >= 0) {
                const previous = condition.state.fetchSub(
                    .{ .waiters = 1, .signals = 0 },
                    .monotonic,
                );
                std.debug.assert(previous.waiters > 0);
                return error.Timeout;
            },
            .duration => unreachable,
        }
    }
}
