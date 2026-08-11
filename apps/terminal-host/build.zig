const builtin = @import("builtin");
const std = @import("std");
// Imported into the build script (not as a build module) so the envelope
// fixture cap is computed from the authoritative protocol record limit.
const contract = @import("src/protocol/contract.zig");

const required_zig_version = "0.15.2";
const default_host_version = "0.1.0";
// Cross-language contract: search for `terminal-host-version-wire-limit`
// before changing this value, then update every occurrence together.
const max_host_version_bytes = 64;
// The envelope fixture may legally span the full transport-record bound: a
// 64-byte OBTP header declaring an 8 MiB terminal-frame payload. Tied to the
// authoritative protocol record limit so a framing change cannot silently
// make the boundary fixture unreachable; the conformance test asserts the tie.
const envelope_fixture_max_bytes = contract.Limits.transport_record_bytes;

fn readFixture(
    builder: *std.Build,
    option_name: []const u8,
    description: []const u8,
    maximum_bytes: usize,
) []const u8 {
    const fixture_path = builder.option([]const u8, option_name, description) orelse return "";
    return std.fs.cwd().readFileAlloc(builder.allocator, fixture_path, maximum_bytes) catch |err|
        std.debug.panic("could not read fixture {s}: {s}", .{ fixture_path, @errorName(err) });
}

pub fn build(builder: *std.Build) void {
    if (!std.mem.eql(u8, builtin.zig_version_string, required_zig_version)) {
        std.debug.panic(
            "terminal-host requires Zig {s}, received {s}",
            .{ required_zig_version, builtin.zig_version_string },
        );
    }

    const target = builder.standardTargetOptions(.{});
    const optimize = builder.standardOptimizeOption(.{});
    const host_version = builder.option(
        []const u8,
        "host-version",
        "Semantic version embedded in orchbee-terminal-host",
    ) orelse default_host_version;
    _ = std.SemanticVersion.parse(host_version) catch
        std.debug.panic("invalid terminal Host semantic version: {s}", .{host_version});
    if (host_version.len > max_host_version_bytes) {
        std.debug.panic(
            "terminal Host semantic version exceeds {d} bytes",
            .{max_host_version_bytes},
        );
    }
    const build_options = builder.addOptions();
    build_options.addOption([]const u8, "host_version", host_version);

    const terminal_input_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol/semantic_input.zig"),
        .target = target,
        .optimize = optimize,
    });
    const protocol_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const model_module = builder.createModule(.{
        .root_source_file = builder.path("src/model/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const platform_module = builder.createModule(.{
        .root_source_file = builder.path("src/platform/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const engine_module = builder.createModule(.{
        .root_source_file = builder.path("src/engine/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const core_module = builder.createModule(.{
        .root_source_file = builder.path("src/core/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const control_module = builder.createModule(.{
        .root_source_file = builder.path("src/control/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const service_module = builder.createModule(.{
        .root_source_file = builder.path("src/service/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const transport_module = builder.createModule(.{
        .root_source_file = builder.path("src/transport/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const cli_module = builder.createModule(.{
        .root_source_file = builder.path("src/cli/root.zig"),
        .target = target,
        .optimize = optimize,
    });
    const terminal_host_module = builder.createModule(.{
        .root_source_file = builder.path("src/terminal_host.zig"),
        .target = target,
        .optimize = optimize,
    });
    const executable_module = builder.createModule(.{
        .root_source_file = builder.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });

    const ghostty = builder.dependency("ghostty", .{
        .target = target,
        .optimize = optimize,
        .@"emit-lib-vt" = true,
        .@"emit-xcframework" = false,
        .@"lib-version-string" = "1.3.2",
    });

    protocol_module.addImport("terminal-input", terminal_input_module);
    model_module.addImport("terminal-protocol", protocol_module);
    engine_module.addImport("ghostty-vt", ghostty.module("ghostty-vt"));
    engine_module.addImport("terminal-input", terminal_input_module);
    engine_module.addImport("terminal-protocol", protocol_module);
    engine_module.addImport("terminal-host-model", model_module);
    engine_module.addImport("terminal-host-platform", platform_module);
    core_module.addImport("terminal-host-engine", engine_module);
    core_module.addImport("terminal-host-model", model_module);
    control_module.addImport("terminal-protocol", protocol_module);
    control_module.addImport("terminal-host-core", core_module);
    control_module.addImport("terminal-host-engine", engine_module);
    control_module.addImport("terminal-host-model", model_module);
    service_module.addImport("terminal-input", terminal_input_module);
    service_module.addImport("terminal-protocol", protocol_module);
    service_module.addImport("terminal-host-control", control_module);
    service_module.addImport("terminal-host-core", core_module);
    service_module.addImport("terminal-host-engine", engine_module);
    service_module.addImport("terminal-host-model", model_module);
    service_module.addImport("terminal-host-platform", platform_module);
    service_module.addOptions("build-options", build_options);
    transport_module.addImport("terminal-protocol", protocol_module);
    transport_module.addImport("terminal-host-service", service_module);
    cli_module.addImport("terminal-host-model", model_module);
    cli_module.addImport("terminal-host-transport", transport_module);
    terminal_host_module.addImport("terminal-host-cli", cli_module);
    terminal_host_module.addImport("terminal-host-service", service_module);
    terminal_host_module.addImport("terminal-host-transport", transport_module);
    executable_module.addImport("terminal-host", terminal_host_module);
    const benchmark_module = builder.createModule(.{
        .root_source_file = builder.path("src/benchmark/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    benchmark_module.addImport("terminal-host-cli", cli_module);
    benchmark_module.addImport("terminal-host-engine", engine_module);
    benchmark_module.addImport("terminal-host-model", model_module);

    const cli_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/cli/__tests__/cli_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    cli_test_module.addImport("terminal-host-cli", cli_module);
    cli_test_module.addImport("terminal-host-model", model_module);
    const benchmark_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/benchmark/__tests__/benchmark_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    benchmark_test_module.addImport("terminal-host-benchmark", benchmark_module);
    const protocol_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol/__tests__/protocol_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    protocol_test_module.addImport("terminal-input", terminal_input_module);
    protocol_test_module.addImport("terminal-protocol", protocol_module);
    const model_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/model/__tests__/model_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    model_test_module.addImport("terminal-host-model", model_module);
    const platform_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/platform/__tests__/platform_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    platform_test_module.addImport("terminal-host-platform", platform_module);
    const engine_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/engine/__tests__/engine_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    engine_test_module.addImport("ghostty-vt", ghostty.module("ghostty-vt"));
    engine_test_module.addImport("terminal-protocol", protocol_module);
    engine_test_module.addImport("terminal-host-engine", engine_module);
    engine_test_module.addImport("terminal-host-model", model_module);
    engine_test_module.addImport("terminal-host-platform", platform_module);
    const core_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/core/__tests__/core_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    core_test_module.addImport("terminal-host-core", core_module);
    core_test_module.addImport("terminal-host-model", model_module);
    const control_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/control/__tests__/control_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    control_test_module.addImport("terminal-protocol", protocol_module);
    control_test_module.addImport("terminal-host-control", control_module);
    control_test_module.addImport("terminal-host-core", core_module);
    control_test_module.addImport("terminal-host-engine", engine_module);
    const service_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/service/__tests__/service_boundary_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    service_test_module.addImport("terminal-protocol", protocol_module);
    service_test_module.addImport("terminal-host-control", control_module);
    service_test_module.addImport("terminal-host-core", core_module);
    service_test_module.addImport("terminal-host-engine", engine_module);
    service_test_module.addImport("terminal-host-model", model_module);
    service_test_module.addImport("terminal-host-platform", platform_module);
    service_test_module.addImport("terminal-host-service", service_module);
    const transport_test_module = builder.createModule(.{
        .root_source_file = builder.path("src/transport/__tests__/transport_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    transport_test_module.addImport("terminal-protocol", protocol_module);
    transport_test_module.addImport("terminal-host-core", core_module);
    transport_test_module.addImport("terminal-host-service", service_module);
    transport_test_module.addImport("terminal-host-transport", transport_module);

    switch (target.result.os.tag) {
        .freebsd, .linux, .macos => {
            transport_module.addIncludePath(builder.path("src/transport"));
            transport_module.addCSourceFile(.{
                .file = builder.path("src/transport/posix_endpoint_security.c"),
                .flags = &.{"-std=c17"},
            });
            platform_module.addCSourceFile(.{
                .file = builder.path("src/platform/process_session.c"),
                .flags = &.{"-std=c17"},
            });
            const pty_c = builder.addTranslateC(.{
                .root_source_file = ghostty.path("src/pty.c"),
                .target = target,
                .optimize = optimize,
            });
            if (target.result.os.tag == .macos) {
                if (builtin.target.os.tag.isDarwin()) {
                    const libc = std.zig.LibCInstallation.findNative(.{
                        .allocator = builder.allocator,
                        .target = &target.result,
                        .verbose = false,
                    }) catch @panic("could not locate native macOS libc headers");
                    pty_c.addSystemIncludePath(.{ .cwd_relative = libc.sys_include_dir.? });
                } else {
                    pty_c.addAfterIncludePath(
                        builder.path("src/platform/darwin-cross-headers"),
                    );
                }
            }
            if (target.result.os.tag == .macos) {
                transport_test_module.addIncludePath(builder.path("src/transport/__tests__"));
                transport_test_module.addCSourceFile(.{
                    .file = builder.path("src/transport/__tests__/posix_endpoint_security_test.c"),
                    .flags = &.{"-std=c17"},
                });
            }
            const pty_module = pty_c.createModule();
            platform_module.addImport("pty-c", pty_module);
        },
        .windows => {
            transport_module.addIncludePath(builder.path("src/transport"));
            transport_test_module.addIncludePath(builder.path("src/transport"));
            transport_module.addCSourceFile(.{
                .file = builder.path("src/transport/local_ipc_windows.c"),
                .flags = &.{"-std=c17"},
            });
            transport_module.linkSystemLibrary("advapi32", .{});
            platform_module.addCSourceFile(.{
                .file = builder.path("src/platform/process_job_windows.c"),
                .flags = &.{"-std=c17"},
            });
        },
        else => {},
    }

    const executable = builder.addExecutable(.{
        .name = "orchbee-terminal-host",
        .root_module = executable_module,
    });
    executable.linkLibC();
    builder.installArtifact(executable);

    const benchmark_executable = builder.addExecutable(.{
        .name = "terminal-host-benchmark",
        .root_module = benchmark_module,
    });
    benchmark_executable.linkLibC();
    const run_benchmark = builder.addRunArtifact(benchmark_executable);
    if (builder.args) |arguments| run_benchmark.addArgs(arguments);
    const benchmark_step = builder.step(
        "benchmark",
        "Measure no-demand and active-projection terminal throughput",
    );
    benchmark_step.dependOn(&run_benchmark.step);

    const test_step = builder.step("test", "Run terminal Host tests");
    const test_compile_step = builder.step(
        "test-compile",
        "Compile terminal Host executables and tests without running them",
    );
    test_compile_step.dependOn(&executable.step);
    test_compile_step.dependOn(&benchmark_executable.step);
    const focused_cli_tests = builder.addTest(.{
        .name = "terminal-host-cli-tests",
        .root_module = cli_test_module,
    });
    focused_cli_tests.linkLibC();
    const run_focused_cli_tests = builder.addRunArtifact(focused_cli_tests);
    const focused_cli_test_step = builder.step(
        "test-cli",
        "Run Terminal Host launcher argument tests",
    );
    focused_cli_test_step.dependOn(&run_focused_cli_tests.step);
    const focused_transport_tests = builder.addTest(.{
        .name = "terminal-host-transport-tests",
        .root_module = transport_test_module,
    });
    focused_transport_tests.linkLibC();
    const run_focused_transport_tests = builder.addRunArtifact(focused_transport_tests);
    const focused_transport_test_step = builder.step(
        "test-transport",
        "Run terminal Host local-transport tests",
    );
    focused_transport_test_step.dependOn(&run_focused_transport_tests.step);
    const focused_transport_compile_step = builder.step(
        "test-transport-compile",
        "Compile terminal Host local-transport tests without running them",
    );
    focused_transport_compile_step.dependOn(&focused_transport_tests.step);
    for ([_]*std.Build.Module{
        cli_test_module,
        benchmark_test_module,
        protocol_test_module,
        model_test_module,
        platform_test_module,
        engine_test_module,
        core_test_module,
        control_test_module,
        service_test_module,
        transport_test_module,
    }) |test_module| {
        const tests = builder.addTest(.{ .root_module = test_module });
        tests.linkLibC();
        const run_tests = builder.addRunArtifact(tests);
        test_step.dependOn(&run_tests.step);
        test_compile_step.dependOn(&tests.step);
    }

    const conformance_options = builder.addOptions();
    conformance_options.addOption([]const u8, "frame_fixture", readFixture(
        builder,
        "frame-fixture",
        "Canonical terminal frame fixture path",
        8 * 1024 * 1024,
    ));
    conformance_options.addOption([]const u8, "projection_fixture", readFixture(
        builder,
        "projection-fixture",
        "Canonical terminal projection-buffer fixture path",
        8 * 1024 * 1024,
    ));
    conformance_options.addOption([]const u8, "envelope_fixture", readFixture(
        builder,
        "envelope-fixture",
        "Canonical transport envelope fixture path",
        envelope_fixture_max_bytes,
    ));
    conformance_options.addOption(usize, "envelope_fixture_max_bytes", envelope_fixture_max_bytes);
    conformance_options.addOption([]const u8, "input_fixture", readFixture(
        builder,
        "input-fixture",
        "Canonical WriteInput fixture path",
        1024 * 1024 + 16,
    ));
    inline for (.{
        .{ "key_input_fixture", "key-input-fixture", "Canonical KeyInput fixture path" },
        .{ "text_input_fixture", "text-input-fixture", "Canonical TextInput fixture path" },
        .{ "mouse_input_fixture", "mouse-input-fixture", "Canonical MouseInput fixture path" },
        .{ "focus_input_fixture", "focus-input-fixture", "Canonical FocusInput fixture path" },
        .{ "paste_input_fixture", "paste-input-fixture", "Canonical PasteInput fixture path" },
    }) |fixture| {
        conformance_options.addOption([]const u8, fixture[0], readFixture(
            builder,
            fixture[1],
            fixture[2],
            32 + 1024 * 1024,
        ));
    }
    conformance_options.addOption([]const u8, "row_window_fixture", readFixture(
        builder,
        "row-window-fixture",
        "Canonical terminal row-window fixture path",
        2 * 1024 * 1024,
    ));
    conformance_options.addOption([]const u8, "terminal_failed_fixture", readFixture(
        builder,
        "terminal-failed-fixture",
        "Canonical TerminalFailed fixture path",
        64 * 1024,
    ));
    conformance_options.addOption([]const u8, "control_fixture", readFixture(
        builder,
        "control-fixture",
        "Canonical cross-language control fixture path",
        2 * 1024 * 1024,
    ));
    conformance_options.addOption([]const u8, "utf8_bom_fixture", readFixture(
        builder,
        "utf8-bom-fixture",
        "UTF-8 BOM conformance fixture path",
        64 * 1024,
    ));
    const conformance_module = builder.createModule(.{
        .root_source_file = builder.path("src/__tests__/protocol_conformance_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    conformance_module.addImport("terminal-protocol", protocol_module);
    conformance_module.addImport("terminal-host-control", control_module);
    conformance_module.addOptions("conformance-options", conformance_options);
    const conformance_tests = builder.addTest(.{ .root_module = conformance_module });
    conformance_tests.linkLibC();
    const run_conformance_tests = builder.addRunArtifact(conformance_tests);
    const conformance_step = builder.step(
        "test-protocol",
        "Run terminal protocol cross-language conformance tests",
    );
    conformance_step.dependOn(&run_conformance_tests.step);
    test_compile_step.dependOn(&conformance_tests.step);
}
