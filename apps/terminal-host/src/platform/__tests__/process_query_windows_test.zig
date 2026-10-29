const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const subject = platform.process;
const query = subject.testing.process_query;
const allocator = std.testing.allocator;
const windows = std.os.windows;

fn rawFacts() query.RawFactsType {
    var raw = std.mem.zeroes(query.RawFactsType);
    raw.creation_time = 19;
    raw.console_host = 7;
    raw.image_length = 3;
    raw.image[0..3].* = .{ 'x', '.', 'e' };
    raw.command_length = 4;
    raw.command[0..4].* = .{ 0x4e2d, ' ', 0xd83d, 0xde00 };
    raw.directory_length = std.math.maxInt(u32);
    return raw;
}

fn decodeWithAllocationFailures(alloc: std.mem.Allocator) !void {
    const raw = rawFacts();
    const decoded = try query.decodeFacts(alloc, 23, &raw, 7);
    defer decoded.deinit(alloc);
    try std.testing.expectEqualStrings("中 😀", decoded.command_line.?);
    try std.testing.expectEqualStrings("x.e", decoded.image.?);
    try std.testing.expectEqual(@as(?u32, null), decoded.parent_pid);
    try std.testing.expectEqual(@as(?[]const u8, null), decoded.current_directory);
}

test "process facts strictly decode Unicode, classify the console, and roll back every allocating prefix" {
    try std.testing.checkAllAllocationFailures(allocator, decodeWithAllocationFailures, .{});
    var raw = rawFacts();
    for ([_]struct { host: u32, reference: ?u32, expected: subject.ProcessConsole }{
        .{ .host = 7, .reference = 7, .expected = .attached },
        .{ .host = 7, .reference = 8, .expected = .detached },
        .{ .host = 0, .reference = 7, .expected = .none },
        .{ .host = 0, .reference = null, .expected = .none },
        .{ .host = 7, .reference = null, .expected = .unknown },
        .{ .host = std.math.maxInt(u32), .reference = 7, .expected = .unknown },
    }) |case| {
        raw.console_host = case.host;
        const decoded = try query.decodeFacts(allocator, 23, &raw, case.reference);
        defer decoded.deinit(allocator);
        try std.testing.expectEqual(case.expected, decoded.console);
    }
    raw = rawFacts();
    raw.command[3] = 'x'; // Unpaired surrogate is absent, never replacement text.
    const invalid = try query.decodeFacts(allocator, 23, &raw, 7);
    defer invalid.deinit(allocator);
    try std.testing.expectEqual(@as(?[]const u8, null), invalid.command_line);
    raw.command_length = 4097;
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 23, &raw, 7));
    raw = rawFacts();
    raw.command[0] = 0;
    const nul = try query.decodeFacts(allocator, 23, &raw, 7);
    defer nul.deinit(allocator);
    try std.testing.expectEqual(@as(?[]const u8, null), nul.command_line);
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 0, &raw, 7));
    raw.creation_time = 0;
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFacts(allocator, 23, &raw, 7));
}

extern "kernel32" fn GetCurrentProcessId() callconv(.winapi) u32;
extern "kernel32" fn GetProcessId(process: windows.HANDLE) callconv(.winapi) u32;
extern "kernel32" fn AssignProcessToJobObject(job: windows.HANDLE, process: windows.HANDLE) callconv(.winapi) windows.BOOL;
extern "kernel32" fn ResumeThread(thread: windows.HANDLE) callconv(.winapi) windows.DWORD;
extern "kernel32" fn TerminateProcess(process: windows.HANDLE, exit_code: windows.UINT) callconv(.winapi) windows.BOOL;
extern "kernel32" fn LocalFree(memory: ?*anyopaque) callconv(.winapi) ?*anyopaque;
extern "kernel32" fn CreateProcessW(
    application_name: ?windows.LPWSTR,
    command_line: ?windows.LPWSTR,
    process_attributes: ?*windows.SECURITY_ATTRIBUTES,
    thread_attributes: ?*windows.SECURITY_ATTRIBUTES,
    inherit_handles: windows.BOOL,
    creation_flags: windows.DWORD,
    environment: ?*const anyopaque,
    current_directory: ?windows.LPWSTR,
    startup_info: *windows.STARTUPINFOW,
    process_information: *windows.PROCESS.INFORMATION,
) callconv(.winapi) windows.BOOL;
extern "advapi32" fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
    text: [*:0]const u16,
    revision: windows.DWORD,
    descriptor: *?*anyopaque,
    size: ?*windows.DWORD,
) callconv(.winapi) windows.BOOL;

const create_suspended: windows.DWORD = 0x0000_0004;
const create_no_window: windows.DWORD = 0x0800_0000;
const detached_process: windows.DWORD = 0x0000_0008;

/// A controlled Job member started by the test with its own security descriptor.
const Member = struct {
    process: windows.HANDLE,
    thread: windows.HANDLE,
    pid: u32,

    fn spawn(job: windows.HANDLE, command: []const u8, sddl: ?[]const u8, flags: windows.DWORD, run: bool) !Member {
        const command_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, command);
        defer allocator.free(command_w);
        var descriptor: ?*anyopaque = null;
        defer if (descriptor) |value| {
            _ = LocalFree(value);
        };
        var attributes: windows.SECURITY_ATTRIBUTES = .{ .nLength = @sizeOf(windows.SECURITY_ATTRIBUTES), .lpSecurityDescriptor = null, .bInheritHandle = .FALSE };
        if (sddl) |text| {
            const text_w = try std.unicode.utf8ToUtf16LeAllocZ(allocator, text);
            defer allocator.free(text_w);
            if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(text_w.ptr, 1, &descriptor, null).toBool())
                return error.SecurityDescriptorUnavailable;
            attributes.lpSecurityDescriptor = descriptor;
        }
        var startup = std.mem.zeroes(windows.STARTUPINFOW);
        startup.cb = @sizeOf(windows.STARTUPINFOW);
        var information: windows.PROCESS.INFORMATION = undefined;
        if (!CreateProcessW(null, command_w.ptr, if (sddl != null) &attributes else null, null, .FALSE, flags | create_suspended, null, null, &startup, &information).toBool())
            return error.SpawnFailed;
        const member: Member = .{ .process = information.hProcess, .thread = information.hThread, .pid = GetProcessId(information.hProcess) };
        errdefer member.deinit();
        if (!AssignProcessToJobObject(job, member.process).toBool()) return error.JobAssignmentFailed;
        if (run and ResumeThread(member.thread) == std.math.maxInt(windows.DWORD)) return error.ResumeFailed;
        return member;
    }

    fn deinit(self: Member) void {
        _ = TerminateProcess(self.process, 1);
        _ = windows.CloseHandle(self.thread);
        _ = windows.CloseHandle(self.process);
    }
};

fn rowFor(sample: anytype, pid: u32) ?@TypeOf(sample.processes[0]) {
    for (sample.processes) |row| if (row.pid == pid) return row;
    return null;
}

fn spawnShell(directory: []const u8, command: []const u8) !subject.Process {
    const executable = try subject.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(executable);
    return subject.Process.spawn(allocator, .{
        .arguments = &.{ "/D", "/S", "/C", command },
        .columns = 80,
        .rows = 24,
        .current_directory = directory,
        .environment = &.{},
        .executable = executable,
    });
}

fn stopShell(process: *subject.Process) void {
    process.forceTerminate() catch {};
    _ = process.wait() catch {};
    process.finishPtyAfterChildExit() catch {};
}

test "Windows process samples expose owned shell and child facts, deny outsiders, and retain exited root identity" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const directory = try subject.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(directory);
    var process = try spawnShell(directory, "ping -t 127.0.0.1 >nul");
    defer process.deinit();
    var stopped = false;
    defer if (!stopped) stopShell(&process);
    var outsider: ?windows.HANDLE = null;
    try std.testing.expectEqual(@as(c_int, 0), query.openJobProcess(process.job_handle, GetCurrentProcessId(), &outsider));
    try std.testing.expect(outsider == null);

    var root_pid: u32 = 0;
    var observed = false;
    const timer = try platform.monotonic.Timer.start();
    while (timer.read() < 3 * std.time.ns_per_s) {
        var sample = try subject.sampleProcesses(&process, allocator);
        defer sample.deinit();
        root_pid = sample.root_pid;
        var shell = false;
        var child = false;
        for (sample.processes) |row| {
            try std.testing.expect(row.creation_time > 0);
            if (row.pid == root_pid) {
                try std.testing.expect(std.ascii.endsWithIgnoreCase(row.image.?, "cmd.exe"));
                try std.testing.expect(std.mem.indexOf(u8, row.command_line.?, "ping -t") != null);
                try std.testing.expect(row.console == .attached);
                // RTL CurrentDirectory retains a trailing separator.
                try std.testing.expect(std.ascii.eqlIgnoreCase(
                    std.mem.trimEnd(u8, directory, "\\/"),
                    std.mem.trimEnd(u8, row.current_directory.?, "\\/"),
                ));
                shell = true;
            } else if (row.image) |image| {
                if (std.ascii.endsWithIgnoreCase(image, "ping.exe")) {
                    try std.testing.expectEqual(@as(?u32, root_pid), row.parent_pid);
                    // Until its console connection completes, a child reports its creator.
                    child = row.console == .attached;
                }
            }
        }
        observed = shell and child;
        if (observed) break;
        platform.monotonic.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(observed);
    stopShell(&process);
    stopped = true;
    var ended = try subject.sampleProcesses(&process, allocator);
    defer ended.deinit();
    try std.testing.expectEqual(root_pid, ended.root_pid);
    try std.testing.expectEqual(@as(usize, 0), ended.processes.len);
}

test "Windows process samples keep opaque, read-denied, other-console, GUI, console-less, and churning members over 64" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    const directory = try subject.environmentVariableOwned(allocator, "SystemRoot");
    defer allocator.free(directory);
    // The shell churns short-lived console children for the whole test.
    var process = try spawnShell(directory, "for /L %i in (1,1,100000) do @cmd /D /C exit");
    defer process.deinit();
    defer stopShell(&process);
    const job = process.job_handle;
    const self_pid = GetCurrentProcessId();

    // No access at all: an opaque row. Limited rights without memory reads: a readable image
    // and command line without a current directory. CREATE_NO_WINDOW gives it another console.
    const denied = try Member.spawn(job, "ping -n 120 127.0.0.1", "D:", create_no_window, true);
    defer denied.deinit();
    const read_denied = try Member.spawn(job, "ping -n 121 127.0.0.1", "D:(A;;0x101000;;;WD)", create_no_window, true);
    defer read_denied.deinit();
    // A GUI-subsystem image reports its creator rather than this console; it stays suspended,
    // so no window opens. DETACHED_PROCESS leaves a console program without any console.
    const gui = try Member.spawn(job, "rundll32.exe", null, 0, false);
    defer gui.deinit();
    const console_less = try Member.spawn(job, "ping -n 122 127.0.0.1", null, detached_process, true);
    defer console_less.deinit();
    var crowd: [70]Member = undefined;
    var crowd_count: usize = 0;
    defer for (crowd[0..crowd_count]) |member| member.deinit();
    while (crowd_count < crowd.len) : (crowd_count += 1)
        crowd[crowd_count] = try Member.spawn(job, "cmd.exe /D /C exit", null, create_no_window, false);

    var samples: usize = 0;
    var settled = false;
    const timer = try platform.monotonic.Timer.start();
    while (timer.read() < 5 * std.time.ns_per_s and (samples < 40 or !settled)) : (samples += 1) {
        // Membership churn, opaque rows, and more than 64 members never fail the sample.
        var sample = try subject.sampleProcesses(&process, allocator);
        defer sample.deinit();
        try std.testing.expect(sample.processes.len > 64);
        for (sample.processes, 0..) |row, index| {
            try std.testing.expect(row.creation_time > 0);
            if (index > 0) try std.testing.expect(sample.processes[index - 1].pid < row.pid);
        }
        for (crowd) |member| try std.testing.expect(rowFor(sample, member.pid) != null);
        const opaque_row = rowFor(sample, denied.pid).?;
        try std.testing.expectEqual(@as(?[]const u8, null), opaque_row.image);
        try std.testing.expectEqual(@as(?[]const u8, null), opaque_row.command_line);
        try std.testing.expect(opaque_row.console == .unknown);
        try std.testing.expectEqual(@as(?u32, self_pid), opaque_row.parent_pid);
        const gui_row = rowFor(sample, gui.pid).?;
        try std.testing.expect(std.ascii.endsWithIgnoreCase(gui_row.image.?, "rundll32.exe"));
        try std.testing.expect(gui_row.console == .detached);
        try std.testing.expect(rowFor(sample, console_less.pid).?.console == .none);
        try std.testing.expect(rowFor(sample, sample.root_pid).?.console == .attached);
        // The other console is attached once the child finishes its own startup.
        const limited = rowFor(sample, read_denied.pid).?;
        try std.testing.expect(std.mem.indexOf(u8, limited.command_line.?, "-n 121") != null);
        try std.testing.expectEqual(@as(?[]const u8, null), limited.current_directory);
        if (limited.console == .detached) settled = true;
    }
    try std.testing.expect(settled);
    try std.testing.expect(samples >= 40);
}
