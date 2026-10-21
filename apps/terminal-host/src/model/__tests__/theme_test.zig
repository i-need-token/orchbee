const std = @import("std");

const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_theme = model.theme;

test "theme state reserves revision zero for the launch baseline" {
    const operation_id = identifiers.TerminalThemeOperationId.fromBytes([_]u8{0x44} ** 16);
    try std.testing.expect(terminal_theme.validState(.{
        .operation_id = null,
        .revision = 0,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(terminal_theme.validState(.{
        .operation_id = operation_id,
        .revision = 1,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(!terminal_theme.validState(.{
        .operation_id = operation_id,
        .revision = 0,
        .terminal = terminal_theme.default,
    }));
    try std.testing.expect(!terminal_theme.validState(.{
        .operation_id = null,
        .revision = 1,
        .terminal = terminal_theme.default,
    }));
}
