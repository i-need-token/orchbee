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
    const count = self.core_state.terminalCount();
    const end = offset + @min(maximum_terminals, count - offset);
    for (self.core_state.terminalKeys()[offset..end]) |terminal_id| {
        self.core_state.synchronizeCloseActivity(terminal_id, .unknown);
        if (self.runtimeFor(terminal_id)) |runtime| {
            self.core_state.synchronizeCloseActivity(terminal_id, switch (runtime.sampleCloseActivityForClose()) {
                .idle => .idle,
                .active => .active,
                .unknown => .unknown,
            });
        }
    }
    return .{
        .generation = self.core_state.inventoryGeneration(),
        .host_epoch = self.core_state.epoch,
        .terminals = self.core_state.terminalValues(),
    };
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
