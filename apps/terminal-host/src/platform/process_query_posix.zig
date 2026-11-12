const std = @import("std");
const builtin = @import("builtin");
const monotonic = @import("monotonic.zig");
const sample = @import("process_sample.zig");

const path_bytes = 4096;
// Argument text longer than three UTF-8 bytes per UTF-16 unit cannot fit the text bound.
const argument_bytes = 3 * sample.maximum_text_code_units;
const absent = std.math.maxInt(u32);
const facts_gone = 0;
const facts_read = 1;

const RawFacts = extern struct {
    start: u64,
    terminal: u64,
    parent_pid: u32,
    terminal_present: u32,
    image_length: u32,
    arguments_length: u32,
    directory_length: u32,
    reserved: u32,
    image: [path_bytes]u8,
    arguments: [argument_bytes]u8,
    directory: [path_bytes]u8,
};
comptime {
    std.debug.assert(@offsetOf(RawFacts, "image") == 40);
    std.debug.assert(@offsetOf(RawFacts, "arguments") == 4136);
    std.debug.assert(@offsetOf(RawFacts, "directory") == 16424);
    std.debug.assert(@sizeOf(RawFacts) == 20520);
    std.debug.assert(@alignOf(RawFacts) == 8);
}

extern fn ob_posix_session_members(session: c_int, pids: [*]i32, count: *u32) c_int;
extern fn ob_posix_process_facts(session: c_int, pid: c_int, facts: *RawFacts, scratch: [*]u8, scratch_bytes: usize) c_int;
extern fn ob_posix_argument_scratch_bytes() usize;
extern fn ob_posix_boot_time_ns(nanoseconds: *i64) c_int;
extern fn ob_posix_clock_ticks_per_second() c_long;

/// FILETIME of the Unix epoch: the wire's creation time is 100 ns units since 1601.
const unix_epoch_filetime: u64 = 116_444_736_000_000_000;

/// FILETIME of a process the kernel started `ticks` clock ticks after a boot at
/// `boot_ns` Unix nanoseconds. Rounding down keeps the time no later than the start,
/// so a later creation time still proves a later start.
fn filetimeFromBootTicks(boot_ns: u64, ticks: u64, ticks_per_second: u64) ?u64 {
    if (boot_ns == 0 or ticks_per_second == 0) return null;
    const since_boot = @as(u128, ticks) * std.time.ns_per_s / ticks_per_second;
    const nanoseconds = @as(u128, boot_ns) + since_boot;
    return std.math.cast(u64, nanoseconds / 100 + unix_epoch_filetime);
}

fn filetimeFromUnixMicroseconds(microseconds: u64) ?u64 {
    const intervals = std.math.mul(u64, microseconds, 10) catch return null;
    return std.math.add(u64, intervals, unix_epoch_filetime) catch null;
}

/// Linux boot time on the wall clock, kept for the Host's life and shared by every
/// terminal's samples. Each reading falls short of the true boot time by the gap between
/// its two clock reads, and a creation time that moved with that gap would break
/// PID-plus-creation identity between samples. The kept value sits one tolerance below the
/// reading that set it, so later readings' gaps leave it in place and it stays below the
/// true boot time: creation times never follow the real start. A reading below it, or more
/// than two tolerances above it, means the wall clock stepped and replaces it. Zero is unread.
var linux_boot_ns: std.atomic.Value(u64) = .init(0);
const boot_tolerance_ns = std.time.ns_per_ms;

fn linuxBootNanoseconds() sample.Error!u64 {
    var reading: i64 = 0;
    if (ob_posix_boot_time_ns(&reading) != 0 or reading <= boot_tolerance_ns) return error.ProcessSampleUnavailable;
    const fresh: u64 = @intCast(reading);
    var kept = linux_boot_ns.load(.acquire);
    while (true) {
        if (kept != 0 and kept <= fresh and fresh - kept <= 2 * boot_tolerance_ns) return kept;
        const replacement = fresh - boot_tolerance_ns;
        kept = linux_boot_ns.cmpxchgWeak(kept, replacement, .acq_rel, .acquire) orelse return replacement;
    }
}

const Clock = struct {
    boot_ns: u64 = 0,
    ticks_per_second: u64 = 0,

    fn read() sample.Error!Clock {
        if (builtin.os.tag != .linux) return .{};
        const ticks = ob_posix_clock_ticks_per_second();
        if (ticks <= 0) return error.ProcessSampleUnavailable;
        return .{ .boot_ns = try linuxBootNanoseconds(), .ticks_per_second = @intCast(ticks) };
    }

    fn creationTime(self: Clock, start: u64) ?u64 {
        return if (builtin.os.tag == .linux)
            filetimeFromBootTicks(self.boot_ns, start, self.ticks_per_second)
        else
            filetimeFromUnixMicroseconds(start);
    }
};

/// Quotes one argument as Python's `shlex.quote` does: bare when every byte is in its
/// safe set, otherwise single-quoted with each `'` written as `'"'"'`, so a POSIX word
/// splitter recovers the exact argument.
fn appendQuoted(allocator: std.mem.Allocator, out: *std.ArrayList(u8), argument: []const u8) !void {
    const safe = argument.len > 0 and for (argument) |byte| switch (byte) {
        'A'...'Z', 'a'...'z', '0'...'9', '_', '@', '%', '+', '=', ':', ',', '.', '/', '-' => {},
        else => break false,
    } else true;
    if (safe) return out.appendSlice(allocator, argument);
    try out.append(allocator, '\'');
    var rest = argument;
    while (std.mem.indexOfScalar(u8, rest, '\'')) |index| {
        try out.appendSlice(allocator, rest[0..index]);
        try out.appendSlice(allocator, "'\"'\"'");
        rest = rest[index + 1 ..];
    }
    try out.appendSlice(allocator, rest);
    try out.append(allocator, '\'');
}

/// The wire carries one command-line string. POSIX argv is NUL-separated bytes: each
/// argument is quoted for a POSIX shell and the arguments are joined by one space.
/// Trailing NULs are dropped because a program that rewrites its title pads the argv
/// area with them; a trailing empty argument is lost with them. Text that is not UTF-8
/// or exceeds the UTF-16 bound is unavailable, never truncated.
fn commandLine(allocator: std.mem.Allocator, arguments: []const u8) error{OutOfMemory}!?[]const u8 {
    const used = std.mem.trimEnd(u8, arguments, "\x00");
    if (!std.unicode.utf8ValidateSlice(used)) return null;
    var out: std.ArrayList(u8) = .empty;
    errdefer out.deinit(allocator);
    if (used.len > 0) {
        var iterator = std.mem.splitScalar(u8, used, 0);
        try appendQuoted(allocator, &out, iterator.first());
        while (iterator.next()) |argument| {
            try out.append(allocator, ' ');
            try appendQuoted(allocator, &out, argument);
        }
    }
    const units = std.unicode.calcUtf16LeLen(out.items) catch unreachable;
    if (units > sample.maximum_text_code_units) {
        out.deinit(allocator);
        return null;
    }
    return try out.toOwnedSlice(allocator);
}

const deleted_suffix = " (deleted)";

/// Linux marks an unlinked executable or directory by appending ` (deleted)`. The image
/// keeps its path, the program it was started from, so an Agent that replaced its own
/// binary is still recognized; a deleted directory no longer names one and is absent.
fn pathText(allocator: std.mem.Allocator, bytes: []const u8, keep_deleted: bool) error{OutOfMemory}!?[]const u8 {
    var path = bytes;
    if (builtin.os.tag == .linux and std.mem.endsWith(u8, path, deleted_suffix)) {
        if (!keep_deleted) return null;
        path = path[0 .. path.len - deleted_suffix.len];
    }
    if (path.len == 0 or std.mem.indexOfScalar(u8, path, 0) != null) return null;
    if (!std.unicode.utf8ValidateSlice(path)) return null;
    const units = std.unicode.calcUtf16LeLen(path) catch unreachable;
    if (units > sample.maximum_text_code_units) return null;
    return try allocator.dupe(u8, path);
}

fn rawText(buffer: []const u8, length: u32) sample.Error!?[]const u8 {
    if (length == absent) return null;
    if (length > buffer.len) return error.ProcessSampleUnavailable;
    return buffer[0..length];
}

/// `reference` is the live root's controlling terminal, or null when it is unknown.
/// Every session member shares the root's terminal until the root, the session leader,
/// exits; the kernel then detaches the terminal from the whole session, so a member's
/// missing terminal no longer tells how it was started and its console is unknown.
fn console(raw: *const RawFacts, reference: ?u64) sample.Console {
    const root = reference orelse return .unknown;
    if (raw.terminal_present == 0) return .none;
    return if (raw.terminal == root) .attached else .detached;
}

fn decode(allocator: std.mem.Allocator, pid: u32, raw: *const RawFacts, reference: ?u64, clock: Clock) sample.Error!sample.Process {
    const creation_time = clock.creationTime(raw.start) orelse return error.ProcessSampleUnavailable;
    if (creation_time == 0) return error.ProcessSampleUnavailable;
    const parent_pid: ?u32 = if (raw.parent_pid == 0) null else raw.parent_pid;
    // A member that withholds its image is opaque: identity and parent only.
    const image_bytes = try rawText(&raw.image, raw.image_length) orelse return .{
        .pid = pid,
        .parent_pid = parent_pid,
        .creation_time = creation_time,
        .console = .unknown,
        .image = null,
        .command_line = null,
        .current_directory = null,
    };
    const image = try pathText(allocator, image_bytes, true);
    errdefer if (image) |value| allocator.free(value);
    const command = if (try rawText(&raw.arguments, raw.arguments_length)) |bytes|
        try commandLine(allocator, bytes)
    else
        null;
    errdefer if (command) |value| allocator.free(value);
    const directory = if (try rawText(&raw.directory, raw.directory_length)) |bytes|
        try pathText(allocator, bytes, false)
    else
        null;
    return .{
        .pid = pid,
        .parent_pid = parent_pid,
        .creation_time = creation_time,
        .console = console(raw, reference),
        .image = image,
        .command_line = command,
        .current_directory = directory,
    };
}

fn checkBudget(timer: *const monotonic.Timer) !void {
    if (timer.read() >= 250 * std.time.ns_per_ms) return error.ProcessSampleUnavailable;
}

fn lessPid(_: void, left: sample.Process, right: sample.Process) bool {
    return left.pid < right.pid;
}

/// Samples the members of the session the root leads, the containment domain the Host
/// signals and drains. The caller holds the session identity fence, so the root is not
/// reaped and its session key cannot name another session during the scan. Each row is a
/// member that was live when read; one that exits or leaves meanwhile is omitted, and
/// one whose identity cannot be read fails the sample rather than being hidden.
pub fn collect(allocator: std.mem.Allocator, session: std.posix.pid_t) sample.Error!sample.Sample {
    const timer = monotonic.Timer.start() catch return error.ProcessSampleUnavailable;
    const root_pid = std.math.cast(u32, session) orelse return error.ProcessSampleUnavailable;
    if (root_pid == 0) return error.ProcessSampleUnavailable;
    const clock = try Clock.read();
    var buffer: [sample.maximum_processes]i32 = undefined;
    var count: u32 = 0;
    switch (ob_posix_session_members(session, &buffer, &count)) {
        1 => {},
        2 => return error.ProcessSampleLimit,
        else => return error.ProcessSampleUnavailable,
    }
    if (count > buffer.len) return error.ProcessSampleLimit;
    const candidates = buffer[0..count];
    std.mem.sort(i32, candidates, {}, std.sort.asc(i32));
    for (candidates, 0..) |pid, index| {
        if (pid <= 0 or (index > 0 and candidates[index - 1] == pid)) return error.ProcessSampleUnavailable;
    }
    // The root is read first: while it lives, its terminal is the session's terminal.
    if (std.mem.indexOfScalar(i32, candidates, session)) |index| {
        std.mem.swap(i32, &candidates[0], &candidates[index]);
    }
    const scratch = try allocator.alloc(u8, ob_posix_argument_scratch_bytes());
    defer allocator.free(scratch);
    const rows = try allocator.alloc(sample.Process, candidates.len);
    var initialized: usize = 0;
    errdefer {
        for (rows[0..initialized]) |row| row.deinit(allocator);
        allocator.free(rows);
    }
    var reference: ?u64 = null;
    var raw: RawFacts = undefined;
    for (candidates) |pid| {
        try checkBudget(&timer);
        switch (ob_posix_process_facts(session, pid, &raw, scratch.ptr, scratch.len)) {
            facts_read => {},
            facts_gone => continue,
            else => return error.ProcessSampleUnavailable,
        }
        if (pid == session and raw.terminal_present != 0) reference = raw.terminal;
        rows[initialized] = try decode(allocator, @intCast(pid), &raw, reference, clock);
        initialized += 1;
    }
    try checkBudget(&timer);
    std.mem.sort(sample.Process, rows[0..initialized], {}, lessPid);
    const processes = try allocator.realloc(rows, initialized);
    return .{ .allocator = allocator, .root_pid = root_pid, .processes = processes };
}

pub const testing = if (builtin.is_test) struct {
    pub const RawFactsType = RawFacts;
    pub const ClockType = Clock;
    pub const decodeFn = decode;
    pub const absent_length = absent;
    pub const commandLineFn = commandLine;
    pub const filetimeFromBootTicksFn = filetimeFromBootTicks;
    pub const filetimeFromUnixMicrosecondsFn = filetimeFromUnixMicroseconds;
} else struct {};
