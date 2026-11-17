const target_builtin = @import("builtin");
const pty_process = @import("terminal-host-platform").process;
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

pub fn pauseOutputRead(self: *state.State) !void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.pause_read) |pause_read| {
            return pause_read(self.testing_hooks.context.?);
        }
    }
    try pty_process.pauseRead(&self.process);
}

pub fn resumeOutputRead(self: *state.State) !void {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.resume_read) |resume_read| {
            return resume_read(self.testing_hooks.context.?);
        }
    }
    try pty_process.resumeRead(&self.process);
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
    self.forced_end_pending = false;
    if (self.containment_confirmed.load(.acquire)) return;
    self.process.forceTerminate() catch |err| {
        if (err != error.KeeperReleasePending) return err;
        // The whole-Job fallback waits for a Keeper's release. The end stands, and the owner
        // retries it, so its dispatch thread never waits for a Keeper.
        self.forced_end_pending = true;
    };
}

/// Retries a forced end that waited for a Keeper's release; nothing when none waits.
pub fn continueForcedEnd(self: *state.State) !void {
    if (self.forced_end_pending) try forceTerminate(self);
}

pub fn hasActiveCommand(self: *state.State) !bool {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.has_active_command) |observe| {
            return observe(self.testing_hooks.context.?);
        }
    }
    return self.process.hasActiveCommand();
}

pub fn readPty(self: *state.State, buffer: []u8, mode: pty_process.ReadMode) !pty_process.Read {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.read_pty_pausable) |read| {
            return read(self.testing_hooks.context.?, buffer, mode);
        }
        if (self.testing_hooks.read_pty) |read| {
            const count = try read(self.testing_hooks.context.?, buffer);
            return if (count == 0) .end else .{ .bytes = count };
        }
    }
    return pty_process.readPausable(&self.process, buffer, mode);
}

pub fn writePtySlice(self: *state.State, bytes: []const u8) !usize {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.write_pty) |write| {
            return write(self.testing_hooks.context.?, bytes);
        }
    }
    return self.process.write(bytes);
}
