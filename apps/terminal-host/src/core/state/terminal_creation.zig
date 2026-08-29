const std = @import("std");
const model = @import("terminal-host-model");
const terminal_configuration = model.configuration;
const terminal_theme = model.theme;
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const CreationId = contracts.CreationId;
const LaunchDigest = contracts.LaunchDigest;
const TerminalId = contracts.TerminalId;
const TerminalSession = contracts.TerminalSession;

pub fn resolveCreation(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    launch_digest: LaunchDigest,
) CoreError!?TerminalId {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    return self.terminal_registry.resolveCreation(creation_id, launch_digest, null);
}

pub fn resolveCreationWithOverlay(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    launch_digest: LaunchDigest,
    environment_overlay_digest: ?LaunchDigest,
) CoreError!?TerminalId {
    if (!storage.connected(self, connection_id)) return error.MissingConnection;
    return self.terminal_registry.resolveCreation(
        creation_id,
        launch_digest,
        environment_overlay_digest,
    );
}

pub fn reserveTerminal(self: anytype) (CoreError || std.mem.Allocator.Error)!void {
    try self.terminal_registry.reserveAdmission();
}

// The aggregate calls these commits only after reserving both registry
// capacities and constructing the complete retained-terminal allocation.
// `session` is its embedded stable field. Host commands are linearized on one
// thread, so the validated admission facts cannot change between prepare and
// the infallible commit.

pub fn commitRunningTerminal(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    terminal_id: TerminalId,
    launch_digest: LaunchDigest,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    configuration: terminal_configuration.Configuration,
    initial_theme: terminal_theme.Theme,
    session: *TerminalSession,
) void {
    commitRunningTerminalWithOverlay(
        self,
        connection_id,
        creation_id,
        terminal_id,
        launch_digest,
        [_]u8{0} ** 32,
        columns,
        rows,
        cell_pixel_width,
        cell_pixel_height,
        configuration,
        initial_theme,
        session,
    );
}

pub fn commitRunningTerminalWithOverlay(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    terminal_id: TerminalId,
    launch_digest: LaunchDigest,
    environment_overlay_digest: LaunchDigest,
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    configuration: terminal_configuration.Configuration,
    initial_theme: terminal_theme.Theme,
    session: *TerminalSession,
) void {
    self.commitAwaitingStartTerminalWithOverlay(
        connection_id,
        creation_id,
        terminal_id,
        launch_digest,
        environment_overlay_digest,
        configuration,
        initial_theme,
        session,
    );
    self.commitTerminalStart(terminal_id, .{
        .cell_pixel_height = cell_pixel_height,
        .cell_pixel_width = cell_pixel_width,
        .columns = columns,
        .rows = rows,
    });
}

pub fn commitAwaitingStartTerminal(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    terminal_id: TerminalId,
    launch_digest: LaunchDigest,
    configuration: terminal_configuration.Configuration,
    initial_theme: terminal_theme.Theme,
    session: *TerminalSession,
) void {
    commitAwaitingStartTerminalWithOverlay(
        self,
        connection_id,
        creation_id,
        terminal_id,
        launch_digest,
        [_]u8{0} ** 32,
        configuration,
        initial_theme,
        session,
    );
}

pub fn commitAwaitingStartTerminalWithOverlay(
    self: anytype,
    connection_id: ConnectionId,
    creation_id: CreationId,
    terminal_id: TerminalId,
    launch_digest: LaunchDigest,
    environment_overlay_digest: LaunchDigest,
    configuration: terminal_configuration.Configuration,
    initial_theme: terminal_theme.Theme,
    session: *TerminalSession,
) void {
    std.debug.assert(terminal_configuration.valid(configuration));
    std.debug.assert(terminal_theme.valid(initial_theme));
    std.debug.assert(storage.connected(self, connection_id));
    self.terminal_registry.validateAdmission(creation_id, terminal_id) catch unreachable;
    session.* = .{
        .configuration_state = .{ .effective = configuration },
        .creation_id = creation_id,
        .engine_observation = null,
        .environment_overlay_digest = environment_overlay_digest,
        .launch_digest = launch_digest,
        .lifecycle = .{ .awaiting_start = .{} },
        .start_geometry = null,
        .terminal_id = terminal_id,
        .theme_state = .{ .terminal = initial_theme },
    };
    self.terminal_registry.commitAssumeCapacity(session);
}
