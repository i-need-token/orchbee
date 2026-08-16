const std = @import("std");

const host_service = @import("terminal-host-service").service;
const composition = @import("composition_observation.zig");
const compare = @import("composition_reference_compare.zig");
const reducer = @import("composition_reference_reducer.zig");
const reference_state = @import("composition_reference_state.zig");
const semantic_trace = @import("composition_trace.zig");

pub const Reference = reference_state.Reference;
pub const RefinementError = reference_state.RefinementError;

/// Couples the semantic trace, executable reducer, owner-local invariant
/// oracles, and the cross-owner observation comparison into one test seam.
pub const Refinement = struct {
    reference: Reference,
    trace: semantic_trace.Trace,

    pub fn init(
        allocator: std.mem.Allocator,
        observation: *const composition.Observation,
    ) !Refinement {
        return .{
            .reference = try Reference.init(allocator, observation),
            .trace = semantic_trace.Trace.init(allocator),
        };
    }

    pub fn initService(
        allocator: std.mem.Allocator,
        service: *host_service.HostService,
        connection: ?*const host_service.ConnectionState,
    ) !Refinement {
        try composition.checkInvariants(allocator, service, connection);
        var observation = try composition.capture(allocator, service, connection);
        defer observation.deinit();
        return init(allocator, &observation);
    }

    pub fn deinit(self: *Refinement) void {
        self.reference.deinit();
        self.trace.deinit();
        self.* = undefined;
    }

    pub fn step(
        self: *Refinement,
        action: semantic_trace.Action,
        outcome: semantic_trace.Outcome,
        observation: *const composition.Observation,
    ) !void {
        return self.steps(&.{.{ .action = action, .outcome = outcome }}, observation);
    }

    /// One externally atomic call may linearize several named semantic
    /// actions. Reduce all of them before comparing with its one stable
    /// post-call observation; the trace still retains each action separately.
    pub fn steps(
        self: *Refinement,
        inputs: []const semantic_trace.StepInput,
        observation: *const composition.Observation,
    ) !void {
        try observation.checkInvariants();
        var exact = true;
        for (inputs) |input| {
            const recorded = try self.trace.append(input.action, input.outcome);
            if (std.meta.activeTag(input.action) == .reconcile) {
                if (inputs.len != 1 or input.outcome != .applied) {
                    return error.InvalidAppliedOutcome;
                }
                return self.reference.reconcile(observation);
            }
            try reducer.apply(&self.reference, recorded);
            exact = exact and input.outcome != .unknown;
        }
        if (exact) try compare.expectObservation(&self.reference, observation);
    }

    pub fn stepService(
        self: *Refinement,
        service: *host_service.HostService,
        connection: ?*const host_service.ConnectionState,
        action: semantic_trace.Action,
        outcome: semantic_trace.Outcome,
    ) !void {
        try composition.checkInvariants(self.reference.allocator, service, connection);
        var observation = try composition.capture(self.reference.allocator, service, connection);
        defer observation.deinit();
        try self.step(action, outcome, &observation);
    }

    pub fn stepsService(
        self: *Refinement,
        service: *host_service.HostService,
        connection: ?*const host_service.ConnectionState,
        inputs: []const semantic_trace.StepInput,
    ) !void {
        try composition.checkInvariants(self.reference.allocator, service, connection);
        var observation = try composition.capture(self.reference.allocator, service, connection);
        defer observation.deinit();
        try self.steps(inputs, &observation);
    }
};

pub fn apply(reference: *Reference, step: semantic_trace.Step) !void {
    return reducer.apply(reference, step);
}

pub fn expectObservation(
    reference: *const Reference,
    observation: *const composition.Observation,
) RefinementError!void {
    return compare.expectObservation(reference, observation);
}
