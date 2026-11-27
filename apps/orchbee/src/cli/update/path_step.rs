//! The PATH step a completing Client runs under `install.lock`, before it moves the pointer, so a
//! failure leaves the pointer where it was; and the refresh the automatic helper runs before its
//! completion. On Windows the step places `<root>\bin\<stem>.exe` and names `<root>\bin` in the user
//! `Path`; on macOS and Linux it links `~/.local/bin/<stem>` to the version's Client, and on macOS
//! it also puts `~/.local/bin` on new shells' `PATH` through the person's shell startup file.
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(super) use unix::{
    follow_up, location, own_link, plan, refresh_launcher, refresh_present, run, store_link,
};
#[cfg(not(windows))]
pub(super) use unix::{profile_present, remove_profile};
#[cfg(windows)]
pub(super) use windows::{follow_up, location, plan, refresh_launcher, refresh_present, run};

/// What the PATH step would do, planned from the state before the completion runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Planned {
    /// Nothing of Orchbee's is in place yet.
    Add,
    /// The store's own entry is in place; the step keeps it, naming this version.
    Keep,
    /// An earlier layout's or another store's Orchbee entry is in place; the step replaces it.
    Replace,
}
