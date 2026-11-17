//! The `engine` section: the per-terminal state the engine keeps beside Ghostty's. Dynamic and
//! default colors are not repeated here; Ghostty's TERMINAL record in the `screen` section
//! carries both, so the two sections cannot disagree.
const std = @import("std");
const model = @import("terminal-host-model");
const protocol = @import("terminal-protocol");
const wire = @import("wire.zig");
pub const held_input = @import("held_input.zig");
pub const shell_state = @import("shell_state.zig");

const Error = wire.Error;
const Allocation = std.mem.Allocator.Error;
const Limits = protocol.Limits;

pub const Profile = struct {
    color_scheme: model.profile.ColorScheme,
    default_cursor_blinking: bool,
    default_cursor_shape: model.profile.CursorShape,
    device_primary_conformance_level: u16,
    device_primary_features: []const u16,
    device_secondary_type: u16,
    device_secondary_firmware_version: u16,
    device_secondary_rom_cartridge: u16,
    device_tertiary_unit_id: u32,
    enquiry_response: []const u8,
    version_response: []const u8,
};

pub const Clipboard = struct {
    sequence: u64,
    text: []const u8,
};

pub const ApplicationSignal = struct {
    sequence: u64,
    payload: []const u8,
};

pub const ConsoleSize = struct {
    columns: u16,
    rows: u16,
};

/// ConPTY signals queued and not yet written. A quiesce exports only once the signal sender is
/// idle, so an H1 Host writes none, and refuses a record that names one.
pub const ConptyCoordination = struct {
    size: ?ConsoleSize = null,
    clear: bool = false,
    size_after_clear: ?ConsoleSize = null,

    pub fn empty(self: ConptyCoordination) bool {
        return self.size == null and !self.clear and self.size_after_clear == null;
    }
};

pub const EngineState = struct {
    profile: Profile,
    bell_sequence: u64,
    bell_overflow_pending: bool,
    feedback_overflow_pending: bool,
    /// The content revision at which the title or directory last changed; zero when never.
    metadata_revision: u64,
    progress: protocol.TerminalProgress,
    program_status: ?protocol.TerminalProgramStatus,
    command_completion: ?protocol.TerminalCommandCompletion,
    command_sequence: u64,
    /// How long before the export the running command started.
    command_elapsed_ns: ?u64,
    clipboard_sequence: u64,
    clipboard_pending: ?Clipboard,
    application_signal_sequence: u64,
    /// Oldest first.
    application_signals: []const ApplicationSignal,
    mouse_input_epoch: u64,
    mouse_input_epoch_exhausted: bool,
    typed_input_sequence: u64,
    held_input: []const held_input.Held,
    prompt_gate: shell_state.PromptGate,
    command_observation: shell_state.CommandObservation,
    /// The Host's scanners' bytes of an unfinished sequence; an H1 Host writes none.
    scanner_partial_state: []const u8,
    synchronized_output_remaining_ns: ?u64,
    cursor_paint_remaining_ns: ?u64,
    conpty: ConptyCoordination,
};

const max_application_signals = 64;
const max_scanner_partial_bytes = 64 * 1024;

const SchemeCode = wire.Code(model.profile.ColorScheme, &.{ .dark, .light });
const ShapeCode = wire.Code(model.profile.CursorShape, &.{ .block, .underline, .bar });
const ProgressCode = wire.Code(protocol.TerminalProgressState, &.{ .remove, .set, .@"error", .indeterminate, .pause });
const StatusCode = wire.Code(protocol.TerminalProgramStatusState, &.{ .idle, .working, .blocked, .done, .@"error" });

pub fn encode(out: wire.Writer, state: EngineState) (Error || Allocation)!void {
    const profile = state.profile;
    try out.int(u8, SchemeCode.encode(profile.color_scheme));
    try out.flag(profile.default_cursor_blinking);
    try out.int(u8, ShapeCode.encode(profile.default_cursor_shape));
    try out.int(u16, profile.device_primary_conformance_level);
    try out.int(u8, std.math.cast(u8, profile.device_primary_features.len) orelse return error.InvalidRecord);
    for (profile.device_primary_features) |feature| try out.int(u16, feature);
    try out.int(u16, profile.device_secondary_type);
    try out.int(u16, profile.device_secondary_firmware_version);
    try out.int(u16, profile.device_secondary_rom_cartridge);
    try out.int(u32, profile.device_tertiary_unit_id);
    try out.counted(u16, profile.enquiry_response);
    try out.counted(u16, profile.version_response);

    try out.int(u64, state.bell_sequence);
    try out.flag(state.bell_overflow_pending);
    try out.flag(state.feedback_overflow_pending);
    try out.int(u64, state.metadata_revision);
    try out.int(u8, ProgressCode.encode(state.progress.state));
    try out.optionalInt(u8, state.progress.value);
    try out.flag(state.program_status != null);
    if (state.program_status) |status| {
        try out.int(u8, StatusCode.encode(status.state));
        try out.int(u64, status.reported_at_nanoseconds);
    }
    try out.flag(state.command_completion != null);
    if (state.command_completion) |completion| {
        try out.int(u64, completion.sequence);
        try out.int(u64, completion.duration_milliseconds);
        try out.int(u64, completion.completed_at_milliseconds);
        try out.int(u8, completion.exit_code);
    }
    try out.int(u64, state.command_sequence);
    try out.optionalInt(u64, state.command_elapsed_ns);

    try out.int(u64, state.clipboard_sequence);
    try out.flag(state.clipboard_pending != null);
    if (state.clipboard_pending) |pending| {
        try out.int(u64, pending.sequence);
        try out.counted(u32, pending.text);
    }
    try out.int(u64, state.application_signal_sequence);
    try out.int(u8, std.math.cast(u8, state.application_signals.len) orelse return error.InvalidRecord);
    for (state.application_signals) |signal| {
        try out.int(u64, signal.sequence);
        try out.counted(u32, signal.payload);
    }

    try out.int(u64, state.mouse_input_epoch);
    try out.flag(state.mouse_input_epoch_exhausted);
    try out.int(u64, state.typed_input_sequence);
    try held_input.encode(out, state.held_input);
    try shell_state.encodePromptGate(out, state.prompt_gate);
    try shell_state.encodeCommandObservation(out, state.command_observation);
    try out.counted(u32, state.scanner_partial_state);
    try out.optionalInt(u64, state.synchronized_output_remaining_ns);
    try out.optionalInt(u64, state.cursor_paint_remaining_ns);
    try encodeSize(out, state.conpty.size);
    try out.flag(state.conpty.clear);
    try encodeSize(out, state.conpty.size_after_clear);
}

/// Every slice of the result lives in `arena`.
pub fn decode(arena: std.mem.Allocator, payload: []const u8) (Error || Allocation)!EngineState {
    var in: wire.Reader = .init(payload);
    const color_scheme = try SchemeCode.decode(try in.int(u8));
    const default_cursor_blinking = try in.flag();
    const default_cursor_shape = try ShapeCode.decode(try in.int(u8));
    const conformance = try in.int(u16);
    const features = try arena.alloc(u16, try in.count(u8, Limits.terminal_profile_features, 2));
    for (features) |*feature| feature.* = try in.int(u16);
    const profile: Profile = .{
        .color_scheme = color_scheme,
        .default_cursor_blinking = default_cursor_blinking,
        .default_cursor_shape = default_cursor_shape,
        .device_primary_conformance_level = conformance,
        .device_primary_features = features,
        .device_secondary_type = try in.int(u16),
        .device_secondary_firmware_version = try in.int(u16),
        .device_secondary_rom_cartridge = try in.int(u16),
        .device_tertiary_unit_id = try in.int(u32),
        .enquiry_response = try arena.dupe(u8, try in.counted(u16, Limits.terminal_response_bytes)),
        .version_response = try arena.dupe(u8, try in.counted(u16, Limits.terminal_response_bytes)),
    };

    const bell_sequence = try in.int(u64);
    const bell_overflow_pending = try in.flag();
    const feedback_overflow_pending = try in.flag();
    const metadata_revision = try in.int(u64);
    const progress: protocol.TerminalProgress = .{
        .state = try ProgressCode.decode(try in.int(u8)),
        .value = try in.optionalInt(u8),
    };
    if (progress.value) |value| if (value > 100 or progress.state == .remove or
        progress.state == .indeterminate) return error.InvalidRecord;
    const program_status: ?protocol.TerminalProgramStatus = if (try in.flag()) .{
        .state = try StatusCode.decode(try in.int(u8)),
        .reported_at_nanoseconds = try in.int(u64),
    } else null;
    if (program_status) |status| if (status.reported_at_nanoseconds == 0) return error.InvalidRecord;
    const command_completion: ?protocol.TerminalCommandCompletion = if (try in.flag()) .{
        .sequence = try in.int(u64),
        .duration_milliseconds = try in.int(u64),
        .completed_at_milliseconds = try in.int(u64),
        .exit_code = try in.int(u8),
    } else null;
    const command_sequence = try in.int(u64);
    const command_elapsed_ns = try in.optionalInt(u64);

    const clipboard_sequence = try in.int(u64);
    const clipboard_pending: ?Clipboard = if (try in.flag()) .{
        .sequence = try in.int(u64),
        .text = try arena.dupe(u8, try in.counted(u32, Limits.clipboard_text_bytes)),
    } else null;
    if (clipboard_pending) |pending| if (pending.sequence == 0 or pending.sequence > clipboard_sequence) {
        return error.InvalidRecord;
    };
    const application_signal_sequence = try in.int(u64);
    const signals = try arena.alloc(ApplicationSignal, try in.count(u8, max_application_signals, 12));
    var previous_sequence: u64 = 0;
    for (signals) |*signal| {
        signal.* = .{
            .sequence = try in.int(u64),
            .payload = try arena.dupe(u8, try in.counted(u32, Limits.terminal_application_signal_bytes)),
        };
        if (signal.sequence <= previous_sequence or signal.sequence > application_signal_sequence or
            signal.payload.len == 0) return error.InvalidRecord;
        previous_sequence = signal.sequence;
    }

    const result: EngineState = .{
        .profile = profile,
        .bell_sequence = bell_sequence,
        .bell_overflow_pending = bell_overflow_pending,
        .feedback_overflow_pending = feedback_overflow_pending,
        .metadata_revision = metadata_revision,
        .progress = progress,
        .program_status = program_status,
        .command_completion = command_completion,
        .command_sequence = command_sequence,
        .command_elapsed_ns = command_elapsed_ns,
        .clipboard_sequence = clipboard_sequence,
        .clipboard_pending = clipboard_pending,
        .application_signal_sequence = application_signal_sequence,
        .application_signals = signals,
        .mouse_input_epoch = try in.int(u64),
        .mouse_input_epoch_exhausted = try in.flag(),
        .typed_input_sequence = try in.int(u64),
        .held_input = try held_input.decode(arena, &in),
        .prompt_gate = try shell_state.decodePromptGate(&in),
        .command_observation = try shell_state.decodeCommandObservation(&in),
        .scanner_partial_state = try arena.dupe(u8, try in.counted(u32, max_scanner_partial_bytes)),
        .synchronized_output_remaining_ns = try in.optionalInt(u64),
        .cursor_paint_remaining_ns = try in.optionalInt(u64),
        .conpty = .{
            .size = try decodeSize(&in),
            .clear = try in.flag(),
            .size_after_clear = try decodeSize(&in),
        },
    };
    try in.finish();
    if (result.mouse_input_epoch == 0) return error.InvalidRecord;
    return result;
}

fn encodeSize(out: wire.Writer, size: ?ConsoleSize) Allocation!void {
    try out.flag(size != null);
    const value = size orelse return;
    try out.int(u16, value.columns);
    try out.int(u16, value.rows);
}

fn decodeSize(in: *wire.Reader) Error!?ConsoleSize {
    if (!try in.flag()) return null;
    return .{ .columns = try in.int(u16), .rows = try in.int(u16) };
}
