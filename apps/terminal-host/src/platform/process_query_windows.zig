const std = @import("std");
const windows = std.os.windows;
const monotonic = @import("monotonic.zig");
const sample = @import("process_sample.zig");

const RawFacts = extern struct {
    creation_time: u64,
    parent_pid: u32,
    image_length: u32,
    command_length: u32,
    directory_length: u32,
    console_host: u32,
    reserved: u32,
    image: [sample.maximum_text_code_units]u16,
    command: [sample.maximum_text_code_units]u16,
    directory: [sample.maximum_text_code_units]u16,
};
comptime {
    std.debug.assert(@offsetOf(RawFacts, "image") == 32);
    std.debug.assert(@offsetOf(RawFacts, "command") == 8224);
    std.debug.assert(@offsetOf(RawFacts, "directory") == 16416);
    std.debug.assert(@sizeOf(RawFacts) == 24608);
    std.debug.assert(@alignOf(RawFacts) == 8);
}

const console_unknown = std.math.maxInt(u32);
const open_member = 1;
const open_denied = 2;
const facts_read = 1;
const facts_withheld = 2;

extern fn ob_windows_job_process_ids(job: windows.HANDLE, ids: [*]u32, count: *u32) c_int;
extern fn ob_windows_open_job_process(job: windows.HANDLE, pid: u32, out: *?windows.HANDLE) c_int;
extern fn ob_windows_process_facts(process: windows.HANDLE, facts: *RawFacts) c_int;
extern fn ob_windows_console_host(process: windows.HANDLE) u32;
extern fn ob_windows_process_identities(pids: [*]const u32, count: u32, creation: [*]u64, parent: [*]u32) windows.BOOL;
extern "kernel32" fn GetProcessId(process: windows.HANDLE) callconv(.winapi) u32;

fn enumerate(job: windows.HANDLE, buffer: *[sample.maximum_processes]u32) ![]u32 {
    var count: u32 = 0;
    switch (ob_windows_job_process_ids(job, buffer, &count)) {
        1 => {},
        2 => return error.ProcessSampleLimit,
        else => return error.ProcessSampleUnavailable,
    }
    if (count > buffer.len) return error.ProcessSampleLimit;
    const values = buffer[0..count];
    std.mem.sort(u32, values, {}, std.sort.asc(u32));
    for (values, 0..) |pid, index| {
        if (pid == 0 or (index > 0 and values[index - 1] == pid))
            return error.ProcessSampleUnavailable;
    }
    return values;
}

fn text(allocator: std.mem.Allocator, buffer: []const u16, length: u32) sample.Error!?[]const u8 {
    if (length == std.math.maxInt(u32)) return null;
    if (length > buffer.len) return error.ProcessSampleUnavailable;
    const value = buffer[0..length];
    if (std.mem.indexOfScalar(u16, value, 0) != null) return null;
    return std.unicode.utf16LeToUtf8Alloc(allocator, value) catch |err| switch (err) {
        error.DanglingSurrogateHalf, error.ExpectedSecondSurrogateHalf, error.UnexpectedSecondSurrogateHalf => null,
        error.OutOfMemory => return error.OutOfMemory,
    };
}

/// `reference` is the live root's console host, or null when it is unknown.
fn console(host: u32, reference: ?u32) sample.Console {
    if (host == 0) return .none;
    if (host == console_unknown) return .unknown;
    const root = reference orelse return .unknown;
    return if (host == root) .attached else .detached;
}

fn decode(allocator: std.mem.Allocator, pid: u32, raw: *const RawFacts, reference: ?u32) !sample.Process {
    if (pid == 0 or raw.creation_time == 0) return error.ProcessSampleUnavailable;
    const image = try text(allocator, &raw.image, raw.image_length);
    errdefer if (image) |value| allocator.free(value);
    const command = try text(allocator, &raw.command, raw.command_length);
    errdefer if (command) |value| allocator.free(value);
    const directory = try text(allocator, &raw.directory, raw.directory_length);
    return .{
        .pid = pid,
        .parent_pid = if (raw.parent_pid == 0) null else raw.parent_pid,
        .creation_time = raw.creation_time,
        .console = console(raw.console_host, reference),
        .image = image,
        .command_line = command,
        .current_directory = directory,
    };
}

fn checkBudget(timer: *const monotonic.Timer) !void {
    if (timer.read() >= 250 * std.time.ns_per_ms) return error.ProcessSampleUnavailable;
}

/// The root's console is the Terminal's console only while the root is live.
fn consoleReference(root: windows.HANDLE) ?u32 {
    const host = ob_windows_console_host(root);
    return if (host == 0 or host == console_unknown) null else host;
}

/// Members that withhold limited query rights become opaque rows. Their identity
/// comes from one system snapshot, and a second Job enumeration then confirms each
/// PID is still a member, so a recycled PID never inherits a published identity.
fn appendOpaque(job: windows.HANDLE, pids: []const u32, rows: []sample.Process, initialized: *usize) !void {
    var creation: [sample.maximum_processes]u64 = undefined;
    var parent: [sample.maximum_processes]u32 = undefined;
    if (!ob_windows_process_identities(pids.ptr, @intCast(pids.len), &creation, &parent).toBool())
        return error.ProcessSampleUnavailable;
    var buffer: [sample.maximum_processes]u32 = undefined;
    const members = try enumerate(job, &buffer);
    for (pids, 0..) |pid, index| {
        if (creation[index] == 0 or std.sort.binarySearch(u32, members, pid, orderPid) == null) continue;
        rows[initialized.*] = .{
            .pid = pid,
            .parent_pid = if (parent[index] == 0) null else parent[index],
            .creation_time = creation[index],
            .console = .unknown,
            .image = null,
            .command_line = null,
            .current_directory = null,
        };
        initialized.* += 1;
    }
}

fn orderPid(key: u32, item: u32) std.math.Order {
    return std.math.order(key, item);
}

fn lessPid(_: void, left: sample.Process, right: sample.Process) bool {
    return left.pid < right.pid;
}

/// Each row is a member that was live when it was read. A member that exits during
/// the sample is omitted rather than failing the sample, and a member that denies
/// limited rights is an explicit opaque row rather than a hidden one.
pub fn collect(allocator: std.mem.Allocator, job: windows.HANDLE, root: windows.HANDLE) sample.Error!sample.Sample {
    const timer = monotonic.Timer.start() catch return error.ProcessSampleUnavailable;
    const root_pid = GetProcessId(root);
    if (root_pid == 0) return error.ProcessSampleUnavailable;
    const reference = consoleReference(root);
    var first: [sample.maximum_processes]u32 = undefined;
    const ids = try enumerate(job, &first);
    const rows = try allocator.alloc(sample.Process, ids.len);
    var initialized: usize = 0;
    errdefer {
        for (rows[0..initialized]) |row| row.deinit(allocator);
        allocator.free(rows);
    }
    // Keep process objects pinned until every row is published.
    var handles: [sample.maximum_processes]?windows.HANDLE = .{null} ** sample.maximum_processes;
    defer for (handles) |handle| {
        if (handle) |value| _ = windows.CloseHandle(value);
    };
    var opaque_pids: [sample.maximum_processes]u32 = undefined;
    var opaque_count: usize = 0;
    for (ids, 0..) |pid, index| {
        try checkBudget(&timer);
        var handle: ?windows.HANDLE = null;
        const opened = ob_windows_open_job_process(job, pid, &handle);
        handles[index] = handle;
        var raw: RawFacts = undefined;
        const read = if (opened == open_member) ob_windows_process_facts(handle.?, &raw) else 0;
        if (opened == open_denied or read == facts_withheld) {
            opaque_pids[opaque_count] = pid;
            opaque_count += 1;
        } else if (read == facts_read) {
            rows[initialized] = try decode(allocator, pid, &raw, reference);
            initialized += 1;
        }
    }
    if (opaque_count > 0) {
        try checkBudget(&timer);
        try appendOpaque(job, opaque_pids[0..opaque_count], rows, &initialized);
    }
    try checkBudget(&timer);
    std.mem.sort(sample.Process, rows[0..initialized], {}, lessPid);
    const processes = allocator.realloc(rows, initialized) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
    };
    return .{ .allocator = allocator, .root_pid = root_pid, .processes = processes };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const RawFactsType = RawFacts;
    pub const decodeFacts = decode;
    pub const openJobProcess = ob_windows_open_job_process;
} else struct {};
