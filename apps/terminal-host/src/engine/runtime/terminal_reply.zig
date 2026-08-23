const std = @import("std");
const linefeed = @import("linefeed.zig");
const state = @import("state.zig");

/// Queues one terminal-generated reply while terminal_mutex is held. The
/// canonical linefeed mode is observed before exact reply capacity is charged.
pub fn enqueueLocked(self: *state.State, bytes: []const u8) !void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    return enqueueInputLocked(self, bytes);
}

/// Queues one reply while terminal_mutex and input_mutex are both held. This
/// form lets mode effects observe their external authority and publish the
/// corresponding reply at one terminal -> input commit point.
pub fn enqueueInputLocked(self: *state.State, bytes: []const u8) !void {
    std.debug.assert(bytes.len != 0);
    if (self.failure_fenced) return error.TerminalRuntimeUnavailable;
    if (self.writer_stop) return error.TerminalExited;

    const linefeed_enabled = self.terminal.modes.get(.linefeed);
    const byte_count = try linefeed.expandedLength(bytes, linefeed_enabled);
    const reservation = try self.input_queue.reserve(byte_count, .terminal, .{});
    errdefer std.debug.assert(self.input_queue.cancel(reservation.identifier));
    const copy = try linefeed.duplicateExpanded(
        self.allocator,
        bytes,
        linefeed_enabled,
        byte_count,
    );
    errdefer self.allocator.free(copy);
    std.debug.assert(self.input_queue.publish(reservation, copy));
    self.input_condition.broadcast();
}
