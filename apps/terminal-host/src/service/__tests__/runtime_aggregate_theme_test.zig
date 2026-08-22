const harness = @import("runtime_aggregate_harness.zig");
const std = harness.std;
const core_module = harness.core_module;
const model = @import("terminal-host-model");
const subject = harness.subject;
const terminal_launch = harness.terminal_launch;
const terminal_runtime = harness.terminal_runtime;
const testId = harness.testId;
const validatedLongRunningLaunch = harness.validatedLongRunningLaunch;

const replacement_palette = [_]u32{0x22_33_44} ** 256;
const replacement: model.theme.Theme = .{
    .color_scheme = .light,
    .default_background_rgb = 0xee_ee_ee,
    .default_cursor_rgb = 0x12_34_56,
    .default_foreground_rgb = 0x11_11_11,
    .palette = replacement_palette,
};

test "runtime aggregate commits one complete theme and preserves it across replay stale and failure" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 1;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x61));
    const terminal_id = try createRunningTerminal(&aggregate, connection_id, 0x62);
    const runtime = aggregate.runtimeFor(terminal_id).?;
    const operation_id = operationId(0x63);

    const applied = try aggregate.applyTerminalTheme(
        connection_id,
        terminal_id,
        operation_id,
        0,
        replacement,
    );
    try std.testing.expectEqual(subject.TerminalThemeTransition.applied, applied.transition);
    try std.testing.expectEqual(@as(u64, 1), applied.current.revision);
    try std.testing.expect(model.theme.eql(replacement, applied.current.terminal));
    try std.testing.expect(model.theme.eql(replacement, runtime.themeDefaults()));
    const after_apply = runtime.geometry();

    const replay = try aggregate.applyTerminalTheme(
        connection_id,
        terminal_id,
        operation_id,
        0,
        replacement,
    );
    try std.testing.expectEqual(subject.TerminalThemeTransition.already_applied, replay.transition);
    try std.testing.expectEqualDeep(applied.current, replay.current);
    try std.testing.expectEqualDeep(after_apply, runtime.geometry());

    const stale = try aggregate.applyTerminalTheme(
        connection_id,
        terminal_id,
        operationId(0x64),
        0,
        model.theme.default,
    );
    try std.testing.expectEqual(subject.TerminalThemeTransition.stale, stale.transition);
    try std.testing.expectEqualDeep(applied.current, stale.current);
    try std.testing.expectEqualDeep(after_apply, runtime.geometry());

    try std.testing.expectError(
        error.ThemeOperationConflict,
        aggregate.applyTerminalTheme(
            connection_id,
            terminal_id,
            operation_id,
            1,
            model.theme.default,
        ),
    );
    var invalid = replacement;
    invalid.default_background_rgb = 0x01_00_00_00;
    try std.testing.expectError(
        error.InvalidTheme,
        aggregate.applyTerminalTheme(
            connection_id,
            terminal_id,
            operationId(0x65),
            1,
            invalid,
        ),
    );
    try std.testing.expectEqualDeep(applied.current, aggregate.core().findTerminal(terminal_id).?.themeState());
    try std.testing.expect(model.theme.eql(replacement, runtime.themeDefaults()));
}

test "runtime theme failure never commits the semantic theme revision" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 2;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x71));
    const terminal_id = try createRunningTerminal(&aggregate, connection_id, 0x72);
    const runtime = aggregate.runtimeFor(terminal_id).?;
    const before = aggregate.core().findTerminal(terminal_id).?.themeState();
    terminal_runtime.testing.stateForTesting(runtime).revision.store(std.math.maxInt(u64), .release);

    try std.testing.expectError(
        error.TerminalRevisionOverflow,
        aggregate.applyTerminalTheme(
            connection_id,
            terminal_id,
            operationId(0x73),
            0,
            replacement,
        ),
    );
    try std.testing.expectEqualDeep(before, aggregate.core().findTerminal(terminal_id).?.themeState());
    try std.testing.expect(!model.theme.eql(replacement, runtime.themeDefaults()));
}

test "deferred terminal theme commit updates the owned start preparation" {
    var aggregate = subject.HostRuntimeAggregate.init(std.testing.allocator);
    defer aggregate.deinitChecked(true) catch unreachable;
    const connection_id: core_module.ConnectionId = 3;
    _ = try aggregate.core().connect(connection_id, testId(core_module.ClientId, 0x81));
    var launch_fixture = try validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    var deferred_launch = launch_fixture.validated.launch;
    deferred_launch.geometry = null;
    const deferred = try terminal_launch.inspect(std.testing.allocator, deferred_launch);
    var create = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, 0x82),
        deferred,
    );
    defer create.deinit();
    const created = try aggregate.commitPreparedTerminal(&create);
    const terminal_id = created.identity().terminal_id;
    try std.testing.expect(aggregate.runtimeFor(terminal_id) == null);

    const result = try aggregate.applyTerminalTheme(
        connection_id,
        terminal_id,
        operationId(0x83),
        0,
        replacement,
    );
    try std.testing.expectEqual(subject.TerminalThemeTransition.applied, result.transition);

    var start = try aggregate.prepareStartTerminal(connection_id, terminal_id, .{
        .cell_pixel_height = 16,
        .cell_pixel_width = 8,
        .columns = 80,
        .rows = 24,
    });
    _ = try aggregate.commitPreparedTerminalStart(&start);
    try std.testing.expect(model.theme.eql(
        replacement,
        aggregate.runtimeFor(terminal_id).?.themeDefaults(),
    ));
}

fn createRunningTerminal(
    aggregate: *subject.HostRuntimeAggregate,
    connection_id: core_module.ConnectionId,
    id_byte: u8,
) !core_module.TerminalId {
    var launch_fixture = try validatedLongRunningLaunch();
    defer launch_fixture.deinit();
    const validated = launch_fixture.validated;
    var preparation = try aggregate.prepareCreateTerminal(
        connection_id,
        testId(core_module.CreationId, id_byte),
        validated,
    );
    defer preparation.deinit();
    return (try aggregate.commitPreparedTerminal(&preparation)).identity().terminal_id;
}

fn operationId(byte: u8) model.identifiers.TerminalThemeOperationId {
    return .fromBytes([_]u8{byte} ** 16);
}
