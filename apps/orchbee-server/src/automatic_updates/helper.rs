//! One helper run: the store's active Client with `--automatic-update-v1`, named by `active.json`,
//! in a below-normal Job that ends with this process, its standard output bounded and its end
//! bounded by the run limit and the schedule's stop. It reads the store only to find that Client.
use super::{Stop, outcome::Outcome};
use orchbee_local_runtime::{
    AUTOMATIC_UPDATE_HELPER, CohortArtifact, active_release, private_filesystem,
    release_store_root, run_background_command_exchange,
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
pub(super) fn run(stop: &Stop) -> Outcome {
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
    let Ok(system) = winsafe::GetSystemDirectory() else {
        return Outcome::NotStarted("spawn-failed");
    };
    let mut command = Command::new(&client);
    command.arg(AUTOMATIC_UPDATE_HELPER).current_dir(system);
    let output = run_background_command_exchange(
        command,
        OUTPUT_LIMIT,
        Some(Instant::now() + RUN_LIMIT),
        || Ok(stop.requested()),
    );
    // The exchange starts and settles the helper in one call, so the pins stay for the whole run
    // rather than ending once the process exists as Session start's do. They hold no more than the
    // running helper's own image already does: the folders down to the version it runs from, which
    // stays the active or the previous version, and an update never removes either.
    drop(pins);
    Outcome::from_output(&output)
}

/// Pins every folder down to the Client's version folder, which must be private, and the Client,
/// which must be a private regular file with one link, so the program started is the admitted file
/// inside `<store root>\versions\<version>\`.
fn admit(client: &Path) -> io::Result<Vec<File>> {
    let directory = client.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let mut pins = private_filesystem::open_tree(directory)?;
    pins.push(private_filesystem::open_file(client, false, false)?);
    Ok(pins)
}
