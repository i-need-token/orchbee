//! The remote helper `orchbee --machine-link-v1`: the acceptor of one `machine-link` connection on
//! standard input and output, carrying several Sessions' byte streams, routed commands, and Session
//! starts. `docs/machines.md#link-helper` owns the ownership, bounds, and ends kept here; the
//! contract in `schemas/machine` owns every byte.
#[cfg(windows)]
mod carrier;
#[cfg(windows)]
mod channels;
#[cfg(windows)]
mod state;

use crate::{cli::Output, localization::Locale};

/// Serves one link until it ends. The status is 0 for an ended link and 5 for a refused start, a
/// refused `OBHS` frame, or a violation; nothing but `OBHS` and `OBMX` bytes reaches standard
/// output. Other platforms answer unsupported before reading a byte.
pub(crate) fn serve(locale: Locale) -> Output {
    #[cfg(windows)]
    {
        let _ = locale; // The helper writes no text; routed commands carry their own locale.
        Output {
            status: carrier::serve(),
            stdout: Vec::new(),
            stderr: String::new(),
            finalization: None,
        }
    }
    #[cfg(not(windows))]
    {
        crate::cli::unsupported_platform(None, locale)
    }
}
