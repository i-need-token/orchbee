const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const monotonic = platform.monotonic;
const subject = platform.process;
const query = subject.testing.posix_process_query;
const allocator = std.testing.allocator;

const unix_epoch_filetime: u64 = 116_444_736_000_000_000;

/// Splits a command line the way a POSIX shell splits words made only of bare,
/// single-quoted, and double-quoted text without expansions.
fn splitWords(alloc: std.mem.Allocator, line: []const u8) !std.ArrayList([]u8) {
    var words: std.ArrayList([]u8) = .empty;
    errdefer {
        for (words.items) |word| alloc.free(word);
        words.deinit(alloc);
    }
    var index: usize = 0;
    while (index < line.len) {
        if (line[index] == ' ') {
            index += 1;
            continue;
        }
        var word: std.ArrayList(u8) = .empty;
        errdefer word.deinit(alloc);
        while (index < line.len and line[index] != ' ') {
            const quote = line[index];
            if (quote == '\'' or quote == '"') {
                const end = std.mem.indexOfScalarPos(u8, line, index + 1, quote) orelse return error.UnterminatedQuote;
                try word.appendSlice(alloc, line[index + 1 .. end]);
                index = end + 1;
            } else {
                try word.append(alloc, quote);
                index += 1;
            }
        }
        try words.append(alloc, try word.toOwnedSlice(alloc));
    }
    return words;
}

fn expectRoundTrip(arguments: []const []const u8) !void {
    var packed_arguments: std.ArrayList(u8) = .empty;
    defer packed_arguments.deinit(allocator);
    for (arguments) |argument| {
        try packed_arguments.appendSlice(allocator, argument);
        try packed_arguments.append(allocator, 0);
    }
    const line = (try query.commandLineFn(allocator, packed_arguments.items)).?;
    defer allocator.free(line);
    var words = try splitWords(allocator, line);
    defer {
        for (words.items) |word| allocator.free(word);
        words.deinit(allocator);
    }
    try std.testing.expectEqual(arguments.len, words.items.len);
    for (arguments, words.items) |expected, actual| try std.testing.expectEqualStrings(expected, actual);
}

test "POSIX command lines split back into the exact argv and stay within the text bound" {
    try expectRoundTrip(&.{ "sleep", "300" });
    try expectRoundTrip(&.{ "bash", "-c", "sleep 300; echo \"done\" $HOME" });
    try expectRoundTrip(&.{ "printf", "it's", "", "'", "a\tb\nc", "中文 😀", "--x=1,2:/p@q%r+s" });

    // A title rewrite pads the argv area with NULs; the padding is not an argument.
    const padded = (try query.commandLineFn(allocator, "node\x00server.js\x00\x00\x00\x00")).?;
    defer allocator.free(padded);
    var words = try splitWords(allocator, padded);
    defer {
        for (words.items) |word| allocator.free(word);
        words.deinit(allocator);
    }
    try std.testing.expectEqual(@as(usize, 2), words.items.len);

    try std.testing.expectEqual(@as(?[]const u8, null), try query.commandLineFn(allocator, "bad\xff\x00"));
    const at_bound = [_]u8{'a'} ** 4096;
    const fits = (try query.commandLineFn(allocator, &at_bound)).?;
    defer allocator.free(fits);
    try std.testing.expectEqual(@as(usize, 4096), fits.len);
    const over_bound = [_]u8{'a'} ** 4097;
    try std.testing.expectEqual(@as(?[]const u8, null), try query.commandLineFn(allocator, &over_bound));
    // Quoting counts against the bound: the line, not the raw argv, must fit.
    const quoted_over = [_]u8{' '} ** 4095;
    try std.testing.expectEqual(@as(?[]const u8, null), try query.commandLineFn(allocator, &quoted_over));
}

test "POSIX creation times share the Windows FILETIME epoch and never follow the real start" {
    try std.testing.expectEqual(unix_epoch_filetime, query.filetimeFromUnixMicrosecondsFn(0).?);
    try std.testing.expectEqual(unix_epoch_filetime + 17_000_000_000_000_010, query.filetimeFromUnixMicrosecondsFn(1_700_000_000_000_001).?);

    const boot_ns: u64 = 1_700_000_000_123_456_789;
    var previous: u64 = 0;
    for ([_]u64{ 0, 1, 99, 100, 12_345, 987_654_321 }) |ticks| {
        for ([_]u64{ 100, 250, 1000 }) |per_second| {
            const filetime = query.filetimeFromBootTicksFn(boot_ns, ticks, per_second).?;
            const start_ns = @as(u128, boot_ns) * per_second + @as(u128, ticks) * std.time.ns_per_s;
            const filetime_ns = (@as(u128, filetime - unix_epoch_filetime) * 100) * per_second;
            try std.testing.expect(filetime_ns <= start_ns);
            try std.testing.expect(start_ns - filetime_ns < 100 * @as(u128, per_second) + per_second);
        }
        const at_100 = query.filetimeFromBootTicksFn(boot_ns, ticks, 100).?;
        try std.testing.expect(at_100 >= previous);
        previous = at_100;
    }
    try std.testing.expectEqual(@as(?u64, null), query.filetimeFromBootTicksFn(0, 1, 100));
    try std.testing.expectEqual(@as(?u64, null), query.filetimeFromBootTicksFn(boot_ns, 1, 0));
}

fn rawFacts() query.RawFactsType {
    var raw = std.mem.zeroes(query.RawFactsType);
    // Valid as Linux clock ticks since boot and as macOS microseconds since 1970.
    raw.start = 12_345;
    raw.parent_pid = 0;
    raw.terminal_present = 1;
    raw.terminal = 7;
    raw.image_length = 14;
    raw.image[0..14].* = "/usr/bin/sleep".*;
    raw.arguments_length = 10;
    raw.arguments[0..10].* = "sleep\x00300\x00".*;
    raw.directory_length = 4;
    raw.directory[0..4].* = "/tmp".*;
    return raw;
}

const clock: query.ClockType = .{ .boot_ns = 1_700_000_000_000_000_000, .ticks_per_second = 100 };

fn decodeWithAllocationFailures(alloc: std.mem.Allocator) !void {
    const raw = rawFacts();
    const decoded = try query.decodeFn(alloc, 23, &raw, 7, clock);
    defer decoded.deinit(alloc);
    try std.testing.expectEqualStrings("/usr/bin/sleep", decoded.image.?);
    try std.testing.expectEqualStrings("sleep 300", decoded.command_line.?);
    try std.testing.expectEqualStrings("/tmp", decoded.current_directory.?);
    try std.testing.expectEqual(@as(?u32, null), decoded.parent_pid);
    try std.testing.expectEqual(subject.ProcessConsole.attached, decoded.console);
}

test "POSIX process facts decode, classify the controlling terminal, and roll back every allocating prefix" {
    try std.testing.checkAllAllocationFailures(allocator, decodeWithAllocationFailures, .{});
    var raw = rawFacts();
    for ([_]struct { present: u32, terminal: u64, reference: ?u64, expected: subject.ProcessConsole }{
        .{ .present = 1, .terminal = 7, .reference = 7, .expected = .attached },
        .{ .present = 1, .terminal = 7, .reference = 8, .expected = .detached },
        .{ .present = 0, .terminal = 0, .reference = 7, .expected = .none },
        .{ .present = 0, .terminal = 0, .reference = null, .expected = .unknown },
        .{ .present = 1, .terminal = 7, .reference = null, .expected = .unknown },
    }) |case| {
        raw.terminal_present = case.present;
        raw.terminal = case.terminal;
        const decoded = try query.decodeFn(allocator, 23, &raw, case.reference, clock);
        defer decoded.deinit(allocator);
        try std.testing.expectEqual(case.expected, decoded.console);
    }
    // A member that withholds its image is opaque even when other facts were copied.
    raw = rawFacts();
    raw.image_length = query.absent_length;
    const opaque_row = try query.decodeFn(allocator, 23, &raw, 7, clock);
    try std.testing.expectEqual(subject.ProcessConsole.unknown, opaque_row.console);
    try std.testing.expectEqual(@as(?[]const u8, null), opaque_row.command_line);
    try std.testing.expectEqual(@as(?[]const u8, null), opaque_row.current_directory);
    // A length beyond its buffer is a broken observation, never a truncated text.
    raw = rawFacts();
    raw.arguments_length = raw.arguments.len + 1;
    try std.testing.expectError(error.ProcessSampleUnavailable, query.decodeFn(allocator, 23, &raw, 7, clock));
}

// Like the Host, wait for the stopped and killed session to drain before observing the
// leader's exit.
fn endSession(process: *subject.Process) !void {
    try process.forceTerminate();
    for (0..400) |_| {
        if (try process.processTreeContained()) break;
        monotonic.sleep(5 * std.time.ns_per_ms);
    } else return error.TestUnexpectedResult;
    _ = try process.wait();
    try process.releaseProcessIdentity();
}

/// A shell whose terminal output is drained as the Host drains it. Unread output can hold
/// a macOS session leader's exit in the terminal's last close.
const Shell = struct {
    process: subject.Process,
    reader: std.Thread,

    fn start(self: *Shell, script: []const u8) !void {
        self.process = try subject.Process.spawn(allocator, .{
            .arguments = &.{ "-c", script },
            .columns = 80,
            .current_directory = null,
            .environment = &.{},
            .executable = "/bin/sh",
            .rows = 24,
        });
        errdefer {
            endSession(&self.process) catch {};
            self.process.deinit();
        }
        self.reader = try std.Thread.spawn(.{}, drain, .{&self.process});
        self.process.registerReaderThread(self.reader.getHandle());
    }

    fn drain(process: *subject.Process) void {
        var buffer: [1024]u8 = undefined;
        while (true) {
            const length = process.read(&buffer) catch return;
            if (length == 0) return;
        }
    }

    fn stop(self: *Shell) void {
        if (!self.process.identityReleased()) endSession(&self.process) catch {};
        self.process.abortRead() catch {};
        self.reader.join();
        self.process.deinit();
    }
};

const Member = std.meta.Elem(@FieldType(subject.ProcessSample, "processes"));

fn row(sample: *const subject.ProcessSample, command: []const u8) ?Member {
    for (sample.processes) |process| {
        if (process.command_line) |line| if (std.mem.eql(u8, line, command)) return process;
    }
    return null;
}

fn awaitRows(process: *subject.Process, minimum: usize) !subject.ProcessSample {
    for (0..400) |_| {
        var sample = try subject.sampleProcesses(process, allocator);
        if (sample.processes.len >= minimum) return sample;
        sample.deinit();
        monotonic.sleep(5 * std.time.ns_per_ms);
    }
    return error.TestUnexpectedResult;
}

fn contains(sample: *const subject.ProcessSample, pid: u32) bool {
    for (sample.processes) |process| if (process.pid == pid) return true;
    return false;
}

test "POSIX process samples list the live session, exclude other sessions, and end with the identity" {
    if (builtin.os.tag != .linux and builtin.os.tag != .macos) return error.SkipZigTest;
    var shell: Shell = undefined;
    try shell.start("sleep 31 & /bin/sh -c 'sleep 32; :' & wait");
    defer shell.stop();
    const process = &shell.process;
    var other_shell: Shell = undefined;
    try other_shell.start("sleep 33; :");
    defer other_shell.stop();
    const outsider = &other_shell.process;

    var sample = try awaitRows(process, 4);
    defer sample.deinit();
    const root_pid: u32 = @intCast(process.child_id);
    try std.testing.expectEqual(root_pid, sample.root_pid);
    try std.testing.expect(!contains(&sample, @intCast(outsider.child_id)));
    var previous: u32 = 0;
    for (sample.processes) |member| {
        try std.testing.expect(member.pid > previous);
        previous = member.pid;
        try std.testing.expectEqual(subject.ProcessConsole.attached, member.console);
        try std.testing.expect(member.image != null and member.current_directory != null);
        // A parent never starts after its child: the Server's descent rule.
        for (sample.processes) |parent| if (member.parent_pid == parent.pid) {
            try std.testing.expect(parent.creation_time <= member.creation_time);
        };
    }
    const nested = row(&sample, "/bin/sh -c 'sleep 32; :'") orelse return error.TestUnexpectedResult;
    const background = row(&sample, "sleep 31") orelse return error.TestUnexpectedResult;
    const grandchild = row(&sample, "sleep 32") orelse return error.TestUnexpectedResult;
    try std.testing.expectEqual(@as(?u32, root_pid), nested.parent_pid);
    try std.testing.expectEqual(@as(?u32, root_pid), background.parent_pid);
    try std.testing.expectEqual(@as(?u32, nested.pid), grandchild.parent_pid);
    try std.testing.expect(std.mem.endsWith(u8, background.image.?, "/sleep"));

    // PID plus creation time names one process across samples.
    var again = try subject.sampleProcesses(process, allocator);
    defer again.deinit();
    for (again.processes) |member| if (member.pid == background.pid) {
        try std.testing.expectEqual(background.creation_time, member.creation_time);
    };

    var other = try awaitRows(outsider, 2);
    defer other.deinit();
    for (sample.processes) |member| try std.testing.expect(!contains(&other, member.pid));

    // A member that exits is omitted at once, before its parent reaps it.
    try std.posix.kill(@intCast(background.pid), std.posix.SIG.KILL);
    var gone = false;
    for (0..400) |_| {
        var later = try subject.sampleProcesses(process, allocator);
        defer later.deinit();
        if (!contains(&later, background.pid)) {
            gone = true;
            break;
        }
        monotonic.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(gone);

    try endSession(process);
    var released = try subject.sampleProcesses(process, allocator);
    defer released.deinit();
    try std.testing.expectEqual(root_pid, released.root_pid);
    try std.testing.expectEqual(@as(usize, 0), released.processes.len);
}
