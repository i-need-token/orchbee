const std = @import("std");
const machine = @import("composition_machine_harness.zig");
const property = @import("composition_property_harness.zig");

// Generated state-machine refinement suite for TH-001 through TH-013 and
// TH-016 through TH-020 as applicable. Every candidate starts from a fresh HostService, executes the stable
// semantic vocabulary, and checks the composition oracle plus reference model
// after every applied, replayed, or rejected action. A failure is delta-debugged
// and emitted as an exact one-byte replay string.
test "generated HostService traces refine the executable composition model" {
    if (std.process.hasEnvVarConstant(property.replay_environment)) {
        const encoded = try std.process.getEnvVarOwned(
            std.testing.allocator,
            property.replay_environment,
        );
        defer std.testing.allocator.free(encoded);
        const commands = try property.decode(std.testing.allocator, encoded);
        defer std.testing.allocator.free(commands);
        return runWithShrink(0, commands);
    }

    if (std.process.hasEnvVarConstant(property.seed_environment)) {
        const encoded_seed = try std.process.getEnvVarOwned(
            std.testing.allocator,
            property.seed_environment,
        );
        defer std.testing.allocator.free(encoded_seed);
        const seed = try std.fmt.parseInt(u64, encoded_seed, 0);
        const commands = try property.generate(std.testing.allocator, seed, 96);
        defer std.testing.allocator.free(commands);
        return runWithShrink(seed, commands);
    }

    const seeds = [_]u64{
        0x243f6a8885a308d3,
        0x13198a2e03707344,
        0xa4093822299f31d0,
        0x082efa98ec4e6c89,
        0x452821e638d01377,
        0xbe5466cf34e90c6c,
    };
    for (seeds) |seed| {
        const commands = try property.generate(std.testing.allocator, seed, 96);
        defer std.testing.allocator.free(commands);
        try runWithShrink(seed, commands);
    }
}

test "property replay codec is exact and rejects unknown commands" {
    const expected = [_]property.Command{
        .model_create,
        .model_child_exit,
        .model_pty_drain,
        .model_publish,
        .model_dispose,
    };
    const encoded = try property.encode(std.testing.allocator, &expected);
    defer std.testing.allocator.free(encoded);
    const decoded = try property.decode(std.testing.allocator, encoded);
    defer std.testing.allocator.free(decoded);
    try std.testing.expectEqualSlices(property.Command, &expected, decoded);
    try std.testing.expectError(
        error.InvalidReplayCommand,
        property.decode(std.testing.allocator, "?"),
    );
}

test "property shrinker removes irrelevant ranges from a failing trace" {
    const original = [_]property.Command{
        .native_attach_owner,
        .model_create,
        .native_input,
        .model_failure,
        .poll,
        .model_contain,
        .native_reset,
    };
    const minimized = try property.shrink(
        std.testing.allocator,
        &original,
        {},
        syntheticFailure,
    );
    defer std.testing.allocator.free(minimized);
    try std.testing.expectEqual(@as(usize, 2), minimized.len);
    try std.testing.expectEqual(property.Command.model_failure, minimized[0]);
    try std.testing.expectEqual(property.Command.model_contain, minimized[1]);
}

fn runWithShrink(seed: u64, commands: []const property.Command) !void {
    machine.run(commands) catch |original_error| {
        const minimized = try property.shrink(
            std.testing.allocator,
            commands,
            {},
            hostTraceFails,
        );
        defer std.testing.allocator.free(minimized);
        const replay = try property.encode(std.testing.allocator, minimized);
        defer std.testing.allocator.free(replay);
        std.debug.print(
            "HostService property failure: seed=0x{x}, {s}={s}, error={s}\n",
            .{ seed, property.replay_environment, replay, @errorName(original_error) },
        );
        return original_error;
    };
}

fn hostTraceFails(_: void, commands: []const property.Command) bool {
    machine.run(commands) catch return true;
    return false;
}

fn syntheticFailure(_: void, commands: []const property.Command) bool {
    var failure_index: ?usize = null;
    for (commands, 0..) |command, index| {
        if (command == .model_failure and failure_index == null) failure_index = index;
        if (command == .model_contain and failure_index != null and failure_index.? < index) {
            return true;
        }
    }
    return false;
}
