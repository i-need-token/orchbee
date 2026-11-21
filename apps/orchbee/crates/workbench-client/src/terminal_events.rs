//! Deterministic host event decoding and typed terminal replies, independent of dispatch,
//! Client view state, terminal admission and physical effects.

// module-layer: values

pub(crate) mod appearance;
pub(crate) mod decoder;
pub(crate) mod graphics;
pub(crate) mod keys;
pub(crate) mod mouse;
pub(crate) mod pointer;
pub(crate) mod utf8;

pub use decoder::{DecodedItem, DecodedKind, LegacyDecoder};
pub use mouse::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, encode_cursor_arrows,
    encode_pane_report,
};
pub(crate) use mouse::{PanePointerRoute, route_pane_pointer};

/// The five values defined by DECRPM for one DEC private mode, shared by every DECRQM probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModeReportState {
    Unsupported,
    Set,
    Reset,
    PermanentlySet,
    PermanentlyReset,
}

/// Metadata replies produced by the outer terminal. These are consumed before
/// any keyboard, modal-editor or pane-input owner can observe their bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReply {
    Graphics(graphics::Reply),
    Pointer(pointer::HostReply),
    Appearance(appearance::ColorReply),
    Grapheme(ModeReportState),
}

/// Crate-internal `DecodedItem` facts the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait DecodedItemTestAccess {
    fn owned(&self) -> bool;
}

#[cfg(any(test, feature = "test-support"))]
impl DecodedItemTestAccess for DecodedItem {
    fn owned(&self) -> bool {
        self.owned
    }
}
