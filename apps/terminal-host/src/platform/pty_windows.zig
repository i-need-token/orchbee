const std = @import("std");
const builtin = @import("builtin");

const conpty_signals = @import("conpty_signals_windows.zig");
const process_types = @import("process_types.zig");
const thread_sync = @import("thread_sync.zig");
const windows_api = @import("windows_api.zig");
const windows_conpty = @import("conpty_windows.zig");
const Size = @import("pty.zig").Size;

const Read = process_types.Read;
const ReadMode = process_types.ReadMode;

pub const WindowsPty = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    allocator: std.mem.Allocator,
    conpty: windows_conpty.Api,
    input: windows.HANDLE,
    output: windows.HANDLE,
    /// The reader's lock, guarding the read fields below. It is held only for bookkeeping and
    /// the pause's cancel: no ConPTY call runs under it, because a resize, clear or close can
    /// wait for the very output this reader has not read yet.
    read_mutex: thread_sync.Mutex = .{},
    read_abort_requested: bool = false,
    read_active: bool = false,
    /// A standing pause: a read in `stop_at_pause` mode reports it instead of reading.
    read_pause_requested: bool = false,
    /// The pause cancelled the active read, which then reports the pause even if it was
    /// withdrawn meanwhile: a cancelled read is never end of output.
    read_pause_cancelled: bool = false,
    /// Counts started reads, so a pause cancels only the read active when it was requested.
    read_sequence: u64 = 0,
    read_condition: thread_sync.Condition = .{},
    reader_thread: ?std.Thread.Handle = null,
    /// Owns the pseudoconsole: every resize, clear and close goes through its sender.
    signals: *conpty_signals.Signals,

    pub fn open(allocator: std.mem.Allocator, size: Size) !WindowsPty {
        const columns = try checkedCoordinate(size.columns);
        const rows = try checkedCoordinate(size.rows);
        var conpty = try windows_conpty.Api.init();
        errdefer conpty.deinit();
        var input_console: windows.HANDLE = undefined;
        var input: windows.HANDLE = undefined;
        var output: windows.HANDLE = undefined;
        var output_console: windows.HANDLE = undefined;
        if (!windows_api.CreatePipe(&input_console, &input, null, 0).toBool()) {
            return windows.unexpectedError(windows.GetLastError());
        }
        errdefer {
            _ = windows.CloseHandle(input_console);
            _ = windows.CloseHandle(input);
        }
        if (!windows_api.CreatePipe(&output, &output_console, null, 0).toBool()) {
            return windows.unexpectedError(windows.GetLastError());
        }
        errdefer {
            _ = windows.CloseHandle(output);
            _ = windows.CloseHandle(output_console);
        }

        if (!windows_api.SetHandleInformation(input, windows_api.HANDLE_FLAG_INHERIT, 0).toBool()) {
            return windows.unexpectedError(windows.GetLastError());
        }
        if (!windows_api.SetHandleInformation(output, windows_api.HANDLE_FLAG_INHERIT, 0).toBool()) {
            return windows.unexpectedError(windows.GetLastError());
        }
        var pseudo_console: windows_api.HPCON = undefined;
        const result = conpty.create(
            .{ .X = columns, .Y = rows },
            input_console,
            output_console,
            0,
            &pseudo_console,
        );
        if (result != windows_api.S_OK) return error.OpenPtyFailed;
        errdefer conpty.close(pseudo_console);
        const signals = try conpty_signals.Signals.start(allocator, &conpty, pseudo_console);
        _ = windows.CloseHandle(input_console);
        _ = windows.CloseHandle(output_console);
        return .{
            .allocator = allocator,
            .conpty = conpty,
            .input = input,
            .output = output,
            .signals = signals,
        };
    }

    pub fn deinit(self: *WindowsPty) void {
        self.closeConsole();
        self.signals.destroy(self.allocator);
        _ = windows.CloseHandle(self.input);
        _ = windows.CloseHandle(self.output);
        self.conpty.deinit();
        self.* = undefined;
    }

    pub fn abortRead(self: *WindowsPty) !void {
        self.read_mutex.lock();
        if (self.read_abort_requested and !self.read_active) {
            self.read_mutex.unlock();
            return;
        }
        self.read_abort_requested = true;
        self.read_condition.broadcast();
        var cancellation_requested = false;
        while (self.read_active) {
            if (cancellation_requested) {
                self.read_condition.wait(&self.read_mutex);
                continue;
            }
            const reader_thread = self.reader_thread orelse {
                self.read_mutex.unlock();
                return error.MissingReaderThread;
            };
            self.read_mutex.unlock();
            if (windows_api.CancelSynchronousIo(reader_thread).toBool()) {
                cancellation_requested = true;
            } else switch (windows.GetLastError()) {
                // The reader marks itself active immediately before entering
                // ReadFile. If cancellation wins that narrow gap, retry until
                // either the read starts or the reader observes the abort.
                .NOT_FOUND => std.Thread.yield() catch {},
                else => |err| return windows.unexpectedError(err),
            }
            self.read_mutex.lock();
        }
        self.read_mutex.unlock();
    }

    /// Stands a pause and cancels the read active when it was requested, returning once that
    /// read has left. The cancel runs under the reader's lock, so it reaches no later read; a
    /// read that returns its bytes despite the cancel keeps them. Nothing changes after an abort.
    pub fn pauseRead(self: *WindowsPty) !void {
        self.read_mutex.lock();
        defer self.read_mutex.unlock();
        if (self.read_abort_requested or self.read_pause_requested) return;
        self.read_pause_requested = true;
        const target = self.read_sequence;
        var cancellation_requested = false;
        while (self.read_active and self.read_sequence == target and !self.read_abort_requested) {
            if (cancellation_requested) {
                self.read_condition.wait(&self.read_mutex);
                continue;
            }
            // A read registers its thread before it starts.
            if (windows_api.CancelSynchronousIo(self.reader_thread.?).toBool()) {
                self.read_pause_cancelled = true;
                cancellation_requested = true;
            } else switch (windows.GetLastError()) {
                // The read is marked active just before ReadFile, or has left it and waits for
                // this lock: let it move on, then look again.
                .NOT_FOUND => {
                    self.read_mutex.unlock();
                    std.Thread.yield() catch {};
                    self.read_mutex.lock();
                },
                else => |err| {
                    self.read_pause_requested = false;
                    return windows.unexpectedError(err);
                },
            }
        }
    }

    /// Withdraws a standing pause, so the next read reads.
    pub fn resumeRead(self: *WindowsPty) !void {
        self.read_mutex.lock();
        defer self.read_mutex.unlock();
        self.read_pause_requested = false;
    }

    pub fn registerReaderThread(self: *WindowsPty, thread: std.Thread.Handle) void {
        self.read_mutex.lock();
        defer self.read_mutex.unlock();
        std.debug.assert(self.reader_thread == null);
        self.reader_thread = thread;
        self.read_condition.broadcast();
    }

    /// Closes the pseudoconsole once, after its sender stops; a signal the console has not taken
    /// is cancelled rather than waited for.
    pub fn closeConsole(self: *WindowsPty) void {
        self.signals.close();
    }

    pub fn closeSlave(_: *WindowsPty) void {}

    /// The pseudoconsole a child launches into; valid until `closeConsole`.
    pub fn pseudoConsole(self: *const WindowsPty) windows_api.HPCON {
        return self.signals.pseudo_console;
    }

    /// Asks ConPTY to clear its own buffer, keeping only the cursor row at the top with the
    /// cursor's column. True when the signal is queued: ConPTY then places later output by
    /// coordinates that count from that row, so the caller's screen must match. It never waits
    /// for the console.
    pub fn clearKeepingCursorRow(self: *WindowsPty) bool {
        return self.signals.clearKeepingCursorRow();
    }

    /// Reads one batch for a reader that never pauses: zero is end of output or abort.
    pub fn read(self: *WindowsPty, buffer: []u8) !usize {
        return switch (try self.readPausable(buffer, .through_pause)) {
            .bytes => |count| count,
            .end, .paused => 0,
        };
    }

    /// Reads one batch: `end` at end of output and once aborted, `paused` when a pause cancelled
    /// the read or, in `stop_at_pause` mode, stood before it started.
    pub fn readPausable(self: *WindowsPty, buffer: []u8, mode: ReadMode) !Read {
        self.read_mutex.lock();
        while (self.reader_thread == null and !self.read_abort_requested) {
            self.read_condition.wait(&self.read_mutex);
        }
        if (self.read_abort_requested) {
            self.read_mutex.unlock();
            return .end;
        }
        if (self.read_pause_requested and mode == .stop_at_pause) {
            self.read_mutex.unlock();
            return .paused;
        }
        std.debug.assert(!self.read_active);
        self.read_active = true;
        self.read_pause_cancelled = false;
        self.read_sequence +%= 1;
        self.read_mutex.unlock();
        var read_bytes: windows.DWORD = 0;
        const failure: ?windows.Win32Error = if (windows_api.ReadFile(
            self.output,
            buffer.ptr,
            @intCast(@min(buffer.len, std.math.maxInt(windows.DWORD))),
            &read_bytes,
            null,
        ).toBool()) null else windows.GetLastError();
        self.read_mutex.lock();
        self.read_active = false;
        const paused = self.read_pause_cancelled and !self.read_abort_requested;
        self.read_condition.broadcast();
        self.read_mutex.unlock();
        const err = failure orelse
            return if (read_bytes == 0) .end else .{ .bytes = read_bytes };
        return switch (err) {
            .BROKEN_PIPE, .HANDLE_EOF => .end,
            // A cancelled read returns no bytes; were it to, they would be lost, so only this
            // terminal fails instead.
            .OPERATION_ABORTED => if (read_bytes != 0)
                error.AbortedReadReturnedBytes
            else if (paused) .paused else .end,
            else => windows.unexpectedError(err),
        };
    }

    pub fn write(self: *const WindowsPty, bytes: []const u8) !usize {
        var written: windows.DWORD = 0;
        if (!windows_api.WriteFile(
            self.input,
            bytes.ptr,
            @intCast(@min(bytes.len, std.math.maxInt(windows.DWORD))),
            &written,
            null,
        ).toBool()) return switch (windows.GetLastError()) {
            .INVALID_USER_BUFFER, .NOT_ENOUGH_MEMORY, .NOT_ENOUGH_QUOTA, .WORKING_SET_QUOTA => error.SystemResources,
            .OPERATION_ABORTED => error.OperationAborted,
            .BROKEN_PIPE, .NO_DATA => error.BrokenPipe,
            .INVALID_HANDLE => error.NotOpenForWriting,
            .LOCK_VIOLATION => error.LockViolation,
            .NETNAME_DELETED => error.ConnectionResetByPeer,
            .ACCESS_DENIED => error.AccessDenied,
            .IO_PENDING => unreachable,
            else => |err| windows.unexpectedError(err),
        };
        return written;
    }

    /// Queues the size for the signal sender and returns without waiting for the console. A
    /// closed console, or an earlier signal that failed, is the error.
    pub fn setSize(self: *WindowsPty, size: Size) !void {
        const columns = try checkedCoordinate(size.columns);
        const rows = try checkedCoordinate(size.rows);
        try self.signals.resize(.{ .X = columns, .Y = rows });
    }

    fn checkedCoordinate(value: u16) !i16 {
        return std.math.cast(i16, value) orelse error.PtySizeOutOfRange;
    }
};
