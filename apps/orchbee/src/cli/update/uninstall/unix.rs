//! Uninstall is built only on Windows; every port here refuses or finds nothing, and admission
//! answers the platform refusal before any of them runs.
use orchbee_local_runtime::DataRoot;
use std::{
    io,
    path::{Path, PathBuf},
    time::Instant,
};

pub(super) struct Locations;

pub(super) struct StoreProcess {
    pub(super) pid: u32,
    pub(super) image: PathBuf,
}

pub(super) fn store_root() -> io::Result<PathBuf> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn protected_folders() -> Vec<PathBuf> {
    Vec::new()
}

pub(super) fn locations() -> io::Result<Locations> {
    Err(io::ErrorKind::Unsupported.into())
}

impl Locations {
    pub(super) fn path_key(&self) -> String {
        String::new()
    }

    pub(super) fn registration_key(&self) -> String {
        String::new()
    }

    pub(super) fn environment(&self) -> &str {
        ""
    }

    pub(super) fn shortcut(&self) -> Option<&Path> {
        None
    }
}

pub(super) fn path_entry_present(_locations: &Locations, _root: &Path) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn registration_present(_locations: &Locations) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn remove_registration(_locations: &Locations) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn machine_hub(_data: &DataRoot, _deadline: Instant) -> Option<u32> {
    None
}

pub(super) fn end_machine_hub(_data: &DataRoot, _deadline: Instant) -> bool {
    false
}

pub(super) fn store_processes(_root: &Path, _except: &[u32]) -> Vec<StoreProcess> {
    Vec::new()
}

pub(super) fn parent() -> Option<StoreProcess> {
    None
}

pub(super) fn inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

pub(super) fn same(left: &Path, right: &Path) -> bool {
    left == right
}

pub(super) fn npm_carrier(_parent: Option<&StoreProcess>) -> bool {
    false
}

pub(super) fn schedule_removal(
    _files: &[PathBuf],
    _folders: &[PathBuf],
    _marks: &[PathBuf],
    _root: Option<&Path>,
) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
