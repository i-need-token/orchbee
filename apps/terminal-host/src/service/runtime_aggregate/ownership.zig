const std = @import("std");
const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const host_retained_terminal = @import("../retained_terminal.zig");
const contracts = @import("contracts.zig");

const RetainedTerminal = host_retained_terminal.RetainedTerminal;
const RetainedCleanupResult = contracts.RetainedCleanupResult;
const RetainedCleanupStatus = contracts.RetainedCleanupStatus;

pub fn deinitChecked(
    self: anytype,
    request_incomplete_termination: bool,
) !void {
    if (request_incomplete_termination) {
        // Runtime destruction escalates to force termination when this
        // graceful request fails or the PTY has not drained.
        for (self.core_state.terminalValues()) |session| {
            const engine = self.runtimeFor(session.terminal_id) orelse continue;
            const completion = engine.completion();
            if (!completion.child_exited or !completion.pty_drained) {
                engine.terminate() catch {};
            }
        }
    }
    shutdownAndReleaseTerminalEngines(self) catch |err| {
        // THDD-001: a failed fatal-teardown pass must never abandon an
        // incomplete shutdown after one failed pass. Retain the
        // cleanup ownership with a bounded retry deadline. A settled
        // terminal records that work independently so its outcome remains
        // immutable; an unsettled failure keeps using lifecycle
        // containment. The complete aggregate is retained for that window.
        self.maintenance.retainUnconfirmedCleanup(self);
        return err;
    };
    self.core_state.deinit();
    self.* = undefined;
}

pub fn retainedCleanupPending(self: anytype) bool {
    return self.maintenance.retainedCleanupPending(self);
}

pub fn retainedCleanupStatus(self: anytype) RetainedCleanupStatus {
    return self.maintenance.retainedCleanupStatus();
}

pub fn runRetainedCleanupPass(self: anytype) bool {
    return self.maintenance.runRetainedCleanupPass(self);
}

pub fn runRetainedCleanupWindow(self: anytype) RetainedCleanupResult {
    return self.maintenance.runRetainedCleanupWindow(self);
}

pub fn finishRetainedCleanup(self: anytype) !void {
    try shutdownAndReleaseTerminalEngines(self);
    self.core_state.deinit();
    self.* = undefined;
}

pub fn ownedTerminalCount(self: anytype) usize {
    return self.core_state.terminalCount();
}

pub fn runtimeAtOwnedTerminal(
    self: anytype,
    index: usize,
) ?*terminal_runtime.TerminalRuntime {
    return self.runtimeFor(self.core_state.terminalValues()[index].terminal_id);
}

pub fn destroyLastOwnedTerminal(self: anytype) void {
    const index = self.core_state.terminalCount() - 1;
    const terminal_id = self.core_state.terminalKeys()[index];
    const testing_model = if (comptime target_builtin.is_test)
        core_module.testing.isModelTerminal(&self.core_state, terminal_id)
    else
        false;
    const session = self.core_state.takeTerminalForHostShutdown(terminal_id);
    if (comptime target_builtin.is_test) {
        if (testing_model) {
            core_module.testing.destroyModelTerminal(&self.core_state, session);
            return;
        }
    }
    const retained = RetainedTerminal.fromMutableSession(session);
    if (retained.runtime) |runtime| runtime.destroyAfterShutdown();
    retained.deinitStartPreparation(self.allocator);
    self.allocator.destroy(retained);
}

pub fn isEmpty(self: anytype) bool {
    return self.core_state.connectionCount() == 0 and
        self.core_state.terminalCount() == 0;
}

pub fn runtimeCount(self: anytype) usize {
    var count: usize = 0;
    for (self.core_state.terminalValues()) |session| {
        if (self.runtimeFor(session.terminal_id) != null) count += 1;
    }
    return count;
}

pub fn runtimeFor(
    self: anytype,
    terminal_id: core_module.TerminalId,
) ?*terminal_runtime.TerminalRuntime {
    const session = self.core_state.findTerminal(terminal_id) orelse return null;
    if (comptime target_builtin.is_test) {
        if (core_module.testing.isModelTerminal(&self.core_state, terminal_id)) return null;
    }
    return RetainedTerminal.fromSession(session).runtime;
}

pub fn shutdownAndReleaseTerminalEngines(owner: anytype) !void {
    var first_error: ?anyerror = null;
    for (0..ownedTerminalCount(owner)) |index| {
        const engine = runtimeAtOwnedTerminal(owner, index) orelse continue;
        engine.shutdown() catch |err| {
            if (first_error == null) first_error = err;
        };
    }
    if (first_error) |err| return err;
    while (ownedTerminalCount(owner) > 0) destroyLastOwnedTerminal(owner);
}
