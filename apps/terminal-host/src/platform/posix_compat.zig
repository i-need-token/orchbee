const std = @import("std");

/// Call only at executable startup, before acquiring owned descriptors or
/// starting workers. Electron can leave non-CLOEXEC Chromium descriptors in a
/// spawned Host; retaining them also retains its debugger and storage locks.
pub fn closeInheritedLinuxDescriptors(io: std.Io) !void {
    var descriptors = try std.Io.Dir.openDirAbsolute(io, "/proc/self/fd", .{ .iterate = true });
    defer descriptors.close(io);
    var iterator = descriptors.iterate();
    while (try iterator.next(io)) |entry| {
        const descriptor = try std.fmt.parseInt(std.posix.fd_t, entry.name, 10);
        if (descriptor <= 2 or descriptor == descriptors.handle) continue;
        close(descriptor);
    }
}

pub const PipeError = error{
    SystemFdQuotaExceeded,
    ProcessFdQuotaExceeded,
} || std.posix.UnexpectedError;

pub fn pipe2(flags: std.posix.O) PipeError![2]std.posix.fd_t {
    if (@TypeOf(std.posix.system.pipe2) != void) {
        var descriptors: [2]std.posix.fd_t = undefined;
        switch (std.posix.errno(std.posix.system.pipe2(&descriptors, flags))) {
            .SUCCESS => return descriptors,
            .INVAL, .FAULT => unreachable,
            .NFILE => return error.SystemFdQuotaExceeded,
            .MFILE => return error.ProcessFdQuotaExceeded,
            else => |err| return std.posix.unexpectedErrno(err),
        }
    }

    var descriptors: [2]std.posix.fd_t = undefined;
    switch (std.posix.errno(std.posix.system.pipe(&descriptors))) {
        .SUCCESS => {},
        .INVAL, .FAULT => unreachable,
        .NFILE => return error.SystemFdQuotaExceeded,
        .MFILE => return error.ProcessFdQuotaExceeded,
        else => |err| return std.posix.unexpectedErrno(err),
    }
    errdefer {
        for (descriptors) |descriptor| _ = std.posix.system.close(descriptor);
    }

    if (flags.CLOEXEC) {
        for (descriptors) |descriptor| {
            switch (std.posix.errno(std.posix.system.fcntl(
                descriptor,
                std.posix.F.SETFD,
                @as(u32, std.posix.FD_CLOEXEC),
            ))) {
                .SUCCESS => {},
                .INVAL, .BADF => unreachable,
                else => |err| return std.posix.unexpectedErrno(err),
            }
        }
    }

    const status_flags: u32 = flags: {
        var value = flags;
        value.CLOEXEC = false;
        break :flags @bitCast(value);
    };
    if (status_flags != 0) {
        for (descriptors) |descriptor| {
            switch (std.posix.errno(std.posix.system.fcntl(
                descriptor,
                std.posix.F.SETFL,
                status_flags,
            ))) {
                .SUCCESS => {},
                .INVAL, .BADF => unreachable,
                else => |err| return std.posix.unexpectedErrno(err),
            }
        }
    }
    return descriptors;
}

pub fn fork() !std.posix.pid_t {
    const result = std.posix.system.fork();
    switch (std.posix.errno(result)) {
        .SUCCESS => return @intCast(result),
        .AGAIN, .NOMEM => return error.SystemResources,
        else => |err| return std.posix.unexpectedErrno(err),
    }
}

pub fn close(descriptor: std.posix.fd_t) void {
    _ = std.posix.system.close(descriptor);
}

pub fn dup2(source: std.posix.fd_t, target: std.posix.fd_t) !std.posix.fd_t {
    const result = std.c.dup2(source, target);
    if (result >= 0) return result;
    return switch (std.posix.errno(result)) {
        .BADF => error.FileDescriptorInvalid,
        .BUSY, .INTR => error.OperationInterrupted,
        .MFILE => error.ProcessFdQuotaExceeded,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub fn fcntl(descriptor: std.posix.fd_t, command: c_int, argument: c_int) !c_int {
    const result = std.c.fcntl(descriptor, command, argument);
    if (result >= 0) return result;
    return switch (std.posix.errno(result)) {
        .BADF => error.FileDescriptorInvalid,
        .INTR => error.OperationInterrupted,
        .MFILE => error.ProcessFdQuotaExceeded,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub fn chdir(path: []const u8) !void {
    const path_z = try std.posix.toPosixPath(path);
    if (std.c.chdir(&path_z) == 0) return;
    return switch (std.posix.errno(-1)) {
        .ACCES => error.AccessDenied,
        .LOOP => error.SymLinkLoop,
        .NAMETOOLONG => error.NameTooLong,
        .NOENT => error.FileNotFound,
        .NOTDIR => error.NotDir,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub fn fchdir(descriptor: std.posix.fd_t) !void {
    if (std.c.fchdir(descriptor) == 0) return;
    return switch (std.posix.errno(-1)) {
        .ACCES => error.AccessDenied,
        .BADF => error.FileDescriptorInvalid,
        .NOTDIR => error.NotDir,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub fn send(descriptor: std.posix.fd_t, bytes: []const u8, flags: u32) !usize {
    const result = std.c.send(descriptor, bytes.ptr, bytes.len, flags);
    if (result >= 0) return @intCast(result);
    return switch (std.posix.errno(result)) {
        .AGAIN => error.WouldBlock,
        .BADF => error.FileDescriptorInvalid,
        .CONNRESET => error.ConnectionResetByPeer,
        .INTR => error.OperationInterrupted,
        .NOTCONN, .PIPE => error.SocketUnconnected,
        .NOBUFS, .NOMEM => error.SystemResources,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub const ExecveError = error{
    AccessDenied,
    FileBusy,
    FileNotFound,
    InvalidExe,
    NameTooLong,
    NotDir,
    SymLinkLoop,
    SystemResources,
} || std.posix.UnexpectedError;

pub fn execveZ(
    path: [*:0]const u8,
    argv: [*:null]const ?[*:0]const u8,
    environment: [*:null]const ?[*:0]const u8,
) ExecveError {
    const result = std.c.execve(path, argv, environment);
    std.debug.assert(result < 0);
    return switch (std.posix.errno(result)) {
        .ACCES, .PERM => error.AccessDenied,
        .AGAIN, .NOMEM => error.SystemResources,
        .LOOP => error.SymLinkLoop,
        .NAMETOOLONG => error.NameTooLong,
        .NOENT => error.FileNotFound,
        .NOEXEC => error.InvalidExe,
        .NOTDIR => error.NotDir,
        .TXTBSY => error.FileBusy,
        .FAULT => unreachable,
        else => |err| std.posix.unexpectedErrno(err),
    };
}

pub const WaitResult = struct {
    pid: std.posix.pid_t,
    status: u32,
};

pub fn waitpid(pid: std.posix.pid_t, options: c_int) WaitResult {
    var status: c_int = 0;
    while (true) {
        const result = std.c.waitpid(pid, &status, options);
        if (result >= 0) return .{ .pid = result, .status = @bitCast(status) };
        switch (std.posix.errno(result)) {
            .INTR => continue,
            .CHILD => return .{ .pid = 0, .status = 0 },
            else => |err| std.debug.panic("waitpid failed: {s}", .{@tagName(err)}),
        }
    }
}

pub fn write(descriptor: std.posix.fd_t, bytes: []const u8) !usize {
    if (bytes.len == 0) return 0;
    while (true) {
        const result = std.posix.system.write(
            descriptor,
            bytes.ptr,
            @min(bytes.len, std.math.maxInt(isize)),
        );
        switch (std.posix.errno(result)) {
            .SUCCESS => return @intCast(result),
            .INTR => continue,
            .AGAIN => return error.WouldBlock,
            .BADF => return error.NotOpenForWriting,
            .DESTADDRREQ => return error.NoAddress,
            .DQUOT => return error.DiskQuota,
            .FAULT, .INVAL => unreachable,
            .FBIG => return error.FileTooBig,
            .IO => return error.InputOutput,
            .NOSPC => return error.NoSpaceLeft,
            .PERM => return error.AccessDenied,
            .PIPE => return error.BrokenPipe,
            .CONNRESET => return error.ConnectionResetByPeer,
            else => |err| return std.posix.unexpectedErrno(err),
        }
    }
}

pub fn exit(status: u8) noreturn {
    std.c._exit(status);
}
