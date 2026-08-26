const std = @import("std");
const thread_sync = @import("terminal-host-platform").thread_sync;

pub const FirstAttempt = enum(u8) {
    none,
    engine_pending,
    host_pending,
    host_running,
    done,
};

/// Arbitrates the single first containment attempt between an engine failure
/// worker and a Host-originated failure fence. Later Host calls wait for the
/// winner and become retries; they can never overtake an in-flight owner.
pub const FirstAttemptGate = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    state: std.atomic.Value(FirstAttempt) = .init(.none),

    pub fn reserveEngine(self: *FirstAttemptGate) void {
        self.reserve(.engine_pending);
    }

    pub fn reserveHost(self: *FirstAttemptGate) void {
        self.reserve(.host_pending);
    }

    fn reserve(self: *FirstAttemptGate, state: FirstAttempt) void {
        std.debug.assert(state == .engine_pending or state == .host_pending);
        std.debug.assert(self.state.load(.acquire) == .none);
        self.state.store(state, .release);
    }

    pub fn complete(self: *FirstAttemptGate, expected: FirstAttempt) void {
        self.mutex.lock();
        std.debug.assert(self.state.load(.acquire) == expected);
        self.state.store(.done, .release);
        self.condition.broadcast();
        self.mutex.unlock();
    }

    /// Returns true only to the Host caller that owns a Host-originated first
    /// attempt. `waiting` fires outside the mutex before a blocking wait.
    pub fn awaitHost(
        self: *FirstAttemptGate,
        context: ?*anyopaque,
        waiting: ?*const fn (*anyopaque) void,
    ) bool {
        while (true) {
            self.mutex.lock();
            switch (self.state.load(.acquire)) {
                .none, .done => {
                    self.mutex.unlock();
                    return false;
                },
                .host_pending => {
                    self.state.store(.host_running, .release);
                    self.mutex.unlock();
                    return true;
                },
                .engine_pending, .host_running => {
                    self.mutex.unlock();
                    if (waiting) |notify| notify(context.?);
                    self.mutex.lock();
                    while (switch (self.state.load(.acquire)) {
                        .engine_pending, .host_running => true,
                        .none, .host_pending, .done => false,
                    }) {
                        self.condition.wait(&self.mutex);
                    }
                    self.mutex.unlock();
                },
            }
        }
    }

    pub fn load(self: *const FirstAttemptGate) FirstAttempt {
        return self.state.load(.acquire);
    }
};
