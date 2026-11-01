//! Wakes the one service loop when a PTY reader commits output or a projection cadence falls due,
//! so a frame does not wait out the poll interval. A wake is only a hint: the loop still owns every
//! delivery decision, and its poll interval remains the bound in a process without a wake.
const std = @import("std");
const builtin = @import("builtin");
const posix_compat = @import("posix_compat.zig");

const windows = std.os.windows;
const is_windows = builtin.os.tag == .windows;

const WindowsApi = if (is_windows) struct {
    const create_waitable_timer_high_resolution: windows.DWORD = 0x0000_0002;
    const timer_all_access: windows.DWORD = 0x001f_0003;

    extern "kernel32" fn CreateEventW(?*anyopaque, windows.BOOL, windows.BOOL, ?[*:0]const u16) callconv(.winapi) ?windows.HANDLE;
    extern "kernel32" fn CreateWaitableTimerExW(?*anyopaque, ?[*:0]const u16, windows.DWORD, windows.DWORD) callconv(.winapi) ?windows.HANDLE;
    extern "kernel32" fn SetEvent(windows.HANDLE) callconv(.winapi) windows.BOOL;
    extern "kernel32" fn SetWaitableTimer(windows.HANDLE, *const i64, i32, ?*anyopaque, ?*anyopaque, windows.BOOL) callconv(.winapi) windows.BOOL;
    extern "kernel32" fn CloseHandle(windows.HANDLE) callconv(.winapi) windows.BOOL;
} else struct {};

// Each object is prepared once before any reader starts and retained for the process lifetime. The
// Windows objects reset themselves when a wait observes them.
var windows_event: std.atomic.Value(usize) = .init(0);
var windows_timer: std.atomic.Value(usize) = .init(0);
var posix_read: std.atomic.Value(i32) = .init(-1);
var posix_write: std.atomic.Value(i32) = .init(-1);
var posix_due_ms: std.atomic.Value(i32) = .init(-1);

/// Best effort: without a wake the service keeps its poll interval.
pub fn prepare() void {
    if (is_windows) {
        prepareWindowsObject(&windows_event, WindowsApi.CreateEventW(null, .FALSE, .FALSE, null));
        prepareWindowsObject(&windows_timer, WindowsApi.CreateWaitableTimerExW(
            null,
            null,
            WindowsApi.create_waitable_timer_high_resolution,
            WindowsApi.timer_all_access,
        ));
    } else {
        if (posix_write.load(.acquire) != -1) return;
        const descriptors = posix_compat.pipe2(.{ .CLOEXEC = true, .NONBLOCK = true }) catch return;
        if (posix_write.cmpxchgStrong(-1, descriptors[1], .acq_rel, .acquire) != null) {
            posix_compat.close(descriptors[0]);
            posix_compat.close(descriptors[1]);
            return;
        }
        posix_read.store(descriptors[0], .release);
    }
}

fn prepareWindowsObject(slot: *std.atomic.Value(usize), object: ?windows.HANDLE) void {
    const created = object orelse return;
    if (slot.cmpxchgStrong(0, @intFromPtr(created), .acq_rel, .acquire) != null) {
        _ = WindowsApi.CloseHandle(created);
    }
}

/// Ends the current or next service wait. Safe from any thread.
pub fn signal() void {
    if (is_windows) {
        const event = windows_event.load(.acquire);
        if (event != 0) _ = WindowsApi.SetEvent(@ptrFromInt(event));
    } else {
        const descriptor = posix_write.load(.acquire);
        if (descriptor == -1) return;
        // A full pipe already holds an undrained wake.
        _ = posix_compat.write(descriptor, &[_]u8{1}) catch {};
    }
}

/// Ends the next service wait this long from now. Called by the service loop itself.
pub fn signalAfter(nanoseconds: u64) void {
    if (is_windows) {
        const timer = windows_timer.load(.acquire);
        if (timer == 0) return;
        // A negative due time is relative, in 100 ns units.
        const due: i64 = -@as(i64, @intCast(@max(nanoseconds / 100, 1)));
        _ = WindowsApi.SetWaitableTimer(@ptrFromInt(timer), &due, 0, null, null, .FALSE);
    } else {
        const milliseconds = std.math.divCeil(u64, nanoseconds, std.time.ns_per_ms) catch unreachable;
        posix_due_ms.store(@intCast(@min(milliseconds, std.math.maxInt(i32))), .release);
    }
}

pub fn windowsEvent() ?windows.HANDLE {
    const event = windows_event.load(.acquire);
    return if (event == 0) null else @ptrFromInt(event);
}

pub fn windowsTimer() ?windows.HANDLE {
    const timer = windows_timer.load(.acquire);
    return if (timer == 0) null else @ptrFromInt(timer);
}

/// A negative descriptor, which `poll` ignores, in a process without a wake.
pub fn readDescriptor() i32 {
    return posix_read.load(.acquire);
}

/// The next `poll` bound: a due cadence inside `maximum_ms` shortens it once.
pub fn pollTimeoutMs(maximum_ms: i32) i32 {
    const due = posix_due_ms.swap(-1, .acq_rel);
    return if (due < 0) maximum_ms else @min(due, maximum_ms);
}

pub fn drain() void {
    const descriptor = posix_read.load(.acquire);
    if (descriptor == -1) return;
    var bytes: [64]u8 = undefined;
    while (true) {
        const count = std.posix.read(descriptor, &bytes) catch return;
        if (count < bytes.len) return;
    }
}
