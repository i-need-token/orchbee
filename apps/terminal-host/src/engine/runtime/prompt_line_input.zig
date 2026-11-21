//! Prompt-gated input: one shell command line is typed only while the Host can
//! confirm that its root shell waits at a fresh, empty prompt. Otherwise no
//! queue slot is reserved and no byte is written.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_input = @import("terminal-input");
const terminal_input_encoding = @import("../input_encoding.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const failure_ops = @import("failure.zig");
const input_admission_ops = @import("input_admission.zig");
const key_tap = @import("key_tap.zig");
const prepared_input = @import("prepared_input.zig");
const process_io_ops = @import("process_io.zig");
const prompt_gate = @import("prompt_gate.zig");

/// Records one OSC 133 mark in stream order. The reader holds terminal_mutex.
pub fn observeLocked(
    self: *state.State,
    command: ghostty_vt.StreamAction.SemanticPrompt,
) void {
    const gate = &self.prompt_gate;
    switch (command.action) {
        .fresh_line_new_prompt,
        .new_command,
        .prompt_start,
        => gate.promptStart(&self.terminal, switch (command.readOption(.prompt_kind) orelse .initial) {
            .initial => .primary,
            .right => .right,
            .continuation, .secondary => .other,
        }),
        .end_prompt_start_input,
        .end_prompt_start_input_terminate_eol,
        => gate.inputStart(&self.terminal, clock.monotonicTimeNanoseconds(self), inputFacts(self)),
        .end_input_start_output, .end_command => gate.commandBoundary(&self.terminal),
        .fresh_line => {},
    }
}

/// Checks the launch, process, screen, mark, input and anchor rules, then the
/// quiet period, and reserves the encoded line. It never waits: the Host
/// applies requests one at a time, so a young prompt is refused as settling and
/// the caller retries. Publication repeats every rule except the process sample.
pub fn prepare(
    self: *state.State,
    input: terminal_input.PromptLineInput,
    maximum_bytes: usize,
) !prepared_input.Internal {
    if (input.content.len == 0) return error.EmptyInput;
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    try failure_ops.ensureRuntimeWorkAvailableLocked(self);
    if (self.child_exited.load(.acquire)) return error.TerminalExited;
    if (!self.process.interactive_shell) return error.PromptNotShell;
    if (process_io_ops.hasActiveCommand(self) catch true) return error.PromptBusy;
    const ready = try self.prompt_gate.check(&self.terminal, inputFacts(self), 0);
    const marked_at = ready.marked_at_nanoseconds orelse return error.PromptChanged;
    const now = clock.monotonicTimeNanoseconds(self) orelse return error.PromptChanged;
    if (now -| marked_at < prompt_gate.quiet_nanoseconds) return error.PromptSettling;
    const encoded = try encodeLocked(self, input, maximum_bytes);
    var prepared = try input_admission_ops.prepareOwnedInputLocked(self, encoded, maximum_bytes);
    prepared.prompt_gate_serial = ready.serial;
    return prepared;
}

/// The paste follows the terminal's bracketed-paste mode; each key is a tap.
fn encodeLocked(
    self: *state.State,
    input: terminal_input.PromptLineInput,
    maximum_bytes: usize,
) ![]u8 {
    const paste = try terminal_input_encoding.encodePaste(
        self.allocator,
        &self.terminal,
        input.content,
        maximum_bytes,
    );
    var line: std.ArrayList(u8) = .fromOwnedSlice(paste);
    errdefer line.deinit(self.allocator);
    if (input.right_arrow) _ = try key_tap.append(self, &line, "ArrowRight");
    if (try key_tap.append(self, &line, "Enter") == 0) return error.InvalidKey;
    if (line.items.len > maximum_bytes) return error.InputLimitExceeded;
    return line.toOwnedSlice(self.allocator);
}

fn inputFacts(self: *state.State) prompt_gate.InputFacts {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    return .{
        .pending_entries = self.input_queue.charge(.client).entries,
        .sequence = self.typed_input_sequence,
    };
}
