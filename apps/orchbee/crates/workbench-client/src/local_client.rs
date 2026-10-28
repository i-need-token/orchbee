//! Production transport boundary for the native application.

pub use orchbee_local_client::{
    LocalClient, LocalClientConnectError, LocalClientOrigin, LocalOperationClock,
    OutcomeUnknownReason, SubmissionError, agent, cancellation, exchange_wake_fault,
    exchange_wake_ready, workbench,
};
pub(crate) use orchbee_local_client::{LocalClientShutdown, multiplex, subscription};

pub(crate) use orchbee_local_client::RetainedCarrier;

#[cfg(any(test, feature = "test-support"))]
pub use orchbee_local_client::test_support::{self, wake_fault_io, wake_ready_io};
