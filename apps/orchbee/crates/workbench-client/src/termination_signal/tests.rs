use std::io;

use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    low_level::raise,
};

use super::{TerminationSignal, TerminationSource};

#[test]
fn observes_every_registered_termination_signal() -> io::Result<()> {
    for signal in [SIGHUP, SIGINT, SIGQUIT, SIGTERM] {
        let monitor = TerminationSignal::install()?;
        assert!(!monitor.requested());
        raise(signal)?;
        assert!(monitor.requested(), "signal {signal} was not observed");
        drop(monitor);
    }
    Ok(())
}
