const std = @import("std");

const arguments = @import("ssh-runtime-arguments");
const project_root = @import("ssh-project-root-service");
const protocol = @import("ssh-environment-protocol");

test "launcher accepts only the exact public commands and isolated worker surface" {
    const instance_id = "0123456789abcdef0123456789abcdef";
    const serve = try arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    });
    try std.testing.expectEqualStrings(
        "/tmp/orchbee-runtime",
        serve.serve.runtime_directory,
    );
    try std.testing.expectEqualStrings(instance_id, serve.serve.runtime_instance_id);
    try std.testing.expectEqualStrings(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        serve.serve.runtime_build_digest,
    );

    const attach = try arguments.parse(&.{
        "attach",
        "--instance-id",
        instance_id,
        "--runtime-directory",
        "/tmp/orchbee-runtime",
    });
    try std.testing.expectEqualStrings(instance_id, attach.attach.runtime_instance_id);
    try std.testing.expectEqual(
        arguments.Command.project_root_worker,
        try arguments.parse(&.{"project-root-worker"}),
    );

    try std.testing.expectError(error.InvalidArguments, arguments.parse(&.{
        "execute",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidRuntimeInstanceId, arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        "NOT-A-RUNTIME-INSTANCE",
        "--build-digest",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidBuildDigest, arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        "not-a-build-digest",
    }));
}

test "protocol wire constants stay aligned with version 0.1.0" {
    try std.testing.expectEqual(@as(usize, 16), protocol.header_bytes);
    try std.testing.expectEqual(@as(usize, 1024 * 1024), protocol.maximum_frame_bytes);
    try std.testing.expectEqual(@as(u8, 1), @intFromEnum(protocol.Kind.hello));
    try std.testing.expectEqual(@as(u8, 2), @intFromEnum(protocol.Kind.hello_accepted));
    try std.testing.expectEqual(@as(u8, 18), @intFromEnum(protocol.Kind.channel_data));
    try std.testing.expectEqual(@as(u8, 19), @intFromEnum(protocol.Kind.channel_credit));
    try std.testing.expectEqual(@as(u8, 0), protocol.protocol_major);
    try std.testing.expectEqual(@as(u8, 1), protocol.protocol_minor);
    try std.testing.expectEqual(@as(u8, 0), protocol.protocol_patch);
    try std.testing.expectEqualStrings("project-root-v1", protocol.project_root_service_id);
}

test "project-root request decoder and path boundary are strict" {
    const allocator = std.testing.allocator;
    var request = try project_root.decodeRequest(
        allocator,
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/project\"}",
    );
    defer request.deinit();
    try std.testing.expectEqualStrings("/srv/project", request.value.path);

    for ([_][]const u8{
        "{\"type\":\"inspect-project-root\",\"path\":\"relative\"}",
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/project\",\"extra\":true}",
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/\\nproject\"}",
        "{\"type\":\"other\",\"path\":\"/srv/project\"}",
    }) |payload| {
        try std.testing.expectError(error.InvalidRequest, project_root.decodeRequest(allocator, payload));
    }
}
