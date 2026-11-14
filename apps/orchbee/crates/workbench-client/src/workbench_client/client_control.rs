//! Remote navigation lane: a shared-connection control subscription whose steps the UI owner
//! advances.
mod lane;
pub(in crate::workbench_client) use lane::{
    ClientControlLane, CommandCompletion, CommandFailure, Sample,
};
#[cfg(test)]
pub(in crate::workbench_client) use lane::{Delivery, LayoutCompletion};
