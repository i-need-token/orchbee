const std = @import("std");
const target_builtin = @import("builtin");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");

const max_frame_strings = protocol.Limits.frame_strings;
const max_frame_styles = protocol.Limits.frame_styles;
pub const max_string_bytes = protocol.Limits.string_bytes;
pub const no_string_index = std.math.maxInt(u32);
pub const no_base_revision = std.math.maxInt(u64);
pub const color_default = protocol.color_default;
const color_none = protocol.color_none;
pub const color_palette = protocol.color_palette;

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
    if (terminal.modes.get(.mouse_alternate_scroll)) result |= 1 << 18;
    return result;
}

pub fn cursorShape(style: anytype) u8 {
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

/// The soft-wrap topology projected for one Ghostty page row.
///
/// Ghostty joins two rows when the upper row wraps, as its selection and reflow do. Some edits
/// leave the page's lower half of that pair disagreeing: inserting or deleting lines or ED1 inside
/// a wrapped line keeps or drops a continuation, and erasing a row that starts with a wrapped wide
/// character keeps the spacer head above it. A row therefore continues exactly when the page row
/// above it wraps, and keeps its trailing spacer head only while it wraps into a row that starts
/// with the wide character. Every adjacent pair then satisfies the projection formats' wrap and
/// spacer-head invariants, whichever window of rows a frame carries.
pub const RowTopology = packed struct(u8) {
    wrap: bool = false,
    continuation: bool = false,
    spacer_head: bool = false,
    reserved: u5 = 0,

    pub fn read(pin: ghostty_vt.Pin) RowTopology {
        const row = pin.rowAndCell().row;
        const cells = pin.cells(.all);
        const spacer_head = row.wrap and cells[cells.len - 1].wide == .spacer_head and
            if (pin.down(1)) |below| below.cells(.all)[0].wide == .wide else true;
        return .{
            .wrap = row.wrap,
            .continuation = if (pin.up(1)) |above| above.rowAndCell().row.wrap else false,
            .spacer_head = spacer_head,
        };
    }

    /// The row header's wrap and wrap-continuation bits.
    pub fn flags(self: RowTopology) u8 {
        return @as(u8, @intFromBool(self.wrap)) | @as(u8, @intFromBool(self.continuation)) << 1;
    }

    /// The width code of a cell in this row. A spacer head that no longer leads a wrapped wide
    /// character is a narrow blank.
    pub fn width(self: RowTopology, wide: ghostty_vt.page.Cell.Wide, column: usize, columns: usize) u8 {
        if (wide == .spacer_head and !(self.spacer_head and column + 1 == columns)) {
            return cellWidth(.narrow);
        }
        return cellWidth(wide);
    }
};

pub const StyleRecord = struct {
    background: u32,
    flags: u16,
    foreground: u32,
    hyperlink_index: u32,
    underline: u32,
    underline_kind: u8,
};

pub const DefaultColors = struct {
    foreground: u32 = color_default,
    background: u32 = color_default,
    palette_overrides: ghostty_vt.color.PaletteMask = .initEmpty(),
};

pub fn defaultColors(
    terminal: *const ghostty_vt.Terminal,
    colors: *const ghostty_vt.RenderState.Colors,
) DefaultColors {
    // Ghostty owns override provenance even when an OSC value equals its
    // configured default. Reverse-screen mode already swaps the captured
    // effective colors and must remain visible to host-relative consumers.
    if (terminal.modes.get(.reverse_colors)) return .{
        .foreground = rgbCode(colors.foreground),
        .background = rgbCode(colors.background),
        .palette_overrides = terminal.colors.palette.mask,
    };
    return .{
        .foreground = if (terminal.colors.foreground.override) |value| rgbCode(value) else color_default,
        .background = if (terminal.colors.background.override) |value| rgbCode(value) else color_default,
        .palette_overrides = terminal.colors.palette.mask,
    };
}

pub fn styleRecord(
    cell: ghostty_vt.page.Cell,
    captured_style: ghostty_vt.Style,
    colors: *const ghostty_vt.RenderState.Colors,
    defaults: DefaultColors,
    hyperlink_index: u32,
) StyleRecord {
    const flags = captured_style.flags;
    return .{
        .background = switch (cell.content_tag) {
            .bg_color_palette => paletteValue(cell.content.color_palette.data, &colors.palette, defaults.palette_overrides),
            .bg_color_rgb => rgbCode(.{
                .r = cell.content.color_rgb.r,
                .g = cell.content.color_rgb.g,
                .b = cell.content.color_rgb.b,
            }),
            else => colorValue(captured_style.bg_color, &colors.palette, defaults.background, defaults.palette_overrides),
        },
        .flags = (if (flags.bold) @as(u16, 1) else 0) |
            (if (flags.italic) @as(u16, 1 << 1) else 0) |
            (if (flags.faint) @as(u16, 1 << 2) else 0) |
            (if (flags.blink) @as(u16, 1 << 3) else 0) |
            (if (flags.inverse) @as(u16, 1 << 4) else 0) |
            (if (flags.invisible) @as(u16, 1 << 5) else 0) |
            (if (flags.strikethrough) @as(u16, 1 << 6) else 0) |
            (if (flags.overline) @as(u16, 1 << 7) else 0),
        .foreground = colorValue(captured_style.fg_color, &colors.palette, defaults.foreground, defaults.palette_overrides),
        .hyperlink_index = hyperlink_index,
        .underline = colorValue(captured_style.underline_color, &colors.palette, color_none, defaults.palette_overrides),
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

pub fn capturedStyle(cells: anytype, column: usize) ghostty_vt.Style {
    const cell = cells.items(.raw)[column];
    if (cell.hasStyling()) return cells.items(.style)[column];
    return .{};
}

fn colorValue(
    value: ghostty_vt.Style.Color,
    palette: *const ghostty_vt.color.Palette,
    fallback: u32,
    overrides: ghostty_vt.color.PaletteMask,
) u32 {
    return switch (value) {
        .none => fallback,
        .palette => |index| paletteValue(index, palette, overrides),
        .rgb => |rgb| rgbCode(rgb),
    };
}

fn paletteValue(index: u8, palette: *const ghostty_vt.color.Palette, overrides: ghostty_vt.color.PaletteMask) u32 {
    return if (index < 16 and !overrides.isSet(index)) color_palette | @as(u32, index) else rgbCode(palette[index]);
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
        return if (value) |bytes| blk: {
            if (!std.unicode.utf8ValidateSlice(bytes)) break :blk no_string_index;
            break :blk try self.add(bytes);
        } else no_string_index;
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
        return if (value) |bytes| if (std.unicode.utf8ValidateSlice(bytes))
            self.indexOf(bytes) orelse no_string_index
        else
            no_string_index else no_string_index;
    }
};

const LinkStringMapTestingState = if (target_builtin.is_test) struct {
    resolutions: usize = 0,
} else struct {};

pub const LinkStringMap = struct {
    allocator: std.mem.Allocator,
    indices: []u32,
    testing_state: LinkStringMapTestingState = .{},

    pub fn init(allocator: std.mem.Allocator, value_count: usize) !LinkStringMap {
        const indices = try allocator.alloc(u32, value_count);
        @memset(indices, no_string_index);
        return .{ .allocator = allocator, .indices = indices };
    }

    pub fn deinit(self: *LinkStringMap) void {
        self.allocator.free(self.indices);
        self.* = undefined;
    }

    pub fn resolve(
        self: *LinkStringMap,
        link_table_index: u32,
        link_table_values: anytype,
        strings: *StringTable,
    ) !u32 {
        if (link_table_index == no_string_index) return no_string_index;
        std.debug.assert(link_table_index < self.indices.len);
        const mapped_index = &self.indices[link_table_index];
        if (mapped_index.* != no_string_index) return mapped_index.*;
        if (comptime target_builtin.is_test) self.testing_state.resolutions += 1;
        mapped_index.* = try strings.add(link_table_values[link_table_index]);
        return mapped_index.*;
    }

    pub fn mapped(self: *const LinkStringMap, link_table_index: u32) u32 {
        if (link_table_index == no_string_index) return no_string_index;
        std.debug.assert(link_table_index < self.indices.len);
        const index = self.indices[link_table_index];
        std.debug.assert(index != no_string_index);
        return index;
    }

    pub fn testingResolutionCount(self: *const LinkStringMap) usize {
        if (comptime target_builtin.is_test) return self.testing_state.resolutions;
        return 0;
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

    pub fn written(self: *LimitedWriter) []u8 {
        return self.bytes.items;
    }

    pub fn toOwnedSlice(self: *LimitedWriter) ![]u8 {
        return self.bytes.toOwnedSlice(self.allocator);
    }
};

pub const FixedLimitedWriter = struct {
    bytes: []u8,
    length: usize = 0,
    limit: usize,

    pub fn init(bytes: []u8, limit: usize) FixedLimitedWriter {
        std.debug.assert(limit <= bytes.len);
        return .{ .bytes = bytes, .limit = limit };
    }

    pub fn appendSlice(self: *FixedLimitedWriter, value: []const u8) !void {
        if (value.len > self.limit -| self.length) return error.ProjectionTooLarge;
        @memcpy(self.bytes[self.length..][0..value.len], value);
        self.length += value.len;
    }

    pub fn appendZeros(self: *FixedLimitedWriter, count: usize) !void {
        if (count > self.limit -| self.length) return error.ProjectionTooLarge;
        @memset(self.bytes[self.length..][0..count], 0);
        self.length += count;
    }

    pub fn appendInt(self: *FixedLimitedWriter, comptime Integer: type, value: Integer) !void {
        var buffer: [@sizeOf(Integer)]u8 = undefined;
        if (@sizeOf(Integer) == 1) {
            buffer[0] = @intCast(value);
        } else {
            std.mem.writeInt(Integer, &buffer, value, .little);
        }
        try self.appendSlice(&buffer);
    }

    pub fn written(self: *FixedLimitedWriter) []u8 {
        return self.bytes[0..self.length];
    }
};
