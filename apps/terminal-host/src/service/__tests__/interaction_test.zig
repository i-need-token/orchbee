const std = @import("std");
const target_builtin = @import("builtin");
const control_boundary = @import("terminal-host-control");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const control_encode = control_boundary.encode;
const host_outgoing = service_boundary.testing.outgoing;
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
const subject = service_boundary.testing.interaction;
const terminal_launch = engine_boundary.launch;
const terminal_runtime = engine_boundary.runtime;

const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
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
            .version_response = "Orchbee interaction test",
        },
    };
}

const TestRuntime = struct {
    runtime: *terminal_runtime.TerminalRuntime,
};

fn createReadableRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) !TestRuntime {
    const arguments = [_][]const u8{ "-c", "read value" };
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        testLaunch(&arguments),
    );
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x30),
    );
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x31),
        validated,
        .{},
    );
    defer preparation.deinit();
    const created = try aggregate.commitPreparedTerminal(&preparation);
    const terminal_id = created.identity().terminal_id;
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    return .{
        .runtime = aggregate.runtimeFor(terminal_id).?,
    };
}

fn fillOutgoingLeaving(
    outgoing: *host_outgoing.Outgoing,
    host_epoch: core_module.HostEpoch,
    remaining_bytes: usize,
) !void {
    const header_bytes = protocol.transport_envelope_header_bytes;
    const first_payload_bytes = protocol.Limits.frame_bytes;
    const first_record_bytes = header_bytes + first_payload_bytes;
    const second_payload_bytes = host_outgoing.testing.byte_limit -
        remaining_bytes - first_record_bytes - header_bytes;
    const payload = try std.testing.allocator.alloc(u8, first_payload_bytes);
    defer std.testing.allocator.free(payload);
    @memset(payload, 0);
    switch (try outgoing.append(
        protocol.MessageKind.snapshot_frame,
        .terminal_frame,
        payload,
        .{ .event = host_epoch },
    )) {
        .appended => {},
        .full => return error.ExpectedFillerAppend,
    }
    switch (try outgoing.append(
        protocol.MessageKind.snapshot_frame,
        .terminal_frame,
        payload[0..second_payload_bytes],
        .{ .event = host_epoch },
    )) {
        .appended => {},
        .full => return error.ExpectedFillerAppend,
    }
}

test "readable interaction capability distinguishes authorization from runtime ownership" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const terminal_id = testId(core_module.TerminalId, 0x22);
    const readable_attachment_id = testId(core_module.AttachmentId, 0x23);
    const input_attachment_id = testId(core_module.AttachmentId, 0x24);
    _ = try aggregate.core().connect(
        connection_id,
        testId(core_module.ClientId, 0x20),
    );
    _ = try aggregate.core().createTerminal(
        connection_id,
        testId(core_module.CreationId, 0x21),
        terminal_id,
        [_]u8{0x25} ** 32,
        80,
        24,
    );
    _ = try aggregate.core().attach(
        connection_id,
        readable_attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    _ = try aggregate.core().attach(
        connection_id,
        input_attachment_id,
        terminal_id,
        .{ .input = true, .read = false, .resize = false },
        false,
    );

    const controller = subject.Controller.init(&aggregate);
    try std.testing.expectError(
        error.TerminalRuntimeMissing,
        controller.readableAttachment(connection_id, readable_attachment_id),
    );
    try std.testing.expectError(
        error.ReadableAttachmentUnavailable,
        controller.readableAttachment(connection_id, input_attachment_id),
    );
    try std.testing.expectError(
        error.ReadableAttachmentUnavailable,
        controller.readableAttachment(connection_id + 1, readable_attachment_id),
    );

    _ = try aggregate.core().detach(connection_id, readable_attachment_id);
    _ = try aggregate.core().detach(connection_id, input_attachment_id);
    _ = try aggregate.core().childExited(terminal_id, .{ .exit_code = 0 });
    _ = try aggregate.core().ptyDrained(terminal_id);
    _ = try aggregate.core().disposeTerminal(connection_id, terminal_id);
    _ = aggregate.disconnect(connection_id);
}

test "empty interaction scheduler completes without producing a batch" {
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(false) catch unreachable;
    var scheduler: subject.Scheduler = .{};
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();

    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(std.testing.allocator, &aggregate, 1, &outgoing),
    );
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
}

test "readable capability binds identity and provisional export rollback restores sequence" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x32);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    const interaction = try subject.Controller.init(&aggregate).readableAttachment(
        connection_id,
        attachment_id,
    );
    const geometry = binding.runtime.geometry();
    _ = try interaction.applySelectionCommand(.{
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });

    var provisional = try interaction.startSelectionExport(7, .plain);
    try std.testing.expectEqual(@as(u64, 7), provisional.result().started.export_sequence);
    provisional.deinit();

    var retried = try interaction.startSelectionExport(7, .plain);
    try std.testing.expectEqual(@as(u64, 7), retried.result().started.export_sequence);
    retried.commit();
    retried.deinit();
    _ = try interaction.cancelSelectionExport(7);
}

test "batch full commits the interaction prefix and retries the current attachment" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x42);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    const geometry = binding.runtime.geometry();
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );

    var scheduler: subject.Scheduler = .{};
    var first = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first.deinit();
    try fillOutgoingLeaving(&first, aggregate.core().epoch, 256);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &first,
        ),
    );
    try std.testing.expectEqual(@as(usize, 3), first.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(first.records()[2])).kind,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.selection_changed,
        (try protocol.validateTransportEnvelope(retry.records()[0])).kind,
    );
}

test "allocator failure rolls back every earlier interaction mutation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x52);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    const geometry = binding.runtime.geometry();
    const sampled_history = (try binding.runtime.takeHistoryEvent(attachment_id)).?;
    var allocation_counter = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{},
    );
    const counting_allocator = allocation_counter.allocator();
    const sampled_payload = try control_encode.historyChanged(
        counting_allocator,
        sampled_history,
    );
    counting_allocator.free(sampled_payload);
    const history_allocation_count = allocation_counter.alloc_index;
    try binding.runtime.rollbackHistoryEvent(attachment_id, sampled_history);
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.startSelectionExport(attachment_id, 1, .plain);

    var scheduler: subject.Scheduler = .{};
    var failed = host_outgoing.Outgoing.init(std.testing.allocator);
    defer failed.deinit();
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        // Let history encoding complete, then fail the first allocation for
        // the later selection record. The scheduler must roll the already
        // taken history event back even though its record reached this batch.
        .{ .fail_index = history_allocation_count },
    );
    try std.testing.expectError(
        error.OutOfMemory,
        scheduler.schedule(
            failing.allocator(),
            &aggregate,
            connection_id,
            &failed,
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), failed.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(failed.records()[0])).kind,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 2), retry.records().len);
    try std.testing.expectEqual(
        protocol.MessageKind.history_changed,
        (try protocol.validateTransportEnvelope(retry.records()[0])).kind,
    );
    try std.testing.expectEqual(
        protocol.MessageKind.selection_export_chunk,
        (try protocol.validateTransportEnvelope(retry.records()[1])).kind,
    );
}

test "full cancellation record remains queued until a later batch commits it" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x62);
    const binding = try createReadableRuntime(&aggregate, connection_id, attachment_id);
    const geometry = binding.runtime.geometry();
    _ = try binding.runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    _ = try binding.runtime.startSelectionExport(attachment_id, 1, .plain);
    _ = try binding.runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );
    try std.testing.expectEqual(
        terminal_runtime.SelectionSnapshot.active,
        std.meta.activeTag((try binding.runtime.takeSelectionEvent(attachment_id)).?),
    );

    var scheduler: subject.Scheduler = .{};
    var full = host_outgoing.Outgoing.init(std.testing.allocator);
    defer full.deinit();
    try fillOutgoingLeaving(&full, aggregate.core().epoch, 64);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.batch_full,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &full,
        ),
    );
    try std.testing.expect(
        (try binding.runtime.peekSelectionExportCancellation(attachment_id)) != null,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &aggregate,
        connection_id,
        &retry,
    );
    try std.testing.expectEqual(@as(usize, 1), retry.records().len);
    const finished = try protocol.validateTransportEnvelope(retry.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.selection_export_finished, finished.kind);
    try std.testing.expect(std.mem.indexOf(u8, finished.payload, "\"status\":\"cancelled\"") != null);
    try std.testing.expect(
        (try binding.runtime.peekSelectionExportCancellation(attachment_id)) == null,
    );
}
