const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const terminal_input = @import("terminal-input");
const input_encoding = @import("input_encoding.zig");
const input_queue = @import("input_queue.zig");

const max_key_encoded_bytes = input_encoding.max_key_bytes;
const max_mouse_encoded_bytes = input_encoding.max_mouse_bytes;
const mouse_button_count = @typeInfo(terminal_input.MouseButton).@"enum".fields.len;

pub const AttachmentId = identifiers.AttachmentId;

const OwnedKeyInput = struct {
    composing: bool,
    consumed_modifiers: terminal_input.Modifiers,
    key: ghostty_vt.input.Key,
    modifiers: terminal_input.Modifiers,
    text: []u8,
    unshifted_codepoint: u21,

    fn deinit(self: *OwnedKeyInput, allocator: std.mem.Allocator) void {
        allocator.free(self.text);
        self.* = undefined;
    }

    fn release(self: OwnedKeyInput) terminal_input.KeyInput {
        return .{
            .action = .release,
            .composing = self.composing,
            .consumed_modifiers = self.consumed_modifiers,
            .key = self.key.w3c(),
            .modifiers = self.modifiers,
            .text = self.text,
            .unshifted_codepoint = self.unshifted_codepoint,
        };
    }
};

const AttachmentState = struct {
    detaching: bool = false,
    keys: std.ArrayList(OwnedKeyInput) = .empty,
    mouse_order: [mouse_button_count]terminal_input.MouseButton = undefined,
    mouse_order_len: usize = 0,
    mouse_sample: ?terminal_input.MouseInput = null,
    pending_key_slots: usize = 0,

    fn deinit(self: *AttachmentState, allocator: std.mem.Allocator) void {
        for (self.keys.items) |*key| key.deinit(allocator);
        self.keys.deinit(allocator);
        self.* = undefined;
    }

    fn keyIndex(self: *const AttachmentState, key: ghostty_vt.input.Key) ?usize {
        for (self.keys.items, 0..) |owned, index| {
            if (owned.key == key) return index;
        }
        return null;
    }

    fn mouseIndex(self: *const AttachmentState, button: terminal_input.MouseButton) ?usize {
        for (self.mouse_order[0..self.mouse_order_len], 0..) |owned, index| {
            if (owned == button) return index;
        }
        return null;
    }

    fn mouseButtons(self: *const AttachmentState) terminal_input.MouseButtons {
        var buttons: terminal_input.MouseButtons = .{};
        for (self.mouse_order[0..self.mouse_order_len]) |button| {
            setMouseButton(&buttons, button, true);
        }
        return buttons;
    }

    fn repairBytes(self: *const AttachmentState) usize {
        return self.keys.items.len * max_key_encoded_bytes +
            self.mouse_order_len * max_mouse_encoded_bytes;
    }

    fn repairEntries(self: *const AttachmentState) usize {
        return @intFromBool(self.keys.items.len != 0 or self.mouse_order_len != 0);
    }
};

pub const Mutation = union(enum) {
    key_release: ghostty_vt.input.Key,
    key_retain: OwnedKeyInput,
    mouse: terminal_input.MouseInput,

    pub fn deinit(self: *Mutation, allocator: std.mem.Allocator) void {
        switch (self.*) {
            .key_retain => |*key| key.deinit(allocator),
            .key_release, .mouse => {},
        }
        self.* = undefined;
    }
};

pub const PreparedMutation = struct {
    attachment_id: AttachmentId,
    key_slot_reserved: bool = false,
    mutation: union(enum) {
        detach,
        input: Mutation,
    },

    fn deinit(self: *PreparedMutation, allocator: std.mem.Allocator) void {
        switch (self.mutation) {
            .input => |*mutation| mutation.deinit(allocator),
            .detach => {},
        }
        self.* = undefined;
    }
};

pub const PreparedRepair = struct {
    bytes: ?[]u8,
    ownership: PreparedMutation,
};

/// Owns only the held key and mouse facts needed to repair one attachment's
/// accepted input on detach. Capability and terminal binding remain Core facts.
/// TerminalRuntime serializes every operation with its input mutex.
pub const Ledger = struct {
    allocator: std.mem.Allocator,
    attachments: std.AutoArrayHashMap(AttachmentId, AttachmentState),
    repair_charge: input_queue.Charge = .{},

    pub fn init(allocator: std.mem.Allocator) Ledger {
        return .{
            .allocator = allocator,
            .attachments = .init(allocator),
        };
    }

    pub fn deinit(self: *Ledger) void {
        for (self.attachments.values()) |*attachment| attachment.deinit(self.allocator);
        self.attachments.deinit();
        self.* = undefined;
    }

    pub fn ensureAttachmentCapacity(self: *Ledger) !void {
        try self.attachments.ensureUnusedCapacity(1);
    }

    pub fn commitAttachment(self: *Ledger, attachment_id: AttachmentId) void {
        self.attachments.putAssumeCapacityNoClobber(attachment_id, .{});
    }

    pub fn contains(self: *const Ledger, attachment_id: AttachmentId) bool {
        return self.attachments.contains(attachment_id);
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
        if (attachment.detaching) return error.MissingAttachment;
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
            .key_release, .mouse => {},
        }
        const owned = mutation.*;
        mutation.* = undefined;
        return .{
            .attachment_id = attachment_id,
            .key_slot_reserved = key_slot_reserved,
            .mutation = .{ .input = owned },
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
        switch (pending.mutation) {
            .detach => if (!current.detaching) return error.MissingAttachment,
            .input => if (current.detaching) return error.MissingAttachment,
        }

        const additional_repair_bytes: usize = switch (pending.mutation) {
            .input => |mutation| switch (mutation) {
                .key_retain => |key| if (current.keyIndex(key.key) == null)
                    max_key_encoded_bytes
                else
                    0,
                .mouse => |mouse| switch (mouse.action) {
                    .press => |button| if (mouse.pressed_buttons.contains(button) and
                        current.mouseIndex(button) == null)
                        max_mouse_encoded_bytes
                    else
                        0,
                    .release, .motion => 0,
                },
                .key_release => 0,
            },
            .detach => 0,
        };
        const additional_repair_entries = @intFromBool(
            additional_repair_bytes != 0 and current.repairEntries() == 0,
        );
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
        const previous_repair_bytes = attachment.repairBytes();
        const previous_repair_entries = attachment.repairEntries();
        switch (pending.mutation) {
            .detach => {
                self.updateRepairCharge(previous_repair_bytes, previous_repair_entries, 0, 0);
                var removed = self.attachments.fetchSwapRemove(pending.attachment_id).?;
                removed.value.deinit(self.allocator);
            },
            .input => |mutation| {
                switch (mutation) {
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
                    .mouse => |mouse| commitMouseOwnership(attachment, mouse),
                }
                self.updateRepairCharge(
                    previous_repair_bytes,
                    previous_repair_entries,
                    attachment.repairBytes(),
                    attachment.repairEntries(),
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
                .detach => attachment.detaching = false,
                .input => {},
            }
        }
        pending.deinit(self.allocator);
    }

    pub fn discard(self: *Ledger, attachment_id: AttachmentId) void {
        if (self.attachments.fetchSwapRemove(attachment_id)) |removed_value| {
            var removed = removed_value.value;
            self.updateRepairCharge(removed.repairBytes(), removed.repairEntries(), 0, 0);
            removed.deinit(self.allocator);
        }
    }

    pub fn prepareRepair(
        self: *Ledger,
        allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
        terminal: *ghostty_vt.Terminal,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !PreparedRepair {
        const attachment = self.attachments.getPtr(attachment_id) orelse
            return error.MissingAttachment;
        if (attachment.detaching) return error.MissingAttachment;
        attachment.detaching = true;
        errdefer attachment.detaching = false;

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
                setMouseButton(&pressed_buttons, button, false);
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

        const bytes = if (encoded.items.len == 0)
            null
        else
            try encoded.toOwnedSlice(allocator);
        return .{
            .bytes = bytes,
            .ownership = .{
                .attachment_id = attachment_id,
                .mutation = .detach,
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
        if (!attachment.detaching) return error.MissingAttachment;
        if (byte_count > attachment.repairBytes()) return error.InputLimitExceeded;
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

pub fn mutationForInput(
    allocator: std.mem.Allocator,
    input: terminal_input.SemanticInput,
) !?Mutation {
    return switch (input) {
        .key => |value| key: {
            const key = try input_encoding.parseKey(value.key);
            if (value.action == .release) break :key .{ .key_release = key };
            const text = try allocator.dupe(u8, value.text);
            break :key .{ .key_retain = .{
                .composing = value.composing,
                .consumed_modifiers = value.consumed_modifiers,
                .key = key,
                .modifiers = value.modifiers,
                .text = text,
                .unshifted_codepoint = value.unshifted_codepoint,
            } };
        },
        .mouse => |value| .{ .mouse = value },
        .focus, .paste, .text => null,
    };
}

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

fn commitMouseOwnership(attachment: *AttachmentState, input: terminal_input.MouseInput) void {
    switch (input.action) {
        .press => |button| {
            if (!input.pressed_buttons.contains(button)) return;
            if (attachment.mouseIndex(button) == null) {
                std.debug.assert(attachment.mouse_order_len < attachment.mouse_order.len);
                attachment.mouse_order[attachment.mouse_order_len] = button;
                attachment.mouse_order_len += 1;
            }
        },
        .release => |button| {
            if (attachment.mouseIndex(button)) |index| {
                var current = index;
                while (current + 1 < attachment.mouse_order_len) : (current += 1) {
                    attachment.mouse_order[current] = attachment.mouse_order[current + 1];
                }
                attachment.mouse_order_len -= 1;
            }
        },
        .motion => {},
    }
    attachment.mouse_sample = if (attachment.mouse_order_len == 0) null else input;
}

fn setMouseButton(
    buttons: *terminal_input.MouseButtons,
    button: terminal_input.MouseButton,
    pressed: bool,
) void {
    switch (button) {
        .left => buttons.left = pressed,
        .right => buttons.right = pressed,
        .middle => buttons.middle = pressed,
        .four => buttons.four = pressed,
        .five => buttons.five = pressed,
        .six => buttons.six = pressed,
        .seven => buttons.seven = pressed,
        .eight => buttons.eight = pressed,
        .nine => buttons.nine = pressed,
        .ten => buttons.ten = pressed,
        .eleven => buttons.eleven = pressed,
    }
}
