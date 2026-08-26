const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const attachment_ownership = @import("input_ownership_attachment.zig");
const input_encoding = @import("input_encoding.zig");
const input_queue = @import("input_queue.zig");

const max_key_encoded_bytes = input_encoding.max_key_bytes;
const max_mouse_encoded_bytes = input_encoding.max_mouse_bytes;
const max_focus_encoded_bytes = input_encoding.max_focus_bytes;

pub const AttachmentId = identifiers.AttachmentId;
const AttachmentState = attachment_ownership.AttachmentState;
pub const mutationForInput = attachment_ownership.mutationForInput;

pub const FocusTransition = enum {
    none,
    gained,
    lost,
};

pub const Mutation = attachment_ownership.Mutation;

pub const PreparedMutation = struct {
    attachment_id: AttachmentId,
    attachment_revision: u64,
    key_slot_reserved: bool = false,
    mutation: union(enum) {
        repair: RepairDisposition,
        input: Mutation,
    },

    fn deinit(self: *PreparedMutation, allocator: std.mem.Allocator) void {
        switch (self.mutation) {
            .input => |*mutation| mutation.deinit(allocator),
            .repair => {},
        }
        self.* = undefined;
    }
};

pub const PreparedFocusMutation = struct {
    ownership: PreparedMutation,
    transition: FocusTransition,
};

pub const RepairDisposition = enum {
    remove,
    retain,
};

pub const PreparedRepair = struct {
    bytes: ?[]u8,
    ownership: PreparedMutation,
};

/// Owns only the held key, mouse, and focus facts needed to repair one
/// attachment's accepted input. Capability, visibility, and terminal binding
/// remain Core facts. TerminalRuntime serializes every operation with its
/// input mutex.
pub const Ledger = struct {
    allocator: std.mem.Allocator,
    attachments: std.AutoArrayHashMapUnmanaged(AttachmentId, AttachmentState),
    focused_count: usize = 0,
    repair_charge: input_queue.Charge = .{},

    pub fn init(allocator: std.mem.Allocator) Ledger {
        return .{
            .allocator = allocator,
            .attachments = .empty,
        };
    }

    pub fn deinit(self: *Ledger) void {
        for (self.attachments.values()) |*attachment| attachment.deinit(self.allocator);
        self.attachments.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn ensureAttachmentCapacity(self: *Ledger) !void {
        try self.attachments.ensureUnusedCapacity(self.allocator, 1);
    }

    pub fn commitAttachment(self: *Ledger, attachment_id: AttachmentId) void {
        self.attachments.putAssumeCapacityNoClobber(attachment_id, .{});
    }

    pub fn contains(self: *const Ledger, attachment_id: AttachmentId) bool {
        return self.attachments.contains(attachment_id);
    }

    pub fn attachmentFocused(self: *const Ledger, attachment_id: AttachmentId) bool {
        const attachment = self.attachments.get(attachment_id) orelse return false;
        return attachment.focused;
    }

    pub fn focusedCount(self: *const Ledger) usize {
        return self.focused_count;
    }

    pub fn invariantHolds(self: *const Ledger) bool {
        var observed_count: usize = 0;
        var expected_repair_bytes = self.focusRepairBytes();
        var expected_repair_entries = self.focusRepairEntries();
        for (self.attachments.values()) |attachment| {
            observed_count += @intFromBool(attachment.focused);
            expected_repair_bytes += attachment.repairBytes();
            expected_repair_entries += attachment.repairEntries();
        }
        if (observed_count != self.focused_count) return false;
        return self.repair_charge.bytes == expected_repair_bytes and
            self.repair_charge.entries == expected_repair_entries;
    }

    pub fn repairCharge(self: *const Ledger) input_queue.Charge {
        return self.repair_charge;
    }

    pub fn reserveMutation(
        self: *Ledger,
        attachment_id: AttachmentId,
        mutation: *Mutation,
    ) !PreparedMutation {
        const attachment = self.attachments.getPtr(attachment_id) orelse
            return error.MissingAttachment;
        if (attachment.repairing) return error.MissingAttachment;
        var key_slot_reserved = false;
        switch (mutation.*) {
            .key_retain => {
                try attachment.keys.ensureUnusedCapacity(
                    self.allocator,
                    attachment.pending_key_slots + 1,
                );
                attachment.pending_key_slots += 1;
                key_slot_reserved = true;
            },
            .focus, .key_release, .mouse => {},
        }
        const owned = mutation.*;
        mutation.* = undefined;
        return .{
            .attachment_id = attachment_id,
            .attachment_revision = attachment.revision,
            .key_slot_reserved = key_slot_reserved,
            .mutation = .{ .input = owned },
        };
    }

    pub fn reserveFocusMutation(
        self: *Ledger,
        attachment_id: AttachmentId,
        focused: bool,
    ) !PreparedFocusMutation {
        const attachment = self.attachments.getPtr(attachment_id) orelse
            return error.MissingAttachment;
        if (attachment.repairing) return error.MissingAttachment;
        const transition: FocusTransition = if (attachment.focused == focused)
            .none
        else if (focused and self.focused_count == 0)
            .gained
        else if (!focused and self.focused_count == 1)
            .lost
        else
            .none;
        return .{
            .ownership = .{
                .attachment_id = attachment_id,
                .attachment_revision = attachment.revision,
                .mutation = .{ .input = .{ .focus = .{
                    .expected_attachment_focused = attachment.focused,
                    .expected_focused_count = self.focused_count,
                    .focused = focused,
                } } },
            },
            .transition = transition,
        };
    }

    pub fn validatePrepared(
        self: *Ledger,
        prepared: ?PreparedMutation,
        client_charge: input_queue.Charge,
    ) !void {
        const pending = prepared orelse return;
        const current = self.attachments.getPtr(pending.attachment_id) orelse
            return error.MissingAttachment;
        if (current.revision != pending.attachment_revision) return error.MissingAttachment;
        switch (pending.mutation) {
            .repair => if (!current.repairing) return error.MissingAttachment,
            .input => |mutation| {
                if (current.repairing) return error.MissingAttachment;
                if (mutation == .focus) {
                    const focus = mutation.focus;
                    if (current.focused != focus.expected_attachment_focused) {
                        return error.MissingAttachment;
                    }
                    if (focus.focused != focus.expected_attachment_focused and
                        self.focused_count != focus.expected_focused_count)
                    {
                        return error.MissingAttachment;
                    }
                }
            },
        }

        var additional_repair_bytes: usize = 0;
        var additional_repair_entries: usize = 0;
        switch (pending.mutation) {
            .input => |mutation| switch (mutation) {
                .focus => |focus| if (focus.focused and
                    !focus.expected_attachment_focused and self.focused_count == 0)
                {
                    additional_repair_bytes = max_focus_encoded_bytes;
                    additional_repair_entries = 1;
                },
                .key_retain => |key| if (current.keyIndex(key.key) == null) {
                    additional_repair_bytes = max_key_encoded_bytes;
                    additional_repair_entries = @intFromBool(current.repairEntries() == 0);
                },
                .mouse => |mouse| switch (mouse.action) {
                    .press => |button| if (mouse.pressed_buttons.contains(button) and
                        current.mouseIndex(button) == null)
                    {
                        additional_repair_bytes = max_mouse_encoded_bytes;
                        additional_repair_entries = @intFromBool(current.repairEntries() == 0);
                    },
                    .release, .motion => {},
                },
                .key_release => {},
            },
            .repair => {},
        }
        if (additional_repair_bytes > input_queue.client_capacity_bytes -|
            client_charge.bytes -| self.repair_charge.bytes)
        {
            return error.InputBackpressure;
        }
        if (additional_repair_entries > input_queue.client_capacity_entries -|
            client_charge.entries -| self.repair_charge.entries)
        {
            return error.InputBackpressure;
        }
    }

    pub fn commitPrepared(self: *Ledger, prepared: *?PreparedMutation) void {
        const pending = prepared.* orelse return;
        const attachment = self.attachments.getPtr(pending.attachment_id) orelse unreachable;
        if (pending.key_slot_reserved) {
            std.debug.assert(attachment.pending_key_slots > 0);
            attachment.pending_key_slots -= 1;
        }
        const previous_repair_bytes = attachment.repairBytes() + self.focusRepairBytes();
        const previous_repair_entries = attachment.repairEntries() + self.focusRepairEntries();
        switch (pending.mutation) {
            .repair => |disposition| {
                if (attachment.focused) {
                    std.debug.assert(self.focused_count > 0);
                    self.focused_count -= 1;
                    attachment.focused = false;
                }
                attachment.resetHeldState(self.allocator);
                const next_focus_bytes = self.focusRepairBytes();
                const next_focus_entries = self.focusRepairEntries();
                self.updateRepairCharge(
                    previous_repair_bytes,
                    previous_repair_entries,
                    next_focus_bytes,
                    next_focus_entries,
                );
                switch (disposition) {
                    .remove => {
                        var removed = self.attachments.fetchSwapRemove(pending.attachment_id).?;
                        removed.value.deinit(self.allocator);
                    },
                    .retain => {
                        std.debug.assert(attachment.revision < std.math.maxInt(u64));
                        attachment.revision += 1;
                        attachment.repairing = false;
                    },
                }
            },
            .input => |mutation| {
                switch (mutation) {
                    .focus => |focus| if (attachment.focused != focus.focused) {
                        if (focus.focused) {
                            self.focused_count += 1;
                        } else {
                            std.debug.assert(self.focused_count > 0);
                            self.focused_count -= 1;
                        }
                        attachment.focused = focus.focused;
                    },
                    .key_release => |key| {
                        if (attachment.keyIndex(key)) |index| {
                            var removed = attachment.keys.orderedRemove(index);
                            removed.deinit(self.allocator);
                        }
                    },
                    .key_retain => |key| {
                        if (attachment.keyIndex(key.key)) |index| {
                            attachment.keys.items[index].deinit(self.allocator);
                            attachment.keys.items[index] = key;
                        } else {
                            attachment.keys.appendAssumeCapacity(key);
                        }
                    },
                    .mouse => |mouse| attachment.commitMouseOwnership(mouse),
                }
                self.updateRepairCharge(
                    previous_repair_bytes,
                    previous_repair_entries,
                    attachment.repairBytes() + self.focusRepairBytes(),
                    attachment.repairEntries() + self.focusRepairEntries(),
                );
            },
        }
        prepared.* = null;
    }

    pub fn cancelPrepared(self: *Ledger, pending: *PreparedMutation) void {
        if (self.attachments.getPtr(pending.attachment_id)) |attachment| {
            if (pending.key_slot_reserved) {
                std.debug.assert(attachment.pending_key_slots > 0);
                attachment.pending_key_slots -= 1;
            }
            switch (pending.mutation) {
                .repair => if (attachment.revision == pending.attachment_revision) {
                    attachment.repairing = false;
                },
                .input => {},
            }
        }
        pending.deinit(self.allocator);
    }

    pub fn discard(self: *Ledger, attachment_id: AttachmentId) void {
        if (self.attachments.fetchSwapRemove(attachment_id)) |removed_value| {
            var removed = removed_value.value;
            const previous_repair_bytes = removed.repairBytes() + self.focusRepairBytes();
            const previous_repair_entries = removed.repairEntries() + self.focusRepairEntries();
            if (removed.focused) {
                std.debug.assert(self.focused_count > 0);
                self.focused_count -= 1;
            }
            self.updateRepairCharge(
                previous_repair_bytes,
                previous_repair_entries,
                self.focusRepairBytes(),
                self.focusRepairEntries(),
            );
            removed.deinit(self.allocator);
        }
    }

    pub fn prepareRepair(
        self: *Ledger,
        allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
        disposition: RepairDisposition,
        terminal: *ghostty_vt.Terminal,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !PreparedRepair {
        const attachment = self.attachments.getPtr(attachment_id) orelse
            return error.MissingAttachment;
        if (attachment.repairing) return error.MissingAttachment;
        if (disposition == .retain and attachment.revision == std.math.maxInt(u64)) {
            return error.InputSequenceExhausted;
        }
        attachment.repairing = true;
        errdefer attachment.repairing = false;

        var encoded: std.ArrayList(u8) = .empty;
        defer encoded.deinit(allocator);
        var key_buffer: [max_key_encoded_bytes]u8 = undefined;
        for (attachment.keys.items) |key| {
            const release = try input_encoding.encodeKey(
                &key_buffer,
                terminal,
                key.key,
                key.release(),
            );
            try appendRepairBytes(&encoded, allocator, release);
        }

        var pressed_buttons = attachment.mouseButtons();
        if (attachment.mouse_sample) |sample| {
            var mouse_buffer: [max_mouse_encoded_bytes]u8 = undefined;
            for (attachment.mouse_order[0..attachment.mouse_order_len]) |button| {
                attachment_ownership.setMouseButton(&pressed_buttons, button, false);
                var release = sample;
                release.action = .{ .release = button };
                release.pressed_buttons = pressed_buttons;
                const bytes = try input_encoding.encodeMouse(
                    &mouse_buffer,
                    terminal,
                    cell_pixel_width,
                    cell_pixel_height,
                    release,
                );
                try appendRepairBytes(&encoded, allocator, bytes);
            }
        }

        if (attachment.focused and self.focused_count == 1 and
            input_encoding.reportsFocus(terminal))
        {
            var focus_buffer: [max_focus_encoded_bytes]u8 = undefined;
            const focus_out = try input_encoding.encodeFocus(&focus_buffer, false);
            try appendRepairBytes(&encoded, allocator, focus_out);
        }

        const bytes = if (encoded.items.len == 0)
            null
        else
            try encoded.toOwnedSlice(allocator);
        return .{
            .bytes = bytes,
            .ownership = .{
                .attachment_id = attachment_id,
                .attachment_revision = attachment.revision,
                .mutation = .{ .repair = disposition },
            },
        };
    }

    pub fn validateRepairReservation(
        self: *const Ledger,
        attachment_id: AttachmentId,
        byte_count: usize,
    ) !void {
        const attachment = self.attachments.get(attachment_id) orelse
            return error.MissingAttachment;
        if (!attachment.repairing) return error.MissingAttachment;
        const focus_bytes: usize = if (attachment.focused and self.focused_count == 1)
            max_focus_encoded_bytes
        else
            0;
        if (byte_count > attachment.repairBytes() + focus_bytes) {
            return error.InputLimitExceeded;
        }
    }

    pub fn resetRetainingAttachment(self: *Ledger, attachment_id: AttachmentId) void {
        const attachment = self.attachments.getPtr(attachment_id) orelse return;
        const previous_repair_bytes = attachment.repairBytes() + self.focusRepairBytes();
        const previous_repair_entries = attachment.repairEntries() + self.focusRepairEntries();
        if (attachment.focused) {
            std.debug.assert(self.focused_count > 0);
            self.focused_count -= 1;
            attachment.focused = false;
        }
        attachment.resetHeldState(self.allocator);
        attachment.repairing = false;
        if (attachment.revision < std.math.maxInt(u64)) attachment.revision += 1;
        self.updateRepairCharge(
            previous_repair_bytes,
            previous_repair_entries,
            self.focusRepairBytes(),
            self.focusRepairEntries(),
        );
    }

    fn focusRepairBytes(self: *const Ledger) usize {
        return if (self.focused_count == 0) 0 else max_focus_encoded_bytes;
    }

    fn focusRepairEntries(self: *const Ledger) usize {
        return @intFromBool(self.focused_count != 0);
    }

    fn updateRepairCharge(
        self: *Ledger,
        previous_bytes: usize,
        previous_entries: usize,
        next_bytes: usize,
        next_entries: usize,
    ) void {
        std.debug.assert(self.repair_charge.bytes >= previous_bytes);
        std.debug.assert(self.repair_charge.entries >= previous_entries);
        self.repair_charge.bytes = self.repair_charge.bytes - previous_bytes + next_bytes;
        self.repair_charge.entries = self.repair_charge.entries - previous_entries + next_entries;
    }
};

fn appendRepairBytes(
    encoded: *std.ArrayList(u8),
    allocator: std.mem.Allocator,
    bytes: []const u8,
) !void {
    if (bytes.len > input_queue.client_capacity_bytes -| encoded.items.len) {
        return error.InputLimitExceeded;
    }
    try encoded.appendSlice(allocator, bytes);
}
