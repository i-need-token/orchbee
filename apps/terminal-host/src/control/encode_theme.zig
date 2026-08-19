const std = @import("std");

const common = @import("encode_common.zig");

const identifierText = common.identifierText;

pub fn terminalThemeResult(allocator: std.mem.Allocator, result: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalThemeResult(&writer.writer, result);
    return writer.toOwnedSlice();
}

pub fn writeTerminalThemeResult(writer: *std.Io.Writer, result: anytype) !void {
    var operation_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    try writer.writeAll("{\"current\":");
    try writeTerminalThemeState(writer, result.current);
    try writer.print(
        ",\"operationId\":\"{s}\",\"terminalId\":\"{s}\",\"transition\":\"{s}\"}}",
        .{
            identifierText(result.operation_id, &operation_buffer),
            identifierText(result.terminal_id, &terminal_buffer),
            transitionText(result.transition),
        },
    );
}

pub fn terminalThemeState(allocator: std.mem.Allocator, state: anytype) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writeTerminalThemeState(&writer.writer, state);
    return writer.toOwnedSlice();
}

pub fn writeTerminalThemeState(writer: *std.Io.Writer, state: anytype) !void {
    try writer.writeAll("{\"operationId\":");
    if (state.operation_id) |operation_id| {
        var operation_buffer: [32]u8 = undefined;
        try writer.print("\"{s}\"", .{identifierText(operation_id, &operation_buffer)});
    } else {
        try writer.writeAll("null");
    }
    try writer.print(",\"revision\":\"{d}\",\"terminal\":", .{state.revision});
    try writeTerminalTheme(writer, state.terminal);
    try writer.writeByte('}');
}

fn writeTerminalTheme(writer: *std.Io.Writer, theme: anytype) !void {
    try writer.print(
        "{{\"colorScheme\":\"{s}\",\"defaultBackgroundRgb\":{d},\"defaultCursorRgb\":",
        .{ @tagName(theme.color_scheme), theme.default_background_rgb },
    );
    if (theme.default_cursor_rgb) |cursor| {
        try writer.print("{d}", .{cursor});
    } else {
        try writer.writeAll("null");
    }
    try writer.print(",\"defaultForegroundRgb\":{d},\"palette\":[", .{
        theme.default_foreground_rgb,
    });
    for (theme.palette, 0..) |color, index| {
        if (index != 0) try writer.writeByte(',');
        try writer.print("{d}", .{color});
    }
    try writer.writeAll("]}");
}

fn transitionText(transition: anytype) []const u8 {
    return switch (transition) {
        .already_applied => "already-applied",
        .applied => "applied",
        .stale => "stale",
    };
}
