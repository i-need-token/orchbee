const std = @import("std");
const harness = @import("service_harness.zig");
const aggregate_harness = @import("runtime_aggregate_harness.zig");
const schedule = @import("deterministic_schedule.zig");
const control_encode = harness.control_boundary.encode;
const protocol = harness.protocol;
const runtime_module = harness.terminal_runtime;
const Outgoing = harness.Outgoing;
const ApplicationSignalScheduler = harness.service_boundary.testing.ApplicationSignalSchedulerType;
const MetadataScheduler = harness.service_boundary.testing.MetadataSchedulerType;

const correlation_apc = "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\";
const launch_apc = "\x1b_orchbee.agent.v1;eyJraW5kIjoibGF1bmNoIiwidmVyc2lvbiI6MX0\x1b\\";
const correlation_payload = "{\"kind\":\"correlation\",\"version\":1}";
const launch_payload = "{\"kind\":\"launch\",\"version\":1}";

// Only scheduler-owned allocations fail. Runtime creation and retained source
// events use the stable testing allocator. The reader stays parked until
// teardown, so native startup output cannot change the sampled metadata.
const Fixture = struct {
    service: harness.HostService,
    connection: harness.ConnectionState = .awaiting_hello,
    readers: [2]schedule.Gate = .{ .{}, .{} },

    fn connect(self: *Fixture) !void {
        try harness.testHandshake(&self.service, &self.connection, "72727272727272727272727272727272");
    }

    fn connectionId(self: *Fixture) !u64 {
        return (try harness.activeConnection(&self.connection)).connection_id;
    }

    fn aggregate(self: *Fixture) *harness.aggregate_module.HostRuntimeAggregate {
        return harness.serviceAggregate(&self.service);
    }

    fn addRuntime(self: *Fixture, index: usize) !*runtime_module.TerminalRuntime {
        var launch = try aggregate_harness.longRunningProcessLaunch();
        defer launch.deinit();
        var spawner: aggregate_harness.HookedThreadSpawner = .{ .hooks = .{
            .context = &self.readers[index],
            .read_pty = parkedRead,
        } };
        const runtime = try runtime_module.testing.createWithOptionsUsingSpawnerFn(
            std.testing.allocator,
            launch.launch,
            .{ .cell_pixel_height = 16, .cell_pixel_width = 8 },
            &spawner,
        );
        errdefer {
            self.readers[index].release();
            runtime.destroy();
        }
        const aggregate_owner = self.aggregate();
        try aggregate_owner.core().reserveTerminal();
        try harness.aggregate_module.testing.adoptRunningTerminalFn(
            aggregate_owner,
            try self.connectionId(),
            harness.testId(harness.core_module.CreationId, 0x74, index),
            harness.testId(harness.core_module.TerminalId, 0x75, index),
            [_]u8{0x76} ** 32,
            80,
            24,
            8,
            16,
            .{},
            runtime,
        );
        return runtime;
    }

    fn deinit(self: *Fixture) void {
        for (&self.readers) |*reader| reader.release();
        self.service.disconnect(&self.connection);
        self.service.deinitChecked() catch |err| {
            std.debug.panic("event fixture cleanup failed: {s}", .{@errorName(err)});
        };
    }

    fn parkedRead(context: *anyopaque, _: []u8) anyerror!usize {
        const reader: *schedule.Gate = @ptrCast(@alignCast(context));
        reader.pause();
        return 0;
    }
};

test "application signal encoding failure releases its clone and preserves the queue head" {
    var fixture: Fixture = .{ .service = .init(std.testing.allocator) };
    defer fixture.deinit();
    try fixture.connect();
    const runtime = try fixture.addRuntime(0);
    try commitBytes(runtime, correlation_apc);

    var counter = std.testing.FailingAllocator.init(std.testing.allocator, .{});
    var sample = (try runtime.prepareApplicationSignal(counter.allocator())).?;
    sample.deinit();
    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{
        .fail_index = counter.alloc_index,
        .resize_fail_index = counter.resize_index,
    });
    var scheduler: ApplicationSignalScheduler = .{};
    {
        var failed = Outgoing.init(std.testing.allocator);
        defer failed.deinit();
        try std.testing.expectError(error.WriteFailed, scheduler.schedule(
            failing.allocator(),
            fixture.aggregate(),
            try fixture.connectionId(),
            &failed,
        ));
        try std.testing.expect(failing.has_induced_failure);
        try std.testing.expectEqual(@as(usize, 0), failed.records().len);
    }
    try expectSignal(runtime, 1, correlation_payload);

    var retry = Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    try std.testing.expectEqual(.complete, try scheduler.schedule(
        std.testing.allocator,
        fixture.aggregate(),
        try fixture.connectionId(),
        &retry,
    ));
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    try expectNoSignal(runtime);
}

test "application signal append failure discards the prefix without consuming either queue" {
    var fixture: Fixture = .{ .service = .init(std.testing.allocator) };
    defer fixture.deinit();
    try fixture.connect();
    const first = try fixture.addRuntime(0);
    const second = try fixture.addRuntime(1);
    try commitBytes(first, correlation_apc ++ launch_apc);
    try commitBytes(second, correlation_apc);
    const aggregate = fixture.aggregate();
    const core = aggregate.core();
    var first_event = (try first.prepareApplicationSignal(std.testing.allocator)).?;
    defer first_event.deinit();
    const first_terminal = core.terminalValues()[0];
    const first_payload = try control_encode.terminalApplicationSignal(
        std.testing.allocator,
        core.epoch,
        first_terminal.creation_id,
        first_terminal.terminal_id,
        first_event,
    );
    defer std.testing.allocator.free(first_payload);
    var second_event = (try second.prepareApplicationSignal(std.testing.allocator)).?;
    defer second_event.deinit();
    const second_terminal = core.terminalValues()[1];
    const second_payload = try control_encode.terminalApplicationSignal(
        std.testing.allocator,
        core.epoch,
        second_terminal.creation_id,
        second_terminal.terminal_id,
        second_event,
    );
    defer std.testing.allocator.free(second_payload);

    const failure_point = try secondAppendFailurePoint(
        core.epoch,
        protocol.MessageKind.terminal_application_signal,
        first_payload,
        second_payload,
    );
    var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{
        .fail_index = failure_point.alloc,
        .resize_fail_index = failure_point.resize,
    });
    var scheduler: ApplicationSignalScheduler = .{};
    {
        // The real caller destroys the whole partial batch on error. It must
        // never be reused or written before the fresh retry below.
        var failed = Outgoing.init(failing.allocator());
        defer failed.deinit();
        try std.testing.expectError(error.OutOfMemory, scheduler.schedule(
            std.testing.allocator,
            aggregate,
            try fixture.connectionId(),
            &failed,
        ));
        try std.testing.expect(failing.has_induced_failure);
        try std.testing.expectEqual(@as(usize, 1), failed.records().len);
        const envelope = try protocol.validateTransportEnvelope(failed.records()[0]);
        try std.testing.expectEqualStrings(first_payload, envelope.payload);
    }
    try expectSignal(first, 1, correlation_payload);
    try expectSignal(second, 1, correlation_payload);
    {
        var full = Outgoing.init(std.testing.allocator);
        defer full.deinit();
        try fillBatch(&full, core.epoch);
        try std.testing.expectEqual(.batch_full, try scheduler.schedule(
            std.testing.allocator,
            aggregate,
            try fixture.connectionId(),
            &full,
        ));
    }
    try expectSignal(first, 1, correlation_payload);
    try expectSignal(second, 1, correlation_payload);
    {
        var retry = Outgoing.init(std.testing.allocator);
        defer retry.deinit();
        _ = try scheduler.schedule(std.testing.allocator, aggregate, try fixture.connectionId(), &retry);
        try std.testing.expectEqual(@as(usize, 2), retry.records().len);
        const first_record = try protocol.validateTransportEnvelope(retry.records()[0]);
        const second_record = try protocol.validateTransportEnvelope(retry.records()[1]);
        try std.testing.expectEqualStrings(first_payload, first_record.payload);
        try std.testing.expectEqualStrings(second_payload, second_record.payload);
    }
    try expectSignal(first, 2, launch_payload);
    try expectNoSignal(second);
    {
        var next = Outgoing.init(std.testing.allocator);
        defer next.deinit();
        _ = try scheduler.schedule(std.testing.allocator, aggregate, try fixture.connectionId(), &next);
        try std.testing.expectEqual(@as(usize, 1), next.records().len);
    }
    try expectNoSignal(first);
    var empty = Outgoing.init(std.testing.allocator);
    defer empty.deinit();
    _ = try scheduler.schedule(std.testing.allocator, aggregate, try fixture.connectionId(), &empty);
    try std.testing.expectEqual(@as(usize, 0), empty.records().len);
}

test "metadata delivery releases failed clones and commits only a successful fresh batch" {
    var fixture: Fixture = .{ .service = .init(std.testing.allocator) };
    defer fixture.deinit();
    try fixture.connect();
    const runtime = try fixture.addRuntime(0);
    const second = try fixture.addRuntime(1);
    const connection_id = try fixture.connectionId();
    const metadata_bytes = "\x1b]2;Allocation title\x1b\\\x1b]7;file:///allocation-fixture\x1b\\";
    try commitBytes(runtime, metadata_bytes);
    try commitBytes(second, metadata_bytes);
    const revision = runtime.geometry().revision;
    const second_revision = second.geometry().revision;

    var counter = std.testing.FailingAllocator.init(std.testing.allocator, .{});
    var sample = (try runtime.prepareMetadataEvent(counter.allocator(), connection_id)).?;
    sample.deinit();
    var scheduler: MetadataScheduler = .{};
    {
        var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{
            .fail_index = counter.alloc_index,
            .resize_fail_index = counter.resize_index,
        });
        var failed = Outgoing.init(std.testing.allocator);
        defer failed.deinit();
        try std.testing.expectError(error.WriteFailed, scheduler.schedule(
            failing.allocator(),
            fixture.aggregate(),
            connection_id,
            &failed,
        ));
        try std.testing.expect(failing.has_induced_failure);
        try std.testing.expectEqual(@as(usize, 0), failed.records().len);
    }
    try expectMetadata(runtime, connection_id, revision);
    try expectMetadata(second, connection_id, second_revision);
    const core = fixture.aggregate().core();
    const first_payload = try metadataPayload(runtime, connection_id, core, 0);
    defer std.testing.allocator.free(first_payload);
    const second_payload = try metadataPayload(second, connection_id, core, 1);
    defer std.testing.allocator.free(second_payload);
    const failure_point = try secondAppendFailurePoint(
        core.epoch,
        protocol.MessageKind.terminal_metadata_changed,
        first_payload,
        second_payload,
    );
    {
        var failing = std.testing.FailingAllocator.init(std.testing.allocator, .{
            .fail_index = failure_point.alloc,
            .resize_fail_index = failure_point.resize,
        });
        var failed = Outgoing.init(failing.allocator());
        defer failed.deinit();
        try std.testing.expectError(error.OutOfMemory, scheduler.schedule(
            std.testing.allocator,
            fixture.aggregate(),
            connection_id,
            &failed,
        ));
        try std.testing.expect(failing.has_induced_failure);
        try std.testing.expectEqual(@as(usize, 1), failed.records().len);
        const envelope = try protocol.validateTransportEnvelope(failed.records()[0]);
        try std.testing.expectEqualStrings(first_payload, envelope.payload);
    }
    try expectMetadata(runtime, connection_id, revision);
    try expectMetadata(second, connection_id, second_revision);
    {
        var full = Outgoing.init(std.testing.allocator);
        defer full.deinit();
        try fillBatch(&full, fixture.aggregate().core().epoch);
        try std.testing.expectEqual(.batch_full, try scheduler.schedule(
            std.testing.allocator,
            fixture.aggregate(),
            connection_id,
            &full,
        ));
    }
    try expectMetadata(runtime, connection_id, revision);
    try expectMetadata(second, connection_id, second_revision);
    {
        var retry = Outgoing.init(std.testing.allocator);
        defer retry.deinit();
        try std.testing.expectEqual(.complete, try scheduler.schedule(
            std.testing.allocator,
            fixture.aggregate(),
            connection_id,
            &retry,
        ));
        try std.testing.expectEqual(@as(usize, 2), retry.records().len);
        for (retry.records(), [_][]const u8{ first_payload, second_payload }) |record, expected| {
            const envelope = try protocol.validateTransportEnvelope(record);
            try std.testing.expectEqual(protocol.MessageKind.terminal_metadata_changed, envelope.kind);
            try std.testing.expectEqualStrings(expected, envelope.payload);
        }
    }
    for ([_]*runtime_module.TerminalRuntime{ runtime, second }) |source| {
        var duplicate = try source.prepareMetadataEvent(std.testing.allocator, connection_id);
        defer if (duplicate) |*event| event.deinit();
        try std.testing.expect(duplicate == null);
    }
    var empty = Outgoing.init(std.testing.allocator);
    defer empty.deinit();
    _ = try scheduler.schedule(std.testing.allocator, fixture.aggregate(), connection_id, &empty);
    try std.testing.expectEqual(@as(usize, 0), empty.records().len);
}

fn commitBytes(runtime: *runtime_module.TerminalRuntime, bytes: []const u8) !void {
    try std.testing.expectEqual(.committed, harness.commitReaderBatch(runtime, bytes));
}

fn expectSignal(runtime: *runtime_module.TerminalRuntime, sequence: u64, payload: []const u8) !void {
    var event = (try runtime.prepareApplicationSignal(std.testing.allocator)) orelse return error.ExpectedSignal;
    defer event.deinit();
    try std.testing.expectEqual(sequence, event.sequence);
    try std.testing.expectEqualStrings(payload, event.payload);
}

fn expectNoSignal(runtime: *runtime_module.TerminalRuntime) !void {
    var event = try runtime.prepareApplicationSignal(std.testing.allocator);
    defer if (event) |*present| present.deinit();
    try std.testing.expect(event == null);
}

fn expectMetadata(runtime: *runtime_module.TerminalRuntime, connection_id: u64, revision: u64) !void {
    var event = (try runtime.prepareMetadataEvent(std.testing.allocator, connection_id)) orelse return error.ExpectedMetadata;
    defer event.deinit();
    try std.testing.expectEqual(revision, event.revision);
    try std.testing.expectEqualStrings("Allocation title", event.title.?);
    try std.testing.expectEqualStrings("file:///allocation-fixture", event.current_directory.?);
}

fn metadataPayload(
    runtime: *runtime_module.TerminalRuntime,
    connection_id: u64,
    core: *harness.core_module.HostCore,
    index: usize,
) ![]u8 {
    var event = (try runtime.prepareMetadataEvent(std.testing.allocator, connection_id)).?;
    defer event.deinit();
    const terminal = core.terminalValues()[index];
    return control_encode.terminalMetadataChanged(
        std.testing.allocator,
        core.epoch,
        terminal.creation_id,
        terminal.terminal_id,
        event,
    );
}

const AppendFailurePoint = struct { alloc: usize, resize: usize };

fn secondAppendFailurePoint(
    epoch: harness.core_module.HostEpoch,
    kind: u16,
    first: []const u8,
    second: []const u8,
) !AppendFailurePoint {
    var counter = std.testing.FailingAllocator.init(std.testing.allocator, .{});
    var sample = Outgoing.init(counter.allocator());
    defer sample.deinit();
    try std.testing.expectEqual(.appended, std.meta.activeTag(try sample.append(
        kind,
        .control_json,
        first,
        .{ .event = epoch },
    )));
    const point: AppendFailurePoint = .{ .alloc = counter.alloc_index, .resize = counter.resize_index };
    try std.testing.expectEqual(.appended, std.meta.activeTag(try sample.append(
        kind,
        .control_json,
        second,
        .{ .event = epoch },
    )));
    try std.testing.expect(counter.alloc_index > point.alloc or counter.resize_index > point.resize);
    return point;
}

fn fillBatch(outgoing: *Outgoing, epoch: harness.core_module.HostEpoch) !void {
    for (0..harness.host_outgoing.testing.record_limit) |_| {
        try std.testing.expectEqual(.appended, std.meta.activeTag(try outgoing.append(
            protocol.MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = epoch },
        )));
    }
}
