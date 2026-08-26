const harness = @import("runtime_aggregate_harness.zig");
const std = harness.std;
const subject = harness.subject;

test "scrollback compression opportunities are globally throttled fair and burst-free" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const testing = subject.testing;
    const interval = testing.scrollbackCompressionOpportunityIntervalNs;

    try std.testing.expectEqual(
        @as(?usize, null),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 0, 0),
    );
    try std.testing.expectEqual(
        @as(?usize, 0),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, 0),
    );
    try std.testing.expectEqual(@as(usize, 1), testing.compressionScanIndex(&aggregate));
    try std.testing.expectEqual(interval, testing.nextCompressionOpportunityAt(&aggregate));
    try std.testing.expectEqual(
        @as(?usize, null),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, interval - 1),
    );
    try std.testing.expectEqual(
        @as(?usize, 1),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, interval),
    );
    try std.testing.expectEqual(
        @as(?usize, 2),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, 100 * interval),
    );
    try std.testing.expectEqual(
        @as(?usize, null),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, 100 * interval),
    );
    try std.testing.expectEqual(
        @as(?usize, 0),
        testing.takeScrollbackCompressionIndexFn(&aggregate, 3, 101 * interval),
    );
}
