//! Production transport boundary for the native application.

pub use orchbee_local_client::{
    LocalClient, LocalClientConnectError, LocalClientOrigin, LocalOperationClock,
    OutcomeUnknownReason, SubmissionError, exchange_wake_fault, exchange_wake_ready,
};
pub(crate) use orchbee_local_client::{
    LocalClientShutdown, agent, cancellation, multiplex, subscription, workbench,
};

pub(crate) use orchbee_local_client::RetainedCarrier;

#[cfg(any(test, feature = "test-support"))]
pub(crate) use orchbee_local_client::test_support;

use orchbee_connection_handshake::{Build, CohortIdentity, ConnectionKind, Offer, ProductVersion};

/// This build's product version, which every Client offer and refusal names.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// This build's offer on a Session's Client endpoint. `cohort` is the verified cohort identity the
/// Client runs from, when it has one; it only describes the build and decides nothing.
pub fn client_offer(cohort: Option<&str>) -> Offer {
    Offer::local(
        ConnectionKind::ClientServer,
        ProductVersion::new(CLIENT_VERSION).expect("the package version is printable ASCII"),
        Build {
            commit: None,
            cohort: cohort.and_then(|identity| CohortIdentity::parse(identity).ok()),
        },
        orchbee_local_runtime::environment_name(),
    )
}
#[cfg(any(test, feature = "test-support"))]
pub use orchbee_local_client::test_support::{wake_fault_io, wake_ready_io};
