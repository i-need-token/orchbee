//! The Keeper's Win32 handle operations: opening a registered Host, pulling a Host's seven handles,
//! the checks a hold makes on its own duplicates, and the wait that sees a Host end. None of them
//! reads, writes, or names a pipe, so none waits behind another process's blocked synchronous read
//! of a held pipe, as probe P1 measured.

const std = @import("std");
const builtin = @import("builtin");
const keeper_protocol = @import("terminal-host-keeper-protocol");

const message = keeper_protocol.message;
const windows = std.os.windows;

pub const Handle = windows.HANDLE;
const BOOL = windows.BOOL;
const DWORD = windows.DWORD;
const ULONG = u32;

const SYNCHRONIZE: DWORD = 0x0010_0000;
const PROCESS_DUP_HANDLE: DWORD = 0x0040;
const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;
const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const JOB_OBJECT_QUERY: DWORD = 0x0004;
const FILE_READ_DATA: DWORD = 0x0001;
const FILE_WRITE_DATA: DWORD = 0x0002;
const DUPLICATE_SAME_ACCESS: DWORD = 0x0002;
const WT_EXECUTEONLYONCE: ULONG = 0x0000_0008;
const object_basic_information: c_int = 0;
const object_type_information: c_int = 2;
/// `PUBLIC_OBJECT_BASIC_INFORMATION`, which `NtQueryObject` takes at exactly this size.
const basic_information_bytes = 56;

const FileTime = extern struct { low: DWORD, high: DWORD };
const UnicodeString = extern struct { length: u16, maximum_length: u16, buffer: ?[*]const u16 };
const BasicInformation = extern struct {
    attributes: ULONG,
    granted_access: DWORD,
    handle_count: ULONG,
    pointer_count: ULONG,
    reserved: [10]ULONG,
};

comptime {
    if (builtin.os.tag == .windows) std.debug.assert(@sizeOf(BasicInformation) == basic_information_bytes);
}

/// The callback a Host-end wait runs on a thread-pool thread, with the context it was armed with.
pub const EndCallback = *const fn (context: ?*anyopaque, timed_out: u8) callconv(.winapi) void;

extern "kernel32" fn GetCurrentProcess() callconv(.winapi) Handle;
extern "kernel32" fn OpenProcess(access: DWORD, inherit: BOOL, process_id: DWORD) callconv(.winapi) ?Handle;
extern "kernel32" fn DuplicateHandle(
    source_process: Handle,
    source: Handle,
    target_process: Handle,
    target: *Handle,
    access: DWORD,
    inherit: BOOL,
    options: DWORD,
) callconv(.winapi) BOOL;
extern "kernel32" fn CloseHandle(handle: Handle) callconv(.winapi) BOOL;
extern "kernel32" fn GetProcessId(process: Handle) callconv(.winapi) DWORD;
extern "kernel32" fn GetProcessTimes(
    process: Handle,
    creation: *FileTime,
    exit: *FileTime,
    kernel: *FileTime,
    user: *FileTime,
) callconv(.winapi) BOOL;
extern "kernel32" fn IsProcessInJob(process: Handle, job: ?Handle, result: *BOOL) callconv(.winapi) BOOL;
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
extern "ntdll" fn NtQueryObject(
    handle: ?Handle,
    class: c_int,
    information: ?*anyopaque,
    length: ULONG,
    returned: ?*ULONG,
) callconv(.winapi) i32;

/// The Keeper's duplicates of one holding, in the order of `message.Handles.roles`.
pub const Duplicates = [message.Handles.roles.len]Handle;

const Role = enum { job, program, console, signal, reference, input, output };
const ObjectType = enum { job, process, file, other };

const expected_types = [_]ObjectType{ .job, .process, .process, .file, .file, .file, .file };
/// What each duplicate must grant for the Keeper's own checks and its pipe direction: the Job a
/// membership query, both processes a query, the write ends writing, and the read end reading.
const required_access = [_]DWORD{
    JOB_OBJECT_QUERY,
    0,
    0,
    FILE_WRITE_DATA,
    0,
    FILE_WRITE_DATA,
    FILE_READ_DATA,
};

pub const OpenError = error{ HandleFailed, IdentityMismatch };

/// Opens a Host the Worker registers, for its end, its creation time, and duplication from it.
pub fn openHost(process: message.Process) OpenError!Handle {
    const access = SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_DUP_HANDLE;
    const handle = OpenProcess(access, .FALSE, process.pid) orelse return error.HandleFailed;
    errdefer close(handle);
    const identity = processIdentity(handle) orelse return error.HandleFailed;
    if (!identity.eql(process)) return error.IdentityMismatch;
    return handle;
}

/// Duplicates the seven handles `handles` names from `host` into the Keeper, each with its
/// source's access and not inheritable. All or nothing.
pub fn pull(host: Handle, handles: message.Handles) error{HandleFailed}!Duplicates {
    const values = [_]u32{
        handles.job,       handles.program, handles.console, handles.signal,
        handles.reference, handles.input,   handles.output,
    };
    var duplicates: Duplicates = undefined;
    var made: usize = 0;
    errdefer closeAll(duplicates[0..made]);
    for (values) |value| {
        var target: Handle = undefined;
        if (!DuplicateHandle(
            host,
            @ptrFromInt(value),
            GetCurrentProcess(),
            &target,
            0,
            .FALSE,
            DUPLICATE_SAME_ACCESS,
        ).toBool()) return error.HandleFailed;
        duplicates[made] = target;
        made += 1;
    }
    return duplicates;
}

/// Whether the Keeper's own duplicates match `facts`: each object's type and access, both process
/// identities, and the program's membership in the Job.
pub fn matches(duplicates: Duplicates, facts: message.Facts) bool {
    for (duplicates, expected_types, required_access) |handle, kind, access| {
        if (objectType(handle) != kind) return false;
        const granted = grantedAccess(handle) orelse return false;
        if (granted & access != access) return false;
    }
    const program = duplicates[@intFromEnum(Role.program)];
    const console = duplicates[@intFromEnum(Role.console)];
    for ([_]Handle{ program, console }) |handle| {
        const granted = grantedAccess(handle) orelse return false;
        if (granted & (PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION) == 0) return false;
    }
    const program_identity = processIdentity(program) orelse return false;
    const console_identity = processIdentity(console) orelse return false;
    if (!program_identity.eql(facts.program) or !console_identity.eql(facts.console)) return false;
    var in_job: BOOL = .FALSE;
    if (!IsProcessInJob(program, duplicates[@intFromEnum(Role.job)], &in_job).toBool()) return false;
    return in_job.toBool();
}

/// A process's id and creation time, or null when the handle cannot be queried.
pub fn processIdentity(process: Handle) ?message.Process {
    const pid = GetProcessId(process);
    if (pid == 0) return null;
    var creation: FileTime = undefined;
    var exit: FileTime = undefined;
    var kernel: FileTime = undefined;
    var user: FileTime = undefined;
    if (!GetProcessTimes(process, &creation, &exit, &kernel, &user).toBool()) return null;
    const created = (@as(u64, creation.high) << 32) | creation.low;
    if (created == 0) return null;
    return .{ .pid = pid, .created = created };
}

fn objectType(handle: Handle) ObjectType {
    var buffer: [512]u8 align(@alignOf(UnicodeString)) = undefined;
    var returned: ULONG = 0;
    if (NtQueryObject(handle, object_type_information, &buffer, buffer.len, &returned) != 0) return .other;
    const name: *const UnicodeString = @ptrCast(&buffer);
    const units = name.buffer orelse return .other;
    const text = units[0 .. name.length / 2];
    const names = [_]struct { []const u16, ObjectType }{
        .{ std.unicode.utf8ToUtf16LeStringLiteral("Job"), .job },
        .{ std.unicode.utf8ToUtf16LeStringLiteral("Process"), .process },
        .{ std.unicode.utf8ToUtf16LeStringLiteral("File"), .file },
    };
    for (names) |entry| if (std.mem.eql(u16, text, entry[0])) return entry[1];
    return .other;
}

fn grantedAccess(handle: Handle) ?DWORD {
    var information: BasicInformation = undefined;
    var returned: ULONG = 0;
    if (NtQueryObject(handle, object_basic_information, &information, basic_information_bytes, &returned) != 0) {
        return null;
    }
    return information.granted_access;
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
