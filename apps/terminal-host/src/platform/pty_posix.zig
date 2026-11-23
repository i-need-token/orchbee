const std = @import("std");
const builtin = @import("builtin");

const posix_compat = @import("posix_compat.zig");
const process_types = @import("process_types.zig");
const thread_sync = @import("thread_sync.zig");
const Size = @import("pty.zig").Size;

const Read = process_types.Read;
const ReadMode = process_types.ReadMode;

extern "c" fn tcgetpgrp(handle: c_int) c_int;

pub const PosixPty = if (builtin.os.tag == .windows) void else struct {
    const posix = std.posix;
    const c = @import("pty-c");

    abort_read_fd: posix.fd_t,
    abort_write_fd: posix.fd_t,
    master: posix.fd_t,
    /// Guards the abort pipe's marker accounting below. An abort's marker is never drained; a
    /// pause's marker is drained by the read it woke or by the resume that withdrew it.
    read_control: thread_sync.Mutex = .{},
    read_aborted: bool = false,
    read_pause: enum {
        none,
        /// The pause's marker waits in the abort pipe.
        signalled,
        /// A read drained the marker and reported the pause.
        observed,
    } = .none,
    slave: ?posix.fd_t,

    pub fn open(_: std.mem.Allocator, size: Size) !PosixPty {
        var native_size = nativeSize(size);
        var master: posix.fd_t = undefined;
        var slave: posix.fd_t = undefined;
        if (c.openpty(&master, &slave, null, null, &native_size) < 0) {
            return error.OpenPtyFailed;
        }
        const abort_pipe = posix_compat.pipe2(.{ .CLOEXEC = true }) catch |err| {
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

    /// A PTY from the master of a terminal another Host opened, which a Keeper passed on; there
    /// is no slave. On error the caller still owns `master`.
    pub fn adopt(master: posix.fd_t) !PosixPty {
        try setCloseOnExec(master);
        const abort_pipe = try posix_compat.pipe2(.{ .CLOEXEC = true });
        return .{
            .abort_read_fd = abort_pipe[0],
            .abort_write_fd = abort_pipe[1],
            .master = master,
            .slave = null,
        };
    }

    pub fn deinit(self: *PosixPty) void {
        if (self.slave) |slave| _ = posix.system.close(slave);
        self.closeMaster();
        _ = posix.system.close(self.abort_read_fd);
        _ = posix.system.close(self.abort_write_fd);
        self.* = undefined;
    }

    /// Closes the master early, once no thread can use it again: a later open may reuse the
    /// descriptor number. Later master operations then fail instead of reaching that file.
    pub fn closeMaster(self: *PosixPty) void {
        if (self.master < 0) return;
        _ = posix.system.close(self.master);
        self.master = -1;
    }

    pub fn abortRead(self: *PosixPty) !void {
        self.read_control.lock();
        defer self.read_control.unlock();
        if (self.read_aborted) return;
        // A pause's marker still in the pipe wakes the read, and now stays for the abort.
        if (self.read_pause != .signalled) try self.writeMarker();
        self.read_aborted = true;
    }

    /// Stands a pause: its marker wakes the read in progress, which reports the pause. Nothing
    /// changes after an abort.
    pub fn pauseRead(self: *PosixPty) !void {
        self.read_control.lock();
        defer self.read_control.unlock();
        if (self.read_aborted or self.read_pause != .none) return;
        try self.writeMarker();
        self.read_pause = .signalled;
    }

    /// Withdraws a standing pause, draining its marker if no read took it, so the next read reads.
    pub fn resumeRead(self: *PosixPty) !void {
        self.read_control.lock();
        defer self.read_control.unlock();
        if (self.read_pause == .signalled and !self.read_aborted) try self.drainMarker();
        self.read_pause = .none;
    }

    fn writeMarker(self: *PosixPty) !void {
        const marker = [_]u8{1};
        _ = try posix_compat.write(self.abort_write_fd, &marker);
    }

    /// Takes the pause's marker, which is known to be in the pipe.
    fn drainMarker(self: *PosixPty) !void {
        var marker: [1]u8 = undefined;
        if (try posix.read(self.abort_read_fd, &marker) != 1) return error.PauseMarkerMissing;
    }

    /// Takes a standing pause before a read starts, draining its marker: a read that stops at a
    /// pause reports it, and one that reads through it reads, as on Windows.
    fn takePause(self: *PosixPty) !bool {
        self.read_control.lock();
        defer self.read_control.unlock();
        if (self.read_aborted or self.read_pause == .none) return false;
        if (self.read_pause == .signalled) try self.drainMarker();
        self.read_pause = .observed;
        return true;
    }

    /// Classifies a readable abort pipe; a resume may have drained the pause's marker first.
    fn markerWoke(self: *PosixPty) !enum { aborted, paused, withdrawn } {
        self.read_control.lock();
        defer self.read_control.unlock();
        if (self.read_aborted) return .aborted;
        if (self.read_pause != .signalled) return .withdrawn;
        try self.drainMarker();
        self.read_pause = .observed;
        return .paused;
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
        // descriptor. Drop the master before installing the slave.
        _ = posix.system.close(self.master);
        return attachControllingTerminal(slave);
    }

    /// In a forked child: a new session with `slave` as its controlling terminal and standard
    /// streams, keeping any slave descriptor that is itself one of the resulting streams.
    pub fn attachControllingTerminal(slave: posix.fd_t) !void {
        if (c.setsid() < 0) return error.CreateSessionFailed;
        if (c.ioctl(slave, c.TIOCSCTTY, @as(c_ulong, 0)) < 0) {
            return error.SetControllingTerminalFailed;
        }
        try duplicateTo(slave, posix.STDIN_FILENO);
        try duplicateTo(slave, posix.STDOUT_FILENO);
        try duplicateTo(slave, posix.STDERR_FILENO);
        if (slave > posix.STDERR_FILENO) _ = posix.system.close(slave);
    }

    /// Reads one batch for a reader that never pauses: zero is end of output or abort.
    pub fn read(self: *PosixPty, buffer: []u8) !usize {
        return switch (try self.readPausable(buffer, .through_pause)) {
            .bytes => |count| count,
            .end, .paused => 0,
        };
    }

    /// Reads one batch: `end` at end of output and once aborted, `paused` when a pause woke the
    /// read or, in `stop_at_pause` mode, stood before it started.
    pub fn readPausable(self: *PosixPty, buffer: []u8, mode: ReadMode) !Read {
        if (try self.takePause() and mode == .stop_at_pause) return .paused;
        var descriptors = [_]posix.pollfd{
            .{ .fd = self.master, .events = posix.POLL.IN, .revents = 0 },
            .{ .fd = self.abort_read_fd, .events = posix.POLL.IN, .revents = 0 },
        };
        while (true) {
            _ = try posix.poll(&descriptors, -1);
            if (descriptors[1].revents != 0) switch (try self.markerWoke()) {
                .aborted => return .end,
                .paused => return .paused,
                .withdrawn => {},
            };
            if (descriptors[0].revents & (posix.POLL.IN | posix.POLL.HUP | posix.POLL.ERR) != 0) {
                const count = posix.read(self.master, buffer) catch |err| switch (err) {
                    error.InputOutput => return .end,
                    else => return err,
                };
                return if (count == 0) .end else .{ .bytes = count };
            }
            descriptors[0].revents = 0;
            descriptors[1].revents = 0;
        }
    }

    pub fn write(self: *const PosixPty, bytes: []const u8) !usize {
        return posix_compat.write(self.master, bytes);
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
        const flags = std.c.fcntl(fd, posix.F.GETFD, @as(c_int, 0));
        if (flags < 0) return error.ConfigurePtyFailed;
        if (std.c.fcntl(fd, posix.F.SETFD, flags | posix.FD_CLOEXEC) < 0) {
            return error.ConfigurePtyFailed;
        }
    }

    fn duplicateTo(source: posix.fd_t, target: posix.fd_t) !void {
        if (source == target) {
            const flags = std.c.fcntl(source, posix.F.GETFD, @as(c_int, 0));
            if (flags < 0) return error.ConfigurePtyFailed;
            if (std.c.fcntl(source, posix.F.SETFD, flags & ~@as(c_int, posix.FD_CLOEXEC)) < 0) {
                return error.ConfigurePtyFailed;
            }
            return;
        }
        _ = try posix_compat.dup2(source, target);
    }
};
