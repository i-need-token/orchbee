const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const render_encoding = @import("render_encoding.zig");

const directory_entry_bytes = 8;
const directory_offset = 160;
const no_string_offset = std.math.maxInt(u32);
const section_element_bytes = [_]u8{ 2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1 };

pub const Parameters = struct {
    attachment_id: identifiers.AttachmentId,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    generation: u64,
    host_epoch: identifiers.HostEpoch,
    request_sequence: u64,
    revision: u64,
    start_row: u64,
};

const Layout = struct {
    lengths: [section_element_bytes.len]usize,
    offsets: [section_element_bytes.len]usize,
    string_offset: usize,
};

const StringRange = struct {
    length: u32,
    offset: u32,
};

fn preflightGeometry(
    columns: usize,
    rows: usize,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) error{ProjectionTooLarge}!usize {
    if (columns == 0 or columns > protocol.Limits.frame_columns or
        rows == 0 or rows > protocol.Limits.frame_rows or
        cell_pixel_width == 0 or cell_pixel_width > protocol.Limits.cell_pixel_dimension or
        cell_pixel_height == 0 or cell_pixel_height > protocol.Limits.cell_pixel_dimension)
    {
        return error.ProjectionTooLarge;
    }
    const cell_count = std.math.mul(usize, rows, columns) catch
        return error.ProjectionTooLarge;
    if (cell_count > protocol.Limits.frame_cells) return error.ProjectionTooLarge;
    const pixel_width = std.math.mul(usize, columns, cell_pixel_width) catch
        return error.ProjectionTooLarge;
    const pixel_height = std.math.mul(usize, rows, cell_pixel_height) catch
        return error.ProjectionTooLarge;
    if (pixel_width > std.math.maxInt(u16) or pixel_height > std.math.maxInt(u16)) {
        return error.ProjectionTooLarge;
    }
    return cell_count;
}

fn alignForward(value: usize, alignment: usize) error{ProjectionTooLarge}!usize {
    const remainder = value % alignment;
    if (remainder == 0) return value;
    return std.math.add(usize, value, alignment - remainder) catch
        error.ProjectionTooLarge;
}

fn calculateLayout(row_count: usize, cell_count: usize) error{ProjectionTooLarge}!Layout {
    var result: Layout = .{
        .lengths = undefined,
        .offsets = undefined,
        .string_offset = protocol.terminal_projection_buffer_header_bytes,
    };
    var next_offset: usize = protocol.terminal_projection_buffer_header_bytes;
    for (section_element_bytes[0..15], 0..) |element_bytes, section| {
        const count = if (section <= 2) row_count else cell_count;
        const offset = try alignForward(next_offset, element_bytes);
        const length = std.math.mul(usize, count, element_bytes) catch
            return error.ProjectionTooLarge;
        result.offsets[section] = offset;
        result.lengths[section] = length;
        next_offset = std.math.add(usize, offset, length) catch
            return error.ProjectionTooLarge;
    }
    result.offsets[15] = next_offset;
    result.lengths[15] = 0;
    result.string_offset = next_offset;
    return result;
}

fn writeInt(
    bytes: []u8,
    comptime Integer: type,
    offset: usize,
    value: Integer,
) void {
    if (@sizeOf(Integer) == 1) {
        bytes[offset] = @intCast(value);
    } else {
        std.mem.writeInt(Integer, bytes[offset..][0..@sizeOf(Integer)], value, .little);
    }
}

fn appendString(
    writer: anytype,
    frame_offset: usize,
    string_offset: usize,
    value: []const u8,
) !StringRange {
    if (value.len > render_encoding.max_string_bytes) return error.ProjectionTooLarge;
    const relative_offset = writer.written().len - frame_offset - string_offset;
    const offset = std.math.cast(u32, relative_offset) orelse return error.ProjectionTooLarge;
    const length = std.math.cast(u32, value.len) orelse return error.ProjectionTooLarge;
    try writer.appendSlice(value);
    return .{ .length = length, .offset = offset };
}

fn appendOptionalString(
    writer: anytype,
    frame_offset: usize,
    string_offset: usize,
    value: ?[]const u8,
) !StringRange {
    const bytes = value orelse return .{ .length = 0, .offset = no_string_offset };
    if (!std.unicode.utf8ValidateSlice(bytes) or bytes.len > render_encoding.max_string_bytes) {
        return .{ .length = 0, .offset = no_string_offset };
    }
    return appendString(writer, frame_offset, string_offset, bytes);
}

pub fn encodeRenderSnapshot(
    writer: anytype,
    terminal: *ghostty_vt.Terminal,
    pins: []const ghostty_vt.Pin,
    parameters: Parameters,
) !u32 {
    const cell_count = try preflightGeometry(
        terminal.cols,
        pins.len,
        parameters.cell_pixel_width,
        parameters.cell_pixel_height,
    );
    const layout = try calculateLayout(pins.len, cell_count);
    const frame_offset = writer.written().len;
    try writer.appendZeros(layout.string_offset);

    const title = try appendOptionalString(
        writer,
        frame_offset,
        layout.string_offset,
        terminal.getTitle(),
    );
    const current_directory = try appendOptionalString(
        writer,
        frame_offset,
        layout.string_offset,
        terminal.getPwd(),
    );
    const colors = renderColors(terminal);
    for (pins, 0..) |pin, row_index| {
        var frame = writer.written()[frame_offset..];
        const row = pin.rowAndCell().row;
        writeInt(frame, u16, layout.offsets[0] + row_index * 2, @intCast(row_index));
        frame[layout.offsets[1] + row_index] =
            (if (row.wrap) @as(u8, 1) else 0) |
            (if (row.wrap_continuation) @as(u8, 2) else 0);
        frame[layout.offsets[2] + row_index] = render_encoding.rowPromptClass(row.semantic_prompt);
        for (pin.cells(.all), 0..) |*cell, column| {
            const cell_index = row_index * terminal.cols + column;
            var grapheme_buffer: [render_encoding.max_string_bytes]u8 = undefined;
            const grapheme = render_encoding.cellGrapheme(
                cell.*,
                pin.grapheme(cell) orelse &.{},
                &grapheme_buffer,
            ) catch return error.ProjectionTooLarge;
            const grapheme_range = try appendString(
                writer,
                frame_offset,
                layout.string_offset,
                grapheme,
            );
            const hyperlink_range = try appendOptionalString(
                writer,
                frame_offset,
                layout.string_offset,
                hyperlinkTarget(pin, cell.*, column),
            );
            frame = writer.written()[frame_offset..];
            const style = render_encoding.styleRecord(
                cell.*,
                pin.style(cell),
                &colors,
                render_encoding.no_string_index,
            );
            writeInt(frame, u32, layout.offsets[3] + cell_index * 4, grapheme_range.offset);
            writeInt(frame, u16, layout.offsets[4] + cell_index * 2, @intCast(grapheme_range.length));
            writeInt(frame, u32, layout.offsets[5] + cell_index * 4, hyperlink_range.offset);
            writeInt(frame, u16, layout.offsets[6] + cell_index * 2, @intCast(hyperlink_range.length));
            writeInt(frame, u32, layout.offsets[7] + cell_index * 4, style.foreground);
            writeInt(frame, u32, layout.offsets[8] + cell_index * 4, style.background);
            writeInt(frame, u32, layout.offsets[9] + cell_index * 4, style.underline);
            writeInt(frame, u16, layout.offsets[10] + cell_index * 2, style.flags);
            frame[layout.offsets[11] + cell_index] = style.underline_kind;
            frame[layout.offsets[12] + cell_index] = render_encoding.cellWidth(cell.wide);
            frame[layout.offsets[13] + cell_index] = render_encoding.semanticContent(cell.semantic_content);
            frame[layout.offsets[14] + cell_index] = if (cell.protected) 1 else 0;
        }
    }

    var frame = writer.written()[frame_offset..];
    const frame_length = frame.len;
    const string_length = frame_length - layout.string_offset;
    if (frame_length > std.math.maxInt(u32) or string_length > std.math.maxInt(u32)) {
        return error.ProjectionTooLarge;
    }
    @memcpy(frame[0..4], protocol.terminal_projection_buffer_magic);
    frame[4] = protocol.terminal_projection_buffer_format_major;
    frame[5] = protocol.terminal_projection_buffer_format_minor;
    frame[6] = @intFromEnum(protocol.FrameKind.snapshot);
    writeInt(frame, u32, 8, @intCast(frame_length));
    writeInt(frame, u16, 12, protocol.terminal_projection_buffer_header_bytes);
    writeInt(frame, u16, 14, terminal.cols);
    writeInt(frame, u16, 16, @intCast(pins.len));
    writeInt(frame, u16, 18, @intCast(pins.len));
    writeInt(frame, u32, 20, @intCast(cell_count));
    @memcpy(frame[24..40], &parameters.host_epoch.bytes);
    @memcpy(frame[40..56], &parameters.attachment_id.bytes);
    writeInt(frame, u64, 56, parameters.request_sequence);
    writeInt(frame, u64, 64, render_encoding.no_base_revision);
    writeInt(frame, u64, 72, parameters.revision);
    writeInt(frame, u64, 80, parameters.generation);
    writeInt(frame, u64, 88, parameters.start_row);
    writeInt(frame, u16, 100, parameters.cell_pixel_width);
    writeInt(frame, u16, 102, parameters.cell_pixel_height);
    frame[104] = 1;
    frame[106] = if (terminal.screens.active_key == .primary) 1 else 2;
    writeInt(frame, u32, 108, render_encoding.terminalInteraction(terminal));
    writeInt(frame, u32, 112, render_encoding.rgbCode(colors.foreground));
    writeInt(frame, u32, 116, render_encoding.rgbCode(colors.background));
    writeInt(frame, u32, 120, if (colors.cursor) |value|
        render_encoding.rgbCode(value)
    else
        render_encoding.color_default);
    writeInt(frame, u32, 124, title.offset);
    writeInt(frame, u32, 128, title.length);
    writeInt(frame, u32, 132, current_directory.offset);
    writeInt(frame, u32, 136, current_directory.length);
    writeInt(frame, u32, 140, @intCast(string_length));
    frame[144] = protocol.terminal_projection_buffer_format_patch;
    for (0..section_element_bytes.len) |section| {
        const entry_offset = directory_offset + section * directory_entry_bytes;
        const length = if (section == 15) string_length else layout.lengths[section];
        writeInt(frame, u32, entry_offset, @intCast(layout.offsets[section]));
        writeInt(frame, u32, entry_offset + 4, @intCast(length));
    }
    return @intCast(cell_count);
}

fn renderColors(terminal: *const ghostty_vt.Terminal) ghostty_vt.RenderState.Colors {
    const background = terminal.colors.background.get() orelse ghostty_vt.color.RGB{
        .r = 0,
        .g = 0,
        .b = 0,
    };
    const foreground = terminal.colors.foreground.get() orelse ghostty_vt.color.RGB{
        .r = 0xff,
        .g = 0xff,
        .b = 0xff,
    };
    return .{
        .background = if (terminal.modes.get(.reverse_colors)) foreground else background,
        .foreground = if (terminal.modes.get(.reverse_colors)) background else foreground,
        .cursor = terminal.colors.cursor.get(),
        .palette = terminal.colors.palette.current,
    };
}

fn hyperlinkTarget(pin: ghostty_vt.Pin, cell: ghostty_vt.page.Cell, column: usize) ?[]const u8 {
    if (!cell.hyperlink) return null;
    const page = &pin.node.data;
    const page_cell = page.getRowAndCell(column, pin.y).cell;
    const identifier = page.lookupHyperlink(page_cell) orelse return null;
    const entry = page.hyperlink_set.get(page.memory, identifier);
    return entry.uri.slice(page.memory);
}
