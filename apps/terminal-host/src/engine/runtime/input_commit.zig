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
) !void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    // Zero-byte prepared semantic inputs share the same post-exit client
    // admission gate so every PreparedInput.commit has one phase boundary.
    if (self.writer_stop or self.client_input_closed or
        self.child_exited.load(.acquire)) return error.TerminalExited;
    try validatePreparedInputOwnershipLocked(self, ownership.*);
    commitPreparedInputOwnershipLocked(self, ownership);
}

pub fn publishInput(self: *state.State, reservation: contracts.InputReservation, bytes: []u8) !void {
    var ownership: ?contracts.PreparedInputOwnership = null;
    return publishPreparedInput(self, reservation, bytes, &ownership);
}

pub fn publishPreparedInput(
    self: *state.State,
    reservation: contracts.InputReservation,
    bytes: []u8,
    ownership: *?contracts.PreparedInputOwnership,
) !void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
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
