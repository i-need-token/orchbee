const std = @import("std");
const control_boundary = @import("terminal-host-control");
const protocol = @import("terminal-protocol");
const options = @import("conformance-options");

const control_decode = control_boundary.decode;
const control_encode = control_boundary.encode;

const frame_fixture = options.frame_fixture;
const projection_fixture = options.projection_fixture;
const packed_projection_conformance_fixture = options.packed_projection_conformance_fixture;
const envelope_fixture = options.envelope_fixture;
const envelope_fixture_max_bytes = options.envelope_fixture_max_bytes;
const input_fixture = options.input_fixture;
const key_input_fixture = options.key_input_fixture;
const text_input_fixture = options.text_input_fixture;
const mouse_input_fixture = options.mouse_input_fixture;
const focus_input_fixture = options.focus_input_fixture;
const paste_input_fixture = options.paste_input_fixture;
const row_window_fixture = options.row_window_fixture;
const terminal_failed_fixture = options.terminal_failed_fixture;
const control_fixture = options.control_fixture;
const utf8_bom_fixture = options.utf8_bom_fixture;

fn fixturePrefix(bytes: []const u8, length: usize) []const u8 {
    std.debug.assert(bytes.len >= length);
    return bytes[0..length];
}

fn readFixtureU16(bytes: []const u8, offset: usize) u16 {
    std.debug.assert(bytes.len >= offset + @sizeOf(u16));
    return std.mem.readInt(u16, bytes[offset..][0..@sizeOf(u16)], .little);
}

fn readFixtureU32(bytes: []const u8, offset: usize) u32 {
    std.debug.assert(bytes.len >= offset + @sizeOf(u32));
    return std.mem.readInt(u32, bytes[offset..][0..@sizeOf(u32)], .little);
}

const PackedProjectionObservations = struct {
    detail: ?[]const u8 = null,
    ignoredOptionalFlags: ?u8 = null,
    kind: ?[]const u8 = null,
    rowCount: ?u16 = null,
    rowIndices: ?[]const u16 = null,
};

const PackedProjectionExpected = struct {
    code: ?[]const u8 = null,
    observations: ?PackedProjectionObservations = null,
    result: []const u8,
};

const PackedProjectionCase = struct {
    bytesBase64: []const u8,
    codec: []const u8,
    expected: PackedProjectionExpected,
    name: []const u8,
    strictBoundary: []const u8,
};

const PackedProjectionCorpus = struct {
    cases: []const PackedProjectionCase,
    formatVersion: []const u8,
    schemaVersion: u8,
};

const ProjectionSuccessObservation = struct {
    kind: protocol.FrameKind,
    row_count: u16,
};

const RowWindowSuccessObservation = struct {
    detail: protocol.RowWindowDetail,
    row_count: u16,
};

const projection_conformance_case_names = [_][]const u8{
    "obpb-width-out-of-range",
    "obpb-row-flag-unknown",
    "obpb-row-index-out-of-range",
    "obpb-prompt-class-out-of-range",
    "obpb-whole-string-invalid-utf8",
    "obpb-grapheme-length-splits-scalar",
    "obpb-grapheme-offset-null-nonnullable",
    "obpb-color-out-of-range",
    "obpb-style-flag-unknown",
    "obpb-underline-kind-out-of-range",
    "obpb-width-spacer-tail-without-head",
    "obpb-row-ends-with-wide-cell",
    "obpb-semantic-out-of-range",
    "obpb-protected-out-of-range",
    "obpb-adjacent-wrap-disagreement",
    "obpb-interaction-unknown-required-flag",
    "obpb-interaction-mouse-mode-conflict",
    "obpb-sparse-delta-increasing-row-indices",
    "obpb-sparse-delta-duplicate-row-indices",
    "obpb-sparse-delta-descending-row-indices",
    "obpb-optional-header-flag-01",
    "obpb-optional-header-flags-7f",
    "obpb-unknown-required-header-flag-80",
};

const row_window_conformance_case_names = [_][]const u8{
    "obrw-optional-header-flag-01",
    "obrw-optional-header-flags-7f",
    "obrw-unknown-required-header-flag-80",
    "obrw-projection-section-length-mismatch",
    "obrw-graphics-section-length-mismatch",
    "obrw-graphics-update-not-snapshot",
};

const packed_projection_conformance_case_count =
    projection_conformance_case_names.len + row_window_conformance_case_names.len;

comptime {
    if (projection_conformance_case_names.len != 23) @compileError("expected 23 OBPB cases");
    if (row_window_conformance_case_names.len != 6) @compileError("expected 6 OBRW cases");
}

const PackedProjectionCaseRoute = enum {
    projection_buffer,
    row_window,
};

const PackedProjectionCaseIdentity = struct {
    inventory_index: usize,
    route: PackedProjectionCaseRoute,
};

fn packedProjectionCaseIdentity(
    name: []const u8,
    codec: []const u8,
) ?PackedProjectionCaseIdentity {
    if (std.mem.eql(u8, codec, "OBPB")) {
        for (projection_conformance_case_names, 0..) |authorized_name, index| {
            if (std.mem.eql(u8, name, authorized_name)) {
                return .{ .inventory_index = index, .route = .projection_buffer };
            }
        }
        return null;
    }
    if (std.mem.eql(u8, codec, "OBRW")) {
        for (row_window_conformance_case_names, 0..) |authorized_name, index| {
            if (std.mem.eql(u8, name, authorized_name)) {
                return .{
                    .inventory_index = projection_conformance_case_names.len + index,
                    .route = .row_window,
                };
            }
        }
        return null;
    }
    return null;
}

const PackedProjectionInventory = struct {
    seen: [packed_projection_conformance_case_count]bool =
        [_]bool{false} ** packed_projection_conformance_case_count,
    projection_case_count: usize = 0,
    row_window_case_count: usize = 0,

    fn observe(
        self: *PackedProjectionInventory,
        name: []const u8,
        codec: []const u8,
    ) !PackedProjectionCaseRoute {
        const identity = packedProjectionCaseIdentity(name, codec) orelse
            return error.InvalidConformanceCorpus;
        if (self.seen[identity.inventory_index]) return error.InvalidConformanceCorpus;
        self.seen[identity.inventory_index] = true;
        switch (identity.route) {
            .projection_buffer => self.projection_case_count += 1,
            .row_window => self.row_window_case_count += 1,
        }
        return identity.route;
    }

    fn validateComplete(self: *const PackedProjectionInventory) !void {
        if (self.projection_case_count != projection_conformance_case_names.len or
            self.row_window_case_count != row_window_conformance_case_names.len)
        {
            return error.InvalidConformanceCorpus;
        }
        for (self.seen) |was_seen| {
            if (!was_seen) return error.InvalidConformanceCorpus;
        }
    }
};

fn decodeCanonicalBase64(
    allocator: std.mem.Allocator,
    encoded: []const u8,
) ![]u8 {
    if (encoded.len == 0) return error.InvalidConformanceCorpus;
    const decoded_length = std.base64.standard.Decoder.calcSizeForSlice(encoded) catch
        return error.InvalidConformanceCorpus;
    if (decoded_length == 0) return error.InvalidConformanceCorpus;
    const decoded = try allocator.alloc(u8, decoded_length);
    errdefer allocator.free(decoded);
    std.base64.standard.Decoder.decode(decoded, encoded) catch
        return error.InvalidConformanceCorpus;

    const canonical = try allocator.alloc(
        u8,
        std.base64.standard.Encoder.calcSize(decoded.len),
    );
    defer allocator.free(canonical);
    _ = std.base64.standard.Encoder.encode(canonical, decoded);
    if (!std.mem.eql(u8, canonical, encoded)) return error.InvalidConformanceCorpus;
    return decoded;
}

fn expectedPackedProjectionError(expected: PackedProjectionExpected) ?protocol.ProtocolError {
    if (!std.mem.eql(u8, expected.result, "failure") or expected.observations != null) {
        return null;
    }
    const code = expected.code orelse return null;
    if (std.mem.eql(u8, code, "invalid-field")) return error.InvalidField;
    if (std.mem.eql(u8, code, "invalid-utf8")) return error.InvalidUtf8;
    if (std.mem.eql(u8, code, "length-mismatch")) return error.LengthMismatch;
    if (std.mem.eql(u8, code, "unknown-required-flag")) return error.UnknownRequiredFlag;
    return null;
}

fn expectedProjectionSuccess(
    expected: PackedProjectionExpected,
) !ProjectionSuccessObservation {
    if (!std.mem.eql(u8, expected.result, "success") or expected.code != null) {
        return error.InvalidConformanceCorpus;
    }
    const observations = expected.observations orelse return error.InvalidConformanceCorpus;
    if (observations.detail != null or
        (observations.ignoredOptionalFlags orelse 0x80) > 0x7f)
    {
        return error.InvalidConformanceCorpus;
    }
    const row_count = observations.rowCount orelse return error.InvalidConformanceCorpus;
    const row_indices = observations.rowIndices orelse return error.InvalidConformanceCorpus;
    if (row_indices.len != @as(usize, row_count)) return error.InvalidConformanceCorpus;
    var previous: ?u16 = null;
    for (row_indices) |row| {
        if (row >= 4096 or (previous != null and row <= previous.?)) {
            return error.InvalidConformanceCorpus;
        }
        previous = row;
    }
    const kind_name = observations.kind orelse return error.InvalidConformanceCorpus;
    const kind: protocol.FrameKind = if (std.mem.eql(u8, kind_name, "snapshot"))
        .snapshot
    else if (std.mem.eql(u8, kind_name, "delta"))
        .delta
    else
        return error.InvalidConformanceCorpus;
    return .{ .kind = kind, .row_count = row_count };
}

fn expectedRowWindowSuccess(
    expected: PackedProjectionExpected,
) !RowWindowSuccessObservation {
    if (!std.mem.eql(u8, expected.result, "success") or expected.code != null) {
        return error.InvalidConformanceCorpus;
    }
    const observations = expected.observations orelse return error.InvalidConformanceCorpus;
    if (observations.kind != null or observations.rowIndices != null or
        (observations.ignoredOptionalFlags orelse 0x80) > 0x7f)
    {
        return error.InvalidConformanceCorpus;
    }
    const row_count = observations.rowCount orelse return error.InvalidConformanceCorpus;
    if (row_count == 0 or row_count > 4096) return error.InvalidConformanceCorpus;
    const detail_name = observations.detail orelse return error.InvalidConformanceCorpus;
    const detail: protocol.RowWindowDetail = if (std.mem.eql(u8, detail_name, "render"))
        .render
    else if (std.mem.eql(u8, detail_name, "text"))
        .text
    else
        return error.InvalidConformanceCorpus;
    return .{ .detail = detail, .row_count = row_count };
}

fn buildRenderRowWindow(allocator: std.mem.Allocator, snapshot: []const u8) ![]u8 {
    var graphics: [protocol.terminal_graphics_header_bytes]u8 = @splat(0);
    @memcpy(graphics[0..4], protocol.terminal_graphics_magic);
    graphics[4] = protocol.terminal_graphics_format_major;
    graphics[5] = protocol.terminal_graphics_format_minor;
    graphics[6] = protocol.terminal_graphics_format_patch;
    graphics[7] = @intFromEnum(protocol.TerminalGraphicsUpdateKind.snapshot);
    std.mem.writeInt(u32, graphics[8..12], graphics.len, .little);

    const body_length = snapshot.len + graphics.len;
    const total = protocol.terminal_row_window_header_bytes + body_length;
    const bytes = try allocator.alloc(u8, total);
    @memcpy(bytes[0..4], protocol.terminal_row_window_magic);
    bytes[4] = protocol.terminal_row_window_format_major;
    bytes[5] = protocol.terminal_row_window_format_minor;
    bytes[6] = 1; // render detail
    bytes[7] = 0; // flags
    std.mem.writeInt(u32, bytes[8..12], @intCast(total), .little);
    std.mem.writeInt(u16, bytes[12..14], protocol.terminal_row_window_header_bytes, .little);
    std.mem.writeInt(u16, bytes[14..16], 6, .little); // columns
    @memset(bytes[16..32], 0x11); // host epoch
    @memset(bytes[32..48], 0x22); // attachment id
    std.mem.writeInt(u64, bytes[48..56], 7, .little); // sequence
    std.mem.writeInt(u64, bytes[56..64], 42, .little); // revision
    std.mem.writeInt(u64, bytes[64..72], 3, .little); // generation
    std.mem.writeInt(u64, bytes[72..80], 125, .little); // total rows
    std.mem.writeInt(u64, bytes[80..88], 123, .little); // start row
    std.mem.writeInt(u16, bytes[88..90], 2, .little); // row count
    std.mem.writeInt(u16, bytes[90..92], 0, .little); // reserved
    std.mem.writeInt(u32, bytes[92..96], 12, .little); // entry count
    std.mem.writeInt(u32, bytes[96..100], @intCast(body_length), .little); // body length
    bytes[100] = protocol.terminal_row_window_format_patch;
    std.mem.writeInt(u32, bytes[101..105], @intCast(snapshot.len), .little);
    std.mem.writeInt(u32, bytes[105..109], graphics.len, .little);
    @memset(bytes[109..112], 0); // reserved tail
    @memcpy(bytes[112..][0..snapshot.len], snapshot);
    @memcpy(bytes[112 + snapshot.len ..], &graphics);
    return bytes;
}

const TextRowWindowMapping = struct {
    byte_offset: u32,
    utf16_offset: u32,
    column: u16,
    width: u8,
};

/// Hand-assembles a one-row text-detail row window directly from wire layout so
/// conformance tests can feed the native validator a corpus the Host cannot emit.
fn buildTextRowWindow(
    allocator: std.mem.Allocator,
    columns: u16,
    text: []const u8,
    mappings: []const TextRowWindowMapping,
) ![]u8 {
    const mapping_count: u32 = @intCast(mappings.len);
    const body_length: u32 = 12 + @as(u32, @intCast(text.len)) + mapping_count * 12;
    const total: usize = protocol.terminal_row_window_header_bytes + body_length;
    const bytes = try allocator.alloc(u8, total);
    @memcpy(bytes[0..4], protocol.terminal_row_window_magic);
    bytes[4] = protocol.terminal_row_window_format_major;
    bytes[5] = protocol.terminal_row_window_format_minor;
    bytes[6] = 2; // text detail
    bytes[7] = 0; // flags
    std.mem.writeInt(u32, bytes[8..12], @intCast(total), .little);
    std.mem.writeInt(u16, bytes[12..14], protocol.terminal_row_window_header_bytes, .little);
    std.mem.writeInt(u16, bytes[14..16], columns, .little);
    @memset(bytes[16..32], 0x11); // host epoch
    @memset(bytes[32..48], 0x22); // attachment id
    std.mem.writeInt(u64, bytes[48..56], 7, .little); // sequence
    std.mem.writeInt(u64, bytes[56..64], 42, .little); // revision
    std.mem.writeInt(u64, bytes[64..72], 3, .little); // generation
    std.mem.writeInt(u64, bytes[72..80], 125, .little); // total rows
    std.mem.writeInt(u64, bytes[80..88], 123, .little); // start row
    std.mem.writeInt(u16, bytes[88..90], 1, .little); // row count
    std.mem.writeInt(u16, bytes[90..92], 0, .little); // reserved
    std.mem.writeInt(u32, bytes[92..96], mapping_count, .little); // entry count
    std.mem.writeInt(u32, bytes[96..100], body_length, .little); // body length
    bytes[100] = protocol.terminal_row_window_format_patch;
    @memset(bytes[101..112], 0); // reserved tail
    bytes[112] = 0; // row flags: no wrap, not identity-ascii
    bytes[113] = 0; // prompt class: none
    std.mem.writeInt(u16, bytes[114..116], 0, .little);
    std.mem.writeInt(u32, bytes[116..120], @intCast(text.len), .little);
    std.mem.writeInt(u32, bytes[120..124], mapping_count, .little);
    @memcpy(bytes[124..][0..text.len], text);
    var offset: usize = 124 + text.len;
    for (mappings) |mapping| {
        std.mem.writeInt(u32, bytes[offset..][0..4], mapping.byte_offset, .little);
        std.mem.writeInt(u32, bytes[offset + 4 ..][0..4], mapping.utf16_offset, .little);
        std.mem.writeInt(u16, bytes[offset + 8 ..][0..2], mapping.column, .little);
        bytes[offset + 10] = mapping.width;
        bytes[offset + 11] = 0;
        offset += 12;
    }
    return bytes;
}

const ControlCommandsFixture = struct {
    apply_terminal_theme: []const u8,
    attach_terminal: []const u8,
    claim_resize_lease: []const u8,
    clear_terminal: []const u8,
    create_terminal: []const u8,
    create_terminal_deferred: []const u8,
    detach_terminal: []const u8,
    empty: []const u8,
    frame_applied: []const u8,
    hello: []const u8,
    history_demand: []const []const u8,
    history_observed: []const u8,
    list_terminals: []const u8,
    need_snapshot: []const []const u8,
    projection_demand: []const []const u8,
    read_rows: []const []const u8,
    resize_terminal: []const u8,
    reset_terminal: []const u8,
    rows_applied: []const u8,
    selection_command: []const []const u8,
    selection_export: []const []const u8,
    selection_export_cancel: []const []const u8,
    selection_export_chunk_applied: []const u8,
    selection_gesture: []const []const u8,
    set_terminal_defaults: []const u8,
    start_terminal: []const u8,
    terminal_target: []const u8,
    update_attachment_input_capability: []const u8,
    update_terminal_configuration: []const u8,
};

const ControlOutputsFixture = struct {
    attachment_acknowledged: []const u8,
    attachment_capabilities: []const u8,
    empty: []const u8,
    frame_applied: []const u8,
    hello_accepted: []const u8,
    history_changed: []const []const u8,
    history_demand_accepted: []const []const u8,
    history_observed: []const u8,
    input_accepted: []const u8,
    need_snapshot: []const []const u8,
    projection_blocked: []const []const u8,
    projection_demand_accepted: []const []const u8,
    protocol_rejected: []const u8,
    resize_accepted: []const u8,
    resize_lease_changed: []const u8,
    rows_applied: []const u8,
    selection_changed: []const []const u8,
    selection_export_chunk_applied: []const u8,
    selection_export_finished: []const []const u8,
    selection_export_started: []const []const u8,
    terminal_acknowledged: []const u8,
    terminal_bell: []const u8,
    terminal_clipboard_write_request: []const u8,
    terminal_configuration_accepted: []const u8,
    terminal_control_accepted: []const []const u8,
    terminal_defaults_accepted: []const u8,
    terminal_exited: []const []const u8,
    terminal_failed: []const []const u8,
    terminal_identity: []const u8,
    terminal_list: []const u8,
    terminal_start_accepted: []const []const u8,
    terminal_started: []const u8,
    terminal_theme_result: []const []const u8,
    typed_failure: []const []const u8,
};

const ControlMessageRequestContractFixture = struct {
    payload_encoding: []const u8,
    success_kind: u16,
};

const ControlMessageContractFixture = struct {
    event_encoding: ?[]const u8,
    kind: u16,
    name: []const u8,
    request: ?ControlMessageRequestContractFixture,
    response_encoding: ?[]const u8,
};

const ControlRawCaseFixture = struct {
    decoder: []const u8,
    expected_code: ?[]const u8,
    expected_integer: ?u64,
    name: []const u8,
    payload_base64: []const u8,
};

const ProtocolLimitsFixture = struct {
    cell_pixel_dimension: u64,
    clipboard_text_bytes: u64,
    control_json_bytes: u64,
    environment_name_bytes: u64,
    extension_bytes: u64,
    frame_bytes: u64,
    frame_cells: u64,
    frame_columns: u64,
    frame_extensions: u64,
    frame_rows: u64,
    frame_strings: u64,
    frame_styles: u64,
    terminal_graphics_destination_pixels: u64,
    terminal_graphics_dimension: u64,
    terminal_graphics_extension_bytes: u64,
    terminal_graphics_images: u64,
    terminal_graphics_placements: u64,
    terminal_graphics_resource_bytes: u64,
    input_accepted_bytes: u64,
    input_bytes: u64,
    inventory_page_terminals: u64,
    key_text_bytes: u64,
    launch_arguments: u64,
    launch_environment_entries: u64,
    optional_feature_name_bytes: u64,
    optional_features: u64,
    pending_input_bytes: u64,
    pty_read_batch_bytes: u64,
    raw_payload_bytes: u64,
    row_window_bytes: u64,
    row_window_rows: u64,
    scrollback_bytes: u64,
    search_query_bytes: u64,
    selection_export_bytes: u64,
    selection_export_chunk_bytes: u64,
    selection_preview_bytes: u64,
    semantic_input_payload_bytes: u64,
    string_bytes: u64,
    terminal_profile_features: u64,
    terminal_profile_palette_entries: u64,
    terminal_response_bytes: u64,
    transport_record_bytes: u64,
    write_input_payload_bytes: u64,
};

const ControlConformanceFixture = struct {
    commands: ControlCommandsFixture,
    message_contracts: []const ControlMessageContractFixture,
    outputs: ControlOutputsFixture,
    protocol_limits: ProtocolLimitsFixture,
    raw_cases: []const ControlRawCaseFixture,
};

const Utf8BomInputCaseFixture = struct {
    decoder: []const u8,
    expected_code: ?[]const u8,
    expected_text_base64: ?[]const u8,
    name: []const u8,
    payload_base64: []const u8,
};

const Utf8BomConformanceFixture = struct {
    cases: []const Utf8BomInputCaseFixture,
};

const TestIdentifier = struct { bytes: [16]u8 };

const TestRuntimeFailureKind = enum {
    process_control,
    process_observation,
    pty_read,
    pty_write,
    resize,
    terminal_reply,
    worker_integrity,
};

const TestRuntimeFailureStage = enum {
    admit_terminal_reply,
    apply_resize,
    contain_process,
    dispatch_termination,
    observe_process,
    read_output,
    release_output,
    release_process_identity,
    synchronize_worker,
    write_input,
};

const TestRuntimeFailure = struct {
    kind: TestRuntimeFailureKind,
    stage: TestRuntimeFailureStage,
};

const TestExitResult = union(enum) {
    exit_code: u32,
    signal: u32,
    unknown: u32,
};

const TestProcessObservation = union(enum) {
    observed: TestExitResult,
    unavailable: TestRuntimeFailure,
};

const TestFailureNotice = struct {
    creation_id: TestIdentifier,
    failure: TestRuntimeFailure,
    final_revision: u64,
    process: TestProcessObservation,
    terminal_id: TestIdentifier,
};

const TestExitNotice = struct {
    creation_id: TestIdentifier,
    final_revision: u64,
    result: TestExitResult,
    terminal_id: TestIdentifier,
};

const TestTerminalPhase = enum {
    awaiting_start,
    running,
    finishing,
    containing_failure,
    exited,
    failed,
};

const TestFailureContainment = union(enum) {
    stopping: struct { process: ?TestProcessObservation },
    blocked: struct {
        cleanup_failure: TestRuntimeFailure,
        process: ?TestProcessObservation,
    },
};

const TestTerminalConfiguration = struct { scrollback_bytes: usize };
const TestTerminalGeometry = struct { columns: u16, rows: u16 };

const test_theme_palette = palette: {
    var palette: [256]u32 = undefined;
    for (&palette, 0..) |*color, index| color.* = @intCast(index * 0x01_01_01);
    break :palette palette;
};

const TestTerminalTheme = struct {
    color_scheme: enum { dark, light } = .dark,
    default_background_rgb: u32 = 0x10_10_10,
    default_cursor_rgb: ?u32 = null,
    default_foreground_rgb: u32 = 0xf0_f0_f0,
    palette: [256]u32 = test_theme_palette,
};

const TestTerminalThemeState = struct {
    operation_id: ?TestIdentifier = null,
    revision: u64 = 0,
    terminal: TestTerminalTheme = .{},
};

const TestStartTransition = enum { started, already_started };

const TestStartOutcome = struct {
    geometry: struct {
        cell_pixel_height: u16,
        cell_pixel_width: u16,
        columns: u16,
        rows: u16,
    },
    identity: struct {
        creation_id: TestIdentifier,
        terminal_id: TestIdentifier,
    },
    transition: TestStartTransition,
};

const TestTerminal = struct {
    creation_id: TestIdentifier,
    terminal_id: TestIdentifier,
    columns: ?u16 = 120,
    rows: ?u16 = 40,
    configuration: TestTerminalConfiguration = .{ .scrollback_bytes = 10_000_000 },
    configuration_revision: u64 = 2,
    revision: u64 = 42,
    phase_value: TestTerminalPhase,
    exit_result: ?TestExitResult = null,
    failure: ?TestRuntimeFailure = null,
    containment: ?TestFailureContainment = null,
    process: ?TestProcessObservation = null,
    terminate_requested: bool = false,
    theme_state: TestTerminalThemeState = .{},

    pub fn phase(self: @This()) TestTerminalPhase {
        return self.phase_value;
    }

    pub fn effectiveConfiguration(self: @This()) TestTerminalConfiguration {
        return self.configuration;
    }

    pub fn configurationRevision(self: @This()) u64 {
        return self.configuration_revision;
    }

    pub fn maybeObservedGeometry(self: @This()) ?TestTerminalGeometry {
        return .{
            .columns = self.columns orelse return null,
            .rows = self.rows orelse return null,
        };
    }

    pub fn observedRevision(self: @This()) u64 {
        return self.revision;
    }

    pub fn exitResult(self: @This()) ?TestExitResult {
        return self.exit_result;
    }

    pub fn runtimeFailure(self: @This()) ?TestRuntimeFailure {
        return self.failure;
    }

    pub fn failureContainment(self: @This()) ?TestFailureContainment {
        return self.containment;
    }

    pub fn failedProcess(self: @This()) ?TestProcessObservation {
        return self.process;
    }

    pub fn terminateRequested(self: @This()) bool {
        return self.terminate_requested;
    }

    pub fn themeState(self: @This()) TestTerminalThemeState {
        return self.theme_state;
    }
};

const TestProjectionDemand = enum { active, paused };
const TestTerminalControlOperation = enum { clear, reset };
const TestSnapshotReason = enum {
    activity_stale,
    base_mismatch,
    decoder_rejected,
    dimensions_mismatch,
    generation_mismatch,
    identity_mismatch,
    sequence_gap,
    store_reset,
};
const TestHistoryDemand = enum { following_bottom, viewing_history };
const TestProjectionBlock = enum { capture_allocation_failed, projection_too_large };
const TestAnchorStatus = enum { evicted, none, valid };
const TestExportFormat = enum { plain };
const TestExportStatus = enum { cancelled, completed };
const TestSelectionPoint = struct { column: u16, row: u64 };
const TestSelectionSnapshot = union(enum) {
    none,
    evicted,
    active: struct {
        end: TestSelectionPoint,
        rectangle: bool,
        start: TestSelectionPoint,
    },
};

const TerminalFailedWire = struct {
    failure: struct {
        kind: []const u8,
        stage: []const u8,
    },
    finalRevision: []const u8,
    identity: struct {
        creationId: []const u8,
        hostEpoch: []const u8,
        terminalId: []const u8,
    },
    process: struct {
        kind: []const u8,
        result: struct {
            code: u32,
            kind: []const u8,
        },
    },
};

test "canonical TypeScript terminal frame fixture conforms in Zig" {
    const summary = try protocol.validateTerminalFrame(std.testing.allocator, frame_fixture);
    try std.testing.expectEqual(protocol.FrameKind.delta, summary.kind);
    try std.testing.expectEqual(@as(u64, 7), summary.sequence);
    try std.testing.expectEqual(@as(?u64, 41), summary.base_revision);
    try std.testing.expectEqual(@as(u64, 42), summary.revision);
    try std.testing.expectEqual(@as(u64, 3), summary.generation);
    try std.testing.expectEqual(@as(u16, 6), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
    try std.testing.expectEqual(@as(usize, 12), summary.cell_count);
    try std.testing.expectEqual(@as(u16, 2), summary.style_count);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u16, 1), summary.extension_count);
    try std.testing.expectEqualStrings("Orchbee 🐝", summary.title.?);
    try std.testing.expectEqualStrings("/tmp/项目", summary.current_directory.?);
    try std.testing.expectEqual(protocol.CursorShape.bar, summary.cursor_shape);
    try std.testing.expectEqual(protocol.ActiveScreen.alternate, summary.active_screen);
    try std.testing.expectEqual(@as(u64, 123), summary.scrollback_rows);
    try std.testing.expectEqual(@as(u16, 8), summary.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 16), summary.cell_pixel_height);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x11} ** 16), &summary.host_epoch);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
}

test "canonical TypeScript terminal projection-buffer fixture conforms in Zig" {
    const summary = try protocol.validateTerminalProjectionBuffer(projection_fixture);
    try std.testing.expectEqual(protocol.FrameKind.delta, summary.kind);
    try std.testing.expectEqual(@as(u64, 7), summary.sequence);
    try std.testing.expectEqual(@as(?u64, 41), summary.base_revision);
    try std.testing.expectEqual(@as(u64, 42), summary.revision);
    try std.testing.expectEqual(@as(u64, 3), summary.generation);
    try std.testing.expectEqual(@as(u16, 6), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.rows);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u64, 123), summary.scrollback_rows);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x11} ** 16), &summary.host_epoch);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
}

test "packed projection inventory accepts every authorized ID exactly once in any order" {
    var inventory = PackedProjectionInventory{};

    var index = row_window_conformance_case_names.len;
    while (index > 0) {
        index -= 1;
        try std.testing.expectEqual(
            PackedProjectionCaseRoute.row_window,
            try inventory.observe(row_window_conformance_case_names[index], "OBRW"),
        );
    }
    index = projection_conformance_case_names.len;
    while (index > 0) {
        index -= 1;
        try std.testing.expectEqual(
            PackedProjectionCaseRoute.projection_buffer,
            try inventory.observe(projection_conformance_case_names[index], "OBPB"),
        );
    }
    try inventory.validateComplete();
}

test "packed projection inventory rejects unknown duplicate and incomplete case IDs" {
    var unknown = PackedProjectionInventory{};
    try std.testing.expectError(
        error.InvalidConformanceCorpus,
        unknown.observe("obpb-unrecognized-but-valid", "OBPB"),
    );

    var duplicate = PackedProjectionInventory{};
    _ = try duplicate.observe(projection_conformance_case_names[0], "OBPB");
    try std.testing.expectError(
        error.InvalidConformanceCorpus,
        duplicate.observe(projection_conformance_case_names[0], "OBPB"),
    );

    var incomplete = PackedProjectionInventory{};
    try std.testing.expectError(error.InvalidConformanceCorpus, incomplete.validateComplete());
}

test "shared packed projection corpus drives both native strict validators" {
    const allocator = std.testing.allocator;
    try std.testing.expect(packed_projection_conformance_fixture.len > 0);
    const parsed = try std.json.parseFromSlice(
        PackedProjectionCorpus,
        allocator,
        packed_projection_conformance_fixture,
        .{},
    );
    defer parsed.deinit();
    try std.testing.expectEqual(@as(u8, 1), parsed.value.schemaVersion);
    try std.testing.expectEqualStrings("0.0.3", parsed.value.formatVersion);
    try std.testing.expectEqual(@as(usize, 29), parsed.value.cases.len);

    var inventory = PackedProjectionInventory{};
    for (parsed.value.cases) |case| {
        const route = try inventory.observe(case.name, case.codec);
        const bytes = try decodeCanonicalBase64(allocator, case.bytesBase64);
        defer allocator.free(bytes);

        switch (route) {
            .projection_buffer => {
                if (!std.mem.eql(u8, case.strictBoundary, "projection-buffer")) {
                    return error.InvalidConformanceCorpus;
                }
                if (std.mem.eql(u8, case.expected.result, "failure")) {
                    const expected = expectedPackedProjectionError(case.expected) orelse
                        return error.InvalidConformanceCorpus;
                    if (protocol.validateTerminalProjectionBuffer(bytes)) |_| {
                        std.debug.print("packed projection case unexpectedly passed: {s}\n", .{case.name});
                        return error.TestExpectedError;
                    } else |err| {
                        if (err != expected) {
                            std.debug.print("packed projection case {s}: expected {s}, got {s}\n", .{
                                case.name,
                                @errorName(expected),
                                @errorName(err),
                            });
                            return error.TestUnexpectedError;
                        }
                    }
                } else {
                    const expected = try expectedProjectionSuccess(case.expected);
                    const summary = protocol.validateTerminalProjectionBuffer(bytes) catch |err| {
                        std.debug.print("packed projection case {s} failed: {s}\n", .{
                            case.name,
                            @errorName(err),
                        });
                        return err;
                    };
                    try std.testing.expectEqual(expected.kind, summary.kind);
                    try std.testing.expectEqual(expected.row_count, summary.row_count);
                }
            },
            .row_window => {
                if (!std.mem.eql(u8, case.strictBoundary, "row-window")) {
                    return error.InvalidConformanceCorpus;
                }
                if (std.mem.eql(u8, case.expected.result, "failure")) {
                    const expected = expectedPackedProjectionError(case.expected) orelse
                        return error.InvalidConformanceCorpus;
                    if (protocol.validateTerminalRowWindow(allocator, bytes)) |_| {
                        std.debug.print("packed row-window case unexpectedly passed: {s}\n", .{case.name});
                        return error.TestExpectedError;
                    } else |err| {
                        if (err != expected) {
                            std.debug.print("packed row-window case {s}: expected {s}, got {s}\n", .{
                                case.name,
                                @errorName(expected),
                                @errorName(err),
                            });
                            return error.TestUnexpectedError;
                        }
                    }
                } else {
                    const expected = try expectedRowWindowSuccess(case.expected);
                    const summary = protocol.validateTerminalRowWindow(allocator, bytes) catch |err| {
                        std.debug.print("packed row-window case {s} failed: {s}\n", .{
                            case.name,
                            @errorName(err),
                        });
                        return err;
                    };
                    try std.testing.expectEqual(expected.detail, summary.detail);
                    try std.testing.expectEqual(expected.row_count, summary.row_count);
                }
            },
        }
    }
    try inventory.validateComplete();
}

test "render row window delegates to the strict projection scan" {
    const allocator = std.testing.allocator;
    const parsed = try std.json.parseFromSlice(
        PackedProjectionCorpus,
        allocator,
        packed_projection_conformance_fixture,
        .{},
    );
    defer parsed.deinit();

    var authoritative_case: ?PackedProjectionCase = null;
    for (parsed.value.cases) |case| {
        if (std.mem.eql(u8, case.name, "obpb-width-out-of-range")) {
            if (authoritative_case != null) return error.InvalidConformanceCorpus;
            authoritative_case = case;
        }
    }
    const corpus_case = authoritative_case orelse return error.InvalidConformanceCorpus;
    const identity = packedProjectionCaseIdentity(corpus_case.name, corpus_case.codec) orelse
        return error.InvalidConformanceCorpus;
    if (identity.route != .projection_buffer or
        !std.mem.eql(u8, corpus_case.strictBoundary, "projection-buffer"))
    {
        return error.InvalidConformanceCorpus;
    }

    const expected = expectedPackedProjectionError(corpus_case.expected) orelse
        return error.InvalidConformanceCorpus;
    const malformed_projection = try decodeCanonicalBase64(allocator, corpus_case.bytesBase64);
    defer allocator.free(malformed_projection);
    const window = try buildRenderRowWindow(allocator, malformed_projection);
    defer allocator.free(window);
    try std.testing.expectError(expected, protocol.validateTerminalRowWindow(allocator, window));
}

test "canonical TypeScript terminal row-window fixture conforms in Zig" {
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, row_window_fixture);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u64, 8), summary.sequence);
    try std.testing.expectEqual(@as(u64, 42), summary.revision);
    try std.testing.expectEqual(@as(u64, 3), summary.generation);
    try std.testing.expectEqual(@as(u16, 6), summary.columns);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u32, 6), summary.entry_count);
    try std.testing.expectEqual(@as(u64, 100), summary.start_row);
    try std.testing.expectEqual(@as(u64, 125), summary.total_rows);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x11} ** 16), &summary.host_epoch);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
}

test "canonical TypeScript envelope carries the exact canonical frame" {
    const summary = try protocol.validateTransportEnvelope(envelope_fixture);
    try std.testing.expect(summary.event);
    try std.testing.expect(!summary.response);
    try std.testing.expect(!summary.optional_message);
    try std.testing.expectEqual(@as(u16, 42), summary.kind);
    try std.testing.expectEqual(@as(u8, 3), summary.payload_encoding);
    try std.testing.expectEqual(protocol.current_protocol_major, summary.protocol_major);
    try std.testing.expectEqual(protocol.current_protocol_minor, summary.protocol_minor);
    try std.testing.expectEqual(protocol.current_protocol_patch, summary.protocol_patch);
    try std.testing.expectEqualSlices(u8, frame_fixture, summary.payload);
}

/// Hand-assembles a wire-valid OBTP transport envelope whose row-window
/// payload is exactly the row-window bound, making the whole record equal the
/// transport-record bound (64-byte header + 44 MiB payload). The payload is
/// opaque to the envelope validator, so a zero-filled body is a valid
/// row-window payload declaration for envelope admission purposes.
fn buildBoundaryEnvelopeRecord(allocator: std.mem.Allocator) ![]u8 {
    const payload_length = protocol.Limits.row_window_bytes;
    const record_length = protocol.transport_envelope_header_bytes + payload_length;
    const bytes = try allocator.alloc(u8, record_length);
    errdefer allocator.free(bytes);
    @memset(bytes, 0);
    @memcpy(bytes[0..4], "OBTP");
    std.mem.writeInt(u16, bytes[4..6], protocol.transport_envelope_header_bytes, .little);
    std.mem.writeInt(u16, bytes[6..8], 4, .little); // flags: event
    std.mem.writeInt(u32, bytes[8..12], @intCast(record_length), .little);
    std.mem.writeInt(u32, bytes[12..16], @intCast(payload_length), .little);
    std.mem.writeInt(u16, bytes[16..18], protocol.current_protocol_major, .little);
    std.mem.writeInt(u16, bytes[18..20], protocol.current_protocol_minor, .little);
    std.mem.writeInt(u16, bytes[20..22], protocol.MessageKind.rows_snapshot, .little);
    bytes[22] = @intFromEnum(protocol.PayloadEncoding.row_window);
    std.mem.writeInt(u16, bytes[56..58], protocol.current_protocol_patch, .little);
    return bytes;
}

test "envelope fixture cap is tied to the authoritative transport-record bound" {
    try std.testing.expectEqual(protocol.Limits.transport_record_bytes, envelope_fixture_max_bytes);
    try std.testing.expectEqual(
        protocol.Limits.row_window_bytes + protocol.transport_envelope_header_bytes,
        envelope_fixture_max_bytes,
    );
}

test "envelope fixture admission admits a record at the transport-record bound" {
    const allocator = std.testing.allocator;
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();

    const record = try buildBoundaryEnvelopeRecord(allocator);
    defer allocator.free(record);
    try std.testing.expectEqual(protocol.Limits.transport_record_bytes, record.len);

    const path = "envelope-at-record-bound.bin";
    {
        const file = try tmp.dir.createFile(std.testing.io, path, .{});
        defer file.close(std.testing.io);
        try file.writeStreamingAll(std.testing.io, record);
    }
    const admitted = try tmp.dir.readFileAlloc(
        std.testing.io,
        path,
        allocator,
        .limited(envelope_fixture_max_bytes + 1),
    );
    defer allocator.free(admitted);
    try std.testing.expectEqual(protocol.Limits.transport_record_bytes, admitted.len);

    const summary = try protocol.validateTransportEnvelope(admitted);
    try std.testing.expectEqual(@as(usize, protocol.Limits.row_window_bytes), summary.payload.len);
    try std.testing.expectEqual(protocol.MessageKind.rows_snapshot, summary.kind);
    try std.testing.expectEqual(@intFromEnum(protocol.PayloadEncoding.row_window), summary.payload_encoding);

    // When the build is configured with a boundary-size envelope fixture, the
    // fixture itself must validate to the row-window-bound payload as well.
    if (envelope_fixture.len == protocol.Limits.transport_record_bytes) {
        const fixture_summary = try protocol.validateTransportEnvelope(envelope_fixture);
        try std.testing.expectEqual(@as(usize, protocol.Limits.row_window_bytes), fixture_summary.payload.len);
    }
}

test "envelope fixture admission rejects one byte over the transport-record bound" {
    const allocator = std.testing.allocator;
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();

    const record = try buildBoundaryEnvelopeRecord(allocator);
    defer allocator.free(record);
    const over = try allocator.alloc(u8, record.len + 1);
    defer allocator.free(over);
    @memcpy(over[0..record.len], record);
    over[record.len] = 0;

    const path = "envelope-over-record-bound.bin";
    {
        const file = try tmp.dir.createFile(std.testing.io, path, .{});
        defer file.close(std.testing.io);
        try file.writeStreamingAll(std.testing.io, over);
    }
    try std.testing.expectError(
        error.StreamTooLong,
        tmp.dir.readFileAlloc(
            std.testing.io,
            path,
            allocator,
            .limited(envelope_fixture_max_bytes + 1),
        ),
    );
}

test "canonical TypeScript TerminalFailed fixture matches the native encoder" {
    const allocator = std.testing.allocator;
    const encoded = try control_encode.terminalFailed(
        allocator,
        TestIdentifier{ .bytes = [_]u8{0x11} ** 16 },
        TestFailureNotice{
            .creation_id = .{ .bytes = [_]u8{0x33} ** 16 },
            .failure = .{
                .kind = .pty_read,
                .stage = .read_output,
            },
            .final_revision = 42,
            .process = .{ .observed = .{ .exit_code = 1 } },
            .terminal_id = .{ .bytes = [_]u8{0x44} ** 16 },
        },
    );
    defer allocator.free(encoded);
    const fixture = try std.json.parseFromSlice(
        TerminalFailedWire,
        allocator,
        terminal_failed_fixture,
        .{},
    );
    defer fixture.deinit();
    const native = try std.json.parseFromSlice(TerminalFailedWire, allocator, encoded, .{});
    defer native.deinit();

    for ([_]TerminalFailedWire{ fixture.value, native.value }) |value| {
        try std.testing.expectEqualStrings("pty-read", value.failure.kind);
        try std.testing.expectEqualStrings("read-output", value.failure.stage);
        try std.testing.expectEqualStrings("42", value.finalRevision);
        try std.testing.expectEqualStrings("33333333333333333333333333333333", value.identity.creationId);
        try std.testing.expectEqualStrings("11111111111111111111111111111111", value.identity.hostEpoch);
        try std.testing.expectEqualStrings("44444444444444444444444444444444", value.identity.terminalId);
        try std.testing.expectEqualStrings("observed", value.process.kind);
        try std.testing.expectEqualStrings("exit-code", value.process.result.kind);
        try std.testing.expectEqual(@as(u32, 1), value.process.result.code);
    }
}

test "TypeScript and native message contracts are identical" {
    const allocator = std.testing.allocator;
    const fixture = try parseControlFixture(allocator);
    defer fixture.deinit();

    var seen = [_]bool{false} ** 65_536;
    for (fixture.value.message_contracts) |expected| {
        try std.testing.expect(!seen[expected.kind]);
        seen[expected.kind] = true;
        const actual = protocol.messageContract(expected.kind) orelse
            return error.TestUnexpectedResult;
        try std.testing.expectEqualStrings(expected.name, actual.name);
        try expectOptionalPayloadEncoding(expected.event_encoding, actual.event_encoding);
        try expectOptionalPayloadEncoding(expected.response_encoding, actual.response_encoding);
        if (expected.request) |expected_request| {
            const actual_request = actual.request orelse return error.TestUnexpectedResult;
            try std.testing.expectEqualStrings(
                expected_request.payload_encoding,
                payloadEncodingName(actual_request.payload_encoding),
            );
            try std.testing.expectEqual(expected_request.success_kind, actual_request.success_kind);
        } else {
            try std.testing.expect(actual.request == null);
        }
    }
    for (0..65_536) |value| {
        const kind: u16 = @intCast(value);
        try std.testing.expectEqual(seen[value], protocol.isKnownMessageKind(kind));
    }
}

test "TypeScript and native protocol limits are identical" {
    const fixture = try parseControlFixture(std.testing.allocator);
    defer fixture.deinit();

    inline for (@typeInfo(ProtocolLimitsFixture).@"struct".fields) |field| {
        const expected = @field(fixture.value.protocol_limits, field.name);
        const actual: u64 = @intCast(@field(protocol.Limits, field.name));
        try std.testing.expectEqual(expected, actual);
    }
}

test "acknowledged input bytes equal the input bound while the raw payload reserves its authority header" {
    try std.testing.expectEqual(protocol.Limits.input_bytes, protocol.Limits.input_accepted_bytes);
    try std.testing.expectEqual(protocol.Limits.input_bytes + 24, protocol.Limits.write_input_payload_bytes);
}

test "every TypeScript control command fixture crosses the native typed decoder" {
    const allocator = std.testing.allocator;
    const fixture = try parseControlFixture(allocator);
    defer fixture.deinit();
    const commands = fixture.value.commands;

    {
        var decoded = try control_decode.decodeApplyTerminalTheme(
            allocator,
            commands.apply_terminal_theme,
        );
        defer decoded.deinit();
        try std.testing.expectEqual(@as(u64, 0), decoded.value.expected_theme_revision);
        try std.testing.expectEqual(@as(u32, 0x10_10_10), decoded.value.terminal.default_background_rgb);
        try std.testing.expectEqual(@as(u32, 0xff_ff_ff), decoded.value.terminal.palette[255]);
    }
    {
        var decoded = try control_decode.decodeEmpty(allocator, commands.empty);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeHello(allocator, commands.hello);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeListTerminals(allocator, commands.list_terminals);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeCreateTerminal(allocator, commands.create_terminal);
        defer decoded.deinit();
        try std.testing.expect(decoded.value.launch.launch.geometry != null);
        try std.testing.expect(decoded.value.launch.launch.profile.default_cursor_blinking);
        try std.testing.expectEqual(
            @as(@TypeOf(decoded.value.launch.launch.profile.default_cursor_shape), .underline),
            decoded.value.launch.launch.profile.default_cursor_shape,
        );
    }
    {
        var decoded = try control_decode.decodeCreateTerminal(
            allocator,
            commands.create_terminal_deferred,
        );
        defer decoded.deinit();
        try std.testing.expect(decoded.value.launch.launch.geometry == null);
    }
    {
        var decoded = try control_decode.decodeStartTerminal(allocator, commands.start_terminal);
        defer decoded.deinit();
        try std.testing.expectEqual(@as(u16, 96), decoded.value.geometry.columns);
        try std.testing.expectEqual(@as(u16, 32), decoded.value.geometry.rows);
    }
    {
        var decoded = try control_decode.decodeSetTerminalDefaults(
            allocator,
            commands.set_terminal_defaults,
        );
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeUpdateTerminalConfiguration(
            allocator,
            commands.update_terminal_configuration,
        );
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeTerminalTarget(allocator, commands.terminal_target);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeAttachTerminal(allocator, commands.attach_terminal);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeClaimResizeLease(
            allocator,
            commands.claim_resize_lease,
        );
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeDetachTerminal(allocator, commands.detach_terminal);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeUpdateAttachmentInputCapability(
            allocator,
            commands.update_attachment_input_capability,
        );
        defer decoded.deinit();
        try std.testing.expectEqual(@as(u64, 2), decoded.value.capability_revision);
        try std.testing.expect(!decoded.value.input);
    }
    {
        var decoded = try control_decode.decodeResizeTerminal(allocator, commands.resize_terminal);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeTerminalControl(allocator, commands.clear_terminal);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeTerminalControl(allocator, commands.reset_terminal);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeFrameApplied(allocator, commands.frame_applied);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 8), commands.need_snapshot.len);
    for (commands.need_snapshot) |payload| {
        var decoded = try control_decode.decodeNeedSnapshot(allocator, payload);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 2), commands.projection_demand.len);
    for (commands.projection_demand) |payload| {
        var decoded = try control_decode.decodeProjectionDemand(allocator, payload);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 4), commands.read_rows.len);
    for (commands.read_rows) |payload| {
        var decoded = try control_decode.decodeReadRows(allocator, payload);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeRowsApplied(allocator, commands.rows_applied);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 2), commands.history_demand.len);
    for (commands.history_demand) |payload| {
        var decoded = try control_decode.decodeHistoryDemand(allocator, payload);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeHistoryObserved(
            allocator,
            commands.history_observed,
        );
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 4), commands.selection_gesture.len);
    for (commands.selection_gesture) |payload| {
        var decoded = try control_decode.decodeSelectionGesture(allocator, payload);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 4), commands.selection_command.len);
    for (commands.selection_command) |payload| {
        var decoded = try control_decode.decodeSelectionCommand(allocator, payload);
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 1), commands.selection_export.len);
    for (commands.selection_export) |payload| {
        var decoded = try control_decode.decodeSelectionExport(allocator, payload);
        defer decoded.deinit();
    }
    {
        var decoded = try control_decode.decodeSelectionExportChunkApplied(
            allocator,
            commands.selection_export_chunk_applied,
        );
        defer decoded.deinit();
    }
    try std.testing.expectEqual(@as(usize, 4), commands.selection_export_cancel.len);
    for (commands.selection_export_cancel) |payload| {
        var decoded = try control_decode.decodeSelectionExportCancel(allocator, payload);
        defer decoded.deinit();
    }
}

test "TypeScript raw control cases cross strict native admission" {
    const allocator = std.testing.allocator;
    const fixture = try parseControlFixture(allocator);
    defer fixture.deinit();
    try std.testing.expectEqual(@as(usize, 15), fixture.value.raw_cases.len);

    for (fixture.value.raw_cases) |case| {
        const payload_length = try std.base64.standard.Decoder.calcSizeForSlice(
            case.payload_base64,
        );
        const payload = try allocator.alloc(u8, payload_length);
        defer allocator.free(payload);
        try std.base64.standard.Decoder.decode(payload, case.payload_base64);

        if (std.mem.eql(u8, case.decoder, "empty")) {
            try expectRawControlCase(
                control_decode.decodeEmpty,
                noObservedInteger,
                case,
                payload,
            );
        } else if (std.mem.eql(u8, case.decoder, "list-terminals")) {
            try expectRawControlCase(
                control_decode.decodeListTerminals,
                listTerminalsObservedInteger,
                case,
                payload,
            );
        } else if (std.mem.eql(u8, case.decoder, "set-terminal-defaults")) {
            try expectRawControlCase(
                control_decode.decodeSetTerminalDefaults,
                defaultsObservedInteger,
                case,
                payload,
            );
        } else if (std.mem.eql(u8, case.decoder, "selection-export")) {
            try expectRawControlCase(
                control_decode.decodeSelectionExport,
                noObservedInteger,
                case,
                payload,
            );
        } else if (std.mem.eql(u8, case.decoder, "terminal-target")) {
            try expectRawControlCase(
                control_decode.decodeTerminalTarget,
                noObservedInteger,
                case,
                payload,
            );
        } else {
            std.debug.print("unknown raw control decoder in {s}: {s}\n", .{
                case.name,
                case.decoder,
            });
            return error.TestUnexpectedResult;
        }
    }
}

test "native control replies and events conform to TypeScript schemas" {
    const allocator = std.testing.allocator;
    const fixture = try parseControlFixture(allocator);
    defer fixture.deinit();
    const outputs = fixture.value.outputs;
    const host_epoch = TestIdentifier{ .bytes = [_]u8{0x11} ** 16 };
    const attachment_id = TestIdentifier{ .bytes = [_]u8{0x22} ** 16 };
    const creation_id = TestIdentifier{ .bytes = [_]u8{0x33} ** 16 };
    const terminal_id = TestIdentifier{ .bytes = [_]u8{0x44} ** 16 };
    const theme_operation_id = TestIdentifier{ .bytes = [_]u8{0x66} ** 16 };
    const other_theme_operation_id = TestIdentifier{ .bytes = [_]u8{0x77} ** 16 };

    try expectJsonEquivalent(allocator, outputs.empty, control_encode.empty_control);
    try expectOwnedJson(allocator, outputs.protocol_rejected, try control_encode.protocolRejected(
        allocator,
        0,
        1,
        0,
        protocol.current_protocol_major,
        protocol.current_protocol_minor,
        protocol.current_protocol_patch,
    ));
    try expectOwnedJson(allocator, outputs.hello_accepted, try control_encode.helloAccepted(
        allocator,
        .{
            .frame_bytes = 8 * 1024 * 1024,
            .input_bytes = 1024 * 1024,
            .row_window_bytes = 44 * 1024 * 1024,
        },
        host_epoch,
        "0.1.0",
    ));
    const typed_failures = [_]struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    }{
        .{ .code = "invalid-state", .message = "The command is not legal now.", .retryable = false },
        .{ .code = "backpressure", .message = "Try again later.", .retryable = true },
    };
    try std.testing.expectEqual(typed_failures.len, outputs.typed_failure.len);
    for (typed_failures, outputs.typed_failure) |failure, expected| {
        try expectOwnedJson(allocator, expected, try control_encode.typedFailure(
            allocator,
            failure.code,
            failure.message,
            failure.retryable,
        ));
    }

    const identity_json = try control_encode.terminalIdentity(
        allocator,
        creation_id,
        host_epoch,
        terminal_id,
    );
    defer allocator.free(identity_json);
    try expectJsonEquivalent(allocator, outputs.terminal_identity, identity_json);
    try expectJsonEquivalent(allocator, outputs.terminal_started, identity_json);
    const start_outcomes = [_]TestStartOutcome{
        .{
            .geometry = .{
                .cell_pixel_height = 18,
                .cell_pixel_width = 9,
                .columns = 96,
                .rows = 32,
            },
            .identity = .{ .creation_id = creation_id, .terminal_id = terminal_id },
            .transition = .started,
        },
        .{
            .geometry = .{
                .cell_pixel_height = 18,
                .cell_pixel_width = 9,
                .columns = 96,
                .rows = 32,
            },
            .identity = .{ .creation_id = creation_id, .terminal_id = terminal_id },
            .transition = .already_started,
        },
    };
    try std.testing.expectEqual(start_outcomes.len, outputs.terminal_start_accepted.len);
    for (start_outcomes, outputs.terminal_start_accepted) |outcome, expected| {
        try expectOwnedJson(
            allocator,
            expected,
            try control_encode.terminalStartAccepted(allocator, host_epoch, outcome),
        );
    }
    try expectOwnedJson(
        allocator,
        outputs.terminal_acknowledged,
        try control_encode.terminalAcknowledged(allocator, terminal_id),
    );
    try expectOwnedJson(
        allocator,
        outputs.terminal_defaults_accepted,
        try control_encode.terminalDefaultsAccepted(allocator, 25_000_000, 3),
    );
    try expectOwnedJson(
        allocator,
        outputs.terminal_configuration_accepted,
        try control_encode.terminalConfigurationAccepted(allocator, .{
            .configuration = .{ .scrollback_bytes = 25_000_000 },
            .configuration_revision = 3,
            .generation = 4,
            .history_rows_evicted = 5,
            .revision = 43,
            .terminal_id = terminal_id,
        }),
    );
    const theme_results = [_]struct {
        current: TestTerminalThemeState,
        operation_id: TestIdentifier,
        terminal_id: TestIdentifier,
        transition: enum { applied, already_applied, stale },
    }{
        .{
            .current = .{ .operation_id = theme_operation_id, .revision = 1 },
            .operation_id = theme_operation_id,
            .terminal_id = terminal_id,
            .transition = .applied,
        },
        .{
            .current = .{ .operation_id = theme_operation_id, .revision = 1 },
            .operation_id = theme_operation_id,
            .terminal_id = terminal_id,
            .transition = .already_applied,
        },
        .{
            .current = .{ .operation_id = other_theme_operation_id, .revision = 2 },
            .operation_id = theme_operation_id,
            .terminal_id = terminal_id,
            .transition = .stale,
        },
    };
    try std.testing.expectEqual(theme_results.len, outputs.terminal_theme_result.len);
    for (theme_results, outputs.terminal_theme_result) |result, expected| {
        try expectOwnedJson(
            allocator,
            expected,
            try control_encode.terminalThemeResult(allocator, result),
        );
    }
    try expectOwnedJson(
        allocator,
        outputs.attachment_capabilities,
        try control_encode.attachmentCapabilities(allocator, .{
            .attachment_id = attachment_id,
            .capability_revision = 2,
            .capabilities = .{ .input = false, .read = false, .resize = false },
            .resize_lease = false,
            .terminal_id = terminal_id,
        }),
    );
    try expectOwnedJson(
        allocator,
        outputs.resize_lease_changed,
        try control_encode.resizeLeaseChanged(allocator, .{
            .attachment_id = attachment_id,
            .resize_lease = true,
            .terminal_id = terminal_id,
        }),
    );
    try expectOwnedJson(
        allocator,
        outputs.attachment_acknowledged,
        try control_encode.attachmentAcknowledged(allocator, attachment_id),
    );
    try expectOwnedJson(
        allocator,
        outputs.input_accepted,
        try control_encode.inputAccepted(allocator, .{
            .attachment_id = attachment_id,
            .byte_count = 12,
            .capability_revision = 1,
        }),
    );
    try expectOwnedJson(
        allocator,
        outputs.resize_accepted,
        try control_encode.resizeAccepted(allocator, .{
            .attachment_id = attachment_id,
            .geometry = .{
                .cell_pixel_height = 16,
                .cell_pixel_width = 8,
                .columns = 100,
                .rows = 30,
            },
            .revision = 43,
        }),
    );
    const control_operations = [_]TestTerminalControlOperation{
        .clear,
        .reset,
    };
    try std.testing.expectEqual(control_operations.len, outputs.terminal_control_accepted.len);
    for (control_operations, outputs.terminal_control_accepted, 0..) |operation, expected, index| {
        var writer: std.Io.Writer.Allocating = .init(allocator);
        defer writer.deinit();
        try control_encode.writeTerminalControlAccepted(&writer.writer, .{
            .attachment_id = attachment_id,
            .generation = 4 + index,
            .operation = operation,
            .revision = 43 + index,
            .terminal_id = terminal_id,
        });
        try expectOwnedJson(allocator, expected, try writer.toOwnedSlice());
    }

    const projection_demands = [_]TestProjectionDemand{ .active, .paused };
    try std.testing.expectEqual(projection_demands.len, outputs.projection_demand_accepted.len);
    for (projection_demands, outputs.projection_demand_accepted) |demand, expected| {
        try expectOwnedJson(allocator, expected, try control_encode.projectionDemandAccepted(
            allocator,
            attachment_id,
            demand,
        ));
    }
    try expectOwnedJson(
        allocator,
        outputs.frame_applied,
        try encodeWithWriter(allocator, control_encode.writeFrameApplied, .{
            .attachment_id = attachment_id,
            .generation = 3,
            .revision = 42,
            .sequence = 7,
        }),
    );
    const snapshot_reasons = [_]TestSnapshotReason{
        .activity_stale,
        .base_mismatch,
        .decoder_rejected,
        .dimensions_mismatch,
        .generation_mismatch,
        .identity_mismatch,
        .sequence_gap,
        .store_reset,
    };
    try std.testing.expectEqual(snapshot_reasons.len, outputs.need_snapshot.len);
    for (snapshot_reasons, outputs.need_snapshot, 1..) |reason, expected, sequence| {
        try expectOwnedJson(
            allocator,
            expected,
            try encodeWithWriter(allocator, control_encode.writeNeedSnapshot, .{
                .attachment_id = attachment_id,
                .reason = reason,
                .sequence = sequence,
            }),
        );
    }
    try expectOwnedJson(
        allocator,
        outputs.rows_applied,
        try encodeWithWriter(allocator, control_encode.writeRowsApplied, .{
            .attachment_id = attachment_id,
            .generation = 3,
            .request_sequence = 8,
            .revision = 42,
        }),
    );
    const history_demands = [_]TestHistoryDemand{ .following_bottom, .viewing_history };
    try std.testing.expectEqual(history_demands.len, outputs.history_demand_accepted.len);
    for (history_demands, outputs.history_demand_accepted) |demand, expected| {
        try expectOwnedJson(
            allocator,
            expected,
            try encodeWithWriter(allocator, control_encode.writeHistoryDemandAccepted, .{
                .attachment_id = attachment_id,
                .demand = demand,
            }),
        );
    }
    try expectOwnedJson(
        allocator,
        outputs.history_observed,
        try encodeWithWriter(allocator, control_encode.writeHistoryObserved, .{
            .attachment_id = attachment_id,
            .generation = 3,
            .revision = 42,
            .sequence = 5,
        }),
    );
    try expectOwnedJson(
        allocator,
        outputs.selection_export_chunk_applied,
        try encodeWithWriter(allocator, control_encode.writeSelectionExportChunkApplied, .{
            .attachment_id = attachment_id,
            .chunk_sequence = 2,
            .export_sequence = 9,
        }),
    );

    const projection_blocks = [_]TestProjectionBlock{
        .capture_allocation_failed,
        .projection_too_large,
    };
    try std.testing.expectEqual(projection_blocks.len, outputs.projection_blocked.len);
    for (projection_blocks, outputs.projection_blocked) |reason, expected| {
        try expectOwnedJson(allocator, expected, try control_encode.projectionBlocked(
            allocator,
            attachment_id,
            reason,
            42,
        ));
    }
    const anchor_statuses = [_]TestAnchorStatus{ .evicted, .none, .valid };
    try std.testing.expectEqual(anchor_statuses.len, outputs.history_changed.len);
    for (anchor_statuses, 0..) |anchor_status, index| {
        try expectOwnedJson(allocator, outputs.history_changed[index], try control_encode.historyChanged(
            allocator,
            .{
                .anchor_status = anchor_status,
                .attachment_id = attachment_id,
                .generation = 3,
                .revision = 42,
                .sequence = index + 1,
                .total_rows = 125,
            },
        ));
    }
    try expectOwnedJson(
        allocator,
        outputs.terminal_bell,
        try control_encode.terminalBell(allocator, attachment_id, 6),
    );
    try expectOwnedJson(
        allocator,
        outputs.terminal_clipboard_write_request,
        try control_encode.terminalClipboardWriteRequest(
            allocator,
            attachment_id,
            .{ .sequence = 7, .text = "copied from terminal" },
        ),
    );
    const selection_snapshots = [_]TestSelectionSnapshot{
        .none,
        .evicted,
        .{ .active = .{
            .end = .{ .column = 7, .row = 102 },
            .rectangle = true,
            .start = .{ .column = 4, .row = 100 },
        } },
    };
    try std.testing.expectEqual(selection_snapshots.len, outputs.selection_changed.len);
    for (selection_snapshots, outputs.selection_changed) |snapshot, expected| {
        try expectOwnedJson(allocator, expected, try control_encode.selectionChanged(
            allocator,
            attachment_id,
            snapshot,
        ));
    }
    const formats = [_]TestExportFormat{.plain};
    try std.testing.expectEqual(formats.len, outputs.selection_export_started.len);
    for (formats, outputs.selection_export_started, 0..) |format, expected, index| {
        try expectOwnedJson(allocator, expected, try control_encode.selectionExportStarted(
            allocator,
            attachment_id,
            .{
                .byte_length = 10 + index,
                .export_sequence = index + 1,
                .format = format,
            },
        ));
    }
    const statuses = [_]TestExportStatus{ .cancelled, .completed };
    try std.testing.expectEqual(statuses.len, outputs.selection_export_finished.len);
    for (statuses, outputs.selection_export_finished) |status, expected| {
        try expectOwnedJson(allocator, expected, try control_encode.selectionExportFinished(
            allocator,
            .{ .attachment_id = attachment_id, .byte_length = 12, .export_sequence = 9 },
            status,
        ));
    }
}

test "native lifecycle variants conform to TypeScript discriminated unions" {
    const allocator = std.testing.allocator;
    const fixture = try parseControlFixture(allocator);
    defer fixture.deinit();
    const outputs = fixture.value.outputs;
    const host_epoch = TestIdentifier{ .bytes = [_]u8{0x11} ** 16 };
    const creation_id = TestIdentifier{ .bytes = [_]u8{0x33} ** 16 };
    const terminal_id = TestIdentifier{ .bytes = [_]u8{0x44} ** 16 };
    const read_failure: TestRuntimeFailure = .{
        .kind = .pty_read,
        .stage = .read_output,
    };
    const resize_failure: TestRuntimeFailure = .{
        .kind = .resize,
        .stage = .apply_resize,
    };
    const cleanup_failure: TestRuntimeFailure = .{
        .kind = .pty_read,
        .stage = .release_output,
    };
    const observation_failure: TestRuntimeFailure = .{
        .kind = .process_observation,
        .stage = .observe_process,
    };
    const terminals = [_]TestTerminal{
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .columns = null,
            .rows = null,
            .revision = 0,
            .phase_value = .awaiting_start,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .running,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .finishing,
            .exit_result = .{ .unknown = 7 },
            .terminate_requested = true,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .containing_failure,
            .failure = read_failure,
            .containment = .{ .stopping = .{ .process = null } },
            .terminate_requested = true,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .containing_failure,
            .failure = resize_failure,
            .containment = .{ .blocked = .{
                .cleanup_failure = cleanup_failure,
                .process = .{ .observed = .{ .exit_code = 1 } },
            } },
            .terminate_requested = true,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .exited,
            .exit_result = .{ .signal = 15 },
            .terminate_requested = true,
        },
        .{
            .creation_id = creation_id,
            .terminal_id = terminal_id,
            .phase_value = .failed,
            .failure = .{
                .kind = .worker_integrity,
                .stage = .synchronize_worker,
            },
            .process = .{ .unavailable = observation_failure },
            .terminate_requested = true,
        },
    };
    try expectOwnedJson(allocator, outputs.terminal_list, try control_encode.terminalList(
        allocator,
        host_epoch,
        9,
        &terminals,
        0,
        terminals.len,
    ));

    const exits = [_]TestExitNotice{
        .{
            .creation_id = creation_id,
            .final_revision = 42,
            .result = .{ .exit_code = 0 },
            .terminal_id = terminal_id,
        },
        .{
            .creation_id = creation_id,
            .final_revision = 42,
            .result = .{ .signal = 15 },
            .terminal_id = terminal_id,
        },
        .{
            .creation_id = creation_id,
            .final_revision = 42,
            .result = .{ .unknown = 7 },
            .terminal_id = terminal_id,
        },
    };
    try std.testing.expectEqual(exits.len, outputs.terminal_exited.len);
    for (exits, outputs.terminal_exited) |notice, expected| {
        try expectOwnedJson(
            allocator,
            expected,
            try control_encode.terminalExited(allocator, host_epoch, notice),
        );
    }

    const failure_pairs = [_]struct {
        kind: TestRuntimeFailureKind,
        stage: TestRuntimeFailureStage,
    }{
        .{ .kind = .process_control, .stage = .dispatch_termination },
        .{ .kind = .process_observation, .stage = .observe_process },
        .{ .kind = .pty_read, .stage = .read_output },
        .{ .kind = .pty_write, .stage = .write_input },
        .{ .kind = .resize, .stage = .apply_resize },
        .{ .kind = .terminal_reply, .stage = .admit_terminal_reply },
        .{ .kind = .worker_integrity, .stage = .synchronize_worker },
    };
    try std.testing.expectEqual(failure_pairs.len, outputs.terminal_failed.len);
    for (failure_pairs, outputs.terminal_failed, 0..) |pair, expected, index| {
        const process: TestProcessObservation = if (index == 1)
            .{ .unavailable = observation_failure }
        else switch (index % 3) {
            0 => .{ .observed = .{ .exit_code = 1 } },
            1 => .{ .observed = .{ .signal = 15 } },
            else => .{ .observed = .{ .unknown = 7 } },
        };
        const notice: TestFailureNotice = .{
            .creation_id = creation_id,
            .failure = .{
                .kind = pair.kind,
                .stage = pair.stage,
            },
            .final_revision = 42 + index,
            .process = process,
            .terminal_id = terminal_id,
        };
        try expectOwnedJson(
            allocator,
            expected,
            try control_encode.terminalFailed(allocator, host_epoch, notice),
        );
    }
}

test "canonical TypeScript WriteInput fixture conforms in Zig" {
    const summary = try protocol.validateWriteInput(input_fixture);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &summary.attachment_id);
    try std.testing.expectEqual(@as(u64, 7), summary.capability_revision);
    try std.testing.expectEqualSlices(u8, &.{ 0, 0x1b, 0xf0, 0x9f, 0x90, 0x8d }, summary.bytes);
}

test "canonical TypeScript semantic-input fixtures conform in Zig" {
    const decoded_key = try protocol.validateSemanticInput(
        protocol.MessageKind.key_input,
        key_input_fixture,
    );
    try std.testing.expectEqualSlices(u8, &([_]u8{0x22} ** 16), &decoded_key.attachment_id);
    try std.testing.expectEqual(@as(u64, 7), decoded_key.capability_revision);
    const key = switch (decoded_key.input) {
        .key => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expectEqual(protocol.KeyAction.press, key.action);
    try std.testing.expect(key.modifiers.shift);
    try std.testing.expect(key.modifiers.shift_right);
    try std.testing.expect(key.consumed_modifiers.shift);
    try std.testing.expect(!key.consumed_modifiers.shift_right);
    try std.testing.expectEqualStrings("KeyA", key.key);
    try std.testing.expectEqualStrings("A", key.text);
    try std.testing.expect(key.option_as_alt);
    try std.testing.expectEqual(@as(u21, 'a'), key.unshifted_codepoint);

    const decoded_text = try protocol.validateSemanticInput(
        protocol.MessageKind.text_input,
        text_input_fixture,
    );
    const text = switch (decoded_text.input) {
        .text => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expectEqualStrings("中文", text.text);
    const decoded_paste = try protocol.validateSemanticInput(
        protocol.MessageKind.paste_input,
        paste_input_fixture,
    );
    const paste = switch (decoded_paste.input) {
        .paste => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expect(paste.bracketed);
    try std.testing.expectEqualStrings("line 1\nline 2", paste.text);

    const decoded_mouse = try protocol.validateSemanticInput(
        protocol.MessageKind.mouse_input,
        mouse_input_fixture,
    );
    const mouse = switch (decoded_mouse.input) {
        .mouse => |value| value,
        else => return error.TestUnexpectedResult,
    };
    const motion = switch (mouse.action) {
        .motion => |value| value,
        else => return error.TestUnexpectedResult,
    };
    const motion_button = switch (motion) {
        .hover => return error.TestUnexpectedResult,
        .drag => |button| button,
    };
    try std.testing.expectEqual(protocol.MouseButton.left, motion_button);
    try std.testing.expect(mouse.modifiers.alt);
    try std.testing.expect(mouse.pressed_buttons.contains(.left));
    try std.testing.expectEqual(@as(f32, 10.5), mouse.x);
    try std.testing.expectEqual(@as(f32, -2.25), mouse.y);

    const decoded_focus = try protocol.validateSemanticInput(
        protocol.MessageKind.focus_input,
        focus_input_fixture,
    );
    const focus = switch (decoded_focus.input) {
        .focus => |value| value,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expect(focus.focused);

    const invalid_key = try std.testing.allocator.dupe(u8, key_input_fixture);
    defer std.testing.allocator.free(invalid_key);
    const key_offset = std.mem.indexOf(u8, invalid_key, "KeyA") orelse return error.TestUnexpectedResult;
    invalid_key[key_offset + 3] = '0';
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(invalid_key));
}

test "TypeScript UTF-8 BOM input cases cross native admission" {
    const allocator = std.testing.allocator;
    const fixture = try parseUtf8BomFixture(allocator);
    defer fixture.deinit();
    try std.testing.expectEqual(@as(usize, 3), fixture.value.cases.len);

    for (fixture.value.cases) |case| {
        const kind = if (std.mem.eql(u8, case.decoder, "key-input"))
            protocol.MessageKind.key_input
        else if (std.mem.eql(u8, case.decoder, "text-input"))
            protocol.MessageKind.text_input
        else if (std.mem.eql(u8, case.decoder, "paste-input"))
            protocol.MessageKind.paste_input
        else {
            std.debug.print("unknown UTF-8 BOM input decoder in {s}: {s}\n", .{
                case.name,
                case.decoder,
            });
            return error.TestUnexpectedResult;
        };
        const payload_length = try std.base64.standard.Decoder.calcSizeForSlice(
            case.payload_base64,
        );
        const payload = try allocator.alloc(u8, payload_length);
        defer allocator.free(payload);
        try std.base64.standard.Decoder.decode(payload, case.payload_base64);

        const decoded = protocol.validateSemanticInput(kind, payload) catch |err| {
            const expected_code = case.expected_code orelse {
                std.debug.print("UTF-8 BOM input case unexpectedly failed: {s}: {s}\n", .{
                    case.name,
                    @errorName(err),
                });
                return err;
            };
            try std.testing.expectEqualStrings(expected_code, protocolErrorCode(err));
            try std.testing.expect(case.expected_text_base64 == null);
            continue;
        };

        if (case.expected_code) |expected_code| {
            std.debug.print("UTF-8 BOM input case unexpectedly accepted: {s}: {s}\n", .{
                case.name,
                expected_code,
            });
            return error.TestExpectedError;
        }
        const expected_text_base64 = case.expected_text_base64 orelse
            return error.TestUnexpectedResult;
        const expected_text_length = try std.base64.standard.Decoder.calcSizeForSlice(
            expected_text_base64,
        );
        const expected_text = try allocator.alloc(u8, expected_text_length);
        defer allocator.free(expected_text);
        try std.base64.standard.Decoder.decode(expected_text, expected_text_base64);
        const actual_text = switch (decoded.input) {
            .text => |value| value.text,
            .paste => |value| value.text,
            else => return error.TestUnexpectedResult,
        };
        try std.testing.expectEqualSlices(u8, expected_text, actual_text);
    }
}

test "native semantic-input decoder rejects non-canonical intent with matching errors" {
    const allocator = std.testing.allocator;

    const zero_identifier = try allocator.dupe(u8, input_fixture);
    defer allocator.free(zero_identifier);
    @memset(zero_identifier[0..16], 0);
    try std.testing.expectError(error.InvalidIdentifier, protocol.validateWriteInput(zero_identifier));

    const invalid_action = try allocator.dupe(u8, key_input_fixture);
    defer allocator.free(invalid_action);
    invalid_action[24] = 3;
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(invalid_action));

    const unknown_key_flag = try allocator.dupe(u8, key_input_fixture);
    defer allocator.free(unknown_key_flag);
    unknown_key_flag[25] |= 1 << 2;
    try std.testing.expectError(error.UnknownRequiredFlag, protocol.validateKeyInput(unknown_key_flag));

    const invalid_key_utf8 = try allocator.dupe(u8, key_input_fixture);
    defer allocator.free(invalid_key_utf8);
    invalid_key_utf8[40] = 0xff;
    try std.testing.expectError(error.InvalidUtf8, protocol.validateKeyInput(invalid_key_utf8));

    const side_without_base = try allocator.dupe(u8, key_input_fixture);
    defer allocator.free(side_without_base);
    std.mem.writeInt(u16, side_without_base[26..28], 1 << 6, .little);
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(side_without_base));

    const consumed_but_inactive = try allocator.dupe(u8, key_input_fixture);
    defer allocator.free(consumed_but_inactive);
    std.mem.writeInt(u16, consumed_but_inactive[28..30], 1 << 1, .little);
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(consumed_but_inactive));

    const missing_mouse_button = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(missing_mouse_button);
    missing_mouse_button[24] = 0;
    missing_mouse_button[25] = 0;
    try std.testing.expectError(error.InvalidField, protocol.validateMouseInput(missing_mouse_button));

    const missing_motion_button = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(missing_motion_button);
    missing_motion_button[25] = 0;
    try std.testing.expectError(error.InvalidField, protocol.validateMouseInput(missing_motion_button));

    const inactive_motion_button = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(inactive_motion_button);
    std.mem.writeInt(u16, inactive_motion_button[36..38], 0, .little);
    try std.testing.expectError(error.InvalidField, protocol.validateMouseInput(inactive_motion_button));

    const wheel_press = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(wheel_press);
    wheel_press[24] = 0;
    wheel_press[25] = 4;
    std.mem.writeInt(u16, wheel_press[36..38], 0, .little);
    const decoded_wheel = try protocol.validateMouseInput(wheel_press);
    const wheel = switch (decoded_wheel.input) {
        .mouse => |value| value,
        else => return error.TestUnexpectedResult,
    };
    const wheel_button = switch (wheel.action) {
        .press => |button| button,
        else => return error.TestUnexpectedResult,
    };
    try std.testing.expectEqual(protocol.MouseButton.four, wheel_button);
}

fn buildKeyInputPayload(allocator: std.mem.Allocator, key: []const u8) ![]u8 {
    const bytes = try allocator.alloc(u8, 40 + key.len);
    @memset(bytes, 0);
    @memset(bytes[0..16], 0x01); // non-zero attachment id
    std.mem.writeInt(u64, bytes[16..24], 1, .little); // capability revision
    bytes[24] = 1; // action: press
    bytes[25] = 0; // flags: not composing
    std.mem.writeInt(u16, bytes[30..32], @intCast(key.len), .little);
    @memcpy(bytes[40..][0..key.len], key);
    return bytes;
}

fn buildTextInputPayload(allocator: std.mem.Allocator, text_length: u32, body: []const u8) ![]u8 {
    const bytes = try allocator.alloc(u8, 28 + body.len);
    @memset(bytes, 0);
    @memset(bytes[0..16], 0x22); // non-zero attachment id
    std.mem.writeInt(u64, bytes[16..24], 1, .little); // capability revision
    std.mem.writeInt(u32, bytes[24..28], text_length, .little);
    @memcpy(bytes[28..], body);
    return bytes;
}

test "native key decoder rejects the non-contract F+1 function code" {
    const allocator = std.testing.allocator;
    const key = try buildKeyInputPayload(allocator, "F+1");
    defer allocator.free(key);
    // The reachable 43-byte payload: attachment 0x01 x16, revision, press, zero flags,
    // modifiers, consumed modifiers, codepoint and text length, key length 3,
    // and the bytes 0x46 0x2b 0x31 ("F+1") as the function key.
    try std.testing.expectEqual(@as(usize, 43), key.len);
    try std.testing.expectEqualSlices(u8, &([_]u8{0x01} ** 16), key[0..16]);
    try std.testing.expectEqualSlices(u8, "F+1", key[40..]);
    try std.testing.expectError(error.InvalidField, protocol.validateKeyInput(key));
}

test "native key decoder still admits contract function codes F1..F25" {
    const allocator = std.testing.allocator;
    for ([_][]const u8{ "F1", "F2", "F9", "F10", "F25" }) |key_name| {
        const key = try buildKeyInputPayload(allocator, key_name);
        defer allocator.free(key);
        const decoded = try protocol.validateKeyInput(key);
        const value = switch (decoded.input) {
            .key => |value| value,
            else => return error.TestUnexpectedResult,
        };
        try std.testing.expectEqualStrings(key_name, value.key);
    }
}

test "native text decoder checks declared length before rejecting empty text" {
    const allocator = std.testing.allocator;
    // Reachable corruption: a valid nonempty TextInput with its little-endian
    // declared length (bytes 24..28) zeroed while the body is retained.
    const zero_length_text = try allocator.dupe(u8, text_input_fixture);
    defer allocator.free(zero_length_text);
    std.mem.writeInt(u32, zero_length_text[24..28], 0, .little);
    try std.testing.expectError(
        error.LengthMismatch,
        protocol.validateCommittedTextInput(zero_length_text),
    );
    const zero_length_paste = try allocator.dupe(u8, paste_input_fixture);
    defer allocator.free(zero_length_paste);
    std.mem.writeInt(u32, zero_length_paste[28..32], 0, .little);
    try std.testing.expectError(
        error.LengthMismatch,
        protocol.validatePasteInput(zero_length_paste),
    );

    // A header-only payload with a truly empty declared text is an invalid
    // field, matching the TypeScript min-length rejection of empty text.
    const empty_payload = try buildTextInputPayload(allocator, 0, "");
    defer allocator.free(empty_payload);
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateCommittedTextInput(empty_payload),
    );
}

test "native mouse decoder classifies pressed-buttons overflow as an invalid field" {
    const allocator = std.testing.allocator;
    const mouse = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(mouse);
    // pressed_buttons is the u16 at bytes 36..38; setting byte 37 bit 3 adds
    // pressed-buttons bit 11 (0x0800) to the fixture's pressed left button.
    mouse[37] |= 0x08;
    const pressed = std.mem.readInt(u16, mouse[36..38], .little);
    try std.testing.expect(pressed & 0x0800 != 0);
    try std.testing.expectError(error.InvalidField, protocol.validateMouseInput(mouse));
}

test "native mouse decoder retains unknown-required-flag for reserved and modifier bits" {
    const allocator = std.testing.allocator;
    const reserved = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(reserved);
    std.mem.writeInt(u16, reserved[38..40], 1, .little);
    try std.testing.expectError(error.UnknownRequiredFlag, protocol.validateMouseInput(reserved));

    const unknown_modifier = try allocator.dupe(u8, mouse_input_fixture);
    defer allocator.free(unknown_modifier);
    std.mem.writeInt(u16, unknown_modifier[26..28], 1 << 12, .little);
    try std.testing.expectError(error.UnknownRequiredFlag, protocol.validateMouseInput(unknown_modifier));
}

test "native decoder rejects malformed magic and unknown required extension" {
    const allocator = std.testing.allocator;
    const invalid_magic = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(invalid_magic);
    invalid_magic[0] = 0;
    try std.testing.expectError(
        error.InvalidMagic,
        protocol.validateTerminalFrame(allocator, invalid_magic),
    );

    const required_extension = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(required_extension);
    std.mem.writeInt(
        u16,
        required_extension[required_extension.len - 10 ..][0..2],
        0x8000,
        .little,
    );
    try std.testing.expectError(
        error.UnknownRequiredFlag,
        protocol.validateTerminalFrame(allocator, required_extension),
    );
}

test "native envelope decoder matches required-message and payload bounds" {
    const allocator = std.testing.allocator;
    const unknown_required = try allocator.dupe(u8, envelope_fixture);
    defer allocator.free(unknown_required);
    std.mem.writeInt(u16, unknown_required[20..22], 60_000, .little);
    try std.testing.expectError(
        error.UnknownRequiredMessage,
        protocol.validateTransportEnvelope(unknown_required),
    );

    const unknown_optional = try allocator.dupe(u8, unknown_required);
    defer allocator.free(unknown_optional);
    const flags = std.mem.readInt(u16, unknown_optional[6..8], .little);
    std.mem.writeInt(u16, unknown_optional[6..8], flags | 1, .little);
    const optional_summary = try protocol.validateTransportEnvelope(unknown_optional);
    try std.testing.expect(optional_summary.optional_message);

    const oversized_payload_bytes = 64 * 1024 + 1;
    const oversized_control = try allocator.alloc(
        u8,
        protocol.transport_envelope_header_bytes + oversized_payload_bytes,
    );
    defer allocator.free(oversized_control);
    @memcpy(
        oversized_control[0..protocol.transport_envelope_header_bytes],
        fixturePrefix(envelope_fixture, protocol.transport_envelope_header_bytes),
    );
    @memset(oversized_control[protocol.transport_envelope_header_bytes..], 0);
    std.mem.writeInt(u32, oversized_control[8..12], @intCast(oversized_control.len), .little);
    std.mem.writeInt(u32, oversized_control[12..16], oversized_payload_bytes, .little);
    oversized_control[22] = 1;
    try std.testing.expectError(
        error.LimitExceeded,
        protocol.validateTransportEnvelope(oversized_control),
    );
}

test "native frame decoder rejects non-canonical wrap, spacer, style, and patch fields" {
    const allocator = std.testing.allocator;
    const locations = try locateFrameTables(frame_fixture);
    const columns = readFixtureU16(frame_fixture, 88);

    const invalid_underline = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(invalid_underline);
    invalid_underline[locations.first_style + 18] = 6;
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalFrame(allocator, invalid_underline),
    );

    const invalid_spacer_head = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(invalid_spacer_head);
    const non_final_cell_flags = locations.first_row + 8 + 4 * 8 + 6;
    invalid_spacer_head[non_final_cell_flags] =
        (invalid_spacer_head[non_final_cell_flags] & ~@as(u8, 0x03)) | 0x03;
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalFrame(allocator, invalid_spacer_head),
    );

    const invalid_wrap = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(invalid_wrap);
    const second_row = locations.first_row + 8 + @as(usize, columns) * 8;
    invalid_wrap[second_row + 2] &= ~@as(u8, 0x02);
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalFrame(allocator, invalid_wrap),
    );

    const different_patch = try allocator.dupe(u8, frame_fixture);
    defer allocator.free(different_patch);
    different_patch[144] = 4;
    try std.testing.expectError(
        error.UnsupportedVersion,
        protocol.validateTerminalFrame(allocator, different_patch),
    );
}

test "native row-window decoder rejects non-canonical mappings, wrap, prompt, and patch" {
    const allocator = std.testing.allocator;
    const text_length = readFixtureU32(row_window_fixture, 116);
    const mapping_count = readFixtureU32(row_window_fixture, 120);
    const first_mapping = 124 + @as(usize, text_length);
    const second_mapping = first_mapping + 12;
    const second_row = first_mapping + @as(usize, mapping_count) * 12;

    const invalid_unicode_offset = try allocator.dupe(u8, row_window_fixture);
    defer allocator.free(invalid_unicode_offset);
    std.mem.writeInt(u32, invalid_unicode_offset[second_mapping..][0..4], 2, .little);
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalRowWindow(allocator, invalid_unicode_offset),
    );

    const overlapping_columns = try allocator.dupe(u8, row_window_fixture);
    defer allocator.free(overlapping_columns);
    overlapping_columns[first_mapping + 10] = 2;
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalRowWindow(allocator, overlapping_columns),
    );

    const invalid_prompt = try allocator.dupe(u8, row_window_fixture);
    defer allocator.free(invalid_prompt);
    invalid_prompt[113] = 3;
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalRowWindow(allocator, invalid_prompt),
    );

    const invalid_wrap = try allocator.dupe(u8, row_window_fixture);
    defer allocator.free(invalid_wrap);
    invalid_wrap[second_row] &= ~@as(u8, 0x02);
    try std.testing.expectError(
        error.InvalidField,
        protocol.validateTerminalRowWindow(allocator, invalid_wrap),
    );

    const different_patch = try allocator.dupe(u8, row_window_fixture);
    defer allocator.free(different_patch);
    different_patch[100] = protocol.terminal_row_window_format_patch + 1;
    try std.testing.expectError(
        error.UnsupportedVersion,
        protocol.validateTerminalRowWindow(allocator, different_patch),
    );
}

test "native text row window rejects one mapped cell spanning more than the per-cell string bound" {
    const allocator = std.testing.allocator;
    const text = try allocator.alloc(u8, 16_385);
    defer allocator.free(text);
    @memset(text, 0x61);
    const mappings = [_]TextRowWindowMapping{.{
        .byte_offset = 0,
        .utf16_offset = 0,
        .column = 0,
        .width = 1,
    }};
    const window = try buildTextRowWindow(allocator, 1, text, &mappings);
    defer allocator.free(window);
    try std.testing.expectEqual(@as(usize, 16_521), window.len);
    try std.testing.expectError(
        error.LimitExceeded,
        protocol.validateTerminalRowWindow(allocator, window),
    );
}

test "native text row window admits one mapped cell at exactly the per-cell string bound" {
    const allocator = std.testing.allocator;
    const text = try allocator.alloc(u8, 16_384);
    defer allocator.free(text);
    @memset(text, 0x61);
    const mappings = [_]TextRowWindowMapping{.{
        .byte_offset = 0,
        .utf16_offset = 0,
        .column = 0,
        .width = 1,
    }};
    const window = try buildTextRowWindow(allocator, 1, text, &mappings);
    defer allocator.free(window);
    const summary = try protocol.validateTerminalRowWindow(allocator, window);
    try std.testing.expectEqual(@as(u32, 1), summary.entry_count);
}

test "native text row window rejects an over-cap interior mapped cell span" {
    const allocator = std.testing.allocator;
    const text = try allocator.alloc(u8, 16_386);
    defer allocator.free(text);
    @memset(text, 0x61);
    const mappings = [_]TextRowWindowMapping{
        .{ .byte_offset = 0, .utf16_offset = 0, .column = 0, .width = 1 },
        .{ .byte_offset = 16_385, .utf16_offset = 16_385, .column = 1, .width = 1 },
    };
    const window = try buildTextRowWindow(allocator, 2, text, &mappings);
    defer allocator.free(window);
    try std.testing.expectError(
        error.LimitExceeded,
        protocol.validateTerminalRowWindow(allocator, window),
    );
}

test "native text row window admits aggregate row text above the per-cell bound with bounded spans" {
    const allocator = std.testing.allocator;
    const text = try allocator.alloc(u8, 32_768);
    defer allocator.free(text);
    @memset(text, 0x61);
    const mappings = [_]TextRowWindowMapping{
        .{ .byte_offset = 0, .utf16_offset = 0, .column = 0, .width = 1 },
        .{ .byte_offset = 16_384, .utf16_offset = 16_384, .column = 1, .width = 1 },
    };
    const window = try buildTextRowWindow(allocator, 2, text, &mappings);
    defer allocator.free(window);
    const summary = try protocol.validateTerminalRowWindow(allocator, window);
    try std.testing.expectEqual(@as(u32, 2), summary.entry_count);
}

test "native WriteInput decoder rejects unscoped and oversized payloads" {
    try std.testing.expectError(error.Truncated, protocol.validateWriteInput(&([_]u8{0x22} ** 16)));

    const oversized = try std.testing.allocator.alloc(u8, 24 + 1024 * 1024 + 1);
    defer std.testing.allocator.free(oversized);
    @memset(oversized, 0x22);
    try std.testing.expectError(error.LimitExceeded, protocol.validateWriteInput(oversized));
}

fn payloadEncodingName(encoding: protocol.PayloadEncoding) []const u8 {
    return switch (encoding) {
        .control_json => "control-json",
        .raw_bytes => "raw-bytes",
        .row_window => "row-window",
        .terminal_frame => "terminal-frame",
    };
}

fn expectOptionalPayloadEncoding(
    expected: ?[]const u8,
    actual: ?protocol.PayloadEncoding,
) !void {
    if (expected) |expected_value| {
        try std.testing.expectEqualStrings(
            expected_value,
            payloadEncodingName(actual orelse return error.TestUnexpectedResult),
        );
    } else {
        try std.testing.expect(actual == null);
    }
}

fn parseControlFixture(
    allocator: std.mem.Allocator,
) !std.json.Parsed(ControlConformanceFixture) {
    try std.testing.expect(control_fixture.len > 0);
    return std.json.parseFromSlice(
        ControlConformanceFixture,
        allocator,
        control_fixture,
        .{},
    );
}

fn parseUtf8BomFixture(
    allocator: std.mem.Allocator,
) !std.json.Parsed(Utf8BomConformanceFixture) {
    try std.testing.expect(utf8_bom_fixture.len > 0);
    return std.json.parseFromSlice(
        Utf8BomConformanceFixture,
        allocator,
        utf8_bom_fixture,
        .{},
    );
}

fn expectRawControlCase(
    comptime decode_fn: anytype,
    comptime observed_integer_fn: anytype,
    case: ControlRawCaseFixture,
    payload: []const u8,
) !void {
    var decoded = decode_fn(std.testing.allocator, payload) catch |err| {
        const expected_code = case.expected_code orelse {
            std.debug.print("raw control case unexpectedly failed: {s}: {s}\n", .{
                case.name,
                @errorName(err),
            });
            return err;
        };
        try std.testing.expectEqualStrings(expected_code, controlDecodeErrorCode(err));
        try std.testing.expect(case.expected_integer == null);
        return;
    };
    defer decoded.deinit();

    if (case.expected_code) |expected_code| {
        std.debug.print("raw control case unexpectedly accepted: {s}: {s}\n", .{
            case.name,
            expected_code,
        });
        return error.TestExpectedError;
    }
    try std.testing.expectEqual(
        case.expected_integer,
        observed_integer_fn(decoded.value),
    );
}

fn controlDecodeErrorCode(err: control_decode.DecodeError) []const u8 {
    return switch (err) {
        error.InvalidUtf8 => "invalid-utf8",
        error.InvalidJson => "invalid-json",
        error.InvalidField => "invalid-field",
        error.InvalidIdentifier => "invalid-identifier",
        error.CommandInspectionOutOfMemory => "resource-exhausted",
        error.OutOfMemory => "out-of-memory",
    };
}

fn protocolErrorCode(err: protocol.ProtocolError) []const u8 {
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

fn noObservedInteger(_: anytype) ?u64 {
    return null;
}

fn listTerminalsObservedInteger(value: anytype) ?u64 {
    return @intCast(value.maximum_terminals);
}

fn defaultsObservedInteger(value: anytype) ?u64 {
    return @intCast(value.scrollback_bytes);
}

fn encodeWithWriter(
    allocator: std.mem.Allocator,
    comptime write_fn: anytype,
    value: anytype,
) ![]u8 {
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try write_fn(&writer.writer, value);
    return writer.toOwnedSlice();
}

fn expectOwnedJson(
    allocator: std.mem.Allocator,
    expected: []const u8,
    actual: []u8,
) !void {
    defer allocator.free(actual);
    try expectJsonEquivalent(allocator, expected, actual);
}

fn expectJsonEquivalent(
    allocator: std.mem.Allocator,
    expected: []const u8,
    actual: []const u8,
) !void {
    const expected_value = try std.json.parseFromSlice(std.json.Value, allocator, expected, .{});
    defer expected_value.deinit();
    const actual_value = try std.json.parseFromSlice(std.json.Value, allocator, actual, .{});
    defer actual_value.deinit();
    try expectJsonValueEquivalent(expected_value.value, actual_value.value);
}

fn expectJsonValueEquivalent(expected: std.json.Value, actual: std.json.Value) !void {
    try std.testing.expectEqual(std.meta.activeTag(expected), std.meta.activeTag(actual));
    switch (expected) {
        .null => {},
        .bool => |value| try std.testing.expectEqual(value, actual.bool),
        .integer => |value| try std.testing.expectEqual(value, actual.integer),
        .float => |value| try std.testing.expectEqual(value, actual.float),
        .number_string => |value| try std.testing.expectEqualStrings(value, actual.number_string),
        .string => |value| try std.testing.expectEqualStrings(value, actual.string),
        .array => |values| {
            try std.testing.expectEqual(values.items.len, actual.array.items.len);
            for (values.items, actual.array.items) |expected_item, actual_item| {
                try expectJsonValueEquivalent(expected_item, actual_item);
            }
        },
        .object => |values| {
            try std.testing.expectEqual(values.count(), actual.object.count());
            var iterator = values.iterator();
            while (iterator.next()) |entry| {
                const actual_value = actual.object.get(entry.key_ptr.*) orelse {
                    std.debug.print("missing JSON object member: {s}\n", .{entry.key_ptr.*});
                    return error.TestExpectedEqual;
                };
                try expectJsonValueEquivalent(entry.value_ptr.*, actual_value);
            }
        },
    }
}

fn locateFrameTables(bytes: []const u8) !struct { first_row: usize, first_style: usize } {
    if (bytes.len < protocol.terminal_frame_header_bytes) return error.TestUnexpectedResult;
    const string_count = std.mem.readInt(u32, bytes[124..128], .little);
    const style_count = std.mem.readInt(u16, bytes[128..130], .little);
    var offset: usize = protocol.terminal_frame_header_bytes;
    for (0..string_count) |_| {
        if (offset > bytes.len -| 4) return error.TestUnexpectedResult;
        const length = std.mem.readInt(u32, bytes[offset..][0..4], .little);
        offset = std.math.add(usize, offset, 4 + @as(usize, length)) catch
            return error.TestUnexpectedResult;
        if (offset > bytes.len) return error.TestUnexpectedResult;
    }
    const first_row = std.math.add(
        usize,
        offset,
        @as(usize, style_count) * 20,
    ) catch return error.TestUnexpectedResult;
    if (first_row > bytes.len) return error.TestUnexpectedResult;
    return .{ .first_row = first_row, .first_style = offset };
}
