//! Exact terminal-content provenance and bounded inspection on an independent viewer.
//!
//! This application-private owner has no terminal input, controller, clipboard, URL-routing,
//! or presentation policy. Callers provide one exact painted source and a cancellation guard;
//! the reader owns the temporary attachment, content fence, read budgets, and one Detach attempt.

use std::time::Instant;

mod reader;
mod source;

pub(crate) use reader::ReadAttachment;
pub use reader::process_directory;
pub(crate) use source::{ContentSource, PaintedRows, TerminalScreen};
pub use source::SnapshotMember;

/// Cancellation and deadline policy supplied by the owning read lane.
pub trait Guard {
    /// The owning adapter's cancellation or deadline refusal.
    type Error;

    /// Refuse work whose cancellation or deadline owner has settled.
    fn check(&mut self, deadline: Instant) -> Result<(), Self::Error>;
    /// Return the optional descriptor that interrupts an in-flight local transport wait.
    fn wake(&self) -> Option<orchbee_local_ipc::LocalIo<'_>>;
}

/// A typed content-read refusal, preserving whether the owning gate caused it.
#[derive(Debug)]
pub(crate) enum Error<E> {
    Gate(E),
    IdentityUnavailable,
    TransportUnavailable,
    ProtocolInvalid,
    LimitExceeded,
    SourceChanged,
}
