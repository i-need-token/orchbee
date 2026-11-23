//! The background-program kinds of the message registry, 110 to 115, which only a Worker that
//! negotiated `background-programs` sends or receives. `contract.zig` owns every other kind.

const contract = @import("contract.zig");

const MessageContract = contract.MessageContract;
const RequestOperation = contract.RequestOperation;
const M = contract.MessageKind;

pub fn messageContract(kind: u16) ?MessageContract {
    return switch (kind) {
        M.start_program => controlRequest("StartProgram", .start_program, M.start_program),
        M.program_exited => .{ .name = "ProgramExited", .event_encoding = .control_json },
        M.cancel_program => controlRequest("CancelProgram", .cancel_program, M.cancel_program),
        M.read_program_output => controlRequest("ReadProgramOutput", .read_program_output, M.read_program_output),
        M.dispose_program => controlRequest("DisposeProgram", .dispose_program, M.dispose_program),
        M.list_programs => controlRequest("ListPrograms", .list_programs, M.list_programs),
        else => null,
    };
}

/// A control request answered by a control response of its own kind.
fn controlRequest(name: []const u8, operation: RequestOperation, kind: u16) MessageContract {
    return .{
        .name = name,
        .request = .{ .operation = operation, .payload_encoding = .control_json, .success_kind = kind },
        .response_encoding = .control_json,
    };
}
