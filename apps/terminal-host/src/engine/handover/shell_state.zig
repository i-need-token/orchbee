//! Prompt-gate and shell-observation state. Both keep pins into the screen, which travel as the
//! screen coordinates they held, or as stale when the pin no longer named its row.
const std = @import("std");
const protocol = @import("terminal-protocol");
const wire = @import("wire.zig");

const Error = wire.Error;
const Allocation = std.mem.Allocator.Error;

pub const ScreenKey = enum { primary, alternate };

/// A cell in a screen's coordinates, the oldest history row being row zero.
pub const Point = struct {
    screen: ScreenKey,
    x: u16,
    y: u32,
};

pub const AnchorPlace = union(enum) {
    holds: Point,
    /// The pin's screen was replaced or its row discarded; the anchor can never hold again.
    stale,
};

pub const Anchor = struct {
    input_pending: bool,
    input_sequence: u64,
    line_digest: u64,
    /// How long before the export the input start arrived.
    marked_age_ns: ?u64,
    pending_wrap: bool,
    place: AnchorPlace,
};

pub const PromptPhase = enum { unmarked, prompting, input, away };

pub const PromptGate = struct {
    phase: PromptPhase,
    serial: u64,
    /// Present exactly in phase `input`.
    anchor: ?Anchor,
};

pub const Origin = union(enum) {
    none,
    /// The tracked row is still the screen's oldest.
    holds: ScreenKey,
    /// The tracked row moved or left; the next refresh advances the output epoch.
    moved,
};

pub const CommandObservation = struct {
    committed: protocol.TerminalCommandObservation,
    origin: Origin,
    exhausted: bool,
};

const PhaseCode = wire.Code(PromptPhase, &.{ .unmarked, .prompting, .input, .away });
const KeyCode = wire.Code(ScreenKey, &.{ .primary, .alternate });

pub fn encodePromptGate(out: wire.Writer, gate: PromptGate) Allocation!void {
    try out.int(u8, PhaseCode.encode(gate.phase));
    try out.int(u64, gate.serial);
    try out.flag(gate.anchor != null);
    const anchor = gate.anchor orelse return;
    try out.flag(anchor.input_pending);
    try out.int(u64, anchor.input_sequence);
    try out.int(u64, anchor.line_digest);
    try out.optionalInt(u64, anchor.marked_age_ns);
    try out.flag(anchor.pending_wrap);
    switch (anchor.place) {
        .holds => |point| {
            try out.int(u8, 1);
            try out.int(u8, KeyCode.encode(point.screen));
            try out.int(u16, point.x);
            try out.int(u32, point.y);
        },
        .stale => try out.int(u8, 2),
    }
}

pub fn decodePromptGate(in: *wire.Reader) Error!PromptGate {
    const phase = try PhaseCode.decode(try in.int(u8));
    const serial = try in.int(u64);
    const anchor: ?Anchor = if (try in.flag()) .{
        .input_pending = try in.flag(),
        .input_sequence = try in.int(u64),
        .line_digest = try in.int(u64),
        .marked_age_ns = try in.optionalInt(u64),
        .pending_wrap = try in.flag(),
        .place = switch (try in.int(u8)) {
            1 => .{ .holds = .{
                .screen = try KeyCode.decode(try in.int(u8)),
                .x = try in.int(u16),
                .y = try in.int(u32),
            } },
            2 => .stale,
            else => return error.InvalidRecord,
        },
    } else null;
    if ((phase == .input) != (anchor != null)) return error.InvalidRecord;
    return .{ .phase = phase, .serial = serial, .anchor = anchor };
}

pub fn encodeCommandObservation(out: wire.Writer, observation: CommandObservation) Allocation!void {
    const committed = observation.committed;
    try out.int(u64, committed.output_epoch);
    try out.flag(committed.ready);
    try out.flag(committed.command != null);
    if (committed.command) |command| {
        try out.bytes(&command.id);
        try out.flag(command.completed);
        try out.int(i32, command.exit_code);
        try out.int(u64, command.start_row);
        try out.int(u16, command.start_column);
        try out.int(u64, command.end_row);
        try out.int(u16, command.end_column);
        try out.int(u64, command.generation);
        try out.int(u64, command.output_epoch);
        try out.int(u64, command.started_at_nanoseconds);
    }
    switch (observation.origin) {
        .none => try out.int(u8, 0),
        .holds => |key| {
            try out.int(u8, 1);
            try out.int(u8, KeyCode.encode(key));
        },
        .moved => try out.int(u8, 2),
    }
    try out.flag(observation.exhausted);
}

pub fn decodeCommandObservation(in: *wire.Reader) Error!CommandObservation {
    var committed: protocol.TerminalCommandObservation = .{
        .output_epoch = try in.int(u64),
        .ready = try in.flag(),
    };
    if (try in.flag()) {
        const id = try in.array(16);
        if (std.mem.allEqual(u8, &id, 0)) return error.InvalidRecord;
        committed.command = .{
            .id = id,
            .completed = try in.flag(),
            .exit_code = try in.int(i32),
            .start_row = try in.int(u64),
            .start_column = try in.int(u16),
            .end_row = try in.int(u64),
            .end_column = try in.int(u16),
            .generation = try in.int(u64),
            .output_epoch = try in.int(u64),
            .started_at_nanoseconds = try in.int(u64),
        };
    }
    const origin: Origin = switch (try in.int(u8)) {
        0 => .none,
        1 => .{ .holds = try KeyCode.decode(try in.int(u8)) },
        2 => .moved,
        else => return error.InvalidRecord,
    };
    if (committed.output_epoch == 0) return error.InvalidRecord;
    return .{ .committed = committed, .origin = origin, .exhausted = try in.flag() };
}
