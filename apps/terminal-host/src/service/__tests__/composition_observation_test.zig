const harness = @import("service_harness.zig");
const aggregate_harness = @import("runtime_aggregate_harness.zig");

const std = harness.std;
const composition = harness.host_composition_observation;
const core_module = harness.core_module;
const ConnectionState = harness.ConnectionState;
const HostService = harness.HostService;
const expectCompositionInvariants = harness.expectCompositionInvariants;
const testHandshake = harness.testHandshake;
const testId = harness.testId;

test "composition observation checks Core runtime and attachment owners together" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }

    try expectCompositionInvariants(&service, &connection);
    try testHandshake(&service, &connection, "91919191919191919191919191919191");
    const active = try harness.activeConnection(&connection);

    var launch_fixture = try aggregate_harness.validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var preparation = try service.runtime_aggregate.prepareCreateTerminal(
        active.connection_id,
        testId(core_module.CreationId, 0x91, 0),
        validated,
    );
    defer preparation.deinit();
    const terminal_id = switch (try service.runtime_aggregate.commitPreparedTerminal(&preparation)) {
        .created => |identity| identity.terminal_id,
        .replayed => return error.ExpectedCreatedTerminal,
    };
    try expectCompositionInvariants(&service, &connection);

    const attachment_id = testId(core_module.AttachmentId, 0x91, 1);
    _ = try service.runtime_aggregate.attach(
        active.connection_id,
        attachment_id,
        terminal_id,
        .{ .input = true, .read = true, .resize = true },
        true,
    );
    try expectCompositionInvariants(&service, &connection);

    var observation = try composition.capture(
        std.testing.allocator,
        &service,
        &connection,
    );
    defer observation.deinit();
    try std.testing.expectEqual(@as(usize, 1), observation.runtime_count);
    try std.testing.expectEqual(@as(usize, 1), observation.terminals.len);
    try std.testing.expectEqual(@as(usize, 1), observation.attachments.len);
    try std.testing.expect(observation.attachments[0].runtime_has_input_ledger);
    try std.testing.expect(observation.attachments[0].runtime_has_history_state);

    _ = try service.runtime_aggregate.detach(active.connection_id, attachment_id);
    try expectCompositionInvariants(&service, &connection);
}

test "composition observation admits explicit model-only service fixtures" {
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .awaiting_hello;
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshake(&service, &connection, "92929292929292929292929292929292");
    const active = try harness.activeConnection(&connection);
    const terminal_id = testId(core_module.TerminalId, 0x92, 0);
    _ = try core_module.testing.createTerminal(
        service.runtime_aggregate.core(),
        active.connection_id,
        testId(core_module.CreationId, 0x92, 1),
        terminal_id,
        [_]u8{0x92} ** 32,
        80,
        24,
    );

    try expectCompositionInvariants(&service, &connection);
    var observation = try composition.capture(
        std.testing.allocator,
        &service,
        &connection,
    );
    defer observation.deinit();
    try std.testing.expect(observation.terminals[0].model_only);
    try std.testing.expect(observation.terminals[0].runtime == null);
}
