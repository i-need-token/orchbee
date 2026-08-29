const std = @import("std");

pub const AttachOptions = struct {
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
};

pub const ServeOptions = struct {
    runtime_build_digest: []const u8,
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
};

pub const Command = union(enum) {
    version,
    project_root_worker,
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
    const is_serve = std.mem.eql(u8, command_name, "serve");
    const is_attach = std.mem.eql(u8, command_name, "attach");
    const expected_argument_count: usize = if (is_serve) 7 else 5;
    if ((!is_serve and !is_attach) or arguments.len != expected_argument_count) {
        return error.InvalidArguments;
    }

    var runtime_build_digest: ?[]const u8 = null;
    var runtime_directory: ?[]const u8 = null;
    var runtime_instance_id: ?[]const u8 = null;
    var index: usize = 1;
    while (index < arguments.len) : (index += 2) {
        const name = arguments[index];
        const value = arguments[index + 1];
        if (std.mem.eql(u8, name, "--build-digest")) {
            if (runtime_build_digest != null) return error.InvalidArguments;
            runtime_build_digest = value;
        } else if (std.mem.eql(u8, name, "--runtime-directory")) {
            if (runtime_directory != null) return error.InvalidArguments;
            runtime_directory = value;
        } else if (std.mem.eql(u8, name, "--instance-id")) {
            if (runtime_instance_id != null) return error.InvalidArguments;
            runtime_instance_id = value;
        } else {
            return error.InvalidArguments;
        }
    }

    const options: AttachOptions = .{
        .runtime_directory = runtime_directory orelse return error.InvalidArguments,
        .runtime_instance_id = runtime_instance_id orelse return error.InvalidArguments,
    };
    if (!std.fs.path.isAbsolute(options.runtime_directory)) {
        return error.RuntimeDirectoryNotAbsolute;
    }
    if (!validIdentity(options.runtime_instance_id)) return error.InvalidRuntimeInstanceId;

    if (is_serve) {
        const build_digest = runtime_build_digest orelse return error.InvalidArguments;
        if (!validHexIdentity(build_digest, 64)) return error.InvalidBuildDigest;
        return .{ .serve = .{
            .runtime_build_digest = build_digest,
            .runtime_directory = options.runtime_directory,
            .runtime_instance_id = options.runtime_instance_id,
        } };
    }
    if (runtime_build_digest != null) return error.InvalidArguments;
    return .{ .attach = options };
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
