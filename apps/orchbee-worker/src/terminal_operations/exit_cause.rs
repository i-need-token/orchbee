//! Classifies the error that ended the serving loop into the exit record's site, reason, and peer,
//! so the log says which wait ended the Worker. It reads only error types and the connection's
//! first Host failure; it does not decide whether the loop ends.

use std::{error::Error, io};

use orchbee_capability_node_protocol::{ReadMessageError, WriteMessageError};

use crate::{
    diagnostic_events::{ExitCause, Peer, Reason, Site},
    host_connection::{HostBrokerConnection, HostFailure, PeerFailure},
};

/// Wraps the serving loop's final error in its classified cause.
pub(super) fn attach(error: Box<dyn Error>, host: &HostBrokerConnection) -> Box<dyn Error> {
    if error.is::<ExitCause>() {
        return error;
    }
    let failure = host_failure(error.as_ref(), host);
    let (reason, peer, waited) = failure.map_or_else(
        || {
            let (reason, peer) = other(error.as_ref());
            (reason, peer, None)
        },
        |failure| {
            let (reason, peer) = match (failure.class, failure.deadline) {
                (PeerFailure::Server, true) => (Reason::Deadline, Peer::Server),
                (_, true) => (Reason::Deadline, Peer::Host),
                (PeerFailure::Host, false) => (Reason::HostClosed, Peer::Host),
                (PeerFailure::Protocol, false) => (Reason::HostProtocol, Peer::Host),
                (PeerFailure::Server, false) => (Reason::ServerClosed, Peer::Server),
            };
            let waited = (!failure.budget.is_zero()).then_some((failure.waited, failure.budget));
            (reason, peer, waited)
        },
    );
    Box::new(ExitCause {
        site: host.pacing.site(),
        reason,
        peer,
        waited,
        source: error,
    })
}

/// Whether the serving loop's final error is a Host exchange that passed its bound unanswered,
/// which [`attach`] records as a Host deadline.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn host_unanswered(error: &(dyn Error + 'static), host: &HostBrokerConnection) -> bool {
    host_failure(error, host)
        .is_some_and(|failure| failure.deadline && !matches!(failure.class, PeerFailure::Server))
}

/// The Host failure behind the serving loop's final error: one it carries, or the connection's
/// first when the loop reports a peer connection ended.
fn host_failure(error: &(dyn Error + 'static), host: &HostBrokerConnection) -> Option<HostFailure> {
    HostFailure::find(error).or_else(|| peer_lost(error).then(|| host.pacing.failure())?)
}

/// Wraps an error that ended the loop before any Host existed.
pub(super) fn attach_without_host(error: Box<dyn Error>, site: Site) -> Box<dyn Error> {
    if error.is::<ExitCause>() {
        return error;
    }
    let (reason, peer) = other(error.as_ref());
    Box::new(ExitCause {
        site,
        reason,
        peer,
        waited: None,
        source: error,
    })
}

/// The loop's own report that a peer connection ended; the connection's Host failure, if any,
/// says which.
fn peer_lost(error: &(dyn Error + 'static)) -> bool {
    io_error(error).is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::ConnectionAborted
                | io::ErrorKind::UnexpectedEof
                | io::ErrorKind::BrokenPipe
        )
    })
}

/// A failure with no Host failure behind it: the Server's transport or protocol, or the Worker.
fn other(error: &(dyn Error + 'static)) -> (Reason, Peer) {
    let server = |error: &io::Error| match error.kind() {
        io::ErrorKind::TimedOut => (Reason::Deadline, Peer::Server),
        io::ErrorKind::InvalidData => (Reason::ServerProtocol, Peer::Server),
        _ => (Reason::ServerClosed, Peer::Server),
    };
    if let Some(error) = error.downcast_ref::<ReadMessageError>() {
        return match error {
            ReadMessageError::Io(error) => server(error),
            ReadMessageError::Protocol(_) => (Reason::ServerProtocol, Peer::Server),
        };
    }
    if let Some(error) = error.downcast_ref::<WriteMessageError>() {
        return match error {
            WriteMessageError::Io(error) => server(error),
            WriteMessageError::Protocol(_) => (Reason::ServerProtocol, Peer::Server),
        };
    }
    if let Some(error) = io_error(error) {
        return match error.kind() {
            io::ErrorKind::ConnectionAborted
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::BrokenPipe => (Reason::ServerClosed, Peer::Server),
            // A Server request the loop cannot serve: fenced, out of order, or a result.
            io::ErrorKind::InvalidData | io::ErrorKind::PermissionDenied => {
                (Reason::ServerProtocol, Peer::Server)
            }
            io::ErrorKind::Other => (Reason::State, Peer::None),
            _ => (Reason::Local, Peer::None),
        };
    }
    (Reason::Local, Peer::None)
}

fn io_error<'a>(error: &'a (dyn Error + 'static)) -> Option<&'a io::Error> {
    error
        .downcast_ref::<io::Error>()
        .or_else(|| match error.downcast_ref::<ReadMessageError>() {
            Some(ReadMessageError::Io(error)) => Some(error),
            _ => match error.downcast_ref::<WriteMessageError>() {
                Some(WriteMessageError::Io(error)) => Some(error),
                _ => None,
            },
        })
}
