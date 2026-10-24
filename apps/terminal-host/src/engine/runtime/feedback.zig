const std = @import("std");
const thread_sync = @import("terminal-host-platform").thread_sync;
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const failure_ops = @import("failure.zig");
const mode_effects = @import("mode_effects.zig");

const max_clipboard_text_bytes = protocol.Limits.clipboard_text_bytes;
const max_application_signal_bytes = protocol.Limits.terminal_application_signal_bytes;
const private_application_signal_prefix = "orchbee.agent.v1;";
pub const max_private_application_signal_apc_bytes = private_application_signal_prefix.len +
    ((max_application_signal_bytes + 2) / 3) * 4;

pub fn privateApplicationSignalEffect(
    handler: *ghostty_vt.TerminalStream.Handler,
    sequence: ghostty_vt.UnknownSequence,
) void {
    const self = runtimeFromTerminalHandler(handler);
    std.debug.assert(self.reader_commit_in_progress);
    const apc = switch (sequence) {
        .apc => |value| value,
    };
    if (!apc.truncated) {
        if (self.command_observation.accept(
            &self.terminal,
            self.generation,
            apc.content,
        ) catch {
            self.feedback_sequence_overflow_pending = true;
            return;
        }) return;
    }
    if (apc.truncated or
        !std.mem.startsWith(u8, apc.content, private_application_signal_prefix)) return;
    const encoded = apc.content[private_application_signal_prefix.len..];
    if (encoded.len == 0) return;
    if (self.application_signal_pending_count + self.application_signal_staged_count >=
        contracts.max_pending_application_signals) return;
    if (self.application_signal_sequence_staged == std.math.maxInt(u64)) {
        self.feedback_sequence_overflow_pending = true;
        return;
    }
    const decoded_length = std.base64.url_safe_no_pad.Decoder.calcSizeForSlice(encoded) catch return;
    if (decoded_length > max_application_signal_bytes) return;
    const payload = self.allocator.alloc(u8, decoded_length) catch return;
    var retained = false;
    defer if (!retained) self.allocator.free(payload);
    std.base64.url_safe_no_pad.Decoder.decode(payload, encoded) catch return;
    if (!std.unicode.utf8ValidateSlice(payload) or
        std.mem.indexOfScalar(u8, payload, 0) != null) return;
    self.application_signal_sequence_staged += 1;
    self.application_signal_staged[self.application_signal_staged_count] = .{
        .payload = payload,
        .sequence = self.application_signal_sequence_staged,
    };
    self.application_signal_staged_count += 1;
    retained = true;
}

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
            self.command_observation.staged.ready = false;
            self.close_prompt_phase_staged = .command_active;
            stageCloseInputLineAcknowledgement(self);
            self.command_started_at_ns_staged = clock.monotonicTimeNanoseconds(self);
        },
        .end_command => {
            self.close_prompt_phase_staged = .command_finished;
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
        .fresh_line_new_prompt,
        .new_command,
        .prompt_start,
        => {
            self.close_prompt_phase_staged = .prompt_start;
            mode_effects.recoverMouseTrackingAtPromptLocked(self);
        },
        .end_prompt_start_input,
        .end_prompt_start_input_terminate_eol,
        => self.close_prompt_phase_staged = .input_ready,
        .fresh_line,
        => {},
    }
}

/// A C marker acknowledges one line only when that line was already accepted
/// at marker parse time. This prevents an empty/canceled read from consuming
/// a command published afterward, even if the reader batch commits later.
fn stageCloseInputLineAcknowledgement(self: *state.State) void {
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.close_input_lines_exhausted) return;
    const pending_lines = self.close_input_lines_submitted - self.close_input_lines_consumed;
    if (pending_lines > self.close_input_line_acknowledgements_staged) {
        self.close_input_line_acknowledgements_staged += 1;
    }
}

pub fn beginReaderBatch(self: *state.State) void {
    std.debug.assert(self.application_signal_staged_count == 0);
    std.debug.assert(self.clipboard_write_staged == null);
    std.debug.assert(self.progress_staged == null);
    std.debug.assert(!self.command_batch_active);
    self.command_batch_active = true;
    self.command_observation.begin();
    self.close_prompt_phase_staged = self.close_prompt_phase;
    self.close_input_line_acknowledgements_staged = 0;
    self.command_completion_staged = self.command_completion;
    self.command_sequence_staged = self.command_sequence;
    self.command_started_at_ns_staged = self.command_started_at_ns;
    self.application_signal_sequence_staged = self.application_signal_sequence;
}

pub fn abortReaderBatch(self: *state.State) void {
    for (self.application_signal_staged[0..self.application_signal_staged_count]) |signal| {
        self.allocator.free(signal.payload);
    }
    self.application_signal_staged_count = 0;
    self.application_signal_sequence_staged = 0;
    if (self.clipboard_write_staged) |text| self.allocator.free(text);
    self.clipboard_write_staged = null;
    self.progress_staged = null;
    self.command_batch_active = false;
    self.command_completion_staged = null;
    self.command_sequence_staged = 0;
    self.command_started_at_ns_staged = null;
    self.close_prompt_phase_staged = self.close_prompt_phase;
    self.close_input_line_acknowledgements_staged = 0;
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

    if (self.application_signal_staged_count > 0) {
        for (self.application_signal_staged[0..self.application_signal_staged_count]) |signal| {
            std.debug.assert(
                self.application_signal_pending_count < contracts.max_pending_application_signals,
            );
            const tail = (self.application_signal_pending_head +
                self.application_signal_pending_count) % contracts.max_pending_application_signals;
            self.application_signal_pending[tail] = signal;
            self.application_signal_pending_count += 1;
        }
        self.application_signal_staged_count = 0;
        self.application_signal_sequence = self.application_signal_sequence_staged;
    }

    if (self.close_input_line_acknowledgements_staged != 0) {
        self.input_mutex.lock();
        if (!self.close_input_lines_exhausted) {
            self.close_input_lines_consumed += self.close_input_line_acknowledgements_staged;
        }
        self.input_mutex.unlock();
    }
    self.close_prompt_phase = self.close_prompt_phase_staged;
    self.close_prompt_phase_staged = .unknown;
    self.close_input_line_acknowledgements_staged = 0;
    self.command_started_at_ns = self.command_started_at_ns_staged;
    self.command_observation.commit();
    self.command_completion = self.command_completion_staged;
    self.command_sequence = self.command_sequence_staged;
    self.command_batch_active = false;
    self.command_completion_staged = null;
    self.command_sequence_staged = 0;
    self.command_started_at_ns_staged = null;
}

pub fn prepareApplicationSignal(
    self: *state.State,
    allocator: std.mem.Allocator,
) !?contracts.ApplicationSignalEvent {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    failure_ops.waitForReaderCommitLocked(self);
    try failure_ops.ensureReadableStateAvailableLocked(self);
    if (self.application_signal_pending_count == 0) return null;
    const pending = self.application_signal_pending[self.application_signal_pending_head];
    return .{
        .allocator = allocator,
        .payload = try allocator.dupe(u8, pending.payload),
        .sequence = pending.sequence,
    };
}

pub fn commitApplicationSignal(
    self: *state.State,
    event: contracts.ApplicationSignalEvent,
) void {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    if (self.application_signal_pending_count == 0) return;
    const pending = self.application_signal_pending[self.application_signal_pending_head];
    if (pending.sequence != event.sequence) return;
    self.allocator.free(pending.payload);
    self.application_signal_pending_head =
        (self.application_signal_pending_head + 1) % contracts.max_pending_application_signals;
    self.application_signal_pending_count -= 1;
    if (self.application_signal_pending_count == 0) self.application_signal_pending_head = 0;
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
    self.command_observation.deinit(&self.terminal);
    abortReaderBatch(self);
    for (0..self.application_signal_pending_count) |offset| {
        const index = (self.application_signal_pending_head + offset) %
            contracts.max_pending_application_signals;
        self.allocator.free(self.application_signal_pending[index].payload);
    }
    self.application_signal_pending_count = 0;
    self.application_signal_pending_head = 0;
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

fn runtimeFromTerminalHandler(
    handler: *ghostty_vt.TerminalStream.Handler,
) *state.State {
    const host_handler: *terminal_stream.Handler = @fieldParentPtr("terminal_handler", handler);
    return runtimeFromHandler(host_handler);
}
