//! The settled outcome of one machine observation, shared by every machine job and by the
//! Client state that holds the latest result.
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observation<T> {
    Loading,
    Ready(Arc<T>),
    Unavailable,
}
