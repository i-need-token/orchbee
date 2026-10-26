//! Plugin source processes remain unavailable on Unix; no synthetic command outcome is produced.
use super::{NativeRunner, Unsupported};

pub(super) fn runner() -> Result<Box<dyn NativeRunner>, Unsupported> {
    Err(Unsupported)
}
