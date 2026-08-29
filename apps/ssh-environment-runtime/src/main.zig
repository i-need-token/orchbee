const std = @import("std");

const arguments = @import("ssh-runtime-arguments");
const attachment = @import("ssh-runtime-attachment");
const project_root_service = @import("ssh-project-root-service");
const runtime_server = @import("ssh-runtime-server");

pub fn main(init: std.process.Init) !void {
    const argv = try init.minimal.args.toSlice(init.arena.allocator());
    if (argv.len == 0) return error.InvalidArguments;
    switch (try arguments.parse(argv[1..])) {
        .version => try writeVersion(init.io),
        .project_root_worker => try project_root_service.runWorker(init.io),
        .serve => |options| try runtime_server.serve(
            init.io,
            init.gpa,
            options.runtime_directory,
            options.runtime_instance_id,
            options.runtime_build_digest,
        ),
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
