//! The install and activation engine below the CLI's build checks. Every location, key and
//! command it uses arrives as a value, so tests drive it in process with the test key and fixture
//! roots while only the CLI entry layer reads the release environment.
use super::{
    Context, Error, Report,
    acquire::Source,
    complete,
    install::{self, Activation, Completion, Probe},
    manifest::{self, Release},
    signature::{self, Refusal, TrustedKey},
};
use crate::cli::configuration;
use orchbee_local_runtime::UpdateChannel;
use std::{ffi::OsString, path::PathBuf};

/// Everything one update reads besides the release feed itself.
pub(super) struct Inputs<'a> {
    /// The channel `channel set` saves before anything else.
    pub(super) channel: Option<UpdateChannel>,
    /// The environment through which the settings file and the data root resolve.
    pub(super) state: &'a dyn Fn(&str) -> Option<OsString>,
    /// The manifest location before `{channel}` expansion; absent means unconfigured.
    pub(super) manifest_source: Option<String>,
    /// The public keys a manifest signature must verify against.
    pub(super) keys: &'a [TrustedKey],
    /// The admitted store root, or why its configuration was refused.
    pub(super) install_root: Result<PathBuf, Error>,
    pub(super) probe: Probe,
    pub(super) activation: Activation,
}

/// Everything one `install` reads.
pub(super) struct InstallInputs<'a> {
    /// The signed release directory the running Client sits in, or why it has none.
    pub(super) directory: Result<PathBuf, Error>,
    /// The public keys a manifest signature must verify against.
    pub(super) keys: &'a [TrustedKey],
    /// The admitted store root, or why its configuration was refused.
    pub(super) install_root: Result<PathBuf, Error>,
    pub(super) probe: Probe,
    pub(super) activation: Activation,
    pub(super) completion: Completion,
}

/// Saves the channel, then admits, verifies, publishes and activates one release, recording each
/// reached phase in `report`. The signature verifies before the manifest is parsed, every file
/// verifies before the staged Client runs, and activation follows publication. A settings file that
/// cannot be saved or read stops the run with its typed reason in `report`. A channel switch is
/// the person's choice, so it may move to a lower version.
///
/// # Errors
/// Returns the first refusal or failure; `report` keeps the phase and every committed effect.
pub(super) fn run(
    inputs: &Inputs<'_>,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    context.check()?;
    if let Some(channel) = inputs.channel {
        match orchbee_local_runtime::set_update_channel(channel, inputs.state) {
            Ok(path) => report.config_path = Some(path),
            Err(error) => return Err(report.configuration(error.into())),
        }
        report.channel_saved = true;
    }
    let selected = match configuration::read_valid(inputs.state) {
        Ok(load) => load.settings.update_channel,
        Err(error) => return Err(report.configuration(error)),
    };
    report.channel = Some(selected);
    report.phase = "source";
    // A build without a trusted key can install nothing, so it never contacts a feed.
    if inputs.keys.is_empty() {
        return Err(Error::SigningKeysUnavailable);
    }
    let source = Source::configured(inputs.manifest_source.as_deref(), selected)?;
    report.phase = "manifest";
    let (release, document) = admit(&source, inputs.keys, context)?;
    if release.channel != selected {
        return Err(Error::ManifestInvalid);
    }
    report.version = Some(release.version.clone());
    report.build_id = Some(release.build_id.clone());
    let root = inputs.install_root.clone()?;
    let completion = Completion {
        path: complete::PATH_STEP_BUILT,
        allow_downgrade: inputs.channel.is_some(),
    };
    install::admit_store(&root, &release, completion.allow_downgrade, report)?;
    report.phase = "download";
    let path = install::publish(
        &root,
        &source,
        &release,
        &document,
        inputs.probe,
        context,
        report,
    )?;
    report.phase = "activate";
    install::activate(
        &root,
        &path,
        &release,
        inputs.activation,
        completion,
        context,
        report,
    )?;
    report.phase = "complete";
    Ok(())
}

/// Admits, verifies, publishes and activates the signed release directory the running Client sits
/// in, of whichever channel it names. The lower-version refusal runs before anything is copied.
///
/// # Errors
/// Returns the first refusal or failure; `report` keeps the phase and every committed effect.
pub(super) fn install(
    inputs: &InstallInputs<'_>,
    context: &mut Context,
    report: &mut Report,
) -> Result<(), Error> {
    context.check()?;
    report.phase = "source";
    if inputs.keys.is_empty() {
        return Err(Error::SigningKeysUnavailable);
    }
    let source = Source::release_directory(&inputs.directory.clone()?)?;
    report.phase = "manifest";
    let (mut release, document) =
        admit(&source, inputs.keys, context).map_err(|error| match error {
            Error::LocalManifestRead => Error::NotReleaseDirectory,
            other => other,
        })?;
    // A release directory holds every file at its path, so no download location is consulted.
    for artifact in &mut release.files {
        artifact.url.clone_from(&artifact.path);
    }
    report.channel = Some(release.channel);
    report.version = Some(release.version.clone());
    report.build_id = Some(release.build_id.clone());
    let root = inputs.install_root.clone()?;
    install::admit_store(&root, &release, inputs.completion.allow_downgrade, report)?;
    report.phase = "copy";
    let path = install::publish(
        &root,
        &source,
        &release,
        &document,
        inputs.probe,
        context,
        report,
    )?;
    report.phase = "activate";
    install::activate(
        &root,
        &path,
        &release,
        inputs.activation,
        inputs.completion,
        context,
        report,
    )?;
    report.phase = "complete";
    Ok(())
}

/// Reads the manifest and its signature document, verifies the signature before parsing, and
/// admits the manifest for this platform. Returns the release and the signature document.
fn admit(
    source: &Source,
    keys: &[TrustedKey],
    context: &mut Context,
) -> Result<(Release, Vec<u8>), Error> {
    let bytes = source.manifest(context)?;
    let document = source.signature(context)?;
    let trusted = signature::verify(&document, &bytes, keys).map_err(|refusal| match refusal {
        Refusal::NoTrustedKeys => Error::SigningKeysUnavailable,
        Refusal::ManifestTooLarge => Error::ManifestInvalid,
        Refusal::Untrusted => Error::SignatureUntrusted,
        Refusal::Malformed | Refusal::Invalid => Error::SignatureInvalid,
    })?;
    let release =
        Release::parse(trusted, &manifest::own_platform()).ok_or(Error::ManifestInvalid)?;
    Ok((release, document))
}
