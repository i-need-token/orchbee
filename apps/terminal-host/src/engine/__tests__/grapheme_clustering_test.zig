const harness = @import("runtime_harness.zig");
const std = harness.std;
const ReaderBatchOutcome = harness.ReaderBatchOutcome;
const cancelInput = harness.cancelInput;
const commitReaderBatch = harness.commitReaderBatch;
const createIdleRuntimeWithGeometryAndHooks = harness.createIdleRuntimeWithGeometryAndHooks;
const expectNextQueuedTerminalBytes = harness.expectNextQueuedTerminalBytes;
const GatedPtyProbe = harness.GatedPtyProbe;
const WriterProbe = harness.WriterProbe;

test "grapheme clustering is on by default, DECRST 2027 turns it off, and DECRQM reports the state" {
    var probe: GatedPtyProbe(WriterProbe) = .{ .probe = .{ .pause_at = .copied } };
    const runtime = try createIdleRuntimeWithGeometryAndHooks(80, 24, .{}, probe.hooks());
    probe.runtime = runtime;
    defer {
        probe.reader.release();
        probe.probe.copied.release();
        runtime.destroy();
    }
    probe.reader.wait();

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2027$p"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?2027;1$y"));

    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1b[?2027l\x1b[?2027$p"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?2027;2$y"));

    // A program's full reset returns the mode to its default, which is on.
    try std.testing.expectEqual(
        ReaderBatchOutcome.committed,
        commitReaderBatch(runtime, "\x1bc\x1b[?2027$p"),
    );
    cancelInput(runtime, try expectNextQueuedTerminalBytes(runtime, "\x1b[?2027;1$y"));
}
