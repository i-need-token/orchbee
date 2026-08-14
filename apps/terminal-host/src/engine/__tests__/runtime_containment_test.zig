const std = @import("std");

const subject = @import("terminal-host-engine").testing.runtime_containment;

test "Host owns a reserved Host containment attempt exactly once" {
    var gate: subject.FirstAttemptGate = .{};
    gate.reserveHost();
    try std.testing.expect(gate.awaitHost(null, null));
    try std.testing.expectEqual(subject.FirstAttempt.host_running, gate.load());
    gate.complete(.host_running);
    try std.testing.expect(!gate.awaitHost(null, null));
}

test "Host waits for the engine's first containment attempt" {
    var gate: subject.FirstAttemptGate = .{};
    gate.reserveEngine();
    var waiting: Signal = .{};
    var call: AwaitCall = .{ .gate = &gate, .waiting = &waiting };
    const thread = try std.Thread.spawn(.{}, AwaitCall.run, .{&call});
    defer thread.join();

    waiting.wait();
    gate.complete(.engine_pending);
    call.finished.wait();
    try std.testing.expect(!call.owns_attempt);
    try std.testing.expectEqual(subject.FirstAttempt.done, gate.load());
}

const AwaitCall = struct {
    finished: Signal = .{},
    gate: *subject.FirstAttemptGate,
    owns_attempt: bool = true,
    waiting: *Signal,

    fn run(self: *AwaitCall) void {
        self.owns_attempt = self.gate.awaitHost(self.waiting, notifyWaiting);
        self.finished.notify();
    }

    fn notifyWaiting(context: *anyopaque) void {
        const signal: *Signal = @ptrCast(@alignCast(context));
        signal.notify();
    }
};

const Signal = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    value: bool = false,

    fn notify(self: *Signal) void {
        self.mutex.lock();
        self.value = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn wait(self: *Signal) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (!self.value) self.condition.wait(&self.mutex);
    }
};
