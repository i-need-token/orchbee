const builtin = @import("builtin");
const std = @import("std");

const required_zig_version = "0.16.0";
const default_runtime_version = "0.2.0";
const authored_c_flags: []const []const u8 = &.{
    "-std=c17",
    "-Wall",
    "-Wextra",
    "-Wpedantic",
    "-Werror=incompatible-pointer-types",
};

pub fn build(builder: *std.Build) void {
    if (!std.mem.eql(u8, builtin.zig_version_string, required_zig_version)) {
        std.debug.panic(
            "ssh-environment-runtime requires Zig {s}, received {s}",
            .{ required_zig_version, builtin.zig_version_string },
        );
    }

    var target = builder.standardTargetOptions(.{});
    // Zig's bundled MSVC headers fail to compile on some Windows SDK
    // configurations, and Zig links with its own lld anyway, so fall back to
    // the GNU ABI unless one was requested explicitly.
    if (target.result.os.tag == .windows and target.query.abi == null) {
        var query = target.query;
        query.abi = .gnu;
        target = builder.resolveTargetQuery(query);
    }
    const optimize = builder.standardOptimizeOption(.{});
    const strip = builder.option(
        bool,
        "strip",
        "Strip the final executable; production Runtime releases require true",
    ) orelse false;
    const runtime_platform = supportedPlatform(target.result);
    const runtime_version = builder.option(
        []const u8,
        "runtime-version",
        "Semantic version embedded in orchbee-ssh-environment-runtime",
    ) orelse default_runtime_version;
    _ = std.SemanticVersion.parse(runtime_version) catch
        std.debug.panic("invalid SSH Environment Runtime semantic version: {s}", .{runtime_version});
    if (runtime_version.len > 64) {
        std.debug.panic("SSH Environment Runtime semantic version exceeds 64 bytes", .{});
    }
    const build_options = builder.addOptions();
    build_options.addOption([]const u8, "runtime_version", runtime_version);
    build_options.addOption([]const u8, "runtime_platform", runtime_platform);

    const arguments_module = builder.createModule(.{
        .root_source_file = builder.path("src/arguments.zig"),
        .target = target,
        .optimize = optimize,
    });
    const json_wire_module = builder.createModule(.{
        .root_source_file = builder.path("src/json_wire.zig"),
        .target = target,
        .optimize = optimize,
    });
    const protocol_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol.zig"),
        .target = target,
        .optimize = optimize,
    });
    protocol_module.addImport("ssh-json-wire", json_wire_module);
    const security_module = builder.createModule(.{
        .root_source_file = builder.path("src/private_ipc_security.zig"),
        .target = target,
        .optimize = optimize,
    });
    security_module.addIncludePath(builder.path("src"));
    security_module.addCSourceFile(.{
        .file = builder.path("src/private_ipc_security.c"),
        .flags = authored_c_flags,
    });
    const server_module = builder.createModule(.{
        .root_source_file = builder.path("src/runtime_server.zig"),
        .target = target,
        .optimize = optimize,
    });
    server_module.addImport("ssh-environment-protocol", protocol_module);
    server_module.addImport("ssh-runtime-private-ipc", security_module);
    server_module.addOptions("build-options", build_options);
    const project_root_service_module = builder.createModule(.{
        .root_source_file = builder.path("src/project_root_service.zig"),
        .target = target,
        .optimize = optimize,
    });
    project_root_service_module.addImport("ssh-environment-protocol", protocol_module);
    project_root_service_module.addImport("ssh-json-wire", json_wire_module);
    server_module.addImport("ssh-project-root-service", project_root_service_module);
    const terminal_host_relay_module = builder.createModule(.{
        .root_source_file = builder.path("src/terminal_host_relay.zig"),
        .target = target,
        .optimize = optimize,
    });
    terminal_host_relay_module.addImport("ssh-runtime-private-ipc", security_module);
    server_module.addImport("ssh-terminal-host-relay", terminal_host_relay_module);
    const attachment_module = builder.createModule(.{
        .root_source_file = builder.path("src/attachment.zig"),
        .target = target,
        .optimize = optimize,
    });
    attachment_module.addImport("ssh-runtime-private-ipc", security_module);

    const executable_module = builder.createModule(.{
        .root_source_file = builder.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
        .strip = strip,
    });
    executable_module.addImport("ssh-runtime-arguments", arguments_module);
    executable_module.addImport("ssh-runtime-attachment", attachment_module);
    executable_module.addImport("ssh-project-root-service", project_root_service_module);
    executable_module.addImport("ssh-runtime-server", server_module);
    executable_module.addImport("ssh-terminal-host-relay", terminal_host_relay_module);

    const executable = builder.addExecutable(.{
        .name = "orchbee-ssh-environment-runtime",
        .root_module = executable_module,
    });
    executable.root_module.link_libc = true;
    builder.installArtifact(executable);

    const test_module = builder.createModule(.{
        .root_source_file = builder.path("src/__tests__/runtime_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    test_module.addImport("ssh-runtime-arguments", arguments_module);
    test_module.addImport("ssh-environment-protocol", protocol_module);
    test_module.addImport("ssh-project-root-service", project_root_service_module);
    test_module.addImport("ssh-terminal-host-relay", terminal_host_relay_module);
    const tests = builder.addTest(.{ .root_module = test_module });
    tests.root_module.link_libc = true;
    const run_tests = builder.addRunArtifact(tests);
    const attachment_tests = builder.addTest(.{ .root_module = attachment_module });
    attachment_tests.root_module.link_libc = true;
    const run_attachment_tests = builder.addRunArtifact(attachment_tests);

    const test_step = builder.step("test", "Build the Runtime and run unit tests");
    test_step.dependOn(&executable.step);
    test_step.dependOn(&run_tests.step);
    test_step.dependOn(&run_attachment_tests.step);
    const test_compile_step = builder.step(
        "test-compile",
        "Compile the Runtime and unit tests without running them",
    );
    test_compile_step.dependOn(&executable.step);
    test_compile_step.dependOn(&tests.step);
    test_compile_step.dependOn(&attachment_tests.step);
}

fn supportedPlatform(target: std.Target) []const u8 {
    if (target.os.tag == .macos and target.cpu.arch == .aarch64) return "darwin-arm64";
    if (target.os.tag == .linux and target.cpu.arch == .x86_64 and target.abi == .gnu) {
        return "linux-x64";
    }
    std.debug.panic(
        "unsupported SSH Environment Runtime target {s}-{s}-{s}; Slice 2.2 supports aarch64-macos and x86_64-linux-gnu only",
        .{ @tagName(target.cpu.arch), @tagName(target.os.tag), @tagName(target.abi) },
    );
}
