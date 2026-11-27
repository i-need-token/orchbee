//! The File Bar's wire contracts: the identities its traffic names and OBFL, the private link
//! between a Server and the `orchbee-files` process it admitted. Callers own transport,
//! deadlines, admission and scope; decoding a value grants no authority.

mod identity;
mod link;

pub use identity::{CallId, ConnectionId, InvalidIdentity, LaneId, LinkToken, WorkspaceId};
pub use link::{
    BuildIdentity, CallClose, DrainReason, FrameKind, GrantClass, LINK_MAGIC, LINK_VERSION,
    LinkError, LinkFrame, LinkHello, LinkMessage, LinkRefusal, LinkRequest, LinkSide,
    MAXIMUM_ENVIRONMENT_BYTES, MAXIMUM_LINK_PAYLOAD_BYTES, MAXIMUM_ROOT_BYTES,
    MAXIMUM_VERSION_BYTES, ScopeUpdate, WorkspaceScope, decode_link_message, encode_link_message,
    read_link_message, write_link_message,
};
