//! The `OBHR` envelope and its checksummed sections. docs/handover-record.md is the format's
//! authority; this file is its encoder and decoder, and nothing else reads the framing.
const std = @import("std");
const wire = @import("wire.zig");

pub const magic = "OBHR";
pub const generation: u16 = 1;
/// The protocol reference bounds the record its chunks carry by the same 128 MiB.
pub const max_record_bytes: usize = 128 * 1024 * 1024;
pub const max_product_version_bytes = 64;
pub const max_pin_bytes = 128;
pub const max_sections = 32;
const section_header_bytes = 12;
const required_flag: u16 = 1;

pub const Crc = std.hash.crc.Crc32Iscsi;

pub const Tag = enum(u16) {
    terminal = 1,
    configuration = 2,
    screen = 3,
    engine = 4,
    console = 5,
    _,
};

pub const Section = struct {
    tag: Tag,
    required: bool,
    payload: []const u8,
};

pub const DecodeError = error{
    /// Malformed, truncated, or failing a checksum.
    InvalidRecord,
    UnsupportedGeneration,
    ForeignPin,
    UnknownRequiredSection,
    RecordTooLarge,
};

pub const Header = struct {
    product_version: []const u8,
    pin: []const u8,
};

/// Writes the envelope and `sections`, whose tags ascend, into one allocation.
pub fn encode(
    allocator: std.mem.Allocator,
    header: Header,
    sections: []const Section,
) (error{ InvalidRecord, RecordTooLarge } || std.mem.Allocator.Error)![]u8 {
    if (!printable(header.product_version, max_product_version_bytes) or
        !printable(header.pin, max_pin_bytes) or
        sections.len == 0 or sections.len > max_sections) return error.InvalidRecord;
    var total: usize = 4 + 2 + 2 + 1 + header.product_version.len + 1 + header.pin.len + 2 + 4;
    var previous: u16 = 0;
    for (sections) |section| {
        const tag = @intFromEnum(section.tag);
        if (tag <= previous or section.payload.len > std.math.maxInt(u32)) return error.InvalidRecord;
        previous = tag;
        total += section_header_bytes + section.payload.len;
        if (total > max_record_bytes) return error.RecordTooLarge;
    }

    var list: std.ArrayList(u8) = try .initCapacity(allocator, total);
    errdefer list.deinit(allocator);
    const out: wire.Writer = .{ .allocator = allocator, .list = &list };
    try out.bytes(magic);
    try out.int(u16, generation);
    try out.int(u16, 0);
    try out.counted(u8, header.product_version);
    try out.counted(u8, header.pin);
    try out.int(u16, @intCast(sections.len));
    try out.int(u32, Crc.hash(list.items));
    for (sections) |section| {
        const start = list.items.len;
        try out.int(u16, @intFromEnum(section.tag));
        try out.int(u16, if (section.required) required_flag else 0);
        try out.int(u32, @intCast(section.payload.len));
        var crc: Crc = .init();
        crc.update(list.items[start..]);
        crc.update(section.payload);
        try out.int(u32, crc.final());
        try out.bytes(section.payload);
    }
    std.debug.assert(list.items.len == total);
    return list.toOwnedSlice(allocator);
}

pub const Decoded = struct {
    header: Header,
    sections: [max_sections]Section = undefined,
    count: usize = 0,

    pub fn find(self: *const Decoded, tag: Tag) ?[]const u8 {
        for (self.sections[0..self.count]) |section| {
            if (section.tag == tag) return section.payload;
        }
        return null;
    }
};

/// Checks the framing of untrusted bytes: every checksum, the generation, and the pin before any
/// section is read. The result borrows `bytes`.
pub fn decode(bytes: []const u8, expected_pin: []const u8) DecodeError!Decoded {
    if (bytes.len > max_record_bytes) return error.RecordTooLarge;
    var in: wire.Reader = .init(bytes);
    if (!std.mem.eql(u8, &(try in.array(4)), magic)) return error.InvalidRecord;
    if (try in.int(u16) != generation) return error.UnsupportedGeneration;
    if (try in.int(u16) != 0) return error.InvalidRecord;
    const product_version = try in.counted(u8, max_product_version_bytes);
    const pin = try in.counted(u8, max_pin_bytes);
    const count = try in.int(u16);
    const header_end = in.offset;
    if (try in.int(u32) != Crc.hash(bytes[0..header_end])) return error.InvalidRecord;
    if (!printable(product_version, max_product_version_bytes) or !printable(pin, max_pin_bytes) or
        count == 0 or count > max_sections) return error.InvalidRecord;

    var result: Decoded = .{ .header = .{ .product_version = product_version, .pin = pin } };
    var previous: u16 = 0;
    for (0..count) |_| {
        const start = in.offset;
        const tag = try in.int(u16);
        const flags = try in.int(u16);
        const length = try in.int(u32);
        const declared = try in.int(u32);
        const payload = try in.take(length);
        var crc: Crc = .init();
        crc.update(bytes[start..][0..8]);
        crc.update(payload);
        if (crc.final() != declared) return error.InvalidRecord;
        if (tag <= previous or flags & ~required_flag != 0) return error.InvalidRecord;
        previous = tag;
        result.sections[result.count] = .{
            .tag = @enumFromInt(tag),
            .required = flags & required_flag != 0,
            .payload = payload,
        };
        result.count += 1;
    }
    try in.finish();

    // Corruption is judged first, so a damaged pin never reads as a foreign one.
    if (!std.mem.eql(u8, pin, expected_pin)) return error.ForeignPin;
    for (result.sections[0..result.count]) |section| switch (section.tag) {
        .terminal, .configuration, .screen, .engine, .console => {},
        _ => if (section.required) return error.UnknownRequiredSection,
    };
    return result;
}

fn printable(text: []const u8, maximum: usize) bool {
    if (text.len == 0 or text.len > maximum) return false;
    for (text) |byte| if (byte < 0x21 or byte > 0x7e) return false;
    return true;
}
