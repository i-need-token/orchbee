const target_builtin = @import("builtin");
const state = @import("state.zig");

pub fn abortOutputRead(self: *state.State) !void {
    self.output_abort_requested.store(true, .release);
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.abort_read) |abort_read| {
            return abort_read(self.testing_hooks.context.?);
        }
    }
    try self.process.abortRead();
}

pub fn terminate(self: *state.State) !void {
    // Confirmed containment makes termination an idempotent no-op without
    // consulting the platform session. The POSIX process-identity fence
    // still serializes signal vs. release even when this atomic read is
    // stale, so a reused numeric SID is never signaled after release.
    if (self.containment_confirmed.load(.acquire)) return;
    try self.process.terminate();
}

pub fn containmentConfirmed(self: *const state.State) bool {
    return self.containment_confirmed.load(.acquire);
}

pub fn forceTerminate(self: *state.State) !void {
    if (self.containment_confirmed.load(.acquire)) return;
    try self.process.forceTerminate();
}

pub fn readPty(self: *state.State, buffer: []u8) !usize {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.read_pty) |read| {
            return read(self.testing_hooks.context.?, buffer);
        }
    }
    return self.process.read(buffer);
}

pub fn writePtySlice(self: *state.State, bytes: []const u8) !usize {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.write_pty) |write| {
            return write(self.testing_hooks.context.?, bytes);
        }
    }
    return self.process.write(bytes);
}
