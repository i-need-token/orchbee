//! Production transport boundary for the native application.

pub(crate) use orchbee_local_client::{
    LocalClient, LocalClientConnectError, LocalClientOrigin, LocalClientShutdown,
    LocalOperationClock, OutcomeUnknownReason, SubmissionError, agent, cancellation,
    exchange_wake_fault, exchange_wake_ready, multiplex, subscription, workbench,
};

#[cfg(windows)]
pub(crate) use orchbee_local_client::RetainedCarrier;
