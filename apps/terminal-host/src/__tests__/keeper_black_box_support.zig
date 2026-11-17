//! Shared pieces of the Keeper's real-process test: the Win32 calls it makes, an overlapped pipe
//! client with deadlines, the `OBHS` and `OBKP` exchanges, and process identities.

const std = @import("std");
const build_options = @import("build-options");
const handshake = @import("terminal-host-handshake");
const keeper_protocol = @import("terminal-host-keeper-protocol");
const platform = @import("terminal-host-platform");

pub const message = keeper_protocol.message;
pub const monotonic = platform.monotonic;
const windows = std.os.windows;

pub const HANDLE = windows.HANDLE;
pub const BOOL = windows.BOOL;
pub const DWORD = windows.DWORD;

pub const GENERIC_READ: DWORD = 0x8000_0000;
pub const GENERIC_WRITE: DWORD = 0x4000_0000;
pub const ERROR_ACCESS_DENIED: DWORD = 5;
const OPEN_EXISTING: DWORD = 3;
const FILE_FLAG_OVERLAPPED: DWORD = 0x4000_0000;
const SECURITY_SQOS_PRESENT: DWORD = 0x0010_0000;
const SECURITY_IDENTIFICATION: DWORD = 0x0001_0000;
const ERROR_PIPE_BUSY: DWORD = 231;
const ERROR_IO_PENDING: DWORD = 997;
pub const WAIT_OBJECT_0: DWORD = 0;
pub const WAIT_TIMEOUT: DWORD = 258;
pub const INFINITE: DWORD = 0xFFFF_FFFF;
const DUPLICATE_CLOSE_SOURCE: DWORD = 0x1;
const DUPLICATE_SAME_ACCESS: DWORD = 0x2;
const SYNCHRONIZE: DWORD = 0x0010_0000;
const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const CREATE_SUSPENDED: DWORD = 0x4;
const CREATE_NO_WINDOW: DWORD = 0x0800_0000;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x2000;
const job_object_extended_limit_information: c_int = 9;
const TOKEN_DUPLICATE: DWORD = 0x0002;
const TOKEN_QUERY: DWORD = 0x0008;
const TOKEN_ADJUST_DEFAULT: DWORD = 0x0080;
const TOKEN_ASSIGN_PRIMARY: DWORD = 0x0001;
const MAXIMUM_ALLOWED: DWORD = 0x0200_0000;
const security_impersonation: c_int = 2;
const token_primary: c_int = 1;
const token_integrity_level: c_int = 25;
const SE_GROUP_INTEGRITY: DWORD = 0x20;

pub const Overlapped = extern struct {
    internal: usize = 0,
    internal_high: usize = 0,
    offset: DWORD = 0,
    offset_high: DWORD = 0,
    event: ?HANDLE = null,
};
pub const FileTime = extern struct { low: DWORD, high: DWORD };
const StartupInfo = extern struct {
    cb: DWORD,
    reserved: ?[*:0]u16 = null,
    desktop: ?[*:0]u16 = null,
    title: ?[*:0]u16 = null,
    x: DWORD = 0,
    y: DWORD = 0,
    x_size: DWORD = 0,
    y_size: DWORD = 0,
    x_count_chars: DWORD = 0,
    y_count_chars: DWORD = 0,
    fill_attribute: DWORD = 0,
    flags: DWORD = 0,
    show_window: u16 = 0,
    reserved2_bytes: u16 = 0,
    reserved2: ?*u8 = null,
    standard_input: ?HANDLE = null,
    standard_output: ?HANDLE = null,
    standard_error: ?HANDLE = null,
};
const ProcessInformation = extern struct {
    process: HANDLE,
    thread: HANDLE,
    process_id: DWORD,
    thread_id: DWORD,
};
const BasicLimitInformation = extern struct {
    per_process_user_time_limit: i64 = 0,
    per_job_user_time_limit: i64 = 0,
    limit_flags: DWORD = 0,
    minimum_working_set_size: usize = 0,
    maximum_working_set_size: usize = 0,
    active_process_limit: DWORD = 0,
    affinity: usize = 0,
    priority_class: DWORD = 0,
    scheduling_class: DWORD = 0,
};
const ExtendedLimitInformation = extern struct {
    basic: BasicLimitInformation = .{},
    io_counters: [6]u64 = @splat(0),
    process_memory_limit: usize = 0,
    job_memory_limit: usize = 0,
    peak_process_memory_used: usize = 0,
    peak_job_memory_used: usize = 0,
};
const SidAndAttributes = extern struct { sid: *anyopaque, attributes: DWORD };

extern "kernel32" fn CreateFileW(
    name: [*:0]const u16,
    access: DWORD,
    share: DWORD,
    security: ?*anyopaque,
    disposition: DWORD,
    flags: DWORD,
    template: ?HANDLE,
) callconv(.winapi) HANDLE;
extern "kernel32" fn WaitNamedPipeW(name: [*:0]const u16, timeout: DWORD) callconv(.winapi) BOOL;
extern "kernel32" fn ReadFile(file: HANDLE, buffer: [*]u8, length: DWORD, read: ?*DWORD, overlapped: ?*Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn WriteFile(file: HANDLE, buffer: [*]const u8, length: DWORD, written: ?*DWORD, overlapped: ?*Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn GetOverlappedResult(file: HANDLE, overlapped: *Overlapped, transferred: *DWORD, wait: BOOL) callconv(.winapi) BOOL;
extern "kernel32" fn CancelIoEx(file: HANDLE, overlapped: ?*Overlapped) callconv(.winapi) BOOL;
extern "kernel32" fn CreateEventW(attributes: ?*anyopaque, manual_reset: BOOL, initial: BOOL, name: ?[*:0]const u16) callconv(.winapi) ?HANDLE;
extern "kernel32" fn SetEvent(event: HANDLE) callconv(.winapi) BOOL;
pub extern "kernel32" fn WaitForSingleObject(handle: HANDLE, milliseconds: DWORD) callconv(.winapi) DWORD;
pub extern "kernel32" fn CloseHandle(handle: HANDLE) callconv(.winapi) BOOL;
extern "kernel32" fn GetCurrentProcess() callconv(.winapi) HANDLE;
extern "kernel32" fn GetCurrentThread() callconv(.winapi) HANDLE;
extern "kernel32" fn DuplicateHandle(
    source_process: HANDLE,
    source: HANDLE,
    target_process: HANDLE,
    target: *HANDLE,
    access: DWORD,
    inherit: BOOL,
    options: DWORD,
) callconv(.winapi) BOOL;
pub extern "kernel32" fn GetProcessId(process: HANDLE) callconv(.winapi) DWORD;
extern "kernel32" fn GetProcessTimes(process: HANDLE, creation: *FileTime, exit: *FileTime, kernel: *FileTime, user: *FileTime) callconv(.winapi) BOOL;
extern "kernel32" fn OpenProcess(access: DWORD, inherit: BOOL, process_id: DWORD) callconv(.winapi) ?HANDLE;
pub extern "kernel32" fn GetExitCodeProcess(process: HANDLE, code: *DWORD) callconv(.winapi) BOOL;
extern "kernel32" fn GetSystemTimePreciseAsFileTime(time: *FileTime) callconv(.winapi) void;
extern "kernel32" fn CreateProcessW(
    application: ?[*:0]const u16,
    command_line: ?[*:0]u16,
    process_attributes: ?*anyopaque,
    thread_attributes: ?*anyopaque,
    inherit: BOOL,
    flags: DWORD,
    environment: ?*anyopaque,
    directory: ?[*:0]const u16,
    startup: *StartupInfo,
    information: *ProcessInformation,
) callconv(.winapi) BOOL;
extern "kernel32" fn CreateJobObjectW(attributes: ?*anyopaque, name: ?[*:0]const u16) callconv(.winapi) ?HANDLE;
extern "kernel32" fn SetInformationJobObject(job: HANDLE, class: c_int, information: *anyopaque, length: DWORD) callconv(.winapi) BOOL;
extern "kernel32" fn AssignProcessToJobObject(job: HANDLE, process: HANDLE) callconv(.winapi) BOOL;
extern "advapi32" fn ImpersonateAnonymousToken(thread: HANDLE) callconv(.winapi) BOOL;
extern "advapi32" fn RevertToSelf() callconv(.winapi) BOOL;
extern "advapi32" fn OpenProcessToken(process: HANDLE, access: DWORD, token: *HANDLE) callconv(.winapi) BOOL;
extern "advapi32" fn DuplicateTokenEx(
    existing: HANDLE,
    access: DWORD,
    attributes: ?*anyopaque,
    impersonation_level: c_int,
    token_type: c_int,
    token: *HANDLE,
) callconv(.winapi) BOOL;
extern "advapi32" fn SetTokenInformation(token: HANDLE, class: c_int, information: *anyopaque, length: DWORD) callconv(.winapi) BOOL;
extern "advapi32" fn CreateProcessAsUserW(
    token: HANDLE,
    application: ?[*:0]const u16,
    command_line: ?[*:0]u16,
    process_attributes: ?*anyopaque,
    thread_attributes: ?*anyopaque,
    inherit: BOOL,
    flags: DWORD,
    environment: ?*anyopaque,
    directory: ?[*:0]const u16,
    startup: *StartupInfo,
    information: *ProcessInformation,
) callconv(.winapi) BOOL;

pub fn lastError() DWORD {
    return @intFromEnum(windows.GetLastError());
}

pub fn handleValue(handle: HANDLE) u32 {
    return @intCast(@intFromPtr(handle));
}

pub fn handleFromValue(value: u64) HANDLE {
    return @ptrFromInt(@as(usize, @intCast(value)));
}

pub fn fileTimeValue(time: FileTime) u64 {
    return (@as(u64, time.high) << 32) | time.low;
}

pub fn now() u64 {
    var time: FileTime = undefined;
    GetSystemTimePreciseAsFileTime(&time);
    return fileTimeValue(time);
}

pub fn pipeName(buffer: *[96]u16, endpoint: []const u8) [:0]const u16 {
    const prefix = std.unicode.utf8ToUtf16LeStringLiteral("\\\\.\\pipe\\orchbee-terminal-host-");
    @memcpy(buffer[0..prefix.len], prefix);
    for (endpoint, 0..) |byte, index| buffer[prefix.len + index] = byte;
    buffer[prefix.len + endpoint.len] = 0;
    return buffer[0 .. prefix.len + endpoint.len :0];
}

pub const Opened = union(enum) { handle: HANDLE, failed: DWORD };

/// Opens the client end of a Keeper's endpoint as the calling thread's identity, at the
/// identification-only level the production client opens at.
pub fn openEndpoint(endpoint: []const u8, access: DWORD, overlapped: bool) Opened {
    var name_buffer: [96]u16 = undefined;
    const name = pipeName(&name_buffer, endpoint);
    var attempts: u32 = 0;
    while (true) : (attempts += 1) {
        const flags = (if (overlapped) FILE_FLAG_OVERLAPPED else 0) | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION;
        const handle = CreateFileW(name.ptr, access, 0, null, OPEN_EXISTING, flags, null);
        if (handle != windows.INVALID_HANDLE_VALUE) return .{ .handle = handle };
        const code = lastError();
        if (code != ERROR_PIPE_BUSY or attempts >= 50) return .{ .failed = code };
        _ = WaitNamedPipeW(name.ptr, 100);
    }
}

/// Whether a Keeper's endpoint has a listening instance, waiting up to `timeout_ms` for one.
pub fn endpointListening(endpoint: []const u8, timeout_ms: u32) bool {
    var name_buffer: [96]u16 = undefined;
    const name = pipeName(&name_buffer, endpoint);
    var timer = monotonic.Timer.start() catch return false;
    while (timer.read() < @as(u64, timeout_ms) * std.time.ns_per_ms) {
        if (WaitNamedPipeW(name.ptr, 50).toBool()) return true;
        monotonic.sleep(10 * std.time.ns_per_ms);
    }
    return false;
}

/// Duplicates a handle out of `process` into this process, closing it there.
pub fn takeHandle(process: HANDLE, value: u64) !HANDLE {
    var duplicate: HANDLE = undefined;
    if (!DuplicateHandle(process, handleFromValue(value), GetCurrentProcess(), &duplicate, 0, .FALSE, DUPLICATE_SAME_ACCESS | DUPLICATE_CLOSE_SOURCE).toBool())
        return error.DuplicateFailed;
    return duplicate;
}

pub fn processOf(handle: HANDLE) !message.Process {
    var creation: FileTime = undefined;
    var exit: FileTime = undefined;
    var kernel: FileTime = undefined;
    var user: FileTime = undefined;
    if (!GetProcessTimes(handle, &creation, &exit, &kernel, &user).toBool()) return error.ProcessTimes;
    return .{ .pid = GetProcessId(handle), .created = fileTimeValue(creation) };
}

pub fn selfProcess() !message.Process {
    return processOf(GetCurrentProcess());
}

/// The process's exit time, or null while it runs.
pub fn exitTime(handle: HANDLE) ?u64 {
    if (WaitForSingleObject(handle, 0) != WAIT_OBJECT_0) return null;
    var creation: FileTime = undefined;
    var exit: FileTime = undefined;
    var kernel: FileTime = undefined;
    var user: FileTime = undefined;
    if (!GetProcessTimes(handle, &creation, &exit, &kernel, &user).toBool()) return null;
    return fileTimeValue(exit);
}

/// Whether the process with this identity still runs.
pub fn alive(process: message.Process) bool {
    const handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, .FALSE, process.pid) orelse return false;
    defer _ = CloseHandle(handle);
    const identity = processOf(handle) catch return false;
    if (!identity.eql(process)) return false;
    return WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
}

/// A manual-reset event, initially unset.
pub fn manualEvent() !HANDLE {
    return CreateEventW(null, .TRUE, .FALSE, null) orelse error.SystemResources;
}

pub fn setEvent(event: HANDLE) void {
    _ = SetEvent(event);
}

pub fn exitCode(handle: HANDLE) ?DWORD {
    var code: DWORD = 0;
    if (!GetExitCodeProcess(handle, &code).toBool()) return null;
    return code;
}

/// A Job that ends its processes when its last handle closes.
pub fn killOnCloseJob() !HANDLE {
    const job = CreateJobObjectW(null, null) orelse return error.JobFailed;
    errdefer _ = CloseHandle(job);
    var limits: ExtendedLimitInformation = .{};
    limits.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(job, job_object_extended_limit_information, &limits, @sizeOf(ExtendedLimitInformation)).toBool())
        return error.JobFailed;
    return job;
}

pub fn assignSelf(job: HANDLE) !void {
    if (!AssignProcessToJobObject(job, GetCurrentProcess()).toBool()) return error.JobFailed;
}

/// Starts `command_line` suspended in `job`; it never runs.
pub fn spawnSuspended(allocator: std.mem.Allocator, application: []const u8, job: HANDLE) !HANDLE {
    const application_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, application);
    defer allocator.free(application_w);
    const command_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, "stand-in-suspended --suspended");
    defer allocator.free(command_w);
    var startup: StartupInfo = .{ .cb = @sizeOf(StartupInfo) };
    var information: ProcessInformation = undefined;
    if (!CreateProcessW(application_w.ptr, command_w.ptr, null, null, .FALSE, CREATE_SUSPENDED | CREATE_NO_WINDOW, null, null, &startup, &information).toBool())
        return error.SpawnFailed;
    _ = CloseHandle(information.thread);
    if (!AssignProcessToJobObject(job, information.process).toBool()) {
        _ = CloseHandle(information.process);
        return error.JobFailed;
    }
    return information.process;
}

/// Starts this test's executable with `arguments` under a Low-integrity copy of this process's
/// token, without inherited handles.
pub fn spawnLowIntegrity(allocator: std.mem.Allocator, application: []const u8, arguments: []const u8) !HANDLE {
    var token: HANDLE = undefined;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY, &token).toBool())
        return error.TokenFailed;
    defer _ = CloseHandle(token);
    var low_token: HANDLE = undefined;
    if (!DuplicateTokenEx(token, MAXIMUM_ALLOWED, null, security_impersonation, token_primary, &low_token).toBool())
        return error.TokenFailed;
    defer _ = CloseHandle(low_token);
    // S-1-16-4096, the Low mandatory level.
    var low_sid = [_]u8{ 1, 1, 0, 0, 0, 0, 0, 16, 0x00, 0x10, 0x00, 0x00 };
    var label: SidAndAttributes = .{ .sid = &low_sid, .attributes = SE_GROUP_INTEGRITY };
    if (!SetTokenInformation(low_token, token_integrity_level, &label, @sizeOf(SidAndAttributes) + low_sid.len).toBool())
        return error.TokenFailed;
    const application_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, application);
    defer allocator.free(application_w);
    const command_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, arguments);
    defer allocator.free(command_w);
    var startup: StartupInfo = .{ .cb = @sizeOf(StartupInfo) };
    var information: ProcessInformation = undefined;
    if (!CreateProcessAsUserW(low_token, application_w.ptr, command_w.ptr, null, null, .FALSE, CREATE_NO_WINDOW, null, null, &startup, &information).toBool())
        return error.SpawnFailed;
    _ = CloseHandle(information.thread);
    return information.process;
}

/// Opens the endpoint while this thread impersonates the anonymous token; returns the open's
/// error code, or zero when it opened.
pub fn openAsAnonymous(endpoint: []const u8, access: DWORD) !DWORD {
    if (!ImpersonateAnonymousToken(GetCurrentThread()).toBool()) return error.ImpersonationFailed;
    const opened = openEndpoint(endpoint, access, false);
    if (!RevertToSelf().toBool()) std.process.exit(99);
    return switch (opened) {
        .handle => |handle| handle: {
            _ = CloseHandle(handle);
            break :handle 0;
        },
        .failed => |code| code,
    };
}

pub const Frame = struct { magic: [4]u8, payload: []u8 };
/// What one frame read saw: a frame, the end of the connection after so many bytes, or the
/// deadline after so many bytes.
pub const Received = union(enum) { frame: Frame, closed: usize, timeout: usize };

/// One overlapped client end of a Keeper connection.
pub const Pipe = struct {
    handle: HANDLE,
    read_event: HANDLE,
    write_event: HANDLE,

    pub const Error = error{ Closed, Timeout };

    pub fn fromHandle(handle: HANDLE) !Pipe {
        const read_event = CreateEventW(null, .TRUE, .FALSE, null) orelse return error.SystemResources;
        errdefer _ = CloseHandle(read_event);
        const write_event = CreateEventW(null, .TRUE, .FALSE, null) orelse return error.SystemResources;
        return .{ .handle = handle, .read_event = read_event, .write_event = write_event };
    }

    pub fn open(endpoint: []const u8) !Pipe {
        return switch (openEndpoint(endpoint, GENERIC_READ | GENERIC_WRITE, true)) {
            .handle => |handle| fromHandle(handle),
            .failed => error.OpenFailed,
        };
    }

    pub fn close(self: *Pipe) void {
        _ = CloseHandle(self.handle);
        _ = CloseHandle(self.read_event);
        _ = CloseHandle(self.write_event);
        self.* = undefined;
    }

    pub fn writeAll(self: *Pipe, bytes: []const u8, timeout_ms: u32) Error!void {
        var offset: usize = 0;
        while (offset < bytes.len) offset += try self.transfer(.write, @constCast(bytes[offset..]), timeout_ms);
    }

    pub fn readFrame(self: *Pipe, allocator: std.mem.Allocator, timeout_ms: u32) !Received {
        var timer = try monotonic.Timer.start();
        var header: [8]u8 = undefined;
        switch (self.fill(&header, &timer, timeout_ms)) {
            .complete => {},
            .closed => |count| return .{ .closed = count },
            .timeout => |count| return .{ .timeout = count },
        }
        const length = std.mem.readInt(u32, header[4..8], .big);
        if (length > keeper_protocol.frame.maximum_payload_bytes) return error.FrameTooLong;
        const payload = try allocator.alloc(u8, length);
        switch (self.fill(payload, &timer, timeout_ms)) {
            .complete => {},
            .closed => |count| return .{ .closed = header.len + count },
            .timeout => |count| return .{ .timeout = header.len + count },
        }
        return .{ .frame = .{ .magic = header[0..4].*, .payload = payload } };
    }

    /// Reads whatever arrives until the connection ends or the deadline passes, and counts it.
    pub fn drain(self: *Pipe, timeout_ms: u32) Received {
        var timer = monotonic.Timer.start() catch return .{ .timeout = 0 };
        var buffer: [16 * 1024]u8 = undefined;
        var total: usize = 0;
        while (true) {
            const elapsed_ms = timer.read() / std.time.ns_per_ms;
            if (elapsed_ms >= timeout_ms) return .{ .timeout = total };
            total += self.transfer(.read, &buffer, @intCast(timeout_ms - elapsed_ms)) catch |err| switch (err) {
                error.Closed => return .{ .closed = total },
                error.Timeout => return .{ .timeout = total },
            };
        }
    }

    const Filled = union(enum) { complete, closed: usize, timeout: usize };

    fn fill(self: *Pipe, buffer: []u8, timer: *monotonic.Timer, timeout_ms: u32) Filled {
        var filled: usize = 0;
        while (filled < buffer.len) {
            const elapsed_ms = timer.read() / std.time.ns_per_ms;
            if (elapsed_ms >= timeout_ms) return .{ .timeout = filled };
            filled += self.transfer(.read, buffer[filled..], @intCast(timeout_ms - elapsed_ms)) catch |err| switch (err) {
                error.Closed => return .{ .closed = filled },
                error.Timeout => return .{ .timeout = filled },
            };
        }
        return .complete;
    }

    fn transfer(self: *Pipe, comptime direction: enum { read, write }, buffer: []u8, timeout_ms: u32) Error!usize {
        const event = if (direction == .read) self.read_event else self.write_event;
        var overlapped: Overlapped = .{ .event = event };
        const length: DWORD = @intCast(@min(buffer.len, 64 * 1024));
        const started = if (direction == .read)
            ReadFile(self.handle, buffer.ptr, length, null, &overlapped)
        else
            WriteFile(self.handle, buffer.ptr, length, null, &overlapped);
        var transferred: DWORD = 0;
        if (!started.toBool()) {
            if (lastError() != ERROR_IO_PENDING) return error.Closed;
            if (WaitForSingleObject(event, timeout_ms) != WAIT_OBJECT_0) {
                _ = CancelIoEx(self.handle, &overlapped);
                // A transfer that finished as it was cancelled still delivered its bytes.
                if (GetOverlappedResult(self.handle, &overlapped, &transferred, .TRUE).toBool() and transferred > 0)
                    return transferred;
                return error.Timeout;
            }
        }
        if (!GetOverlappedResult(self.handle, &overlapped, &transferred, .FALSE).toBool()) return error.Closed;
        if (transferred == 0) return error.Closed;
        return transferred;
    }
};

pub const Answer = union(enum) {
    reply: struct { payload: []u8, elapsed_ns: u64 },
    closed,
    timeout,
};

/// One Keeper connection as a Host or the Worker sees it.
pub const Session = struct {
    pipe: Pipe,
    next_id: u32 = 1,

    pub fn close(self: *Session) void {
        self.pipe.close();
    }

    /// Offers `keeper` as a client of this build would and requires its answer.
    pub fn offer(self: *Session, arena: std.mem.Allocator) !void {
        const profile = handshake.registry.keeperProfile(
            build_options.host_version,
            .{ .commit = build_options.source_commit },
            build_options.environment,
        );
        const frame = try handshake.encode.initiatorFrame(arena, .{ .offer = .{
            .connection = profile.connection,
            .generations = profile.generations,
            .capabilities = profile.capabilities,
            .product_version = profile.product_version,
            .build = profile.build,
            .environment = profile.environment,
            .fingerprint = profile.fingerprint,
        } });
        try self.pipe.writeAll(frame, 3000);
        const answer = switch (try self.pipe.readFrame(arena, 4000)) {
            .frame => |received| received,
            else => return error.NoAnswer,
        };
        if (!std.mem.eql(u8, &answer.magic, handshake.frame.magic)) return error.UnexpectedFrame;
        switch (try handshake.decode.acceptorPayload(arena, answer.payload)) {
            .answer => {},
            else => return error.OfferRefused,
        }
    }

    /// Sends one control request in place of an offer and returns the Keeper's answer.
    pub fn control(self: *Session, arena: std.mem.Allocator, kind: handshake.message.RequestKind) !handshake.message.AcceptorMessage {
        const frame = try handshake.encode.initiatorFrame(arena, .{ .request = .{
            .kind = kind,
            .environment = build_options.environment,
        } });
        try self.pipe.writeAll(frame, 3000);
        const answer = switch (try self.pipe.readFrame(arena, 4000)) {
            .frame => |received| received,
            else => return error.NoAnswer,
        };
        return handshake.decode.acceptorPayload(arena, answer.payload);
    }

    pub fn id(self: *Session) u32 {
        defer self.next_id += 1;
        return self.next_id;
    }

    /// Sends one request and reads its reply within the reply deadline and a margin.
    pub fn send(self: *Session, arena: std.mem.Allocator, request: message.Request) !Answer {
        const frame = try keeper_protocol.encode.initiatorFrame(arena, .{ .request = request });
        return self.sendFrame(arena, frame);
    }

    pub fn sendPayload(self: *Session, arena: std.mem.Allocator, payload: []const u8) !Answer {
        return self.sendFrame(arena, try keeper_protocol.frame.encode(arena, payload));
    }

    pub fn sendFrame(self: *Session, arena: std.mem.Allocator, frame: []const u8) !Answer {
        var timer = try monotonic.Timer.start();
        self.pipe.writeAll(frame, 3000) catch |err| return switch (err) {
            error.Closed => .closed,
            error.Timeout => .timeout,
        };
        return switch (try self.pipe.readFrame(arena, 4000)) {
            .frame => |received| .{ .reply = .{ .payload = received.payload, .elapsed_ns = timer.read() } },
            .closed => .closed,
            .timeout => .timeout,
        };
    }

    /// Sends a request and returns its decoded reply body; anything else fails.
    pub fn call(self: *Session, arena: std.mem.Allocator, body: message.Request.Body) !message.Reply.Body {
        const request_id = self.id();
        const answer = try self.send(arena, .{ .id = request_id, .body = body });
        const reply = switch (answer) {
            .reply => |received| try decodeReply(arena, received.payload),
            .closed => return error.ConnectionClosed,
            .timeout => return error.ReplyTimeout,
        };
        if (reply.id != request_id) return error.ReplyIdMismatch;
        return reply.body;
    }

    /// `hello` for `role`; the reply body.
    pub fn hello(self: *Session, arena: std.mem.Allocator, role: message.Role) !message.Reply.Body {
        return self.call(arena, .{ .hello = role });
    }
};

pub fn decodeReply(arena: std.mem.Allocator, payload: []const u8) !message.Reply {
    return switch (try keeper_protocol.decode.acceptorPayload(arena, payload)) {
        .reply => |reply| reply,
        .event => error.UnexpectedEvent,
    };
}

pub fn encodeReply(arena: std.mem.Allocator, reply: message.Reply) ![]u8 {
    return keeper_protocol.encode.acceptorFrame(arena, .{ .reply = reply });
}

pub fn refusedWith(body: message.Reply.Body, reason: message.Reason) bool {
    return switch (body) {
        .refused => |actual| actual == reason,
        else => false,
    };
}

pub fn reasonName(body: message.Reply.Body) []const u8 {
    return switch (body) {
        .refused => |reason| reason.name(),
        else => @tagName(body),
    };
}
