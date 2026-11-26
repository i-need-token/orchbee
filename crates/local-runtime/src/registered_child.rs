//! Direct-child registration is a shared lifecycle capability; platform adapters own suspended process creation.
/// A managed process registers only the directly created child owned by its role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionChild {
    Worker,
    /// A Worker that replaces one proven gone. Only the recorded Server starts it, after its own
    /// handle proved the recorded Worker gone or a failed replacement left no process; the Host keeps
    /// its registration, and a Host still spawning refuses the start.
    ReplacementWorker,
    /// A Host in a slot not started in this generation.
    Host(crate::HostSlot),
    /// A Host that takes the place of the Host recorded in its slot once that Host is proven gone,
    /// so the programs the Keeper kept can be recovered into it. Only the recorded Worker starts
    /// it.
    RecoveryHost(crate::HostSlot),
    /// The Session's Keeper in a slot, which only the recorded Worker starts: the first one, one
    /// that replaces a Keeper proven gone, or the one a Keeper update starts beside the current one.
    Keeper(crate::KeeperSlot),
}

/// The step of a registered child's start that failed. A failed [`spawn_session_child`] carries it
/// inside its I/O error, whose kind and operating-system code stay the failed operation's own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionChildStep {
    /// Reading the Session record, taking its writer lock, or publishing the reservation.
    Registration,
    /// Creating the suspended process.
    Creation,
    /// Publishing the created process's identity and releasing its initial thread.
    Release,
}

impl SessionChildStep {
    /// The step a failed start names, when `error` came from one.
    pub fn of(error: &std::io::Error) -> Option<Self> {
        registered::step_of(error)
    }
}

mod registered;
#[cfg(windows)]
pub(crate) use registered::spawn_registered;
pub(crate) use registered::spawn_registered_with;
pub use registered::{
    make_session_keeper_current, record_session_host_epoch, release_session_host,
    release_session_keeper, spawn_session_child,
};
