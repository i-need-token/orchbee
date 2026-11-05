const contracts = @import("contracts.zig");
const links = @import("links.zig");
const graphics = @import("graphics.zig");
const state = @import("state.zig");
const render_encoding = @import("../render_encoding.zig");
const protocol = @import("terminal-protocol");

pub const ProjectionError = contracts.ProjectionError;
pub const AttachmentId = contracts.AttachmentId;
pub const HostEpoch = contracts.HostEpoch;
pub const Counters = contracts.Counters;
pub const FrameKind = contracts.FrameKind;
pub const EncodeRequest = contracts.EncodeRequest;
pub const EncodedFrame = contracts.EncodedFrame;
pub const Projection = state.Projection;

pub const testing = if (@import("builtin").is_test) struct {
    pub const capturedStyleFn = render_encoding.capturedStyle;
    pub const styleRecordFn = render_encoding.styleRecord;
    pub const frame_byte_limit = protocol.Limits.frame_bytes;
    pub const link_cache_byte_limit = protocol.Limits.frame_bytes;
    pub const link_entry_limit = protocol.Limits.frame_strings;
    pub const link_value_byte_limit = render_encoding.max_string_bytes;
    pub const LinkTableType = links.LinkTable;
    pub const LinkStringMapType = render_encoding.LinkStringMap;
    pub const SourceLinkCacheType = links.SourceLinkCache;
    pub const SourceLinkKeyType = links.SourceLinkKey;
    pub const GraphicsStateType = graphics.State;
    pub const ImageFormat = graphics.ImageFormat;
    pub const linkTableDeinitFn = links.LinkTable.deinit;
    pub const linkTableInitFn = links.LinkTable.init;
    pub const linkTableInternFn = links.LinkTable.intern;
    pub const linkTableInternCallCountFn = links.LinkTable.testingInternCallCount;
    pub const linkWorkObservationFn = state.Projection.linkWorkObservation;
    pub const projection_source = @embedFile("encoding.zig");
    pub const StringTableType = render_encoding.StringTable;
} else struct {};
