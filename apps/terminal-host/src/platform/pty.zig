const std = @import("std");
const builtin = @import("builtin");

const windows_api = @import("windows_api.zig");

extern "c" fn tcgetpgrp(handle: c_int) c_int;

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

    abort_read_fd: posix.fd_t,
    abort_write_fd: posix.fd_t,
    master: posix.fd_t,
    read_aborted: std.atomic.Value(bool) = .init(false),
    slave: ?posix.fd_t,

    pub fn open(size: Size) !PosixPty {
        var native_size = nativeSize(size);
        var master: posix.fd_t = undefined;
        var slave: posix.fd_t = undefined;
        if (c.openpty(&master, &slave, null, null, &native_size) < 0) {
            return error.OpenPtyFailed;
        }
        const abort_pipe = posix.pipe2(.{ .CLOEXEC = true }) catch |err| {
            _ = posix.system.close(master);
            _ = posix.system.close(slave);
            return err;
        };
        errdefer {
            _ = posix.system.close(master);
            _ = posix.system.close(slave);
            _ = posix.system.close(abort_pipe[0]);
            _ = posix.system.close(abort_pipe[1]);
        }

        try setCloseOnExec(master);
        try setCloseOnExec(slave);
        var attributes: c.termios = undefined;
        if (c.tcgetattr(master, &attributes) != 0) return error.ConfigurePtyFailed;
        attributes.c_iflag |= c.IUTF8;
        if (c.tcsetattr(master, c.TCSANOW, &attributes) != 0) return error.ConfigurePtyFailed;
        return .{
            .abort_read_fd = abort_pipe[0],
            .abort_write_fd = abort_pipe[1],
            .master = master,
            .slave = slave,
        };
    }

    pub fn deinit(self: *PosixPty) void {
        if (self.slave) |slave| _ = posix.system.close(slave);
        _ = posix.system.close(self.master);
        _ = posix.system.close(self.abort_read_fd);
        _ = posix.system.close(self.abort_write_fd);
        self.* = undefined;
    }

    pub fn abortRead(self: *PosixPty) !void {
        if (self.read_aborted.load(.acquire)) return;
        const marker = [_]u8{1};
        _ = try posix.write(self.abort_write_fd, &marker);
        self.read_aborted.store(true, .release);
    }

    pub fn closeSlave(self: *PosixPty) void {
        if (self.slave) |slave| {
            _ = posix.system.close(slave);
            self.slave = null;
        }
    }

    pub fn childSetup(self: PosixPty) !void {
        const slave = self.slave orelse return error.MissingSlave;
        // openpty may reuse 0, 1, or 2 when the caller closed a standard
        // descriptor. Drop the master before installing the slave, then keep
        // any slave descriptor that is itself one of the resulting streams.
        _ = posix.system.close(self.master);
        if (c.setsid() < 0) return error.CreateSessionFailed;
        if (c.ioctl(slave, c.TIOCSCTTY, @as(c_ulong, 0)) < 0) {
            return error.SetControllingTerminalFailed;
        }
        try duplicateTo(slave, posix.STDIN_FILENO);
        try duplicateTo(slave, posix.STDOUT_FILENO);
        try duplicateTo(slave, posix.STDERR_FILENO);
        if (slave > posix.STDERR_FILENO) _ = posix.system.close(slave);
    }

    pub fn read(self: *PosixPty, buffer: []u8) !usize {
        var descriptors = [_]posix.pollfd{
            .{ .fd = self.master, .events = posix.POLL.IN, .revents = 0 },
            .{ .fd = self.abort_read_fd, .events = posix.POLL.IN, .revents = 0 },
        };
        while (true) {
            _ = try posix.poll(&descriptors, -1);
            if (descriptors[1].revents != 0) return 0;
            if (descriptors[0].revents & (posix.POLL.IN | posix.POLL.HUP | posix.POLL.ERR) != 0) {
                return posix.read(self.master, buffer) catch |err| switch (err) {
                    error.BrokenPipe, error.InputOutput => 0,
                    else => return err,
                };
            }
            descriptors[0].revents = 0;
            descriptors[1].revents = 0;
        }
    }

    pub fn write(self: *const PosixPty, bytes: []const u8) !usize {
        return posix.write(self.master, bytes);
    }

    pub fn foregroundProcessGroup(self: *const PosixPty) !posix.pid_t {
        const process_group = tcgetpgrp(self.master);
        if (process_group < 0) return error.ProcessObservationFailed;
        return @intCast(process_group);
    }

    pub fn registerReaderThread(_: *PosixPty, _: std.Thread.Handle) void {}

    pub fn setSize(self: *const PosixPty, size: Size) !void {
        var native_size = nativeSize(size);
        if (c.ioctl(self.master, c.TIOCSWINSZ, @intFromPtr(&native_size)) < 0) {
            return error.ResizePtyFailed;
        }
    }

    fn nativeSize(size: Size) c.struct_winsize {
        return .{
            .ws_row = size.rows,
            .ws_col = size.columns,
            .ws_xpixel = size.pixel_width,
            .ws_ypixel = size.pixel_height,
        };
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
    lifecycle_mutex: std.Thread.Mutex = .{},
    output: windows.HANDLE,
    pseudo_console: windows_api.HPCON,
    read_abort_requested: bool = false,
    read_active: bool = false,
    read_condition: std.Thread.Condition = .{},
    reader_thread: ?std.Thread.Handle = null,

    pub fn open(size: Size) !WindowsPty {
        const columns = try checkedCoordinate(size.columns);
        const rows = try checkedCoordinate(size.rows);
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
            .{ .X = columns, .Y = rows },
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

    pub fn abortRead(self: *WindowsPty) !void {
        self.lifecycle_mutex.lock();
        if (self.read_abort_requested and !self.read_active) {
            self.lifecycle_mutex.unlock();
            return;
        }
        self.read_abort_requested = true;
        self.read_condition.broadcast();
        var cancellation_requested = false;
        while (self.read_active) {
            if (cancellation_requested) {
                self.read_condition.wait(&self.lifecycle_mutex);
                continue;
            }
            const reader_thread = self.reader_thread orelse {
                self.lifecycle_mutex.unlock();
                return error.MissingReaderThread;
            };
            self.lifecycle_mutex.unlock();
            if (windows_api.CancelSynchronousIo(reader_thread) != 0) {
                cancellation_requested = true;
            } else switch (windows.kernel32.GetLastError()) {
                // The reader marks itself active immediately before entering
                // ReadFile. If cancellation wins that narrow gap, retry until
                // either the read starts or the reader observes the abort.
                .NOT_FOUND => std.Thread.yield() catch {},
                else => |err| return windows.unexpectedError(err),
            }
            self.lifecycle_mutex.lock();
        }
        self.lifecycle_mutex.unlock();
    }

    pub fn registerReaderThread(self: *WindowsPty, thread: std.Thread.Handle) void {
        self.lifecycle_mutex.lock();
        defer self.lifecycle_mutex.unlock();
        std.debug.assert(self.reader_thread == null);
        self.reader_thread = thread;
        self.read_condition.broadcast();
    }

    pub fn closeConsole(self: *WindowsPty) void {
        self.lifecycle_mutex.lock();
        defer self.lifecycle_mutex.unlock();
        if (self.console_closed) return;
        windows_api.ClosePseudoConsole(self.pseudo_console);
        self.console_closed = true;
    }

    pub fn closeSlave(_: *WindowsPty) void {}

    pub fn read(self: *WindowsPty, buffer: []u8) !usize {
        self.lifecycle_mutex.lock();
        while (self.reader_thread == null and !self.read_abort_requested) {
            self.read_condition.wait(&self.lifecycle_mutex);
        }
        if (self.read_abort_requested) {
            self.lifecycle_mutex.unlock();
            return 0;
        }
        std.debug.assert(!self.read_active);
        self.read_active = true;
        self.lifecycle_mutex.unlock();
        defer {
            self.lifecycle_mutex.lock();
            self.read_active = false;
            self.read_condition.broadcast();
            self.lifecycle_mutex.unlock();
        }
        var read_bytes: windows.DWORD = 0;
        if (windows.kernel32.ReadFile(
            self.output,
            buffer.ptr,
            @intCast(@min(buffer.len, std.math.maxInt(windows.DWORD))),
            &read_bytes,
            null,
        ) == 0) return switch (windows.kernel32.GetLastError()) {
            .BROKEN_PIPE, .HANDLE_EOF, .OPERATION_ABORTED => 0,
            else => |err| windows.unexpectedError(err),
        };
        return read_bytes;
    }

    pub fn write(self: *const WindowsPty, bytes: []const u8) !usize {
        return windows.WriteFile(self.input, bytes, null);
    }

    pub fn setSize(self: *WindowsPty, size: Size) !void {
        const columns = try checkedCoordinate(size.columns);
        const rows = try checkedCoordinate(size.rows);
        self.lifecycle_mutex.lock();
        defer self.lifecycle_mutex.unlock();
        if (self.console_closed) return error.PtyClosed;
        const result = windows_api.ResizePseudoConsole(
            self.pseudo_console,
            .{ .X = columns, .Y = rows },
        );
        if (result != windows.S_OK) return error.ResizePtyFailed;
    }

    fn checkedCoordinate(value: u16) !i16 {
        return std.math.cast(i16, value) orelse error.PtySizeOutOfRange;
    }
};
