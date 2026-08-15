const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

pub fn completion(self: *const state.State) contracts.Completion {
    // Normal classification is linearized under terminal_mutex after every
    // terminal mutation. Once it is visible, final_revision cannot advance.
    const classification = self.completion_classification.load();
    const pty_drained = self.pty_drained.load(.acquire);
    const child_exited = self.child_exited.load(.acquire);
    const writer_stopped = self.writer_stopped.load(.acquire);
    switch (classification) {
        .normal => {
            std.debug.assert(child_exited);
            std.debug.assert(self.exit_result != null);
            std.debug.assert(pty_drained);
            std.debug.assert(writer_stopped);
        },
        .pending, .failed => {},
    }
    return .{
        .child_exited = child_exited,
        .classification = classification,
        .exit_result = if (child_exited) self.exit_result else null,
        .final_revision = self.revision.load(.acquire),
        .pty_drained = pty_drained,
        .writer_stopped = writer_stopped,
    };
}

pub fn waitForCompletion(self: *state.State) !contracts.Completion {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    while (true) {
        const completion_state = completion(self);
        switch (completion_state.classification) {
            .pending => {},
            .normal => return completion_state,
            .failed => |failure| return switch (failure) {
                .resize => error.TerminalResizeFailed,
                .process_observation => error.ProcessObservationFailed,
                .pty_read => error.PtyReadFailed,
                .pty_write => error.PtyWriteFailed,
                .terminal_reply => error.TerminalReplyDropped,
                .worker_integrity => error.WorkerIntegrityFailed,
            },
        }
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        self.input_condition.wait(&self.input_mutex);
    }
}

pub fn markNormalCompletionReady(self: *state.State) void {
    // This lock is the engine's settled boundary. A mutation already in
    // progress completes before classification; every later mutation sees
    // .normal and is rejected by ensureRuntimeWorkAvailableLocked.
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    if (self.failure_fenced or self.resize_effect_in_flight or
        !self.child_exited.load(.acquire) or
        !self.pty_drained.load(.acquire) or
        !self.writer_stopped.load(.acquire)) return;
    if (!self.completion_classification.markNormal()) return;
    self.input_mutex.lock();
    self.input_condition.broadcast();
    self.input_mutex.unlock();
}
