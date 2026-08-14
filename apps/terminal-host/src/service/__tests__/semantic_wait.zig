const std = @import("std");

const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const service_boundary = @import("terminal-host-service");
const host_outgoing = service_boundary.testing.outgoing;
const host_service = service_boundary.service;

const semantic_wait_timeout_ns: u64 = 10 * std.time.ns_per_s;

const Deadline = struct {
    timer: std.time.Timer,
    timeout_ns: u64,

    fn init(timeout_ns: u64) !Deadline {
        return .{
            .timer = try std.time.Timer.start(),
            .timeout_ns = timeout_ns,
        };
    }

    fn wait(
        self: *Deadline,
        condition: *std.Thread.Condition,
        mutex: *std.Thread.Mutex,
    ) !void {
        const elapsed = self.timer.read();
        if (elapsed >= self.timeout_ns) return error.SemanticWaitTimeout;
        condition.timedWait(mutex, self.timeout_ns - elapsed) catch
            return error.SemanticWaitTimeout;
    }
};

fn normalCompletion(
    runtime: *terminal_runtime.TerminalRuntime,
    deadline: *Deadline,
) !terminal_runtime.Completion {
    runtime.input_mutex.lock();
    defer runtime.input_mutex.unlock();
    while (true) {
        const completion = runtime.completion();
        switch (completion.classification) {
            .normal => return completion,
            .failed => return error.RuntimeFailedBeforeExpectedExit,
            .pending => {},
        }
        if (runtime.failure_fenced) return error.RuntimeUnavailableBeforeExpectedExit;
        try deadline.wait(&runtime.input_condition, &runtime.input_mutex);
    }
}

pub fn runtimeText(
    runtime: *terminal_runtime.TerminalRuntime,
    allocator: std.mem.Allocator,
    expected: []const u8,
) !void {
    var deadline = try Deadline.init(semantic_wait_timeout_ns);
    while (true) {
        const observed_revision = runtime.geometry().revision;
        const output = try runtime.plainString(allocator);
        const found = std.mem.indexOf(u8, output, expected) != null;
        allocator.free(output);
        if (found) return;

        runtime.terminal_mutex.lock();
        if (runtime.revision.load(.acquire) != observed_revision) {
            runtime.terminal_mutex.unlock();
            continue;
        }
        switch (runtime.completion().classification) {
            .normal => {
                runtime.terminal_mutex.unlock();
                return error.ExpectedRuntimeTextMissing;
            },
            .failed => {
                runtime.terminal_mutex.unlock();
                return error.RuntimeFailedBeforeExpectedText;
            },
            .pending => {},
        }
        deadline.wait(&runtime.terminal_condition, &runtime.terminal_mutex) catch |err| {
            runtime.terminal_mutex.unlock();
            return err;
        };
        runtime.terminal_mutex.unlock();
    }
}

pub fn terminalExits(
    service: *host_service.HostService,
    connection: *const host_service.ConnectionState,
    terminal_ids: []const core_module.TerminalId,
) !host_outgoing.Outgoing {
    std.debug.assert(terminal_ids.len > 0);
    var deadline = try Deadline.init(semantic_wait_timeout_ns);
    for (terminal_ids) |terminal_id| {
        const runtime = service.runtime_aggregate.runtimeFor(terminal_id) orelse
            return error.ExpectedTerminalRuntime;
        _ = try normalCompletion(runtime, &deadline);
    }

    var events = try service.pollEvents(connection);
    errdefer events.deinit();
    for (terminal_ids) |terminal_id| {
        const terminal = service.runtime_aggregate.core().findTerminal(terminal_id) orelse
            return error.ExpectedRetainedTerminal;
        if (terminal.phase() != .exited) return error.ExpectedTerminalExit;
    }
    return events;
}

test "an exhausted semantic deadline fails without blocking" {
    var condition: std.Thread.Condition = .{};
    var mutex: std.Thread.Mutex = .{};
    var deadline = try Deadline.init(0);
    mutex.lock();
    defer mutex.unlock();
    try std.testing.expectError(
        error.SemanticWaitTimeout,
        deadline.wait(&condition, &mutex),
    );
}
