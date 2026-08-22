const identifiers = @import("terminal-host-model").identifiers;
const terminal_input = @import("terminal-input");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const attachment_input_admission_ops = @import("attachment_input_admission.zig");
const input_admission_ops = @import("input_admission.zig");
const prepared_input = @import("prepared_input.zig");

pub fn Type(comptime Runtime: type, comptime PreparedInput: type) type {
    return struct {
        pub fn enqueueInput(self: *Runtime, bytes: []const u8) !void {
            return input_admission_ops.enqueueInput(stateFrom(self), bytes);
        }

        pub fn enqueueRawInput(self: *Runtime, bytes: []const u8) !usize {
            return input_admission_ops.enqueueRawInput(stateFrom(self), bytes);
        }

        pub fn prepareRawInput(self: *Runtime, bytes: []const u8) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareRawInput(stateFrom(self), bytes));
        }

        pub fn enqueueKeyInput(self: *Runtime, input: contracts.KeyInput) !usize {
            return input_admission_ops.enqueueKeyInput(stateFrom(self), input);
        }

        pub fn keyInputWouldEmit(self: *Runtime, input: contracts.KeyInput) !bool {
            return input_admission_ops.keyInputWouldEmit(stateFrom(self), input);
        }

        pub fn enqueueKeyInputWithLimit(
            self: *Runtime,
            input: contracts.KeyInput,
            max_output_bytes: usize,
        ) !usize {
            return input_admission_ops.enqueueKeyInputWithLimit(stateFrom(self), input, max_output_bytes);
        }

        pub fn prepareKeyInput(
            self: *Runtime,
            input: contracts.KeyInput,
            max_output_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareKeyInput(stateFrom(self), input, max_output_bytes));
        }

        pub fn enqueueTextInput(self: *Runtime, text: []const u8) !usize {
            return input_admission_ops.enqueueTextInput(stateFrom(self), text);
        }

        pub fn enqueueTextInputWithLimit(
            self: *Runtime,
            text: []const u8,
            max_output_bytes: usize,
        ) !usize {
            return input_admission_ops.enqueueTextInputWithLimit(stateFrom(self), text, max_output_bytes);
        }

        pub fn prepareTextInput(
            self: *Runtime,
            text: []const u8,
            max_output_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareTextInput(stateFrom(self), text, max_output_bytes));
        }

        pub fn enqueueMouseInput(self: *Runtime, input: contracts.MouseInput) !usize {
            return input_admission_ops.enqueueMouseInput(stateFrom(self), input);
        }

        pub fn mouseInputWouldEmit(self: *Runtime, input: contracts.MouseInput) !bool {
            return input_admission_ops.mouseInputWouldEmit(stateFrom(self), input);
        }

        pub fn enqueueMouseInputWithLimit(
            self: *Runtime,
            input: contracts.MouseInput,
            max_output_bytes: usize,
        ) !usize {
            return input_admission_ops.enqueueMouseInputWithLimit(stateFrom(self), input, max_output_bytes);
        }

        pub fn prepareMouseInput(
            self: *Runtime,
            input: contracts.MouseInput,
            max_output_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareMouseInput(stateFrom(self), input, max_output_bytes));
        }

        pub fn enqueueFocusInput(self: *Runtime, focused: bool) !usize {
            return input_admission_ops.enqueueFocusInput(stateFrom(self), focused);
        }

        pub fn focusInputWouldEmit(self: *Runtime) bool {
            return input_admission_ops.focusInputWouldEmit(stateFrom(self));
        }

        pub fn enqueueFocusInputWithLimit(
            self: *Runtime,
            focused: bool,
            max_output_bytes: usize,
        ) !usize {
            return input_admission_ops.enqueueFocusInputWithLimit(stateFrom(self), focused, max_output_bytes);
        }

        pub fn prepareFocusInput(
            self: *Runtime,
            focused: bool,
            max_output_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareFocusInput(stateFrom(self), focused, max_output_bytes));
        }

        pub fn enqueuePasteInput(
            self: *Runtime,
            text: []const u8,
            bracketed: bool,
        ) !usize {
            return input_admission_ops.enqueuePasteInput(stateFrom(self), text, bracketed);
        }

        pub fn enqueuePasteInputWithLimit(
            self: *Runtime,
            text: []const u8,
            bracketed: bool,
            max_output_bytes: usize,
        ) !usize {
            return input_admission_ops.enqueuePasteInputWithLimit(stateFrom(self), text, bracketed, max_output_bytes);
        }

        pub fn preparePasteInput(
            self: *Runtime,
            text: []const u8,
            bracketed: bool,
            max_output_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.preparePasteInput(stateFrom(self), text, bracketed, max_output_bytes));
        }

        pub fn prepareSemanticInput(
            self: *Runtime,
            input: terminal_input.SemanticInput,
            maximum_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try input_admission_ops.prepareSemanticInput(stateFrom(self), input, maximum_bytes));
        }

        pub fn prepareAttachmentSemanticInput(
            self: *Runtime,
            attachment_id: contracts.AttachmentId,
            input: terminal_input.SemanticInput,
            maximum_bytes: usize,
        ) !PreparedInput {
            return exposePreparedInput(try attachment_input_admission_ops.prepareSemanticInput(stateFrom(self), attachment_id, input, maximum_bytes));
        }

        pub fn prepareAttachmentInputRepair(
            self: *Runtime,
            attachment_id: contracts.AttachmentId,
        ) !PreparedInput {
            return exposePreparedInput(try attachment_input_admission_ops.prepareRepair(stateFrom(self), attachment_id));
        }

        pub fn prepareAttachmentInputReset(
            self: *Runtime,
            attachment_id: contracts.AttachmentId,
        ) !PreparedInput {
            return exposePreparedInput(try attachment_input_admission_ops.prepareReset(stateFrom(self), attachment_id));
        }

        pub fn pendingInputBytes(self: *Runtime) usize {
            return input_admission_ops.pendingInputBytes(stateFrom(self));
        }

        pub fn takeWrittenBytes(self: *Runtime) usize {
            return input_admission_ops.takeWrittenBytes(stateFrom(self));
        }

        pub fn takeOutputBatches(self: *Runtime) u64 {
            return input_admission_ops.takeOutputBatches(stateFrom(self));
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }

        fn exposePreparedInput(internal: prepared_input.Internal) PreparedInput {
            return .{
                .affects_close_activity = internal.affects_close_activity,
                .bytes = internal.bytes,
                .ownership = internal.ownership,
                .reservation = internal.reservation,
                .runtime = @ptrCast(internal.runtime),
            };
        }
    };
}
