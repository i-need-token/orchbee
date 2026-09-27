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
const row_flags_mask: u8 = 0x03;
const style_flags_mask: u16 = 0x00ff;
const interaction_mask: u32 = 0x0007_ffff;
const mouse_mode_mask: u32 = 0x0000_00f0;
const mouse_format_mask: u32 = 0x0000_0f00;
const color_rgb_mask: u32 = 0x00ff_ffff;
const color_default: u32 = 0x0100_0000;
const color_none: u32 = 0x0200_0000;

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
    theme_revision: u64,
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
    if (length > contract.Limits.string_bytes) return error.LimitExceeded;
    if (offset > bytes.len or length > bytes.len - offset) return error.InvalidField;
    if (!std.unicode.utf8ValidateSlice(bytes[offset..][0..length])) return error.InvalidUtf8;
}

fn validProjectionColor(value: u32) bool {
    return value <= color_rgb_mask or value == color_default or value == color_none;
}

fn isUtf8Boundary(bytes: []const u8, offset: usize) bool {
    if (offset == 0 or offset == bytes.len) return true;
    return bytes[offset] & 0xc0 != 0x80;
}

const StringRangeStatus = enum {
    valid,
    invalid_null,
    limit_exceeded,
    invalid_range,
    invalid_boundary,
};

fn stringRangeStatus(
    string_bytes: []const u8,
    offset: u32,
    length: u32,
    nullable: bool,
) StringRangeStatus {
    if (offset == max_u32) {
        return if (nullable and length == 0) .valid else .invalid_null;
    }
    if (length > contract.Limits.string_bytes) return .limit_exceeded;
    if (offset > string_bytes.len or length > string_bytes.len - offset) return .invalid_range;
    const end = @as(usize, offset) + length;
    if (!isUtf8Boundary(string_bytes, offset) or !isUtf8Boundary(string_bytes, end)) {
        return .invalid_boundary;
    }
    return .valid;
}

fn requireStringRange(status: StringRangeStatus) contract.ProtocolError!void {
    switch (status) {
        .valid => {},
        .invalid_null, .invalid_range => return error.InvalidField,
        .limit_exceeded => return error.LimitExceeded,
        .invalid_boundary => return error.InvalidUtf8,
    }
}

fn sectionSlice(bytes: []const u8, layout: Layout, section: usize) []const u8 {
    const offset = @as(usize, layout.offsets[section]);
    const length = @as(usize, layout.lengths[section]);
    return bytes[offset..][0..length];
}

const ParsedProjection = struct {
    attachment_id: [16]u8,
    base_revision: ?u64,
    columns: u16,
    generation: u64,
    host_epoch: [16]u8,
    kind: frame_codec.FrameKind,
    layout: Layout,
    revision: u64,
    row_count: u16,
    rows: u16,
    scrollback_rows: u64,
    sequence: u64,
    theme_revision: u64,
};

/// Validates the fixed header, canonical section layout and directory, alignment padding,
/// reserved fields, interaction flags, and the two nullable metadata strings. It deliberately
/// does not rescan per-cell values produced from canonical Ghostty state; that strict scan is
/// performed by `validateProjectionSections` after this private structural helper succeeds.
fn parseProjectionBuffer(bytes: []const u8) contract.ProtocolError!ParsedProjection {
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
    const theme_revision = try reader.readInt(u64);
    const reserved_metadata = try reader.readBytes(7);
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
    const grid_cell_count = std.math.mul(
        u32,
        @as(u32, columns),
        @as(u32, rows),
    ) catch return error.InvalidField;
    if (columns == 0 or columns > contract.Limits.frame_columns or
        rows == 0 or rows > contract.Limits.frame_rows or
        grid_cell_count > contract.Limits.frame_cells or
        row_count > rows or
        cell_count != @as(u32, row_count) * columns or
        cell_count > contract.Limits.frame_cells or sequence == 0 or generation == 0 or
        cursor_column >= columns or cursor_row >= rows or
        (cell_pixel_width == 0) != (cell_pixel_height == 0) or
        cell_pixel_width > contract.Limits.cell_pixel_dimension or
        cell_pixel_height > contract.Limits.cell_pixel_dimension or
        @as(u32, columns) * cell_pixel_width > std.math.maxInt(u16) or
        @as(u32, rows) * cell_pixel_height > std.math.maxInt(u16) or
        cursor_shape < 1 or cursor_shape > 3 or active_screen < 1 or active_screen > 2 or
        foreground > color_rgb_mask or background > color_rgb_mask or
        (cursor_color != color_default and cursor_color > color_rgb_mask))
    {
        return error.InvalidField;
    }
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
            if (byte != 0) return error.InvalidField;
        }
        prior_end = @as(usize, layout.offsets[section]) + layout.lengths[section];
    }
    if (interaction & ~interaction_mask != 0) return error.UnknownRequiredFlag;
    if (@popCount(interaction & mouse_mode_mask) > 1 or
        @popCount(interaction & mouse_format_mask) > 1)
    {
        return error.InvalidField;
    }
    const strings = sectionSlice(bytes, layout, 15);
    try validateMetadataString(strings, title_offset, title_length);
    try validateMetadataString(strings, directory_value_offset, directory_length);
    return .{
        .attachment_id = attachment_id,
        .base_revision = base_revision,
        .columns = columns,
        .generation = generation,
        .host_epoch = host_epoch,
        .kind = kind,
        .layout = layout,
        .revision = revision,
        .row_count = row_count,
        .rows = rows,
        .scrollback_rows = scrollback_rows,
        .sequence = sequence,
        .theme_revision = theme_revision,
    };
}

/// Performs the complete strict section scan matching `decodeTerminalProjectionBuffer`:
/// whole-string UTF-8, row indices/flags/prompt classes, adjacent wrap agreement, and per-cell
/// colors, style flags, underline kinds, widths, semantic/protected values, string ranges, and
/// wide-cell topology. This is the trusted-view boundary used for raw, Gateway, and external
/// records; the private structural helper above never admits payload cells.
fn validateProjectionSections(
    bytes: []const u8,
    parsed: ParsedProjection,
) contract.ProtocolError!void {
    const layout = parsed.layout;
    const strings = sectionSlice(bytes, layout, 15);
    if (!std.unicode.utf8ValidateSlice(strings)) return error.InvalidUtf8;

    const row_indices = sectionSlice(bytes, layout, 0);
    const row_flags = sectionSlice(bytes, layout, 1);
    const prompt_classes = sectionSlice(bytes, layout, 2);
    const grapheme_offsets = sectionSlice(bytes, layout, 3);
    const grapheme_lengths = sectionSlice(bytes, layout, 4);
    const hyperlink_offsets = sectionSlice(bytes, layout, 5);
    const hyperlink_lengths = sectionSlice(bytes, layout, 6);
    const foreground_colors = sectionSlice(bytes, layout, 7);
    const background_colors = sectionSlice(bytes, layout, 8);
    const underline_colors = sectionSlice(bytes, layout, 9);
    const style_flags = sectionSlice(bytes, layout, 10);
    const underline_kinds = sectionSlice(bytes, layout, 11);
    const widths = sectionSlice(bytes, layout, 12);
    const semantic_contents = sectionSlice(bytes, layout, 13);
    const protected_cells = sectionSlice(bytes, layout, 14);

    const columns = parsed.columns;
    var previous_row: ?u16 = null;
    var previous_row_flags: u8 = 0;
    var previous_spacer_head = false;
    for (0..@as(usize, parsed.row_count)) |row_offset| {
        const row = std.mem.readInt(u16, row_indices[2 * row_offset ..][0..2], .little);
        const row_flags_value = row_flags[row_offset];
        const prompt_class = prompt_classes[row_offset];
        if (row_flags_value & ~row_flags_mask != 0) return error.UnknownRequiredFlag;
        if (prompt_class > 2 or row >= parsed.rows or
            (previous_row != null and row <= previous_row.?) or
            (parsed.kind == .snapshot and row != @as(u16, @intCast(row_offset))))
        {
            return error.InvalidField;
        }
        const follows_prior_row = if (previous_row) |prior_row|
            row == prior_row + 1
        else
            false;
        if (follows_prior_row and
            ((previous_row_flags & 1 != 0) != (row_flags_value & 2 != 0)))
        {
            return error.InvalidField;
        }
        const first_cell = row_offset * @as(usize, columns);
        const end_cell = first_cell + @as(usize, columns);
        var previous_width: ?u8 = null;
        var first_width: ?u8 = null;
        var spacer_head = false;
        for (first_cell..end_cell) |cell_offset| {
            const style_flags_value = std.mem.readInt(
                u16,
                style_flags[2 * cell_offset ..][0..2],
                .little,
            );
            const width = widths[cell_offset];
            const semantic = semantic_contents[cell_offset];
            const protected_cell = protected_cells[cell_offset];
            const underline_kind = underline_kinds[cell_offset];
            const foreground = std.mem.readInt(
                u32,
                foreground_colors[4 * cell_offset ..][0..4],
                .little,
            );
            const background = std.mem.readInt(
                u32,
                background_colors[4 * cell_offset ..][0..4],
                .little,
            );
            const underline = std.mem.readInt(
                u32,
                underline_colors[4 * cell_offset ..][0..4],
                .little,
            );
            if (style_flags_value & ~style_flags_mask != 0) return error.UnknownRequiredFlag;
            if (width > 3 or semantic > 2 or protected_cell > 1 or underline_kind > 5 or
                !validProjectionColor(foreground) or
                !validProjectionColor(background) or
                !validProjectionColor(underline) or
                (width == 2 and previous_width != 1) or
                (previous_width == 1 and width != 2) or
                (width == 3 and (cell_offset != end_cell - 1 or row_flags_value & 1 == 0)))
            {
                return error.InvalidField;
            }
            const grapheme_offset = std.mem.readInt(
                u32,
                grapheme_offsets[4 * cell_offset ..][0..4],
                .little,
            );
            const grapheme_length = std.mem.readInt(
                u16,
                grapheme_lengths[2 * cell_offset ..][0..2],
                .little,
            );
            try requireStringRange(stringRangeStatus(strings, grapheme_offset, grapheme_length, false));
            const hyperlink_offset = std.mem.readInt(
                u32,
                hyperlink_offsets[4 * cell_offset ..][0..4],
                .little,
            );
            const hyperlink_length = std.mem.readInt(
                u16,
                hyperlink_lengths[2 * cell_offset ..][0..2],
                .little,
            );
            try requireStringRange(stringRangeStatus(strings, hyperlink_offset, hyperlink_length, true));
            if (cell_offset == first_cell) first_width = width;
            if (width == 3) spacer_head = true;
            previous_width = width;
        }
        if (previous_width == 1) return error.InvalidField;
        if (follows_prior_row and previous_spacer_head and first_width != 1) {
            return error.InvalidField;
        }
        previous_row = row;
        previous_row_flags = row_flags_value;
        previous_spacer_head = spacer_head;
    }
}

pub fn validateTerminalProjectionBuffer(bytes: []const u8) contract.ProtocolError!ProjectionBufferSummary {
    const parsed = try parseProjectionBuffer(bytes);
    try validateProjectionSections(bytes, parsed);
    return .{
        .attachment_id = parsed.attachment_id,
        .base_revision = parsed.base_revision,
        .columns = parsed.columns,
        .generation = parsed.generation,
        .host_epoch = parsed.host_epoch,
        .kind = parsed.kind,
        .revision = parsed.revision,
        .row_count = parsed.row_count,
        .rows = parsed.rows,
        .scrollback_rows = parsed.scrollback_rows,
        .sequence = parsed.sequence,
        .theme_revision = parsed.theme_revision,
    };
}
