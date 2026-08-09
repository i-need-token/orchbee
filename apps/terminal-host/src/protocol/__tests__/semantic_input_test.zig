const std = @import("std");
const subject = @import("terminal-input");

test "mouse button set preserves wire identity" {
    const buttons = subject.MouseButtons.fromValidatedBits(
        (@as(u16, 1) << (@intFromEnum(subject.MouseButton.right) - 1)) |
            (@as(u16, 1) << (@intFromEnum(subject.MouseButton.five) - 1)),
    );

    try std.testing.expect(buttons.any());
    try std.testing.expect(buttons.contains(.right));
    try std.testing.expect(buttons.contains(.five));
    try std.testing.expect(!buttons.contains(.left));
}

test "mouse motion distinguishes hover from drag" {
    const hover: subject.MouseAction = .{ .motion = .hover };
    const drag: subject.MouseAction = .{ .motion = .{ .drag = .left } };

    switch (hover.motion) {
        .hover => {},
        .drag => return error.TestUnexpectedResult,
    }
    switch (drag.motion) {
        .hover => return error.TestUnexpectedResult,
        .drag => |button| try std.testing.expectEqual(subject.MouseButton.left, button),
    }
}

test "modifier fields retain the validated wire bit layout" {
    const modifiers = subject.Modifiers.fromValidatedBits(
        (@as(u16, 1) << 1) | (@as(u16, 1) << 7),
    );

    try std.testing.expect(modifiers.control);
    try std.testing.expect(modifiers.control_right);
    try std.testing.expect(!modifiers.shift);
    try std.testing.expectEqual(@as(u16, 0x0082), modifiers.bits());
}
