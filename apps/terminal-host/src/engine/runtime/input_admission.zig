const std = @import("std");
const terminal_input = @import("terminal-input");
const terminal_input_encoding = @import("../input_encoding.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const prepared_input = @import("prepared_input.zig");
const failure_ops = @import("failure.zig");
const input_commit_ops = @import("input_commit.zig");
const linefeed = @import("linefeed.zig");

pub fn enqueueInput(self: *state.State, bytes: []const u8) !void {
    _ = try enqueueRawInput(self, bytes);
}

pub fn enqueueRawInput(self: *state.State, bytes: []const u8) !usize {
    var prepared = try prepareRawInput(self, bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn prepareRawInput(self: *state.State, bytes: []const u8) !prepared_input.Internal {
    if (bytes.len == 0) return error.EmptyInput;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    return prepareEncodedInputLocked(self, bytes, true, std.math.maxInt(usize));
}

pub fn enqueueKeyInput(self: *state.State, input: contracts.KeyInput) !usize {
    return enqueueKeyInputWithLimit(self, input, std.math.maxInt(usize));
}

pub fn keyInputWouldEmit(self: *state.State, input: contracts.KeyInput) !bool {
    const key = try terminal_input_encoding.parseKey(input.key);
    var buffer: [contracts.max_key_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    const encoded = try terminal_input_encoding.encodeKey(
        &buffer,
        &self.terminal,
        key,
        input,
    );
    return encoded.len != 0;
}

pub fn enqueueKeyInputWithLimit(
    self: *state.State,
    input: contracts.KeyInput,
    max_output_bytes: usize,
) !usize {
    var prepared = try prepareKeyInput(self, input, max_output_bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn prepareKeyInput(
    self: *state.State,
    input: contracts.KeyInput,
    max_output_bytes: usize,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    const key = try terminal_input_encoding.parseKey(input.key);
    var buffer: [contracts.max_key_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    const encoded = try terminal_input_encoding.encodeKey(
        &buffer,
        &self.terminal,
        key,
        input,
    );
    return prepareEncodedInputLocked(self, encoded, true, max_output_bytes);
}

pub fn enqueueTextInput(self: *state.State, text: []const u8) !usize {
    return enqueueTextInputWithLimit(self, text, std.math.maxInt(usize));
}

pub fn enqueueTextInputWithLimit(
    self: *state.State,
    text: []const u8,
    max_output_bytes: usize,
) !usize {
    var prepared = try prepareTextInput(self, text, max_output_bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn prepareTextInput(
    self: *state.State,
    text: []const u8,
    max_output_bytes: usize,
) !prepared_input.Internal {
    if (text.len == 0) return error.EmptyInput;
    if (text.len > max_output_bytes) return error.InputLimitExceeded;
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    return prepareEncodedInputLocked(self, text, true, max_output_bytes);
}

pub fn enqueueMouseInput(self: *state.State, input: contracts.MouseInput) !usize {
    return enqueueMouseInputWithLimit(self, input, std.math.maxInt(usize));
}

pub fn mouseInputWouldEmit(self: *state.State, input: contracts.MouseInput) !bool {
    var buffer: [contracts.max_mouse_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    const encoded = try terminal_input_encoding.encodeMouse(
        &buffer,
        &self.terminal,
        self.cell_pixel_width,
        self.cell_pixel_height,
        input,
    );
    return encoded.len != 0;
}

pub fn enqueueMouseInputWithLimit(
    self: *state.State,
    input: contracts.MouseInput,
    max_output_bytes: usize,
) !usize {
    var prepared = try prepareMouseInput(self, input, max_output_bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn prepareMouseInput(
    self: *state.State,
    input: contracts.MouseInput,
    max_output_bytes: usize,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    var buffer: [contracts.max_mouse_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    const encoded = try terminal_input_encoding.encodeMouse(
        &buffer,
        &self.terminal,
        self.cell_pixel_width,
        self.cell_pixel_height,
        input,
    );
    return prepareEncodedInputLocked(self, encoded, true, max_output_bytes);
}

pub fn enqueueFocusInput(self: *state.State, focused: bool) !usize {
    return enqueueFocusInputWithLimit(self, focused, std.math.maxInt(usize));
}

pub fn focusInputWouldEmit(self: *state.State) bool {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    return terminal_input_encoding.reportsFocus(&self.terminal);
}

pub fn enqueueFocusInputWithLimit(
    self: *state.State,
    focused: bool,
    max_output_bytes: usize,
) !usize {
    var prepared = try prepareFocusInput(self, focused, max_output_bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn prepareFocusInput(
    self: *state.State,
    focused: bool,
    max_output_bytes: usize,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    var buffer: [contracts.max_focus_encoded_bytes]u8 = undefined;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    const reporting_enabled = terminal_input_encoding.reportsFocus(&self.terminal);
    const encoded = if (reporting_enabled)
        try terminal_input_encoding.encodeFocus(&buffer, focused)
    else
        "";
    var prepared = try prepareEncodedInputLocked(self, encoded, false, max_output_bytes);
    prepared.focus_reporting_enabled = reporting_enabled;
    return prepared;
}

pub fn enqueuePasteInput(
    self: *state.State,
    text: []const u8,
    bracketed: bool,
) !usize {
    return enqueuePasteInputWithLimit(self, text, bracketed, std.math.maxInt(usize));
}

pub fn enqueuePasteInputWithLimit(
    self: *state.State,
    text: []const u8,
    bracketed: bool,
    max_output_bytes: usize,
) !usize {
    var prepared = try preparePasteInput(self, text, bracketed, max_output_bytes);
    defer prepared.deinit();
    return prepared.commit();
}

pub fn preparePasteInput(
    self: *state.State,
    text: []const u8,
    bracketed: bool,
    max_output_bytes: usize,
) !prepared_input.Internal {
    if (text.len == 0) return error.EmptyInput;
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.terminal.modes.get(.bracketed_paste) != bracketed) {
        return error.PasteModeChanged;
    }
    const encoded = try terminal_input_encoding.encodePaste(
        self.allocator,
        &self.terminal,
        text,
        max_output_bytes,
    );
    return prepareOwnedInputLocked(self, encoded, true, max_output_bytes);
}

pub fn prepareSemanticInput(
    self: *state.State,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) !prepared_input.Internal {
    return switch (input) {
        .focus => |value| prepareFocusInput(self, value.focused, maximum_bytes),
        .key => |value| prepareKeyInput(self, value, maximum_bytes),
        .mouse => |value| prepareMouseInput(self, value, maximum_bytes),
        .paste => |value| preparePasteInput(self, value.text, value.bracketed, maximum_bytes),
        .text => |value| prepareTextInput(self, value.text, maximum_bytes),
    };
}

pub fn pendingInputBytes(self: *state.State) usize {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    return self.input_queue.pendingBytes();
}

pub fn takeWrittenBytes(self: *state.State) usize {
    return self.written_since_sync.swap(0, .acq_rel);
}

pub fn takeOutputBatches(self: *state.State) u64 {
    return self.output_batches_since_sync.swap(0, .acq_rel);
}

pub fn prepareEncodedInputLocked(
    self: *state.State,
    bytes: []const u8,
    affects_close_activity: bool,
    maximum_bytes: usize,
) !prepared_input.Internal {
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    if (bytes.len == 0) return .{
        .affects_close_activity = affects_close_activity,
        .bytes = null,
        .reservation = null,
        .runtime = self,
    };
    const byte_count = expandedInputLengthLocked(self, bytes, maximum_bytes) catch |err| {
        return err;
    };
    const reservation = try reserveInput(self, byte_count, false, .client);
    errdefer input_commit_ops.cancelInput(self, reservation.identifier);
    const copy = try linefeed.duplicateExpanded(
        self.allocator,
        bytes,
        self.terminal.modes.get(.linefeed),
        byte_count,
    );
    return .{
        .affects_close_activity = affects_close_activity,
        .bytes = copy,
        .reservation = reservation,
        .runtime = self,
    };
}

pub fn prepareOwnedInputLocked(
    self: *state.State,
    bytes: []u8,
    affects_close_activity: bool,
    maximum_bytes: usize,
) !prepared_input.Internal {
    failure_ops.ensureRuntimeWorkAvailableLocked(self) catch |err| {
        self.allocator.free(bytes);
        return err;
    };
    if (self.child_exited.load(.acquire)) {
        self.allocator.free(bytes);
        return error.TerminalExited;
    }
    if (bytes.len == 0) {
        self.allocator.free(bytes);
        return .{
            .affects_close_activity = affects_close_activity,
            .bytes = null,
            .reservation = null,
            .runtime = self,
        };
    }
    const byte_count = expandedInputLengthLocked(self, bytes, maximum_bytes) catch |err| {
        self.allocator.free(bytes);
        return err;
    };
    const owned = if (byte_count == bytes.len)
        bytes
    else
        linefeed.duplicateExpanded(
            self.allocator,
            bytes,
            true,
            byte_count,
        ) catch |err| {
            self.allocator.free(bytes);
            return err;
        };
    if (owned.ptr != bytes.ptr) self.allocator.free(bytes);
    const reservation = reserveInput(self, byte_count, false, .client) catch |err| {
        self.allocator.free(owned);
        return err;
    };
    return .{
        .affects_close_activity = affects_close_activity,
        .bytes = owned,
        .reservation = reservation,
        .runtime = self,
    };
}

pub fn expandedInputLengthLocked(
    self: *const state.State,
    bytes: []const u8,
    maximum_bytes: usize,
) !usize {
    const byte_count = linefeed.expandedLength(
        bytes,
        self.terminal.modes.get(.linefeed),
    ) catch return error.InputLimitExceeded;
    if (byte_count > maximum_bytes) return error.InputLimitExceeded;
    return byte_count;
}

pub fn reserveInput(
    self: *state.State,
    byte_count: usize,
    wait: bool,
    source: contracts.InputSource,
) !contracts.InputReservation {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    if (self.writer_stop) return error.TerminalExited;
    if (source == .client and self.client_input_closed) return error.TerminalExited;
    while (!self.input_queue.hasCapacity(
        byte_count,
        source,
        input_commit_ops.inputRepairCharge(self, source),
    )) {
        if (!wait) return error.InputBackpressure;
        self.input_condition.wait(&self.input_mutex);
        if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
        if (self.writer_stop) return error.TerminalExited;
        if (source == .client and self.client_input_closed) return error.TerminalExited;
    }
    return self.input_queue.reserve(
        byte_count,
        source,
        input_commit_ops.inputRepairCharge(self, source),
    );
}
