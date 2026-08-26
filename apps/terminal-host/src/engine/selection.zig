const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const terminal_interaction = @import("terminal-host-model").interaction;

pub const Point = terminal_interaction.Point;
pub const GestureGeometry = terminal_interaction.GestureGeometry;
pub const GesturePosition = terminal_interaction.GesturePosition;
pub const GestureEvent = terminal_interaction.GestureEvent;

pub fn Gesture(comptime AttachmentId: type) type {
    return struct {
        attachment_id: AttachmentId,
        event: GestureEvent,
        generation: u64,
        revision: u64,
    };
}

pub const Command = terminal_interaction.Command;

pub fn CommandRequest(comptime AttachmentId: type) type {
    return struct {
        attachment_id: AttachmentId,
        command: Command,
        generation: u64,
        revision: u64,
    };
}

pub const Snapshot = union(enum) {
    active: struct {
        end: Point,
        rectangle: bool,
        start: Point,
    },
    evicted,
    none,
};

pub const Error = error{
    AnchorEvicted,
    InvalidPoint,
    InvalidRequest,
    MissingSelection,
    StaleGeneration,
    StaleRevision,
};

/// Attachment-local owner of Ghostty selection and gesture pins.
///
/// History owns the attachment lookup and export ledger; this value owns all
/// Ghostty resources whose validity is scoped to one selection surface.
pub const State = struct {
    gesture: ghostty_vt.SelectionGesture = .init,
    gesture_active: bool = false,
    selection: ?ghostty_vt.Selection = null,
    event: ?Snapshot = null,
    screen_generation: usize = 0,
    screen_key: ghostty_vt.ScreenSet.Key = .primary,

    pub fn active(self: *const State) bool {
        return self.selection != null;
    }

    pub fn clear(self: *State, terminal: *ghostty_vt.Terminal) void {
        self.resetGesture(terminal);
        self.release(terminal);
        self.event = null;
    }

    /// Returns true when a retained selection became invalid and dependent
    /// export work must be invalidated by the history owner.
    pub fn noteMutation(self: *State, terminal: *ghostty_vt.Terminal) bool {
        const selection = self.selection orelse return false;
        if (!selection.start().garbage and !selection.end().garbage) return false;
        self.release(terminal);
        self.event = .evicted;
        return true;
    }

    pub fn generationChanged(self: *State, terminal: *ghostty_vt.Terminal) void {
        self.resetGesture(terminal);
        if (self.selection == null) return;
        const current = self.snapshot(terminal) catch .evicted;
        switch (current) {
            .active => self.event = current,
            .evicted, .none => {
                self.release(terminal);
                self.event = .evicted;
            },
        }
    }

    pub fn screenChanged(self: *State, terminal: *ghostty_vt.Terminal) void {
        self.resetGesture(terminal);
        const was_active = self.active();
        self.release(terminal);
        if (was_active) self.event = .evicted;
    }

    pub fn takeEvent(self: *State) ?Snapshot {
        const current = self.event;
        self.event = null;
        return current;
    }

    pub fn rollbackEvent(self: *State, event: Snapshot) void {
        if (self.event == null) self.event = event;
    }

    pub fn applyGesture(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: anytype,
    ) (Error || std.mem.Allocator.Error)!Snapshot {
        try validateCurrent(
            request.revision,
            request.generation,
            current_revision,
            current_generation,
        );
        switch (request.event) {
            .start => |start| {
                if (start.click_count == 0) return error.InvalidRequest;
                const point_pin = try pointPin(terminal.screens.active, start.point);
                const behavior: ghostty_vt.SelectionGesture.Behavior = switch (start.click_count) {
                    1 => .cell,
                    2 => .word,
                    3 => if (start.select_output) .output else .line,
                    else => unreachable,
                };
                const behaviors: [3]ghostty_vt.SelectionGesture.Behavior = .{
                    behavior,
                    behavior,
                    behavior,
                };
                var gesture: ghostty_vt.SelectionGesture = .init;
                errdefer gesture.deinit(terminal);
                const initial = try gesture.press(terminal, .{
                    .behaviors = &behaviors,
                    .max_distance = 0,
                    .pin = point_pin,
                    .repeat_interval = 0,
                    .time = null,
                    .word_boundary_codepoints = &default_word_boundaries,
                    .xpos = @floatFromInt(start.position.x),
                    .ypos = @floatFromInt(start.position.y),
                });
                try self.replace(terminal, initial);
                self.resetGesture(terminal);
                self.gesture = gesture;
                self.gesture_active = true;
            },
            .update => |update| {
                if (!self.gesture_active) return error.InvalidRequest;
                if (!gestureAnchorValid(terminal, &self.gesture)) {
                    self.resetGesture(terminal);
                    return error.AnchorEvicted;
                }
                const point_pin = try pointPin(terminal.screens.active, update.point);
                const selection = self.gesture.drag(terminal, .{
                    .geometry = .{
                        .cell_width = update.geometry.cell_width,
                        .columns = update.geometry.columns,
                        .padding_left = update.geometry.padding_left,
                        .screen_height = update.geometry.screen_height,
                    },
                    .pin = point_pin,
                    .rectangle = update.rectangle,
                    .word_boundary_codepoints = &default_word_boundaries,
                    .xpos = @floatFromInt(update.position.x),
                    .ypos = @floatFromInt(update.position.y),
                });
                try self.replace(terminal, selection);
            },
            .end => |point| {
                if (!self.gesture_active) return error.InvalidRequest;
                if (!gestureAnchorValid(terminal, &self.gesture)) {
                    self.resetGesture(terminal);
                    return error.AnchorEvicted;
                }
                const point_pin = try pointPin(terminal.screens.active, point);
                self.gesture.release(terminal, .{ .pin = point_pin });
                self.gesture_active = false;
            },
            .cancel => self.resetGesture(terminal),
        }
        return self.snapshot(terminal);
    }

    pub fn applyCommand(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: anytype,
    ) (Error || std.mem.Allocator.Error)!Snapshot {
        try validateCurrent(
            request.revision,
            request.generation,
            current_revision,
            current_generation,
        );
        const screen = terminal.screens.active;
        const selection: ?ghostty_vt.Selection = switch (request.command) {
            .clear => null,
            .select_all => screen.selectAll(),
            .set_range => |range| .init(
                try pointPin(screen, range.start),
                try pointPin(screen, range.end),
                range.rectangle,
            ),
            .select_output => |point| screen.selectOutput(try pointPin(screen, point)),
        };
        try self.replace(terminal, selection);
        self.resetGesture(terminal);
        return self.snapshot(terminal);
    }

    pub fn snapshot(
        self: *const State,
        terminal: *ghostty_vt.Terminal,
    ) Error!Snapshot {
        const selection = self.selection orelse return .none;
        if (!self.screenValid(terminal) or selection.start().garbage or selection.end().garbage) {
            return .evicted;
        }
        const screen = terminal.screens.get(self.screen_key) orelse return .evicted;
        const start = screen.pages.pointFromPin(.screen, selection.start()) orelse return .evicted;
        const end = screen.pages.pointFromPin(.screen, selection.end()) orelse return .evicted;
        return .{ .active = .{
            .end = .{ .column = end.screen.x, .row = end.screen.y },
            .rectangle = selection.rectangle,
            .start = .{ .column = start.screen.x, .row = start.screen.y },
        } };
    }

    pub fn exportSelection(
        self: *State,
        terminal: *ghostty_vt.Terminal,
    ) Error!ghostty_vt.Selection {
        const selection = self.selection orelse return error.MissingSelection;
        if (selection.start().garbage or selection.end().garbage or !self.screenValid(terminal)) {
            self.release(terminal);
            return error.AnchorEvicted;
        }
        return selection;
    }

    fn replace(
        self: *State,
        terminal: *ghostty_vt.Terminal,
        selection: ?ghostty_vt.Selection,
    ) std.mem.Allocator.Error!void {
        if (selection) |untracked| {
            const tracked = try untracked.track(terminal.screens.active);
            self.release(terminal);
            self.event = null;
            self.selection = tracked;
            self.screen_key = terminal.screens.active_key;
            self.screen_generation = terminal.screens.generation(terminal.screens.active_key);
        } else {
            self.release(terminal);
            self.event = null;
        }
    }

    fn resetGesture(self: *State, terminal: *ghostty_vt.Terminal) void {
        self.gesture.reset(terminal);
        self.gesture_active = false;
    }

    fn release(self: *State, terminal: *ghostty_vt.Terminal) void {
        const selection = self.selection orelse return;
        self.selection = null;
        if (!self.screenValid(terminal)) return;
        const screen = terminal.screens.get(self.screen_key) orelse return;
        selection.deinit(screen);
    }

    fn screenValid(self: *const State, terminal: *const ghostty_vt.Terminal) bool {
        return terminal.screens.generation(self.screen_key) == self.screen_generation and
            terminal.screens.get(self.screen_key) != null;
    }
};

const default_word_boundaries = [_]u21{
    0,   ' ', '\t', '\'', '"',
    '│',
    '`', '|', ':',  ';',  ',',
    '(', ')', '[',  ']',  '{',
    '}', '<', '>',  '$',
};

pub fn validateCurrent(
    request_revision: u64,
    request_generation: u64,
    current_revision: u64,
    current_generation: u64,
) Error!void {
    if (request_revision != current_revision) return error.StaleRevision;
    if (request_generation != current_generation) return error.StaleGeneration;
}

fn pointPin(screen: *ghostty_vt.Screen, point: Point) Error!ghostty_vt.Pin {
    if (point.row > std.math.maxInt(u32)) return error.InvalidPoint;
    return screen.pages.pin(.{ .screen = .{
        .x = point.column,
        .y = @intCast(point.row),
    } }) orelse error.InvalidPoint;
}

fn gestureAnchorValid(
    terminal: *ghostty_vt.Terminal,
    gesture: *const ghostty_vt.SelectionGesture,
) bool {
    const pin = gesture.validatedLeftClickPin(&terminal.screens) orelse return false;
    return !pin.garbage and terminal.screens.active.pages.pointFromPin(.screen, pin.*) != null;
}
