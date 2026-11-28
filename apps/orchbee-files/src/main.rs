//! `orchbee-files`, the File Bar's file and Git service for one Worker generation. It admits itself
//! to the Server that asked the Worker to start it, serves the File Bar records that Server relays,
//! and ends at the Server's drain, at the link's end or at a termination signal. Scope,
//! containment and every file or Git effect stay in this process; the Server only relays.

#![deny(unsafe_code)]

mod admission;
mod blocking;
mod calls;
mod edit;
mod entry;
mod error;
mod exit;
mod fs;
mod git;
mod grants;
#[cfg(unix)]
mod groups;
mod invocation;
mod link;
mod operations;
mod outbound;
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed COM apartment, shell item and IFileOperation calls are isolated here"
)]
mod recycle_bin_native;
mod requests;
mod roots;
mod scope;
#[cfg(test)]
mod seeded;
#[cfg(unix)]
mod signals;
mod subscriptions;

use std::{ffi::OsString, process::ExitCode, sync::mpsc};

use orchbee_local_runtime::DataRoot;

use crate::{
    exit::{Outcome, Stage},
    link::End,
};

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    // Git starts this executable as its askpass program and editor, which only asks the relay of
    // the process that started Git and reads nothing else.
    if let Some(code) = git::helper(&arguments) {
        return code;
    }
    // A root whose layout this build cannot read is refused before anything in it is read, the
    // log included, as the Worker that started this process refuses it.
    let root = DataRoot::resolve(|name| std::env::var_os(name)).ok();
    if root
        .as_ref()
        .is_some_and(|root| root.admit_layout().is_err())
    {
        return ExitCode::FAILURE;
    }
    orchbee_local_runtime::install_diagnostic_log(orchbee_local_runtime::LogRole::Files);
    exit::end_on_panic();
    let outcome = run(arguments.into_iter(), root);
    outcome.record();
    outcome.exit_code()
}

fn run(arguments: impl Iterator<Item = OsString>, root: Option<DataRoot>) -> Outcome {
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
    // Before any child starts, so the Worker hears of every group this process could leave.
    #[cfg(unix)]
    groups::install();
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
    match link::serve(stream, ends, &ended, root) {
        End::Drained(reason) => Outcome::Drained(reason),
        End::Signal(signal) => Outcome::Signalled(signal),
        End::ServerGone => Outcome::failed(Stage::ServerGone, None),
        End::Failed(stage, error) => Outcome::failed(stage, Some(error)),
    }
}
