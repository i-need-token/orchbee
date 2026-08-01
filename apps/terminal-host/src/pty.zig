const std = @import("std");
const builtin = @import("builtin");

const windows_api = @import("windows_api.zig");

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

const PosixPty = if (builtin.os.tag == .windows) void else struct {
    const posix = std.posix;
    const c = @import("pty-c");

    master: posix.fd_t,
    slave: ?posix.fd_t,

    pub fn open(size: Size) !PosixPty {
        var native_size = extern struct {
            ws_row: u16,
            ws_col: u16,
            ws_xpixel: u16,
            ws_ypixel: u16,
        }{
            .ws_row = size.rows,
            .ws_col = size.columns,
            .ws_xpixel = size.pixel_width,
            .ws_ypixel = size.pixel_height,
        };
        var master: posix.fd_t = undefined;
        var slave: posix.fd_t = undefined;
        if (c.openpty(&master, &slave, null, null, @ptrCast(&native_size)) < 0) {
            return error.OpenPtyFailed;
        }
        errdefer {
            _ = posix.system.close(master);
            _ = posix.system.close(slave);
        }

        try setCloseOnExec(master);
        try setCloseOnExec(slave);
        var attributes: c.termios = undefined;
        if (c.tcgetattr(master, &attributes) != 0) return error.ConfigurePtyFailed;
        attributes.c_iflag |= c.IUTF8;
        if (c.tcsetattr(master, c.TCSANOW, &attributes) != 0) return error.ConfigurePtyFailed;
        return .{ .master = master, .slave = slave };
    }

    pub fn deinit(self: *PosixPty) void {
        if (self.slave) |slave| _ = posix.system.close(slave);
        _ = posix.system.close(self.master);
        self.* = undefined;
    }

    pub fn closeSlave(self: *PosixPty) void {
        if (self.slave) |slave| {
            _ = posix.system.close(slave);
            self.slave = null;
        }
    }

    pub fn childSetup(self: PosixPty) !void {
        const slave = self.slave orelse return error.MissingSlave;
        if (c.setsid() < 0) return error.CreateSessionFailed;
        if (c.ioctl(slave, c.TIOCSCTTY, @as(c_ulong, 0)) < 0) {
            return error.SetControllingTerminalFailed;
        }
        try duplicateTo(slave, posix.STDIN_FILENO);
        try duplicateTo(slave, posix.STDOUT_FILENO);
        try duplicateTo(slave, posix.STDERR_FILENO);
        _ = posix.system.close(slave);
        _ = posix.system.close(self.master);
    }

    pub fn read(self: *const PosixPty, buffer: []u8) !usize {
        return posix.read(self.master, buffer) catch |err| switch (err) {
            error.BrokenPipe, error.InputOutput => 0,
            else => return err,
        };
    }

    pub fn write(self: *const PosixPty, bytes: []const u8) !usize {
        return posix.write(self.master, bytes);
    }

    pub fn setSize(self: *const PosixPty, size: Size) !void {
        var native_size = extern struct {
            ws_row: u16,
            ws_col: u16,
            ws_xpixel: u16,
            ws_ypixel: u16,
        }{
            .ws_row = size.rows,
            .ws_col = size.columns,
            .ws_xpixel = size.pixel_width,
            .ws_ypixel = size.pixel_height,
        };
        if (c.ioctl(self.master, c.TIOCSWINSZ, @intFromPtr(&native_size)) < 0) {
            return error.ResizePtyFailed;
        }
    }

    fn setCloseOnExec(fd: posix.fd_t) !void {
        const flags = try posix.fcntl(fd, posix.F.GETFD, 0);
        _ = try posix.fcntl(fd, posix.F.SETFD, flags | posix.FD_CLOEXEC);
    }

    fn duplicateTo(source: posix.fd_t, target: posix.fd_t) !void {
        if (source == target) {
            const flags = try posix.fcntl(source, posix.F.GETFD, 0);
            _ = try posix.fcntl(source, posix.F.SETFD, flags & ~@as(usize, posix.FD_CLOEXEC));
            return;
        }
        _ = try posix.dup2(source, target);
    }
};

const WindowsPty = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    console_closed: bool = false,
    input: windows.HANDLE,
    output: windows.HANDLE,
    pseudo_console: windows_api.HPCON,

    pub fn open(size: Size) !WindowsPty {
        var input_console: windows.HANDLE = undefined;
        var input: windows.HANDLE = undefined;
        var output: windows.HANDLE = undefined;
        var output_console: windows.HANDLE = undefined;
        if (windows_api.CreatePipe(&input_console, &input, null, 0) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
        errdefer {
            _ = windows.CloseHandle(input_console);
            _ = windows.CloseHandle(input);
        }
        if (windows_api.CreatePipe(&output, &output_console, null, 0) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
        errdefer {
            _ = windows.CloseHandle(output);
            _ = windows.CloseHandle(output_console);
        }

        try windows.SetHandleInformation(input, windows.HANDLE_FLAG_INHERIT, 0);
        try windows.SetHandleInformation(output, windows.HANDLE_FLAG_INHERIT, 0);
        var pseudo_console: windows_api.HPCON = undefined;
        const result = windows_api.CreatePseudoConsole(
            .{ .X = @intCast(size.columns), .Y = @intCast(size.rows) },
            input_console,
            output_console,
            0,
            &pseudo_console,
        );
        if (result != windows.S_OK) return error.OpenPtyFailed;
        _ = windows.CloseHandle(input_console);
        _ = windows.CloseHandle(output_console);
        return .{
            .input = input,
            .output = output,
            .pseudo_console = pseudo_console,
        };
    }

    pub fn deinit(self: *WindowsPty) void {
        self.closeConsole();
        _ = windows.CloseHandle(self.input);
        _ = windows.CloseHandle(self.output);
        self.* = undefined;
    }

    pub fn closeConsole(self: *WindowsPty) void {
        if (self.console_closed) return;
        windows_api.ClosePseudoConsole(self.pseudo_console);
        self.console_closed = true;
    }

    pub fn closeSlave(_: *WindowsPty) void {}

    pub fn read(self: *const WindowsPty, buffer: []u8) !usize {
        return windows.ReadFile(self.output, buffer, null) catch |err| switch (err) {
            error.BrokenPipe => 0,
            else => return err,
        };
    }

    pub fn write(self: *const WindowsPty, bytes: []const u8) !usize {
        return windows.WriteFile(self.input, bytes, null);
    }

    pub fn setSize(self: *const WindowsPty, size: Size) !void {
        const result = windows_api.ResizePseudoConsole(
            self.pseudo_console,
            .{ .X = @intCast(size.columns), .Y = @intCast(size.rows) },
        );
        if (result != windows.S_OK) return error.ResizePtyFailed;
    }
};

test "PTY opens with bounded dimensions and resizes" {
    var instance = try Pty.open(.{ .rows = 24, .columns = 80 });
    defer instance.deinit();
    try instance.setSize(.{ .rows = 40, .columns = 120 });
}
