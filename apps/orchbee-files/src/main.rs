//! `orchbee-files`, the File Bar's file and Git service for one Worker generation. It admits itself
//! to the Server that asked the Worker to start it, serves the File Bar records that Server relays,
//! and ends at the Server's drain, at the link's end or at a termination signal. Scope,
//! containment and every file or Git effect stay in this process; the Server only relays.

mod admission;
mod blocking;
mod calls;
mod entry;
mod error;
mod exit;
mod grants;
mod invocation;
mod link;
mod operations;
mod outbound;
mod roots;
mod scope;
#[cfg(unix)]
mod signals;
mod subscriptions;

use std::{ffi::OsString, process::ExitCode, sync::mpsc};

use crate::{
    exit::{Outcome, Stage},
    link::End,
};

fn main() -> ExitCode {
    // A root whose layout this build cannot read is refused before anything in it is read, the
    // log included, as the Worker that started this process refuses it.
    if orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .is_ok_and(|root| root.admit_layout().is_err())
    {
        return ExitCode::FAILURE;
    }
    orchbee_local_runtime::install_diagnostic_log(orchbee_local_runtime::LogRole::Files);
    exit::end_on_panic();
    let outcome = run(std::env::args_os().skip(1));
    outcome.record();
    outcome.exit_code()
}

fn run(arguments: impl Iterator<Item = OsString>) -> Outcome {
    let Some(endpoint) = invocation::parse(arguments) else {
        return Outcome::failed(Stage::Invocation, None);
    };
    let token = match admission::read_token(&mut std::io::stdin().lock()) {
        Ok(token) => token,
        Err(error) => return Outcome::failed(Stage::Token, Some(error.kind())),
    };
    let stream = match admission::connect(&endpoint) {
        Ok(stream) => stream,
        Err(error) => return Outcome::failed(Stage::Connect, Some(error.kind())),
    };
    if let Err(error) = admission::admit(&stream, token) {
        return error.outcome();
    }
    admission::write_ready_marker(&mut std::io::stdout().lock());
    exit::started();
    let (ends, ended) = mpsc::channel();
    // Caught only once admitted: until then no child exists, so a signal's default action ends
    // the process with nothing left behind.
    #[cfg(unix)]
    {
        let ends = ends.clone();
        if let Err(error) = signals::catch().and_then(|caught| {
            caught.forward(move |signal| {
                let _ = ends.send(End::Signal(signal));
            })
        }) {
            return Outcome::failed(Stage::Signals, Some(error.kind()));
        }
    }
    match link::serve(stream, ends, &ended) {
        End::Drained(reason) => Outcome::Drained(reason),
        End::Signal(signal) => Outcome::Signalled(signal),
        End::ServerGone => Outcome::failed(Stage::ServerGone, None),
        End::Failed(stage, error) => Outcome::failed(stage, Some(error)),
    }
}
