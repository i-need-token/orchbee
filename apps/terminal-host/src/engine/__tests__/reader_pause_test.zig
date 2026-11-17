//! The reader's undoable stop against scripted platform reads: each cut of a corpus of
//! sequences meets a known parser state, so a pause parks exactly at ground and reads on inside
//! a sequence. The real-PTY witnesses are in conpty_reader_pause_test.zig.
const harness = @import("runtime_harness.zig");
const screen_text = @import("screen_text.zig");
const std = harness.std;
const pty_process = harness.pty_process;
const thread_sync = harness.thread_sync;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const TerminalRuntime = harness.TerminalRuntime;
const ReaderParkWait = harness.subject.ReaderParkWait;

const park_timeout_ns = 10 * std.time.ns_per_s;
const capture_limit_bytes = 512 * 1024;

/// Stands in for the platform read: it serves the chunks the test pushes and reports a pause as
/// the platform does. A pause stops a read that stops at pauses, and cancels only the read
/// active when it was requested.
const ScriptedOutput = struct {
    condition: thread_sync.Condition = .{},
    mutex: thread_sync.Mutex = .{},
    aborted: bool = false,
    chunk: ?[]const u8 = null,
    committed: usize = 0,
    pause_cancel: bool = false,
    pause_standing: bool = false,
    reading: ?pty_process.ReadMode = null,
    stopped: bool = false,
    /// Runs inside the platform pause instead of standing it, then refuses it.
    refuse_pause: ?*const fn (*ScriptedOutput) void = null,
    refuse_resume: bool = false,
    runtime: *TerminalRuntime = undefined,
    refusal_park: ?ReaderParkWait = null,

    fn fromContext(context: *anyopaque) *ScriptedOutput {
        return @ptrCast(@alignCast(context));
    }

    fn read(context: *anyopaque, buffer: []u8, mode: pty_process.ReadMode) anyerror!pty_process.Read {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.pause_standing and mode == .stop_at_pause) return .paused;
        self.reading = mode;
        self.pause_cancel = false;
        self.condition.broadcast();
        defer {
            self.reading = null;
            self.condition.broadcast();
        }
        while (true) {
            if (self.aborted) return .end;
            if (self.chunk) |chunk| {
                std.debug.assert(chunk.len <= buffer.len);
                @memcpy(buffer[0..chunk.len], chunk);
                self.chunk = null;
                return .{ .bytes = chunk.len };
            }
            if (self.pause_cancel) return .paused;
            self.condition.wait(&self.mutex);
        }
    }

    fn pauseRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        if (self.refuse_pause) |refuse| {
            refuse(self);
            return error.InjectedPauseFailure;
        }
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.pause_standing) return;
        self.pause_standing = true;
        if (self.reading != null) self.pause_cancel = true;
        self.condition.broadcast();
    }

    fn resumeRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        if (self.refuse_resume) return error.InjectedResumeFailure;
        self.mutex.lock();
        defer self.mutex.unlock();
        self.pause_standing = false;
    }

    fn abortRead(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        self.aborted = true;
        self.condition.broadcast();
    }

    fn batchFinished(context: *anyopaque, outcome: harness.ReaderBatchOutcome) void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(outcome == .committed);
        self.committed += 1;
        self.condition.broadcast();
    }

    fn readerStopped(context: *anyopaque) void {
        const self = fromContext(context);
        self.mutex.lock();
        defer self.mutex.unlock();
        self.stopped = true;
        self.condition.broadcast();
    }

    fn hooks(self: *ScriptedOutput) RuntimeTestingHooks {
        return .{
            .abort_read = abortRead,
            .context = self,
            .pause_read = pauseRead,
            .read_pty_pausable = read,
            .reader_batch_finished = batchFinished,
            .reader_stopped = readerStopped,
            .resume_read = resumeRead,
        };
    }

    /// Hands the reader one chunk and waits until its batch committed.
    fn commit(self: *ScriptedOutput, chunk: []const u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.chunk == null);
        const target = self.committed + 1;
        self.chunk = chunk;
        self.condition.broadcast();
        while (self.committed < target) self.condition.wait(&self.mutex);
    }

    /// Waits until the reader reads on through a standing pause.
    fn awaitReadThrough(self: *ScriptedOutput) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.reading != .through_pause) self.condition.wait(&self.mutex);
    }

    fn awaitStopped(self: *ScriptedOutput) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (!self.stopped) self.condition.wait(&self.mutex);
    }
};

const Segment = struct {
    bytes: []const u8,
    /// A cut inside leaves a parser or scanner inside a sequence.
    sequence: bool,
};

/// Every kind of sequence the Host's parsers keep across reads: Ghostty's CSI, OSC, DCS, APC and
/// escape states, its UTF-8 decoder, the OSC 7501 scanner, and the private APC beside them.
const corpus_segments = [_]Segment{
    .{ .bytes = "a", .sequence = false },
    .{ .bytes = "\x1b[31m", .sequence = true },
    .{ .bytes = "b", .sequence = false },
    .{ .bytes = "\x1b[0m", .sequence = true },
    .{ .bytes = "\x1b]2;pause-title\x07", .sequence = true },
    .{ .bytes = "c", .sequence = false },
    .{ .bytes = "\x1b]7501;state=idle\x1b\\", .sequence = true },
    .{ .bytes = "d", .sequence = false },
    .{ .bytes = "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\", .sequence = true },
    .{ .bytes = "e", .sequence = false },
    .{ .bytes = "\x1b_orchbee.command.v1;ready\x1b\\", .sequence = true },
    .{ .bytes = "f", .sequence = false },
    .{ .bytes = "\x1bPzz\x1b\\", .sequence = true },
    .{ .bytes = "\xc3\xa9", .sequence = true },
    .{ .bytes = "\xe4\xb8\xad", .sequence = true },
    .{ .bytes = "\xf0\x9f\x98\x80", .sequence = true },
    .{ .bytes = "\x1b7", .sequence = true },
    .{ .bytes = "\x1b8", .sequence = true },
    .{ .bytes = "g\r\n", .sequence = false },
};

const corpus = blk: {
    var bytes: []const u8 = "";
    for (corpus_segments) |segment| bytes = bytes ++ segment.bytes;
    break :blk bytes;
};

const corpus_line = "abcdef\xc3\xa9\xe4\xb8\xad\xf0\x9f\x98\x80g";

/// Whether the corpus cut at `offset` leaves every parser and scanner at ground.
fn groundAt(offset: usize) bool {
    var start: usize = 0;
    for (corpus_segments) |segment| {
        const end = start + segment.bytes.len;
        if (offset == start or offset == end) return true;
        if (offset < end) return !segment.sequence;
        start = end;
    }
    unreachable;
}

fn createScriptedRuntime(output: *ScriptedOutput) !*TerminalRuntime {
    const runtime = try harness.createRuntimeWithHooks(output.hooks());
    output.runtime = runtime;
    return runtime;
}

fn countLines(text: []const u8, expected: []const u8) struct { matching: usize, other: usize } {
    var matching: usize = 0;
    var other: usize = 0;
    var lines = std.mem.splitScalar(u8, text, '\n');
    while (lines.next()) |line| {
        if (line.len == 0) continue;
        if (std.mem.eql(u8, line, expected)) matching += 1 else other += 1;
    }
    return .{ .matching = matching, .other = other };
}

test "reader pause amid sequences cut across reads parks only at ground and loses nothing" {
    var output: ScriptedOutput = .{};
    const runtime = try createScriptedRuntime(&output);
    defer runtime.destroy();

    var parked: usize = 0;
    var read_on: usize = 0;
    var corpora: usize = 0;
    for (1..corpus.len) |cut| {
        // The program either goes on during the pause, or the pause is withdrawn first.
        for ([_]bool{ false, true }) |withdraw_first| {
            output.commit(corpus[0..cut]);
            try runtime.pauseReader();
            if (groundAt(cut)) {
                try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
                parked += 1;
                try runtime.resumeReader();
                output.commit(corpus[cut..]);
            } else {
                output.awaitReadThrough();
                try std.testing.expectEqual(ReaderParkWait.mid_sequence, runtime.awaitReaderParked(0));
                read_on += 1;
                if (withdraw_first) {
                    try runtime.resumeReader();
                    output.commit(corpus[cut..]);
                } else {
                    // The batch that ends the sequence leaves the parsers at ground.
                    output.commit(corpus[cut..]);
                    try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
                    try runtime.resumeReader();
                }
            }
            corpora += 1;
        }
    }
    try std.testing.expect(parked > 0 and read_on > 0);

    var capture = try screen_text.read(runtime, std.testing.allocator, capture_limit_bytes);
    defer capture.deinit();
    const counts = countLines(capture.text(), corpus_line);
    try std.testing.expectEqual(corpora, counts.matching);
    try std.testing.expectEqual(@as(usize, 0), counts.other);
}

test "reader pause the platform refuses is withdrawn and the reader reads on" {
    const Refusal = struct {
        /// The reader parks at ground before the platform refuses, so the rollback must wake it.
        fn parkFirst(self: *ScriptedOutput) void {
            self.commit("parked-before-refusal\r\n");
            self.refusal_park = self.runtime.awaitReaderParked(park_timeout_ns);
        }
    };
    var output: ScriptedOutput = .{ .refuse_pause = Refusal.parkFirst };
    const runtime = try createScriptedRuntime(&output);
    defer runtime.destroy();

    try std.testing.expectError(error.InjectedPauseFailure, runtime.pauseReader());
    try std.testing.expectEqual(ReaderParkWait.parked, output.refusal_park.?);
    try std.testing.expectEqual(ReaderParkWait.timed_out, runtime.awaitReaderParked(0));
    output.commit("read-on-after-refusal\r\n");

    var capture = try screen_text.read(runtime, std.testing.allocator, capture_limit_bytes);
    defer capture.deinit();
    try std.testing.expect(std.mem.indexOf(u8, capture.text(), "parked-before-refusal\nread-on-after-refusal") != null);
}

test "a parked reader ends when its terminal fails or shuts down" {
    {
        var output: ScriptedOutput = .{};
        const runtime = try createScriptedRuntime(&output);
        defer runtime.destroy();
        output.commit("before-failure\r\n");
        try runtime.pauseReader();
        try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
        harness.recordReaderFailure(runtime);
        output.awaitStopped();
        try std.testing.expectEqual(ReaderParkWait.stopped, runtime.awaitReaderParked(0));
    }
    {
        var output: ScriptedOutput = .{};
        const runtime = try createScriptedRuntime(&output);
        output.commit("before-shutdown\r\n");
        try runtime.pauseReader();
        try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
        runtime.destroy();
        try std.testing.expect(output.stopped);
    }
}

test "a pause the platform cannot withdraw fails only its terminal" {
    var output: ScriptedOutput = .{};
    const runtime = try createScriptedRuntime(&output);
    defer runtime.destroy();
    output.commit("before-resume\r\n");
    try runtime.pauseReader();
    try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
    output.refuse_resume = true;
    try std.testing.expectError(error.InjectedResumeFailure, runtime.resumeReader());
    output.awaitStopped();
    try std.testing.expectEqual(harness.subject.RuntimeFailureKind.pty_read, runtime.primaryFailure().?);
}
