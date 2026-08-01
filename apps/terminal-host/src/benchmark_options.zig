const std = @import("std");

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

pub fn parse(arguments: []const []const u8) !Config {
    var config: Config = .{};
    var bytes_explicit = false;
    var index: usize = 1;
    while (index < arguments.len) : (index += 2) {
        if (index + 1 >= arguments.len) return error.InvalidArguments;
        const name = arguments[index];
        const value = arguments[index + 1];
        if (std.mem.eql(u8, name, "--bytes")) {
            config.bytes = try std.fmt.parseInt(u64, value, 10);
            bytes_explicit = true;
        } else if (std.mem.eql(u8, name, "--cell-pixel-height")) {
            config.cell_pixel_height = try std.fmt.parseInt(u16, value, 10);
        } else if (std.mem.eql(u8, name, "--cell-pixel-width")) {
            config.cell_pixel_width = try std.fmt.parseInt(u16, value, 10);
        } else if (std.mem.eql(u8, name, "--columns")) {
            config.columns = try std.fmt.parseInt(u16, value, 10);
        } else if (std.mem.eql(u8, name, "--file")) {
            if (!std.fs.path.isAbsolute(value)) return error.InvalidArguments;
            config.file_path = value;
        } else if (std.mem.eql(u8, name, "--rows")) {
            config.rows = try std.fmt.parseInt(u16, value, 10);
        } else if (std.mem.eql(u8, name, "--mode")) {
            config.mode = if (std.mem.eql(u8, value, "no-demand"))
                .no_demand
            else if (std.mem.eql(u8, value, "active"))
                .active
            else
                return error.InvalidArguments;
        } else {
            return error.InvalidArguments;
        }
    }
    if (config.file_path != null and bytes_explicit) return error.InvalidArguments;
    if (config.bytes == 0 or config.cell_pixel_height == 0 or
        config.cell_pixel_width == 0 or config.columns == 0 or config.rows == 0)
    {
        return error.InvalidArguments;
    }
    return config;
}

test "benchmark arguments keep no-demand as the default" {
    const parsed = try parse(&.{
        "terminal-host-benchmark",
        "--bytes",
        "524288000",
        "--columns",
        "120",
        "--rows",
        "40",
    });
    try std.testing.expectEqual(Mode.no_demand, parsed.mode);
    try std.testing.expectEqual(@as(u64, 524_288_000), parsed.bytes);
    try std.testing.expectError(error.InvalidArguments, parse(&.{
        "terminal-host-benchmark",
        "--mode",
        "unknown",
    }));
}

test "benchmark arguments accept one absolute file fixture with explicit cell geometry" {
    const path = if (@import("builtin").os.tag == .windows)
        "C:\\orchbee\\fixture.txt"
    else
        "/tmp/orchbee-fixture.txt";
    const parsed = try parse(&.{
        "terminal-host-benchmark",
        "--file",
        path,
        "--cell-pixel-width",
        "18",
        "--cell-pixel-height",
        "36",
        "--columns",
        "113",
        "--rows",
        "38",
        "--mode",
        "active",
    });
    try std.testing.expectEqualStrings(path, parsed.file_path.?);
    try std.testing.expectEqual(@as(u16, 18), parsed.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 36), parsed.cell_pixel_height);
    try std.testing.expectEqual(@as(u16, 113), parsed.columns);
    try std.testing.expectEqual(@as(u16, 38), parsed.rows);
    try std.testing.expectEqual(Mode.active, parsed.mode);
}

test "benchmark arguments reject ambiguous or relative file fixtures" {
    try std.testing.expectError(error.InvalidArguments, parse(&.{
        "terminal-host-benchmark",
        "--file",
        "relative.txt",
    }));
    const path = if (@import("builtin").os.tag == .windows)
        "C:\\orchbee\\fixture.txt"
    else
        "/tmp/orchbee-fixture.txt";
    try std.testing.expectError(error.InvalidArguments, parse(&.{
        "terminal-host-benchmark",
        "--bytes",
        "500000000",
        "--file",
        path,
    }));
}
