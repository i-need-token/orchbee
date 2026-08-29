const std = @import("std");

pub const AttachOptions = struct {
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
};

pub const TerminalHostOptions = struct {
    build_digest: []const u8,
    endpoint_id: []const u8,
    executable_path: []const u8,
    runtime_directory: []const u8,
    version: []const u8,
};

pub const TerminalHostRelayWorkerOptions = struct {
    endpoint_id: []const u8,
    executable_path: []const u8,
    runtime_directory: []const u8,
};

pub const ServeOptions = struct {
    release_cohort_digest: []const u8,
    runtime_build_digest: []const u8,
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
    terminal_host: TerminalHostOptions,
};

pub const Command = union(enum) {
    version,
    project_root_worker,
    terminal_host_relay_worker: TerminalHostRelayWorkerOptions,
    serve: ServeOptions,
    attach: AttachOptions,
};

pub fn parse(arguments: []const []const u8) !Command {
    if (arguments.len == 1 and std.mem.eql(u8, arguments[0], "--version")) {
        return .version;
    }
    if (arguments.len == 1 and std.mem.eql(u8, arguments[0], "project-root-worker")) {
        return .project_root_worker;
    }
    if (arguments.len == 0) return error.InvalidArguments;

    const command_name = arguments[0];
    if (std.mem.eql(u8, command_name, "attach")) return parseAttach(arguments);
    if (std.mem.eql(u8, command_name, "serve")) return parseServe(arguments);
    if (std.mem.eql(u8, command_name, "terminal-host-relay-worker")) {
        return parseTerminalHostRelayWorker(arguments);
    }
    return error.InvalidArguments;
}

fn parseAttach(arguments: []const []const u8) !Command {
    if (arguments.len != 5) return error.InvalidArguments;
    var runtime_directory: ?[]const u8 = null;
    var runtime_instance_id: ?[]const u8 = null;
    var index: usize = 1;
    while (index < arguments.len) : (index += 2) {
        const name = arguments[index];
        const value = arguments[index + 1];
        if (std.mem.eql(u8, name, "--runtime-directory")) {
            if (runtime_directory != null) return error.InvalidArguments;
            runtime_directory = value;
        } else if (std.mem.eql(u8, name, "--instance-id")) {
            if (runtime_instance_id != null) return error.InvalidArguments;
            runtime_instance_id = value;
        } else return error.InvalidArguments;
    }
    const options: AttachOptions = .{
        .runtime_directory = runtime_directory orelse return error.InvalidArguments,
        .runtime_instance_id = runtime_instance_id orelse return error.InvalidArguments,
    };
    try validateAttach(options);
    return .{ .attach = options };
}

fn parseServe(arguments: []const []const u8) !Command {
    if (arguments.len != 19) return error.InvalidArguments;
    var release_cohort_digest: ?[]const u8 = null;
    var runtime_build_digest: ?[]const u8 = null;
    var runtime_directory: ?[]const u8 = null;
    var runtime_instance_id: ?[]const u8 = null;
    var terminal_host_build_digest: ?[]const u8 = null;
    var terminal_host_endpoint_id: ?[]const u8 = null;
    var terminal_host_executable: ?[]const u8 = null;
    var terminal_host_runtime_directory: ?[]const u8 = null;
    var terminal_host_version: ?[]const u8 = null;
    var index: usize = 1;
    while (index < arguments.len) : (index += 2) {
        const name = arguments[index];
        const value = arguments[index + 1];
        const target: *?[]const u8 = if (std.mem.eql(u8, name, "--build-digest"))
            &runtime_build_digest
        else if (std.mem.eql(u8, name, "--release-cohort-digest"))
            &release_cohort_digest
        else if (std.mem.eql(u8, name, "--runtime-directory"))
            &runtime_directory
        else if (std.mem.eql(u8, name, "--instance-id"))
            &runtime_instance_id
        else if (std.mem.eql(u8, name, "--terminal-host-build-digest"))
            &terminal_host_build_digest
        else if (std.mem.eql(u8, name, "--terminal-host-endpoint-id"))
            &terminal_host_endpoint_id
        else if (std.mem.eql(u8, name, "--terminal-host-executable"))
            &terminal_host_executable
        else if (std.mem.eql(u8, name, "--terminal-host-runtime-directory"))
            &terminal_host_runtime_directory
        else if (std.mem.eql(u8, name, "--terminal-host-version"))
            &terminal_host_version
        else
            return error.InvalidArguments;
        if (target.* != null) return error.InvalidArguments;
        target.* = value;
    }
    const options: ServeOptions = .{
        .release_cohort_digest = release_cohort_digest orelse return error.InvalidArguments,
        .runtime_build_digest = runtime_build_digest orelse return error.InvalidArguments,
        .runtime_directory = runtime_directory orelse return error.InvalidArguments,
        .runtime_instance_id = runtime_instance_id orelse return error.InvalidArguments,
        .terminal_host = .{
            .build_digest = terminal_host_build_digest orelse return error.InvalidArguments,
            .endpoint_id = terminal_host_endpoint_id orelse return error.InvalidArguments,
            .executable_path = terminal_host_executable orelse return error.InvalidArguments,
            .runtime_directory = terminal_host_runtime_directory orelse return error.InvalidArguments,
            .version = terminal_host_version orelse return error.InvalidArguments,
        },
    };
    try validateAttach(.{
        .runtime_directory = options.runtime_directory,
        .runtime_instance_id = options.runtime_instance_id,
    });
    if (!validHexIdentity(options.runtime_build_digest, 64) or
        !validHexIdentity(options.release_cohort_digest, 64) or
        !validHexIdentity(options.terminal_host.build_digest, 64))
    {
        return error.InvalidBuildDigest;
    }
    try validateTerminalHost(options.terminal_host);
    return .{ .serve = options };
}

fn parseTerminalHostRelayWorker(arguments: []const []const u8) !Command {
    if (arguments.len != 7) return error.InvalidArguments;
    var endpoint_id: ?[]const u8 = null;
    var executable_path: ?[]const u8 = null;
    var runtime_directory: ?[]const u8 = null;
    var index: usize = 1;
    while (index < arguments.len) : (index += 2) {
        const name = arguments[index];
        const value = arguments[index + 1];
        const target: *?[]const u8 = if (std.mem.eql(u8, name, "--terminal-host-endpoint-id"))
            &endpoint_id
        else if (std.mem.eql(u8, name, "--terminal-host-executable"))
            &executable_path
        else if (std.mem.eql(u8, name, "--terminal-host-runtime-directory"))
            &runtime_directory
        else
            return error.InvalidArguments;
        if (target.* != null) return error.InvalidArguments;
        target.* = value;
    }
    const options: TerminalHostRelayWorkerOptions = .{
        .endpoint_id = endpoint_id orelse return error.InvalidArguments,
        .executable_path = executable_path orelse return error.InvalidArguments,
        .runtime_directory = runtime_directory orelse return error.InvalidArguments,
    };
    if (!validIdentity(options.endpoint_id)) return error.InvalidEndpointId;
    if (!std.fs.path.isAbsolute(options.executable_path)) return error.ExecutablePathNotAbsolute;
    if (!std.fs.path.isAbsolute(options.runtime_directory)) {
        return error.RuntimeDirectoryNotAbsolute;
    }
    return .{ .terminal_host_relay_worker = options };
}

fn validateAttach(options: AttachOptions) !void {
    if (!std.fs.path.isAbsolute(options.runtime_directory)) {
        return error.RuntimeDirectoryNotAbsolute;
    }
    if (!validIdentity(options.runtime_instance_id)) return error.InvalidRuntimeInstanceId;
}

fn validateTerminalHost(options: TerminalHostOptions) !void {
    if (!validIdentity(options.endpoint_id)) return error.InvalidEndpointId;
    if (!std.fs.path.isAbsolute(options.executable_path)) return error.ExecutablePathNotAbsolute;
    if (!std.fs.path.isAbsolute(options.runtime_directory)) {
        return error.RuntimeDirectoryNotAbsolute;
    }
    if (options.version.len == 0 or options.version.len > 64) return error.InvalidVersion;
    _ = std.SemanticVersion.parse(options.version) catch return error.InvalidVersion;
}

pub fn validIdentity(value: []const u8) bool {
    return validHexIdentity(value, 32);
}

fn validHexIdentity(value: []const u8, expected_length: usize) bool {
    if (value.len != expected_length) return false;
    for (value) |byte| switch (byte) {
        '0'...'9', 'a'...'f' => {},
        else => return false,
    };
    return true;
}
