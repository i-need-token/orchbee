//! Test probe for terminal text: the complete active screen, history and
//! viewport, as plain text with trailing blanks trimmed and soft wraps joined.
//! It reads one committed terminal state under the terminal lock and never
//! changes the viewport, selection, or any other runtime state.
const harness = @import("runtime_harness.zig");
const std = harness.std;
const ghostty_vt = harness.ghostty_vt;

pub const ScreenText = struct {
    allocator: std.mem.Allocator,
    storage: []u8,
    byte_length: usize,
    geometry: harness.subject.Geometry,
    screen: enum { primary, alternate },
    total_rows: u64,

    pub fn text(self: *const ScreenText) []const u8 {
        return self.storage[0..self.byte_length];
    }

    pub fn deinit(self: *ScreenText) void {
        self.allocator.free(self.storage);
        self.* = undefined;
    }
};

/// Fails with `error.ScreenTextTooLarge` when the text exceeds `maximum_bytes`.
pub fn read(
    runtime: *harness.TerminalRuntime,
    allocator: std.mem.Allocator,
    maximum_bytes: usize,
) !ScreenText {
    // One extra byte distinguishes an exact fit from an overflow.
    const storage = try allocator.alloc(u8, maximum_bytes + 1);
    errdefer allocator.free(storage);
    const state = harness.runtimeState(runtime);
    state.terminal_mutex.lock();
    defer state.terminal_mutex.unlock();
    // A reader batch commits as a whole; never read in the middle of one.
    while (state.reader_commit_in_progress) state.terminal_condition.wait(&state.terminal_mutex);
    const terminal = &state.terminal;
    var writer: std.Io.Writer = .fixed(storage);
    var formatter: ghostty_vt.formatter.ScreenFormatter = .init(
        terminal.screens.active,
        .{ .emit = .plain, .trim = true, .unwrap = true },
    );
    formatter.format(&writer) catch return error.ScreenTextTooLarge;
    if (writer.end > maximum_bytes) return error.ScreenTextTooLarge;
    return .{
        .allocator = allocator,
        .storage = storage,
        .byte_length = writer.end,
        .geometry = .{
            .cell_pixel_height = state.cell_pixel_height,
            .cell_pixel_width = state.cell_pixel_width,
            .columns = terminal.cols,
            .generation = state.generation,
            .revision = state.revision.load(.monotonic),
            .rows = terminal.rows,
        },
        .screen = if (terminal.screens.active_key == .primary) .primary else .alternate,
        .total_rows = terminal.screens.active.pages.total_rows,
    };
}
