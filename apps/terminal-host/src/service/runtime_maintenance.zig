const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const pty_process = @import("terminal-host-platform").process;
const terminal_runtime = @import("terminal-host-engine").runtime;

const max_runtime_sync_per_poll = 64;
const cleanup_retry_delay_ns: i128 = std.time.ns_per_s;
/// Bounded number of cleanup-eligibility inspections allowed per
/// synchronization pass. The cleanup-retry cursor advances by the number of
/// terminals actually inspected, so a registry larger than this budget never
/// turns one nominally bounded pass into a full-registry scan.
const cleanup_retry_inspection_budget = 16;

/// THDD-001 retained-cleanup observability after a failed fatal-teardown pass.
pub const RetainedCleanupStatus = struct {
    active: bool = false,
    attempts: usize = 0,
    exhausted: bool = false,
};

/// Outcome of the bounded retained-cleanup window. A retained failure keeps
/// process-tree ownership explicit rather than silently abandoning it.
pub const RetainedCleanupResult = enum {
    complete,
    retained_failed,
};

const retained_cleanup_max_attempts = 32;
const retained_cleanup_backoff_ns = std.time.ns_per_s;

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

/// Host-lifetime monotonic clock for cleanup-retry scheduling. Both the
/// one-second retry deadline and the due-eligibility check read this clock, so
/// an adjustable wall clock (CLOCK_REALTIME / system calendar time) can neither
/// postpone nor accelerate cleanup retries. Production samples elapsed
/// monotonic time since the clock's first scheduling use and clamps readings so
/// they never go backward; tests inject a logical clock.
pub const CleanupClock = struct {
    epoch: ?std.time.Instant = null,
    previous: ?std.time.Instant = null,
    context: ?*anyopaque = null,
    read_ns: *const fn (clock: *CleanupClock) i128 = readMonotonicNs,

    pub fn now(self: *CleanupClock) i128 {
        return self.read_ns(self);
    }

    fn readMonotonicNs(self: *CleanupClock) i128 {
        const current = std.time.Instant.now() catch unreachable;
        if (self.epoch) |epoch| {
            var reading = current;
            if (reading.order(self.previous.?) == .lt) {
                reading = self.previous.?;
            }
            self.previous = reading;
            return @as(i128, reading.since(epoch));
        }
        self.epoch = current;
        self.previous = current;
        return 0;
    }
};

/// Owns fair runtime-observation scans, cleanup retry scheduling, and the
/// conversion of immutable engine observations into Host lifecycle facts.
pub const Maintenance = struct {
    cleanup_clock: CleanupClock = .{},
    cleanup_retry_scan_index: usize = 0,
    outcome_scan_index: usize = 0,
    retained_cleanup: RetainedCleanupStatus = .{},
    runtime_sync_index: usize = 0,

    /// Whether any retained process tree still lacks confirmed containment.
    pub fn retainedCleanupPending(
        self: *const Maintenance,
        core: *const core_module.HostCore,
    ) bool {
        _ = self;
        for (core.terminalValues()) |session| {
            const runtime = session.runtime() orelse continue;
            if (!runtime.containmentConfirmed()) return true;
        }
        return false;
    }

    pub fn retainedCleanupStatus(self: *const Maintenance) RetainedCleanupStatus {
        return self.retained_cleanup;
    }

    /// Drives one scheduled cleanup retry pass and records bounded progress.
    pub fn runRetainedCleanupPass(
        self: *Maintenance,
        core: *core_module.HostCore,
    ) bool {
        self.synchronizeRuntimes(core);
        self.retained_cleanup.attempts += 1;
        return !self.retainedCleanupPending(core);
    }

    /// Runs the bounded THDD-001 retry window. Production sleeps between
    /// passes; deterministic tests advance their injected logical clock.
    pub fn runRetainedCleanupWindow(
        self: *Maintenance,
        core: *core_module.HostCore,
    ) RetainedCleanupResult {
        var attempts: usize = 0;
        while (attempts < retained_cleanup_max_attempts) : (attempts += 1) {
            if (self.runRetainedCleanupPass(core)) {
                std.log.info("terminal Host completed retained cleanup after {d} pass(es)", .{attempts + 1});
                return .complete;
            }
            if (comptime !target_builtin.is_test) {
                std.Thread.sleep(retained_cleanup_backoff_ns);
            }
        }
        self.markRetainedCleanupExhausted(core);
        return .retained_failed;
    }

    /// Converts every still-unconfirmed runtime into scheduled retained
    /// containment work while keeping the aggregate and Core ownership alive.
    pub fn retainUnconfirmedCleanup(
        self: *Maintenance,
        core: *core_module.HostCore,
    ) void {
        var retained = false;
        for (core.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const runtime = session.runtime() orelse continue;
            if (runtime.containmentConfirmed()) continue;
            retained = true;
            const primary_failure: core_module.RuntimeFailure = switch (runtime.completion().classification) {
                .failed => |kind| runtimeFailureFromEngineKind(kind),
                .pending, .normal => .observe_process,
            };
            _ = core.runtimeFailed(terminal_id, primary_failure) catch continue;
            _ = core.failureContainmentBlocked(
                terminal_id,
                .contain_process,
                self.nextCleanupRetryAt(),
            ) catch continue;
        }
        if (retained) self.retained_cleanup = .{ .active = true };
    }

    /// Exhausting the bounded window leaves a stable never-due retry marker;
    /// ownership remains retained and the terminal is observably failed.
    pub fn markRetainedCleanupExhausted(
        self: *Maintenance,
        core: *core_module.HostCore,
    ) void {
        for (core.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const runtime = session.runtime() orelse continue;
            if (runtime.containmentConfirmed()) continue;
            const containment = session.failureContainment() orelse continue;
            const cleanup_failure: core_module.RuntimeCleanupFailure = switch (containment) {
                .blocked => |blocked| blocked.cleanup_failure,
                .stopping => .contain_process,
            };
            _ = core.failureContainmentBlocked(
                terminal_id,
                cleanup_failure,
                std.math.maxInt(i128),
            ) catch continue;
        }
        self.retained_cleanup.exhausted = true;
    }

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
        self.retryOneBlockedCleanup(core, self.cleanup_clock.now());
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

    /// Synchronizes exactly one retained terminal independently of the fair
    /// background cursor. A terminal-scoped command performs this exact pass
    /// before its Core admission check so a target the bounded background scan
    /// has not yet visited still receives target-aware ordering: an
    /// already-published engine classification is copied, and containment is
    /// completed or recorded, before any termination dispatch or disposal
    /// phase decision.
    pub fn synchronizeTerminal(
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
    ) void {
        const terminal = core.findTerminal(terminal_id) orelse return;
        const runtime = terminal.runtime() orelse return;
        self.synchronizeRuntime(core, terminal_id, runtime);
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
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        _ = synchronizeRuntimeObservation(core, terminal_id, runtime) catch unreachable;
        runtime.shutdown() catch |err| {
            _ = core.failureContainmentBlocked(
                terminal_id,
                cleanupFailure(err),
                self.nextCleanupRetryAt(),
            ) catch unreachable;
            return err;
        };
        const completion = runtime.completion();
        const geometry = synchronizeRuntimeObservation(
            core,
            terminal_id,
            runtime,
        ) catch unreachable;
        std.debug.assert(geometry.revision == completion.final_revision);
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
        const limit = @min(count, cleanup_retry_inspection_budget);
        var inspected: usize = 0;
        while (inspected < limit) : (inspected += 1) {
            const index = (start + inspected) % count;
            const terminal_id = core.terminalKeys()[index];
            const terminal = core.terminalValues()[index];
            const retry_at_ns = terminal.lifecycle.cleanupRetryAt() orelse continue;
            if (retry_at_ns > now_ns) continue;
            self.cleanup_retry_scan_index = (index + 1) % count;
            return terminal_id;
        }
        self.cleanup_retry_scan_index = (start + inspected) % count;
        return null;
    }

    /// Computes the next cleanup retry deadline from the Host-lifetime
    /// monotonic clock with saturating one-second arithmetic. The deadline is
    /// internal scheduling state, not part of the wire summary.
    pub fn nextCleanupRetryAt(self: *Maintenance) i128 {
        return std.math.add(
            i128,
            self.cleanup_clock.now(),
            cleanup_retry_delay_ns,
        ) catch std.math.maxInt(i128);
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
        if (core.findTerminal(terminal_id) == null) return;
        _ = synchronizeRuntimeObservation(core, terminal_id, runtime) catch unreachable;
        const terminal = core.findTerminal(terminal_id).?;
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
                    std.debug.assert(
                        completion.final_revision ==
                            core.findTerminal(terminal_id).?.observedRevision(),
                    );
                    _ = core.runtimeFailed(terminal_id, failure) catch unreachable;
                    self.containRuntimeFailure(core, terminal_id, runtime) catch {};
                } else {
                    _ = synchronizeRuntimeCompletion(core, terminal_id, completion);
                }
            },
        }
    }
};

pub fn synchronizeRuntimeObservation(
    core: *core_module.HostCore,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) core_module.CoreError!terminal_runtime.Geometry {
    const geometry = runtime.geometry();
    try core.synchronizeGeometry(
        terminal_id,
        geometry.columns,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
        geometry.revision,
    );
    return geometry;
}

pub fn cleanupFailure(
    err: terminal_runtime.ShutdownError,
) core_module.RuntimeCleanupFailure {
    return switch (err) {
        error.ProcessContainmentFailed => .contain_process,
        error.OutputReleaseFailed => .release_output,
        error.ProcessIdentityReleaseFailed => .release_process_identity,
    };
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
    return runtimeFailureFromEngineKind(failure);
}

pub fn runtimeFailureFromEngineKind(
    failure: terminal_runtime.RuntimeFailureKind,
) core_module.RuntimeFailure {
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
