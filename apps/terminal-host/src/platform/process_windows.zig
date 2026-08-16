const std = @import("std");
const builtin = @import("builtin");

const process_common = @import("process_common.zig");
const process_types = @import("process_types.zig");
const pty_module = @import("pty.zig");
const windows_api = @import("windows_api.zig");

const ExitResult = process_types.ExitResult;
const Launch = process_types.Launch;
fn windowsExecutableCandidate(
    arena: std.mem.Allocator,
    candidate: []const u8,
    executable: []const u8,
) ![:0]u16 {
    // CreateProcess supplies the implicit .exe suffix only for a bare
    // executable name. A qualified name such as `tools\\probe` is an
    // exact path candidate and must not silently become `tools\\probe.exe`.
    const can_append_extension = std.mem.indexOfAny(u8, executable, "\\/") == null;
    const with_extension = if (can_append_extension and
        std.mem.indexOfScalar(u8, executable, '.') == null)
        try std.fmt.allocPrint(arena, "{s}.exe", .{candidate})
    else
        candidate;
    return std.unicode.utf8ToUtf16LeAllocZ(arena, with_extension);
}

fn joinWindowsPath(arena: std.mem.Allocator, directory: []const u8, name: []const u8) ![]u8 {
    if (directory.len == 0) return arena.dupe(u8, name);
    // A trailing drive designator keeps drive-relative syntax: `C:` joined with
    // `probe` is `C:probe` (the drive's current directory), never `C:\probe`.
    const separator = if (directory[directory.len - 1] == '\\' or
        directory[directory.len - 1] == '/' or
        directory[directory.len - 1] == ':')
        ""
    else
        "\\";
    return std.fmt.allocPrint(arena, "{s}{s}{s}", .{ directory, separator, name });
}

const WindowsPathKind = enum { empty, drive_relative, ordinary_relative, absolute };

fn windowsPathKind(directory: []const u8) WindowsPathKind {
    if (directory.len == 0) return .empty;
    if (std.fs.path.isAbsoluteWindows(directory)) return .absolute;
    if (directory.len >= 2 and std.ascii.isAlphabetic(directory[0]) and directory[1] == ':')
        return .drive_relative;
    return .ordinary_relative;
}

pub fn windowsSearchCandidates(
    arena: std.mem.Allocator,
    executable: []const u8,
    application_directory: ?[]const u8,
    parent_current_directory: ?[]const u8,
    system_directory: ?[]const u8,
    include_current_directory: bool,
    path: ?[]const u8,
) ![][:0]u16 {
    var candidates: std.ArrayList([:0]u16) = .empty;
    if (application_directory) |directory| {
        try candidates.append(arena, try windowsExecutableCandidate(
            arena,
            try joinWindowsPath(arena, directory, executable),
            executable,
        ));
    }
    if (include_current_directory) {
        if (parent_current_directory) |directory| {
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, directory, executable),
                executable,
            ));
        }
    }
    if (system_directory) |directory| {
        try candidates.append(arena, try windowsExecutableCandidate(
            arena,
            try joinWindowsPath(arena, directory, executable),
            executable,
        ));
        if (std.mem.lastIndexOfAny(u8, directory, "\\/")) |separator| {
            const windows_directory = directory[0..separator];
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, windows_directory, "System"),
                executable,
            ));
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, windows_directory, executable),
                executable,
            ));
        }
    }
    // A missing PATH contributes no PATH candidates. Its absence has no
    // current-directory semantics of its own, so a policy-excluded current
    // directory cannot be reintroduced through a synthesized empty component.
    if (path) |effective_path| {
        var iterator = std.mem.splitScalar(u8, effective_path, ';');
        while (iterator.next()) |directory| {
            const path_kind = windowsPathKind(directory);
            if (path_kind == .empty) {
                // An explicitly empty PATH component means the current
                // directory, matching CreateProcess's PATH search. It is a
                // supplied entry, not the policy-controlled implicit slot.
                if (parent_current_directory) |parent| {
                    const path_candidate = try joinWindowsPath(arena, parent, executable);
                    try candidates.append(arena, try windowsExecutableCandidate(arena, path_candidate, executable));
                }
                continue;
            }
            const base: []const u8 = switch (path_kind) {
                // A drive-relative entry such as `C:tools` is relative to the
                // drive's current directory, never to the parent process
                // current directory, so it must not be prefixed with it.
                .drive_relative, .absolute => directory,
                .ordinary_relative => if (parent_current_directory) |parent|
                    try joinWindowsPath(arena, parent, directory)
                else
                    continue,
                .empty => unreachable,
            };
            const path_candidate = try joinWindowsPath(arena, base, executable);
            try candidates.append(arena, try windowsExecutableCandidate(arena, path_candidate, executable));
        }
    }
    return candidates.toOwnedSlice(arena);
}

pub const Process = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    job_handle: windows.HANDLE,
    process_handle: windows.HANDLE,
    pty: pty_module.Pty,

    pub fn spawn(allocator: std.mem.Allocator, launch: Launch) !Process {
        var environment = try process_common.environmentForLaunch(allocator, launch.environment);
        defer environment.deinit();
        var arena_state = std.heap.ArenaAllocator.init(allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();

        var arguments = try arena.alloc([]const u8, launch.arguments.len + 1);
        arguments[0] = launch.executable;
        @memcpy(arguments[1..], launch.arguments);
        const command_line = try windowsCommandLine(arena, arguments);
        const command_line_template = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_line);
        const command_line_w = try arena.allocSentinel(u16, command_line_template.len, 0);
        const environment_w = try std.process.createWindowsEnvBlock(arena, &environment);
        const directory_w = if (launch.current_directory) |directory|
            try std.unicode.utf8ToUtf16LeAllocZ(arena, directory)
        else
            null;

        var pty = try pty_module.Pty.open(.{
            .columns = launch.columns,
            .pixel_height = try process_common.pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try process_common.pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();
        const job_handle = windows_api.ob_windows_terminal_job_create() orelse
            return windows.unexpectedError(windows.kernel32.GetLastError());
        errdefer windows.CloseHandle(job_handle);

        var attribute_list_size: usize = 0;
        _ = windows_api.InitializeProcThreadAttributeList(null, 2, 0, &attribute_list_size);
        const attribute_list = try arena.alloc(u8, attribute_list_size);
        if (windows_api.InitializeProcThreadAttributeList(
            attribute_list.ptr,
            2,
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
        var job_list = [_]windows.HANDLE{job_handle};
        if (windows_api.UpdateProcThreadAttribute(
            attribute_list.ptr,
            0,
            windows_api.PROC_THREAD_ATTRIBUTE_JOB_LIST,
            @ptrCast(&job_list),
            @sizeOf(@TypeOf(job_list)),
            null,
            null,
        ) == 0) return windows.unexpectedError(windows.kernel32.GetLastError());

        var startup_info: windows_api.STARTUPINFOEXW = std.mem.zeroes(windows_api.STARTUPINFOEXW);
        startup_info.StartupInfo.cb = @sizeOf(windows_api.STARTUPINFOEXW);
        startup_info.lpAttributeList = attribute_list.ptr;
        var process_information: windows.PROCESS_INFORMATION = undefined;
        const flags = windows_api.CREATE_SUSPENDED |
            windows_api.CREATE_UNICODE_ENVIRONMENT |
            windows_api.EXTENDED_STARTUPINFO_PRESENT;
        const application_names = try executableCandidates(arena, &environment, launch);
        var spawned = false;
        for (application_names) |application_name| {
            // CreateProcessW may modify its command-line buffer even when the
            // launch attempt fails. PATH resolution can try several application
            // candidates, so restore the exact command for every attempt.
            @memcpy(command_line_w, command_line_template);
            if (windows_api.CreateProcessW(
                application_name.ptr,
                command_line_w.ptr,
                null,
                null,
                windows.FALSE,
                flags,
                environment_w.ptr,
                if (directory_w) |directory| directory.ptr else null,
                @ptrCast(&startup_info.StartupInfo),
                &process_information,
            ) != 0) {
                spawned = true;
                break;
            }
            switch (windows.kernel32.GetLastError()) {
                .FILE_NOT_FOUND, .PATH_NOT_FOUND => continue,
                .BAD_EXE_FORMAT => return error.SpawnFailed,
                else => |err| return windows.unexpectedError(err),
            }
        }
        if (!spawned) return error.SpawnFailed;
        errdefer windows.CloseHandle(process_information.hProcess);
        defer windows.CloseHandle(process_information.hThread);
        if (windows_api.ob_windows_terminal_job_assign_and_resume(
            job_handle,
            process_information.hProcess,
            process_information.hThread,
        ) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
        return .{
            .job_handle = job_handle,
            .process_handle = process_information.hProcess,
            .pty = pty,
        };
    }

    pub fn deinit(self: *Process) void {
        _ = windows.CloseHandle(self.process_handle);
        _ = windows.CloseHandle(self.job_handle);
        self.pty.deinit();
        self.* = undefined;
    }

    pub fn read(self: *Process, buffer: []u8) !usize {
        return self.pty.read(buffer);
    }

    pub fn registerReaderThread(self: *Process, thread: std.Thread.Handle) void {
        self.pty.registerReaderThread(thread);
    }

    pub fn write(self: *const Process, bytes: []const u8) !usize {
        return self.pty.write(bytes);
    }

    pub fn resize(self: *Process, columns: u16, rows: u16) !void {
        try self.resizePixels(columns, rows, 0, 0);
    }

    pub fn resizePixels(
        self: *Process,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !void {
        try self.pty.setSize(.{
            .columns = columns,
            .pixel_height = try process_common.pixelExtent(rows, cell_pixel_height),
            .pixel_width = try process_common.pixelExtent(columns, cell_pixel_width),
            .rows = rows,
        });
    }

    pub fn terminate(self: *const Process) !void {
        if (windows.kernel32.TerminateProcess(self.process_handle, 1) == 0) {
            const terminate_error = windows.kernel32.GetLastError();
            if (windows.kernel32.WaitForSingleObject(self.process_handle, 0) ==
                windows.WAIT_OBJECT_0)
            {
                return;
            }
            return windows.unexpectedError(terminate_error);
        }
    }

    pub fn forceTerminate(self: *const Process) !void {
        if (windows_api.ob_windows_terminal_job_terminate(self.job_handle, 1) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
    }

    pub fn processTreeContained(self: *const Process) !bool {
        var active_processes: windows.DWORD = 0;
        if (windows_api.ob_windows_terminal_job_active_processes(
            self.job_handle,
            &active_processes,
        ) == 0) {
            return error.ProcessContainmentQueryFailed;
        }
        return active_processes == 0;
    }

    pub fn hasActiveCommand(self: *const Process) !bool {
        var active_processes: windows.DWORD = 0;
        if (windows_api.ob_windows_terminal_job_active_processes(
            self.job_handle,
            &active_processes,
        ) == 0) {
            return error.ProcessObservationFailed;
        }
        return active_processes > 1;
    }

    pub fn wait(self: *Process) !ExitResult {
        if (windows.kernel32.WaitForSingleObject(self.process_handle, windows.INFINITE) !=
            windows.WAIT_OBJECT_0)
        {
            return error.ProcessObservationFailed;
        }
        var code: windows.DWORD = undefined;
        if (windows.kernel32.GetExitCodeProcess(self.process_handle, &code) == 0) {
            return error.ProcessObservationFailed;
        }
        return windowsTerminatedExitResult(code);
    }

    pub fn releaseProcessIdentity(_: *Process) !void {}

    pub fn identityReleased(_: *const Process) bool {
        return true;
    }

    pub fn signalsIssued(_: *const Process) u64 {
        return 0;
    }

    pub fn finishPtyAfterChildExit(self: *Process) !void {
        // Holding HPCON keeps the pseudoconsole alive, but closing it while a
        // descendant is still attached sends that descendant CTRL_CLOSE_EVENT
        // and can block indefinitely on Windows versions before 11 24H2. The
        // Job is the complete descendant authority, so zero active processes is
        // the first safe point at which to close the pseudoconsole.
        while (true) {
            var active_processes: windows.DWORD = 0;
            if (windows_api.ob_windows_terminal_job_active_processes(
                self.job_handle,
                &active_processes,
            ) == 0) {
                return error.ProcessObservationFailed;
            }
            if (active_processes == 0) break;
            std.Thread.sleep(10 * std.time.ns_per_ms);
        }
        self.pty.closeConsole();
    }

    pub fn abortRead(self: *Process) !void {
        try self.pty.abortRead();
    }

    fn executableCandidates(
        arena: std.mem.Allocator,
        environment: *const std.process.EnvMap,
        launch: Launch,
    ) ![][:0]u16 {
        if (std.mem.indexOfAny(u8, launch.executable, "\\/") != null) {
            const candidate = if (launch.current_directory) |directory|
                if (!std.fs.path.isAbsolute(launch.executable))
                    try joinWindowsPath(arena, directory, launch.executable)
                else
                    launch.executable
            else
                launch.executable;
            var candidates: std.ArrayList([:0]u16) = .empty;
            try candidates.append(arena, try windowsExecutableCandidate(arena, candidate, launch.executable));
            return candidates.toOwnedSlice(arena);
        }

        // Match CreateProcess's non-PATH precedence without allowing the host
        // process's PATH to leak into a launch that supplied an effective one.
        const include_current_directory = try windowsNeedsCurrentDirectory(arena, launch.executable);
        // The NeedCurrentDirectoryForExePathW policy controls the implicit
        // current-directory slot in CreateProcess's search order. Relative
        // entries that the child explicitly supplied in PATH still resolve
        // against the parent current directory regardless of that policy.
        const parent_current_directory = windowsCurrentDirectory(arena) catch null;
        return windowsSearchCandidates(
            arena,
            launch.executable,
            windowsApplicationDirectory(arena) catch null,
            parent_current_directory,
            windowsSystemDirectory(arena) catch null,
            include_current_directory,
            environment.get("PATH"),
        );
    }

    fn windowsNeedsCurrentDirectory(arena: std.mem.Allocator, executable: []const u8) !bool {
        const executable_w = try std.unicode.utf8ToUtf16LeAllocZ(arena, executable);
        return windows_api.NeedCurrentDirectoryForExePathW(executable_w.ptr) != 0;
    }

    fn windowsApplicationDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows.kernel32.GetModuleFileNameW(null, &buffer, buffer.len);
        if (length == 0 or length >= buffer.len) return error.WindowsPathUnavailable;
        const utf8 = try std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]);
        const separator = std.mem.lastIndexOfAny(u8, utf8, "\\/") orelse return error.WindowsPathUnavailable;
        return utf8[0..separator];
    }

    fn windowsCurrentDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows.kernel32.GetCurrentDirectoryW(buffer.len, &buffer);
        if (length == 0 or length >= buffer.len) return error.WindowsPathUnavailable;
        return std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]);
    }

    fn windowsSystemDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows.kernel32.GetSystemDirectoryW(@ptrCast(&buffer), buffer.len);
        if (length == 0 or length >= buffer.len) return error.WindowsPathUnavailable;
        return std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]);
    }
};

/// Classifies the DWORD that GetExitCodeProcess returned after
/// WaitForSingleObject reported WAIT_OBJECT_0 on the child process handle. A
/// signaled wait proves the process object has terminated, so every returned
/// DWORD is the process's actual exit status and participates in normal
/// completion (TH-008). STILL_ACTIVE (259) describes a process that is still
/// running; a terminated process may legitimately exit with status 259, so the
/// value is preserved as `exit_code` rather than rejected as observation
/// failure (THCR-041). A failed wait or failed GetExitCodeProcess call is
/// mapped to `ProcessObservationFailed` by the caller, never by this
/// classification.
pub fn windowsTerminatedExitResult(code: u32) !ExitResult {
    return .{ .exit_code = code };
}
pub fn windowsCommandLine(allocator: std.mem.Allocator, arguments: []const []const u8) ![:0]u8 {
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
