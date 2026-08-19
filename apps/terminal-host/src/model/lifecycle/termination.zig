const std = @import("std");
const types = @import("types.zig");

const TerminationAdmission = types.TerminationAdmission;
const TerminationRequest = types.TerminationRequest;

pub fn admission(
    current: TerminationRequest,
    request: TerminationRequest,
) TerminationAdmission {
    return switch (request) {
        .none => unreachable,
        .graceful => if (current == .none) .dispatch else .replay,
        .force => if (current == .force) .replay else .dispatch,
    };
}

pub fn commit(current: *TerminationRequest, request: TerminationRequest) void {
    switch (request) {
        .none => unreachable,
        .graceful => std.debug.assert(current.* == .none),
        .force => std.debug.assert(current.* != .force),
    }
    current.* = request;
}
