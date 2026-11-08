//! Direction-specific `OBHS` decoders. A payload that is not one strict JSON object is an
//! `Encoding` failure; a strict object that is not a valid message for its direction is a
//! `Message` failure. Every returned slice lives in the caller's arena.

const std = @import("std");

const frame = @import("frame.zig");
const json = @import("json.zig");
const message = @import("message.zig");

const GenerationRange = message.GenerationRange;

pub const PayloadError = error{ Encoding, Message, OutOfMemory };
pub const FrameError = frame.DecodeError || PayloadError;

/// Decodes exactly one complete in-memory frame an initiator sends.
pub fn initiatorFrame(arena: std.mem.Allocator, bytes: []const u8) FrameError!message.InitiatorMessage {
    return initiatorPayload(arena, try frame.completePayload(bytes));
}

/// Decodes exactly one complete in-memory frame an acceptor sends.
pub fn acceptorFrame(arena: std.mem.Allocator, bytes: []const u8) FrameError!message.AcceptorMessage {
    return acceptorPayload(arena, try frame.completePayload(bytes));
}

pub fn initiatorPayload(arena: std.mem.Allocator, payload: []const u8) PayloadError!message.InitiatorMessage {
    const object = try messageObject(arena, payload);
    const kind = message.MessageType.fromName(object.type_name) orelse {
        // An unknown request needs only a valid environment; its other members are unread.
        const environment = try name(object.get("environment") orelse return error.Message);
        return .{ .unknown_request = .{ .request = object.type_name, .environment = environment } };
    };
    return switch (kind) {
        .offer => .{ .offer = try offer(arena, object) },
        .query => .{ .request = try request(arena, object, .query) },
        .drain => .{ .request = try request(arena, object, .drain) },
        .exit => .{ .request = try request(arena, object, .exit) },
        .answer, .rejection, .query_result, .drain_result, .exit_result, .unsupported => error.Message,
    };
}

pub fn acceptorPayload(arena: std.mem.Allocator, payload: []const u8) PayloadError!message.AcceptorMessage {
    const object = try messageObject(arena, payload);
    const kind = message.MessageType.fromName(object.type_name) orelse return error.Message;
    switch (kind) {
        .answer => {
            try closed(object.members, &.{ "type", "generation", "capabilities", "productVersion", "build", "extensions" });
            return .{ .answer = .{
                .generation = try integer(u16, try object.required("generation"), 1),
                .capabilities = try capabilities(arena, try object.required("capabilities")),
                .product_version = try productVersion(try object.required("productVersion")),
                .build = try build(try object.required("build")),
                .extensions = try extensions(arena, object.get("extensions")),
            } };
        },
        .rejection => {
            try closed(object.members, &.{ "type", "reason", "generations", "productVersion", "environment", "extensions" });
            return .{ .rejection = .{
                .reason = message.RejectionReason.fromName(try name(try object.required("reason"))),
                .generations = try generations(try object.required("generations")),
                .product_version = try productVersion(try object.required("productVersion")),
                .environment = try name(try object.required("environment")),
                .extensions = try extensions(arena, object.get("extensions")),
            } };
        },
        .query_result => return .{ .query_result = try queryResult(arena, object) },
        .drain_result => {
            try closed(object.members, &.{ "type", "state", "extensions" });
            return .{ .drain_result = .{
                .state = message.ProcessState.fromName(try name(try object.required("state"))),
                .extensions = try extensions(arena, object.get("extensions")),
            } };
        },
        .exit_result => {
            try closed(object.members, &.{ "type", "extensions" });
            return .{ .exit_result = .{ .extensions = try extensions(arena, object.get("extensions")) } };
        },
        .unsupported => {
            try closed(object.members, &.{ "type", "request", "extensions" });
            return .{ .unsupported = .{
                .request = try name(try object.required("request")),
                .extensions = try extensions(arena, object.get("extensions")),
            } };
        },
        .offer, .query, .drain, .exit => return error.Message,
    }
}

const Object = struct {
    members: []const json.Member,
    type_name: []const u8,

    fn get(self: Object, member_name: []const u8) ?json.Value {
        return lookup(self.members, member_name);
    }

    fn required(self: Object, member_name: []const u8) PayloadError!json.Value {
        return self.get(member_name) orelse error.Message;
    }
};

fn messageObject(arena: std.mem.Allocator, payload: []const u8) PayloadError!Object {
    const members = try json.parseObject(arena, payload);
    const type_name = try name(lookup(members, "type") orelse return error.Message);
    return .{ .members = members, .type_name = type_name };
}

fn offer(arena: std.mem.Allocator, object: Object) PayloadError!message.Offer {
    try closed(object.members, &.{ "type", "connection", "generations", "capabilities", "productVersion", "build", "environment", "fingerprint", "extensions" });
    const environment = try name(try object.required("environment"));
    const fingerprint = if (object.get("fingerprint")) |value|
        try hex(value, message.fingerprint_hex_digits)
    else
        null;
    if (!message.isEnvironment(environment, fingerprint)) return error.Message;
    return .{
        .connection = try name(try object.required("connection")),
        .generations = try generations(try object.required("generations")),
        .capabilities = try capabilities(arena, try object.required("capabilities")),
        .product_version = try productVersion(try object.required("productVersion")),
        .build = try build(try object.required("build")),
        .environment = environment,
        .fingerprint = fingerprint,
        .extensions = try extensions(arena, object.get("extensions")),
    };
}

fn request(arena: std.mem.Allocator, object: Object, kind: message.RequestKind) PayloadError!message.ControlRequest {
    try closed(object.members, &.{ "type", "environment", "extensions" });
    return .{
        .kind = kind,
        .environment = try name(try object.required("environment")),
        .extensions = try extensions(arena, object.get("extensions")),
    };
}

fn queryResult(arena: std.mem.Allocator, object: Object) PayloadError!message.QueryResult {
    try closed(object.members, &.{ "type", "processes", "restartKeepsTerminals", "extensions" });
    const entries = try array(try object.required("processes"));
    if (entries.len > message.maximum_query_processes) return error.Message;
    const processes = try arena.alloc(message.ProcessEntry, entries.len);
    for (entries, processes) |entry, *process| process.* = try processEntry(arena, entry);
    const result: message.QueryResult = .{
        .processes = processes,
        .restart_keeps_terminals = if (object.get("restartKeepsTerminals")) |value| try boolean(value) else null,
        .extensions = try extensions(arena, object.get("extensions")),
    };
    if (!result.valid()) return error.Message;
    return result;
}

fn processEntry(arena: std.mem.Allocator, value: json.Value) PayloadError!message.ProcessEntry {
    const members = try objectMembers(value);
    try closed(members, &.{ "role", "productVersion", "build", "environment", "pid", "state", "terminals", "connections" });
    const entries = try array(try requiredMember(members, "connections"));
    if (entries.len > message.maximum_process_connections) return error.Message;
    const connections = try arena.alloc(message.ConnectionEntry, entries.len);
    for (entries, connections) |entry, *connection| {
        const fields = try objectMembers(entry);
        try closed(fields, &.{ "kind", "generations", "capabilities" });
        connection.* = .{
            .kind = try name(try requiredMember(fields, "kind")),
            .generations = try generations(try requiredMember(fields, "generations")),
            .capabilities = if (lookup(fields, "capabilities")) |names| try capabilities(arena, names) else null,
        };
    }
    return .{
        .role = message.ProcessRole.fromName(try name(try requiredMember(members, "role"))),
        .product_version = try productVersion(try requiredMember(members, "productVersion")),
        .build = try build(try requiredMember(members, "build")),
        .environment = try name(try requiredMember(members, "environment")),
        .pid = try integer(u32, try requiredMember(members, "pid"), 1),
        .state = message.ProcessState.fromName(try name(try requiredMember(members, "state"))),
        .terminals = try integer(u32, try requiredMember(members, "terminals"), 0),
        .connections = connections,
    };
}

fn lookup(members: []const json.Member, member_name: []const u8) ?json.Value {
    for (members) |member| if (std.mem.eql(u8, member.name, member_name)) return member.value;
    return null;
}

fn requiredMember(members: []const json.Member, member_name: []const u8) PayloadError!json.Value {
    return lookup(members, member_name) orelse error.Message;
}

/// Refuses a member outside `allowed`; strict JSON already refused repeats.
fn closed(members: []const json.Member, comptime allowed: []const []const u8) PayloadError!void {
    for (members) |member| if (!contains(allowed, member.name)) return error.Message;
}

fn contains(comptime allowed: []const []const u8, member_name: []const u8) bool {
    inline for (allowed) |candidate| if (std.mem.eql(u8, candidate, member_name)) return true;
    return false;
}

fn objectMembers(value: json.Value) PayloadError![]const json.Member {
    return switch (value) {
        .object => |members| members,
        else => error.Message,
    };
}

fn array(value: json.Value) PayloadError![]const json.Value {
    return switch (value) {
        .array => |items| items,
        else => error.Message,
    };
}

fn string(value: json.Value) PayloadError![]const u8 {
    return switch (value) {
        .string => |text| text,
        else => error.Message,
    };
}

fn boolean(value: json.Value) PayloadError!bool {
    return switch (value) {
        .boolean => |flag| flag,
        else => error.Message,
    };
}

fn name(value: json.Value) PayloadError![]const u8 {
    const text = try string(value);
    if (!message.isName(text)) return error.Message;
    return text;
}

fn productVersion(value: json.Value) PayloadError![]const u8 {
    const text = try string(value);
    if (!message.isProductVersion(text)) return error.Message;
    return text;
}

fn hex(value: json.Value, digits: usize) PayloadError![]const u8 {
    const text = try string(value);
    if (!message.isLowercaseHex(text, digits)) return error.Message;
    return text;
}

/// A plain integer token, without fraction, exponent, or sign, from `minimum` to `Int`'s maximum.
fn integer(comptime Int: type, value: json.Value, minimum: Int) PayloadError!Int {
    const text = switch (value) {
        .number => |token| token,
        else => return error.Message,
    };
    for (text) |byte| if (byte < '0' or byte > '9') return error.Message;
    const parsed = std.fmt.parseInt(Int, text, 10) catch return error.Message;
    if (parsed < minimum) return error.Message;
    return parsed;
}

fn generations(value: json.Value) PayloadError!GenerationRange {
    const members = try objectMembers(value);
    try closed(members, &.{ "lowest", "highest" });
    const range: GenerationRange = .{
        .lowest = try integer(u16, try requiredMember(members, "lowest"), 1),
        .highest = try integer(u16, try requiredMember(members, "highest"), 1),
    };
    if (!range.valid()) return error.Message;
    return range;
}

fn capabilities(arena: std.mem.Allocator, value: json.Value) PayloadError![]const []const u8 {
    const items = try array(value);
    if (items.len > message.maximum_capabilities) return error.Message;
    const names = try arena.alloc([]const u8, items.len);
    for (items, names) |item, *capability| capability.* = try string(item);
    if (!message.isCapabilityList(names)) return error.Message;
    return names;
}

fn build(value: json.Value) PayloadError!message.Build {
    const members = try objectMembers(value);
    try closed(members, &.{ "commit", "cohort" });
    return .{
        .commit = if (lookup(members, "commit")) |commit| try hex(commit, message.commit_hex_digits) else null,
        .cohort = if (lookup(members, "cohort")) |cohort| try hex(cohort, message.cohort_hex_digits) else null,
    };
}

/// The only open slot: names follow the name grammar and values are any strict JSON.
fn extensions(arena: std.mem.Allocator, value: ?json.Value) PayloadError!message.Extensions {
    const members = try objectMembers(value orelse return &.{});
    for (members) |member| if (!message.isName(member.name)) return error.Message;
    const sorted = try arena.dupe(json.Member, members);
    std.mem.sort(json.Member, sorted, {}, memberLessThan);
    return sorted;
}

fn memberLessThan(_: void, left: json.Member, right: json.Member) bool {
    return std.mem.lessThan(u8, left.name, right.name);
}
