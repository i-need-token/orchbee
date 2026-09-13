const std = @import("std");

const terminal_input = @import("terminal-input");
const contract = @import("contract.zig");
const wire = @import("wire.zig");

const ProtocolError = contract.ProtocolError;
const MessageKind = contract.MessageKind;
const Modifiers = terminal_input.Modifiers;
const MouseButton = terminal_input.MouseButton;
const MouseButtons = terminal_input.MouseButtons;
const Reader = wire.Reader;
const copyIdentifier = wire.copyIdentifier;
const max_input_bytes = contract.Limits.input_bytes;
const max_write_input_payload_bytes = contract.Limits.write_input_payload_bytes;
const max_semantic_input_payload_bytes = contract.Limits.semantic_input_payload_bytes;
const max_key_text_bytes = contract.Limits.key_text_bytes;

pub const WriteInputSummary = struct {
    attachment_id: [16]u8,
    bytes: []const u8,
    capability_revision: u64,
};

pub const DecodedSemanticInput = struct {
    attachment_id: [16]u8,
    capability_revision: u64,
    input: terminal_input.SemanticInput,
};

const AttachmentTextSummary = struct {
    attachment_id: [16]u8,
    capability_revision: u64,
    text: []const u8,
};

pub fn validateWriteInput(bytes: []const u8) ProtocolError!WriteInputSummary {
    if (bytes.len <= 24) return error.Truncated;
    if (bytes.len > max_write_input_payload_bytes) return error.LimitExceeded;
    const capability_revision = std.mem.readInt(u64, bytes[16..24], .little);
    if (capability_revision == 0) return error.InvalidField;
    return .{
        .attachment_id = try copyIdentifier(bytes[0..16]),
        .bytes = bytes[24..],
        .capability_revision = capability_revision,
    };
}

fn modifiersHaveKnownBits(flags: u16) bool {
    return flags & ~@as(u16, 0x03ff) == 0;
}

fn modifiersAreCanonical(flags: u16) bool {
    inline for (.{ .{ 6, 0 }, .{ 7, 1 }, .{ 8, 2 }, .{ 9, 3 } }) |pair| {
        if (flags & (@as(u16, 1) << pair[0]) != 0 and
            flags & (@as(u16, 1) << pair[1]) == 0)
        {
            return false;
        }
    }
    return true;
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
        // parseUnsigned rejects a leading '+' or '-', so a suffix such as
        // "+1" can never decode as a valid function key; parseInt accepts it.
        const number = std.fmt.parseUnsigned(u8, key[1..], 10) catch 0;
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

pub fn validateKeyInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    if (bytes.len < 40) return error.Truncated;
    if (bytes.len > max_semantic_input_payload_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const action_code = try reader.readInt(u8);
    const flags = try reader.readInt(u8);
    const modifiers = try reader.readInt(u16);
    const consumed_modifiers = try reader.readInt(u16);
    const key_length = try reader.readInt(u16);
    const unshifted_codepoint = try reader.readInt(u32);
    const text_length = try reader.readInt(u32);
    if (!modifiersHaveKnownBits(modifiers) or !modifiersHaveKnownBits(consumed_modifiers) or
        flags & ~@as(u8, 3) != 0)
    {
        return error.UnknownRequiredFlag;
    }
    if (action_code > 2 or !modifiersAreCanonical(modifiers) or
        !modifiersAreCanonical(consumed_modifiers) or
        consumed_modifiers & ~modifiers != 0)
    {
        return error.InvalidField;
    }
    if (text_length > max_key_text_bytes) return error.LimitExceeded;
    const body_length = std.math.add(usize, key_length, text_length) catch return error.LimitExceeded;
    if (body_length != bytes.len - 40) return error.LengthMismatch;
    if (unshifted_codepoint > 0x10ffff or
        (unshifted_codepoint >= 0xd800 and unshifted_codepoint <= 0xdfff))
    {
        return error.InvalidField;
    }
    const key = try reader.readBytes(key_length);
    const text = try reader.readBytes(text_length);
    if (!std.unicode.utf8ValidateSlice(key) or !std.unicode.utf8ValidateSlice(text)) {
        return error.InvalidUtf8;
    }
    if (!validKeyCode(key)) return error.InvalidField;
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .input = .{ .key = .{
            .action = switch (action_code) {
                0 => .release,
                1 => .press,
                2 => .repeat,
                else => unreachable,
            },
            .composing = flags & 1 != 0,
            .consumed_modifiers = Modifiers.fromValidatedBits(consumed_modifiers),
            .key = key,
            .modifiers = Modifiers.fromValidatedBits(modifiers),
            .option_as_alt = flags & (1 << 1) != 0,
            .text = text,
            .unshifted_codepoint = @intCast(unshifted_codepoint),
        } },
    };
}

fn validateTextInput(bytes: []const u8) ProtocolError!AttachmentTextSummary {
    if (bytes.len < 28) return error.Truncated;
    if (bytes.len > 28 + max_input_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const text_length = try reader.readInt(u32);
    if (text_length != bytes.len - 28) return error.LengthMismatch;
    if (text_length == 0) return error.InvalidField;
    const text = try reader.readBytes(text_length);
    if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidUtf8;
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .text = text,
    };
}

pub fn validateCommittedTextInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    const summary = try validateTextInput(bytes);
    return .{
        .attachment_id = summary.attachment_id,
        .capability_revision = summary.capability_revision,
        .input = .{ .text = .{ .text = summary.text } },
    };
}

pub fn validatePasteInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    if (bytes.len < 32) return error.Truncated;
    if (bytes.len > 32 + max_input_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const flags = try reader.readInt(u8);
    const reserved_byte = try reader.readInt(u8);
    const reserved_word = try reader.readInt(u16);
    const text_length = try reader.readInt(u32);
    if (flags & ~@as(u8, 1) != 0 or reserved_byte != 0 or reserved_word != 0) {
        return error.UnknownRequiredFlag;
    }
    if (text_length != bytes.len - 32) return error.LengthMismatch;
    if (text_length == 0) return error.InvalidField;
    const text = try reader.readBytes(text_length);
    if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidUtf8;
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .input = .{ .paste = .{
            .bracketed = flags & 1 != 0,
            .text = text,
        } },
    };
}

pub fn validatePasteAutoInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    if (bytes.len < 32) return error.Truncated;
    if (bytes.len > 32 + contract.Limits.paste_auto_input_bytes) return error.LimitExceeded;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const flags = try reader.readInt(u8);
    const reserved_byte = try reader.readInt(u8);
    const reserved_word = try reader.readInt(u16);
    const content_length = try reader.readInt(u32);
    if (flags != 0 or reserved_byte != 0 or reserved_word != 0) {
        return error.UnknownRequiredFlag;
    }
    if (content_length != bytes.len - 32) return error.LengthMismatch;
    if (content_length == 0) return error.InvalidField;
    const content = try reader.readBytes(content_length);
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .input = .{ .paste_auto = .{ .content = content } },
    };
}

pub fn validateMouseInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    if (bytes.len < 40) return error.Truncated;
    if (bytes.len != 40) return error.LengthMismatch;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const action_code = try reader.readInt(u8);
    const button_code = try reader.readInt(u8);
    const modifiers = try reader.readInt(u16);
    const x: f32 = @bitCast(try reader.readInt(u32));
    const y: f32 = @bitCast(try reader.readInt(u32));
    const pressed_buttons = try reader.readInt(u16);
    const reserved = try reader.readInt(u16);
    if (reserved != 0 or !modifiersHaveKnownBits(modifiers)) {
        return error.UnknownRequiredFlag;
    }
    if (pressed_buttons & ~@as(u16, 0x07ff) != 0) {
        return error.InvalidField;
    }
    if (!modifiersAreCanonical(modifiers) or action_code > 2 or button_code > 11 or
        (action_code != 2 and button_code == 0) or !std.math.isFinite(x) or !std.math.isFinite(y))
    {
        return error.InvalidField;
    }
    if (action_code == 2) {
        if ((button_code == 0) != (pressed_buttons == 0)) return error.InvalidField;
        if (button_code != 0) {
            const button_shift: u4 = @intCast(button_code - 1);
            if (pressed_buttons & (@as(u16, 1) << button_shift) == 0) {
                return error.InvalidField;
            }
        }
    }
    const button: ?MouseButton = switch (button_code) {
        0 => null,
        1 => .left,
        2 => .right,
        3 => .middle,
        4 => .four,
        5 => .five,
        6 => .six,
        7 => .seven,
        8 => .eight,
        9 => .nine,
        10 => .ten,
        11 => .eleven,
        else => unreachable,
    };
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .input = .{ .mouse = .{
            .action = switch (action_code) {
                0 => .{ .press = button.? },
                1 => .{ .release = button.? },
                2 => .{ .motion = if (button) |active_button|
                    .{ .drag = active_button }
                else
                    .hover },
                else => unreachable,
            },
            .modifiers = Modifiers.fromValidatedBits(modifiers),
            .pressed_buttons = MouseButtons.fromValidatedBits(pressed_buttons),
            .x = x,
            .y = y,
        } },
    };
}

pub fn validateFocusInput(bytes: []const u8) ProtocolError!DecodedSemanticInput {
    if (bytes.len < 28) return error.Truncated;
    if (bytes.len != 28) return error.LengthMismatch;
    var reader = Reader.init(bytes, 0);
    const attachment_id = try copyIdentifier(try reader.readBytes(16));
    const capability_revision = try reader.readInt(u64);
    if (capability_revision == 0) return error.InvalidField;
    const focused = try reader.readInt(u8);
    const reserved = try reader.readBytes(3);
    if ((focused != 0 and focused != 1) or reserved[0] != 0 or reserved[1] != 0 or reserved[2] != 0) {
        return error.UnknownRequiredFlag;
    }
    return .{
        .attachment_id = attachment_id,
        .capability_revision = capability_revision,
        .input = .{ .focus = .{ .focused = focused == 1 } },
    };
}

pub fn validateSemanticInput(kind: u16, bytes: []const u8) ProtocolError!DecodedSemanticInput {
    return switch (kind) {
        MessageKind.key_input => validateKeyInput(bytes),
        MessageKind.text_input => validateCommittedTextInput(bytes),
        MessageKind.mouse_input => validateMouseInput(bytes),
        MessageKind.focus_input => validateFocusInput(bytes),
        MessageKind.paste_input => validatePasteInput(bytes),
        MessageKind.paste_auto_input => validatePasteAutoInput(bytes),
        else => error.UnknownRequiredMessage,
    };
}
