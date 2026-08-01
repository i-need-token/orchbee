const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");

pub const max_frame_bytes = 8 * 1024 * 1024;
pub const max_frame_cells = 256 * 1024;
pub const max_frame_strings = 4096;
pub const max_frame_styles = 1024;
pub const max_string_bytes = 16 * 1024;
pub const no_string_index = std.math.maxInt(u32);
pub const no_base_revision = std.math.maxInt(u64);
pub const color_default = 0x01000000;
pub const color_none = 0x02000000;

pub const ProjectionError = error{
    CaptureAllocationFailed,
    ProjectionTooLarge,
};

pub const Counters = struct {
    render_state_creations: u64 = 0,
    captures: u64 = 0,
    encodes: u64 = 0,
};

pub const FrameKind = enum {
    snapshot,
    delta,
};

pub const EncodeRequest = struct {
    attachment_id: [16]u8,
    base_generation: ?u64,
    base_revision: ?u64,
    force_snapshot: bool,
    host_epoch: [16]u8,
    maximum_bytes: usize,
    sequence: u64,
};

pub const EncodedFrame = struct {
    base_revision: ?u64,
    bytes: []u8,
    generation: u64,
    kind: FrameKind,
    revision: u64,
    sequence: u64,
};

const LinkRow = struct {
    bytes: std.ArrayList(u8) = .empty,
    lengths: std.ArrayList(u32) = .empty,
    offsets: std.ArrayList(u32) = .empty,

    fn deinit(self: *LinkRow, allocator: std.mem.Allocator) void {
        self.bytes.deinit(allocator);
        self.lengths.deinit(allocator);
        self.offsets.deinit(allocator);
        self.* = .{};
    }

    fn clear(self: *LinkRow) void {
        self.bytes.clearRetainingCapacity();
        self.lengths.clearRetainingCapacity();
        self.offsets.clearRetainingCapacity();
    }

    fn rebuild(
        self: *LinkRow,
        allocator: std.mem.Allocator,
        pin: ghostty_vt.PageList.Pin,
        cells: []const ghostty_vt.page.Cell,
    ) (std.mem.Allocator.Error || error{ProjectionTooLarge})!void {
        self.clear();
        var any_link = false;
        for (cells) |cell| any_link = any_link or cell.hyperlink;
        if (!any_link) return;

        try self.offsets.resize(allocator, cells.len);
        try self.lengths.resize(allocator, cells.len);
        @memset(self.offsets.items, no_string_index);
        @memset(self.lengths.items, 0);

        const page: *ghostty_vt.Page = &pin.node.data;
        for (cells, 0..) |cell, column| {
            if (!cell.hyperlink) continue;
            const page_cell = page.getRowAndCell(column, pin.y).cell;
            const identifier = page.lookupHyperlink(page_cell) orelse continue;
            const entry = page.hyperlink_set.get(page.memory, identifier);
            const target = entry.uri.slice(page.memory);
            if (target.len > max_string_bytes or target.len > max_frame_bytes -| self.bytes.items.len) {
                return error.ProjectionTooLarge;
            }
            self.offsets.items[column] = @intCast(self.bytes.items.len);
            self.lengths.items[column] = @intCast(target.len);
            try self.bytes.appendSlice(allocator, target);
        }
    }

    fn uri(self: *const LinkRow, column: usize) ?[]const u8 {
        if (column >= self.offsets.items.len) return null;
        const offset = self.offsets.items[column];
        const length = self.lengths.items[column];
        if (offset == no_string_index or length == 0) return null;
        return self.bytes.items[offset..][0..length];
    }
};

pub const Projection = struct {
    allocator: std.mem.Allocator,
    capture_full: bool = false,
    captured_cell_pixel_height: u16 = 1,
    captured_cell_pixel_width: u16 = 1,
    captured_current_directory: ?[]u8 = null,
    captured_generation: u64 = 1,
    captured_interaction: u32 = 0,
    captured_revision: u64 = 0,
    captured_scrollback_rows: u64 = 0,
    captured_title: ?[]u8 = null,
    counters: Counters = .{},
    links: std.ArrayList(LinkRow) = .empty,
    render_state: ?ghostty_vt.RenderState = null,
    row_revisions: std.ArrayList(u64) = .empty,

    pub fn init(allocator: std.mem.Allocator) Projection {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Projection) void {
        self.resetRenderState();
        self.links.deinit(self.allocator);
        self.row_revisions.deinit(self.allocator);
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.* = undefined;
    }

    pub fn resetAfterCaptureFailure(self: *Projection) void {
        self.resetRenderState();
    }

    fn resetRenderState(self: *Projection) void {
        if (self.render_state) |*state| state.deinit(self.allocator);
        self.render_state = null;
        self.row_revisions.clearRetainingCapacity();
        for (self.links.items) |*row| row.deinit(self.allocator);
        self.links.clearRetainingCapacity();
    }

    pub fn beginCapture(
        self: *Projection,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) (std.mem.Allocator.Error || error{ProjectionTooLarge})!void {
        if (self.render_state == null) {
            self.render_state = .empty;
            self.counters.render_state_creations += 1;
        }
        const state = &self.render_state.?;
        try state.beginUpdate(self.allocator, terminal);

        const dimensions_changed = self.row_revisions.items.len != state.rows;
        self.capture_full = dimensions_changed or generation != self.captured_generation or
            state.dirty == .full;

        while (self.links.items.len > state.rows) {
            var row = self.links.pop().?;
            row.deinit(self.allocator);
        }
        while (self.links.items.len < state.rows) try self.links.append(self.allocator, .{});
        const previous_row_count = self.row_revisions.items.len;
        try self.row_revisions.resize(self.allocator, state.rows);
        if (state.rows > previous_row_count) {
            @memset(self.row_revisions.items[previous_row_count..], revision);
        }

        const row_data = state.row_data.slice();
        const row_dirties = row_data.items(.dirty);
        const row_pins = row_data.items(.pin);
        const row_cells = row_data.items(.cells);
        for (0..state.rows) |row_index| {
            if (!self.capture_full and !row_dirties[row_index]) continue;
            try self.links.items[row_index].rebuild(
                self.allocator,
                row_pins[row_index],
                row_cells[row_index].items(.raw),
            );
        }

        const title = try duplicateOptional(self.allocator, terminal.getTitle());
        errdefer if (title) |value| self.allocator.free(value);
        const current_directory = try duplicateOptional(self.allocator, terminal.getPwd());
        errdefer if (current_directory) |value| self.allocator.free(value);
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.captured_title = title;
        self.captured_current_directory = current_directory;
        self.captured_revision = revision;
        self.captured_generation = generation;
        self.captured_cell_pixel_width = cell_pixel_width;
        self.captured_cell_pixel_height = cell_pixel_height;
        self.captured_scrollback_rows = terminal.screens.active.pages.total_rows - terminal.rows;
        self.captured_interaction = terminalInteraction(terminal);
    }

    pub fn endCapture(self: *Projection) void {
        const state = &self.render_state.?;
        state.endUpdate();
        const dirties = state.row_data.items(.dirty);
        for (dirties, 0..) |*dirty, row_index| {
            if (self.capture_full or dirty.*) self.row_revisions.items[row_index] = self.captured_revision;
            dirty.* = false;
        }
        state.dirty = .false;
        self.counters.captures += 1;
    }

    pub fn needsCapture(self: *const Projection, revision: u64, generation: u64) bool {
        return self.render_state == null or self.captured_revision < revision or
            self.captured_generation != generation;
    }

    pub fn encode(
        self: *Projection,
        request: EncodeRequest,
    ) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
        const state = if (self.render_state) |*value| value else return error.CaptureAllocationFailed;
        const snapshot = request.force_snapshot or request.base_revision == null or
            request.base_generation == null or
            request.base_generation.? != self.captured_generation;
        const kind: FrameKind = if (snapshot) .snapshot else .delta;
        const base_revision: ?u64 = if (snapshot) null else request.base_revision;
        if (!snapshot and base_revision.? >= self.captured_revision) return error.ProjectionTooLarge;

        var strings = StringTable.init(self.allocator);
        defer strings.deinit();
        var styles = StyleTable.init(self.allocator);
        defer styles.deinit();
        const title_index = try strings.addOptional(self.captured_title);
        const current_directory_index = try strings.addOptional(self.captured_current_directory);

        var included_rows: usize = 0;
        const row_data = state.row_data.slice();
        const row_cells = row_data.items(.cells);
        for (0..state.rows) |row_index| {
            if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
            included_rows += 1;
            const cells = row_cells[row_index].slice();
            for (0..state.cols) |column| {
                var grapheme_buffer: [max_string_bytes]u8 = undefined;
                const grapheme = try cellGrapheme(
                    cells.items(.raw)[column],
                    cells.items(.grapheme)[column],
                    &grapheme_buffer,
                );
                _ = try strings.add(grapheme);
                const hyperlink_index = try strings.addOptional(self.links.items[row_index].uri(column));
                const style = styleRecord(
                    cells.items(.raw)[column],
                    capturedStyle(cells, column),
                    &state.colors,
                    hyperlink_index,
                );
                _ = try styles.add(style);
            }
        }

        const hard_limit = @min(request.maximum_bytes, max_frame_bytes);
        var writer = LimitedWriter.init(self.allocator, hard_limit);
        errdefer writer.deinit();
        try writer.appendSlice("OBTF");
        try writer.appendInt(u8, 2);
        try writer.appendInt(u8, 0);
        try writer.appendInt(u8, if (snapshot) 1 else 2);
        try writer.appendInt(u8, 0);
        const total_length_offset = writer.bytes.items.len;
        try writer.appendInt(u32, 0);
        try writer.appendInt(u16, 160);
        try writer.appendInt(u16, 0);
        try writer.appendSlice(&request.host_epoch);
        try writer.appendSlice(&request.attachment_id);
        try writer.appendInt(u64, request.sequence);
        try writer.appendInt(u64, base_revision orelse no_base_revision);
        try writer.appendInt(u64, self.captured_revision);
        try writer.appendInt(u64, self.captured_generation);
        try writer.appendInt(u64, self.captured_scrollback_rows);
        try writer.appendInt(u16, state.cols);
        try writer.appendInt(u16, state.rows);
        const cursor = state.cursor.viewport;
        const cursor_column: u16 = if (cursor) |value| value.x else @min(state.cursor.active.x, state.cols - 1);
        const cursor_row: u16 = if (cursor) |value| value.y else @min(state.cursor.active.y, state.rows - 1);
        try writer.appendInt(u16, cursor_column);
        try writer.appendInt(u16, cursor_row);
        try writer.appendInt(u8, cursorShape(state.cursor.visual_style));
        try writer.appendInt(u8, (if (state.cursor.visible and cursor != null) @as(u8, 1) else 0) |
            (if (state.cursor.blinking) @as(u8, 2) else 0) |
            (if (cursor != null and cursor.?.wide_tail) @as(u8, 4) else 0));
        try writer.appendInt(u8, if (state.screen == .primary) 1 else 2);
        try writer.appendInt(u8, 0);
        try writer.appendInt(u32, self.captured_interaction);
        try writer.appendInt(u32, rgbCode(state.colors.foreground));
        try writer.appendInt(u32, rgbCode(state.colors.background));
        try writer.appendInt(u32, if (state.colors.cursor) |value| rgbCode(value) else color_default);
        try writer.appendInt(u32, title_index);
        try writer.appendInt(u32, current_directory_index);
        try writer.appendInt(u32, @intCast(strings.values.items.len));
        try writer.appendInt(u16, @intCast(styles.values.items.len));
        try writer.appendInt(u16, @intCast(included_rows));
        try writer.appendInt(u16, 0);
        try writer.appendInt(u16, 0);
        try writer.appendInt(u16, self.captured_cell_pixel_width);
        try writer.appendInt(u16, self.captured_cell_pixel_height);
        try writer.appendInt(u32, @intCast(included_rows * state.cols));
        try writer.appendZeros(16);
        std.debug.assert(writer.bytes.items.len == 160);

        for (strings.values.items) |value| {
            try writer.appendInt(u32, @intCast(value.len));
            try writer.appendSlice(value);
        }
        for (styles.values.items) |style| {
            try writer.appendInt(u32, style.foreground);
            try writer.appendInt(u32, style.background);
            try writer.appendInt(u32, style.underline);
            try writer.appendInt(u32, style.hyperlink_index);
            try writer.appendInt(u16, style.flags);
            try writer.appendInt(u8, style.underline_kind);
            try writer.appendInt(u8, 0);
        }

        const row_raw = row_data.items(.raw);
        for (0..state.rows) |row_index| {
            if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
            const row = row_raw[row_index];
            try writer.appendInt(u16, @intCast(row_index));
            try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
                (if (row.wrap_continuation) @as(u8, 2) else 0));
            try writer.appendInt(u8, rowPromptClass(row.semantic_prompt));
            try writer.appendInt(u16, state.cols);
            try writer.appendInt(u16, 0);
            const cells = row_cells[row_index].slice();
            for (0..state.cols) |column| {
                var grapheme_buffer: [max_string_bytes]u8 = undefined;
                const grapheme = try cellGrapheme(
                    cells.items(.raw)[column],
                    cells.items(.grapheme)[column],
                    &grapheme_buffer,
                );
                const grapheme_index = strings.indexOf(grapheme).?;
                const hyperlink_index = strings.indexOfOptional(self.links.items[row_index].uri(column));
                const style = styleRecord(
                    cells.items(.raw)[column],
                    capturedStyle(cells, column),
                    &state.colors,
                    hyperlink_index,
                );
                try writer.appendInt(u32, grapheme_index);
                try writer.appendInt(u16, styles.indexOf(style).?);
                const raw = cells.items(.raw)[column];
                try writer.appendInt(u8, cellWidth(raw.wide) |
                    (semanticContent(raw.semantic_content) << 2) |
                    (if (raw.protected) @as(u8, 0x10) else 0));
                try writer.appendInt(u8, 0);
            }
        }

        if (writer.bytes.items.len > std.math.maxInt(u32)) return error.ProjectionTooLarge;
        std.mem.writeInt(u32, writer.bytes.items[total_length_offset..][0..4], @intCast(writer.bytes.items.len), .little);
        self.counters.encodes += 1;
        return .{
            .base_revision = base_revision,
            .bytes = try writer.toOwnedSlice(),
            .generation = self.captured_generation,
            .kind = kind,
            .revision = self.captured_revision,
            .sequence = request.sequence,
        };
    }
};

fn duplicateOptional(allocator: std.mem.Allocator, value: ?[:0]const u8) !?[]u8 {
    const bytes = value orelse return null;
    if (bytes.len > max_string_bytes) return error.ProjectionTooLarge;
    return try allocator.dupe(u8, bytes);
}

pub fn terminalInteraction(terminal: *const ghostty_vt.Terminal) u32 {
    var result: u32 = 0;
    if (terminal.modes.get(.cursor_keys)) result |= 1;
    if (terminal.modes.get(.keypad_keys)) result |= 1 << 1;
    if (terminal.modes.get(.bracketed_paste)) result |= 1 << 2;
    if (terminal.modes.get(.focus_event)) result |= 1 << 3;
    result |= switch (terminal.flags.mouse_event) {
        .none => 0,
        .x10 => 1 << 4,
        .normal => 1 << 5,
        .button => 1 << 6,
        .any => 1 << 7,
    };
    result |= switch (terminal.flags.mouse_format) {
        .x10 => 0,
        .utf8 => 1 << 8,
        .sgr => 1 << 9,
        .urxvt => 1 << 10,
        .sgr_pixels => 1 << 11,
    };
    if (terminal.flags.modify_other_keys_2) result |= 1 << 12;
    const kitty = terminal.screens.active.kitty_keyboard.current();
    if (kitty.disambiguate) result |= 1 << 13;
    if (kitty.report_events) result |= 1 << 14;
    if (kitty.report_alternates) result |= 1 << 15;
    if (kitty.report_all) result |= 1 << 16;
    if (kitty.report_associated) result |= 1 << 17;
    return result;
}

fn cursorShape(style: anytype) u8 {
    return switch (style) {
        .block, .block_hollow => 1,
        .bar => 2,
        .underline => 3,
    };
}

pub fn cellGrapheme(
    cell: ghostty_vt.page.Cell,
    trailing: []const u21,
    buffer: *[max_string_bytes]u8,
) error{ProjectionTooLarge}![]const u8 {
    var offset: usize = 0;
    if (cell.codepoint() != 0) {
        var encoded: [4]u8 = undefined;
        const length = std.unicode.utf8Encode(cell.codepoint(), &encoded) catch
            return error.ProjectionTooLarge;
        @memcpy(buffer[offset..][0..length], encoded[0..length]);
        offset += length;
    }
    if (cell.hasGrapheme()) {
        for (trailing) |codepoint| {
            var encoded: [4]u8 = undefined;
            const length = std.unicode.utf8Encode(codepoint, &encoded) catch
                return error.ProjectionTooLarge;
            if (length > buffer.len - offset) return error.ProjectionTooLarge;
            @memcpy(buffer[offset..][0..length], encoded[0..length]);
            offset += length;
        }
    }
    return buffer[0..offset];
}

pub fn cellWidth(value: ghostty_vt.page.Cell.Wide) u8 {
    return switch (value) {
        .narrow => 0,
        .wide => 1,
        .spacer_tail => 2,
        .spacer_head => 3,
    };
}

pub fn semanticContent(value: ghostty_vt.page.Cell.SemanticContent) u8 {
    return switch (value) {
        .output => 0,
        .input => 1,
        .prompt => 2,
    };
}

pub fn rowPromptClass(value: ghostty_vt.page.Row.SemanticPrompt) u8 {
    return switch (value) {
        .none => 0,
        .prompt => 1,
        .prompt_continuation => 2,
    };
}

pub const StyleRecord = struct {
    background: u32,
    flags: u16,
    foreground: u32,
    hyperlink_index: u32,
    underline: u32,
    underline_kind: u8,
};

pub fn styleRecord(
    cell: ghostty_vt.page.Cell,
    captured_style: ghostty_vt.Style,
    colors: *const ghostty_vt.RenderState.Colors,
    hyperlink_index: u32,
) StyleRecord {
    const flags = captured_style.flags;
    return .{
        .background = if (captured_style.bg(&cell, &colors.palette)) |value| rgbCode(value) else color_default,
        .flags = (if (flags.bold) @as(u16, 1) else 0) |
            (if (flags.italic) @as(u16, 1 << 1) else 0) |
            (if (flags.faint) @as(u16, 1 << 2) else 0) |
            (if (flags.blink) @as(u16, 1 << 3) else 0) |
            (if (flags.inverse) @as(u16, 1 << 4) else 0) |
            (if (flags.invisible) @as(u16, 1 << 5) else 0) |
            (if (flags.strikethrough) @as(u16, 1 << 6) else 0) |
            (if (flags.overline) @as(u16, 1 << 7) else 0),
        .foreground = colorValue(captured_style.fg_color, &colors.palette, color_default),
        .hyperlink_index = hyperlink_index,
        .underline = colorValue(captured_style.underline_color, &colors.palette, color_none),
        .underline_kind = switch (flags.underline) {
            .none => 0,
            .single => 1,
            .double => 2,
            .curly => 3,
            .dotted => 4,
            .dashed => 5,
        },
    };
}

fn capturedStyle(cells: anytype, column: usize) ghostty_vt.Style {
    const cell = cells.items(.raw)[column];
    if (cell.hasStyling() or cell.content_tag == .bg_color_palette or
        cell.content_tag == .bg_color_rgb)
    {
        return cells.items(.style)[column];
    }
    return .{};
}

fn colorValue(value: ghostty_vt.Style.Color, palette: *const ghostty_vt.color.Palette, fallback: u32) u32 {
    return switch (value) {
        .none => fallback,
        .palette => |index| rgbCode(palette[index]),
        .rgb => |rgb| rgbCode(rgb),
    };
}

pub fn rgbCode(rgb: ghostty_vt.color.RGB) u32 {
    return (@as(u32, rgb.r) << 16) | (@as(u32, rgb.g) << 8) | rgb.b;
}

pub const StringTable = struct {
    allocator: std.mem.Allocator,
    indices: std.StringHashMapUnmanaged(u32) = .empty,
    values: std.ArrayList([]u8) = .empty,

    pub fn init(allocator: std.mem.Allocator) StringTable {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *StringTable) void {
        self.indices.deinit(self.allocator);
        for (self.values.items) |value| self.allocator.free(value);
        self.values.deinit(self.allocator);
    }

    pub fn addOptional(self: *StringTable, value: ?[]const u8) !u32 {
        return if (value) |bytes| self.add(bytes) else no_string_index;
    }

    pub fn add(self: *StringTable, value: []const u8) !u32 {
        if (value.len > max_string_bytes) return error.ProjectionTooLarge;
        if (self.indices.get(value)) |index| return index;
        if (self.values.items.len >= max_frame_strings) return error.ProjectionTooLarge;
        const copy = try self.allocator.dupe(u8, value);
        errdefer self.allocator.free(copy);
        try self.values.append(self.allocator, copy);
        errdefer _ = self.values.pop();
        const index: u32 = @intCast(self.values.items.len - 1);
        try self.indices.put(self.allocator, copy, index);
        return index;
    }

    pub fn indexOf(self: *const StringTable, value: []const u8) ?u32 {
        return self.indices.get(value);
    }

    pub fn indexOfOptional(self: *const StringTable, value: ?[]const u8) u32 {
        return if (value) |bytes| self.indexOf(bytes).? else no_string_index;
    }
};

pub const StyleTable = struct {
    allocator: std.mem.Allocator,
    indices: std.AutoHashMapUnmanaged(StyleRecord, u16) = .empty,
    values: std.ArrayList(StyleRecord) = .empty,

    pub fn init(allocator: std.mem.Allocator) StyleTable {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *StyleTable) void {
        self.indices.deinit(self.allocator);
        self.values.deinit(self.allocator);
    }

    pub fn add(self: *StyleTable, value: StyleRecord) !u16 {
        if (self.indices.get(value)) |index| return index;
        if (self.values.items.len >= max_frame_styles) return error.ProjectionTooLarge;
        try self.values.append(self.allocator, value);
        errdefer _ = self.values.pop();
        const index: u16 = @intCast(self.values.items.len - 1);
        try self.indices.put(self.allocator, value, index);
        return index;
    }

    pub fn indexOf(self: *const StyleTable, value: StyleRecord) ?u16 {
        return self.indices.get(value);
    }
};

pub const LimitedWriter = struct {
    allocator: std.mem.Allocator,
    bytes: std.ArrayList(u8) = .empty,
    limit: usize,

    pub fn init(allocator: std.mem.Allocator, limit: usize) LimitedWriter {
        return .{ .allocator = allocator, .limit = limit };
    }

    pub fn deinit(self: *LimitedWriter) void {
        self.bytes.deinit(self.allocator);
    }

    pub fn appendSlice(self: *LimitedWriter, value: []const u8) !void {
        if (value.len > self.limit -| self.bytes.items.len) return error.ProjectionTooLarge;
        try self.bytes.appendSlice(self.allocator, value);
    }

    pub fn appendZeros(self: *LimitedWriter, count: usize) !void {
        if (count > self.limit -| self.bytes.items.len) return error.ProjectionTooLarge;
        const start = self.bytes.items.len;
        try self.bytes.resize(self.allocator, start + count);
        @memset(self.bytes.items[start..], 0);
    }

    pub fn appendInt(self: *LimitedWriter, comptime Integer: type, value: Integer) !void {
        var buffer: [@sizeOf(Integer)]u8 = undefined;
        if (@sizeOf(Integer) == 1) {
            buffer[0] = @intCast(value);
        } else {
            std.mem.writeInt(Integer, &buffer, value, .little);
        }
        try self.appendSlice(&buffer);
    }

    pub fn toOwnedSlice(self: *LimitedWriter) ![]u8 {
        return self.bytes.toOwnedSlice(self.allocator);
    }
};

test "no-demand projection owns no render state or frame allocation" {
    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try std.testing.expect(projection.render_state == null);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.render_state_creations);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.captures);
    try std.testing.expectEqual(@as(u64, 0), projection.counters.encodes);
}

test "projection captures Ghostty state into a valid format 2 snapshot" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 4,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("A\x1b[31mB");

    var projection = Projection.init(std.testing.allocator);
    defer projection.deinit();
    try projection.beginCapture(&terminal, 1, 1, 8, 16);
    projection.endCapture();
    const frame = try projection.encode(.{
        .attachment_id = [_]u8{2} ** 16,
        .base_generation = null,
        .base_revision = null,
        .force_snapshot = true,
        .host_epoch = [_]u8{1} ** 16,
        .maximum_bytes = max_frame_bytes,
        .sequence = 1,
    });
    defer std.testing.allocator.free(frame.bytes);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    try std.testing.expectEqual(protocol.FrameKind.snapshot, summary.kind);
    try std.testing.expectEqual(@as(u64, 1), summary.revision);
    try std.testing.expectEqual(@as(u16, 4), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
}
