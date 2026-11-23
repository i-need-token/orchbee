//! A terminal leader's or background program's start, shared by a POSIX Host and the macOS Keeper:
//! fork, then in the child the launch directory, a new session, for a terminal with the PTY slave
//! as controlling terminal and standard streams and the standard input in place of the slave's when
//! given, for a program without a controlling terminal and with the streams the plan names, no
//! other descriptor, and `execve` of the executable or of each candidate in order. The parent
//! learns whether the exec happened from a close-on-exec status pipe, and reaps a child that did
//! not start.

const std = @import("std");
const builtin = @import("builtin");
const posix_compat = @import("posix_compat.zig");
const process_session = @import("process_session.zig");
const PosixPty = @import("pty_posix.zig").PosixPty;

const posix = std.posix;

pub const Plan = struct {
    /// The executable as named; one without a folder runs from `candidates`.
    executable: []const u8,
    argv: [*:null]const ?[*:0]const u8,
    environment: [*:null]const ?[*:0]const u8,
    candidates: []const [*:0]const u8,
    directory: ?[]const u8 = null,
    streams: Streams,
};

pub const Streams = union(enum) {
    /// A terminal: the PTY master, which the child closes before it installs the slave, the
    /// slave, and the standard input to use in place of the slave's.
    pty: struct { master: posix.fd_t, slave: posix.fd_t, input: ?posix.fd_t = null },
    /// A background program: its standard input, output, and error, such as the null device or a
    /// pipe's write end.
    pipes: struct { input: posix.fd_t, output: posix.fd_t, errors: posix.fd_t },
};

/// Starts the leader and returns its process id once it has exec'd. The caller still owns every
/// descriptor the plan names.
pub fn start(plan: Plan) error{ SpawnFailed, SystemResources }!posix.pid_t {
    const error_pipe = posix_compat.pipe2(.{ .CLOEXEC = true }) catch return error.SystemResources;
    // macOS marks a new descriptor close-on-exec only after creating it, as a control lane does
    // for each connection it accepts, so a shell forked meanwhile would keep that connection open.
    // The child keeps only its standard streams and the exec-status channel, which closes on exec.
    const descriptor_limit = if (comptime builtin.os.tag.isDarwin())
        posix_compat.descriptorTableSize()
    else
        0;
    const child_id = posix_compat.fork() catch {
        posix_compat.close(error_pipe[0]);
        posix_compat.close(error_pipe[1]);
        return error.SystemResources;
    };
    if (child_id == 0) {
        posix_compat.close(error_pipe[0]);
        if (plan.directory) |directory| {
            posix_compat.chdir(directory) catch reportChildFailure(error_pipe[1]);
        }
        switch (plan.streams) {
            .pty => |terminal| {
                posix_compat.close(terminal.master);
                PosixPty.attachControllingTerminal(terminal.slave) catch reportChildFailure(error_pipe[1]);
                if (terminal.input) |input| {
                    _ = posix_compat.dup2(input, posix.STDIN_FILENO) catch reportChildFailure(error_pipe[1]);
                }
            },
            .pipes => |pipes| {
                if (std.c.setsid() < 0) reportChildFailure(error_pipe[1]);
                inline for (.{ pipes.input, pipes.output, pipes.errors }, .{ posix.STDIN_FILENO, posix.STDOUT_FILENO, posix.STDERR_FILENO }) |source, target| {
                    _ = posix_compat.dup2(source, target) catch reportChildFailure(error_pipe[1]);
                }
            },
        }
        if (comptime builtin.os.tag.isDarwin()) {
            posix_compat.closeNonStandardDescriptors(descriptor_limit, error_pipe[1]);
        }
        reportExecFailure(error_pipe[1], execWithEffectivePath(plan.executable, plan.argv, plan.environment, plan.candidates));
    }
    posix_compat.close(error_pipe[1]);
    defer posix_compat.close(error_pipe[0]);
    var marker: [1]u8 = undefined;
    const failure_bytes = posix.read(error_pipe[0], &marker) catch {
        // Losing the exec-status channel makes the launch outcome unknowable. Contain both a child
        // that already created its session and one that failed before setsid, then reap it.
        process_session.forceTerminateSession(child_id, process_session.NativeSessionSignaler{}) catch {};
        posix.kill(child_id, posix.SIG.KILL) catch {};
        _ = posix_compat.waitpid(child_id, 0);
        return error.SpawnFailed;
    };
    if (failure_bytes != 0) {
        _ = posix_compat.waitpid(child_id, 0);
        return error.SpawnFailed;
    }
    return child_id;
}

fn reportChildFailure(error_fd: posix.fd_t) noreturn {
    _ = posix_compat.write(error_fd, &.{1}) catch 0;
    exitChildFailure();
}

fn reportExecFailure(error_fd: posix.fd_t, exec_error: anyerror) noreturn {
    const marker = [_]u8{@truncate(@intFromError(exec_error))};
    _ = posix_compat.write(error_fd, &marker) catch 0;
    exitChildFailure();
}

fn exitChildFailure() noreturn {
    if (builtin.link_libc) {
        std.c._exit(127);
    } else {
        posix_compat.exit(127);
    }
}

fn execWithEffectivePath(
    executable: []const u8,
    argv: [*:null]const ?[*:0]const u8,
    environment: [*:null]const ?[*:0]const u8,
    candidates: []const [*:0]const u8,
) posix_compat.ExecveError {
    if (std.mem.indexOfScalar(u8, executable, '/') != null) {
        return posix_compat.execveZ(argv[0].?, argv, environment);
    }
    var seen_access_denied = false;
    var last_error: posix_compat.ExecveError = error.FileNotFound;
    for (candidates) |candidate| {
        last_error = posix_compat.execveZ(candidate, argv, environment);
        switch (last_error) {
            error.AccessDenied => seen_access_denied = true,
            error.FileNotFound, error.NotDir => {},
            else => |err| return err,
        }
    }
    if (seen_access_denied) return error.AccessDenied;
    return last_error;
}
