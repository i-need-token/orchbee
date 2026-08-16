const std = @import("std");

pub const Gate = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,
    released: bool = false,

    pub fn pause(self: *Gate) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        while (!self.released) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    pub fn wait(self: *Gate) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    pub fn release(self: *Gate) void {
        self.mutex.lock();
        self.released = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

pub const Signal = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,

    pub fn notify(self: *Signal) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    pub fn wait(self: *Signal) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }
};

/// A script is stronger than a pair of sleeps: each participant advances only
/// when its named event is next. Unexpected events are retained as a mismatch
/// instead of being silently reordered by the host scheduler.
pub fn Script(comptime Event: type) type {
    return struct {
        condition: std.Thread.Condition = .{},
        cursor: usize = 0,
        expected: []const Event,
        mismatch: bool = false,
        mutex: std.Thread.Mutex = .{},

        const Self = @This();

        pub fn init(expected: []const Event) Self {
            return .{ .expected = expected };
        }

        pub fn arrive(self: *Self, event: Event) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            while (!self.mismatch and self.cursor < self.expected.len and
                self.expected[self.cursor] != event)
            {
                if (!contains(self.expected[self.cursor..], event)) {
                    self.mismatch = true;
                    self.condition.broadcast();
                    return;
                }
                self.condition.wait(&self.mutex);
            }
            if (self.mismatch or self.cursor == self.expected.len) {
                self.mismatch = true;
                self.condition.broadcast();
                return;
            }
            self.cursor += 1;
            self.condition.broadcast();
        }

        pub fn waitForCursor(self: *Self, minimum: usize) void {
            self.mutex.lock();
            defer self.mutex.unlock();
            while (!self.mismatch and self.cursor < minimum) {
                self.condition.wait(&self.mutex);
            }
        }

        pub fn expectComplete(self: *Self) error{ScheduleMismatch}!void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.mismatch or self.cursor != self.expected.len) {
                return error.ScheduleMismatch;
            }
        }

        fn contains(events: []const Event, event: Event) bool {
            for (events) |candidate| if (candidate == event) return true;
            return false;
        }
    };
}
