const harness = @import("runtime_harness.zig");
const std = harness.std;
const target_builtin = harness.target_builtin;
const ghostty_vt = harness.ghostty_vt;
const engine_boundary = harness.engine_boundary;
const model_boundary = harness.model_boundary;
const platform_boundary = harness.platform_boundary;
const protocol = harness.protocol;
const semantic_wait = harness.semantic_wait;
const subject = harness.subject;
const terminal_launch = harness.terminal_launch;
const pty_process = harness.pty_process;
const applyScrollbackBytes = harness.applyScrollbackBytes;
const cancelInput = harness.cancelInput;
const clearTestingHooks = harness.clearTestingHooks;
const closeClientInput = harness.closeClientInput;
const commitReaderBatch = harness.commitReaderBatch;
const completionClassificationLoad = harness.completionClassificationLoad;
const createWithOptionsUsingSpawner = harness.createWithOptionsUsingSpawner;
const deinitOwnedProfile = harness.deinitOwnedProfile;
const FirstContainmentAttempt = harness.FirstContainmentAttempt;
const InputReservation = harness.InputReservation;
const InputSource = harness.InputSource;
const initOwnedProfile = harness.initOwnedProfile;
const inputQueueSnapshot = harness.inputQueueSnapshot;
const modifiers = harness.modifiers;
const mouseEncodeEvent = harness.mouseEncodeEvent;
const mouseHasPressedButton = harness.mouseHasPressedButton;
const markNormalCompletionReady = harness.markNormalCompletionReady;
const ProjectionEncodeRequest = harness.ProjectionEncodeRequest;
const AttachmentId = harness.AttachmentId;
const HostEpoch = harness.HostEpoch;
const reserveInput = harness.reserveInput;
const TerminalRuntime = harness.TerminalRuntime;
const runtimeState = harness.runtimeState;
const max_pending_input_bytes = harness.max_pending_input_bytes;
const max_pending_input_entries = harness.max_pending_input_entries;
const max_pending_terminal_reply_bytes = harness.max_pending_terminal_reply_bytes;
const max_writer_slice_bytes = harness.max_writer_slice_bytes;
const terminal_reply_burst_bound = harness.terminal_reply_burst_bound;
const publishInput = harness.publishInput;
const recordReaderFailure = harness.recordReaderFailure;
const recordTerminalReplyFailure = harness.recordTerminalReplyFailure;
const recordWorkerIntegrityFailure = harness.recordWorkerIntegrityFailure;
const recordWriterFailure = harness.recordWriterFailure;
const releaseOutputForShutdown = harness.releaseOutputForShutdown;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const resizePtyWithRollback = harness.resizePtyWithRollback;
const RuntimeTestingHooks = harness.RuntimeTestingHooks;
const Geometry = harness.Geometry;
const requireProjectionFrame = harness.requireProjectionFrame;
const TestGate = harness.TestGate;
const TestSignal = harness.TestSignal;
const exerciseOwnedProfileAllocations = harness.exerciseOwnedProfileAllocations;
const FailingThreadSpawner = harness.FailingThreadSpawner;
const CountingThreadSpawner = harness.CountingThreadSpawner;
const HookedThreadSpawner = harness.HookedThreadSpawner;
const createRuntimeWithHooks = harness.createRuntimeWithHooks;
const createRuntimeWithHooksAndCommand = harness.createRuntimeWithHooksAndCommand;
const waitForWriterStop = harness.waitForWriterStop;
const createIdleRuntime = harness.createIdleRuntime;
const destroyRuntimeWithIdentityRetry = harness.destroyRuntimeWithIdentityRetry;
const createIdleRuntimeWithGeometry = harness.createIdleRuntimeWithGeometry;
const parseTerminalInput = harness.parseTerminalInput;
const WriterPause = harness.WriterPause;
const WriterProbe = harness.WriterProbe;
const RepeatingGate = harness.RepeatingGate;
const BurstProbe = harness.BurstProbe;
const DescendantProbe = harness.DescendantProbe;
const ReaderProbe = harness.ReaderProbe;
const CanonicalSnapshot = harness.CanonicalSnapshot;
const canonicalSnapshot = harness.canonicalSnapshot;
const expectCanonicalSnapshotEqual = harness.expectCanonicalSnapshotEqual;
const readFrameInt = harness.readFrameInt;
const projectionFrameRowOffset = harness.projectionFrameRowOffset;
const expectIncludedProjectionRows = harness.expectIncludedProjectionRows;
const FrameHyperlinkObservation = harness.FrameHyperlinkObservation;
const observeFrameHyperlinks = harness.observeFrameHyperlinks;
const writeProjectionOutput = harness.writeProjectionOutput;
const expectProjectionRowRevisions = harness.expectProjectionRowRevisions;
const expectProjectionLinkTargets = harness.expectProjectionLinkTargets;
const expectInputQueuesCleared = harness.expectInputQueuesCleared;
const ContainmentProbe = harness.ContainmentProbe;
const FailureReporter = harness.FailureReporter;
const ShutdownProbe = harness.ShutdownProbe;
const OutputCleanupProbe = harness.OutputCleanupProbe;
const ResizeEffectOutcome = harness.ResizeEffectOutcome;
const ResizeProbe = harness.ResizeProbe;
const ResizeObservation = harness.ResizeObservation;
const TerminalDrivenResizeProbe = harness.TerminalDrivenResizeProbe;
const TerminalResizeSerializationProbe = harness.TerminalResizeSerializationProbe;
const ResizeCall = harness.ResizeCall;
const ReaderBatchCall = harness.ReaderBatchCall;
const expectRealPtyQueryReplies = harness.expectRealPtyQueryReplies;
const driveReplyBurstUntilClient = harness.driveReplyBurstUntilClient;
const parseDescendantPid = harness.parseDescendantPid;
const procCommEquals = harness.procCommEquals;
const processAlive = harness.processAlive;

test "terminal generated replies are excluded from client input accounting" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    const arguments = [_][]const u8{
        "-c",
        "stty -echo -icanon min 9 time 20; printf '\\033[c'; " ++
            "response=$(dd bs=9 count=1 2>/dev/null); " ++
            "expected=$(printf '\\033[?62;22c'); [ \"$response\" = \"$expected\" ]",
    };
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = &arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .rows = 24,
    });
    defer runtime.destroy();

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const completion = runtime.completion();
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);
    switch (completion.classification) {
        .normal => {},
        .pending, .failed => return error.ExpectedNormalCompletion,
    }
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}
test "real PTY preserves the legacy terminal query and reply corpus" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const query_shell = "\\033[5n\\033[6n" ++
        "\\033[?996n\\033[>q\\005" ++
        "\\033[c\\033[>c\\033[=c" ++
        "\\033P$qm\\033\\\\" ++
        "\\033[1;31m\\033P$qm\\033\\\\" ++
        "\\033[5 q\\033P$q q\\033\\\\" ++
        "\\033[3;20r\\033P$qr\\033\\\\" ++
        "\\033P$qs\\033\\\\" ++
        "\\033[?69h\\033[5;70s\\033P$qs\\033\\\\" ++
        "\\033P$q\"q\\033\\\\";
    const expected = "\x1b[0n\x1b[1;1R" ++
        "\x1b[?997;1n" ++
        "\x1bP>|orchbee-test 2.4.0\x1b\\" ++
        "orchbee-ready" ++
        "\x1b[?62;22c\x1b[>1;0;0c\x1bP!|00000000\x1b\\" ++
        "\x1bP1$r0m\x1b\\" ++
        "\x1bP1$r0;1;31m\x1b\\" ++
        "\x1bP1$r5 q\x1b\\" ++
        "\x1bP1$r3;20r\x1b\\" ++
        "\x1bP0$r\x1b\\" ++
        "\x1bP1$r5;70s\x1b\\" ++
        "\x1bP0$r\x1b\\";
    const expected_shell = "\\033[0n\\033[1;1R" ++
        "\\033[?997;1n" ++
        "\\033P>|orchbee-test 2.4.0\\033\\\\" ++
        "orchbee-ready" ++
        "\\033[?62;22c\\033[>1;0;0c\\033P!|00000000\\033\\\\" ++
        "\\033P1$r0m\\033\\\\" ++
        "\\033P1$r0;1;31m\\033\\\\" ++
        "\\033P1$r5 q\\033\\\\" ++
        "\\033P1$r3;20r\\033\\\\" ++
        "\\033P0$r\\033\\\\" ++
        "\\033P1$r5;70s\\033\\\\" ++
        "\\033P0$r\\033\\\\";
    try expectRealPtyQueryReplies(
        80,
        24,
        .dark,
        query_shell,
        expected_shell,
        expected.len,
    );

    const size_expected = "\x1b[4;32;80t\x1b[6;16;8t\x1b[8;2;10t";
    try expectRealPtyQueryReplies(
        10,
        2,
        .dark,
        "\\033[14t\\033[16t\\033[18t",
        "\\033[4;32;80t\\033[6;16;8t\\033[8;2;10t",
        size_expected.len,
    );

    const light_expected = "\x1b[?997;2n";
    try expectRealPtyQueryReplies(
        80,
        24,
        .light,
        "\\033[?996n",
        "\\033[?997;2n",
        light_expected.len,
    );

    const color_query_shell = "\\033]10;rgb:d8/de/e9\\033\\\\" ++
        "\\033]11;rgb:10/14/18\\033\\\\" ++
        "\\033]12;rgb:f5/a9/7f\\033\\\\" ++
        "\\033]4;1;rgb:aa/bb/cc\\033\\\\" ++
        "\\033]10;?;?;?\\033\\\\" ++
        "\\033]4;1;?\\033\\\\";
    const color_expected = "\x1b]10;rgb:d8d8/dede/e9e9\x1b\\" ++
        "\x1b]11;rgb:1010/1414/1818\x1b\\" ++
        "\x1b]12;rgb:f5f5/a9a9/7f7f\x1b\\" ++
        "\x1b]4;1;rgb:aaaa/bbbb/cccc\x1b\\";
    try expectRealPtyQueryReplies(
        80,
        24,
        .dark,
        color_query_shell,
        "\\033]10;rgb:d8d8/dede/e9e9\\033\\\\" ++
            "\\033]11;rgb:1010/1414/1818\\033\\\\" ++
            "\\033]12;rgb:f5f5/a9a9/7f7f\\033\\\\" ++
            "\\033]4;1;rgb:aaaa/bbbb/cccc\\033\\\\",
        color_expected.len,
    );
}
test "pinned Ghostty OSC color updates and queries share one color state" {
    const Reply = struct {
        var bytes: [256]u8 = undefined;
        var length: usize = 0;

        fn reset() void {
            length = 0;
        }

        fn writePty(
            _: *ghostty_vt.TerminalStream.Handler,
            data: []const u8,
        ) void {
            if (data.len > bytes.len - length) @panic("color reply exceeds test buffer");
            @memcpy(bytes[length..][0..data.len], data);
            length += data.len;
        }
    };
    Reply.reset();

    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 80,
        .rows = 24,
    });
    defer terminal.deinit(std.testing.allocator);
    var stream = terminal.vtStream();
    defer stream.deinit();
    stream.handler.effects.write_pty = Reply.writePty;

    stream.nextSlice(
        "\x1b]10;rgb:d8/de/e9\x1b\\" ++
            "\x1b]11;rgb:10/14/18\x1b\\" ++
            "\x1b]12;rgb:f5/a9/7f\x1b\\" ++
            "\x1b]4;1;rgb:aa/bb/cc\x1b\\",
    );
    try std.testing.expectEqual(@as(usize, 0), Reply.length);

    stream.nextSlice("\x1b]10;?;?;?\x1b\\\x1b]4;1;?\x1b\\");
    try std.testing.expectEqualStrings(
        "\x1b]10;rgb:d8d8/dede/e9e9\x1b\\" ++
            "\x1b]11;rgb:1010/1414/1818\x1b\\" ++
            "\x1b]12;rgb:f5f5/a9a9/7f7f\x1b\\" ++
            "\x1b]4;1;rgb:aaaa/bbbb/cccc\x1b\\",
        Reply.bytes[0..Reply.length],
    );
}
test "root child exit rejects new client input without stopping terminal replies" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    closeClientInput(runtime);
    try std.testing.expectError(
        error.TerminalExited,
        reserveInput(runtime, 1, false, .client),
    );
    const reply = try reserveInput(runtime, 1, false, .terminal);
    try publishInput(runtime, reply, try std.testing.allocator.dupe(u8, "x"));
    runtimeState(runtime).input_mutex.lock();
    try std.testing.expect(!runtimeState(runtime).writer_stop);
    runtimeState(runtime).input_mutex.unlock();
    try runtime.forceTerminate();
}
test "runtime queue applies bounded backpressure before allocation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 1" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 3 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const bytes = try std.testing.allocator.alloc(u8, max_pending_input_bytes + 1);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'x');
    try std.testing.expectError(error.InputBackpressure, runtime.enqueueInput(bytes));
    try runtime.terminate();
}
test "accepted client entry makes bounded progress behind a continuously replenished reply stream" {
    var probe: BurstProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer {
        probe.detached.store(true, .release);
        probe.gate.unpause();
    }

    // A child continuously emits reply-producing queries. Admit one full reply
    // burst and park the writer on it before accepting the client entry, so the
    // client waits behind reply work deterministically.
    for (0..terminal_reply_burst_bound) |_| {
        const reply_reservation = try reserveInput(runtime, 3, false, .terminal);
        try publishInput(runtime, reply_reservation, try std.testing.allocator.dupe(u8, "rrr"));
    }
    probe.gate.wait();
    const client_reservation = try reserveInput(runtime, 5, false, .client);
    try publishInput(runtime, client_reservation, try std.testing.allocator.dupe(u8, "aaaaa"));

    // The reply stream is replenished as the writer drains it. Without the
    // reply burst bound the accepted client entry would starve behind the
    // endless stream; with the bound it is delivered after exactly one burst.
    const before_client = try driveReplyBurstUntilClient(
        runtime,
        &probe,
        4 * terminal_reply_burst_bound,
    );
    if (before_client == null) return error.ClientStarvedBehindReplyStream;
    try std.testing.expectEqual(@as(usize, terminal_reply_burst_bound), before_client.?);
    try std.testing.expectEqual(@as(usize, 5), runtime.takeWrittenBytes());
}
test "terminal reply burst enforces the exact yield bound and replies keep priority within the burst" {
    var probe: BurstProbe = .{};
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer {
        probe.detached.store(true, .release);
        probe.gate.unpause();
    }

    // The accepted client entry holds the front FIFO position while a reply
    // stream larger than one burst sits behind it. Reserve it first so the
    // writer parks on a reply slice, then publish it before driving.
    const client_reservation = try reserveInput(runtime, 5, false, .client);
    for (0..2 * terminal_reply_burst_bound) |_| {
        const reply_reservation = try reserveInput(runtime, 3, false, .terminal);
        try publishInput(runtime, reply_reservation, try std.testing.allocator.dupe(u8, "rrr"));
    }
    probe.gate.wait();
    try publishInput(runtime, client_reservation, try std.testing.allocator.dupe(u8, "aaaaa"));

    const before_client = try driveReplyBurstUntilClient(
        runtime,
        &probe,
        4 * terminal_reply_burst_bound,
    );
    if (before_client == null) return error.ClientStarvedBehindReplyStream;
    // Exactly one reply burst precedes the client: the writer yields after the
    // bounded slice budget, not after the whole injected stream.
    try std.testing.expectEqual(@as(usize, terminal_reply_burst_bound), before_client.?);
    // Within the burst every written slice is a terminal reply (priority), and
    // the accepted client entry is delivered once the writer yields.
    const written = probe.writtenBytes();
    for (written[0..terminal_reply_burst_bound]) |byte| {
        try std.testing.expectEqual(@as(u8, 'r'), byte);
    }
    try std.testing.expectEqual(@as(usize, 5), runtime.takeWrittenBytes());
}
test "discarding prepared input releases its ordered queue reservation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    var prepared = try runtime.prepareRawInput("abc");
    try std.testing.expectEqual(@as(usize, 3), prepared.byteCount());
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    prepared.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "prepared input losing the failure fence is unavailable and releases its reservation" {
    const runtime = try createRuntimeWithHooks(.{});
    defer runtime.destroy();

    var prepared = try runtime.prepareRawInput("prepared");
    defer prepared.deinit();
    try std.testing.expectEqual(@as(usize, 8), runtime.pendingInputBytes());

    recordReaderFailure(runtime);
    try std.testing.expectError(error.TerminalRuntimeUnavailable, prepared.commit());
    waitForWriterStop(runtime);

    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
    try std.testing.expectEqual(subject.RuntimeFailureKind.pty_read, runtime.primaryFailure().?);
}
test "post-exit client closure rejects a prepared client commit and releases its reservation" {
    const runtime = try createRuntimeWithHooks(.{});
    defer runtime.destroy();

    var prepared = try runtime.prepareTextInput("prepared", std.math.maxInt(usize));
    defer prepared.deinit();
    try std.testing.expectEqual(@as(usize, 8), runtime.pendingInputBytes());
    try std.testing.expectEqual(
        @as(usize, 1),
        inputQueueSnapshot(runtime).client_charge.entries,
    );

    // Root exit closes client admission while writer_stop is still false; the
    // previously reserved publication must fail at its one commit boundary.
    closeClientInput(runtime);
    try std.testing.expectError(error.TerminalExited, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    const snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), snapshot.entry_count);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.entries);
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}
test "client input publication linearizes against post-exit client closure" {
    // Order 1: publication acquires input_mutex before closure. It is accepted
    // exactly once and stays as the queue's single published client entry.
    {
        var probe: WriterProbe = .{ .pause_at = .copied };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            probe.copied.release();
            clearTestingHooks(runtime);
            runtime.destroy();
        }
        const first = try reserveInput(runtime, 1, false, .client);
        try publishInput(runtime, first, try std.testing.allocator.dupe(u8, "a"));
        probe.copied.wait();
        closeClientInput(runtime);
        const snapshot = inputQueueSnapshot(runtime);
        try std.testing.expectEqual(@as(usize, 1), snapshot.entry_count);
        try std.testing.expectEqual(@as(usize, 1), snapshot.client_charge.bytes);
        try std.testing.expectEqual(@as(usize, 1), snapshot.client_charge.entries);
    }

    // Order 2: closure acquires input_mutex before publication. The reservation
    // is removed, its byte and entry charge released, and the owned bytes freed.
    {
        const runtime = try createRuntimeWithHooks(.{});
        defer runtime.destroy();
        const second = try reserveInput(runtime, 1, false, .client);
        closeClientInput(runtime);
        try std.testing.expectError(
            error.TerminalExited,
            publishInput(runtime, second, try std.testing.allocator.dupe(u8, "b")),
        );
        const snapshot = inputQueueSnapshot(runtime);
        try std.testing.expectEqual(@as(usize, 0), snapshot.entry_count);
        try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.bytes);
        try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.entries);
    }
}
test "client admission closure rejects client commits while terminal replies still publish" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    // A terminal-generated reply reservation still publishes after client
    // admission has closed; only client source entries are gated.
    const reply = try reserveInput(runtime, 1, false, .terminal);
    closeClientInput(runtime);
    try publishInput(runtime, reply, try std.testing.allocator.dupe(u8, "r"));
    probe.copied.wait();
    const reply_snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 1), reply_snapshot.entry_count);
    try std.testing.expectEqual(@as(usize, 1), reply_snapshot.terminal_reply_charge.bytes);
    try std.testing.expectEqual(@as(usize, 1), reply_snapshot.terminal_reply_charge.entries);

    // A zero-byte prepared semantic input shares the same one-phase commit gate.
    var noop = try runtime.prepareFocusInput(false, std.math.maxInt(usize));
    defer noop.deinit();
    try std.testing.expectEqual(@as(usize, 0), noop.byteCount());
    try std.testing.expectError(error.TerminalExited, noop.commit());
    const client_snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), client_snapshot.client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), client_snapshot.client_charge.entries);
}
test "post-exit descendant keeping the PTY open cannot receive a later committed client input" {
    const builtin = @import("builtin");
    if (builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: DescendantProbe = .{};
    const arguments = [_][]const u8{ "-c", "sleep 30" };
    const runtime = try createRuntimeWithHooksAndCommand(probe.hooks(), "/bin/sh", &arguments);
    defer {
        probe.read_gate.release();
        clearTestingHooks(runtime);
        runtime.destroy();
    }

    // The root child is still running: reserve a client FIFO position and its bytes.
    const descendant_text = "descendant-input";
    var prepared = try runtime.prepareTextInput(descendant_text, std.math.maxInt(usize));
    defer prepared.deinit();
    try std.testing.expectEqual(descendant_text.len, runtime.pendingInputBytes());

    // Root exit closes client admission; the PTY-holding descendant keeps the
    // writer running, so writer_stop stays false.
    try runtime.terminate();
    runtimeState(runtime).input_mutex.lock();
    while (!runtimeState(runtime).child_exited.load(.acquire)) {
        runtimeState(runtime).input_condition.wait(&runtimeState(runtime).input_mutex);
    }
    const writer_still_open = !runtimeState(runtime).writer_stop;
    runtimeState(runtime).input_mutex.unlock();
    try std.testing.expect(writer_still_open);

    // A preparation committed only after child exit is rejected and its bytes
    // never reach the descendant's PTY.
    try std.testing.expectError(error.TerminalExited, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    const snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), snapshot.entry_count);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), snapshot.client_charge.entries);
    try std.testing.expectEqual(@as(usize, 0), runtime.takeWrittenBytes());
}
test "client and reply writer slices obey the final failure-fence ordering" {
    const Case = struct {
        expected_calls: usize,
        pause_at: WriterPause,
        source: InputSource,
    };
    const cases = [_]Case{
        .{ .expected_calls = 0, .pause_at = .copied, .source = .client },
        .{ .expected_calls = 1, .pause_at = .started, .source = .client },
        .{ .expected_calls = 0, .pause_at = .copied, .source = .terminal },
        .{ .expected_calls = 1, .pause_at = .started, .source = .terminal },
    };
    const bytes = try std.testing.allocator.alloc(u8, max_writer_slice_bytes + 17);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'w');

    for (cases) |case| {
        var probe: WriterProbe = .{ .pause_at = case.pause_at };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer runtime.destroy();
        defer probe.copied.release();
        defer probe.started.release();

        const reservation = try reserveInput(runtime, bytes.len, false, case.source);
        try publishInput(
            runtime,
            reservation,
            try std.testing.allocator.dupe(u8, bytes),
        );
        switch (case.pause_at) {
            .copied => probe.copied.wait(),
            .started => probe.started.wait(),
        }

        recordReaderFailure(runtime);
        switch (case.pause_at) {
            .copied => probe.copied.release(),
            .started => probe.started.release(),
        }
        waitForWriterStop(runtime);

        const counts = probe.counts();
        try std.testing.expectEqual(case.expected_calls, counts.calls);
        try std.testing.expectEqual(
            case.expected_calls * max_writer_slice_bytes,
            counts.bytes,
        );
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
        const expected_client_bytes = if (case.source == .client)
            case.expected_calls * max_writer_slice_bytes
        else
            0;
        try std.testing.expectEqual(expected_client_bytes, runtime.takeWrittenBytes());
    }
}
test "an already-read PTY batch losing terminal_mutex cannot mutate canonical state" {
    var probe: ReaderProbe = .{ .batch = "must-not-commit" };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer probe.batch_read.release();
    defer probe.batch_finished.release();

    probe.batch_read.wait();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();

    recordReaderFailure(runtime);
    probe.batch_read.release();
    probe.batch_finished.wait();
    var after = try canonicalSnapshot(runtime);
    defer after.deinit();

    try std.testing.expectEqual(ReaderBatchOutcome.failure_fenced, probe.observedOutcome().?);
    try std.testing.expectEqual(before.revision, after.revision);
    try std.testing.expectEqual(before.generation, after.generation);
    try std.testing.expectEqual(before.total_rows, after.total_rows);
    try std.testing.expectEqualStrings(before.text, after.text);
    try std.testing.expectEqual(@as(u64, 0), runtime.takeOutputBatches());

    probe.batch_finished.release();
    probe.stopped.wait();
    try std.testing.expectEqual(@as(usize, 1), probe.readCallCount());
}
test "a PTY batch winning terminal_mutex commits once and freezes at the later fence" {
    var probe: ReaderProbe = .{ .batch = "reader-winner" };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer runtime.destroy();
    defer probe.batch_read.release();
    defer probe.batch_finished.release();

    probe.batch_read.wait();
    var before = try canonicalSnapshot(runtime);
    defer before.deinit();
    probe.batch_read.release();
    probe.batch_finished.wait();
    try std.testing.expectEqual(ReaderBatchOutcome.committed, probe.observedOutcome().?);

    var committed = try canonicalSnapshot(runtime);
    defer committed.deinit();
    try std.testing.expectEqual(before.revision + 1, committed.revision);
    try std.testing.expect(std.mem.indexOf(u8, committed.text, "reader-winner") != null);

    recordReaderFailure(runtime);
    probe.batch_finished.release();
    probe.stopped.wait();
    var frozen = try canonicalSnapshot(runtime);
    defer frozen.deinit();

    try std.testing.expectEqual(committed.revision, frozen.revision);
    try std.testing.expectEqual(committed.generation, frozen.generation);
    try std.testing.expectEqual(committed.total_rows, frozen.total_rows);
    try std.testing.expectEqualStrings(committed.text, frozen.text);
    try std.testing.expectEqual(@as(u64, 1), runtime.takeOutputBatches());
    try std.testing.expectEqual(@as(usize, 1), probe.readCallCount());
}
test "zero-byte semantic encodings do not retain queue metadata" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    for (0..max_pending_input_entries + 1) |_| {
        try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    }
    const empty_snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), empty_snapshot.entry_count);
    try std.testing.expectEqual(@as(usize, 0), empty_snapshot.client_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), empty_snapshot.terminal_reply_charge.bytes);

    var entry_reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&entry_reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const entry_reply = try reserveInput(runtime, 1, false, .terminal);
    cancelInput(runtime, entry_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    for (entry_reservations) |reservation| cancelInput(runtime, reservation.identifier);
    const byte_reservation = try reserveInput(runtime, max_pending_input_bytes, false, .client);
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    const byte_reply = try reserveInput(
        runtime,
        max_pending_terminal_reply_bytes,
        false,
        .terminal,
    );
    cancelInput(runtime, byte_reply.identifier);
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInput(true));
    cancelInput(runtime, byte_reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "semantic input limits apply to encoded terminal bytes" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    try std.testing.expectEqual(@as(usize, 3), try runtime.enqueueTextInputWithLimit("abc", 3));
    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.enqueueTextInputWithLimit("abcd", 3),
    );
    try std.testing.expectEqual(@as(usize, 0), try runtime.enqueueFocusInputWithLimit(true, 0));
    try runtime.forceTerminate();
}
test "shared terminal focus emits only aggregate zero and nonzero transitions" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const first_attachment = AttachmentId.fromBytes([_]u8{0x21} ** 16);
    const second_attachment = AttachmentId.fromBytes([_]u8{0x22} ** 16);
    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        var attachment = try runtime.prepareAttachment(
            attachment_id,
            .{ .input = true, .read = false, .resize = false },
        );
        attachment.commit();
    }
    parseTerminalInput(runtime, "\x1b[?1004h");

    const steps = [_]struct {
        attachment_id: AttachmentId,
        expected_bytes: []const u8,
        focused: bool,
    }{
        .{ .attachment_id = first_attachment, .expected_bytes = "\x1b[I", .focused = true },
        .{ .attachment_id = first_attachment, .expected_bytes = "", .focused = true },
        .{ .attachment_id = second_attachment, .expected_bytes = "", .focused = true },
        .{ .attachment_id = first_attachment, .expected_bytes = "", .focused = false },
        .{ .attachment_id = first_attachment, .expected_bytes = "", .focused = false },
        .{ .attachment_id = second_attachment, .expected_bytes = "\x1b[O", .focused = false },
        .{ .attachment_id = second_attachment, .expected_bytes = "", .focused = false },
    };
    for (steps) |step| {
        var prepared = try runtime.prepareAttachmentSemanticInput(
            step.attachment_id,
            .{ .focus = .{ .focused = step.focused } },
            std.math.maxInt(usize),
        );
        defer prepared.deinit();
        try std.testing.expectEqual(step.expected_bytes.len, prepared.byteCount());
        if (prepared.bytes) |bytes| {
            try std.testing.expectEqualSlices(u8, step.expected_bytes, bytes);
        } else {
            try std.testing.expectEqual(@as(usize, 0), step.expected_bytes.len);
        }
        _ = try prepared.commit();
    }
    try runtime.forceTerminate();
}
test "attachment repair orders key mouse and final aggregate focus release" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    const first_attachment = AttachmentId.fromBytes([_]u8{0x23} ** 16);
    const second_attachment = AttachmentId.fromBytes([_]u8{0x24} ** 16);
    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        var attachment = try runtime.prepareAttachment(
            attachment_id,
            .{ .input = true, .read = false, .resize = false },
        );
        attachment.commit();
    }
    parseTerminalInput(runtime, "\x1b[=3;1u\x1b[?1002h\x1b[?1006h\x1b[?1004h");

    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        var focus = try runtime.prepareAttachmentSemanticInput(
            attachment_id,
            .{ .focus = .{ .focused = true } },
            std.math.maxInt(usize),
        );
        _ = try focus.commit();
    }

    const key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{ .control = true },
        .text = "a",
        .unshifted_codepoint = 'a',
    };
    const mouse: subject.MouseInput = .{
        .action = .{ .press = .left },
        .modifiers = .{ .shift = true },
        .pressed_buttons = .{ .left = true },
        .x = 24,
        .y = 32,
    };
    for ([_]protocol.SemanticInput{
        .{ .key = key },
        .{ .mouse = mouse },
    }) |input| {
        var prepared = try runtime.prepareAttachmentSemanticInput(
            second_attachment,
            input,
            std.math.maxInt(usize),
        );
        _ = try prepared.commit();
    }

    var first_repair = try runtime.prepareAttachmentInputRepair(first_attachment);
    defer first_repair.deinit();
    try std.testing.expectEqual(@as(usize, 0), first_repair.byteCount());
    _ = try first_repair.commit();

    var expected: std.ArrayList(u8) = .empty;
    defer expected.deinit(std.testing.allocator);
    var key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = key.composing,
        .consumed_modifiers = key.consumed_modifiers,
        .key = key.key,
        .modifiers = key.modifiers,
        .text = key.text,
        .unshifted_codepoint = key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected.appendSlice(std.testing.allocator, key_release.bytes.?);
    key_release.deinit();
    var mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = mouse.modifiers,
        .pressed_buttons = .{},
        .x = mouse.x,
        .y = mouse.y,
    }, std.math.maxInt(usize));
    try expected.appendSlice(std.testing.allocator, mouse_release.bytes.?);
    mouse_release.deinit();
    try expected.appendSlice(std.testing.allocator, "\x1b[O");

    var second_repair = try runtime.prepareAttachmentInputRepair(second_attachment);
    defer second_repair.deinit();
    try std.testing.expectEqualSlices(u8, expected.items, second_repair.bytes.?);
    _ = try second_repair.commit();
    try std.testing.expectEqual(
        @as(usize, 0),
        inputQueueSnapshot(runtime).repair_charge.bytes,
    );
    try runtime.forceTerminate();
}
test "attachment input reset repairs ownership, retains the ledger, and rejects stale input" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x25} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();
    parseTerminalInput(runtime, "\x1b[=3;1u\x1b[?1002h\x1b[?1006h\x1b[?1004h");

    const key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{ .control = true },
        .text = "a",
        .unshifted_codepoint = 'a',
    };
    const mouse: subject.MouseInput = .{
        .action = .{ .press = .left },
        .modifiers = .{ .shift = true },
        .pressed_buttons = .{ .left = true },
        .x = 24,
        .y = 32,
    };
    for ([_]protocol.SemanticInput{
        .{ .key = key },
        .{ .mouse = mouse },
        .{ .focus = .{ .focused = true } },
    }) |input| {
        var prepared = try runtime.prepareAttachmentSemanticInput(
            attachment_id,
            input,
            std.math.maxInt(usize),
        );
        _ = try prepared.commit();
    }

    var stale = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyB",
            .modifiers = .{},
            .text = "b",
            .unshifted_codepoint = 'b',
        } },
        std.math.maxInt(usize),
    );
    defer stale.deinit();

    var expected: std.ArrayList(u8) = .empty;
    defer expected.deinit(std.testing.allocator);
    var key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = key.composing,
        .consumed_modifiers = key.consumed_modifiers,
        .key = key.key,
        .modifiers = key.modifiers,
        .text = key.text,
        .unshifted_codepoint = key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected.appendSlice(std.testing.allocator, key_release.bytes.?);
    key_release.deinit();
    var mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = mouse.modifiers,
        .pressed_buttons = .{},
        .x = mouse.x,
        .y = mouse.y,
    }, std.math.maxInt(usize));
    try expected.appendSlice(std.testing.allocator, mouse_release.bytes.?);
    mouse_release.deinit();
    try expected.appendSlice(std.testing.allocator, "\x1b[O");

    var reset = try runtime.prepareAttachmentInputReset(attachment_id);
    defer reset.deinit();
    try std.testing.expectEqualSlices(u8, expected.items, reset.bytes.?);
    _ = try reset.commit();
    try std.testing.expect(subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(!subject.testing.attachmentFocusClaim(
        runtime,
        attachment_id,
    ));
    const reset_snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), reset_snapshot.repair_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), reset_snapshot.repair_charge.entries);
    try std.testing.expectError(error.MissingAttachment, stale.commit());

    var fresh = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .text = .{ .text = "fresh" } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 5), try fresh.commit());
    try runtime.forceTerminate();
}
test "paste encoding requires the reviewed bracketed mode under the terminal lock" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();

    try std.testing.expectError(
        error.PasteModeChanged,
        runtime.preparePasteInput("payload", true, std.math.maxInt(usize)),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    parseTerminalInput(runtime, "\x1b[?2004h");
    try std.testing.expectError(
        error.PasteModeChanged,
        runtime.preparePasteInput("payload", false, std.math.maxInt(usize)),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var prepared = try runtime.preparePasteInput("payload", true, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[200~payload\x1b[201~", prepared.bytes.?);
    prepared.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "attachment detach repairs only its accepted key and mouse ownership" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    const first_attachment = AttachmentId.fromBytes([_]u8{0x31} ** 16);
    const second_attachment = AttachmentId.fromBytes([_]u8{0x32} ** 16);
    for ([_]AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        var attachment = try runtime.prepareAttachment(
            attachment_id,
            .{ .input = true, .read = false, .resize = false },
        );
        attachment.commit();
    }
    parseTerminalInput(runtime, "\x1b[=3;1u\x1b[?1002h\x1b[?1006h");

    const first_key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{ .control = true },
        .text = "a",
        .unshifted_codepoint = 'a',
    };
    const second_key: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyB",
        .modifiers = .{ .shift = true },
        .text = "B",
        .unshifted_codepoint = 'b',
    };
    const first_mouse: subject.MouseInput = .{
        .action = .{ .press = .left },
        .modifiers = .{ .control = true },
        .pressed_buttons = .{ .left = true },
        .x = 24,
        .y = 32,
    };
    const second_mouse: subject.MouseInput = .{
        .action = .{ .press = .right },
        .modifiers = .{ .shift = true },
        .pressed_buttons = .{ .right = true },
        .x = 40,
        .y = 48,
    };
    for ([_]struct { AttachmentId, protocol.SemanticInput }{
        .{ first_attachment, .{ .key = first_key } },
        .{ first_attachment, .{ .mouse = first_mouse } },
        .{ second_attachment, .{ .key = second_key } },
        .{ second_attachment, .{ .mouse = second_mouse } },
        // Wheel presses are stateless and must not become detach ownership.
        .{ first_attachment, .{ .mouse = .{
            .action = .{ .press = .four },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 24,
            .y = 32,
        } } },
    }) |entry| {
        var prepared = try runtime.prepareAttachmentSemanticInput(
            entry[0],
            entry[1],
            std.math.maxInt(usize),
        );
        _ = try prepared.commit();
    }

    var expected_first: std.ArrayList(u8) = .empty;
    defer expected_first.deinit(std.testing.allocator);
    var first_key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = first_key.composing,
        .consumed_modifiers = first_key.consumed_modifiers,
        .key = first_key.key,
        .modifiers = first_key.modifiers,
        .text = first_key.text,
        .unshifted_codepoint = first_key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected_first.appendSlice(std.testing.allocator, first_key_release.bytes.?);
    first_key_release.deinit();
    var first_mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = first_mouse.modifiers,
        .pressed_buttons = .{},
        .x = first_mouse.x,
        .y = first_mouse.y,
    }, std.math.maxInt(usize));
    try expected_first.appendSlice(std.testing.allocator, first_mouse_release.bytes.?);
    first_mouse_release.deinit();

    var first_repair = try runtime.prepareAttachmentInputRepair(first_attachment);
    defer first_repair.deinit();
    try std.testing.expectEqualSlices(u8, expected_first.items, first_repair.bytes.?);
    _ = try first_repair.commit();
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        first_attachment,
    ));
    try std.testing.expect(subject.testing.hasAttachmentInputOwnership(
        runtime,
        second_attachment,
    ));

    var expected_second: std.ArrayList(u8) = .empty;
    defer expected_second.deinit(std.testing.allocator);
    var second_key_release = try runtime.prepareKeyInput(.{
        .action = .release,
        .composing = second_key.composing,
        .consumed_modifiers = second_key.consumed_modifiers,
        .key = second_key.key,
        .modifiers = second_key.modifiers,
        .text = second_key.text,
        .unshifted_codepoint = second_key.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try expected_second.appendSlice(std.testing.allocator, second_key_release.bytes.?);
    second_key_release.deinit();
    var second_mouse_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .right },
        .modifiers = second_mouse.modifiers,
        .pressed_buttons = .{},
        .x = second_mouse.x,
        .y = second_mouse.y,
    }, std.math.maxInt(usize));
    try expected_second.appendSlice(std.testing.allocator, second_mouse_release.bytes.?);
    second_mouse_release.deinit();

    var second_repair = try runtime.prepareAttachmentInputRepair(second_attachment);
    defer second_repair.deinit();
    try std.testing.expectEqualSlices(u8, expected_second.items, second_repair.bytes.?);
    _ = try second_repair.commit();
    try runtime.forceTerminate();
}
test "prepared attachment input cannot commit after its owner detaches" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x33} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();

    var prepared = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyA",
            .modifiers = .{},
            .text = "a",
            .unshifted_codepoint = 'a',
        } },
        std.math.maxInt(usize),
    );
    defer prepared.deinit();

    runtime.detachInput(attachment_id);
    try std.testing.expectError(error.MissingAttachment, prepared.commit());
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "attachment detach repair uses its precharged input capacity" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }
    const attachment_id = AttachmentId.fromBytes([_]u8{0x34} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();
    parseTerminalInput(runtime, "\x1b[=3;1u");

    var press = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyA",
            .modifiers = .{},
            .text = "a",
            .unshifted_codepoint = 'a',
        } },
        std.math.maxInt(usize),
    );
    _ = try press.commit();
    probe.copied.wait();

    const queued_before_fill = inputQueueSnapshot(runtime).client_charge.bytes;
    const fill_size = subject.testing.pending_input_limit -
        subject.max_key_encoded_bytes - queued_before_fill;
    const fill = try std.testing.allocator.alloc(u8, fill_size);
    defer std.testing.allocator.free(fill);
    @memset(fill, 'x');
    var prepared_fill = try runtime.prepareRawInput(fill);
    _ = try prepared_fill.commit();
    try std.testing.expectError(error.InputBackpressure, runtime.prepareRawInput("x"));

    var repair = try runtime.prepareAttachmentInputRepair(attachment_id);
    defer repair.deinit();
    try std.testing.expect(repair.bytes.?.len > 0);
    _ = try repair.commit();
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    probe.copied.release();
    try runtime.forceTerminate();
}
test "final aggregate focus release uses its precharged input capacity" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }
    const attachment_id = AttachmentId.fromBytes([_]u8{0x36} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();
    parseTerminalInput(runtime, "\x1b[?1004h");

    var claim = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 3), try claim.commit());
    probe.copied.wait();

    const queued_before_fill = inputQueueSnapshot(runtime).client_charge.bytes;
    const fill_size = subject.testing.pending_input_limit -
        subject.max_focus_encoded_bytes - queued_before_fill;
    const fill = try std.testing.allocator.alloc(u8, fill_size);
    defer std.testing.allocator.free(fill);
    @memset(fill, 'x');
    var prepared_fill = try runtime.prepareRawInput(fill);
    _ = try prepared_fill.commit();
    try std.testing.expectError(error.InputBackpressure, runtime.prepareRawInput("x"));

    var release = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .focus = .{ .focused = false } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqualSlices(u8, "\x1b[O", release.bytes.?);
    _ = try release.commit();
    try std.testing.expectEqual(
        @as(usize, 0),
        inputQueueSnapshot(runtime).repair_charge.bytes,
    );
    probe.copied.release();
    try runtime.forceTerminate();
}
test "attachment detach repair allocation failure fences the live runtime" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    const attachment_id = AttachmentId.fromBytes([_]u8{0x35} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();

    // The disabled mouse mode admits no bytes but still owns the accepted
    // held semantic state. Enabling tracking afterward makes detach encoding
    // allocate without leaving an ordinary client entry for the writer.
    var press = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .mouse = .{
            .action = .{ .press = .left },
            .modifiers = .{},
            .pressed_buttons = .{ .left = true },
            .x = 1,
            .y = 1,
        } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 0), try press.commit());
    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");

    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    const runtime_allocator = runtimeState(runtime).allocator;
    runtimeState(runtime).allocator = failing.allocator();
    runtime.detachInput(attachment_id);
    runtimeState(runtime).allocator = runtime_allocator;

    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(
        subject.RuntimeFailureKind.worker_integrity,
        runtime.primaryFailure().?,
    );
    try std.testing.expect(!subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
}
test "attachment input reset allocation failure fences and clears retained ownership" {
    var containment_probe: ContainmentProbe = .{ .pause_first_attempt = false };
    const runtime = try createRuntimeWithHooks(containment_probe.hooks());
    defer {
        clearTestingHooks(runtime);
        destroyRuntimeWithIdentityRetry(runtime);
    }
    const attachment_id = AttachmentId.fromBytes([_]u8{0x37} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();

    var claim = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 0), try claim.commit());
    parseTerminalInput(runtime, "\x1b[?1004h");

    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    const runtime_allocator = runtimeState(runtime).allocator;
    runtimeState(runtime).allocator = failing.allocator();
    runtime.resetInput(attachment_id);
    runtimeState(runtime).allocator = runtime_allocator;

    try std.testing.expect(subject.testing.failureFenced(runtime));
    try std.testing.expectEqual(
        subject.RuntimeFailureKind.worker_integrity,
        runtime.primaryFailure().?,
    );
    try std.testing.expect(subject.testing.hasAttachmentInputOwnership(
        runtime,
        attachment_id,
    ));
    try std.testing.expect(!subject.testing.attachmentFocusClaim(
        runtime,
        attachment_id,
    ));
    const reset_snapshot = inputQueueSnapshot(runtime);
    try std.testing.expectEqual(@as(usize, 0), reset_snapshot.repair_charge.bytes);
    try std.testing.expectEqual(@as(usize, 0), reset_snapshot.repair_charge.entries);
}
test "finite mouse coordinates remain bounded across tracking modes and formats" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();

    const maximum_finite: f32 = @bitCast(@as(u32, 0x7f7fffff));
    parseTerminalInput(runtime, "\x1b[?1000h");
    var boundary_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[M#p!", boundary_release.bytes.?);
    boundary_release.deinit();

    const clear_modes =
        "\x1b[?1000l\x1b[?1002l\x1b[?1003l" ++
        "\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";
    const tracking_modes = [_][]const u8{
        "\x1b[?1000h",
        "\x1b[?1002h",
        "\x1b[?1003h",
    };
    const formats = [_][]const u8{
        "",
        "\x1b[?1005h",
        "\x1b[?1006h",
        "\x1b[?1015h",
        "\x1b[?1016h",
    };
    const positions = [_][2]f32{
        .{ maximum_finite, 0 },
        .{ -maximum_finite, 0 },
        .{ 0, maximum_finite },
        .{ 0, -maximum_finite },
    };
    for (tracking_modes) |tracking_mode| {
        for (formats) |format| {
            parseTerminalInput(runtime, clear_modes);
            parseTerminalInput(runtime, tracking_mode);
            parseTerminalInput(runtime, format);
            for (positions) |position| {
                var prepared = try runtime.prepareMouseInput(.{
                    .action = .{ .release = .left },
                    .modifiers = .{},
                    .pressed_buttons = .{},
                    .x = position[0],
                    .y = position[1],
                }, std.math.maxInt(usize));
                try std.testing.expect(prepared.byteCount() > 0);
                try std.testing.expect(prepared.byteCount() <= subject.max_mouse_encoded_bytes);
                prepared.deinit();
            }
        }
    }

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1000h");
    var outside_press = try runtime.prepareMouseInput(.{
        .action = .{ .press = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqual(@as(usize, 0), outside_press.byteCount());
    outside_press.deinit();

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1002h\x1b[?1006h");
    var outside_drag = try runtime.prepareMouseInput(.{
        .action = .{ .motion = .{ .drag = .left } },
        .modifiers = .{},
        .pressed_buttons = .{ .left = true },
        .x = maximum_finite,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[<32;80;1M", outside_drag.bytes.?);
    outside_drag.deinit();

    parseTerminalInput(runtime, clear_modes ++ "\x1b[?1000h\x1b[?1016h");
    var representable_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 650,
        .y = 10,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;650;10m",
        representable_pixels.bytes.?,
    );
    representable_pixels.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "finite mouse normalization is safe at maximum legal pixel geometry" {
    const runtime = try createIdleRuntimeWithGeometry(17, 17, .{
        .cell_pixel_height = 3_855,
        .cell_pixel_width = 3_855,
    });
    defer runtime.destroy();
    const maximum_finite: f32 = @bitCast(@as(u32, 0x7f7fffff));

    parseTerminalInput(runtime, "\x1b[?1000h");
    var grid_release = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1b[M#11", grid_release.bytes.?);
    grid_release.deinit();

    parseTerminalInput(runtime, "\x1b[?1016h");
    var positive_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = maximum_finite,
        .y = maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;2147483520;2147483520m",
        positive_pixels.bytes.?,
    );
    positive_pixels.deinit();

    var negative_pixels = try runtime.prepareMouseInput(.{
        .action = .{ .release = .left },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = -maximum_finite,
        .y = -maximum_finite,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(
        u8,
        "\x1b[<0;-2147483648;-2147483648m",
        negative_pixels.bytes.?,
    );
    negative_pixels.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "maximum key text uses the negotiated encoded-byte limit" {
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    parseTerminalInput(runtime, "\x1b[>24u");

    const a_text = [_]u8{'a'} ** 4096;
    const a_input: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{},
        .text = &a_text,
        .unshifted_codepoint = 'a',
    };
    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.prepareKeyInput(a_input, 12_293),
    );
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var prepared_a = try runtime.prepareKeyInput(a_input, 12_294);
    try std.testing.expectEqual(@as(usize, 12_294), prepared_a.byteCount());
    try std.testing.expectEqual(@as(usize, 12_294), runtime.pendingInputBytes());
    prepared_a.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    const d_text = [_]u8{'d'} ** 4096;
    var prepared_d = try runtime.prepareKeyInput(.{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyD",
        .modifiers = .{},
        .text = &d_text,
        .unshifted_codepoint = 'd',
    }, 16_391);
    try std.testing.expectEqual(@as(usize, 16_391), prepared_d.byteCount());
    prepared_d.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());

    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = 0 },
    );
    const runtime_allocator = runtimeState(runtime).allocator;
    runtimeState(runtime).allocator = failing.allocator();
    const allocation_result = runtime.prepareKeyInput(a_input, 12_294);
    runtimeState(runtime).allocator = runtime_allocator;
    try std.testing.expectError(error.OutOfMemory, allocation_result);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "macOS Option-as-Alt intent uses authoritative terminal encoding modes" {
    const builtin = @import("builtin");
    if (builtin.os.tag != .macos) return error.SkipZigTest;
    const runtime = try createIdleRuntime(.{});
    defer runtime.destroy();
    parseTerminalInput(runtime, "\x1b[?1036h");

    const base: subject.KeyInput = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyB",
        .modifiers = .{ .alt = true },
        .text = "∫",
        .unshifted_codepoint = 'b',
    };
    var unicode = try runtime.prepareKeyInput(base, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "∫", unicode.bytes.?);
    unicode.deinit();

    var terminal_alt = try runtime.prepareKeyInput(.{
        .action = base.action,
        .composing = base.composing,
        .consumed_modifiers = base.consumed_modifiers,
        .key = base.key,
        .modifiers = base.modifiers,
        .option_as_alt = true,
        .text = base.text,
        .unshifted_codepoint = base.unshifted_codepoint,
    }, std.math.maxInt(usize));
    try std.testing.expectEqualSlices(u8, "\x1bb", terminal_alt.bytes.?);
    terminal_alt.deinit();
    try runtime.forceTerminate();
}
test "every admitted mouse button has a normal SGR encoding" {
    const runtime = try createIdleRuntime(.{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
    });
    defer runtime.destroy();
    parseTerminalInput(runtime, "\x1b[?1000h\x1b[?1006h");

    const cases = [_]struct {
        button: protocol.MouseButton,
        expected: []const u8,
    }{
        .{ .button = .left, .expected = "\x1b[<0;1;1M" },
        .{ .button = .right, .expected = "\x1b[<2;1;1M" },
        .{ .button = .middle, .expected = "\x1b[<1;1;1M" },
        .{ .button = .four, .expected = "\x1b[<64;1;1M" },
        .{ .button = .five, .expected = "\x1b[<65;1;1M" },
        .{ .button = .six, .expected = "\x1b[<66;1;1M" },
        .{ .button = .seven, .expected = "\x1b[<67;1;1M" },
        .{ .button = .eight, .expected = "\x1b[<128;1;1M" },
        .{ .button = .nine, .expected = "\x1b[<129;1;1M" },
        .{ .button = .ten, .expected = "\x1b[<130;1;1M" },
        .{ .button = .eleven, .expected = "\x1b[<131;1;1M" },
    };
    for (cases) |case| {
        var prepared = try runtime.prepareMouseInput(.{
            .action = .{ .press = case.button },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 0,
            .y = 0,
        }, std.math.maxInt(usize));
        try std.testing.expectEqualSlices(u8, case.expected, prepared.bytes.?);
        prepared.deinit();
    }

    const clear_modes =
        "\x1b[?1000l\x1b[?1002l\x1b[?1003l" ++
        "\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";
    const tracking_modes = [_][]const u8{
        "\x1b[?1000h",
        "\x1b[?1002h",
        "\x1b[?1003h",
    };
    const formats = [_][]const u8{
        "",
        "\x1b[?1005h",
        "\x1b[?1006h",
        "\x1b[?1015h",
        "\x1b[?1016h",
    };
    for (tracking_modes) |tracking_mode| {
        for (formats) |format| {
            parseTerminalInput(runtime, clear_modes);
            parseTerminalInput(runtime, tracking_mode);
            parseTerminalInput(runtime, format);
            for ([_]protocol.MouseButton{ .ten, .eleven }) |button| {
                var prepared = try runtime.prepareMouseInput(.{
                    .action = .{ .press = button },
                    .modifiers = .{},
                    .pressed_buttons = .{},
                    .x = 0,
                    .y = 0,
                }, std.math.maxInt(usize));
                try std.testing.expect(prepared.byteCount() > 0);
                prepared.deinit();
            }
        }
    }

    parseTerminalInput(runtime, clear_modes);
    var suppressed = try runtime.prepareMouseInput(.{
        .action = .{ .press = .ten },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 0,
        .y = 0,
    }, std.math.maxInt(usize));
    try std.testing.expectEqual(@as(usize, 0), suppressed.byteCount());
    suppressed.deinit();
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "input queue bounds entry metadata independently of byte charge" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 30" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 31 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    var reservations: [max_pending_input_entries]InputReservation = undefined;
    for (&reservations) |*reservation| {
        reservation.* = try reserveInput(runtime, 1, false, .client);
    }
    try std.testing.expectError(
        error.InputBackpressure,
        reserveInput(runtime, 1, false, .client),
    );
    for (reservations) |reservation| cancelInput(runtime, reservation.identifier);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    try runtime.forceTerminate();
}
test "ordered input reservations prevent overtaking and cancellation leaves no gap" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try pty_process.environmentVariableOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "read value; printf '<%s>' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =^<!value!^>",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    var writer_observation: semantic_wait.GenerationSignal = .{};
    const runtime = try createRuntimeWithHooksAndCommand(.{
        .context = &writer_observation,
        .writer_waiting_for_input = semantic_wait.GenerationSignal.notify,
    }, executable, arguments);
    defer runtime.destroy();

    const first = try reserveInput(runtime, 1, false, .client);
    const cancelled = try reserveInput(runtime, 1, false, .client);
    const third = try reserveInput(runtime, 2, false, .client);
    try publishInput(runtime, third, try std.testing.allocator.dupe(u8, if (builtin.os.tag == .windows) "C\r" else "C\n"));
    cancelInput(runtime, cancelled.identifier);
    try semantic_wait.writerQueueObservation(runtime, &writer_observation);
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    try publishInput(runtime, first, try std.testing.allocator.dupe(u8, "A"));

    _ = try runtime.waitForCompletion();
    try runtime.shutdown();
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "<AC>") != null);
}

test "linefeed mode expands carriage returns before semantic and reply admission" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[20h"),
    );

    var text = try runtime.prepareTextInput("a\rb", 4);
    defer text.deinit();
    try std.testing.expectEqual(@as(usize, 4), text.byteCount());
    try std.testing.expectEqualStrings("a\r\nb", text.bytes.?);

    try std.testing.expectError(
        error.InputLimitExceeded,
        runtime.prepareTextInput("a\rb", 3),
    );

    var raw = try runtime.prepareRawInput("\r");
    defer raw.deinit();
    try std.testing.expectEqualStrings("\r\n", raw.bytes.?);

    var paste = try runtime.preparePasteInput("a\rb", false, 4);
    defer paste.deinit();
    try std.testing.expectEqualStrings("a\r\nb", paste.bytes.?);

    const runtime_state = runtimeState(runtime);
    runtime_state.terminal_mutex.lock();
    const terminal_handler = &runtime_state.stream.handler.terminal_handler;
    terminal_handler.effects.write_pty.?(terminal_handler, "a\rb");
    runtime_state.terminal_mutex.unlock();
    probe.copied.wait();
    const reply = try expectNextQueuedTerminalBytes(runtime, "a\r\nb");
    cancelInput(runtime, reply);
    probe.copied.release();
    try runtime.forceTerminate();
}

test "focus mode reports the aggregate immediately and rejects stale preparation" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }
    const attachment_id = AttachmentId.fromBytes([_]u8{0x71} ** 16);
    var attachment = try runtime.prepareAttachment(
        attachment_id,
        .{ .input = true, .read = false, .resize = false },
    );
    attachment.commit();

    var claim = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .focus = .{ .focused = true } },
        std.math.maxInt(usize),
    );
    try std.testing.expectEqual(@as(usize, 0), try claim.commit());

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?1004h"),
    );
    const first_reply = try expectNextQueuedTerminalBytes(runtime, "\x1b[I");
    cancelInput(runtime, first_reply);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?1004l"),
    );
    var stale = try runtime.prepareAttachmentSemanticInput(
        attachment_id,
        .{ .focus = .{ .focused = false } },
        std.math.maxInt(usize),
    );
    defer stale.deinit();
    try std.testing.expectEqual(@as(usize, 0), stale.byteCount());

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?1004h"),
    );
    const second_reply = try expectNextQueuedTerminalBytes(runtime, "\x1b[I");
    cancelInput(runtime, second_reply);
    try std.testing.expectError(error.FocusModeChanged, stale.commit());
    try std.testing.expect(subject.testing.attachmentFocusClaim(runtime, attachment_id));
}

test "mouse input epoch rejects stale preparation and retires only unstarted hover" {
    {
        const runtime = try createIdleRuntime(.{});
        defer runtime.destroy();
        var stale = try runtime.prepareMouseInput(.{
            .action = .{ .motion = .hover },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 0,
            .y = 0,
        }, std.math.maxInt(usize));
        defer stale.deinit();
        try std.testing.expectEqual(@as(usize, 0), stale.byteCount());
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?1003h\x1b[?1006h"),
        );
        try std.testing.expectError(error.MouseModeChanged, stale.commit());
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    }

    {
        const runtime = try createIdleRuntime(.{});
        defer runtime.destroy();
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?1003h\x1b[?1006h"),
        );
        var stale = try runtime.prepareMouseInput(.{
            .action = .{ .motion = .hover },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 0,
            .y = 0,
        }, std.math.maxInt(usize));
        defer stale.deinit();
        try std.testing.expect(stale.byteCount() != 0);
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?1003l"),
        );
        try std.testing.expectError(error.MouseModeChanged, stale.commit());
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    }

    {
        var probe: WriterProbe = .{ .pause_at = .copied };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            probe.copied.release();
            clearTestingHooks(runtime);
            runtime.destroy();
        }
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?1003h\x1b[?1006h"),
        );
        try std.testing.expect((try runtime.enqueueMouseInput(.{
            .action = .{ .motion = .hover },
            .modifiers = .{},
            .pressed_buttons = .{},
            .x = 0,
            .y = 0,
        })) != 0);
        probe.copied.wait();
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?1003l"),
        );
        try std.testing.expectEqual(@as(usize, 1), try runtime.enqueueTextInput("x"));
        probe.copied.release();
        probe.retired.wait();
        probe.accounted.wait();
        const counts = probe.counts();
        try std.testing.expectEqual(@as(usize, 1), counts.calls);
        try std.testing.expectEqual(@as(usize, 1), counts.bytes);
        try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
    }
}

test "started mouse hover completes across a later mode change" {
    var probe: WriterProbe = .{ .pause_at = .started };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.started.release();
        clearTestingHooks(runtime);
        runtime.destroy();
    }
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?1003h\x1b[?1006h"),
    );
    var hover = try runtime.prepareMouseInput(.{
        .action = .{ .motion = .hover },
        .modifiers = .{},
        .pressed_buttons = .{},
        .x = 0,
        .y = 0,
    }, std.math.maxInt(usize));
    defer hover.deinit();
    const hover_bytes = hover.byteCount();
    try std.testing.expect(hover_bytes != 0);
    try std.testing.expectEqual(hover_bytes, try hover.commit());
    probe.started.wait();
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?1003l"),
    );
    probe.started.release();
    probe.accounted.wait();
    const counts = probe.counts();
    try std.testing.expectEqual(@as(usize, 1), counts.calls);
    try std.testing.expectEqual(hover_bytes, counts.bytes);
    try std.testing.expectEqual(@as(usize, 0), runtime.pendingInputBytes());
}

test "confirmed local prompt clears orphaned mouse tracking without crossing process authority" {
    const cases = [_]struct {
        observation: ActiveCommandObservation,
        expect_enabled: bool,
    }{
        .{ .observation = .inactive, .expect_enabled = false },
        .{ .observation = .active, .expect_enabled = true },
        .{ .observation = .failed, .expect_enabled = true },
    };
    for ([_][]const u8{ "\x1b]133;A\x07", "\x1b]133;P\x07" }) |prompt_marker| {
        for (cases) |case| {
            var probe: ActiveCommandProbe = .{ .observation = case.observation };
            const runtime = try createRuntimeWithHooks(probe.hooks());
            defer {
                clearTestingHooks(runtime);
                runtime.destroy();
            }
            try std.testing.expectEqual(
                ReaderBatchOutcome.committed,
                commitReaderBatch(runtime, "\x1b[?1003h\x1b[?1006h"),
            );
            try std.testing.expectEqual(
                ReaderBatchOutcome.committed,
                commitReaderBatch(runtime, prompt_marker),
            );
            try expectMouseTrackingEnabled(runtime, case.expect_enabled);
            try std.testing.expectEqual(@as(usize, 1), probe.calls);
        }
    }

    var probe: ActiveCommandProbe = .{ .observation = .inactive };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        clearTestingHooks(runtime);
        runtime.destroy();
    }
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(
            runtime,
            "\x1b[?1003h\x1b[?1006h\x1b]133;A\x07\x1b[?1003h\x1b[?1006h",
        ),
    );
    try expectMouseTrackingEnabled(runtime, true);
}

test "in-band size mode reports enable and both geometry commit paths" {
    var probe: WriterProbe = .{ .pause_at = .copied };
    const runtime = try createRuntimeWithHooks(probe.hooks());
    defer {
        probe.copied.release();
        runtime.destroy();
    }

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2048h"),
    );
    var expected: [128]u8 = undefined;
    const initial_reply = try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    );
    cancelInput(runtime, initial_reply);

    _ = try runtime.resizeGeometry(100, 30, 9, 18);
    const attachment_reply = try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    );
    cancelInput(runtime, attachment_reply);

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?40h\x1b[?3h"),
    );
    try std.testing.expectEqual(@as(u16, 132), runtime.geometry().columns);
    const terminal_reply = try expectNextQueuedTerminalBytes(
        runtime,
        try inBandSizeReport(&expected, runtime.geometry()),
    );
    cancelInput(runtime, terminal_reply);

    const unchanged = runtime.geometry();
    _ = try runtime.resizeGeometry(
        unchanged.columns,
        unchanged.rows,
        unchanged.cell_pixel_width,
        unchanged.cell_pixel_height,
    );
    try std.testing.expectEqual(
        @as(usize, 0),
        inputQueueSnapshot(runtime).terminal_reply_charge.bytes,
    );

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2048l"),
    );
    _ = try runtime.resizeGeometry(120, 35, 10, 20);
    try std.testing.expectEqual(
        @as(usize, 0),
        inputQueueSnapshot(runtime).terminal_reply_charge.bytes,
    );
}

test "mode-effect reply exhaustion fences reader and attachment resize commits" {
    {
        const runtime = try createIdleRuntime(.{});
        defer runtime.destroy();
        _ = try reserveInput(
            runtime,
            max_pending_terminal_reply_bytes,
            false,
            .terminal,
        );
        try std.testing.expectEqual(
            ReaderBatchOutcome.runtime_failed,
            commitReaderBatch(runtime, "\x1b[?2048h"),
        );
        try std.testing.expectEqual(
            subject.RuntimeFailureKind.terminal_reply,
            runtime.primaryFailure().?,
        );
        const runtime_state = runtimeState(runtime);
        runtime_state.terminal_mutex.lock();
        defer runtime_state.terminal_mutex.unlock();
        try std.testing.expect(runtime_state.terminal.modes.get(.in_band_size_reports));
    }

    {
        var probe: WriterProbe = .{ .pause_at = .copied };
        const runtime = try createRuntimeWithHooks(probe.hooks());
        defer {
            probe.copied.release();
            runtime.destroy();
        }
        try std.testing.expectEqual(
            ReaderBatchOutcome.committed,
            commitReaderBatch(runtime, "\x1b[?2048h"),
        );
        var expected: [128]u8 = undefined;
        const enable_reply = try expectNextQueuedTerminalBytes(
            runtime,
            try inBandSizeReport(&expected, runtime.geometry()),
        );
        cancelInput(runtime, enable_reply);
        _ = try reserveInput(
            runtime,
            max_pending_terminal_reply_bytes,
            false,
            .terminal,
        );
        try std.testing.expectError(
            error.TerminalRuntimeUnavailable,
            runtime.resizeGeometry(90, 25, 8, 16),
        );
        try std.testing.expectEqual(
            subject.RuntimeFailureKind.terminal_reply,
            runtime.primaryFailure().?,
        );
        const retained = runtime.geometry();
        try std.testing.expectEqual(@as(u16, 90), retained.columns);
        try std.testing.expectEqual(@as(u16, 25), retained.rows);
    }
}

fn expectNextQueuedTerminalBytes(
    runtime: *TerminalRuntime,
    expected: []const u8,
) !u64 {
    var buffer: [256]u8 = undefined;
    const runtime_state = runtimeState(runtime);
    runtime_state.input_mutex.lock();
    defer runtime_state.input_mutex.unlock();
    if (!runtime_state.input_queue.hasWritableEntry()) return error.ExpectedQueuedInput;
    const slice = runtime_state.input_queue.copyNextWriterSlice(&buffer);
    try std.testing.expectEqual(InputSource.terminal, slice.source);
    try std.testing.expectEqualStrings(expected, buffer[0..slice.length]);
    return slice.identifier;
}

fn inBandSizeReport(buffer: []u8, geometry: anytype) ![]const u8 {
    return std.fmt.bufPrint(buffer, "\x1b[48;{d};{d};{d};{d}t", .{
        geometry.rows,
        geometry.columns,
        @as(u64, geometry.rows) * geometry.cell_pixel_height,
        @as(u64, geometry.columns) * geometry.cell_pixel_width,
    });
}

const ActiveCommandObservation = enum {
    active,
    failed,
    inactive,
};

const ActiveCommandProbe = struct {
    calls: usize = 0,
    observation: ActiveCommandObservation,

    fn hooks(self: *ActiveCommandProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .has_active_command = hasActiveCommand,
        };
    }

    fn hasActiveCommand(context: *anyopaque) anyerror!bool {
        const self: *ActiveCommandProbe = @ptrCast(@alignCast(context));
        self.calls += 1;
        return switch (self.observation) {
            .active => true,
            .inactive => false,
            .failed => error.InjectedProcessObservationFailure,
        };
    }
};

fn expectMouseTrackingEnabled(runtime: *TerminalRuntime, enabled: bool) !void {
    const runtime_state = runtimeState(runtime);
    runtime_state.terminal_mutex.lock();
    defer runtime_state.terminal_mutex.unlock();
    try std.testing.expectEqual(enabled, runtime_state.terminal.modes.get(.mouse_event_any));
    try std.testing.expectEqual(enabled, runtime_state.terminal.modes.get(.mouse_format_sgr));
    if (enabled) {
        try std.testing.expectEqual(.any, runtime_state.terminal.flags.mouse_event);
        try std.testing.expectEqual(.sgr, runtime_state.terminal.flags.mouse_format);
    } else {
        try std.testing.expectEqual(.none, runtime_state.terminal.flags.mouse_event);
        try std.testing.expectEqual(.x10, runtime_state.terminal.flags.mouse_format);
    }
}
