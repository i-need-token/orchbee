const std = @import("std");

const arguments = @import("ssh-runtime-arguments");
const attachment = @import("ssh-runtime-attachment");
const project_root_service = @import("ssh-project-root-service");
const runtime_server = @import("ssh-runtime-server");
const terminal_host_relay = @import("ssh-terminal-host-relay");

pub fn main(init: std.process.Init) !void {
    const argv = try init.minimal.args.toSlice(init.arena.allocator());
    if (argv.len == 0) return error.InvalidArguments;
    switch (try arguments.parse(argv[1..])) {
        .version => try writeVersion(init.io),
        .project_root_worker => try project_root_service.runWorker(init.io, init.gpa),
        .terminal_host_relay_worker => |options| try terminal_host_relay.runWorker(
            init.io,
            init.gpa,
            options.executable_path,
            options.runtime_directory,
            options.endpoint_id,
        ),
        .serve => |options| {
            const home_directory = init.environ_map.get("HOME") orelse
                return error.HomeDirectoryUnavailable;
            if (!project_root_service.validPath(home_directory)) {
                return error.HomeDirectoryUnavailable;
            }
            const configured_login_shell = init.environ_map.get("SHELL") orelse "/bin/sh";
            const login_shell = if (project_root_service.validPath(configured_login_shell))
                configured_login_shell
            else
                "/bin/sh";
            try runtime_server.serve(
                init.io,
                init.gpa,
                home_directory,
                login_shell,
                options.runtime_directory,
                options.runtime_instance_id,
                options.runtime_build_digest,
                options.release_cohort_digest,
                .{
                    .build_digest = options.terminal_host.build_digest,
                    .endpoint_id = options.terminal_host.endpoint_id,
                    .executable_path = options.terminal_host.executable_path,
                    .runtime_directory = options.terminal_host.runtime_directory,
                    .version = options.terminal_host.version,
                },
            );
        },
        .attach => |options| try attachment.attach(
            init.io,
            init.gpa,
            options.runtime_directory,
            options.runtime_instance_id,
        ),
    }
}

fn writeVersion(io: std.Io) !void {
    var buffer: [192]u8 = undefined;
    var stdout = std.Io.File.stdout().writerStreaming(io, &buffer);
    try stdout.interface.print(
        "orchbee-ssh-environment-runtime {s} {s}\n",
        .{
            runtime_server.runtime_version,
            runtime_server.runtime_platform,
        },
    );
    try stdout.interface.flush();
}
