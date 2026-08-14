const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const selection = @import("terminal-host-engine").testing.selection;

test "selection state owns and releases its Ghostty pins" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");

    var state: selection.State = .{};
    defer state.clear(&terminal);
    const initial_pins = terminal.screens.active.pages.countTrackedPins();
    const snapshot = try state.applyCommand(&terminal, 1, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 1,
    });
    try std.testing.expectEqual(selection.Snapshot.active, std.meta.activeTag(snapshot));
    try std.testing.expect(state.active());
    try std.testing.expect(terminal.screens.active.pages.countTrackedPins() > initial_pins);

    state.screenChanged(&terminal);
    try std.testing.expect(!state.active());
    try std.testing.expectEqual(
        selection.Snapshot.evicted,
        std.meta.activeTag(state.takeEvent().?),
    );
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
}

test "selection state rejects stale identity before mutation" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var state: selection.State = .{};
    defer state.clear(&terminal);

    try std.testing.expectError(error.StaleRevision, state.applyCommand(&terminal, 2, 2, .{
        .command = selection.Command.clear,
        .generation = 1,
        .revision = 1,
    }));
    try std.testing.expect(!state.active());
}
