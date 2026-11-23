const std = @import("std");
const builtin = @import("builtin");

const fork_launch = @import("fork_launch_posix.zig");
const process_common = @import("process_common.zig");
const process_sample = @import("process_sample.zig");
const posix_compat = @import("posix_compat.zig");
const process_session = @import("process_session.zig");
const process_types = @import("process_types.zig");
const pty_module = @import("pty.zig");
const keeper_record = if (builtin.os.tag == .macos) @import("process_keeper_darwin.zig") else struct {};
const handover = @import("process_posix_handover.zig");
const is_macos = builtin.os.tag == .macos;

const ExitResult = process_types.ExitResult;
const Launch = process_types.Launch;

extern "c" fn orchbee_observe_process_exit(
    process_id: c_int,
    result_kind: *c_int,
    result_value: *u32,
) c_int;
extern "c" fn getpgid(pid: c_int) c_int;
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
    allocator: std.mem.Allocator,
    /// What the macOS Keeper links read: the release-first gate and the facts a hold names.
    keeper: if (builtin.os.tag == .macos) *keeper_record.Record else void,
    /// A recovered terminal's leader, which is not this Host's child: a Keeper parents it, or
    /// nobody here does.
    adopted: bool = false,
    /// A settled terminal restored from a record: nothing here is open, watched, or ended.
    inert: bool = false,
    interactive_shell: bool = false,
    launch_kind: process_types.LaunchKind = .other,
    exit_reaped: bool = false,
    exit_result: ?ExitResult = null,
    pty: pty_module.Pty,
    // Serialized process-identity fence shared by signaling, containment
    // confirmation, and release. child_id doubles as the session identifier;
    // it is safe to pass to the session signaler only while the leader is
    // retained (see SessionIdentityState).
    identity: SessionIdentityState = .{},

    /// A terminal that moves here or leaves; see `process_posix_handover.zig`.
    pub const Held = handover.Held;
    pub const adopt = handover.adopt;
    pub const stopWait = handover.stopWait;
    pub const drop = handover.drop;
    pub const settled = handover.settled;

    pub fn spawn(allocator: std.mem.Allocator, launch: Launch) !Process {
        // FreeBSD must become and verify the process reaper before any terminal
        // leader is forked. Other POSIX adapters validate this as a no-op.
        try process_session.prepareProcessTracking();
        var environment = try process_common.environmentForLaunch(allocator, launch.environment);
        defer environment.deinit();
        var arena_state = std.heap.ArenaAllocator.init(allocator);
        defer arena_state.deinit();
        const arena = arena_state.allocator();

        const integration = try @import("posix_shell_integration.zig").prepare(
            arena,
            &environment,
            launch.executable,
            launch.arguments,
        );
        const argument_count = integration.arguments.len + 1;
        const argv = try arena.allocSentinel(?[*:0]const u8, argument_count, null);
        argv[0] = (try arena.dupeZ(u8, launch.executable)).ptr;
        for (integration.arguments, 1..) |argument, index| {
            argv[index] = (try arena.dupeZ(u8, argument)).ptr;
        }
        const environment_pointer = (try environment.createPosixBlock(arena, .{})).slice.ptr;
        const executable_candidates = try executableCandidates(arena, &environment, launch.executable);

        var pty = try pty_module.Pty.open(allocator, .{
            .columns = launch.columns,
            .pixel_height = try process_common.pixelExtent(launch.rows, launch.cell_pixel_height),
            .pixel_width = try process_common.pixelExtent(launch.columns, launch.cell_pixel_width),
            .rows = launch.rows,
        });
        errdefer pty.deinit();
        const keeper = if (comptime builtin.os.tag == .macos) try keeper_record.Record.create(allocator, pty.master) else {};
        errdefer if (comptime builtin.os.tag == .macos) keeper.destroy(allocator);
        var child_stdin_fd: ?posix.fd_t = null;
        if (launch.stdin_fd) |raw_fd| {
            const duplicated = try duplicateCloexec(@intCast(raw_fd));
            child_stdin_fd = @intCast(duplicated);
        }
        errdefer if (child_stdin_fd) |fd| {
            _ = posix.system.close(fd);
        };
        const plan: fork_launch.Plan = .{
            .executable = launch.executable,
            .argv = argv,
            .environment = environment_pointer,
            .candidates = executable_candidates,
            .directory = launch.current_directory,
            .streams = .{ .pty = .{ .master = pty.master, .slave = pty.slave.?, .input = child_stdin_fd } },
        };
        const child_id = if (comptime builtin.os.tag == .macos)
            try startThroughKeeper(launch, keeper, plan, integration.launch_kind)
        else
            try fork_launch.start(plan);
        if (child_stdin_fd) |fd| {
            _ = posix.system.close(fd);
            child_stdin_fd = null;
        }
        pty.closeSlave();
        if (comptime builtin.os.tag == .macos) if (!keeper.isParented()) keeper.started(child_id);
        return .{
            .child_id = child_id,
            .allocator = allocator,
            .keeper = keeper,
            .pty = pty,
            .identity = .{},
            .interactive_shell = @import("interactive_shell.zig").isInteractive(launch.executable, launch.arguments),
            .launch_kind = integration.launch_kind,
        };
    }

    /// Whether the session identifier still names the leader's session: a process keeps the ended
    /// leader, or the leader runs with its recorded start time. A recovered leader nobody parents,
    /// and one whose parent released it or ended, is reaped by another process once it ends.
    fn sessionNamed(self: *Process) bool {
        if (comptime !is_macos) return true;
        return self.keeper.keepsLeader(self.adopted) or self.keeper.leaderRuns(self.child_id);
    }

    pub fn deinit(self: *Process) void {
        if (self.inert) {
            self.* = undefined;
            return;
        }
        // Reaping is a fallible ownership transition and must have completed in
        // TerminalRuntime.shutdown before this infallible resource destructor.
        std.debug.assert(self.exit_reaped);
        if (comptime builtin.os.tag == .macos) self.keeper.destroy(self.allocator);
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

    /// A POSIX PTY keeps no screen buffer of its own, so there is nothing to clear.
    pub fn clearConsoleKeepingCursorRow(_: *Process) bool {
        return false;
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
        if (!self.sessionNamed()) return;
        try self.identity.signal(
            self.child_id,
            process_session.NativeSessionSignaler{},
            posix.SIG.HUP,
        );
    }

    pub fn forceTerminate(self: *Process) !void {
        if (!self.sessionNamed()) return;
        try self.identity.forceTerminate(
            self.child_id,
            process_session.NativeSessionSignaler{},
        );
    }

    pub fn processTreeContained(self: *Process) !bool {
        if (!self.sessionNamed()) return true;
        return try self.identity.contained(
            self.child_id,
            process_session.NativeSessionQuery{},
        );
    }

    /// Samples the session the root leads, inside the identity fence. Once the
    /// leader is reaped its session identifier may name a later session, so the
    /// sample keeps the root PID and lists no members.
    pub fn sampleProcesses(self: *Process, allocator: std.mem.Allocator) process_sample.Error!process_sample.Sample {
        const Sampler = struct {
            allocator: std.mem.Allocator,
            pub fn observe(sampler: @This(), session_id: posix.pid_t) process_sample.Error!process_sample.Sample {
                return @import("process_query_posix.zig").collect(sampler.allocator, session_id);
            }
        };
        const root_pid = std.math.cast(u32, self.child_id) orelse return error.ProcessSampleUnavailable;
        if (!self.sessionNamed()) return .{ .allocator = allocator, .root_pid = root_pid, .processes = &.{} };
        return try self.identity.observe(process_sample.Sample, self.child_id, Sampler{ .allocator = allocator }) orelse
            .{ .allocator = allocator, .root_pid = root_pid, .processes = &.{} };
    }

    pub fn hasActiveCommand(self: *const Process) !bool {
        if (!self.interactive_shell) return true;
        return foregroundCommandActive(self.child_id, try self.pty.foregroundProcessGroup());
    }

    pub fn isForeground(self: *const Process, pid: u32) !bool {
        const group = getpgid(std.math.cast(c_int, pid) orelse return false);
        return group > 0 and group == try self.pty.foregroundProcessGroup();
    }

    pub fn wait(self: *Process) !ExitResult {
        if (self.exit_result) |result| return result;
        if (comptime builtin.os.tag == .macos) {
            const result = try self.keeper.awaitLeader(self.child_id, self.adopted) orelse return error.Stopped;
            self.exit_result = result;
            return result;
        }
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
        if (comptime builtin.os.tag == .macos) if (self.keeper.isParented()) {
            try self.identity.release(self.child_id, KeeperReaper{ .record = self.keeper });
            self.exit_reaped = true;
            return;
        } else if (self.adopted) {
            // Its Host forked it, so `launchd` reaps it once it ends.
            try self.identity.release(self.child_id, NoReaper{});
            self.exit_reaped = true;
            return;
        };
        if (self.exit_result != null) {
            try self.identity.release(
                self.child_id,
                process_session.NativeObservedExitReaper{},
            );
        } else {
            try self.identity.release(
                self.child_id,
                process_session.NativeSessionReaper{},
            );
        }
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

    /// Release first, without waiting: true once no Keeper holds the terminal. Only a macOS
    /// terminal is ever held.
    pub fn releaseKeepers(self: *Process) bool {
        if (comptime builtin.os.tag != .macos) return true;
        return self.keeper.gate.tryPass();
    }

    /// Closes the PTY master for shutdown after containment, once the reader and writer have
    /// stopped. A session leader that owns the terminal may be unable to finish exiting while
    /// output nobody reads is queued; the close discards it.
    pub fn closePtyForShutdown(self: *Process) void {
        self.pty.closeMaster();
    }

    pub fn abortRead(self: *Process) !void {
        try self.pty.abortRead();
    }

    /// Starts the leader through the attached Keeper, which then parents it, or forks it here when
    /// no Keeper is attached or the Keeper started nothing.
    fn startThroughKeeper(
        launch: Launch,
        record: *keeper_record.Record,
        plan: fork_launch.Plan,
        launch_kind: process_types.LaunchKind,
    ) !posix.pid_t {
        const spawner = launch.keeper orelse return fork_launch.start(plan);
        const outcome = spawner.spawn(spawner.context, &.{
            .record = record,
            .master = plan.streams.pty.master,
            .slave = plan.streams.pty.slave,
            .input = plan.streams.pty.input,
            .executable = plan.executable,
            .argv = plan.argv,
            .environment = plan.environment,
            .candidates = plan.candidates,
            .directory = plan.directory,
            .launch_kind = launch_kind,
        });
        return switch (outcome) {
            .started => |leader| leader,
            .refused => fork_launch.start(plan),
            .spawn_failed, .unanswered => error.SpawnFailed,
        };
    }

    /// Releases a parented leader's identity through its parent's `reap`.
    const KeeperReaper = struct {
        record: *keeper_record.Record,

        pub fn reap(self: KeeperReaper, _: posix.pid_t) !void {
            self.record.awaitReap();
        }
    };

    /// A recovered leader's identity release when nobody here parents it: nothing is left to reap.
    const NoReaper = struct {
        pub fn reap(_: NoReaper, _: posix.pid_t) !void {}
    };

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
        return @intCast(try posix_compat.fcntl(fd, command, 3));
    }

    fn executableCandidates(
        arena: std.mem.Allocator,
        environment: *const std.process.Environ.Map,
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

    pub fn forceTerminateSession(session_id: posix.pid_t, signaler: anytype) !void {
        return process_session.forceTerminateSession(session_id, signaler);
    }

    pub fn signalStableSessionMembersForTesting(
        session_id: posix.pid_t,
        source: anytype,
        signal_number: std.posix.SIG,
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

    pub fn reapObservedProcessExitForTesting(
        session_id: posix.pid_t,
        reaper: anytype,
        delay: anytype,
    ) !void {
        return process_session.reapObservedProcessExit(session_id, reaper, delay);
    }
};

pub fn executableCandidatesForTesting(
    arena: std.mem.Allocator,
    environment: *const std.process.Environ.Map,
    executable: []const u8,
) ![][*:0]const u8 {
    if (builtin.os.tag == .windows) return error.Unsupported;
    return Process.executableCandidates(arena, environment, executable);
}
