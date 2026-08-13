const core_module = @import("terminal-host-core").state;
const host_input_admission = @import("input_admission.zig");
const host_response = @import("response.zig");
const protocol = @import("terminal-protocol");
const terminal_history = @import("terminal-host-engine").history;

const MessageKind = protocol.MessageKind;

pub const Details = host_response.FailureDetails;
pub const CoreFailure = core_module.CoreError || error{OutOfMemory};
pub const InteractionFailure = terminal_history.HistoryError || error{
    OutOfMemory,
    TerminalResizeFailed,
    TerminalRuntimeUnavailable,
};

pub const runtime_state_diverged: Details = .{
    .code = "internal-error",
    .message = "Host and terminal runtime state diverged",
    .retryable = false,
};

pub const InputFailure = struct {
    code: []const u8,
    retryable: bool,
};

pub fn semanticInputMessage(kind: u16) []const u8 {
    return switch (kind) {
        MessageKind.focus_input => "FocusInput payload is invalid",
        MessageKind.key_input => "KeyInput payload is invalid",
        MessageKind.mouse_input => "MouseInput payload is invalid",
        MessageKind.paste_input => "PasteInput payload is invalid",
        MessageKind.text_input => "TextInput payload is invalid",
        else => "semantic input payload is invalid",
    };
}

pub fn fromInput(rejection: host_input_admission.Rejection) InputFailure {
    return switch (rejection) {
        .backpressure => .{ .code = "input-backpressure", .retryable = true },
        .resource_exhausted => .{ .code = "resource-exhausted", .retryable = true },
        .sequence_exhausted, .encoding_failed => .{ .code = "resource-exhausted", .retryable = false },
        .terminal_exited, .terminal_runtime_unavailable => .{ .code = "invalid-phase", .retryable = false },
        .input_limit_exceeded => .{ .code = "limit-exceeded", .retryable = false },
        .invalid_input => .{ .code = "invalid-field", .retryable = false },
        .paste_mode_changed => .{ .code = "input-mode-changed", .retryable = false },
    };
}

pub fn protocolCode(err: protocol.ProtocolError) []const u8 {
    return switch (err) {
        error.InvalidField => "invalid-field",
        error.InvalidIdentifier => "invalid-identifier",
        error.InvalidMagic => "invalid-magic",
        error.InvalidUtf8 => "invalid-utf8",
        error.LengthMismatch => "length-mismatch",
        error.LimitExceeded => "limit-exceeded",
        error.Truncated => "truncated",
        error.UnknownRequiredFlag => "unknown-required-flag",
        error.UnknownRequiredMessage => "unknown-required-message",
        error.UnsupportedVersion => "unsupported-version",
    };
}

pub fn fromCore(err: CoreFailure) Details {
    return switch (err) {
        error.AttachmentConflict => .{ .code = "attachment-conflict", .message = "AttachmentId conflicts with retained state", .retryable = false },
        error.AttachmentSequenceOverflow => .{ .code = "resource-exhausted", .message = "attachment admission sequence space is exhausted", .retryable = false },
        error.ConnectionConflict => .{ .code = "broker-active", .message = "another broker session is already active", .retryable = true },
        error.CreationConflict => .{ .code = "creation-conflict", .message = "CreationId has a different launch", .retryable = false },
        error.FailureContainmentIncomplete => .{ .code = "cleanup-incomplete", .message = "terminal runtime cleanup is incomplete", .retryable = true },
        error.InvalidCapabilities => .{ .code = "invalid-field", .message = "attachment must request at least one capability", .retryable = false },
        error.InvalidConfiguration => .{ .code = "invalid-field", .message = "terminal configuration is invalid", .retryable = false },
        error.InvalidFrameAcknowledgement => .{ .code = "invalid-frame-acknowledgement", .message = "frame acknowledgement does not match the in-flight projection", .retryable = false },
        error.InvalidPhase => .{ .code = "invalid-phase", .message = "operation is invalid in the terminal phase", .retryable = false },
        error.InvalidProcessObservation => .{ .code = "internal-error", .message = "Host observed an invalid process result", .retryable = false },
        error.InventoryGenerationOverflow => .{ .code = "resource-exhausted", .message = "terminal inventory generation space is exhausted", .retryable = false },
        error.MissingConnection => .{ .code = "missing-connection", .message = "client connection is not active", .retryable = false },
        error.MissingResource => .{ .code = "missing-resource", .message = "terminal or attachment is missing", .retryable = false },
        error.OutcomePublicationPending => .{ .code = "outcome-publication-pending", .message = "terminal outcome publication is pending", .retryable = true },
        error.RevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal revision space is exhausted", .retryable = false },
        error.ResizeLeaseDenied => .{ .code = "resize-lease-denied", .message = "attachment does not own the resize lease", .retryable = false },
        error.SequenceOverflow => .{ .code = "resource-exhausted", .message = "attachment frame sequence space is exhausted", .retryable = false },
        error.TerminalConflict => .{ .code = "terminal-conflict", .message = "TerminalId conflicts with retained state", .retryable = true },
        error.OutOfMemory => .{ .code = "resource-exhausted", .message = "Host could not retain bounded state", .retryable = true },
    };
}

pub fn fromCleanup(failure: core_module.RuntimeCleanupFailure) Details {
    return switch (failure) {
        .contain_process => .{
            .code = "cleanup-contain-process",
            .message = "terminal process-tree containment did not complete",
            .retryable = true,
        },
        .release_output => .{
            .code = "cleanup-release-output",
            .message = "terminal PTY output release did not complete",
            .retryable = true,
        },
        .release_process_identity => .{
            .code = "cleanup-release-process-identity",
            .message = "terminal process-identity release did not complete",
            .retryable = true,
        },
    };
}

pub fn fromInteraction(err: InteractionFailure, message: []const u8) Details {
    const mapped: struct { code: []const u8, retryable: bool } = switch (err) {
        error.AnchorEvicted => .{ .code = "history-evicted", .retryable = true },
        error.AttachmentConflict => .{ .code = "interaction-failed", .retryable = false },
        error.ExportConflict => .{ .code = "export-in-progress", .retryable = true },
        error.ExportTooLarge, error.ProjectionTooLarge => .{ .code = "limit-exceeded", .retryable = false },
        error.InvalidAcknowledgement => .{ .code = "invalid-acknowledgement", .retryable = false },
        error.InvalidPoint, error.InvalidRequest => .{ .code = "invalid-field", .retryable = false },
        error.MissingAttachment, error.MissingSelection => .{ .code = "missing-resource", .retryable = false },
        error.StaleGeneration => .{ .code = "stale-generation", .retryable = true },
        error.StaleRevision => .{ .code = "stale-revision", .retryable = true },
        error.OutOfMemory => .{ .code = "resource-exhausted", .retryable = true },
        error.TerminalResizeFailed, error.TerminalRuntimeUnavailable => .{ .code = "invalid-phase", .retryable = false },
    };
    return .{
        .code = mapped.code,
        .message = message,
        .retryable = mapped.retryable,
    };
}
