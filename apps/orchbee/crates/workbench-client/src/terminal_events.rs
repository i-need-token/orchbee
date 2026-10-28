//! Deterministic host event decoding and typed terminal replies, independent of dispatch,
//! Client view state, terminal admission and physical effects.

pub(crate) mod appearance;
pub(crate) mod decoder;
pub(crate) mod graphics;
pub(crate) mod keys;
pub(crate) mod mouse;
pub(crate) mod pointer;
pub(crate) mod utf8;

pub use decoder::DecodedItem;
pub use decoder::DecodedKind;
pub use decoder::LegacyDecoder;
pub use mouse::MouseAction;
pub use mouse::MouseButton;
pub use mouse::MouseEvent;
pub use mouse::PaneEncode;
pub use mouse::PaneMouseEvent;
pub use mouse::encode_cursor_arrows;
pub use mouse::encode_pane_report;
pub(crate) use mouse::{PanePointerRoute, route_pane_pointer};

/// Metadata replies produced by the outer terminal. These are consumed before
/// any keyboard, modal-editor or pane-input owner can observe their bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReply {
    Graphics(graphics::Reply),
    Pointer(pointer::HostReply),
    Appearance(appearance::ColorReply),
}
