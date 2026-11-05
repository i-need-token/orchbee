const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

pub fn cancelPreparedInputOwnership(
    self: *state.State,
    pending: *contracts.PreparedInputOwnership,
) void {
    self.input_mutex.lock();
    self.input_ownership.cancelPrepared(pending);
    self.input_mutex.unlock();
}

pub fn validatePreparedInputCommit(
    self: *state.State,
    ownership: *?contracts.PreparedInputOwnership,
    focus_reporting_enabled: ?bool,
    mouse_input_epoch: ?u64,
) !void {
    lockPreparedInputCommit(self, focus_reporting_enabled, mouse_input_epoch);
    defer unlockPreparedInputCommit(self, focus_reporting_enabled, mouse_input_epoch);
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    // Zero-byte prepared semantic inputs share the same post-exit client
    // admission gate so every PreparedInput.commit has one phase boundary.
    if (self.writer_stop or self.client_input_closed or
        self.child_exited.load(.acquire)) return error.TerminalExited;
    try validateFocusModeLocked(self, focus_reporting_enabled);
    try validateMouseInputEpochLocked(self, mouse_input_epoch);
    try validatePreparedInputOwnershipLocked(self, ownership.*);
    commitPreparedInputOwnershipLocked(self, ownership);
}

pub fn publishInput(self: *state.State, reservation: contracts.InputReservation, bytes: []u8) !void {
    var ownership: ?contracts.PreparedInputOwnership = null;
    return publishPreparedInput(self, reservation, bytes, &ownership, null, null);
}

pub fn publishPreparedInput(
    self: *state.State,
    reservation: contracts.InputReservation,
    bytes: []u8,
    ownership: *?contracts.PreparedInputOwnership,
    focus_reporting_enabled: ?bool,
    mouse_input_epoch: ?u64,
) !void {
    lockPreparedInputCommit(self, focus_reporting_enabled, mouse_input_epoch);
    defer unlockPreparedInputCommit(self, focus_reporting_enabled, mouse_input_epoch);
    const source = self.input_queue.reservationSource(reservation.identifier) orelse {
        self.allocator.free(bytes);
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        return error.TerminalExited;
    };
    // Root exit closes client admission under this same mutex. A client
    // reservation prepared while running must still be rejected at this
    // final publication boundary, even while a PTY-holding descendant
    // keeps writer_stop false. child_exited is the running-to-finishing
    // fact itself; including it closes the window before closeClientInput
    // publishes. Terminal-generated replies remain publishable until the
    // writer actually stops.
    if (self.failure_fenced or self.writer_stop or
        (source == .client and
            (self.client_input_closed or self.child_exited.load(.acquire))))
    {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        return error.TerminalExited;
    }
    validateFocusModeLocked(self, focus_reporting_enabled) catch |err| {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        return err;
    };
    validateMouseInputEpochLocked(self, mouse_input_epoch) catch |err| {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        return err;
    };
    validatePreparedInputOwnershipLocked(self, ownership.*) catch |err| {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        return err;
    };
    std.debug.assert(self.input_queue.publish(reservation, bytes));
    commitPreparedInputOwnershipLocked(self, ownership);
    self.input_condition.broadcast();
}

/// Mouse and focus input validate terminal mode under terminal -> input, so
/// their publication commits with the mode they were encoded for. Other input
/// needs only input.
fn lockPreparedInputCommit(
    self: *state.State,
    focus_reporting_enabled: ?bool,
    mouse_input_epoch: ?u64,
) void {
    if (focus_reporting_enabled != null or mouse_input_epoch != null) self.terminal_mutex.lock();
    self.input_mutex.lock();
}

fn unlockPreparedInputCommit(
    self: *state.State,
    focus_reporting_enabled: ?bool,
    mouse_input_epoch: ?u64,
) void {
    self.input_mutex.unlock();
    if (focus_reporting_enabled != null or mouse_input_epoch != null) self.terminal_mutex.unlock();
}

fn validateMouseInputEpochLocked(self: *const state.State, expected: ?u64) !void {
    const epoch = expected orelse return;
    if (self.mouse_input_epoch_exhausted) return error.TerminalRuntimeUnavailable;
    if (self.mouse_input_epoch != epoch) return error.MouseModeChanged;
}

fn validateFocusModeLocked(
    self: *const state.State,
    expected: ?bool,
) !void {
    const enabled = expected orelse return;
    if (self.terminal.modes.get(.focus_event) != enabled) {
        return error.FocusModeChanged;
    }
}

pub fn validatePreparedInputOwnershipLocked(
    self: *state.State,
    ownership: ?contracts.PreparedInputOwnership,
) !void {
    return self.input_ownership.validatePrepared(
        ownership,
        self.input_queue.charge(.client),
    );
}

pub fn commitPreparedInputOwnershipLocked(
    self: *state.State,
    ownership: *?contracts.PreparedInputOwnership,
) void {
    self.input_ownership.commitPrepared(ownership);
}

pub fn cancelInput(self: *state.State, identifier: u64) void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.input_queue.cancel(identifier)) self.input_condition.broadcast();
}

pub fn clearInputQueueLocked(self: *state.State) void {
    self.input_queue.clear();
    self.input_condition.broadcast();
}

pub fn inputRepairCharge(self: *state.State, source: contracts.InputSource) contracts.InputCharge {
    return switch (source) {
        .client => self.input_ownership.repairCharge(),
        .terminal => .{},
    };
}
