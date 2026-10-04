//! Exact, one-effect link activation shared by the native TUI and CLI adapters.
//!
//! This application-private owner keeps painted terminal identity, bounded
//! content reads, ordered plugin routing, and the native activation lane out
//! of either presentation surface. Callers contribute only their admission
//! and cancellation adapters; no terminal-control capability enters here.

pub(crate) mod context;
mod lane;
pub(crate) mod plugin;
mod reader;
mod source;
mod windows;

pub(crate) use lane::{Admission, Driver, Notice};
pub(crate) use plugin::{Activation, Error, Guard, Invocation, Selection};
pub(crate) use reader::ReadSession;
pub(crate) use source::{PaintedLinkSource, PaintedRows, SnapshotMember};
