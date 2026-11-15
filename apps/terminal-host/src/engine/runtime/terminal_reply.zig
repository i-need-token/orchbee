const std = @import("std");
const diagnostic_log = @import("terminal-host-platform").diagnostic_log;
const linefeed = @import("linefeed.zig");
const state = @import("state.zig");

/// Queues one terminal-generated reply while terminal_mutex is held. The
/// canonical linefeed mode is observed before exact reply capacity is charged.
pub fn enqueueLocked(self: *state.State, bytes: []const u8) void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    enqueueInputLocked(self, bytes);
}

/// Queues one reply while terminal_mutex and input_mutex are both held. This
/// form lets mode effects observe their external authority and publish the
/// corresponding reply at one terminal -> input commit point.
///
/// A reply never fails the terminal. Like xterm, the Host drops a reply that does
/// not fit the bounded reply queue or cannot be copied and counts it; a failed or
/// exiting terminal has no program left to answer, so its replies are discarded.
pub fn enqueueInputLocked(self: *state.State, bytes: []const u8) void {
    if (bytes.len == 0 or self.failure_fenced or self.writer_stop) return;
    publish(self, bytes) catch noteDropped(self);
}

/// Counts one reply that was produced but could not be queued.
pub fn noteDropped(self: *state.State) void {
    _ = self.dropped_terminal_replies.fetchAdd(1, .monotonic);
    diagnostic_log.noteDroppedTerminalReply();
}

fn publish(self: *state.State, bytes: []const u8) !void {
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
