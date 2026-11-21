const std = @import("std");
const builtin = @import("builtin");

const process_common = @import("process_common.zig");
const process_types = @import("process_types.zig");
const pty_module = @import("pty.zig");
const termination_module = @import("process_windows_termination.zig");
const ConsoleTree = @import("process_windows_console.zig").ConsoleTree;
const handover = @import("process_windows_handover.zig");
const windows_api = @import("windows_api.zig");
const executable_search = @import("windows_executable_search.zig");
const windowsExecutableCandidate = executable_search.windowsExecutableCandidate;
const joinWindowsPath = executable_search.joinWindowsPath;
pub const windowsSearchCandidates = executable_search.windowsSearchCandidates;

const ExitResult = process_types.ExitResult;
const Launch = process_types.Launch;

/// Establish the Host's child signal policy once, before endpoint admission or
/// concurrent terminal launches. A detached launcher may have inherited or set
/// the process-global ignore bit; clearing it needs no attached console and
/// leaves each future child's raw-mode and handler choices to that child.
pub fn prepareTerminalChildren() !void {
    if (!windows_api.SetConsoleCtrlHandler(null, .FALSE).toBool()) {
        return std.os.windows.unexpectedError(std.os.windows.GetLastError());
    }
}

pub const childCurrentDirectory = @import("windows_command_line.zig").childCurrentDirectory;

pub const Process = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    job_handle: windows.HANDLE,
    process_handle: windows.HANDLE,
    interactive_shell: bool = false,
    launch_kind: process_types.LaunchKind = .other,
    pty: pty_module.Pty,
    termination: termination_module.Termination = .{},
    console: ConsoleTree = .{},
    /// Manual-reset: wakes the wait thread without the program's end, so a terminal that moved to
    /// another Host stops being watched here without any step of its end.
    stop_event: windows.HANDLE,
    /// A settled terminal restored from a record: no program, console, or handle here.
    inert: bool = false,

    /// A held console of a terminal moving here; see `process_windows_handover.zig`.
    pub const Held = handover.Held;
    pub const adopt = handover.adopt;
    pub const drop = handover.drop;
    pub const settled = handover.settled;

    pub fn spawn(allocator: std.mem.Allocator, requested: Launch) !Process {
        var environment = try process_common.environmentForLaunch(allocator, requested.environment);
        defer environment.deinit();
        var arena_state = std.heap.ArenaAllocator.init(allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();
        var launch = requested;
        if (requested.current_directory) |directory|
            launch.current_directory = try childCurrentDirectory(arena, directory);

        const shell_arguments = try @import("powershell_integration.zig").arguments(arena, launch.executable, launch.arguments);
        var arguments = try arena.alloc([]const u8, shell_arguments.len + 1);
        // cmd.exe parses argv0 itself and mistakes forward-slash path segments
        // for switches. Keep application selection and caller arguments literal.
        const executable_argument = try arena.dupe(u8, launch.executable);
        std.mem.replaceScalar(u8, executable_argument, '/', '\\');
        arguments[0] = executable_argument;
        @memcpy(arguments[1..], shell_arguments);
        const batch = @import("windows_command_line.zig").isBatch(executable_argument);
        const command_line = if (batch)
            try @import("windows_command_line.zig").batch(arena, executable_argument, launch.arguments)
        else
            try windowsCommandLine(arena, arguments);
        const command_line_template = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_line);
        const command_line_w = try arena.allocSentinel(u16, command_line_template.len, 0);
        try @import("cmd_integration.zig").markPrompt(arena, &environment, launch.executable, launch.arguments);
        const environment_w = try environment.createWindowsBlock(arena, .{});
        const directory_w = if (launch.current_directory) |directory|
            try std.unicode.utf8ToUtf16LeAllocZ(arena, directory)
        else
            null;

        const stop_event = windows_api.CreateEventW(null, .TRUE, .FALSE, null) orelse
            return windows.unexpectedError(windows.GetLastError());
        errdefer windows.CloseHandle(stop_event);
        var pty = try pty_module.Pty.open(allocator, .{
            .columns = launch.columns,
            .pixel_height = try process_common.pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try process_common.pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();
        const job_handle = windows_api.ob_windows_terminal_job_create() orelse
            return windows.unexpectedError(windows.GetLastError());
        errdefer windows.CloseHandle(job_handle);

        var attribute_list_size: usize = 0;
        _ = windows_api.InitializeProcThreadAttributeList(null, 2, 0, &attribute_list_size);
        const attribute_list = try arena.alloc(u8, attribute_list_size);
        if (!windows_api.InitializeProcThreadAttributeList(
            attribute_list.ptr,
            2,
            0,
            &attribute_list_size,
        ).toBool()) return windows.unexpectedError(windows.GetLastError());
        defer windows_api.DeleteProcThreadAttributeList(attribute_list.ptr);
        if (!windows_api.UpdateProcThreadAttribute(
            attribute_list.ptr,
            0,
            windows_api.PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            pty.pseudoConsole(),
            @sizeOf(windows_api.HPCON),
            null,
            null,
        ).toBool()) return windows.unexpectedError(windows.GetLastError());
        var job_list = [_]windows.HANDLE{job_handle};
        if (!windows_api.UpdateProcThreadAttribute(
            attribute_list.ptr,
            0,
            windows_api.PROC_THREAD_ATTRIBUTE_JOB_LIST,
            @ptrCast(&job_list),
            @sizeOf(@TypeOf(job_list)),
            null,
            null,
        ).toBool()) return windows.unexpectedError(windows.GetLastError());

        var startup_info: windows_api.STARTUPINFOEXW = std.mem.zeroes(windows_api.STARTUPINFOEXW);
        startup_info.StartupInfo.cb = @sizeOf(windows_api.STARTUPINFOEXW);
        startup_info.StartupInfo.dwFlags = windows.STARTF_USESTDHANDLES;
        startup_info.lpAttributeList = attribute_list.ptr;
        var process_information: windows.PROCESS.INFORMATION = undefined;
        const flags = windows_api.CREATE_SUSPENDED |
            windows_api.CREATE_UNICODE_ENVIRONMENT |
            windows_api.EXTENDED_STARTUPINFO_PRESENT;
        const application_names = if (batch) names: {
            // A script may override PATH/ComSpec, never the command processor itself.
            const system = try windowsSystemDirectory(arena);
            const command_processor = try joinWindowsPath(arena, system, "cmd.exe");
            const names = try arena.alloc([:0]u16, 1);
            names[0] = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_processor);
            break :names names;
        } else try executableCandidates(arena, &environment, launch);
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
                .FALSE,
                flags,
                environment_w.slice.ptr,
                if (directory_w) |directory| directory.ptr else null,
                @ptrCast(&startup_info.StartupInfo),
                &process_information,
            ).toBool()) {
                spawned = true;
                break;
            }
            switch (windows.GetLastError()) {
                .FILE_NOT_FOUND, .PATH_NOT_FOUND => continue,
                .BAD_EXE_FORMAT => return error.SpawnFailed,
                else => |err| return windows.unexpectedError(err),
            }
        }
        if (!spawned) return error.SpawnFailed;
        errdefer windows.CloseHandle(process_information.hProcess);
        defer windows.CloseHandle(process_information.hThread);
        if (!windows_api.ob_windows_terminal_job_assign_and_resume(
            job_handle,
            process_information.hProcess,
            process_information.hThread,
        ).toBool()) {
            return windows.unexpectedError(windows.GetLastError());
        }
        return .{
            .job_handle = job_handle,
            .process_handle = process_information.hProcess,
            .interactive_shell = @import("interactive_shell.zig").isInteractive(launch.executable, launch.arguments),
            .launch_kind = @import("interactive_shell.zig").launchKind(launch.executable, launch.arguments),
            .pty = pty,
            .stop_event = stop_event,
        };
    }

    pub fn deinit(self: *Process) void {
        if (self.inert) {
            self.* = undefined;
            return;
        }
        _ = windows.CloseHandle(self.stop_event);
        // Release first: no Keeper still holds the handles closed below; after a shutdown, at once.
        self.termination.keepers.pass();
        self.console.releaseMembers(self.job_handle, self.process_handle, &self.termination);
        _ = windows.CloseHandle(self.process_handle);
        _ = windows.CloseHandle(self.job_handle);
        self.console.deinit();
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

    /// True when the console buffer behind the child now holds only the cursor row.
    pub fn clearConsoleKeepingCursorRow(self: *Process) bool {
        return self.pty.clearKeepingCursorRow();
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

    pub fn terminate(self: *Process) !void {
        try self.termination.terminate(self.process_handle);
    }

    pub fn forceTerminate(self: *Process) !void {
        try self.console.forceTerminate(self.job_handle, self.process_handle, &self.termination);
    }

    /// Release first, without waiting: from now on no Keeper holds the terminal anew, and each
    /// holding it releases it. True once none holds it.
    pub fn releaseKeepers(self: *Process) bool {
        return self.termination.keepers.tryPass();
    }

    pub fn processTreeContained(self: *Process) !bool {
        return self.console.treeContained(self.job_handle, self.process_handle, &self.termination);
    }

    pub fn hasActiveCommand(self: *Process) !bool {
        if (!self.interactive_shell) return true;
        var active_processes: windows.DWORD = 0;
        if (!windows_api.ob_windows_terminal_job_active_processes(
            self.job_handle,
            &active_processes,
        ).toBool()) {
            return error.ProcessObservationFailed;
        }
        const root_state = windows_api.WaitForSingleObject(self.process_handle, 0);
        const root_live = root_state == windows_api.WAIT_TIMEOUT;
        if (!root_live and root_state != windows_api.WAIT_OBJECT_0) return error.ProcessObservationFailed;
        if (root_live and active_processes == 0) return error.ProcessObservationFailed;
        if (active_processes == @intFromBool(root_live)) return false;
        // Another member exists; only one holding the Terminal console is a command.
        return self.console.holdsCommand(self.job_handle, self.process_handle, &self.termination);
    }

    /// The program's exit, or `error.Stopped` once `stopWait` asked the wait to end without it.
    pub fn wait(self: *Process) !ExitResult {
        // Console ownership must stay readable after the root exits.
        try self.console.learnWhileLive(self.process_handle, self.stop_event);
        const handles = [_]windows.HANDLE{ self.process_handle, self.stop_event };
        switch (windows_api.WaitForMultipleObjects(handles.len, &handles, .FALSE, windows_api.INFINITE)) {
            windows_api.WAIT_OBJECT_0 => {},
            windows_api.WAIT_OBJECT_0 + 1 => return error.Stopped,
            else => return error.ProcessObservationFailed,
        }
        var code: windows.DWORD = undefined;
        if (!windows_api.GetExitCodeProcess(self.process_handle, &code).toBool()) {
            return error.ProcessObservationFailed;
        }
        return windowsTerminatedExitResult(code);
    }

    /// Ends `wait` without the program's end; the program and its console are untouched.
    pub fn stopWait(self: *Process) void {
        _ = windows_api.SetEvent(self.stop_event);
    }

    /// No signal waits for the console or is being written.
    pub fn signalsIdle(self: *Process) bool {
        return self.pty.signalsIdle();
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
        // Job is the complete descendant authority, and only its members that
        // hold this console are attached, so closing waits for those alone.
        try self.console.awaitDetached(self.job_handle, self.process_handle, &self.termination);
        // Release first: a Keeper still holding the console would keep it open after the close.
        // Only the wait thread waits here; a shutdown comes once `releaseKeepers` is true.
        self.termination.keepers.pass();
        self.pty.closeConsole();
    }

    pub fn abortRead(self: *Process) !void {
        try self.pty.abortRead();
    }

    fn executableCandidates(
        arena: std.mem.Allocator,
        environment: *const std.process.Environ.Map,
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
        return windows_api.NeedCurrentDirectoryForExePathW(executable_w.ptr).toBool();
    }

    fn windowsApplicationDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows_api.GetModuleFileNameW(null, &buffer, buffer.len);
        if (length == 0 or length >= buffer.len) return error.WindowsPathUnavailable;
        const utf8 = try std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]);
        const separator = std.mem.lastIndexOfAny(u8, utf8, "\\/") orelse return error.WindowsPathUnavailable;
        return utf8[0..separator];
    }

    fn windowsCurrentDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows_api.GetCurrentDirectoryW(buffer.len, &buffer);
        if (length == 0 or length >= buffer.len) return error.WindowsPathUnavailable;
        return std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]);
    }

    fn windowsSystemDirectory(arena: std.mem.Allocator) ![]u8 {
        var buffer: [32768]u16 = undefined;
        const length = windows_api.GetSystemDirectoryW(@ptrCast(&buffer), buffer.len);
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
pub const windowsCommandLine = @import("windows_command_line.zig").executable;
