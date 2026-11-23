//! Direction-specific `OBKP` decoders. A payload that is not one strict JSON object is an
//! `Encoding` failure; a strict object that is not a valid message for its direction is a
//! `Message` failure. Every returned slice lives in the caller's arena.

const std = @import("std");

const handshake = @import("terminal-host-handshake");
const frame = @import("frame.zig");
const message = @import("message.zig");
const decode_pty = @import("decode_pty.zig");

const json = handshake.json;
const grammar = handshake.message;

pub const PayloadError = error{ Encoding, Message, OutOfMemory };
pub const FrameError = frame.DecodeError || PayloadError;

/// Decodes exactly one complete in-memory frame an initiator sends.
pub fn initiatorFrame(arena: std.mem.Allocator, bytes: []const u8) FrameError!message.InitiatorMessage {
    return initiatorPayload(arena, try frame.completePayload(bytes));
}

/// Decodes exactly one complete in-memory frame the Keeper sends.
pub fn acceptorFrame(arena: std.mem.Allocator, bytes: []const u8) FrameError!message.AcceptorMessage {
    return acceptorPayload(arena, try frame.completePayload(bytes));
}

pub fn initiatorPayload(arena: std.mem.Allocator, payload: []const u8) PayloadError!message.InitiatorMessage {
    const object = try messageObject(arena, payload);
    const id = try requestId(object.get("id") orelse return error.Message);
    const kind = message.RequestKind.fromName(object.type_name) orelse {
        if (isReplyName(object.type_name)) return error.Message;
        // An unknown request needs only a valid id; its other members are unread.
        return .{ .unknown_request = .{ .request = object.type_name, .id = id } };
    };
    const body: message.Request.Body = switch (kind) {
        .hello => blk: {
            try closed(object.members, &.{ "type", "id", "role", "extensions" });
            break :blk .{ .hello = message.Role.fromName(try name(try object.required("role"))) };
        },
        .register_host, .unregister_host => blk: {
            try closed(object.members, &.{ "type", "id", "process", "extensions" });
            const process = try processValue(try object.required("process"));
            break :blk if (kind == .register_host) .{ .register_host = process } else .{ .unregister_host = process };
        },
        .status => blk: {
            try closed(object.members, &.{ "type", "id", "after", "extensions" });
            break :blk .{ .status = if (object.get("after")) |after| try randomId(after) else null };
        },
        .hold => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "handles", "facts", "extensions" });
            break :blk .{ .hold = .{
                .holding = try randomId(try object.required("holding")),
                .handles = try handlesValue(try object.required("handles")),
                .facts = try factsValue(try object.required("facts")),
            } };
        },
        .mark, .assign => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "for", "mark", "extensions" });
            const mark: message.Mark = .{
                .holding = try randomId(try object.required("holding")),
                .target = try processValue(try object.required("for")),
                .mark = try randomId(try object.required("mark")),
            };
            break :blk if (kind == .mark) .{ .mark = mark } else .{ .assign = mark };
        },
        .@"resume", .inspect, .decline => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "mark", "extensions" });
            const marked: message.Marked = .{
                .holding = try randomId(try object.required("holding")),
                .mark = try randomId(try object.required("mark")),
            };
            break :blk switch (kind) {
                .@"resume" => .{ .@"resume" = marked },
                .inspect => .{ .inspect = marked },
                else => .{ .decline = marked },
            };
        },
        .claim => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "mark", "program", "terminal", "extensions" });
            break :blk .{ .claim = .{
                .holding = try randomId(try object.required("holding")),
                .mark = try randomId(try object.required("mark")),
                .program = try processValue(try object.required("program")),
                .terminal = if (object.get("terminal")) |terminal| try terminalValue(terminal) else null,
            } };
        },
        .hold_program => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "handles", "facts", "extensions" });
            break :blk .{ .hold_program = .{
                .holding = try randomId(try object.required("holding")),
                .handles = try programHandlesValue(try object.required("handles")),
                .facts = try programFactsValue(try object.required("facts")),
            } };
        },
        .release, .discard, .reap => blk: {
            try closed(object.members, &.{ "type", "id", "holding", "extensions" });
            const holding = try randomId(try object.required("holding"));
            break :blk switch (kind) {
                .release => .{ .release = holding },
                .discard => .{ .discard = holding },
                else => .{ .reap = holding },
            };
        },
        .hold_pty => .{ .hold_pty = try decode_pty.holdPty(object) },
        .spawn => .{ .spawn = try decode_pty.spawn(arena, object) },
    };
    return .{ .request = .{
        .id = id,
        .body = body,
        .extensions = try extensions(arena, object.get("extensions")),
    } };
}

pub fn acceptorPayload(arena: std.mem.Allocator, payload: []const u8) PayloadError!message.AcceptorMessage {
    const object = try messageObject(arena, payload);
    // A frame without an id is an event; the members of one this build does not know are unread.
    const id_value = object.get("id") orelse {
        if (std.mem.eql(u8, object.type_name, "exited")) return .{ .exited = try decode_pty.exited(object) };
        return .{ .event = object.type_name };
    };
    const id = try requestId(id_value);
    const body: message.Reply.Body = if (std.mem.eql(u8, object.type_name, "refused")) blk: {
        try closed(object.members, &.{ "type", "id", "reason", "extensions" });
        break :blk .{ .refused = message.Reason.fromName(try name(try object.required("reason"))) };
    } else if (std.mem.eql(u8, object.type_name, "unsupported")) blk: {
        try closed(object.members, &.{ "type", "id", "request", "extensions" });
        break :blk .{ .unsupported = try name(try object.required("request")) };
    } else blk: {
        const kind = message.RequestKind.fromResultName(object.type_name) orelse return error.Message;
        break :blk try resultBody(arena, object, kind);
    };
    return .{ .reply = .{
        .id = id,
        .body = body,
        .extensions = try extensions(arena, object.get("extensions")),
    } };
}

fn resultBody(arena: std.mem.Allocator, object: Object, kind: message.RequestKind) PayloadError!message.Reply.Body {
    switch (kind) {
        .status => {
            try closed(object.members, &.{ "type", "id", "holdings", "hosts", "next", "extensions" });
            const entries = try array(try object.required("holdings"));
            if (entries.len > message.maximum_status_entries) return error.Message;
            const holdings = try arena.alloc(message.HoldingEntry, entries.len);
            for (entries, holdings) |entry, *holding| holding.* = try holdingEntry(entry);
            const hosts: ?[]message.HostEntry = if (object.get("hosts")) |value| hosts: {
                const items = try array(value);
                if (items.len > message.maximum_status_entries) return error.Message;
                const decoded = try arena.alloc(message.HostEntry, items.len);
                for (items, decoded) |item, *host| host.* = try hostEntry(item);
                break :hosts decoded;
            } else null;
            const page: message.StatusPage = .{
                .holdings = holdings,
                .hosts = hosts,
                .next = if (object.get("next")) |next| try randomId(next) else null,
            };
            if (!page.valid()) return error.Message;
            return .{ .status = page };
        },
        .@"resume", .decline => {
            try closed(object.members, &.{ "type", "id", "outcome", "extensions" });
            const outcome = message.Outcome.fromName(try name(try object.required("outcome")));
            return if (kind == .@"resume") .{ .@"resume" = outcome } else .{ .decline = outcome };
        },
        .inspect => {
            // A PTY holding's master arrives beside the message, so its result names no handles.
            if (object.get("handles") == null) return .{ .inspect_pty = try decode_pty.inspection(object) };
            try closed(object.members, &.{ "type", "id", "handles", "facts", "extensions" });
            const handles = try object.required("handles");
            // A program holding's handles name its process where a terminal's name its program.
            if (lookup(try objectMembers(handles), "process") != null) return .{ .inspect_program = .{
                .handles = try programHandlesValue(handles),
                .facts = try programFactsValue(try object.required("facts")),
            } };
            return .{ .inspect = .{
                .handles = try handlesValue(handles),
                .facts = try factsValue(try object.required("facts")),
            } };
        },
        .spawn => {
            try closed(object.members, &.{ "type", "id", "leader", "extensions" });
            return .{ .spawn = try processValue(try object.required("leader")) };
        },
        inline .hello, .register_host, .unregister_host, .hold, .mark, .claim, .release, .assign, .discard, .hold_program, .hold_pty, .reap => |empty| {
            try closed(object.members, &.{ "type", "id", "extensions" });
            return @unionInit(message.Reply.Body, @tagName(empty), {});
        },
    }
}

fn isReplyName(text: []const u8) bool {
    return std.mem.eql(u8, text, "refused") or std.mem.eql(u8, text, "unsupported") or
        message.RequestKind.fromResultName(text) != null;
}

pub const Object = struct {
    members: []const json.Member,
    type_name: []const u8,

    pub fn get(self: Object, member_name: []const u8) ?json.Value {
        return lookup(self.members, member_name);
    }

    pub fn required(self: Object, member_name: []const u8) PayloadError!json.Value {
        return self.get(member_name) orelse error.Message;
    }
};

fn messageObject(arena: std.mem.Allocator, payload: []const u8) PayloadError!Object {
    const members = try json.parseObject(arena, payload);
    const type_name = try name(lookup(members, "type") orelse return error.Message);
    return .{ .members = members, .type_name = type_name };
}

fn holdingEntry(value: json.Value) PayloadError!message.HoldingEntry {
    const members = try objectMembers(value);
    const kind: ?[]const u8 = if (lookup(members, "kind")) |kind| try name(kind) else null;
    const is_pty = if (kind) |text| std.mem.eql(u8, text, "pty") else false;
    if (is_pty) {
        try closed(members, &.{ "holding", "owner", "state", "for", "kind", "facts", "parented", "ended" });
    } else {
        try closed(members, &.{ "holding", "owner", "state", "for", "kind", "facts" });
    }
    const facts = try requiredMember(members, "facts");
    return .{
        .holding = try randomId(try requiredMember(members, "holding")),
        .owner = try processValue(try requiredMember(members, "owner")),
        .state = message.HoldingState.fromName(try name(try requiredMember(members, "state"))),
        .target = if (lookup(members, "for")) |target| try processValue(target) else null,
        .facts = if (kind) |text| facts: {
            if (is_pty) break :facts .{ .pty = try decode_pty.ptyHolding(members) };
            if (!std.mem.eql(u8, text, "program")) return error.Message;
            break :facts .{ .program = try programFactsValue(facts) };
        } else .{ .terminal = try factsValue(facts) },
    };
}

fn programHandlesValue(value: json.Value) PayloadError!message.ProgramHandles {
    const members = try objectMembers(value);
    try closed(members, &.{ "job", "process", "stdout", "stderr" });
    return .{
        .job = try handleValue(try requiredMember(members, "job")),
        .process = try handleValue(try requiredMember(members, "process")),
        .stdout = if (lookup(members, "stdout")) |handle| try handleValue(handle) else null,
        .stderr = if (lookup(members, "stderr")) |handle| try handleValue(handle) else null,
    };
}

fn handleValue(value: json.Value) PayloadError!u32 {
    const handle = try hexInteger(u64, value);
    if (handle == 0 or handle > std.math.maxInt(u32)) return error.Message;
    return @intCast(handle);
}

fn programFactsValue(value: json.Value) PayloadError!message.ProgramFacts {
    const members = try objectMembers(value);
    try closed(members, &.{ "process", "programId", "label" });
    const facts: message.ProgramFacts = .{
        .process = try processValue(try requiredMember(members, "process")),
        .program_id = try hexBytes(16, try requiredMember(members, "programId")),
        .label = try string(try requiredMember(members, "label")),
    };
    if (!facts.valid()) return error.Message;
    return facts;
}

fn hostEntry(value: json.Value) PayloadError!message.HostEntry {
    const members = try objectMembers(value);
    try closed(members, &.{ "process", "ended" });
    return .{
        .process = try processValue(try requiredMember(members, "process")),
        .ended = switch (try requiredMember(members, "ended")) {
            .boolean => |flag| flag,
            else => return error.Message,
        },
    };
}

pub fn processValue(value: json.Value) PayloadError!message.Process {
    const members = try objectMembers(value);
    try closed(members, &.{ "pid", "created" });
    const created = try hexInteger(u64, try requiredMember(members, "created"));
    if (created == 0) return error.Message;
    return .{ .pid = try integer(u32, try requiredMember(members, "pid"), 1), .created = created };
}

fn handlesValue(value: json.Value) PayloadError!message.Handles {
    const members = try objectMembers(value);
    try closed(members, &message.Handles.roles);
    var handles: message.Handles = undefined;
    inline for (message.Handles.roles) |role| {
        const handle = try hexInteger(u64, try requiredMember(members, role));
        if (handle == 0 or handle > std.math.maxInt(u32)) return error.Message;
        @field(handles, role) = @intCast(handle);
    }
    return handles;
}

pub fn terminalValue(value: json.Value) PayloadError!message.Terminal {
    const members = try objectMembers(value);
    try closed(members, &.{ "creationId", "hostEpoch", "terminalId" });
    return .{
        .creation_id = try hexBytes(16, try requiredMember(members, "creationId")),
        .host_epoch = try hexBytes(16, try requiredMember(members, "hostEpoch")),
        .terminal_id = try hexBytes(16, try requiredMember(members, "terminalId")),
    };
}

fn factsValue(value: json.Value) PayloadError!message.Facts {
    const members = try objectMembers(value);
    try closed(members, &.{ "terminal", "program", "console", "consoleBuild", "signalFormat", "launch" });
    return .{
        .terminal = try terminalValue(try requiredMember(members, "terminal")),
        .program = try processValue(try requiredMember(members, "program")),
        .console = try processValue(try requiredMember(members, "console")),
        .console_build = try hexBytes(32, try requiredMember(members, "consoleBuild")),
        .signal_format = try integer(u16, try requiredMember(members, "signalFormat"), 1),
        .launch = try name(try requiredMember(members, "launch")),
    };
}

pub fn randomId(value: json.Value) PayloadError!message.RandomId {
    const bytes = try hexBytes(16, value);
    if (message.isZero(&bytes)) return error.Message;
    return bytes;
}

fn requestId(value: json.Value) PayloadError!u32 {
    return integer(u32, value, 1);
}

pub fn lookup(members: []const json.Member, member_name: []const u8) ?json.Value {
    for (members) |member| if (std.mem.eql(u8, member.name, member_name)) return member.value;
    return null;
}

pub fn requiredMember(members: []const json.Member, member_name: []const u8) PayloadError!json.Value {
    return lookup(members, member_name) orelse error.Message;
}

/// Refuses a member outside `allowed`; strict JSON already refused repeats.
pub fn closed(members: []const json.Member, comptime allowed: []const []const u8) PayloadError!void {
    for (members) |member| if (!contains(allowed, member.name)) return error.Message;
}

fn contains(comptime allowed: []const []const u8, member_name: []const u8) bool {
    inline for (allowed) |candidate| if (std.mem.eql(u8, candidate, member_name)) return true;
    return false;
}

pub fn objectMembers(value: json.Value) PayloadError![]const json.Member {
    return switch (value) {
        .object => |members| members,
        else => error.Message,
    };
}

pub fn array(value: json.Value) PayloadError![]const json.Value {
    return switch (value) {
        .array => |items| items,
        else => error.Message,
    };
}

pub fn string(value: json.Value) PayloadError![]const u8 {
    return switch (value) {
        .string => |text| text,
        else => error.Message,
    };
}

pub fn name(value: json.Value) PayloadError![]const u8 {
    const text = try string(value);
    if (!grammar.isName(text)) return error.Message;
    return text;
}

/// Exactly `length * 2` lowercase hexadecimal digits.
fn hexBytes(comptime length: usize, value: json.Value) PayloadError![length]u8 {
    const text = try string(value);
    if (!grammar.isLowercaseHex(text, length * 2)) return error.Message;
    var bytes: [length]u8 = undefined;
    _ = std.fmt.hexToBytes(&bytes, text) catch return error.Message;
    return bytes;
}

/// Exactly `@bitSizeOf(Int) / 4` lowercase hexadecimal digits, read as one big-endian integer.
fn hexInteger(comptime Int: type, value: json.Value) PayloadError!Int {
    const bytes = try hexBytes(@sizeOf(Int), value);
    return std.mem.readInt(Int, &bytes, .big);
}

/// A plain integer token, without fraction, exponent, or sign, from `minimum` to `Int`'s maximum.
pub fn integer(comptime Int: type, value: json.Value, minimum: Int) PayloadError!Int {
    const text = switch (value) {
        .number => |token| token,
        else => return error.Message,
    };
    for (text) |byte| if (byte < '0' or byte > '9') return error.Message;
    const parsed = std.fmt.parseInt(Int, text, 10) catch return error.Message;
    if (parsed < minimum) return error.Message;
    return parsed;
}

/// The only open slot: names follow the name grammar and values are any strict JSON.
fn extensions(arena: std.mem.Allocator, value: ?json.Value) PayloadError!message.Extensions {
    const members = try objectMembers(value orelse return &.{});
    for (members) |member| if (!grammar.isName(member.name)) return error.Message;
    const sorted = try arena.dupe(json.Member, members);
    std.mem.sort(json.Member, sorted, {}, memberLessThan);
    return sorted;
}

fn memberLessThan(_: void, left: json.Member, right: json.Member) bool {
    return std.mem.lessThan(u8, left.name, right.name);
}
