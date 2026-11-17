const builtin = @import("builtin");

pub const Size = extern struct {
    rows: u16,
    columns: u16,
    pixel_width: u16 = 0,
    pixel_height: u16 = 0,
};

pub const Pty = switch (builtin.os.tag) {
    .windows => WindowsPty,
    .freebsd, .linux, .macos => PosixPty,
    else => @compileError("terminal-host has no PTY implementation for this target"),
};

const PosixPty = @import("pty_posix.zig").PosixPty;
const WindowsPty = @import("pty_windows.zig").WindowsPty;
