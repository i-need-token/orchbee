const std = @import("std");

const core_module = @import("terminal-host-core").state;
const pty_process = @import("terminal-host-platform").process;
const terminal_runtime = @import("terminal-host-engine").runtime;

const max_runtime_sync_per_poll = 64;
const cleanup_retry_delay_ns: i128 = std.time.ns_per_s;

pub const TerminalExitNotice = struct {
    creation_id: core_module.CreationId,
    final_revision: u64,
    result: core_module.ExitResult,
    terminal_id: core_module.TerminalId,

    pub fn fromTerminal(terminal: core_module.TerminalSession) TerminalExitNotice {
        std.debug.assert(terminal.phase() == .exited);
        return .{
            .creation_id = terminal.creation_id,
            .final_revision = terminal.settledFinalRevision().?,
            .result = terminal.exitResult().?,
            .terminal_id = terminal.terminal_id,
        };
    }
};

pub const TerminalFailureNotice = struct {
    creation_id: core_module.CreationId,
    failure: core_module.RuntimeFailure,
    final_revision: u64,
    process: core_module.ProcessObservation,
    terminal_id: core_module.TerminalId,
};

pub const TerminalOutcomeNotice = union(enum) {
    exited: TerminalExitNotice,
    failed: TerminalFailureNotice,

    pub fn token(self: TerminalOutcomeNotice) core_module.OutcomePublicationToken {
        return switch (self) {
            .exited => |notice| .{
                .final_revision = notice.final_revision,
                .kind = .exited,
                .terminal_id = notice.terminal_id,
            },
            .failed => |notice| .{
                .final_revision = notice.final_revision,
                .kind = .failed,
                .terminal_id = notice.terminal_id,
            },
        };
    }
};

pub const PendingOutcomeBatch = struct {
    outcome_notice_count: usize = 0,
    outcome_notices: [max_runtime_sync_per_poll]TerminalOutcomeNotice = undefined,

    pub fn outcomes(self: *const PendingOutcomeBatch) []const TerminalOutcomeNotice {
        return self.outcome_notices[0..self.outcome_notice_count];
    }
};

/// Owns fair runtime-observation scans, cleanup retry scheduling, and the
/// conversion of immutable engine observations into Host lifecycle facts.
pub const Maintenance = struct {
    cleanup_retry_scan_index: usize = 0,
    outcome_scan_index: usize = 0,
    runtime_sync_index: usize = 0,

    pub fn synchronizeRuntimes(
        self: *Maintenance,
        core: *core_module.HostCore,
    ) void {
        const count = core.terminalCount();
        if (count == 0) return;
        const start = self.runtime_sync_index % count;
        const limit = @min(count, max_runtime_sync_per_poll);
        for (0..limit) |offset| {
            const index = (start + offset) % count;
            const terminal_id = core.terminalKeys()[index];
            const runtime = core.terminalValues()[index].runtime() orelse continue;
            self.synchronizeRuntime(core, terminal_id, runtime);
        }
        self.runtime_sync_index = (start + limit) % count;
        self.retryOneBlockedCleanup(core, std.time.nanoTimestamp());
    }

    /// Synchronizes exactly the bounded inventory slice that is about to be
    /// serialized, independently of the fair background scan cursor.
    pub fn synchronizeTerminalRange(
        self: *Maintenance,
        core: *core_module.HostCore,
        offset: usize,
        maximum_terminals: usize,
    ) void {
        const count = core.terminalCount();
        std.debug.assert(offset <= count);
        const end = offset + @min(maximum_terminals, count - offset);
        for (offset..end) |index| {
            const terminal_id = core.terminalKeys()[index];
            const runtime = core.terminalValues()[index].runtime() orelse continue;
            self.synchronizeRuntime(core, terminal_id, runtime);
        }
    }

    pub fn pendingOutcomes(
        self: *Maintenance,
        core: *core_module.HostCore,
        maximum_notices: usize,
    ) PendingOutcomeBatch {
        var batch: PendingOutcomeBatch = .{};
        std.debug.assert(maximum_notices <= batch.outcome_notices.len);
        if (maximum_notices == 0) return batch;
        const count = core.terminalCount();
        if (count == 0) return batch;
        const start = self.outcome_scan_index % count;
        const limit = @min(count, max_runtime_sync_per_poll);
        var scanned: usize = 0;
        while (scanned < limit and batch.outcome_notice_count < maximum_notices) {
            const index = (start + scanned) % count;
            scanned += 1;
            const terminal = core.terminalValues()[index];
            if (terminal.pendingOutcome()) |pending| {
                batch.outcome_notices[batch.outcome_notice_count] =
                    terminalOutcomeNotice(pending);
                batch.outcome_notice_count += 1;
            }
        }
        self.outcome_scan_index = (start + scanned) % count;
        return batch;
    }

    pub fn recordUnsafeResizeFailure(
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) void {
        _ = core.runtimeFailed(terminal_id, .apply_resize) catch unreachable;
        self.containRuntimeFailure(core, terminal_id, runtime) catch {};
    }

    pub fn containRuntimeFailure(
        _: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        runtime.shutdown() catch |err| {
            _ = core.failureContainmentBlocked(
                terminal_id,
                cleanupFailure(err),
                nextCleanupRetryAt(),
            ) catch unreachable;
            return err;
        };
        const completion = runtime.completion();
        _ = core.synchronizeRevision(
            terminal_id,
            completion.final_revision,
        ) catch unreachable;
        const observation: core_module.ProcessObservation = if (completion.child_exited)
            .{ .observed = exitResult(completion.exit_result.?) }
        else
            .{ .unavailable = .observe_process };
        _ = core.failureContained(terminal_id, observation) catch unreachable;
    }

    pub fn takeDueCleanupRetry(
        self: *Maintenance,
        core: *core_module.HostCore,
        now_ns: i128,
    ) ?core_module.TerminalId {
        const count = core.terminalCount();
        if (count == 0) return null;
        const start = self.cleanup_retry_scan_index % count;
        for (0..count) |offset| {
            const index = (start + offset) % count;
            const terminal_id = core.terminalKeys()[index];
            const terminal = core.terminalValues()[index];
            const retry_at_ns = terminal.lifecycle.cleanupRetryAt() orelse continue;
            if (retry_at_ns > now_ns) continue;
            self.cleanup_retry_scan_index = (index + 1) % count;
            return terminal_id;
        }
        self.cleanup_retry_scan_index = (start + 1) % count;
        return null;
    }

    pub fn cleanupRetryScanIndex(self: *const Maintenance) usize {
        return self.cleanup_retry_scan_index;
    }

    fn retryOneBlockedCleanup(
        self: *Maintenance,
        core: *core_module.HostCore,
        now_ns: i128,
    ) void {
        const terminal_id = self.takeDueCleanupRetry(core, now_ns) orelse return;
        const terminal = core.findTerminal(terminal_id) orelse return;
        const runtime = terminal.runtime() orelse return;
        self.containRuntimeFailure(core, terminal_id, runtime) catch {};
    }

    fn synchronizeRuntime(
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) void {
        const terminal = core.findTerminal(terminal_id) orelse return;
        if (terminal.lifecycle.needsContainmentWork()) {
            self.containRuntimeFailure(core, terminal_id, runtime) catch {};
        } else switch (terminal.phase()) {
            .containing_failure => {},
            .exited => {
                const completion = runtime.completion();
                switch (completion.classification) {
                    .normal => {},
                    .pending, .failed => unreachable,
                }
                std.debug.assert(completion.final_revision == terminal.observedRevision());
                std.debug.assert(terminal.settledFinalRevision().? == terminal.observedRevision());
            },
            .failed => {
                const completion = runtime.completion();
                switch (completion.classification) {
                    // A Host-side effect failure (for example termination
                    // dispatch) may retain a failed outcome without asking the
                    // engine to classify its own completion. Runtime shutdown
                    // has still joined every worker, so all three engine
                    // classifications are immutable at this Host state.
                    .pending, .normal, .failed => {},
                }
                std.debug.assert(completion.final_revision == terminal.observedRevision());
                std.debug.assert(terminal.settledFinalRevision().? == terminal.observedRevision());
            },
            else => {
                const completion = runtime.completion();
                if (runtimeFailureFromObservation(completion)) |failure| {
                    _ = core.synchronizeRevision(
                        terminal_id,
                        completion.final_revision,
                    ) catch unreachable;
                    _ = core.runtimeFailed(terminal_id, failure) catch unreachable;
                    self.containRuntimeFailure(core, terminal_id, runtime) catch {};
                } else {
                    _ = synchronizeRuntimeCompletion(core, terminal_id, completion);
                }
            },
        }
    }
};

pub fn cleanupFailure(
    err: terminal_runtime.ShutdownError,
) core_module.RuntimeCleanupFailure {
    return switch (err) {
        error.ProcessContainmentFailed => .contain_process,
        error.OutputReleaseFailed => .release_output,
        error.ProcessIdentityReleaseFailed => .release_process_identity,
    };
}

pub fn nextCleanupRetryAt() i128 {
    return std.math.add(
        i128,
        std.time.nanoTimestamp(),
        cleanup_retry_delay_ns,
    ) catch std.math.maxInt(i128);
}

fn exitResult(result: pty_process.ExitResult) core_module.ExitResult {
    return switch (result) {
        .exit_code => |code| .{ .exit_code = code },
        .signal => |signal| .{ .signal = signal },
        .unknown => |unknown| .{ .unknown = unknown },
    };
}

pub fn runtimeFailureFromObservation(
    completion: terminal_runtime.Completion,
) ?core_module.RuntimeFailure {
    const failure = switch (completion.classification) {
        .pending, .normal => return null,
        .failed => |failure| failure,
    };
    return switch (failure) {
        .resize => .apply_resize,
        .process_observation => .observe_process,
        .pty_read => .read_output,
        .pty_write => .write_input,
        .terminal_reply => .admit_terminal_reply,
        .worker_integrity => .synchronize_worker,
    };
}

fn terminalOutcomeNotice(pending: core_module.PendingOutcome) TerminalOutcomeNotice {
    return switch (pending) {
        .exited => |outcome| .{ .exited = .{
            .creation_id = outcome.creation_id,
            .final_revision = outcome.final_revision,
            .result = outcome.result,
            .terminal_id = outcome.terminal_id,
        } },
        .failed => |outcome| .{ .failed = .{
            .creation_id = outcome.creation_id,
            .failure = outcome.failure,
            .final_revision = outcome.final_revision,
            .process = outcome.process,
            .terminal_id = outcome.terminal_id,
        } },
    };
}

pub fn synchronizeRuntimeCompletion(
    core: *core_module.HostCore,
    terminal_id: core_module.TerminalId,
    completion: terminal_runtime.Completion,
) ?TerminalExitNotice {
    const normal_completion_ready = switch (completion.classification) {
        .pending => false,
        .normal => true,
        .failed => return null,
    };
    const terminal = core.findTerminal(terminal_id) orelse return null;
    if (terminal.settledFinalRevision()) |final_revision| {
        std.debug.assert(final_revision == completion.final_revision);
        std.debug.assert(final_revision == terminal.observedRevision());
        return null;
    }
    _ = core.synchronizeRevision(terminal_id, completion.final_revision) catch unreachable;
    if (completion.pty_drained and !terminal.ptyDrained()) {
        if (!terminal.childExited() or normal_completion_ready) {
            _ = core.ptyDrained(terminal_id) catch unreachable;
        }
    }
    const after_drain = core.findTerminal(terminal_id).?;
    if (completion.child_exited and !after_drain.childExited()) {
        if (!after_drain.ptyDrained() or normal_completion_ready) {
            _ = core.childExited(terminal_id, exitResult(completion.exit_result.?)) catch unreachable;
        }
    }
    const updated_terminal = core.findTerminal(terminal_id).?;
    if (updated_terminal.phase() != .exited) return null;
    return TerminalExitNotice.fromTerminal(updated_terminal.*);
}
