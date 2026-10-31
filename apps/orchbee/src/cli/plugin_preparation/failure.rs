//! Typed local plugin preparation failures, before any terminal or Client mutation.
use crate::cli::{Failure, FailureKind, stage_reason_details};

#[derive(Clone, Copy)]
pub(in crate::cli) enum Reason {
    SelectorUnavailable,
    RegistryUnavailable,
    PluginNotFound,
    PluginDisabled,
    PaneNotFound,
    UnsupportedPlatform,
    InvalidTitle,
    InvalidGeometry,
    PopupFocusRequired,
    PopupTargetUnsupported,
    ClientListingUnavailable,
    ClientNotReady,
    BindingUnavailable,
    InvalidPopupSpec,
    ClientChanged,
    ContextInvalid,
    TransportLoss,
    InvalidReply,
    PaneChanged,
    PluginUnavailable,
}

impl Reason {
    pub(in crate::cli) fn failure(self) -> Failure {
        use FailureKind::{
            InternalFailure, NotFound, ProtocolInvalid, Rejected, ServiceUnavailable,
        };
        let (kind, reason) = match self {
            Self::SelectorUnavailable => (InternalFailure, "selector-unavailable"),
            Self::RegistryUnavailable => (ServiceUnavailable, "registry-unavailable"),
            Self::PluginNotFound => (NotFound, "plugin-not-found"),
            Self::PluginDisabled => (Rejected, "plugin-disabled"),
            Self::PaneNotFound => (NotFound, "pane-not-found"),
            Self::UnsupportedPlatform => (Rejected, "unsupported-platform"),
            Self::InvalidTitle => (Rejected, "invalid-title"),
            Self::InvalidGeometry => (Rejected, "invalid-geometry"),
            Self::PopupFocusRequired => (Rejected, "popup-focus-required"),
            Self::PopupTargetUnsupported => (Rejected, "popup-target-unsupported"),
            Self::ClientListingUnavailable => (ServiceUnavailable, "client-listing-unavailable"),
            Self::ClientNotReady => (Rejected, "client-not-ready"),
            Self::BindingUnavailable => (ServiceUnavailable, "binding-unavailable"),
            Self::InvalidPopupSpec => (Rejected, "invalid-popup-spec"),
            Self::ClientChanged => (Rejected, "client-changed"),
            Self::ContextInvalid => (Rejected, "context-invalid"),
            Self::TransportLoss => (FailureKind::TransportLoss, "transport-loss"),
            Self::InvalidReply => (ProtocolInvalid, "invalid-reply"),
            Self::PaneChanged => (Rejected, "pane-changed"),
            Self::PluginUnavailable => (ServiceUnavailable, "plugin-unavailable"),
        };
        Failure::new(kind).details(stage_reason_details("plugin", reason))
    }
}

/// Publish a Client's refusal of one preparation read or of the popup open as `client-refused`
/// with the Client's typed reason. Every other failure keeps its own contract.
pub(in crate::cli) fn client_refusal(mut failure: Failure) -> Failure {
    if failure.details.is_none()
        && failure.failure_schema.is_none()
        && let Some(reason) = failure.client_refusal
    {
        let mut details = stage_reason_details("plugin", "client-refused");
        details.insert("clientReason".into(), serde_json::json!(reason));
        failure.details = Some(details);
    }
    failure
}

#[cfg(test)]
mod tests;
