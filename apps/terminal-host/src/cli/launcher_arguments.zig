const std = @import("std");

const argument_options = @import("argument_options.zig");
const local_ipc = @import("terminal-host-transport");
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;

const endpoint_directory_option = "--endpoint-directory";
const endpoint_id_option = "--endpoint-id";
const host_epoch_option = "--host-epoch";
const runtime_directory_option = "--runtime-directory";
const scrollback_bytes_option = "--scrollback-bytes";
const serve_option_names = [_][]const u8{
    endpoint_directory_option,
    endpoint_id_option,
    host_epoch_option,
    runtime_directory_option,
    scrollback_bytes_option,
};
const keep_option_names = [_][]const u8{
    endpoint_id_option,
    runtime_directory_option,
};

pub const Command = union(enum) {
    version,
    serve: ServeOptions,
    /// The Keeper, which holds terminals' handles for the Hosts of one Session.
    keep: KeepOptions,
};

pub const KeepOptions = struct {
    endpoint_id: []const u8,
    runtime_directory: []const u8,
};

pub const ServeOptions = struct {
    /// Where a macOS or Linux Host binds its endpoint; without it the endpoint goes in the
    /// runtime directory. A Windows Host names a pipe instead and ignores it.
    endpoint_directory: ?[]const u8,
    endpoint_id: []const u8,
    /// A recovery Host's epoch, the lost Host's, so its terminals keep their identities.
    host_epoch: ?model.identifiers.HostEpoch,
    runtime_directory: []const u8,
    terminal_defaults: terminal_configuration.Configuration,
};

pub fn parse(allocator: std.mem.Allocator, arguments: []const []const u8) !Command {
    if (arguments.len == 1 and std.mem.eql(u8, arguments[0], "--version")) {
        return .version;
    }
    if (arguments.len == 0) return error.InvalidArguments;
    var options = try argument_options.Options.parse(allocator, arguments[1..]);
    defer options.deinit();

    if (std.mem.eql(u8, arguments[0], "keep")) return .{ .keep = try parseKeepOptions(options) };
    if (!std.mem.eql(u8, arguments[0], "serve")) return error.InvalidArguments;

    return .{ .serve = try parseServeOptions(options) };
}

fn parseKeepOptions(options: argument_options.Options) !KeepOptions {
    try options.ensureOnly(&keep_option_names);
    const runtime_directory = try options.requireOne(runtime_directory_option);
    const endpoint_id = try options.requireOne(endpoint_id_option);
    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    if (!local_ipc.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
    return .{ .endpoint_id = endpoint_id, .runtime_directory = runtime_directory };
}

fn parseServeOptions(options: argument_options.Options) !ServeOptions {
    try options.ensureOnly(&serve_option_names);

    const runtime_directory = try options.requireOne(runtime_directory_option);
    const endpoint_id = try options.requireOne(endpoint_id_option);
    const endpoint_directory = try options.optionalOne(endpoint_directory_option);
    var terminal_defaults: terminal_configuration.Configuration = .{};
    if (try options.optionalOne(scrollback_bytes_option)) |value| {
        terminal_defaults.scrollback_bytes = argument_options.parseInteger(usize, value) catch
            return error.InvalidScrollbackBytes;
    }

    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    if (endpoint_directory) |directory| {
        if (!std.fs.path.isAbsolute(directory)) return error.EndpointDirectoryNotAbsolute;
    }
    if (!local_ipc.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
    if (!terminal_configuration.valid(terminal_defaults)) return error.InvalidScrollbackBytes;
    const host_epoch = if (try options.optionalOne(host_epoch_option)) |value|
        model.identifiers.parseLowerHex(model.identifiers.HostEpoch, value) catch return error.InvalidHostEpoch
    else
        null;
    return .{
        .endpoint_directory = endpoint_directory,
        .endpoint_id = endpoint_id,
        .host_epoch = host_epoch,
        .runtime_directory = runtime_directory,
        .terminal_defaults = terminal_defaults,
    };
}
