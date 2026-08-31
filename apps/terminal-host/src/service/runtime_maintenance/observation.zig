const std = @import("std");
const core_module = @import("terminal-host-core").state;
const pty_process = @import("terminal-host-platform").process;
const terminal_runtime = @import("terminal-host-engine").runtime;
const contracts = @import("contracts.zig");

const TerminalExitNotice = contracts.TerminalExitNotice;
const TerminalOutcomeNotice = contracts.TerminalOutcomeNotice;

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

pub fn cleanupFailureFromPhase(
    phase: terminal_runtime.ShutdownCleanupPhase,
) core_module.RuntimeCleanupFailure {
    return switch (phase) {
        .contain_process => .contain_process,
        .release_output => .release_output,
        .release_process_identity => .release_process_identity,
    };
}

pub fn exitResult(result: pty_process.ExitResult) core_module.ExitResult {
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

pub fn terminalOutcomeNotice(pending: core_module.PendingOutcome) TerminalOutcomeNotice {
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
