const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

pub const ProjectionError = error{
    CaptureAllocationFailed,
    ProjectionTooLarge,
};

pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;

pub const Counters = struct {
    render_state_creations: u64 = 0,
    captures: u64 = 0,
    encodes: u64 = 0,
};

pub const FrameKind = protocol.FrameKind;

pub const EncodeRequest = struct {
    attachment_id: AttachmentId,
    base_generation: ?u64,
    base_revision: ?u64,
    force_snapshot: bool,
    host_epoch: HostEpoch,
    maximum_bytes: usize,
    sequence: u64,
};

pub const EncodedFrame = struct {
    allocation: []u8,
    base_revision: ?u64,
    bytes: []u8,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    generation: u64,
    kind: FrameKind,
    revision: u64,
    rows: u16,
    sequence: u64,
};
