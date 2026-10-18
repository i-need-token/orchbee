const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");

const subject = platform.process.testing.windows_environment;
const EnvironmentDelta = platform.process.EnvironmentDelta;
const EnvironmentMap = std.process.Environ.Map;

test "Windows terminal environment refreshes registry PATH and preserves process-only values" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    try inherited.put("Path", "C:\\stale-service-bin");
    try inherited.put("USERPROFILE", "C:\\Users\\alice");
    try inherited.put("USERNAME", "alice");
    try inherited.put("ComSpec", "C:\\Windows\\System32\\cmd.exe");

    const machine = [_]subject.Value{
        .{ .name = "SystemRoot", .value = "C:\\Windows", .kind = .literal },
        .{ .name = "PATH", .value = "%SystemRoot%\\System32;C:\\Tools;c:\\tools", .kind = .expandable },
        .{ .name = "USERNAME", .value = "SYSTEM", .kind = .literal },
        .{ .name = "ScopeValue", .value = "machine", .kind = .literal },
    };
    const user = [_]subject.Value{
        .{ .name = "path", .value = "%USERPROFILE%\\bin;C:\\TOOLS;D:\\User", .kind = .expandable },
        .{ .name = "scopevalue", .value = "user", .kind = .literal },
    };
    var environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = &machine },
            .user = .{ .values = &user },
        },
        &.{},
    );
    defer environment.deinit();

    try std.testing.expectEqualStrings(
        "C:\\Windows\\System32;C:\\Tools;C:\\Users\\alice\\bin;D:\\User",
        environment.get("PATH").?,
    );
    try std.testing.expectEqualStrings("alice", environment.get("username").?);
    try std.testing.expectEqualStrings(
        "C:\\Windows\\System32\\cmd.exe",
        environment.get("COMSPEC").?,
    );
    try std.testing.expectEqualStrings("user", environment.get("SCOPEVALUE").?);
}

test "Windows terminal environment resolves dependencies and bounds cycles and unknown names" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    const values = [_]subject.Value{
        .{ .name = "ROOT", .value = "C:\\base", .kind = .literal },
        .{ .name = "LITERAL", .value = "%ROOT%\\literal", .kind = .literal },
        .{ .name = "FIRST", .value = "%root%\\first", .kind = .expandable },
        .{ .name = "SECOND", .value = "%First%\\second", .kind = .expandable },
        .{ .name = "UNKNOWN", .value = "%DOES_NOT_EXIST%\\tail", .kind = .expandable },
        .{ .name = "SELF", .value = "%self%\\tail", .kind = .expandable },
        .{ .name = "CYCLE_A", .value = "%cycle_b%", .kind = .expandable },
        .{ .name = "CYCLE_B", .value = "%cycle_a%", .kind = .expandable },
    };
    var environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = &values },
            .user = .missing,
        },
        &.{},
    );
    defer environment.deinit();

    try std.testing.expectEqualStrings("C:\\base\\first", environment.get("first").?);
    try std.testing.expectEqualStrings("%ROOT%\\literal", environment.get("LITERAL").?);
    try std.testing.expectEqualStrings(
        "C:\\base\\first\\second",
        environment.get("SECOND").?,
    );
    try std.testing.expectEqualStrings(
        "%DOES_NOT_EXIST%\\tail",
        environment.get("UNKNOWN").?,
    );
    try std.testing.expectEqualStrings("%self%\\tail", environment.get("SELF").?);
    try std.testing.expectEqualStrings("%cycle_a%", environment.get("CYCLE_A").?);
    try std.testing.expectEqualStrings("%cycle_b%", environment.get("CYCLE_B").?);
}

test "Windows terminal environment gives launch entries and session overlays final priority" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    try inherited.put("MARKER", "inherited");
    try inherited.put("PATH", "inherited-path");
    try inherited.put("ORCHBEE_TERMINAL_ID", "inherited-terminal");
    try inherited.put("REMOVED", "inherited");
    const machine = [_]subject.Value{
        .{ .name = "MARKER", .value = "machine", .kind = .literal },
        .{ .name = "Path", .value = "machine-path", .kind = .literal },
        .{ .name = "REMOVED", .value = "machine", .kind = .literal },
    };
    const user = [_]subject.Value{
        .{ .name = "marker", .value = "user", .kind = .literal },
        .{ .name = "PATH", .value = "user-path", .kind = .literal },
    };
    const delta = [_]EnvironmentDelta{
        .{ .name = "Marker", .value = "explicit" },
        .{ .name = "path", .value = "explicit-path" },
        .{ .name = "ORCHBEE_TERMINAL_ID", .value = "current-terminal" },
        .{ .name = "removed", .value = null },
    };
    var environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = &machine },
            .user = .{ .values = &user },
        },
        &delta,
    );
    defer environment.deinit();

    try std.testing.expectEqualStrings("explicit", environment.get("MARKER").?);
    try std.testing.expectEqualStrings("explicit-path", environment.get("PATH").?);
    try std.testing.expectEqualStrings(
        "current-terminal",
        environment.get("orchbee_terminal_id").?,
    );
    try std.testing.expect(environment.get("REMOVED") == null);
}

test "Windows terminal environment expands each PATH value according to its own kind" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    const values = [_]subject.Value{
        .{ .name = "ROOT", .value = "C:\\base", .kind = .literal },
        .{ .name = "PATH", .value = "%ROOT%\\literal", .kind = .literal },
    };
    const user = [_]subject.Value{
        .{ .name = "Path", .value = "%root%\\expanded", .kind = .expandable },
    };
    var environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{
            .machine = .{ .values = &values },
            .user = .{ .values = &user },
        },
        &.{},
    );
    defer environment.deinit();

    try std.testing.expectEqualStrings(
        "%ROOT%\\literal;C:\\base\\expanded",
        environment.get("PATH").?,
    );
}

test "Windows terminal environment handles missing scopes and fails on registry errors" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    try inherited.put("PATH", "inherited-path");
    var environment = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .missing, .user = .missing },
        &.{.{ .name = "EXPLICIT", .value = "value" }},
    );
    defer environment.deinit();
    try std.testing.expectEqualStrings("inherited-path", environment.get("path").?);
    try std.testing.expectEqualStrings("value", environment.get("explicit").?);

    for ([_]subject.Snapshot{
        .{ .machine = .unavailable, .user = .missing },
        .{ .machine = .missing, .user = .unavailable },
    }) |snapshot| {
        try std.testing.expectError(
            error.RegistrySnapshotUnavailable,
            subject.buildEnvironment(std.testing.allocator, &inherited, snapshot, &.{}),
        );
    }
}

test "Windows terminal environment consumes the current snapshot for each launch" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var inherited = EnvironmentMap.init(std.testing.allocator);
    defer inherited.deinit();
    try inherited.put("PATH", "service-start-path");
    const first_path = [_]subject.Value{
        .{ .name = "Path", .value = "C:\\first", .kind = .literal },
    };
    const second_path = [_]subject.Value{
        .{ .name = "PATH", .value = "C:\\second", .kind = .literal },
    };
    var first = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .{ .values = &first_path }, .user = .missing },
        &.{},
    );
    defer first.deinit();
    var second = try subject.buildEnvironment(
        std.testing.allocator,
        &inherited,
        .{ .machine = .{ .values = &second_path }, .user = .missing },
        &.{},
    );
    defer second.deinit();
    try std.testing.expectEqualStrings("C:\\first", first.get("PATH").?);
    try std.testing.expectEqualStrings("C:\\second", second.get("PATH").?);
}
