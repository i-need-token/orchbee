//! A background program's process: started on Windows without a console window inside a fresh
//! kill-on-close Job, with the null device as input and each output sent to the null device or to
//! a pipe the caller reads. It owns no table, thread, or protocol; the Host's program owner does.
//! Other systems start no background program in the Host.

const std = @import("std");
const builtin = @import("builtin");
const process_common = @import("process_common.zig");
const process_types = @import("process_types.zig");
const windows_command_line = @import("windows_command_line.zig");

const is_windows = builtin.os.tag == .windows;
const windows = std.os.windows;

pub const Handle = if (is_windows) windows.HANDLE else void;

/// The two outputs a program may send to the caller, in this order.
pub const Stream = enum(u1) { stdout, stderr };

pub const Launch = struct {
    /// An absolute path to the program, or to a batch script run through the command processor.
    executable: []const u8,
    arguments: []const []const u8,
    /// Appended unquoted after the quoted arguments; refused for a batch script.
    raw_argument: ?[]const u8 = null,
    /// An absolute directory.
    directory: []const u8,
    environment: []const process_types.EnvironmentDelta,
    /// Whether each of stdout and stderr reaches a pipe instead of the null device.
    capture: [2]bool,
};

pub const Spawned = struct {
    process: Handle,
    job: Handle,
    /// The read end of each captured stream.
    streams: [2]?Handle,
    pid: u32,
    created: u64,
};

pub const SpawnError = error{
    OutOfMemory,
    /// This system starts no background program in the Host.
    Unsupported,
    /// The command cannot be expressed: a relative path, a batch script with a raw argument, or
    /// text Windows cannot carry.
    InvalidCommand,
    /// No program exists at the path.
    NotFound,
    SpawnFailed,
};

const Native = extern struct {
    process: ?windows.HANDLE,
    job: ?windows.HANDLE,
    output: ?windows.HANDLE,
    errors: ?windows.HANDLE,
    pid: windows.DWORD,
};

extern fn ob_windows_program_spawn(
    application: [*:0]const u16,
    command_line: [*:0]u16,
    environment: [*]const u16,
    directory: [*:0]const u16,
    capture_output: windows.BOOL,
    capture_errors: windows.BOOL,
    spawned: *Native,
) windows.BOOL;
extern fn ob_windows_terminal_job_terminate(job: windows.HANDLE, exit_code: windows.UINT) windows.BOOL;
extern fn ob_windows_terminal_job_active_processes(job: windows.HANDLE, active: *windows.DWORD) windows.BOOL;
extern "kernel32" fn ReadFile(file: windows.HANDLE, buffer: [*]u8, bytes: windows.DWORD, read: *windows.DWORD, overlapped: ?*anyopaque) callconv(.winapi) windows.BOOL;
extern "kernel32" fn WaitForSingleObject(handle: windows.HANDLE, milliseconds: windows.DWORD) callconv(.winapi) windows.DWORD;
extern "kernel32" fn GetExitCodeProcess(process: windows.HANDLE, code: *windows.DWORD) callconv(.winapi) windows.BOOL;
extern "kernel32" fn GetSystemDirectoryW(buffer: [*]u16, size: windows.UINT) callconv(.winapi) windows.UINT;

pub fn spawn(allocator: std.mem.Allocator, launch: Launch) SpawnError!Spawned {
    if (comptime !is_windows) return error.Unsupported;
    if (!std.fs.path.isAbsoluteWindows(launch.executable) or !std.fs.path.isAbsoluteWindows(launch.directory))
        return error.InvalidCommand;
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const executable = try arena.dupe(u8, launch.executable);
    std.mem.replaceScalar(u8, executable, '/', '\\');
    const batch = windows_command_line.isBatch(executable);
    const command_line = if (batch) line: {
        if (launch.raw_argument != null) return error.InvalidCommand;
        break :line windows_command_line.batch(arena, executable, launch.arguments) catch |err| switch (err) {
            error.OutOfMemory => return error.OutOfMemory,
            else => return error.InvalidCommand,
        };
    } else line: {
        const argv = try arena.alloc([]const u8, launch.arguments.len + 1);
        argv[0] = executable;
        @memcpy(argv[1..], launch.arguments);
        const quoted = windows_command_line.executable(arena, argv) catch return error.OutOfMemory;
        const raw = launch.raw_argument orelse break :line quoted;
        break :line try std.mem.concatWithSentinel(arena, u8, &.{ quoted, " ", raw }, 0);
    };
    const application = if (batch) try systemCommandProcessor(arena) else executable;
    const directory = try windows_command_line.childCurrentDirectory(arena, launch.directory);
    var environment = process_common.inheritedEnvironmentForLaunch(arena, launch.environment) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.SpawnFailed,
    };
    const environment_w = environment.createWindowsBlock(arena, .{}) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        error.InvalidWtf8 => return error.InvalidCommand,
    };
    const application_w = utf16(arena, application) catch |err| return err;
    const command_line_w = utf16(arena, command_line) catch |err| return err;
    const directory_w = utf16(arena, directory) catch |err| return err;
    var native: Native = std.mem.zeroes(Native);
    if (!ob_windows_program_spawn(
        application_w.ptr,
        command_line_w.ptr,
        environment_w.slice.ptr,
        directory_w.ptr,
        if (launch.capture[0]) .TRUE else .FALSE,
        if (launch.capture[1]) .TRUE else .FALSE,
        &native,
    ).toBool()) return switch (windows.GetLastError()) {
        .FILE_NOT_FOUND, .PATH_NOT_FOUND => error.NotFound,
        .NOT_ENOUGH_MEMORY, .OUTOFMEMORY => error.OutOfMemory,
        else => error.SpawnFailed,
    };
    const process = native.process.?;
    const identity = @import("keeper_handles_windows.zig").processIdentity(process);
    return .{
        .process = process,
        .job = native.job.?,
        .streams = .{ native.output, native.errors },
        .pid = native.pid,
        // A process this Host just created always answers its times; zero never names one.
        .created = if (identity) |known| known.created else 0,
    };
}

/// Reads what a captured stream has; 0 once every writer has closed it or the read failed.
pub fn read(stream: Handle, buffer: []u8) usize {
    if (comptime !is_windows) return 0;
    var count: windows.DWORD = 0;
    const length: windows.DWORD = @intCast(@min(buffer.len, std.math.maxInt(windows.DWORD)));
    if (!ReadFile(stream, buffer.ptr, length, &count, null).toBool()) return 0;
    return count;
}

/// Whether the process has exited, waiting at most `milliseconds`.
pub fn waitExit(process: Handle, milliseconds: u32) bool {
    if (comptime !is_windows) return true;
    return WaitForSingleObject(process, milliseconds) == 0;
}

/// The exit code of a process that has exited.
pub fn exitCode(process: Handle) ?u32 {
    if (comptime !is_windows) return null;
    var code: windows.DWORD = 0;
    if (!GetExitCodeProcess(process, &code).toBool()) return null;
    return code;
}

/// Ends every member of the Job with exit code 1, as the Worker's own owner ends a command's tree.
pub fn endMembers(job: Handle) bool {
    if (comptime !is_windows) return true;
    return ob_windows_terminal_job_terminate(job, 1).toBool();
}

/// How many processes the Job still holds, or null when it cannot be read.
pub fn activeMembers(job: Handle) ?u32 {
    if (comptime !is_windows) return 0;
    var active: windows.DWORD = 0;
    if (!ob_windows_terminal_job_active_processes(job, &active).toBool()) return null;
    return active;
}

pub fn close(handle: Handle) void {
    if (comptime is_windows) windows.CloseHandle(handle);
}

fn utf16(arena: std.mem.Allocator, text: []const u8) SpawnError![:0]u16 {
    if (std.mem.indexOfScalar(u8, text, 0) != null) return error.InvalidCommand;
    return std.unicode.utf8ToUtf16LeAllocZ(arena, text) catch |err| switch (err) {
        error.OutOfMemory => error.OutOfMemory,
        else => error.InvalidCommand,
    };
}

fn systemCommandProcessor(arena: std.mem.Allocator) SpawnError![]const u8 {
    var buffer: [32768]u16 = undefined;
    const length = GetSystemDirectoryW(&buffer, buffer.len);
    if (length == 0 or length >= buffer.len) return error.SpawnFailed;
    const system = std.unicode.utf16LeToUtf8Alloc(arena, buffer[0..length]) catch |err| switch (err) {
        error.OutOfMemory => return error.OutOfMemory,
        else => return error.SpawnFailed,
    };
    return std.mem.concat(arena, u8, &.{ system, "\\cmd.exe" }) catch error.OutOfMemory;
}
