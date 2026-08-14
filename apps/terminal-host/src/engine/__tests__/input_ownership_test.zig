const std = @import("std");

const subject = @import("terminal-host-engine").testing.input_ownership;

test "accepted held key owns bounded repair credit until release" {
    var ledger = subject.Ledger.init(std.testing.allocator);
    defer ledger.deinit();
    const attachment_id = subject.AttachmentId.fromBytes([_]u8{0x11} ** 16);
    try ledger.ensureAttachmentCapacity();
    ledger.commitAttachment(attachment_id);

    var press = (try subject.mutationForInput(std.testing.allocator, .{ .key = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{},
        .text = "a",
        .unshifted_codepoint = 'a',
    } })).?;
    var prepared_press: ?subject.PreparedMutation = try ledger.reserveMutation(
        attachment_id,
        &press,
    );
    try ledger.validatePrepared(prepared_press, .{});
    ledger.commitPrepared(&prepared_press);

    const held_charge = ledger.repairCharge();
    try std.testing.expect(held_charge.bytes > 0);
    try std.testing.expectEqual(@as(usize, 1), held_charge.entries);

    var release = (try subject.mutationForInput(std.testing.allocator, .{ .key = .{
        .action = .release,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyA",
        .modifiers = .{},
        .text = "a",
        .unshifted_codepoint = 'a',
    } })).?;
    var prepared_release: ?subject.PreparedMutation = try ledger.reserveMutation(
        attachment_id,
        &release,
    );
    try ledger.validatePrepared(prepared_release, .{});
    ledger.commitPrepared(&prepared_release);

    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().bytes);
    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().entries);
}

test "cancelled held input mutates neither ledger nor repair charge" {
    var ledger = subject.Ledger.init(std.testing.allocator);
    defer ledger.deinit();
    const attachment_id = subject.AttachmentId.fromBytes([_]u8{0x22} ** 16);
    try ledger.ensureAttachmentCapacity();
    ledger.commitAttachment(attachment_id);

    var mutation = (try subject.mutationForInput(std.testing.allocator, .{ .key = .{
        .action = .press,
        .composing = false,
        .consumed_modifiers = .{},
        .key = "KeyB",
        .modifiers = .{},
        .text = "b",
        .unshifted_codepoint = 'b',
    } })).?;
    var prepared = try ledger.reserveMutation(attachment_id, &mutation);
    ledger.cancelPrepared(&prepared);

    try std.testing.expect(ledger.contains(attachment_id));
    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().bytes);
    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().entries);
}
