const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const contracts = @import("contracts.zig");

const PendingOutcomeBatch = contracts.PendingOutcomeBatch;

pub fn synchronizeRuntimeObservation(
    self: anytype,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) core_module.CoreError!terminal_runtime.Geometry {
    return host_runtime_maintenance.synchronizeRuntimeObservation(
        &self.core_state,
        terminal_id,
        runtime,
    );
}

pub fn observeRuntime(
    self: anytype,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) ?terminal_runtime.Geometry {
    return host_runtime_maintenance.observeRuntime(&self.core_state, terminal_id, runtime);
}

pub fn noteRefusedObservation(
    self: anytype,
    terminal_id: core_module.TerminalId,
    err: core_module.CoreError,
) void {
    host_runtime_maintenance.noteRefusedObservation(&self.core_state, terminal_id, err);
}

pub fn synchronizeRuntimes(self: anytype) void {
    self.maintenance.synchronizeRuntimes(self);
}

pub fn synchronizeTerminalRange(
    self: anytype,
    offset: usize,
    maximum_terminals: usize,
) void {
    self.maintenance.synchronizeTerminalRange(
        self,
        offset,
        maximum_terminals,
    );
}

pub fn pendingOutcomes(
    self: anytype,
    maximum_notices: usize,
) PendingOutcomeBatch {
    return self.maintenance.pendingOutcomes(&self.core_state, maximum_notices);
}

pub fn synchronizeTerminal(
    self: anytype,
    terminal_id: core_module.TerminalId,
) void {
    self.maintenance.synchronizeTerminal(self, terminal_id);
}
