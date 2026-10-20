//! Terminal-owned shell correlation and invalidation of numeric row cursors.
//! The terminal mutex owns this state; reader batches publish shell facts at commit.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");

const prefix = "orchbee.command.v1;";

pub const Tracker = struct {
    committed: protocol.TerminalCommandObservation = .{ .output_epoch = 1 },
    staged: protocol.TerminalCommandObservation = .{ .output_epoch = 1 },
    origin: ?Origin = null,
    exhausted: bool = false,

    const Origin = struct {
        pin: *ghostty_vt.Pin,
        screen_key: ghostty_vt.ScreenSet.Key,
        screen_generation: usize,
    };

    pub fn deinit(self: *Tracker, terminal: *ghostty_vt.Terminal) void {
        const origin = self.origin orelse return;
        self.origin = null;
        if (terminal.screens.generation(origin.screen_key) != origin.screen_generation) return;
        const screen = terminal.screens.get(origin.screen_key) orelse return;
        screen.pages.untrackPin(origin.pin);
    }

    /// A tracked oldest-row pin detects eviction even when retained row count is unchanged.
    pub fn refresh(self: *Tracker, terminal: *ghostty_vt.Terminal) !void {
        if (self.exhausted) return error.OutputCursorExhausted;
        const screen = terminal.screens.active;
        const key = terminal.screens.active_key;
        const generation = terminal.screens.generation(key);
        if (self.origin) |origin| {
            if (origin.screen_key == key and origin.screen_generation == generation and
                !origin.pin.garbage)
            {
                if (screen.pages.pointFromPin(.screen, origin.pin.*)) |point| {
                    if (point.screen.y == 0) return;
                }
            }
            self.deinit(terminal);
            self.committed.output_epoch = std.math.add(u64, self.committed.output_epoch, 1) catch {
                self.exhausted = true;
                return error.OutputCursorExhausted;
            };
            self.staged.output_epoch = self.committed.output_epoch;
        }
        const pin = screen.pages.pin(.{ .screen = .{ .x = 0, .y = 0 } }) orelse
            return error.InvalidPoint;
        self.origin = .{
            .pin = try screen.pages.trackPin(pin),
            .screen_key = key,
            .screen_generation = generation,
        };
    }

    pub fn begin(self: *Tracker) void {
        self.staged = self.committed;
    }

    pub fn commit(self: *Tracker) void {
        self.committed = self.staged;
    }

    pub fn accept(
        self: *Tracker,
        terminal: *ghostty_vt.Terminal,
        generation: u64,
        bytes: []const u8,
    ) !bool {
        if (!std.mem.startsWith(u8, bytes, prefix)) return false;
        var parts = std.mem.splitScalar(u8, bytes[prefix.len..], ';');
        const action = parts.next() orelse return true;
        if (std.mem.eql(u8, action, "ready")) {
            if (parts.next() == null) self.staged.ready = true;
            return true;
        }
        const text_id = parts.next() orelse return true;
        if (text_id.len != 32) return true;
        for (text_id) |byte| {
            if (!std.ascii.isDigit(byte) and !(byte >= 'a' and byte <= 'f')) return true;
        }
        var id: [16]u8 = undefined;
        _ = std.fmt.hexToBytes(&id, text_id) catch return true;
        if (std.mem.allEqual(u8, &id, 0)) return true;
        const screen = terminal.screens.active;
        const row = screen.pages.total_rows - terminal.rows + screen.cursor.y;
        if (std.mem.eql(u8, action, "start")) {
            if (parts.next() != null) return true;
            try self.refresh(terminal);
            self.staged.ready = false;
            self.staged.command = .{
                .id = id,
                .start_row = @intCast(row),
                .start_column = screen.cursor.x,
                .generation = generation,
                .output_epoch = self.staged.output_epoch,
            };
        } else if (std.mem.eql(u8, action, "finish")) {
            const status = parts.next() orelse return true;
            if (parts.next() != null) return true;
            const exit_code = std.fmt.parseInt(i32, status, 10) catch return true;
            const command = if (self.staged.command) |*value| value else return true;
            if (command.completed or !std.mem.eql(u8, &command.id, &id)) return true;
            command.completed = true;
            command.exit_code = exit_code;
            command.end_row = @intCast(row);
            command.end_column = screen.cursor.x;
        }
        return true;
    }
};
