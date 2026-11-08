//! Strict payload JSON: one RFC 8259 object of valid UTF-8 with optional surrounding whitespace,
//! no repeated member at any depth, at most `maximum_depth` nested arrays and objects counting the
//! message object as depth 1, finite binary64 numbers, and no lone surrogate escape. The
//! standard scanner supplies the RFC syntax, UTF-8 strings, and surrogate pairing; this layer adds
//! the remaining rules and the output tree. Integer spelling is a member rule for the decoders.

const std = @import("std");

pub const maximum_depth = 32;

pub const Value = union(enum) {
    null,
    boolean: bool,
    /// The exact number token, already proved finite.
    number: []const u8,
    string: []const u8,
    array: []const Value,
    object: []const Member,
};

pub const Member = struct {
    name: []const u8,
    value: Value,
};

pub const ParseError = error{ Encoding, OutOfMemory };

/// Parses one strict object payload; every returned slice lives in `arena`.
pub fn parseObject(arena: std.mem.Allocator, payload: []const u8) ParseError![]const Member {
    if (!std.unicode.utf8ValidateSlice(payload)) return error.Encoding;
    var scanner = std.json.Scanner.initCompleteInput(arena, payload);
    if (try next(&scanner, arena) != .object_begin) return error.Encoding;
    const members = try parseMembers(&scanner, arena, 1);
    if (try next(&scanner, arena) != .end_of_document) return error.Encoding;
    return members;
}

/// The nesting depth of `value` alone: 0 for a scalar, 1 for a flat array or object.
pub fn depth(value: Value) usize {
    return switch (value) {
        .null, .boolean, .number, .string => 0,
        .array => |items| blk: {
            var deepest: usize = 0;
            for (items) |item| deepest = @max(deepest, depth(item));
            break :blk deepest + 1;
        },
        .object => |members| blk: {
            var deepest: usize = 0;
            for (members) |member| deepest = @max(deepest, depth(member.value));
            break :blk deepest + 1;
        },
    };
}

fn next(scanner: *std.json.Scanner, arena: std.mem.Allocator) ParseError!std.json.Token {
    return scanner.nextAllocMax(arena, .alloc_always, std.json.default_max_value_len) catch |err| switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        else => error.Encoding,
    };
}

/// `container_depth` is the depth a container at this position would have.
fn parseValue(
    scanner: *std.json.Scanner,
    arena: std.mem.Allocator,
    token: std.json.Token,
    container_depth: usize,
) ParseError!Value {
    return switch (token) {
        .object_begin => .{ .object = try parseMembers(scanner, arena, container_depth) },
        .array_begin => .{ .array = try parseItems(scanner, arena, container_depth) },
        .true => .{ .boolean = true },
        .false => .{ .boolean = false },
        .null => .null,
        .allocated_number => |text| number: {
            const value = std.fmt.parseFloat(f64, text) catch return error.Encoding;
            if (!std.math.isFinite(value)) return error.Encoding;
            break :number .{ .number = text };
        },
        .allocated_string => |text| .{ .string = text },
        else => error.Encoding,
    };
}

fn parseMembers(
    scanner: *std.json.Scanner,
    arena: std.mem.Allocator,
    container_depth: usize,
) ParseError![]const Member {
    if (container_depth > maximum_depth) return error.Encoding;
    var members: std.ArrayList(Member) = .empty;
    while (true) {
        switch (try next(scanner, arena)) {
            .object_end => break,
            .allocated_string => |name| {
                const token = try next(scanner, arena);
                const value = try parseValue(scanner, arena, token, container_depth + 1);
                try members.append(arena, .{ .name = name, .value = value });
            },
            else => return error.Encoding,
        }
    }
    try rejectRepeatedNames(arena, members.items);
    return members.items;
}

fn parseItems(
    scanner: *std.json.Scanner,
    arena: std.mem.Allocator,
    container_depth: usize,
) ParseError![]const Value {
    if (container_depth > maximum_depth) return error.Encoding;
    var items: std.ArrayList(Value) = .empty;
    while (true) {
        const token = try next(scanner, arena);
        if (token == .array_end) break;
        try items.append(arena, try parseValue(scanner, arena, token, container_depth + 1));
    }
    return items.items;
}

fn rejectRepeatedNames(arena: std.mem.Allocator, members: []const Member) ParseError!void {
    if (members.len < 2) return;
    const names = try arena.alloc([]const u8, members.len);
    for (members, names) |member, *name| name.* = member.name;
    std.mem.sort([]const u8, names, {}, lessThan);
    for (names[1..], names[0 .. names.len - 1]) |later, earlier| {
        if (std.mem.eql(u8, earlier, later)) return error.Encoding;
    }
}

pub fn lessThan(_: void, left: []const u8, right: []const u8) bool {
    return std.mem.lessThan(u8, left, right);
}
