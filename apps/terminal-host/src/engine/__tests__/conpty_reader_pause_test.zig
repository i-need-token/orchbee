//! The reader's undoable stop through a real PTY. On Windows the engine suite runs this on system
//! ConHost and the bundled ConPTY suite on the pinned bundle. A program stopping inside its own
//! sequence is witnessed here on POSIX, and on the pinned bundle by the black-box moves between
//! Host processes, where the console passes the partial sequence through to the reader.
const harness = @import("runtime_harness.zig");
const screen_text = @import("screen_text.zig");
const std = harness.std;
const builtin = harness.target_builtin;
const monotonic = harness.platform_boundary.monotonic;
const pty_process = harness.pty_process;
const thread_sync = harness.thread_sync;
const TerminalRuntime = harness.TerminalRuntime;
const ReaderParkWait = harness.subject.ReaderParkWait;

const allocator = std.testing.allocator;
const pause_cycles = 1000;
/// Most cycles must meet the program writing, or the probe would test an idle reader.
const minimum_cycles_with_output = 100;
/// Lets the program write between cycles; Windows rounds it up to its timer tick.
const pause_gap_ns = 5 * std.time.ns_per_ms;
const flood_lines = 20_000;
const flood_pauses = 20;
const flood_pause_gap_ns = 100 * std.time.ns_per_ms;
/// The quiesce's own bound on reaching a park.
const park_timeout_ns = std.time.ns_per_s;
const output_timeout_ns = 60 * std.time.ns_per_s;
const capture_limit_bytes = 512 * 1024;

/// Records every byte the platform read returned, in order, so loss and repeats are seen where
/// the pause acts.
const RecordedOutput = struct {
    ready: harness.TestGate = .{},
    runtime: *TerminalRuntime = undefined,
    mutex: thread_sync.Mutex = .{},
    bytes: std.ArrayList(u8) = .empty,
    failure: ?anyerror = null,

    fn read(context: *anyopaque, buffer: []u8, mode: pty_process.ReadMode) anyerror!pty_process.Read {
        const self: *RecordedOutput = @ptrCast(@alignCast(context));
        self.ready.pause();
        const result = try pty_process.readPausable(&harness.runtimeState(self.runtime).process, buffer, mode);
        switch (result) {
            .bytes => |count| {
                self.mutex.lock();
                defer self.mutex.unlock();
                self.bytes.appendSlice(allocator, buffer[0..count]) catch |err| {
                    self.failure = err;
                };
            },
            .end, .paused => {},
        }
        return result;
    }

    fn length(self: *RecordedOutput) usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.bytes.items.len;
    }

    fn contains(self: *RecordedOutput, needle: []const u8) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return std.mem.indexOf(u8, self.bytes.items, needle) != null;
    }

    fn awaitText(self: *RecordedOutput, needle: []const u8) !void {
        var timer = try monotonic.Timer.start();
        while (!self.contains(needle)) {
            if (timer.read() >= output_timeout_ns) return error.ProbeOutputTimedOut;
            monotonic.sleep(10 * std.time.ns_per_ms);
        }
    }

    /// The numbered lines recorded so far, checked once each and in order from 1.
    fn numberedLines(self: *RecordedOutput, marker: []const u8) !usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        try std.testing.expect(self.failure == null);
        return countNumberedLines(self.bytes.items, marker);
    }

    fn awaitNumberedLines(self: *RecordedOutput, marker: []const u8, count: usize) !void {
        var timer = try monotonic.Timer.start();
        while (try self.numberedLines(marker) != count) {
            if (timer.read() >= output_timeout_ns) return error.ProbeOutputTimedOut;
            monotonic.sleep(10 * std.time.ns_per_ms);
        }
    }

    fn deinit(self: *RecordedOutput) void {
        self.bytes.deinit(allocator);
    }
};

/// Starts the probe with its reads recorded; the first read waits until the runtime is known.
fn startProbe(output: *RecordedOutput, executable: []const u8, arguments: []const []const u8) !*TerminalRuntime {
    const runtime = try harness.createRuntimeWithHooksAndCommand(
        .{ .context = output, .read_pty_pausable = RecordedOutput.read },
        executable,
        arguments,
    );
    output.runtime = runtime;
    output.ready.release();
    return runtime;
}

/// Runs a cmd command line on Windows or a shell command elsewhere.
fn startCommand(output: *RecordedOutput, windows_command: []const u8, posix_command: []const u8) !*TerminalRuntime {
    if (builtin.os.tag != .windows) return startProbe(output, "/bin/sh", &.{ "-c", posix_command });
    const executable = try pty_process.environmentVariableOwned(allocator, "ComSpec");
    defer allocator.free(executable);
    return startProbe(output, executable, &.{ "/D", "/S", "/C", windows_command });
}

/// Counts the lines numbered after `marker` in the stream without its escape sequences, failing
/// unless they run from 1 once each and in order. A number the stream has not finished is left
/// out.
fn countNumberedLines(stream: []const u8, marker: []const u8) !usize {
    var text: std.ArrayList(u8) = .empty;
    defer text.deinit(allocator);
    try text.ensureTotalCapacity(allocator, stream.len);
    var index: usize = 0;
    while (index < stream.len) {
        if (stream[index] != 0x1b) {
            text.appendAssumeCapacity(stream[index]);
            index += 1;
            continue;
        }
        index += 1;
        if (index >= stream.len) break;
        switch (stream[index]) {
            '[' => {
                index += 1;
                while (index < stream.len and (stream[index] < 0x40 or stream[index] > 0x7e)) index += 1;
                index += 1;
            },
            ']' => {
                while (index < stream.len and stream[index] != 0x07 and
                    !(stream[index] == 0x1b and index + 1 < stream.len and stream[index + 1] == '\\')) index += 1;
                index += if (index < stream.len and stream[index] == 0x1b) 2 else 1;
            },
            else => index += 1,
        }
    }
    var counted: usize = 0;
    var rest = text.items;
    while (std.mem.indexOf(u8, rest, marker)) |position| {
        rest = rest[position + marker.len ..];
        var digits: usize = 0;
        while (digits < rest.len and std.ascii.isDigit(rest[digits])) digits += 1;
        if (digits == 0 or digits == rest.len) continue;
        const number = try std.fmt.parseInt(usize, rest[0..digits], 10);
        if (number != counted + 1) {
            std.debug.print("{s}{d} where {s}{d} was expected\n", .{ marker, number, marker, counted + 1 });
            return error.NumberedLineLostOrRepeated;
        }
        counted = number;
        rest = rest[digits..];
    }
    return counted;
}

test "reader pause and resume 1,000 times loses and repeats no numbered line" {
    var output: RecordedOutput = .{};
    defer output.deinit();
    // A line about every 10 ms, so pauses meet blocked reads, reads in progress and reads that
    // complete as they are cancelled. Each ping takes a process start.
    const runtime = try startCommand(
        &output,
        "for /L %i in (1,1,1000000) do @(echo line-%i & ping -n 1 127.0.0.1 >nul)",
        "i=1; while :; do echo line-$i; i=$((i+1)); sleep 0.01; done",
    );
    defer harness.destroyRuntimeWithIdentityRetry(runtime);

    try output.awaitText("line-1");
    var cycles_with_output: usize = 0;
    for (0..pause_cycles) |_| {
        const before = output.length();
        try runtime.pauseReader();
        try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
        try runtime.resumeReader();
        monotonic.sleep(pause_gap_ns);
        if (output.length() > before) cycles_with_output += 1;
    }
    try std.testing.expect(cycles_with_output >= minimum_cycles_with_output);
    try std.testing.expect(try output.numberedLines("line-") >= minimum_cycles_with_output);
}

test "reader pause during a flood parks within the quiesce bound and loses nothing" {
    var output: RecordedOutput = .{};
    defer output.deinit();
    const count = std.fmt.comptimePrint("{d}", .{flood_lines});
    const runtime = try startCommand(
        &output,
        "(for /L %i in (1,1," ++ count ++ ") do @echo flood-%i) & echo flood-end & ping -n 601 127.0.0.1 >nul",
        "seq -f 'flood-%.0f' 1 " ++ count ++ "; echo flood-end; exec sleep 600",
    );
    defer harness.destroyRuntimeWithIdentityRetry(runtime);

    try output.awaitText("flood-1");
    var pauses_with_output: usize = 0;
    for (0..flood_pauses) |_| {
        const before = output.length();
        try runtime.pauseReader();
        try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
        try runtime.resumeReader();
        monotonic.sleep(flood_pause_gap_ns);
        if (output.length() > before) pauses_with_output += 1;
    }
    try std.testing.expect(pauses_with_output > 0);
    // A number counts once the line end after it is read; a title names none.
    try output.awaitNumberedLines("flood-", flood_lines);
}

test "a pause while the program stops inside a sequence never parks, and withdrawing it loses nothing" {
    // The program is a POSIX shell; `handover_black_box_test.zig` witnesses one on the pinned bundle.
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var output: RecordedOutput = .{};
    defer output.deinit();
    const runtime = try startProbe(&output, "/bin/sh", &.{
        "-c",
        "stty -echo; printf 'stop\\033[3'; read reply; printf '1mmid\\033[0m\\n'; exec sleep 600",
    });
    defer harness.destroyRuntimeWithIdentityRetry(runtime);

    try output.awaitText("stop\x1b[3");
    try runtime.pauseReader();
    try std.testing.expectEqual(ReaderParkWait.mid_sequence, runtime.awaitReaderParked(park_timeout_ns));
    try runtime.resumeReader();
    _ = try runtime.enqueueRawInput("go\n");
    try output.awaitText("1mmid");

    var timer = try monotonic.Timer.start();
    while (true) {
        var capture = try screen_text.read(runtime, allocator, capture_limit_bytes);
        defer capture.deinit();
        if (std.mem.indexOf(u8, capture.text(), "stopmid") != null) {
            try std.testing.expect(std.mem.indexOf(u8, capture.text(), "[3") == null);
            break;
        }
        if (timer.read() >= output_timeout_ns) return error.ProbeOutputTimedOut;
        monotonic.sleep(10 * std.time.ns_per_ms);
    }
}
