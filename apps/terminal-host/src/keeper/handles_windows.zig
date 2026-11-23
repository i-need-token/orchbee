//! The Keeper's Win32 handle operations: opening a registered Host, pulling a Host's seven handles,
//! the checks a hold makes on its own duplicates, and the wait that sees a Host end. None of them
//! reads, writes, or names a pipe, so none waits behind another process's blocked synchronous read
//! of a held pipe, as probe P1 measured.

const std = @import("std");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const shared = @import("terminal-host-platform").keeper.handles;

const message = keeper_protocol.message;
const windows = std.os.windows;

pub const Handle = windows.HANDLE;
const BOOL = windows.BOOL;
const DWORD = windows.DWORD;
const ULONG = u32;

const WT_EXECUTEONLYONCE: ULONG = 0x0000_0008;

/// The callback a Host-end wait runs on a thread-pool thread, with the context it was armed with.
pub const EndCallback = *const fn (context: ?*anyopaque, timed_out: u8) callconv(.winapi) void;

extern "kernel32" fn CloseHandle(handle: Handle) callconv(.winapi) BOOL;
extern "kernel32" fn RegisterWaitForSingleObject(
    wait: *?Handle,
    object: Handle,
    callback: EndCallback,
    context: ?*anyopaque,
    milliseconds: ULONG,
    flags: ULONG,
) callconv(.winapi) BOOL;
extern "kernel32" fn UnregisterWaitEx(wait: Handle, completion: ?Handle) callconv(.winapi) BOOL;
extern "kernel32" fn CreateEventW(attributes: ?*anyopaque, manual_reset: BOOL, initial: BOOL, name: ?[*:0]const u16) callconv(.winapi) ?Handle;
extern "kernel32" fn SetEvent(event: Handle) callconv(.winapi) BOOL;
extern "kernel32" fn GetCurrentProcessId() callconv(.winapi) DWORD;

/// The Keeper's duplicates of one holding, in the order of `message.Handles.roles`.
pub const Duplicates = shared.Duplicates;

pub const OpenError = shared.OpenError;

/// Opens a Host the Worker registers, for its end, its creation time, and duplication from it.
pub fn openHost(process: message.Process) OpenError!Handle {
    return shared.openPeer(.{ .pid = process.pid, .created = process.created });
}

/// Duplicates the seven handles `handles` names from `host` into the Keeper, each with its
/// source's access and not inheritable. All or nothing.
pub fn pull(host: Handle, handles: message.Handles) error{HandleFailed}!Duplicates {
    return shared.pull(host, .{
        handles.job,       handles.program, handles.console, handles.signal,
        handles.reference, handles.input,   handles.output,
    });
}

/// Whether the Keeper's own duplicates match `facts`: each object's type and access, both process
/// identities, and the program's membership in the Job.
pub fn matches(duplicates: Duplicates, facts: message.Facts) bool {
    return shared.matches(
        duplicates,
        .{ .pid = facts.program.pid, .created = facts.program.created },
        .{ .pid = facts.console.pid, .created = facts.console.created },
    );
}

/// The Keeper's duplicates of one program holding.
pub const ProgramDuplicates = shared.ProgramDuplicates;

/// Duplicates the handles of a program `handles` names from `host` into the Keeper, each with its
/// source's access and not inheritable. All or nothing.
pub fn pullProgram(host: Handle, handles: message.ProgramHandles) error{HandleFailed}!ProgramDuplicates {
    return shared.pullProgram(host, .{
        .job = handles.job,
        .process = handles.process,
        .streams = .{ handles.stdout, handles.stderr },
    });
}

/// Whether the Keeper's own duplicates of a program match `facts`: each object's type and access,
/// the process identity, and its membership in the Job.
pub fn programMatches(duplicates: ProgramDuplicates, facts: message.ProgramFacts) bool {
    return shared.programMatches(duplicates, .{ .pid = facts.process.pid, .created = facts.process.created });
}

/// A process's id and creation time, or null when the handle cannot be queried.
pub fn processIdentity(process: Handle) ?message.Process {
    const identity = shared.processIdentity(process) orelse return null;
    return .{ .pid = identity.pid, .created = identity.created };
}

pub fn close(handle: Handle) void {
    _ = CloseHandle(handle);
}

pub fn closeAll(handles: []const Handle) void {
    for (handles) |handle| close(handle);
}

/// Arms a one-shot thread-pool wait that runs `callback` with `context` once `process` ends.
pub fn watchEnd(process: Handle, callback: EndCallback, context: usize) ?Handle {
    var wait: ?Handle = null;
    if (!RegisterWaitForSingleObject(
        &wait,
        process,
        callback,
        @ptrFromInt(context),
        std.math.maxInt(ULONG),
        WT_EXECUTEONLYONCE,
    ).toBool()) return null;
    return wait;
}

/// Removes a wait and waits for a callback it already started. Never call it with the application
/// lock held, since the callback takes that lock.
pub fn unwatch(wait: Handle) void {
    _ = UnregisterWaitEx(wait, windows.INVALID_HANDLE_VALUE);
}

/// Removes a wait from inside its own callback, which cannot wait for itself; the one-shot wait
/// touches its object no more once its callback runs.
pub fn unwatchFromCallback(wait: Handle) void {
    _ = UnregisterWaitEx(wait, null);
}

/// A self-resetting event that wakes one connection's readiness wait.
pub fn createWakeEvent() ?Handle {
    return CreateEventW(null, .FALSE, .FALSE, null);
}

pub fn signal(event: Handle) void {
    _ = SetEvent(event);
}

pub fn currentProcessId() u32 {
    return GetCurrentProcessId();
}
