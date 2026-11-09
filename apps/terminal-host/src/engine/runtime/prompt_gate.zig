//! Terminal-owned evidence that the root shell waits at a fresh, empty prompt.
//! The terminal mutex owns this state. Reader effects record OSC 133 prompt marks
//! in stream order; prompt-gated input checks the evidence before it reserves a
//! queue slot and again when it publishes.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");

/// A prompt's input start must be at least this old before gated input writes.
pub const quiet_nanoseconds: u64 = 100 * std.time.ns_per_ms;

/// Each refusal names the first rule the terminal fails, in rule order.
pub const Refusal = error{
    PromptNotShell,
    PromptBusy,
    PromptFullScreen,
    PromptUnmarked,
    PromptNotAtPrompt,
    PromptInputPending,
    PromptChanged,
};

/// A, N, or P with no kind or the initial kind is primary; P may also name a
/// right, continuation, or secondary prompt.
pub const PromptKind = enum { primary, right, other };

pub const Phase = enum {
    /// No primary prompt start has arrived since launch.
    unmarked,
    /// A primary prompt started and its input start has not arrived.
    prompting,
    /// The latest primary prompt reached its input start; the anchor is set.
    input,
    /// Output, a command end, or a non-primary prompt followed the latest mark.
    away,
};

/// Client input facts read under input_mutex. Focus reports and the Host's own
/// terminal replies are not client input here.
pub const InputFacts = struct {
    /// Count of published client inputs.
    sequence: u64,
    /// Client queue entries that are reserved or not yet written.
    pending_entries: usize,
};

pub const Ready = struct {
    marked_at_nanoseconds: ?u64,
    serial: u64,
};

const Anchor = struct {
    input_pending: bool,
    input_sequence: u64,
    line_digest: u64,
    marked_at_nanoseconds: ?u64,
    pending_wrap: bool,
    pin: *ghostty_vt.Pin,
    screen_generation: usize,
    screen_key: ghostty_vt.ScreenSet.Key,
};

pub const Tracker = struct {
    anchor: ?Anchor = null,
    phase: Phase = .unmarked,
    /// Advances on every transition, so a publication proves no mark arrived
    /// after its preparation.
    serial: u64 = 0,

    pub fn deinit(self: *Tracker, terminal: *ghostty_vt.Terminal) void {
        self.release(terminal);
    }

    /// A primary prompt starts a candidate. A right prompt belongs to the prompt
    /// it draws in; a continuation or secondary prompt means the shell is not at
    /// a fresh prompt.
    pub fn promptStart(self: *Tracker, terminal: *ghostty_vt.Terminal, kind: PromptKind) void {
        switch (kind) {
            .primary => self.enter(terminal, .prompting),
            .right => if (self.phase != .prompting and self.phase != .unmarked) {
                self.enter(terminal, .away);
            },
            .other => if (self.phase != .unmarked) self.enter(terminal, .away),
        }
    }

    /// B or I. Only the input start of a primary prompt anchors the cursor.
    pub fn inputStart(
        self: *Tracker,
        terminal: *ghostty_vt.Terminal,
        marked_at_nanoseconds: ?u64,
        input: InputFacts,
    ) void {
        if (self.phase != .prompting) {
            if (self.phase != .unmarked) self.enter(terminal, .away);
            return;
        }
        self.enter(terminal, .away);
        const screen = terminal.screens.active;
        const key = terminal.screens.active_key;
        const point = screen.pages.pin(.{ .active = .{
            .x = screen.cursor.x,
            .y = screen.cursor.y,
        } }) orelse return;
        const pin = screen.pages.trackPin(point) catch return;
        self.anchor = .{
            .input_pending = input.pending_entries != 0,
            .input_sequence = input.sequence,
            .line_digest = lineDigest(point),
            .marked_at_nanoseconds = marked_at_nanoseconds,
            .pending_wrap = screen.cursor.pending_wrap,
            .pin = pin,
            .screen_generation = terminal.screens.generation(key),
            .screen_key = key,
        };
        self.phase = .input;
    }

    /// C or D. The shell left its prompt; cmd never sends these.
    pub fn commandBoundary(self: *Tracker, terminal: *ghostty_vt.Terminal) void {
        if (self.phase != .unmarked) self.enter(terminal, .away);
    }

    /// Checks the screen, mark, input and anchor rules. The caller holds
    /// terminal_mutex and has checked the launch and process rules first.
    /// `own_entries` counts the caller's reserved queue entries.
    pub fn check(
        self: *const Tracker,
        terminal: *ghostty_vt.Terminal,
        input: InputFacts,
        own_entries: usize,
    ) Refusal!Ready {
        if (terminal.screens.active_key != .primary) return error.PromptFullScreen;
        const anchor = switch (self.phase) {
            .unmarked => return error.PromptUnmarked,
            .prompting, .away => return error.PromptNotAtPrompt,
            .input => self.anchor.?,
        };
        if (anchor.input_pending or input.sequence != anchor.input_sequence or
            input.pending_entries != own_entries) return error.PromptInputPending;
        if (!anchorHolds(terminal, anchor)) return error.PromptChanged;
        return .{
            .marked_at_nanoseconds = anchor.marked_at_nanoseconds,
            .serial = self.serial,
        };
    }

    fn enter(self: *Tracker, terminal: *ghostty_vt.Terminal, phase: Phase) void {
        self.release(terminal);
        self.phase = phase;
        self.serial +%= 1;
    }

    fn release(self: *Tracker, terminal: *ghostty_vt.Terminal) void {
        const anchor = self.anchor orelse return;
        self.anchor = null;
        if (terminal.screens.generation(anchor.screen_key) != anchor.screen_generation) return;
        const screen = terminal.screens.get(anchor.screen_key) orelse return;
        screen.pages.untrackPin(anchor.pin);
    }
};

/// The cursor is still where the input started, on the same screen, and the
/// text from there to the end of the logical line is unchanged.
fn anchorHolds(terminal: *ghostty_vt.Terminal, anchor: Anchor) bool {
    const key = terminal.screens.active_key;
    if (key != anchor.screen_key or
        terminal.screens.generation(key) != anchor.screen_generation or
        anchor.pin.garbage) return false;
    const screen = terminal.screens.active;
    const point = screen.pages.pointFromPin(.active, anchor.pin.*) orelse return false;
    if (point.active.x != screen.cursor.x or point.active.y != screen.cursor.y or
        screen.cursor.pending_wrap != anchor.pending_wrap) return false;
    return lineDigest(anchor.pin.*) == anchor.line_digest;
}

fn lineDigest(start: ghostty_vt.Pin) u64 {
    var hasher = std.hash.Wyhash.init(0);
    var row_pin = start;
    var subset: ghostty_vt.Pin.CellSubset = .right;
    while (true) {
        for (row_pin.cells(subset)) |*cell| {
            const codepoint: u32 = cell.codepoint();
            hasher.update(std.mem.asBytes(&codepoint));
            hasher.update(&.{@intFromEnum(cell.wide)});
            if (cell.content_tag == .codepoint_grapheme) {
                if (row_pin.grapheme(cell)) |extra| hasher.update(std.mem.sliceAsBytes(extra));
            }
        }
        if (!row_pin.rowAndCell().row.wrap) break;
        row_pin = row_pin.down(1) orelse break;
        subset = .all;
    }
    return hasher.final();
}
