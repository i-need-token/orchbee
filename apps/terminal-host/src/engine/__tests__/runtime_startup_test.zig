const std = @import("std");

const subject = @import("terminal-host-engine").testing.runtime_startup;

test "runtime startup gate publishes one shared terminal decision" {
    var ready: subject.Gate = .{};
    ready.ready();
    try std.testing.expect(ready.await());
    try std.testing.expect(ready.await());

    var aborted: subject.Gate = .{};
    aborted.abort();
    try std.testing.expect(!aborted.await());
    try std.testing.expect(!aborted.await());
}
