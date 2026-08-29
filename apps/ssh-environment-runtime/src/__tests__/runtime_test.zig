const std = @import("std");

const arguments = @import("ssh-runtime-arguments");
const project_root = @import("ssh-project-root-service");
const protocol = @import("ssh-environment-protocol");

test "launcher accepts only the exact public commands and isolated worker surface" {
    const instance_id = "0123456789abcdef0123456789abcdef";
    const endpoint_id = "fedcba9876543210fedcba9876543210";
    const runtime_digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const cohort_digest = "1111111111111111111111111111111111111111111111111111111111111111";
    const terminal_host_digest = "2222222222222222222222222222222222222222222222222222222222222222";
    const serve = try arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        runtime_digest,
        "--release-cohort-digest",
        cohort_digest,
        "--terminal-host-version",
        "0.1.0",
        "--terminal-host-build-digest",
        terminal_host_digest,
        "--terminal-host-executable",
        "/opt/orchbee/terminal-host",
        "--terminal-host-runtime-directory",
        "/tmp/orchbee-terminal-host",
        "--terminal-host-endpoint-id",
        endpoint_id,
    });
    try std.testing.expectEqualStrings(
        "/tmp/orchbee-runtime",
        serve.serve.runtime_directory,
    );
    try std.testing.expectEqualStrings(instance_id, serve.serve.runtime_instance_id);
    try std.testing.expectEqualStrings(
        runtime_digest,
        serve.serve.runtime_build_digest,
    );
    try std.testing.expectEqualStrings(cohort_digest, serve.serve.release_cohort_digest);
    try std.testing.expectEqualStrings(endpoint_id, serve.serve.terminal_host.endpoint_id);
    try std.testing.expectEqualStrings("0.1.0", serve.serve.terminal_host.version);

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
    const relay_worker = try arguments.parse(&.{
        "terminal-host-relay-worker",
        "--terminal-host-executable",
        "/opt/orchbee/terminal-host",
        "--terminal-host-runtime-directory",
        "/tmp/orchbee-terminal-host",
        "--terminal-host-endpoint-id",
        endpoint_id,
    });
    try std.testing.expectEqualStrings(
        endpoint_id,
        relay_worker.terminal_host_relay_worker.endpoint_id,
    );

    try std.testing.expectError(error.InvalidArguments, arguments.parse(&.{
        "execute",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        runtime_digest,
    }));
    try std.testing.expectError(error.InvalidRuntimeInstanceId, arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        "NOT-A-RUNTIME-INSTANCE",
        "--build-digest",
        runtime_digest,
        "--release-cohort-digest",
        cohort_digest,
        "--terminal-host-version",
        "0.1.0",
        "--terminal-host-build-digest",
        terminal_host_digest,
        "--terminal-host-executable",
        "/opt/orchbee/terminal-host",
        "--terminal-host-runtime-directory",
        "/tmp/orchbee-terminal-host",
        "--terminal-host-endpoint-id",
        endpoint_id,
    }));
    try std.testing.expectError(error.InvalidBuildDigest, arguments.parse(&.{
        "serve",
        "--runtime-directory",
        "/tmp/orchbee-runtime",
        "--instance-id",
        instance_id,
        "--build-digest",
        "not-a-build-digest",
        "--release-cohort-digest",
        cohort_digest,
        "--terminal-host-version",
        "0.1.0",
        "--terminal-host-build-digest",
        terminal_host_digest,
        "--terminal-host-executable",
        "/opt/orchbee/terminal-host",
        "--terminal-host-runtime-directory",
        "/tmp/orchbee-terminal-host",
        "--terminal-host-endpoint-id",
        endpoint_id,
    }));
}

test "protocol wire constants stay aligned with version 0.3.0" {
    try std.testing.expectEqual(@as(usize, 16), protocol.header_bytes);
    try std.testing.expectEqual(@as(usize, 1024 * 1024), protocol.maximum_frame_bytes);
    try std.testing.expectEqual(@as(u8, 1), @intFromEnum(protocol.Kind.hello));
    try std.testing.expectEqual(@as(u8, 2), @intFromEnum(protocol.Kind.hello_accepted));
    try std.testing.expectEqual(@as(u8, 18), @intFromEnum(protocol.Kind.channel_data));
    try std.testing.expectEqual(@as(u8, 19), @intFromEnum(protocol.Kind.channel_credit));
    try std.testing.expectEqual(@as(u8, 0), protocol.protocol_major);
    try std.testing.expectEqual(@as(u8, 3), protocol.protocol_minor);
    try std.testing.expectEqual(@as(u8, 0), protocol.protocol_patch);
    try std.testing.expectEqualStrings("project-root-v2", protocol.project_root_service_id);
    try std.testing.expectEqualStrings("terminal-host-v1", protocol.terminal_host_service_id);
    try std.testing.expectEqual(@as(usize, 65_536), protocol.terminal_host_maximum_chunk_bytes);
}

test "project-root request decoder and path boundary are strict" {
    const allocator = std.testing.allocator;
    var request = try project_root.decodeRequest(
        allocator,
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/project\"}",
    );
    defer request.deinit();
    try std.testing.expectEqualStrings("/srv/project", request.value.path.?);

    var home_listing = try project_root.decodeRequest(
        allocator,
        "{\"type\":\"list-project-directory\",\"path\":null}",
    );
    defer home_listing.deinit();
    try std.testing.expectEqual(project_root.RequestKind.list, project_root.requestKind(home_listing.value));
    try std.testing.expect(home_listing.value.path == null);

    for ([_][]const u8{
        "{\"type\":\"inspect-project-root\",\"path\":\"relative\"}",
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/project\",\"extra\":true}",
        "{\"type\":\"inspect-project-root\",\"path\":\"/srv/\\nproject\"}",
        "{\"type\":\"other\",\"path\":\"/srv/project\"}",
    }) |payload| {
        try std.testing.expectError(error.InvalidRequest, project_root.decodeRequest(allocator, payload));
    }
}
