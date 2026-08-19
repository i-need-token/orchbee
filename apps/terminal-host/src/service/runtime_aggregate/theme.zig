const core_module = @import("terminal-host-core").state;
const model = @import("terminal-host-model");
const terminal_theme = model.theme;
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const TerminalThemeError = contracts.TerminalThemeError;
const TerminalThemeResult = contracts.TerminalThemeResult;
const TerminalThemeTransition = contracts.TerminalThemeTransition;

pub fn applyTerminalTheme(
    self: anytype,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    operation_id: model.identifiers.TerminalThemeOperationId,
    expected_revision: u64,
    next_theme: terminal_theme.Theme,
) TerminalThemeError!TerminalThemeResult {
    const admission = try self.core_state.prepareTerminalTheme(
        connection_id,
        terminal_id,
        operation_id,
        expected_revision,
        next_theme,
    );
    switch (admission) {
        .already_applied => return result(
            self,
            terminal_id,
            operation_id,
            .already_applied,
        ),
        .stale => return result(self, terminal_id, operation_id, .stale),
        .apply => {},
    }

    const session = self.core_state.findTerminal(terminal_id) orelse unreachable;
    const retained = RetainedTerminal.fromMutableSession(@constCast(session));
    switch (session.phase()) {
        .awaiting_start => try applyPreparedTheme(retained, session.themeState(), next_theme),
        .running, .finishing => try applyRuntimeTheme(
            self,
            connection_id,
            retained,
            operation_id,
            expected_revision,
            next_theme,
        ),
        .containing_failure, .exited, .failed => unreachable,
    }

    self.core_state.commitTerminalTheme(terminal_id, operation_id, next_theme);
    if (retained.runtime) |runtime| {
        _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch unreachable;
    }
    return result(self, terminal_id, operation_id, .applied);
}

fn applyPreparedTheme(
    retained: *RetainedTerminal,
    current: terminal_theme.State,
    next_theme: terminal_theme.Theme,
) TerminalThemeError!void {
    if (retained.runtime != null) return error.RuntimeStateDiverged;
    const preparation = if (retained.start_preparation) |*value|
        value
    else
        return error.TerminalRuntimeMissing;
    if (!terminal_theme.eql(preparation.themeDefaults(), current.terminal)) {
        return error.RuntimeStateDiverged;
    }
    preparation.applyTheme(next_theme) catch return error.InvalidTheme;
}

fn applyRuntimeTheme(
    self: anytype,
    connection_id: core_module.ConnectionId,
    retained: *RetainedTerminal,
    operation_id: model.identifiers.TerminalThemeOperationId,
    expected_revision: u64,
    next_theme: terminal_theme.Theme,
) TerminalThemeError!void {
    if (retained.start_preparation != null) return error.RuntimeStateDiverged;
    const runtime = retained.runtime orelse return error.TerminalRuntimeMissing;
    _ = self.synchronizeRuntimeObservation(retained.session.terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    const readmission = try self.core_state.prepareTerminalTheme(
        connection_id,
        retained.session.terminal_id,
        operation_id,
        expected_revision,
        next_theme,
    );
    if (readmission != .apply) return error.RuntimeStateDiverged;
    if (!terminal_theme.eql(runtime.themeDefaults(), retained.session.themeState().terminal)) {
        return error.RuntimeStateDiverged;
    }
    _ = runtime.applyTheme(next_theme) catch |err| return switch (err) {
        error.InvalidTheme => error.InvalidTheme,
        error.RevisionOverflow => error.TerminalRevisionOverflow,
        error.TerminalExited,
        error.TerminalResizeFailed,
        error.TerminalRuntimeUnavailable,
        => error.TerminalRuntimeUnavailable,
    };
}

fn result(
    self: anytype,
    terminal_id: core_module.TerminalId,
    operation_id: model.identifiers.TerminalThemeOperationId,
    transition: TerminalThemeTransition,
) TerminalThemeResult {
    return .{
        .current = self.core_state.findTerminal(terminal_id).?.themeState(),
        .operation_id = operation_id,
        .terminal_id = terminal_id,
        .transition = transition,
    };
}
