pub const std = @import("std");
pub const target_builtin = @import("builtin");
pub const control_boundary = @import("terminal-host-control");
pub const core_boundary = @import("terminal-host-core");
pub const engine_boundary = @import("terminal-host-engine");
pub const protocol = @import("terminal-protocol");
pub const service_boundary = @import("terminal-host-service");

pub const core_module = core_boundary.state;
pub const control_encode = control_boundary.encode;
pub const control_wire = control_boundary.testing.wire;
pub const host_outgoing = service_boundary.testing.outgoing;
pub const host_runtime_aggregate = service_boundary.testing.runtime_aggregate;
pub const subject = service_boundary.testing.interaction;
pub const service_subject = service_boundary.service;
pub const terminal_launch = engine_boundary.launch;
pub const terminal_runtime = engine_boundary.runtime;

pub const commitReaderBatch = terminal_runtime.testing.commitReaderBatchFn;
pub const createModel = core_module.testing.createTerminal;
pub const createWithOptionsUsingSpawner = terminal_runtime.testing.createWithOptionsUsingSpawnerFn;
pub const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
pub const ReaderBatchOutcome = terminal_runtime.testing.ReaderBatchOutcomeType;
pub const RuntimeTestingHooks = terminal_runtime.testing.RuntimeTestingHooksType;
pub const SchedulerTestingHooks = subject.testing.SchedulerTestingHooksType;

pub const features = [_]u16{22};
pub const palette = [_]u32{0} ** 256;

pub fn serviceAggregate(
    service: *service_subject.HostService,
) *host_runtime_aggregate.HostRuntimeAggregate {
    return &service.runtime_aggregate;
}

pub fn serviceCore(service: *service_subject.HostService) *core_module.HostCore {
    return serviceAggregate(service).core();
}

pub const RecordingAllocator = struct {
    backing: std.mem.Allocator,
    alloc_request_count: usize = 0,
    max_requested: usize = 0,
    row_window_request_count: usize = 0,

    pub fn init(backing: std.mem.Allocator) RecordingAllocator {
        return .{ .backing = backing };
    }

    pub fn allocator(self: *RecordingAllocator) std.mem.Allocator {
        return .{ .ptr = self, .vtable = &vtable };
    }

    pub fn reset(self: *RecordingAllocator) void {
        self.alloc_request_count = 0;
        self.max_requested = 0;
        self.row_window_request_count = 0;
    }

    pub fn recordRequest(self: *RecordingAllocator, len: usize) void {
        self.max_requested = @max(self.max_requested, len);
        if (len == protocol.Limits.row_window_bytes) self.row_window_request_count += 1;
    }

    pub fn alloc(
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

    pub fn resize(
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

    pub fn remap(
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

    pub fn free(
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

pub fn testId(comptime Id: type, byte: u8) Id {
    return Id.fromBytes([_]u8{byte} ** 16);
}

pub fn testLaunch(arguments: []const []const u8) terminal_launch.Launch {
    return testLaunchWithGeometry(arguments, 80, 24, 8, 16);
}

pub fn testLaunchWithGeometry(
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

pub const TestRuntime = struct {
    runtime: *terminal_runtime.TerminalRuntime,
    terminal_id: core_module.TerminalId,
};

pub fn createReadableRuntime(
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

pub fn createReadableRuntimeWithGeometry(
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

pub const HookedThreadSpawner = struct {
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

pub fn createHookedReadableRuntime(
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
    var preparation = try terminal_launch.prepareRuntime(validated, std.testing.allocator, .{});
    defer preparation.deinit(std.testing.allocator);
    try aggregate.core().reserveTerminal();
    var spawner: HookedThreadSpawner = .{ .hooks = hooks };
    const runtime = try createWithOptionsUsingSpawner(
        std.testing.allocator,
        preparation.process_launch,
        preparation.runtime_options,
        &spawner,
    );
    try host_runtime_aggregate.testing.adoptRunningTerminalFn(
        aggregate,
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

pub const ContainmentFailureProbe = struct {
    allow_success: bool = false,
    contain_calls: usize = 0,
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    pub fn fromContext(context: *anyopaque) *ContainmentFailureProbe {
        return @ptrCast(@alignCast(context));
    }

    pub fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.contain_calls += 1;
        if (!self.allow_success) return error.InjectedContainmentFailure;
        try self.runtime.?.forceTerminate();
    }

    pub fn hooks(self: *ContainmentFailureProbe) RuntimeTestingHooks {
        return .{
            .contain_process_tree = contain,
            .context = self,
        };
    }
};

pub const CancellationCommitBarrier = struct {
    calls: usize = 0,
    runtime: *terminal_runtime.TerminalRuntime,

    pub fn fromContext(context: *anyopaque) *CancellationCommitBarrier {
        return @ptrCast(@alignCast(context));
    }

    pub fn afterRecordsAppended(context: *anyopaque) void {
        const self = fromContext(context);
        self.calls += 1;
        recordReaderFailure(self.runtime);
    }

    pub fn hooks(self: *CancellationCommitBarrier) SchedulerTestingHooks {
        return .{
            .after_cancellation_records_appended = afterRecordsAppended,
            .context = self,
        };
    }
};

pub fn queueSelectionExportCancellation(
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

pub fn pendingCancellation(
    runtime: *terminal_runtime.TerminalRuntime,
    attachment_id: core_module.AttachmentId,
) !?terminal_runtime.SelectionExportCancellation {
    runtime.terminal_mutex.lock();
    defer runtime.terminal_mutex.unlock();
    const prepared = try runtime.history.prepareSelectionExportCancellation(attachment_id);
    return if (prepared) |token| token.cancellation else null;
}

pub fn expectCancellationRecord(
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

pub fn retainRows(runtime: *terminal_runtime.TerminalRuntime, row_count: usize) !void {
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

pub fn requestRenderRows(
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

pub fn fillOutgoingLeaving(
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

pub fn expectBellRecord(
    record: []const u8,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
) !void {
    const envelope = try protocol.validateTransportEnvelope(record);
    try std.testing.expectEqual(protocol.MessageKind.terminal_bell, envelope.kind);
    const Wire = struct {
        attachmentId: []const u8,
        sequence: []const u8,
    };
    const parsed = try control_wire.decode(Wire, std.testing.allocator, envelope.payload);
    defer parsed.deinit();
    var attachment_buffer: [32]u8 = undefined;
    try std.testing.expectEqualStrings(
        control_encode.identifierText(attachment_id, &attachment_buffer),
        parsed.value.attachmentId,
    );
    try std.testing.expectEqual(sequence, try std.fmt.parseInt(u64, parsed.value.sequence, 10));
}

pub const HistoryChangedSummary = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub fn historyChangedSummary(record: []const u8) !HistoryChangedSummary {
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

pub fn dispatchHistoryObserved(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    return dispatchControlAcknowledgement(
        service,
        connection,
        protocol.MessageKind.history_observed,
        attachment_id,
        sequence,
        revision,
        generation,
        correlation_byte,
        "sequence",
    );
}

pub fn dispatchFrameApplied(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    return dispatchControlAcknowledgement(
        service,
        connection,
        protocol.MessageKind.frame_applied,
        attachment_id,
        sequence,
        revision,
        generation,
        correlation_byte,
        "sequence",
    );
}

pub fn dispatchRowsApplied(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    request_sequence: u64,
    revision: u64,
    generation: u64,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    return dispatchControlAcknowledgement(
        service,
        connection,
        protocol.MessageKind.rows_applied,
        attachment_id,
        request_sequence,
        revision,
        generation,
        correlation_byte,
        "requestSequence",
    );
}

pub fn dispatchControlAcknowledgement(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    kind: u16,
    attachment_id: core_module.AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
    correlation_byte: u8,
    sequence_field: []const u8,
) !host_outgoing.Outgoing {
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"{s}\":\"{d}\"}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            generation,
            revision,
            sequence_field,
            sequence,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        kind,
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

pub fn dispatchSelectionExportChunkApplied(
    service: *service_subject.HostService,
    connection: *service_subject.ConnectionState,
    attachment_id: core_module.AttachmentId,
    export_sequence: u64,
    chunk_sequence: u32,
    correlation_byte: u8,
) !host_outgoing.Outgoing {
    var attachment_buffer: [32]u8 = undefined;
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"chunkSequence\":{d},\"exportSequence\":\"{d}\"}}",
        .{
            control_encode.identifierText(attachment_id, &attachment_buffer),
            chunk_sequence,
            export_sequence,
        },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        protocol.MessageKind.selection_export_chunk_applied,
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

pub fn exportChunkInBatch(
    outgoing: *const host_outgoing.Outgoing,
    expected_export_sequence: u64,
) !?u32 {
    for (outgoing.records()) |record| {
        const envelope = try protocol.validateTransportEnvelope(record);
        if (envelope.kind != protocol.MessageKind.selection_export_chunk) continue;
        const export_sequence = std.mem.readInt(u64, envelope.payload[16..24], .little);
        if (export_sequence != expected_export_sequence) continue;
        return std.mem.readInt(u32, envelope.payload[24..28], .little);
    }
    return null;
}

pub fn expectSameKindAcknowledgementSuccess(
    outgoing: *const host_outgoing.Outgoing,
    kind: u16,
    correlation_byte: u8,
) !void {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(kind, envelope.kind);
    try std.testing.expect(envelope.response);
    try std.testing.expect(!envelope.event);
    try std.testing.expectEqualSlices(
        u8,
        &[_]u8{correlation_byte} ** 16,
        &envelope.correlation_id.?,
    );
}

// THCR-029 regression: a late duplicate of an already-applied frame
// acknowledgement must return the registered same-kind success response rather
// than a typed_failure, while a newer frame is in flight.

// THCR-029 regression: a late duplicate of the applied row window
// acknowledgement must return the registered same-kind success response rather
// than a typed_failure, while a newer row window is pending.

// THCR-029 regression: replaying the acknowledgement of the last completed
// export must return the registered same-kind success response rather than a
// typed_failure, while a newer export is in flight.

pub fn expectExportChunkAcknowledgementFinished(
    outgoing: *const host_outgoing.Outgoing,
    correlation_byte: u8,
) !void {
    try std.testing.expectEqual(@as(usize, 2), outgoing.records().len);
    const acknowledgement = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(
        protocol.MessageKind.selection_export_chunk_applied,
        acknowledgement.kind,
    );
    try std.testing.expect(acknowledgement.response);
    try std.testing.expect(!acknowledgement.event);
    try std.testing.expectEqualSlices(
        u8,
        &[_]u8{correlation_byte} ** 16,
        &acknowledgement.correlation_id.?,
    );
    const finished = try protocol.validateTransportEnvelope(outgoing.records()[1]);
    try std.testing.expectEqual(protocol.MessageKind.selection_export_finished, finished.kind);
}
