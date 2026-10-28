//! Exact, one-effect link activation shared by the native TUI and CLI adapters.
//!
//! `terminal_content` supplies exact painted provenance and bounded reads. This
//! link owner adds cell resolution, ordered plugin/browser routing, and the
//! native activation lane outside either presentation surface. Callers
//! contribute only admission and cancellation adapters; no terminal-control
//! capability enters here.

mod browser;
pub mod context;
mod lane;
pub mod plugin;
mod reader;

#[cfg(test)]
pub(crate) use crate::terminal_content::PaintedRows;
pub(crate) use crate::terminal_content::ContentSource as PaintedLinkSource;
pub use crate::terminal_content::{Guard, SnapshotMember};
pub(crate) use lane::{Admission, Driver, Notice};
pub use plugin::{Activation, Error};
pub(crate) use reader::ReadAttachment;
