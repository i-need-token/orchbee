//! Routes each relayed call to the operation that serves it. Routing runs on the link's reader,
//! so an operation that may block hands its call to a thread of its own. No operation is served
//! yet: every call closes as unsupported, which a Client reads as a machine without that
//! operation.

use orchbee_files_protocol::CallClose;

use crate::calls::Call;

/// Serves `call`.
pub(crate) fn dispatch(call: Call) {
    call.close(CallClose::Unsupported);
}
