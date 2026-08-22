const std = @import("std");
const builtin = @import("builtin");

const process_common = @import("process_common.zig");
const process_session = @import("process_session.zig");
const process_types = @import("process_types.zig");
const pty_module = @import("pty.zig");

const ExitResult = process_types.ExitResult;
const Launch = process_types.Launch;

extern "c" fn orchbee_observe_process_exit(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
pub const SessionIdentityState = process_session.SessionIdentityState;

pub fn foregroundCommandActive(
    shell_process_group: c_int,
    foreground_process_group: c_int,
) bool {
    return foreground_process_group != shell_process_group;
}

pub const Process = if (builtin.os.tag == .windows) void else struct {
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

    pub fn spawn(allocator: std.mem.Allocator, launch: Launch) !Process {
        // FreeBSD must become and verify the process reaper before any terminal
        // leader is forked. Other POSIX adapters validate this as a no-op.
        try process_session.prepareProcessTracking();
        var environment = try process_common.environmentForLaunch(allocator, launch.environment);
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
            .pixel_height = try process_common.pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try process_common.pixelExtent(launch.columns, launch.cell_pixel_width),
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
            process_session.forceTerminateSession(
                child_id,
                process_session.NativeSessionSignaler{},
            ) catch {};
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

    pub fn deinit(self: *Process) void {
        // Reaping is a fallible ownership transition and must have completed in
        // TerminalRuntime.shutdown before this infallible resource destructor.
        std.debug.assert(self.exit_reaped);
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

    pub fn resize(self: *const Process, columns: u16, rows: u16) !void {
        try self.resizePixels(columns, rows, 0, 0);
    }

    pub fn resizePixels(
        self: *const Process,
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
        try self.identity.signal(
            self.child_id,
            process_session.NativeSessionSignaler{},
            posix.SIG.HUP,
        );
    }

    pub fn forceTerminate(self: *Process) !void {
        try self.identity.forceTerminate(
            self.child_id,
            process_session.NativeSessionSignaler{},
        );
    }

    pub fn processTreeContained(self: *Process) !bool {
        return try self.identity.contained(
            self.child_id,
            process_session.NativeSessionQuery{},
        );
    }

    pub fn hasActiveCommand(self: *const Process) !bool {
        return foregroundCommandActive(self.child_id, try self.pty.foregroundProcessGroup());
    }

    pub fn wait(self: *Process) !ExitResult {
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

    pub fn releaseProcessIdentity(self: *Process) !void {
        if (self.exit_reaped) return;
        try self.identity.release(self.child_id, process_session.NativeSessionReaper{});
        self.exit_reaped = true;
    }

    /// True once waitpid has reaped the session leader. After this point the
    /// numeric session identifier is no longer owned and is never consulted for
    /// signaling, containment enumeration, or release again.
    pub fn identityReleased(self: *const Process) bool {
        return self.identity.isReleased();
    }

    /// Counts every session-signaler consultation that passed the identity
    /// fence. Test evidence asserts that a settled identity never increases it.
    pub fn signalsIssued(self: *const Process) u64 {
        return self.identity.signals_issued.load(.monotonic);
    }

    pub fn finishPtyAfterChildExit(_: *Process) !void {}

    pub fn abortRead(self: *Process) !void {
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

    pub fn forceTerminateSession(session_id: posix.pid_t, signaler: anytype) !void {
        return process_session.forceTerminateSession(session_id, signaler);
    }

    pub fn signalStableSessionMembersForTesting(
        session_id: posix.pid_t,
        source: anytype,
        signal_number: u8,
    ) !void {
        return process_session.signalStableSessionMembers(session_id, source, signal_number);
    }

    pub fn countStableSessionMembersForTesting(
        session_id: posix.pid_t,
        source: anytype,
    ) !u32 {
        return process_session.countStableSessionMembers(session_id, source);
    }

    pub fn sessionLiveMembersForTesting(session_id: posix.pid_t) !u32 {
        return process_session.sessionLiveMembers(session_id);
    }

    pub fn countFreebsdReaperMembersForTesting(
        session_id: posix.pid_t,
        query: anytype,
    ) !u32 {
        return process_session.countFreebsdReaperMembers(session_id, query);
    }
};

pub fn executableCandidatesForTesting(
    arena: std.mem.Allocator,
    environment: *const std.process.EnvMap,
    executable: []const u8,
) ![][*:0]const u8 {
    if (builtin.os.tag == .windows) return error.Unsupported;
    return Process.executableCandidates(arena, environment, executable);
}
