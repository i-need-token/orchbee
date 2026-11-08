//! Direction-specific `OBHS` encoders. Each refuses a message that breaks its grammar, so an
//! encoded frame always decodes in its direction. Members appear in the contract's order, absent
//! optional members are omitted, and `extensions` is written only when it has members.

const std = @import("std");

const frame = @import("frame.zig");
const json = @import("json.zig");
const message = @import("message.zig");

pub const EncodeError = frame.EncodeError || error{InvalidMessage};

/// Encodes one caller-owned frame an acceptor sends.
pub fn acceptorFrame(allocator: std.mem.Allocator, value: message.AcceptorMessage) EncodeError![]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    writeAcceptor(&output.writer, value) catch |err| return mapWriteError(err);
    return frame.encode(allocator, output.written());
}

/// Encodes one caller-owned frame an initiator sends.
pub fn initiatorFrame(allocator: std.mem.Allocator, value: message.InitiatorMessage) EncodeError![]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    writeInitiator(&output.writer, value) catch |err| return mapWriteError(err);
    return frame.encode(allocator, output.written());
}

const WriteError = std.Io.Writer.Error || error{InvalidMessage};

fn mapWriteError(err: WriteError) EncodeError {
    return switch (err) {
        error.WriteFailed => error.OutOfMemory,
        error.InvalidMessage => error.InvalidMessage,
    };
}

const Object = struct {
    writer: *std.Io.Writer,
    empty: bool = true,

    fn begin(writer: *std.Io.Writer) WriteError!Object {
        try writer.writeByte('{');
        return .{ .writer = writer };
    }

    fn end(self: *Object) WriteError!void {
        try self.writer.writeByte('}');
    }

    fn key(self: *Object, member_name: []const u8) WriteError!void {
        if (!self.empty) try self.writer.writeByte(',');
        self.empty = false;
        try writeString(self.writer, member_name);
        try self.writer.writeByte(':');
    }

    fn text(self: *Object, member_name: []const u8, value: []const u8) WriteError!void {
        try self.key(member_name);
        try writeString(self.writer, value);
    }

    fn integer(self: *Object, member_name: []const u8, value: anytype) WriteError!void {
        try self.key(member_name);
        try self.writer.print("{d}", .{value});
    }
};

fn writeInitiator(writer: *std.Io.Writer, value: message.InitiatorMessage) WriteError!void {
    var object = try Object.begin(writer);
    switch (value) {
        .offer => |offer| {
            if (!message.isName(offer.connection)) return error.InvalidMessage;
            if (!message.isEnvironment(offer.environment, offer.fingerprint)) return error.InvalidMessage;
            try object.text("type", message.MessageType.offer.name());
            try object.text("connection", offer.connection);
            try writeGenerations(&object, "generations", offer.generations);
            try writeCapabilities(&object, offer.capabilities);
            try writeVersion(&object, offer.product_version);
            try writeBuild(&object, offer.build);
            try object.text("environment", offer.environment);
            if (offer.fingerprint) |fingerprint| try object.text("fingerprint", fingerprint);
            try writeExtensions(&object, offer.extensions);
        },
        .request => |request| {
            try object.text("type", request.kind.name());
            try writeName(&object, "environment", request.environment);
            try writeExtensions(&object, request.extensions);
        },
        .unknown_request => |request| {
            if (message.MessageType.fromName(request.request) != null) return error.InvalidMessage;
            try writeName(&object, "type", request.request);
            try writeName(&object, "environment", request.environment);
        },
    }
    try object.end();
}

fn writeAcceptor(writer: *std.Io.Writer, value: message.AcceptorMessage) WriteError!void {
    var object = try Object.begin(writer);
    switch (value) {
        .answer => |answer| {
            if (answer.generation < 1) return error.InvalidMessage;
            try object.text("type", message.MessageType.answer.name());
            try object.integer("generation", answer.generation);
            try writeCapabilities(&object, answer.capabilities);
            try writeVersion(&object, answer.product_version);
            try writeBuild(&object, answer.build);
            try writeExtensions(&object, answer.extensions);
        },
        .rejection => |rejection| {
            try object.text("type", message.MessageType.rejection.name());
            try object.text("reason", rejection.reason.name());
            try writeGenerations(&object, "generations", rejection.generations);
            try writeVersion(&object, rejection.product_version);
            try writeName(&object, "environment", rejection.environment);
            try writeExtensions(&object, rejection.extensions);
        },
        .query_result => |result| {
            if (!result.valid()) return error.InvalidMessage;
            try object.text("type", message.MessageType.query_result.name());
            try object.key("processes");
            try writer.writeByte('[');
            for (result.processes, 0..) |process, index| {
                if (index > 0) try writer.writeByte(',');
                try writeProcess(writer, process);
            }
            try writer.writeByte(']');
            if (result.restart_keeps_terminals) |keeps| {
                try object.key("restartKeepsTerminals");
                try writer.writeAll(if (keeps) "true" else "false");
            }
            try writeExtensions(&object, result.extensions);
        },
        .drain_result => |result| {
            try object.text("type", message.MessageType.drain_result.name());
            try object.text("state", result.state.name());
            try writeExtensions(&object, result.extensions);
        },
        .exit_result => |result| {
            try object.text("type", message.MessageType.exit_result.name());
            try writeExtensions(&object, result.extensions);
        },
        .unsupported => |reply| {
            try object.text("type", message.MessageType.unsupported.name());
            try writeName(&object, "request", reply.request);
            try writeExtensions(&object, reply.extensions);
        },
    }
    try object.end();
}

fn writeProcess(writer: *std.Io.Writer, process: message.ProcessEntry) WriteError!void {
    var object = try Object.begin(writer);
    try object.text("role", process.role.name());
    try writeVersion(&object, process.product_version);
    try writeBuild(&object, process.build);
    try writeName(&object, "environment", process.environment);
    try object.integer("pid", process.pid);
    try object.text("state", process.state.name());
    try object.integer("terminals", process.terminals);
    try object.key("connections");
    try writer.writeByte('[');
    for (process.connections, 0..) |connection, index| {
        if (index > 0) try writer.writeByte(',');
        var entry = try Object.begin(writer);
        try writeName(&entry, "kind", connection.kind);
        try writeGenerations(&entry, "generations", connection.generations);
        if (connection.capabilities) |names| try writeCapabilities(&entry, names);
        try entry.end();
    }
    try writer.writeByte(']');
    try object.end();
}

fn writeName(object: *Object, member_name: []const u8, value: []const u8) WriteError!void {
    if (!message.isName(value)) return error.InvalidMessage;
    try object.text(member_name, value);
}

fn writeVersion(object: *Object, value: []const u8) WriteError!void {
    if (!message.isProductVersion(value)) return error.InvalidMessage;
    try object.text("productVersion", value);
}

fn writeGenerations(object: *Object, member_name: []const u8, range: message.GenerationRange) WriteError!void {
    if (!range.valid()) return error.InvalidMessage;
    try object.key(member_name);
    var nested = try Object.begin(object.writer);
    try nested.integer("lowest", range.lowest);
    try nested.integer("highest", range.highest);
    try nested.end();
}

fn writeCapabilities(object: *Object, names: []const []const u8) WriteError!void {
    if (!message.isCapabilityList(names)) return error.InvalidMessage;
    try object.key("capabilities");
    try object.writer.writeByte('[');
    for (names, 0..) |capability, index| {
        if (index > 0) try object.writer.writeByte(',');
        try writeString(object.writer, capability);
    }
    try object.writer.writeByte(']');
}

fn writeBuild(object: *Object, build: message.Build) WriteError!void {
    if (!message.isBuild(build)) return error.InvalidMessage;
    try object.key("build");
    var nested = try Object.begin(object.writer);
    if (build.commit) |commit| try nested.text("commit", commit);
    if (build.cohort) |cohort| try nested.text("cohort", cohort);
    try nested.end();
}

fn writeExtensions(object: *Object, members: message.Extensions) WriteError!void {
    if (members.len == 0) return;
    for (members, 0..) |member, index| {
        if (!message.isName(member.name)) return error.InvalidMessage;
        if (index > 0 and !std.mem.lessThan(u8, members[index - 1].name, member.name)) {
            return error.InvalidMessage;
        }
        // The extensions object is depth 2, so its values may nest 30 more levels.
        if (2 + json.depth(member.value) > json.maximum_depth) return error.InvalidMessage;
    }
    try object.key("extensions");
    try writeValue(object.writer, .{ .object = members });
}

fn writeValue(writer: *std.Io.Writer, value: json.Value) WriteError!void {
    switch (value) {
        .null => try writer.writeAll("null"),
        .boolean => |flag| try writer.writeAll(if (flag) "true" else "false"),
        .number => |token| try writer.writeAll(token),
        .string => |text| try writeString(writer, text),
        .array => |items| {
            try writer.writeByte('[');
            for (items, 0..) |item, index| {
                if (index > 0) try writer.writeByte(',');
                try writeValue(writer, item);
            }
            try writer.writeByte(']');
        },
        .object => |members| {
            var object = try Object.begin(writer);
            for (members) |member| {
                try object.key(member.name);
                try writeValue(writer, member.value);
            }
            try object.end();
        },
    }
}

fn writeString(writer: *std.Io.Writer, text: []const u8) WriteError!void {
    try writer.writeByte('"');
    for (text) |byte| switch (byte) {
        '"' => try writer.writeAll("\\\""),
        '\\' => try writer.writeAll("\\\\"),
        '\n' => try writer.writeAll("\\n"),
        '\r' => try writer.writeAll("\\r"),
        '\t' => try writer.writeAll("\\t"),
        0x08 => try writer.writeAll("\\b"),
        0x0c => try writer.writeAll("\\f"),
        0x00...0x07, 0x0b, 0x0e...0x1f => try writer.print("\\u{x:0>4}", .{byte}),
        else => try writer.writeByte(byte),
    };
    try writer.writeByte('"');
}
