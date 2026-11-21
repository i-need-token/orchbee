//! Screen-gated key input: one key from a closed list is typed only while the
//! screen is still the frame its sender decided on, the process it answers
//! still runs in this terminal, and no person has acted since the sender first
//! saw what it answers. Otherwise no queue slot is reserved and no byte is
//! written. A hold types nothing and makes every key decided before it fail.
const std = @import("std");
const target_builtin = @import("builtin");
const pty_process = @import("terminal-host-platform").process;
const terminal_input = @import("terminal-input");
const state = @import("state.zig");
const failure_ops = @import("failure.zig");
const input_admission_ops = @import("input_admission.zig");
const input_commit_ops = @import("input_commit.zig");
const key_tap = @import("key_tap.zig");
const prepared_input = @import("prepared_input.zig");
const screen_gate = @import("screen_gate.zig");

/// Checks every gate rule and reserves the encoded key. It never waits and the
/// sender never resends: a refusal means the sender reads the screen again.
/// Publication repeats every rule except the process sample, under the same
/// two locks as the write.
pub fn prepare(
    self: *state.State,
    input: terminal_input.ScreenGatedKeyInput,
    maximum_bytes: usize,
) !prepared_input.Internal {
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    const request = switch (input) {
        .hold => return prepareHold(self),
        .key => |request| request,
    };
    // A process sample is too slow for terminal_mutex, so it comes first. An
    // unreadable sample refuses like a missing process.
    if (!(hasProcess(self, request.process) catch false)) return error.ScreenAgentGone;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    const gate: screen_gate.Gate = .{
        .generation = request.generation,
        .revision = request.revision,
        .first_revision = request.first_revision,
    };
    {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        try input_commit_ops.checkScreenGateLocked(self, gate, 0);
    }
    var line: std.ArrayList(u8) = .empty;
    errdefer line.deinit(self.allocator);
    if (try key_tap.append(self, &line, keyName(request.key)) == 0) return error.InvalidKey;
    if (line.items.len > maximum_bytes) return error.InputLimitExceeded;
    const encoded = try line.toOwnedSlice(self.allocator);
    var prepared = try input_admission_ops.prepareOwnedInputLocked(self, encoded, maximum_bytes);
    prepared.screen_gate = gate;
    return prepared;
}

/// A hold is a client input of no bytes: its commit records a person's action
/// on the current screen like any other, and every earlier key names a first
/// revision at or before that screen.
fn prepareHold(self: *state.State) !prepared_input.Internal {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    return input_admission_ops.prepareEncodedInputLocked(self, "", 0);
}

fn hasProcess(self: *state.State, identity: terminal_input.ProcessIdentity) !bool {
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.has_process) |observe| {
            return observe(self.testing_hooks.context.?, identity);
        }
    }
    var sample = try pty_process.sampleProcesses(&self.process, self.allocator);
    defer sample.deinit();
    for (sample.processes) |process| {
        if (process.pid == identity.pid and process.creation_time == identity.creation_time) return true;
    }
    return false;
}

fn keyName(key: terminal_input.ScreenGatedKey) []const u8 {
    return switch (key) {
        .enter => "Enter",
    };
}
