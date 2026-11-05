//! OSC 7501 program status: a bounded scanner that finds complete `ESC ] 7501 ; ... ST|BEL`
//! sequences in child output, and the parser of their root-record reports.
//!
//! The pinned Ghostty discards this unknown OSC, so the Host recognizes it beside Ghostty's
//! parser with the same VT500 string boundaries: ESC, CAN, and SUB end any OSC, and only a BEL
//! or `ESC \` completes one. The grammar follows the Program Status specification
//! (https://www.superlogical.com/rex/docs/build/program-status). Only the root record is
//! retained; records with an `id` and every display field are validated and then dropped.
const std = @import("std");
const protocol = @import("terminal-protocol");

pub const State = protocol.TerminalProgramStatusState;

/// The specification's whole-sequence limit, introducer and two-byte terminator included.
pub const max_sequence_bytes = 4096;
const introducer_digits = "7501;";
const max_body_bytes = max_sequence_bytes - "\x1b]".len - introducer_digits.len - "\x1b\\".len;

pub const Terminator = enum { bel, st };

pub const Sequence = union(enum) {
    /// Feature detection; the reply mirrors the query's terminator.
    query: Terminator,
    /// The pair list of a report, borrowed from the scanner until its next `scan`.
    report: []const u8,
};

pub const Step = struct {
    /// Bytes consumed by this step; a completed sequence ends at its last terminator byte.
    consumed: usize,
    sequence: ?Sequence = null,
};

/// Byte state that persists across reader batches, owned by one terminal's reader commit.
pub const Scanner = struct {
    phase: Phase = .ground,
    prefix_length: usize = 0,
    body_length: usize = 0,
    /// A control byte, a non-ASCII byte, or a body beyond the bound: drop at the terminator.
    malformed: bool = false,
    body: [max_body_bytes]u8 = undefined,

    const Phase = enum { ground, escape, prefix, body, body_escape, other_osc };

    /// Consumes `bytes` up to and including the terminator of the first complete OSC 7501
    /// sequence, or all of them. Always consumes at least one byte of a nonempty input.
    pub fn scan(self: *Scanner, bytes: []const u8) Step {
        for (bytes, 0..) |byte, index| {
            if (self.next(byte)) |sequence| {
                return .{ .consumed = index + 1, .sequence = sequence };
            }
        }
        return .{ .consumed = bytes.len };
    }

    fn next(self: *Scanner, byte: u8) ?Sequence {
        switch (self.phase) {
            .ground => {
                if (byte == 0x1b) self.phase = .escape;
            },
            .escape => self.phase = switch (byte) {
                ']' => blk: {
                    self.prefix_length = 0;
                    break :blk .prefix;
                },
                0x1b => .escape,
                else => .ground,
            },
            .prefix => {
                if (self.endsString(byte)) return null;
                if (byte != introducer_digits[self.prefix_length]) {
                    self.phase = .other_osc;
                } else if (self.prefix_length + 1 == introducer_digits.len) {
                    self.phase = .body;
                    self.body_length = 0;
                    self.malformed = false;
                } else {
                    self.prefix_length += 1;
                }
            },
            .body => switch (byte) {
                0x07 => {
                    self.phase = .ground;
                    return self.complete(.bel);
                },
                0x1b => self.phase = .body_escape,
                0x18, 0x1a => self.phase = .ground,
                else => self.append(byte),
            },
            .body_escape => {
                if (byte == '\\') {
                    self.phase = .ground;
                    return self.complete(.st);
                }
                // ESC alone already ended the OSC without a valid terminator; this byte
                // follows that ESC as Ghostty's parser sees it.
                self.phase = .escape;
                return self.next(byte);
            },
            .other_osc => _ = self.endsString(byte),
        }
        return null;
    }

    /// Applies the VT500 bytes that end any OSC string, reporting whether `byte` was one.
    fn endsString(self: *Scanner, byte: u8) bool {
        switch (byte) {
            0x07, 0x18, 0x1a => self.phase = .ground,
            0x1b => self.phase = .escape,
            else => return false,
        }
        return true;
    }

    fn append(self: *Scanner, byte: u8) void {
        if (byte < 0x20 or byte >= 0x7f or self.body_length == self.body.len) {
            self.malformed = true;
            return;
        }
        self.body[self.body_length] = byte;
        self.body_length += 1;
    }

    fn complete(self: *Scanner, terminator: Terminator) ?Sequence {
        if (self.malformed) return null;
        const body = self.body[0..self.body_length];
        if (std.mem.eql(u8, body, "?")) return .{ .query = terminator };
        return .{ .report = body };
    }
};

/// The effect of one report on the root record.
pub const Report = union(enum) {
    set: State,
    clear,
};

const Field = enum { state, id, app, msg, title };

/// Parses one report body. Returns null when the report must be ignored: a missing or unknown
/// state, a record other than the root, or an exceeded limit, undecodable base64, or control
/// text, which discards the whole report. A pair without `=`, with an empty or non-`[a-z]` key,
/// or with a byte outside the value set is skipped; unknown keys are ignored, and a repeated key
/// keeps its last value.
pub fn parse(body: []const u8) ?Report {
    var values: [std.enums.values(Field).len]?[]const u8 = @splat(null);
    var pairs = std.mem.splitScalar(u8, body, ':');
    while (pairs.next()) |pair| {
        const separator = std.mem.indexOfScalar(u8, pair, '=') orelse continue;
        const key = pair[0..separator];
        const value = pair[separator + 1 ..];
        if (key.len == 0 or !allBytes(key, isKeyByte) or !allBytes(value, isValueByte)) continue;
        if (key.len > 16) return null;
        const field = std.meta.stringToEnum(Field, key) orelse continue;
        values[@intFromEnum(field)] = value;
    }
    if (values[@intFromEnum(Field.app)]) |app| if (app.len > 32) return null;
    if (values[@intFromEnum(Field.msg)]) |msg| if (!validText(msg, 2732, 2048)) return null;
    if (values[@intFromEnum(Field.title)]) |title| if (!validText(title, 256, 192)) return null;
    // Any id, valid or not, addresses no root effect: a child record or an ignored report.
    if (values[@intFromEnum(Field.id)] != null) return null;
    const state = values[@intFromEnum(Field.state)] orelse return null;
    if (std.mem.eql(u8, state, "clear")) return .clear;
    return .{ .set = std.meta.stringToEnum(State, state) orelse return null };
}

fn allBytes(bytes: []const u8, comptime admitted: fn (u8) bool) bool {
    for (bytes) |byte| if (!admitted(byte)) return false;
    return true;
}

fn isKeyByte(byte: u8) bool {
    return std.ascii.isLower(byte);
}

fn isValueByte(byte: u8) bool {
    return std.ascii.isAlphanumeric(byte) or std.mem.indexOfScalar(u8, "_.,+/=-", byte) != null;
}

/// Base64 UTF-8 text within its encoded and decoded limits and free of control characters.
/// The encoded size is checked before decoding, and padding is optional.
fn validText(encoded: []const u8, max_encoded: usize, comptime max_decoded: usize) bool {
    if (encoded.len > max_encoded) return false;
    const decoder = if (encoded.len % 4 == 0)
        std.base64.standard.Decoder
    else
        std.base64.standard_no_pad.Decoder;
    const length = decoder.calcSizeForSlice(encoded) catch return false;
    if (length > max_decoded) return false;
    var buffer: [max_decoded]u8 = undefined;
    decoder.decode(buffer[0..length], encoded) catch return false;
    const view = std.unicode.Utf8View.init(buffer[0..length]) catch return false;
    var codepoints = view.iterator();
    while (codepoints.nextCodepoint()) |codepoint| {
        if (codepoint < 0x20 or (codepoint >= 0x7f and codepoint < 0xa0)) return false;
    }
    return true;
}
