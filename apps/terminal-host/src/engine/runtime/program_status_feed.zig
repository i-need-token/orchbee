//! Reader-batch delivery of OSC 7501 program status beside Ghostty's stream.
//!
//! Output is fed to Ghostty in slices that end at each complete OSC 7501 sequence, so a query
//! reply and a root-record change take effect in stream order relative to Ghostty's own replies
//! and OSC 133 effects. Record changes are staged with the reader batch and committed with it.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const program_status = @import("../program_status.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const terminal_reply = @import("terminal_reply.zig");

pub fn feedLocked(self: *state.State, bytes: []const u8) void {
    std.debug.assert(self.reader_commit_in_progress);
    var rest = bytes;
    while (rest.len != 0) {
        const step = self.program_status_scanner.scan(rest);
        self.stream.nextSlice(rest[0..step.consumed]);
        rest = rest[step.consumed..];
        switch (step.sequence orelse continue) {
            .query => |terminator| terminal_reply.enqueueLocked(self, switch (terminator) {
                .bel => "\x1b]7501;?\x07",
                .st => "\x1b]7501;?\x1b\\",
            }) catch {
                self.reply_failure_pending = true;
            },
            .report => |body| switch (program_status.parse(body) orelse continue) {
                .clear => self.program_status_staged = null,
                .set => |reported| self.program_status_staged = .{
                    .state = reported,
                    // Without real time the report cannot be ordered against process starts.
                    .reported_at_nanoseconds = clock.wallTimeNanoseconds(self) orelse continue,
                },
            },
        }
    }
}

pub fn beginBatch(self: *state.State) void {
    self.program_status_staged = self.program_status;
}

pub fn commitBatch(self: *state.State) void {
    self.program_status = self.program_status_staged;
}

/// RIS removes every record; the Host reset control calls this outside a reader batch.
pub fn resetLocked(self: *state.State) void {
    if (self.reader_commit_in_progress) {
        self.program_status_staged = null;
    } else {
        self.program_status = null;
    }
}

/// A new shell prompt ends a live report: `working`, `blocked`, and, by Orchbee's choice of
/// the specification's option, `idle`. `done` and `error` survive. A primary prompt start is
/// a new prompt too, because Orchbee's PowerShell integration marks its prompts with `P`.
pub fn shellPromptLocked(self: *state.State, command: ghostty_vt.StreamAction.SemanticPrompt) void {
    std.debug.assert(self.reader_commit_in_progress);
    switch (command.action) {
        .fresh_line_new_prompt, .new_command => {},
        .prompt_start => switch (command.readOption(.prompt_kind) orelse .initial) {
            .initial => {},
            .right, .continuation, .secondary => return,
        },
        else => return,
    }
    const current = self.program_status_staged orelse return;
    switch (current.state) {
        .idle, .working, .blocked => self.program_status_staged = null,
        .done, .@"error" => {},
    }
}
