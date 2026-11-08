const std = @import("std");

const argument_options = @import("argument_options.zig");
const local_ipc = @import("terminal-host-transport");
const terminal_configuration = @import("terminal-host-model").configuration;

const editor_document_directory_option = "--editor-document-directory";
const endpoint_id_option = "--endpoint-id";
const runtime_directory_option = "--runtime-directory";
const scrollback_bytes_option = "--scrollback-bytes";
const serve_option_names = [_][]const u8{
    editor_document_directory_option,
    endpoint_id_option,
    runtime_directory_option,
    scrollback_bytes_option,
};

pub const Command = union(enum) {
    version,
    serve: ServeOptions,
};

pub const ServeOptions = struct {
    /// Where editor documents go; a Host started without it opens no editor.
    editor_document_directory: ?[]const u8,
    endpoint_id: []const u8,
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

    if (!std.mem.eql(u8, arguments[0], "serve")) return error.InvalidArguments;

    return .{ .serve = try parseServeOptions(options) };
}

fn parseServeOptions(options: argument_options.Options) !ServeOptions {
    try options.ensureOnly(&serve_option_names);

    const runtime_directory = try options.requireOne(runtime_directory_option);
    const endpoint_id = try options.requireOne(endpoint_id_option);
    const editor_document_directory = try options.optionalOne(editor_document_directory_option);
    var terminal_defaults: terminal_configuration.Configuration = .{};
    if (try options.optionalOne(scrollback_bytes_option)) |value| {
        terminal_defaults.scrollback_bytes = argument_options.parseInteger(usize, value) catch
            return error.InvalidScrollbackBytes;
    }

    if (!std.fs.path.isAbsolute(runtime_directory)) return error.RuntimeDirectoryNotAbsolute;
    if (editor_document_directory) |directory| {
        if (!std.fs.path.isAbsolute(directory)) return error.EditorDocumentDirectoryNotAbsolute;
    }
    if (!local_ipc.validEndpointId(endpoint_id)) return error.InvalidEndpointId;
    if (!terminal_configuration.valid(terminal_defaults)) return error.InvalidScrollbackBytes;
    return .{
        .editor_document_directory = editor_document_directory,
        .endpoint_id = endpoint_id,
        .runtime_directory = runtime_directory,
        .terminal_defaults = terminal_defaults,
    };
}
