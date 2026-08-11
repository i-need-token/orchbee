const std = @import("std");
const builtin = @import("builtin");

const pty_module = @import("pty.zig");
const windows_api = @import("windows_api.zig");

extern "c" fn orchbee_signal_process_session(session_id: c_int, signal_number: c_int) c_int;
extern "c" fn orchbee_process_session_live_members(session_id: c_int, count: *u32) c_int;
extern "c" fn orchbee_observe_process_exit(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
extern "c" fn orchbee_reap_process_exit(process_id: c_int) c_int;

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
    // POSIX-only borrowed descriptor used as the child's stdin. The benchmark
    // opens and validates its fixture before launch, then the child inherits it.
    stdin_fd: ?isize = null,
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

/// Serializes session signaling, containment confirmation, and identity release
/// at the POSIX process-identity owner (THCR-006). The numeric session
/// identifier is consulted for signaling only while the launch session leader is
/// retained; once waitpid has released the identity (or the session is confirmed
/// contained), graceful and force requests become idempotent no-ops that never
/// enumerate processes by the numeric identifier. A reused numeric SID can
/// therefore never receive HUP, STOP, KILL, or CONT from a settled terminal.
const SessionIdentityState = if (builtin.os.tag == .windows)
    struct {}
else
    struct {
        mutex: std.Thread.Mutex = .{},
        released: std.atomic.Value(bool) = .init(false),
        signals_issued: std.atomic.Value(u64) = .init(0),

        pub fn signal(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            signaler: anytype,
            signal_number: u8,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            _ = self.signals_issued.fetchAdd(1, .monotonic);
            try signaler.signal(session_id, signal_number);
        }

        pub fn forceTerminate(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            signaler: anytype,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            _ = self.signals_issued.fetchAdd(1, .monotonic);
            try sessionForceTerminate(session_id, signaler);
        }

        pub fn contained(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            query: anytype,
        ) !bool {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return true;
            var count: u32 = 0;
            try query.liveMembers(session_id, &count);
            return count == 0;
        }

        pub fn release(
            self: *SessionIdentityState,
            session_id: std.posix.pid_t,
            reaper: anytype,
        ) !void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.released.load(.acquire)) return;
            try reaper.reap(session_id);
            self.released.store(true, .release);
        }

        pub fn isReleased(self: *const SessionIdentityState) bool {
            return self.released.load(.acquire);
        }
    };

/// STOP narrows the race in which a descendant can create another session
/// member while the platform enumerates it. A partial STOP failure must not
/// leave already-stopped members frozen forever. POSIX-only: the constant
/// table is not meaningful on Windows.
fn sessionForceTerminate(session_id: std.posix.pid_t, signaler: anytype) !void {
    signaler.signal(session_id, std.posix.SIG.STOP) catch {};
    signaler.signal(session_id, std.posix.SIG.KILL) catch |err| {
        signaler.signal(session_id, std.posix.SIG.CONT) catch {};
        return err;
    };
}

const PosixProcess = if (builtin.os.tag == .windows) void else struct {
    const posix = std.posix;
    const default_path = "/usr/local/bin:/bin/:/usr/bin";

    child_id: posix.pid_t,
    exit_reaped: bool = false,
    exit_result: ?ExitResult = null,
    pty: pty_module.Pty,
    // Serialized process-identity fence shared by signaling, containment
    // confirmation, and release. child_id doubles as the session identifier;
    // it is safe to pass to the session signaler only while the leader is
    // retained (see SessionIdentityState).
    identity: SessionIdentityState = .{},

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
        const executable_candidates = try executableCandidates(arena, &environment, launch.executable);

        var pty = try pty_module.Pty.open(.{
            .columns = launch.columns,
            .pixel_height = try pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();
        var child_stdin_fd: ?posix.fd_t = null;
        if (launch.stdin_fd) |raw_fd| {
            const duplicated = try duplicateCloexec(@intCast(raw_fd));
            child_stdin_fd = @intCast(duplicated);
        }
        errdefer if (child_stdin_fd) |fd| posix.close(fd);
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
            if (child_stdin_fd) |fd| {
                posix.dup2(fd, posix.STDIN_FILENO) catch reportChildFailure(error_pipe[1]);
            }
            const exec_error = execWithEffectivePath(
                launch.executable,
                argv,
                environment_pointer,
                executable_candidates,
            );
            reportExecFailure(error_pipe[1], exec_error);
        }

        posix.close(error_pipe[1]);
        error_write_open = false;
        if (child_stdin_fd) |fd| {
            posix.close(fd);
            child_stdin_fd = null;
        }
        pty.closeSlave();
        var marker: [1]u8 = undefined;
        const failure_bytes = posix.read(error_pipe[0], &marker) catch {
            // Losing the exec-status channel makes launch outcome unknowable.
            // Contain both a child that already created its session and one
            // that failed before setsid, then reap it.
            forceTerminateSession(child_id, NativeSessionSignaler{}) catch {};
            posix.kill(child_id, posix.SIG.KILL) catch {};
            _ = posix.waitpid(child_id, 0);
            return error.SpawnFailed;
        };
        posix.close(error_pipe[0]);
        error_read_open = false;
        if (failure_bytes != 0) {
            _ = posix.waitpid(child_id, 0);
            return error.SpawnFailed;
        }
        return .{ .child_id = child_id, .pty = pty, .identity = .{} };
    }

    pub fn deinit(self: *PosixProcess) void {
        // Reaping is a fallible ownership transition and must have completed in
        // TerminalRuntime.shutdown before this infallible resource destructor.
        std.debug.assert(self.exit_reaped);
        self.pty.deinit();
        self.* = undefined;
    }

    pub fn read(self: *PosixProcess, buffer: []u8) !usize {
        return self.pty.read(buffer);
    }

    pub fn registerReaderThread(self: *PosixProcess, thread: std.Thread.Handle) void {
        self.pty.registerReaderThread(thread);
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

    pub fn terminate(self: *PosixProcess) !void {
        try self.identity.signal(self.child_id, NativeSessionSignaler{}, posix.SIG.HUP);
    }

    pub fn forceTerminate(self: *PosixProcess) !void {
        try self.identity.forceTerminate(self.child_id, NativeSessionSignaler{});
    }

    pub fn processTreeContained(self: *PosixProcess) !bool {
        return try self.identity.contained(self.child_id, NativeSessionQuery{});
    }

    pub fn wait(self: *PosixProcess) !ExitResult {
        if (self.exit_result) |result| return result;
        var result_kind: c_int = 0;
        var result_value: u32 = 0;
        const observe_error = orchbee_observe_process_exit(
            self.child_id,
            &result_kind,
            &result_value,
        );
        if (observe_error != 0) return error.ProcessObservationFailed;
        const result: ExitResult = switch (result_kind) {
            1 => .{ .exit_code = result_value },
            2 => .{ .signal = result_value },
            else => .{ .unknown = result_value },
        };
        self.exit_result = result;
        return result;
    }

    pub fn releaseProcessIdentity(self: *PosixProcess) !void {
        if (self.exit_reaped) return;
        try self.identity.release(self.child_id, NativeSessionReaper{});
        self.exit_reaped = true;
    }

    /// True once waitpid has reaped the session leader. After this point the
    /// numeric session identifier is no longer owned and is never consulted for
    /// signaling, containment enumeration, or release again.
    pub fn identityReleased(self: *const PosixProcess) bool {
        return self.identity.isReleased();
    }

    /// Counts every session-signaler consultation that passed the identity
    /// fence. Test evidence asserts that a settled identity never increases it.
    pub fn signalsIssued(self: *const PosixProcess) u64 {
        return self.identity.signals_issued.load(.monotonic);
    }

    pub fn finishPtyAfterChildExit(_: *PosixProcess) !void {}

    pub fn abortRead(self: *PosixProcess) !void {
        try self.pty.abortRead();
    }

    fn reportChildFailure(error_fd: posix.fd_t) noreturn {
        _ = posix.write(error_fd, &.{1}) catch 0;
        exitChildFailure();
    }

    fn reportExecFailure(error_fd: posix.fd_t, exec_error: anyerror) noreturn {
        const marker = [_]u8{@truncate(@intFromError(exec_error))};
        _ = posix.write(error_fd, &marker) catch 0;
        exitChildFailure();
    }

    fn exitChildFailure() noreturn {
        if (builtin.link_libc) {
            std.c._exit(127);
        } else {
            posix.exit(127);
        }
    }

    fn duplicateCloexec(fd: posix.fd_t) !posix.fd_t {
        // Zig's std.c.F table does not expose Linux's platform-specific
        // F_DUPFD_CLOEXEC value on every cross target. Keep the operation
        // atomic (rather than dup + SETFD, which races another fork) and use
        // the values from each supported platform's fcntl headers.
        const command: i32 = switch (builtin.os.tag) {
            .linux => 1030,
            .freebsd => 17,
            .macos => 67,
            else => unreachable,
        };
        return @intCast(try posix.fcntl(fd, command, 3));
    }

    fn executableCandidates(
        arena: std.mem.Allocator,
        environment: *const std.process.EnvMap,
        executable: []const u8,
    ) ![][*:0]const u8 {
        if (std.mem.indexOfScalar(u8, executable, '/') != null) return &.{};
        const path = environment.get("PATH") orelse default_path;
        var count: usize = 0;
        // POSIX PATH treats an empty component as the child current
        // directory. Preserve those components instead of using
        // tokenizeScalar, which silently drops leading, trailing, and
        // repeated separators. The child has already chdir'ed before it
        // executes this plan, so `./name` keeps the lookup relative to the
        // requested launch directory without consulting the host PATH.
        var iterator = std.mem.splitScalar(u8, path, ':');
        while (iterator.next()) |_| count += 1;
        const candidates = try arena.alloc([*:0]const u8, count);
        iterator = std.mem.splitScalar(u8, path, ':');
        for (candidates) |*candidate| {
            const directory = iterator.next().?;
            const prefix = if (directory.len == 0) "." else directory;
            const separator = if (std.mem.endsWith(u8, prefix, "/")) "" else "/";
            const length = std.math.add(
                usize,
                prefix.len + separator.len,
                executable.len,
            ) catch
                return error.NameTooLong;
            if (length >= posix.PATH_MAX) return error.NameTooLong;
            const path_buffer = try arena.alloc(u8, length + 1);
            @memcpy(path_buffer[0..prefix.len], prefix);
            @memcpy(path_buffer[prefix.len..][0..separator.len], separator);
            @memcpy(path_buffer[prefix.len + separator.len ..][0..executable.len], executable);
            path_buffer[length] = 0;
            candidate.* = @ptrCast(path_buffer.ptr);
        }
        return candidates;
    }

    fn execWithEffectivePath(
        executable: []const u8,
        argv: [*:null]const ?[*:0]const u8,
        environment: [*:null]const ?[*:0]const u8,
        candidates: []const [*:0]const u8,
    ) posix.ExecveError {
        if (std.mem.indexOfScalar(u8, executable, '/') != null) {
            return posix.execveZ(argv[0].?, argv, environment);
        }
        var seen_access_denied = false;
        var last_error: posix.ExecveError = error.FileNotFound;
        for (candidates) |candidate| {
            last_error = posix.execveZ(candidate, argv, environment);
            switch (last_error) {
                error.AccessDenied => seen_access_denied = true,
                error.FileNotFound, error.NotDir => {},
                else => |err| return err,
            }
        }
        if (seen_access_denied) return error.AccessDenied;
        return last_error;
    }

    fn signalSession(session_id: posix.pid_t, signal_number: u8) !void {
        if (orchbee_signal_process_session(session_id, signal_number) != 0) {
            return error.SessionTerminationFailed;
        }
    }

    const NativeSessionSignaler = struct {
        fn signal(_: NativeSessionSignaler, session_id: posix.pid_t, signal_number: u8) !void {
            try signalSession(session_id, signal_number);
        }
    };

    const NativeSessionQuery = struct {
        fn liveMembers(_: NativeSessionQuery, session_id: posix.pid_t, count: *u32) !void {
            if (orchbee_process_session_live_members(session_id, count) != 0) {
                return error.ProcessContainmentQueryFailed;
            }
        }
    };

    const NativeSessionReaper = struct {
        fn reap(_: NativeSessionReaper, session_id: posix.pid_t) !void {
            if (orchbee_reap_process_exit(session_id) != 0) return error.ProcessReapFailed;
        }
    };

    pub fn forceTerminateSession(session_id: posix.pid_t, signaler: anytype) !void {
        return sessionForceTerminate(session_id, signaler);
    }
};

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

fn windowsSearchCandidates(
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

const WindowsProcess = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    job_handle: windows.HANDLE,
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
        const command_line_template = try std.unicode.utf8ToUtf16LeAllocZ(arena, command_line);
        const command_line_w = try arena.allocSentinel(u16, command_line_template.len, 0);
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

    pub fn deinit(self: *WindowsProcess) void {
        _ = windows.CloseHandle(self.process_handle);
        _ = windows.CloseHandle(self.job_handle);
        self.pty.deinit();
        self.* = undefined;
    }

    pub fn read(self: *WindowsProcess, buffer: []u8) !usize {
        return self.pty.read(buffer);
    }

    pub fn registerReaderThread(self: *WindowsProcess, thread: std.Thread.Handle) void {
        self.pty.registerReaderThread(thread);
    }

    pub fn write(self: *const WindowsProcess, bytes: []const u8) !usize {
        return self.pty.write(bytes);
    }

    pub fn resize(self: *WindowsProcess, columns: u16, rows: u16) !void {
        try self.resizePixels(columns, rows, 0, 0);
    }

    pub fn resizePixels(
        self: *WindowsProcess,
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
            const terminate_error = windows.kernel32.GetLastError();
            if (windows.kernel32.WaitForSingleObject(self.process_handle, 0) ==
                windows.WAIT_OBJECT_0)
            {
                return;
            }
            return windows.unexpectedError(terminate_error);
        }
    }

    pub fn forceTerminate(self: *const WindowsProcess) !void {
        if (windows_api.ob_windows_terminal_job_terminate(self.job_handle, 1) == 0) {
            return windows.unexpectedError(windows.kernel32.GetLastError());
        }
    }

    pub fn processTreeContained(self: *const WindowsProcess) !bool {
        var active_processes: windows.DWORD = 0;
        if (windows_api.ob_windows_terminal_job_active_processes(
            self.job_handle,
            &active_processes,
        ) == 0) {
            return error.ProcessContainmentQueryFailed;
        }
        return active_processes == 0;
    }

    pub fn wait(self: *WindowsProcess) !ExitResult {
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

    pub fn releaseProcessIdentity(_: *WindowsProcess) !void {}

    pub fn identityReleased(_: *const WindowsProcess) bool {
        return true;
    }

    pub fn signalsIssued(_: *const WindowsProcess) u64 {
        return 0;
    }

    pub fn finishPtyAfterChildExit(self: *WindowsProcess) !void {
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

    pub fn abortRead(self: *WindowsProcess) !void {
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
fn windowsTerminatedExitResult(code: u32) !ExitResult {
    return .{ .exit_code = code };
}

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

pub const testing = if (builtin.is_test) struct {
    pub const posixExecutableCandidatesFn = posixExecutableCandidatesForTesting;
    pub const PosixProcessType = PosixProcess;
    pub const SessionIdentityStateType = SessionIdentityState;
    pub const windowsCommandLineFn = windowsCommandLine;
    pub const windowsSearchCandidatesFn = windowsSearchCandidates;
    pub const windowsTerminatedExitResultFn = windowsTerminatedExitResult;
} else struct {};

fn posixExecutableCandidatesForTesting(
    arena: std.mem.Allocator,
    environment: *const std.process.EnvMap,
    executable: []const u8,
) ![][*:0]const u8 {
    if (builtin.os.tag == .windows) return error.Unsupported;
    return PosixProcess.executableCandidates(arena, environment, executable);
}
