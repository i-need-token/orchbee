const std = @import("std");
const subject = @import("terminal-host-benchmark").testing.BenchmarkOptionsModule;

const Config = subject.Config;
const Mode = subject.Mode;
const parse = subject.parse;

fn parseBenchmark(arguments: []const []const u8) !Config {
    return switch (try parse(arguments)) {
        .benchmark => |config| config,
        .emit => error.UnexpectedEmitCommand,
    };
}

test "benchmark arguments keep no-demand as the default" {
    const defaults = try parseBenchmark(&.{});
    try std.testing.expectEqual(Mode.no_demand, defaults.mode);

    const parsed = try parseBenchmark(&.{
        "--bytes",
        "524288000",
        "--columns",
        "120",
        "--rows",
        "40",
    });
    try std.testing.expectEqual(Mode.no_demand, parsed.mode);
    try std.testing.expectEqual(@as(u64, 524_288_000), parsed.bytes);
    try std.testing.expectError(error.InvalidArguments, parseBenchmark(&.{
        "--mode",
        "unknown",
    }));
}

test "benchmark arguments accept one absolute file fixture with explicit cell geometry" {
    const path = if (@import("builtin").os.tag == .windows)
        "C:\\orchbee\\fixture.txt"
    else
        "/tmp/orchbee-fixture.txt";
    const parsed = try parseBenchmark(&.{
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
    try std.testing.expectError(error.InvalidArguments, parseBenchmark(&.{
        "--file",
        "relative.txt",
    }));
    const path = if (@import("builtin").os.tag == .windows)
        "C:\\orchbee\\fixture.txt"
    else
        "/tmp/orchbee-fixture.txt";
    try std.testing.expectError(error.InvalidArguments, parseBenchmark(&.{
        "--bytes",
        "500000000",
        "--file",
        path,
    }));
    try std.testing.expectError(error.InvalidArguments, parseBenchmark(&.{
        "--mode",
        "active",
        "--mode",
        "no-demand",
    }));
}

test "benchmark arguments recognize only the exact internal emit command" {
    const command = try parse(&.{ "--emit", "4096" });
    switch (command) {
        .emit => |byte_count| try std.testing.expectEqual(@as(u64, 4096), byte_count),
        .benchmark => return error.ExpectedEmitCommand,
    }
    try std.testing.expectError(error.InvalidArguments, parse(&.{ "--emit", "0" }));
    try std.testing.expectError(error.InvalidArguments, parse(&.{"--emit"}));
    try std.testing.expectError(error.InvalidArguments, parse(&.{ "--emit", "1", "extra" }));
    try std.testing.expectError(error.InvalidArguments, parse(&.{ "--emit", "invalid" }));
}
