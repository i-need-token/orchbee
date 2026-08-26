const std = @import("std");
const thread_sync = @import("terminal-host-platform").thread_sync;

const State = enum {
    pending,
    ready,
    aborted,
};

/// One-shot gate that parks runtime workers until process construction either
/// commits or aborts. All workers observe the same terminal decision.
pub const Gate = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    state: State = .pending,

    fn finish(self: *Gate, state: State) void {
        std.debug.assert(state != .pending);
        self.mutex.lock();
        std.debug.assert(self.state == .pending);
        self.state = state;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    pub fn ready(self: *Gate) void {
        self.finish(.ready);
    }

    pub fn abort(self: *Gate) void {
        self.finish(.aborted);
    }

    pub fn await(self: *Gate) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.state == .pending) self.condition.wait(&self.mutex);
        return self.state == .ready;
    }
};
