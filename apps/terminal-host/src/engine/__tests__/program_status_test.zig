const harness = @import("runtime_harness.zig");
const std = harness.std;
const protocol = harness.protocol;
const program_status = harness.engine_boundary.testing.program_status;
const cancelInput = harness.cancelInput;
const commitReaderBatch = harness.commitReaderBatch;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const requireProjectionFrame = harness.requireProjectionFrame;
const runtimeState = harness.runtimeState;
const AttachmentId = harness.AttachmentId;
const GatedPtyProbe = harness.GatedPtyProbe;
const HostEpoch = harness.HostEpoch;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const TerminalRuntime = harness.TerminalRuntime;
const WriterProbe = harness.WriterProbe;

/// Scans `bytes` in `chunk`-sized reads and records each completed sequence as `?bel|`,
/// `?st|`, or `<body>|`, so chunk boundaries cannot change what is recognized.
fn scanChunks(bytes: []const u8, chunk: usize, out: *std.ArrayList(u8)) !void {
    var scanner: program_status.Scanner = .{};
    var offset: usize = 0;
    while (offset < bytes.len) {
        const end = @min(bytes.len, offset + chunk);
        var rest = bytes[offset..end];
        while (rest.len != 0) {
            const step = scanner.scan(rest);
            try std.testing.expect(step.consumed > 0);
            rest = rest[step.consumed..];
            const sequence = step.sequence orelse continue;
            switch (sequence) {
                .query => |terminator| try out.print(std.testing.allocator, "?{t}|", .{terminator}),
                .report => |body| try out.print(std.testing.allocator, "{s}|", .{body}),
            }
        }
        offset = end;
    }
}

test "OSC 7501 scanner finds complete sequences across every chunk boundary" {
    const max_body = program_status.max_sequence_bytes - "\x1b]7501;\x1b\\".len;
    const at_bound = "\x1b]7501;" ++ "a" ** max_body ++ "\x1b\\";
    const over_bound = "\x1b]7501;" ++ "a" ** (max_body + 1) ++ "\x1b\\";
    const cases = [_]struct { input: []const u8, expected: []const u8 }{
        .{ .input = "\x1b]7501;?\x07", .expected = "?bel|" },
        .{ .input = "\x1b]7501;?\x1b\\", .expected = "?st|" },
        .{ .input = "out\x1b]7501;state=idle\x07x\x1b]7501;state=done\x1b\\", .expected = "state=idle|state=done|" },
        // Other OSCs, including longer numbers sharing the prefix, are not program status.
        .{ .input = "\x1b]0;7501;state=idle\x07\x1b]75011;state=idle\x07", .expected = "" },
        // ESC, CAN, and SUB end an OSC without completing it; the ESC may begin the next one.
        .{ .input = "\x1b]7501;state=idle\x1b[m\x1b]7501;state=done\x07", .expected = "state=done|" },
        .{ .input = "\x1b]7501;state=idle\x18\x1b]7501;state=idle\x1a", .expected = "" },
        .{ .input = "\x1b]7501;state=idle\x1b\x1b]7501;state=done\x07", .expected = "state=done|" },
        // Control and non-ASCII bytes inside the body drop the sequence at its terminator.
        .{ .input = "\x1b]7501;state=idle\x01\x07\x1b]7501;state=\xc3\xa9\x07", .expected = "" },
        .{ .input = at_bound, .expected = "a" ** max_body ++ "|" },
        .{ .input = over_bound ++ "\x1b]7501;state=idle\x07", .expected = "state=idle|" },
        .{ .input = "\x1b]7501; ?\x07", .expected = " ?|" },
    };
    for (cases) |case| {
        for ([_]usize{ 1, 2, 7, case.input.len }) |chunk| {
            var actual: std.ArrayList(u8) = .empty;
            defer actual.deinit(std.testing.allocator);
            try scanChunks(case.input, chunk, &actual);
            try std.testing.expectEqualStrings(case.expected, actual.items);
        }
    }
}

test "OSC 7501 scanner ends each step at the completing terminator" {
    var scanner: program_status.Scanner = .{};
    const bytes = "ab\x1b]7501;state=working\x07cd";
    const step = scanner.scan(bytes);
    try std.testing.expectEqual(bytes.len - "cd".len, step.consumed);
    try std.testing.expectEqualStrings("state=working", step.sequence.?.report);
    const rest = scanner.scan(bytes[step.consumed..]);
    try std.testing.expectEqual(@as(usize, 2), rest.consumed);
    try std.testing.expect(rest.sequence == null);
}

test "OSC 7501 parser keeps only valid root-record states within every limit" {
    const Expected = ?program_status.Report;
    const cases = [_]struct { body: []const u8, expected: Expected }{
        .{ .body = "state=idle", .expected = .{ .set = .idle } },
        .{ .body = "state=working", .expected = .{ .set = .working } },
        .{ .body = "state=blocked", .expected = .{ .set = .blocked } },
        .{ .body = "state=done", .expected = .{ .set = .done } },
        .{ .body = "state=error", .expected = .{ .set = .@"error" } },
        .{ .body = "state=clear", .expected = .clear },
        .{ .body = "app=opencode:state=working:msg=aGk=:title=aGk:kind=agent", .expected = .{ .set = .working } },
        // A repeated key keeps its last value; malformed pairs and unknown keys are skipped.
        .{ .body = "state=idle:state=blocked", .expected = .{ .set = .blocked } },
        .{ .body = "junk:=x:Bad=1:state=done:note=a b", .expected = .{ .set = .done } },
        .{ .body = "state=done:state=wor king", .expected = .{ .set = .done } },
        // Missing or unknown states, and every non-root record, are ignored.
        .{ .body = "", .expected = null },
        .{ .body = "app=pi", .expected = null },
        .{ .body = "state=busy", .expected = null },
        .{ .body = "state=Idle", .expected = null },
        .{ .body = "state=working:id=build", .expected = null },
        .{ .body = "state=clear:id=build/test", .expected = null },
        .{ .body = "state=idle:id=", .expected = null },
        // An exceeded limit, undecodable base64, or control text discards the whole report.
        .{ .body = "abcdefghijklmnopq=1:state=idle", .expected = null },
        .{ .body = "abcdefghijklmnop=1:state=idle", .expected = .{ .set = .idle } },
        .{ .body = "app=" ++ "a" ** 33 ++ ":state=idle", .expected = null },
        .{ .body = "app=" ++ "a" ** 32 ++ ":state=idle", .expected = .{ .set = .idle } },
        .{ .body = "msg=a:state=idle", .expected = null },
        .{ .body = "msg=AQ==:state=idle", .expected = null },
        .{ .body = "msg=/w==:state=idle", .expected = null },
        .{ .body = "title=" ++ "QUFB" ** 64 ++ "QQ:state=idle", .expected = null },
        .{ .body = "title=" ++ "QUFB" ** 64 ++ ":state=idle", .expected = .{ .set = .idle } },
        .{ .body = "msg=" ++ "QUFB" ** 683 ++ ":state=idle", .expected = null },
        .{ .body = "msg=" ++ "QUFB" ** 682 ++ "QUE:state=idle", .expected = .{ .set = .idle } },
    };
    for (cases) |case| {
        const actual = program_status.parse(case.body);
        std.testing.expectEqualDeep(case.expected, actual) catch |err| {
            std.debug.print("body: {s}\n", .{case.body});
            return err;
        };
    }
}

const WallClockProbe = struct {
    milliseconds: u64,

    pub fn hooks(self: *WallClockProbe) RuntimeTestingHooks {
        return .{ .context = self, .wall_time_milliseconds = read };
    }

    fn read(context: *anyopaque) u64 {
        const self: *WallClockProbe = @ptrCast(@alignCast(context));
        return self.milliseconds;
    }
};

fn projectedStatus(runtime: *TerminalRuntime, sequence: u64) !?protocol.TerminalProgramStatus {
    const frame = try requireProjectionFrame(try runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        0,
        .{
            .attachment_id = AttachmentId.fromBytes([_]u8{0x75} ** 16),
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = HostEpoch.fromBytes([_]u8{0x01} ** 16),
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = sequence,
        },
    ));
    defer std.testing.allocator.free(frame.allocation);
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame.bytes);
    return summary.program_status;
}

fn commit(runtime: *TerminalRuntime, bytes: []const u8) !void {
    try std.testing.expectEqual(ReaderBatchOutcome.committed, commitReaderBatch(runtime, bytes));
}

test "OSC 7501 root record projects with its report time and ends at prompts and resets" {
    var probe: GatedPtyProbe(WallClockProbe) = .{ .probe = .{ .milliseconds = 1_786_947_200_000 } };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        runtime.destroy();
    }
    probe.reader.wait();

    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 1));
    try commit(runtime, "\x1b]7501;app=t:state=working\x07");
    try std.testing.expectEqualDeep(@as(?protocol.TerminalProgramStatus, .{
        .state = .working,
        .reported_at_nanoseconds = 1_786_947_200_000 * std.time.ns_per_ms,
    }), try projectedStatus(runtime, 2));

    // Ignored reports and child records leave the root record and its time untouched.
    probe.probe.milliseconds += 1;
    try commit(runtime, "\x1b]7501;state=busy\x07\x1b]7501;id=a:state=clear\x07");
    try std.testing.expectEqual(
        @as(u64, 1_786_947_200_000 * std.time.ns_per_ms),
        (try projectedStatus(runtime, 3)).?.reported_at_nanoseconds,
    );

    // Secondary prompts keep a live report; a new prompt ends it.
    try commit(runtime, "\x1b]133;P;k=s\x07");
    try std.testing.expectEqual(protocol.TerminalProgramStatusState.working, (try projectedStatus(runtime, 4)).?.state);
    try commit(runtime, "\x1b]133;A\x07");
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 5));

    // Effects apply in stream order inside one batch.
    try commit(runtime, "\x1b]7501;state=blocked\x07\x1b]133;P\x07\x1b]7501;state=idle\x1b\\");
    try std.testing.expectEqual(protocol.TerminalProgramStatusState.idle, (try projectedStatus(runtime, 6)).?.state);
    try commit(runtime, "\x1b]133;N\x07");
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 7));

    // Completed reports survive prompts but not a full reset or the Host reset.
    try commit(runtime, "\x1b]7501;state=done\x07\x1b]133;A\x07");
    try std.testing.expectEqual(protocol.TerminalProgramStatusState.done, (try projectedStatus(runtime, 8)).?.state);
    try commit(runtime, "\x1bc");
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 9));
    try commit(runtime, "\x1b]7501;state=error\x07");
    _ = try runtime.applyTerminalControl(.reset);
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 10));

    // A report split across reader batches is recognized once complete, and clear removes it.
    try commit(runtime, "\x1b]7501;sta");
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 11));
    try commit(runtime, "te=working\x07");
    try std.testing.expectEqual(protocol.TerminalProgramStatusState.working, (try projectedStatus(runtime, 12)).?.state);
    try commit(runtime, "\x1b]7501;state=clear\x07");
    try std.testing.expectEqual(@as(?protocol.TerminalProgramStatus, null), try projectedStatus(runtime, 13));
}

fn expectNextReply(runtime: *TerminalRuntime, expected: []const u8) !void {
    var buffer: [64]u8 = undefined;
    const runtime_state = runtimeState(runtime);
    const identifier = blk: {
        runtime_state.input_mutex.lock();
        defer runtime_state.input_mutex.unlock();
        if (!runtime_state.input_queue.hasWritableEntry()) return error.ExpectedQueuedInput;
        const slice = runtime_state.input_queue.copyNextWriterSlice(&buffer);
        try std.testing.expectEqual(harness.InputSource.terminal, slice.source);
        try std.testing.expectEqualStrings(expected, buffer[0..slice.length]);
        break :blk slice.identifier;
    };
    cancelInput(runtime, identifier);
}

test "OSC 7501 query replies in stream order with the query's terminator" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    try commit(runtime, "\x1b[5n\x1b]7501;?\x07\x1b[5n\x1b]7501;?\x1b\\");
    try expectNextReply(runtime, "\x1b[0n");
    try expectNextReply(runtime, "\x1b]7501;?\x07");
    try expectNextReply(runtime, "\x1b[0n");
    try expectNextReply(runtime, "\x1b]7501;?\x1b\\");
}
