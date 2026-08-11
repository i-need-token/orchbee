const std = @import("std");
const target_builtin = @import("builtin");
const control_boundary = @import("terminal-host-control");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");
const service_boundary = @import("terminal-host-service");

const core_module = core_boundary.state;
const control_encode = control_boundary.encode;
const control_wire = control_boundary.wire;
const host_outgoing = service_boundary.testing.outgoing;
const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
const subject = service_boundary.testing.interaction;
const service_subject = service_boundary.service;
const terminal_launch = engine_boundary.launch;
const terminal_runtime = engine_boundary.runtime;

const commitReaderBatch = terminal_runtime.testing.commitReaderBatchFn;
const createWithOptionsUsingSpawner = terminal_runtime.testing.createWithOptionsUsingSpawnerFn;
const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
const RuntimeTestingHooks = terminal_runtime.testing.RuntimeTestingHooksType;
const SchedulerTestingHooks = subject.testing.SchedulerTestingHooksType;

const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

const RecordingAllocator = struct {
    backing: std.mem.Allocator,
    alloc_request_count: usize = 0,
    max_requested: usize = 0,
    row_window_request_count: usize = 0,

    fn init(backing: std.mem.Allocator) RecordingAllocator {
        return .{ .backing = backing };
    }

    fn allocator(self: *RecordingAllocator) std.mem.Allocator {
        return .{ .ptr = self, .vtable = &vtable };
    }

    fn reset(self: *RecordingAllocator) void {
        self.alloc_request_count = 0;
        self.max_requested = 0;
        self.row_window_request_count = 0;
    }

    fn recordRequest(self: *RecordingAllocator, len: usize) void {
        self.max_requested = @max(self.max_requested, len);
        if (len == protocol.Limits.row_window_bytes) self.row_window_request_count += 1;
    }

    fn alloc(
        context: *anyopaque,
        len: usize,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.alloc_request_count += 1;
        self.recordRequest(len);
        return self.backing.rawAlloc(len, alignment, return_address);
    }

    fn resize(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) bool {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawResize(memory, alignment, new_len, return_address);
    }

    fn remap(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) ?[*]u8 {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.recordRequest(new_len);
        return self.backing.rawRemap(memory, alignment, new_len, return_address);
    }

    fn free(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) void {
        const self: *RecordingAllocator = @ptrCast(@alignCast(context));
        self.backing.rawFree(memory, alignment, return_address);
    }

    const vtable: std.mem.Allocator.VTable = .{
        .alloc = alloc,
        .resize = resize,
        .remap = remap,
        .free = free,
    };
};

fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

fn testLaunch(arguments: []const []const u8) terminal_launch.Launch {
    return testLaunchWithGeometry(arguments, 80, 24, 8, 16);
}

fn testLaunchWithGeometry(
    arguments: []const []const u8,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) terminal_launch.Launch {
    return .{
        .arguments = arguments,
        .configuration = null,
        .current_directory = null,
        .environment = &.{},
        .executable = "/bin/sh",
        .geometry = .{
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .columns = columns,
            .rows = rows,
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
    terminal_id: core_module.TerminalId,
};

fn createReadableRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
) !TestRuntime {
    return createReadableRuntimeWithGeometry(
        aggregate,
        connection_id,
        attachment_id,
        80,
        24,
        8,
        16,
    );
}

fn createReadableRuntimeWithGeometry(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) !TestRuntime {
    const arguments = [_][]const u8{ "-c", "read value" };
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        testLaunchWithGeometry(
            &arguments,
            columns,
            rows,
            cell_pixel_width,
            cell_pixel_height,
        ),
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
        .terminal_id = terminal_id,
    };
}

const HookedThreadSpawner = struct {
    hooks: RuntimeTestingHooks,

    pub fn terminalRuntimeTestingHooks(self: *HookedThreadSpawner) RuntimeTestingHooks {
        return self.hooks;
    }

    pub fn spawn(
        _: *HookedThreadSpawner,
        comptime thread_function: fn (*terminal_runtime.TerminalRuntime) void,
        runtime: *terminal_runtime.TerminalRuntime,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

fn createHookedReadableRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    creation_id: core_module.CreationId,
    terminal_id: core_module.TerminalId,
    attachment_id: core_module.AttachmentId,
    hooks: RuntimeTestingHooks,
) !TestRuntime {
    const arguments = [_][]const u8{ "-c", "read value" };
    const validated = try terminal_launch.inspect(
        std.testing.allocator,
        testLaunch(&arguments),
    );
    var preparation = try validated.prepareRuntime(std.testing.allocator, .{});
    defer preparation.deinit(std.testing.allocator);
    try aggregate.core().reserveTerminal();
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    const runtime = try createWithOptionsUsingSpawner(
        std.testing.allocator,
        preparation.process_launch,
        preparation.runtime_options,
        &spawner,
    );
    aggregate.core().commitRunningTerminal(
        connection_id,
        creation_id,
        terminal_id,
        validated.launch_digest,
        preparation.process_launch.columns,
        preparation.process_launch.rows,
        preparation.process_launch.cell_pixel_width,
        preparation.process_launch.cell_pixel_height,
        preparation.configuration,
        runtime,
    );
    _ = try aggregate.attach(
        connection_id,
        attachment_id,
        terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    return .{ .runtime = runtime, .terminal_id = terminal_id };
}

const ContainmentFailureProbe = struct {
    allow_success: bool = false,
    contain_calls: usize = 0,
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    fn fromContext(context: *anyopaque) *ContainmentFailureProbe {
        return @ptrCast(@alignCast(context));
    }

    fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.contain_calls += 1;
        if (!self.allow_success) return error.InjectedContainmentFailure;
        try self.runtime.?.forceTerminate();
    }

    fn hooks(self: *ContainmentFailureProbe) RuntimeTestingHooks {
        return .{
            .contain_process_tree = contain,
            .context = self,
        };
    }
};

const CancellationCommitBarrier = struct {
    calls: usize = 0,
    runtime: *terminal_runtime.TerminalRuntime,

    fn fromContext(context: *anyopaque) *CancellationCommitBarrier {
        return @ptrCast(@alignCast(context));
    }

    fn afterRecordsAppended(context: *anyopaque) void {
        const self = fromContext(context);
        self.calls += 1;
        recordReaderFailure(self.runtime);
    }

    fn hooks(self: *CancellationCommitBarrier) SchedulerTestingHooks {
        return .{
            .after_cancellation_records_appended = afterRecordsAppended,
            .context = self,
        };
    }
};

fn queueSelectionExportCancellation(
    runtime: *terminal_runtime.TerminalRuntime,
    attachment_id: core_module.AttachmentId,
    export_sequence: u64,
) !terminal_runtime.SelectionExportCancellation {
    const geometry = runtime.geometry();
    _ = try runtime.applySelectionCommand(.{
        .attachment_id = attachment_id,
        .command = .select_all,
        .generation = geometry.generation,
        .revision = geometry.revision,
    });
    const started = (try runtime.startSelectionExport(
        attachment_id,
        export_sequence,
        .plain,
    )).started;
    _ = try runtime.resizeGeometry(
        geometry.columns + 1,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    );
    _ = try runtime.takeSelectionEvent(attachment_id);
    return .{
        .attachment_id = attachment_id,
        .byte_length = started.byte_length,
        .export_sequence = started.export_sequence,
    };
}

fn pendingCancellation(
    runtime: *terminal_runtime.TerminalRuntime,
    attachment_id: core_module.AttachmentId,
) !?terminal_runtime.SelectionExportCancellation {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    const prepared = try runtime.history.prepareSelectionExportCancellation(attachment_id);
    return if (prepared) |token| token.cancellation else null;
}

fn expectCancellationRecord(
    record: []const u8,
    cancellation: terminal_runtime.SelectionExportCancellation,
) !void {
    const envelope = try protocol.validateTransportEnvelope(record);
    try std.testing.expectEqual(protocol.MessageKind.selection_export_finished, envelope.kind);
    try std.testing.expect(envelope.correlation_id == null);
    try std.testing.expect(!envelope.response);
    try std.testing.expect(envelope.event);
    var attachment_buffer: [32]u8 = undefined;
    const expected_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"status\":\"cancelled\"}}",
        .{
            control_encode.identifierText(cancellation.attachment_id, &attachment_buffer),
            cancellation.byte_length,
            cancellation.export_sequence,
        },
    );
    defer std.testing.allocator.free(expected_payload);
    try std.testing.expectEqualSlices(u8, expected_payload, envelope.payload);
}

fn retainRows(runtime: *terminal_runtime.TerminalRuntime, row_count: usize) !void {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    while (runtime.terminal.screens.active.pages.total_rows < row_count) {
        try runtime.terminal.printString("\n");
    }
    try std.testing.expectEqual(
        row_count,
        runtime.terminal.screens.active.pages.total_rows,
    );
}

fn requestRenderRows(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    geometry: terminal_runtime.Geometry,
    request_sequence: u64,
    row_count: u16,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"detail\":\"render\",\"generation\":\"{d}\",\"maximumBytes\":{d},\"origin\":{{\"kind\":\"absolute\",\"row\":\"0\"}},\"purpose\":\"viewport\",\"requestSequence\":\"{d}\",\"revision\":\"{d}\",\"rowCount\":{d}}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            geometry.generation,
            protocol.Limits.row_window_bytes,
            request_sequence,
            geometry.revision,
            row_count,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.read_rows,
        .control_json,
        payload,
        [_]u8{correlation_byte} ** 16,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    return service.handle(connection, record);
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
    const pending = aggregate.core().findTerminal(terminal_id).?.pendingOutcome() orelse
        return error.ExpectedPendingOutcome;
    _ = aggregate.core().markOutcomePublished(switch (pending) {
        .exited => |outcome| outcome.token,
        .failed => |outcome| outcome.token,
    });
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

test "read rows service boundary maps oversized render geometry to limit exceeded" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    const valid_row_count: u16 = 3_640;
    const invalid_row_count: u16 = valid_row_count + 1;
    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x3a);
    const binding = try createReadableRuntimeWithGeometry(
        &service.runtime_aggregate,
        connection_id,
        attachment_id,
        8,
        24,
        8,
        18,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    try retainRows(binding.runtime, invalid_row_count);
    const geometry = binding.runtime.geometry();

    var oversized = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        1,
        invalid_row_count,
        0x41,
    );
    defer oversized.deinit();
    try std.testing.expectEqual(@as(usize, 1), oversized.records().len);
    const failure_envelope = try protocol.validateTransportEnvelope(oversized.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.typed_failure, failure_envelope.kind);
    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const failure = try control_wire.decode(
        Failure,
        std.testing.allocator,
        failure_envelope.payload,
    );
    defer failure.deinit();
    try std.testing.expectEqualStrings("limit-exceeded", failure.value.code);

    var adjacent = try requestRenderRows(
        &service,
        &connection,
        attachment_id,
        geometry,
        1,
        valid_row_count,
        0x42,
    );
    defer adjacent.deinit();
    try std.testing.expectEqual(@as(usize, 1), adjacent.records().len);
    const rows_envelope = try protocol.validateTransportEnvelope(adjacent.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.rows_snapshot, rows_envelope.kind);
    const summary = try protocol.validateTerminalRowWindow(
        std.testing.allocator,
        rows_envelope.payload,
    );
    try std.testing.expectEqual(valid_row_count, summary.row_count);
    try std.testing.expectEqual(@as(u64, 1), summary.sequence);
}

test "read rows service boundary uses bounded scratch for an exact empty text row" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var recording = RecordingAllocator.init(std.testing.allocator);
    var service = service_subject.HostService.init(recording.allocator());
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x3b);
    const binding = try createReadableRuntimeWithGeometry(
        &service.runtime_aggregate,
        connection_id,
        attachment_id,
        1,
        1,
        8,
        16,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    const geometry = binding.runtime.geometry();
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"detail\":\"text\",\"generation\":\"{d}\",\"maximumBytes\":124,\"origin\":{{\"kind\":\"absolute\",\"row\":\"0\"}},\"purpose\":\"search\",\"requestSequence\":\"1\",\"revision\":\"{d}\",\"rowCount\":1}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            geometry.generation,
            geometry.revision,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.read_rows,
        .control_json,
        payload,
        [_]u8{0x43} ** 16,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);

    recording.reset();
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();

    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(protocol.MessageKind.rows_snapshot, envelope.kind);
    try std.testing.expectEqual(@as(usize, 124), envelope.payload.len);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, envelope.payload);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u16, 1), summary.columns);
    try std.testing.expectEqual(@as(u16, 1), summary.row_count);
    try std.testing.expectEqual(@as(u32, 0), summary.entry_count);
    try std.testing.expect(recording.alloc_request_count > 0);
    try std.testing.expectEqual(@as(usize, 0), recording.row_window_request_count);
    try std.testing.expect(recording.max_requested < protocol.Limits.row_window_bytes);
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
        (try binding.runtime.prepareSelectionExportCancellation(attachment_id)) != null,
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
        (try binding.runtime.prepareSelectionExportCancellation(attachment_id)) == null,
    );
}

test "THCR-001 scheduler commits an appended cancellation across fatal containment failure" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: ContainmentFailureProbe = .{};
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    defer probe.allow_success = true;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x70));
    const attachment_id = testId(core_module.AttachmentId, 0x71);
    const binding = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x72),
        testId(core_module.TerminalId, 0x73),
        attachment_id,
        probe.hooks(),
    );
    probe.runtime = binding.runtime;
    const cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        attachment_id,
        1,
    );
    try std.testing.expectEqualDeep(
        cancellation,
        (try pendingCancellation(binding.runtime, attachment_id)).?,
    );

    var barrier: CancellationCommitBarrier = .{ .runtime = binding.runtime };
    var scheduler: subject.Scheduler = .{};
    subject.testing.setSchedulerTestingHooks(&scheduler, barrier.hooks());
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &outgoing,
        ),
    );

    try std.testing.expectEqual(@as(usize, 1), barrier.calls);
    try std.testing.expectEqual(@as(usize, 1), probe.contain_calls);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    try expectCancellationRecord(outgoing.records()[0], cancellation);
    try std.testing.expect((try pendingCancellation(binding.runtime, attachment_id)) == null);
    try std.testing.expect(!binding.runtime.readableStateAvailable());
    try std.testing.expectEqual(
        terminal_runtime.RuntimeFailureKind.pty_read,
        binding.runtime.primaryFailure().?,
    );

    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(@as(usize, 2), probe.contain_calls);
    const blocked = aggregate.core().findTerminal(binding.terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.containing_failure, blocked.phase());
    try std.testing.expectEqual(core_module.RuntimeFailure.read_output, blocked.runtimeFailure().?);
    switch (blocked.failureContainment().?) {
        .blocked => |containment| try std.testing.expectEqual(
            core_module.RuntimeCleanupFailure.contain_process,
            containment.cleanup_failure,
        ),
        .stopping => return error.ExpectedBlockedContainment,
    }

    _ = try aggregate.core().failureContainmentBlocked(
        binding.terminal_id,
        .contain_process,
        0,
    );
    probe.allow_success = true;
    aggregate.synchronizeRuntimes();
    try std.testing.expectEqual(@as(usize, 3), probe.contain_calls);
    const retained_failure = aggregate.core().findTerminal(binding.terminal_id).?;
    try std.testing.expectEqual(core_module.TerminalPhase.failed, retained_failure.phase());
    try std.testing.expectEqual(
        core_module.RuntimeFailure.read_output,
        retained_failure.runtimeFailure().?,
    );
}

test "THCR-001 complete two-cancellation batch has no committed prefix" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var later_probe: ContainmentFailureProbe = .{};
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    defer later_probe.allow_success = true;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x80));
    const first_attachment_id = testId(core_module.AttachmentId, 0x81);
    const first = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x82),
        testId(core_module.TerminalId, 0x83),
        first_attachment_id,
        .{},
    );
    const later_attachment_id = testId(core_module.AttachmentId, 0x84);
    const later = try createHookedReadableRuntime(
        &aggregate,
        connection_id,
        testId(core_module.CreationId, 0x85),
        testId(core_module.TerminalId, 0x86),
        later_attachment_id,
        later_probe.hooks(),
    );
    later_probe.runtime = later.runtime;
    const first_cancellation = try queueSelectionExportCancellation(
        first.runtime,
        first_attachment_id,
        11,
    );
    const later_cancellation = try queueSelectionExportCancellation(
        later.runtime,
        later_attachment_id,
        22,
    );

    var barrier: CancellationCommitBarrier = .{ .runtime = later.runtime };
    var scheduler: subject.Scheduler = .{};
    subject.testing.setSchedulerTestingHooks(&scheduler, barrier.hooks());
    var outgoing = host_outgoing.Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &outgoing,
        ),
    );

    try std.testing.expectEqual(@as(usize, 1), barrier.calls);
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    try expectCancellationRecord(outgoing.records()[0], first_cancellation);
    try expectCancellationRecord(outgoing.records()[1], later_cancellation);
    try std.testing.expect((try pendingCancellation(first.runtime, first_attachment_id)) == null);
    try std.testing.expect((try pendingCancellation(later.runtime, later_attachment_id)) == null);
    try std.testing.expect(first.runtime.readableStateAvailable());
    try std.testing.expect(!later.runtime.readableStateAvailable());
}

test "THCR-001 allocator failure preserves every prepared cancellation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const first_attachment_id = testId(core_module.AttachmentId, 0x92);
    const binding = try createReadableRuntime(&aggregate, connection_id, first_attachment_id);
    const second_attachment_id = testId(core_module.AttachmentId, 0x93);
    _ = try aggregate.attach(
        connection_id,
        second_attachment_id,
        binding.terminal_id,
        .{ .input = false, .read = true, .resize = false },
        false,
    );
    const first_cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        first_attachment_id,
        31,
    );
    const second_cancellation = try queueSelectionExportCancellation(
        binding.runtime,
        second_attachment_id,
        32,
    );
    while (try binding.runtime.takeSelectionEvent(first_attachment_id)) |_| {}
    while (try binding.runtime.takeSelectionEvent(second_attachment_id)) |_| {}

    var allocation_counter = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{},
    );
    const counting_allocator = allocation_counter.allocator();
    const sampled_payload = try control_encode.selectionExportFinished(
        counting_allocator,
        first_cancellation,
        subject.SelectionExportStatus.cancelled,
    );
    counting_allocator.free(sampled_payload);
    const cancellation_allocation_count = allocation_counter.alloc_index;

    var scheduler: subject.Scheduler = .{};
    var failed = host_outgoing.Outgoing.init(std.testing.allocator);
    defer failed.deinit();
    var failing = std.testing.FailingAllocator.init(
        std.testing.allocator,
        .{ .fail_index = cancellation_allocation_count },
    );
    // The allocating writer maps its backing allocator exhaustion to WriteFailed.
    try std.testing.expectError(
        error.WriteFailed,
        scheduler.schedule(
            failing.allocator(),
            &aggregate,
            connection_id,
            &failed,
        ),
    );
    try std.testing.expectEqual(@as(usize, 1), failed.records().len);
    try expectCancellationRecord(failed.records()[0], first_cancellation);
    try std.testing.expectEqualDeep(
        first_cancellation,
        (try pendingCancellation(binding.runtime, first_attachment_id)).?,
    );
    try std.testing.expectEqualDeep(
        second_cancellation,
        (try pendingCancellation(binding.runtime, second_attachment_id)).?,
    );

    var retry = host_outgoing.Outgoing.init(std.testing.allocator);
    defer retry.deinit();
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            &retry,
        ),
    );
    try std.testing.expectEqual(@as(usize, 2), retry.records().len);
    try expectCancellationRecord(retry.records()[0], first_cancellation);
    try expectCancellationRecord(retry.records()[1], second_cancellation);
    try std.testing.expect((try pendingCancellation(binding.runtime, first_attachment_id)) == null);
    try std.testing.expect((try pendingCancellation(binding.runtime, second_attachment_id)) == null);
}

const HistoryChangedSummary = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

fn historyChangedSummary(record: []const u8) !HistoryChangedSummary {
    const envelope = try protocol.validateTransportEnvelope(record);
    try std.testing.expectEqual(protocol.MessageKind.history_changed, envelope.kind);
    const Wire = struct {
        anchorStatus: []const u8,
        attachmentId: []const u8,
        generation: []const u8,
        revision: []const u8,
        sequence: []const u8,
        totalRows: []const u8,
    };
    const parsed = try control_wire.decode(Wire, std.testing.allocator, envelope.payload);
    defer parsed.deinit();
    return .{
        .generation = try std.fmt.parseInt(u64, parsed.value.generation, 10),
        .revision = try std.fmt.parseInt(u64, parsed.value.revision, 10),
        .sequence = try std.fmt.parseInt(u64, parsed.value.sequence, 10),
    };
}

fn dispatchHistoryObserved(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            generation,
            revision,
            sequence,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.history_observed,
        .control_json,
        payload,
        [_]u8{correlation_byte} ** 16,
        service.runtime_aggregate.core().epoch.bytes,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    return service.handle(connection, record);
}

test "THCR-013 service schedules distinct wire sequences and the exact acknowledgement unblocks later mutation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var service = service_subject.HostService.init(std.testing.allocator);
    var connection: service_subject.ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    const connection_id: core_module.ConnectionId = 1;
    const attachment_id = testId(core_module.AttachmentId, 0x16);
    const binding = try createReadableRuntime(
        &service.runtime_aggregate,
        connection_id,
        attachment_id,
    );
    connection = .{ .active = .{
        .connection_id = connection_id,
        .limits = .{
            .frame_bytes = protocol.Limits.frame_bytes,
            .input_bytes = protocol.Limits.input_bytes,
            .row_window_bytes = protocol.Limits.row_window_bytes,
        },
    } };
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);
    var scheduler: subject.Scheduler = .{};

    var first = host_outgoing.Outgoing.init(std.testing.allocator);
    defer first.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &service.runtime_aggregate,
        connection_id,
        &first,
    );
    try std.testing.expectEqual(@as(usize, 1), first.records().len);
    const first_event = try historyChangedSummary(first.records()[0]);
    try std.testing.expectEqual(@as(u64, 1), first_event.sequence);
    var ack1 = try dispatchHistoryObserved(
        &service,
        &connection,
        attachment_id,
        first_event.sequence,
        first_event.revision,
        first_event.generation,
        0x61,
    );
    defer ack1.deinit();

    // Leave and re-enter viewing history without a terminal mutation, so the
    // next event carries the same revision/generation identity.
    try binding.runtime.setHistoryDemand(attachment_id, .following_bottom);
    try binding.runtime.setHistoryDemand(attachment_id, .viewing_history);

    var second = host_outgoing.Outgoing.init(std.testing.allocator);
    defer second.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &service.runtime_aggregate,
        connection_id,
        &second,
    );
    try std.testing.expectEqual(@as(usize, 1), second.records().len);
    const second_event = try historyChangedSummary(second.records()[0]);
    try std.testing.expectEqual(first_event.sequence + 1, second_event.sequence);
    try std.testing.expectEqual(first_event.revision, second_event.revision);
    try std.testing.expectEqual(first_event.generation, second_event.generation);
    var ack2 = try dispatchHistoryObserved(
        &service,
        &connection,
        attachment_id,
        second_event.sequence,
        second_event.revision,
        second_event.generation,
        0x62,
    );
    defer ack2.deinit();

    // Mutate and confirm the scheduler emits the next history event after the
    // exact second-sequence acknowledgement.
    std.debug.assert(commitReaderBatch(binding.runtime, "x") == .committed);
    var third = host_outgoing.Outgoing.init(std.testing.allocator);
    defer third.deinit();
    _ = try scheduler.schedule(
        std.testing.allocator,
        &service.runtime_aggregate,
        connection_id,
        &third,
    );
    try std.testing.expectEqual(@as(usize, 1), third.records().len);
    const third_event = try historyChangedSummary(third.records()[0]);
    try std.testing.expectEqual(second_event.sequence + 1, third_event.sequence);
    try std.testing.expectEqual(second_event.revision + 1, third_event.revision);
}
