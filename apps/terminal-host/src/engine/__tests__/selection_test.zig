const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const selection = @import("terminal-host-engine").testing.selection;

test "selection state owns and releases its Ghostty pins" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
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

test "select all uses Ghostty content bounds instead of viewport bounds" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 12,
        .rows = 5,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("  alpha  ");

    var state: selection.State = .{};
    defer state.clear(&terminal);
    const snapshot = try state.applyCommand(&terminal, 1, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 1,
    });

    try std.testing.expectEqual(selection.Snapshot.active, std.meta.activeTag(snapshot));
    try std.testing.expectEqual(@as(u16, 2), snapshot.active.start.column);
    try std.testing.expectEqual(@as(u64, 0), snapshot.active.start.row);
    try std.testing.expectEqual(@as(u16, 6), snapshot.active.end.column);
    try std.testing.expectEqual(@as(u64, 0), snapshot.active.end.row);
    try std.testing.expectEqualStrings("alpha", snapshot.previewText().?);
}

test "selection snapshot publishes only a complete bounded preview" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = @intCast(selection.max_preview_bytes),
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    const text = [_]u8{'x'} ** selection.max_preview_bytes;
    try terminal.printString(&text);

    var state: selection.State = .{};
    defer state.clear(&terminal);
    const at_limit = try state.applyCommand(&terminal, 1, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 1,
    });
    try std.testing.expectEqual(
        selection.max_preview_bytes,
        at_limit.previewText().?.len,
    );

    try terminal.printString("x");
    const above_limit = try state.applyCommand(&terminal, 1, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 1,
    });
    try std.testing.expect(above_limit.previewText() == null);
}

test "select all leaves an empty terminal unselected" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .rows = 5,
    });
    defer terminal.deinit(std.testing.allocator);

    var state: selection.State = .{};
    defer state.clear(&terminal);
    const snapshot = try state.applyCommand(&terminal, 1, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 1,
    });

    try std.testing.expectEqual(selection.Snapshot.none, std.meta.activeTag(snapshot));
    try std.testing.expect(!state.active());
}

test "a command naming a row rejects stale identity before mutation" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var state: selection.State = .{};
    defer state.clear(&terminal);

    try std.testing.expectError(error.StaleRevision, state.applyCommand(&terminal, 2, 1, .{
        .command = selection.Command{ .select_output = .{ .column = 0, .row = 0 } },
        .generation = 1,
        .revision = 1,
    }));
    try std.testing.expectError(error.StaleGeneration, state.applyCommand(&terminal, 2, 2, .{
        .command = selection.Command{ .set_range = .{
            .end = .{ .column = 1, .row = 0 },
            .rectangle = false,
            .start = .{ .column = 0, .row = 0 },
        } },
        .generation = 1,
        .revision = 2,
    }));
    try std.testing.expect(!state.active());
}

test "a command naming no row outlives output but not a replaced screen" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.io, std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha");
    var state: selection.State = .{};
    defer state.clear(&terminal);

    _ = try state.applyCommand(&terminal, 5, 1, .{
        .command = selection.Command.select_all,
        .generation = 1,
        .revision = 4,
    });
    try std.testing.expect(state.active());

    try std.testing.expectError(error.StaleGeneration, state.applyCommand(&terminal, 6, 2, .{
        .command = selection.Command.clear,
        .generation = 1,
        .revision = 6,
    }));
    try std.testing.expect(state.active());

    _ = try state.applyCommand(&terminal, 7, 1, .{
        .command = selection.Command.clear,
        .generation = 1,
        .revision = 6,
    });
    try std.testing.expect(!state.active());
}
