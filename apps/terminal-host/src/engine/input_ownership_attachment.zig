const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const terminal_input = @import("terminal-input");
const input_encoding = @import("input_encoding.zig");

const max_key_encoded_bytes = input_encoding.max_key_bytes;
const max_mouse_encoded_bytes = input_encoding.max_mouse_bytes;
const mouse_button_count = @typeInfo(terminal_input.MouseButton).@"enum".fields.len;

pub const OwnedKeyInput = struct {
    composing: bool,
    consumed_modifiers: terminal_input.Modifiers,
    key: ghostty_vt.input.Key,
    modifiers: terminal_input.Modifiers,
    text: []u8,
    unshifted_codepoint: u21,

    pub fn deinit(self: *OwnedKeyInput, allocator: std.mem.Allocator) void {
        allocator.free(self.text);
        self.* = undefined;
    }

    pub fn release(self: OwnedKeyInput) terminal_input.KeyInput {
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

/// Owns the deterministic held-input state for one attachment. The parent
/// ledger coordinates aggregate focus and queue repair charge across these
/// independent states.
pub const AttachmentState = struct {
    focused: bool = false,
    keys: std.ArrayList(OwnedKeyInput) = .empty,
    mouse_order: [mouse_button_count]terminal_input.MouseButton = undefined,
    mouse_order_len: usize = 0,
    mouse_sample: ?terminal_input.MouseInput = null,
    pending_key_slots: usize = 0,
    repairing: bool = false,
    revision: u64 = 1,

    pub fn deinit(self: *AttachmentState, allocator: std.mem.Allocator) void {
        for (self.keys.items) |*key| key.deinit(allocator);
        self.keys.deinit(allocator);
        self.* = undefined;
    }

    pub fn keyIndex(self: *const AttachmentState, key: ghostty_vt.input.Key) ?usize {
        for (self.keys.items, 0..) |owned, index| {
            if (owned.key == key) return index;
        }
        return null;
    }

    pub fn mouseIndex(self: *const AttachmentState, button: terminal_input.MouseButton) ?usize {
        for (self.mouse_order[0..self.mouse_order_len], 0..) |owned, index| {
            if (owned == button) return index;
        }
        return null;
    }

    pub fn mouseButtons(self: *const AttachmentState) terminal_input.MouseButtons {
        var buttons: terminal_input.MouseButtons = .{};
        for (self.mouse_order[0..self.mouse_order_len]) |button| {
            setMouseButton(&buttons, button, true);
        }
        return buttons;
    }

    pub fn repairBytes(self: *const AttachmentState) usize {
        return self.keys.items.len * max_key_encoded_bytes +
            self.mouse_order_len * max_mouse_encoded_bytes;
    }

    pub fn repairEntries(self: *const AttachmentState) usize {
        return @intFromBool(self.keys.items.len != 0 or self.mouse_order_len != 0);
    }

    pub fn resetHeldState(self: *AttachmentState, allocator: std.mem.Allocator) void {
        for (self.keys.items) |*key| key.deinit(allocator);
        self.keys.clearRetainingCapacity();
        self.mouse_order_len = 0;
        self.mouse_sample = null;
    }

    pub fn commitMouseOwnership(self: *AttachmentState, input: terminal_input.MouseInput) void {
        switch (input.action) {
            .press => |button| {
                if (!input.pressed_buttons.contains(button)) return;
                if (self.mouseIndex(button) == null) {
                    std.debug.assert(self.mouse_order_len < self.mouse_order.len);
                    self.mouse_order[self.mouse_order_len] = button;
                    self.mouse_order_len += 1;
                }
            },
            .release => |button| {
                if (self.mouseIndex(button)) |index| {
                    var current = index;
                    while (current + 1 < self.mouse_order_len) : (current += 1) {
                        self.mouse_order[current] = self.mouse_order[current + 1];
                    }
                    self.mouse_order_len -= 1;
                }
            },
            .motion => {},
        }
        self.mouse_sample = if (self.mouse_order_len == 0) null else input;
    }
};

pub const FocusMutation = struct {
    expected_attachment_focused: bool,
    expected_focused_count: usize,
    focused: bool,
};

pub const Mutation = union(enum) {
    focus: FocusMutation,
    key_release: ghostty_vt.input.Key,
    key_retain: OwnedKeyInput,
    mouse: terminal_input.MouseInput,

    pub fn deinit(self: *Mutation, allocator: std.mem.Allocator) void {
        switch (self.*) {
            .key_retain => |*key| key.deinit(allocator),
            .focus, .key_release, .mouse => {},
        }
        self.* = undefined;
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

pub fn setMouseButton(
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
