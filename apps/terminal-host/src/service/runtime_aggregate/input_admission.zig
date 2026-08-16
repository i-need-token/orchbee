const core_module = @import("terminal-host-core").state;
const terminal_input = @import("terminal-input");
const contracts = @import("contracts.zig");

const InputAdmission = contracts.InputAdmission;
const InputPreparation = contracts.InputPreparation;

pub fn acceptRawInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    bytes: []const u8,
) InputAdmission {
    var preparation = self.prepareRawInput(connection_id, attachment_id, bytes);
    defer preparation.deinit();
    return preparation.commit();
}

pub fn prepareRawInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    bytes: []const u8,
) InputPreparation {
    const attachment = self.core_state.authorizeInput(
        connection_id,
        attachment_id,
    ) catch |err| return InputPreparation.initCoreRejected(err);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse
        return InputPreparation.initRuntimeMissing();

    const runtime_input = runtime.prepareRawInput(bytes) catch |err|
        return InputPreparation.initRuntimeRejected(err);
    return InputPreparation.initPrepared(attachment_id, runtime_input);
}

pub fn acceptSemanticInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) InputAdmission {
    var preparation = self.prepareSemanticInput(
        connection_id,
        attachment_id,
        input,
        maximum_bytes,
    );
    defer preparation.deinit();
    return preparation.commit();
}

pub fn prepareSemanticInput(
    self: anytype,
    connection_id: core_module.ConnectionId,
    attachment_id: core_module.AttachmentId,
    input: terminal_input.SemanticInput,
    maximum_bytes: usize,
) InputPreparation {
    const attachment = self.core_state.authorizeInput(
        connection_id,
        attachment_id,
    ) catch |err| return InputPreparation.initCoreRejected(err);
    const terminal_id = attachment.terminal_id;
    const runtime = self.runtimeFor(terminal_id) orelse
        return InputPreparation.initRuntimeMissing();

    const runtime_input = runtime.prepareAttachmentSemanticInput(
        attachment_id,
        input,
        maximum_bytes,
    ) catch |err| return InputPreparation.initRuntimeRejected(err);
    return InputPreparation.initPrepared(attachment_id, runtime_input);
}
