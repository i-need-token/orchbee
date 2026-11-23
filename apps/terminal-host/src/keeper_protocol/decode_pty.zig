//! The decoders of `keeper`'s POSIX messages, on the shared value rules of `decode.zig`.

const std = @import("std");

const handshake = @import("terminal-host-handshake");
const decode = @import("decode.zig");
const message = @import("message.zig");

const json = handshake.json;
const pty = message.pty;
const PayloadError = decode.PayloadError;
const Object = decode.Object;

pub fn holdPty(object: Object) PayloadError!pty.HoldPty {
    try decode.closed(object.members, &.{ "type", "id", "holding", "facts", "extensions" });
    return .{
        .holding = try decode.randomId(try object.required("holding")),
        .facts = try ptyFacts(try object.required("facts")),
    };
}

pub fn spawn(arena: std.mem.Allocator, object: Object) PayloadError!pty.Spawn {
    try decode.closed(object.members, &.{ "type", "id", "holding", "input", "launch", "facts", "extensions" });
    const facts_members = try decode.objectMembers(try object.required("facts"));
    try decode.closed(facts_members, &.{ "terminal", "slave", "launch" });
    return .{
        .holding = try decode.randomId(try object.required("holding")),
        .input = try boolean(try object.required("input")),
        .launch = try launch(arena, try object.required("launch")),
        .facts = .{
            .terminal = try decode.terminalValue(try decode.requiredMember(facts_members, "terminal")),
            .slave = try slave(try decode.requiredMember(facts_members, "slave")),
            .launch = try decode.name(try decode.requiredMember(facts_members, "launch")),
        },
    };
}

pub fn ptyFacts(value: json.Value) PayloadError!pty.PtyFacts {
    const members = try decode.objectMembers(value);
    try decode.closed(members, &.{ "terminal", "leader", "slave", "launch" });
    return .{
        .terminal = try decode.terminalValue(try decode.requiredMember(members, "terminal")),
        .leader = try decode.processValue(try decode.requiredMember(members, "leader")),
        .slave = try slave(try decode.requiredMember(members, "slave")),
        .launch = try decode.name(try decode.requiredMember(members, "launch")),
    };
}

/// A PTY holding's `facts`, `parented`, and optional `ended`, from the members of a status entry or
/// an `inspect-result`.
pub fn ptyHolding(members: []const json.Member) PayloadError!pty.PtyHolding {
    return .{
        .facts = try ptyFacts(try decode.requiredMember(members, "facts")),
        .parented = try boolean(try decode.requiredMember(members, "parented")),
        .ended = if (decode.lookup(members, "ended")) |ended| try exitStatus(ended) else null,
    };
}

pub fn inspection(object: Object) PayloadError!pty.PtyHolding {
    try decode.closed(object.members, &.{ "type", "id", "facts", "parented", "ended", "extensions" });
    return ptyHolding(object.members);
}

pub fn exited(object: Object) PayloadError!pty.Exited {
    try decode.closed(object.members, &.{ "type", "holding", "status", "extensions" });
    return .{
        .holding = try decode.randomId(try object.required("holding")),
        .status = try exitStatus(try object.required("status")),
    };
}

fn exitStatus(value: json.Value) PayloadError!pty.ExitStatus {
    const members = try decode.objectMembers(value);
    if (members.len != 1) return error.Message;
    if (decode.lookup(members, "code")) |code| return .{ .code = try decode.integer(u8, code, 0) };
    if (decode.lookup(members, "signal")) |signal| return .{ .signal = try decode.integer(u8, signal, 1) };
    return error.Message;
}

fn launch(arena: std.mem.Allocator, value: json.Value) PayloadError!pty.Launch {
    const members = try decode.objectMembers(value);
    try decode.closed(members, &.{ "executable", "arguments", "environment", "candidates", "directory" });
    const decoded: pty.Launch = .{
        .executable = try decode.string(try decode.requiredMember(members, "executable")),
        .arguments = try strings(arena, try decode.requiredMember(members, "arguments")),
        .environment = try strings(arena, try decode.requiredMember(members, "environment")),
        .candidates = try strings(arena, try decode.requiredMember(members, "candidates")),
        .directory = if (decode.lookup(members, "directory")) |directory| try decode.string(directory) else null,
    };
    if (!pty.validLaunch(decoded)) return error.Message;
    return decoded;
}

fn strings(arena: std.mem.Allocator, value: json.Value) PayloadError![]const []const u8 {
    const items = try decode.array(value);
    if (items.len > pty.maximum_launch_items) return error.Message;
    const decoded = try arena.alloc([]const u8, items.len);
    for (items, decoded) |item, *text| text.* = try decode.string(item);
    return decoded;
}

fn slave(value: json.Value) PayloadError![]const u8 {
    const text = try decode.string(value);
    if (!pty.validSlave(text)) return error.Message;
    return text;
}

fn boolean(value: json.Value) PayloadError!bool {
    return switch (value) {
        .boolean => |flag| flag,
        else => error.Message,
    };
}
