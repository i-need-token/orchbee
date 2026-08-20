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

test "focus claims own one aggregate repair credit across attachments" {
    var ledger = subject.Ledger.init(std.testing.allocator);
    defer ledger.deinit();
    const first_attachment = subject.AttachmentId.fromBytes([_]u8{0x31} ** 16);
    const second_attachment = subject.AttachmentId.fromBytes([_]u8{0x32} ** 16);
    for ([_]subject.AttachmentId{ first_attachment, second_attachment }) |attachment_id| {
        try ledger.ensureAttachmentCapacity();
        ledger.commitAttachment(attachment_id);
    }

    const first_claim = try ledger.reserveFocusMutation(first_attachment, true);
    try std.testing.expectEqual(subject.FocusTransition.gained, first_claim.transition);
    var first_ownership: ?subject.PreparedMutation = first_claim.ownership;
    try ledger.validatePrepared(first_ownership, .{});
    ledger.commitPrepared(&first_ownership);
    const aggregate_charge = ledger.repairCharge();
    try std.testing.expect(aggregate_charge.bytes > 0);
    try std.testing.expectEqual(@as(usize, 1), aggregate_charge.entries);

    const second_claim = try ledger.reserveFocusMutation(second_attachment, true);
    try std.testing.expectEqual(subject.FocusTransition.none, second_claim.transition);
    var second_ownership: ?subject.PreparedMutation = second_claim.ownership;
    try ledger.validatePrepared(second_ownership, .{});
    ledger.commitPrepared(&second_ownership);
    try std.testing.expectEqual(aggregate_charge, ledger.repairCharge());
    try std.testing.expectEqual(@as(usize, 2), ledger.focusedCount());

    const first_release = try ledger.reserveFocusMutation(first_attachment, false);
    try std.testing.expectEqual(subject.FocusTransition.none, first_release.transition);
    var first_release_ownership: ?subject.PreparedMutation = first_release.ownership;
    try ledger.validatePrepared(first_release_ownership, .{});
    ledger.commitPrepared(&first_release_ownership);
    try std.testing.expectEqual(aggregate_charge, ledger.repairCharge());

    const second_release = try ledger.reserveFocusMutation(second_attachment, false);
    try std.testing.expectEqual(subject.FocusTransition.lost, second_release.transition);
    var second_release_ownership: ?subject.PreparedMutation = second_release.ownership;
    try ledger.validatePrepared(second_release_ownership, .{});
    ledger.commitPrepared(&second_release_ownership);
    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().bytes);
    try std.testing.expectEqual(@as(usize, 0), ledger.repairCharge().entries);
    try std.testing.expect(ledger.invariantHolds());
}
