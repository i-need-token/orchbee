const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const contracts = @import("contracts.zig");
const host_aggregate = @import("host_aggregate.zig");
const configuration_ops = @import("configuration.zig");
const creation_ops = @import("creation.zig");
const geometry_ops = @import("geometry.zig");
const termination_ops = @import("termination.zig");

const HostRuntimeAggregate = host_aggregate.HostRuntimeAggregate;
const RetainedCleanupResult = contracts.RetainedCleanupResult;
const RetainedCleanupStatus = contracts.RetainedCleanupStatus;
const TerminationError = contracts.TerminationError;
const TerminationMode = contracts.TerminationMode;

fn recordUnsafeResizeFailureForTesting(
    aggregate: *HostRuntimeAggregate,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) void {
    aggregate.maintenance.recordUnsafeResizeFailure(
        &aggregate.core_state,
        terminal_id,
        runtime,
    );
}

fn takeDueCleanupRetryForTesting(
    aggregate: *HostRuntimeAggregate,
    now_ns: i128,
) ?core_module.TerminalId {
    return aggregate.maintenance.takeDueCleanupRetry(&aggregate.core_state, now_ns);
}

fn retainUnconfirmedCleanupForTesting(aggregate: *HostRuntimeAggregate) void {
    aggregate.maintenance.retainUnconfirmedCleanup(aggregate);
}

fn cleanupRetryScanIndexForTesting(aggregate: *const HostRuntimeAggregate) usize {
    return aggregate.maintenance.cleanupRetryScanIndex();
}

fn takeScrollbackCompressionIndexForTesting(
    aggregate: *HostRuntimeAggregate,
    count: usize,
    now_ns: i128,
) ?usize {
    return aggregate.maintenance.takeScrollbackCompressionIndex(count, now_ns);
}

fn requestTerminationUsingForTesting(
    aggregate: *HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    mode: TerminationMode,
    dispatcher: anytype,
    failure_fencer: anytype,
    containment: anytype,
) TerminationError!core_module.TransitionOutcome {
    return termination_ops.requestTerminationUsing(
        aggregate,
        connection_id,
        terminal_id,
        mode,
        dispatcher,
        failure_fencer,
        containment,
    );
}

fn shutdownAndReleaseTerminalEnginesForTesting(owner: anytype) !void {
    var first_error: ?anyerror = null;
    for (0..owner.ownedTerminalCount()) |index| {
        const engine = owner.runtimeAtOwnedTerminal(index) orelse continue;
        engine.shutdown() catch |err| {
            if (first_error == null) first_error = err;
        };
    }
    if (first_error) |err| return err;
    while (owner.ownedTerminalCount() > 0) owner.destroyLastOwnedTerminal();
}

pub const testing = if (target_builtin.is_test) struct {
    pub const cleanupFailureFn = host_runtime_maintenance.cleanupFailure;
    pub const cleanupRetryScanIndexFn = cleanupRetryScanIndexForTesting;
    pub const CleanupClockType = host_runtime_maintenance.CleanupClock;
    pub const scrollbackCompressionOpportunityIntervalNs =
        host_runtime_maintenance.scrollback_compression_opportunity_interval_ns;
    pub const RetainedCleanupResultType = RetainedCleanupResult;
    pub const RetainedCleanupStatusType = RetainedCleanupStatus;

    pub fn finishRetainedCleanup(aggregate: *HostRuntimeAggregate) !void {
        try aggregate.finishRetainedCleanup();
    }

    pub fn markRetainedCleanupExhausted(aggregate: *HostRuntimeAggregate) void {
        aggregate.maintenance.markRetainedCleanupExhausted(aggregate);
    }

    pub fn projectionReleaseDecisions(aggregate: *const HostRuntimeAggregate) usize {
        return aggregate.testing_state.projection_release_decisions;
    }

    pub fn resetProjectionReleaseDecisions(aggregate: *HostRuntimeAggregate) void {
        aggregate.testing_state.projection_release_decisions = 0;
    }

    pub fn setAfterConfigurationCommitHook(
        aggregate: *HostRuntimeAggregate,
        hook: ?*const fn (?*anyopaque, *terminal_runtime.TerminalRuntime) void,
        context: ?*anyopaque,
    ) void {
        aggregate.testing_state.after_configuration_commit = hook;
        aggregate.testing_state.after_configuration_commit_context = context;
    }

    pub fn setAfterResizeCommitHook(
        aggregate: *HostRuntimeAggregate,
        hook: ?*const fn (?*anyopaque, *terminal_runtime.TerminalRuntime) void,
        context: ?*anyopaque,
    ) void {
        aggregate.testing_state.after_resize_commit = hook;
        aggregate.testing_state.after_resize_commit_context = context;
    }

    pub fn retainedCleanupPending(aggregate: *const HostRuntimeAggregate) bool {
        return aggregate.retainedCleanupPending();
    }

    pub const retainedCleanupStatusFn = HostRuntimeAggregate.retainedCleanupStatus;
    pub const retainUnconfirmedCleanupFn = retainUnconfirmedCleanupForTesting;

    pub fn runRetainedCleanupPass(aggregate: *HostRuntimeAggregate) bool {
        return aggregate.runRetainedCleanupPass();
    }

    pub const runRetainedCleanupWindowFn = HostRuntimeAggregate.runRetainedCleanupWindow;

    pub fn setCleanupClockForTesting(
        aggregate: *HostRuntimeAggregate,
        read_ns: *const fn (*host_runtime_maintenance.CleanupClock) i128,
        context: ?*anyopaque,
    ) void {
        aggregate.maintenance.cleanup_clock.read_ns = read_ns;
        aggregate.maintenance.cleanup_clock.context = context;
    }

    pub const recordUnsafeResizeFailureFn = recordUnsafeResizeFailureForTesting;
    pub const configurationObservationIsCompatibleFn =
        configuration_ops.configurationObservationIsCompatible;
    pub const resizeObservationIsCompatibleFn = geometry_ops.resizeObservationIsCompatible;
    pub const adoptRunningTerminalFn = creation_ops.adoptRunningTerminalForTesting;
    pub const requestTerminationUsingFn = requestTerminationUsingForTesting;
    pub const runtimeFailureFromObservationFn =
        host_runtime_maintenance.runtimeFailureFromObservation;
    pub const shutdownAndReleaseTerminalEnginesFn =
        shutdownAndReleaseTerminalEnginesForTesting;
    pub const synchronizeRuntimeCompletionFn =
        host_runtime_maintenance.synchronizeRuntimeCompletion;
    pub const takeDueCleanupRetryFn = takeDueCleanupRetryForTesting;
    pub const takeScrollbackCompressionIndexFn =
        takeScrollbackCompressionIndexForTesting;

    pub fn compressionScanIndex(aggregate: *const HostRuntimeAggregate) usize {
        return aggregate.maintenance.compressionScanIndex();
    }

    pub fn nextCompressionOpportunityAt(aggregate: *const HostRuntimeAggregate) i128 {
        return aggregate.maintenance.nextCompressionOpportunityAt();
    }
} else struct {};
