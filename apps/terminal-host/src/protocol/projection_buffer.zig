const std = @import("std");

const contract = @import("contract.zig");
const frame_codec = @import("frame.zig");
const wire = @import("wire.zig");

const directory_entry_bytes = 8;
const directory_entry_count = 16;
const directory_offset = 160;
const max_u32 = std.math.maxInt(u32);
const no_base_revision = std.math.maxInt(u64);
const section_element_bytes = [_]u8{ 2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1 };

pub const ProjectionBufferSummary = struct {
    attachment_id: [16]u8,
    base_revision: ?u64,
    columns: u16,
    generation: u64,
    host_epoch: [16]u8,
    kind: frame_codec.FrameKind,
    revision: u64,
    row_count: u16,
    rows: u16,
    scrollback_rows: u64,
    sequence: u64,
};

const Layout = struct {
    lengths: [directory_entry_count]u32,
    offsets: [directory_entry_count]u32,
    total_length: usize,
};

fn alignForward(value: usize, alignment: usize) contract.ProtocolError!usize {
    const remainder = value % alignment;
    if (remainder == 0) return value;
    return std.math.add(usize, value, alignment - remainder) catch error.LimitExceeded;
}

fn calculateLayout(row_count: u16, cell_count: u32, string_bytes: u32) contract.ProtocolError!Layout {
    var result: Layout = .{
        .lengths = undefined,
        .offsets = undefined,
        .total_length = contract.terminal_projection_buffer_header_bytes,
    };
    for (section_element_bytes, 0..) |element_bytes, section| {
        const count: usize = if (section <= 2)
            row_count
        else if (section == 15)
            string_bytes
        else
            cell_count;
        const offset = try alignForward(result.total_length, element_bytes);
        const length = std.math.mul(usize, count, element_bytes) catch return error.LimitExceeded;
        result.offsets[section] = std.math.cast(u32, offset) orelse return error.LimitExceeded;
        result.lengths[section] = std.math.cast(u32, length) orelse return error.LimitExceeded;
        result.total_length = std.math.add(usize, offset, length) catch return error.LimitExceeded;
    }
    if (result.total_length > contract.Limits.frame_bytes) return error.LimitExceeded;
    return result;
}

fn validateMetadataString(bytes: []const u8, offset: u32, length: u32) contract.ProtocolError!void {
    if (offset == max_u32) {
        if (length != 0) return error.InvalidField;
        return;
    }
    if (offset > bytes.len or length > bytes.len - offset) return error.InvalidField;
    if (!std.unicode.utf8ValidateSlice(bytes[offset..][0..length])) return error.InvalidUtf8;
}

pub fn validateTerminalProjectionBuffer(bytes: []const u8) contract.ProtocolError!ProjectionBufferSummary {
    if (bytes.len < contract.terminal_projection_buffer_header_bytes) return error.Truncated;
    if (bytes.len > contract.Limits.frame_bytes) return error.LimitExceeded;
    try wire.requireMagic(bytes, contract.terminal_projection_buffer_magic);

    var reader = wire.Reader.init(bytes, 4);
    const format_major = try reader.readInt(u8);
    const format_minor = try reader.readInt(u8);
    const kind_code = try reader.readInt(u8);
    const flags = try reader.readInt(u8);
    const total_length = try reader.readInt(u32);
    const header_length = try reader.readInt(u16);
    const columns = try reader.readInt(u16);
    const rows = try reader.readInt(u16);
    const row_count = try reader.readInt(u16);
    const cell_count = try reader.readInt(u32);
    const host_epoch = try wire.copyIdentifier(try reader.readBytes(16));
    const attachment_id = try wire.copyIdentifier(try reader.readBytes(16));
    const sequence = try reader.readInt(u64);
    const encoded_base_revision = try reader.readInt(u64);
    const revision = try reader.readInt(u64);
    const generation = try reader.readInt(u64);
    const scrollback_rows = try reader.readInt(u64);
    const cursor_column = try reader.readInt(u16);
    const cursor_row = try reader.readInt(u16);
    const cell_pixel_width = try reader.readInt(u16);
    const cell_pixel_height = try reader.readInt(u16);
    const cursor_shape = try reader.readInt(u8);
    const cursor_flags = try reader.readInt(u8);
    const active_screen = try reader.readInt(u8);
    const reserved_screen = try reader.readInt(u8);
    const interaction = try reader.readInt(u32);
    const foreground = try reader.readInt(u32);
    const background = try reader.readInt(u32);
    const cursor_color = try reader.readInt(u32);
    const title_offset = try reader.readInt(u32);
    const title_length = try reader.readInt(u32);
    const directory_value_offset = try reader.readInt(u32);
    const directory_length = try reader.readInt(u32);
    const string_byte_length = try reader.readInt(u32);
    const format_patch = try reader.readInt(u8);
    const reserved_metadata = try reader.readBytes(15);
    var encoded_offsets: [directory_entry_count]u32 = undefined;
    var encoded_lengths: [directory_entry_count]u32 = undefined;
    for (0..directory_entry_count) |section| {
        encoded_offsets[section] = try reader.readInt(u32);
        encoded_lengths[section] = try reader.readInt(u32);
    }
    const reserved_directory = try reader.readBytes(32);
    std.debug.assert(reader.offset == contract.terminal_projection_buffer_header_bytes);

    if (format_major != contract.terminal_projection_buffer_format_major or
        format_minor != contract.terminal_projection_buffer_format_minor or
        format_patch != contract.terminal_projection_buffer_format_patch or
        header_length != contract.terminal_projection_buffer_header_bytes)
    {
        return error.UnsupportedVersion;
    }
    if (total_length != bytes.len) return error.LengthMismatch;
    if (flags & 0x80 != 0 or reserved_screen != 0 or cursor_flags & ~@as(u8, 0x07) != 0) {
        return error.UnknownRequiredFlag;
    }
    for (reserved_metadata) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    for (reserved_directory) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    if (columns == 0 or columns > contract.Limits.frame_columns or
        rows == 0 or rows > contract.Limits.frame_rows or
        row_count == 0 or row_count > rows or
        cell_count != @as(u32, row_count) * columns or
        cell_count > contract.Limits.frame_cells or sequence == 0 or generation == 0 or
        cursor_column >= columns or cursor_row >= rows or
        cell_pixel_width == 0 or cell_pixel_width > contract.Limits.cell_pixel_dimension or
        cell_pixel_height == 0 or cell_pixel_height > contract.Limits.cell_pixel_dimension or
        @as(u32, columns) * cell_pixel_width > std.math.maxInt(u16) or
        @as(u32, rows) * cell_pixel_height > std.math.maxInt(u16) or
        cursor_shape < 1 or cursor_shape > 3 or active_screen < 1 or active_screen > 2 or
        foreground > 0x00ff_ffff or background > 0x00ff_ffff or
        (cursor_color != 0x0100_0000 and cursor_color > 0x00ff_ffff) or
        interaction & ~@as(u32, 0x0003_ffff) != 0)
    {
        return error.InvalidField;
    }
    const mouse_modes = @popCount(interaction & 0x0000_00f0);
    const mouse_formats = @popCount(interaction & 0x0000_0f00);
    if (mouse_modes > 1 or mouse_formats > 1) return error.InvalidField;
    const kind: frame_codec.FrameKind = switch (kind_code) {
        1 => .snapshot,
        2 => .delta,
        else => return error.InvalidField,
    };
    const base_revision: ?u64 = if (encoded_base_revision == no_base_revision)
        null
    else
        encoded_base_revision;
    if ((kind == .snapshot and (base_revision != null or row_count != rows)) or
        (kind == .delta and (base_revision == null or base_revision.? >= revision)))
    {
        return error.InvalidField;
    }

    const layout = try calculateLayout(row_count, cell_count, string_byte_length);
    if (layout.total_length != bytes.len) return error.LengthMismatch;
    var prior_end: usize = contract.terminal_projection_buffer_header_bytes;
    for (0..directory_entry_count) |section| {
        if (encoded_offsets[section] != layout.offsets[section] or
            encoded_lengths[section] != layout.lengths[section])
        {
            return error.InvalidField;
        }
        for (bytes[prior_end..layout.offsets[section]]) |byte| {
            if (byte != 0) return error.UnknownRequiredFlag;
        }
        prior_end = @as(usize, layout.offsets[section]) + layout.lengths[section];
    }
    const strings_offset = layout.offsets[15];
    const strings = bytes[strings_offset..][0..string_byte_length];
    try validateMetadataString(strings, title_offset, title_length);
    try validateMetadataString(strings, directory_value_offset, directory_length);
    return .{
        .attachment_id = attachment_id,
        .base_revision = base_revision,
        .columns = columns,
        .generation = generation,
        .host_epoch = host_epoch,
        .kind = kind,
        .revision = revision,
        .row_count = row_count,
        .rows = rows,
        .scrollback_rows = scrollback_rows,
        .sequence = sequence,
    };
}
