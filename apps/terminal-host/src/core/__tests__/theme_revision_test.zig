const harness = @import("core_harness.zig");
const std = harness.std;
const model = harness.model_boundary;
const subject = harness.subject;
const ClientId = harness.ClientId;
const ConnectionId = harness.ConnectionId;
const CreationId = harness.CreationId;
const HostCore = harness.HostCore;
const HostEpoch = harness.HostEpoch;
const TerminalId = harness.TerminalId;
const createModel = harness.createModel;
const expectCoreInvariants = harness.expectCoreInvariants;
const testId = harness.testId;

const palette = [_]u32{0x01_02_03} ** 256;
const replacement: model.theme.Theme = .{
    .color_scheme = .light,
    .default_background_rgb = 0xee_ee_ee,
    .default_cursor_rgb = 0x12_34_56,
    .default_foreground_rgb = 0x11_11_11,
    .palette = palette,
};

test "terminal theme compare-and-set commits once and checks exact replay before stale input" {
    const ids = themeIds();
    var core = try runningCore(ids);
    defer core.deinit();
    const operation_id = operationId(0x31);

    const initial = core.findTerminal(ids.terminal).?.themeState();
    try std.testing.expectEqual(@as(u64, 0), initial.revision);
    try std.testing.expect(initial.operation_id == null);
    try std.testing.expectEqual(
        subject.ThemeAdmission.apply,
        try core.prepareTerminalTheme(ids.connection, ids.terminal, operation_id, 0, replacement),
    );
    try std.testing.expectEqualDeep(initial, core.findTerminal(ids.terminal).?.themeState());

    core.commitTerminalTheme(ids.terminal, operation_id, replacement);
    const applied = core.findTerminal(ids.terminal).?.themeState();
    try std.testing.expectEqual(@as(u64, 1), applied.revision);
    try std.testing.expect(applied.operation_id.?.eql(operation_id));
    try std.testing.expect(model.theme.eql(replacement, applied.terminal));

    try std.testing.expectEqual(
        subject.ThemeAdmission.already_applied,
        try core.prepareTerminalTheme(ids.connection, ids.terminal, operation_id, 0, replacement),
    );
    try std.testing.expectEqual(
        subject.ThemeAdmission.stale,
        try core.prepareTerminalTheme(
            ids.connection,
            ids.terminal,
            operationId(0x32),
            0,
            replacement,
        ),
    );
    try std.testing.expectError(
        error.ThemeOperationConflict,
        core.prepareTerminalTheme(
            ids.connection,
            ids.terminal,
            operation_id,
            1,
            model.theme.default,
        ),
    );
    try std.testing.expectEqualDeep(applied, core.findTerminal(ids.terminal).?.themeState());
    try expectCoreInvariants(&core);
}

test "terminal theme rejection and revision exhaustion preserve the complete prior state" {
    const ids = themeIds();
    var core = try runningCore(ids);
    defer core.deinit();
    subject.testing.setThemeRevision(&core, ids.terminal, std.math.maxInt(u64));
    subject.testing.setThemeOperationId(&core, ids.terminal, operationId(0x41));
    const before = core.findTerminal(ids.terminal).?.themeState();

    try std.testing.expectError(
        error.RevisionOverflow,
        core.prepareTerminalTheme(
            ids.connection,
            ids.terminal,
            operationId(0x42),
            std.math.maxInt(u64),
            replacement,
        ),
    );
    var invalid = replacement;
    invalid.palette[255] = 0x01_00_00_00;
    try std.testing.expectError(
        error.InvalidTheme,
        core.prepareTerminalTheme(
            ids.connection,
            ids.terminal,
            operationId(0x43),
            std.math.maxInt(u64),
            invalid,
        ),
    );
    try std.testing.expectEqualDeep(before, core.findTerminal(ids.terminal).?.themeState());
}

const ThemeIds = struct {
    connection: ConnectionId,
    creation: CreationId,
    terminal: TerminalId,
};

fn themeIds() ThemeIds {
    return .{
        .connection = 0x21,
        .creation = testId(CreationId, 0x22),
        .terminal = testId(TerminalId, 0x23),
    };
}

fn runningCore(ids: ThemeIds) !HostCore {
    var core = HostCore.init(std.testing.allocator, testId(HostEpoch, 0x20));
    errdefer core.deinit();
    try std.testing.expectEqual(
        .applied,
        try core.connect(ids.connection, testId(ClientId, 0x24)),
    );
    try std.testing.expectEqual(
        .applied,
        try createModel(
            &core,
            ids.connection,
            ids.creation,
            ids.terminal,
            [_]u8{0x25} ** 32,
            80,
            24,
        ),
    );
    return core;
}

fn operationId(byte: u8) model.identifiers.TerminalThemeOperationId {
    return .fromBytes([_]u8{byte} ** 16);
}
