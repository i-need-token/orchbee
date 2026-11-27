//! Restores a terminal from a handover record into a new runtime, in two steps. `prepare` decodes
//! and checks the record in full and builds the engine, which reads nothing and runs no worker, so
//! a refused record creates nothing. `start` then adopts the held console that the claim checks
//! passed and starts every worker behind the startup gate, which only the claim opens; a settled
//! terminal starts nothing, since its program ended on the old Host.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const platform = @import("terminal-host-platform");
const pty_process = platform.process;
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
const handover_export = @import("handover_export.zig");
const handover_shell = @import("handover_shell.zig");
const handover_input = @import("handover_input.zig");
const handover_images = @import("handover_images.zig");
const runtime_completion = @import("../runtime_completion.zig");

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

/// A restored runtime that runs no worker yet; `start` or `abandon` ends this state.
pub const Prepared = struct {
    runtime: *state.State,
    facts: Facts,

    /// Frees a prepared runtime that never started.
    pub fn abandon(self: *Prepared) void {
        self.release(false);
    }

    /// `adopted` is comptime, since only `start`, on a Host that moves or recovers terminals, adopts
    /// a program.
    fn release(self: *Prepared, comptime adopted: bool) void {
        const runtime = self.runtime;
        const allocator = runtime.allocator;
        creation.cleanupFailedThreadStartup(runtime);
        if (adopted) runtime.process.drop();
        allocator.destroy(runtime);
        allocator.free(self.facts.held_attachments);
        self.* = undefined;
    }
};

/// How a prepared terminal reaches its program.
pub const Program = union(enum) {
    /// A running terminal's held console, whose duplicates the claim checks passed, or on macOS and
    /// Linux a recovered PTY master and its leader.
    held: pty_process.Held,
    /// A settled terminal, whose program ended on the old Host.
    settled,
};

pub fn prepare(allocator: std.mem.Allocator, record: []const u8) !Prepared {
    png_decoder.install();
    var decoded = try handover.decode(allocator, record, handover_export.pin);
    defer decoded.deinit();
    const geometry = decoded.terminal.geometry;
    const engine = decoded.engine;
    const profile = try profileFrom(engine.profile);
    try checkEngine(decoded.terminal, engine);

    var reader: std.Io.Reader = .fixed(decoded.screen);
    var lifted: LiftedLimits = .{};
    var snapshot = decodeScreen(allocator, &reader, &lifted) catch |err| return switch (err) {
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
    errdefer creation.cleanupFailedThreadStartup(self);
    // The stream resets the platform's prompt-redraw default; the record's value stands.
    const shell_redraws_prompt = self.terminal.flags.shell_redraws_prompt;
    creation.wireStream(self);
    self.terminal.flags.shell_redraws_prompt = shell_redraws_prompt;
    applyHostLimits(self);
    try handover_images.restore(&self.terminal, decoded.images);
    try install(self, engine, held_attachments);
    restoreScrollbackLimits(&lifted);
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

/// Starts a prepared terminal. A running one adopts its held program and starts every worker
/// behind the startup gate, held for the move until its claim. `start` owns what the program holds:
/// on failure it closes and nothing remains of the runtime. A settled one starts nothing.
pub fn start(prepared: *Prepared, program: Program, spawner: anytype) !void {
    // Only a Host that moves or recovers terminals imports one.
    if (comptime !pty_process.recovery_supported) unreachable;
    const self = prepared.runtime;
    switch (program) {
        // Only a moved record arrives settled.
        .settled => if (comptime pty_process.moves_supported) settle(self, prepared.facts.terminal) else unreachable,
        .held => |held| {
            var owned = held;
            self.process = pty_process.Process.adopt(self.allocator, held) catch |err| {
                pty_process.closeHeld(&owned);
                prepared.abandon();
                return err;
            };
            self.move = .held;
            creation.adoptTestingHooks(self, spawner);
            noteArrival(self);
            creation.startGatedWorkers(self, spawner) catch |err| {
                prepared.release(true);
                return err;
            };
        },
    }
}

/// The move is a person's input on the arrival screen for the screen gate. The record keeps the
/// generation and revision a sender names a frame by, so every key decided before the move names a
/// first-seen revision at or before this one and is refused however late it arrives, and the quiet
/// time runs from here. The old Host's input record does not travel: each person's input there came
/// before its quiesce, at or before this revision. Read on the runtime's final clock, before any
/// worker runs.
fn noteArrival(self: *state.State) void {
    self.screen_gate_input.notePerson(self.revision.load(.monotonic), clock.monotonicTimeNanoseconds(self));
}

/// A settled terminal's runtime: no process and no worker, with its outcome already reached.
fn settle(self: *state.State, terminal: handover.TerminalFacts) void {
    self.process = pty_process.Process.settled(terminal.launch_kind, terminal.interactive_shell);
    self.child_exited.store(true, .release);
    self.pty_drained.store(true, .release);
    self.writer_stopped.store(true, .release);
    self.writer_stop = true;
    self.client_input_closed = true;
    self.containment_confirmed.store(true, .release);
    self.threads_joined = true;
    self.startup.abort();
    switch (terminal.inputs.lifecycle) {
        .running => unreachable,
        .exited => |exited| {
            self.exit_result = exitResult(exited.result);
            _ = self.completion_classification.markNormal();
        },
        // The failures that only the Host's own dispatch records, the reply admission and the
        // termination, have no engine kind; the imported Core record carries every one.
        .failed => |failed| if (engineFailure(failed.failure)) |kind| {
            self.completion_classification.markFailure(kind);
        },
    }
}

fn engineFailure(failure: model.lifecycle.RuntimeFailure) ?runtime_completion.FailureKind {
    return switch (failure) {
        .apply_resize => .resize,
        .observe_process => .process_observation,
        .read_output => .pty_read,
        .write_input => .pty_write,
        .synchronize_worker => .worker_integrity,
        .admit_terminal_reply, .dispatch_termination => null,
    };
}

fn exitResult(result: model.lifecycle.ExitResult) pty_process.ExitResult {
    return switch (result) {
        .exit_code => |code| .{ .exit_code = code },
        .signal => |signal| .{ .signal = signal },
        .unknown => |unknown| .{ .unknown = unknown },
    };
}

/// For tests: restores `record` with the stand-in `launch` running at the record's geometry in
/// place of a held console, and opens the startup gate at once.
pub fn restoreStandIn(
    allocator: std.mem.Allocator,
    record: []const u8,
    launch: contracts.ProcessLaunch,
    spawner: anytype,
) !Prepared {
    var prepared = try prepare(allocator, record);
    const geometry = prepared.facts.terminal.geometry;
    var stand_in = launch;
    stand_in.columns = geometry.columns;
    stand_in.rows = geometry.rows;
    stand_in.cell_pixel_width = geometry.cell_pixel_width;
    stand_in.cell_pixel_height = geometry.cell_pixel_height;
    const checked = terminal_runtime_launch.prepare(allocator, stand_in, .{
        .cell_pixel_height = geometry.cell_pixel_height,
        .cell_pixel_width = geometry.cell_pixel_width,
        .profile = null,
        .scrollback_bytes = prepared.runtime.scrollback_bytes,
    }) catch |err| {
        prepared.abandon();
        return err;
    };
    creation.adoptTestingHooks(prepared.runtime, spawner);
    noteArrival(prepared.runtime);
    creation.startWorkers(prepared.runtime, checked.process_launch, spawner) catch |err| {
        prepared.abandon();
        return err;
    };
    return prepared;
}

/// A screen's scrollback byte limit as the record declares it, lifted while its history decodes.
const LiftedLimit = struct { screen: *ghostty_vt.Screen, limit: ?usize };
const LiftedLimits = std.EnumMap(ghostty_vt.ScreenSet.Key, LiftedLimit);

/// Restores the record's screen section with every history page the exporting screen held, since
/// the record's prompt anchor and image placements count rows from the top of that history.
/// Ghostty drops the oldest pages that would exceed a screen's byte limit, and a screen can hold
/// more than its limit: a page that needs room for more styles, graphemes, or links grows without
/// pruning another. Each limit stays lifted until `restoreScrollbackLimits`.
fn decodeScreen(
    allocator: std.mem.Allocator,
    reader: *std.Io.Reader,
    lifted: *LiftedLimits,
) ghostty_vt.snapshot.DecodeExactError!ghostty_vt.snapshot.Decoded {
    var decoder: ghostty_vt.snapshot.Decoder = .init(reader);
    var result = try decoder.ready(allocator, platform.thread_sync.io(), .{ .max_continuation_bytes = 0 });
    errdefer result.deinit(allocator);
    const terminal = &result.terminal.?;
    var screens = terminal.screens.all.iterator();
    while (screens.next()) |entry| {
        const screen = entry.value.*;
        const explicit = screen.pages.limits.bytes.explicit;
        // Ghostty stores an absent limit as the largest value.
        lifted.put(entry.key, .{ .screen = screen, .limit = if (explicit == std.math.maxInt(usize)) null else explicit });
        screen.pages.limits.set(.bytes, null);
    }
    while (try decoder.next(allocator, terminal)) |_| {}
    // The section ends at its FINISH record, as `decodeExact` requires.
    _ = reader.peekByte() catch |err| switch (err) {
        error.EndOfStream => return result,
        else => |other| return other,
    };
    return error.TrailingData;
}

/// Restores each screen's byte limit once the record's screen points are tracked, without pruning:
/// the screen holds what the old Host's held under the same limit, and its next page prunes the
/// oldest as the old Host's would have.
fn restoreScrollbackLimits(lifted: *LiftedLimits) void {
    var screens = lifted.iterator();
    while (screens.next()) |entry| entry.value.screen.pages.limits.set(.bytes, entry.value.limit);
}

/// The bounds the Host sets on every terminal and Ghostty's snapshot does not carry; the snapshot
/// carries each screen's scrollback limit.
fn applyHostLimits(self: *state.State) void {
    self.terminal.setKittyGraphicsSizeLimit(self.allocator, protocol.Limits.terminal_graphics_resource_bytes);
    self.terminal.setKittyGraphicsLoadingLimits(creation.kittyLoadingLimits(self.kitty_temp_directory.?));
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
    // A failed import's cleanup frees what these fields name, so each value is built before it is
    // assigned: Zig writes a literal in place, and a `try` inside one would leave the field naming
    // text that was never allocated.
    if (engine.clipboard_pending) |pending| {
        const text = try self.allocator.dupe(u8, pending.text);
        self.clipboard_write_pending = .{ .sequence = pending.sequence, .text = text };
    }
    self.application_signal_sequence = engine.application_signal_sequence;
    for (engine.application_signals) |signal| {
        const payload = try self.allocator.dupe(u8, signal.payload);
        self.application_signal_pending[self.application_signal_pending_count] = .{
            .payload = payload,
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
