const builtin = @import("builtin");
const std = @import("std");

const required_zig_version = "0.15.2";

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

    const host_module = builder.createModule(.{
        .root_source_file = builder.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    const protocol_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol.zig"),
        .target = target,
        .optimize = optimize,
    });
    host_module.addImport("terminal-protocol", protocol_module);
    const benchmark_module = builder.createModule(.{
        .root_source_file = builder.path("src/benchmark.zig"),
        .target = target,
        .optimize = optimize,
    });
    benchmark_module.addImport("terminal-protocol", protocol_module);

    const ghostty = builder.dependency("ghostty", .{
        .target = target,
        .optimize = optimize,
        .@"emit-lib-vt" = true,
        .@"emit-xcframework" = false,
        .@"lib-version-string" = "1.3.2",
    });
    host_module.addImport("ghostty-vt", ghostty.module("ghostty-vt"));
    benchmark_module.addImport("ghostty-vt", ghostty.module("ghostty-vt"));
    switch (target.result.os.tag) {
        .freebsd, .linux, .macos => {
            host_module.addCSourceFile(.{
                .file = builder.path("src/process_session.c"),
                .flags = &.{"-std=c17"},
            });
            benchmark_module.addCSourceFile(.{
                .file = builder.path("src/process_session.c"),
                .flags = &.{"-std=c17"},
            });
            const pty_c = builder.addTranslateC(.{
                .root_source_file = ghostty.path("src/pty.c"),
                .target = target,
                .optimize = optimize,
            });
            if (target.result.os.tag == .macos) {
                const libc = std.zig.LibCInstallation.findNative(.{
                    .allocator = builder.allocator,
                    .target = &target.result,
                    .verbose = false,
                }) catch @panic("could not locate native macOS libc headers");
                pty_c.addSystemIncludePath(.{ .cwd_relative = libc.sys_include_dir.? });
            }
            const pty_module = pty_c.createModule();
            host_module.addImport("pty-c", pty_module);
            benchmark_module.addImport("pty-c", pty_module);
        },
        .windows => {
            host_module.addIncludePath(builder.path("src"));
            host_module.addCSourceFile(.{
                .file = builder.path("src/local_ipc_windows.c"),
                .flags = &.{"-std=c17"},
            });
            host_module.addCSourceFile(.{
                .file = builder.path("src/process_job_windows.c"),
                .flags = &.{"-std=c17"},
            });
            benchmark_module.addIncludePath(builder.path("src"));
            benchmark_module.addCSourceFile(.{
                .file = builder.path("src/local_ipc_windows.c"),
                .flags = &.{"-std=c17"},
            });
            benchmark_module.addCSourceFile(.{
                .file = builder.path("src/process_job_windows.c"),
                .flags = &.{"-std=c17"},
            });
            host_module.linkSystemLibrary("advapi32", .{});
            benchmark_module.linkSystemLibrary("advapi32", .{});
        },
        else => {},
    }

    const executable = builder.addExecutable(.{
        .name = "terminal-host",
        .root_module = host_module,
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

    const tests = builder.addTest(.{ .root_module = host_module });
    tests.linkLibC();
    const run_tests = builder.addRunArtifact(tests);
    const benchmark_options_tests = builder.addTest(.{
        .root_module = builder.createModule(.{
            .root_source_file = builder.path("src/benchmark_options.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    const run_benchmark_options_tests = builder.addRunArtifact(benchmark_options_tests);
    const test_step = builder.step("test", "Run terminal Host foundation tests");
    test_step.dependOn(&run_tests.step);
    test_step.dependOn(&run_benchmark_options_tests.step);

    const test_compile_step = builder.step(
        "test-compile",
        "Compile terminal Host foundation tests without running them",
    );
    test_compile_step.dependOn(&tests.step);
    test_compile_step.dependOn(&benchmark_executable.step);
    test_compile_step.dependOn(&benchmark_options_tests.step);

    const conformance_options = builder.addOptions();
    conformance_options.addOption([]const u8, "frame_fixture", readFixture(
        builder,
        "frame-fixture",
        "Canonical terminal frame fixture path",
        8 * 1024 * 1024,
    ));
    conformance_options.addOption([]const u8, "envelope_fixture", readFixture(
        builder,
        "envelope-fixture",
        "Canonical transport envelope fixture path",
        8 * 1024 * 1024,
    ));
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
    const conformance_module = builder.createModule(.{
        .root_source_file = builder.path("src/protocol_conformance_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    conformance_module.addImport("terminal-protocol", protocol_module);
    conformance_module.addOptions("conformance-options", conformance_options);
    const conformance_tests = builder.addTest(.{ .root_module = conformance_module });
    const run_conformance_tests = builder.addRunArtifact(conformance_tests);
    const conformance_step = builder.step(
        "test-protocol",
        "Run terminal protocol cross-language conformance tests",
    );
    conformance_step.dependOn(&run_conformance_tests.step);
}
