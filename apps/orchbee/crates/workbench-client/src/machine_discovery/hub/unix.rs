//! macOS and Linux have no hub yet: no holder starts, and nothing is asked of a hub.
use super::{Hub, Shared};
use crate::{
    background_thread::BackgroundThread,
    machine_login::{Answer, Token},
};
use orchbee_machine_protocol::{Id, hub::AnswerEndpoint};
use std::{io, sync::Arc};

pub(super) fn spawn(_shared: Arc<Shared>) -> io::Result<BackgroundThread> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn reload(hub: Hub) {
    match hub {}
}

pub(super) fn connect(_shared: &Arc<Shared>, _machine: String, _number: u64) {}

/// No episode is claimed without a hub.
pub(super) fn dismiss(_shared: &Arc<Shared>, _login: Id) {}

/// No question is asked without a hub; the answer is wiped as it drops.
pub(super) fn deliver(
    _shared: &Arc<Shared>,
    _token: Token,
    _endpoint: AnswerEndpoint,
    answer: Answer,
) {
    drop(answer);
}
