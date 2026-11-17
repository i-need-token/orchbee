//! The `terminal`, `configuration`, and `console` sections: the Host's facts about one terminal
//! beside its engine. The exporting Host's owner supplies the facts the engine does not hold.
const std = @import("std");
const model = @import("terminal-host-model");
const LaunchKind = @import("terminal-host-platform").process.LaunchKind;
const wire = @import("wire.zig");

const lifecycle = model.lifecycle;
const Geometry = model.geometry.Geometry;
const Error = wire.Error;
const Allocation = std.mem.Allocator.Error;

pub const Id = [16]u8;
pub const Digest = [32]u8;

pub const TerminalIdentity = struct {
    creation_id: Id,
    host_epoch: Id,
    terminal_id: Id,
};

pub const Exited = struct {
    final_revision: u64,
    result: lifecycle.ExitResult,
    termination: lifecycle.TerminationRequest,
};

pub const Failed = struct {
    final_revision: u64,
    failure: lifecycle.RuntimeFailure,
    process: lifecycle.ProcessObservation,
};

pub const Lifecycle = union(enum) {
    running: lifecycle.TerminationRequest,
    exited: Exited,
    failed: Failed,
};

/// What the terminal section carries that the engine does not hold.
pub const TerminalInputs = struct {
    /// Absent until the Keeper holds the terminal and a quiesce marks it.
    holding_id: ?Id = null,
    mark_id: ?Id = null,
    identity: TerminalIdentity,
    lifecycle: Lifecycle,
    start_geometry: ?Geometry,
    launch_fingerprint: Digest,
    environment_overlay_digest: Digest,
};

pub const TerminalFacts = struct {
    inputs: TerminalInputs,
    revision: u64,
    generation: u64,
    geometry: Geometry,
    viewport_geometry: Geometry,
    launch_kind: LaunchKind,
    interactive_shell: bool,
};

pub const ConfigurationFacts = struct {
    configuration: model.configuration.Configuration,
    revision: u64,
    theme: model.theme.State,
};

pub const ProcessIdentity = struct {
    pid: u32,
    /// The process's creation time as the platform reports it.
    created: u64,
};

pub const ConsoleFacts = struct {
    conpty_version: []const u8,
    open_console_sha256: Digest,
    signal_format: u16,
    program: ProcessIdentity,
    console: ProcessIdentity,
};

const max_conpty_version_bytes = 64;

const TerminationCode = wire.Code(lifecycle.TerminationRequest, &.{ .none, .graceful, .force });
const FailureCode = wire.Code(lifecycle.RuntimeFailure, &.{
    .admit_terminal_reply,
    .apply_resize,
    .dispatch_termination,
    .observe_process,
    .read_output,
    .synchronize_worker,
    .write_input,
});
const LaunchCode = wire.Code(LaunchKind, &.{ .managed_powershell, .cmd, .managed_zsh, .managed_bash, .other });
const SchemeCode = wire.Code(model.theme.ColorScheme, &.{ .dark, .light });

/// Each encoder refuses what its decoder refuses, so an export never writes a record that an
/// import would refuse.
pub fn encodeTerminal(out: wire.Writer, facts: TerminalFacts) (Error || Allocation)!void {
    const inputs = facts.inputs;
    for ([_]?Id{ inputs.holding_id, inputs.mark_id }) |id| {
        if (id) |bytes| if (optionalId(bytes) == null) return error.InvalidRecord;
    }
    try out.bytes(&(inputs.holding_id orelse zero_id));
    try out.bytes(&(inputs.mark_id orelse zero_id));
    try out.bytes(&inputs.identity.creation_id);
    try out.bytes(&inputs.identity.host_epoch);
    try out.bytes(&inputs.identity.terminal_id);
    switch (inputs.lifecycle) {
        .running => |termination| {
            try out.int(u8, 1);
            try out.int(u8, TerminationCode.encode(termination));
        },
        .exited => |exited| {
            try out.int(u8, 2);
            try out.int(u64, exited.final_revision);
            try out.int(u8, TerminationCode.encode(exited.termination));
            try encodeExitResult(out, exited.result);
        },
        .failed => |failed| {
            try out.int(u8, 3);
            try out.int(u64, failed.final_revision);
            try out.int(u8, FailureCode.encode(failed.failure));
            switch (failed.process) {
                .observed => |result| {
                    try out.int(u8, 1);
                    try encodeExitResult(out, result);
                },
                .unavailable => try out.int(u8, 2),
            }
        },
    }
    try out.int(u64, facts.revision);
    try out.int(u64, facts.generation);
    try encodeGeometry(out, facts.geometry);
    try encodeGeometry(out, facts.viewport_geometry);
    try out.flag(inputs.start_geometry != null);
    if (inputs.start_geometry) |geometry| try encodeGeometry(out, geometry);
    try out.int(u8, LaunchCode.encode(facts.launch_kind));
    try out.flag(facts.interactive_shell);
    try out.bytes(&inputs.launch_fingerprint);
    try out.bytes(&inputs.environment_overlay_digest);
}

pub fn decodeTerminal(payload: []const u8) Error!TerminalFacts {
    var in: wire.Reader = .init(payload);
    const holding_id = optionalId(try in.array(16));
    const mark_id = optionalId(try in.array(16));
    const identity: TerminalIdentity = .{
        .creation_id = try in.array(16),
        .host_epoch = try in.array(16),
        .terminal_id = try in.array(16),
    };
    const state: Lifecycle = switch (try in.int(u8)) {
        1 => .{ .running = try TerminationCode.decode(try in.int(u8)) },
        2 => .{ .exited = .{
            .final_revision = try in.int(u64),
            .termination = try TerminationCode.decode(try in.int(u8)),
            .result = try decodeExitResult(&in),
        } },
        3 => .{ .failed = .{
            .final_revision = try in.int(u64),
            .failure = try FailureCode.decode(try in.int(u8)),
            .process = switch (try in.int(u8)) {
                1 => .{ .observed = try decodeExitResult(&in) },
                2 => .{ .unavailable = .observe_process },
                else => return error.InvalidRecord,
            },
        } },
        else => return error.InvalidRecord,
    };
    const revision = try in.int(u64);
    const generation = try in.int(u64);
    const geometry = try decodeGeometry(&in);
    const viewport_geometry = try decodeGeometry(&in);
    const start_geometry = if (try in.flag()) try decodeGeometry(&in) else null;
    const launch_kind = try LaunchCode.decode(try in.int(u8));
    const interactive_shell = try in.flag();
    const launch_fingerprint = try in.array(32);
    const environment_overlay_digest = try in.array(32);
    try in.finish();
    return .{
        .inputs = .{
            .holding_id = holding_id,
            .mark_id = mark_id,
            .identity = identity,
            .lifecycle = state,
            .start_geometry = start_geometry,
            .launch_fingerprint = launch_fingerprint,
            .environment_overlay_digest = environment_overlay_digest,
        },
        .revision = revision,
        .generation = generation,
        .geometry = geometry,
        .viewport_geometry = viewport_geometry,
        .launch_kind = launch_kind,
        .interactive_shell = interactive_shell,
    };
}

pub fn encodeConfiguration(out: wire.Writer, facts: ConfigurationFacts) (Error || Allocation)!void {
    if (!model.configuration.valid(facts.configuration) or
        !model.theme.validState(facts.theme)) return error.InvalidRecord;
    try out.int(u64, facts.configuration.scrollback_bytes);
    try out.int(u64, facts.revision);
    try out.flag(facts.theme.operation_id != null);
    if (facts.theme.operation_id) |id| try out.bytes(&id.bytes);
    try out.int(u64, facts.theme.revision);
    const theme = facts.theme.terminal;
    try out.int(u8, SchemeCode.encode(theme.color_scheme));
    try out.int(u32, theme.default_background_rgb);
    try out.int(u32, theme.default_foreground_rgb);
    try out.optionalInt(u32, theme.default_cursor_rgb);
    for (theme.palette) |color| try out.int(u32, color);
}

pub fn decodeConfiguration(payload: []const u8) Error!ConfigurationFacts {
    var in: wire.Reader = .init(payload);
    const scrollback_bytes = std.math.cast(usize, try in.int(u64)) orelse return error.InvalidRecord;
    const revision = try in.int(u64);
    const operation_id: ?model.identifiers.TerminalThemeOperationId =
        if (try in.flag()) .fromBytes(try in.array(16)) else null;
    var result: ConfigurationFacts = .{
        .configuration = .{ .scrollback_bytes = scrollback_bytes },
        .revision = revision,
        .theme = .{
            .operation_id = operation_id,
            .revision = try in.int(u64),
            .terminal = .{
                .color_scheme = try SchemeCode.decode(try in.int(u8)),
                .default_background_rgb = try in.int(u32),
                .default_foreground_rgb = try in.int(u32),
                .default_cursor_rgb = try in.optionalInt(u32),
                .palette = undefined,
            },
        },
    };
    for (&result.theme.terminal.palette) |*color| color.* = try in.int(u32);
    try in.finish();
    if (!model.configuration.valid(result.configuration) or
        !model.theme.validState(result.theme)) return error.InvalidRecord;
    return result;
}

pub fn encodeConsole(out: wire.Writer, facts: ConsoleFacts) (Error || Allocation)!void {
    if (!validVersion(facts.conpty_version) or facts.signal_format == 0 or
        facts.program.pid == 0 or facts.console.pid == 0) return error.InvalidRecord;
    try out.counted(u8, facts.conpty_version);
    try out.bytes(&facts.open_console_sha256);
    try out.int(u16, facts.signal_format);
    for ([_]ProcessIdentity{ facts.program, facts.console }) |process| {
        try out.int(u32, process.pid);
        try out.int(u64, process.created);
    }
}

/// The result borrows `payload`.
pub fn decodeConsole(payload: []const u8) Error!ConsoleFacts {
    var in: wire.Reader = .init(payload);
    const version = try in.counted(u8, max_conpty_version_bytes);
    if (!validVersion(version)) return error.InvalidRecord;
    const sha256 = try in.array(32);
    const signal_format = try in.int(u16);
    var processes: [2]ProcessIdentity = undefined;
    for (&processes) |*process| process.* = .{ .pid = try in.int(u32), .created = try in.int(u64) };
    try in.finish();
    if (signal_format == 0 or processes[0].pid == 0 or processes[1].pid == 0) return error.InvalidRecord;
    return .{
        .conpty_version = version,
        .open_console_sha256 = sha256,
        .signal_format = signal_format,
        .program = processes[0],
        .console = processes[1],
    };
}

const zero_id: Id = [_]u8{0} ** 16;

fn optionalId(bytes: Id) ?Id {
    return if (std.mem.allEqual(u8, &bytes, 0)) null else bytes;
}

fn validVersion(text: []const u8) bool {
    if (text.len == 0 or text.len > max_conpty_version_bytes) return false;
    for (text) |byte| if (byte < 0x21 or byte > 0x7e) return false;
    return true;
}

fn encodeExitResult(out: wire.Writer, result: lifecycle.ExitResult) Allocation!void {
    switch (result) {
        .exit_code => |value| {
            try out.int(u8, 1);
            try out.int(u32, value);
        },
        .signal => |value| {
            try out.int(u8, 2);
            try out.int(u32, value);
        },
        .unknown => |value| {
            try out.int(u8, 3);
            try out.int(u32, value);
        },
    }
}

fn decodeExitResult(in: *wire.Reader) Error!lifecycle.ExitResult {
    const tag = try in.int(u8);
    const value = try in.int(u32);
    return switch (tag) {
        1 => .{ .exit_code = value },
        2 => .{ .signal = value },
        3 => .{ .unknown = value },
        else => error.InvalidRecord,
    };
}

fn encodeGeometry(out: wire.Writer, geometry: Geometry) (Error || Allocation)!void {
    if (!model.geometry.isValid(geometry)) return error.InvalidRecord;
    try out.int(u16, geometry.columns);
    try out.int(u16, geometry.rows);
    try out.int(u16, geometry.cell_pixel_width);
    try out.int(u16, geometry.cell_pixel_height);
}

fn decodeGeometry(in: *wire.Reader) Error!Geometry {
    const geometry: Geometry = .{
        .columns = try in.int(u16),
        .rows = try in.int(u16),
        .cell_pixel_width = try in.int(u16),
        .cell_pixel_height = try in.int(u16),
    };
    if (!model.geometry.isValid(geometry)) return error.InvalidRecord;
    return geometry;
}
