//! Persistent Windows SSH carriers. Native admission remains end-to-end; no frame is replayed.
#[path = "program.rs"]
mod program;
#[path = "relay.rs"]
mod relay;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
use super::StartedCarrier;
use crate::{machine_ssh, machines::Profile};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::{ModeCheckedLocalSessionRendezvous, SessionScope};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Starts the native relay; the caller immediately adopts its lifetime before Client admission.
pub(super) fn prepare(
    profile: &Profile,
    admitted: &machine_ssh::Client,
) -> io::Result<StartedCarrier> {
    let ssh = machine_ssh::executable()?;
    let remote = program::command(&admitted.executable, profile.draft.session.as_str());
    let target = profile.draft.target.clone();
    start(move || Ok(machine_ssh::command(&ssh, &target, &remote)))
}

fn start(
    command: impl Fn() -> io::Result<std::process::Command> + Send + 'static,
) -> io::Result<StartedCarrier> {
    let id = crate::machines::ProfileId::generate()
        .map_err(|_| io::Error::other("machine endpoint identity unavailable"))?;
    let endpoint = PathBuf::from(format!(r"\\.\pipe\orchbee-machine-{}", id.as_str()));
    let listener = LocalListener::bind(&endpoint)?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let task = thread::Builder::new()
        .name("orchbee-machine-relay".into())
        .spawn(move || {
            let mut lanes = Vec::new();
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) if lanes.len() < 16 => {
                        if let Ok(lane) =
                            command().and_then(|command| relay::Lane::command(stream, command))
                        {
                            lanes.push(lane);
                        }
                    }
                    Ok(_) => {} // Capacity refusal closes only that candidate lane.
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
                let mut index = 0;
                while index < lanes.len() {
                    if matches!(lanes[index].step(), Ok(true)) {
                        index += 1;
                    } else {
                        let mut lane = lanes.swap_remove(index);
                        let _settled = lane.settle();
                    }
                }
                thread::sleep(Duration::from_millis(5));
            }
            let mut failure = None;
            for lane in &mut lanes {
                if let Err(error) = lane.settle() {
                    failure = Some(error);
                }
            }
            failure.map_or(Ok(()), Err)
        })?;
    Ok(StartedCarrier {
        endpoint,
        stop,
        task,
    })
}
/// Private binary helper `--machine-tunnel-v1 <session>`: relays bytes to the selected existing
/// same-user Session of this installation's environment. It judges no peer; the Session admits the
/// relayed Client through the handshake.
pub fn serve() -> io::Result<()> {
    use std::io::IsTerminal;
    if io::stdin().is_terminal() || io::stdout().is_terminal() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let (base, scope) = crate::local_activation::runtime_location()?;
    let session = ModeCheckedLocalSessionRendezvous::validate_mode_checked(
        &base,
        SessionScope::parse(&scope).map_err(io::Error::other)?,
    )
    .map_err(io::Error::other)?;
    let stream = LocalStream::connect_before(session.client_socket_path(), deadline)?;
    stream.require_same_user()?;
    let mut lane = relay::Lane::stdio(stream)?;
    let result = loop {
        match lane.step() {
            Ok(true) => thread::sleep(Duration::from_millis(5)),
            Ok(false) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    let settled = lane.settle();
    result.and(settled)
}
