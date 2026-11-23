//! Direction-specific `OBKP` encoders. Each refuses a message that breaks its grammar, so an
//! encoded frame always decodes in its direction. Members appear in the contract's order, absent
//! optional members are omitted, and `extensions` is written only when it has members.

const std = @import("std");

const handshake = @import("terminal-host-handshake");
const frame = @import("frame.zig");
const message = @import("message.zig");

const json = handshake.json;
const grammar = handshake.message;

pub const EncodeError = frame.EncodeError || error{InvalidMessage};

/// Encodes one caller-owned frame an initiator sends.
pub fn initiatorFrame(allocator: std.mem.Allocator, value: message.InitiatorMessage) EncodeError![]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    writeInitiator(&output.writer, value) catch |err| return mapWriteError(err);
    return frame.encode(allocator, output.written());
}

/// Encodes one caller-owned frame the Keeper sends.
pub fn acceptorFrame(allocator: std.mem.Allocator, value: message.AcceptorMessage) EncodeError![]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    writeAcceptor(&output.writer, value) catch |err| return mapWriteError(err);
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

    fn name(self: *Object, member_name: []const u8, value: []const u8) WriteError!void {
        if (!grammar.isName(value)) return error.InvalidMessage;
        try self.text(member_name, value);
    }

    fn integer(self: *Object, member_name: []const u8, value: anytype) WriteError!void {
        try self.key(member_name);
        try self.writer.print("{d}", .{value});
    }

    fn id(self: *Object, value: u32) WriteError!void {
        if (value == 0) return error.InvalidMessage;
        try self.integer("id", value);
    }

    fn hex(self: *Object, member_name: []const u8, bytes: []const u8) WriteError!void {
        try self.key(member_name);
        try self.writer.writeByte('"');
        for (bytes) |byte| try self.writer.print("{x:0>2}", .{byte});
        try self.writer.writeByte('"');
    }

    fn randomId(self: *Object, member_name: []const u8, value: message.RandomId) WriteError!void {
        if (message.isZero(&value)) return error.InvalidMessage;
        try self.hex(member_name, &value);
    }

    fn hexInteger(self: *Object, member_name: []const u8, value: u64) WriteError!void {
        var bytes: [8]u8 = undefined;
        std.mem.writeInt(u64, &bytes, value, .big);
        try self.hex(member_name, &bytes);
    }

    fn process(self: *Object, member_name: []const u8, value: message.Process) WriteError!void {
        if (value.pid == 0 or value.created == 0) return error.InvalidMessage;
        try self.key(member_name);
        var inner = try Object.begin(self.writer);
        try inner.integer("pid", value.pid);
        try inner.hexInteger("created", value.created);
        try inner.end();
    }

    fn handles(self: *Object, value: message.Handles) WriteError!void {
        try self.key("handles");
        var inner = try Object.begin(self.writer);
        inline for (message.Handles.roles) |role| {
            const handle = @field(value, role);
            if (handle == 0) return error.InvalidMessage;
            try inner.hexInteger(role, handle);
        }
        try inner.end();
    }

    fn programHandles(self: *Object, value: message.ProgramHandles) WriteError!void {
        try self.key("handles");
        var inner = try Object.begin(self.writer);
        if (value.job == 0 or value.process == 0) return error.InvalidMessage;
        try inner.hexInteger("job", value.job);
        try inner.hexInteger("process", value.process);
        inline for (.{ "stdout", "stderr" }) |role| if (@field(value, role)) |handle| {
            if (handle == 0) return error.InvalidMessage;
            try inner.hexInteger(role, handle);
        };
        try inner.end();
    }

    fn programFacts(self: *Object, value: message.ProgramFacts) WriteError!void {
        if (!value.valid()) return error.InvalidMessage;
        try self.key("facts");
        var inner = try Object.begin(self.writer);
        try inner.process("process", value.process);
        try inner.hex("programId", &value.program_id);
        try inner.text("label", value.label);
        try inner.end();
    }

    fn terminal(self: *Object, member_name: []const u8, value: message.Terminal) WriteError!void {
        try self.key(member_name);
        var inner = try Object.begin(self.writer);
        try inner.hex("creationId", &value.creation_id);
        try inner.hex("hostEpoch", &value.host_epoch);
        try inner.hex("terminalId", &value.terminal_id);
        try inner.end();
    }

    fn facts(self: *Object, value: message.Facts) WriteError!void {
        if (value.signal_format == 0) return error.InvalidMessage;
        try self.key("facts");
        var inner = try Object.begin(self.writer);
        try inner.terminal("terminal", value.terminal);
        try inner.process("program", value.program);
        try inner.process("console", value.console);
        try inner.hex("consoleBuild", &value.console_build);
        try inner.integer("signalFormat", value.signal_format);
        try inner.name("launch", value.launch);
        try inner.end();
    }

    fn ptyFacts(self: *Object, value: message.pty.PtyFacts) WriteError!void {
        if (!message.pty.validSlave(value.slave)) return error.InvalidMessage;
        try self.key("facts");
        var inner = try Object.begin(self.writer);
        try inner.terminal("terminal", value.terminal);
        try inner.process("leader", value.leader);
        try inner.text("slave", value.slave);
        try inner.name("launch", value.launch);
        try inner.end();
    }

    /// A PTY holding's `facts`, `parented`, and `ended` when the leader ended.
    fn ptyHolding(self: *Object, value: message.pty.PtyHolding) WriteError!void {
        try self.ptyFacts(value.facts);
        try self.boolean("parented", value.parented);
        if (value.ended) |status| try self.exitStatus("ended", status);
    }

    fn exitStatus(self: *Object, member_name: []const u8, value: message.pty.ExitStatus) WriteError!void {
        try self.key(member_name);
        var inner = try Object.begin(self.writer);
        switch (value) {
            .code => |code| try inner.integer("code", code),
            .signal => |signal| {
                if (signal == 0) return error.InvalidMessage;
                try inner.integer("signal", signal);
            },
        }
        try inner.end();
    }

    fn boolean(self: *Object, member_name: []const u8, value: bool) WriteError!void {
        try self.key(member_name);
        try self.writer.writeAll(if (value) "true" else "false");
    }

    fn strings(self: *Object, member_name: []const u8, values: []const []const u8) WriteError!void {
        try self.key(member_name);
        try self.writer.writeByte('[');
        for (values, 0..) |value, index| {
            if (index > 0) try self.writer.writeByte(',');
            try writeString(self.writer, value);
        }
        try self.writer.writeByte(']');
    }

    fn spawn(self: *Object, value: message.pty.Spawn) WriteError!void {
        if (!message.pty.validLaunch(value.launch) or !message.pty.validSlave(value.facts.slave)) return error.InvalidMessage;
        try self.randomId("holding", value.holding);
        try self.boolean("input", value.input);
        try self.key("launch");
        var launch = try Object.begin(self.writer);
        try launch.text("executable", value.launch.executable);
        try launch.strings("arguments", value.launch.arguments);
        try launch.strings("environment", value.launch.environment);
        try launch.strings("candidates", value.launch.candidates);
        if (value.launch.directory) |directory| try launch.text("directory", directory);
        try launch.end();
        try self.key("facts");
        var facts_object = try Object.begin(self.writer);
        try facts_object.terminal("terminal", value.facts.terminal);
        try facts_object.text("slave", value.facts.slave);
        try facts_object.name("launch", value.facts.launch);
        try facts_object.end();
    }

    fn extensions(self: *Object, members: message.Extensions) WriteError!void {
        if (members.len == 0) return;
        try self.key("extensions");
        var inner = try Object.begin(self.writer);
        for (members) |member| {
            if (!grammar.isName(member.name)) return error.InvalidMessage;
            try inner.key(member.name);
            try writeValue(self.writer, member.value);
        }
        try inner.end();
    }
};

fn writeInitiator(writer: *std.Io.Writer, value: message.InitiatorMessage) WriteError!void {
    var object = try Object.begin(writer);
    switch (value) {
        .request => |request| {
            try object.text("type", std.meta.activeTag(request.body).name());
            try object.id(request.id);
            switch (request.body) {
                .hello => |role| try object.text("role", role.name()),
                .register_host, .unregister_host => |process| try object.process("process", process),
                .status => |after| if (after) |holding| try object.randomId("after", holding),
                .hold => |hold| {
                    try object.randomId("holding", hold.holding);
                    try object.handles(hold.handles);
                    try object.facts(hold.facts);
                },
                .mark, .assign => |mark| {
                    try object.randomId("holding", mark.holding);
                    try object.process("for", mark.target);
                    try object.randomId("mark", mark.mark);
                },
                .@"resume", .inspect, .decline => |marked| {
                    try object.randomId("holding", marked.holding);
                    try object.randomId("mark", marked.mark);
                },
                .claim => |claim| {
                    try object.randomId("holding", claim.holding);
                    try object.randomId("mark", claim.mark);
                    try object.process("program", claim.program);
                    if (claim.terminal) |terminal| try object.terminal("terminal", terminal);
                },
                .release, .discard, .reap => |holding| try object.randomId("holding", holding),
                .hold_pty => |hold| {
                    try object.randomId("holding", hold.holding);
                    try object.ptyFacts(hold.facts);
                },
                .spawn => |spawn| try object.spawn(spawn),
                .reexec => |program| {
                    if (!message.validProgram(program)) return error.InvalidMessage;
                    try object.text("program", program);
                },
                .hold_program => |hold| {
                    try object.randomId("holding", hold.holding);
                    try object.programHandles(hold.handles);
                    try object.programFacts(hold.facts);
                },
            }
            try object.extensions(request.extensions);
        },
        .unknown_request => |request| {
            if (message.RequestKind.fromName(request.request) != null) return error.InvalidMessage;
            try object.name("type", request.request);
            try object.id(request.id);
        },
    }
    try object.end();
}

fn writeAcceptor(writer: *std.Io.Writer, value: message.AcceptorMessage) WriteError!void {
    var object = try Object.begin(writer);
    switch (value) {
        .event => |event| try object.name("type", event),
        .exited => |exited| {
            try object.text("type", "exited");
            try object.randomId("holding", exited.holding);
            try object.exitStatus("status", exited.status);
        },
        .reply => |reply| {
            switch (reply.body) {
                .refused => |reason| {
                    try object.text("type", "refused");
                    try object.id(reply.id);
                    try object.text("reason", reason.name());
                },
                .unsupported => |request| {
                    try object.text("type", "unsupported");
                    try object.id(reply.id);
                    try object.name("request", request);
                },
                .status => |page| {
                    if (!page.valid()) return error.InvalidMessage;
                    try object.text("type", message.RequestKind.status.resultName());
                    try object.id(reply.id);
                    try writeStatus(&object, page);
                },
                .@"resume", .decline => |outcome| {
                    const kind: message.RequestKind = if (reply.body == .@"resume") .@"resume" else .decline;
                    try object.text("type", kind.resultName());
                    try object.id(reply.id);
                    try object.text("outcome", outcome.name());
                },
                .inspect => |inspection| {
                    try object.text("type", message.RequestKind.inspect.resultName());
                    try object.id(reply.id);
                    try object.handles(inspection.handles);
                    try object.facts(inspection.facts);
                },
                .inspect_program => |inspection| {
                    try object.text("type", message.RequestKind.inspect.resultName());
                    try object.id(reply.id);
                    try object.programHandles(inspection.handles);
                    try object.programFacts(inspection.facts);
                },
                .inspect_pty => |holding| {
                    try object.text("type", message.RequestKind.inspect.resultName());
                    try object.id(reply.id);
                    try object.ptyHolding(holding);
                },
                .spawn => |leader| {
                    try object.text("type", message.RequestKind.spawn.resultName());
                    try object.id(reply.id);
                    try object.process("leader", leader);
                },
                inline .hello, .register_host, .unregister_host, .hold, .mark, .claim, .release, .assign, .discard, .hold_program, .hold_pty, .reap, .reexec => |_, tag| {
                    try object.text("type", @field(message.RequestKind, @tagName(tag)).resultName());
                    try object.id(reply.id);
                },
            }
            try object.extensions(reply.extensions);
        },
    }
    try object.end();
}

fn writeStatus(object: *Object, page: message.StatusPage) WriteError!void {
    try object.key("holdings");
    try object.writer.writeByte('[');
    for (page.holdings, 0..) |entry, index| {
        if (index > 0) try object.writer.writeByte(',');
        var inner = try Object.begin(object.writer);
        try inner.randomId("holding", entry.holding);
        try inner.process("owner", entry.owner);
        try inner.text("state", entry.state.name());
        if (entry.target) |target| try inner.process("for", target);
        switch (entry.facts) {
            .terminal => |facts| try inner.facts(facts),
            .program => |facts| {
                try inner.text("kind", "program");
                try inner.programFacts(facts);
            },
            .pty => |holding| {
                try inner.text("kind", "pty");
                try inner.ptyHolding(holding);
            },
        }
        try inner.end();
    }
    try object.writer.writeByte(']');
    if (page.hosts) |hosts| {
        try object.key("hosts");
        try object.writer.writeByte('[');
        for (hosts, 0..) |host, index| {
            if (index > 0) try object.writer.writeByte(',');
            var inner = try Object.begin(object.writer);
            try inner.process("process", host.process);
            try inner.key("ended");
            try object.writer.writeAll(if (host.ended) "true" else "false");
            try inner.end();
        }
        try object.writer.writeByte(']');
    }
    if (page.next) |next| try object.randomId("next", next);
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
    if (!std.unicode.utf8ValidateSlice(text)) return error.InvalidMessage;
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
