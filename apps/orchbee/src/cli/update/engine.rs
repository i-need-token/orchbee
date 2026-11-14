//! The install and activation engine below the CLI's build checks. Every location, key and
//! command it uses arrives as a value, so tests drive it in process with the test key and fixture
//! roots while only the CLI entry layer reads the release environment.
use super::{
    Context, Error, Report,
    acquire::Source,
    archive::{Download, Unpack},
    complete,
    feed::Feed,
    install::{self, Activation, Completion, Probe, Standing, Stored},
    manifest::{self, Release},
    signature::{self, Refusal, TrustedKey, TrustedManifest},
};
use crate::cli::configuration;
use orchbee_local_runtime::UpdateChannel;
use std::{ffi::OsString, path::PathBuf};

/// Everything one update reads besides the feed itself.
pub(super) struct Inputs<'a> {
    /// The channel `channel set` saves before anything else.
    pub(super) channel: Option<UpdateChannel>,
    /// The environment through which the settings file and the data root resolve.
    pub(super) state: &'a dyn Fn(&str) -> Option<OsString>,
    /// The feed location before `{channel}` expansion; absent means unconfigured.
    pub(super) feed_source: Option<String>,
    /// The public keys a feed or manifest signature must verify against.
    pub(super) keys: &'a [TrustedKey],
    /// The admitted store root, or why its configuration was refused.
    pub(super) install_root: Result<PathBuf, Error>,
    /// Whether a loopback feed address is admitted. Only a test build passes true.
    pub(super) loopback: bool,
    pub(super) probe: Probe,
    pub(super) activation: Activation,
    /// Unpacks the verified archive into its private folder.
    pub(super) unpack: Unpack<'a>,
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

/// What the store says an update does next.
enum Next {
    /// Up to date or lower: the run is complete and nothing was requested or written.
    Done,
    /// The offered version is already stored, not active: activate it without the archive.
    Stored {
        manifest: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Download, unpack and install the archive.
    Download,
}

/// Saves the channel, then reads and verifies the signed feed and decides from it and the store
/// alone, so "up to date", a lower feed and a version conflict end before any archive request.
/// Otherwise, under the download lock and after deciding again, it activates a stored copy of the
/// offered version, or downloads the one archive bounded by its signed size, checks its size and
/// SHA-256, unpacks it and installs the unpacked release directory exactly as `install` does, which
/// verifies the release's own signature and every file again. Each reached phase is recorded in
/// `report`. A settings file that cannot be saved or read stops the run with its typed reason in
/// `report`. A channel switch is the person's choice, so it may move to a lower version.
///
/// # Errors
/// Returns the first refusal or failure; `report` keeps the phase and every committed effect. The
/// download folder holds only its lock afterwards, unless `cleanupPending` names it.
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
    let source = Source::configured(inputs.feed_source.as_deref(), selected, inputs.loopback)?;
    report.phase = "manifest";
    let feed = admit_feed(&source, inputs.keys, context)?;
    if feed.channel != selected {
        return Err(Error::ManifestInvalid);
    }
    report.version = Some(feed.version.clone());
    report.build_id = Some(feed.build_id.clone());
    let root = inputs.install_root.clone()?;
    let completion = Completion {
        path: complete::PATH_STEP_BUILT,
        allow_downgrade: inputs.channel.is_some(),
    };
    if matches!(
        decide(&root, &feed, completion, context, report)?,
        Next::Done
    ) {
        return Ok(());
    }
    report.phase = "download";
    let mut download = Download::acquire(&root, context, report)?;
    let result = (|| {
        // Another update may have finished while this one read the feed.
        let next = decide(&root, &feed, completion, context, report)?;
        let (directory, mut release, signature) = match next {
            Next::Done => return Ok(()),
            Next::Stored {
                manifest,
                signature,
            } => {
                report.phase = "verify";
                // The signed feed binds these bytes by their digest, which `decide` compared.
                let release = Release::parse(
                    TrustedManifest::installed(&manifest),
                    &manifest::own_platform(),
                )
                .ok_or(Error::ManifestInvalid)?;
                (
                    root.join("versions").join(&feed.version),
                    release,
                    signature,
                )
            }
            Next::Download => {
                let verified =
                    download.fetch(&source.artifact(&feed.archive.url)?, &feed.archive, context)?;
                let unpacked = download.unpack(verified, inputs.unpack, context)?;
                report.phase = "verify";
                let (release, signature) =
                    admit(&Source::release_directory(&unpacked)?, inputs.keys, context).map_err(
                        |error| match error {
                            // The archive's one folder is not a signed release directory.
                            Error::LocalSourceRead => Error::ManifestInvalid,
                            other => other,
                        },
                    )?;
                (unpacked, release, signature)
            }
        };
        if !feed.names(&release) {
            return Err(Error::ManifestInvalid);
        }
        let source = Source::release_directory(&directory)?;
        directory_files(&mut release);
        let path = install::publish(
            &root,
            &source,
            &release,
            &signature,
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
    })();
    download.finish(report);
    result
}

/// Decides from the verified feed and the store alone, in this order: the offered release is
/// active (up to date); it is lower than the active one without a channel switch (up to date);
/// its version is stored with another manifest (`version-conflict`); it is stored but not active;
/// otherwise it must be downloaded. Nothing is requested or written.
fn decide(
    root: &std::path::Path,
    feed: &Feed,
    completion: Completion,
    context: &mut Context,
    report: &mut Report,
) -> Result<Next, Error> {
    match install::admit_store(root, &feed.version, &feed.manifest, report)? {
        Standing::Active => {
            report.published = Some(root.join("versions").join(&feed.version));
            report.reused = true;
            report.activated = true;
            report.up_to_date = true;
            report.phase = "complete";
            return Ok(Next::Done);
        }
        Standing::Lower if !completion.allow_downgrade => {
            report.up_to_date = true;
            report.phase = "complete";
            return Ok(Next::Done);
        }
        Standing::Admitted | Standing::Lower => {}
    }
    match install::stored_version(root, &feed.version, &feed.manifest, context)? {
        Stored::Absent => Ok(Next::Download),
        Stored::Same {
            manifest,
            signature,
        } => Ok(Next::Stored {
            manifest,
            signature,
        }),
        Stored::Other => Err(Error::VersionConflict),
    }
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
            Error::LocalSourceRead => Error::NotReleaseDirectory,
            other => other,
        })?;
    directory_files(&mut release);
    report.channel = Some(release.channel);
    report.version = Some(release.version.clone());
    report.build_id = Some(release.build_id.clone());
    let root = inputs.install_root.clone()?;
    // Run from a version directory of the store, install verifies and activates that version again.
    if install::admit_store(&root, &release.version, &release.identity, report)? == Standing::Lower
        && !inputs.completion.allow_downgrade
    {
        return Err(Error::DowngradeRefused);
    }
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

/// A release directory holds every file at its path, so no download location is consulted.
fn directory_files(release: &mut Release) {
    for artifact in &mut release.files {
        artifact.url.clone_from(&artifact.path);
    }
}

/// Reads the feed and its signature document into memory, verifies the signature under the feed
/// domain before parsing, and admits the feed for this platform. Nothing touches the disk.
fn admit_feed(source: &Source, keys: &[TrustedKey], context: &mut Context) -> Result<Feed, Error> {
    let bytes = source.feed(context)?;
    let document = source.signature(context)?;
    let trusted = signature::verify_feed(&document, &bytes, keys).map_err(refused)?;
    Feed::parse(trusted, &manifest::own_platform()).ok_or(Error::ManifestInvalid)
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
    let trusted = signature::verify(&document, &bytes, keys).map_err(refused)?;
    let release =
        Release::parse(trusted, &manifest::own_platform()).ok_or(Error::ManifestInvalid)?;
    Ok((release, document))
}

fn refused(refusal: Refusal) -> Error {
    match refusal {
        Refusal::NoTrustedKeys => Error::SigningKeysUnavailable,
        Refusal::Oversized => Error::ManifestInvalid,
        Refusal::Untrusted => Error::SignatureUntrusted,
        Refusal::Malformed | Refusal::Invalid => Error::SignatureInvalid,
    }
}
