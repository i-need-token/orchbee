//! One helper run: the store's active Client with `--automatic-update`, named by `active.json` and
//! spelled as that version reads it, at low priority, its standard output bounded and its end
//! bounded by the run limit and the schedule's stop. On Windows it runs in a below-normal Job that
//! ends with this process; on macOS and Linux in a process group of its own under `nice`, with an
//! empty signal mask. It reads the store only to find that Client and its version.
use super::{UpdateJob, outcome::Outcome};
use orchbee_local_runtime::{
    AUTOMATIC_UPDATE_HELPER, CohortArtifact, CommandOutput, active_release, private_filesystem,
    release_store_root,
};
use std::{
    fs::File,
    io,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

/// The most standard output kept from the helper; its document is far smaller, and a cut document
/// reads as no answer.
const OUTPUT_LIMIT: usize = 64 * 1024;
/// The longest a run may take before the Server ends it: the helper's longest own run, a six-hour
/// archive transfer beside its 600-second command deadline, with room to spare.
const RUN_LIMIT: Duration = Duration::from_secs(7 * 60 * 60);

/// Runs the helper once and answers how the run ended. The helper inherits this process's
/// environment, so the store root, the feed and proxy variables reach it unchanged.
pub(super) fn run(job: &UpdateJob) -> Outcome {
    let Ok(root) = release_store_root(|name| std::env::var_os(name)) else {
        return Outcome::NotStarted("no-store-root");
    };
    let release = match active_release(&root) {
        Ok(Some(release)) => release,
        Ok(None) => return Outcome::NotStarted("no-pointer"),
        Err(_) => return Outcome::NotStarted("pointer-unreadable"),
    };
    let client = release.directory.join(CohortArtifact::Client.file_name());
    let Ok(pins) = admit(&client) else {
        return Outcome::NotStarted("client-missing");
    };
    let helper = AUTOMATIC_UPDATE_HELPER.for_release(&release.version);
    let Some(output) = exchange(&client, helper, job) else {
        return Outcome::NotStarted("spawn-failed");
    };
    // The exchange starts and settles the helper in one call, so the pins stay for the whole run
    // rather than ending once the process exists as Session start's do. They hold no more than the
    // running helper's own image already does: the folders down to the version it runs from, which
    // stays the active or the previous version, and an update never removes either.
    drop(pins);
    Outcome::from_output(&output)
}

/// Runs the helper `helper` in a below-normal Job, with the system folder as working folder so that
/// no run holds a folder the store may remove. `None` when that folder is unknown.
#[cfg(windows)]
fn exchange(client: &Path, helper: &str, job: &UpdateJob) -> Option<CommandOutput> {
    let system = winsafe::GetSystemDirectory().ok()?;
    let mut command = Command::new(client);
    // A Server started by an announced TUI inherits its launcher's relaunch variable.
    let relaunch = orchbee_local_runtime::RELAUNCH_PROTOCOL_VARIABLE;
    command
        .arg(helper)
        .env_remove(relaunch.current)
        .env_remove(relaunch.earlier)
        .current_dir(system);
    Some(orchbee_local_runtime::run_background_command_exchange(
        command,
        OUTPUT_LIMIT,
        Some(Instant::now() + RUN_LIMIT),
        || Ok(job.stopped()),
    ))
}

/// The system's `nice`, which lowers its own priority and then becomes the helper, so the helper
/// and everything it starts run at that priority from their first instruction. Linux keeps it at
/// the same path, as it keeps the `curl` and `tar` the update runs.
#[cfg(unix)]
const NICE: &str = "/usr/bin/nice";
/// The priority increment, `nice`'s own default: the helper yields to every ordinary process.
#[cfg(unix)]
const NICENESS: &str = "10";

/// Runs the helper `helper` under `nice` in a process group of its own, with an empty signal mask
/// and the Server's environment; its working folder is the Server's, as a POSIX folder can always
/// be removed. Settling the group sends `SIGTERM`, which the helper answers by settling its own
/// downloads, and `SIGKILL` two seconds later.
#[cfg(unix)]
fn exchange(client: &Path, helper: &str, job: &UpdateJob) -> Option<CommandOutput> {
    let mut command = Command::new(NICE);
    command.arg("-n").arg(NICENESS).arg(client).arg(helper);
    Some(orchbee_local_runtime::run_unmasked_unix_command(
        command,
        orchbee_local_runtime::SpawnEnvironment::Inherited,
        OUTPUT_LIMIT,
        Some(Instant::now() + RUN_LIMIT),
        || Ok(job.stopped()),
    ))
}

/// Pins every folder down to the Client's version folder, which must be private, and the Client,
/// which must be a private regular file with one link, so the program started is the admitted file
/// inside the store's `versions/<version>/`.
fn admit(client: &Path) -> io::Result<Vec<File>> {
    let directory = client.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let mut pins = private_filesystem::open_tree(directory)?;
    pins.push(private_filesystem::open_file(client, false, false)?);
    Ok(pins)
}
