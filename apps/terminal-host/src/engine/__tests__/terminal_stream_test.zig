const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const engine_boundary = @import("terminal-host-engine");
const terminal_stream = engine_boundary.testing.terminal_stream;

test "Host OSC parser allocator enforces and releases its byte budget" {
    var state = terminal_stream.ParserAllocator.init(std.testing.allocator);
    defer state.deinit();
    const allocator = state.allocator();

    {
        const full = try allocator.alloc(u8, terminal_stream.osc_allocation_limit_bytes);
        defer allocator.free(full);
        try std.testing.expectError(error.OutOfMemory, allocator.alloc(u8, 1));
    }

    const recovered = try allocator.alloc(u8, terminal_stream.osc_allocation_limit_bytes);
    allocator.free(recovered);
}

const ReplyCapture = struct {
    stream: terminal_stream.Stream,
    bytes: [512]u8 = undefined,
    length: usize = 0,

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

test "Host terminal stream restores scoped DECRQSS replies across parser chunks" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);

    var capture: ReplyCapture = .{
        .stream = .init(.init(&terminal)),
    };
    defer capture.stream.deinit();
    capture.stream.handler.terminal_handler.effects.write_pty = ReplyCapture.writePty;

    capture.stream.nextSlice("A\x1bP$");
    capture.stream.nextSlice("qm");
    capture.stream.nextSlice("\x1b\\");
    capture.stream.nextSlice(
        "\x1b[1;31m\x1bP$qm\x1b\\" ++
            "\x1b[5 q\x1bP$q q\x1b\\" ++
            "\x1b[3;20r\x1bP$qr\x1b\\" ++
            "\x1bP$qs\x1b\\" ++
            "\x1b[?69h\x1b[5;70s\x1bP$qs\x1b\\" ++
            "\x1bP$q\"q\x1b\\",
    );
    try std.testing.expectEqualStrings(
        "\x1bP1$r0m\x1b\\" ++
            "\x1bP1$r0;1;31m\x1b\\" ++
            "\x1bP1$r5 q\x1b\\" ++
            "\x1bP1$r3;20r\x1b\\" ++
            "\x1bP0$r\x1b\\" ++
            "\x1bP1$r5;70s\x1b\\" ++
            "\x1bP0$r\x1b\\",
        capture.bytes[0..capture.length],
    );

    const before_unsupported = capture.length;
    capture.stream.nextSlice("\x1bP+q544e\x1b\\");
    try std.testing.expectEqual(before_unsupported, capture.length);

    capture.stream.handler.vt(.dcs_hook, .{ .intermediates = "$", .final = 'q' });
    capture.stream.handler.vt(.dcs_put, 'm');
    capture.stream.handler.vt(.dcs_hook, .{ .intermediates = "+", .final = 'q' });
    capture.stream.handler.vt(.dcs_unhook, {});
    try std.testing.expectEqual(before_unsupported, capture.length);

    const plain = try terminal.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expectEqualStrings("A", plain);
}
