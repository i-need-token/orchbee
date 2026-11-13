//! The one client-side owner of the machine hub: attach or start, re-attachment after the hub goes
//! away, requests and their replies, events, ending a hub on purpose, and delivering an answer to a
//! question's own endpoint, which never passes the hub. The CLI and the TUI reach
//! the hub only through it. `apps/orchbee/docs/machines.md#client-owner` owns the rules, and the
//! contract in `schemas/machine` owns every byte. Windows only, as the hub is.

mod answer;
mod attach;
mod control;
mod holders;
mod owner;
mod sessions;

pub use answer::{DeliveryFailure, deliver};
pub use control::{EndFailure, HubWatch, end, present, running};
pub use owner::{
    ChannelDelivery, ChannelFailure, ChannelHandle, HubClient, Notice, OpenCancel, Opening,
    RequestFailure,
};
pub use sessions::{SessionRead, read_sessions, runtime_base};

use orchbee_local_runtime::DataRoot;
use orchbee_machine_protocol::hub::HubState;

/// The hub process's hidden entry, an internal pairing between this program and its own start
/// path; a release may rename it.
pub const HUB_ENTRY: &str = "--machine-hub-v1";

/// What a client is to the hub, as `attach` states it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Attachment {
    /// The client holds the hub.
    pub presence: bool,
    /// The client can answer a login question.
    pub asks: bool,
    /// The client receives `hub`, `catalog` and `machine` events.
    pub watch: bool,
}

/// A connection the hub answered with `attached`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Attached {
    /// The hub's state in its reply.
    pub hub: HubState,
    /// The hubs this attachment started, of at most eight.
    pub starts: u8,
}

/// Why no attachment was reached, or why the owner ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachFailure {
    /// The data root or its `internal/machines/` folder is unavailable.
    DataRoot,
    /// The hub kept answering `capacity` for a second, no hub could start, or a hub dropped five
    /// connections within a minute.
    Unavailable,
    /// The endpoint refused this process, or its server runs as another user.
    Denied,
    /// The hub was still closing at the deadline; for the owner, also a hub that was asked to stop
    /// when no other took its place.
    Closing,
    /// A hub this client cannot use serves the data root.
    Incompatible(Incompatible),
    /// The hub or this client broke the protocol; no re-attachment follows.
    Lost,
    /// The deadline passed otherwise.
    Timeout,
}

/// The hub that refused this client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Incompatible {
    /// A build of another environment serves this data root; it refuses control requests too.
    Environment {
        /// That build's environment name.
        environment: String,
    },
    /// Another version of this environment's hub runs, and a Session or a client of its version
    /// may hold it, so it was not replaced; for the owner whose hub was asked to stop, somebody
    /// else started it.
    Held {
        /// The running hub's product version.
        version: String,
    },
    /// The hub refused the offer for another reason or answered outside the handshake.
    Protocol,
}

/// The data root of this process's environment.
///
/// # Errors
///
/// [`AttachFailure::DataRoot`] when it cannot be resolved.
pub fn data_root() -> Result<DataRoot, AttachFailure> {
    DataRoot::resolve(|name| std::env::var_os(name)).map_err(|_| AttachFailure::DataRoot)
}
