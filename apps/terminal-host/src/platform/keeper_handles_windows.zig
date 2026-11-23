//! The seven handles of one holding, as the process that pulls them sees them: opening the process
//! that holds them, pulling them by the values a Keeper message names, and the checks on the
//! pulled duplicates. The Keeper pulls from a Host at `hold`, and a claiming Host pulls from the
//! Keeper after `inspect`. None of them reads, writes, or names a pipe, so none waits behind
//! another process's blocked synchronous read of a held pipe, as probe P1 measured.

const std = @import("std");
const builtin = @import("builtin");

const windows = std.os.windows;
const Handle = windows.HANDLE;
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
const object_basic_information: c_int = 0;
const object_type_information: c_int = 2;
/// `PUBLIC_OBJECT_BASIC_INFORMATION`, which `NtQueryObject` takes at exactly this size.
const basic_information_bytes = 56;

/// job, program, console, signal, reference, input, output.
pub const role_count = 7;
pub const Duplicates = [role_count]Handle;
/// The handle values a Keeper message names, in role order.
pub const Values = [role_count]u32;

pub const Role = enum(u3) { job, program, console, signal, reference, input, output };

pub const Identity = struct {
    pid: u32,
    /// The process's creation time as a `FILETIME`, never zero.
    created: u64,

    pub fn eql(self: Identity, other: Identity) bool {
        return self.pid == other.pid and self.created == other.created;
    }
};

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

const ObjectType = enum { job, process, file, other };

const expected_types = [role_count]ObjectType{ .job, .process, .process, .file, .file, .file, .file };
/// What each duplicate must grant for these checks and its pipe direction: the Job a membership
/// query, both processes a query, the write ends writing, and the read end reading.
const required_access = [role_count]DWORD{
    JOB_OBJECT_QUERY,
    0,
    0,
    FILE_WRITE_DATA,
    0,
    FILE_WRITE_DATA,
    FILE_READ_DATA,
};

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
extern "ntdll" fn NtQueryObject(
    handle: ?Handle,
    class: c_int,
    information: ?*anyopaque,
    length: ULONG,
    returned: ?*ULONG,
) callconv(.winapi) i32;
extern fn ob_windows_terminal_job_kills_on_close(job: Handle, kills: *BOOL) BOOL;

pub const OpenError = error{ HandleFailed, IdentityMismatch };

/// Opens `process` for its end, its creation time, and duplication from it, and checks that the
/// process the id names now is the one that was meant.
pub fn openPeer(process: Identity) OpenError!Handle {
    const access = SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_DUP_HANDLE;
    const handle = OpenProcess(access, .FALSE, process.pid) orelse return error.HandleFailed;
    errdefer close(handle);
    const identity = processIdentity(handle) orelse return error.HandleFailed;
    if (!identity.eql(process)) return error.IdentityMismatch;
    return handle;
}

/// Duplicates the seven handles `values` names from `peer` into this process, each with its
/// source's access and not inheritable. All or nothing.
pub fn pull(peer: Handle, values: Values) error{HandleFailed}!Duplicates {
    var duplicates: Duplicates = undefined;
    var made: usize = 0;
    errdefer closeAll(duplicates[0..made]);
    for (values) |value| {
        var target: Handle = undefined;
        if (!DuplicateHandle(peer, @ptrFromInt(value), GetCurrentProcess(), &target, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool())
            return error.HandleFailed;
        duplicates[made] = target;
        made += 1;
    }
    return duplicates;
}

/// Whether pulled duplicates match a holding's facts: each object's type and access, both process
/// identities, and the program's membership in the Job.
pub fn matches(duplicates: Duplicates, program: Identity, console: Identity) bool {
    for (duplicates, expected_types, required_access) |handle, kind, access| {
        if (objectType(handle) != kind) return false;
        const granted = grantedAccess(handle) orelse return false;
        if (granted & access != access) return false;
    }
    const program_handle = duplicates[@intFromEnum(Role.program)];
    const console_handle = duplicates[@intFromEnum(Role.console)];
    for ([_]Handle{ program_handle, console_handle }) |handle| {
        const granted = grantedAccess(handle) orelse return false;
        if (granted & (PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION) == 0) return false;
    }
    const program_identity = processIdentity(program_handle) orelse return false;
    const console_identity = processIdentity(console_handle) orelse return false;
    if (!program_identity.eql(program) or !console_identity.eql(console)) return false;
    var in_job: BOOL = .FALSE;
    if (!IsProcessInJob(program_handle, duplicates[@intFromEnum(Role.job)], &in_job).toBool()) return false;
    return in_job.toBool();
}

/// Whether the Job of pulled duplicates ends its members when its last handle closes, as every
/// terminal's Job does; a claiming Host adopts no Job without it.
pub fn jobKillsOnClose(duplicates: Duplicates) bool {
    return killsOnClose(duplicates[@intFromEnum(Role.job)]);
}

/// A background program's handles, as the process that pulled them holds them: its Job, its
/// process, and the read end of each stream its Host captures.
pub const ProgramDuplicates = struct {
    job: Handle,
    process: Handle,
    streams: [2]?Handle,

    pub fn close(self: ProgramDuplicates) void {
        _ = CloseHandle(self.job);
        _ = CloseHandle(self.process);
        for (self.streams) |stream| if (stream) |handle| {
            _ = CloseHandle(handle);
        };
    }

    /// The values a Keeper message names; null when one exceeds 2^32.
    pub fn values(self: ProgramDuplicates) ?ProgramValues {
        var streams: [2]?u32 = .{ null, null };
        for (self.streams, &streams) |stream, *value| if (stream) |handle| {
            value.* = std.math.cast(u32, @intFromPtr(handle)) orelse return null;
        };
        return .{
            .job = std.math.cast(u32, @intFromPtr(self.job)) orelse return null,
            .process = std.math.cast(u32, @intFromPtr(self.process)) orelse return null,
            .streams = streams,
        };
    }
};

pub const ProgramValues = struct { job: u32, process: u32, streams: [2]?u32 };

/// Duplicates a program's handles from `peer` into this process, each with its source's access
/// and not inheritable. All or nothing.
pub fn pullProgram(peer: Handle, source: ProgramValues) error{HandleFailed}!ProgramDuplicates {
    const job = try duplicate(peer, source.job);
    errdefer close(job);
    const process_handle = try duplicate(peer, source.process);
    errdefer close(process_handle);
    var streams: [2]?Handle = .{ null, null };
    errdefer for (streams) |stream| if (stream) |handle| close(handle);
    for (source.streams, &streams) |value, *stream| if (value) |handle| {
        stream.* = try duplicate(peer, handle);
    };
    return .{ .job = job, .process = process_handle, .streams = streams };
}

/// Duplicates one of this process's own handles, with its access and not inheritable.
pub fn duplicateOwn(handle: Handle) error{HandleFailed}!Handle {
    var target: Handle = undefined;
    if (!DuplicateHandle(GetCurrentProcess(), handle, GetCurrentProcess(), &target, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool())
        return error.HandleFailed;
    return target;
}

fn duplicate(peer: Handle, value: u32) error{HandleFailed}!Handle {
    var target: Handle = undefined;
    if (!DuplicateHandle(peer, @ptrFromInt(value), GetCurrentProcess(), &target, 0, .FALSE, DUPLICATE_SAME_ACCESS).toBool())
        return error.HandleFailed;
    return target;
}

/// Whether pulled program duplicates match the program's identity: the Job may be queried, the
/// process is `program` and a member of that Job, and each stream is a readable file.
pub fn programMatches(duplicates: ProgramDuplicates, program: Identity) bool {
    if (objectType(duplicates.job) != .job) return false;
    const job_access = grantedAccess(duplicates.job) orelse return false;
    if (job_access & JOB_OBJECT_QUERY == 0) return false;
    if (objectType(duplicates.process) != .process) return false;
    const process_access = grantedAccess(duplicates.process) orelse return false;
    if (process_access & (PROCESS_QUERY_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION) == 0) return false;
    if (process_access & SYNCHRONIZE == 0) return false;
    for (duplicates.streams) |stream| if (stream) |handle| {
        if (objectType(handle) != .file) return false;
        const access = grantedAccess(handle) orelse return false;
        if (access & FILE_READ_DATA == 0) return false;
    };
    const identity = processIdentity(duplicates.process) orelse return false;
    if (!identity.eql(program)) return false;
    var in_job: BOOL = .FALSE;
    if (!IsProcessInJob(duplicates.process, duplicates.job, &in_job).toBool()) return false;
    return in_job.toBool();
}

/// Whether a Job ends its members when its last handle closes, as every program's Job does.
pub fn killsOnClose(job: Handle) bool {
    var kills: BOOL = .FALSE;
    if (!ob_windows_terminal_job_kills_on_close(job, &kills).toBool()) return false;
    return kills.toBool();
}

/// A process's id and creation time, or null when the handle cannot be queried.
pub fn processIdentity(process: Handle) ?Identity {
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
