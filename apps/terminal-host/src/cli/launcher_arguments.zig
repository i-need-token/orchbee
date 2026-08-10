const std = @import("std");

const argument_options = @import("argument_options.zig");
const local_ipc = @import("terminal-host-transport");
const terminal_configuration = @import("terminal-host-model").configuration;

const endpoint_id_option = "--endpoint-id";
const runtime_directory_option = "--runtime-directory";
const scrollback_bytes_option = "--scrollback-bytes";
const posix_prepare_command = "posix-prepare-runtime";
const posix_connect_command = "posix-connect";
const serve_option_names = [_][]const u8{
    endpoint_id_option,
    runtime_directory_option,
    scrollback_bytes_option,
};

pub const Command = union(enum) {
    posix_connect: EndpointOptions,
    posix_prepare_runtime: RuntimeDirectoryOptions,
    version,
    serve: ServeOptions,
};

pub const EndpointOptions = struct {
    endpoint_id: []const u8,
    runtime_directory: []const u8,
};

pub const RuntimeDirectoryOptions = struct {
    runtime_directory: []const u8,
};

pub const ServeOptions = struct {
    endpoint_id: []const u8,
    runtime_directory: []const u8,
    terminal_defaults: terminal_configuration.Configuration,
};

pub fn parse(arguments: []const []const u8) !Command {
    if (arguments.len == 1 and std.mem.eql(u8, arguments[0], "--version")) {
        return .version;
    }
    if (arguments.len == 0) return error.InvalidArguments;
    if (std.mem.eql(u8, arguments[0], posix_prepare_command)) {
        const options = try argument_options.Options.parse(arguments[1..]);
        try options.ensureOnly(&.{runtime_directory_option});
        const runtime_directory = try options.require(runtime_directory_option);
        if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
        return .{ .posix_prepare_runtime = .{ .runtime_directory = runtime_directory } };
    }
    if (std.mem.eql(u8, arguments[0], posix_connect_command)) {
        const endpoint = try parseEndpointOptions(arguments[1..]);
        return .{ .posix_connect = endpoint };
    }
    if (!std.mem.eql(u8, arguments[0], "serve")) return error.InvalidArguments;

    const options = try argument_options.Options.parse(arguments[1..]);
    try options.ensureOnly(&serve_option_names);

    const runtime_directory = try options.require(runtime_directory_option);
    const endpoint_id = try options.require(endpoint_id_option);
    var terminal_defaults: terminal_configuration.Configuration = .{};
    if (options.get(scrollback_bytes_option)) |value| {
        terminal_defaults.scrollback_bytes = argument_options.parseInteger(usize, value) catch
            return error.InvalidScrollbackBytes;
    }

    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    if (!local_ipc.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
    if (!terminal_configuration.valid(terminal_defaults)) return error.InvalidScrollbackBytes;
    return .{ .serve = .{
        .endpoint_id = endpoint_id,
        .runtime_directory = runtime_directory,
        .terminal_defaults = terminal_defaults,
    } };
}

fn parseEndpointOptions(arguments: []const []const u8) !EndpointOptions {
    const options = try argument_options.Options.parse(arguments);
    try options.ensureOnly(&.{ endpoint_id_option, runtime_directory_option });
    const runtime_directory = try options.require(runtime_directory_option);
    const endpoint_id = try options.require(endpoint_id_option);
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    if (!local_ipc.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
    return .{
        .endpoint_id = endpoint_id,
        .runtime_directory = runtime_directory,
    };
}
