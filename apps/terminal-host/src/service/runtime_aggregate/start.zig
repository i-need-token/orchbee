const std = @import("std");
const target_builtin = @import("builtin");

const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const model = @import("terminal-host-model");
const terminal_geometry = model.geometry;
const terminal_runtime = terminal_engine.runtime;
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const StartBuild = contracts.StartBuild;
const StartCommitError = contracts.StartCommitError;
const StartOutcome = contracts.StartOutcome;
const StartPreparation = contracts.StartPreparation;
const StartPrepareError = contracts.StartPrepareError;

pub fn prepareStartTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    geometry: terminal_geometry.Geometry,
) StartPrepareError!StartPreparation {
    const admission = try self.core_state.prepareTerminalStart(
        connection_id,
        terminal_id,
        geometry,
    );
    const terminal = self.core_state.findTerminal(terminal_id) orelse unreachable;
    const identity: contracts.TerminalIdentity = .{
        .creation_id = terminal.creation_id,
        .terminal_id = terminal_id,
    };
    return switch (admission) {
        .replay => .{ .replayed = .{
            .geometry = terminal.startGeometry().?,
            .identity = identity,
            .transition = .already_started,
        } },
        .start => .{ .pending = .{
            .connection_id = connection_id,
            .geometry = geometry,
            .identity = identity,
        } },
    };
}

/// Takes a pending start out of its preparation for a background build. On error the preparation
/// is unchanged.
pub fn beginStartBuild(
    self: anytype,
    preparation: *StartPreparation,
) error{TerminalRuntimeMissing}!StartBuild {
    const pending = preparation.pending;
    const retained = awaitingRecord(self, pending.identity.terminal_id) orelse
        return error.TerminalRuntimeMissing;
    preparation.* = .consumed;
    return .{ .pending = pending, .retained = retained };
}

/// Commits a start whose build returned `runtime`. The terminal must still be the awaiting
/// record the build read; on error nothing changed and the caller owns `runtime`.
pub fn commitStartBuild(
    self: anytype,
    build: *StartBuild,
    runtime: *terminal_runtime.TerminalRuntime,
) error{TerminalRuntimeMissing}!StartOutcome {
    const terminal_id = build.pending.identity.terminal_id;
    if (awaitingRecord(self, terminal_id) != build.retained) return error.TerminalRuntimeMissing;
    build.retained.runtime = runtime;
    build.retained.deinitStartPreparation(self.allocator);
    self.core_state.commitTerminalStart(terminal_id, build.pending.geometry);
    return .{
        .geometry = build.pending.geometry,
        .identity = build.pending.identity,
        .transition = .started,
    };
}

/// The retained record of `terminal_id` while it awaits a start with its launch, or null.
fn awaitingRecord(self: anytype, terminal_id: core_module.TerminalId) ?*RetainedTerminal {
    const session = self.core_state.findTerminal(terminal_id) orelse return null;
    if (session.phase() != .awaiting_start) return null;
    const retained = RetainedTerminal.fromMutableSession(@constCast(session));
    if (retained.start_preparation == null) return null;
    std.debug.assert(retained.runtime == null);
    return retained;
}

/// The synchronous composition of the start stages that aggregate tests drive directly. A failed
/// build leaves the preparation pending, as it was.
pub fn commitPreparedTerminalStart(
    self: anytype,
    preparation: *StartPreparation,
) StartCommitError!StartOutcome {
    if (comptime !target_builtin.is_test) {
        @compileError("synchronous start is available only to service tests");
    }
    switch (preparation.*) {
        .replayed => |outcome| return outcome,
        .consumed => unreachable,
        .pending => {
            var build = try beginStartBuild(self, preparation);
            const runtime = terminal_runtime.TerminalRuntime.createWithOptions(
                self.allocator,
                build.processLaunch(),
                build.runtimeOptions(),
            ) catch |err| {
                preparation.* = .{ .pending = build.pending };
                return if (err == error.OutOfMemory)
                    error.RuntimeAllocationOutOfMemory
                else
                    error.RuntimeSpawnFailed;
            };
            return commitStartBuild(self, &build, runtime) catch unreachable;
        },
    }
}
