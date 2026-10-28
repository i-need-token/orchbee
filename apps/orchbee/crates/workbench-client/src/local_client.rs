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
#[cfg(any(test, feature = "test-support"))]
pub use orchbee_local_client::test_support::{wake_fault_io, wake_ready_io};
