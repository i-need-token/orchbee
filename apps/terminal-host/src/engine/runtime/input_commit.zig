const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

/// Facts a prepared input must still match when it publishes.
pub const Expectations = struct {
    focus_report: bool = false,
    focus_reporting_enabled: ?bool = null,
    mouse_input_epoch: ?u64 = null,
    prompt_gate_serial: ?u64 = null,

    /// These facts are terminal state, so publication also holds terminal_mutex.
    fn needsTerminal(self: Expectations) bool {
        return self.focus_reporting_enabled != null or self.mouse_input_epoch != null or
            self.prompt_gate_serial != null;
    }
};

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
    const expectations: Expectations = .{
        .focus_reporting_enabled = focus_reporting_enabled,
        .mouse_input_epoch = mouse_input_epoch,
    };
    lockPreparedInputCommit(self, expectations);
    defer unlockPreparedInputCommit(self, expectations);
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
    return publishPreparedInput(self, reservation, bytes, &ownership, .{});
}

pub fn publishPreparedInput(
    self: *state.State,
    reservation: contracts.InputReservation,
    bytes: []u8,
    ownership: *?contracts.PreparedInputOwnership,
    expectations: Expectations,
) !void {
    lockPreparedInputCommit(self, expectations);
    defer unlockPreparedInputCommit(self, expectations);
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
    validateFocusModeLocked(self, expectations.focus_reporting_enabled) catch |err| {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        return err;
    };
    validateMouseInputEpochLocked(self, expectations.mouse_input_epoch) catch |err| {
        std.debug.assert(self.input_queue.cancel(reservation.identifier));
        self.allocator.free(bytes);
        self.input_condition.broadcast();
        return err;
    };
    validatePromptGateLocked(self, expectations.prompt_gate_serial) catch |err| {
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
    if (source == .client and !expectations.focus_report) self.typed_input_sequence +%= 1;
    commitPreparedInputOwnershipLocked(self, ownership);
    self.input_condition.broadcast();
}

/// Mouse, focus and prompt-gated input validate terminal state under
/// terminal -> input, so their publication commits with the state they were
/// prepared for. Other input needs only input.
fn lockPreparedInputCommit(self: *state.State, expectations: Expectations) void {
    if (expectations.needsTerminal()) self.terminal_mutex.lock();
    self.input_mutex.lock();
}

fn unlockPreparedInputCommit(self: *state.State, expectations: Expectations) void {
    self.input_mutex.unlock();
    if (expectations.needsTerminal()) self.terminal_mutex.unlock();
}

/// Repeats the prompt gate's terminal and input rules at publication. A reader
/// batch in progress, any new prompt mark, or any other client input refuses.
/// The process rule was sampled at preparation and is not sampled again here.
fn validatePromptGateLocked(self: *state.State, expected: ?u64) !void {
    const serial = expected orelse return;
    if (self.reader_commit_in_progress or self.prompt_gate.serial != serial) {
        return error.PromptChanged;
    }
    _ = try self.prompt_gate.check(&self.terminal, .{
        .pending_entries = self.input_queue.charge(.client).entries,
        .sequence = self.typed_input_sequence,
    }, 1);
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
