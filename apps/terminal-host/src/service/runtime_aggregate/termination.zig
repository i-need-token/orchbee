const std = @import("std");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const DisposeError = contracts.DisposeError;
const DisposeResult = contracts.DisposeResult;
const TerminationError = contracts.TerminationError;
const TerminationMode = contracts.TerminationMode;

pub fn requestTermination(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    mode: TerminationMode,
) TerminationError!core_module.TransitionOutcome {
    var dispatcher: SystemTerminationDispatcher = .{};
    var failure_fencer: RuntimeFailureFencer = .{};
    var containment: RuntimeFailureContainment = .{ .maintenance = &self.maintenance };
    return requestTerminationUsing(
        self,
        connection_id,
        terminal_id,
        mode,
        &dispatcher,
        &failure_fencer,
        &containment,
    );
}

pub fn requestTerminationUsing(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    mode: TerminationMode,
    dispatcher: anytype,
    failure_fencer: anytype,
    containment: anytype,
) TerminationError!core_module.TransitionOutcome {
    // THCR-036: the bounded background scan may not have visited this
    // exact terminal. Copy an already-published engine classification
    // (normal completion or failure) and complete or record containment
    // before any Core admission or dispatch decision, so a skipped target
    // is never dispatched against a stale running/finishing lifecycle.
    self.synchronizeTerminal(terminal_id);
    const request: core_module.TerminationRequest = switch (mode) {
        .graceful => .graceful,
        .force => .force,
    };
    const admission = try self.core_state.prepareTermination(
        connection_id,
        terminal_id,
        request,
    );
    if (admission == .replay) return .idempotent;

    const runtime = self.runtimeFor(terminal_id) orelse {
        const failure: core_module.RuntimeFailure = .synchronize_worker;
        _ = self.core_state.runtimeFailed(terminal_id, failure) catch {};
        _ = self.core_state.failureContainmentBlocked(
            terminal_id,
            .contain_process,
            self.maintenance.nextCleanupRetryAt(),
        ) catch {};
        return error.TerminalRuntimeMissing;
    };
    if (admission == .containment_retry) {
        containment.contain(&self.core_state, terminal_id, runtime) catch
            return error.TerminationDispatchFailed;
        return .applied;
    }
    dispatcher.dispatch(runtime, mode) catch {
        // The engine gate is the first visible failure transition. Core
        // publishes the Host-side primary cause only after admission,
        // writer start, and reader mutation are irreversibly closed.
        switch (failure_fencer.fence(runtime)) {
            .host_won => self.core_state.commitTerminationDispatchFailure(terminal_id),
            .engine_won => |failure| {
                const transition = self.core_state.runtimeFailed(
                    terminal_id,
                    host_runtime_maintenance.runtimeFailureFromEngineKind(failure),
                ) catch unreachable;
                std.debug.assert(transition == .applied);
            },
        }
        containment.contain(&self.core_state, terminal_id, runtime) catch {};
        return error.TerminationDispatchFailed;
    };
    self.core_state.commitPreparedTermination(terminal_id, request);
    return .applied;
}

pub fn disposeTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
) DisposeError!DisposeResult {
    // THCR-036: the bounded background scan may not have visited this
    // exact terminal. Synchronize its already-published engine
    // classification and complete or record containment before the
    // disposal phase decision, so a settled target is never rejected as
    // invalid-phase against a stale running/finishing lifecycle.
    self.synchronizeTerminal(terminal_id);
    const admission = try self.core_state.validateTerminalDisposal(
        connection_id,
        terminal_id,
    );
    switch (admission) {
        .idempotent => return .{ .transition = .idempotent },
        // An already-blocked cleanup retains its exact failing phase.
        // Return it directly before any runtime lookup or shutdown; the
        // independent maintenance pass retries cleanup and this request
        // never mutates or retries the blocked phase.
        .cleanup_blocked => |failure| return .{ .cleanup_blocked = failure },
        .containment_stopping => return error.FailureContainmentIncomplete,
        .dispose => {},
    }
    const awaiting_start = self.core_state.findTerminal(terminal_id).?.phase() == .awaiting_start;
    const runtime = self.runtimeFor(terminal_id);

    if (runtime == null) {
        if (!awaiting_start) return error.TerminalRuntimeMissing;
        self.releaseStartResources(terminal_id) catch {
            _ = self.core_state.retainedCleanupBlocked(
                terminal_id,
                .release_editor_document,
                self.maintenance.nextCleanupRetryAt(),
            ) catch unreachable;
            return .{ .cleanup_blocked = .release_editor_document };
        };
        const session = self.core_state.commitTerminalRemoval(terminal_id);
        const retained = RetainedTerminal.fromMutableSession(session);
        retained.deinitStartPreparation(self.allocator);
        self.allocator.destroy(retained);
        return .{ .transition = .applied };
    }

    // Dispose is the final owner handoff. Join the engine while the
    // RetainedTerminal still binds it to this session; a failed shutdown
    // leaves the complete retained record intact for a safe retry.
    runtime.?.shutdown() catch |err| {
        const cleanup_failure = host_runtime_maintenance.cleanupFailure(err);
        _ = self.core_state.retainedCleanupBlocked(
            terminal_id,
            cleanup_failure,
            self.maintenance.nextCleanupRetryAt(),
        ) catch unreachable;
        return .{ .cleanup_blocked = cleanup_failure };
    };
    const session = self.core_state.commitTerminalRemoval(terminal_id);
    const retained = RetainedTerminal.fromMutableSession(session);
    std.debug.assert(retained.runtime == runtime.?);
    runtime.?.destroyAfterShutdown();
    self.allocator.destroy(retained);
    return .{ .transition = .applied };
}

const SystemTerminationDispatcher = struct {
    fn dispatch(
        _: *SystemTerminationDispatcher,
        runtime: *terminal_runtime.TerminalRuntime,
        mode: TerminationMode,
    ) !void {
        try dispatchTermination(runtime, mode);
    }
};

const RuntimeFailureFencer = struct {
    fn fence(
        _: *RuntimeFailureFencer,
        runtime: *terminal_runtime.TerminalRuntime,
    ) terminal_runtime.HostFailureFenceResult {
        return runtime.fenceHostFailure();
    }
};

const RuntimeFailureContainment = struct {
    maintenance: *host_runtime_maintenance.Maintenance,

    fn contain(
        self: *RuntimeFailureContainment,
        core: *core_module.HostCore,
        terminal_id: core_module.TerminalId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !void {
        try self.maintenance.containRuntimeFailure(core, terminal_id, runtime);
    }
};

fn dispatchTermination(
    runtime: *terminal_runtime.TerminalRuntime,
    mode: TerminationMode,
) !void {
    switch (mode) {
        .graceful => try runtime.terminate(),
        .force => try runtime.forceTerminate(),
    }
}
