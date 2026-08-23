const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const terminal_stream = engine_boundary.testing.terminal_stream;

const ReplyCapture = struct {
    stream: terminal_stream.Stream,
    bytes: [4096]u8 = undefined,
    length: usize = 0,

    fn init(terminal: *ghostty_vt.Terminal) ReplyCapture {
        return .{ .stream = .init(.init(terminal)) };
    }

    fn writePty(
        terminal_handler: *ghostty_vt.TerminalStream.Handler,
        bytes: [:0]const u8,
    ) void {
        const host_handler: *terminal_stream.Handler = @fieldParentPtr(
            "terminal_handler",
            terminal_handler,
        );
        const stream: *terminal_stream.Stream = @fieldParentPtr("handler", host_handler);
        const self: *ReplyCapture = @fieldParentPtr("stream", stream);
        std.debug.assert(bytes.len <= self.bytes.len - self.length);
        @memcpy(self.bytes[self.length..][0..bytes.len], bytes);
        self.length += bytes.len;
    }
};

test "Host disables Glyph Protocol until its graphics are projected" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);

    var capture = ReplyCapture.init(&terminal);
    defer capture.stream.deinit();
    capture.stream.handler.terminal_handler.effects.write_pty = ReplyCapture.writePty;

    capture.stream.nextSlice("\x1b_25a1;s\x1b\\");
    capture.stream.nextSlice("\x1b_25a1;r;cp=e0a0;AAAAAAAAAAAAAA==\x1b\\");

    try std.testing.expectEqual(@as(usize, 0), capture.length);
    try std.testing.expect(!terminal.glyph_glossary.contains(0xE0A0));
}

test "Host XTGETTCAP returns one ordered xterm-256color response across chunks" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);

    var capture = ReplyCapture.init(&terminal);
    defer capture.stream.deinit();
    capture.stream.handler.terminal_handler.effects.write_pty = ReplyCapture.writePty;

    capture.stream.nextSlice("\x1bP+");
    capture.stream.nextSlice("q544e;436f;524742;6b63757531;");
    capture.stream.nextSlice("6b663132\x1b\\");

    try std.testing.expectEqualStrings(
        "\x1bP1+r" ++
            "544E=787465726D2D323536636F6C6F72;" ++
            "436F=323536;" ++
            "524742=38;" ++
            "6B63757531=1B4F41;" ++
            "6B663132=1B5B32347E" ++
            "\x1b\\",
        capture.bytes[0..capture.length],
    );
}

test "Host XTGETTCAP stops at unsupported names and rejects malformed first names" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);

    var capture = ReplyCapture.init(&terminal);
    defer capture.stream.deinit();
    capture.stream.handler.terminal_handler.effects.write_pty = ReplyCapture.writePty;

    capture.stream.nextSlice("\x1bP+q544e;4e4f5045;436f\x1b\\");
    capture.stream.nextSlice("\x1bP+q5\x1b\\");
    capture.stream.nextSlice("\x1bP$qm\x1b\\");

    try std.testing.expectEqualStrings(
        "\x1bP1+r544E=787465726D2D323536636F6C6F72\x1b\\" ++
            "\x1bP0+r\x1b\\" ++
            "\x1bP1$r0m\x1b\\",
        capture.bytes[0..capture.length],
    );
}

test "Host XTGETTCAP bounds request retention" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);

    var capture = ReplyCapture.init(&terminal);
    defer capture.stream.deinit();
    capture.stream.handler.terminal_handler.effects.write_pty = ReplyCapture.writePty;

    const over_limit =
        [_]u8{'4'} ** (terminal_stream.xtgettcap_request_limit_bytes + 1);
    capture.stream.nextSlice("\x1bP+q");
    capture.stream.nextSlice(&over_limit);
    capture.stream.nextSlice("\x1b\\");

    try std.testing.expectEqualStrings(
        "\x1bP0+r\x1b\\",
        capture.bytes[0..capture.length],
    );
}
