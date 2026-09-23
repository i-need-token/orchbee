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
    image: [sample.maximum_text_code_units]u16,
    command: [sample.maximum_text_code_units]u16,
    directory: [sample.maximum_text_code_units]u16,
};
comptime {
    std.debug.assert(@offsetOf(RawFacts, "image") == 24);
    std.debug.assert(@offsetOf(RawFacts, "command") == 8216);
    std.debug.assert(@offsetOf(RawFacts, "directory") == 16408);
    std.debug.assert(@sizeOf(RawFacts) == 24600);
    std.debug.assert(@alignOf(RawFacts) == 8);
}

extern fn ob_windows_job_process_ids(job: windows.HANDLE, ids: [*]u32, count: *u32) c_int;
extern fn ob_windows_open_job_process(job: windows.HANDLE, pid: u32) ?windows.HANDLE;
extern fn ob_windows_process_facts(process: windows.HANDLE, facts: *RawFacts) windows.BOOL;
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

fn decode(allocator: std.mem.Allocator, pid: u32, raw: *const RawFacts) !sample.Process {
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
        .image = image,
        .command_line = command,
        .current_directory = directory,
    };
}

fn checkBudget(timer: *const monotonic.Timer) !void {
    if (timer.read() >= 250 * std.time.ns_per_ms) return error.ProcessSampleUnavailable;
}

pub fn collect(allocator: std.mem.Allocator, job: windows.HANDLE, root: windows.HANDLE) sample.Error!sample.Sample {
    const timer = monotonic.Timer.start() catch return error.ProcessSampleUnavailable;
    const root_pid = GetProcessId(root);
    if (root_pid == 0) return error.ProcessSampleUnavailable;
    var first: [sample.maximum_processes]u32 = undefined;
    const ids = try enumerate(job, &first);
    const rows = try allocator.alloc(sample.Process, ids.len);
    var initialized: usize = 0;
    errdefer {
        for (rows[0..initialized]) |row| row.deinit(allocator);
        allocator.free(rows);
    }
    // Keep process objects pinned until both membership observations finish.
    var handles: [sample.maximum_processes]?windows.HANDLE = .{null} ** sample.maximum_processes;
    defer for (handles) |handle| {
        if (handle) |value| _ = windows.CloseHandle(value);
    };
    for (ids, 0..) |pid, index| {
        try checkBudget(&timer);
        const handle = ob_windows_open_job_process(job, pid) orelse return error.ProcessSampleChanged;
        handles[index] = handle;
        var raw: RawFacts = undefined;
        if (!ob_windows_process_facts(handle, &raw).toBool()) return error.ProcessSampleUnavailable;
        rows[index] = try decode(allocator, pid, &raw);
        initialized += 1;
    }
    try checkBudget(&timer);
    var last: [sample.maximum_processes]u32 = undefined;
    if (!std.mem.eql(u32, ids, try enumerate(job, &last))) return error.ProcessSampleChanged;
    try checkBudget(&timer);
    return .{ .allocator = allocator, .root_pid = root_pid, .processes = rows };
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const RawFactsType = RawFacts;
    pub const decodeFacts = decode;
    pub const openJobProcess = ob_windows_open_job_process;
} else struct {};
