const std = @import("std");
const platform = @import("terminal-host-platform");
const monotonic = platform.monotonic;
const thread_sync = platform.thread_sync;

const terminal_runtime = @import("terminal-host-engine").runtime;
const runtimeState = terminal_runtime.testing.stateForTesting;

const semantic_wait_timeout_ns: u64 = 10 * std.time.ns_per_s;

const Deadline = struct {
    timer: monotonic.Timer,
    timeout_ns: u64,

    fn init(timeout_ns: u64) !Deadline {
        return .{
            .timer = try monotonic.Timer.start(),
            .timeout_ns = timeout_ns,
        };
    }

    fn wait(
        self: *Deadline,
        condition: *thread_sync.Condition,
        mutex: *thread_sync.Mutex,
    ) !void {
        const elapsed = self.timer.read();
        if (elapsed >= self.timeout_ns) return error.SemanticWaitTimeout;
        condition.timedWait(mutex, self.timeout_ns - elapsed) catch
            return error.SemanticWaitTimeout;
    }
};

pub const GenerationSignal = struct {
    condition: thread_sync.Condition = .{},
    generation: u64 = 0,
    mutex: thread_sync.Mutex = .{},

    pub fn notify(context: *anyopaque) void {
        const self: *GenerationSignal = @ptrCast(@alignCast(context));
        self.mutex.lock();
        self.generation += 1;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn snapshot(self: *GenerationSignal) u64 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.generation;
    }

    fn waitAfter(self: *GenerationSignal, generation: u64) !void {
        var deadline = try Deadline.init(semantic_wait_timeout_ns);
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.generation == generation) {
            try deadline.wait(&self.condition, &self.mutex);
        }
    }
};

pub fn childExit(
    runtime: *terminal_runtime.TerminalRuntime,
) !terminal_runtime.Completion {
    var deadline = try Deadline.init(semantic_wait_timeout_ns);
    runtimeState(runtime).input_mutex.lock();
    defer runtimeState(runtime).input_mutex.unlock();
    while (!runtimeState(runtime).child_exited.load(.acquire)) {
        try deadline.wait(&runtimeState(runtime).input_condition, &runtimeState(runtime).input_mutex);
    }
    return runtime.completion();
}

pub fn writerQueueObservation(
    runtime: *terminal_runtime.TerminalRuntime,
    signal: *GenerationSignal,
) !void {
    // Acquiring input_mutex establishes that an active writer has either not
    // started yet or has reached its condition wait. Broadcasting while the
    // mutex is held then requires a later queue observation before the hook can
    // advance the signal generation.
    runtimeState(runtime).input_mutex.lock();
    const generation = signal.snapshot();
    runtimeState(runtime).input_condition.broadcast();
    runtimeState(runtime).input_mutex.unlock();
    try signal.waitAfter(generation);
}

test "an exhausted semantic deadline fails without blocking" {
    var condition: thread_sync.Condition = .{};
    var mutex: thread_sync.Mutex = .{};
    var deadline = try Deadline.init(0);
    mutex.lock();
    defer mutex.unlock();
    try std.testing.expectError(
        error.SemanticWaitTimeout,
        deadline.wait(&condition, &mutex),
    );
}
