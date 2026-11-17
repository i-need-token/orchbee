//! The prompt gate's anchor and the command observation's origin keep pins into the screen. A
//! record carries the screen coordinates a live pin names; a pin that no longer names its row
//! travels as stale or moved, and the importer gives it a pin marked discarded, as Ghostty marks a
//! pin whose row it removed, so every later check decides as it would have in the exporting Host.
const ghostty_vt = @import("ghostty-vt");
const shell = @import("../handover/root.zig").engine_section.shell_state;
const state = @import("state.zig");
const prompt_gate = @import("prompt_gate.zig");
const command_observation = @import("command_observation.zig");

const Key = ghostty_vt.ScreenSet.Key;
pub const Error = error{ InvalidRecord, OutOfMemory };

pub fn exportPromptGate(self: *state.State, now_ns: ?u64) shell.PromptGate {
    const tracker = &self.prompt_gate;
    const phase = sameTag(shell.PromptPhase, tracker.phase);
    const anchor = tracker.anchor orelse return .{ .phase = phase, .serial = tracker.serial, .anchor = null };
    return .{
        .phase = phase,
        .serial = tracker.serial,
        .anchor = .{
            .input_pending = anchor.input_pending,
            .input_sequence = anchor.input_sequence,
            .line_digest = anchor.line_digest,
            .marked_age_ns = if (anchor.marked_at_nanoseconds) |marked|
                if (now_ns) |now| now -| marked else null
            else
                null,
            .pending_wrap = anchor.pending_wrap,
            .place = if (livePoint(&self.terminal, anchor.pin, anchor.screen_key, anchor.screen_generation)) |point|
                .{ .holds = point }
            else
                .stale,
        },
    };
}

pub fn exportCommandObservation(self: *state.State) shell.CommandObservation {
    const tracker = &self.command_observation;
    const origin: shell.Origin = if (tracker.origin) |origin|
        if (livePoint(&self.terminal, origin.pin, origin.screen_key, origin.screen_generation)) |point|
            if (point.y == 0) .{ .holds = point.screen } else .moved
        else
            .moved
    else
        .none;
    return .{ .committed = tracker.committed, .origin = origin, .exhausted = tracker.exhausted };
}

/// Installs the decoded gate. The caller holds a fresh tracker, so nothing is released.
pub fn importPromptGate(self: *state.State, gate: shell.PromptGate, now_ns: ?u64) Error!void {
    self.prompt_gate = .{ .phase = sameTag(prompt_gate.Phase, gate.phase), .serial = gate.serial };
    const anchor = gate.anchor orelse return;
    const tracked = switch (anchor.place) {
        .holds => |point| try track(&self.terminal, sameTag(Key, point.screen), point.x, point.y),
        .stale => try trackDiscarded(&self.terminal),
    };
    self.prompt_gate.anchor = .{
        .input_pending = anchor.input_pending,
        .input_sequence = anchor.input_sequence,
        .line_digest = anchor.line_digest,
        .marked_at_nanoseconds = if (anchor.marked_age_ns) |age|
            if (now_ns) |now| now -| age else null
        else
            null,
        .pending_wrap = anchor.pending_wrap,
        .pin = tracked.pin,
        .screen_generation = tracked.generation,
        .screen_key = tracked.key,
    };
}

pub fn importCommandObservation(self: *state.State, observation: shell.CommandObservation) Error!void {
    self.command_observation = .{
        .committed = observation.committed,
        .staged = observation.committed,
        .exhausted = observation.exhausted,
    };
    const tracked = switch (observation.origin) {
        .none => return,
        .holds => |key| try track(&self.terminal, sameTag(Key, key), 0, 0),
        .moved => try trackDiscarded(&self.terminal),
    };
    self.command_observation.origin = .{
        .pin = tracked.pin,
        .screen_key = tracked.key,
        .screen_generation = tracked.generation,
    };
}

/// The screen point a tracked pin names, or null when its screen was replaced or its row removed.
fn livePoint(terminal: *ghostty_vt.Terminal, pin: *ghostty_vt.Pin, key: Key, generation: usize) ?shell.Point {
    if (terminal.screens.generation(key) != generation or pin.garbage) return null;
    const screen = terminal.screens.get(key) orelse return null;
    const point = screen.pages.pointFromPin(.screen, pin.*) orelse return null;
    return .{ .screen = sameTag(shell.ScreenKey, key), .x = point.screen.x, .y = point.screen.y };
}

const Tracked = struct {
    pin: *ghostty_vt.Pin,
    key: Key,
    generation: usize,
};

fn track(terminal: *ghostty_vt.Terminal, key: Key, x: u16, y: u32) Error!Tracked {
    const screen = terminal.screens.get(key) orelse return error.InvalidRecord;
    const pin = screen.pages.pin(.{ .screen = .{ .x = x, .y = y } }) orelse return error.InvalidRecord;
    return .{
        .pin = try screen.pages.trackPin(pin),
        .key = key,
        .generation = terminal.screens.generation(key),
    };
}

fn trackDiscarded(terminal: *ghostty_vt.Terminal) Error!Tracked {
    const tracked = try track(terminal, .primary, 0, 0);
    tracked.pin.garbage = true;
    return tracked;
}

/// The member of `T` named as `value`'s tag; a member `T` lacks fails to compile.
fn sameTag(comptime T: type, value: anytype) T {
    return switch (value) {
        inline else => |tag| @field(T, @tagName(tag)),
    };
}
