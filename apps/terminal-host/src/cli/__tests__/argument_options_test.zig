const std = @import("std");

const subject = @import("terminal-host-cli").argument_options;
const Options = subject.Options;

test "argument options provide unordered dictionary-style lookup" {
    const options = try Options.parse(&.{
        "--rows",
        "40",
        "--mode",
        "active",
    });
    try options.ensureOnly(&.{ "--mode", "--rows" });
    try std.testing.expectEqualStrings("40", try options.require("--rows"));
    try std.testing.expectEqualStrings("active", options.get("--mode").?);
    try std.testing.expect(options.get("--missing") == null);
}

test "argument options reject malformed duplicate and unsupported entries" {
    try std.testing.expectError(error.InvalidArguments, Options.parse(&.{"--rows"}));
    try std.testing.expectError(error.InvalidArguments, Options.parse(&.{ "rows", "40" }));
    try std.testing.expectError(error.InvalidArguments, Options.parse(&.{ "--", "40" }));
    try std.testing.expectError(error.InvalidArguments, Options.parse(&.{
        "--rows",
        "40",
        "--rows",
        "41",
    }));

    const options = try Options.parse(&.{ "--unknown", "value" });
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
