const std = @import("std");

const subject = @import("terminal-host-cli").argument_options;
const Options = subject.Options;

test "argument options preserve flags, scalar values, and repeated values" {
    var options = try Options.parse(std.testing.allocator, &.{
        "--trace",
        "--rows",
        "40",
        "--mode",
        "active",
        "--rows",
        "41",
        "--trace",
    });
    defer options.deinit();
    try options.ensureOnly(&.{ "--mode", "--rows", "--trace" });
    try options.requireFlag("--trace");
    const row_values = try options.requireMany("--rows");
    try std.testing.expectEqual(@as(usize, 2), row_values.len);
    switch (try options.require("--rows")) {
        .many => |values| {
            try std.testing.expectEqualStrings("40", values[0]);
            try std.testing.expectEqualStrings("41", values[1]);
        },
        else => return error.ExpectedManyValues,
    }
    switch (try options.require("--mode")) {
        .one => |value| try std.testing.expectEqualStrings("active", value),
        else => return error.ExpectedOneValue,
    }
    try std.testing.expectError(error.InvalidArguments, options.requireOne("--rows"));
    try std.testing.expectEqualStrings("active", (try options.optionalOne("--mode")).?);
    try std.testing.expect(options.get("--missing") == null);
}

test "argument options reject malformed and unsupported entries" {
    try std.testing.expectError(
        error.InvalidArguments,
        Options.parse(std.testing.allocator, &.{ "rows", "40" }),
    );
    try std.testing.expectError(
        error.InvalidArguments,
        Options.parse(std.testing.allocator, &.{ "--", "40" }),
    );
    try std.testing.expectError(
        error.InvalidArguments,
        Options.parse(std.testing.allocator, &.{ "--trace", "--trace", "enabled" }),
    );

    var options = try Options.parse(std.testing.allocator, &.{ "--unknown", "value" });
    defer options.deinit();
    try std.testing.expectError(
        error.InvalidArguments,
        options.ensureOnly(&.{"--rows"}),
    );
    try std.testing.expectError(error.InvalidArguments, options.require("--rows"));
}

test "argument integer conversion normalizes malformed and overflowing values" {
    try std.testing.expectEqual(@as(u16, 40), try subject.parseInteger(u16, "40"));
    try std.testing.expectError(error.InvalidArguments, subject.parseInteger(u16, "invalid"));
    try std.testing.expectError(error.InvalidArguments, subject.parseInteger(u16, "65536"));
}
