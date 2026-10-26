//! Deterministic host event decoding and typed terminal replies, independent of dispatch,
//! Client view state, terminal admission and physical effects.

pub(crate) mod appearance;
pub(crate) mod decoder;
pub(crate) mod graphics;
pub(crate) mod keys;
pub(crate) mod mouse;
pub(crate) mod pointer;
pub(crate) mod utf8;

pub(crate) use decoder::{DecodedItem, DecodedKind, LegacyDecoder};
pub(crate) use mouse::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PanePointerRoute,
    encode_cursor_arrows, encode_pane_report, route_pane_pointer,
};

/// Metadata replies produced by the outer terminal. These are consumed before
/// any keyboard, modal-editor or pane-input owner can observe their bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostReply {
    Graphics(graphics::Reply),
    Pointer(pointer::HostReply),
    Appearance(appearance::ColorReply),
}
