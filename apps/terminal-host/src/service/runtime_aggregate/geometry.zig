const target_builtin = @import("builtin");
const core_module = @import("terminal-host-core").state;
const terminal_runtime = @import("terminal-host-engine").runtime;
const terminal_geometry = @import("terminal-host-model").geometry;
const host_runtime_maintenance = @import("../runtime_maintenance/root.zig");
const contracts = @import("contracts.zig");

const ResizeError = contracts.ResizeError;
const TerminalResize = contracts.TerminalResize;

pub fn resizeObservationIsCompatible(
    requested: terminal_geometry.Geometry,
    committed_revision: u64,
    observed: terminal_runtime.Geometry,
) bool {
    if (observed.revision < committed_revision) return false;
    if (observed.revision > committed_revision) return true;
    return observed.columns == requested.columns and
        observed.rows == requested.rows and
        observed.cell_pixel_width == requested.cell_pixel_width and
        observed.cell_pixel_height == requested.cell_pixel_height;
}

pub fn resizeTerminal(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    geometry: terminal_geometry.Geometry,
) ResizeError!TerminalResize {
    if (!terminal_geometry.isValid(geometry)) return error.InvalidGeometry;
    const attachment = self.core_state.resolveAttachment(
        connection_id,
        attachment_id,
        .resize,
    ) catch |err| return err;
    if (!attachment.resize_lease) return error.ResizeLeaseDenied;
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse return error.TerminalRuntimeMissing;
    _ = self.synchronizeRuntimeObservation(terminal_id, runtime) catch
        return error.RuntimeStateDiverged;
    _ = try self.core_state.resize(
        connection_id,
        attachment_id,
        geometry.columns,
        geometry.rows,
    );
    const revision = runtime.resizeGeometry(
        geometry.columns,
        geometry.rows,
        geometry.cell_pixel_width,
        geometry.cell_pixel_height,
    ) catch |err| {
        if (err == error.RevisionOverflow) return error.TerminalRevisionOverflow;
        if (err == error.TerminalRuntimeUnavailable) return error.TerminalRuntimeUnavailable;
        if (!runtime.isResizeFenced()) return error.RuntimeResizeFailed;
        const failure = runtime.primaryFailure() orelse .resize;
        _ = self.core_state.runtimeFailed(
            terminal_id,
            host_runtime_maintenance.runtimeFailureFromEngineKind(failure),
        ) catch unreachable;
        self.maintenance.containRuntimeFailure(
            &self.core_state,
            terminal_id,
            runtime,
        ) catch {};
        return error.UnsafeRuntimeAfterResizeFailure;
    };
    if (comptime target_builtin.is_test) {
        if (self.testing_state.after_resize_commit) |hook| {
            hook(self.testing_state.after_resize_commit_context, runtime);
        }
    }
    const runtime_geometry = runtime.geometry();
    if (!resizeObservationIsCompatible(geometry, revision, runtime_geometry)) {
        return error.RuntimeStateDiverged;
    }
    self.core_state.synchronizeGeometry(
        terminal_id,
        runtime_geometry.columns,
        runtime_geometry.rows,
        runtime_geometry.cell_pixel_width,
        runtime_geometry.cell_pixel_height,
        runtime_geometry.revision,
    ) catch return error.RuntimeStateDiverged;
    return .{
        .attachment_id = attachment_id,
        .geometry = geometry,
        .revision = revision,
        .terminal_id = terminal_id,
    };
}
