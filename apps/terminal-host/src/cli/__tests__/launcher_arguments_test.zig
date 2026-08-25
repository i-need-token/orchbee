const builtin = @import("builtin");
const std = @import("std");
const cli_boundary = @import("terminal-host-cli");
const model_boundary = @import("terminal-host-model");

const launcher_arguments = cli_boundary.launcher_arguments;
const subject = struct {
    pub const Command = launcher_arguments.Command;
    pub const ServeOptions = launcher_arguments.ServeOptions;

    pub fn parse(arguments: []const []const u8) !Command {
        return launcher_arguments.parse(std.testing.allocator, arguments);
    }
};
const terminal_configuration = model_boundary.configuration;

fn runtimeDirectory() []const u8 {
    return if (builtin.os.tag == .windows)
        "C:\\orchbee-runtime"
    else
        "/tmp/orchbee-runtime";
}

fn expectServe(command: subject.Command) !subject.ServeOptions {
    return switch (command) {
        .serve => |options| options,
        .version => error.ExpectedServeCommand,
    };
}

test "launcher recognizes only the exact version command" {
    switch (try subject.parse(&.{"--version"})) {
        .version => {},
        .serve => return error.ExpectedVersionCommand,
    }
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{}));
    try std.testing.expectError(
        error.InvalidArguments,
        subject.parse(&.{ "--version", "extra" }),
    );
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{"version"}));
}

test "launcher rejects removed endpoint helper commands" {
    const runtime_directory = runtimeDirectory();
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "prepare-runtime",
        "--runtime-directory",
        runtime_directory,
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "connect",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "posix-prepare-runtime",
        "--runtime-directory",
        runtime_directory,
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "posix-connect",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    }));
}

test "launcher parses the private serve command into typed options" {
    const runtime_directory = runtimeDirectory();
    const parsed = try expectServe(try subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--scrollback-bytes",
        "25000000",
    }));
    try std.testing.expectEqualStrings(runtime_directory, parsed.runtime_directory);
    try std.testing.expectEqualStrings(
        "0123456789abcdef0123456789abcdef",
        parsed.endpoint_id,
    );
    try std.testing.expectEqual(@as(usize, 25_000_000), parsed.terminal_defaults.scrollback_bytes);

    const defaults = try expectServe(try subject.parse(&.{
        "serve",
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--runtime-directory",
        runtime_directory,
    }));
    try std.testing.expectEqual(
        terminal_configuration.default_scrollback_bytes,
        defaults.terminal_defaults.scrollback_bytes,
    );

    const zero = try expectServe(try subject.parse(&.{
        "serve",
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--runtime-directory",
        runtime_directory,
        "--scrollback-bytes",
        "0",
    }));
    try std.testing.expectEqual(@as(usize, 0), zero.terminal_defaults.scrollback_bytes);

    const maximum = try expectServe(try subject.parse(&.{
        "serve",
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--runtime-directory",
        runtime_directory,
        "--scrollback-bytes",
        "1000000000",
    }));
    try std.testing.expectEqual(
        terminal_configuration.testing.scrollback_limit,
        maximum.terminal_defaults.scrollback_bytes,
    );
}

test "launcher rejects malformed serve command shapes" {
    const runtime_directory = runtimeDirectory();
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{"serve"}));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "serve",
        "--runtime-directory",
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--unknown",
        "value",
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidArguments, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--scrollback-bytes",
        "1",
        "--scrollback-bytes",
        "2",
    }));
}

test "launcher validates serve option values at the CLI boundary" {
    const runtime_directory = runtimeDirectory();
    try std.testing.expectError(error.RuntimeDirectoryNotAbsolute, subject.parse(&.{
        "serve",
        "--runtime-directory",
        "relative/runtime",
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
    }));
    try std.testing.expectError(error.InvalidEndpointId, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "ABCDEF0123456789abcdef0123456789",
    }));
    try std.testing.expectError(error.InvalidScrollbackBytes, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--scrollback-bytes",
        "invalid",
    }));
    try std.testing.expectError(error.InvalidScrollbackBytes, subject.parse(&.{
        "serve",
        "--runtime-directory",
        runtime_directory,
        "--endpoint-id",
        "0123456789abcdef0123456789abcdef",
        "--scrollback-bytes",
        "1000000001",
    }));
}
