const std = @import("std");
const terminal_input = @import("terminal-input");
const terminal_input_ownership = @import("../input_ownership.zig");
const terminal_input_encoding = @import("../input_encoding.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const prepared_input = @import("prepared_input.zig");
const failure_ops = @import("failure.zig");
const input_admission_ops = @import("input_admission.zig");
const linefeed = @import("linefeed.zig");

/// Coordinates attachment-scoped semantic input with its held-input ledger.
/// Terminal-scoped encoding stays in input_admission; this module owns the
/// additional terminal -> input lock order and atomic queue/ledger commit.
pub fn prepareSemanticInput(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) !prepared_input.Internal {
    if (input == .focus) {
        return prepareFocusInput(
            self,
            attachment_id,
            input.focus.focused,
            maximum_bytes,
        );
    }
    var prepared = try input_admission_ops.prepareSemanticInput(self, input, maximum_bytes);
    errdefer prepared.deinit();
    prepared.ownership = try prepareInputMutation(self, attachment_id, input);
    return prepared;
}

fn prepareFocusInput(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    focused: bool,
    maximum_bytes: usize,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    var buffer: [contracts.max_focus_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;

    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    const focus = try self.input_ownership.reserveFocusMutation(attachment_id, focused);
    var ownership = focus.ownership;
    errdefer self.input_ownership.cancelPrepared(&ownership);

    const encoded = if (!terminal_input_encoding.reportsFocus(&self.terminal))
        ""
    else switch (focus.transition) {
        .none => "",
        .gained => try terminal_input_encoding.encodeFocus(&buffer, true),
        .lost => try terminal_input_encoding.encodeFocus(&buffer, false),
    };
    if (encoded.len > maximum_bytes) return error.InputLimitExceeded;

    const reservation = if (encoded.len == 0)
        null
    else if (focus.transition == .lost)
        try self.input_queue.reservePrechargedClient(encoded.len)
    else
        try self.input_queue.reserve(
            encoded.len,
            .client,
            self.input_ownership.repairCharge(),
        );
    errdefer if (reservation) |pending| {
        std.debug.assert(self.input_queue.cancel(pending.identifier));
    };
    try self.input_ownership.validatePrepared(
        ownership,
        self.input_queue.charge(.client),
    );
    const bytes = if (encoded.len == 0) null else try self.allocator.dupe(u8, encoded);
    return .{
        .bytes = bytes,
        .focus_reporting_enabled = terminal_input_encoding.reportsFocus(&self.terminal),
        .ownership = ownership,
        .reservation = reservation,
        .runtime = self,
    };
}

pub fn prepareRepair(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
) !prepared_input.Internal {
    return prepareCleanup(self, attachment_id, .remove);
}

pub fn prepareReset(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
) !prepared_input.Internal {
    return prepareCleanup(self, attachment_id, .retain);
}

fn prepareCleanup(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    disposition: terminal_input_ownership.RepairDisposition,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;

    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    const repairs_aggregate_focus =
        self.input_ownership.attachmentFocused(attachment_id) and
        self.input_ownership.focusedCount() == 1;
    var repair = try self.input_ownership.prepareRepair(
        self.allocator,
        attachment_id,
        disposition,
        &self.terminal,
        self.cell_pixel_width,
        self.cell_pixel_height,
    );
    errdefer {
        if (repair.bytes) |bytes| self.allocator.free(bytes);
        self.input_ownership.cancelPrepared(&repair.ownership);
    }
    if (repair.bytes) |bytes| {
        const byte_count = try input_admission_ops.expandedInputLengthLocked(
            self,
            bytes,
            std.math.maxInt(usize),
        );
        if (byte_count != bytes.len) {
            const expanded = try linefeed.duplicateExpanded(
                self.allocator,
                bytes,
                true,
                byte_count,
            );
            self.allocator.free(bytes);
            repair.bytes = expanded;
        }
    }
    const reservation = if (repair.bytes) |owned|
        try reserveRepairInputLocked(self, attachment_id, owned.len)
    else
        null;
    return .{
        .bytes = repair.bytes,
        .focus_reporting_enabled = if (repairs_aggregate_focus)
            terminal_input_encoding.reportsFocus(&self.terminal)
        else
            null,
        .ownership = repair.ownership,
        .reservation = reservation,
        .runtime = self,
    };
}

fn prepareInputMutation(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    input: terminal_input.SemanticInput,
) !?contracts.PreparedInputOwnership {
    var mutation = try terminal_input_ownership.mutationForInput(
        self.allocator,
        input,
    ) orelse return null;
    errdefer mutation.deinit(self.allocator);
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    return try self.input_ownership.reserveMutation(attachment_id, &mutation);
}

pub fn hasOwnership(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
) bool {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    return self.input_ownership.contains(attachment_id);
}

fn discardOwnership(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
) void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    self.input_ownership.discard(attachment_id);
}

fn resetOwnership(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
) void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    self.input_ownership.resetRetainingAttachment(attachment_id);
}

pub fn failRepair(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    err: anyerror,
    phase: []const u8,
) void {
    const runtime_already_closed = err == error.TerminalExited or
        err == error.TerminalRuntimeUnavailable or err == error.MissingAttachment;
    if (!runtime_already_closed) {
        std.log.warn("attachment input repair {s}: {s}; fencing the terminal runtime", .{
            phase,
            @errorName(err),
        });
    }
    discardOwnership(self, attachment_id);
    if (!runtime_already_closed) failure_ops.recordWorkerIntegrityFailure(self);
}

pub fn failReset(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    err: anyerror,
    phase: []const u8,
) void {
    const runtime_already_closed = err == error.TerminalExited or
        err == error.TerminalRuntimeUnavailable or err == error.MissingAttachment;
    if (!runtime_already_closed) {
        std.log.warn("attachment input reset {s}: {s}; fencing the terminal runtime", .{
            phase,
            @errorName(err),
        });
    }
    resetOwnership(self, attachment_id);
    if (!runtime_already_closed) failure_ops.recordWorkerIntegrityFailure(self);
}

fn reserveRepairInputLocked(
    self: *state.State,
    attachment_id: contracts.AttachmentId,
    byte_count: usize,
) !contracts.InputReservation {
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    if (self.writer_stop or self.client_input_closed) return error.TerminalExited;
    try self.input_ownership.validateRepairReservation(attachment_id, byte_count);
    return self.input_queue.reservePrechargedClient(byte_count);
}
