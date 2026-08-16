const harness = @import("service_harness.zig");
const aggregate_harness = @import("runtime_aggregate_harness.zig");

const std = harness.std;
const core_module = harness.core_module;
const reference = harness.host_composition_reference;
const semantic_trace = harness.host_composition_trace;
const ConnectionState = harness.ConnectionState;
const HostService = harness.HostService;
const testId = harness.testId;

// Stateful refinement bridge for TH-001 through TH-007, TH-010 through
// TH-012, and TH-016. Action names are the runtime-model transition
// vocabulary; every step runs owner-local and cross-owner invariants before
// comparing the product observation with the executable reference reducer.
test "semantic trace refines a native HostService composition" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    var refinement = try reference.Refinement.initService(
        std.testing.allocator,
        &service,
        &connection,
    );
    defer refinement.deinit();

    try harness.testHandshake(&service, &connection, "22222222222222222222222222222222");
    var active = try harness.activeConnection(&connection);
    try refinement.stepService(&service, &connection, .{ .connect = .{
        .client_id = core_module.ClientId.fromBytes([_]u8{0x22} ** 16),
        .connection_id = active.connection_id,
        .limits = .{
            .frame_bytes = active.limits.frame_bytes,
            .input_bytes = active.limits.input_bytes,
            .row_window_bytes = active.limits.row_window_bytes,
        },
    } }, .applied);

    const validated = try aggregate_harness.validatedLongRunningLaunch();
    const creation_id = testId(core_module.CreationId, 0xa1, 0);
    var preparation = try service.runtime_aggregate.prepareCreateTerminal(
        active.connection_id,
        creation_id,
        validated,
    );
    defer preparation.deinit();
    const terminal_id = switch (try service.runtime_aggregate.commitPreparedTerminal(&preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    const create_action: semantic_trace.Create = .{
        .configuration = validated.launch.configuration orelse
            service.runtime_aggregate.core().terminalDefaults(),
        .creation_id = creation_id,
        .geometry = validated.launch.geometry,
        .launch_digest = validated.launch_digest,
        .terminal_id = terminal_id,
    };
    try refinement.stepService(
        &service,
        &connection,
        .{ .create = create_action },
        .applied,
    );

    const first_attachment = testId(core_module.AttachmentId, 0xa1, 1);
    const first_attach: semantic_trace.Attach = .{
        .attachment_id = first_attachment,
        .capabilities = .{ .input = true, .read = true, .resize = true },
        .request_resize_lease = true,
        .terminal_id = terminal_id,
    };
    _ = try service.runtime_aggregate.attach(
        active.connection_id,
        first_attachment,
        terminal_id,
        first_attach.capabilities,
        first_attach.request_resize_lease,
    );
    try refinement.stepService(&service, &connection, .{ .attach = first_attach }, .applied);

    const second_attachment = testId(core_module.AttachmentId, 0xa1, 2);
    const second_attach: semantic_trace.Attach = .{
        .attachment_id = second_attachment,
        .capabilities = .{ .input = false, .read = false, .resize = true },
        .request_resize_lease = false,
        .terminal_id = terminal_id,
    };
    _ = try service.runtime_aggregate.attach(
        active.connection_id,
        second_attachment,
        terminal_id,
        second_attach.capabilities,
        second_attach.request_resize_lease,
    );
    try refinement.stepService(&service, &connection, .{ .attach = second_attach }, .applied);
    _ = try service.runtime_aggregate.attach(
        active.connection_id,
        second_attachment,
        terminal_id,
        second_attach.capabilities,
        second_attach.request_resize_lease,
    );
    try refinement.stepService(&service, &connection, .{ .attach = second_attach }, .replayed);

    const invalid_attach: semantic_trace.Attach = .{
        .attachment_id = testId(core_module.AttachmentId, 0xa1, 3),
        .capabilities = .{ .input = false, .read = false, .resize = false },
        .request_resize_lease = false,
        .terminal_id = terminal_id,
    };
    try std.testing.expectError(
        error.InvalidCapabilities,
        service.runtime_aggregate.attach(
            active.connection_id,
            invalid_attach.attachment_id,
            terminal_id,
            invalid_attach.capabilities,
            false,
        ),
    );
    try refinement.stepService(&service, &connection, .{ .attach = invalid_attach }, .rejected);

    try std.testing.expectEqual(
        core_module.TransitionOutcome.applied,
        try service.runtime_aggregate.setProjectionDemand(
            active.connection_id,
            first_attachment,
            .active,
        ),
    );
    try refinement.stepService(&service, &connection, .{ .set_projection_demand = .{
        .attachment_id = first_attachment,
        .demand = .active,
    } }, .applied);
    var outgoing = try harness.pollEventsChecked(&service, &connection);
    defer outgoing.deinit();
    try refinement.stepService(&service, &connection, .poll, .applied);

    const updated_configuration = harness.terminal_configuration.Configuration{
        .scrollback_bytes = 1_000_000,
    };
    _ = try service.runtime_aggregate.updateTerminalConfiguration(
        active.connection_id,
        terminal_id,
        updated_configuration,
    );
    try refinement.stepService(&service, &connection, .{ .update_configuration = .{
        .configuration = updated_configuration,
        .terminal_id = terminal_id,
    } }, .applied);

    const updated_geometry = harness.model_boundary.geometry.Geometry{
        .cell_pixel_height = 18,
        .cell_pixel_width = 9,
        .columns = 100,
        .rows = 30,
    };
    _ = try service.runtime_aggregate.resizeTerminal(
        active.connection_id,
        first_attachment,
        updated_geometry,
    );
    try refinement.stepService(&service, &connection, .{ .resize = .{
        .attachment_id = first_attachment,
        .geometry = updated_geometry,
    } }, .applied);

    _ = try service.runtime_aggregate.applyTerminalControl(
        active.connection_id,
        first_attachment,
        .clear,
    );
    try refinement.stepService(&service, &connection, .{ .apply_terminal_control = .{
        .attachment_id = first_attachment,
        .operation = .clear,
    } }, .applied);

    const input = service.runtime_aggregate.acceptRawInput(
        active.connection_id,
        first_attachment,
        "x",
    );
    switch (input) {
        .accepted => {},
        .core_rejected, .runtime_missing, .runtime_rejected => return error.ExpectedAcceptedInput,
    }
    try refinement.stepService(&service, &connection, .{ .accept_input = .{
        .attachment_id = first_attachment,
        .byte_count = 1,
    } }, .unknown);
    try refinement.stepService(&service, &connection, .reconcile, .applied);

    _ = try service.runtime_aggregate.detach(active.connection_id, first_attachment);
    try refinement.stepService(&service, &connection, .{ .detach = first_attachment }, .applied);
    _ = try service.runtime_aggregate.detach(active.connection_id, second_attachment);
    try refinement.stepService(&service, &connection, .{ .detach = second_attachment }, .applied);

    const first_connection_id = active.connection_id;
    service.disconnect(&connection);
    try refinement.stepService(
        &service,
        &connection,
        .{ .disconnect = first_connection_id },
        .applied,
    );
    try harness.testHandshake(&service, &connection, "23232323232323232323232323232323");
    active = try harness.activeConnection(&connection);
    try refinement.stepService(&service, &connection, .{ .connect = .{
        .client_id = core_module.ClientId.fromBytes([_]u8{0x23} ** 16),
        .connection_id = active.connection_id,
        .limits = .{
            .frame_bytes = active.limits.frame_bytes,
            .input_bytes = active.limits.input_bytes,
            .row_window_bytes = active.limits.row_window_bytes,
        },
    } }, .applied);

    var replay = try service.runtime_aggregate.prepareCreateTerminal(
        active.connection_id,
        creation_id,
        validated,
    );
    defer replay.deinit();
    const replayed_id = switch (try service.runtime_aggregate.commitPreparedTerminal(&replay)) {
        .replayed => |identity| identity.terminal_id,
        .created => return error.ExpectedReplayedTerminal,
    };
    try std.testing.expect(replayed_id.eql(terminal_id));
    try refinement.stepService(
        &service,
        &connection,
        .{ .create = create_action },
        .replayed,
    );

    try std.testing.expectEqual(@as(usize, 18), refinement.trace.items().len);
    try std.testing.expectEqualStrings("Create", refinement.trace.items()[17].action.name());
    try std.testing.expectEqual(semantic_trace.Outcome.replayed, refinement.trace.items()[17].outcome);
}

test "semantic trace vocabulary uses stable unique names" {
    for (semantic_trace.vocabulary, 0..) |name, index| {
        try std.testing.expect(name.len > 0);
        for (semantic_trace.vocabulary[index + 1 ..]) |candidate| {
            try std.testing.expect(!std.mem.eql(u8, name, candidate));
        }
    }
    const poll: semantic_trace.Action = .poll;
    const reconcile: semantic_trace.Action = .reconcile;
    try std.testing.expectEqualStrings("Poll", poll.name());
    try std.testing.expectEqualStrings("Reconcile", reconcile.name());
}
