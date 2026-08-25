const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const projection_buffer = @import("projection_buffer.zig");
const render_encoding = @import("render_encoding.zig");

pub const max_bytes = protocol.Limits.row_window_bytes;
pub const max_entries = protocol.Limits.frame_cells;
pub const header_bytes = protocol.terminal_row_window_header_bytes;

pub const RequestMetadata = struct {
    attachment_id: identifiers.AttachmentId,
    detail: protocol.RowWindowDetail,
    generation: u64,
    host_epoch: identifiers.HostEpoch,
    request_sequence: u64,
    revision: u64,
    theme_revision: u64,
};

pub const Parameters = struct {
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    hard_limit: usize,
    request: RequestMetadata,
    row_count: u16,
    start_pin: ghostty_vt.Pin,
    start_row: u64,
    total_rows: u64,
};

pub const Error = error{
    InvalidRequest,
    ProjectionTooLarge,
};

pub fn encode(
    allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    parameters: Parameters,
) (Error || std.mem.Allocator.Error)![]u8 {
    var writer = render_encoding.LimitedWriter.init(allocator, parameters.hard_limit);
    errdefer writer.deinit();
    try encodeToWriter(allocator, terminal, parameters, &writer);
    return writer.toOwnedSlice();
}

pub fn encodeInto(
    scratch_allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    parameters: Parameters,
    destination: []u8,
) (Error || std.mem.Allocator.Error)![]u8 {
    if (destination.len < parameters.hard_limit) return error.InvalidRequest;
    var writer = render_encoding.FixedLimitedWriter.init(destination, parameters.hard_limit);
    try encodeToWriter(scratch_allocator, terminal, parameters, &writer);
    return writer.written();
}

fn encodeToWriter(
    scratch_allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    parameters: Parameters,
    writer: anytype,
) (Error || std.mem.Allocator.Error)!void {
    if (parameters.hard_limit < header_bytes or parameters.hard_limit > max_bytes or
        parameters.row_count == 0)
    {
        return error.InvalidRequest;
    }
    const pins = try scratch_allocator.alloc(ghostty_vt.Pin, parameters.row_count);
    defer scratch_allocator.free(pins);
    var iterator = parameters.start_pin.rowIterator(.right_down, null);
    for (pins) |*captured_pin| {
        const pin = iterator.next() orelse return error.InvalidRequest;
        if (pin.cells(.all).len != terminal.cols) return error.InvalidRequest;
        captured_pin.* = pin;
    }

    try writer.appendZeros(header_bytes);
    const body_offset = writer.written().len;
    const entry_count = switch (parameters.request.detail) {
        .render => try projection_buffer.encodeRenderSnapshot(
            scratch_allocator,
            writer,
            terminal,
            pins,
            .{
                .attachment_id = parameters.request.attachment_id,
                .cell_pixel_height = parameters.cell_pixel_height,
                .cell_pixel_width = parameters.cell_pixel_width,
                .generation = parameters.request.generation,
                .host_epoch = parameters.request.host_epoch,
                .request_sequence = parameters.request.request_sequence,
                .revision = parameters.request.revision,
                .start_row = parameters.start_row,
                .theme_revision = parameters.request.theme_revision,
            },
        ),
        .text => try encodeTextBody(writer, pins),
    };
    const body_length = writer.written().len - body_offset;
    var header = render_encoding.FixedLimitedWriter.init(
        writer.written()[0..header_bytes],
        header_bytes,
    );
    try header.appendSlice(protocol.terminal_row_window_magic);
    try header.appendInt(u8, protocol.terminal_row_window_format_major);
    try header.appendInt(u8, protocol.terminal_row_window_format_minor);
    try header.appendInt(u8, @intFromEnum(parameters.request.detail));
    try header.appendInt(u8, 0);
    try header.appendInt(u32, @intCast(header_bytes + body_length));
    try header.appendInt(u16, header_bytes);
    try header.appendInt(u16, terminal.cols);
    try header.appendSlice(&parameters.request.host_epoch.bytes);
    try header.appendSlice(&parameters.request.attachment_id.bytes);
    try header.appendInt(u64, parameters.request.request_sequence);
    try header.appendInt(u64, parameters.request.revision);
    try header.appendInt(u64, parameters.request.generation);
    try header.appendInt(u64, parameters.total_rows);
    try header.appendInt(u64, parameters.start_row);
    try header.appendInt(u16, parameters.row_count);
    try header.appendInt(u16, 0);
    try header.appendInt(u32, entry_count);
    try header.appendInt(u32, @intCast(body_length));
    try header.appendInt(u8, protocol.terminal_row_window_format_patch);
    try header.appendZeros(11);
    std.debug.assert(header.written().len == header_bytes);
}

fn encodeTextBody(
    writer: anytype,
    pins: []const ghostty_vt.Pin,
) (Error || std.mem.Allocator.Error)!u32 {
    var entry_count: u32 = 0;
    for (pins) |pin| {
        const cells = pin.cells(.all);
        const logical_columns = textColumns(cells);
        const identity_ascii = isIdentityAsciiRow(pin, cells[0..logical_columns]);
        var text_bytes: u32 = 0;
        var mapping_count: u32 = 0;
        for (cells[0..logical_columns]) |*cell| {
            if (isSpacer(cell.*)) continue;
            const cell_bytes = try cellTextByteLength(pin, cell);
            text_bytes = std.math.add(u32, text_bytes, cell_bytes) catch
                return error.ProjectionTooLarge;
            mapping_count += 1;
        }
        const row = pin.rowAndCell().row;
        try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
            (if (row.wrap_continuation) @as(u8, 2) else 0) |
            (if (identity_ascii) @as(u8, 4) else 0));
        try writer.appendInt(u8, render_encoding.rowPromptClass(row.semantic_prompt));
        try writer.appendInt(u16, 0);
        try writer.appendInt(u32, text_bytes);
        try writer.appendInt(u32, if (identity_ascii) 0 else mapping_count);
        for (cells[0..logical_columns]) |*cell| {
            if (isSpacer(cell.*)) continue;
            try appendCellText(writer, pin, cell);
        }
        if (!identity_ascii) {
            var byte_offset: u32 = 0;
            var utf16_offset: u32 = 0;
            for (cells[0..logical_columns], 0..) |*cell, column| {
                if (isSpacer(cell.*)) continue;
                try writer.appendInt(u32, byte_offset);
                try writer.appendInt(u32, utf16_offset);
                try writer.appendInt(u16, @intCast(column));
                try writer.appendInt(u8, if (cell.wide == .wide) 2 else 1);
                try writer.appendInt(u8, 0);
                byte_offset = std.math.add(u32, byte_offset, try cellTextByteLength(pin, cell)) catch
                    return error.ProjectionTooLarge;
                utf16_offset = std.math.add(u32, utf16_offset, try cellTextUtf16Length(pin, cell)) catch
                    return error.ProjectionTooLarge;
            }
        }
        if (mapping_count > max_entries - entry_count) return error.ProjectionTooLarge;
        entry_count = std.math.add(u32, entry_count, mapping_count) catch
            return error.ProjectionTooLarge;
    }
    return entry_count;
}

fn textColumns(cells: []const ghostty_vt.page.Cell) usize {
    var index = cells.len;
    while (index > 0) {
        index -= 1;
        if (cells[index].hasText()) return index + 1;
    }
    return 0;
}

fn isSpacer(cell: ghostty_vt.page.Cell) bool {
    return cell.wide == .spacer_tail or cell.wide == .spacer_head;
}

fn isIdentityAsciiRow(pin: ghostty_vt.Pin, cells: []const ghostty_vt.page.Cell) bool {
    for (cells) |*cell| {
        if (cell.wide != .narrow) return false;
        if (!cell.hasText()) continue;
        if (cell.codepoint() > 0x7f) return false;
        if (pin.grapheme(cell)) |trailing| {
            if (trailing.len != 0) return false;
        }
    }
    return true;
}

fn cellTextByteLength(pin: ghostty_vt.Pin, cell: *const ghostty_vt.page.Cell) Error!u32 {
    if (!cell.hasText()) return 1;
    const max_length: u32 = @intCast(render_encoding.max_string_bytes);
    var length: u32 = @intCast(std.unicode.utf8CodepointSequenceLength(cell.codepoint()) catch
        return error.ProjectionTooLarge);
    if (length > max_length) return error.ProjectionTooLarge;
    if (pin.grapheme(cell)) |trailing| {
        for (trailing) |codepoint| {
            const codepoint_length = std.unicode.utf8CodepointSequenceLength(codepoint) catch
                return error.ProjectionTooLarge;
            length = std.math.add(u32, length, @intCast(codepoint_length)) catch
                return error.ProjectionTooLarge;
            if (length > max_length) return error.ProjectionTooLarge;
        }
    }
    return length;
}

fn cellTextUtf16Length(pin: ghostty_vt.Pin, cell: *const ghostty_vt.page.Cell) Error!u32 {
    if (!cell.hasText()) return 1;
    const max_length: u32 = @intCast(render_encoding.max_string_bytes * 2);
    var length: u32 = if (cell.codepoint() > 0xffff) 2 else 1;
    if (pin.grapheme(cell)) |trailing| {
        for (trailing) |codepoint| {
            length = std.math.add(u32, length, if (codepoint > 0xffff) 2 else 1) catch
                return error.ProjectionTooLarge;
            if (length > max_length) return error.ProjectionTooLarge;
        }
    }
    return length;
}

fn appendCellText(
    writer: anytype,
    pin: ghostty_vt.Pin,
    cell: *const ghostty_vt.page.Cell,
) !void {
    if (!cell.hasText()) {
        try writer.appendSlice(" ");
        return;
    }
    var buffer: [render_encoding.max_string_bytes]u8 = undefined;
    const grapheme = try render_encoding.cellGrapheme(
        cell.*,
        pin.grapheme(cell) orelse &.{},
        &buffer,
    );
    try writer.appendSlice(grapheme);
}
