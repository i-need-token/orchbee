const harness = @import("service_harness.zig");
const aggregate_harness = @import("runtime_aggregate_harness.zig");
const property = @import("composition_property_harness.zig");

const std = harness.std;
const core_module = harness.core_module;
const reference = harness.host_composition_reference;
const semantic_trace = harness.host_composition_trace;

const ConnectionState = harness.ConnectionState;
const HostService = harness.HostService;

pub fn run(commands: []const property.Command) !void {
    var machine = try Machine.init();
    defer machine.deinit();
    for (commands) |command| try machine.execute(command);
}

const Machine = struct {
    service: HostService,
    connection: ConnectionState,
    refinement: reference.Refinement,
    validated: aggregate_harness.terminal_launch.Validated,
    native_create: semantic_trace.Create,
    native_owner_id: core_module.AttachmentId,
    native_observer_id: core_module.AttachmentId,
    model_attachment_id: core_module.AttachmentId,
    model_creation_id: core_module.CreationId,
    model_terminal_id: core_module.TerminalId,

    fn init() !Machine {
        var service = HostService.init(std.testing.allocator);
        errdefer service.deinit();
        var connection: ConnectionState = .awaiting_hello;
        var refinement = try reference.Refinement.initService(
            std.testing.allocator,
            &service,
            &connection,
        );
        errdefer refinement.deinit();

        try harness.testHandshake(&service, &connection, "31313131313131313131313131313131");
        const active = try harness.activeConnection(&connection);
        try refinement.stepService(&service, &connection, .{ .connect = .{
            .client_id = core_module.ClientId.fromBytes([_]u8{0x31} ** 16),
            .connection_id = active.connection_id,
            .limits = limits(active),
        } }, .applied);

        const validated = try aggregate_harness.validatedLongRunningLaunch();
        const creation_id = harness.testId(core_module.CreationId, 0xb1, 0);
        var preparation = try service.runtime_aggregate.prepareCreateTerminal(
            active.connection_id,
            creation_id,
            validated,
        );
        defer preparation.deinit();
        const terminal_id = switch (try service.runtime_aggregate.commitPreparedTerminal(&preparation)) {
            .created => |identity| identity.terminal_id,
            .replayed => return error.ExpectedCreatedTerminal,
        };
        const create_action: semantic_trace.Create = .{
            .configuration = validated.launch.configuration orelse
                service.runtime_aggregate.core().terminalDefaults(),
            .creation_id = creation_id,
            .geometry = validated.launch.geometry.?,
            .launch_digest = validated.launch_digest,
            .terminal_id = terminal_id,
        };
        try refinement.stepService(
            &service,
            &connection,
            .{ .create = create_action },
            .applied,
        );

        return .{
            .service = service,
            .connection = connection,
            .refinement = refinement,
            .validated = validated,
            .native_create = create_action,
            .native_owner_id = harness.testId(core_module.AttachmentId, 0xb1, 1),
            .native_observer_id = harness.testId(core_module.AttachmentId, 0xb1, 2),
            .model_attachment_id = harness.testId(core_module.AttachmentId, 0xb2, 1),
            .model_creation_id = harness.testId(core_module.CreationId, 0xb2, 0),
            .model_terminal_id = harness.testId(core_module.TerminalId, 0xb2, 0),
        };
    }

    fn deinit(self: *Machine) void {
        self.refinement.deinit();
        self.service.disconnect(&self.connection);
        self.service.deinit();
        self.* = undefined;
    }

    fn execute(self: *Machine, command: property.Command) !void {
        switch (command) {
            .native_attach_owner => try self.attachNative(self.nativeOwner()),
            .native_attach_observer => try self.attachNative(self.nativeObserver()),
            .native_attach_conflict => try self.attachNative(self.nativeConflict()),
            .native_demand_active => try self.setNativeDemand(.active),
            .native_demand_paused => try self.setNativeDemand(.paused),
            .native_input => try self.acceptNativeInput(),
            .native_resize_small => try self.resizeNative(.{
                .cell_pixel_height = 16,
                .cell_pixel_width = 8,
                .columns = 90,
                .rows = 25,
            }),
            .native_resize_large => try self.resizeNative(.{
                .cell_pixel_height = 20,
                .cell_pixel_width = 10,
                .columns = 120,
                .rows = 40,
            }),
            .native_config_small => try self.configureNative(.{ .scrollback_bytes = 512_000 }),
            .native_config_default => try self.configureNative(.{}),
            .native_clear => try self.controlNative(.clear),
            .native_reset => try self.controlNative(.reset),
            .native_detach_owner => try self.detach(self.native_owner_id),
            .native_detach_observer => try self.detach(self.native_observer_id),
            .disconnect => try self.disconnect(),
            .connect => try self.connect(),
            .native_create_replay => try self.replayNativeCreate(),
            .poll => try self.poll(),
            .model_create => try self.createModel(),
            .model_attach_read => try self.attachModel(),
            .model_detach => try self.detach(self.model_attachment_id),
            .model_graceful => try self.terminateModel(.graceful),
            .model_force => try self.terminateModel(.force),
            .model_child_exit => try self.observeModelChildExit(),
            .model_pty_drain => try self.observeModelPtyDrain(),
            .model_failure => try self.observeModelFailure(),
            .model_contain => try self.containModelFailure(),
            .model_publish => try self.publishModelOutcome(),
            .model_dispose => try self.disposeModel(),
        }
    }

    fn attachNative(self: *Machine, action: semantic_trace.Attach) !void {
        const connection_id = self.activeConnectionId() orelse
            return self.record(.{ .attach = action }, .rejected);
        const result = self.service.runtime_aggregate.attach(
            connection_id,
            action.attachment_id,
            action.terminal_id,
            action.capabilities,
            action.request_resize_lease,
        ) catch return self.record(.{ .attach = action }, .rejected);
        try self.record(.{ .attach = action }, transitionOutcome(result.transition));
    }

    fn setNativeDemand(self: *Machine, demand: core_module.ProjectionDemand) !void {
        const action: semantic_trace.Action = .{ .set_projection_demand = .{
            .attachment_id = self.native_owner_id,
            .demand = demand,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.setProjectionDemand(
            connection_id,
            self.native_owner_id,
            demand,
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn acceptNativeInput(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .accept_input = .{
            .attachment_id = self.native_owner_id,
            .byte_count = 1,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.acceptRawInput(
            connection_id,
            self.native_owner_id,
            "x",
        );
        const outcome: semantic_trace.Outcome = switch (result) {
            .accepted => .applied,
            .core_rejected, .runtime_missing, .runtime_rejected => .rejected,
        };
        try self.record(action, outcome);
    }

    fn resizeNative(self: *Machine, geometry: harness.model_boundary.geometry.Geometry) !void {
        const action: semantic_trace.Action = .{ .resize = .{
            .attachment_id = self.native_owner_id,
            .geometry = geometry,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const terminal = self.service.runtime_aggregate.core().findTerminal(
            self.native_create.terminal_id,
        ) orelse return self.record(action, .rejected);
        const replayed = std.meta.eql(terminal.observedGeometry(), geometry);
        _ = self.service.runtime_aggregate.resizeTerminal(
            connection_id,
            self.native_owner_id,
            geometry,
        ) catch return self.record(action, .rejected);
        try self.record(action, if (replayed) .replayed else .applied);
    }

    fn configureNative(
        self: *Machine,
        configuration: harness.terminal_configuration.Configuration,
    ) !void {
        const action: semantic_trace.Action = .{ .update_configuration = .{
            .configuration = configuration,
            .terminal_id = self.native_create.terminal_id,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const terminal = self.service.runtime_aggregate.core().findTerminal(
            self.native_create.terminal_id,
        ) orelse return self.record(action, .rejected);
        const replayed = std.meta.eql(terminal.effectiveConfiguration(), configuration);
        _ = self.service.runtime_aggregate.updateTerminalConfiguration(
            connection_id,
            self.native_create.terminal_id,
            configuration,
        ) catch return self.record(action, .rejected);
        try self.record(action, if (replayed) .replayed else .applied);
    }

    fn controlNative(self: *Machine, operation: semantic_trace.TerminalControlOperation) !void {
        const action: semantic_trace.Action = .{ .apply_terminal_control = .{
            .attachment_id = self.native_owner_id,
            .operation = operation,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        _ = self.service.runtime_aggregate.applyTerminalControl(
            connection_id,
            self.native_owner_id,
            switch (operation) {
                .clear => .clear,
                .reset => .reset,
            },
        ) catch return self.record(action, .rejected);
        try self.record(action, .applied);
    }

    fn detach(self: *Machine, attachment_id: core_module.AttachmentId) !void {
        const action: semantic_trace.Action = .{ .detach = attachment_id };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.detach(
            connection_id,
            attachment_id,
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result.outcome));
    }

    fn disconnect(self: *Machine) !void {
        const connection_id = self.activeConnectionId() orelse {
            self.service.disconnect(&self.connection);
            return self.record(.{ .disconnect = 0 }, .replayed);
        };
        self.service.disconnect(&self.connection);
        try self.record(.{ .disconnect = connection_id }, .applied);
    }

    fn connect(self: *Machine) !void {
        if (self.activeConnectionId()) |connection_id| {
            const active = try harness.activeConnection(&self.connection);
            return self.record(.{ .connect = .{
                .client_id = core_module.ClientId.fromBytes([_]u8{0x33} ** 16),
                .connection_id = connection_id,
                .limits = limits(active),
            } }, .rejected);
        }
        try harness.testHandshake(&self.service, &self.connection, "33333333333333333333333333333333");
        const active = try harness.activeConnection(&self.connection);
        try self.record(.{ .connect = .{
            .client_id = core_module.ClientId.fromBytes([_]u8{0x33} ** 16),
            .connection_id = active.connection_id,
            .limits = limits(active),
        } }, .applied);
    }

    fn replayNativeCreate(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .create = self.native_create };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        var preparation = self.service.runtime_aggregate.prepareCreateTerminal(
            connection_id,
            self.native_create.creation_id,
            self.validated,
        ) catch return self.record(action, .rejected);
        defer preparation.deinit();
        const result = self.service.runtime_aggregate.commitPreparedTerminal(&preparation) catch
            return self.record(action, .rejected);
        try self.record(action, if (result.isCreated()) .applied else .replayed);
    }

    fn poll(self: *Machine) !void {
        const model_pending_before = self.modelOutcomePending();
        var outgoing = self.service.pollEvents(&self.connection) catch
            return self.record(.poll, .rejected);
        defer outgoing.deinit();
        if (model_pending_before and !self.modelOutcomePending()) {
            return self.refinement.stepsService(
                &self.service,
                &self.connection,
                &.{
                    .{
                        .action = .{ .publish_outcome = self.model_terminal_id },
                        .outcome = .applied,
                    },
                    .{ .action = .poll, .outcome = .applied },
                },
            );
        }
        try self.record(.poll, .applied);
    }

    fn createModel(self: *Machine) !void {
        const action = self.modelCreate();
        const connection_id = self.activeConnectionId() orelse
            return self.record(.{ .create = action }, .rejected);
        const result = core_module.testing.createTerminal(
            self.service.runtime_aggregate.core(),
            connection_id,
            action.creation_id,
            action.terminal_id,
            action.launch_digest,
            action.geometry.columns,
            action.geometry.rows,
        ) catch return self.record(.{ .create = action }, .rejected);
        try self.record(.{ .create = action }, transitionOutcome(result));
    }

    fn attachModel(self: *Machine) !void {
        const action = self.modelAttachment();
        const connection_id = self.activeConnectionId() orelse
            return self.record(.{ .attach = action }, .rejected);
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(.{ .attach = action }, .rejected);
        if (terminal.phase() != .running) return self.record(.{ .attach = action }, .rejected);
        const result = self.service.runtime_aggregate.core().attach(
            connection_id,
            action.attachment_id,
            action.terminal_id,
            action.capabilities,
            action.request_resize_lease,
        ) catch return self.record(.{ .attach = action }, .rejected);
        try self.record(.{ .attach = action }, transitionOutcome(result));
    }

    fn terminateModel(self: *Machine, mode: semantic_trace.TerminationMode) !void {
        const action: semantic_trace.Action = .{ .request_termination = .{
            .mode = mode,
            .terminal_id = self.model_terminal_id,
        } };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const result = switch (mode) {
            .graceful => self.service.runtime_aggregate.core().requestTerminate(
                connection_id,
                self.model_terminal_id,
            ),
            .force => self.service.runtime_aggregate.core().requestForceTerminate(
                connection_id,
                self.model_terminal_id,
            ),
        } catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn observeModelChildExit(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .observe_child_exit = self.model_terminal_id };
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(action, .rejected);
        if (terminal.phase() == .containing_failure or terminal.phase() == .failed) {
            return self.record(action, .rejected);
        }
        const result = self.service.runtime_aggregate.core().childExited(
            self.model_terminal_id,
            .{ .exit_code = 0 },
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn observeModelPtyDrain(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .observe_pty_drain = self.model_terminal_id };
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(action, .rejected);
        if (terminal.phase() == .containing_failure or terminal.phase() == .failed) {
            return self.record(action, .rejected);
        }
        const result = self.service.runtime_aggregate.core().ptyDrained(
            self.model_terminal_id,
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn observeModelFailure(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .observe_runtime_failure = self.model_terminal_id };
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(action, .rejected);
        if (terminal.phase() == .exited) return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.core().runtimeFailed(
            self.model_terminal_id,
            .read_output,
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn containModelFailure(self: *Machine) !void {
        const rejected_action: semantic_trace.Action = .{ .confirm_failure_containment = .{
            .process_observed = false,
            .terminal_id = self.model_terminal_id,
        } };
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(rejected_action, .rejected);
        if (terminal.phase() != .containing_failure and terminal.phase() != .failed) {
            return self.record(rejected_action, .rejected);
        }
        const result = self.service.runtime_aggregate.core().failureContained(
            self.model_terminal_id,
            .{ .unavailable = .observe_process },
        ) catch return self.record(rejected_action, .rejected);
        const contained = self.service.runtime_aggregate.core().findTerminal(
            self.model_terminal_id,
        ) orelse unreachable;
        try self.record(.{ .confirm_failure_containment = .{
            .process_observed = contained.childExited(),
            .terminal_id = self.model_terminal_id,
        } }, transitionOutcome(result));
    }

    fn publishModelOutcome(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .publish_outcome = self.model_terminal_id };
        const terminal = self.service.runtime_aggregate.core().findTerminal(self.model_terminal_id) orelse
            return self.record(action, .rejected);
        const token = if (terminal.pendingOutcome()) |pending| switch (pending) {
            .exited => |outcome| outcome.token,
            .failed => |outcome| outcome.token,
        } else if (terminal.settledFinalRevision()) |revision| core_module.OutcomePublicationToken{
            .final_revision = revision,
            .kind = if (terminal.phase() == .exited) .exited else .failed,
            .terminal_id = self.model_terminal_id,
        } else return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.core().markOutcomePublished(token);
        try self.record(action, transitionOutcome(result));
    }

    fn disposeModel(self: *Machine) !void {
        const action: semantic_trace.Action = .{ .dispose = self.model_terminal_id };
        const connection_id = self.activeConnectionId() orelse
            return self.record(action, .rejected);
        const result = self.service.runtime_aggregate.core().disposeTerminal(
            connection_id,
            self.model_terminal_id,
        ) catch return self.record(action, .rejected);
        try self.record(action, transitionOutcome(result));
    }

    fn record(
        self: *Machine,
        action: semantic_trace.Action,
        outcome: semantic_trace.Outcome,
    ) !void {
        try self.refinement.stepService(
            &self.service,
            &self.connection,
            action,
            outcome,
        );
    }

    fn activeConnectionId(self: *const Machine) ?core_module.ConnectionId {
        return switch (self.connection) {
            .active => |active| active.connection_id,
            .awaiting_hello, .closing => null,
        };
    }

    fn modelOutcomePending(self: *Machine) bool {
        const terminal = self.service.runtime_aggregate.core().findTerminal(
            self.model_terminal_id,
        ) orelse return false;
        return terminal.pendingOutcome() != null;
    }

    fn nativeOwner(self: *const Machine) semantic_trace.Attach {
        return .{
            .attachment_id = self.native_owner_id,
            .capabilities = .{ .input = true, .read = true, .resize = true },
            .request_resize_lease = true,
            .terminal_id = self.native_create.terminal_id,
        };
    }

    fn nativeObserver(self: *const Machine) semantic_trace.Attach {
        return .{
            .attachment_id = self.native_observer_id,
            .capabilities = .{ .input = true, .read = true, .resize = true },
            .request_resize_lease = false,
            .terminal_id = self.native_create.terminal_id,
        };
    }

    fn nativeConflict(self: *const Machine) semantic_trace.Attach {
        return .{
            .attachment_id = self.native_owner_id,
            .capabilities = .{ .input = false, .read = true, .resize = false },
            .request_resize_lease = false,
            .terminal_id = self.native_create.terminal_id,
        };
    }

    fn modelCreate(self: *Machine) semantic_trace.Create {
        return .{
            .configuration = self.service.runtime_aggregate.core().terminalDefaults(),
            .creation_id = self.model_creation_id,
            .geometry = .{
                .cell_pixel_height = 1,
                .cell_pixel_width = 1,
                .columns = 80,
                .rows = 24,
            },
            .launch_digest = [_]u8{0xb2} ** 32,
            .model_only = true,
            .terminal_id = self.model_terminal_id,
        };
    }

    fn modelAttachment(self: *const Machine) semantic_trace.Attach {
        return .{
            .attachment_id = self.model_attachment_id,
            .capabilities = .{ .input = false, .read = true, .resize = false },
            .request_resize_lease = false,
            .terminal_id = self.model_terminal_id,
        };
    }
};

fn limits(active: *const ConnectionState.Active) semantic_trace.ConnectionLimits {
    return .{
        .frame_bytes = active.limits.frame_bytes,
        .input_bytes = active.limits.input_bytes,
        .row_window_bytes = active.limits.row_window_bytes,
    };
}

fn transitionOutcome(outcome: core_module.TransitionOutcome) semantic_trace.Outcome {
    return switch (outcome) {
        .applied => .applied,
        .idempotent => .replayed,
    };
}
