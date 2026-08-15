const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const containment_ops = @import("containment.zig");
const completion_ops = @import("completion.zig");
const input_commit_ops = @import("input_commit.zig");
const process_io_ops = @import("process_io.zig");

pub fn isResizeFenced(self: *const state.State) bool {
    return self.resize_fenced.load(.acquire);
}

pub fn closeFailureFenceLocked(
    self: *state.State,
    failure: ?contracts.RuntimeFailureKind,
) bool {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.failure_fenced) return false;
    self.failure_fenced = true;
    self.writer_stop = true;
    if (failure) |classification| {
        self.completion_classification.markFailure(classification);
    }
    if (failure == null) {
        self.first_containment_attempt.reserveHost();
    } else {
        self.first_containment_attempt.reserveEngine();
    }
    self.input_condition.broadcast();
    return true;
}

pub fn closeFailureFence(
    self: *state.State,
    failure: ?contracts.RuntimeFailureKind,
) bool {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.failure_fence_starting) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
    self.terminal_mutex.lock();
    while (self.reader_commit_in_progress) {
        self.terminal_condition.wait(&self.terminal_mutex);
    }
    const won = closeFailureFenceLocked(self, failure);
    self.terminal_mutex.unlock();
    return won;
}

pub fn fenceHostFailure(self: *state.State) contracts.HostFailureFenceResult {
    if (closeFailureFence(self, null)) return .host_won;
    return if (primaryFailure(self)) |failure|
        .{ .engine_won = failure }
    else
        .host_won;
}

pub fn primaryFailure(self: *const state.State) ?contracts.RuntimeFailureKind {
    return switch (self.completion_classification.load()) {
        .failed => |failure| failure,
        .pending, .normal => null,
    };
}

pub fn waitForReaderCommitLocked(self: *state.State) void {
    while (self.reader_commit_in_progress) {
        if (comptime target_builtin.is_test) {
            if (self.testing_hooks.reader_commit_waiting) |notify| {
                notify(self.testing_hooks.context.?);
            }
        }
        self.terminal_condition.wait(&self.terminal_mutex);
    }
}

pub fn ensureRuntimeWorkAvailableLocked(self: *state.State) !void {
    waitForReaderCommitLocked(self);
    if (isResizeFenced(self)) return error.TerminalResizeFailed;
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    switch (self.completion_classification.load()) {
        .pending => {},
        .normal, .failed => return error.TerminalRuntimeUnavailable,
    }
}

pub fn ensureReadableStateAvailableLocked(self: *state.State) !void {
    waitForReaderCommitLocked(self);
    if (isResizeFenced(self)) return error.TerminalResizeFailed;
    switch (self.completion_classification.load()) {
        .failed => |failure| {
            if (failure == .resize) return error.TerminalResizeFailed;
            if (!self.failure_fenced) return error.TerminalRuntimeUnavailable;
        },
        .pending, .normal => {},
    }
    if (self.failure_fenced and !self.containment_confirmed.load(.acquire)) {
        return error.TerminalRuntimeUnavailable;
    }
}

pub fn readableStateAvailable(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    ensureReadableStateAvailableLocked(self) catch return false;
    return true;
}

pub fn finishFailureFence(self: *state.State) void {
    // The bounded fence and classification are already visible. External
    // PTY/process operations and fallible containment happen only after
    // both engine state locks have been released.
    defer containment_ops.completeFirstContainmentAttempt(self, .engine_pending);
    process_io_ops.abortOutputRead(self) catch {};
    containAfterRuntimeFailure(self);
}

pub fn containAfterRuntimeFailure(self: *state.State) void {
    containment_ops.containProcessTree(self) catch {};
}

pub fn recordFailure(self: *state.State, failure: contracts.RuntimeFailureKind) void {
    if (closeFailureFence(self, failure)) finishFailureFence(self);
}

pub fn recordReaderFailure(self: *state.State) void {
    recordFailure(self, .pty_read);
}

pub fn recordWorkerIntegrityFailure(self: *state.State) void {
    recordFailure(self, .worker_integrity);
}

pub fn recordWriterFailure(self: *state.State) void {
    const won = closeFailureFence(self, .pty_write);
    self.input_mutex.lock();
    input_commit_ops.clearInputQueueLocked(self);
    self.input_mutex.unlock();
    if (won) finishFailureFence(self);
}

pub fn releaseProcessIdentityFromWorker(self: *state.State) void {
    self.process.releaseProcessIdentity() catch {
        recordFailure(self, .process_observation);
        return;
    };
    completion_ops.markNormalCompletionReady(self);
}

pub fn recordProcessObservationFailure(self: *state.State) void {
    recordFailure(self, .process_observation);
    self.input_mutex.lock();
    while (!self.release_process_identity) {
        self.input_condition.wait(&self.input_mutex);
    }
    self.input_mutex.unlock();
    releaseProcessIdentityFromWorker(self);
}
