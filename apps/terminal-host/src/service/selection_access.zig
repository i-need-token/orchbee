const core_module = @import("terminal-host-core").state;
const terminal_engine = @import("terminal-host-engine");
const terminal_history = terminal_engine.history;
const terminal_runtime = terminal_engine.runtime;

pub const SelectionGestureRequest = struct {
    event: terminal_history.SelectionGestureEvent,
    generation: u64,
};

pub const SelectionCommandRequest = struct {
    command: terminal_history.SelectionCommand,
    generation: u64,
    revision: u64,
};

const SelectionExportHandoffState = union(enum) {
    provisional: struct {
        result: terminal_runtime.SelectionExportStartResult,
        runtime: *terminal_runtime.TerminalRuntime,
    },
    committed,
};

/// A request-scoped selection capability produced only after the aggregate has
/// resolved a physical connection, read authority, and readable runtime.
pub const SelectionAccess = struct {
    attachment_id: core_module.AttachmentId,
    runtime: *terminal_runtime.TerminalRuntime,

    pub fn init(
        attachment_id: core_module.AttachmentId,
        runtime: *terminal_runtime.TerminalRuntime,
    ) SelectionAccess {
        return .{ .attachment_id = attachment_id, .runtime = runtime };
    }

    pub fn applySelectionGesture(
        self: SelectionAccess,
        request: SelectionGestureRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.runtime.applySelectionGesture(.{
            .attachment_id = self.attachment_id,
            .event = request.event,
            .generation = request.generation,
        });
    }

    pub fn applySelectionCommand(
        self: SelectionAccess,
        request: SelectionCommandRequest,
    ) !terminal_runtime.SelectionSnapshot {
        return self.runtime.applySelectionCommand(.{
            .attachment_id = self.attachment_id,
            .command = request.command,
            .generation = request.generation,
            .revision = request.revision,
        });
    }

    pub fn startSelectionExport(
        self: SelectionAccess,
        export_sequence: u64,
        format: terminal_runtime.SelectionExportFormat,
    ) !SelectionExportHandoff {
        return .{
            .state = .{ .provisional = .{
                .result = try self.runtime.startSelectionExport(
                    self.attachment_id,
                    export_sequence,
                    format,
                ),
                .runtime = self.runtime,
            } },
        };
    }

    pub fn selectionExportChunkApplied(
        self: SelectionAccess,
        export_sequence: u64,
        chunk_sequence: u32,
    ) !?terminal_runtime.SelectionExportFinished {
        return self.runtime.selectionExportChunkApplied(
            self.attachment_id,
            export_sequence,
            chunk_sequence,
        );
    }

    pub fn cancelSelectionExport(
        self: SelectionAccess,
        export_sequence: u64,
    ) !terminal_runtime.SelectionExportFinished {
        return self.runtime.cancelSelectionExport(
            self.attachment_id,
            export_sequence,
        );
    }
};

pub const SelectionExportHandoff = struct {
    state: SelectionExportHandoffState,

    pub fn result(self: *const SelectionExportHandoff) terminal_runtime.SelectionExportStartResult {
        return switch (self.state) {
            .provisional => |provisional| provisional.result,
            .committed => unreachable,
        };
    }

    pub fn commit(self: *SelectionExportHandoff) void {
        switch (self.state) {
            .provisional => self.state = .committed,
            .committed => unreachable,
        }
    }

    pub fn deinit(self: *SelectionExportHandoff) void {
        switch (self.state) {
            .provisional => |provisional| provisional.runtime.rollbackSelectionExportStart(
                provisional.result,
            ),
            .committed => {},
        }
        self.* = undefined;
    }
};
