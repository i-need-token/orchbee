const std = @import("std");
const target_builtin = @import("builtin");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
const subject = service_boundary.testing.projection;
const terminal_launch = engine_boundary.launch;
const terminal_runtime = engine_boundary.runtime;

const MessageKind = protocol.MessageKind;
const host_outgoing = service_boundary.testing.outgoing;
const Outgoing = host_outgoing.Outgoing;

const projection_interval_ns: u64 = 16 * std.time.ns_per_ms;
const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

fn testIdentifier(namespace: u8, index: usize) [16]u8 {
    var result = [_]u8{0} ** 16;
    result[0] = namespace;
    std.mem.writeInt(u64, result[8..16], @intCast(index + 1), .little);
    return result;
}

fn testId(comptime Id: type, namespace: u8, index: usize) Id {
    return Id.fromBytes(testIdentifier(namespace, index));
}

fn testLaunch(arguments: []const []const u8) terminal_launch.Launch {
    return .{
        .arguments = arguments,
        .configuration = null,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .geometry = .{
            .cell_pixel_height = 16,
            .cell_pixel_width = 8,
            .columns = 80,
            .rows = 24,
        },
        .profile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &features,
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &palette,
            .version_response = "Orchbee projection scheduler test",
        },
    };
}

const RuntimeFixture = struct {
    runtime: *terminal_runtime.TerminalRuntime,
    terminal_id: core_module.TerminalId,
};

fn createRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
) !RuntimeFixture {
    const arguments = [_][]const u8{ "-c", "read value" };
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        testLaunch(&arguments),
    );
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x41, 0),
    );
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x42, 0),
        validated,
        .{},
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = created.identity().terminal_id;
    return .{
        .runtime = aggregate.runtimeFor(terminal_id).?,
        .terminal_id = terminal_id,
    };
}

fn activateAttachments(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    terminal_id: core_module.TerminalId,
    attachment_ids: []const core_module.AttachmentId,
) !void {
    for (attachment_ids) |attachment_id| {
        _ = try aggregate.attach(
            connection_id,
            attachment_id,
            terminal_id,
            .{ .input = false, .read = true, .resize = false },
            false,
        );
        _ = try aggregate.setProjectionDemand(
            connection_id,
            attachment_id,
            .active,
        );
    }
}

fn resizeSynchronously(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    terminal_id: core_module.TerminalId,
    runtime: *terminal_runtime.TerminalRuntime,
) !void {
    const before = runtime.geometry();
    const columns: u16 = if (before.columns == 80) 81 else 80;
    const revision = try runtime.resizeGeometry(
        columns,
        before.rows,
        before.cell_pixel_width,
        before.cell_pixel_height,
    );
    const after = runtime.geometry();
    try aggregate.core().synchronizeGeometry(
        terminal_id,
        after.columns,
        after.rows,
        after.cell_pixel_width,
        after.cell_pixel_height,
        revision,
    );
}

fn onlyProjectionFrame(outgoing: *const Outgoing) !protocol.FrameSummary {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expect(
        envelope.kind == MessageKind.snapshot_frame or
            envelope.kind == MessageKind.delta_frame,
    );
    return protocol.validateTerminalFrame(std.testing.allocator, envelope.payload);
}

fn acknowledgeFrame(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    frame: protocol.FrameSummary,
) !void {
    _ = try aggregate.core().frameApplied(
        connection_id,
        core_module.AttachmentId.fromBytes(frame.attachment_id),
        frame.sequence,
        frame.revision,
        frame.generation,
    );
}

fn fillOutgoingRecordLimit(
    outgoing: *Outgoing,
    host_epoch: core_module.HostEpoch,
) !void {
    for (0..host_outgoing.testing.record_limit) |_| {
        switch (try outgoing.append(
            MessageKind.terminal_started,
            .control_json,
            "{}",
            .{ .event = host_epoch },
        )) {
            .appended => {},
            .full => return error.ExpectedFillerAppend,
        }
    }
}

test "a full dispatch batch preserves its committed projection prefix" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x31, 0),
    );

    const first_terminal = testId(core_module.TerminalId, 0x32, 0);
    const second_terminal = testId(core_module.TerminalId, 0x32, 1);
    const first_attachment = testId(core_module.AttachmentId, 0x33, 0);
    const second_attachment = testId(core_module.AttachmentId, 0x33, 1);
    const terminal_ids = [_]core_module.TerminalId{ first_terminal, second_terminal };
    const attachment_ids = [_]core_module.AttachmentId{ first_attachment, second_attachment };
    for (terminal_ids, attachment_ids, 0..) |terminal_id, attachment_id, index| {
        var launch_digest = [_]u8{0} ** 32;
        launch_digest[0] = @intCast(index + 1);
        _ = try aggregate.core().createTerminal(
            connection_id,
            testId(core_module.CreationId, 0x34, index),
            terminal_id,
            launch_digest,
            80,
            24,
        );
        _ = try aggregate.core().attach(connection_id, attachment_id, terminal_id, .{
            .input = false,
            .read = true,
            .resize = false,
        }, false);
        _ = try aggregate.core().setProjectionDemand(
            connection_id,
            attachment_id,
            .active,
        );
    }

    const first_allocation = try std.testing.allocator.alloc(
        u8,
        host_outgoing.owned_payload_prefix_bytes + protocol.Limits.frame_bytes,
    );
    var first_encoded: terminal_runtime.ProjectionFrame = .{
        .allocation = first_allocation,
        .base_revision = null,
        .bytes = first_allocation[host_outgoing.owned_payload_prefix_bytes..],
        .generation = 1,
        .kind = .snapshot,
        .revision = 1,
        .sequence = 1,
    };
    @memset(first_encoded.bytes, 0);
    defer if (first_encoded.allocation.len != 0) {
        std.testing.allocator.free(first_encoded.allocation);
    };
    const second_allocation = try std.testing.allocator.alloc(
        u8,
        host_outgoing.owned_payload_prefix_bytes + protocol.Limits.frame_bytes,
    );
    var second_encoded: terminal_runtime.ProjectionFrame = .{
        .allocation = second_allocation,
        .base_revision = null,
        .bytes = second_allocation[host_outgoing.owned_payload_prefix_bytes..],
        .generation = 1,
        .kind = .snapshot,
        .revision = 1,
        .sequence = 1,
    };
    @memset(second_encoded.bytes, 0);
    defer if (second_encoded.allocation.len != 0) {
        std.testing.allocator.free(second_encoded.allocation);
    };
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    const appendProjectionFrame = subject.testing.appendProjectionFrameFn;

    switch (try appendProjectionFrame(
        &aggregate,
        &outgoing,
        first_attachment,
        MessageKind.snapshot_frame,
        &first_encoded,
    )) {
        .appended => {},
        .batch_full => return error.ExpectedProjectionAppend,
    }
    switch (try appendProjectionFrame(
        &aggregate,
        &outgoing,
        second_attachment,
        MessageKind.snapshot_frame,
        &second_encoded,
    )) {
        .appended => return error.ExpectedFullBatch,
        .batch_full => {},
    }
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expect(
        aggregate.core().findAttachment(first_attachment).?.projectionInFlight() != null,
    );
    try std.testing.expect(
        aggregate.core().findAttachment(second_attachment).?.projectionInFlight() == null,
    );
    try std.testing.expectEqual(
        @as(u64, 1),
        aggregate.core().findAttachment(second_attachment).?.nextProjectionSequence(),
    );
}

test "same-terminal projection scheduling alternates after synchronous resize" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    const attachment_ids = [_]core_module.AttachmentId{
        testId(core_module.AttachmentId, 0x51, 0),
        testId(core_module.AttachmentId, 0x51, 1),
    };
    try activateAttachments(
        &aggregate,
        connection_id,
        fixture.terminal_id,
        &attachment_ids,
    );

    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, 2 * std.time.ns_per_hour, 0);
    var first = Outgoing.init(std.testing.allocator);
    defer first.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            protocol.Limits.frame_bytes,
            &first,
        ),
    );
    const first_frame = try onlyProjectionFrame(&first);
    try std.testing.expectEqualSlices(
        u8,
        &testIdentifier(0x51, 0),
        &first_frame.attachment_id,
    );
    try acknowledgeFrame(&aggregate, connection_id, first_frame);
    try resizeSynchronously(&aggregate, fixture.terminal_id, fixture.runtime);

    subject.testing.setClockSampleFn(
        &scheduler,
        2 * std.time.ns_per_hour,
        projection_interval_ns,
    );
    var second = Outgoing.init(std.testing.allocator);
    defer second.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            protocol.Limits.frame_bytes,
            &second,
        ),
    );
    const second_frame = try onlyProjectionFrame(&second);
    try std.testing.expectEqualSlices(
        u8,
        &testIdentifier(0x51, 1),
        &second_frame.attachment_id,
    );
    try std.testing.expectEqual(@as(usize, 2), subject.testing.clockSampleCountFn(&scheduler));
}

test "128 continuously eligible same-terminal attachments emit in 128 polls" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    var attachment_ids: [128]core_module.AttachmentId = undefined;
    for (&attachment_ids, 0..) |*attachment_id, index| {
        attachment_id.* = testId(core_module.AttachmentId, 0x52, index);
    }
    try activateAttachments(
        &aggregate,
        connection_id,
        fixture.terminal_id,
        &attachment_ids,
    );

    var scheduler: subject.Scheduler = .{};
    var emitted = [_]bool{false} ** attachment_ids.len;
    for (0..attachment_ids.len) |poll| {
        subject.testing.setClockSampleFn(
            &scheduler,
            3 * std.time.ns_per_hour,
            @as(u64, @intCast(poll)) * projection_interval_ns,
        );
        var outgoing = Outgoing.init(std.testing.allocator);
        defer outgoing.deinit();
        try std.testing.expectEqual(
            host_outgoing.ScheduleOutcome.complete,
            try scheduler.schedule(
                std.testing.allocator,
                &aggregate,
                connection_id,
                protocol.Limits.frame_bytes,
                &outgoing,
            ),
        );
        const frame = try onlyProjectionFrame(&outgoing);
        try std.testing.expectEqualSlices(
            u8,
            &testIdentifier(0x52, poll),
            &frame.attachment_id,
        );
        try std.testing.expect(!emitted[poll]);
        emitted[poll] = true;
        try acknowledgeFrame(&aggregate, connection_id, frame);
        try resizeSynchronously(&aggregate, fixture.terminal_id, fixture.runtime);
    }
    for (emitted) |was_emitted| try std.testing.expect(was_emitted);
    try std.testing.expectEqual(
        attachment_ids.len,
        subject.testing.clockSampleCountFn(&scheduler),
    );
}

test "full batch retries the exact unsent projection without advancing delivery" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    const attachment_ids = [_]core_module.AttachmentId{
        testId(core_module.AttachmentId, 0x53, 0),
        testId(core_module.AttachmentId, 0x53, 1),
    };
    try activateAttachments(
        &aggregate,
        connection_id,
        fixture.terminal_id,
        &attachment_ids,
    );

    const geometry = fixture.runtime.geometry();
    const expected = try fixture.runtime.encodeProjectionFrameOwned(
        std.testing.allocator,
        host_outgoing.owned_payload_prefix_bytes,
        .{
            .attachment_id = attachment_ids[0],
            .base_generation = null,
            .base_revision = null,
            .force_snapshot = true,
            .host_epoch = aggregate.core().epoch,
            .maximum_bytes = protocol.Limits.frame_bytes,
            .sequence = 1,
        },
    );
    defer std.testing.allocator.free(expected.allocation);
    try std.testing.expectEqual(geometry.generation, expected.generation);
    try std.testing.expectEqual(geometry.revision, expected.revision);

    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, std.time.ns_per_hour, 0);
    var full = Outgoing.init(std.testing.allocator);
    defer full.deinit();
    try fillOutgoingRecordLimit(&full, aggregate.core().epoch);
    const deadline_before = aggregate.nextProjectionAt(fixture.terminal_id).?;
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            protocol.Limits.frame_bytes,
            &full,
        ),
    );
    try std.testing.expectEqual(host_outgoing.testing.record_limit, full.records().len);
    try std.testing.expectEqual(@as(usize, 0), subject.testing.scanIndexFn(&scheduler));
    try std.testing.expectEqual(deadline_before, aggregate.nextProjectionAt(fixture.terminal_id).?);
    for (attachment_ids) |attachment_id| {
        const attachment = aggregate.core().findAttachment(attachment_id).?;
        try std.testing.expect(attachment.projectionInFlight() == null);
        try std.testing.expect(attachment.projectionReady() != null);
        try std.testing.expectEqual(@as(u64, 1), attachment.nextProjectionSequence());
    }

    subject.testing.setClockSampleFn(&scheduler, 0, 0);
    var retry = Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            protocol.Limits.frame_bytes,
            &retry,
        ),
    );
    const envelope = try protocol.validateTransportEnvelope(retry.records()[0]);
    const retried_frame = try protocol.validateTerminalFrame(
        std.testing.allocator,
        envelope.payload,
    );
    try std.testing.expectEqualSlices(
        u8,
        &testIdentifier(0x53, 0),
        &retried_frame.attachment_id,
    );
    try std.testing.expectEqual(@as(u64, 1), retried_frame.sequence);
    try std.testing.expectEqualSlices(u8, expected.bytes, envelope.payload);
}

test "projection cadence uses the exact monotonic boundary across wall-clock rollback" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    const attachment_id = testId(core_module.AttachmentId, 0x54, 0);
    try activateAttachments(
        &aggregate,
        connection_id,
        fixture.terminal_id,
        &.{attachment_id},
    );

    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, std.time.ns_per_hour, 0);
    var initial = Outgoing.init(std.testing.allocator);
    defer initial.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        protocol.Limits.frame_bytes,
        &initial,
    );
    const initial_frame = try onlyProjectionFrame(&initial);
    try acknowledgeFrame(&aggregate, connection_id, initial_frame);
    try resizeSynchronously(&aggregate, fixture.terminal_id, fixture.runtime);
    try std.testing.expectEqual(
        @as(i128, projection_interval_ns),
        aggregate.nextProjectionAt(fixture.terminal_id).?,
    );

    subject.testing.setClockSampleFn(&scheduler, 0, projection_interval_ns - 1);
    var before_boundary = Outgoing.init(std.testing.allocator);
    defer before_boundary.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        protocol.Limits.frame_bytes,
        &before_boundary,
    );
    try std.testing.expectEqual(@as(usize, 0), before_boundary.records().len);
    try std.testing.expectEqual(
        @as(i128, projection_interval_ns),
        aggregate.nextProjectionAt(fixture.terminal_id).?,
    );

    subject.testing.setClockSampleFn(
        &scheduler,
        -std.time.ns_per_hour,
        projection_interval_ns,
    );
    var at_boundary = Outgoing.init(std.testing.allocator);
    defer at_boundary.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        protocol.Limits.frame_bytes,
        &at_boundary,
    );
    _ = try onlyProjectionFrame(&at_boundary);
    try std.testing.expectEqual(@as(usize, 3), subject.testing.clockSampleCountFn(&scheduler));
}

test "timer initialization failure leaves scheduler delivery deadline and outgoing unchanged" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    const attachment_ids = [_]core_module.AttachmentId{
        testId(core_module.AttachmentId, 0x55, 0),
        testId(core_module.AttachmentId, 0x55, 1),
    };
    try activateAttachments(
        &aggregate,
        connection_id,
        fixture.terminal_id,
        &attachment_ids,
    );
    const flight = try aggregate.core().commitFrame(
        attachment_ids[0],
        null,
        0,
        fixture.runtime.geometry().generation,
        .snapshot,
    );
    const deadline_before: i128 = 123_456_789;
    try std.testing.expect(aggregate.setNextProjectionAt(fixture.terminal_id, deadline_before));

    var scheduler: subject.Scheduler = .{};
    subject.testing.failTimerInitializationFn(&scheduler);
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    switch (try outgoing.append(
        MessageKind.terminal_started,
        .control_json,
        "{}",
        .{ .event = aggregate.core().epoch },
    )) {
        .appended => {},
        .full => return error.ExpectedFillerAppend,
    }
    const existing_record = outgoing.records()[0];

    try std.testing.expectError(
        error.TimerUnsupported,
        scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            protocol.Limits.frame_bytes,
            &outgoing,
        ),
    );
    try std.testing.expect(!subject.testing.timerInitializedFn(&scheduler));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.clockSampleCountFn(&scheduler));
    try std.testing.expectEqual(@as(usize, 0), subject.testing.scanIndexFn(&scheduler));
    try std.testing.expectEqual(deadline_before, aggregate.nextProjectionAt(fixture.terminal_id).?);
    const first = aggregate.core().findAttachment(attachment_ids[0]).?;
    try std.testing.expectEqualDeep(flight, first.projectionInFlight().?.frame);
    try std.testing.expectEqual(@as(u64, 2), first.nextProjectionSequence());
    const second = aggregate.core().findAttachment(attachment_ids[1]).?;
    try std.testing.expect(second.projectionInFlight() == null);
    try std.testing.expect(second.projectionReady() != null);
    try std.testing.expectEqual(@as(u64, 1), second.nextProjectionSequence());
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try std.testing.expectEqual(existing_record.ptr, outgoing.records()[0].ptr);
    try std.testing.expectEqualSlices(u8, existing_record, outgoing.records()[0]);
}

test "empty projection scheduler completes without producing a batch" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    var scheduler: subject.Scheduler = .{};
    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            1,
            protocol.Limits.frame_bytes,
            &outgoing,
        ),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}
