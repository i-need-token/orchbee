//! What the program was last told of focus, keys, and buttons. The export reads each attachment's
//! held state. The import replays it through the ledger's own rules under fresh attachment ids;
//! the importing Host's owner detaches them once it claims the terminal, so the program hears the
//! releases a detached attachment's repair sends.
const std = @import("std");
const terminal_input = @import("terminal-input");
const identifiers = @import("terminal-host-model").identifiers;
const thread_sync = @import("terminal-host-platform").thread_sync;
const Held = @import("../handover/root.zig").engine_section.held_input.Held;
const input_ownership = @import("../input_ownership.zig");
const attachment_ownership = @import("../input_ownership_attachment.zig");

const AttachmentId = identifiers.AttachmentId;
const Ledger = input_ownership.Ledger;

/// Every slice of the result lives in `arena`, except key text, which the ledger owns; encode the
/// record before the input lock is released.
pub fn exportHeld(ledger: *const Ledger, arena: std.mem.Allocator) error{ InputPending, OutOfMemory }![]const Held {
    var result: std.ArrayList(Held) = .empty;
    for (ledger.attachments.values()) |*attachment| {
        // A repair or a key change is between its reservation and its commit.
        if (attachment.repairing or attachment.pending_key_slots != 0) return error.InputPending;
        if (!attachment.focused and attachment.keys.items.len == 0 and attachment.mouse_order_len == 0) continue;
        const keys = try arena.alloc(terminal_input.KeyInput, attachment.keys.items.len);
        for (keys, attachment.keys.items) |*key, owned| {
            key.* = owned.release();
            key.action = .press;
        }
        try result.append(arena, .{
            .focused = attachment.focused,
            .keys = keys,
            .buttons = try arena.dupe(terminal_input.MouseButton, attachment.mouse_order[0..attachment.mouse_order_len]),
            .sample = attachment.mouse_sample,
        });
    }
    return result.items;
}

/// Replays `held` into an empty ledger and writes the attachment id each entry received to `ids`.
/// A record the ledger would not reach this way is refused.
pub fn importHeld(ledger: *Ledger, held: []const Held, ids: []AttachmentId) error{ InvalidRecord, OutOfMemory }!void {
    std.debug.assert(ids.len == held.len);
    for (held, ids) |entry, *id| {
        id.* = freshId(ledger);
        try ledger.ensureAttachmentCapacity();
        ledger.commitAttachment(id.*);
        if (entry.focused) {
            const focus = ledger.reserveFocusMutation(id.*, true) catch return error.InvalidRecord;
            try apply(ledger, focus.ownership);
        }
        for (entry.keys) |key| {
            var mutation = (input_ownership.mutationForInput(ledger.allocator, .{ .key = key }) catch |err| return switch (err) {
                error.OutOfMemory => error.OutOfMemory,
                else => error.InvalidRecord,
            }) orelse return error.InvalidRecord;
            if (ledger.attachments.getPtr(id.*).?.keyIndex(mutation.key_retain.key) != null) {
                mutation.deinit(ledger.allocator);
                return error.InvalidRecord;
            }
            try replay(ledger, id.*, &mutation);
        }
        var pressed: terminal_input.MouseButtons = .{};
        for (entry.buttons) |button| {
            const sample = entry.sample orelse return error.InvalidRecord;
            attachment_ownership.setMouseButton(&pressed, button, true);
            var mutation: input_ownership.Mutation = .{ .mouse = .{
                .action = .{ .press = button },
                .modifiers = sample.modifiers,
                .pressed_buttons = pressed,
                .x = sample.x,
                .y = sample.y,
            } };
            try replay(ledger, id.*, &mutation);
        }
        if (entry.sample) |sample| {
            var mutation: input_ownership.Mutation = .{ .mouse = sample };
            try replay(ledger, id.*, &mutation);
        }
        const attachment = ledger.attachments.getPtr(id.*).?;
        if (!std.mem.eql(terminal_input.MouseButton, attachment.mouse_order[0..attachment.mouse_order_len], entry.buttons) or
            !std.meta.eql(attachment.mouse_sample, entry.sample)) return error.InvalidRecord;
    }
}

fn replay(ledger: *Ledger, id: AttachmentId, mutation: *input_ownership.Mutation) error{ InvalidRecord, OutOfMemory }!void {
    const prepared = ledger.reserveMutation(id, mutation) catch |err| {
        mutation.deinit(ledger.allocator);
        return switch (err) {
            error.OutOfMemory => error.OutOfMemory,
            else => error.InvalidRecord,
        };
    };
    try apply(ledger, prepared);
}

fn apply(ledger: *Ledger, prepared: input_ownership.PreparedMutation) error{InvalidRecord}!void {
    var pending: ?input_ownership.PreparedMutation = prepared;
    ledger.validatePrepared(pending, .{}) catch {
        ledger.cancelPrepared(&pending.?);
        return error.InvalidRecord;
    };
    ledger.commitPrepared(&pending);
}

fn freshId(ledger: *const Ledger) AttachmentId {
    while (true) {
        var bytes: [16]u8 = undefined;
        thread_sync.io().random(&bytes);
        if (std.mem.allEqual(u8, &bytes, 0)) continue;
        const id: AttachmentId = .fromBytes(bytes);
        if (!ledger.contains(id)) return id;
    }
}
