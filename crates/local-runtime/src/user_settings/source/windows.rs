//! Existing Windows configuration-file reads; the parent admits capability before effects.
use std::{fs::File, io, path::Path};

pub(super) fn open(path: &Path) -> io::Result<File> {
    File::open(path)
}
