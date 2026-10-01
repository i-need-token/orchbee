// Keep these synchronized with TypeScript. Released changes (>= 1.0.0) also update
// packages/terminal-protocol/docs/version-history.md in the same change.
pub const terminal_frame_header_bytes = 160;
pub const terminal_frame_magic = "OBTF";
pub const terminal_frame_format_major: u8 = 0;
pub const terminal_frame_format_minor: u8 = 0;
pub const terminal_frame_format_patch: u8 = 3;
pub const terminal_frame_extension_close_activity: u16 = 1;
pub const terminal_frame_extension_kitty_graphics: u16 = 2;
pub const terminal_frame_extension_progress: u16 = 3;
pub const terminal_frame_extension_command_completion: u16 = 4;
pub const terminal_graphics_header_bytes = 40;
pub const terminal_graphics_magic = "OBKG";
pub const terminal_graphics_format_major: u8 = 0;
pub const terminal_graphics_format_minor: u8 = 0;
pub const terminal_graphics_format_patch: u8 = 2;

const frame_extension = @import("frame_extension.zig");
pub const TerminalCloseActivity = frame_extension.TerminalCloseActivity;
pub const TerminalProgressState = frame_extension.TerminalProgressState;
pub const TerminalProgress = frame_extension.TerminalProgress;
pub const TerminalCommandCompletion = frame_extension.TerminalCommandCompletion;
pub const terminal_projection_buffer_header_bytes = 320;
pub const terminal_projection_buffer_magic = "OBPB";
pub const terminal_projection_buffer_format_major: u8 = 0;
pub const terminal_projection_buffer_format_minor: u8 = 0;
pub const terminal_projection_buffer_format_patch: u8 = 2;
pub const terminal_row_window_header_bytes = 112;
pub const terminal_row_window_magic = "OBRW";
pub const terminal_row_window_format_major: u8 = 0;
pub const terminal_row_window_format_minor: u8 = 0;
pub const terminal_row_window_format_patch: u8 = 3;
pub const transport_envelope_header_bytes = 64;
pub const current_protocol_major: u16 = 0;
pub const current_protocol_minor: u16 = 0;
pub const current_protocol_patch: u16 = 25;

pub const Limits = struct {
    pub const cell_pixel_dimension = 4096;
    pub const clipboard_text_bytes: usize = 16 * 1024;
    pub const control_json_bytes: usize = 64 * 1024;
    pub const extension_bytes: usize = 33 * 1024 * 1024;
    pub const frame_bytes: usize = 36 * 1024 * 1024;
    pub const frame_cells = 256 * 1024;
    pub const frame_columns = 4096;
    pub const frame_extensions = 64;
    pub const frame_rows = 4096;
    pub const frame_strings = 4096;
    pub const frame_styles = 1024;
    pub const terminal_graphics_destination_pixels = 65_535;
    pub const terminal_graphics_dimension = 4096;
    pub const terminal_graphics_extension_bytes: usize = 33 * 1024 * 1024;
    pub const terminal_graphics_images = 256;
    pub const terminal_graphics_placements = 1024;
    pub const terminal_graphics_resource_bytes: usize = 32 * 1024 * 1024;
    pub const input_bytes: usize = 1024 * 1024;
    pub const input_accepted_bytes: usize = input_bytes;
    pub const inventory_page_terminals = 128;
    pub const key_text_bytes = 4096;
    pub const launch_arguments = 4096;
    pub const launch_environment_entries = 1024;
    pub const environment_overlay_entries = 16;
    pub const environment_name_bytes: usize = 32 * 1024;
    pub const environment_overlay_value_bytes: usize = 4 * 1024;
    pub const optional_feature_name_bytes: usize = 64;
    pub const optional_features = 64;
    pub const paste_auto_input_bytes: usize = 256 * 1024;
    pub const pending_input_bytes: usize = 4 * 1024 * 1024;
    pub const pty_read_batch_bytes: usize = 256 * 1024;
    pub const row_window_bytes: usize = 44 * 1024 * 1024;
    pub const row_window_rows = 4096;
    pub const search_query_bytes: usize = 16 * 1024;
    pub const scrollback_bytes: usize = 1_000_000_000;
    pub const selection_export_bytes: usize = 16 * 1024 * 1024;
    pub const selection_export_chunk_bytes: usize = 1024 * 1024;
    pub const selection_preview_bytes: usize = 4 * 1024;
    pub const string_bytes = 16 * 1024;
    pub const terminal_profile_features = 64;
    pub const terminal_application_signal_bytes: usize = 4 * 1024;
    pub const terminal_profile_palette_entries = 256;
    pub const terminal_response_bytes: usize = 256;
    pub const transport_record_bytes: usize = row_window_bytes + transport_envelope_header_bytes;
    pub const raw_payload_bytes: usize = semantic_input_payload_bytes;
    pub const semantic_input_payload_bytes: usize = 32 + input_bytes;
    pub const paste_auto_input_payload_bytes: usize = 32 + paste_auto_input_bytes;
    pub const write_input_payload_bytes: usize = 24 + input_bytes;
};

pub const MessageKind = @import("message_kind.zig").MessageKind;

pub const ProtocolError = error{
    InvalidField,
    InvalidIdentifier,
    InvalidMagic,
    InvalidUtf8,
    LengthMismatch,
    LimitExceeded,
    Truncated,
    UnknownRequiredFlag,
    UnknownRequiredMessage,
    UnsupportedVersion,
};

pub const PayloadEncoding = enum(u8) {
    control_json = 1,
    raw_bytes = 2,
    terminal_frame = 3,
    row_window = 4,
    graphics_image = 5,
};

/// Native dispatch operation owned by the protocol adapter. It is not a wire
/// field; it makes Host request coverage exhaustive when the registry grows.
pub const RequestOperation = enum {
    decode_graphics_png,
    read_process_info,
    apply_terminal_theme,
    attach_terminal,
    claim_resize_lease,
    clear_terminal,
    create_terminal,
    detach_terminal,
    dispose_terminal,
    force_terminate_terminal,
    frame_applied,
    hello,
    history_observed,
    list_terminals,
    need_snapshot,
    ping,
    read_rows,
    resize_terminal,
    reset_terminal,
    rows_applied,
    selection_command,
    selection_export,
    selection_export_cancel,
    selection_export_chunk_applied,
    selection_gesture,
    semantic_input,
    set_history_demand,
    set_projection_demand,
    set_terminal_defaults,
    start_terminal,
    terminate_terminal,
    update_terminal_configuration,
    update_attachment_input_capability,
    write_input,
};

pub const RequestMessageContract = struct {
    operation: RequestOperation,
    payload_encoding: PayloadEncoding,
    success_kind: u16,
};

pub const MessageContract = struct {
    event_encoding: ?PayloadEncoding = null,
    name: []const u8,
    request: ?RequestMessageContract = null,
    response_encoding: ?PayloadEncoding = null,
};

fn requestOnly(
    name: []const u8,
    operation: RequestOperation,
    payload_encoding: PayloadEncoding,
    success_kind: u16,
) MessageContract {
    return .{
        .name = name,
        .request = .{
            .operation = operation,
            .payload_encoding = payload_encoding,
            .success_kind = success_kind,
        },
    };
}

fn requestAndResponse(
    name: []const u8,
    operation: RequestOperation,
    payload_encoding: PayloadEncoding,
    success_kind: u16,
    response_encoding: PayloadEncoding,
) MessageContract {
    var contract = requestOnly(name, operation, payload_encoding, success_kind);
    contract.response_encoding = response_encoding;
    return contract;
}

fn responseOnly(name: []const u8, encoding: PayloadEncoding) MessageContract {
    return .{ .name = name, .response_encoding = encoding };
}

fn eventOnly(name: []const u8, encoding: PayloadEncoding) MessageContract {
    return .{ .event_encoding = encoding, .name = name };
}

fn responseAndEvent(name: []const u8, encoding: PayloadEncoding) MessageContract {
    return .{
        .event_encoding = encoding,
        .name = name,
        .response_encoding = encoding,
    };
}

/// The complete current-version message registry. Request entries own their
/// successful response kind in addition to their payload encoding.
pub fn messageContract(kind: u16) ?MessageContract {
    const M = MessageKind;
    return switch (kind) {
        M.decode_graphics_png => requestOnly("DecodeGraphicsPng", .decode_graphics_png, .graphics_image, M.graphics_png_decoded),
        M.graphics_png_decoded => responseOnly("GraphicsPngDecoded", .graphics_image),
        M.hello => requestOnly("Hello", .hello, .control_json, M.hello_accepted),
        M.hello_accepted => responseOnly("HelloAccepted", .control_json),
        M.protocol_rejected => responseOnly("ProtocolRejected", .control_json),
        M.ping => requestAndResponse("Ping", .ping, .control_json, M.ping, .control_json),
        M.typed_failure => responseOnly("TypedFailure", .control_json),
        M.read_process_info => requestAndResponse("ReadProcessInfo", .read_process_info, .control_json, M.read_process_info, .control_json),
        M.list_terminals => requestAndResponse(
            "ListTerminals",
            .list_terminals,
            .control_json,
            M.list_terminals,
            .control_json,
        ),
        M.create_terminal => requestAndResponse(
            "CreateTerminal",
            .create_terminal,
            .control_json,
            M.create_terminal,
            .control_json,
        ),
        M.terminate_terminal => requestAndResponse(
            "TerminateTerminal",
            .terminate_terminal,
            .control_json,
            M.terminate_terminal,
            .control_json,
        ),
        M.dispose_terminal => requestAndResponse(
            "DisposeTerminal",
            .dispose_terminal,
            .control_json,
            M.dispose_terminal,
            .control_json,
        ),
        M.terminal_started => eventOnly("TerminalStarted", .control_json),
        M.terminal_exited => eventOnly("TerminalExited", .control_json),
        M.start_terminal => requestOnly(
            "StartTerminal",
            .start_terminal,
            .control_json,
            M.terminal_start_accepted,
        ),
        M.terminal_start_accepted => responseOnly("TerminalStartAccepted", .control_json),
        M.force_terminate_terminal => requestAndResponse(
            "ForceTerminateTerminal",
            .force_terminate_terminal,
            .control_json,
            M.force_terminate_terminal,
            .control_json,
        ),
        M.terminal_failed => eventOnly("TerminalFailed", .control_json),
        M.attach_terminal => requestOnly(
            "AttachTerminal",
            .attach_terminal,
            .control_json,
            M.attachment_capabilities,
        ),
        M.detach_terminal => requestAndResponse(
            "DetachTerminal",
            .detach_terminal,
            .control_json,
            M.detach_terminal,
            .control_json,
        ),
        M.attachment_capabilities => responseOnly("AttachmentCapabilities", .control_json),
        M.resize_lease_changed => eventOnly("ResizeLeaseChanged", .control_json),
        M.update_attachment_input_capability => requestOnly(
            "UpdateAttachmentInputCapability",
            .update_attachment_input_capability,
            .control_json,
            M.attachment_capabilities,
        ),
        M.claim_resize_lease => requestOnly(
            "ClaimResizeLease",
            .claim_resize_lease,
            .control_json,
            M.attachment_capabilities,
        ),
        M.write_input => requestOnly("WriteInput", .write_input, .raw_bytes, M.input_accepted),
        M.input_accepted => responseOnly("InputAccepted", .control_json),
        M.resize_terminal => requestOnly(
            "ResizeTerminal",
            .resize_terminal,
            .control_json,
            M.resize_accepted,
        ),
        M.resize_accepted => responseOnly("ResizeAccepted", .control_json),
        M.clear_terminal => requestOnly(
            "ClearTerminal",
            .clear_terminal,
            .control_json,
            M.terminal_control_accepted,
        ),
        M.reset_terminal => requestOnly(
            "ResetTerminal",
            .reset_terminal,
            .control_json,
            M.terminal_control_accepted,
        ),
        M.terminal_control_accepted => responseOnly(
            "TerminalControlAccepted",
            .control_json,
        ),
        M.key_input => requestOnly("KeyInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.text_input => requestOnly("TextInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.mouse_input => requestOnly("MouseInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.focus_input => requestOnly("FocusInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.paste_input => requestOnly("PasteInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.paste_auto_input => requestOnly("PasteAutoInput", .semantic_input, .raw_bytes, M.input_accepted),
        M.snapshot_frame => eventOnly("SnapshotFrame", .terminal_frame),
        M.delta_frame => eventOnly("DeltaFrame", .terminal_frame),
        M.frame_applied => requestAndResponse(
            "FrameApplied",
            .frame_applied,
            .control_json,
            M.frame_applied,
            .control_json,
        ),
        M.need_snapshot => requestAndResponse(
            "NeedSnapshot",
            .need_snapshot,
            .control_json,
            M.need_snapshot,
            .control_json,
        ),
        M.read_rows => requestOnly("ReadRows", .read_rows, .control_json, M.rows_snapshot),
        M.rows_snapshot => responseOnly("RowsSnapshot", .row_window),
        M.set_projection_demand => requestOnly(
            "SetProjectionDemand",
            .set_projection_demand,
            .control_json,
            M.projection_demand_accepted,
        ),
        M.projection_demand_accepted => responseOnly(
            "ProjectionDemandAccepted",
            .control_json,
        ),
        M.projection_blocked => eventOnly("ProjectionBlocked", .control_json),
        M.rows_applied => requestAndResponse(
            "RowsApplied",
            .rows_applied,
            .control_json,
            M.rows_applied,
            .control_json,
        ),
        M.set_history_demand => requestOnly(
            "SetHistoryDemand",
            .set_history_demand,
            .control_json,
            M.history_demand_accepted,
        ),
        M.history_demand_accepted => responseOnly("HistoryDemandAccepted", .control_json),
        M.history_changed => eventOnly("HistoryChanged", .control_json),
        M.history_observed => requestAndResponse(
            "HistoryObserved",
            .history_observed,
            .control_json,
            M.history_observed,
            .control_json,
        ),
        M.terminal_bell => eventOnly("TerminalBell", .control_json),
        M.terminal_clipboard_write_request => eventOnly("TerminalClipboardWriteRequest", .control_json),
        M.terminal_metadata_changed => eventOnly("TerminalMetadataChanged", .control_json),
        M.terminal_application_signal => eventOnly("TerminalApplicationSignal", .control_json),
        M.selection_gesture => requestOnly(
            "SelectionGesture",
            .selection_gesture,
            .control_json,
            M.selection_changed,
        ),
        M.selection_command => requestOnly(
            "SelectionCommand",
            .selection_command,
            .control_json,
            M.selection_changed,
        ),
        M.selection_changed => responseAndEvent("SelectionChanged", .control_json),
        M.selection_export => requestOnly(
            "SelectionExport",
            .selection_export,
            .control_json,
            M.selection_export_started,
        ),
        M.selection_export_started => responseOnly("SelectionExportStarted", .control_json),
        M.selection_export_chunk => eventOnly("SelectionExportChunk", .raw_bytes),
        M.selection_export_chunk_applied => requestAndResponse(
            "SelectionExportChunkApplied",
            .selection_export_chunk_applied,
            .control_json,
            M.selection_export_chunk_applied,
            .control_json,
        ),
        M.selection_export_finished => responseAndEvent(
            "SelectionExportFinished",
            .control_json,
        ),
        M.selection_export_cancel => requestOnly(
            "SelectionExportCancel",
            .selection_export_cancel,
            .control_json,
            M.selection_export_finished,
        ),
        M.set_terminal_defaults => requestOnly(
            "SetTerminalDefaults",
            .set_terminal_defaults,
            .control_json,
            M.terminal_defaults_accepted,
        ),
        M.terminal_defaults_accepted => responseOnly("TerminalDefaultsAccepted", .control_json),
        M.update_terminal_configuration => requestOnly(
            "UpdateTerminalConfiguration",
            .update_terminal_configuration,
            .control_json,
            M.terminal_configuration_accepted,
        ),
        M.terminal_configuration_accepted => responseOnly(
            "TerminalConfigurationAccepted",
            .control_json,
        ),
        M.apply_terminal_theme => requestOnly(
            "ApplyTerminalTheme",
            .apply_terminal_theme,
            .control_json,
            M.terminal_theme_result,
        ),
        M.terminal_theme_result => responseOnly("TerminalThemeResult", .control_json),
        else => null,
    };
}

pub fn messagePayloadEncoding(
    kind: u16,
    response: bool,
    event: bool,
) ?PayloadEncoding {
    const contract = messageContract(kind) orelse return null;
    if (event) return contract.event_encoding;
    if (response) return contract.response_encoding;
    return if (contract.request) |request| request.payload_encoding else null;
}

pub fn isKnownMessageKind(kind: u16) bool {
    return messageContract(kind) != null;
}

pub fn maximumPayloadBytes(encoding: u8) ProtocolError!usize {
    return switch (encoding) {
        @intFromEnum(PayloadEncoding.control_json) => Limits.control_json_bytes,
        @intFromEnum(PayloadEncoding.graphics_image) => 32 * 1024 * 1024 + 56,
        @intFromEnum(PayloadEncoding.raw_bytes) => Limits.raw_payload_bytes,
        @intFromEnum(PayloadEncoding.terminal_frame) => Limits.frame_bytes,
        @intFromEnum(PayloadEncoding.row_window) => Limits.row_window_bytes,
        else => error.InvalidField,
    };
}
