const std = @import("std");

const argument_options = @import("terminal-host-cli").argument_options;

const bytes_option = "--bytes";
const cell_pixel_height_option = "--cell-pixel-height";
const cell_pixel_width_option = "--cell-pixel-width";
const columns_option = "--columns";
const file_option = "--file";
const mode_option = "--mode";
const rows_option = "--rows";
const benchmark_option_names = [_][]const u8{
    bytes_option,
    cell_pixel_height_option,
    cell_pixel_width_option,
    columns_option,
    file_option,
    mode_option,
    rows_option,
};

pub const Mode = enum {
    active,
    no_demand,
};

pub const Config = struct {
    bytes: u64 = 64 * 1024 * 1024,
    cell_pixel_height: u16 = 18,
    cell_pixel_width: u16 = 9,
    columns: u16 = 120,
    file_path: ?[]const u8 = null,
    mode: Mode = .no_demand,
    rows: u16 = 40,
};

pub const Command = union(enum) {
    emit: u64,
    benchmark: Config,
};

pub fn parse(arguments: []const []const u8) !Command {
    if (arguments.len == 2 and std.mem.eql(u8, arguments[0], "--emit")) {
        const byte_count = try argument_options.parseInteger(u64, arguments[1]);
        if (byte_count == 0) return error.InvalidArguments;
        return .{ .emit = byte_count };
    }
    return .{ .benchmark = try parseBenchmark(arguments) };
}

fn parseBenchmark(arguments: []const []const u8) !Config {
    const options = try argument_options.Options.parse(arguments);
    try options.ensureOnly(&benchmark_option_names);

    var config: Config = .{};
    const bytes_explicit = options.get(bytes_option) != null;
    if (options.get(bytes_option)) |value| {
        config.bytes = try argument_options.parseInteger(u64, value);
    }
    if (options.get(cell_pixel_height_option)) |value| {
        config.cell_pixel_height = try argument_options.parseInteger(u16, value);
    }
    if (options.get(cell_pixel_width_option)) |value| {
        config.cell_pixel_width = try argument_options.parseInteger(u16, value);
    }
    if (options.get(columns_option)) |value| {
        config.columns = try argument_options.parseInteger(u16, value);
    }
    if (options.get(file_option)) |value| {
        if (!std.fs.path.isAbsolute(value)) return error.InvalidArguments;
        config.file_path = value;
    }
    if (options.get(rows_option)) |value| {
        config.rows = try argument_options.parseInteger(u16, value);
    }
    if (options.get(mode_option)) |value| {
        config.mode = if (std.mem.eql(u8, value, "no-demand"))
            .no_demand
        else if (std.mem.eql(u8, value, "active"))
            .active
        else
            return error.InvalidArguments;
    }
    if (config.file_path != null and bytes_explicit) return error.InvalidArguments;
    if (config.bytes == 0 or config.cell_pixel_height == 0 or
        config.cell_pixel_width == 0 or config.columns == 0 or config.rows == 0)
    {
        return error.InvalidArguments;
    }
    return config;
}
