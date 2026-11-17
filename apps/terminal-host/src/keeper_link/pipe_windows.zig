//! The Host's client end of one Keeper connection. Every transfer is overlapped and waits on its
//! completion, the Keeper process, and the link's wake event together, so no transfer outlives its
//! deadline or the Keeper, and the link sees a terminal that began ending while it waits.

const std = @import("std");
const monotonic = @import("terminal-host-platform").monotonic;

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const DWORD = windows.DWORD;
const BOOL = windows.BOOL;

const GENERIC_READ: DWORD = 0x8000_0000;
const GENERIC_WRITE: DWORD = 0x4000_0000;
const OPEN_EXISTING: DWORD = 3;
const FILE_FLAG_OVERLAPPED: DWORD = 0x4000_0000;
/// The Keeper may identify the Host but never impersonate it.
const SECURITY_SQOS_PRESENT: DWORD = 0x0010_0000;
const SECURITY_IDENTIFICATION: DWORD = 0x0001_0000;
const ERROR_PIPE_BUSY: DWORD = 231;
const ERROR_IO_PENDING: DWORD = 997;
const WAIT_OBJECT_0: DWORD = 0;
const WAIT_TIMEOUT: DWORD = 258;

const Overlapped = extern struct {
    internal: usize = 0,
    internal_high: usize = 0,
    offset: DWORD = 0,
    offset_high: DWORD = 0,
    event: ?HANDLE = null,
};

extern "kernel32" fn CreateFileW(name: [*:0]const u16, access: DWORD, share: DWORD, security: ?*anyopaque, disposition: DWORD, flags: DWORD, template: ?HANDLE) callconv(.winapi) HANDLE;
extern "kernel32" fn WaitNamedPipeW(name: [*:0]const u16, timeout: DWORD) callconv(.winapi) BOOL;
extern "kernel32" fn GetNamedPipeServerProcessId(pipe: HANDLE, process_id: *DWORD) callconv(.winapi) BOOL;
extern "kernel32" fn ReadFile(file: HANDLE, buffer: [*]u8, length: DWORD, read: ?*DWORD, overlapped: *Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn WriteFile(file: HANDLE, buffer: [*]const u8, length: DWORD, written: ?*DWORD, overlapped: *Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn GetOverlappedResult(file: HANDLE, overlapped: *Overlapped, transferred: *DWORD, wait: BOOL) callconv(.winapi) BOOL;
extern "kernel32" fn CancelIoEx(file: HANDLE, overlapped: ?*Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn CreateEventW(attributes: ?*anyopaque, manual_reset: BOOL, initial: BOOL, name: ?[*:0]const u16) callconv(.winapi) ?HANDLE;
extern "kernel32" fn WaitForMultipleObjects(count: DWORD, handles: [*]const HANDLE, wait_all: BOOL, milliseconds: DWORD) callconv(.winapi) DWORD;

pub const Error = error{
    /// The deadline passed before the transfer completed.
    Deadline,
    /// The Keeper process ended.
    KeeperEnded,
    /// The connection ended or failed.
    Closed,
};

pub const Pipe = struct {
    handle: HANDLE,
    event: HANDLE,

    /// Opens the Keeper endpoint `endpoint_id` before `deadline_ns` on `timer`.
    pub fn open(endpoint_id: []const u8, timer: *const monotonic.Timer, deadline_ns: u64) !Pipe {
        var name_buffer: [96]u16 = undefined;
        const name = pipeName(&name_buffer, endpoint_id);
        const handle = while (true) {
            const opened = CreateFileW(
                name.ptr,
                GENERIC_READ | GENERIC_WRITE,
                0,
                null,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null,
            );
            if (opened != windows.INVALID_HANDLE_VALUE) break opened;
            if (@intFromEnum(windows.GetLastError()) != ERROR_PIPE_BUSY) return error.Closed;
            const remaining_ms = (deadline_ns -| timer.read()) / std.time.ns_per_ms;
            if (remaining_ms == 0) return error.Deadline;
            _ = WaitNamedPipeW(name.ptr, @intCast(@min(remaining_ms, 100)));
        };
        errdefer windows.CloseHandle(handle);
        const event = CreateEventW(null, .TRUE, .FALSE, null) orelse return error.Closed;
        return .{ .handle = handle, .event = event };
    }

    pub fn close(self: *Pipe) void {
        windows.CloseHandle(self.handle);
        windows.CloseHandle(self.event);
        self.* = undefined;
    }

    /// The process id of the endpoint's server.
    pub fn serverProcessId(self: *const Pipe) ?u32 {
        var process_id: DWORD = 0;
        if (!GetNamedPipeServerProcessId(self.handle, &process_id).toBool()) return null;
        return process_id;
    }

    /// Writes all of `bytes` before `deadline_ns` on `timer`. `watch` has the Keeper process
    /// handle `keeper`, the wake event `wake`, and `woken(context)`, which runs on this thread each
    /// time the wake event fires while the wait goes on.
    pub fn writeAll(self: *Pipe, bytes: []const u8, timer: *const monotonic.Timer, deadline_ns: u64, watch: anytype) Error!void {
        var offset: usize = 0;
        while (offset < bytes.len) offset += try self.transfer(.write, @constCast(bytes[offset..]), timer, deadline_ns, watch);
    }

    /// A frame source for `frame.readPayload`: each read returns at least one byte before the
    /// deadline, or fails.
    pub fn Source(comptime WatchType: type) type {
        return struct {
            pipe: *Pipe,
            timer: *const monotonic.Timer,
            deadline_ns: u64,
            watch: WatchType,

            pub fn read(self: *@This(), bytes: []u8) Error!usize {
                return self.pipe.transfer(.read, bytes, self.timer, self.deadline_ns, self.watch);
            }
        };
    }

    fn transfer(
        self: *Pipe,
        comptime direction: enum { read, write },
        buffer: []u8,
        timer: *const monotonic.Timer,
        deadline_ns: u64,
        watch: anytype,
    ) Error!usize {
        if (timer.read() >= deadline_ns) return error.Deadline;
        var overlapped: Overlapped = .{ .event = self.event };
        const length: DWORD = @intCast(@min(buffer.len, 64 * 1024));
        const started = switch (direction) {
            .read => ReadFile(self.handle, buffer.ptr, length, null, &overlapped),
            .write => WriteFile(self.handle, buffer.ptr, length, null, &overlapped),
        };
        if (!started.toBool() and @intFromEnum(windows.GetLastError()) != ERROR_IO_PENDING) return error.Closed;
        const handles = [_]HANDLE{ self.event, watch.keeper, watch.wake };
        const failure: ?Error = while (true) {
            const remaining_ms = (deadline_ns -| timer.read() + std.time.ns_per_ms - 1) / std.time.ns_per_ms;
            if (remaining_ms == 0) break error.Deadline;
            switch (WaitForMultipleObjects(handles.len, &handles, .FALSE, @intCast(@min(remaining_ms, std.math.maxInt(u31))))) {
                WAIT_OBJECT_0 => break null,
                WAIT_OBJECT_0 + 1 => break error.KeeperEnded,
                WAIT_OBJECT_0 + 2 => watch.woken(watch.context),
                WAIT_TIMEOUT => break error.Deadline,
                else => break error.Closed,
            }
        };
        var transferred: DWORD = 0;
        if (failure) |err| {
            _ = CancelIoEx(self.handle, &overlapped);
            // The cancelled transfer must finish before its buffer and overlapped go; one that
            // completed as it was cancelled still delivered its bytes.
            if (GetOverlappedResult(self.handle, &overlapped, &transferred, .TRUE).toBool() and transferred > 0)
                return transferred;
            return err;
        }
        if (!GetOverlappedResult(self.handle, &overlapped, &transferred, .FALSE).toBool() or transferred == 0)
            return error.Closed;
        return transferred;
    }
};

fn pipeName(buffer: *[96]u16, endpoint_id: []const u8) [:0]const u16 {
    const prefix = std.unicode.utf8ToUtf16LeStringLiteral("\\\\.\\pipe\\orchbee-terminal-host-");
    @memcpy(buffer[0..prefix.len], prefix);
    for (endpoint_id, 0..) |byte, index| buffer[prefix.len + index] = byte;
    buffer[prefix.len + endpoint_id.len] = 0;
    return buffer[0 .. prefix.len + endpoint_id.len :0];
}
