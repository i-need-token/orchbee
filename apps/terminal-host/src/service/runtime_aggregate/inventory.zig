const std = @import("std");
const core_module = @import("terminal-host-core").state;
const contracts = @import("contracts.zig");

const TerminalInventory = contracts.TerminalInventory;

pub fn core(self: anytype) *core_module.HostCore {
    return &self.core_state;
}

pub fn coreRead(self: anytype) *const core_module.HostCore {
    return &self.core_state;
}

pub fn hostEpoch(self: anytype) core_module.HostEpoch {
    return self.core_state.epoch;
}

pub fn inventoryGeneration(self: anytype) u64 {
    return self.core_state.inventoryGeneration();
}

pub fn terminalCount(self: anytype) usize {
    return self.core_state.terminalCount();
}

pub fn terminalInventory(
    self: anytype,
    offset: usize,
    maximum_terminals: usize,
) error{InvalidInventoryOffset}!TerminalInventory {
    if (offset > self.core_state.terminalCount()) return error.InvalidInventoryOffset;
    self.synchronizeTerminalRange(offset, maximum_terminals);
    return .{
        .generation = self.core_state.inventoryGeneration(),
        .host_epoch = self.core_state.epoch,
        .terminals = self.core_state.terminalValues(),
    };
}

/// Look up one retained terminal. An exact read checks both indexes before synchronizing it.
pub fn readTerminalLifecycle(
    self: anytype,
    terminal_id: core_module.TerminalId,
    creation_id: ?core_module.CreationId,
) error{ MissingResource, IdentityConflict }!*const core_module.TerminalSession {
    const terminal = self.core_state.findTerminal(terminal_id) orelse {
        if (creation_id) |id| {
            if (self.core_state.terminalForCreation(id) != null) return error.IdentityConflict;
        }
        return error.MissingResource;
    };
    if (creation_id) |id| {
        if (!terminal.creation_id.eql(id)) return error.IdentityConflict;
    }
    self.synchronizeTerminal(terminal_id);
    return self.core_state.findTerminal(terminal_id) orelse error.MissingResource;
}

pub fn connect(
    self: anytype,
    connection_id: core_module.ConnectionId,
    client_id: core_module.ClientId,
) core_module.CoreError!core_module.TransitionOutcome {
    return self.core_state.connect(connection_id, client_id);
}

pub fn commitOutcomePublications(
    self: anytype,
    tokens: []const core_module.OutcomePublicationToken,
) void {
    for (tokens) |token| {
        const transition = self.core_state.markOutcomePublished(token);
        std.debug.assert(transition == .applied);
    }
}
