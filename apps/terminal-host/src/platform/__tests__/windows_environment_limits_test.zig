const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");

const subject = platform.process.testing.windows_environment;
const EnvironmentDelta = platform.process.EnvironmentDelta;
const EnvironmentMap = std.process.Environ.Map;

test "Windows terminal environment enforces aggregate snapshot count and byte bounds" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();

    const values = try std.testing.allocator.alloc(
        subject.Value,
        subject.limits.registry_values + 1,
    );
    defer std.testing.allocator.free(values);
    for (values) |*value| {
        value.* = .{ .name = "SAME", .value = "", .kind = .literal };
    }
    const count_split = subject.limits.registry_values / 2;
    var at_count_limit = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = values[0..count_split] },
            .user = .{ .values = values[count_split..subject.limits.registry_values] },
        },
        &.{},
    );
    at_count_limit.deinit();
    try std.testing.expectError(
        error.RegistrySnapshotTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{
                .machine = .{ .values = values[0..count_split] },
                .user = .{ .values = values[count_split..] },
            },
            &.{},
        ),
    );

    const entry_overhead = 2 * (1 + 2);
    const first_value_length = (subject.limits.registry_snapshot_bytes - entry_overhead) / 2;
    const second_value_length = subject.limits.registry_snapshot_bytes -
        entry_overhead - first_value_length;
    const first_value = try std.testing.allocator.alloc(u8, first_value_length);
    defer std.testing.allocator.free(first_value);
    @memset(first_value, 'x');
    const second_value = try std.testing.allocator.alloc(u8, second_value_length + 1);
    defer std.testing.allocator.free(second_value);
    @memset(second_value, 'y');
    const exact = [_]subject.Value{
        .{ .name = "A", .value = first_value, .kind = .literal },
        .{ .name = "B", .value = second_value[0..second_value_length], .kind = .literal },
    };
    var at_byte_limit = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = exact[0..1] },
            .user = .{ .values = exact[1..] },
        },
        &.{},
    );
    at_byte_limit.deinit();
    const over = [_]subject.Value{
        .{ .name = "A", .value = first_value, .kind = .literal },
        .{ .name = "B", .value = second_value, .kind = .literal },
    };
    try std.testing.expectError(
        error.RegistrySnapshotTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{
                .machine = .{ .values = over[0..1] },
                .user = .{ .values = over[1..] },
            },
            &.{},
        ),
    );

    const single_oversized = try std.testing.allocator.alloc(
        u8,
        subject.limits.registry_value_bytes + 1,
    );
    defer std.testing.allocator.free(single_oversized);
    @memset(single_oversized, 'z');
    const oversized_value = [_]subject.Value{.{
        .name = "OVERSIZED",
        .value = single_oversized,
        .kind = .literal,
    }};
    try std.testing.expectError(
        error.RegistrySnapshotTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{ .machine = .{ .values = &oversized_value }, .user = .missing },
            &.{},
        ),
    );
}

test "Windows terminal environment enforces expansion output bounds" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    const half = subject.limits.expanded_value_bytes / 2;
    const dependency = try std.testing.allocator.alloc(u8, half + 1);
    defer std.testing.allocator.free(dependency);
    @memset(dependency, 'd');

    const exact = [_]subject.Value{
        .{ .name = "DEPENDENCY", .value = dependency[0..half], .kind = .literal },
        .{ .name = "EXPANDED", .value = "%DEPENDENCY%%DEPENDENCY%", .kind = .expandable },
    };
    var at_limit = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .{ .values = &exact }, .user = .missing },
        &.{},
    );
    defer at_limit.deinit();
    try std.testing.expectEqual(
        subject.limits.expanded_value_bytes,
        at_limit.get("EXPANDED").?.len,
    );

    const over = [_]subject.Value{
        .{ .name = "DEPENDENCY", .value = dependency, .kind = .literal },
        .{ .name = "EXPANDED", .value = "%DEPENDENCY%%DEPENDENCY%", .kind = .expandable },
    };
    try std.testing.expectError(
        error.RegistryExpansionTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{ .machine = .{ .values = &over }, .user = .missing },
            &.{},
        ),
    );
}

test "Windows terminal environment bounds combined PATH and final projection" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();

    const machine_length = subject.limits.expanded_value_bytes / 2;
    const user_length = subject.limits.expanded_value_bytes - machine_length - 1;
    const machine_path = try std.testing.allocator.alloc(u8, machine_length);
    defer std.testing.allocator.free(machine_path);
    @memset(machine_path, 'a');
    const user_path = try std.testing.allocator.alloc(u8, user_length + 1);
    defer std.testing.allocator.free(user_path);
    @memset(user_path, 'b');
    const machine = [_]subject.Value{.{
        .name = "Path",
        .value = machine_path,
        .kind = .literal,
    }};
    const exact_user = [_]subject.Value{.{
        .name = "PATH",
        .value = user_path[0..user_length],
        .kind = .literal,
    }};
    var exact_path = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = &machine },
            .user = .{ .values = &exact_user },
        },
        &.{},
    );
    defer exact_path.deinit();
    try std.testing.expectEqual(
        subject.limits.expanded_value_bytes,
        exact_path.get("PATH").?.len,
    );
    const over_user = [_]subject.Value{.{
        .name = "PATH",
        .value = user_path,
        .kind = .literal,
    }};
    try std.testing.expectError(
        error.EnvironmentProjectionTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{
                .machine = .{ .values = &machine },
                .user = .{ .values = &over_user },
            },
            &.{},
        ),
    );

    const private_name = "ORCHBEE_PRIVATE_BOUNDARY";
    const private_value_length = subject.limits.projected_environment_bytes -
        private_name.len - 3;
    const private_value = try std.testing.allocator.alloc(u8, private_value_length + 1);
    defer std.testing.allocator.free(private_value);
    @memset(private_value, 'p');
    try inherited.put(private_name, private_value[0..private_value_length]);
    var exact_environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .missing, .user = .missing },
        &.{},
    );
    exact_environment.deinit();
    try inherited.put(private_name, private_value);
    try std.testing.expectError(
        error.EnvironmentProjectionTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{ .machine = .missing, .user = .missing },
            &.{},
        ),
    );
    _ = inherited.orderedRemove(private_name);

    var arena_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const delta = try arena.alloc(
        EnvironmentDelta,
        subject.limits.projected_environment_entries + 1,
    );
    for (delta, 0..) |*entry, index| {
        entry.* = .{
            .name = try std.fmt.allocPrint(arena, "BOUND_{d}", .{index}),
            .value = "",
        };
    }
    var exact_count = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .missing, .user = .missing },
        delta[0..subject.limits.projected_environment_entries],
    );
    exact_count.deinit();
    try std.testing.expectError(
        error.EnvironmentProjectionTooLarge,
        subject.buildEnvironment(
            std.testing.allocator,
            &inherited,
            .{ .machine = .missing, .user = .missing },
            delta,
        ),
    );
}
