//! The install and activation engine below the CLI's release check. Every location, key and
//! command it uses arrives as a value, so tests drive it in process with the test key and fixture
//! roots while only the CLI entry layer reads the release environment.
use super::{
    Context, Error, Report,
    acquire::Source,
    install::{self, Activation, Probe},
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
    /// The admitted installation root, or why its configuration was refused.
    pub(super) install_root: Result<PathBuf, Error>,
    pub(super) probe: Probe,
    pub(super) activation: Activation,
}

/// Saves the channel, then admits, verifies, publishes and activates one release, recording each
/// reached phase in `report`. The signature verifies before the manifest is parsed, every file
/// verifies before the staged Client runs, and activation follows publication. A settings file that
/// cannot be saved or read stops the run with its typed reason in `report`.
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
    let bytes = source.manifest(context)?;
    let document = source.signature(context)?;
    let trusted =
        signature::verify(&document, &bytes, inputs.keys).map_err(|refusal| match refusal {
            Refusal::NoTrustedKeys => Error::SigningKeysUnavailable,
            Refusal::ManifestTooLarge => Error::ManifestInvalid,
            Refusal::Untrusted => Error::SignatureUntrusted,
            Refusal::Malformed | Refusal::Invalid => Error::SignatureInvalid,
        })?;
    let release = Release::parse(trusted, &manifest::own_platform())
        .filter(|release| release.channel == selected)
        .ok_or(Error::ManifestInvalid)?;
    report.version = Some(release.version.clone());
    report.build_id = Some(release.build_id.clone());
    let root = inputs.install_root.clone()?;
    report.phase = "download";
    let path = install::publish(&root, &source, &release, inputs.probe, context, report)?;
    report.phase = "activate";
    install::activate(&root, &path, inputs.activation, context)?;
    report.activated = true;
    report.phase = "complete";
    Ok(())
}
