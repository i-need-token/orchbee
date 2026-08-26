const std = @import("std");
const core_module = @import("terminal-host-core").state;
const monotonic = @import("terminal-host-platform").monotonic;

pub const max_runtime_sync_per_poll = 64;

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
    timer: ?monotonic.Timer = null,
    previous_ns: u64 = 0,
    context: ?*anyopaque = null,
    read_ns: *const fn (clock: *CleanupClock) i128 = readMonotonicNs,

    pub fn now(self: *CleanupClock) i128 {
        return self.read_ns(self);
    }

    fn readMonotonicNs(self: *CleanupClock) i128 {
        if (self.timer) |*timer| {
            const reading = @max(timer.read(), self.previous_ns);
            self.previous_ns = reading;
            return reading;
        }
        self.timer = monotonic.Timer.start() catch unreachable;
        self.previous_ns = 0;
        return 0;
    }
};
