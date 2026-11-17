//! Restores a terminal from a handover record into a new runtime. The record is decoded and
//! checked in full before any worker or process starts, and a refused record creates nothing.
//! Until the Host adopts a held console, the restored runtime starts the stand-in launch its
//! caller passes, at the record's geometry.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const handover = @import("../handover/root.zig");
const png_decoder = @import("../png_decoder.zig");
const terminal_runtime_launch = @import("../runtime_launch.zig");
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const clock = @import("clock.zig");
const creation = @import("creation.zig");
const feedback = @import("feedback.zig");
const terminal_control = @import("terminal_control.zig");
const handover_export = @import("handover_export.zig");
const handover_shell = @import("handover_shell.zig");
const handover_input = @import("handover_input.zig");

const EngineState = handover.EngineState;

pub const Facts = struct {
    /// Borrows the record.
    product_version: []const u8,
    terminal: handover.TerminalFacts,
    configuration: handover.ConfigurationFacts,
    /// Borrows the record.
    console: ?handover.ConsoleFacts,
    /// Allocator-owned: the attachments that hold what the program was last told of focus, keys,
    /// and buttons. The owner detaches each once it claims the terminal.
    held_attachments: []contracts.AttachmentId,
};

pub const Restored = struct {
    runtime: *state.State,
    facts: Facts,
};

pub fn restore(
    allocator: std.mem.Allocator,
    record: []const u8,
    launch: contracts.ProcessLaunch,
    spawner: anytype,
) !Restored {
    png_decoder.install();
    var decoded = try handover.decode(allocator, record, handover_export.pin);
    defer decoded.deinit();
    const geometry = decoded.terminal.geometry;
    const engine = decoded.engine;
    const profile = try profileFrom(engine.profile);
    try checkEngine(decoded.terminal, engine);
    var stand_in = launch;
    stand_in.columns = geometry.columns;
    stand_in.rows = geometry.rows;
    stand_in.cell_pixel_width = geometry.cell_pixel_width;
    stand_in.cell_pixel_height = geometry.cell_pixel_height;
    const checked = try terminal_runtime_launch.prepare(allocator, stand_in, .{
        .cell_pixel_height = geometry.cell_pixel_height,
        .cell_pixel_width = geometry.cell_pixel_width,
        .profile = profile,
        .scrollback_bytes = decoded.configuration.configuration.scrollback_bytes,
    });

    var reader: std.Io.Reader = .fixed(decoded.screen);
    var snapshot = ghostty_vt.snapshot.decodeExact(allocator, platform.thread_sync.io(), &reader, .{
        .max_continuation_bytes = 0,
    }) catch |err| return switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        error.ContinuationLimitExceeded => error.ContinuationPending,
        else => error.InvalidRecord,
    };
    defer snapshot.deinit(allocator);
    const decoded_terminal = &snapshot.terminal.?;
    if (decoded_terminal.cols != geometry.columns or decoded_terminal.rows != geometry.rows) {
        return error.InvalidRecord;
    }

    const held_attachments = try allocator.alloc(contracts.AttachmentId, engine.held_input.len);
    errdefer allocator.free(held_attachments);
    const self = try allocator.create(state.State);
    errdefer allocator.destroy(self);
    const resources = resources: {
        var owned_profile = try contracts.OwnedProfile.init(allocator, profile);
        errdefer owned_profile.deinit(allocator);
        const kitty_temp_directory = try platform.temp_directory.resolveOwned(allocator);
        break :resources .{ .profile = owned_profile, .kitty_temp_directory = kitty_temp_directory };
    };
    self.* = .{
        .allocator = allocator,
        .cell_pixel_height = geometry.cell_pixel_height,
        .cell_pixel_width = geometry.cell_pixel_width,
        .command_timer = platform.monotonic.Timer.start() catch null,
        .generation = decoded.terminal.generation,
        .input_queue = .init(allocator),
        .input_ownership = .init(allocator),
        .kitty_temp_directory = resources.kitty_temp_directory,
        .viewport_geometry = decoded.terminal.viewport_geometry,
        .process = undefined,
        .profile = resources.profile,
        .projection = .init(allocator),
        .history = .init(allocator),
        .revision = .init(decoded.terminal.revision),
        .stream = undefined,
        .stream_allocator = .init(allocator, terminal_stream.osc_allocation_limit_bytes),
        .scrollback_bytes = decoded.configuration.configuration.scrollback_bytes,
        .terminal = snapshot.toOwned(),
    };
    creation.adoptTestingHooks(self, spawner);
    errdefer creation.cleanupFailedThreadStartup(self);
    // The stream resets the platform's prompt-redraw default; the record's value stands.
    const shell_redraws_prompt = self.terminal.flags.shell_redraws_prompt;
    creation.wireStream(self);
    self.terminal.flags.shell_redraws_prompt = shell_redraws_prompt;
    applyHostLimits(self);
    try install(self, engine, held_attachments);
    try creation.startWorkers(self, checked.process_launch, spawner);
    return .{
        .runtime = self,
        .facts = .{
            .product_version = decoded.product_version,
            .terminal = decoded.terminal,
            .configuration = decoded.configuration,
            .console = decoded.console,
            .held_attachments = held_attachments,
        },
    };
}

/// The bounds the Host sets on every terminal and Ghostty's snapshot does not carry.
fn applyHostLimits(self: *state.State) void {
    self.terminal.setKittyGraphicsSizeLimit(self.allocator, protocol.Limits.terminal_graphics_resource_bytes);
    self.terminal.setKittyGraphicsLoadingLimits(creation.kittyLoadingLimits(self.kitty_temp_directory.?));
    _ = terminal_control.applyScrollbackBytes(&self.terminal, self.scrollback_bytes);
    self.stream.handler.resized();
}

fn install(self: *state.State, engine: EngineState, held_attachments: []contracts.AttachmentId) !void {
    // Deadlines and ages travel as durations; the clock is moved back so the oldest one still
    // reads as a time after the clock's start.
    if (self.command_timer) |*timer| {
        const anchor_age = if (engine.prompt_gate.anchor) |anchor| anchor.marked_age_ns orelse 0 else 0;
        const offset = @max(engine.command_elapsed_ns orelse 0, anchor_age);
        timer.started_at = timer.started_at.subDuration(.fromNanoseconds(offset));
    }
    const now = clock.monotonicTimeNanoseconds(self);
    self.bell_sequence = engine.bell_sequence;
    self.bell_sequence_overflow_pending = engine.bell_overflow_pending;
    self.feedback_sequence_overflow_pending = engine.feedback_overflow_pending;
    self.metadata_revision.store(engine.metadata_revision, .release);
    self.progress = engine.progress;
    self.program_status = engine.program_status;
    self.command_completion = engine.command_completion;
    self.command_sequence = engine.command_sequence;
    self.command_started_at_ns = if (engine.command_elapsed_ns) |elapsed|
        if (now) |value| value -| elapsed else null
    else
        null;
    self.clipboard_write_sequence = engine.clipboard_sequence;
    if (engine.clipboard_pending) |pending| self.clipboard_write_pending = .{
        .sequence = pending.sequence,
        .text = try self.allocator.dupe(u8, pending.text),
    };
    self.application_signal_sequence = engine.application_signal_sequence;
    for (engine.application_signals) |signal| {
        self.application_signal_pending[self.application_signal_pending_count] = .{
            .payload = try self.allocator.dupe(u8, signal.payload),
            .sequence = signal.sequence,
        };
        self.application_signal_pending_count += 1;
    }
    self.application_signal_pending_available.store(self.application_signal_pending_count > 0, .release);
    self.mouse_input_epoch = engine.mouse_input_epoch;
    self.mouse_input_epoch_exhausted = engine.mouse_input_epoch_exhausted;
    self.typed_input_sequence = engine.typed_input_sequence;
    try handover_shell.importPromptGate(self, engine.prompt_gate, now);
    try handover_shell.importCommandObservation(self, engine.command_observation);
    try handover_input.importHeld(&self.input_ownership, engine.held_input, held_attachments);
    self.synchronized_output_deadline_ns = deadline(engine.synchronized_output_remaining_ns, now);
    self.cursor_paint_deadline_ns = deadline(engine.cursor_paint_remaining_ns, now);
}

fn deadline(remaining_ns: ?u64, now_ns: ?u64) ?u64 {
    const remaining = remaining_ns orelse return null;
    const now = now_ns orelse return null;
    return std.math.add(u64, now, remaining) catch null;
}

/// Facts the section codecs cannot judge alone, checked before anything is built.
fn checkEngine(terminal: handover.TerminalFacts, engine: EngineState) error{InvalidRecord}!void {
    if (terminal.generation == 0 or engine.metadata_revision > terminal.revision) return error.InvalidRecord;
    if (engine.clipboard_pending) |pending| if (!feedback.validClipboardText(pending.text)) {
        return error.InvalidRecord;
    };
    for (engine.application_signals) |signal| {
        if (!std.unicode.utf8ValidateSlice(signal.payload) or
            std.mem.indexOfScalar(u8, signal.payload, 0) != null) return error.InvalidRecord;
    }
}

/// The engine section's profile as a launch profile; the colors are the snapshot's, so the
/// profile's own stay at a valid placeholder that nothing applies.
fn profileFrom(profile: handover.engine_section.Profile) error{InvalidRecord}!model.profile.Profile {
    const result: model.profile.Profile = .{
        .color_scheme = profile.color_scheme,
        .default_background_rgb = 0,
        .default_cursor_blinking = profile.default_cursor_blinking,
        .default_cursor_rgb = null,
        .default_cursor_shape = profile.default_cursor_shape,
        .default_foreground_rgb = 0,
        .device_primary_conformance_level = profile.device_primary_conformance_level,
        .device_primary_features = profile.device_primary_features,
        .device_secondary_type = profile.device_secondary_type,
        .device_secondary_firmware_version = profile.device_secondary_firmware_version,
        .device_secondary_rom_cartridge = profile.device_secondary_rom_cartridge,
        .device_tertiary_unit_id = profile.device_tertiary_unit_id,
        .enquiry_response = profile.enquiry_response,
        .palette = &placeholder_palette,
        .version_response = profile.version_response,
    };
    if (!model.profile.valid(result)) return error.InvalidRecord;
    return result;
}

const placeholder_palette = [_]u32{0} ** protocol.Limits.terminal_profile_palette_entries;
