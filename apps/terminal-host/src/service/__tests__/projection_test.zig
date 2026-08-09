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

const commitReaderBatch = terminal_runtime.testing.commitReaderBatchFn;
const createWithOptionsUsingSpawner =
    terminal_runtime.testing.createWithOptionsUsingSpawnerFn;
const ReadCommitCheckpoint = terminal_runtime.testing.ReadCommitCheckpointType;
const recordReaderFailure = terminal_runtime.testing.recordReaderFailureFn;
const RuntimeTestingHooks = terminal_runtime.testing.RuntimeTestingHooksType;

const MessageKind = protocol.MessageKind;
const host_outgoing = service_boundary.testing.outgoing;
const Outgoing = host_outgoing.Outgoing;

const projection_interval_ns: u64 = 16 * std.time.ns_per_ms;
const features = [_]u16{22};
const palette = [_]u32{0} ** 256;

const TestGate = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,
    released: bool = false,

    fn pause(self: *TestGate) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        while (!self.released) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn wait(self: *TestGate) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }

    fn release(self: *TestGate) void {
        self.mutex.lock();
        self.released = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }
};

const TestSignal = struct {
    condition: std.Thread.Condition = .{},
    mutex: std.Thread.Mutex = .{},
    reached: bool = false,

    fn notify(self: *TestSignal) void {
        self.mutex.lock();
        self.reached = true;
        self.condition.broadcast();
        self.mutex.unlock();
    }

    fn wait(self: *TestSignal) void {
        self.mutex.lock();
        while (!self.reached) self.condition.wait(&self.mutex);
        self.mutex.unlock();
    }
};

const FailOnceAfterCaptureAllocator = struct {
    backing: std.mem.Allocator,
    gate: TestGate = .{},
    fail_next_allocation: bool = true,

    fn allocator(self: *FailOnceAfterCaptureAllocator) std.mem.Allocator {
        return .{ .ptr = self, .vtable = &vtable };
    }

    fn alloc(
        context: *anyopaque,
        len: usize,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) ?[*]u8 {
        const self: *FailOnceAfterCaptureAllocator = @ptrCast(@alignCast(context));
        if (self.fail_next_allocation) {
            self.fail_next_allocation = false;
            self.gate.pause();
            return null;
        }
        return self.backing.rawAlloc(len, alignment, return_address);
    }

    fn resize(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) bool {
        const self: *FailOnceAfterCaptureAllocator = @ptrCast(@alignCast(context));
        return self.backing.rawResize(memory, alignment, new_len, return_address);
    }

    fn remap(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        new_len: usize,
        return_address: usize,
    ) ?[*]u8 {
        const self: *FailOnceAfterCaptureAllocator = @ptrCast(@alignCast(context));
        return self.backing.rawRemap(memory, alignment, new_len, return_address);
    }

    fn free(
        context: *anyopaque,
        memory: []u8,
        alignment: std.mem.Alignment,
        return_address: usize,
    ) void {
        const self: *FailOnceAfterCaptureAllocator = @ptrCast(@alignCast(context));
        self.backing.rawFree(memory, alignment, return_address);
    }

    const vtable: std.mem.Allocator.VTable = .{
        .alloc = alloc,
        .resize = resize,
        .remap = remap,
        .free = free,
    };
};

const CaptureAdvanceProbe = struct {
    calls: usize = 0,
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    fn fromContext(context: *anyopaque) *CaptureAdvanceProbe {
        return @ptrCast(@alignCast(context));
    }

    fn captureStarting(context: *anyopaque) void {
        const self = fromContext(context);
        self.calls += 1;
        std.debug.assert(commitReaderBatch(self.runtime.?, "advance-before-capture") == .committed);
    }

    fn hooks(self: *CaptureAdvanceProbe) RuntimeTestingHooks {
        return .{
            .context = self,
            .projection_capture_starting = captureStarting,
        };
    }
};

const FailureCommitProbe = struct {
    allow_containment: bool = false,
    checkpoint: ReadCommitCheckpoint,
    checkpoint_gate: TestGate = .{},
    containment_calls: usize = 0,
    failure_fence_starting: TestSignal = .{},
    runtime: ?*terminal_runtime.TerminalRuntime = null,

    fn fromContext(context: *anyopaque) *FailureCommitProbe {
        return @ptrCast(@alignCast(context));
    }

    fn contain(context: *anyopaque) anyerror!void {
        const self = fromContext(context);
        self.containment_calls += 1;
        if (!self.allow_containment) return error.InjectedContainmentFailure;
        try self.runtime.?.forceTerminate();
    }

    fn failureFenceStarting(context: *anyopaque) void {
        fromContext(context).failure_fence_starting.notify();
    }

    fn readCommitCheckpoint(context: *anyopaque, checkpoint: ReadCommitCheckpoint) void {
        const self = fromContext(context);
        if (checkpoint == self.checkpoint) self.checkpoint_gate.pause();
    }

    fn hooks(self: *FailureCommitProbe) RuntimeTestingHooks {
        return .{
            .contain_process_tree = contain,
            .context = self,
            .failure_fence_starting = failureFenceStarting,
            .read_commit_checkpoint = readCommitCheckpoint,
        };
    }
};

fn requireProjectionFrame(
    outcome: terminal_runtime.ProjectionEncodeOutcome,
) !terminal_runtime.ProjectionFrame {
    return switch (outcome) {
        .frame => |frame| frame,
        .blocked => error.UnexpectedProjectionBlock,
    };
}

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

const ScheduleCall = struct {
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    allocator: std.mem.Allocator,
    connection_id: core_module.ConnectionId,
    failure: ?anyerror = null,
    maximum_frame_bytes: usize,
    outcome: ?host_outgoing.ScheduleOutcome = null,
    outgoing: *Outgoing,
    scheduler: *subject.Scheduler,

    fn run(self: *ScheduleCall) void {
        self.outcome = self.scheduler.schedule(
            self.allocator,
            self.aggregate,
            self.connection_id,
            self.maximum_frame_bytes,
            self.outgoing,
        ) catch |err| {
            self.failure = err;
            return;
        };
    }

    fn expectComplete(self: *const ScheduleCall) !void {
        if (self.failure) |failure| return failure;
        try std.testing.expectEqual(host_outgoing.ScheduleOutcome.complete, self.outcome.?);
    }
};

const FailureCall = struct {
    runtime: *terminal_runtime.TerminalRuntime,

    fn run(self: *FailureCall) void {
        recordReaderFailure(self.runtime);
    }
};

fn createRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
) !RuntimeFixture {
    return createRuntimeAt(aggregate, connection_id, 0);
}

fn createRuntimeAt(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    index: usize,
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
        testId(core_module.CreationId, 0x42, index),
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

fn createHookedRuntime(
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    index: usize,
    hooks: RuntimeTestingHooks,
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
    const terminal_id = testId(core_module.TerminalId, 0x43, index);
    aggregate.core().commitRunningTerminal(
        connection_id,
        testId(core_module.CreationId, 0x42, index),
        terminal_id,
        validated.launch_digest,
        preparation.process_launch.columns,
        preparation.process_launch.rows,
        preparation.process_launch.cell_pixel_width,
        preparation.process_launch.cell_pixel_height,
        preparation.configuration,
        runtime,
    );
    return .{ .runtime = runtime, .terminal_id = terminal_id };
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

fn onlyProjectionBlockRevision(outgoing: *const Outgoing) !u64 {
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    const envelope = try protocol.validateTransportEnvelope(outgoing.records()[0]);
    try std.testing.expectEqual(MessageKind.projection_blocked, envelope.kind);
    var parsed = try std.json.parseFromSlice(
        std.json.Value,
        std.testing.allocator,
        envelope.payload,
        .{},
    );
    defer parsed.deinit();
    const revision = parsed.value.object.get("revision") orelse
        return error.MissingProjectionBlockRevision;
    if (revision != .string) return error.InvalidProjectionBlockRevision;
    return std.fmt.parseInt(u64, revision.string, 10);
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
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const first_runtime = try createRuntimeAt(&aggregate, connection_id, 0);
    const second_runtime = try createRuntimeAt(&aggregate, connection_id, 1);
    const first_attachment = testId(core_module.AttachmentId, 0x33, 0);
    const second_attachment = testId(core_module.AttachmentId, 0x33, 1);
    const terminal_ids = [_]core_module.TerminalId{
        first_runtime.terminal_id,
        second_runtime.terminal_id,
    };
    const attachment_ids = [_]core_module.AttachmentId{ first_attachment, second_attachment };
    for (terminal_ids, attachment_ids) |terminal_id, attachment_id| {
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
        .revision = 0,
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
        .revision = 0,
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
        first_runtime.runtime,
        &outgoing,
        first_attachment,
        MessageKind.snapshot_frame,
        &first_encoded,
    )) {
        .appended => {},
        .batch_full => return error.ExpectedProjectionAppend,
        .unavailable => return error.ExpectedReadableRuntime,
    }
    switch (try appendProjectionFrame(
        &aggregate,
        second_runtime.runtime,
        &outgoing,
        second_attachment,
        MessageKind.snapshot_frame,
        &second_encoded,
    )) {
        .appended => return error.ExpectedFullBatch,
        .batch_full => {},
        .unavailable => return error.ExpectedReadableRuntime,
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
    const expected = try requireProjectionFrame(try fixture.runtime.encodeProjectionFrameOwned(
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
    ));
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

test "THCR-009 projection block retains the captured revision and retries a newer observation" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createRuntime(&aggregate, connection_id);
    const attachment_id = testId(core_module.AttachmentId, 0x56, 0);
    try activateAttachments(&aggregate, connection_id, fixture.terminal_id, &.{attachment_id});
    const captured_revision = fixture.runtime.geometry().revision;

    var failing: FailOnceAfterCaptureAllocator = .{ .backing = std.testing.allocator };
    var blocked_outgoing = Outgoing.init(failing.allocator());
    defer blocked_outgoing.deinit();
    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, 0, 0);
    var call: ScheduleCall = .{
        .aggregate = &aggregate,
        .allocator = std.testing.allocator,
        .connection_id = connection_id,
        .maximum_frame_bytes = protocol.Limits.frame_bytes,
        .outgoing = &blocked_outgoing,
        .scheduler = &scheduler,
    };
    const thread = try std.Thread.spawn(.{}, ScheduleCall.run, .{&call});
    failing.gate.wait();
    const mutation_outcome = commitReaderBatch(fixture.runtime, "advanced-after-capture");
    const advanced_revision = fixture.runtime.geometry().revision;
    failing.gate.release();
    thread.join();
    try std.testing.expectEqual(.committed, mutation_outcome);
    try std.testing.expectEqual(captured_revision + 1, advanced_revision);
    try call.expectComplete();

    try std.testing.expectEqual(
        captured_revision,
        try onlyProjectionBlockRevision(&blocked_outgoing),
    );
    const raw_block = aggregate.core().attachmentValues()[0].projectionBlocked().?;
    try std.testing.expectEqual(captured_revision, raw_block.observed_revision);
    try std.testing.expectEqual(
        core_module.ProjectionBlock.capture_allocation_failed,
        raw_block.reason,
    );
    const refreshed = aggregate.core().findAttachment(attachment_id).?;
    try std.testing.expect(refreshed.projectionBlocked() == null);
    try std.testing.expect(refreshed.projectionReady().?.forceSnapshot());

    subject.testing.setClockSampleFn(&scheduler, 0, projection_interval_ns);
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
    const frame = try onlyProjectionFrame(&retry);
    try std.testing.expectEqual(advanced_revision, frame.revision);
    try std.testing.expectEqual(@as(u64, 1), frame.sequence);
}

test "THCR-009 attempted revision comes from capture rather than the scheduler sample" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: CaptureAdvanceProbe = .{};
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createHookedRuntime(&aggregate, connection_id, 0, probe.hooks());
    probe.runtime = fixture.runtime;
    const attachment_id = testId(core_module.AttachmentId, 0x57, 0);
    try activateAttachments(&aggregate, connection_id, fixture.terminal_id, &.{attachment_id});
    const scheduler_revision = fixture.runtime.geometry().revision;

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, 0, 0);
    try std.testing.expectEqual(
        host_outgoing.ScheduleOutcome.complete,
        try scheduler.schedule(
            std.testing.allocator,
            &aggregate,
            connection_id,
            0,
            &outgoing,
        ),
    );
    const captured_revision = fixture.runtime.geometry().revision;
    try std.testing.expectEqual(@as(usize, 1), probe.calls);
    try std.testing.expectEqual(scheduler_revision + 1, captured_revision);
    try std.testing.expectEqual(captured_revision, try onlyProjectionBlockRevision(&outgoing));
    const raw_block = aggregate.core().attachmentValues()[0].projectionBlocked().?;
    try std.testing.expectEqual(captured_revision, raw_block.observed_revision);
    try std.testing.expectEqual(
        core_module.ProjectionBlock.projection_too_large,
        raw_block.reason,
    );
}

test "THCR-018 failure publication rejects a provisional frame without scheduler advance" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: FailureCommitProbe = .{ .checkpoint = .before_lock };
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createHookedRuntime(&aggregate, connection_id, 0, probe.hooks());
    probe.runtime = fixture.runtime;
    defer probe.allow_containment = true;
    const attachment_ids = [_]core_module.AttachmentId{
        testId(core_module.AttachmentId, 0x58, 0),
        testId(core_module.AttachmentId, 0x58, 1),
    };
    try activateAttachments(&aggregate, connection_id, fixture.terminal_id, &attachment_ids);

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, 0, 0);
    const deadline_before = aggregate.nextProjectionAt(fixture.terminal_id).?;
    var call: ScheduleCall = .{
        .aggregate = &aggregate,
        .allocator = std.testing.allocator,
        .connection_id = connection_id,
        .maximum_frame_bytes = protocol.Limits.frame_bytes,
        .outgoing = &outgoing,
        .scheduler = &scheduler,
    };
    const thread = try std.Thread.spawn(.{}, ScheduleCall.run, .{&call});
    probe.checkpoint_gate.wait();
    const provisional_record_count = outgoing.records().len;
    const provisional_reserved_records = host_outgoing.testing.reservedRecordCount(&outgoing);
    const provisional_reserved_bytes = host_outgoing.testing.reservedByteCount(&outgoing);

    recordReaderFailure(fixture.runtime);
    const failure_before_release = fixture.runtime.primaryFailure();
    const readable_before_release = fixture.runtime.readableStateAvailable();
    const containment_calls_before_release = probe.containment_calls;
    probe.checkpoint_gate.release();
    thread.join();
    try call.expectComplete();

    try std.testing.expectEqual(@as(usize, 0), provisional_record_count);
    try std.testing.expectEqual(@as(usize, 1), provisional_reserved_records);
    try std.testing.expect(provisional_reserved_bytes > 0);
    try std.testing.expectEqual(
        terminal_runtime.RuntimeFailureKind.pty_read,
        failure_before_release.?,
    );
    try std.testing.expect(!readable_before_release);
    try std.testing.expectEqual(@as(usize, 1), containment_calls_before_release);
    const first = aggregate.core().attachmentValues()[0];
    const second = aggregate.core().attachmentValues()[1];
    try std.testing.expectEqual(@as(usize, 0), outgoing.records().len);
    try std.testing.expectEqual(@as(usize, 0), host_outgoing.testing.byteCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), host_outgoing.testing.reservedRecordCount(&outgoing));
    try std.testing.expectEqual(@as(usize, 0), host_outgoing.testing.reservedByteCount(&outgoing));
    try std.testing.expect(first.projectionInFlight() == null);
    try std.testing.expect(second.projectionInFlight() == null);
    try std.testing.expectEqual(@as(u64, 1), first.nextProjectionSequence());
    try std.testing.expectEqual(@as(u64, 1), second.nextProjectionSequence());
    try std.testing.expectEqual(deadline_before, aggregate.nextProjectionAt(fixture.terminal_id).?);
    try std.testing.expectEqual(@as(usize, 0), subject.testing.scanIndexFn(&scheduler));
}

test "THCR-018 winning read commit precedes a waiting failure publisher" {
    if (target_builtin.os.tag == .windows) return error.SkipZigTest;

    var probe: FailureCommitProbe = .{ .checkpoint = .locked };
    var aggregate = host_runtime_aggregate.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    const fixture = try createHookedRuntime(&aggregate, connection_id, 0, probe.hooks());
    probe.runtime = fixture.runtime;
    defer probe.allow_containment = true;
    const attachment_id = testId(core_module.AttachmentId, 0x59, 0);
    try activateAttachments(&aggregate, connection_id, fixture.terminal_id, &.{attachment_id});

    var outgoing = Outgoing.init(std.testing.allocator);
    defer outgoing.deinit();
    var scheduler: subject.Scheduler = .{};
    subject.testing.setClockSampleFn(&scheduler, 0, 0);
    var call: ScheduleCall = .{
        .aggregate = &aggregate,
        .allocator = std.testing.allocator,
        .connection_id = connection_id,
        .maximum_frame_bytes = protocol.Limits.frame_bytes,
        .outgoing = &outgoing,
        .scheduler = &scheduler,
    };
    const scheduler_thread = try std.Thread.spawn(.{}, ScheduleCall.run, .{&call});
    probe.checkpoint_gate.wait();
    const provisional_record_count = outgoing.records().len;
    const provisional_reserved_records = host_outgoing.testing.reservedRecordCount(&outgoing);
    const flight_before_release = aggregate.core().attachmentValues()[0].projectionInFlight();

    var failure_call: FailureCall = .{ .runtime = fixture.runtime };
    var scheduler_joined = false;
    errdefer {
        if (!scheduler_joined) {
            probe.checkpoint_gate.release();
            scheduler_thread.join();
        }
    }
    const failure_thread = try std.Thread.spawn(.{}, FailureCall.run, .{&failure_call});
    probe.failure_fence_starting.wait();
    const failure_before_release = fixture.runtime.primaryFailure();

    probe.checkpoint_gate.release();
    scheduler_thread.join();
    scheduler_joined = true;
    failure_thread.join();
    try call.expectComplete();
    try std.testing.expectEqual(@as(usize, 0), provisional_record_count);
    try std.testing.expectEqual(@as(usize, 1), provisional_reserved_records);
    try std.testing.expect(flight_before_release == null);
    try std.testing.expect(failure_before_release == null);
    try std.testing.expectEqual(@as(usize, 1), outgoing.records().len);
    _ = try onlyProjectionFrame(&outgoing);
    const attachment = aggregate.core().attachmentValues()[0];
    try std.testing.expect(attachment.projectionInFlight() != null);
    try std.testing.expectEqual(@as(u64, 2), attachment.nextProjectionSequence());
    try std.testing.expectEqual(
        @as(i128, projection_interval_ns),
        aggregate.nextProjectionAt(fixture.terminal_id).?,
    );
    try std.testing.expectEqual(
        terminal_runtime.RuntimeFailureKind.pty_read,
        fixture.runtime.primaryFailure().?,
    );
    try std.testing.expectEqual(@as(usize, 1), probe.containment_calls);
    try std.testing.expect(!fixture.runtime.readableStateAvailable());
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
