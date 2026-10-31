//! Shared presentation lifetime for Client-owned TUI surface incarnations, and the renderer's
//! per-surface geometry verdict that drives it.
//!
//! Surface owners keep exact successful-frame proof separately. This module
//! only names the verdict and classifies an evaluated geometry block as refusal
//! before admission or suspension after admission.

/// Presentation lifetime for one Client-owned TUI surface incarnation.
///
/// The phase is independent of the exact token published by a successful
/// frame. Owners keep that draw proof separately and revoke it whenever the
/// surface is not painted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PresentationPhase {
    /// The surface has been opened but has not completed its first paint.
    #[default]
    Pending,
    /// This incarnation has completed at least one paint and has no later
    /// evaluated geometry block.
    Admitted,
    /// A previously admitted incarnation is temporarily geometry-blocked.
    Suspended,
}

/// The owner-level meaning of a renderer's geometry-blocked verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeometrySettlement {
    /// The new incarnation never became visible and must be refused.
    Refused,
    /// The existing incarnation remains owned and may resume after a later paint.
    Suspended,
}

impl PresentationPhase {
    /// Records a complete successful paint for this incarnation.
    pub(crate) fn presented(&mut self) {
        *self = Self::Admitted;
    }

    /// Classifies one evaluated geometry block without conflating it with a
    /// missing receipt caused by absence or a covering owner.
    pub(crate) fn geometry_blocked(&mut self) -> GeometrySettlement {
        match self {
            Self::Pending => GeometrySettlement::Refused,
            Self::Admitted | Self::Suspended => {
                *self = Self::Suspended;
                GeometrySettlement::Suspended
            }
        }
    }

    /// Whether this phase permits exact successful-frame proof to authorize
    /// ordinary controls. Owners must still require that separate proof.
    pub(crate) const fn interactive(self) -> bool {
        matches!(self, Self::Admitted)
    }
}

/// The renderer's geometry verdict for one modal surface in a completed draw.
/// `Presented` proves the complete surface was written. `NotPresented` means
/// only that the renderer evaluated this surface and the offered geometry could
/// not present it; the owning UI state decides whether that is a first-draw
/// refusal or a temporary suspension. An absent receipt means the surface was
/// absent or covered and revokes draw authority without making a geometry
/// verdict. Rendering itself never mutates UI state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModalPresentation<T> {
    Presented(T),
    NotPresented,
}

impl<T> ModalPresentation<T> {
    pub(crate) fn into_presented(self) -> Option<T> {
        match self {
            Self::Presented(token) => Some(token),
            Self::NotPresented => None,
        }
    }
}
