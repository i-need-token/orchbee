const std = @import("std");
const thread_sync = @import("terminal-host-platform").thread_sync;
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const failure_ops = @import("failure.zig");

const max_clipboard_text_bytes = protocol.Limits.clipboard_text_bytes;

pub fn clipboardContentsEffect(
    handler: *terminal_stream.Handler,
    contents: ghostty_vt.StreamAction.ClipboardContents,
) void {
    const self = runtimeFromHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    if (contents.kind != 'c' or
        std.mem.eql(u8, contents.data, "?") or
        self.clipboard_write_pending != null or
        self.clipboard_write_staged != null) return;

    const decoded_length = std.base64.standard.Decoder.calcSizeForSlice(
        contents.data,
    ) catch return;
    if (decoded_length > max_clipboard_text_bytes) return;
    const text = self.allocator.alloc(u8, decoded_length) catch return;
    var retained = false;
    defer if (!retained) self.allocator.free(text);
    std.base64.standard.Decoder.decode(text, contents.data) catch return;
    if (!validClipboardText(text)) return;
    self.clipboard_write_staged = text;
    retained = true;
}

pub fn progressReportEffect(
    handler: *terminal_stream.Handler,
    report: ghostty_vt.osc.Command.ProgressReport,
) void {
    const self = runtimeFromHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    const progress: contracts.TerminalProgress = .{
        .state = switch (report.state) {
            .remove => .remove,
            .set => .set,
            .@"error" => .@"error",
            .indeterminate => .indeterminate,
            .pause => .pause,
        },
        .value = if (report.progress) |value| @min(value, 100) else null,
    };
    if ((progress.state == .remove or progress.state == .indeterminate) and
        progress.value != null) return;
    self.progress_staged = progress;
}

pub fn semanticPromptEffect(
    handler: *terminal_stream.Handler,
    command: ghostty_vt.StreamAction.SemanticPrompt,
) void {
    const self = runtimeFromHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    std.debug.assert(self.command_batch_active);
    switch (command.action) {
        .end_input_start_output => {
            self.command_started_at_ns_staged = clock.monotonicTimeNanoseconds(self);
        },
        .end_command => {
            const started_at_ns = self.command_started_at_ns_staged orelse return;
            self.command_started_at_ns_staged = null;
            const completed_at_ns = clock.monotonicTimeNanoseconds(self) orelse return;
            const completed_at_ms = wallTimeMilliseconds(self) orelse return;
            if (completed_at_ns < started_at_ns) return;
            if (self.command_sequence_staged == std.math.maxInt(u64)) {
                self.feedback_sequence_overflow_pending = true;
                return;
            }
            self.command_sequence_staged += 1;
            const raw_exit_code: i32 = command.readOption(.exit_code) orelse 0;
            self.command_completion_staged = .{
                .completed_at_milliseconds = completed_at_ms,
                .duration_milliseconds = (completed_at_ns - started_at_ns) / std.time.ns_per_ms,
                .exit_code = std.math.cast(u8, raw_exit_code) orelse 1,
                .sequence = self.command_sequence_staged,
            };
        },
        .end_prompt_start_input,
        .end_prompt_start_input_terminate_eol,
        .fresh_line,
        .fresh_line_new_prompt,
        .new_command,
        .prompt_start,
        => {},
    }
}

pub fn beginReaderBatch(self: *state.State) void {
    std.debug.assert(self.clipboard_write_staged == null);
    std.debug.assert(self.progress_staged == null);
    std.debug.assert(!self.command_batch_active);
    self.command_batch_active = true;
    self.command_completion_staged = self.command_completion;
    self.command_sequence_staged = self.command_sequence;
    self.command_started_at_ns_staged = self.command_started_at_ns;
}

pub fn abortReaderBatch(self: *state.State) void {
    if (self.clipboard_write_staged) |text| self.allocator.free(text);
    self.clipboard_write_staged = null;
    self.progress_staged = null;
    self.command_batch_active = false;
    self.command_completion_staged = null;
    self.command_sequence_staged = 0;
    self.command_started_at_ns_staged = null;
}

pub fn commitReaderBatch(self: *state.State) void {
    std.debug.assert(self.command_batch_active);
    if (self.clipboard_write_staged) |text| {
        self.clipboard_write_staged = null;
        if (self.clipboard_write_pending == null) {
            if (self.clipboard_write_sequence == std.math.maxInt(u64)) {
                self.feedback_sequence_overflow_pending = true;
                self.allocator.free(text);
            } else {
                self.clipboard_write_sequence += 1;
                self.clipboard_write_pending = .{
                    .sequence = self.clipboard_write_sequence,
                    .text = text,
                };
            }
        } else {
            self.allocator.free(text);
        }
    }

    if (self.progress_staged) |progress| {
        self.progress_staged = null;
        if (!std.meta.eql(self.progress, progress)) self.progress = progress;
    }

    self.command_started_at_ns = self.command_started_at_ns_staged;
    self.command_completion = self.command_completion_staged;
    self.command_sequence = self.command_sequence_staged;
    self.command_batch_active = false;
    self.command_completion_staged = null;
    self.command_sequence_staged = 0;
    self.command_started_at_ns_staged = null;
}

pub fn prepareClipboardWriteRequest(
    self: *state.State,
) !?contracts.PreparedClipboardWriteRequest {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    try failure_ops.ensureReadableStateAvailableLocked(self);
    const pending = self.clipboard_write_pending orelse return null;
    if (self.clipboard_write_in_flight) return null;
    self.clipboard_write_in_flight = true;
    return .{ .sequence = pending.sequence, .text = pending.text };
}

pub fn rollbackClipboardWriteRequest(
    self: *state.State,
    prepared: contracts.PreparedClipboardWriteRequest,
) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    const pending = self.clipboard_write_pending orelse unreachable;
    std.debug.assert(self.clipboard_write_in_flight);
    std.debug.assert(pending.sequence == prepared.sequence);
    self.clipboard_write_in_flight = false;
}

pub fn commitClipboardWriteRequest(
    self: *state.State,
    prepared: contracts.PreparedClipboardWriteRequest,
) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    const pending = self.clipboard_write_pending orelse unreachable;
    std.debug.assert(self.clipboard_write_in_flight);
    std.debug.assert(pending.sequence == prepared.sequence);
    self.allocator.free(pending.text);
    self.clipboard_write_pending = null;
    self.clipboard_write_in_flight = false;
}

pub fn deinit(self: *state.State) void {
    abortReaderBatch(self);
    if (self.clipboard_write_pending) |pending| self.allocator.free(pending.text);
    self.clipboard_write_pending = null;
    self.clipboard_write_in_flight = false;
}

fn validClipboardText(text: []const u8) bool {
    if (!std.unicode.utf8ValidateSlice(text)) return false;
    for (text) |byte| {
        if (byte == 0x7f or (byte < 0x20 and byte != '\t' and byte != '\n' and byte != '\r')) {
            return false;
        }
    }
    return true;
}

fn wallTimeMilliseconds(self: *state.State) ?u64 {
    if (comptime @import("builtin").is_test) {
        if (self.testing_hooks.wall_time_milliseconds) |read| {
            return read(self.testing_hooks.context.?);
        }
    }
    return std.math.cast(
        u64,
        std.Io.Timestamp.now(thread_sync.io(), .real).toMilliseconds(),
    );
}

fn runtimeFromHandler(handler: *terminal_stream.Handler) *state.State {
    const stream: *terminal_stream.Stream = @fieldParentPtr("handler", handler);
    return @alignCast(@fieldParentPtr("stream", stream));
}
