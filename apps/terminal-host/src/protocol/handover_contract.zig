//! The Host handover kinds of the message registry, 90 to 105, which only a Worker that negotiated
//! `keeper` or `handover` sends or receives. `contract.zig` owns every other kind.

const contract = @import("contract.zig");

const MessageContract = contract.MessageContract;
const PayloadEncoding = contract.PayloadEncoding;
const RequestOperation = contract.RequestOperation;
const M = contract.MessageKind;

pub fn messageContract(kind: u16) ?MessageContract {
    return switch (kind) {
        M.attach_keeper => controlRequest("AttachKeeper", .attach_keeper, M.attach_keeper),
        M.detach_keeper => controlRequest("DetachKeeper", .detach_keeper, M.detach_keeper),
        M.keeper_lost => .{ .name = "KeeperLost", .event_encoding = .control_json },
        M.keeper_unresponsive => .{ .name = "KeeperUnresponsive", .event_encoding = .control_json },
        M.terminal_unheld => .{ .name = "TerminalUnheld", .event_encoding = .control_json },
        M.quiesce_terminal => controlRequest("QuiesceTerminal", .quiesce_terminal, M.quiesce_terminal),
        M.read_handover_record => request("ReadHandoverRecord", .read_handover_record, .control_json, M.handover_record_chunk),
        M.handover_record_chunk => .{ .name = "HandoverRecordChunk", .response_encoding = .handover_record },
        M.resume_terminal => controlRequest("ResumeTerminal", .resume_terminal, M.resume_terminal),
        M.drop_terminal => controlRequest("DropTerminal", .drop_terminal, M.drop_terminal),
        M.import_terminal => controlRequest("ImportTerminal", .import_terminal, M.import_terminal),
        M.write_handover_record => request("WriteHandoverRecord", .write_handover_record, .handover_record, M.handover_record_written),
        M.handover_record_written => .{ .name = "HandoverRecordWritten", .response_encoding = .control_json },
        M.restore_terminal => controlRequest("RestoreTerminal", .restore_terminal, M.restore_terminal),
        M.claim_terminal => controlRequest("ClaimTerminal", .claim_terminal, M.claim_terminal),
        M.discard_import => controlRequest("DiscardImport", .discard_import, M.discard_import),
        else => null,
    };
}

fn request(
    name: []const u8,
    operation: RequestOperation,
    payload_encoding: PayloadEncoding,
    success_kind: u16,
) MessageContract {
    return .{
        .name = name,
        .request = .{ .operation = operation, .payload_encoding = payload_encoding, .success_kind = success_kind },
    };
}

/// A control request answered by a control response of its own kind.
fn controlRequest(name: []const u8, operation: RequestOperation, kind: u16) MessageContract {
    var value = request(name, operation, .control_json, kind);
    value.response_encoding = .control_json;
    return value;
}
