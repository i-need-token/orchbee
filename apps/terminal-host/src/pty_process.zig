const std = @import("std");
const builtin = @import("builtin");

const pty_module = @import("pty.zig");
const windows_api = @import("windows_api.zig");

pub const EnvironmentDelta = struct {
    name: []const u8,
    value: ?[]const u8,
};

pub const Launch = struct {
    arguments: []const []const u8,
    cell_pixel_height: u16 = 0,
    cell_pixel_width: u16 = 0,
    columns: u16,
    current_directory: ?[]const u8,
    environment: []const EnvironmentDelta,
    executable: []const u8,
    rows: u16,
};

pub const ExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};

pub const Process = switch (builtin.os.tag) {
    .windows => WindowsProcess,
    .freebsd, .linux, .macos => PosixProcess,
    else => @compileError("terminal-host has no process implementation for this target"),
};

const PosixProcess = if (builtin.os.tag == .windows) void else struct {
    const posix = std.posix;

    child_id: posix.pid_t,
    pty: pty_module.Pty,

    pub fn spawn(allocator: std.mem.Allocator, launch: Launch) !PosixProcess {
        var environment = try environmentForLaunch(allocator, launch.environment);
        defer environment.deinit();
        var arena_state = std.heap.ArenaAllocator.init(allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();

        const argument_count = launch.arguments.len + 1;
        const argv = try arena.allocSentinel(?[*:0]const u8, argument_count, null);
        argv[0] = (try arena.dupeZ(u8, launch.executable)).ptr;
        for (launch.arguments, 1..) |argument, index| {
            argv[index] = (try arena.dupeZ(u8, argument)).ptr;
        }
        const environment_pointer = (try std.process.createNullDelimitedEnvMap(arena, &environment)).ptr;

        var pty = try pty_module.Pty.open(.{
            .columns = launch.columns,
            .pixel_height = try pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();
        const error_pipe = try posix.pipe2(.{ .CLOEXEC = true });
        var error_read_open = true;
        var error_write_open = true;
        errdefer {
            if (error_read_open) posix.close(error_pipe[0]);
            if (error_write_open) posix.close(error_pipe[1]);
        }

        const child_id = try posix.fork();
        if (child_id == 0) {
            posix.close(error_pipe[0]);
            if (launch.current_directory) |directory| {
                posix.chdir(directory) catch reportChildFailure(error_pipe[1]);
            }
            pty.childSetup() catch reportChildFailure(error_pipe[1]);
            const exec_error = posix.execvpeZ_expandArg0(
                .no_expand,
                argv.ptr[0].?,
                argv.ptr,
                environment_pointer,
            );
            reportExecFailure(error_pipe[1], exec_error);
        }

        posix.close(error_pipe[1]);
        error_write_open = false;
        pty.closeSlave();
        var marker: [1]u8 = undefined;
        const failure_bytes = posix.read(error_pipe[0], &marker) catch 1;
        posix.close(error_pipe[0]);
        error_read_open = false;
        if (failure_bytes != 0) {
            _ = posix.waitpid(child_id, 0);
            return error.SpawnFailed;
        }
        return .{ .child_id = child_id, .pty = pty };
    }

    pub fn deinit(self: *PosixProcess) void {
        self.pty.deinit();
        self.* = undefined;
    }

    pub fn read(self: *const PosixProcess, buffer: []u8) !usize {
        return self.pty.read(buffer);
    }

    pub fn write(self: *const PosixProcess, bytes: []const u8) !usize {
        return self.pty.write(bytes);
    }

    pub fn resize(self: *const PosixProcess, columns: u16, rows: u16) !void {
        try self.resizePixels(columns, rows, 0, 0);
    }

    pub fn resizePixels(
        self: *const PosixProcess,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !void {
        try self.pty.setSize(.{
            .columns = columns,
            .pixel_height = try pixelExtent(rows, cell_pixel_height),
            .pixel_width = try pixelExtent(columns, cell_pixel_width),
            .rows = rows,
        });
    }

    pub fn terminate(self: *const PosixProcess) !void {
        posix.kill(-self.child_id, posix.SIG.HUP) catch |err| switch (err) {
            error.ProcessNotFound => {},
            else => return err,
        };
    }

    pub fn wait(self: *const PosixProcess) ExitResult {
        const result = posix.waitpid(self.child_id, 0);
        return if (posix.W.IFEXITED(result.status))
            .{ .exit_code = posix.W.EXITSTATUS(result.status) }
        else if (posix.W.IFSIGNALED(result.status))
            .{ .signal = posix.W.TERMSIG(result.status) }
        else
            .{ .unknown = result.status };
    }

    fn reportChildFailure(error_fd: posix.fd_t) noreturn {
        _ = posix.write(error_fd, &.{1}) catch 0;
        posix.exit(127);
    }

    fn reportExecFailure(error_fd: posix.fd_t, exec_error: anyerror) noreturn {
        const marker = [_]u8{@truncate(@intFromError(exec_error))};
        _ = posix.write(error_fd, &marker) catch 0;
        posix.exit(127);
    }
};

const WindowsProcess = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    process_handle: windows.HANDLE,
    pty: pty_module.Pty,

    pub fn spawn(allocator: std.mem.Allocator, launch: Launch) !WindowsProcess {
        var environment = try environmentForLaunch(allocator, launch.environment);
        defer environment.deinit();
        var arena_state = std.heap.ArenaAllocator.init(allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();

        var arguments = try arena.alloc([]const u8, launch.arguments.len + 1);
        arguments[0] = launch.executable;
        @memcpy(arguments[1..], launch.arguments);
        const command_line = try windowsCommandLine(arena, arguments);
        const command_line_w = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_line);
        const environment_w = try std.process.createWindowsEnvBlock(arena, &environment);
        const directory_w = if (launch.current_directory) |directory|
            try std.unicode.utf8ToUtf16LeAllocZ(arena, directory)
        else
            null;

        var pty = try pty_module.Pty.open(.{
            .columns = launch.columns,
            .pixel_height = try pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();

        var attribute_list_size: usize = 0;
        _ = windows_api.InitializeProcThreadAttributeList(null, 1, 0, &attribute_list_size);
        const attribute_list = try arena.alloc(u8, attribute_list_size);
        if (windows_api.InitializeProcThreadAttributeList(
            attribute_list.ptr,
            1,
            0,
            &attribute_list_size,
        ) == 0) return windows.unexpectedError(windows.kernel32.GetLastError());
        defer windows_api.DeleteProcThreadAttributeList(attribute_list.ptr);
        if (windows_api.UpdateProcThreadAttribute(
            attribute_list.ptr,
            0,
            windows_api.PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            pty.pseudo_console,
            @sizeOf(windows_api.HPCON),
            null,
            null,
        ) == 0) return windows.unexpectedError(windows.kernel32.GetLastError());

        var startup_info: windows_api.STARTUPINFOEXW = std.mem.zeroes(windows_api.STARTUPINFOEXW);
        startup_info.StartupInfo.cb = @sizeOf(windows_api.STARTUPINFOEXW);
        startup_info.lpAttributeList = attribute_list.ptr;
        var process_information: windows.PROCESS_INFORMATION = undefined;
        const flags = windows_api.CREATE_UNICODE_ENVIRONMENT |
            windows_api.EXTENDED_STARTUPINFO_PRESENT;
        if (windows_api.CreateProcessW(
            null,
            command_line_w.ptr,
            null,
            null,
            windows.TRUE,
            flags,
            environment_w.ptr,
            if (directory_w) |directory| directory.ptr else null,
            @ptrCast(&startup_info.StartupInfo),
            &process_information,
        ) == 0) {
            return switch (windows.kernel32.GetLastError()) {
                .FILE_NOT_FOUND, .PATH_NOT_FOUND, .BAD_EXE_FORMAT => error.SpawnFailed,
                else => |err| windows.unexpectedError(err),
            };
        }
        _ = windows.CloseHandle(process_information.hThread);
        return .{ .process_handle = process_information.hProcess, .pty = pty };
    }

    pub fn deinit(self: *WindowsProcess) void {
        _ = windows.CloseHandle(self.process_handle);
        self.pty.deinit();
        self.* = undefined;
    }

    pub fn read(self: *const WindowsProcess, buffer: []u8) !usize {
        return self.pty.read(buffer);
    }

    pub fn write(self: *const WindowsProcess, bytes: []const u8) !usize {
        return self.pty.write(bytes);
    }

    pub fn resize(self: *const WindowsProcess, columns: u16, rows: u16) !void {
        try self.resizePixels(columns, rows, 0, 0);
    }

    pub fn resizePixels(
        self: *const WindowsProcess,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !void {
        try self.pty.setSize(.{
            .columns = columns,
            .pixel_height = try pixelExtent(rows, cell_pixel_height),
            .pixel_width = try pixelExtent(columns, cell_pixel_width),
            .rows = rows,
        });
    }

    pub fn terminate(self: *const WindowsProcess) !void {
        if (windows.kernel32.TerminateProcess(self.process_handle, 1) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
    }

    pub fn wait(self: *WindowsProcess) ExitResult {
        _ = windows.kernel32.WaitForSingleObject(self.process_handle, windows.INFINITE);
        var code: windows.DWORD = undefined;
        if (windows.kernel32.GetExitCodeProcess(self.process_handle, &code) == 0) {
            self.pty.closeConsole();
            return .{ .unknown = @intFromEnum(windows.kernel32.GetLastError()) };
        }
        self.pty.closeConsole();
        return .{ .exit_code = code };
    }
};

fn pixelExtent(cells: u16, cell_pixels: u16) !u16 {
    return std.math.mul(u16, cells, cell_pixels);
}

fn environmentForLaunch(
    allocator: std.mem.Allocator,
    delta: []const EnvironmentDelta,
) !std.process.EnvMap {
    var environment = try std.process.getEnvMap(allocator);
    errdefer environment.deinit();
    for (delta) |entry| {
        if (entry.value) |value| {
            try environment.put(entry.name, value);
        } else {
            environment.remove(entry.name);
        }
    }
    return environment;
}

fn windowsCommandLine(allocator: std.mem.Allocator, arguments: []const []const u8) ![:0]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    const writer = &output.writer;
    for (arguments, 0..) |argument, argument_index| {
        if (argument_index != 0) try writer.writeByte(' ');
        const requires_quotes = argument.len == 0 or std.mem.indexOfAny(u8, argument, " \t\n\"") != null;
        if (!requires_quotes) {
            try writer.writeAll(argument);
            continue;
        }
        try writer.writeByte('"');
        var backslashes: usize = 0;
        for (argument) |byte| switch (byte) {
            '\\' => backslashes += 1,
            '"' => {
                try writer.splatByteAll('\\', backslashes * 2 + 1);
                try writer.writeByte('"');
                backslashes = 0;
            },
            else => {
                try writer.splatByteAll('\\', backslashes);
                try writer.writeByte(byte);
                backslashes = 0;
            },
        };
        try writer.splatByteAll('\\', backslashes * 2);
        try writer.writeByte('"');
    }
    return output.toOwnedSliceSentinel(0);
}

test "PTY process preserves exact arguments, environment, and exit" {
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf '%s:%s' \"$ORCHBEE_TEST_VALUE\" \"$1\"",
        "orchbee-shell",
        "argument with spaces",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/S",
        "/C",
        "<nul set /p =%ORCHBEE_TEST_VALUE%:argument with spaces",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const environment = [_]EnvironmentDelta{
        .{ .name = "ORCHBEE_TEST_VALUE", .value = "environment" },
    };
    var process = try Process.spawn(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &environment,
        .executable = executable,
        .rows = 24,
    });
    defer process.deinit();

    var output: [256]u8 = undefined;
    const length = try process.read(&output);
    try std.testing.expect(std.mem.indexOf(u8, output[0..length], "environment:argument with spaces") != null);
    try std.testing.expectEqual(ExitResult{ .exit_code = 0 }, process.wait());
}

test "failed exec leaves no retained child" {
    try std.testing.expectError(
        error.SpawnFailed,
        Process.spawn(std.testing.allocator, .{
            .arguments = &.{},
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = if (builtin.os.tag == .windows)
                "Z:\\orchbee\\does\\not\\exist.exe"
            else
                "/orchbee/does/not/exist",
            .rows = 24,
        }),
    );
}
