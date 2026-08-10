const std = @import("std");
const builtin = @import("builtin");
const subject = @import("terminal-host-transport");

const proxy = subject.testing.posixProxy;

const ProxyThreadContext = struct {
    failure: ?anyerror = null,
    input: std.posix.fd_t,
    output: std.posix.fd_t,
    socket: std.posix.socket_t,
};

const ServerWriterContext = struct {
    bytes: []const u8,
    failure: ?anyerror = null,
    stream: std.net.Stream,
};

test "POSIX helper proxy applies bounded bidirectional backpressure and releases descriptors" {
    if (builtin.os.tag == .windows) return error.SkipZigTest;
    var temporary = std.testing.tmpDir(.{});
    defer temporary.cleanup();
    var original_directory = try std.fs.cwd().openDir(".", .{});
    defer original_directory.close();
    try std.posix.fchdir(temporary.dir.fd);
    defer std.posix.fchdir(original_directory.fd) catch {};
    const address = try std.net.Address.initUnix("proxy.s");
    var listener = try address.listen(.{ .kernel_backlog = 1 });
    defer listener.deinit();
    defer temporary.dir.deleteFile("proxy.s") catch {};

    const client = try std.net.connectUnixSocket("proxy.s");
    var accepted = try listener.accept();
    const input_pipe = try std.posix.pipe();
    const output_pipe = try std.posix.pipe();
    var proxy_context: ProxyThreadContext = .{
        .input = input_pipe[0],
        .output = output_pipe[1],
        .socket = client.handle,
    };
    const proxy_thread = try std.Thread.spawn(.{}, runProxy, .{&proxy_context});

    const request = "authenticated request bytes";
    var input_writer: std.fs.File = .{ .handle = input_pipe[1] };
    try input_writer.writeAll(request);
    input_writer.close();
    var received_request: [request.len]u8 = undefined;
    try readExactly(accepted.stream, &received_request);
    try std.testing.expectEqualSlices(u8, request, &received_request);
    var end_probe: [1]u8 = undefined;
    try std.testing.expectEqual(@as(usize, 0), try accepted.stream.read(&end_probe));

    const response_length = proxy.buffer_bytes * 2 + 17;
    const response = try std.testing.allocator.alloc(u8, response_length);
    defer std.testing.allocator.free(response);
    for (response, 0..) |*byte, index| byte.* = @intCast(index % 251);
    var writer_context: ServerWriterContext = .{
        .bytes = response,
        .stream = accepted.stream,
    };
    const writer_thread = try std.Thread.spawn(.{}, runServerWriter, .{&writer_context});

    var output_reader: std.fs.File = .{ .handle = output_pipe[0] };
    defer output_reader.close();
    const received_response = try output_reader.readToEndAlloc(
        std.testing.allocator,
        response_length + 1,
    );
    defer std.testing.allocator.free(received_response);
    writer_thread.join();
    proxy_thread.join();
    if (writer_context.failure) |failure| return failure;
    if (proxy_context.failure) |failure| return failure;
    try std.testing.expectEqualSlices(u8, response, received_response);
}

fn runProxy(context: *ProxyThreadContext) void {
    defer std.posix.close(context.input);
    defer std.posix.close(context.output);
    defer std.posix.close(context.socket);
    proxy.run(context.socket, context.input, context.output) catch |failure| {
        context.failure = failure;
    };
}

fn runServerWriter(context: *ServerWriterContext) void {
    defer context.stream.close();
    context.stream.writeAll(context.bytes) catch |failure| {
        context.failure = failure;
    };
}

fn readExactly(stream: std.net.Stream, bytes: []u8) !void {
    var offset: usize = 0;
    while (offset < bytes.len) {
        const length = try stream.read(bytes[offset..]);
        if (length == 0) return error.UnexpectedEndOfStream;
        offset += length;
    }
}
