//! macOS has no CLI signal owner yet: arming registers nothing and changes no signal mask.
//! `SIGINT` and `SIGTERM` keep their inherited action and end the invocation at once with
//! `128 + signo` and no document; an operation the Session already accepted keeps running.
use super::NativeSignals;
use crate::cli::SourceFinalization;
use orchbee_local_ipc::LocalIo;
use std::io;

struct Inherited;

pub(super) fn arm() -> io::Result<Box<dyn NativeSignals>> {
    Ok(Box::new(Inherited))
}

impl NativeSignals for Inherited {
    fn wake(&self) -> Option<LocalIo<'_>> {
        None
    }

    fn take(&mut self) -> io::Result<Option<i32>> {
        Ok(None)
    }

    fn finish(self: Box<Self>) -> io::Result<Option<SourceFinalization>> {
        Ok(None)
    }
}
