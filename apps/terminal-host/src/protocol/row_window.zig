const std = @import("std");

const contract = @import("contract.zig");
const projection_buffer_codec = @import("projection_buffer.zig");
const wire = @import("wire.zig");

const ProtocolError = contract.ProtocolError;
const Reader = wire.Reader;
const copyIdentifier = wire.copyIdentifier;
const requireMagic = wire.requireMagic;
const max_row_window_bytes = contract.Limits.row_window_bytes;
const max_frame_cells = contract.Limits.frame_cells;
const max_frame_columns = contract.Limits.frame_columns;
const max_frame_rows = contract.Limits.frame_rows;
const max_string_bytes = contract.Limits.string_bytes;
const terminal_row_window_header_bytes = contract.terminal_row_window_header_bytes;
const terminal_row_window_magic = contract.terminal_row_window_magic;
const terminal_row_window_format_major = contract.terminal_row_window_format_major;
const terminal_row_window_format_minor = contract.terminal_row_window_format_minor;
const terminal_row_window_format_patch = contract.terminal_row_window_format_patch;
const validateTerminalProjectionBuffer = projection_buffer_codec.validateTerminalProjectionBuffer;

pub const RowWindowDetail = enum(u8) {
    render = 1,
    text = 2,
};

pub const RowWindowSummary = struct {
    attachment_id: [16]u8,
    columns: u16,
    detail: RowWindowDetail,
    entry_count: u32,
    generation: u64,
    host_epoch: [16]u8,
    revision: u64,
    row_count: u16,
    sequence: u64,
    start_row: u64,
    total_rows: u64,
};

pub fn validateTerminalRowWindow(
    allocator: std.mem.Allocator,
    bytes: []const u8,
) (ProtocolError || std.mem.Allocator.Error)!RowWindowSummary {
    _ = allocator;
    if (bytes.len < terminal_row_window_header_bytes) return error.Truncated;
    if (bytes.len > max_row_window_bytes) return error.LimitExceeded;
    try requireMagic(bytes, terminal_row_window_magic);

    var reader = Reader.init(bytes, 4);
    const format_major = try reader.readInt(u8);
    const format_minor = try reader.readInt(u8);
    const detail_code = try reader.readInt(u8);
    const flags = try reader.readInt(u8);
    const total_length = try reader.readInt(u32);
    const header_length = try reader.readInt(u16);
    const columns = try reader.readInt(u16);
    const host_epoch = try copyIdentifier(try reader.readBytes(16));
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const sequence = try reader.readInt(u64);
    const revision = try reader.readInt(u64);
    const generation = try reader.readInt(u64);
    const total_rows = try reader.readInt(u64);
    const start_row = try reader.readInt(u64);
    const row_count = try reader.readInt(u16);
    const reserved = try reader.readInt(u16);
    const entry_count = try reader.readInt(u32);
    const body_length = try reader.readInt(u32);
    const format_patch = try reader.readInt(u8);
    const reserved_tail = try reader.readBytes(11);

    if (format_major != terminal_row_window_format_major or
        format_minor != terminal_row_window_format_minor or
        format_patch != terminal_row_window_format_patch or
        header_length != terminal_row_window_header_bytes)
    {
        return error.UnsupportedVersion;
    }
    if (total_length != bytes.len or body_length != bytes.len - header_length) {
        return error.LengthMismatch;
    }
    if (reserved != 0 or flags & 0x80 != 0) return error.UnknownRequiredFlag;
    for (reserved_tail) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    if (columns == 0 or columns > max_frame_columns or row_count == 0 or
        row_count > max_frame_rows or entry_count > max_frame_cells or sequence == 0 or
        generation == 0 or start_row > total_rows or row_count > total_rows - start_row)
    {
        return error.InvalidField;
    }
    const detail: RowWindowDetail = switch (detail_code) {
        @intFromEnum(RowWindowDetail.render) => .render,
        @intFromEnum(RowWindowDetail.text) => .text,
        else => return error.InvalidField,
    };
    const body = bytes[header_length..];
    switch (detail) {
        .render => {
            const frame = try validateTerminalProjectionBuffer(body);
            if (frame.kind != .snapshot or
                !std.mem.eql(u8, &frame.host_epoch, &host_epoch) or
                !std.mem.eql(u8, &frame.attachment_id, &attachment_id) or
                frame.sequence != sequence or frame.revision != revision or
                frame.generation != generation or frame.columns != columns or
                frame.scrollback_rows != start_row or
                frame.rows != row_count or entry_count != @as(u32, row_count) * columns)
            {
                return error.InvalidField;
            }
        },
        .text => try validateTextRowWindowBody(body, row_count, columns, entry_count),
    }
    return .{
        .attachment_id = attachment_id,
        .columns = columns,
        .detail = detail,
        .entry_count = entry_count,
        .generation = generation,
        .host_epoch = host_epoch,
        .revision = revision,
        .row_count = row_count,
        .sequence = sequence,
        .start_row = start_row,
        .total_rows = total_rows,
    };
}

fn validateTextRowWindowBody(
    body: []const u8,
    row_count: u16,
    columns: u16,
    expected_entry_count: u32,
) ProtocolError!void {
    var reader = Reader.init(body, 0);
    var entries: u32 = 0;
    var previous_wrap: ?bool = null;
    for (0..row_count) |_| {
        const row_flags = try reader.readInt(u8);
        const prompt_class = try reader.readInt(u8);
        const reserved = try reader.readInt(u16);
        const text_length = try reader.readInt(u32);
        const mapping_count = try reader.readInt(u32);
        const supported_row_flags: u8 = 0x07;
        if (row_flags & ~supported_row_flags != 0 or reserved != 0) {
            return error.UnknownRequiredFlag;
        }
        if (prompt_class > 2) return error.InvalidField;
        if (mapping_count > columns or mapping_count > max_frame_cells - entries) {
            return error.LimitExceeded;
        }
        const wrap = row_flags & 1 != 0;
        const wrap_continuation = row_flags & 2 != 0;
        if (previous_wrap) |prior_wrap| {
            if (prior_wrap != wrap_continuation) return error.InvalidField;
        }
        previous_wrap = wrap;
        const text = try reader.readBytes(text_length);
        if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidUtf8;
        const identity_ascii = row_flags & 0x04 != 0;
        if (identity_ascii) {
            if (mapping_count != 0 or text.len > columns) return error.InvalidField;
            for (text) |byte| if (byte > 0x7f) return error.InvalidField;
            const logical_entries: u32 = @intCast(text.len);
            if (logical_entries > max_frame_cells - entries) return error.LimitExceeded;
            entries += logical_entries;
            continue;
        }
        const utf16_length = try utf16Length(text);
        if (text.len > 0 and mapping_count == 0) return error.InvalidField;
        var previous_byte: ?u32 = null;
        var previous_utf16: ?u32 = null;
        var previous_column_end: u32 = 0;
        var text_byte_position: usize = 0;
        var text_utf16_position: u32 = 0;
        for (0..mapping_count) |mapping_index| {
            const byte_offset = try reader.readInt(u32);
            const utf16_offset = try reader.readInt(u32);
            const column = try reader.readInt(u16);
            const width = try reader.readInt(u8);
            const mapping_reserved = try reader.readInt(u8);
            if (mapping_reserved != 0) return error.UnknownRequiredFlag;
            if (byte_offset >= text.len or utf16_offset >= utf16_length or
                @as(u32, column) + width > columns or
                (width != 1 and width != 2) or
                (mapping_index == 0 and (byte_offset != 0 or utf16_offset != 0)) or
                (previous_byte != null and byte_offset <= previous_byte.?) or
                (previous_utf16 != null and utf16_offset <= previous_utf16.?) or
                @as(u32, column) < previous_column_end)
            {
                return error.InvalidField;
            }
            while (text_byte_position < byte_offset) {
                const codepoint_bytes = std.unicode.utf8ByteSequenceLength(
                    text[text_byte_position],
                ) catch return error.InvalidUtf8;
                if (text_byte_position + codepoint_bytes > byte_offset) {
                    return error.InvalidField;
                }
                const codepoint = std.unicode.utf8Decode(
                    text[text_byte_position..][0..codepoint_bytes],
                ) catch return error.InvalidUtf8;
                text_byte_position += codepoint_bytes;
                text_utf16_position = std.math.add(
                    u32,
                    text_utf16_position,
                    if (codepoint > 0xffff) 2 else 1,
                ) catch return error.LimitExceeded;
            }
            if (text_byte_position != byte_offset or text_utf16_position != utf16_offset) {
                return error.InvalidField;
            }
            if (previous_byte != null and
                @as(usize, byte_offset - previous_byte.?) > max_string_bytes)
            {
                return error.LimitExceeded;
            }
            previous_byte = byte_offset;
            previous_utf16 = utf16_offset;
            previous_column_end = @as(u32, column) + width;
        }
        if (mapping_count > 0 and
            @as(usize, text.len) - @as(usize, previous_byte.?) > max_string_bytes)
        {
            return error.LimitExceeded;
        }
        entries += mapping_count;
    }
    if (reader.offset != body.len or entries != expected_entry_count) return error.LengthMismatch;
}

fn utf16Length(bytes: []const u8) ProtocolError!u32 {
    const view = std.unicode.Utf8View.init(bytes) catch return error.InvalidUtf8;
    var iterator = view.iterator();
    var result: u32 = 0;
    while (iterator.nextCodepoint()) |codepoint| {
        result = std.math.add(u32, result, if (codepoint > 0xffff) 2 else 1) catch
            return error.LimitExceeded;
    }
    return result;
}
