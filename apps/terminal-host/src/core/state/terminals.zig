const std = @import("std");
const target_builtin = @import("builtin");
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;
const terminal_geometry = model.geometry;
const terminal_theme = model.theme;
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const ExitResult = contracts.ExitResult;
const OutcomePublicationToken = contracts.OutcomePublicationToken;
const ProcessObservation = contracts.ProcessObservation;
const RuntimeCleanupFailure = contracts.RuntimeCleanupFailure;
const RuntimeFailure = contracts.RuntimeFailure;
const TerminalDisposalAdmission = contracts.TerminalDisposalAdmission;
const TerminalId = contracts.TerminalId;
const TerminalThemeOperationId = contracts.TerminalThemeOperationId;
const TerminalSession = contracts.TerminalSession;
const StartAdmission = contracts.StartAdmission;
const TerminationAdmission = contracts.TerminationAdmission;
const TerminationRequest = contracts.TerminationRequest;
const TransitionOutcome = contracts.TransitionOutcome;
const ThemeAdmission = contracts.ThemeAdmission;

pub fn prepareTerminalTheme(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    operation_id: TerminalThemeOperationId,
    expected_revision: u64,
    next_theme: terminal_theme.Theme,
) CoreError!ThemeAdmission {
    if (!terminal_theme.valid(next_theme)) return error.InvalidTheme;
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const terminal = try storage.requireTerminal(self, terminal_id);
    const current = terminal.themeState();
    if (current.operation_id) |current_operation_id| {
        if (current_operation_id.eql(operation_id)) {
            if (terminal_theme.eql(current.terminal, next_theme)) return .already_applied;
            return error.ThemeOperationConflict;
        }
    }
    if (!terminal.acceptsThemeUpdate()) return error.InvalidPhase;
    if (current.revision != expected_revision) return .stale;
    if (current.revision == std.math.maxInt(u64)) return error.RevisionOverflow;
    return .apply;
}

pub fn commitTerminalTheme(
    self: anytype,
    terminal_id: TerminalId,
    operation_id: TerminalThemeOperationId,
    next_theme: terminal_theme.Theme,
) void {
    std.debug.assert(terminal_theme.valid(next_theme));
    const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
    std.debug.assert(terminal.acceptsThemeUpdate());
    std.debug.assert(terminal.theme_state.revision < std.math.maxInt(u64));
    terminal.theme_state = .{
        .operation_id = operation_id,
        .revision = terminal.theme_state.revision + 1,
        .terminal = next_theme,
    };
    if (self.broker) |*broker| broker.invalidateTerminalPresentation(terminal_id);
}

pub fn prepareTerminalStart(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    geometry: terminal_geometry.Geometry,
) CoreError!StartAdmission {
    if (!terminal_geometry.isValid(geometry)) return error.InvalidPhase;
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const terminal = try storage.requireTerminal(self, terminal_id);
    if (terminal.phase() == .awaiting_start) return .start;
    const initial_geometry = terminal.startGeometry() orelse return error.InvalidPhase;
    return if (terminal_geometry.eql(initial_geometry, geometry))
        .replay
    else
        error.StartConflict;
}

pub fn commitTerminalStart(
    self: anytype,
    terminal_id: TerminalId,
    geometry: terminal_geometry.Geometry,
) void {
    std.debug.assert(terminal_geometry.isValid(geometry));
    const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
    const transition = terminal.lifecycle.start() catch unreachable;
    std.debug.assert(transition == .applied);
    terminal.engine_observation = .{ .geometry = geometry };
    terminal.start_geometry = geometry;
}

pub fn updateTerminalConfiguration(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    configuration: terminal_configuration.Configuration,
) CoreError!TransitionOutcome {
    if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const terminal = try storage.requireTerminal(self, terminal_id);
    if (!terminal.acceptsConfigurationUpdate()) return error.InvalidPhase;
    if (terminal.effectiveConfiguration().scrollback_bytes == configuration.scrollback_bytes) {
        return .idempotent;
    }
    if (terminal.configurationRevision() == std.math.maxInt(u64)) {
        return error.RevisionOverflow;
    }
    terminal.configuration_state.effective = configuration;
    terminal.configuration_state.revision += 1;
    return .applied;
}

pub fn requestTerminate(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
) CoreError!TransitionOutcome {
    switch (try self.prepareTermination(connection_id, terminal_id, .graceful)) {
        .replay => return .idempotent,
        .dispatch => {
            self.commitPreparedTermination(terminal_id, .graceful);
            return .applied;
        },
        .containment_retry => unreachable,
    }
}

pub fn requestForceTerminate(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
) CoreError!TransitionOutcome {
    switch (try self.prepareTermination(connection_id, terminal_id, .force)) {
        .replay => return .idempotent,
        .dispatch => {
            self.commitPreparedTermination(terminal_id, .force);
            return .applied;
        },
        .containment_retry => {
            const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
            terminal.lifecycle.commitContainmentRetry();
            return .applied;
        },
    }
}

pub fn prepareTermination(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
    request: TerminationRequest,
) CoreError!TerminationAdmission {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const terminal = try storage.requireTerminal(self, terminal_id);
    if (terminal.phase() == .awaiting_start) return error.InvalidPhase;
    std.debug.assert(request != .none);
    return terminal.lifecycle.prepareTermination(request);
}

pub fn commitPreparedTermination(
    self: anytype,
    terminal_id: TerminalId,
    request: TerminationRequest,
) void {
    std.debug.assert(request != .none);
    const terminal = self.terminal_registry.find(terminal_id) orelse unreachable;
    terminal.lifecycle.commitPreparedTermination(request);
}

pub fn commitTerminationDispatchFailure(
    self: anytype,
    terminal_id: TerminalId,
) void {
    const transition = self.runtimeFailed(terminal_id, .dispatch_termination) catch unreachable;
    std.debug.assert(transition == .applied);
}

pub fn childExited(
    self: anytype,
    terminal_id: TerminalId,
    result: ExitResult,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.lifecycle.childExit(result, terminal.observedRevision());
}

pub fn ptyDrained(self: anytype, terminal_id: TerminalId) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.lifecycle.ptyDrain(terminal.observedRevision());
}

pub fn disposeTerminal(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
) CoreError!TransitionOutcome {
    if (comptime !target_builtin.is_test) {
        @compileError("HostCore.disposeTerminal is available only to deterministic Core tests");
    }
    const admission = try self.validateTerminalDisposal(connection_id, terminal_id);
    switch (admission) {
        .idempotent => return .idempotent,
        // The Core convenience path has no cleanup-phase payload, so both
        // containment barriers reject as incomplete containment here;
        // HostRuntimeAggregate carries the typed blocked phase to the
        // service boundary instead.
        .containment_stopping, .cleanup_blocked => return error.FailureContainmentIncomplete,
        .dispose => {},
    }
    const terminal = self.commitTerminalRemoval(terminal_id);
    storage.destroyTestingTerminalStorage(self, terminal);
    return .applied;
}

pub fn commitTerminalRemoval(self: anytype, terminal_id: TerminalId) *TerminalSession {
    const terminal = self.terminal_registry.get(terminal_id).?;
    std.debug.assert(terminal.phase() == .awaiting_start or
        terminal.phase() == .exited or terminal.phase() == .failed);
    std.debug.assert(terminal.pendingOutcome() == null);
    std.debug.assert(!terminal.retainedCleanupPending());
    storage.removeTerminalAttachments(self, terminal_id);
    return self.terminal_registry.removeAssumePresent(terminal_id, true);
}

pub fn takeTerminalForHostShutdown(
    self: anytype,
    terminal_id: TerminalId,
) *TerminalSession {
    storage.removeTerminalAttachments(self, terminal_id);
    return self.terminal_registry.removeAssumePresent(terminal_id, false);
}

pub fn validateTerminalDisposal(
    self: anytype,
    connection_id: ConnectionId,
    terminal_id: TerminalId,
) CoreError!TerminalDisposalAdmission {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    const terminal = self.terminal_registry.get(terminal_id) orelse return .idempotent;
    if (terminal.retainedCleanupFailure()) |failure| {
        return .{ .cleanup_blocked = failure };
    }
    switch (try terminal.lifecycle.requireDisposable()) {
        .dispose => {},
        .containment_stopping => return .containment_stopping,
        .cleanup_blocked => |failure| return .{ .cleanup_blocked = failure },
    }
    try self.terminal_registry.requireGenerationAdvance();
    return .dispose;
}

pub fn runtimeFailed(
    self: anytype,
    terminal_id: TerminalId,
    failure: RuntimeFailure,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.lifecycle.runtimeFailed(failure);
}

pub fn failureContainmentBlocked(
    self: anytype,
    terminal_id: TerminalId,
    cleanup_failure: RuntimeCleanupFailure,
    retry_at_ns: i128,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.lifecycle.containmentBlocked(cleanup_failure, retry_at_ns);
}

pub fn failureContained(
    self: anytype,
    terminal_id: TerminalId,
    observation: ProcessObservation,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.lifecycle.containmentCompleted(
        observation,
        terminal.observedRevision(),
    );
}

pub fn retainedCleanupBlocked(
    self: anytype,
    terminal_id: TerminalId,
    cleanup_failure: RuntimeCleanupFailure,
    retry_at_ns: i128,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    if (terminal.settledFinalRevision() == null) return error.InvalidPhase;
    return terminal.retained_cleanup.block(cleanup_failure, retry_at_ns);
}

pub fn retainedCleanupCompleted(
    self: anytype,
    terminal_id: TerminalId,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.retained_cleanup.completeCleanup();
}

pub fn retainedCleanupExhausted(
    self: anytype,
    terminal_id: TerminalId,
) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    return terminal.retained_cleanup.exhaust();
}

pub fn synchronizeRevision(self: anytype, terminal_id: TerminalId, revision: u64) CoreError!TransitionOutcome {
    const terminal = try storage.requireTerminal(self, terminal_id);
    switch (terminal.phase()) {
        .running, .finishing, .containing_failure => {},
        .awaiting_start => return error.InvalidPhase,
        .exited, .failed => {
            // Settled engines may still serve retained read-only state,
            // but their final revision is immutable. Accepting the exact
            // value is a coherence check, never a new mutation.
            if (revision == terminal.observedRevision()) return .idempotent;
            return error.InvalidPhase;
        },
    }
    if (revision < terminal.observedRevision()) return error.InvalidPhase;
    if (revision == terminal.observedRevision()) return .idempotent;
    terminal.engine_observation.?.revision = revision;
    return .applied;
}

pub fn synchronizeGeometry(
    self: anytype,
    terminal_id: TerminalId,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    revision: u64,
) CoreError!void {
    const terminal = try storage.requireTerminal(self, terminal_id);
    const observed_geometry = terminal.observedGeometry();
    const geometry_changed = observed_geometry.columns != columns or
        observed_geometry.rows != rows or
        observed_geometry.cell_pixel_width != cell_pixel_width or
        observed_geometry.cell_pixel_height != cell_pixel_height;
    const revision_changed = terminal.observedRevision() != revision;
    switch (terminal.phase()) {
        .running, .finishing, .containing_failure => {},
        .awaiting_start => return error.InvalidPhase,
        .exited, .failed => {
            if (!geometry_changed and !revision_changed) return;
            return error.InvalidPhase;
        },
    }
    if (revision < terminal.observedRevision()) return error.InvalidPhase;
    if (geometry_changed and !revision_changed) return error.InvalidPhase;
    if (!geometry_changed and !revision_changed) return;
    terminal.engine_observation.?.geometry = .{
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .columns = columns,
        .rows = rows,
    };
    terminal.engine_observation.?.revision = revision;
    if (geometry_changed) {
        terminal.engine_observation.?.geometry_changed_at_revision = revision;
    }
}

pub fn markOutcomePublished(
    self: anytype,
    token: OutcomePublicationToken,
) TransitionOutcome {
    const terminal = self.terminal_registry.find(token.terminal_id) orelse return .idempotent;
    const final_revision = terminal.settledFinalRevision() orelse return .idempotent;
    if (final_revision != token.final_revision or
        terminal.observedRevision() != final_revision)
    {
        return .idempotent;
    }
    return terminal.lifecycle.markOutcomePublished(token.kind);
}

pub fn findTerminal(self: anytype, terminal_id: TerminalId) ?*const TerminalSession {
    return self.terminal_registry.get(terminal_id);
}

pub fn terminalCount(self: anytype) usize {
    return self.terminal_registry.count();
}

pub fn terminalKeys(self: anytype) []const TerminalId {
    return self.terminal_registry.keys();
}

pub fn terminalValues(self: anytype) []const *const TerminalSession {
    return self.terminal_registry.values();
}

pub fn inventoryGeneration(self: anytype) u64 {
    return self.terminal_registry.generation();
}
