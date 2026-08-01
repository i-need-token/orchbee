const std = @import("std");

pub const terminal_frame_header_bytes = 160;
pub const terminal_row_window_header_bytes = 112;
pub const transport_envelope_header_bytes = 64;

const max_frame_bytes = 8 * 1024 * 1024;
const max_control_json_bytes = 64 * 1024;
const max_frame_cells = 256 * 1024;
const max_frame_columns = 4096;
const max_frame_extensions = 64;
const max_frame_rows = 4096;
const max_frame_strings = 4096;
const max_frame_styles = 1024;
const max_cell_pixel_dimension = 4096;
const max_extension_bytes = 1024 * 1024;
const max_input_bytes = 1024 * 1024;
const max_write_input_payload_bytes = 16 + max_input_bytes;
const max_semantic_input_payload_bytes = 32 + max_input_bytes;
const max_raw_payload_bytes = max_semantic_input_payload_bytes;
const max_key_text_bytes = 4096;
const max_row_window_bytes = 2 * 1024 * 1024;
const max_string_bytes = 16 * 1024;
const max_transport_record_bytes = max_frame_bytes + transport_envelope_header_bytes;
const no_string_index = std.math.maxInt(u32);
const no_base_revision = std.math.maxInt(u64);

pub const ProtocolError = error{
    InvalidField,
    InvalidMagic,
    InvalidUtf8,
    LengthMismatch,
    LimitExceeded,
    Truncated,
    UnknownRequiredFlag,
    UnknownRequiredMessage,
    UnsupportedVersion,
};

pub const FrameKind = enum {
    snapshot,
    delta,
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

pub const RowWindowDetail = enum {
    render,
    text,
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

pub const FrameSummary = struct {
    attachment_id: [16]u8,
    base_revision: ?u64,
    cell_count: usize,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    current_directory: ?[]const u8,
    cursor_column: u16,
    cursor_row: u16,
    cursor_shape: CursorShape,
    cursor_wide_tail: bool,
    extension_count: u16,
    generation: u64,
    host_epoch: [16]u8,
    kind: FrameKind,
    revision: u64,
    row_count: u16,
    rows: u16,
    scrollback_rows: u64,
    sequence: u64,
    string_count: u32,
    style_count: u16,
    title: ?[]const u8,
    active_screen: ActiveScreen,
};

pub const EnvelopeSummary = struct {
    correlation_id: ?[16]u8,
    event: bool,
    host_epoch: ?[16]u8,
    kind: u16,
    optional_message: bool,
    payload: []const u8,
    payload_encoding: u8,
    protocol_major: u16,
    protocol_minor: u16,
    response: bool,
};

pub const PayloadEncoding = enum(u8) {
    control_json = 1,
    raw_bytes = 2,
    terminal_frame = 3,
    row_window = 4,
};

pub const WriteInputSummary = struct {
    attachment_id: [16]u8,
    bytes: []const u8,
};

pub const KeyInputSummary = struct {
    attachment_id: [16]u8,
    action: u8,
    composing: bool,
    consumed_modifiers: u16,
    key: []const u8,
    modifiers: u16,
    text: []const u8,
    unshifted_codepoint: u32,
};

pub const TextInputSummary = struct {
    attachment_id: [16]u8,
    text: []const u8,
};

pub const MouseInputSummary = struct {
    action: u8,
    attachment_id: [16]u8,
    button: ?u8,
    modifiers: u16,
    pressed_buttons: u16,
    x: f32,
    y: f32,
};

pub const FocusInputSummary = struct {
    attachment_id: [16]u8,
    focused: bool,
};

const Reader = struct {
    bytes: []const u8,
    offset: usize,

    fn init(bytes: []const u8, offset: usize) Reader {
        return .{ .bytes = bytes, .offset = offset };
    }

    fn readBytes(self: *Reader, length: usize) ProtocolError![]const u8 {
        if (length > self.bytes.len -| self.offset) return error.Truncated;
        const value = self.bytes[self.offset .. self.offset + length];
        self.offset += length;
        return value;
    }

    fn readInt(self: *Reader, comptime Int: type) ProtocolError!Int {
        const bytes = try self.readBytes(@sizeOf(Int));
        return std.mem.readInt(Int, bytes[0..@sizeOf(Int)], .little);
    }
};

fn copyIdentifier(bytes: []const u8) ProtocolError![16]u8 {
    if (bytes.len != 16) return error.InvalidField;
    var result: [16]u8 = undefined;
    @memcpy(&result, bytes);
    var non_zero = false;
    for (result) |byte| non_zero = non_zero or byte != 0;
    if (!non_zero) return error.InvalidField;
    return result;
}

fn optionalIdentifier(bytes: []const u8) ProtocolError!?[16]u8 {
    var non_zero = false;
    for (bytes) |byte| non_zero = non_zero or byte != 0;
    return if (non_zero) try copyIdentifier(bytes) else null;
}

fn requireMagic(bytes: []const u8, expected: *const [4]u8) ProtocolError!void {
    if (bytes.len < expected.len) return error.Truncated;
    if (!std.mem.eql(u8, bytes[0..4], expected)) return error.InvalidMagic;
}

fn stringAt(strings: []const []const u8, index: u32) ProtocolError!?[]const u8 {
    if (index == no_string_index) return null;
    if (index >= strings.len) return error.InvalidField;
    return strings[index];
}

pub fn isKnownMessageKind(kind: u16) bool {
    return switch (kind) {
        1...5,
        10...17,
        20...23,
        30...38,
        41...49,
        50...54,
        60...68,
        => true,
        else => false,
    };
}

fn maximumPayloadBytes(encoding: u8) ProtocolError!usize {
    return switch (encoding) {
        1 => max_control_json_bytes,
        2 => max_raw_payload_bytes,
        3 => max_frame_bytes,
        4 => max_row_window_bytes,
        else => error.InvalidField,
    };
}

pub fn validateWriteInput(bytes: []const u8) ProtocolError!WriteInputSummary {
    if (bytes.len <= 16) return error.Truncated;
    if (bytes.len > max_write_input_payload_bytes) return error.LimitExceeded;
    return .{
        .attachment_id = try copyIdentifier(bytes[0..16]),
        .bytes = bytes[16..],
    };
}

fn validModifiers(flags: u16) bool {
    return flags & ~@as(u16, 0x03ff) == 0;
}

fn validKeyCode(key: []const u8) bool {
    if (key.len == 4 and std.mem.startsWith(u8, key, "Key") and key[3] >= 'A' and key[3] <= 'Z') {
        return true;
    }
    if (key.len == 6 and std.mem.startsWith(u8, key, "Digit") and key[5] >= '0' and key[5] <= '9') {
        return true;
    }
    if (key.len == 7 and std.mem.startsWith(u8, key, "Numpad") and key[6] >= '0' and key[6] <= '9') {
        return true;
    }
    if (key.len >= 2 and key.len <= 3 and key[0] == 'F') {
        const number = std.fmt.parseInt(u8, key[1..], 10) catch 0;
        if (number >= 1 and number <= 25 and (key.len == 2 or key[1] != '0')) return true;
    }
    inline for (.{
        "Unidentified",         "Backquote",          "Backslash",
        "BracketLeft",          "BracketRight",       "Comma",
        "Equal",                "IntlBackslash",      "IntlRo",
        "IntlYen",              "Minus",              "Period",
        "Quote",                "Semicolon",          "Slash",
        "AltLeft",              "AltRight",           "Backspace",
        "CapsLock",             "ContextMenu",        "ControlLeft",
        "ControlRight",         "Enter",              "MetaLeft",
        "MetaRight",            "ShiftLeft",          "ShiftRight",
        "Space",                "Tab",                "Convert",
        "KanaMode",             "NonConvert",         "Delete",
        "End",                  "Help",               "Home",
        "Insert",               "PageDown",           "PageUp",
        "ArrowDown",            "ArrowLeft",          "ArrowRight",
        "ArrowUp",              "NumLock",            "NumpadAdd",
        "NumpadBackspace",      "NumpadClear",        "NumpadClearEntry",
        "NumpadComma",          "NumpadDecimal",      "NumpadDivide",
        "NumpadEnter",          "NumpadEqual",        "NumpadMemoryAdd",
        "NumpadMemoryClear",    "NumpadMemoryRecall", "NumpadMemoryStore",
        "NumpadMemorySubtract", "NumpadMultiply",     "NumpadParenLeft",
        "NumpadParenRight",     "NumpadSubtract",     "NumpadSeparator",
        "NumpadUp",             "NumpadDown",         "NumpadRight",
        "NumpadLeft",           "NumpadBegin",        "NumpadHome",
        "NumpadEnd",            "NumpadInsert",       "NumpadDelete",
        "NumpadPageUp",         "NumpadPageDown",     "Escape",
        "Fn",                   "FnLock",             "PrintScreen",
        "ScrollLock",           "Pause",              "BrowserBack",
        "BrowserFavorites",     "BrowserForward",     "BrowserHome",
        "BrowserRefresh",       "BrowserSearch",      "BrowserStop",
        "Eject",                "LaunchApp1",         "LaunchApp2",
        "LaunchMail",           "MediaPlayPause",     "MediaSelect",
        "MediaStop",            "MediaTrackNext",     "MediaTrackPrevious",
        "Power",                "Sleep",              "AudioVolumeDown",
        "AudioVolumeMute",      "AudioVolumeUp",      "WakeUp",
        "Copy",                 "Cut",                "Paste",
    }) |candidate| {
        if (std.mem.eql(u8, key, candidate)) return true;
    }
    return false;
}

pub fn validateKeyInput(bytes: []const u8) ProtocolError!KeyInputSummary {
    if (bytes.len < 32) return error.Truncated;
    if (bytes.len > max_semantic_input_payload_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const action = try reader.readInt(u8);
    const flags = try reader.readInt(u8);
    const modifiers = try reader.readInt(u16);
    const consumed_modifiers = try reader.readInt(u16);
    const key_length = try reader.readInt(u16);
    const unshifted_codepoint = try reader.readInt(u32);
    const text_length = try reader.readInt(u32);
    if (action > 2 or !validModifiers(modifiers) or !validModifiers(consumed_modifiers) or
        flags & ~@as(u8, 1) != 0)
    {
        return error.UnknownRequiredFlag;
    }
    if (text_length > max_key_text_bytes) return error.LimitExceeded;
    const body_length = std.math.add(usize, key_length, text_length) catch return error.LimitExceeded;
    if (body_length != bytes.len - 32) return error.LengthMismatch;
    if (unshifted_codepoint > 0x10ffff or
        (unshifted_codepoint >= 0xd800 and unshifted_codepoint <= 0xdfff))
    {
        return error.InvalidField;
    }
    const key = try reader.readBytes(key_length);
    const text = try reader.readBytes(text_length);
    if (!validKeyCode(key) or !std.unicode.utf8ValidateSlice(text)) {
        return if (!std.unicode.utf8ValidateSlice(text)) error.InvalidUtf8 else error.InvalidField;
    }
    return .{
        .action = action,
        .attachment_id = attachment_id,
        .composing = flags & 1 != 0,
        .consumed_modifiers = consumed_modifiers,
        .key = key,
        .modifiers = modifiers,
        .text = text,
        .unshifted_codepoint = unshifted_codepoint,
    };
}

fn validateTextInput(bytes: []const u8) ProtocolError!TextInputSummary {
    if (bytes.len < 20) return error.Truncated;
    if (bytes.len > 20 + max_input_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const text_length = try reader.readInt(u32);
    if (text_length == 0) return error.InvalidField;
    if (text_length != bytes.len - 20) return error.LengthMismatch;
    const text = try reader.readBytes(text_length);
    if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidUtf8;
    return .{ .attachment_id = attachment_id, .text = text };
}

pub fn validateCommittedTextInput(bytes: []const u8) ProtocolError!TextInputSummary {
    return validateTextInput(bytes);
}

pub fn validatePasteInput(bytes: []const u8) ProtocolError!TextInputSummary {
    return validateTextInput(bytes);
}

pub fn validateMouseInput(bytes: []const u8) ProtocolError!MouseInputSummary {
    if (bytes.len < 32) return error.Truncated;
    if (bytes.len != 32) return error.LengthMismatch;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const action = try reader.readInt(u8);
    const button_code = try reader.readInt(u8);
    const modifiers = try reader.readInt(u16);
    const x: f32 = @bitCast(try reader.readInt(u32));
    const y: f32 = @bitCast(try reader.readInt(u32));
    const pressed_buttons = try reader.readInt(u16);
    const reserved = try reader.readInt(u16);
    if (reserved != 0 or !validModifiers(modifiers) or pressed_buttons & ~@as(u16, 0x07ff) != 0) {
        return error.UnknownRequiredFlag;
    }
    if (action > 2 or button_code > 11 or !std.math.isFinite(x) or !std.math.isFinite(y)) {
        return error.InvalidField;
    }
    return .{
        .action = action,
        .attachment_id = attachment_id,
        .button = if (button_code == 0) null else button_code,
        .modifiers = modifiers,
        .pressed_buttons = pressed_buttons,
        .x = x,
        .y = y,
    };
}

pub fn validateFocusInput(bytes: []const u8) ProtocolError!FocusInputSummary {
    if (bytes.len < 20) return error.Truncated;
    if (bytes.len != 20) return error.LengthMismatch;
    const attachment_id = try copyIdentifier(bytes[0..16]);
    if ((bytes[16] != 0 and bytes[16] != 1) or bytes[17] != 0 or bytes[18] != 0 or bytes[19] != 0) {
        return error.UnknownRequiredFlag;
    }
    return .{ .attachment_id = attachment_id, .focused = bytes[16] == 1 };
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
    try requireMagic(bytes, "OBTF");

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
    const reserved_tail = try reader.readBytes(16);

    if (format_major != 2 or format_minor > 0 or header_length != terminal_frame_header_bytes) {
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
        cell_pixel_width == 0 or cell_pixel_width > max_cell_pixel_dimension or
        cell_pixel_height == 0 or cell_pixel_height > max_cell_pixel_dimension or
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
    if (interaction_flags & ~@as(u32, 0x0003ffff) != 0) return error.UnknownRequiredFlag;
    if (@popCount(interaction_flags & 0x000000f0) > 1 or
        @popCount(interaction_flags & 0x00000f00) > 1)
    {
        return error.InvalidField;
    }

    const kind: FrameKind = switch (kind_code) {
        1 => .snapshot,
        2 => .delta,
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
        if (previous_row) |prior_row| {
            if (row <= prior_row) return error.InvalidField;
        }
        if (kind == .snapshot and row != row_offset) return error.InvalidField;
        cell_count += count;
        if (cell_count > max_frame_cells) return error.LimitExceeded;
        previous_row = row;

        var previous_width: ?u8 = null;
        for (0..count) |_| {
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
                (previous_width == 1 and width != 2)) return error.InvalidField;
            previous_width = width;
        }
        if (previous_width == 1) return error.InvalidField;
    }
    if (cell_count != declared_cell_count) return error.LengthMismatch;

    var extension_bytes: usize = 0;
    for (0..extension_count) |_| {
        _ = try reader.readInt(u16);
        const extension_flags = try reader.readInt(u16);
        const length = try reader.readInt(u32);
        if (extension_flags & 0x8000 != 0) return error.UnknownRequiredFlag;
        extension_bytes += length;
        if (length > max_extension_bytes or extension_bytes > max_extension_bytes) {
            return error.LimitExceeded;
        }
        _ = try reader.readBytes(length);
    }
    if (reader.offset != bytes.len) return error.LengthMismatch;

    return .{
        .attachment_id = attachment_id,
        .active_screen = active_screen,
        .base_revision = base_revision,
        .cell_count = cell_count,
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .columns = columns,
        .current_directory = try stringAt(strings, current_directory_index),
        .cursor_column = cursor_column,
        .cursor_row = cursor_row,
        .cursor_shape = cursor_shape,
        .cursor_wide_tail = cursor_flags & 0x04 != 0,
        .extension_count = extension_count,
        .generation = generation,
        .host_epoch = host_epoch,
        .kind = kind,
        .revision = revision,
        .row_count = row_count,
        .rows = rows,
        .scrollback_rows = scrollback_rows,
        .sequence = sequence,
        .string_count = string_count,
        .style_count = style_count,
        .title = try stringAt(strings, title_index),
    };
}

pub fn validateTerminalRowWindow(
    allocator: std.mem.Allocator,
    bytes: []const u8,
) (ProtocolError || std.mem.Allocator.Error)!RowWindowSummary {
    if (bytes.len < terminal_row_window_header_bytes) return error.Truncated;
    if (bytes.len > max_row_window_bytes) return error.LimitExceeded;
    try requireMagic(bytes, "OBRW");

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
    const reserved_tail = try reader.readBytes(12);

    if (format_major != 1 or format_minor > 1 or
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
        1 => .render,
        2 => .text,
        else => return error.InvalidField,
    };
    const body = bytes[header_length..];
    switch (detail) {
        .render => {
            const frame = try validateTerminalFrame(allocator, body);
            if (frame.kind != .snapshot or
                !std.mem.eql(u8, &frame.host_epoch, &host_epoch) or
                !std.mem.eql(u8, &frame.attachment_id, &attachment_id) or
                frame.sequence != sequence or frame.revision != revision or
                frame.generation != generation or frame.columns != columns or
                frame.rows != row_count or entry_count != @as(u32, row_count) * columns)
            {
                return error.InvalidField;
            }
        },
        .text => try validateTextRowWindowBody(
            body,
            format_minor,
            row_count,
            columns,
            entry_count,
        ),
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
    format_minor: u8,
    row_count: u16,
    columns: u16,
    expected_entry_count: u32,
) ProtocolError!void {
    var reader = Reader.init(body, 0);
    var entries: u32 = 0;
    for (0..row_count) |_| {
        const row_flags = try reader.readInt(u8);
        const prompt_class = try reader.readInt(u8);
        const reserved = try reader.readInt(u16);
        const text_length = try reader.readInt(u32);
        const mapping_count = try reader.readInt(u32);
        const supported_row_flags: u8 = 0x03 | (if (format_minor >= 1) @as(u8, 0x04) else 0);
        if (row_flags & ~supported_row_flags != 0 or reserved != 0) {
            return error.UnknownRequiredFlag;
        }
        if (prompt_class > 2 or mapping_count > columns or
            mapping_count > max_frame_cells - entries)
        {
            return error.LimitExceeded;
        }
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
        var previous_byte: ?u32 = null;
        var previous_utf16: ?u32 = null;
        var previous_column: ?u16 = null;
        for (0..mapping_count) |_| {
            const byte_offset = try reader.readInt(u32);
            const utf16_offset = try reader.readInt(u32);
            const column = try reader.readInt(u16);
            const width = try reader.readInt(u8);
            const mapping_reserved = try reader.readInt(u8);
            if (mapping_reserved != 0) return error.UnknownRequiredFlag;
            if (byte_offset > text.len or utf16_offset > utf16_length or column >= columns or
                (width != 1 and width != 2) or
                (previous_byte != null and byte_offset <= previous_byte.?) or
                (previous_utf16 != null and utf16_offset <= previous_utf16.?) or
                (previous_column != null and column <= previous_column.?))
            {
                return error.InvalidField;
            }
            previous_byte = byte_offset;
            previous_utf16 = utf16_offset;
            previous_column = column;
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

pub fn validateTransportEnvelope(bytes: []const u8) ProtocolError!EnvelopeSummary {
    if (bytes.len < transport_envelope_header_bytes) return error.Truncated;
    if (bytes.len > max_transport_record_bytes) return error.LimitExceeded;
    try requireMagic(bytes, "OBTP");

    var reader = Reader.init(bytes, 4);
    const header_length = try reader.readInt(u16);
    const flags = try reader.readInt(u16);
    const total_length = try reader.readInt(u32);
    const payload_length = try reader.readInt(u32);
    const protocol_major = try reader.readInt(u16);
    const protocol_minor = try reader.readInt(u16);
    const kind = try reader.readInt(u16);
    const payload_encoding = try reader.readInt(u8);
    const reserved_byte = try reader.readInt(u8);
    const correlation_id = try optionalIdentifier(try reader.readBytes(16));
    const host_epoch = try optionalIdentifier(try reader.readBytes(16));
    const reserved = try reader.readBytes(8);

    if (header_length != transport_envelope_header_bytes) return error.UnsupportedVersion;
    if (total_length != bytes.len or payload_length != bytes.len - header_length) {
        return error.LengthMismatch;
    }
    if (reserved_byte != 0 or flags & 0xff00 != 0) return error.UnknownRequiredFlag;
    for (reserved) |byte| if (byte != 0) return error.UnknownRequiredFlag;
    const event = flags & 4 != 0;
    const response = flags & 2 != 0;
    if (event and response) return error.InvalidField;
    const optional_message = flags & 1 != 0;
    if (!isKnownMessageKind(kind) and !optional_message) return error.UnknownRequiredMessage;
    if (payload_length > try maximumPayloadBytes(payload_encoding)) return error.LimitExceeded;

    return .{
        .correlation_id = correlation_id,
        .event = event,
        .host_epoch = host_epoch,
        .kind = kind,
        .optional_message = optional_message,
        .payload = bytes[header_length..],
        .payload_encoding = payload_encoding,
        .protocol_major = protocol_major,
        .protocol_minor = protocol_minor,
        .response = response,
    };
}

pub fn transportRecordLength(header: []const u8) ProtocolError!usize {
    if (header.len < transport_envelope_header_bytes) return error.Truncated;
    try requireMagic(header, "OBTP");
    const header_length = std.mem.readInt(u16, header[4..6], .little);
    const total_length = std.mem.readInt(u32, header[8..12], .little);
    const payload_length = std.mem.readInt(u32, header[12..16], .little);
    if (header_length != transport_envelope_header_bytes) return error.UnsupportedVersion;
    if (total_length < transport_envelope_header_bytes or
        total_length > max_transport_record_bytes or
        payload_length != total_length - transport_envelope_header_bytes)
    {
        return error.LengthMismatch;
    }
    return total_length;
}

pub fn encodeTransportEnvelope(
    allocator: std.mem.Allocator,
    kind: u16,
    encoding: PayloadEncoding,
    payload: []const u8,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
    response: bool,
    event: bool,
) (ProtocolError || std.mem.Allocator.Error)![]u8 {
    return encodeTransportEnvelopeVersion(
        allocator,
        0,
        kind,
        encoding,
        payload,
        correlation_id,
        host_epoch,
        response,
        event,
    );
}

pub fn encodeTransportEnvelopeVersion(
    allocator: std.mem.Allocator,
    protocol_minor: u16,
    kind: u16,
    encoding: PayloadEncoding,
    payload: []const u8,
    correlation_id: ?[16]u8,
    host_epoch: ?[16]u8,
    response: bool,
    event: bool,
) (ProtocolError || std.mem.Allocator.Error)![]u8 {
    if (protocol_minor > 2) return error.UnsupportedVersion;
    if (event and response) return error.InvalidField;
    if (payload.len > try maximumPayloadBytes(@intFromEnum(encoding))) return error.LimitExceeded;
    const total_length = transport_envelope_header_bytes + payload.len;
    if (total_length > max_transport_record_bytes) return error.LimitExceeded;
    const bytes = try allocator.alloc(u8, total_length);
    errdefer allocator.free(bytes);
    @memset(bytes, 0);
    @memcpy(bytes[0..4], "OBTP");
    std.mem.writeInt(u16, bytes[4..6], transport_envelope_header_bytes, .little);
    var flags: u16 = 0;
    if (response) flags |= 2;
    if (event) flags |= 4;
    std.mem.writeInt(u16, bytes[6..8], flags, .little);
    std.mem.writeInt(u32, bytes[8..12], @intCast(total_length), .little);
    std.mem.writeInt(u32, bytes[12..16], @intCast(payload.len), .little);
    std.mem.writeInt(u16, bytes[16..18], 1, .little);
    std.mem.writeInt(u16, bytes[18..20], protocol_minor, .little);
    std.mem.writeInt(u16, bytes[20..22], kind, .little);
    bytes[22] = @intFromEnum(encoding);
    if (correlation_id) |identifier| @memcpy(bytes[24..40], &identifier);
    if (host_epoch) |identifier| @memcpy(bytes[40..56], &identifier);
    @memcpy(bytes[transport_envelope_header_bytes..], payload);
    return bytes;
}

test "frame validator rejects zero required identifiers" {
    var bytes = [_]u8{0} ** terminal_frame_header_bytes;
    @memcpy(bytes[0..4], "OBTF");
    try std.testing.expectError(error.InvalidField, validateTerminalFrame(std.testing.allocator, &bytes));
}
