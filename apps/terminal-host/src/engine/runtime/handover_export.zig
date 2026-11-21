//! Exports one terminal's handover record. The export only reads the runtime, under the terminal
//! lock and then the input lock, so every refusal or failure leaves the terminal exactly as it was
//! and a resumed reader reads on.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const handover_options = @import("handover-options");
const handover = @import("../handover/root.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const reader_pause = @import("reader_pause.zig");
const handover_shell = @import("handover_shell.zig");
const handover_input = @import("handover_input.zig");

/// The exporting Host's Ghostty pin: the package hash its build fetched.
pub const pin = handover_options.ghostty_package_hash;

pub const Inputs = struct {
    terminal: handover.TerminalInputs,
    configuration_revision: u64,
    theme: model.theme.State,
    console: ?handover.ConsoleFacts = null,
};

pub const Error = error{
    /// The terminal failed, or a resize fenced it.
    Fenced,
    /// A resize's platform effect is in flight.
    ResizeInFlight,
    /// Ghostty's parser or one of the Host's scanners is inside a sequence.
    MidSequence,
    /// The reader has not parked.
    NotParked,
    /// Ghostty's snapshot carries no Kitty images; until the image section exists such a
    /// terminal stays on its Host.
    HoldsImage,
    /// Accepted input waits to be written, or an attachment's input change is in flight.
    InputPending,
    /// The terminal holds state Ghostty's snapshot cannot carry.
    Unrepresentable,
} || handover.EncodeError;

pub fn exportRecord(self: *state.State, allocator: std.mem.Allocator, inputs: Inputs) Error![]u8 {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    // The checks run here, under the lock the reader parks behind, so nothing moves between them
    // and the encode.
    if (self.failure_fenced or self.resize_fenced.load(.acquire)) return error.Fenced;
    if (self.resize_effect_in_flight) return error.ResizeInFlight;
    if (!reader_pause.atGroundLocked(self)) return error.MidSequence;
    if (self.reader_pause != .parked) return error.NotParked;
    if (holdsImage(self)) return error.HoldsImage;
    self.input_mutex.lock();
    defer self.input_mutex.unlock();
    if (self.input_queue.entries.items.len != 0) return error.InputPending;
    return encodeLocked(self, allocator, inputs, .running);
}

/// Encodes a settled terminal's record. Its program ended and its output was read to the end, so
/// no byte follows: what the program was last told of keys and buttons is not carried, and a
/// sequence the output ended inside can never complete, so it is not carried either.
pub fn exportSettledRecord(self: *state.State, allocator: std.mem.Allocator, inputs: Inputs) Error![]u8 {
    self.terminal_mutex.lock();
    defer self.terminal_mutex.unlock();
    if (holdsImage(self)) return error.HoldsImage;
    return encodeLocked(self, allocator, inputs, .settled);
}

const Kind = enum { running, settled };

fn encodeLocked(self: *state.State, allocator: std.mem.Allocator, inputs: Inputs, kind: Kind) Error![]u8 {
    var arena: std.heap.ArenaAllocator = .init(allocator);
    defer arena.deinit();
    const engine = try engineState(self, arena.allocator(), kind);
    var screen: std.Io.Writer.Allocating = .init(allocator);
    defer screen.deinit();
    ghostty_vt.snapshot.encode(allocator, &screen.writer, &self.terminal, .{ .continuation = .ground }) catch |err|
        return switch (err) {
            error.OutOfMemory, error.WriteFailed => error.OutOfMemory,
            else => error.Unrepresentable,
        };
    return handover.encode(allocator, .{
        .product_version = handover_options.host_version,
        .pin = pin,
        .terminal = .{
            .inputs = inputs.terminal,
            .revision = self.revision.load(.acquire),
            .generation = self.generation,
            .geometry = .{
                .columns = self.terminal.cols,
                .rows = self.terminal.rows,
                .cell_pixel_width = self.cell_pixel_width,
                .cell_pixel_height = self.cell_pixel_height,
            },
            .viewport_geometry = self.viewport_geometry,
            .launch_kind = self.process.launch_kind,
            .interactive_shell = self.process.interactive_shell,
        },
        .configuration = .{
            .configuration = .{ .scrollback_bytes = self.scrollback_bytes },
            .revision = inputs.configuration_revision,
            .theme = inputs.theme,
        },
        .screen = screen.written(),
        .engine = engine,
        .console = inputs.console,
    });
}

fn holdsImage(self: *const state.State) bool {
    if (self.graphics_media.pending != null or self.graphics_media.stalled != null) return true;
    for ([_]ghostty_vt.ScreenSet.Key{ .primary, .alternate }) |key| {
        const screen = self.terminal.screens.get(key) orelse continue;
        const images = &screen.kitty_images;
        if (images.images.count() != 0 or images.placements.count() != 0 or images.loading != null) return true;
    }
    return false;
}

fn engineState(self: *state.State, arena: std.mem.Allocator, kind: Kind) error{ InputPending, OutOfMemory }!handover.EngineState {
    const now = clock.monotonicTimeNanoseconds(self);
    const profile = &self.profile;
    const features = try arena.alloc(u16, profile.device_primary_features.len);
    for (features, profile.device_primary_features) |*target, feature| target.* = @intFromEnum(feature);
    const signals = try arena.alloc(handover.engine_section.ApplicationSignal, self.application_signal_pending_count);
    for (signals, 0..) |*signal, offset| {
        const entry = self.application_signal_pending[
            (self.application_signal_pending_head + offset) % contracts.max_pending_application_signals
        ];
        signal.* = .{ .sequence = entry.sequence, .payload = entry.payload };
    }
    return .{
        .profile = .{
            .color_scheme = profile.color_scheme,
            .default_cursor_blinking = profile.default_cursor_blinking,
            .default_cursor_shape = profile.default_cursor_shape,
            .device_primary_conformance_level = profile.device_primary_conformance_level,
            .device_primary_features = features,
            .device_secondary_type = profile.device_secondary_type,
            .device_secondary_firmware_version = profile.device_secondary_firmware_version,
            .device_secondary_rom_cartridge = profile.device_secondary_rom_cartridge,
            .device_tertiary_unit_id = profile.device_tertiary_unit_id,
            .enquiry_response = profile.enquiry_response,
            .version_response = profile.version_response,
        },
        .bell_sequence = self.bell_sequence,
        .bell_overflow_pending = self.bell_sequence_overflow_pending,
        .feedback_overflow_pending = self.feedback_sequence_overflow_pending,
        .metadata_revision = self.metadata_revision.load(.monotonic),
        .progress = self.progress,
        .program_status = self.program_status,
        .command_completion = self.command_completion,
        .command_sequence = self.command_sequence,
        .command_elapsed_ns = if (self.command_started_at_ns) |started|
            if (now) |value| value -| started else null
        else
            null,
        .clipboard_sequence = self.clipboard_write_sequence,
        .clipboard_pending = if (self.clipboard_write_pending) |pending|
            .{ .sequence = pending.sequence, .text = pending.text }
        else
            null,
        .application_signal_sequence = self.application_signal_sequence,
        .application_signals = signals,
        .mouse_input_epoch = self.mouse_input_epoch,
        .mouse_input_epoch_exhausted = self.mouse_input_epoch_exhausted,
        .typed_input_sequence = self.typed_input_sequence,
        .held_input = switch (kind) {
            .running => try handover_input.exportHeld(&self.input_ownership, arena),
            .settled => &.{},
        },
        .prompt_gate = handover_shell.exportPromptGate(self, now),
        .command_observation = handover_shell.exportCommandObservation(self),
        .scanner_partial_state = "",
        .synchronized_output_remaining_ns = remaining(self.synchronized_output_deadline_ns, now),
        .cursor_paint_remaining_ns = remaining(self.cursor_paint_deadline_ns, now),
        .conpty = .{},
    };
}

fn remaining(deadline_ns: ?u64, now_ns: ?u64) ?u64 {
    const deadline = deadline_ns orelse return null;
    const now = now_ns orelse return null;
    return deadline -| now;
}
