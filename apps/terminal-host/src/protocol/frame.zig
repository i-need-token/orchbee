const std = @import("std");

const contract = @import("contract.zig");
const graphics_extension = @import("graphics_extension.zig");
const wire = @import("wire.zig");

const ProtocolError = contract.ProtocolError;
const Reader = wire.Reader;
const copyIdentifier = wire.copyIdentifier;
const requireMagic = wire.requireMagic;
const max_frame_bytes = contract.Limits.frame_bytes;
const max_frame_cells = contract.Limits.frame_cells;
const max_frame_columns = contract.Limits.frame_columns;
const max_frame_extensions = contract.Limits.frame_extensions;
const max_frame_rows = contract.Limits.frame_rows;
const max_frame_strings = contract.Limits.frame_strings;
const max_frame_styles = contract.Limits.frame_styles;
const max_cell_pixel_dimension = contract.Limits.cell_pixel_dimension;
const max_extension_bytes = contract.Limits.extension_bytes;
const max_string_bytes = contract.Limits.string_bytes;
const no_string_index = std.math.maxInt(u32);
const no_base_revision = std.math.maxInt(u64);
const terminal_frame_header_bytes = contract.terminal_frame_header_bytes;
const terminal_frame_magic = contract.terminal_frame_magic;
const terminal_frame_format_major = contract.terminal_frame_format_major;
const terminal_frame_format_minor = contract.terminal_frame_format_minor;
const terminal_frame_format_patch = contract.terminal_frame_format_patch;

pub const FrameKind = enum(u8) {
    snapshot = 1,
    delta = 2,
};

pub const CursorShape = enum {
    block,
    bar,
    underline,
};

pub const ActiveScreen = enum {
    primary,
    alternate,
};

pub const FrameSummary = struct {
    attachment_id: [16]u8,
    base_revision: ?u64,
    cell_count: usize,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    close_activity: ?contract.TerminalCloseActivity,
    columns: u16,
    command_completion: ?contract.TerminalCommandCompletion,
    command_observation: ?contract.TerminalCommandObservation,
    current_directory: ?[]const u8,
    cursor_column: u16,
    cursor_row: u16,
    cursor_shape: CursorShape,
    cursor_wide_tail: bool,
    extension_count: u16,
    generation: u64,
    graphics: ?graphics_extension.Summary,
    host_epoch: [16]u8,
    kind: FrameKind,
    progress: contract.TerminalProgress,
    revision: u64,
    row_count: u16,
    rows: u16,
    scrollback_rows: u64,
    sequence: u64,
    string_count: u32,
    style_count: u16,
    theme_revision: u64,
    title: ?[]const u8,
    active_screen: ActiveScreen,
};

fn stringAt(strings: []const []const u8, index: u32) ProtocolError!?[]const u8 {
    if (index == no_string_index) return null;
    if (index >= strings.len) return error.InvalidField;
    return strings[index];
}

fn validColor(value: u32) bool {
    return value <= 0x00ffffff or value == 0x01000000 or value == 0x02000000;
}

pub fn validateTerminalFrame(
    allocator: std.mem.Allocator,
    bytes: []const u8,
) (ProtocolError || std.mem.Allocator.Error)!FrameSummary {
    if (bytes.len < terminal_frame_header_bytes) return error.Truncated;
    if (bytes.len > max_frame_bytes) return error.LimitExceeded;
    try requireMagic(bytes, terminal_frame_magic);

    var reader = Reader.init(bytes, 4);
    const format_major = try reader.readInt(u8);
    const format_minor = try reader.readInt(u8);
    const kind_code = try reader.readInt(u8);
    const flags = try reader.readInt(u8);
    const total_length = try reader.readInt(u32);
    const header_length = try reader.readInt(u16);
    const reserved_header = try reader.readInt(u16);
    const host_epoch = try copyIdentifier(try reader.readBytes(16));
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const sequence = try reader.readInt(u64);
    const encoded_base_revision = try reader.readInt(u64);
    const revision = try reader.readInt(u64);
    const generation = try reader.readInt(u64);
    const scrollback_rows = try reader.readInt(u64);
    const columns = try reader.readInt(u16);
    const rows = try reader.readInt(u16);
    const cursor_column = try reader.readInt(u16);
    const cursor_row = try reader.readInt(u16);
    const cursor_shape_code = try reader.readInt(u8);
    const cursor_flags = try reader.readInt(u8);
    const active_screen_code = try reader.readInt(u8);
    const reserved_screen = try reader.readInt(u8);
    const interaction_flags = try reader.readInt(u32);
    const default_foreground = try reader.readInt(u32);
    const default_background = try reader.readInt(u32);
    const default_cursor = try reader.readInt(u32);
    const title_index = try reader.readInt(u32);
    const current_directory_index = try reader.readInt(u32);
    const string_count = try reader.readInt(u32);
    const style_count = try reader.readInt(u16);
    const row_count = try reader.readInt(u16);
    const extension_count = try reader.readInt(u16);
    const reserved_tables = try reader.readInt(u16);
    const cell_pixel_width = try reader.readInt(u16);
    const cell_pixel_height = try reader.readInt(u16);
    const declared_cell_count = try reader.readInt(u32);
    const format_patch = try reader.readInt(u8);
    const theme_revision = try reader.readInt(u64);
    const reserved_tail = try reader.readBytes(7);

    if (format_major != terminal_frame_format_major or
        format_minor != terminal_frame_format_minor or
        format_patch != terminal_frame_format_patch or
        header_length != terminal_frame_header_bytes)
    {
        return error.UnsupportedVersion;
    }
    if (total_length != bytes.len) return error.LengthMismatch;
    if (reserved_header != 0 or reserved_screen != 0 or reserved_tables != 0 or
        cursor_flags & ~@as(u8, 0x07) != 0 or flags & 0x80 != 0)
    {
        return error.UnknownRequiredFlag;
    }
    for (reserved_tail) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    if (columns == 0 or columns > max_frame_columns or rows == 0 or rows > max_frame_rows or
        @as(u32, columns) * rows > max_frame_cells or
        cursor_column >= columns or cursor_row >= rows or sequence == 0 or generation == 0 or
        (cell_pixel_width == 0) != (cell_pixel_height == 0) or
        cell_pixel_width > max_cell_pixel_dimension or
        cell_pixel_height > max_cell_pixel_dimension or
        @as(u32, columns) * cell_pixel_width > std.math.maxInt(u16) or
        @as(u32, rows) * cell_pixel_height > std.math.maxInt(u16))
    {
        return error.InvalidField;
    }
    if (string_count > max_frame_strings or style_count > max_frame_styles or
        row_count > max_frame_rows or declared_cell_count > max_frame_cells or
        extension_count > max_frame_extensions)
    {
        return error.LimitExceeded;
    }
    if (default_foreground > 0x00ffffff or default_background > 0x00ffffff or
        (default_cursor != 0x01000000 and default_cursor > 0x00ffffff))
    {
        return error.InvalidField;
    }
    if (interaction_flags & ~@as(u32, 0x0007ffff) != 0) return error.UnknownRequiredFlag;
    if (@popCount(interaction_flags & 0x000000f0) > 1 or
        @popCount(interaction_flags & 0x00000f00) > 1)
    {
        return error.InvalidField;
    }

    const kind: FrameKind = switch (kind_code) {
        @intFromEnum(FrameKind.snapshot) => .snapshot,
        @intFromEnum(FrameKind.delta) => .delta,
        else => return error.InvalidField,
    };
    const base_revision: ?u64 = if (encoded_base_revision == no_base_revision)
        null
    else
        encoded_base_revision;
    if ((kind == .snapshot and base_revision != null) or
        (kind == .delta and (base_revision == null or base_revision.? >= revision)) or
        (kind == .snapshot and
            (row_count != rows or declared_cell_count != @as(u32, rows) * columns)) or
        (kind == .delta and declared_cell_count != @as(u32, row_count) * columns))
    {
        return error.InvalidField;
    }
    const cursor_shape: CursorShape = switch (cursor_shape_code) {
        1 => .block,
        2 => .bar,
        3 => .underline,
        else => return error.InvalidField,
    };
    const active_screen: ActiveScreen = switch (active_screen_code) {
        1 => .primary,
        2 => .alternate,
        else => return error.InvalidField,
    };

    const strings = try allocator.alloc([]const u8, string_count);
    defer allocator.free(strings);
    for (strings) |*string| {
        const length = try reader.readInt(u32);
        if (length > max_string_bytes) return error.LimitExceeded;
        string.* = try reader.readBytes(length);
        if (!std.unicode.utf8ValidateSlice(string.*)) return error.InvalidUtf8;
    }

    for (0..style_count) |_| {
        const foreground = try reader.readInt(u32);
        const background = try reader.readInt(u32);
        const underline = try reader.readInt(u32);
        const hyperlink_index = try reader.readInt(u32);
        const style_flags = try reader.readInt(u16);
        const underline_kind = try reader.readInt(u8);
        const reserved = try reader.readInt(u8);
        if (reserved != 0 or style_flags & ~@as(u16, 0x00ff) != 0) {
            return error.UnknownRequiredFlag;
        }
        if (!validColor(foreground) or !validColor(background) or !validColor(underline) or
            underline_kind > 5)
        {
            return error.InvalidField;
        }
        if (hyperlink_index != no_string_index and hyperlink_index >= string_count) {
            return error.InvalidField;
        }
    }

    var cell_count: usize = 0;
    var previous_row: ?u16 = null;
    var previous_row_flags: u8 = 0;
    var previous_spacer_head = false;
    for (0..row_count) |row_offset| {
        const row = try reader.readInt(u16);
        const row_flags = try reader.readInt(u8);
        const prompt_class = try reader.readInt(u8);
        const count = try reader.readInt(u16);
        const reserved = try reader.readInt(u16);
        if (reserved != 0 or row_flags & ~@as(u8, 0x03) != 0) {
            return error.UnknownRequiredFlag;
        }
        if (prompt_class > 2 or count != columns or row >= rows) {
            return error.InvalidField;
        }
        const follows_previous_row = if (previous_row) |prior_row| row == prior_row + 1 else false;
        if (previous_row) |prior_row| {
            if (row <= prior_row) return error.InvalidField;
            if (follows_previous_row and
                ((previous_row_flags & 1 != 0) != (row_flags & 2 != 0)))
            {
                return error.InvalidField;
            }
        }
        if (kind == .snapshot and row != row_offset) return error.InvalidField;
        cell_count += count;
        if (cell_count > max_frame_cells) return error.LimitExceeded;
        var previous_width: ?u8 = null;
        var first_width: ?u8 = null;
        var spacer_head = false;
        for (0..count) |column| {
            const grapheme_index = try reader.readInt(u32);
            const style_index = try reader.readInt(u16);
            const cell_flags = try reader.readInt(u8);
            const cell_reserved = try reader.readInt(u8);
            if (cell_reserved != 0 or cell_flags & ~@as(u8, 0x1f) != 0) {
                return error.UnknownRequiredFlag;
            }
            const width = cell_flags & 0x03;
            const semantic = (cell_flags >> 2) & 0x03;
            if (grapheme_index >= string_count or style_index >= style_count or semantic > 2) {
                return error.InvalidField;
            }
            if ((width == 2 and previous_width != 1) or
                (previous_width == 1 and width != 2) or
                (width == 3 and (column != count - 1 or row_flags & 1 == 0)))
            {
                return error.InvalidField;
            }
            if (column == 0) first_width = width;
            if (width == 3) spacer_head = true;
            previous_width = width;
        }
        if (previous_width == 1) return error.InvalidField;
        if (follows_previous_row and previous_spacer_head and first_width != 1) {
            return error.InvalidField;
        }
        previous_row = row;
        previous_row_flags = row_flags;
        previous_spacer_head = spacer_head;
    }
    if (cell_count != declared_cell_count) return error.LengthMismatch;

    var extension_bytes: usize = 0;
    var graphics: ?graphics_extension.Summary = null;
    var progress: ?contract.TerminalProgress = null;
    var command_completion: ?contract.TerminalCommandCompletion = null;
    var command_observation: ?contract.TerminalCommandObservation = null;
    var command_started_at: ?u64 = null;
    var close_activity: ?contract.TerminalCloseActivity = null;
    for (0..extension_count) |_| {
        const extension_kind = try reader.readInt(u16);
        const extension_flags = try reader.readInt(u16);
        const length = try reader.readInt(u32);
        if (extension_flags & 0x8000 != 0) return error.UnknownRequiredFlag;
        extension_bytes = std.math.add(usize, extension_bytes, length) catch
            return error.LimitExceeded;
        if (length > max_extension_bytes or extension_bytes > max_extension_bytes) {
            return error.LimitExceeded;
        }
        const extension = try reader.readBytes(length);
        if (extension_kind == contract.terminal_frame_extension_close_activity) {
            if (close_activity != null or extension.len != 1) return error.InvalidField;
            close_activity = std.enums.fromInt(
                contract.TerminalCloseActivity,
                extension[0],
            ) orelse return error.InvalidField;
            if (close_activity == .unknown) return error.InvalidField;
        } else if (extension_kind == contract.terminal_frame_extension_kitty_graphics) {
            if (graphics != null) return error.InvalidField;
            graphics = try graphics_extension.validate(extension);
            if (kind == .snapshot and graphics.?.kind != .snapshot) {
                return error.InvalidField;
            }
        } else if (extension_kind == contract.terminal_frame_extension_progress) {
            if (progress != null or extension.len != 2) return error.InvalidField;
            const state = std.enums.fromInt(
                contract.TerminalProgressState,
                extension[0],
            ) orelse return error.InvalidField;
            const value: ?u8 = if (extension[1] == std.math.maxInt(u8))
                null
            else if (extension[1] <= 100)
                extension[1]
            else
                return error.InvalidField;
            if ((state == .remove or state == .indeterminate) and value != null) {
                return error.InvalidField;
            }
            progress = .{ .state = state, .value = value };
        } else if (extension_kind == contract.terminal_frame_extension_command_completion) {
            if (command_completion != null or extension.len != 25) return error.InvalidField;
            const completion: contract.TerminalCommandCompletion = .{
                .sequence = std.mem.readInt(u64, extension[0..8], .little),
                .duration_milliseconds = std.mem.readInt(u64, extension[8..16], .little),
                .completed_at_milliseconds = std.mem.readInt(u64, extension[16..24], .little),
                .exit_code = extension[24],
            };
            if (completion.sequence == 0 or completion.completed_at_milliseconds == 0) {
                return error.InvalidField;
            }
            command_completion = completion;
        } else if (extension_kind == contract.terminal_frame_extension_command_observation) {
            if (command_observation != null or (extension.len != 10 and extension.len != 66)) return error.InvalidField;
            const epoch = std.mem.readInt(u64, extension[0..8], .little);
            if (epoch == 0 or extension[8] > 1) return error.InvalidField;
            var observation: contract.TerminalCommandObservation = .{
                .output_epoch = epoch,
                .ready = extension[8] == 1,
            };
            switch (extension[9]) {
                0 => if (extension.len != 10) return error.InvalidField,
                1, 2 => {
                    if (extension.len != 66) return error.InvalidField;
                    const command: contract.TerminalCommandObservation.Command = .{
                        .id = extension[10..26].*,
                        .completed = extension[9] == 2,
                        .start_row = std.mem.readInt(u64, extension[26..34], .little),
                        .start_column = std.mem.readInt(u16, extension[34..36], .little),
                        .end_row = std.mem.readInt(u64, extension[36..44], .little),
                        .end_column = std.mem.readInt(u16, extension[44..46], .little),
                        .generation = std.mem.readInt(u64, extension[46..54], .little),
                        .output_epoch = std.mem.readInt(u64, extension[54..62], .little),
                        .exit_code = std.mem.readInt(i32, extension[62..66], .little),
                    };
                    if (std.mem.allEqual(u8, &command.id, 0) or command.generation == 0 or command.output_epoch == 0 or
                        (!command.completed and (command.end_row != 0 or command.end_column != 0 or command.exit_code != 0))) return error.InvalidField;
                    observation.command = command;
                },
                else => return error.InvalidField,
            }
            command_observation = observation;
        } else if (extension_kind == contract.terminal_frame_extension_command_start) {
            if (command_started_at != null or extension.len != 8) return error.InvalidField;
            command_started_at = std.mem.readInt(u64, extension[0..8], .little);
            if (command_started_at == 0) return error.InvalidField;
        }
    }
    if (reader.offset != bytes.len) return error.LengthMismatch;
    // The start time annotates the invocation of this frame's command observation.
    if (command_started_at) |started_at| {
        const observation = if (command_observation) |*value| value else return error.InvalidField;
        const command = if (observation.command) |*value| value else return error.InvalidField;
        command.started_at_nanoseconds = started_at;
    }

    return .{
        .attachment_id = attachment_id,
        .active_screen = active_screen,
        .base_revision = base_revision,
        .cell_count = cell_count,
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .close_activity = close_activity,
        .columns = columns,
        .command_completion = command_completion,
        .command_observation = command_observation,
        .current_directory = try stringAt(strings, current_directory_index),
        .cursor_column = cursor_column,
        .cursor_row = cursor_row,
        .cursor_shape = cursor_shape,
        .cursor_wide_tail = cursor_flags & 0x04 != 0,
        .extension_count = extension_count,
        .generation = generation,
        .graphics = graphics,
        .host_epoch = host_epoch,
        .kind = kind,
        .progress = progress orelse .{},
        .revision = revision,
        .row_count = row_count,
        .rows = rows,
        .scrollback_rows = scrollback_rows,
        .sequence = sequence,
        .string_count = string_count,
        .style_count = style_count,
        .theme_revision = theme_revision,
        .title = try stringAt(strings, title_index),
    };
}
