const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const monotonic = @import("terminal-host-platform").monotonic;
const contracts = @import("contracts.zig");
const runtime_observation = @import("observation.zig");

const CleanupClock = contracts.CleanupClock;
const PendingOutcomeBatch = contracts.PendingOutcomeBatch;
const RetainedCleanupResult = contracts.RetainedCleanupResult;
const RetainedCleanupStatus = contracts.RetainedCleanupStatus;
const max_runtime_sync_per_poll = contracts.max_runtime_sync_per_poll;

const cleanup_retry_delay_ns: i128 = std.time.ns_per_s;
/// Bounded number of cleanup-eligibility inspections allowed per
/// synchronization pass. The cleanup-retry cursor advances by the number of
/// terminals actually inspected, so a registry larger than this budget never
/// turns one nominally bounded pass into a full-registry scan.
const cleanup_retry_inspection_budget = 16;
const retained_cleanup_max_attempts = 32;
const retained_cleanup_backoff_ns = std.time.ns_per_s;

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
        owner: anytype,
    ) bool {
        _ = self;
        const core = owner.coreRead();
        for (core.terminalValues()) |session| {
            const runtime = owner.runtimeFor(session.terminal_id) orelse continue;
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
        owner: anytype,
    ) bool {
        self.synchronizeRuntimes(owner);
        self.retained_cleanup.attempts += 1;
        return !self.retainedCleanupPending(owner);
    }

    /// Runs the bounded THDD-001 retry window. Production sleeps between
    /// passes; deterministic tests advance their injected logical clock.
    pub fn runRetainedCleanupWindow(
        self: *Maintenance,
        owner: anytype,
    ) RetainedCleanupResult {
        var attempts: usize = 0;
        while (attempts < retained_cleanup_max_attempts) : (attempts += 1) {
            if (self.runRetainedCleanupPass(owner)) {
                std.log.info("terminal Host completed retained cleanup after {d} pass(es)", .{attempts + 1});
                return .complete;
            }
            if (comptime !target_builtin.is_test) {
                monotonic.sleep(retained_cleanup_backoff_ns);
            }
        }
        self.markRetainedCleanupExhausted(owner);
        return .retained_failed;
    }

    /// Converts every still-unconfirmed runtime into scheduled retained
    /// containment work while keeping the aggregate and Core ownership alive.
    pub fn retainUnconfirmedCleanup(
        self: *Maintenance,
        owner: anytype,
    ) void {
        const core = owner.core();
        var retained = false;
        for (core.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const runtime = owner.runtimeFor(terminal_id) orelse continue;
            if (runtime.containmentConfirmed()) continue;
            retained = true;

            // A normal outcome may already be settled while POSIX descendants
            // remain in the retained process domain. Copy an immutable normal
            // completion before choosing the cleanup owner, then keep cleanup
            // independent when the lifecycle has settled.
            const completion = runtime.completion();
            if (core.findTerminal(terminal_id).?.settledFinalRevision() == null) {
                switch (completion.classification) {
                    .normal => {
                        _ = runtime_observation.synchronizeRuntimeObservation(
                            core,
                            terminal_id,
                            runtime,
                        ) catch unreachable;
                        _ = runtime_observation.synchronizeRuntimeCompletion(core, terminal_id, completion);
                    },
                    .pending, .failed => {},
                }
            }
            if (core.findTerminal(terminal_id).?.settledFinalRevision() != null) {
                _ = core.retainedCleanupBlocked(
                    terminal_id,
                    .contain_process,
                    self.nextCleanupRetryAt(),
                ) catch unreachable;
                continue;
            }

            const primary_failure: core_module.RuntimeFailure = switch (completion.classification) {
                .failed => |kind| runtime_observation.runtimeFailureFromEngineKind(kind),
                .pending, .normal => .observe_process,
            };
            _ = core.runtimeFailed(terminal_id, primary_failure) catch unreachable;
            _ = core.failureContainmentBlocked(
                terminal_id,
                .contain_process,
                self.nextCleanupRetryAt(),
            ) catch unreachable;
        }
        if (retained) self.retained_cleanup = .{ .active = true };
    }

    /// Exhausting the bounded window leaves a stable explicit cleanup state;
    /// ownership remains retained without rewriting a settled outcome.
    pub fn markRetainedCleanupExhausted(
        self: *Maintenance,
        owner: anytype,
    ) void {
        const core = owner.core();
        for (core.terminalValues()) |session| {
            const terminal_id = session.terminal_id;
            const runtime = owner.runtimeFor(terminal_id) orelse continue;
            if (runtime.containmentConfirmed()) continue;
            if (session.retainedCleanupPending()) {
                _ = core.retainedCleanupExhausted(terminal_id) catch unreachable;
                continue;
            }
            const containment = session.failureContainment() orelse continue;
            const cleanup_failure: core_module.RuntimeCleanupFailure = switch (containment) {
                .blocked => |blocked| blocked.cleanup_failure,
                .stopping => .contain_process,
            };
            _ = core.failureContainmentBlocked(
                terminal_id,
                cleanup_failure,
                std.math.maxInt(i128),
            ) catch unreachable;
        }
        self.retained_cleanup.exhausted = true;
    }

    pub fn synchronizeRuntimes(
        self: *Maintenance,
        owner: anytype,
    ) void {
        const core = owner.core();
        const count = core.terminalCount();
        if (count == 0) return;
        const start = self.runtime_sync_index % count;
        const limit = @min(count, max_runtime_sync_per_poll);
        for (0..limit) |offset| {
            const index = (start + offset) % count;
            const terminal_id = core.terminalKeys()[index];
            const runtime = owner.runtimeFor(terminal_id) orelse continue;
            self.synchronizeRuntime(core, terminal_id, runtime);
        }
        self.runtime_sync_index = (start + limit) % count;
        self.retryOneBlockedCleanup(owner, self.cleanup_clock.now());
    }

    /// Synchronizes exactly the bounded inventory slice that is about to be
    /// serialized, independently of the fair background scan cursor.
    pub fn synchronizeTerminalRange(
        self: *Maintenance,
        owner: anytype,
        offset: usize,
        maximum_terminals: usize,
    ) void {
        const core = owner.core();
        const count = core.terminalCount();
        std.debug.assert(offset <= count);
        const end = offset + @min(maximum_terminals, count - offset);
        for (offset..end) |index| {
            const terminal_id = core.terminalKeys()[index];
            const runtime = owner.runtimeFor(terminal_id) orelse continue;
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
        owner: anytype,
        terminal_id: core_module.TerminalId,
    ) void {
        const core = owner.core();
        if (core.findTerminal(terminal_id) == null) return;
        const runtime = owner.runtimeFor(terminal_id) orelse return;
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
                    runtime_observation.terminalOutcomeNotice(pending);
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
        _ = runtime_observation.synchronizeRuntimeObservation(core, terminal_id, runtime) catch unreachable;
        runtime.shutdown() catch |err| {
            _ = core.failureContainmentBlocked(
                terminal_id,
                runtime_observation.cleanupFailure(err),
                self.nextCleanupRetryAt(),
            ) catch unreachable;
            return err;
        };
        const completion = runtime.completion();
        const geometry = runtime_observation.synchronizeRuntimeObservation(
            core,
            terminal_id,
            runtime,
        ) catch unreachable;
        std.debug.assert(geometry.revision == completion.final_revision);
        const observation: core_module.ProcessObservation = if (completion.child_exited)
            .{ .observed = runtime_observation.exitResult(completion.exit_result.?) }
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
            const retained_retry_at_ns = terminal.retainedCleanupRetryAt();
            const lifecycle_retry_at_ns = terminal.lifecycle.cleanupRetryAt();
            std.debug.assert(retained_retry_at_ns == null or lifecycle_retry_at_ns == null);
            const retry_at_ns = retained_retry_at_ns orelse lifecycle_retry_at_ns orelse continue;
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
        owner: anytype,
        now_ns: i128,
    ) void {
        const core = owner.core();
        const terminal_id = self.takeDueCleanupRetry(core, now_ns) orelse return;
        const terminal = core.findTerminal(terminal_id) orelse return;
        const runtime = owner.runtimeFor(terminal_id) orelse return;
        if (terminal.retainedCleanupRetryAt() != null) {
            self.retryRetainedCleanup(core, terminal_id, runtime) catch {};
        } else {
            self.containRuntimeFailure(core, terminal_id, runtime) catch {};
        }
    }

    fn retryRetainedCleanup(
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        runtime.shutdown() catch |err| {
            _ = core.retainedCleanupBlocked(
                terminal_id,
                runtime_observation.cleanupFailure(err),
                self.nextCleanupRetryAt(),
            ) catch unreachable;
            return err;
        };
        _ = core.retainedCleanupCompleted(terminal_id) catch unreachable;
    }

    fn synchronizeRuntime(
        self: *Maintenance,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) void {
        const retained_terminal = core.findTerminal(terminal_id) orelse return;
        switch (retained_terminal.retainedCleanup()) {
            .inactive => {},
            .blocked => {
                if (runtime.containmentConfirmed()) {
                    _ = core.retainedCleanupCompleted(terminal_id) catch unreachable;
                }
                return;
            },
            .complete, .exhausted => return,
        }
        _ = runtime_observation.synchronizeRuntimeObservation(core, terminal_id, runtime) catch unreachable;
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
                if (runtime_observation.runtimeFailureFromObservation(completion)) |failure| {
                    std.debug.assert(
                        completion.final_revision ==
                            core.findTerminal(terminal_id).?.observedRevision(),
                    );
                    _ = core.runtimeFailed(terminal_id, failure) catch unreachable;
                    self.containRuntimeFailure(core, terminal_id, runtime) catch {};
                } else {
                    _ = runtime_observation.synchronizeRuntimeCompletion(core, terminal_id, completion);
                }
            },
        }
    }
};
