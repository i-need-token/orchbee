//! Unix activation has no restore-feedback reader and performs no storage probe.
use super::{Read, Unsupported};
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;

pub(in crate::local_activation) fn read(_session: &ModeCheckedLocalSessionRendezvous) -> Read {
    Err(Unsupported)
}
